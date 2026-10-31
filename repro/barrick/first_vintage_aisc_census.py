"""Independent first-registered aggregation of the existing Python AISC readers.

No Rust consistency implementation is imported or read. Visible sheets only.
Usage: python3 -I repro/barrick/first_vintage_aisc_census.py [workspace_root]
"""
import json
import os
import re
import sys
import tomllib
from collections import Counter, defaultdict

root = os.path.abspath(sys.argv[1] if len(sys.argv) > 1
                       else os.path.join(os.path.dirname(__file__), '..', '..'))
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(root, 'scripts/oracle'))
import census2
import share_census
import region_census

with open(os.path.join(root, 'registries/sources/barrick.toml'), 'rb') as f:
    reg = tomllib.load(f)
quarter = {s['file']: s['quarter'] for s in reg['source']}
nsheets, raw = census2.run(root, displayed=True)
rows = {r[:5]: list(r) for r in raw if r[4] != 'column'}
assert len(rows) == sum(r[4] != 'column' for r in raw)
assert all(r[0] in quarter for r in raw)

share = share_census.run(root)
for kind, r in share:
    col = re.fullmatch(r'([A-Z]+)\d+', r['cell'])[1]
    key = (r['file'], r['sheet'], col, r['subject'], 'COS/oz')
    assert rows[key][5] == 'v'
    rows[key][5:] = [r['state'], kind]

regions = region_census.gather(root)
for r in regions:
    verdict, reason, at, used, excluded, lo, hi = region_census.evaluate(r)
    col = re.fullmatch(r'([A-Z]+)\d+', r['cell'])[1]
    key = (r['file'], r['sheet'], col, r['region'], 'COS/oz')
    assert rows[key][5] == 'v'
    rows[key][5:] = [{'refused': 'v', 'refuted': 'iv', 'not_refuted': 'i'}[verdict], reason]


def period(sheet):
    if m := re.fullmatch(r'Q([1-4]) ([0-9]{4})', sheet):
        return m[2] + 'Q' + m[1]
    if m := re.fullmatch(r'([0-9]{4})', sheet):
        return m[1] + 'FY'
    if m := re.fullmatch(r'YTD Q([23]) ([0-9]{4})', sheet):
        return m[2] + {'2': 'H1', '3': '9M'}[m[1]]
    if m := re.fullmatch(r'YTD (Sept 30|Dec 31) ([0-9]{4})', sheet):
        return m[2] + {'Sept 30': '9M', 'Dec 31': 'FY'}[m[1]]
    raise ValueError(sheet)


def identity(r, maps):
    fn, sh, col, subject, family, state, detail = r
    scope = ()
    all_regions = set(region_census.MAP_TO_2025Q3) | set(region_census.MAP_FROM_2025Q4)
    if maps and subject in all_regions:
        scope = tuple(sorted(region_census.region_map(quarter[fn]).get(subject, ['unknown:' + quarter[fn]])))
    return period(sh), subject, family, scope


states = ['i', 'ii', 'iii', 'iv', 'v', 'np']


def counts(rs):
    by = defaultdict(Counter)
    total = Counter()
    for r in rs:
        by[r[4]][r[5]] += 1
        total[r[5]] += 1
    return {
        'total': {s: total[s] for s in states},
        'by_family': {f: {s: c[s] for s in states} for f, c in sorted(by.items())},
    }


result = {
    'visible_sheets': nsheets,
    'registered_files': len(reg['source']),
    'all_instances': len(rows),
    'all': counts(rows.values()),
    'overrides': {'share': len(share), 'region': len(regions)},
    'first': {},
}
for maps in [False, True]:
    grouped = defaultdict(list)
    for r in rows.values():
        grouped[identity(r, maps)].append(r)
    first = []
    ties = []
    for key, rs in sorted(grouped.items()):
        rs.sort(key=lambda r: (quarter[r[0]], r[0], r[1], r[2]))
        earliest = [r for r in rs if quarter[r[0]] == quarter[rs[0][0]]]
        if len(earliest) > 1:
            ties.append({'identity': key, 'rows': earliest})
        assert len({r[5] for r in earliest}) == 1, ties[-1]
        first.append(rs[0])
    result['first']['with_region_map' if maps else 'period_subject_family'] = {
        'identities': len(grouped), 'ties': ties, **counts(first),
    }

books = {}
seq = []
for r in sorted(rows.values(), key=lambda r: (quarter[r[0]], r[0], r[1], r[2])):
    if (r[3], r[4], period(r[1])) not in [('Kibali', 'TCC', '2024Q2'), ('Porgera', 'AISC', '2024Q2')]:
        continue
    if r[0] not in books:
        books[r[0]] = census2.load(os.path.join(root, reg['corpus_dir'], r[0]))
    label = {'TCC': 'total cash costs', 'AISC': 'all-in sustaining costs'}[r[4]]
    hits = [ref[1:] for ref, v in books[r[0]][r[1]].items()
            if re.fullmatch('A[0-9]+', ref) and census2.norm(v).lower() == label]
    assert len(hits) == 1, hits
    seq.append({'release': quarter[r[0]], 'file': r[0], 'sheet': r[1], 'cell': r[2] + hits[0],
                'subject': r[3], 'family': r[4], 'state': r[5]})
result['sequences'] = seq
print(json.dumps(result, indent=2))
