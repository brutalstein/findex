"""Explicitly gated six-session Codex subscription bugfix pilot; no API fallback."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import threading
import time
import tomllib

from bugfix_fixtures import TASKS, fixture_env, run_repro
from common import REPO, WORK, write_json
from mcp_client import McpProcess
from tool_comparison_live import codex_env, codex_subscription_error, load_results, real_usage, trial_attempt, usage_summary

OUT = REPO / 'validation/results/20261028-bugfix-live'
TOOLS = ('findex', 'graphify', 'codebase-memory')
APPROVAL_ENV = 'FINDEX_CODEX_BUGFIX_APPROVAL'
APPROVAL = 'I approve 6 Codex subscription bugfix sessions'
CAP = 6
TIMEOUT = 300
READ_TOOLS = {'findex': ['findex_context','findex_search','findex_find_symbol','findex_symbols','findex_graph_symbol','findex_impact'],
              'graphify': ['query_graph','get_node','get_neighbors'],
              'codebase-memory': ['search_graph','get_code_snippet','check_index_coverage','trace_path']}


def locations() -> dict:
    home=Path(os.environ['USERPROFILE'])
    manifest=OUT/'pilot-manifest.json'
    frozen=json.loads(manifest.read_text()).get('findex_binary','target/quality/candidate/findex.exe') if manifest.exists() else 'target/release/findex.exe'
    return {'codex':Path(os.environ.get('CODEX_APP_RUNTIME_CLI_PATH',str(home/'AppData/Local/OpenAI/Codex/bin/2993767bd1dfb009/codex.exe'))),
            'findex':REPO/frozen,
            'codebase-memory':home/'.cbm-bin/codebase-memory-mcp.exe',
            'graphify':home/'AppData/Local/Programs/Python/Python312/Scripts/graphify.exe',
            'graphify-python':home/'AppData/Local/Programs/Python/Python312/python.exe'}


def bugfix_env(env: dict) -> dict:
    # A standalone client must not inherit the parent app's tool pipe or permission context.
    return {k:v for k,v in codex_env(env).items() if not k.startswith('CODEX_') or k=='CODEX_HOME'}


def native_env(trial: dict) -> dict:
    env=bugfix_env(fixture_env((REPO/trial['root']).parent/'native-client-state'))
    # MXC owns its package temp paths under the real LocalAppData; MCP state is isolated separately.
    env['LOCALAPPDATA']=os.environ['LOCALAPPDATA']
    env['PATH']=str(locations()['graphify-python'].parent)+os.pathsep+env['PATH']
    return env


def server(trial: dict) -> dict:
    paths=locations(); root=REPO/trial['root']; tool=trial['tool']
    if tool=='findex':
        return {'command':str(paths['findex']),'args':['mcp'], 'env':{'LOCALAPPDATA':str(root.parent/'findex-state')}}
    if tool=='graphify':
        return {'command':str(paths['graphify-python']),'args':['-m','graphify.serve',str(root.parent/'graph/graphify-out/graph.json')]}
    config_file=Path(os.environ.get('CODEX_HOME',str(Path(os.environ['USERPROFILE'])/'.codex')))/'config.toml'
    registered=tomllib.loads(config_file.read_text(encoding='utf-8')).get('mcp_servers',{}).get('codebase-memory',{})
    runtime={k:v for k,v in registered.get('env',{}).items() if k in ('CBM_RUNTIME_DIR','CBM_CACHE_DIR')}
    return {'command':str(paths[tool]),'args':[], 'env':dict(runtime,LOCALAPPDATA=str(root.parent/'state'))}


def overrides(profile: dict, trial: dict) -> list[str]:
    config=dict(profile, forced_login_method='chatgpt',model_provider='openai',approval_policy='never',
                default_permissions=':workspace',
                approvals_reviewer='user', project_doc_max_bytes=0,
                **{'windows.sandbox':'mxc','features.plugins':False,'features.multi_agent':False,'features.multi_agent_v2':False})
    name=trial['tool']; settings=server(trial)
    for key,value in settings.items():
        if key=='env':
            config.update({f'mcp_servers.{name}.env.{k}':v for k,v in value.items()})
        else: config[f'mcp_servers.{name}.{key}']=value
    config.update({f'mcp_servers.{name}.enabled_tools':READ_TOOLS[name],
                   f'mcp_servers.{name}.default_tools_approval_mode':'approve',
                   f'mcp_servers.{name}.startup_timeout_sec':60})
    args=[]
    for key,value in config.items(): args+=['-c',key+'='+json.dumps(value)]
    return args


def prepare() -> None:
    if (OUT/'pilot-manifest.json').exists(): raise ValueError('Preserve frozen pilot inputs')
    oracle=json.loads((OUT/'fixture-oracles.json').read_text())
    assert len(oracle)==2 and all(v['status']=='PASS' for v in oracle.values())
    for key in TASKS:
        expected='FileExistsError' if key.startswith('requests') else 'AssertionError'
        assert expected in (OUT/'raw'/f'{key}-red.log').read_text(), 'Baseline failed for an unintended reason'
    config=Path(os.environ.get('CODEX_HOME',str(Path(os.environ['USERPROFILE'])/'.codex')))/'config.toml'
    values=tomllib.loads(config.read_text(encoding='utf-8')) if config.exists() else {}
    profile={key:values[key] for key in ('model','model_reasoning_effort','service_tier') if key in values}
    assert profile.get('model'), 'Current native model profile must be explicit before live testing'
    binary=locations()['findex']; digest=hashlib.sha256(binary.read_bytes()).hexdigest()
    frozen=WORK/'live-bugfix'/OUT.name/'bin'/digest/'findex.exe';frozen.parent.mkdir(parents=True,exist_ok=True)
    if not frozen.exists(): shutil.copy2(binary,frozen)
    assert hashlib.sha256(frozen.read_bytes()).hexdigest()==digest,'Immutable candidate mismatch'
    manifest={'authorization':'User renewed live subscription comparison after semantic coding; at most six new starts, no paid APIs or subagents. Earlier starts remain historical.',
              'max_sessions':CAP,'timeout_seconds':TIMEOUT,'profile':profile,'tasks':oracle,'trials':[],
              'findex_binary':frozen.relative_to(REPO).as_posix(),
              'tool_sha256':{k:hashlib.sha256(v.read_bytes()).hexdigest() for k,v in locations().items()},
              'limits':['Two curated real issues, one repetition per tool; not a general or statistically powered bugfix claim',
                        'Setup/index/build costs excluded from model session time and reported separately',
                        'No APIs, external providers, agents or subagents; workspace-write only']}
    for case_index,(key,spec) in enumerate(TASKS.items()):
        order=TOOLS if case_index%2==0 else TOOLS[::-1]
        for tool in order:
            project=WORK/'live-bugfix'/OUT.name/'trials'/key/tool/'project'
            print('Preparing',key,tool,flush=True)
            started=time.monotonic()
            shutil.copytree(WORK/'live-bugfix/seeds'/key,project,
                            ignore=shutil.ignore_patterns('.git','target','node_modules','graphify-out','__pycache__','*.pyc'))
            (project/'repro.py').write_text(spec['repro'],encoding='utf-8',newline='\n')
            subprocess.run(['git','init','-q',str(project)],check=True)
            subprocess.run(['git','add','.'],cwd=project,check=True,capture_output=True)
            subprocess.run(['git','-c','user.name=Findex validation','-c','user.email=validation@local.invalid','commit','-qm','Frozen bugfix fixture'],cwd=project,check=True)
            env=fixture_env(project.parent/'state')
            if spec['corpus']=='ripgrep':
                warm=subprocess.run(['cargo','build','--quiet','--locked'],cwd=project,env=env,capture_output=True,text=True,timeout=900)
                if warm.returncode: raise RuntimeError(warm.stderr[-2000:])
            trial={'key':key+':'+tool,'task':key,'tool':tool,'root':project.relative_to(REPO).as_posix(),
                   'baseline_head':subprocess.run(['git','rev-parse','HEAD'],cwd=project,capture_output=True,text=True,check=True).stdout.strip(),
                   'source_tree':subprocess.run(['git','write-tree'],cwd=project,capture_output=True,text=True,check=True).stdout.strip(),
                   'repro_sha256':hashlib.sha256((project/'repro.py').read_bytes()).hexdigest()}
            settings=server(trial)
            if tool=='graphify':
                built=subprocess.run([str(locations()['graphify']),'extract',str(project),'--code-only','--max-workers','2','--out',str(project.parent/'graph')],capture_output=True,text=True,timeout=900)
                if built.returncode: raise RuntimeError(built.stderr[-1500:])
            else:
                child=McpProcess(Path(settings['command']),project,dict(env,**settings.get('env',{})),server_args=settings['args'])
                try:
                    if tool=='findex':
                        response=child.call_tool('findex_context',{'task':'certificate extraction' if key.startswith('requests') else 'fish completions','budget_tokens':1200})
                    else:
                        trial['project_name']='findex-bugfix-'+OUT.name+'-'+key
                        response=child.call_tool('index_repository',{'repo_path':str(project),'name':trial['project_name'],'mode':'full','persistence':False})
                    if response[3]: raise RuntimeError('Native benchmark index setup failed: '+response[1][:500])
                finally: child.close()
                if tool=='findex' and spec['corpus']=='ripgrep':
                    analyzer=shutil.which('rust-analyzer')
                    if not analyzer:raise RuntimeError('rust-analyzer required for the frozen Rust semantic arm')
                    trial['semantic_provider_sha256']=hashlib.sha256(Path(analyzer).read_bytes()).hexdigest()
                    generated=subprocess.run([analyzer,'scip','.'],cwd=project,env=env,capture_output=True,text=True,timeout=900)
                    if generated.returncode:raise RuntimeError('SCIP generation failed: '+generated.stderr[-1500:])
                    imported=subprocess.run([str(locations()['findex']),'graph-import-scip','index.scip'],cwd=project,env=dict(env,**settings['env']),capture_output=True,text=True,timeout=180)
                    if imported.returncode:raise RuntimeError('SCIP import failed: '+imported.stderr[-1500:])
                    trial['semantic_imported']=True
                    trial['scip_sha256']=hashlib.sha256((project/'index.scip').read_bytes()).hexdigest()
                    (project/'index.scip').unlink()
            trial['setup_seconds']=round(time.monotonic()-started,3)
            manifest['trials'].append(trial)
    for key in TASKS:
        assert len({t['source_tree'] for t in manifest['trials'] if t['task']==key})==1
    write_json(OUT/'pilot-manifest.json',manifest)
    print('Six identical-source arms prepared; zero model calls',flush=True)


class NativeClient:
    def __init__(self, trial: dict, profile: dict, env: dict):
        self.events=queue.Queue();self.errors=[];self.seq=0
        self.proc=subprocess.Popen([str(locations()['codex']),'app-server','--stdio',*overrides(profile,trial)],
            cwd=REPO/trial['root'],env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf-8',errors='replace')
        def read():
            for line in self.proc.stdout:
                self.events.put(json.loads(line))
            self.events.put(None)
        threading.Thread(target=read,daemon=True).start()
        threading.Thread(target=lambda:self.errors.extend(self.proc.stderr),daemon=True).start()
    def request(self,method,params):
        self.seq+=1
        self.proc.stdin.write(json.dumps({'jsonrpc':'2.0','id':self.seq,'method':method,'params':params})+'\n');self.proc.stdin.flush()
        deadline=time.monotonic()+120
        while time.monotonic()<deadline:
            event=self.events.get(timeout=max(.1,deadline-time.monotonic()))
            if event is None: raise RuntimeError('Native client closed: '+''.join(self.errors)[-1200:])
            if event.get('id')==self.seq:
                if 'error' in event: raise RuntimeError(str(event['error']))
                return event['result']
        raise TimeoutError(method)
    def close(self):
        self.proc.stdin.close()
        try:self.proc.wait(timeout=20)
        except subprocess.TimeoutExpired:self.proc.kill();self.proc.wait()
        self.proc.stdout.close();self.proc.stderr.close()


def preflight() -> None:
    manifest=json.loads((OUT/'pilot-manifest.json').read_text())
    checks=[]
    for trial in manifest['trials']:
        env=native_env(trial)
        isolated_home=(REPO/trial['root']).parent/'native-codex-home'
        isolated_home.mkdir(exist_ok=True)
        env['CODEX_HOME']=str(isolated_home)
        client=NativeClient(trial,manifest['profile'],env)
        try:
            client.request('initialize',{'clientInfo':{'name':'findex-bugfix-preflight','version':'1'},'capabilities':{'experimentalApi':True}})
            started=client.request('thread/start',{'cwd':str(REPO/trial['root']),'ephemeral':True,'approvalPolicy':'never','sandbox':'workspace-write'})
            assert started['sandbox']['type']=='workspaceWrite', started['sandbox']
            root=REPO/trial['root']
            outside=str(root.parent/'permission-probe-outside.txt').replace("'","''")
            script=("$ErrorActionPreference='Stop'; [IO.File]::WriteAllText('permission-probe.txt','ok'); "
                    "if([IO.File]::ReadAllText('permission-probe.txt') -ne 'ok'){throw 'readback failed'}; "
                    "Remove-Item -LiteralPath permission-probe.txt; $blocked=$false; "
                    f"try{{[IO.File]::WriteAllText('{outside}','unexpected')}}catch{{$blocked=$true}}; "
                    f"if(!$blocked){{Remove-Item -LiteralPath '{outside}'; throw 'outside write was allowed'}}; "
                    "'WRITE_EXEC_BOUNDARY_OK'")
            command=client.request('command/exec',{'cwd':str(root),'command':['powershell.exe','-NoProfile','-Command',script],
                'sandboxPolicy':started['sandbox'],'timeoutMs':15000})
            assert command['exitCode']==0 and not command['stderr'] and 'WRITE_EXEC_BOUNDARY_OK' in command['stdout'],command
            # MXC private-temp/MSVC linking is unsupported here. The independent
            # native host grader runs the unchanged regression for every arm.
            repro=run_repro(root,fixture_env(root.parent/'grader-state'))
            expected='FileExistsError' if trial['task'].startswith('requests') else 'AssertionError'
            assert repro.returncode!=0 and expected in repro.stderr,repro.stderr[-1500:]
            tid=started['thread']['id']
            servers=client.request('mcpServerStatus/list',{'threadId':tid})['data']
            selected=next(s for s in servers if s['name']==trial['tool'])
            assert set(selected['tools'])==set(READ_TOOLS[trial['tool']]), selected
            if trial['tool']=='findex': name,args='findex_search',{'query':'extract_zipped_paths' if trial['task'].startswith('requests') else 'fish','format':'compact'}
            elif trial['tool']=='graphify': name,args='query_graph',{'question':'extract_zipped_paths' if trial['task'].startswith('requests') else 'fish','depth':2,'token_budget':1200}
            else: name,args='search_graph',{'project':trial['project_name'],'query':'extract_zipped_paths' if trial['task'].startswith('requests') else 'fish','format':'tree','limit':5}
            result=client.request('mcpServer/tool/call',{'threadId':tid,'server':trial['tool'],'tool':name,'arguments':args})
            assert result.get('isError') is False and result.get('content'),result
            checks.append({'key':trial['key'],'status':'PASS','model':started.get('model'),'tools':list(selected['tools']),
                           'effective_sandbox':started['sandbox'],'write_exec_boundary':'PASS'})
            checks[-1]['baseline_repro_executable']='NATIVE_HOST_GRADER'
        finally:
            client.close()
            (OUT/'raw'/('preflight-'+trial['key'].replace(':','-')+'.log')).write_text(''.join(client.errors),encoding='utf-8')
        print('Native no-model preflight PASS',trial['key'],flush=True)
    write_json(OUT/'native-preflight-v2.json',{'checks':checks,'model_sessions':0,
               'codex_sha256':hashlib.sha256(locations()['codex'].read_bytes()).hexdigest()})


def execution_record(code: int, events: list[dict], tool: str) -> dict:
    items=[e['item'] for e in events if e.get('type')=='item.completed' and isinstance(e.get('item'),dict)]
    usage=next((e.get('usage') or {} for e in reversed(events) if e.get('type')=='turn.completed'),{})
    calls=[item for item in items if item.get('type')=='mcp_tool_call' and item.get('server')==tool]
    successful=[item for item in calls if item.get('status')=='completed' and not item.get('error')
                and isinstance(item.get('result'),dict) and not item['result'].get('isError')
                and 'requires approval' not in json.dumps(item).lower()]
    agents=[item for item in items if 'collab' in item.get('type','')]
    final=next((item.get('text','') for item in reversed(items) if item.get('type')=='agent_message'),'')
    blocked='read-only' in final.lower() and not any(item.get('type')=='file_change' for item in items)
    return {'exit_code':code,'usage':usage,'mcp_calls':len(successful),'attempted_mcp_calls':len(calls),
            'valid':code==0 and real_usage(usage) and bool(successful) and not agents and not blocked,
            'infrastructure_failure':'reported_read_only' if blocked else None,'final':final}


def validate_patch(trial: dict, env: dict, raw: Path) -> dict:
    root=REPO/trial['root']; spec=TASKS[trial['task']]
    head=subprocess.run(['git','rev-parse','HEAD'],cwd=root,capture_output=True,text=True,check=True).stdout.strip()
    modified=subprocess.run(['git','diff','--name-only','HEAD'],cwd=root,capture_output=True,text=True,check=True).stdout.splitlines()
    added=subprocess.run(['git','ls-files','--others','--exclude-standard'],cwd=root,capture_output=True,text=True,check=True).stdout.splitlines()
    changed=sorted(set(modified+added))
    intact=hashlib.sha256((root/'repro.py').read_bytes()).hexdigest()==trial['repro_sha256']
    allowed=bool(changed) and all(path.startswith(spec['source_prefix']) and not (root/path).is_symlink() for path in changed)
    patch=subprocess.run(['git','diff','HEAD','--',spec['source_prefix']],cwd=root,capture_output=True,text=True,check=True).stdout
    (raw/'patch.diff').write_text(patch,encoding='utf-8')
    if head!=trial['baseline_head'] or not intact or not allowed:
        return {'correct':False,'changed_files':changed,'fixture_intact':intact,'source_only':allowed,'reason':'No source fix or fixture/metadata was changed'}
    grading_started=time.monotonic()
    tested=run_repro(root,env)
    (raw/'regression.log').write_text(tested.stdout+tested.stderr,encoding='utf-8')
    return {'correct':tested.returncode==0,'changed_files':changed,'fixture_intact':True,'source_only':True,
            'regression_exit':tested.returncode,'grader_seconds':round(time.monotonic()-grading_started,3),
            'grader_execution':'independent-native-windows-host',
            'scope':'Unchanged real-issue reproduction; not the full upstream test suite'}


def run_live() -> None:
    if os.environ.get('CI') or os.environ.get('GITHUB_ACTIONS'):
        raise SystemExit('BLOCKED: live model sessions never run in CI')
    manifest=json.loads((OUT/'pilot-manifest.json').read_text())
    preflight=json.loads((OUT/'native-preflight-v2.json').read_text())
    assert len(preflight['checks'])==CAP and all(c['status']=='PASS' for c in preflight['checks'])
    assert all(c['write_exec_boundary']=='PASS' for c in preflight['checks'])
    assert all(c['baseline_repro_executable']=='NATIVE_HOST_GRADER' for c in preflight['checks'])
    assert preflight['codex_sha256']==manifest['tool_sha256']['codex'], 'Preflight and frozen live client differ'
    assert len(manifest['trials'])==manifest['max_sessions']==CAP
    for key,path in locations().items():
        if hashlib.sha256(path.read_bytes()).hexdigest()!=manifest['tool_sha256'][key]:
            raise SystemExit('BLOCKED: a frozen native tool changed')
    clean=codex_env(dict(os.environ))
    login=subprocess.run([str(locations()['codex']),'login','status'],env=clean,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=30)
    problem=codex_subscription_error(login.returncode,login.stdout+login.stderr)
    if problem: raise SystemExit(problem)
    saved=OUT/'live-results.json'; results=load_results(saved)
    for trial in manifest['trials']:
        if any(row['key']==trial['key'] for row in results['sessions']): continue
        root=REPO/trial['root']; tool=trial['tool'];spec=TASKS[trial['task']]
        raw=OUT/'raw/live'/trial['key'].replace(':','-');raw.mkdir(parents=True,exist_ok=True)
        env=native_env(trial)
        prompt=(spec['prompt'].replace('Run python repro.py. ','')+' Start by navigating with the supplied '+tool+' MCP server, then verify source and edit the application. '
                'Use built-in source reading, editing and local test commands as needed. Do not use the network, install packages, '
                'read secrets/authentication, launch agents/subagents/other model clients, or edit outside this workspace. '
                'Do not commit, reset, stage, delete tests or change metadata. Finish with a short account of the fix and actual tests. ')
        prompt+='For this campaign do not run repro.py or compile: the independent native Windows host grader runs the unchanged reproduction after your patch. Sandbox private-temp and MSVC linking are unsupported. State that tests are left to the host grader. '
        if tool=='findex': prompt+='Prefer findex_context with budget_tokens=1200; use compact for search/symbol lookups. '
        if trial.get('semantic_imported'):prompt+='Compiler SCIP evidence is imported. findex_graph_symbol accepts path + one-based line + zero-based UTF-8 byte column for semantic disambiguation; references are not proven calls. '
        if tool=='codebase-memory': prompt+='Use existing project '+trial['project_name']+'; prefer tree search output, do not reindex. '
        argv=[str(locations()['codex']),'exec','--ignore-user-config','--ephemeral','--json','--sandbox','workspace-write','-C',str(root),*overrides(manifest['profile'],trial),prompt]
        record={'key':trial['key'],'client':'codex','task':trial['task'],'tool':tool}
        print('Starting authorized trial',trial['key'],flush=True)
        with trial_attempt(saved,results,record,CAP):
            started=time.monotonic()
            try:
                completed=subprocess.run(argv,env=env,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=TIMEOUT)
            except subprocess.TimeoutExpired as exc:
                (raw/'timeout.log').write_text(str(exc),encoding='utf-8')
                raise
            (raw/'events.jsonl').write_text(completed.stdout,encoding='utf-8')
            (raw/'stderr.txt').write_text(completed.stderr,encoding='utf-8')
            events=[]
            for line in completed.stdout.splitlines():
                try:events.append(json.loads(line))
                except ValueError:pass
            record.update(execution_record(completed.returncode,events,tool),seconds=round(time.monotonic()-started,3))
            if record['valid']: record.update(validate_patch(trial,env,raw))
        write_json(OUT/'live-usage-summary.json',{'summary':usage_summary(results),'authorized_new_starts':CAP,
            'actual_new_starts':results['started'],'prior_historical_starts':json.loads((OUT/'authorization.json').read_text()).get('earlier_started_sessions',12) if (OUT/'authorization.json').exists() else 12,
            'limits':'Two curated real issues, one repetition per tool; regression-only success, not the full upstream suite or statistical/general superiority. All source-based setup is excluded from model usage.'})
        print(trial['key'],'valid=',record['valid'],'regression_pass=',record.get('correct'),flush=True)
        if results.get('stopped'):break


def main() -> None:
    global OUT
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode',choices=('prepare','preflight','run'))
    parser.add_argument('--out',type=Path,default=OUT)
    parser.add_argument('--enable-live-model-tests',action='store_true')
    args=parser.parse_args()
    OUT=args.out.resolve()
    if not OUT.is_relative_to((REPO/'validation/results').resolve()):
        raise SystemExit('BLOCKED: campaign output must stay inside validation/results')
    if args.mode=='prepare':prepare();return
    if args.mode=='preflight':preflight();return
    if not args.enable_live_model_tests or os.environ.get(APPROVAL_ENV)!=APPROVAL:
        raise SystemExit('NOT_RUN: explicit six-session subscription authorization and live flag required')
    run_live()


if __name__=='__main__':main()
