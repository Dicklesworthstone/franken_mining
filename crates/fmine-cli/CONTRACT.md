# CONTRACT.md — `fmine-cli`

## 1. Charter & Layer
- **Crate:** `fmine-cli`
- **Layer:** `U` (User Surfaces & CLI)
- **Subsystem:** `Surfaces`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["cli", "envelope"]`

## 2. Mathematical & Domain Formulation
The `fmine` binary and library surface (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

- **Envelope:** `FMT-ENV-1` (`schema_version: "fmine.env.v1"`).
- **Verbs:**
  - `assure public --issuer <issuer>`: the gate-eligible run over registered public filings;
  - `assure public --synthetic`: the known-answer run, never gate-eligible;
  - `doctor`;
  - `claim show <id>`;
  - `robot-docs`.

## 3. Authority, Epistemics & Representation
- Every response is an `FMT-ENV-1` envelope.
- A successful assurance run carries the run's `anc_…` anchor id, and the report as payload.
- The payload holds:
  - `lane`, `gate_eligible` and a `no_claim` line;
  - the four INV-002 flags, all false;
  - sources with SHA-256, the extraction census, the identity ledger with cited exceptions, and the quarantine;
  - the state root and `next_actions` (`[{command, why}]`).
- A refusal carries `{code, exit_code, what, fix}` and a null payload.

## 4. Public API Surface
- `OutputFormat`, `CliCommand`, `Envelope`
- `parse_args`, `execute_command`

## 5. Guards & Typed Refusals
- **Exit 0:** ok.
- **Exit 2:** a usage error with no refusal code. This covers:
  - an unknown verb or flag;
  - a missing `--issuer`;
  - an unregistered issuer;
  - `--issuer` together with `--synthetic`.
- **Exit 3–8:** the catalog refusal's own exit code. For example, `FMINE-REF-502` (exit 8) means a registered file
  is missing or altered.
- `claim show` refuses (`FMINE-REF-502`): M0 has no claim store, and the verb never fabricates a claim.

## 6. Proof Obligations (`PO-*`)
- `PO-CLI-1`: `assure public --synthetic` exits 0 with `lane` `synthetic`, `gate_eligible` false and the seeded
  totals.
- `PO-CLI-2`: `assure public --issuer barrick` without the registered files exits 8 and names them.
- `PO-CLI-3` (real files, ignored test): with the files it exits 0 with 34 tables, 2,595 instances and totals
  `{i 2120, ii 3, iii 4, iv 17, v 391, np 60}`.

## 7. Failure Modes & Deterministic Fallbacks
None: every failure is a usage error or a typed refusal.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-crs`, `fmine-evidence`, `fmine-claim`,
  `fmine-registry`, `fmine-array`, `fmine-numerics`, `fmine-calib`, `fmine-decide`, `fmine-io`, `fmine-public`,
  `fmine-metrics`.

## 10. Non-Goals & Scope Boundary
- Computes nothing itself: all calculation is delegated to the `V`/`E`/`K`/`S0` crates.
- Never writes to any external system.
