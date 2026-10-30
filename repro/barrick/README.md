# Reproducing the Barrick public-lane run

This kit lets anyone check FrankenMining's public-lane run on Barrick's quarterly workbooks. The repository holds
no workbook bytes. You fetch the files from the issuer's own host, a script checks each against the size and
SHA-256 registered in this repository, and the tests check that the run gives the expected state root and counts.

FrankenMining is not affiliated with Barrick; these are Barrick's public files, fetched from Barrick's host.

## What a passing run shows

- The 18 registered files and this code revision give the state root, anchor and instance counts in the last
  `[[expected]]` row of [`expected.toml`](expected.toml).
- The counts are also checked against independent Python readers of the same cells (`scripts/oracle/`). The root
  is a reproducibility pin, not a second oracle: it moves whenever the code's output moves. Each move appends a
  row to `expected.toml` with its reason and evidence id. Rows are never edited.
- Two values in each row can be recomputed without this code: `registry_epoch` (sha256 over the sorted
  `file<TAB>sha256` lines of the registry) and `census_digest_of_digests` (sha256 over `file<TAB>numeric digest`
  lines in registry order, with the digests from `python3 -I scripts/oracle/fidelity.py`).

## Run it

You need git, bash, awk, curl, `sha256sum` or `shasum`, and Rust through rustup. The repository's
`rust-toolchain.toml` picks the version.

```sh
git clone https://github.com/Dicklesworthstone/franken_mining
cd franken_mining
repro/barrick/fetch.sh
cargo test -p fmine-public -p fmine-cli -- --ignored
```

`fetch.sh` reads `registries/sources/barrick.toml`. It downloads only the files that are missing, two seconds
apart, saves each under its registered name only when its size and SHA-256 match, and verifies every file
already present without overwriting it. It prints one JSON line per file and a summary line.

To see the root and anchor yourself:

```sh
cargo run -q -p fmine-cli -- assure public --issuer barrick --through 2026Q2
```

The envelope's `certificate` is the state root and `anchor_id` is the anchor. Compare them with the last row of
`expected.toml`. `cargo run -q -p fmine-cli -- doctor` lists each registered file as present and verified, or
not. `repro/barrick/ci_real_files.sh` runs the whole sequence the way CI does.

## If something does not match

| Symptom | Meaning |
|---|---|
| `fetch.sh` reports a mismatch | The issuer re-posted the file: a new vintage. Never edit the digest; register the new file as a new row. |
| Root mismatch while every fetch verified | The code changed without a new `expected.toml` row, or the run is not deterministic. File a defect. |
| Counts mismatch | An extraction or ledger defect. File a defect. |

## Registering a new quarter

A new quarter is data: two `[[source]]` rows in `registries/sources/barrick.toml`, run by a binary built before
the files existed (bd-s9j3; GATE-BULL-01's blind-layout clause). For Q3 2026 the issuer publishes at
2026-11-09 11:00Z.

Before the release, push the revision that will run and record its commit and binary digest:

```sh
git rev-parse HEAD
cargo build --release --locked -p fmine-cli
shasum -a 256 target/release/fmine
```

On release day:

1. Take each URL from the issuer's quarterly results page. Never construct one: the naming rule has already
   changed once (`Barrick_Q3_2024_AISC.xlsx`, then `AISC_Q1_2025.xlsx`, and Q1 2025 sits under `2025/01/`).
2. Download each file into a new, empty directory, and note its byte count and SHA-256.
3. Append one `[[source]]` row per file, copying the fields of the 2026Q2 rows.
   - `quarter = "2026Q3"`, with `path` relative to `url_prefix` and `url_basis = "fetched and verified <date>"`.
   - Name the files `Barrick_Q3_2026_Mine_Stats.xlsx` and `Barrick_Q3_2026_AISC.xlsx`. The independent readers
     find AISC files by the `*AISC.xlsx` pattern.
   - Edit nothing else: subjects and labels were frozen before the release. `git diff --stat` must list only
     the registry.
4. Run `repro/barrick/fetch.sh`. It downloads the two files again and keeps them only when they match the rows.
5. Run the independent readers before the Rust run, and keep their summary lines:
   - `fidelity.py`, `census2.py`, `share_census.py` and `hidden_census.py`;
   - the commands are in `scripts/oracle/README.md`.
6. Check that `shasum -a 256 target/release/fmine` equals the recorded digest. Then run:
   - `target/release/fmine doctor`;
   - `target/release/fmine assure public --issuer barrick`;
   - `target/release/fmine assure public --issuer barrick --through 2026Q2`. Its root must still equal the
     last row of `expected.toml`, which stays the 2026Q2 record.
7. Record a new E id holding:
   - the commit, binary digest and push time;
   - the two rows;
   - the independent readers' numbers;
   - the new root and counts, and the instances that differ from the 2026Q2 run.

## Do not redistribute the files

RISK-31: do not redistribute the files; fetch them from the issuer. Do not mirror them, commit them or attach
them to issues. `.gitignore` keeps `corpus_of_input_docs/` and `*.xlsx` out of git, and CI fails if a workbook
is ever committed.

## Selectors

This section records how the run finds what it reads. It is generated from the code and
`registries/sources/barrick.toml`, and a test fails if the two differ.

<!-- BEGIN GENERATED SELECTORS -->
### Row anchors

A sheet's column-A labels are read lower-cased with whitespace collapsed. Each row below is the first label, top to bottom, that matches one of its labels. A sheet with no cost-of-sales row holds no AISC table; one that has it but lacks another row is refused as a table, naming the row.

| Row | Labels | Match |
|---|---|---|
| Cost of sales | `cost of sales applicable to gold production` | equals |
| Total cash costs | `total cash costs` | equals |
| All-in sustaining costs | `all-in sustaining costs` | equals |
| Ounces sold | `ounces sold` | starts with |
| Cost of sales per ounce | `cost of sales per ounce`, `cos/oz` | equals |
| Total cash costs per ounce | `total cash costs per ounce`, `tcc/oz` | equals |
| All-in sustaining costs per ounce | `all-in sustaining costs per ounce`, `aisc/oz` | equals |

### Subjects

A column header names a subject when it equals a registered subject, or is the subject followed by one lower-case footnote letter. Any other header is reported as unregistered. Regions are segment roll-ups.

- Nevada Gold Mines
- Carlin
- Cortez
- Turquoise Ridge
- Phoenix
- Long Canyon
- Hemlo
- Pueblo Viejo
- Veladero
- Porgera
- Loulo-Gounkoto
- Kibali
- North Mara
- Tongon
- Bulyanhulu
- North America (region)
- Latin America & Asia Pacific (region)
- Africa & Middle East (region)

### Reconstructions

An identity that closes only when a component published elsewhere is added. The run re-reads the component cell and checks that each listed cell's text starts with the listed prefix; if any check fails, the instance stays undecidable.

| Evidence | Table | Subject | Family | Component | Label checks | Basis |
|---|---|---|---|---|---|---|
| E816 | `Barrick_Q1_2025_AISC.xlsx Q1 2025` | Africa & Middle East | aisc_per_oz | `Barrick_Q1_2025_Mine_Stats.xlsx Minestats3!B32` | `A32` starts `Minesite sustaining`; `B2` starts `Loulo-Gounkoto`; `A3` starts `For the three months ended March 31`; `B3` starts `2025` | reconstructed |
| E816 | `Barrick_Q2_2025_AISC.xlsx Q1 2025` | Africa & Middle East | aisc_per_oz | `Barrick_Q1_2025_Mine_Stats.xlsx Minestats3!B32` | `A32` starts `Minesite sustaining`; `B2` starts `Loulo-Gounkoto`; `A3` starts `For the three months ended March 31`; `B3` starts `2025` | reconstructed |
| E816 | `Barrick_Q2_2025_AISC.xlsx YTD Q2 2025` | Africa & Middle East | aisc_per_oz | `Barrick_Q2_2025_Mine_Stats.xlsx Minestats3!B74` | `A74` starts `Minesite sustaining`; `B44` starts `Loulo-Gounkoto`; `A45` starts `For the six months ended June 30`; `B45` starts `2025` | reconstructed |

### Registered files

| File | Kind | Quarter | URL | URL basis | Bytes | SHA-256 |
|---|---|---|---|---|---|---|
| `Barrick_Q1_2024_Mine_Stats.xlsx` | mine_stats | 2024Q1 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2024/q1/Barrick_Q1_2024_Mine_Stats.xlsx | W1 table | 43668 | `e2e1243ec0549f4c487c773869876e2acade401a0b6cd1141adf4b9296aec840` |
| `Barrick_Q2_2024_Mine_Stats.xlsx` | mine_stats | 2024Q2 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2024/q2/Barrick_Q2_2024_Mine_Stats.xlsx | W1 table | 109832 | `1a73d6d7d4658f923757e873eec147ccc74aa8aada6a439f3c98da8540ff68d5` |
| `Barrick_Q3_2024_Mine_Stats.xlsx` | mine_stats | 2024Q3 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2024/q3/Barrick_Q3_2024_Mine_Stats.xlsx | W1 table | 110573 | `d8786ce96f7129bf4cc240e01d2fe7840bfde0df4082517975578e69d73ead3b` |
| `Barrick_Q4_2024_Mine_Stats.xlsx` | mine_stats | 2024Q4 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2024/q4/Barrick_Q4_2024_Mine_Stats.xlsx | W1 table | 110614 | `f5e7db7c4657927f96e8684b40f9b50c0061fa2e0c22912aa480babdb49ef4cd` |
| `Barrick_Q1_2025_Mine_Stats.xlsx` | mine_stats | 2025Q1 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2025/01/Barrick_Q1_2025_Mine_Stats.xlsx | W1 table | 45695 | `ec5158f8b4624643ee83d0099e4f3354dc550729ebb443a2dbfacfc29315998b` |
| `Barrick_Q2_2025_Mine_Stats.xlsx` | mine_stats | 2025Q2 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2025/q2/Barrick_Q2_2025_Mine_Stats.xlsx | W1 table | 107135 | `599864f9958b5a109530c321e9481fa7d59c53ed2c730be96f51035390a2d822` |
| `Barrick_Q3_2025_Mine_Stats.xlsx` | mine_stats | 2025Q3 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2025/q3/Barrick_Q3_2025_Mine_Stats.xlsx | W1 table | 108497 | `b55b857766195fe90464cf813e7a71a5112439d606a6fc96ec3b6c5f01b58715` |
| `Barrick_Q4_2025_Mine_Stats.xlsx` | mine_stats | 2025Q4 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2025/q4/Barrick_Q4_2025_Mine_Stats.xlsx | W1 table | 117396 | `94f6692acfa5f133b97ae028f172cc9c5e7527945788da7bd9a06ffd11c7eec4` |
| `Barrick_Q1_2026_Mine_Stats.xlsx` | mine_stats | 2026Q1 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2026/q1/Barrick_Q1_2026_Mine_Stats.xlsx | W1 table | 55644 | `ddef08dd544f0df511fc30f53c1dd889bb6878257d394b8baad7ac3dbfa8c5d2` |
| `Barrick_Q2_2026_Mine_Stats.xlsx` | mine_stats | 2026Q2 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2026/q2/Barrick_Q2_2026_Mine_Stats.xlsx | W1 table | 131296 | `cf0d3f4fea9c96f967c138d0aca549f4ff4f4a233bd2c5946c4035bf96c3df8e` |
| `Barrick_Q3_2024_AISC.xlsx` | aisc_reconciliation | 2024Q3 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2024/q3/Barrick_Q3_2024_AISC.xlsx | fetched and verified 2026-10-30 | 40977 | `d063ea7c1cd667a6db9c9de1b3aa4cb0ff2dba4a8cb24f6ce889e56af2fd0ef7` |
| `Barrick_Q4_2024_AISC.xlsx` | aisc_reconciliation | 2024Q4 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2024/q4/Barrick_Q4_2024_AISC.xlsx | fetched and verified 2026-10-30 | 42141 | `f842696915b7a44d00194e01a3bdf4a76d1e52d0e31c2610a91be70afb4b1b4d` |
| `Barrick_Q1_2025_AISC.xlsx` | aisc_reconciliation | 2025Q1 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2025/01/AISC_Q1_2025.xlsx | fetched and verified 2026-10-30 | 32587 | `06951434c3a3e325553dbe25018544c7329bc1a8b6e01ab0d3fcdeba2bb4388e` |
| `Barrick_Q2_2025_AISC.xlsx` | aisc_reconciliation | 2025Q2 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2025/q2/AISC_Q2_2025.xlsx | fetched and verified 2026-10-30 | 41599 | `1cbd6b739a276d64e4752ebbc67c369720ccb8eec16845a4858dccd04fd6d178` |
| `Barrick_Q3_2025_AISC.xlsx` | aisc_reconciliation | 2025Q3 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2025/q3/AISC_Q3_2025.xlsx | fetched and verified 2026-10-30 | 41020 | `62901aaab21d64057422a950e29e0ad2ecebad525e0c2b8b9185e4843d9ecfd3` |
| `Barrick_Q4_2025_AISC.xlsx` | aisc_reconciliation | 2025Q4 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2025/q4/AISC_Q4_2025.xlsx | fetched and verified 2026-10-30 | 40336 | `2ce7b8e46b36d745228487e683b221101c4db3019ba67acedea96967486d6755` |
| `Barrick_Q1_2026_AISC.xlsx` | aisc_reconciliation | 2026Q1 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2026/q1/AISC_Q1_2026.xlsx | fetched and verified 2026-10-30 | 43718 | `dbe567e90730b4ef87330b1eaa539c7289a92781e452b949c24f7b705464ef28` |
| `Barrick_Q2_2026_AISC.xlsx` | aisc_reconciliation | 2026Q2 | https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2026/q2/AISC_Q2_2026.xlsx | fetched and verified 2026-10-30 | 49738 | `66515a4ae4305de130b72fa1a3a78bf6aa51ac5a10350fc43732bb621c6d0e99` |
<!-- END GENERATED SELECTORS -->
