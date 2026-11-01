//! DESIGN (BD-344): check supplied ancestry at each forecast's registration, never at its later scoring date.
//! This adapter checks metadata; it does not independently establish times or rerun a forecast generator.

use std::collections::BTreeMap;
use std::path::Path;

use fmine_claim::dag::{Availability, ConeCut, ConeError, NodeKind, TemporalGraph, TemporalNode};
use fmine_core::hash::sha256;
use fmine_core::id::ObjectId;
use fmine_time::{Anchor, RecordTime, TimeEvidence};

use super::{
    Assessment, AssessmentFailure, Refusal, RefusalCode, Replay, Row, invalid, js, parse_subset,
    utc,
};

pub(super) const NOT_CHECKED: &str = "{\"status\":\"not_checked\",\"reason\":\"No provenance bundle was supplied\",\"generator_replay\":\"not_performed\"}";
const SCHEMA: &str = "fmine.forecast-provenance.v1";

struct Binding {
    forecast_id: ObjectId,
    anchor: String,
}

struct Bundle {
    lane: String,
    graph: TemporalGraph,
    anchors: BTreeMap<String, Anchor>,
    bindings: BTreeMap<String, Binding>,
}

impl Bundle {
    fn parse(text: &str) -> Result<Self, Refusal> {
        let doc = parse_subset(text).map_err(|e| invalid(e.to_string()))?;
        let top = |key: &str| match doc.top_get(key).map(|f| &f.value) {
            Some(super::TomlValue::Str(s)) => Some(s.as_str()),
            _ => None,
        };
        if doc.top.len() != 2 || top("schema") != Some(SCHEMA) {
            return Err(invalid(format!(
                "provenance requires schema = {SCHEMA:?} and lane"
            )));
        }
        let lane = top("lane")
            .filter(|s| matches!(*s, "synthetic" | "supplied"))
            .ok_or_else(|| invalid("provenance lane must be synthetic or supplied"))?
            .to_string();
        let mut nodes = Vec::new();
        let mut marks: BTreeMap<String, Vec<(String, u64)>> = BTreeMap::new();
        let mut bindings = BTreeMap::new();
        for entry in &doc.rows {
            let row = Row(entry);
            let result = match entry.table.as_str() {
                "node" => node(&row).map(|node| nodes.push(node)),
                "anchor" => (|| {
                    row.allowed(&["id", "cell", "seq"], &[])?;
                    let id = super::identifier(row.string("id")?)?;
                    let cell = row.string("cell")?;
                    marks
                        .entry(id.to_string())
                        .or_default()
                        .push((cell.to_string(), sequence(&row)?));
                    Ok(())
                })(),
                "binding" => (|| {
                    row.allowed(&["forecast", "forecast_id", "anchor"], &[])?;
                    let alias = super::identifier(row.string("forecast")?)?;
                    let binding = Binding {
                        forecast_id: object_id(row.string("forecast_id")?)?,
                        anchor: super::identifier(row.string("anchor")?)?.to_string(),
                    };
                    if bindings.insert(alias.to_string(), binding).is_some() {
                        return Err(invalid(format!(
                            "duplicate provenance binding for {alias:?}"
                        )));
                    }
                    Ok(())
                })(),
                _ => Err(invalid(format!(
                    "unknown provenance table {:?}",
                    entry.table
                ))),
            };
            result.map_err(|mut e| {
                e.what = format!("line {} [[{}]]: {}", entry.line, entry.table, e.what);
                e
            })?;
        }
        let anchors = marks
            .into_iter()
            .map(|(id, marks)| {
                Anchor::new(marks)
                    .map(|anchor| (id, anchor))
                    .map_err(|e| invalid(e.to_string()))
            })
            .collect::<Result<_, _>>()?;
        let graph = TemporalGraph::new(nodes).map_err(|e| e.refusal())?;
        Ok(Self {
            lane,
            graph,
            anchors,
            bindings,
        })
    }

    fn verify(
        &self,
        replay: &Replay,
        report: &Assessment,
        input_sha: &str,
    ) -> Result<String, AssessmentFailure> {
        if self.lane != replay.lane {
            return Err(invalid("forecast and provenance lanes must agree").into());
        }
        // Reject a stale or mistyped binding even when it names another cohort in this transcript.
        for (alias, binding) in &self.bindings {
            if replay.forecasts.get(alias) != Some(&binding.forecast_id) {
                return Err(temporal(format!(
                    "provenance binding {alias:?} does not name its frozen forecast ID"
                ))
                .into());
            }
        }
        let mut checked = Vec::new();
        for member in &report.per_member {
            let record = replay
                .ledger
                .forecast(member.forecast_id)
                .ok_or_else(|| invalid("assessment forecast missing"))?;
            let alias = replay
                .aliases
                .get(&member.forecast_id)
                .ok_or_else(|| invalid("assessment alias missing"))?;
            let binding = self.bindings.get(alias).ok_or_else(|| {
                temporal(format!(
                    "forecast {alias:?} has no provenance binding; every cohort member is required"
                ))
            })?;
            let anchor = self.anchors.get(&binding.anchor).ok_or_else(|| {
                temporal(format!(
                    "forecast {alias:?} names absent anchor {:?}",
                    binding.anchor
                ))
            })?;
            let root = member.forecast_id;
            if let Some(node) = self.graph.node(&root)
                && (node.kind != NodeKind::Forecast
                    || node.parents.is_empty()
                    || !matches!(node.availability, Availability::Derived { performed_at: Some(t), .. } if t == record.registered_ns))
            {
                return Err(temporal(format!("forecast {alias:?}: root {root} must be a derived Forecast with declared input parents and performed_at equal to registered_at")).into());
            }
            let cut = ConeCut {
                decision_ns: record.registered_ns,
                anchor: anchor.clone(),
            };
            let cone = self
                .graph
                .check(root, &cut)
                .map_err(|e| failure(alias, root, record.registered_ns, e))?;
            let nodes = cone
                .nodes()
                .iter()
                .map(|id| js(&id.to_string()))
                .collect::<Vec<_>>()
                .join(",");
            checked.push(format!(
                "{{\"forecast\":{},\"forecast_id\":{},\"decision_cut\":{},\"anchor\":{},\"cone_digest\":{},\"nodes\":[{}]}}",
                js(alias), js(&root.to_string()), utc(record.registered_ns), js(&binding.anchor), js(&cone.digest().to_hex()), nodes
            ));
        }
        Ok(format!(
            "{{\"status\":\"checked\",\"scope\":\"declared_ancestry_at_registration\",\"provenance_sha256\":{},\"time_evidence\":\"supplied_not_verified\",\"generator_replay\":\"not_performed\",\"forecasts\":[{}]}}",
            js(input_sha),
            checked.join(",")
        ))
    }
}

pub(super) fn verify(
    path: &Path,
    replay: &Replay,
    report: &Assessment,
) -> Result<String, AssessmentFailure> {
    let bytes = std::fs::read(path).map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("forecast provenance {}: {e}", path.display()),
        )
    })?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| invalid("forecast provenance must be UTF-8"))?;
    Bundle::parse(text)?.verify(replay, report, &sha256(&bytes).to_hex())
}

/// Provenance must preserve a live registration's exact nanoseconds. The journal's frozen request
/// grammar remains unchanged; only supplied provenance timestamps admit fractional seconds.
fn timestamp(text: &str) -> Result<i64, Refusal> {
    let Some((whole, fraction)) = text.split_once('.') else {
        return super::time(text);
    };
    let invalid_time = || {
        invalid(format!(
            "{text:?} must be UTC with 1..=9 fractional second digits within the signed nanosecond range"
        ))
    };
    let fraction = fraction
        .strip_suffix('Z')
        .filter(|digits| {
            (1..=9).contains(&digits.len()) && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        .ok_or_else(invalid_time)?;
    let seconds = super::parse_utc(&format!("{whole}Z")).ok_or_else(invalid_time)?;
    let mut nanos = fraction
        .bytes()
        .fold(0_i128, |n, byte| n * 10 + i128::from(byte - b'0'));
    for _ in fraction.len()..9 {
        nanos *= 10;
    }
    i64::try_from(i128::from(seconds) * 1_000_000_000 + nanos).map_err(|_| invalid_time())
}

fn node(row: &Row<'_>) -> Result<TemporalNode, Refusal> {
    row.allowed(
        &[
            "id",
            "kind",
            "parents",
            "origin",
            "available_at",
            "performed_at",
            "record_cell",
            "seq",
            "recorded_at",
        ],
        &[],
    )?;
    let id = object_id(row.string("id")?)?;
    let kind = match row.string("kind")? {
        "observation" => NodeKind::Observation,
        "attested_interpretation" => NodeKind::AttestedInterpretation,
        "fitted_parameters" => NodeKind::FittedParameters,
        "correlation_card" => NodeKind::CorrelationCard,
        "exclusion_act" => NodeKind::ExclusionAct,
        "price_deck" => NodeKind::PriceDeck,
        "registry_row" => NodeKind::RegistryRow,
        "reference_prior" => NodeKind::ReferencePrior,
        "claim" => NodeKind::Claim,
        "forecast" => NodeKind::Forecast,
        "decision" => NodeKind::Decision,
        "recipe" => NodeKind::Recipe,
        other => return Err(invalid(format!("unknown temporal node kind {other:?}"))),
    };
    let parents = row
        .optional_string("parents")?
        .map(|s| s.split(',').map(|s| object_id(s.trim())).collect())
        .transpose()?
        .unwrap_or_default();
    let available_at = row
        .optional_string("available_at")?
        .map(timestamp)
        .transpose()?;
    let performed_at = row
        .optional_string("performed_at")?
        .map(timestamp)
        .transpose()?;
    let availability = match row.string("origin")? {
        "source" if performed_at.is_none() => Availability::Source { available_at },
        "derived" => Availability::Derived {
            performed_at,
            declared_available_at: available_at,
        },
        _ => {
            return Err(invalid(
                "origin must be source without performed_at, or derived",
            ));
        }
    };
    let cell = row.optional_string("record_cell")?;
    let seq = row.optional_integer("seq")?;
    let recorded_at = row
        .optional_string("recorded_at")?
        .map(timestamp)
        .transpose()?;
    let recorded_at = match (cell, seq, recorded_at) {
        (None, None, None) => None,
        (Some(cell), Some(_), Some(at)) => Some(RecordTime::new(
            cell,
            sequence(row)?,
            at,
            TimeEvidence::OfflineUncertain,
        )),
        _ => {
            return Err(invalid(
                "record_cell, seq and recorded_at must be supplied together",
            ));
        }
    };
    Ok(TemporalNode {
        id,
        kind,
        parents,
        availability,
        recorded_at,
    })
}

fn object_id(text: &str) -> Result<ObjectId, Refusal> {
    text.parse()
        .map_err(|e| invalid(format!("invalid provenance object ID {text:?}: {e}")))
}

fn sequence(row: &Row<'_>) -> Result<u64, Refusal> {
    row.optional_integer("seq")?
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| invalid("seq must be a nonnegative integer"))
}

fn temporal(message: String) -> Refusal {
    Refusal::new(RefusalCode::TemporalConeViolation110, message)
}

fn failure(alias: &str, root: ObjectId, cut: i64, error: ConeError) -> AssessmentFailure {
    let frontier = if let ConeError::Violation(violation) = &error {
        violation.frontier().iter().map(|v| {
            let path = v.path.iter().map(|id| js(&id.to_string())).collect::<Vec<_>>().join(",");
            format!(
                "{{\"node\":{},\"kind\":{},\"available_at\":{},\"record_cell\":{},\"record_seq\":{},\"after_cut\":{},\"after_anchor\":{},\"path\":[{}]}}",
                js(&v.node.to_string()), js(&format!("{:?}", v.kind)), utc(v.available_at),
                js(v.recorded_at.cell()), v.recorded_at.seq(), v.after_cut, v.after_anchor, path
            )
        }).collect::<Vec<_>>().join(",")
    } else {
        String::new()
    };
    AssessmentFailure {
        diagnostics: Some(format!(
            "{{\"temporal_validation\":{{\"status\":\"refused\",\"forecast\":{},\"root\":{},\"decision_cut\":{},\"detail\":{},\"frontier\":[{}],\"generator_replay\":\"not_performed\"}}}}",
            js(alias),
            js(&root.to_string()),
            utc(cut),
            js(&error.to_string()),
            frontier
        )),
        refusal: error.refusal(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_calib::OutcomeSelector;

    const BUNDLE: &str = include_str!("../../../../tests/forecasts/provenance_synthetic.toml");
    const LEDGER: &str = include_str!("../../../../tests/forecasts/cohort_synthetic.toml");

    fn check(bundle: &str, ledger: &str) -> Result<String, AssessmentFailure> {
        let mut replay = Replay::parse(ledger).expect("synthetic ledger");
        let assessment = replay
            .assessment(
                "synthetic-demo",
                super::super::time("2026-10-31T00:00:00Z").expect("time"),
                OutcomeSelector::FirstPublished,
            )
            .expect("assessment");
        Bundle::parse(bundle)?.verify(&replay, &assessment, &sha256(bundle.as_bytes()).to_hex())
    }

    #[test]
    fn all_frozen_members_are_checked_at_registration_including_missing_outcomes() {
        let result = check(BUNDLE, LEDGER).expect("clean ancestry");
        assert_eq!(result, check(BUNDLE, LEDGER).expect("same inputs"));
        assert_eq!(result.matches("\"cone_digest\"").count(), 3);
        assert_eq!(
            result
                .matches("\"decision_cut\":\"2026-07-01T00:00:00Z\"")
                .count(),
            3
        );
        assert!(result.contains("south-unit-cost"));
        assert!(result.contains("\"time_evidence\":\"supplied_not_verified\""));
        assert!(result.contains("\"generator_replay\":\"not_performed\""));
    }

    #[test]
    fn fitted_input_exclusion_and_deck_leaks_refuse_with_frontier_paths() {
        for (before, node) in [
            (
                "2026-06-15T00:00:00Z",
                "evd_11111111111111111111111111111111",
            ),
            (
                "2026-06-20T00:00:00Z",
                "evd_33333333333333333333333333333333",
            ),
            (
                "2026-06-25T00:00:00Z",
                "evd_44444444444444444444444444444444",
            ),
        ] {
            // The evidence was visible by October's assessment, but unavailable at July's registration.
            let changed = BUNDLE.replace(before, "2026-08-01T00:00:00Z");
            let error = check(&changed, LEDGER).expect_err("future evidence");
            assert_eq!(error.refusal.code, "FMINE-REF-110");
            let diagnostics = error.diagnostics.expect("typed cone diagnostic");
            assert!(
                diagnostics.contains(&format!("\"node\":\"{node}\"")),
                "{diagnostics}"
            );
            assert!(diagnostics.contains("\"path\":[\"for_"));
            assert!(diagnostics.contains("\"after_cut\":true"));
            assert!(diagnostics.contains("2026-07-01T00:00:00Z"));
        }
    }

    #[test]
    fn anchor_visibility_is_required_even_when_wall_clock_is_early() {
        let changed = BUNDLE.replace(
            "\ncell = \"observations\"\nseq = 10",
            "\ncell = \"observations\"\nseq = 9",
        );
        let error = check(&changed, LEDGER).expect_err("outside anchor");
        assert_eq!(error.refusal.code, "FMINE-REF-110");
        assert!(
            error
                .diagnostics
                .expect("diagnostic")
                .contains("\"after_anchor\":true")
        );
    }

    #[test]
    fn missing_time_missing_member_and_wrong_forecast_binding_never_pass() {
        let missing_time = BUNDLE.replace("available_at = \"2026-06-15T00:00:00Z\"\n", "");
        assert_eq!(
            check(&missing_time, LEDGER)
                .expect_err("missing time")
                .refusal
                .code,
            "FMINE-REF-110"
        );
        let tail = BUNDLE.rfind("[[binding]]").expect("final binding");
        let error = check(&BUNDLE[..tail], LEDGER).expect_err("missing unresolved member");
        assert!(error.refusal.what.contains("south-unit-cost"));
        let changed = LEDGER.replace("lo = \"90\"", "lo = \"91\"");
        let error = check(BUNDLE, &changed).expect_err("different forecast payload");
        assert!(error.refusal.what.contains("frozen forecast ID"));
        let changed = BUNDLE.replace(
            "performed_at = \"2026-07-01T00:00:00Z\"",
            "performed_at = \"2026-06-30T00:00:00Z\"",
        );
        assert!(
            check(&changed, LEDGER)
                .expect_err("backdated forecast root")
                .refusal
                .what
                .contains("registered_at")
        );
    }

    #[test]
    fn strict_schema_and_lane_checks_preserve_the_evidence_boundary() {
        for changed in [
            BUNDLE.replace("lane = \"synthetic\"", "lane = \"public\""),
            BUNDLE.replace("kind = \"observation\"", "kind = \"invented\""),
            BUNDLE.replace("origin = \"source\"", "origin = \"guess\""),
            BUNDLE.replace("seq = 10", "seq = -1"),
            format!(
                "{BUNDLE}\n[[anchor]]\nid = \"registration\"\ncell = \"observations\"\nseq = 10\n"
            ),
            format!(
                "{BUNDLE}\n[[binding]]\nforecast = \"south-unit-cost\"\nforecast_id = \"for_76ff13dac8c451928240cc7c75548508\"\nanchor = \"registration\"\n"
            ),
        ] {
            assert!(
                Bundle::parse(&changed).is_err(),
                "malformed provenance accepted"
            );
        }
        assert!(
            check(
                &BUNDLE.replace("lane = \"synthetic\"", "lane = \"supplied\""),
                LEDGER
            )
            .is_err()
        );
    }

    #[test]
    fn unrelated_future_objects_do_not_rewrite_any_cone_digest() {
        let extra = format!(
            "{BUNDLE}\n[[node]]\nid = \"evd_55555555555555555555555555555555\"\nkind = \"registry_row\"\norigin = \"source\"\navailable_at = \"2027-01-01T00:00:00Z\"\n"
        );
        let original = Bundle::parse(BUNDLE).expect("bundle");
        let changed = Bundle::parse(&extra).expect("bundle with an unrelated incomplete node");
        for binding in original.bindings.values() {
            let cut = ConeCut {
                decision_ns: super::super::time("2026-07-01T00:00:00Z").expect("time"),
                anchor: original.anchors[&binding.anchor].clone(),
            };
            assert_eq!(
                original
                    .graph
                    .check(binding.forecast_id, &cut)
                    .expect("clean")
                    .digest(),
                changed
                    .graph
                    .check(binding.forecast_id, &cut)
                    .expect("unrelated")
                    .digest()
            );
        }
    }
}
