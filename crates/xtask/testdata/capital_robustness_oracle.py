"""Independent exact transport-witness check for the published synthetic capital example.

The two-state quarter-mass grid contains every feasible-polytope vertex for this
fixture. Primal and dual checks additionally prove optimality against the complete
transport polytope, without calling the Rust solver or its certificate checker.
"""
from fractions import Fraction
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[3]
MODEL = ROOT / "tests" / "capital" / "wasserstein.toml"
MILLION = 1_000_000


def rat(value):
    assert isinstance(value, str), ("expected an exact rational string", value)
    return Fraction(value)


def invoke(binary, model=MODEL, workspace=ROOT):
    output = subprocess.run(
        [str(binary), "decide", "program", str(model), "--json",
         "--workspace", str(workspace)],
        capture_output=True, text=True, encoding="utf-8", check=False, timeout=60,
    )
    assert output.returncode == 0, (output.stdout, output.stderr)
    assert len(output.stdout.splitlines()) == 1, output.stdout
    result = json.loads(output.stdout)
    assert result["exit_code"] == 0 and result["refusal"] is None, result
    assert len(result["flags"]) == 4
    assert all(value is False for value in result["flags"].values())
    assert result["certificate"] is None and result["anchor_id"] is None
    return output.stdout, result["payload"]


def certificate(cert, state_ids, weights, distances, radius, objective):
    size = len(state_ids)
    assert rat(cert["radius"]) == radius
    rows, columns = [Fraction(0) for _ in state_ids], [Fraction(0) for _ in state_ids]
    cost, seen = Fraction(0), set()
    for arc in cert["arcs"]:
        i, j, mass = arc["source"], arc["target"], rat(arc["mass"])
        assert type(i) is int and type(j) is int
        assert 0 <= i < size and 0 <= j < size and mass > 0
        assert (i, j) not in seen, "duplicate transport arc"
        seen.add((i, j))
        rows[i] += mass
        columns[j] += mass
        cost += mass * distances[state_ids[i], state_ids[j]]
    assert rows == [weights[state] for state in state_ids]
    assert columns == [rat(value) for value in cert["destination_weights"]]
    assert sum(columns) == 1 and all(value >= 0 for value in columns)
    assert cost == rat(cert["transport_cost"]) and 0 <= cost <= radius
    primal = sum(columns[j] * objective[state] for j, state in enumerate(state_ids))
    assert primal == rat(cert["worst_case_value"])
    multiplier = rat(cert["dual_lambda"])
    potentials = [rat(value) for value in cert["dual_potentials"]]
    assert multiplier >= 0 and len(potentials) == size
    for i, source in enumerate(state_ids):
        for target in state_ids:
            assert potentials[i] + multiplier * distances[source, target] >= objective[target]
    dual = sum(weights[state] * potentials[i] for i, state in enumerate(state_ids))
    dual += multiplier * radius
    assert dual == rat(cert["dual_value"]) == primal
    assert rat(cert["duality_gap"]) == 0


def main(binary):
    raw = MODEL.read_bytes()
    first, payload = invoke(binary)
    second, _ = invoke(binary)
    assert first == second, "same source model changed its rendered receipt"
    assert MODEL.read_bytes() == raw, "read-only comparison changed its source"
    source = payload["source"]
    assert Path(source["path"]).resolve() == MODEL
    assert source["sha256"] == hashlib.sha256(raw).hexdigest()
    assert source["bytes"] == len(raw)
    assert payload["mode"] == "declared_static_capital_comparison"
    report = payload["program"]
    summary = report["summary"]
    stress = summary["robustness"]
    design_claim = report["inputs"]["robustness"]
    result_claim = report["claim"]
    assert design_claim["value"] == stress["design"]
    assert design_claim["id"] in result_claim["parents"]
    for claim in [design_claim, result_claim]:
        assert claim["anchors"], "source-bound Claim lost its anchors"
        assert all(Path(anchor["artifact_path"]).resolve() == MODEL
                   and anchor["sha256"] == source["sha256"]
                   for anchor in claim["anchors"])
        assert {term["scenario_id"] for term in claim["uu_terms"]} == {"UU-omitted-regime"}
    assert result_claim["uu_terms"] == design_claim["uu_terms"]
    assert stress["status"] == "ok" and stress["objective_unit"] == "USD"
    assert stress["scope"] == "nominally_feasible_static_bundles"
    assert stress["regret_reference"] == "all_nominally_feasible_static_bundles"
    assert stress["eligibility_basis"] == "original_nominal_structural_and_tail_loss_screens"
    assert stress["stressed_tail_loss_feasibility_certified"] is False
    assert "not recomputed or certified" in stress["no_claim"]

    choices = {tuple(row["selected"]): row for row in summary["choices"]}
    assert set(choices) == {(), ("risky",), ("safe",), ("risky", "safe")}
    ids = {selection: row["bundle_id"] for selection, row in choices.items()}
    assert stress["structural_rejections"] == [ids["risky", "safe"]]
    assert stress["nominal_risk_rejections"] == []
    assert stress["nominal_expected_value_winners"] == [ids["risky",]]
    assert summary["feasible_winners"] == [ids["risky",]]
    assert stress["maximin_mean_winners"] == [ids["safe",]]
    assert stress["minimax_expected_regret_winners"] == [ids["risky",]]
    assert stress["nominal_winner_survives_maximin"] is False
    assert stress["nominal_winner_survives_minimax_regret"] is True

    state_ids = stress["state_ids"]
    assert state_ids == ["bad", "good"]
    weights = {"good": Fraction(3, 4), "bad": Fraction(1, 4)}
    assert [rat(v) for v in stress["nominal_weights"]] == [weights[s] for s in state_ids]
    radius = Fraction(1, 4)
    assert rat(stress["design"]["radius"]) == radius
    distances = {(row["from_state"], row["to_state"]): rat(row["distance"])
                 for row in stress["design"]["distances"]}
    expected_distances = {(a, b): Fraction(a != b) for a in state_ids for b in state_ids}
    assert distances == expected_distances
    assert len(stress["design"]["distances"]) == 4

    payoffs = {
        ids[()]: {"good": 0, "bad": 0},
        ids["risky",]: {"good": 20 * MILLION, "bad": -12 * MILLION},
        ids["safe",]: {"good": 8 * MILLION, "bad": 8 * MILLION},
    }
    best = {state: max(row[state] for row in payoffs.values()) for state in state_ids}
    distributions = []
    for good_to_bad in range(4):
        for bad_to_good in range(2):
            forward, backward = Fraction(good_to_bad, 4), Fraction(bad_to_good, 4)
            if forward + backward <= radius:
                distributions.append({
                    "good": weights["good"] - forward + backward,
                    "bad": weights["bad"] + forward - backward,
                })
    rows = {row["bundle_id"]: row for row in stress["bundles"]}
    assert len(rows) == len(stress["bundles"]) == 3 and set(rows) == set(payoffs)
    for identifier, values in payoffs.items():
        row = rows[identifier]
        regrets = {state: best[state] - values[state] for state in state_ids}
        dot = lambda law, objective: sum(law[state] * objective[state] for state in state_ids)
        assert rat(row["nominal_mean"]) == dot(weights, values)
        assert rat(row["nominal_mean_regret"]) == dot(weights, regrets)
        assert rat(row["worst_case_mean"]) == min(dot(law, values) for law in distributions)
        assert rat(row["worst_case_expected_regret"]) == max(
            dot(law, regrets) for law in distributions)
        certificate(row["loss_certificate"], state_ids, weights, distances, radius,
                    {state: -values[state] for state in state_ids})
        certificate(row["regret_certificate"], state_ids, weights, distances, radius, regrets)
        assert rat(row["loss_certificate"]["worst_case_value"]) == -rat(row["worst_case_mean"])
        assert rat(row["regret_certificate"]["worst_case_value"]) == rat(
            row["worst_case_expected_regret"])
    print("capital Wasserstein: exact independent vertices and all primal/dual witnesses verified")


def adaptive_reference():
    module = importlib.util.spec_from_file_location(
        "capital_recourse_reference", Path(__file__).with_name("capital_recourse_oracle.py"))
    reference = importlib.util.module_from_spec(module)
    module.loader.exec_module(reference)
    return reference


def adaptive_fixture(reference, spec, radius):
    states = [state for state, _ in spec["states"]]
    rows = [reference.base.block("robustness", {
        "method": "wasserstein_1", "radius": str(radius.numerator) + "/" + str(radius.denominator),
        "metric_basis": "Synthetic unit distance between shared states",
        "radius_basis": "Declared sensitivity, no empirical confidence claim",
    })]
    rows += [reference.base.block("transport_distance", {
        "from": source, "to": target, "value": "0/1" if source == target else "1/1",
    }) for source in states for target in states]
    return reference.fixture(reference.model(spec) + "".join(rows))


def adaptive_main(binary):
    reference = adaptive_reference()
    # The independent existing policy oracle accounts for all cash, shares and information sets.
    specs = [reference.study_example(), reference.study_example(grouped=True),
             reference.study_example(gated=False), reference.equity_example(),
             reference.adaptive_only_example()]
    for spec in specs:
        expected = reference.enumerate_policies(spec)
        weights = expected["weights"]
        assert len(weights) == 2 and set(weights.values()) == {Fraction(1, 2)}
        for radius in (Fraction(0), Fraction(1, 4), Fraction(1, 2), Fraction(1)):
            root = adaptive_fixture(reference, spec, radius)
            model = root / reference.FILE
            raw = model.read_bytes()
            first, payload = invoke(binary, model, root)
            assert first == invoke(binary, model, root)[0]
            assert model.read_bytes() == raw
            assert payload["source"]["sha256"] == hashlib.sha256(raw).hexdigest()
            report = payload["program"]
            reference.CapitalRecourseScenarios().check_policies(report, spec)
            summary = report["summary"]
            stress = summary["robustness"]
            assert stress["scope"] == "nominally_feasible_adaptive_policies"
            assert stress["regret_reference"] == "all_nominally_feasible_adaptive_policies"
            assert stress["objective_unit"] == ("USD/share" if "equity" in spec else "USD")
            assert stress["stressed_tail_loss_feasibility_certified"] is False
            assert report["static_comparison"]["summary"]["robustness"]["scope"] == "nominally_feasible_static_bundles"
            design = report["static_comparison"]["inputs"]["robustness"]
            assert design["id"] in report["claim"]["parents"]
            assert all(anchor in report["claim"]["anchors"] for anchor in design["anchors"])
            ids = {reference.policy_key(row["initial_selected"], row["branches"]): row["policy_id"]
                   for row in summary["policies"]}
            payoffs, structural, risk = {}, [], []
            for key, row in expected["policies"].items():
                identifier = ids[key]
                if row["rejections"]:
                    structural.append(identifier)
                elif row["risk_rejected"]:
                    risk.append(identifier)
                else:
                    payoffs[identifier] = (row["per_share"] if "equity" in spec else row)["values"]
            assert sorted(stress["structural_rejections"]) == sorted(structural)
            assert sorted(stress["nominal_risk_rejections"]) == sorted(risk)
            assert stress["nominal_expected_value_winners"] == summary["feasible_winners"]
            states = stress["state_ids"]
            assert states == sorted(weights)
            best = {state: max(row[state] for row in payoffs.values()) for state in states} if payoffs else {}
            distributions = [{states[0]: Fraction(2 - forward + backward, 4),
                              states[1]: Fraction(2 + forward - backward, 4)}
                             for forward in range(3) for backward in range(3)
                             if Fraction(forward + backward, 4) <= radius]
            distances = {(a, b): Fraction(a != b) for a in states for b in states}
            rows = {row["policy_id"]: row for row in stress["policies"]}
            assert set(rows) == set(payoffs) and len(rows) == len(stress["policies"])
            for identifier, values in payoffs.items():
                row = rows[identifier]
                regrets = {state: best[state] - values[state] for state in states}
                dot = lambda law, objective: sum(law[state] * objective[state] for state in states)
                assert rat(row["nominal_mean"]) == dot(weights, values)
                assert rat(row["nominal_mean_regret"]) == dot(weights, regrets)
                assert rat(row["worst_case_mean"]) == min(dot(law, values) for law in distributions)
                assert rat(row["worst_case_expected_regret"]) == max(dot(law, regrets) for law in distributions)
                certificate(row["loss_certificate"], states, weights, distances, radius,
                            {state: -values[state] for state in states})
                certificate(row["regret_certificate"], states, weights, distances, radius, regrets)
            for field, value, choose in (("maximin_mean_winners", "worst_case_mean", max),
                                        ("minimax_expected_regret_winners", "worst_case_expected_regret", min)):
                optimum = choose(rat(row[value]) for row in rows.values()) if rows else None
                assert sorted(stress[field]) == sorted(identifier for identifier, row in rows.items()
                                                      if rat(row[value]) == optimum)
            if radius == 0:
                assert stress["maximin_mean_winners"] == summary["feasible_winners"]
    print("adaptive capital Wasserstein: 20 independently enumerated policy/transport scenarios verified")


if __name__ == "__main__":
    assert len(sys.argv) in (2, 3), "usage: capital_robustness_oracle.py /path/to/fmine [--adaptive|--adaptive-fixture]"
    binary = Path(sys.argv[1]).resolve()
    if len(sys.argv) == 2:
        main(binary)
    elif sys.argv[2] == "--adaptive":
        adaptive_main(binary)
    else:
        assert sys.argv[2] == "--adaptive-fixture"
        reference = adaptive_reference()
        print(adaptive_fixture(reference, reference.study_example(), Fraction(1, 4)))
