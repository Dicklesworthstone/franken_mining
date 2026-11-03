"""Independent exact capital-call ownership, cash and policy known answers.

Subscription shares follow the declared pre-money equation, using only supplied
inputs. Each period then reuses the independent cash/debt reference with those
settled contributions and ownership rights. Its temporary cash decomposition is
translated back to native flows and explicit call components before comparison.
No Rust intermediate, selected bundle or policy is used to calculate an answer.
All examples are invented and make no claim about any actual JV agreement.
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
entity, base = debt.entity, debt.base
FILE = base.FILE
METHOD = "pre_money_subscription"
CONVENTION = "native_excludes_modeled_capital_calls"
CALL_BASIS = 'Invented equal-price subscription, explicit pre-money valuation; no actual JV terms \\ "Ω"'
MONEY_FIELDS = ("pre_money_value", "required_funding", "parent_contribution", "external_contribution")


def with_calls(spec):
    funding = spec["entity_funding"]
    funding["cash_basis"] = "Invented native USD cash excluding modeled capital calls and any separately modeled debt"
    funding["capital_calls"] = {"method": METHOD, "cash_flow_convention": CONVENTION, "calls": []}
    for edge in funding["distributions"]:
        edge["basis"] = "Invented initial ownership; subsequent declared subscriptions change dividend rights"
    return spec


def call(spec, name, rows, *, period="first", sequence=1, candidate="build", recipient="jv"):
    spec["entity_funding"]["capital_calls"]["calls"].append({
        "id": name, "entity_id": recipient, "period_id": period, "sequence": sequence,
        "candidate_id": candidate, "basis": CALL_BASIS,
        "states": {state: dict(zip(MONEY_FIELDS, map(str, values), strict=True))
                   for (state, _), values in zip(spec["states"], rows, strict=True)},
    })
    return spec


def simple_example(states=None, periods=entity.TWO_PERIODS):
    states = [("base", "1/1")] if states is None else states
    spec = entity.with_entities(base.specification({"build": [50] * len(states)}, states=states,
                                costs={"build": "100"}, budget="100"),
                                [("parent", 200, 20), ("jv", 20, 20)], periods)
    entity.distribution(spec, "jv", "parent", "3/5")
    return with_calls(spec)


def recourse_example():
    spec = entity.with_entities(base.specification(
        {"study": [-5, -5], "build": [100, -20]}, states=[("good", "1/2"), ("bad", "1/2")],
        costs={"study": "5", "build": "100"}, budget="105", requires=[("build", "study")]),
        [("parent", 125, 20), ("jv", 20, 20)], entity.THREE_PERIODS)
    entity.distribution(spec, "jv", "parent", "3/5")
    with_calls(spec)
    entity.native(spec, "first", "parent", "study", [-5, -5])
    entity.native(spec, "second", "jv", "study", [-10, -10])
    entity.native(spec, "second", "jv", "build", [-100, -100])
    entity.native(spec, "third", "jv", "build", [210, 0])
    call(spec, "study_obligation", [(100, 10, 10, 0)] * 2, period="second", candidate="study")
    call(spec, "build_subscription", [(110, 100, 60, 40), (110, 100, 100, 0)], period="second", sequence=2)
    spec["recourse"] = {
        "information_candidate": "study", "observation_date": "2030-05-01",
        "information_basis": "Invented paid observation reveals partner funding before the build commitment",
        "stages": {"study": "initial", "build": "recourse"}, "observations": {"good": "good", "bad": "bad"},
    }
    base.with_equity(spec, [1000, 1000], {}, cap="1/20")
    spec["equity"]["entity"] = "parent"
    return spec


def call_blocks(spec):
    funding = spec["entity_funding"]["capital_calls"]
    rows = [base.block("entity_capital_calls", {key: funding[key] for key in ("method", "cash_flow_convention")})]
    for item in funding["calls"]:
        declaration = {"id": item["id"], "entity": item["entity_id"], "period": item["period_id"],
                       "sequence": str(item["sequence"]), "basis": item["basis"]}
        if item["candidate_id"] is not None:
            declaration["candidate"] = item["candidate_id"]
        rows.append(base.block("capital_call", declaration))
        rows += [base.block("capital_call_state", {"call": item["id"], "state": state, **values})
                 for state, values in item["states"].items()]
    return rows


def model(spec, *, reverse=False, include_uu=True):
    prefix = debt.model if "debt" in spec["entity_funding"] else entity.model
    rows = call_blocks(spec)
    return prefix(spec, reverse=reverse, include_uu=include_uu) + "".join(reversed(rows) if reverse else rows)


def fixture(text=None):
    root = Path(tempfile.mkdtemp(prefix="fmine-capital-call-"))
    (root / FILE).write_text(model(recourse_example()) if text is None else text, encoding="utf-8")
    return root


def ledger(funding, selected, state):
    """Settle subscriptions, then solve the independently supplied cash/debt equations."""
    periods = sorted(funding["periods"], key=lambda row: (row["cash_period"], row["id"]))
    entities = sorted(funding["entities"])
    edges = {row["entity_id"]: row for row in funding["distributions"]}
    shares = {name: F(edge["parent_share"]) for name, edge in edges.items()}
    balances = {name: F(funding["opening_cash"][name][state]) for name in entities}
    principal = {name: F(states[state]) for name, states in funding.get("debt", {}).get("opening_principal", {}).items()}
    result = {"state_id": state, **{key: [] for key in ("checkpoints", "transfers", "distributions", "conservation", "capital_calls")}}
    if "debt" in funding:
        result["debt"] = []
    for index, period in enumerate(periods):
        pid, date = period["id"], period["cash_period"].split("/")[1]
        movements = {name: dict.fromkeys(("parent_in", "parent_out", "external_in"), F(0)) for name in entities}
        calls = sorted((item for item in funding["capital_calls"]["calls"] if item["period_id"] == pid),
                       key=lambda item: (item["entity_id"], item["sequence"], item["id"]))
        for item in calls:
            child = item["entity_id"]
            parent = edges[child]["parent_entity_id"]
            values = {key: F(value) for key, value in item["states"][state].items()}
            active = item["candidate_id"] is None or item["candidate_id"] in selected
            paid = values["parent_contribution"] if active else F(0)
            outside = values["external_contribution"] if active else F(0)
            before = shares[child]
            # New money buys the same equity value per ownership unit for both parties.
            parent_value = before * values["pre_money_value"] + paid
            total_value = values["pre_money_value"] + paid + outside
            shares[child] = parent_value / total_value
            movements[parent]["parent_out"] += paid
            movements[child]["parent_in"] += paid
            movements[child]["external_in"] += outside
            result["capital_calls"].append({
                "call_id": item["id"], "entity_id": child, "parent_entity_id": parent,
                "period_id": pid, "checkpoint_date": date, "sequence": item["sequence"],
                "candidate_id": item["candidate_id"], "basis": item["basis"], "active": active,
                "pre_money_value": values["pre_money_value"], "required_funding": values["required_funding"],
                "declared_parent_contribution": values["parent_contribution"],
                "declared_external_contribution": values["external_contribution"],
                "parent_contribution": paid, "external_contribution": outside,
                "funding_surplus": paid + outside - values["required_funding"] if active else F(0),
                "parent_share_before": before, "parent_share_after": shares[child],
                "external_share_after": 1 - shares[child], "ownership_residual": F(0),
            })
        settled = copy.deepcopy(funding)
        settled["periods"] = [period]
        settled["opening_date"] = period["cash_period"].split("/")[0]
        for name in entities:
            settled["opening_cash"][name][state] = str(balances[name])
            flow = movements[name]
            net = flow["parent_in"] + flow["external_in"] - flow["parent_out"]
            settled["baseline_flows"][pid][name][state] = str(F(settled["baseline_flows"][pid][name][state]) + net)
        for edge in settled["distributions"]:
            edge["parent_share"] = str(shares[edge["entity_id"]])
        for name, value in principal.items():
            settled["debt"]["opening_principal"][name][state] = str(value)
        reference = debt.ledger if "debt" in funding else entity.entity_ledger
        part = reference(settled, selected, state)
        for row in part["checkpoints"]:
            if row["phase"] == "opening":
                if index == 0:
                    result["checkpoints"].append(row)
                continue
            flow = movements[row["entity_id"]]
            row["baseline_cash_flow"] -= flow["parent_in"] + flow["external_in"] - flow["parent_out"]
            row["capital_call"] = flow.copy()
            result["checkpoints"].append(row)
            if row["phase"] == "closing":
                balances[row["entity_id"]] = row["closing_cash"]
        for row in part["conservation"]:
            totals = {key: sum((flow[key] for flow in movements.values()), F(0)) for key in ("parent_in", "parent_out", "external_in")}
            row["baseline_cash_flow"] -= totals["external_in"]
            row["capital_call"] = totals
        for key in ("transfers", "distributions", "conservation"):
            result[key].extend(part[key])
        if "debt" in funding:
            result["debt"].extend(part["debt"])
            principal.update({row["facility_id"]: row["closing_principal"] for row in part["debt"]})
    result["minimum_slack"] = min(row["slack"] for row in result["checkpoints"])
    return result


def realized(spec, selected, state):
    cash = ledger(spec["entity_funding"], selected, state)
    spend = sum((F(row["cost"]) for row in spec["candidates"] if row["id"] in selected), F(0))
    reasons = [{"kind": "entity_cash_shortfall", "entity_id": row["entity_id"], "state_id": state,
                "period_id": row["period_id"], "checkpoint_date": row["checkpoint_date"], "phase": row["phase"], "shortfall": -row["slack"]}
               for row in cash["checkpoints"] if row["slack"] < 0]
    reasons += [{"kind": "entity_capital_call_shortfall", "entity_id": row["entity_id"], "call_id": row["call_id"],
                 "state_id": state, "period_id": row["period_id"], "checkpoint_date": row["checkpoint_date"],
                 "shortfall": -row["funding_surplus"]} for row in cash["capital_calls"] if row["funding_surplus"] < 0]
    equity = base.equity_states(spec, selected, [state])[0] if "equity" in spec else None
    if equity and equity["ending_shares"] <= 0:
        reasons.append({"kind": "nonpositive_share_count", "state_id": state, "shares": equity["ending_shares"]})
    return {"selected": selected, "spend": spend, "budget_slack": F(spec["budget"]) - spend,
            "net_nav": sum((F(spec["payoffs"][name][state]) for name in selected), F(0)),
            "ledger": cash, "reasons": reasons, "equity": equity}


def expected(spec, adaptive):
    rows = {}
    if not adaptive:
        for selected in entity.subsets([row["id"] for row in spec["candidates"]]):
            states = {state: realized(spec, selected, state) for state, _ in sorted(spec["states"])}
            rows[selected] = {"states": states, "rejections": entity.structural(spec, selected)
                              + [reason for row in states.values() for reason in row["reasons"]]}
        return entity.finish(spec, rows)
    information = spec["recourse"]
    initial = [name for name, stage in information["stages"].items() if stage == "initial"]
    later = [name for name, stage in information["stages"].items() if stage == "recourse"]
    costs = {row["id"]: F(row["cost"]) for row in spec["candidates"]}
    weights = {state: F(weight) for state, weight in spec["states"]}
    for committed in entity.subsets(initial):
        acquired = information["information_candidate"] in committed
        observations = sorted(set(information["observations"].values())) if acquired else [None]
        for actions in itertools.product(entity.subsets(later), repeat=len(observations)):
            continuation = dict(zip(observations, actions, strict=True))
            branches = [{"observation_id": observation, "selected": selected,
                         "probability": sum((weight for state, weight in weights.items()
                                             if not acquired or information["observations"][state] == observation), F(0)),
                         "spend": sum((costs[name] for name in selected), F(0))} for observation, selected in continuation.items()]
            states, reasons = {}, []
            for state in sorted(weights):
                observation = information["observations"][state] if acquired else None
                selected = tuple(sorted((*committed, *continuation[observation])))
                row = realized(spec, selected, state)
                row["observation_id"] = observation
                states[state] = row
                reasons += [{"state_id": state, "reason": reason} for reason in entity.structural(spec, selected) + row["reasons"]]
            rows[(committed, tuple(continuation.items()))] = {
                "states": states, "rejections": reasons, "branches": branches, "information_acquired": acquired,
                "initial_spend": sum((costs[name] for name in committed), F(0)),
            }
    return entity.finish(spec, rows)


class CapitalCallScenarios(unittest.TestCase):
    envelope = base.CapitalProgramScenarios.envelope
    refused = base.CapitalProgramScenarios.refused
    check_statistics = base.CapitalProgramScenarios.check_statistics
    check_claims = entity.CapitalEntityScenarios.check_claims
    row = staticmethod(entity.CapitalEntityScenarios.row)
    state = staticmethod(debt.CapitalDebtScenarios.state)

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
        fixed = expected(spec, False)
        static = report["static_comparison"] if adaptive else report
        self.check_rows(static, spec, fixed, False)
        self.check_evidence(static, source, spec)
        if adaptive:
            policies = expected(spec, True)
            self.check_rows(report, spec, policies, True)
            suffix = "_per_share" if "equity" in spec else ""
            for field, policy_value, static_value in (("net_adaptive_value", policies["after_value"], fixed["after_value"]),
                                                       ("unconstrained_net_adaptive_value", policies["before_value"], fixed["before_value"])):
                value = None if policy_value is None or static_value is None else policy_value - static_value
                actual = report["summary"][field + suffix]
                self.assertEqual(None if actual is None else F(actual), value)
                self.assertIsNone(report["summary"][field if suffix else field + "_per_share"])
            information = report["inputs"]["information"]
            self.assertEqual(set(report["claim"]["parents"]), set(static["claim"]["parents"]) | {information["id"]})
            self.check_claims([information, report["claim"]], source)
        for flag in ("reserve_disclosure_claim", "execution_authorized", "geotechnical_signoff", "regulatory_compliance_claim"):
            self.assertFalse(report[flag])
        return report

    def check_rows(self, report, spec, known, adaptive):
        entity.CapitalEntityScenarios.check_rows(self, report, spec, known, adaptive)
        for row in report["summary"]["policies" if adaptive else "choices"]:
            key = entity.key_of(row) if adaptive else tuple(row["selected"])
            for state in row["entity_funding"]["states"]:
                reference = known["rows"][key]["states"][state["state_id"]]["ledger"]
                self.assertEqual(state["capital_calls"], entity.serialized(reference["capital_calls"]), (key, state["state_id"]))
                if "debt" in reference:
                    self.assertEqual(state["debt"], entity.serialized(reference["debt"]))
                for call_row in state["capital_calls"]:
                    self.assertEqual(F(call_row["parent_share_after"]) + F(call_row["external_share_after"]), 1)
                    self.assertEqual(F(call_row["ownership_residual"]), 0)
                for cash in state["conservation"]:
                    moves = cash.get("debt", dict.fromkeys(("draw", "interest", "scheduled_principal", "sweep"), "0"))
                    calls = cash["capital_call"]
                    self.assertEqual(F(calls["parent_in"]), F(calls["parent_out"]))
                    self.assertEqual(F(cash["closing_cash"]) + F(cash["external_distributions"]) + F(moves["interest"])
                                     + F(moves["scheduled_principal"]) + F(moves["sweep"]),
                                     F(cash["opening_cash"]) + F(cash["baseline_cash_flow"]) + F(cash["candidate_cash_flow"])
                                     + F(moves["draw"]) + F(calls["external_in"]))

    def check_evidence(self, report, source, spec):
        checker = debt.CapitalDebtScenarios.check_evidence if "debt" in spec["entity_funding"] else entity.CapitalEntityScenarios.check_evidence
        checker(self, report, source, spec)
        claim = report["inputs"]["entity_funding"]
        actual = claim["value"]["capital_calls"]
        declared = spec["entity_funding"]["capital_calls"]
        self.assertEqual(actual["method"], declared["method"])
        self.assertEqual(actual["cash_flow_convention"], declared["cash_flow_convention"])
        self.assertEqual(len(actual["calls"]), len(declared["calls"]))
        for row, known in zip(sorted(actual["calls"], key=lambda item: item["id"]), sorted(declared["calls"], key=lambda item: item["id"]), strict=True):
            for field in ("id", "entity_id", "period_id", "sequence", "candidate_id", "basis"):
                self.assertEqual(row[field], known[field])
            self.assertEqual({item["state_id"]: tuple(F(item[field]) for field in MONEY_FIELDS) for item in row["states"]},
                             {state: tuple(F(values[field]) for field in MONEY_FIELDS) for state, values in known["states"].items()})
            self.assertTrue(all(item[field + "_currency"] == "USD" for item in row["states"] for field in MONEY_FIELDS))
        suffixes = (["entity capital-call method and cash-flow convention"]
                    + [f"capital call {row['id']}" for row in declared["calls"]]
                    + [f"capital call state {row['id']}/{state}" for row in declared["calls"] for state in row["states"]])
        for suffix in suffixes:
            self.assertTrue(any(anchor["locator"].endswith(": " + suffix) for anchor in claim["anchors"]), suffix)

    @staticmethod
    def dividend(state, period):
        return next(row for row in state["distributions"] if row["period_id"] == period and row["entity_id"] == "jv")

    def test_pro_rata_and_both_sides_top_up_using_same_state_ownership(self):
        spec = simple_example([(name, "1/3") for name in ("pro_rata", "parent_topup", "partner_topup")])
        call(spec, "growth", [(100, 100, 60, 40), (100, 100, 100, 0), (100, 100, 0, 100)])
        entity.native(spec, "first", "jv", "build", [-100] * 3)
        entity.native(spec, "second", "jv", "build", [100] * 3)
        report = self.program(spec)
        built = self.row(report, ("build",))
        for name, share in (("pro_rata", F(3, 5)), ("parent_topup", F(4, 5)), ("partner_topup", F(3, 10))):
            state = self.state(built, name)
            self.assertEqual(F(state["capital_calls"][0]["parent_share_after"]), share)
            self.assertEqual(F(self.dividend(state, "second")["parent_receipt"]), share * 100)
        self.assertEqual(F(built["statistics"]["mean"]), 50)  # Declared NAV is not recharged for cash movements.

    def test_earlier_dividends_use_old_rights_and_inactive_calls_preserve_them(self):
        spec = simple_example(periods=entity.THREE_PERIODS)
        call(spec, "topup", [(100, 100, 100, 0)], period="second")
        spec["entity_funding"]["baseline_flows"]["first"]["jv"]["base"] = "100"
        entity.native(spec, "second", "jv", "build", [-100])
        entity.native(spec, "third", "jv", "build", [100])
        report = self.program(spec)
        state = self.state(self.row(report, ("build",)))
        self.assertEqual(F(self.dividend(state, "first")["parent_receipt"]), 60)
        self.assertEqual(F(self.dividend(state, "third")["parent_receipt"]), 80)
        self.assertEqual(F(next(row for row in state["checkpoints"] if row["entity_id"] == "parent" and row["period_id"] == "third" and row["phase"] == "closing")["closing_cash"]), 240)
        idle = self.state(self.row(report, ()))["capital_calls"][0]
        self.assertFalse(idle["active"])
        self.assertEqual(F(idle["declared_parent_contribution"]), 100)
        self.assertEqual(F(idle["parent_contribution"]), 0)
        self.assertEqual(F(idle["parent_share_after"]), F(3, 5))

    def test_declared_sequence_changes_serial_subscriptions_and_is_not_id_order(self):
        spec = simple_example()
        call(spec, "z_parent", [(100, 100, 100, 0)], sequence=1)
        call(spec, "a_partner", [(200, 100, 0, 100)], sequence=2)
        entity.native(spec, "first", "jv", "build", [-200])
        entity.native(spec, "second", "jv", "build", [150])
        report = self.program(spec)
        state = self.state(self.row(report, ("build",)))
        self.assertEqual([row["call_id"] for row in state["capital_calls"]], ["z_parent", "a_partner"])
        self.assertEqual(F(state["capital_calls"][-1]["parent_share_after"]), F(8, 15))
        self.assertEqual(F(self.dividend(state, "second")["parent_receipt"]), 80)
        for item in spec["entity_funding"]["capital_calls"]["calls"]:
            item["sequence"] = 3 - item["sequence"]
        reversed_report = self.program(spec)
        self.assertEqual(F(self.state(self.row(reversed_report, ("build",)))["capital_calls"][-1]["parent_share_after"]), F(7, 10))

    def test_underfunded_calls_reject_despite_cash_and_keep_partial_subscription(self):
        spec = simple_example()
        spec["entity_funding"]["opening_cash"]["jv"]["base"] = "100"
        call(spec, "short", [(100, 100, 60, 0)])
        entity.native(spec, "first", "jv", "build", [-100])
        report = self.program(spec)
        built = self.row(report, ("build",))
        self.assertEqual(len(built["structural_rejections"]), 1)
        self.assertEqual(built["structural_rejections"][0]["kind"], "entity_capital_call_shortfall")
        self.assertEqual(F(built["structural_rejections"][0]["shortfall"]), 40)
        self.assertEqual(F(self.state(built)["capital_calls"][0]["parent_share_after"]), F(3, 4))
        self.assertIsNone(built["statistics"])
        spec["entity_funding"]["capital_calls"]["calls"][0]["candidate_id"] = None
        all_rejected = self.program(spec)["summary"]
        self.assertEqual(all_rejected["feasible_winners"], [])
        self.assertEqual(all_rejected["unconstrained_winners"], [])

    def test_full_parent_debit_is_not_clipped_and_later_cash_cannot_erase_failure(self):
        spec = simple_example()
        spec["entity_funding"]["opening_cash"]["parent"]["base"] = "30"
        call(spec, "unaffordable", [(100, 100, 100, 0)])
        entity.native(spec, "first", "jv", "build", [-100])
        entity.native(spec, "second", "jv", "build", [200])
        built = self.row(self.program(spec), ("build",))
        state = self.state(built)
        self.assertEqual(F(state["capital_calls"][0]["parent_contribution"]), 100)
        parent = [row for row in state["checkpoints"] if row["entity_id"] == "parent"]
        self.assertEqual(F(parent[1]["closing_cash"]), -70)
        self.assertEqual(F(parent[-1]["closing_cash"]), 90)
        self.assertTrue(built["structural_rejections"])
        self.assertIsNone(built["statistics"])

    def test_subminor_shares_zero_and_oversubscribed_calls_are_exact(self):
        spec = simple_example()
        spec["entity_funding"]["distributions"][0]["parent_share"] = "123/200"
        call(spec, "tiny", [("0.01", "0.02", "0.01", "0.01")])
        entity.native(spec, "first", "jv", "build", ["-0.02"])
        entity.native(spec, "second", "jv", "build", ["0.01"])
        state = self.state(self.row(self.program(spec), ("build",)))
        self.assertEqual(F(state["capital_calls"][0]["parent_share_after"]), F(323, 600))
        self.assertEqual(F(self.dividend(state, "second")["parent_receipt"]), F(323, 60000))
        self.assertEqual(F(self.dividend(state, "second")["external_distribution"]), F(277, 60000))
        spec = simple_example()
        call(spec, "empty", [(100, 0, 0, 0)])
        call(spec, "oversubscribed", [(100, 100, 90, 30)], sequence=2)
        state = self.state(self.row(self.program(spec), ("build",)))
        self.assertEqual(F(state["capital_calls"][0]["parent_share_after"]), F(3, 5))
        self.assertEqual(F(state["capital_calls"][1]["parent_share_after"]), F(15, 22))
        self.assertEqual(F(state["capital_calls"][1]["funding_surplus"]), 20)

    def test_recourse_keeps_future_initial_calls_on_abandoned_paths(self):
        report = self.program(recourse_example())
        self.assertEqual(len(report["summary"]["policies"]), 6)
        self.assertEqual(len(report["static_comparison"]["summary"]["choices"]), 4)
        winner = next(row for row in report["summary"]["policies"] if row["policy_id"] in report["summary"]["feasible_winners"])
        self.assertEqual(F(winner["per_share_statistics"]["mean"]), F(9, 20))
        self.assertEqual(F(winner["per_share_tail_loss"]), F(1, 20))
        self.assertEqual(F(report["summary"]["net_adaptive_value_per_share"]), F(9, 20))
        for name, share, cash in (("good", F(13, 21), 180), ("bad", F(7, 11), 110)):
            state = self.state(winner, name)
            self.assertTrue(state["capital_calls"][0]["active"])
            self.assertEqual(state["capital_calls"][1]["active"], name == "good")
            self.assertEqual(F(state["capital_calls"][-1]["parent_share_after"]), share)
            self.assertEqual(F(next(row for row in state["checkpoints"] if row["entity_id"] == "parent" and row["period_id"] == "third" and row["phase"] == "closing")["closing_cash"]), cash)
        spec = recourse_example()
        spec["recourse"]["observations"] = {"good": "same", "bad": "same"}
        shared = self.program(spec)
        self.assertEqual(F(shared["summary"]["net_adaptive_value_per_share"]), 0)

    def test_calls_fund_service_but_do_not_invent_cfads_or_unlock_debt(self):
        spec = debt.with_debt(simple_example(), [("loan", "jv", 100)])
        call(spec, "capital", [(100, 20, 20, 0)])
        funding = spec["entity_funding"]
        funding["opening_cash"]["parent"]["base"] = "100"
        funding["opening_cash"]["jv"]["base"] = "30"
        funding["baseline_flows"]["second"]["jv"]["base"] = "30"
        for period, cfads in (("first", "0"), ("second", "20")):
            funding["debt"]["baseline_flows"][period]["loan"]["base"].update(interest_rate="1/10", cfads=cfads)
        state = self.state(self.row(self.program(spec), ("build",)))
        first, second = state["debt"]
        self.assertEqual(F(first["cash_before_service"]), 50)
        self.assertEqual(F(first["dscr"]), 0)
        self.assertEqual(F(self.dividend(state, "first")["gross_distribution"]), 0)
        self.assertEqual(F(second["dscr"]), 2)
        self.assertEqual(F(self.dividend(state, "second")["parent_receipt"]), F(80, 3))
        funding["debt"]["facilities"][0]["sweep_fraction"] = "1/2"
        swept = self.state(self.row(self.program(spec), ("build",)))
        self.assertEqual(F(swept["debt"][0]["sweep"]), 10)
        self.assertEqual(F(swept["debt"][1]["opening_principal"]), 90)
        self.assertEqual(F(swept["debt"][1]["interest"]), 9)

    def test_nested_calls_conserve_intermediate_funding_and_each_ownership_edge(self):
        spec = entity.with_entities(base.specification({"build": [50]}, states=[("base", "1/1")],
                                    costs={"build": "100"}, budget="100"),
                                    [("parent", 200, 20), ("holding", 20, 20), ("jv", 20, 20)])
        entity.distribution(spec, "holding", "parent", "1/2")
        entity.distribution(spec, "jv", "holding", "3/5")
        with_calls(spec)
        call(spec, "holding_raise", [(100, 100, 100, 0)], recipient="holding")
        call(spec, "jv_raise", [(100, 100, 60, 40)])
        entity.native(spec, "first", "jv", "build", [-100])
        entity.native(spec, "second", "jv", "build", [100])
        state = self.state(self.row(self.program(spec), ("build",)))
        first = next(row for row in state["checkpoints"] if row["entity_id"] == "holding" and row["period_id"] == "first" and row["phase"] == "pre_distribution")
        self.assertEqual(first["capital_call"], {"parent_in": "100/1", "parent_out": "60/1", "external_in": "0/1"})
        self.assertEqual(F(first["closing_cash"]), 60)
        last = next(row for row in state["distributions"] if row["entity_id"] == "holding" and row["period_id"] == "second")
        self.assertEqual(F(last["parent_share"]), F(3, 4))
        self.assertEqual(F(last["parent_receipt"]), 45)
        self.assertEqual(F(self.dividend(state, "second")["parent_receipt"]), 60)
        closing = next(row for row in state["checkpoints"] if row["entity_id"] == "parent" and row["period_id"] == "second" and row["phase"] == "closing")
        self.assertEqual(F(closing["closing_cash"]), 175)

    def test_full_sources_unused_calls_and_input_order_remain_bound(self):
        spec = simple_example()
        spec["candidates"][0]["cost"] = "999"
        call(spec, "unused", [(100, 100, 60, 40)])
        root = fixture(model(spec))
        original = self.program(spec, root)
        self.assertEqual(original, self.program(spec, root))
        (root / FILE).write_text(model(spec, reverse=True), encoding="utf-8")
        reordered = self.program(spec, root)
        self.assertEqual(original["summary"], reordered["summary"])
        self.assertNotEqual(original["digest"], reordered["digest"])
        self.assertEqual(original["inputs"]["entity_funding"]["value_tag"], reordered["inputs"]["entity_funding"]["value_tag"])
        spec["entity_funding"]["capital_calls"]["calls"][0]["states"]["base"]["pre_money_value"] = "101"
        (root / FILE).write_text(model(spec), encoding="utf-8")
        changed = self.program(spec, root)
        self.assertEqual(original["summary"]["feasible_winners"], changed["summary"]["feasible_winners"])
        self.assertNotEqual(original["inputs"]["entity_funding"]["value_tag"], changed["inputs"]["entity_funding"]["value_tag"])
        self.assertNotEqual(original["digest"], changed["digest"])

    def test_missing_duplicate_unknown_and_nonexplicit_call_declarations_refuse(self):
        spec = call(simple_example(), "growth", [(100, 100, 60, 40)])
        text, blocks = model(spec), call_blocks(spec)
        for row in blocks:
            with self.subTest(missing=row.splitlines()[1]):
                self.refused(text.replace(row, "", 1))
            with self.subTest(duplicate=row.splitlines()[1]):
                self.refused(text + row)
        for old, new in [('method = "pre_money_subscription"', 'method = "guessed_penalty"'),
                         (f'cash_flow_convention = "{CONVENTION}"', 'cash_flow_convention = "native_includes_calls"'),
                         ('sequence = "1"', 'sequence = "0"'), ('sequence = "1"', 'sequence = "65"'),
                         ('sequence = "1"', 'sequence = "1/1"'), ('sequence = "1"', 'sequence = 1'),
                         ('pre_money_value = "100"', 'pre_money_value = "0"'),
                         ('parent_contribution = "60"', 'parent_contribution = "-1"'),
                         ('external_contribution = "40"', 'external_contribution = "-1"'),
                         ('required_funding = "100"', 'required_funding = "-1"'),
                         ('call = "growth"', 'call = "unknown"')]:
            with self.subTest(new=new):
                self.refused(text.replace(old, new))
        self.refused(text.replace('[[capital_call]]', '[[capital_call]]\nunknown = "1"'))
        partial = call(simple_example([("a", "1/2"), ("b", "1/2")]), "growth", [(100, 100, 60, 40)] * 2)
        partial["entity_funding"]["capital_calls"]["calls"][0]["states"].pop("b")
        self.refused(model(partial))
        duplicate = copy.deepcopy(spec)
        call(duplicate, "other", [(100, 0, 0, 0)], sequence=1)
        self.refused(model(duplicate))
        for field, value in (("entity_id", "parent"), ("period_id", "unknown"), ("candidate_id", "unknown")):
            unknown = copy.deepcopy(spec)
            unknown["entity_funding"]["capital_calls"]["calls"][0][field] = value
            self.refused(model(unknown))
        self.refused(model(spec, include_uu=False), "204")

    def test_future_calls_cannot_change_pre_observation_ownership_or_requirements(self):
        for values in ((100, 0, 0, 0), (100, 10, 0, 0), (100, 10, 10, 0)):
            spec = recourse_example()
            item = spec["entity_funding"]["capital_calls"]["calls"][1]
            item["period_id"] = "first"
            item["states"] = {state: dict(zip(MONEY_FIELDS, map(str, values), strict=True)) for state, _ in spec["states"]}
            self.refused(model(spec), "105")


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
