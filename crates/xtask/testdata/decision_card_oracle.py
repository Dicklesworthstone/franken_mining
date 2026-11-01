"""BD-225 known answers against the actual fmine binary, using only the standard library.

All inputs are declared, invented assumptions. GB-29 supplies the two-state arithmetic;
the other fixtures independently check weighted tails, ties and source-bound behavior.
Money expectations use exact Fraction values in major USD, never binary floats.
"""

import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


BINARY = None
FILE = "decision-model.toml"
STATES = [("high", "3/5"), ("low", "2/5")]
ACTIONS = [("build", "Build now", "action"), ("do_nothing", "Do nothing", "do_nothing")]
PAYOFFS = {
    "build": {"high": "300000000", "low": "-200000000"},
    "do_nothing": {"high": "0", "low": "0"},
}


def block(kind, fields):
    return f"\n[[{kind}]]\n" + "".join(f"{key} = {json.dumps(value)}\n" for key, value in fields)


def payoff(action, state, value):
    return block("payoff", [("action", action), ("state", state), ("value", value)])


def model(states=None, actions=None, values=None, reverse=False, include_uu=True):
    states = STATES if states is None else states
    actions = ACTIONS if actions is None else actions
    values = PAYOFFS if values is None else values
    rows = [block("state", [("id", name), ("weight", weight)]) for name, weight in states]
    rows += [block("action", [("id", name), ("label", label), ("role", role)])
             for name, label, role in actions]
    rows += [payoff(action, state, values[action][state])
             for action, _, _ in actions for state, _ in states]
    if include_uu:
        rows.append(block("uu", [("id", "unmodelled_delay"),
                                 ("description", "Invented delay outside the declared scenarios"),
                                 ("trigger", "An unmodelled permitting delay occurs"),
                                 ("downside_usd_m", "5.0")]))
    if reverse:
        rows.reverse()
    return ('schema = "fmine.decision-model.v1"\nmodel_key = "invented_expansion"\n'
            'question = "Compare the explicitly declared alternatives"\ncurrency = "USD"\n'
            'valuation_basis = "Invented 2026 USD NPV, real, full project basis"\n' + "".join(rows))


def fixture(text=None):
    # Keep every fixture for diagnosis; no source files are deleted by this oracle.
    root = Path(tempfile.mkdtemp(prefix="fmine-decision-card-"))
    (root / FILE).write_text(model() if text is None else text, encoding="utf-8")
    return root


def invoke(root, output_format="json"):
    return subprocess.run(
        [str(BINARY), "decide", "card", FILE, "--workspace", str(root), "--format", output_format],
        capture_output=True, text=True, check=False, timeout=30,
    )


class DecisionCardScenarios(unittest.TestCase):
    def envelope(self, root):
        output = invoke(root)
        self.assertEqual(len(output.stdout.splitlines()), 1, output.stdout + output.stderr)
        result = json.loads(output.stdout)
        self.assertEqual(output.returncode, result["exit_code"], output.stdout)
        self.assertEqual(result["verb"], "decide card")
        self.assertEqual(len(result["flags"]), 4)
        self.assertTrue(all(value is False for value in result["flags"].values()))
        self.assertIsNone(result["certificate"])
        self.assertIsNone(result["anchor_id"])
        return result

    def card(self, root):
        raw = (root / FILE).read_bytes()
        result = self.envelope(root)
        self.assertEqual(result["exit_code"], 0, result)
        self.assertIsNone(result["refusal"])
        payload = result["payload"]
        self.assertEqual(payload["mode"], "declared_model_comparison")
        self.assertEqual((root / FILE).read_bytes(), raw)
        self.assertEqual(payload["source"], {
            "path": str(root / FILE), "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw),
        })
        card = payload["card"]
        self.assertEqual(card["schema"], "fmine.decision-card.v1")
        self.assertEqual(card["mode"], "finite_scenario_draft")
        self.assertEqual(card["weakest_input_color"], "estimated")
        self.assertEqual(len(card["digest"]), 64)
        self.assertTrue(all(value is False for value in card["flags"].values()))
        self.assertIn("not calibrated probabilities", card["no_claim"])
        self.assertEqual(card["output_claim"]["value"], card["summary"])
        return card

    def refused(self, text, code=None):
        root = fixture(text)
        before = (root / FILE).read_bytes()
        result = self.envelope(root)
        self.assertNotEqual(result["exit_code"], 0, result)
        self.assertIsNone(result["payload"])
        self.assertIsNotNone(result["refusal"])
        if code is not None:
            self.assertEqual(result["refusal"]["code"], f"FMINE-REF-{code}")
        self.assertEqual((root / FILE).read_bytes(), before)

    @staticmethod
    def alternatives(card):
        return {row["action_id"]: row for row in card["summary"]["alternatives"]}

    def test_gb29_known_answers_keep_complete_payoffs_and_assumed_source_claims(self):
        root = fixture()
        card = self.card(root)
        summary = card["summary"]
        self.assertEqual(summary["currency"], "USD")
        self.assertEqual(summary["monetary_scale"], "major_currency_units")
        self.assertEqual(summary["expected_value_order"], ["build", "do_nothing"])
        self.assertEqual(summary["expected_value_winners"], ["build"])
        self.assertEqual(summary["minimax_regret_winners"], ["build"])
        rows = self.alternatives(card)
        for action, expected in {
            "build": (100000000, 80000000, 200000000, 200000000),
            "do_nothing": (0, 180000000, 300000000, 0),
        }.items():
            row = rows[action]
            actual = tuple(Fraction(row[key]) for key in ("mean", "mean_regret", "max_regret", "cvar95_loss"))
            self.assertEqual(actual, expected)
            self.assertEqual([state["state_id"] for state in row["states"]], ["high", "low"])
            for state in row["states"]:
                self.assertEqual(Fraction(state["regret"]),
                                 Fraction(state["best_in_hindsight"]) - Fraction(state["payoff"]))
                self.assertGreaterEqual(Fraction(state["regret"]), 0)
        self.assertEqual(list(map(Fraction, rows["build"]["p10_p50_p90"])),
                         [-200000000, 300000000, 300000000])
        inputs = [card["input_claims"]["weights"], *card["input_claims"]["actions"]]
        self.assertEqual(set(card["output_claim"]["parents"]), {claim["id"] for claim in inputs})
        source_hash = hashlib.sha256((root / FILE).read_bytes()).hexdigest()
        for claim in [*inputs, card["output_claim"]]:
            self.assertEqual(claim["evidence"], "assumed")
            self.assertEqual(claim["confidentiality"], "Internal")
            self.assertEqual(claim["color"]["name"], "estimated")
            self.assertTrue(claim["uu_terms"])
            self.assertTrue(claim["anchors"])
            self.assertTrue(all(source["sha256"] == source_hash for source in claim["anchors"]))
        self.assertEqual(card, self.card(root))

    def test_safe_alternative_changes_minimax_without_replacing_expected_value_leader(self):
        actions = [*ACTIONS, ("safe", "Certain smaller return", "action")]
        values = {**PAYOFFS, "safe": {"high": "55000000", "low": "55000000"}}
        card = self.card(fixture(model(actions=actions, values=values)))
        self.assertEqual(card["summary"]["expected_value_order"], ["build", "safe", "do_nothing"])
        self.assertEqual(card["summary"]["minimax_regret_winners"], ["safe"])
        rows = self.alternatives(card)
        self.assertEqual(Fraction(rows["build"]["max_regret"]), 255000000)
        self.assertEqual(Fraction(rows["safe"]["max_regret"]), 245000000)
        self.assertEqual(Fraction(rows["safe"]["mean_regret"]), 147000000)
        self.assertEqual(Fraction(rows["build"]["mean"]), 100000000)

    def test_cvar_uses_fractional_mass_at_the_five_percent_tail_boundary(self):
        states = [("crash", "1/50"), ("bad", "2/25"), ("gain", "9/10")]
        values = {"build": {"crash": "-100", "bad": "-50", "gain": "10"},
                  "do_nothing": {name: "0" for name, _ in states}}
        row = self.alternatives(self.card(fixture(model(states=states, values=values))))["build"]
        # The worst 5% contains all 2% at loss 100 and only 3% of the 8% at loss 50.
        self.assertEqual(Fraction(row["cvar95_loss"]), (Fraction(2, 100) * 100 + Fraction(3, 100) * 50) / Fraction(5, 100))
        self.assertEqual(Fraction(row["cvar95_loss"]), 70)
        self.assertEqual(Fraction(row["mean"]), 3)
        self.assertEqual(list(map(Fraction, row["p10_p50_p90"])), [-50, 10, 10])

    def test_all_gain_alternative_keeps_negative_signed_tail_loss(self):
        values = {**PAYOFFS, "build": {"high": "30", "low": "10"}}
        row = self.alternatives(self.card(fixture(model(values=values))))["build"]
        self.assertEqual(Fraction(row["cvar95_loss"]), -10)
        self.assertEqual(Fraction(row["mean"]), 22)
        self.assertEqual(Fraction(row["mean_regret"]), 0)

    def test_exact_ties_keep_all_winners_and_use_ids_only_for_display_order(self):
        actions = [("zeta", "First supplied", "action"), ACTIONS[1], ("alpha", "Last supplied", "action")]
        values = {"zeta": {"high": "10", "low": "10"},
                  "alpha": {"high": "10", "low": "10"}, "do_nothing": PAYOFFS["do_nothing"]}
        summary = self.card(fixture(model(actions=actions, values=values)))["summary"]
        self.assertEqual(summary["expected_value_winners"], ["alpha", "zeta"])
        self.assertEqual(summary["minimax_regret_winners"], ["alpha", "zeta"])
        self.assertEqual(summary["expected_value_primary"], "alpha")
        self.assertEqual(summary["runner_up"], "zeta")

    def test_reordered_source_bytes_change_provenance_but_not_statistics_or_claim_order(self):
        root = fixture(model())
        original_hash = hashlib.sha256((root / FILE).read_bytes()).hexdigest()
        first = self.card(root)
        (root / FILE).write_text(model(reverse=True), encoding="utf-8")
        reordered = self.card(root)
        self.assertNotEqual(original_hash, hashlib.sha256((root / FILE).read_bytes()).hexdigest())
        self.assertEqual(first["summary"], reordered["summary"])
        self.assertNotEqual(first["digest"], reordered["digest"])
        self.assertNotEqual(first["output_claim"]["id"], reordered["output_claim"]["id"])
        self.assertNotEqual(first["output_claim"]["value_tag"], reordered["output_claim"]["value_tag"])
        self.assertEqual(first["input_claims"]["weights"]["value_tag"],
                         reordered["input_claims"]["weights"]["value_tag"])
        self.assertEqual([claim["value_tag"] for claim in first["input_claims"]["actions"]],
                         [claim["value_tag"] for claim in reordered["input_claims"]["actions"]])
        for card in (first, reordered):
            self.assertEqual([claim["value"]["action_id"] for claim in card["input_claims"]["actions"]],
                             ["build", "do_nothing"])
            self.assertEqual([state["state_id"] for state in card["input_claims"]["weights"]["value"]["states"]],
                             ["high", "low"])

    def test_incomplete_or_ambiguous_matrix_and_missing_do_nothing_refuse(self):
        original = model()
        for text in (
            original.replace(payoff("build", "low", "-200000000"), ""),
            original + payoff("build", "low", "-200000000"),
            original + payoff("undeclared", "high", "1"),
            original.replace('role = "do_nothing"', 'role = "action"'),
        ):
            with self.subTest(text=text[-100:]):
                self.refused(text, "105")

    def test_malformed_or_nonunit_weights_never_normalize_or_fill_a_missing_mass(self):
        for old, new in (("2/5", "1/5"), ("3/5", "0/1"), ("3/5", "1/0"), ("3/5", "0.6")):
            with self.subTest(weight=new):
                self.refused(model().replace(f'weight = "{old}"', f'weight = "{new}"'), "109")

    def test_exact_minor_units_work_and_invalid_currency_precision_or_upgrades_refuse(self):
        values = {**PAYOFFS, "build": {"high": "0.0001", "low": "-0.0001"}}
        row = self.alternatives(self.card(fixture(model(values=values))))["build"]
        self.assertEqual(Fraction(row["mean"]), Fraction(1, 50000))
        for text in (
            model().replace('currency = "USD"', 'currency = "UNREGISTERED"'),
            model().replace('value = "300000000"', 'value = "0.00001"'),
            'evidence = "measured"\n' + model(),
            'execution_authorized = true\n' + model(),
            model().replace('weight = "3/5"', 'weight = "3/5"\ncalibration = "verified"'),
        ):
            with self.subTest(text=text[:100]):
                self.refused(text, "506")

    def test_unknown_unknowns_cannot_be_omitted_or_given_nonpositive_exposure(self):
        for text in (model(include_uu=False),
                     model().replace('downside_usd_m = "5.0"', 'downside_usd_m = "0"'),
                     model().replace('downside_usd_m = "5.0"', 'downside_usd_m = "NaN"')):
            with self.subTest(text=text[-100:]):
                self.refused(text, "204")


def main():
    global BINARY
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--fixture-only", action="store_true")
    args = parser.parse_args()
    BINARY = args.binary.resolve()
    if args.fixture_only:
        print(fixture())
        return 0
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(DecisionCardScenarios)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
