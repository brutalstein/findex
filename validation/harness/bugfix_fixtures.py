"""Two curated real upstream bug fixes; local red/green oracles, no model calls."""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys

from common import REPO, WORK, write_json

REQUESTS_REPRO = r'''
import errno, os, pathlib, tempfile, zipfile
from unittest.mock import patch
from requests.utils import extract_zipped_paths

with tempfile.TemporaryDirectory() as folder:
    root=pathlib.Path(folder); dest=root/'output'; dest.mkdir()
    archive=root/'bundle.zip'; member='package/certs/ca.pem'; content=b'certificate-fixture\n'
    with zipfile.ZipFile(archive,'w') as stream: stream.writestr(member,content)
    original=os.makedirs; fired=[False]
    def race(path,*args,**kwargs):
        if str(path).startswith(str(dest)) and not fired[0]:
            fired[0]=True; original(path,*args,**kwargs)
            raise FileExistsError(errno.EEXIST,'directory created concurrently',str(path))
        return original(path,*args,**kwargs)
    with patch('tempfile.gettempdir',return_value=str(dest)), patch('os.makedirs',side_effect=race):
        result=pathlib.Path(extract_zipped_paths(str(archive)+os.sep+member))
    assert result.read_bytes()==content,'extracted content was lost or corrupted'
print('PASS: concurrent directory creation does not break ZIP certificate extraction')
'''.lstrip()

FISH_REPRO = r'''
import pathlib,re,subprocess
root=pathlib.Path(__file__).resolve().parent
result=subprocess.run(['cargo','run','--quiet','--locked','--','--generate','complete-fish'],cwd=root,capture_output=True,text=True,check=True)
entries=[line.split(' -d ',1)[0] for line in result.stdout.splitlines() if line.startswith('complete ')]
for name in ('no-ignore','no-hidden','no-ignore-dot'):
    token=re.escape(name)
    matches=[line for line in entries if re.search(r"(?:^|\s)-l\s+(?:'"+token+"'|\""+token+"\"|"+token+r")(?=\s|$)",line)]
    assert len(matches)==1,(name,'must have exactly one fish long-option declaration')
    assert not re.search(r'(^|\s)-s\s',matches[0]),(name,'negative option inherited a short alias')
assert not any('!LONG!' in line or '!SHORT!' in line or '!DOC!' in line for line in entries)
print('PASS: negative fish options use -l and do not inherit short aliases')
'''.lstrip()

TASKS = {
    'requests-5223': {'corpus': 'requests', 'source_prefix': 'requests/', 'repro': REQUESTS_REPRO,
        'prompt': 'ZIP certificate extraction can race when another process creates a nested destination directory after its existence check. Make extraction tolerate that race and preserve correct file bytes. Keep missing-member/non-archive behavior. Run python repro.py. Modify application source only; do not change repro.py, metadata or lockfiles.'},
    'ripgrep-2659': {'corpus': 'ripgrep', 'source_prefix': 'crates/core/', 'repro': FISH_REPRO,
        'prompt': 'Generated fish completions do not correctly declare negated long options. Each negated option must use fish long-option syntax, and must not inherit a short alias from its positive flag. Preserve other completions. Run python repro.py. Modify application source only; do not change repro.py, metadata or lockfiles.'},
}


def fixture_env(state: Path) -> dict:
    compiler = subprocess.run(['rustup','which','--toolchain','stable','rustc'],capture_output=True,text=True,check=True).stdout.strip()
    state.mkdir(parents=True,exist_ok=True)
    return dict(os.environ, RUSTC=compiler, PATH=str(Path(compiler).parent)+os.pathsep+os.environ['PATH'], LOCALAPPDATA=str(state))


def run_repro(project: Path, env: dict, timeout: int = 900) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable,'repro.py'],cwd=project,env=env,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=timeout)


def prepare() -> None:
    out=REPO/'validation/results/20261028-bugfix-live'
    if (out/'fixture-oracles.json').exists():
        raise ValueError('Preserve fixture verification; use existing validated seeds')
    out.mkdir(parents=True,exist_ok=True)
    rows=[json.loads(line) for line in (REPO/'validation/golden/issue_localization.jsonl').read_text().splitlines()]
    results={}
    for key,spec in TASKS.items():
        task=next(row for row in rows if row['id']==key)
        project=WORK/'live-bugfix/seeds'/key
        project.parent.mkdir(parents=True,exist_ok=True)
        if project.exists(): raise ValueError('Seed already exists; preserve attempts')
        subprocess.run(['git','-C',str(WORK/'history'/spec['corpus']),'worktree','add','--detach','-q',str(project),task['base_commit']],check=True)
        (project/'repro.py').write_text(spec['repro'],encoding='utf-8',newline='\n')
        env=fixture_env(WORK/'live-bugfix/oracle-state'/key)
        print(key,'baseline reproduction',flush=True)
        before=run_repro(project,env)
        raw=out/'raw';raw.mkdir(exist_ok=True)
        (raw/(key+'-red.log')).write_text(before.stdout+before.stderr,encoding='utf-8')
        if before.returncode==0: raise RuntimeError('Baseline did not reproduce the bug')
        patch=subprocess.run(['git','-C',str(WORK/'history'/spec['corpus']),'diff',task['base_commit'],task['fix_commit'],'--',*task['gold_files']],capture_output=True,check=True).stdout
        subprocess.run(['git','apply','-'],input=patch,cwd=project,check=True)
        print(key,'upstream fix reproduction',flush=True)
        after=run_repro(project,env)
        (raw/(key+'-green.log')).write_text(after.stdout+after.stderr,encoding='utf-8')
        if after.returncode: raise RuntimeError((after.stdout+after.stderr)[-2000:])
        subprocess.run(['git','apply','-R','-'],input=patch,cwd=project,check=True)
        assert not subprocess.run(['git','diff','--name-only'],cwd=project,capture_output=True,text=True,check=True).stdout.strip()
        results[key]={'base_commit':task['base_commit'],'upstream_fix_commit':task['fix_commit'],'baseline_exit':before.returncode,'upstream_fix_exit':after.returncode,'status':'PASS','scope':'Curated real issue; public offline reproduction, not an unseen broad bugfix benchmark'}
        write_json(out/'fixture-oracles.json',results)
    print('Both frozen base failures and upstream-fix passes verified; zero model calls',flush=True)


if __name__=='__main__':
    prepare()
