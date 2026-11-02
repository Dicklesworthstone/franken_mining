"""Independent exact finite capital-program checks against the actual fmine binary.

Inputs are declared assumptions. GB-30 uses the plan's displayed buyback payoffs,
whose exact mean is USD 61.18 million. Expected subsets, joint-state payoffs,
weighted tails and regret are computed with Python Fraction, without Rust output.
"""

import argparse
from fractions import Fraction
import hashlib
import itertools
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest


BINARY = None
FILE = "capital-program.toml"
QUESTION = 'Compare invented capital choices "jointly" \\ Ω'
BASIS = "Invented 2030 real USD net delta NAV, full project ownership"
REFERENCE = "Declared reference-class widening; illustrative and uncalibrated"
UNKNOWN = {
    "id": "outside_scenarios", "description": 'Unmodelled "permit" delay \\ Ω',
    "trigger": "A permitting delay outside the declared states occurs", "downside_usd_m": "5.0",
}


def candidate(name, cost, group=None):
    row = {"id": name, "label": f"Invented {name}", "cost": str(cost),
           "reference_class_basis": f"Declared reference-class adjustment for {name}"}
    if group is not None:
        row["exclusion_group"] = group
    return row


def specification(values, *, states=None, costs=None, budget="2", cap=None,
                  tail="1/5", groups=None, requires=()):
    states = [("low", "1/2"), ("high", "1/2")] if states is None else states
    costs = {name: "1" for name in values} if costs is None else costs
    groups = {} if groups is None else groups
    return {
        "states": states,
        "candidates": [candidate(name, costs[name], groups.get(name)) for name in values],
        "payoffs": {name: {state: str(value) for (state, _), value in zip(states, row, strict=True)}
                    for name, row in values.items()},
        "budget": str(budget), "cap": None if cap is None else str(cap), "tail": str(tail),
        "requires": list(requires), "reference_class_basis": REFERENCE,
    }


def gb30(stream=False):
    values = {
        "A": ["-60", "40", "220", "480", "220"],
        "D": ["-70.6", "0", "70.6", "258.8", "47.1"],
        "E": ["-150", "60", "420", "960", "-330"],
    }
    if stream:
        values["E_s"] = ["-90", "100", "410", "820", "-260"]
    return specification(
        {name: [str(Fraction(value) * 1_000_000) for value in row]
         for name, row in values.items()},
        states=[(f"s{i}", "1/5") for i in range(1, 6)],
        costs={name: "400000000" for name in values}, budget="800000000", cap="150000000",
        groups={"E": "greenfield", "E_s": "greenfield"},
    )


def fields(values):
    return "".join(f"{key} = {json.dumps(value, ensure_ascii=False)}\n" for key, value in values.items())


def block(kind, values):
    return f"\n[[{kind}]]\n" + fields(values)


def model(spec, *, reverse=False, include_uu=True):
    top = {
        "schema": "fmine.capital-program.v1", "model_key": "invented_capital_program",
        "question": QUESTION, "currency": "USD", "valuation_basis": BASIS,
        "budget": spec["budget"], "tail_mass": spec["tail"],
        "payoff_convention": "additive_net_delta_nav",
        "reference_class_basis": spec["reference_class_basis"],
    }
    if spec["cap"] is not None:
        top["max_tail_loss"] = spec["cap"]
    rows = [block("state", {"id": name, "weight": weight}) for name, weight in spec["states"]]
    rows += [block("candidate", row) for row in spec["candidates"]]
    rows += [block("payoff", {"candidate": name, "state": state, "value": value})
             for name, values in spec["payoffs"].items() for state, value in values.items()]
    rows += [block("requires", {"candidate": name, "prerequisite": prerequisite})
             for name, prerequisite in spec["requires"]]
    if include_uu:
        rows.append(block("uu", UNKNOWN))
    return fields(top) + "".join(reversed(rows) if reverse else rows)


def fixture(text=None):
    # Retain all invented inputs for diagnosis; the oracle deletes no files.
    root = Path(tempfile.mkdtemp(prefix="fmine-capital-program-"))
    (root / FILE).write_text(model(gb30()) if text is None else text, encoding="utf-8")
    return root


def weighted_tail_loss(payoffs, weights, mass):
    remaining, total = mass, Fraction(0)
    for state in sorted(payoffs, key=lambda state: (payoffs[state], state)):
        take = min(remaining, weights[state])
        total -= take * payoffs[state]
        remaining -= take
        if remaining == 0:
            return total / mass
    raise AssertionError("fixture weights do not cover the requested tail")


def quantile(payoffs, weights, probability):
    cumulative = Fraction(0)
    for state in sorted(payoffs, key=lambda state: (payoffs[state], state)):
        cumulative += weights[state]
        if cumulative >= probability:
            return payoffs[state]
    raise AssertionError("fixture weights do not cover the requested quantile")


def enumerate_expected(spec):
    candidates = {row["id"]: row for row in spec["candidates"]}
    weights = {name: Fraction(weight) for name, weight in spec["states"]}
    assert sum(weights.values()) == 1
    budget, mass = Fraction(spec["budget"]), Fraction(spec["tail"])
    cap = None if spec["cap"] is None else Fraction(spec["cap"])
    choices = {}
    names = sorted(candidates)
    for count in range(len(names) + 1):
        for selected in itertools.combinations(names, count):
            spend = sum((Fraction(candidates[name]["cost"]) for name in selected), Fraction(0))
            groups = [candidates[name]["exclusion_group"] for name in selected
                      if "exclusion_group" in candidates[name]]
            reasons = set()
            if spend > budget:
                reasons.add("budget_exceeded")
            if len(groups) != len(set(groups)):
                reasons.add("mutual_exclusion")
            if any(name in selected and prerequisite not in selected for name, prerequisite in spec["requires"]):
                reasons.add("missing_prerequisite")
            choice = {"spend": spend, "budget_slack": budget - spend, "reasons": reasons}
            if not reasons:
                values = {state: sum((Fraction(spec["payoffs"][name][state]) for name in selected), Fraction(0))
                          for state in weights}
                choice.update({
                    "values": values, "mean": sum(weights[state] * value for state, value in values.items()),
                    "tail_loss": weighted_tail_loss(values, weights, mass),
                    "cvar95_loss": weighted_tail_loss(values, weights, Fraction(1, 20)),
                    "p10": quantile(values, weights, Fraction(1, 10)),
                    "p50": quantile(values, weights, Fraction(1, 2)),
                    "p90": quantile(values, weights, Fraction(9, 10)),
                })
                choice["risk_rejected"] = cap is not None and choice["tail_loss"] > cap
            choices[selected] = choice
    structural = {key: row for key, row in choices.items() if not row["reasons"]}
    hindsight = {state: max(row["values"][state] for row in structural.values()) for state in weights}
    for row in structural.values():
        regrets = {state: hindsight[state] - value for state, value in row["values"].items()}
        row["mean_regret"] = sum(weights[state] * value for state, value in regrets.items())
        row["max_regret"] = max(regrets.values())
        row["hindsight"], row["regrets"] = hindsight, regrets
    risk_feasible = {key: row for key, row in structural.items() if not row["risk_rejected"]}

    def winners(population):
        if not population:
            return set()
        best = max(row["mean"] for row in population.values())
        return {key for key, row in population.items() if row["mean"] == best}

    price = (max(row["mean"] for row in structural.values()) - max(row["mean"] for row in risk_feasible.values())
             if risk_feasible else None)
    return choices, weights, winners(structural), winners(risk_feasible), price


class CapitalProgramScenarios(unittest.TestCase):
    def envelope(self, root):
        output = subprocess.run(
            [str(BINARY), "decide", "program", FILE, "--workspace", str(root)],
            capture_output=True, text=True, encoding="utf-8", check=False, timeout=30,
        )
        self.assertEqual(len(output.stdout.splitlines()), 1, output.stdout + output.stderr)
        result = json.loads(output.stdout, parse_constant=lambda value: self.fail(f"non-JSON literal {value}"))
        self.assertEqual(output.returncode, result["exit_code"], output.stdout)
        self.assertEqual(result["verb"], "decide program")
        self.assertEqual(len(result["flags"]), 4)
        self.assertTrue(all(value is False for value in result["flags"].values()))
        self.assertIsNone(result["certificate"])
        self.assertIsNone(result["anchor_id"])
        return result

    def program(self, spec, root=None):
        root = fixture(model(spec)) if root is None else root
        raw = (root / FILE).read_bytes()
        envelope = self.envelope(root)
        self.assertEqual(envelope["exit_code"], 0, envelope)
        self.assertIsNone(envelope["refusal"])
        payload = envelope["payload"]
        self.assertEqual(payload["mode"], "declared_static_capital_comparison")
        source = {"path": str(root / FILE), "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}
        self.assertEqual(payload["source"], source)
        self.assertEqual((root / FILE).read_bytes(), raw)
        report = payload["program"]
        self.assertEqual(report["schema"], "fmine.capital-program.v1")
        self.assertEqual(len(bytes.fromhex(report["digest"])), 32)
        self.assertTrue(report["no_claim"])
        for flag in ("reserve_disclosure_claim", "execution_authorized", "geotechnical_signoff", "regulatory_compliance_claim"):
            self.assertFalse(report[flag])
        self.check_arithmetic(report, spec)
        self.check_evidence(report, spec, source)
        return report

    def check_arithmetic(self, report, spec):
        summary = report["summary"]
        self.assertEqual(summary["question"], QUESTION)
        self.assertEqual(summary["currency"], "USD")
        self.assertEqual(summary["monetary_scale"], "major_currency_units")
        self.assertIsNone(summary["selection_frequency"])
        expected, weights, unconstrained, feasible, price = enumerate_expected(spec)
        rows = {tuple(row["selected"]): row for row in summary["choices"]}
        self.assertEqual(len(rows), len(summary["choices"]))
        self.assertEqual(set(rows), set(expected))
        for selected, row in rows.items():
            known = expected[selected]
            self.assertEqual(list(selected), sorted(set(selected)))
            for field in ("spend", "budget_slack"):
                self.assertEqual(Fraction(row[field]), known[field], (selected, field))
            self.assertEqual({reason["kind"] for reason in row["structural_rejections"]}, known["reasons"], selected)
            if known["reasons"]:
                self.assertIsNone(row["statistics"])
                self.assertIsNone(row["tail_loss"])
                self.assertFalse(row["risk_rejected"])
                continue
            self.assertEqual(Fraction(row["tail_loss"]), known["tail_loss"], selected)
            self.assertEqual(row["risk_rejected"], known["risk_rejected"], selected)
            for field in ("mean", "p10", "p50", "p90", "cvar95_loss", "mean_regret", "max_regret"):
                self.assertEqual(Fraction(row["statistics"][field]), known[field], (selected, field))
            state_rows = row["statistics"]["states"]
            self.assertEqual([state["state_id"] for state in state_rows], sorted(weights))
            for state in state_rows:
                name = state["state_id"]
                self.assertEqual(Fraction(state["weight"]), weights[name])
                self.assertEqual(Fraction(state["payoff"]), known["values"][name])
                self.assertEqual(Fraction(state["best_in_hindsight"]), known["hindsight"][name])
                self.assertEqual(Fraction(state["regret"]), known["regrets"][name])
        membership = {row["bundle_id"]: selected for selected, row in rows.items()}
        self.assertEqual(len(membership), len(rows))
        for field, winners in (("unconstrained_winners", unconstrained), ("feasible_winners", feasible)):
            self.assertEqual(len(summary[field]), len(set(summary[field])))
            self.assertEqual({membership[name] for name in summary[field]}, winners)
        actual_price = summary["expected_value_cost_of_risk_limit"]
        self.assertEqual(None if actual_price is None else Fraction(actual_price), price)
        self.assertEqual(summary["status"], "ok" if feasible else "no_risk_feasible_program")

    def check_evidence(self, report, spec, source):
        inputs = report["inputs"]
        claims = [inputs["policy"], inputs["weights"], *inputs["candidates"]]
        result = report["claim"]
        self.assertEqual(result["value"], report["summary"])
        self.assertEqual(set(result["parents"]), {claim["id"] for claim in claims})
        self.assertEqual(len(result["parents"]), len(claims))
        self.assertEqual(inputs["policy"]["value"], report["summary"]["policy"])
        policy = inputs["policy"]["value"]
        for name, expected in (("budget", spec["budget"]), ("tail_mass", spec["tail"])):
            self.assertEqual(Fraction(policy[name]), Fraction(expected))
        self.assertEqual(None if policy["max_tail_loss"] is None else Fraction(policy["max_tail_loss"]),
                         None if spec["cap"] is None else Fraction(spec["cap"]))
        self.assertEqual(policy["payoff_convention"], "additive_net_delta_nav")
        self.assertEqual(policy["reference_class_basis"], spec["reference_class_basis"])
        self.assertEqual(policy["valuation_basis"], BASIS)
        self.assertEqual(policy["currency"], "USD")
        self.assertEqual(policy["budget_currency"], "USD")
        self.assertEqual(policy["max_tail_loss_currency"], None if spec["cap"] is None else "USD")
        weight_value = inputs["weights"]["value"]
        self.assertEqual(weight_value["question"], QUESTION)
        self.assertEqual(len(weight_value["states"]), len(spec["states"]))
        self.assertEqual({row["state_id"]: Fraction(row["weight"]) for row in weight_value["states"]},
                         {name: Fraction(weight) for name, weight in spec["states"]})
        candidate_claims = {claim["value"]["candidate_id"]: claim for claim in inputs["candidates"]}
        self.assertEqual(set(candidate_claims), {row["id"] for row in spec["candidates"]})
        for row in spec["candidates"]:
            value = candidate_claims[row["id"]]["value"]
            self.assertEqual(Fraction(value["cost"]), Fraction(row["cost"]))
            self.assertEqual(value["cost_currency"], "USD")
            self.assertEqual(value["currency"], "USD")
            self.assertEqual(value["valuation_basis"], BASIS)
            self.assertEqual(value["label"], row["label"])
            self.assertEqual(value["reference_class_basis"], row["reference_class_basis"])
            self.assertEqual(value["exclusion_group"], row.get("exclusion_group"))
            self.assertEqual(value["requires"], sorted(prerequisite for name, prerequisite in spec["requires"] if name == row["id"]))
            self.assertEqual(len(value["payoffs"]), len(spec["states"]))
            self.assertEqual({entry["state_id"]: Fraction(entry["value"]) for entry in value["payoffs"]},
                             {name: Fraction(amount) for name, amount in spec["payoffs"][row["id"]].items()})
            self.assertTrue(all(entry["currency"] == "USD" for entry in value["payoffs"]))
        for claim in [*claims, result]:
            self.assertTrue(bytes.fromhex(claim["canonical_body_hex"]))
            for field in ("id", "metric_key", "value_tag", "evidence_tag", "epistemic_tag", "state_tag", "additivity"):
                self.assertIn(field, claim)
            self.assertEqual(claim["representation"], "deterministic")
            self.assertEqual(claim["support"], "period_aggregate")
            self.assertEqual(claim["evidence"], "assumed")
            self.assertEqual(claim["state"], "estimated")
            self.assertEqual(claim["confidentiality"], "Internal")
            self.assertEqual(claim["color"]["name"], "estimated")
            self.assertFalse(claim["stale"])
            self.assertFalse(claim["calibration_failed"])
            self.assertIsNone(claim["supersedes"])
            self.assertTrue(claim["anchors"])
            self.assertTrue(all(anchor["sha256"] == source["sha256"] and anchor["artifact_path"] == source["path"]
                                and anchor["locator"] for anchor in claim["anchors"]))
            self.assertEqual(claim["uu_terms"], [{
                "scenario_id": UNKNOWN["id"], "description": UNKNOWN["description"],
                "trigger_indicator": UNKNOWN["trigger"], "downside_exposure_usd_m": 5.0,
                "downside_bits": struct.pack(">d", 5.0).hex(),
            }])

    def refused(self, text, code=None):
        root = fixture(text)
        before = (root / FILE).read_bytes()
        result = self.envelope(root)
        self.assertNotEqual(result["exit_code"], 0, result)
        self.assertIsNone(result["payload"])
        self.assertIsNotNone(result["refusal"])
        self.assertTrue(result["refusal"]["code"].startswith("FMINE-REF-"))
        if code is not None:
            self.assertEqual(result["refusal"]["code"], f"FMINE-REF-{code}")
        self.assertEqual((root / FILE).read_bytes(), before)

    @staticmethod
    def rows(report):
        return {tuple(row["selected"]): row for row in report["summary"]["choices"]}

    def test_gb30_displayed_buyback_and_exact_price_of_risk(self):
        report = self.program(gb30())
        rows = self.rows(report)
        for selected, mean in ((('D',), 61_180_000), (('A', 'D'), 241_180_000), (('A', 'E'), 372_000_000)):
            self.assertEqual(Fraction(rows[selected]["statistics"]["mean"]), mean)
        self.assertEqual(Fraction(rows[("A", "D")]["tail_loss"]), 130_600_000)
        self.assertEqual(Fraction(report["summary"]["expected_value_cost_of_risk_limit"]), 130_820_000)

    def test_gb30_stream_substitutes_for_greenfield_at_the_binding_tail_limit(self):
        report = self.program(gb30(stream=True))
        row = self.rows(report)[("A", "E_s")]
        self.assertEqual(Fraction(row["statistics"]["mean"]), 376_000_000)
        self.assertEqual(Fraction(row["tail_loss"]), 150_000_000)
        self.assertFalse(row["risk_rejected"])
        self.assertIn(row["bundle_id"], report["summary"]["feasible_winners"])

    def test_joint_states_distinguish_correlated_and_offsetting_candidate_risk(self):
        correlated = self.program(specification({"a": ["-80", "120"], "b": ["-80", "120"]}, cap="0", tail="1/2"))
        offsetting = self.program(specification({"a": ["-80", "120"], "b": ["120", "-80"]}, cap="0", tail="1/2"))
        self.assertEqual(Fraction(self.rows(correlated)[("a", "b")]["tail_loss"]), 160)
        self.assertEqual(Fraction(self.rows(offsetting)[("a", "b")]["tail_loss"]), -40)
        self.assertNotEqual(correlated["summary"]["feasible_winners"], offsetting["summary"]["feasible_winners"])

    def test_tail_boundary_integrates_fractional_state_mass_exactly(self):
        spec = specification({"project": ["-100", "-50", "100"]},
                             states=[("crash", "1/10"), ("bad", "1/5"), ("gain", "7/10")], cap="75")
        row = self.rows(self.program(spec))[("project",)]
        self.assertEqual(Fraction(row["tail_loss"]), 75)
        self.assertFalse(row["risk_rejected"])
        spec["cap"] = "74.9999"
        self.assertTrue(self.rows(self.program(spec))[("project",)]["risk_rejected"])

    def test_net_delta_nav_is_not_charged_capital_a_second_time(self):
        report = self.program(specification({"project": ["5", "5"]}, costs={"project": "100"}, budget="100", cap="-5"))
        row = self.rows(report)[("project",)]
        self.assertEqual(Fraction(row["spend"]), 100)
        self.assertEqual(Fraction(row["statistics"]["mean"]), 5)
        self.assertEqual(Fraction(row["tail_loss"]), -5)
        self.assertIn(row["bundle_id"], report["summary"]["feasible_winners"])

    def test_all_negative_programs_keep_hold_cash_as_the_unique_winner(self):
        report = self.program(specification({"a": ["-1", "-3"], "b": ["-5", "-2"]}))
        hold = self.rows(report)[()]["bundle_id"]
        self.assertEqual(report["summary"]["feasible_winners"], [hold])

    def test_sole_structurally_feasible_hold_has_real_zero_statistics(self):
        report = self.program(specification({"too_expensive": ["100", "100"]}, budget="0"))
        hold = self.rows(report)[()]
        self.assertEqual(Fraction(hold["statistics"]["mean"]), 0)
        self.assertEqual(Fraction(hold["tail_loss"]), 0)
        self.assertEqual(report["summary"]["feasible_winners"], [hold["bundle_id"]])

    def test_signed_negative_risk_limit_can_leave_no_risk_feasible_program(self):
        report = self.program(specification({"risky": ["-10", "20"]}, cap="-1"))
        self.assertEqual(report["summary"]["status"], "no_risk_feasible_program")
        self.assertEqual(report["summary"]["feasible_winners"], [])
        self.assertIsNone(report["summary"]["expected_value_cost_of_risk_limit"])

    def test_prerequisite_closure_and_budget_are_enforced_on_whole_subsets(self):
        spec = specification({"permit": ["-10", "-10"], "build": ["100", "100"], "extension": ["20", "20"]},
                             costs={"permit": "20", "build": "50", "extension": "10"}, budget="70",
                             requires=[("build", "permit"), ("extension", "build")])
        report = self.program(spec)
        winner = self.rows(report)[("build", "permit")]
        self.assertEqual(Fraction(winner["statistics"]["mean"]), 90)
        self.assertEqual(report["summary"]["feasible_winners"], [winner["bundle_id"]])

    def test_cycles_missing_prerequisites_and_duplicate_edges_refuse(self):
        base = specification({"a": ["1", "2"], "b": ["3", "4"]})
        for edges in ([('a', 'b'), ('b', 'a')], [('a', 'a')], [('a', 'absent')], [('absent', 'a')], [('a', 'b'), ('a', 'b')]):
            with self.subTest(edges=edges):
                base["requires"] = edges
                self.refused(model(base))

    def test_matrix_weights_required_unknowns_and_undeclared_fields_refuse(self):
        base = specification({"a": ["1", "2"]})
        text = model(base)
        payoff = block("payoff", {"candidate": "a", "state": "low", "value": "1"})
        for bad in (
            text.replace(payoff, ""), text + payoff,
            text + block("state", {"id": "low", "weight": "1/2"}),
            text + block("payoff", {"candidate": "absent", "state": "low", "value": "1"}),
            text + block("payoff", {"candidate": "a", "state": "absent", "value": "1"}),
            text.replace('weight = "1/2"', 'weight = "1/3"'),
            text.replace('tail_mass = "1/5"', 'tail_mass = "0/1"'),
            text.replace('cost = "1"', 'cost = "-1"'),
            'evidence = "measured"\n' + text,
        ):
            with self.subTest(tail=bad[-120:]):
                self.refused(bad)
        self.refused(model(base, include_uu=False), "204")

    def test_exact_ties_survive_source_reordering_without_a_silent_unique_selection(self):
        spec = specification({"zeta": ["10", "10"], "alpha": ["10", "10"]}, budget="1")
        root = fixture(model(spec))
        first = self.program(spec, root)
        self.assertEqual(len(first["summary"]["feasible_winners"]), 2)
        (root / FILE).write_text(model(spec, reverse=True), encoding="utf-8")
        reordered = self.program(spec, root)
        self.assertEqual(first["summary"], reordered["summary"])
        self.assertNotEqual(first["digest"], reordered["digest"])
        self.assertNotEqual(first["claim"]["id"], reordered["claim"]["id"])

    def test_source_and_constraint_only_changes_bind_receipt_identity(self):
        spec = specification({"a": ["2", "3"]}, budget="10", cap="10")
        root = fixture(model(spec))
        first = self.program(spec, root)
        (root / FILE).write_text(model(spec) + "\n# Source-only attribution edit\n", encoding="utf-8")
        source_changed = self.program(spec, root)
        self.assertEqual(first["summary"], source_changed["summary"])
        self.assertNotEqual(first["digest"], source_changed["digest"])
        self.assertNotEqual(first["claim"]["id"], source_changed["claim"]["id"])
        for field, changed in (("budget", "11"), ("cap", "11"), ("reference_class_basis", REFERENCE + "; explicit amendment")):
            with self.subTest(field=field):
                old = spec[field]
                spec[field] = changed
                (root / FILE).write_text(model(spec), encoding="utf-8")
                revised = self.program(spec, root)
                self.assertNotEqual(first["digest"], revised["digest"])
                self.assertNotEqual(first["inputs"]["policy"]["value_tag"], revised["inputs"]["policy"]["value_tag"])
                self.assertEqual(first["summary"]["feasible_winners"], revised["summary"]["feasible_winners"])
                self.assertEqual(self.rows(first)[("a",)]["statistics"], self.rows(revised)[("a",)]["statistics"])
                spec[field] = old

    def test_minor_units_remain_exact_and_aggregate_overflow_refuses(self):
        exact = specification({"a": ["0.0001", "-0.0001"], "b": ["0.0002", "0.0002"]},
                              states=[("low", "1/3"), ("high", "2/3")],
                              costs={"a": "0.0001", "b": "0.0001"}, budget="0.0002")
        report = self.program(exact)
        self.assertEqual(Fraction(self.rows(report)[("a",)]["statistics"]["mean"]), Fraction(-1, 30000))
        large = "17014118346046923173168730371588410.5727"
        overflow = specification({"a": [large], "b": [large]}, states=[("only", "1/1")])
        self.refused(model(overflow), "207")

    def test_bounded_eight_candidate_and_256_state_domains_are_explicit(self):
        self.program(specification({f"c{i}": [str(i)] for i in range(8)}, states=[("only", "1/1")], budget="4"))
        states = [(f"s{i:03}", "1/256") for i in range(256)]
        self.program(specification({"a": ["1"] * 256}, states=states))
        self.refused(model(specification({f"c{i}": [str(i)] for i in range(9)}, states=[("only", "1/1")], budget="4")))
        states = [(f"s{i:03}", "1/257") for i in range(257)]
        self.refused(model(specification({"a": ["1"] * 257}, states=states)))


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
