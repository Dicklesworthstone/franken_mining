"""Invented OMF bytes and exact scheduling oracles for the actual planning binary.

The independent stdlib ZIP/gzip/Parquet serializer extends the seam in
plan_pit_omf.rs to multiple attributes. Feasibility, objectives and dual prices
are checked by enumerating definitions, without calling a production kernel.
"""

from copy import deepcopy
from fractions import Fraction as Q
from itertools import product
from pathlib import Path
import gzip
import hashlib
import io
import json
import math
import random
import struct
import subprocess
import sys
import tempfile
import zipfile

BINARY = Path(sys.argv[1])
ROOT = Path(tempfile.mkdtemp(prefix="fmine-plan-schedule-omf-oracle-"))
COUNT = (2, 1, 2)
FACTOR_SCALE = 2**40


def varint(n):
    out = bytearray()
    while n > 127:
        out.append((n & 127) | 128)
        n >>= 7
    out.append(n)
    return bytes(out)


def integer(n, kind=5):
    return kind, varint(2*n if n >= 0 else -2*n-1)


def text(s):
    encoded = s.encode()
    return 8, varint(len(encoded)) + encoded


def fields(rows):
    out, last = bytearray(), 0
    for key, (kind, body) in rows:
        out.append((key-last)*16 + kind)
        out.extend(body)
        last = key
    return 12, bytes(out) + b'\0'


def sequence(items):
    kind = items[0][0]
    head = bytes([len(items)*16 + kind]) if len(items) < 15 else bytes([240+kind])+varint(len(items))
    return 9, head + b''.join(body for _, body in items)


def parquet(values):
    n = len(values)
    levels = b''.join(bytes([2, int(v is not None)]) for v in values)
    body = struct.pack('<I', len(levels)) + levels
    body += b''.join(struct.pack('<d', v) for v in values if v is not None)
    page = fields([(1,integer(0)), (2,integer(len(body))), (3,integer(len(body))),
                   (5,fields([(1,integer(n)), (2,integer(0)), (3,integer(3)), (4,integer(3))]))])[1] + body
    schema = sequence([fields([(4,text('schema')), (5,integer(1))]),
                       fields([(1,integer(5)), (3,integer(1)), (4,text('number'))])])
    column = fields([(2,integer(4,6)), (3,fields([
        (1,integer(5)), (2,sequence([integer(0),integer(3)])),
        (3,sequence([text('number')])), (4,integer(0)), (5,integer(n,6)),
        (6,integer(len(page),6)), (7,integer(len(page),6)), (9,integer(4,6))]))])
    group = fields([(1,sequence([column])), (2,integer(len(page),6)), (3,integer(n,6))])
    footer = fields([(1,integer(1)), (2,schema), (3,integer(n,6)), (4,sequence([group]))])[1]
    return b'PAR1' + page + footer + struct.pack('<I',len(footer)) + b'PAR1'


def write_omf(attributes, defect=None, *, units=None):
    attrs, members = [], []
    for name, values in attributes.items():
        filename = name + '.parquet'
        declared_units = (units or {}).get(name, 'USD' if name == 'value' else 'synthetic units')
        attrs.append({'name': name, 'units': declared_units,
                      'location': 'Primitives', 'data': {'type': 'Number', 'values':
                      {'filename': filename, 'item_count': len(values)}}})
        members.append((filename, parquet(values)))
    if defect == 'duplicate_attribute':
        attrs.append(deepcopy(attrs[0]))
    geometry = {'type': 'BlockModel', 'orient': {'origin': [0, 0, 0]},
                'grid': {'type': 'Regular', 'size': [12.5, 12.5, 12.5], 'count': COUNT}}
    if defect == 'tilted':
        geometry['orient'].update(v=[0, 0.6, 0.8], w=[0, -0.8, 0.6])
    element = {'name': 'section', 'geometry': geometry, 'attributes': attrs}
    project = {'name': 'Invented OMF schedule', 'units': 'm',
               'coordinate_reference_system': 'EPSG:32633', 'date': '2026-01-01T00:00:00Z',
               'elements': [element, deepcopy(element)] if defect == 'duplicate_model' else [element]}
    output = io.BytesIO()
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_STORED) as archive:
        members.insert(0, ('index.json.gz', gzip.compress(json.dumps(project).encode(), mtime=0)))
        for name, data in members:
            archive.writestr(zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0)), data)
        archive.comment = b'Open Mining Format 2.0'
    (ROOT / 'section.omf').write_bytes(output.getvalue())


def spec():
    return {'schema': 'fmine.omf-schedule.v1', 'block_model': 'section',
            'value_attribute': 'value', 'value_per_unit': 4,
            'value_rounding': 'nearest_integer_half_away_from_zero',
            'slope': {'rise': 1, 'run': 1, 'levels': 1}, 'periods': 2,
            'discount': {'rate': 1, 'scale': 10},
            'capacities': [{'attribute': 'tonnes', 'per_unit': 1, 'limits_scaled': [4, 4]}],
            'search_rounds': 20, 'bound_steps': 64}


def data():
    return {'value': [7.125, -1.125, -1.25, -1.375], 'tonnes': [1.2] * 4}


def write_spec(declaration):
    (ROOT / 'schedule.json').write_text(json.dumps(declaration), encoding='utf-8')


def run(extra=(), *, success=True, arguments=None):
    args = arguments if arguments is not None else [
        'plan', 'schedule', '--omf', 'section.omf', '--spec', 'schedule.json', '--list-schedule']
    result = subprocess.run([str(BINARY), *args, *extra, '--workspace', str(ROOT)],
                            capture_output=True, text=True, check=False)
    report = json.loads(result.stdout)
    assert result.returncode == report['exit_code'], (result.stderr, report)
    assert all(value is False for value in report['flags'].values()), report['flags']
    if success:
        assert result.returncode == 0 and report['status'] == 'ok', report
    else:
        assert result.returncode != 0 and report['status'] != 'ok', report
    return report


def quantity(value):
    return Q(int(value['numerator']), int(value['denominator']))


def nearest(value):
    value = Q(str(value))
    magnitude = abs(value)
    rounded = (2*magnitude.numerator + magnitude.denominator) // (2*magnitude.denominator)
    return rounded if value >= 0 else -rounded


def requirements(declaration):
    # Parent-block coordinates are OMF U-fastest, then V, then upward W.
    nx, ny, nz = COUNT
    coordinates = [(u, v, w) for w in range(nz) for v in range(ny) for u in range(nx)]
    slope = declaration['slope']
    return [[p for p, (x, y, z) in enumerate(coordinates)
             if 0 < z-w <= slope['levels'] and
             ((x-u)**2 + (y-v)**2)*slope['rise']**2 <= (z-w)**2*slope['run']**2]
            for u, v, w in coordinates]


def verify(report, declaration, attributes, *, optimum=False):
    payload = report['payload']
    assert payload['schema'] == 'fmine.plan-schedule.omf.v1'
    for key, name in [('omf', 'section.omf'), ('spec', 'schedule.json')]:
        original = (ROOT / name).read_bytes()
        assert payload['sources'][key]['sha256'] == hashlib.sha256(original).hexdigest()
        assert payload['sources'][key]['bytes'] == len(original)
    factors = [FACTOR_SCALE]
    for _ in range(declaration['periods'] - 1):
        rate = declaration['discount']
        factors.append(factors[-1] * rate['scale'] // (rate['scale'] + rate['rate']))
    assert int(payload['discount']['factor_scale']) == FACTOR_SCALE
    assert list(map(int, payload['discount']['factors'])) == factors
    weights = [nearest(Q(str(v)) * declaration['value_per_unit']) for v in attributes['value']]
    uses = [[math.ceil(Q(str(v)) * row['per_unit']) for v in attributes[row['attribute']]]
            for row in declaration['capacities']]
    limits = [row['limits_scaled'] for row in declaration['capacities']]
    requires = requirements(declaration)
    denominator = declaration['value_per_unit'] * FACTOR_SCALE

    def evaluate(periods):
        assert len(periods) == len(weights)
        assert all(t is None or isinstance(t, int) and 0 <= t < len(factors) for t in periods)
        values = [sum(v for v, chosen in zip(weights, periods) if chosen == t) for t in range(len(factors))]
        used = [[sum(u for u, chosen in zip(row, periods) if chosen == t) for t in range(len(factors))]
                for row in uses]
        missing = [(b, p) for b, t in enumerate(periods) if t is not None for p in requires[b]
                   if periods[p] is None or periods[p] > t]
        over = [(r, t) for r, row in enumerate(used) for t, total in enumerate(row) if total > limits[r][t]]
        value = sum(v * f for v, f in zip(values, factors))
        return value, values, used, missing, over

    selected = payload['selected']
    value, values, used, missing, over = evaluate(selected['periods'])
    checked = selected['checked']
    assert checked['feasible'] and not missing and not over, selected
    assert int(checked['mined_blocks']) == sum(t is not None for t in selected['periods'])
    assert list(map(int, checked['period_values'])) == values
    assert [list(map(int, row)) for row in checked['resource_used']] == used
    assert quantity(selected['discounted_value']) == Q(value, denominator)
    certificate = payload['certificate']
    assert certificate['kind'] == 'integer_lagrangian' and certificate['verified'] is True
    prices = [list(map(int, row)) for row in certificate['prices']]
    assert len(prices) == len(limits)
    assert all(len(row) == len(factors) and all(p >= 0 for p in row) for row in prices)
    best, relaxed_best = None, None
    for periods in product([None, *range(len(factors))], repeat=len(weights)):
        candidate, _, resource, unmet, excess = evaluate(periods)
        if unmet:
            continue
        relaxed = candidate - sum(prices[r][t] * amount for r, row in enumerate(resource)
                                  for t, amount in enumerate(row))
        relaxed_best = relaxed if relaxed_best is None else max(relaxed_best, relaxed)
        if not excess:
            best = candidate if best is None else max(best, candidate)
    replayed = relaxed_best + sum(prices[r][t] * limit for r, row in enumerate(limits)
                                 for t, limit in enumerate(row))
    upper = quantity(certificate['upper_bound'])
    assert upper == Q(replayed, denominator), (certificate, replayed)
    assert Q(value, denominator) <= Q(best, denominator) <= upper
    assert quantity(certificate['gap']) == upper - Q(value, denominator)
    assert 1 <= len(certificate['history']) <= declaration['bound_steps']
    if optimum:
        assert value == best, (selected, best)
    return payload, evaluate, denominator


def schedules():
    declaration, attributes = spec(), data()
    write_spec(declaration)
    write_omf(attributes)
    report = run()
    verify(report, declaration, attributes, optimum=True)
    assert run() == report
    # Different attribute scales and nonintegral uses catch nearest-use rounding and reused scales.
    declaration['capacities'].append({'attribute': 'water', 'per_unit': 4, 'limits_scaled': [2, 6]})
    attributes['water'] = [0.5] * 4
    write_spec(declaration)
    write_omf(attributes)
    verify(run(), declaration, attributes)
    # At 10%, the third recursive factor is one less than flooring the closed-form power.
    declaration, attributes = spec(), data()
    declaration['periods'] = 3
    declaration['capacities'][0]['limits_scaled'] = [2, 2, 2]
    write_spec(declaration)
    write_omf(attributes)
    payload, _, _ = verify(run(), declaration, attributes)
    assert int(payload['discount']['factors'][2]) == 908687295681
    # A negative-use compensator has negative value and lies outside the ordinary ultimate pit.
    # It enables a feasible value of 4, so a bound restricted to that pit would be unsound.
    declaration, attributes = spec(), {'value': [6, -2, 0, 0], 'blend': [5, -5, 0, 0]}
    declaration['capacities'] = [{'attribute': 'blend', 'per_unit': 1, 'limits_scaled': [0, 0]}]
    write_spec(declaration)
    write_omf(attributes)
    payload, _, _ = verify(run(), declaration, attributes)
    assert quantity(payload['certificate']['upper_bound']) >= 4
    # A feasible negative candidate cannot displace the empty incumbent.
    declaration, attributes = spec(), data()
    declaration['candidate'] = {'attribute': 'period', 'first': 1}
    attributes['value'] = [-1.125, -2, -3, -4]
    attributes['period'] = [None, None, 1, None]
    write_spec(declaration)
    write_omf(attributes)
    payload, _, _ = verify(run(), declaration, attributes, optimum=True)
    assert payload['selected']['origin'] == 'empty'
    assert payload['selected']['periods'] == [None] * 4
    assert quantity(payload['selected']['discounted_value']) == 0
    assert payload['candidate']['checked']['feasible']
    assert quantity(payload['candidate']['discounted_value']) < 0
    rng = random.Random(194250)
    for _ in range(6):
        declaration = spec()
        declaration['discount'] = {'rate': rng.choice([0, 1, 3]), 'scale': 7}
        declaration['capacities'][0].update(per_unit=2, limits_scaled=[5, 7])
        attributes = {'value': [rng.randint(-12, 36) / 8 for _ in range(4)],
                      'tonnes': [0.1, 1.2, 1.0, 1.6]}
        write_spec(declaration)
        write_omf(attributes)
        verify(run(), declaration, attributes)


def candidates():
    declaration, attributes = spec(), data()
    declaration['candidate'] = {'attribute': 'period', 'first': 1}
    write_spec(declaration)
    for periods, feasible in [([2, None, 1, 1], True), ([1, None, None, None], False),
                              ([1, None, 1, 1], False)]:
        attributes['period'] = periods
        write_omf(attributes)
        payload, evaluate, denominator = verify(run(), declaration, attributes, optimum=True)
        supplied = [None if t is None else t - 1 for t in periods]
        value, _, _, unmet, over = evaluate(supplied)
        candidate = payload['candidate']
        assert candidate['checked']['feasible'] is feasible
        assert quantity(candidate['discounted_value']) == Q(value, denominator)
        if feasible:
            assert quantity(payload['selected']['discounted_value']) >= Q(value, denominator)
        else:
            assert candidate.get('improved_value') is None, candidate
            if unmet:
                assert int(candidate['checked']['unmet_requirements']) > 0
                witnesses = {tuple(map(int, pair)) for pair in candidate['checked']['first_unmet']}
                assert witnesses and witnesses <= set(unmet), (witnesses, unmet)
            if over:
                assert [tuple(map(int, pair)) for pair in candidate['checked']['over_limit']] == over
    for bad in [0, 1.5, 3, float('nan')]:
        attributes['period'] = [bad, None, 1, 1]
        write_omf(attributes)
        run(success=False)


def refusals():
    declaration, attributes = spec(), data()
    attributes['period'] = [2, None, 1, 1]
    write_omf(attributes)
    invalid = []
    for key in declaration:
        missing = deepcopy(declaration)
        del missing[key]
        invalid.append(missing)
    for path in [(), ('slope',), ('discount',), ('capacities', 0)]:
        unknown = deepcopy(declaration)
        target = unknown
        for part in path:
            target = target[part]
        target['undeclared'] = 1
        invalid.append(unknown)
    for key, value in [('schema', 'fmine.omf-schedule.v999'), ('value_per_unit', 0),
                       ('value_rounding', 'nearest'), ('periods', 0), ('periods', 2**32),
                       ('periods', 257), ('search_rounds', 1001), ('bound_steps', 0),
                       ('bound_steps', 257),
                       ('candidate', {'attribute': 'period', 'first': 1, 'undeclared': 1})]:
        invalid.append({**declaration, key: value})
    for rate in [{'rate': -1, 'scale': 10}, {'rate': 1, 'scale': 0}]:
        invalid.append({**declaration, 'discount': rate})
    for key, value in [('per_unit', 0), ('limits_scaled', [4]), ('limits_scaled', [-1, 4])]:
        bad = deepcopy(declaration)
        bad['capacities'][0][key] = value
        invalid.append(bad)
    for bad in invalid:
        write_spec(bad)
        run(success=False)
    raw = json.dumps(declaration)
    for duplicate in [raw.replace('"periods": 2', '"periods": 2, "periods": 3'),
                      raw.replace('"rise": 1', '"rise": 1, "rise": 2')]:
        (ROOT / 'schedule.json').write_text(duplicate, encoding='utf-8')
        run(success=False)
    write_spec(declaration)
    for defect in ['duplicate_attribute', 'duplicate_model', 'tilted']:
        write_omf(attributes, defect)
        run(success=False)
    for bad in [None, float('nan')]:
        changed = deepcopy(attributes)
        changed['tonnes'][0] = bad
        write_omf(changed)
        run(success=False)
    write_omf({'value': attributes['value']})
    run(success=False)


def command():
    declaration, attributes = spec(), data()
    write_spec(declaration)
    write_omf(attributes)
    report = run()
    verify(report, declaration, attributes)
    no_list = run(arguments=['plan', 'schedule', '--omf', 'section.omf', '--spec', 'schedule.json'])
    assert no_list['payload']['selected'].get('periods') is None
    for extra in [['--pcpsp', 'x'], ['--prec', 'x'], ['--candidate', 'x'], ['--decimals', '0'],
                  ['--use-decimals', '0'], ['--share-decimals', '0'], ['--round'],
                  ['--lp-guidance'], ['--rounds', '1'], ['--bound-steps', '1'],
                  ['--omf', 'x'], ['--spec', 'x'], ['--unknown']]:
        assert run(extra, success=False)['exit_code'] == 2
    for args in [['plan', 'schedule', '--omf', 'section.omf'],
                 ['plan', 'schedule', '--spec', 'schedule.json'],
                 ['plan', 'schedule', '--pcpsp', 'x', '--prec', 'y', '--spec', 'schedule.json']]:
        assert run(arguments=args, success=False)['exit_code'] == 2
    docs = run(arguments=['robot-docs'])
    assert 'fmine.omf-schedule.v1' in json.dumps(docs)
    markdown = subprocess.run([str(BINARY), 'plan', 'schedule', '--omf', 'section.omf',
                               '--spec', 'schedule.json', '--format', 'markdown', '--workspace', str(ROOT)],
                              capture_output=True, text=True, check=False)
    assert markdown.returncode == 0, markdown.stderr
    for name in ['section.omf', 'schedule.json']:
        assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() in markdown.stdout
    declaration['discount']['rate'] = 2
    write_spec(declaration)
    changed = run()
    verify(changed, declaration, attributes)
    assert changed['payload']['sources']['omf'] == report['payload']['sources']['omf']
    assert changed['payload']['sources']['spec']['sha256'] != report['payload']['sources']['spec']['sha256']
    assert changed['payload']['discount']['factors'] != report['payload']['discount']['factors']
    attributes['value'][0] += 0.25
    write_omf(attributes)
    changed_omf = run()
    verify(changed_omf, declaration, attributes)
    assert changed_omf['payload']['sources']['omf']['sha256'] != changed['payload']['sources']['omf']['sha256']
    assert changed_omf['payload']['sources']['spec'] == changed['payload']['sources']['spec']
    assert changed_omf['payload']['selected']['discounted_value'] != changed['payload']['selected']['discounted_value']


if __name__ == '__main__':
    globals()[sys.argv[2]]()
    print('OMF schedule oracle passed:', sys.argv[2])
