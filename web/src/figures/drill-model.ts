/**
 * The exploration model behind the drill scene, free of any drawing so that scripts can run it: the prospect's
 * block grid, the drillholes and their assays, the contained-gold interval from the engine's per-block predictions,
 * the rule that ranks the next hole, and the decision the value-of-information kernel prices. Synthetic throughout.
 */
import { heading, makeBody, OZ, phi, type Body } from "./drill-body";
import type { Sample, VoiInput } from "./engine";
import { rng } from "./lib";
import * as THREE from "three";

/* ---------- The prospect ---------- */
/** Block edge, metres, and the grid: 24 east by 20 north by 16 down. */
export const B = 20;
export const NX = 24;
export const NZ = 20;
export const NY = 16;
export const NB = NX * NZ * NY;
export const X0 = -NX * 10;
export const Z0 = -NZ * 10;
export const TOP = 0;
export const BOTTOM = TOP - NY * B;
/** Dry bulk density, t/m³, and so tonnes in a block. */
export const DENSITY = 2.7;
export const BLOCK_T = B * B * B * DENSITY;
/** The reporting cut-off, g/t. */
export const CUTOFF = 0.5;
/** The registered covariance, in ln(grade) units² (illustrative). */
export const VARIOGRAM = { nugget: 0.06, sill: 1.2, rangeM: 90, neighbors: 12 } as const;
export const SD_MAX = Math.sqrt(VARIOGRAM.nugget + VARIOGRAM.sill);
/** Assay composites, metres. */
export const COMPOSITE = 10;
/** Diamond drilling, all-in, $ per metre (illustrative). */
export const COST_PER_M = 310;
/** The decision the next hole informs (assumed): whether the ground it tests joins the resource drill-out, valued at
 * $60 an ounce in the ground, drilled out on a 25 m grid. */
export const ECON = { perOz: 60, spacing: 25 } as const;
/** Drilling the ground out on the resource grid: metres ≈ volume / spacing², at the same rate per metre. */
export const drillOutCost = (blocks: number) => ((blocks * B * B * B) / (ECON.spacing * ECON.spacing)) * COST_PER_M;
/** Mine-grid offsets, so collars read like survey coordinates. */
export const GRID_E = 5000;
export const GRID_N = 10000;
/** Holes on the prospect are drilled across the lens, towards this bearing. */
export const BEARING = 340;
export const DIP = 60;

export const blockIndex = (i: number, j: number, k: number) => i + NX * (j + NZ * k);
export function blockCentre(b: number, out: [number, number, number] = [0, 0, 0]) {
  const i = b % NX;
  const j = Math.floor(b / NX) % NZ;
  const k = Math.floor(b / (NX * NZ));
  out[0] = X0 + (i + 0.5) * B;
  out[1] = TOP - (k + 0.5) * B;
  out[2] = Z0 + (j + 0.5) * B;
  return out;
}

export interface Prospect {
  body: Body;
  /** Block centres as x, y, z triples. */
  centres: Float64Array;
  /** Prior mean and the truth, ln(grade), per block. */
  prior: Float64Array;
  truth: Float64Array;
  /** Gold above the cut-off in the truth, troy ounces. */
  truthOz: number;
}

export function makeProspect(seed = 11): Prospect {
  const body = makeBody({ seed, centre: [10, -150, -5], trend: 70, plunge: 32, dip: 62, size: [195, 82, 52], peak: 4.5, background: 0.035, priorLift: 1.8 });
  const centres = new Float64Array(NB * 3);
  const prior = new Float64Array(NB);
  const truth = new Float64Array(NB);
  const c: [number, number, number] = [0, 0, 0];
  let grams = 0;
  for (let b = 0; b < NB; b++) {
    blockCentre(b, c);
    centres.set(c, b * 3);
    prior[b] = body.prior(c[0], c[1], c[2]);
    // A block's true grade: the mean of four points inside it, so the truth has block support.
    let g = 0;
    for (const [dx, dy, dz] of [[-5, -5, -5], [5, 5, -5], [5, -5, 5], [-5, 5, 5]] as const) g += body.grade(c[0] + dx, c[1] + dy, c[2] + dz);
    g /= 4;
    truth[b] = Math.log(g);
    if (g >= CUTOFF) grams += g * BLOCK_T;
  }
  return { body, centres, prior, truth, truthOz: grams / OZ };
}

/* ---------- Holes ---------- */
export interface Composite {
  from: number;
  to: number;
  /** Assayed grade, g/t. */
  g: number;
  x: number;
  y: number;
  z: number;
}
export interface Hole {
  id: string;
  collar: THREE.Vector3;
  bearing: number;
  dip: number;
  length: number;
  dir: THREE.Vector3;
  composites: Composite[];
  /** Who sited it: the starting programme, the agent's rule, or a person's click. */
  by: "plan" | "agent" | "person";
}

export const holeId = (n: number) => `DH-${String(n).padStart(2, "0")}`;

/** A hole from a collar, down to the model's floor (or `length`). */
export function designHole(n: number, collar: THREE.Vector3, bearing: number, dip: number, by: Hole["by"], length?: number): Hole {
  const dir = heading(bearing, dip);
  const depth = collar.y - (BOTTOM + 8);
  const len = length ?? Math.round(depth / Math.sin(THREE.MathUtils.degToRad(dip)) / 10) * 10;
  return { id: holeId(n), collar: collar.clone(), bearing, dip, length: len, dir, composites: [], by };
}

/** Points down a hole at composite centres, as x, y, z triples (no assays). */
export function compositeCentres(h: Hole) {
  const out: [number, number, number][] = [];
  for (let d = COMPOSITE / 2; d < h.length; d += COMPOSITE) {
    out.push([h.collar.x + h.dir.x * d, h.collar.y + h.dir.y * d, h.collar.z + h.dir.z * d]);
  }
  return out;
}

/**
 * Assays down a hole: the truth averaged over each 10 m composite, with a laboratory and sampling error of 12%
 * (log-normal), seeded by the hole's number so that the same hole always returns the same assays.
 */
export function assay(h: Hole, body: Body, seed: number) {
  const r = rng(seed * 9973 + 17);
  const noise = () => {
    const u = Math.max(1e-9, r());
    return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * r());
  };
  h.composites = [];
  for (let d = 0; d + COMPOSITE <= h.length + 1e-9; d += COMPOSITE) {
    let g = 0;
    for (let s = 0; s < 4; s++) {
      const t = d + (s + 0.5) * (COMPOSITE / 4);
      g += body.grade(h.collar.x + h.dir.x * t, h.collar.y + h.dir.y * t, h.collar.z + h.dir.z * t);
    }
    g = (g / 4) * Math.exp(0.12 * noise() - 0.0072);
    const m = d + COMPOSITE / 2;
    h.composites.push({ from: d, to: d + COMPOSITE, g, x: h.collar.x + h.dir.x * m, y: h.collar.y + h.dir.y * m, z: h.collar.z + h.dir.z * m });
  }
  return h;
}

/** The engine's samples from every hole: ln(grade) with the prior mean at each composite. */
export function samplesOf(holes: Hole[], prior: (x: number, y: number, z: number) => number): Sample[] {
  const out: Sample[] = [];
  for (const h of holes) for (const c of h.composites) out.push({ x: c.x, y: c.y, z: c.z, v: Math.log(c.g), prior: prior(c.x, c.y, c.z) });
  return out;
}

/* ---------- Contained gold ---------- */
const Z90 = 1.2815515655446004;
export interface Interval {
  /** Expected ounces, the spread, and the P10, P50 and P90 of a log-normal with that mean and spread. */
  mean: number;
  sd: number;
  p10: number;
  p50: number;
  p90: number;
}

/* Panels of 3 × 3 × 3 blocks for the covariance of the total. */
const PX = Math.ceil(NX / 3);
const PZ = Math.ceil(NZ / 3);
const PY = Math.ceil(NY / 3);
const NP = PX * PZ * PY;
const panelOf = new Int32Array(NB);
const panelRho = new Float64Array(NP * NP);
{
  for (let b = 0; b < NB; b++) {
    const i = b % NX;
    const j = Math.floor(b / NX) % NZ;
    const k = Math.floor(b / (NX * NZ));
    panelOf[b] = Math.floor(i / 3) + PX * (Math.floor(j / 3) + PZ * Math.floor(k / 3));
  }
  const pc = (p: number) => [(p % PX) * 3 * B, Math.floor(p / PX) % PZ * 3 * B, Math.floor(p / (PX * PZ)) * 3 * B] as const;
  for (let p = 0; p < NP; p++) {
    const a = pc(p);
    for (let q = 0; q < NP; q++) {
      const c = pc(q);
      panelRho[p * NP + q] = Math.exp(-Math.hypot(a[0] - c[0], a[1] - c[1], a[2] - c[2]) / VARIOGRAM.rangeM);
    }
  }
}

/**
 * Block support. The predictor answers for a point at the block's centre; a block averages away the nugget and part
 * of the short-range variation, γ̄(v, v) ≈ sill · (1 − exp(−13.2 m / range)) for a 20 m cube (13.2 m is the mean
 * distance between two points in it). The block's ln-grade keeps the point's expected grade with that smaller sd.
 */
export const SUPPORT = VARIOGRAM.nugget + VARIOGRAM.sill * (1 - Math.exp(-(0.6617 * B) / VARIOGRAM.rangeM));
export function blockLn(mu: number, s: number): [mean: number, sd: number] {
  const v = Math.max(1e-4, s * s - SUPPORT);
  return [mu + (s * s - v) / 2, Math.sqrt(v)];
}

/** One block's expected gold above the cut-off and its variance, grams, from the point prediction N(mu, s²). */
export function blockGold(mu0: number, s0: number): [mean: number, variance: number] {
  const [mu, s] = blockLn(mu0, s0);
  const lc = Math.log(CUTOFF);
  const s2 = s * s;
  const m1 = BLOCK_T * Math.exp(mu + s2 / 2) * phi((mu + s2 - lc) / s);
  const m2 = BLOCK_T * BLOCK_T * Math.exp(2 * mu + 2 * s2) * phi((mu + 2 * s2 - lc) / s);
  return [m1, Math.max(0, m2 - m1 * m1)];
}

const panelMean = new Float64Array(NP);
const panelSd = new Float64Array(NP);
/**
 * Gold above the cut-off, troy ounces, from each block's predictive mean and sd of ln(grade): each block's expected
 * gold above the cut-off under a log-normal grade, summed; and the spread of the total with the blocks' errors
 * taken as correlated the way the covariance model says, fully within a 60 m panel and by exp(−d / range) between
 * panels (cautious, since data make the real errors less correlated). The P10, P50 and P90 are a log-normal's with
 * that mean and spread.
 */
export function containedOz(mean: ArrayLike<number>, sd: ArrayLike<number>, only?: ArrayLike<number>): Interval {
  panelMean.fill(0);
  panelSd.fill(0);
  const n = only ? only.length : mean.length;
  for (let o = 0; o < n; o++) {
    const b = only ? only[o]! : o;
    const mu = mean[b]!;
    const s = sd[b]!;
    if (!Number.isFinite(mu) || !Number.isFinite(s)) continue;
    const [m, v] = blockGold(mu, Math.max(s, 1e-4));
    const p = panelOf[b]!;
    panelMean[p]! += m;
    panelSd[p]! += Math.sqrt(v);
  }
  let total = 0;
  let variance = 0;
  for (let p = 0; p < NP; p++) {
    total += panelMean[p]!;
    const sp = panelSd[p]!;
    if (!sp) continue;
    const row = p * NP;
    for (let q = 0; q < NP; q++) if (panelSd[q]) variance += panelRho[row + q]! * sp * panelSd[q]!;
  }
  const m = total / OZ;
  const s = Math.sqrt(variance) / OZ;
  const ls2 = Math.log(1 + (s * s) / Math.max(m * m, 1e-9));
  const lm = Math.log(Math.max(m, 1e-9)) - ls2 / 2;
  const ls = Math.sqrt(ls2);
  return { mean: m, sd: s, p10: Math.exp(lm - Z90 * ls), p50: Math.exp(lm), p90: Math.exp(lm + Z90 * ls) };
}

/** Each block's spread of gold above the cut-off, ounces: what drilling near it could take away. */
export function widthShare(mean: ArrayLike<number>, sd: ArrayLike<number>, out: Float64Array) {
  for (let b = 0; b < mean.length; b++) out[b] = Math.sqrt(blockGold(mean[b]!, Math.max(sd[b]!, 1e-4))[1]) / OZ;
  return out;
}

/* ---------- Where to drill next ---------- */
export interface Candidate {
  collar: THREE.Vector3;
  bearing: number;
  dip: number;
  score: number;
}

/**
 * Ranks collars on a 30 m grid by how much of the interval's width lies close to the hole each would drill: a
 * block within the covariance range of the trace gives up about ρ(d)² of its share, ρ(d) = exp(−d / range). The
 * engine then prices the winner exactly. `ground` gives the surface height.
 */
export function rankCandidates(
  share: Float64Array,
  centres: Float64Array,
  holes: Hole[],
  ground: (x: number, z: number) => number,
  count = 1,
): Candidate[] {
  const live: number[] = [];
  for (let b = 0; b < share.length; b++) if (share[b]! > 20) live.push(b);
  const R = VARIOGRAM.rangeM;
  const out: Candidate[] = [];
  const step = 30;
  for (let x = X0 + 30; x <= X0 + NX * B - 30; x += step) {
    for (let z = Z0 + 30; z <= Z0 + NZ * B - 30; z += step) {
      if (holes.some((h) => Math.hypot(h.collar.x - x, h.collar.z - z) < 40)) continue;
      for (const [bearing, dip] of [[BEARING, DIP], [0, 90]] as const) {
        const collar = new THREE.Vector3(x, ground(x, z), z);
        const h = designHole(0, collar, bearing, dip, "agent");
        const pts = compositeCentres(h).filter((_, i) => i % 3 === 1);
        let score = 0;
        for (const b of live) {
          const bx = centres[b * 3]!;
          const by = centres[b * 3 + 1]!;
          const bz = centres[b * 3 + 2]!;
          let d2 = Infinity;
          for (const p of pts) {
            const dd = (p[0] - bx) ** 2 + (p[1] - by) ** 2 + (p[2] - bz) ** 2;
            if (dd < d2) d2 = dd;
          }
          if (d2 > 9 * R * R) continue;
          const rho = Math.exp(-Math.sqrt(d2) / R);
          score += share[b]! * rho * rho;
        }
        // Metres cost money: a short hole that does as much is better.
        out.push({ collar, bearing, dip, score: score / (1 + h.length / 2000) });
      }
    }
  }
  out.sort((a, b) => b.score - a.score);
  return out.slice(0, count);
}

/** Blocks within `ranges` covariance ranges of a hole's trace. */
export function blocksNear(h: Hole, centres: Float64Array, ranges: number) {
  const pts = compositeCentres(h).filter((_, i) => i % 2 === 0);
  const R = ranges * VARIOGRAM.rangeM;
  const near: number[] = [];
  for (let b = 0; b < NB; b++) {
    const bx = centres[b * 3]!;
    const by = centres[b * 3 + 1]!;
    const bz = centres[b * 3 + 2]!;
    for (const p of pts) {
      if ((p[0] - bx) ** 2 + (p[1] - by) ** 2 + (p[2] - bz) ** 2 < R * R) {
        near.push(b);
        break;
      }
    }
  }
  return near;
}

/* ---------- What the hole is worth ---------- */
/** Acklam's inverse normal CDF. */
function qnorm(p: number): number {
  const a = [-39.69683028665376, 220.9460984245205, -275.9285104469687, 138.357751867269, -30.66479806614716, 2.506628277459239];
  const b = [-54.47609879822406, 161.5858368580409, -155.6989798598866, 66.80131188771972, -13.28068155288572];
  const c = [-0.007784894002430293, -0.3223964580411365, -2.400758277161838, -2.549732539343734, 4.374664141464968, 2.938163982698783];
  const d = [0.007784695709041462, 0.3224671290700398, 2.445134137142996, 3.754408661907416];
  const poly = (k: number[], x: number) => k.reduce((s, v) => s * x + v, 0);
  if (p < 0.02425) {
    const q = Math.sqrt(-2 * Math.log(p));
    return poly(c, q) / (poly(d, q) * q + 1);
  }
  if (p > 1 - 0.02425) return -qnorm(1 - p);
  const q = p - 0.5;
  const r = q * q;
  return (poly(a, r) * q) / (poly(b, r) * r + 1);
}

/** States in the decision: equally likely values of the contained gold, at the interval's log-normal quantiles. */
export const STATES = 11;

/**
 * The decision the hole informs, for the value-of-information kernel. States: eleven equally likely values of the
 * contained gold (the log-normal's quantiles at 1/22, 3/22, …). Actions: advance to a resource drill-out (worth
 * `perOz` an ounce, less its cost) or drop the target (0). The hole is a noisy reading of the contained gold: on the
 * log scale, a Gaussian update that shrinks the spread from `now` to `after` has noise τ² = σ₀²σ₁² / (σ₀² − σ₁²).
 * The reading is reported in eleven bands split halfway between the states.
 */
export function holeDecision(now: Interval, after: Interval, holeCost: number, drillOut: number): VoiInput {
  const ls = (iv: Interval) => (Math.log(iv.p90) - Math.log(iv.p10)) / (2 * Z90);
  const s0 = Math.max(1e-6, ls(now));
  const s1 = Math.min(s0 * 0.9999, Math.max(1e-6, ls(after)));
  const tau = Math.sqrt((s0 * s0 * s1 * s1) / (s0 * s0 - s1 * s1));
  const lm = Math.log(now.p50);
  const xs = Array.from({ length: STATES }, (_, i) => lm + s0 * qnorm((i + 0.5) / STATES));
  const cut = [-Infinity, ...xs.slice(1).map((x, i) => (x + xs[i]!) / 2), Infinity];
  const likelihood = xs.map((x) => xs.map((_, j) => phi((cut[j + 1]! - x) / tau) - phi((cut[j]! - x) / tau)));
  // Each row must sum to one: put the rounding on the largest cell.
  for (const row of likelihood) {
    const sum = row.reduce((a, b) => a + b, 0);
    const big = row.indexOf(Math.max(...row));
    row[big]! += 1 - sum;
  }
  return {
    prior: xs.map(() => 1 / STATES),
    payoffs: xs.map((x) => [ECON.perOz * Math.exp(x) - drillOut, 0]),
    likelihood,
    cost: holeCost,
  };
}

export const gridE = (x: number) => Math.round(x + GRID_E);
export const gridN = (z: number) => Math.round(-z + GRID_N);
