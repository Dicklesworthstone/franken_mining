//! Source-bound bootstrap configuration and complete conditional robustness results.
//! Determinism: Exact

use super::super::render::{ids, js, md, ratio, rq};
use super::*;
use std::fmt::Write as _;

impl SelectionStabilityDesign {
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"model_key\":{},\"method\":{},\"seed\":{},\"replicates\":{},\"draws_per_replicate\":{},\"sampling_basis\":{}}}",
            js(&self.model_key),
            js(&self.method),
            js(&self.seed.to_string()),
            self.replicates,
            self.draws_per_replicate,
            js(&self.sampling_basis)
        )
    }
}

impl SelectionStabilitySummary {
    #[must_use]
    pub fn to_json(&self) -> String {
        let alternatives = self.alternatives.iter().map(|row| format!(
            "{{\"alternative_id\":{},\"structurally_feasible\":{},\"nominal_winner\":{},\"risk_feasible_replicates\":{},\"co_winner_replicates\":{},\"selection_frequency\":{},\"fragile\":{}}}",
            js(&row.alternative_id), row.structurally_feasible, row.nominal_winner,
            row.risk_feasible_replicates, row.co_winner_replicates, rq(row.selection_frequency), row.fragile
        )).collect::<Vec<_>>().join(",");
        let candidates = self.candidate_frequencies.iter().map(|row| format!(
            "{{\"candidate_id\":{},\"scope\":{},\"state_id\":{},\"certain_replicates\":{},\"possible_replicates\":{},\"frequency_lower\":{},\"frequency_upper\":{}}}",
            js(&row.candidate_id), js(&row.scope), row.state_id.as_deref().map_or_else(|| "null".into(), js),
            row.certain_replicates, row.possible_replicates, rq(row.frequency_lower), rq(row.frequency_upper)
        )).collect::<Vec<_>>().join(",");
        let replicates = self.replicates.iter().map(|row| format!(
            "{{\"index\":{},\"state_counts\":[{}],\"feasible_winners\":{},\"risk_feasible_alternatives\":{}}}",
            row.index, row.state_counts.iter().map(u32::to_string).collect::<Vec<_>>().join(","),
            ids(&row.feasible_winners), row.risk_feasible_alternatives
        )).collect::<Vec<_>>().join(",");
        format!(
            "{{\"design\":{},\"state_ids\":{},\"objective_unit\":{},\"nominal_winners\":{},\"nominal_runner_up\":{},\"no_feasible_replicates\":{},\"definite_flip_frequency\":{},\"possible_flip_frequency\":{},\"alternatives\":[{alternatives}],\"candidate_frequencies\":[{candidates}],\"replicates\":[{replicates}],\"fragile_threshold\":\"4/5\",\"structural_feasibility\":\"fixed_over_complete_original_support\",\"tie_rule\":\"all_exact_co_winners_retained\",\"flip_event\":\"choice_outside_nominal_co_winner_set\",\"frequency_interpretation\":\"conditional_on_declared_law_not_calibrated_probability_of_optimality\"}}",
            self.design.to_json(),
            ids(&self.state_ids),
            js(&self.objective_unit),
            ids(&self.nominal_winners),
            ids(&self.nominal_runner_up),
            self.no_feasible_replicates,
            self.definite_flip_frequency
                .map_or_else(|| "null".into(), rq),
            self.possible_flip_frequency
                .map_or_else(|| "null".into(), rq)
        )
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let listed = |values: &[String]| {
            if values.is_empty() {
                "none".into()
            } else {
                md(&values.join(", "))
            }
        };
        let frequency = |value: Option<Rat>| {
            value.map_or_else(|| "unavailable: no nominal winner".into(), ratio)
        };
        let mut out = format!(
            "\n## Selection stability\n\n**Declared sampling basis:** {}. Method: `{}`; seed: `{}`; {} replicates of {} draws from the original weighted joint-state law. Objective: {}.\n\n**Nominal co-winners:** {}. **Nominal runner-up co-winners:** {}. **Choice outside the nominal co-winner set, definite / possible frequency:** {} / {}. **Replicates with no feasible choice:** {} of {}.\n\nThese are conditional resampling frequencies, not calibrated probabilities of optimality or confidence intervals. Original cash, contractual and share-count constraints remain binding in zero-count states. Every replicate recomputes all risk limits. Exact ties retain every co-winner; their inclusion frequencies may sum above one. A definite change has no nominal co-winner left; a possible change has at least one new co-winner outside the nominal set. Changes within the nominal tied set are represented by candidate inclusion bounds. Empty feasible sets remain in every denominator and are reported separately from ranking changes. A nominal co-winner with inclusion frequency below 4/5 is flagged **fragile**.\n\n| Alternative | Original structural feasibility | Risk-feasible replicates | Co-winner replicates | Selection frequency | Nominal status |\n|---|---|---:|---:|---:|---|\n",
            md(&self.design.sampling_basis),
            md(&self.design.method),
            self.design.seed,
            self.design.replicates,
            self.design.draws_per_replicate,
            md(&self.objective_unit),
            listed(&self.nominal_winners),
            listed(&self.nominal_runner_up),
            frequency(self.definite_flip_frequency),
            frequency(self.possible_flip_frequency),
            self.no_feasible_replicates,
            self.design.replicates
        );
        for row in &self.alternatives {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                md(&row.alternative_id),
                if row.structurally_feasible {
                    "feasible"
                } else {
                    "rejected"
                },
                row.risk_feasible_replicates,
                row.co_winner_replicates,
                ratio(row.selection_frequency),
                if row.fragile {
                    "fragile co-winner"
                } else if row.nominal_winner {
                    "co-winner"
                } else {
                    "alternative"
                }
            );
        }
        out.push_str("\n### Candidate inclusion bounds\n\nThe lower count requires inclusion in every co-winner; the upper count requires inclusion in any co-winner. Realized-state rows describe the complete policy's decision on that path, including unsampled paths; they are not state-occurrence probabilities.\n\n| Candidate | Decision scope | State | Certain replicates | Possible replicates | Frequency lower | Frequency upper |\n|---|---|---|---:|---:|---:|---:|\n");
        for row in &self.candidate_frequencies {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} |",
                md(&row.candidate_id),
                md(&row.scope),
                row.state_id.as_deref().map_or_else(|| "—".into(), md),
                row.certain_replicates,
                row.possible_replicates,
                ratio(row.frequency_lower),
                ratio(row.frequency_upper)
            );
        }
        let _ = write!(
            out,
            "\n### Every resample\n\nState-count order: {}.\n\n| Replicate | Counts | Risk-feasible alternatives | Co-winners |\n|---:|---|---:|---|\n",
            md(&self.state_ids.join(", "))
        );
        for row in &self.replicates {
            let counts = row
                .state_counts
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                row.index,
                counts,
                row.risk_feasible_alternatives,
                listed(&row.feasible_winners)
            );
        }
        out
    }
}
