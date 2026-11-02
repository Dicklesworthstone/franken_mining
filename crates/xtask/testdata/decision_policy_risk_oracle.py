"""Independent exact checks for DecisionCard expected-value/tail-risk comparisons.

All models are invented. Fraction arithmetic recomputes the expected value, loss probability,
and CVaR95 ordering from the declared payoff laws rather than trusting the Rust ranking.
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


def cvar95(distribution):
    """distribution is [(net_payoff, probability)]; largest 5% of signed loss."""
    remaining = F(1, 20)
    integral = F(0)
    for payoff, mass in sorted(distribution):
        if remaining == 0:
            break
        take = min(remaining, mass)
        integral += payoff * take
        remaining -= take
    assert remaining == 0
    return -integral / F(1, 20)


def act_law(card, action_id):
    row = next(x for x in card["summary"]["alternatives"] if x["action_id"] == action_id)
    return [(F(s["payoff"]), F(s["weight"])) for s in row["states"]]


def risk_points(card):
    return card["summary"]["information"]["risk_comparison"]


class RiskComparisonScenarios(unittest.TestCase):
    envelope = base.DecisionCardScenarios.envelope
    card = base.DecisionCardScenarios.card

    def with_information(self, info_text):
        return self.card(base.fixture(base.model() + info_text))

    def assert_point(self, point, law):
        mean = sum(value * mass for value, mass in law)
        loss = sum(mass for value, mass in law if value < 0)
        self.assertEqual(F(point["expected_value"]), mean)
        self.assertEqual(F(point["cvar95_loss"]), cvar95(law))
        self.assertEqual(F(point["loss_probability"]), loss)

    def test_gb29_expected_value_gain_is_kept_separate_from_worse_tail_loss(self):
        card = self.with_information(base.information())
        risk = risk_points(card)
        by_id = {row["id"]: row for row in risk["tail_risk_order"]}

        build = act_law(card, "build")
        nothing = act_law(card, "do_nothing")
        # Exact joint law from the declared 60/40 prior and 85%-accurate symmetric test.
        info = [
            (F(300000000 - 9800000), F(51, 100)),
            (F(-9800000), F(43, 100)),
            (F(-200000000 - 9800000), F(3, 50)),
        ]
        self.assert_point(by_id["build"], build)
        self.assert_point(by_id["do_nothing"], nothing)
        self.assert_point(by_id["testwork"], info)

        self.assertEqual([row["id"] for row in risk["tail_risk_order"]],
                         ["do_nothing", "build", "testwork"])
        self.assertEqual([row["id"] for row in risk["tail_risk_winners"]], ["do_nothing"])
        self.assertEqual([row["id"] for row in risk["expected_value_cvar95_frontier"]],
                         ["testwork", "build", "do_nothing"])
        self.assertGreater(F(by_id["testwork"]["expected_value"]), F(by_id["build"]["expected_value"]))
        self.assertGreater(F(by_id["testwork"]["cvar95_loss"]), F(by_id["build"]["cvar95_loss"]))
        self.assertIn("never silently scalarized", risk["rule"])

    def test_costly_uninformative_test_is_dominated_and_removed_only_from_frontier(self):
        matrix = [
            ("high", "seen", "1/1"), ("high", "impossible", "0/1"),
            ("low", "seen", "1/1"), ("low", "impossible", "0/1"),
        ]
        card = self.with_information(base.information(
            option="uninformative", matrix=matrix, signals=("seen", "impossible"),
            test_cost="10000000", delay_cost="0", delay_days=0,
        ))
        risk = risk_points(card)
        self.assertIn("uninformative", [row["id"] for row in risk["tail_risk_order"]])
        self.assertNotIn("uninformative",
                         [row["id"] for row in risk["expected_value_cvar95_frontier"]])
        rows = {row["id"]: row for row in risk["tail_risk_order"]}
        self.assertGreater(F(rows["build"]["expected_value"]), F(rows["uninformative"]["expected_value"]))
        self.assertLess(F(rows["build"]["cvar95_loss"]), F(rows["uninformative"]["cvar95_loss"]))

    def test_identical_zero_cost_policy_and_action_both_remain_on_frontier(self):
        matrix = [
            ("high", "seen", "1/1"), ("high", "impossible", "0/1"),
            ("low", "seen", "1/1"), ("low", "impossible", "0/1"),
        ]
        card = self.with_information(base.information(
            option="mirror", matrix=matrix, signals=("seen", "impossible"),
            test_cost="0", delay_cost="0", delay_days=0,
        ))
        risk = risk_points(card)
        rows = {row["id"]: row for row in risk["tail_risk_order"]}
        for key in ["expected_value", "cvar95_loss", "loss_probability"]:
            self.assertEqual(rows["build"][key], rows["mirror"][key])
        frontier = [row["id"] for row in risk["expected_value_cvar95_frontier"]]
        self.assertIn("build", frontier)
        self.assertIn("mirror", frontier)


def main():
    global base
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    args = parser.parse_args()
    base.BINARY = args.binary.resolve()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(RiskComparisonScenarios)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
