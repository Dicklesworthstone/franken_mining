"""Known answers for the chi-square upper tail in fmine-numerics' GATE-HELM-02 test (bd-227), independent of its
closed form.

Each value is the upper tail P(X >= t) of a chi-square variable with k degrees of freedom, integrated from its
density, x^(k/2 - 1) e^(-x/2) / (2^(k/2) Gamma(k/2)), with composite Simpson on [t, t + 60 + 40 sqrt(2k)] (beyond
it the integrand is below 1e-26). The density uses CPython's math.lgamma, math.exp and math.log, not fmine-core's
det functions. The Rust code sums the closed form instead (a Poisson sum for even k, erfc plus a sum for odd k).
"""

import math
import sys

POINTS = [
    (3.841458820694124, 1),
    (0.5, 1),
    (5.991464547107979, 2),
    (7.814727903251178, 3),
    (0.5, 3),
    (2.0, 4),
    (40.0, 7),
    (23.209251158954356, 10),
    (3.0738, 12),
    (12.0, 12),
    (28.2995, 12),
    (60.0, 25),
]


def density(x, k):
    if x <= 0.0:
        return 0.0
    return math.exp((k / 2 - 1) * math.log(x) - x / 2 - (k / 2) * math.log(2) - math.lgamma(k / 2))


def simpson(f, a, b, n=400_000):
    h = (b - a) / n
    total = f(a) + f(b)
    for i in range(1, n):
        total += (4 if i % 2 else 2) * f(a + i * h)
    return total * h / 3.0


def upper_tail(t, k):
    return simpson(lambda x: density(x, k), t, t + 60.0 + 40.0 * math.sqrt(2 * k))


def main():
    scipy_sf = None
    if "--scipy" in sys.argv:
        from scipy.stats import chi2

        scipy_sf = chi2.sf
    for t, k in POINTS:
        line = f"t={t!r} k={k} sf={upper_tail(t, k)!r}"
        if scipy_sf is not None:
            line += f" scipy={float(scipy_sf(t, k))!r}"
        print(line)


if __name__ == "__main__":
    main()
