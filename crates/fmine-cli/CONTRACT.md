# CONTRACT.md — `fmine-cli`

## 1. Charter & Layer
- **Crate:** `fmine-cli`
- **Layer:** `U` (User Surfaces & CLI)
- **Subsystem:** `Surfaces`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["cli", "envelope"]`

## 2. Mathematical & Domain Formulation
Implements the `fmine` binary and library surface (`WP-06`, `BD-111`–`BD-117`, `BD-071`), including `FMT-ENV-1` `Envelope` (`schema_version: "fmine.env.v1"`), deterministic exit-code taxonomy (`0`, `10`, `20`, `30`, `40`, `50`), and the `assure public --issuer barrick`, `doctor`, `claim show`, and `robot-docs` verbs.

## 3. Authority, Epistemics & Representation
Every CLI response is wrapped in an `FMT-ENV-1` `Envelope` carrying an `FMT-ANCHOR-1` `anchor_id` or a typed `Refusal { code, exit_code, what, fix }`.

## 4. Public API Surface
- `OutputFormat`, `CliCommand`, `Envelope`
- `parse_args`, `execute_command`

## 5. Guards & Typed Refusals
- Maps every `Refusal` to its canonical exit code (`10..50`) and JSON refusal payload.

## 6. Proof Obligations (`PO-*`)
- `PO-CLI-1`: `fmine assure public --issuer barrick` exits `0` and emits a valid `FMT-ENV-1` JSON envelope with `workbooks_verified: 18` and `anc_...` ledger anchor.

## 7. Failure Modes & Deterministic Fallbacks
Unknown verbs or invalid arguments return exit code `2` with usage instructions.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-crs`, `fmine-evidence`, `fmine-claim`, `fmine-registry`, `fmine-array`, `fmine-numerics`, `fmine-calib`, `fmine-decide`, `fmine-io`, `fmine-public`, `fmine-metrics`.

## 10. Non-Goals & Scope Boundary
Does not bypass kernel or registry validation; delegates all calculations to `V`/`E`/`K`/`S0` crates.
