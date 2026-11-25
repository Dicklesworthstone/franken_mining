/**
 * The synthetic forecast record behind the journal figure: a site's quarterly gold guidance and its grade model's
 * forecast for every blast polygon (8 a month), each registered and frozen before its outcome. Outcomes are drawn
 * from the forecasts themselves, so the record is calibrated until the grade model drifts after Q6 (when switched
 * on): from then on each polygon comes in one forecast standard deviation below its forecast.
 *
 * Scores are the standard ones (Gaussian CRPS, the 80% central interval score, the PIT). The drift monitor is
 * FrankenMining's betting e-process (fmine-calib, through engine.monitor), fed each polygon's standardised
 * residual. Nothing here touches the DOM, so Bun scripts can import it.
 */
import { monitor } from "./engine";
import { rng } from "./lib";

export const QUARTERS = 12;
export const PER = 24;
export const N = QUARTERS * PER;
export const DRIFT_AT = 6 * PER;
export const BIAS = 1;
/** The polygon draws and the quarter-level draws: chosen so the calibrated record looks like an honest one. */
export const SEED = 38;
export const QSEED = 35;
const RECOVERY = 0.91;
const G_PER_OZ = 31.1035;
/** Quarter-level uncertainty in the guidance (recovery, tonnage, timing), koz. */
const SQ = 2.2;
const Z80 = 1.2815515655446004;

export interface Poly {
  k: number;
  q: number;
  /** Forecast grade and sd, g/t; tonnes, kt. */
  mu: number;
  sd: number;
  kt: number;
  /** The standard normal draw behind its outcome. */
  z: number;
}
export interface Guide {
  q: number;
  mean: number;
  sd: number;
  p10: number;
  p90: number;
  /** The frozen forecast's canonical text and its sha256. */
  text: string;
  hash: string;
}
export interface Outcome {
  value: number;
  /** −1 below P10, 0 inside, 1 above P90. */
  side: -1 | 0 | 1;
  crps: number;
  score: number;
  pit: number;
}
export interface World {
  drift: boolean;
  /** Per polygon: standardised residual, PIT, inside the 80% interval. */
  z: Float64Array;
  pit: Float64Array;
  hit: Uint8Array;
  /** Log wealth of the location (bias) and dispersion (spread) bets after each polygon; the alarm threshold. */
  logL: Float64Array;
  logD: Float64Array;
  logThreshold: number;
  alarmAt: number;
  outcomes: Outcome[];
  ms: number;
}

const normal = (r: () => number) => () => Math.sqrt(-2 * Math.log(1 - r())) * Math.cos(2 * Math.PI * r());

/** Φ, by the complementary error function (Numerical Recipes erfcc, |error| < 1.2e-7). */
export function phi(z: number) {
  const x = Math.abs(z) / Math.SQRT2;
  const t = 1 / (1 + 0.5 * x);
  const e =
    t *
    Math.exp(
      -x * x -
        1.26551223 +
        t * (1.00002368 + t * (0.37409196 + t * (0.09678418 + t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277)))))))),
    );
  return z >= 0 ? 1 - e / 2 : e / 2;
}
const pdf = (z: number) => Math.exp(-0.5 * z * z) / Math.sqrt(2 * Math.PI);
/** CRPS of N(mu, sd²) at y. */
export const crps = (mu: number, sd: number, y: number) => {
  const z = (y - mu) / sd;
  return sd * (z * (2 * phi(z) - 1) + 2 * pdf(z) - 1 / Math.sqrt(Math.PI));
};
/** The central 80% interval score: width plus 10× any miss distance. */
export const intervalScore = (lo: number, hi: number, y: number) =>
  hi - lo + (2 / 0.2) * Math.max(0, lo - y) + (2 / 0.2) * Math.max(0, y - hi);

const ror = (x: number, n: number) => (x >>> n) | (x << (32 - n));
/** SHA-256 of a UTF-8 string, as hex. Synchronous so the frozen cards can show it at once. */
export function sha256(text: string): string {
  const primes: number[] = [];
  for (let n = 2; primes.length < 64; n++) if (primes.every((p) => n % p)) primes.push(n);
  const frac = (x: number) => ((x - Math.floor(x)) * 2 ** 32) >>> 0;
  const K = primes.map((p) => frac(Math.cbrt(p)));
  const H = primes.slice(0, 8).map((p) => frac(Math.sqrt(p)));
  const bytes = [...new TextEncoder().encode(text)];
  const bits = bytes.length * 8;
  bytes.push(0x80);
  while (bytes.length % 64 !== 56) bytes.push(0);
  for (let i = 7; i >= 0; i--) bytes.push(Math.floor(bits / 2 ** (8 * i)) & 255);
  const w = new Array<number>(64).fill(0);
  for (let o = 0; o < bytes.length; o += 64) {
    for (let i = 0; i < 16; i++)
      w[i] = (bytes[o + 4 * i]! << 24) | (bytes[o + 4 * i + 1]! << 16) | (bytes[o + 4 * i + 2]! << 8) | bytes[o + 4 * i + 3]!;
    for (let i = 16; i < 64; i++) {
      const a = w[i - 15]!;
      const b = w[i - 2]!;
      w[i] = (w[i - 16]! + (ror(a, 7) ^ ror(a, 18) ^ (a >>> 3)) + w[i - 7]! + (ror(b, 17) ^ ror(b, 19) ^ (b >>> 10))) | 0;
    }
    const v = H.slice();
    for (let i = 0; i < 64; i++) {
      const [a, b, c, d, e, f, g, h] = v as [number, number, number, number, number, number, number, number];
      const t1 = (h + (ror(e, 6) ^ ror(e, 11) ^ ror(e, 25)) + ((e & f) ^ (~e & g)) + K[i]! + w[i]!) | 0;
      const t2 = ((ror(a, 2) ^ ror(a, 13) ^ ror(a, 22)) + ((a & b) ^ (a & c) ^ (b & c))) | 0;
      v.splice(0, 8, (t1 + t2) | 0, a, b, c, (d + t1) | 0, e, f, g);
    }
    for (let i = 0; i < 8; i++) H[i] = (H[i]! + v[i]!) | 0;
  }
  return H.map((x) => (x >>> 0).toString(16).padStart(8, "0")).join("");
}

/** The forecasts, the same in both worlds: they are frozen before anyone knows whether the model will drift. */
export function plan() {
  const draw = normal(rng(SEED));
  const level = [1.95, 2.06, 1.9, 1.8, 2.12, 2.2, 2.04, 1.92, 1.84, 2.0, 2.16, 2.08];
  const polys: Poly[] = [];
  for (let k = 0; k < N; k++) {
    const q = Math.floor(k / PER);
    polys.push({
      k,
      q,
      mu: level[q]! + 0.16 * Math.sin(k * 0.9) + 0.07 * Math.sin(k * 2.3 + 1),
      sd: 0.2 + 0.06 * Math.abs(Math.sin(k * 0.37)),
      kt: 40 + 2.5 * Math.sin(k * 1.7),
      z: draw(),
    });
  }
  const koz = (kt: number, gpt: number) => (kt * 1000 * gpt * RECOVERY) / G_PER_OZ / 1000;
  const guides: Guide[] = [];
  for (let q = 0; q < QUARTERS; q++) {
    const in_q = polys.filter((p) => p.q === q);
    const mean = in_q.reduce((s, p) => s + koz(p.kt, p.mu), 0);
    const sd = Math.sqrt(in_q.reduce((s, p) => s + koz(p.kt, p.sd) ** 2, 0) + SQ * SQ);
    const text = [
      `id = "synthetic-site-q${q + 1}-gold"`,
      `model_key = "site-guidance/3"`,
      `kind = "gaussian"`,
      `mean = "${mean.toFixed(2)}"`,
      `sd = "${sd.toFixed(2)}"`,
      `unit = "koz"`,
      `outcome_due = "Q${q + 1} close + 30 days"`,
    ].join("\n");
    guides.push({ q, mean, sd, p10: mean - Z80 * sd, p90: mean + Z80 * sd, text, hash: sha256(text) });
  }
  const qdraw = normal(rng(QSEED));
  const eps = guides.map(() => qdraw());
  return { polys, guides, eps, koz };
}
export type Plan = ReturnType<typeof plan>;

/** One history of outcomes, scored, with the engine's e-process run over the polygon stream. */
export async function world(p: Plan, drift: boolean): Promise<World> {
  const z = new Float64Array(N);
  const pit = new Float64Array(N);
  const hit = new Uint8Array(N);
  const logL = new Float64Array(N);
  const logD = new Float64Array(N);
  const t0 = performance.now();
  // The registered model, standardised: each polygon's forecast is N(0, 1) in its own sd units, and the polygons are
  // far apart, so the monitor's predictive at each one is exactly the frozen forecast.
  const mon = await monitor({ nugget: 0.5, sill: 0.5, rangeM: 1, neighbors: 1, alpha: 0.05 });
  let alarmAt = -1;
  let logThreshold = Math.log(40);
  for (const poly of p.polys) {
    const zk = poly.z - (drift && poly.k >= DRIFT_AT ? BIAS : 0);
    z[poly.k] = zk;
    const o = mon.observe(poly.k * 1000, 0, 0, 0, zk);
    pit[poly.k] = o.pit;
    hit[poly.k] = Math.abs(zk) <= Z80 ? 1 : 0;
    logL[poly.k] = o.logELocation;
    logD[poly.k] = o.logEDispersion;
    logThreshold = o.logThreshold;
    if (o.alarm && alarmAt < 0) alarmAt = poly.k;
  }
  mon.free();
  const ms = performance.now() - t0;
  const outcomes: Outcome[] = p.guides.map((g, q) => {
    const value =
      p.polys.filter((x) => x.q === q).reduce((s, x) => s + p.koz(x.kt, x.mu + x.sd * z[x.k]!), 0) + p.eps[q]! * SQ;
    const side: Outcome["side"] = value < g.p10 ? -1 : value > g.p90 ? 1 : 0;
    return {
      value,
      side,
      crps: crps(g.mean, g.sd, value),
      score: intervalScore(g.p10, g.p90, value),
      pit: phi((value - g.mean) / g.sd),
    };
  });
  return { drift, z, pit, hit, logL, logD, logThreshold, alarmAt, outcomes, ms };
}

/** 1 − the worse of the scaled 80% coverage error and the PIT's distance from uniform, as the assessment reports. */
export function calibrationScore(pits: number[], hits: number) {
  const n = pits.length;
  if (!n) return NaN;
  const sorted = [...pits].sort((a, b) => a - b);
  let ks = 0;
  sorted.forEach((u, i) => {
    ks = Math.max(ks, (i + 1) / n - u, u - i / n);
  });
  const error = Math.abs(5 * hits - 4 * n) / n;
  return 1 - Math.min(1, Math.max(error, ks));
}
