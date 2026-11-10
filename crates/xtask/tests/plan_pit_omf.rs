//! BD-194/250: real OMF bytes through the planning verb, with an independent geometric closure oracle.
//! All block values and slopes here are synthetic declarations, not mine or geotechnical evidence.

#![cfg(test)]

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-plan-pit-omf-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

// A single optional DOUBLE column, PLAIN data, RLE definition levels, uncompressed Parquet v1 pages.
// Python's standard ZIP/gzip writers supply independently encoded archive headers and CRCs.
// Neither the candidate reader nor its testkit writes these bytes. Geometry/value oracles below do
// not depend on this serializer or on the production slope/closure algorithms.
const FIXTURE: &str = r#"
import gzip, io, json, struct, sys, zipfile

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
    s = s.encode()
    return 8, varint(len(s)) + s

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
    # Every row has its own RLE run, so a null exercises optional-value decoding too.
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

case = json.load(sys.stdin)
values, defect = case['values'], case['defect']
if defect == 'nonfinite': values[0] = float('nan')
geometry = {'type':'BlockModel', 'orient':{'origin':[0,0,0]},
            'grid':{'type':'Regular', 'size':[12.5,12.5,12.5], 'count':case['count']}}
if defect == 'tilted' or defect.endswith('_before_geometry'):
    geometry['orient'].update(v=[0,0.6,0.8],w=[0,-0.8,0.6])
if defect == 'nonbinary_size': geometry['grid']['size'][0] = 0.1
attribute = {'name':'value','units':'USD','location':'Primitives',
             'data':{'type':'Number','values':{'filename':'values.parquet','item_count':len(values)}}}
element = {'name':'section','geometry':geometry,'attributes':[attribute]}
if defect == 'missing_attribute_before_geometry': element['attributes'] = []
if defect == 'ambiguous_attribute_before_geometry': element['attributes'] = [attribute,attribute]
project = {'name':'Synthetic planning section','units':'m','coordinate_reference_system':'EPSG:32633',
           'date':'2026-01-01T00:00:00Z',
           'elements':[element,element] if defect == 'ambiguous' else [element]}
out = io.BytesIO()
with zipfile.ZipFile(out,'w',compression=zipfile.ZIP_STORED) as z:
    for name,data in [('index.json.gz',gzip.compress(json.dumps(project).encode(),mtime=0)),
                      ('values.parquet',parquet(values))]:
        z.writestr(zipfile.ZipInfo(name,date_time=(1980,1,1,0,0,0)),data)
    z.comment = b'Open Mining Format 2.0'
sys.stdout.buffer.write(out.getvalue())
"#;

fn fixture(root: &Path, count: [u32; 3], values: &[Option<f64>], defect: &str) -> Vec<u8> {
    let values: Vec<_> = values
        .iter()
        .map(|v| v.map_or_else(|| "null".into(), |v| v.to_string()))
        .collect();
    let json = format!(
        "{{\"count\":{count:?},\"values\":[{}],\"defect\":\"{defect}\"}}",
        values.join(",")
    );
    let mut child = Command::new("python3")
        .args(["-I", "-c", FIXTURE])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(json.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    fs::write(root.join("section.omf"), &output.stdout).unwrap();
    output.stdout
}

fn args(rise: u32, run: u32, levels: u32) -> Vec<String> {
    [
        "plan",
        "pit",
        "--omf",
        "section.omf",
        "--block-model",
        "section",
        "--value-attribute",
        "value",
        "--per-unit",
        "4",
        "--slope-rise",
        &rise.to_string(),
        "--slope-run",
        &run.to_string(),
        "--slope-levels",
        &levels.to_string(),
        "--round",
        "--list-blocks",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn run(root: &Path, args: &[String]) -> Envelope {
    let (envelope, format) = invoke(args, root);
    assert_eq!(format, OutputFormat::Json);
    envelope
}

fn ids(json: &str) -> Vec<usize> {
    json.split_once("\"in_pit\":[")
        .unwrap()
        .1
        .split(']')
        .next()
        .unwrap()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect()
}

fn value(json: &str) -> i64 {
    json.split_once("\"scaled_value\":\"")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn section(ore: f64) -> Vec<Option<f64>> {
    (0..15)
        .map(|id| Some(if id == 2 { ore } else { -1.0 }))
        .collect()
}

#[test]
fn omf_pit_reads_real_bytes_preserves_order_and_binds_every_source() {
    let root = root();
    let bytes = fixture(&root, [5, 1, 3], &section(9.25), "");
    let output = run(&root, &args(1, 1, 2));
    let json = output.to_json();
    assert_eq!(output.exit_code, 0, "{json}");
    // Bottom centre plus three middle and five top blocks: 37 - 8*4 = 5 quarter-units.
    assert_eq!(value(&json), 5);
    assert_eq!(ids(&json), vec![2, 6, 7, 8, 10, 11, 12, 13, 14]);
    for text in [
        "\"schema\":\"fmine.plan-pit.omf.v1\"",
        "\"format\":\"omf2\"",
        "\"value_units\":\"USD\"",
        "\"length_units\":\"m\"",
        "\"coordinate_reference_system\":\"EPSG:32633\"",
        "\"per_unit\":4",
        "\"value\":\"5/4\"",
        "\"pit_blocks\":9",
        "\"rise\":1,\"run\":1,\"levels\":2",
        "\"verified\":true",
        "\"raw_attribute_optimality_bound\":null",
        "\"block_order\":\"omf_u_then_v_then_w_bottom_up\"",
        "\"reserve_disclosure_claim\":false",
        "\"execution_authorized\":false",
        "\"geotechnical_signoff\":false",
        "\"regulatory_compliance_claim\":false",
    ] {
        assert!(json.contains(text), "missing {text}: {json}");
    }
    assert!(json.contains(&sha256(&bytes).to_hex()));
    assert_eq!(fs::read(root.join("section.omf")).unwrap(), bytes);
    assert_eq!(run(&root, &args(1, 1, 2)).to_json(), json);
    assert!(output.no_claim.unwrap().contains("scaled integer"));
    assert!(output.rendered_markdown.unwrap().contains("Source SHA-256"));
}

// Directly impose every declared upward cone on OMF-order coordinates; never use the production
// arc reduction, OMF order mapping, solver, or certificate to calculate the expected objective.
fn brute_force(
    weights: &[i64],
    count: [u32; 3],
    rise: u32,
    run: u32,
    levels: u32,
) -> (i64, Vec<Vec<usize>>) {
    let [nx, ny, _] = count.map(i64::from);
    let coordinates = |b: usize| {
        let b = b as i64;
        (b % nx, (b / nx) % ny, b / (nx * ny))
    };
    let required: Vec<Vec<usize>> = (0..weights.len())
        .map(|b| {
            let (u, v, w) = coordinates(b);
            (0..weights.len())
                .filter(|&p| {
                    let (x, y, z) = coordinates(p);
                    let dz = z - w;
                    dz > 0
                        && dz <= i64::from(levels)
                        && ((x - u).pow(2) + (y - v).pow(2)) * i64::from(rise).pow(2)
                            <= dz.pow(2) * i64::from(run).pow(2)
                })
                .collect()
        })
        .collect();
    let value = (0_u32..1 << weights.len())
        .filter(|set| {
            (0..weights.len())
                .all(|b| (set >> b) & 1 == 0 || required[b].iter().all(|p| (set >> p) & 1 == 1))
        })
        .map(|set| {
            weights
                .iter()
                .enumerate()
                .filter(|(b, _)| (set >> b) & 1 == 1)
                .map(|(_, v)| v)
                .sum()
        })
        .max()
        .unwrap();
    (value, required)
}

#[test]
fn omf_pits_match_independent_geometric_enumeration_at_multiple_slopes() {
    let root = root();
    for case in 0_i64..8 {
        let weights: Vec<_> = (0_i64..12).map(|b| (b * 13 + case * 7) % 39 - 17).collect();
        let values: Vec<_> = weights.iter().map(|&v| Some(v as f64 / 4.0)).collect();
        fixture(&root, [3, 2, 2], &values, "");
        for (rise, slope_run) in [(1, 1), (2, 1), (1, 2)] {
            let output = run(&root, &args(rise, slope_run, 1));
            let json = output.to_json();
            assert_eq!(output.exit_code, 0, "{json}");
            let (expected, required) = brute_force(&weights, [3, 2, 2], rise, slope_run, 1);
            assert_eq!(
                value(&json),
                expected,
                "case {case}, slope {rise}/{slope_run}"
            );
            let selected = ids(&json);
            assert_eq!(
                selected.iter().map(|&id| weights[id]).sum::<i64>(),
                expected
            );
            assert!(
                selected
                    .iter()
                    .all(|&b| required[b].iter().all(|p| selected.contains(p)))
            );
        }
    }
}

#[test]
fn omf_pit_rounding_and_perturbed_ore_change_the_integer_model() {
    let root = root();
    let mut previous = String::new();
    for (ore, expected) in [(9.25, 5), (7.75, 0), (8.125, 1)] {
        let bytes = fixture(&root, [5, 1, 3], &section(ore), "");
        let output = run(&root, &args(1, 1, 2));
        let json = output.to_json();
        assert_eq!(output.exit_code, 0, "{json}");
        assert_eq!(value(&json), expected);
        assert!(json.contains(&sha256(&bytes).to_hex()));
        assert_ne!(json, previous);
        previous = json;
    }
    let mut weights = section(9.25);
    weights[7] = Some(-1.125); // -4.5 scaled units round to -5, not -4.
    fixture(&root, [5, 1, 3], &weights, "");
    assert_eq!(value(&run(&root, &args(1, 1, 2)).to_json()), 4);
}

#[test]
fn omf_pit_refuses_ambiguous_missing_and_unsupported_inputs() {
    let root = root();
    for (defect, code, detail) in [
        ("ambiguous", "FMINE-REF-506", "section"),
        ("tilted", "FMINE-REF-109", "W straight up"),
        ("nonbinary_size", "FMINE-REF-109", "not integers"),
        ("nonfinite", "FMINE-REF-109", "not a finite i64"),
        // Attribute admission must precede PitModel construction, including geometry and slope allocation.
        (
            "missing_attribute_before_geometry",
            "FMINE-REF-506",
            "0 attributes",
        ),
        (
            "ambiguous_attribute_before_geometry",
            "FMINE-REF-506",
            "2 attributes",
        ),
    ] {
        let bytes = fixture(&root, [5, 1, 3], &section(9.25), defect);
        let output = run(&root, &args(1, 1, 2));
        assert_eq!(output.status, "refused", "{}", output.to_json());
        let refusal = output.refusal.unwrap();
        assert_eq!(refusal.code, code);
        assert!(refusal.what.contains(detail), "{}", refusal.what);
        assert!(refusal.what.contains(&sha256(&bytes).to_hex()));
    }
    let mut missing = section(9.25);
    missing[7] = None;
    fixture(&root, [5, 1, 3], &missing, "");
    assert!(
        run(&root, &args(1, 1, 2))
            .refusal
            .unwrap()
            .what
            .contains("has no value")
    );
    fs::write(root.join("section.omf"), b"not an OMF archive").unwrap();
    assert_eq!(
        run(&root, &args(1, 1, 2)).refusal.unwrap().code,
        "FMINE-REF-506"
    );
    let mut call = args(1, 1, 2);
    call[3] = "absent.omf".into();
    assert_eq!(run(&root, &call).exit_code, 5);
    call[3] = ".".into();
    assert!(
        run(&root, &call)
            .refusal
            .unwrap()
            .what
            .contains("regular file")
    );
}

#[test]
fn omf_pit_parser_requires_explicit_scaling_and_rejects_cross_mode_options() {
    let root = root();
    let base = args(1, 1, 2);
    for flag in [
        "--round",
        "--block-model",
        "--value-attribute",
        "--per-unit",
        "--slope-rise",
        "--slope-run",
        "--slope-levels",
    ] {
        let mut call = base.clone();
        let position = call.iter().position(|v| v == flag).unwrap();
        call.drain(position..position + if flag == "--round" { 1 } else { 2 });
        assert_eq!(run(&root, &call).exit_code, 2, "{flag}");
    }
    for (flag, value) in [
        ("--upit", "x"),
        ("--prec", "x"),
        ("--decimals", "4"),
        ("--candidate", "x"),
        ("--per-unit", "4"),
        ("--slope-rise", "1"),
    ] {
        let mut call = base.clone();
        call.extend([flag.into(), value.into()]);
        assert_eq!(run(&root, &call).exit_code, 2, "{flag}");
    }
    for flag in [
        "--per-unit",
        "--slope-rise",
        "--slope-run",
        "--slope-levels",
    ] {
        for invalid in ["0", "-1", "1.5", "4294967296"] {
            let mut call = base.clone();
            let position = call.iter().position(|v| v == flag).unwrap();
            call[position + 1] = invalid.into();
            assert_eq!(run(&root, &call).exit_code, 2, "{flag} {invalid}");
        }
    }
    for flag in [
        "--block-model",
        "--value-attribute",
        "--per-unit",
        "--slope-rise",
        "--slope-run",
        "--slope-levels",
    ] {
        let call = ["plan", "pit", "--upit", "x", "--prec", "y", flag, "1"].map(str::to_owned);
        assert_eq!(run(&root, &call).exit_code, 2, "orphan {flag}");
    }
}

#[test]
fn omf_pit_library_and_binary_match_and_robot_docs_expose_the_path() {
    let root = root();
    fixture(&root, [5, 1, 3], &section(9.25), "");
    let call = args(1, 1, 2);
    let output = run(&root, &call);
    assert_eq!(output.exit_code, 0, "{}", output.to_json());
    let binary = std::env::var_os("FMINE_TEST_BINARY").map_or_else(
        || {
            let executable = std::env::current_exe().unwrap();
            let profile = executable
                .ancestors()
                .find(|path| {
                    path.file_name()
                        .is_some_and(|name| name == "deps" || name == "build")
                })
                .and_then(Path::parent)
                .expect("Cargo profile directory; set FMINE_TEST_BINARY for another layout");
            profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
        },
        PathBuf::from,
    );
    for (format, expected) in [
        ("json", output.to_json()),
        ("markdown", output.rendered_markdown.unwrap()),
    ] {
        let result = Command::new(&binary)
            .args(&call)
            .args(["--format", format, "--workspace"])
            .arg(&root)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(0), "{:?}", result.stderr);
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            format!("{expected}\n")
        );
    }
    let docs = invoke(&["robot-docs".into()], &root).0;
    for text in [
        "plan pit --omf",
        "--slope-rise",
        "--slope-levels",
        "fmine.plan-pit.omf.v1",
    ] {
        assert!(docs.payload_json.contains(text), "{text}");
    }
}
