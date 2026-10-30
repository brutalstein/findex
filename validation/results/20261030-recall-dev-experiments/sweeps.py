import sys; sys.path.insert(0,'validation/results/20261030-recall-dev-experiments')
from replay import *
rows=load()
def metrics(order_fn, rs):
    h1=h3=h10=0; rec=0
    for r in rs:
        order=order_fn(r)[:32]; g=set(r['gold'])
        rk=grank(order,g)
        h1+= rk==1; h3+= bool(rk and rk<=3); h10+= bool(rk and rk<=10)
        rec+= len(set(order[:10])&g)/len(g)
    n=len(rs); return f"h1={h1/n:.3f} h3={h3/n:.3f} h10={h10/n:.3f} rec10={rec/n:.3f}"
actual=lambda r: r['ranked'] and None
V2=[r for r in rows if r['set']=='v2']; V1=[r for r in rows if r['set']=='v1']
base=lambda r: rank_concept(r['candidates'])
print('concept-only all65 ', metrics(base,rows))
print('  v2-dev(28)       ', metrics(base,V2)); print('  v1-dev(37)       ', metrics(base,V1))
cr=[r for r in rows if r['ranked']]
print('concept-routed(52) ', metrics(base,cr))
ir=[r for r in rows if not r['ranked']]
print('ident-routed(13) concept-only', metrics(base,ir))
# raw bm25 order
raw=lambda r: [c['path'] for c in r['candidates']]
print('raw bm25 all65     ', metrics(raw,rows))
# sweeps
for b in (0.75,0.5,0.3,0.1,0.0):
    print('b',b, metrics(lambda r: rank_concept(r['candidates'],b=b),rows))
for name,sc in {
 'no-filename':lambda c,m:c['bm25']*(1+c['symbol_ratio'])*weight(c['path'])*ladj(c['size'],m),
 'no-symbol':lambda c,m:c['bm25']*(1+2*c['filename'])*weight(c['path'])*ladj(c['size'],m),
 'no-pathweight':lambda c,m:c['bm25']*(1+2*c['filename'])*(1+c['symbol_ratio'])*ladj(c['size'],m),
 'filename x1':lambda c,m:c['bm25']*(1+c['filename'])*(1+c['symbol_ratio'])*weight(c['path'])*ladj(c['size'],m),
 'filename x4':lambda c,m:c['bm25']*(1+4*c['filename'])*(1+c['symbol_ratio'])*weight(c['path'])*ladj(c['size'],m),
 'symbol x2':lambda c,m:c['bm25']*(1+2*c['filename'])*(1+2*c['symbol_ratio'])*weight(c['path'])*ladj(c['size'],m),
 'symbol x3':lambda c,m:c['bm25']*(1+2*c['filename'])*(1+3*c['symbol_ratio'])*weight(c['path'])*ladj(c['size'],m),
}.items():
    print(name, metrics(lambda r: rank_concept(r['candidates'],score=sc),rows))
