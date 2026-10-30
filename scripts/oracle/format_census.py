# Number-format census of the registered workbooks (dossier check for the cell-number-formats bead, bd-jg43).
# Not a pre-registered oracle: written during bead research, after the Rust reader existed. It reads the
# workbooks only (stdlib), never the Rust output. It prints, per file and sheet, the cells whose stored value is
# not an integer, and the totals by number-format code.
# usage: python3 -I scripts/oracle/format_census.py <dir with the .xlsx files>
import sys, zipfile, collections, os
from xml.etree import ElementTree as ET
NS={'m':'http://schemas.openxmlformats.org/spreadsheetml/2006/main','r':'http://schemas.openxmlformats.org/officeDocument/2006/relationships'}
M='{%s}'%NS['m']
d=sys.argv[1]
tot=collections.Counter(); nonint=collections.Counter(); nonint_cells=collections.defaultdict(list)
for fn in sorted(os.listdir(d)):
    if not fn.endswith('.xlsx'): continue
    z=zipfile.ZipFile(os.path.join(d,fn))
    st=ET.fromstring(z.read('xl/styles.xml'))
    fmts={}
    nf=st.find('m:numFmts',NS)
    if nf is not None:
        for f in nf.findall('m:numFmt',NS): fmts[int(f.get('numFmtId'))]=f.get('formatCode')
    xfs=[int(x.get('numFmtId','0')) for x in st.find('m:cellXfs',NS).findall('m:xf',NS)]
    wb=ET.fromstring(z.read('xl/workbook.xml'))
    rels=ET.fromstring(z.read('xl/_rels/workbook.xml.rels'))
    rmap={r.get('Id'):r.get('Target') for r in rels}
    for s in wb.find('m:sheets',NS).findall('m:sheet',NS):
        if s.get('state','visible')!='visible': continue
        t=rmap[s.get('{%s}id'%NS['r'])].lstrip('/'); t=t if t.startswith('xl/') else 'xl/'+t
        ws=ET.fromstring(z.read(t))
        for c in ws.iter(M+'c'):
            if c.get('t') not in (None,'n'): continue
            v=c.find(M+'v')
            if v is None or not (v.text or '').strip(): continue
            try: x=float(v.text)
            except: continue
            fid=xfs[int(c.get('s','0'))]
            code=fmts.get(fid,'builtin:%d'%fid)
            key=code
            tot[key]+=1
            if x!=int(x) if abs(x)<1e15 else False:
                nonint[key]+=1
                if 'AISC' in fn: nonint_cells[(fn,s.get('name'))].append((c.get('r'),v.text))
for k in tot: print(repr(k), 'numeric', tot[k], 'nonint', nonint[k])
print('AISC non-integer cells by sheet:')
for k,v in sorted(nonint_cells.items()): print(k, len(v), v[:12])
