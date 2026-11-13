//! OMF scheduling through the command surface; invented archive bytes and independent integer oracles.
#![cfg(test)]

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::{anchor::SourceAnchor, sha256};
use fmine_io::omf::json::{Json, parse};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SCALE: i128 = 1 << 40;

fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-omf-schedule-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

// Independent optional-DOUBLE Parquet encoding and Python standard ZIP/gzip writers; no production
// writer, schedule, precedence builder or solver contributes to the expected arithmetic below.
const FIXTURE: &str = r#"
import gzip, io, json, struct, sys, zipfile
def varint(n):
    out = bytearray()
    while n > 127:
        out.append((n & 127) | 128)
        n >>= 7
    out.append(n)
    return bytes(out)
def integer(n, kind=5): return kind, varint(2*n if n >= 0 else -2*n-1)
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
    return 9, head + b''.join(body for _,body in items)
def parquet(values):
    n = len(values)
    levels = b''.join(bytes([2, int(v is not None)]) for v in values)
    body = struct.pack('<I',len(levels)) + levels
    body += b''.join(struct.pack('<d',v) for v in values if v is not None)
    page = fields([(1,integer(0)),(2,integer(len(body))),(3,integer(len(body))),
                   (5,fields([(1,integer(n)),(2,integer(0)),(3,integer(3)),(4,integer(3))]))])[1] + body
    schema = sequence([fields([(4,text('schema')),(5,integer(1))]),
                       fields([(1,integer(5)),(3,integer(1)),(4,text('number'))])])
    column = fields([(2,integer(4,6)),(3,fields([
        (1,integer(5)),(2,sequence([integer(0),integer(3)])),
        (3,sequence([text('number')])),(4,integer(0)),(5,integer(n,6)),
        (6,integer(len(page),6)),(7,integer(len(page),6)),(9,integer(4,6))]))])
    group = fields([(1,sequence([column])),(2,integer(len(page),6)),(3,integer(n,6))])
    footer = fields([(1,integer(1)),(2,schema),(3,integer(n,6)),(4,sequence([group]))])[1]
    return b'PAR1' + page + footer + struct.pack('<I',len(footer)) + b'PAR1'
case = json.load(sys.stdin)
arrays, attributes = [], []
for name,values in case['attributes'].items():
    filename = name + '.parquet'
    arrays.append((filename,parquet(values)))
    attributes.append({'name':name,'units':'USD' if name == 'value' else 't',
        'location':'Primitives','data':{'type':'Number',
        'values':{'filename':filename,'item_count':len(values)}}})
project = {'name':'Invented schedule model','units':'m','coordinate_reference_system':'EPSG:32633',
    'date':'2026-01-01T00:00:00Z','elements':[{'name':'section','attributes':attributes,
    'geometry':{'type':'BlockModel','orient':{'origin':[0,0,0]},
        'grid':{'type':'Regular','size':[1,1,1],'count':case['count']}}}]}
out = io.BytesIO()
with zipfile.ZipFile(out,'w',compression=zipfile.ZIP_STORED) as z:
    for name,data in [('index.json.gz',gzip.compress(json.dumps(project).encode(),mtime=0))] + arrays:
        z.writestr(zipfile.ZipInfo(name,date_time=(1980,1,1,0,0,0)),data)
    z.comment = b'Open Mining Format 2.0'
sys.stdout.buffer.write(out.getvalue())
"#;

fn fixture(root: &Path, count: [u32; 3], attributes: &[(&str, Vec<Option<f64>>)]) -> Vec<u8> {
    let attrs = attributes
        .iter()
        .map(|(name, values)| {
            let values = values
                .iter()
                .map(|n| n.map_or_else(|| "null".into(), |n| n.to_string()))
                .collect::<Vec<_>>()
                .join(",");
            format!("\"{name}\":[{values}]")
        })
        .collect::<Vec<_>>()
        .join(",");
    let input = format!("{{\"count\":{count:?},\"attributes\":{{{attrs}}}}}");
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
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    fs::write(root.join("section.omf"), &output.stdout).unwrap();
    output.stdout
}

fn section(ore: f64, plan: Option<Vec<Option<f64>>>) -> Vec<(&'static str, Vec<Option<f64>>)> {
    let mut attrs = vec![
        (
            "value",
            (0..15)
                .map(|b| Some(if b == 2 { ore } else { -1.0 }))
                .collect(),
        ),
        ("tonnes", vec![Some(1.2); 15]),
    ];
    if let Some(plan) = plan {
        attrs.push(("plan", plan));
    }
    attrs
}

const SPEC: &str = r#"{
"schema":"fmine.omf-schedule.v1","block_model":"section","value_attribute":"value",
"value_per_unit":4,"value_rounding":"nearest_integer_half_away_from_zero",
"slope":{"rise":1,"run":1,"levels":2},"periods":3,"discount":{"rate":1,"scale":10},
"capacities":[{"attribute":"tonnes","per_unit":1,"limits_scaled":[10,10,10]}],
"search_rounds":100,"bound_steps":32
}"#;

fn args() -> Vec<String> {
    [
        "plan",
        "schedule",
        "--omf",
        "section.omf",
        "--spec",
        "schedule.json",
        "--list-schedule",
    ]
    .map(str::to_string)
    .into()
}

fn candidate_spec() -> String {
    SPEC.replace(
        "\"bound_steps\":32",
        "\"bound_steps\":32,\"candidate\":{\"attribute\":\"plan\",\"first\":1}",
    )
}

fn run(root: &Path, args: &[String]) -> Envelope {
    let (e, format) = invoke(args, root);
    assert_eq!(format, OutputFormat::Json);
    e
}

fn run_spec(root: &Path, spec: &str) -> Envelope {
    fs::write(root.join("schedule.json"), spec).unwrap();
    run(root, &args())
}

fn good(root: &Path, spec: &str) -> (Envelope, Json) {
    let e = run_spec(root, spec);
    assert_eq!(e.exit_code, 0, "{}", e.to_json());
    let json = parse(e.payload_json.as_bytes()).unwrap();
    (e, json)
}

fn field<'a>(json: &'a Json, key: &str) -> &'a Json {
    json.get(key).unwrap()
}
fn string<'a>(json: &'a Json, key: &str) -> &'a str {
    field(json, key).as_str().unwrap()
}
fn ints(json: &Json) -> Vec<i128> {
    json.as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap().parse().unwrap())
        .collect()
}
fn numerator(json: &Json) -> i128 {
    string(json, "numerator").parse().unwrap()
}
fn assignment(json: &Json) -> Vec<Option<usize>> {
    field(json, "periods")
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            if *n == Json::Null {
                None
            } else {
                Some(n.u64().unwrap() as usize)
            }
        })
        .collect()
}

#[test]
fn time_expansion_budget_is_enforced_during_slope_construction() {
    let root = root();
    // Only 512 blocks. The declared shallow one-layer cone requires all 256 upper blocks from
    // every lower block: 65,536 arcs. At 256 periods the pre-expansion arc cap is
    // floor(8,000,000 / 256) - 512 = 30,738, well before the full CSR could be built.
    fixture(
        &root,
        [16, 16, 2],
        &[
            ("value", vec![Some(1.0); 512]),
            ("tonnes", vec![Some(1.0); 512]),
        ],
    );
    let declaration = SPEC
        .replace("\"periods\":3", "\"periods\":256")
        .replace("\"rate\":1,\"scale\":10", "\"rate\":0,\"scale\":1")
        .replace("\"run\":1", "\"run\":100")
        .replace("\"levels\":2", "\"levels\":1")
        .replace("[10,10,10]", &format!("[{}]", vec!["512"; 256].join(",")));
    let refusal = run_spec(&root, &declaration);
    assert_ne!(refusal.exit_code, 0);
    assert!(
        refusal
            .to_json()
            .contains("retained arc budget 30738 exceeded"),
        "{}",
        refusal.to_json()
    );
    assert!(!refusal.payload_json.contains("\"selected\""));
}

#[test]
fn a_long_thin_column_is_admitted_by_its_retained_arcs_and_has_a_zero_gap() {
    let root = root();
    fixture(
        &root,
        [1, 1, 100],
        &[
            (
                "value",
                (0..100)
                    .map(|b| Some(if b == 0 { 101.0 } else { -1.0 }))
                    .collect(),
            ),
            ("tonnes", vec![Some(1.0); 100]),
        ],
    );
    let declaration = SPEC
        .replace("\"periods\":3", "\"periods\":100")
        .replace("\"rate\":1,\"scale\":10", "\"rate\":0,\"scale\":1")
        .replace("\"levels\":2", "\"levels\":99")
        .replace("\"search_rounds\":100", "\"search_rounds\":0")
        .replace("\"bound_steps\":32", "\"bound_steps\":1")
        .replace("[10,10,10]", &format!("[{}]", vec!["100"; 100].join(",")));
    // The former rectangular admission counted (100 * 3 * 3 * 99 + 100) * 100 = 8,920,000
    // expanded entries and refused. A column needs only 99 adjacent-layer arcs; the same bound
    // on the retained graph is (99 + 100) * 100 = 19,900. Mining every block earns 101 - 99 = 2.
    let (_, json) = good(&root, &declaration);
    assert_eq!(field(field(&json, "model"), "arcs").u64(), Some(99));
    let selected = field(&json, "selected");
    assert_eq!(
        field(field(selected, "checked"), "mined_blocks").u64(),
        Some(100)
    );
    assert!(assignment(selected).iter().all(Option::is_some));
    assert_eq!(numerator(field(selected, "discounted_value")), 8 * SCALE);
    let certificate = field(&json, "certificate");
    assert_eq!(numerator(field(certificate, "upper_bound")), 8 * SCALE);
    assert_eq!(numerator(field(certificate, "gap")), 0);
}

#[test]
fn archive_to_schedule_preserves_order_exact_values_capacities_and_source_changes() {
    let root = root();
    let bytes = fixture(&root, [5, 1, 3], &section(9.25, None));
    let (e, json) = good(&root, SPEC);
    let selected = field(&json, "selected");
    let check = field(selected, "checked");
    let f1 = SCALE * 10 / 11;
    assert_eq!(
        numerator(field(selected, "discounted_value")),
        -16 * SCALE + 21 * f1
    );
    assert_eq!(
        string(field(selected, "discounted_value"), "denominator"),
        (4 * SCALE).to_string()
    );
    assert_eq!(ints(field(check, "period_values")), [-16, 21, 0]);
    assert_eq!(
        ints(&field(check, "resource_used").as_array().unwrap()[0]),
        [8, 10, 0]
    );
    assert_eq!(field(check, "feasible"), &Json::Bool(true));
    let p = assignment(selected);
    for b in [2, 6, 7, 8] {
        assert_eq!(p[b], Some(1), "block {b}");
    }
    for b in [0, 1, 3, 4, 5, 9] {
        assert_eq!(p[b], None, "block {b}");
    }
    assert_eq!((10..15).filter(|&b| p[b] == Some(0)).count(), 4);
    assert_eq!(
        string(&json, "block_order"),
        "omf_u_then_v_then_w_bottom_up"
    );
    assert!(e.to_json().contains(&sha256(&bytes).to_hex()));
    assert_eq!(
        string(field(field(&json, "sources"), "spec"), "sha256"),
        sha256(SPEC.as_bytes()).to_hex()
    );
    assert_eq!(run(&root, &args()).to_json(), e.to_json());
    for no_claim in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(e.to_json().contains(&format!("\"{no_claim}\":false")));
    }
    assert!(
        e.rendered_markdown
            .as_ref()
            .unwrap()
            .contains("| tonnes | t | 1 | [\"8\",\"10\",\"0\"] | [\"10\",\"10\",\"10\"] |")
    );
    assert!(e.no_claim.unwrap().contains("heuristic"));
    fixture(&root, [5, 1, 3], &section(10.25, None));
    let (_, changed) = good(&root, SPEC);
    assert_eq!(
        numerator(field(field(&changed, "selected"), "discounted_value")),
        -16 * SCALE + 25 * f1
    );
    assert_ne!(
        field(field(&changed, "sources"), "omf"),
        field(field(&json, "sources"), "omf")
    );
    let different_work = SPEC.replace("\"search_rounds\":100", "\"search_rounds\":99");
    let (_, work) = good(&root, &different_work);
    assert_ne!(
        field(field(&work, "sources"), "spec"),
        field(field(&changed, "sources"), "spec")
    );
    assert_eq!(
        field(field(&work, "sources"), "omf"),
        field(field(&changed, "sources"), "omf")
    );
}

#[test]
fn imported_schedule_is_checked_then_improved_without_losing_the_original() {
    let root = root();
    let plan = (0..15)
        .map(|b| match b {
            10..=14 => Some(1.0),
            2 | 6..=8 => Some(2.0),
            _ => None,
        })
        .collect();
    fixture(&root, [5, 1, 3], &section(9.25, Some(plan)));
    let (_, json) = good(&root, &candidate_spec());
    let candidate = field(&json, "candidate");
    assert_eq!(
        field(field(candidate, "checked"), "feasible"),
        &Json::Bool(true)
    );
    assert_eq!(
        numerator(field(candidate, "discounted_value")),
        -20 * SCALE + 25 * (SCALE * 10 / 11)
    );
    assert_eq!(
        numerator(field(candidate, "improved_value")),
        -16 * SCALE + 21 * (SCALE * 10 / 11)
    );
    // The worksheet path replaces the independently generated baseline only on a strict improvement.
    assert_eq!(string(field(&json, "selected"), "origin"), "pit_ranked");
    assert_eq!(
        field(field(&json, "selected"), "discounted_value"),
        field(candidate, "improved_value")
    );
    assert_eq!(assignment(candidate)[10], Some(0));
    assert_eq!(field(candidate, "first").i64(), Some(1));
    assert_eq!(
        field(candidate, "improved_periods")
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p.u64() == Some(0))
            .count(),
        4
    );
}

#[test]
fn infeasible_candidates_keep_precedence_or_capacity_witnesses_beside_the_independent_plan() {
    let root = root();
    for whole_cone in [false, true] {
        let plan = (0..15)
            .map(|b| (b == 2 || (whole_cone && matches!(b, 6..=8 | 10..=14))).then_some(1.0))
            .collect();
        fixture(&root, [5, 1, 3], &section(9.25, Some(plan)));
        let (_, json) = good(&root, &candidate_spec());
        let candidate = field(&json, "candidate");
        let check = field(candidate, "checked");
        assert_eq!(field(check, "feasible"), &Json::Bool(false));
        assert_eq!(field(candidate, "improved_value"), &Json::Null);
        assert_eq!(field(candidate, "improved_periods"), &Json::Null);
        assert_eq!(string(field(&json, "selected"), "origin"), "pit_ranked");
        assert_eq!(
            field(field(field(&json, "selected"), "checked"), "feasible"),
            &Json::Bool(true)
        );
        if whole_cone {
            assert_eq!(field(check, "unmet_requirements").u64(), Some(0));
            assert_eq!(field(check, "over_limit"), &parse(b"[[0,0]]").unwrap());
            assert_eq!(
                ints(&field(check, "resource_used").as_array().unwrap()[0]),
                [18, 0, 0]
            );
        } else {
            assert_eq!(field(check, "unmet_requirements").u64(), Some(3));
            assert_eq!(
                field(check, "first_unmet"),
                &parse(b"[[2,6],[2,7],[2,8]]").unwrap()
            );
        }
        assert_eq!(assignment(candidate)[2], Some(0));
    }
}

// The 2x1x2 model's two lower blocks require both upper blocks under the stated 45-degree cone.
// Enumerate every assignment, not the production CSR/checker. Also replay the price witness by
// enumerating the capacity-relaxed problem in the same exact integer discount units.
fn independent_optima(prices: &[Vec<i128>]) -> (i128, i128) {
    let weights = [24, 20, -8, -8];
    let uses = [[3, 4, 0, 0], [2, 2, 2, 2]];
    let limits = [[0, 4, 4], [4, 4, 4]];
    let factors = [SCALE, SCALE * 10 / 11, (SCALE * 10 / 11) * 10 / 11];
    let (mut best, mut priced) = (0, i128::MIN);
    for mut code in 0..256 {
        let mut p = [None; 4];
        for t in &mut p {
            let n = code % 4;
            code /= 4;
            *t = (n < 3).then_some(n);
        }
        if (0..2).any(|b| p[b].is_some_and(|t| (2..4).any(|q| p[q].is_none_or(|u| u > t)))) {
            continue;
        }
        let mut value = 0;
        let mut used = [[0_i128; 3]; 2];
        for (b, t) in p.iter().enumerate() {
            if let Some(t) = t {
                value += weights[b] * factors[*t];
                for r in 0..2 {
                    used[r][*t] += uses[r][b];
                }
            }
        }
        if (0..2).all(|r| (0..3).all(|t| used[r][t] <= limits[r][t])) {
            best = best.max(value);
        }
        let adjusted = value
            + (0..2)
                .map(|r| {
                    (0..3)
                        .map(|t| prices[r][t] * (limits[r][t] - used[r][t]))
                        .sum::<i128>()
                })
                .sum::<i128>();
        priced = priced.max(adjusted);
    }
    (best, priced)
}

#[test]
fn multiple_resources_and_the_full_model_bound_match_independent_enumeration() {
    let root = root();
    fixture(
        &root,
        [2, 1, 2],
        &[
            ("value", [6.0, 5.0, -2.0, -2.0].map(Some).into()),
            ("tonnes", vec![Some(1.2); 4]),
            ("mill", [3.0, 4.0, 0.0, 0.0].map(Some).into()),
        ],
    );
    let declaration = SPEC.replace(
        "\"capacities\":[{\"attribute\":\"tonnes\",\"per_unit\":1,\"limits_scaled\":[10,10,10]}]",
        "\"capacities\":[{\"attribute\":\"mill\",\"per_unit\":1,\"limits_scaled\":[0,4,4]},{\"attribute\":\"tonnes\",\"per_unit\":1,\"limits_scaled\":[4,4,4]}]",
    );
    let (_, json) = good(&root, &declaration);
    let bound = field(&json, "certificate");
    let prices = field(bound, "prices")
        .as_array()
        .unwrap()
        .iter()
        .map(ints)
        .collect::<Vec<_>>();
    let (best, replayed) = independent_optima(&prices);
    assert_eq!(
        numerator(field(field(&json, "selected"), "discounted_value")),
        best
    );
    assert_eq!(numerator(field(bound, "upper_bound")), replayed);
    assert!(replayed >= best);
    assert_eq!(numerator(field(bound, "gap")), replayed - best);
    assert_eq!(field(bound, "weight_shift").u64(), Some(0));
    // Milling separates the two ore blocks, while the tonnes row admits one waste block beside
    // the first ore block. Delaying that -8 waste from period 0 to 1 gains 8 * (SCALE - F1).
    // Independent enumeration gives two optima differing only in which identical waste goes first.
    let f1 = SCALE * 10 / 11;
    let f2 = f1 * 10 / 11;
    assert_eq!(best, -8 * SCALE + 16 * f1 + 20 * f2);
    assert_eq!(
        assignment(field(&json, "selected")),
        [Some(1), Some(2), Some(0), Some(1)]
    );
    let checked = field(field(&json, "selected"), "checked");
    assert_eq!(ints(field(checked, "period_values")), [-8, 16, 20]);
    assert_eq!(
        field(checked, "resource_used")
            .as_array()
            .unwrap()
            .iter()
            .map(ints)
            .collect::<Vec<_>>(),
        [vec![0, 3, 4], vec![2, 4, 2]]
    );
    let resources = field(&json, "capacities").as_array().unwrap();
    assert_eq!(string(&resources[0], "attribute"), "mill");
    assert_eq!(string(&resources[1], "attribute"), "tonnes");
}

#[test]
fn an_empty_schedule_beats_a_negative_discounted_incumbent() {
    let root = root();
    let plan = (0..15)
        .map(|b| match b {
            10..=14 => Some(1.0),
            2 | 6..=8 => Some(2.0),
            _ => None,
        })
        .collect();
    fixture(&root, [5, 1, 3], &section(8.25, Some(plan)));
    let (_, json) = good(&root, &candidate_spec());
    assert_eq!(string(field(&json, "selected"), "origin"), "empty");
    assert_eq!(
        numerator(field(field(&json, "selected"), "discounted_value")),
        0
    );
    assert!(
        assignment(field(&json, "selected"))
            .iter()
            .all(Option::is_none)
    );
    let candidate = field(&json, "candidate");
    assert_eq!(
        numerator(field(candidate, "discounted_value")),
        -20 * SCALE + 21 * (SCALE * 10 / 11)
    );
    assert_eq!(
        numerator(field(candidate, "improved_value")),
        -16 * SCALE + 17 * (SCALE * 10 / 11)
    );
    assert!(numerator(field(candidate, "improved_value")) < 0);
}

#[test]
fn malformed_requests_sources_and_candidate_periods_fail_without_a_numeric_result() {
    let root = root();
    let bytes = fixture(&root, [5, 1, 3], &section(9.25, None));
    for (old, new) in [
        ("\"value_per_unit\":4", "\"value_per_unit\":0"),
        ("\"periods\":3", "\"periods\":0"),
        ("\"periods\":3", "\"periods\":257"),
        ("\"rate\":1", "\"rate\":-1"),
        ("\"scale\":10", "\"scale\":0"),
        ("\"search_rounds\":100", "\"search_rounds\":1001"),
        ("\"bound_steps\":32", "\"bound_steps\":0"),
        ("\"bound_steps\":32", "\"bound_steps\":257"),
        (
            "\"value_attribute\":\"value\"",
            "\"value_attribute\":\"missing\"",
        ),
        ("\"per_unit\":1", "\"per_unit\":0"),
        ("nearest_integer_half_away_from_zero", "undeclared_rounding"),
    ] {
        assert!(SPEC.contains(old));
        let e = run_spec(&root, &SPEC.replace(old, new));
        assert_ne!(e.exit_code, 0, "{}", e.to_json());
        assert!(!e.payload_json.contains("\"selected\""));
    }
    for declaration in [
        SPEC.replace("\"periods\":3", "\"periods\":3,\"periods\":2"),
        SPEC.replace("\"slope\":{", "\"slope\":{\"unknown\":1,"),
        SPEC.replace(
            "\"bound_steps\":32",
            "\"bound_steps\":32,\"candidate\":{\"attribute\":\"plan\"}",
        ),
        SPEC.replace("[10,10,10]", "[1,2]"),
        SPEC.replace("[10,10,10]", "[1.1,2,3]"),
        SPEC.replace("[10,10,10]", "[-1,2,3]"),
    ] {
        let e = run_spec(&root, &declaration);
        assert_ne!(e.exit_code, 0, "{}", e.to_json());
        assert!(!e.payload_json.contains("\"selected\""));
    }
    fs::write(root.join("schedule.json"), SPEC).unwrap();
    for extra in [
        vec!["--round"],
        vec!["--pcpsp", "x"],
        vec!["--candidate-attribute", "plan"],
        vec!["--list-schedule"],
        vec!["--omf", "x"],
        vec!["--spec", "x"],
    ] {
        let mut a = args();
        a.extend(extra.into_iter().map(str::to_string));
        assert_eq!(run(&root, &a).exit_code, 2);
    }
    for bad in [0.0, 5.0, 1.5] {
        let mut plan = vec![None; 15];
        plan[2] = Some(bad);
        fixture(&root, [5, 1, 3], &section(9.25, Some(plan)));
        let e = run_spec(&root, &candidate_spec());
        assert_ne!(e.exit_code, 0, "{}", e.to_json());
        assert!(!e.payload_json.contains("\"selected\""));
    }
    let forged = SourceAnchor::new("section.omf", [0; 32], "declared OMF source");
    let spec_source = SourceAnchor::new(
        "schedule.json",
        sha256(SPEC.as_bytes()).0,
        "declared schedule",
    );
    assert_eq!(
        fmine_io::omf::scheduling::evaluate(&bytes, &forged, SPEC, &spec_source, true)
            .err()
            .unwrap()
            .code,
        "FMINE-REF-505"
    );
    let mut a = args();
    a.retain(|s| s != "--spec" && s != "schedule.json");
    assert_eq!(run(&root, &a).exit_code, 2);
    let mut a = args();
    let at = a.iter().position(|s| s == "--omf").unwrap() + 1;
    a[at] = "absent.omf".into();
    assert_ne!(run(&root, &a).exit_code, 0);
}

const SECTION_MPS: &str = include_str!("../../fmine-io/testdata/omf_section_schedule.mps");
const HIGHS_COLUMNS: &str =
    include_str!("../../../fixtures/fmine-io/omf_section_schedule_highs_columns.txt");
const HIGHS_ROWS: &str =
    include_str!("../../../fixtures/fmine-io/omf_section_schedule_highs_rows.txt");

fn export_lp(root: &Path, spec: &str) -> (Envelope, Json) {
    fs::write(root.join("schedule.json"), spec).unwrap();
    let mut a = args();
    a.push("--export-lp".into());
    let e = run(root, &a);
    assert_eq!(e.exit_code, 0, "{}", e.to_json());
    let json = parse(e.payload_json.as_bytes()).unwrap();
    (e, json)
}

fn import_lp(root: &Path, digest: &str, primal: Option<&str>, dual: Option<&str>) -> Envelope {
    let mut a = args();
    a.extend(["--lp-sha256".into(), digest.into()]);
    for (flag, path, text) in [
        ("--lp-primal", "primal.txt", primal),
        ("--lp-dual", "dual.txt", dual),
    ] {
        if let Some(text) = text {
            fs::write(root.join(path), text).unwrap();
            a.extend([flag.into(), path.into()]);
        }
    }
    run(root, &a)
}

fn one_period_spec() -> String {
    SPEC.replace("\"periods\":3", "\"periods\":1")
        .replace("\"value_per_unit\":4", "\"value_per_unit\":1")
        .replace("[10,10,10]", "[4]")
        .replace("\"search_rounds\":100", "\"search_rounds\":0")
}

#[test]
fn external_lp_export_matches_the_pinned_relaxation_and_binds_both_source_snapshots() {
    let root = root();
    let bytes = fixture(&root, [5, 1, 3], &section(9.25, None));
    let (e, exported) = export_lp(&root, SPEC);
    let mps = string(&exported, "mps");
    let digest = string(&exported, "mps_sha256");
    let expected = format!(
        "* fmine.omf-schedule-lp.v1\n* omf_sha256={}\n* spec_sha256={}\n{}",
        sha256(&bytes),
        sha256(SPEC.as_bytes()),
        SECTION_MPS.replacen("NAME section", "NAME fmine_schedule", 1),
    );
    assert_eq!(mps, expected);
    assert_eq!(digest, sha256(mps.as_bytes()).to_hex());
    assert_eq!(field(&exported, "columns").u64(), Some(45));
    assert_eq!(field(&exported, "rows").u64(), Some(111));
    let parsed = fmine_lp::mps::read(mps).unwrap();
    assert_eq!(
        fmine_lp::mps::write("fmine_schedule", &parsed.problem).unwrap(),
        SECTION_MPS.replacen("NAME section", "NAME fmine_schedule", 1)
    );
    assert!(exported.get("selected").is_none());
    assert!(exported.get("certificate").is_none());
    assert_eq!(export_lp(&root, SPEC).0.to_json(), e.to_json());
    assert!(e.rendered_markdown.as_ref().unwrap().contains(mps));

    // A search-only or whitespace edit leaves the coefficient matrix unchanged but changes the
    // pinned request. The caller cannot apply an old solver result to a different source snapshot.
    for spec in [
        format!("{SPEC}\n"),
        SPEC.replace("\"bound_steps\":32", "\"bound_steps\":31"),
    ] {
        let (_, changed) = export_lp(&root, &spec);
        assert_ne!(string(&changed, "mps_sha256"), digest);
        assert_eq!(
            string(&changed, "mps").split_once("NAME ").unwrap().1,
            mps.split_once("NAME ").unwrap().1
        );
        let stale = import_lp(&root, digest, Some(HIGHS_COLUMNS), Some(HIGHS_ROWS));
        assert!(
            stale.to_json().contains("FMINE-REF-505"),
            "{}",
            stale.to_json()
        );
        assert!(!stale.payload_json.contains("\"selected\""));
    }
    fs::write(root.join("schedule.json"), SPEC).unwrap();
    fixture(&root, [5, 1, 3], &section(10.25, None));
    let stale = import_lp(&root, digest, Some(HIGHS_COLUMNS), None);
    assert!(stale.to_json().contains("FMINE-REF-505"));
}

#[test]
fn pinned_highs_values_keep_the_binary64_bound_separate_from_the_exact_schedule_gap() {
    let root = root();
    fixture(&root, [5, 1, 3], &section(9.25, None));
    let (_, baseline) = good(&root, SPEC);
    let (_, exported) = export_lp(&root, SPEC);
    let e = import_lp(
        &root,
        string(&exported, "mps_sha256"),
        Some(HIGHS_COLUMNS),
        Some(HIGHS_ROWS),
    );
    assert_eq!(e.exit_code, 0, "{}", e.to_json());
    let json = parse(e.payload_json.as_bytes()).unwrap();
    assert_eq!(field(&json, "selected"), field(&baseline, "selected"));
    assert_eq!(field(&json, "certificate"), field(&baseline, "certificate"));
    assert_eq!(field(&json, "external_lp_selected"), &Json::Bool(false));
    let external = field(&json, "external_lp");
    let primal = field(external, "primal");
    assert_eq!(field(primal, "searched_starts").u64(), Some(6));
    assert_eq!(
        ints(field(field(primal, "checked"), "period_values")),
        [-16, 21, 0]
    );
    let exact = field(primal, "integer_discounted_value");
    assert_eq!(numerator(exact), 3_398_490_485_834);
    assert_eq!(string(exact, "denominator"), "4398046511104");
    let dual = field(external, "dual");
    assert_eq!(string(dual, "status"), "certified");
    let bound = field(dual, "upper_bound_scaled").f64().unwrap();
    assert!((bound - 475.0 / 99.0).abs() < 1e-6, "{bound}");
    assert_eq!(
        field(field(external, "objective"), "value_per_unit").u64(),
        Some(4)
    );
    assert_eq!(
        field(
            field(external, "objective"),
            "used_for_integer_schedule_gap"
        ),
        &Json::Bool(false)
    );
    assert_eq!(
        string(field(primal, "source"), "sha256"),
        sha256(HIGHS_COLUMNS.as_bytes()).to_hex()
    );
    assert_eq!(
        string(field(dual, "source"), "sha256"),
        sha256(HIGHS_ROWS.as_bytes()).to_hex()
    );
    let markdown = e.rendered_markdown.as_ref().unwrap();
    assert!(markdown.contains("scaled value units"));
    assert!(markdown.contains("never used to tighten the integer schedule certificate"));

    // An arbitrary row-sign convention may weaken the LP bound. It cannot alter or tighten the
    // separate integer certificate. Nonmonotone, row-infeasible shares remain advisory as well.
    let wrong_sign = HIGHS_ROWS
        .lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            match (fields.next(), fields.next()) {
                (Some(name), Some(value)) if name.starts_with('R') => {
                    format!("{name} {}\n", -value.parse::<f64>().unwrap())
                }
                _ => format!("{line}\n"),
            }
        })
        .collect::<String>();
    for value in [0.0, 1.0] {
        let shares = (0..45)
            .map(|i| format!("C{i} {value}\n"))
            .collect::<String>();
        let e = import_lp(
            &root,
            string(&exported, "mps_sha256"),
            Some(&shares),
            Some(&wrong_sign),
        );
        assert_eq!(e.exit_code, 0, "{}", e.to_json());
        let json = parse(e.payload_json.as_bytes()).unwrap();
        assert_eq!(field(&json, "selected"), field(&baseline, "selected"));
        assert_eq!(field(&json, "certificate"), field(&baseline, "certificate"));
    }
}

#[test]
fn external_primal_can_improve_the_exact_incumbent_and_keep_negative_blend_helpers() {
    let root = root();
    fixture(
        &root,
        [3, 1, 1],
        &[
            ("value", vec![Some(6.0), Some(5.0), Some(5.0)]),
            ("tonnes", vec![Some(3.0), Some(2.0), Some(2.0)]),
        ],
    );
    let spec = one_period_spec();
    let (_, baseline) = good(&root, &spec);
    assert_eq!(
        numerator(field(field(&baseline, "selected"), "discounted_value")),
        6 * SCALE
    );
    let (_, exported) = export_lp(&root, &spec);
    let e = import_lp(
        &root,
        string(&exported, "mps_sha256"),
        Some("C0 0\nC1 1\nC2 1\n"),
        Some("R0 -2.5\n"),
    );
    assert_eq!(e.exit_code, 0, "{}", e.to_json());
    let json = parse(e.payload_json.as_bytes()).unwrap();
    assert_eq!(string(field(&json, "selected"), "origin"), "external_lp");
    assert_eq!(
        assignment(field(&json, "selected")),
        [None, Some(0), Some(0)]
    );
    assert_eq!(
        numerator(field(field(&json, "selected"), "discounted_value")),
        10 * SCALE
    );
    assert_eq!(field(&json, "external_lp_selected"), &Json::Bool(true));
    let certificate = field(&json, "certificate");
    assert!(numerator(field(certificate, "upper_bound")) >= 10 * SCALE);
    assert_eq!(
        numerator(field(certificate, "gap")),
        numerator(field(certificate, "upper_bound")) - 10 * SCALE
    );

    // The -1 helper is outside the positive-value ultimate pit but supplies the signed blend
    // credit required by the +5 block. Export and imported guidance must use the full model.
    fixture(
        &root,
        [2, 1, 1],
        &[
            ("value", vec![Some(-1.0), Some(5.0)]),
            ("blend", vec![Some(-1.0), Some(1.0)]),
        ],
    );
    let spec = spec.replace("tonnes", "blend").replace("[4]", "[0]");
    let (_, baseline) = good(&root, &spec);
    assert_eq!(
        numerator(field(field(&baseline, "selected"), "discounted_value")),
        0
    );
    let (_, exported) = export_lp(&root, &spec);
    assert_eq!(field(&exported, "columns").u64(), Some(2));
    let e = import_lp(
        &root,
        string(&exported, "mps_sha256"),
        Some("C0 1\nC1 1\n"),
        Some("R0 -1\n"),
    );
    assert_eq!(e.exit_code, 0, "{}", e.to_json());
    let json = parse(e.payload_json.as_bytes()).unwrap();
    assert_eq!(assignment(field(&json, "selected")), [Some(0), Some(0)]);
    assert_eq!(
        numerator(field(field(&json, "selected"), "discounted_value")),
        4 * SCALE
    );
    assert_eq!(
        field(
            field(field(&json, "external_lp"), "dual"),
            "upper_bound_scaled"
        )
        .f64(),
        Some(4.0)
    );
}

#[test]
fn boxed_zero_row_lps_accept_empty_duals_and_negative_models_keep_the_empty_incumbent() {
    let root = root();
    let spec = one_period_spec().replace(
        "[{\"attribute\":\"tonnes\",\"per_unit\":1,\"limits_scaled\":[4]}]",
        "[]",
    );
    for (value, expected) in [(5.0, 5), (-5.0, 0)] {
        fixture(&root, [1, 1, 1], &[("value", vec![Some(value)])]);
        let (_, exported) = export_lp(&root, &spec);
        assert_eq!(field(&exported, "columns").u64(), Some(1));
        assert_eq!(field(&exported, "rows").u64(), Some(0));
        let e = import_lp(
            &root,
            string(&exported, "mps_sha256"),
            None,
            Some("# no rows\n"),
        );
        assert_eq!(e.exit_code, 0, "{}", e.to_json());
        let json = parse(e.payload_json.as_bytes()).unwrap();
        assert_eq!(
            numerator(field(field(&json, "selected"), "discounted_value")),
            expected * SCALE
        );
        assert_eq!(field(field(&json, "external_lp"), "primal"), &Json::Null);
        assert_eq!(
            field(
                field(field(&json, "external_lp"), "dual"),
                "upper_bound_scaled"
            )
            .f64(),
            Some(expected as f64)
        );
    }
}

#[test]
fn malformed_or_unbound_external_lp_values_refuse_before_any_schedule_is_admitted() {
    let root = root();
    let bytes = fixture(&root, [5, 1, 3], &section(9.25, None));
    let (_, exported) = export_lp(&root, SPEC);
    let digest = string(&exported, "mps_sha256");
    let zeroes = (0..45).map(|i| format!("C{i} 0\n")).collect::<String>();
    for bad in [
        zeroes.replacen("C0 0\n", "", 1),
        format!("{zeroes}C0 0\n"),
        zeroes.replacen("C0", "C00", 1),
        zeroes.replacen("C0", "R0", 1),
        zeroes.replacen("C0", "C45", 1),
        zeroes.replacen("C0 0", "C0 NaN", 1),
        zeroes.replacen("C0 0", "C0 inf", 1),
        zeroes.replacen("C0 0", "C0 -0.000001", 1),
        zeroes.replacen("C0 0", "C0 1.000001", 1),
        zeroes.replacen("C0 0", "C0 0 ignored", 1),
    ] {
        let e = import_lp(&root, digest, Some(&bad), None);
        assert_ne!(e.exit_code, 0, "{}", e.to_json());
        assert!(!e.payload_json.contains("\"selected\""));
    }
    for bad in ["R0 0\n", "C0 0\n", "R0 NaN\n"] {
        let e = import_lp(&root, digest, None, Some(bad));
        assert_ne!(e.exit_code, 0, "{}", e.to_json());
    }
    for extra in [
        vec![
            "--export-lp",
            "--lp-sha256",
            digest,
            "--lp-primal",
            "primal.txt",
        ],
        vec!["--lp-sha256", digest],
        vec!["--lp-primal", "primal.txt"],
        vec!["--export-lp", "--export-lp"],
    ] {
        let mut a = args();
        a.extend(extra.into_iter().map(str::to_string));
        assert_eq!(run(&root, &a).exit_code, 2);
    }
    let model_source = SourceAnchor::new("section.omf", sha256(&bytes).0, "model");
    let spec_source = SourceAnchor::new("schedule.json", sha256(SPEC.as_bytes()).0, "spec");
    let forged = SourceAnchor::new("primal.txt", [0; 32], "unbound primal");
    let refusal = fmine_io::omf::scheduling::evaluate_with_lp(
        &bytes,
        &model_source,
        SPEC,
        &spec_source,
        true,
        Some(fmine_io::omf::scheduling::external_lp::Request::Import {
            mps_sha256: digest,
            primal: Some((&zeroes, &forged)),
            dual: None,
        }),
    )
    .unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-505");
}

#[test]
fn external_lp_text_budget_refuses_before_relaxation_allocation() {
    let root = root();
    fixture(&root, [64, 64, 1], &[("value", vec![Some(1.0); 4096])]);
    let spec = SPEC.replace("\"periods\":3", "\"periods\":32").replace(
        "[{\"attribute\":\"tonnes\",\"per_unit\":1,\"limits_scaled\":[10,10,10]}]",
        "[]",
    );
    fs::write(root.join("schedule.json"), spec).unwrap();
    let mut a = args();
    a.push("--export-lp".into());
    let e = run(&root, &a);
    assert!(e.to_json().contains("FMINE-REF-207"), "{}", e.to_json());
    assert!(e.to_json().contains("MPS text budget"));
    assert!(!e.payload_json.contains("\"mps\""));
}

#[test]
fn external_lp_commands_match_the_actual_binary_json_and_markdown() {
    let executable = std::env::var_os("FMINE_TEST_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let current = std::env::current_exe().unwrap();
            let profile = current
                .ancestors()
                .find(|path| {
                    path.file_name()
                        .is_some_and(|name| name == "deps" || name == "build")
                })
                .and_then(Path::parent)
                .unwrap();
            profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
        });
    let root = root();
    fixture(&root, [5, 1, 3], &section(9.25, None));
    let (_, exported) = export_lp(&root, SPEC);
    let digest = string(&exported, "mps_sha256");
    fs::write(root.join("primal.txt"), HIGHS_COLUMNS).unwrap();
    fs::write(root.join("dual.txt"), HIGHS_ROWS).unwrap();
    for extra in [
        vec!["--export-lp"],
        vec![
            "--lp-sha256",
            digest,
            "--lp-primal",
            "primal.txt",
            "--lp-dual",
            "dual.txt",
        ],
    ] {
        let mut a = args();
        a.extend(extra.into_iter().map(str::to_string));
        let expected = run(&root, &a);
        assert_eq!(expected.exit_code, 0);
        for format in ["json", "markdown"] {
            let output = Command::new(&executable)
                .args(&a)
                .arg("--workspace")
                .arg(&root)
                .args(["--format", format])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            let text = String::from_utf8(output.stdout).unwrap();
            let expected = if format == "json" {
                expected.to_json()
            } else {
                expected.rendered_markdown.as_ref().unwrap().clone()
            };
            assert_eq!(text.trim_end(), expected.trim_end());
        }
    }
}
