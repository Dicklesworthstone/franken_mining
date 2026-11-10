"""Invented OMF shells through the real binary, checked by exact closed-set enumeration.

The serializer is the independent Python stdlib fixture seam used by OMF
scheduling. No expected shell, objective or nesting decision calls a Rust kernel.
All numbers and geometry are synthetic declarations, never evidence about a mine.
"""

from copy import deepcopy
from fractions import Fraction as Q
from itertools import product
from pathlib import Path
import hashlib
import importlib.util
import json
import random
import subprocess
import sys

fixture_spec = importlib.util.spec_from_file_location(
    'omf_synthetic_fixture', Path(__file__).with_name('plan_schedule_omf_oracle.py'))
fixture = importlib.util.module_from_spec(fixture_spec)
fixture_spec.loader.exec_module(fixture)
BINARY, ROOT = fixture.BINARY, fixture.ROOT
UNITS = {'revenue': 'USD/block', 'cost': 'USD/block'}


def spec():
    return {'schema': 'fmine.omf-shells.v1', 'block_model': 'section',
            'revenue_attribute': 'revenue', 'cost_attribute': 'cost', 'per_unit': 4,
            'value_rounding': 'nearest_integer_half_away_from_zero',
            'slope': {'rise': 1, 'run': 1, 'levels': 1},
            'factors': [{'num': n, 'den': d} for n, d in [(1, 2), (1, 1), (3, 2), (2, 1)]]}


def data():
    return {'revenue': [6.25, 2.25, 0, 0], 'cost': [1.125, 3.125, 1.125, 1.125]}


def write_spec(declaration):
    (ROOT / 'shells.json').write_text(json.dumps(declaration), encoding='utf-8')


def run(extra=(), *, success=True, arguments=None):
    args = arguments if arguments is not None else [
        'plan', 'shells', '--omf', 'section.omf', '--spec', 'shells.json', '--list-blocks']
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


def verify(report, declaration, attributes):
    payload = report['payload']
    assert payload['schema'] == 'fmine.plan-shells.omf.v1'
    assert payload['block_order'] == 'omf_u_then_v_then_w_bottom_up'
    assert payload['model']['value_units'] == 'USD/block'
    assert payload['model']['per_unit'] == declaration['per_unit']
    for key, name in [('omf', 'section.omf'), ('spec', 'shells.json')]:
        original = (ROOT / name).read_bytes()
        assert payload['sources'][key]['sha256'] == hashlib.sha256(original).hexdigest()
        assert payload['sources'][key]['bytes'] == len(original)
    certificate = payload['certificate']
    assert certificate['kind'] == 'integer_nested_closures'
    assert certificate['verified'] is True and certificate['nested'] is True
    assert certificate['flow_witnesses_exported'] is False
    revenue = [fixture.nearest(Q(str(v)) * declaration['per_unit']) for v in attributes['revenue']]
    cost = [fixture.nearest(Q(str(v)) * declaration['per_unit']) for v in attributes['cost']]
    requires = fixture.requirements(declaration)
    closed = []
    for mask in product([False, True], repeat=len(revenue)):
        selected = {b for b, present in enumerate(mask) if present}
        if all(set(requires[b]) <= selected for b in selected):
            closed.append(selected)
    first = payload['first_shell']
    assert len(first) == len(revenue)
    assert all(k is None or isinstance(k, int) and 0 <= k < len(declaration['factors']) for k in first)
    assert len(payload['shells']) == len(declaration['factors'])
    previous = set()
    for k, (shell, factor) in enumerate(zip(payload['shells'], declaration['factors'])):
        num, den = factor['num'], factor['den']
        assert shell['factor'] == {'num': str(num), 'den': str(den)}
        weights = [num * r - den * c for r, c in zip(revenue, cost)]
        best = max(sum(weights[b] for b in candidate) for candidate in closed)
        optimal = [candidate for candidate in closed if sum(weights[b] for b in candidate) == best]
        selected = {b for b, entry in enumerate(first) if entry is not None and entry <= k}
        assert selected in optimal, (k, selected, optimal, weights)
        assert previous <= selected
        assert int(shell['scaled_value']) == best
        assert int(shell['value']['numerator']) == best
        assert int(shell['value']['denominator']) == declaration['per_unit'] * den
        assert Q(int(shell['value']['numerator']), int(shell['value']['denominator'])) == Q(
            sum(weights[b] for b in selected), declaration['per_unit'] * den)
        assert shell['blocks'] == len(selected)
        assert shell['added_blocks'] == len(selected - previous)
        assert shell['solved_pit_blocks'] in {len(candidate) for candidate in optimal}
        assert shell['solved_pit_blocks'] <= shell['blocks']
        previous = selected
    return payload


def shells():
    declaration, attributes = spec(), data()
    write_spec(declaration)
    fixture.write_omf(attributes, units=UNITS)
    report = run()
    payload = verify(report, declaration, attributes)
    assert payload['first_shell'] == [1, 2, 1, 1]
    assert [s['blocks'] for s in payload['shells']] == [0, 3, 4, 4]
    assert [int(s['scaled_value']) for s in payload['shells']] == [0, 10, 46, 40]
    assert run() == report
    # Preserve unreduced declaration factors and their exact denominator scale.
    declaration['factors'] = [{'num': n, 'den': d} for n, d in [(2, 4), (2, 2), (6, 4), (6, 3)]]
    write_spec(declaration)
    verify(run(), declaration, attributes)
    # This block never enters; the omission must remain null in original OMF order.
    declaration = spec()
    attributes['revenue'][1] = 0
    write_spec(declaration)
    fixture.write_omf(attributes, units=UNITS)
    assert verify(run(), declaration, attributes)['first_shell'] == [1, None, 1, 1]
    # Signed costs are allowed. Their half-away rounding must agree with positive-cost rounding.
    attributes = data()
    attributes['cost'][1] = -3.125
    fixture.write_omf(attributes, units=UNITS)
    verify(run(), declaration, attributes)
    # Exact neighboring i64 ratios would compare equal after conversion to binary64.
    maximum = 2**63 - 1
    declaration['factors'] = [{'num': maximum - 1, 'den': maximum}, {'num': maximum, 'den': maximum}]
    attributes = {'revenue': [0] * 4, 'cost': [0] * 4}
    write_spec(declaration)
    fixture.write_omf(attributes, units=UNITS)
    verify(run(), declaration, attributes)
    rng = random.Random(194251)
    for _ in range(8):
        declaration = spec()
        attributes = {'revenue': [rng.randint(0, 32) / 8 for _ in range(4)],
                      'cost': [rng.randint(-8, 28) / 8 for _ in range(4)]}
        write_spec(declaration)
        fixture.write_omf(attributes, units=UNITS)
        verify(run(), declaration, attributes)


def sources():
    declaration, attributes = spec(), data()
    write_spec(declaration)
    fixture.write_omf(attributes, units=UNITS)
    original = run()
    verify(original, declaration, attributes)
    # A byte-only declaration change still changes its binding, with identical mathematical shells.
    (ROOT / 'shells.json').write_text(json.dumps(declaration, indent=2) + '\n', encoding='utf-8')
    whitespace = run()
    verify(whitespace, declaration, attributes)
    assert whitespace['payload']['sources']['omf'] == original['payload']['sources']['omf']
    assert whitespace['payload']['sources']['spec']['sha256'] != original['payload']['sources']['spec']['sha256']
    assert whitespace['payload']['shells'] == original['payload']['shells']
    assert whitespace['payload']['first_shell'] == original['payload']['first_shell']
    attributes['revenue'][0] += 0.25
    fixture.write_omf(attributes, units=UNITS)
    changed = run()
    verify(changed, declaration, attributes)
    assert changed['payload']['sources']['omf']['sha256'] != whitespace['payload']['sources']['omf']['sha256']
    assert changed['payload']['sources']['spec'] == whitespace['payload']['sources']['spec']
    assert changed['payload']['shells'] != whitespace['payload']['shells']
    no_list = run(arguments=['plan', 'shells', '--omf', 'section.omf', '--spec', 'shells.json'])
    assert no_list['payload'].get('first_shell') is None
    assert no_list['payload']['shells'] == changed['payload']['shells']
    markdown = subprocess.run([str(BINARY), 'plan', 'shells', '--omf', 'section.omf', '--spec',
                               'shells.json', '--format', 'markdown', '--workspace', str(ROOT)],
                              capture_output=True, text=True, check=False)
    assert markdown.returncode == 0, markdown.stderr
    for name in ['section.omf', 'shells.json']:
        assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() in markdown.stdout
    assert 'fmine.omf-shells.v1' in json.dumps(run(arguments=['robot-docs']))


def refusals():
    declaration, attributes = spec(), data()
    fixture.write_omf(attributes, units=UNITS)
    invalid = []
    for key in declaration:
        missing = deepcopy(declaration)
        del missing[key]
        invalid.append(missing)
    for path in [(), ('slope',), ('factors', 0)]:
        unknown = deepcopy(declaration)
        target = unknown
        for part in path:
            target = target[part]
        target['undeclared'] = 1
        invalid.append(unknown)
    for key, value in [('schema', 'fmine.omf-shells.v999'), ('per_unit', 0), ('per_unit', 1.5),
                       ('value_rounding', 'nearest'), ('factors', []), ('revenue_attribute', ''),
                       ('factors', [{'num': 1, 'den': 0}]), ('factors', [{'num': -1, 'den': 2}]),
                       ('factors', [{'num': 0, 'den': 1}]), ('factors', [{'num': 1.5, 'den': 2}]),
                       ('factors', [{'num': n, 'den': 1} for n in range(1, 66)])]:
        invalid.append({**declaration, key: value})
    for bad in invalid:
        write_spec(bad)
        run(success=False)
    for factors in [[{'num': 2, 'den': 1}, {'num': 1, 'den': 1}],
                    [{'num': 1, 'den': 2}, {'num': 2, 'den': 4}]]:
        write_spec({**declaration, 'factors': factors})
        assert run(success=False)['refusal']['code'] == 'FMINE-REF-207'
    raw = json.dumps(declaration)
    for duplicate in [raw.replace('"per_unit": 4', '"per_unit": 4, "per_unit": 4'),
                      raw.replace('"num": 1', '"num": 1, "num": 1', 1)]:
        (ROOT / 'shells.json').write_text(duplicate, encoding='utf-8')
        run(success=False)
    write_spec(declaration)
    for units, code in [({'revenue': 'USD/block', 'cost': 'CAD/block'}, 'FMINE-REF-105'),
                        ({'revenue': '', 'cost': ''}, 'FMINE-REF-107'),
                        ({'revenue': ' ', 'cost': 'USD/block'}, 'FMINE-REF-107')]:
        fixture.write_omf(attributes, units=units)
        assert run(success=False)['refusal']['code'] == code
    for defect in ['duplicate_attribute', 'duplicate_model', 'tilted']:
        fixture.write_omf(attributes, defect, units=UNITS)
        run(success=False)
    for key in ['revenue', 'cost']:
        for bad in [None, float('nan')]:
            changed = deepcopy(attributes)
            changed[key][0] = bad
            fixture.write_omf(changed, units=UNITS)
            run(success=False)
    changed = deepcopy(attributes)
    changed['revenue'][0] = -0.125
    fixture.write_omf(changed, units=UNITS)
    assert run(success=False)['refusal']['code'] == 'FMINE-REF-207'
    fixture.write_omf({'revenue': attributes['revenue']}, units=UNITS)
    run(success=False)
    fixture.write_omf(attributes, units=UNITS)
    write_spec({**declaration, 'factors': [{'num': 2**62, 'den': 1}]})
    assert run(success=False)['refusal']['code'] == 'FMINE-REF-207'


def command():
    declaration, attributes = spec(), data()
    write_spec(declaration)
    fixture.write_omf(attributes, units=UNITS)
    verify(run(), declaration, attributes)
    for extra in [['--upit', 'x'], ['--prec', 'x'], ['--pcpsp', 'x'], ['--candidate', 'x'],
                  ['--per-unit', '4'], ['--block-model', 'section'], ['--decimals', '0'],
                  ['--round'], ['--lp-guidance'], ['--list-schedule'], ['--list-blocks'],
                  ['--omf', 'x'], ['--spec', 'x'], ['--unknown']]:
        assert run(extra, success=False)['exit_code'] == 2
    for args in [['plan', 'shells', '--omf', 'section.omf'],
                 ['plan', 'shells', '--spec', 'shells.json'],
                 ['plan', 'pit', '--omf', 'section.omf', '--spec', 'shells.json']]:
        assert run(arguments=args, success=False)['exit_code'] == 2
    for args in [['plan', 'shells', '--omf', 'absent.omf', '--spec', 'shells.json'],
                 ['plan', 'shells', '--omf', '.', '--spec', 'shells.json'],
                 ['plan', 'shells', '--omf', 'section.omf', '--spec', '.']]:
        assert run(arguments=args, success=False)['refusal']['code'] == 'FMINE-REF-506'
    (ROOT / 'shells.json').write_text(' ' * (1024*1024 + 1), encoding='utf-8')
    assert run(success=False)['refusal']['code'] == 'FMINE-REF-506'


if __name__ == '__main__':
    globals()[sys.argv[2]]()
    print('OMF shells oracle passed:', sys.argv[2])
