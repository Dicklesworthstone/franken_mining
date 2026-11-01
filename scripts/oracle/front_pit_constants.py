#!/usr/bin/env python3
"""Constants of fmine-calib's spatial-front PIT monitor (bd-311, plan section 10.4, GB-08).

Under the null the standardized residual z is N(0, 1). The monitor bets on two scores:
  location   psi_loc(z)  = tanh(z / 2)                 (mean 0 by symmetry)
  dispersion psi_disp(z) = tanh((z^2 - 1) / 2) - c0    (mean 0 once centred)

This prints
  c0      = E[tanh((Z^2 - 1) / 2)]
  q_loc   = E[tanh(Z / 2)^2]
  q_disp  = E[(tanh((Z^2 - 1) / 2) - c0)^2]
each integrated against the standard normal density by Simpson's rule on [-12, 12] (the density beyond is below
1e-31) with CPython's math.tanh and math.exp, at n and n / 2 intervals so the convergence shows. It also prints the
two 58%-coverage alternatives of the BD-311 acceptance, from statistics.NormalDist:
  delta  : the location shift with P(|Z + delta| <= z_0.9) = 0.58
  spread : the scale s with P(|s Z| <= z_0.9) = 0.58, that is z_0.9 / z_0.79

Standard library only. Same author as the Rust test that pins these values; the Rust side integrates with its own
Simpson on fmine_core::det, so the two share the method but not the libm.
"""

import math
import statistics

LIMIT = 12.0
N = 1_200_000


def simpson(f, a, b, n):
    """Composite Simpson; math.fsum keeps the summation exact, so only the rule's own error remains."""
    h = (b - a) / n
    terms = [f(a), f(b)]
    terms.extend((4.0 if i % 2 else 2.0) * f(a + i * h) for i in range(1, n))
    return math.fsum(terms) * h / 3.0


def density(z):
    return math.exp(-0.5 * z * z) / math.sqrt(2.0 * math.pi)


def moments(n):
    c0 = simpson(lambda z: math.tanh((z * z - 1.0) / 2.0) * density(z), -LIMIT, LIMIT, n)
    q_loc = simpson(lambda z: math.tanh(z / 2.0) ** 2 * density(z), -LIMIT, LIMIT, n)
    q_disp = simpson(
        lambda z: (math.tanh((z * z - 1.0) / 2.0) - c0) ** 2 * density(z), -LIMIT, LIMIT, n
    )
    return c0, q_loc, q_disp


def main():
    fine = moments(N)
    coarse = moments(N // 2)
    for name, value, other in zip(("c0", "q_loc", "q_disp"), fine, coarse):
        print(f"{name} {value!r} (n/2 differs by {value - other:.1e})")

    unit = statistics.NormalDist()
    q = unit.inv_cdf(0.9)
    lo, hi = 0.0, 3.0
    for _ in range(200):
        mid = 0.5 * (lo + hi)
        if unit.cdf(q - mid) - unit.cdf(-q - mid) > 0.58:
            lo = mid
        else:
            hi = mid
    print(f"delta {0.5 * (lo + hi)!r}")
    print(f"spread {q / unit.inv_cdf(0.79)!r}")


if __name__ == "__main__":
    main()
