from contextlib import ExitStack
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root/'validation/harness'))
from common import write_json
from mcp_client import McpProcess
from payload_cost import expand_hits

out = root/'validation/results/20261030-retrieval-integrity/navigation'
out.mkdir()
queries = [json.loads(line) for line in (root/'validation/golden/symbol_navigation.jsonl').read_text(encoding='utf-8').splitlines()]
queries = [q for q in queries if q['split']=='heldout' and q['corpus'] in ('requests','zod','fmt','ripgrep')]
binaries = {'baseline':root/'target/retrieval-integrity/baseline/findex.exe', 'candidate':root/'target/release/findex.exe'}
write_json(out/'manifest.json', {'binary_sha256':{name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in binaries.items()}, 'queries':queries, 'model_calls':0})
rows=[]
for corpus in ('requests','zod','fmt','ripgrep'):
    project=root/'target/tool-comparison/corpora'/corpus
    commit=subprocess.run(['git','rev-parse','HEAD'],cwd=project,capture_output=True,text=True,check=True).stdout.strip()
    selected=[q for q in queries if q['corpus']==corpus]
    assert {q['commit'] for q in selected} == {commit}
    with ExitStack() as stack:
        clients={}
        for name,binary in binaries.items():
            env=dict(os.environ,LOCALAPPDATA=str(root/'target/retrieval-integrity/navigation-state'/name/corpus))
            client=McpProcess(binary,project,env)
            stack.callback(client.close)
            clients[name]=client
        for query in selected:
            exact=query['type']=='symbol_exact'
            tool='findex_find_symbol' if exact else 'findex_search'
            args={'name' if exact else 'query':query['name'] if exact else query['query'], 'limit':10, 'format':'compact'}
            hits={}
            for name,client in clients.items():
                payload,text,seconds,error=client.call_tool(tool,args)
                assert not error,(query['id'],name,text)
                hits[name]=expand_hits(payload)['hits']
            assert hits['baseline']==hits['candidate'],query['id']
            rows.append({'id':query['id'],'same_hits':True})
    print(corpus,len(selected),'unchanged',flush=True)
write_json(out/'results.json',{'status':'PASS','controls':len(rows),'rows':rows})

env=dict(os.environ,LOCALAPPDATA=str(root/'target/retrieval-integrity/self-state'))
client=McpProcess(binaries['candidate'],root,env)
try:
    records=[]
    for task,path in [('Where is `enqueue_event` defined?','crates/findex-watch/src/lib.rs'),
                      ('Where is `load_symbol_sites` defined?','crates/findex-relations/src/lib.rs'),
                      ('Where is `search_ranked_in_db` defined?','crates/findex-index/src/lib.rs')]:
        for budget in (200,500,1200,2000):
            payload,text,seconds,error=client.call_tool('findex_context',{'task':task,'budget_tokens':budget})
            assert not error,text
            assert path in text,(task,budget,text)
            assert len(text)/4 <= budget,(task,budget,len(text))
            records.append({'task':task,'budget':budget,'chars':len(text),'seconds':seconds,'text':text})
    write_json(out/'self-hosting.json',{'status':'PASS','queries':records,'scope':'This candidate MCP binary on its own working source; no model sessions'})
finally:
    client.close()
print('Self-hosted Findex: 12 source-location/budget checks PASS',flush=True)
