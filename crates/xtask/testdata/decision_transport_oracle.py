"""Independent exact finite-support Wasserstein checks against the actual CLI.

Every model is invented. The oracle enumerates primal transport-polytope vertices
with Fraction arithmetic; it does not reproduce the production dual/hull solver.
With one row-sum equality per source and one cost inequality, a vertex sends each
source to one destination, except that at most one source can split between two
destinations when the cost budget binds. This is exhaustive for our 2--4 states.
"""

import argparse
from fractions import Fraction as F
from functools import lru_cache
import importlib.util
from itertools import combinations, product
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    "decision_fixture", Path(__file__).with_name("decision_card_oracle.py")
)
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)

EXAMPLE = Path(__file__).resolve().parents[3] / "tests/decisions/wasserstein.toml"
STATES = [("adverse", F(1, 4)), ("favorable", F(3, 4))]
VALUES = {
    "build": {"adverse": F(-12), "favorable": F(12)},
    "steady": {"adverse": F(5), "favorable": F(5)},
    "hold": {"adverse": F(0), "favorable": F(0)},
}
DISTANCES = ((F(0), F(1)), (F(1), F(0)))
METRIC_BASIS = "Invented dimensionless separation between the named scenarios"
RADIUS_BASIS = "Declared stress budget, not a fitted or calibrated confidence region"


def rational(value):
    value = F(value)
    return f"{value.numerator}/{value.denominator}"


def money(value):
    """Encode an exactly representable major-currency amount without float conversion."""
    minor = F(value) * 10000
    assert minor.denominator == 1
    integer = minor.numerator
    sign = "-" if integer < 0 else ""
    integer = abs(integer)
    return f"{sign}{integer // 10000}.{integer % 10000:04d}"


def robustness(states, distances, radius, reverse=False,
               metric_basis=METRIC_BASIS, radius_basis=RADIUS_BASIS):
    rows = [base.block("robustness", [
        ("method", "wasserstein_1"), ("radius", rational(radius)),
        ("metric_basis", metric_basis), ("radius_basis", radius_basis),
    ])]
    rows += [base.block("transport_distance", [
        ("from", source), ("to", target), ("value", rational(distances[i][j])),
    ]) for i, (source, _) in enumerate(states) for j, (target, _) in enumerate(states)]
    if reverse:
        rows.reverse()
    return "".join(rows)


def model(states=STATES, values=VALUES, distances=DISTANCES, radius=F(1, 4),
          reverse=False, currency="USD", include_robustness=True, **bases):
    actions = [(name, f"Invented {name}",
                "do_nothing" if name in ("hold", "do_nothing") else "action")
               for name in values]
    text = base.model(
        states=[(name, rational(weight)) for name, weight in states], actions=actions,
        values={action: {state: money(value) for state, value in payoffs.items()}
                for action, payoffs in values.items()}, reverse=reverse,
    )
    text = text.replace('currency = "USD"', f'currency = "{currency}"')
    text = text.replace("Invented 2026 USD NPV", f"Invented 2026 {currency} NPV")
    if include_robustness:
        text += robustness(states, distances, radius, reverse, **bases)
    return text


def line_metric(positions):
    return tuple(tuple(abs(F(a) - F(b)) for b in positions) for a in positions)


@lru_cache(maxsize=64)
def destination_vertices(weights, distances, radius):
    """Enumerate feasible primal vertices, retaining their complete destination laws."""
    size = len(weights)
    assert 2 <= size <= 4
    assert all(weight > 0 for weight in weights) and sum(weights) == 1
    assert radius >= 0
    laws = set()
    for destinations in product(range(size), repeat=size):
        cost = sum(weights[i] * distances[i][j] for i, j in enumerate(destinations))
        if cost <= radius:
            law = [F(0)] * size
            for i, j in enumerate(destinations):
                law[j] += weights[i]
            laws.add(tuple(law))
    for split in range(size):
        others = [i for i in range(size) if i != split]
        for destinations in product(range(size), repeat=size - 1):
            cost = sum(weights[i] * distances[i][j] for i, j in zip(others, destinations))
            law = [F(0)] * size
            for i, j in zip(others, destinations):
                law[j] += weights[i]
            for first, second in combinations(range(size), 2):
                delta = distances[split][first] - distances[split][second]
                if delta == 0:
                    continue
                to_first = (radius - cost - weights[split] * distances[split][second]) / delta
                if 0 <= to_first <= weights[split]:
                    candidate = law.copy()
                    candidate[first] += to_first
                    candidate[second] += weights[split] - to_first
                    laws.add(tuple(candidate))
    assert laws, "The identity transport is always feasible for a nonnegative radius"
    return tuple(sorted(laws))


def worst_value(weights, distances, radius, losses):
    return max(sum(mass * loss for mass, loss in zip(law, losses))
               for law in destination_vertices(tuple(weights), tuple(map(tuple, distances)), F(radius)))


class PrimalOracleKnownAnswers(unittest.TestCase):
    def test_two_state_split_and_saturation_have_hand_answers(self):
        weights = (F(1, 4), F(3, 4))
        self.assertEqual(worst_value(weights, DISTANCES, F(0), (F(12), F(-12))), -6)
        self.assertEqual(worst_value(weights, DISTANCES, F(1, 4), (F(12), F(-12))), 0)
        self.assertEqual(worst_value(weights, DISTANCES, F(1, 4), (F(17), F(0))), F(17, 2))
        self.assertEqual(worst_value(weights, DISTANCES, F(1, 4), (F(0), F(7))), 7)
        self.assertEqual(worst_value(weights, DISTANCES, F(1), (F(12), F(-12))), 12)

    def test_three_state_budget_crosses_two_distinct_transport_slopes(self):
        weights, distances = (F(1, 5), F(3, 10), F(1, 2)), line_metric((0, 1, 3))
        losses = (F(0), F(2), F(9))
        for radius, expected in [(F(0), F(51, 10)), (F(1, 2), F(137, 20)),
                                 (F(3, 5), F(36, 5)), (F(1), F(42, 5)), (F(6, 5), F(9))]:
            with self.subTest(radius=radius):
                self.assertEqual(worst_value(weights, distances, radius, losses), expected)

    def test_four_state_negative_constant_and_metric_scaling_are_exact(self):
        weights = (F(1, 10), F(1, 5), F(3, 10), F(2, 5))
        distances = line_metric((0, F(1, 3), F(5, 3), F(8, 3)))
        self.assertEqual(worst_value(weights, distances, F(2, 7), (F(-7),) * 4), -7)
        losses = (F(9), F(-7), F(2), F(6))
        scaled = tuple(tuple(7 * value for value in row) for row in distances)
        self.assertEqual(worst_value(weights, distances, F(2, 7), losses),
                         worst_value(weights, scaled, F(2), losses))


class TransportScenarios(unittest.TestCase):
    envelope = base.DecisionCardScenarios.envelope
    card = base.DecisionCardScenarios.card
    refused = base.DecisionCardScenarios.refused

    def exact(self, value):
        self.assertIsInstance(value, str)
        result = F(value)
        self.assertEqual(value, rational(result), "All exact values use reduced n/d strings")
        return result

    def certificate(self, certificate, weights, distances, radius, losses):
        size = len(weights)
        self.assertEqual(self.exact(certificate["radius"]), radius)
        destinations = list(map(self.exact, certificate["destination_weights"]))
        self.assertEqual(len(destinations), size)
        self.assertTrue(all(mass >= 0 for mass in destinations))
        self.assertEqual(sum(destinations), 1)
        source_mass, target_mass = [F(0)] * size, [F(0)] * size
        cost, primal = F(0), F(0)
        arcs = []
        for arc in certificate["arcs"]:
            source, target = arc["source"], arc["target"]
            self.assertIs(type(source), int)
            self.assertIs(type(target), int)
            self.assertTrue(0 <= source < size and 0 <= target < size)
            mass = self.exact(arc["mass"])
            self.assertGreater(mass, 0)
            source_mass[source] += mass
            target_mass[target] += mass
            cost += mass * distances[source][target]
            primal += mass * losses[target]
            arcs.append((source, target, mass))
        self.assertEqual(source_mass, list(weights))
        self.assertEqual(target_mass, destinations)
        self.assertEqual(self.exact(certificate["transport_cost"]), cost)
        self.assertLessEqual(cost, radius)
        self.assertEqual(primal, sum(mass * loss for mass, loss in zip(destinations, losses)))
        self.assertEqual(self.exact(certificate["worst_case_value"]), primal)
        # The independent optimization is over primal vertices, not production breakpoints.
        self.assertEqual(primal, worst_value(weights, distances, radius, losses))
        slope = self.exact(certificate["dual_lambda"])
        potentials = list(map(self.exact, certificate["dual_potentials"]))
        self.assertGreaterEqual(slope, 0)
        self.assertEqual(len(potentials), size)
        for source in range(size):
            for target in range(size):
                self.assertGreaterEqual(potentials[source] + slope * distances[source][target],
                                        losses[target])
        dual = slope * radius + sum(mass * value for mass, value in zip(weights, potentials))
        self.assertEqual(self.exact(certificate["dual_value"]), dual)
        self.assertEqual(self.exact(certificate["duality_gap"]), dual - primal)
        self.assertEqual(dual, primal)
        self.assertEqual(slope * (radius - cost), 0)
        for source, target, _ in arcs:
            self.assertEqual(potentials[source] + slope * distances[source][target], losses[target])
        return primal

    def checked(self, text=None, states=STATES, values=VALUES, distances=DISTANCES,
                radius=F(1, 4), currency="USD"):
        if text is None:
            text = model(states, values, distances, radius, currency=currency)
        root = base.fixture(text)
        card = self.card(root)
        summary = card["summary"]
        report = summary["robustness"]
        self.assertEqual(report["scope"], "act_now_actions")
        self.assertEqual(report["distance_unit"], "dimensionless")
        self.assertEqual(report["currency"], currency)
        self.assertEqual(report["monetary_scale"], "major_currency_units")
        self.assertTrue(report["no_claim"])
        self.assertEqual(report["design"]["model_key"], summary["model_key"])
        self.assertEqual(report["design"]["method"], "wasserstein_1")
        self.assertEqual(self.exact(report["design"]["radius"]), radius)
        self.assertTrue(report["design"]["metric_basis"])
        self.assertTrue(report["design"]["radius_basis"])
        canonical = sorted(range(len(states)), key=lambda index: states[index][0])
        names = [states[index][0] for index in canonical]
        weights = [F(states[index][1]) for index in canonical]
        metric = [[F(distances[i][j]) for j in canonical] for i in canonical]
        self.assertEqual(report["state_ids"], names)
        self.assertEqual(list(map(self.exact, report["nominal_weights"])), weights)
        self.assertEqual(report["design"]["distances"], [
            {"from_state": source, "to_state": target, "distance": rational(metric[i][j])}
            for i, source in enumerate(names) for j, target in enumerate(names)
        ])
        hindsight = [max(F(payoffs[state]) for payoffs in values.values()) for state in names]
        means, regrets, nominal_means = {}, {}, {}
        self.assertEqual([row["action_id"] for row in report["alternatives"]], sorted(values))
        for row in report["alternatives"]:
            action = row["action_id"]
            payoffs = [F(values[action][state]) for state in names]
            losses = [-value for value in payoffs]
            state_regret = [best - payoff for best, payoff in zip(hindsight, payoffs)]
            nominal_means[action] = sum(weight * payoff for weight, payoff in zip(weights, payoffs))
            self.assertEqual(self.exact(row["nominal_mean"]), nominal_means[action])
            self.assertEqual(self.exact(row["nominal_mean_regret"]),
                             sum(weight * value for weight, value in zip(weights, state_regret)))
            means[action] = -self.certificate(row["loss_certificate"], weights, metric, radius, losses)
            regrets[action] = self.certificate(row["regret_certificate"], weights, metric, radius, state_regret)
            self.assertEqual(self.exact(row["worst_case_mean"]), means[action])
            self.assertEqual(self.exact(row["worst_case_expected_regret"]), regrets[action])
        mean_order = sorted(values, key=lambda action: (-means[action], action))
        regret_order = sorted(values, key=lambda action: (regrets[action], action))
        mean_winners = [action for action in mean_order if means[action] == means[mean_order[0]]]
        regret_winners = [action for action in regret_order if regrets[action] == regrets[regret_order[0]]]
        nominal = sorted(action for action in values if nominal_means[action] == max(nominal_means.values()))
        self.assertEqual(report["maximin_mean_order"], mean_order)
        self.assertEqual(report["maximin_mean_winners"], mean_winners)
        self.assertEqual(report["minimax_expected_regret_order"], regret_order)
        self.assertEqual(report["minimax_expected_regret_winners"], regret_winners)
        self.assertEqual(report["nominal_expected_value_winners"], nominal)
        self.assertEqual(report["nominal_winner_survives_maximin"], bool(set(nominal) & set(mean_winners)))
        self.assertEqual(report["nominal_winner_survives_minimax_regret"], bool(set(nominal) & set(regret_winners)))
        claim = card["input_claims"]["robustness"]
        self.assertEqual(claim["value"], report["design"])
        self.assertEqual(len(claim["anchors"]), 1 + len(states) ** 2)
        source_lines = {line for line, value in enumerate(text.splitlines(), 1)
                        if value.strip() in ("[[robustness]]", "[[transport_distance]]")}
        self.assertEqual({int(anchor["locator"].rsplit("line ", 1)[1])
                          for anchor in claim["anchors"]}, source_lines)
        inputs = [card["input_claims"]["weights"], *card["input_claims"]["actions"],
                  *card["input_claims"]["information_options"], claim]
        self.assertEqual(set(card["output_claim"]["parents"]), {item["id"] for item in inputs})
        source_hash = base.hashlib.sha256((root / base.FILE).read_bytes()).hexdigest()
        for item in [*inputs, card["output_claim"]]:
            self.assertEqual(item["evidence"], "assumed")
            self.assertEqual(item["confidentiality"], "Internal")
            self.assertEqual(item["color"]["name"], "estimated")
            self.assertTrue(item["uu_terms"])
            self.assertTrue(item["anchors"])
            self.assertTrue(all(anchor["sha256"] == source_hash for anchor in item["anchors"]))
            self.assertTrue(all(anchor["artifact_path"] == str(root / base.FILE)
                                for anchor in item["anchors"]))
        self.assertEqual(card, self.card(root), "Identical current source bytes give identical certificates")
        return root, card

    @staticmethod
    def alternatives(card):
        return {row["action_id"]: row for row in card["summary"]["robustness"]["alternatives"]}

    def test_runnable_example_changes_both_robust_choices_with_hand_answers(self):
        scaled = {action: {state: 1000000 * value for state, value in payoffs.items()}
                  for action, payoffs in VALUES.items()}
        root, card = self.checked(EXAMPLE.read_text(encoding="utf-8"), values=scaled)
        report = card["summary"]["robustness"]
        self.assertEqual(report["nominal_expected_value_winners"], ["build"])
        self.assertEqual(report["maximin_mean_winners"], ["steady"])
        self.assertEqual(report["minimax_expected_regret_winners"], ["steady"])
        self.assertFalse(report["nominal_winner_survives_maximin"])
        self.assertFalse(report["nominal_winner_survives_minimax_regret"])
        rows = self.alternatives(card)
        for action, mean, worst_mean, worst_regret in [
            ("build", 6000000, 0, 8500000), ("steady", 5000000, 5000000, 7000000),
            ("hold", 0, 0, 12000000),
        ]:
            self.assertEqual(tuple(F(rows[action][key]) for key in
                                   ("nominal_mean", "worst_case_mean", "worst_case_expected_regret")),
                             (mean, worst_mean, worst_regret))
        markdown = base.invoke(root, "markdown")
        self.assertEqual(markdown.returncode, 0, markdown.stderr)
        for fragment in ("Wasserstein robustness", "1/4", "8500000/1", "7000000/1", "steady"):
            self.assertIn(fragment, markdown.stdout)

    def test_radius_zero_reproduces_nominal_rankings_and_keeps_all_ties(self):
        states = [("adverse", F(1, 2)), ("favorable", F(1, 2))]
        values = {"build": {"adverse": F(-3), "favorable": F(7)},
                  "steady": {"adverse": F(2), "favorable": F(2)}, "hold": VALUES["hold"]}
        _, card = self.checked(states=states, values=values, radius=F(0))
        report = card["summary"]["robustness"]
        self.assertEqual(report["maximin_mean_winners"], ["build", "steady"])
        self.assertEqual(report["minimax_expected_regret_winners"], ["build", "steady"])
        for row in report["alternatives"]:
            self.assertEqual(row["worst_case_mean"], row["nominal_mean"])
            self.assertEqual(row["worst_case_expected_regret"], row["nominal_mean_regret"])

    def test_three_and_four_state_transport_budgets_match_primal_vertices(self):
        cases = [
            ([("c", F(1, 5)), ("a", F(3, 10)), ("b", F(1, 2))],
             line_metric((0, 1, 3)), ((7, -2, 11), (4, 8, -5), (-3, -1, -2)),
             (F(1, 7), F(1, 2), F(6, 5))),
            ([("d", F(1, 10)), ("b", F(1, 5)), ("a", F(3, 10)), ("c", F(2, 5))],
             ((F(0), F(1), F(2), F(3)), (F(1), F(0), F(3), F(2)),
              (F(2), F(3), F(0), F(1)), (F(3), F(2), F(1), F(0))),
             ((12, -7, 8, 2), (4, 9, -3, 5), (-1, 0, -2, 1)),
             (F(1, 7), F(5, 8), F(7, 5))),
        ]
        for states, distances, amounts, radii in cases:
            values = {action: {state: F(value) for (state, _), value in zip(states, row)}
                      for action, row in zip(("build", "hedge", "hold"), amounts)}
            for radius in radii:
                with self.subTest(states=len(states), radius=radius):
                    self.checked(states=states, values=values, distances=distances, radius=radius)

    def test_saturating_radius_recovers_pure_maximum_same_state_regret(self):
        states = [("a", F(1, 10)), ("b", F(1, 5)), ("c", F(3, 10)), ("d", F(2, 5))]
        distances = line_metric((0, F(1, 3), F(5, 3), F(8, 3)))
        values = {action: {state: F(value) for (state, _), value in zip(states, row)}
                  for action, row in [("build", (12, -7, 8, 2)), ("hedge", (4, 9, -3, 5)),
                                      ("hold", (-1, 0, -2, 1))]}
        _, card = self.checked(states=states, values=values, distances=distances, radius=F(8, 3))
        report = card["summary"]["robustness"]
        nominal = {row["action_id"]: row for row in card["summary"]["alternatives"]}
        for row in report["alternatives"]:
            action = row["action_id"]
            self.assertEqual(F(row["worst_case_mean"]), min(values[action].values()))
            self.assertEqual(row["worst_case_expected_regret"], nominal[action]["max_regret"])
        self.assertEqual(report["minimax_expected_regret_winners"], card["summary"]["minimax_regret_winners"])

    def test_metric_geometry_changes_the_answer_and_joint_rescaling_preserves_it(self):
        _, baseline = self.checked()
        wider = tuple(tuple(8 * value for value in row) for row in DISTANCES)
        _, changed = self.checked(distances=wider)
        self.assertEqual(changed["summary"]["robustness"]["maximin_mean_winners"], ["build"])
        self.assertEqual(changed["summary"]["robustness"]["minimax_expected_regret_winners"], ["build"])
        scaled = tuple(tuple(F(3, 7) * value for value in row) for row in DISTANCES)
        _, rescaled = self.checked(distances=scaled, radius=F(3, 28))
        for action, row in self.alternatives(baseline).items():
            other = self.alternatives(rescaled)[action]
            for key in ("nominal_mean", "worst_case_mean", "nominal_mean_regret", "worst_case_expected_regret"):
                self.assertEqual(row[key], other[key])
        self.assertNotEqual(baseline["input_claims"]["robustness"]["value_tag"],
                            rescaled["input_claims"]["robustness"]["value_tag"])

    def test_canonical_permutation_and_source_only_changes_rebind_claims(self):
        _, original = self.checked()
        for text in (model(reverse=True), "# Same assumptions, separately identified source bytes.\n" + model()):
            _, changed = self.checked(text)
            self.assertEqual(original["summary"], changed["summary"])
            self.assertEqual(original["input_claims"]["robustness"]["value_tag"],
                             changed["input_claims"]["robustness"]["value_tag"])
            self.assertNotEqual(original["input_claims"]["robustness"]["id"],
                                changed["input_claims"]["robustness"]["id"])
            self.assertNotEqual(original["digest"], changed["digest"])
            self.assertNotEqual(original["output_claim"]["id"], changed["output_claim"]["id"])
            self.assertNotEqual(original["output_claim"]["value_tag"], changed["output_claim"]["value_tag"])

    def test_basis_and_radius_changes_are_bound_into_the_design_and_output(self):
        _, original = self.checked()
        for field in ("metric_basis", "radius_basis"):
            _, changed = self.checked(model(**{field: "A separately declared invented basis"}))
            self.assertEqual(self.alternatives(original), self.alternatives(changed))
            self.assertNotEqual(original["input_claims"]["robustness"]["value_tag"],
                                changed["input_claims"]["robustness"]["value_tag"])
            self.assertNotEqual(original["output_claim"]["value_tag"], changed["output_claim"]["value_tag"])
        _, changed = self.checked(radius=F(1, 3))
        self.assertNotEqual(self.alternatives(original), self.alternatives(changed))
        self.assertNotEqual(original["input_claims"]["robustness"]["value_tag"],
                            changed["input_claims"]["robustness"]["value_tag"])

    def test_currency_affine_money_and_negative_payoffs_preserve_regret_semantics(self):
        _, original = self.checked()
        shifted = {action: {state: 3 * value - 100 for state, value in payoffs.items()}
                   for action, payoffs in VALUES.items()}
        _, negative = self.checked(values=shifted, currency="CAD")
        tiny = {action: {state: value / 10000 for state, value in payoffs.items()}
                for action, payoffs in VALUES.items()}
        _, fractional = self.checked(values=tiny, currency="EUR")
        for action, row in self.alternatives(original).items():
            for key in ("nominal_mean", "worst_case_mean", "nominal_mean_regret", "worst_case_expected_regret"):
                shift = -100 if key in ("nominal_mean", "worst_case_mean") else 0
                self.assertEqual(F(self.alternatives(negative)[action][key]), 3 * F(row[key]) + shift)
                self.assertEqual(F(self.alternatives(fractional)[action][key]), F(row[key]) / 10000)
        for result in (negative, fractional):
            for key in ("maximin_mean_order", "minimax_expected_regret_order"):
                self.assertEqual(original["summary"]["robustness"][key], result["summary"]["robustness"][key])

    def test_all_ties_and_dominated_actions_remain_in_the_population(self):
        values = {**VALUES, "alpha": VALUES["steady"], "zeta": VALUES["steady"]}
        _, tied = self.checked(values=values)
        for key in ("maximin_mean_winners", "minimax_expected_regret_winners"):
            self.assertEqual(tied["summary"]["robustness"][key], ["alpha", "steady", "zeta"])
        _, original = self.checked()
        dominated = {**VALUES, "dominated": {state: F(-100) for state, _ in STATES}}
        _, enlarged = self.checked(values=dominated)
        for action, row in self.alternatives(original).items():
            self.assertEqual(row, self.alternatives(enlarged)[action])
        self.assertIn("dominated", self.alternatives(enlarged))
        self.assertNotEqual(original["digest"], enlarged["digest"])

    def test_nominal_information_options_stay_separate_and_legacy_fields_are_unchanged(self):
        legacy = self.card(base.fixture(base.model() + base.information()))
        self.assertNotIn("robustness", legacy["summary"])
        self.assertNotIn("robustness", legacy["input_claims"])
        states = [(state, F(weight)) for state, weight in base.STATES]
        values = {action: {state: F(value) for state, value in payoffs.items()}
                  for action, payoffs in base.PAYOFFS.items()}
        text = base.model() + base.information() + robustness(states, DISTANCES, F(1, 4))
        _, robust = self.checked(text, states=states, values=values)
        without_robustness = {key: value for key, value in robust["summary"].items() if key != "robustness"}
        self.assertEqual(legacy["summary"], without_robustness)
        self.assertEqual(legacy["summary"]["information"], robust["summary"]["information"])
        self.assertEqual(set(self.alternatives(robust)), set(values))
        self.assertNotIn("testwork", self.alternatives(robust))

    def test_incomplete_or_nonmetric_geometry_and_unsupported_designs_refuse(self):
        original = model()
        off_diagonal = base.block("transport_distance", [("from", "adverse"),
                                  ("to", "favorable"), ("value", "1/1")])
        variants = [
            original.replace(off_diagonal, ""), original + off_diagonal,
            original.replace(off_diagonal, off_diagonal.replace('"1/1"', '"2/1"')),
            original.replace('value = "0/1"', 'value = "1/1"', 1),
            original.replace('value = "1/1"', 'value = "0/1"'),
            original.replace('value = "1/1"', 'value = "-1/1"'),
            original.replace('radius = "1/4"', 'radius = "-1/4"'),
            original.replace('radius = "1/4"', 'radius = "0.25"'),
            original.replace('method = "wasserstein_1"', 'method = "wasserstein_2"'),
            original.replace(f'metric_basis = "{METRIC_BASIS}"', 'metric_basis = ""'),
            original.replace('to = "favorable"', 'to = "foreign"', 1),
        ]
        states = [("a", F(1, 3)), ("b", F(1, 3)), ("c", F(1, 3))]
        values = {"build": {state: F(1) for state, _ in states},
                  "hold": {state: F(0) for state, _ in states}}
        variants.append(model(states, values, ((F(0), F(1), F(3)),
                                               (F(1), F(0), F(1)), (F(3), F(1), F(0)))))
        for index, text in enumerate(variants):
            with self.subTest(variant=index):
                self.refused(text)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path, nargs="?")
    parser.add_argument("--fixture-only", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.fixture_only:
        print(base.fixture(EXAMPLE.read_text(encoding="utf-8")))
        return 0
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(PrimalOracleKnownAnswers)
    if not args.self_test:
        if args.binary is None:
            parser.error("binary is required unless --self-test or --fixture-only is supplied")
        base.BINARY = args.binary.resolve()
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(TransportScenarios))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
