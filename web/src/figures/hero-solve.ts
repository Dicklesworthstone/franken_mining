/**
 * The opening scene's twin, computed the way the platform computes it. A drilling campaign is assayed from the
 * synthetic orebody the Explore and Plan scenes share. FrankenMining's spatial predictor estimates every block of the
 * Plan grid from those assays. Each block is priced, with only blocks close enough to the drilling to be classified
 * allowed to count as ore. The certified pseudoflow then re-solves the ultimate pit from today's surveyed surface
 * down, with mined-out blocks free.
 *
 * Everything runs in a module worker, so the scene never stalls; where a worker cannot start, it runs in the page in
 * slices. As in pit-solve.ts, this one file is both ends: the worker imports it by its own URL.
 */
import * as THREE from "three";
import { engineUrl, estimate, init, pit } from "./engine";
import { OZ, makeBody } from "./drill-body";
import { VARIOGRAM, assay, designHole } from "./drill-model";
import { BLOCK_T, COLS, N, NX, NY, NZ, RECOVERY, START, centreX, centreY, centreZ, cutoff, grid, values } from "./pit-model";

export const url = import.meta.url;

/** A block counts as ore only when the drilling knows it this well: sd of ln(grade) under this (about "indicated"). */
export const CLASSIFIED_SD = 0.78;

export interface TwinHole {
  id: string;
  collar: [number, number, number];
  dir: [number, number, number];
  /** Composite grades down the hole, g/t, one per 10 m. */
  g: Float32Array;
}
export interface Twin {
  holes: TwinHole[];
  samples: number;
  /** Per block: expected grade (g/t), sd of ln(grade), mined out already, in the re-solved pit, ore at this price. */
  grade: Float32Array;
  sd: Float32Array;
  air: Uint8Array;
  inPit: Uint8Array;
  ore: Uint8Array;
  estimateMs: number;
  queries: number;
  /** The pit: its value (dollars), the engine's own check of its certificate, its wall time and blocks. */
  value: number;
  verified: boolean;
  solveMs: number;
  /** Still in the ground inside the pit: ore and waste tonnes, mean ore grade, recovered ounces, benches below today. */
  oreT: number;
  wasteT: number;
  oreGrade: number;
  oz: number;
  benches: number;
  cutoff: number;
  price: number;
  where: "worker" | "page";
}

type Request = { type: "init"; engine: string } | { type: "solve"; ground: Float32Array };
type Reply = { type: "twin"; twin: Omit<Twin, "where"> } | { type: "error"; message: string } | { type: "ready" };

/**
 * The operating pit's orebody: a sister lens to the Explore and Plan scenes' (the same seed, so the same texture of
 * grade), lying deeper and a little longer, so that most of it is still below today's floor. Synthetic.
 */
export const hostBody = () =>
  makeBody({ seed: 11, centre: [10, -232, 10], trend: 70, plunge: 26, dip: 62, size: [240, 92, 58], peak: 4.5, background: 0.035, priorLift: 1.8 });

/** A campaign on a staggered 55 m grid, collared south of the lens so the holes, dipping 60° towards 340°, cross it. */
function campaign(body: ReturnType<typeof makeBody>) {
  const holes = [];
  let n = 1;
  for (let iz = -3; iz <= 2; iz++) {
    for (let ix = -4; ix <= 4; ix++) {
      const x = 10 + ix * 55 + (iz & 1 ? 27 : 0);
      const z = 140 + iz * 55;
      holes.push(assay(designHole(n, new THREE.Vector3(x, 0, z), 340, 60, "plan"), body, 1100 + n));
      n++;
    }
  }
  return holes;
}

async function work(ground: Float32Array, slice: () => Promise<void>): Promise<Omit<Twin, "where">> {
  const body = hostBody();
  const holes = campaign(body);
  const samples = holes.flatMap((h) => h.composites.map((c) => ({ x: c.x, y: c.y, z: c.z, v: Math.log(c.g), prior: body.prior(c.x, c.y, c.z) })));

  // Every block, estimated from the assays: conditional mean and sd of ln(grade) at its centre.
  const grade = new Float32Array(N);
  const sd = new Float32Array(N);
  const CHUNK = 3000;
  let busy = 0;
  for (let b0 = 0; b0 < N; b0 += CHUNK) {
    const n = Math.min(CHUNK, N - b0);
    const q = new Float64Array(n * 3);
    const prior = new Float64Array(n);
    for (let o = 0; o < n; o++) {
      const b = b0 + o;
      const i = b % NX;
      const j = Math.floor(b / NX) % NZ;
      const k = Math.floor(b / COLS);
      const x = centreX(i);
      const y = centreY(k);
      const z = centreZ(j);
      q[o * 3] = x;
      q[o * 3 + 1] = y;
      q[o * 3 + 2] = z;
      prior[o] = body.prior(x, y, z);
    }
    const t = performance.now();
    const r = await estimate({ ...VARIOGRAM, samples, prior, queries: q });
    busy += performance.now() - t;
    for (let o = 0; o < n; o++) {
      const mu = r.mean[o]!;
      const s = r.sd[o]!;
      // The expected grade of a log-normal; a refused query stays at the prior's.
      grade[b0 + o] = Number.isFinite(mu) ? Math.exp(mu + (s * s) / 2) : Math.exp(prior[o]!);
      sd[b0 + o] = Number.isFinite(s) ? s : Math.sqrt(VARIOGRAM.nugget + VARIOGRAM.sill);
    }
    await slice();
  }

  // Priced at the Plan scene's starting economics. Blocks the drilling does not know well enough are waste, and
  // blocks above today's surveyed surface are already gone: free to include, worth nothing.
  const econ = START;
  const priced = grade.slice();
  for (let b = 0; b < N; b++) if (sd[b]! > CLASSIFIED_SD) priced[b] = 0;
  const v = values(priced, econ);
  const air = new Uint8Array(N);
  for (let b = 0; b < N; b++) {
    const c = b % COLS;
    const k = Math.floor(b / COLS);
    if (centreY(k) > ground[c]!) {
      air[b] = 1;
      v.value[b] = 0;
    }
  }
  const p = await pit({ ...grid(econ.slope), value: v.value });

  let oreT = 0;
  let wasteT = 0;
  let grams = 0;
  let deepest = 0;
  for (let b = 0; b < N; b++) {
    if (!p.inPit[b] || air[b]) continue;
    const k = Math.floor(b / COLS);
    deepest = Math.max(deepest, k + 1);
    if (v.ore[b]) {
      oreT += BLOCK_T;
      grams += priced[b]! * BLOCK_T;
    } else wasteT += BLOCK_T;
  }
  // Benches below today's floor: the deepest pit level less the deepest mined-out level.
  let floorK = 0;
  for (let c = 0; c < COLS; c++) {
    let k = 0;
    while (k < NY && air[c + k * COLS]) k++;
    floorK = Math.max(floorK, k);
  }
  return {
    holes: holes.map((h) => ({
      id: h.id,
      collar: [h.collar.x, h.collar.y, h.collar.z],
      dir: [h.dir.x, h.dir.y, h.dir.z],
      g: Float32Array.from(h.composites.map((c) => c.g)),
    })),
    samples: samples.length,
    grade,
    sd,
    air,
    inPit: p.inPit,
    ore: v.ore,
    estimateMs: busy,
    queries: N,
    value: p.value,
    verified: p.verified,
    solveMs: p.ms,
    oreT,
    wasteT,
    oreGrade: oreT > 0 ? grams / oreT : 0,
    oz: (grams * RECOVERY) / OZ,
    benches: Math.max(0, deepest - floorK),
    cutoff: cutoff(econ),
    price: econ.price,
  };
}

/** Runs the twin: in a worker where one starts, else in the page a slice at a time. */
export async function solveTwin(ground: Float32Array): Promise<Twin> {
  const engine = new URL(engineUrl(), location.href).href;
  try {
    const src = `import ${JSON.stringify(url)};`;
    const blob = URL.createObjectURL(new Blob([src], { type: "text/javascript" }));
    const w = new Worker(blob, { type: "module", name: "fmine-hero" });
    const twin = await new Promise<Omit<Twin, "where">>((ok, fail) => {
      w.addEventListener("message", (e: MessageEvent<Reply>) => {
        if (e.data.type === "twin") ok(e.data.twin);
        else if (e.data.type === "error") fail(new Error(e.data.message));
      });
      w.addEventListener("error", () => fail(new Error("worker did not start")), { once: true });
      w.postMessage({ type: "init", engine } satisfies Request);
      w.postMessage({ type: "solve", ground } satisfies Request);
      setTimeout(() => fail(new Error("worker timed out")), 45000);
    });
    w.terminate();
    return { ...twin, where: "worker" };
  } catch {
    await init(engine);
    const twin = await work(ground, () => new Promise((r) => setTimeout(r, 16)));
    return { ...twin, where: "page" };
  }
}

/* ---------- In the worker ---------- */
type Scope = { onmessage: ((e: MessageEvent<Request>) => void) | null; postMessage(m: Reply, transfer?: Transferable[]): void };
if (typeof document === "undefined" && typeof self !== "undefined" && typeof (self as { importScripts?: unknown }).importScripts === "function") {
  const scope = self as unknown as Scope;
  let started: Promise<void> | undefined;
  scope.onmessage = (e) => {
    const m = e.data;
    if (m.type === "init") {
      started = init(m.engine);
      return;
    }
    void (async () => {
      try {
        await started;
        const twin = await work(m.ground, async () => {});
        scope.postMessage({ type: "twin", twin }, [twin.grade.buffer, twin.sd.buffer, twin.air.buffer, twin.inPit.buffer as ArrayBuffer, twin.ore.buffer]);
      } catch (err) {
        scope.postMessage({ type: "error", message: err instanceof Error ? err.message : String(err) });
      }
    })();
  };
}
