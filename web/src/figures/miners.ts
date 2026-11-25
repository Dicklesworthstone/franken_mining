/**
 * Mine workers in PPE on the procedural people of `people.ts`: hard hats (white for supervisors and engineers,
 * yellow for operators, a cap lamp underground), hi-vis coveralls, two-tone shirts or vests over dark workwear with
 * retroreflective tape, safety glasses, gloves, steel-toe boots, an optional half-face respirator, and what the job
 * puts in the hands (a tablet, a clipboard, a radio, a core tray). Faceless and unbranded.
 *
 * A miner is about six draw calls: four skinned meshes (cloth, gear, skin, hair) whose geometry is shared by everyone
 * of the same look and build, one rigid hat mesh on the head bone (shell, glasses, lamp, respirator), and whatever
 * prop the hands hold. Hi-vis, tape and lamp glow are vertex colours with a per-vertex `aux` channel read by the
 * shared materials (see `auxPatch`), so a crew of thirty shares a handful of programs and geometries.
 *
 * Units are metres, y up; the person stands on y = 0 at the group's origin facing +z. Targets passed to `walkTo`,
 * `work` and `face` are points in the group's parent's frame. Nothing here touches the DOM at import time.
 */
import * as THREE from "three";
import {
  AUX_PLAIN,
  BI,
  DEG,
  HAIR_COLOURS,
  Routine,
  SIDES,
  SKIN_TONES,
  Soup,
  Staff,
  V,
  X,
  Y,
  Z,
  addGeo,
  auxPatch,
  blendPose,
  boxSdf,
  buildBoots,
  buildHand,
  buildHead,
  buildNeck,
  buildTop,
  buildTrousers,
  clamp,
  copyPose,
  crouch,
  crouchDeep,
  dirOf,
  headShape,
  lerp,
  loft,
  mat4,
  mj,
  newPose,
  orthoTo,
  paint,
  peopleMaterials,
  qAxis,
  rigid,
  rng,
  setPalm,
  setTool,
  smooth,
  tape,
  topAt,
  trunkW,
  withEdges,
  wrapPi,
  type Aux,
  type Body,
  type BodyPose,
  type CarrySpec,
  type Cohort,
  type Detail,
  type Paint,
  type PartPaint,
  type LimbPaint,
  type RoutineStep,
  type Side,
  type StaffPart,
  type Torso,
  type WorkDef,
  type WorkOpts,
  type Wts,
} from "./people";

/* ------------------------------------------------------------------------------------------------------------ */
/* Public types                                                                                                  */
/* ------------------------------------------------------------------------------------------------------------ */

export type MinerRole = "operator" | "geologist" | "engineer" | "supervisor" | "blaster" | "surveyor" | "underground";
export const MINER_ROLES: readonly MinerRole[] = ["operator", "geologist", "engineer", "supervisor", "blaster", "surveyor", "underground"];

/**
 * What a miner does at a spot. `stand` idles (weight shifts, looking about); `walk` with a target walks there;
 * `point` points at the target; `inspect` bends or squats over it (a core tray, the ground, a bench); `tablet` holds
 * a tablet (or clipboard) and works on it; `carry` carries a core tray; `climb` stands at a ladder's foot, hands on
 * its rails and a foot on the first rung; `radio` talks on a handheld radio; `crouch` squats on the heels.
 * `tablet`, `radio` and `carry` carry on through walks.
 */
export type MinerWork = "stand" | "walk" | "point" | "inspect" | "tablet" | "carry" | "climb" | "radio" | "crouch";

export interface MinerOptions {
  role: MinerRole;
  /** Build: 5th, 50th or 95th percentile row (default from the seed). */
  cohort?: Cohort;
  /** Picks the look's variant, skin tone, hair, and phases idle motion (default 1). */
  seed?: number;
  /** A cap lamp on the hat (default: underground only). */
  lamp?: boolean;
  /** The lamp casts light (a SpotLight per miner: costly, default false). */
  lampLight?: boolean;
  /** A half-face respirator (default: underground only). */
  respirator?: boolean;
  /** Mesh detail: 2 close up (default), 1 mid range, 0 far. */
  detail?: Detail;
  /** Cast shadows (default true). */
  shadows?: boolean;
  /** The hand-held prop `tablet` shows (default by role and seed). */
  board?: "tablet" | "clipboard";
}

export interface MinerCost {
  /** Draw calls for the person as now shown (skinned parts, hat, visible props; shadow passes not counted). */
  drawCalls: number;
  triangles: number;
  bones: number;
}

export interface Miner {
  readonly group: THREE.Group;
  readonly role: MinerRole;
  /** The body underneath (skeleton, IK and the core poses), for advanced use. */
  readonly staff: Staff;
  /** What the miner is doing now: walking or turning on the way, or the current work. */
  readonly activity: "walk" | "turn" | MinerWork;
  /** Advance time by dt seconds and pose. Call once a frame. */
  update(dt: number): void;
  /** Walk through these points (parent frame; y ignored) at `speed` m/s (default 1.1). Queued after a walk under way. */
  walkTo(points: readonly THREE.Vector3[], speed?: number): void;
  /** Start a work at the current spot (walking up to the target first for `inspect` and `climb`). */
  work(kind: MinerWork, target?: THREE.Vector3): void;
  /** Turn on the spot to face a point. */
  face(target: THREE.Vector3): void;
  /** Draw calls, triangles and bones as now shown. */
  cost(): MinerCost;
  dispose(): void;
}

/* ------------------------------------------------------------------------------------------------------------ */
/* Materials                                                                                                     */
/* ------------------------------------------------------------------------------------------------------------ */

export interface MinerMaterials {
  /** Woven workwear and hi-vis, by vertex colour and `aux` (fluorescent dye, retroreflective tape). */
  cloth: THREE.MeshPhysicalMaterial;
  /** Boots, gloves, belts, straps and the lamp cord. */
  gear: THREE.MeshPhysicalMaterial;
  skin(tone: THREE.ColorRepresentation): THREE.MeshPhysicalMaterial;
  hair(color: THREE.ColorRepresentation): THREE.MeshPhysicalMaterial;
  /** Moulded hard-hat plastic, glasses, the lamp and the respirator. */
  hat: THREE.MeshPhysicalMaterial;
  /** Hand-held props: tablet, clipboard, radio, core tray and core. */
  prop: THREE.MeshPhysicalMaterial;
}

let minerMats: MinerMaterials | null = null;

/** The materials every miner shares (made on first use). */
export function minerMaterials(): MinerMaterials {
  if (minerMats) return minerMats;
  const M = peopleMaterials();
  minerMats = {
    cloth: M.cloth,
    gear: M.gear,
    skin: M.skin,
    hair: M.hair,
    hat: auxPatch(
      new THREE.MeshPhysicalMaterial({
        vertexColors: true,
        side: THREE.DoubleSide,
        color: 0xffffff,
        roughness: 0.34,
        metalness: 0,
        clearcoat: 0.55,
        clearcoatRoughness: 0.22,
      }),
    ),
    prop: auxPatch(
      new THREE.MeshPhysicalMaterial({
        vertexColors: true,
        side: THREE.DoubleSide,
        color: 0xffffff,
        roughness: 0.6,
        metalness: 0,
        clearcoat: 0.12,
        clearcoatRoughness: 0.5,
      }),
    ),
  };
  return minerMats;
}

/* ------------------------------------------------------------------------------------------------------------ */
/* Palette                                                                                                       */
/* ------------------------------------------------------------------------------------------------------------ */

const RETRO: Aux = [1, 0, 0, 1];
const FLUO: Aux = [0, 0.7, 0, 1];
const GLOSS: Aux = [0, 0, 0, 0];
const GLOW: Aux = [0, 0, 1, 0];

interface Palette {
  orange: Paint;
  lime: Paint;
  orangeTrim: Paint;
  limeTrim: Paint;
  navy: Paint;
  navyDark: Paint;
  khaki: Paint;
  sky: Paint;
  charcoal: Paint;
  retro: Paint;
  white: Paint;
}
let pal: Palette | null = null;
function P(): Palette {
  if (pal) return pal;
  pal = {
    orange: paint("#f65206", FLUO),
    lime: paint("#b4dc08", FLUO),
    orangeTrim: paint("#d8480a", FLUO),
    limeTrim: paint("#9cc012", FLUO),
    navy: paint("#1b2436"),
    navyDark: paint("#151c2a"),
    khaki: paint("#9c8a66"),
    sky: paint("#89a9c6"),
    charcoal: paint("#2b2d31"),
    retro: paint("#aeb3b8", RETRO),
    white: paint(0xffffff),
  };
  return pal;
}

/* ------------------------------------------------------------------------------------------------------------ */
/* Looks                                                                                                         */
/* ------------------------------------------------------------------------------------------------------------ */

interface Look {
  /** Geometry key: everything below but the skin and hair colours. */
  key: string;
  coverall: boolean;
  /** Top: upper colour above `split` (a height fraction between waist and chest: two-tone), lower below. */
  topUpper: Paint;
  topLower: Paint;
  split: boolean;
  sleeveUpper: Paint;
  sleeveLower: Paint;
  hoops: boolean;
  braces: boolean;
  armBands: 0 | 1 | 2;
  legBands: boolean;
  vest: { body: Paint; trim: Paint } | null;
  trousers: Paint;
  cargo: boolean;
  boots: "lace" | "gum";
  bootUpper: Paint;
  gloves: { back: Paint; palm: Paint; cuff: Paint };
  belt: boolean;
  bun: boolean;
}

interface HatLook {
  key: string;
  shell: Paint;
  lamp: boolean;
  respirator: boolean;
  glasses: "smoke" | "clear";
  stickers: boolean;
  brim: "cap" | "full";
}

function looks(role: MinerRole, seed: number, o: MinerOptions, fem: number): { look: Look; hat: HatLook; tone: string; hair: string; board: "tablet" | "clipboard" } {
  const C = P();
  const r = rng(seed * 4099 + 11);
  const v = Math.floor(r() * 3);
  const pick = <T>(a: readonly T[]) => a[Math.floor(r() * a.length) % a.length]!;
  const tone = pick(SKIN_TONES);
  const hair = pick(HAIR_COLOURS);
  // Leather riggers (pale hide, a blue cuff), hi-vis cut-resistant knit (dark dipped palm), or grey knit.
  const riggers = { back: paint("#d9c293"), palm: paint("#c7ad7b"), cuff: paint("#2f5d9a") };
  const cutGlove = { back: paint("#b4dc08", FLUO), palm: paint("#2b2b2e"), cuff: paint("#b4dc08", FLUO) };
  const greyGlove = { back: paint("#8a8f96"), palm: paint("#2a2b2e"), cuff: paint("#8a8f96") };
  const blackBoot = paint("#1c1a19");
  const brownBoot = paint("#33251a");
  const base: Look = {
    key: "",
    coverall: false,
    topUpper: C.navy,
    topLower: C.navy,
    split: false,
    sleeveUpper: C.navy,
    sleeveLower: C.navy,
    hoops: false,
    braces: false,
    armBands: 0,
    legBands: false,
    vest: null,
    trousers: C.navy,
    cargo: true,
    boots: "lace",
    bootUpper: r() < 0.6 ? blackBoot : brownBoot,
    gloves: riggers,
    belt: false,
    bun: r() < fem * 0.55,
  };
  const white = paint("#efefeb");
  const yellow = paint("#f6c400");
  let hat: Omit<HatLook, "key"> = { shell: yellow, lamp: false, respirator: false, glasses: "smoke", stickers: r() < 0.5, brim: "cap" };
  let look: Look = base;
  let board: "tablet" | "clipboard" = "tablet";
  const coverall = (c: Paint, extra: Partial<Look> = {}): Look => ({
    ...base,
    coverall: true,
    topUpper: c,
    topLower: c,
    sleeveUpper: c,
    sleeveLower: c,
    trousers: c,
    hoops: true,
    braces: true,
    armBands: 1,
    legBands: true,
    ...extra,
  });
  const twoTone = (c: Paint, extra: Partial<Look> = {}): Look => ({
    ...base,
    topUpper: c,
    topLower: C.navy,
    split: true,
    sleeveUpper: c,
    sleeveLower: C.navy,
    hoops: true,
    armBands: 1,
    legBands: true,
    trousers: C.navy,
    ...extra,
  });
  const vest = (shirt: Paint, vc: "orange" | "lime", trousers: Paint, extra: Partial<Look> = {}): Look => ({
    ...base,
    topUpper: shirt,
    topLower: shirt,
    sleeveUpper: shirt,
    sleeveLower: shirt,
    vest: vc === "orange" ? { body: C.orange, trim: C.orangeTrim } : { body: C.lime, trim: C.limeTrim },
    trousers,
    ...extra,
  });
  switch (role) {
    case "operator":
      look = v === 0 ? coverall(C.orange) : v === 1 ? twoTone(C.orange) : coverall(C.lime);
      look.gloves = r() < 0.5 ? riggers : cutGlove;
      break;
    case "geologist":
      look = v === 2 ? vest(C.khaki, "orange", C.navy) : vest(C.navy, "lime", v === 0 ? C.khaki : C.charcoal);
      look.gloves = greyGlove;
      hat = { ...hat, shell: white };
      board = r() < 0.6 ? "tablet" : "clipboard";
      break;
    case "engineer":
      look = vest(v === 1 ? C.navy : C.sky, v === 2 ? "orange" : "lime", C.navy, { legBands: v === 1 });
      look.gloves = greyGlove;
      hat = { ...hat, shell: white, brim: v === 2 ? "full" : "cap" };
      break;
    case "supervisor":
      look = vest(C.navy, v === 1 ? "lime" : "orange", C.charcoal, { cargo: false });
      look.gloves = riggers;
      hat = { ...hat, shell: white, stickers: true };
      board = "clipboard";
      break;
    case "blaster":
      look = twoTone(v === 2 ? C.lime : C.orange);
      look.gloves = riggers;
      break;
    case "surveyor":
      look = v === 0 ? twoTone(C.lime, { armBands: 0 }) : vest(C.navy, "lime", C.navy, { legBands: v === 2 });
      look.gloves = cutGlove;
      hat = { ...hat, shell: white };
      break;
    case "underground":
      look = coverall(C.orange, { armBands: 2, boots: "gum", bootUpper: paint("#161718"), belt: true, gloves: cutGlove });
      hat = { ...hat, shell: v === 2 ? white : yellow, glasses: "clear", stickers: true };
      break;
  }
  hat.lamp = o.lamp ?? role === "underground";
  hat.respirator = o.respirator ?? role === "underground";
  const hex = (p: Paint) => p.c.getHexString();
  look.key = [
    role,
    look.coverall ? "c" : "s",
    hex(look.topUpper),
    hex(look.topLower),
    hex(look.sleeveUpper),
    hex(look.sleeveLower),
    look.split ? 1 : 0,
    look.hoops ? 1 : 0,
    look.braces ? 1 : 0,
    look.armBands,
    look.legBands ? 1 : 0,
    look.vest ? hex(look.vest.body) : "-",
    hex(look.trousers),
    look.cargo ? 1 : 0,
    look.boots,
    hex(look.bootUpper),
    hex(look.gloves.back),
    look.belt ? 1 : 0,
    look.bun ? 1 : 0,
  ].join(":");
  const hatKey = [hex(hat.shell), hat.lamp ? 1 : 0, hat.respirator ? 1 : 0, hat.glasses, hat.stickers ? 1 : 0, hat.brim].join(":");
  return { look, hat: { ...hat, key: hatKey }, tone, hair, board: o.board ?? board };
}

/* ------------------------------------------------------------------------------------------------------------ */
/* Garments: hi-vis tape, vests, straps over the shoulders                                                       */
/* ------------------------------------------------------------------------------------------------------------ */

/** Tape width: 50 mm, the usual retroreflective strip. */
const TAPE = 0.05;

/** Paint by a coordinate with bands of tape at [lo, hi] intervals. */
function banded(base: (u: number) => Paint, bands: readonly (readonly [number, number])[], tape: Paint): (u: number) => Paint {
  return (u) => {
    for (const [a, b] of bands) if (u >= a && u <= b) return tape;
    return base(u);
  };
}

/** The torso's two hoops of tape (waist and chest), heights. */
function hoopBands(B: Body): [number, number][] {
  const s = B.s;
  const lo = B.waistY - 0.03 * s;
  const hi = B.chestY - 0.115 * s;
  return [
    [lo, lo + TAPE],
    [hi, hi + TAPE],
  ];
}

/** Outward surface normal of a top (with `ease`) at height y, angle th. */
function topNormal(B: Body, T: Torso, y: number, th: number, ease: number, out = V()): THREE.Vector3 {
  const e = 0.004;
  const a = topAt(B, T, y, th + e, ease);
  const b = topAt(B, T, y, th - e, ease);
  const c = topAt(B, T, y + e, th, ease);
  const d = topAt(B, T, y - e, th, ease);
  return out.crossVectors(a.sub(b), c.sub(d)).normalize();
}

/** A point `off` out from a top's surface along its true normal (layers stay apart where the surface slopes). */
function onTop(B: Body, T: Torso, y: number, th: number, ease: number, off: number, out = V()): THREE.Vector3 {
  return topAt(B, T, y, th, ease, 0, out).addScaledVector(topNormal(B, T, y, th, ease), off);
}

/**
 * A path over the shoulder at a fixed distance `x` from the midline (signed: + the left): up the front from height
 * yF, over the trapezius where the top's sections narrow to x, and down the back to yB; `lift` off a top with `ease`.
 */
function overShoulder(B: Body, T: Torso, x: number, yF: number, yB: number, ease: number, lift: number, detail: Detail): { pts: THREE.Vector3[]; nrm: THREE.Vector3[] } {
  const s = B.s;
  const y1 = B.shY - 0.035 * s;
  const nV = detail === 2 ? 18 : detail === 1 ? 10 : 5;
  const nO = detail === 2 ? 10 : 6;
  const sg = Math.sign(x) || 1;
  const pts: THREE.Vector3[] = [];
  const nrm: THREE.Vector3[] = [];
  // Off the surface along its true normal: over the top of the shoulder that is mostly up.
  const push = (y: number, th: number) => {
    const n = topNormal(B, T, y, th, ease);
    pts.push(topAt(B, T, y, th, ease).addScaledVector(n, lift));
    nrm.push(n);
  };
  // Height on the narrowing top where the section at angle th reaches x (bisection: sections narrow upward).
  const yAt = (th: number) => {
    let lo = y1;
    let hi = B.neckBaseY;
    for (let k = 0; k < 22; k++) {
      const m = (lo + hi) / 2;
      if (Math.abs(T.at(m, th).x) > Math.abs(x)) lo = m;
      else hi = m;
    }
    return (lo + hi) / 2;
  };
  for (let i = 0; i < nV; i++) {
    const y = lerp(yF, y1, i / nV);
    push(y, T.angleAtX(y, x));
  }
  const th0 = T.angleAtX(y1, x);
  const th1 = sg * Math.PI - th0;
  for (let i = 0; i <= nO; i++) {
    const th = lerp(th0, th1, i / nO);
    push(i === 0 || i === nO ? y1 : Math.max(y1, yAt(th)), th);
  }
  for (let i = 1; i <= nV; i++) {
    const y = lerp(y1, yB, i / nV);
    push(y, sg * Math.PI - T.angleAtX(y, x));
  }
  return { pts, nrm };
}

interface VestSpec {
  body: Paint;
  trim: Paint;
  tape: Paint;
  /** The shirt's ease under it. */
  ease: number;
}

/**
 * A hi-vis vest over a shirt: a shell standing off the shirt from the hip to the chest, its top cut into a V at the
 * front, the armholes and a straight back, with straps over the shoulders, bound edges, a zip, two hoops of tape and
 * a brace of tape up each side over the shoulders.
 */
function buildVest(soup: Soup, B: Body, T: Torso, v: VestSpec, detail: Detail): void {
  const s = B.s;
  const ease = v.ease;
  const lift = 0.0095 * s;
  const n = detail === 2 ? 48 : detail === 1 ? 32 : 16;
  const y0 = B.hemY + 0.03 * s;
  const yFlat = B.chestY - 0.055 * s;
  const xs = 0.1 * s;
  const sw = 0.084 * s;
  const yS = B.armpitY + 0.03 * s;
  const yV = B.chestY - 0.045 * s;
  const yArm = B.armpitY - 0.05 * s;
  const yBack = B.armpitY + 0.045 * s;
  const thOut = Math.abs(T.angleAtX(yS, xs + sw / 2));
  const xIn = xs - sw / 2;
  const topY = (th: number) => {
    const a = Math.abs(wrapPi(th));
    const front = a <= Math.PI / 2;
    const af = front ? a : Math.PI - a;
    if (af >= thOut) {
      // The armhole: steep under the strap, rounding out at the side seam.
      const u = clamp((af - thOut) / (Math.PI / 2 - thOut), 0, 1);
      return yArm + (yS - yArm) * (1 - Math.sin((u * Math.PI) / 2));
    }
    const x = Math.abs(T.at(yS, th).x);
    if (x >= xIn) return yS;
    return front ? lerp(yV, yS, Math.pow(x / xIn, 0.9)) : yBack;
  };
  const hoops = hoopBands(B);
  const nBody = detail === 2 ? 12 : detail === 1 ? 8 : 4;
  let ys: number[] = [];
  for (let i = 0; i <= nBody; i++) ys.push(lerp(y0, yFlat, i / nBody));
  ys = withEdges(ys, hoops.flat(), y0, yFlat);
  const nTop = detail === 2 ? 6 : detail === 1 ? 4 : 2;
  const ths: number[] = [];
  for (let j = 0; j < n; j++) ths.push((j / n) * Math.PI * 2 + Math.PI);
  const rows: THREE.Vector3[][] = [];
  const rowY: ((j: number) => number)[] = [];
  for (const y of ys) rowY.push(() => y);
  for (let k = 1; k <= nTop; k++) rowY.push((j) => lerp(yFlat, topY(ths[j]!), k / nTop));
  for (const fy of rowY) rows.push(ths.map((th, j) => onTop(B, T, fy(j), th, ease, lift)));
  const paintY = banded(() => v.body, hoops, v.tape);
  // A vest rides on the trunk more than on the thighs (it ends above the hips), so a squat does not drag its hem.
  const W = (p: THREE.Vector3) => trunkW(B, p, 0.2);
  loft(soup, rows, { w: (_i, _j, p) => W(p), color: (i) => paintY(i < ys.length ? ys[i]! : yFlat + 1) });

  // Bound edges: the top (neckline, armholes, back) and the hem.
  const edge = (fy: (j: number) => number, up: number) => {
    const pts: THREE.Vector3[] = [];
    const nrm: THREE.Vector3[] = [];
    for (let j = 0; j <= n; j++) {
      const jj = j % n;
      const y = fy(jj) + up;
      pts.push(onTop(B, T, y, ths[jj]!, ease, lift + 0.0012 * s));
      nrm.push(topNormal(B, T, y, ths[jj]!, ease));
    }
    tape(soup, pts, nrm, 0.014 * s, 0.0032 * s, W, v.trim, detail);
  };
  edge(rowY[rowY.length - 1]!, -0.003 * s);
  edge(() => y0, 0.004 * s);
  // The zip.
  if (detail > 0) {
    const pts: THREE.Vector3[] = [];
    const nrm: THREE.Vector3[] = [];
    for (let k = 0; k <= 8; k++) {
      const y = lerp(y0, yV, k / 8);
      pts.push(onTop(B, T, y, 0, ease, lift + 0.0016 * s));
      nrm.push(topNormal(B, T, y, 0, ease));
    }
    tape(soup, pts, nrm, 0.011 * s, 0.003 * s, W, v.trim, detail);
  }
  // Straps over the shoulders, and the braces of tape on them.
  for (const sd of [1, -1]) {
    // Layered by thickness: the strap's top stands under the brace's underside.
    const st = overShoulder(B, T, sd * xs, yS - 0.022 * s, yS - 0.022 * s, ease, lift + 0.0006 * s, detail);
    tape(soup, st.pts, st.nrm, sw, 0.0024 * s, W, v.body, detail);
    const br = overShoulder(B, T, sd * xs, y0 + 0.012 * s, y0 + 0.012 * s, ease, lift + 0.0037 * s, detail);
    tape(soup, br.pts, br.nrm, TAPE * s, 0.0014 * s, W, v.tape, detail);
  }
}

/** A coverall's braces of tape: up the front, over the shoulders and down the back, from hoop to hoop. */
function buildBraces(soup: Soup, B: Body, T: Torso, ease: number, tapeP: Paint, detail: Detail): void {
  const s = B.s;
  const [lo] = hoopBands(B);
  const yF = lo![1] - 0.004 * s;
  for (const sd of [1, -1]) {
    const br = overShoulder(B, T, sd * 0.098 * s, yF, yF, ease, 0.0016 * s, detail);
    tape(soup, br.pts, br.nrm, TAPE * s, 0.0018 * s, (p) => trunkW(B, p), tapeP, detail);
  }
}

/** A plain belt round the top at height y (its centre), 44 mm wide, with a buckle at the front. */
function beltRing(soup: Soup, B: Body, T: Torso, ease: number, y: number, c: Paint, detail: Detail, buckle = false): void {
  const s = B.s;
  const n = detail === 2 ? 32 : detail === 1 ? 20 : 12;
  const rings: THREE.Vector3[][] = [];
  for (const [dy, dl] of [[-0.022, 0.0015], [-0.022, 0.0045], [0.022, 0.0045], [0.022, 0.0015]] as const)
    rings.push(Array.from({ length: n }, (_, j) => topAt(B, T, y + dy * s, (j / n) * Math.PI * 2 + Math.PI, ease, dl * s)));
  loft(soup, rings, { w: (_i, _j, p) => trunkW(B, p), color: c, out: (_i, _j, p) => V(p.x, 0, p.z - 0.01) });
  if (buckle) {
    const nn = topNormal(B, T, y, 0, ease);
    const q = new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(V().crossVectors(Y, nn).normalize(), Y, nn));
    addGeo(soup, new THREE.BoxGeometry(0.052 * s, 0.046 * s, 0.0045 * s), mat4(topAt(B, T, y, 0, ease, 0.0055 * s), q), (p) => trunkW(B, p), paint("#8b8f94", [0, 0, 0, 0.25]));
  }
}

/**
 * A belt over the waist with a cap-lamp battery at the back right, a self-rescuer at the back left, and the lamp's
 * cord from the battery up the back to the nape (the hat carries it on to the lamp).
 */
function buildBelt(soup: Soup, B: Body, T: Torso, ease: number, detail: Detail): void {
  const s = B.s;
  const y = B.waistY - 0.045 * s;
  const n = detail === 2 ? 32 : detail === 1 ? 20 : 12;
  const W = (p: THREE.Vector3) => trunkW(B, p);
  beltRing(soup, B, T, ease, y, paint("#202224"), detail);
  const at = (th: number, out: number, dy = 0) => topAt(B, T, y + dy, th, ease, out);
  const frame = (th: number) => {
    const nn = topNormal(B, T, y, th, ease);
    return new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(V().crossVectors(Y, nn).normalize(), Y, nn));
  };
  // Buckle.
  addGeo(soup, new THREE.BoxGeometry(0.06 * s, 0.05 * s, 0.012 * s), mat4(at(0, 0.012 * s), frame(0)), W, paint("#7d8186", GLOSS));
  // Battery: a rounded block on the belt.
  const thB = Math.PI - 0.62;
  addGeo(soup, new THREE.BoxGeometry(0.15 * s, 0.1 * s, 0.05 * s, 2, 2, 1), mat4(at(thB, 0.034 * s, 0.01 * s), frame(thB)), W, paint("#2c2f33"));
  addGeo(soup, new THREE.BoxGeometry(0.155 * s, 0.018 * s, 0.054 * s), mat4(at(thB, 0.034 * s, 0.05 * s), frame(thB)), W, paint("#e3b21b"));
  // Self-rescuer: a steel canister in a pouch.
  const thR = -(Math.PI - 0.62);
  const seg = detail === 2 ? 16 : 10;
  addGeo(soup, new THREE.CylinderGeometry(0.046 * s, 0.046 * s, 0.17 * s, seg), mat4(at(thR, 0.05 * s, -0.01 * s), frame(thR)), W, paint("#9ba1a6", [0, 0, 0, 0.4]));
  addGeo(soup, new THREE.CylinderGeometry(0.049 * s, 0.049 * s, 0.03 * s, seg), mat4(at(thR, 0.05 * s, 0.07 * s), frame(thR)), W, paint("#e05a12", FLUO));
  addGeo(soup, new THREE.CylinderGeometry(0.049 * s, 0.049 * s, 0.03 * s, seg), mat4(at(thR, 0.05 * s, -0.075 * s), frame(thR)), W, paint("#e05a12", FLUO));
  // The lamp cord: from the battery up the back to the nape, then up to the hat's back (riding with the head).
  const pts: THREE.Vector3[] = [];
  const nrm: THREE.Vector3[] = [];
  const nC = detail === 2 ? 14 : 8;
  for (let k = 0; k <= nC; k++) {
    const u = k / nC;
    const yy = lerp(y + 0.06 * s, B.neckBaseY - 0.01 * s, u);
    const th = lerp(thB + 0.15, Math.PI + 0.12, smooth(0, 0.5, u));
    pts.push(topAt(B, T, yy, th, ease, 0.008 * s));
    nrm.push(topNormal(B, T, yy, th, ease));
  }
  const Hs = headShape(B);
  const top = Hs.c.clone().add(V(-0.02 * s, 0.01 * s, -0.125 * s));
  const mid = B.J.neck!.clone().add(V(-0.012 * s, 0.01 * s, -0.085 * s));
  pts.push(mid, top);
  nrm.push(V(0, 0.2, -1).normalize(), V(0, 0.3, -1).normalize());
  tape(
    soup,
    pts,
    nrm,
    0.009 * s,
    0.009 * s,
    (p) => {
      const h = smooth(B.neckBaseY, B.J.head!.y + 0.03 * s, p.y);
      if (h <= 0) return W(p);
      return [[BI.spine_03!, 1 - h], [BI.head!, h]] as Wts;
    },
    paint("#141516"),
    detail,
  );
}

/* ------------------------------------------------------------------------------------------------------------ */
/* Dressing a body                                                                                              */
/* ------------------------------------------------------------------------------------------------------------ */

interface BodyGeo {
  cloth: THREE.BufferGeometry;
  gear: THREE.BufferGeometry;
  skin: THREE.BufferGeometry;
  hair: THREE.BufferGeometry;
}
const bodyCache = new Map<string, BodyGeo>();
const hatCache = new Map<string, THREE.BufferGeometry>();

function buildBody(B: Body, T: Torso, L: Look, detail: Detail): BodyGeo {
  const s = B.s;
  const C = P();
  const cloth = new Soup();
  const gear = new Soup();
  const skin = new Soup();
  const hair = new Soup();
  const ease = (L.coverall ? 0.009 : 0.006) * s;
  const hoops = L.hoops ? hoopBands(B) : [];
  const [lo] = hoopBands(B);
  const splitY = lo![1];
  const bodyBase = (y: number) => (L.split && y < splitY ? L.topLower : L.topUpper);
  const body: PartPaint = (u) => banded(bodyBase, hoops, C.retro)(u);
  // Sleeve tape: one band above the elbow, and one round the forearm.
  const lu = B.upper;
  const armBands: [number, number][] = [];
  if (L.armBands >= 1) armBands.push([lu - 0.135 * s, lu - 0.135 * s + TAPE * s]);
  if (L.armBands >= 2) armBands.push([lu + 0.09 * s, lu + 0.09 * s + TAPE * s]);
  const sleeveSplit = lu - 0.135 * s + TAPE * s;
  const sleeveBase = (t: number) => (L.split && t > sleeveSplit ? L.sleeveLower : L.sleeveUpper);
  const sleeve: LimbPaint = (t) => banded(sleeveBase, armBands, C.retro)(t);
  // Under a vest the shirt is tucked into a belted waist (and its chest pockets stay out of the vest's way); a
  // coverall's top meets its legs at an elastic waist.
  const tucked = L.vest !== null || L.coverall;
  buildTop(
    cloth,
    B,
    T,
    {
      body,
      sleeve,
      bodyEdges: [...hoops.flat(), ...(L.split ? [splitY] : [])],
      sleeveEdges: [...armBands.flat(), ...(L.split ? [sleeveSplit] : [])],
      placket: L.split ? undefined : paint(L.topUpper.c.clone().multiplyScalar(0.88), L.topUpper.a),
      placketTo: tucked ? B.waistY - 0.01 * s : undefined,
      pockets: L.vest || L.hoops ? undefined : L.topUpper,
      collar: L.split ? L.topUpper : undefined,
      ease,
      hem: tucked ? B.waistY - 0.026 * s : undefined,
    },
    detail,
  );
  if (L.vest) beltRing(cloth, B, T, ease, B.waistY - 0.03 * s, paint("#2a2420"), detail, true);
  else if (L.coverall && !L.belt) beltRing(cloth, B, T, ease, B.waistY - 0.03 * s, paint(L.topLower.c.clone().multiplyScalar(0.9), L.topLower.a), detail, false);
  if (L.braces) buildBraces(cloth, B, T, ease, C.retro, detail);
  if (L.vest) buildVest(cloth, B, T, { body: L.vest.body, trim: L.vest.trim, tape: C.retro, ease }, detail);
  // Trousers (or the coverall's legs): tape twice round below the knee.
  const lt = B.thigh;
  const legBands: [number, number][] = L.legBands
    ? [
        [lt + 0.1 * s, lt + 0.1 * s + TAPE * s],
        [lt + 0.19 * s, lt + 0.19 * s + TAPE * s],
      ]
    : [];
  const gum = L.boots === "gum";
  const sole = B.lift;
  buildTrousers(
    cloth,
    B,
    T,
    {
      seat: () => L.trousers,
      leg: (t) => banded(() => L.trousers, legBands, C.retro)(t),
      legEdges: legBands.flat(),
      hem: gum ? sole + 0.3 * s : sole + 0.075 * s,
      ease: gum ? 1.08 : 1.12,
      cargo: L.cargo && !L.coverall ? paint(L.trousers.c.clone().multiplyScalar(0.92)) : L.coverall ? L.trousers : undefined,
    },
    detail,
  );
  if (L.belt) buildBelt(gear, B, T, ease, detail);
  buildBoots(
    gear,
    B,
    gum
      ? { kind: "gum", upper: L.bootUpper, sole: paint("#0e0f10"), midsole: paint("#2a2b2c"), shaft: 0.36 }
      : { kind: "lace", upper: L.bootUpper, sole: paint("#121213"), midsole: paint("#2e2b28"), laces: paint("#16130f"), toe: paint(L.bootUpper.c.clone().multiplyScalar(0.7)) },
    detail,
  );
  for (const side of SIDES) buildHand(gear, B, side, { paint: L.gloves.back, glove: { thick: 0.0026, palm: L.gloves.palm, cuff: L.gloves.cuff, cuffLen: 0.055 } }, detail);
  buildNeck(skin, B, new THREE.Color(1, 1, 1), detail);
  buildHead(skin, hair, B, { skin: new THREE.Color(1, 1, 1), skinDark: new THREE.Color(0.78, 0.74, 0.72), hair: new THREE.Color(1, 1, 1), bun: L.bun, crown: false }, detail);
  const grime = L.coverall || L.split ? 0.34 : 0.22;
  dust(cloth, B, grime);
  dust(gear, B, grime * 1.3);
  return { cloth: cloth.geometry(), gear: gear.geometry(), skin: skin.geometry(), hair: hair.geometry() };
}

/**
 * Site dust in the vertex colours: heaviest on the boots and trouser hems, fading up the legs, in soft blotches;
 * retroreflective tape keeps more of its face (it is wiped clean more often).
 */
function dust(soup: Soup, B: Body, amount: number): void {
  const s = B.s;
  const dc = new THREE.Color("#8d7058");
  const P = soup.pos;
  const C = soup.col;
  const A = soup.aux;
  for (let i = 0; i < soup.verts; i++) {
    const x = P[3 * i]!;
    const y = P[3 * i + 1]!;
    const z = P[3 * i + 2]!;
    const n = 0.5 + 0.5 * Math.sin(x * 23.1 + z * 17.3 + y * 9.7) * Math.sin(x * 7.3 - y * 13.1 + z * 5.9 + 1.7);
    let k = amount * (0.85 * smooth(0.55 * s, 0.02, y) + 0.3 * n * smooth(1.15 * s, 0.25 * s, y) + 0.08 * n);
    if ((A[4 * i] ?? 0) > 0) k *= 0.45;
    k = clamp(k, 0, 0.8);
    for (let c = 0; c < 3; c++) {
      const j = 3 * i + c;
      C[j] = lerp(C[j]!, c === 0 ? dc.r : c === 1 ? dc.g : dc.b, k);
    }
  }
}

/* ------------------------------------------------------------------------------------------------------------ */
/* The hat: shell, peak, ribs, suspension, cap lamp, stickers, glasses and respirator, rigid on the head bone     */
/* ------------------------------------------------------------------------------------------------------------ */

function buildHat(B: Body, H: HatLook, detail: Detail): THREE.BufferGeometry {
  const s = B.s;
  const Hs = headShape(B);
  const soup = new Soup();
  const W = rigid("head");
  const Wf = () => W;
  const d = V();
  const nLon = detail === 2 ? 48 : detail === 1 ? 30 : 16;
  const nLat = detail === 2 ? 11 : detail === 1 ? 7 : 4;
  const capLat = (lon: number) => {
    let lo = -Math.PI / 2;
    let hi = Math.PI / 2;
    for (let k = 0; k < 24; k++) {
      const m = (lo + hi) / 2;
      dirOf(m, lon, d);
      if (d.dot(Hs.capN) > Hs.capK) hi = m;
      else lo = m;
    }
    return (lo + hi) / 2;
  };
  // The shell's clearance over the head: the suspension holds it about 2 cm off at the band and 3.5 cm at the crown;
  // longer at the back, as hard hats are.
  const gap = (dd: THREE.Vector3) => (0.019 + 0.017 * smooth(0, 0.95, dd.y) + 0.008 * smooth(0.1, -0.9, dd.z) * (1 - smooth(0.3, 0.9, dd.y))) * s;
  const shellAt = (lat: number, lon: number, off = 0, out = V()) => {
    dirOf(lat, lon, d);
    return out.copy(Hs.c).addScaledVector(d, Hs.r(d) + gap(d) + off);
  };
  const shell = H.shell;
  const lons: number[] = [];
  for (let j = 0; j < nLon; j++) lons.push((j / nLon) * Math.PI * 2);
  const lat0 = lons.map((lon) => capLat(lon) - 0.03);
  // Shell.
  const rows: THREE.Vector3[][] = [];
  for (let i = 0; i <= nLat; i++) {
    const u = i / nLat;
    rows.push(lons.map((lon, j) => shellAt(lerp(lat0[j]!, Math.PI / 2, 1 - (1 - u) ** 1.25), lon)));
  }
  loft(soup, rows, { w: () => W, color: shell, out: (_i, _j, p) => p.clone().sub(Hs.c) });
  // Brim: a short lip all round (the rain gutter's edge), drawn out into a peak at the front (or a full brim).
  const full = H.brim === "full";
  const brimRows: THREE.Vector3[][] = [[], [], [], [], []];
  for (let j = 0; j < nLon; j++) {
    const lon = lons[j]!;
    const root = rows[0]![j]!;
    const h = V(root.x - Hs.c.x, 0, root.z - Hs.c.z).normalize();
    const front = Math.cos(lon);
    const peak = full ? 0.045 + 0.02 * smooth(0.3, 1, front) : 0.012 + 0.046 * Math.pow(smooth(0.2, 1, front), 0.85);
    const ext = peak * s;
    const drop = ext * (full ? 0.32 : 0.16 + 0.12 * smooth(0.3, 1, front));
    const th = 0.0045 * s;
    brimRows[0]!.push(root.clone().addScaledVector(h, -0.004 * s).add(V(0, -0.006 * s, 0)));
    brimRows[1]!.push(root.clone().addScaledVector(h, ext).add(V(0, -drop - th, 0)));
    brimRows[2]!.push(root.clone().addScaledVector(h, ext + 0.0025 * s).add(V(0, -drop - th * 0.45, 0)));
    brimRows[3]!.push(root.clone().addScaledVector(h, ext).add(V(0, -drop, 0)));
    brimRows[4]!.push(root.clone().addScaledVector(h, 0.002 * s).add(V(0, 0.003 * s, 0)));
  }
  loft(soup, brimRows, { w: () => W, color: shell, out: (i, j, p) => (i === 2 ? p.clone().sub(Hs.c) : i < 2 ? V(0, -1, 0) : V(0, 1, 0)) });
  // The gutter's raised rim just above the brim.
  if (detail > 0) {
    const pts: THREE.Vector3[] = [];
    const nrm: THREE.Vector3[] = [];
    for (let j = 0; j <= nLon; j++) {
      const jj = j % nLon;
      pts.push(shellAt(lat0[jj]! + 0.075, lons[jj]!, 0.0015 * s));
      nrm.push(dirOf(lat0[jj]! + 0.075, lons[jj]!, V()));
    }
    tape(soup, pts, nrm, 0.009 * s, 0.007 * s, Wf, shell, detail);
  }
  // Ribs: a raised ridge along the crown and one either side.
  const rib = (tilt: number, width: number, thick: number, a0: number, a1: number) => {
    const pts: THREE.Vector3[] = [];
    const nrm: THREE.Vector3[] = [];
    const n = detail === 2 ? 22 : 12;
    const q = new THREE.Quaternion().setFromAxisAngle(Z, tilt);
    for (let k = 0; k <= n; k++) {
      const a = lerp(a0, a1, k / n);
      const dd = V(0, Math.sin(a), Math.cos(a)).applyQuaternion(q).normalize();
      const r = Hs.r(dd) + gap(dd);
      pts.push(Hs.c.clone().addScaledVector(dd, r));
      nrm.push(dd);
    }
    tape(soup, pts, nrm, width * s, thick * s, Wf, shell, detail);
  };
  rib(0, 0.026, 0.017, 0.42, Math.PI - 0.12);
  if (detail > 0) {
    rib(0.36, 0.012, 0.009, 0.62, Math.PI - 0.35);
    rib(-0.36, 0.012, 0.009, 0.62, Math.PI - 0.35);
  }
  // The suspension's headband, dark under the shell.
  {
    const band: THREE.Vector3[][] = [];
    for (const [dl, off] of [[-0.05, 0.004], [0.1, 0.004]] as const) band.push(lons.map((lon, j) => { dirOf(lat0[j]! + dl, lon, d); return Hs.c.clone().addScaledVector(d, Hs.r(d) + off * s); }));
    loft(soup, band, { w: () => W, color: paint("#26282b"), out: (_i, _j, p) => p.clone().sub(Hs.c) });
  }
  // Reflective stickers on the sides and back.
  if (H.stickers) {
    for (const [l0, l1] of [[1.25, 1.85], [-1.85, -1.25], [2.85, 3.43]] as const) {
      const pts: THREE.Vector3[] = [];
      const nrm: THREE.Vector3[] = [];
      for (let k = 0; k <= 6; k++) {
        const lon = lerp(l0, l1, k / 6);
        const la = capLat(lon) + 0.36;
        pts.push(shellAt(la, lon, 0.0006 * s));
        nrm.push(dirOf(la, lon, V()));
      }
      tape(soup, pts, nrm, 0.022 * s, 0.0012 * s, Wf, P().retro, detail);
    }
  }
  // Cap lamp: a bracket on the front of the shell, the housing and its lens, and the cord back over the left side.
  if (H.lamp) {
    const la = lat0[0]! + 0.38;
    const base = shellAt(la, 0);
    const nrm = dirOf(la, 0, V());
    const fwd = V(0, 0.08, 1).normalize();
    const q = new THREE.Quaternion().setFromUnitVectors(Y, fwd);
    const centre = base.clone().addScaledVector(nrm, 0.012 * s).addScaledVector(fwd, 0.018 * s);
    const seg = detail === 2 ? 20 : 12;
    const dark = paint("#24262a");
    addGeo(soup, new THREE.BoxGeometry(0.034 * s, 0.03 * s, 0.03 * s), mat4(base.clone().addScaledVector(nrm, 0.006 * s), new THREE.Quaternion().setFromUnitVectors(Z, nrm)), W, dark);
    addGeo(soup, new THREE.CylinderGeometry(0.03 * s, 0.026 * s, 0.042 * s, seg), mat4(centre, q), W, dark);
    addGeo(soup, new THREE.TorusGeometry(0.027 * s, 0.0035 * s, 6, seg), mat4(centre.clone().addScaledVector(fwd, 0.021 * s), new THREE.Quaternion().setFromUnitVectors(Z, fwd)), W, paint("#6b7076", [0, 0, 0, 0.3]));
    addGeo(soup, new THREE.CircleGeometry(0.024 * s, seg), mat4(centre.clone().addScaledVector(fwd, 0.0212 * s), new THREE.Quaternion().setFromUnitVectors(Z, fwd)), W, paint("#fff2d0", GLOW));
    const pts: THREE.Vector3[] = [];
    const nn: THREE.Vector3[] = [];
    const n = detail === 2 ? 16 : 9;
    for (let k = 0; k <= n; k++) {
      const lon = lerp(0.16, Math.PI - 0.02, k / n);
      const l2 = capLat(lon) + lerp(0.36, 0.1, k / n);
      pts.push(shellAt(l2, lon, 0.005 * s));
      nn.push(dirOf(l2, lon, V()));
    }
    tape(soup, pts, nn, 0.009 * s, 0.009 * s, Wf, paint("#141516"), detail);
  }
  // Safety glasses: a wraparound lens, a brow bar and the arms over the ears.
  {
    const dy = B.eyeY - Hs.c.y;
    const latE = Math.asin(clamp(dy / (0.092 * s), -0.9, 0.9));
    const lensP = H.glasses === "smoke" ? paint("#1f262d", GLOSS) : paint("#66737d", GLOSS);
    const frame = paint("#1a1b1d");
    const nU = detail === 2 ? 26 : detail === 1 ? 16 : 8;
    const nV = detail === 2 ? 4 : 2;
    const half = 1.2;
    const lens: THREE.Vector3[][] = [];
    const off = (lon: number) => (0.017 - 0.006 * smooth(0.5, 1.2, Math.abs(lon))) * s;
    for (let i = 0; i <= nV; i++) {
      const row: THREE.Vector3[] = [];
      for (let j = 0; j <= nU; j++) {
        const lon = lerp(-half, half, j / nU);
        const lo = latE - 0.2 + 0.1 * Math.exp(-((lon / 0.2) ** 2)) + 0.04 * smooth(0.7, 1.2, Math.abs(lon));
        const hi = latE + 0.16;
        const la = lerp(lo, hi, i / nV);
        dirOf(la, lon, d);
        row.push(Hs.c.clone().addScaledVector(d, Hs.r(d) + off(lon)));
      }
      lens.push(row);
    }
    loft(soup, lens, { closed: false, w: () => W, color: lensP, out: (_i, _j, p) => p.clone().sub(Hs.c) });
    const bar: THREE.Vector3[] = [];
    const barN: THREE.Vector3[] = [];
    for (let j = 0; j <= nU; j++) {
      const lon = lerp(-half - 0.04, half + 0.04, j / nU);
      const la = latE + 0.165;
      dirOf(la, lon, d);
      bar.push(Hs.c.clone().addScaledVector(d, Hs.r(d) + off(lon) + 0.001 * s));
      barN.push(d.clone());
    }
    tape(soup, bar, barN, 0.009 * s, 0.005 * s, Wf, frame, detail);
    for (const sg of [1, -1]) {
      const pts: THREE.Vector3[] = [];
      const nn: THREE.Vector3[] = [];
      const path: [number, number, number][] = [
        [half + 0.02, latE + 0.13, 0.011],
        [1.5, latE + 0.16, 0.012],
        [1.78, 0.03, 0.022],
        [2.0, 0.0, 0.016],
        [2.12, -0.2, 0.008],
        [2.15, -0.34, 0.005],
      ];
      for (const [lon, la, o] of path) {
        dirOf(la, sg * lon, d);
        pts.push(Hs.c.clone().addScaledVector(d, Hs.r(d) + o * s));
        nn.push(d.clone());
      }
      tape(soup, pts, nn, 0.007 * s, 0.003 * s, Wf, frame, detail);
    }
  }
  // Half-face respirator: the elastomer body over the nose and mouth, two filter cartridges, a valve and straps.
  if (H.respirator) {
    // Matte elastomer and moulded filter housings (no clearcoat: aux w above 1).
    const body = paint("#4a5463", [0, 0, 0, 3.2]);
    const filt = paint("#b8457c", [0, 0, 0, 2.6]);
    const seg = detail === 2 ? 20 : 12;
    const latM = -0.62;
    dirOf(latM, 0, d);
    const mc = Hs.c.clone().addScaledVector(d, Hs.r(d) - 0.014 * s);
    addGeo(soup, new THREE.SphereGeometry(1, seg, detail === 2 ? 14 : 8), mat4(mc, new THREE.Quaternion().setFromAxisAngle(X, 0.35), V(0.05 * s, 0.056 * s, 0.033 * s)), W, body);
    // The exhalation valve.
    dirOf(latM - 0.12, 0, d);
    const vc = Hs.c.clone().addScaledVector(d, Hs.r(d) + 0.017 * s);
    addGeo(soup, new THREE.CylinderGeometry(0.014 * s, 0.016 * s, 0.01 * s, seg), mat4(vc, new THREE.Quaternion().setFromUnitVectors(Y, d.clone().add(V(0, -0.2, 0)).normalize())), W, paint("#2a2f37", [0, 0, 0, 3]));
    for (const sg of [1, -1]) {
      dirOf(latM + 0.04, sg * 0.72, d);
      const out = d.clone().add(V(0, 0, 0.5)).normalize();
      const cc = Hs.c.clone().addScaledVector(d, Hs.r(d) + 0.016 * s);
      const q = new THREE.Quaternion().setFromUnitVectors(Y, out);
      addGeo(soup, new THREE.CylinderGeometry(0.029 * s, 0.03 * s, 0.016 * s, seg), mat4(cc, q), W, filt);
      addGeo(soup, new THREE.CylinderGeometry(0.023 * s, 0.027 * s, 0.004 * s, seg), mat4(cc.clone().addScaledVector(out, 0.009 * s), q), W, paint("#9a3767", [0, 0, 0, 2.6]));
      // The lower strap round the neck, under the ears.
      const pts: THREE.Vector3[] = [];
      const nn: THREE.Vector3[] = [];
      for (let k = 0; k <= 8; k++) {
        const lon = sg * lerp(0.85, Math.PI, k / 8);
        const la = lerp(-0.62, -0.5, k / 8);
        dirOf(la, lon, d);
        pts.push(Hs.c.clone().addScaledVector(d, Hs.r(d) + 0.003 * s));
        nn.push(d.clone());
      }
      tape(soup, pts, nn, 0.012 * s, 0.002 * s, Wf, paint("#2e3540"), detail);
    }
  }
  const g = soup.geometry(false);
  const J = B.J.head!;
  g.translate(-J.x, -J.y, -J.z);
  g.computeBoundingSphere();
  return g;
}

/* ------------------------------------------------------------------------------------------------------------ */
/* Props                                                                                                         */
/* ------------------------------------------------------------------------------------------------------------ */

/** A core tray: 1 m long, four channels of NQ core, its frame at the centre of its rim, long side along x. */
export const CORE_TRAY: CarrySpec = { length: 1.0, width: 0.3, depth: 0.06, flange: 0.006, taper: 0.03, lip: 0.006, gripHalf: 0.49 };
const TABLET = { w: 0.25, h: 0.17, t: 0.022 };

const propCache = new Map<string, THREE.BufferGeometry>();
function cachedProp(key: string, make: () => THREE.BufferGeometry): THREE.BufferGeometry {
  let g = propCache.get(key);
  if (!g) {
    g = make();
    propCache.set(key, g);
  }
  return g;
}
const RIGID: Wts = [[0, 1]];

function coreTrayGeo(seed: number): THREE.BufferGeometry {
  const K = CORE_TRAY;
  const soup = new Soup();
  const plastic = paint("#26282b");
  const box = (w: number, h: number, dd: number, x: number, y: number, z: number, c: Paint) => addGeo(soup, new THREE.BoxGeometry(w, h, dd), mat4(V(x, y, z)), RIGID, c);
  const wall = 0.006;
  box(K.length, wall, K.width, 0, -K.depth + wall / 2, 0, plastic);
  for (const sz of [1, -1]) box(K.length, K.depth, wall, 0, -K.depth / 2, sz * (K.width / 2 - wall / 2), plastic);
  for (const sx of [1, -1]) box(wall, K.depth, K.width, sx * (K.length / 2 - wall / 2), -K.depth / 2, 0, plastic);
  const nCh = 4;
  const chW = (K.width - wall) / nCh;
  for (let k = 1; k < nCh; k++) box(K.length - 2 * wall, K.depth * 0.8, 0.004, 0, -K.depth * 0.6, -K.width / 2 + wall / 2 + k * chW, plastic);
  // Core: NQ (47.6 mm), broken into pieces, greys and browns, some wet.
  const r = rng(seed * 7 + 3);
  const rocks = ["#5f625b", "#6e6052", "#47484a", "#7d786d", "#584e44"];
  const rad = 0.0238;
  for (let k = 0; k < nCh; k++) {
    const z = -K.width / 2 + wall / 2 + (k + 0.5) * chW;
    let x = -K.length / 2 + wall + 0.004;
    const xEnd = K.length / 2 - wall - 0.004 - (k === nCh - 1 ? 0.18 : 0);
    const base = rocks[Math.floor(r() * rocks.length)]!;
    while (x < xEnd - 0.03) {
      const len = Math.min(xEnd - x, 0.06 + r() * 0.24);
      const c = new THREE.Color(base).offsetHSL(0, (r() - 0.5) * 0.05, (r() - 0.5) * 0.08);
      const wet = r() < 0.4;
      addGeo(soup, new THREE.CylinderGeometry(rad, rad, len - 0.003, 12, 1), mat4(V(x + len / 2, -K.depth + wall + rad, z), new THREE.Quaternion().setFromAxisAngle(Z, Math.PI / 2)), RIGID, paint(c, wet ? [0, 0, 0, 0.35] : AUX_PLAIN));
      x += len;
    }
  }
  return soup.geometry(false);
}

function tabletGeo(): THREE.BufferGeometry {
  const soup = new Soup();
  const { w, h, t } = TABLET;
  addGeo(soup, new THREE.BoxGeometry(w, t, h), mat4(V()), RIGID, paint("#202224"));
  // Rubber bumpers on the corners.
  for (const sx of [1, -1]) for (const sz of [1, -1]) addGeo(soup, new THREE.BoxGeometry(0.035, t + 0.006, 0.035), mat4(V(sx * (w / 2 - 0.012), 0, sz * (h / 2 - 0.012))), RIGID, paint("#e0601a"));
  // The screen: a map-like layout of soft blocks, lit.
  const nx = 8;
  const nz = 5;
  const sw = w - 0.03;
  const sh = h - 0.03;
  const r = rng(77);
  for (let i = 0; i < nx; i++)
    for (let k = 0; k < nz; k++) {
      const c = k === nz - 1 ? "#2e5f8f" : i < 2 ? "#3b4a58" : ["#4f7a5a", "#6c8f62", "#8a9c6b", "#b0a774", "#5d7f8f"][Math.floor(r() * 5)]!;
      addGeo(soup, new THREE.PlaneGeometry(sw / nx - 0.0015, sh / nz - 0.0015).rotateX(-Math.PI / 2), mat4(V(-sw / 2 + (i + 0.5) * (sw / nx), t / 2 + 0.0006, -sh / 2 + (k + 0.5) * (sh / nz))), RIGID, paint(c, [0, 0, 0.16, 0]));
    }
  return soup.geometry(false);
}

function clipboardGeo(): THREE.BufferGeometry {
  const soup = new Soup();
  const { w, h } = { w: 0.23, h: 0.32 };
  addGeo(soup, new THREE.BoxGeometry(w, 0.006, h), mat4(V(0, 0, 0)), RIGID, paint("#5b4a36"));
  addGeo(soup, new THREE.BoxGeometry(w - 0.02, 0.003, h - 0.05), mat4(V(0, 0.0045, -0.012)), RIGID, paint("#f1efe8"));
  for (let k = 0; k < 9; k++) addGeo(soup, new THREE.BoxGeometry(w - 0.06, 0.0002, 0.003), mat4(V(-0.005, 0.0062, -0.11 + k * 0.024)), RIGID, paint("#8f98a8"));
  addGeo(soup, new THREE.BoxGeometry(0.09, 0.012, 0.03), mat4(V(0, 0.008, h / 2 - 0.02)), RIGID, paint("#9aa0a6", [0, 0, 0, 0.3]));
  return soup.geometry(false);
}

/** A handheld radio in the grip frame: body along +z (the antenna end), its front towards +y. */
function radioGeo(): THREE.BufferGeometry {
  const soup = new Soup();
  const body = paint("#1b1c1e");
  addGeo(soup, new THREE.BoxGeometry(0.058, 0.034, 0.125), mat4(V(0, 0, 0.025)), RIGID, body);
  addGeo(soup, new THREE.BoxGeometry(0.05, 0.004, 0.06), mat4(V(0, 0.018, 0.04)), RIGID, paint("#2c2e31"));
  addGeo(soup, new THREE.BoxGeometry(0.03, 0.002, 0.018), mat4(V(0, 0.0175, 0.0)), RIGID, paint("#6c8a6a", [0, 0, 0.35, 0]));
  addGeo(soup, new THREE.CylinderGeometry(0.0055, 0.0075, 0.085, 8), mat4(V(0.016, 0, 0.13), new THREE.Quaternion().setFromAxisAngle(X, Math.PI / 2)), RIGID, body);
  addGeo(soup, new THREE.CylinderGeometry(0.007, 0.007, 0.012, 10), mat4(V(-0.014, 0, 0.093), new THREE.Quaternion().setFromAxisAngle(X, Math.PI / 2)), RIGID, paint("#e0601a"));
  return soup.geometry(false);
}

/** A core tray mesh for a scene (the one miners carry): shared geometry, the shared prop material. */
export function makeCoreTray(seed = 1): THREE.Mesh {
  const m = new THREE.Mesh(cachedProp(`tray:${seed % 4}`, () => coreTrayGeo(seed % 4)), minerMaterials().prop);
  m.castShadow = true;
  m.receiveShadow = true;
  return m;
}

/* ------------------------------------------------------------------------------------------------------------ */
/* Work poses                                                                                                    */
/* ------------------------------------------------------------------------------------------------------------ */

const _hq = new THREE.Quaternion();
const _hv = V();

/** Where the knee of leg i comes for the pose's pelvis and foot (a two-bone fold, the knee forward). */
function kneeAt(st: Staff, p: BodyPose, i: number, out = V()): THREE.Vector3 {
  const B = st.B;
  const side = SIDES[i]!;
  _hq.setFromAxisAngle(Y, p.yaw).multiply(qAxis(X, p.tilt)).multiply(qAxis(Z, p.roll));
  const hip = _hv.subVectors(B.J[`thigh_${side}`]!, B.J.pelvis!).applyQuaternion(_hq).add(p.pelvis).clone();
  const ank = p.feet[i]!.pos;
  const u = V().subVectors(ank, hip);
  const dd = Math.min(u.length(), B.thigh + B.shin - 1e-4);
  u.normalize();
  const a = B.thigh;
  const b = B.shin;
  const along = (a * a - b * b + dd * dd) / (2 * dd);
  const h = Math.sqrt(Math.max(0, a * a - along * along));
  const fwd = orthoTo(V(sideSign(side) * 0.15, 0, 1).normalize(), u);
  return out.copy(hip).addScaledVector(u, along).addScaledVector(fwd, h);
}
const sideSign = (s: Side) => (s === "L" ? 1 : -1);

/** The palm point that puts the straight forefinger's tip at `tip`, fingers along `fingers`, palm facing `facing`. */
function palmForTip(st: Staff, side: Side, tip: THREE.Vector3, fingers: THREE.Vector3, facing: THREE.Vector3, out = V()): THREE.Vector3 {
  const B = st.B;
  const H = B.hands[side];
  const dg = H.digits.index;
  const tipB = dg.base.clone().addScaledVector(dg.dir, dg.len[0] + dg.len[1] + dg.len[2]);
  const rel = tipB.sub(B.J[`hand_${side}`]!.clone().add(H.palmPos));
  const ca = rel.dot(H.da);
  const cn = rel.dot(H.dn);
  const ct = rel.dot(H.dt);
  const hand = Math.sign(H.dt.dot(V().crossVectors(H.da, H.dn))) || 1;
  const third = V().crossVectors(fingers, facing).normalize().multiplyScalar(hand);
  return out.copy(tip).addScaledVector(fingers, -ca).addScaledVector(facing, -cn).addScaledVector(third, -ct);
}

/** A hand on the hip: the palm on the side of the pelvis, fingers forward and down, the elbow out and back. */
function handOnHip(st: Staff, p: BodyPose, i: number): void {
  const B = st.B;
  const s = B.s;
  const sd = i === 0 ? 1 : -1;
  const pos = st.torso.at(B.waistY - 0.035 * s, sd * Math.PI * 0.5).add(V(sd * 0.022 * s, 0, 0.01 * s));
  pos.x += p.pelvis.x;
  pos.z += p.pelvis.z;
  setPalm(p.arms[i]!, pos, V(-sd * 0.15, -0.55, 1).normalize(), V(-sd, 0, 0.1).normalize(), V(sd, -0.05, -0.85).normalize(), "flat", 0.55);
}

const TABLET_SDF = (q: THREE.Vector3) => {
  const dx = Math.abs(q.x) - TABLET.w / 2;
  const dy = Math.abs(q.y) - TABLET.t / 2;
  const dz = Math.abs(q.z) - TABLET.h / 2;
  return Math.hypot(Math.max(dx, 0), Math.max(dy, 0), Math.max(dz, 0)) + Math.min(Math.max(dx, dy, dz), 0);
};
const TRAY_SDF = boxSdf(CORE_TRAY);

/** The miner's own works (the core's `carry` takes the core tray). */
function minerWorks(role: MinerRole): Record<string, WorkDef> {
  const handsOnHips = role === "supervisor" || role === "engineer";
  return {
    idle: {
      pose(st, t, _o, p) {
        // Now and then a longer look about.
        const ph = t + st.phase * 40;
        const look = Math.sin(ph * 0.13) * smooth(0.55, 0.9, Math.sin(ph * 0.071));
        p.gaze.set(2.2 * look, st.B.eyeY - 0.1, 3);
        if (handsOnHips && st.phase > 0.45) {
          handOnHip(st, p, 0);
          handOnHip(st, p, 1);
        }
      },
    },
    point: {
      pose(st, t, o, p) {
        const B = st.B;
        const s = B.s;
        const tgt = o.at ? st.toBody(o.at, V()) : V(1, 1.2, 8);
        const left = tgt.x > 0.3 * Math.abs(tgt.z) && tgt.z > -0.5;
        const side: Side = left ? "L" : "R";
        const i = left ? 0 : 1;
        const sd = left ? 1 : -1;
        const az = Math.atan2(tgt.x, tgt.z);
        p.twist += clamp(az * 0.3, -0.45, 0.45);
        p.yaw += clamp(az * 0.12, -0.15, 0.15);
        const sh = B.J[`upper_arm_${side}`]!.clone();
        const dir = tgt.clone().sub(sh);
        const hz = Math.hypot(dir.x, dir.z);
        const el = clamp(Math.atan2(dir.y, hz), -40 * DEG, 50 * DEG);
        const azS = Math.atan2(dir.x, dir.z);
        dir.set(Math.sin(azS) * Math.cos(el), Math.sin(el), Math.cos(azS) * Math.cos(el));
        // Point, hold with a couple of beats, let the arm fall while talking it over, point again.
        const cyc = (t + st.phase * 9) % 9;
        const up = 1 - smooth(5.2, 6.0, cyc) * (1 - smooth(7.6, 8.5, cyc));
        const beat = 0.02 * s * Math.sin(t * 6) * smooth(0.4, 1.2, cyc) * (1 - smooth(2.4, 3.2, cyc));
        // Nearly straight: the palm point lies a hand's breadth past the wrist.
        const hand = sh.addScaledVector(dir, (B.upper + B.fore) * 0.97 + B.hands[side].palmPos.length() * 0.9 + beat);
        const facing = orthoTo(V(-sd * 0.35, -1, 0).normalize(), dir);
        setPalm(p.arms[i]!, hand, dir, facing, V(sd * 0.75, -0.7, -0.1).normalize(), up > 0.5 ? "point" : "relax", 1);
        p.arms[i]!.ik = mj(up);
        p.clav[i * 2] = (p.clav[i * 2] ?? 0) + 0.05;
        p.clav[i * 2 + 1] = (p.clav[i * 2 + 1] ?? 0) + 0.06;
        if (handsOnHips || st.phase > 0.6) handOnHip(st, p, 1 - i);
        p.gaze.copy(tgt);
      },
    },
    inspect: {
      pose(st, t, o, p) {
        const B = st.B;
        const s = B.s;
        const tgt = o.at ? st.toBody(o.at, V()) : V(0, 0.03, 0.6 * s);
        const ph = t + st.phase * 9;
        const scan = 0.22 * Math.sin(ph * 0.42);
        if (tgt.y < 0.5 * s) {
          crouchDeep(st, p, 2.05 + 0.3 * clamp((0.2 - tgt.y) / 0.2, 0, 1));
          const kL = kneeAt(st, p, 0);
          const kR = kneeAt(st, p, 1);
          // The left hand on the left knee, the right reaching out to the core now and then, else on its knee.
          setPalm(p.arms[0]!, kL.clone().add(V(0.006 * s, 0.045 * s, 0.035 * s)), V(-0.15, -0.35, 1).normalize(), V(0.1, -1, 0.1).normalize(), V(0.9, -0.2, -0.4).normalize(), "flat", 0.5);
          const cyc = ph % 7;
          const reach = smooth(0.4, 1.4, cyc) * (1 - smooth(4.4, 5.4, cyc));
          const rest = kR.clone().add(V(-0.006 * s, 0.045 * s, 0.035 * s));
          const touch = tgt.clone().add(V(scan, 0.03 + 0.015 * Math.sin(ph * 1.7), 0));
          const fingers = V(0, -0.5, 1).normalize();
          const tip = palmForTip(st, "R", touch, fingers, V(0, -0.85, -0.5).normalize());
          const restF = V(0.15, -0.35, 1).normalize();
          const pos = rest.clone().lerp(tip, reach);
          setPalm(p.arms[1]!, pos, restF.lerp(fingers, reach).normalize(), V(-0.1, -1, 0.1).normalize(), V(-0.9, -0.3, -0.4).normalize(), reach > 0.4 ? "point" : "flat", reach > 0.4 ? 1 : 0.5);
          p.gaze.copy(tgt).add(V(scan * 1.2, 0, 0));
        } else {
          const c = clamp((1.25 * s - tgt.y) / (0.7 * s), 0.3, 1.15);
          crouch(st, p, c);
          // The left palm on the near edge, the right forefinger tracing along it.
          const edge = tgt.clone().add(V(0.3 * s, 0.012, -0.12));
          setPalm(p.arms[0]!, edge, V(0, -0.15, 1).normalize(), V(0, -1, 0), V(0.9, -0.4, -0.3).normalize(), "flat", 0.4);
          const fingers = V(0, -0.6, 1).normalize();
          const tipP = tgt.clone().add(V(-0.05 + scan, 0.012, 0.02));
          setPalm(p.arms[1]!, palmForTip(st, "R", tipP, fingers, V(0, -0.8, -0.6).normalize()), fingers, V(0, -0.8, -0.6).normalize(), V(-0.9, -0.4, -0.3).normalize(), "point", 1);
          p.gaze.copy(tipP);
        }
      },
    },
    tablet: {
      walk: true,
      pose(st, t, _o, p) {
        const B = st.B;
        const s = B.s;
        // The tablet held low in front of the chest by the left hand, its screen turned up to the eyes.
        const c = V(0.025 * s, B.chestY - 0.16 * s, 0.3 * s);
        const eye = V(0, B.eyeY, 0.08 * s);
        const n = eye.sub(c).normalize();
        const up = orthoTo(V(0, 0.25, 1).normalize(), n);
        const xa = V().crossVectors(n, up).normalize();
        p.itemOn = true;
        p.itemPos.copy(c);
        p.itemQuat.setFromRotationMatrix(new THREE.Matrix4().makeBasis(xa, n, up));
        const palm = c.clone().addScaledVector(xa, TABLET.w / 2 - 0.045).addScaledVector(n, -(TABLET.t / 2 + 0.02 * s));
        setPalm(p.arms[0]!, palm, xa.clone().negate().addScaledVector(up, 0.25).normalize(), n, V(0.6, -0.8, -0.1).normalize(), "grasp", 1);
        // The right forefinger taps about the screen: hovering, then down and up.
        const ph = t * 0.9 + st.phase * 13;
        const k = Math.floor(ph);
        const f = ph - k;
        const hx = Math.sin(k * 12.9898) * 0.5;
        const hz = Math.sin(k * 78.233) * 0.5;
        const tap = Math.max(0, Math.sin(Math.PI * clamp((f - 0.55) / 0.3, 0, 1)));
        const spot = c.clone().addScaledVector(xa, (-0.05 + 0.13 * hx) * 0.8).addScaledVector(up, 0.09 * hz * 0.8).addScaledVector(n, TABLET.t / 2 + 0.004 + 0.03 * (1 - tap));
        const fingers = n.clone().negate().addScaledVector(up, 0.9).normalize();
        const facing = n.clone().negate().addScaledVector(up, -0.4).normalize();
        setPalm(p.arms[1]!, palmForTip(st, "R", spot, fingers, facing), fingers, facing, V(-0.8, -0.6, -0.2).normalize(), "point", 1);
        p.gaze.copy(c).addScaledVector(xa, 0.04 * hx);
        p.flex += 0.05;
      },
    },
    radio: {
      walk: true,
      pose(st, t, _o, p) {
        const B = st.B;
        const s = B.s;
        // Talk at the mouth, then listen with it lowered a little; the free hand idles (or rests on the hip).
        const ph = (t + st.phase * 11) % 9;
        const talk = smooth(0, 0.8, ph) * (1 - smooth(5, 5.9, ph));
        const mouth = V(-0.055 * s, B.eyeY - 0.085 * s, 0.135 * s);
        const low = V(-0.13 * s, B.chestY - 0.02 * s, 0.17 * s);
        const pos = low.lerp(mouth, talk);
        const axis = V(0.12, 1, 0.12 * (1 - talk)).normalize();
        const face = V(0.45, 0, -1).normalize();
        setTool(p.arms[1]!, pos, axis, orthoTo(face, axis), V(-1, -0.6, -0.3).normalize(), 0.028, "tool", 0.4);
        p.gaze.set(0.5 * Math.sin(t * 0.2 + st.phase * 6), B.eyeY - 0.25 * (1 - talk), 3);
      },
    },
    climb: {
      feet: true,
      pose(st, t, o, p) {
        const B = st.B;
        const s = B.s;
        const tgt = o.at ? st.toBody(o.at, V()) : V(0, 0, 0.34 * s);
        const rail = 0.23;
        const rung = o.surfaceY ?? 0.3;
        // The left foot steps up onto the first rung over the work's first second.
        const u = smooth(0.1, 1.1, t);
        const f = p.feet[0]!;
        const up = V(Math.max(0.1 * s, tgt.x + 0.1 * s), rung + B.ankY, tgt.z - B.mtpFwd - 0.02);
        f.pos.lerp(up, mj(u));
        f.pos.y += 0.1 * Math.sin(Math.PI * u);
        f.quat.setFromAxisAngle(Y, 3 * DEG);
        p.pelvis.z += 0.05 * s * u;
        p.pelvis.x += 0.02 * s * u;
        p.flex += 0.05 * u;
        // Hands on the rails, the left higher, ready to climb.
        for (let i = 0; i < 2; i++) {
          const sd = i === 0 ? 1 : -1;
          const a = p.arms[i]!;
          const hy = B.shY + (i === 0 ? 0.17 : 0.03) * s + 0.008 * Math.sin(t * 0.7 + i);
          a.ik = 1;
          a.grip = "bar";
          a.roll = 0.8;
          a.spinMax = 0;
          a.pos.set(tgt.x + sd * rail, hy, tgt.z);
          a.a.set(0, 1, 0);
          a.b.set(-sd * 0.35, 0, 1).normalize();
          a.pole.set(sd * 0.9, -0.6, -0.4).normalize();
          a.shape = "bar";
          a.close = 1;
          a.radius = 0.019;
        }
        p.gaze.set(tgt.x, B.eyeY + 0.9, tgt.z + 0.3);
      },
    },
    crouch: {
      pose(st, t, _o, p) {
        const B = st.B;
        const s = B.s;
        crouchDeep(st, p, 2.15);
        // Forearms on the thighs near the knees, hands loose between them.
        for (let i = 0; i < 2; i++) {
          const sd = i === 0 ? 1 : -1;
          const k = kneeAt(st, p, i);
          setPalm(p.arms[i]!, k.add(V(-sd * 0.075 * s, -0.04 * s, 0.07 * s)), V(-sd * 0.45, -0.75, 0.45).normalize(), V(-sd, -0.1, -0.2).normalize(), V(sd * 0.3, -0.6, -0.8).normalize(), "relax", 1);
        }
        p.gaze.set(0.8 * Math.sin(t * 0.17 + st.phase * 5), 0.5, 3);
      },
    },
  };
}

/* ------------------------------------------------------------------------------------------------------------ */
/* The miner                                                                                                     */
/* ------------------------------------------------------------------------------------------------------------ */

const FINAL = 1e6;
const FADE = 0.7;
type Prop = "tray" | "tablet" | "radio" | null;
const propOf = (k: MinerWork): Prop => (k === "carry" ? "tray" : k === "tablet" ? "tablet" : k === "radio" ? "radio" : null);
const workKind = (k: MinerWork): string => (k === "stand" ? "idle" : k);
const WALKABLE: ReadonlySet<MinerWork> = new Set(["tablet", "radio", "carry"]);

class MinerImpl implements Miner {
  readonly group: THREE.Group;
  readonly staff: Staff;
  private routine: Routine;
  private rt = 0;
  private clock = 0;
  private moveEnd = 0;
  private mode: { kind: MinerWork; at?: THREE.Vector3 } = { kind: "stand" };
  private prevProp: Prop = null;
  private queue: (() => void)[] = [];
  private fadeFrom = newPose();
  private fadeT = FADE;
  private hat: THREE.Mesh;
  private props: { tray: THREE.Mesh; tablet: THREE.Mesh; radio: THREE.Mesh };
  private lamp: THREE.SpotLight | null = null;

  constructor(readonly role: MinerRole, o: MinerOptions) {
    const seed = o.seed ?? 1;
    const detail: Detail = o.detail ?? 2;
    const cohort: Cohort = o.cohort ?? (["p05", "p50", "p50", "p95", "p50", "p05", "p95"] as const)[Math.floor(rng(seed * 131 + 7)() * 7)]!;
    const fem = cohort === "p05" ? 1 : 0;
    const L = looks(role, seed, o, fem);
    const mats = minerMaterials();
    const sole = L.look.boots === "gum" ? 0.03 : 0.036;
    const dress = (B: Body, T: Torso, d: Detail): StaffPart[] => {
      const key = `${L.look.key}|${cohort}|${d}`;
      let g = bodyCache.get(key);
      if (!g) {
        g = buildBody(B, T, L.look, d);
        bodyCache.set(key, g);
      }
      return [
        { key: "cloth", geometry: g.cloth, material: mats.cloth, shared: true },
        { key: "gear", geometry: g.gear, material: mats.gear, shared: true },
        { key: "skin", geometry: g.skin, material: mats.skin(L.tone), shared: true },
        { key: "hair", geometry: g.hair, material: mats.hair(L.hair), shared: true },
      ];
    };
    const st = (this.staff = new Staff(cohort, detail, { seed, shadows: o.shadows, sole, dress, works: minerWorks(role), carry: CORE_TRAY }));
    st.itemKg = 18;
    this.group = st.group;
    this.group.name = `miner-${role}-${seed}`;
    const B = st.B;
    // The hat, rigid on the head bone.
    const hk = `${L.hat.key}|${cohort}|${detail}`;
    let hg = hatCache.get(hk);
    if (!hg) {
      hg = buildHat(B, L.hat, detail);
      hatCache.set(hk, hg);
    }
    this.hat = new THREE.Mesh(hg, mats.hat);
    this.hat.name = "hat";
    this.hat.castShadow = o.shadows !== false;
    this.hat.receiveShadow = o.shadows !== false;
    st.bones.get("head")!.add(this.hat);
    if (L.hat.lamp && o.lampLight) {
      const Hs = headShape(B);
      const at = Hs.c.clone().sub(B.J.head!).add(V(0, 0.07 * B.s, 0.14 * B.s));
      const l = new THREE.SpotLight(0xfff0d8, 25, 30, 0.42, 0.65, 2);
      l.position.copy(at);
      l.target.position.copy(at).add(V(0, -0.25, 1));
      st.bones.get("head")!.add(l, l.target);
      this.lamp = l;
    }
    // Props.
    const prop = (g: THREE.BufferGeometry, name: string, parent: THREE.Object3D) => {
      const m = new THREE.Mesh(g, mats.prop);
      m.name = name;
      m.castShadow = o.shadows !== false;
      m.receiveShadow = o.shadows !== false;
      m.visible = false;
      parent.add(m);
      return m;
    };
    this.props = {
      tray: prop(cachedProp(`tray:${seed % 4}`, () => coreTrayGeo(seed % 4)), "core-tray", st.item),
      tablet: prop(L.board === "tablet" ? cachedProp("tablet", tabletGeo) : cachedProp("clipboard", clipboardGeo), L.board, st.item),
      radio: prop(cachedProp("radio", radioGeo), "radio", st.bones.get("grip_R")!),
    };
    this.routine = this.plan([], "stand");
    this.routine.apply(0);
  }

  get activity(): "walk" | "turn" | MinerWork {
    if (this.rt < this.moveEnd) {
      const s = this.routine.poseAt(this.rt).state;
      if (s.activity === "walk" || s.activity === "turn") return s.activity;
    }
    return this.mode.kind;
  }
  private get moving(): boolean {
    return this.rt < this.moveEnd;
  }

  /** A new routine from where the miner stands: these moves, then the work held from then on. */
  private plan(moves: RoutineStep[], kind: MinerWork, at?: THREE.Vector3): Routine {
    const st = this.staff;
    const opts: WorkOpts = at ? { at } : {};
    const steps: RoutineStep[] = [...moves, { kind: "work", work: workKind(kind), duration: FINAL, opts, held: true }];
    const r = new Routine(st, st.placement, steps, false, this.clock);
    this.rt = 0;
    this.moveEnd = r.duration - FINAL;
    return r;
  }
  private replan(moves: RoutineStep[], kind: MinerWork, at?: THREE.Vector3): void {
    copyPose(this.fadeFrom, this.staff.current);
    this.prevProp = this.fadeT >= FADE * 0.5 ? propOf(this.mode.kind) : this.prevProp;
    this.fadeT = 0;
    this.mode = { kind, at };
    this.routine = this.plan(moves, kind, at);
  }
  /** Parent-frame point to the routine's floor frame (the group's own height taken off). */
  private local(p: THREE.Vector3): THREE.Vector3 {
    return V(p.x, p.y - this.group.position.y, p.z);
  }
  private yawTo(p: THREE.Vector3): number {
    const pl = this.staff.placement;
    return Math.atan2(p.x - pl.x, p.z - pl.z);
  }

  walkTo(points: readonly THREE.Vector3[], speed = 1.1): void {
    if (!points.length) return;
    this.sync();
    if (this.moving) {
      this.queue.push(() => this.walkTo(points, speed));
      return;
    }
    const keep = WALKABLE.has(this.mode.kind);
    const kind: MinerWork = keep ? this.mode.kind : "stand";
    const lead: RoutineStep[] = !keep && this.mode.kind !== "stand" ? [{ kind: "work", work: "idle", duration: 0.6, held: true }] : [];
    const upper = keep ? workKind(kind) : "free";
    this.replan([...lead, { kind: "walk", path: points.map((p) => [p.x, p.z] as const), speed, upper }], kind, keep ? this.mode.at : undefined);
  }

  work(kind: MinerWork, target?: THREE.Vector3): void {
    this.sync();
    if (this.moving) {
      this.queue.push(() => this.work(kind, target));
      return;
    }
    if (kind === "walk") {
      if (target) this.walkTo([target]);
      return;
    }
    const st = this.staff;
    const at = target ? this.local(target) : undefined;
    const keep = WALKABLE.has(this.mode.kind);
    const upper = keep && kind === this.mode.kind ? workKind(kind) : "free";
    const moves: RoutineStep[] = [];
    if (at && (kind === "inspect" || kind === "climb")) {
      // Walk up to it: stand off a little from a low target, closer to a bench or a ladder's foot.
      const pl = st.placement;
      const standOff = kind === "climb" ? 0.34 : at.y < 0.5 ? 0.6 : 0.42;
      const dx = at.x - pl.x;
      const dz = at.z - pl.z;
      const D = Math.hypot(dx, dz);
      const yaw = Math.atan2(dx, dz);
      if (Math.abs(D - standOff) > 0.14 || D < 1e-3) {
        const ux = D > 1e-3 ? dx / D : Math.sin(pl.yaw);
        const uz = D > 1e-3 ? dz / D : Math.cos(pl.yaw);
        if (this.mode.kind !== "stand" && !keep) moves.push({ kind: "work", work: "idle", duration: 0.6, held: true });
        moves.push({ kind: "walk", path: [[at.x - ux * standOff, at.z - uz * standOff]], speed: 0.9, upper });
        moves.push({ kind: "turn", yaw: Math.atan2(at.x - (at.x - ux * standOff), at.z - (at.z - uz * standOff)), upper });
      } else if (Math.abs(wrapPi(yaw - pl.yaw)) > 0.12) moves.push({ kind: "turn", yaw, upper });
    } else if (at && kind === "point") {
      // Point over the shoulder by turning partly towards it.
      const off = wrapPi(this.yawTo(at) - st.placement.yaw);
      if (Math.abs(off) > 75 * DEG) moves.push({ kind: "turn", yaw: st.placement.yaw + off - Math.sign(off) * 35 * DEG, upper });
    } else if (at && kind !== "stand") {
      const off = wrapPi(this.yawTo(at) - st.placement.yaw);
      if (Math.abs(off) > 0.15) moves.push({ kind: "turn", yaw: st.placement.yaw + off, upper });
    }
    this.replan(moves, kind, at);
  }

  face(target: THREE.Vector3): void {
    this.sync();
    if (this.moving) {
      this.queue.push(() => this.face(target));
      return;
    }
    const keep = WALKABLE.has(this.mode.kind) || this.mode.kind === "stand";
    const kind: MinerWork = keep ? this.mode.kind : "stand";
    const upper = WALKABLE.has(kind) ? workKind(kind) : "free";
    const yaw = this.yawTo(target);
    this.replan([{ kind: "turn", yaw, upper }], kind, keep ? this.mode.at : undefined);
  }

  /** Moved by hand while standing (the group's position or heading set): carry on from the new spot. */
  private sync(): void {
    if (this.moving) return;
    const st = this.staff;
    const pl = st.placement;
    const g = this.group;
    if (Math.abs(g.position.x - pl.x) > 1e-6 || Math.abs(g.position.z - pl.z) > 1e-6 || Math.abs(wrapPi(g.rotation.y - pl.yaw)) > 1e-6) {
      st.place(g.position.x, g.position.z, g.rotation.y);
      // A work bound to a spot (a tray, a ladder, something pointed at) does not follow the move.
      if (this.mode.at && !WALKABLE.has(this.mode.kind)) this.mode = { kind: "stand" };
      this.routine = this.plan([], this.mode.kind, this.mode.at);
    }
  }

  update(dt: number): void {
    const st = this.staff;
    const step = clamp(dt, 0, 0.1);
    this.clock += step;
    this.rt += step;
    this.fadeT += step;
    if (!this.moving) {
      this.sync();
      const next = this.queue.shift();
      if (next) next();
    }
    const r = this.routine.poseAt(this.rt);
    const pose = r.pose;
    const k = this.fadeT < FADE ? mj(this.fadeT / FADE) : 1;
    if (k < 1) blendPose(pose, this.fadeFrom, pose, k);
    const prop = k < 0.5 ? this.prevProp : propOf(this.mode.kind);
    this.props.tray.visible = prop === "tray";
    this.props.tablet.visible = prop === "tablet";
    this.props.radio.visible = prop === "radio";
    st.itemSdf = prop === "tray" ? TRAY_SDF : prop === "tablet" ? TABLET_SDF : null;
    st.pose(pose);
  }

  cost(): MinerCost {
    let calls = 0;
    let tris = 0;
    const count = (m: THREE.Mesh) => {
      if (!m.visible) return;
      calls++;
      const g = m.geometry;
      tris += (g.index ? g.index.count : g.getAttribute("position").count) / 3;
    };
    for (const m of this.staff.meshes) count(m);
    count(this.hat);
    count(this.props.tray);
    count(this.props.tablet);
    count(this.props.radio);
    return { drawCalls: calls, triangles: tris, bones: this.staff.skeleton.bones.length };
  }

  dispose(): void {
    this.lamp?.dispose();
    this.staff.dispose();
    this.queue.length = 0;
  }
}

/** Make a mine worker in PPE for a role (see `MinerOptions`). Geometry and materials are shared across miners. */
export function makeMiner(o: MinerOptions): Miner {
  return new MinerImpl(o.role, o);
}

/** Free the geometry every miner shares (and the shared props). Only once no miner is left. */
export function disposeMinerCaches(): void {
  for (const g of bodyCache.values()) for (const x of [g.cloth, g.gear, g.skin, g.hair]) x.dispose();
  for (const g of hatCache.values()) g.dispose();
  for (const g of propCache.values()) g.dispose();
  bodyCache.clear();
  hatCache.clear();
  propCache.clear();
}
