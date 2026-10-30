# Read-only stdlib census of per-ounce identities on visible AISC sheets (mirror of aisc.rs logic, for dossier checks).
# NOT independent of the Rust code: it mirrors aisc.rs's rules, so agreement with the Rust ledger is not evidence.
# The independent per-ounce reference is census2.py. Lines ending in MISS are rate instances outside the interval.
# usage: python3 -I scripts/oracle/rate_census.py <AISC .xlsx files>
import sys, zipfile, re
import xml.etree.ElementTree as ET
from fractions import Fraction as F
NS = {'m': 'http://schemas.openxmlformats.org/spreadsheetml/2006/main',
      'r': 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'}
def col2n(c):
    n = 0
    for ch in c: n = n*26 + ord(ch)-64
    return n
def n2col(n):
    s = ''
    while n: n, r = divmod(n-1, 26); s = chr(65+r)+s
    return s
def load(path):
    z = zipfile.ZipFile(path)
    ss = []
    if 'xl/sharedStrings.xml' in z.namelist():
        for si in ET.fromstring(z.read('xl/sharedStrings.xml')).findall('m:si', NS):
            ss.append(''.join(t.text or '' for t in si.iter('{%s}t' % NS['m'])))
    wb = ET.fromstring(z.read('xl/workbook.xml'))
    rels = ET.fromstring(z.read('xl/_rels/workbook.xml.rels'))
    tgt = {r.get('Id'): r.get('Target') for r in rels}
    out = []
    for s in wb.find('m:sheets', NS):
        name = s.get('name'); state = s.get('state') or 'visible'
        t = tgt[s.get('{%s}id' % NS['r'])]
        t = t.lstrip('/'); t = t if t.startswith('xl/') else 'xl/' + t
        cells = {}
        for c in ET.fromstring(z.read(t)).iter('{%s}c' % NS['m']):
            ref = c.get('r'); m = re.match(r'([A-Z]+)(\d+)', ref)
            v = c.find('m:v', NS); typ = c.get('t')
            if v is None:
                isel = c.find('m:is', NS)
                if isel is not None: cells[(int(m.group(2)), col2n(m.group(1)))] = ''.join(x.text or '' for x in isel.iter('{%s}t' % NS['m']))
                continue
            if typ == 's': val = ss[int(v.text)]
            elif typ in ('str', 'inlineStr', 'e'): val = v.text
            else:
                try: val = float(v.text)
                except: val = v.text
            cells[(int(m.group(2)), col2n(m.group(1)))] = val
        out.append((name, state, cells))
    return out
def norm(x): return ' '.join(str(x).split()).lower()
def isnum(x): return isinstance(x, float)
def rate_ok(n, k, q, r):
    lo_n, hi_n = F(2*n - k, 2), F(2*n + k, 2)
    corners = [1000*a/b for a in (lo_n, hi_n) for b in (F(2*q-1, 2), F(2*q+1, 2))]
    return min(corners) <= F(2*r+1, 2) and max(corners) >= F(2*r-1, 2)
for path in sys.argv[1:]:
    fname = path.split('/')[-1]
    for name, state, cells in load(path):
        lab = {r: norm(v) for (r, c), v in cells.items() if c == 1 and isinstance(v, str)}
        cos = [r for r, l in sorted(lab.items()) if l == 'cost of sales applicable to gold production']
        if not cos: continue
        def first(names): return next((r for r, l in sorted(lab.items()) if l in names), None)
        cos = cos[0]; tcc = first(['total cash costs']); aisc = first(['all-in sustaining costs'])
        oz = next((r for r, l in sorted(lab.items()) if l.startswith('ounces sold')), None)
        rr = {'cos': first(['cost of sales per ounce', 'cos/oz']), 'tcc': first(['total cash costs per ounce', 'tcc/oz']), 'aisc': first(['all-in sustaining costs per ounce', 'aisc/oz'])}
        b1 = [r for r in sorted(lab) if cos <= r < tcc]; b2 = [r for r in sorted(lab) if tcc < r < aisc]
        nci = [r for r in b1 + b2 if lab[r] == 'non-controlling interests']
        hdr = max(range(1, cos), key=lambda r: sum(1 for (rr_, c), v in cells.items() if rr_ == r and c >= 2 and isinstance(v, str) and v.strip()))
        for (r_, c), h in sorted(cells.items(), key=lambda kv: kv[0][1]):
            if r_ != hdr or c < 2 or not isinstance(h, str): continue
            g = lambda r: cells.get((r, c))
            q = g(oz)
            for fam, nrow, lines in (('cos', cos, [cos]), ('tcc', tcc, b1), ('aisc', aisc, b1 + b2)):
                rv, nv = g(rr[fam]), g(nrow)
                if not (isnum(rv) and isnum(q) and isnum(nv)) or rv == 0 or q == 0: continue
                if any(isnum(g(r)) and g(r) != int(g(r)) for r in lines + [nrow, oz, rr[fam]]): continue
                if fam == 'cos' and any(isnum(g(r)) and g(r) != 0 for r in nci): continue
                k = sum(1 for r in lines if isnum(g(r)))
                ok = rate_ok(int(nv), k, int(q), int(rv))
                if not ok or ('veladero' in norm(h) or 'kibali' in norm(h) or 'hemlo' in norm(h)) and fam=='cos' and False:
                    print(f"{fname}|{name}|{state}|{' '.join(h.split())}|{n2col(c)}{rr[fam]}|{fam}|N={int(nv)} k={k} Q={int(q)} r={int(rv)}|{'OK' if ok else 'MISS'}")
