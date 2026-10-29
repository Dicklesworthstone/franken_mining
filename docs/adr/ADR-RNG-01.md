# ADR-RNG-01: Single Deterministic Production RNG (`Philox4x32-10` + `Sobol/Owen`)

- **Status:** Accepted (2026-10-29, `BD-014`, `§27.3`, `§29.2`)
- **Context:** FrankenMining requires cross-ISA bit-reproducible conditional simulations (`Strata`), Monte Carlo scenario trees (`Pushback`, `Helm`), discrete-event fleet simulations (`Headframe`), and Common Random Numbers (`CRN`) across design alternatives and wireframe boundary updates (`BD-317`). `frankensim` provides `Philox4x32-10` while `franken_numpy` (`fnp-random`) uses `Philox4x64-10` (`OPEN-SUITE-2`).
- **Decision:**
  1. Production randomness comes from **one** generator family: lifted `frankensim` `Philox4x32-10` and scrambled `Sobol/Owen` quasi-Monte Carlo sequences in `fmine_core::rand`.
  2. Every random draw is counter-based and keyed by the canonical 6-tuple `(seed_root, partition, domain, var_group, realization, brick_or_entity_id)` so stream values are invariant to thread count, tile scheduling order, and polygon boundary updates (`"strata_bg"`).
  3. `fnp-random` is confined to dev/oracle cross-checks (`FD-FNP-01`).
- **Consequences:** Single golden-stream surface across `aarch64-darwin`, `x86_64-linux`, and `wasm32`; zero sequential RNG state contention; deterministic `CRN` variance reduction on every `DecisionCard`.

---

# ADR-NAMING-01: Crate Prefix `fmine-` and Binary Names `fmine` / `fmined`

- **Status:** Accepted (2026-10-29, `BD-014`, `OPEN-B4-09`, `§28.1`, `E651–E652`)
- **Context:** Shorter prefixes collide across the FrankenSuite and host OS: `fm-*` is `frankenmermaid`'s crate prefix (`fm-core`, `fm-cli`), `fmd` is `franken_markdown`, `fmn` is `franken_manim`, and `/usr/bin/fm` is macOS's Foundation Models CLI.
- **Decision:**
  1. All workspace crates use the `fmine-` prefix (`fmine-core`, `fmine-units`, `fmine-time`, ..., `fmine-cli`).
  2. Binaries are `fmine` (operator/robot CLI), `fmined` (node daemon), `fmine-mcp`, and `fmine-tui`.
  3. Binary format magic is `FMNE` (`[0x46, 0x4D, 0x4E, 0x45]`); refusal codes use `FMINE-REF-xxx`; environment variables use `FMINE_*`.
- **Evidence:** Direct `crates.io` API query (`E651`, 2026-10-29) confirmed HTTP 404 / 0 results for `fmine`, `fmined`, `fmine-core`, `fmine-cli`, `franken_mining`, `franken-mining`, and `frankenmining`.
