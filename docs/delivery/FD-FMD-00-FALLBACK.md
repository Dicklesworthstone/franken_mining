# FD-FMD-00 Fallback Record — `fmine-report` Plain-Markdown Renderer

- **Foundation ID:** `FD-FMD-00` (`franken_markdown`, `frankenmermaid`)
- **Plan authority:** `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` §39 row `BD-123` (`"INT-FMD-001, INT-FMERM-001, or the FD-FMD-00 fallback taken (plain markdown without diagrams; §37.4 firewall, R7-C085, R7-C103)"`) and `franken-stack.lock` (`pin = "OPEN"`, `fallback = "Plain markdown rendering without diagrams."`).
- **Beads:** `bd-123`, `bd-124`
- **Date:** 2026-11-25

## Decision

Because `FD-FMD-00` (`franken_markdown`, `frankenmermaid`) remains `pin = "OPEN"` in `franken-stack.lock`, `crates/fmine-report` takes the explicitly authorized **`FD-FMD-00` plain-Markdown fallback** without diagrams (`renderer_tag = "fd-fmd-00-plain-markdown-v1"`).

## What This Ships

1. Deterministic, zero-external-dependency plain-Markdown report rendering (`U → S0`) in [`crates/fmine-report/src/lib.rs`](../../crates/fmine-report/src/lib.rs).
2. Enforcement of `INV-002` (`ReportAuthorityFlags` with all four flags unconditionally `false` + mandatory `NO_CLAIM_BANNER`), `INV-007` (`Confidentiality` export clearance), `FMINE-REF-507` (`Claim::check_id`), and `§7.2` G-EXEC numeric suppression via `fmine_claim::render::build_fm_row`.
3. Canonicalized anchors, certificates, and section claim rows producing byte-identical UTF-8 Markdown (`LF` endings) and bound `SHA-256` / `BLAKE3` digests (`verify_rendered_report` refuses single-byte tampering with `FMINE-REF-505`).

## Retirement / Upgrade Path

Once upstream `franken_markdown` and `frankenmermaid` pins are locked in `franken-stack.lock` and pass `INT-FMD-001` / `INT-FMERM-001`, diagram and rich-Markdown adapters can be confined inside `fmine-report` behind the same `ReportSpec -> RenderedReport` API without altering the plain-Markdown fallback lane.
