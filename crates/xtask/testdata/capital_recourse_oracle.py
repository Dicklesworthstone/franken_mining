"""Independent Fraction policy oracle for the real two-stage capital command.

The policy population is constructed with combinations of initial commitments and
products of information-set continuations. Every realized path is accounted for
directly, without reading Rust's static-subset catalog or deriving expectations
from the binary. Only scalar statistics, fixture serialization and the previously
independent static checker are shared with the static capital oracle.
"""

import argparse
import copy
from fractions import Fraction
import hashlib
import importlib.util
import itertools
import json
from pathlib import Path
import tempfile
import unittest


_module = importlib.util.spec_from_file_location(
    "capital_program_reference", Path(__file__).with_name("capital_program_oracle.py"))
base = importlib.util.module_from_spec(_module)
_module.loader.exec_module(base)
FILE = base.FILE
INFORMATION = 'Invented paid observation partition, no terminal-state foresight \\ "Ω"'


def with_recourse(spec, observations, *, initial=("study",), gated=True):
    names = [candidate["id"] for candidate in spec["candidates"]]
    spec["recourse"] = {
        "information_candidate": "study", "observation_date": "2030-07-01",
        "information_basis": INFORMATION,
        "stages": {name: "initial" if name in initial else "recourse" for name in names},
        "observations": dict(zip((state for state, _ in spec["states"]), observations, strict=True)),
    }
    if gated:
        spec["requires"] += [(name, "study") for name in names if name not in initial]
    return spec


def study_example(*, grouped=False, gated=True):
    return with_recourse(base.with_schedule(base.specification(
        {"study": ["-10", "-10"], "build": ["100", "-60"]},
        states=[("good", "1/2"), ("bad", "1/2")],
        costs={"study": "10", "build": "40"}, budget="50"),
        ["100", "100"], {},
        {"construction": {"study": ["-10", "-10"]},
         "operation": {"build": ["-40", "-40"]}}),
        ["same", "same"] if grouped else ["good", "bad"], gated=gated)


def global_risk_example():
    return with_recourse(base.with_schedule(base.specification(
        {"study": ["-0.1"] * 4, "build": ["-100", "40", "-100", "20"]},
        states=[("rare_bad", "1/100"), ("rare_good", "9/100"),
                ("common_bad", "9/100"), ("common_good", "81/100")],
        costs={"study": "0.1", "build": "1"}, budget="1.1", cap="10"),
        ["100"] * 4, {},
        {"construction": {"study": ["-0.1"] * 4}, "operation": {"build": ["-1"] * 4}}),
        ["rare", "rare", "common", "common"])


def equity_example():
    return with_recourse(base.with_equity(base.with_schedule(base.specification(
        {"study": ["-10", "-10"], "issue": ["160", "160"], "project": ["100", "-100"]},
        states=[("good", "1/2"), ("bad", "1/2")],
        costs={"study": "10", "issue": "0", "project": "100"}, budget="110"),
        ["30", "30"], {},
        {"construction": {"study": ["-10", "-10"]},
         "operation": {"issue": ["160", "160"], "project": ["-100", "-100"]}}),
        ["1000", "1000"], {"issue": ["20", "20"]}, cap="1/10"), ["good", "bad"])


def adaptive_only_example():
    return with_recourse(base.with_schedule(base.specification(
        {"study": ["-1", "-1"], "A": ["5", "-5"], "B": ["-5", "5"]},
        states=[("a", "1/2"), ("b", "1/2")],
        costs={"study": "1", "A": "10", "B": "10"}, budget="11", cap="0",
        groups={"A": "alternative", "B": "alternative"}),
        ["11", "11"], {"operation": ["-2", "-2"]},
        {"construction": {"study": ["-1", "-1"]},
         "operation": {"A": ["10", "-10"], "B": ["-10", "10"]}},
        periods=[("construction", "2030-01-01/2030-06-30", "10"),
                 ("operation", "2030-07-01/2030-12-31", "10")]), ["A", "B"])


def model(spec, *, reverse=False, include_uu=True):
    recourse = spec["recourse"]
    rows = [base.block("recourse", {key: recourse[key] for key in (
        "information_candidate", "observation_date", "information_basis")})]
    rows += [base.block("candidate_stage", {"candidate": name, "stage": stage})
             for name, stage in recourse["stages"].items()]
    rows += [base.block("state_observation", {"state": state, "observation": observation})
             for state, observation in recourse["observations"].items()]
    return base.model(spec, reverse=reverse, include_uu=include_uu) + "".join(reversed(rows) if reverse else rows)


def fixture(text=None):
    # Deliberately retain invented inputs on failure; no oracle deletes files.
    root = Path(tempfile.mkdtemp(prefix="fmine-capital-recourse-"))
    (root / FILE).write_text(model(equity_example()) if text is None else text, encoding="utf-8")
    return root


def subsets(names):
    return [selected for size in range(len(names) + 1) for selected in itertools.combinations(sorted(names), size)]


def policy_key(initial, branches):
    return tuple(initial), tuple((branch["observation_id"], tuple(branch["selected"])) for branch in branches)


def objective(row, equity):
    return row["per_share"]["mean"] if equity else row["mean"]


def winners(rows, equity):
    if not rows:
        return set()
    maximum = max(objective(row, equity) for row in rows.values())
    return {key for key, row in rows.items() if objective(row, equity) == maximum}


def best(rows, chosen, equity):
    return None if not chosen else objective(rows[next(iter(chosen))], equity)


def difference(first, second):
    return None if first is None or second is None else first - second


def enumerate_policies(spec):
    candidates = {candidate["id"]: candidate for candidate in spec["candidates"]}
    weights = {state: Fraction(weight) for state, weight in spec["states"]}
    assert sum(weights.values()) == 1
    basis = spec["recourse"]
    classes = sorted(set(basis["observations"].values()))
    initial = [name for name, stage in basis["stages"].items() if stage == "initial"]
    later = [name for name, stage in basis["stages"].items() if stage == "recourse"]
    equity = spec.get("equity")
    cap = equity["max_tail_loss_per_share"] if equity else spec["cap"]
    cap = None if cap is None else Fraction(cap)
    mass, budget = Fraction(spec["tail"]), Fraction(spec["budget"])
    periods = sorted(spec["funding_schedule"]["periods"], key=lambda row: (row["cash_period"], row["id"]))
    checkpoint_order = {None: 0, **{row["id"]: index + 1 for index, row in enumerate(periods)}}
    population = {}
    for committed in subsets(initial):
        acquired = basis["information_candidate"] in committed
        observed = classes if acquired else [None]
        for decisions in itertools.product(subsets(later), repeat=len(observed)):
            branches = [{
                "observation_id": observation, "selected": selected,
                "probability": sum((weight for state, weight in weights.items()
                                    if not acquired or basis["observations"][state] == observation), Fraction(0)),
                "spend": sum((Fraction(candidates[name]["cost"]) for name in selected), Fraction(0)),
            } for observation, selected in zip(observed, decisions, strict=True)]
            continuation = dict(zip(observed, decisions, strict=True))
            row = {"initial": committed, "information_acquired": acquired, "branches": branches,
                   "initial_spend": sum((Fraction(candidates[name]["cost"]) for name in committed), Fraction(0)),
                   "states": {}, "rejections": [], "checkpoints": [], "equity": [], "values": {}}
            for state in sorted(weights):
                observation = basis["observations"][state] if acquired else None
                selected = tuple(sorted((*committed, *continuation[observation])))
                spend = sum((Fraction(candidates[name]["cost"]) for name in selected), Fraction(0))
                nav = sum((Fraction(spec["payoffs"][name][state]) for name in selected), Fraction(0))
                row["states"][state] = {"selected": selected, "observation_id": observation,
                                        "spend": spend, "budget_slack": budget - spend, "net_nav": nav}
                row["values"][state] = nav
                reasons = []
                if spend > budget:
                    reasons.append({"kind": "budget_exceeded"})
                reasons += [{"kind": "missing_prerequisite", "candidate": name, "prerequisite": prerequisite}
                            for name, prerequisite in spec["requires"] if name in selected and prerequisite not in selected]
                groups = {}
                for name in selected:
                    if group := candidates[name].get("exclusion_group"):
                        groups.setdefault(group, []).append(name)
                reasons += [{"kind": "mutual_exclusion", "group": group, "candidates": members}
                            for group, members in sorted(groups.items()) if len(members) > 1]
                checkpoints = base.scheduled_checkpoints(spec["funding_schedule"], selected, [state])
                row["checkpoints"].extend(checkpoints)
                reasons += [{"kind": "cash_schedule_shortfall", "period_id": cash["period_id"],
                             "checkpoint_date": cash["checkpoint_date"], "state_id": state, "shortfall": -cash["slack"]}
                            for cash in checkpoints if cash["slack"] < 0]
                if equity:
                    position = base.equity_states(spec, selected, [state])[0]
                    row["equity"].append(position)
                    if position["ending_shares"] <= 0:
                        reasons.append({"kind": "nonpositive_share_count", "state_id": state,
                                        "shares": position["ending_shares"]})
                row["rejections"] += [{"state_id": state, "reason": reason} for reason in reasons]
            row["checkpoints"].sort(key=lambda cash: (checkpoint_order[cash["period_id"]], cash["state_id"]))
            if not row["rejections"]:
                row.update(base.payoff_statistics(row["values"], weights, mass))
                if equity:
                    values = {state["state_id"]: state["delta_nav_per_share"] for state in row["equity"]}
                    row["per_share"] = base.payoff_statistics(values, weights, mass)
                row["risk_rejected"] = cap is not None and (row["per_share"] if equity else row)["tail_loss"] > cap
            population[policy_key(committed, branches)] = row
    structural = {key: row for key, row in population.items() if not row["rejections"]}
    for normalized in (False, True) if equity else (False,):
        rows = [row["per_share"] if normalized else row for row in structural.values()]
        hindsight = {state: max(row["values"][state] for row in rows) for state in weights} if rows else {}
        for row in rows:
            regrets = {state: hindsight[state] - value for state, value in row["values"].items()}
            row.update(hindsight=hindsight, regrets=regrets,
                       mean_regret=sum(weights[state] * value for state, value in regrets.items()),
                       max_regret=max(regrets.values()))
    feasible = {key: row for key, row in structural.items() if not row["risk_rejected"]}
    before, after = winners(structural, equity), winners(feasible, equity)
    static, _, static_before, static_after, _ = base.enumerate_expected(spec)
    before_value, after_value = best(population, before, equity), best(population, after, equity)
    return {"policies": population, "weights": weights, "unconstrained": before, "feasible": after,
            "risk_cost": difference(before_value, after_value),
            "premium": difference(after_value, best(static, static_after, equity)),
            "unconstrained_premium": difference(before_value, best(static, static_before, equity)),
            "static": static, "static_unconstrained": static_before, "static_feasible": static_after}


def serialized(value):
    if isinstance(value, Fraction):
        return base.ratio(value)
    if isinstance(value, dict):
        return {key: serialized(item) for key, item in value.items()}
    if isinstance(value, (tuple, list)):
        return [serialized(item) for item in value]
    return value


class CapitalRecourseScenarios(unittest.TestCase):
    envelope = base.CapitalProgramScenarios.envelope
    refused = base.CapitalProgramScenarios.refused
    check_statistics = base.CapitalProgramScenarios.check_statistics

    def program(self, spec, root=None):
        root = fixture(model(spec)) if root is None else root
        raw = (root / FILE).read_bytes()
        envelope = self.envelope(root)
        self.assertEqual(envelope["exit_code"], 0, envelope)
        self.assertIsNone(envelope["refusal"])
        payload = envelope["payload"]
        self.assertEqual(payload["mode"], "declared_two_stage_capital_comparison")
        source = {"path": str(root / FILE), "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}
        self.assertEqual(payload["source"], source)
        self.assertEqual((root / FILE).read_bytes(), raw)
        report = payload["program"]
        self.assertEqual(report["schema"], "fmine.capital-recourse.v1")
        self.assertEqual(len(bytes.fromhex(report["digest"])), 32)
        self.assertTrue(report["no_claim"])
        for flag in ("reserve_disclosure_claim", "execution_authorized", "geotechnical_signoff", "regulatory_compliance_claim"):
            self.assertFalse(report[flag])
        self.check_policies(report, spec)
        base.CapitalProgramScenarios.check_arithmetic(self, report["static_comparison"], spec)
        base.CapitalProgramScenarios.check_evidence(self, report["static_comparison"], spec, source)
        self.check_information(report, spec, source)
        return report

    def check_policies(self, report, spec):
        expected = enumerate_policies(spec)
        summary = report["summary"]
        rows = {policy_key(row["initial_selected"], row["branches"]): row for row in summary["policies"]}
        self.assertEqual(len(rows), len(summary["policies"]))
        self.assertEqual(set(rows), set(expected["policies"]))
        weights = expected["weights"]
        static_rows = {row["bundle_id"]: tuple(row["selected"])
                       for row in report["static_comparison"]["summary"]["choices"]}
        for key, row in rows.items():
            known = expected["policies"][key]
            self.assertEqual(row["information_acquired"], known["information_acquired"])
            self.assertEqual(Fraction(row["initial_spend"]), known["initial_spend"])
            self.assertEqual(row["branches"], serialized(known["branches"]))
            self.assertEqual([state["state_id"] for state in row["states"]], sorted(weights))
            for actual in row["states"]:
                declared = known["states"][actual["state_id"]]
                self.assertEqual(tuple(actual["selected"]), declared["selected"])
                self.assertEqual(static_rows[actual["bundle_id"]], declared["selected"])
                self.assertEqual(actual["observation_id"], declared["observation_id"])
                for field in ("spend", "budget_slack", "net_nav"):
                    self.assertEqual(Fraction(actual[field]), declared[field], (key, actual["state_id"], field))
            self.assertCountEqual(row["structural_rejections"], serialized(known["rejections"]))
            self.assertEqual(row["funding_schedule"]["checkpoints"], serialized(known["checkpoints"]))
            self.assertEqual(Fraction(row["funding_schedule"]["minimum_slack"]),
                             min(cash["slack"] for cash in known["checkpoints"]))
            self.assertEqual(row["equity"], {"states": serialized(known["equity"])} if "equity" in spec else None)
            if known["rejections"]:
                for field in ("statistics", "tail_loss", "per_share_statistics", "per_share_tail_loss"):
                    self.assertIsNone(row[field])
                self.assertFalse(row["risk_rejected"])
                continue
            self.assertEqual(Fraction(row["tail_loss"]), known["tail_loss"])
            self.assertEqual(row["risk_rejected"], known["risk_rejected"])
            self.check_statistics(row["statistics"], known, weights, (key, "USD"))
            if "equity" in spec:
                self.assertEqual(Fraction(row["per_share_tail_loss"]), known["per_share"]["tail_loss"])
                self.check_statistics(row["per_share_statistics"], known["per_share"], weights, (key, "USD/share"))
            else:
                self.assertIsNone(row["per_share_statistics"])
                self.assertIsNone(row["per_share_tail_loss"])
        membership = {row["policy_id"]: key for key, row in rows.items()}
        self.assertEqual(len(membership), len(rows))
        for field, winners_key in (("unconstrained_winners", "unconstrained"), ("feasible_winners", "feasible")):
            self.assertEqual({membership[name] for name in summary[field]}, expected[winners_key])
            self.assertEqual(len(summary[field]), len(set(summary[field])))
        equity = "equity" in spec
        for dollars, per_share, known in (
            ("expected_value_cost_of_risk_limit", "expected_per_share_cost_of_risk_limit", expected["risk_cost"]),
            ("net_adaptive_value", "net_adaptive_value_per_share", expected["premium"]),
            ("unconstrained_net_adaptive_value", "unconstrained_net_adaptive_value_per_share", expected["unconstrained_premium"]),
        ):
            active, inactive = (per_share, dollars) if equity else (dollars, per_share)
            self.assertEqual(None if summary[active] is None else Fraction(summary[active]), known)
            self.assertIsNone(summary[inactive])
        self.assertEqual(summary["objective_unit"], "USD/share" if equity else "USD")
        self.assertEqual(summary["per_share_objective"], equity)
        self.assertEqual(summary["statistics_unit"], "USD")
        self.assertEqual(summary["per_share_statistics_unit"], "USD/share")
        self.assertEqual(summary["regret_reference"], "all_structurally_feasible_policies_before_risk_limit")
        self.assertIsNone(summary["selection_frequency"])
        self.assertEqual(summary["status"], "ok" if expected["feasible"] else
                         "no_risk_feasible_policy" if expected["unconstrained"] else "no_structurally_feasible_policy")
        self.assertEqual(summary["net_adaptive_value_status"], "available" if expected["premium"] is not None else
                         "no_risk_feasible_policy" if not expected["feasible"] else "no_feasible_static_comparator")
        self.assertEqual(summary["unconstrained_net_adaptive_value_status"],
                         "available" if expected["unconstrained_premium"] is not None else
                         "no_structurally_feasible_policy" if not expected["unconstrained"] else
                         "no_structurally_feasible_static_comparator")
        # Every static commitment is representable by a constant continuation, including gate-off choices.
        for selected, static in expected["static"].items():
            initial = tuple(name for name in selected if spec["recourse"]["stages"][name] == "initial")
            later = tuple(name for name in selected if name not in initial)
            gate = spec["recourse"]["information_candidate"]
            signals = sorted(set(spec["recourse"]["observations"].values())) if gate in initial else [None]
            key = initial, tuple((signal, later) for signal in signals)
            self.assertIn(key, rows)
            policy = expected["policies"][key]
            self.assertEqual(bool(policy["rejections"]), bool(static["reasons"]))
            if not static["reasons"]:
                self.assertEqual(policy["values"], static["values"])
                self.assertEqual(policy["tail_loss"], static["tail_loss"])
                self.assertEqual(policy["risk_rejected"], static["risk_rejected"])

    def check_information(self, report, spec, source):
        claim = report["inputs"]["information"]
        value, basis = claim["value"], spec["recourse"]
        self.assertEqual(value, report["summary"]["information"])
        for field in ("information_candidate", "observation_date", "information_basis"):
            self.assertEqual(value[field], basis[field])
        self.assertEqual(value["candidate_stages"], [
            {"candidate_id": name, "stage": stage} for name, stage in sorted(basis["stages"].items())])
        self.assertEqual(value["observations"], [
            {"state_id": state, "observation_id": observation} for state, observation in sorted(basis["observations"].items())])
        self.assertEqual(value["model_key"], "invented_capital_program")
        self.assertEqual(value["currency"], "USD")
        self.assertEqual(value["valuation_basis"], base.BASIS)
        locators = [anchor["locator"] for anchor in claim["anchors"]]
        sections = (["recourse information gate, observation date and basis"]
                    + [f"candidate stage {name}" for name in basis["stages"]]
                    + [f"state observation {state}/{observation}"
                       for state, observation in basis["observations"].items()])
        for section in sections:
            self.assertTrue(any(locator.endswith(": " + section) for locator in locators), section)
        static_parents = set(report["static_comparison"]["claim"]["parents"])
        self.assertEqual(set(report["claim"]["parents"]), static_parents | {claim["id"]})
        self.assertEqual(report["claim"]["value"], report["summary"])
        for item in (claim, report["claim"]):
            self.assertTrue(bytes.fromhex(item["canonical_body_hex"]))
            self.assertEqual(item["representation"], "deterministic")
            self.assertEqual(item["support"], "period_aggregate")
            self.assertEqual(item["evidence"], "assumed")
            self.assertEqual(item["state"], "estimated")
            self.assertFalse(item["stale"])
            self.assertTrue(item["uu_terms"])
            self.assertTrue(item["anchors"])
            self.assertTrue(all(anchor["artifact_path"] == source["path"] and anchor["sha256"] == source["sha256"]
                                and anchor["locator"] for anchor in item["anchors"]))
        self.assertTrue(all(anchor in report["claim"]["anchors"] for anchor in claim["anchors"]))

    @staticmethod
    def find(report, branches, initial=("study",)):
        key = tuple(initial), tuple((name, tuple(selected)) for name, selected in branches)
        return next(row for row in report["summary"]["policies"]
                    if policy_key(row["initial_selected"], row["branches"]) == key)

    def test_study_is_sunk_on_abandonment_and_information_has_exact_value(self):
        report = self.program(study_example())
        winner = self.find(report, [("bad", ()), ("good", ("build",))])
        self.assertEqual(report["summary"]["feasible_winners"], [winner["policy_id"]])
        self.assertEqual(Fraction(winner["statistics"]["mean"]), 40)
        self.assertEqual(Fraction(report["summary"]["net_adaptive_value"]), 30)
        self.assertEqual(len(report["summary"]["policies"]), 6)
        abandoned = next(state for state in winner["states"] if state["state_id"] == "bad")
        self.assertEqual((Fraction(abandoned["net_nav"]), Fraction(abandoned["spend"])), (-10, 10))

    def test_coarse_information_cannot_reveal_terminal_states(self):
        report = self.program(study_example(grouped=True))
        self.assertEqual(len(report["summary"]["policies"]), 4)
        self.assertEqual(Fraction(report["summary"]["net_adaptive_value"]), 0)
        winner = self.find(report, [("same", ("build",))])
        self.assertEqual(Fraction(winner["statistics"]["mean"]), 10)

    def test_without_the_gate_only_one_uniform_continuation_is_allowed(self):
        report = self.program(study_example(gated=False))
        common = self.find(report, [(None, ("build",))], initial=())
        self.assertEqual(Fraction(common["statistics"]["mean"]), 20)
        self.assertEqual(Fraction(report["summary"]["net_adaptive_value"]), 20)
        self.assertEqual(len(report["summary"]["policies"]), 6)

    def test_unconditional_fractional_tail_selects_neither_greedy_nor_nodewise_solution(self):
        report = self.program(global_risk_example())
        winner = self.find(report, [("common", ()), ("rare", ("build",))])
        self.assertEqual(report["summary"]["feasible_winners"], [winner["policy_id"]])
        self.assertEqual(Fraction(winner["statistics"]["mean"]), Fraction(5, 2))
        self.assertEqual(Fraction(winner["tail_loss"]), Fraction(51, 10))
        self.assertEqual(Fraction(winner["statistics"]["cvar95_loss"]), Fraction(201, 10))
        self.assertEqual(Fraction(report["summary"]["expected_value_cost_of_risk_limit"]), Fraction(36, 5))
        self.assertEqual(Fraction(report["summary"]["net_adaptive_value"]), Fraction(5, 2))
        self.assertEqual(Fraction(report["summary"]["unconstrained_net_adaptive_value"]), 0)

    def test_equity_and_cash_realize_one_policy_with_unit_isolated_statistics(self):
        report = self.program(equity_example())
        winner = self.find(report, [("bad", ()), ("good", ("issue", "project"))])
        self.assertEqual(report["summary"]["feasible_winners"], [winner["policy_id"]])
        self.assertEqual(Fraction(winner["statistics"]["mean"]), 120)
        self.assertEqual(Fraction(winner["per_share_statistics"]["mean"]), Fraction(19, 120))
        self.assertEqual(Fraction(winner["per_share_tail_loss"]), Fraction(1, 10))
        self.assertEqual(Fraction(report["summary"]["net_adaptive_value_per_share"]), Fraction(19, 120))
        cash = {row["state_id"]: Fraction(row["closing_cash"]) for row in winner["funding_schedule"]["checkpoints"]
                if row["period_id"] == "operation"}
        self.assertEqual(cash, {"bad": Fraction(20), "good": Fraction(80)})
        self.assertEqual(len(report["summary"]["policies"]), 20)

    def test_share_unit_rescaling_preserves_policy_economics(self):
        spec = equity_example()
        first = self.program(spec)
        spec["equity"]["shares_per_unit"] = "1000/1"
        spec["equity"]["initial_share_units"] = "1/10"
        spec["equity"]["share_changes"]["issue"] = {"good": "1/50", "bad": "1/50"}
        scaled = self.program(spec)
        self.assertEqual(first["summary"]["policies"], scaled["summary"]["policies"])
        self.assertNotEqual(first["digest"], scaled["digest"])

    def test_an_unselected_invalid_share_state_does_not_reject_an_adaptive_path(self):
        spec = base.with_equity(study_example(), ["1000", "1000"], {"build": ["0", "-100"]})
        report = self.program(spec)
        winner = self.find(report, [("bad", ()), ("good", ("build",))])
        self.assertEqual(Fraction(winner["per_share_statistics"]["mean"]), Fraction(2, 5))
        invalid = self.find(report, [("bad", ("build",)), ("good", ("build",))])
        self.assertIsNone(invalid["per_share_statistics"])
        self.assertTrue(any(row["reason"]["kind"] == "nonpositive_share_count" for row in invalid["structural_rejections"]))

    def test_future_financing_cannot_repair_preobservation_shortfall(self):
        spec = equity_example()
        spec["funding_schedule"]["opening_cash"] = {"good": "29.9999", "bad": "29.9999"}
        report = self.program(spec)
        hold = self.find(report, [(None, ())], initial=())
        self.assertEqual(report["summary"]["feasible_winners"], [hold["policy_id"]])
        recovered = self.find(report, [("bad", ()), ("good", ("issue", "project"))])
        self.assertTrue(any(row["reason"]["kind"] == "cash_schedule_shortfall"
                            and row["reason"]["period_id"] == "construction"
                            and Fraction(row["reason"]["shortfall"]) == Fraction(1, 10000)
                            for row in recovered["structural_rejections"]))

    def test_mutually_exclusive_branches_can_finance_a_policy_with_no_static_comparator(self):
        report = self.program(adaptive_only_example())
        winner = self.find(report, [("A", ("A",)), ("B", ("B",))])
        self.assertEqual(report["summary"]["feasible_winners"], [winner["policy_id"]])
        self.assertEqual(Fraction(winner["statistics"]["mean"]), 4)
        self.assertEqual(Fraction(winner["statistics"]["mean_regret"]), 0)
        self.assertEqual(Fraction(winner["statistics"]["max_regret"]), 0)
        self.assertIsNone(report["summary"]["net_adaptive_value"])
        self.assertEqual(report["summary"]["net_adaptive_value_status"], "no_feasible_static_comparator")

    def test_abandonment_retains_an_initial_commitments_later_cash_obligation(self):
        spec = equity_example()
        spec["payoffs"]["study"] = {"good": "-11", "bad": "-11"}
        spec["funding_schedule"]["candidate_flows"]["operation"]["study"] = {"good": "-1", "bad": "-1"}
        report = self.program(spec)
        hold = self.find(report, [(None, ())], initial=())
        self.assertEqual(report["summary"]["feasible_winners"], [hold["policy_id"]])
        abandoned = self.find(report, [("bad", ()), ("good", ("issue", "project"))])
        self.assertIsNone(abandoned["statistics"])
        self.assertIsNone(abandoned["per_share_statistics"])
        self.assertEqual({row["state_id"]: Fraction(row["net_nav"]) for row in abandoned["states"]},
                         {"good": Fraction(249), "bad": Fraction(-11)})
        cash = {row["state_id"]: Fraction(row["closing_cash"])
                for row in abandoned["funding_schedule"]["checkpoints"] if row["period_id"] == "operation"}
        self.assertEqual(cash, {"good": Fraction(79), "bad": Fraction(19)})
        self.assertTrue(any(row["reason"]["kind"] == "cash_schedule_shortfall"
                            and row["reason"]["period_id"] == "operation"
                            and row["reason"]["state_id"] == "bad"
                            and Fraction(row["reason"]["shortfall"]) == 1
                            for row in abandoned["structural_rejections"]))

    def test_multiple_initial_commitments_are_retained_in_every_branch(self):
        spec = base.specification({"study": ["-2", "-2"], "permit": ["3", "3"], "build": ["20", "-10"]},
                                  states=[("good", "1/2"), ("bad", "1/2")],
                                  costs={"study": "2", "permit": "1", "build": "5"}, budget="8")
        spec = with_recourse(base.with_schedule(spec, ["100", "100"], {},
                             {"construction": {"study": ["-2", "-2"], "permit": ["-1", "-1"]},
                              "operation": {"build": ["-5", "-5"]}}), ["good", "bad"], initial=("study", "permit"))
        report = self.program(spec)
        self.assertEqual(len(report["summary"]["policies"]), 12)
        winner = self.find(report, [("bad", ()), ("good", ("build",))], initial=("permit", "study"))
        self.assertEqual(report["summary"]["feasible_winners"], [winner["policy_id"]])
        self.assertEqual(Fraction(winner["statistics"]["mean"]), 11)

    def test_empty_feasible_populations_are_reported_without_invented_comparators(self):
        spec = study_example()
        spec["cap"] = "-1000"
        report = self.program(spec)
        self.assertEqual(report["summary"]["status"], "no_risk_feasible_policy")
        spec["cap"] = None
        spec["funding_schedule"]["opening_cash"] = {"good": "19", "bad": "19"}
        report = self.program(spec)
        self.assertEqual(report["summary"]["status"], "no_structurally_feasible_policy")
        self.assertEqual(len(report["summary"]["policies"]), 6)

    def test_reruns_are_identical_and_source_order_assumptions_and_losers_bind(self):
        spec = study_example()
        root = fixture(model(spec))
        first = self.program(spec, root)
        self.assertEqual(first, self.program(spec, root))
        (root / FILE).write_text(model(spec, reverse=True), encoding="utf-8")
        reordered = self.program(spec, root)
        self.assertEqual(first["summary"], reordered["summary"])
        self.assertNotEqual(first["digest"], reordered["digest"])
        self.assertNotEqual(first["inputs"]["information"]["id"], reordered["inputs"]["information"]["id"])
        changed = copy.deepcopy(spec)
        changed["recourse"]["information_basis"] += "; amended declaration"
        (root / FILE).write_text(model(changed), encoding="utf-8")
        amended = self.program(changed, root)
        self.assertEqual(first["summary"]["policies"], amended["summary"]["policies"])
        self.assertNotEqual(first["inputs"]["information"]["value_tag"], amended["inputs"]["information"]["value_tag"])
        self.assertNotEqual(first["digest"], amended["digest"])
        changed = copy.deepcopy(spec)
        changed["payoffs"]["build"]["bad"] = "-61"
        (root / FILE).write_text(model(changed), encoding="utf-8")
        losing = self.program(changed, root)
        self.assertEqual(first["summary"]["feasible_winners"], losing["summary"]["feasible_winners"])
        self.assertNotEqual(first["digest"], losing["digest"])

    def test_parser_refuses_unknown_duplicate_incomplete_and_orphan_information(self):
        text = model(study_example())
        cases = [
            text + base.block("candidate_stage", {"candidate": "build", "stage": "recourse"}),
            text + base.block("state_observation", {"state": "bad", "observation": "bad"}),
            text + base.block("recourse", {"information_candidate": "study", "observation_date": "2030-07-01", "information_basis": INFORMATION}),
            text.replace(base.block("candidate_stage", {"candidate": "build", "stage": "recourse"}), ""),
            text.replace(base.block("state_observation", {"state": "bad", "observation": "bad"}), ""),
            text.replace('stage = "recourse"', 'stage = "clairvoyant"'),
            text.replace('information_candidate = "study"', 'information_candidate = "foreign"'),
            text.replace('observation = "bad"', 'observation = "contains spaces"'),
            text.replace('observation_date = "2030-07-01"', 'observation_date = "2030-06-30"'),
            text.replace(base.block("recourse", {"information_candidate": "study", "observation_date": "2030-07-01", "information_basis": INFORMATION}), ""),
            model(study_example(), include_uu=False),
        ]
        for malformed in cases:
            with self.subTest(text=malformed[-250:]):
                self.refused(malformed)
        self.refused(text + base.block("terminal_state_override", {"state": "bad", "candidate": "build"}), "506")
        self.refused(text.replace('information_candidate = "study"', 'information_candidate = "study"\nunknown_policy_field = "yes"'), "506")

    def test_future_cash_and_free_information_are_refused_before_selection(self):
        for amendment in ("early_recourse", "unpaid", "nonnegative_nav", "deferred_cost", "missing_cash"):
            spec = study_example()
            if amendment == "early_recourse":
                spec["funding_schedule"]["candidate_flows"]["construction"]["build"]["bad"] = "1"
            elif amendment == "unpaid":
                next(row for row in spec["candidates"] if row["id"] == "study")["cost"] = "0"
            elif amendment == "nonnegative_nav":
                spec["payoffs"]["study"]["bad"] = "0"
            elif amendment == "deferred_cost":
                spec["funding_schedule"]["candidate_flows"]["construction"]["study"]["bad"] = "0"
                spec["funding_schedule"]["candidate_flows"]["operation"]["study"]["bad"] = "-10"
            else:
                del spec["funding_schedule"]
            with self.subTest(amendment=amendment):
                self.refused(model(spec))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--fixture-only", action="store_true")
    options = parser.parse_args()
    base.BINARY = options.binary.resolve()
    if options.fixture_only:
        print(fixture())
    else:
        unittest.main(argv=[__file__])
