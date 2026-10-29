# Three blind spots after the three-model duel

**Date:** 2026-10-29. **Status:** DESIGN and HYPOTHESIS, not implemented capabilities.
I checked the three written candidate inventories—[COD](WIZARD_IDEAS_COD.md),
[CC](WIZARD_IDEAS_CC.md) and [AGY](WIZARD_IDEAS_AGY.md), plus
[AGY's expansion](WIZARD_IDEAS_AGY_6_15.md)—against relevant plan sections.
These workflows were absent from those inventories as explicit end-to-end proposals; their shared
foundations are already planned, and the nearest overlaps are identified below.

**INFERENCE.** The common blind spot was the boundary between an analysis and somebody relying on it.
We proposed better calculations, sources, forecasts and model diagnostics, but gave less attention to an
issued answer that later becomes unusable, a committee applying a nonlinear reward rule to uncertain inputs,
or a loss whose financial recovery depends on a different evidence contract. The round-7 audit makes the
first gap concrete: it found synthetic stand-ins and wrong figures presented as issuer results, not merely
an overly wide uncertainty interval. It does not establish that those outputs were actually distributed to
any external recipient.

All examples below use public or explicitly synthetic facts. No personal data, site exports or MNPI enter
this repository. Outputs remain conditional evidence, with INV-002/INV-011 intact and FA as the book of
record. Scores are subjective overall self-assessments on 0–1000, not calibrated probabilities of adoption.
No proposal assumes the current M0 importer works or constitutes authorization to implement during this duel.

| Rank | ID | New workflow | Main underserved consumer | Self-score |
|---:|---|---|---|---:|
| 1 | BS-COD-01 | Withdraw an issued answer and check its status before reusing a copy | IR, audit committee, JV recipient, lender and their agents | 810 |
| 2 | BS-COD-02 | Propagate metric-definition uncertainty through a compensation formula | Remuneration committee and corporate finance | 740 |
| 3 | BS-COD-03 | Separate an interruption's economic loss from conditional insurance recovery | Insurance/risk team, CFO, insurer and lender reviewer | 620 |

## BS-COD-01 — Issued-report withdrawal and recipient-side reuse checks

**DESIGN — Subsystems, user and workflow.** Adit and Plumb, with Bedrock anchors and Testpit verification;
Portal handles public source references. An IR analyst, board-pack author, authorized JV recipient or lender
agent holds a copied report. Before inserting one of its numbers into a fresh decision or document, the
consumer checks whether that exact issued artifact is still eligible for current reliance under the selected
freshness policy. When a producer discovers a material defect, the producer prepares a withdrawal tied to
the artifact digest and affected statements, leaving the original available as historical evidence.

### What is new

**FACT.** INV-013 and COD-04 mark dependent Claims stale and identify reviewed decisions needing attention.
CC-14 explains differences between evidence cuts. §30.5 binds reports to anchors, while §33.6 verifies
certificate chains, MMR/CAS roots and regeneration. None of those specified checks tells the holder of a
detached PDF or cached agent answer that the producer has since withdrawn the issued artifact itself.

**DESIGN.** Add issuance and reliance status as a consumer-visible relation: artifact digest, relevant output
Claims, issuing anchor, withdrawal/supersession event and the last status cut the consumer checked. This
extends beyond recomputing an internal decision: it reaches copies outside the live dependency graph and
covers implementation defects that leave the source file unchanged. A certificate can remain historically
authentic while its numerical conclusion has been withdrawn; the verifier must be able to report both.

### First shippable slice and mechanism

**DESIGN.** Begin with a synthetic public-lane issuer, one deterministic report, a copy held by two generic
organizational roles and a later producer-discovered calculation defect. Use a small immutable issuance/status
event set and the existing digest/anchor seams, not a new messaging platform or separate ledger. A local
consumer check reads the trusted status cut and returns: current at that cut, withdrawn, superseded, or
current status unknown. A withdrawal names affected statements and a corrective artifact only when one
exists; it cannot silently replace the original bytes or carry an old sign-off forward.

The numerical report/export path must consume this status. An agent requesting current reuse of a withdrawn
result receives a structured refusal and its known replacement/status request, rather than the old value
with a warning hidden elsewhere. A historical inspection can still open the original with its historical
status. This is product state because it changes what a production consumer may present as current evidence.

When admitted authority/bundle components exist, distribute status receipts through those existing trust
mechanisms. A receipt names its authority, sequence/as-of cut and policy validity interval; it proves only
what that authority asserted at that cut. A signature alone does not prove globally latest status, and a
network-isolated reader cannot learn about an unseen withdrawal. Expired status, unavailable trusted time
or an unknown authority therefore cannot become “current.” Configured freshness bounds are owner policy,
not a mathematical promise of instantaneous recall.

Asupersync can later schedule bounded status retrieval in adapters, keeping IO and time out of K/E kernels.
No automatic emails or partner messages are needed for the first slice. Any later distribution remains
human-authorized through existing channels. Fixtures use generic role IDs, with no personal contact details;
an acknowledgment means the role's endpoint received a notice, not that a human understood or approved it.

### Test

**DESIGN — Known answers and protocol verification.** The old report's digest remains valid after withdrawal,
but a fresh current-use check must refuse it. Verify offline/unknown-time behavior, expired receipts, stale
status replay, unauthorized status queries, a replacement that is itself withdrawn and a defect affecting
several emitted copies but no source update. The affected-Claim set must identify the seeded statements
without claiming every section is wrong. When runtime protocols are admitted, interruption/restart tests
must preserve the withdrawal and avoid announcing a replacement before it is complete.

Reproduce the audit's defect shape using a synthetic generated-input substitution, not a fresh fixture that
mislabels fake figures as Barrick's. The relevant negative control is a report whose hashes all verify but
whose producer has explicitly withdrawn its conclusion.

### What could kill it

**HYPOTHESIS / kill.** Stop if consumers never reuse detached outputs, if a trusted status channel cannot be
operated within the existing architecture, or if the existing report service already enforces these exact
issued-copy semantics. Do not claim all copies were recalled: report only the issued population and
acknowledgments actually known. If a parallel issuance registry adds ceremony without gating reuse, retire
it and keep only the consumer check in the existing export/verification path. This does not prevent a
malicious or mistaken producer from issuing a bad result in the first place; the source and computation
repairs remain mandatory.

**INFERENCE — Self-score: 810/1000.** The audit demonstrates the failure class, and an agent's reuse decision
is a concrete consumer. The adoption and trust-channel burden prevent this from being a universal assurance
solution or a reason to postpone honest M0 extraction.

## BS-COD-02 — Compensation formula exposure to uncertain metric definitions

**DESIGN — Subsystems, user and workflow.** Bullion and Helm with Portal inputs and Adit presentation.
The remuneration committee or corporate finance team asks: “Do the admitted metric definitions, adjustments
and rounding ranges support one scheme-level performance factor, or can they cross an award threshold?”
An agent returns the governing formula clauses and the metric/basis choice responsible for each possible
factor. The committee decides the policy and any actual award.

### What is new

**FACT.** §23.6 prices economic sacrifices caused by KPI constraints, while COD-30 stresses transformations
that change a KPI without changing the selected economic objective. CC-07 seeks intertemporal distortions
in AISC, and CC-04/AGY-05 check wording/basis. These do not evaluate the actual piecewise scheme function
from an admitted corporate metric to a threshold/target/cap performance factor.

**DESIGN.** The new object is that function, including which metric vintage and approved adjustment policy
it accepts. Compute its image over all admitted metric values or interpretation branches, not only at a
convenient midpoint. A small metric ambiguity may produce a large factor ambiguity at a threshold; a larger
metric change may produce none on a capped portion. That is different from a KPI's shadow price in a capital
optimizer and useful even when no capital optimization exists.

### First shippable slice and mechanism

**DESIGN.** Start with a synthetic financial-only corporate scheme: one threshold/target/cap table, one
dimensionless weight, two explicitly supplied metric-definition branches and a rounding interval. Use no
salary, grant, individual achievement, name or person-linked identifier. The output is an aggregate scheme
index under stated inputs, not an estimate of anyone's compensation.

**INFERENCE — Synthetic known answer.** Suppose fixed other components contribute 0.75 to the scheme index,
and a component weighted 0.25 contributes 0 below normalized achievement 0.90 and 1 at or above 0.90.
If the admitted achievement range is 0.89–0.91, the total index can be 0.75–1.00. Reporting a midpoint-based
1.00 as uniquely established would hide the threshold uncertainty. This range is an identified set under
the synthetic rule, not a probability interval or a payable amount.

Implement a pure exact piecewise evaluator using existing quantity and ratio types. Enumerate rule
breakpoints and the admitted branches; preserve non-monotone rules rather than assuming that endpoints
always suffice. Rule text and its structured interpretation require a role-reviewed version. An unknown
adjustment or restatement policy stays unresolved instead of borrowing the public non-GAAP definition by
default. E605–E607 and OPEN-BULL-1 explain why that distinction matters, but do not supply a complete scheme
formula or prove an actual award was wrong.

Then capture one publicly disclosed corporate scheme formula and relevant metric definition, if independently
reviewed sources expose sufficient terms. Reproduce only the disclosed factor calculation, with hashes and
locators, omitting personal award records. If the terms are incomplete, show precisely which clause prevents
reproduction; do not infer undisclosed committee discretion. No private remuneration data is needed in the
repository.

Keep this as a consumer of `fmine-metrics` and Helm cards, not a payroll or share-plan administration system.
Safety and critical-control gates remain hard, human-owned conditions; the evaluator does not price their
relaxation or authorize payment. FA remains the source/record for any actual accounting integration later.
The same already-reviewed policy branch may be run on original and restated metrics, but the software must
not decide whether a restatement legally changes an award or triggers a recovery action.

### Test

**DESIGN — Known answers and public-filing reproduction.** Exhaustively enumerate a small rational grid and
every threshold boundary, comparing the factor set with an independently authored rule table. Include ties,
caps, overlapping interpretation branches, wrong metric vintages, missing discretion and rounding that
straddles a threshold. A pure unit change must preserve the factor. A public case is admitted only after
independent capture of both formula and input definition; a correct reproduction makes no claim about the
scheme's fairness or any individual's entitlement.

### What could kill it

**HYPOTHESIS / kill.** Stop if no sufficiently disclosed formula can be reproduced, if committees' existing
calculators already expose definition/vintage uncertainty correctly, or if the meaningful workflow requires
personal remuneration data or automatic legal interpretation. Do not build a generic compensation engine.
The smallest surviving result may be an ambiguity warning and a specific policy decision for the committee,
with no numerical factor until that decision is recorded.

**INFERENCE — Self-score: 740/1000.** It serves a named decision owner in §23.6 with a nonlinear consequence
the three lists did not compute. Disclosure completeness and adoption remain substantial gates, but its
bounded mathematics does not need a mine scheduler or a causal deferral model.

## BS-COD-03 — Conditional insurance recovery and interruption evidence

**DESIGN — Subsystems, user and workflow.** Helm and Bullion, consuming read-only Headframe/Crucible event
and loss estimates through Portal/Adit. The risk/insurance team, CFO, insurer reviewer or project lender
asks which part of a stated interruption loss is conditionally recoverable under approved policy terms,
what evidence is missing and which time-sensitive evidence task would resolve the uncertainty. The tool
prepares a local review package; it does not file a claim, contact an insurer or determine legal coverage.

### What is new

**FACT.** COD-05 models dated accessible cash; AGY-06 models settlement cash; CC-15 records disclosed
disruptions; §16 models reliability and maintenance; §23 models scenarios and uses streams as tail insurance.
The candidate inventories contain no explicit insurance-policy loss-recovery calculation or evidence workflow.
An operating loss, a receivable, an insurance claim and an insurer's accepted payment are distinct objects.

**DESIGN.** Add a conditional indemnity calculation over a finite human-approved term interpretation, with
occurrence dates, waiting periods, deductibles, limits and insured entity/perimeter. Keep economic loss,
policy-eligible loss and actual recoveries separate. The important question is often which missing event,
duration or value evidence changes the possible recovery, rather than another estimate of the mine's gross
loss. A disclosure's mention of an outage does not establish its cause or its coverage.

### First shippable slice and mechanism

**DESIGN.** Start entirely synthetic with one interruption, one currency, one accepted loss-rate premise and
one explicitly defined policy: a waiting period applied to loss days, then a monetary deductible, then a
payment limit. The sequence is part of this synthetic contract, not a claim that all real policies work that
way. Use exact `Money`, durations and a finite event fold, with the existing calendar/basis types.

**INFERENCE — Synthetic known answer.** For a six-day interruption with an assumed contribution loss of
$1M/day, a two-day waiting period leaves $4M of eligible loss. A subsequent $0.5M deductible and $3M payment
limit produce a $3M conditional recovery, compared with a $6M stated economic loss. If an unresolved but
admissible coverage branch excludes the event, the candidate recoveries include zero; no branch probability
is invented and the $3M is not booked as cash or a receivable.

The result shows the event/loss evidence and approved term clause behind each operation. For example,
uncertain eligible duration can identify the need for an independent outage chronology, while an unverified
loss baseline identifies the need for a defensible contribution-margin premise. A configured evidence/notice
deadline can be displayed as a required human task, without interpreting statutes or sending a notice.
Use only one pre-agreed term interpretation in the initial evaluator; conflicting endorsements or causes
can remain unresolved rather than triggering a general policy-language parser.

The economic calculation belongs in the planned Helm cash-flow/contract-evaluation scope, with Bullion
reading FA-certified actual costs when a real deployment is admitted. Review the crate charter deliberately
before adding this new domain; do not hide an insurance engine inside a generic utility crate or assume a
charter automatically expands. No new crate or independent insurance subledger is proposed. Pure kernels
receive approved terms and event Claims; adapters own IO, and any later asupersync orchestration uses the
existing bounded/cancellable service model.

Real policies, negotiations, claim files and site event records stay outside this public repository.
Before M2, the positive capability is a synthetic conditional payoff/evidence example, not a Barrick claim
reconstruction or an assertion that insurance will fund a liquidity gap. A later sponsor must supply a
narrow authorized case and an approved term owner. Publicly stated recoveries, if used later, are parity
targets only after independent source capture and never proof of the policy interpretation.

### Test

**DESIGN — Synthetic truth and independent contract oracle.** Independently enumerate eligible days and
cash operations for the stated toy terms, testing a loss shorter than the waiting period, a limit reached,
several explicitly grouped events, wrong insured-entity basis and a missing loss premise. Verify that paid
recovery cannot exceed the admitted limit or eligible amount under those terms and that an unresolved
exclusion cannot become unconditional payable cash. Change the order of synthetic deductible/waiting rules
and require the result identity to change; order is contract semantics, not an arbitrary implementation
detail. A human-approved reference worksheet can supply an additional oracle, independently captured rather
than regenerated from the candidate.

### What could kill it

**HYPOTHESIS / kill.** Stop if a risk team or insurer will not nominate a repeatable evidence question, if
the necessary approved terms cannot be supplied, if legal disputes dominate every case, or if an existing
claims service already produces the same conditional bridge. If it starts requiring actuarial pricing,
automatic coverage judgments, negotiation or a replacement claim-administration platform, narrow it back
to the finite evaluator or abandon the workflow. A modelled reduction in uninsured loss is never authority
to weaken a safety control or take an operational risk.

**INFERENCE — Self-score: 620/1000.** This reaches an underserved financial consequence of the operating
evidence chain, with a small verifiable first calculation. Its larger commercial value is conditional on
access and an agreed interpretation, so it belongs behind the M0 delivery and sponsor gates, not on the
immediate implementation path.
