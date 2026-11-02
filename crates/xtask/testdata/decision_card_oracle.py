"""BD-225/227 known answers against the actual fmine binary, using only the standard library.

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
LIKELIHOODS = [("high", "good", "17/20"), ("high", "bad", "3/20"),
               ("low", "good", "3/20"), ("low", "bad", "17/20")]


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


def likelihood(option, state, signal, probability):
    return block("likelihood", [("option", option), ("state", state),
                                 ("signal", signal), ("probability", probability)])


def information(option="testwork", matrix=None, signals=("good", "bad"),
                test_cost="6000000", delay_cost="3800000", delay_days=180, reverse=False):
    matrix = LIKELIHOODS if matrix is None else matrix
    rows = [block("information_option", [("id", option), ("label", "Invented metallurgical test"),
                                          ("test_cost", test_cost), ("delay_cost", delay_cost),
                                          ("delay_days", delay_days)])]
    rows += [block("signal", [("option", option), ("id", signal)]) for signal in signals]
    rows += [likelihood(option, state, signal, probability) for state, signal, probability in matrix]
    if reverse:
        rows.reverse()
    return "".join(rows)


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
        self.assertIsNone(summary["information"])
        self.assertEqual(card["input_claims"]["information_options"], [])
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


    def information_summary(self, card):
        result = card["summary"]["information"]
        self.assertEqual(result["method"], "exact_finite_enumeration")
        self.assertIsNone(result["monte_carlo_se"])
        self.assertTrue(result["standard_error_note"])
        self.assertEqual(result["baseline_primary"], "build")
        return result

    def test_gb29_information_value_posteriors_and_policy_are_exact(self):
        card = self.card(fixture(model() + information()))
        result = self.information_summary(card)
        self.assertEqual(tuple(Fraction(result[k]) for k in
                               ("baseline_value", "perfect_information_value", "evpi")),
                         (100000000, 180000000, 80000000))
        option = result["options"][0]
        self.assertEqual(tuple(Fraction(option[k]) for k in
                               ("gross_value", "evsi", "total_cost", "net_voi", "net_value")),
                         (141000000, 41000000, 9800000, 31200000, 131200000))
        self.assertEqual(option["status"], "positive")
        self.assertEqual(Fraction(option["strict_change_probability"]), Fraction(43, 100))
        self.assertFalse(option["baseline_optimal_after_every_reachable_signal"])
        signals = {row["signal_id"]: row for row in option["signals"]}
        for name, probability, posterior_high, best in (
            ("good", Fraction(57, 100), Fraction(17, 19), "build"),
            ("bad", Fraction(43, 100), Fraction(9, 43), "do_nothing"),
        ):
            signal = signals[name]
            self.assertTrue(signal["reachable"])
            self.assertEqual(Fraction(signal["probability"]), probability)
            posterior = {r["state_id"]: Fraction(r["probability"])
                         for r in signal["posterior_state_weights"]}
            self.assertEqual(posterior, {"high": posterior_high, "low": 1 - posterior_high})
            conditional = {r["action_id"]: Fraction(r["value"])
                           for r in signal["posterior_action_values"]}
            self.assertEqual(conditional["build"], posterior_high * 300000000 - (1 - posterior_high) * 200000000)
            self.assertEqual(conditional["do_nothing"], 0)
            self.assertEqual(signal["best_actions"], [best])
            self.assertEqual(signal["selected_action"], best)
            self.assertEqual(Fraction(signal["conditional_value"]), conditional[best])
        self.assertEqual([(r["kind"], r["id"]) for r in result["overall_winners"]],
                         [("information_option", "testwork")])
        self.assertEqual(Fraction(result["overall_winners"][0]["expected_value"]), 131200000)
        self.assertEqual(card["summary"]["alternatives"],
                         self.card(fixture(model()))["summary"]["alternatives"])

    def test_perfect_uninformative_and_costly_options_keep_evsi_and_net_value_distinct(self):
        perfect = [("high", "good", "1/1"), ("high", "bad", "0/1"),
                   ("low", "good", "0/1"), ("low", "bad", "1/1")]
        unrelated = [(state, signal, "1/2") for state, _ in STATES for signal in ("good", "bad")]
        text = model() + information("perfect", perfect, test_cost="0", delay_cost="0", delay_days=0)
        text += information("unrelated", unrelated, test_cost="0", delay_cost="0", delay_days=0)
        # Declared delay cost is independent of days: the evaluator must not invent a discount rule.
        text += information("costly", test_cost="0", delay_cost="50000000", delay_days=0)
        result = self.information_summary(self.card(fixture(text)))
        options = {row["option_id"]: row for row in result["options"]}
        for name, evsi, net_voi, net_value, status in (
            ("perfect", 80000000, 80000000, 180000000, "positive"),
            ("unrelated", 0, 0, 100000000, "zero"),
            ("costly", 41000000, -9000000, 91000000, "negative"),
        ):
            row = options[name]
            self.assertEqual(tuple(Fraction(row[k]) for k in ("evsi", "net_voi", "net_value")),
                             (evsi, net_voi, net_value))
            self.assertEqual(row["status"], status)
            self.assertGreaterEqual(Fraction(row["evsi"]), 0)
            self.assertLessEqual(Fraction(row["evsi"]), Fraction(result["evpi"]))
        self.assertEqual(result["option_order"], ["perfect", "unrelated", "costly"])
        self.assertEqual(result["option_winners"], ["perfect"])
        self.assertTrue(options["unrelated"]["baseline_optimal_after_every_reachable_signal"])

    def test_no_flip_threshold_and_nearby_prior_keep_exact_ties_and_negative_net_voi(self):
        for high, low, evsi, net_voi, no_flip in (
            ("34/43", "9/43", 0, -9800000, True),
            ("39/50", "11/50", 2300000, -7500000, False),
        ):
            with self.subTest(prior=high):
                result = self.information_summary(self.card(fixture(
                    model(states=[("high", high), ("low", low)]) + information())))
                option = result["options"][0]
                self.assertEqual(Fraction(option["evsi"]), evsi)
                self.assertEqual(Fraction(option["net_voi"]), net_voi)
                self.assertEqual(option["status"], "negative")
                self.assertEqual(option["baseline_optimal_after_every_reachable_signal"], no_flip)
                self.assertEqual([(r["kind"], r["id"]) for r in result["overall_winners"]],
                                 [("act_now", "build")])
                bad = next(s for s in option["signals"] if s["signal_id"] == "bad")
                if no_flip:
                    self.assertEqual(Fraction(option["strict_change_probability"]), 0)
                    self.assertEqual(bad["best_actions"], ["build", "do_nothing"])
                    self.assertFalse(bad["strict_primary_change"])
                    self.assertEqual(Fraction(bad["conditional_value"]), 0)
                else:
                    self.assertEqual(Fraction(option["strict_change_probability"]), Fraction(38, 125))
                    self.assertTrue(bad["strict_primary_change"])

    def test_unreachable_signal_stays_visible_without_invented_posterior_or_policy(self):
        matrix = [*LIKELIHOODS, ("high", "impossible", "0/1"), ("low", "impossible", "0/1")]
        text = model() + information(matrix=matrix, signals=("good", "bad", "impossible"))
        option = self.information_summary(self.card(fixture(text)))["options"][0]
        self.assertEqual(len(option["signals"]), 3)
        signal = next(row for row in option["signals"] if row["signal_id"] == "impossible")
        self.assertFalse(signal["reachable"])
        self.assertEqual(Fraction(signal["probability"]), 0)
        for key in ("posterior_state_weights", "posterior_action_values", "selected_action",
                    "conditional_value", "strict_primary_change"):
            self.assertIsNone(signal[key])
        self.assertEqual(signal["best_actions"], [])
        self.assertEqual(Fraction(option["evsi"]), 41000000)
        self.refused(text.replace(likelihood("testwork", "low", "impossible", "0/1"), ""), "105")

    def test_tied_options_remain_separate_and_never_become_an_assumed_independent_batch(self):
        result = self.information_summary(self.card(fixture(
            model() + information("zeta") + information("alpha"))))
        self.assertEqual([row["option_id"] for row in result["options"]], ["alpha", "zeta"])
        self.assertEqual(result["option_order"], ["alpha", "zeta"])
        self.assertEqual(result["option_winners"], ["alpha", "zeta"])
        self.assertEqual(len(result["overall_order"]), len(ACTIONS) + 2)
        self.assertEqual([(r["kind"], r["id"]) for r in result["overall_winners"]],
                         [("information_option", "alpha"), ("information_option", "zeta")])
        self.assertTrue(all(Fraction(r["expected_value"]) == 131200000
                            for r in result["overall_winners"]))

    def test_information_membership_refuses_orphans_duplicates_and_missing_cells(self):
        original = model() + information()
        for text in (
            original.replace(likelihood("testwork", "high", "bad", "3/20"), ""),
            original + likelihood("testwork", "high", "bad", "3/20"),
            original + likelihood("undeclared", "high", "bad", "1/1"),
            original + likelihood("testwork", "unknown_state", "bad", "1/1"),
            original + likelihood("testwork", "high", "unknown_signal", "1/1"),
            original + information("incomplete", matrix=LIKELIHOODS[:-1]),
        ):
            with self.subTest(text=text[-110:]):
                self.refused(text, "105")

    def test_information_cost_and_likelihood_domains_refuse_without_normalization(self):
        original = model() + information()
        for old, new, code in (
            ('test_cost = "6000000"', 'test_cost = "-1"', "109"),
            ('delay_cost = "3800000"', 'delay_cost = "-1"', "109"),
            ('delay_days = 180', 'delay_days = -1', "109"),
            ('probability = "17/20"', 'probability = "21/20"', "109"),
            ('probability = "17/20"', 'probability = "16/20"', "109"),
            ('probability = "17/20"', 'probability = "1/0"', "109"),
            ('delay_days = 180', 'delay_days = "180"', "506"),
            ('test_cost = "6000000"', 'test_cost = "6000000"\nevidence = "measured"', "506"),
        ):
            with self.subTest(replacement=new):
                self.refused(original.replace(old, new), code)

    def test_all_information_claims_and_source_uu_mutations_bind_the_result_identity(self):
        root = fixture(model() + information("zeta", test_cost="50000000") + information("alpha"))
        original = self.card(root)
        claims = original["input_claims"]
        inputs = [claims["weights"], *claims["actions"], *claims["information_options"]]
        self.assertEqual(len(claims["information_options"]), 2)
        self.assertEqual(original["summary"]["information"]["option_winners"], ["alpha"])
        for claim in claims["information_options"]:
            self.assertEqual(claim["value"]["signals"], ["bad", "good"])
            self.assertEqual([(row["state_id"], row["signal_id"])
                              for row in claim["value"]["likelihoods"]],
                             [("high", "bad"), ("high", "good"), ("low", "bad"), ("low", "good")])
        self.assertEqual(set(original["output_claim"]["parents"]), {row["id"] for row in inputs})
        source_hash = hashlib.sha256((root / FILE).read_bytes()).hexdigest()
        for claim in [*inputs, original["output_claim"]]:
            self.assertEqual(claim["evidence"], "assumed")
            self.assertEqual(claim["confidentiality"], "Internal")
            self.assertEqual(claim["color"]["name"], "estimated")
            self.assertEqual(len(claim["uu_terms"]), 1)
            self.assertTrue(claim["anchors"])
            self.assertTrue(all(a["sha256"] == source_hash for a in claim["anchors"]))
        reordered_text = model(reverse=True) + information("alpha", reverse=True)
        reordered_text += information("zeta", test_cost="50000000", reverse=True)
        (root / FILE).write_text(reordered_text, encoding="utf-8")
        reordered = self.card(root)
        self.assertEqual(original["summary"], reordered["summary"])
        self.assertNotEqual(original["digest"], reordered["digest"])
        self.assertNotEqual(original["output_claim"]["id"], reordered["output_claim"]["id"])
        self.assertEqual([c["value_tag"] for c in claims["information_options"]],
                         [c["value_tag"] for c in reordered["input_claims"]["information_options"]])
        (root / FILE).write_text(reordered_text.replace('downside_usd_m = "5.0"',
                                                       'downside_usd_m = "6.0"'), encoding="utf-8")
        changed = self.card(root)
        self.assertEqual(reordered["summary"], changed["summary"])
        self.assertEqual(changed["output_claim"]["uu_terms"][0]["downside_exposure_usd_m"], 6)
        self.assertNotEqual(reordered["digest"], changed["digest"])
        self.assertNotEqual(reordered["output_claim"]["value_tag"], changed["output_claim"]["value_tag"])


def main():
    global BINARY
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--fixture-only", action="store_true")
    parser.add_argument("--with-information", action="store_true")
    args = parser.parse_args()
    BINARY = args.binary.resolve()
    if args.fixture_only:
        print(fixture(model() + information() if args.with_information else model()))
        return 0
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(DecisionCardScenarios)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
