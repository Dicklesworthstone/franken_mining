"""Independent Fraction/brute-force oracle for the real production scheduling CLI.

Invented models only. No expected value or feasibility decision calls Rust code.
The model's integer discount recurrence is reproduced from the published contract;
the remaining scheduling arithmetic enumerates definitions directly.
"""

from fractions import Fraction as Q
from itertools import product
import hashlib
import json
from pathlib import Path
import random
import subprocess
import sys
import tempfile

BINARY = Path(sys.argv[1])
REPO = Path(__file__).resolve().parents[3]
ROOT = Path(tempfile.mkdtemp(prefix="fmine-plan-schedule-oracle-"))


def fraction(value):
    return Q(int(value["numerator"]), int(value["denominator"]))


def command(extra=(), *, success=True):
    args = [str(BINARY), "plan", "schedule", "--pcpsp", "model.pcpsp", "--prec", "model.prec",
            "--decimals", "0", "--use-decimals", "0", "--list-schedule", "--workspace", str(ROOT), *extra]
    run = subprocess.run(args, capture_output=True, text=True, check=False)
    report = json.loads(run.stdout)
    assert run.returncode == report["exit_code"], (run.stderr, report)
    if success:
        assert run.returncode == 0, report
    else:
        assert run.returncode != 0 and report["status"] != "ok", report
    assert all(value is False for value in report["flags"].values()), report["flags"]
    return report


def value_and_feasible(values, uses, requires, periods, caps, schedule, factors):
    mined, totals, used, value = {}, {}, {}, Q(0)
    for b, d, t, part in schedule:
        assert 0 <= b < len(values) and 0 <= d < len(values[b]) and 0 <= t < periods and 0 < part <= 1
        if b in mined and mined[b] != t:
            return value, False
        mined[b] = t
        totals[b] = totals.get(b, Q(0)) + part
        value += values[b][d] * part * Q(factors[t], 2**40)
        for r, coefficient in enumerate(uses[b][d]):
            used[r, t] = used.get((r, t), Q(0)) + coefficient * part
    feasible = all(total == 1 for total in totals.values())
    feasible &= all(p in mined and mined[p] <= t for b, t in mined.items() for p in requires[b])
    feasible &= all(amount <= caps[r][t] for (r, t), amount in used.items())
    return value, feasible


def rows(text):
    result = []
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith("%"):
            continue
        b, d, t, part = line.split()
        result.append((int(b), int(d), int(t), Q(part)))
    return result


def verify(report, values, uses, requires, caps, expected=None):
    p = report["payload"]
    assert p["schema"] == "fmine.plan-schedule.v1"
    factors = [int(f) for f in p["discount_factors"]]
    selected = p["selected"]
    value, feasible = value_and_feasible(values, uses, requires, len(factors), caps,
                                        rows(selected["solution"]), factors)
    assert feasible and selected["check"]["feasible"], selected
    assert value == fraction(selected["discounted_value"]), (value, selected)
    bound, gap = fraction(p["bound"]["value"]), fraction(p["bound"]["gap"])
    assert p["bound"]["verified"] and bound >= value and gap == bound - value
    if expected is not None:
        assert value >= expected and bound >= expected, (value, bound, expected)
    for key, file in [("pcpsp", "model.pcpsp"), ("prec", "model.prec")]:
        original = (ROOT / file).read_bytes()
        assert p["model"][key]["sha256"] == hashlib.sha256(original).hexdigest()
        assert p["model"][key]["bytes"] == len(original)
    # Replay the emitted dual prices independently by enumerating every closure schedule
    # and whole destination, with capacity rows moved into the objective. A linear
    # destination relaxation attains its maximum at a whole destination per block.
    prices = [[int(x) for x in row] for row in p["bound"]["prices"]]
    best = None
    choices = [None] + [(t, d) for t in range(len(factors)) for d in range(len(values[0]))]
    for allocation in product(choices, repeat=len(values)):
        if any(slot is not None and any(allocation[q] is None or allocation[q][0] > slot[0]
                                       for q in requires[b]) for b, slot in enumerate(allocation)):
            continue
        relaxed = sum(values[b][d] * factors[t] - sum(prices[r][t] * u for r, u in enumerate(uses[b][d]))
                      for b, slot in enumerate(allocation) if slot is not None for t, d in [slot])
        best = relaxed if best is None else max(best, relaxed)
    replayed = best + sum(prices[r][t] * cap for r, row in enumerate(caps) for t, cap in enumerate(row))
    assert bound == Q(replayed, 2**40), (bound, replayed, prices)
    return p


def known_split():
    for suffix in ("pcpsp", "prec"):
        (ROOT / ("model." + suffix)).write_bytes((REPO / "tests/planning" / ("split." + suffix)).read_bytes())
    supplied = (REPO / "tests/planning/split.sol").read_text()
    (ROOT / "candidate.sol").write_text(supplied)
    values = [[-4, 12], [-2, 8], [-3, 21]]
    uses = [[[1, 0], [1, 2]], [[1, 0], [1, -1]], [[1, 0], [1, 0]]]
    requires, caps = [[], [], [0]], [[2, 2], [0, 0]]
    report = command(["--candidate", "candidate.sol"])
    p = verify(report, values, uses, requires, caps, Q(33))
    assert fraction(p["selected"]["discounted_value"]) == 33
    assert p["selected"]["origin"] == "external_candidate"
    assert p["candidate"]["check"]["feasible"]
    assert p["selected"]["check"]["period_value_numerators"] == ["12000000000", "21000000000"]
    assert command(["--candidate", "candidate.sol"]) == report
    (ROOT / "candidate.sol").write_text(p["selected"]["solution"])
    assert command(["--candidate", "candidate.sol"])["payload"]["selected"] == p["selected"]
    # Preserve a complete block but violate the blend by exactly 2e-9.
    (ROOT / "candidate.sol").write_text(supplied.replace("0 0 0 0.5", "0 0 0 0.499999999").replace("0 1 0 0.5", "0 1 0 0.500000001"))
    bad = verify(command(["--candidate", "candidate.sol"]), values, uses, requires, caps)
    assert not bad["candidate"]["check"]["feasible"]
    assert bad["candidate"]["check"]["incomplete_blocks"] == 0
    assert bad["candidate"]["check"]["over_limit"] == [["1", "0"]]
    assert bad["selected"]["origin"] != "external_candidate"
    for malformed in ["0 0 0 1.000000001\n", "0 0 0 0.5\n0 1 1 0.5\n", "0 0 0 1\n0 0 0 1\n"]:
        (ROOT / "candidate.sol").write_text(malformed)
        command(["--candidate", "candidate.sol"], success=False)
    for options in [["--rounds", "1001"], ["--bound-steps", "0"], ["--share-decimals", "10"], ["--round", "--round"], ["--candidate"]]:
        command(options, success=False)


def joint_routes():
    for suffix in ("pcpsp", "prec"):
        (ROOT / ("model." + suffix)).write_bytes((REPO / "tests/planning" / ("joint_route." + suffix)).read_bytes())
    values, uses, requires, caps = [[6, 10]], [[[0], [1]]], [[]], [[0, 1]]
    unchanged = verify(command(["--rounds", "0"]), values, uses, requires, caps)
    assert fraction(unchanged["selected"]["discounted_value"]) == 5
    assert rows(unchanged["selected"]["solution"]) == [(0, 1, 1, Q(1))]
    report = command()
    p = verify(report, values, uses, requires, caps, Q(6))
    assert fraction(p["selected"]["discounted_value"]) == 6
    assert rows(p["selected"]["solution"]) == [(0, 0, 0, Q(1))]
    assert p["selected"]["origin"] == "heuristic"
    assert command() == report
    (ROOT / "candidate.sol").write_text("0 1 1 1\n")
    supplied = verify(command(["--candidate", "candidate.sol"]), values, uses, requires, caps, Q(6))
    assert supplied["candidate"]["check"]["feasible"]
    assert fraction(supplied["selected"]["discounted_value"]) == 6


def generated_models():
    rng = random.Random(0x51A17)
    for case in range(16):
        values = [[rng.randint(-7, 14), rng.randint(-7, 14)] for _ in range(3)]
        uses = [[[1, rng.randint(-2, 2)] for _ in range(2)] for _ in range(3)]
        requires = [[], [0] if case % 2 else [], [0, 1] if case % 3 == 0 else []]
        caps = [[2, 2], [0, 0]]
        rate = Q(1, 4) if case % 2 else Q(0)
        factors = [2**40, (2**40 * rate.denominator) // (rate.denominator + rate.numerator)]
        text = "NAME: generated\nTYPE: PCPSP\nNBLOCKS: 3\nNPERIODS: 2\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 2\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: " + ("0.25" if rate else "0") + "\nRESOURCE_CONSTRAINT_LIMITS:\n"
        text += "".join(f"{r} {t} L {cap}\n" for r, row in enumerate(caps) for t, cap in enumerate(row))
        text += "OBJECTIVE_FUNCTION:\n" + "".join(f"{b} {v[0]} {v[1]}\n" for b, v in enumerate(values))
        text += "RESOURCE_CONSTRAINT_COEFFICIENTS:\n" + "".join(f"{b} {d} {r} {u}\n" for b in range(3) for d in range(2) for r, u in enumerate(uses[b][d])) + "EOF\n"
        (ROOT / "model.pcpsp").write_text(text)
        (ROOT / "model.prec").write_text("".join(f"{b} {len(req)}" + "".join(f" {q}" for q in req) + "\n" for b, req in enumerate(requires)))
        best, best_rows = Q(0), []
        choices = [None] + [(t, part) for t in range(2) for part in (Q(0), Q(1, 2), Q(1))]
        for allocation in product(choices, repeat=3):
            schedule = [(b, d, t, part if d == 1 else 1-part) for b, slot in enumerate(allocation) if slot is not None for t, part in [slot] for d in range(2) if (part if d == 1 else 1-part) > 0]
            value, feasible = value_and_feasible(values, uses, requires, 2, caps, schedule, factors)
            if feasible and value > best:
                best, best_rows = value, schedule
        (ROOT / "candidate.sol").write_text("".join(f"{b} {d} {t} " + ("0.5" if part == Q(1,2) else "1") + "\n" for b,d,t,part in best_rows))
        p = verify(command(["--candidate", "candidate.sol", "--bound-steps", "16"]), values, uses, requires, caps, best)
        assert [int(x) for x in p["discount_factors"]] == factors
        automatic = verify(command(["--bound-steps", "16"]), values, uses, requires, caps)
        assert fraction(automatic["selected"]["discounted_value"]) >= 0


def automatic_fractional():
    for suffix in ("pcpsp", "prec"):
        (ROOT / ("model." + suffix)).write_bytes((REPO / "tests/planning" / ("fractional." + suffix)).read_bytes())
    values, uses, requires, caps = [[0, 10]], [[[0], [2]]], [[]], [[1]]
    original = (ROOT / "model.pcpsp").read_text()
    report = command()
    p = verify(report, values, uses, requires, caps, Q(5))
    assert p["candidate"] is None
    assert p["selected"]["origin"] in ("fractional_greedy", "fractional_improved")
    assert fraction(p["selected"]["discounted_value"]) == 5
    assert rows(p["selected"]["solution"]) == [(0, 0, 0, Q(1, 2)), (0, 1, 0, Q(1, 2))]
    assert command() == report
    whole = verify(command(["--share-decimals", "0"]), values, uses, requires, caps)
    assert fraction(whole["selected"]["discounted_value"]) == 0
    # Exact integer step must floor 1/3 at each requested decimal scale.
    (ROOT / "model.pcpsp").write_text(original.replace("0 1 0 2", "0 1 0 3"))
    uses = [[[0], [3]]]
    for digits in (1, 2, 9):
        part = Q(10**digits // 3, 10**digits)
        p = verify(command(["--share-decimals", str(digits)]), values, uses, requires, caps)
        assert fraction(p["selected"]["discounted_value"]) == 10 * part
        assert rows(p["selected"]["solution"])[1][3] == part
    # Local whole search drops the negative dump, but its fractional seed remains profitable.
    (ROOT / "model.pcpsp").write_text(original.replace("0 0 10", "0 -2 10"))
    p = verify(command(), [[-2, 10]], [[[0], [2]]], requires, caps, Q(4))
    assert fraction(p["selected"]["discounted_value"]) == 4
    # A same-period precedence cycle has a feasible external whole mining set
    # that Kahn's internal start cannot construct. Its fractional improvement
    # remains available even when whole local-search rounds are explicitly zero.
    cyclic = original.replace("NBLOCKS: 1", "NBLOCKS: 2")
    cyclic = cyclic.replace("0 0 10\n", "0 0 10\n1 0 0\n")
    (ROOT / "model.pcpsp").write_text(cyclic)
    (ROOT / "model.prec").write_text("0 1 1\n1 1 0\n")
    (ROOT / "candidate.sol").write_text("0 0 0 1\n1 0 0 1\n")
    p = verify(command(["--candidate", "candidate.sol", "--rounds", "0"]),
               [[0, 10], [0, 0]], [[[0], [2]], [[0], [0]]], [[1], [0]], caps, Q(5))
    assert fraction(p["selected"]["discounted_value"]) == 5
    assert p["selected"]["origin"] == "fractional_candidate"


def master_mixtures():
    # Independent exact optima: 4 + 8x + 12y under x <= 1/2, y <= 3/4;
    # and 1 + 2x + 4y under x == y, x + y <= 1. Each case needs a mixture
    # of distinct whole assignments rather than one line from the baseline.
    cases = [
        ([[2, 10], [2, 14]],
         [[[0, 0], [2, 0]], [[0, 0], [0, 4]]],
         [[1], [3]], Q(17), Q(4),
         "0 0 0 0.5\n0 1 0 0.5\n1 0 0 0.25\n1 1 0 0.75\n"),
        ([[1, 3, 5]],
         [[[0, 0], [1, -1], [-1, 1]]],
         [[0], [0]], Q(4), Q(1),
         "0 1 0 0.5\n0 2 0 0.5\n"),
    ]
    for values, uses, caps, expected, whole_value, solution in cases:
        n, destinations = len(values), len(values[0])
        requires = [[] for _ in values]
        text = (f"NAME: exact-master-mixture\nTYPE: PCPSP\nNBLOCKS: {n}\n"
                f"NPERIODS: 1\nNDESTINATIONS: {destinations}\n"
                f"NRESOURCE_SIDE_CONSTRAINTS: {len(caps)}\n"
                "NGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0\n"
                "RESOURCE_CONSTRAINT_LIMITS:\n")
        text += "".join(f"{r} 0 L {row[0]}\n" for r, row in enumerate(caps))
        text += "OBJECTIVE_FUNCTION:\n"
        text += "".join(f"{b} " + " ".join(map(str, row)) + "\n" for b, row in enumerate(values))
        text += "RESOURCE_CONSTRAINT_COEFFICIENTS:\n"
        text += "".join(f"{b} {d} {r} {u}\n" for b in range(n)
                        for d in range(destinations) for r, u in enumerate(uses[b][d]))
        (ROOT / "model.pcpsp").write_text(text + "EOF\n")
        (ROOT / "model.prec").write_text("".join(f"{b} 0\n" for b in range(n)))
        options = ["--share-decimals", "3"]
        report = command(options)
        p = verify(report, values, uses, requires, caps, expected)
        assert fraction(p["selected"]["discounted_value"]) == expected
        assert p["selected"]["solution"] == solution
        assert p["selected"]["origin"] == "fractional_greedy"
        assert command(options) == report
        (ROOT / "candidate.sol").write_text(solution)
        admitted = verify(command([*options, "--candidate", "candidate.sol"]),
                          values, uses, requires, caps, expected)
        assert admitted["selected"]["origin"] == "external_candidate"
        assert admitted["selected"]["solution"] == solution
        whole = verify(command(["--share-decimals", "0"]), values, uses, requires, caps)
        assert fraction(whole["selected"]["discounted_value"]) == whole_value
        if destinations == 3:
            # Preserve unit block mass but violate the second signed row by
            # exactly two units of the requested decimal lattice.
            changed = solution.replace("0 1 0 0.5", "0 1 0 0.499")
            changed = changed.replace("0 2 0 0.5", "0 2 0 0.501")
            (ROOT / "candidate.sol").write_text(changed)
            rejected = verify(command([*options, "--candidate", "candidate.sol"]),
                              values, uses, requires, caps, expected)
            assert not rejected["candidate"]["check"]["feasible"]
            assert rejected["selected"]["solution"] == solution


def lp_guidance():
    def write_model(values, uses, requires, caps):
        n, periods = len(values), len(caps[0])
        text = f"NAME: lp-guidance\nTYPE: PCPSP\nNBLOCKS: {n}\nNPERIODS: {periods}\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: {len(caps)}\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0\nRESOURCE_CONSTRAINT_LIMITS:\n"
        text += "".join(f"{r} {t} L {cap}\n" for r, row in enumerate(caps) for t, cap in enumerate(row))
        text += "OBJECTIVE_FUNCTION:\n" + "".join(f"{b} {v[0]} {v[1]}\n" for b, v in enumerate(values))
        text += "RESOURCE_CONSTRAINT_COEFFICIENTS:\n" + "".join(f"{b} {d} {r} {u}\n" for b in range(n) for d in range(2) for r, u in enumerate(uses[b][d])) + "EOF\n"
        (ROOT / "model.pcpsp").write_text(text)
        (ROOT / "model.prec").write_text("".join(f"{b} {len(req)}" + "".join(f" {q}" for q in req) + "\n" for b, req in enumerate(requires)))

    values, uses, requires, caps = [[10, 10], [9, 9], [9, 9]], [[[10], [10]], [[5], [5]], [[5], [5]]], [[], [], []], [[10]]
    write_model(values, uses, requires, caps)
    baseline = command()
    assert "lp_guidance" not in baseline["payload"]
    assert fraction(verify(baseline, values, uses, requires, caps)["selected"]["discounted_value"]) == 10
    guided = command(["--lp-guidance"])
    p = verify(guided, values, uses, requires, caps, Q(18))
    assert fraction(p["selected"]["discounted_value"]) == 18
    assert p["selected"]["origin"] == "lp_guided"
    g = p["lp_guidance"]
    assert g["status"] == "optimal" and 0 < g["iterations"] <= g["iteration_limit"] == 100000
    assert g["columns"] == 9 and g["rows"] == 4 and g["scoped_blocks"] == 3
    assert g["starts"] == 6 and 1 <= g["searched_starts"] <= 6 and 1 <= g["polished_seeds"] <= 12
    assert fraction(g["best_guided_value"]) == 18
    assert g["lp_objective_used_as_certificate"] is False
    assert "lp_objective" not in g and p["bound"]["method"] == "lagrangian_full_model"
    assert command(["--lp-guidance"]) == guided and command() == baseline

    # One PDHG step cannot solve this instance. Its tentative point remains
    # advisory and cannot displace the established feasible lower bound.
    short = verify(command(["--lp-guidance", "--lp-iterations", "1"]), values, uses, requires, caps, Q(10))
    assert short["lp_guidance"]["status"] == "iteration_limit"
    assert short["lp_guidance"]["iterations"] == 1
    markdown = subprocess.run([str(BINARY), "plan", "schedule", "--pcpsp", "model.pcpsp", "--prec", "model.prec", "--lp-guidance", "--format", "markdown", "--workspace", str(ROOT)], capture_output=True, text=True, check=False)
    assert markdown.returncode == 0 and "LP guidance:" in markdown.stdout
    assert "advisory" in markdown.stdout and "Lagrangian replay" in markdown.stdout

    for options in [["--lp-iterations", "1"], ["--lp-guidance", "--lp-guidance"], ["--lp-guidance", "--lp-iterations", "0"], ["--lp-guidance", "--lp-iterations", "1000001"], ["--lp-guidance", "--lp-iterations"], ["--lp-guidance", "--lp-iterations", "2", "--lp-iterations", "3"]]:
        assert command(options, success=False)["status"] == "usage_error"

    # Negative-value material with a signed blending credit must stay in scope.
    values, uses, requires, caps = [[-2, -2], [10, 10]], [[[-1], [-1]], [[1], [1]]], [[], []], [[0]]
    write_model(values, uses, requires, caps)
    p = verify(command(["--lp-guidance"]), values, uses, requires, caps, Q(8))
    assert p["lp_guidance"]["all_blocks_for_signed_resources"] and p["lp_guidance"]["scoped_blocks"] == 2
    assert p["selected"]["check"]["mined_blocks"] == 2

    # A complete feasible external mining set on a precedence cycle beats every
    # Kahn start; its exact half-mill split survives LP guidance unchanged.
    values, uses, requires, caps = [[0, 10], [0, 0]], [[[0], [2]], [[0], [0]]], [[1], [0]], [[1]]
    write_model(values, uses, requires, caps)
    supplied = "0 0 0 0.5\n0 1 0 0.5\n1 0 0 1\n"
    (ROOT / "candidate.sol").write_text(supplied)
    p = verify(command(["--lp-guidance", "--candidate", "candidate.sol"]), values, uses, requires, caps, Q(5))
    assert p["selected"]["origin"] == "external_candidate" and p["selected"]["solution"] == supplied
    assert fraction(p["lp_guidance"]["best_guided_value"]) == 0

    # No profitable closed set is a successful empty result, without invoking PDLP.
    values, uses, requires, caps = [[-2, -1]], [[[0], [0]]], [[]], [[1]]
    write_model(values, uses, requires, caps)
    p = verify(command(["--lp-guidance"]), values, uses, requires, caps, Q(0))
    assert p["lp_guidance"]["status"] == "empty_scope" and p["lp_guidance"]["iterations"] == 0

    # Small source text with an excessive time expansion is rejected before allocation.
    text = "NAME: oversized\nTYPE: PCPSP\nNBLOCKS: 1\nNPERIODS: 65536\nNDESTINATIONS: 16\nNRESOURCE_SIDE_CONSTRAINTS: 0\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0\nRESOURCE_CONSTRAINT_LIMITS:\nOBJECTIVE_FUNCTION:\n0 " + " ".join(["1"] * 16) + "\nRESOURCE_CONSTRAINT_COEFFICIENTS:\nEOF\n"
    (ROOT / "model.pcpsp").write_text(text)
    (ROOT / "model.prec").write_text("0 0\n")
    refused = command(["--lp-guidance"], success=False)
    assert refused["payload"] is None and refused["refusal"]["code"] == "FMINE-REF-207"
    assert "work budget exceeded" in refused["refusal"]["what"]


known_split()
automatic_fractional()
master_mixtures()
joint_routes()
generated_models()
lp_guidance()
print("PASS: exact split and simultaneous-mixture oracles, joint route/period5-to6 improvement, 16 enumerated models, LP-guided10-to18 improvement, advisory iteration-limit shares, signed scope, external dominance and preallocation refusal; global bounds independently replayed")
