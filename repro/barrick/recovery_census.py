"""Independent BD-335 recovery-gap oracle, written before reading the Rust implementation.

Usage: python3 -I repro/barrick/recovery_census.py [workspace_root] > /outside/repo/result.json
Requires pdftotext and the registered, locally downloaded PDFs/workbooks. Stdlib only; the
existing independent XML/number-format readers supply cells, never a Rust candidate result.
No real historical tokens are embedded here. All registered bytes are verified before reading.

The annual 2013--2022 baseline is PDF page 39 of the Lumwana site visit. Models are constant
implied tails (M-a) and OLS tails against head grade (M-b). Percent is a named unit throughout.
All algebra and input-rounding boxes use Fraction; square roots use an independent integer
grid enclosure. Residual sample SD (n-1) and regression residual standard error (n-2) are separate.
Quarterly reads choose the earliest registered release as a whole, preserving comparative
vintages. OLS projection uses the centred expression to retain intercept/slope dependence.
Enclosures describe input rounding, not statistical confidence or predictive calibration.
Neither model identifies a cause, lost copper, or a recoverable cash flow. The annual/quarterly
support mismatch and absent ore, grind, throughput and mass-pull covariates remain explicit.
"""

import hashlib
import json
import re
import subprocess
import sys
import tomllib
from decimal import Decimal, localcontext
from fractions import Fraction as F
from math import isqrt
from pathlib import Path

sys.dont_write_bytecode = True
ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/oracle"))
import mine_stats_census as ms


class Box:
    """Closed exact rational box; no floating point enters the algebra."""

    def __init__(self, lo, hi=None):
        self.lo, self.hi = F(lo), F(lo if hi is None else hi)
        if self.lo > self.hi:
            raise ValueError("reversed interval")

    def __add__(self, other):
        return Box(self.lo + other.lo, self.hi + other.hi)

    def __sub__(self, other):
        return Box(self.lo - other.hi, self.hi - other.lo)

    def __mul__(self, other):
        corners = [a * b for a in (self.lo, self.hi) for b in (other.lo, other.hi)]
        return Box(min(corners), max(corners))

    def __truediv__(self, other):
        if other.lo <= 0 <= other.hi:
            raise ValueError("denominator box contains zero")
        return self * Box(1 / other.hi, 1 / other.lo)

    def square(self):
        return Box(0 if self.lo <= 0 <= self.hi else min(self.lo ** 2, self.hi ** 2),
                   max(self.lo ** 2, self.hi ** 2))

    def contains(self, point):
        return self.lo <= point <= self.hi


def summed(values):
    out = Box(0)
    for value in values:
        out = out + value
    return out


def decimal(value):
    return Decimal(value.numerator) / Decimal(value.denominator)


def sqrt_box(value):
    """A proven decimal-grid bound, independent of binary/libm square root."""
    if value.lo < 0:
        raise ValueError("negative square-root bound")
    scale = 10 ** 60
    low = isqrt(value.lo.numerator * scale ** 2 // value.lo.denominator)
    high = isqrt(value.hi.numerator * scale ** 2 // value.hi.denominator)
    if F(high, scale) ** 2 != value.hi:
        high += 1
    return Box(F(low, scale), F(high, scale))


def quantity(point, bounds):
    if not bounds.contains(point):
        raise ValueError(f"point outside independent box: {point}")
    return {"point": point, "lo": bounds.lo, "hi": bounds.hi}


def numeric_token(token):
    plain = token.rstrip("%")
    if not re.fullmatch(r"[0-9]+(?:\.[0-9]+)?", plain):
        raise ValueError(f"not a positive decimal token: {token}")
    places = len(plain.partition(".")[2])
    point, half = F(plain), F(1, 2 * 10 ** places)
    return point, Box(point - half, point + half)


def fit(rows):
    n = len(rows)
    if n < 3:
        raise ValueError("at least three annual rows needed")
    h, r = [row[0] for row in rows], [row[1] for row in rows]
    hb, rb = [row[2] for row in rows], [row[3] for row in rows]
    tails = [x * (1 - y / 100) for x, y in zip(h, r)]
    tb = [x * (Box(1) - y / Box(100)) for x, y in zip(hb, rb)]
    mean_h, mean_t = sum(h) / n, sum(tails) / n
    # Point coefficients use centred sums; interval denominator uses the pairwise identity.
    sxx = sum((x - mean_h) ** 2 for x in h)
    sxy = sum((x - mean_h) * (y - mean_t) for x, y in zip(h, tails))
    if sxx <= 0:
        raise ValueError("constant historical grade")
    slope, intercept = sxy / sxx, mean_t - sxy / sxx * mean_h
    mhb, mtb = summed(hb) / Box(n), summed(tb) / Box(n)
    sxxb = summed((hb[i] - hb[j]).square() for i in range(n) for j in range(i)) / Box(n)
    sxyb = summed((hb[i] - hb[j]) * (tb[i] - tb[j]) for i in range(n) for j in range(i)) / Box(n)
    if sxxb.lo <= 0:
        raise ValueError("historical grade variation unresolved by rounding")
    slopeb = sxyb / sxxb
    interceptb = mtb - slopeb * mhb
    residuals = [y - intercept - slope * x for x, y in zip(h, tails)]
    sse = sum(e * e for e in residuals)
    sseb = summed((y - (mtb + slopeb * (x - mhb))).square() for x, y in zip(hb, tb))
    out = {"n": n, "mean_head_percent": quantity(mean_h, mhb),
           "mean_tails_percent": quantity(mean_t, mtb), "intercept_percent": quantity(intercept, interceptb),
           "slope": quantity(slope, slopeb), "sxx": quantity(sxx, sxxb), "sxy": quantity(sxy, sxyb),
           "sse": quantity(sse, sseb), "point_residual_sum": sum(residuals)}
    for name, dof in (("residual_sample_sd", n - 1), ("regression_residual_standard_error", n - 2)):
        variance = sse / dof
        enclosure = sqrt_box(sseb / Box(dof))
        out[name] = {"degrees_of_freedom": dof, "variance": quantity(variance, sseb / Box(dof)),
                     "point_decimal": decimal(variance).sqrt(), "lo": enclosure.lo, "hi": enclosure.hi}
    return out


def interval(value):
    return Box(value["lo"], value["hi"])


def project(model, data):
    h, r, tonnes, copper = (data[name]["point"] for name in ("head", "recovery", "processed", "produced"))
    hb, rb, tonnesb, copperb = (interval(data[name]) for name in ("head", "recovery", "processed", "produced"))
    if hb.lo <= 0 or tonnesb.lo <= 0 or copperb.lo <= 0 or rb.lo < 0 or rb.hi > 100:
        raise ValueError("operating domain not proven by rounding boxes")
    out = {}
    for label in ("M-a", "M-b"):
        t, tb = model["mean_tails_percent"]["point"], interval(model["mean_tails_percent"])
        if label == "M-b":
            t += model["slope"]["point"] * (h - model["mean_head_percent"]["point"])
            tb = tb + interval(model["slope"]) * (hb - interval(model["mean_head_percent"]))
        expected, eb = 100 * (1 - t / h), Box(100) * (Box(1) - tb / hb)
        gap, gb = expected - r, eb - rb
        difference, db = tonnes * h * gap / 10000, tonnesb * hb * gb / Box(10000)
        out[label] = {"expected_recovery_percent": quantity(expected, eb), "gap_pp": quantity(gap, gb),
                      "copper_difference_kt": quantity(difference, db),
                      "fraction_reported_copper": quantity(difference / copper, db / copperb),
                      "gap_sign_indeterminate": gb.contains(0)}
    return out


def verified(root, row, directory):
    path = root / directory / row["file"]
    blob = path.read_bytes()
    if len(blob) != row["bytes"] or hashlib.sha256(blob).hexdigest() != row["sha256"]:
        raise ValueError(f"registered bytes/hash mismatch: {row['file']}")
    return path


def pdf_text(path, page):
    return subprocess.run(["pdftotext", "-f", str(page), "-l", str(page), "-layout", str(path), "-"],
                          check=True, capture_output=True, text=True).stdout


def source(row, sheet, cell, text):
    return {"at": f"{row['file']}:{sheet}!{cell}", "sha256": row["sha256"], "text": text}


def annual_inputs(registry):
    doc = next(d for d in registry["document"] if d["file"] == "Lumwana_Site_Visit_25092025.pdf")
    text = pdf_text(verified(ROOT, doc, doc["dir"]), 39)
    title = "Appendix A: Past Production Records for Lumwana"
    if title not in " ".join(text.split()) or "Head Grade" not in text or "Recovery" not in text:
        raise ValueError("historical table headings not confirmed")
    rows, evidence = [], []
    for year in range(2013, 2023):
        matches = re.findall(rf"^\s*{year}\s+[\d,]+\s+([\d.]+)\s+[\d,]+\s+[\d,]+\s+([\d.]+)\s*$",
                             text, re.MULTILINE)
        if len(matches) != 1:
            raise ValueError(f"historical annual row absent/ambiguous: {year}")
        h, r = matches[0]
        hp, hb = numeric_token(h)
        rp, rb = numeric_token(r)
        rows.append((hp, rp, hb, rb))
        evidence.append({"period": f"{year}FY", "support": "annual", "page": 39, "table": title,
                         "file": doc["file"], "sha256": doc["sha256"], "row": str(year),
                         "Head Grade (% Cu)": h, "Recovery (%)": r,
                         "raw_ownership_label": "unprinted"})
    ownership = next(d for d in registry["document"] if d["file"] == "Barrick_AIF_2025.pdf")
    ownership_text = pdf_text(verified(ROOT, ownership, ownership["dir"]), 38)
    if "Copper Mineral Reserves" not in ownership_text or "Based on attributable tonnes" not in ownership_text:
        raise ValueError("ownership table headings not confirmed")
    labels = re.findall(r"Lumwana\s+surface\s+\((\d+(?:\.\d+)?)%\)", ownership_text)
    if labels != ["100"]:
        raise ValueError("numeric ownership row not confirmed")
    context = {"file": ownership["file"], "sha256": ownership["sha256"], "page": 38, "printed_page": 37,
               "table": "Copper Mineral Reserves", "row": "Lumwana surface (100%)",
               "col": "Based on attributable tonnes", "token": labels[0], "unit": "percent",
               "scope": "separately cited ownership context; raw source basis is not rewritten"}
    return rows, evidence, context


def quarterly_inputs(registry):
    quarters, verified_files = {}, []
    for src in sorted(registry["source"], key=lambda s: (s["quarter"], s["file"])):
        if src["kind"] != "mine_stats":
            continue
        path = verified(ROOT, src, registry["corpus_dir"])
        verified_files.append(src["file"])
        rel = ms.Release(str(path), int(src["quarter"][:4]), int(src["quarter"][-1]))
        selected = []
        for sheet in rel.ms_sheets:
            block = sheet.ms_blocks.get(f"Q{rel.q}")
            if sheet.metal != "cu" or block is None:
                continue
            for group in ("current", "prior"):
                col = block.cols.get(("Lumwana", group))
                if col is not None:
                    selected.append((sheet, block, group, col))
        for sheet, block, group, col in selected:
            year = rel.year - (group == "prior")
            period = f"{year}Q{rel.q}"
            if period in quarters:
                continue
            record = {"period": period, "release": src["quarter"], "file": src["file"],
                      "sha256": src["sha256"], "support": "quarter", "group": group,
                      "vintage": "original current quarter" if group == "current" else "comparative; original unavailable",
                      "raw_ownership_label": "unprinted", "inputs": {}, "context": []}
            header_col = next(c for c in range(col, 1, -1) if not sheet.is_empty(block.header_row - 1, c))
            header = sheet.text(block.header_row - 1, header_col)
            shares = re.findall(r"\(([\d.]+)%\)", header)
            if shares:
                record["raw_ownership_label"] = shares
            record["context"].extend([
                source(src, sheet.name, f"{ms.letters(header_col)}{block.header_row - 1}", header),
                source(src, sheet.name, f"A{block.header_row}", sheet.text(block.header_row, 1)),
                source(src, sheet.name, f"{ms.letters(col)}{block.header_row}", str(year)),
            ])
            fields = {"processed": ("tonnes processed (thousands)", False), "head": ("processed", True),
                      "recovery": ("recovery rate (percent)", True),
                      "produced": ("production (thousands of tonnes)", False)}
            for name, (label, percent) in fields.items():
                hits = block.labels.get(label, [])
                if name == "produced" and not hits:
                    aliases = [a for a in registry.get("mine_stats_label_alias", [])
                               if a["first"] <= src["quarter"] <= a["last"] and a["reads_as"].lower() == label]
                    for alias in aliases:
                        hits += block.labels.get(alias["label"].lower(), [])
                        record["context"].append({"registry_alias": alias})
                    notes = [(row, sheet.text(row, 1)) for row in sheet.rows if row == block.end
                             and "quantities in tonnes rather than pounds" in (sheet.text(row, 1) or "")]
                    if len(notes) != 1:
                        raise ValueError(f"stale production-unit footnote unconfirmed: {period}")
                    row, note = notes[0]
                    record["context"].append(source(src, sheet.name, f"A{row}", note))
                if len(hits) != 1:
                    raise ValueError(f"selected field absent/ambiguous: {period} {name}")
                row, ref = hits[0], f"{ms.letters(col)}{hits[0]}"
                shown = rel.shown.get((sheet.name, ref))
                if not shown or shown[0] != "shown" or shown[1].endswith("%") != percent:
                    raise ValueError(f"unusable selected cell: {period} {name} {shown}")
                point, box = numeric_token(shown[1])
                record["inputs"][name] = {**quantity(point, box), **source(src, sheet.name, ref, shown[1]),
                                          "label": source(src, sheet.name, f"A{row}", sheet.text(row, 1)),
                                          "stored": sheet.cells[(row, col)][1]}
            if any("registry_alias" in context for context in record["context"]):
                summary = rel.sheets["QTD"]
                copper_block = summary.blocks["cu"]
                summary_row = copper_block.rows["Lumwana"][0]
                summary_col = copper_block.cols[(group, "prod_100")]
                ref = f"{ms.letters(summary_col)}{summary_row}"
                displayed = rel.shown[("QTD", ref)]
                if displayed[0] != "shown" or F(displayed[1]) != record["inputs"]["produced"]["point"]:
                    raise ValueError(f"stale unit alias not confirmed by same-release kilotonne summary: {period}")
                record["context"].extend([
                    source(src, "QTD", f"{ms.letters(summary_col)}{copper_block.header_row}",
                           summary.text(copper_block.header_row, summary_col)),
                    source(src, "QTD", ref, displayed[1]),
                ])
            quarters[period] = record
    return [quarters[p] for p in sorted(quarters)], verified_files


def synthetic_known_answer():
    rows = [(F(h), F(r), Box(h), Box(r)) for h, r in ((1, 80), (2, 85), (4, "87.5"))]
    model = fit(rows)
    assert model["mean_tails_percent"]["point"] == F(1, 3)
    assert model["intercept_percent"]["point"] == model["slope"]["point"] == F(1, 10)
    data = {name: quantity(F(value), Box(value)) for name, value in
            (("head", 2), ("recovery", 82), ("processed", 1000), ("produced", F(82, 5)))}
    answer = project(model, data)
    assert answer["M-a"]["gap_pp"]["point"] == F(4, 3)
    assert answer["M-a"]["copper_difference_kt"]["point"] == F(4, 15)
    assert answer["M-b"]["gap_pp"]["point"] == 3
    assert answer["M-b"]["fraction_reported_copper"]["point"] == F(3, 82)
    return {"fit": model, "projection": answer}


def encode(value):
    if isinstance(value, F):
        return str(value)
    if isinstance(value, Decimal):
        return format(value, ".60g")
    raise TypeError(type(value).__name__)


def main():
    registry = tomllib.loads((ROOT / "registries/sources/barrick.toml").read_text())
    with localcontext() as ctx:
        ctx.prec = 80
        synthetic = synthetic_known_answer()
        rows, evidence, ownership = annual_inputs(registry)
        model = fit(rows)
        quarters, files = quarterly_inputs(registry)
        for quarter in quarters:
            quarter["models"] = project(model, quarter["inputs"])
        out = {"schema": "fmine.independent.recovery.v1", "training": evidence, "ownership_context": ownership,
               "fit": model, "quarters": quarters, "verified_mine_stats_files": files,
               "quarter_count": len(quarters), "synthetic_known_answer": synthetic,
               "calibration": "uncalibrated descriptive model; input-rounding enclosures only",
               "unknown_unknowns": ["annual training versus quarterly application", "ore type", "grind size",
                                     "throughput beyond the printed totals", "mass pull", "model misspecification"],
               "causal_identified": False, "cash_or_lost_copper_claim": False}
        print(json.dumps(out, sort_keys=True, indent=2, default=encode))


if __name__ == "__main__":
    main()
