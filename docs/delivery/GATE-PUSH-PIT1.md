# GATE-PUSH-PIT1: MineLib UPIT exact, integer-scaled, with verified closure (BD-250)

Verdict: PASS (measured locally at `ef49fcf` on 2026-11-01; self-verified only; MineLib's files are not in the repository, so CI cannot run it)

- **Gate (plan §15.4):** pit limits equal the MineLib UPIT optima exactly (integer-scaled), and every pit's
  closure is verified.
- **Recorded by:** SwiftBasin (agent). `pitflow` landed in `13fafe4`. This record is from a fresh run at
  `ef49fcf`. Since `13fafe4` the module has gained shells (`d8c922f`) and slope arcs (`22f57b5`), both outside
  the UPIT path.

## Measured at `ef49fcf`

```
FMINE_MINELIB_DIR=<local copy> FMINE_MINELIB_INSTANCES=newman1,kd,marvin,mclaughlin,mclaughlin_limit,p4hd,sm2,w23,zuck_large,zuck_medium,zuck_small \
  cargo test --release -p fmine-optimize -- --include-ignored --nocapture
```

This ran on an Apple M5 Ultra (macOS, rustc 1.100.0-nightly 2026-08-30); rch fell back to local. 30 tests
passed and 0 failed, among them `minelib_upit_exact_with_verified_closure`. For each instance, the test asserts:
1. `pitflow`'s certificate verifies the closure.
2. The value equals an independent Dinic maximum flow exactly.
3. The value is within one whole unit of MineLib's published UPIT optimum, widened by the rounding bound below.

| instance | blocks | arcs | value (×10⁻⁶) | rounded values | pit blocks | solve s | Dinic s |
|---|---:|---:|---:|---:|---:|---:|---:|
| newman1 | 1,060 | 3,922 | 26,086,899.025970 | 0 | 1,059 | 0.000 | 0.001 |
| kd | 14,153 | 219,778 | 652,195,036.914887 | 0 | 12,154 | 0.007 | 0.053 |
| marvin | 53,271 | 650,631 | 1,415,655,436.136541 | 4,874 | 8,516 | 0.017 | 0.115 |
| mclaughlin | 2,140,342 | 73,143,770 | 1,495,886,962 | 0 | 110,624 | 1.181 | 7.226 |
| mclaughlin_limit | 112,687 | 3,035,483 | 1,495,726,474 | 0 | 110,225 | 0.204 | 1.923 |
| p4hd | 40,947 | 738,609 | 293,373,256 | 0 | 28,844 | 0.057 | 0.707 |
| sm2 | 99,014 | 96,642 | 2,743,603,730.3 | 0 | 18,388 | 0.005 | 0.054 |
| w23 | 74,260 | 764,786 | 510,973,998 | 0 | 49,061 | 0.069 | 0.872 |
| zuck_large | 96,821 | 1,053,105 | 122,220,279.68 | 34 | 96,821 | 0.068 | 1.764 |
| zuck_medium | 29,277 | 1,271,207 | 1,075,124,490.289275 | 88 | 27,387 | 0.113 | 0.785 |
| zuck_small | 9,400 | 145,640 | 1,422,726,897.734958 | 30 | 9,399 | 0.006 | 0.062 |

The published optima (whole units) are those in `PUBLISHED` in `crates/fmine-optimize/tests/minelib_upit.rs`,
from MineLib's Results page.

## Limits

- **Rounding.** Values are scaled by 10⁶. marvin and the three zuck files carry more decimals than that, so those
  values are rounded half away from zero, and the count is above. Each rounding moves a pit's value by at most
  ½ × 10⁻⁶, so the rounded problem's optimum is within half that count (in 10⁻⁶ units) of the file's. For these
  four instances the gate is therefore exact on the rounded problem, and within that bound of the file's optimum.
- **Published figures are whole units,** with the rounding mode unstated. The one-unit tolerance covers rounding or
  truncation. newman1 is also checked against 26,086,899.03, the optimum Gurobi reports in AMPL's MineLib example.
- **Data.** MineLib's licence is not recorded, so its files are not committed, and the test is `#[ignore]`. CI cannot
  run it. No other agent or machine has reproduced it.
