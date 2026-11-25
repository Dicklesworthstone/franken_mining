/**
 * The Plan scene's solver: FrankenMining's certified ultimate pit and nested shells (engine.ts, the workspace's own
 * Rust as WebAssembly), run in a Web Worker so that dragging a slider never stalls the page.
 *
 * This module is both ends. On the page, `solver()` starts a module worker that imports this same file (its own
 * URL, from `import.meta.url`) and talks to it; where workers cannot start, it runs the same code in the page. In
 * the worker, the code at the bottom answers: it builds the block model once, then for each request prices every
 * block, calls `pit()` and then `shells()`, and posts each result as soon as it has it.
 */
import { EngineError, engineUrl, init, pit, shells, type PitResult, type ShellsResult } from "./engine";
import { makeProspect } from "./drill-model";
import { FACTORS, grades, grid, values, type Econ } from "./pit-model";

export const url = import.meta.url;

type Request = { type: "init"; engine: string } | { type: "model" } | { type: "solve"; id: number; econ: Econ };
type Reply =
  | { type: "model"; grade: Float32Array; ms: number }
  | { type: "pit"; id: number; pit: PitResult; ore: Uint8Array; value: Float64Array }
  | { type: "shells"; id: number; shells: ShellsResult }
  | { type: "error"; id: number; message: string };

/** The work itself, the same in the worker and in the page. */
function core() {
  let grade: Float32Array | undefined;
  let built = 0;
  const model = () => {
    if (!grade) {
      const t = performance.now();
      grade = grades(makeProspect(11).body);
      built = performance.now() - t;
    }
    return grade;
  };
  return {
    model: () => ({ grade: model(), ms: built }),
    async solve(econ: Econ, post: (r: Reply) => void, id: number) {
      try {
        const g = model();
        const v = values(g, econ);
        const at = grid(econ.slope);
        const p = await pit({ ...at, value: v.value });
        post({ type: "pit", id, pit: p, ore: v.ore, value: v.value });
        const s = await shells({ ...at, revenue: v.revenue, cost: v.cost, factors: FACTORS });
        post({ type: "shells", id, shells: s });
      } catch (e) {
        post({ type: "error", id, message: e instanceof Error ? e.message : String(e) });
      }
    },
  };
}

export interface Solver {
  /** Every block's grade, g/t, and how long the model took to build. */
  model(): Promise<{ grade: Float32Array; ms: number }>;
  /** Solves the pit at `econ`, then its shells; calls back with each as it lands. */
  solve(econ: Econ, onPit: (p: PitResult, ore: Uint8Array, value: Float64Array) => void, onShells: (s: ShellsResult) => void): Promise<void>;
  /** Where the work runs. */
  readonly where: "worker" | "page";
}

/** A solver in a worker, or in the page where a module worker cannot start. */
export async function solver(): Promise<Solver> {
  const engine = new URL(engineUrl(), location.href).href;
  let worker: Worker | undefined;
  // The worker's first answer is the block model: it proves the worker runs, and the page needs the grades.
  let model: { grade: Float32Array; ms: number } | undefined;
  try {
    const src = `import ${JSON.stringify(url)};`;
    const blob = URL.createObjectURL(new Blob([src], { type: "text/javascript" }));
    worker = new Worker(blob, { type: "module", name: "fmine-pit" });
    // A worker that cannot load its module reports an error event; fall back to the page then.
    await new Promise<void>((ok, fail) => {
      const w = worker!;
      const done = (e: MessageEvent<Reply>) => {
        if (e.data.type !== "model") return;
        w.removeEventListener("message", done);
        model = { grade: e.data.grade, ms: e.data.ms };
        ok();
      };
      w.addEventListener("message", done);
      w.addEventListener("error", () => fail(new Error("worker did not start")), { once: true });
      w.postMessage({ type: "init", engine } satisfies Request);
      w.postMessage({ type: "model" } satisfies Request);
      setTimeout(() => fail(new Error("worker timed out")), 20000);
    });
  } catch {
    worker?.terminate();
    worker = undefined;
  }

  if (!worker) {
    await init(engine);
    const c = core();
    return {
      where: "page",
      model: async () => c.model(),
      solve: (econ, onPit, onShells) =>
        new Promise<void>((done, fail) => {
          void c.solve(
            econ,
            (r) => {
              if (r.type === "pit") onPit(r.pit, r.ore, r.value);
              else if (r.type === "shells") {
                onShells(r.shells);
                done();
              } else if (r.type === "error") fail(new EngineError(r.message));
            },
            0,
          );
        }),
    };
  }

  const w = worker;
  let next = 1;
  const waiting = new Map<number, { onPit: Parameters<Solver["solve"]>[1]; onShells: Parameters<Solver["solve"]>[2]; done: () => void; fail: (e: Error) => void }>();
  const known = model!;
  w.addEventListener("message", (e: MessageEvent<Reply>) => {
    const r = e.data;
    if (r.type === "model") return;
    const job = waiting.get(r.id);
    if (!job) return;
    if (r.type === "pit") job.onPit(r.pit, r.ore, r.value);
    else if (r.type === "shells") {
      waiting.delete(r.id);
      job.onShells(r.shells);
      job.done();
    } else {
      waiting.delete(r.id);
      job.fail(new EngineError(r.message));
    }
  });
  return {
    where: "worker",
    model: () => Promise.resolve(known),
    solve: (econ, onPit, onShells) =>
      new Promise<void>((done, fail) => {
        const id = next++;
        waiting.set(id, { onPit, onShells, done, fail });
        w.postMessage({ type: "solve", id, econ } satisfies Request);
      }),
  };
}

/* ---------- In the worker ---------- */
type Scope = { onmessage: ((e: MessageEvent<Request>) => void) | null; postMessage(m: Reply, transfer?: Transferable[]): void };
if (typeof document === "undefined" && typeof self !== "undefined" && typeof (self as { importScripts?: unknown }).importScripts === "function") {
  const scope = self as unknown as Scope;
  const c = core();
  let started: Promise<void> | undefined;
  scope.onmessage = (e) => {
    const m = e.data;
    if (m.type === "init") {
      started = init(m.engine);
      return;
    }
    if (m.type === "model") {
      // The grades go to the page as a copy: the worker keeps its own for pricing.
      const { grade, ms } = c.model();
      scope.postMessage({ type: "model", grade: grade.slice(), ms });
      return;
    }
    void (async () => {
      await started;
      await c.solve(
        m.econ,
        (r) => {
          const transfer: Transferable[] = [];
          if (r.type === "pit") transfer.push(r.pit.inPit.buffer as ArrayBuffer, r.ore.buffer as ArrayBuffer, r.value.buffer as ArrayBuffer);
          if (r.type === "shells") transfer.push(r.shells.firstShell.buffer as ArrayBuffer);
          scope.postMessage(r, transfer);
        },
        m.id,
      );
    })();
  };
}
