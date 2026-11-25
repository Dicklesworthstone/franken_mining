/**
 * The engine bridge against independent answers. Needs web/wasm/pkg/engine.wasm (`bun run wasm`).
 * Run: cd web && bun test src/engine.test.ts
 */
import { beforeAll, describe, expect, test } from "bun:test";
import { cvar, estimate, grind, init, monitor, pit, schedule, shells, voi } from "./figures/engine";

const wasmPath = new URL("../wasm/pkg/engine.wasm", import.meta.url);

beforeAll(async () => {
  await init(await Bun.file(wasmPath).arrayBuffer());
});

/** mulberry32, as figures/lib.ts. */
function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
function normal(r: () => number) {
  return () => Math.sqrt(-2 * Math.log(1 - r())) * Math.cos(2 * Math.PI * r());
}

describe("pit", () => {
  /** The best closed set by enumeration, closed under the true cone of slope rise/run (k = 0 top). */
  function bruteForce(nx: number, ny: number, nz: number, size: number, rise: number, run: number, v: number[]) {
    const n = nx * ny * nz;
    const requires: number[][] = [];
    for (let k = 0; k < nz; k++)
      for (let j = 0; j < ny; j++)
        for (let i = 0; i < nx; i++) {
          const req: number[] = [];
          for (let k2 = 0; k2 < k; k2++)
            for (let j2 = 0; j2 < ny; j2++)
              for (let i2 = 0; i2 < nx; i2++) {
                const h2 = ((i2 - i) * size) ** 2 + ((j2 - j) * size) ** 2;
                const dz = (k - k2) * size;
                if (h2 * rise * rise <= dz * dz * run * run) req.push(i2 + nx * (j2 + ny * k2));
              }
          requires.push(req);
        }
    let best = 0;
    for (let mask = 0; mask < 1 << n; mask++) {
      let value = 0;
      let closed = true;
      for (let b = 0; b < n && closed; b++) {
        if (!((mask >> b) & 1)) continue;
        value += v[b]!;
        for (const r of requires[b]!) if (!((mask >> r) & 1)) closed = false;
      }
      if (closed && value > best) best = value;
    }
    return best;
  }

  for (const [nx, ny, nz, rise, run] of [
    [5, 1, 3, 1, 1],
    [3, 3, 2, 1, 1],
    [4, 4, 1, 1, 1],
    [6, 1, 3, 2, 1],
  ] as const) {
    test(`${nx}x${ny}x${nz} at ${rise}/${run} matches enumeration of closed sets`, async () => {
      const r = rng(nx * 100 + ny * 10 + nz);
      for (let trial = 0; trial < 20; trial++) {
        const value = Array.from({ length: nx * ny * nz }, () => Math.round((r() - 0.62) * 20));
        // levels = nz − 1 covers every layer, so the crate's closure is the true cone's.
        const got = await pit({ nx, ny, nz, dx: 10, dy: 10, dz: 10, rise, run, levels: Math.max(1, nz - 1), value });
        expect(got.verified).toBe(true);
        expect(got.value).toBe(bruteForce(nx, ny, nz, 10, rise, run, value));
        const sum = value.reduce((s, v, b) => s + (got.inPit[b] ? v : 0), 0);
        expect(sum).toBe(got.value);
      }
    });
  }

  test("zUp flips layers both ways", async () => {
    const g = { nx: 3, ny: 1, nz: 2, dx: 10, dy: 10, dz: 10, rise: 1, run: 1, levels: 1 };
    // Top layer first: a rich block under a cheap cap.
    const down = await pit({ ...g, value: [-1, -1, -1, 0, 5, 0] });
    const up = await pit({ ...g, zUp: true, value: [0, 5, 0, -1, -1, -1] });
    expect(down.value).toBe(2);
    expect(up.value).toBe(2);
    expect(down.inPit[4]).toBe(1);
    expect([...up.inPit]).toEqual([...down.inPit.slice(3), ...down.inPit.slice(0, 3)]);
  });

  test("timing: 40 x 40 x 20 at 45 degrees", async () => {
    const nx = 40, ny = 40, nz = 20;
    const r = rng(7);
    const value = new Float64Array(nx * ny * nz);
    for (let k = 0; k < nz; k++)
      for (let j = 0; j < ny; j++)
        for (let i = 0; i < nx; i++) {
          // A pipe-like orebody under 30 m of cover: ore up to $60k a block, waste −$1.5k ± $0.5k.
          const d = Math.hypot(i - 20, j - 20) + Math.max(0, 3 - k) * 3;
          value[i + nx * (j + ny * k)] = (d < 8 ? 60_000 * (1 - d / 8) : 0) - 1_500 + 1_000 * (r() - 0.5);
        }
    const got = await pit({ nx, ny, nz, dx: 10, dy: 10, dz: 10, slopeDeg: 45, value });
    console.log(
      `pit 40x40x20: ${got.ms.toFixed(1)} ms, ${got.blocks} blocks in pit, value $${got.value.toFixed(0)}, ` +
        `${got.arcs} arcs (full cones ${got.fullConeArcs}), verified ${got.verified}`,
    );
    expect(got.verified).toBe(true);
    expect(got.blocks).toBeGreaterThan(0);
  });
});

describe("shells and schedule", () => {
  const nx = 16, ny = 16, nz = 8;
  const n = nx * ny * nz;
  const revenue = new Float64Array(n);
  const cost = new Float64Array(n).fill(1_000);
  for (let k = 0; k < nz; k++)
    for (let j = 0; j < ny; j++)
      for (let i = 0; i < nx; i++) {
        const d = Math.hypot(i - 8, j - 8, (k - 5) * 1.3);
        revenue[i + nx * (j + ny * k)] = Math.max(0, 20_000 * (1 - d / 6));
      }
  const grid = { nx, ny, nz, dx: 10, dy: 10, dz: 10, slopeDeg: 45 };

  test("shells nest and the last one is the pit at that factor", async () => {
    const factors = [0.4, 0.6, 0.8, 1.0];
    const s = await shells({ ...grid, revenue, cost, factors });
    expect(s.shells.length).toBe(4);
    for (let i = 1; i < 4; i++) expect(s.shells[i]!.blocks).toBeGreaterThanOrEqual(s.shells[i - 1]!.blocks);
    expect(s.shells[0]!.blocks).toBeGreaterThan(0);
    expect(s.shells[3]!.blocks).toBeGreaterThan(s.shells[0]!.blocks);
    const p = await pit({ ...grid, value: revenue.map((r, b) => r - cost[b]!) });
    expect(s.shells[3]!.value).toBeCloseTo(p.value, 0);
    const inLast = [...s.firstShell].filter((f) => f >= 0 && f <= 3).length;
    expect(inLast).toBe(s.shells[3]!.blocks);
  });

  test("greedy schedule is feasible and respects capacity", async () => {
    const value = revenue.map((r, b) => r - cost[b]!);
    const sch = await schedule({ ...grid, value, periods: 6, capacity: 40 });
    expect(sch.feasible).toBe(true);
    expect(sch.mined).toBeGreaterThan(0);
    for (const t of sch.periodTonnes) expect(t).toBeLessThanOrEqual(40);
  });
});

describe("estimate", () => {
  const model = { nugget: 1e-6, sill: 0.8, rangeM: 60, neighbors: 12 };
  const r = rng(11);
  const samples = Array.from({ length: 200 }, () => ({
    x: 400 * r(),
    y: 400 * r(),
    z: 100 * r(),
    v: Math.log(0.5 + 3 * r()),
  }));

  test("reproduces a sample at its own location with a tiny nugget", async () => {
    const q = samples.slice(0, 20).flatMap((s) => [s.x, s.y, s.z]);
    const got = await estimate({ ...model, samples, prior: 0, queries: q });
    samples.slice(0, 20).forEach((s, i) => {
      expect(Math.abs(got.mean[i]! - s.v)).toBeLessThan(1e-8);
      expect(got.sd[i]!).toBeCloseTo(Math.sqrt(model.nugget), 9);
    });
  });

  test("tends to the prior far from every sample", async () => {
    const got = await estimate({ ...model, samples, prior: 0.25, queries: [50_000, 50_000, 0, -9_000, 0, 50] });
    for (let i = 0; i < 2; i++) {
      expect(Math.abs(got.mean[i]! - 0.25)).toBeLessThan(1e-12);
      expect(got.sd[i]!).toBeCloseTo(Math.sqrt(model.nugget + model.sill), 12);
    }
  });

  test("timing: 8,000 queries against 600 samples", async () => {
    const r2 = rng(5);
    const many = Array.from({ length: 600 }, () => ({ x: 800 * r2(), y: 800 * r2(), z: 200 * r2(), v: r2() - 0.5 }));
    const queries = new Float64Array(8000 * 3);
    for (let q = 0; q < 8000; q++) queries.set([800 * r2(), 800 * r2(), 200 * r2()], q * 3);
    const got = await estimate({ nugget: 0.1, sill: 0.6, rangeM: 120, neighbors: 12, samples: many, prior: 0, queries });
    console.log(
      `estimate 8000 x 600 (12 neighbours): ${got.ms.toFixed(0)} ms, ${got.probes} probes, ${got.refused} refused`,
    );
    expect(got.refused).toBe(0);
    expect(got.mean.every(Number.isFinite)).toBe(true);
  });
});

describe("monitor", () => {
  const model = { nugget: 0.05, sill: 0.5, rangeM: 40, neighbors: 8 };
  /** Mines a 20 x 20 bench block by block. Each value is drawn from the model's own prediction (the null), shifted
   * by `bias` sd and scaled by `spread`. */
  async function run(seed: number, bias: number, spread: number, alpha = 0.05) {
    const mon = await monitor({ ...model, alpha });
    const z = normal(rng(seed));
    let alarmAt = -1;
    let last = undefined as ReturnType<typeof mon.observe> | undefined;
    for (let step = 0; step < 400; step++) {
      const x = (step % 20) * 10, y = Math.floor(step / 20) * 10;
      const pred = mon.estimate([x, y, 0], 0);
      const value = pred.mean[0]! + pred.sd[0]! * (spread * z() + bias);
      last = mon.observe(x, y, 0, 0, value);
      expect(last.mean).toBeCloseTo(pred.mean[0]!, 10);
      expect(last.sd).toBeCloseTo(pred.sd[0]!, 10);
      if (last.alarm && alarmAt < 0) alarmAt = step;
    }
    mon.free();
    return { alarmAt, last: last! };
  }

  test("stays quiet on well-specified sequences (false alarms within alpha)", async () => {
    // Under the null each run alarms with probability at most alpha = 0.05 (Ville). 20 runs: P(4 or more) ≈ 1.6%.
    let alarms = 0;
    for (let seed = 1; seed <= 20; seed++) if ((await run(seed, 0, 1)).alarmAt >= 0) alarms++;
    console.log(`monitor: ${alarms} of 20 well-specified runs alarmed (alpha 0.05)`);
    expect(alarms).toBeLessThanOrEqual(3);
  });

  test("alarms when the geology departs from the model", async () => {
    const shifted = await run(4, 0.6, 1);
    const wider = await run(5, 0, 1.8);
    console.log(`monitor: bias 0.6 sd alarms at block ${shifted.alarmAt}; spread x1.8 at block ${wider.alarmAt}`);
    expect(shifted.alarmAt).toBeGreaterThanOrEqual(0);
    expect(shifted.last.maxLogELocation).toBeGreaterThanOrEqual(shifted.last.logThreshold);
    expect(wider.alarmAt).toBeGreaterThanOrEqual(0);
    expect(wider.last.maxLogEDispersion).toBeGreaterThanOrEqual(wider.last.logThreshold);
  });
});

describe("plant and decisions", () => {
  test("Bond: the mean of throughputs is not the throughput at the mean hardness", async () => {
    const g = await grind({ wi: [5, 20], f80: 400, p80: 100, powerKw: 1000, utilization: 0.8 });
    expect(g.throughput.mean).toBeCloseTo(200, 9);
    expect(g.atMeanWi.throughput).toBeCloseTo(128, 9);
    expect([...g.points.throughput]).toEqual([320, 80]);
  });

  test("EVPI and EVSI of a two-state decision", async () => {
    const v = await voi({
      prior: [0.5, 0.5],
      payoffs: [
        [10, 0],
        [-5, 0],
      ],
      likelihood: [
        [0.9, 0.1],
        [0.2, 0.8],
      ],
      cost: 0.5,
    });
    // Mine pays 2.5 a priori; perfect information is worth 0.5 · 5 = 2.5.
    expect(v.bestAction).toBe(0);
    expect(v.evpi).toBeCloseTo(2.5, 12);
    expect(v.evsi!).toBeGreaterThan(0);
    expect(v.evsi!).toBeLessThanOrEqual(v.evpi);
  });

  test("CVaR", async () => {
    const c = await cvar([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 0.8);
    expect(c.cvar).toBeCloseTo(9.5, 12);
  });
});
