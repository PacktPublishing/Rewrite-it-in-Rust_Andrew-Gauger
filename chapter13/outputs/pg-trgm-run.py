import subprocess, re, sys, statistics
DATA="/Users/andy/Users/andy/rewrite-it-rust/part 2/crate/.pgdata"
exp={}
for l in open('/tmp/pgtrgm/expected.tsv'):
    q,k=l.rstrip('\n').split('\t'); exp[q]=k
def sql(q):
    ql=q.strip().lower(); assert "'" not in ql and '%' not in ql and '_' not in ql
    return f"""
select lower(text) like '{ql}%' as x from products limit 0;
"""
def where(ql):
    return (f"lower(text) like '%{ql}%' or lower(detail) like '%{ql}%' or lower(keywords) like '%{ql}%'")
def order(ql):
    return (f"case when lower(text) like '{ql}%' then 0 when ' '||lower(text) like '% {ql}%' then 1 "
            f"when lower(text) like '%{ql}%' then 2 else 3 end, key")
def run(script):
    return subprocess.run(["psql","-h",DATA,"-p","5433","-U","postgres","-d","trgm_scratch","-At","-q","-v","ON_ERROR_STOP=1"],
        input=script,capture_output=True,text=True,check=True).stdout
rows=[]
for q in exp:
    ql=q.strip().lower()
    stmt=f"select key from products where {where(ql)} order by {order(ql)} limit 12"
    res={}
    for mode,pre in (("gin",""),("seq","set enable_bitmapscan=off; set enable_indexscan=off;")):
        s=pre+"\n"+(f"explain (analyze, format json) {stmt};\n")*5
        out=run(s)
        times=[float(m) for m in re.findall(r'"Execution Time": ([0-9.]+)',out)]
        plan="Bitmap" if "Bitmap" in out else "Seq" if "Seq Scan" in out else "?"
        res[mode]=(min(times),plan)
    got=",".join(run(pre_stmt:=stmt+";").split())
    same= got==exp[q]
    rows.append((q,res["gin"][0],res["gin"][1],res["seq"][0],same,(len(got.split(',')) if got else 0)))
print(f"{'query':<16}{'gin ms':>9} {'plan':<7}{'seq ms':>9}  same-as-scan")
for q,g,p,s,same,n in rows:
    print(f"{q:<16}{g:>9.3f} {p:<7}{s:>9.3f}  {'yes' if same else 'NO'} ({n} rows)")
g=sorted(r[1] for r in rows); s=sorted(r[3] for r in rows)
print(f"gin median {g[len(g)//2]:.3f} worst {g[-1]:.3f}; seq median {s[len(s)//2]:.3f} worst {s[-1]:.3f}; same on {sum(r[4] for r in rows)} of {len(rows)}")
