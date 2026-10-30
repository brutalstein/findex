import sys; sys.path.insert(0,'validation/results/20261030-recall-dev-experiments')
from replay import *
rows=load(); V2=[r for r in rows if r['set']=='v2']; V1=[r for r in rows if r['set']=='v1']
def met(fn, rs):
    h1=h10=0; rec=0
    for r in rs:
        order=fn(r)[:32]; g=set(r['gold']); rk=grank(order,g)
        h1+= rk==1; h10+= bool(rk and rk<=10); rec+=len(set(order[:10])&g)/len(g)
    n=len(rs); return f"h1={h1/n:.3f} h10={h10/n:.3f} r10={rec/n:.3f}"
variants={'baseline':lambda r:rank_concept(r['candidates']),
 'b=0.1':lambda r:rank_concept(r['candidates'],b=0.1),
 'symbol x3':lambda r:rank_concept(r['candidates'],score=lambda c,m:c['bm25']*(1+2*c['filename'])*(1+3*c['symbol_ratio'])*weight(c['path'])*ladj(c['size'],m))}
for k,fn in variants.items(): print(f"{k:10s} v1-dev {met(fn,V1)} | v2-dev {met(fn,V2)}")
