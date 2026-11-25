/**
 * The synthetic underground mine behind the Underground figure: where everything is (levels, decline, orebody,
 * stopes, headings, crews) and the claims FrankenMining keeps about it. Pure data and small functions, no DOM and no
 * three.js, so the geometry modules and the page share one source. Metres, y up (surface at 0), x east, z south.
 * Nothing here describes a real mine.
 */
import { clamp, mix, smooth } from "./lib";
import { noise2 } from "./stage";

/* ---------- Frame ---------- */

/** Level floors, every 25 m. */
export const LEVELS = [-50, -75, -100, -125, -150, -175] as const;
export type Level = (typeof LEVELS)[number];
/** The cutaway block of ground around the mine. */
export const BOX = { x0: -150, x1: 205, z0: -118, z1: 118, y0: -218 } as const;
/** Rock density used for every tonnage, t/m³ (an estimate the site sets). */
export const DENSITY = 2.8;

/* ---------- Orebody ---------- */

const ORE = { x0: -24, x1: 160, top: -34, bottom: -208 } as const;
const ore = noise2(41);
const oreB = noise2(97);
/** How far the orebody's centre plane has moved north at depth y (it dips north at about 72°). */
export const dipShift = (y: number) => (y + 100) * 0.325;
export const oreMid = (x: number, y: number) => -35 + dipShift(y) + 5.5 * ore.fbm(x * 0.017 + 3.1, y * 0.019 - 1.7, 3);
/** True thickness of the lens at (x, y): about 12 to 22 m, pinching out at its edges. */
export function oreThick(x: number, y: number) {
  const u = (x - ORE.x0) / (ORE.x1 - ORE.x0);
  const v = (y - ORE.bottom) / (ORE.top - ORE.bottom);
  if (u <= 0 || u >= 1 || v <= 0 || v >= 1) return 0;
  const edge = smooth(u / 0.075) * smooth((1 - u) / 0.075) * smooth(v / 0.07) * smooth((1 - v) / 0.1);
  return clamp(16.5 + 16 * oreB.fbm(x * 0.011 - 7, y * 0.014 + 2, 4), 9, 26) * Math.sqrt(edge);
}
/** Footwall (south, +z) and hanging-wall (north) contacts. */
export const zFW = (x: number, y: number) => oreMid(x, y) + oreThick(x, y) / 2;
export const zHW = (x: number, y: number) => oreMid(x, y) - oreThick(x, y) / 2;
export const ORE_RANGE = ORE;

/** The footwall drive on a level runs this far south of the ore. */
export const driveZ = (y: number) => -35 + dipShift(y) + 27;

/* ---------- Stopes ---------- */

export type StopeState = "planned" | "drilled" | "fired" | "filled" | "curing";
export interface Stope {
  id: string;
  /** Extraction (bottom) level, and the drill (top) level 25 m above. */
  level: Level;
  k: number;
  primary: boolean;
  state: StopeState;
  xa: number;
  xb: number;
  /** Grade from the block model, g/t Au (estimated). */
  grade: number;
  /** Paste fill: days since the pour, measured strength in MPa, and the age of that test. */
  fill?: { days: number; mpa: number; test: string };
}
/** Stope k spans 18 m along strike. */
export const stopeX = (k: number): [number, number] => [-6 + 18 * (k - 1), 12 + 18 * (k - 1)];
export const stopeMidX = (k: number) => 3 + 18 * (k - 1);
const sid = (level: number, k: number) => `${k % 2 ? "P" : "S"}${-level}-${String(k).padStart(2, "0")}`;

/** States by block (extraction level) and position 1–8: mined bottom-up, primaries (odd) before secondaries. */
const STATES: Record<number, StopeState[]> = {
  [-75]: ["planned", "planned", "planned", "planned", "planned", "planned", "planned", "planned"],
  [-100]: ["planned", "planned", "fired", "planned", "drilled", "planned", "planned", "planned"],
  [-125]: ["filled", "drilled", "filled", "planned", "curing", "drilled", "filled", "planned"],
  [-150]: ["filled", "filled", "filled", "filled", "filled", "filled", "filled", "planned"],
  [-175]: ["planned", "planned", "planned", "planned", "planned", "planned", "planned", "planned"],
};
const FILLS: Record<string, { days: number; mpa: number; test: string }> = {
  "P125-01": { days: 70, mpa: 1.46, test: "56-day cylinders" },
  "P125-03": { days: 58, mpa: 1.21, test: "56-day cylinders" },
  "P125-05": { days: 9, mpa: 0.58, test: "7-day cylinders" },
  "P125-07": { days: 31, mpa: 1.18, test: "28-day cylinders" },
};
export const STOPES: Stope[] = [];
for (const level of [-75, -100, -125, -150, -175] as Level[]) {
  STATES[level]!.forEach((state, i) => {
    const k = i + 1;
    const [xa, xb] = stopeX(k);
    const id = sid(level, k);
    const g = 4.6 + 0.9 * ore.n(k * 0.71 + 3, level * 0.033) + (k === 2 && level === -125 ? -0.25 : 0);
    STOPES.push({
      id,
      level,
      k,
      primary: k % 2 === 1,
      state,
      xa,
      xb,
      grade: Math.round(g * 100) / 100,
      fill: FILLS[id] ?? (state === "filled" ? { days: 120 + k * 9, mpa: 1.3 + (k % 3) * 0.06, test: "56-day cylinders" } : undefined),
    });
  });
}
export const stope = (id: string) => STOPES.find((s) => s.id === id)!;
/** The stopes the figure tells its story with. */
export const FEATURE = { mucking: "P100-03", curing: "P125-05", refused: "S125-06", draft: "S125-02", neighbour: "P125-07" } as const;
/** Design strength for a 25 m exposed paste wall (MPa), and when the curing fill reaches it. */
export const PASTE = { design: 1.0, reachDays: 15, poured: "15 Nov", cylinders14: "29 Nov", binder: 4.5 };

/* ---------- Decline ---------- */

/** A stadium-shaped spiral decline, one lap (200 m) per level at 1 in 8, so every level access leaves from the same spot. */
export const DECLINE = { cx: -58, cz: 48, a: 20, r: 60 / Math.PI, gradient: 8, portalY: -10, arc0: 80, w: 5.6, h: 5.8 } as const;
const LAP = 4 * DECLINE.a + 2 * Math.PI * DECLINE.r; // 200 m
/** Position and heading on the stadium at arc length `arc` (north straight east, east bend, south straight west, west bend). */
export function stadium(arc: number) {
  const { cx, cz, a, r } = DECLINE;
  const s = ((arc % LAP) + LAP) % LAP;
  const pr = Math.PI * r;
  if (s < a) return { x: cx + s, z: cz - r, dx: 1, dz: 0 };
  if (s < a + pr) {
    const f = (s - a) / r;
    return { x: cx + a + r * Math.sin(f), z: cz - r * Math.cos(f), dx: Math.cos(f), dz: Math.sin(f) };
  }
  if (s < 3 * a + pr) return { x: cx + a - (s - a - pr), z: cz + r, dx: -1, dz: 0 };
  if (s < 3 * a + 2 * pr) {
    const f = (s - 3 * a - pr) / r;
    return { x: cx - a - r * Math.sin(f), z: cz + r * Math.cos(f), dx: -Math.cos(f), dz: -Math.sin(f) };
  }
  return { x: cx - a + (s - 3 * a - 2 * pr), z: cz - r, dx: 1, dz: 0 };
}
/** Distance down the decline from the portal to a level's access, and the decline's floor there. */
export const declineS = (y: number) => (DECLINE.portalY - y) * DECLINE.gradient;
export const declineAt = (s: number) => {
  const p = stadium(DECLINE.arc0 + s);
  return { ...p, y: DECLINE.portalY - s / DECLINE.gradient };
};
/** The decline is being deepened: its face is 42 m past the 175 access. */
export const DECLINE_END = declineS(-175) + 42;
/** Box cut: from the surface down to the portal at 1 in 6, east of the portal. */
export const BOXCUT = { x0: -38, x1: 22, z: 48 + 60 / Math.PI, w: 7.5 } as const;

/* ---------- Raises and fixed places ---------- */

export const SHAFT = { x: -80, z: 14, r: 3.3, bottom: -196 } as const;
export const RAR = { x: 176, z: -12, r: 2.6, bottom: -152 } as const;
export const OREPASS = { x: -30, r: 1.7, top: -75, bottom: -198 } as const;
export const orepassZ = (y: number) => driveZ(y) - 9.5;
export const REFUGE = { level: -125 as Level, x: -44 } as const;
export const SUBSTATION = { level: -125 as Level, x: -16 } as const;
export const PASTE_HOLE = { x: 36, z: 8.25 } as const;
/** The development heading on 175: the drive has reached x = 62 going east. */
export const L175_FACE = 62;

/** Which crosscuts each level has, and how each ends (a face in ore, a brow over an open stope, or a fill barricade). */
export type XcEnd = "face" | "brow" | "barricade";
export const CROSSCUTS: Record<number, [k: number, end: XcEnd][]> = {
  [-50]: [],
  [-75]: [
    [3, "brow"],
    [5, "face"],
    [7, "face"],
  ],
  [-100]: [
    [1, "face"],
    [2, "face"],
    [3, "brow"],
    [5, "face"],
    [6, "face"],
    [7, "face"],
  ],
  [-125]: [
    [1, "barricade"],
    [2, "face"],
    [3, "barricade"],
    [4, "face"],
    [5, "barricade"],
    [6, "face"],
    [7, "barricade"],
    [8, "face"],
  ],
  [-150]: [
    [1, "barricade"],
    [2, "barricade"],
    [3, "barricade"],
    [4, "barricade"],
    [5, "barricade"],
    [6, "barricade"],
    [7, "barricade"],
  ],
  [-175]: [],
};
/** East end of each level's footwall drive (175 is still being developed). */
export const driveEast = (level: number) => (level === -175 ? L175_FACE : 160);
export const DRIVE_WEST = -68;

/* ---------- Ground support: each heading's critical-control state ---------- */

export type Support = "verified" | "due" | "overdue";
export interface Heading {
  id: string;
  name: string;
  /** Tunnel that ends at this heading: a level's crosscut, the 175 drive or the decline. */
  at: { level: number; k?: number } | "decline" | "l175";
  status: Support;
  /** When the support was installed and last verified, on the shift's clock (minutes since midnight, negative = days ago). */
  installed: string;
  checked: string;
  by: string;
  note: string;
}
export const HEADINGS: Heading[] = [
  {
    id: "H-175E",
    name: "175 east drive face",
    at: "l175",
    status: "verified",
    installed: "Bolts and mesh to 2.6 m from the face, 06:05",
    checked: "06:40",
    by: "shift supervisor",
    note: "Jumbo drilling the next cut",
  },
  {
    id: "H-DEC",
    name: "Decline face, below 175",
    at: "decline",
    status: "due",
    installed: "Bolts and mesh 08:10",
    checked: "not yet",
    by: "geotechnical engineer, now",
    note: "Verification window closes 11:00",
  },
  { id: "H-100-07", name: "100 XC-07 face", at: { level: -100, k: 7 }, status: "verified", installed: "23 Nov", checked: "07:55", by: "shift supervisor", note: "Stope access, in ore" },
  { id: "H-100-01", name: "100 XC-01 face", at: { level: -100, k: 1 }, status: "verified", installed: "21 Nov", checked: "07:20", by: "shift supervisor", note: "Stope access, in ore" },
  { id: "H-075-05", name: "075 XC-05 drill drive", at: { level: -75, k: 5 }, status: "due", installed: "22 Nov", checked: "yesterday 10:52", by: "shift supervisor", note: "24 h window closes 10:52" },
  { id: "H-075-07", name: "075 XC-07 face", at: { level: -75, k: 7 }, status: "verified", installed: "20 Nov", checked: "08:05", by: "geotechnical engineer", note: "Stope access, in ore" },
  {
    id: "H-125-08",
    name: "125 XC-08 face",
    at: { level: -125, k: 8 },
    status: "overdue",
    installed: "19 Nov",
    checked: "21 Nov 09:30",
    by: "shift supervisor",
    note: "72 h window passed 9 h ago · barricaded, no entry",
  },
  { id: "H-125-04", name: "125 XC-04 face", at: { level: -125, k: 4 }, status: "verified", installed: "18 Nov", checked: "08:30", by: "shift supervisor", note: "Stope access, in ore" },
];

/* ---------- People: every tag underground ---------- */

export type Role = "operator" | "underground" | "engineer" | "surveyor" | "supervisor" | "blaster";
export interface Person {
  tag: string;
  role: Role;
  job: string;
  /** Where they stand (floor point) or the machine they are in. */
  at: { level: number; x: number; z: number; face?: [number, number]; work?: "stand" | "tablet" | "radio" | "inspect" | "point" | "crouch"; target?: [number, number, number] } | "lhd" | "jumbo";
  read: number;
}
export interface Crew {
  id: string;
  name: string;
  place: string;
  people: Person[];
}
const L100 = driveZ(-100);
const L125 = driveZ(-125);
const L175 = driveZ(-175);
export const CREWS: Crew[] = [
  { id: "prod", name: "Production", place: "100 level, LHD 04", people: [{ tag: "T-0412", role: "operator", job: "LHD operator", at: "lhd", read: 38 }] },
  {
    id: "dev",
    name: "Development",
    place: "175 east face",
    people: [
      { tag: "T-0388", role: "operator", job: "Jumbo operator", at: "jumbo", read: 41 },
      { tag: "T-0233", role: "underground", job: "Offsider", at: { level: -175, x: L175_FACE - 24, z: L175 + 1.9, face: [L175_FACE, L175], work: "radio" }, read: 41 },
    ],
  },
  {
    id: "geo",
    name: "Geotechnical",
    place: "Decline face",
    people: [{ tag: "T-0157", role: "engineer", job: "Geotechnical engineer", at: { level: 0, x: 0, z: 0, work: "point" }, read: 52 }],
  },
  {
    id: "survey",
    name: "Survey",
    place: "075 XC-03, stope brow",
    people: [{ tag: "T-0290", role: "surveyor", job: "Mine surveyor (stope scan)", at: { level: -75, x: stopeMidX(3) - 1.2, z: 0, work: "tablet" }, read: 64 }],
  },
  {
    id: "paste",
    name: "Paste fill",
    place: "125 XC-05, fill barricade",
    people: [
      { tag: "T-0501", role: "underground", job: "Fill technician", at: { level: -125, x: stopeMidX(5) + 1.1, z: 0, work: "crouch" }, read: 29 },
      { tag: "T-0517", role: "underground", job: "Fill technician", at: { level: -125, x: stopeMidX(5) - 1.2, z: 0, work: "tablet" }, read: 29 },
    ],
  },
  {
    id: "super",
    name: "Supervision",
    place: "100 level access",
    people: [{ tag: "T-0102", role: "supervisor", job: "Shift supervisor", at: { level: -100, x: -50, z: L100 + 2.3, face: [-30, L100], work: "tablet" }, read: 17 }],
  },
  {
    id: "services",
    name: "Services",
    place: "050 level, east drive",
    people: [
      { tag: "T-0344", role: "underground", job: "Services fitter", at: { level: -50, x: 101, z: driveZ(-50) - 1.4, face: [110, driveZ(-50)], work: "point" }, read: 88 },
      { tag: "T-0351", role: "underground", job: "Services fitter", at: { level: -50, x: 104, z: driveZ(-50) + 1.6, face: [101, driveZ(-50)], work: "radio" }, read: 88 },
    ],
  },
  {
    id: "elec",
    name: "Electrical",
    place: "125 substation",
    people: [{ tag: "T-0620", role: "engineer", job: "Electrician", at: { level: -125, x: SUBSTATION.x, z: L125 + 6.2, work: "inspect" }, read: 73 }],
  },
];
export const TAGS = CREWS.reduce((n, c) => n + c.people.length, 0);
/** The shift's register: who badged in for this shift underground. */
export const REGISTER = TAGS;
export const CLOCK = "10:40";

/* ---------- Paste fill: strength gain with cure time ---------- */

/**
 * Unconfined strength of the curing fill at `days`, MPa: the hyperbolic gain UCS(t) = U·t / (a + t), fitted through
 * the 7-day cylinders (0.58 MPa) and the mix design's 1.00 MPa at day 15. Illustrative: the page fits this curve;
 * FrankenMining keeps the cylinder breaks as measured claims and the curve as an estimate.
 */
export const CURE = (() => {
  const t1 = 7;
  const u1 = 0.58;
  const t2 = PASTE.reachDays;
  const u2 = PASTE.design;
  const a = (u2 * t2 * t1 - u1 * t1 * t2) / (u1 * t2 - u2 * t1);
  const U = (u1 * (a + t1)) / t1;
  return { a, U, at: (days: number) => (U * days) / (a + days), cylinders: [{ days: 7, mpa: 0.58 }] as const };
})();

/* ---------- Ventilation: the airflow each place needs for its diesels, and what the anemometers read ---------- */

/** The site's diesel rule: 0.06 m³/s of fresh air for every kW of diesel engine rating working in a place. */
export const AIR_PER_KW = 0.06;
/** Floor airflow for any level with people on it, m³/s (an illustrative site rule). */
export const AIR_FLOOR = 6;
/** Open cross-section of a 5.5 × 5.5 m arched drive, m², and of the 1.4 m vent duct. */
export const DRIVE_AREA = 5.5 * 5.5 * 0.93;
export const DUCT = { diameter: 1.4, area: Math.PI * 0.7 * 0.7 } as const;
export interface LevelAir {
  level: Level;
  /** The diesels working on the level now, by engine rating. */
  diesel: { name: string; kw: number }[];
  /** Anemometer traverse at the level's access, m³/s (measured). */
  measured: number;
  /** A forcing fan and duct to a dead-end heading, if one runs. */
  duct?: { to: string; measured: number };
}
/** Fresh air comes down the decline and the shaft, crosses each level west to east, and leaves up the return raise. */
export const AIR: LevelAir[] = [
  { level: -50, diesel: [{ name: "Services pickup", kw: 110 }], measured: 8.1 },
  { level: -75, diesel: [], measured: 6.4 },
  { level: -100, diesel: [{ name: "LHD 04", kw: 280 }], measured: 19.6, duct: { to: "XC-03 draw point", measured: 9.8 } },
  { level: -125, diesel: [{ name: "Charge-up vehicle (parked)", kw: 0 }], measured: 7.2 },
  { level: -150, diesel: [], measured: 6.1 },
  { level: -175, diesel: [{ name: "Jumbo 02 tramming engine (off while drilling)", kw: 0 }, { name: "Shotcrete sprayer", kw: 120 }], measured: 12.4, duct: { to: "175 east face", measured: 8.6 } },
];
export const airNeed = (l: LevelAir) => Math.max(AIR_FLOOR, l.diesel.reduce((n, d) => n + d.kw, 0) * AIR_PER_KW);
/** Main fan on the return raise collar, m³/s measured, and its fan pressure, kPa. */
export const FAN = { measured: 68.5, kPa: 1.9, kw: 220 } as const;

/* ---------- Ground instruments ---------- */

export interface Instrument {
  id: string;
  kind: "extensometer" | "convergence";
  level: Level;
  x: number;
  /** Which tunnel it is in: a crosscut k or the footwall drive. */
  k?: number;
  /** Rate now and total since installed, mm; the trigger rate the geotechnical engineer set, mm/day. */
  rate: number;
  total: number;
  trigger: number;
  where: string;
}
export const INSTRUMENTS: Instrument[] = [
  { id: "EXT-125-05", kind: "extensometer", level: -125, x: stopeMidX(5), k: 5, rate: 0.6, total: 7.4, trigger: 1.0, where: "125 XC-05 brow, beside the curing fill" },
  { id: "EXT-100-03", kind: "extensometer", level: -100, x: stopeMidX(3), k: 3, rate: 0.3, total: 11.2, trigger: 1.0, where: "100 XC-03 brow, over the draw point" },
  { id: "CNV-175-E", kind: "convergence", level: -175, x: L175_FACE - 16, rate: 0.9, total: 4.1, trigger: 1.5, where: "175 east drive, 16 m from the face" },
  { id: "CNV-125-08", kind: "convergence", level: -125, x: stopeMidX(8), k: 8, rate: 1.7, total: 9.6, trigger: 1.5, where: "125 XC-08, barricaded heading" },
];

/* ---------- Agent: the refused blast plan and the re-sequenced draft ---------- */

export const PRICE = { usdPerOz: 2600, recovery: 0.92 } as const;
export const ozOf = (t: number, g: number) => (t * g) / 31.1035;
export const usdOf = (t: number, g: number) => ozOf(t, g) * PRICE.recovery * PRICE.usdPerOz;

/** A lighter or darker version of a hex colour, for quick tints. */
export const shade = (hex: string, k: number) => {
  const n = parseInt(hex.slice(1), 16);
  const c = (v: number) => Math.round(clamp(mix(v, k > 0 ? 255 : 0, Math.abs(k)), 0, 255));
  return `#${((c(n >> 16) << 16) | (c((n >> 8) & 255) << 8) | c(n & 255)).toString(16).padStart(6, "0")}`;
};
