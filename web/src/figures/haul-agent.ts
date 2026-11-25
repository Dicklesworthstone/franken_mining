/**
 * What the dispatch agent reads in the ledger and how it sizes a queue, as plain arithmetic over the mine's own
 * records (the per-minute time records and the custody ledger). Nothing here is typed in: every figure the agent
 * quotes comes out of the running simulation.
 *
 *   q̄       trucks waiting at S1, on average, over the window
 *   idle₂   share of the window L2 stood waiting for a truck
 *   r₂      L2's loading rate while it has a truck, t/h
 *   k₂      what one more truck adds at L2: mean weighed payload ÷ L2's cycle with no waiting, t/h
 *   lost    min(q̄, 1) × min(k₂, idle₂ × r₂)      the haulage standing in the queue that L2 could have used
 */
import type { Mine } from "./opencut";

export interface Sizing {
  /** Seconds of records looked at. */
  window: number;
  /** Trucks waiting at S1 on average, and truck-minutes queued there in the window. */
  qAvg: number;
  qMin: number;
  /** Share of the window L2 waited for trucks. */
  idle2: number;
  /** L2's loading rate while it has a truck, t/h. */
  rate2: number;
  /** L2's cycle with no waiting, seconds. */
  cycle2: number;
  /** Mean weighed payload of the loads in the window, tonnes, and how many loads that is. */
  payload: number;
  loads: number;
  /** What one more truck at L2 would move, t/h. */
  truckRate2: number;
  /** Tonnes an hour the queue is costing. */
  lost: number;
  /** Whether S1 stays fed with one truck fewer (more than one truck waiting on average). */
  fed: boolean;
  /** Ore tonnes an hour delivered over the window, and waste. */
  ore: number;
  waste: number;
}

export function size(mine: Mine, windowMin = 30): Sizing {
  const now = mine.time;
  const from = now - windowMin * 60;
  const rows = mine.minutes.filter((r) => r.t >= from && r.t < now);
  // The minute under way counts for the part of it that has run.
  const span = rows.reduce((s, r) => s + Math.min(60, now - r.t), 0) || 1;
  const sum = (f: (r: (typeof rows)[number]) => number) => rows.reduce((s, r) => s + f(r), 0);
  const q = sum((r) => r.queue.S1 ?? 0);
  const idle = sum((r) => r.idle.L2 ?? 0);
  const loaded2 = sum((r) => r.loaded.L2 ?? 0);
  const L2 = mine.loaders.find((l) => l.id === "L2");
  const busy2 = span - idle;
  const rate2 =
    loaded2 > 0 && busy2 > 120
      ? (loaded2 / busy2) * 3600
      : L2
        ? ((L2.passes * (L2.passT[0] + L2.passT[1])) / 2 / (L2.passes * L2.passTime)) * 3600
        : 0;
  const recent = mine.ledger.filter((c) => (c.tDepart ?? 0) >= from);
  const payload = recent.length ? recent.reduce((s, c) => s + c.payloadT, 0) / recent.length : 360;
  const cycle2 = mine.cycle("L2");
  const truckRate2 = (payload / cycle2) * 3600;
  const qAvg = q / span;
  const idle2 = idle / span;
  const lost = Math.min(qAvg, 1) * Math.min(truckRate2, idle2 * rate2);
  return {
    window: span,
    qAvg,
    qMin: q / 60,
    idle2,
    rate2,
    cycle2,
    payload,
    loads: recent.length,
    truckRate2,
    lost,
    fed: qAvg > 1,
    ore: (sum((r) => r.ore) / span) * 3600,
    waste: (sum((r) => r.waste) / span) * 3600,
  };
}

/** Trucks waiting at a loader, minute by minute, for a sparkline: the mean over each minute. */
export function queueTrace(mine: Mine, loader: string, minutes: number): number[] {
  const rows = mine.minutes.slice(-minutes);
  return rows.map((r) => (r.queue[loader] ?? 0) / Math.max(1, Math.min(60, mine.time - r.t)));
}

/** The loss the agent sizes, minute by minute over the window, t/h: the series the engine takes the tail of. */
export function lossByMinute(mine: Mine, z: Sizing, windowMin = 30): number[] {
  const now = mine.time;
  return mine.minutes
    .filter((r) => r.t >= now - windowMin * 60 && r.t + 60 <= now)
    .map((r) => Math.min((r.queue.S1 ?? 0) / 60, 1) * Math.min(z.truckRate2, ((r.idle.L2 ?? 0) / 60) * z.rate2));
}

export interface CycleParts {
  loader: string;
  loads: number;
  /** Mean seconds in each part of the cycle, from the custody records of loads tipped in the window. */
  queue: number;
  spot: number;
  load: number;
  haul: number;
  tip: number;
  back: number;
  total: number;
  /** Passes per load, mean. */
  passes: number;
}

/**
 * A loader's truck cycle from the ledger: queued (arrival to reversing in), spotting (reversing in), loading (under
 * the loader to pulling away), hauling loaded, tipping, and the empty return to the next arrival.
 */
export function cycleParts(mine: Mine, loader: string, windowMin = 40, tip = 24): CycleParts | null {
  const now = mine.time;
  const L = mine.ledger.filter((c) => c.loader === loader && (c.tTip ?? 0) >= now - windowMin * 60);
  if (!L.length) return null;
  const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0);
  const queue = mean(L.map((c) => c.tSpot - c.tArrive));
  const spot = mean(L.map((c) => (c.tUnder ?? c.tFirst ?? c.tSpot) - c.tSpot));
  const load = mean(L.map((c) => (c.tDepart ?? 0) - (c.tUnder ?? c.tSpot)));
  // The ledger closes a load partway through the tip; the truck reached the tipping point a little before.
  const haul = mean(L.map((c) => (c.tTip ?? 0) - (c.tDepart ?? 0) - tip * 0.38));
  // The return: from the end of the tip to arriving at the loader again, from the same truck's next load.
  const back: number[] = [];
  for (const c of L) {
    const nxt = mine.ledger.find((d) => d.truck === c.truck && d.n > c.n) ?? null;
    const open = nxt ? null : mine.trucks.find((t) => t.id === c.truck)?.load ?? null;
    const arrive = (nxt ?? open)?.tArrive;
    if (arrive !== undefined && arrive > (c.tTip ?? 0)) back.push(arrive - (c.tTip ?? 0) - tip * 0.62);
  }
  const ret = mean(back);
  return {
    loader,
    loads: L.length,
    queue,
    spot,
    load,
    haul,
    tip,
    back: ret,
    total: queue + spot + load + haul + tip + ret,
    passes: mean(L.map((c) => c.passes.length)),
  };
}

export interface PayloadStats {
  n: number;
  mean: number;
  sd: number;
  min: number;
  max: number;
  /** Share of loads over 110% and over 120% of rated. */
  over110: number;
  over120: number;
}

/** Weighed payloads of the loads in the window (the payload monitor's numbers), against the rated payload. */
export function payloadStats(mine: Mine, rated: number, windowMin = 60): PayloadStats | null {
  const xs = mine.ledger.filter((c) => (c.tTip ?? 0) >= mine.time - windowMin * 60).map((c) => c.payloadT);
  if (!xs.length) return null;
  const mean = xs.reduce((a, b) => a + b, 0) / xs.length;
  const sd = Math.sqrt(xs.reduce((a, b) => a + (b - mean) ** 2, 0) / Math.max(1, xs.length - 1));
  return {
    n: xs.length,
    mean,
    sd,
    min: Math.min(...xs),
    max: Math.max(...xs),
    over110: xs.filter((x) => x > rated * 1.1).length / xs.length,
    over120: xs.filter((x) => x > rated * 1.2).length / xs.length,
  };
}
