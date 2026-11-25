/**
 * The three numbers in the tailings figure that FrankenMining's own kernels compute, live, in the browser (web/wasm):
 *
 * - `staleEstimate`: fmine-calib's conditional Gaussian predictor puts a number and an interval on the stale tip
 *   VWP-04 from the five tips that read today, conditioned on their departures from the design seepage heads.
 *   An estimate is not a reading: FMINE-REF-404 still refuses the "within TARP" statement.
 * - `readingValue`: fmine-numerics' expected value of sample information for reading VWP-04 by hand before the
 *   rain, against acting on the estimate alone (illustrative payoffs).
 * - `freeboardTail`: fmine-numerics' CVaR of the pond's peak rise over an ensemble of storms, the tail of the
 *   freeboard rather than its middle.
 *
 * The water balance itself is TypeScript (`tailings-balance.ts`): the engine has no TSF water-balance kernel yet.
 */
import { cvar, estimate, voi, type Sample } from "./engine";
import { crestZ, headKPa, PIEZOMETERS, SECTION_X } from "./tailings-model";

/** Exponential covariance of the departures from design head, m²: a 40 m range, as the tips' spread suggests. */
const MODEL = { nugget: 0.04, sill: 0.6, rangeM: 40, neighbors: 5 };

/** Standard normal CDF (Abramowitz and Stegun 7.1.26). */
export function phi(z: number) {
  const t = 1 / (1 + 0.3275911 * (Math.abs(z) / Math.SQRT2));
  const y = 1 - ((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * Math.exp(-(z * z) / 2);
  return z >= 0 ? (1 + y) / 2 : (1 - y) / 2;
}

export interface StaleEstimate {
  /** Head, and pressure with its 90% interval, kPa. */
  head: number;
  sdHead: number;
  kpa: number;
  lo: number;
  hi: number;
  /** Probability the pressure stands above trigger level 1. */
  pAboveL1: number;
  l1: number;
  ms: number;
}

export async function staleEstimate(): Promise<StaleEstimate> {
  const stale = PIEZOMETERS.find((p) => p.stale)!;
  const at = (s: number, y: number) => ({ x: SECTION_X, y: crestZ(SECTION_X) + s, z: y });
  const samples: Sample[] = PIEZOMETERS.filter((p) => !p.stale).map((p) => ({ ...at(p.s, p.y), v: p.head - p.design }));
  const q = at(stale.s, stale.y);
  const r = await estimate({ ...MODEL, samples, prior: 0, queries: [q.x, q.y, q.z] });
  const mean = r.mean[0]!;
  const sd = r.sd[0]!;
  if (!Number.isFinite(mean) || !Number.isFinite(sd)) throw new Error("the predictor refused the query");
  const head = stale.design + mean;
  const l1 = headKPa(stale, stale.l1);
  return {
    head,
    sdHead: sd,
    kpa: headKPa(stale, head),
    lo: headKPa(stale, head - 1.645 * sd),
    hi: headKPa(stale, head + 1.645 * sd),
    pAboveL1: 1 - phi((stale.l1 - head) / Math.max(1e-6, sd)),
    l1,
    ms: r.ms,
  };
}

export interface ReadingValue {
  /** 0: carry on as planned; 1: act on the estimate now (lower the pond, stop deposition near BH-B). */
  best: number;
  evpi: number;
  evsi: number;
  net: number;
  cost: number;
  ms: number;
}

/**
 * The value of a hand reading of VWP-04 before the storm, in thousands of dollars (illustrative payoffs). States:
 * the pressure is below L1, or above it. Actions: carry on, or act now. Carrying on while it is above L1 costs more
 * the bigger the storm. The vibrating-wire readout is good to about ±0.5 kPa, so the reading all but settles it.
 */
export async function readingValue(pAbove: number, stormMm: number): Promise<ReadingValue> {
  const t = performance.now();
  const p = Math.min(0.999, Math.max(0.001, pAbove));
  const cost = 3;
  const r = await voi({
    prior: [1 - p, p],
    payoffs: [
      [0, -90],
      [-(250 + 2.5 * stormMm), -150],
    ],
    likelihood: [
      [0.98, 0.02],
      [0.03, 0.97],
    ],
    cost,
  });
  return { best: r.bestAction, evpi: r.evpi, evsi: r.evsi ?? 0, net: r.netVoi ?? 0, cost, ms: performance.now() - t };
}

export interface Tail {
  alpha: number;
  mean: number;
  var: number;
  cvar: number;
  ms: number;
}

/** CVaR of the pond's peak rise (m) over the ensemble, at 95%. */
export async function freeboardTail(rises: ArrayLike<number>, alpha = 0.95): Promise<Tail> {
  const t = performance.now();
  const r = await cvar(rises, alpha);
  return { alpha: r.alpha, mean: r.meanLoss, var: r.var, cvar: r.cvar, ms: performance.now() - t };
}
