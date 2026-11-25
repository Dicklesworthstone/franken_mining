/**
 * How a diamond core hole gets drilled, as numbers: the rig trams to the pad, levels on its jacks, raises its mast
 * to the hole's dip, then cores in 3 m runs. Each run takes the time to cut 3 m at the rock's penetration rate, a
 * wireline trip to bring the core up (longer the deeper the hole), and a rod added at the collar. Shifts are 12 h,
 * two a day, with time lost to surveys, bit changes and repairs.
 *
 * The rates are illustrative for a surface rig coring HQ in a weathered desert profile; the weathering bands match
 * the strata drawn on the scene's walls (terrain.ts). Nothing here touches the DOM or three.js.
 */
import { CUTOFF, type Hole } from "./drill-model";

export const OPS = {
  /** HQ wireline rods and core barrel run, m. */
  rodM: 3,
  runM: 3,
  /** Overshot trip speed down and up the rods, m/s. */
  wirelineMs: 1.5,
  /** Per run: latch the inner tube, break the core, lay it in the trays, min. */
  latchMin: 6,
  /** Per rod: break the head off, screw a rod in, min. */
  rodAddMin: 4,
  /** Share of rig time spent drilling and tripping, the rest lost to surveys, bit changes and repairs. */
  availability: 0.85,
  /** Tracked carrier on a dirt track, km/h. */
  trammKmh: 2.5,
  /** Mast raise rate on its hydraulic rams, degrees per second. */
  mastDegS: 1.5,
  /** Levelling, aligning the mast on the bearing, plumbing water, min. */
  setupMin: 75,
  /** Bit speed, rev/min. */
  rpm: 650,
  /** HQ core fills a tray every 4 m or so. */
  trayM: 4,
} as const;

export interface Rock {
  name: string;
  /** Penetration rate while cutting, m/h. */
  rop: number;
}

/** The rock at a vertical depth below the natural surface; quartz-sulphide ore is harder to cut. */
export function rockAt(depth: number, grade: number): Rock {
  if (grade >= CUTOFF) return { name: "silicified ore", rop: 2.2 };
  if (depth < 4) return { name: "soil", rop: 9 };
  if (depth < 34) return { name: "oxide", rop: 6.5 };
  if (depth < 70) return { name: "transitional rock", rop: 4.4 };
  return { name: "fresh rock", rop: 3.2 };
}

export interface Run {
  from: number;
  to: number;
  rock: Rock;
  /** Hours cutting, tripping and adding the rod, and the rig hours at the run's end (availability included). */
  cut: number;
  trip: number;
  hours: number;
  at: number;
}

export interface Plan {
  runs: Run[];
  /** Rig hours to tram, set up and raise the mast, then to core the hole; days in all. */
  tramH: number;
  setupH: number;
  mastH: number;
  coreH: number;
  days: number;
}

/** The hole's runs and its time, from the tram distance and its assays (grade by composite). */
export function plan(h: Hole, tramM: number): Plan {
  const runs: Run[] = [];
  const sin = Math.sin((h.dip * Math.PI) / 180);
  let at = 0;
  for (let from = 0; from < h.length - 1e-9; from += OPS.runM) {
    const to = Math.min(h.length, from + OPS.runM);
    const mid = (from + to) / 2;
    const c = h.composites.find((k) => mid >= k.from && mid < k.to);
    const rock = rockAt(mid * sin, c?.g ?? 0);
    const cut = (to - from) / rock.rop;
    const trip = (2 * to) / OPS.wirelineMs / 3600 + OPS.latchMin / 60 + OPS.rodAddMin / 60;
    const hours = (cut + trip) / OPS.availability;
    at += hours;
    runs.push({ from, to, rock, cut, trip, hours, at });
  }
  const tramH = tramM / 1000 / OPS.trammKmh;
  const setupH = OPS.setupMin / 60;
  const mastH = h.dip / OPS.mastDegS / 3600;
  return { runs, tramH, setupH, mastH, coreH: at, days: (tramH + setupH + mastH + at) / 24 };
}

/** The depth reached after `hours` of coring, interpolated within the run under way. */
export function depthAt(p: Plan, hours: number): { depth: number; run: number; rock: Rock; inRun: number } {
  let prev = 0;
  for (let i = 0; i < p.runs.length; i++) {
    const r = p.runs[i]!;
    if (hours < r.at) {
      // Cutting comes first in each run, then the trip and the rod change.
      const f = Math.min(1, (hours - prev) / (r.cut / OPS.availability));
      return { depth: r.from + (r.to - r.from) * f, run: i, rock: r.rock, inRun: (hours - prev) / r.hours };
    }
    prev = r.at;
  }
  const last = p.runs[p.runs.length - 1]!;
  return { depth: last.to, run: p.runs.length - 1, rock: last.rock, inRun: 1 };
}

/** "Day 3, 14:20" from rig hours since mobilising. */
export function clockOf(hours: number) {
  const day = Math.floor(hours / 24) + 1;
  const h = Math.floor(hours % 24);
  const m = Math.floor((hours * 60) % 60);
  return `day ${day}, ${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}
