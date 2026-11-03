"""Independent Fraction checks for complete multi-entity capital comparisons.

Each entity's ending NAV and actual share count are calculated directly from its
declared candidate maps, before averaging. All entities share one structural
population; their separate tail limits intersect and the primary entity alone
ranks choices. Policy paths are enumerated directly by information sets, without
reading Rust's catalog, indices, intermediate statistics or selected alternatives.
Existing independent cash/debt references and scalar statistics are reused.
Every example is invented and makes no claim about issuer transaction terms.
"""

import argparse
import copy
from fractions import Fraction as F
import hashlib
import importlib.util
import itertools
from pathlib import Path
import tempfile
import unittest

_module = importlib.util.spec_from_file_location("debt_reference", Path(__file__).with_name("capital_debt_oracle.py"))
debt = importlib.util.module_from_spec(_module)
_module.loader.exec_module(debt)
entity = debt.entity
base = entity.base
FILE = base.FILE
VALUATION_BASIS = 'Invented separately supplied entity equity NAV, already net of all transaction economics \\ "Ω"'


def with_valuation(spec, name, baseline, nav, shares, *, initial="60/1", scale="1/1", cap=None):
    states = [state for state, _ in spec["states"]]
    zeros = ["0/1"] * len(states)
    spec.setdefault("entity_valuations", []).append({
        "entity": name, "share_class": "Invented ordinary shares", "valuation_basis": VALUATION_BASIS,
        "shares_per_unit": base.ratio(scale), "initial_share_units": base.ratio(initial),
        "max_tail_loss_per_share": None if cap is None else base.ratio(cap),
        "baseline_nav": dict(zip(states, map(str, baseline), strict=True)),
        "nav_changes": {row["id"]: dict(zip(states, map(str, nav[row["id"]]), strict=True)) for row in spec["candidates"]},
        "share_changes": {row["id"]: dict(zip(states, map(base.ratio, shares.get(row["id"], zeros)), strict=True))
                          for row in spec["candidates"]},
    })
    return spec


def primary(spec, baseline=None, cap=None):
    base.with_equity(spec, [1000] * len(spec["states"]) if baseline is None else baseline, {}, cap=cap)
    spec["equity"]["entity"] = "parent"
    return spec


def static_example():
    return with_valuation(primary(entity.with_entities(base.specification(
        {"aggressive": [100, 100], "fair": [60, 60]}, states=[("good", "1/2"), ("bad", "1/2")],
        costs={"aggressive": "0", "fair": "0"}, budget="0", groups={"aggressive": "deal", "fair": "deal"}),
        [("parent", 100, 20)])), "carve-out", [600, 600],
        {"aggressive": [120, 120], "fair": [126, 126]}, {"aggressive": [30, 30], "fair": [6, 6]}, cap="0")


def recourse_example():
    spec = primary(entity.with_entities(base.specification(
        {"study": [-10, -10], "aggressive": [160, -80], "fair": [100, -40]},
        states=[("good", "1/2"), ("bad", "1/2")], costs={"study": "10", "aggressive": "20", "fair": "20"},
        budget="30", groups={"aggressive": "deal", "fair": "deal"}, requires=[("aggressive", "study"), ("fair", "study")]),
        [("parent", 30, 20), ("carve-out", 20, 20)], entity.THREE_PERIODS), cap="1/10")
    entity.native(spec, "first", "parent", "study", [-10, -10])
    for deal in ("aggressive", "fair"):
        entity.native(spec, "third", "parent", deal, [-20, -20])
    spec["entity_funding"]["baseline_flows"]["second"]["carve-out"] = {"good": "50", "bad": "0"}
    entity.distribution(spec, "carve-out", "parent", "3/5", "50")
    spec["recourse"] = {
        "information_candidate": "study", "observation_date": "2030-05-01",
        "information_basis": "Invented paid observation before a prior dividend funds the terminal deal",
        "stages": {"study": "initial", "aggressive": "recourse", "fair": "recourse"},
        "observations": {"good": "good", "bad": "bad"},
    }
    return with_valuation(spec, "carve-out", [600, 600],
                          {"study": [-6, -6], "aggressive": [120, -60], "fair": [126, -54]},
                          {"aggressive": [30, 30], "fair": [6, 6]}, cap="1/10")


def global_risk_example():
    states = [("rare_bad", "1/100"), ("rare_good", "9/100"), ("common_bad", "9/100"), ("common_good", "81/100")]
    spec = primary(entity.with_entities(base.specification(
        {"study": ["-0.1"] * 4, "build": [10, 40, 10, 20]}, states=states,
        costs={"study": "0.1", "build": "1"}, budget="1.1", requires=[("build", "study")]), [("parent", 100, 20)]))
    spec["equity"]["initial_share_units"] = "1/1"
    entity.native(spec, "first", "parent", "study", ["-0.1"] * 4)
    entity.native(spec, "second", "parent", "build", [-1] * 4)
    spec["recourse"] = {
        "information_candidate": "study", "observation_date": "2030-07-01",
        "information_basis": "Invented rare/common observation does not reveal the terminal result",
        "stages": {"study": "initial", "build": "recourse"},
        "observations": dict(zip((state for state, _ in states), ["rare", "rare", "common", "common"], strict=True)),
    }
    return with_valuation(spec, "carve-out", [100] * 4,
                          {"study": ["-0.1"] * 4, "build": [-100, 40, -100, 20]}, {}, initial="1", cap="10")


def valuation_blocks(spec):
    rows = []
    for value in spec["entity_valuations"]:
        header = {key: value[key] for key in ("entity", "share_class", "valuation_basis", "shares_per_unit", "initial_share_units")}
        if value["max_tail_loss_per_share"] is not None:
            header["max_tail_loss_per_share"] = value["max_tail_loss_per_share"]
        rows.append(base.block("entity_valuation", header))
        rows += [base.block("entity_baseline_nav", {"entity": value["entity"], "state": state, "value": nav})
                 for state, nav in value["baseline_nav"].items()]
        for table, field in (("entity_candidate_payoff", "nav_changes"), ("entity_candidate_share_change", "share_changes")):
            rows += [base.block(table, {"entity": value["entity"], "candidate": candidate, "state": state, "value": amount})
                     for candidate, states in value[field].items() for state, amount in states.items()]
    return rows


def model(spec, *, reverse=False, include_uu=True):
    prefix = debt.model if "debt" in spec["entity_funding"] else entity.model
    rows = valuation_blocks(spec)
    return prefix(spec, reverse=reverse, include_uu=include_uu) + "".join(reversed(rows) if reverse else rows)


def fixture(text=None):
    root = Path(tempfile.mkdtemp(prefix="fmine-capital-valuation-"))
    (root / FILE).write_text(model(recourse_example()) if text is None else text, encoding="utf-8")
    return root


def realized(spec, selected, state):
    cash_reference = debt.realized if "debt" in spec["entity_funding"] else entity.realized
    row = cash_reference(spec, selected, state)
    row["entity_valuations"] = {}
    for value in spec["entity_valuations"]:
        outcome = base.equity_states({"equity": value, "payoffs": value["nav_changes"]}, selected, [state])[0]
        row["entity_valuations"][value["entity"]] = outcome
        if outcome["ending_shares"] <= 0:
            row["reasons"].append({"kind": "entity_nonpositive_share_count", "entity": value["entity"],
                                   "share_class": value["share_class"], "state_id": state, "shares": outcome["ending_shares"]})
    return row


def finish(spec, rows):
    result = entity.finish(spec, rows)
    weights = result["weights"]
    admissible = {key: row for key, row in rows.items() if not row["rejections"]}
    for row in rows.values():
        row["secondary_statistics"] = {}
    for value in spec["entity_valuations"]:
        name = value["entity"]
        cap = None if value["max_tail_loss_per_share"] is None else F(value["max_tail_loss_per_share"])
        for row in admissible.values():
            values = {state: path["entity_valuations"][name]["delta_nav_per_share"] for state, path in row["states"].items()}
            statistics = base.payoff_statistics(values, weights, F(spec["tail"]))
            statistics["risk_rejected"] = cap is not None and statistics["tail_loss"] > cap
            row["secondary_statistics"][name] = statistics
            row["risk_rejected"] |= statistics["risk_rejected"]
        population = [row["secondary_statistics"][name] for row in admissible.values()]
        hindsight = {state: max(row["values"][state] for row in population) for state in weights} if population else {}
        for row in population:
            regrets = {state: hindsight[state] - payoff for state, payoff in row["values"].items()}
            row.update(hindsight=hindsight, regrets=regrets,
                       mean_regret=sum(weights[state] * regret for state, regret in regrets.items()), max_regret=max(regrets.values()))
    feasible = {key: row for key, row in admissible.items() if not row["risk_rejected"]}
    best = max((row["per_share"]["mean"] for row in feasible.values()), default=None)
    result["after"] = {key for key, row in feasible.items() if row["per_share"]["mean"] == best}
    result["after_value"] = best
    result["risk_cost"] = None if best is None else result["before_value"] - best
    return result


def expected_static(spec):
    rows = {}
    for selected in entity.subsets([row["id"] for row in spec["candidates"]]):
        states = {state: realized(spec, selected, state) for state, _ in sorted(spec["states"])}
        rows[selected] = {"states": states, "rejections": entity.structural(spec, selected)
                          + [reason for row in states.values() for reason in row["reasons"]]}
    return finish(spec, rows)


def expected_policies(spec):
    information = spec["recourse"]
    initial = [name for name, stage in information["stages"].items() if stage == "initial"]
    later = [name for name, stage in information["stages"].items() if stage == "recourse"]
    costs = {row["id"]: F(row["cost"]) for row in spec["candidates"]}
    weights = {state: F(weight) for state, weight in spec["states"]}
    rows = {}
    for committed in entity.subsets(initial):
        acquired = information["information_candidate"] in committed
        observations = sorted(set(information["observations"].values())) if acquired else [None]
        for actions in itertools.product(entity.subsets(later), repeat=len(observations)):
            continuations = dict(zip(observations, actions, strict=True))
            branches = [{"observation_id": observation, "selected": selected,
                         "probability": sum((weight for state, weight in weights.items()
                                             if not acquired or information["observations"][state] == observation), F(0)),
                         "spend": sum((costs[name] for name in selected), F(0))}
                        for observation, selected in continuations.items()]
            states, reasons = {}, []
            for state in sorted(weights):
                observation = information["observations"][state] if acquired else None
                selected = tuple(sorted((*committed, *continuations[observation])))
                row = realized(spec, selected, state)
                row["observation_id"] = observation
                states[state] = row
                reasons += [{"state_id": state, "reason": reason} for reason in entity.structural(spec, selected) + row["reasons"]]
            rows[(committed, tuple(continuations.items()))] = {
                "states": states, "rejections": reasons, "branches": branches, "information_acquired": acquired,
                "initial_spend": sum((costs[name] for name in committed), F(0)),
            }
    return finish(spec, rows)


class CapitalValuationScenarios(unittest.TestCase):
    envelope = base.CapitalProgramScenarios.envelope
    refused = base.CapitalProgramScenarios.refused
    check_statistics = base.CapitalProgramScenarios.check_statistics
    check_claims = entity.CapitalEntityScenarios.check_claims
    row = staticmethod(entity.CapitalEntityScenarios.row)

    def program(self, spec, root=None):
        root = fixture(model(spec)) if root is None else root
        raw = (root / FILE).read_bytes()
        envelope = self.envelope(root)
        self.assertEqual(envelope["exit_code"], 0, envelope)
        self.assertIsNone(envelope["refusal"])
        source = {"path": str(root / FILE), "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}
        self.assertEqual(envelope["payload"]["source"], source)
        self.assertEqual((root / FILE).read_bytes(), raw)
        report = envelope["payload"]["program"]
        adaptive = "recourse" in spec
        self.assertEqual(envelope["payload"]["mode"], "declared_two_stage_capital_comparison" if adaptive else "declared_static_capital_comparison")
        self.assertEqual(report["schema"], "fmine.capital-recourse.v1" if adaptive else "fmine.capital-program.v1")
        static = report["static_comparison"] if adaptive else report
        fixed = expected_static(spec)
        self.check_rows(static, spec, fixed, False)
        self.check_evidence(static, source, spec)
        if adaptive:
            policies = expected_policies(spec)
            self.check_rows(report, spec, policies, True)
            for field, policy_value, static_value in (("net_adaptive_value", policies["after_value"], fixed["after_value"]),
                                                       ("unconstrained_net_adaptive_value", policies["before_value"], fixed["before_value"])):
                expected = None if policy_value is None or static_value is None else policy_value - static_value
                actual = report["summary"][field + "_per_share"]
                self.assertEqual(None if actual is None else F(actual), expected)
                self.assertIsNone(report["summary"][field])
            information = report["inputs"]["information"]
            self.assertEqual(set(report["claim"]["parents"]), set(static["claim"]["parents"]) | {information["id"]})
            self.check_claims([information, report["claim"]], source)
        for flag in ("reserve_disclosure_claim", "execution_authorized", "geotechnical_signoff", "regulatory_compliance_claim"):
            self.assertFalse(report[flag])
        return report

    def check_rows(self, report, spec, expected, adaptive):
        entity.CapitalEntityScenarios.check_rows(self, report, spec, expected, adaptive)
        declared = {row["entity"]: row for row in spec["entity_valuations"]}
        for row in report["summary"]["policies" if adaptive else "choices"]:
            key = entity.key_of(row) if adaptive else tuple(row["selected"])
            known = expected["rows"][key]
            self.assertEqual([value["entity"] for value in row["entity_valuations"]], sorted(declared))
            for value in row["entity_valuations"]:
                name, inputs = value["entity"], declared[value["entity"]]
                self.assertEqual(value["share_class"], inputs["share_class"])
                self.assertEqual(value["max_tail_loss_per_share"], inputs["max_tail_loss_per_share"])
                self.assertEqual(value["equity"], entity.serialized({"states": [state["entity_valuations"][name] for state in known["states"].values()]}))
                if known["rejections"]:
                    self.assertIsNone(value["per_share_statistics"])
                    self.assertIsNone(value["per_share_tail_loss"])
                    self.assertFalse(value["risk_rejected"])
                else:
                    statistics = known["secondary_statistics"][name]
                    self.check_statistics(value["per_share_statistics"], statistics, expected["weights"], (key, name))
                    self.assertEqual(F(value["per_share_tail_loss"]), statistics["tail_loss"])
                    self.assertEqual(value["risk_rejected"], statistics["risk_rejected"])
            if "debt" in spec["entity_funding"]:
                for state in row["entity_funding"]["states"]:
                    self.assertEqual(state["debt"], entity.serialized(known["states"][state["state_id"]]["ledger"]["debt"]))

    def check_evidence(self, report, source, spec):
        inputs = report["inputs"]
        claims = [inputs["policy"], inputs["weights"], *inputs["candidates"], inputs["equity"], inputs["entity_funding"], *inputs["entity_valuations"]]
        self.assertEqual(set(report["claim"]["parents"]), {claim["id"] for claim in claims})
        self.assertEqual(report["claim"]["value"], report["summary"])
        self.check_claims([*claims, report["claim"]], source)
        declared = {row["entity"]: row for row in spec["entity_valuations"]}
        self.assertEqual([claim["value"]["equity"]["entity"] for claim in inputs["entity_valuations"]], sorted(declared))
        self.assertEqual([claim["value"] for claim in inputs["entity_valuations"]], report["summary"]["entity_valuations"])
        for claim in inputs["entity_valuations"]:
            value = claim["value"]
            equity = value["equity"]
            known = declared[equity["entity"]]
            self.assertEqual(equity["model_key"], inputs["policy"]["value"]["model_key"])
            self.assertEqual(equity["currency"], "USD")
            for field in ("entity", "share_class", "valuation_basis", "shares_per_unit", "initial_share_units", "max_tail_loss_per_share"):
                self.assertEqual(equity[field], known[field], field)
            self.assertEqual({row["state_id"]: F(row["value"]) for row in equity["baseline_nav"]},
                             {state: F(value) for state, value in known["baseline_nav"].items()})
            self.assertEqual(len(equity["baseline_nav"]), len(spec["states"]))
            for actual, source_key, amount_key in ((value["nav_changes"], "nav_changes", "value"), (equity["share_changes"], "share_changes", "units")):
                self.assertEqual(len(actual), len(spec["states"]) * len(spec["candidates"]))
                self.assertEqual({(row["candidate_id"], row["state_id"]): F(row[amount_key]) for row in actual},
                                 {(candidate, state): F(amount) for candidate, states in known[source_key].items() for state, amount in states.items()})
            self.assertTrue(all(row["currency"] == "USD" for row in value["nav_changes"] + equity["baseline_nav"]))
            name = known["entity"]
            sections = [f"entity valuation {name} basis, shares and per-share risk limit", "model_key", "currency",
                        "payoff_convention", f"UU {base.UNKNOWN['id']}"]
            sections += [f"entity baseline NAV {name}/{state}" for state in known["baseline_nav"]]
            sections += [f"entity candidate payoff {name}/{candidate}/{state}"
                         for candidate, states in known["nav_changes"].items() for state in states]
            sections += [f"entity candidate share change {name}/{candidate}/{state}"
                         for candidate, states in known["share_changes"].items() for state in states]
            self.assertCountEqual([anchor["locator"].rsplit(": ", 1)[-1] for anchor in claim["anchors"]], sections)

    @staticmethod
    def policy(report, good=(), bad=()):
        return next(row for row in report["summary"]["policies"] if row["initial_selected"] == ["study"]
                    and {(branch["observation_id"], tuple(branch["selected"])) for branch in row["branches"]}
                    == {("good", tuple(good)), ("bad", tuple(bad))})

    def test_parent_gain_with_positive_child_nav_can_still_dilute_child_shareholders(self):
        report = self.program(static_example())
        harmful, fair = self.row(report, ("aggressive",)), self.row(report, ("fair",))
        self.assertEqual(report["summary"]["unconstrained_winners"], [harmful["bundle_id"]])
        self.assertEqual(report["summary"]["feasible_winners"], [fair["bundle_id"]])
        self.assertEqual(F(harmful["entity_valuations"][0]["equity"]["states"][0]["nav_change"]), 120)
        self.assertEqual(F(harmful["entity_valuations"][0]["per_share_statistics"]["mean"]), -2)
        self.assertEqual(F(fair["entity_valuations"][0]["per_share_statistics"]["mean"]), 1)
        self.assertEqual(F(report["summary"]["expected_per_share_cost_of_risk_limit"]), F(2, 5))

    def test_primary_and_multiple_secondary_caps_intersect_without_blending_objectives(self):
        spec = with_valuation(static_example(), "third-perimeter", [100, 100],
                              {"aggressive": [0, 0], "fair": [-20, -20]}, {}, initial="10", cap="0")
        report = self.program(spec)
        self.assertEqual(report["summary"]["feasible_winners"], [self.row(report, ())["bundle_id"]])
        self.assertEqual(F(report["summary"]["expected_per_share_cost_of_risk_limit"]), 1)
        spec = static_example()
        spec["equity"]["max_tail_loss_per_share"] = "-3/4"
        report = self.program(spec)
        self.assertFalse(report["summary"]["feasible_winners"])
        self.assertIsNone(report["summary"]["expected_per_share_cost_of_risk_limit"])
        fair = self.row(report, ("fair",))
        self.assertTrue(fair["risk_rejected"])
        self.assertFalse(fair["entity_valuations"][0]["risk_rejected"])

    def test_joint_state_ratios_fractional_tail_and_normalized_share_units(self):
        spec = with_valuation(primary(entity.with_entities(base.specification(
            {"issue": [50, 50]}, costs={"issue": "0"}, budget="0", tail="3/4"), [("parent", 100, 20)])),
            "carve-out", [600, 300], {"issue": [120, 120]}, {"issue": [30, 0]}, cap="2/3")
        report = self.program(spec)
        secondary = self.row(report, ("issue",))["entity_valuations"][0]
        self.assertEqual(F(secondary["per_share_statistics"]["mean"]), 0)
        self.assertEqual(F(secondary["per_share_tail_loss"]), F(2, 3))
        self.assertFalse(secondary["risk_rejected"])
        value = spec["entity_valuations"][0]
        value["shares_per_unit"], value["initial_share_units"] = "10/1", "6/1"
        value["share_changes"]["issue"] = {state: base.ratio(F(change) / 10) for state, change in value["share_changes"]["issue"].items()}
        rescaled = self.program(spec)
        self.assertEqual(report["summary"]["choices"], rescaled["summary"]["choices"])
        value["max_tail_loss_per_share"] = "19999/30000"
        self.assertTrue(self.row(self.program(spec), ("issue",))["risk_rejected"])

    def test_all_entity_regret_uses_the_joint_structural_population(self):
        spec = static_example()
        spec["equity"]["share_changes"]["aggressive"] = {state: "-100/1" for state, _ in spec["states"]}
        spec["entity_valuations"][0]["nav_changes"]["aggressive"] = {state: "1200" for state, _ in spec["states"]}
        report = self.program(spec)
        fair = self.row(report, ("fair",))
        self.assertEqual(F(fair["per_share_statistics"]["mean_regret"]), 0)
        self.assertEqual(F(fair["entity_valuations"][0]["per_share_statistics"]["mean_regret"]), 0)
        invalid = self.row(report, ("aggressive",))
        self.assertIsNone(invalid["entity_valuations"][0]["per_share_statistics"])
        spec = static_example()
        spec["entity_valuations"][0]["share_changes"]["aggressive"] = {state: "-60/1" for state, _ in spec["states"]}
        self.assertEqual(F(self.row(self.program(spec), ("fair",))["per_share_statistics"]["mean_regret"]), 0)

    def test_funded_recourse_selects_the_fair_deal_and_retains_all_twenty_policies(self):
        report = self.program(recourse_example())
        fair, harmful = self.policy(report, ("fair",)), self.policy(report, ("aggressive",))
        self.assertEqual(len(report["summary"]["policies"]), 20)
        self.assertEqual(len(report["static_comparison"]["summary"]["choices"]), 8)
        self.assertEqual(report["summary"]["unconstrained_winners"], [harmful["policy_id"]])
        self.assertEqual(report["summary"]["feasible_winners"], [fair["policy_id"]])
        self.assertEqual(F(fair["per_share_statistics"]["mean"]), F(2, 5))
        self.assertEqual(F(fair["entity_valuations"][0]["per_share_statistics"]["mean"]), F(89, 220))
        self.assertEqual(F(harmful["entity_valuations"][0]["per_share_tail_loss"]), F(31, 15))
        self.assertEqual(F(report["summary"]["net_adaptive_value_per_share"]), F(2, 5))

    def test_coarse_information_cannot_choose_distinct_terminal_entity_terms(self):
        spec = recourse_example()
        spec["recourse"]["observations"] = {state: "same" for state, _ in spec["states"]}
        report = self.program(spec)
        self.assertEqual(F(report["summary"]["net_adaptive_value_per_share"]), 0)
        winners = [row for row in report["summary"]["policies"] if row["policy_id"] in report["summary"]["feasible_winners"]]
        self.assertTrue(all(all(not state["selected"] for state in row["states"]) for row in winners))

    def test_secondary_nonpositive_shares_reject_only_the_realized_state(self):
        spec = recourse_example()
        spec["entity_funding"]["baseline_flows"]["third"]["parent"]["bad"] = "20"
        spec["entity_valuations"][0]["share_changes"]["fair"]["bad"] = "-60/1"
        report = self.program(spec)
        winner = self.policy(report, ("fair",))
        self.assertFalse(winner["structural_rejections"])
        self.assertEqual(report["summary"]["feasible_winners"], [winner["policy_id"]])
        invalid = self.policy(report, ("fair",), ("fair",))
        self.assertTrue(any(row["state_id"] == "bad" and row["reason"]["kind"] == "entity_nonpositive_share_count"
                            for row in invalid["structural_rejections"]))
        failed = next(row for row in invalid["entity_valuations"][0]["equity"]["states"] if row["state_id"] == "bad")
        self.assertIsNone(failed["delta_nav_per_share"])

    def test_initial_secondary_nav_and_share_commitments_remain_after_abandonment(self):
        spec = recourse_example()
        child = spec["entity_valuations"][0]
        child["share_changes"]["study"] = {state: "6/1" for state, _ in spec["states"]}
        child["max_tail_loss_per_share"] = "1/1"
        winner = self.policy(self.program(spec), ("fair",))
        child = winner["entity_valuations"][0]
        abandoned = next(row for row in child["equity"]["states"] if row["state_id"] == "bad")
        self.assertEqual(F(abandoned["share_change"]), 6)
        self.assertEqual(F(abandoned["nav_change"]), -6)
        self.assertEqual(F(abandoned["delta_nav_per_share"]), -1)
        self.assertEqual(F(child["per_share_statistics"]["mean"]), F(-1, 2))
        self.assertEqual(F(child["per_share_tail_loss"]), 1)
        self.assertFalse(winner["risk_rejected"])

    def test_secondary_tail_is_unconditional_not_an_observation_node_limit(self):
        report = self.program(global_risk_example())
        winner = next(row for row in report["summary"]["policies"] if row["policy_id"] in report["summary"]["feasible_winners"])
        self.assertEqual({branch["observation_id"]: branch["selected"] for branch in winner["branches"]}, {"rare": ["build"], "common": []})
        self.assertEqual(F(winner["per_share_statistics"]["mean"]), F(18, 5))
        self.assertEqual(F(winner["entity_valuations"][0]["per_share_tail_loss"]), F(51, 10))
        self.assertEqual(F(winner["entity_valuations"][0]["per_share_statistics"]["cvar95_loss"]), F(201, 10))

    def test_debt_and_secondary_valuations_use_the_same_realized_paths(self):
        spec = with_valuation(debt.recourse_example(), "jv", [600, 600],
                              {"study": [-6, -6], "project": [126, -54]}, {"project": [6, 6]}, cap="1/10")
        report = self.program(spec)
        winner = self.policy(report, ("project",))
        self.assertEqual(F(winner["per_share_statistics"]["mean"]), F(9, 20))
        self.assertEqual(F(winner["entity_valuations"][0]["per_share_statistics"]["mean"]), F(89, 220))
        for state in winner["entity_funding"]["states"]:
            final = next(row for row in state["debt"] if row["period_id"] == "third")
            self.assertEqual(F(final["closing_principal"]), 29 if state["state_id"] == "good" else 79)

    def test_full_sources_losing_values_and_input_order_bind_without_changing_ranking(self):
        spec = static_example()
        next(row for row in spec["candidates"] if row["id"] == "aggressive")["cost"] = "999"
        root = fixture(model(spec))
        original = self.program(spec, root)
        self.assertEqual(original, self.program(spec, root))
        (root / FILE).write_text(model(spec, reverse=True), encoding="utf-8")
        reordered = self.program(spec, root)
        self.assertEqual(original["summary"], reordered["summary"])
        self.assertNotEqual(original["digest"], reordered["digest"])
        spec["entity_valuations"][0]["nav_changes"]["aggressive"]["good"] = "121"
        (root / FILE).write_text(model(spec), encoding="utf-8")
        changed = self.program(spec, root)
        self.assertEqual(original["summary"]["feasible_winners"], changed["summary"]["feasible_winners"])
        self.assertNotEqual(original["digest"], changed["digest"])
        self.assertNotEqual(original["inputs"]["entity_valuations"][0]["value_tag"], changed["inputs"]["entity_valuations"][0]["value_tag"])

    def test_unknown_duplicate_orphan_partial_and_nonexplicit_secondary_rows_refuse(self):
        spec = static_example()
        text = model(spec)
        first = valuation_blocks(spec)[0]
        self.refused(text + first)
        self.refused(text.replace(first, ""))
        self.refused(text.replace('[[entity_valuation]]', '[[entity_valuation]]\nunknown = "1"', 1))
        self.refused(text.replace('shares_per_unit = "1/1"', 'shares_per_unit = "1"'))
        for field in ("baseline_nav", "nav_changes", "share_changes"):
            partial = copy.deepcopy(spec)
            values = partial["entity_valuations"][0][field]
            if field == "baseline_nav":
                values.pop("good")
            else:
                values["aggressive"].pop("good")
            self.refused(model(partial))
        duplicate = copy.deepcopy(spec)
        duplicate["entity_valuations"][0]["entity"] = "parent"
        self.refused(model(duplicate))
        orphan = text + base.block("entity_candidate_payoff", {"entity": "foreign", "candidate": "fair", "state": "good", "value": "1"})
        self.refused(orphan)
        self.refused(model(spec, include_uu=False))


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
