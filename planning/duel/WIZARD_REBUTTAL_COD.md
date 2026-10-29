# Codex rebuttal: two retained mechanisms, two rejected guarantees

**Date:** 2026-10-29. Read alongside my [reactions and revised self-scores](WIZARD_REACTIONS_COD.md).
Sources are the complete [CC](WIZARD_SCORES_CC_ON_COD.md) and
[AGY](WIZARD_SCORES_AGY_ON_COD.md) reviews, the original proposal files and the plan sections cited below.
No implementation or plan change is made here.

## Selection rule and confidence

**DESIGN.** To avoid choosing convenient opponents, I interpret “scored lowest” mechanically: use the mean
of CC's and AGY's scores for each of my fifteen ideas, then defend the two lowest. These are COD-16 at 575
and COD-26 at 577.5. The mean selects the cases; it does not turn differently calibrated judgments into a
statistical estimate. Both reviewers recognize these mechanisms' strengths, so the real disagreement is
their required scope and distinct contribution, not whether the reviewers deny the underlying problem.

For the attacks, I take the two lowest individual scores across all thirty ideas I previously evaluated:
AGY-08 at 180 and AGY-14 at 260. Both happen to be Gemini proposals; I am not substituting a higher-scored
Claude proposal merely to make the model count symmetric.

| Role | Idea | Scores at issue | Position defended here |
|---|---|---|---|
| Defend | COD-16: action-aware forecasts and causal benefit | CC 480; AGY 670; revised self 590 | Keep policy/estimand/action resolution inside the forecast ledger |
| Defend | COD-26: evidence-family withdrawal stress | CC 445; AGY 710; revised self 605 | Keep prospective family refits inside the robustness view |
| Attack | AGY-08: `CryptographicDataRoom` | My score 180 | Reject completeness/zero-leakage claims from the supplied construction |
| Attack | AGY-14: `UndergroundStopeSequenceStressAndFillCoupler` | My score 260 | Reject the advertised engineering guarantees and unvalidated recipe extrapolation |

**HYPOTHESIS — Confidence convention.** Percentages below are subjective, uncalibrated confidence in my
stated recommendation on the submitted design, not probabilities of equipment failure, geological truth,
cryptographic safety or successful adoption. No synthetic result is treated as real-site reconciliation
(§33.1), and all proposed workflows retain INV-002, INV-011 and FA's accounting authority.

## Defense 1 — COD-16: a registered forecast needs an action-policy target

### Claim

**INFERENCE.** Preregistering a forecast, logging an intervention and later fitting a generic causal estimator
does not by itself specify whether the observed outcome resolves that forecast. I defend an explicit link
between the forecast's estimand, its action policy, the action actually taken and the permitted resolution.
That is a necessary correctness condition for the vendor score, even when no causal benefit can be identified.

CC and AGY are right that a separate DiD engine or experiment-management product would duplicate planned
work. I accept their proposed merge into COD-08. My disagreement is with treating the retained increment as
only an extra status label: the label needs a target-policy contract or it can mis-score ordinary maintenance
and become a convenient way to hide difficult vendor forecasts.

### Evidence

**FACT.** Plan §16.5 registers vendor failure predictions and scores them from maintenance records and
outcomes. G-CRUC-5 requires a pre-period forecast for a vendor benefit claim. §19.6 supplies preregistered
counterfactual analysis, and §22.7/PO-HELM-3 requires resolution or declared non-resolution. Those are the right
existing homes; none of these cited passages alone makes an observed treated outcome equal to the
counterfactual outcome under no intervention.

**INFERENCE — Synthetic identification counterexample.** An alarm predicts failure under action A = 0.
A human instead takes preventive action A = 1 and observes no failure, Y(1) = 0. Two worlds remain compatible
with exactly that record:

| World | Observed outcome Y(1) | Unobserved no-action outcome Y(0) | Prevented failure? |
|---|---:|---:|---|
| W0 | 0 | 0 | No |
| W1 | 0 | 1 | Yes |

The observed record identifies neither the no-action outcome nor the causal saving. This is an
identification argument, not an empirical claim about any mine or vendor. Calling the alarm a false positive
in both worlds is unjustified; crediting a saved failure in both worlds is equally unjustified.

**DESIGN.** A prediction conditional on the site's normal maintenance policy can still resolve normally if
that policy was implemented. A prediction conditional on a materially different no-action policy requires
its predeclared resolution rule or an explicitly model-dependent counterfactual assessment. Keep the entry
in its frozen cohort, display intervention and missing-counterfactual status, and withhold an unconditional
accuracy or saving claim when its target was not observed. Do not delete it from the denominator or let a
vendor retrospectively redefine the target after seeing the action.

This matters to a CFO comparing vendor accuracy and realized benefit, and to the GM who must never be
penalized for taking a required preventive action. It needs no personal data or automated intervention.
The first synthetic acceptance cases distinguish non-adoption, normal-policy adoption, a changed action
policy and an actually observed policy-conditional target; real causal benefit still requires a credible
human-approved design.

AGY's ABAB/crossover suggestion can be appropriate in some approved non-safety settings, but it does not
solve the counterexample automatically. Carryover, residence-time mixing, feed drift and interference can
invalidate a naive comparison (§17.3, BD-313, GB-41). Choosing an admissible design remains necessary, and
required safety actions cannot become experimental controls.

### Verdict

**DESIGN — Retain the target-policy-action link as a forecast admission/resolution requirement; merge it into
COD-08 and existing vendor scoring, and defer real causal ROI until its design and data gates are met.**
My revised score remains **590/1000** because access, causal identification and standalone novelty remain weak.
**Confidence: 95%** in retaining the narrow correctness requirement, not in the ability to estimate a particular
site's avoided loss. If the existing implementation already enforces these exact semantics, this proposal
reduces to an acceptance case rather than another feature.

## Defense 2 — COD-26: family withdrawal asks a question a flip margin does not

### Claim

**INFERENCE.** A parameter threshold tells a reviewer how much an input must change to reverse a decision;
it does not identify what happens when a particular evidence family is unavailable and the original model is
refitted. I defend the latter as a qualified prospective audit stress inside the existing §22 robustness view.
It is neither an estimate that a source is defective nor a general proof that an audit will be valuable.

I concede both reviewers' duplication point: COD-03 supplies origin semantics and COD-04 supplies much of the
rerun machinery. Shared machinery should be implemented once. The separate consumer question is which
independent source check could materially challenge a particular recommendation before any defect is known.

### Evidence

**FACT.** §22.2 supplies flip margins; §22.5 supplies scenario regret, distributional robustness and selection
stability. §20.6 supplies identifiability checks. None of those cited mechanisms automatically runs
source-family deletion followed by a posterior refit. COD-03's shared-bias admission rule likewise does not
choose and execute prospective withdrawal cases.

**INFERENCE — Synthetic Gaussian counterexample.** Let θ be incremental gross value in $/dry tonne for an
alternative route, with prior mean 0 and variance 1 in the corresponding squared units. Two explicitly
independent source families supply estimates A = 4 and B = 0, each with variance 1 conditional on θ.
Using the route costs an additional $1/dry tonne; the alternative is to retain the baseline with zero
incremental value. Under this declared toy model, posterior precision is the prior precision plus the
precision of each retained source:

| Evidence retained | Posterior mean θ | Expected incremental net value of route | Preferred alternative |
|---|---:|---:|---|
| A and B | 4/3 | +1/3 $/dry tonne | Route |
| B only | 0 | −1 $/dry tonne | Baseline |
| A only | 2 | +1 $/dry tonne | Route |

Every row has a proper, finite posterior, so this is not a trivial structural non-identifiability refusal.
The withdrawal of A changes the action; withdrawal of B does not. A flip threshold at θ = 1 alone does not
tell the audit lead which family changes the fitted estimate that way. Nor does the table establish that A
is wrong: its result is conditional sensitivity under the registered model.

**DESIGN.** Freeze the family definitions, model, priors, fallback and action meanings before the stress.
Withdraw all dependent information that requires the family, preserving legitimate independent data.
Recompute the posterior and action comparison exactly for a bounded small case, with an independent
known-answer calculation. If the model cannot be refitted honestly, report an unresolved case rather than
inventing a derivative substitute. If removal only destroys identifiability, that lost admission is itself
the result, not an allegedly measured action reversal.

AGY's proposed minimum individual-assay deletion solver changes the question. Ten thousand assays sharing
one calibration are not ten thousand independent audit opportunities; minimizing row count can select a
different and misleading unit of vulnerability. A family stress preserves the declared common-error unit.
Searching individual subsets would additionally require a grouping/weighting policy and a separate
computational scope, with no evidence yet that a user needs it.

The remaining adoption test is concrete: a QP or controller must identify an independent check or review
choice changed by the report. No expected audit value is reported without an admissible distribution of
audit outcomes, and successful single-family tests do not establish robustness to family pairs or retained
systematic bias.

### Verdict

**DESIGN — Retain bounded family withdrawal as one explicit §22 robustness case, reusing COD-03/04; retire
the separate product pitch and any minimum-assay-removal expansion.**
My revised score is **605/1000**, reflecting a useful but conditional consumer of planned machinery.
**Confidence: 85%** that the narrow stress is worth retaining when an audit owner and an honestly refittable
model exist. That is not confidence that the stressed source is defective, or that an audit will find a
material error. If no distinct review choice results, ordinary robustness reporting is sufficient.

## Attack 1 — AGY-08: commitments cannot certify an unobserved population

### Claim

**INFERENCE.** The [data-room proposal, Idea 8](WIZARD_IDEAS_AGY_6_15.md)
promises completeness of raw evidence and zero leakage from a staged Merkle/MMR bundle. The supplied
construction supports a narrower statement: disclosed bytes can be checked against a previously accepted
commitment. It does not establish the stronger guarantees, making the proposed assurance dangerous at a
JV transfer-price or financing decision (§11.7, §31.2).

### Evidence

**INFERENCE — Pre-commit omission counterexample.** In world W0, the source population contains clean record
r1. In world W1, it contains r1 and a failed QA/QC batch r2, but the producer omits r2 before forming the
committed population. The producer can emit the same tree, declared count, signed total and valid inclusion
proof for r1 in both worlds. A recipient seeing the same bundle cannot determine whether r2 ever existed.
No hash collision or signature forgery is required.

A trusted, independently controlled complete source journal would change the assumptions, but the protocol
would still need to prove completeness of the authorized spatial query against that population. Merely
signing a root, carrying a count or showing membership of selected leaves is not that proof. This separates
completeness relative to a committed universe from completeness relative to all relevant real source records.

**INFERENCE.** At Stage 1, equality between disclosed panel sums and a committed header shows consistency
with those declared totals. If the fine-scale children are hidden, the claim that their summaries were
honestly computed needs an appropriate verifiable computation relation or trusted independent attestation;
hashing an internal node together with an asserted aggregate does not itself verify the relation. Stage-2
unsealing can expose more data for later checks, but cannot retroactively justify an unsupported Stage-1
assurance used to price the deal.

Salting commitments can support hiding under specified cryptographic assumptions. It does not, by itself,
prove zero leakage from manifest structure, counts, overlapping authorized queries, disclosed aggregates or
the staged release protocol. A test that fresh salts produce different-looking hashes is not a proof of the
whole protocol's computational indistinguishability. The proposal's “0 bits” claim has neither an explicit
permitted-leakage model nor a supplied proof establishing it.

**FACT.** The plan already supplies scoped partner projections, CAS/MMR commitments and the offline verifier
(§31.2, §33.6, BD-140). Those are useful and should be reused. The honest extension is staged authorized
unsealing with clearly stated commitment and source-population assumptions, not a 600 LOC claim to prove
complete honest computation over undisclosed evidence. Printing the same roots in a PDF changes presentation,
not proof strength.

### Verdict

**DESIGN — Reject the submitted completeness and zero-leakage guarantees; retain ordinary scoped bundles,
tamper checks and staged unsealing under explicit trust and leakage assumptions.**
My score remains **180/1000** for the package as submitted. **Confidence: 98%** that its stated mechanism does
not establish its headline guarantees. A revised proposal could earn a materially better score by giving a
complete committed-universe/query protocol and independently reviewed proof construction, or by narrowing
the promise to the existing verifiable-bundle capability without pretending it proves what was never supplied.

## Attack 2 — AGY-14: tractable scheduling equations are not validated rock mechanics

### Claim

**INFERENCE.** The [stope-sequence proposal, Idea 14](WIZARD_IDEAS_AGY_6_15.md)
makes a legitimate planning concern appear solved by a small linear influence model, a universal topological
cut and a resistant-binder alternative with infinite validity. The formulation's tractability is not evidence
that its schedule prevents rockburst, predicts dilution or remains admissible after fill degradation.

### Evidence

**FACT.** §16.3 and G-HEAD-7 already require exposure-specific fill strength and QA evidence, refusing
secondary/tertiary extraction when fill is insufficient, missing or conflicted. E233/E234 establish that
degradation matters; they do not establish an infinite strength window for a different binder formulation.
§14.3 requires empirical ground-control cards to carry validity domains and dilution forecasts to be scored
against cavity scans. These requirements remain after an optimization solver returns a feasible schedule.

**INFERENCE.** The proposed kernel makes stress linear in mining/fill indicators once its influence
coefficients are chosen. It does not establish that the chosen coefficients, superposition, scalar stress
representation or dilution conversion represent interacting openings in the intended domain. Matching a
single-opening analytic solution within 2% does not validate the multi-opening transfer model or the asserted
seismic and recovery consequences. Moving the same unchecked stress calculation to a post-solve certificate
does not cure that model-validity gap.

There is also a concrete limitation in the advertised anti-pillar cut:

```text
x_left + x_right − x_middle ≤ 1
```

With both neighbors mined and the middle unmined, its left side is 2. That remains true even if both
neighbors have mature, adequately verified fill, because the cut contains no fill or support variable.
An engineer may deliberately choose this conservative sequencing policy, but it is not a universal theorem
that every excluded primary/secondary state is physically inadmissible. It therefore needs policy ownership
and an opportunity-cost assessment, not the label of a general physical trap detector.

Finally, assigning a resistant 5% binder τ_max = ∞ converts missing long-term degradation evidence into an
unbounded permission. A finite tested curve with uncertainty, exposure dependence and an out-of-validity
refusal is the appropriate input. If an existing recipe falls below its acceptance basis, the permissible
response set must allow refusal, re-support or redesign; a modeled expiry is not an instruction to hurry
extraction. False envelope flags alone do not repair an overconfident engineering recommendation
(INV-002, G-HELM-1, G-HEAD-7).

**DESIGN.** A defensible narrow contribution would compare synthetic candidate schedules using finite,
engineer-approved cure/degradation cards and the existing guard, retaining unknowns. Stress-based optimization
needs independent interacting-opening benchmarks, a specified validity domain and site calibration before
any physical benefit claim. Public or synthetic evidence can establish model parity and implementation
correctness, not the claimed real-mine safety or recovered-gold gain (§33.1, §37).

### Verdict

**DESIGN — Reject the full stress-and-binder assurance package as submitted; retain a small two-sided,
evidence-bounded fill-window extension only within the existing human-reviewed planning workflow.**
My score remains **260/1000**. **Confidence: 95%** that the stated validation and approximately 600 LOC slice
do not support its advertised engineering guarantees. This does not reject all elastic screening models or
sequence optimization; it requires their scope and evidence to govern what the resulting card may claim,
with no geotechnical sign-off, operational authorization or control-system write path.
