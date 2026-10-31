"""Independent BD-333 waterfall census, written before reading the new Rust kernel/adapter.

Usage: python3 -I repro/barrick/waterfall_census.py [workspace_root]
Stdlib only; reuses the pre-existing independent XML/format readers in scripts/oracle.
No workbook, candidate output, or oracle table is written. Numbers are public-source inferences,
never causal attribution. The +10% grade control exists only in memory.

Rules: same-quarter YoY subjects present in both releases; current, first-published cells only.
Missing original AISC releases remain named refusals. Gold site counts exclude copper and regions.
Net AISC is authenticated by its gold reconciliation title, attributable-ounce row and matching
same-release summary/physical shares; unprinted shares are unknown, including Hemlo's. Existing
same-subject identity iii/v findings matter only when anchored in the endpoint's own release and
using a selected operand. Later information and unrelated aggregate failures do not taint a site.
Signed additive fallback uses the published TCC subtotal plus signed sustaining adjustments, requiring
identical named frontiers and exact displayed footing; no plug or absent-line imputation. The TCC
subtotal is explicitly an aggregate. Full gross component counts still set the cost rounding radius.
Rounding uses displayed half-steps; AISC numerator half-width is k/2 over numeric component cells.
Interval bars use independent 80-digit Decimal box arithmetic with outward final decimal steps.
These are conservative rounding enclosures, not statistical confidence intervals.
"""

import hashlib
import json
import re
import sys
import tomllib
from collections import Counter, defaultdict
from decimal import Decimal, localcontext
from fractions import Fraction as F
from pathlib import Path

sys.dont_write_bytecode = True
ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/oracle"))
import census2
import formats
import mine_stats_census as ms
import share_census
from xl import load

GRAMS_PER_TROY_OUNCE = F("31.1034768")
FIELDS = {"T": "tonnes processed (thousands)", "g": "processed",
          "R": "recovery rate (percent)", "produced": "production (thousands of ounces)"}


def norm(text):
    return census2.norm(text).lower()


def canonical_subject(name):
    return "Nevada Gold Mines" if name == "Nevada Gold Mines LLC" else name


def fraction(text):
    try:
        return F(text)
    except (ValueError, TypeError, ZeroDivisionError):
        return None


def dec(x):
    return Decimal(x.numerator) / Decimal(x.denominator) if isinstance(x, F) else Decimal(x)


def logmean(a, b):
    return a if a == b else (a - b) / (a.ln() - b.ln())


def encode(obj):
    if isinstance(obj, F):
        return str(obj)
    if isinstance(obj, Decimal):
        return format(obj, ".55g")
    raise TypeError(type(obj).__name__)


def cell(src, sheet, ref, raw, shown, percent=False):
    out = {"at": f"{src['file']}:{sheet}!{ref}", "sha256": src["sha256"], "stored": raw,
           "displayed": shown[1] if shown else None, "value": None, "half": None}
    if raw is None or raw == "":
        out["kind"] = "blank"
    elif shown and shown[0] == "token" and fraction(raw) == 0:
        out.update(kind="number", value=F(0), half=F(1, 2))
    elif shown and shown[0] == "shown":
        text = shown[1]
        is_percent = text.endswith("%")
        if is_percent != percent:
            out["kind"] = "unit_unconfirmed"
        else:
            number = text.rstrip("%")
            places = len(number.partition(".")[2])
            out.update(kind="number", value=F(number) / (100 if percent else 1),
                       half=F(1, 2 * 10 ** places * (100 if percent else 1)))
    elif str(raw).strip() in census2.PLACE:
        out["kind"] = "placeholder"
    else:
        out["kind"] = "unreadable"
    return out


def evidence(src, sheet, ref, text):
    return {"at": f"{src['file']}:{sheet}!{ref}", "sha256": src["sha256"], "text": text}


def printed_share(text):
    hits = re.findall(r"\(\s*([0-9]+(?:\.[0-9]+)?)\s*%\s*\)", text or "")
    return F(hits[0]) / 100 if len(hits) == 1 else None


def mine_endpoints(src, path):
    year, q = int(src["quarter"][:4]), int(src["quarter"][-1])
    rel = ms.Release(str(path), year, q)
    out = {}
    for sheet in rel.ms_sheets:
        block = sheet.ms_blocks.get(f"Q{q}")
        if block is None:
            continue
        for (subject, group), col in block.cols.items():
            if group != "current":
                continue
            subject = canonical_subject(subject)
            if subject in out:
                raise ValueError(f"duplicate current physical column: {src['quarter']} {subject}")
            header_col = next(c for c in range(col, 1, -1) if not sheet.is_empty(block.header_row - 1, c))
            header = sheet.text(block.header_row - 1, header_col)
            header_ref = f"{ms.letters(header_col)}{block.header_row - 1}"
            row = {"subject": subject, "metal": sheet.metal, "share": printed_share(header),
                   "physical_header": evidence(src, sheet.name, header_ref, header), "factors": {}}
            for key, label in FIELDS.items():
                hits = block.labels.get(label, [])
                if len(hits) != 1:
                    row["factors"][key] = {"kind": "row_absent_or_ambiguous", "value": None, "half": None,
                                           "label": label, "hits": hits}
                    continue
                r = hits[0]
                ref = f"{ms.letters(col)}{r}"
                raw = sheet.cells.get((r, col), (None, None))[1]
                row["factors"][key] = cell(src, sheet.name, ref, raw, rel.shown.get((sheet.name, ref)), key == "R")
                row["factors"][key]["label_at"] = evidence(src, sheet.name, f"A{r}", sheet.text(r, 1))
            out[subject] = row
    return rel, out


def aisc_endpoints(src, path):
    quarter = src["quarter"]
    sheet = f"Q{quarter[-1]} {quarter[:4]}"
    sheets = load(str(path))
    if sheet not in census2.visible(str(path)) or sheet not in sheets:
        return {}, [{"release": quarter, "reason": "own_period_aisc_sheet_absent", "sheet": sheet}]
    cells = sheets[sheet]
    shown = {}
    formats.read_displayed(str(path), False, shown)
    labels = {int(ref[1:]): norm(v) for ref, v in cells.items() if re.fullmatch(r"A[0-9]+", ref)}
    def one(*names):
        hits = [r for r, label in labels.items() if label in names]
        if len(hits) != 1:
            raise ValueError((src["file"], sheet, names, hits))
        return hits[0]
    rc = one("cost of sales applicable to gold production")
    rt, ra = one("total cash costs"), one("all-in sustaining costs")
    rp = one("all-in sustaining costs per ounce", "aisc/oz")
    oq = [r for r, label in labels.items() if label.startswith("ounces sold")]
    if len(oq) != 1:
        raise ValueError((src["file"], "ounce rows", oq))
    rq = oq[0]
    hr = max(range(1, rc), key=lambda r: sum(census2.subject(cells.get(f"{census2.colname(c)}{r}", ""))
                                           is not None for c in range(2, 60)))
    b1 = [r for r in sorted(labels) if rc <= r < rt]
    b2 = [r for r in sorted(labels) if rt < r < ra]
    out = {}
    for c in range(2, 60):
        col = census2.colname(c)
        subject = census2.subject(cells.get(f"{col}{hr}", ""))
        if subject is None:
            continue
        def read(r):
            ref = f"{col}{r}"
            return cell(src, sheet, ref, cells.get(ref), shown.get((sheet, ref)))
        components = []
        for block_id, rows in [(1, b1), (2, b2)]:
            for r in rows:
                part = read(r)
                part.update(line=f"block{block_id}:{labels[r]}", label_at=evidence(src, sheet, f"A{r}", cells[f"A{r}"]))
                components.append(part)
        tcc = read(rt)
        tcc.update(line="total cash costs (reported aggregate)", label_at=evidence(src, sheet, f"A{rt}", cells[f"A{rt}"]))
        out[subject] = {"C": read(ra), "Q": read(rq), "P": read(rp), "components": components,
                        "frontier": [tcc] + [x for x in components if x["line"].startswith("block2:")],
                        "header": evidence(src, sheet, f"{col}{hr}", cells[f"{col}{hr}"]),
                        "title": evidence(src, sheet, "A1", cells.get("A1")),
                        "ounces_label": evidence(src, sheet, f"A{rq}", cells[f"A{rq}"]),
                        "table": (src["file"], sheet, col), "rows": {"b1": b1, "b2": b2, "C": ra, "Q": rq, "TCC": rt}}
    return out, []


def summary_share(src, rel, subject):
    sheet = rel.sheets["QTD"]
    found = []
    for (r, c), (kind, text) in sheet.cells.items():
        if c != 1 or kind not in ms.TEXT_TYPES or text is None:
            continue
        name, _ = ms.subject_of(text)
        if name and canonical_subject(name) == subject:
            found.append((printed_share(text), evidence(src, "QTD", f"A{r}", text)))
    return found


def basis_issue(endpoint, registry):
    a, physical = endpoint.get("aisc"), endpoint.get("physical")
    if a is None:
        return "first_published_aisc_absent"
    if physical is None:
        return "physical_site_column_absent"
    if "reconciliation of gold cost of sales" not in norm(a["title"]["text"]):
        return "gold_reconciliation_title_unconfirmed"
    if "attributable basis (000s ounces)" not in norm(a["ounces_label"]["text"]):
        return "attributable_ounce_basis_unconfirmed"
    share = physical["share"]
    if share is None:
        return "physical_share_unprinted"
    if not 0 < share <= 1:
        return "physical_share_outside_domain"
    summary = endpoint["summary_share"]
    if len(summary) != 1 or summary[0][0] != share:
        return "summary_and_physical_share_not_confirmed"
    registered = [s for s in registry.get("share", []) if s["subject"] == endpoint["subject"]]
    if registered and F(registered[0]["num"], registered[0]["den"]) != share:
        return "registered_share_mismatch"
    return None


def exact_values(endpoint):
    a = endpoint["aisc"]
    return {key: a[key]["value"] for key in ("C", "Q", "P")}


def component_value(part):
    if part["kind"] == "number":
        return part["value"]
    if part["kind"] == "placeholder" and part["stored"].strip() in ("-", "–", "—"):
        return F(0)  # Published dash in an existing reconciliation row, never an absent line.
    return None


def components_foot(endpoint, frontier="frontier"):
    a = endpoint["aisc"]
    values = [component_value(c) for c in a[frontier]]
    return all(v is not None for v in values) and sum(values) == a["C"]["value"]


def fallback(e0, e1):
    lines = [{c["line"]: c for c in e["aisc"]["frontier"]} for e in (e0, e1)]
    if any(len(x) != len(e["aisc"]["frontier"]) for x, e in zip(lines, (e0, e1))):
        return None, "duplicate_frontier_line"
    if lines[0].keys() != lines[1].keys():
        return None, "frontier_line_sets_differ"
    if not all(components_foot(e) for e in (e0, e1)):
        return None, "component_midpoints_do_not_foot"
    p0, p1 = exact_values(e0), exact_values(e1)
    w0, w1 = 1 / p0["Q"], 1 / p1["Q"]
    bars = {k: 1000 * (component_value(lines[1][k]) - component_value(lines[0][k])) * (w0 + w1) / 2
            for k in sorted(lines[0])}
    volume = 1000 * (p0["C"] + p1["C"]) / 2 * (w1 - w0)
    total = sum(bars.values()) + volume
    assert total == 1000 * (p1["C"] / p1["Q"] - p0["C"] / p0["Q"])
    old_sets = [{c["line"] for c in e["aisc"]["components"]} for e in (e0, e1)]
    return {"frontier": "reported TCC aggregate plus signed block2 adjustments", "line_bars": bars,
            "volume_effect": volume, "sum": total, "closure_exact": True,
            "prior_full_component_line_sets_match": old_sets[0] == old_sets[1],
            "prior_full_component_sums_match": all(components_foot(e, "components") for e in (e0, e1))}, None


def lmdi(p0, p1):
    a0, a1 = 1000 * p0["C"] / p0["Q"], 1000 * p1["C"] / p1["Q"]
    weight = logmean(dec(a1), dec(a0))
    def factors(p):
        b = GRAMS_PER_TROY_OUNCE * p["produced"] / (p["T"] * p["g"] * p["R"])
        return {"C": p["C"], "T": p["T"], "g": p["g"], "R": p["R"], "B": b, "S": p["Q"] / p["produced"]}
    x0, x1 = factors(p0), factors(p1)
    bars = {k: weight * dec(x1[k] / x0[k]).ln() * (1 if k == "C" else -1) for k in x0}
    return {"weight": weight, "bars": bars, "sum": sum(bars.values()),
            "closure_error": sum(bars.values()) - dec(a1 - a0), "B": [x0["B"], x1["B"]],
            "S": [x0["S"], x1["S"]]}


def interval_bars(e0, e1):
    def divide(x, y):
        return (x[0] / y[1], x[1] / y[0])
    def multiply(x, y):
        vals = [a * b for a in x for b in y]
        return min(vals), max(vals)
    def boxes(e):
        a = e["aisc"]
        c_half = F(sum(c["kind"] == "number" for c in a["components"]), 2)
        inputs = {"C": (a["C"]["value"], c_half), "Q": (a["Q"]["value"], a["Q"]["half"])}
        inputs.update({k: (v["value"], v["half"]) for k, v in e["physical"]["factors"].items()})
        z = {k: (v - h, v + h) for k, (v, h) in inputs.items()}
        if any(lo <= 0 for lo, hi in z.values()):
            return None
        z["A"] = tuple(1000 * v for v in divide(z["C"], z["Q"]))
        z["B"] = tuple(GRAMS_PER_TROY_OUNCE * v for v in divide(z["produced"], multiply(multiply(z["T"], z["g"]), z["R"])))
        z["S"] = divide(z["Q"], z["produced"])
        return z
    z0, z1 = boxes(e0), boxes(e1)
    if z0 is None or z1 is None:
        return None
    weight = (logmean(dec(z1["A"][0]), dec(z0["A"][0])).next_minus(),
              logmean(dec(z1["A"][1]), dec(z0["A"][1])).next_plus())
    out = {}
    for k in ("C", "T", "g", "R", "B", "S"):
        ratio = divide(z1[k], z0[k])
        logs = (dec(ratio[0]).ln().next_minus(), dec(ratio[1]).ln().next_plus())
        lo, hi = multiply(weight, logs)
        if k != "C":
            lo, hi = -hi, -lo
        out[k] = {"lower": lo.next_minus(), "upper": hi.next_plus(), "sign_not_determined": lo <= 0 <= hi}
    return out


def main():
    registry = tomllib.loads((ROOT / "registries/sources/barrick.toml").read_text())
    sources, releases, physical, aisc, issues = {}, {}, {}, {}, []
    for src in registry["source"]:
        path = ROOT / registry["corpus_dir"] / src["file"]
        blob = path.read_bytes()
        if len(blob) != src["bytes"] or hashlib.sha256(blob).hexdigest() != src["sha256"]:
            raise ValueError(f"registered bytes/hash mismatch: {src['file']}")
        sources[(src["quarter"], src["kind"])] = src
        if src["kind"] == "mine_stats":
            releases[src["quarter"]], physical[src["quarter"]] = mine_endpoints(src, path)
        elif src["kind"] == "aisc_reconciliation":
            aisc[src["quarter"]], found = aisc_endpoints(src, path)
            issues.extend(found)
    _, ledger_rows = census2.run(str(ROOT), displayed=True)
    share_decisions = {}
    for _, row in share_census.run(str(ROOT)):
        col = re.fullmatch(r"([A-Z]+)[0-9]+", row["cell"])[1]
        share_decisions[(row["file"], row["sheet"], col, row["subject"], "COS/oz")] = row
    aisc_bad = defaultdict(list)
    for file, sheet, col, subject, family, state, detail in ledger_rows:
        corrected = share_decisions.get((file, sheet, col, subject, family))
        if corrected is not None:
            state, detail = corrected["state"], corrected
        if state in ("iii", "v") and family != "column":
            aisc_bad[(file, sheet, col, subject)].append({"family": family, "state": state, "reason": detail})
    ms_releases = {(rel.year, rel.q): rel for rel in releases.values()}
    site_identities, _ = ms.site_sheet_instances(ms_releases)
    instances = (ms.ownership_instances(ms_releases) + ms.quarter_sum_instances(ms_releases) + site_identities
                 + ms.sqs_instances(ms_releases, ms.Registry(str(ROOT / "registries/sources/barrick.toml"))))
    ms_bad = defaultdict(list)
    for item in instances:
        if item["state"] in ("iii", "v"):
            quarter = item["anchor"].split(":", 1)[0]
            ms_bad[(quarter, canonical_subject(item["subject"]))].append(item)
    subjects = {q: set(physical[q]) | set(aisc.get(q, {})) for q in releases}
    region_names = {s["name"] for s in registry["subject"] if s.get("kind") == "region"}
    pairs = []
    for q1 in sorted(releases):
        q0 = str(int(q1[:4]) - 1) + q1[4:]
        if q0 not in releases:
            continue
        for subject in sorted(subjects[q0] & subjects[q1]):
            pair = {"subject": subject, "from": q0, "to": q1, "kind": None, "reasons": []}
            if subject in region_names or subject in ("Total Gold", "Total Copper"):
                pair.update(scope="aggregate", kind="excluded", reasons=["aggregate_scope"])
                pairs.append(pair)
                continue
            if any(physical[q].get(subject, {}).get("metal") == "cu" for q in (q0, q1)):
                pair.update(scope="copper", kind="excluded", reasons=["copper_scope"])
                pairs.append(pair)
                continue
            pair["scope"] = "gold_site"
            endpoints = []
            for q in (q0, q1):
                e = {"subject": subject, "release": q, "physical": physical[q].get(subject), "aisc": aisc.get(q, {}).get(subject),
                     "summary_share": summary_share(sources[(q, "mine_stats")], releases[q], subject)}
                e["basis_issue"] = basis_issue(e, registry)
                e["consistency_issues"] = []
                if e["aisc"]:
                    e["consistency_issues"].extend(aisc_bad[(*e["aisc"]["table"], subject)])
                selected = {v["at"].split(":", 1)[1] for v in (e["physical"] or {}).get("factors", {}).values() if "at" in v}
                for item in ms_bad[(q, subject)]:
                    touched = [c for c in item["cells"] if c and c.split(":", 1)[1] in selected]
                    if touched:
                        e["consistency_issues"].append({"family": item["family"], "state": item["state"],
                                                         "reason": item["reason"], "anchor": item["anchor"], "operands": touched})
                endpoints.append(e)
            pair["endpoints"] = endpoints
            shares = [(e["physical"] or {}).get("share") for e in endpoints]
            if all(s is not None for s in shares) and shares[0] != shares[1]:
                pair["reasons"].append("ownership_basis_changes_between_periods")
            if all(e["aisc"] and all(e["aisc"][k]["value"] is not None for k in ("C", "Q", "P"))
                   and e["aisc"]["Q"]["value"] > 0 for e in endpoints):
                p0, p1 = [exact_values(e) for e in endpoints]
                a0, a1 = 1000 * p0["C"] / p0["Q"], 1000 * p1["C"] / p1["Q"]
                pair["rates"] = {"computed": [a0, a1], "printed": [p0["P"], p1["P"]],
                                 "delta": a1 - a0, "endpoint_gaps": [p0["P"] - a0, p1["P"] - a1],
                                 "gap": (p1["P"] - p0["P"]) - (a1 - a0)}
            for e in endpoints:
                if e["basis_issue"]:
                    pair["reasons"].append(f"{e['release']}:{e['basis_issue']}")
                pair["reasons"].extend(f"{e['release']}:{i['family']}:{i['state']}" for i in e["consistency_issues"])
                if e["aisc"] and any(e["aisc"][k]["value"] is None for k in ("C", "Q", "P")):
                    pair["reasons"].append(f"{e['release']}:cost_ounces_or_printed_rate_not_published")
                elif e["aisc"] and e["aisc"]["Q"]["value"] <= 0:
                    pair["reasons"].append(f"{e['release']}:nonpositive_ounces_sold")
            missing = [f"{e['release']}:{k}:{v['kind']}:{v.get('displayed') or v.get('stored')}"
                       for e in endpoints for k, v in (e["physical"] or {}).get("factors", {}).items()
                       if v["value"] is None or v["value"] <= 0 or (k == "R" and v["value"] > 1)]
            pair["physical_factor_issues"] = missing
            if pair["reasons"]:
                pair["kind"] = "refused"
            elif missing or any(e["aisc"]["C"]["value"] <= 0 for e in endpoints):
                bridge, why = fallback(*endpoints)
                pair["kind"] = "additive_fallback" if bridge else "refused"
                pair["reasons"] = missing or ["nonpositive_cost"]
                if bridge:
                    pair["fallback"] = bridge
                else:
                    pair["reasons"].append(why)
            else:
                points = [{**exact_values(e), **{k: v["value"] for k, v in e["physical"]["factors"].items()}} for e in endpoints]
                pair["kind"] = "lmdi"
                pair["lmdi"] = lmdi(*points)
                pair["intervals"] = interval_bars(*endpoints)
                if pair["intervals"] is None:
                    bridge, why = fallback(*endpoints)
                    pair["kind"] = "additive_fallback" if bridge else "refused"
                    pair["reasons"] = ["rounding_box_not_strictly_positive"]
                    if bridge:
                        pair["fallback"] = bridge
                    else:
                        pair["reasons"].append(why)
                else:
                    assert all(pair["intervals"][k]["lower"] <= v <= pair["intervals"][k]["upper"]
                               for k, v in pair["lmdi"]["bars"].items())
                if subject == "North Mara" and q0 == "2025Q2" and q1 == "2026Q2":
                    assert pair["rates"]["delta"] == F(45640, 37) and pair["rates"]["gap"] == F(1905, 37)
                    changed = dict(points[1], g=points[1]["g"] * F(11, 10))
                    control = lmdi(points[0], changed)
                    pair["grade_control"] = {"multiplier": "11/10", "grade_shift": control["bars"]["g"] - pair["lmdi"]["bars"]["g"],
                                             "balance_shift": control["bars"]["B"] - pair["lmdi"]["bars"]["B"],
                                             "closure_error": control["closure_error"], "non_identifying": True}
            pairs.append(pair)
    gold = [p for p in pairs if p["scope"] == "gold_site"]
    eligible = [p for p in gold if p["kind"] == "lmdi"]
    summary = {"gold_site_pairs": len(gold), "gold_by_kind": dict(sorted(Counter(p["kind"] for p in gold).items())),
               "gold_pairs_with_both_first_published_aisc": sum(all(e["aisc"] for e in p["endpoints"]) for p in gold),
               "excluded_by_scope": dict(sorted(Counter(p["scope"] for p in pairs if p["scope"] != "gold_site").items())),
               "lmdi_bars": 6 * len(eligible),
               "sign_not_determined": sum(v["sign_not_determined"] for p in eligible for v in p["intervals"].values())}
    summary["excluded_by_subject"] = dict(sorted(Counter(p["subject"] for p in pairs if p["scope"] != "gold_site").items()))
    summary["fallback_prior_full_line_sets_match"] = sum(p["fallback"]["prior_full_component_line_sets_match"]
                                                        for p in gold if p["kind"] == "additive_fallback")
    report = {"schema": "independent-waterfall-census/1", "descriptive_only": True, "identifying": False,
              "verified_files": [{k: s[k] for k in ("file", "quarter", "kind", "sha256")} for s in registry["source"]],
              "summary": summary, "input_issues": issues,
              "comparison_to_plan": {"historical_nonfactorable": "26/127", "historical_sign_indeterminate": "72/606",
                                     "scope_differences": ["current first-published sources only; no historical comparative substitution",
                                                           "2024Q1 and 2024Q2 have no registered original AISC workbook",
                                                           "copper and aggregates are excluded from gold-site counts",
                                                           "same-release selected-operand consistency and ownership evidence are required",
                                                           "rounding bounds use component-sum k/2 and independent conservative interval arithmetic"],
                                     "historical_row_manifest_available": False},
              "pairs": pairs}
    print(json.dumps(report, default=encode, ensure_ascii=False, indent=2, sort_keys=True))


if __name__ == "__main__":
    with localcontext() as context:
        context.prec = 80
        main()
