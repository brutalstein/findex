import hashlib
import json
import os
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'validation/harness'))
from common import CommandLog, environment, sanitize, write_json

out = root / 'validation/results/20261030-retrieval-integrity/native'
out.mkdir()
env = dict(os.environ, TEMP=str(root/'target/retrieval-integrity/tmp'),
           TMP=str(root/'target/retrieval-integrity/tmp'),
           LOCALAPPDATA=str(root/'target/retrieval-integrity/private-state'))
log = CommandLog(out)
commands = [
    ('fmt', ['cargo', 'fmt', '--all', '--', '--check']),
    ('clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings']),
    ('rust-tests', ['cargo', 'test', '--workspace', '--all-targets']),
    ('release', ['cargo', 'build', '--release', '-p', 'findex']),
    ('python-tests', [sys.executable, '-m', 'unittest', 'discover', '-s', 'validation/harness', '-p', 'test_*.py']),
]
results = []
write_json(out/'environment.json', environment())
for name, command in commands:
    result = log.run(command, cwd=root, env=env, timeout=1200)
    (out/f'{name}.txt').write_text(sanitize(result.stdout + result.stderr), encoding='utf-8')
    results.append({'gate':name, 'exit_code':result.returncode, 'status':'PASS' if result.returncode == 0 else 'FAIL'})
    write_json(out/'gates.json', results)
    print(name, result.returncode, (result.stdout + result.stderr)[-800:], flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
write_json(out/'binary.json', {'sha256':hashlib.sha256((root/'target/release/findex.exe').read_bytes()).hexdigest()})
