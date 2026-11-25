# FACT: The input is the independent openpyxl read from mine_stats_extract3.py (W1).
# Usage: python3 -B scripts/oracle/mine_stats_vintage_census.py PROFILE WORKBOOK_DIR EXTRACT ASSURANCE
# EXTRACT must come from the same registered Mine Stats workbooks as ASSURANCE.
"""Check the first-to-later Mine Stats ledger by exact source pair and disposition."""

import argparse
import json
import tomllib
from collections import Counter, defaultdict
from decimal import Decimal
from pathlib import Path

from formats import read_displayed


def read_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise SystemExit(f"{path}: {error}") from error


def release(row):
    return f"{row['fy']}Q{row['fq']}"


def locator(row):
    return f"Barrick_{row['file']}_Mine_Stats.xlsx:{row['sheet']}!{row['cell']}"


def active(row, quarter):
    return row["first"] <= quarter <= row.get("last", quarter)


def series_key(row, aliases):
    metric = row["field"] or row["metric"]
    for alias in aliases:
        if alias["label"] == metric and active(alias, release(row)):
            metric = alias["reads_as"]
            break
    section = (row["section"] or "").casefold()
    if section == "costs per ounce ($/oz)":
        # W1: this label gains a redundant unit when Pueblo Viejo moves sheets.
        section = "costs per ounce"
    return row["view"], row["period"], row["site"], section, metric.casefold()


def period_bounds(period):
    year, suffix = period[:4], period[4:]
    if suffix in ("H1", "9M", "FY"):
        return year + "Q1", year + {"H1": "Q2", "9M": "Q3", "FY": "Q4"}[suffix]
    return period, period


def numeric_status(row, statuses):
    first, last = period_bounds(row["period"])
    month = {"Q1": "01", "Q2": "04", "Q3": "07", "Q4": "10"}[first[4:]]
    start_day = f"{first[:4]}-{month}-01"
    for status in statuses:
        if status["subject"] != row["site"]:
            continue
        if status["status"] == "disposed" and start_day > status["closed"]:
            return "disposed"
        if status["status"] == "withheld" and active(status, first) and active(status, last):
            return "withheld"
    return None


def assertion_value(row, displayed, statuses):
    raw = row["value"]
    if raw is None:
        return "blank", None
    if isinstance(raw, str):
        return ("nil" if raw.strip() == "—" else raw.strip()), raw
    if raw == 0 and numeric_status(row, statuses):
        return "nil", str(raw)
    kind, text = displayed[locator(row)]
    if kind == "token" and raw == 0 and text.strip() in ("-", "–", "—"):
        return "number", Decimal(0)
    if kind != "shown":
        return "display_refused", None
    if text.endswith("%"):
        return "number", Decimal(text[:-1]).scaleb(-2)
    return "number", Decimal(text)


def disposition(first, later, values, members, corrections):
    for row in (first, later):
        if "_prod_100_" not in (row["field"] or "") and any(
            f"({share}%)" in row["site_raw"] for share in corrections[row["site"]]
        ):
            return "iii", None, "registered_basis_correction", False
        kind, _ = values[locator(row)]
        if kind == "display_refused":
            return None, "display_refused", "display_refused", False
    site = first["site"]
    if site in members and members[site][release(first)] != members[site][release(later)]:
        return None, "region_map", "region_map", False
    a, b = values[locator(first)], values[locator(later)]
    if a[0] == b[0] == "number":
        return ("i", None, None, False) if a == b else ("ii", None, "value", False)
    if a[0] != b[0]:
        return None, "non_numeric_comparison", "token", False
    return None, "non_numeric_comparison", None, a != b


def expected_pairs(rows, profile, displayed):
    groups = defaultdict(dict)
    for row in rows:
        key = series_key(row, profile.get("mine_stats_label_alias", []))
        group = groups[key]
        if release(row) in group:
            raise ValueError(f"ambiguous oracle series {key} in {release(row)}")
        group[release(row)] = row
    regions = (r["name"] for r in profile["subject"] if r.get("kind") == "region")
    quarters = {release(row) for row in rows}
    members = {
        region: {
            q: {r["member"] for r in profile["region_member"] if r["region"] == region and active(r, q)}
            for q in quarters
        }
        for region in regions
    }
    corrections = defaultdict(set)
    for row in profile.get("printed_share_correction", []):
        corrections[row["subject"]].add(row["printed"])
    values = {
        locator(row): assertion_value(row, displayed, profile["subject_status"])
        for row in rows
    }
    pairs = {}
    for group in groups.values():
        first, *later = (group[q] for q in sorted(group))
        for row in later:
            pair = locator(first), locator(row)
            if pair in pairs:
                raise ValueError(f"duplicate oracle pairing {pair}")
            pairs[pair] = disposition(first, row, values, members, corrections)
    return pairs


def receipt_pairs(ledger, sources):
    pairs = {}
    for entry in ledger["entries"]:
        first, = entry["first"]
        later, = entry["current"]
        for ref in (first, later):
            source = sources[ref["at"].split(":", 1)[0]]
            if (ref["sha256"], ref["release"]) != source or not ref["claim_id"]:
                raise ValueError(f"invalid citation {ref}")
        pair = first["at"], later["at"]
        if pair in pairs:
            raise ValueError(f"duplicate receipt pair {pair}")
        state, exclusion = entry["state"], entry["exclusion"]
        comparison = entry["comparison"]
        reason = comparison["reason"]
        changed = ("value", "token")
        comparison_states = {
            None: "unchanged",
            "display_refused": "not_comparable",
            "region_map": "not_comparable",
            "registered_basis_correction": "conflict",
        }
        comparison_state = "restated_silent" if reason in changed else comparison_states[reason]
        if comparison["state"] != comparison_state:
            raise ValueError(f"inconsistent comparison reason and state for {pair}")
        if state == "ii" and (entry["explanation"], entry["disclosed"]) != ("reconstructed", False):
            raise ValueError(f"unexpected disclosure state for {pair}")
        if exclusion == "region_map" and not any(
            note["evidence"] == "E461" for note in comparison["disclosures"]
        ):
            raise ValueError(f"missing E461 region evidence for {pair}")
        pairs[pair] = state, exclusion, reason, comparison["token_glyph_changed"]
    return pairs


def check_claims(rows, payload, profile, sources):
    aliases = {row["label"]: row["subject"] for row in profile["share"] if "label" in row}
    claims = payload["public_claims"]["cells"]
    by_locator = {claim["at"]: claim for claim in claims}
    if len(by_locator) != len(claims) or len({claim["claim_id"] for claim in claims}) != len(claims):
        raise ValueError("duplicate Claim locator or identifier")
    mine_claims = {at for at in by_locator if at.split(":", 1)[0] in sources}
    if mine_claims != {locator(row) for row in rows}:
        raise ValueError("Mine Stats Claim locators differ from the independent extraction")
    kinds = Counter()
    for row in rows:
        at = locator(row)
        claim = by_locator[at]
        subject = aliases.get(row["site"], row["site"])
        sha256, vintage = sources[at.split(":", 1)[0]]
        if (claim["subject"], claim["period"], claim["vintage"], claim["sha256"]) != (
            subject, row["period"], vintage, sha256
        ) or not all(claim[key] for key in ("ownership_basis", "scope", "unit", "line")):
            raise ValueError(f"incorrect Claim key for {at}")
        value = claim["value"]
        raw = row["value"]
        status = numeric_status(row, profile["subject_status"])
        if raw is None:
            matches = value["kind"] == "missing"
        elif isinstance(raw, str):
            kind = "nil" if raw.strip() == "—" else "not_applicable"
            matches = value["kind"] == kind and value["token"] == raw
        elif raw == 0 and status:
            matches = value["kind"] == "nil" and value["reason"] == status and Decimal(value["token"]) == 0
        else:
            flag = "after_disposal" if status == "disposed" else status
            matches = value["kind"] == "number" and float(value["stored"]) == raw and value["under_status"] == flag
        if not matches:
            raise ValueError(f"incorrect Claim value for {at}: workbook {raw!r}, receipt {value}")
        kinds[value["kind"]] += 1
    operands = 0
    for instance in payload["instances"]:
        for operand in instance["cited"]["operands"]:
            at = operand["at"]
            if "_AISC.xlsx:" not in at or operand["role"] == "label_check":
                continue
            claim = by_locator.get(at)
            if claim is None or (claim["claim_id"], claim["sha256"]) != (operand["claim_id"], operand["sha256"]):
                raise ValueError(f"missing or inconsistent keyed AISC operand {at}")
            operands += 1
    print(f"mine_stats_claims={len(mine_claims)} aisc_operand_references={operands}")
    print("mine_stats_value_kinds=" + json.dumps(kinds, sort_keys=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("profile", "workbook_dir", "extract", "assurance"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    profile = tomllib.loads(args.profile.read_text(encoding="utf-8"))
    receipt = read_json(args.assurance)
    if receipt["exit_code"] != 0 or receipt["payload"]["issuer"] != "barrick":
        raise SystemExit("expected a successful Barrick assurance receipt")
    payload = receipt["payload"]
    sources = {}
    displayed = {}
    for source in profile["source"]:
        quarter = source["quarter"]
        if source["kind"] != "mine_stats" or (payload["through"] and quarter > payload["through"]):
            continue
        file = source["file"]
        cells = {}
        workbook = args.workbook_dir / file
        result = read_displayed(workbook, False, cells)
        if result["sha256"] != source["sha256"] or workbook.stat().st_size != source["bytes"]:
            raise SystemExit(f"unregistered bytes for {workbook}")
        sources[file] = source["sha256"], quarter
        displayed.update({f"{file}:{sheet}!{cell}": value for (sheet, cell), value in cells.items()})
    rows = read_json(args.extract)
    if not sources or {locator(row).split(":", 1)[0] for row in rows} != set(sources):
        raise SystemExit("independent extraction does not cover the registered file set")
    ledger = payload["cross_vintage_ledger"]["mine_stats"]
    expected = expected_pairs(rows, profile, displayed)
    actual = receipt_pairs(ledger, sources)
    missing = expected.keys() - actual.keys()
    extra = actual.keys() - expected.keys()
    mismatches = {pair for pair in expected.keys() & actual.keys() if expected[pair] != actual[pair]}
    print(f"workbooks={len(sources)} cells={len(rows)} expected={len(expected)} actual={len(actual)}")
    print(f"missing={len(missing)} extra={len(extra)} mismatched={len(mismatches)}")
    if missing or extra or mismatches:
        for name, pairs in (("missing", missing), ("extra", extra), ("mismatched", mismatches)):
            for pair in sorted(pairs)[:8]:
                print(name, *pair, expected.get(pair), actual.get(pair))
        raise SystemExit(1)
    counts = Counter(state or exclusion for state, exclusion, _, _ in expected.values())
    states = {state: counts[state] for state in ("i", "ii", "iii", "iv", "v")}
    totals = {
        "comparisons": len(expected),
        "instances": sum(states.values()),
        "excluded": len(expected) - sum(states.values()),
        "by_state": states,
        "ii_reconstructed": states["ii"],
        "ii_stated": 0,
    }
    for key, want in totals.items():
        if ledger[key] != want:
            raise SystemExit(f"{key}: expected {want}, got {ledger[key]}")
    for name, count in sorted(counts.items()):
        print(f"{name}={count}")
    print(f"token_changes={sum(reason == 'token' for _, _, reason, _ in expected.values())}")
    print(f"glyph_changes={sum(glyph for _, _, _, glyph in expected.values())}")
    check_claims(rows, payload, profile, sources)


if __name__ == "__main__":
    main()
