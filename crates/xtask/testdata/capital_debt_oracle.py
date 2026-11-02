"""Independent Fraction debt, covenant and cash-sweep checks through the real CLI.

Each selected path is calculated directly from declared inputs. Principal follows
its exact repayment recurrence; entity payouts solve child-receipt equations
recursively, applying each debtor's sweep before its outgoing dividend. This does
not read Rust's indexing, traversal order, subset catalog or expected results.
Earlier independent scalar statistics and complete-population checks are reused.
All fixtures are invented assumptions, not issuer financing terms or evidence.
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

_module = importlib.util.spec_from_file_location("entity_reference", Path(__file__).with_name("capital_entity_oracle.py"))
entity = importlib.util.module_from_spec(_module)
_module.loader.exec_module(entity)
base = entity.base
FILE = base.FILE
CONVENTION = "native_excludes_modeled_debt"
TERMS = 'Invented senior external debt; full-period rate on opening plus draw \\ "Ω"'
CFADS_BASIS = "Invented operating covenant metric; excludes debt proceeds and internal funding"


def with_debt(spec, facilities):
    funding = spec["entity_funding"]
    states = [state for state, _ in spec["states"]]
    candidates = [row["id"] for row in spec["candidates"]]
    funding["debt"] = {
        "cash_flow_convention": CONVENTION,
        "facilities": [{"id": name, "entity_id": debtor, "basis": TERMS, "cfads_basis": CFADS_BASIS,
                        "minimum_dscr": "3/2", "sweep_fraction": "0/1", "allow_distributions_without_service": True}
                       for name, debtor, _ in facilities],
        "opening_principal": {name: {state: str(principal) for state in states} for name, _, principal in facilities},
        "baseline_flows": {period["id"]: {name: {state: {"interest_rate": "0/1", "draw": "0", "principal_payment_cap": "0", "cfads": "0"}
                                                        for state in states} for name, _, _ in facilities} for period in funding["periods"]},
        "candidate_flows": {period["id"]: {name: {candidate: {state: {"draw": "0", "principal_payment_cap": "0", "cfads": "0"}
                                                                      for state in states} for candidate in candidates}
                                              for name, _, _ in facilities} for period in funding["periods"]},
    }
    return spec


def roll_forward_example():
    spec = with_debt(entity.idle_example([("parent", 20, 20), ("jv", 20, 20)]), [("loan", "jv", 100)])
    entity.distribution(spec, "jv", "parent", "3/5")
    funding = spec["entity_funding"]
    debt = funding["debt"]
    debt["facilities"][0]["sweep_fraction"] = "1/2"
    for period in ("first", "second"):
        funding["baseline_flows"][period]["jv"]["base"] = "50"
        debt["baseline_flows"][period]["loan"]["base"].update(interest_rate="1/10", principal_payment_cap="20", cfads="50")
    return spec


def nested_example():
    spec = with_debt(entity.idle_example([("parent", 20, 20), ("holding", 20, 20), ("mine", 20, 20)]),
                     [("a_root", "parent", 20), ("z_intermediate", "holding", 50)])
    entity.distribution(spec, "holding", "parent", "3/4")
    entity.distribution(spec, "mine", "holding", "1/1")
    spec["entity_funding"]["baseline_flows"]["first"]["mine"]["base"] = "80"
    for facility in spec["entity_funding"]["debt"]["facilities"]:
        facility["sweep_fraction"] = "1/2"
    return spec


def financing_example():
    spec = with_debt(entity.with_entities(base.specification({"borrow": [0], "project": [100]}, states=[("base", "1/1")],
                     costs={"borrow": "0", "project": "50"}, budget="50"), [("parent", 20, 20)]), [("loan", "parent", 0)])
    entity.native(spec, "first", "parent", "project", [-50])
    funding = spec["entity_funding"]
    funding["baseline_flows"]["first"]["parent"]["base"] = "5"
    funding["baseline_flows"]["second"]["parent"]["base"] = "60"
    debt = funding["debt"]
    debt["baseline_flows"]["first"]["loan"]["base"]["interest_rate"] = "1/10"
    debt["candidate_flows"]["first"]["loan"]["borrow"]["base"]["draw"] = "50"
    debt["candidate_flows"]["second"]["loan"]["borrow"]["base"]["principal_payment_cap"] = "50"
    debt["baseline_flows"]["second"]["loan"]["base"]["cfads"] = "60"
    debt["candidate_flows"]["second"]["loan"]["project"]["base"]["cfads"] = "20"
    return spec


def recourse_example(equity=True):
    spec = with_debt(entity.recourse_example(equity), [("loan", "jv", 100)])
    next(row for row in spec["candidates"] if row["id"] == "project")["cost"] = "20"
    spec["budget"] = "25"
    entity.native(spec, "third", "parent", "project", [-20, -20])
    funding = spec["entity_funding"]
    debt = funding["debt"]
    debt["facilities"][0]["sweep_fraction"] = "1/2"
    for state, cfads, later in [("good", "130", 3), ("bad", "30", 8)]:
        funding["baseline_flows"]["second"]["jv"][state] = cfads
        funding["baseline_flows"]["third"]["jv"][state] = str(later + 1)
        debt["baseline_flows"]["second"]["loan"][state].update(interest_rate="1/10", principal_payment_cap="20", cfads=cfads)
        debt["baseline_flows"]["third"]["loan"][state].update(interest_rate="1/10", cfads=str(later))
        debt["candidate_flows"]["third"]["loan"]["study"][state].update(principal_payment_cap="1", cfads="1")
    return spec


def debt_blocks(spec):
    debt = spec["entity_funding"]["debt"]
    rows = [base.block("entity_debt", {"cash_flow_convention": debt["cash_flow_convention"]})]
    rows += [base.block("debt_facility", {"id": row["id"], "entity": row["entity_id"], "basis": row["basis"],
                                         "cfads_basis": row["cfads_basis"], "minimum_dscr": row["minimum_dscr"],
                                         "sweep_fraction": row["sweep_fraction"],
                                         "allow_distributions_without_service": str(row["allow_distributions_without_service"]).lower()})
             for row in debt["facilities"]]
    rows += [base.block("debt_opening_principal", {"facility": facility, "state": state, "value": value})
             for facility, states in debt["opening_principal"].items() for state, value in states.items()]
    rows += [base.block("debt_service", {"facility": facility, "period": period, "state": state, **values})
             for period, facilities in debt["baseline_flows"].items() for facility, states in facilities.items() for state, values in states.items()]
    rows += [base.block("candidate_debt_service", {"facility": facility, "period": period, "candidate": candidate, "state": state, **values})
             for period, facilities in debt["candidate_flows"].items() for facility, candidates in facilities.items()
             for candidate, states in candidates.items() for state, values in states.items()]
    return rows


def model(spec, *, reverse=False, include_uu=True):
    rows = debt_blocks(spec)
    return entity.model(spec, reverse=reverse, include_uu=include_uu) + "".join(reversed(rows) if reverse else rows)


def fixture(text=None):
    root = Path(tempfile.mkdtemp(prefix="fmine-capital-debt-"))
    (root / FILE).write_text(model(recourse_example()) if text is None else text, encoding="utf-8")
    return root


def ledger(funding, selected, state):
    """Solve debt recurrences and recursive child receipts using only declared inputs."""
    entities = sorted(funding["entities"])
    periods = sorted(funding["periods"], key=lambda row: (row["cash_period"], row["id"]))
    debt = funding["debt"]
    facilities = {row["entity_id"]: row for row in debt["facilities"]}
    principal = {row["id"]: F(debt["opening_principal"][row["id"]][state]) for row in debt["facilities"]}
    balances = {name: F(funding["opening_cash"][name][state]) for name in entities}
    edges = {row["entity_id"]: row for row in funding["distributions"]}
    children = {name: sorted(child for child, edge in edges.items() if edge["parent_entity_id"] == name) for name in entities}
    checkpoints, transfers, distributions, conservation, debts = [], [], [], [], []

    def checkpoint(name, period, phase, opening, native=F(0), project=F(0), incoming=F(0), outgoing=F(0),
                   dividends_in=F(0), dividends_out=F(0), movements=None):
        pid = period["id"] if period else None
        floor = F(funding["floors"][pid or periods[0]["id"]][name])
        closing = opening + native + project + incoming - outgoing + dividends_in - dividends_out
        if movements is not None:
            closing += movements["draw"] - movements["interest"] - movements["scheduled_principal"] - movements["sweep"]
        row = {"entity_id": name, "period_id": pid,
               "checkpoint_date": period["cash_period"].split("/")[1] if period else funding["opening_date"], "phase": phase,
               "opening_cash": opening, "baseline_cash_flow": native, "candidate_cash_flow": project,
               "transfer_in": incoming, "transfer_out": outgoing, "distribution_in": dividends_in, "distribution_out": dividends_out,
               "closing_cash": closing, "minimum_cash": floor, "slack": closing - floor}
        if movements is not None:
            row["debt"] = movements.copy()
        return row

    for name in entities:
        checkpoints.append(checkpoint(name, None, "opening", balances[name]))
    for period in periods:
        pid, date = period["id"], period["cash_period"].split("/")[1]
        opening = balances.copy()
        native = {name: F(funding["baseline_flows"][pid][name][state]) for name in entities}
        project = {name: sum((F(funding["candidate_flows"][pid][name][candidate][state]) for candidate in selected), F(0)) for name in entities}
        tin, tout = dict.fromkeys(entities, F(0)), dict.fromkeys(entities, F(0))
        for row in sorted((row for row in funding["transfers"] if row["period_id"] == pid), key=lambda row: row["id"]):
            active = row["candidate_id"] is None or row["candidate_id"] in selected
            declared = F(row["amounts"][state])
            actual = declared if active else F(0)
            tin[row["to_entity"]] += actual
            tout[row["from_entity"]] += actual
            transfers.append({"transfer_id": row["id"], "from_entity": row["from_entity"], "to_entity": row["to_entity"],
                              "period_id": pid, "checkpoint_date": date, "candidate_id": row["candidate_id"], "basis": row["basis"],
                              "active": active, "declared_amount": declared, "actual_amount": actual})
        pre = {name: opening[name] + native[name] + project[name] + tin[name] - tout[name] for name in entities}
        service, movements = {}, {}
        for name, facility in facilities.items():
            fid = facility["id"]
            baseline = debt["baseline_flows"][pid][fid][state]
            changes = [debt["candidate_flows"][pid][fid][candidate][state] for candidate in selected]
            extra = {field: sum((F(change[field]) for change in changes), F(0)) for field in ("draw", "principal_payment_cap", "cfads")}
            draw = F(baseline["draw"]) + extra["draw"]
            before = principal[fid] + draw
            interest = before * F(baseline["interest_rate"])
            cap = F(baseline["principal_payment_cap"]) + extra["principal_payment_cap"]
            scheduled = min(cap, before)
            due = interest + scheduled
            cfads = F(baseline["cfads"]) + extra["cfads"]
            dscr = cfads / due if due else None
            status = "no_service_due" if due == 0 else ("covered" if dscr >= F(facility["minimum_dscr"]) else "below_minimum")
            allowed = facility["allow_distributions_without_service"] if due == 0 else status == "covered"
            service[name] = {"facility_id": fid, "entity_id": name, "period_id": pid, "checkpoint_date": date,
                             "opening_principal": principal[fid], "baseline_draw": F(baseline["draw"]), "candidate_draw": extra["draw"],
                             "draw": draw, "principal_before_service": before, "interest_rate": F(baseline["interest_rate"]), "interest": interest,
                             "baseline_principal_payment_cap": F(baseline["principal_payment_cap"]),
                             "candidate_principal_payment_cap": extra["principal_payment_cap"], "principal_payment_cap": cap,
                             "scheduled_principal": scheduled, "scheduled_service": due, "baseline_cfads": F(baseline["cfads"]),
                             "candidate_cfads": extra["cfads"], "cfads": cfads, "dscr": dscr, "minimum_dscr": F(facility["minimum_dscr"]),
                             "coverage_status": status, "distribution_allowed": allowed,
                             "allow_distributions_without_service": facility["allow_distributions_without_service"],
                             "cash_before_service": pre[name], "cash_after_service": pre[name] + draw - due,
                             "minimum_cash": F(funding["floors"][pid][name]), "sweep_fraction": F(facility["sweep_fraction"])}
            movements[name] = {"draw": draw, "interest": interest, "scheduled_principal": scheduled, "sweep": F(0)}
            pre[name] += draw - due
        checkpoints += [checkpoint(name, period, "pre_distribution", opening[name], native[name], project[name], tin[name], tout[name],
                                   movements=movements.get(name)) for name in entities]
        gross, incoming, payout_rows = {}, {}, {}

        def payout(name):
            if name in gross:
                return gross[name]
            incoming[name] = sum((F(edges[child]["parent_share"]) * payout(child) for child in children[name]), F(0))
            cash = pre[name] + incoming[name]
            floor = F(funding["floors"][pid][name])
            if name in service:
                row = service[name]
                available = max(F(0), cash - floor)
                sweep = min(row["principal_before_service"] - row["scheduled_principal"], row["sweep_fraction"] * available)
                principal[row["facility_id"]] = row["principal_before_service"] - row["scheduled_principal"] - sweep
                row.update(cash_before_sweep=cash, available_sweep_cash=available, sweep=sweep, cash_after_sweep=cash - sweep,
                           closing_principal=principal[row["facility_id"]], principal_residual=F(0))
                assert row["closing_principal"] + row["scheduled_principal"] + sweep == row["opening_principal"] + row["draw"]
                movements[name]["sweep"] = sweep
                cash -= sweep
            if name not in edges:
                gross[name] = F(0)
                return F(0)
            edge, gate = edges[name], funding["gates"][pid][name][state]
            allowed = gate["allowed"] and (name not in service or service[name]["distribution_allowed"])
            surplus = max(F(0), cash - floor)
            gross[name] = min(F(gate["maximum_distribution"]), surplus) if allowed else F(0)
            share = F(edge["parent_share"])
            row = {"entity_id": name, "parent_entity_id": edge["parent_entity_id"], "period_id": pid, "checkpoint_date": date,
                   "basis": edge["basis"], "parent_share": share, "external_share": 1 - share, "allowed": gate["allowed"],
                   "maximum_distribution": F(gate["maximum_distribution"]), "cash_before_distribution": cash, "minimum_cash": floor,
                   "available_surplus": surplus, "gross_distribution": gross[name], "parent_receipt": gross[name] * share,
                   "external_distribution": gross[name] * (1 - share), "cash_after_distribution": cash - gross[name]}
            if name in service:
                row["debt_gate"] = {key: service[name][key] for key in ("facility_id", "dscr", "minimum_dscr", "coverage_status", "distribution_allowed")}
                row["debt_gate"]["effective_allowed"] = allowed
            payout_rows[name] = row
            return gross[name]

        for name in entities:
            payout(name)
        debts += sorted(service.values(), key=lambda row: row["facility_id"])
        distributions += list(payout_rows.values())
        for name in entities:
            row = checkpoint(name, period, "closing", opening[name], native[name], project[name], tin[name], tout[name],
                             incoming[name], gross[name], movements.get(name))
            checkpoints.append(row)
            balances[name] = row["closing_cash"]
        totals = {field: sum((row[field] for row in movements.values()), F(0)) for field in ("draw", "interest", "scheduled_principal", "sweep")}
        outside = sum((row["external_distribution"] for row in payout_rows.values()), F(0))
        residual = (sum(balances.values()) + outside + totals["interest"] + totals["scheduled_principal"] + totals["sweep"]
                    - sum(opening.values()) - sum(native.values()) - sum(project.values()) - totals["draw"])
        assert residual == 0
        conservation.append({"period_id": pid, "checkpoint_date": date, "opening_cash": sum(opening.values()),
                             "baseline_cash_flow": sum(native.values()), "candidate_cash_flow": sum(project.values()),
                             "internal_transfers": sum(tin.values()), "internal_distributions": sum(incoming.values()),
                             "external_distributions": outside, "closing_cash": sum(balances.values()), "residual": residual, "debt": totals})
    return {"state_id": state, "checkpoints": checkpoints, "transfers": transfers, "distributions": distributions,
            "conservation": conservation, "debt": debts, "minimum_slack": min(row["slack"] for row in checkpoints)}


def realized(spec, selected, state):
    cash = ledger(spec["entity_funding"], selected, state)
    costs = {row["id"]: F(row["cost"]) for row in spec["candidates"]}
    spend = sum((costs[name] for name in selected), F(0))
    reasons = [{"kind": "entity_cash_shortfall", "entity_id": row["entity_id"], "state_id": state,
                "period_id": row["period_id"], "checkpoint_date": row["checkpoint_date"], "phase": row["phase"], "shortfall": -row["slack"]}
               for row in cash["checkpoints"] if row["slack"] < 0]
    equity = base.equity_states(spec, selected, [state])[0] if "equity" in spec else None
    if equity and equity["ending_shares"] <= 0:
        reasons.append({"kind": "nonpositive_share_count", "state_id": state, "shares": equity["ending_shares"]})
    return {"selected": selected, "spend": spend, "budget_slack": F(spec["budget"]) - spend,
            "net_nav": sum((F(spec["payoffs"][name][state]) for name in selected), F(0)),
            "ledger": cash, "reasons": reasons, "equity": equity}


def expected_static(spec):
    rows = {}
    for selected in entity.subsets([row["id"] for row in spec["candidates"]]):
        states = {state: realized(spec, selected, state) for state, _ in sorted(spec["states"])}
        rows[selected] = {"states": states, "rejections": entity.structural(spec, selected)
                          + [reason for row in states.values() for reason in row["reasons"]]}
    return entity.finish(spec, rows)


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
    return entity.finish(spec, rows)


class CapitalDebtScenarios(unittest.TestCase):
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
            suffix = "_per_share" if "equity" in spec else ""
            for field, policy_value, static_value in (("net_adaptive_value", policies["after_value"], fixed["after_value"]),
                                                       ("unconstrained_net_adaptive_value", policies["before_value"], fixed["before_value"])):
                expected = None if policy_value is None or static_value is None else policy_value - static_value
                actual = report["summary"][field + suffix]
                self.assertEqual(None if actual is None else F(actual), expected)
                self.assertIsNone(report["summary"][field if suffix else field + "_per_share"])
            information = report["inputs"]["information"]
            self.assertEqual(set(report["claim"]["parents"]), set(static["claim"]["parents"]) | {information["id"]})
            self.check_claims([information, report["claim"]], source)
        for flag in ("reserve_disclosure_claim", "execution_authorized", "geotechnical_signoff", "regulatory_compliance_claim"):
            self.assertFalse(report[flag])
        return report

    def check_rows(self, report, spec, expected, adaptive):
        entity.CapitalEntityScenarios.check_rows(self, report, spec, expected, adaptive)
        for row in report["summary"]["policies" if adaptive else "choices"]:
            key = entity.key_of(row) if adaptive else tuple(row["selected"])
            known = expected["rows"][key]
            for state in row["entity_funding"]["states"]:
                expected_debt = known["states"][state["state_id"]]["ledger"]["debt"]
                self.assertEqual(state["debt"], entity.serialized(expected_debt), (key, state["state_id"], "complete debt ledger"))
                for item in state["debt"]:
                    self.assertEqual(F(item["closing_principal"]) + F(item["scheduled_principal"]) + F(item["sweep"]),
                                     F(item["opening_principal"]) + F(item["draw"]))
                    self.assertEqual(F(item["principal_residual"]), 0)
                for item in state["conservation"]:
                    movement = item["debt"]
                    self.assertEqual(F(item["closing_cash"]) + F(item["external_distributions"]) + F(movement["interest"])
                                     + F(movement["scheduled_principal"]) + F(movement["sweep"]),
                                     F(item["opening_cash"]) + F(item["baseline_cash_flow"]) + F(item["candidate_cash_flow"]) + F(movement["draw"]))

    def check_evidence(self, report, source, spec):
        entity.CapitalEntityScenarios.check_evidence(self, report, source, spec)
        claim = report["inputs"]["entity_funding"]
        actual = claim["value"]["debt"]
        known = spec["entity_funding"]["debt"]
        self.assertEqual(actual["cash_flow_convention"], known["cash_flow_convention"])
        self.assertEqual(actual["facilities"], sorted(known["facilities"], key=lambda row: row["id"]))
        self.assertEqual(len(actual["opening_principal"]), len(known["facilities"]) * len(spec["states"]))
        self.assertEqual({(row["facility_id"], row["state_id"]): F(row["value"]) for row in actual["opening_principal"]},
                         {(facility, state): F(amount) for facility, states in known["opening_principal"].items() for state, amount in states.items()})
        self.assertTrue(all(row["currency"] == "USD" for row in actual["opening_principal"]))
        periods, states, candidates = len(spec["entity_funding"]["periods"]), len(spec["states"]), len(spec["candidates"])
        self.assertEqual(len(actual["baseline_flows"]), len(known["facilities"]) * periods * states)
        self.assertEqual(len(actual["candidate_flows"]), len(known["facilities"]) * periods * candidates * states)
        for key, include_candidate in (("baseline_flows", False), ("candidate_flows", True)):
            fields = ("interest_rate", "draw", "principal_payment_cap", "cfads") if not include_candidate else ("draw", "principal_payment_cap", "cfads")
            supplied = {}
            for period, facilities in known[key].items():
                for facility, entries in facilities.items():
                    for candidate, state_rows in entries.items() if include_candidate else [(None, entries)]:
                        for state, values in state_rows.items():
                            supplied[(period, facility, candidate, state)] = tuple(F(values[field]) for field in fields)
            self.assertEqual({(row["period_id"], row["facility_id"], row.get("candidate_id"), row["state_id"]): tuple(F(row[field]) for field in fields)
                              for row in actual[key]}, supplied)
            self.assertTrue(all(row[field + "_currency"] == "USD" for row in actual[key] for field in ("draw", "principal_payment_cap", "cfads")))
        suffixes = (["entity debt cash-flow convention"]
                    + [f"debt facility {row['id']}" for row in actual["facilities"]]
                    + [f"debt opening principal {row['facility_id']}/{row['state_id']}" for row in actual["opening_principal"]]
                    + [f"debt service {row['facility_id']}/{row['period_id']}/{row['state_id']}" for row in actual["baseline_flows"]]
                    + [f"candidate debt service {row['facility_id']}/{row['period_id']}/{row['candidate_id']}/{row['state_id']}" for row in actual["candidate_flows"]])
        for suffix in suffixes:
            self.assertTrue(any(anchor["locator"].endswith(": " + suffix) for anchor in claim["anchors"]), suffix)

    @staticmethod
    def state(row, name="base"):
        return next(state for state in row["entity_funding"]["states"] if state["state_id"] == name)

    @staticmethod
    def service(state, facility="loan", period="first"):
        return next(row for row in state["debt"] if row["facility_id"] == facility and row["period_id"] == period)

    def test_exact_service_sweep_and_interest_carry(self):
        report = self.program(roll_forward_example())
        hold = self.row(report, ())
        self.assertEqual(F(hold["statistics"]["mean"]), 0)
        state = self.state(hold)
        first, second = state["debt"]
        self.assertEqual([F(first[key]) for key in ("interest", "scheduled_principal", "dscr", "sweep", "closing_principal")],
                         [10, 20, F(5, 3), 10, 70])
        self.assertEqual([F(second[key]) for key in ("interest", "scheduled_principal", "dscr", "sweep", "closing_principal")],
                         [7, 20, F(50, 27), F(23, 2), F(77, 2)])
        self.assertEqual([F(row["closing_cash"]) for row in state["conservation"]], [46, F(529, 10)])

    def test_lock_retains_cash_and_exact_coverage_boundary_unlocks(self):
        spec = roll_forward_example()
        funding = spec["entity_funding"]
        funding["debt"]["facilities"][0]["sweep_fraction"] = "0/1"
        funding["baseline_flows"]["first"]["jv"]["base"] = "36"
        funding["debt"]["baseline_flows"]["first"]["loan"]["base"]["cfads"] = "36"
        state = self.state(self.row(self.program(spec), ()))
        self.assertEqual(self.service(state)["coverage_status"], "below_minimum")
        self.assertEqual([F(row["gross_distribution"]) for row in state["distributions"]], [0, 28])
        self.assertEqual(F(self.service(state, period="second")["dscr"]), F(25, 14))
        funding["baseline_flows"]["first"]["jv"]["base"] = "45"
        funding["debt"]["baseline_flows"]["first"]["loan"]["base"]["cfads"] = "45"
        funding["gates"]["second"]["jv"]["base"]["allowed"] = False
        state = self.state(self.row(self.program(spec), ()))
        self.assertEqual(F(self.service(state)["dscr"]), F(3, 2))
        self.assertEqual([F(row["gross_distribution"]) for row in state["distributions"]], [15, 0])
        self.assertTrue(state["distributions"][1]["debt_gate"]["distribution_allowed"])
        self.assertFalse(state["distributions"][1]["debt_gate"]["effective_allowed"])

    def test_nested_receipts_feed_both_intermediate_and_root_sweeps(self):
        spec = nested_example()
        state = self.state(self.row(self.program(spec), ()))
        root = self.service(state, "a_root")
        middle = self.service(state, "z_intermediate")
        self.assertEqual([F(middle[key]) for key in ("cash_before_sweep", "sweep", "closing_principal")], [100, 40, 10])
        self.assertEqual([F(root[key]) for key in ("cash_before_sweep", "sweep", "closing_principal")], [50, 15, 5])
        self.assertEqual(F(state["conservation"][0]["closing_cash"]), 75)
        self.assertEqual(F(state["conservation"][0]["external_distributions"]), 10)
        self.assertEqual(F(state["conservation"][0]["debt"]["sweep"]), 55)
        spec["entity_funding"]["debt"]["baseline_flows"]["first"]["z_intermediate"]["base"].update(principal_payment_cap="5", cfads="10")
        failed = self.row(self.program(spec), ())
        self.assertIsNone(failed["statistics"])
        self.assertTrue(any(row["entity_id"] == "holding" and row["period_id"] == "first"
                            and row["phase"] == "pre_distribution" and F(row["shortfall"]) == 5 for row in failed["structural_rejections"]))
        state = self.state(failed)
        self.assertEqual(F(self.service(state, "z_intermediate")["cash_before_sweep"]), 95)
        self.assertEqual(F(self.service(state, "z_intermediate")["sweep"]), F(75, 2))

    def test_derived_lock_reverses_parent_selection_despite_abundant_jv_cash(self):
        spec = with_debt(entity.with_entities(base.specification({"big": [50], "small": [10]}, states=[("base", "1/1")],
                         costs={"big": "30", "small": "5"}, budget="30"), [("parent", 25, 20), ("jv", 100, 20)]), [("loan", "jv", 100)])
        entity.distribution(spec, "jv", "parent", "3/5")
        entity.native(spec, "second", "parent", "big", [-30])
        entity.native(spec, "second", "parent", "small", [-5])
        funding = spec["entity_funding"]
        funding["baseline_flows"]["first"]["jv"]["base"] = "40"
        funding["debt"]["facilities"][0]["allow_distributions_without_service"] = False
        funding["debt"]["baseline_flows"]["first"]["loan"]["base"].update(principal_payment_cap="40", cfads="40")
        report = self.program(spec)
        self.assertEqual(report["summary"]["feasible_winners"], [self.row(report, ("small",))["bundle_id"]])
        self.assertIsNone(self.row(report, ("big",))["statistics"])
        funding["debt"]["facilities"][0]["minimum_dscr"] = "1/1"
        report = self.program(spec)
        self.assertEqual(report["summary"]["feasible_winners"], [self.row(report, ("big",))["bundle_id"]])

    def test_candidate_draws_accrue_interest_without_becoming_cfads_or_extra_nav(self):
        report = self.program(financing_example())
        both = self.row(report, ("borrow", "project"))
        self.assertEqual(report["summary"]["feasible_winners"], [both["bundle_id"]])
        self.assertEqual(F(both["statistics"]["mean"]), 100)
        self.assertEqual(F(self.row(report, ("borrow",))["statistics"]["mean"]), 0)
        first, second = self.state(both)["debt"]
        self.assertEqual([F(first[key]) for key in ("candidate_draw", "interest", "cfads", "dscr", "cash_after_service")], [50, 5, 0, 0, 20])
        self.assertEqual([F(second[key]) for key in ("baseline_cfads", "candidate_cfads", "dscr", "scheduled_principal", "cash_after_service")],
                         [60, 20, F(8, 5), 50, 30])
        self.assertEqual(self.service(self.state(self.row(report, ("borrow",))), period="second")["coverage_status"], "below_minimum")

    def test_mandatory_service_is_not_clipped_and_later_recovery_preserves_failure(self):
        spec = roll_forward_example()
        funding = spec["entity_funding"]
        funding["debt"]["facilities"][0]["sweep_fraction"] = "0/1"
        for period, value in (("first", "5"), ("second", "100")):
            funding["baseline_flows"][period]["jv"]["base"] = value
            funding["debt"]["baseline_flows"][period]["loan"]["base"]["cfads"] = value
        hold = self.row(self.program(spec), ())
        self.assertIsNone(hold["statistics"])
        state = self.state(hold)
        self.assertEqual([F(self.service(state)[key]) for key in ("scheduled_service", "cash_after_service", "dscr", "closing_principal")],
                         [30, -5, F(1, 6), 80])
        self.assertEqual(F(state["distributions"][1]["gross_distribution"]), 47)

    def test_explicit_zero_service_permission_and_full_repayment_caps(self):
        spec = with_debt(entity.idle_example([("parent", 20, 20), ("jv", 20, 20)]), [("loan", "jv", 5)])
        entity.distribution(spec, "jv", "parent", "1/3")
        funding = spec["entity_funding"]
        funding["baseline_flows"]["first"]["jv"]["base"] = "20"
        debt = funding["debt"]
        debt["facilities"][0]["sweep_fraction"] = "1/1"
        debt["baseline_flows"]["first"]["loan"]["base"]["cfads"] = "20"
        for allowed, dividend in ((True, 15), (False, 0)):
            debt["facilities"][0]["allow_distributions_without_service"] = allowed
            state = self.state(self.row(self.program(spec), ()))
            first = self.service(state)
            self.assertIsNone(first["dscr"])
            self.assertEqual(first["coverage_status"], "no_service_due")
            self.assertEqual(first["distribution_allowed"], allowed)
            self.assertEqual(F(first["sweep"]), 5)
            self.assertEqual(F(first["closing_principal"]), 0)
            self.assertEqual(F(state["distributions"][0]["gross_distribution"]), dividend)
        debt["baseline_flows"]["first"]["loan"]["base"]["principal_payment_cap"] = "10"
        state = self.state(self.row(self.program(spec), ()))
        first = self.service(state)
        self.assertEqual([F(first[key]) for key in ("principal_payment_cap", "scheduled_principal", "sweep", "dscr")], [10, 5, 0, 4])
        self.assertEqual(F(state["distributions"][0]["gross_distribution"]), 15)

    def test_realized_recourse_and_continuing_initial_debt_have_exact_two_objectives(self):
        for equity in (False, True):
            with self.subTest(equity=equity):
                report = self.program(recourse_example(equity))
                self.assertEqual(len(report["summary"]["policies"]), 6)
                winner = next(row for row in report["summary"]["policies"] if row["policy_id"] in report["summary"]["feasible_winners"])
                self.assertEqual(F(winner["statistics"]["mean"]), 45)
                self.assertEqual(F(winner["tail_loss"]), 5)
                self.assertEqual(F(report["summary"]["net_adaptive_value_per_share" if equity else "net_adaptive_value"]), F(9, 20) if equity else 45)
                good, bad = self.state(winner, "good"), self.state(winner, "bad")
                self.assertEqual(F(self.service(good, period="third")["closing_principal"]), 29)
                self.assertEqual(F(self.service(bad, period="third")["closing_principal"]), 79)
                self.assertEqual(F(self.service(bad, period="third")["candidate_principal_payment_cap"]), 1)
                self.assertEqual(F(self.service(bad, period="third")["interest"]), 8)
                if equity:
                    self.assertEqual(F(winner["per_share_statistics"]["mean"]), F(9, 20))
                    self.assertEqual(F(winner["per_share_tail_loss"]), F(1, 20))

    def test_fractional_global_tail_and_indistinguishable_observations(self):
        spec = recourse_example()
        spec["states"] = [("good", "9/10"), ("bad", "1/10")]
        spec["equity"]["max_tail_loss_per_share"] = "-9/20"
        report = self.program(spec)
        winner = next(row for row in report["summary"]["policies"] if row["policy_id"] in report["summary"]["feasible_winners"])
        self.assertEqual(F(winner["per_share_statistics"]["mean"]), F(17, 20))
        self.assertEqual(F(winner["per_share_tail_loss"]), F(-9, 20))
        spec["equity"]["max_tail_loss_per_share"] = "-1/2"
        self.assertEqual(self.program(spec)["summary"]["feasible_winners"], [])
        coarse = recourse_example()
        coarse["recourse"]["observations"] = {"good": "same", "bad": "same"}
        report = self.program(coarse)
        self.assertEqual(F(report["summary"]["net_adaptive_value_per_share"]), 0)
        self.assertEqual(len(report["summary"]["policies"]), 4)

    def test_source_order_terms_and_unused_debt_cells_bind_the_receipt(self):
        spec = roll_forward_example()
        root = fixture(model(spec))
        first = self.program(spec, root)
        self.assertEqual(first, self.program(spec, root))
        (root / FILE).write_text(model(spec, reverse=True), encoding="utf-8")
        reversed_report = self.program(spec, root)
        self.assertEqual(first["summary"], reversed_report["summary"])
        self.assertNotEqual(first["digest"], reversed_report["digest"])
        for change in ("losing_cfads", "terms_basis", "cfads_basis", "permission"):
            changed = copy.deepcopy(spec)
            debt = changed["entity_funding"]["debt"]
            if change == "losing_cfads":
                debt["candidate_flows"]["second"]["loan"]["unused"]["base"]["cfads"] = "1"
            elif change == "terms_basis":
                debt["facilities"][0]["basis"] += "; revised source terms"
            elif change == "cfads_basis":
                debt["facilities"][0]["cfads_basis"] += "; independently declared metric"
            else:
                debt["facilities"][0]["allow_distributions_without_service"] = False
            (root / FILE).write_text(model(changed), encoding="utf-8")
            amended = self.program(changed, root)
            self.assertEqual(first["summary"]["feasible_winners"], amended["summary"]["feasible_winners"])
            self.assertNotEqual(first["inputs"]["entity_funding"]["value_tag"], amended["inputs"]["entity_funding"]["value_tag"])
            self.assertNotEqual(first["digest"], amended["digest"])

    def test_incomplete_duplicate_unknown_orphan_and_malformed_debt_refuse(self):
        spec = recourse_example()
        text, blocks = model(spec), debt_blocks(spec)
        for table in ("entity_debt", "debt_facility", "debt_opening_principal", "debt_service", "candidate_debt_service"):
            row = next(row for row in blocks if f"[[{table}]]" in row)
            with self.subTest(table=table):
                self.refused(text.replace(row, "", 1))
                self.refused(text + row)
        for old, new in [
            ('cash_flow_convention = "native_excludes_modeled_debt"', 'cash_flow_convention = "already_net"'),
            ('minimum_dscr = "3/2"', 'minimum_dscr = "0/1"'),
            ('sweep_fraction = "1/2"', 'sweep_fraction = "3/2"'),
            ('allow_distributions_without_service = "true"', 'allow_distributions_without_service = true'),
            ('interest_rate = "1/10"', 'interest_rate = "0.1"'),
            ('interest_rate = "1/10"', 'interest_rate = "-1/10"'),
            ('principal_payment_cap = "20"', 'principal_payment_cap = "-1"'),
            ('facility = "loan"', 'facility = "foreign"'),
        ]:
            with self.subTest(field=old):
                self.refused(text.replace(old, new, 1))
        self.refused(text.replace('cash_flow_convention = "native_excludes_modeled_debt"',
                                 'cash_flow_convention = "native_excludes_modeled_debt"\nimplicit_topup = "yes"'), "506")
        self.refused(text + base.block("guessed_dscr", {"value": "2/1"}), "506")
        self.refused(model(spec, include_uu=False))

    def test_pre_observation_debt_actions_and_metric_only_study_costs_refuse(self):
        for field, value in (("draw", "1"), ("principal_payment_cap", "1"), ("cfads", "1"), ("cfads", "-1")):
            spec = recourse_example()
            spec["entity_funding"]["debt"]["candidate_flows"]["first"]["loan"]["project"]["good"][field] = value
            self.refused(model(spec), "105")
        spec = recourse_example()
        entity.native(spec, "first", "parent", "study", [0, 0])
        for state in ("good", "bad"):
            spec["entity_funding"]["debt"]["candidate_flows"]["first"]["loan"]["study"][state]["cfads"] = "-5"
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
