# Minimal stdlib xlsx loader shared by census.py and census2.py (independent of fmine-io).
import zipfile,re,sys
def load(path):
    z=zipfile.ZipFile(path)
    ss=[]
    if 'xl/sharedStrings.xml' in z.namelist():
        ss=[''.join(re.findall(r'<t[^>]*>(.*?)</t>',si,re.S)) for si in re.findall(r'<si>(.*?)</si>',z.read('xl/sharedStrings.xml').decode(),re.S)]
    wb=z.read('xl/workbook.xml').decode()
    names=re.findall(r'<sheet [^>]*name="([^"]+)"[^>]*r:id="([^"]+)"',wb)
    rels=z.read('xl/_rels/workbook.xml.rels').decode()
    rm=dict(re.findall(r'Id="([^"]+)"[^>]*Target="([^"]+)"',rels)); rm.update({b:a for a,b in re.findall(r'Target="([^"]+)"[^>]*Id="([^"]+)"',rels)})
    out={}
    for n,rid in names:
        x=z.read('xl/'+rm[rid].replace('/xl/','').replace('xl/','')).decode()
        c={}
        for m in re.finditer(r'<c r="([A-Z]+[0-9]+)"([^>]*?)(?:/>|>(.*?)</c>)',x,re.S):
            r,a,b=m.groups(); v=re.search(r'<v>(.*?)</v>',b or '')
            if v: c[r]=ss[int(v.group(1))] if 't="s"' in a else v.group(1)
        out[n]=c
    return out
