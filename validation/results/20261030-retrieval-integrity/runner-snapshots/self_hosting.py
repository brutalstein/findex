import os
from pathlib import Path
import sys

root=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(root/'validation/harness'))
from common import write_json
from mcp_client import McpProcess

out=root/'validation/results/20261030-retrieval-integrity/navigation/self-hosting.json'
assert not out.exists()
client=McpProcess(root/'target/release/findex.exe',root,dict(os.environ,LOCALAPPDATA=str(root/'target/retrieval-integrity/self-state')))
records=[]
try:
    for task,path in [('Where is `enqueue_event` defined?','crates/findex-watch/src/lib.rs'),
                      ('Where is `load_symbol_sites` defined?','crates/findex-relations/src/lib.rs'),
                      ('Where is `search_ranked_in_db` defined?','crates/findex-index/src/lib.rs')]:
        for budget in (200,500,1200,2000):
            payload,text,seconds,error=client.call_tool('findex_context',{'task':task,'budget_tokens':budget})
            assert not error,text
            assert len(text)/4<=budget,(task,budget,len(text))
            assert path in text or (budget<1200 and 'TRUNCATED' in text),(task,budget,text)
            records.append({'task':task,'budget':budget,'chars':len(text),'seconds':seconds,'has_source':path in text,'text':text})
            write_json(out,{'status':'PASS' if len(records)==12 else 'INCOMPLETE','queries':records,'scope':'Candidate MCP on its own working source. Small budgets may omit whole definition bodies, with explicit truncation.'})
finally:
    client.close()
print('12 self-hosted source-location/budget checks PASS',flush=True)
