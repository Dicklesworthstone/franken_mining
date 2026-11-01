"""Known answers for gaussian_crps and gaussian_log_score (bd-048), independent of fmine-calib's closed form.

CRPS is integrated from its definition, CRPS(F, y) = int (F(x) - 1{x >= y})^2 dx, with composite Simpson on
[mu - 14 sigma, y] and [y, mu + 14 sigma] (beyond 14 sigma the integrand is below 1e-44). F is the normal CDF from
math.erfc. The log score is -ln of the normal density, from math.log and math.pi. Both use CPython's libm, not
fmine-numerics. A closed-form value is printed beside each, as a cross-check of the integration only.
"""

import math


def cdf(x, mu, s):
    return 0.5 * math.erfc(-(x - mu) / (s * math.sqrt(2.0)))


def simpson(f, a, b, n=200_000):
    if a == b:
        return 0.0
    h = (b - a) / n
    total = f(a) + f(b)
    for i in range(1, n):
        total += (4 if i % 2 else 2) * f(a + i * h)
    return total * h / 3.0


def crps_integral(mu, s, y):
    lo, hi = mu - 14 * s, mu + 14 * s
    left = simpson(lambda x: cdf(x, mu, s) ** 2, min(lo, y), y)
    right = simpson(lambda x: (cdf(x, mu, s) - 1.0) ** 2, y, max(hi, y))
    return left + right


def crps_closed(mu, s, y):
    z = (y - mu) / s
    return s * (z * (2 * cdf(z, 0, 1) - 1) + 2 * math.exp(-z * z / 2) / math.sqrt(2 * math.pi) - 1 / math.sqrt(math.pi))


def log_score(mu, s, y):
    z = (y - mu) / s
    return 0.5 * math.log(2 * math.pi) + math.log(s) + 0.5 * z * z


CASES = [(0.0, 1.0, 0.0), (0.0, 1.0, 1.0), (0.0, 1.0, -2.5), (2.0, 0.5, 2.0), (4.0, 0.5, 2.0), (100.0, 25.0, 160.0),
         (-3.0, 0.01, -2.97), (1.0, 3.0, 9.0)]

for mu, s, y in CASES:
    a, c = crps_integral(mu, s, y), crps_closed(mu, s, y)
    print(f"({mu!r}, {s!r}, {y!r}) crps_integral={a!r} crps_closed={c!r} rel={abs(a - c) / c:.1e} log={log_score(mu, s, y)!r}")
