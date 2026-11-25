/**
 * The blast figure's numbers, with no DOM and no renderer: the bench frame, the drilled pattern and its firing
 * times, the grade-control ore polygon, the heave field (true, as the rock moved, and as eight buried monitors
 * measured it), and the dilution and ore loss of digging to the pre-blast line or to the moved one.
 *
 * Everything is synthetic. The economics follow the plan's illustrative blast-movement card (GB-18): $3,000/oz,
 * 90% recovery, $22/t to process, 1.5 g/t ore, 0.1 g/t waste, 2.7 t/m³.
 *
 * The grade-control model and the heave field between the monitors start as synthetic stand-ins here; once
 * FrankenMining's engine has estimated them in the browser (blast-engine.ts), `adopt()` swaps its answers in, and
 * ORE, ORE_MOVED, grade(), heaveEstimate() and outcome read from them.
 */
import { noise2 } from "./stage";

/* ---------- The bench frame ----------
 * The working bench curves round the pit. `u` runs along the crest (metres of arc at the crest), `s` runs out from
 * the crest towards the pit's centre: s < 0 is the bench top behind the crest, s > 0 the floor below it. The pit's
 * centre is at (0, 0, R0) in world coordinates, so near the origin u ≈ x and s ≈ z. */
export const R0 = 420;
export function toWorld(u: number, s: number): [number, number] {
  const r = R0 - s;
  const phi = u / R0;
  return [r * Math.sin(phi), R0 - r * Math.cos(phi)];
}
export function toBench(x: number, z: number): [number, number] {
  const dz = R0 - z;
  return [R0 * Math.atan2(x, dz), R0 - Math.hypot(x, dz)];
}

/* ---------- The pattern ---------- */
export const BENCH = {
  /** Bench height, subdrill and hole diameter, m. */
  H: 15,
  sub: 1.5,
  dia: 0.229,
  /** Stemming: 25 hole diameters, the usual rule for crushed-aggregate stemming. */
  stem: 25 * 0.229,
  /** Emulsion blend density, t/m³, and rock density. */
  product: 1.25,
  rock: 2.7,
};
export const PATTERN = { name: "225-08", rows: 5, per: 12, burden: 6.5, spacing: 7.5, front: -5, holeMs: 42, rowMs: 67 };

export interface Hole {
  row: number;
  i: number;
  u: number;
  s: number;
  /** Firing time, ms after the first hole. */
  ms: number;
  x: number;
  z: number;
}

export const holes: Hole[] = (() => {
  const out: Hole[] = [];
  const P = PATTERN;
  for (let row = 0; row < P.rows; row++) {
    const s = P.front - row * P.burden;
    // Staggered: odd rows sit half a spacing along. Spacing is true along each row, which is shorter than the crest.
    const k = (R0 - s) / R0;
    for (let i = 0; i < P.per; i++) {
      const along = (i - (P.per - 1) / 2) * P.spacing + (row % 2 ? P.spacing / 2 : 0) - P.spacing / 4;
      const u = along / k;
      const [x, z] = toWorld(u, s);
      out.push({ row, i, u, s, ms: row * P.rowMs + i * P.holeMs, x, z });
    }
  }
  return out;
})();
export const lastMs = Math.max(...holes.map((h) => h.ms));

export const facts = (() => {
  const depth = BENCH.H + BENCH.sub;
  const column = depth - BENCH.stem;
  const perM = (Math.PI / 4) * BENCH.dia ** 2 * BENCH.product * 1000; // kg per metre of hole
  const kg = perM * column;
  const volume = PATTERN.burden * PATTERN.spacing * BENCH.H; // m³ broken per hole
  // The least gap between any two firing times (the 8 ms rule).
  const times = holes.map((h) => h.ms).sort((a, b) => a - b);
  let gap = Infinity;
  for (let i = 1; i < times.length; i++) gap = Math.min(gap, times[i]! - times[i - 1]!);
  return {
    depth,
    column,
    kgPerM: perM,
    kg,
    volume,
    pf: kg / volume,
    pfT: kg / (volume * BENCH.rock),
    holes: holes.length,
    totalKg: kg * holes.length,
    tonnes: volume * holes.length * BENCH.rock,
    gap,
  };
})();

/* ---------- How fast the rock leaves, from the charge (Richards & Moore, 2004) ----------
 * Launch speed v = k·(√m / B)^1.3 for face burst, with m the charge per metre of hole (kg/m) and B the burden (m);
 * the same with the stemming height for rock torn from the collar. k is a site constant, about 13.5 for an average
 * blast and up to about 27 for the worst case; it is assumed here, as it would be until a site calibrates it. */
export const THROW = (() => {
  const G = 9.81;
  const root = Math.sqrt(facts.kgPerM);
  const k = { typical: 13.5, worst: 24 };
  const face = (kk: number) => kk * (root / PATTERN.burden) ** 1.3;
  const collar = (kk: number) => kk * (root / BENCH.stem) ** 1.3;
  return {
    k,
    face: face(k.typical),
    collar: collar(k.typical),
    faceWorst: face(k.worst),
    collarWorst: collar(k.worst),
    /** Furthest throw on level ground at 45°, v²/g, for the worst case. */
    reach: Math.max(face(k.worst), collar(k.worst)) ** 2 / G,
    /** Loose over in-place volume of the muck. */
    swell: 1.3,
  };
})();

/* ---------- The blast's footprint ---------- */
/** Back break line (behind the last row) and how far the muck runs out past the crest. */
export const BACK = PATTERN.front - (PATTERN.rows - 1) * PATTERN.burden - PATTERN.burden * 0.62;
export const TOE = 31;
const uMin = Math.min(...holes.map((h) => h.u)) - PATTERN.spacing * 0.5;
const uMax = Math.max(...holes.map((h) => h.u)) + PATTERN.spacing * 0.5;
export const SPAN = { uMin, uMax, mid: (uMin + uMax) / 2, half: (uMax - uMin) / 2 };

/* ---------- The ore polygon from the grade-control model ---------- */
/** A closed Catmull–Rom loop through control points, sampled evenly. */
function loop(ctrl: [number, number][], per = 10): [number, number][] {
  const n = ctrl.length;
  const out: [number, number][] = [];
  for (let k = 0; k < n; k++) {
    const p0 = ctrl[(k - 1 + n) % n]!;
    const p1 = ctrl[k]!;
    const p2 = ctrl[(k + 1) % n]!;
    const p3 = ctrl[(k + 2) % n]!;
    for (let j = 0; j < per; j++) {
      const t = j / per;
      const t2 = t * t;
      const t3 = t2 * t;
      const f = (a: number, b: number, c: number, d: number) =>
        0.5 * (2 * b + (-a + c) * t + (2 * a - 5 * b + 4 * c - d) * t2 + (-a + 3 * b - 3 * c + d) * t3);
      out.push([f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]);
    }
  }
  return out;
}
/** The synthetic ore body (u, s), the truth the grade-control samples are drawn from: the front edge a few metres
 * behind the crest, the back edge near the last row. */
export const ORE_TRUE: [number, number][] = loop([
  [-30, -5.5],
  [-14, -4.2],
  [2, -5.2],
  [18, -4.0],
  [33, -5.6],
  [40.5, -9],
  [41, -17],
  [37, -25],
  [29, -31.5],
  [14, -32.5],
  [0, -30.5],
  [-12, -32],
  [-24, -29],
  [-34, -22],
  [-39, -14],
  [-37, -8],
]);

/**
 * A polygon's edges filed by the rows (bands of s) they span. Only an edge whose s-range holds s can cross the line
 * through s, so a point needs testing against only the edges filed under its row. Polygons here are made once and
 * never edited, so each is filed once, on first use.
 */
type Rows = { lo: number; hi: number; w: number; rows: Int32Array[] };
const ROWS = new WeakMap<readonly [number, number][], Rows>();
function rowsOf(poly: readonly [number, number][]): Rows {
  const known = ROWS.get(poly);
  if (known) return known;
  let lo = Infinity;
  let hi = -Infinity;
  for (const [, s] of poly) {
    if (s < lo) lo = s;
    if (s > hi) hi = s;
  }
  const n = Math.max(1, Math.min(2048, poly.length >> 1));
  const w = (hi - lo) / n || 1;
  const row = (s: number) => Math.max(0, Math.min(n - 1, Math.floor((s - lo) / w)));
  const lists: number[][] = Array.from({ length: n }, () => []);
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const a = poly[i]![1];
    const b = poly[j]![1];
    for (let k = row(Math.min(a, b)), k1 = row(Math.max(a, b)); k <= k1; k++) lists[k]!.push(i);
  }
  const made = { lo, hi, w, rows: lists.map((l) => Int32Array.from(l)) };
  ROWS.set(poly, made);
  return made;
}

/** Whether (u, s) is inside the polygon: the even-odd crossing test, against the edges filed under s's row. */
export function inside(poly: readonly [number, number][], u: number, s: number) {
  const r = rowsOf(poly);
  // An edge crosses only if one end is above s and the other is not: none can when s is outside the polygon's span.
  if (!(s >= r.lo && s <= r.hi)) return false;
  const edges = r.rows[Math.max(0, Math.min(r.rows.length - 1, Math.floor((s - r.lo) / r.w)))]!;
  let c = false;
  for (let e = 0; e < edges.length; e++) {
    const i = edges[e]!;
    const [ui, si] = poly[i]!;
    const [uj, sj] = poly[i === 0 ? poly.length - 1 : i - 1]!;
    if (si > s !== sj > s && u < ((uj - ui) * (s - si)) / (sj - si) + ui) c = !c;
  }
  return c;
}

/** The synthetic truth's grade at a point (g/t): ore inside the body, waste outside. */
const gn = noise2(31);
export function trueGrade(u: number, s: number) {
  const inOre = inside(ORE_TRUE, u, s);
  const v = gn.fbm(u / 26, s / 26, 3);
  return inOre ? 1.5 + 0.9 * v : Math.max(0.02, 0.1 + 0.22 * v);
}

/** The grade-control model's ore boundary and grade: the synthetic body until the engine's estimate is adopted. */
export let ORE: [number, number][] = ORE_TRUE;
let gradeFn: (u: number, s: number) => number = trueGrade;
/** The grade-control model's grade at a point, g/t. */
export function grade(u: number, s: number) {
  return gradeFn(u, s);
}

/* ---------- Heave ---------- */
const hn = noise2(57);
/** Where the rock at (u, s) on the bench actually went, metres along and out: toward the pit, a little east. */
export function heave(u: number, s: number): [number, number] {
  const back = Math.min(1, Math.max(0, (s - BACK) / (0 - BACK)));
  // The front rows throw furthest; the back moves least; the ends of the blast are held by the rock beside them.
  const ends = 1 - 0.28 * Math.min(1, Math.max(0, (Math.abs(u - SPAN.mid) - (SPAN.half - 14)) / 14));
  const out = (1.7 + 3.2 * back ** 1.35 + 1.1 * Math.max(0, (s + 7) / 7)) * ends + 0.75 * hn.fbm(u / 17, s / 17, 3);
  // Holes fire west to east along each row, so the rock leans east, and the ends spread outwards.
  const along = 0.9 + 0.035 * (u - SPAN.mid) + 0.5 * hn.fbm(u / 15 + 7, s / 15 - 3, 2);
  return [along, out];
}

export interface Monitor {
  id: string;
  u: number;
  s: number;
  /** Depth below the collar at which it was buried, m. */
  depth: number;
  /** The movement it reported, m (along, out, down). */
  du: number;
  ds: number;
  dv: number;
}
/** Eight blast-movement monitors buried near the ore contacts before the blast, as they reported after it. */
export const monitors: Monitor[] = (() => {
  const at: [number, number, number][] = [
    [-31, -9, 7],
    [-12, -6.5, 9],
    [10, -7, 6],
    [30, -9.5, 8],
    [-28, -24, 8],
    [-6, -29, 6],
    [16, -28.5, 9],
    [36, -21, 7],
  ];
  // Repeatable small reading errors, ±0.25 m.
  const err = [0.18, -0.12, 0.22, -0.2, 0.08, -0.16, 0.12, -0.05];
  return at.map(([u, s, depth], k) => {
    const [du, ds] = heave(u, s);
    return { id: `M${k + 1}`, u, s, depth, du: du + err[k]! * 0.6, ds: ds + err[(k + 3) % 8]!, dv: -(1.2 + 0.9 * ((s - BACK) / -BACK)) };
  });
})();

/** The heave field as the monitors tell it: the engine's estimate once adopted, else a stand-in below. */
let fieldFn: ((u: number, s: number) => [number, number]) | null = null;
export function heaveEstimate(u: number, s: number): [number, number] {
  return fieldFn ? fieldFn(u, s) : heaveStandIn(u, s);
}
/** Stand-in for the field: a smooth trend plus inverse-distance residuals. */
function heaveStandIn(u: number, s: number): [number, number] {
  let wsum = 0;
  let au = 0;
  let as = 0;
  for (const m of monitors) {
    const d2 = (u - m.u) ** 2 + (s - m.s) ** 2 + 36;
    const w = 1 / d2;
    wsum += w;
    au += w * m.du;
    as += w * (m.ds - trend(m.s));
  }
  return [au / wsum, trend(s) + as / wsum];
}
/** The forward movement's trend with distance from the back of the blast, fitted to the monitors by least squares. */
const trendFit = (() => {
  const xs = monitors.map((m) => (m.s - BACK) / -BACK);
  const ys = monitors.map((m) => m.ds);
  const n = xs.length;
  const mx = xs.reduce((a, b) => a + b, 0) / n;
  const my = ys.reduce((a, b) => a + b, 0) / n;
  let sxy = 0;
  let sxx = 0;
  for (let i = 0; i < n; i++) {
    sxy += (xs[i]! - mx) * (ys[i]! - my);
    sxx += (xs[i]! - mx) ** 2;
  }
  const b = sxy / sxx;
  return { a: my - b * mx, b };
})();
export function trend(s: number) {
  return trendFit.a + trendFit.b * ((s - BACK) / -BACK);
}

/** A polygon moved by a heave field. */
export function moved(poly: readonly [number, number][], field: (u: number, s: number) => [number, number]): [number, number][] {
  return poly.map(([u, s]) => {
    const [du, ds] = field(u, s);
    return [u + du, s + ds];
  });
}
export let ORE_MOVED = moved(ORE, heaveEstimate);

/* ---------- Dilution and ore loss ---------- */
export const ECON = { price: 3000, recovery: 0.9, process: 22, oreGrade: 1.5, wasteGrade: 0.1, oz: 31.1035 };

export interface Dig {
  /** Tonnes sent to the mill, and of those, waste. */
  mill: number;
  waste: number;
  /** Ore tonnes that go to the dump. */
  lost: number;
  dilution: number;
  loss: number;
}
export interface Outcome {
  ore: number;
  pre: Dig;
  post: Dig;
  /** Value of digging to the moved line instead of the pre-blast one, $. */
  value: number;
  /** Ore kept and dilution avoided by the moved line, t. */
  kept: number;
  avoided: number;
  /** Ranges of the measured movement, m. */
  out: [number, number];
  along: [number, number];
}

/** Counts, on a half-metre grid of the blasted rock, where each piece of ore and waste landed and which line digs
 * it: the grade-control model says what is ore in place, the true heave (plus mixing) says where it went. */
export function count(ore: readonly [number, number][], movedLine: readonly [number, number][]): Outcome {
  const step = 0.5;
  const t = step * step * BENCH.H * BENCH.rock;
  // Mixing at the contacts: each piece lands a little off the smooth field (a dilution kernel, σ ≈ 0.7 m).
  let seed = 1234567;
  const rand = () => {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    return seed / 0x7fffffff;
  };
  const gauss = () => Math.sqrt(-2 * Math.log(rand() + 1e-9)) * Math.cos(2 * Math.PI * rand());
  const pre = { mill: 0, waste: 0, lost: 0 };
  const post = { mill: 0, waste: 0, lost: 0 };
  let oreT = 0;
  for (let u = SPAN.uMin; u <= SPAN.uMax; u += step) {
    for (let s = BACK; s <= 0; s += step) {
      const isOre = inside(ore, u, s);
      if (isOre) oreT += t;
      const [du, ds] = heave(u, s);
      const pu = u + du + 0.7 * gauss();
      const ps = s + ds + 0.7 * gauss();
      const a = inside(ore, pu, ps);
      const b = inside(movedLine, pu, ps);
      if (a) {
        pre.mill += t;
        if (!isOre) pre.waste += t;
      } else if (isOre) pre.lost += t;
      if (b) {
        post.mill += t;
        if (!isOre) post.waste += t;
      } else if (isOre) post.lost += t;
    }
  }
  const dig = (d: typeof pre): Dig => ({ ...d, dilution: d.waste / d.mill, loss: d.lost / oreT });
  const E = ECON;
  const perOreT = (E.oreGrade / E.oz) * E.recovery * E.price - E.process;
  const perWasteT = E.process - (E.wasteGrade / E.oz) * E.recovery * E.price;
  const cost = (d: typeof pre) => d.lost * perOreT + d.waste * perWasteT;
  const outs = monitors.map((m) => m.ds);
  const alongs = monitors.map((m) => m.du);
  return {
    ore: oreT,
    pre: dig(pre),
    post: dig(post),
    value: cost(pre) - cost(post),
    kept: pre.lost - post.lost,
    avoided: pre.waste - post.waste,
    out: [Math.min(...outs), Math.max(...outs)],
    along: [Math.min(...alongs), Math.max(...alongs)],
  };
}
export let outcome: Outcome = count(ORE, ORE_MOVED);

/** What the engine estimated, swapped in for the stand-ins above. */
export interface Adopted {
  ore: [number, number][];
  grade: (u: number, s: number) => number;
  field: (u: number, s: number) => [number, number];
}
export function adopt(m: Adopted) {
  ORE = m.ore;
  gradeFn = m.grade;
  fieldFn = m.field;
  ORE_MOVED = moved(ORE, heaveEstimate);
  outcome = count(ORE, ORE_MOVED);
}
