"""Exact flip-margin checks against the real CLI; every model is invented.

Reuse the existing declared-model fixture and envelope checks. Fraction reconstruction
checks the emitted primal/dual witnesses without implementing the candidate's greedy solver.
"""

import argparse
from fractions import Fraction as F
import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    "decision_fixture", Path(__file__).with_name("decision_card_oracle.py")
)
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)


class SensitivityScenarios(unittest.TestCase):
    envelope = base.DecisionCardScenarios.envelope
    card = base.DecisionCardScenarios.card

    def checked(self, text):
        root = base.fixture(text)
        card = self.card(root)
        summary = card["summary"]
        report = summary["sensitivity"]
        self.assertEqual(report["status"], "ok", report)
        self.assertEqual(report["currency"], summary["currency"])
        self.assertEqual(report["monetary_scale"], "major_currency_units")
        self.assertEqual(report["primary"], summary["expected_value_primary"])
        self.assertIn("not a probability", report["no_claim"])
        alternatives = {row["action_id"]: row for row in summary["alternatives"]}
        primary = alternatives[report["primary"]]
        weights = {row["state_id"]: F(row["weight"]) for row in primary["states"]}
        payoffs = {row["state_id"]: F(row["payoff"]) for row in primary["states"]}
        self.assertEqual(sum(weights.values()), 1)
        expected_ids = sorted(set(alternatives) - {report["primary"]})
        self.assertEqual([row["action_id"] for row in report["comparisons"]], expected_ids)
        costs, radii = {}, {}
        for row in report["comparisons"]:
            rival = alternatives[row["action_id"]]
            other = {state["state_id"]: F(state["payoff"]) for state in rival["states"]}
            d = {state: value - other[state] for state, value in payoffs.items()}
            gap = sum(weights[state] * value for state, value in d.items())
            minimum = min(d.values())
            self.assertGreaterEqual(gap, 0)
            self.assertEqual(F(row["uniform_cost_to_tie"]), gap)
            self.assertEqual(F(row["minimum_state_advantage"]), minimum)
            self.assertEqual(row["strict_overtake_possible"], minimum < 0)
            costs[row["action_id"]] = gap
            boundary = row["weight_boundary"]
            if boundary is None:
                self.assertGreater(minimum, 0)
                self.assertEqual(row["weight_status"], "strict_pointwise_dominance")
                continue
            self.assertEqual(row["weight_status"], "tie_reachable")
            radius = F(boundary["total_variation"])
            self.assertGreaterEqual(radius, 0)
            self.assertLessEqual(radius, 1)
            q = weights.copy()
            transferred = F(0)
            for transfer in boundary["transfers"]:
                source, target = transfer["from_state"], transfer["to_state"]
                mass = F(transfer["mass"])
                self.assertNotEqual(source, target)
                self.assertGreater(mass, 0)
                q[source] -= mass
                q[target] += mass
                transferred += mass
            self.assertTrue(all(value >= 0 for value in q.values()))
            self.assertEqual(sum(q.values()), 1)
            self.assertEqual(sum(q[state] * value for state, value in d.items()), 0)
            self.assertEqual(sum(abs(q[state] - weight) for state, weight in weights.items()) / 2, radius)
            self.assertEqual(transferred, radius)
            if gap == 0:
                self.assertEqual(radius, 0)
                self.assertEqual(boundary["transfers"], [])
                self.assertIsNone(boundary["dual"])
            else:
                dual = boundary["dual"]
                slope, intercept = F(dual["slope"]), F(dual["intercept"])
                self.assertGreater(slope, 0)
                expected = sum(weights[state] * max(0, value - minimum - slope)
                               for state, value in d.items())
                self.assertEqual(intercept, expected)
                self.assertEqual(slope * radius + intercept, gap)
            radii[row["action_id"]] = radius
        cost = min(costs.values())
        self.assertEqual(F(report["uniform_cost_to_tie"]), cost)
        self.assertEqual(report["cost_challengers"], sorted(k for k, v in costs.items() if v == cost))
        if radii:
            radius = min(radii.values())
            self.assertEqual(F(report["total_variation_to_tie"]), radius)
            self.assertEqual(report["weight_challengers"], sorted(k for k, v in radii.items() if v == radius))
        else:
            self.assertIsNone(report["total_variation_to_tie"])
            self.assertEqual(report["weight_challengers"], [])
        self.assertEqual(card, self.card(root), "same source bytes produce the same full report")
        return root, card

    def test_expansion_known_answers_and_real_boundary_re_evaluation(self):
        root, card = self.checked(base.model())
        report = card["summary"]["sensitivity"]
        self.assertEqual(F(report["uniform_cost_to_tie"]), 100_000_000)
        self.assertEqual(F(report["total_variation_to_tie"]), F(1, 5))
        transfer = report["comparisons"][0]["weight_boundary"]["transfers"]
        self.assertEqual(transfer, [{"from_state": "high", "to_state": "low", "mass": "1/5"}])
        _, tied = self.checked(base.model(states=[("high", "2/5"), ("low", "3/5")]))
        self.assertEqual(tied["summary"]["expected_value_winners"], ["build", "do_nothing"])
        _, reversed_card = self.checked(base.model(states=[("high", "39/100"), ("low", "61/100")]))
        self.assertEqual(reversed_card["summary"]["expected_value_winners"], ["do_nothing"])
        shifted = {"build": {"high": "200000000", "low": "-300000000"}, base.ACTIONS[1][0]: base.PAYOFFS["do_nothing"]}
        _, tied_cost = self.checked(base.model(values=shifted))
        self.assertEqual(tied_cost["summary"]["expected_value_winners"], ["build", "do_nothing"])
        markdown = base.invoke(root, "markdown")
        self.assertEqual(markdown.returncode, 0, markdown.stderr)
        for text in ["Expected-value flip margins", "100000000/1", "1/5", "high", "low", "not a probability"]:
            self.assertIn(text, markdown.stdout)

    def test_all_challengers_are_checked_not_only_the_runner_up(self):
        states = [("high", "99/100"), ("low", "1/100")]
        actions = [*base.ACTIONS, ("near", "Near competitor", "action"), ("tail", "Tail competitor", "action")]
        values = {"build": {"high": "10", "low": "10"}, "near": {"high": "9", "low": "9"},
                  "tail": {"high": "0", "low": "100"}, "do_nothing": {"high": "0", "low": "0"}}
        _, card = self.checked(base.model(states=states, actions=actions, values=values))
        report = card["summary"]["sensitivity"]
        self.assertEqual(card["summary"]["runner_up"], "near")
        self.assertEqual(report["cost_challengers"], ["near"])
        self.assertEqual(F(report["uniform_cost_to_tie"]), 1)
        self.assertEqual(report["weight_challengers"], ["tail"])
        self.assertEqual(F(report["total_variation_to_tie"]), F(9, 100))

    def test_dominance_tie_only_and_existing_ties_are_not_conflated(self):
        states = [("high", "1/2"), ("low", "1/2")]
        for values, radius, strict in [((2, 1), None, False), ((1, 0), F(1, 2), False),
                                       ((1, -1), F(0), True), ((0, 0), F(0), False)]:
            with self.subTest(values=values):
                payoffs = {"build": dict(zip(("high", "low"), map(str, values))),
                           "do_nothing": {"high": "0", "low": "0"}}
                _, card = self.checked(base.model(states=states, values=payoffs))
                report = card["summary"]["sensitivity"]
                self.assertEqual(report["comparisons"][0]["strict_overtake_possible"], strict)
                actual = report["total_variation_to_tie"]
                self.assertEqual(None if actual is None else F(actual), radius)

    def test_multiple_donors_and_exact_affine_invariance(self):
        states = [("a", "1/10"), ("b", "4/5"), ("c", "1/10")]
        values = {"build": {"a": "10", "b": "4", "c": "-1"},
                  "do_nothing": {name: "0" for name, _ in states}}
        _, original = self.checked(base.model(states=states, values=values))
        report = original["summary"]["sensitivity"]
        self.assertEqual(F(report["total_variation_to_tie"]), F(7, 10))
        proof = report["comparisons"][0]["weight_boundary"]
        self.assertEqual([F(row["mass"]) for row in proof["transfers"]], [F(1, 10), F(3, 5)])
        self.assertEqual(F(proof["dual"]["slope"]), 5)
        self.assertEqual(F(proof["dual"]["intercept"]), F(3, 5))
        shifts = {"a": 40, "b": -100, "c": 7}
        changed = {action: {state: str(3 * int(value) + shifts[state]) for state, value in payoff.items()}
                   for action, payoff in values.items()}
        _, scaled = self.checked(base.model(states=states, values=changed, reverse=True))
        stress = scaled["summary"]["sensitivity"]
        self.assertEqual(stress["total_variation_to_tie"], report["total_variation_to_tie"])
        self.assertEqual(F(stress["uniform_cost_to_tie"]), 3 * F(report["uniform_cost_to_tie"]))
        self.assertEqual(stress["comparisons"][0]["weight_boundary"]["transfers"], proof["transfers"])

    def test_all_equal_weakest_challengers_and_ordering_are_retained(self):
        actions = [*base.ACTIONS, ("also_hold", "Another hold", "action")]
        values = {**base.PAYOFFS, "also_hold": {"high": "0", "low": "0"}}
        _, original = self.checked(base.model(actions=actions, values=values))
        _, reordered = self.checked(base.model(actions=actions, values=values, reverse=True))
        first, second = original["summary"]["sensitivity"], reordered["summary"]["sensitivity"]
        self.assertEqual(first, second)
        self.assertEqual(first["cost_challengers"], ["also_hold", "do_nothing"])
        self.assertEqual(first["weight_challengers"], ["also_hold", "do_nothing"])
        self.assertNotEqual(original["output_claim"]["id"], reordered["output_claim"]["id"])

    def test_information_policy_does_not_silently_redefine_the_stressed_choice(self):
        _, baseline = self.checked(base.model())
        _, informed = self.checked(base.model() + base.information())
        self.assertEqual(baseline["summary"]["sensitivity"], informed["summary"]["sensitivity"])
        self.assertEqual(informed["summary"]["information"]["overall_winners"][0]["id"], "testwork")
        self.assertEqual(informed["summary"]["sensitivity"]["primary"], "build")
        self.assertNotEqual(baseline["digest"], informed["digest"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    args = parser.parse_args()
    base.BINARY = args.binary.resolve()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(SensitivityScenarios)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
