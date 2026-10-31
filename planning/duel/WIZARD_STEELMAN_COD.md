# Codex steelman of the two opposing #1 ideas

**Date:** 2026-10-29. **Sources:** CC's [Public Track Record](WIZARD_IDEAS_CC.md), AGY's
[GuidanceCredibilityBridge](WIZARD_IDEAS_AGY.md), the technical constitution and the
[round-7 audit](../mor/r7/MODES_OF_REASONING_REPORT_AND_ANALYSIS_OF_PROJECT.md).
All implementation paths below are **DESIGN**; adoption and second-order benefits are **HYPOTHESIS**.
Worked values explicitly identified as synthetic are not issuer findings.

**FACT — Starting state matters.** The audit's Bottom Line and C001–C006/C058–C059 describe the committed
M0 command substituting generated workbook inputs and emitting wrong issuer-attributed figures. Existing
code contains useful parsing, exact-quantity and scoring seams, not an admitted end-to-end assurance product.
The current working-tree plan incorporates stricter source registration and three separate GATE-BULL-01
components in §21.6; that design correction is not evidence that the code now implements them.

**INFERENCE — Revised judgment.** Steelmanning raises my assessment of both product ideas. I had given too
little weight to the decision value available before either a reliable miss probability or a complete
physical attribution exists. The revised scores below apply to the disciplined implementation described here;
they do not validate the original proposals' unsupported statistical or mathematical guarantees, and the
earlier score files remain an unchanged record of that review.

| Idea | Previous score | Revised score | Reason for the change |
|---|---:|---:|---|
| CC Idea 1: Public Track Record | 730 | 790 | An auditable history of commitments has value independent of proving forecasting skill |
| AGY Idea 1: GuidanceCredibilityBridge | 430 | 610 | A common-basis guidance review can be useful with partial identification and no universal coherence theorem |

## CC Idea 1 — The Public Track Record

### The insight I initially undervalued

**INFERENCE.** The scarce asset is not another model predicting the mine. It is a stable account of what
the organization committed to, what changed before resolution and which eventual observation answers that
commitment. A public statement supplies an externally observable commitment even when the issuer will share
no operating database. That gives FrankenMining a useful evidence object before it has earned the right to
estimate the mine's dynamics.

The strongest product is therefore a longitudinal commitment record, not a site league table. Initial
guidance, a revision, an outcome restatement and a changed reporting perimeter must remain different events.
A CFO or IR reviewer should be able to answer: “Did the result meet the original target, the last target, both
or neither, and what exactly changed between them?” A regulator or JV reviewer can examine that same public
record without accepting any FrankenMining model of performance.

This is particularly well suited to the project's wedge. Reproducing an actual is recognizable but can elicit
“we already know that.” Reproducing the full sequence of statements tests organizational memory and makes a
specific future review easier. A clean history is a useful result too: the product need not find a
discrepancy to save the work of assembling an accurate answer (GB-38, §37.1–§37.3).

### The strongest implementation path

**DESIGN — A small executable contract.** Start with one issuer, one site, one annual production target and
its revisions, not every cost metric across a dozen sites. Each imported commitment needs the target period,
entity/perimeter, ownership basis, quantity/unit, original range or point, qualification text represented by
reviewed structured conditions, publication evidence and exact source locator/hash. Each outcome needs its
publication vintage and matching target semantics. A missing target definition produces unresolved status,
not a convenient match to the nearest available actual.

Do not put an issuer's planning range into today's Gaussian `FrozenForecast` by inventing a mean/std-dev
interpretation. The first resolver can operate on exact typed endpoints and a target-compatible actual,
returning within/below/above/unresolved plus the exact distance to the relevant boundary. Generic resolution
math belongs in the existing calibration/quantity seams; issuer-specific source selection remains in Portal,
and Adit renders Claims with the full no-claim envelope. No K-layer dependency on an E-layer metric engine
is needed.

**DESIGN — First shippable slice without assuming M0 works.**

1. Independently capture the chosen public guidance and outcome records with two readers, source hashes and
   page/cell locators; E586/E587 and E451/E474 identify relevant source families, not an automatic answer key.
   These are reviewed data records, not issuer values embedded as success constants in Rust.
2. Use a narrowly fail-closed path that verifies the supplied original source files against those hashes.
   If the existing workbook parser is used, select the actual labels/cells and compare with an independent
   captured read; remove the fallback from this path before it can produce an issuer result. Missing files,
   missing cells and basis mismatches refuse. The report states whether a value came from a reviewed
   transcription or an admitted extractor.
3. Produce one complete site-year history from those records: every eligible commitment, the first eligible
   outcome, any later outcome vintage and the predeclared resolution result for each comparison. Do not
   present the narrow case as a pass for all M0 workbooks.
4. Freeze the selector and resolution rules for the next genuinely prospective cohort, with the information
   cut and external time proof required by §21.6/BD-068. Expand history only after the first case is reproduced.

Archival publication time and this system's current ingest/record time remain separate. A documented
pre-outcome publication supports a reconstructed historical commitment; it is not permission to backdate
the ledger or claim that FrankenMining prospectively registered its scoring method years ago (§6.3, §10.1).
Unproved historical versions remain unresolved rather than becoming “immutably preregistered” by assertion.

**DESIGN — Stronger scoring discipline.** Initially report counts and signed misses descriptively, retaining
the entire cohort and all non-resolution reasons as in COD-08. A range without a stated probability is not
automatically an 80% interval. A fixed diagnostic interval loss may be declared separately, but cannot imply
that the issuer elicited those quantiles. Site-year observations sharing a year shock are not magically
independent; hierarchy, conformal correction and races wait for their assumptions and sample size. The
current plan itself says the early public cohort demonstrates the protocol rather than positive calibration
skill (§21.6, round-7 C118/C119).

The first CLI may remain synchronous. When admitted runtime components orchestrate it, asupersync can bound
source parsing and cohort resolution in cancellable regions, with IO confined to adapters and publication
only after a complete result exists. A cancellation must leave no partially resolved cohort masquerading as
a final track record; existing authority/CAS formats should carry the result rather than a second database.

**DESIGN — Tests.** Independently reproduce the selected public source values; use exact synthetic cases for
range ties, point guidance, changed ownership, revisions after period end, late outcomes and restated actuals.
Removing an unfavorable outcome must not improve a purported whole-cohort result. Altering a source or
selector must change the relevant identity/result, while an as-of query before a restatement must remain
unchanged. These establish source parity, bookkeeping and known answers separately, not historical skill.

### Second-order benefits beyond the original pitch

**HYPOTHESIS.** The commitment record can reveal whether the organization is changing the target, its
definition or its estimate of the same target. That is a useful governance distinction before anyone
attributes a miss to price, geology or execution. It also makes later model evaluation less ambiguous:
the target/outcome contract is already explicit when FrankenMining adds its own forecasts.

It can reduce repeated IR and audit work after a restatement because the answer to “which statement did
this actual resolve?” is stored as a reproducible relation, rather than reconstructed separately in each
board paper. It can also expose missing public information as a precise question—an unavailable vintage,
unclear perimeter or unspecified metric basis—without pretending that more statistical sophistication would
repair the missing definition.

### Pre-emptive defense against the two strongest objections

**Objection 1: “Guidance is a business target, not a probability forecast.”** That is a reason to model its
meaning explicitly. The first product measures attainment of a published commitment and describes its
revisions; it does not call the range miscalibrated without a warranted probabilistic interpretation.
This keeps the record useful while avoiding an unwarranted statistical judgement.

**Objection 2: “Three years are too few, and finance already has a spreadsheet.”** Three years may be far too
few for persistent-site rankings or finite distribution-free calibration guarantees. They can still support
a fully cited record of those three years. The adoption test is whether a reviewer can answer a difficult
vintage/basis question faster and with fewer errors than the current worksheet, not whether the system can
manufacture a significant result. If the worksheet already does that reliably, this surface should not grow.

### Residual concerns and final position

**INFERENCE.** Historical completeness, reliable publication evidence, perimeter changes and correlated
outcomes remain real costs. A polished record can still be misread as evidence that a small observed hit
fraction is the issuer's true coverage probability. Maintaining archived versions also needs a source-rights
decision consistent with the public-lane rules; originals stay outside the repository.

**DESIGN.** Ship the commitment history before the calibration story, and require BD-075's reviewers to name
a decision or recurring review task it improves. My revised score is **790/1000**: the product has stronger
early value than I initially credited, while the original hierarchical rankings, “controllable” attribution
and automatic recalibration still need independent justification.

## AGY Idea 1 — GuidanceCredibilityBridge

### The insight that survives the erroneous formulas

**INFERENCE.** A variance review compares a result with a benchmark, but the benchmark itself contains a
price deck, ownership perimeter, denominator and rules. If those move, calling the difference “operating
execution” can give management credit or blame for something the comparison has not held constant.
The valuable idea is an executable, explicitly conditioned benchmark whose assumptions survive from release
preparation into the post-release review.

This could make one workflow serve CFO, IR and the site GM. They can all see the original public result,
the result under an expressly chosen comparison basis and the portion that still lacks an explanation.
It does not require agreement that every remaining dollar was controllable. That shared calculation is
more useful than separate departmental narratives built from subtly different bases.

The pre-release side has a second strength: a declared aggregate identity can surface an unstated corporate/JV
layer, inconsistent denominator or incompatible draft inputs before publication. That is worth doing even
when no probability law for hitting guidance is available. The product can therefore earn a place as a
release-review calculation before it matures into a probabilistic guidance model.

### A more specific, sound implementation

**DESIGN — Separate three objects.** Keep the published target and actual immutable; construct a distinct
conditional benchmark; attach an attribution only for the transformations actually supported. Hard scenario
envelopes, aspirational planning ranges and probabilistic quantile intervals must be distinct types/records.
Marginal quantile endpoints are not hard joint bounds, and no certificate should infer universal coherence
from the sum of those endpoints.

For extensive quantities, test the declared ownership/perimeter identity. For AISC, bind annual cost amounts
and sold ounces first, then compute the ratio; do not average quarterly ratios or apply ownership twice to
already-attributable figures. Corporate/JV layers need explicit amount-versus-per-ounce units and named
evidence, rather than becoming a residual that makes the result pass (§21.6, G-BULL-3).

**DESIGN — First shippable slice.** Start with one public annual guidance/actual pair on a verified common
basis and one expressly disclosed price sensitivity, if a source capture confirms its meaning and validity
range. Reuse the same narrow fail-closed acquisition and independent transcription/extraction checks described
above; today's substituted workbook path is not an input. A week-one source gate must confirm the sensitivity
and the target/outcome denominator. If no suitable public sensitivity exists, ship the fully labeled synthetic
calculation and source-gap result, not a fabricated Barrick bridge or a claim of public demonstration.

Implement one pure exact transformation: price movement multiplied by the registered sensitivity, with
commodity, currency, period and normalization units checked. The result includes the original range, shifted
range, original miss and residual against the shifted benchmark. A slope valid only locally cannot be
extrapolated through a fiscal step without an additional term model. Sources and Claims retain their
evidence class; a disclosed management sensitivity is not thereby a measured causal response.

**INFERENCE — Rechecking AGY's explicitly illustrative arithmetic.** Its $1,400/oz midpoint, $800/oz price
move and $5.50/oz cost increase per $100/oz price move produce a $44/oz benchmark shift. An actual of
$1,472/oz is $72/oz above the original midpoint and $28/oz above the shifted midpoint. Those two bars close
exactly, while the original $1,350–1,450/oz range shifts to $1,394–1,494/oz. This supports “inside this
price-conditioned range under the stated sensitivity,” not “only price caused the miss.”

**DESIGN — Earn additional bars individually.** A public price transform does not reveal guided throughput,
grade, recovery, inventory or spending baselines. Retain the unsupported remainder as unidentified. A later
authorized plan snapshot can support LMDI or another declared allocation over those inputs, with interactions,
zero boundaries and valuation order handled explicitly. Numerical closure is a check on the identity, not
proof that six operational causes have been identified. No unexplained remainder may be renamed unit-spend
efficiency to complete the picture.

For the first aggregate envelope test, use a tiny declared hard scenario set and verify every candidate
against the amount/denominator identities. If a continuous two-site box is supported, exact endpoint/vertex
enumeration with a strictly positive denominator provides an independent small oracle; its result is
conditional on that envelope. A probabilistic extension instead requires a registered joint model and its
calibration status. Report site coverage, expected miss count, group coverage and all-sites-hit probability
as different quantities; a small all-sites-hit probability does not prove group guidance is improbable.

Portal imports facts and reviewed transformations; Bullion evaluates the bridge; Helm consumes its supported
decision implications; Adit renders the two bases side by side. Use the existing exact `Money`/quantity types
and canonical Claim identities, not the original bare-`f64` public API sketch. Pure kernels need no IO or time;
admitted asupersync adapters provide bounded execution and atomic publication, rather than a new runtime or
an implicit write path to plant controls.

**DESIGN — Tests.** Use independently calculated synthetic price-shift cases, zero shift, unit rescaling,
incompatible sold/produced bases, step boundaries and missing sensitivities. Mutate an actual source value
and require the corresponding source identity and dependent result to change. Independently reproduce the
chosen public pair; compare exact small aggregate-envelope cases with exhaustive enumeration. Include a
valid joint distribution whose aggregate quantiles exceed summed marginal quantiles so the old coherence
inequality cannot return as a refusal rule. Tests cannot turn unobserved operational baselines into known
inputs.

### Second-order benefits beyond the original pitch

**HYPOTHESIS.** A reusable conditional benchmark can identify double counting in review narratives: a price
effect cannot first be removed mechanically and then credited again inside the spending explanation. It can
also make the next guidance cycle more explicit about which sensitivity is disclosed, which assumptions are
fixed and what cannot be separated from public data.

Keeping the unconditional and conditioned views together gives the site and finance a common
basis. Investors' experienced result remains visible, while an operational review can use a
declared comparison basis without rewriting history. Eventually the same bridge can make a larger fiscal
scenario model easier to inspect, because the marginal adjustment and its validity domain already exist as
a small independently tested calculation rather than a hidden model coefficient.

### Pre-emptive defense against the two strongest objections

**Objection 1: “This will normalize every miss away.”** The original public outcome
and original target stay first, and every adjustment is separately sourced and named. The residual remains
unidentified when the evidence does not support attribution. An adjustment can make a miss larger as readily
as smaller, and a reviewer can reproduce both views without relying on any single narrative.

**Objection 2: “The required data and joint probabilities are missing.”** Then a smaller bridge is the honest
product. A verified price sensitivity and one matched actual can answer a bounded comparison question;
full physical attribution and joint success probabilities are later optional capabilities. This is valuable
only if finance says that smaller result changes a recurring review; it cannot be marketed as a six-cause
assurance certificate in advance of that evidence.

### Residual concerns and final position

**INFERENCE.** Public sensitivity disclosures may be too coarse or absent, and their implicit volume/FX
assumptions may prevent a useful exact bridge. Adjustments are policy-dependent and can still be presented
selectively. The original quantile-coherence inequality, corporate variance formula and forced six-factor
identification remain unsound; the steelman repairs their role rather than defending them.

**DESIGN.** Keep the common-basis release/review workflow and gate each additional inference. My revised
score is **610/1000**, up from 430 because the partial, explicitly conditioned calculation has more independent
user value than I credited. It remains below the Public Track Record: its meaningful public source gate is
tighter and its output is easier to mistake for causal attribution. If it cannot secure one independently
reproducible public case or a later authorized plan baseline, stop expansion rather than fill the missing bars.
