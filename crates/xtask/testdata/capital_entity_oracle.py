"""Independent exact entity cash and policy checks against the actual fmine binary.

The reference computes each entity's native cash and fixed transfers directly,
then solves the acyclic distribution equations recursively. It does not read
Rust's evaluation order, intermediate ledger, static subset cache or winners.
Only the earlier independent oracle's scalar Fraction statistics and input
scaffolding are reused. Invented inputs are assumptions, not issuer evidence.
"""

import argparse
import copy
from fractions import Fraction as F
import hashlib
import importlib.util
import itertools
import json
from pathlib import Path
import tempfile
import unittest

_module = importlib.util.spec_from_file_location("capital_reference", Path(__file__).with_name("capital_program_oracle.py"))
base = importlib.util.module_from_spec(_module)
_module.loader.exec_module(base)
FILE = base.FILE
TWO_PERIODS = [("first", "2030-01-01/2030-06-30"), ("second", "2030-07-01/2030-12-31")]
THREE_PERIODS = [("first", "2030-01-01/2030-04-30"), ("second", "2030-05-01/2030-08-31"),
                 ("third", "2030-09-01/2030-12-31")]
CASH_BASIS = 'Invented legal-entity USD cash, explicit funding calls and permissions \\ "Ω"'
DISTRIBUTION_BASIS = "Invented fixed ownership; complementary distribution paid outside this graph"
TRANSFER_BASIS = "Invented fixed capital call; no automatic top-up or dilution"


def with_entities(spec, entities, periods=TWO_PERIODS):
    states = [state for state, _ in spec["states"]]
    candidates = [row["id"] for row in spec["candidates"]]
    spec["entity_funding"] = {
        "cash_basis": CASH_BASIS, "opening_date": "2030-01-01", "objective_entity": "parent",
        "entities": {entity: f"Invented {entity}" for entity, _, _ in entities},
        "periods": [{"id": period, "cash_period": dates} for period, dates in periods],
        "floors": {period: {entity: str(floor) for entity, _, floor in entities} for period, _ in periods},
        "opening_cash": {entity: {state: str(opening) for state in states} for entity, opening, _ in entities},
        "baseline_flows": {period: {entity: {state: "0" for state in states} for entity, _, _ in entities}
                           for period, _ in periods},
        "candidate_flows": {period: {entity: {candidate: {state: "0" for state in states} for candidate in candidates}
                                     for entity, _, _ in entities} for period, _ in periods},
        "transfers": [], "distributions": [], "gates": {period: {} for period, _ in periods},
    }
    return spec


def distribution(spec, child, parent, share, cap="1000"):
    funding = spec["entity_funding"]
    funding["distributions"].append({"entity_id": child, "parent_entity_id": parent,
                                     "parent_share": base.ratio(share), "basis": DISTRIBUTION_BASIS})
    for period in funding["periods"]:
        funding["gates"][period["id"]][child] = {state: {"allowed": True, "maximum_distribution": str(cap)}
                                                for state, _ in spec["states"]}
    return spec


def transfer(spec, name, payer, recipient, amounts, candidate=None, period="first"):
    spec["entity_funding"]["transfers"].append({
        "id": name, "from_entity": payer, "to_entity": recipient, "period_id": period,
        "candidate_id": candidate, "amounts": dict(zip((s for s, _ in spec["states"]), map(str, amounts), strict=True)),
        "basis": TRANSFER_BASIS,
    })


def native(spec, period, entity, candidate, values):
    spec["entity_funding"]["candidate_flows"][period][entity][candidate] = dict(
        zip((state for state, _ in spec["states"]), map(str, values), strict=True))


def ring_fence_example():
    spec = with_entities(base.specification({"big": [30], "small": [10]}, states=[("base", "1/1")],
                         costs={"big": "40", "small": "10"}, budget="40"),
                         [("parent", 30, 20), ("jv", 100, 20)])
    native(spec, "first", "parent", "big", [-40])
    native(spec, "first", "parent", "small", [-10])
    return spec


def calls_example():
    spec = with_entities(base.specification({"build": [50, 50]}, states=[("good", "1/2"), ("bad", "1/2")],
                         costs={"build": "60"}, budget="60"),
                         [("parent", 100, 20), ("partner", 40, 0), ("jv", 20, 20)])
    transfer(spec, "parent_call", "parent", "jv", [60, 60], "build")
    transfer(spec, "partner_call", "partner", "jv", [40, 40], "build")
    native(spec, "second", "jv", "build", [-100, -100])
    return spec


def idle_example(entities, periods=TWO_PERIODS):
    return with_entities(base.specification({"unused": [-1]}, states=[("base", "1/1")], budget="0"), entities, periods)


def recourse_example(equity=True):
    spec = with_entities(base.specification({"study": [-5, -5], "project": [100, -20]},
                         states=[("good", "1/2"), ("bad", "1/2")], costs={"study": "5", "project": "50"},
                         budget="55", requires=[("project", "study")]),
                         [("parent", 25, 20), ("jv", 20, 20)], THREE_PERIODS)
    native(spec, "first", "parent", "study", [-5, -5])
    native(spec, "third", "parent", "project", [-50, -50])
    spec["entity_funding"]["baseline_flows"]["second"]["jv"] = {"good": "100", "bad": "0"}
    distribution(spec, "jv", "parent", "3/5")
    spec["recourse"] = {"information_candidate": "study", "observation_date": "2030-05-01",
                        "information_basis": "Invented study reveals Good or Bad before second-period receipts and third-period spending",
                        "stages": {"study": "initial", "project": "recourse"},
                        "observations": {"good": "good", "bad": "bad"}}
    if equity:
        base.with_equity(spec, [1000, 1000], {}, cap="1/20")
        spec["equity"]["entity"] = "parent"
    return spec


def model(spec, *, reverse=False, include_uu=True):
    f = spec["entity_funding"]
    rows = [base.block("entity_funding", {key: f[key] for key in ("cash_basis", "opening_date", "objective_entity")})]
    rows += [base.block("entity", {"id": entity, "label": label}) for entity, label in f["entities"].items()]
    rows += [base.block("entity_period", period) for period in f["periods"]]
    rows += [base.block("entity_floor", {"entity": entity, "period": period, "value": value})
             for period, entities in f["floors"].items() for entity, value in entities.items()]
    rows += [base.block("entity_opening_cash", {"entity": entity, "state": state, "value": value})
             for entity, states in f["opening_cash"].items() for state, value in states.items()]
    rows += [base.block("entity_baseline_cash_flow", {"entity": entity, "period": period, "state": state, "value": value})
             for period, entities in f["baseline_flows"].items() for entity, states in entities.items() for state, value in states.items()]
    rows += [base.block("entity_candidate_cash_flow", {"entity": entity, "period": period, "candidate": name, "state": state, "value": value})
             for period, entities in f["candidate_flows"].items() for entity, candidates in entities.items()
             for name, states in candidates.items() for state, value in states.items()]
    for item in f["transfers"]:
        declaration = {"id": item["id"], "from_entity": item["from_entity"], "to_entity": item["to_entity"],
                       "period": item["period_id"], "basis": item["basis"]}
        if item["candidate_id"] is not None:
            declaration["candidate"] = item["candidate_id"]
        rows.append(base.block("entity_transfer", declaration))
        rows += [base.block("entity_transfer_amount", {"transfer": item["id"], "state": state, "value": value})
                 for state, value in item["amounts"].items()]
    rows += [base.block("entity_distribution", {"entity": item["entity_id"], "parent_entity": item["parent_entity_id"],
                                               "parent_share": item["parent_share"], "basis": item["basis"]})
             for item in f["distributions"]]
    rows += [base.block("entity_distribution_gate", {"entity": entity, "period": period, "state": state,
                                                     "allowed": str(gate["allowed"]).lower(), "maximum_distribution": gate["maximum_distribution"]})
             for period, entities in f["gates"].items() for entity, states in entities.items() for state, gate in states.items()]
    if recourse := spec.get("recourse"):
        rows.append(base.block("recourse", {key: recourse[key] for key in ("information_candidate", "observation_date", "information_basis")}))
        rows += [base.block("candidate_stage", {"candidate": candidate, "stage": stage}) for candidate, stage in recourse["stages"].items()]
        rows += [base.block("state_observation", {"state": state, "observation": observation}) for state, observation in recourse["observations"].items()]
    return base.model(spec, reverse=reverse, include_uu=include_uu) + "".join(reversed(rows) if reverse else rows)


def fixture(text=None):
    root = Path(tempfile.mkdtemp(prefix="fmine-capital-entity-"))
    (root / FILE).write_text(model(recourse_example()) if text is None else text, encoding="utf-8")
    return root


def serialized(value):
    if isinstance(value, F):
        return base.ratio(value)
    if isinstance(value, dict):
        return {key: serialized(item) for key, item in value.items()}
    if isinstance(value, (tuple, list)):
        return [serialized(item) for item in value]
    return value


def entity_ledger(f, selected, state):
    entities = sorted(f["entities"])
    periods = sorted(f["periods"], key=lambda row: (row["cash_period"], row["id"]))
    balances = {entity: F(f["opening_cash"][entity][state]) for entity in entities}
    edges = {row["entity_id"]: row for row in f["distributions"]}
    children = {entity: [child for child, edge in edges.items() if edge["parent_entity_id"] == entity] for entity in entities}
    checkpoints, transfers, distributions, conservation = [], [], [], []

    def checkpoint(entity, period, date, phase, opening, baseline=F(0), candidate=F(0), tin=F(0), tout=F(0), din=F(0), dout=F(0)):
        floor = F(f["floors"][period or periods[0]["id"]][entity])
        closing = opening + baseline + candidate + tin - tout + din - dout
        return {"entity_id": entity, "period_id": period, "checkpoint_date": date, "phase": phase,
                "opening_cash": opening, "baseline_cash_flow": baseline, "candidate_cash_flow": candidate,
                "transfer_in": tin, "transfer_out": tout, "distribution_in": din, "distribution_out": dout,
                "closing_cash": closing, "minimum_cash": floor, "slack": closing - floor}

    for entity in entities:
        checkpoints.append(checkpoint(entity, None, f["opening_date"], "opening", balances[entity]))
    for period in periods:
        pid, closing_date = period["id"], period["cash_period"].split("/")[1]
        opening = balances.copy()
        baseline = {entity: F(f["baseline_flows"][pid][entity][state]) for entity in entities}
        candidate = {entity: sum((F(f["candidate_flows"][pid][entity][name][state]) for name in selected), F(0)) for entity in entities}
        tin, tout = dict.fromkeys(entities, F(0)), dict.fromkeys(entities, F(0))
        for item in sorted((t for t in f["transfers"] if t["period_id"] == pid), key=lambda t: t["id"]):
            active = item["candidate_id"] is None or item["candidate_id"] in selected
            declared = F(item["amounts"][state])
            amount = declared if active else F(0)
            tin[item["to_entity"]] += amount
            tout[item["from_entity"]] += amount
            transfers.append({"transfer_id": item["id"], "from_entity": item["from_entity"], "to_entity": item["to_entity"],
                              "period_id": pid, "checkpoint_date": closing_date, "candidate_id": item["candidate_id"],
                              "basis": item["basis"], "active": active, "declared_amount": declared, "actual_amount": amount})
        pre = {entity: opening[entity] + baseline[entity] + candidate[entity] + tin[entity] - tout[entity] for entity in entities}
        checkpoints += [checkpoint(entity, pid, closing_date, "pre_distribution", opening[entity], baseline[entity], candidate[entity],
                                   tin[entity], tout[entity]) for entity in entities]
        gross, incoming, results = {}, {}, {}

        def payout(entity):
            if entity in gross:
                return gross[entity]
            incoming[entity] = sum((F(edges[child]["parent_share"]) * payout(child) for child in children[entity]), F(0))
            if entity not in edges:
                gross[entity] = F(0)
                return F(0)
            edge, gate = edges[entity], f["gates"][pid][entity][state]
            available_cash = pre[entity] + incoming[entity]
            floor = F(f["floors"][pid][entity])
            surplus = max(F(0), available_cash - floor)
            gross[entity] = min(F(gate["maximum_distribution"]), surplus) if gate["allowed"] else F(0)
            parent_share = F(edge["parent_share"])
            results[entity] = {"entity_id": entity, "parent_entity_id": edge["parent_entity_id"], "period_id": pid,
                               "checkpoint_date": closing_date, "basis": edge["basis"], "parent_share": parent_share,
                               "external_share": 1 - parent_share, "allowed": gate["allowed"],
                               "maximum_distribution": F(gate["maximum_distribution"]), "cash_before_distribution": available_cash,
                               "minimum_cash": floor, "available_surplus": surplus, "gross_distribution": gross[entity],
                               "parent_receipt": gross[entity] * parent_share, "external_distribution": gross[entity] * (1 - parent_share),
                               "cash_after_distribution": available_cash - gross[entity]}
            return gross[entity]

        for entity in entities:
            payout(entity)
        distributions.extend(results.values())
        for entity in entities:
            row = checkpoint(entity, pid, closing_date, "closing", opening[entity], baseline[entity], candidate[entity],
                             tin[entity], tout[entity], incoming[entity], gross[entity])
            checkpoints.append(row)
            balances[entity] = row["closing_cash"]
        outside = sum((item["external_distribution"] for item in results.values()), F(0))
        residual = sum(balances.values()) + outside - sum(opening.values()) - sum(baseline.values()) - sum(candidate.values())
        assert residual == 0
        conservation.append({"period_id": pid, "checkpoint_date": closing_date, "opening_cash": sum(opening.values()),
                             "baseline_cash_flow": sum(baseline.values()), "candidate_cash_flow": sum(candidate.values()),
                             "internal_transfers": sum(tin.values()), "internal_distributions": sum(incoming.values()),
                             "external_distributions": outside, "closing_cash": sum(balances.values()), "residual": residual})
    return {"state_id": state, "checkpoints": checkpoints, "transfers": transfers, "distributions": distributions,
            "conservation": conservation, "minimum_slack": min(row["slack"] for row in checkpoints)}


def subsets(names):
    return [selected for count in range(len(names) + 1) for selected in itertools.combinations(sorted(names), count)]


def structural(spec, selected):
    candidates = {row["id"]: row for row in spec["candidates"]}
    reasons = []
    if sum((F(candidates[name]["cost"]) for name in selected), F(0)) > F(spec["budget"]):
        reasons.append({"kind": "budget_exceeded"})
    reasons += [{"kind": "missing_prerequisite", "candidate": name, "prerequisite": prerequisite}
                for name, prerequisite in spec["requires"] if name in selected and prerequisite not in selected]
    groups = {}
    for name in selected:
        if group := candidates[name].get("exclusion_group"):
            groups.setdefault(group, []).append(name)
    reasons += [{"kind": "mutual_exclusion", "group": group, "candidates": names} for group, names in groups.items() if len(names) > 1]
    return reasons


def realized(spec, selected, state):
    cost = {row["id"]: F(row["cost"]) for row in spec["candidates"]}
    spend = sum((cost[name] for name in selected), F(0))
    ledger = entity_ledger(spec["entity_funding"], selected, state)
    reasons = [{"kind": "entity_cash_shortfall", "entity_id": row["entity_id"], "state_id": state,
                "period_id": row["period_id"], "checkpoint_date": row["checkpoint_date"], "phase": row["phase"], "shortfall": -row["slack"]}
               for row in ledger["checkpoints"] if row["slack"] < 0]
    equity = base.equity_states(spec, selected, [state])[0] if "equity" in spec else None
    if equity and equity["ending_shares"] <= 0:
        reasons.append({"kind": "nonpositive_share_count", "state_id": state, "shares": equity["ending_shares"]})
    return {"selected": selected, "spend": spend, "budget_slack": F(spec["budget"]) - spend,
            "net_nav": sum((F(spec["payoffs"][name][state]) for name in selected), F(0)),
            "ledger": ledger, "reasons": reasons, "equity": equity}


def finish(spec, rows):
    weights = {state: F(weight) for state, weight in spec["states"]}
    equity = "equity" in spec
    cap = spec["equity"]["max_tail_loss_per_share"] if equity else spec["cap"]
    cap = None if cap is None else F(cap)
    admissible = {key: row for key, row in rows.items() if not row["rejections"]}
    for row in admissible.values():
        values = {state: item["net_nav"] for state, item in row["states"].items()}
        row.update(base.payoff_statistics(values, weights, F(spec["tail"])))
        if equity:
            values = {state: item["equity"]["delta_nav_per_share"] for state, item in row["states"].items()}
            row["per_share"] = base.payoff_statistics(values, weights, F(spec["tail"]))
        row["risk_rejected"] = cap is not None and (row["per_share"] if equity else row)["tail_loss"] > cap
    for normalized in (False, True) if equity else (False,):
        population = [row["per_share"] if normalized else row for row in admissible.values()]
        best = {state: max(row["values"][state] for row in population) for state in weights} if population else {}
        for row in population:
            regrets = {state: best[state] - value for state, value in row["values"].items()}
            row.update(hindsight=best, regrets=regrets, mean_regret=sum(weights[state] * regret for state, regret in regrets.items()),
                       max_regret=max(regrets.values()))
    feasible = {key: row for key, row in admissible.items() if not row["risk_rejected"]}

    def objective(row):
        return (row["per_share"] if equity else row)["mean"]

    def winners(population):
        maximum = max((objective(row) for row in population.values()), default=None)
        return {key for key, row in population.items() if objective(row) == maximum}, maximum

    before, before_value = winners(admissible)
    after, after_value = winners(feasible)
    return {"rows": rows, "weights": weights, "before": before, "after": after,
            "before_value": before_value, "after_value": after_value,
            "risk_cost": None if after_value is None else before_value - after_value}


def expected_static(spec):
    rows = {}
    for selected in subsets([row["id"] for row in spec["candidates"]]):
        states = {state: realized(spec, selected, state) for state, _ in sorted(spec["states"])}
        rows[selected] = {"states": states, "rejections": structural(spec, selected)
                          + [reason for state in states.values() for reason in state["reasons"]]}
    return finish(spec, rows)


def key_of(policy):
    return tuple(policy["initial_selected"]), tuple((branch["observation_id"], tuple(branch["selected"])) for branch in policy["branches"])


def expected_policies(spec):
    recourse = spec["recourse"]
    initial = [name for name, stage in recourse["stages"].items() if stage == "initial"]
    later = [name for name, stage in recourse["stages"].items() if stage == "recourse"]
    candidates = {row["id"]: row for row in spec["candidates"]}
    weights = {state: F(weight) for state, weight in spec["states"]}
    rows = {}
    for committed in subsets(initial):
        acquired = recourse["information_candidate"] in committed
        observations = sorted(set(recourse["observations"].values())) if acquired else [None]
        for actions in itertools.product(subsets(later), repeat=len(observations)):
            continuation = dict(zip(observations, actions, strict=True))
            branches = [{"observation_id": observation, "selected": selected,
                         "probability": sum((weight for state, weight in weights.items()
                                             if not acquired or recourse["observations"][state] == observation), F(0)),
                         "spend": sum((F(candidates[name]["cost"]) for name in selected), F(0))}
                        for observation, selected in continuation.items()]
            states, reasons = {}, []
            for state in sorted(weights):
                observation = recourse["observations"][state] if acquired else None
                selected = tuple(sorted((*committed, *continuation[observation])))
                row = realized(spec, selected, state)
                row["observation_id"] = observation
                states[state] = row
                reasons += [{"state_id": state, "reason": reason} for reason in structural(spec, selected) + row["reasons"]]
            rows[(committed, tuple(continuation.items()))] = {
                "states": states, "rejections": reasons, "branches": branches, "information_acquired": acquired,
                "initial_spend": sum((F(candidates[name]["cost"]) for name in committed), F(0)),
            }
    return finish(spec, rows)


class CapitalEntityScenarios(unittest.TestCase):
    envelope = base.CapitalProgramScenarios.envelope
    refused = base.CapitalProgramScenarios.refused
    check_statistics = base.CapitalProgramScenarios.check_statistics

    def program(self, spec, root=None):
        root = fixture(model(spec)) if root is None else root
        raw = (root / FILE).read_bytes()
        result = self.envelope(root)
        self.assertEqual(result["exit_code"], 0, result)
        self.assertIsNone(result["refusal"])
        source = {"path": str(root / FILE), "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}
        self.assertEqual(result["payload"]["source"], source)
        self.assertEqual((root / FILE).read_bytes(), raw)
        report = result["payload"]["program"]
        adaptive = "recourse" in spec
        self.assertEqual(result["payload"]["mode"], "declared_two_stage_capital_comparison" if adaptive else "declared_static_capital_comparison")
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
                self.assertEqual(None if report["summary"][field + suffix] is None else F(report["summary"][field + suffix]), expected)
                self.assertIsNone(report["summary"][field if suffix else field + "_per_share"])
            info = report["inputs"]["information"]
            self.assertEqual(set(report["claim"]["parents"]), set(static["claim"]["parents"]) | {info["id"]})
            self.check_claims([info, report["claim"]], source)
        for flag in ("reserve_disclosure_claim", "execution_authorized", "geotechnical_signoff", "regulatory_compliance_claim"):
            self.assertFalse(report[flag])
        return report

    def check_rows(self, report, spec, expected, adaptive):
        summary = report["summary"]
        rows = {key_of(row) if adaptive else tuple(row["selected"]): row for row in summary["policies" if adaptive else "choices"]}
        self.assertEqual(set(rows), set(expected["rows"]))
        self.assertEqual(len(rows), len(summary["policies" if adaptive else "choices"]))
        for key, row in rows.items():
            known = expected["rows"][key]
            self.assertCountEqual(row["structural_rejections"], serialized(known["rejections"]))
            ledger = row["entity_funding"]
            self.assertEqual([item["state_id"] for item in ledger["states"]], sorted(known["states"]))
            for item in ledger["states"]:
                declared = known["states"][item["state_id"]]["ledger"]
                for field in ("state_id", "checkpoints", "transfers", "conservation", "minimum_slack"):
                    self.assertEqual(item[field], serialized(declared[field]), (key, item["state_id"], field))
                self.assertCountEqual(item["distributions"], serialized(declared["distributions"]))
                self.assertTrue(all(F(row["residual"]) == 0 for row in item["conservation"]))
            self.assertEqual(F(ledger["minimum_slack"]), min(item["ledger"]["minimum_slack"] for item in known["states"].values()))
            equity = {"states": [item["equity"] for item in known["states"].values()]} if "equity" in spec else None
            self.assertEqual(row.get("equity"), serialized(equity))
            if adaptive:
                self.assertEqual(row["branches"], serialized(known["branches"]))
                self.assertEqual(row["information_acquired"], known["information_acquired"])
                self.assertEqual(F(row["initial_spend"]), known["initial_spend"])
                self.assertIsNone(row["funding_schedule"])
                for item in row["states"]:
                    declared = known["states"][item["state_id"]]
                    self.assertEqual(tuple(item["selected"]), declared["selected"])
                    self.assertEqual(item["observation_id"], declared["observation_id"])
                    for field in ("spend", "budget_slack", "net_nav"):
                        self.assertEqual(F(item[field]), declared[field])
            else:
                first = next(iter(known["states"].values()))
                self.assertEqual(F(row["spend"]), first["spend"])
                self.assertEqual(F(row["budget_slack"]), first["budget_slack"])
            if known["rejections"]:
                self.assertIsNone(row["statistics"])
                self.assertIsNone(row["tail_loss"])
                self.assertIsNone(row.get("per_share_statistics"))
                self.assertFalse(row["risk_rejected"])
            else:
                self.check_statistics(row["statistics"], known, expected["weights"], key)
                self.assertEqual(F(row["tail_loss"]), known["tail_loss"])
                self.assertEqual(row["risk_rejected"], known["risk_rejected"])
                if "equity" in spec:
                    self.check_statistics(row["per_share_statistics"], known["per_share"], expected["weights"], key)
                    self.assertEqual(F(row["per_share_tail_loss"]), known["per_share"]["tail_loss"])
        membership = {row["policy_id" if adaptive else "bundle_id"]: key for key, row in rows.items()}
        for field, known in (("unconstrained_winners", "before"), ("feasible_winners", "after")):
            self.assertEqual({membership[value] for value in summary[field]}, expected[known])
        field = "expected_per_share_cost_of_risk_limit" if "equity" in spec else "expected_value_cost_of_risk_limit"
        self.assertEqual(None if summary[field] is None else F(summary[field]), expected["risk_cost"])

    def check_claims(self, claims, source):
        for claim in claims:
            self.assertTrue(bytes.fromhex(claim["canonical_body_hex"]))
            self.assertEqual(claim["representation"], "deterministic")
            self.assertEqual(claim["evidence"], "assumed")
            self.assertEqual(claim["state"], "estimated")
            self.assertFalse(claim["stale"])
            self.assertTrue(claim["uu_terms"])
            self.assertTrue(claim["anchors"])
            self.assertTrue(all(anchor["artifact_path"] == source["path"] and anchor["sha256"] == source["sha256"]
                                and anchor["locator"] for anchor in claim["anchors"]))

    def check_evidence(self, report, source, spec):
        inputs, f = report["inputs"], spec["entity_funding"]
        claim = inputs["entity_funding"]
        value = claim["value"]
        self.assertEqual(value, report["summary"]["entity_funding"])
        for field in ("cash_basis", "opening_date", "objective_entity"):
            self.assertEqual(value[field], f[field])
        self.assertEqual(value["currency"], "USD")
        entity_count, period_count, state_count = len(f["entities"]), len(f["periods"]), len(spec["states"])
        for field, count in (("entities", entity_count), ("periods", period_count),
                             ("floors", entity_count * period_count), ("opening_cash", entity_count * state_count),
                             ("baseline_flows", entity_count * period_count * state_count),
                             ("candidate_flows", entity_count * period_count * state_count * len(spec["candidates"])),
                             ("transfers", len(f["transfers"])), ("distributions", len(f["distributions"])),
                             ("gates", len(f["distributions"]) * period_count * state_count)):
            self.assertEqual(len(value[field]), count, field)
        self.assertEqual({row["id"]: row["label"] for row in value["entities"]}, f["entities"])
        self.assertEqual(value["periods"], sorted(f["periods"], key=lambda row: (row["cash_period"], row["id"])))
        self.assertEqual({(row["entity_id"], row["period_id"]): F(row["minimum_cash"]) for row in value["floors"]},
                         {(entity, period): F(amount) for period, entities in f["floors"].items() for entity, amount in entities.items()})
        self.assertEqual({(row["entity_id"], row["state_id"]): F(row["value"]) for row in value["opening_cash"]},
                         {(entity, state): F(amount) for entity, states in f["opening_cash"].items() for state, amount in states.items()})
        self.assertEqual({(row["entity_id"], row["period_id"], row["state_id"]): F(row["value"]) for row in value["baseline_flows"]},
                         {(entity, period, state): F(amount) for period, entities in f["baseline_flows"].items()
                          for entity, states in entities.items() for state, amount in states.items()})
        self.assertEqual({(row["entity_id"], row["period_id"], row["candidate_id"], row["state_id"]): F(row["value"])
                          for row in value["candidate_flows"]},
                         {(entity, period, candidate, state): F(amount) for period, entities in f["candidate_flows"].items()
                          for entity, candidates in entities.items() for candidate, states in candidates.items() for state, amount in states.items()})
        self.assertCountEqual(value["distributions"], f["distributions"])
        for actual, known in zip(sorted(value["transfers"], key=lambda row: row["id"]), sorted(f["transfers"], key=lambda row: row["id"]), strict=True):
            for field in ("id", "from_entity", "to_entity", "period_id", "candidate_id", "basis"):
                self.assertEqual(actual[field], known[field])
            self.assertEqual({row["state_id"]: F(row["value"]) for row in actual["amounts"]},
                             {state: F(amount) for state, amount in known["amounts"].items()})
        self.assertEqual({(row["entity_id"], row["period_id"], row["state_id"]): (row["allowed"], F(row["maximum_distribution"])) for row in value["gates"]},
                         {(entity, period, state): (gate["allowed"], F(gate["maximum_distribution"])) for period, entities in f["gates"].items()
                          for entity, states in entities.items() for state, gate in states.items()})
        suffixes = (["entity funding objective, opening date and cash basis"]
                    + [f"entity {entity}" for entity in f["entities"]]
                    + [f"entity period {period['id']}" for period in f["periods"]]
                    + [f"entity floor {row['entity_id']}/{row['period_id']}" for row in value["floors"]]
                    + [f"entity opening cash {row['entity_id']}/{row['state_id']}" for row in value["opening_cash"]]
                    + [f"entity baseline cash flow {row['entity_id']}/{row['period_id']}/{row['state_id']}" for row in value["baseline_flows"]]
                    + [f"entity candidate cash flow {row['entity_id']}/{row['period_id']}/{row['candidate_id']}/{row['state_id']}" for row in value["candidate_flows"]]
                    + [f"entity transfer {row['id']}" for row in value["transfers"]]
                    + [f"entity transfer amount {row['id']}/{amount['state_id']}" for row in value["transfers"] for amount in row["amounts"]]
                    + [f"entity distribution {row['entity_id']}/{row['parent_entity_id']}" for row in value["distributions"]]
                    + [f"entity distribution gate {row['entity_id']}/{row['period_id']}/{row['state_id']}" for row in value["gates"]])
        locators = [anchor["locator"] for anchor in claim["anchors"]]
        for suffix in suffixes:
            self.assertTrue(any(locator.endswith(": " + suffix) for locator in locators), suffix)
        parents = [inputs["policy"], inputs["weights"], *inputs["candidates"], claim]
        if "equity" in spec:
            parents.append(inputs["equity"])
        self.assertEqual(set(report["claim"]["parents"]), {item["id"] for item in parents})
        self.assertEqual(report["claim"]["value"], report["summary"])
        self.check_claims([*parents, report["claim"]], source)

    @staticmethod
    def row(report, selected):
        return next(row for row in report["summary"]["choices"] if row["selected"] == list(selected))

    def test_ring_fenced_cash_changes_the_selected_project(self):
        report = self.program(ring_fence_example())
        small = self.row(report, ("small",))
        self.assertEqual(report["summary"]["feasible_winners"], [small["bundle_id"]])
        self.assertEqual(F(small["statistics"]["mean"]), 10)
        self.assertIsNone(self.row(report, ("big",))["statistics"])

    def test_fixed_calls_and_missing_partner_funding(self):
        spec = calls_example()
        report = self.program(spec)
        self.assertEqual(F(self.row(report, ("build",))["statistics"]["mean"]), 50)
        spec["entity_funding"]["transfers"][1]["amounts"]["bad"] = "0"
        failed = self.program(spec)
        self.assertIsNone(self.row(failed, ("build",))["statistics"])
        hold = self.row(failed, ())
        self.assertEqual(failed["summary"]["feasible_winners"], [hold["bundle_id"]])

    def test_cap_retains_cash_and_outside_share_is_never_parent_cash(self):
        spec = distribution(idle_example([("parent", 20, 20), ("jv", 100, 20)]), "jv", "parent", "3/5", "50")
        report = self.program(spec)
        state = self.row(report, ())["entity_funding"]["states"][0]
        self.assertEqual([F(row["external_distributions"]) for row in state["conservation"]], [20, 12])
        self.assertEqual([F(row["closing_cash"]) for row in state["conservation"]], [100, 88])

    def test_thirds_remain_exact_and_zero_or_full_parent_shares_conserve_cash(self):
        for share, parent, outside in [("1/3", F(1, 3), F(2, 3)), ("0/1", F(0), F(1)), ("1/1", F(1), F(0))]:
            with self.subTest(share=share):
                spec = distribution(idle_example([("parent", 20, 20), ("jv", 21, 20)]), "jv", "parent", share, "1")
                row = self.row(self.program(spec), ())["entity_funding"]["states"][0]["distributions"][0]
                self.assertEqual(F(row["parent_receipt"]), parent)
                self.assertEqual(F(row["external_distribution"]), outside)

    def test_multilevel_ownership_is_resolved_child_first(self):
        spec = with_entities(base.specification({"big": [40], "small": [30]}, states=[("base", "1/1")],
                             costs={"big": "40", "small": "38"}, budget="40"),
                             [("parent", 20, 20), ("holding", 10, 10), ("mine", 100, 20)])
        distribution(spec, "holding", "parent", "4/5")
        distribution(spec, "mine", "holding", "3/5")
        native(spec, "second", "parent", "big", [-40])
        native(spec, "second", "parent", "small", [-38])
        report = self.program(spec)
        small = self.row(report, ("small",))
        self.assertEqual(report["summary"]["feasible_winners"], [small["bundle_id"]])
        conservation = small["entity_funding"]["states"][0]["conservation"][0]
        self.assertEqual(F(conservation["external_distributions"]), F(208, 5))
        self.assertEqual(F(conservation["closing_cash"]), F(442, 5))

    def test_locked_cash_carries_and_later_release_keeps_the_earlier_failure(self):
        spec = with_entities(base.specification({"early": [50], "wait": [30]}, states=[("base", "1/1")],
                             costs={"early": "30", "wait": "30"}, budget="30"), [("parent", 20, 20), ("jv", 20, 20)], THREE_PERIODS)
        distribution(spec, "jv", "parent", "3/5")
        f = spec["entity_funding"]
        f["baseline_flows"]["first"]["jv"]["base"] = "100"
        f["gates"]["first"]["jv"]["base"]["allowed"] = False
        native(spec, "first", "parent", "early", [-30])
        native(spec, "third", "parent", "wait", [-30])
        report = self.program(spec)
        wait = self.row(report, ("wait",))
        self.assertEqual(report["summary"]["feasible_winners"], [wait["bundle_id"]])
        early = self.row(report, ("early",))
        self.assertIsNone(early["statistics"])
        self.assertTrue(any(row["period_id"] == "first" and F(row["shortfall"]) == 30 for row in early["structural_rejections"]))

    def test_same_period_dividend_does_not_erase_the_pre_distribution_breach(self):
        spec = ring_fence_example()
        distribution(spec, "jv", "parent", "3/5")
        report = self.program(spec)
        big = self.row(report, ("big",))
        self.assertIsNone(big["statistics"])
        rows = big["entity_funding"]["states"][0]["checkpoints"]
        parent = {row["phase"]: F(row["closing_cash"]) for row in rows if row["entity_id"] == "parent" and row["period_id"] == "first"}
        self.assertEqual(parent, {"pre_distribution": F(-10), "closing": F(38)})

    def test_recourse_realized_states_have_exact_usd_and_per_share_adaptive_value(self):
        for equity in (False, True):
            with self.subTest(equity=equity):
                report = self.program(recourse_example(equity))
                winner = next(row for row in report["summary"]["policies"] if row["policy_id"] in report["summary"]["feasible_winners"])
                self.assertEqual(F(winner["statistics"]["mean"]), 45)
                self.assertEqual(F(winner["tail_loss"]), 5)
                field = "net_adaptive_value_per_share" if equity else "net_adaptive_value"
                self.assertEqual(F(report["summary"][field]), F(9, 20) if equity else 45)
                if equity:
                    self.assertEqual(F(winner["per_share_statistics"]["mean"]), F(9, 20))
                    self.assertEqual(F(winner["per_share_tail_loss"]), F(1, 20))

    def test_rows_sources_ownership_gates_and_losing_inputs_bind(self):
        spec = distribution(ring_fence_example(), "jv", "parent", "3/5", "0")
        root = fixture(model(spec))
        first = self.program(spec, root)
        self.assertEqual(first, self.program(spec, root))
        (root / FILE).write_text(model(spec, reverse=True), encoding="utf-8")
        reordered = self.program(spec, root)
        self.assertEqual(first["summary"], reordered["summary"])
        self.assertNotEqual(first["digest"], reordered["digest"])
        for change in ("loser", "ownership", "gate_basis"):
            changed = copy.deepcopy(spec)
            if change == "loser":
                native(changed, "first", "jv", "big", [-1])
            elif change == "ownership":
                changed["entity_funding"]["distributions"][0]["parent_share"] = "1/2"
            else:
                changed["entity_funding"]["distributions"][0]["basis"] += "; amended declaration"
            (root / FILE).write_text(model(changed), encoding="utf-8")
            amended = self.program(changed, root)
            self.assertEqual(first["summary"]["feasible_winners"], amended["summary"]["feasible_winners"])
            self.assertNotEqual(first["inputs"]["entity_funding"]["value_tag"], amended["inputs"]["entity_funding"]["value_tag"])
            self.assertNotEqual(first["digest"], amended["digest"])

    def test_incomplete_foreign_duplicate_unknown_and_orphan_rows_refuse(self):
        spec = recourse_example()
        text = model(spec)
        missing = base.block("entity_candidate_cash_flow", {"entity": "jv", "period": "third", "candidate": "project", "state": "bad", "value": "0"})
        cases = [text.replace(missing, ""), text + missing,
                 text.replace('objective_entity = "parent"', 'objective_entity = "foreign"'),
                 text.replace('parent_entity = "parent"', 'parent_entity = "jv"'),
                 text.replace('parent_share = "3/5"', 'parent_share = "6/5"'),
                 text.replace('allowed = "true"', 'allowed = "unknown"'),
                 text.replace('cash_period = "2030-05-01/2030-08-31"', 'cash_period = "2030-05-02/2030-08-31"'),
                 text.replace(base.block("entity_funding", {key: spec["entity_funding"][key]
                                                           for key in ("cash_basis", "opening_date", "objective_entity")}), ""),
                 model(spec, include_uu=False)]
        for malformed in cases:
            with self.subTest(tail=malformed[-100:]):
                self.refused(malformed)
        self.refused(text + base.block("implicit_parent_topup", {"value": "40"}), "506")
        self.refused(text.replace('objective_entity = "parent"', 'objective_entity = "parent"\nunknown = "yes"'), "506")
        funding = spec["entity_funding"]
        conflicting = base.block("funding_schedule", {"cash_basis": "another pool", "opening_date": funding["opening_date"]})
        self.refused(text + conflicting)

    def test_transfer_matrix_and_cycle_refusals_are_not_silently_repaired(self):
        spec = calls_example()
        text = model(spec)
        amount = base.block("entity_transfer_amount", {"transfer": "partner_call", "state": "bad", "value": "40"})
        for invalid in [text.replace(amount, ""), text + amount,
                        text.replace('to_entity = "jv"', 'to_entity = "foreign"'),
                        text.replace(amount, amount.replace('value = "40"', 'value = "-1"'))]:
            self.refused(invalid)
        spec = distribution(idle_example([("parent", 20, 20), ("jv", 21, 20)]), "jv", "parent", "1/2")
        distribution(spec, "parent", "jv", "1/2")
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
