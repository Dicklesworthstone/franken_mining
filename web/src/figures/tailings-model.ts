/**
 * The synthetic tailings storage facility behind "Bring in the storm": its ground, its embankment and beach, its
 * instruments, and the water balance that says what a storm does to the pond. Every number here is illustrative.
 *
 * Units are metres, y up; a reduced level (RL) is 1,000 m plus y. The embankment crosses a desert valley along x; its
 * crest line bends gently upstream in the middle, and `s` is the distance downstream of that line. Upstream (−z) the
 * valley bends left, and the beach of tailings falls from the crest to the pond at its head. The embankment is raised
 * on the centreline: a starter dam and three raises, each with a core of compacted clay, a filter and drain downstream
 * of it, and rockfill shells.
 *
 * Nothing here touches the DOM, so a Bun script can import it.
 */
import { clamp, smooth } from "./lib";
import { noise2 } from "./stage";

/* ---------- Levels ---------- */
export const RL = (y: number) => 1000 + y;
/** Design crest. */
export const CREST = 36;
/** Tailings level against the embankment, below the crest. */
export const BEACH_TOP = 35.2;
/** The pond today, from the radar gauge on the decant tower. */
export const POND = 33.38;
/** The least freeboard the engineer of record's design allows. */
export const MIN_FREEBOARD = 1.5;
/** Where section A–A cuts the embankment. */
export const SECTION_X = 34;

const g1 = noise2(31);
const g2 = noise2(57);
const g3 = noise2(83);
const g4 = noise2(97);

/** The crest's centreline, bowed gently upstream in the middle. */
export const crestZ = (x: number) => 1e-4 * x * x;
/** Settlement of the crest since the last raise, as the survey found it: deepest over the tallest section. */
export const settle = (x: number) => 0.14 * Math.exp(-(((x - 20) / 150) ** 2));
/** The crest's low point, which sets the freeboard. */
export const CREST_LOW_X = 20;
export const CREST_LOW = CREST - settle(CREST_LOW_X);

/* ---------- The ground ---------- */
/** The valley's centre: straight through the embankment, bending left upstream. */
export const valleyX = (z: number) => {
  const c = Math.max(z, -1200);
  return (c < 0 ? -5e-4 * c * c : 0) + 18 * Math.sin(z / 420);
};
/** Where the seepage collection pond is dug, at the toe: centre and half-sizes at its rim. */
export const SEEP = { x: -12, z: 156, hx: 30, hz: 17, depth: 3.2, slope: 2.5 };
export const seepDepth = (x: number, z: number) => {
  const inX = SEEP.hx - Math.abs(x - SEEP.x);
  const inZ = SEEP.hz - Math.abs(z - SEEP.z);
  return clamp(Math.min(inX, inZ) / SEEP.slope, 0, SEEP.depth);
};

/** Natural ground (before roads), the valley and the hills around it. */
export function natural(x: number, z: number, cell = 0) {
  const xv = x - valleyX(z);
  const raw = z < 0 ? 0.016 * -z + 2.6e-5 * z * z : -0.011 * z;
  const floor = raw < 40 ? raw : 40 + 30 * (1 - Math.exp(-(raw - 40) / 30));
  // Below the embankment the valley opens out onto the plain.
  const open = 1 - 0.62 * smooth((z - 60) / 700);
  const side = 75 * (1 - Math.exp(-((xv / 430) ** 2))) * open;
  const ridge = 24 * Math.max(0, g1.fbm(x / 640, z / 640, 4) + 0.12) * smooth((Math.abs(xv) - 250) / 320);
  // On a coarse grid the fine swells would alias into a pattern of facets: they fade out as the cells grow.
  const swell = 3.2 * g2.fbm(x / 250, z / 250, cell > 40 ? 2 : 4) + 0.7 * g2.fbm(x / 48, z / 48, 3) * (1 - smooth((cell - 6) / 14));
  // Gullies cut into the hills beyond the impoundment's rim, and shallow washes across the plain below the dam.
  const hills = smooth((Math.abs(xv) - 430) / 260);
  const plain = smooth((z - 260) / 300) * (1 - hills);
  const gn = Math.abs(g4.fbm(x / 230, z / 230, cell > 40 ? 2 : 3));
  const gully = -7 * Math.max(0, 1 - gn * 4.5) ** 2.2 * hills - 1.3 * Math.max(0, 1 - gn * 6) ** 2 * plain;
  return floor + side + ridge + swell + gully;
}
export const ground = (x: number, z: number, cell = 0) => natural(x, z, cell) - seepDepth(x, z);

/* ---------- The embankment ---------- */
export const DAM = {
  crestHalf: 7,
  /** Upstream and downstream slopes, horizontal to one vertical. */
  up: 2,
  down: 2.5,
  berm: 6,
  /** Height of the downstream faces between berms. */
  face: 12,
  /** Raise levels: the starter dam's crest, then each raise's. */
  raises: [
    { name: "Starter dam", year: 2014, top: 12 },
    { name: "Raise 1", year: 2018, top: 20 },
    { name: "Raise 2", year: 2021, top: 28 },
    { name: "Raise 3", year: 2024, top: 36 },
  ],
};
/** Height of the embankment's surface at `s` metres downstream of the crest centreline (before settlement). */
export function damTop(s: number) {
  const h = DAM.crestHalf;
  if (s < -h) return CREST - (-h - s) / DAM.up;
  if (s <= h) return CREST + 0.12 * (1 - (s / h) ** 2);
  let y = CREST;
  let at = h;
  const run = DAM.face * DAM.down;
  for (let k = 0; ; k++) {
    if (s <= at + run || k === 2) return y - (s - at) / DAM.down;
    y -= DAM.face;
    at += run;
    if (s <= at + DAM.berm) return y - 0.04 * (s - at);
    at += DAM.berm;
    y -= 0.04 * DAM.berm;
  }
}
/** The downstream toe, where the last face meets the original ground (about). */
export const TOE_S = 109;
export const damAt = (x: number, z: number) => damTop(z - crestZ(x)) - settle(x);

/* ---------- The tailings ---------- */
/** How far the beach falls below its top at `d` metres from the crest: steep near the spigots, flat near the pond. */
export const beachDrop = (d: number) => (0.014 * d) / (1 + d / 220);
/** The water's edge today, in metres from the crest's upstream edge (the beach meets the pond). */
export const SHORE = (BEACH_TOP - POND) / (0.014 - (BEACH_TOP - POND) / 220);
/** Distance upstream of the crest's upstream edge. */
export const upstream = (x: number, z: number) => crestZ(x) - DAM.crestHalf - z;
/** Where the beach falls to a level, in metres from the crest (the noise-free profile). */
export const edgeAt = (level: number) => {
  const drop = BEACH_TOP - level;
  return drop <= 0 ? 0 : drop / Math.max(1e-4, 0.014 - drop / 220);
};

/** The tailings surface: the beach above the pond, the pond's bed below it. -Infinity where there are none. */
export function tailings(x: number, z: number) {
  const d = upstream(x, z);
  if (d < -1.6) return -Infinity;
  const dd = Math.max(0, d);
  const rough = 0.07 * g3.fbm(x / 24, z / 24, 3) + 0.11 * g3.fbm(x / 150 + 7, z / 150, 3);
  if (dd <= SHORE) return BEACH_TOP - beachDrop(dd) + rough * smooth(dd / 25);
  return POND - Math.min(4.2, 0.035 * (dd - SHORE)) + rough * 0.6;
}
/** What lies under the pond or the beach: tailings, or the natural ground where it rises above them. */
export const bed = (x: number, z: number) => Math.max(ground(x, z), tailings(x, z));

/* ---------- Instruments ---------- */
export type Zone = "tailings" | "core" | "drain" | "foundation";
export interface Piezometer {
  id: string;
  hole: string;
  zone: Zone;
  /** Tip: metres downstream of the crest line, and height. */
  s: number;
  y: number;
  /** Piezometric head as a height (the level water stands at in a pipe from the tip). */
  head: number;
  /** Trigger levels 1 and 2 of the facility's TARP, as heads. */
  l1: number;
  l2: number;
  /** The head the design seepage analysis expects at the tip. */
  design: number;
  /** Minutes since its last reading. */
  age: number;
  stale?: boolean;
}
const READ_AGE = 18;
export const PIEZOMETERS: Piezometer[] = [
  { id: "VWP-01", hole: "BH-A", zone: "tailings", s: -27, y: 24, head: 31.96, l1: 33.2, l2: 34.0, design: 31.6, age: READ_AGE },
  { id: "VWP-02", hole: "BH-A", zone: "foundation", s: -27, y: -6, head: 28.55, l1: 30.5, l2: 32.0, design: 28.0, age: READ_AGE },
  { id: "VWP-03", hole: "BH-B", zone: "core", s: -1, y: 16, head: 24.42, l1: 27.0, l2: 29.0, design: 24.0, age: READ_AGE },
  // Its logger channel stopped reporting nineteen days ago.
  { id: "VWP-04", hole: "BH-B", zone: "foundation", s: -1, y: -7, head: 17.86, l1: 19.5, l2: 21.5, design: 18.2, age: 19 * 1440 + 13 * 60 + 8, stale: true },
  { id: "VWP-05", hole: "BH-C", zone: "drain", s: 40, y: 1.0, head: 1.9, l1: 4.0, l2: 5.5, design: 1.8, age: READ_AGE },
  { id: "VWP-06", hole: "BH-D", zone: "foundation", s: 76, y: -6, head: 1.22, l1: 3.5, l2: 5.0, design: 1.0, age: READ_AGE },
];
/** Pressure at the tip, kPa, from its head. */
export const kPa = (p: Piezometer) => (p.head - p.y) * 9.81;
/** Pressure at a tip for any head, kPa, and back. */
export const headKPa = (p: Piezometer, head: number) => (head - p.y) * 9.81;
export const kPaHead = (p: Piezometer, kpa: number) => p.y + kpa / 9.81;
/** How long since a reading, in words a dashboard would use. */
export const ageText = (min: number) =>
  min < 90 ? `${Math.round(min)} min` : min < 2880 ? `${Math.round(min / 60)} h` : `${Math.floor(min / 1440)} days`;
/** Inclinometers on the downstream berms beside section A–A, read by a manual traverse every fortnight. */
export const INCLINOMETERS = [
  { id: "INC-01", s: 40, x: SECTION_X + 7, bottom: -16, mm: 4.1, rate: 0.2, days: 9 },
  { id: "INC-02", s: 76, x: SECTION_X + 7, bottom: -16, mm: 2.6, rate: 0.1, days: 9 },
];
/** The boreholes the tips hang in: where each starts at the surface. */
export const HOLES: Record<string, number> = { "BH-A": -27, "BH-B": -1, "BH-C": 40, "BH-D": 76 };

/** The phreatic surface along section A–A, as (s, height) through the tailings, the core, the drains and the toe. */
export const PHREATIC: [number, number][] = [
  [-175, 33.1],
  [-120, 32.75],
  [-60, 32.25],
  [-27, 31.96],
  [-12, 31.55],
  [-5.6, 31.2],
  [-1, 24.4],
  [4.6, 14.6],
  [6.2, 7.5],
  [7.6, 2.0],
  [30, 1.95],
  [76, 1.6],
  [104, 1.05],
  [111, -0.6],
];

/** Survey prisms on the crest and the berms: the crest's low point carries P-07. */
export const PRISMS = (() => {
  const list: { id: string; x: number; s: number }[] = [];
  let n = 1;
  for (let x = -280; x <= 280; x += 50) list.push({ id: `P-${String(n++).padStart(2, "0")}`, x: x + (x === 20 ? 0 : 0), s: DAM.crestHalf - 0.9 });
  for (let x = -200; x <= 200; x += 100) list.push({ id: `P-${String(n++).padStart(2, "0")}`, x, s: 7 + DAM.face * DAM.down + 1.5 });
  return list;
})();
export const DATES = {
  crestSurvey: "2 Nov",
  crestSurveyAge: "22 days",
  bathymetry: "14 Oct",
  pondRead: "15:30",
  pondAge: "30 min",
  staleSince: "5 Nov, 02:52",
};

/* ---------- Water ---------- */
/** Seventy-two-hour rainfall for a return period, from the site's (synthetic) rainfall-frequency curve. */
export const rainFor = (years: number) => 50 + 29.5 * Math.log(years);
export const yearsFor = (mm: number) => Math.exp((mm - 50) / 29.5);
/** The probable maximum precipitation over 72 hours, at this synthetic site. */
export const PMP = 480;
export const RUNOFF = { pond: 1, beach: 0.7, hills: 0.3 };
/** Hillside that drains into the facility past the diversion channels. */
export const HILLS_M2 = 1_250_000;

export interface Balance {
  /** Pond area and volume today. */
  area0: number;
  volume0: number;
  /** Tailings beach and pond together. */
  impoundment: number;
  /** Storm inflow for a 72-hour rainfall, m³ (the pumps off). */
  inflow(mm: number): number;
  /** Rise of the pond that stores a volume, m. */
  rise(volume: number): number;
  /** Pond area and volume at a rise. */
  areaAt(rise: number): number;
  volumeAt(rise: number): number;
}

/** The pond's stage–storage curve, counted on a grid over the bed: what level a volume of water stands at. */
export function balance(cell = 4): Balance {
  const beds: number[] = [];
  let impoundment = 0;
  for (let z = -980; z <= -60; z += cell) {
    for (let x = -560; x <= 300; x += cell) {
      if (upstream(x, z) <= 0) continue;
      const t = tailings(x, z);
      const g = ground(x, z);
      const b = Math.max(t, g);
      if (t >= g) impoundment += cell * cell;
      if (b < POND + 4) beds.push(b);
    }
  }
  beds.sort((a, b) => a - b);
  const prefix = new Float64Array(beds.length + 1);
  for (let i = 0; i < beds.length; i++) prefix[i + 1] = prefix[i]! + beds[i]!;
  const below = (level: number) => {
    let lo = 0;
    let hi = beds.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (beds[mid]! < level) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  };
  const vol = (level: number) => {
    const k = below(level);
    return cell * cell * (k * level - prefix[k]!);
  };
  const area = (level: number) => below(level) * cell * cell;
  const v0 = vol(POND);
  const a0 = area(POND);
  return {
    area0: a0,
    volume0: v0,
    impoundment,
    inflow: (mm) => (mm / 1000) * (a0 * RUNOFF.pond + Math.max(0, impoundment - a0) * RUNOFF.beach + HILLS_M2 * RUNOFF.hills),
    rise(volume) {
      if (volume <= 0) return 0;
      let lo = 0;
      let hi = 4;
      for (let i = 0; i < 40; i++) {
        const mid = (lo + hi) / 2;
        if (vol(POND + mid) - v0 < volume) lo = mid;
        else hi = mid;
      }
      return (lo + hi) / 2;
    },
    areaAt: (r) => area(POND + r),
    volumeAt: (r) => vol(POND + r),
  };
}
