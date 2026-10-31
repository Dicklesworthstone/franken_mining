//! `DecisionCard`, flip-margin sensitivity (`BD-227`), EVSI/VOI campaign ranking (`BD-225`,
//! `GATE-HELM-02`), and `EpistemicOnePager` renderer (`§9.6`, `BD-052`, `BD-054`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

use fmine_claim::Claim;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{ClaimColor, Refusal};
use fmine_numerics::voi::{VoiProblem, VoiSummary};

/// Parameter perturbation threshold that flips the optimal action (`BD-227`).
#[derive(Debug, Clone, PartialEq)]
pub struct FlipMargin {
    pub parameter_name: String,
    pub baseline_value: f64,
    pub flip_value: f64,
    pub units: String,
    pub switches_to_action: String,
}

impl FlipMargin {
    /// Computes the linear break-even parameter value `flip_value` where an action with
    /// advantage `npv_advantage_usd_m` (`>= 0`) and linear sensitivity `d_npv_d_param` (`!= 0`)
    /// loses its optimality against `switches_to_action`.
    #[must_use]
    pub fn from_linear_sensitivity(
        parameter_name: impl Into<String>,
        baseline_value: f64,
        npv_advantage_usd_m: f64,
        d_npv_d_param: f64,
        units: impl Into<String>,
        switches_to_action: impl Into<String>,
    ) -> Self {
        let delta = if d_npv_d_param.abs() > 1e-12 {
            -npv_advantage_usd_m / d_npv_d_param
        } else {
            0.0
        };
        Self {
            parameter_name: parameter_name.into(),
            baseline_value,
            flip_value: baseline_value + delta,
            units: units.into(),
            switches_to_action: switches_to_action.into(),
        }
    }
}

/// Candidate information-gathering campaign (e.g., twin drillhole, met column, piezometer) (`BD-225`).
#[derive(Debug, Clone, PartialEq)]
pub struct VoiCampaign {
    pub campaign_id: String,
    pub description: String,
    pub cost_usd_m: f64,
    pub likelihood: Vec<Vec<f64>>,
}

/// Evaluated and ranked VOI campaign (`GATE-HELM-02`).
#[derive(Debug, Clone, PartialEq)]
pub struct VoiRankedItem {
    pub campaign_id: String,
    pub description: String,
    pub cost_usd_m: f64,
    pub summary: VoiSummary,
    pub benefit_cost_ratio: f64,
}

/// Evaluates and ranks candidate `VoiCampaign`s by descending `net_voi` (`EVSI - cost`),
/// breaking ties by `campaign_id` (`BD-225`, `GATE-HELM-02`).
#[must_use]
pub fn rank_voi_campaigns(problem: &VoiProblem, campaigns: &[VoiCampaign]) -> Vec<VoiRankedItem> {
    let mut items: Vec<VoiRankedItem> = campaigns
        .iter()
        .map(|c| {
            let summary = problem.compute_evsi(&c.likelihood, c.cost_usd_m);
            let bcr = if c.cost_usd_m > 1e-9 {
                summary.evsi / c.cost_usd_m
            } else {
                0.0
            };
            VoiRankedItem {
                campaign_id: c.campaign_id.clone(),
                description: c.description.clone(),
                cost_usd_m: c.cost_usd_m,
                summary,
                benefit_cost_ratio: bcr,
            }
        })
        .collect();

    items.sort_by(|a, b| {
        b.summary
            .net_voi
            .total_cmp(&a.summary.net_voi)
            .then_with(|| a.campaign_id.cmp(&b.campaign_id))
    });
    items
}

/// Executive and operational `DecisionCard` (`§22`, `BD-052`).
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionCard {
    pub decision_id: ObjectId,
    pub title: String,
    pub recommended_action: String,
    pub runner_up_action: String,
    pub expected_npv_usd_m: Claim<f64>,
    pub cvar95_loss_usd_m: f64,
    pub flip_margins: Vec<FlipMargin>,
    pub top_voi_items: Vec<VoiRankedItem>,
    pub color: ClaimColor,
}

impl DecisionCard {
    /// Constructs a validated `DecisionCard`, enforcing `INV-015` (`FMINE-REF-204` if `UU` slot is missing)
    /// and `NL-4` (`FMINE-REF-205` if `expected_npv_usd_m` is `Red`).
    pub fn build(
        title: impl Into<String>,
        recommended_action: impl Into<String>,
        runner_up_action: impl Into<String>,
        expected_npv_usd_m: Claim<f64>,
        cvar95_loss_usd_m: f64,
        flip_margins: Vec<FlipMargin>,
        top_voi_items: Vec<VoiRankedItem>,
    ) -> Result<Self, Refusal> {
        expected_npv_usd_m.verify_uu_slot_for_decision()?;
        let color = expected_npv_usd_m.require_decision_grade()?;
        let title = title.into();
        let recommended_action = recommended_action.into();
        let runner_up_action = runner_up_action.into();
        let tag = format!(
            "{title}:{recommended_action}:{}",
            expected_npv_usd_m.claim_id
        );
        let decision_id = ObjectId::from_canonical_bytes(PrefixKind::Decision, tag.as_bytes());

        Ok(Self {
            decision_id,
            title,
            recommended_action,
            runner_up_action,
            expected_npv_usd_m,
            cvar95_loss_usd_m,
            flip_margins,
            top_voi_items,
            color,
        })
    }

    /// Renders the `§9.6` `EpistemicOnePager` in canonical Markdown (`BD-054`).
    #[must_use]
    pub fn render_one_pager_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# Epistemic One-Pager: {} (`{}`)\n\n",
            self.title, self.decision_id
        ));
        out.push_str(&format!(
            "- **Status Color**: `{:?}` | **Evidence**: `{:?}` | **Knowledge**: `{:?}`\n",
            self.color, self.expected_npv_usd_m.evidence, self.expected_npv_usd_m.knowledge
        ));
        out.push_str(&format!(
            "- **Recommended Action**: {} (vs. {})\n",
            self.recommended_action, self.runner_up_action
        ));
        out.push_str(&format!(
            "- **Expected Value**: `${:.2}M` | **CVaR95 Tail Loss**: `${:.2}M`\n\n",
            self.expected_npv_usd_m.value, self.cvar95_loss_usd_m
        ));

        out.push_str("## Flip Margins (What Flips the Decision)\n");
        for fm in &self.flip_margins {
            out.push_str(&format!(
                "- **{}**: baseline `{:.3} {}` -> flips at `{:.3} {}` to `{}`\n",
                fm.parameter_name,
                fm.baseline_value,
                fm.units,
                fm.flip_value,
                fm.units,
                fm.switches_to_action
            ));
        }

        out.push_str("\n## Unknown-Unknown (`UU`) Structural Tail Scenarios\n");
        for uu in &self.expected_npv_usd_m.uu_terms {
            out.push_str(&format!(
                "- **{}** (`${:.1}M` downside): {} (Trigger: {})\n",
                uu.scenario_id, uu.downside_exposure_usd_m, uu.description, uu.trigger_indicator
            ));
        }

        out.push_str("\n## Ranked Value-of-Information (`VOI`) Campaigns\n");
        for v in &self.top_voi_items {
            out.push_str(&format!(
                "- **{}**: EVSI `${:.2}M`, Cost `${:.2}M`, Net VOI `${:.2}M` (BCR `{:.2}x`)\n",
                v.campaign_id,
                v.summary.evsi,
                v.cost_usd_m,
                v.summary.net_voi,
                v.benefit_cost_ratio
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::{
        AdditivityClass, ConfidentialityTier, Representation, SpatialSupport, UuTerm,
    };
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::{EvidenceClass, KnowledgeState};

    fn make_npv_claim(with_uu: bool, evidence: EvidenceClass) -> Claim<f64> {
        let uu = if with_uu {
            vec![UuTerm {
                scenario_id: "UU-GEO-07".to_string(),
                description: "Unmapped fault gouge in footwall".to_string(),
                downside_exposure_usd_m: 35.0,
                trigger_indicator: "RQD < 25% in pilot hole".to_string(),
            }]
        } else {
            vec![]
        };
        Claim::new(
            "pushback_4_npv",
            145.0,
            "145.0",
            Representation::Quantiles {
                p10: 80.0,
                p50: 145.0,
                p90: 210.0,
            },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            evidence,
            KnowledgeState::KnownUnknown,
            ConfidentialityTier::Internal,
            uu,
            vec![],
            vec![SourceAnchor::new("study.pdf", [0x22; 32], "Table 14-1")],
        )
    }

    #[test]
    fn test_decision_card_uu_and_flip_margin_guards() {
        // test_missing_uu_refuses_204: a missing UuTerm on a KnownUnknown claim refuses with FMINE-REF-204
        // (unbudgeted unknowns), never 404 (stale TSF instrumentation, a tailings-safety code).
        let bad_claim = make_npv_claim(false, EvidenceClass::Calibrated);
        let err = DecisionCard::build(
            "Pushback 4",
            "Proceed",
            "Defer",
            bad_claim,
            40.0,
            vec![],
            vec![],
        )
        .expect_err("must refuse missing UU");
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-204", 3));

        // A Placeholder claim refuses with FMINE-REF-205 (calibration required for decision-grade use), never
        // 401 (unknown reactive ground, a blasting-safety code).
        let red_claim = make_npv_claim(true, EvidenceClass::Placeholder);
        let err_red = DecisionCard::build(
            "Pushback 4",
            "Proceed",
            "Defer",
            red_claim,
            40.0,
            vec![],
            vec![],
        )
        .expect_err("must refuse Placeholder claim");
        assert_eq!((err_red.code, err_red.exit_code), ("FMINE-REF-205", 3));

        // Valid calibrated claim with UuTerm succeeds and computes flip margin.
        let ok_claim = make_npv_claim(true, EvidenceClass::Calibrated);
        let fm = FlipMargin::from_linear_sensitivity(
            "Au_Recovery",
            0.86,
            24.0,
            300.0,
            "fraction",
            "Leach Pad Only",
        );
        assert!((fm.flip_value - 0.78).abs() < 1e-9);
        let card = DecisionCard::build(
            "Pushback 4",
            "Proceed Mill",
            "Leach Pad Only",
            ok_claim,
            40.0,
            vec![fm],
            vec![],
        )
        .expect("valid card");
        assert_eq!(card.color, ClaimColor::Amber);
        let md = card.render_one_pager_markdown();
        assert!(md.contains("UU-GEO-07"));
        assert!(md.contains("Au_Recovery"));
    }

    #[test]
    fn test_voi_campaign_ranking_gate_helm_02() {
        let problem = VoiProblem {
            state_probs: vec![0.5, 0.5],
            payoffs: vec![vec![100.0, 0.0], vec![-80.0, 0.0]],
        };
        let campaigns = vec![
            VoiCampaign {
                campaign_id: "CAMP-LOW".to_string(),
                description: "Single scout hole".to_string(),
                cost_usd_m: 1.5,
                likelihood: vec![vec![0.65, 0.35], vec![0.35, 0.65]],
            },
            VoiCampaign {
                campaign_id: "CAMP-HIGH".to_string(),
                description: "3-hole HQ fence + met locked-cycle".to_string(),
                cost_usd_m: 3.0,
                likelihood: vec![vec![0.92, 0.08], vec![0.08, 0.92]],
            },
        ];
        let ranked = rank_voi_campaigns(&problem, &campaigns);
        assert_eq!(ranked[0].campaign_id, "CAMP-HIGH");
        assert!(ranked[0].summary.net_voi > ranked[1].summary.net_voi);
    }
}
