# CONTRACT.md — `fmine-public`

## 1. Charter & Layer
- **Crate:** `fmine-public`
- **Layer:** `V` (Verification, Ingestion & Assurance)
- **Subsystem:** `Assurance`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["sources", "aisc", "synthetic"]`

## 2. Mathematical & Domain Formulation
The public lane of M0 (`fmine assure public`).

- `sources`: the source registry (`BD-055`, `BD-322`). The 18 public Barrick workbooks (Mine Stats 2024Q1–2026Q2,
  AISC reconciliations 2024Q3–2026Q2) are pinned by file name, URL, byte size and SHA-256 from W1 (E451, E452).
  The registry also holds the 18 reporting subjects printed in the AISC table headers and the registered
  reconstructions (E816).
- `aisc`: a label-anchored reader for the AISC reconciliation tables. It finds rows by column-A label and the
  header row by registered subjects, so no coordinate is fixed. It also builds the identity ledger
  (`GATE-BULL-01` part 2, plan §21.6).
  - **Declared construction:** component sum. TCC = Σ block-1 lines; AISC = TCC + Σ block-2 lines. Each rate is
    r = 1000·N/Q, checked on intervals (N ± k/2, Q ± 1/2, r ± 1/2). All arithmetic is exact (`i128`).
  - **States:** (i) consistent; (ii) explained by a registered reconstruction, re-verified on the run;
    (iii) issuer discrepancy, with an exact certificate and any single-line sign-flip witness;
    (iv) undecidable; (v) refused; not published (excluded by name).
  - **Alternative construction:** subtotal rounded once. It is reported, never scored.
- `synthetic`: invented workbooks in the real layout, seeded with one known answer per state. They are never
  gate-eligible.
- `lib`:
  - `load_registered_sources` verifies every registered file;
  - `assess` builds the ledger and the state root;
  - `PublicAssuranceReport::to_json` is the envelope payload;
  - `cell_to_public_claim` is the public-basis `Claim` builder (`BD-058`).

## 3. Authority, Epistemics & Representation
- Every reported exception cites file, file SHA-256, `sheet!cell`, subject, family and ownership basis
  (`IdentityFamily::basis`).
- The `LedgerAnchor` state root (BLAKE3 over an `M0AS` v2 frame) binds:
  - every input's SHA-256, numeric digest and census;
  - every instance's state and detail;
  - unregistered headers and table refusals.

  Changing any input cell moves the root.
- The report carries `lane`, `gate_eligible`, a `no_claim` line, and the four INV-002 flags, all false.

## 4. Public API Surface
- `sources::{SourceKind, RegisteredSource, RegisteredReconstruction, IssuerProfile, BARRICK, BARRICK_SOURCES,
  BARRICK_SUBJECTS, BARRICK_RECONSTRUCTIONS, issuer_profile}`
- `aisc::{IdentityFamily, IdentityState, IdentityInstance, FootingCertificate, UnregisteredHeader, AiscTable,
  RateMiss, locate_table, ledger_for_table, rate_interval_holds, reconstruction_closes, match_subject, cell_text}`
- `synthetic::{synthetic_aisc_cells, synthetic_workbooks, SYNTHETIC_SUBJECTS, SYNTHETIC_RECONSTRUCTIONS}`
- `Lane`, `LoadedSource`, `TableRefusal`, `AlternateCounts`, `PublicAssuranceReport`, `load_registered_sources`,
  `assess`, `run_public_assurance`, `run_barrick_m0_assurance`, `run_synthetic_assurance`, `cell_to_public_claim`

## 5. Guards & Typed Refusals
- `FMINE-REF-502` (exit 8) names every registered file that is missing, digest-mismatched or unparseable. A run never
  substitutes content.
- `FMINE-REF-401` from `cell_to_public_claim` on text or formula-error cells.
- Instance-level refusals (state (v)) carry a reason:
  - an unrounded stored value;
  - a non-numeric line;
  - "attributable share needed" when a non-zero non-controlling-interest line separates the bases of cost of sales
    and ounces;
  - one of rate and ounces being zero.

## 6. Proof Obligations (`PO-*`)
- `PO-PUB-1` (real files, ignored test; recorded in MOR_CASE E820–E823): the census and digest of all 18 workbooks
  equal an independent reader's.
  - **Ledger:** 34 visible AISC tables, 2,595 instances. Footing: 1,004 (i), 4 (iii), 6 (v), 24 not published.
    Per ounce: 1,116 (i), 3 (ii), 17 (iv), 385 (v), 36 not published.
  - **Alternative construction:** 1,008 footing and 994 rate instances consistent.
  - **Identities:** each (ii) and (iii) instance is identified.
- `PO-PUB-2` (synthetic): every seeded answer is reproduced, including the sign-flip witness. Perturbing one cell
  moves the state root and the affected state. A wrong component label leaves the instance in (iv).
- `PO-PUB-3`: a workspace without the files refuses, naming all 18.

## 7. Failure Modes & Deterministic Fallbacks
- A sheet with a cost-of-sales row but no locatable structure becomes a `TableRefusal`.
- A header that names no registered subject becomes an `UnregisteredHeader`.
- Neither stops the run, and neither is ever guessed.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01` (parts 1 and 2; part 3, issuer consistency across vintages, is not yet scored)

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-evidence`, `fmine-claim`, `fmine-io` (and, in the manifest, `fmine-units`,
  `fmine-time`, `fmine-metrics`, not used by this crate since BD-321).

## 10. Non-Goals & Scope Boundary
- Operates only on public disclosures (`ConfidentialityTier::Public`).
- Makes no reserve, resource, geotechnical, regulatory or investment claim.
- Implies no issuer affiliation.
