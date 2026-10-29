# Common brief for every Round-7 independent claim auditor

RUN_ID `20261029T183653Z`. Repository `/Users/jemanuel/projects/franken_mining` (HEAD `68e7091`, not yet pushed).
MOR project root for this run: `planning/mor/r7/`. Case: `planning/mor/r7/MOR_CASE.md`. Canonical ledger:
`planning/mor/r7/MOR_CLAIMS.jsonl` (read your batch's records there; ignore which pass wrote them).
Schema: `/Users/jemanuel/.claude/skills/modes-of-reasoning-project-analysis/references/CLAIM_SCHEMA.md` §8–§9.

You did not author these claims. Determine the strongest statement the evidence and the claim-type contract
support. The integrity rules in `planning/mor/r7/PASS_BRIEF_COMMON.md` bind you (read them first), with one
change: your only output is your audit JSONL file.

The M0 debug binary built from HEAD is at
`/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/fmine_debug_68e7091`
(usage: `<bin> assure public --issuer barrick --workspace <dir> [--format json|markdown]`; also `doctor`,
`robot-docs`, `claim show`). Scratch space for reproductions:
`/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/<AUDIT_ID_PREFIX>/`.
Do not start cargo builds. Do not delete files. Use `python3 -I`.

For every claim in your batch:
1. Inspect or reproduce the cited evidence yourself; do not rely on the claimant's summary.
2. Check source fidelity (exact file/line, sheet/cell, page, URL) or value authority.
3. Apply the type-specific contract (CLAIM_SCHEMA.md §8).
4. Check the inference for missing premises and category errors.
5. Check scope, definitions, horizon, assumptions, deployment and project identity.
6. Do a bounded credible alternative/defeater check; write NONE_IDENTIFIED_AFTER_BOUNDED_CHECK when truthful.
7. Assess whether the evidence paths are independent.
8. Run a focused dynamic test for runtime, reachability, numerical or compatibility claims when feasible
   (run the binary, recompute the arithmetic, read the workbook cell).
9. Decide decision materiality.

Write one JSON object per claim (one per line, no fences) to
`planning/mor/r7/.mor/run-20261029T183653Z/audit/<YOUR_BATCH_FILE>.jsonl` with the fields: `run_id`
("20261029T183653Z"), `audit_id` ("<AUDIT_ID_PREFIX>-C###"), `claim_id`, `auditor` (your assignment id),
`outcome` (VERIFIED | PARTIALLY_SUPPORTED | HYPOTHESIS | REJECTED | DUPLICATE | NON_MATERIAL | SUPERSEDED),
`corrected_statement`, `evidence_inspected` (E ids and/or exact locators), `execution_performed` (command, or
null), `resolution_analysis`, `alternative_or_defeater_check`, `independence_assessment` (list of strings),
`remaining_uncertainty`, `decision_impact`, `audit_timestamp_utc`.

A recommendation or design option is VERIFIED only if its supporting claims hold, the action follows under the
owner's loss function (case Decision Contract), and it names an acceptance test; otherwise use
PARTIALLY_SUPPORTED or HYPOTHESIS and say what is missing. Do not preserve a false argument as a perspective. Do
not average confidence or infer truth from agent count. Do not convert a value into an empirical claim.

When done, reply with at most 8 lines: counts by outcome, and the claims you REJECTED or materially corrected
(one line each).
