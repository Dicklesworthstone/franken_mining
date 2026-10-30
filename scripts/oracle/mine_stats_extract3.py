# Oracle for bd-gwf1.1 (label-anchored Mine Stats reader). Copied unchanged, apart from this header, from the W1
# research scratch (w1/extract3.py, written 2026-10-29 00:13, before any Rust Mine Stats reader existed). Its output
# long_all3.json (22,732 rows, sha256 8992ae5085591926037374d3c9f913b31e38d7cce0366bf2f2ca1c37577881dd) is the
# reference W1 section 4's token census was taken from. It uses openpyxl, a reader independent of fmine-io.
# Run: uv run -q --with openpyxl python3 -I scripts/oracle/mine_stats_extract3.py <new-out.json> <Mine_Stats xlsx...>
# The workbooks are not in git; never commit long_all3.json (derived tables wait on OPEN-R7-1).
"""Normalize ALL Barrick Mine Stats workbooks (Q1 2024 .. Q2 2026) into a long table with file:sheet!cell locators.
Adapted from scratchpad/scripts/extract.py (C1). Label-anchored; no fixed row numbers.
Usage: python3 -I extract2.py <out.json> <xlsx> [<xlsx> ...]   (downloads are parsed as data only)"""
import sys, re, json, os, openpyxl
from openpyxl.utils import get_column_letter

CHILD = {s.lower() for s in ['Open Pit Ore', 'Open Pit Waste', 'Underground', 'Open Pit', 'Oxide Mill', 'Roasters',
         'Autoclave', 'Heap Leach', 'Open Pit mined', 'Underground mined', 'Processed', 'Royalties', 'Depreciation',
         'Total cash costs', 'All-in sustaining costs', 'Minesite sustaining', 'Projects', 'C1 cash costs']}


def clean(label):
    s = re.sub(r'\s+', ' ', str(label)).strip()
    m = re.match(r'^(.*?[^\d\s])(\d{1,2})$', s)
    if m and not s.endswith('%)'):
        return m.group(1).strip(), int(m.group(2))
    return s, None


def own(site):
    m = re.search(r'\(([\d.]+)%\)', site)
    return (float(m.group(1)) / 100, 'printed') if m else (1.0, 'unprinted')


def sitename(s):
    s, fn = clean(s)
    s = re.sub(r'\s*\([\d.]+%\)', '', s).strip()
    return {'Copper - Total': 'Total Copper'}.get(s, s)


def period_kind(label):
    l = label.lower()
    for k, v in (('three', 'Q'), ('six', 'H1'), ('nine', '9M'), ('twelve', 'FY')):
        if k in l:
            return v
    raise SystemExit(f'unknown period label {label!r}')


def summary_metric(name):
    raw = re.sub(r'\s+', ' ', str(name)).lower()
    is100 = '100%' in raw
    n = re.sub(r'\d', '', raw)
    metal = 'au' if 'gold' in n or '$/oz' in n else ('cu' if 'copper' in n or '$/lb' in n else None)
    if 'produced' in n and is100:
        f = 'prod_100'
    elif 'produced' in n and 'attributable' in n:
        f = 'prod_attr'
    elif 'sold' in n:
        f = 'sold_attr'
    elif 'cost of sales' in n:
        f = 'cos'
    elif 'total cash costs' in n:
        f = 'tcc'
    elif 'c cash costs' in n or 'c1 cash costs' in n:
        f = 'c1'
    elif 'all-in sustaining' in n:
        f = 'aisc'
    else:
        return None
    unit = {'au': {'prod_100': 'koz', 'prod_attr': 'koz', 'sold_attr': 'koz'},
            'cu': {'prod_100': 'kt', 'prod_attr': 'kt', 'sold_attr': 'kt'}}[metal].get(f)
    if unit:
        return f'{metal}_{f}_{unit}'
    return f'{f}_usd_per_{"oz" if metal == "au" else "lb"}'


rows = []
out = sys.argv[1]
for path in sys.argv[2:]:
    fkey = os.path.basename(path).replace('Barrick_', '').replace('_Mine_Stats.xlsx', '')  # e.g. Q3_2025
    fq, fy = int(fkey[1]), int(fkey[3:])
    wb = openpyxl.load_workbook(path, data_only=True)
    for ws in wb.worksheets:
        if ws.title in ('QTD', 'YTD'):
            r = 1
            while r <= min(ws.max_row, 60):
                a = ws.cell(r, 1).value
                if isinstance(a, str) and a.strip().startswith('For the'):
                    plabel = re.sub(r'\s+', ' ', a.strip())
                    kind = period_kind(plabel)
                    names = {c: summary_metric(ws.cell(r + 1, c).value) for c in range(2, 15)}
                    blocks = [(range(2, 8), ws.cell(r, 2).value), (range(9, 15), ws.cell(r, 9).value)]
                    rr = r + 2
                    while rr <= ws.max_row:
                        sraw = ws.cell(rr, 1).value
                        if sraw is None or re.match(r'^\s*\d+\s', str(sraw)) or str(sraw).startswith('For the'):
                            break
                        o, ob = own(str(sraw))
                        for cols, year in blocks:
                            for c in cols:
                                fid = names.get(c)
                                if fid is None:
                                    continue
                                period = f'{year}Q{fq}' if kind == 'Q' else f'{year}{kind}'
                                v = ws.cell(rr, c).value
                                rows.append(dict(file=fkey, fq=fq, fy=fy, sheet=ws.title, cell=f'{get_column_letter(c)}{rr}',
                                                 period_label=plabel, year=year, period=period, vintage='current' if year == fy else 'comparative',
                                                 view='summary', metal=fid[:2] if fid[:2] in ('au', 'cu') else None,
                                                 site_raw=str(sraw).strip(), site=sitename(sraw), own=o, own_basis=ob,
                                                 section=None, metric=str(ws.cell(r + 1, c).value), field=fid, value=v,
                                                 fmt=ws.cell(rr, c).number_format))
                        rr += 1
                    r = rr
                else:
                    r += 1
        elif ws.title.startswith('Minestats'):
            r = 1
            while r <= ws.max_row:
                a = ws.cell(r, 1).value
                if isinstance(a, str) and a.strip().startswith('For the'):
                    plabel = re.sub(r'\s+', ' ', a.strip())
                    kind = period_kind(plabel)
                    site_row = r - 1
                    cols = [c for c in range(2, 16) if isinstance(ws.cell(r, c).value, int)]
                    site_of = {}
                    for c in cols:
                        cc = c
                        while cc >= 2 and ws.cell(site_row, cc).value is None:
                            cc -= 1
                        site_of[c] = str(ws.cell(site_row, cc).value).strip()
                    parent = None
                    rr = r + 1
                    while rr <= ws.max_row:
                        lab = ws.cell(rr, 1).value
                        if lab is None:
                            rr += 1
                            continue
                        labs = str(lab).strip()
                        if re.match(r'^\d+\s', labs) or labs.startswith('BARRICK') or labs.startswith('Mine Statistics') or labs.startswith('For the'):
                            break
                        name, fn = clean(labs)
                        has = any(ws.cell(rr, c).value is not None for c in cols)
                        if name.lower() in CHILD and parent is not None:
                            sec = parent
                        else:
                            sec = None
                            parent = name
                        if has:
                            for c in cols:
                                year = ws.cell(r, c).value
                                period = f'{year}Q{fq}' if kind == 'Q' else f'{year}{kind}'
                                o, ob = own(site_of[c])
                                rows.append(dict(file=fkey, fq=fq, fy=fy, sheet=ws.title, cell=f'{get_column_letter(c)}{rr}',
                                                 period_label=plabel, year=year, period=period, vintage='current' if year == fy else 'comparative',
                                                 view='minestats', metal='cu' if ws.title == 'Minestats4' else 'au',
                                                 site_raw=site_of[c], site=sitename(site_of[c]), own=o, own_basis=ob,
                                                 section=sec, metric=name, metric_fn=fn, field=None, value=ws.cell(rr, c).value,
                                                 fmt=ws.cell(rr, c).number_format))
                        rr += 1
                    r = rr
                else:
                    r += 1

json.dump(rows, open(out, 'x'), indent=0, default=str)  # 'x': never overwrite
print(len(rows), 'rows')
