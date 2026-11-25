/**
 * The underground figure's numbers that FrankenMining's own engine computes, live in the browser: stope grades from
 * the drill composites by fmine-calib's conditional Gaussian predictor, and the mucked grade of the fired stope bet
 * against that model by the same crate's drift monitor (two e-processes). Everything they are fed is synthetic: the
 * drilling samples a made-up grade field, and the buckets sample the same field diluted by the scanned overbreak.
 */
import { EngineError, estimate, monitor, type Monitor, type Observation, type Sample } from "./engine";
import { clamp } from "./lib";
import { noise2 } from "./stage";
import { driveZ, LEVELS, oreThick, STOPES, zFW, zHW, type Stope } from "./underground-plan";

/** The registered exponential covariance of ln(grade) in the ore domain, fitted to the composites (an estimate). */
export const VARIOGRAM = { nugget: 0.05, sill: 0.06, rangeM: 18, neighbors: 12 } as const;
/** The domain's prior mean of ln(g/t). */
export const PRIOR = Math.log(4.4);
const field = noise2(211);
const fieldB = noise2(5);

/** The synthetic grade the drilling and the buckets sample, ln(g/t). Nothing estimates from it directly. */
export const truthLn = (x: number, y: number, z: number) =>
  PRIOR + 1.9 * field.fbm(x * 0.017 + 0.37 * z * 0.017 + 1.3, y * 0.02 - 0.4, 3) + 0.55 * fieldB.n(x * 0.045 - z * 0.04, y * 0.045);

/** A repeatable standard normal for the i-th draw. */
function gauss(i: number) {
  const h = (k: number) => {
    const t = Math.sin(k * 12.9898 + 78.233) * 43758.5453;
    return t - Math.floor(t);
  };
  const u = Math.max(1e-9, h(i * 2 + 1));
  return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * h(i * 2 + 2));
}
const inLens = (x: number, y: number, z: number) => oreThick(x, y) > 0.5 && z > zHW(x, y) && z < zFW(x, y);

/**
 * Diamond drilling from each footwall drive: a fan of four holes every 15 m along strike, north through the lens,
 * logged in 2 m composites. Only the ore intervals feed the ore domain's estimate.
 */
export function drillComposites(): Sample[] {
  const out: Sample[] = [];
  let n = 0;
  for (const L of LEVELS) {
    const z0 = driveZ(L);
    for (let x = -18; x <= 160; x += 15) {
      for (const dip of [-38, -12, 14, 40]) {
        const r = (dip * Math.PI) / 180;
        const sx = Math.sin(((x * 7) % 11) * 0.01);
        for (let d = 3; d < 90; d += 2) {
          const px = x + d * sx;
          const py = L + 1.5 + d * Math.sin(r);
          const pz = z0 - d * Math.cos(r);
          if (!inLens(px, py, pz)) continue;
          out.push({ x: px, y: py, z: pz, v: truthLn(px, py, pz) + Math.sqrt(VARIOGRAM.nugget) * gauss(n++) });
        }
      }
    }
  }
  return out;
}

/** Points inside a stope that its grade is estimated at: 3 along strike, 3 up the 25 m, 3 across the lens. */
function stopePoints(s: Stope): number[] {
  const q: number[] = [];
  const pad = 3;
  for (const fx of [0, 0.5, 1])
    for (const fy of [0.15, 0.5, 0.85])
      for (const fz of [0.2, 0.5, 0.8]) {
        const x = s.xa + pad + (s.xb - s.xa - 2 * pad) * fx;
        const y = s.level + 25 * fy;
        const a = zHW(x, y);
        const b = zFW(x, y);
        q.push(x, y, a + (b - a) * fz);
      }
  return q;
}

export interface StopeGrade {
  /** Expected grade and an 80% interval, g/t (the interval treats the stope's points as fully correlated, so it is
   * wider than the truth). */
  mean: number;
  lo: number;
  hi: number;
  /** Mean predictive sd of ln(grade) over its points. */
  sd: number;
}
export interface Grades {
  byStope: Map<string, StopeGrade>;
  composites: number;
  points: number;
  refused: number;
  ms: number;
}

/** Every stope's grade from the drill composites, in one engine call. */
/**
 * Every stope's grade from the drill composites. Each query is conditioned on its own nearest composites, so the
 * stopes go to the engine eight at a time with `between()` (a frame) in between, and the answer is the same as one call.
 */
export async function stopeGrades(samples: Sample[], between?: () => Promise<void>): Promise<Grades> {
  const byStope = new Map<string, StopeGrade>();
  let points = 0;
  let refused = 0;
  let ms = 0;
  for (let c = 0; c < STOPES.length; c += 8) {
    const queries: number[] = [];
    const spans: [Stope, number, number][] = [];
    for (const s of STOPES.slice(c, c + 8)) {
      const a = queries.length / 3;
      queries.push(...stopePoints(s));
      spans.push([s, a, queries.length / 3]);
    }
    const r = await estimate({ ...VARIOGRAM, samples, prior: PRIOR, queries });
    points += queries.length / 3;
    refused += r.refused;
    ms += r.ms;
    for (const [s, a, b] of spans) {
      let mean = 0;
      let m = 0;
      let sd = 0;
      let n = 0;
      for (let i = a; i < b; i++) {
        const mi = r.mean[i]!;
        const si = r.sd[i]!;
        if (!Number.isFinite(mi) || !Number.isFinite(si)) continue;
        mean += Math.exp(mi + (si * si) / 2);
        m += mi;
        sd += si;
        n++;
      }
      if (!n) continue;
      m /= n;
      sd /= n;
      byStope.set(s.id, { mean: mean / n, lo: Math.exp(m - 1.2816 * sd), hi: Math.exp(m + 1.2816 * sd), sd });
    }
    if (between && c + 8 < STOPES.length) await between();
  }
  return { byStope, composites: samples.length, points, refused, ms };
}

/** A point in the fired stope that a bucket's muck came from: broken rock from anywhere in the opening, drawn down. */
function muckPoint(s: Stope, i: number): [number, number, number] {
  const h = (k: number) => {
    const t = Math.sin((i * 3 + k) * 91.345 + 7.1) * 24634.6345;
    return t - Math.floor(t);
  };
  const x = s.xa + 1 + (s.xb - s.xa - 2) * h(1);
  const y = s.level + 1 + 23 * Math.sqrt(h(2));
  const a = zHW(x, y);
  const b = zFW(x, y);
  return [x, y, a + (b - a) * clamp(h(3), 0.05, 0.95)];
}

export interface MuckWatch {
  /** Buckets bet on so far, and the latest observation. */
  buckets: number;
  /** The bucket (1-based) at which either e-process first crossed 2/alpha; 0 while it has not. */
  alarmAt: number;
  last: Observation | null;
  /** Mucked grade against the model's prediction for the same muck, geometric mean ratio. */
  ratio: number;
  ms: number;
  /** Bets one more bucket (the next one tipped). */
  tip(): Observation | null;
  free(): void;
}

/**
 * The drift monitor on the fired stope's buckets: conditioned on the drill composites, it predicts each bucket's
 * ln(grade) at the point the muck came from and bets on the result. The synthetic buckets carry the scanned
 * overbreak's dilution (waste at no grade), which is what the monitor should come to notice.
 */
export async function muckWatch(samples: Sample[], s: Stope, dilution: number, already: number, between?: () => Promise<void>): Promise<MuckWatch> {
  const t = performance.now();
  const mon: Monitor = await monitor({ ...VARIOGRAM, alpha: 0.05 });
  mon.condition(samples, PRIOR);
  const diluteLn = Math.log(clamp(1 - dilution, 0.3, 1));
  let sumResid = 0;
  const w: MuckWatch = {
    buckets: 0,
    alarmAt: 0,
    last: null,
    ratio: 1,
    ms: 0,
    tip() {
      const i = w.buckets;
      const [x, y, z] = muckPoint(s, i);
      const v = truthLn(x, y, z) + diluteLn + Math.sqrt(VARIOGRAM.nugget) * gauss(50_000 + i);
      try {
        const o = mon.observe(x, y, z, PRIOR, v);
        w.buckets++;
        w.last = o;
        if (o.alarm && !w.alarmAt) w.alarmAt = w.buckets;
        sumResid += v - o.mean;
        w.ratio = Math.exp(sumResid / w.buckets);
        return o;
      } catch (e) {
        if (e instanceof EngineError) return null;
        throw e;
      }
    },
    free: () => mon.free(),
  };
  w.ms = performance.now() - t;
  for (let i = 0; i < already; i++) {
    const t1 = performance.now();
    w.tip();
    w.ms += performance.now() - t1;
    if (between && i % 200 === 199) await between();
  }
  return w;
}
