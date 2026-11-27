# CONTRACT.md — `fmine-drill`

## 1. Purpose
Corebox drillhole records on the authority ledger (`BD-163`, plan §12.1): collar, survey, interval, sample, dispatch, assay result, QC insertion and lab certificate. A record is written once and corrected only by superseding it with a reason; every version stays readable. A commit-time law refuses a record that names one that is not recorded.

- **Crate:** `fmine-drill`
- **Layer:** K1
- **Subsystem:** Corebox
- **LOC budget:** 15,000 planned / 17,000 ceiling
- **Allowed Module Families:** `["records", "store"]`

## 2. Inputs
- Typed records (`records::DrillRecord`) from importers, and corrections with their reasons (`store::Change`).
- An authority-ledger cell: `fmine_ledger::l0::L0Ledger` with its `CellLease` for `store::commit`. For a durable `fmine_ledger::segment::SemanticSegmentWriter`, the caller builds the batch with `store::batch_for` and `store::delta_ops` and appends it with `append_with_law(&batch, store::drill_law)`; this crate does no IO.
- Dependencies: `fmine-core` (codec, digests), `fmine-evidence` (refusals) and `fmine-ledger` (`K1 -> S0`, `K1 -> S1`). Zero external crates.

## 3. Outputs
- One `DRLR` `FMNE` frame (version 1, CRC32C) per record version, stored at `drill/<kind>/<id>` as a `Put` (first version) or a `Supersede` (a correction with its reason).
- Reads: `store::current`, `store::history` (every version, oldest first, with batch sequence and epoch) and `store::as_of` (the version live after a given number of committed batches).

## 4. Invariants
- **Supersede, never edit (`INV-003`, plan §8.1):** a `Put` over a recorded key and a `Supersede` of an unrecorded key are refused by the ledger; every earlier version keeps its bytes and stays readable as of its time (`supersession::test_corrections_supersede_and_every_version_stays_readable`, `supersession::test_an_edit_in_place_is_refused_and_changes_nothing`).
- **A correction says why:** a `Supersede` carries a non-empty reason and a `Put` carries none.
- **No dangling names:** a survey or interval names a recorded collar; a sample its intervals; a dispatch its samples; a result its sample, dispatch and certificate; a QC insertion its sample and, for a duplicate or umpire, the original. A record and what it names may arrive in one batch. Ids never disappear, so the names stay good after any correction.
- **Result and dispatch agree:** a result's dispatch lists its sample and went to the lab that reported it. A dispatch correction cannot change the lab or drop a sample, so this survives corrections.
- **QC material is not core:** a CRM or blank sits on a `QcMaterial` sample, which is taken from no interval; a duplicate or umpire names drilled samples on both sides. A sample correction cannot change which kind it is.
- **One encoding per version:** decoding refuses bytes that are not exactly what the encoder writes for the record they hold (for example `-0.0`, or trailing bytes).
- **Same law on both rungs:** `store::drill_law` runs in `L0Ledger::commit_with_conservation` and `SemanticSegmentWriter::append_with_law`; both admit and refuse the same batches and reach the same head (`supersession::test_the_law_refuses_dangling_and_inconsistent_records_and_writes_nothing`).
- Records that cannot be true alone are refused before any write: an interval whose `to` is not past its `from`, an azimuth outside [0, 360), a dip outside [−90, 90], an assay mass fraction outside [0, 1], a "no survey" station with an orientation, a collar with no position or with planned and actual positions in two CRSs.

## 5. Refusals
- `DrillError::Invalid` → `FMINE-REF-109` (a field outside what the record allows; two changes to one record in one batch).
- `DrillError::CrsMismatch` → `FMINE-REF-106` (planned and actual collar positions in different CRSs).
- `DrillError::Corrupt` → `FMINE-REF-505` (bytes that are not one canonical record version).
- Ledger refusals keep the ledger's codes: `KeyAlreadyExists` → `FMINE-REF-502`, `KeyNotFound` → `FMINE-REF-201`, and the law's violations (`ConservationViolation`) → `FMINE-REF-101`.

## 6. Determinism class
`BitwisePortable` (plan §29.2): integer, byte and ordered-map operations; floats are only compared and stored as IEEE-754 bits.

## 7. Evidence class of outputs
Records as imported: layer L0 Evidence (plan §13.2). This crate assigns no evidence class; an interval's logger (human, scanner, model) is kept so that later stages can.

## 8. Validity domain
- Depths along the hole in integer millimetres; positions in a registered CRS with one-sigma accuracies; assay values as mass fractions, censored values kept as bounds.
- Records are checked alone and by name. The law does not check depths against the hole (an interval below the last survey) or interval overlap per logger; those belong to desurvey (`BD-164`) and the overlap join (`BD-165`).

## 9. Oracles
- Hand-built records and frames in the unit tests, including frames written with the record writer but holding values the checks refuse, and a `-0.0` frame resealed with a valid CRC32C.
- The ledger's own refusals (`fmine-ledger`), observed on both rungs.

## 10. Known gaps
- No importer writes these records yet (acQuire dialect, `BD-188`).
- MNPI embargo state on results is not in the record (`BD-170`); a later frame version adds it.
- Certificate originals are named by SHA-256 only; storing and checking them in CAS is `BD-171`. `OB-LAB` (dispatched → received or declared lost) is `BD-172`.
- `Structure`, `Density`, `GeometTest`, `GeotechLog` and `TwinPair` (plan §12.1) are not here yet (`BD-285` for twins).

## 11. Public API
- `records::{DrillError, RecordKind, HoleType, PositionMethod, SurveyInstrument, RunDirection, RecoveryMethod, SampleType, AssayMethod, QaqcRegime, BlankStage, DuplicateStage, Position, Collar, Orientation, Survey, Recovery, Alteration, Logger, Interval, CustodyEvent, Sample, Dispatch, AssayValue, CertificateRef, AssayResult, QcKind, QcInsertion, LabCertificate, DrillRecord, RecordVersion, encode_version, decode_version, RECORD_TAG, RECORD_VERSION, MAX_ID_BYTES, MAX_TEXT_BYTES, MAX_LIST_ITEMS}`
- `store::{KEY_PREFIX, record_key, parse_key, Change, StoreError, delta_ops, batch_for, commit, drill_law, StoredVersion, history, current, as_of}`

## 12. Proof obligations
- `BD-163` (supersession tests, `tests/supersession.rs`, both rungs): `supersession::test_corrections_supersede_and_every_version_stays_readable`, `supersession::test_an_edit_in_place_is_refused_and_changes_nothing`, `supersession::test_the_law_refuses_dangling_and_inconsistent_records_and_writes_nothing`, `supersession::test_records_written_one_per_batch_or_together_read_the_same`.
- Record checks and codec: `records::tests::every_record_kind_and_variant_round_trips_with_one_encoding`, `records::tests::records_that_cannot_be_true_are_refused`, `records::tests::decoding_refuses_torn_tampered_and_non_canonical_bytes`.
