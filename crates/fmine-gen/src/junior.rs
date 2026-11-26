//! Determinism: BitwisePortable
//!
//! Synthetic junior drill-release generator and sealed-truth detection scorer (`BD-350.S5`,
//! `bd-gqu3.4`, plan §23.2 & §39 WP-15).
//!
//! Generates deterministic (`PhiloxStream` + `RngKey`) synthetic junior drill-release packs with
//! six sealed selective-reporting classes (`HeadlineCarriedByInclusion`, `OmittedHoles`,
//! `NoTrueWidth`, `MetalEquivalentUnstatedTerms`, `UnstatedCutoffOrDilution`, and `Clean`).
//! Ground truth (seed class, generated intervals, omitted holes, and the later full release) is
//! sealed inside [`crate::seal::SealedTruth`] so the auditing engine receives ONLY rendered text
//! and declared context — never seed labels (`GATE-GEN-01`).
//!
//! Attempting to read sealed junior truth directly fails at compile time:
//! ```compile_fail,E0616
//! fn leak_seed(sealed: &fmine_gen::seal::SealedTruth<fmine_gen::junior::JuniorWorldTruth>) -> fmine_gen::junior::JuniorSeedClass {
//!     sealed.truth.seed_class
//! }
//! ```
//!
//! Every generated document is explicitly labelled synthetic and never names or resembles any
//! real mining company or issuer.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_core::codec::Encoder;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_explore::intercept::{
    DrillReleaseAudit, GradeUnit, InclusionRelation, Intercept, LengthUnit, PrintedQty,
    ReleaseDisclosureContext, audit_drill_release,
};
use fmine_io::intercept::{ParseOutcome, parse_intercept_phrase};
use fmine_units::ExactQty;

use crate::GenError;
use crate::review::clopper_pearson;
use crate::seal::{SealKey, SealedTruth};

/// Explicit synthetic-only label attached to every generated junior release packet.
pub const SYNTHETIC_JUNIOR_LABEL: &str = "SYNTHETIC_JUNIOR_TESTPIT_ONLY: invented synthetic drill releases for detector validation; never a real company disclosure.";

/// Six selective-reporting classes from `BD-350.S5` (`bd-gqu3.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum JuniorSeedClass {
    /// Headline composite carried by high-grade inclusion with remainder strictly below cut-off.
    HeadlineCarriedByInclusion = 1,
    /// Omitted holes under `"results from n holes"` (`HYPOTHESIS`, scored against later full release).
    OmittedHoles = 2,
    /// Downhole length reported without a true-width statement (or with `"true width unknown"`).
    NoTrueWidth = 3,
    /// Metal-equivalent grade (`AuEq`) with unstated metal prices or recoveries.
    MetalEquivalentUnstatedTerms = 4,
    /// Composite cut-off, internal dilution, or top cut unstated.
    UnstatedCutoffOrDilution = 5,
    /// Clean control release with no selective-reporting seed (must raise zero flags).
    Clean = 6,
}

impl JuniorSeedClass {
    /// All six classes in canonical deterministic order.
    pub const ALL: [Self; 6] = [
        Self::HeadlineCarriedByInclusion,
        Self::OmittedHoles,
        Self::NoTrueWidth,
        Self::MetalEquivalentUnstatedTerms,
        Self::UnstatedCutoffOrDilution,
        Self::Clean,
    ];

    /// Stable machine-readable class name matching `DisclosureFlag::kind_str` (plus `"clean"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HeadlineCarriedByInclusion => "remainder_below_cutoff",
            Self::OmittedHoles => "hole_sequence_gap",
            Self::NoTrueWidth => "no_true_width",
            Self::MetalEquivalentUnstatedTerms => "metal_equivalent_unstated_terms",
            Self::UnstatedCutoffOrDilution => "unstated_cutoff_or_dilution",
            Self::Clean => "clean",
        }
    }

    /// Expected flag kind emitted by `fmine_explore::intercept` for seeded classes (`None` for `Clean`).
    #[must_use]
    pub const fn expected_flag_kind(self) -> Option<&'static str> {
        match self {
            Self::Clean => None,
            other => Some(other.as_str()),
        }
    }
}

/// Configuration for generating a synthetic junior drill-release campaign (`BD-350.S5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JuniorCampaignConfig {
    /// Deterministic Philox root seed (`RngKey::seed_root`).
    pub seed_root: u64,
    /// Replicates generated for each of the 6 seed classes (`1..=50`, bounded by exact Clopper–Pearson).
    pub replicates_per_class: u32,
    /// Nominal drilled hole count per synthetic junior release (`>= 4`).
    pub holes_per_release: u32,
}

impl JuniorCampaignConfig {
    fn validate(&self) -> Result<(), GenError> {
        if self.replicates_per_class == 0
            || self.replicates_per_class > crate::review::MAX_EXACT_CLOPPER_PEARSON_N
        {
            return Err(GenError::InvalidConfig {
                field: "replicates_per_class",
                detail: format!(
                    "must lie in 1..={}, got {}",
                    crate::review::MAX_EXACT_CLOPPER_PEARSON_N,
                    self.replicates_per_class
                ),
            });
        }
        if self.holes_per_release < 4 || self.holes_per_release > 64 {
            return Err(GenError::InvalidConfig {
                field: "holes_per_release",
                detail: format!("must lie in 4..=64, got {}", self.holes_per_release),
            });
        }
        Ok(())
    }
}

/// Engine-visible synthetic drill-release document.
///
/// Deliberately carries ZERO seed labels, ZERO omitted-hole lists, and ZERO truth fields
/// (`GATE-GEN-01`, `test_engine_input_has_no_seed_labels`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JuniorReleasePacket {
    pub doc_id: String,
    pub synthetic_label: &'static str,
    pub grade_basis: String,
    pub phrases: Vec<String>,
    pub context: ReleaseDisclosureContext,
}

impl JuniorReleasePacket {
    /// Render this synthetic packet as a `fmine.drill-release-check.v1` TOML worksheet suitable
    /// for offline CLI inspection (`fmine check drill-release <file>`).
    #[must_use]
    pub fn render_drill_release_worksheet_toml(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "schema = \"fmine.drill-release-check.v1\"");
        let _ = writeln!(
            out,
            "document = \"Synthetic junior pack {} ({})\"",
            self.doc_id, SYNTHETIC_JUNIOR_LABEL
        );
        let _ = writeln!(out, "evidence = \"synthetic\"");
        let _ = writeln!(out, "grade_basis = \"{}\"", self.grade_basis);
        if let Some((cutoff, unit)) = self.context.composite_cutoff {
            let _ = writeln!(out, "cutoff = \"{}/{}\"", cutoff.num(), cutoff.den());
            let _ = writeln!(out, "cutoff_unit = \"{}\"", unit.as_str());
        }
        if let Some(declared) = self.context.declared_hole_count {
            let _ = writeln!(out, "declared_holes = \"{declared}\"");
        }
        let _ = writeln!(
            out,
            "internal_dilution_stated = \"{}\"",
            self.context.internal_dilution_stated
        );
        let _ = writeln!(out, "top_cut_stated = \"{}\"", self.context.top_cut_stated);
        let _ = writeln!(
            out,
            "metal_eq_prices_stated = \"{}\"",
            self.context.metal_eq_prices_stated
        );
        let _ = writeln!(
            out,
            "metal_eq_recoveries_stated = \"{}\"",
            self.context.metal_eq_recoveries_stated
        );
        for (idx, phrase) in self.phrases.iter().enumerate() {
            let _ = writeln!(out, "\n[[phrase]]");
            let _ = writeln!(
                out,
                "locator = \"synthetic {}, line {}\"",
                self.doc_id,
                idx + 1
            );
            let _ = writeln!(out, "text = {}", json_quote(phrase));
            let has_tw = match parse_intercept_phrase(phrase, None) {
                ParseOutcome::Parsed(ic) => ic.true_width_stated,
                ParseOutcome::FallbackToCitedMode { .. } => false,
            };
            let _ = writeln!(out, "true_width_stated = \"{has_tw}\"");
        }
        out
    }
}

/// Sealed ground truth for one synthetic junior release (`GATE-GEN-01`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JuniorWorldTruth {
    pub doc_id: String,
    pub seed_class: JuniorSeedClass,
    pub generated_intercepts: Vec<Intercept>,
    pub omitted_holes: Vec<String>,
    /// Full release containing every drilled hole (used to score `HYPOTHESIS` hole-gap signals).
    pub later_full_release_phrases: Vec<String>,
    pub later_full_release_context: ReleaseDisclosureContext,
}

impl JuniorWorldTruth {
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"JUNT", 1);
        enc.put_str(&self.doc_id);
        enc.put_u8(self.seed_class as u8);
        enc.put_u64(self.generated_intercepts.len() as u64);
        for ic in &self.generated_intercepts {
            enc.put_str(&ic.hole);
            enc.put_bytes(&ic.length.value.num().to_le_bytes());
            enc.put_bytes(&ic.length.value.den().to_le_bytes());
            enc.put_bytes(&ic.grade.value.num().to_le_bytes());
            enc.put_bytes(&ic.grade.value.den().to_le_bytes());
            enc.put_u64(ic.children.len() as u64);
        }
        enc.put_u64(self.omitted_holes.len() as u64);
        for h in &self.omitted_holes {
            enc.put_str(h);
        }
        enc.put_u64(self.later_full_release_phrases.len() as u64);
        for p in &self.later_full_release_phrases {
            enc.put_str(p);
        }
        enc.finish()
    }
}

/// Output produced by the blind auditor operating solely on a [`JuniorReleasePacket`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JuniorAuditOutput {
    pub doc_id: String,
    pub parsed_intercepts: Vec<Intercept>,
    pub audit: DrillReleaseAudit,
    pub intercept_ndjson: Vec<String>,
}

/// Per-class detection rate summary with exact 90% Clopper–Pearson interval (`BD-350.S5`).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDetectionStats {
    pub seed_class: JuniorSeedClass,
    pub seeded_count: u32,
    pub detected_count: u32,
    pub rate: f64,
    pub cp90_lo: f64,
    pub cp90_hi: f64,
}

/// `HYPOTHESIS` scoring of `HoleSequenceGap` flags against sealed later full releases.
#[derive(Debug, Clone, PartialEq)]
pub struct HoleGapHypothesisScore {
    pub releases_with_omitted_holes: u32,
    pub true_positives: u32,
    pub false_positives: u32,
    pub false_negatives: u32,
    /// Fraction of sealed omitted-hole IDs actually recovered in the later full release and flagged.
    pub true_positive_rate_cp90: (f64, f64),
}

/// Full sealed-truth campaign scorecard (`BD-350.S5`, `bd-gqu3.4`).
#[derive(Debug, Clone, PartialEq)]
pub struct JuniorCampaignScorecard {
    pub synthetic_label: &'static str,
    pub releases: usize,
    pub clean_releases: usize,
    pub clean_flags: usize,
    pub by_class: BTreeMap<JuniorSeedClass, ClassDetectionStats>,
    pub hole_gap_hypothesis: HoleGapHypothesisScore,
    pub flag_ndjson: Vec<String>,
    pub summary_ndjson: String,
}

/// Sealed-truth harness for generating and scoring synthetic junior campaigns (`GATE-GEN-01`).
pub struct JuniorScoringHarness {
    key: SealKey,
}

impl JuniorScoringHarness {
    /// Mint a new harness with a private unforgeable `SealKey`.
    #[must_use]
    pub fn new(seed_root: u64) -> Self {
        Self {
            key: SealKey::mint(seed_root, "bd-350-s5-junior-harness"),
        }
    }

    /// Generate a deterministic synthetic junior campaign across all 6 seed classes.
    ///
    /// Every ground-truth label is sealed with this harness's private `SealKey` before returning.
    pub fn generate_campaign(
        &self,
        cfg: &JuniorCampaignConfig,
    ) -> Result<Vec<(JuniorReleasePacket, SealedTruth<JuniorWorldTruth>)>, GenError> {
        cfg.validate()?;
        let mut out =
            Vec::with_capacity(JuniorSeedClass::ALL.len() * (cfg.replicates_per_class as usize));
        let mut doc_seq = 0u32;
        for (class_idx, &seed_class) in JuniorSeedClass::ALL.iter().enumerate() {
            for rep in 0..cfg.replicates_per_class {
                doc_seq += 1;
                let rng_key = RngKey {
                    seed_root: cfg.seed_root,
                    partition: 0x4A55_4E31, // "JUN1"
                    domain: (class_idx as u32) + 1,
                    var_group: 1,
                    realization: rep,
                    brick: 0,
                };
                let (packet, truth) = generate_single_release(
                    rng_key,
                    doc_seq,
                    seed_class,
                    rep,
                    cfg.holes_per_release,
                )?;
                let sealed = SealedTruth::seal(
                    truth.doc_id.clone(),
                    cfg.seed_root,
                    &truth.canonical_bytes(),
                    &self.key,
                    truth,
                );
                out.push((packet, sealed));
            }
        }
        Ok(out)
    }

    /// Blindly parse and audit a [`JuniorReleasePacket`] using ONLY `fmine-io::intercept` and
    /// `fmine-explore::intercept` (never accepts `SealedTruth`).
    pub fn audit_packet(packet: &JuniorReleasePacket) -> Result<JuniorAuditOutput, Refusal> {
        let mut parsed = Vec::with_capacity(packet.phrases.len());
        for phrase in &packet.phrases {
            match parse_intercept_phrase(phrase, None) {
                ParseOutcome::Parsed(ic) => parsed.push(*ic),
                ParseOutcome::FallbackToCitedMode { raw_phrase, reason } => {
                    return Err(Refusal::new(
                        RefusalCode::SourceCellUnusable208,
                        format!(
                            "synthetic junior {}: phrase failed grammar round-trip ({reason}): {raw_phrase:?}",
                            packet.doc_id
                        ),
                    ));
                }
            }
        }
        let audit = audit_drill_release(&parsed, &packet.context)?;
        let intercept_ndjson = audit
            .intervals
            .iter()
            .map(|iv| format_doc_intercept_ndjson(&packet.doc_id, iv))
            .collect();
        Ok(JuniorAuditOutput {
            doc_id: packet.doc_id.clone(),
            parsed_intercepts: parsed,
            audit,
            intercept_ndjson,
        })
    }

    /// Open every seal using this harness's key and compute detection rates, 90% Clopper–Pearson
    /// intervals, clean false-alarm count, and `HoleSequenceGap` hypothesis scoring.
    pub fn score_campaign(
        &self,
        evaluated: &[(JuniorAuditOutput, SealedTruth<JuniorWorldTruth>)],
    ) -> Result<JuniorCampaignScorecard, GenError> {
        if evaluated.is_empty() {
            return Err(GenError::InvalidConfig {
                field: "evaluated",
                detail: "campaign scorecard requires at least one evaluated release".to_string(),
            });
        }

        let mut seeded_by_class: BTreeMap<JuniorSeedClass, u32> = BTreeMap::new();
        let mut detected_by_class: BTreeMap<JuniorSeedClass, u32> = BTreeMap::new();
        let mut clean_releases = 0usize;
        let mut clean_flags = 0usize;
        let mut hole_gap_seeded = 0u32;
        let mut hole_gap_tp = 0u32;
        let mut hole_gap_fp = 0u32;
        let mut hole_gap_fn = 0u32;
        let mut flag_ndjson = Vec::new();
        let mut seen = BTreeSet::new();

        for (output, sealed) in evaluated {
            let truth = sealed.open(&self.key)?;
            if output.doc_id != truth.doc_id {
                return Err(GenError::InvalidPrediction {
                    detail: format!(
                        "mismatched doc_id: output '{}' vs sealed '{}'",
                        output.doc_id, truth.doc_id
                    ),
                });
            }
            if !seen.insert(truth.doc_id.as_str()) {
                return Err(GenError::InvalidPrediction {
                    detail: format!("duplicate audited release '{}'", truth.doc_id),
                });
            }

            *seeded_by_class.entry(truth.seed_class).or_insert(0) += 1;

            let fired_hole_gap = output.audit.flags_by_kind.contains_key("hole_sequence_gap");
            let truth_has_omitted_in_full = !truth.omitted_holes.is_empty()
                && truth.later_full_release_phrases.len() > truth.generated_intercepts.len();
            if truth_has_omitted_in_full {
                hole_gap_seeded += 1;
                if fired_hole_gap {
                    hole_gap_tp += 1;
                } else {
                    hole_gap_fn += 1;
                }
            } else if fired_hole_gap {
                hole_gap_fp += 1;
            }

            match truth.seed_class.expected_flag_kind() {
                None => {
                    clean_releases += 1;
                    let total_flags: usize = output.audit.flags_by_kind.values().sum();
                    clean_flags += total_flags;
                    if total_flags == 0 {
                        *detected_by_class.entry(truth.seed_class).or_insert(0) += 1;
                    }
                    flag_ndjson.push(format!(
                        "{{\"event\":\"flag\",\"doc\":{},\"kind\":\"clean\",\"seeded\":false,\"detected\":{}}}",
                        json_quote(&output.doc_id),
                        total_flags > 0
                    ));
                }
                Some(expected_kind) => {
                    let detected = output.audit.flags_by_kind.contains_key(expected_kind);
                    if detected {
                        *detected_by_class.entry(truth.seed_class).or_insert(0) += 1;
                    }
                    flag_ndjson.push(format!(
                        "{{\"event\":\"flag\",\"doc\":{},\"kind\":{},\"seeded\":true,\"detected\":{}}}",
                        json_quote(&output.doc_id),
                        json_quote(expected_kind),
                        detected
                    ));
                }
            }
        }

        let mut by_class = BTreeMap::new();
        for (&class, &seeded) in &seeded_by_class {
            let detected = *detected_by_class.get(&class).unwrap_or(&0);
            let (cp90_lo, cp90_hi) =
                clopper_pearson(detected, seeded, 0.10).map_err(|e| GenError::InvalidConfig {
                    field: "clopper_pearson",
                    detail: e.what,
                })?;
            by_class.insert(
                class,
                ClassDetectionStats {
                    seed_class: class,
                    seeded_count: seeded,
                    detected_count: detected,
                    rate: f64::from(detected) / f64::from(seeded),
                    cp90_lo,
                    cp90_hi,
                },
            );
        }

        let hole_gap_cp90 = if hole_gap_seeded > 0 {
            clopper_pearson(hole_gap_tp, hole_gap_seeded, 0.10).map_err(|e| {
                GenError::InvalidConfig {
                    field: "hole_gap_hypothesis",
                    detail: e.what,
                }
            })?
        } else {
            (0.0, 1.0)
        };

        let summary_ndjson =
            format_summary_ndjson(evaluated.len(), clean_releases, clean_flags, &by_class);

        Ok(JuniorCampaignScorecard {
            synthetic_label: SYNTHETIC_JUNIOR_LABEL,
            releases: evaluated.len(),
            clean_releases,
            clean_flags,
            by_class,
            hole_gap_hypothesis: HoleGapHypothesisScore {
                releases_with_omitted_holes: hole_gap_seeded,
                true_positives: hole_gap_tp,
                false_positives: hole_gap_fp,
                false_negatives: hole_gap_fn,
                true_positive_rate_cp90: hole_gap_cp90,
            },
            flag_ndjson,
            summary_ndjson,
        })
    }
}

fn generate_single_release(
    rng_key: RngKey,
    doc_seq: u32,
    seed_class: JuniorSeedClass,
    replicate: u32,
    holes_per_release: u32,
) -> Result<(JuniorReleasePacket, JuniorWorldTruth), GenError> {
    let mut stream = PhiloxStream::new(rng_key);
    // Blind sequential identifiers: never encode seed_class in any engine-visible packet field.
    let doc_id = format!("SYN-JUN-DOC-{doc_seq:04}");
    let prefix = format!("SJ{doc_seq:03}-");
    let cutoff = ExactQty::new(1, 2).map_err(|_| GenError::InvalidConfig {
        field: "cutoff",
        detail: "invalid 0.50 cutoff".to_string(),
    })?;

    let is_metal_eq = matches!(seed_class, JuniorSeedClass::MetalEquivalentUnstatedTerms);
    let metal_sym = if is_metal_eq { "AuEq" } else { "Au" };
    let omit_true_width = matches!(seed_class, JuniorSeedClass::NoTrueWidth);

    // Choose which hole index (1-based) to omit for OmittedHoles (strictly interior so a sequence gap appears).
    let omitted_index: Option<u32> = if matches!(seed_class, JuniorSeedClass::OmittedHoles) {
        let interior_span = u64::from(holes_per_release.saturating_sub(2)).max(1);
        let picked = 2 + (stream.next_u64() % interior_span) as u32;
        Some(picked)
    } else {
        None
    };

    let mut released_phrases = Vec::new();
    let mut released_intercepts = Vec::new();
    let mut full_phrases = Vec::new();
    let mut omitted_holes = Vec::new();

    for hole_idx in 1..=holes_per_release {
        let hole_id = format!("{prefix}{hole_idx:03}");
        let use_within_form = (hole_idx + replicate) % 2 == 1;
        let seed_low_remainder =
            matches!(seed_class, JuniorSeedClass::HeadlineCarriedByInclusion) && hole_idx == 1;

        let (phrase, intercept) = build_synthetic_hole_intercept(
            &mut stream,
            &hole_id,
            metal_sym,
            is_metal_eq,
            use_within_form,
            seed_low_remainder,
            omit_true_width,
        )?;

        full_phrases.push(phrase.clone());
        if omitted_index == Some(hole_idx) {
            omitted_holes.push(hole_id);
        } else {
            released_phrases.push(phrase);
            released_intercepts.push(intercept);
        }
    }

    let mut context = ReleaseDisclosureContext::complete(cutoff, GradeUnit::GramsPerTonne);
    context.declared_hole_count = Some(holes_per_release);

    match seed_class {
        JuniorSeedClass::MetalEquivalentUnstatedTerms => {
            if replicate.is_multiple_of(2) {
                context.metal_eq_prices_stated = false;
            } else {
                context.metal_eq_recoveries_stated = false;
            }
        }
        JuniorSeedClass::UnstatedCutoffOrDilution => match replicate % 3 {
            0 => context.composite_cutoff = None,
            1 => context.internal_dilution_stated = false,
            _ => context.top_cut_stated = false,
        },
        JuniorSeedClass::HeadlineCarriedByInclusion
        | JuniorSeedClass::OmittedHoles
        | JuniorSeedClass::NoTrueWidth
        | JuniorSeedClass::Clean => {}
    }

    let mut full_context = ReleaseDisclosureContext::complete(cutoff, GradeUnit::GramsPerTonne);
    full_context.declared_hole_count = Some(holes_per_release);

    let packet = JuniorReleasePacket {
        doc_id: doc_id.clone(),
        synthetic_label: SYNTHETIC_JUNIOR_LABEL,
        grade_basis: metal_sym.to_string(),
        phrases: released_phrases,
        context,
    };
    let truth = JuniorWorldTruth {
        doc_id,
        seed_class,
        generated_intercepts: released_intercepts,
        omitted_holes,
        later_full_release_phrases: full_phrases,
        later_full_release_context: full_context,
    };
    Ok((packet, truth))
}

fn build_synthetic_hole_intercept(
    stream: &mut PhiloxStream,
    hole_id: &str,
    metal_sym: &str,
    is_metal_equivalent: bool,
    use_within_form: bool,
    seed_low_remainder: bool,
    omit_true_width: bool,
) -> Result<(String, Intercept), GenError> {
    // Integer metres guarantee clean exact decimal arithmetic without rounding ambiguity.
    let rem_len_m = 10 + (stream.next_u64() % 6) as i128; // 10..=15 m
    let child_len_m = 1 + (stream.next_u64() % 2) as i128; // 1..=2 m
    let host_len_m = rem_len_m + child_len_m;

    let (rem_grade_cents, child_grade_cents) = if seed_low_remainder {
        // Remainder in [0.10, 0.18] g/t, strictly below 0.50 g/t cut-off across the full rounding box.
        let r = 10 + (stream.next_u64() % 9) as i128;
        let c = 1200 + (stream.next_u64() % 600) as i128; // 12.00..17.99 g/t
        (r, c)
    } else {
        // Clean remainder in [2.20, 3.80] g/t, strictly above 0.50 g/t; child in [12.00, 20.00] > host.
        let r = 220 + (stream.next_u64() % 161) as i128;
        let c = 1200 + (stream.next_u64() % 801) as i128;
        (r, c)
    };

    // Host composite grade rounded to 2 decimal places (hundredths of g/t):
    let total_metal_cents = rem_len_m * rem_grade_cents + child_len_m * child_grade_cents;
    let host_grade_cents = (total_metal_cents + host_len_m / 2) / host_len_m;

    let host_len_str = format!("{host_len_m}.00");
    let child_len_str = format!("{child_len_m}.00");
    let host_grade_str = format_cents(host_grade_cents);
    let child_grade_str = format_cents(child_grade_cents);

    let from_m = 80 + (stream.next_u64() % 120) as i128;
    let to_m = from_m + host_len_m;
    let child_from_m = from_m + 2;
    let child_to_m = child_from_m + child_len_m;
    let true_width_m = (host_len_m * 8) / 10;

    let tw_suffix = if omit_true_width {
        if stream.next_u64().is_multiple_of(2) {
            String::new()
        } else {
            " (true width unknown)".to_string()
        }
    } else {
        format!(" (true width {true_width_m} m)")
    };

    let (phrase, child_rel, include_spans) = if use_within_form {
        (
            format!(
                "Hole {hole_id}: {child_len_str} m @ {child_grade_str} g/t {metal_sym} within {host_len_str} m @ {host_grade_str} g/t {metal_sym}{tw_suffix}"
            ),
            InclusionRelation::Within,
            false,
        )
    } else {
        (
            format!(
                "{hole_id}: {host_len_str} m @ {host_grade_str} g/t {metal_sym} from {from_m}.00 m to {to_m}.00 m including {child_len_str} m @ {child_grade_str} g/t {metal_sym} from {child_from_m}.00 m to {child_to_m}.00 m{tw_suffix}"
            ),
            InclusionRelation::Including,
            true,
        )
    };

    let parse_qty = |s: &str| -> Result<PrintedQty, GenError> {
        PrintedQty::parse_decimal(s).map_err(|e| GenError::InvalidConfig {
            field: "printed_qty",
            detail: e.what,
        })
    };

    let child_ic = Intercept {
        hole: hole_id.to_string(),
        from: if include_spans {
            Some(parse_qty(&format!("{child_from_m}.00"))?)
        } else {
            None
        },
        to: if include_spans {
            Some(parse_qty(&format!("{child_to_m}.00"))?)
        } else {
            None
        },
        length: parse_qty(&child_len_str)?,
        length_unit: LengthUnit::Metres,
        grade: parse_qty(&child_grade_str)?,
        grade_unit: GradeUnit::GramsPerTonne,
        metal_symbol: Some(metal_sym.to_string()),
        is_metal_equivalent,
        is_downhole_length: true,
        true_width_stated: !omit_true_width,
        relation: child_rel,
        children: Vec::new(),
    };
    let host_ic = Intercept {
        hole: hole_id.to_string(),
        from: if include_spans {
            Some(parse_qty(&format!("{from_m}.00"))?)
        } else {
            None
        },
        to: if include_spans {
            Some(parse_qty(&format!("{to_m}.00"))?)
        } else {
            None
        },
        length: parse_qty(&host_len_str)?,
        length_unit: LengthUnit::Metres,
        grade: parse_qty(&host_grade_str)?,
        grade_unit: GradeUnit::GramsPerTonne,
        metal_symbol: Some(metal_sym.to_string()),
        is_metal_equivalent,
        is_downhole_length: true,
        true_width_stated: !omit_true_width,
        relation: InclusionRelation::Host,
        children: vec![child_ic],
    };

    Ok((phrase, host_ic))
}

fn format_cents(cents: i128) -> String {
    format!("{}.{:02}", cents / 100, (cents % 100).abs())
}

fn format_doc_intercept_ndjson(
    doc_id: &str,
    iv: &fmine_explore::intercept::InterceptAudit,
) -> String {
    let (rem_lo, rem_hi) = match &iv.remainder {
        Some(r) => (
            format!(
                "\"{}/{}\"",
                r.remainder_grade_interval.lo.num(),
                r.remainder_grade_interval.lo.den()
            ),
            format!(
                "\"{}/{}\"",
                r.remainder_grade_interval.hi.num(),
                r.remainder_grade_interval.hi.den()
            ),
        ),
        None => ("null".to_string(), "null".to_string()),
    };
    let flags: Vec<String> = iv
        .flags
        .iter()
        .map(|f| format!("\"{}\"", f.kind_str()))
        .collect();
    format!(
        "{{\"event\":\"intercept\",\"doc\":{},\"hole\":{},\"remainder_lo\":{},\"remainder_hi\":{},\"flags\":[{}]}}",
        json_quote(doc_id),
        json_quote(&iv.hole),
        rem_lo,
        rem_hi,
        flags.join(",")
    )
}

fn format_summary_ndjson(
    releases: usize,
    clean_releases: usize,
    clean_flags: usize,
    by_class: &BTreeMap<JuniorSeedClass, ClassDetectionStats>,
) -> String {
    let detected_entries: Vec<String> = by_class
        .values()
        .map(|s| format!("\"{}\":{}", s.seed_class.as_str(), s.detected_count))
        .collect();
    let cp90_entries: Vec<String> = by_class
        .values()
        .map(|s| {
            format!(
                "\"{}\":[{:.4},{:.4}]",
                s.seed_class.as_str(),
                s.cp90_lo,
                s.cp90_hi
            )
        })
        .collect();
    format!(
        "{{\"event\":\"junior_summary\",\"releases\":{},\"clean\":{},\"clean_flags\":{},\"by_class_detected\":{{{}}},\"by_class_rate_cp90\":{{{}}}}}",
        releases,
        clean_releases,
        clean_flags,
        detected_entries.join(","),
        cp90_entries.join(",")
    )
}

fn json_quote(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cfg() -> JuniorCampaignConfig {
        JuniorCampaignConfig {
            seed_root: 0xBD35_0505_u64,
            replicates_per_class: 10,
            holes_per_release: 6,
        }
    }

    #[test]
    fn test_clean_synthetic_releases_raise_zero_flags() {
        let cfg = test_cfg();
        let harness = JuniorScoringHarness::new(cfg.seed_root);
        let campaign = harness.generate_campaign(&cfg).expect("generate campaign");
        let evaluated: Vec<_> = campaign
            .into_iter()
            .map(|(packet, sealed)| {
                let out = JuniorScoringHarness::audit_packet(&packet).expect("audit packet");
                (out, sealed)
            })
            .collect();
        let scorecard = harness.score_campaign(&evaluated).expect("score campaign");
        assert_eq!(scorecard.clean_releases, 10);
        assert_eq!(
            scorecard.clean_flags, 0,
            "clean synthetic releases must raise zero flags across all replicates"
        );
    }

    #[test]
    fn test_each_seed_class_detected_with_rate() {
        let cfg = test_cfg();
        let harness = JuniorScoringHarness::new(cfg.seed_root);
        let campaign = harness.generate_campaign(&cfg).expect("generate campaign");
        let evaluated: Vec<_> = campaign
            .into_iter()
            .map(|(packet, sealed)| {
                let out = JuniorScoringHarness::audit_packet(&packet).expect("audit packet");
                (out, sealed)
            })
            .collect();
        let scorecard = harness.score_campaign(&evaluated).expect("score campaign");
        assert_eq!(scorecard.releases, 60);

        for &class in &JuniorSeedClass::ALL {
            let stats = scorecard
                .by_class
                .get(&class)
                .expect("every seed class present in scorecard");
            assert_eq!(stats.seeded_count, 10);
            assert_eq!(
                stats.detected_count, 10,
                "expected 10/10 detection for class {:?}",
                class
            );
            assert!((stats.rate - 1.0).abs() < 1e-12);
            assert!(
                stats.cp90_lo > 0.70 && stats.cp90_hi <= 1.0,
                "90% Clopper-Pearson interval for 10/10 should be [~0.74, 1.0], got [{}, {}]",
                stats.cp90_lo,
                stats.cp90_hi
            );
        }

        assert!(
            scorecard
                .summary_ndjson
                .contains("\"event\":\"junior_summary\"")
        );
        assert!(
            scorecard
                .flag_ndjson
                .iter()
                .all(|l| l.contains("\"event\":\"flag\""))
        );
    }

    #[test]
    fn test_hole_gap_hypothesis_scored_against_full_release() {
        let cfg = test_cfg();
        let harness = JuniorScoringHarness::new(cfg.seed_root);
        let campaign = harness.generate_campaign(&cfg).expect("generate campaign");
        let mut evaluated = Vec::with_capacity(campaign.len());
        for (packet, sealed) in campaign {
            let out = JuniorScoringHarness::audit_packet(&packet).expect("audit packet");
            evaluated.push((out, sealed));
        }

        let scorecard = harness.score_campaign(&evaluated).expect("score campaign");
        assert_eq!(
            scorecard.hole_gap_hypothesis.releases_with_omitted_holes,
            10
        );
        assert_eq!(scorecard.hole_gap_hypothesis.true_positives, 10);
        assert_eq!(scorecard.hole_gap_hypothesis.false_positives, 0);
        assert_eq!(scorecard.hole_gap_hypothesis.false_negatives, 0);
        assert!(scorecard.hole_gap_hypothesis.true_positive_rate_cp90.0 > 0.70);

        // Verify the sealed later full releases themselves raise zero hole-sequence-gap flags.
        for (_, sealed) in &evaluated {
            let truth = sealed.open(&harness.key).expect("open own seal");
            if truth.seed_class == JuniorSeedClass::OmittedHoles {
                let full_packet = JuniorReleasePacket {
                    doc_id: format!("{}-full", truth.doc_id),
                    synthetic_label: SYNTHETIC_JUNIOR_LABEL,
                    grade_basis: "Au".to_string(),
                    phrases: truth.later_full_release_phrases.clone(),
                    context: truth.later_full_release_context.clone(),
                };
                let full_out =
                    JuniorScoringHarness::audit_packet(&full_packet).expect("audit full release");
                assert!(
                    full_out.audit.is_clean(),
                    "later full release should clear all hole-gap flags, got {:?}",
                    full_out.audit.flags_by_kind
                );
            }
        }
    }

    #[test]
    fn test_rendered_release_round_trips_through_grammar() {
        let cfg = JuniorCampaignConfig {
            seed_root: 0x1234_5678,
            replicates_per_class: 4,
            holes_per_release: 5,
        };
        let harness = JuniorScoringHarness::new(cfg.seed_root);
        let campaign = harness.generate_campaign(&cfg).expect("generate campaign");
        for (packet, sealed) in &campaign {
            let out = JuniorScoringHarness::audit_packet(packet).expect("grammar round-trip");
            let truth = sealed.open(&harness.key).expect("open seal");
            assert_eq!(
                out.parsed_intercepts, truth.generated_intercepts,
                "rendered phrases in {} must round-trip identically to generated intercepts",
                packet.doc_id
            );
            let toml = packet.render_drill_release_worksheet_toml();
            assert!(toml.contains("schema = \"fmine.drill-release-check.v1\""));
            assert!(toml.contains("SYNTHETIC_JUNIOR_TESTPIT_ONLY"));
        }
    }

    #[test]
    fn test_generator_deterministic_under_rng_key() {
        let cfg = test_cfg();
        let h1 = JuniorScoringHarness::new(cfg.seed_root);
        let h2 = JuniorScoringHarness::new(cfg.seed_root);
        let c1 = h1.generate_campaign(&cfg).expect("c1");
        let c2 = h2.generate_campaign(&cfg).expect("c2");
        assert_eq!(c1.len(), c2.len());
        for ((p1, s1), (p2, s2)) in c1.iter().zip(c2.iter()) {
            assert_eq!(p1, p2);
            assert_eq!(s1.truth_commitment(), s2.truth_commitment());
        }
    }

    #[test]
    fn test_campaign_refuses_a_duplicate_release() {
        let cfg = test_cfg();
        let harness = JuniorScoringHarness::new(cfg.seed_root);
        let mut evaluated = Vec::new();
        for _ in 0..2 {
            let (packet, sealed) = harness
                .generate_campaign(&cfg)
                .expect("generate campaign")
                .into_iter()
                .next()
                .expect("first release");
            evaluated.push((
                JuniorScoringHarness::audit_packet(&packet).expect("audit"),
                sealed,
            ));
        }
        assert!(matches!(
            harness.score_campaign(&evaluated),
            Err(GenError::InvalidPrediction { detail })
                if detail.contains("duplicate audited release")
        ));
    }

    #[test]
    fn test_engine_input_has_no_seed_labels() {
        let cfg = JuniorCampaignConfig {
            seed_root: 99,
            replicates_per_class: 2,
            holes_per_release: 4,
        };
        let harness = JuniorScoringHarness::new(cfg.seed_root);
        let intruder = JuniorScoringHarness::new(cfg.seed_root + 1);
        let campaign = harness.generate_campaign(&cfg).expect("campaign");
        let (packet, sealed) = &campaign[0];

        // 1. SealedTruth Debug output redacts truth; intruder SealKey is refused.
        let dbg = format!("{sealed:?}");
        assert!(dbg.contains("<sealed>"));
        assert!(!dbg.contains("HeadlineCarriedByInclusion"));
        assert!(
            intruder
                .score_campaign(&[(
                    JuniorScoringHarness::audit_packet(packet).expect("audit"),
                    SealedTruth::seal(
                        "forged",
                        cfg.seed_root,
                        b"forged",
                        &harness.key,
                        sealed.open(&harness.key).expect("open").clone(),
                    ),
                )])
                .is_err()
        );

        // 2. Packet fields contain only synthetic text & disclosure context, never seed labels or real issuers.
        let packet_dbg = format!("{packet:?}");
        for forbidden in [
            "Barrick",
            "Newmont",
            "AngloGold",
            "HeadlineCarriedByInclusion",
            "OmittedHoles",
            "NoTrueWidth",
            "remainder_below_cutoff",
            "hole_sequence_gap",
            "no_true_width",
            "metal_equivalent_unstated_terms",
            "unstated_cutoff_or_dilution",
            "clean",
        ] {
            assert!(
                !packet_dbg.contains(forbidden),
                "engine input leaked forbidden token {forbidden:?}"
            );
        }
    }
}
