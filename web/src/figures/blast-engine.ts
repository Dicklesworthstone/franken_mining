/**
 * The blast figure's calls into FrankenMining's engine, live in the browser: fmine-calib's Gaussian spatial predictor
 * (the SpatialFrontPitMonitor's Vecchia-conditioned predictor, compiled to wasm) estimates
 *
 * 1. the grade-control model, block by block on a 1 m grid, from grade-control drilling on a 6 × 5 m grid; the ore
 *    boundary is where the estimate crosses the 0.5 g/t cut-off, with the 10% and 90% probability lines either side;
 * 2. the heave field between the eight buried movement monitors, both components, with its standard deviation, so
 *    the moved dig line carries a band.
 *
 * The samples are synthetic, drawn from blast-plan's synthetic ore body; the predictor and its answers are real.
 * No DOM at import.
 */
import { estimateInSteps, type Sample } from "./engine";
import { BACK, monitors, ORE_TRUE, SPAN, trend, trueGrade } from "./blast-plan";

/** A value on a regular (u, s) grid, sampled bilinearly. */
export class Field {
  constructor(
    readonly u0: number,
    readonly s0: number,
    readonly step: number,
    readonly nu: number,
    readonly ns: number,
    readonly v: Float64Array,
  ) {}
  at(u: number, s: number) {
    const fu = Math.min(this.nu - 1.001, Math.max(0, (u - this.u0) / this.step));
    const fs = Math.min(this.ns - 1.001, Math.max(0, (s - this.s0) / this.step));
    const i = Math.floor(fu);
    const j = Math.floor(fs);
    const a = fu - i;
    const b = fs - j;
    const k = j * this.nu + i;
    const v = this.v;
    return (v[k]! * (1 - a) + v[k + 1]! * a) * (1 - b) + (v[k + this.nu]! * (1 - a) + v[k + this.nu + 1]! * a) * b;
  }
}

const gridOf = (u0: number, u1: number, s0: number, s1: number, step: number) => {
  const nu = Math.round((u1 - u0) / step) + 1;
  const ns = Math.round((s1 - s0) / step) + 1;
  const q = new Float64Array(nu * ns * 3);
  for (let j = 0; j < ns; j++) for (let i = 0; i < nu; i++) q.set([u0 + i * step, s0 + j * step, 0], (j * nu + i) * 3);
  return { u0, s0, step, nu, ns, q };
};

/* ---------- Contouring ---------- */
type Pt = [number, number];
/** Marching squares: the closed loop of `f = 0` with the largest area (f > 0 inside). The grid's rim counts as f < 0. */
function contour(f: Field): Pt[] {
  const { nu, ns, u0, s0, step } = f;
  const val = (i: number, j: number) => (i <= 0 || j <= 0 || i >= nu - 1 || j >= ns - 1 ? -1 : f.v[j * nu + i]!);
  const pos = (i: number, j: number): Pt => [u0 + i * step, s0 + j * step];
  const cross = (a: Pt, b: Pt, fa: number, fb: number): Pt => {
    const t = fa / (fa - fb);
    return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
  };
  // Edges keyed so that neighbouring cells share them exactly.
  const segs = new Map<string, string[]>();
  const pts = new Map<string, Pt>();
  const link = (ka: string, kb: string) => {
    segs.set(ka, [...(segs.get(ka) ?? []), kb]);
    segs.set(kb, [...(segs.get(kb) ?? []), ka]);
  };
  for (let j = 0; j < ns - 1; j++) {
    for (let i = 0; i < nu - 1; i++) {
      const fa = val(i, j);
      const fb = val(i + 1, j);
      const fc = val(i + 1, j + 1);
      const fd = val(i, j + 1);
      const c = (fa > 0 ? 1 : 0) | (fb > 0 ? 2 : 0) | (fc > 0 ? 4 : 0) | (fd > 0 ? 8 : 0);
      if (c === 0 || c === 15) continue;
      const e: Record<string, [string, Pt]> = {
        b: [`h${i},${j}`, cross(pos(i, j), pos(i + 1, j), fa, fb)],
        r: [`v${i + 1},${j}`, cross(pos(i + 1, j), pos(i + 1, j + 1), fb, fc)],
        t: [`h${i},${j + 1}`, cross(pos(i, j + 1), pos(i + 1, j + 1), fd, fc)],
        l: [`v${i},${j}`, cross(pos(i, j), pos(i, j + 1), fa, fd)],
      };
      const centre = (fa + fb + fc + fd) / 4;
      const pairs: Record<number, string[][]> = {
        1: [["l", "b"]],
        2: [["b", "r"]],
        3: [["l", "r"]],
        4: [["r", "t"]],
        5: centre > 0 ? [["l", "t"], ["b", "r"]] : [["l", "b"], ["r", "t"]],
        6: [["b", "t"]],
        7: [["l", "t"]],
        8: [["t", "l"]],
        9: [["t", "b"]],
        10: centre > 0 ? [["b", "l"], ["t", "r"]] : [["b", "r"], ["t", "l"]],
        11: [["t", "r"]],
        12: [["r", "l"]],
        13: [["r", "b"]],
        14: [["b", "l"]],
      };
      for (const [x, y] of pairs[c]!) {
        const [ka, pa] = e[x!]!;
        const [kb, pb] = e[y!]!;
        pts.set(ka, pa);
        pts.set(kb, pb);
        link(ka, kb);
      }
    }
  }
  const seen = new Set<string>();
  let best: Pt[] = [];
  let bestArea = 0;
  for (const start of segs.keys()) {
    if (seen.has(start)) continue;
    const loop: Pt[] = [];
    let prev = "";
    let cur = start;
    while (!seen.has(cur)) {
      seen.add(cur);
      loop.push(pts.get(cur)!);
      const next = (segs.get(cur) ?? []).find((k) => k !== prev && !seen.has(k));
      if (!next) break;
      prev = cur;
      cur = next;
    }
    let area = 0;
    for (let k = 0; k < loop.length; k++) {
      const [ax, ay] = loop[k]!;
      const [bx, by] = loop[(k + 1) % loop.length]!;
      area += ax * by - bx * ay;
    }
    if (Math.abs(area) > bestArea) {
      bestArea = Math.abs(area);
      best = loop;
    }
  }
  return best;
}

/** Chaikin corner cutting, then even resampling: a surveyor's smooth line through the contour. */
function smoothLoop(p: Pt[], passes = 2, every = 1.2): Pt[] {
  if (p.length < 4) return [];
  let q = p;
  for (let k = 0; k < passes; k++) {
    const out: Pt[] = [];
    for (let i = 0; i < q.length; i++) {
      const [ax, ay] = q[i]!;
      const [bx, by] = q[(i + 1) % q.length]!;
      out.push([0.75 * ax + 0.25 * bx, 0.75 * ay + 0.25 * by], [0.25 * ax + 0.75 * bx, 0.25 * ay + 0.75 * by]);
    }
    q = out;
  }
  const out: Pt[] = [q[0]!];
  let need = every;
  for (let i = 0; i < q.length; i++) {
    let [ax, ay] = q[i]!;
    const [bx, by] = q[(i + 1) % q.length]!;
    let d = Math.hypot(bx - ax, by - ay);
    while (d >= need) {
      const t = need / d;
      ax += (bx - ax) * t;
      ay += (by - ay) * t;
      out.push([ax, ay]);
      d -= need;
      need = every;
    }
    need -= d;
  }
  // Run counter-clockwise in (u, s), as the stand-in does.
  let area = 0;
  for (let k = 0; k < out.length; k++) area += out[k]![0] * out[(k + 1) % out.length]![1] - out[(k + 1) % out.length]![0] * out[k]![1];
  return area < 0 ? out.reverse() : out;
}

/* ---------- The calls ---------- */
const CUT = Math.log(0.5);
/** Φ, for the probability that a block is above cut-off. */
const phi = (z: number) => {
  const t = 1 / (1 + 0.2316419 * Math.abs(z));
  const d = 0.3989423 * Math.exp((-z * z) / 2);
  const p = d * t * (0.3193815 + t * (-0.3565638 + t * (1.781478 + t * (-1.821256 + t * 1.330274))));
  return z > 0 ? 1 - p : p;
};

export interface GradeModel {
  /** The ore boundary (P(above cut-off) = 0.5), and the 90% and 10% probability lines. */
  ore: Pt[];
  sure: Pt[];
  maybe: Pt[];
  /** Median grade, g/t, from the estimate's mean of ln(grade). */
  grade: Field;
  samples: number;
  blocks: number;
  ms: number;
  refused: number;
}

/** The grade-control model from grade-control drilling: 6 m along the bench, 5 m across, over the blast and round it. */
export async function gradeModel(): Promise<GradeModel> {
  let seed = 90210;
  const rand = () => {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    return seed / 0x7fffffff;
  };
  const gauss = () => Math.sqrt(-2 * Math.log(rand() + 1e-9)) * Math.cos(2 * Math.PI * rand());
  const samples: Sample[] = [];
  for (let s = -1.5; s >= BACK - 9; s -= 5) {
    const row = Math.round(-s / 5);
    for (let u = SPAN.uMin - 10 + (row % 2) * 3; u <= SPAN.uMax + 10; u += 6) {
      // Assays: a 15 m bench composite, ±12% sampling and assay error.
      samples.push({ x: u + (rand() - 0.5) * 0.6, y: s + (rand() - 0.5) * 0.6, z: 0, v: Math.log(trueGrade(u, s)) + 0.12 * gauss() });
    }
  }
  const vs = samples.map((p) => p.v);
  const mean = vs.reduce((a, b) => a + b, 0) / vs.length;
  const varr = vs.reduce((a, b) => a + (b - mean) ** 2, 0) / vs.length;
  const g = gridOf(SPAN.uMin - 8, SPAN.uMax + 8, BACK - 6, 0, 1);
  const r = await estimateInSteps({ nugget: 0.04 * varr, sill: 0.96 * varr, rangeM: 16, neighbors: 12, samples, prior: mean, queries: g.q });
  const n = g.nu * g.ns;
  const med = new Float64Array(n);
  const p50 = new Float64Array(n);
  const p90 = new Float64Array(n);
  const p10 = new Float64Array(n);
  for (let k = 0; k < n; k++) {
    const m = r.mean[k]!;
    const sd = Math.max(1e-6, r.sd[k]!);
    const p = Number.isNaN(m) ? 0 : phi((m - CUT) / sd);
    med[k] = Number.isNaN(m) ? 0.05 : Math.exp(m);
    p50[k] = p - 0.5;
    p90[k] = p - 0.9;
    p10[k] = p - 0.1;
  }
  const F = (v: Float64Array) => new Field(g.u0, g.s0, g.step, g.nu, g.ns, v);
  const ore = smoothLoop(contour(F(p50)));
  // Fall back to the synthetic body if the contour came out degenerate.
  return {
    ore: ore.length > 20 ? ore : ORE_TRUE,
    sure: smoothLoop(contour(F(p90))),
    maybe: smoothLoop(contour(F(p10))),
    grade: F(med),
    samples: samples.length,
    blocks: n,
    ms: r.ms,
    refused: r.refused,
  };
}

export interface HeaveModel {
  /** Mean movement along and out (m), and the sd of each. */
  along: Field;
  out: Field;
  sdAlong: Field;
  sdOut: Field;
  blocks: number;
  ms: number;
}

/** The heave field between the monitors: the forward movement about its fitted trend, the sideways about its mean. */
export async function heaveModel(): Promise<HeaveModel> {
  // The field is smooth at the scale of the monitors' spacing: a 2 m grid, sampled bilinearly, carries it.
  const g = gridOf(SPAN.uMin - 8, SPAN.uMax + 8, BACK - 6, 4, 2);
  const n = g.nu * g.ns;
  // Monitor readings are good to about ±0.17 m (1 sd); the residual field varies over about 25 m.
  const vg = { nugget: 0.03, sill: 0.32, rangeM: 25, neighbors: 8 };
  const priorOut = new Float64Array(n);
  for (let k = 0; k < n; k++) priorOut[k] = trend(g.q[k * 3 + 1]!);
  const out = await estimateInSteps({
    ...vg,
    samples: monitors.map((m) => ({ x: m.u, y: m.s, z: 0, v: m.ds, prior: trend(m.s) })),
    prior: priorOut,
    queries: g.q,
  });
  const meanAlong = monitors.reduce((a, m) => a + m.du, 0) / monitors.length;
  const along = await estimateInSteps({
    ...vg,
    samples: monitors.map((m) => ({ x: m.u, y: m.s, z: 0, v: m.du })),
    prior: meanAlong,
    queries: g.q,
  });
  const F = (v: Float64Array) => new Field(g.u0, g.s0, g.step, g.nu, g.ns, v);
  return { along: F(along.mean), out: F(out.mean), sdAlong: F(along.sd), sdOut: F(out.sd), blocks: n * 2, ms: out.ms + along.ms };
}
