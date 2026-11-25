/**
 * A synthetic gold orebody for the Explore and Plan scenes, generated in the browser from a seed: a plunging lens
 * of mineralisation with a high-grade shoot, roughened by a Gaussian random field, inside barren host rock. A
 * second, smoother field stands for what a chargeability survey would suggest before any hole is drilled: the
 * geologist's prior. Nothing here is measured at any real place.
 *
 * Coordinates are three.js world metres: x east, y up, z south (north is −z). Bearings are degrees clockwise from
 * north. Grades are grams of gold per tonne (g/t).
 */
import * as THREE from "three";
import { clamp, rng } from "./lib";

export type Field3 = (x: number, y: number, z: number) => number;

/** A standard normal sampler from a seeded uniform. */
export function normals(seed: number) {
  const r = rng(seed);
  let spare = NaN;
  return () => {
    if (!Number.isNaN(spare)) {
      const v = spare;
      spare = NaN;
      return v;
    }
    let u = 0;
    while (u <= 1e-12) u = r();
    const m = Math.sqrt(-2 * Math.log(u));
    const a = 2 * Math.PI * r();
    spare = m * Math.sin(a);
    return m * Math.cos(a);
  };
}

/**
 * A stationary Gaussian random field with unit variance and close to exponential covariance exp(−h / range), by
 * random Fourier features: the exponential covariance's spectrum in three dimensions is a multivariate Cauchy, so
 * each wave vector is a Gaussian vector over range·|N(0,1)|. Waves shorter than `shortest` metres are folded back
 * to that length, which keeps the field from aliasing at block scale.
 */
export function gaussianField(seed: number, rangeM: number, features = 128, shortest = 14): Field3 {
  const z = normals(seed);
  const r = rng(seed * 7 + 3);
  const wx = new Float64Array(features);
  const wy = new Float64Array(features);
  const wz = new Float64Array(features);
  const ph = new Float64Array(features);
  const cap = (2 * Math.PI) / shortest;
  for (let n = 0; n < features; n++) {
    const u = Math.max(0.05, Math.abs(z()));
    let a = z() / (rangeM * u);
    let b = z() / (rangeM * u);
    let c = z() / (rangeM * u);
    const m = Math.hypot(a, b, c);
    if (m > cap) {
      a *= cap / m;
      b *= cap / m;
      c *= cap / m;
    }
    wx[n] = a;
    wy[n] = b;
    wz[n] = c;
    ph[n] = 2 * Math.PI * r();
  }
  const k = Math.sqrt(2 / features);
  return (x, y, zz) => {
    let s = 0;
    for (let n = 0; n < features; n++) s += Math.cos(wx[n]! * x + wy[n]! * y + wz[n]! * zz + ph[n]!);
    return s * k;
  };
}

/** A unit vector along a bearing (degrees from north, clockwise) and a plunge (degrees below horizontal). */
export function heading(bearing: number, plunge: number, out = new THREE.Vector3()) {
  const b = THREE.MathUtils.degToRad(bearing);
  const p = THREE.MathUtils.degToRad(plunge);
  return out.set(Math.sin(b) * Math.cos(p), -Math.sin(p), -Math.cos(b) * Math.cos(p));
}

export interface BodyOptions {
  seed?: number;
  /** Centre of the lens. */
  centre: [number, number, number];
  /** Where the lens plunges (bearing), how steeply, and its half-lengths along the plunge, across it and through it. */
  trend?: number;
  plunge?: number;
  /** Dip of the lens's plane, degrees, which tilts its thin axis. */
  dip?: number;
  size?: [along: number, across: number, thick: number];
  /** Grade of the barren host and at the heart of the lens, g/t. */
  background?: number;
  peak?: number;
  /** Standard deviation of the random field in ln(grade), and its range. */
  roughness?: number;
  rangeM?: number;
  /** How far above the background, in ln(grade), the prior's anomaly rises at its heart. */
  priorLift?: number;
}

export interface Body {
  /** ln(grade) of the truth at a point. */
  lnGrade: Field3;
  grade: Field3;
  /** 0 outside the lens, 1 at its heart. */
  shape: Field3;
  /** The prior mean of ln(grade) a chargeability anomaly would suggest: offset, wider and smoother than the truth. */
  prior: Field3;
  centre: THREE.Vector3;
  axis: THREE.Vector3;
  trend: number;
}

export function makeBody(o: BodyOptions): Body {
  const seed = o.seed ?? 11;
  const trend = o.trend ?? 70;
  const c = new THREE.Vector3(...o.centre);
  const u = heading(trend, o.plunge ?? 32);
  // Across the plunge, horizontally; then the thin axis, tilted by the lens's dip.
  const v0 = new THREE.Vector3(u.z, 0, -u.x).normalize();
  const w0 = new THREE.Vector3().crossVectors(u, v0).normalize();
  const tilt = THREE.MathUtils.degToRad(90 - (o.dip ?? 65));
  const v = v0.clone().multiplyScalar(Math.cos(tilt)).addScaledVector(w0, Math.sin(tilt));
  const w = new THREE.Vector3().crossVectors(u, v).normalize();
  const [La, Lb, Lc] = o.size ?? [230, 95, 62];
  const bg = Math.log(o.background ?? 0.035);
  const top = Math.log(o.peak ?? 3.2);
  const sigma = o.roughness ?? 0.55;
  const field = gaussianField(seed, o.rangeM ?? 70);
  const wobble = gaussianField(seed + 101, 260, 48, 60);
  const shoot = c.clone().addScaledVector(u, La * 0.28).addScaledVector(v, Lb * 0.15);
  const d = new THREE.Vector3();
  const shape: Field3 = (x, y, z) => {
    d.set(x - c.x, y - c.y, z - c.z);
    const t = d.dot(u);
    // A gentle bend along the plunge and a pinch-and-swell in thickness.
    const a = d.dot(v) - 0.0011 * t * t;
    const b = d.dot(w) + 0.00055 * t * t;
    const swell = 1 + 0.28 * wobble(x, y, z);
    const lens = Math.exp(-((t / La) ** 2) - ((a / (Lb * swell)) ** 2) - ((b / (Lc * swell)) ** 2));
    d.set(x - shoot.x, y - shoot.y, z - shoot.z);
    const hs = Math.exp(-((d.dot(u) / (La * 0.32)) ** 2) - ((d.dot(v) / (Lb * 0.42)) ** 2) - ((d.dot(w) / (Lc * 0.55)) ** 2));
    return clamp(lens * 0.86 + hs * 0.34, 0, 1);
  };
  const lnGrade: Field3 = (x, y, z) => bg + (top - bg) * shape(x, y, z) + sigma * field(x, y, z);
  // The geologist's picture before drilling: a smoother anomaly, displaced and turned a little from the truth.
  const lift = o.priorLift ?? 1.8;
  const pc = c.clone().add(new THREE.Vector3(26, 16, 18));
  const pu = heading(trend - 14, (o.plunge ?? 32) - 8);
  const pv = new THREE.Vector3(pu.z, 0, -pu.x).normalize();
  const pw = new THREE.Vector3().crossVectors(pu, pv).normalize();
  const prior: Field3 = (x, y, z) => {
    d.set(x - pc.x, y - pc.y, z - pc.z);
    const g = Math.exp(-((d.dot(pu) / (La * 1.25)) ** 2) - ((d.dot(pv) / (Lb * 1.4)) ** 2) - ((d.dot(pw) / (Lc * 1.7)) ** 2));
    return bg + lift * g;
  };
  return { lnGrade, grade: (x, y, z) => Math.exp(lnGrade(x, y, z)), shape, prior, centre: c, axis: u, trend };
}

/* ---------- Colour ---------- */

const GRADE_STOPS: [number, string][] = [
  [0.05, "#1d1630"],
  [0.15, "#3a2a66"],
  [0.3, "#6a4bc4"],
  [0.5, "#d0662a"],
  [1.0, "#f2a33c"],
  [2.0, "#ffd166"],
  [4.0, "#fff4d6"],
];
const gradeStops = GRADE_STOPS.map(([g, c]) => [Math.log(g), new THREE.Color(c)] as const);

/** A grade's colour, on a log scale: dull violet for barren rock, copper at the cut-off, gold and white-hot above. */
export function gradeColour(g: number, out = new THREE.Color()) {
  const l = Math.log(Math.max(g, 1e-6));
  const first = gradeStops[0]!;
  const last = gradeStops[gradeStops.length - 1]!;
  if (l <= first[0]) return out.copy(first[1]);
  if (l >= last[0]) return out.copy(last[1]);
  for (let i = 1; i < gradeStops.length; i++) {
    const [b, cb] = gradeStops[i]!;
    if (l <= b) {
      const [a, ca] = gradeStops[i - 1]!;
      return out.copy(ca).lerp(cb, (l - a) / (b - a));
    }
  }
  return out.copy(last[1]);
}

/** CSS gradient stops for a legend of the grade ramp between two grades. */
export function gradeGradient(lo: number, hi: number) {
  const span = Math.log(hi) - Math.log(lo);
  return GRADE_STOPS.filter(([g]) => g >= lo * 0.999 && g <= hi * 1.001)
    .map(([g, c]) => `${c} ${(((Math.log(g) - Math.log(lo)) / span) * 100).toFixed(1)}%`)
    .join(", ");
}

const SD_STOPS = ["#000000", "#16203f", "#2f3f8f", "#6f56e0", "#b48cff", "#e7d4ff"].map((c) => new THREE.Color(c));
/** Uncertainty's colour, from 0 (known) to 1 (as unknown as the prior): nothing, then a deepening violet haze. */
export function sdColour(f: number, out = new THREE.Color()) {
  const x = clamp(f, 0, 0.999) * (SD_STOPS.length - 1);
  const i = Math.floor(x);
  return out.copy(SD_STOPS[i]!).lerp(SD_STOPS[i + 1]!, x - i);
}
export const SD_GRADIENT = "#000000 0%, #16203f 20%, #2f3f8f 40%, #6f56e0 60%, #b48cff 80%, #e7d4ff 100%";

/** Standard normal CDF (Abramowitz–Stegun 7.1.26 on erf; |error| < 1.5e-7). */
export function phi(x: number) {
  const t = 1 / (1 + 0.3275911 * (Math.abs(x) / Math.SQRT2));
  const y = 1 - ((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * Math.exp(-(x * x) / 2);
  return x >= 0 ? 0.5 * (1 + y) : 0.5 * (1 - y);
}

/** Troy ounces in a gram's worth. */
export const OZ = 31.1034768;
