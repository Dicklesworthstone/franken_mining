//! `DecisionCard`, flip-margin sensitivity (`BD-227`), EVSI/VOI campaign ranking (`BD-225`,
//! `GATE-HELM-02`), and `EpistemicOnePager` renderer (`§9.6`, `BD-052`, `BD-054`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

use std::collections::BTreeSet;

use fmine_claim::Claim;
use fmine_claim::dag::replay::ReplayCertificate;
use fmine_claim::dag::{CheckedCone, ConeCut, TemporalGraph};
use fmine_core::codec::Encoder;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{Color, Refusal, RefusalCode};
use fmine_numerics::voi::{VoiAnalysis, VoiError, VoiExperiment, VoiProblem, VoiSummary};

pub mod card;

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
    /// Full conditional decision policy from the same evaluation as `summary`.
    pub analysis: VoiAnalysis,
    /// Undefined for every zero-cost campaign, including one with zero EVSI.
    pub benefit_cost_ratio: Option<f64>,
}

/// Evaluates and ranks candidate `VoiCampaign`s by descending `net_voi` (`EVSI - cost`),
/// breaking ties by `campaign_id` (`BD-225`). The finite model is validated once; any invalid model,
/// campaign identity, likelihood or cost refuses the entire ranking. Results are conditional on the
/// supplied model and do not establish calibration or the Monte Carlo `GATE-HELM-02`.
pub fn rank_voi_campaigns(
    problem: &VoiProblem,
    campaigns: &[VoiCampaign],
) -> Result<Vec<VoiRankedItem>, Refusal> {
    let validated = problem.validate().map_err(voi_refusal)?;
    let mut identifiers = BTreeSet::new();
    for campaign in campaigns {
        if campaign.campaign_id.trim().is_empty()
            || !identifiers.insert(campaign.campaign_id.as_str())
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "VOI campaigns require nonempty, distinct campaign identifiers",
            ));
        }
    }
    let mut items = campaigns
        .iter()
        .map(|c| {
            let analysis = validated
                .evaluate(VoiExperiment {
                    likelihood: &c.likelihood,
                    cost: c.cost_usd_m,
                })
                .map_err(voi_refusal)?;
            let benefit_cost_ratio = if c.cost_usd_m > 0.0 {
                let ratio = analysis.summary.evsi / c.cost_usd_m;
                if !ratio.is_finite() || (analysis.summary.evsi > 0.0 && ratio == 0.0) {
                    return Err(Refusal::new(
                        RefusalCode::NumericalCertificateFailed207,
                        format!(
                            "VOI campaign {} benefit-cost ratio is not representable",
                            c.campaign_id
                        ),
                    ));
                }
                Some(ratio)
            } else {
                None
            };
            Ok(VoiRankedItem {
                campaign_id: c.campaign_id.clone(),
                description: c.description.clone(),
                cost_usd_m: c.cost_usd_m,
                summary: analysis.summary,
                analysis,
                benefit_cost_ratio,
            })
        })
        .collect::<Result<Vec<_>, Refusal>>()?;

    items.sort_by(|a, b| {
        b.summary
            .net_voi
            .total_cmp(&a.summary.net_voi)
            .then_with(|| a.campaign_id.cmp(&b.campaign_id))
    });
    Ok(items)
}

fn voi_refusal(error: VoiError) -> Refusal {
    Refusal::new(
        if error.is_numerical_failure() {
            RefusalCode::NumericalCertificateFailed207
        } else {
            RefusalCode::OutsidePhysicalDomain109
        },
        format!("VOI model: {error}"),
    )
}

fn encode_voi_summary(enc: &mut Encoder, summary: VoiSummary) {
    enc.put_u64(summary.prior_best_action as u64);
    for value in [
        summary.prior_expected_value,
        summary.perfect_info_expected_value,
        summary.evpi,
        summary.evsi,
        summary.net_voi,
    ] {
        enc.put_u64(value.to_bits());
    }
}

fn encode_values(enc: &mut Encoder, values: &[f64]) {
    enc.put_u64(values.len() as u64);
    for value in values {
        enc.put_u64(value.to_bits());
    }
}

fn encode_optional_values(enc: &mut Encoder, values: Option<&[f64]>) {
    match values {
        Some(values) => {
            enc.put_u8(1);
            encode_values(enc, values);
        }
        None => enc.put_u8(0),
    }
}

fn encode_indices(enc: &mut Encoder, indices: &[usize]) {
    enc.put_u64(indices.len() as u64);
    for index in indices {
        enc.put_u64(*index as u64);
    }
}

fn display_values(values: &[f64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("{value:.6}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
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
    pub color: Color,
}

/// An immutable card snapshot whose expected-NPV provenance passed the historical cut and whose output was
/// reproduced over the filtered store (`BD-344`, `G-HELM-4`). The legacy ancillary card fields have no separate
/// Claim provenance: replay of their bytes does not establish that their inputs were completely declared.
///
/// The checked card cannot be replaced or edited after validation:
///
/// ```compile_fail,E0616
/// fn replace(checked: &mut fmine_decide::RetrospectiveDecisionCard, card: fmine_decide::DecisionCard) {
///     checked.card = card;
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RetrospectiveDecisionCard {
    card: DecisionCard,
    cone: CheckedCone,
    replay: ReplayCertificate,
}

impl RetrospectiveDecisionCard {
    /// Immutable access to the original card. Only its expected-NPV Claim has the checked temporal ancestry.
    #[must_use]
    pub fn card(&self) -> &DecisionCard {
        &self.card
    }

    /// The checked cut and declared expected-NPV ancestry.
    #[must_use]
    pub fn cone(&self) -> &CheckedCone {
        &self.cone
    }

    /// The output and read receipts of the full and filtered runs.
    #[must_use]
    pub fn replay(&self) -> &ReplayCertificate {
        &self.replay
    }

    /// Renders the checked scope beside the ordinary card; no certificate for the whole decision is asserted.
    #[must_use]
    pub fn render_one_pager_markdown(&self) -> String {
        let mut out = format!(
            "## Historical check: expected-NPV input provenance\n\n\
             - **Claim root**: `{}`\n\
             - **Decision cut**: `{}` UTC nanoseconds since Unix epoch\n\
             - **Checked ancestry**: {} objects; cone digest `{}`\n\
             - **Full / filtered replay**: identical output and read receipts; output digest `{}`\n\n\
             Scope: the declared ancestry of the expected-NPV Claim passed the availability and record-anchor \
             checks. Replay covers the displayed card and its exact numeric values. Action choice, CVaR, flip \
             margins and VOI retain their existing inputs without separate Claim provenance; this check does \
             not establish complete historical sourcing for those fields or certify the whole decision.\n\n",
            self.cone.root(),
            self.cone.cut().decision_ns,
            self.cone.nodes().len(),
            self.cone.digest(),
            self.replay.output_digest(),
        );
        out.push_str("**Record anchor used by this cone**:\n\n");
        for (cell, sequence) in self.cone.anchor_marks() {
            out.push_str(&format!("- `{cell}`: commit `{sequence}`\n"));
        }
        out.push('\n');
        out.push_str(&self.card.render_one_pager_markdown());
        out
    }
}

impl DecisionCard {
    /// Constructs a validated `DecisionCard`, enforcing `INV-015` (`FMINE-REF-204` if `UU` slot is missing)
    /// and `NL-4` (`FMINE-REF-205` if `expected_npv_usd_m` has no colour to show).
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
        let color = expected_npv_usd_m.color()?;
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

    /// Seals this card only after its actual expected-NPV root and parents match the temporal graph and the
    /// supplied replay certificate matches this exact output, root, cut and cone. Obtain `replay` by returning
    /// [`Self::temporal_replay_bytes`] from the generator passed to `fmine_claim::dag::replay::verify_replay`.
    ///
    /// Ordinary [`Self::build`] has no historical claim. This checked wrapper covers the declared expected-NPV
    /// provenance; other legacy fields have byte replay but no independent temporal provenance.
    pub fn build_as_of(
        self,
        graph: &TemporalGraph,
        cut: &ConeCut,
        replay: ReplayCertificate,
    ) -> Result<RetrospectiveDecisionCard, Refusal> {
        self.expected_npv_usd_m.check_id()?;
        self.expected_npv_usd_m.verify_uu_slot_for_decision()?;
        if self.color != self.expected_npv_usd_m.color()? {
            return Err(Refusal::new(
                RefusalCode::CalibrationRequired205,
                "retrospective card colour differs from its expected-NPV Claim",
            ));
        }
        graph
            .verify_claim(&self.expected_npv_usd_m)
            .map_err(|error| error.refusal())?;
        let cone = graph
            .check(self.expected_npv_usd_m.claim_id, cut)
            .map_err(|error| error.refusal())?;
        if replay.root() != cone.root()
            || replay.cut() != cone.cut()
            || replay.cone_digest() != cone.digest()
            || !replay.matches_output(&self.temporal_replay_bytes())
        {
            return Err(Refusal::new(
                RefusalCode::TemporalConeViolation110,
                "replay certificate does not bind this card output, expected-NPV root, cut and checked cone",
            ));
        }
        Ok(RetrospectiveDecisionCard {
            card: self,
            cone,
            replay,
        })
    }

    /// Internal replay projection, version 2: the rendered card, expected-NPV canonical Claim body and exact
    /// bits of displayed numeric values, VOI totals and conditional policies. Exact bits prevent edits hidden
    /// by Markdown's decimal formatting from reusing a certificate. This is an in-process replay input, not a
    /// new durable card format.
    #[must_use]
    pub fn temporal_replay_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"DCRP", 2);
        enc.put_str(&self.render_one_pager_markdown());
        enc.put_bytes(&self.expected_npv_usd_m.canonical_body());
        enc.put_u64(self.expected_npv_usd_m.value.to_bits());
        enc.put_u64(self.cvar95_loss_usd_m.to_bits());
        enc.put_u64(self.flip_margins.len() as u64);
        for margin in &self.flip_margins {
            enc.put_u64(margin.baseline_value.to_bits());
            enc.put_u64(margin.flip_value.to_bits());
        }
        enc.put_u64(self.expected_npv_usd_m.uu_terms.len() as u64);
        for term in &self.expected_npv_usd_m.uu_terms {
            enc.put_u64(term.downside_exposure_usd_m.to_bits());
        }
        enc.put_u64(self.top_voi_items.len() as u64);
        for item in &self.top_voi_items {
            enc.put_str(&item.description);
            enc.put_u64(item.cost_usd_m.to_bits());
            encode_optional_values(
                &mut enc,
                item.benefit_cost_ratio.as_ref().map(std::slice::from_ref),
            );
            encode_voi_summary(&mut enc, item.summary);
            encode_voi_summary(&mut enc, item.analysis.summary);
            encode_values(&mut enc, &item.analysis.prior_action_values);
            encode_indices(&mut enc, &item.analysis.prior_best_actions);
            enc.put_u64(item.analysis.signals.len() as u64);
            for signal in &item.analysis.signals {
                enc.put_u64(signal.signal_index as u64);
                enc.put_u64(signal.probability.to_bits());
                encode_optional_values(&mut enc, signal.posterior_state_probs.as_deref());
                encode_indices(&mut enc, &signal.best_actions);
                encode_optional_values(&mut enc, signal.action_expected_payoffs.as_deref());
                enc.put_u64(signal.evsi_contribution.to_bits());
            }
        }
        enc.finish()
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
            "- **Status Color**: `{}` | **Evidence**: `{:?}` | **Epistemic**: `{:?}` | **State**: `{:?}`\n",
            self.color.name(),
            self.expected_npv_usd_m.evidence,
            self.expected_npv_usd_m.epistemic,
            self.expected_npv_usd_m.state
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

        out.push_str("\n## Ranked Value-of-Information (`VOI`) Campaigns\n\n");
        if !self.top_voi_items.is_empty() {
            out.push_str(
                "Finite-model values conditional on the supplied priors, payoffs and signal likelihoods; \
                 these probabilities do not establish calibration. State and action indices are zero-based \
                 positions in the supplied problem, without a mapping to this card's named actions. \
                 Payoffs, costs and information values use USD millions.\n\n",
            );
        }
        for v in &self.top_voi_items {
            let ratio = v.benefit_cost_ratio.map_or_else(
                || "undefined (zero campaign cost)".to_string(),
                |ratio| format!("{ratio:.2}x"),
            );
            out.push_str(&format!(
                "### {}\n\n{}\n\n\
                 EVSI `${:.2}M`, Cost `${:.2}M`, Net VOI `${:.2}M`; benefit/cost: {}.\n\n\
                 Before observing a signal, best action indices: `{:?}`; action expected values: `{}`.\n\n\
                 | Signal | Model probability | Posterior state probabilities | Best action indices | Action expected values ($M) | EVSI contribution ($M) |\n\
                 | --- | ---: | --- | --- | --- | ---: |\n",
                v.campaign_id,
                v.description,
                v.summary.evsi,
                v.cost_usd_m,
                v.summary.net_voi,
                ratio,
                v.analysis.prior_best_actions,
                display_values(&v.analysis.prior_action_values),
            ));
            for signal in &v.analysis.signals {
                let posterior = signal.posterior_state_probs.as_deref().map_or_else(
                    || "impossible signal; no posterior".to_string(),
                    display_values,
                );
                let values = signal
                    .action_expected_payoffs
                    .as_deref()
                    .map_or_else(|| "not applicable".to_string(), display_values);
                out.push_str(&format!(
                    "| {} | {:.6} | {} | {:?} | {} | {:.6} |\n",
                    signal.signal_index,
                    signal.probability,
                    posterior,
                    signal.best_actions,
                    values,
                    signal.evsi_contribution,
                ));
            }
            out.push('\n');
            if v.analysis.prior_best_actions.iter().any(|action| {
                v.analysis
                    .signals
                    .iter()
                    .filter(|signal| signal.probability > 0.0)
                    .all(|signal| signal.best_actions.contains(action))
            }) {
                out.push_str(
                    "A prior-optimal action remains optimal for every possible signal; no test result \
                     requires changing that choice.\n\n",
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::dag::replay::{ReplayStore, verify_replay};
    use fmine_claim::dag::{Availability, NodeKind, TemporalNode};
    use fmine_claim::{AdditivityClass, Confidentiality, Representation, SpatialSupport, UuTerm};
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::color::{ColorRank, ValidityDomain};
    use fmine_evidence::{AssertionSource, EpistemicStatus, EvidenceClass, KnowledgeState, KuKind};
    use fmine_time::{Anchor, RecordTime, TimeEvidence};

    fn make_npv_claim(with_uu: bool, evidence: EvidenceClass) -> Claim<f64> {
        make_npv_claim_with_parents(with_uu, evidence, vec![])
    }

    fn make_npv_claim_with_parents(
        with_uu: bool,
        evidence: EvidenceClass,
        parents: Vec<ObjectId>,
    ) -> Claim<f64> {
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
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::internal(),
            uu,
            parents,
            vec![SourceAnchor::new("study.pdf", [0x22; 32], "Table 14-1")],
        )
    }

    fn temporal_card(parents: Vec<ObjectId>) -> DecisionCard {
        DecisionCard::build(
            "Synthetic retrospective input control",
            "Collect information",
            "Do nothing",
            make_npv_claim_with_parents(true, EvidenceClass::ModelledCalibrated, parents),
            40.0,
            vec![FlipMargin::from_linear_sensitivity(
                "head grade",
                2.0,
                10.0,
                100.0,
                "g/t",
                "Do nothing",
            )],
            vec![],
        )
        .expect("synthetic card")
    }

    fn temporal_fixture(card: &DecisionCard) -> (TemporalGraph, ConeCut, ReplayStore) {
        let claim = &card.expected_npv_usd_m;
        let mut nodes = vec![TemporalNode {
            id: claim.claim_id,
            kind: NodeKind::Claim,
            parents: claim.parents.clone(),
            availability: Availability::Derived {
                performed_at: Some(50),
                declared_available_at: None,
            },
            recorded_at: Some(RecordTime::new(
                "synthetic",
                2,
                50,
                TimeEvidence::HostSynced,
            )),
        }];
        for parent in &claim.parents {
            nodes.push(TemporalNode {
                id: *parent,
                kind: NodeKind::Observation,
                parents: vec![],
                availability: Availability::Source {
                    available_at: Some(20),
                },
                recorded_at: Some(RecordTime::new(
                    "synthetic",
                    1,
                    20,
                    TimeEvidence::HostSynced,
                )),
            });
        }
        let graph = TemporalGraph::new(nodes).expect("synthetic graph");
        let cut = ConeCut {
            decision_ns: 100,
            anchor: Anchor::new([("synthetic", 2)]).expect("synthetic anchor"),
        };
        let mut store = ReplayStore::new();
        store
            .insert(claim.claim_id, claim.canonical_body())
            .expect("freeze claim body");
        (graph, cut, store)
    }

    fn temporal_replay(
        card: &DecisionCard,
        graph: &TemporalGraph,
        cut: &ConeCut,
        store: &ReplayStore,
    ) -> ReplayCertificate {
        let claim = &card.expected_npv_usd_m;
        // A synthetic binding control: the stored input is checked against the test card before its output
        // is replayed. This is not a real hindcast or evidence of model skill.
        verify_replay(store, graph, claim.claim_id, cut, |view| {
            assert_eq!(view.read(claim.claim_id)?, claim.canonical_body());
            Ok(card.temporal_replay_bytes())
        })
        .expect("clean replay")
    }

    #[test]
    fn retrospective_card_seals_actual_root_and_replayed_output() {
        let parent = ObjectId::from_canonical_bytes(PrefixKind::Evidence, b"synthetic observation");
        let card = temporal_card(vec![parent]);
        let (graph, cut, store) = temporal_fixture(&card);
        let replay = temporal_replay(&card, &graph, &cut, &store);
        let checked = card
            .clone()
            .build_as_of(&graph, &cut, replay)
            .expect("checked snapshot");
        assert_eq!(checked.card(), &card);
        assert_eq!(checked.cone().root(), card.expected_npv_usd_m.claim_id);
        assert_eq!(checked.cone().nodes().len(), 2);
        assert_eq!(checked.cone().available_at(&parent), Some(20));
        assert!(
            checked
                .replay()
                .matches_output(&card.temporal_replay_bytes())
        );
        let shown = checked.render_one_pager_markdown();
        assert!(shown.contains("Historical check: expected-NPV input provenance"));
        assert!(shown.contains("not establish complete historical sourcing"));
        assert!(shown.contains("`synthetic`: commit `2`"));
        assert!(shown.contains(&checked.cone().digest().to_string()));
        assert!(shown.ends_with(&card.render_one_pager_markdown()));
    }

    #[test]
    fn retrospective_card_refuses_unrelated_root_and_omitted_parents() {
        let parent = ObjectId::from_canonical_bytes(PrefixKind::Evidence, b"synthetic observation");
        let card = temporal_card(vec![parent]);
        let (graph, cut, store) = temporal_fixture(&card);
        let replay = temporal_replay(&card, &graph, &cut, &store);
        let other = temporal_card(vec![]);
        let (other_graph, _, _) = temporal_fixture(&other);
        assert_eq!(
            other
                .build_as_of(&other_graph, &cut, replay)
                .expect_err("different root")
                .code,
            "FMINE-REF-110"
        );
        // A graph with the correct root id but missing its declared parent is not sufficient.
        let mut nodes: Vec<_> = graph.nodes().cloned().collect();
        nodes
            .iter_mut()
            .find(|n| n.id == card.expected_npv_usd_m.claim_id)
            .expect("root")
            .parents
            .clear();
        let incomplete = TemporalGraph::new(nodes).expect("structurally valid graph");
        let replay = temporal_replay(&card, &incomplete, &cut, &store);
        let error = card
            .build_as_of(&incomplete, &cut, replay)
            .expect_err("omitted parent");
        assert_eq!(error.code, "FMINE-REF-110");
        assert!(error.what.contains("parent"));
    }

    #[test]
    fn retrospective_card_refuses_late_ancestor_and_changed_cut_or_cone() {
        let parent = ObjectId::from_canonical_bytes(PrefixKind::Evidence, b"synthetic observation");
        let card = temporal_card(vec![parent]);
        let (graph, cut, store) = temporal_fixture(&card);
        let replay = temporal_replay(&card, &graph, &cut, &store);
        for availability in [21, 101] {
            let mut nodes: Vec<_> = graph.nodes().cloned().collect();
            nodes
                .iter_mut()
                .find(|n| n.id == parent)
                .expect("parent")
                .availability = Availability::Source {
                available_at: Some(availability),
            };
            let changed = TemporalGraph::new(nodes).expect("changed metadata graph");
            let error = card
                .clone()
                .build_as_of(&changed, &cut, replay.clone())
                .expect_err("changed cone cannot reuse the certificate");
            assert_eq!(error.code, "FMINE-REF-110");
            if availability > cut.decision_ns {
                assert!(error.what.contains(&parent.to_string()));
                assert!(error.what.contains("available 101"));
            }
        }
        let earlier_cut = ConeCut {
            decision_ns: 99,
            anchor: cut.anchor.clone(),
        };
        assert_eq!(
            card.clone()
                .build_as_of(&graph, &earlier_cut, replay.clone())
                .expect_err("different cut")
                .code,
            "FMINE-REF-110"
        );
        let earlier_anchor = ConeCut {
            decision_ns: 100,
            anchor: Anchor::new([("synthetic", 1)]).expect("earlier anchor"),
        };
        assert_eq!(
            card.build_as_of(&graph, &earlier_anchor, replay)
                .expect_err("root not recorded at this anchor")
                .code,
            "FMINE-REF-110"
        );
    }

    #[test]
    fn retrospective_card_refuses_output_or_claim_edits_after_replay() {
        let card = temporal_card(vec![]);
        let (graph, cut, store) = temporal_fixture(&card);
        let replay = temporal_replay(&card, &graph, &cut, &store);
        let mut edited = card.clone();
        edited.expected_npv_usd_m.value += 0.000_01;
        assert_eq!(
            edited.render_one_pager_markdown(),
            card.render_one_pager_markdown()
        );
        assert_eq!(
            edited
                .build_as_of(&graph, &cut, replay.clone())
                .expect_err("changed raw value")
                .code,
            "FMINE-REF-110"
        );
        let mut edited = card;
        edited.expected_npv_usd_m.metric_key.push_str("_edited");
        assert_eq!(
            edited
                .build_as_of(&graph, &cut, replay)
                .expect_err("changed Claim body")
                .code,
            "FMINE-REF-507"
        );
    }

    #[test]
    fn temporal_replay_projection_binds_subdisplay_numeric_edits() {
        let original = temporal_card(vec![]);
        let shown = original.render_one_pager_markdown();
        let bytes = original.temporal_replay_bytes();
        for field in 0..5 {
            let mut edited = original.clone();
            match field {
                0 => edited.expected_npv_usd_m.value += 0.000_01,
                1 => edited.cvar95_loss_usd_m += 0.000_01,
                2 => edited.flip_margins[0].baseline_value += 0.000_01,
                3 => edited.flip_margins[0].flip_value += 0.000_01,
                _ => edited.expected_npv_usd_m.uu_terms[0].downside_exposure_usd_m += 0.000_01,
            }
            assert_eq!(shown, edited.render_one_pager_markdown(), "field {field}");
            assert_ne!(bytes, edited.temporal_replay_bytes(), "field {field}");
        }
    }

    #[test]
    fn test_decision_card_uu_and_flip_margin_guards() {
        // test_missing_uu_refuses_204: a missing UuTerm on a KnownUnknown claim refuses with FMINE-REF-204
        // (unbudgeted unknowns), never 404 (stale TSF instrumentation, a tailings-safety code).
        let bad_claim = make_npv_claim(false, EvidenceClass::ModelledCalibrated);
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

        // An unreviewed assertion refuses with FMINE-REF-205 (calibration required for decision-grade use), never
        // 401 (unknown reactive ground, a blasting-safety code).
        let red_claim = make_npv_claim(
            true,
            EvidenceClass::Assertion {
                source: AssertionSource::Agent,
            },
        );
        let err_red = DecisionCard::build(
            "Pushback 4",
            "Proceed",
            "Defer",
            red_claim,
            40.0,
            vec![],
            vec![],
        )
        .expect_err("must refuse an unreviewed assertion");
        assert_eq!((err_red.code, err_red.exit_code), ("FMINE-REF-205", 3));

        // Valid calibrated claim with UuTerm succeeds and computes flip margin.
        let ok_claim = make_npv_claim(true, EvidenceClass::ModelledCalibrated);
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
        // No calibration domain was named, so the calibrated class has nothing to stand on yet.
        assert_eq!(card.color.rank(), ColorRank::Estimated);
        let md = card.render_one_pager_markdown();
        assert!(md.contains("UU-GEO-07"));
        assert!(md.contains("Au_Recovery"));
        assert!(md.contains("**Status Color**: `estimated`"), "{md}");
        assert!(
            md.contains("**Epistemic**: `KnownUnknown(Parametric)`"),
            "{md}"
        );
        assert!(md.contains("**State**: `Estimated`"), "{md}");

        // Naming the domain and its calibration record carries it onto the card.
        let anchored = make_npv_claim(true, EvidenceClass::ModelledCalibrated).with_regime(
            ValidityDomain::unconstrained().with("feed_grade", 1.0, 3.0),
            "calib-2026-q3",
        );
        let card = DecisionCard::build(
            "Pushback 4",
            "Proceed Mill",
            "Leach Pad Only",
            anchored,
            40.0,
            vec![],
            vec![],
        )
        .expect("valid card");
        assert_eq!(card.color.rank(), ColorRank::Validated);
    }

    #[test]
    fn voi_campaigns_rank_by_net_value() {
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
        let ranked = rank_voi_campaigns(&problem, &campaigns).expect("valid finite campaigns");
        assert_eq!(ranked[0].campaign_id, "CAMP-HIGH");
        assert!(ranked[0].summary.net_voi > ranked[1].summary.net_voi);
    }

    fn testwork_problem() -> VoiProblem {
        VoiProblem {
            state_probs: vec![0.6, 0.4],
            payoffs: vec![vec![300.0, 0.0], vec![-200.0, 0.0]],
        }
    }

    fn testwork_campaign(id: &str, cost: f64) -> VoiCampaign {
        VoiCampaign {
            campaign_id: id.into(),
            description: "Synthetic GB-29 metallurgical testwork and delay".into(),
            cost_usd_m: cost,
            likelihood: vec![vec![0.85, 0.15], vec![0.15, 0.85]],
        }
    }

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-10 * expected.abs().max(1.0),
            "{actual} differs from {expected}"
        );
    }

    #[test]
    fn gb29_ranking_exposes_the_observation_that_changes_the_action() {
        let mut useless = testwork_campaign("uninformative", 1.0);
        useless.likelihood = vec![vec![0.5, 0.5], vec![0.5, 0.5]];
        let ranked = rank_voi_campaigns(
            &testwork_problem(),
            &[useless, testwork_campaign("testwork", 9.8)],
        )
        .expect("declared finite model");
        let testwork = &ranked[0];
        assert_eq!(testwork.campaign_id, "testwork");
        assert_eq!(testwork.summary, testwork.analysis.summary);
        close(testwork.summary.prior_expected_value, 100.0);
        close(testwork.summary.evpi, 80.0);
        close(testwork.summary.evsi, 41.0);
        close(testwork.summary.net_voi, 31.2);
        close(
            testwork.benefit_cost_ratio.expect("positive cost"),
            41.0 / 9.8,
        );
        assert_eq!(testwork.analysis.prior_best_actions, [0]);
        close(testwork.analysis.prior_action_values[0], 100.0);
        let high = &testwork.analysis.signals[0];
        let low = &testwork.analysis.signals[1];
        close(high.probability, 0.57);
        close(low.probability, 0.43);
        close(
            high.posterior_state_probs.as_ref().expect("possible")[0],
            17.0 / 19.0,
        );
        close(
            low.posterior_state_probs.as_ref().expect("possible")[0],
            9.0 / 43.0,
        );
        close(
            high.action_expected_payoffs.as_ref().expect("possible")[0],
            4700.0 / 19.0,
        );
        close(
            low.action_expected_payoffs.as_ref().expect("possible")[0],
            -4100.0 / 43.0,
        );
        assert_eq!(high.best_actions, [0]);
        assert_eq!(low.best_actions, [1]);
        close(high.evsi_contribution + low.evsi_contribution, 41.0);
        close(ranked[1].summary.evsi, 0.0);
        close(ranked[1].summary.net_voi, -1.0);

        let mut card = temporal_card(vec![]);
        let previous_color = card.color.clone();
        card.top_voi_items = ranked;
        let shown = card.render_one_pager_markdown();
        assert!(shown.contains("EVSI `$41.00M`, Cost `$9.80M`, Net VOI `$31.20M`"));
        assert!(shown.contains("| 0 | 0.570000 | [0.894737, 0.105263] | [0] |"));
        assert!(shown.contains("| 1 | 0.430000 | [0.209302, 0.790698] | [1] |"));
        assert!(shown.contains("zero-based positions in the supplied problem"));
        assert!(shown.contains("do not establish calibration"));
        assert!(shown.contains("no test result requires changing that choice"));
        assert_eq!(card.color, previous_color);
    }

    #[test]
    fn campaign_and_action_ties_and_impossible_signals_are_retained() {
        let problem = VoiProblem {
            state_probs: vec![0.5, 0.5],
            payoffs: vec![vec![10.0, 10.0, 0.0], vec![-10.0, -10.0, 0.0]],
        };
        let mut campaign = testwork_campaign("beta", 0.0);
        campaign.likelihood = vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]];
        let mut alpha = campaign.clone();
        alpha.campaign_id = "alpha".into();
        let ranked = rank_voi_campaigns(&problem, &[campaign, alpha]).expect("valid ties");
        assert_eq!(ranked[0].campaign_id, "alpha");
        assert_eq!(ranked[1].campaign_id, "beta");
        let analysis = &ranked[0].analysis;
        assert_eq!(analysis.prior_best_actions, [0, 1, 2]);
        assert_eq!(analysis.signals[0].best_actions, [0, 1]);
        assert_eq!(analysis.signals[1].best_actions, [2]);
        let impossible = &analysis.signals[2];
        assert_eq!(impossible.signal_index, 2);
        close(impossible.probability, 0.0);
        assert_eq!(impossible.posterior_state_probs, None);
        assert_eq!(impossible.action_expected_payoffs, None);
        assert!(impossible.best_actions.is_empty());
        close(impossible.evsi_contribution, 0.0);
        close(analysis.summary.evsi, 5.0);
        assert_eq!(ranked[0].benefit_cost_ratio, None);

        let mut card = temporal_card(vec![]);
        card.top_voi_items = ranked;
        let shown = card.render_one_pager_markdown();
        assert!(shown.contains("benefit/cost: undefined (zero campaign cost)"));
        assert!(shown.contains(
            "| 2 | 0.000000 | impossible signal; no posterior | [] | not applicable | 0.000000 |"
        ));

        let mut free_useless = testwork_campaign("free-useless", 0.0);
        free_useless.likelihood = vec![vec![1.0], vec![1.0]];
        let ranked =
            rank_voi_campaigns(&problem, &[free_useless]).expect("free uninformative test");
        close(ranked[0].summary.evsi, 0.0);
        assert_eq!(ranked[0].benefit_cost_ratio, None);
    }

    #[test]
    fn invalid_campaigns_refuse_the_whole_ranking() {
        let problem = testwork_problem();
        let valid = testwork_campaign("valid", 9.8);
        let mut invalid = Vec::new();
        for id in ["", "  ", "valid"] {
            invalid.push(testwork_campaign(id, 1.0));
        }
        for cost in [-1.0, f64::NAN, f64::INFINITY] {
            invalid.push(testwork_campaign("bad-cost", cost));
        }
        for likelihood in [
            vec![],
            vec![vec![1.0]],
            vec![vec![0.5, 0.5], vec![1.0]],
            vec![vec![0.8, 0.8], vec![0.5, 0.5]],
            vec![vec![-0.5, 1.5], vec![0.5, 0.5]],
            vec![vec![f64::NAN, 0.5], vec![0.5, 0.5]],
        ] {
            let mut campaign = testwork_campaign("bad-likelihood", 1.0);
            campaign.likelihood = likelihood;
            invalid.push(campaign);
        }
        for campaign in invalid {
            let error = rank_voi_campaigns(&problem, &[valid.clone(), campaign])
                .expect_err("no partial ranking after a malformed campaign");
            assert_eq!(error.code, "FMINE-REF-109");
        }
        let invalid_problem = VoiProblem {
            state_probs: vec![0.6, 0.6],
            payoffs: problem.payoffs,
        };
        assert_eq!(
            rank_voi_campaigns(&invalid_problem, &[])
                .expect_err("validate even when no campaigns are requested")
                .code,
            "FMINE-REF-109"
        );
    }

    #[test]
    fn small_positive_cost_keeps_its_ratio_and_unrepresentable_ratios_refuse() {
        let ranked = rank_voi_campaigns(
            &testwork_problem(),
            &[testwork_campaign("small-cost", 1e-10)],
        )
        .expect("positive cost is not treated as free");
        close(ranked[0].benefit_cost_ratio.expect("defined"), 41.0 / 1e-10);
        assert_eq!(
            rank_voi_campaigns(
                &testwork_problem(),
                &[testwork_campaign("overflow", f64::MIN_POSITIVE)],
            )
            .expect_err("an infinite ratio is not a usable ranking output")
            .code,
            "FMINE-REF-207"
        );
        let tiny_payoffs = VoiProblem {
            state_probs: vec![0.5, 0.5],
            payoffs: vec![vec![f64::MIN_POSITIVE, 0.0], vec![0.0, f64::MIN_POSITIVE]],
        };
        let costly_perfect_test = VoiCampaign {
            campaign_id: "underflow".into(),
            description: "Synthetic representable gain with an unrepresentable ratio".into(),
            cost_usd_m: f64::MAX,
            likelihood: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        };
        assert_eq!(
            rank_voi_campaigns(&tiny_payoffs, &[costly_perfect_test])
                .expect_err("a positive ratio must not silently underflow to zero")
                .code,
            "FMINE-REF-207"
        );
    }

    #[test]
    fn replay_projection_binds_subdisplay_policy_and_summary_edits() {
        let mut card = temporal_card(vec![]);
        card.top_voi_items =
            rank_voi_campaigns(&testwork_problem(), &[testwork_campaign("testwork", 9.8)])
                .expect("synthetic policy");
        let original_bytes = card.temporal_replay_bytes();
        let shown = card.render_one_pager_markdown();
        for field in 0..6 {
            let mut changed = card.clone();
            let item = &mut changed.top_voi_items[0];
            match field {
                0 => item.analysis.prior_action_values[0] += 1e-10,
                1 => item.analysis.signals[0].probability += 1e-10,
                2 => {
                    item.analysis.signals[0]
                        .posterior_state_probs
                        .as_mut()
                        .expect("possible")[0] += 1e-10
                }
                3 => {
                    item.analysis.signals[0]
                        .action_expected_payoffs
                        .as_mut()
                        .expect("possible")[0] += 1e-10
                }
                4 => item.analysis.signals[0].evsi_contribution += 1e-10,
                _ => item.analysis.summary.evpi += 1e-10,
            }
            assert_eq!(shown, changed.render_one_pager_markdown(), "field {field}");
            assert_ne!(
                original_bytes,
                changed.temporal_replay_bytes(),
                "field {field}"
            );
        }
    }
}
