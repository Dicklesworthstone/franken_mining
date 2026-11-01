# Fixture capture record (§33.2)

<!--
Each file under fixtures/ needs a record beside it, named <fixture file name>.capture.toml, holding the nine fields
below once each as `key = "value"` lines (`#` comments are allowed). `xtask check-all` and `xtask check-oracle`
fail on:
- a fixture with no record, or a record with no fixture;
- a missing, empty, repeated or unknown field;
- a digest that is not sha256:<64 lower-case hex>;
- an outputs_digest that is not the fixture file's sha256;
- a date that is not YYYY-MM-DD.

A fixture is produced by a pinned external tool or a published source, in a separate lab runner, and is never
regenerated from FrankenMining output. Regenerating one changes its sha256, so the record fails until a new capture
is recorded. That needs a bead, a reason and a reviewer who is not the author of the code under test (§33.2).

The licence is the fixture's own (RISK-17). No issuer data goes in a fixture before the owner and counsel decide
(RISK-31).

When the oracle is a command that prints the fixture, let `xtask capture-fixture` run it and write both files, so the
record's command and digests are those of the run (BD-136):

  cargo run -p xtask -- capture-fixture fixtures/<dir>/<file> --oracle "<tool>" --version "<version>" \
    --captured-by "<role>" --licence "<terms>" --input <generator or input file> [--note "<what it is for>"] \
    -- <command> <args>...

A fixture from a published source, with no command to run, gets its record by hand from the form below.
-->

```toml
oracle = "<the tool or published source, e.g. MineLib UPIT>"
version = "<its version, release or retrieval id>"
platform = "<where it ran, e.g. x86_64-linux>"
inputs_digest = "sha256:<64 hex of the inputs, or of their manifest>"
outputs_digest = "sha256:<64 hex of the fixture file itself>"
command = "<the command that produced it>"
captured_by = "<role of who ran the capture>"
date = "<YYYY-MM-DD>"
licence = "<SPDX id or the source's terms>"
```
