# Cross-vintage census of the AISC workbooks (dossier check for the vintage-ingestion and part-3 beads, bd-s9j3,
# bd-504e). Not a pre-registered oracle: written during bead research. It pairs the same (subject, period, line)
# across workbooks of different vintages and counts stored-value differences (2026-10-29: 3,722 comparisons, 0
# differences). It reads the workbooks only (stdlib), never the Rust output.
# usage: python3 -I scripts/oracle/vintage_census.py <dir with the .xlsx files>
import sys, zipfile, os, re, collections
from xml.etree import ElementTree as ET
NS={'m':'http://schemas.openxmlformats.org/spreadsheetml/2006/main','r':'http://schemas.openxmlformats.org/officeDocument/2006/relationships'}
M='{%s}'%NS['m']
SUBJ=["Nevada Gold Mines","Carlin","Cortez","Turquoise Ridge","Phoenix","Long Canyon","Hemlo","Pueblo Viejo","Veladero","Porgera","Loulo-Gounkoto","Kibali","North Mara","Tongon","Bulyanhulu","North America","Latin America & Asia Pacific","Africa & Middle East"]
SYN={'cos/oz':'cost of sales per ounce','tcc/oz':'total cash costs per ounce','aisc/oz':'all-in sustaining costs per ounce'}
def subj(h):
    for s in SUBJ:
        if h==s or (h.startswith(s) and len(h)==len(s)+1 and h[-1].islower()): return s
    return None
def period(name):
    n=name.strip()
    m=re.fullmatch(r'Q([1-4]) (\d{4})',n)
    if m: return f'{m.group(2)}Q{m.group(1)}'
    m=re.fullmatch(r'YTD (?:Q2|June 30) (\d{4})',n)
    if m: return f'{m.group(1)}H1'
    m=re.fullmatch(r'YTD (?:Q3|Sept 30) (\d{4})',n)
    if m: return f'{m.group(1)}9M'
    m=re.fullmatch(r'(?:YTD Dec 31 )?(\d{4})',n)
    if m: return f'{m.group(1)}FY'
    return 'UNMAPPED:'+n
d=sys.argv[1]
obs=collections.defaultdict(dict)  # key -> {file: (cell, raw)}
order=[]
for fn in sorted(os.listdir(d)):
    if not fn.endswith('AISC.xlsx'): continue
    z=zipfile.ZipFile(os.path.join(d,fn))
    ss=ET.fromstring(z.read('xl/sharedStrings.xml'))
    strs=[''.join(t.text or '' for t in si.iter(M+'t')) for si in ss.findall('m:si',NS)]
    wb=ET.fromstring(z.read('xl/workbook.xml'))
    rels=ET.fromstring(z.read('xl/_rels/workbook.xml.rels'))
    rmap={r.get('Id'):r.get('Target') for r in rels}
    for s in wb.find('m:sheets',NS).findall('m:sheet',NS):
        if s.get('state','visible')!='visible': continue
        t=rmap[s.get('{%s}id'%NS['r'])].lstrip('/'); t=t if t.startswith('xl/') else 'xl/'+t
        ws=ET.fromstring(z.read(t))
        cells={}
        for c in ws.iter(M+'c'):
            m=re.match(r'([A-Z]+)(\d+)',c.get('r'))
            v=c.find(M+'v'); val=v.text if v is not None else None
            if val is None: 
                isv=c.find(M+'is')
                if isv is not None: val=''.join(x.text or '' for x in isv.iter(M+'t'))
            if c.get('t')=='s' and val is not None: val=strs[int(val)]
            cells[(m.group(1),int(m.group(2)))]=(c.get('t'),val)
        labels={r:' '.join(v[1].split()).lower() for (col,r),v in cells.items() if col=='A' and v[1]}
        labels={r:SYN.get(l,l) for r,l in labels.items()}
        cos=[r for r,l in labels.items() if l=='cost of sales applicable to gold production']
        if not cos: continue
        cos=min(cos)
        end=[r for r,l in labels.items() if l=='all-in sustaining costs per ounce']
        end=min(end)
        # header row: row above cos with most subject headers
        best=None;bn=0
        for r in range(1,cos):
            n=sum(1 for (col,rr),v in cells.items() if rr==r and col!='A' and v[1] and subj(' '.join(str(v[1]).split())))
            if n>bn: bn=n;best=r
        heads={col:subj(' '.join(str(v[1]).split())) for (col,rr),v in cells.items() if rr==best and col!='A' and v[1]}
        seen=collections.Counter()
        for r in sorted(l for l in labels if cos<=l<=end):
            lab=labels[r]; seen[lab]+=1
            for col,sj in heads.items():
                if not sj: continue
                tv=cells.get((col,r))
                key=(period(s.get('name')),sj,lab,seen[lab])
                obs[key][fn]=(f"{s.get('name')}!{col}{r}", None if tv is None else tv[1])
multi={k:v for k,v in obs.items() if len(v)>=2}
print('keys',len(obs),'in >=2 vintages',len(multi), 'pairs', sum(len(v)-1 for v in multi.values()))
def num(x):
    try: return float(x)
    except: return None
changed=[]
for k,v in sorted(multi.items()):
    vals=[(fn,cell,raw) for fn,(cell,raw) in sorted(v.items())]
    raws=set((num(r) if num(r) is not None else ('tok',r)) for _,_,r in vals)
    if len(raws)>1: changed.append((k,vals))
print('changed keys',len(changed))
bylab=collections.Counter(k[2] for k,_ in changed)
print(bylab.most_common(20))
for k,vals in changed[:80]: print(k, vals)
print('--- samples')
for k,v in sorted(multi.items()):
    if (k[1]=='Kibali' and k[0]=='2024Q2' and k[2] in ('costs allocated to by-products','total cash costs')) or (k[1]=='North America' and k[2]=='all-in sustaining costs' ) or (k[1]=='Pueblo Viejo' and k[2]=='all-in sustaining costs' and k[0].startswith('2025')):
        print(k, sorted(v.items()))
per_file=collections.Counter()
for k,v in obs.items():
    for fn in v: per_file[(fn,k[0])]+=1
for x in sorted(per_file): print(x, per_file[x])
