"""DESIGN: independent exact capital-selection checks under reported bootstrap draws.

The Rust sampler's count vectors are the test inputs, not an expected fixture.
This oracle independently enumerates the original feasible population from the
declared economic inputs, then recomputes every replicate's means, signed tails,
joint risk limits, complete co-winner set and selection/flip frequencies using
Fraction. Sampler arithmetic and seeded replay have separate kernel known answers.
Zero-count states never erase original cash, share-count or contractual failures.
All models here are synthetic conditional laws, not calibrated investment advice.
"""

import argparse
import copy
from fractions import Fraction as F
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


def reference(name):
    module = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(module)
    module.loader.exec_module(result)
    return result


calls = reference("capital_call_oracle")
valuation = reference("capital_valuation_oracle")
recourse = reference("capital_recourse_oracle")
entity, base = calls.entity, calls.base
FILE, BINARY = base.FILE, None
DESIGN = {
    "method": "weighted_multinomial", "seed": "229", "replicates": "200", "draws_per_replicate": "5",
    "sampling_basis": "Synthetic conditional scenario-law resampling, not empirical calibration",
}


def fragile_example():
    return base.specification({"expansion": [-8, 10], "steady": [3, 3]},
                              states=[("bad", "1/3"), ("good", "2/3")],
                              costs={"expansion": "10", "steady": "10"}, budget="10", cap="5", tail="2/5")


def design(spec):
    return {**DESIGN, **spec.get("stability", {})}


def model(spec, reverse=False):
    if "entity_valuations" in spec:
        writer = valuation.model
    elif "capital_calls" in spec.get("entity_funding", {}):
        writer = calls.model
    elif "entity_funding" in spec:
        writer = entity.model
    elif "recourse" in spec:
        writer = recourse.model
    else:
        writer = base.model
    return writer(spec, reverse=reverse) + base.block("selection_stability", design(spec))


def fixture(text=None):
    # Retain these reproducible diagnostic inputs; the oracle removes no files.
    root = Path(tempfile.mkdtemp(prefix="fmine-capital-stability-"))
    (root / FILE).write_text(model(calls.recourse_example()) if text is None else text, encoding="utf-8")
    return root


def population(spec, adaptive):
    if "entity_valuations" in spec:
        rows = (valuation.expected_policies if adaptive else valuation.expected_static)(spec)["rows"]
    elif "capital_calls" in spec.get("entity_funding", {}):
        rows = calls.expected(spec, adaptive)["rows"]
    elif "entity_funding" in spec:
        rows = (entity.expected_policies if adaptive else entity.expected_static)(spec)["rows"]
    elif adaptive:
        rows = recourse.enumerate_policies(spec)["policies"]
    else:
        rows = base.enumerate_expected(spec)[0]
    cap = spec["equity"]["max_tail_loss_per_share"] if "equity" in spec else spec["cap"]
    result = {}
    for key, row in rows.items():
        eligible = not row.get("rejections", row.get("reasons", []))
        objective = row.get("per_share", {}) if "equity" in spec else row
        values = objective.get("values", {}) if eligible else {}
        limits = [(values, F(cap))] if eligible and cap is not None else []
        for value in spec.get("entity_valuations", []):
            if eligible and value["max_tail_loss_per_share"] is not None:
                limits.append((row["secondary_statistics"][value["entity"]]["values"],
                               F(value["max_tail_loss_per_share"])))
        result[key] = {"eligible": eligible, "values": values, "limits": limits}
    return result


def rank(rows, weights, tail):
    means = {}
    for name, row in rows.items():
        if row["eligible"] and all(base.weighted_tail_loss(values, weights, tail) <= cap
                                   for values, cap in row["limits"]):
            means[name] = sum((weights[state] * value for state, value in row["values"].items()), F(0))
    best = max(means.values(), default=None)
    winners = {name for name, mean in means.items() if mean == best}
    next_mean = max((mean for mean in means.values() if mean != best), default=None)
    runner_up = {name for name, mean in means.items() if mean == next_mean}
    return winners, runner_up, means


class CapitalStabilityScenarios(unittest.TestCase):
    def program(self, spec, root=None):
        root = fixture(model(spec)) if root is None else root
        raw = (root / FILE).read_bytes()
        output = subprocess.run([str(BINARY), "decide", "program", FILE, "--workspace", str(root)],
                                capture_output=True, text=True, check=False, timeout=60)
        self.assertEqual(output.returncode, 0, output.stdout + output.stderr)
        envelope = json.loads(output.stdout)
        self.assertEqual(envelope["exit_code"], 0)
        self.assertIsNone(envelope["refusal"])
        self.assertTrue(all(flag is False for flag in envelope["flags"].values()))
        source = {"path": str(root / FILE), "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}
        self.assertEqual(envelope["payload"]["source"], source)
        self.assertEqual((root / FILE).read_bytes(), raw)
        report = envelope["payload"]["program"]
        adaptive = "recourse" in spec
        static = report["static_comparison"] if adaptive else report
        self.check_report(static, spec, False)
        claim = static["inputs"]["selection_stability"]
        self.assertEqual(claim["evidence"], "assumed")
        self.assertIn(claim["id"], static["claim"]["parents"])
        self.assertTrue(claim["anchors"])
        self.assertTrue(all(anchor["sha256"] == source["sha256"] for anchor in claim["anchors"]))
        if adaptive:
            self.check_report(report, spec, True)
            self.assertIn(claim["id"], report["claim"]["parents"])
            counts = lambda value: [row["state_counts"] for row in value["summary"]["selection_frequency"]["replicates"]]
            self.assertEqual(counts(report), counts(static))
        for flag in ("reserve_disclosure_claim", "execution_authorized", "geotechnical_signoff", "regulatory_compliance_claim"):
            self.assertFalse(report[flag])
        return report

    def check_report(self, report, spec, adaptive):
        summary = report["summary"]
        result = summary["selection_frequency"]
        self.assertEqual(summary["selection_frequency_status"], "conditional_weighted_bootstrap")
        declared = design(spec)
        expected_design = {"model_key": "invented_capital_program", **declared}
        for field in ("replicates", "draws_per_replicate"):
            expected_design[field] = int(expected_design[field])
        self.assertEqual(result["design"], expected_design)
        states = sorted(state for state, _ in spec["states"])
        self.assertEqual(result["state_ids"], states)
        known = population(spec, adaptive)
        actual = summary["policies" if adaptive else "choices"]
        rows = {entity.key_of(row) if adaptive else tuple(row["selected"]): row for row in actual}
        self.assertEqual(len(rows), len(actual))
        self.assertEqual(set(rows), set(known))
        id_field = "policy_id" if adaptive else "bundle_id"
        by_id = {rows[key][id_field]: value for key, value in known.items()}
        keys_by_id = {rows[key][id_field]: key for key in known}
        self.assertEqual(len(by_id), len(rows))
        for key, value in known.items():
            self.assertEqual(not rows[key]["structural_rejections"], value["eligible"])
        tail = F(spec["tail"])
        nominal, runner_up, _ = rank(by_id, {state: F(weight) for state, weight in spec["states"]}, tail)
        self.assertEqual(summary["feasible_winners"], sorted(nominal))
        self.assertEqual(result["nominal_winners"], sorted(nominal))
        self.assertEqual(result["nominal_runner_up"], sorted(runner_up))
        replications, draws = int(declared["replicates"]), int(declared["draws_per_replicate"])
        self.assertEqual(len(result["replicates"]), replications)
        selected = dict.fromkeys(by_id, 0)
        feasible = dict.fromkeys(by_id, 0)
        projections = [("initial", None), *(("realized_state", state) for state in states)] if adaptive else [("static", None)]
        candidate_counts = {(candidate["id"], scope, state): [0, 0] for candidate in spec["candidates"]
                            for scope, state in projections}

        def includes(alternative, candidate, scope, state):
            key = keys_by_id[alternative]
            if not adaptive:
                return candidate in key
            initial, branches = key
            if scope == "initial":
                return candidate in initial
            information = spec["recourse"]
            observation = information["observations"][state] if information["information_candidate"] in initial else None
            return candidate in initial or candidate in dict(branches)[observation]

        empty = definite = possible = 0
        for index, replicate in enumerate(result["replicates"]):
            self.assertEqual(replicate["index"], index)
            counts = replicate["state_counts"]
            self.assertEqual(len(counts), len(states))
            self.assertTrue(all(type(n) is int and 0 <= n <= draws for n in counts))
            self.assertEqual(sum(counts), draws)
            weights = {state: F(count, draws) for state, count in zip(states, counts, strict=True)}
            winners, _, admitted = rank(by_id, weights, tail)
            self.assertEqual(replicate["feasible_winners"], sorted(winners), (index, counts))
            self.assertEqual(replicate["risk_feasible_alternatives"], len(admitted))
            empty += not winners
            definite += bool(winners and nominal and not winners & nominal)
            possible += bool(nominal and winners - nominal)
            for name in admitted:
                feasible[name] += 1
            for name in winners:
                selected[name] += 1
            for (candidate, scope, state), bounds in candidate_counts.items():
                membership = [includes(name, candidate, scope, state) for name in winners]
                bounds[0] += bool(membership) and all(membership)
                bounds[1] += any(membership)
        self.assertEqual(result["no_feasible_replicates"], empty)
        for field, count in (("definite_flip_frequency", definite), ("possible_flip_frequency", possible)):
            self.assertEqual(None if result[field] is None else F(result[field]),
                             F(count, replications) if nominal else None)
        frequencies = {row["alternative_id"]: row for row in result["alternatives"]}
        self.assertEqual(len(frequencies), len(result["alternatives"]))
        self.assertEqual(set(frequencies), set(by_id))
        for name, row in frequencies.items():
            self.assertEqual(row["structurally_feasible"], by_id[name]["eligible"])
            self.assertEqual(row["nominal_winner"], name in nominal)
            self.assertEqual(row["risk_feasible_replicates"], feasible[name])
            self.assertEqual(row["co_winner_replicates"], selected[name])
            self.assertEqual(F(row["selection_frequency"]), F(selected[name], replications))
            self.assertEqual(row["fragile"], name in nominal and F(selected[name], replications) < F(4, 5))
        candidates = {(row["candidate_id"], row["scope"], row["state_id"]): row
                      for row in result["candidate_frequencies"]}
        self.assertEqual(len(candidates), len(result["candidate_frequencies"]))
        self.assertEqual(set(candidates), set(candidate_counts))
        for key, (lower, upper) in candidate_counts.items():
            row = candidates[key]
            self.assertEqual(row["certain_replicates"], lower)
            self.assertEqual(row["possible_replicates"], upper)
            self.assertEqual(F(row["frequency_lower"]), F(lower, replications))
            self.assertEqual(F(row["frequency_upper"]), F(upper, replications))

    @staticmethod
    def frequency(report, selected):
        choice = next(row for row in report["summary"]["choices"] if tuple(row["selected"]) == selected)
        return next(row for row in report["summary"]["selection_frequency"]["alternatives"]
                    if row["alternative_id"] == choice["bundle_id"])

    def test_dominant_choice_and_rejected_joint_spend(self):
        report = self.program(base.specification({"dominant": [10, 12], "weak": [1, 2]}, budget="1"))
        self.assertEqual(self.frequency(report, ("dominant",))["co_winner_replicates"], 200)
        self.assertFalse(self.frequency(report, ("dominant", "weak"))["structurally_feasible"])

    def test_nonuniform_fragility_and_fractional_tail_boundary(self):
        report = self.program(fragile_example())
        growth = self.frequency(report, ("expansion",))
        self.assertTrue(growth["nominal_winner"])
        self.assertTrue(growth["fragile"])
        count = sum(row["state_counts"][0] <= 1 for row in report["summary"]["selection_frequency"]["replicates"])
        self.assertEqual(growth["co_winner_replicates"], count)
        self.assertEqual(self.frequency(report, ("steady",))["co_winner_replicates"], 200 - count)

    def test_all_co_winners_survive_without_arbitrary_tie_breaks(self):
        report = self.program(base.specification({"left": [5, 5], "right": [5, 5]}, budget="1"))
        for name in ("left", "right"):
            self.assertEqual(self.frequency(report, (name,))["co_winner_replicates"], 200)
        for row in report["summary"]["selection_frequency"]["candidate_frequencies"]:
            self.assertEqual((row["certain_replicates"], row["possible_replicates"]), (0, 200))
        self.assertEqual(F(report["summary"]["selection_frequency"]["possible_flip_frequency"]), 0)

    def test_partial_ties_have_distinct_definite_and_possible_flips(self):
        spec = base.specification({"left": [6, 0], "right": [0, 6]},
                                  states=[("bad", "1/3"), ("good", "2/3")], budget="1")
        spec["stability"] = {"draws_per_replicate": "4"}
        result = self.program(spec)["summary"]["selection_frequency"]
        self.assertGreater(F(result["possible_flip_frequency"]), F(result["definite_flip_frequency"]))

    def test_empty_risk_populations_remain_in_denominator(self):
        spec = base.specification({"volatile": [-3, 3]}, cap="-1", tail="1/1")
        spec["stability"] = {"draws_per_replicate": "4"}
        result = self.program(spec)["summary"]["selection_frequency"]
        self.assertEqual(result["nominal_winners"], [])
        self.assertGreater(result["no_feasible_replicates"], 0)
        self.assertLess(result["no_feasible_replicates"], 200)
        self.assertIsNone(result["definite_flip_frequency"])

    def test_omitted_bad_draw_does_not_erase_cash_failure(self):
        spec = base.with_funding(base.specification({"tempting": [100, 100]}), [30, 100], {"tempting": [20, 20]})
        report = self.program(spec)
        self.assertEqual(self.frequency(report, ("tempting",))["co_winner_replicates"], 0)
        self.assertTrue(any(0 in row["state_counts"] for row in report["summary"]["selection_frequency"]["replicates"]))
        spec["funding"]["available_cash"] = {"low": "10", "high": "10"}
        self.assertEqual(self.program(spec)["summary"]["selection_frequency"]["no_feasible_replicates"], 200)

    def test_share_failure_remains_binding_when_state_is_not_drawn(self):
        spec = base.with_equity(base.specification({"retire": [100, 100], "sound": [1, 1]}, budget="1"),
                                [1000, 1000], {"retire": [-100, 0]})
        report = self.program(spec)
        self.assertFalse(self.frequency(report, ("retire",))["structurally_feasible"])
        self.assertEqual(self.frequency(report, ("sound",))["co_winner_replicates"], 200)

    def test_objective_remains_nav_per_share_not_total_nav(self):
        spec = base.with_equity(base.specification({"issue": [160, 160], "project": [100, 100]}, budget="1"),
                                [1000, 1000], {"issue": [20, 20]})
        report = self.program(spec)
        self.assertEqual(self.frequency(report, ("project",))["co_winner_replicates"], 200)
        self.assertEqual(self.frequency(report, ("issue",))["co_winner_replicates"], 0)

    def test_secondary_entity_cap_is_recomputed_on_each_joint_draw(self):
        spec = valuation.primary(entity.with_entities(base.specification(
            {"aggressive": [10, 10], "steady": [4, 4]}, budget="1", tail="1/1"), [("parent", 100, 20)]))
        valuation.with_valuation(spec, "carve-out", [100, 100],
                                 {"aggressive": [-12, 12], "steady": [2, 2]}, {}, initial="1", cap="0")
        report = self.program(spec)
        self.assertGreater(self.frequency(report, ("aggressive",))["co_winner_replicates"], 0)
        self.assertGreater(self.frequency(report, ("steady",))["co_winner_replicates"], 0)

    def test_adaptive_global_risk_uses_paired_unconditional_scenarios(self):
        self.program(recourse.global_risk_example())
        self.program(valuation.global_risk_example())

    def test_adaptive_calls_keep_initial_obligations_and_original_failures(self):
        report = self.program(calls.recourse_example())
        result = report["summary"]["selection_frequency"]
        self.assertEqual(len(report["summary"]["policies"]), 6)
        self.assertTrue(any(0 in row["state_counts"] for row in result["replicates"]))
        self.assertTrue(any(not row["structurally_feasible"] for row in result["alternatives"]))
        bad_build = next(row for row in result["candidate_frequencies"]
                         if row["candidate_id"] == "build" and row["state_id"] == "bad")
        self.assertEqual(bad_build["possible_replicates"], 0)

    def test_replay_source_order_seed_and_replicate_prefix_are_bound(self):
        spec = fragile_example()
        root = fixture(model(spec))
        original = self.program(spec, root)
        self.assertEqual(original, self.program(spec, root))
        (root / FILE).write_text(model(spec, reverse=True), encoding="utf-8")
        reordered = self.program(spec, root)
        self.assertEqual(original["summary"], reordered["summary"])
        self.assertNotEqual(original["digest"], reordered["digest"])
        larger = copy.deepcopy(spec)
        larger["stability"] = {"replicates": "201"}
        extended = self.program(larger)
        self.assertEqual(original["summary"]["selection_frequency"]["replicates"],
                         extended["summary"]["selection_frequency"]["replicates"][:200])
        changed = copy.deepcopy(spec)
        changed["stability"] = {"seed": "230"}
        new_seed = self.program(changed)
        self.assertNotEqual(original["summary"]["selection_frequency"]["replicates"],
                            new_seed["summary"]["selection_frequency"]["replicates"])
        self.assertNotEqual(original["inputs"]["selection_stability"]["value_tag"],
                            new_seed["inputs"]["selection_stability"]["value_tag"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--fixture-only", action="store_true")
    options = parser.parse_args()
    BINARY = options.binary.resolve()
    if options.fixture_only:
        print(fixture())
    else:
        unittest.main(argv=[__file__])
