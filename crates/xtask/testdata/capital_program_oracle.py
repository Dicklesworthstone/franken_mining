"""Independent exact finite capital-program checks against the actual fmine binary.

Inputs are declared assumptions. GB-30 uses the plan's displayed buyback payoffs,
whose exact mean is USD 61.18 million. Expected subsets, joint-state payoffs,
weighted tails and regret are computed with Python Fraction, without Rust output.
Optional funding is checked on every joint state, independently of net-NAV payoffs.
Timed funding carries exact signed cash flows between declared checkpoints; later
receipts never erase an earlier breach, and a financing action may rescue hold.
Equity models compare the change in whole-bundle NAV per actual share, keeping
total-dollar NAV and share-normalized statistics as separate accounting columns.
"""

import argparse
from datetime import date, timedelta
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
CASH_BASIS = 'Invented nominal USD cash at 100% ownership, "available" before spending \\ Ω'
SCHEDULE_BASIS = 'Invented nominal USD signed period cash flows, same entity and ownership \\ Ω'
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


def with_funding(spec, available, requirements, minimum="20"):
    states = [name for name, _ in spec["states"]]
    spec["funding"] = {
        "cash_period": "2030-01-01/2030-12-31", "cash_basis": CASH_BASIS,
        "minimum_cash": str(minimum),
        "available_cash": dict(zip(states, map(str, available), strict=True)),
        "requirements": {name: dict(zip(states, map(str, values), strict=True))
                         for name, values in requirements.items()},
    }
    return spec


def funded_example():
    return with_funding(
        specification({"a": ["30", "30"], "b": ["20", "20"]},
                      costs={"a": "9", "b": "7"}, budget="16"),
        ["120", "120"], {"a": ["60", "40"], "b": ["40", "60"]},
    )


def with_schedule(spec, opening, baseline, flows, *, periods=None,
                  opening_date="2030-01-01"):
    periods = ([
        ("construction", "2030-01-01/2030-06-30", "20"),
        ("operation", "2030-07-01/2030-12-31", "20"),
    ] if periods is None else periods)
    states = [name for name, _ in spec["states"]]
    candidates = [row["id"] for row in spec["candidates"]]
    zeros = ["0"] * len(states)
    # Omitted events in this fixture helper are emitted as explicit zero rows.
    # The input format itself must require every matrix cell, including zeroes.
    spec["funding_schedule"] = {
        "cash_basis": SCHEDULE_BASIS, "opening_date": opening_date,
        "periods": [{"id": name, "cash_period": window, "minimum_cash": str(minimum)}
                    for name, window, minimum in periods],
        "opening_cash": dict(zip(states, map(str, opening), strict=True)),
        "baseline_flows": {
            period: dict(zip(states, map(str, baseline.get(period, zeros)), strict=True))
            for period, _, _ in periods
        },
        "candidate_flows": {
            period: {
                name: dict(zip(states, map(str, flows.get(period, {}).get(name, zeros)), strict=True))
                for name in candidates
            } for period, _, _ in periods
        },
    }
    return spec


def scheduled_example():
    return with_schedule(
        specification({"bridge": ["-8", "-5"], "plant": ["20", "80"]},
                      states=[("low", "1/5"), ("high", "4/5")],
                      costs={"bridge": "0", "plant": "40"}, budget="40"),
        ["100", "100"],
        {"construction": ["-90", "-90"], "operation": ["80", "120"]},
        {"construction": {"bridge": ["60", "60"], "plant": ["-50", "-40"]},
         "operation": {"bridge": ["-68", "-65"], "plant": ["20", "50"]}},
    )


def ratio(value):
    value = Fraction(value)
    return f"{value.numerator}/{value.denominator}"


def with_equity(spec, baseline, changes, *, initial="100/1", scale="1/1", cap=None):
    states = [name for name, _ in spec["states"]]
    spec["payoff_convention"] = "additive_equity_nav_with_share_changes"
    spec["equity"] = {
        "entity": 'Invented parent "equity" \\ Ω', "share_class": "Invented ordinary shares",
        "shares_per_unit": ratio(scale), "initial_share_units": ratio(initial),
        "max_tail_loss_per_share": None if cap is None else ratio(cap),
        "baseline_nav": dict(zip(states, map(str, baseline), strict=True)),
        "share_changes": {
            row["id"]: dict(zip(states, map(ratio, changes.get(row["id"], ["0/1"] * len(states))), strict=True))
            for row in spec["candidates"]
        },
    }
    return spec


def equity_example():
    return with_equity(
        specification({"buyback": ["-80", "-80"], "project": ["100", "100"], "issue": ["160", "160"]},
                      costs={"buyback": "80", "project": "100", "issue": "0"}, budget="180"),
        ["1000", "1000"], {"buyback": ["-10", "-10"], "issue": ["20", "20"]},
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
        "payoff_convention": spec.get("payoff_convention", "additive_net_delta_nav"),
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
    if equity := spec.get("equity"):
        declaration = {key: equity[key] for key in ("entity", "share_class", "shares_per_unit", "initial_share_units")}
        if equity["max_tail_loss_per_share"] is not None:
            declaration["max_tail_loss_per_share"] = equity["max_tail_loss_per_share"]
        rows.append(block("equity_basis", declaration))
        rows += [block("baseline_equity_nav", {"state": state, "value": value})
                 for state, value in equity["baseline_nav"].items()]
        rows += [block("candidate_share_change", {"candidate": name, "state": state, "value": value})
                 for name, values in equity["share_changes"].items() for state, value in values.items()]
    if funding := spec.get("funding"):
        rows.append(block("funding", {key: funding[key] for key in ("cash_period", "cash_basis", "minimum_cash")}))
        rows += [block("available_cash", {"state": state, "value": value})
                 for state, value in funding["available_cash"].items()]
        rows += [block("cash_requirement", {"candidate": name, "state": state, "value": value})
                 for name, values in funding["requirements"].items() for state, value in values.items()]
    if schedule := spec.get("funding_schedule"):
        rows.append(block("funding_schedule", {key: schedule[key] for key in ("cash_basis", "opening_date")}))
        rows += [block("cash_period", row) for row in schedule["periods"]]
        rows += [block("opening_cash", {"state": state, "value": value})
                 for state, value in schedule["opening_cash"].items()]
        rows += [block("baseline_cash_flow", {"period": period, "state": state, "value": value})
                 for period, values in schedule["baseline_flows"].items() for state, value in values.items()]
        rows += [block("candidate_cash_flow", {"period": period, "candidate": name, "state": state, "value": value})
                 for period, candidates in schedule["candidate_flows"].items()
                 for name, values in candidates.items() for state, value in values.items()]
    if include_uu:
        rows.append(block("uu", UNKNOWN))
    return fields(top) + "".join(reversed(rows) if reverse else rows)


def fixture(text=None):
    # Retain all invented inputs for diagnosis; the oracle deletes no files.
    root = Path(tempfile.mkdtemp(prefix="fmine-capital-program-"))
    (root / FILE).write_text(model(funded_example()) if text is None else text, encoding="utf-8")
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


def scheduled_checkpoints(schedule, selected, states):
    periods = sorted(schedule["periods"], key=lambda row: (row["cash_period"], row["id"]))
    balances = {state: Fraction(schedule["opening_cash"][state]) for state in states}
    checkpoints = []
    first_floor = Fraction(periods[0]["minimum_cash"])
    for state in sorted(states):
        opening = balances[state]
        checkpoints.append({
            "period_id": None, "checkpoint_date": schedule["opening_date"], "state_id": state,
            "opening_cash": opening, "baseline_cash_flow": Fraction(0), "candidate_cash_flow": Fraction(0),
            "closing_cash": opening, "minimum_cash": first_floor, "slack": opening - first_floor,
        })
    for period in periods:
        period_id = period["id"]
        minimum = Fraction(period["minimum_cash"])
        for state in sorted(states):
            opening = balances[state]
            baseline = Fraction(schedule["baseline_flows"][period_id][state])
            candidate_flow = sum((Fraction(schedule["candidate_flows"][period_id][name][state])
                                  for name in selected), Fraction(0))
            closing = opening + baseline + candidate_flow
            checkpoints.append({
                "period_id": period_id, "checkpoint_date": period["cash_period"].split("/")[1],
                "state_id": state, "opening_cash": opening, "baseline_cash_flow": baseline,
                "candidate_cash_flow": candidate_flow, "closing_cash": closing,
                "minimum_cash": minimum, "slack": closing - minimum,
            })
            balances[state] = closing
    return checkpoints


def equity_states(spec, selected, states):
    equity = spec["equity"]
    scale = Fraction(equity["shares_per_unit"])
    baseline_shares = Fraction(equity["initial_share_units"]) * scale
    rows = []
    for state in sorted(states):
        baseline_nav = Fraction(equity["baseline_nav"][state])
        nav_change = sum((Fraction(spec["payoffs"][name][state]) for name in selected), Fraction(0))
        share_change = sum((Fraction(equity["share_changes"][name][state]) for name in selected), Fraction(0)) * scale
        ending_nav, ending_shares = baseline_nav + nav_change, baseline_shares + share_change
        baseline_per_share = baseline_nav / baseline_shares
        ending_per_share = ending_nav / ending_shares if ending_shares > 0 else None
        rows.append({
            "state_id": state, "baseline_nav": baseline_nav, "nav_change": nav_change,
            "ending_nav": ending_nav, "baseline_shares": baseline_shares, "share_change": share_change,
            "ending_shares": ending_shares, "baseline_nav_per_share": baseline_per_share,
            "ending_nav_per_share": ending_per_share,
            "delta_nav_per_share": None if ending_per_share is None else ending_per_share - baseline_per_share,
        })
    return rows


def payoff_statistics(values, weights, mass):
    return {
        "values": values, "mean": sum(weights[state] * value for state, value in values.items()),
        "tail_loss": weighted_tail_loss(values, weights, mass),
        "cvar95_loss": weighted_tail_loss(values, weights, Fraction(1, 20)),
        "p10": quantile(values, weights, Fraction(1, 10)),
        "p50": quantile(values, weights, Fraction(1, 2)),
        "p90": quantile(values, weights, Fraction(9, 10)),
    }


def enumerate_expected(spec):
    candidates = {row["id"]: row for row in spec["candidates"]}
    weights = {name: Fraction(weight) for name, weight in spec["states"]}
    assert sum(weights.values()) == 1
    budget, mass = Fraction(spec["budget"]), Fraction(spec["tail"])
    equity = spec.get("equity")
    cap_value = equity["max_tail_loss_per_share"] if equity else spec["cap"]
    cap = None if cap_value is None else Fraction(cap_value)
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
            if funding := spec.get("funding"):
                cash = {}
                for state in weights:
                    available = Fraction(funding["available_cash"][state])
                    required = sum((Fraction(funding["requirements"][name][state]) for name in selected), Fraction(0))
                    minimum = Fraction(funding["minimum_cash"])
                    cash[state] = {"available_cash": available, "cash_required": required,
                                   "remaining_cash": available - required, "minimum_cash": minimum,
                                   "slack": available - required - minimum}
                choice["funding"] = cash
                if any(row["slack"] < 0 for row in cash.values()):
                    reasons.add("liquidity_shortfall")
            if schedule := spec.get("funding_schedule"):
                checkpoints = scheduled_checkpoints(schedule, selected, weights)
                choice["funding_schedule"] = checkpoints
                if any(row["slack"] < 0 for row in checkpoints):
                    reasons.add("cash_schedule_shortfall")
            if equity:
                positions = equity_states(spec, selected, weights)
                choice["equity"] = positions
                if any(row["ending_shares"] <= 0 for row in positions):
                    reasons.add("nonpositive_share_count")
            if not reasons:
                values = {state: sum((Fraction(spec["payoffs"][name][state]) for name in selected), Fraction(0))
                          for state in weights}
                choice.update(payoff_statistics(values, weights, mass))
                if equity:
                    per_share_values = {row["state_id"]: row["delta_nav_per_share"] for row in positions}
                    choice["per_share"] = payoff_statistics(per_share_values, weights, mass)
                risk = choice["per_share"] if equity else choice
                choice["risk_rejected"] = cap is not None and risk["tail_loss"] > cap
            choices[selected] = choice
    structural = {key: row for key, row in choices.items() if not row["reasons"]}
    populations = [list(structural.values())]
    if equity:
        populations.append([row["per_share"] for row in structural.values()])
    for population in populations:
        hindsight = {state: max(row["values"][state] for row in population) for state in weights} if population else {}
        for row in population:
            regrets = {state: hindsight[state] - value for state, value in row["values"].items()}
            row["mean_regret"] = sum(weights[state] * value for state, value in regrets.items())
            row["max_regret"] = max(regrets.values())
            row["hindsight"], row["regrets"] = hindsight, regrets
    risk_feasible = {key: row for key, row in structural.items() if not row["risk_rejected"]}

    def objective(row):
        return row["per_share"]["mean"] if equity else row["mean"]

    def winners(population):
        if not population:
            return set()
        best = max(objective(row) for row in population.values())
        return {key for key, row in population.items() if objective(row) == best}

    price = (max(objective(row) for row in structural.values()) - max(objective(row) for row in risk_feasible.values())
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
            shortfalls = [reason for reason in row["structural_rejections"] if reason["kind"] == "liquidity_shortfall"]
            if "funding" in known:
                actual = row["funding"]
                cash = known["funding"]
                self.assertEqual([state["state_id"] for state in actual["states"]], sorted(weights))
                for state in actual["states"]:
                    for field, value in cash[state["state_id"]].items():
                        self.assertEqual(Fraction(state[field]), value, (selected, state["state_id"], field))
                self.assertEqual(Fraction(actual["minimum_slack"]), min(state["slack"] for state in cash.values()))
                expected_shortfalls = {name: -state["slack"] for name, state in cash.items() if state["slack"] < 0}
                self.assertEqual(len(shortfalls), len(expected_shortfalls))
                self.assertEqual({reason["state_id"]: Fraction(reason["shortfall"]) for reason in shortfalls}, expected_shortfalls)
            else:
                self.assertIsNone(row["funding"])
                self.assertEqual(shortfalls, [])
            dated_shortfalls = [reason for reason in row["structural_rejections"]
                                if reason["kind"] == "cash_schedule_shortfall"]
            if "funding_schedule" in known:
                actual = row["funding_schedule"]
                checkpoints = known["funding_schedule"]
                self.assertEqual(len(actual["checkpoints"]), len(checkpoints))
                for observed, declared in zip(actual["checkpoints"], checkpoints, strict=True):
                    for field in ("period_id", "checkpoint_date", "state_id"):
                        self.assertEqual(observed[field], declared[field], (selected, field))
                    for field in ("opening_cash", "baseline_cash_flow", "candidate_cash_flow",
                                  "closing_cash", "minimum_cash", "slack"):
                        self.assertEqual(Fraction(observed[field]), declared[field],
                                         (selected, declared["period_id"], declared["state_id"], field))
                self.assertEqual(Fraction(actual["minimum_slack"]), min(row["slack"] for row in checkpoints))
                expected_shortfalls = {
                    (row["period_id"], row["checkpoint_date"], row["state_id"]): -row["slack"]
                    for row in checkpoints if row["slack"] < 0
                }
                self.assertEqual(len(dated_shortfalls), len(expected_shortfalls))
                self.assertEqual({(row["period_id"], row["checkpoint_date"], row["state_id"]): Fraction(row["shortfall"])
                                  for row in dated_shortfalls}, expected_shortfalls)
            else:
                self.assertNotIn("funding_schedule", row)
                self.assertEqual(dated_shortfalls, [])
            share_rejections = [reason for reason in row["structural_rejections"]
                                if reason["kind"] == "nonpositive_share_count"]
            if "equity" in known:
                positions = row["equity"]["states"]
                self.assertEqual(len(positions), len(known["equity"]))
                for observed, declared in zip(positions, known["equity"], strict=True):
                    self.assertEqual(observed["state_id"], declared["state_id"])
                    for field, expected_value in declared.items():
                        if field == "state_id":
                            continue
                        actual_value = None if observed[field] is None else Fraction(observed[field])
                        self.assertEqual(actual_value, expected_value, (selected, declared["state_id"], field))
                invalid = {state["state_id"]: state["ending_shares"] for state in known["equity"]
                           if state["ending_shares"] <= 0}
                self.assertEqual(len(share_rejections), len(invalid))
                self.assertEqual({reason["state_id"]: Fraction(reason["shares"]) for reason in share_rejections}, invalid)
            else:
                for field in ("equity", "per_share_statistics", "per_share_tail_loss"):
                    self.assertNotIn(field, row)
                self.assertEqual(share_rejections, [])
            if known["reasons"]:
                self.assertIsNone(row["statistics"])
                self.assertIsNone(row["tail_loss"])
                if "equity" in known:
                    self.assertIsNone(row["per_share_statistics"])
                    self.assertIsNone(row["per_share_tail_loss"])
                self.assertFalse(row["risk_rejected"])
                continue
            self.assertEqual(Fraction(row["tail_loss"]), known["tail_loss"], selected)
            self.assertEqual(row["risk_rejected"], known["risk_rejected"], selected)
            self.check_statistics(row["statistics"], known, weights, (selected, "USD"))
            if "equity" in known:
                self.assertEqual(Fraction(row["per_share_tail_loss"]), known["per_share"]["tail_loss"], selected)
                self.check_statistics(row["per_share_statistics"], known["per_share"], weights, (selected, "USD/share"))
        membership = {row["bundle_id"]: selected for selected, row in rows.items()}
        self.assertEqual(len(membership), len(rows))
        for field, winners in (("unconstrained_winners", unconstrained), ("feasible_winners", feasible)):
            self.assertEqual(len(summary[field]), len(set(summary[field])))
            self.assertEqual({membership[name] for name in summary[field]}, winners)
        if "equity" in spec:
            self.assertEqual(summary["objective"], "expected_change_in_equity_nav_per_share")
            for field in ("objective_unit", "per_share_statistics_unit", "per_share_tail_loss_unit"):
                self.assertEqual(summary[field], "USD/share")
            for field in ("statistics_unit", "tail_loss_unit"):
                self.assertEqual(summary[field], "USD")
            self.assertIsNone(summary["expected_value_cost_of_risk_limit"])
            actual_price = summary["expected_per_share_cost_of_risk_limit"]
        else:
            for field in ("objective", "objective_unit", "statistics_unit", "per_share_statistics_unit",
                          "tail_loss_unit", "per_share_tail_loss_unit", "expected_per_share_cost_of_risk_limit"):
                self.assertNotIn(field, summary)
            actual_price = summary["expected_value_cost_of_risk_limit"]
        self.assertEqual(None if actual_price is None else Fraction(actual_price), price)
        status = "ok" if feasible else "no_risk_feasible_program" if unconstrained else "no_structurally_feasible_program"
        self.assertEqual(summary["status"], status)

    def check_statistics(self, observed, known, weights, context):
        for field in ("mean", "p10", "p50", "p90", "cvar95_loss", "mean_regret", "max_regret"):
            self.assertEqual(Fraction(observed[field]), known[field], (context, field))
        self.assertEqual([state["state_id"] for state in observed["states"]], sorted(weights))
        for state in observed["states"]:
            name = state["state_id"]
            self.assertEqual(Fraction(state["weight"]), weights[name])
            self.assertEqual(Fraction(state["payoff"]), known["values"][name])
            self.assertEqual(Fraction(state["best_in_hindsight"]), known["hindsight"][name])
            self.assertEqual(Fraction(state["regret"]), known["regrets"][name])

    def check_evidence(self, report, spec, source):
        inputs = report["inputs"]
        claims = [inputs["policy"], inputs["weights"], *inputs["candidates"]]
        if funding := spec.get("funding"):
            claim = inputs["funding"]
            claims.append(claim)
            value = claim["value"]
            self.assertEqual(value, report["summary"]["funding"])
            self.assertEqual(value["model_key"], inputs["policy"]["value"]["model_key"])
            self.assertEqual(value["currency"], "USD")
            self.assertEqual(value["minimum_cash_currency"], "USD")
            self.assertEqual(Fraction(value["minimum_cash"]), Fraction(funding["minimum_cash"]))
            for field in ("cash_period", "cash_basis"):
                self.assertEqual(value[field], funding[field])
            self.assertEqual(len(value["available_cash"]), len(funding["available_cash"]))
            self.assertEqual({row["state_id"]: Fraction(row["value"]) for row in value["available_cash"]},
                             {name: Fraction(amount) for name, amount in funding["available_cash"].items()})
            requirements = {(name, state): Fraction(amount) for name, states in funding["requirements"].items()
                            for state, amount in states.items()}
            self.assertEqual(len(value["requirements"]), len(requirements))
            self.assertEqual({(row["candidate_id"], row["state_id"]): Fraction(row["value"])
                              for row in value["requirements"]}, requirements)
            self.assertTrue(all(row["currency"] == "USD" for row in value["available_cash"] + value["requirements"]))
            locators = [anchor["locator"] for anchor in claim["anchors"]]
            for section in (["funding cash window, basis and minimum cash"]
                            + [f"available cash {state}" for state in funding["available_cash"]]
                            + [f"cash requirement {name}/{state}" for name, state in requirements]):
                self.assertTrue(any(locator.endswith(": " + section) for locator in locators), section)
        else:
            self.assertIsNone(inputs["funding"])
            self.assertIsNone(report["summary"]["funding"])
        if schedule := spec.get("funding_schedule"):
            claim = inputs["funding_schedule"]
            claims.append(claim)
            value = claim["value"]
            self.assertEqual(value, report["summary"]["funding_schedule"])
            self.assertEqual(value["model_key"], inputs["policy"]["value"]["model_key"])
            self.assertEqual(value["currency"], "USD")
            for field in ("cash_basis", "opening_date"):
                self.assertEqual(value[field], schedule[field])
            self.assertEqual([row["state_id"] for row in value["opening_cash"]], sorted(schedule["opening_cash"]))
            self.assertEqual({row["state_id"]: Fraction(row["value"]) for row in value["opening_cash"]},
                             {name: Fraction(amount) for name, amount in schedule["opening_cash"].items()})
            periods = sorted(schedule["periods"], key=lambda row: (row["cash_period"], row["id"]))
            self.assertEqual([row["id"] for row in value["periods"]], [row["id"] for row in periods])
            for observed, declared in zip(value["periods"], periods, strict=True):
                self.assertEqual(observed["cash_period"], declared["cash_period"])
                self.assertEqual(observed["minimum_cash_currency"], "USD")
                self.assertEqual(Fraction(observed["minimum_cash"]), Fraction(declared["minimum_cash"]))
            baseline = [
                (period["id"], state, Fraction(amount))
                for period in periods for state, amount in sorted(schedule["baseline_flows"][period["id"]].items())
            ]
            candidate_flows = [
                (period["id"], name, state, Fraction(amount))
                for period in periods for name, states in sorted(schedule["candidate_flows"][period["id"]].items())
                for state, amount in sorted(states.items())
            ]
            self.assertEqual([(row["period_id"], row["state_id"], Fraction(row["value"]))
                              for row in value["baseline_flows"]], baseline)
            self.assertEqual([(row["period_id"], row["candidate_id"], row["state_id"], Fraction(row["value"]))
                              for row in value["candidate_flows"]], candidate_flows)
            self.assertTrue(all(row["currency"] == "USD"
                                for row in value["opening_cash"] + value["baseline_flows"] + value["candidate_flows"]))
            locators = [anchor["locator"] for anchor in claim["anchors"]]
            sections = (["funding schedule opening date and cash basis"]
                        + [f"cash period {period['id']}" for period in periods]
                        + [f"opening cash {state}" for state in schedule["opening_cash"]]
                        + [f"baseline cash flow {period}/{state}" for period, state, _ in baseline]
                        + [f"candidate cash flow {period}/{name}/{state}" for period, name, state, _ in candidate_flows])
            for section in sections:
                self.assertTrue(any(locator.endswith(": " + section) for locator in locators), section)
        else:
            self.assertNotIn("funding_schedule", inputs)
            self.assertNotIn("funding_schedule", report["summary"])
        if equity := spec.get("equity"):
            claim = inputs["equity"]
            claims.append(claim)
            value = claim["value"]
            self.assertEqual(value, report["summary"]["equity"])
            self.assertEqual(value["model_key"], inputs["policy"]["value"]["model_key"])
            self.assertEqual(value["currency"], "USD")
            self.assertEqual(value["valuation_basis"], BASIS)
            for field in ("entity", "share_class"):
                self.assertEqual(value[field], equity[field])
            for field in ("shares_per_unit", "initial_share_units", "max_tail_loss_per_share"):
                self.assertEqual(None if value[field] is None else Fraction(value[field]),
                                 None if equity[field] is None else Fraction(equity[field]))
            self.assertEqual([(row["state_id"], Fraction(row["value"])) for row in value["baseline_nav"]],
                             [(state, Fraction(amount)) for state, amount in sorted(equity["baseline_nav"].items())])
            self.assertTrue(all(row["currency"] == "USD" for row in value["baseline_nav"]))
            changes = [(name, state, Fraction(units)) for name, states in sorted(equity["share_changes"].items())
                       for state, units in sorted(states.items())]
            self.assertEqual([(row["candidate_id"], row["state_id"], Fraction(row["units"]))
                              for row in value["share_changes"]], changes)
            locators = [anchor["locator"] for anchor in claim["anchors"]]
            sections = (["equity basis and per-share risk limit"]
                        + [f"baseline equity NAV {state}" for state in equity["baseline_nav"]]
                        + [f"candidate share change {name}/{state}" for name, state, _ in changes])
            for section in sections:
                self.assertTrue(any(locator.endswith(": " + section) for locator in locators), section)
        else:
            self.assertNotIn("equity", inputs)
            self.assertNotIn("equity", report["summary"])
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
        self.assertEqual(policy["payoff_convention"], spec.get("payoff_convention", "additive_net_delta_nav"))
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

    def test_joint_cash_boundary_and_one_state_overrun_change_the_winner(self):
        spec = funded_example()
        first = self.program(spec)
        joint = self.rows(first)[("a", "b")]
        self.assertEqual(first["summary"]["feasible_winners"], [joint["bundle_id"]])
        self.assertEqual(Fraction(joint["funding"]["minimum_slack"]), 0)
        self.assertEqual(Fraction(joint["spend"]), 16)
        self.assertEqual(Fraction(joint["statistics"]["mean"]), 50)
        self.assertTrue(all(Fraction(row["cash_required"]) == 100 for row in joint["funding"]["states"]))
        spec["funding"]["requirements"]["b"]["high"] = "60.0001"
        overrun = self.program(spec)
        self.assertEqual(Fraction(self.rows(overrun)[("a", "b")]["funding"]["minimum_slack"]), Fraction(-1, 10000))
        self.assertEqual(overrun["summary"]["feasible_winners"], [self.rows(overrun)[("a",)]["bundle_id"]])
        with_funding(spec, ["120", "120"], {"a": ["80", "20"], "b": ["80", "20"]})
        correlated = self.program(spec)
        spec["funding"]["requirements"]["b"] = {"low": "20", "high": "80"}
        offsetting = self.program(spec)
        self.assertEqual(Fraction(self.rows(correlated)[("a", "b")]["funding"]["minimum_slack"]), -60)
        self.assertEqual(Fraction(self.rows(offsetting)[("a", "b")]["funding"]["minimum_slack"]), 0)
        self.assertNotEqual(correlated["summary"]["feasible_winners"], offsetting["summary"]["feasible_winners"])

    def test_cash_floor_can_reject_hold_without_fabricating_risk_statistics(self):
        spec = with_funding(
            specification({"a": ["-10", "20"], "never": ["100", "100"]},
                          costs={"a": "1", "never": "100"}, budget="1", cap="-1"),
            ["19.9999", "19.9998"], {"a": ["0", "0"], "never": ["0", "0"]},
        )
        report = self.program(spec)
        self.assertEqual(report["summary"]["status"], "no_structurally_feasible_program")
        reasons = self.rows(report)[("never",)]["structural_rejections"]
        self.assertEqual({row["kind"] for row in reasons}, {"budget_exceeded", "liquidity_shortfall"})
        self.assertEqual(len([row for row in reasons if row["kind"] == "liquidity_shortfall"]), 2)
        spec["funding"]["available_cash"] = {"low": "20", "high": "20"}
        report = self.program(spec)
        self.assertEqual(report["summary"]["status"], "no_risk_feasible_program")
        self.assertIsNotNone(self.rows(report)[()]["statistics"])

    def test_funding_matrix_admits_every_candidate_and_refuses_missing_foreign_or_duplicate_rows(self):
        spec = with_funding(
            specification({"a": ["2", "3"], "never": ["100", "100"]},
                          costs={"a": "1", "never": "100"}, budget="1"),
            ["22", "22"], {"a": ["1", "1"], "never": ["0", "0"]},
        )
        self.program(spec)
        text = model(spec)
        declaration = block("funding", {key: spec["funding"][key] for key in ("cash_period", "cash_basis", "minimum_cash")})
        available = block("available_cash", {"state": "high", "value": "22"})
        zero = block("cash_requirement", {"candidate": "never", "state": "high", "value": "0"})
        for bad in (
            text.replace(zero, ""), text.replace(available, ""),
            text + zero, text + available, text + declaration,
            text.replace(declaration, ""),
            text.replace(available, block("available_cash", {"state": "absent", "value": "22"})),
            text.replace(zero, block("cash_requirement", {"candidate": "absent", "state": "high", "value": "0"})),
            text.replace(zero, block("cash_requirement", {"candidate": "never", "state": "absent", "value": "0"})),
        ):
            with self.subTest(input_tail=bad[-200:]):
                self.refused(bad)

    def test_funding_dates_basis_exact_nonnegative_money_and_sum_overflow_are_enforced(self):
        spec = funded_example()
        text = model(spec)
        declaration = {key: spec["funding"][key] for key in ("cash_period", "cash_basis", "minimum_cash")}
        for section, row, amount_key in (
            ("funding", declaration, "minimum_cash"),
            ("available_cash", {"state": "low", "value": "120"}, "value"),
            ("cash_requirement", {"candidate": "a", "state": "low", "value": "60"}, "value"),
        ):
            for amount in ("-0.0001", "0.00001"):
                with self.subTest(section=section, amount=amount):
                    self.refused(text.replace(block(section, row), block(section, {**row, amount_key: amount})))
        for field, value in (("cash_basis", ""), ("cash_period", "2030-02-30/2030-12-31"),
                             ("cash_period", "2031-01-01/2030-12-31")):
            with self.subTest(field=field, value=value):
                self.refused(text.replace(block("funding", declaration), block("funding", {**declaration, field: value})))
        large = "17014118346046923173168730371588410.5727"
        spec["funding"]["requirements"]["a"]["high"] = large
        spec["funding"]["requirements"]["b"]["high"] = large
        self.refused(model(spec), "207")

    def test_funding_identity_binds_all_inputs_without_changing_legacy_economics(self):
        spec = funded_example()
        funding = spec.pop("funding")
        legacy = self.program(spec)
        funding["available_cash"] = {"low": "1000", "high": "1000"}
        spec["funding"] = funding
        root = fixture(model(spec))
        first = self.program(spec, root)
        for selected, row in self.rows(first).items():
            for field in ("spend", "budget_slack", "statistics", "tail_loss", "risk_rejected", "structural_rejections"):
                self.assertEqual(row[field], self.rows(legacy)[selected][field])
        self.assertEqual(first["summary"]["feasible_winners"], legacy["summary"]["feasible_winners"])
        for text in (model(spec) + "\n# Source-only funding attribution edit\n", model(spec, reverse=True)):
            (root / FILE).write_text(text, encoding="utf-8")
            changed = self.program(spec, root)
            self.assertEqual(first["summary"], changed["summary"])
            self.assertNotEqual(first["digest"], changed["digest"])
            self.assertNotEqual(first["inputs"]["funding"]["id"], changed["inputs"]["funding"]["id"])
            self.assertEqual(first["inputs"]["funding"]["value_tag"], changed["inputs"]["funding"]["value_tag"])
        for container, field, value in (
            (funding, "cash_basis", CASH_BASIS + "; amended"),
            (funding, "cash_period", "2030-01-01/2031-12-31"),
            (funding, "minimum_cash", "21"),
            (funding["available_cash"], "high", "1001"),
            (funding["requirements"]["b"], "high", "60.0001"),
        ):
            with self.subTest(field=field, value=value):
                previous = container[field]
                container[field] = value
                (root / FILE).write_text(model(spec), encoding="utf-8")
                changed = self.program(spec, root)
                self.assertNotEqual(first["digest"], changed["digest"])
                self.assertNotEqual(first["claim"]["id"], changed["claim"]["id"])
                self.assertNotEqual(first["inputs"]["funding"]["value_tag"], changed["inputs"]["funding"]["value_tag"])
                self.assertEqual(first["summary"]["feasible_winners"], changed["summary"]["feasible_winners"])
                self.assertEqual(self.rows(first)[("a", "b")]["statistics"], self.rows(changed)[("a", "b")]["statistics"])
                container[field] = previous


    def test_cash_schedule_early_breaches_survive_surplus_and_financing_can_rescue_hold(self):
        report = self.program(scheduled_example())
        rows = self.rows(report)
        for selected in ((), ("plant",)):
            row = rows[selected]
            self.assertIsNone(row["statistics"])
            self.assertEqual({reason["period_id"] for reason in row["structural_rejections"]}, {"construction"})
            self.assertEqual(len(row["structural_rejections"]), 2)
            closing = [checkpoint for checkpoint in row["funding_schedule"]["checkpoints"]
                       if checkpoint["period_id"] == "operation"]
            self.assertTrue(all(Fraction(checkpoint["slack"]) > 0 for checkpoint in closing))
        bridge, funded_plant = rows[("bridge",)], rows[("bridge", "plant")]
        self.assertEqual(Fraction(bridge["statistics"]["mean"]), Fraction(-28, 5))
        self.assertEqual(Fraction(bridge["statistics"]["mean_regret"]), 68)
        self.assertEqual(Fraction(funded_plant["statistics"]["mean"]), Fraction(312, 5))
        self.assertEqual(Fraction(funded_plant["funding_schedule"]["minimum_slack"]), 0)
        self.assertEqual(report["summary"]["feasible_winners"], [funded_plant["bundle_id"]])

    def test_cash_schedule_carries_prior_balances_and_applies_period_specific_floors(self):
        spec = with_schedule(
            specification({"works": ["10", "10"]}), ["50", "50"], {},
            {"z_construction": {"works": ["-10", "-10"]}, "a_operation": {"works": ["-15", "-15"]}},
            periods=[("z_construction", "2030-01-01/2030-06-30", "20"),
                     ("a_operation", "2030-07-01/2030-12-31", "30")],
        )
        rejected = self.rows(self.program(spec))[("works",)]
        final = [row for row in rejected["funding_schedule"]["checkpoints"] if row["period_id"] == "a_operation"]
        for row in final:
            self.assertEqual(Fraction(row["opening_cash"]), 40)
            self.assertEqual(Fraction(row["closing_cash"]), 25)
            self.assertEqual(Fraction(row["minimum_cash"]), 30)
            self.assertEqual(Fraction(row["slack"]), -5)
        self.assertEqual(len(rejected["structural_rejections"]), 2)
        spec["funding_schedule"]["candidate_flows"]["a_operation"]["works"] = {"low": "-10", "high": "-10"}
        admitted = self.rows(self.program(spec))[("works",)]
        self.assertEqual(admitted["structural_rejections"], [])
        self.assertEqual(Fraction(admitted["funding_schedule"]["minimum_slack"]), 0)
        self.assertEqual(Fraction(admitted["statistics"]["mean"]), 10)

    def test_cash_schedule_reports_every_dated_breach_including_unrepairable_opening(self):
        spec = with_schedule(
            specification({"financing": ["100", "100"]}), ["19.9999", "20"],
            {"first": ["0", "-1"], "second": ["5", "0"], "recovery": ["100", "100"]},
            {"first": {"financing": ["100", "100"]}},
            periods=[("first", "2030-01-01/2030-01-31", "20"),
                     ("second", "2030-02-01/2030-02-28", "30"),
                     ("recovery", "2030-03-01/2030-03-31", "5")],
        )
        report = self.program(spec)
        self.assertEqual(report["summary"]["status"], "no_structurally_feasible_program")
        rows = self.rows(report)
        self.assertEqual(len(rows[()]["structural_rejections"]), 5)
        reasons = rows[("financing",)]["structural_rejections"]
        self.assertEqual(len(reasons), 1)
        self.assertIsNone(reasons[0]["period_id"])
        self.assertEqual(reasons[0]["checkpoint_date"], "2030-01-01")
        self.assertEqual(reasons[0]["state_id"], "low")
        self.assertEqual(Fraction(reasons[0]["shortfall"]), Fraction(1, 10000))
        for row in rows.values():
            self.assertIsNone(row["statistics"])
            self.assertTrue(all(Fraction(checkpoint["slack"]) > 0
                                for checkpoint in row["funding_schedule"]["checkpoints"]
                                if checkpoint["period_id"] == "recovery"))

    def test_cash_schedule_sole_negative_nav_financing_has_real_statistics_without_hold(self):
        spec = with_schedule(
            specification({"facility": ["-7"]}, states=[("only", "1/1")],
                          costs={"facility": "0"}, budget="0", cap="7"),
            ["30"], {"construction": ["-20"]}, {"construction": {"facility": ["10"]}},
        )
        report = self.program(spec)
        rows = self.rows(report)
        self.assertIsNone(rows[()]["statistics"])
        facility = rows[("facility",)]
        self.assertEqual(Fraction(facility["statistics"]["mean"]), -7)
        self.assertEqual(Fraction(facility["statistics"]["states"][0]["best_in_hindsight"]), -7)
        self.assertEqual(Fraction(facility["statistics"]["mean_regret"]), 0)
        self.assertEqual(Fraction(facility["tail_loss"]), 7)
        self.assertEqual(report["summary"]["feasible_winners"], [facility["bundle_id"]])
        spec["cap"] = "6.9999"
        rejected = self.program(spec)
        self.assertEqual(rejected["summary"]["status"], "no_risk_feasible_program")
        self.assertEqual(rejected["summary"]["unconstrained_winners"], [facility["bundle_id"]])
        self.assertTrue(self.rows(rejected)[("facility",)]["risk_rejected"])

    def test_cash_schedule_regret_uses_only_feasible_nonempty_financing_choices(self):
        spec = with_schedule(
            specification({"left": ["-10", "30"], "right": ["20", "-5"], "never": ["1000", "1000"]},
                          costs={"left": "1", "right": "1", "never": "100"}, budget="1"),
            ["30", "30"], {"construction": ["-20", "-20"]},
            {"construction": {"left": ["10", "10"], "right": ["10", "10"]}},
        )
        report = self.program(spec)
        rows = self.rows(report)
        self.assertEqual({selected for selected, row in rows.items() if row["statistics"] is not None},
                         {("left",), ("right",)})
        self.assertEqual(Fraction(rows[("left",)]["statistics"]["mean_regret"]), 15)
        self.assertEqual(Fraction(rows[("right",)]["statistics"]["mean_regret"]), Fraction(35, 2))
        self.assertEqual(Fraction(rows[("left",)]["statistics"]["max_regret"]), 30)
        self.assertEqual(Fraction(rows[("right",)]["statistics"]["max_regret"]), 35)
        self.assertEqual(report["summary"]["feasible_winners"], [rows[("left",)]["bundle_id"]])

    def test_cash_schedule_preserves_fractional_tail_mass_when_hold_is_infeasible(self):
        spec = with_schedule(
            specification({"facility": ["-100", "-50", "100"]},
                          states=[("crash", "1/10"), ("bad", "1/5"), ("gain", "7/10")], cap="75"),
            ["30", "30", "30"], {"construction": ["-20", "-20", "-20"]},
            {"construction": {"facility": ["10", "10", "10"]}},
        )
        report = self.program(spec)
        facility = self.rows(report)[("facility",)]
        self.assertEqual(Fraction(facility["statistics"]["mean"]), 50)
        self.assertEqual(Fraction(facility["tail_loss"]), 75)
        self.assertEqual(Fraction(facility["statistics"]["cvar95_loss"]), 100)
        self.assertEqual(Fraction(facility["statistics"]["mean_regret"]), 0)
        self.assertEqual(report["summary"]["feasible_winners"], [facility["bundle_id"]])
        spec["cap"] = "74.9999"
        self.assertEqual(self.program(spec)["summary"]["status"], "no_risk_feasible_program")

    def test_cash_schedule_uses_common_states_without_independent_candidate_resampling(self):
        spec = with_schedule(
            specification({"a": ["30", "30"], "b": ["20", "20"]}), ["120", "120"], {},
            {"construction": {"a": ["-80", "-20"], "b": ["-80", "-20"]}},
        )
        correlated = self.program(spec)
        joint = self.rows(correlated)[("a", "b")]
        self.assertEqual(Fraction(joint["funding_schedule"]["minimum_slack"]), -60)
        self.assertEqual(len(joint["structural_rejections"]), 2)
        spec["funding_schedule"]["candidate_flows"]["construction"]["b"] = {"low": "-20", "high": "-80"}
        offsetting = self.program(spec)
        joint = self.rows(offsetting)[("a", "b")]
        self.assertEqual(Fraction(joint["funding_schedule"]["minimum_slack"]), 0)
        self.assertEqual(offsetting["summary"]["feasible_winners"], [joint["bundle_id"]])
        self.assertNotEqual(correlated["summary"]["feasible_winners"], offsetting["summary"]["feasible_winners"])

    def test_cash_schedule_preserves_minor_unit_equality_before_one_unit_shortfall(self):
        spec = with_schedule(
            specification({"a": ["0.0001", "0.0001"]}), ["20.0002", "20.0002"],
            {"construction": ["-0.0001", "-0.0001"], "operation": ["0.0002", "0.0002"]},
            {"construction": {"a": ["-0.0001", "-0.0001"]}, "operation": {"a": ["-0.0001", "-0.0001"]}},
        )
        admitted = self.rows(self.program(spec))[("a",)]
        self.assertEqual(Fraction(admitted["funding_schedule"]["minimum_slack"]), 0)
        self.assertEqual(Fraction(admitted["statistics"]["mean"]), Fraction(1, 10000))
        spec["funding_schedule"]["candidate_flows"]["construction"]["a"]["high"] = "-0.0002"
        rejected = self.rows(self.program(spec))[("a",)]
        self.assertEqual(Fraction(rejected["funding_schedule"]["minimum_slack"]), Fraction(-1, 10000))
        self.assertEqual(len(rejected["structural_rejections"]), 1)
        self.assertEqual(rejected["structural_rejections"][0]["checkpoint_date"], "2030-06-30")

    def test_cash_schedule_requires_complete_unique_matrices_even_for_losing_candidates(self):
        spec = with_schedule(
            specification({"a": ["2", "3"], "never": ["100", "100"]},
                          costs={"a": "1", "never": "100"}, budget="1"),
            ["100", "100"], {}, {"construction": {"a": ["-1", "-1"]}},
        )
        self.program(spec)
        text = model(spec)
        schedule = spec["funding_schedule"]
        declaration = block("funding_schedule", {key: schedule[key] for key in ("cash_basis", "opening_date")})
        opening = block("opening_cash", {"state": "high", "value": "100"})
        baseline = block("baseline_cash_flow", {"period": "operation", "state": "high", "value": "0"})
        zero = block("candidate_cash_flow", {"period": "operation", "candidate": "never", "state": "high", "value": "0"})
        period = block("cash_period", schedule["periods"][1])
        for row in (declaration, opening, baseline, zero, period):
            for bad in (text.replace(row, ""), text + row):
                with self.subTest(matrix_row=row):
                    self.refused(bad)
        for section, row in (
            ("opening_cash", {"state": "absent", "value": "100"}),
            ("baseline_cash_flow", {"period": "absent", "state": "high", "value": "0"}),
            ("baseline_cash_flow", {"period": "operation", "state": "absent", "value": "0"}),
            ("candidate_cash_flow", {"period": "absent", "candidate": "never", "state": "high", "value": "0"}),
            ("candidate_cash_flow", {"period": "operation", "candidate": "absent", "state": "high", "value": "0"}),
            ("candidate_cash_flow", {"period": "operation", "candidate": "never", "state": "absent", "value": "0"}),
        ):
            with self.subTest(section=section, row=row):
                self.refused(text + block(section, row))
        for section, row, extra in (
            ("funding_schedule", {key: schedule[key] for key in ("cash_basis", "opening_date")}, {"assume_funded": "true"}),
            ("cash_period", schedule["periods"][0], {"ignore_shortfall": "true"}),
            ("baseline_cash_flow", {"period": "operation", "state": "high", "value": "0"}, {"currency": "EUR"}),
        ):
            with self.subTest(undeclared_field=section):
                self.refused(text.replace(block(section, row), block(section, {**row, **extra})))
        with_funding(spec, ["100", "100"], {"a": ["1", "1"], "never": ["0", "0"]})
        self.refused(model(spec))

    def test_cash_schedule_refuses_ambiguous_chronology_and_enforces_sixteen_period_bound(self):
        spec = scheduled_example()
        text = model(spec)
        schedule = spec["funding_schedule"]
        header = {key: schedule[key] for key in ("cash_basis", "opening_date")}
        for field, value in (("cash_basis", ""), ("opening_date", "2030-02-30"),
                             ("opening_date", "2029-12-31"), ("opening_date", "2030-1-1")):
            with self.subTest(field=field, value=value):
                self.refused(text.replace(block("funding_schedule", header),
                                         block("funding_schedule", {**header, field: value})))
        for index, window in (
            (0, "2030-01-02/2030-06-30"),
            (0, "2030-01-01/2030-02-30"),
            (0, "2030-06-30/2030-01-01"),
            (1, "2030-06-30/2030-12-31"),
            (1, "2030-07-02/2030-12-31"),
            (1, "2030-01-01/2030-12-31"),
            (1, "2030-07-01"),
        ):
            row = schedule["periods"][index]
            with self.subTest(window=window):
                self.refused(text.replace(block("cash_period", row), block("cash_period", {**row, "cash_period": window})))
        opening = date(2032, 2, 28)
        periods = [(f"p{i:02}", f"{opening + timedelta(days=i)}/{opening + timedelta(days=i)}", "20")
                   for i in range(17)]
        for count in (16, 17):
            bounded = with_schedule(
                specification({"a": ["1"]}, states=[("only", "1/1")]), ["20"], {}, {},
                periods=periods[:count], opening_date=str(opening),
            )
            if count == 16:
                report = self.program(bounded)
                self.assertEqual(len(self.rows(report)[()]["funding_schedule"]["checkpoints"]), 17)
                self.assertEqual(Fraction(self.rows(report)[()]["funding_schedule"]["minimum_slack"]), 0)
            else:
                self.refused(model(bounded))

    def test_cash_schedule_validates_signed_money_and_checked_aggregate_and_carry_arithmetic(self):
        spec = scheduled_example()
        text = model(spec)
        for section, row, key, values in (
            ("opening_cash", {"state": "low", "value": "100"}, "value", ("-0.0001", "0.00001")),
            ("cash_period", spec["funding_schedule"]["periods"][0], "minimum_cash", ("-0.0001", "0.00001")),
            ("baseline_cash_flow", {"period": "construction", "state": "low", "value": "-90"}, "value", ("0.00001", "-0.00001")),
            ("candidate_cash_flow", {"period": "construction", "candidate": "bridge", "state": "low", "value": "60"}, "value", ("0.00001", "-0.00001")),
        ):
            for amount in values:
                with self.subTest(section=section, amount=amount):
                    self.refused(text.replace(block(section, row), block(section, {**row, key: amount})))
        large = "17014118346046923173168730371588410.5727"
        periods = [("construction", "2030-01-01/2030-06-30", "0"),
                   ("operation", "2030-07-01/2030-12-31", "0")]
        summed = with_schedule(
            specification({"a": ["0"], "b": ["0"]}, states=[("only", "1/1")]), ["0"], {},
            {"construction": {"a": [large], "b": [large]}}, periods=periods,
        )
        self.refused(model(summed), "207")
        for sign in ("", "-"):
            carried = with_schedule(
                specification({"a": ["0"]}, states=[("only", "1/1")]), ["0"],
                {"construction": [sign + large], "operation": [sign + "1"]}, {}, periods=periods,
            )
            with self.subTest(carry_sign=sign):
                self.refused(model(carried), "207")

    def test_cash_schedule_identity_is_canonical_source_bound_and_preserves_legacy_economics(self):
        spec = specification({"a": ["30", "30"], "b": ["20", "20"], "never": ["100", "100"]},
                             costs={"a": "1", "b": "1", "never": "100"}, budget="2")
        legacy = self.program(spec)
        with_schedule(spec, ["1000", "1000"], {"construction": ["-10", "-10"], "operation": ["10", "10"]},
                      {"construction": {"a": ["-10", "-10"], "b": ["-20", "-20"]}})
        schedule = spec["funding_schedule"]
        root = fixture(model(spec))
        first = self.program(spec, root)
        self.assertEqual(first, self.program(spec, root))
        for selected, row in self.rows(first).items():
            for field in ("spend", "budget_slack", "statistics", "tail_loss", "risk_rejected", "structural_rejections"):
                self.assertEqual(row[field], self.rows(legacy)[selected][field])
        self.assertEqual(first["summary"]["feasible_winners"], legacy["summary"]["feasible_winners"])
        for text in (model(spec) + "\n# Source-only checkpoint attribution edit\n", model(spec, reverse=True)):
            (root / FILE).write_text(text, encoding="utf-8")
            changed = self.program(spec, root)
            self.assertEqual(first["summary"], changed["summary"])
            self.assertNotEqual(first["digest"], changed["digest"])
            self.assertNotEqual(first["claim"]["id"], changed["claim"]["id"])
            self.assertNotEqual(first["inputs"]["funding_schedule"]["id"], changed["inputs"]["funding_schedule"]["id"])
            self.assertEqual(first["inputs"]["funding_schedule"]["value_tag"], changed["inputs"]["funding_schedule"]["value_tag"])
        for container, field, value in (
            (schedule, "cash_basis", SCHEDULE_BASIS + "; revised"),
            (schedule["periods"][1], "cash_period", "2030-07-01/2031-12-31"),
            (schedule["periods"][0], "minimum_cash", "21"),
            (schedule["opening_cash"], "high", "1001"),
            (schedule["baseline_flows"]["construction"], "high", "-10.0001"),
            (schedule["candidate_flows"]["operation"]["never"], "high", "0.0001"),
        ):
            with self.subTest(field=field, value=value):
                previous = container[field]
                container[field] = value
                (root / FILE).write_text(model(spec), encoding="utf-8")
                changed = self.program(spec, root)
                self.assertNotEqual(first["digest"], changed["digest"])
                self.assertNotEqual(first["claim"]["id"], changed["claim"]["id"])
                self.assertNotEqual(first["inputs"]["funding_schedule"]["value_tag"], changed["inputs"]["funding_schedule"]["value_tag"])
                self.assertEqual(first["summary"]["feasible_winners"], changed["summary"]["feasible_winners"])
                self.assertEqual(self.rows(first)[("a", "b")]["statistics"], self.rows(changed)[("a", "b")]["statistics"])
                container[field] = previous
        schedule["opening_date"] = "2029-12-31"
        schedule["periods"][0]["cash_period"] = "2029-12-31/2030-06-30"
        (root / FILE).write_text(model(spec), encoding="utf-8")
        changed = self.program(spec, root)
        self.assertNotEqual(first["inputs"]["funding_schedule"]["value_tag"], changed["inputs"]["funding_schedule"]["value_tag"])
        self.assertEqual(first["summary"]["feasible_winners"], changed["summary"]["feasible_winners"])


    def test_equity_buyback_issuance_and_project_interaction_match_hand_accounting(self):
        report = self.program(equity_example())
        rows = self.rows(report)
        for selected, expected in (((), 0), (("buyback",), Fraction(2, 9)),
                                   (("project",), 1), (("buyback", "project"), Fraction(4, 3)),
                                   (("issue",), Fraction(-1, 3)), (("issue", "project"), Fraction(1, 2))):
            self.assertEqual(Fraction(rows[selected]["per_share_statistics"]["mean"]), expected)
        self.assertEqual(Fraction(rows[("buyback",)]["statistics"]["mean"]), -80)
        self.assertEqual(Fraction(rows[("issue",)]["statistics"]["mean"]), 160)
        combined = rows[("buyback", "project")]
        self.assertEqual(Fraction(combined["statistics"]["mean"]), 20)
        self.assertNotEqual(Fraction(combined["per_share_statistics"]["mean"]),
                            Fraction(rows[("buyback",)]["per_share_statistics"]["mean"])
                            + Fraction(rows[("project",)]["per_share_statistics"]["mean"]))
        self.assertEqual(report["summary"]["feasible_winners"], [combined["bundle_id"]])
        self.assertEqual(Fraction(rows[("buyback",)]["statistics"]["mean_regret"]), 340)
        self.assertEqual(Fraction(rows[("buyback",)]["per_share_statistics"]["mean_regret"]), Fraction(10, 9))
        for baseline in ("0", "-1000"):
            with self.subTest(signed_equity_nav=baseline):
                spec = with_equity(specification({"project": ["100", "100"]}), [baseline, baseline], {})
                self.assertEqual(Fraction(self.rows(self.program(spec))[("project",)]["per_share_statistics"]["mean"]), 1)

    def test_equity_objective_reverses_total_nav_ranking(self):
        spec = specification({"buyback": ["-80", "-80"], "project": ["20", "20"]},
                             costs={"buyback": "80", "project": "80"}, budget="80")
        legacy = self.program(spec)
        self.assertEqual(legacy["summary"]["feasible_winners"], [self.rows(legacy)[("project",)]["bundle_id"]])
        with_equity(spec, ["1000", "1000"], {"buyback": ["-10", "-10"]})
        report = self.program(spec)
        rows = self.rows(report)
        self.assertEqual(Fraction(rows[("buyback",)]["per_share_statistics"]["mean"]), Fraction(2, 9))
        self.assertEqual(Fraction(rows[("project",)]["per_share_statistics"]["mean"]), Fraction(1, 5))
        self.assertEqual(report["summary"]["feasible_winners"], [rows[("buyback",)]["bundle_id"]])
        for selected, row in rows.items():
            self.assertEqual(row["statistics"], self.rows(legacy)[selected]["statistics"])
            self.assertEqual(row["tail_loss"], self.rows(legacy)[selected]["tail_loss"])

    def test_equity_share_unit_normalization_preserves_actual_positions_and_exact_objective(self):
        original = equity_example()
        first = self.program(original)
        for scale in (Fraction(1_000_000), Fraction(3, 2)):
            spec = equity_example()
            equity = spec["equity"]
            equity["shares_per_unit"] = ratio(scale)
            equity["initial_share_units"] = ratio(Fraction(100) / scale)
            equity["share_changes"] = {
                name: {state: ratio(Fraction(units) / scale) for state, units in states.items()}
                for name, states in original["equity"]["share_changes"].items()
            }
            with self.subTest(shares_per_unit=scale):
                normalized = self.program(spec)
                self.assertEqual(first["summary"]["feasible_winners"], normalized["summary"]["feasible_winners"])
                self.assertNotEqual(first["inputs"]["equity"]["value_tag"], normalized["inputs"]["equity"]["value_tag"])
                for selected, row in self.rows(normalized).items():
                    for field in ("equity", "statistics", "per_share_statistics", "tail_loss", "per_share_tail_loss"):
                        self.assertEqual(row[field], self.rows(first)[selected][field], (scale, selected, field))

    def test_equity_fractional_tail_limit_changes_winner_without_reinterpreting_total_usd_risk(self):
        spec = with_equity(
            specification({"buyback": ["-80", "-80", "-80"]},
                          states=[("crash", "1/10"), ("weak", "1/5"), ("gain", "7/10")]),
            ["500", "650", "1000"], {"buyback": ["-10", "-10", "-10"]}, cap="1/4",
        )
        report = self.program(spec)
        row = self.rows(report)[("buyback",)]
        self.assertEqual(Fraction(row["per_share_statistics"]["mean"]), Fraction(4, 45))
        self.assertEqual(Fraction(row["per_share_tail_loss"]), Fraction(1, 4))
        self.assertEqual(Fraction(row["per_share_statistics"]["cvar95_loss"]), Fraction(1, 3))
        self.assertEqual(Fraction(row["tail_loss"]), 80)
        self.assertFalse(row["risk_rejected"])
        self.assertEqual(report["summary"]["feasible_winners"], [row["bundle_id"]])
        spec["equity"]["max_tail_loss_per_share"] = "2499/10000"
        limited = self.program(spec)
        self.assertTrue(self.rows(limited)[("buyback",)]["risk_rejected"])
        self.assertEqual(limited["summary"]["feasible_winners"], [self.rows(limited)[()]["bundle_id"]])
        self.assertEqual(Fraction(limited["summary"]["expected_per_share_cost_of_risk_limit"]), Fraction(4, 45))
        self.assertIsNone(limited["summary"]["expected_value_cost_of_risk_limit"])
        spec["equity"]["max_tail_loss_per_share"] = "-1/100"
        self.assertEqual(self.program(spec)["summary"]["status"], "no_risk_feasible_program")

    def test_equity_joint_state_pairing_changes_dilution_risk_and_selection(self):
        spec = with_equity(specification({"issue": ["200", "200"]}),
                           ["500", "1500"], {"issue": ["10", "30"]})
        correlated = self.program(spec)
        self.assertEqual(Fraction(self.rows(correlated)[("issue",)]["per_share_statistics"]["mean"]), Fraction(-40, 143))
        self.assertEqual(correlated["summary"]["feasible_winners"], [self.rows(correlated)[()]["bundle_id"]])
        spec["equity"]["share_changes"]["issue"] = {"low": "30/1", "high": "10/1"}
        paired = self.program(spec)
        self.assertEqual(Fraction(self.rows(paired)[("issue",)]["per_share_statistics"]["mean"]), Fraction(60, 143))
        self.assertEqual(paired["summary"]["feasible_winners"], [self.rows(paired)[("issue",)]["bundle_id"]])
        self.assertEqual(self.rows(correlated)[("issue",)]["statistics"], self.rows(paired)[("issue",)]["statistics"])

    def test_equity_nonpositive_ending_shares_reject_whole_bundles_and_retain_all_state_ledgers(self):
        spec = with_equity(
            specification({"a": ["-40", "-40"], "b": ["-40", "-40"]}, budget="1"),
            ["1000", "1000"], {"a": ["-60", "-10"], "b": ["-50", "-90"]},
        )
        report = self.program(spec)
        rejected = self.rows(report)[("a", "b")]
        self.assertEqual({reason["kind"] for reason in rejected["structural_rejections"]},
                         {"budget_exceeded", "nonpositive_share_count"})
        self.assertEqual(len(rejected["structural_rejections"]), 3)
        self.assertIsNone(rejected["statistics"])
        self.assertIsNone(rejected["per_share_statistics"])
        for state in rejected["equity"]["states"]:
            self.assertEqual(Fraction(state["ending_nav"]), 920)
            self.assertEqual(Fraction(state["ending_shares"]), {"low": -10, "high": 0}[state["state_id"]])
            self.assertIsNone(state["ending_nav_per_share"])
            self.assertIsNone(state["delta_nav_per_share"])
        # One invalid scenario rejects the alternative, without suppressing the other state's calculation.
        spec["equity"]["share_changes"]["a"] = {"low": "-100/1", "high": "-10/1"}
        rejected = self.rows(self.program(spec))[("a",)]
        self.assertIsNone(rejected["per_share_statistics"])
        states = {row["state_id"]: row for row in rejected["equity"]["states"]}
        self.assertIsNone(states["low"]["delta_nav_per_share"])
        self.assertEqual(Fraction(states["high"]["delta_nav_per_share"]), Fraction(2, 3))

    def test_equity_issuance_funds_a_project_without_charging_cash_or_proceeds_twice(self):
        spec = equity_example()
        spec["budget"] = "100"
        without_cash = self.program(spec)
        self.assertEqual(without_cash["summary"]["feasible_winners"], [self.rows(without_cash)[("project",)]["bundle_id"]])
        with_schedule(spec, ["100", "100"], {},
                      {"construction": {"buyback": ["-80", "-80"], "project": ["-100", "-100"], "issue": ["160", "160"]}})
        report = self.program(spec)
        rows = self.rows(report)
        self.assertIsNone(rows[("project",)]["per_share_statistics"])
        financed = rows[("issue", "project")]
        self.assertEqual(Fraction(financed["statistics"]["mean"]), 260)
        self.assertEqual(Fraction(financed["per_share_statistics"]["mean"]), Fraction(1, 2))
        self.assertEqual(report["summary"]["feasible_winners"], [financed["bundle_id"]])
        self.assertEqual(Fraction(rows[("buyback",)]["funding_schedule"]["minimum_slack"]), 0)
        for state in financed["equity"]["states"]:
            self.assertEqual(Fraction(state["ending_nav"]), 1260)
            self.assertEqual(Fraction(state["ending_shares"]), 120)
        self.assertTrue(all(Fraction(row["closing_cash"]) == 160
                            for row in financed["funding_schedule"]["checkpoints"] if row["period_id"] is not None))

    def test_equity_sole_feasible_dilutive_issuance_retains_negative_objective_and_zero_regret(self):
        spec = with_equity(specification({"issue": ["160", "160"]}),
                           ["1000", "1000"], {"issue": ["20", "20"]}, cap="1/3")
        with_schedule(spec, ["30", "30"], {"construction": ["-20", "-20"]},
                      {"construction": {"issue": ["160", "160"]}})
        report = self.program(spec)
        row = self.rows(report)[("issue",)]
        self.assertIsNone(self.rows(report)[()]["per_share_statistics"])
        self.assertEqual(Fraction(row["statistics"]["mean"]), 160)
        self.assertEqual(Fraction(row["per_share_statistics"]["mean"]), Fraction(-1, 3))
        self.assertEqual(Fraction(row["per_share_statistics"]["mean_regret"]), 0)
        self.assertEqual(Fraction(row["per_share_tail_loss"]), Fraction(1, 3))
        self.assertTrue(all(Fraction(state["best_in_hindsight"]) == Fraction(-1, 3)
                            for state in row["per_share_statistics"]["states"]))
        self.assertEqual(report["summary"]["feasible_winners"], [row["bundle_id"]])
        spec["equity"]["max_tail_loss_per_share"] = "3333/10000"
        self.assertEqual(self.program(spec)["summary"]["status"], "no_risk_feasible_program")

    def test_equity_one_actual_share_and_zero_changes_reproduce_legacy_statistics(self):
        spec = specification({"a": ["-10", "20"], "b": ["30", "-5"]}, tail="3/4")
        legacy = self.program(spec)
        with_equity(spec, ["1200", "800"], {}, initial="1/1")
        report = self.program(spec)
        for selected, row in self.rows(report).items():
            old = self.rows(legacy)[selected]
            self.assertEqual(row["statistics"], old["statistics"])
            self.assertEqual(row["per_share_statistics"], old["statistics"])
            self.assertEqual(row["per_share_tail_loss"], old["tail_loss"])
        self.assertEqual(report["summary"]["feasible_winners"], legacy["summary"]["feasible_winners"])

    def test_equity_complete_unique_matrices_and_explicit_convention_are_required(self):
        spec = with_equity(
            specification({"project": ["100", "100"], "never": ["0", "0"]},
                          costs={"project": "1", "never": "100"}, budget="1"),
            ["1000", "1000"], {},
        )
        self.program(spec)
        text = model(spec)
        basis = {key: spec["equity"][key] for key in ("entity", "share_class", "shares_per_unit", "initial_share_units")}
        rows = [block("equity_basis", basis), block("baseline_equity_nav", {"state": "high", "value": "1000"}),
                block("candidate_share_change", {"candidate": "never", "state": "high", "value": "0/1"})]
        for row in rows:
            for bad in (text.replace(row, ""), text + row):
                with self.subTest(matrix_row=row):
                    self.refused(bad)
        for section, row in (
            ("baseline_equity_nav", {"state": "absent", "value": "1000"}),
            ("candidate_share_change", {"candidate": "absent", "state": "high", "value": "0/1"}),
            ("candidate_share_change", {"candidate": "never", "state": "absent", "value": "0/1"}),
        ):
            with self.subTest(orphan=section, row=row):
                self.refused(text + block(section, row))
        self.refused(text.replace('payoff_convention = "additive_equity_nav_with_share_changes"',
                                 'payoff_convention = "additive_net_delta_nav"'))
        self.refused(text.replace(rows[0], block("equity_basis", {**basis, "assume_nondilutive": "true"})))
        self.refused('max_tail_loss = "10"\n' + text)
        without_equity = specification({"a": ["1", "1"]})
        without_equity["payoff_convention"] = "additive_equity_nav_with_share_changes"
        self.refused(model(without_equity))

    def test_equity_invalid_initial_shares_units_ratios_and_normalization_overflow_refuse(self):
        spec = equity_example()
        text = model(spec)
        basis = {key: spec["equity"][key] for key in ("entity", "share_class", "shares_per_unit", "initial_share_units")}
        for field in ("initial_share_units", "shares_per_unit"):
            for value in ("0/1", "-1/1", "1/0", "1.0", "NaN"):
                with self.subTest(field=field, value=value):
                    self.refused(text.replace(block("equity_basis", basis), block("equity_basis", {**basis, field: value})))
        for field in ("entity", "share_class"):
            self.refused(text.replace(block("equity_basis", basis), block("equity_basis", {**basis, field: ""})))
        change = {"candidate": "project", "state": "high", "value": "0/1"}
        for value in ("1/0", "0.1", "NaN"):
            with self.subTest(share_change=value):
                self.refused(text.replace(block("candidate_share_change", change),
                                         block("candidate_share_change", {**change, "value": value})))
        for value in ("1/0", "0.25", "NaN"):
            with self.subTest(per_share_cap=value):
                self.refused(text.replace(block("equity_basis", basis),
                                         block("equity_basis", {**basis, "max_tail_loss_per_share": value})))
        baseline = {"state": "high", "value": "1000"}
        self.refused(text.replace(block("baseline_equity_nav", baseline),
                                 block("baseline_equity_nav", {**baseline, "value": "0.00001"})))
        spec["equity"]["shares_per_unit"] = "2/1"
        spec["equity"]["initial_share_units"] = "170141183460469231731687303715884105727/1"
        self.refused(model(spec), "207")

    def test_equity_identity_binds_every_input_and_is_source_sensitive_but_order_invariant(self):
        spec = with_equity(
            specification({"project": ["100", "100"], "never": ["0", "0"]},
                          costs={"project": "1", "never": "100"}, budget="1"),
            ["1000", "1000"], {}, cap="10/1",
        )
        equity = spec["equity"]
        root = fixture(model(spec))
        first = self.program(spec, root)
        self.assertEqual(first, self.program(spec, root))
        for text in (model(spec, reverse=True), model(spec) + "\n# Source-only equity basis amendment\n"):
            (root / FILE).write_text(text, encoding="utf-8")
            changed = self.program(spec, root)
            self.assertEqual(first["summary"], changed["summary"])
            self.assertNotEqual(first["digest"], changed["digest"])
            self.assertNotEqual(first["claim"]["id"], changed["claim"]["id"])
            self.assertNotEqual(first["inputs"]["equity"]["id"], changed["inputs"]["equity"]["id"])
            self.assertEqual(first["inputs"]["equity"]["value_tag"], changed["inputs"]["equity"]["value_tag"])
        for container, field, value in (
            (equity, "entity", equity["entity"] + "; amended perimeter"),
            (equity, "share_class", "Invented amended ordinary share class"),
            (equity, "max_tail_loss_per_share", "11/1"),
            (equity["baseline_nav"], "high", "1001"),
            (equity["share_changes"]["never"], "high", "1/10000"),
        ):
            with self.subTest(field=field, value=value):
                previous = container[field]
                container[field] = value
                (root / FILE).write_text(model(spec), encoding="utf-8")
                changed = self.program(spec, root)
                self.assertNotEqual(first["digest"], changed["digest"])
                self.assertNotEqual(first["claim"]["id"], changed["claim"]["id"])
                self.assertNotEqual(first["inputs"]["equity"]["value_tag"], changed["inputs"]["equity"]["value_tag"])
                self.assertEqual(first["summary"]["feasible_winners"], changed["summary"]["feasible_winners"])
                self.assertEqual(self.rows(first)[("project",)]["per_share_statistics"],
                                 self.rows(changed)[("project",)]["per_share_statistics"])
                container[field] = previous
        equity["initial_share_units"] = "200/1"
        (root / FILE).write_text(model(spec), encoding="utf-8")
        changed = self.program(spec, root)
        self.assertNotEqual(first["inputs"]["equity"]["value_tag"], changed["inputs"]["equity"]["value_tag"])
        self.assertEqual(Fraction(self.rows(changed)[("project",)]["per_share_statistics"]["mean"]), Fraction(1, 2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--fixture-only", action="store_true")
    parser.add_argument("--schedule-fixture-only", action="store_true")
    parser.add_argument("--equity-fixture-only", action="store_true")
    options = parser.parse_args()
    BINARY = options.binary.resolve()
    if options.equity_fixture_only:
        print(fixture(model(equity_example())))
    elif options.schedule_fixture_only:
        print(fixture(model(scheduled_example())))
    elif options.fixture_only:
        print(fixture())
    else:
        unittest.main(argv=[__file__])
