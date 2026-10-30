import json,sys
sys.path.insert(0,'validation/harness')
CODE={"c","cc","cjs","cpp","cs","cxx","go","h","hh","hpp","hxx","java","js","jsx","kt","lua","m","mjs","mm","php","ps1","py","pyi","rb","rs","scala","sh","swift","ts","tsx"}
def is_code(p): return '.' in p and p.rsplit('.',1)[1].lower() in CODE
def is_test(p):
    l=p.lower(); n=l.rsplit('/',1)[-1]
    return any(x in ("test","tests","__tests__","spec","specs","testing") for x in l.split('/')) or n.startswith("test_") or any(m in n for m in (".test.",".spec.","_test.","_tests."))
VEND={"vendor","vendored","_vendor","third_party","third-party","thirdparty","extern","external","deps","node_modules","site-packages"}
AUX={"doc","docs","documentation","support","example","examples","sample","samples","benchmark","benchmarks","bench","scripts"}
def dirs(p): return [d.lower() for d in (p.rsplit('/',1)[0] if '/' in p else '').split('/')]
def is_vend(p): return any(d in VEND or (i>0 and d=="packages") for i,d in enumerate(dirs(p)))
def is_aux(p): return any(d in AUX for d in dirs(p))
def weight(p): return 0.5 if is_vend(p) else 0.7 if is_aux(p) else 1.0
def ladj(length,mean,b=0.3):
    r=length/mean if mean>0 else 1.0
    norm=lambda b:1.2*(1-b+b*r)
    return (1+norm(0.75))/(1+norm(b))
def rank_concept(cands,score=None,b=0.3):
    mean=sum(c['size'] for c in cands)/max(len(cands),1)
    sc=score or (lambda c,m: c['bm25']*(1+2*c['filename'])*(1+c['symbol_ratio'])*weight(c['path'])*ladj(c['size'],m,b))
    key=lambda c:((not is_code(c['path']),is_test(c['path'])),sc(c,mean),c['path'])
    return [c['path'] for c in sorted(cands,key=key)]
def load(tags=('v2','v1')):
    rows=[]
    for t in tags:
        d=json.load(open(f'validation/results/20261030-recall-{t}-dev-p/candidate-recall.json'))
        for r in d['per_task']:
            r['set']=t; r['files']=r['candidates']['files']; r['df']=r['candidates']['df']; r['candidates']=r['candidates']['rows']; rows.append(r)
    return rows
def grank(order,gold,n=32):
    for i,p in enumerate(order[:n]):
        if p in gold: return i+1
    return None
if __name__=='__main__':
    rows=load(); ok=bad=0
    for r in rows:
        if not r['ranked']: continue
        order=rank_concept(r['candidates']); g=set(r['gold'])
        mine=grank(order,g); 
        if mine==r['final_rank']: ok+=1
        else:
            bad+=1; print('MISMATCH',r['id'],'rust',r['final_rank'],'py',mine, 'rust_top',r['ranked'][:3],'py_top',order[:3])
    print('match',ok,'mismatch',bad)
