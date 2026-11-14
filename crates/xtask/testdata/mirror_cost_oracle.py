"""Invented-source acceptance cases for BD-346.S5; exact expectations use Python Fraction.

Uses the existing authenticated-document fixture seam. No source values or conclusions here
are represented as authentic issuer evidence or as a full public-corpus gate.
"""
import argparse
from fractions import Fraction
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mirror_oracle import Fixture, pair, EQUITY, successful, refused, no_authority, bounds, rat


DEFINITION = (
    ";metal=gold;denominator=sold;allocation=by_product;credits=separate_line;inventory=included;"
    "ga={ga};reclamation=operating_only;accounting=as_reported;definition_page=2;"
    "definition_table=Cost policy;definition_row=Cost definition;definition_col=2025"
)


def cost_rows(kind, token, ga, layer=None):
    rows = pair(f"{kind}_attr_usd_m", token, "attributable", "USD_m")
    for row in rows:
        row.basis += DEFINITION.format(ga=ga)
    if layer:
        field, amount, target = layer
        extra = pair(f"{kind}_{field}_adjustment_attr_usd_m", amount, "attributable", "USD_m")
        for row in extra:
            row.basis += DEFINITION.format(ga=ga) + f";target={target}"
        rows += extra
    return rows


class CostMirror(unittest.TestCase):
    binary = None

    def fixture(self, tag, kind="aisc", bridge=False):
        f = Fixture(self.binary, "cost-" + tag)
        for document, amount, share, ga, layer in (
            (f.left[0], "60", "60", "included", ("ga", "0", "included") if bridge else None),
            (f.right[0], "36" if bridge else "40", "40", "excluded" if bridge else "included", ("ga", "4", "included") if bridge else None),
        ):
            document.readings = pair(EQUITY, share, "equity", "percent") + cost_rows(kind, amount, ga, layer)
        selector = {"aisc": "aisc", "cash_costs": "cash-costs", "cost_of_sales": "cost-of-sales"}[kind]
        return f, f.arguments() + ["--metric", selector]

    def test_same_definition_costs_keep_exact_rounding_sources_and_claim_metadata(self):
        for kind in ("aisc", "cash_costs", "cost_of_sales"):
            with self.subTest(kind=kind):
                f, args = self.fixture(kind, kind)
                f.write()
                report = successful(f.invoke(args))
                p = report["payload"]
                self.assertEqual(p["schema"], "fmine.jv_cost_mirror.v1")
                self.assertEqual(p["verdict"]["status"], "consistent")
                self.assertEqual(bounds(p["common"]["value"]["interval"]), (Fraction(595, 6), Fraction(605, 6)))
                self.assertEqual(p["common"]["value"]["definition"]["denominator"], "sold")
                self.assertEqual(p["common"]["value"]["metric"], kind)
                self.assertFalse(p["gate_eligible"])
                source_ids = {c["claim_id"] for c in p["inputs"]}
                for aligned in p["aligned"]:
                    self.assertTrue(set(aligned["claim"]["parents"]) <= source_ids)
                    self.assertIn(aligned["claim"]["claim_id"], p["common"]["parents"])
                for source in p["inputs"]:
                    self.assertTrue(source["canonical_body_hex"])
                    self.assertFalse(source["stale"])
                    self.assertFalse(source["calibration_failed"])
                    self.assertIn("Cost policy", str(source["anchors"]))
                    self.assertIn(source["value"]["source"]["sha256"], str(p["sources"]))
                    self.assertEqual(source["value"]["readers"], ["reader_a", "reader_b"])
                    self.assertIn(source["value"]["raw_token"], ("60", "40"))
                self.assertEqual(report.raw, f.invoke(args).raw)
                no_authority(report)
                text = f.markdown(args)
                self.assertIn("USD-million costs", text)
                self.assertIn("Left.pdf", text)
                self.assertIn("Right.pdf", text)
                self.assertIn("reader_a, reader_b", text)
                self.assertIn(p["payload_digest"], text)

    def test_two_sided_bridge_and_negative_adjustments_preserve_exact_images(self):
        f, args = self.fixture("bridge", bridge=True)
        f.write()
        p = successful(f.invoke(args))["payload"]
        self.assertEqual(p["verdict"]["status"], "explained")
        self.assertEqual(p["layers"], ["ga"])
        self.assertEqual(bounds(p["common"]["value"]["interval"]), (Fraction(295, 3), Fraction(305, 3)))
        self.assertEqual(len(p["inputs"]), 4)
        self.assertTrue(all(len(x["claim"]["parents"]) == 2 for x in p["aligned"]))
        f.left[0].readings = pair(EQUITY, "60", "equity", "percent") + cost_rows("aisc", "60", "included", ("ga", "-6", "excluded"))
        f.right[0].readings = pair(EQUITY, "40", "equity", "percent") + cost_rows("aisc", "36", "excluded", ("ga", "0", "excluded"))
        f.write()
        signed = successful(f.invoke(args))["payload"]
        self.assertEqual(bounds(signed["common"]["value"]["interval"]), (Fraction(265, 3), Fraction(275, 3)))
        self.assertNotEqual(p["payload_digest"], signed["payload_digest"])
        self.assertTrue(any(bounds(c["value"]["cost"]["interval"])[1] < 0 for c in signed["inputs"] if c["value"]["layer"]))

    def test_conflict_is_an_exact_separation_not_a_selected_reporter(self):
        f, args = self.fixture("conflict")
        f.right[0].token("aisc_attr_usd_m", "80")
        f.write()
        p = successful(f.invoke(args))["payload"]
        self.assertEqual(p["verdict"]["status"], "conflict")
        self.assertIsNone(p["common"])
        proof = p["verdict"]["certificate"]
        self.assertEqual(proof["kind"], "exact_interval_separation")
        self.assertEqual(rat(proof["lower"]), Fraction(795, 4))
        self.assertEqual(rat(proof["upper"]), Fraction(605, 6))
        self.assertGreater(rat(proof["lower"]), rat(proof["upper"]))

    def test_missing_or_incompatible_basis_and_source_controls_refuse(self):
        cases = (("layer", "FMINE-REF-104"), ("definition", "FMINE-REF-104"),
                 ("unit", "FMINE-REF-105"), ("ownership", "FMINE-REF-105"),
                 ("perimeter", "FMINE-REF-102"), ("accounting", "FMINE-REF-104"),
                 ("future", "FMINE-REF-108"), ("missing", "FMINE-REF-506"),
                 ("corrupt", "FMINE-REF-505"), ("reader", "FMINE-REF-202"),
                 ("variant", "FMINE-REF-104"), ("denominator", "FMINE-REF-104"),
                 ("metal", "FMINE-REF-104"))
        for case, code in cases:
            with self.subTest(case=case):
                f, args = self.fixture(case, bridge=case == "layer")
                d = f.right[0]
                if case == "layer":
                    d.readings = [r for r in d.readings if "adjustment" not in r.quantity]
                elif case == "definition":
                    for r in d.readings:
                        r.basis = r.basis.replace(";credits=separate_line", "")
                elif case == "unit":
                    for r in d.readings:
                        if r.quantity.endswith("aisc_attr_usd_m"): r.unit = "USD_per_oz"
                elif case == "ownership": d.token(EQUITY, "0")
                elif case == "perimeter": d.change_basis("members=mine-a+mine-b", "members=mine-a")
                elif case == "accounting":
                    d.readings = pair(EQUITY, "40", "equity", "percent") + cost_rows("aisc", "40", "included", ("accounting", "0", "historical_cost"))
                elif case == "future": d.published_at = "2026-02-02"
                elif case == "missing": d.write_bytes = False
                elif case == "reader": d.readings[-1].token = "999"
                elif case == "variant":
                    extra = pair("aisc_100_usd_m", "100", "100%", "USD_m")
                    for r in extra: r.basis += DEFINITION.format(ga="included")
                    d.readings += extra
                elif case == "denominator":
                    for r in d.readings: r.basis = r.basis.replace("denominator=sold", "denominator=produced")
                elif case == "metal":
                    for r in d.readings: r.basis = r.basis.replace("metal=gold", "metal=copper")
                f.write()
                if case == "corrupt": (f.root / "corpus/beta/Right.pdf").write_bytes(b"changed bytes")
                refused(f.invoke(args), code)

    def test_publication_metadata_changes_receipt_without_changing_costs(self):
        f, args = self.fixture("metadata")
        f.write()
        a = successful(f.invoke(args))["payload"]
        f.right[0].published_at = "2026-01-16"
        f.write()
        b = successful(f.invoke(args))["payload"]
        self.assertNotEqual(a["payload_digest"], b["payload_digest"])
        self.assertEqual(a["common"]["value"]["interval"], b["common"]["value"]["interval"])
        self.assertNotEqual(a["common"]["claim_id"], b["common"]["claim_id"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    args, rest = parser.parse_known_args()
    CostMirror.binary = args.binary.resolve()
    unittest.main(argv=[sys.argv[0], *rest])
