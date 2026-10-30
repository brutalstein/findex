import sys,math; sys.path.insert(0,'validation/results/20261030-recall-dev-experiments')
from replay import *
rows=load(); V2=[r for r in rows if r['set']=='v2']; V1=[r for r in rows if r['set']=='v1']
def met(fn, rs):
    h1=h3=h10=0; rec=0
    for r in rs:
        order=fn(r)[:32]; g=set(r['gold']); rk=grank(order,g)
        h1+= rk==1; h3+= bool(rk and rk<=3); h10+= bool(rk and rk<=10); rec+=len(set(order[:10])&g)/len(g)
    n=len(rs); return (h1/n,h3/n,h10/n,rec/n)
def show(name,fn):
    a=met(fn,rows); b=met(fn,V1); c=met(fn,V2)
    print(f"{name:26s} all h1={a[0]:.3f} h3={a[1]:.3f} h10={a[2]:.3f} r10={a[3]:.3f} | v1 h1={b[0]:.3f} r10={b[3]:.3f} | v2 h1={c[0]:.3f} r10={c[3]:.3f}")
def idf(r,t):
    df=r['df'].get(t,0); N=r['files']
    return math.log((N-df+0.5)/(df+0.5)+1)
def cov(r,c):
    tot=sum(idf(r,t) for t in r['df'] if r['df'][t]>0)
    return sum(idf(r,t) for t in c['matched'])/tot if tot else 0
def coord(r,c):
    n=len([t for t in r['df'] if r['df'][t]>0]); return len(c['matched'])/n if n else 0
def with_factor(fac):
    def fn(r):
        mean=sum(c['size'] for c in r['candidates'])/max(len(r['candidates']),1)
        cs=r['candidates']
        key=lambda c:((not is_code(c['path']),is_test(c['path'])),
            c['bm25']*(1+2*c['filename'])*(1+c['symbol_ratio'])*weight(c['path'])*ladj(c['size'],mean)*fac(r,c),c['path'])
        return [c['path'] for c in sorted(cs,key=key)]
    return fn
show('baseline',with_factor(lambda r,c:1.0))
for g in (0.5,1,2,4):
    show(f'idfcov 1+{g}*cov',with_factor(lambda r,c,g=g:1+g*cov(r,c)))
for g in (0.5,1,2):
    show(f'coord 1+{g}*coord',with_factor(lambda r,c,g=g:1+g*coord(r,c)))
for p in (1,2):
    show(f'cov^{p}',with_factor(lambda r,c,p=p:(0.1+cov(r,c))**p))
