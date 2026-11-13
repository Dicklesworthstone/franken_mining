"""Synthetic closed boxes through the real design verb, with exact independent oracles.

Reuse the independent stdlib OMF writer's Number columns and Thrift primitives.
Only the required Surface columns are added here. Box intersections, sub-cell
counts and optimal closed sets use Fraction and enumeration, never Rust output.
"""

from copy import deepcopy
from fractions import Fraction as Q
from itertools import product
from pathlib import Path
import gzip
import hashlib
import importlib.util
import io
import json
import struct
import subprocess
import sys
import zipfile

fixture_spec = importlib.util.spec_from_file_location(
    'omf_synthetic_fixture', Path(__file__).with_name('plan_schedule_omf_oracle.py'))
fixture = importlib.util.module_from_spec(fixture_spec)
fixture_spec.loader.exec_module(fixture)
BINARY, ROOT = fixture.BINARY, fixture.ROOT
SIZE = Q(25, 2)
COUNT = (2, 1, 2)
BOX = ((Q(25, 8), Q(0), Q(0)), (Q(175, 8), SIZE, 2 * SIZE))
UNITS = {'value': 'USD/block', 'tonnes': 'dry tonnes/block'}
FACES = [(0, 2, 1), (1, 2, 3), (4, 5, 6), (5, 7, 6),
         (0, 1, 4), (1, 5, 4), (2, 6, 3), (3, 6, 7),
         (0, 4, 2), (2, 4, 6), (1, 3, 5), (3, 7, 5)]


def surface_columns(rows, names, *, unsigned=False):
    """Required PLAIN DOUBLE or UINT32 columns, one uncompressed v1 page each."""
    integer, text, fields, sequence = fixture.integer, fixture.text, fixture.fields, fixture.sequence
    physical = 1 if unsigned else 5
    data, columns, leaves = bytearray(b'PAR1'), [], []
    for axis, name in enumerate(names):
        body = b''.join(struct.pack('<I' if unsigned else '<d', row[axis]) for row in rows)
        page = fields([(1, integer(0)), (2, integer(len(body))), (3, integer(len(body))),
                       (5, fields([(1, integer(len(rows))), (2, integer(0)),
                                   (3, integer(3)), (4, integer(3))]))])[1] + body
        offset = len(data)
        data.extend(page)
        schema = [(1, integer(physical)), (3, integer(0)), (4, text(name))]
        if unsigned:
            schema.append((6, integer(13)))  # Parquet UINT_32 converted type.
        leaves.append(fields(schema))
        columns.append(fields([(2, integer(offset, 6)), (3, fields([
            (1, integer(physical)), (2, sequence([integer(0), integer(3)])),
            (3, sequence([text(name)])), (4, integer(0)), (5, integer(len(rows), 6)),
            (6, integer(len(page), 6)), (7, integer(len(page), 6)), (9, integer(offset, 6))]))]))
    schema = sequence([fields([(4, text('schema')), (5, integer(len(names)))])] + leaves)
    group = fields([(1, sequence(columns)), (2, integer(len(data) - 4, 6)),
                    (3, integer(len(rows), 6))])
    footer = fields([(1, integer(1)), (2, schema), (3, integer(len(rows), 6)),
                     (4, sequence([group]))])[1]
    return bytes(data) + footer + struct.pack('<I', len(footer)) + b'PAR1'


def write_omf(attributes, box=BOX, *, count=COUNT, units=None, defect=None, origin=(0, 0, 0)):
    fixture.COUNT = count
    fixture.write_omf(attributes, defect, units=UNITS if units is None else units)
    path = ROOT / 'section.omf'
    with zipfile.ZipFile(path) as archive:
        members = {name: archive.read(name) for name in archive.namelist()}
    project = json.loads(gzip.decompress(members['index.json.gz']))
    project['origin'] = [10_000_000, -20_000_000, 30_000_000]
    project['elements'][0]['geometry']['orient']['origin'] = list(origin)
    vertices = [[float(box[(corner >> axis) & 1][axis]) for axis in range(3)]
                for corner in range(8)]
    faces = list(FACES)
    if defect == 'open':
        faces = faces[1:]
    elif defect == 'inward':
        faces = [(a, c, b) for a, b, c in faces]
    elif defect == 'triangle_work':
        faces *= 10
    elif defect == 'vertex_limit':
        vertices.extend([[0.0, 0.0, 0.0]] * (100_001 - len(vertices)))
    surface = {'name': 'design', 'geometry': {'type': 'Surface', 'origin': list(origin),
               'vertices': {'filename': 'vertices.parquet', 'item_count': len(vertices)},
               'triangles': {'filename': 'triangles.parquet', 'item_count': len(faces)}}}
    project['elements'].append(surface)
    if defect == 'duplicate_surface':
        project['elements'].append(deepcopy(surface))
    members['vertices.parquet'] = surface_columns(vertices, ('x', 'y', 'z'))
    members['triangles.parquet'] = surface_columns(faces, ('a', 'b', 'c'), unsigned=True)
    members['index.json.gz'] = gzip.compress(json.dumps(project).encode(), mtime=0)
    output = io.BytesIO()
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_STORED) as archive:
        for name, contents in members.items():
            archive.writestr(zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0)), contents)
        archive.comment = b'Open Mining Format 2.0'
    path.write_bytes(output.getvalue())


def spec():
    return {'schema': 'fmine.omf-design.v1', 'block_model': 'section',
            'value_attribute': 'value', 'value_per_unit': 4,
            'value_rounding': 'nearest_integer_half_away_from_zero',
            'surface': 'design', 'depth': 2, 'slope': {'rise': 1, 'run': 1, 'levels': 1},
            'quantities': [{'attribute': 'tonnes', 'per_unit': 10}]}


def data():
    return {'value': [7.125, -1.125, -1.25, -1.375], 'tonnes': [1.2, 1.4, 1.6, 1.8]}


def write_spec(declaration):
    (ROOT / 'design.json').write_text(json.dumps(declaration), encoding='utf-8')


def run(extra=(), *, success=True, arguments=None):
    args = arguments if arguments is not None else [
        'plan', 'design', '--omf', 'section.omf', '--spec', 'design.json', '--list-blocks']
    result = subprocess.run([str(BINARY), *args, *extra, '--workspace', str(ROOT)],
                            capture_output=True, text=True, check=False, timeout=30)
    report = json.loads(result.stdout)
    assert result.returncode == report['exit_code'], (result.stderr, report)
    assert all(value is False for value in report['flags'].values()), report['flags']
    assert (result.returncode == 0 and report['status'] == 'ok') == success, report
    return report


def coordinates(count):
    return [(u, v, w) for w in range(count[2]) for v in range(count[1]) for u in range(count[0])]


def analytic_box(box, depth, count=COUNT):
    side = 2 ** depth
    counts, fractions = [], []
    for block in coordinates(count):
        inside, possible, fraction = 1, 1, Q(1)
        for axis, index in enumerate(block):
            lo, hi = index * SIZE, (index + 1) * SIZE
            fraction *= max(Q(0), min(hi, box[1][axis]) - max(lo, box[0][axis])) / SIZE
            pieces = [(lo + j * SIZE / side, lo + (j + 1) * SIZE / side) for j in range(side)]
            inside *= sum(box[0][axis] <= a and b <= box[1][axis] for a, b in pieces)
            possible *= sum(max(a, box[0][axis]) < min(b, box[1][axis]) for a, b in pieces)
        counts.append([inside, possible])
        fractions.append(fraction)
    return counts, fractions


def totals(weights, selected, fractions):
    shell = sum(w for w, inside in zip(weights, selected) if inside)
    design = sum(w * part for w, part in zip(weights, fractions))
    left = sum(w * (1 - part) for w, part, inside in zip(weights, fractions, selected) if inside)
    added = sum(w * part for w, part, inside in zip(weights, fractions, selected) if not inside)
    return {'shell': shell, 'design': design, 'left': left, 'added': added, 'loss': shell - design}


def verify(report, declaration, attributes, box=BOX):
    payload = report['payload']
    assert payload['schema'] == 'fmine.plan-design.omf.v1'
    assert payload['block_order'] == 'omf_u_then_v_then_w_bottom_up'
    assert payload['model']['length_units'] == 'm'
    assert payload['model']['coordinate_reference_system'] == 'EPSG:32633'
    for key, name in [('omf', 'section.omf'), ('spec', 'design.json')]:
        original = (ROOT / name).read_bytes()
        assert payload['sources'][key]['sha256'] == hashlib.sha256(original).hexdigest()
        assert payload['sources'][key]['bytes'] == len(original)
    cells = 8 ** declaration['depth']
    assert payload['depth'] == declaration['depth'] and payload['cells_per_block'] == cells
    expected_cells, fractions = analytic_box(box, declaration['depth'])
    assert payload['design_cells'] == expected_cells, (payload['design_cells'], expected_cells)
    selected = payload['shell_membership']
    assert len(selected) == len(coordinates(COUNT)) and all(type(x) is bool for x in selected)
    assert payload['blocks'] == len(selected) and payload['shell_blocks'] == sum(selected)
    requires = fixture.requirements(declaration)
    values = [fixture.nearest(Q(str(x)) * declaration['value_per_unit']) for x in attributes['value']]
    feasible = [mask for mask in product((False, True), repeat=len(values))
                if all(not mask[b] or all(mask[p] for p in requires[b]) for b in range(len(mask)))]
    optimum = max(sum(w for w, included in zip(values, mask) if included) for mask in feasible)
    assert tuple(selected) in feasible
    assert sum(w for w, included in zip(values, selected) if included) == optimum
    certificate = payload['certificate']
    assert certificate['integer_shell_verified'] is True
    assert int(certificate['scaled_shell_value']) == optimum
    for key in ['design_slopes_checked', 'flow_witness_exported', 'statistical_confidence_interval']:
        assert certificate[key] is False
    quantities = [{'attribute': 'value', 'per_unit': declaration['value_per_unit']}, *declaration['quantities']]
    assert [row['attribute'] for row in payload['quantities']] == [q['attribute'] for q in quantities]
    for row, quantity in zip(payload['quantities'], quantities):
        name, scale = quantity['attribute'], quantity['per_unit']
        assert row['units'] == UNITS[name] and row['per_unit'] == scale
        assert row['uniform_within_parent_block'] is True
        weights = [Q(fixture.nearest(Q(str(x)) * scale), scale) for x in attributes[name]]
        truth = totals(weights, selected, fractions)
        # Exhaust all endpoint choices; this oracle does not use the kernel's interval reduction.
        extremes = [totals(weights, selected, parts)
                    for parts in product(*[(Q(lo, cells), Q(hi, cells)) for lo, hi in expected_cells])]
        for key, actual in truth.items():
            bound = row[key]
            assert all(isinstance(bound[k], str) for k in ['lower', 'upper', 'denominator'])
            assert int(bound['denominator']) == scale * cells
            lo, hi = [Q(int(bound[k]), int(bound['denominator'])) for k in ['lower', 'upper']]
            assert lo <= actual <= hi, (name, key, lo, actual, hi)
            assert [lo, hi] == [min(e[key] for e in extremes), max(e[key] for e in extremes)]
    return payload


def geometry():
    declaration, attributes = spec(), data()
    write_spec(declaration)
    write_omf(attributes)
    original = run()
    payload = verify(original, declaration, attributes)
    assert payload['shell_membership'] == [True, False, True, True]
    assert payload['design_cells'] == [[48, 48]] * 4
    assert run() == original
    # The infeasible design omits required waste above the ore: a negative loss must remain negative.
    ore_only = ((Q(0), Q(0), Q(0)), (SIZE, SIZE, SIZE))
    write_omf(attributes, ore_only)
    payload = verify(run(), declaration, attributes, ore_only)
    assert int(payload['quantities'][0]['loss']['upper']) < 0
    assert payload['design_cells'] == [[64, 64], [0, 0], [0, 0], [0, 0]]
    # Non-lattice faces have analytically known bands; deeper enclosures become strictly tighter.
    rough = ((Q(1), Q(0), Q(0)), (Q(24), SIZE, 2 * SIZE))
    write_omf(attributes, rough)
    widths = []
    for depth in range(5):
        declaration['depth'] = depth
        write_spec(declaration)
        row = verify(run(), declaration, attributes, rough)['quantities'][0]['design']
        widths.append(Q(int(row['upper']) - int(row['lower']), int(row['denominator'])))
    assert all(a > b for a, b in zip(widths, widths[1:])), widths
    # Shared origins cancel, while each element's own translation is still applied.
    declaration = spec()
    write_spec(declaration)
    write_omf(attributes, origin=(1000, -2000, 300))
    translated = verify(run(), declaration, attributes)
    assert translated['quantities'] == original['payload']['quantities']


def sources():
    declaration, attributes = spec(), data()
    write_spec(declaration)
    write_omf(attributes)
    original = verify(run(), declaration, attributes)
    (ROOT / 'design.json').write_text(json.dumps(declaration, indent=2) + '\n', encoding='utf-8')
    whitespace = verify(run(), declaration, attributes)
    assert whitespace['quantities'] == original['quantities']
    assert whitespace['sources']['spec'] != original['sources']['spec']
    assert whitespace['sources']['omf'] == original['sources']['omf']
    attributes['value'][0] += 1
    write_omf(attributes)
    changed = verify(run(), declaration, attributes)
    assert changed['sources']['omf'] != whitespace['sources']['omf']
    assert changed['sources']['spec'] == whitespace['sources']['spec']
    assert changed['quantities'] != whitespace['quantities']
    args = ['plan', 'design', '--omf', 'section.omf', '--spec', 'design.json']
    compact = run(arguments=args)['payload']
    assert 'shell_membership' not in compact and 'design_cells' not in compact
    assert compact['quantities'] == changed['quantities']
    markdown = subprocess.run([str(BINARY), *args, '--format', 'markdown', '--workspace', str(ROOT)],
                              capture_output=True, text=True, check=False, timeout=30)
    assert markdown.returncode == 0, markdown.stderr
    assert 'Shell minus design' in markdown.stdout and 'negative' in markdown.stdout
    for source in changed['sources'].values():
        assert source['sha256'] in markdown.stdout
    assert 'fmine.omf-design.v1' in json.dumps(run(arguments=['robot-docs'])['payload']['plan_design'])
    declaration['quantities'] = []
    write_spec(declaration)
    assert len(verify(run(), declaration, attributes)['quantities']) == 1


def refusals():
    declaration, attributes = spec(), data()
    write_omf(attributes)
    invalid = [{k: v for k, v in declaration.items() if k != absent} for absent in declaration]
    for path in [(), ('slope',), ('quantities', 0)]:
        bad = deepcopy(declaration)
        target = bad
        for key in path:
            target = target[key]
        target['unknown'] = 1
        invalid.append(bad)
    for key, value in [('schema', 'fmine.omf-design.v999'), ('value_per_unit', 0),
                       ('value_per_unit', 1.5), ('value_rounding', 'floor'), ('surface', ''),
                       ('depth', -1), ('depth', 7), ('depth', 1.5),
                       ('quantities', [{'attribute': 'value', 'per_unit': 1}]),
                       ('quantities', [{'attribute': 'tonnes', 'per_unit': 0}]),
                       ('quantities', declaration['quantities'] * 2),
                       ('quantities', declaration['quantities'] * 17)]:
        invalid.append({**declaration, key: value})
    for bad in invalid:
        write_spec(bad)
        assert run(success=False)['refusal']['code'] == 'FMINE-REF-506'
    raw = json.dumps(declaration)
    (ROOT / 'design.json').write_text(raw.replace('"depth": 2', '"depth": 2, "depth": 2'),
                                    encoding='utf-8')
    assert run(success=False)['refusal']['code'] == 'FMINE-REF-506'
    write_spec(declaration)
    for name in UNITS:
        write_omf(attributes, units={**UNITS, name: ' '})
        assert run(success=False)['refusal']['code'] == 'FMINE-REF-107'
    for defect, code in [('open', '207'), ('inward', '207'), ('duplicate_surface', '506'),
                         ('duplicate_attribute', '506'), ('duplicate_model', '506')]:
        write_omf(attributes, defect=defect)
        assert run(success=False)['refusal']['code'] == 'FMINE-REF-' + code
    outside = ((Q(-1), Q(0), Q(0)), BOX[1])
    write_omf(attributes, outside)
    assert run(success=False)['refusal']['code'] == 'FMINE-REF-109'
    write_omf(attributes)
    for extra in [['--list-blocks'], ['--omf', 'x'], ['--spec', 'x'], ['--round'],
                  ['--per-unit', '4'], ['--candidate', 'x'], ['--unknown']]:
        assert run(extra, success=False)['exit_code'] == 2
    for args in [['plan', 'design', '--omf', 'section.omf'],
                 ['plan', 'design', '--spec', 'design.json']]:
        assert run(arguments=args, success=False)['exit_code'] == 2
    (ROOT / 'design.json').write_text(' ' * (1024 * 1024 + 1), encoding='utf-8')
    assert run(success=False)['refusal']['code'] == 'FMINE-REF-506'


def work():
    declaration = spec()
    declaration['depth'] = 6
    write_spec(declaration)
    # Both product limits must refuse before geometry subdivision or invalid duplicated-triangle checks.
    for count, defect in [((2, 1, 4), None), (COUNT, 'triangle_work')]:
        n = count[0] * count[1] * count[2]
        write_omf({'value': [1] * n, 'tonnes': [1] * n}, count=count, defect=defect)
        refusal = run(success=False)['refusal']
        assert refusal['code'] == 'FMINE-REF-207' and 'geometry work budget exceeded' in refusal['what']
    write_spec(spec())
    write_omf(data(), defect='vertex_limit')
    refusal = run(success=False)['refusal']
    assert refusal['code'] == 'FMINE-REF-207' and 'geometry work budget exceeded' in refusal['what']


if __name__ == '__main__':
    globals()[sys.argv[2]]()
    print('OMF design oracle passed:', sys.argv[2])
