# Codex: fifteen ranked additions worth building

**DESIGN — Ideation proposal, 2026-10-29.** These are proposed extensions, not implementation or gate results.
`COD-01` through `COD-30` are local candidate identifiers, not new plan or Bead identifiers. All examples below
are synthetic unless explicitly labelled FACT. Adoption expectations are HYPOTHESIS; acceptance thresholds are
TARGET. No customer demand, financial benefit, performance result or regulatory acceptance is claimed as measured.

**FACT — Review basis.** This proposal uses the constitution at repository revision
`68e709122ebb0c6b50fb6c1b5dbe558c3921a864`, its Appendix C, Appendix D, §37 and all §39 bead rows, the current
crate source and contracts, registries, and the read-only issue export. References to plan sections below mean
[the technical constitution](../../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md). No confidential source
document or held-back corpus note is reproduced here.

## Selection and ranking

**DESIGN.** I considered 30 candidates, screened each for an owner action, a genuinely additional capability,
an independently testable first slice, and a reason to stop. I then preferred features that turn the existing
evidence spine into a new answer a user can act on. Mathematical novelty alone did not earn a place. The other
25 candidates and their original dispositions appear after the detailed proposals; a title-only inventory ends
the file. A second pass below expands the next ten into ranks #6–#15 while preserving the original five.

| Rank | Candidate | The owner question it adds | Why it survives | Confidence in adoption, HYPOTHESIS |
|---|---|---|---|---|
| 1 | COD-01: Guidance completion test | What must happen from now to year-end, and can those requirements coexist? | Direct executive action; public-data entry; useful before a mine optimizer exists | Medium-high; test against finance's current guidance worksheet |
| 2 | COD-02: Rounding-aware contradiction finder | Can all these published figures be true simultaneously? | Immediate M0 value; turns a discrepancy into a small, defensible explanation | High for technical utility; medium for executive differentiation |
| 3 | COD-03: Independent-evidence accounting | Are we more certain, or have we counted the same observation again? | Protects QP, reconciliation and exploration decisions from false precision | High for correctness value; medium for willingness to change data practice |
| 4 | COD-04: Decision recall after evidence changes | Which previously reviewed decisions must be reconsidered now? | Converts provenance into ongoing decision maintenance and reduces review noise | Medium-high once real cards are in use |
| 5 | COD-05: Metal-to-available-cash bridge | When can the parent actually spend the value in this production forecast? | Gives treasury a concrete decision beyond another operating forecast | Medium; contract and cash-access data are a real gate |

**DESIGN — Build order differs from value ranking.** Establish honest source loading and the narrow arithmetic
slice of COD-02 before presenting COD-01 as assurance. Add COD-03 when observations begin entering statistical
models, COD-04 when reviewed cards exist, and COD-05 with the first finance engagement. This is not a proposal
to run five parallel infrastructure projects or postpone M0 until all five exist.

**FACT — Starting constraints matter.** The source loader currently substitutes generated workbook bytes on
read failure (`fmine-public/src/lib.rs:224`, `fmine-io/src/xlsx.rs:122`); the AISC bridge uses missing-to-zero
conversion and a fixed sales adjustment (`fmine-public/src/lib.rs:326–374`); and the North Mara example is
constructed from literals (`fmine-public/src/lib.rs:437–456`). The current Claim ID omits fields needed for a
complete semantic identity (`fmine-claim/src/lib.rs:312–339`). These are prerequisites to honest use of the
affected features, not novel ideas or evidence that the proposed features already work.

**DESIGN — Shared boundary.** All proposals produce anchored analyses with typed uncertainty and refusals.
All numerical examples and initial fixtures can be public or synthetic. Future site deployments retain their
data in their tenancy; no personal data enters this repository. No proposal actuates equipment, assigns reserve
classes, signs engineering work, asserts regulatory compliance, chooses accounting treatment or replaces FA as
the book of record. Proposed CLI names are illustrative; none is represented as an existing verb.

## 1. COD-01 — Prove what must happen to meet guidance

**DESIGN — Subsystems and users.** Bullion and Helm, with Portal inputs and Adit delivery; later consume
Pushback and Crucible capability envelopes. The primary users are the CFO, executive committee and site GM at
a guidance review. IR receives the same approved-basis analysis. Their agents can submit a target and retrieve
a machine-readable completion requirement rather than inventing a narrative about being on track.

### What is new

**FACT.** §10 and BD-067 freeze forward forecasts; §21.6 explains metric movements; §15 optimizes mine plans;
§23 imposes KPI constraints on capital choices. None explicitly specifies an inverse, partial-year completion
test that jointly solves the remaining operating and financial requirements of an already stated target.

**DESIGN.** Add that inverse problem. A forecast asks what might happen. This feature asks what must happen to
reach a selected point or boundary of guidance, then tests those requirements against a declared feasible
envelope. It exposes a small, verified conflict set when incompatible requirements exclude completion. It must
say whether its conclusion concerns a continuous envelope, an explicitly enumerated scenario set, or merely a
necessary arithmetic condition; those are different claims.

### The two-minute workflow

**DESIGN.** The CFO selects the original guidance vintage, the latest admissible actuals, the ownership and
metric basis, and one target boundary. The result shows:

1. What remains to be produced, sold and spent, with the actuals and target cells beside the calculation.
2. The required remaining throughput, grade, recovery and sales timing, where those inputs are available.
3. Whether those requirements can hold together within the declared operating envelope.
4. The binding assumption, the document needed to resolve it, and any explicitly supplied alternative that
   restores a feasible completion. Humans decide whether to retain guidance, commission work or change plans.

**INFERENCE — Synthetic known answer.** A 1,000 koz annual target after 400 koz delivered requires 600 koz.
With at most 10 Mt of remaining dry feed, grade at most 2 g/t and recovery at most 90%, the declared envelope
allows about 578.7 koz. At those tonnage and recovery limits, the required grade is about 2.074 g/t. The useful
sentence is: “The target exceeds this envelope; either the grade bound, available tonnes, recovery bound or
target must change.” It is not a claim that an entire real mine is physically incapable of meeting guidance.

### Mechanism and first shippable slice

**DESIGN.** Start with one site, an annual production range, a completed half-year and a remaining half-year.
Use exact quantities for cumulative identities and a small set of monotone, unit-checked production relations.
Support three outcomes: a verified completion witness, an exclusion bound under stated assumptions, or
indeterminate with the missing input. If only public totals exist, ship the required remaining production and
the conditional break-even calculation; do not manufacture a plant-capacity bound.

Add sold-volume and AISC requirements only after that positive slice works. Annual AISC is a ratio of summed
numerators to summed sold ounces, never an average of quarterly AISC. Inventory, ownership changes and circuit
boundaries remain explicit. A completion cannot combine the best grade from one joint scenario with the best
recovery from another. A finite scenario search that finds no witness says exactly that; it does not prove
infeasibility of every physically possible continuation.

The arithmetic belongs in the existing Bullion kernel surface, source selection in `fmine-public`, and a
compact result in `fmine-cli`. Reuse `Money`, quantity bases, calendars and Claim provenance. A small analytical
solver is enough for the first slice; a new general MILP solver, mine scheduler or daemon is unnecessary.
Later capability envelopes arrive from the planned domain engines rather than being independently rebuilt.

### Why this ranks first

**HYPOTHESIS.** A finance team is more likely to reuse “these are the exact remaining requirements, and this
pair conflicts” than a new probability of a miss whose calibration is still weak. It changes the meeting from
debating the forecast to identifying the binding assumption. It also creates a practical
bridge between public assurance and a site pilot: the first missing bound is a specific data request.

The innovation is converting disclosure targets into an executable inverse problem. The practical advantage is
that the first useful result needs neither a full digital twin nor an estimate of every uncertain probability.
I am confident in the arithmetic and usefulness of the question; customer preference over an existing finance
worksheet remains to be demonstrated.

### Tests and kill conditions

**DESIGN — Verification.** Use hand-derived synthetic cases and exhaustive enumeration of small discrete
continuations as independent oracles. Include a case where every marginal target is reachable but no joint
completion is; sold/produced differences; a divestment; an unavailable operating bound; and a denominator that
can reach zero. Test that unknown inputs produce indeterminate results and that a feasible witness satisfies
every declared equation. Reproduce public actuals and original guidance from independently captured locators,
including relevant basis and vintage distinctions from E451–E499; do not use candidate output as the oracle.

**TARGET / kill.** In a review of three historical guidance checkpoints chosen before seeing results, the tool
must expose a relevant requirement or inconsistency faster than the existing worksheet in at least two.
Kill the proposed product surface if it only restates subtraction, depends on unavailable bounds for every
useful conclusion, or encourages users to mistake scenario enumeration for physical impossibility. Keep its
exact arithmetic as a small library function rather than expanding it into a planning platform.

## 2. COD-02 — Find contradictions that rounding cannot explain

**DESIGN — Subsystems and users.** Portal and Bullion, with Hallmark reuse and Adit presentation. A controller,
IR reviewer, QP, regulator or JV partner chooses a published table and receives a reproducible answer to
whether its totals, components and ownership columns can share one consistent set of underlying values.

### What is new

**FACT.** §21.6, BD-065, BD-066, BD-272 and BD-275 already require reproduction, identity checks, a discrepancy
register and explicit rounding tolerances. E113–E115 and E144/E147 are existing discrepancy seeds.

**DESIGN.** Extend those checks from separate residual tests to a joint constraint problem. A displayed number
defines a permitted interval under its declared rounding rule. Each occurrence of the same semantic cell
refers to one underlying value across all applicable identities. Return a common feasible assignment, a
checkable contradiction, or an unresolved result. For a contradiction, show a small conflict set and the
minimum discrepancy that this proof establishes. Never silently repair the source table.

This adds a materially stronger explanation than either “outside tolerance” or “probably rounding.” It also
prevents individually plausible reconciliations from requiring different hidden values for the same total.

### The two-minute workflow

**DESIGN.** The reviewer opens a failed identity and sees the source cells, their display intervals, the
applicable equation and one of three statements: rounding admits a common explanation; rounding cannot explain
this conflict; or the rounding/basis information is insufficient. A partner or auditor can recompute the small
witness without opening every workbook. Agents receive the equation and cell set, not a prose verdict.

**INFERENCE — Synthetic known answer.** Under nearest-unit rounding, a total printed as `10` can round from
either `10.40` or `9.60`.
One breakdown has two components printed as `5.20`, requiring the total to be approximately `10.40`; another
has two components printed as `4.80`, requiring approximately `9.60`. Each breakdown can separately agree with
the rounded total. They cannot both describe the same total on the same basis. The shared-variable constraint
reveals the contradiction without guessing a corrected cell.

### Mechanism and first shippable slice

**DESIGN.** Ship additive quarterly/half-year identities and fixed, declared ownership multipliers first.
Represent decimals and rounding endpoints exactly. Preserve the distinction between display uncertainty and
measurement uncertainty: a rounding interval is not a confidence interval. An unknown rounding convention
widens the admissible set conservatively or produces indeterminate status; it never becomes a guessed policy.

Use bounded exact interval propagation and elimination on small identity groups. A feasible result needs a
complete assignment that satisfies the supported constraints. An infeasible result needs a replayable sequence
of sound bound deductions or a checked linear contradiction. Otherwise return indeterminate. A deletion pass
may shrink a conflict to an inclusion-minimal set, but must not call it the smallest-cardinality explanation.
Search budgets cap work and leave an honest unresolved result.

The first product is one diagnostic beside the existing discrepancy entry, not a new scoring regime or an
interactive spreadsheet editor. The registered ±1-last-digit M0 rule remains unchanged; this diagnostic cannot
silently reclassify an explained item as reproduced. Nonlinear recovery, uncertain ownership and ratio systems
can wait. Any later unit/header alternatives are labelled hypotheses and require review before becoming input
semantics. This avoids turning a useful repair into a general symbolic-algebra project.

### Why this ranks second

**HYPOTHESIS.** The public lane earns credibility when its findings withstand the obvious first question:
“Is that just rounding?” A small proof that answers that question is more useful than hundreds of unexplained
residuals. Equally valuable is withdrawing a false finding when a common rounded explanation really exists.

This feature is unusually accretive because it improves both sides of the product promise: fewer false
findings and stronger true findings. It reuses source anchors, exact arithmetic and the discrepancy register,
and can demonstrate value immediately in M0. It ranks below COD-01 because a better assurance finding may
still fail the executive “nothing new” test; inverse guidance addresses an explicit upcoming decision.

### Tests and kill conditions

**DESIGN — Verification.** Pin independently authored rational examples, including the shared-total example,
boundary ties, negative adjustments, mixed display precision and overlapping identities. On bounded small
linear systems, compare with an independent lab feasibility oracle; check every returned assignment and
contradiction separately. Metamorphic tests permute rows, split compatible components and convert exact units.
Public-filing reproductions exercise E113–E115 and E144/E147 with captured source facts, not guessed corrections.

**TARGET / kill.** Every seeded contradiction must produce a valid witness or explicit indeterminate result;
zero feasible test cases may be declared contradictory. On a reviewer-selected set of public discrepancies,
the feature must remove a false alarm or materially clarify a real one. Stop expansion if most cases require
an unsupported nonlinear solver or reviewers find the witnesses harder to assess than the original arithmetic.
Do not build a second validator hierarchy: retain only the small witness checker the consumer needs.

## 3. COD-03 — Count independent evidence before narrowing uncertainty

**DESIGN — Subsystems and users.** Plumb and Corebox, consumed by Touchstone, Strata, Crucible and Lodestar.
The QP, exploration lead or metallurgist asks what genuinely new observations support an estimate, which error
sources are shared, and whether more drilling, another assay method or a different laboratory would add
information. Their agents can distinguish another citation from another observation.

### What is new

**FACT.** The plan has provenance, weakest-input evidence composition, support algebra and a shared realization
index (§7–§9.5; BD-042–BD-046). It models spatial dependence and measurement covariance in individual domains
(§13 and §20). The current `ClaimDag` records derivation edges, not observation-reuse or shared-error contracts.

**DESIGN.** Add statistical dependency semantics to that lineage. Three reports quoting one assay are three
publication assertions about one observation, not three independent assays. Distinct assays may still share a
batch calibration error. Common random numbers align computations; they do not establish that the underlying
evidence is independent. These distinctions must affect model admission and uncertainty, not merely a source
count displayed for humans.

### The two-minute workflow

**DESIGN.** Beside an estimate, show the underlying observations, technical replicates and declared shared
error groups. A QP can inspect a sentence such as “eight cited documents; two observation campaigns; one shared
calibration source.” This is a dependency description, never an automatic effective-sample-size claim.
The user can compare the information added by another reading from the same system with an independent audit,
while the existing VOI machinery values the decision consequence where its model is admissible.

### Mechanism and first shippable slice

**DESIGN.** Start with explicit origin identities and typed relationships: publication copy, superseding
publication, repeated reading of one measurement, technical replicate, independent new observation, and shared
systematic-error group. For physical records use sample, assay-run and certificate/batch identities. For the
public lane use declared links between the originating observation and its subsequent publications. Matching
numbers or similar text alone never establishes identity. Automated proposed matches remain assertions until
accepted; uncertain relationships remain uncertain.

The first positive statistical slice is a small Gaussian measurement model with independent noise plus a
declared batch bias. It can use the existing Cholesky seam; it does not require a geostatistical engine.

**INFERENCE — Synthetic known answer.** For independent readings with a common additive batch error,
`y_i = μ + b + ε_i`, the variance of their mean is `σ_b² + σ_ε²/n`. More independent readings reduce the second
term, not the first. Reprinting an existing reading reduces neither. A separate, genuinely independent batch
can add information under the registered model. This is a useful, testable capability rather than refusal-only
work.

**DESIGN.** Schema identity belongs in `fmine-claim`/Corebox records; statistical admission and covariance belong
in the consuming kernel. Preserve raw records and publication vintages. Do not collapse legitimate replicates
simply because they share a sample, and do not treat a restated value as an independent observation. Unknown
dependence cannot default to zero correlation. If no defensible bound or model exists, withhold the narrower
interval and state what additional assumption would be needed. No upstream evidence class is upgraded.

### Why this ranks third

**HYPOTHESIS.** Users will accept uncertainty more readily when they can see why another document did not make
it disappear. This feature also points to a different action: an independent calibration or sampling audit
instead of buying more correlated observations. It makes the evidence-native thesis operational at a place
where a conventional provenance graph is insufficient.

The mathematical behavior is simple enough to verify before site data arrive, and the benefit spans nearly
every statistical subsystem. It ranks below the two M0 products because physical identity mapping is real work,
and a misleading independence interface would be worse than the current absence of one. The product must earn
its mapping quality on a narrow record family before becoming a workspace-wide promise.

### Tests and kill conditions

**DESIGN — Verification.** Duplicate a public-source assertion through different document paths and require
unchanged inferential information. Generate Gaussian batches with known common and independent errors and
check the analytic variance plus empirical interval coverage against sealed truth. Include independent batches
as a positive control, legitimate same-sample technical replicates, conflicting superseding values and unknown
origins. Reordering or copying publications must not narrow intervals. Origin IDs and dependencies are part of
the canonical inputs, so changing them must change the result identity.

**TARGET / kill.** The initial mapped family must have no false automatic merges in its independently reviewed
fixtures, and recover the analytic common-error result. Keep matching manual if that condition fails. Kill a
generic independence-scoring product if it depends on fuzzy document similarity, if shared-error parameters
cannot be justified, or if an existing domain covariance model already covers the whole use case. Retain only
the observation-reuse guard where that is the smallest useful outcome.

## 4. COD-04 — Recall decisions when their evidence changes

**DESIGN — Subsystems and users.** Plumb, Helm, Hallmark and Adit, with Bedrock anchors. A CFO, QP or GM selects
a previously reviewed card after a restatement, assay withdrawal, changed price deck or revised policy. The
product identifies which decisions require fresh review, which can receive a new bounded assessment, and which
have no dependency on the changed evidence. Agents obtain concrete affected outputs and supported next steps.

### What is new

**FACT.** INV-013, §6.4, BD-046 and the current `ClaimDag::invalidate_and_propagate` mark dependent Claims stale.
§10 has alarms; §13 has incremental spatial recomputation; §22 has flip conditions. Those mechanisms do not
specify a decision-level recall workflow with bounds on the consequence of a particular change.

**DESIGN.** Add the missing link: a source change becomes a bounded question about a previous action, its
conditions and its review obligations. This is more than another stale badge, and more than recomputing every
report on every update. The useful unit is “this reviewed decision and these supporting conditions changed.”

### The two-minute workflow

**DESIGN.** The user opens an old card and a new evidence cut. The result lists affected alternatives, amount
or interval changes, violated preconditions and any changed action ranking. It also distinguishes numerical
changes from lost admissibility: a tiny amount change can still demand review if its supporting attestation,
calibration, scope or wall permission no longer holds.

A new assessment may establish that the former preferred action still wins within a stated bound. The old
Claims remain stale; the system never relabels them current or carries an old sign-off forward. A human decides
whether the new assessment suffices. A regulator or partner can inspect both evidence cuts and the exact reason
a previously supplied analysis was superseded, within their existing access rights.

### Mechanism and first shippable slice

**DESIGN.** Begin with one imported public revision and one simple DecisionCard over a small deterministic
recipe: sums, products, ratios and explicit alternatives. Traverse the existing dependency graph and recompute
the supported affected expressions. Attach the new source anchors and a before/after explanation. Any
unsupported model node requests a full rerun instead of receiving a guessed sensitivity.

Then add conservative change bounds. If an alternative's previous advantage exceeds a valid bound on the
change in its advantage, a new assessment can establish unchanged ranking. Bounds must cover interactions,
validity-domain exits and denominator singularities; local derivatives alone cannot certify nonlinear changes.
A bounding failure produces “review/recompute required,” not “immaterial.” Materiality is a versioned,
human-selected policy, not a hidden universal dollar threshold.

Pure graph and expression operations remain below Adit; orchestration and presentation reuse the current
public/CLI seams. If scheduled later, asupersync owns the bounded task region and cancellation. No correctness
claim depends on a scheduler completing, and cancellation leaves the old result stale. A complete canonical
identity and input closure are prerequisites: a digest that omits support, uncertainty or policy changes cannot
serve as a recall key. Fix that defect narrowly rather than inventing a separate identity system.

### Why this ranks fourth

**HYPOTHESIS.** Maintaining the validity of an answer is more valuable than delivering it once. A reviewed card
otherwise starts decaying immediately as source material changes. Users need to know whether they must stop
using it, and reviewers need relief from updates that have no decision consequence. This feature gives both
without allowing the software to renew a human approval.

It can make agent use substantially more reliable: an agent receives a precise invalidation cause and affected
decision rather than continuing to cite a plausible old number. Its marginal cost can remain small because the
plan already pays for anchors, lineage and cards. It ranks fourth because it needs actual reviewed outputs;
building it before there are decisions to maintain would be premature infrastructure.

### Tests and kill conditions

**DESIGN — Verification.** Compare every incremental result and bound against full recomputation on small
synthetic expression graphs. Seed changes that are immaterial, reverse a ranking, leave values unchanged but
invalidate evidence, cross zero denominators, and jointly overturn a decision despite small individual effects.
Use public comparative revisions described by E474 as a reproduction case, retaining both publication vintages.
Test dependency cycles/corruption as refusals. Once asynchronous execution exists, LabRuntime cancellation and
restart tests must leave no false current assessment and no inherited sign-off.

**TARGET / kill.** Zero seeded decision reversals or admissibility losses may be labelled eligible for a
no-change assessment. If conservative bounds request nearly every full rerun, ship exact incremental reruns
and the recall explanation; abandon the bounding optimization. If the workflow does not reduce missed reviews
or review effort on a preselected card set, keep ordinary staleness propagation and stop there.

## 5. COD-05 — Show when metal becomes spendable cash

**DESIGN — Subsystems and users.** Bullion, Crucible and Helm, with Touchstone quantities and Adit delivery.
Treasury and the CFO inspect whether near-term obligations can be funded from cash actually available to the
relevant entity. Site GMs see which physical or documentary hand-off delays that cash. JV partners can verify
the portion of the bridge they are entitled to see. Agents get a dated cash-availability constraint, not an
instruction to borrow, trade, distribute funds or accelerate an unsafe shipment.

### What is new

**FACT.** §17.6 covers custody and outturn; §21.5 part 8 covers settlement and provisional pricing; §23 covers
liquidity floors and joint scenarios; BD-314 adds entity waterfalls, DSCR locks and distribution restrictions.
Those are important planned ingredients, not new proposals here.

**DESIGN.** The additional product is a date-specific, source-traceable bridge from a physical production or
inventory forecast to funds spendable by a selected legal entity, plus a reverse test of price and payment-delay
conditions that breach a declared cash floor. It makes the timing and accessibility gap an explicit owner
workflow. A positive group NPV, an attributable production figure and a receivable are not substitutes for that
dated availability calculation.

### The two-minute workflow

**DESIGN.** Treasury chooses an entity, a horizon and a policy cash floor. One bridge shows forecast recoverable
metal, shipment/outturn, contractual payable metal, provisional/final receipts, encumbrances and permitted
entity transfers, each on its own date and basis. Certified FA cash and receivable reads are shown separately
from prospective Claims. Unknown contract terms remain unknown; an analyst cannot make them disappear by
choosing a convenient settlement date.

The user then asks “How late can this receipt be?” or “Which price-and-delay combinations breach this floor?”
The output is a frontier under explicit assumptions, with the earliest breach and the amount/date of an
additional inflow needed to avoid it. The user decides what finance or operational action is appropriate.

**INFERENCE — Synthetic known answer.** With opening unrestricted cash of $100M, an $80M outflow on day 10
and a $40M floor, a $50M receipt on day 15 leaves a $20M shortfall against the floor on day 10. The final
balance looks adequate, yet at least $20M must arrive by day 10 to maintain that floor. Moving a receipt between
dates changes liquidity without changing produced ounces or annual revenue.

### Mechanism and first shippable slice

**DESIGN.** Ship one synthetic doré batch, one provisional/final settlement schedule, one currency and one
entity, with a finite set of declared payment delays. Use an ordered event fold over exact `Money`. Keep earned
revenue, receivable, cash receipt and available cash distinct; never count the same provisional/final amount
twice. Add one explicit JV transfer restriction only after the single-entity case is sound. Unknown transfer
permission means unavailable or unresolved under policy, never an assumed release.

This is not an initial full treasury system. Defer hedge optimization, debt execution, legal interpretation,
general contract extraction and multi-country tax engines. Use role-reviewed term records; synthetic terms are
not represented as Barrick agreements. Public filings can demonstrate published balances and physical totals,
but generally cannot establish exact shipment payment terms or a parent's real available cash.

The cash arithmetic belongs in the plan-mapped Helm economic kernel and Bullion adapter slices when those
crates are admitted with real functionality; they do not exist today merely because the plan names them.
Reuse `Money`, calendars and Claim types, and read FA through the planned subprocess boundary. Do not expand
`fmine-core` into a finance engine or give this feature a second balance of record. Later joint price/delay draws
must use the registered scenario space; probabilities remain uncalibrated until scored.

### Why this ranks fifth

**HYPOTHESIS.** Treasury may find a specific funding gap more decision-relevant than another NPV chart, while
the GM gets a clear reason an assay, outturn or documented hand-off matters financially. This is a different
buyer workflow built from the same physical evidence, which can make the project useful outside technical
services. It also catches an expensive category error: treating production, attribution or accounting value as
immediately accessible money.

The first mechanism is only dated cash arithmetic; the difficult part is obtaining and validating its terms.
That is why the idea ranks fifth rather than being sold as an immediate M0 feature. Confidence is conditional
on a treasury sponsor providing a narrow, reviewable data set. Its scope should follow access rather than
creating a speculative implementation of every financial arrangement in §23.

### Tests and kill conditions

**DESIGN — Verification.** Use independent exact-money worked examples and synthetic custody/settlement
histories. Cover partial shipments, moisture changes, payable deductions, provisional-to-final true-ups,
weekend/payment-calendar boundaries and restricted JV distributions. Moving a receipt after an obligation
must not improve the earlier balance. Splitting a receipt must conserve cash; entity transfers must cancel at
the group boundary while preserving restrictions; closing receivables and cash must reconcile to the certified
FA reads when integrated. A lab FA conformance case verifies read-only integration; synthetic results remain
labelled synthetic. A missing term must prevent a falsely precise availability date.

**TARGET / kill.** Before expanding beyond the synthetic slice, a treasury reviewer must identify a real
funding, timing or distribution decision that the existing capital cash-flow view does not answer. Stop if the
necessary terms cannot be obtained, if the result duplicates the current treasury system, or if implementation
starts requiring a replacement GL or broad legal/tax engine. Retain the small event-fold calculation and defer
the product until the data gate opens.

## The next ten: ranks #6–#15

**DESIGN — Second selection pass.** These ten come from the original candidate pool. Rank numbers express
priority in this document; the `COD-*` identifiers remain unchanged so reviews can compare the two passes.
The new details supersede the brief disposition where a candidate was previously marked for deferral; they
do not remove its data gate or imply that it should be built immediately. No code, Beads or plan changes are
part of this expansion. The review remains anchored to the same source revision stated above.

| Rank | Original candidate | Owner action added | Why it comes next |
|---|---|---|---|
| 6 | COD-06 | Offer the smallest sufficient data set for one useful answer | Addresses the access bottleneck without demanding an enterprise integration |
| 7 | COD-08 | Judge forecasts against the complete frozen cohort and outcome vintages | Prevents an impressive track record from being an artifact of missing or revised outcomes |
| 8 | COD-28 | Decide whether untested ore can reverse an expansion or processing choice | Turns a coverage warning into a bounded economic question and targeted testwork |
| 9 | COD-07 | Resolve a JV disagreement at its first comparable divergence | Gives partners something actionable beyond exchanging two internally consistent bundles |
| 10 | COD-10 | Draft a numerical statement with the exact qualifications its evidence requires | Carries assurance through the final sentence that executives and investors actually read |
| 11 | COD-26 | Choose which evidence family deserves an independent audit | Tests robustness to a whole source failure rather than only small parameter movements |
| 12 | COD-14 | Determine which quantities remain estimable after a measurement outage | Supports a defensible close and a precise measurement-recovery request |
| 13 | COD-17 | Buy information only when it can still change the commitment | Adds delivery uncertainty and expiry to otherwise attractive VOI |
| 14 | COD-09 | Compare only what the available metric definitions can identify | Creates useful bounded comparisons without manufacturing a universal peer ranking |
| 15 | COD-16 | Separate prediction quality from the causal benefit of acting on a recommendation | Makes vendor and internal improvement claims assessable when intervention changes the outcome |

**HYPOTHESIS — Ranking confidence.** #6 and #7 have the clearest connection to the immediate wedge. #8–#10
offer strong specialist workflows with larger semantic or access requirements. #11–#14 are focused extensions
that should be admitted by an existing audit, close, study or comparison task. #15 has potentially high value,
but its causal identification and sponsor requirements make it the least immediately buildable of this set.
None needs a new top-level platform. Shared mechanisms should be reused without treating closely related
features as separate proof of customer value.

## 6. COD-06 — Answer the owner's question with less shared data

**DESIGN — Subsystems and users.** Portal, Touchstone, Plumb and Adit. A site GM, metallurgist and data steward
negotiate what to supply for one named assurance question. The executive sponsor sees the answer each smaller
offer would enable. An agent can request the missing fields for that question without broadening its own rights.

### What is new

**FACT.** §34.4 defines access gates and §37.3 lists pilot inputs. §32 and BD-185–BD-186 require declared
import semantics; §20.6 checks identifiability once a measurement model is available. These are necessary
ingredients, but do not specify an executable comparison of alternative, smaller data offers before ingest.

**DESIGN.** Add question-specific answerability. Given an explicit computation and an offered schema, report
which result can be produced, at what support, and which alternative minimal input sets could produce it.
Minimal means no field set can be reduced further within the enumerated recipe catalogue. It does not mean a
globally cheapest or universally sufficient data set. Price, access burden and permitted disclosure can be
owner-supplied selection criteria; the software does not invent a privacy or procurement score.

### The two-minute workflow

**DESIGN.** The GM chooses a narrowly defined recovery-reproduction question and selects the fields the site
is willing to supply. The preview returns a computation it can run, its missing prerequisites and an alternative
offer, such as dry feed mass directly versus wet feed mass plus per-lot moisture. Both routes must reach the
same quantity, time window and metal stage; a different recovery definition is a different answer.

The sponsor can approve the smaller offer, choose a weaker question, or ask for a measurement that separates
two explanations. The product explicitly distinguishes reproducing a published recovery from attributing a
sampler bias. Monthly totals may enable the former while leaving the latter unidentified. That distinction
prevents an easy first import from being sold as completion of the pilot's finding mechanism.

### Mechanism and first shippable slice

**DESIGN.** Start with two hand-authored recipes for one material-balance question. Each recipe declares units,
bases, time alignment, inventory terms, admissible evidence and output semantics. Execute the selected recipe
on synthetic values as part of the preview. A mere list of requested columns is insufficient. A schema-complete
offer can still fail on stale measurements or incompatible support when values arrive; structural answerability
is a necessary condition, not a certification of data quality or statistical identifiability.

Use a finite dependency expansion over the existing computation inputs, with deterministic subset elimination.
No query planner or automated discovery of arbitrary equations is needed. Describe these recipe choices as
product data consumed by Portal's preview and ingest paths; give them the existing registry validation seam,
not a new registry framework. Keep a boundary when a field may be derived only under an additional assumption.
Never substitute a public aggregate for a private measurement merely because their labels match.

The first slice accepts a supplied schema and public/synthetic examples without reading site values. It can
therefore make a reviewable data offer without transferring sensitive raw data. Later in-tenancy execution
honors the normal rights and residency checks before any field is read; the preview does not grant access or
reveal whether inaccessible records exist. A bound or refusal must be returned with an explicit explanation
when the offered data cannot answer the question.

### Why this ranks sixth

**HYPOTHESIS.** A request for a small, explainable data offer is easier for a sponsor to act on than a request
for eight systems' exports. The useful product is the answer unlocked by that offer, not the request document.
It can also reduce wasted implementation: a proposed input set that cannot identify the intended question is
rejected before anyone builds its importer.

This is the strongest runner-up because the plan already treats access as a major delivery risk. Its
mechanism is modest and its first result can run before private data arrive. It stays outside the original
five because the gain depends on an actual negotiation, and some useful questions may genuinely require the
full evidence set. I am confident in bounded recipe enumeration; faster customer access remains a hypothesis.

### Tests and kill conditions

**DESIGN — Verification.** Exhaustively enumerate all field subsets for the two synthetic recipes and compare
the returned minimal sets with an independently authored truth table. Verify the positive answers against
hand-calculated balances. Remove inventory, change moisture basis, shift the reporting window and withdraw a
measurement to ensure structural and value-level refusals remain distinct. Adding an irrelevant column must
not change the result. Adding inaccessible data must not change an unauthorized preview.

**TARGET / kill.** Before extending the catalogue, one sponsor/data-steward review must select a smaller offer
that still produces a useful, explicitly named answer. Stop if the output is only an onboarding checklist,
the smaller offer cannot be executed, or recipe maintenance starts exceeding importer work. Retain the two
recipes as ordinary input validation if they do not change a real sharing decision.

## 7. COD-08 — Keep missing outcomes and restatements in the forecast record

**DESIGN — Subsystems and users.** Plumb, Portal and Adit. The CFO or executive committee evaluates a model,
human role or agent from a fixed cohort of forecasts. QPs and auditors can see what was originally promised,
which outcomes were observable at each review, and whether later restatements change the conclusion.

### What is new

**FACT.** §10, BD-047–BD-049, BD-076 and BD-276 require preregistration, scoring and resolution criteria.
§22.7 requires a resolution or declared non-resolution. The current `FrozenForecast` stores one optional
outcome, and `evaluate_model` scores only entries with a realized value
(`crates/fmine-calib/src/lib.rs:16–155`). It does not represent a history of outcome vintages or cohort bounds.

**DESIGN.** Extend resolution into an append-only, versioned assessment while keeping the forecast immutable.
Display the full frozen denominator, the resolved subset, overdue/missing outcomes, and separate scores under
the original eligible publication and later restated publication. Later outcomes cannot silently replace the
earlier assessment or become additional independent observations.

### The two-minute workflow

**DESIGN.** The CFO selects a cohort defined before its outcomes, such as one model's site-quarter forecasts
for a fiscal year. The result shows resolved, not-yet-due, late, withdrawn, definition-changed and otherwise
unresolvable entries, with their predeclared rules. It presents scores on the observed subset beside what the
whole-cohort result could still be, where honest bounds exist. A drill-through reveals the publication vintage
and basis that resolved each entry.

**INFERENCE — Synthetic known answer.** Of ten frozen 80% intervals, eight outcomes are available and seven
fall inside. Observed-subset coverage is 7/8. Whole-cohort realized coverage could finish anywhere from 7/10
to 9/10. That range is a bound on the missing outcomes' possible contribution, not a statistical confidence
interval and not proof that coverage is calibrated. Hiding the two missing entries would erase this distinction.

### Mechanism and first shippable slice

**DESIGN.** Ship one cohort with a frozen membership list, a resolution rule and a small sequence of outcome
events: initial publication, correction and explicit missingness. Every assessment pins its as-of cut, eligible
outcome source and definition version. An event whose meaning changed is not automatically converted to the
old forecast's target. If the contract did not specify a usable fallback, retain non-resolution with its cause.
No convenient retrospective cohort exclusions are allowed.

For bounded scores such as Brier loss, compute aggregate best/worst contributions of unresolved outcomes.
For continuous targets without a defensible range, say that no finite aggregate bound is available; never
manufacture a CRPS limit or fill a missing actual with its forecast mean. The original estimate, an updated
estimate and a restated actual are distinct objects. Model correction learning uses the explicitly selected
assessment vintage and time cut, avoiding accidental double counting or lookahead.

The new state is consumed by `fmine-calib`'s scoring functions and Portal's outcome selectors. Reuse the
planned ledger; do not build a parallel reporting database. A static report or stable CLI result is sufficient
for the first positive slice. The official M0 numeric-cell denominator remains governed by BD-272; this is a
forecast-cohort feature, not a license to redefine the reproduction gate.

### Why this ranks seventh

**HYPOTHESIS.** A track record is persuasive only if difficult outcomes cannot quietly disappear from it.
Showing what remains unresolved makes comparison fairer to both the model and its reviewers, while exposing
whether an apparent improvement comes from a new forecast or a new version of history. This strengthens the
project's flagship freeze-and-score promise rather than adding another chart.

It ranks immediately after answerability because it can ship on public releases and synthetic cohorts with
little new mathematics. It remains an extension to forecast delivery rather than a separate platform. I am
highly confident in the accounting of outcomes; any claim that it improves executive trust needs a reviewer
test, and small cohorts still do not justify strong calibration claims.

### Tests and kill conditions

**DESIGN — Verification.** Use exhaustive binary outcomes to check cohort coverage and Brier bounds. Reproduce
the synthetic ten-entry example by hand. Add a late correction and ensure the original as-of report remains
byte-identical; later views must show the new vintage separately. Public comparative changes identified by
E474 supply a source-reproduction case after independent fixture capture. Test that an unresolved extreme
case cannot improve a model's whole-cohort ranking merely by being omitted.

**TARGET / kill.** Every registered forecast must remain accounted for at every assessment cut. Stop any
ranking feature that cannot expose its denominator, resolution policy and publication vintage. If existing
ledger/report work already delivers these semantics, merge the requirements into it rather than creating a
second scoreboard. Kill whole-cohort numeric summaries where unbounded missing outcomes make them indefensible.

## 8. COD-28 — Bound what untested ore could do to the decision

**DESIGN — Subsystems and users.** Crucible, Strata and Helm, with Hallmark consuming support material only.
A metallurgist, exploration lead or QP asks whether an untested ore domain could overturn a processing route,
expansion case or proposed testwork budget. The board sees the economic consequence of the sampling gap.

### What is new

**FACT.** §17.4 and BD-300 already require geometallurgical coverage by percentile band and `OutsideValidity`
for extrapolated cards. BD-289 calibrates a lab-to-plant recovery card. Those mechanisms identify an evidence
gap; they do not specify partial-identification bounds on the aggregate decision consequence of untested ore.

**DESIGN.** Extend coverage into a decision bound. Combine measured/tested portions with explicit bounds on
the untested portion, preserving the uncertainty in each portion's weight. Report whether the proposed choice
survives every admissible completion of the untested domain. If it does not, identify which targeted test could
shrink the decisive bound. This adds an answer without promoting an out-of-validity recovery prediction.

### The two-minute workflow

**DESIGN.** The metallurgist opens a candidate's feed population and selects the testwork that actually
represents it. The card reports covered and uncovered contained-metal fractions, the source of their bounds,
the resulting aggregate range, and any action-ranking reversal across that range. A QP can use the evidence
beside their own judgment; no reserve class or inferred conversion recommendation is emitted.

**INFERENCE — Synthetic known answer.** Under an explicitly additive parcel model, suppose 80% of feed metal
belongs to tested parcels with 90% recovery. The remaining 20% has recovery bounded at 40–90%. Aggregate
recovery lies in 80–90%; assigning 90% to everything would hide the downside. The weights here are fractions of
contained feed metal, not dry tonnes. A decision threshold at 85% straddles that bound and merits more evidence.

### Mechanism and first shippable slice

**DESIGN.** Start with two ore strata, one process route and fixed, independently supplied contained-metal
weights. Keep observed uncertainty, selection uncertainty and transfer assumptions separate. The uncovered
bound may be a physical 0–100% range or a narrower, cited model/elicitation bound with its proper evidence
class. A narrower assumption is never rendered as measured recovery or inserted into a disclosure-facing claim.
If the mass or grade of the uncovered stratum is unknown, propagate its allowed weight range rather than
silently normalizing the tested specimens to represent the whole feed.

The first kernel uses exact weighted bounds and an exhaustive endpoint check on a small declared action model.
It should return a positive aggregate bound and a stable/unstable choice assessment. It must not need a spatial
simulation engine or learn a selection model to be useful. Coverage mapping comes from the planned Crucible
and Strata seams, with decision use in `fmine-decide` once the bound is Claim-typed.

Additivity is an explicit validity condition: chemistry, blending or throughput interactions can invalidate
parcel-wise weighted recovery. In that case return an unresolved aggregate or use a separately validated joint
process model. Do not declare that sampling weights alone repair nonlinear extrapolation. Test selection may
be informative; these bounds deliberately avoid pretending it was random. A later test recommendation uses
the existing VOI machinery only where a defensible likelihood exists, otherwise an unpriced separating test.

### Why this ranks eighth

**HYPOTHESIS.** A coverage warning tells the user something is missing; an aggregate bound tells them whether
the missing evidence matters to the commitment. Some cases will support proceeding with a robust choice;
others will justify a precisely scoped campaign. Both are better than either treating an unrepresentative
test average as truth or refusing every calculation because one domain lacks coverage.

I am confident in the bounded first slice because its arithmetic has independent known answers. Transferable
real-world bounds and representative domain weights remain genuine data gates. It ranks below the two earlier
extensions because those gates may make its initial real-site bounds wide, and the existing `OutsideValidity`
warning may already be enough for some users. The feature must demonstrate an additional decision consequence.

### Tests and kill conditions

**DESIGN — Verification.** Enumerate all endpoint assignments of a synthetic uncovered stratum and check that
the aggregate and action bounds enclose every admissible truth. Include different grades to catch tonne-versus-
metal weighting errors, uncertain stratum weights, deliberate selection of easy ore, and a nonlinear blending
negative control that must reject the additive model. Independent hand calculations verify the 80–90% example.
More valid information may narrow the feasible set; merely duplicating a tested sample must not narrow it.

**TARGET / kill.** A metallurgist must identify a commitment or testwork choice that changes because of the
computed bound rather than the coverage badge alone. Stop if the model needs unverifiable lower bounds to be
useful, if nonlinear interactions defeat the chosen slice, or if results are mistaken for software resource
classification. Retain the coverage report and defer the decision extension when the bounds are uninformative.

## 9. COD-07 — Locate the first comparable difference in a JV disagreement

**DESIGN — Subsystems and users.** Bullion, Plumb, Portal and Bedrock, surfaced through Adit. A JV controller
and partner reviewer compare two authorized statements about a shared period. Their agents can identify a
specific basis, version or input difference and prepare a review request without asking for the partner's
entire evidence store.

### What is new

**FACT.** §31.2 already gives partners scoped projections and verifiable FA/FrankenMining bundles; §23 and
BD-314 model entity and ownership waterfalls. The new feature is a bilateral comparison protocol over the
mutually authorized portion of two evaluations, rather than another bundle or a new privacy mechanism.

**DESIGN.** Compare the declared question, period, basis, interest, metric definition, recipe version and
visible inputs in a deterministic order. Return the first comparable divergence and the authorized numerical
consequence of reconciling it. If multiple changes interact, retain that fact; first in comparison order does
not mean first cause, legal responsibility or globally minimal explanation.

### The two-minute workflow

**DESIGN.** The controller loads an operator statement and the partner's authorized statement. A concise result
might establish that both reproduced their own source correctly, but one used a different ownership effective
date or sold-ounce denominator. The users choose whether to rerun on an agreed basis or retain a documented
disagreement. If visible inputs agree but outputs differ, the next question is the recipe implementation or
version. If the necessary comparison is not authorized, the result stops at that boundary.

This is useful even when neither party is wrong: two valid statements can answer different questions. The
result should reduce the dispute to a named difference and a test, not manufacture a blended compromise figure.
Any message or exchange remains a human-authorized action; the first product only prepares a local comparison.

### Mechanism and first shippable slice

**DESIGN.** Start with two public/synthetic copies of one period AISC or attributable-production bridge,
constructed independently, with one seeded definition or ownership difference. Compare explicit structured
exports, not arbitrary natural-language contracts. Normalize units only through declared exact conversions.
A change of economic basis is not a unit conversion and must remain visible as a separate evaluation.

The first report verifies both visible calculations and computes an authorized counterfactual under a shared
input choice. Reuse source anchors, `Money`, quantity basis tags and the planned partner projections. Keep the
comparison in Portal/Bullion adapters and the result in the existing report surface. No new shared database,
partner identity system, consensus ledger or cryptographic proof system is needed.

Hashes can establish that the parties committed the same visible bytes; they do not prove arithmetic over
undisclosed inputs. The first slice makes no such claim. Comparisons must not enumerate inaccessible branches,
their record counts or the existence of secret documents. Where the agreed export cannot support a numerical
explanation, return not comparable within this authorized scope. Access to an explanation is never inferred
from entitlement to a top-line figure, and shared results cannot carry a partner's walled valuation.

### Why this ranks ninth

**HYPOTHESIS.** A partner needs more than “our bundle verifies” when the other verified bundle gives a different
answer. Identifying the first comparable divergence can shorten a difficult reconciliation and make the same
evidence layer useful on both sides. It also offers a modest route to JV adoption: demonstrate the comparison
on an agreed public or synthetic example before anyone shares sensitive data.

The first mechanism is straightforward; the constraint is agreement on the exported question and fields. This
is why it ranks below single-owner workflows. I am confident that a narrow comparison can locate seeded basis
differences, but partner acceptance and actual dispute-time savings are untested. It should be a task-specific
tool inside existing walls, not a bid to replace a JV's governance or legal processes.

### Tests and kill conditions

**DESIGN — Verification.** Independently author paired synthetic statements with ownership changes, stale
definition versions, produced/sold bases, permissible rounding, and actual arithmetic defects. Verify the
explanation by hand and ensure swapping the parties reverses signed deltas without changing the disagreement
class. Unchanged authorized exports must produce indistinguishable outside-principal results when hidden data
are added or removed, following the plan's wall-test approach. Include mixed changes where no single cause
can be established.

**TARGET / kill.** Two role-based reviewers must agree on the identified difference and a concrete next test
without receiving additional unauthorized data. Stop if meaningful comparisons require rights the parties will
not grant, if the report is less clear than a simple bridge spreadsheet, or if anyone needs zero-knowledge
arithmetic guarantees to accept it. Do not solve that last objection by quietly beginning a cryptography project.

## 10. COD-10 — Keep the final numerical sentence bound to its evidence

**DESIGN — Subsystems and users.** Adit, Portal, Bullion and Hallmark. IR and finance reviewers draft short
numeric statements for a release, board paper or QP support document. Agents can populate a controlled draft
from Claims while preserving its period, basis and uncertainty qualifications. Humans edit, approve and file.

### What is new

**FACT.** §30.5 and BD-072/BD-123 already require deterministic reports with anchors, clickable numbers and
the `no_claim` line. §24 has human-controlled disclosure renderers. The extension is semantic binding of a
small vocabulary of numerical sentences, so a valid table cannot be summarized with a misleading denominator,
comparison period or omission of qualifications.

**DESIGN.** Represent each supported sentence as a typed assertion with Claim slots and an explicit predicate:
level, period change, share of total, or interval. Derive the displayed wording and numbers together. Attach
the evidence cut and required qualifications to the sentence, and invalidate the binding if the text is changed
outside the supported template. An ordinary free-text paragraph can coexist, but is labelled outside this check.

### The two-minute workflow

**DESIGN.** The reviewer selects two comparable Claims and asks for a draft period-change sentence. The result
includes the named units and ownership basis, a consistent change calculation, uncertainty where required,
and the source references. If the Claims use different per-ounce denominators or their intervals do not support
a precise increase, the draft offers a suitably qualified range or refuses the requested wording.

An agent cannot strip “uncalibrated,” change attributable to consolidated, or attach the sentence to a different
quarter while retaining its verified binding. A reviewer sees exactly what became unchecked. The benefit is
preserving meaning through the final editorial step, not claiming that generated prose is legally sufficient.

### Mechanism and first shippable slice

**DESIGN.** Start with three sentence forms: a reported level, a like-basis absolute change, and a like-basis
percentage change with a strictly positive baseline. A percentage-point change is a different operation from
a relative-percent change. Zero/negative baselines require an explicit alternative convention or a refusal.
Do not introduce adjectives such as safe, compliant, on track or economically viable into the initial grammar.
Those need separate admissible propositions; formatting two numbers cannot establish them.

Use a compact typed sentence model and deterministic Markdown/NDJSON rendering in the existing surface
architecture. The consumer is the renderer and its draft-review command; the binding is product state rather
than an editorial checklist. Reuse the Claim's provenance and planned export guards. An input's missing
calibration, uncertainty or scope qualifier becomes visible text or blocks the supported sentence, not a
footnote that a prose generator may omit. Source updates use COD-04's invalidation semantics when available.

Do not build a natural-language theorem prover, a new document editor or a general LLM fact checker. An LLM
may suggest the desired sentence form as an untrusted draft; deterministic code fills and checks its typed
slots. For a QP pack the software renders evidence about the human's statement, and never emits a class
assignment or adopts the QP's authority. The initial demo uses only public Claims and synthetic range examples.

### Why this ranks tenth

**HYPOTHESIS.** The sentence that escapes the table is often the one people remember. Preserving the right
numbers while losing their ownership, period or uncertainty can defeat the whole assurance workflow. This
feature carries correctness into an artifact IR, executives and regulators actually consume, while giving
agents a narrowly reliable writing task.

It is buildable with a small grammar and existing rendering rather than a new language model. It ranks below
the preceding features because controlled templates cover only part of a real document, and reviewers may
find them restrictive. I am confident in checking the supported forms; the real product test is whether those
forms capture enough recurring work to save review time without implying broader prose assurance.

### Tests and kill conditions

**DESIGN — Verification.** Pin hand-authored input/wording pairs and independently recompute every derived
number. Mutate units, ownership, period, source vintage, percent conventions and calibration flags one at a
time; the draft must change or lose its verified binding. Test interval comparisons, zero baselines and copied
numbers with different meanings. Golden renders must retain all mandatory qualifiers. Reproduce one public
metric comparison with its exact locators, without treating the public statement as proof of economic causation.

**TARGET / kill.** A finance/IR reviewer must complete a small preselected release-review task faster with no
additional semantic errors and correctly distinguish checked from unchecked prose. Stop if useful coverage
requires unrestricted language understanding, if reviewers routinely remove essential qualifiers to make the
draft acceptable, or if the output is mistaken for filing approval. Keep only the sentence forms that pass.

## 11. COD-26 — Find the evidence family whose failure would change the choice

**DESIGN — Subsystems and users.** Plumb, Helm and Hallmark, consuming Corebox/Crucible observation families.
A QP, GM or audit lead chooses where to spend an independent review budget before a major commitment. Agents
can run a bounded set of declared evidence-withdrawal cases and return the assumptions that actually hold the
decision up.

### What is new

**FACT.** §22 already requires parameter flip margins, robustness views and VOI; §9.4 ranks unused evidence.
COD-03 adds observation identity and shared-error semantics, while COD-04 handles an actual source change.
This extension asks a different prospective question: what happens if an entire registered evidence family is
withdrawn before it is known to be defective?

**DESIGN.** Refit or recompute the decision with each family unavailable under the original, frozen fallback
rules. Report whether the action changes, its advantage shrinks, or the available evidence becomes insufficient
to compare actions. This is source-level fragility analysis; it neither estimates the probability a source is
wrong nor equates a hypothetical withdrawal with a discovered defect.

### The two-minute workflow

**DESIGN.** The audit lead selects a candidate card and asks which source family to check independently.
The result might show that withdrawing one batch calibration removes the basis for a recovery premium, while
withdrawing a reprint of the same report adds no new case. Another family may affect the range but leave the
choice unchanged. The user can commission the high-consequence audit or accept the explicitly described
dependence. The displayed question is about the decision's support, not a league table of trustworthy people.

Rank cases first by lost admissibility, then changed preference and the supported change in advantage. An
owner-supplied review cost can help choose among comparable cases. Do not report an expected audit value
without an admissible model of audit outcomes; sensitivity to withdrawal is not itself VOI.

### Mechanism and first shippable slice

**DESIGN.** Start with two action alternatives, a small Gaussian measurement model and two explicit source
families. Freeze the baseline priors, model version and action semantics. Remove the observations of a family,
recompute the posterior/expected-loss comparison, and retain all valid evidence from the other family. A
withdrawn record is not converted to zero, and the remaining data are not silently reweighted to represent a
population they did not sample. If the original model needs that family to identify a parameter, the correct
result can be loss of comparability.

Family definitions come from explicit provenance and error-group declarations as in COD-03, not file counts.
Removing a calibration source must remove or downgrade all dependent observations that require it. Those
relationships can initially be hand-authored and tested without building a global automatic grouping service.
The existing numerical and decision seams can execute the small case; Adit renders its comparison with the
baseline. Unsupported model refits produce an unresolved stress case rather than a derivative approximation.

The first claim covers only the enumerated single-family withdrawals. It cannot establish robustness to every
pair, common cause or adversarial value replacement. If later pair tests are justified, enumerate a bounded
small set and label its scope. A missing-data stress is also not proof against a biased but retained source;
that is a separate perturbation model. No old sign-off changes and no software classification is generated.

### Why this ranks eleventh

**HYPOTHESIS.** A team may know a parameter is important without knowing which independent audit would most
challenge its conclusion. Source withdrawal translates the evidence spine into that specific review choice.
It can also expose a brittle recommendation that looks stable under small numerical sensitivities because
all the sensitivity inputs come from one underlying campaign.

This is a strong companion to COD-03, but not an excuse to count the same origin machinery as two separate
customer benefits. It ranks here because it needs a refittable model and a real audit choice; COD-03's basic
duplication guard has broader immediate correctness value. Confidence is high in a small enumerated stress
test and conditional in whether it changes where a QP or controller actually spends review effort.

### Tests and kill conditions

**DESIGN — Verification.** Independently calculate the two-family Gaussian posterior with each family absent,
and compare the resulting action choices with exhaustive small cases. Plant a decisive family, a redundant
copy, a necessary calibration dependency and an identification failure. Splitting one family into renamed
files must not make it appear independently robust. Include a case where no individual withdrawal flips the
choice but a pair does, verifying that the single-family report never claims the stronger property.

**TARGET / kill.** On a preselected card, an audit owner must identify a concrete independent check prompted
by the report. Stop if honest refitting is unavailable, if family grouping determines the result arbitrarily,
or if reviewers read sensitivity as a probability of error or fraud. Merge with the ordinary flip-margin
view if it produces no distinct audit choice. Keep the stress scope explicit rather than growing an assurance
framework around an unused diagnostic.

## 12. COD-14 — Show what remains measurable when an instrument is lost

**DESIGN — Subsystems and users.** Touchstone and Crucible, with Plumb/Adit output and later Freeboard evidence
use. A metallurgical accountant or site GM evaluates whether a proposed period-close quantity is still
estimable after a sampler, weightometer or inventory survey becomes unavailable. The question is what can be
reported from the surviving evidence and what extra measurement would restore the missing answer.

### What is new

**FACT.** §20.6 and BD-153 already require identifiability analysis and refusal of non-identifiable bias
attributions. Staleness and observability also appear in the monitoring design. The new workflow is a
measurement-loss analysis of specified close outputs before or during an outage, including combinations that
remain estimable even when individual quantities do not.

**DESIGN.** Compile the active observation equations and conservation constraints into a small observability
test for each requested quantity. Return surviving derived estimates/bounds, lost answers and a separating
measurement. This is narrower than a general “continue operating” recommendation: estimability does not
authorize a close, prove conservation in noisy data, or establish engineering safety.

### The two-minute workflow

**DESIGN.** The accountant selects the current close and marks one instrument unavailable or stale. The
result identifies which requested totals can still be derived, which have wider intervals, and which must be
withheld. It shows the surviving source path and how long the evidence remains valid. The user can narrow
the reported quantity, commission a stocktake, or obtain an alternative measurement, following existing human
approval duties.

**INFERENCE — Synthetic known answer.** For `inflow = outflow + stock change`, measured inflow and stock
change can determine outflow even if its direct instrument is missing. If both outflow and stock change are
unobserved, only their sum is determined by that equation. Returning separate numbers would require extra
evidence or assumptions. The first useful result is identifying the estimable sum, not inventing that split.

### Mechanism and first shippable slice

**DESIGN.** Start with a linear material-flow network containing one latent stockpile and a fixed measurement
map. With observations `H x` and conservation constraints `A x = 0`, a requested linear quantity `qᵀx` is
structurally estimable only if it annihilates the null space of the combined observation/constraint system.
For the tiny first slice, exact rank calculations on integer/rational identities and independently checked
linear solves are sufficient. Numerical conditioning and measurement uncertainty remain separate gates.

The positive path computes the supported linear combination and its propagated covariance under a declared
error model. If actual residuals fail gross-error checks, the close still fails. An informative prior may
support a modelled estimate, but cannot turn structural non-observability into a claim of direct measurement;
the report shows that reliance explicitly and preserves the plan's attribution refusal.

Put this inside the planned Touchstone flow/reconciliation slices, reusing the existing Cholesky kernel for
supported small positive cases. Do not build a second balance engine. Portal supplies active measurement status;
Adit displays the affected close outputs. Once scheduled checks exist, asupersync cancellation leaves the
previous evidence state unchanged and the pending assessment unresolved. No measurement is created by a
retry. A downstream Freeboard view can show loss of observability, never convert the surviving calculations
into a “safe” or “compliant” statement.

### Why this ranks twelfth

**HYPOTHESIS.** A blanket stale badge is insufficient when a site must prepare a monthly pack. Some derived
totals may remain defensible, while a tempting split has become unsupported. Showing that distinction can
reduce both unjustified certainty and unnecessary loss of every output during an outage. The separating
measurement also gives the site a specific request instead of a vague demand to restore all telemetry.

The first mathematics is well bounded and overlaps enough with planned reconciliation to remain small. It
ranks below prospective evidence audit because its value depends on a particular operational close and outage
pattern, and some sites' existing packs may already handle it well. Confidence concerns exact observability
on the declared model, not the validity of every real measurement or every claimed conservation constraint.

### Tests and kill conditions

**DESIGN — Verification.** Exhaustively remove every subset of measurements from a small synthetic network.
Compare reported estimable combinations with an independently authored null-space oracle. For every refused
individual quantity, construct two admissible states with the same surviving observations and different values
of that quantity. Verify positive estimates/covariances against hand examples and simulated noise; inconsistent
measurements must still trigger the existing gross-error path. Test stale status, near-singularity and an
unobserved inventory that makes an apparently simple period ratio invalid.

**TARGET / kill.** The report must never call a non-estimable seeded quantity measured, and a close owner must
find a reporting or measurement-recovery choice it clarifies. Stop if most useful cases need an unvalidated
nonlinear process model, if strong priors are used to conceal the lost information, or if ordinary existing
identifiability diagnostics already serve the workflow. Retain only the small outage comparison in that case.

## 13. COD-17 — Price information against the last date it can be used

**DESIGN — Subsystems and users.** Helm, Corebox and Lodestar, with Portal inputs and Adit delivery. An
exploration lead, project sponsor or GM selects a testwork/drilling campaign before an investment, earn-in or
study commitment. The user chooses whether to buy information, expedite it or decide without it while the
relevant option remains open.

### What is new

**FACT.** §22.4, GB-29 and BD-227 already value information and include cost/delay; §23 has staged recourse.
The current `VoiCampaign` and `rank_voi_campaigns` accept a likelihood and cost, without completion times or a
commitment deadline (`crates/fmine-decide/src/lib.rs:49–97`). The proposed extension makes arrival timing and
expiry part of the information decision, rather than applying a single delay-cost subtraction afterward.

**DESIGN.** Evaluate a finite decision tree in which a result may arrive before or after the last admissible
decision event. Only information available when the action can still change receives value for that commitment.
Keep costs, the option to decide now, and any explicit cost of waiting on the same time basis. Unknown
turnaround or deadlines remain conditional inputs, not guessed calendar facts.

### The two-minute workflow

**DESIGN.** The sponsor opens a commitment card, enters its real decision event, and compares a routine and an
expedited campaign. The result shows net value, the source and calibration status of arrival probabilities,
and the last start dates under the declared schedule. It also identifies a branch where the nominally more
informative test cannot affect this decision because it returns too late. Late information may help a later
decision, but that value needs its own use case instead of being credited here automatically.

**INFERENCE — Synthetic known answer.** Suppose a test has $10M gross information value if on time, costs
$5M regardless, and independently has a 40% chance of arriving before commitment. With no other delay effects,
net value is `0.4 × $10M − $5M = −$1M`, despite a static $5M net VOI. An expedited version with 90% on-time
arrival and $7M cost yields $2M under those same assumptions. These are assumed event-tree inputs, not measured
arrival probabilities or a universal formula for correlated test results and delivery times.

### Mechanism and first shippable slice

**DESIGN.** Ship one two-state investment decision, two signals and a small discrete completion-time table.
Enumerate outcomes and select actions using only the signal/timing observable at each permitted event. Include
the existing “decide now” alternative and a declared fallback at the deadline. Exact finite enumeration avoids
introducing Monte Carlo error in the first slice; money and delay conventions remain explicit boundary types.

The initial toy can declare turnaround independent of state and signal. A real case may violate that
assumption: difficult samples can take longer, and timing itself may convey information. Then the model needs
a joint timing/signal law and an explicit observation policy; do not multiply unconditional on-time probability
by static EVSI. Censored unfinished campaigns stay in the turnaround record. Empirical/elicited timing inputs
retain their evidence class and calibration flags, and arrive as registered forecasts when resolvable.

Extend the current decision/VOI seam and existing calendars. Corebox can later supply actual turnaround
observations. A general lab scheduler, whole-mine POMDP, autonomous procurement system or permit-law engine is
outside the slice. Deadlines and release authority are supplied by the responsible owner, not inferred from a
document by an agent. The system only prepares the comparison; humans authorize spending or schedule changes.

### Why this ranks thirteenth

**HYPOTHESIS.** “Buy more information” can be bad advice when the result misses the choice it was meant to
improve. Pricing that possibility makes existing VOI more operational, especially for staged exploration where
expediting, deferring and walking away are distinct actions. It also turns an abstract value into a clear
question for the campaign owner: can this result arrive in time to change anything?

The finite first case is straightforward to implement and independently check. The adoption confidence is
moderate because some commitments can move and some existing project controls already price delivery well.
It ranks below observability because arrival models and genuine option-expiry events are additional inputs;
a static VOI calculation remains adequate when all feasible completion dates precede the decision.

### Tests and kill conditions

**DESIGN — Verification.** Use a separately authored exhaustive decision-tree oracle, checking the synthetic
negative/positive campaign example. All-on-time arrival with no extra waiting cost must reproduce ordinary
EVSI; arrival after every allowed action change must yield zero information value for this use while preserving
incurred cost. Include a state-dependent completion example that defeats naive probability multiplication,
calendar boundary cases and a missing deadline that forces conditional output. A result arriving after the
decision must never enter an earlier branch's choice.

**TARGET / kill.** A sponsor must identify a campaign whose selection, expedition choice or start date changes
under the explicit deadline model. Stop if relevant deadlines are routinely unconstrained, turnaround cannot
be characterized even conditionally, or the new model always selects the same action as the existing VOI card.
Retain the small timing extension rather than expanding into project-management software.

## 14. COD-09 — State the strongest comparison the metric bases actually permit

**DESIGN — Subsystems and users.** Bullion and Portal with Adit delivery, later Helm consuming only admissible
comparisons. The CFO, IR team or an authorized JV reviewer compares published cost metrics without pretending
that a missing component is known. Agents receive a precisely named comparable quantity, a bounded ordering,
or an explicit statement that the requested comparison cannot be supported.

### What is new

**FACT.** §21.6, BD-060–BD-063 and BD-273 already require versioned metric definitions, named adjustment layers
and refusal of unbridged gaps. E605–E607 and E611 supply plan-indexed examples of distinct metric bases.
The extension searches a small set of approved common views and reports which conclusions are identified
despite incomplete adjustments. It does not weaken the refusal to reproduce a missing published bridge.

**DESIGN.** A comparison is a proposition over a declared common quantity and admissible input set, not a
normalized number generated at any cost. Return exact common components, interval-bounded comparisons or
not-comparable status. Unknown adjustments remain named variables. An ordering is robust only if it holds
throughout the admitted set; overlapping ranges do not imply a tie or justify selecting their midpoints.

### The two-minute workflow

**DESIGN.** The analyst chooses two published metrics and the question, such as which has lower gross cost
before named credits per attributable ounce sold. The result shows the definitions that match, the required
adjustments, available evidence, and the strongest justified conclusion. It may reject the original total-AISC
comparison while still comparing a clearly defined common component. That answer is useful only if the
component serves the owner's actual question.

**INFERENCE — Synthetic known answer.** For a deliberately simple net/gross cost identity, issuer A reports
$1,600/oz after credits bounded at $100–200/oz. Its corresponding gross cost is $1,700–1,800/oz. If B's metric
matches that gross definition and is $1,900/oz, A is lower across the admitted credit range. If B is $1,750/oz,
the evidence does not establish an ordering. Adding credits does not generally convert by-product AISC to
co-product AISC; the example concerns only the explicitly defined gross-before-credits identity.

### Mechanism and first shippable slice

**DESIGN.** Support two definitions and one human-reviewed common view. Treat period, currency, ownership,
metal coverage, sold/produced denominator and inclusion rules as mandatory comparison preconditions. A
conversion requiring undisclosed allocation policy is not silently inserted. Price ratios for GEO and
joint-product allocations remain versioned inputs, not universal normalization constants.

Use exact amount arithmetic and conservative interval propagation for one additive adjustment identity. The
first positive product derives a valid common-component range and a robust comparison when possible. Do not
build a general metric ontology or a peer database. The definition rows are product data consumed by the
comparison function, with the existing schema/golden-test seam. Portal imports pinned public facts; Bullion
performs the supported transformation; Adit renders the proposition and unresolved terms.

Bounds from assumptions are labelled as such. Shared sources and dependencies are preserved, and conservative
enclosures are preferred to false precision. A common-view ordering does not prove management efficiency,
better geology, higher economic value or causation. Public reporting differences can reflect legitimate scope
differences. If the common view is not economically meaningful, the correct result is no useful comparison,
not an impressive-looking league table. Missing terms can become specific evidence requests using COD-06,
while the public lane stays independent of private-source authorship and data.

### Why this ranks fourteenth

**HYPOTHESIS.** Executives routinely want comparisons, but a refusal to compare totals can leave them to make
an unsupported comparison elsewhere. Offering the strongest narrower statement the evidence supports gives
them an alternative that retains the project's honesty. It can also make the discrepancy register more useful:
some apparent performance differences disappear when the actual common components are isolated.

The first arithmetic is small, and the plan already funds metric-definition work. It ranks here because much
of the requirement is an extension of that existing work and real cross-issuer disclosures may leave broad
unidentified sets. I am confident in the bounded example, less confident that a sufficiently useful common
view exists for every requested peer comparison. It must earn its place with one recurring finance question.

### Tests and kill conditions

**DESIGN — Verification.** Independently compute the net/gross example and enumerate allowed adjustment
endpoints. Seed reversed rankings, overlapping ranges, disjoint ranges with incompatible definitions and a
hidden allocation requirement. The last two must not become valid total-metric comparisons. Exact unit
round-trips and operand swaps must preserve the proposition's meaning. Reproduce one public pair with captured
locators and independently reviewed definitions; missing public components remain missing in the fixture.

**TARGET / kill.** Finance must identify a recurring comparison for which the bounded/common-component result
changes interpretation or prevents an unsupported ranking. Stop if all useful outputs require guessed
allocations, if the common view does not serve that question, or if the existing named adjustment bridge already
delivers the whole result. Fold the surviving transformation into `fmine-metrics` rather than creating another
benchmarking platform.

## 15. COD-16 — Measure intervention benefit without confusing prevention with a bad forecast

**DESIGN — Subsystems and users.** Helm, Plumb, Headframe and Crucible, reusing Muster's planned causal-analysis
pattern without person-level data. The GM and CFO evaluate an approved process change or vendor recommendation.
The question is what benefit the intervention caused, separate from how well its initiating prediction was
calibrated. Agents may assemble the evidence and analysis draft; humans approve interventions and conclusions.

### What is new

**FACT.** §16.5 already registers and scores vendor maintenance predictions; §17.8/G-CRUC-5 demands prior
forecasts for vendor benefit claims. §19.6 already proposes preregistered difference-in-differences or staggered
rollout for productivity. A generic before/after chart or a new DiD implementation would therefore be duplicate
work, not this idea.

**DESIGN.** Add an explicit intervention/estimand link across these mechanisms. The forecast states what event
or quantity it predicts under which action policy. A human action record identifies what changed before the
outcome. The benefit assessment uses a predeclared counterfactual design, with implementation cost and a
fixed valuation basis. Prediction quality, adoption, causal physical effect and economic benefit are separate
results; none substitutes for the others.

### The two-minute workflow

**DESIGN.** The GM selects a proposed process adjustment and freezes the baseline policy, eligible comparison
units, outcome, observation window, adoption rule and analysis. After the window, the card shows what was
implemented, what outcomes were observed, whether the comparison remains credible, and the supported incremental
effect and net benefit. It records a null or an inconclusive result as readily as a saving. Economic effects
use the declared price/cost basis and cite FA-certified actual costs where relevant; the assessment never
posts an authoritative balance.

**INFERENCE — Synthetic known answer.** A treated process rises from 100 to 130 units of output while a valid
comparison process rises from 100 to 120 over the same interval. Under the declared parallel-trend design, the
incremental estimate is 10 units, not the treated process's 30-unit before/after gain. That identification
assumption remains explicit; observing those four numbers alone cannot establish it.

### Mechanism and first shippable slice

**DESIGN.** Start with one synthetic non-safety process intervention, a finite registered comparison design,
aggregate period outcomes and a separately supplied cost record. Execute the small positive causal estimator
and its uncertainty calculation against sealed synthetic truth. Include an adoption trace so a proposal that
was never implemented is not credited with a measured saving. Reuse planned forecast, decision and causal
analysis seams; no enterprise experiment-management service or automated equipment-control path is needed.

The distinctive negative case is a prediction of failure under no intervention followed by a preventive action.
The realized no-failure event is not automatically a false positive, but neither is it proof that the action
prevented failure. The no-action outcome is unobserved. Mark the forecast's resolution as intervention-affected
unless its preregistered estimand and resolution contract permit a valid score. Keep it in COD-08's frozen
cohort, never drop it to improve vendor results. A prediction explicitly conditional on the implemented policy
can be scored against that policy's outcome; a different target requires a new forecast.

For real benefit estimation, use an owner-approved credible comparison and preserve all causal assumptions.
Observable confounding, contamination, unequal adoption or missing outcome records can invalidate the chosen
design. No method here claims to detect every unobserved confounder. Where identification fails, report
descriptive outcomes or explicit bounds and withhold the causal saving. The product never proposes withholding
a required safety action to obtain a control group; its first scope is already permitted process alternatives
and synthetic data, with no personal records in the repository.

### Why this ranks fifteenth

**HYPOTHESIS.** A CFO needs to distinguish a vendor that predicts well from one whose adopted advice creates
value. A successful intervention can also destroy the naive test of the prediction that triggered it. Making
those distinctions explicit prevents both unfairly penalizing prevention and crediting every avoided event as
a proven saving. It gives FrankenMining a fair assessment role alongside external operating platforms.

This could become compelling, but it ranks last among these ten because credible causal comparisons, adoption
records and sponsor cooperation are harder than the code. Much of the mathematical tooling is already planned;
the added value is linking estimand, action and outcome honestly. I am confident in the bookkeeping and
synthetic estimator checks, not in a blanket ability to identify real-site savings from observational history.

### Tests and kill conditions

**DESIGN — Verification.** Check the hand-derived 10-unit example and sealed worlds with known zero/positive
effects. Include an observable trend confounder, non-adoption, contaminated comparison units and an action
that changes the forecast target. A no-action risk followed by prevention must never be automatically scored
as either a false alarm or a proven avoided loss. Missing preregistration must prevent measured-benefit status.
Verify exact cost inclusion and that changes to the valuation basis alter economic benefit without rewriting
the physical effect estimate. As-of replay must exclude post-outcome choice of controls and analysis rules.

**TARGET / kill.** Before a real-site extension, a sponsor must accept an admissible comparison design and
agree to record null/inconclusive outcomes. Stop if attribution depends on an unverifiable counterfactual,
if implementation records are absent, or if the existing registered causal workflow already covers the
question. Retain honest action-conditioned forecast resolution and descriptive results without selling them
as causal ROI. That narrower correctness repair remains useful even when the benefit product is not viable.

## Original screening of the remaining 25 candidates

**DESIGN / HYPOTHESIS.** The table preserves the first-pass evaluation. The expanded ranks #6–#15 above take
precedence over their earlier brief dispositions. Each row gives the additional mechanism,
its user workflow, a bounded positive first slice, an independent verification route and a kill condition.
“Defer” is a ranking decision, not a claim of demonstrated customer demand. Public reproduction means pinned
public-filing facts with locators; synthetic truth never establishes real-site value. Section references name
overlap that a later proposal would have to preserve and materially extend.

| ID and addition beyond the current plan | Subsystems; user and workflow | First slice and test | Disposition and what kills it |
|---|---|---|---|
| COD-06: Answerability before data sharing — alternative minimal input sets for one named question, beyond §34.4's access gates | Portal, Touchstone, Adit; sponsor and GM narrow a data request to the answer they want | Two explicit recipes for a recovery question; enumerate missing-column subsets on synthetic data and verify every claimed answerability result | Strong runner-up. Defer until a live data negotiation; kill if it becomes a checklist rather than computing a usable answer from a smaller data offer |
| COD-07: Bilateral JV disagreement protocol — locate the first differing basis or input between two authorized evaluations, beyond bundles/walls in §31 | Bullion, Plumb, Bedrock; partners resolve a disputed roll-up | Compare two synthetic partner statements with one ownership/rounding difference; reproduce the exact divergence | Defer pending partner use. Kill if it requires cross-wall disclosure, invents zero-knowledge guarantees, or merely duplicates a shared spreadsheet |
| COD-08: Forecast survival scoreboard — show original and restated outcomes plus unresolved/dropout cohorts, beyond scoring and declared non-resolution in §10 | Plumb, Portal; CFO judges track record without survivorship selection | One frozen forecast cohort, two public outcome vintages and a missing outcome; verify denominators and score bounds independently | Fold into forecast delivery rather than a separate product. Kill any ranking that excludes unresolved forecasts or invents a point score for an unknown outcome |
| COD-09: Comparability frontier — state which cross-issuer conclusions remain identifiable after basis differences, beyond metric variants in §21.6 | Bullion, Portal; CFO and IR compare peers honestly | Two public issuer definitions; derive the comparable subset or a bound from named adjustments | Defer. Kill if the missing components make every result vacuous or if producing a common headline requires unsupported allocations |
| COD-10: Controlled IR assertions — bind a numeric sentence and its qualifications to Claims, beyond rendering tables in §30.5 | Portal, Hallmark, Adit; IR checks a draft release locally | A small template vocabulary for period change and range statements; synthetic unit/basis/denominator mutations must block incorrect sentences | Useful later, not a general writing assistant. Kill if qualification checking depends on an LLM's unsupported semantic judgment or implies filing approval |
| COD-11: QP review by evidence delta — identify exactly what needs fresh examination since a prior support pack, beyond §24.5's evidence index | Hallmark, Corebox, Strata; QP scopes a repeat review | Two synthetic packs with one withdrawn assay batch and one unchanged domain; independent full-pack comparison | Treat as a specialized COD-04 consumer. Kill if it carries sign-offs forward or claims to replace the QP's determination of review scope |
| COD-12: Reserve economic fragility frontier — quantify the movement in a declared estimator under price/cost changes, beyond a sensitivity ladder in §24.8 | Hallmark, Helm, Pushback; QP and board distinguish deck-supported additions from robust inventory | One synthetic grade-tonnage curve with explicit cutoff economics; exact small enumeration verifies exposed metal | Defer until estimator parity. Kill if it becomes a software reserve classification, treats sensitivity as mineable reserves, or duplicates the existing ladder |
| COD-13: Assay-delay selection sensitivity — test whether missing/late results could change a decision, beyond the lookahead guard in §6.3 | Corebox, Plumb, Lodestar; exploration lead reviews a campaign before all certificates arrive | Synthetic grade-dependent turnaround delays; compute explicit missing-outcome bounds and verify coverage of seeded completions | Defer. Kill any automatic missing-not-at-random correction unsupported by a sampling model; retain honest bounds if point identification fails |
| COD-14: Measurement-outage closure test — compute what remains observable after losing a sensor, beyond current staleness and §20.6 attribution | Touchstone, Crucible, Freeboard; metallurgist chooses whether a proposed close has enough evidence | Remove each measurement from a small synthetic flowsheet; compare rank and estimable combinations with an independent linear oracle | Defer to M1. Kill if reduced observability is disguised by stronger priors or the result is presented as engineering safety assurance |
| COD-15: Calibration at the actual decision threshold — score the event management acts on alongside global scores, beyond §10's family averages | Plumb, Helm; CFO evaluates downside performance near a guidance or cash threshold | Predeclare one threshold, derive event forecasts from registered distributions and check Brier scores against hand calculations | Fold into existing calibration. Kill if thresholds are selected after outcomes or the display implies local calibration from too few resolutions |
| COD-16: Intervention benefit contracts — distinguish forecast accuracy from causal benefit of adopting a recommendation, extending §19.6 beyond workforce | Helm, Headframe, Crucible; GM evaluates an approved operational experiment | One synthetic staggered intervention with a confounder; independent estimator/control cases must recover the known effect or refuse attribution | Defer: site data and experimental design dominate. Kill if counterfactual assumptions cannot be defended; a before/after improvement is insufficient |
| COD-17: Last useful date for information — decide whether a test result can arrive before the option expires, beyond static delay cost in §22.4 | Helm, Corebox, Lodestar; exploration lead chooses a study campaign | Two campaigns with discrete completion dates and an expiring stage; exact event-tree enumeration verifies value and late-result handling | Defer as a VOI extension. Kill if the commitment deadline or turnaround model is unknown, or if static VOI already chooses identically |
| COD-18: Settlement challenge preview — isolate alternative contract interpretations before drafting a counterstatement, beyond the settlement engine in §21.5 | Crucible, Bullion; controller checks a smelter/refinery statement | One synthetic invoice and two approved interpretations of a deduction; exact money oracle and missing-term tests | Defer. Kill if it requires automatic legal interpretation or adds no capability beyond ordinary settlement arithmetic; never send a dispute automatically |
| COD-19: Shared dependency stress — aggregate losses from one common power/logistics dependency, extending the price/country scenario links in §23 | Helm, Headframe, Crucible; executive committee reviews concentration | Two synthetic sites sharing one dated outage input; compare joint output with naive independent outages | Defer until multi-site data exist. Kill if dependency ownership cannot be established or the feature becomes a competing procurement/maintenance platform |
| COD-20: Vendor objective reconciliation — explain why two optimizer objectives differ before comparing gaps, beyond plan feasibility in §15.4 | Pushback, Bullion, Portal; planner compares vendor candidates | Two small synthetic schedules, identical physical constraints and different declared price/cost bases; exact objective reconciliation | Defer; much overlaps three-gap certificates. Kill if objective bases are inaccessible or the ordinary certificate already answers the question |
| COD-21: Search answer completeness — attach the searched universe to “not found,” beyond citation-only agent answers in §30 | Plumb, Portal, Adit; QP or agent checks whether a prescribed public document set contains a required item | One finite public document manifest and deterministic item query; omitted documents must change completeness status | Defer. Kill if it implies absence outside the declared universe, exposes hidden document existence, or needs unrestricted semantic search to assert completeness |
| COD-22: Auditor-chosen spot checks — sample claims after the producer commits the population, beyond offline replay in §33.6 | Testpit, Portal, Hallmark; auditor chooses checks the producer cannot pre-select | Public/synthetic cell population committed by digest, then an auditor seed selects samples; independent sampling and tamper tests | Defer until an auditor wants sampling. Kill if a sample pass is presented as full-population correctness or if it duplicates the auditor's existing selection tool |
| COD-23: Decision stability across approved policy interpretations — separate policy uncertainty from market uncertainty, extending §22 robustness | Helm, Bullion, Hallmark; CFO reviews a choice under two admissible treatments | Two synthetic policy branches and a fixed scenario set; brute-force comparison verifies stable/changed choices | Defer. Kill if software chooses the accounting/legal interpretation or the supposedly admissible branches lack a qualified human owner |
| COD-24: Internal transfer neutrality — show when site metrics improve only by moving value or cost to another unit, extending toll arcs in §15.6 | Bullion, Helm, Touchstone; group CFO and JV reviewer inspect an inter-complex proposal | Two synthetic sites and one transfer; group cash/metal conservation plus site KPI changes checked exactly | Defer; valuable after consolidation. Kill if inter-entity rights make neutrality inapplicable or if this merely restates an existing consolidation elimination |
| COD-25: Stop simulation when the decision is resolved — bound ranking error as a shared scenario prefix grows, beyond a fixed ensemble in §29 | Helm, Strata, Testpit; planner or agent buys only the precision needed | Two bounded synthetic alternatives on common streams; compare a declared sequential rule with exhaustive outcomes and null controls | Defer for performance need. Kill if optional stopping invalidates the error bound, tails are unbounded without a model, or simulation is not the bottleneck |
| COD-26: Evidence removal fragility — find which entire observation family can overturn a decision, extending parameter flips in §22 | Plumb, Helm, Hallmark; QP commissions a targeted independent review | Two synthetic origin families and two actions; exhaustive family removal checks the ranked vulnerabilities | Strong companion to COD-03, not a separate first product. Kill if removal is confused with a probability of fraud/error or the baseline fit cannot be rerun honestly |
| COD-27: Exploration effort overlap accounting — prevent rebranded or spatially duplicated campaigns from manufacturing search coverage, beyond §11 effort inputs | Lodestar, Corebox, Portal; exploration team evaluates acquired historical data | Synthetic overlapping survey footprints and duplicated hole identities; union/coverage oracle and CRS metamorphic checks | Defer pending tract use. Kill if identity is inferred from proximity alone or raw metres are converted into detection probability without a valid model |
| COD-28: Metallurgical sample-selection bounds — carry uncertainty from untested ore into choices, beyond coverage bands in BD-300 | Crucible, Strata, Helm; metallurgist prioritizes testwork before conversion/expansion | Synthetic ore domains with grade-dependent sample selection; bound recovery over the uncovered mass and compare with sealed truth | Strong later extension. Kill if defensible bounds are useless or the existing coverage/OutsideValidity refusal already supplies all decision value |
| COD-29: Fixed-cohort operating bridge — separate genuine site change from exits, ownership and reporting-perimeter change, beyond bitemporal ownership in §24.3 | Bullion, Hallmark, Headframe; CFO compares periods after a divestment or commissioning | Two public/synthetic periods with a declared common cohort; exact entry/exit and ownership decomposition | Fold into comparison workflows. Kill if it hides exited operations, silently restates history or adds nothing beyond named existing aggregation layers |
| COD-30: KPI robustness tests — construct allowed accounting/timing transformations that change the metric but not the chosen economic objective, extending §23.6 Goodhart alarms | Bullion, Helm; remuneration committee reviews scorecard design | Three synthetic transformations for timing, classification and transfers; exact invariant and KPI-change checks | Defer; keep as explicit stress cases. Kill if it recommends manipulating disclosures, assumes contested classifications are permitted or repeats an already visible shadow-price finding |

## Appendix — One-line titles of all 30 candidates

1. **COD-01:** Guidance completion test: what must happen to meet the target?
2. **COD-02:** Rounding-aware contradiction finder for published disclosures.
3. **COD-03:** Independent-evidence accounting before uncertainty reduction.
4. **COD-04:** Decision recall and bounded reassessment after evidence changes.
5. **COD-05:** Metal-to-available-cash bridge and reverse liquidity stress.
6. **COD-06:** Answerability-first data offers for a single owner question.
7. **COD-07:** Bilateral JV disagreement localization without new data sharing.
8. **COD-08:** Forecast survival scoreboard across revisions and missing outcomes.
9. **COD-09:** Cross-issuer metric comparability frontier.
10. **COD-10:** Controlled IR sentences bound to numeric Claims and qualifications.
11. **COD-11:** QP re-review scoped by evidence changes.
12. **COD-12:** Reserve economic fragility under declared price and cost changes.
13. **COD-13:** Sensitivity to informative assay delays and missing results.
14. **COD-14:** Measurement-outage observability and period-close consequences.
15. **COD-15:** Calibration at the threshold that drives the decision.
16. **COD-16:** Causal benefit contracts for adopted interventions.
17. **COD-17:** Last useful date for information before an option expires.
18. **COD-18:** Settlement challenge preview under explicit contract interpretations.
19. **COD-19:** Shared infrastructure and logistics dependency stress.
20. **COD-20:** Vendor optimizer objective reconciliation before gap comparison.
21. **COD-21:** Search completeness statements over declared document universes.
22. **COD-22:** Auditor-selected spot checks after population commitment.
23. **COD-23:** Decision stability across human-approved policy interpretations.
24. **COD-24:** Internal transfer neutrality and site-metric redistribution.
25. **COD-25:** Sequential simulation precision tied to decision stability.
26. **COD-26:** Decision fragility to removal of an observation family.
27. **COD-27:** Overlap-aware accounting for historical exploration effort.
28. **COD-28:** Metallurgical sample-selection bounds for untested ore.
29. **COD-29:** Fixed-cohort operating comparisons across perimeter changes.
30. **COD-30:** KPI robustness tests under explicit economic invariants.
