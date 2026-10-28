"""Native compiler-backed position-resolution smoke; no models or paid APIs.

Six hand-labelled Rust cases, not a competitor or general accuracy benchmark.
Requires an explicitly selected installed rust-analyzer with the SCIP subcommand.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from bugfix_fixtures import fixture_env
from common import REPO, WORK, CommandLog, write_json
from mcp_client import McpProcess

SOURCE = '''pub struct Disk;
pub struct Socket;
impl Disk { pub fn flush(&self) {} }
impl Socket { pub fn flush(&self) {} }
pub use Disk as Storage;
pub fn aliases() {
    let item = Storage;
    item.flush(); // disk alias
    {
        let item = Socket;
        item.flush(); // shadowed socket
    }
    item.flush(); // outer disk
}
pub trait Flush { fn flush(&self); }
impl Flush for Disk { fn flush(&self) {} }
pub fn generic<T: Flush>(item: T) {
    item.flush(); // trait declaration
}
pub fn function() {}
pub use function as alias;
pub fn functions() {
    alias(); // function alias
    let alias = || {};
    alias(); // local closure
}
'''
CASES = [('disk alias', 3, 'flush'), ('shadowed socket', 4, 'flush'),
         ('outer disk', 3, 'flush'), ('trait declaration', 15, 'flush'),
         ('function alias', 20, 'alias'), ('local closure', 24, 'alias')]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=REPO/'target/release/findex.exe')
    parser.add_argument('--rust-analyzer', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    binary, analyzer, out = args.binary.resolve(), args.rust_analyzer.resolve(), args.out.resolve()
    root = (WORK/'semantic-position'/out.name).resolve()
    if not root.is_relative_to(WORK.resolve()):
        raise ValueError('Fixture must stay inside the validation workspace')
    if root.exists() or out.exists():
        raise ValueError('Preserve attempted fixtures and results; choose a fresh output directory')
    (root/'src').mkdir(parents=True)
    (root/'src/lib.rs').write_text(SOURCE, encoding='utf-8', newline='\n')
    (root/'Cargo.toml').write_text('[package]\nname="semantic-fixture"\nversion="0.1.0"\nedition="2021"\n[workspace]\n', encoding='utf-8')
    out.mkdir(parents=True)
    log = CommandLog(out)
    env = dict(fixture_env(root/'state'), CARGO_TARGET_DIR=str(root/'target'), CARGO_NET_OFFLINE='true')
    write_json(out/'manifest.json', {'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
        'analyzer_sha256':hashlib.sha256(analyzer.read_bytes()).hexdigest(),
        'source_sha256':hashlib.sha256(SOURCE.encode()).hexdigest(), 'cases':CASES,
        'model_calls':0, 'paid_api_calls':0, 'limits':'Six authored regression cases; compiler-produced SCIP, not a broad accuracy or competitor study'})
    for command in [['git','init','-q',str(root)], [str(analyzer),'scip','.'],
                    [str(binary),'graph-import-scip','index.scip']]:
        result = log.run(command, cwd=root, env=env, timeout=180)
        if result.returncode:
            raise RuntimeError('Native setup failed: '+result.stderr[-1500:])
    rows=[]
    client=McpProcess(binary,root,env)
    try:
        for marker, definition, token in CASES:
            lines=SOURCE.splitlines()
            line=next(i+1 for i,text in enumerate(lines) if '// '+marker in text)
            column=len(lines[line-1][:lines[line-1].index(token)].encode())+1
            result=log.run([str(binary),'graph-symbol','--at','src/lib.rs',str(line),str(column)],cwd=root,env=env,timeout=30)
            if result.returncode:
                raise RuntimeError('Position lookup failed: '+result.stderr[-1500:])
            report=json.loads(result.stdout)
            definitions=[s['line'] for s in (report.get('evidence') or {}).get('sites',[]) if s['kind']=='definition']
            mcp,text,_,error=client.call_tool('findex_graph_symbol',{'path':'src/lib.rs','line':line,'column':column})
            passed=report['resolution']=='resolved' and definitions==[definition] and not error and mcp==report
            rows.append({'case':marker,'line':line,'column':column,'gold_definition_line':definition,'pass':passed,'result':report})
            write_json(out/'results.json',{'status':'PASS' if len(rows)==len(CASES) and all(r['pass'] for r in rows) else 'INCOMPLETE_OR_FAIL',
                'cases':rows,'scip_sha256':hashlib.sha256((root/'index.scip').read_bytes()).hexdigest(), 'model_calls':0,'paid_api_calls':0})
            assert passed,(marker,report,text)
            print('Compiler + CLI + MCP PASS:',marker,flush=True)
    finally:
        client.close()


if __name__=='__main__':
    main()
