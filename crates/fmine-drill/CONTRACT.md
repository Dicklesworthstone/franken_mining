# CONTRACT.md — `fmine-drill`

## 1. Purpose
Corebox drillhole records on the authority ledger (`BD-163`, plan §12.1): collar, survey, interval, sample, dispatch, assay result, QC insertion and lab certificate. A record is written once and corrected only by superseding it with a reason; every version stays readable. A commit-time law refuses a record that names one that is not recorded. Minimum-curvature desurvey turns a collar and its survey stations into a 3-D trace (`BD-164`, plan §12.2), and a sweep-line join finds the overlapping from/to intervals of two sets, hole by hole (`BD-165`). Compositing cuts domain runs into length- or mass-weighted composites that conserve their inputs (`BD-166`, PO-CORE-2).

- **Crate:** `fmine-drill`
- **Layer:** K1
- **Subsystem:** Corebox
- **LOC budget:** 15,000 planned / 17,000 ceiling
- **Allowed Module Families:** `["composite", "desurvey", "join", "records", "store"]`

## 2. Inputs
- Typed records (`records::DrillRecord`) from importers, and corrections with their reasons (`store::Change`).
- An authority-ledger cell: `fmine_ledger::l0::L0Ledger` with its `CellLease` for `store::commit`. For a durable `fmine_ledger::segment::SemanticSegmentWriter`, the caller builds the batch with `store::batch_for` and `store::delta_ops` and appends it with `append_with_law(&batch, store::drill_law)`; this crate does no IO.
- Dependencies: `fmine-core` (codec, digests, portable trigonometry), `fmine-evidence` (refusals), `fmine-claim` (`SpatialSupport`, `AdditivityClass`) and `fmine-ledger` (`K1 -> S0`, `K1 -> S1`). Zero external crates.
- For compositing: domain runs (`composite::Domain`), inputs with their span, value bounds and optional mass (`composite::Input`, `Bounds::of(&AssayValue)`), and a `composite::Rule` (length, weighting, residual handling, additivity, input support).

## 3. Outputs
- One `DRLR` `FMNE` frame (version 1, CRC32C) per record version, stored at `drill/<kind>/<id>` as a `Put` (first version) or a `Supersede` (a correction with its reason).
- Reads: `store::current`, `store::history` (every version, oldest first, with batch sequence and epoch) and `store::as_of` (the version live after a given number of committed batches).
- `composite::CompositeSet`: composites by depth, each with domain, span, sampled length, weight, value bounds (none when unsampled), `SpatialSupport::Composite { length_mm }` and the inputs it takes; the dropped residual pieces; and the conservation totals with their relative error.
- `join::overlaps` and `join::join_intervals`: every overlapping pair of two span or interval sets with the span they share, sorted by (hole,) left index and right index. `join::self_overlaps`: the overlapping pairs within one set.
- `desurvey::Trace` from `desurvey::desurvey(&Collar, &[Survey])`: each station's point in the collar's CRS, and `Trace::at(depth_mm)`, the point and direction at any depth, marked `extrapolated` above the first station or below the last.

## 4. Invariants
- **Supersede, never edit (`INV-003`, plan §8.1):** a `Put` over a recorded key and a `Supersede` of an unrecorded key are refused by the ledger; every earlier version keeps its bytes and stays readable as of its time (`supersession::test_corrections_supersede_and_every_version_stays_readable`, `supersession::test_an_edit_in_place_is_refused_and_changes_nothing`).
- **A correction says why:** a `Supersede` carries a non-empty reason and a `Put` carries none.
- **No dangling names:** a survey or interval names a recorded collar; a sample its intervals; a dispatch its samples; a result its sample, dispatch and certificate; a QC insertion its sample and, for a duplicate or umpire, the original. A record and what it names may arrive in one batch. Ids never disappear, so the names stay good after any correction.
- **Result and dispatch agree:** a result's dispatch lists its sample and went to the lab that reported it. A dispatch correction cannot change the lab or drop a sample, so this survives corrections.
- **QC material is not core:** a CRM or blank sits on a `QcMaterial` sample, which is taken from no interval; a duplicate or umpire names drilled samples on both sides. A sample correction cannot change which kind it is.
- **One encoding per version:** decoding refuses bytes that are not exactly what the encoder writes for the record they hold (for example `-0.0`, or trailing bytes).
- **Same law on both rungs:** `store::drill_law` runs in `L0Ledger::commit_with_conservation` and `SemanticSegmentWriter::append_with_law`; both admit and refuse the same batches and reach the same head (`supersession::test_the_law_refuses_dangling_and_inconsistent_records_and_writes_nothing`).
- **Desurvey is minimum curvature (PO-CORE-1):** between stations the hole is the circular arc joining their directions. It is within 1 mm per 1,000 m of exact positions on holes built from arcs and straight runs, at every 250 mm and every station, and of the textbook inclination-and-azimuth form on 60 seeded random 2 km holes. Measured: 1.9 nm from the geometry, the rounding of UTM coordinates, and 7e-11 m per 1,000 m from the textbook. The balanced-tangential control (ratio factor 1) is 4.5 m per 1,000 m off, so the comparison detects a wrong method (`desurvey::test_holes_of_arcs_and_straight_runs_desurvey_to_their_true_positions`, `desurvey::test_random_two_kilometre_holes_match_the_textbook_form`).
- **A station on the trace changes nothing:** a station added at a depth with the trace's own direction there moves no point by more than 10 nm, and the order of the input stations does not matter (`desurvey::test_a_station_added_on_the_trace_moves_nothing_and_input_order_does_not_matter`).
- **Compositing conserves (PO-CORE-2):** Σ weight × value over the composites, dropped pieces included, equals the in-domain inputs' total from a separate input-to-domain join to 1e-12 relative, or the set is refused. On 3,200 seeded random sets (200 holes; every residual rule, both weightings, intensive and extensive) the worst was 2.5e-16. Values equal a pair-by-pair recomputation to 5.6e-16, and composites tile every domain with no gap, overlap or boundary crossing. The control, the total without the dropped pieces, is off by 0.10 (`composite::test_seeded_random_holes_conserve_tile_their_domains_and_match_brute_force`).
- **Composites have known values:** hand-worked binary-fraction cases for length and mass weighting, domain contacts, each residual rule, unsampled core (no value, not zero), censored inputs (bounds, no substitution) and extensive quantities (`composite::test_length_weighted_composites_have_their_known_values_and_respect_domains`, `composite::test_each_residual_rule_gives_its_known_pieces`, `composite::test_unsampled_core_mass_weighting_censoring_and_extensive_quantities`).
- **The join is the pair-by-pair answer:** on 320 seeded random cases (970,252 overlapping pairs) of dense, sparse, gridded, nested and identical spans, `overlaps` and `self_overlaps` equal brute force. Spans that only touch do not overlap, and swapping the sides swaps every pair (`join::test_the_sweep_matches_brute_force_on_seeded_random_spans`, `join::test_touching_nested_identical_and_shared_start_spans`). Records on different holes never pair (`join::test_records_join_hole_by_hole`).
- **No assumed straight hole:** a hole with no surveyed station, or declared unsurveyed, is refused. Above the first station and below the last, a measured direction is extended and the point says so (`desurvey::test_beyond_the_stations_the_hole_runs_straight_and_says_so`, `desurvey::test_holes_that_cannot_be_desurveyed_are_refused`).
- Records that cannot be true alone are refused before any write: an interval whose `to` is not past its `from`, an azimuth outside [0, 360), a dip outside [−90, 90], an assay mass fraction outside [0, 1], a "no survey" station with an orientation, a collar with no position or with planned and actual positions in two CRSs.

## 5. Refusals
- `DrillError::Invalid` → `FMINE-REF-109` (a field outside what the record allows; two changes to one record in one batch; a join span whose `to_mm` is not past its `from_mm`; compositing with a zero length, overlapping domains or inputs, bounds that are not finite or ordered, or a missing mass under mass weighting; a hole that cannot be desurveyed: no surveyed station, declared unsurveyed with surveyed stations, a station of another hole, two stations at one depth, consecutive stations in opposite directions).
- `DrillError::CrsMismatch` → `FMINE-REF-106` (planned and actual collar positions in different CRSs).
- `DrillError::Corrupt` → `FMINE-REF-505` (bytes that are not one canonical record version).
- `DrillError::SupportMismatch` → `FMINE-REF-102` (compositing inputs that are neither core samples nor composites).
- `DrillError::NonAdditive` → `FMINE-REF-103` (compositing a non-additive quantity, `INV-014`).
- `DrillError::Conservation` → `FMINE-REF-101` (a composite set that does not conserve its inputs to 1e-12 relative).
- Ledger refusals keep the ledger's codes: `KeyAlreadyExists` → `FMINE-REF-502`, `KeyNotFound` → `FMINE-REF-201`, and the law's violations (`ConservationViolation`) → `FMINE-REF-101`.

## 6. Determinism class
`BitwisePortable` (plan §29.2): integer, byte and ordered-map operations; records compare and store floats as IEEE-754 bits. Desurvey uses IEEE-754 arithmetic and `fmine_core::det` trigonometry and square roots, no std transcendental calls. `join` is `Exact`: integer depths, sorting and comparisons. `composite` uses IEEE-754 addition, multiplication and division with Neumaier compensated sums.

## 7. Evidence class of outputs
Records as imported: layer L0 Evidence (plan §13.2). This crate assigns no evidence class; an interval's logger (human, scanner, model) is kept so that later stages can.

## 8. Validity domain
- Depths along the hole in integer millimetres; positions in a registered CRS with one-sigma accuracies; assay values as mass fractions, censored values kept as bounds.
- Compositing: composites start at the top of each domain run and never cross its base. An input's mass and value are taken as spread evenly along it. Core outside every domain is not composited. A composite of non-uniform length (a residual or merged piece) carries its own length as its support.
- Desurvey: azimuths are bearings from the collar CRS's grid north; magnetic or true north is converted by the importer. One run per depth: in and out runs at the same depth are refused until the caller picks one. The collar's actual position is used, or the planned one when there is no actual (`Trace::collar_from`).
- Records are checked alone and by name. The commit-time law does not check depths against the hole (an interval below the last survey) or interval overlap per logger. `desurvey` and `join::self_overlaps` give the answers; no write is refused on them yet.

## 9. Oracles
- Compositing: hand-worked binary-fraction answers, compared exactly, and a pair-by-pair brute-force recomputation of every composite value on seeded random holes.
- Join: the brute-force pair-by-pair overlap of the two sets, and a hand-worked case of touching, nested, identical and shared-start spans.
- Desurvey: exact positions of holes made of circular arcs and straight runs (minimum curvature is exact on an arc), and the textbook inclination-and-azimuth form (as in Bourgoyne et al., *Applied Drilling Engineering*, SPE, 1986) written in the test with std trigonometry and an arc-cosine dogleg.
- Hand-built records and frames in the unit tests, including frames written with the record writer but holding values the checks refuse, and a `-0.0` frame resealed with a valid CRC32C.
- The ledger's own refusals (`fmine-ledger`), observed on both rungs.

## 10. Known gaps
- No importer writes these records yet (acQuire dialect, `BD-188`).
- MNPI embargo state on results is not in the record (`BD-170`); a later frame version adds it.
- Certificate originals are named by SHA-256 only; storing and checking them in CAS is `BD-171`. `OB-LAB` (dispatched → received or declared lost) is `BD-172`.
- `Structure`, `Density`, `GeometTest`, `GeotechLog` and `TwinPair` (plan §12.1) are not here yet (`BD-285` for twins).
- Desurvey gives points without the survey-error propagation (ISCWSA-style positional covariance) that plan §12.2 asks for (`bd-zx57`). The declared straight-line fallback for unsurveyed short holes needs a planned collar orientation, which `Collar` does not carry.
- No lab or vendor desurvey output is in the oracles yet (PO-CORE-1 names a lab oracle).
- Compositing takes spans and values, not records: grouping a hole's samples and results into inputs, and coding intervals to domains from logging or wireframes, are left to the caller. There is no bulk-density weighting beyond a given input mass, and no compositing across holes.

## 11. Public API
- `records::{DrillError, RecordKind, HoleType, PositionMethod, SurveyInstrument, RunDirection, RecoveryMethod, SampleType, AssayMethod, QaqcRegime, BlankStage, DuplicateStage, Position, Collar, Orientation, Survey, Recovery, Alteration, Logger, Interval, CustodyEvent, Sample, Dispatch, AssayValue, CertificateRef, AssayResult, QcKind, QcInsertion, LabCertificate, DrillRecord, RecordVersion, encode_version, decode_version, RECORD_TAG, RECORD_VERSION, MAX_ID_BYTES, MAX_TEXT_BYTES, MAX_LIST_ITEMS}`
- `store::{KEY_PREFIX, record_key, parse_key, Change, StoreError, delta_ops, batch_for, commit, drill_law, StoredVersion, history, current, as_of}`
- `desurvey::{Point, Direction, CollarFrom, TraceStation, Located, Trace, desurvey}`
- `join::{Span, Overlap, HoleOverlap, overlaps, self_overlaps, join_intervals}`
- `composite::{CONSERVATION_TOLERANCE, Bounds, Input, Domain, Weighting, Residual, Rule, Composite, Conservation, CompositeSet, composite}`
- `records::DrillError` gains `SupportMismatch`, `NonAdditive` and `Conservation`.

## 12. Proof obligations
- `BD-163` (supersession tests, `tests/supersession.rs`, both rungs): `supersession::test_corrections_supersede_and_every_version_stays_readable`, `supersession::test_an_edit_in_place_is_refused_and_changes_nothing`, `supersession::test_the_law_refuses_dangling_and_inconsistent_records_and_writes_nothing`, `supersession::test_records_written_one_per_batch_or_together_read_the_same`.
- `BD-166` (known answers and PO-CORE-2, `tests/composite.rs`): `composite::test_length_weighted_composites_have_their_known_values_and_respect_domains`, `composite::test_each_residual_rule_gives_its_known_pieces`, `composite::test_unsampled_core_mass_weighting_censoring_and_extensive_quantities`, `composite::test_seeded_random_holes_conserve_tile_their_domains_and_match_brute_force`, `composite::test_what_cannot_be_composited_is_refused`.
- `BD-165` (matches brute force, `tests/join.rs`): `join::test_the_sweep_matches_brute_force_on_seeded_random_spans`, `join::test_touching_nested_identical_and_shared_start_spans`, `join::test_records_join_hole_by_hole`, `join::test_a_span_that_does_not_end_past_its_start_is_refused`.
- `BD-164` (PO-CORE-1, `tests/desurvey.rs`): `desurvey::test_holes_of_arcs_and_straight_runs_desurvey_to_their_true_positions`, `desurvey::test_random_two_kilometre_holes_match_the_textbook_form`, `desurvey::test_a_station_added_on_the_trace_moves_nothing_and_input_order_does_not_matter`, `desurvey::test_beyond_the_stations_the_hole_runs_straight_and_says_so`, `desurvey::test_a_collar_with_only_a_planned_position_and_suspect_stations_are_reported`, `desurvey::test_holes_that_cannot_be_desurveyed_are_refused`.
- Record checks and codec: `records::tests::every_record_kind_and_variant_round_trips_with_one_encoding`, `records::tests::records_that_cannot_be_true_are_refused`, `records::tests::decoding_refuses_torn_tampered_and_non_canonical_bytes`.
