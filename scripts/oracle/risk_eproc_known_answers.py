"""Known answers for fmine-numerics' CVaR and betting e-processes (bd-022), in exact rational arithmetic.

CVaR. For n equally likely losses, VaR_alpha is the least loss l with #{L <= l} / n >= alpha, and CVaR_alpha is
Rockafellar and Uryasev's minimum over c of c + E[(L - c)+] / (1 - alpha), taken over the losses themselves (the
function is convex and piecewise linear with its corners there). The Rust code integrates the quantile function
above alpha instead. Every alpha is the exact value of its double.

E-processes. The wealth of `BettingEProcess` and the log wealth of `ScoreBettingEProcess` after each observation,
from the betting rules their doc comments state, with every stake and factor an exact fraction. Only the
logarithm of each factor is a float, from CPython's math.log, not fmine-core's det::ln. These are known answers for
the stated rule; they say nothing about the rule's validity, which the Ville tests check.

Usage: python3 -I scripts/oracle/risk_eproc_known_answers.py
"""

import math
from fractions import Fraction as F

CVAR_CASES = [
    ([3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0], 0.75),
    ([3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0], 0.8),
    ([-2.5, 0.0, 7.25, 1.5, -1.0], 0.9),
    ([10.0], 0.5),
    ([float(i) for i in range(1, 11)], 0.95),
    ([5.0, 5.0, 5.0, 1.0], 0.5),
]

# The prior variance `BettingEProcess::new` starts from, as the double the code holds.
PRIOR_VAR = F(0.08333333333333333)

BETTING_CASES = [
    (0.5, [1.0, 1.0, 0.75, 0.25, 1.0, 0.0, 0.875]),
    (0.125, [1.0, 1.0, 1.0, 1.0, 1.0]),
]

# (lo, hi, prior second moment, truncation, scores)
SCORE_CASES = [
    (-1.0, 2.0, 1.0, 0.5, [1.5, -0.5, 2.0, 0.25, -1.0, 1.0]),
    (-1.0, 2.0, 1.0, 0.2, [2.0, 2.0, 2.0, -1.0]),
]


def cvar(losses, alpha):
    a = F(alpha)
    xs = [F(x) for x in losses]
    n = len(xs)
    var = min(l for l in xs if F(sum(1 for x in xs if x <= l), n) >= a)
    best = min(c + sum(max(x - c, 0) for x in xs) / n / (1 - a) for c in xs)
    return F(sum(xs), n), var, best


def clamp(x, lo, hi):
    return max(lo, min(hi, x))


def betting(null_mean, us):
    mu = F(null_mean)
    s, v, wealth, peak, out = F(0), PRIOR_VAR, F(1), F(1), []
    for u in us:
        d = F(u) - mu
        lam = clamp(s / (v + 1), F(-3, 4), F(3, 4))
        wealth *= 1 + lam * d
        peak = max(peak, wealth)
        s += d
        v += d * d
        out.append(float(wealth))
    return out, float(peak)


def score(lo, hi, q0, c, scores):
    lo, hi, q0, c = F(lo), F(hi), F(q0), F(c)
    s, ss, log_wealth, out = F(0), F(0), 0.0, []
    for x in scores:
        x = F(x)
        lam = clamp(s / (q0 + ss), -c / hi, c / -lo)
        log_wealth += math.log(1 + lam * x)
        s += x
        ss += x * x
        out.append(log_wealth)
    return out


def main():
    for losses, alpha in CVAR_CASES:
        mean, var, cv = cvar(losses, alpha)
        print(f"cvar {losses} {alpha!r}: mean {float(mean)!r} var {float(var)!r} cvar {float(cv)!r} ({cv})")
    for null_mean, us in BETTING_CASES:
        out, peak = betting(null_mean, us)
        print(f"betting {null_mean!r} {us}: wealth {out} max {peak!r}")
    for lo, hi, q0, c, scores in SCORE_CASES:
        print(f"score {lo!r} {hi!r} {q0!r} {c!r} {scores}: log_wealth {score(lo, hi, q0, c, scores)}")


if __name__ == "__main__":
    main()
