//! Public lane (`GATE-BULL-01`, `BD-055`, `BD-058`, `BD-066`, `BD-321`, `BD-322`): loads the registered public
//! workbooks of an issuer, refusing unless every file is present with its registered SHA-256; measures
//! extraction (cell census and numeric-cell digest per workbook); builds the five-state identity ledger over
//! the issuer's AISC reconciliation tables; and commits the whole result to a state root.
//!
//! Nothing here is a stand-in: a missing or altered input is a typed refusal (`FMINE-REF-502`) naming the
//! file. Synthetic workbooks exist only in the synthetic lane, which says so and is never gate-eligible.

pub mod aisc;
pub mod citation;
pub mod key;
pub mod live;
pub mod mine_ledger;
pub mod mine_stats;
pub mod sources;
pub mod synthetic;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_claim::{AdditivityClass, Claim, ConfidentialityTier, Representation, SpatialSupport};
use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal};
use fmine_io::xlsx::{
    CellCensus, CellValue, ExtractedCell, QuarantineItem, Workbook, parse_a1_ref,
};

use crate::aisc::{
    CitedCell, ConsistentBasis, DEFAULT_MISSING_FACT, ExplanationBasis, FootingDiscovery,
    IdentityFamily, IdentityInstance, IdentityState, MemberWeighting, OneCellCheck, OneCellOutcome,
    Operand, OperandRole, OperandValue, RateMiss, ReconstructionCheck, ShareBasis, StatedShare,
    Undecided, UnregisteredHeader, cell_text, confirm_share_label, find_qtd_sheet,
    footing_discovery, ledger_for_table, locate_table, one_cell_checks, operand_at,
    reconstruction_holds,
};
use crate::mine_ledger::{LedgerFile, LedgerRules, MineLedger, mine_ledger};
use crate::mine_stats::{CellRefusal, LabelVocabulary, MineStatsRead, read_mine_stats};
pub use crate::sources::load_issuer_profile;
use crate::sources::{
    ExpectedRoot, IssuerProfile, RegisteredMissingFact, RegisteredReconstruction, SourceKind,
};

/// Converts an extracted cell into a public-basis `Claim<Option<f64>>` (`BD-058`): a number is the value as
/// published, a placeholder is a known unknown, and text or a formula error is refused.
pub fn cell_to_public_claim(
    workbook: &Workbook,
    sheet_name: &str,
    metric_key: &str,
    cell: &ExtractedCell,
    additivity: AdditivityClass,
) -> Result<Claim<Option<f64>>, Refusal> {
    let anchor = workbook.make_anchor(sheet_name, &cell.cell_ref);
    match &cell.value {
        CellValue::Number(v) => Ok(Claim::new(
            metric_key,
            Some(*v),
            &format!("{v}"),
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            additivity,
            EvidenceClass::Exact,
            KnowledgeState::KnownKnown,
            ConfidentialityTier::Public,
            vec![],
            vec![],
            vec![anchor],
        )),
        CellValue::Placeholder(tok) => Ok(Claim::new(
            metric_key,
            None,
            tok,
            Representation::Placeholder { token: tok.clone() },
            SpatialSupport::PeriodAggregate,
            additivity,
            EvidenceClass::Placeholder,
            KnowledgeState::KnownUnknown,
            ConfidentialityTier::Public,
            vec![],
            vec![],
            vec![anchor],
        )),
        CellValue::FormulaError(err_tok) => Err(Refusal::from_code(
            "FMINE-REF-401",
            format!(
                "quarantined formula error '{err_tok}' at {sheet_name}!{}",
                cell.cell_ref
            ),
        )),
        CellValue::Text(txt) => Err(Refusal::from_code(
            "FMINE-REF-401",
            format!(
                "expected a number or placeholder at {sheet_name}!{}, found text '{txt}'",
                cell.cell_ref
            ),
        )),
    }
}

/// Which lane produced a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// Registered public filings, verified by digest. Eligible for `GATE-BULL-01`.
    Public,
    /// Invented workbooks with seeded answers. Never gate-eligible.
    Synthetic,
}

impl Lane {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Synthetic => "synthetic",
        }
    }
}

/// One loaded input workbook with its provenance and extraction census.
#[derive(Debug, Clone)]
pub struct LoadedSource {
    pub file_name: String,
    pub kind: SourceKind,
    pub quarter: String,
    pub url: String,
    pub url_basis: String,
    pub bytes: u64,
    pub workbook: Workbook,
    pub census: CellCensus,
}

fn hex(bytes: &[u8; 32]) -> String {
    Digest32(*bytes).to_hex()
}

/// Loads every registered workbook of `profile` from `<root>/<corpus_dir>/`, checking byte size and SHA-256
/// against the registry. Refuses (`FMINE-REF-502`) naming every missing, altered or unparseable file.
pub fn load_registered_sources(
    profile: &IssuerProfile,
    root: &Path,
) -> Result<Vec<LoadedSource>, Refusal> {
    let dir = root.join(&profile.corpus_dir);
    let mut loaded = Vec::with_capacity(profile.sources.len());
    let mut missing = Vec::new();
    let mut altered = Vec::new();
    let mut unreadable = Vec::new();
    for src in &profile.sources {
        let bytes = match std::fs::read(dir.join(&src.file_name)) {
            Ok(b) => b,
            Err(e) => {
                missing.push(format!("{} ({e})", src.file_name));
                continue;
            }
        };
        let digest = sha256(&bytes).to_hex();
        if bytes.len() as u64 != src.bytes || digest != src.sha256_hex {
            altered.push(format!(
                "{} (registered {} bytes sha256 {}, found {} bytes sha256 {digest})",
                src.file_name,
                src.bytes,
                src.sha256_hex,
                bytes.len()
            ));
            continue;
        }
        let rel = format!("{}/{}", profile.corpus_dir, src.file_name);
        match Workbook::from_bytes(rel, &bytes) {
            Ok(workbook) => {
                let census = workbook.cell_census();
                loaded.push(LoadedSource {
                    file_name: src.file_name.clone(),
                    kind: src.kind,
                    quarter: src.quarter.clone(),
                    url: src.url.clone(),
                    url_basis: src.url_basis.clone(),
                    bytes: src.bytes,
                    workbook,
                    census,
                });
            }
            Err(e) => unreadable.push(format!("{} ({e})", src.file_name)),
        }
    }
    if missing.is_empty() && altered.is_empty() && unreadable.is_empty() {
        return Ok(loaded);
    }
    let mut detail = format!(
        "{} of {} registered {} workbooks unusable under {}",
        missing.len() + altered.len() + unreadable.len(),
        profile.sources.len(),
        profile.issuer,
        dir.display()
    );
    for (label, list) in [
        ("missing", &missing),
        ("digest mismatch", &altered),
        ("unreadable", &unreadable),
    ] {
        if !list.is_empty() {
            let _ = write!(detail, "; {label}: {}", list.join(", "));
        }
    }
    Err(Refusal::from_code("FMINE-REF-502", detail))
}

/// A sheet that holds a cost-of-sales row but whose table structure could not be located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRefusal {
    pub file: String,
    pub sheet: String,
    pub reason: String,
}

/// Report of one assurance run (`fmine assure public`).
#[derive(Debug, Clone)]
pub struct PublicAssuranceReport {
    pub issuer: String,
    pub lane: Lane,
    pub retrieved_utc: String,
    /// The `--through YYYYQn` bound the run applied, if any. Not in the state root: the sources it kept are.
    pub through: Option<String>,
    pub sources: Vec<LoadedSource>,
    pub aisc_tables: usize,
    pub table_refusals: Vec<TableRefusal>,
    pub instances: Vec<IdentityInstance>,
    pub unregistered_headers: Vec<UnregisteredHeader>,
    /// The label-anchored read of each registered Mine Stats workbook, by file name (bd-gwf1.1). The read itself
    /// is not in the state root (it enters with the Mine Stats claims, BD-332.S3); the ledger over it is.
    pub mine_stats: Vec<(String, MineStatsRead)>,
    /// The Mine Stats identity ledger (BD-331.S4): ownership and quarter sums on the summary sheets. Bound into
    /// the state root in its own section; counted apart from `instances` and the totals.
    pub mine_ledger: MineLedger,
    /// The same families run over hidden worksheets (BD-331.S2). Bound into the state root in its own section
    /// and never counted in `instances`, the totals or `GATE-BULL-01`.
    pub hidden_scope: HiddenScope,
    /// Appendix K.4's one-cell cost-of-sales check on every registered-subject column of the visible AISC
    /// tables (bd-prmn.8). A diagnostic: it changes no instance's state and is not in the state root.
    pub one_cell: Vec<OneCellCheck>,
    pub ledger_anchor: LedgerAnchor,
}

/// How every hidden-scope instance is labelled.
pub const HIDDEN_SCOPE: &str = "sheet not displayed to readers";

/// The identity ledger over the AISC tables on hidden worksheets: reported, bound into the root, never counted.
#[derive(Debug, Clone, Default)]
pub struct HiddenScope {
    pub aisc_tables: usize,
    pub instances: Vec<IdentityInstance>,
    pub unregistered_headers: Vec<UnregisteredHeader>,
    pub table_refusals: Vec<TableRefusal>,
}

/// Counts of the alternative construction (subtotal rounded once), reported and never scored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlternateCounts {
    /// Footing instances in states (i) or (iii) that the alternative construction accepts.
    pub footing_consistent: usize,
    /// Per-ounce instances consistent under the declared construction that the alternative also accepts.
    pub rate_consistent: usize,
}

impl PublicAssuranceReport {
    /// Only a public-lane run can count toward `GATE-BULL-01`.
    #[must_use]
    pub fn gate_eligible(&self) -> bool {
        self.lane == Lane::Public
    }

    /// Instance counts keyed by `(family, state code)`.
    #[must_use]
    pub fn state_counts(&self) -> BTreeMap<(IdentityFamily, &'static str), usize> {
        let mut m = BTreeMap::new();
        for i in &self.instances {
            *m.entry((i.family, i.state.code())).or_insert(0) += 1;
        }
        m
    }

    /// Instance count in one state code across all families.
    #[must_use]
    pub fn count_state(&self, code: &str) -> usize {
        self.instances
            .iter()
            .filter(|i| i.state.code() == code)
            .count()
    }

    /// (i) instances whose consistency is only a relaxation that does not refute them (BD-331.S4: a region not
    /// refuted by its members' weighted mean). They show no conflict and prove no value.
    #[must_use]
    pub fn relaxation_only(&self) -> usize {
        self.instances
            .iter()
            .filter(|i| {
                matches!(
                    i.state,
                    IdentityState::Consistent {
                        basis: Some(ConsistentBasis::Members(_)),
                        ..
                    }
                )
            })
            .count()
    }

    #[must_use]
    pub fn alternate_counts(&self) -> AlternateCounts {
        let mut a = AlternateCounts {
            footing_consistent: 0,
            rate_consistent: 0,
        };
        for i in &self.instances {
            let footing = matches!(
                i.family,
                IdentityFamily::TccFooting | IdentityFamily::AiscFooting
            );
            match &i.state {
                IdentityState::Consistent {
                    alternate_consistent: true,
                    ..
                } if footing => a.footing_consistent += 1,
                IdentityState::IssuerDiscrepancy(c) if c.alternate_consistent => {
                    a.footing_consistent += 1
                }
                IdentityState::Consistent {
                    alternate_consistent: true,
                    ..
                } => a.rate_consistent += 1,
                _ => {}
            }
        }
        a
    }

    /// The `mine_stats_ledger` section (BD-331.S4): counts per family and state (refusals by reason), every (iii)
    /// instance, and every jointly infeasible component with its minimum correction sets and implied ranges.
    fn mine_ledger_json(&self, o: &mut String) {
        use mine_ledger::{Family, State};
        let l = &self.mine_ledger;
        let list = |cells: &[String]| cells.iter().map(|c| js(c)).collect::<Vec<_>>().join(",");
        o.push_str("\"mine_stats_ledger\":{\"construction\":\"rounded_once\",\"families\":[");
        for (n, family) in Family::ALL.into_iter().enumerate() {
            let mut by_reason: BTreeMap<&str, usize> = BTreeMap::new();
            for i in l.instances.iter().filter(|i| i.family == family) {
                if let State::Refused { reason, .. } = &i.state {
                    *by_reason.entry(reason.code()).or_insert(0) += 1;
                }
            }
            let _ = write!(
                o,
                "{}{{\"family\":{},\"n\":{}",
                if n == 0 { "" } else { "," },
                js(family.as_str()),
                l.instances.iter().filter(|i| i.family == family).count()
            );
            for code in ["i", "iii", "iv", "v", "np"] {
                let _ = write!(o, ",{}:{}", js(code), l.count(family, code));
            }
            let reasons: Vec<String> = by_reason
                .iter()
                .map(|(r, k)| format!("{}:{k}", js(r)))
                .collect();
            let _ = write!(o, ",\"v_by_reason\":{{{}}}}}", reasons.join(","));
        }
        let _ = write!(
            o,
            "],\"components\":{},\"infeasible_components\":{},\"joint_only\":{},\"indeterminate\":[{}],\
             \"discrepancies\":[",
            l.components,
            l.infeasible.len(),
            l.infeasible.iter().filter(|c| c.joint_only).count(),
            l.indeterminate
                .iter()
                .map(|(id, why)| js(&format!("{id}: {why}")))
                .collect::<Vec<_>>()
                .join(",")
        );
        let iii: Vec<String> = l
            .instances
            .iter()
            .filter(|i| i.state == State::Discrepancy)
            .map(|i| {
                let cells: Vec<String> = i.cells.iter().flatten().cloned().collect();
                format!(
                    "{{\"family\":{},\"subject\":{},\"field\":{},\"group\":{},\"period\":{},\"anchor\":{},\
                     \"cells\":[{}],\"share\":{},\"printed_share\":{},\"certificate\":{},\"component\":{}}}",
                    js(i.family.as_str()),
                    js(&i.subject),
                    js(&i.field),
                    js(i.group.as_str()),
                    js(&i.period.code()),
                    js(&i.anchor),
                    list(&cells),
                    i.share
                        .map_or_else(|| "null".to_string(), |s| js(&mine_ledger::rat_text(s))),
                    i.printed_share.as_deref().map_or_else(|| "null".to_string(), js),
                    if i.nil_token { "\"nil_token\"" } else { "null" },
                    i.component.as_deref().map_or_else(|| "null".to_string(), js)
                )
            })
            .collect();
        o.push_str(&iii.join(","));
        o.push_str("],\"unresolved\":[");
        let iv: Vec<String> = l
            .instances
            .iter()
            .filter_map(|i| match &i.state {
                State::Unresolved(need) => Some((i, need)),
                _ => None,
            })
            .map(|(i, need)| {
                let cells: Vec<String> = i.cells.iter().flatten().cloned().collect();
                format!(
                    "{{\"family\":{},\"subject\":{},\"field\":{},\"group\":{},\"period\":{},\"anchor\":{},\
                     \"cells\":[{}],\"needs\":{},\"certificate\":{}}}",
                    js(i.family.as_str()),
                    js(&i.subject),
                    js(&i.field),
                    js(i.group.as_str()),
                    js(&i.period.code()),
                    js(&i.anchor),
                    list(&cells),
                    js(need.code()),
                    if i.nil_token { "\"nil_token\"" } else { "null" },
                )
            })
            .collect();
        o.push_str(&iv.join(","));
        o.push_str("],\"infeasible\":[");
        let comps: Vec<String> = l
            .infeasible
            .iter()
            .map(|c| {
                let anchors: Vec<String> = c
                    .instances
                    .iter()
                    .filter_map(|&k| l.instances.get(k).map(|i| i.anchor.clone()))
                    .collect();
                let sets: Vec<String> = c
                    .corrections
                    .iter()
                    .map(|s| {
                        let implied: Vec<String> = s
                            .implied
                            .iter()
                            .map(|(cell, r)| format!("{}:{}", js(cell), js(&mine_ledger::range_text(r))))
                            .collect();
                        format!(
                            "{{\"cells\":[{}],\"implied\":{{{}}}}}",
                            list(&s.cells),
                            implied.join(",")
                        )
                    })
                    .collect();
                format!(
                    "{{\"component\":{},\"cells\":[{}],\"anchors\":[{}],\"joint_only\":{},\"minimum\":{},\
                     \"corrections\":[{}]}}",
                    js(&c.id),
                    list(&c.cells),
                    list(&anchors),
                    c.joint_only,
                    c.minimum,
                    sets.join(",")
                )
            })
            .collect();
        o.push_str(&comps.join(","));
        o.push_str("]},");
    }

    /// The `one_cell_cos` section (bd-prmn.8): counts over every checked column, then each failing column.
    fn one_cell_json(&self, o: &mut String) {
        let count = |state: &str| self.one_cell.iter().filter(|c| c.state() == state).count();
        let _ = write!(
            o,
            "\"one_cell_cos\":{{\"diagnostic\":\"Appendix K.4 one-cell cost-of-sales check; not a ledger state, \
             not in the state root\",\"columns\":{},\"pass\":{},\"fail\":{},\"refused\":{},\"failing\":[",
            self.one_cell.len(),
            count("pass"),
            count("fail"),
            count("refused")
        );
        let failing = self.one_cell.iter().filter_map(|c| match &c.outcome {
            OneCellOutcome::Checked {
                basis: (a, b),
                pass: false,
                at_full,
                at_share,
                implied_oz: (lo, hi),
            } => Some(format!(
                "{{\"file\":{},\"sheet\":{},\"column\":{},\"subject\":{},\"basis\":\"{a}/{b}\",\"at_full\":{at_full},\
                 \"at_share\":{},\"implied_oz\":[{},{}]}}",
                js(&c.file),
                js(&c.sheet),
                js(&c.column),
                js(&c.subject),
                at_share.map_or_else(|| "null".to_string(), |s| s.to_string()),
                js(lo),
                js(hi)
            )),
            _ => None,
        });
        o.push_str(&failing.collect::<Vec<_>>().join(","));
        o.push_str("]},");
    }

    /// Which rounding construction the TCC and AISC footings fit (bd-4wvl.4, BD-340.S4 part 1): the visible
    /// scope, then the hidden one. A report: it changes no state and is not in the state root.
    pub fn construction_discovery(
        &self,
    ) -> Result<Vec<(&'static str, FootingDiscovery)>, fmine_units::UnitsError> {
        let mut out = Vec::new();
        for (scope, instances) in [
            ("visible", &self.instances),
            ("hidden", &self.hidden_scope.instances),
        ] {
            out.extend(
                footing_discovery(instances)?
                    .into_iter()
                    .map(|d| (scope, d)),
            );
        }
        Ok(out)
    }

    /// The `construction_discovery` section (bd-4wvl.4): one entry per footing family and scope.
    fn construction_discovery_json(&self, o: &mut String) {
        let _ = write!(
            o,
            "\"construction_discovery\":{{\"report_only\":\"which rounding construction the decided footings fit; \
             never a state, not in the state root\",\"alpha\":{:e},\"null\":\"rounded_once: every printed value \
             is its true value rounded once, independently\",",
            fmine_numerics::exact_feas::discover::ALPHA
        );
        match self.construction_discovery() {
            Err(e) => {
                let _ = write!(o, "\"refused\":{}}},", js(&e.to_string()));
            }
            Ok(found) => {
                let entries: Vec<String> = found
                    .iter()
                    .map(|(scope, d)| discovery_json(scope, d))
                    .collect();
                let _ = write!(o, "\"families\":[{}]}},", entries.join(","));
            }
        }
    }

    /// One (ii), (iii) or (iv) instance as a JSON object; `scope` labels a hidden-scope instance.
    fn exception_json(&self, i: &IdentityInstance, scope: Option<&str>) -> String {
        let mut o = format!(
            "{{\"state\":{},\"label\":{},\"file\":{},\"sha256\":{},\"sheet\":{},\"cell\":{},\
             \"subject\":{},\"family\":{},\"detail\":{}",
            js(i.state.code()),
            js(i.state.label()),
            js(&i.file),
            js(&self.source_sha(&i.file)),
            js(&i.sheet),
            js(&i.cell),
            js(&i.subject),
            js(i.family.as_str()),
            js(&i.state.detail())
        );
        if let Some(code) = i.state.reason_code() {
            let _ = write!(o, ",\"reason_code\":{}", js(code));
        }
        if let Some(scope) = scope {
            let _ = write!(o, ",\"scope\":{}", js(scope));
        }
        let sha_of = |f: &str| self.source_sha(f);
        let derived = citation::derived(i);
        match &i.state {
            IdentityState::IssuerDiscrepancy(c) => {
                let lines: Vec<String> = c.line_cells.iter().map(|l| js(l)).collect();
                // The three numbers as citations (bd-sjrh.1): the printed total as read, the sum and the residual
                // with their rules and inputs.
                let printed = i
                    .operands
                    .iter()
                    .find(|o| o.role == OperandRole::Total)
                    .map_or_else(
                        || "null".to_string(),
                        |o| citation::extracted(o, &sha_of).to_json(),
                    );
                let by_name = |name: &str| {
                    derived
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map_or_else(|| "null".to_string(), |(_, d)| d.to_json())
                };
                let _ = write!(
                    o,
                    ",\"certificate\":{{\"printed\":{},\"component_sum\":{},\"residual\":{},\"total_cell\":{},\
                     \"line_cells\":[{}],\"sign_flip_witness\":{},\"alternate_consistent\":{}}}",
                    printed,
                    by_name("component_sum"),
                    by_name("residual"),
                    js(&c.total_cell),
                    lines.join(","),
                    c.sign_flip_witness
                        .as_deref()
                        .map_or_else(|| "null".to_string(), js),
                    c.alternate_consistent
                );
            }
            IdentityState::Explained { rule, .. } => {
                let _ = write!(o, ",\"rule\":{}", js(rule));
            }
            IdentityState::Undecidable {
                missing_fact,
                closing_document,
                ..
            } => {
                let _ = write!(
                    o,
                    ",\"missing_fact\":{},\"closing_document\":{}",
                    js(missing_fact),
                    closing_document
                        .as_deref()
                        .map_or_else(|| "null".to_string(), js)
                );
            }
            _ => {}
        }
        let _ = write!(o, ",\"cited\":{}", citation::cited_json(i, &sha_of));
        o.push('}');
        o
    }

    /// One compact record per counted instance (bd-sjrh.1): its cell, family, state and cited numbers.
    fn instance_json(&self, i: &IdentityInstance) -> String {
        format!(
            "{{\"at\":{},\"family\":{},\"state\":{},\"cited\":{}}}",
            js(&format!("{}:{}!{}", i.file, i.sheet, i.cell)),
            js(i.family.as_str()),
            js(i.state.code()),
            citation::cited_json(i, &|f| self.source_sha(f))
        )
    }

    /// sha256 over the sorted `"{file}\t{sha256}\n"` lines of the sources this run read (BD-339.S1).
    #[must_use]
    pub fn registry_epoch(&self) -> String {
        let mut lines: Vec<String> = self
            .sources
            .iter()
            .map(|s| format!("{}\t{}\n", s.file_name, hex(&s.workbook.sha256)))
            .collect();
        lines.sort();
        sha256(lines.concat().as_bytes()).to_hex()
    }

    /// sha256 over `"{file}\t{numeric_digest}\n"` in registry order: one value an independent reader of the
    /// cells can reproduce (BD-339.S1).
    #[must_use]
    pub fn census_digest_of_digests(&self) -> String {
        let lines: String = self
            .sources
            .iter()
            .map(|s| format!("{}\t{}\n", s.file_name, hex(&s.census.numeric_digest)))
            .collect();
        sha256(lines.as_bytes()).to_hex()
    }

    /// Compares this run with a row of the expected-roots file: root, anchor, totals, registry epoch and digest
    /// of digests must all be equal. The error names every field that differs.
    pub fn check_expected(&self, expected: &ExpectedRoot) -> Result<(), String> {
        let totals = STATE_CODES.map(|c| self.count_state(c) as u64);
        let checks = [
            (
                "state_root",
                self.ledger_anchor.state_root.to_hex(),
                expected.state_root.clone(),
            ),
            (
                "anchor_id",
                self.ledger_anchor.anchor_id().to_string(),
                expected.anchor_id.clone(),
            ),
            (
                "totals",
                format!("{totals:?}"),
                format!("{:?}", expected.totals),
            ),
            (
                "registry_epoch",
                self.registry_epoch(),
                expected.registry_epoch.clone(),
            ),
            (
                "census_digest_of_digests",
                self.census_digest_of_digests(),
                expected.census_digest_of_digests.clone(),
            ),
        ];
        let differ: Vec<String> = checks
            .iter()
            .filter(|(_, got, want)| got != want)
            .map(|(name, got, want)| format!("{name}: run {got}, expected {want}"))
            .collect();
        if differ.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "run differs from the expected row of commit {} (line {}): {}",
                expected.commit,
                expected.line,
                differ.join("; ")
            ))
        }
    }

    fn source_sha(&self, file: &str) -> String {
        self.sources
            .iter()
            .find(|s| s.file_name == file)
            .map(|s| hex(&s.workbook.sha256))
            .unwrap_or_default()
    }

    /// Canonical JSON payload for the CLI envelope.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut o = String::new();
        // The no-claim line, the four flags and the next actions are envelope fields (fmine-cli), not payload.
        let _ = write!(
            o,
            "{{\"issuer\":{},\"lane\":{},\"gate_eligible\":{},",
            js(&self.issuer),
            js(self.lane.as_str()),
            self.gate_eligible()
        );
        let _ = write!(
            o,
            "\"retrieved_utc\":{},\"through\":{},\"sources\":[",
            js(&self.retrieved_utc),
            self.through
                .as_deref()
                .map_or_else(|| "null".to_string(), js)
        );
        for (n, s) in self.sources.iter().enumerate() {
            let c = &s.census;
            let _ = write!(
                o,
                "{}{{\"file\":{},\"kind\":{},\"quarter\":{},\"url\":{},\"url_basis\":{},\"bytes\":{},\
                 \"sha256\":{},\"visible_sheets\":{},\"hidden_sheets\":{},\"numeric_cells\":{},\
                 \"placeholder_cells\":{},\"formula_error_cells\":{},\"text_cells\":{},\"numeric_digest\":{}}}",
                if n == 0 { "" } else { "," },
                js(&s.file_name),
                js(s.kind.as_str()),
                js(&s.quarter),
                js(&s.url),
                js(&s.url_basis),
                s.bytes,
                js(&hex(&s.workbook.sha256)),
                c.visible_sheets,
                c.hidden_sheets,
                c.numeric_cells,
                c.placeholder_cells,
                c.formula_error_cells,
                c.text_cells,
                js(&hex(&c.numeric_digest))
            );
        }
        let total =
            |f: fn(&CellCensus) -> usize| self.sources.iter().map(|s| f(&s.census)).sum::<usize>();
        let _ = write!(
            o,
            "],\"extraction\":{{\"workbooks\":{},\"visible_sheets\":{},\"numeric_cells\":{},\
             \"placeholder_cells\":{},\"formula_error_cells\":{},\"text_cells\":{}}},",
            self.sources.len(),
            total(|c| c.visible_sheets),
            total(|c| c.numeric_cells),
            total(|c| c.placeholder_cells),
            total(|c| c.formula_error_cells),
            total(|c| c.text_cells)
        );
        o.push_str("\"mine_stats\":[");
        for (n, (file, read)) in self.mine_stats.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"file\":{},\"quarter\":{},\"blocks\":{},\"cells\":{},\"number\":{},\"numeric_zero\":{},\
                 \"nil\":{},\"na\":{},\"missing\":{},\"digest\":{},\"hidden_sheets\":[{}],\"refusals\":[",
                if n == 0 { "" } else { "," },
                js(file),
                js(&read.quarter),
                read.blocks,
                read.values.len(),
                read.count("number"),
                read.numeric_zeros(),
                read.count("nil"),
                read.count("na"),
                read.count("missing"),
                js(&read.digest().to_hex()),
                read.hidden_sheets
                    .iter()
                    .map(|h| js(h))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            for (k, r) in read.refusals.iter().enumerate() {
                let _ = write!(
                    o,
                    "{}{{\"locator\":{},\"reason\":{}}}",
                    if k == 0 { "" } else { "," },
                    js(&r.locator),
                    js(&r.reason)
                );
            }
            o.push_str("]}");
        }
        o.push_str("],");
        self.mine_ledger_json(&mut o);

        let counts = self.state_counts();
        let alt = self.alternate_counts();
        let _ = write!(
            o,
            "\"identity_ledger\":{{\"construction\":\"component_sum\",\"aisc_tables\":{},\"instances\":{},\
             \"by_family\":{{",
            self.aisc_tables,
            self.instances.len()
        );
        for (n, family) in IdentityFamily::ALL.iter().enumerate() {
            let _ = write!(
                o,
                "{}{}:{{\"basis\":{}",
                if n == 0 { "" } else { "," },
                js(family.as_str()),
                js(family.basis())
            );
            for code in STATE_CODES {
                let _ = write!(
                    o,
                    ",{}:{}",
                    js(code),
                    counts.get(&(*family, code)).copied().unwrap_or(0)
                );
            }
            o.push('}');
        }
        o.push_str("},\"totals\":{");
        for (n, code) in STATE_CODES.iter().enumerate() {
            let _ = write!(
                o,
                "{}{}:{}",
                if n == 0 { "" } else { "," },
                js(code),
                self.count_state(code)
            );
        }
        let _ = write!(
            o,
            "}},\"alternate_construction\":{{\"scored\":false,\"footing_consistent\":{},\
             \"rate_consistent_of_declared_consistent\":{}}},\"relaxation_only\":{},\"exceptions\":[",
            alt.footing_consistent,
            alt.rate_consistent,
            self.relaxation_only()
        );
        let exceptions: Vec<String> = self
            .instances
            .iter()
            .filter(|i| matches!(i.state.code(), "ii" | "iii" | "iv"))
            .map(|i| self.exception_json(i, None))
            .collect();
        o.push_str(&exceptions.join(","));
        o.push_str("],");
        for (key, code) in [("refused", "v"), ("not_published", "np")] {
            let mut by_reason: BTreeMap<String, (&str, Vec<String>)> = BTreeMap::new();
            for i in self.instances.iter().filter(|i| i.state.code() == code) {
                let entry = by_reason
                    .entry(i.state.detail())
                    .or_insert_with(|| (i.state.reason_code().unwrap_or_default(), Vec::new()));
                entry.1.push(format!(
                    "{}:{}!{}:{}",
                    i.file,
                    i.sheet,
                    i.cell,
                    i.family.as_str()
                ));
            }
            let _ = write!(o, "{}:[", js(key));
            for (n, (reason, (reason_code, cells))) in by_reason.iter().enumerate() {
                let cells: Vec<String> = cells.iter().map(|c| js(c)).collect();
                let _ = write!(
                    o,
                    "{}{{\"reason\":{},\"reason_code\":{},\"count\":{},\"instances\":[{}]}}",
                    if n == 0 { "" } else { "," },
                    js(reason),
                    js(reason_code),
                    cells.len(),
                    cells.join(",")
                );
            }
            o.push_str("],");
        }
        o.push_str("\"unregistered_headers\":[");
        for (n, h) in self.unregistered_headers.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"file\":{},\"sheet\":{},\"cell\":{},\"header\":{}}}",
                if n == 0 { "" } else { "," },
                js(&h.file),
                js(&h.sheet),
                js(&h.cell),
                js(&h.header)
            );
        }
        o.push_str("],\"table_refusals\":[");
        for (n, t) in self.table_refusals.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"file\":{},\"sheet\":{},\"reason\":{}}}",
                if n == 0 { "" } else { "," },
                js(&t.file),
                js(&t.sheet),
                js(&t.reason)
            );
        }
        o.push_str("]},\"instances\":[");
        let records: Vec<String> = self
            .instances
            .iter()
            .map(|i| self.instance_json(i))
            .collect();
        o.push_str(&records.join(","));
        o.push_str("],");
        self.one_cell_json(&mut o);
        let hidden = &self.hidden_scope;
        let _ = write!(
            o,
            "\"hidden_scope\":{{\"scope\":{},\"counted\":false,\"aisc_tables\":{},\"instances\":{},\"by_state\":{{",
            js(HIDDEN_SCOPE),
            hidden.aisc_tables,
            hidden.instances.len()
        );
        for (n, code) in STATE_CODES.iter().enumerate() {
            let count = hidden
                .instances
                .iter()
                .filter(|i| i.state.code() == *code)
                .count();
            let _ = write!(o, "{}{}:{count}", if n == 0 { "" } else { "," }, js(code));
        }
        let hidden_exceptions: Vec<String> = hidden
            .instances
            .iter()
            .filter(|i| matches!(i.state.code(), "ii" | "iii" | "iv"))
            .map(|i| self.exception_json(i, Some(HIDDEN_SCOPE)))
            .collect();
        let _ = write!(
            o,
            "}},\"exceptions\":[{}],\"unregistered_headers\":{},\"table_refusals\":{}}},",
            hidden_exceptions.join(","),
            hidden.unregistered_headers.len(),
            hidden.table_refusals.len()
        );
        self.construction_discovery_json(&mut o);
        o.push_str("\"quarantine\":[");
        let mut first = true;
        for s in &self.sources {
            for q in &s.workbook.quarantined {
                let item = match q {
                    QuarantineItem::HiddenSheet { sheet_name, state } => format!(
                        "{{\"file\":{},\"kind\":\"hidden_sheet\",\"sheet\":{},\"state\":{}}}",
                        js(&s.file_name),
                        js(sheet_name),
                        js(state)
                    ),
                    QuarantineItem::FormulaErrorCell {
                        sheet_name,
                        cell_ref,
                        error_token,
                    } => format!(
                        "{{\"file\":{},\"kind\":\"formula_error\",\"sheet\":{},\"cell\":{},\"token\":{}}}",
                        js(&s.file_name),
                        js(sheet_name),
                        js(cell_ref),
                        js(error_token)
                    ),
                };
                if !first {
                    o.push(',');
                }
                o.push_str(&item);
                first = false;
            }
        }
        let _ = write!(
            o,
            "],\"state_root\":{},\"anchor_id\":{}}}",
            js(&self.ledger_anchor.state_root.to_hex()),
            js(&self.ledger_anchor.anchor_id().to_string())
        );
        o
    }

    /// The commands worth running after this report, as `(command, why)`: the CLI puts them in the
    /// envelope's `next_actions`.
    #[must_use]
    pub fn next_actions(&self) -> Vec<(&'static str, &'static str)> {
        let other_lane = match self.lane {
            Lane::Public => (
                "fmine assure public --synthetic",
                "known-answer run on synthetic workbooks that exercises every ledger state",
            ),
            Lane::Synthetic => (
                "fmine assure public --issuer barrick",
                "the gate-eligible run over the workbooks in registries/sources/barrick.toml",
            ),
        };
        vec![
            other_lane,
            ("fmine robot-docs", "verbs, envelope fields and exit codes"),
        ]
    }
}

const STATE_CODES: [&str; 6] = ["i", "ii", "iii", "iv", "v", "np"];

/// What an `assure public` report does not claim; the CLI prints it in the envelope's `no_claim` field.
pub const NO_CLAIM: &str = "Reproduces and checks the issuer's published AISC reconciliation tables as printed. \
It makes no reserve, resource, geotechnical, regulatory or investment claim and implies no issuer affiliation.";

/// One family and scope of the `construction_discovery` section, as a JSON object (also the real-file test's log
/// line). Counts and probabilities only, never a cell value. A log tail of −∞ (probability 0) is `null`.
#[must_use]
pub fn discovery_json(scope: &str, d: &FootingDiscovery) -> String {
    let r = &d.report;
    let ln = |x: f64| {
        if x.is_finite() {
            format!("{x:.3}")
        } else {
            "null".to_string()
        }
    };
    let expected = r.expected_rounded_once.to_f64();
    let rate = if r.n == 0 {
        "null".to_string()
    } else {
        format!("{:.4}", expected / r.n as f64)
    };
    format!(
        "{{\"family\":{},\"scope\":{},\"totals\":{},\"foot_exact\":{},\"distinct\":{},\"distinct_foot_exact\":{},\
         \"rounded_once_accepts\":{},\"indeterminate\":{},\"expected_rounded_once\":{expected:.3},\
         \"expected_rate\":{rate},\"ln_upper_tail\":{},\"ln_lower_tail\":{},\"ln_component_sum_error_tail\":{},\
         \"ln_rounded_once_error_tail\":{},\"discovered\":{},\"reason_code\":{}}}",
        js(d.family.as_str()),
        js(scope),
        d.totals,
        d.foot_exact,
        r.n,
        r.foot_exact,
        r.rounded_once_accepts,
        r.indeterminate,
        ln(r.ln_upper_tail),
        ln(r.ln_lower_tail),
        ln(r.ln_component_sum_error_tail),
        ln(r.ln_rounded_once_error_tail),
        js(r.discovered.as_str()),
        r.discovered
            .reason_code()
            .map_or_else(|| "null".to_string(), js)
    )
}

/// JSON string literal.
fn js(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Tries a registered reconstruction for a per-ounce miss. `None` when none is registered for the instance;
/// otherwise (ii) when every label check passes and the component closes the interval, or (iv) naming the
/// check that failed.
fn find_reconstruction<'a>(
    recs: &'a [RegisteredReconstruction],
    file: &str,
    sheet: &str,
    subject: &str,
    family: IdentityFamily,
) -> Option<&'a RegisteredReconstruction> {
    recs.iter().find(|r| {
        r.table_file == file && r.sheet == sheet && r.subject == subject && r.family == family
    })
}

/// The operand at `file:sheet!cell` of the loaded sources; blank when that file or sheet is not loaded.
fn operand_in(
    sources: &[LoadedSource],
    file: &str,
    sheet: &str,
    cell: &str,
    role: OperandRole,
) -> Operand {
    let ws = sources
        .iter()
        .find(|s| s.file_name == file)
        .and_then(|s| s.workbook.sheets.iter().find(|w| w.name == sheet));
    match (ws, parse_a1_ref(cell)) {
        (Some(ws), Some((r, c))) => operand_at(file, ws, r, c, role),
        _ => Operand {
            role,
            file: file.to_string(),
            sheet: sheet.to_string(),
            cell: cell.to_string(),
            value: OperandValue::Blank,
        },
    }
}

/// The cells a registered reconstruction reads (bd-sjrh.1): its stated quote cell, its label-check cells and
/// its component. A cell on a sheet or file that is not loaded is recorded as blank.
fn reconstruction_operands(
    sources: &[LoadedSource],
    rec: &RegisteredReconstruction,
) -> Vec<Operand> {
    let mut out = Vec::new();
    let mut add = |file: &str, sheet: &str, cell: &str, role: OperandRole| {
        out.push(operand_in(sources, file, sheet, cell, role));
    };
    if let ExplanationBasis::Stated { doc, locator, .. } = &rec.basis
        && let Some((sheet, cell)) = locator.split_once('!')
    {
        add(doc, sheet, cell, OperandRole::LabelCheck);
    }
    for (cell, _) in &rec.label_checks {
        add(
            &rec.component_file,
            &rec.component_sheet,
            cell,
            OperandRole::LabelCheck,
        );
    }
    add(
        &rec.component_file,
        &rec.component_sheet,
        &rec.component_cell,
        OperandRole::Component,
    );
    out
}

fn try_reconstruction(
    sources: &[LoadedSource],
    recs: &[RegisteredReconstruction],
    file: &str,
    sheet: &str,
    subject: &str,
    family: IdentityFamily,
    miss: RateMiss,
) -> Option<IdentityState> {
    let rec = find_reconstruction(recs, file, sheet, subject, family)?;
    let fail = |check: ReconstructionCheck| {
        Some(IdentityState::Undecidable {
            why: Undecided::ReconstructionNotApplied {
                evidence_id: rec.evidence_id.clone(),
                check,
            },
            missing_fact: DEFAULT_MISSING_FACT.to_string(),
            closing_document: None,
        })
    };
    if let ExplanationBasis::Stated {
        doc,
        locator,
        quote_prefix,
    } = &rec.basis
    {
        // A stated rule counts only while the issuer's text still says it (BD-331.S2).
        let quote = locator.split_once('!').and_then(|(sheet, cell)| {
            let ws = sources
                .iter()
                .find(|s| s.file_name == *doc)?
                .workbook
                .sheets
                .iter()
                .find(|s| s.name == sheet)?;
            parse_a1_ref(cell).and_then(|(r, c)| cell_text(ws, r, c))
        });
        if !quote
            .as_deref()
            .is_some_and(|t| t.starts_with(quote_prefix))
        {
            return fail(ReconstructionCheck::QuoteMismatch {
                at: format!("{doc}:{locator}"),
                found: quote.unwrap_or_default(),
                expected_prefix: quote_prefix.clone(),
            });
        }
    }
    let Some(src) = sources.iter().find(|s| s.file_name == rec.component_file) else {
        return fail(ReconstructionCheck::ComponentFileNotLoaded {
            file: rec.component_file.clone(),
        });
    };
    let Some(cs) = src
        .workbook
        .sheets
        .iter()
        .find(|s| s.name == rec.component_sheet)
    else {
        return fail(ReconstructionCheck::NoSheet {
            sheet: rec.component_sheet.clone(),
        });
    };
    for (cell, prefix) in &rec.label_checks {
        let text = parse_a1_ref(cell).and_then(|(r, c)| cell_text(cs, r, c));
        if !text.as_deref().is_some_and(|t| t.starts_with(prefix)) {
            return fail(ReconstructionCheck::LabelMismatch {
                at: format!("{}!{cell}", rec.component_sheet),
                found: text.unwrap_or_default(),
                expected_prefix: prefix.clone(),
            });
        }
    }
    let value = parse_a1_ref(&rec.component_cell)
        .and_then(|(r, c)| cs.cell(r, c))
        .and_then(|c| c.value.as_number());
    let c = match value {
        Some(v) if v.fract() == 0.0 && v.abs() < 1e15 => v as i64,
        _ => {
            return fail(ReconstructionCheck::NotWhole {
                at: format!("{}!{}", rec.component_sheet, rec.component_cell),
            });
        }
    };
    let component = CitedCell {
        file: rec.component_file.clone(),
        sheet: rec.component_sheet.clone(),
        cell: rec.component_cell.clone(),
        value: c,
    };
    match reconstruction_holds(miss, c) {
        Ok(true) => Some(IdentityState::Explained {
            evidence_id: rec.evidence_id.clone(),
            rule: rec.rule.clone(),
            component,
            basis: rec.basis.clone(),
        }),
        Ok(false) => fail(ReconstructionCheck::DoesNotClose { component }),
        Err(_) => Some(IdentityState::Refused {
            reason: crate::aisc::RefusedReason::KernelIndeterminate,
        }),
    }
}

/// What the run knows about each subject's ownership share for the release `quarter` (BD-331.S3): regions with
/// their registered members for the release (BD-331.S4), and each registered share confirmed or not against the
/// same-release Mine Stats quarter-to-date label.
fn share_bases(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    quarter: &str,
) -> BTreeMap<String, ShareBasis> {
    let mut out: BTreeMap<String, ShareBasis> = profile
        .regions
        .iter()
        .map(|r| {
            let members = profile.members_of(r, quarter);
            (r.clone(), ShareBasis::Region { members })
        })
        .collect();
    if profile.shares.is_empty() {
        return out;
    }
    let stats = sources
        .iter()
        .find(|s| s.kind == SourceKind::MineStats && s.quarter == quarter);
    let qtd = match stats {
        None => Err(format!("no Mine Stats workbook loaded for {quarter}")),
        Some(s) => find_qtd_sheet(&s.workbook.sheets, quarter)
            .map(|sheet| (s.file_name.as_str(), sheet))
            .map_err(|why| format!("{}: {why}", s.file_name)),
    };
    for share in &profile.shares {
        let basis = match &qtd {
            Err(why) => ShareBasis::Unconfirmed { why: why.clone() },
            Ok((file, sheet)) => {
                match confirm_share_label(sheet, &share.label, share.num, share.den) {
                    Ok(cell) => ShareBasis::Confirmed {
                        num: share.num,
                        den: share.den,
                        evidence: share.evidence.clone(),
                        label_cell: format!("{file}:{}!{cell}", sheet.name),
                    },
                    Err(why) => ShareBasis::Unconfirmed {
                        why: format!("{file}: {why}"),
                    },
                }
            }
        };
        out.insert(share.subject.clone(), basis);
    }
    out
}

/// Reuses the Mine Stats reads and ledger of an earlier run with equal Mine Stats inputs (bd-r7lp). The live
/// proofs rerun [`assess`] once per perturbed cell, the ledger's exact blame dominates a run, and the ledger
/// depends only on the profile and the Mine Stats workbooks. The key is the whole profile and each Mine Stats
/// source's file name, quarter and parsed workbook, compared for equality, so a change to any of them
/// recomputes. A run through a memo gives the report [`assess`] gives. Holds one entry, its first computed run's:
/// the proofs run the unperturbed sources first, and a perturbed Mine Stats cell (a printed share label, say)
/// recomputes without displacing it. Shareable across threads: runs that miss together each compute, and the
/// first to finish sets the entry.
#[derive(Debug, Default)]
pub struct MineStatsMemo {
    entry: OnceLock<MineStatsEntry>,
    hits: AtomicU64,
    misses: AtomicU64,
}

#[derive(Debug)]
struct MineStatsEntry {
    profile: IssuerProfile,
    inputs: Vec<(String, String, Workbook)>,
    reads: Vec<(String, MineStatsRead)>,
    ledger: MineLedger,
}

impl MineStatsEntry {
    fn holds(&self, profile: &IssuerProfile, sources: &[LoadedSource]) -> bool {
        let mut current = sources.iter().filter(|s| s.kind == SourceKind::MineStats);
        self.profile == *profile
            && self.inputs.iter().all(|(file, quarter, workbook)| {
                current.next().is_some_and(|s| {
                    s.file_name == *file && s.quarter == *quarter && s.workbook == *workbook
                })
            })
            && current.next().is_none()
    }
}

impl MineStatsMemo {
    /// Runs that reused the stored entry.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Runs that computed the Mine Stats section.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }
}

/// Reads every Mine Stats source and builds the ledger over the reads, or takes both from `memo` when its
/// entry holds for these inputs.
fn mine_stats_section(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    memo: Option<&MineStatsMemo>,
) -> (Vec<(String, MineStatsRead)>, MineLedger) {
    if let Some(m) = memo
        && let Some(e) = m.entry.get()
        && e.holds(profile, sources)
    {
        m.hits.fetch_add(1, Ordering::Relaxed);
        return (e.reads.clone(), e.ledger.clone());
    }
    let vocabulary: LabelVocabulary = profile.mine_stats_labels.iter().cloned().collect();
    let vocabulary = (!vocabulary.is_empty()).then_some(vocabulary);
    let mine_stats: Vec<(String, MineStatsRead)> = sources
        .iter()
        .filter(|s| s.kind == SourceKind::MineStats)
        .map(|s| {
            // The registry already checks the quarter format; a refusal here is kept, never dropped.
            let read = read_mine_stats(&s.workbook, &s.quarter, vocabulary.as_ref())
                .unwrap_or_else(|refusal| MineStatsRead {
                    quarter: s.quarter.clone(),
                    blocks: 0,
                    values: Vec::new(),
                    refusals: vec![CellRefusal {
                        locator: s.file_name.clone(),
                        reason: refusal.to_string(),
                    }],
                    hidden_sheets: Vec::new(),
                    repeated: Vec::new(),
                });
            (s.file_name.clone(), read)
        })
        .collect();
    let files: Vec<LedgerFile<'_>> = sources
        .iter()
        .filter(|s| s.kind == SourceKind::MineStats)
        .zip(&mine_stats)
        .map(|(s, (_, read))| LedgerFile {
            quarter: &s.quarter,
            read,
            workbook: &s.workbook,
        })
        .collect();
    let ledger = mine_ledger(
        &files,
        &LedgerRules {
            regions: &profile.regions,
            region_members: &profile.region_members,
            statuses: &profile.subject_statuses,
            corrections: &profile.printed_share_corrections,
            aliases: &profile.mine_stats_label_aliases,
            labels: &profile.shares,
        },
    );
    if let Some(m) = memo {
        m.misses.fetch_add(1, Ordering::Relaxed);
        if m.entry.get().is_none() {
            // A run that loses the race to set the entry keeps its own result; the entry stays the first set.
            let _ = m.entry.set(MineStatsEntry {
                profile: profile.clone(),
                inputs: sources
                    .iter()
                    .filter(|s| s.kind == SourceKind::MineStats)
                    .map(|s| (s.file_name.clone(), s.quarter.clone(), s.workbook.clone()))
                    .collect(),
                reads: mine_stats.clone(),
                ledger: ledger.clone(),
            });
        }
    }
    (mine_stats, ledger)
}

/// Builds the identity ledger over every AISC table in `sources` and commits the run to a state root.
#[must_use]
pub fn assess(
    profile: &IssuerProfile,
    lane: Lane,
    sources: Vec<LoadedSource>,
) -> PublicAssuranceReport {
    assess_inner(profile, lane, sources, None)
}

/// [`assess`], taking the Mine Stats section from `memo` when its inputs are unchanged since the memo's last
/// computed run. For callers that rerun the pipeline on perturbed copies of one set of sources.
#[must_use]
pub fn assess_memo(
    profile: &IssuerProfile,
    lane: Lane,
    sources: Vec<LoadedSource>,
    memo: &MineStatsMemo,
) -> PublicAssuranceReport {
    assess_inner(profile, lane, sources, Some(memo))
}

fn assess_inner(
    profile: &IssuerProfile,
    lane: Lane,
    sources: Vec<LoadedSource>,
    memo: Option<&MineStatsMemo>,
) -> PublicAssuranceReport {
    let issuer = profile.issuer.as_str();
    let subjects = profile.subjects.as_slice();
    let reconstructions = profile.reconstructions.as_slice();
    let mut visible = HiddenScope::default();
    let mut hidden = HiddenScope::default();
    let mut one_cell = Vec::new();
    let registered: BTreeMap<String, (i64, i64)> = profile
        .shares
        .iter()
        .map(|s| (s.subject.clone(), (s.num, s.den)))
        .collect();
    for src in sources
        .iter()
        .filter(|s| s.kind == SourceKind::AiscReconciliation)
    {
        let shares = share_bases(profile, &sources, &src.quarter);
        // A registered reconciliation holds a table on its own quarter's sheet ("Q2 2026" for 2026Q2). If that
        // sheet is missing or its cost-of-sales label stops matching, the run refuses the table by name; it
        // never drops it silently (BD-338.S3's label sample found the silent drop).
        let own_sheet = src
            .quarter
            .split_once('Q')
            .map(|(year, q)| format!("Q{q} {year}"));
        if let Some(own) = &own_sheet
            && !src.workbook.sheets.iter().any(|s| s.name == *own)
        {
            visible.table_refusals.push(TableRefusal {
                file: src.file_name.clone(),
                sheet: own.clone(),
                reason: "the release's own quarter sheet is missing".to_string(),
            });
        }
        let scopes = [
            (&src.workbook.sheets, &mut visible, true),
            (&src.workbook.hidden_sheets, &mut hidden, false),
        ];
        for (sheets, scope, shown) in scopes {
            for sheet in sheets {
                match locate_table(sheet, subjects) {
                    Ok(None) if shown && own_sheet.as_ref() == Some(&sheet.name) => {
                        scope.table_refusals.push(TableRefusal {
                            file: src.file_name.clone(),
                            sheet: sheet.name.clone(),
                            reason: "no cost-of-sales row on the release's own quarter sheet"
                                .to_string(),
                        });
                    }
                    Ok(None) => {}
                    Err(reason) => scope.table_refusals.push(TableRefusal {
                        file: src.file_name.clone(),
                        sheet: sheet.name.clone(),
                        reason,
                    }),
                    Ok(Some(table)) => {
                        scope.aisc_tables += 1;
                        let before = scope.instances.len();
                        // The reconstructions a miss consulted, whose cells become operands (bd-sjrh.1).
                        let mut consulted: Vec<(String, IdentityFamily, Vec<Operand>)> = Vec::new();
                        let mut explain =
                            |subject: &str, family: IdentityFamily, miss: RateMiss| {
                                let rec = find_reconstruction(
                                    reconstructions,
                                    &src.file_name,
                                    &sheet.name,
                                    subject,
                                    family,
                                );
                                if let Some(rec) = rec {
                                    consulted.push((
                                        subject.to_string(),
                                        family,
                                        reconstruction_operands(&sources, rec),
                                    ));
                                }
                                try_reconstruction(
                                    &sources,
                                    reconstructions,
                                    &src.file_name,
                                    &sheet.name,
                                    subject,
                                    family,
                                    miss,
                                )
                            };
                        ledger_for_table(
                            &src.file_name,
                            sheet,
                            &table,
                            subjects,
                            &shares,
                            &mut explain,
                            &mut scope.instances,
                            &mut scope.unregistered_headers,
                        );
                        for (subject, family, ops) in consulted {
                            for i in scope.instances[before..]
                                .iter_mut()
                                .filter(|i| i.subject == subject && i.family == family)
                            {
                                i.operands.extend(ops.iter().cloned());
                            }
                        }
                        // A cost-of-sales rate that read the footnote also read its confirmed share label.
                        for i in scope.instances[before..].iter_mut().filter(|i| {
                            i.family == IdentityFamily::CosPerOunce
                                && i.operands.iter().any(|o| o.role == OperandRole::LabelCheck)
                        }) {
                            let label = match shares.get(&i.subject) {
                                Some(ShareBasis::Confirmed { label_cell, .. }) => {
                                    label_cell.split_once(':').and_then(|(f, rest)| {
                                        rest.rsplit_once('!').map(|(s, c)| (f, s, c))
                                    })
                                }
                                _ => None,
                            };
                            if let Some((f, s, c)) = label {
                                let op = operand_in(&sources, f, s, c, OperandRole::LabelCheck);
                                i.operands.push(op);
                            }
                        }
                        if shown {
                            one_cell_checks(
                                &src.file_name,
                                sheet,
                                &table,
                                subjects,
                                &shares,
                                &registered,
                                &mut one_cell,
                            );
                        }
                    }
                }
            }
        }
    }
    for i in visible.instances.iter_mut().chain(&mut hidden.instances) {
        name_missing_fact(i, &profile.missing_facts);
    }
    let HiddenScope {
        aisc_tables,
        instances,
        unregistered_headers: unregistered,
        table_refusals,
    } = visible;

    let (mine_stats, mine_ledger) = mine_stats_section(profile, &sources, memo);

    // The state root binds every input digest, every census and every instance's state, so changing any
    // input cell changes the root. Version 3 adds the hidden-scope section (BD-331.S2). Version 4 binds each
    // state as reason codes and fields, never its prose, and each instance's operands (bd-sjrh.1). Version 5
    // binds each file's displayed values, which the ledger now scores (bd-jg43.3): their digest and the shown,
    // token and refused counts. Version 6 adds the Mine Stats ledger section (BD-331.S4). Version 7 adds its
    // site-sheet family and binds each instance's nil-token certificate.
    let mut enc = Encoder::new_frame(*b"M0AS", 7);
    enc.put_str(issuer);
    enc.put_str(lane.as_str());
    enc.put_u64(sources.len() as u64);
    for s in &sources {
        enc.put_str(&s.file_name);
        enc.put_fixed(&s.workbook.sha256);
        enc.put_fixed(&s.census.numeric_digest);
        let displayed = s.workbook.displayed_census();
        enc.put_fixed(&displayed.digest);
        for n in [
            s.census.visible_sheets,
            s.census.hidden_sheets,
            s.census.numeric_cells,
            s.census.placeholder_cells,
            s.census.formula_error_cells,
            s.census.text_cells,
            displayed.shown,
            displayed.token,
            displayed.refused,
        ] {
            enc.put_u64(n as u64);
        }
    }
    for (instances, unregistered, table_refusals) in [
        (&instances, &unregistered, &table_refusals),
        (
            &hidden.instances,
            &hidden.unregistered_headers,
            &hidden.table_refusals,
        ),
    ] {
        enc.put_u64(instances.len() as u64);
        for i in instances {
            for field in [
                i.file.as_str(),
                &i.sheet,
                &i.cell,
                &i.subject,
                i.family.as_str(),
                i.state.code(),
            ] {
                enc.put_str(field);
            }
            encode_state(&mut enc, &i.state);
            enc.put_u64(i.operands.len() as u64);
            for o in &i.operands {
                enc.put_str(o.role.as_str());
                enc.put_str(&citation::locator(o));
                match &o.value {
                    OperandValue::Int(x) => {
                        enc.put_u8(0);
                        enc.put_i64(*x);
                    }
                    OperandValue::Token(t) => {
                        enc.put_u8(1);
                        enc.put_str(t);
                    }
                    OperandValue::Blank => enc.put_u8(2),
                    OperandValue::Unrounded(t) => {
                        enc.put_u8(3);
                        enc.put_str(t);
                    }
                }
            }
        }
        enc.put_u64(unregistered.len() as u64);
        for h in unregistered {
            enc.put_str(&format!("{}:{}!{}={}", h.file, h.sheet, h.cell, h.header));
        }
        enc.put_u64(table_refusals.len() as u64);
        for t in table_refusals {
            enc.put_str(&format!("{}:{}:{}", t.file, t.sheet, t.reason));
        }
    }
    encode_mine_ledger(&mut enc, &mine_ledger);
    let state_root = blake3_hash(&enc.finish());
    let ledger_anchor = ledger_anchor(issuer, lane, sources.len(), state_root);
    PublicAssuranceReport {
        issuer: issuer.to_string(),
        lane,
        retrieved_utc: profile.retrieved_utc.clone(),
        through: profile.through.clone(),
        sources,
        aisc_tables,
        table_refusals,
        instances,
        unregistered_headers: unregistered,
        mine_stats,
        mine_ledger,
        hidden_scope: hidden,
        one_cell,
        ledger_anchor,
    }
}

/// Binds the Mine Stats ledger into the root frame (v7): every instance's identity, operands and state as codes
/// and locators (a refusal's reason, an (iv) item's needed fact), the share used and the share printed, the
/// nil-token certificate, then every infeasible component with its correction sets and implied ranges.
fn encode_mine_ledger(enc: &mut Encoder, ledger: &MineLedger) {
    let opt = |enc: &mut Encoder, s: Option<&str>| match s {
        None => enc.put_u8(0),
        Some(s) => {
            enc.put_u8(1);
            enc.put_str(s);
        }
    };
    enc.put_u64(ledger.instances.len() as u64);
    for i in &ledger.instances {
        for field in [
            i.family.as_str(),
            &i.subject,
            &i.field,
            i.group.as_str(),
            &i.period.code(),
            &i.anchor,
            i.state.code(),
        ] {
            enc.put_str(field);
        }
        enc.put_u64(i.cells.len() as u64);
        for c in &i.cells {
            opt(enc, c.as_deref());
        }
        match &i.state {
            mine_ledger::State::Refused { reason, at } => {
                enc.put_str(reason.code());
                if let mine_ledger::Reason::Withheld { evidence } = reason {
                    enc.put_str(evidence);
                }
                opt(enc, at.as_deref());
            }
            mine_ledger::State::Unresolved(need) => {
                enc.put_str(need.code());
            }
            _ => enc.put_u8(0),
        }
        opt(enc, i.share.map(mine_ledger::rat_text).as_deref());
        opt(enc, i.printed_share.as_deref());
        enc.put_u8(u8::from(i.nil_token));
        opt(enc, i.component.as_deref());
    }
    enc.put_u64(ledger.components as u64);
    enc.put_u64(ledger.infeasible.len() as u64);
    for c in &ledger.infeasible {
        enc.put_str(&c.id);
        enc.put_u64(c.cells.len() as u64);
        for cell in &c.cells {
            enc.put_str(cell);
        }
        enc.put_u8(u8::from(c.joint_only));
        enc.put_u8(u8::from(c.minimum));
        enc.put_u64(c.corrections.len() as u64);
        for set in &c.corrections {
            enc.put_u64(set.cells.len() as u64);
            for cell in &set.cells {
                enc.put_str(cell);
            }
            enc.put_u64(set.implied.len() as u64);
            for (cell, range) in &set.implied {
                enc.put_str(cell);
                enc.put_str(&mine_ledger::range_text(range));
            }
        }
    }
    enc.put_u64(ledger.indeterminate.len() as u64);
    for (id, reason) in &ledger.indeterminate {
        enc.put_str(&format!("{id}:{reason}"));
    }
}

/// The anchor a run commits to: everything but the state root is fixed by the issuer, the lane and the number of
/// sources read.
fn ledger_anchor(issuer: &str, lane: Lane, sources: usize, state_root: Digest32) -> LedgerAnchor {
    let cell_id = match lane {
        Lane::Public => format!("CELL-PUBLIC({issuer})"),
        Lane::Synthetic => "CELL-SYNTHETIC".to_string(),
    };
    LedgerAnchor {
        format_version: FMT_ANCHOR_V1,
        cell_id,
        ledger_epoch: 1,
        commit_sequence: sources as u64,
        adapter_registry_epoch: 1,
        schema_epoch: 1,
        policy_epoch: 1,
        privacy_epoch: 1,
        residency_epoch: 1,
        state_root,
    }
}

/// Names the fact that would decide a (iv) instance: the registered `(subject, family)` row if there is one,
/// otherwise the text the ledger gave it (its family default, or the stated share rule's).
/// Binds a state into the root frame (v4) as codes, numbers and locators, with a tag before every optional
/// part, so rewording any prose leaves the root alone.
fn encode_state(enc: &mut Encoder, state: &IdentityState) {
    let opt = |enc: &mut Encoder, s: Option<&str>| match s {
        None => enc.put_u8(0),
        Some(s) => {
            enc.put_u8(1);
            enc.put_str(s);
        }
    };
    let miss = |enc: &mut Encoder, m: &RateMiss| {
        for x in [m.r, m.n, m.k, m.q] {
            enc.put_i64(x);
        }
    };
    let share = |enc: &mut Encoder, s: &StatedShare| {
        enc.put_i64(s.num);
        enc.put_i64(s.den);
        for t in [&s.evidence, &s.footnote, &s.label_cell] {
            enc.put_str(t);
        }
    };
    let cell = |enc: &mut Encoder, c: &CitedCell| {
        for t in [&c.file, &c.sheet, &c.cell] {
            enc.put_str(t);
        }
        enc.put_i64(c.value);
    };
    let weighting = |enc: &mut Encoder, w: &MemberWeighting| {
        enc.put_u64(w.used.len() as u64);
        for m in &w.used {
            for t in [&m.subject, &m.rate_cell, &m.ounces_cell] {
                enc.put_str(t);
            }
            enc.put_i64(m.rate);
            enc.put_i64(m.ounces);
        }
        for names in [&w.excluded, &w.absent] {
            enc.put_u64(names.len() as u64);
            for n in names {
                enc.put_str(n);
            }
        }
    };
    match state {
        IdentityState::Consistent {
            alternate_consistent,
            basis,
        } => {
            enc.put_u8(u8::from(*alternate_consistent));
            match basis {
                None => enc.put_u8(0),
                Some(ConsistentBasis::Share(b)) => {
                    enc.put_u8(1);
                    share(enc, b);
                }
                Some(ConsistentBasis::Members(w)) => {
                    enc.put_u8(2);
                    weighting(enc, w);
                }
                Some(ConsistentBasis::MembersExact(w)) => {
                    enc.put_u8(3);
                    weighting(enc, w);
                }
            }
        }
        IdentityState::Explained {
            evidence_id,
            rule,
            component,
            basis,
        } => {
            enc.put_str(evidence_id);
            enc.put_str(rule);
            cell(enc, component);
            match basis {
                ExplanationBasis::Reconstructed => enc.put_u8(0),
                ExplanationBasis::Stated {
                    doc,
                    locator,
                    quote_prefix,
                } => {
                    enc.put_u8(1);
                    for t in [doc, locator, quote_prefix] {
                        enc.put_str(t);
                    }
                }
            }
        }
        IdentityState::IssuerDiscrepancy(c) => {
            for x in [c.printed, c.component_sum, c.residual] {
                enc.put_i64(x);
            }
            enc.put_str(&c.total_cell);
            enc.put_u64(c.line_cells.len() as u64);
            for l in &c.line_cells {
                enc.put_str(l);
            }
            opt(enc, c.sign_flip_witness.as_deref());
            enc.put_u8(u8::from(c.alternate_consistent));
        }
        IdentityState::Undecidable {
            why,
            missing_fact,
            closing_document,
        } => {
            enc.put_str(why.code());
            match why {
                Undecided::RateOutside(m) => miss(enc, m),
                Undecided::ShareOutside {
                    miss: m,
                    share: s,
                    s_interval,
                } => {
                    miss(enc, m);
                    share(enc, s);
                    enc.put_str(s_interval);
                }
                Undecided::ReconstructionNotApplied { evidence_id, check } => {
                    enc.put_str(evidence_id);
                    enc.put_str(check.code());
                    match check {
                        ReconstructionCheck::QuoteMismatch {
                            at,
                            found,
                            expected_prefix,
                        }
                        | ReconstructionCheck::LabelMismatch {
                            at,
                            found,
                            expected_prefix,
                        } => {
                            for t in [at, found, expected_prefix] {
                                enc.put_str(t);
                            }
                        }
                        ReconstructionCheck::ComponentFileNotLoaded { file: t }
                        | ReconstructionCheck::NoSheet { sheet: t }
                        | ReconstructionCheck::NotWhole { at: t } => enc.put_str(t),
                        ReconstructionCheck::DoesNotClose { component } => cell(enc, component),
                    }
                }
                Undecided::RegionOutsideMembers {
                    weighting: w,
                    certificate,
                } => {
                    weighting(enc, w);
                    enc.put_str(certificate);
                }
            }
            enc.put_str(missing_fact);
            opt(enc, closing_document.as_deref());
        }
        IdentityState::Refused { reason } => enc.put_str(reason.code()),
        IdentityState::RefusedAt { reason, at } => {
            enc.put_str(reason.code());
            enc.put_str(at);
        }
        IdentityState::NotPublished { reason } => enc.put_str(reason.code()),
    }
}

fn name_missing_fact(instance: &mut IdentityInstance, registered: &[RegisteredMissingFact]) {
    let IdentityState::Undecidable {
        missing_fact,
        closing_document,
        ..
    } = &mut instance.state
    else {
        return;
    };
    if let Some(row) = registered
        .iter()
        .find(|m| m.subject == instance.subject && m.family == instance.family)
    {
        *missing_fact = format!("{} ({})", row.text, row.evidence);
        closing_document.clone_from(&row.closing_document);
    }
}

/// `fmine assure public --issuer <issuer>`: the gate-eligible run over the registered public workbooks.
///
/// The issuer name `synthetic` is reserved for the invented workbooks: a registry under that name (for
/// example one written by [`synthetic::write_issuer_workspace`]) runs in the synthetic lane and is never
/// gate-eligible, though it goes through the same registry and digest checks.
pub fn run_public_assurance(
    profile: &IssuerProfile,
    root: &Path,
) -> Result<PublicAssuranceReport, Refusal> {
    let sources = load_registered_sources(profile, root)?;
    let lane = if profile.issuer == synthetic::SYNTHETIC_ISSUER {
        Lane::Synthetic
    } else {
        Lane::Public
    };
    Ok(assess(profile, lane, sources))
}

/// The Barrick M0 run (`GATE-BULL-01`): registry and corpus both under `root`.
pub fn run_barrick_m0_assurance(root: &Path) -> Result<PublicAssuranceReport, Refusal> {
    run_public_assurance(&sources::load_issuer_profile(root, "barrick")?, root)
}

/// `fmine assure public --synthetic`: the known-answer run over invented workbooks, with optional edits to
/// the synthetic AISC table (perturbation tests).
pub fn run_synthetic_assurance(
    edits: &[(String, fmine_io::xlsx::MinimalCell)],
) -> Result<PublicAssuranceReport, Refusal> {
    let sources = load_synthetic(synthetic::synthetic_workbooks(edits))?;
    Ok(assess(
        &synthetic::synthetic_profile(),
        Lane::Synthetic,
        sources,
    ))
}

/// Parses in-memory synthetic workbooks `(file, kind, quarter, bytes)` into loaded sources.
fn load_synthetic(
    files: Vec<(&'static str, SourceKind, &'static str, Vec<u8>)>,
) -> Result<Vec<LoadedSource>, Refusal> {
    let mut sources = Vec::new();
    for (file, kind, quarter, bytes) in files {
        let workbook = Workbook::from_bytes(file, &bytes)
            .map_err(|e| Refusal::from_code("FMINE-REF-502", format!("synthetic {file}: {e}")))?;
        let census = workbook.cell_census();
        sources.push(LoadedSource {
            file_name: file.to_string(),
            kind,
            quarter: quarter.to_string(),
            url: String::new(),
            url_basis: "synthetic".to_string(),
            bytes: bytes.len() as u64,
            workbook,
            census,
        });
    }
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aisc::{Calc, NotPublishedReason, RefusedReason, WeightedMember};
    use fmine_io::xlsx::MinimalCell;
    use std::path::PathBuf;

    fn state_of<'a>(
        r: &'a PublicAssuranceReport,
        cell: &str,
        family: IdentityFamily,
    ) -> &'a IdentityState {
        let found = r
            .instances
            .iter()
            .find(|i| i.cell == cell && i.family == family)
            .map(|i| &i.state);
        match found {
            Some(s) => s,
            None => panic!("no instance at {cell} {family:?}"),
        }
    }

    fn calc_of(r: &PublicAssuranceReport, cell: &str, family: IdentityFamily) -> Calc {
        match r
            .instances
            .iter()
            .find(|i| i.cell == cell && i.family == family)
        {
            Some(i) => i.calc,
            None => panic!("no instance at {cell} {family:?}"),
        }
    }

    /// Site F (column G) stores 100.4 and 80.4 under `#,##0` (bd-jg43.3): the ledger takes the displayed 100 and
    /// 80, so both footings foot and the three rates fall in their intervals (worked in `synthetic.rs`'s header).
    #[test]
    fn test_synthetic_g_column_scored_on_displayed_values() {
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        let footing = |printed, component_sum, k_alt| Calc::Footing {
            printed,
            component_sum,
            k_alt,
        };
        let rate = |n, k, q, r| Calc::Rate(RateMiss { n, k, q, r });
        for (cell, family, calc) in [
            ("G8", IdentityFamily::TccFooting, footing(80, 80, 3)),
            ("G11", IdentityFamily::AiscFooting, footing(100, 100, 5)),
            ("G14", IdentityFamily::CosPerOunce, rate(100, 1, 50, 2008)),
            ("G15", IdentityFamily::TccPerOunce, rate(80, 3, 50, 1608)),
            ("G16", IdentityFamily::AiscPerOunce, rate(100, 5, 50, 2008)),
        ] {
            assert_eq!(state_of(&r, cell, family).code(), "i", "{cell}");
            assert_eq!(calc_of(&r, cell, family), calc, "{cell}");
        }
        // The operands carry the displayed whole numbers, never the stored 100.4.
        let g14 = r
            .instances
            .iter()
            .find(|i| i.cell == "G14" && i.family == IdentityFamily::CosPerOunce)
            .expect("G14");
        assert!(
            g14.operands
                .iter()
                .any(|o| o.cell == "G5" && o.value == aisc::OperandValue::Int(100))
        );
    }

    /// Site G (column N) holds a General non-integer at N6: the TCC footing is refused naming the cell and the
    /// format code, while the AISC footing, which does not read N6, foots.
    #[test]
    fn test_synthetic_site_g_general_non_integer_refused() {
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        let n8 = state_of(&r, "N8", IdentityFamily::TccFooting);
        assert_eq!(
            n8,
            &IdentityState::RefusedAt {
                reason: RefusedReason::GeneralNonInteger,
                at: "N6 (General)".to_string(),
            }
        );
        assert_eq!(n8.reason_code(), Some("general_format_non_integer"));
        assert_eq!(
            n8.detail(),
            "general format with non-integer stored value: N6 (General)"
        );
        assert_eq!(state_of(&r, "N11", IdentityFamily::AiscFooting).code(), "i");
        assert_eq!(
            calc_of(&r, "N11", IdentityFamily::AiscFooting),
            Calc::Footing {
                printed: 300,
                component_sum: 300,
                k_alt: 5
            }
        );
    }

    /// Site G's ounces (N13) are under a date format: all three rates are refused naming that cell and its code.
    #[test]
    fn test_synthetic_site_g_date_format_refused() {
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        for (cell, family) in [
            ("N14", IdentityFamily::CosPerOunce),
            ("N15", IdentityFamily::TccPerOunce),
            ("N16", IdentityFamily::AiscPerOunce),
        ] {
            let s = state_of(&r, cell, family);
            assert_eq!(
                s,
                &IdentityState::RefusedAt {
                    reason: RefusedReason::UnsupportedFormat,
                    at: "N13 (mm-dd-yy)".to_string(),
                },
                "{cell}"
            );
            assert_eq!(s.reason_code(), Some("unsupported_number_format"));
            assert_eq!(calc_of(&r, cell, family), Calc::None);
        }
    }

    #[test]
    fn synthetic_lane_reproduces_every_seeded_answer() {
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        assert_eq!(r.lane, Lane::Synthetic);
        assert!(!r.gate_eligible());
        assert_eq!(r.ledger_anchor.cell_id, "CELL-SYNTHETIC");
        assert_eq!((r.aisc_tables, r.instances.len()), (1, 55));
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| r.count_state(c)).collect();
        // Columns K and L (BD-331.S3) add nine (i) and one (iv) to the 35 instances of columns B–H; column M
        // (BD-331.S2) adds four (i) and one stated (ii). On displayed values (bd-jg43.3) G's five (v) are (i), and
        // column N "Site G" adds four (v) and one (i).
        assert_eq!(totals, vec![40, 2, 1, 2, 5, 5]);

        match state_of(&r, "C8", IdentityFamily::TccFooting) {
            IdentityState::IssuerDiscrepancy(c) => {
                assert_eq!((c.printed, c.component_sum, c.residual), (350, 250, 100));
                assert_eq!(c.sign_flip_witness.as_deref(), Some("C6"));
                assert_eq!(c.line_cells, vec!["C5", "C6", "C7"]);
                assert!(!c.alternate_consistent);
            }
            other => panic!("C8 TCC footing: {other:?}"),
        }
        match state_of(&r, "D16", IdentityFamily::AiscPerOunce) {
            s @ IdentityState::Undecidable {
                why,
                missing_fact,
                closing_document,
            } => {
                // Structured (bd-sjrh.1): r 2400, N = D11 200 over k 5 lines, Q = D13 100.
                assert_eq!(
                    why,
                    &Undecided::RateOutside(RateMiss {
                        n: 200,
                        k: 5,
                        q: 100,
                        r: 2400
                    })
                );
                // 405000/199 = 2035.1758…: the upper bound rounds up (it printed 2035.175 before 2026-10-29).
                assert!(
                    s.detail().contains("[1965.174, 2035.176]"),
                    "{}",
                    s.detail()
                );
                // Site C's registered missing fact replaces the family default (BD-331.S2).
                assert_eq!(
                    missing_fact,
                    "the sustaining capital the synthetic issuer used for this rate (synthetic)"
                );
                assert_eq!(closing_document, &None);
            }
            other => panic!("D16: {other:?}"),
        }
        assert_eq!(
            state_of(&r, "E14", IdentityFamily::CosPerOunce),
            &IdentityState::Refused {
                reason: RefusedReason::NoRegisteredShare
            }
        );
        assert_eq!(state_of(&r, "E15", IdentityFamily::TccPerOunce).code(), "i");
        // Stated share 3/4: K prints 1000·¾·400/150 = 2000 (i); L prints 2200 (iv).
        let k14 = state_of(&r, "K14", IdentityFamily::CosPerOunce);
        assert_eq!(
            k14,
            &IdentityState::Consistent {
                alternate_consistent: true,
                basis: Some(ConsistentBasis::Share(StatedShare {
                    num: 3,
                    den: 4,
                    evidence: "synthetic".to_string(),
                    footnote: "Q1 2030!A30".to_string(),
                    label_cell: "Synthetic_Mine_Stats.xlsx:QTD!A5".to_string(),
                })),
            }
        );
        assert_eq!(
            k14.detail(),
            "alternate_construction_consistent=true; stated share 3/4 (synthetic); footnote Q1 2030!A30; label \
             Synthetic_Mine_Stats.xlsx:QTD!A5"
        );
        match state_of(&r, "L14", IdentityFamily::CosPerOunce) {
            s @ IdentityState::Undecidable {
                why, missing_fact, ..
            } => {
                assert!(
                    matches!(why, Undecided::ShareOutside { miss, share, s_interval }
                        if miss.r == 2200 && share.num == 3 && share.den == 4
                            && s_interval == "[0.8210, 0.8290]"),
                    "{why:?}"
                );
                assert!(
                    s.detail().contains("allow s in [0.8210, 0.8290]"),
                    "{}",
                    s.detail()
                );
                assert_eq!(missing_fact, aisc::SHARE_MISSING_FACT);
                assert!(
                    s.detail().ends_with(
                        "; missing fact: the attributable cost of sales or ounces the issuer used"
                    ),
                    "{}",
                    s.detail()
                );
            }
            other => panic!("L14: {other:?}"),
        }
        // Columns G and N: test_synthetic_g_column_scored_on_displayed_values and the two Site G tests.
        assert!(
            r.instances
                .iter()
                .filter(|i| i.column == "F")
                .all(|i| i.state.code() == "np")
        );
        match state_of(&r, "H16", IdentityFamily::AiscPerOunce) {
            IdentityState::Explained { component, .. } => {
                assert_eq!(
                    component,
                    &CitedCell {
                        file: "Synthetic_Mine_Stats.xlsx".to_string(),
                        sheet: "Minestats3".to_string(),
                        cell: "B32".to_string(),
                        value: 13,
                    }
                );
                assert_eq!(
                    component.text(),
                    "Synthetic_Mine_Stats.xlsx:Minestats3!B32=13"
                );
            }
            other => panic!("H16: {other:?}"),
        }
        assert_eq!(
            state_of(&r, "M16", IdentityFamily::AiscPerOunce).label(),
            "explained_stated"
        );
        assert_eq!(r.unregistered_headers.len(), 1);
        assert_eq!(r.unregistered_headers[0].cell, "J2");
        assert!(r.instances.iter().all(|i| i.column != "I"));
        let aisc = r
            .sources
            .iter()
            .find(|s| s.file_name == synthetic::SYNTHETIC_AISC_FILE)
            .expect("aisc");
        assert!(
            aisc.workbook
                .quarantined
                .iter()
                .any(|q| matches!(q, QuarantineItem::HiddenSheet { .. }))
        );
        assert!(
            aisc.workbook
                .quarantined
                .iter()
                .any(|q| matches!(q, QuarantineItem::FormulaErrorCell { .. }))
        );

        let json = r.to_json();
        assert!(json.starts_with(
            "{\"issuer\":\"synthetic\",\"lane\":\"synthetic\",\"gate_eligible\":false"
        ));
        assert!(json.contains("\"sign_flip_witness\":\"C6\""));
        assert!(
            // Site J (BD-331.S2) adds four (i) and one stated (ii); the hidden table is not counted. Columns G and
            // N on displayed values (bd-jg43.3): five (v) become (i), and N adds one (i) and four (v).
            json.contains("\"totals\":{\"i\":40,\"ii\":2,\"iii\":1,\"iv\":2,\"v\":5,\"np\":5}")
        );
        assert!(json.contains(
            "\"hidden_scope\":{\"scope\":\"sheet not displayed to readers\",\"counted\":false,\
             \"aisc_tables\":1,\"instances\":5,\"by_state\":{\"i\":4,\"ii\":0,\"iii\":1,\"iv\":0,\"v\":0,\"np\":0}"
        ));
        // The flags, the no-claim line and the next actions belong to the CLI envelope, not the payload.
        for field in ["\"flags\"", "\"no_claim\"", "\"next_actions\""] {
            assert!(!json.contains(field), "{field} in the payload");
        }
        assert!(json.ends_with(&format!(
            "\"anchor_id\":\"{}\"}}",
            r.ledger_anchor.anchor_id()
        )));
    }

    /// A cell changed in an input moves the state root and the affected instance's state.
    #[test]
    fn perturbing_an_input_cell_moves_the_root_and_the_state() {
        let base = run_synthetic_assurance(&[]).expect("base");
        assert_eq!(
            state_of(&base, "B8", IdentityFamily::TccFooting).code(),
            "i"
        );
        let edited = run_synthetic_assurance(&[("B8".to_string(), MinimalCell::Number(401.0))])
            .expect("edited");
        assert_ne!(
            base.ledger_anchor.state_root,
            edited.ledger_anchor.state_root
        );
        match state_of(&edited, "B8", IdentityFamily::TccFooting) {
            IdentityState::IssuerDiscrepancy(c) => assert_eq!(c.residual, 1),
            other => panic!("B8 after edit: {other:?}"),
        }
        // A change outside every table still moves the root (it is in the numeric digest).
        let far =
            run_synthetic_assurance(&[("Z99".to_string(), MinimalCell::Number(1.0))]).expect("far");
        assert_ne!(base.ledger_anchor.state_root, far.ledger_anchor.state_root);
        // And the run is deterministic.
        let again = run_synthetic_assurance(&[]).expect("again");
        assert_eq!(
            base.ledger_anchor.state_root,
            again.ledger_anchor.state_root
        );
    }

    /// A component whose labels do not match keeps the instance undecidable and says why.
    #[test]
    fn reconstruction_with_a_wrong_label_does_not_apply() {
        let sources = run_synthetic_assurance(&[]).expect("base").sources;
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions[0].label_checks = vec![
            ("A32".to_string(), "Minesite sustaining".to_string()),
            ("B3".to_string(), "2031".to_string()),
        ];
        let r = assess(&profile, Lane::Synthetic, sources);
        match state_of(&r, "H16", IdentityFamily::AiscPerOunce) {
            s @ IdentityState::Undecidable {
                why: Undecided::ReconstructionNotApplied { check, .. },
                ..
            } => {
                assert!(
                    matches!(check, ReconstructionCheck::LabelMismatch { at, expected_prefix, .. }
                        if at == "Minestats3!B3" && expected_prefix == "2031"),
                    "{check:?}"
                );
                assert!(s.detail().contains("B3"), "{}", s.detail());
            }
            other => panic!("H16: {other:?}"),
        }
    }

    /// The registry is read from the workspace; the corpus is absent. Every registered file is named, and the
    /// count comes from the registry, not from a literal.
    #[test]
    fn missing_corpus_refuses_naming_every_registered_file() {
        let profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let n = profile.sources.len();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("no-such-workspace");
        let refusal = run_public_assurance(&profile, &root).expect_err("must refuse");
        assert_eq!((refusal.code, refusal.exit_code), ("FMINE-REF-502", 8));
        assert!(
            refusal
                .what
                .contains(&format!("{n} of {n} registered barrick workbooks unusable")),
            "{}",
            refusal.what
        );
        for src in &profile.sources {
            assert!(
                refusal.what.contains(&src.file_name),
                "{} not named",
                src.file_name
            );
        }
    }

    /// A row added to a copy of the registry is picked up with no code change: the refusal count follows the
    /// registry, and the added file is named.
    #[test]
    fn test_refusal_count_follows_registry_rows() {
        let ws = std::env::temp_dir().join(format!("fmine-public-rows-{}", std::process::id()));
        let reg_dir = ws.join("registries").join("sources");
        std::fs::create_dir_all(&reg_dir).expect("temp registry dir");
        let text = std::fs::read_to_string(sources::registry_path(&workspace_root(), "barrick"))
            .expect("read");
        let base = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .sources
            .len();
        let added = "\n[[source]]\nfile = \"Barrick_Q3_2026_Mine_Stats.xlsx\"\nkind = \"mine_stats\"\n\
                     quarter = \"2026Q3\"\npath = \"2026/q3/Barrick_Q3_2026_Mine_Stats.xlsx\"\n\
                     url_basis = \"W1 naming rule\"\nbytes = 1\nsha256 = \"0000000000000000000000000000000000000000000000000000000000000001\"\n";
        std::fs::write(reg_dir.join("barrick.toml"), format!("{text}{added}")).expect("write");
        let profile = load_issuer_profile(&ws, "barrick").expect("extended registry");
        assert_eq!(profile.sources.len(), base + 1);
        let refusal = run_public_assurance(&profile, &ws).expect_err("no corpus");
        let n = base + 1;
        assert!(
            refusal
                .what
                .contains(&format!("{n} of {n} registered barrick workbooks unusable"))
        );
        assert!(refusal.what.contains("Barrick_Q3_2026_Mine_Stats.xlsx"));
        // `--through 2026Q2` drops the added row again.
        assert_eq!(
            profile.through("2026Q2").expect("bound").sources.len(),
            base
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// The synthetic workbooks with Site H's `QTD` label replaced, assessed under `profile`.
    fn synthetic_run_with(
        profile: &IssuerProfile,
        edits: &[(String, MinimalCell)],
        site_h_label: &str,
    ) -> PublicAssuranceReport {
        let mut files = synthetic::synthetic_workbooks(edits);
        for f in &mut files {
            if f.0 == synthetic::SYNTHETIC_STATS_FILE {
                f.3 = synthetic::synthetic_stats_workbook(site_h_label);
            }
        }
        let sources = load_synthetic(files).expect("synthetic workbooks parse");
        assess(profile, Lane::Synthetic, sources)
    }

    const NO_FOOTNOTE: IdentityState = IdentityState::Refused {
        reason: RefusedReason::FootnoteNotFound,
    };

    /// BD-331.S3: a registered share the same-release label does not print is (v), naming the cell.
    #[test]
    fn test_share_label_mismatch_gives_v() {
        let profile = synthetic::synthetic_profile();
        let r = synthetic_run_with(&profile, &[], "Site H (80%)1");
        assert_eq!(
            state_of(&r, "K14", IdentityFamily::CosPerOunce),
            &IdentityState::RefusedAt {
                reason: RefusedReason::ShareNotConfirmed,
                at: "Synthetic_Mine_Stats.xlsx: QTD!A5 prints 80%, registered 3/4".to_string(),
            }
        );
        // Site I's label still prints 75%, so its rate is still checked.
        assert_eq!(
            state_of(&r, "L14", IdentityFamily::CosPerOunce).code(),
            "iv"
        );
        let r = synthetic_run_with(&profile, &[], "Site Hx (75%)");
        assert_eq!(
            state_of(&r, "K14", IdentityFamily::CosPerOunce).detail(),
            "registered share not confirmed by label: Synthetic_Mine_Stats.xlsx: no column-A label \
             'Site H (x%)' on QTD"
        );
    }

    /// BD-331.S3: the footnote is found by what it says, whatever its letter or row.
    #[test]
    fn test_footnote_located_by_text_not_letter() {
        let other_note = (
            String::from("A30"),
            MinimalCell::Text("h.  Ounces sold are in thousands.".to_string()),
        );
        let moved = [
            other_note.clone(),
            (
                String::from("A41"),
                MinimalCell::Text(synthetic::SYNTHETIC_SHARE_FOOTNOTE.replacen("h.", "k.", 1)),
            ),
        ];
        let r = run_synthetic_assurance(&moved).expect("synthetic");
        match state_of(&r, "K14", IdentityFamily::CosPerOunce) {
            IdentityState::Consistent {
                basis: Some(ConsistentBasis::Share(b)),
                ..
            } => {
                assert_eq!(b.footnote, "Q1 2030!A41");
            }
            other => panic!("K14: {other:?}"),
        }
        // Planted negative: letter "h" stays at A30, but nothing states the attributable basis.
        let r = run_synthetic_assurance(&[other_note]).expect("synthetic");
        for cell in ["K14", "L14"] {
            assert_eq!(
                state_of(&r, cell, IdentityFamily::CosPerOunce),
                &NO_FOOTNOTE
            );
        }
    }

    /// BD-331.S3: a region's cost-of-sales rate stays (v), now for the reason that applies.
    #[test]
    fn test_regions_keep_v_new_reason() {
        let nci = [(String::from("H7"), MinimalCell::Number(-10.0))];
        let r = run_synthetic_assurance(&nci).expect("synthetic");
        assert_eq!(
            state_of(&r, "H14", IdentityFamily::CosPerOunce),
            &IdentityState::Refused {
                reason: RefusedReason::RegionalNumerator
            }
        );
        // Unregistered as a region, the same column is refused for having no share instead.
        let mut profile = synthetic::synthetic_profile();
        profile.regions.clear();
        let r = synthetic_run_with(&profile, &nci, "Site H (75%)1");
        assert_eq!(
            state_of(&r, "H14", IdentityFamily::CosPerOunce),
            &IdentityState::Refused {
                reason: RefusedReason::NoRegisteredShare
            }
        );
        assert!(
            r.instances
                .iter()
                .all(|i| i.state.detail() != "attributable share needed")
        );
    }

    /// The synthetic profile with Region North (column H) registered as Site H (K) and Site I (L), the members
    /// first holding from `first`.
    fn region_profile(first: &str) -> IssuerProfile {
        let mut profile = synthetic::synthetic_profile();
        profile.region_members = ["Site H", "Site I"]
            .iter()
            .map(|m| crate::sources::RegionMemberRow {
                region: "Region North".to_string(),
                member: (*m).to_string(),
                first: first.to_string(),
                last: None,
            })
            .collect();
        profile
    }

    /// Region North with a non-zero NCI line (H7), ounces footing to its members' (H13 = 150 + 150) and rate
    /// `h14`, then `more` on top.
    fn region_run(
        profile: &IssuerProfile,
        h14: f64,
        more: &[(&str, MinimalCell)],
    ) -> PublicAssuranceReport {
        let mut edits = vec![
            (String::from("H7"), MinimalCell::Number(-10.0)),
            (String::from("H13"), MinimalCell::Number(300.0)),
            (String::from("H14"), MinimalCell::Number(h14)),
        ];
        edits.extend(more.iter().map(|(r, c)| ((*r).to_string(), c.clone())));
        synthetic_run_with(profile, &edits, "Site H (75%)1")
    }

    fn weighed(subject: &str, col: &str, rate: i64, ounces: i64) -> WeightedMember {
        WeightedMember {
            subject: subject.to_string(),
            rate_cell: format!("{col}14"),
            rate,
            ounces_cell: format!("{col}13"),
            ounces,
        }
    }

    /// BD-331.S4: a region's rate is weighed against its registered members. Site H prints 2000 and Site I 2200,
    /// each on 150 ounces, so with every printed value ± ½ the weighted mean lies in [2099⅙, 2100⅚]: Region
    /// North at 2100 is (i), proved by the exact decision, not `relaxation_only`; at 2300 or 1900 the McCormick
    /// relaxation refutes it with a checked certificate, (iv), naming what public data leaves open.
    #[test]
    fn test_region_weighed_against_members() {
        let profile = region_profile("2030Q1");
        let weighting = MemberWeighting {
            used: vec![
                weighed("Site H", "K", 2000, 150),
                weighed("Site I", "L", 2200, 150),
            ],
            excluded: Vec::new(),
            absent: Vec::new(),
        };
        let r = region_run(&profile, 2100.0, &[]);
        let h14 = state_of(&r, "H14", IdentityFamily::CosPerOunce);
        assert_eq!(
            h14,
            &IdentityState::Consistent {
                alternate_consistent: true,
                basis: Some(ConsistentBasis::MembersExact(weighting.clone())),
            }
        );
        assert_eq!(
            h14.detail(),
            "alternate_construction_consistent=true; exact weighted mean; weighted by ounces sold over Site H \
             K14/K13, Site I L14/L13"
        );
        assert_eq!(r.relaxation_only(), 0);
        assert!(r.to_json().contains("\"relaxation_only\":0,"));
        let region = r
            .instances
            .iter()
            .find(|i| i.cell == "H14" && i.family == IdentityFamily::CosPerOunce)
            .expect("H14");
        let members: Vec<(&str, OperandRole)> = region
            .operands
            .iter()
            .filter(|o| matches!(o.role, OperandRole::MemberRate | OperandRole::MemberOunces))
            .map(|o| (o.cell.as_str(), o.role))
            .collect();
        assert_eq!(
            members,
            [
                ("K14", OperandRole::MemberRate),
                ("K13", OperandRole::MemberOunces),
                ("L14", OperandRole::MemberRate),
                ("L13", OperandRole::MemberOunces),
            ]
        );
        for far in [2300.0, 1900.0] {
            let r = region_run(&profile, far, &[]);
            match state_of(&r, "H14", IdentityFamily::CosPerOunce) {
                IdentityState::Undecidable {
                    why:
                        Undecided::RegionOutsideMembers {
                            weighting: w,
                            certificate,
                        },
                    missing_fact,
                    closing_document: None,
                } => {
                    assert_eq!(w, &weighting);
                    assert_eq!(certificate.len(), 64, "{certificate}");
                    assert!(certificate.bytes().all(|b| b.is_ascii_hexdigit()));
                    assert_eq!(missing_fact, crate::aisc::REGION_MISSING_FACT);
                }
                other => panic!("H14 at {far}: {other:?}"),
            }
            assert_eq!(r.relaxation_only(), 0);
        }
        // At the edges the relaxation refutes exactly where the exact vertex bound does: 2099 ± ½ and 2101 ± ½
        // meet [2099⅙, 2100⅚]; 2098 ± ½ and 2102 ± ½ do not.
        for (edge, code) in [(2098.0, "iv"), (2099.0, "i"), (2101.0, "i"), (2102.0, "iv")] {
            let r = region_run(&profile, edge, &[]);
            assert_eq!(
                state_of(&r, "H14", IdentityFamily::CosPerOunce).code(),
                code,
                "{edge}"
            );
        }
    }

    /// BD-331.S4: the exact decision's bounds are closed. Site H prints 1000 and Site I 2000, each on 1 ounce, so
    /// with every value ± ½ the members' weighted means span exactly [1249½, 1750½], reached only with both
    /// weights and both rates at a corner of their boxes. 1249 ± ½ and 1751 ± ½ touch that range at one point and
    /// are (i), proved; 1248 and 1752 are (iv), refuted by the McCormick relaxation.
    #[test]
    fn test_region_exact_bounds_are_closed() {
        let profile = region_profile("2030Q1");
        let light = [
            ("H13", MinimalCell::Number(2.0)),
            ("K13", MinimalCell::Number(1.0)),
            ("L13", MinimalCell::Number(1.0)),
            ("K14", MinimalCell::Number(1000.0)),
            ("L14", MinimalCell::Number(2000.0)),
        ];
        for rate in [1249.0, 1751.0] {
            let r = region_run(&profile, rate, &light);
            assert!(
                matches!(
                    state_of(&r, "H14", IdentityFamily::CosPerOunce),
                    IdentityState::Consistent {
                        basis: Some(ConsistentBasis::MembersExact(_)),
                        ..
                    }
                ),
                "{rate}"
            );
            assert_eq!(r.relaxation_only(), 0);
        }
        for rate in [1248.0, 1752.0] {
            let r = region_run(&profile, rate, &light);
            match state_of(&r, "H14", IdentityFamily::CosPerOunce) {
                IdentityState::Undecidable {
                    why: Undecided::RegionOutsideMembers { certificate, .. },
                    ..
                } => assert_eq!(certificate.len(), 64, "{rate}: {certificate}"),
                other => panic!("{rate}: {other:?}"),
            }
        }
    }

    /// BD-331.S4: members that sold nothing (rate and ounces both zero, or both placeholders) are left out by
    /// name, and a registered member the sheet does not print is named as absent; the rest are weighed.
    #[test]
    fn test_region_excludes_and_names_absent() {
        let profile = region_profile("2030Q1");
        let zero = MinimalCell::Number(0.0);
        let place = MinimalCell::Text("-".to_string());
        for (what, l) in [("zero", zero), ("placeholder", place)] {
            let r = region_run(
                &profile,
                2000.0,
                &[
                    ("H13", MinimalCell::Number(150.0)),
                    ("L13", l.clone()),
                    ("L14", l),
                ],
            );
            assert_eq!(
                state_of(&r, "H14", IdentityFamily::CosPerOunce),
                &IdentityState::Consistent {
                    alternate_consistent: true,
                    basis: Some(ConsistentBasis::MembersExact(MemberWeighting {
                        used: vec![weighed("Site H", "K", 2000, 150)],
                        excluded: vec!["Site I".to_string()],
                        absent: Vec::new(),
                    })),
                },
                "{what}"
            );
        }
        let mut with_absent = region_profile("2030Q1");
        with_absent.subjects.push("Site Z".to_string());
        with_absent
            .region_members
            .push(crate::sources::RegionMemberRow {
                region: "Region North".to_string(),
                member: "Site Z".to_string(),
                first: "2030Q1".to_string(),
                last: None,
            });
        let r = region_run(&with_absent, 2100.0, &[]);
        match state_of(&r, "H14", IdentityFamily::CosPerOunce) {
            IdentityState::Consistent {
                basis: Some(ConsistentBasis::MembersExact(w)),
                ..
            } => assert_eq!(w.absent, ["Site Z"]),
            other => panic!("{other:?}"),
        }
        assert!(
            state_of(&r, "H14", IdentityFamily::CosPerOunce)
                .detail()
                .ends_with("; not printed: Site Z")
        );
    }

    /// BD-331.S4 refusals, each naming its cell where it has one: the region's ounces not footing to its
    /// members' (H13 = 100 against 150 + 150); a member's rate beside zero ounces (the ounces), a text rate (the
    /// rate), a zero beside a placeholder (the rate), negative ounces; a member printed twice (the second
    /// header); and no member left to weigh. Unchanged: the footnote is checked first, and a region with no
    /// members registered for the release keeps the regional-numerator refusal.
    #[test]
    fn test_region_refusals() {
        use RefusedReason as R;
        let profile = region_profile("2030Q1");
        let at = |reason, at: &str| IdentityState::RefusedAt {
            reason,
            at: at.to_string(),
        };
        let num = MinimalCell::Number;
        let text = |s: &str| MinimalCell::Text(s.to_string());
        type Edits = Vec<(&'static str, MinimalCell)>;
        let cases: [(&str, Edits, IdentityState); 8] = [
            (
                "ounces do not foot",
                vec![("H13", num(100.0))],
                at(R::RegionOuncesDoNotFoot, "H13"),
            ),
            (
                "rate beside zero ounces",
                vec![("L13", num(0.0))],
                at(R::RegionMemberOperand, "L13"),
            ),
            (
                "text rate",
                vec![("L14", text("x"))],
                at(R::RegionMemberOperand, "L14"),
            ),
            (
                "zero beside a placeholder",
                vec![("L13", num(0.0)), ("L14", text("-"))],
                at(R::RegionMemberOperand, "L14"),
            ),
            (
                "negative ounces",
                vec![("L13", num(-150.0))],
                at(R::RegionMemberOperand, "L13"),
            ),
            (
                "member printed twice",
                vec![("J2", text("Site I"))],
                at(R::RegionMemberOperand, "L2"),
            ),
            (
                "no member sold anything",
                vec![
                    ("K13", num(0.0)),
                    ("K14", num(0.0)),
                    ("L13", num(0.0)),
                    ("L14", num(0.0)),
                ],
                IdentityState::Refused {
                    reason: R::RegionNoMembers,
                },
            ),
            ("footnote first", vec![("A30", text("x"))], NO_FOOTNOTE),
        ];
        for (what, more, want) in cases {
            let r = region_run(&profile, 2100.0, &more);
            assert_eq!(
                state_of(&r, "H14", IdentityFamily::CosPerOunce),
                &want,
                "{what}"
            );
        }
        let later = region_profile("2030Q2");
        assert_eq!(
            state_of(
                &region_run(&later, 2100.0, &[]),
                "H14",
                IdentityFamily::CosPerOunce
            ),
            &IdentityState::Refused {
                reason: R::RegionalNumerator
            }
        );
    }

    /// BD-331.S3 mutation: without its registered share, Site H's rate is (v), not consistent.
    #[test]
    fn test_drop_registered_share_yields_v() {
        let mut profile = synthetic::synthetic_profile();
        profile.shares.retain(|s| s.subject != "Site H");
        let r = synthetic_run_with(&profile, &[], "Site H (75%)1");
        assert_eq!(
            state_of(&r, "K14", IdentityFamily::CosPerOunce),
            &IdentityState::Refused {
                reason: RefusedReason::NoRegisteredShare
            }
        );
        assert_eq!(
            state_of(&r, "L14", IdentityFamily::CosPerOunce).code(),
            "iv"
        );
    }

    fn one_cell<'a>(r: &'a PublicAssuranceReport, column: &str) -> &'a crate::aisc::OneCellCheck {
        let found: Vec<_> = r.one_cell.iter().filter(|c| c.column == column).collect();
        assert_eq!(found.len(), 1, "one check for column {column}");
        found[0]
    }

    /// bd-prmn.8: K.4's one-cell check covers every registered-subject column once, at the column's basis, and
    /// agrees with the ledger's cost-of-sales-per-ounce states; it is reported, not counted.
    #[test]
    fn test_one_cell_check_on_synthetic_columns() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let states: Vec<(&str, &str)> = r
            .one_cell
            .iter()
            .map(|c| (c.column.as_str(), c.state()))
            .collect();
        // B–H and K–N are registered subjects; I ("Footnote") and J ("Mystery Mine") are not. G passes on its
        // displayed values (bd-jg43.3); N's ounces are under a date format.
        assert_eq!(
            states,
            [
                ("B", "pass"),
                ("C", "pass"),
                ("D", "pass"),
                ("E", "refused"),
                ("F", "refused"),
                ("G", "pass"),
                ("H", "pass"),
                ("K", "pass"),
                ("L", "fail"),
                ("M", "pass"),
                ("N", "refused"),
            ]
        );
        // Site D's operands are valid, so its 100% check is still reported: 1000·1000/400 = 2500, printed 2500.
        for (column, reason, at_full) in [
            ("E", "no registered share", Some(true)),
            ("F", "all-zero column", None),
            ("N", "unsupported number format", None),
        ] {
            assert_eq!(
                one_cell(&r, column).outcome,
                OneCellOutcome::Refused {
                    reason,
                    at_full,
                    at_share: None
                },
                "{column}"
            );
        }
        assert!(one_cell(&r, "H").region);
        // Site H: 1000·¾·400/150 = 2000 at its stated share; at 100% the rate would be 2666.67.
        assert_eq!(
            one_cell(&r, "K").outcome,
            OneCellOutcome::Checked {
                basis: (3, 4),
                pass: true,
                at_full: false,
                at_share: Some(true),
                // [1000·3·799/(4·4001), 1000·3·801/(4·3999)] = [149.7750…, 150.2250…].
                implied_oz: ("149.775".to_string(), "150.226".to_string()),
            }
        );
        // Site I prints 2200: it fails at both bases; ounces in [1000·3·799/(4·4401), 1000·3·801/(4·4399)]
        // = [136.1622…, 136.5651…] would close it, against the printed 150.
        assert_eq!(
            one_cell(&r, "L").outcome,
            OneCellOutcome::Checked {
                basis: (3, 4),
                pass: false,
                at_full: false,
                at_share: Some(false),
                implied_oz: ("136.162".to_string(), "136.566".to_string()),
            }
        );
        // Every failing column is a (iv) cost-of-sales-per-ounce instance of the ledger, and the reverse.
        let fails: Vec<String> = r
            .one_cell
            .iter()
            .filter(|c| c.state() == "fail")
            .map(|c| format!("{}!{}", c.sheet, c.column))
            .collect();
        let undecidable: Vec<String> = r
            .instances
            .iter()
            .filter(|i| i.family == IdentityFamily::CosPerOunce && i.state.code() == "iv")
            .map(|i| format!("{}!{}", i.sheet, i.column))
            .collect();
        assert_eq!(fails, undecidable);
        let json = r.to_json();
        assert!(
            json.contains(
                "\"columns\":11,\"pass\":7,\"fail\":1,\"refused\":3,\"failing\":[{\"file\":\"Synthetic_AISC.xlsx\",\
                 \"sheet\":\"Q1 2030\",\"column\":\"L\",\"subject\":\"Site I\",\"basis\":\"3/4\",\"at_full\":false,\
                 \"at_share\":false,\"implied_oz\":[\"136.162\",\"136.566\"]}]},\"hidden_scope\":"
            ),
            "{json}"
        );
    }

    /// bd-prmn.8: a column registered at the wrong share fails at that basis and K.4's verdict names it a basis
    /// rule; with the right share and Site I's miss counted as a known item, the verdict is site-specific.
    #[test]
    fn test_one_cell_wrong_share_is_basis_rule() {
        let site_i = |c: &crate::aisc::OneCellCheck| c.subject == "Site I";
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let v = crate::aisc::one_cell_verdict(&r.one_cell, &site_i);
        assert_eq!(
            (v.fail, v.k4_fail, v.other_fail, v.verdict),
            (1, 1, 0, "site-specific")
        );

        let mut profile = synthetic::synthetic_profile();
        for s in profile.shares.iter_mut().filter(|s| s.subject == "Site H") {
            (s.num, s.den) = (3, 5);
        }
        let r = synthetic_run_with(&profile, &[], "Site H (60%)1");
        match &one_cell(&r, "K").outcome {
            OneCellOutcome::Checked {
                basis: (3, 5),
                pass: false,
                at_full: false,
                at_share: Some(false),
                ..
            } => {}
            other => panic!("K: {other:?}"),
        }
        let v = crate::aisc::one_cell_verdict(&r.one_cell, &site_i);
        assert_eq!(
            (v.fail, v.k4_fail, v.other_fail, v.verdict),
            (2, 1, 1, "basis rule")
        );
        assert_eq!(
            v.pattern,
            [(("Site H".to_string(), (3, 5), Some(false)), 1)]
        );
    }

    /// An instance's operands as (role, `file:sheet!cell`, value).
    fn operands_of(
        r: &PublicAssuranceReport,
        cell: &str,
    ) -> Vec<(OperandRole, String, OperandValue)> {
        let Some(i) = r.instances.iter().find(|i| i.cell == cell) else {
            panic!("no instance at {cell}");
        };
        i.operands
            .iter()
            .map(|o| {
                (
                    o.role,
                    format!("{}:{}!{}", o.file, o.sheet, o.cell),
                    o.value.clone(),
                )
            })
            .collect()
    }

    fn aisc_at(cell: &str) -> String {
        format!(
            "{}:{}!{cell}",
            synthetic::SYNTHETIC_AISC_FILE,
            synthetic::SYNTHETIC_TABLE_SHEET
        )
    }

    /// The synthetic AISC workbook's sha256, from its bytes.
    fn synthetic_aisc_sha() -> String {
        synthetic::synthetic_workbooks(&[])
            .into_iter()
            .find(|w| w.0 == synthetic::SYNTHETIC_AISC_FILE)
            .map(|w| fmine_core::hash::sha256(&w.3).to_hex())
            .expect("synthetic AISC workbook")
    }

    /// bd-sjrh.1 part 1: every refusal and not-published reason has a unique snake_case code and a reason text
    /// with no digits, and so does each next action's text.
    #[test]
    fn test_reason_codes_and_why_have_no_digits() {
        let pairs: Vec<(&str, &str)> = RefusedReason::ALL
            .iter()
            .map(|r| (r.code(), r.why()))
            .chain(NotPublishedReason::ALL.iter().map(|r| (r.code(), r.why())))
            .collect();
        // 16 since BD-331.S6 added `exact_kernel_indeterminate`; 18 since bd-jg43.3 split the unrounded refusal
        // into three by the operand's format; 21 since BD-331.S4 added the three regional refusals.
        assert_eq!(pairs.len(), 21);
        for (code, why) in &pairs {
            assert!(
                !code.is_empty() && code.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                "{code}"
            );
            assert!(!why.bytes().any(|b| b.is_ascii_digit()), "{why}");
        }
        let codes: std::collections::BTreeSet<&str> = pairs.iter().map(|p| p.0).collect();
        assert_eq!(codes.len(), pairs.len());
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        for (_, why) in r.next_actions() {
            assert!(!why.bytes().any(|b| b.is_ascii_digit()), "{why}");
        }
        // Each state's code reaches the payload: E14's refusal, N8's, N14's and D16's.
        let json = r.to_json();
        for code in [
            "\"reason_code\":\"no_registered_share\"",
            "\"reason_code\":\"general_format_non_integer\"",
            "\"reason_code\":\"unsupported_number_format\"",
            "\"reason_code\":\"rate_outside_interval\"",
            "\"reason_code\":\"all_zero_column\"",
        ] {
            assert!(json.contains(code), "{code}");
        }
    }

    /// bd-sjrh.1 part 4, frame v4: the root is deterministic, and moves when one cell does (B8 400 → 401).
    #[test]
    fn test_frame_v4_root_deterministic_and_moves_on_cell_edit() {
        let root = |edits: &[(String, MinimalCell)]| {
            run_synthetic_assurance(edits)
                .expect("synthetic run")
                .ledger_anchor
                .state_root
                .to_hex()
        };
        let base = root(&[]);
        assert_eq!(base, root(&[]));
        assert_ne!(
            base,
            root(&[("B8".to_string(), MinimalCell::Number(401.0))])
        );
    }

    /// bd-sjrh.1 part 4: the frame binds a state's fields. Two (iv) states that differ only in a reconstruction
    /// check's quoted text bind differently, an absent closing document binds apart from an empty one, and an input
    /// that moves without changing D16's state code still moves its bound fields (q).
    #[test]
    fn test_frame_v4_binds_fields_and_operands() {
        let bytes = |s: &IdentityState| {
            let mut enc = Encoder::new_frame(*b"TEST", 1);
            encode_state(&mut enc, s);
            enc.finish()
        };
        let mismatch = |found: &str| IdentityState::Undecidable {
            why: Undecided::ReconstructionNotApplied {
                evidence_id: "E1".to_string(),
                check: ReconstructionCheck::LabelMismatch {
                    at: "S!A1".to_string(),
                    found: found.to_string(),
                    expected_prefix: "x".to_string(),
                },
            },
            missing_fact: "m".to_string(),
            closing_document: None,
        };
        assert_eq!(bytes(&mismatch("a")), bytes(&mismatch("a")));
        assert_ne!(bytes(&mismatch("a")), bytes(&mismatch("b")));
        // None and Some("") are told apart by their tags.
        let with_doc = |d: Option<&str>| match mismatch("a") {
            IdentityState::Undecidable {
                why, missing_fact, ..
            } => IdentityState::Undecidable {
                why,
                missing_fact,
                closing_document: d.map(str::to_string),
            },
            other => other,
        };
        assert_ne!(bytes(&with_doc(None)), bytes(&with_doc(Some(""))));
        // D13 100 → 101 keeps D16 (iv) but changes q in its fields and its operand.
        let d16 = |edits: &[(String, MinimalCell)]| {
            run_synthetic_assurance(edits)
                .expect("synthetic run")
                .instances
                .into_iter()
                .find(|i| i.cell == "D16" && i.family == IdentityFamily::AiscPerOunce)
                .expect("D16")
        };
        let (a, b) = (
            d16(&[]),
            d16(&[("D13".to_string(), MinimalCell::Number(101.0))]),
        );
        assert_eq!((a.state.code(), b.state.code()), ("iv", "iv"));
        assert_ne!(bytes(&a.state), bytes(&b.state));
    }

    /// bd-sjrh.1 part 3: C8 (hand-read from `synthetic.rs`: C5 300, C6 −50, C7 0, C8 350) renders as three cited
    /// numbers: 350 as read, the sum 250 of C5:C7 and the residual 100, in its certificate and its `cited` record.
    #[test]
    fn test_c8_footing_renders_three_cited_numbers() {
        let json = run_synthetic_assurance(&[])
            .expect("synthetic run")
            .to_json();
        let sha = synthetic_aisc_sha();
        let at = |c: &str| format!("\"Synthetic_AISC.xlsx:Q1 2030!{c}\"");
        let x = |v: i64, c: &str| format!("\"v\":{v},\"at\":{},\"sha256\":\"{sha}\"}}", at(c));
        let rule = |r: &str| format!("\"rule\":\"{r}\",\"rules_version\":\"m0-ledger/1\"");
        let lines = format!("{},{},{}", at("C5"), at("C6"), at("C7"));
        let sum = format!(
            "\"v\":250,{},\"in\":[{lines}]}}",
            rule("aisc.footing.component_sum")
        );
        let residual = format!(
            "\"v\":100,{},\"in\":[{},{lines}]}}",
            rule("aisc.footing.residual"),
            at("C8")
        );
        let certificate = format!(
            "\"certificate\":{{\"printed\":{{{},\"component_sum\":{{{sum},\"residual\":{{{residual},\
             \"total_cell\":\"C8\",\"line_cells\":[\"C5\",\"C6\",\"C7\"],\"sign_flip_witness\":\"C6\"",
            x(350, "C8")
        );
        assert!(json.contains(&certificate), "{certificate}");
        let cited = format!(
            "\"cited\":{{\"operands\":[{{\"role\":\"total\",{},{{\"role\":\"line\",{},{{\"role\":\"line\",{},\
             {{\"role\":\"line\",{}],\"derived\":[{{\"name\":\"component_sum\",{sum},{{\"name\":\"residual\",\
             {residual}]}}",
            x(350, "C8"),
            x(300, "C5"),
            x(-50, "C6"),
            x(0, "C7")
        );
        assert!(json.contains(&cited), "{cited}");
    }

    /// bd-sjrh.1 part 3: D16 prints 2400 against N = D11 200 over k = 5 numeric lines (D5, D6, D7, D9, D10) and
    /// Q = D13 100. Hand-derived: 1000·197.5/100.5 = 1965.1741… floors to 1965.174, and 1000·202.5/99.5 =
    /// 2035.1758… ceils to 2035.176.
    #[test]
    fn test_d16_rate_range_cited() {
        let json = run_synthetic_assurance(&[])
            .expect("synthetic run")
            .to_json();
        let at = |c: &str| format!("\"Synthetic_AISC.xlsx:Q1 2030!{c}\"");
        let rule = |r: &str| format!("\"rule\":\"{r}\",\"rules_version\":\"m0-ledger/1\"");
        let counted: Vec<String> = ["D5", "D6", "D7", "D9", "D10"].map(at).to_vec();
        let range_in = format!("[{},{},{}]", at("D13"), at("D11"), counted.join(","));
        let want = format!(
            "\"derived\":[{{\"name\":\"k\",\"v\":5,{},\"in\":[{}]}},\
             {{\"name\":\"range_lo\",\"v\":1965.174,{},\"in\":{range_in}}},\
             {{\"name\":\"range_hi\",\"v\":2035.176,{},\"in\":{range_in}}}]",
            rule("ledger.count"),
            counted.join(","),
            rule("aisc.rate_interval"),
            rule("aisc.rate_interval"),
        );
        assert!(json.contains(&want), "{want}");
        // One record per counted instance, each with operands.
        let (_, records) = json.split_once("\"instances\":[").expect("instances array");
        let records = records.split_once("],\"one_cell_cos\"").expect("end").0;
        assert_eq!(records.matches("\"cited\":{\"operands\":[{").count(), 55);
        assert_eq!(records.matches("{\"at\":").count(), 55);
    }

    /// bd-sjrh.1: C8's footing is computed from its total and block 1 (hand-read from `synthetic.rs`).
    #[test]
    fn test_c8_footing_operands() {
        use OperandRole as R;
        use OperandValue::Int;
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        assert_eq!(
            operands_of(&r, "C8"),
            vec![
                (R::Total, aisc_at("C8"), Int(350)),
                (R::Line, aisc_at("C5"), Int(300)),
                (R::Line, aisc_at("C6"), Int(-50)),
                (R::Line, aisc_at("C7"), Int(0)),
            ]
        );
        // The AISC footing adds the TCC line and block 2, and counts block 1 for its alternate rule.
        let c11: Vec<(OperandRole, String)> = operands_of(&r, "C11")
            .into_iter()
            .map(|o| (o.0, o.1))
            .collect();
        let want: Vec<(OperandRole, String)> = [
            (R::Total, "C11"),
            (R::Line, "C8"),
            (R::Counted, "C5"),
            (R::Counted, "C6"),
            (R::Counted, "C7"),
            (R::Line, "C9"),
            (R::Line, "C10"),
        ]
        .iter()
        .map(|(role, c)| (*role, aisc_at(c)))
        .collect();
        assert_eq!(c11, want);
    }

    /// bd-sjrh.1: Site D's cost of sales per ounce is refused for its NCI line, after reading the footnote. Site A
    /// has a zero NCI line, so its footnote is never read; Site H's confirmed share adds the `QTD` label.
    #[test]
    fn test_cos_refusal_operands_name_nci_row() {
        use OperandRole as R;
        use OperandValue::{Int, Token};
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        let footnote = (
            R::LabelCheck,
            aisc_at("A30"),
            Token(synthetic::SYNTHETIC_SHARE_FOOTNOTE.to_string()),
        );
        assert_eq!(
            operands_of(&r, "E14"),
            vec![
                (R::Rate, aisc_at("E14"), Int(2500)),
                (R::Ounces, aisc_at("E13"), Int(400)),
                (R::Numerator, aisc_at("E5"), Int(1000)),
                (R::NciLine, aisc_at("E7"), Int(-160)),
                footnote.clone(),
            ]
        );
        assert_eq!(
            operands_of(&r, "B14"),
            vec![
                (R::Rate, aisc_at("B14"), Int(1250)),
                (R::Ounces, aisc_at("B13"), Int(400)),
                (R::Numerator, aisc_at("B5"), Int(500)),
                (R::NciLine, aisc_at("B7"), Int(0)),
            ]
        );
        let k14 = operands_of(&r, "K14");
        assert_eq!(k14[4], footnote);
        assert_eq!(
            k14[5],
            (
                R::LabelCheck,
                format!("{}:QTD!A5", synthetic::SYNTHETIC_STATS_FILE),
                Token("Site H (75%)1".to_string())
            )
        );
        assert_eq!(k14.len(), 6);
    }

    /// bd-sjrh.1: H16 is explained by `Minestats3!B32` = 13 after four label checks; M16 also reads its stated
    /// quote, `A31`. F's all-zero column cites every used cell (11).
    #[test]
    fn test_explained_component_cited_cell_h16() {
        use OperandRole as R;
        use OperandValue::{Int, Token};
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        let stats = |c: &str| format!("{}:Minestats3!{c}", synthetic::SYNTHETIC_STATS_FILE);
        let rec = [
            (
                R::LabelCheck,
                stats("A32"),
                Token("Minesite sustaining3".to_string()),
            ),
            (
                R::LabelCheck,
                stats("B2"),
                Token("Site G (60%)1".to_string()),
            ),
            (
                R::LabelCheck,
                stats("A3"),
                Token("For the three months ended March 31,".to_string()),
            ),
            (R::LabelCheck, stats("B3"), Token("2030".to_string())),
            (R::Component, stats("B32"), Int(13)),
        ];
        let h16 = operands_of(&r, "H16");
        // Rate, ounces, numerator, then the five lines of blocks 1 and 2, then the reconstruction.
        assert_eq!(h16.len(), 3 + 5 + 5);
        assert_eq!(
            h16[..3].iter().map(|o| o.0).collect::<Vec<_>>(),
            [R::Rate, R::Ounces, R::Numerator]
        );
        assert_eq!(h16[8..], rec[..]);
        let m16 = operands_of(&r, "M16");
        assert_eq!(m16[8].0, R::LabelCheck);
        assert_eq!(m16[8].1, aisc_at("A31"));
        assert_eq!(m16[9..], rec[..]);
        for cell in ["F8", "F11", "F14", "F15", "F16"] {
            assert_eq!(operands_of(&r, cell).len(), 11, "{cell}");
        }
    }

    /// Cells that place a table or a sheet rather than feed an instance: the AISC sheet's row labels (column A,
    /// rows 1 to 16) and subject headers (row 2), and the `QTD` sheet's title and period line (rows 1 and 3).
    fn structural(file: &str, sheet: &str, row: u32, col: u32) -> bool {
        match (file, sheet) {
            (synthetic::SYNTHETIC_AISC_FILE, synthetic::SYNTHETIC_TABLE_SHEET) => {
                row == 2 || (col == 1 && row <= 16)
            }
            (synthetic::SYNTHETIC_STATS_FILE, "QTD") => row == 1 || row == 3,
            _ => false,
        }
    }

    /// Perturbs one cell in place: a number +1, any other value to the text "x", a blank cell to 1.
    fn perturb(sheet: &mut fmine_io::xlsx::Sheet, row: u32, col: u32) {
        let value = match sheet.cell(row, col).map(|c| &c.value) {
            Some(CellValue::Number(x)) => CellValue::Number(x + 1.0),
            Some(_) => CellValue::Text("x".to_string()),
            None => CellValue::Number(1.0),
        };
        let number_text = match &value {
            CellValue::Number(x) => Some(x.to_string()),
            _ => None,
        };
        let cell = sheet
            .cells_by_coord
            .entry((row, col))
            .or_insert_with(|| ExtractedCell {
                cell_ref: format!("{}{row}", fmine_io::xlsx::col_index_to_letters(col)),
                row,
                col,
                value: CellValue::Number(0.0),
                formula: None,
                number_text: None,
                style: None,
                format: fmine_io::styles::NumberFormat::Builtin(0),
            });
        cell.value = value;
        cell.number_text = number_text;
    }

    /// For each instance cell, the cells whose perturbation changed its state: every non-structural cell of every
    /// visible synthetic sheet, within its used range, one at a time, under `profile` with `edits` applied first.
    /// Observed from the code's behaviour, never from its operand lists.
    fn observed_dependencies(
        profile: &IssuerProfile,
        edits: &[(String, MinimalCell)],
    ) -> (usize, BTreeMap<String, Vec<String>>) {
        let base_sources = load_synthetic(synthetic::synthetic_workbooks(edits)).expect("parse");
        let base = assess(profile, Lane::Synthetic, base_sources.clone());
        let mut swept = 0;
        let mut deps: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (si, src) in base_sources.iter().enumerate() {
            for (ki, sheet) in src.workbook.sheets.iter().enumerate() {
                let rows = sheet.cells_by_coord.keys().map(|k| k.0).max().unwrap_or(0);
                let cols = sheet.cells_by_coord.keys().map(|k| k.1).max().unwrap_or(0);
                for (row, col) in (1..=rows).flat_map(|r| (1..=cols).map(move |c| (r, c))) {
                    if structural(&src.file_name, &sheet.name, row, col) {
                        continue;
                    }
                    swept += 1;
                    let mut sources = base_sources.clone();
                    perturb(&mut sources[si].workbook.sheets[ki], row, col);
                    let r = assess(profile, Lane::Synthetic, sources);
                    let at = format!(
                        "{}:{}!{}{row}",
                        src.file_name,
                        sheet.name,
                        fmine_io::xlsx::col_index_to_letters(col)
                    );
                    for b in &base.instances {
                        let after = r
                            .instances
                            .iter()
                            .find(|e| e.cell == b.cell)
                            .map(|e| &e.state);
                        if after != Some(&b.state) {
                            deps.entry(b.cell.clone()).or_default().push(at.clone());
                        }
                    }
                }
            }
        }
        (swept, deps)
    }

    /// Each observed dependency the instance's operands do not cite, as "uncited input: <cell> → <instance>".
    fn uncited(r: &PublicAssuranceReport, deps: &BTreeMap<String, Vec<String>>) -> Vec<String> {
        let mut out = Vec::new();
        for i in &r.instances {
            let cited: Vec<String> = i
                .operands
                .iter()
                .map(|o| format!("{}:{}!{}", o.file, o.sheet, o.cell))
                .collect();
            for at in deps.get(&i.cell).into_iter().flatten() {
                if !cited.contains(at) {
                    out.push(format!(
                        "uncited input: {at} → {} {}",
                        i.cell,
                        i.family.as_str()
                    ));
                }
            }
        }
        out
    }

    /// bd-sjrh.1: every cell whose perturbation moves an instance's state is among that instance's operands.
    #[test]
    fn test_operands_cover_every_input_cell() {
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        let (swept, deps) = observed_dependencies(&synthetic::synthetic_profile(), &[]);
        let missing = uncited(&r, &deps);
        println!(
            "{{\"test\":\"operands_cover\",\"instances\":{},\"operands\":{},\"swept_cells\":{swept},\
             \"dependencies\":{},\"uncited\":{}}}",
            r.instances.len(),
            r.instances.iter().map(|i| i.operands.len()).sum::<usize>(),
            deps.values().map(Vec::len).sum::<usize>(),
            missing.len()
        );
        assert!(r.instances.iter().all(|i| !i.operands.is_empty()));
        assert!(swept > 300, "{swept}");
        // Every instance but the unrounded column's depends on something the sweep can move.
        assert!(deps.len() >= 30, "{}", deps.len());
        assert!(missing.is_empty(), "{missing:#?}");
    }

    /// bd-r7lp: a run through a shared memo gives the anchor, Mine Stats reads and Mine Stats ledger `assess`
    /// gives, on the base sources and on every one-cell perturbation of every synthetic sheet; the memo is reused
    /// exactly when no Mine Stats cell changed, a perturbed Mine Stats run never displaces the base entry, and a
    /// changed profile recomputes.
    #[test]
    fn test_mine_stats_memo_matches_assess_on_every_one_cell_edit() {
        let profile = synthetic::synthetic_profile();
        let mut base = load_synthetic(synthetic::synthetic_workbooks(&[])).expect("parse");
        // A second Mine Stats release whose summary poses an ownership instance (Site X at 50%), so that the
        // sweep moves the ledger.
        let text = |r: &str, s: &str| (r.to_string(), MinimalCell::Text(s.to_string()));
        let num = |r: &str, v: f64| (r.to_string(), MinimalCell::Number(v));
        let owned = fmine_io::xlsx::build_minimal_xlsx(&[fmine_io::xlsx::MinimalSheet {
            name: "QTD".to_string(),
            hidden: false,
            cells: vec![
                text("A3", "For the three months ended June 30,"),
                num("B3", 2030.0),
                text("B4", "Gold produced (000s oz 100%)"),
                text("C4", "Gold produced (000s oz attributable)"),
                text("D4", "Gold sold (000s oz attributable)"),
                text("A5", "Site X (50%)"),
                num("B5", 200.0),
                num("C5", 100.0),
                num("D5", 90.0),
            ],
        }]);
        let later = vec![(
            synthetic::SYNTHETIC_LATER_STATS_FILE,
            SourceKind::MineStats,
            "2030Q2",
            owned,
        )];
        base.extend(load_synthetic(later).expect("parse"));
        let memo = MineStatsMemo::default();
        let same = |sources: &[LoadedSource], profile: &IssuerProfile| {
            let plain = assess(profile, Lane::Synthetic, sources.to_vec());
            let memoed = assess_memo(profile, Lane::Synthetic, sources.to_vec(), &memo);
            assert_eq!(memoed.ledger_anchor, plain.ledger_anchor);
            assert_eq!(memoed.mine_stats, plain.mine_stats);
            assert_eq!(memoed.mine_ledger, plain.mine_ledger);
            plain
        };
        let first = same(&base, &profile);
        assert_eq!((memo.hits(), memo.misses()), (0, 1));
        let (mut swept, mut stats_cells, mut ledger_moved) = (0, 0, 0);
        for (si, src) in base.iter().enumerate() {
            for (ki, sheet) in src.workbook.sheets.iter().enumerate() {
                let rows = sheet.cells_by_coord.keys().map(|k| k.0).max().unwrap_or(0);
                let cols = sheet.cells_by_coord.keys().map(|k| k.1).max().unwrap_or(0);
                for (row, col) in (1..=rows).flat_map(|r| (1..=cols).map(move |c| (r, c))) {
                    let mut sources = base.clone();
                    perturb(&mut sources[si].workbook.sheets[ki], row, col);
                    // The base entry stays: a perturbed Mine Stats run before this one did not displace it.
                    let hits = memo.hits();
                    same(&base, &profile);
                    assert_eq!(memo.hits() - hits, 1);
                    let hits = memo.hits();
                    let r = same(&sources, &profile);
                    let stats = src.kind == SourceKind::MineStats;
                    assert_eq!(
                        memo.hits() - hits,
                        u64::from(!stats),
                        "{}:{}!{row},{col}",
                        src.file_name,
                        sheet.name
                    );
                    swept += 1;
                    stats_cells += usize::from(stats);
                    ledger_moved += usize::from(r.mine_ledger != first.mine_ledger);
                }
            }
        }
        let mut changed = profile.clone();
        changed.mine_stats_label_aliases.clear();
        changed.regions.push("Region Elsewhere".to_string());
        same(&base, &profile);
        let misses = memo.misses();
        same(&base, &changed);
        assert_eq!(memo.misses() - misses, 1);
        println!(
            "{{\"test\":\"mine_stats_memo\",\"swept_cells\":{swept},\"mine_stats_cells\":{stats_cells},\
             \"ledger_moved\":{ledger_moved},\"hits\":{},\"misses\":{}}}",
            memo.hits(),
            memo.misses()
        );
        assert!(swept > 300 && stats_cells > 0, "{swept} {stats_cells}");
        // The equality checks are not vacuous: some perturbations move the Mine Stats ledger.
        assert!(ledger_moved > 0);
    }

    /// BD-331.S4: the same sweep with Region North weighed against Site H and Site I (consistent at 2100, exactly).
    /// Every cell that moves the region's state is among its operands, the member cells included, and the
    /// region's state does move with each member's rate and ounces.
    #[test]
    fn test_region_operands_cover_every_input_cell() {
        let profile = region_profile("2030Q1");
        let edits = [
            (String::from("H7"), MinimalCell::Number(-10.0)),
            (String::from("H13"), MinimalCell::Number(300.0)),
            (String::from("H14"), MinimalCell::Number(2100.0)),
        ];
        let r = assess(
            &profile,
            Lane::Synthetic,
            load_synthetic(synthetic::synthetic_workbooks(&edits)).expect("parse"),
        );
        assert!(matches!(
            state_of(&r, "H14", IdentityFamily::CosPerOunce),
            IdentityState::Consistent {
                basis: Some(ConsistentBasis::MembersExact(_)),
                ..
            }
        ));
        assert_eq!(r.relaxation_only(), 0);
        let (_, deps) = observed_dependencies(&profile, &edits);
        let missing = uncited(&r, &deps);
        assert!(missing.is_empty(), "{missing:#?}");
        let region: Vec<&str> = deps
            .get("H14")
            .into_iter()
            .flatten()
            .filter_map(|at| at.rsplit_once('!').map(|(_, cell)| cell))
            .collect();
        for member_cell in ["K13", "K14", "L13", "L14"] {
            assert!(region.contains(&member_cell), "{member_cell}: {region:?}");
        }
    }

    /// A broken cost-of-sales label on the release's own quarter sheet refuses the table by name instead of
    /// dropping it (BD-338.S3 finding).
    #[test]
    fn test_own_quarter_table_label_broken_refuses() {
        let r = run_synthetic_assurance(&[(
            "A5".to_string(),
            MinimalCell::Text("Cost of sales applicable to gold productionx".to_string()),
        )])
        .expect("synthetic run");
        let refusals: Vec<(&str, &str)> = r
            .table_refusals
            .iter()
            .map(|t| (t.sheet.as_str(), t.reason.as_str()))
            .collect();
        assert_eq!(
            refusals,
            [(
                synthetic::SYNTHETIC_TABLE_SHEET,
                "no cost-of-sales row on the release's own quarter sheet"
            )]
        );
        assert!(r.instances.is_empty());
    }

    /// A reconciliation without its own quarter's sheet refuses that sheet by name; a table on a renamed sheet is
    /// still read.
    #[test]
    fn test_own_quarter_sheet_missing_refuses() {
        let mut sources = load_synthetic(synthetic::synthetic_workbooks(&[])).expect("parse");
        for s in &mut sources {
            for sheet in &mut s.workbook.sheets {
                if sheet.name == synthetic::SYNTHETIC_TABLE_SHEET {
                    sheet.name = "Q1 2031".to_string();
                }
            }
        }
        let r = assess(&synthetic::synthetic_profile(), Lane::Synthetic, sources);
        let refusals: Vec<(&str, &str)> = r
            .table_refusals
            .iter()
            .map(|t| (t.sheet.as_str(), t.reason.as_str()))
            .collect();
        assert_eq!(
            refusals,
            [(
                synthetic::SYNTHETIC_TABLE_SHEET,
                "the release's own quarter sheet is missing"
            )]
        );
        assert_eq!(r.aisc_tables, 1);
    }

    /// The coverage check catches a dropped operand: C8 without C6.
    #[test]
    fn test_mutation_missing_operands_detected() {
        let mut r = run_synthetic_assurance(&[]).expect("synthetic run");
        let (_, deps) = observed_dependencies(&synthetic::synthetic_profile(), &[]);
        let Some(c8) = r.instances.iter_mut().find(|i| i.cell == "C8") else {
            panic!("no C8");
        };
        c8.operands.retain(|o| o.cell != "C6");
        assert_eq!(
            uncited(&r, &deps),
            [format!("uncited input: {} → C8 tcc_footing", aisc_at("C6"))]
        );
    }

    /// BD-331.S2: a (ii) says whether the issuer states its rule. Region North's is reconstructed; Site J's is
    /// stated at A31. Barrick's E816 rows are registered as reconstructed.
    #[test]
    fn test_explained_basis_labels() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let h16 = state_of(&r, "H16", IdentityFamily::AiscPerOunce);
        assert_eq!(h16.label(), "explained_reconstructed");
        assert_eq!(
            h16.detail(),
            "SYNTHETIC: Synthetic_Mine_Stats.xlsx:Minestats3!B32=13"
        );
        let m16 = state_of(&r, "M16", IdentityFamily::AiscPerOunce);
        assert_eq!(m16.label(), "explained_stated");
        assert_eq!(
            m16.detail(),
            "SYNTHETIC-STATED: Synthetic_Mine_Stats.xlsx:Minestats3!B32=13; stated at \
             Synthetic_AISC.xlsx:Q1 2030!A31"
        );
        let json = r.to_json();
        assert!(json.contains("\"label\":\"explained_stated\""), "{json}");
        let barrick = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let e816: Vec<&ExplanationBasis> = barrick
            .reconstructions
            .iter()
            .filter(|r| r.evidence_id == "E816")
            .map(|r| &r.basis)
            .collect();
        assert_eq!(e816, [&ExplanationBasis::Reconstructed; 3]);
    }

    /// BD-331.S2 planted negative: when the stated quote no longer reads as registered, the explanation is
    /// refused and the instance stays (iv), naming the quote cell. The reconstructed (ii) needs no quote.
    #[test]
    fn test_stated_quote_mismatch_keeps_iv() {
        // The quote is read with its runs of whitespace collapsed; an empty cell reads as "".
        for (text, found_text) in [
            (
                "i.  Site J's all-in sustaining costs per ounce exclude Site G's capital.",
                "i. Site J's all-in sustaining costs per ounce exclude Site G's capital.",
            ),
            ("", ""),
        ] {
            let edit = [(String::from("A31"), MinimalCell::Text(text.to_string()))];
            let r = run_synthetic_assurance(&edit).expect("synthetic");
            match state_of(&r, "M16", IdentityFamily::AiscPerOunce) {
                s @ IdentityState::Undecidable {
                    why, missing_fact, ..
                } => {
                    assert!(
                        matches!(why, Undecided::ReconstructionNotApplied {
                            evidence_id,
                            check: ReconstructionCheck::QuoteMismatch { at, found, .. },
                        } if evidence_id == "SYNTHETIC-STATED"
                            && at == "Synthetic_AISC.xlsx:Q1 2030!A31"
                            && found == found_text),
                        "{why:?}"
                    );
                    assert!(
                        s.detail().starts_with(
                            "registered reconstruction SYNTHETIC-STATED not applied: stated quote \
                             Synthetic_AISC.xlsx:Q1 2030!A31 reads"
                        ),
                        "{}",
                        s.detail()
                    );
                    assert_eq!(missing_fact, aisc::DEFAULT_MISSING_FACT);
                }
                other => panic!("M16 with A31 = {text:?}: {other:?}"),
            }
            assert_eq!(
                state_of(&r, "H16", IdentityFamily::AiscPerOunce).label(),
                "explained_reconstructed"
            );
        }
    }

    /// BD-331.S2: an unregistered (subject, family) names its family's default; a registered row replaces it;
    /// a stated-share miss names the share rule's fact.
    #[test]
    fn test_missing_fact_family_default() {
        let mut profile = synthetic::synthetic_profile();
        profile.missing_facts.clear();
        let r = synthetic_run_with(&profile, &[], "Site H (75%)1");
        let fact = |cell: &str, family| match state_of(&r, cell, family) {
            IdentityState::Undecidable { missing_fact, .. } => missing_fact.clone(),
            other => panic!("{cell}: {other:?}"),
        };
        assert_eq!(
            fact("D16", IdentityFamily::AiscPerOunce),
            aisc::DEFAULT_MISSING_FACT
        );
        assert_eq!(
            fact("L14", IdentityFamily::CosPerOunce),
            aisc::SHARE_MISSING_FACT
        );
        // A row for another family of the same subject does not apply.
        profile.missing_facts = synthetic::synthetic_profile().missing_facts;
        profile.missing_facts[0].family = IdentityFamily::TccPerOunce;
        let r = synthetic_run_with(&profile, &[], "Site H (75%)1");
        match state_of(&r, "D16", IdentityFamily::AiscPerOunce) {
            IdentityState::Undecidable { missing_fact, .. } => {
                assert_eq!(missing_fact, aisc::DEFAULT_MISSING_FACT)
            }
            other => panic!("D16: {other:?}"),
        }
        // Every (iv) of the seeded run names one, in the state and in the report.
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let iv: Vec<&IdentityInstance> = r
            .instances
            .iter()
            .filter(|i| i.state.code() == "iv")
            .collect();
        assert_eq!(iv.len(), 2);
        for i in iv {
            assert!(i.state.detail().contains("; missing fact: the "), "{i:?}");
        }
        assert_eq!(r.to_json().matches("\"missing_fact\":\"the ").count(), 2);
    }

    /// BD-331.S2: missing-fact texts are prose. The two built-in texts and every registered Barrick row have no
    /// digit (the registry refuses one; `fmine-registry` tests that).
    #[test]
    fn test_missing_fact_texts_have_no_digits() {
        let barrick = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        assert_eq!(barrick.missing_facts.len(), 3);
        let texts = [aisc::DEFAULT_MISSING_FACT, aisc::SHARE_MISSING_FACT]
            .into_iter()
            .chain(barrick.missing_facts.iter().map(|m| m.text.as_str()));
        for text in texts {
            assert!(!text.bytes().any(|b| b.is_ascii_digit()), "{text}");
        }
    }

    /// BD-331.S2: hidden sheets are parsed into `hidden_sheets` and leave the census and its digest unchanged.
    #[test]
    fn test_hidden_sheet_parsed_not_censused() {
        let files = synthetic::synthetic_workbooks(&[]);
        let aisc_bytes = files
            .iter()
            .find(|f| f.0 == synthetic::SYNTHETIC_AISC_FILE)
            .map(|f| f.3.clone())
            .expect("aisc");
        let with_hidden = Workbook::from_bytes("a.xlsx", &aisc_bytes).expect("parse");
        let visible_only = Workbook::from_bytes(
            "a.xlsx",
            &fmine_io::xlsx::build_minimal_xlsx(&[fmine_io::xlsx::MinimalSheet {
                name: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
                hidden: false,
                cells: synthetic::synthetic_aisc_cells(),
            }]),
        )
        .expect("parse");
        let names: Vec<&str> = with_hidden
            .hidden_sheets
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(names, ["Scratch", synthetic::SYNTHETIC_HIDDEN_SHEET]);
        assert_eq!(
            with_hidden.hidden_sheets[0]
                .cell(1, 1)
                .and_then(|c| c.value.as_number()),
            Some(1.0)
        );
        assert!(with_hidden.sheet("Scratch").is_none());
        let (a, b) = (with_hidden.cell_census(), visible_only.cell_census());
        assert_eq!((a.hidden_sheets, b.hidden_sheets), (2, 0));
        assert_eq!(
            CellCensus {
                hidden_sheets: 0,
                ..a
            },
            b
        );
    }

    /// BD-331.S2: the hidden table's instances are reported in their own section, labelled, and never counted.
    #[test]
    fn test_hidden_scope_never_counted() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let hidden = &r.hidden_scope;
        assert_eq!(hidden.aisc_tables, 1);
        assert_eq!(r.aisc_tables, 1);
        let states: Vec<(&str, &str)> = hidden
            .instances
            .iter()
            .map(|i| (i.cell.as_str(), i.state.code()))
            .collect();
        assert_eq!(
            states,
            [
                ("B8", "iii"),
                ("B11", "i"),
                ("B14", "i"),
                ("B15", "i"),
                ("B16", "i")
            ]
        );
        assert!(
            hidden
                .instances
                .iter()
                .all(|i| i.sheet == synthetic::SYNTHETIC_HIDDEN_SHEET)
        );
        assert!(
            r.instances
                .iter()
                .all(|i| i.sheet == synthetic::SYNTHETIC_TABLE_SHEET)
        );
        assert_eq!(r.count_state("iii"), 1, "only C8 on the visible sheet");
        let json = r.to_json();
        let (ledger, rest) = json.split_once("\"hidden_scope\":").expect("section");
        assert!(!ledger.contains(synthetic::SYNTHETIC_HIDDEN_SHEET));
        assert!(rest.contains(
            "\"sheet\":\"Q4 2029\",\"cell\":\"B8\",\"subject\":\"Site A\",\"family\":\"tcc_footing\",\
             \"detail\":\"printed 401 vs component sum 400 (residual 1)\",\
             \"scope\":\"sheet not displayed to readers\""
        ));
    }

    /// bd-4wvl.4: the construction discovery reads exactly the decided footings, per family and scope, and the
    /// payload carries it after the hidden scope. A handful of synthetic footings identifies no construction.
    #[test]
    fn test_construction_discovery_on_synthetic_ledger() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let found = r.construction_discovery().expect("discovery");
        let order: Vec<(&str, &str)> = found.iter().map(|(s, d)| (*s, d.family.as_str())).collect();
        assert_eq!(
            order,
            [
                ("visible", "tcc_footing"),
                ("visible", "aisc_footing"),
                ("hidden", "tcc_footing"),
                ("hidden", "aisc_footing")
            ]
        );
        let json = r.to_json();
        let (_, section) = json
            .split_once("\"construction_discovery\":")
            .expect("section");
        assert!(section.starts_with("{\"report_only\":"), "{section}");
        assert!(json.contains("\"table_refusals\":0},\"construction_discovery\":{"));
        for (scope, d) in &found {
            let instances = if *scope == "visible" {
                &r.instances
            } else {
                &r.hidden_scope.instances
            };
            let decided = |code: &str| {
                instances
                    .iter()
                    .filter(|i| i.family == d.family && i.state.code() == code)
                    .count()
            };
            assert!(d.totals > 0, "{scope} {:?}", d.family);
            assert_eq!(
                d.totals,
                decided("i") + decided("iii"),
                "{scope} {:?}",
                d.family
            );
            assert_eq!(d.foot_exact, decided("i"), "{scope} {:?}", d.family);
            assert_eq!(
                d.report.discovered,
                fmine_numerics::exact_feas::discover::Discovered::Ambiguous,
                "{d:?}"
            );
            assert!(section.contains(&discovery_json(scope, d)));
        }
    }

    /// bd-4wvl.4: a footing printed twice with the same subject and numbers (a comparative column) is one draw;
    /// the same numbers under another subject are another.
    #[test]
    fn test_construction_discovery_counts_a_reprinted_footing_once() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let base = footing_discovery(&r.instances).expect("discovery");
        let doubled: Vec<IdentityInstance> =
            r.instances.iter().chain(&r.instances).cloned().collect();
        let twice = footing_discovery(&doubled).expect("discovery");
        let renamed: Vec<IdentityInstance> = r
            .instances
            .iter()
            .cloned()
            .chain(r.instances.iter().cloned().map(|mut i| {
                i.subject.push_str(" (other)");
                i
            }))
            .collect();
        let other = footing_discovery(&renamed).expect("discovery");
        for ((b, t), o) in base.iter().zip(&twice).zip(&other) {
            assert_eq!((t.totals, t.foot_exact), (2 * b.totals, 2 * b.foot_exact));
            assert_eq!(t.report, b.report, "the reprint adds no draw");
            assert_eq!(
                o.report.n,
                2 * b.report.n,
                "another subject is another draw"
            );
        }
    }

    /// BD-331.S2: a hidden cell is outside the counts but inside the root.
    #[test]
    fn test_hidden_scope_moves_root() {
        let base = run_synthetic_assurance(&[]).expect("synthetic");
        let mut files = synthetic::synthetic_workbooks(&[]);
        let mut hidden_cells = synthetic::synthetic_hidden_cells();
        hidden_cells.retain(|(r, _)| r != "B8");
        hidden_cells.push(("B8".to_string(), MinimalCell::Number(400.0)));
        for f in &mut files {
            if f.0 == synthetic::SYNTHETIC_AISC_FILE {
                f.3 = fmine_io::xlsx::build_minimal_xlsx_styled(
                    &[
                        fmine_io::xlsx::MinimalSheet {
                            name: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
                            hidden: false,
                            cells: synthetic::synthetic_aisc_cells(),
                        },
                        fmine_io::xlsx::MinimalSheet {
                            name: "Scratch".to_string(),
                            hidden: true,
                            cells: vec![("A1".to_string(), MinimalCell::Number(1.0))],
                        },
                        fmine_io::xlsx::MinimalSheet {
                            name: synthetic::SYNTHETIC_HIDDEN_SHEET.to_string(),
                            hidden: true,
                            cells: hidden_cells.clone(),
                        },
                    ],
                    Some(&synthetic::synthetic_aisc_styles()),
                );
            }
        }
        let moved = assess(
            &synthetic::synthetic_profile(),
            Lane::Synthetic,
            load_synthetic(files).expect("parse"),
        );
        assert_eq!(moved.instances, base.instances);
        assert_eq!(
            moved.sources[1].census.numeric_digest,
            base.sources[1].census.numeric_digest
        );
        assert_eq!(moved.hidden_scope.instances[0].state.code(), "i");
        assert_ne!(
            moved.ledger_anchor.state_root,
            base.ledger_anchor.state_root
        );
    }

    /// A complete run through a TOML registry in default CI (the real-corpus runs are ignored tests): the two
    /// synthetic workbooks plus a third, later Mine Stats file are written to a scratch corpus, registered by
    /// digest in a generated TOML, and loaded with no code-side profile. The ledger equals the in-code synthetic
    /// profile's, and `--through 2030Q1` drops the third file and leaves the ledger unchanged.
    #[test]
    fn test_synthetic_three_file_toml_profile_runs_end_to_end() {
        let ws = std::env::temp_dir().join(format!("fmine-public-toml-e2e-{}", std::process::id()));
        let reg = synthetic::write_issuer_workspace(&ws).expect("write workspace");
        assert_eq!(reg, sources::registry_path(&ws, "synthetic"));
        let code_profile = synthetic::synthetic_profile();

        let profile = load_issuer_profile(&ws, "synthetic").expect("generated registry loads");
        assert_eq!(profile.sources.len(), 3);
        assert_eq!(profile.reconstructions, code_profile.reconstructions);
        assert_eq!(profile.missing_facts, code_profile.missing_facts);
        let r = run_public_assurance(&profile, &ws).expect("all three files verify by digest");
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| r.count_state(c)).collect();
        assert_eq!(
            totals,
            vec![40, 2, 1, 2, 5, 5],
            "same ledger as the in-code synthetic profile"
        );
        assert_eq!(r.hidden_scope.instances.len(), 5);
        assert_eq!(
            (&profile.regions, &profile.shares),
            (&code_profile.regions, &code_profile.shares)
        );
        assert_eq!(r.sources.len(), 3);
        assert_eq!(r.ledger_anchor.commit_sequence, 3);
        // The reserved issuer name keeps invented numbers out of the public lane; the same registry under
        // another name would be gate-eligible, so the name is what decides.
        assert_eq!((r.lane, r.gate_eligible()), (Lane::Synthetic, false));
        let mut renamed = profile.clone();
        renamed.issuer = "renamed".to_string();
        let public = run_public_assurance(&renamed, &ws).expect("same files");
        assert_eq!((public.lane, public.gate_eligible()), (Lane::Public, true));

        let bounded =
            run_public_assurance(&profile.through("2030Q1").expect("bound"), &ws).expect("run");
        assert_eq!(bounded.sources.len(), 2);
        assert_eq!(bounded.through.as_deref(), Some("2030Q1"));
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| bounded.count_state(c)).collect();
        assert_eq!(totals, vec![40, 2, 1, 2, 5, 5]);
        assert_ne!(
            bounded.ledger_anchor.state_root, r.ledger_anchor.state_root,
            "the source set is in the root"
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// bd-jg43.1 parity: reading styles changes no census count and no numeric digest. The values were printed
    /// by this test at commit 345e2ba, before the styles reader existed, and pasted here. bd-jg43.3 styled the
    /// AISC workbook and added column N (11 numbers and the header "Site G"): its census must equal that of the
    /// same cells written as plain numbers with no styles part.
    #[test]
    fn test_census_unchanged_by_style_reading() {
        let line = |name: &str, bytes: &[u8]| {
            let c = Workbook::from_bytes(name, bytes)
                .expect("synthetic workbook")
                .cell_census();
            format!(
                "{name} {} {} {} {} {} {} {}",
                c.visible_sheets,
                c.hidden_sheets,
                c.numeric_cells,
                c.placeholder_cells,
                c.formula_error_cells,
                c.text_cells,
                hex(&c.numeric_digest)
            )
        };
        let got: Vec<String> = synthetic::synthetic_workbooks(&[])
            .into_iter()
            .map(|(name, _, _, bytes)| line(name, &bytes))
            .collect();
        assert_eq!(
            got[0],
            "Synthetic_Mine_Stats.xlsx 2 0 2 0 0 9 ed492958121280b991212c61d01c9c2a257a551dfb07af145f1f6c117e646c21"
        );
        let plain: Vec<(String, MinimalCell)> = synthetic::synthetic_aisc_cells()
            .into_iter()
            .map(|(r, c)| match c {
                MinimalCell::Styled { raw, .. } => {
                    (r, MinimalCell::Number(raw.parse().expect("decimal")))
                }
                other => (r, other),
            })
            .collect();
        let unstyled = fmine_io::xlsx::build_minimal_xlsx(&[
            fmine_io::xlsx::MinimalSheet {
                name: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
                hidden: false,
                cells: plain,
            },
            fmine_io::xlsx::MinimalSheet {
                name: "Scratch".to_string(),
                hidden: true,
                cells: vec![("A1".to_string(), MinimalCell::Number(1.0))],
            },
            fmine_io::xlsx::MinimalSheet {
                name: synthetic::SYNTHETIC_HIDDEN_SHEET.to_string(),
                hidden: true,
                cells: synthetic::synthetic_hidden_cells(),
            },
        ]);
        assert_eq!(got[1], line(synthetic::SYNTHETIC_AISC_FILE, &unstyled));
        assert!(
            got[1].starts_with("Synthetic_AISC.xlsx 1 2 118 4 1 29 "),
            "{}",
            got[1]
        );
    }

    fn kit_readme() -> String {
        std::fs::read_to_string(
            workspace_root()
                .join("repro")
                .join("barrick")
                .join("README.md"),
        )
        .expect("kit README")
    }

    /// BD-339.S4 golden: the kit README's generated section equals the generator's output from the code's row
    /// anchors and the registry. On a mismatch it prints the section to paste; tests never write files.
    #[test]
    fn test_kit_readme_selectors_match_code() {
        let readme = kit_readme();
        let profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let want = sources::kit_selectors_markdown(&profile);
        let got = sources::kit_readme_section(&readme).expect("both markers");
        assert!(
            got == want.trim_matches('\n'),
            "repro/barrick/README.md's generated section differs; paste this between the markers:\n{}\n{}{}",
            sources::KIT_SELECTORS_BEGIN,
            want,
            sources::KIT_SELECTORS_END
        );
    }

    /// BD-339.S4: the golden comparison sees a one-subject drift in either direction.
    #[test]
    fn test_kit_readme_golden_detects_drift() {
        let readme = kit_readme();
        let section = sources::kit_readme_section(&readme).expect("both markers");
        let mut profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let first = profile.subjects[0].clone();
        assert_eq!(
            section,
            sources::kit_selectors_markdown(&profile).trim_matches('\n')
        );
        let renamed = readme.replacen(&format!("\n- {first}\n"), "\n- Renamed subject\n", 1);
        assert_ne!(renamed, readme, "the subject line is in the README");
        assert_ne!(
            sources::kit_readme_section(&renamed).expect("both markers"),
            sources::kit_selectors_markdown(&profile).trim_matches('\n')
        );
        profile.subjects.remove(0);
        assert_ne!(
            section,
            sources::kit_selectors_markdown(&profile).trim_matches('\n')
        );
    }

    /// BD-339.S4: the kit README carries the no-affiliation sentence verbatim and the RISK-31 instruction.
    #[test]
    fn test_kit_readme_states_no_affiliation_and_risk_31() {
        let readme = kit_readme();
        assert!(readme.contains(
            "FrankenMining is not affiliated with Barrick; these are Barrick's public files, fetched from \
             Barrick's host"
        ));
        assert!(
            readme.contains("RISK-31: do not redistribute the files; fetch them from the issuer.")
        );
    }

    /// BD-339.S1: the expected-roots file parses and every row is well formed. Each anchor is the one the run
    /// derives from its root (18 sources, public lane), so a typo in either is caught without the corpus.
    #[test]
    fn test_expected_roots_file_is_well_formed() {
        let rows = sources::load_expected_roots(&sources::expected_roots_path(
            &workspace_root(),
            "barrick",
        ))
        .expect("expected roots");
        assert!(rows.len() >= 5, "rows are append-only");
        let hex64 = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        let registry = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        for row in &rows {
            let at = format!("row of {} (line {})", row.commit, row.line);
            assert!(
                row.commit.len() >= 7 && row.commit.bytes().all(|b| b.is_ascii_hexdigit()),
                "{at}"
            );
            for h in [
                &row.registry_epoch,
                &row.state_root,
                &row.census_digest_of_digests,
            ] {
                assert!(hex64(h), "{at}: {h}");
            }
            assert!(!row.reason.is_empty() && !row.evidence.is_empty(), "{at}");
            assert_eq!(row.totals.iter().sum::<u64>(), 2595, "{at}");
            let mut root = [0_u8; 32];
            for (i, byte) in root.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&row.state_root[2 * i..2 * i + 2], 16).expect("hex");
            }
            let root = Digest32(root);
            assert_eq!(
                ledger_anchor("barrick", Lane::Public, registry.sources.len(), root)
                    .anchor_id()
                    .to_string(),
                row.anchor_id,
                "{at}"
            );
        }
        // E822's row, resolved by re-running commit 748da79 on the corpus.
        assert_eq!(
            (rows[0].commit.as_str(), rows[0].totals),
            ("748da79", [2120, 3, 4, 17, 391, 60])
        );
        let roots: std::collections::BTreeSet<&str> =
            rows.iter().map(|r| r.state_root.as_str()).collect();
        assert_eq!(roots.len(), rows.len(), "each row records a new root");
    }

    /// BD-339.S1 negative control: one flipped hex digit in any compared field is reported, naming the field.
    #[test]
    fn test_expected_root_comparison_rejects_flipped_digit() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let good = ExpectedRoot {
            commit: "0000000".to_string(),
            registry_epoch: r.registry_epoch(),
            state_root: r.ledger_anchor.state_root.to_hex(),
            anchor_id: r.ledger_anchor.anchor_id().to_string(),
            totals: STATE_CODES.map(|c| r.count_state(c) as u64),
            census_digest_of_digests: r.census_digest_of_digests(),
            reason: "synthetic".to_string(),
            evidence: "synthetic".to_string(),
            line: 1,
        };
        assert_eq!(r.check_expected(&good), Ok(()));
        let flip = |s: &str| {
            let mut b = s.as_bytes().to_vec();
            let last = b.len() - 1;
            b[last] = if b[last] == b'0' { b'1' } else { b'0' };
            String::from_utf8(b).expect("ascii")
        };
        let cases: [(&str, ExpectedRoot); 5] = [
            (
                "state_root",
                ExpectedRoot {
                    state_root: flip(&good.state_root),
                    ..good.clone()
                },
            ),
            (
                "anchor_id",
                ExpectedRoot {
                    anchor_id: flip(&good.anchor_id),
                    ..good.clone()
                },
            ),
            (
                "registry_epoch",
                ExpectedRoot {
                    registry_epoch: flip(&good.registry_epoch),
                    ..good.clone()
                },
            ),
            (
                "census_digest_of_digests",
                ExpectedRoot {
                    census_digest_of_digests: flip(&good.census_digest_of_digests),
                    ..good.clone()
                },
            ),
            (
                "totals",
                ExpectedRoot {
                    totals: [good.totals[0] + 1, 0, 0, 0, 0, 0],
                    ..good.clone()
                },
            ),
        ];
        for (field, bad) in cases {
            let why = r.check_expected(&bad).expect_err(field);
            assert!(why.contains(&format!("{field}: run ")), "{field}: {why}");
        }
    }

    #[test]
    fn test_next_actions_text_has_no_hard_coded_count() {
        let actions = run_synthetic_assurance(&[])
            .expect("synthetic")
            .next_actions();
        let text: String = actions.iter().map(|(c, w)| format!("{c} {w} ")).collect();
        assert!(text.contains("registries/sources/barrick.toml"), "{text}");
        assert!(!text.contains(" 18 "), "{text}");
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    /// GATE-BULL-01 on the real registered workbooks. Every expected value below comes from the independent
    /// Python reader written before this code (scratch `fidelity.py` and `census2.py`, 2026-10-29; recorded in
    /// MOR_CASE E820–E823), never from this implementation's own output.
    ///
    /// The run is bounded `--through 2026Q2`, so it keeps testing the same inputs after later vintages are
    /// registered. The state root at the end is different in kind: it is a regression pin of this
    /// implementation's own output (MOR_CASE E822, updated 2026-10-29 for outward rate rounding), not an
    /// independent value.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_real_files_match_the_independent_reader() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        assert!(r.gate_eligible());
        assert_eq!(r.through.as_deref(), Some("2026Q2"));

        // Part 1, extraction fidelity: visible, hidden, numeric, placeholder, error, text cells and the digest.
        let oracle: &[(&str, [usize; 6], &str)] = &[
            (
                "Barrick_Q1_2024_Mine_Stats.xlsx",
                [6, 0, 1130, 298, 0, 251],
                "293c9e29be4e09cc0eb380b2d80bcbca5aae5656bd65284f275b3265dafc2691",
            ),
            (
                "Barrick_Q1_2025_Mine_Stats.xlsx",
                [6, 0, 1141, 197, 0, 243],
                "f4636a96aa9f97c2e29969c5cd551b107e6a887129454074a527d27315b788a7",
            ),
            (
                "Barrick_Q1_2026_Mine_Stats.xlsx",
                [6, 0, 1112, 242, 0, 249],
                "e96c6202baddf1fea717d8b8ae3fb55211f5a1fe4e35f152f4e8d92eca0dc7a3",
            ),
            (
                "Barrick_Q2_2024_Mine_Stats.xlsx",
                [7, 0, 2299, 556, 0, 490],
                "23e8da7796431d1a13cbbace051ffd7fb048f3dc1e96311b55e2c27853cf0b19",
            ),
            (
                "Barrick_Q2_2025_Mine_Stats.xlsx",
                [7, 0, 2304, 370, 0, 475],
                "e251c3f9b10fd4355e91d4518d02c94d27da097d80e6e3045f99bcb6a66845d0",
            ),
            (
                "Barrick_Q2_2026_Mine_Stats.xlsx",
                [7, 0, 2297, 480, 1092, 494],
                "cab9164142f442cd240606ae7a0652c8953f449c9a4b98f2f8cf081e255ad3b2",
            ),
            (
                "Barrick_Q3_2024_Mine_Stats.xlsx",
                [7, 0, 2305, 550, 0, 489],
                "7b035d292541cd6693239816ea07ed9fb68930887052d8fcd5ffb3baedfc085c",
            ),
            (
                "Barrick_Q3_2025_Mine_Stats.xlsx",
                [7, 0, 2311, 363, 0, 475],
                "e18d37f3b0df75d238a227519e40ffac5e553b00684e489b135e834b11856ad3",
            ),
            (
                "Barrick_Q4_2024_Mine_Stats.xlsx",
                [7, 0, 2308, 546, 0, 489],
                "bbf38e19e04a4d3d1bfe9fe0b26afba69863065873e6459d716e67328038a3b1",
            ),
            (
                "Barrick_Q4_2025_Mine_Stats.xlsx",
                [7, 0, 2357, 350, 0, 486],
                "ddb6af2ae1508e85d01ed3445fb3ce54883e3791a5510427b8c21cdcadef9efb",
            ),
            (
                "Barrick_Q1_2025_AISC.xlsx",
                [3, 0, 1146, 0, 0, 243],
                "fb649ed9d8e09f54f4569c482266e1f33c2017f2045d5d2f65671b5ba0112cfe",
            ),
            (
                "Barrick_Q1_2026_AISC.xlsx",
                [3, 2, 680, 0, 0, 154],
                "38dcba35d809105e525637133ca349722502e36d4facce667c2810d54d63e574",
            ),
            (
                "Barrick_Q2_2025_AISC.xlsx",
                [5, 0, 1910, 0, 0, 400],
                "fdabe9e9ca38b3caa89b14b769ea1f74777ca59220af04b2355d9ac7f6a66fc9",
            ),
            (
                "Barrick_Q2_2026_AISC.xlsx",
                [5, 2, 1088, 0, 0, 254],
                "dbef0e1bab051189f7fed9a7cd6e90ff8aff51ee3ad6a8d3f3181a6e53e59ffb",
            ),
            (
                "Barrick_Q3_2024_AISC.xlsx",
                [5, 0, 1910, 0, 0, 410],
                "9a7c8fe4b3298c63a4abfd7dc89ab87933df588aca80941f7930713b945c1076",
            ),
            (
                "Barrick_Q3_2025_AISC.xlsx",
                [5, 0, 1610, 0, 0, 395],
                "82d1d5c8aae8e9718da38ae52704a49a16dea07e227d70050bb862127c7c9849",
            ),
            (
                "Barrick_Q4_2024_AISC.xlsx",
                [4, 1, 1528, 0, 0, 328],
                "8431c88ef3a474f1c97c4a3485669292145e28f5a37536b56c0e7d79b9f9df0d",
            ),
            (
                "Barrick_Q4_2025_AISC.xlsx",
                [4, 1, 952, 0, 0, 212],
                "954f03ad421d6d20ca4c062e5b11b239ee3c46e97e5ceb4cb49dade616f71d1e",
            ),
        ];
        assert_eq!(
            r.sources.len(),
            oracle.len(),
            "every loaded file has an oracle row"
        );
        for &(file, counts, digest) in oracle {
            let s = r.sources.iter().find(|s| s.file_name == file).expect(file);
            let c = &s.census;
            let got = [
                c.visible_sheets,
                c.hidden_sheets,
                c.numeric_cells,
                c.placeholder_cells,
                c.formula_error_cells,
                c.text_cells,
            ];
            assert_eq!(got, counts, "{file} census");
            assert_eq!(hex(&c.numeric_digest), digest, "{file} numeric digest");
        }

        // Part 2, the identity ledger.
        assert_eq!((r.aisc_tables, r.instances.len()), (34, 2595));
        assert!(r.table_refusals.is_empty() && r.unregistered_headers.is_empty());
        let counts = r.state_counts();
        let get = |f: IdentityFamily, c: &'static str| counts.get(&(f, c)).copied().unwrap_or(0);
        use IdentityFamily::{AiscFooting, AiscPerOunce, CosPerOunce, TccFooting, TccPerOunce};
        // bd-jg43.3 (`census2.py --displayed`, E838): on displayed values E823's three (v) per footing family
        // and per TCC and AISC rate are (i).
        assert_eq!(
            [
                get(TccFooting, "i"),
                get(TccFooting, "iii"),
                get(TccFooting, "v"),
                get(TccFooting, "np")
            ],
            [505, 2, 0, 12]
        );
        assert_eq!(
            [
                get(AiscFooting, "i"),
                get(AiscFooting, "iii"),
                get(AiscFooting, "v"),
                get(AiscFooting, "np")
            ],
            [505, 2, 0, 12]
        );
        assert_eq!(
            [
                get(CosPerOunce, "i"),
                get(CosPerOunce, "iv"),
                get(CosPerOunce, "v"),
                get(CosPerOunce, "np")
            ],
            // BD-331.S3 (census E826): of the 379 rates refused before for a non-zero non-controlling-interest
            // line, 263 fit the stated share (i) and 65 miss (iv); BD-331.S4 (region_census.py): of the 51
            // region instances left, 41 are not refuted by their members (i) and 10 are (iv).
            [111 + 263 + 41, 17 + 65 + 10, 379 - 328 - 51, 12]
        );
        assert_eq!(
            [
                get(TccPerOunce, "i"),
                get(TccPerOunce, "v"),
                get(TccPerOunce, "np")
            ],
            [507, 0, 12]
        );
        assert_eq!(
            [
                get(AiscPerOunce, "i"),
                get(AiscPerOunce, "ii"),
                get(AiscPerOunce, "v"),
                get(AiscPerOunce, "np")
            ],
            [504, 3, 0, 12]
        );
        assert_eq!(
            r.alternate_counts(),
            AlternateCounts {
                // `census2.py --displayed`: 1,014 and 1,000 (E838; 1,008 and 994 on stored values).
                footing_consistent: 1014,
                // A stated-share fit sums one line (k = 1), so the alternative construction accepts all 263. The
                // 41 regions not refuted by their members are not refuted with the region's ounces kept in their
                // own box either (BD-331.S4; the kernel's own result, no independent reader computes it).
                rate_consistent: 1000 + 263 + 41
            }
        );

        let mut iii: Vec<(String, String, String, i64, i64)> = r
            .instances
            .iter()
            .filter_map(|i| match &i.state {
                IdentityState::IssuerDiscrepancy(c) => Some((
                    i.file.clone(),
                    i.sheet.clone(),
                    i.cell.clone(),
                    c.printed,
                    c.component_sum,
                )),
                _ => None,
            })
            .collect();
        iii.sort();
        let col_of = |cell: &str| {
            cell.trim_end_matches(|c: char| c.is_ascii_digit())
                .to_string()
        };
        let found: Vec<(String, String, String, i64, i64)> = iii
            .into_iter()
            .map(|(f, s, c, p, sum)| (f, s, col_of(&c), p, sum))
            .collect();
        let want = |f: &str, c: &str, p: i64, s: i64| {
            (f.to_string(), "Q2 2024".to_string(), c.to_string(), p, s)
        };
        assert_eq!(
            found,
            vec![
                want("Barrick_Q2_2025_AISC.xlsx", "L", 13, 12),
                want("Barrick_Q2_2025_AISC.xlsx", "O", 70, 72),
                want("Barrick_Q3_2024_AISC.xlsx", "L", 13, 12),
                want("Barrick_Q3_2024_AISC.xlsx", "O", 70, 72),
            ]
        );
        let ii: Vec<String> = r
            .instances
            .iter()
            .filter_map(|i| match &i.state {
                IdentityState::Explained { component, .. } => Some(format!(
                    "{}|{}|{}|{}",
                    i.file,
                    i.sheet,
                    component.text(),
                    i.state.label()
                )),
                _ => None,
            })
            .collect();
        // BD-331.S2: no issuer note states E816's rule, so all three are reconstructed.
        assert_eq!(
            ii,
            vec![
                "Barrick_Q1_2025_AISC.xlsx|Q1 2025|Barrick_Q1_2025_Mine_Stats.xlsx:Minestats3!B32=10|\
                 explained_reconstructed",
                "Barrick_Q2_2025_AISC.xlsx|Q1 2025|Barrick_Q1_2025_Mine_Stats.xlsx:Minestats3!B32=10|\
                 explained_reconstructed",
                "Barrick_Q2_2025_AISC.xlsx|YTD Q2 2025|Barrick_Q2_2025_Mine_Stats.xlsx:Minestats3!B74=13|\
                 explained_reconstructed",
            ]
        );
        let iv_subjects: Vec<&str> = r
            .instances
            .iter()
            .filter(|i| i.state.code() == "iv")
            .map(|i| i.subject.as_str())
            .collect();
        assert_eq!(iv_subjects.iter().filter(|s| **s == "Veladero").count(), 12);
        assert_eq!(iv_subjects.iter().filter(|s| **s == "Hemlo").count(), 3);
        assert_eq!(iv_subjects.iter().filter(|s| **s == "Kibali").count(), 2);

        // bd-sjrh.1: every instance cites operands. Each (iii)'s total and summands are among them, read
        // separately from the footing, and the summands add to its component sum. Failure messages and the log
        // name cells and counts only, never values (RISK-31).
        let mut by_role: BTreeMap<&str, usize> = BTreeMap::new();
        for i in &r.instances {
            let at = format!("{}:{}!{}", i.file, i.sheet, i.cell);
            assert!(!i.operands.is_empty(), "{at} has no operands");
            for o in &i.operands {
                *by_role.entry(o.role.as_str()).or_default() += 1;
            }
            if let IdentityState::IssuerDiscrepancy(c) = &i.state {
                let value = |cell: &str| {
                    i.operands
                        .iter()
                        .find(|o| o.file == i.file && o.sheet == i.sheet && o.cell == cell)
                        .map(|o| &o.value)
                };
                assert!(
                    value(&c.total_cell) == Some(&OperandValue::Int(c.printed)),
                    "{at}: total operand differs from the printed total"
                );
                let mut sum = 0;
                for line in &c.line_cells {
                    match value(line) {
                        Some(OperandValue::Int(x)) => sum += x,
                        Some(OperandValue::Token(_) | OperandValue::Blank) => {}
                        _ => panic!("{at}: summand {line} not cited as a whole number or token"),
                    }
                }
                assert!(
                    sum == c.component_sum,
                    "{at}: cited summands do not add to the component sum"
                );
            }
        }
        let roles: Vec<String> = by_role
            .iter()
            .map(|(k, v)| format!("\"{k}\":{v}"))
            .collect();
        println!(
            "{{\"test\":\"real_operands\",\"instances\":{},\"operands\":{},\"by_role\":{{{}}}}}",
            r.instances.len(),
            by_role.values().sum::<usize>(),
            roles.join(",")
        );

        // bd-sjrh.1 part 3, cited numbers: Kibali's 'Q2 2024'!O11 certificate prints 70 against a component sum
        // of 72 (E822), cited at the registered sha256 (registry, not this run). Messages name no values.
        let json = r.to_json();
        let kibali = "Barrick_Q3_2024_AISC.xlsx";
        let sha = &profile
            .sources
            .iter()
            .find(|s| s.file_name == kibali)
            .expect("registered")
            .sha256_hex;
        let at = format!("\"{kibali}:Q2 2024!O11\"");
        let printed = format!("\"printed\":{{\"v\":70,\"at\":{at},\"sha256\":\"{sha}\"}}");
        let sum = "\"component_sum\":{\"v\":72,\"rule\":\"aisc.footing.component_sum\"";
        let residual = "\"residual\":{\"v\":-2,\"rule\":\"aisc.footing.residual\"";
        let cert = json
            .split("\"certificate\":")
            .find(|c| c.starts_with(&format!("{{{printed}")))
            .is_some_and(|c| c.contains(sum) && c.contains(residual));
        assert!(cert, "Kibali O11 certificate is not the cited triple");
        let (_, records) = json.split_once("\"instances\":[").expect("instances array");
        let records = records.split_once("],\"one_cell_cos\"").expect("end").0;
        let n_records = records.matches("{\"at\":").count();
        let n_cited = records.matches("\"cited\":{\"operands\":[{").count();
        assert_eq!((n_records, n_cited), (r.instances.len(), r.instances.len()));
        println!(
            "{{\"test\":\"real_cited\",\"records\":{n_records},\"with_operands\":{n_cited},\"payload_bytes\":{}}}",
            json.len()
        );

        // Regression pin (not independent; see the doc comment): root, anchor, totals, registry epoch and digest
        // of digests equal the last row of repro/barrick/expected.toml, which records why each root moved.
        let expected = sources::load_expected_roots(&sources::expected_roots_path(
            &workspace_root(),
            "barrick",
        ))
        .expect("expected roots");
        let last = expected.last().expect("at least one row");
        if let Err(why) = r.check_expected(last) {
            panic!("{why}");
        }
    }

    /// bd-jg43.3 on the real files: E823's 12 Loulo-Gounkoto instances (column K of `Q1 2026` in the Q1 2026
    /// workbook and of `Q2 2026` and `Q1 2026` in the Q2 2026 workbook; TCC and AISC footings and rates) leave
    /// (v), and every instance's state equals `census2.py --displayed`, run here, except the cost-of-sales rates
    /// that census leaves to the stated-share rule (`share_census.py`, checked by its own test). One line per
    /// E823 instance and a summary (`--nocapture`); operands are logged by cell, never by value (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn test_e823_twelve_instances_leave_v() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r = run_public_assurance(&profile, &root).expect("registered files present");
        let script = "import json, sys\nsys.path.insert(0, 'scripts/oracle')\nimport census2\n\
                      _, res = census2.run('.', displayed=True)\n\
                      for x in res:\n    print(json.dumps([x[0], x[1], x[2], x[4], x[5], str(x[6])]))\n";
        let out = std::process::Command::new("python3")
            .args(["-I", "-c", script])
            .current_dir(&root)
            .output()
            .expect("python3 runs");
        assert!(
            out.status.success(),
            "census2.py failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let family = |f: &str| match f {
            "TCC" => "tcc_footing",
            "AISC" => "aisc_footing",
            "COS/oz" => "cos_per_oz",
            "TCC/oz" => "tcc_per_oz",
            _ => "aisc_per_oz",
        };
        // (file, sheet, column, family) → (state, detail) as the census decides it.
        let mut census: BTreeMap<(String, String, String, String), (String, String)> =
            BTreeMap::new();
        let stdout = String::from_utf8(out.stdout).expect("utf-8 census output");
        for line in stdout.lines() {
            let f: Vec<String> = line
                .trim_matches(['[', ']'])
                .split("\", \"")
                .map(|s| s.trim_matches('"').to_string())
                .collect();
            assert_eq!(f.len(), 6, "{line}");
            let key = (
                f[0].clone(),
                f[1].clone(),
                f[2].clone(),
                family(&f[3]).to_string(),
            );
            census.insert(key, (f[4].clone(), f[5].clone()));
        }
        assert_eq!(census.len(), r.instances.len());
        let (mut compared, mut share_rule, mut differ) = (0, 0, Vec::new());
        for i in &r.instances {
            let key = (
                i.file.clone(),
                i.sheet.clone(),
                i.column.clone(),
                i.family.as_str().to_string(),
            );
            let Some((state, detail)) = census.get(&key) else {
                panic!("census has no {key:?}");
            };
            if i.family == IdentityFamily::CosPerOunce && detail == "attributable share needed" {
                share_rule += 1;
                continue;
            }
            compared += 1;
            if state != i.state.code() {
                differ.push(format!(
                    "{}:{}!{} {}",
                    i.file,
                    i.sheet,
                    i.cell,
                    i.family.as_str()
                ));
            }
        }
        let e823: Vec<&IdentityInstance> = r
            .instances
            .iter()
            .filter(|i| {
                i.column == "K"
                    && i.subject == "Loulo-Gounkoto"
                    && matches!(
                        (i.file.as_str(), i.sheet.as_str()),
                        ("Barrick_Q1_2026_AISC.xlsx", "Q1 2026")
                            | ("Barrick_Q2_2026_AISC.xlsx", "Q2 2026" | "Q1 2026")
                    )
                    && i.family != IdentityFamily::CosPerOunce
            })
            .collect();
        for i in &e823 {
            let cells: Vec<String> = i.operands.iter().map(|o| js(&o.cell)).collect();
            println!(
                "{{\"file\":{},\"sheet\":{},\"cell\":{},\"family\":\"{}\",\"state_before\":\"v\",\
                 \"state_after\":\"{}\",\"displayed_operands\":[{}]}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.cell),
                i.family.as_str(),
                i.state.code(),
                cells.join(",")
            );
        }
        let alt = r.alternate_counts();
        let n = |c: &str| r.count_state(c);
        println!(
            "{{\"instances\":{},\"i\":{},\"ii\":{},\"iii\":{},\"iv\":{},\"v\":{},\"np\":{},\
             \"alternate_footing\":{},\"alternate_rates\":{},\"state_root\":\"{}\",\"compared\":{compared},\
             \"share_rule\":{share_rule},\"oracle_equal\":{}}}",
            r.instances.len(),
            n("i"),
            n("ii"),
            n("iii"),
            n("iv"),
            n("v"),
            n("np"),
            alt.footing_consistent,
            alt.rate_consistent,
            r.ledger_anchor.state_root.to_hex(),
            differ.is_empty()
        );
        assert!(
            differ.is_empty(),
            "states differ from census2.py --displayed: {differ:?}"
        );
        assert_eq!((compared, share_rule), (2595 - 379, 379));
        assert_eq!(e823.len(), 12);
        assert!(e823.iter().all(|i| i.state.code() == "i"));
    }

    /// bd-jg43.1 on the real files: `Workbook::format_census` equals the independent reader
    /// (`python3 -I scripts/oracle/formats.py`, committed at 345e2ba before the Rust existed), run here on the
    /// same files. Lines are compared whole in the oracle's canonical JSON. The totals are also checked against
    /// the table written into the bead before either reader ran. One line per file, one per mismatch, then a
    /// summary (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn test_format_census_matches_independent_reader() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let mut names: Vec<&str> = profile
            .sources
            .iter()
            .map(|s| s.file_name.as_str())
            .collect();
        names.sort_unstable();
        assert_eq!(names.len(), 18);
        let paths: Vec<PathBuf> = names
            .iter()
            .map(|n| root.join(&profile.corpus_dir).join(n))
            .collect();
        let out = std::process::Command::new("python3")
            .arg("-I")
            .arg(root.join("scripts/oracle/formats.py"))
            .args(&paths)
            .current_dir(&root)
            .output()
            .expect("python3 runs");
        assert!(
            out.status.success(),
            "formats.py failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).expect("utf-8 oracle output");
        let oracle: Vec<&str> = stdout.lines().collect();
        assert_eq!(oracle.len(), 19, "18 file lines and a summary");

        let render = |formats: &BTreeMap<String, fmine_io::xlsx::FormatCount>| {
            let body: Vec<String> = formats
                .iter()
                .map(|(code, c)| format!("{}:[{},{}]", js(code), c.numeric, c.non_integer))
                .collect();
            format!("{{{}}}", body.join(","))
        };
        let mut total: BTreeMap<String, fmine_io::xlsx::FormatCount> = BTreeMap::new();
        let mut mismatches = 0;
        for ((name, path), want) in names.iter().zip(&paths).zip(&oracle) {
            let wb = Workbook::from_file(path).expect("registered workbook");
            let census = wb.format_census();
            for (code, c) in &census {
                let t = total.entry(code.clone()).or_default();
                t.numeric += c.numeric;
                t.non_integer += c.non_integer;
            }
            let got = format!(
                "{{\"file\":{},\"sha256\":\"{}\",\"formats\":{}}}",
                js(name),
                hex(&wb.sha256),
                render(&census)
            );
            let equal = got == *want;
            let open = got.strip_suffix('}').expect("rendered object");
            println!("{open},\"oracle_equal\":{equal}}}");
            if !equal {
                mismatches += 1;
                println!("{{\"file\":{},\"rust\":{got},\"oracle\":{want}}}", js(name));
            }
        }
        let numeric_total: usize = total.values().map(|c| c.numeric).sum();
        let non_integer_total: usize = total.values().map(|c| c.non_integer).sum();
        let summary = format!(
            "{{\"files\":18,\"formats\":{},\"numeric_total\":{numeric_total},\"non_integer_total\":{non_integer_total}}}",
            render(&total)
        );
        let pass = mismatches == 0 && summary == oracle[18];
        let open = summary.strip_suffix('}').expect("rendered object");
        println!("{open},\"pass\":{pass}}}");
        assert_eq!(
            mismatches, 0,
            "per-file format census differs from formats.py"
        );
        assert_eq!(summary, oracle[18]);
        // The bead's table (bd-jg43.1, written from a scratch read before either reader): 10 formats, 30,388
        // numeric cells (E820's total), 6,256 of them non-integer.
        assert_eq!(
            (total.len(), numeric_total, non_integer_total),
            (10, 30_388, 6_256)
        );
    }

    /// bd-jg43.2 on the real files: `Workbook::displayed_census` equals `python3 -I scripts/oracle/formats.py
    /// --displayed` (committed at 58d5c98 before the Rust interpreter) per file, compared as whole canonical
    /// lines: shown, token and refused counts and the displayed digest. E823's Loulo-Gounkoto cells are checked
    /// by value. One line per file, then a summary (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn test_displayed_digest_matches_independent_reader() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let mut names: Vec<&str> = profile
            .sources
            .iter()
            .map(|s| s.file_name.as_str())
            .collect();
        names.sort_unstable();
        assert_eq!(names.len(), 18);
        let paths: Vec<PathBuf> = names
            .iter()
            .map(|n| root.join(&profile.corpus_dir).join(n))
            .collect();
        let out = std::process::Command::new("python3")
            .arg("-I")
            .arg(root.join("scripts/oracle/formats.py"))
            .arg("--displayed")
            .args(&paths)
            .current_dir(&root)
            .output()
            .expect("python3 runs");
        assert!(
            out.status.success(),
            "formats.py --displayed failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).expect("utf-8 oracle output");
        let oracle: Vec<&str> = stdout.lines().collect();
        assert_eq!(oracle.len(), 19, "18 file lines and a summary");

        let (mut shown, mut token, mut refused, mut mismatches) = (0, 0, 0, 0);
        for ((name, path), want) in names.iter().zip(&paths).zip(&oracle) {
            let wb = Workbook::from_file(path).expect("registered workbook");
            let c = wb.displayed_census();
            shown += c.shown;
            token += c.token;
            refused += c.refused;
            let got = format!(
                "{{\"file\":{},\"sha256\":\"{}\",\"shown\":{},\"token\":{},\"refused\":{},\"displayed_digest\":\"{}\"}}",
                js(name),
                hex(&wb.sha256),
                c.shown,
                c.token,
                c.refused,
                hex(&c.digest)
            );
            let equal = got == *want;
            let open = got.strip_suffix('}').expect("rendered object");
            println!("{open},\"oracle\":{want},\"pass\":{equal}}}");
            mismatches += usize::from(!equal);
            assert!(
                c.refusals
                    .iter()
                    .all(|r| r.contains(": general_non_integer: "))
            );
        }
        let summary =
            format!("{{\"files\":18,\"shown\":{shown},\"token\":{token},\"refused\":{refused}}}");
        println!("{summary}");
        assert_eq!(
            mismatches, 0,
            "displayed census differs from formats.py --displayed"
        );
        assert_eq!(summary, oracle[18]);
        assert_eq!(shown + token + refused, 30_388, "E820's numeric total");

        // E823: Loulo-Gounkoto's unrounded column K (Other K8, TCC K10, AISC K17) displays as integers. Q1 2026
        // stores -37.899642999999998, 132.100357, 133.100357; Q2 2026 stores -7.822133, 164.17786699999999,
        // 194.17786699999999. Rounded by hand to 0 decimals.
        for (file, sheet, want) in [
            (
                "Barrick_Q1_2026_AISC.xlsx",
                "Q1 2026",
                ["-38", "132", "133"],
            ),
            ("Barrick_Q2_2026_AISC.xlsx", "Q2 2026", ["-8", "164", "194"]),
            (
                "Barrick_Q2_2026_AISC.xlsx",
                "Q1 2026",
                ["-38", "132", "133"],
            ),
        ] {
            let wb = Workbook::from_file(&root.join(&profile.corpus_dir).join(file))
                .expect("registered workbook");
            let sh = wb.sheet(sheet).expect("sheet");
            let text = |r: u32| {
                sh.cell(r, 11)
                    .and_then(|c| c.displayed())
                    .map(|d| d.map(|d| d.text()).unwrap_or_default())
            };
            // No values in the failure message (RISK-31: the BD-339 CI log carries no issuer cell values).
            assert!(
                [text(8), text(10), text(17)] == want.map(|w| Some(w.to_string())),
                "{file} {sheet}: K8, K10 or K17 does not display as expected"
            );
        }
    }

    /// BD-331.S2 on the real files: every (iv), visible or hidden, names a missing fact. One JSON line per (iv)
    /// (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_every_iv_names_missing_fact() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        let mut by_fact: BTreeMap<(String, String), usize> = BTreeMap::new();
        for i in r.instances.iter().chain(&r.hidden_scope.instances) {
            let IdentityState::Undecidable { missing_fact, .. } = &i.state else {
                continue;
            };
            println!(
                "{{\"file\":{},\"sheet\":{},\"cell\":{},\"subject\":{},\"family\":{},\"missing_fact\":{}}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.cell),
                js(&i.subject),
                js(i.family.as_str()),
                js(missing_fact)
            );
            assert!(!missing_fact.is_empty());
            *by_fact
                .entry((i.subject.clone(), missing_fact.clone()))
                .or_default() += 1;
        }
        let visible_iv = r.count_state("iv");
        // 92 since BD-331.S4: ten regions lie outside their members' weighted means (region_census.py).
        assert_eq!(visible_iv, 92);
        let velad = (
            "Veladero".to_string(),
            "the ounce series behind the per-ounce rows (E691, E836)".to_string(),
        );
        // E837 (bd-10om): one text per subject, no longer a shared period-wide one.
        let kibali = "the cost of sales or ounce basis of its fourth-quarter rate: the printed ounces fall \
                      below what the rate implies (E817, E836, E837)";
        let hemlo = "the ounces behind its fourth-quarter and full-year rates: all three per-ounce rates \
                     imply fewer ounces than the ounces sold printed (E817, E836, E837)";
        let visible = |subject: &str, fact: &str| {
            r.instances
                .iter()
                .filter(|i| {
                    i.subject == subject
                        && matches!(&i.state, IdentityState::Undecidable { missing_fact, .. }
                            if missing_fact == fact)
                })
                .count()
        };
        assert_eq!(visible(&velad.0, &velad.1), 12);
        assert_eq!(visible("Kibali", kibali), 2);
        assert_eq!(visible("Hemlo", hemlo), 3);
        let share = r
            .instances
            .iter()
            .filter(|i| {
                matches!(&i.state, IdentityState::Undecidable { missing_fact, .. }
                    if missing_fact == aisc::SHARE_MISSING_FACT)
            })
            .count();
        assert_eq!(share, 65);
        let region = r
            .instances
            .iter()
            .filter(|i| {
                matches!(&i.state, IdentityState::Undecidable { missing_fact, .. }
                    if missing_fact == aisc::REGION_MISSING_FACT)
            })
            .count();
        assert_eq!(region, 10);
        println!(
            "{{\"visible_iv\":{visible_iv},\"hidden_iv\":{},\"by_subject_and_fact\":{}}}",
            r.hidden_scope
                .instances
                .iter()
                .filter(|i| i.state.code() == "iv")
                .count(),
            by_fact.len()
        );
    }

    /// bd-4wvl.4 (E815, E848) on the real files: the AISC tables' footings are `ComponentSum`, visible and hidden
    /// scopes apart. One JSON line per family and scope, then each footing that does not foot, as a locator.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn test_construction_discovery_real_aisc_tables() {
        use fmine_numerics::exact_feas::discover::Discovered;
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let r =
            run_public_assurance(&profile, &root).expect("registered files present and unaltered");
        // The lines the discovery counts are the ones the ledger summed.
        for i in r.instances.iter().chain(&r.hidden_scope.instances) {
            if let (Calc::Footing { component_sum, .. }, "i" | "iii") = (i.calc, i.state.code()) {
                let lines: i64 = i
                    .operands
                    .iter()
                    .filter_map(|o| match (o.role, &o.value) {
                        (OperandRole::Line, OperandValue::Int(x)) => Some(*x),
                        _ => None,
                    })
                    .sum();
                assert_eq!(lines, component_sum, "{}:{}!{}", i.file, i.sheet, i.cell);
            }
        }
        let found = r.construction_discovery().expect("discovery");
        for (scope, d) in &found {
            println!("{}", discovery_json(scope, d));
        }
        let mut not_footing = Vec::new();
        for (scope, instances) in [
            ("visible", &r.instances),
            ("hidden", &r.hidden_scope.instances),
        ] {
            for i in instances.iter().filter(|i| i.state.code() == "iii") {
                if let IdentityState::IssuerDiscrepancy(c) = &i.state {
                    let at = format!("{}:{}!{} {}", i.file, i.sheet, i.cell, i.subject);
                    println!(
                        "{{\"scope\":{},\"family\":{},\"not_footing\":{},\"sign_flip_witness\":{}}}",
                        js(scope),
                        js(i.family.as_str()),
                        js(&at),
                        c.sign_flip_witness
                            .as_deref()
                            .map_or_else(|| "null".to_string(), js)
                    );
                    not_footing.push((scope, at, c.sign_flip_witness.clone()));
                }
            }
        }
        // E815's three cells: Kibali and Porgera in both files that print Q2 2024, North America hidden only.
        let kibali = |f: &str| {
            (
                "visible",
                format!("Barrick_{f}_AISC.xlsx:Q2 2024!O11 Kibali"),
                Some("O7".to_string()),
            )
        };
        let porgera = |f: &str| {
            (
                "visible",
                format!("Barrick_{f}_AISC.xlsx:Q2 2024!L18 Porgera"),
                None,
            )
        };
        not_footing.sort();
        let mut want = vec![
            porgera("Q2_2025"),
            kibali("Q2_2025"),
            porgera("Q3_2024"),
            kibali("Q3_2024"),
            (
                "hidden",
                "Barrick_Q4_2024_AISC.xlsx:Q4 2023!J11 North America".to_string(),
                None,
            ),
        ];
        want.sort();
        assert_eq!(not_footing, want);
        // E838's census: footing i 1,010 and iii 4 visible; E828's hidden census: 170 scoreable, 169 consistent.
        let counts: Vec<(&str, &str, usize, usize, usize, usize)> = found
            .iter()
            .map(|(s, d)| {
                (
                    *s,
                    d.family.as_str(),
                    d.totals,
                    d.foot_exact,
                    d.report.n,
                    d.report.foot_exact,
                )
            })
            .collect();
        assert_eq!(
            counts,
            [
                ("visible", "tcc_footing", 507, 505, 371, 370),
                ("visible", "aisc_footing", 507, 505, 307, 306),
                ("hidden", "tcc_footing", 85, 84, 58, 57),
                ("hidden", "aisc_footing", 85, 85, 58, 58),
            ]
        );
        for (scope, d) in &found {
            assert_eq!(d.report.indeterminate, 0);
            assert_eq!(
                d.report.discovered,
                Discovered::ComponentSum,
                "{scope} {}",
                discovery_json(scope, d)
            );
        }
    }

    /// BD-331.S2 on the real files: the hidden scope. Every count is from the hidden census
    /// (`python3 -I scripts/oracle/hidden_census.py .`, which runs census2.py and share_census.py unchanged on
    /// the hidden sheets). One JSON line per hidden instance and per hidden sheet, then a summary (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn barrick_one_cell_matches_independent_reader() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let r =
            run_public_assurance(&profile, &root).expect("registered files present and unaltered");
        let out = std::process::Command::new("python3")
            .arg("-I")
            .arg(root.join("scripts/oracle/one_cell_cos.py"))
            .arg(&root)
            .current_dir(&root)
            .output()
            .expect("python3 runs");
        assert!(
            out.status.success(),
            "one_cell_cos.py failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).expect("utf-8 oracle output");
        let mut lines: Vec<&str> = stdout.lines().collect();
        let oracle_summary = lines.pop().expect("summary line");
        let key = |line: &str| {
            let f: Vec<&str> = line.splitn(4, "\", \"").collect();
            f[..3].join("|")
        };
        let oracle: BTreeMap<String, &str> = lines.iter().map(|l| (key(l), *l)).collect();
        assert_eq!(oracle.len(), lines.len(), "one oracle line per column");

        // The K.4 items (Appendix K.4 "Still open"; E691, E817, E819), as one_cell_cos.py states them.
        let year = |sheet: &str| {
            sheet
                .split(|c: char| !c.is_ascii_alphanumeric())
                .find(|t| {
                    t.len() == 4 && t.starts_with("20") && t.bytes().all(|b| b.is_ascii_digit())
                })
                .map(str::to_string)
        };
        let k4_item = |c: &crate::aisc::OneCellCheck| {
            let y = year(&c.sheet);
            match c.subject.as_str() {
                "Veladero" => matches!(y.as_deref(), Some("2024" | "2025")),
                "Hemlo" => matches!(c.sheet.as_str(), "Q4 2025" | "2025"),
                "North Mara" => matches!(c.sheet.as_str(), "Q2 2026" | "YTD Q2 2026"),
                "Latin America & Asia Pacific" => y.as_deref() == Some("2023"),
                _ => false,
            }
        };
        let text = |s: Option<&str>| s.map_or_else(|| "null".to_string(), |s| format!("\"{s}\""));
        let mut mismatches = Vec::new();
        for c in &r.one_cell {
            let (basis, reason, at_full, at_share, lo, hi) = match &c.outcome {
                OneCellOutcome::Refused {
                    reason,
                    at_full,
                    at_share,
                } => (None, Some(*reason), *at_full, *at_share, None, None),
                OneCellOutcome::Checked {
                    basis: (a, b),
                    at_full,
                    at_share,
                    implied_oz: (lo, hi),
                    ..
                } => (
                    Some(format!("{a}/{b}")),
                    None,
                    Some(*at_full),
                    *at_share,
                    Some(lo.as_str()),
                    Some(hi.as_str()),
                ),
            };
            let flag = |b: Option<bool>| b.map_or_else(|| "null".to_string(), |b| b.to_string());
            let got = format!(
                "{{\"file\": \"{}\", \"sheet\": \"{}\", \"column\": \"{}\", \"subject\": \"{}\", \"kind\": \"{}\", \
                 \"basis\": {}, \"state\": \"{}\", \"reason\": {}, \"at_full\": {}, \"at_share\": {}, \
                 \"implied_oz_lo\": {}, \"implied_oz_hi\": {}, \"k4_item\": {}}}",
                c.file,
                c.sheet,
                c.column,
                c.subject,
                if c.region { "region" } else { "site" },
                text(basis.as_deref()),
                c.state(),
                text(reason),
                flag(at_full),
                flag(at_share),
                text(lo),
                text(hi),
                k4_item(c)
            );
            // RISK-31: the lines carry implied ounces, so a mismatch is named by its key and field names only.
            let fields =
                |line: &str| -> Vec<String> { line.split(", \"").map(str::to_string).collect() };
            match oracle.get(&key(&got)) {
                Some(want) if *want == got => {}
                Some(want) => {
                    let differ: Vec<String> = fields(want)
                        .iter()
                        .zip(fields(&got))
                        .filter(|(w, g)| *w != g)
                        .map(|(w, _)| w.split("\":").next().unwrap_or("").to_string())
                        .collect();
                    mismatches.push(format!("{} differs in {differ:?}", key(&got)));
                }
                None => mismatches.push(format!("{} not in the oracle", key(&got))),
            }
        }
        assert_eq!(r.one_cell.len(), oracle.len(), "columns checked");
        assert!(
            mismatches.is_empty(),
            "differ from one_cell_cos.py: {mismatches:?}"
        );

        let v = crate::aisc::one_cell_verdict(&r.one_cell, &k4_item);
        let count = |state: &str| r.one_cell.iter().filter(|c| c.state() == state).count();
        let pattern: Vec<String> = v
            .pattern
            .iter()
            .map(|((s, (a, b), other), n)| {
                format!(
                    "{{\"subject\": \"{s}\", \"basis\": \"{a}/{b}\", \"passes_at_other_basis\": {}, \"columns\": {n}}}",
                    other.map_or_else(|| "null".to_string(), |o| o.to_string())
                )
            })
            .collect();
        let got = format!(
            "{{\"columns\": {}, \"pass\": {}, \"fail\": {}, \"refused\": {}, \"k4_fail\": {}, \"other_fail\": {}, \
             \"verdict\": \"{}\", \"pattern\": [{}]",
            r.one_cell.len(),
            count("pass"),
            count("fail"),
            count("refused"),
            v.k4_fail,
            v.other_fail,
            v.verdict,
            pattern.join(", ")
        );
        println!("{got}}}");
        assert!(
            oracle_summary.starts_with(&got),
            "summary differs from one_cell_cos.py"
        );
        assert_eq!(
            (r.one_cell.len(), count("pass"), v.fail, count("refused")),
            (519, 374, 82, 63)
        );
        assert_eq!((v.k4_fail, v.other_fail, v.verdict), (17, 65, "basis rule"));
        // Every failing column is a (iv) cost-of-sales-per-ounce instance of the ledger, and the reverse, but for
        // the regions refuted by their members (BD-331.S4), which the one-cell check leaves unbased.
        let mut fails: Vec<String> = r
            .one_cell
            .iter()
            .filter(|c| c.state() == "fail")
            .map(|c| format!("{}:{}!{}", c.file, c.sheet, c.column))
            .collect();
        let mut undecidable: Vec<String> = r
            .instances
            .iter()
            .filter(|i| i.family == IdentityFamily::CosPerOunce && i.state.code() == "iv")
            .filter(|i| i.state.reason_code() != Some("region_outside_members"))
            .map(|i| format!("{}:{}!{}", i.file, i.sheet, i.column))
            .collect();
        fails.sort_unstable();
        undecidable.sort_unstable();
        assert_eq!(fails, undecidable);
    }

    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_hidden_scope_matches_census() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        let hidden = &r.hidden_scope;
        let mut sheets: BTreeMap<(String, String), BTreeMap<&str, usize>> = BTreeMap::new();
        let (mut footing, mut per_oz) = (BTreeMap::new(), BTreeMap::new());
        for i in &hidden.instances {
            println!(
                "{{\"file\":{},\"sheet\":{},\"column\":{},\"subject\":{},\"family\":{},\"state\":{}}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.column),
                js(&i.subject),
                js(i.family.as_str()),
                js(i.state.code())
            );
            *sheets
                .entry((i.file.clone(), i.sheet.clone()))
                .or_default()
                .entry(i.state.code())
                .or_default() += 1;
            let tally = match i.family {
                IdentityFamily::TccFooting | IdentityFamily::AiscFooting => &mut footing,
                _ => &mut per_oz,
            };
            *tally.entry(i.state.code()).or_insert(0_usize) += 1;
        }
        for ((file, sheet), by_state) in &sheets {
            let states: Vec<String> = by_state
                .iter()
                .map(|(k, v)| format!("{}:{v}", js(k)))
                .collect();
            println!(
                "{{\"file\":{},\"sheet\":{},\"instances\":{},\"by_state\":{{{}}}}}",
                js(file),
                js(sheet),
                by_state.values().sum::<usize>(),
                states.join(",")
            );
        }
        // The census's numbers (2026-10-30): 6 hidden sheets, 435 instances. Its three regional rates in (v) are
        // decided by their members since BD-331.S4: `region_census.py --hidden` finds two not refuted (i) and one
        // refuted (iv), compared one by one in `barrick_regions_match_region_census`.
        let census_footing = BTreeMap::from([("i", 169), ("iii", 1), ("np", 4)]);
        let census_per_oz = BTreeMap::from([("i", 235 + 2), ("iv", 17 + 1), ("np", 6)]);
        let agrees = sheets.len() == 6
            && hidden.instances.len() == 435
            && footing == census_footing
            && per_oz == census_per_oz;
        println!(
            "{{\"hidden_sheets\":{},\"instances\":{},\"iii\":{},\"census_agrees\":{agrees}}}",
            sheets.len(),
            hidden.instances.len(),
            footing.get("iii").copied().unwrap_or(0)
        );
        assert_eq!((hidden.aisc_tables, sheets.len()), (6, 6));
        assert_eq!(hidden.instances.len(), 435);
        assert_eq!(footing, census_footing);
        assert_eq!(per_oz, census_per_oz);
        assert!(hidden.unregistered_headers.is_empty() && hidden.table_refusals.is_empty());
        // E815, E821: North America Q4 2023 total cash costs, hidden in the Q4 2024 file, prints 541 against 540.
        let na = hidden
            .instances
            .iter()
            .find(|i| i.state.code() == "iii")
            .expect("one (iii)");
        assert_eq!(
            (na.file.as_str(), na.sheet.as_str(), na.column.as_str()),
            ("Barrick_Q4_2024_AISC.xlsx", "Q4 2023", "J")
        );
        assert_eq!(
            (na.subject.as_str(), na.family),
            ("North America", IdentityFamily::TccFooting)
        );
        match &na.state {
            IdentityState::IssuerDiscrepancy(c) => {
                assert_eq!((c.printed, c.component_sum), (541, 540))
            }
            other => panic!("{other:?}"),
        }
        // Never counted: the visible ledger is unchanged by the hidden scope.
        assert_eq!((r.aisc_tables, r.instances.len()), (34, 2595));
    }

    /// BD-331.S3 on the real files. Every count is from the independent census E826
    /// (`python3 -I scripts/oracle/share_census.py .`, written without reading this crate). Prints one JSON line
    /// per cost-of-sales rate the rule decides, then a summary line (`--nocapture` to see them).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_share_rule_matches_census() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        // fit, miss, unconfirmed, regions_v, no_registered_share, no_footnote. Since BD-331.S4 the census's
        // regions_v are decided by their members, which `barrick_regions_match_region_census` checks one by one.
        let mut summary = [0_usize; 6];
        let mut by_subject: BTreeMap<String, [usize; 3]> = BTreeMap::new();
        for i in r
            .instances
            .iter()
            .filter(|i| i.family == IdentityFamily::CosPerOunce)
        {
            let slot = match &i.state {
                _ if region_record(i).is_some() => 3,
                IdentityState::Consistent {
                    basis: Some(ConsistentBasis::Share(_)),
                    ..
                } => 0,
                IdentityState::Undecidable {
                    why: Undecided::ShareOutside { .. },
                    ..
                } => 1,
                IdentityState::RefusedAt { .. } => 2,
                IdentityState::Refused {
                    reason: RefusedReason::RegionalNumerator,
                } => 3,
                IdentityState::Refused {
                    reason: RefusedReason::NoRegisteredShare,
                } => 4,
                s if *s == NO_FOOTNOTE => 5,
                _ => continue,
            };
            summary[slot] += 1;
            if slot < 3 {
                by_subject.entry(i.subject.clone()).or_default()[slot] += 1;
            }
            // Locators and codes only: the feasible share interval and the detail carry issuer values (RISK-31).
            println!(
                "{{\"file\":{},\"sheet\":{},\"cell\":{},\"subject\":{},\"state\":{},\"reason_code\":{},\
                 \"slot\":{slot}}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.cell),
                js(&i.subject),
                js(i.state.code()),
                js(i.state.reason_code().unwrap_or("none"))
            );
        }
        let [fit, miss, unconfirmed, regions_v, no_share, no_footnote] = summary;
        let sites = fit + miss + unconfirmed + no_share + no_footnote;
        println!(
            "{{\"sites\":{sites},\"fit\":{fit},\"miss\":{miss},\"unconfirmed\":{unconfirmed},\
             \"regions_v\":{regions_v},\"no_registered_share\":{no_share},\"no_footnote\":{no_footnote}}}"
        );
        assert_eq!((sites, summary), (328, [263, 65, 0, 51, 0, 0]));
        let census: BTreeMap<String, [usize; 3]> = [
            ("Bulyanhulu", 31, 3),
            ("Carlin", 9, 25),
            ("Cortez", 31, 3),
            ("Long Canyon", 3, 0),
            ("Loulo-Gounkoto", 23, 0),
            ("Nevada Gold Mines", 11, 23),
            ("North Mara", 30, 4),
            ("Phoenix", 34, 0),
            ("Pueblo Viejo", 29, 5),
            ("Tongon", 30, 0),
            ("Turquoise Ridge", 32, 2),
        ]
        .into_iter()
        .map(|(s, fit, miss)| (s.to_string(), [fit, miss, 0]))
        .collect();
        assert_eq!(by_subject, census);
    }

    /// The region census's record of an instance the region rule decided (BD-331.S4), as `[file, sheet, cell,
    /// region, verdict, reason, at, members, absent, excluded]` in Python's `json.dumps` form with the lists
    /// sorted (their order carries nothing); `None` for any other instance. A refusal's lists are empty (the
    /// Rust state carries none, so the census's are not compared), and a footing refusal names no cell (the
    /// census names none; the Rust names the region's ounces).
    fn region_record(i: &IdentityInstance) -> Option<String> {
        let (verdict, reason, at, weighting) = match &i.state {
            IdentityState::Consistent {
                basis: Some(ConsistentBasis::Members(w) | ConsistentBasis::MembersExact(w)),
                ..
            } => ("not_refuted", None, None, Some(w)),
            IdentityState::Undecidable {
                why: Undecided::RegionOutsideMembers { weighting, .. },
                ..
            } => ("refuted", None, None, Some(weighting)),
            IdentityState::Refused {
                reason: RefusedReason::RegionNoMembers,
            } => ("refused", Some("no_members"), None, None),
            IdentityState::RefusedAt {
                reason: RefusedReason::RegionOuncesDoNotFoot,
                ..
            } => ("refused", Some("region_ounces_do_not_foot"), None, None),
            IdentityState::RefusedAt {
                reason: RefusedReason::RegionMemberOperand,
                at,
            } => ("refused", Some("member_operand"), Some(at.as_str()), None),
            _ => return None,
        };
        let list = |names: Vec<&String>| {
            let mut names: Vec<String> = names.into_iter().map(|n| js(n)).collect();
            names.sort_unstable();
            format!("[{}]", names.join(", "))
        };
        let used = weighting.map_or_else(Vec::new, |w| w.used.iter().map(|m| &m.subject).collect());
        let absent = weighting.map_or_else(Vec::new, |w| w.absent.iter().collect());
        let excluded = weighting.map_or_else(Vec::new, |w| w.excluded.iter().collect());
        let opt = |s: Option<&str>| s.map_or_else(|| "null".to_string(), js);
        Some(format!(
            "[{}, {}, {}, {}, {}, {}, {}, {}, {}, {}]",
            js(&i.file),
            js(&i.sheet),
            js(&i.cell),
            js(&i.subject),
            js(verdict),
            opt(reason),
            opt(at),
            list(used),
            list(absent),
            list(excluded)
        ))
    }

    /// BD-331.S4 on the real files: the region rule decides exactly the cost-of-sales rates the stated-share
    /// census leaves to member weighting, visible and hidden, and each one's verdict, reason, cell and member
    /// lists equal the independent region census (`python3 -I scripts/oracle/region_census.py .` and
    /// `--hidden`, 57a5aa5, written without reading this crate), run here. Locators and names only (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn barrick_regions_match_region_census() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r =
            run_public_assurance(&profile, &root).expect("registered files present and unaltered");
        // The census's own output, each line re-printed with its lists sorted and a refusal's emptied, then
        // its summary line as printed.
        let script = "import json, subprocess, sys\n\
            out = subprocess.run([sys.executable, '-I', 'scripts/oracle/region_census.py', '.'] + sys.argv[1:], \
            capture_output=True, text=True, check=True).stdout.splitlines()\n\
            keys = ('file', 'sheet', 'cell', 'region', 'verdict', 'reason', 'at')\n\
            for line in out[:-1]:\n    x = json.loads(line)\n    \
            lists = [[] if x['verdict'] == 'refused' else sorted(x[k]) for k in ('members', 'absent', 'excluded')]\n    \
            print(json.dumps([x[k] for k in keys] + lists, ensure_ascii=False))\n\
            print(out[-1])\n";
        let scopes: [(&str, &[IdentityInstance], &[&str], &str); 2] = [
            (
                "visible",
                &r.instances,
                &[],
                r#"{"regions": 51, "refuted": 10, "not_refuted": 41, "refused": 0}"#,
            ),
            (
                "hidden",
                &r.hidden_scope.instances,
                &["--hidden"],
                r#"{"regions": 3, "refuted": 1, "not_refuted": 2, "refused": 0}"#,
            ),
        ];
        for (scope, instances, flags, summary) in scopes {
            let out = std::process::Command::new("python3")
                .args(["-I", "-c", script])
                .args(flags)
                .current_dir(&root)
                .output()
                .expect("python3 runs");
            assert!(
                out.status.success(),
                "region_census.py failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let stdout = String::from_utf8(out.stdout).expect("utf-8 census output");
            let mut census: Vec<&str> = stdout.lines().collect();
            let census_summary = census.pop().expect("summary line");
            census.sort_unstable();
            let mut ours: Vec<String> = instances.iter().filter_map(region_record).collect();
            ours.sort_unstable();
            for line in &ours {
                println!("{{\"scope\":{},\"record\":{line}}}", js(scope));
            }
            println!("{{\"scope\":{},\"census\":{census_summary}}}", js(scope));
            assert_eq!(census_summary, summary, "{scope}");
            assert_eq!(ours, census, "{scope}");
        }
        // The census's check is exact (vertex enumeration), and so is the ledger's since E845: every region the
        // census does not refute is (i) with a checked witness, none `relaxation_only`.
        let exact = |instances: &[IdentityInstance]| {
            instances
                .iter()
                .filter(|i| {
                    matches!(
                        i.state,
                        IdentityState::Consistent {
                            basis: Some(ConsistentBasis::MembersExact(_)),
                            ..
                        }
                    )
                })
                .count()
        };
        assert_eq!(
            (exact(&r.instances), exact(&r.hidden_scope.instances)),
            (41, 2)
        );
        assert_eq!(r.relaxation_only(), 0);
    }

    /// A share as the census prints it: the percentage as an exact decimal (`61.5`, `84`).
    fn percent_text(s: fmine_numerics::exact_feas::Rat) -> String {
        use fmine_numerics::exact_feas::Rat;
        let mut p = s.checked_mul(Rat::from_integer(100)).expect("percent");
        let mut places = 0;
        while p.den() != 1 {
            p = p.checked_mul(Rat::from_integer(10)).expect("decimal");
            places += 1;
            assert!(places < 8, "share {s:?} is not a short decimal");
        }
        let digits = format!("{:0>width$}", p.num(), width = places + 1);
        let (int, frac) = digits.split_at(digits.len() - places);
        if places == 0 {
            int.to_string()
        } else {
            format!("{int}.{frac}")
        }
    }

    /// One Mine Stats ledger instance in `mine_stats_census.py`'s line format. The census names the withheld
    /// exclusion by its evidence (`excluded_e818`) and writes cumulative periods with a space (`2026 H1`).
    fn mine_census_line(l: &mine_ledger::MineLedger, i: &mine_ledger::Instance) -> String {
        use crate::key::PeriodKind;
        use mine_ledger::{Reason, State};
        let opt = |s: Option<String>| s.map_or_else(|| "null".to_string(), |s| js(&s));
        let period = match i.period.kind {
            PeriodKind::Quarter(q) => format!("{}Q{q}", i.period.year),
            PeriodKind::H1 => format!("{} H1", i.period.year),
            PeriodKind::NineMonths => format!("{} 9M", i.period.year),
            PeriodKind::FullYear => format!("{} FY", i.period.year),
        };
        let (reason, at) = match &i.state {
            State::Refused { reason, at } => (
                Some(match reason {
                    Reason::Withheld { evidence } => {
                        format!("excluded_{}", evidence.to_lowercase())
                    }
                    other => other.code().to_string(),
                }),
                at.clone(),
            ),
            State::Unresolved(need) => (Some(need.code().to_string()), None),
            _ => (None, None),
        };
        let cells: Vec<String> = i.cells.iter().map(|c| opt(c.clone())).collect();
        let corrections = i.component.as_ref().map(|id| {
            let c = l
                .infeasible
                .iter()
                .find(|c| &c.id == id)
                .expect("component");
            let sets: Vec<String> = c
                .corrections
                .iter()
                .map(|s| {
                    format!(
                        "[{}]",
                        s.cells.iter().map(|x| js(x)).collect::<Vec<_>>().join(",")
                    )
                })
                .collect();
            format!("[{}]", sets.join(","))
        });
        format!(
            "{{\"family\":{},\"subject\":{},\"field\":{},\"group\":{},\"period\":{},\"anchor\":{},\"cells\":[{}],\
             \"state\":{},\"reason\":{},\"at\":{},\"certificate\":{},\"share\":{},\"printed_share\":{},\
             \"component\":{},\"corrections\":{}}}",
            js(i.family.as_str()),
            js(&i.subject),
            js(&i.field),
            js(i.group.as_str()),
            js(&period),
            js(&i.anchor),
            cells.join(","),
            js(i.state.code()),
            opt(reason),
            opt(at),
            opt(i.nil_token.then(|| "nil_token".to_string())),
            opt(i.share.map(percent_text)),
            opt(i.printed_share.clone()),
            opt(i.component.clone()),
            corrections.unwrap_or_else(|| "null".to_string())
        )
    }

    /// BD-331.S4 (bd-prmn.4): the Mine Stats ledger's ownership and quarter-sum instances equal the independent
    /// census (`scripts/oracle/mine_stats_census.py`, e66f7dc, written from the rule without reading this crate)
    /// line by line: states, reasons, operands, shares, components and every minimum correction set. Then the
    /// implied ranges of every set, compared in memory and named by component only (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn barrick_mine_ledger_matches_census() {
        use mine_ledger::Family;
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r =
            run_public_assurance(&profile, &root).expect("registered files present and unaltered");
        let l = &r.mine_ledger;
        let script = "import json, subprocess, sys\n\
            values = sys.argv[1:] == ['--values']\n\
            out = subprocess.run([sys.executable, '-I', 'scripts/oracle/mine_stats_census.py'] + sys.argv[1:] + \
            ['corpus_of_input_docs/downloaded/mine_stats'], capture_output=True, text=True, check=True).stdout\n\
            if not values:\n    print(out, end='')\n\
            else:\n    for line in out.splitlines():\n        x = json.loads(line)\n        \
            if 'joint_only' not in x or 'cells' not in x:\n            continue\n        \
            for k, d in enumerate(x['implied']):\n            for loc in sorted(d):\n                \
            lo, hi = d[loc]\n                \
            print('\\t'.join([x['component'], str(k), loc, 'unbounded' if 'unbounded' in (lo, hi) else lo + ',' + hi]))\n";
        let census = |flags: &[&str]| {
            let out = std::process::Command::new("python3")
                .args(["-I", "-c", script])
                .args(flags)
                .current_dir(&root)
                .output()
                .expect("python3 runs");
            assert!(
                out.status.success(),
                "mine_stats_census.py failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).expect("utf-8 census output")
        };
        let stdout = census(&[]);
        let lines: Vec<&str> = stdout.lines().collect();
        let instances: Vec<&str> = lines
            .iter()
            .copied()
            // An instance line is a family record with an anchor (E817's check line lists anchors too).
            .filter(|x| x.starts_with("{\"family\":") && x.contains("\"anchor\":"))
            .collect();
        let ours: Vec<String> = l.instances.iter().map(|i| mine_census_line(l, i)).collect();
        assert_eq!(ours.len(), instances.len(), "instance count");
        // A sum's operand order is a representation choice: the census lists members in its rule's map order,
        // the ledger in registry row order (they differ only for North America from 2025Q4). So a sum's member
        // cells are compared as a set, the aggregate's cell last. No sum here has an `at`, which order would set.
        let sum_cells_sorted = |line: &str| -> String {
            let sum = line.contains("\"family\":\"sites_to_region\"")
                || line.contains("\"family\":\"region_to_total\"");
            let Some(start) = line.find("\"cells\":[").map(|k| k + "\"cells\":[".len()) else {
                return line.to_string();
            };
            let Some(len) = line[start..].find(']') else {
                return line.to_string();
            };
            if !sum {
                return line.to_string();
            }
            let mut cells: Vec<&str> = line[start..start + len].split(',').collect();
            let aggregate = cells.pop();
            cells.sort_unstable();
            cells.extend(aggregate);
            format!(
                "{}{}{}",
                &line[..start],
                cells.join(","),
                &line[start + len..]
            )
        };
        // One log line per instance (the census's format: anchor, cells, family, state, reason, component,
        // implied corrections), printed before its comparison so a failing run shows every line up to the miss.
        for (a, b) in ours.iter().zip(&instances) {
            println!("{a}");
            assert_eq!(sum_cells_sorted(a), sum_cells_sorted(b));
        }
        // The sum families' counts (c697b13's census), pinned so that both sides cannot drift together.
        let counts = |f: Family| ["i", "iii", "v", "np"].map(|code| l.count(f, code));
        assert_eq!(counts(Family::SitesToRegion), [282, 0, 24, 0]);
        assert_eq!(counts(Family::RegionToTotal), [204, 0, 0, 0]);
        // Family 5 (ac07fee's census): i, iii, iv, v, np.
        let rate =
            ["i", "iii", "iv", "v", "np"].map(|code| l.count(Family::SoldWeightedRate, code));
        assert_eq!(rate, [252, 0, 30, 24, 0]);
        // Family 2 on the site sheets (9854b6c's census).
        assert_eq!(counts(Family::SiteQuarterSum), [2742, 8, 87, 683]);
        let components: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|x| x.contains("\"joint_only\":") && x.contains("\"instances\":"))
            .collect();
        let ours: Vec<String> = l
            .infeasible
            .iter()
            .map(|c| {
                let list = |xs: &[String]| xs.iter().map(|x| js(x)).collect::<Vec<_>>().join(",");
                let anchors: Vec<String> = c.instances.iter().map(|&k| l.instances[k].anchor.clone()).collect();
                let sets: Vec<String> = c.corrections.iter().map(|s| format!("[{}]", list(&s.cells))).collect();
                format!(
                    "{{\"component\":{},\"cells\":[{}],\"instances\":[{}],\"joint_only\":{},\"corrections\":[{}]}}",
                    js(&c.id),
                    list(&c.cells),
                    list(&anchors),
                    c.joint_only,
                    sets.join(",")
                )
            })
            .collect();
        assert_eq!(ours, components);
        assert!(
            l.infeasible.iter().all(|c| c.minimum),
            "every correction search exhaustive"
        );
        assert!(l.indeterminate.is_empty(), "{:?}", l.indeterminate);
        // The summary lines, as the census prints them.
        for family in Family::ALL {
            let mut by_reason: BTreeMap<String, usize> = BTreeMap::new();
            for i in l.instances.iter().filter(|i| i.family == family) {
                if let mine_ledger::State::Refused { reason, .. } = &i.state {
                    let code = match reason {
                        mine_ledger::Reason::Withheld { evidence } => {
                            format!("excluded_{}", evidence.to_lowercase())
                        }
                        other => other.code().to_string(),
                    };
                    *by_reason.entry(code).or_insert(0) += 1;
                }
            }
            let reasons: Vec<String> = by_reason
                .iter()
                .map(|(r, k)| format!("{}:{k}", js(r)))
                .collect();
            // Only family 5 can be (iv), and only its line counts it.
            let iv = if family == Family::SoldWeightedRate {
                format!(",\"iv\":{}", l.count(family, "iv"))
            } else {
                String::new()
            };
            let line = format!(
                "{{\"family\":{},\"n\":{},\"i\":{},\"iii\":{}{iv},\"v\":{},\"np\":{},\"v_by_reason\":{{{}}}}}",
                js(family.as_str()),
                l.instances.iter().filter(|i| i.family == family).count(),
                l.count(family, "i"),
                l.count(family, "iii"),
                l.count(family, "v"),
                l.count(family, "np"),
                reasons.join(",")
            );
            println!("{line}");
            assert!(lines.contains(&line.as_str()), "{line}");
        }
        let summary = format!(
            "{{\"components\":{},\"infeasible_components\":{},\"joint_only\":{}}}",
            l.components,
            l.infeasible.len(),
            l.infeasible.iter().filter(|c| c.joint_only).count()
        );
        println!("{summary}");
        assert!(lines.contains(&summary.as_str()), "{summary}");
        assert_eq!(
            summary,
            r#"{"components":2091,"infeasible_components":27,"joint_only":2}"#
        );

        // Implied ranges: one line per (component, set, cell), never printed.
        let census_ranges: Vec<String> =
            census(&["--values"]).lines().map(str::to_string).collect();
        let mut ours = Vec::new();
        for c in &l.infeasible {
            for (k, s) in c.corrections.iter().enumerate() {
                for (cell, range) in &s.implied {
                    let text = match range {
                        Ok(i) => {
                            assert!(
                                !i.lo_strict && !i.hi_strict,
                                "{}: closed boxes give closed ends",
                                c.id
                            );
                            format!(
                                "{},{}",
                                mine_ledger::rat_text(i.lo),
                                mine_ledger::rat_text(i.hi)
                            )
                        }
                        Err(why) => why.clone(),
                    };
                    ours.push(format!("{}\t{k}\t{cell}\t{text}", c.id));
                }
            }
        }
        assert!(!ours.is_empty(), "implied ranges compared");
        assert_eq!(ours.len(), census_ranges.len(), "implied range count");
        let differ: Vec<&str> = ours
            .iter()
            .zip(&census_ranges)
            .filter(|(a, b)| a != b)
            .filter_map(|(a, _)| a.split('\t').next())
            .collect();
        assert!(
            differ.is_empty(),
            "implied ranges differ in components {differ:?}"
        );

        // The plan's known answers (C1 DR-01, DR-02, DR-04): each a pair of (iii) on one shared cell.
        let state = |family: Family, anchor: &str| {
            l.instances
                .iter()
                .find(|i| i.family == family && i.anchor == anchor)
                .unwrap_or_else(|| panic!("no {} instance at {anchor}", family.as_str()))
        };
        for (e, own, sum, set) in [
            ("E113", "2026Q1:QTD!C5", "2026Q2:YTD!B5", "2026Q1:QTD!B5"),
            ("E114", "2026Q2:YTD!C7", "2026Q2:YTD!B7", "2026Q2:YTD!B7"),
            ("E115", "2026Q2:QTD!C28", "2026Q2:YTD!B28", "2026Q2:QTD!B28"),
        ] {
            let (o, s) = (
                state(Family::Ownership, own),
                state(Family::QuarterSum, sum),
            );
            assert_eq!((o.state.code(), s.state.code()), ("iii", "iii"), "{e}");
            assert_eq!(o.component, s.component, "{e}: one component");
            let c = l
                .infeasible
                .iter()
                .find(|c| Some(&c.id) == o.component.as_ref())
                .expect("component");
            let sets: Vec<&[String]> = c.corrections.iter().map(|s| s.cells.as_slice()).collect();
            assert_eq!(
                sets,
                [[set.to_string()].as_slice()],
                "{e}: the one minimum set"
            );
        }
        // E113: the range B5 can take from the other cells holds 281; the ledger never prints 281 as a value.
        let b5 = l
            .infeasible
            .iter()
            .flat_map(|c| &c.corrections)
            .flat_map(|s| &s.implied)
            .find(|(cell, _)| cell == "2026Q1:QTD!B5")
            .and_then(|(_, r)| r.as_ref().ok())
            .expect("B5's implied range");
        assert!(
            b5.contains(fmine_numerics::exact_feas::Rat::from_integer(281))
                .expect("exact")
        );
        // E116: the summary prints a number where the site sheet prints the nil token; decided as number = 0,
        // alone.
        let e116: Vec<&mine_ledger::Instance> = l
            .instances
            .iter()
            .filter(|i| i.family == Family::SiteSheet && i.anchor == "2026Q2:YTD!E11")
            .collect();
        assert_eq!(e116.len(), 1, "E116: one site_sheet instance");
        let e116 = e116.first().expect("E116");
        assert_eq!(e116.state.code(), "iii", "E116");
        assert!(e116.nil_token && e116.component.is_none(), "E116");
        assert_eq!(
            e116.cells,
            [
                Some("2026Q2:YTD!E11".to_string()),
                Some("2026Q2:Minestats1!N103".to_string())
            ],
            "E116"
        );
    }

    #[test]
    fn public_claim_from_a_synthetic_cell() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let aisc = r
            .sources
            .iter()
            .find(|s| s.file_name == synthetic::SYNTHETIC_AISC_FILE)
            .expect("aisc");
        let sheet = aisc
            .workbook
            .sheet(synthetic::SYNTHETIC_TABLE_SHEET)
            .expect("sheet");
        let cell = sheet.cell(5, 2).expect("B5");
        let claim = cell_to_public_claim(
            &aisc.workbook,
            &sheet.name,
            "cos_usd_m",
            cell,
            AdditivityClass::ExtensiveAdditive,
        )
        .expect("claim");
        assert_eq!(claim.value, Some(500.0));
        assert_eq!(claim.anchors[0].locator, "Q1 2030!B5");
        let dash = sheet.cell(6, 6).expect("F6");
        let placeholder = cell_to_public_claim(
            &aisc.workbook,
            &sheet.name,
            "dep",
            dash,
            AdditivityClass::ExtensiveAdditive,
        )
        .expect("placeholder");
        assert_eq!(placeholder.value, None);
        let err = sheet.cell(20, 2).expect("B20");
        assert!(
            cell_to_public_claim(
                &aisc.workbook,
                &sheet.name,
                "x",
                err,
                AdditivityClass::ExtensiveAdditive
            )
            .is_err()
        );
    }
}
