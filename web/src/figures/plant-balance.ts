/**
 * A small weighted least-squares mass balance for the plant scene. Every stream carries a dry solids rate and a gold
 * grade; some are measured (an instrument, a standard deviation and an age) and some are inferred, left free for the
 * balance to fill. Each node must conserve solids and gold. The fit moves every measurement as little as its own
 * uncertainty allows until every node closes, by successive linearisation of the bilinear gold constraints:
 *
 *   x̂ = m − V Jᵀ (J V Jᵀ)⁻¹ [c(x) + J (m − x)],  iterated to convergence,
 *
 * then reports each measurement's adjustment over the standard deviation of that adjustment (the normalised
 * adjustment, the usual gross-error test) and the global chi-square. Unmeasured quantities enter with a prior so
 * wide it carries no weight. This is an illustration written for the page, not FrankenMining's engine.
 *
 * No DOM access here: Bun scripts import it to check the numbers.
 */

export type Ev = "measured" | "estimated" | "assumed" | "stale" | "refused";

export interface Quantity {
  id: string;
  /** What it is, in words. */
  label: string;
  unit: string;
  /** The reading, or for an inferred quantity a starting guess. */
  value: number;
  /** Standard deviation of the reading; null when the quantity is inferred. */
  sd: number | null;
  /** The instrument or source, and how old its reading is. */
  source: string;
  age: string;
  ev: Ev;
}

export interface Fit {
  /** Reconciled values, in the order of the quantities. */
  x: number[];
  /** Posterior standard deviations of the reconciled values. */
  sd: number[];
  /** Normalised adjustments (adjustment ÷ its standard deviation); 0 for inferred quantities. */
  z: number[];
  /** Sum of squared standardised adjustments, and its degrees of freedom. */
  chi2: number;
  dof: number;
  /** Constraint residuals of the raw readings (inferred ones filled by the fit), and of the fit. */
  rawResidual: number[];
  residual: number[];
  /** Posterior covariance, for anything derived from the fit. */
  cov: number[][];
  iterations: number;
}

/** Solves A X = B for square A by Gauss–Jordan elimination with partial pivoting. B may have many columns. */
function solve(A: number[][], B: number[][]): number[][] {
  const n = A.length;
  const m = B[0]!.length;
  const a = A.map((r, i) => [...r, ...B[i]!]);
  for (let c = 0; c < n; c++) {
    let p = c;
    for (let r = c + 1; r < n; r++) if (Math.abs(a[r]![c]!) > Math.abs(a[p]![c]!)) p = r;
    [a[c], a[p]] = [a[p]!, a[c]!];
    const piv = a[c]![c]!;
    if (Math.abs(piv) < 1e-300) throw Error("Singular balance");
    const row = a[c]!;
    for (let k = c; k < n + m; k++) row[k]! /= piv;
    for (let r = 0; r < n; r++) {
      if (r === c) continue;
      const f = a[r]![c]!;
      if (!f) continue;
      const rr = a[r]!;
      for (let k = c; k < n + m; k++) rr[k]! -= f * row[k]!;
    }
  }
  return a.map((r) => r.slice(n));
}

const matmul = (A: number[][], B: number[][]) =>
  A.map((r) => B[0]!.map((_, j) => r.reduce((s, v, k) => s + v * B[k]![j]!, 0)));
const transpose = (A: number[][]) => A[0]!.map((_, j) => A.map((r) => r[j]!));

/** Central-difference Jacobian of the constraint function. */
function jacobian(c: (x: number[]) => number[], x: number[]): number[][] {
  const c0 = c(x);
  const J = c0.map(() => x.map(() => 0));
  for (let j = 0; j < x.length; j++) {
    const h = Math.max(1e-6, Math.abs(x[j]!) * 1e-6);
    const xp = [...x];
    const xm = [...x];
    xp[j]! += h;
    xm[j]! -= h;
    const a = c(xp);
    const b = c(xm);
    for (let i = 0; i < c0.length; i++) J[i]![j] = (a[i]! - b[i]!) / (2 * h);
  }
  return J;
}

/**
 * Reconciles the quantities against the node constraints `c(x) = 0`. Inferred quantities get a prior standard
 * deviation `wide` times their starting value, which leaves them effectively free.
 */
export function reconcile(q: Quantity[], c: (x: number[]) => number[], wide = 1e3): Fit {
  const m = q.map((v) => v.value);
  const s2 = q.map((v) => (v.sd === null ? (Math.abs(v.value) * wide + 1) ** 2 : v.sd * v.sd));
  let x = [...m];
  let J: number[][] = [];
  let it = 0;
  for (; it < 40; it++) {
    J = jacobian(c, x);
    const cx = c(x);
    // r = c(x) + J (m − x)
    const r = cx.map((ci, i) => ci + J[i]!.reduce((s, Jij, j) => s + Jij * (m[j]! - x[j]!), 0));
    const VJt = q.map((_, j) => J.map((row) => s2[j]! * row[j]!));
    const S = matmul(J, VJt);
    const lam = solve(S, r.map((v) => [v])).map((v) => v[0]!);
    const next = m.map((mj, j) => mj - VJt[j]!.reduce((s, v, i) => s + v * lam[i]!, 0));
    const step = Math.max(...next.map((v, j) => Math.abs(v - x[j]!) / (Math.abs(x[j]!) + 1e-9)));
    x = next;
    if (step < 1e-10) break;
  }
  // Adjustment covariance V Jᵀ S⁻¹ J V, and the posterior covariance V − that.
  J = jacobian(c, x);
  const VJt = q.map((_, j) => J.map((row) => s2[j]! * row[j]!));
  const S = matmul(J, VJt);
  const SinvJV = solve(S, transpose(VJt));
  const adj = matmul(VJt, SinvJV);
  const cov = q.map((_, i) => q.map((_, j) => (i === j ? s2[i]! : 0) - adj[i]![j]!));
  const sd = cov.map((r, i) => Math.sqrt(Math.max(0, r[i]!)));
  const z = q.map((v, i) => (v.sd === null ? 0 : (x[i]! - m[i]!) / Math.sqrt(Math.max(1e-30, adj[i]![i]!))));
  let chi2 = 0;
  for (let i = 0; i < q.length; i++) if (q[i]!.sd !== null) chi2 += ((x[i]! - m[i]!) / q[i]!.sd!) ** 2;
  const free = q.filter((v) => v.sd === null).length;
  const filled = m.map((v, i) => (q[i]!.sd === null ? x[i]! : v));
  return { x, sd, z, chi2, dof: c(x).length - free, rawResidual: c(filled), residual: c(x), cov, iterations: it + 1 };
}

/** Upper 95% point of the chi-square distribution for small degrees of freedom. */
export const CHI2_95 = [0, 3.84, 5.99, 7.81, 9.49, 11.07, 12.59, 14.07, 15.51, 16.92, 18.31, 19.68, 21.03];

/* ---------- The synthetic concentrator ---------- */

/** Grams per troy ounce. */
export const G_PER_OZ = 31.1035;

/**
 * The plant's quantities over the last 24 hours (rates in dry t/h, grades in g/t, gold in g/h). The true plant
 * behind these readings treats 760 t/h at 1.42 g/t, floats 6% of the mass into a concentrate holding 93% of the
 * gold, leaches 96.8% of that, and so recovers 90.0%. Flotation tailings go through the tailings thickener to the
 * dam; the leach tailings go out on their own line through cyanide destruction. Every reading carries an error
 * drawn within its own standard deviation; the flotation tailings sampler, TS-301, also reads a third low,
 * because its cutter is worn.
 */
/**
 * The plant treats about 1,800 t/h (a SAG mill of 11.6 m and two ball mills of 7.3 m); every rate below is the
 * readings of a 766 t/h pattern scaled by K, which leaves every normalised adjustment and the chi-square unchanged.
 */
const K = 1800 / 766;
export const QUANTITIES: Quantity[] = [
  { id: "t1", label: "Crusher product", unit: "t/h", value: 777 * K, sd: 777 * K * 0.015, source: "Belt scale WT-101 on CV-01, calibrated 11 days ago", age: "3 s", ev: "measured" },
  { id: "dS", label: "Stockpile change", unit: "t/h", value: 19 * K, sd: 7 * K, source: "Drone survey of the coarse-ore stockpile", age: "22 h", ev: "measured" },
  { id: "t2", label: "Mill feed", unit: "t/h", value: 766 * K, sd: 766 * K * 0.01, source: "Belt scale WT-201 on CV-02, calibrated 11 days ago", age: "3 s", ev: "measured" },
  { id: "h", label: "Ore grade, as mined", unit: "g/t", value: 1.46, sd: 1.46 * 0.08, source: "Grade-control model, blocks mined yesterday", age: "6 h", ev: "estimated" },
  { id: "t3", label: "Flotation feed", unit: "t/h", value: 780 * K, sd: 780 * K * 0.03, source: "Flowmeter FT-301 with density gauge DT-301", age: "2 s", ev: "measured" },
  { id: "g3", label: "Flotation feed grade", unit: "g/t", value: 1.47, sd: 1.47 * 0.03, source: "Cyclone-overflow sampler, shift composite", age: "4 h", ev: "measured" },
  { id: "t4", label: "Concentrate", unit: "t/h", value: 46.4 * K, sd: 46.4 * K * 0.05, source: "Flowmeter FT-341 with density gauge DT-341", age: "2 s", ev: "measured" },
  { id: "g4", label: "Concentrate grade", unit: "g/t", value: 21.9, sd: 21.9 * 0.025, source: "Concentrate sampler, shift composite", age: "4 h", ev: "measured" },
  { id: "t5", label: "Flotation tailings", unit: "t/h", value: 720 * K, sd: null, source: "Inferred: flotation feed less concentrate", age: "no meter", ev: "estimated" },
  { id: "g5", label: "Flotation tailings grade", unit: "g/t", value: 0.07, sd: 0.07 * 0.05, source: "Tailings sampler TS-301, shift composite; cutter last inspected 94 days ago", age: "4 h", ev: "measured" },
  { id: "t6", label: "Leach tailings", unit: "t/h", value: 46 * K, sd: null, source: "Inferred: equals the concentrate leached", age: "no meter", ev: "estimated" },
  { id: "g6", label: "Leach tailings grade", unit: "g/t", value: 0.711, sd: 0.711 * 0.06, source: "CIL tailings sampler, solids assay", age: "5 h", ev: "measured" },
  { id: "D", label: "Gold poured", unit: "g/h", value: 971.5 * K, sd: 971.5 * K * 0.004, source: "Doré bar weights and fire assays", age: "7 h", ev: "measured" },
  { id: "t7", label: "Tailings to the dam", unit: "t/h", value: 741 * K, sd: 741 * K * 0.03, source: "Flowmeter FT-501 with density gauge DT-501", age: "2 s", ev: "measured" },
  { id: "g7", label: "Tailings to the dam, grade", unit: "g/t", value: 0.105, sd: 0.105 * 0.04, source: "Tailings sampler TS-501, shift composite; cutter inspected 6 days ago", age: "4 h", ev: "measured" },
];
export const IX = Object.fromEntries(QUANTITIES.map((q, i) => [q.id, i])) as Record<string, number>;

/** The nodes: each conserves solids and, where gold changes hands, gold. Order matters for the labels below. */
export const NODES = [
  "Crushing and stockpile, solids",
  "Grinding, solids",
  "Grinding, gold",
  "Flotation, solids",
  "Flotation, gold",
  "Thickening and leach, solids",
  "Thickening and leach, gold",
  "Tailings thickener, solids",
  "Tailings thickener, gold",
];

export function constraints(x: number[]): number[] {
  const v = (id: string) => x[IX[id]!]!;
  const [t1, dS, t2, h, t3, g3, t4, g4, t5, g5, t6, g6, D, t7, g7] = [
    v("t1"), v("dS"), v("t2"), v("h"), v("t3"), v("g3"), v("t4"), v("g4"), v("t5"), v("g5"), v("t6"), v("g6"), v("D"), v("t7"), v("g7"),
  ];
  return [
    t1 - dS - t2,
    t2 - t3,
    t2 * h - t3 * g3,
    t3 - t4 - t5,
    t3 * g3 - t4 * g4 - t5 * g5,
    t4 - t6,
    t4 * g4 - t6 * g6 - D,
    t5 - t7,
    t5 * g5 - t7 * g7,
  ];
}

export interface Balance {
  /** Every reading in. */
  first: Fit;
  /** The reading the first pass trusts least, by its normalised adjustment, and the runner-up. */
  worst: number;
  next: number;
  /** The worst reading set aside (treated as unknown), as data reconciliation does after a failed test. */
  fit: Fit;
  /** Recovery from the readings as a plant would report it: one less tailings gold over feed gold. */
  rawRecovery: number;
  /** Reconciled recovery (gold poured over reconciled feed gold), and its standard deviation. */
  recovery: number;
  recoverySd: number;
  /** Raw and reconciled head grade, with the reconciled standard deviation. */
  rawHead: number;
  head: number;
  headSd: number;
  /** Feed gold and the overstatement, in troy ounces a month (30.4 days). */
  feedOzMonth: number;
  overstatedOzMonth: number;
}

const rankByZ = (fit: Fit) =>
  fit.z.map((z, i) => [Math.abs(z), i] as const).sort((a, b) => b[0] - a[0]).map(([, i]) => i);

export function balance(): Balance {
  const first = reconcile(QUANTITIES, constraints);
  const [worst, next] = rankByZ(first) as [number, number];
  const aside = QUANTITIES.map((q, i) => (i === worst ? { ...q, sd: null } : q));
  const fit = reconcile(aside, constraints);
  const m = QUANTITIES.map((q) => q.value);
  const r = (id: string) => m[IX[id]!]!;
  const f = (id: string) => fit.x[IX[id]!]!;
  // The plant's own report: tonnes from the meters (tailings by difference), grades from the samplers.
  const feedAu = r("t3") * r("g3");
  const tailAu = (r("t3") - r("t4")) * r("g5") + r("t4") * r("g6");
  const rawRecovery = 1 - tailAu / feedAu;
  // Reconciled: gold poured over reconciled feed gold, with its standard deviation by the delta method.
  const i3 = IX.t3!;
  const ig = IX.g3!;
  const iD = IX.D!;
  const F = f("t3") * f("g3");
  const R = f("D") / F;
  const grad = new Array(QUANTITIES.length).fill(0) as number[];
  grad[iD] = 1 / F;
  grad[i3] = -f("D") / (F * f("t3"));
  grad[ig] = -f("D") / (F * f("g3"));
  let varR = 0;
  for (let i = 0; i < grad.length; i++) for (let j = 0; j < grad.length; j++) varR += grad[i]! * fit.cov[i]![j]! * grad[j]!;
  const hours = 24 * 30.4;
  const feedOzMonth = (F * hours) / G_PER_OZ;
  return {
    first,
    worst,
    next,
    fit,
    rawRecovery,
    recovery: R,
    recoverySd: Math.sqrt(varR),
    rawHead: r("g3"),
    head: f("g3"),
    headSd: fit.sd[ig]!,
    feedOzMonth,
    overstatedOzMonth: (rawRecovery - R) * feedOzMonth,
  };
}
