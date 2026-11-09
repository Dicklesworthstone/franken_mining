"""SYNTHETIC: independent Fraction oracle for actual one-test policy commands."""

from collections import defaultdict
from fractions import Fraction as Q
from itertools import product
from pathlib import Path
import hashlib
import json
import re
import subprocess
import sys
import tempfile
import tomllib

BINARY = Path(sys.argv[1]).resolve()
REPO = Path(__file__).resolve().parents[3]
BASE = (REPO / "tests/decisions/constrained_testwork.toml").read_text()
ROOT = Path(tempfile.mkdtemp(prefix="fmine-policy-oracle-"))
MODEL = ROOT / "policy.toml"


def change(text, key, value):
    changed, count = re.subn(rf"(?m)^{key} = .*?$", f"{key} = {value}", text)
    assert count == 1, (key, count)
    return changed


def command(text, success=True):
    MODEL.write_text(text)
    run = subprocess.run(
        [str(BINARY), "decide", "card", str(MODEL), "--workspace", str(REPO), "--json"],
        capture_output=True, text=True, check=False,
    )
    assert (run.returncode == 0) == success, (run.returncode, run.stdout, run.stderr)
    return json.loads(run.stdout)


def check(text):
    model = tomllib.loads(text)
    weights = {s["id"]: Q(s["weight"]) for s in model["state"]}
    states, actions = sorted(weights), sorted(a["id"] for a in model["action"])
    payoff = {(p["action"], p["state"]): Q(p["value"]) for p in model["payoff"]}
    option = model["information_option"][0]
    likelihood = {(l["state"], l["signal"]): Q(l["probability"]) for l in model["likelihood"]}
    signals = sorted(s["id"] for s in model["signal"]
                     if sum(weights[t] * likelihood[t, s["id"]] for t in states) > 0)
    cost = Q(option["test_cost"]) + Q(option["delay_cost"])
    floor = Q(model["downside_limit"][0]["floor"])
    cap = Q(model["downside_limit"][0]["max_breach_probability"])
    robust = model["policy_optimization"][0]["screen"] == "wasserstein"

    def probability(conditional):
        nominal = sum(weights[s] * conditional[s] for s in states)
        if not robust:
            return nominal
        assert len(states) == 2
        low, high = sorted(states, key=lambda s: conditional[s])
        distance = next(Q(d["value"]) for d in model["transport_distance"]
                        if d["from"] == low and d["to"] == high)
        radius = Q(model["robustness"][0]["radius"])
        moved = min(weights[low], radius / distance)
        return nominal + moved * (conditional[high] - conditional[low])

    feasible = []
    for mapping in product(actions, repeat=len(signals)):
        law, conditional, mean = defaultdict(Q), dict.fromkeys(states, Q(0)), Q(0)
        for state in states:
            for signal, action in zip(signals, mapping):
                net = payoff[action, state] - cost
                p = weights[state] * likelihood[state, signal]
                law[net] += p
                mean += p * net
                if net < floor:
                    conditional[state] += likelihood[state, signal]
        screened = probability(conditional)
        if screened <= cap:
            feasible.append((mean, mapping, screened, law))

    data = command(text)
    assert data["payload"]["source"]["sha256"] == hashlib.sha256(text.encode()).hexdigest()
    search = data["payload"]["card"]["summary"]["downside"]["policy_optimization"]
    report = search["options"][0]
    assert search["policies_evaluated"] == len(actions) ** len(signals)
    assert report["feasible_policies"] == len(feasible)
    if feasible:
        best_mean = max(row[0] for row in feasible)
        winners = [row for row in feasible if row[0] == best_mean]
        _, mapping, screened, law = winners[0]
        actual = report["optimum"]
        assert report["optimal_policy_count"] == len(winners)
        assert Q(actual["nominal_mean"]) == best_mean
        assert Q(actual["screened_breach_probability"]) == screened
        assert [(r["signal_id"], r["action_id"]) for r in actual["signal_actions"]] == list(zip(signals, mapping))
        observed_law = {Q(r["net_payoff"]): Q(r["probability"]) for r in actual["risk"]["distribution"]}
        assert observed_law == {value: p for value, p in law.items() if p}, (observed_law, law)
        assert sum(p * value for value, p in observed_law.items()) == best_mean
    else:
        best_mean = None
        assert report["optimum"] is None and report["optimal_policy_count"] == 0

    act_now = []
    for action in actions:
        conditional = {state: Q(payoff[action, state] < floor) for state in states}
        if probability(conditional) <= cap:
            act_now.append(sum(weights[s] * payoff[action, s] for s in states))
    expected = max(act_now) if act_now else None
    actual = search["best_eligible_act_now"]["best_expected_value"]
    assert (None if actual is None else Q(actual)) == expected
    all_values = act_now + ([] if best_mean is None else [best_mean])
    actual = search["overall"]["best_expected_value"]
    assert (None if actual is None else Q(actual)) == (max(all_values) if all_values else None)
    return data


first = check(BASE)
assert command(BASE) == first
for floor, cap, delay in product(["-10", "0", "50"], ["0/1", "1/100", "1/20", "1/1"], ["0", "2"]):
    text = change(change(change(BASE, "floor", json.dumps(floor)), "max_breach_probability", json.dumps(cap)), "delay_cost", json.dumps(delay))
    check(text)

states = sorted(s["id"] for s in tomllib.loads(BASE)["state"])
geometry = '\n[[robustness]]\nmethod = "wasserstein_1"\nradius = "1/10"\nmetric_basis = "synthetic unit distance"\nradius_basis = "declared test radius"\n'
for left, right in product(states, repeat=2):
    geometry += f'\n[[transport_distance]]\nfrom = "{left}"\nto = "{right}"\nvalue = "{int(left != right)}/1"\n'
for cap in ["0/1", "1/100", "1/20", "1/1"]:
    text = change(change(BASE, "screen", '"wasserstein"'), "max_breach_probability", json.dumps(cap)) + geometry
    check(text)

for key in ["max_policies", "max_work"]:
    refused = command(change(BASE, key, "1"), success=False)
    assert refused["payload"] is None and "no policy optimum" in refused["refusal"]["what"]
print("PASS: 29 exact policy laws, nominal and two-state Wasserstein constraints, ties, infeasible sets, act-now comparisons, deterministic output and work-budget refusals")
