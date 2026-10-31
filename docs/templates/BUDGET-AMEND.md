# BUDGET-AMEND-nnn: `<crate>`

<!--
Copy to docs/delivery/BUDGET-AMEND-nnn-<crate>.md, with nnn the next unused number (§28.4.3). `xtask check-amend`
reads the Crate, New planned and Approval lines, so keep their `- **Field:**` form. New planned is digits only.

A change that adds LOC to a crate over its planned budget fails `check-all` and CI until a record for that crate
is approved with New planned at least the crate's actual LOC. A change with net LOC <= 0 needs no record.

Only the owner writes the approval. Until then the line reads PENDING, and the record covers nothing. Approval
reads exactly: approved by <who>, YYYY-MM-DD.

Once approved, the owner, or whoever the owner names, updates the crate's row in registries/loc_budget.toml and
plan §28.3: the row, and the totals `check_loc_budget` pins.
-->

- **Crate:** `<crate>`
- **Planned / ceiling now:** <planned> / <ceiling> (`registries/loc_budget.toml`)
- **Actual:** <actual> (`cargo run -p xtask -- loc` at `<sha7>`)
- **New planned:** <digits>
- **New ceiling:** <1.15 × new planned, to the nearest thousand as §28.3 rounds>
- **New scope:** <what the extra lines are for, by bead id>
- **Gate served:** <the GATE-* or done-when the scope serves>
- **Alternatives:** <where else the code could live, and why not there>
- **Approval:** PENDING
