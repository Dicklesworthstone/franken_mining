# Common brief for every Round-7 modes-of-reasoning pass

RUN_ID `20261029T183653Z`. Repository: `/Users/jemanuel/projects/franken_mining` (HEAD `68e7091`).
Case file: `planning/mor/r7/MOR_CASE.md` (read the Decision Contract, the run evidence E673–E690, Pivotal
Unknowns and Source Access in full; the imported ids E001–E672 are defined in the standing register
`planning/mor/MOR_CASE.md` and in `planning/corpus/C*.md` and `planning/research/W*.md`).
Run directory: `planning/mor/r7/.mor/run-20261029T183653Z/`.

## Epistemic integrity rules

Your objective is the most truthful and decision-useful result supported by accessible evidence. Your reasoning
mode is a bounded operation, not a persona, ideology, or side to advocate. You are not required to disagree,
criticize, produce a finding, be novel, or fill a quota. A checked NO_MATERIAL_INCREMENT result is fully
acceptable after you state what you examined.

**Read-only boundary.** This is analysis, not implementation. Do not modify product code, tests, fixtures,
specifications, the plan, the beads, or any repository file other than your own raw output. If a focused
reproduction needs temporary files, put them under
`/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/<ASSIGNMENT_ID>/`,
record the exact commands, and leave the checkout unchanged. Do not delete files. Do not start cargo builds (the
host is short of disk, E690); the debug binary `$TMPDIR/rch_target_fmine/debug/fmine` already exists and accepts
`--workspace <dir>` and `--format json|markdown`. Use `python3 -I` for scripts. Do not run git commands that
change state.

**Do not:**
- invent project facts, stakeholder beliefs, source content, commands, test results, external facts or
  authorities;
- treat confidence, labels, certificates, workflow state, agent agreement or prose as evidence;
- weaken tests, narrow the specification to fixtures, change the target or hide failures;
- convert a story, analogy, best practice, hypothetical edge case or owner-known limitation into a discovery;
- rate severity against a deployment or threat model the project does not have;
- recommend changing a project-defining substrate (Rust, asupersync, the FrankenSuite siblings);
- invent an alternative, defeater, competing value or opposing position to fill a field;
- quote text from `planning/corpus/C5_reko_diq_esia.md` or its source PDF, or any personal data.

**Claim discipline.** For every material claim, cite evidence (E ids from the case file; for things the case
does not yet index, cite the exact file and line, sheet and cell, command, or URL with access date, and say it is
new evidence) and state scope, inference, assumptions, materiality, uncertainty, a bounded
alternative/defeater check, and the claim-type-appropriate resolution or reversal condition. If no credible
alternative is found, write NONE_IDENTIFIED_AFTER_BOUNDED_CHECK and state what you examined. Narrow, mark as
HYPOTHESIS, or remove anything the evidence does not support. Unsupported material claims are worse than a
short output. Do not read other passes' raw outputs.

## Output contract

Write exactly one file: `planning/mor/r7/.mor/run-20261029T183653Z/raw/<ASSIGNMENT_ID>.md`. Its first non-empty
line must be exactly one of:

    RESULT_STATUS: MATERIAL_INCREMENT
    RESULT_STATUS: NO_MATERIAL_INCREMENT
    RESULT_STATUS: BLOCKED_BY_MISSING_EVIDENCE

Then include only useful sections:

- **WHAT I CHECKED** — sources, searches, executions, derivations actually examined.
- **CANDIDATE CLAIMS** — zero or more atomic provisional claims, each with: provisional id (`<ASSIGNMENT_ID>-Cn`);
  claim type (observation | deductive | causal | forecast | value | recommendation | design_option | question);
  statement; scope / definitions / time horizon; evidence refs; inference; assumptions; bounded
  alternative/defeater check; resolution or reversal condition; materiality; uncertainty basis. For a
  recommendation also give the supporting claim ids, cost, reversibility and an acceptance test. When a claim
  implies a plan edit, name the plan section and line(s) and say what the corrected text must assert.
- **NULL OR BLOCKED RESULT** — when applicable.

Write in plain, direct English. Use Unicode maths, not LaTeX. When you finish, reply with at most 12 lines:
the result status, the number of candidate claims, and the three most material claims in one line each.
