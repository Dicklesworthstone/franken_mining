"""Independent exact known answers for the real cost-worksheet CLI.

All cost amounts, scope labels and unknowns are invented declared assumptions.
Python Fraction checks the source amounts and contribution arithmetic without
using the Rust implementation to generate expected values.
"""

import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest


FILE = "worksheet.toml"
SUBJECT = '矿山 "A" \\ Oeste'
BASIS = 'Prix déclaré € "nominal" \\ FY2030'
UU_DESCRIPTION = 'Unclassified "coût" \\ Ω'
UU_TRIGGER = 'Audit "矿" \\ ledger'
REGISTRY = Path(__file__).resolve().parents[3] / "registries/metrics.toml"
CASES = (
    ("aisc@wgc-2018", ("ounces_sold", "gold_share", "usd_per_oz"),
     "100", None, "full", None, "1000.01", (
        ("on_site_mining_costs", "800.01", "cost", "800.01"),
        ("corporate_general_and_administrative", "300", "cost", "300"),
        ("lease_payments", "100", "cost", "100"),
        ("by_product_credits", "200", "credit", "-200"),
        ("income_tax", "999", "excluded", "0"),
    )),
    ("aic@wgc-2018", ("ounces_sold", "gold_share", "usd_per_oz"),
     "125/2", None, "full", None, "1200.10", (
        ("on_site_mining_costs", "1000.025", "cost", "1000.025"),
        ("capital_expenditure_non_sustaining", "400.1250", "cost", "400.1250"),
        ("by_product_credits", "200.05", "credit", "-200.05"),
        ("income_tax", "999", "excluded", "0"),
    )),
    ("aisc.co_product@wgc-2018", ("ounces_sold", "gold_share", "usd_per_oz"),
     "100", "3/4", "attributable", "2/5", "1050", (
        ("on_site_mining_costs", "1000", "cost", "750"),
        ("corporate_general_and_administrative", "300", "cost", "225"),
        ("lease_payments", "100", "cost", "75"),
        ("by_product_credits", "300", "excluded", "0"),
        ("income_tax", "999", "excluded", "0"),
    )),
    ("c1.copper@barrick-q2-2026", ("pounds_sold", "copper_share", "usd_per_lb"),
     "400", None, "full", None, "980", (
        ("cost_of_sales", "1000", "cost", "1000"),
        ("depreciation_amortization_removed", "200", "deduction", "-200"),
        ("treatment_refining_charges", "100", "cost", "100"),
        ("equity_method_c1_cash_costs", "150", "cost", "150"),
        ("royalties_removed_from_cost_of_sales", "50", "deduction", "-50"),
        ("costs_allocated_to_by_products", "20", "credit", "-20"),
        ("corporate_general_and_administrative", "40", "excluded", "0"),
        ("reclamation_accretion_and_amortisation_operating_sites", "10", "excluded", "0"),
        ("royalties_added", "50", "excluded", "0"),
        ("exploration_and_evaluation_sustaining", "20", "excluded", "0"),
        ("minesite_sustaining_capital_expenditures", "95", "excluded", "0"),
        ("sustaining_leases", "5", "excluded", "0"),
    )),
    ("aisc.copper@barrick-q2-2026", ("pounds_sold", "copper_share", "usd_per_lb"),
     "400", None, "full", None, "1200", (
        ("cost_of_sales", "1000", "cost", "1000"),
        ("depreciation_amortization_removed", "200", "deduction", "-200"),
        ("treatment_refining_charges", "100", "cost", "100"),
        ("equity_method_c1_cash_costs", "150", "cost", "150"),
        ("royalties_removed_from_cost_of_sales", "50", "deduction", "-50"),
        ("costs_allocated_to_by_products", "20", "credit", "-20"),
        ("corporate_general_and_administrative", "40", "cost", "40"),
        ("reclamation_accretion_and_amortisation_operating_sites", "10", "cost", "10"),
        ("royalties_added", "50", "cost", "50"),
        ("exploration_and_evaluation_sustaining", "20", "cost", "20"),
        ("minesite_sustaining_capital_expenditures", "95", "cost", "95"),
        ("sustaining_leases", "5", "cost", "5"),
    )),
)


def fields(values):
    # The fixture needs only the TOML subset's supported quote/backslash escapes.
    return "".join(f"{key} = {json.dumps(value, ensure_ascii=False)}\n"
                   for key, value in values.items())


def fixture(case=CASES[0]):
    _, units, sold, metal_share, ownership, ownership_share, _, lines = case
    sold_key, share_key, _ = units
    root = Path(tempfile.mkdtemp(prefix="fmine-cost-binary-"))
    # Retain fixture files for diagnosis, matching the existing integration suites.
    (root / "registries").mkdir()
    (root / "registries/metrics.toml").write_bytes(REGISTRY.read_bytes())
    header = {
        "schema": "fmine.cost-worksheet.v1", "issuer": "invented", "subject": SUBJECT,
        "period": "2030FY", "ownership": ownership, "basis": BASIS,
        "currency": "USD", "complete": "true", sold_key: sold,
    }
    if metal_share is not None:
        header[share_key] = metal_share
    if ownership_share is not None:
        header["ownership_share"] = ownership_share
    text = fields(header)
    for item, amount, _, _ in lines:
        text += "\n[[cost]]\n" + fields({"item": item, "usd": amount})
    text += "\n[[uu]]\n" + fields({
        "id": "omitted-cost", "description": UU_DESCRIPTION,
        "trigger": UU_TRIGGER, "downside_usd_m": "0.001",
    })
    (root / FILE).write_text(text, encoding="utf-8")
    return root


def arguments(definition):
    return ["metrics", "cost", "--input", FILE, "--definition", definition]


class CostWorksheet(unittest.TestCase):
    binary = None

    def test_registered_definitions_reconcile_exact_amounts_and_preserve_complete_claims(self):
        for case in CASES:
            definition, units, sold, metal_share, ownership, ownership_share, expected_total, lines = case
            sold_key, share_key, rate_key = units
            with self.subTest(definition=definition):
                root = fixture(case)
                output = subprocess.run(
                    [str(self.binary), *arguments(definition), "--workspace", str(root)],
                    capture_output=True, text=True, encoding="utf-8", check=False, timeout=30,
                )
                self.assertEqual(output.returncode, 0, output.stdout + output.stderr)
                envelope = json.loads(output.stdout)
                self.assertEqual(envelope["exit_code"], 0)
                self.assertIsNone(envelope["refusal"])
                self.assertIsNone(envelope["anchor_id"])
                self.assertIsNone(envelope["certificate"])
                self.assertTrue(all(flag is False for flag in envelope["flags"].values()))
                self.assertTrue(envelope["warnings"])
                payload = envelope["payload"]
                self.assertEqual(payload["schema"], "fmine.cost-worksheet.v1")
                self.assertEqual(payload["mode"], "declared_cost_worksheet")
                for name, path in (("source", root / FILE), ("registry", root / "registries/metrics.toml")):
                    data = path.read_bytes()
                    self.assertEqual(payload[name], {
                        "path": str(path), "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data),
                    })
                receipt = payload["receipt"]
                summary = receipt["summary"]
                result = receipt["claim"]
                self.assertEqual(result["value"], summary)
                self.assertEqual(result["value_tag"], receipt["digest"])
                self.assertEqual(len(bytes.fromhex(receipt["digest"])), 32)
                self.assertEqual(summary["definition"], definition)
                self.assertTrue(summary["declared_complete"])
                self.assertEqual(summary["currency"], "USD")
                self.assertEqual(Fraction(summary[sold_key]), Fraction(sold))
                self.assertEqual(None if summary[share_key] is None else Fraction(summary[share_key]),
                                 None if metal_share is None else Fraction(metal_share))
                for key in {"ounces_sold", "gold_share", "usd_per_oz",
                            "pounds_sold", "copper_share", "usd_per_lb"} - set(units):
                    self.assertNotIn(key, summary)
                expected_scope = {
                    "issuer": "invented", "subject": SUBJECT,
                    "period": {"year": 2030, "first_month": 1, "last_month": 12},
                    "ownership": {"kind": ownership}, "basis": BASIS,
                }
                if ownership_share is not None:
                    expected_scope["ownership"]["share"] = ownership_share
                self.assertEqual(summary["scope"], expected_scope)
                inputs = {claim["claim_id"]: claim for claim in receipt["inputs"]}
                self.assertEqual(set(result["parents"]), set(inputs))
                cost_inputs = {claim["value"]["value"]["item"]: claim for claim in inputs.values()
                               if claim["value"]["value"]["kind"] == "cost"}
                contributions = {row["item"]: row for row in summary["contributions"]}
                self.assertEqual(set(cost_inputs), {line[0] for line in lines})
                self.assertEqual(set(contributions), set(cost_inputs))
                for item, amount, treatment, contribution in lines:
                    source = cost_inputs[item]["value"]["value"]
                    row = contributions[item]
                    self.assertEqual(source["currency"], "USD")
                    self.assertEqual(source["scale"], 10000)
                    self.assertEqual(Fraction(int(source["minor"]), source["scale"]), Fraction(amount))
                    self.assertEqual(row["input_claim_id"], cost_inputs[item]["claim_id"])
                    self.assertEqual(row["amount_minor"], source["minor"])
                    self.assertEqual(row["treatment"], treatment)
                    self.assertEqual(Fraction(row["numerator_usd"]), Fraction(contribution))
                summed = sum(Fraction(row["numerator_usd"]) for row in contributions.values())
                self.assertEqual(summed, Fraction(expected_total))
                self.assertEqual(Fraction(summary["numerator_usd"]), summed)
                self.assertEqual(Fraction(summary[rate_key]), summed / Fraction(sold))
                for claim in [*inputs.values(), result]:
                    self.assertTrue(bytes.fromhex(claim["canonical_body_hex"]))
                    for name in ("metric_key", "value_tag", "epistemic", "knowledge", "representation",
                                 "support", "additivity", "parents", "anchors", "supersedes", "stale",
                                 "calibration_failed", "color"):
                        self.assertIn(name, claim)
                    self.assertEqual(claim["evidence"], "Assumed")
                    self.assertEqual(claim["confidentiality"], "Internal")
                    self.assertEqual(claim["knowledge"], "Known")
                    self.assertIn("KnownUnknown", claim["epistemic"])
                    self.assertEqual(claim["representation"], "Deterministic")
                    self.assertEqual(claim["support"], "PeriodAggregate")
                    self.assertFalse(claim["stale"])
                    self.assertFalse(claim["calibration_failed"])
                    self.assertIsNone(claim["supersedes"])
                    self.assertEqual(claim["color"]["kind"], "estimated")
                    self.assertEqual(claim["value"]["scope"], expected_scope)
                    self.assertTrue(any(a["sha256"] == payload["source"]["sha256"] for a in claim["anchors"]))
                    scope_budget = [{
                        "scenario_id": "omitted-cost", "description": UU_DESCRIPTION,
                        "downside_exposure_usd_m": 0.001, "downside_bits": struct.pack(">d", 0.001).hex(),
                        "trigger_indicator": UU_TRIGGER,
                    }]
                    is_rule = claim["value"].get("value", {}).get("kind") == "rule"
                    self.assertEqual(claim["uu_terms"], scope_budget if claim is result or is_rule else [])
                rule = next(claim for claim in inputs.values() if claim["value"]["value"]["kind"] == "rule")
                self.assertTrue(any(a["sha256"] == payload["registry"]["sha256"] for a in rule["anchors"]))
                self.assertTrue(any(a["sha256"] == payload["registry"]["sha256"] for a in result["anchors"]))
                self.assertEqual(rule["value"]["value"]["definition_tag"], summary["definition_tag"])

    def assert_refused_worksheet(self, root, definition, text, code):
        path = root / FILE
        path.write_text(text, encoding="utf-8")
        output = subprocess.run(
            [str(self.binary), *arguments(definition), "--workspace", str(root)],
            capture_output=True, text=True, encoding="utf-8", check=False, timeout=30,
        )
        self.assertNotEqual(output.returncode, 0, output.stdout + output.stderr)
        envelope = json.loads(output.stdout)
        self.assertEqual(envelope["exit_code"], output.returncode)
        self.assertEqual(envelope["refusal"]["code"], code)
        self.assertIsNone(envelope["anchor_id"])
        self.assertIsNone(envelope["certificate"])
        self.assertTrue(all(flag is False for flag in envelope["flags"].values()))
        for key in ("receipt", "usd_per_lb", "usd_per_oz"):
            self.assertNotIn(key, envelope["payload"])
        self.assertEqual(path.read_text(encoding="utf-8"), text)

    def test_sold_units_and_allocation_are_explicit_and_never_mixed(self):
        for definition, other_sold, other_share in (
            ("aisc@wgc-2018", "pounds_sold", "copper_share"),
            ("c1.copper@barrick-q2-2026", "ounces_sold", "gold_share"),
        ):
            case = next(case for case in CASES if case[0] == definition)
            root = fixture(case)
            original = (root / FILE).read_text(encoding="utf-8")
            sold_line = fields({case[1][0]: case[2]})
            mutations = (
                ("missing sold quantity", original.replace(sold_line, ""), "FMINE-REF-506"),
                ("both sold quantities", original.replace(
                    sold_line, sold_line + fields({other_sold: case[2]})), "FMINE-REF-506"),
                ("wrong metal denominator", original.replace(
                    sold_line, fields({other_sold: case[2]})), "FMINE-REF-105"),
                ("wrong metal allocation", original.replace(
                    sold_line, sold_line + fields({other_share: "1/2"})), "FMINE-REF-105"),
            )
            for label, text, code in mutations:
                with self.subTest(definition=definition, defect=label):
                    self.assertNotEqual(text, original, "mutation must change the input")
                    self.assert_refused_worksheet(root, definition, text, code)

    def test_copper_deductions_and_unsupported_adjustments_refuse(self):
        for definition in ("c1.copper@barrick-q2-2026", "aisc.copper@barrick-q2-2026"):
            case = next(case for case in CASES if case[0] == definition)
            root = fixture(case)
            original = (root / FILE).read_text(encoding="utf-8")
            for item, amount in (
                ("depreciation_amortization_removed", "200"),
                ("royalties_removed_from_cost_of_sales", "50"),
            ):
                text = original.replace(
                    fields({"item": item, "usd": amount}),
                    fields({"item": item, "usd": "-" + amount}),
                )
                with self.subTest(definition=definition, negative_deduction=item):
                    self.assertNotEqual(text, original, "mutation must change the input")
                    self.assert_refused_worksheet(root, definition, text, "FMINE-REF-109")
            # This exact table rule cannot invent an inventory adjustment that might already
            # be included in cost of sales.
            text = original + "\n[[cost]]\n" + fields({
                "item": "inventory_write_down_added", "usd": "10",
            })
            with self.subTest(definition=definition, unsupported="inventory_write_down_added"):
                self.assert_refused_worksheet(root, definition, text, "FMINE-REF-105")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--fixture-only", action="store_true")
    options = parser.parse_args()
    CostWorksheet.binary = options.binary.resolve()
    if options.fixture_only:
        print(fixture())
    else:
        unittest.main(argv=[__file__])
