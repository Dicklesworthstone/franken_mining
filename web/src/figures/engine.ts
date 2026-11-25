/**
 * FrankenMining's own Rust kernels, in the browser: web/wasm compiled to wasm32 (no wasm-bindgen, no imports).
 *
 * - `pit`, `shells`, `schedule`: fmine-optimize's certified ultimate pit (Hochbaum pseudoflow with an independently
 *   verified certificate), nested shells by revenue factor, and TopoSort's greedy schedule.
 * - `estimate`, `monitor`: fmine-calib's SpatialFrontPitMonitor, the Vecchia-conditioned Gaussian predictor and its
 *   sequential PIT e-process alarm.
 * - `grind`, `voi`, `cvar`: fmine-plant's Bond grinding, fmine-numerics' EVPI/EVSI and CVaR.
 *
 * The module loads once, lazily, from the URL in `<meta name="fmine-engine" content="/assets/engine-<hash>.wasm">`
 * (fallback `/assets/engine.wasm`). Tests and scripts call `init(bytes)` first. Every function below is async only
 * for that load; once `ready()` has resolved, `monitor()` handles answer synchronously.
 *
 * Block grids follow the crate: block (i, j, k) is `i + nx·(j + ny·k)`, i east (x), j north (y) and k DOWN from the
 * top (k = 0 is the top bench). Pass `zUp: true` to use k counted UP from the bottom (three.js style); the bridge
 * flips the inputs and outputs.
 */

/** A refusal from the engine: the crate's own message, e.g. "[FMINE-REF-207] (exit 6): ...". */
export class EngineError extends Error {
  /** The FMINE-REF code when the message carries one. */
  readonly code: string | undefined;
  constructor(message: string) {
    super(message);
    this.name = "EngineError";
    this.code = /\[(FMINE-REF-\d+)\]/.exec(message)?.[1];
  }
}

interface Exports {
  memory: WebAssembly.Memory;
  fm_in_f64(len: number): number;
  fm_out_f64_ptr(): number;
  fm_out_f64_len(): number;
  fm_out_i32_ptr(): number;
  fm_out_i32_len(): number;
  fm_out_u8_ptr(): number;
  fm_out_u8_len(): number;
  fm_err_ptr(): number;
  fm_err_len(): number;
  fm_pit(...a: number[]): number;
  fm_shells(...a: number[]): number;
  fm_schedule(...a: number[]): number;
  fm_estimate(samples: number, queries: number, nugget: number, sill: number, rangeM: number, k: number): number;
  fm_mon_new(nugget: number, sill: number, rangeM: number, k: number, alpha: number): number;
  fm_mon_observe(h: number, x: number, y: number, z: number, prior: number, value: number): number;
  fm_mon_condition(h: number, n: number): number;
  fm_mon_estimate(h: number, n: number): number;
  fm_mon_reset(h: number): number;
  fm_mon_free(h: number): number;
  fm_grind(n: number, f80: number, p80: number, powerKw: number, utilization: number): number;
  fm_voi(states: number, actions: number, signals: number, cost: number): number;
  fm_cvar(alpha: number): number;
}

// --- Loading ---

let loading: Promise<Exports> | undefined;

/** The engine's URL: the page's `fmine-engine` meta tag, else `/assets/engine.wasm`. */
export function engineUrl(): string {
  const meta =
    typeof document === "undefined" ? null : document.querySelector<HTMLMetaElement>('meta[name="fmine-engine"]');
  return meta?.content || "/assets/engine.wasm";
}

async function instantiate(source: BufferSource | WebAssembly.Module | string | URL): Promise<Exports> {
  let instance: WebAssembly.Instance;
  if (source instanceof WebAssembly.Module) {
    instance = await WebAssembly.instantiate(source, {});
  } else if (typeof source === "string" || source instanceof URL) {
    const response = fetch(source);
    try {
      instance = (await WebAssembly.instantiateStreaming(response, {})).instance;
    } catch {
      // A server that does not send application/wasm: fall back to the bytes.
      const r = await response;
      if (!r.ok) throw new EngineError(`engine: ${String(source)} answered ${r.status}`);
      instance = await WebAssembly.instantiate(await WebAssembly.compile(await r.arrayBuffer()), {});
    }
  } else {
    instance = await WebAssembly.instantiate(await WebAssembly.compile(source), {});
  }
  return instance.exports as unknown as Exports;
}

/**
 * Starts loading the engine, once. With no argument it fetches `engineUrl()`; tests pass the module's bytes.
 * Later calls return the first call's promise.
 */
export async function init(source?: BufferSource | WebAssembly.Module | string | URL): Promise<void> {
  await load(source);
}

function load(source?: BufferSource | WebAssembly.Module | string | URL): Promise<Exports> {
  loading ??= instantiate(source ?? engineUrl()).catch((e: unknown) => {
    loading = undefined;
    throw e;
  });
  return loading;
}

/** Resolves when the engine is loaded (fetching it if nothing has started the load). */
export async function ready(): Promise<void> {
  await load();
}

// --- Calls ---

interface Out {
  f64: Float64Array;
  i32: Int32Array;
  u8: Uint8Array;
}

const decoder = new TextDecoder();

/** Writes `input` to the engine's input buffer, runs `call`, and copies the output buffers out. */
function invoke(e: Exports, input: Float64Array | null, call: () => number): Out {
  if (input) {
    const ptr = e.fm_in_f64(input.length) >>> 0;
    new Float64Array(e.memory.buffer, ptr, input.length).set(input);
  }
  const status = call();
  if (status < 0) {
    const ptr = e.fm_err_ptr() >>> 0;
    const len = e.fm_err_len() >>> 0;
    throw new EngineError(decoder.decode(new Uint8Array(e.memory.buffer, ptr, len).slice()) || "engine refused");
  }
  const buf = e.memory.buffer;
  return {
    f64: new Float64Array(buf, e.fm_out_f64_ptr() >>> 0, e.fm_out_f64_len() >>> 0).slice(),
    i32: new Int32Array(buf, e.fm_out_i32_ptr() >>> 0, e.fm_out_i32_len() >>> 0).slice(),
    u8: new Uint8Array(buf, e.fm_out_u8_ptr() >>> 0, e.fm_out_u8_len() >>> 0).slice(),
  };
}

const at = (a: ArrayLike<number>, i: number): number => a[i] ?? NaN;
const now = () => performance.now();
/** A turn for the page (input, painting) between stretches of work. */
const breathe = (): Promise<void> => {
  const s = (globalThis as { scheduler?: { yield?: () => Promise<void> } }).scheduler;
  return s?.yield ? s.yield() : new Promise((res) => setTimeout(res, 0));
};

function concat(parts: (ArrayLike<number> | number)[]): Float64Array {
  let n = 0;
  for (const p of parts) n += typeof p === "number" ? 1 : p.length;
  const out = new Float64Array(n);
  let o = 0;
  for (const p of parts) {
    if (typeof p === "number") out[o++] = p;
    else {
      out.set(p, o);
      o += p.length;
    }
  }
  return out;
}

// --- Pit ---

/** A regular block grid with one slope everywhere. */
export interface Grid {
  /** Blocks east, north and down. */
  nx: number;
  ny: number;
  nz: number;
  /** Block sizes in metres (rounded to whole millimetres for the crate's exact cone test). */
  dx: number;
  dy: number;
  dz: number;
  /** Overall slope angle from horizontal; default 45°. Converted to the exact tangent round(tan θ · 10⁴) / 10⁴. */
  slopeDeg?: number;
  /** Or the exact tangent rise / run (positive integers), which wins over `slopeDeg`. */
  rise?: number;
  run?: number;
  /** Cone levels each block requires directly (default 8, at most nz − 1). More is closer to the true cone. */
  levels?: number;
  /** When true, k counts up from the bottom layer; default false (k = 0 is the top, the crate's convention). */
  zUp?: boolean;
}

const blockCount = (g: Grid) => g.nx * g.ny * g.nz;

function geometry(g: Grid): number[] {
  for (const [name, v] of [["nx", g.nx], ["ny", g.ny], ["nz", g.nz]] as const) {
    if (!Number.isInteger(v) || v < 1) throw new EngineError(`grid ${name} must be a positive integer, got ${v}`);
  }
  let rise = g.rise;
  let run = g.run;
  if (rise === undefined || run === undefined) {
    const deg = g.slopeDeg ?? 45;
    if (!(deg > 0 && deg < 90)) throw new EngineError(`slope ${deg}° must lie strictly between 0° and 90°`);
    run = 10_000;
    rise = Math.max(1, Math.round(Math.tan((deg * Math.PI) / 180) * run));
  }
  return [g.nx, g.ny, g.nz, g.dx, g.dy, g.dz, rise, run, g.levels ?? 8];
}

/** Reverses the layer order of per-block data when the caller counts k up from the bottom. */
function layers<T extends Float64Array | Int32Array | Uint8Array>(g: Grid, data: T): T {
  if (!g.zUp) return data;
  const plane = g.nx * g.ny;
  const out = data.slice() as T;
  for (let k = 0; k < g.nz; k++) out.set(data.subarray(k * plane, (k + 1) * plane), (g.nz - 1 - k) * plane);
  return out;
}

function perBlock(g: Grid, v: ArrayLike<number>, what: string): Float64Array {
  const n = blockCount(g);
  if (v.length !== n) throw new EngineError(`${what}: ${v.length} values for ${n} blocks`);
  return layers(g, Float64Array.from(v));
}

export interface PitInput extends Grid {
  /** Each block's value in dollars (any unit; it is scaled to integers, see `scale`). */
  value: ArrayLike<number>;
  /**
   * Integers per dollar. The crate is integer-only, so values are multiplied by `scale` and rounded. Default: the
   * largest power of two keeping the crate far from its 2⁶² overflow guard (about 2⁶⁰ / (n · max|v|), at most 2⁴⁰).
   */
  scale?: number;
}

export interface PitResult {
  /** 1 for blocks in the ultimate pit, in the caller's layer order. */
  inPit: Uint8Array;
  /** The pit's value in dollars: the maximum over every closed set, for the scaled, rounded values. */
  value: number;
  /** The crate's independent `verify` of the pseudoflow certificate, rerun here, agrees with the solver. */
  verified: boolean;
  /** Blocks in the pit. */
  blocks: number;
  /** Precedence arcs kept, and how many the full cones would need. */
  arcs: number;
  fullConeArcs: number;
  boundaryBlocks: number;
  scale: number;
  stats: { mergers: number; splits: number; relabels: number; maxLabel: number };
  /** Wall time of the engine call, arcs included. */
  ms: number;
}

/** The certified ultimate pit (fmine-optimize pitflow). */
export async function pit(input: PitInput): Promise<PitResult> {
  const e = await load();
  const t = now();
  const o = invoke(e, perBlock(input, input.value, "value"), () =>
    e.fm_pit(...geometry(input), input.scale ?? 0),
  );
  const f = o.f64;
  return {
    inPit: layers(input, o.u8),
    value: at(f, 0),
    verified: at(f, 1) === 1,
    arcs: at(f, 2),
    fullConeArcs: at(f, 3),
    boundaryBlocks: at(f, 4),
    blocks: at(f, 5),
    stats: { mergers: at(f, 6), splits: at(f, 7), relabels: at(f, 8), maxLabel: at(f, 9) },
    scale: at(f, 10),
    ms: now() - t,
  };
}

export interface ShellsInput extends Grid {
  /** Revenue per block at factor 1 (non-negative), and cost per block, in dollars. */
  revenue: ArrayLike<number>;
  cost: ArrayLike<number>;
  /** Strictly increasing revenue factors (exact to 1/10⁴), e.g. [0.3, 0.4, …, 1.5]. */
  factors: number[];
  scale?: number;
}

export interface Shell {
  factor: number;
  /** The best pit's value at this factor, in dollars. */
  value: number;
  blocks: number;
  /** Blocks in the pit solved at this factor; fewer than `blocks` only where tied pits differ. */
  solvedPitBlocks: number;
}

export interface ShellsResult {
  /** Per block, the index of the first (smallest) shell holding it, or −1 for none. Block b is in shell s exactly
   * when 0 ≤ firstShell[b] ≤ s. */
  firstShell: Int32Array;
  shells: Shell[];
  arcs: number;
  scale: number;
  ms: number;
}

/** Nested pit shells by revenue factor (fmine-optimize pitflow::shells): each shell certified best at its factor. */
export async function shells(input: ShellsInput): Promise<ShellsResult> {
  const e = await load();
  const t = now();
  const data = concat([perBlock(input, input.revenue, "revenue"), perBlock(input, input.cost, "cost"), input.factors]);
  const o = invoke(e, data, () => e.fm_shells(...geometry(input), input.factors.length, input.scale ?? 0));
  const f = o.f64;
  const list: Shell[] = [];
  for (let s = 2; s + 3 < f.length; s += 4) {
    list.push({ factor: at(f, s), value: at(f, s + 1), blocks: at(f, s + 2), solvedPitBlocks: at(f, s + 3) });
  }
  return { firstShell: layers(input, o.i32), shells: list, scale: at(f, 0), arcs: at(f, 1), ms: now() - t };
}

export interface ScheduleInput extends Grid {
  value: ArrayLike<number>;
  /** Tonnes per block, or one tonnage for every block; default 1. */
  tonnes?: ArrayLike<number> | number;
  periods: number;
  /** Tonnes each period may mine, or one capacity for all. */
  capacity: ArrayLike<number> | number;
  /** Rank per block, lower mined first; NaN leaves a block unmined. Default: the certified ultimate pit's blocks,
   * best value first (precedence always comes first). */
  rank?: ArrayLike<number>;
  scale?: number;
}

export interface ScheduleResult {
  /** Period per block (0-based), −1 for unmined. */
  period: Int32Array;
  mined: number;
  /** Undiscounted value mined, in dollars. */
  value: number;
  /** The crate's own `check` of precedence and capacity, rerun here. */
  feasible: boolean;
  periodValue: Float64Array;
  periodTonnes: Float64Array;
  scale: number;
  ms: number;
}

/** TopoSort's greedy multi-period schedule (fmine-optimize toposort::greedy) with one tonnage limit per period. */
export async function schedule(input: ScheduleInput): Promise<ScheduleResult> {
  const e = await load();
  const n = blockCount(input);
  const P = input.periods;
  const fill = (v: ArrayLike<number> | number | undefined, len: number, d: number) =>
    typeof v === "number" || v === undefined ? new Float64Array(len).fill(v ?? d) : Float64Array.from(v);
  const tonnes = typeof input.tonnes === "object" ? perBlock(input, input.tonnes, "tonnes") : fill(input.tonnes, n, 1);
  const capacity = fill(input.capacity, P, 0);
  if (capacity.length !== P) throw new EngineError(`${capacity.length} capacities for ${P} periods`);
  const rank = input.rank ? perBlock(input, input.rank, "rank") : new Float64Array(n).fill(NaN);
  const t = now();
  const data = concat([perBlock(input, input.value, "value"), tonnes, capacity, rank]);
  const o = invoke(e, data, () => e.fm_schedule(...geometry(input), P, input.scale ?? 0));
  const f = o.f64;
  return {
    period: layers(input, o.i32),
    mined: at(f, 0),
    value: at(f, 1),
    scale: at(f, 2),
    feasible: at(f, 3) === 1,
    periodValue: f.slice(4, 4 + P),
    periodTonnes: f.slice(4 + P, 4 + 2 * P),
    ms: now() - t,
  };
}

// --- Spatial prediction and the drift monitor ---

/** A drill sample. Values are log-grades (the model is Gaussian; feed it ln(grade)). */
export interface Sample {
  x: number;
  y: number;
  z: number;
  /** ln(grade) observed. */
  v: number;
  /** The prior mean at the sample; default the call's scalar `prior`. */
  prior?: number;
}

/** The registered exponential covariance C(h) = nugget·[h = 0] + sill·exp(−h / range), in log-grade units². */
export interface Variogram {
  nugget: number;
  sill: number;
  rangeM: number;
  /** Nearest earlier samples conditioned on (1..16); default 12. */
  neighbors?: number;
}

export interface EstimateInput extends Variogram {
  samples: Sample[];
  /** The prior mean: one number for everywhere, or one per query (then every sample needs its own `prior`). */
  prior: number | ArrayLike<number>;
  /** Query points as x, y, z triples. */
  queries: ArrayLike<number>;
}

export interface EstimateResult {
  /** Conditional mean and sd of ln(grade) at each query; NaN where the crate refused (see `refused`). */
  mean: Float64Array;
  sd: Float64Array;
  refused: number;
  /** observe_sample probes used (about two per query). */
  probes: number;
  ms: number;
}

function samplesData(samples: Sample[], prior: number | ArrayLike<number>): Float64Array {
  const out = new Float64Array(samples.length * 5);
  samples.forEach((s, i) => {
    const p = s.prior ?? (typeof prior === "number" ? prior : NaN);
    if (Number.isNaN(p)) throw new EngineError(`sample ${i} needs a prior when priors are given per query`);
    out.set([s.x, s.y, s.z, p, s.v], i * 5);
  });
  return out;
}

function queriesData(queries: ArrayLike<number>, prior: number | ArrayLike<number>): Float64Array {
  if (queries.length % 3 !== 0) throw new EngineError(`queries hold ${queries.length} numbers, not x, y, z triples`);
  const n = queries.length / 3;
  if (typeof prior !== "number" && prior.length !== n) throw new EngineError(`${prior.length} priors for ${n} queries`);
  const out = new Float64Array(n * 4);
  for (let q = 0; q < n; q++) {
    out[q * 4] = at(queries, q * 3);
    out[q * 4 + 1] = at(queries, q * 3 + 1);
    out[q * 4 + 2] = at(queries, q * 3 + 2);
    out[q * 4 + 3] = typeof prior === "number" ? prior : at(prior, q);
  }
  return out;
}

function estimateResult(o: Out, n: number, t: number): EstimateResult {
  const f = o.f64;
  return { mean: f.slice(0, n), sd: f.slice(n, 2 * n), refused: at(f, 2 * n), probes: at(f, 2 * n + 1), ms: now() - t };
}

/**
 * Conditional Gaussian prediction of ln(grade) from drill samples, by fmine-calib's SpatialFrontPitMonitor: each
 * query is conditioned on its `neighbors` nearest samples under the exponential covariance. Same-location samples
 * make the covariance singular and the crate refuses the queries that use both; jitter or merge them first.
 */
export async function estimate(input: EstimateInput): Promise<EstimateResult> {
  const e = await load();
  const t = now();
  const q = queriesData(input.queries, input.prior);
  const data = concat([samplesData(input.samples, input.prior), q]);
  const n = q.length / 4;
  const o = invoke(e, data, () =>
    e.fm_estimate(input.samples.length, n, input.nugget, input.sill, input.rangeM, input.neighbors ?? 12),
  );
  return estimateResult(o, n, t);
}

/**
 * `estimate` in short calls with a pause for the page between them, so a big grid never holds the page in one long
 * task. Each query is predicted on its own (the probes are undone), so the means and sds are the ones a single call
 * gives. As there, a query the crate refuses comes back NaN, and the call fails only if every query is refused.
 */
export async function estimateInSteps(input: EstimateInput, budgetMs = 20): Promise<EstimateResult> {
  // An engine that cannot load fails here, once, as a single call would.
  await load();
  const n = input.queries.length / 3;
  const mean = new Float64Array(n);
  const sd = new Float64Array(n);
  let refused = 0;
  let probes = 0;
  let ms = 0;
  let first: unknown = null;
  let size = 64;
  for (let a = 0; a < n; ) {
    const b = Math.min(n, a + size);
    const queries = Array.prototype.slice.call(input.queries, a * 3, b * 3) as number[];
    const prior = typeof input.prior === "number" ? input.prior : (Array.prototype.slice.call(input.prior, a, b) as number[]);
    const t0 = now();
    try {
      const r = await estimate({ ...input, prior, queries });
      mean.set(r.mean, a);
      sd.set(r.sd, a);
      refused += r.refused;
      probes += r.probes;
      ms += r.ms;
    } catch (error) {
      // Every query in this stretch refused: NaN for each, as a single call would give them.
      if (!(error instanceof EngineError)) throw error;
      mean.fill(NaN, a, b);
      sd.fill(NaN, a, b);
      refused += b - a;
      first ??= error;
      ms += now() - t0;
    }
    // The next stretch is sized to take about the budget on this machine.
    size = Math.max(16, Math.min(4096, Math.round(((b - a) * budgetMs) / Math.max(1, now() - t0))));
    a = b;
    if (a < n) await breathe();
  }
  if (n > 0 && refused === n && first) throw first;
  // The engine's own time, without the pauses for the page.
  return { mean, sd, refused, probes, ms };
}

export interface MonitorInput extends Variogram {
  /** False-alarm probability over any horizon (Ville); default 0.05, split between the two e-processes. */
  alpha?: number;
}

export interface Observation {
  /** The PIT Φ(z) the crate returned. */
  pit: number;
  /** Either e-process has reached 2/alpha, now or earlier. */
  alarm: boolean;
  /** The predictive mean and sd this observation was judged against, and its standardized residual. */
  mean: number;
  sd: number;
  z: number;
  /** Current e-values (wealth) of the location (bias) and dispersion (spread) bets; the alarm threshold is
   * `threshold` = 2/alpha. Logs are given too, since the e-values can overflow. */
  eLocation: number;
  eDispersion: number;
  logELocation: number;
  logEDispersion: number;
  maxLogELocation: number;
  maxLogEDispersion: number;
  threshold: number;
  logThreshold: number;
  /** Observations bet on so far. */
  steps: number;
  /** The stakes the next observation will be bet at. */
  stakeLocation: number;
  stakeDispersion: number;
}

export interface Monitor {
  /** Predicts, bets on and records one mined block (x, y, z, prior mean, observed ln(grade)). */
  observe(x: number, y: number, z: number, prior: number, value: number): Observation;
  /** Adds samples (e.g. the drillholes) to the conditioning history without betting on them. */
  condition(samples: Sample[], prior?: number): void;
  /** Predictions against the history so far, as `estimate`. */
  estimate(queries: ArrayLike<number>, prior: number | ArrayLike<number>): EstimateResult;
  /** Forgets history and wealth; keeps the model. */
  reset(): void;
  /** Releases the engine's copy. */
  free(): void;
}

/** A sequential drift monitor: fmine-calib's SpatialFrontPitMonitor with its two score-betting e-processes. */
export async function monitor(input: MonitorInput): Promise<Monitor> {
  const e = await load();
  const alpha = input.alpha ?? 0.05;
  const h = e.fm_mon_new(input.nugget, input.sill, input.rangeM, input.neighbors ?? 12, alpha);
  if (h < 0) invoke(e, null, () => h);
  let live = true;
  const alive = () => {
    if (!live) throw new EngineError("monitor already freed");
  };
  return {
    observe(x, y, z, prior, value) {
      alive();
      const f = invoke(e, null, () => e.fm_mon_observe(h, x, y, z, prior, value)).f64;
      return {
        pit: at(f, 0),
        alarm: at(f, 1) === 1,
        mean: at(f, 2),
        sd: at(f, 3),
        z: at(f, 4),
        logELocation: at(f, 5),
        logEDispersion: at(f, 6),
        eLocation: Math.exp(at(f, 5)),
        eDispersion: Math.exp(at(f, 6)),
        maxLogELocation: at(f, 7),
        maxLogEDispersion: at(f, 8),
        logThreshold: at(f, 9),
        threshold: Math.exp(at(f, 9)),
        steps: at(f, 10),
        stakeLocation: at(f, 11),
        stakeDispersion: at(f, 12),
      };
    },
    condition(samples, prior = 0) {
      alive();
      invoke(e, samplesData(samples, prior), () => e.fm_mon_condition(h, samples.length));
    },
    estimate(queries, prior) {
      alive();
      const t = now();
      const q = queriesData(queries, prior);
      return estimateResult(
        invoke(e, q, () => e.fm_mon_estimate(h, q.length / 4)),
        q.length / 4,
        t,
      );
    },
    reset() {
      alive();
      invoke(e, null, () => e.fm_mon_reset(h));
    },
    free() {
      if (!live) return;
      live = false;
      invoke(e, null, () => e.fm_mon_free(h));
    },
  };
}

// --- Plant and decision kernels ---

export interface Distribution {
  mean: number;
  min: number;
  p10: number;
  p50: number;
  p90: number;
  max: number;
}

export interface GrindInput {
  /** Bond ball-mill work index per scenario, kWh/t (the crate's illustrative validity box is 1–60). */
  wi: ArrayLike<number>;
  /** Scenario probabilities (normalised; made exact to 1/10⁹); default equal. */
  weights?: ArrayLike<number>;
  /** Feed and product 80% passing sizes, µm. */
  f80: number;
  p80: number;
  powerKw: number;
  /** Fraction of time running, 0–1. */
  utilization: number;
}

export interface GrindResult {
  /** Throughput (t/h) and specific energy (kWh/t) over the scenarios, moments taken after the model. */
  throughput: Distribution;
  energy: Distribution;
  shutdownProbability: number;
  /** The same model at the weighted-mean hardness: what averaging the inputs first would have said. */
  atMeanWi: { wi: number; throughput: number; energy: number };
  /** Each scenario's throughput and energy, in input order. */
  points: { throughput: Float64Array; energy: Float64Array };
  ms: number;
}

/** Bond grinding throughput over a hardness distribution (fmine-plant grinding::evaluate). */
export async function grind(input: GrindInput): Promise<GrindResult> {
  const e = await load();
  const t = now();
  const n = input.wi.length;
  const data = input.weights ? concat([input.wi, input.weights]) : Float64Array.from(input.wi);
  const f = invoke(e, data, () => e.fm_grind(n, input.f80, input.p80, input.powerKw, input.utilization)).f64;
  const dist = (o: number): Distribution => ({
    mean: at(f, o),
    min: at(f, o + 1),
    p10: at(f, o + 2),
    p50: at(f, o + 3),
    p90: at(f, o + 4),
    max: at(f, o + 5),
  });
  return {
    throughput: dist(0),
    energy: dist(6),
    shutdownProbability: at(f, 12),
    atMeanWi: { throughput: at(f, 13), energy: at(f, 14), wi: at(f, 15) },
    points: { throughput: f.slice(16, 16 + n), energy: f.slice(16 + n, 16 + 2 * n) },
    ms: now() - t,
  };
}

export interface VoiInput {
  /** Prior probability of each state (summing to 1). */
  prior: number[];
  /** payoffs[state][action]. */
  payoffs: number[][];
  /** likelihood[state][signal] = P(signal | state), for EVSI; omit for EVPI only. */
  likelihood?: number[][];
  /** The experiment's cost, in the payoff unit. */
  cost?: number;
}

export interface VoiResult {
  bestAction: number;
  priorValue: number;
  perfectValue: number;
  evpi: number;
  /** null without a likelihood. */
  evsi: number | null;
  netVoi: number | null;
}

/** Expected value of perfect and of sample information (fmine-numerics voi). */
export async function voi(input: VoiInput): Promise<VoiResult> {
  const e = await load();
  const S = input.prior.length;
  const A = input.payoffs[0]?.length ?? 0;
  const L = input.likelihood?.[0]?.length ?? 0;
  const data = concat([input.prior, ...input.payoffs, ...(input.likelihood ?? [])]);
  const f = invoke(e, data, () => e.fm_voi(S, A, L, input.cost ?? 0)).f64;
  const opt = (v: number) => (Number.isNaN(v) ? null : v);
  return {
    bestAction: at(f, 0),
    priorValue: at(f, 1),
    perfectValue: at(f, 2),
    evpi: at(f, 3),
    evsi: opt(at(f, 4)),
    netVoi: opt(at(f, 5)),
  };
}

/** Empirical VaR and CVaR (Rockafellar–Uryasev) of losses at level alpha, e.g. 0.95 (fmine-numerics risk). */
export async function cvar(
  losses: ArrayLike<number>,
  alpha: number,
): Promise<{ alpha: number; meanLoss: number; var: number; cvar: number }> {
  const e = await load();
  const f = invoke(e, Float64Array.from(losses), () => e.fm_cvar(alpha)).f64;
  return { alpha: at(f, 0), meanLoss: at(f, 1), var: at(f, 2), cvar: at(f, 3) };
}
