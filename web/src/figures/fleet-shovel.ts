/**
 * Loading tools: the electric rope shovel (4100-class), the hydraulic face shovel (600 t class) and the
 * trailing power cable that feeds a rope shovel. Generic colours, no lettering.
 *
 * Rope shovel envelope follows the class: crawlers ≈ 11 m long, ≈ 12.6 m over the shoes, house
 * ≈ 11 × 9 m on a deck ≈ 4.3 m up, boom ≈ 20.4 m at 45° (point sheaves ≈ 19.3 m up), gantry ≈ 17 m,
 * ≈ 60 m³ dipper dumping ≈ 11–13 m high. Face shovel: crawler ≈ 8.6 m wide, cab eye ≈ 8.5 m, boom
 * ≈ 8.8 m, stick ≈ 4.6 m, ≈ 34 m³ bottom-dump bucket.
 * Frame: metres, y up, +z forward (the boom at swing 0), origin on the ground under the swing centre.
 */
import * as THREE from "three";
import {
  bakeFree,
  bar,
  beacon,
  beam,
  bottle,
  box,
  clamp01,
  cyl,
  deck,
  grime,
  instance,
  ladder,
  lamp,
  lenses,
  makeTrack,
  mats,
  mirror,
  node,
  paint,
  plate,
  prism,
  rail,
  rng,
  sphere,
  stairs,
  tube,
  v3,
  COLOURS,
  type Lenses,
  type Track,
  type V3,
} from "./fleet-kit";

const deg = THREE.MathUtils.degToRad;
const lerp = THREE.MathUtils.lerp;
const Y = new THREE.Vector3(0, 1, 0);

// ───────────────────────────── local materials ─────────────────────────────

const local = new Map<string, THREE.Material>();
function lm<M extends THREE.Material>(key: string, make: () => M): M {
  const hit = local.get(key);
  if (hit) return hit as M;
  const m = make();
  m.name = key;
  local.set(key, m);
  return m;
}
const ropeMat = () => lm("shovel:rope", () => new THREE.MeshStandardMaterial({ color: "#3a3833", metalness: 0.8, roughness: 0.48 }));
const cableMat = () =>
  lm("shovel:cable", () =>
    grime(new THREE.MeshStandardMaterial({ color: "#d9531a", roughness: 0.72, metalness: 0 }), {
      dust: "#b29d7f",
      mud: "#5e4d3c",
      scale: 0.7,
      variation: 0.6,
    }),
  );
const dipperMat = () => paint("#3b3a37", { rough: 0.7, coat: 0.05, mudAmount: 1, dustAmount: 1.2 });
const wearSteel = () =>
  lm("shovel:wearsteel", () =>
    grime(new THREE.MeshStandardMaterial({ color: "#6f6c66", metalness: 0.8, roughness: 0.38 }), { mudAmount: 0.6, dustAmount: 1 }),
  );

function swapLenses(root: THREE.Object3D, from: Lenses, to: Lenses) {
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh) return;
    if (m.material === from.white) m.material = to.white;
    else if (m.material === from.amber) m.material = to.amber;
    else if (m.material === from.red) m.material = to.red;
  });
}

/** Louvred panel on a wall facing +z of its local frame (rotate with ry), centred at (x, y, z). */
function louvre(p: THREE.Object3D, frame: THREE.Material, slat: THREE.Material, w: number, h: number, x: number, y: number, z: number, ry = 0) {
  const g = node(p, x, y, z, { ry });
  box(g, frame, w, h, 0.08, 0, 0, 0, { r: 0.02 });
  const n = Math.max(3, Math.floor(h / 0.14));
  for (let i = 0; i < n; i++) box(g, slat, w - 0.12, 0.035, 0.1, 0, -h / 2 + 0.08 + (i * (h - 0.16)) / (n - 1), 0.06, { rx: 0.5, r: 0.005 });
}

/** Door outline with a handle on a wall facing +z of its local frame. */
function door(p: THREE.Object3D, mat: THREE.Material, handle: THREE.Material, x: number, y: number, z: number, ry = 0, w = 0.9, h = 2.0) {
  const g = node(p, x, y, z, { ry });
  box(g, mat, w, h, 0.06, 0, h / 2, 0, { r: 0.02 });
  box(g, handle, 0.04, 0.22, 0.06, w * 0.36, h * 0.5, 0.05, { r: 0.01 });
  for (const yy of [0.25, h - 0.25]) box(g, handle, 0.06, 0.14, 0.06, -w * 0.47, yy, 0.04, { r: 0.01 });
}

// A two-part hydraulic ram (barrel from pin A toward B, rod from pin B toward A), updated per frame.
interface Ram {
  a: THREE.Object3D;
  b: THREE.Object3D;
  barrel: THREE.Object3D;
  rod: THREE.Object3D;
}
const _A = new THREE.Vector3();
const _B = new THREE.Vector3();
const _D = new THREE.Vector3();
function placeRam(r: Ram, frame: THREE.Object3D) {
  r.a.getWorldPosition(_A);
  r.b.getWorldPosition(_B);
  frame.worldToLocal(_A);
  frame.worldToLocal(_B);
  _D.subVectors(_B, _A);
  if (_D.lengthSq() < 1e-8) return;
  _D.normalize();
  r.barrel.position.copy(_A);
  r.barrel.quaternion.setFromUnitVectors(Y, _D);
  r.rod.position.copy(_B);
  r.rod.quaternion.setFromUnitVectors(Y, _D.negate());
}
function ramParts(body: THREE.Material, r: number, lb: number, lr: number) {
  const b = new THREE.Group();
  cyl(b, body, r, lb, 0, lb / 2 + r * 0.6, 0, { seg: 16 });
  cyl(b, body, r * 1.12, 0.16, 0, lb + r * 0.6 - 0.08, 0, { seg: 16 });
  cyl(b, body, r * 0.75, r * 2.2, 0, 0, 0, { axis: "x", seg: 14 });
  box(b, body, r * 1.4, r * 1.3, r * 1.2, 0, r * 0.45, 0, { r: 0.03 });
  bar(b, mats.hose(), v3(r * 0.9, r * 1.5, 0), v3(r * 0.9, lb * 0.85, 0), 0.035, 6);
  const rd = new THREE.Group();
  cyl(rd, mats.chrome(), r * 0.62, lr, 0, lr / 2 + r * 0.5, 0, { seg: 14 });
  cyl(rd, body, r * 0.7, r * 2.0, 0, 0, 0, { axis: "x", seg: 14 });
  box(rd, body, r * 1.2, r * 1.1, r * 1.1, 0, r * 0.35, 0, { r: 0.03 });
  return {
    barrel: bakeFree(b, { mudTop: 2, mudBottom: -2, mud: 0.4 }),
    rod: bakeFree(rd, { mudTop: 2, mudBottom: -2, mud: 0.2 }),
  };
}

// ───────────────────────────── rope shovel ─────────────────────────────

/** Key dimensions of the rope shovel, metres (house frame unless noted). */
export const ROPE_SHOVEL = {
  trackX: 4.9,
  track: { length: 8.6, sprocketR: 1.1, idlerR: 1.0, shoeW: 2.8, pitch: 0.66 },
  ringY: 3.3,
  deckY: 4.3,
  foot: { y: 4.9, z: 4.4 },
  boomLen: 20.4,
  boomAng: deg(45),
  shaftS: 9.2,
  handleLen: 12.5,
  handleX: 1.45,
  crowd: [5.4, 11.4] as const,
  hoistY: [4.0, 13.6] as const,
  sheaveR: 1.75,
  apex: { y: 17.0, z: -2.6 },
} as const;

const RS = ROPE_SHOVEL;
const boomDir = new THREE.Vector2(Math.cos(RS.boomAng), Math.sin(RS.boomAng)); // (z, y)
const atBoom = (s: number, off = 0) =>
  // point s metres along the boom axis from the foot pin, `off` toward the boom's top face, as (z, y)
  new THREE.Vector2(RS.foot.z + boomDir.x * s - boomDir.y * off, RS.foot.y + boomDir.y * s + boomDir.x * off);
const SHAFT = atBoom(RS.shaftS);
const POINT = atBoom(RS.boomLen);

export interface RopeShovelOptions {
  /** House colour (default cream). */
  colour?: string;
  seed?: number;
}

export interface RopeShovelState {
  /** House rotation about the swing centre, rad (positive turns the boom toward +x). */
  swing: number;
  /** Dipper handle extension 0..1. */
  crowd: number;
  /** Dipper height 0..1: 0 at the toe of the face, 1 high enough to dump over an ultra-class truck. */
  hoist: number;
  /** Trip door 0 closed .. 1 fully open. */
  door: number;
  lights: boolean;
  /** Crawler travel, m. */
  distance: number;
}

export interface RopeShovel {
  group: THREE.Group;
  state: RopeShovelState;
  set(s: Partial<RopeShovelState>): void;
  update(dt: number): void;
  lenses: Lenses;
  anchors: {
    /** Centre of the dipper (follows hoist, crowd and swing). */
    dipper: THREE.Object3D;
    /** Where the trailing cable enters the lower works, at ground level behind the carbody. */
    cable: THREE.Object3D;
    /** The rotating upper works. */
    house: THREE.Object3D;
  };
}

interface RopeTemplate {
  lower: THREE.Group;
  house: THREE.Group;
  saddle: THREE.Group;
  slide: THREE.Group;
  door: THREE.Group;
  bail: THREE.Group;
  lens: Lenses;
}
const ropeTemplates = new Map<string, RopeTemplate>();

/** Dipper geometry in the handle-slide frame: z along the handle toward the dipper, y the dipper's mouth side. */
const DIP = { x: 2.7, yBot: -2.2, yTop: 2.3, yBack: 1.7, zBack: 0.25, zFront: 3.55, chamfer: 0.35 } as const;
const BAIL_PIN = v3(0, 1.15, 2.7);
const BAIL_LEN = 3.6;

function buildRopeTemplate(colour: string): RopeTemplate {
  const H = paint(colour, { rough: 0.5, coat: 0.35 });
  const D = mats.dark();
  const K = mats.black();
  const S = mats.steel();
  const YL = mats.yellow();
  const G = mats.glass();
  const GR = mats.grate();
  const RED = mats.red();
  const W = wearSteel();
  const lens = lenses();

  // ───────── lower works (static; tracks are added per instance) ─────────
  const lo = new THREE.Group();
  box(lo, D, 6.8, 1.9, 6.6, 0, 2.2, 0, { r: 0.18, seg: 2 });
  for (const s of [-1, 1]) {
    // side frames joining the carbody to the crawler frames
    box(lo, D, 1.6, 1.2, 4.6, s * 3.9, 2.0, 0, { r: 0.1 });
    // propel motor and gearcase at the rear of each crawler, inboard
    box(lo, D, 1.2, 1.4, 1.6, s * 3.2, 1.7, -4.3, { r: 0.14 });
    cyl(lo, D, 0.55, 1.1, s * 2.75, 2.0, -4.3, { axis: "x", seg: 20 });
    cyl(lo, S, 0.28, 0.3, s * 2.12, 2.0, -4.3, { axis: "x", seg: 14 });
    box(lo, D, 0.9, 0.9, 1.3, s * 2.8, 2.2, -3.0, { r: 0.08 });
    // crawler frame top cover plates
    box(lo, D, 1.0, 0.2, 7.2, s * (RS.trackX - 1.55), 2.62, 0.3, { r: 0.04 });
  }
  // swing circle and roller path
  cyl(lo, D, 4.4, 0.25, 0, 3.24, 0, { seg: 48 });
  cyl(lo, S, 4.2, 0.12, 0, 3.4, 0, { seg: 48 });
  // centre post, collector ring and the cable entry at the rear
  cyl(lo, D, 0.9, 0.4, 0, 3.4, 0, { seg: 20 });
  box(lo, D, 1.6, 1.0, 0.9, 0, 1.75, -3.6, { r: 0.08 });
  cyl(lo, K, 0.16, 0.6, 0, 1.55, -4.2, { axis: "z", seg: 12 });
  // tie-down lugs and steps on the carbody
  for (const z of [-2.6, 2.6]) for (const s of [-1, 1]) box(lo, D, 0.3, 0.4, 0.5, s * 3.5, 1.3, z, { r: 0.04 });
  lamp(lo, D, lens.white, 2.0, 2.6, 3.32, { w: 0.4, h: 0.25 });
  lamp(lo, D, lens.white, -2.0, 2.6, 3.32, { w: 0.4, h: 0.25 });
  const lower = bakeFree(lo, { mudTop: 3.2, mudBottom: 0.6, mud: 1.1 });

  // ───────── house (rotating upper, static parts) ─────────
  const ho = new THREE.Group();
  const dy = RS.deckY;
  // revolving frame and deck
  box(ho, D, 10.2, 0.85, 13.6, 0, RS.ringY + 0.42, -2.3, { r: 0.12 });
  deck(ho, GR, 11.2, 13.8, 0, dy, -2.5, 0.14);
  for (const s of [-1, 1]) box(ho, D, 0.12, 0.35, 13.8, s * 5.6, dy - 0.2, -2.5, { r: 0.02 }); // deck edge channels
  box(ho, D, 11.2, 0.35, 0.12, 0, dy - 0.2, -9.4, { r: 0.02 });
  // counterweight box under the rear of the deck
  box(ho, D, 9.4, 0.95, 3.0, 0, 2.9, -7.8, { r: 0.12 });
  for (let x = -4.2; x <= 4.21; x += 1.4) box(ho, D, 0.1, 0.8, 0.1, x, 2.9, -9.33, { r: 0.02 });
  // machinery house: walls, belt line, ribs, eave trim
  const hx = 4.4;
  const hz0 = -8.8;
  const hz1 = 3.6;
  const hy1 = 10.7;
  box(ho, H, hx * 2, hy1 - dy, hz1 - hz0, 0, (dy + hy1) / 2, (hz0 + hz1) / 2, { r: 0.14, seg: 2 });
  box(ho, H, hx * 2 + 0.22, 0.2, hz1 - hz0 + 0.22, 0, hy1 + 0.02, (hz0 + hz1) / 2, { r: 0.05 }); // eave trim
  box(ho, H, hx * 2 + 0.1, 0.16, hz1 - hz0 + 0.1, 0, 7.4, (hz0 + hz1) / 2, { r: 0.04 }); // belt line
  for (const s of [-1, 1]) {
    for (let z = hz0 + 1.0; z < hz1 - 0.6; z += 1.3) box(ho, H, 0.1, hy1 - dy - 0.3, 0.16, s * (hx + 0.04), (dy + hy1) / 2, z, { r: 0.02 });
    // louvred air intakes high on the walls, doors at deck level
    louvre(ho, H, D, 2.4, 1.3, s * (hx + 0.05), 9.3, -5.6, s * Math.PI / 2);
    louvre(ho, H, D, 2.4, 1.3, s * (hx + 0.05), 9.3, -2.4, s * Math.PI / 2);
    louvre(ho, H, D, 1.6, 0.9, s * (hx + 0.05), 6.0, -7.6, s * Math.PI / 2);
    door(ho, H, S, s * (hx + 0.04), dy, -4.0, s * Math.PI / 2);
    door(ho, H, S, s * (hx + 0.04), dy, 1.4, s * Math.PI / 2);
  }
  for (let x = -hx + 0.9; x < hx - 0.5; x += 1.3) box(ho, H, 0.16, hy1 - dy - 0.3, 0.1, x, (dy + hy1) / 2, hz0 - 0.04, { r: 0.02 });
  louvre(ho, H, D, 3.0, 1.6, -1.8, 8.9, hz0 - 0.06, Math.PI);
  louvre(ho, H, D, 3.0, 1.6, 1.8, 8.9, hz0 - 0.06, Math.PI);
  door(ho, H, S, 0, dy, hz0 - 0.05, Math.PI, 1.6, 2.3);
  // front wall: hoist-rope openings, a recess, small doors
  box(ho, K, 2.9, 0.9, 0.1, 0, 9.85, hz1 + 0.04, { r: 0.02 });
  box(ho, H, 3.3, 0.18, 0.4, 0, 10.35, hz1 + 0.15, { r: 0.03 });
  door(ho, H, S, -2.6, dy, hz1 + 0.04, 0, 0.9, 2.0);
  louvre(ho, H, D, 1.8, 1.0, -2.6, 8.4, hz1 + 0.05);
  // roof: pressurizer/filter units, aircon, exhaust fans, lifting lugs, a roof rail
  box(ho, H, 2.6, 1.4, 2.0, -2.2, hy1 + 0.75, -6.4, { r: 0.1 });
  louvre(ho, H, D, 2.2, 1.0, -2.2, hy1 + 0.75, -5.35);
  box(ho, H, 2.0, 1.0, 1.6, 2.4, hy1 + 0.55, -6.8, { r: 0.08 });
  for (const x of [-2.6, 0.2]) {
    cyl(ho, D, 0.55, 0.5, x, hy1 + 0.3, -0.8, { seg: 20 });
    cyl(ho, K, 0.5, 0.06, x, hy1 + 0.56, -0.8, { seg: 20 });
  }
  rail(ho, YL, [v3(-hx + 0.15, hy1 + 0.1, hz0 + 0.15), v3(hx - 0.15, hy1 + 0.1, hz0 + 0.15), v3(hx - 0.15, hy1 + 0.1, hz1 - 0.15)], { h: 1.05 });
  rail(ho, YL, [v3(-hx + 0.15, hy1 + 0.1, hz1 - 0.15), v3(-hx + 0.15, hy1 + 0.1, hz0 + 0.15)], { h: 1.05 });
  ladder(ho, YL, v3(-hx - 0.25, dy, hz0 - 0.3), v3(-hx - 0.25, hy1 + 1.0, hz0 - 0.3), 0.5);
  beacon(ho, K, lens.amber, 3.4, hy1 + 0.1, -8.2, 1.4);
  beacon(ho, K, lens.amber, -3.4, hy1 + 0.1, -8.2, 1.4);
  // deck handrails all round (gap for the stair at the rear left and the boom at the front)
  const e = 5.5;
  rail(ho, YL, [v3(e, dy, -6.5), v3(e, dy, 2.95)], { h: 1.1 });
  rail(ho, YL, [v3(2.6, dy, 4.1), v3(3.1, dy, 4.1)], { h: 1.1 });
  rail(ho, YL, [v3(-2.6, dy, 4.1), v3(-e, dy, 4.1), v3(-e, dy, -9.3), v3(e, dy, -9.3), v3(e, dy, -8.2)], { h: 1.1 });
  // access stairway from the ground at the rear left up to the deck
  stairs(ho, D, GR, YL, v3(6.05, 0.3, -11.6), v3(6.05, dy, -7.65), 0.95);
  deck(ho, GR, 1.1, 1.1, 6.05, dy, -7.15, 0.1);
  box(ho, D, 0.3, 0.3, 1.4, 5.7, dy - 0.3, -7.3, { r: 0.03 });
  rail(ho, YL, [v3(6.55, dy, -7.75), v3(6.55, dy, -6.6), v3(5.5, dy, -6.6)], { h: 1.1 });
  // fire bottles and an electrical cabinet on the deck
  bottle(ho, RED, S, 4.85, dy, -2.2, 0.9, 0.13);
  bottle(ho, RED, S, 4.85, dy, -1.85, 0.9, 0.13);
  box(ho, H, 0.6, 1.6, 1.4, -4.9, dy + 0.8, -3.2, { r: 0.05 });
  // under-deck floodlights onto the crawlers, and deck-front lights onto the face
  for (const [x, z] of [
    [4.6, -8.8],
    [-4.6, -8.8],
    [4.6, 3.6],
    [-4.6, 3.6],
  ] as const)
    lamp(ho, D, lens.white, x, dy - 0.45, z, { w: 0.4, h: 0.25, pitch: Math.PI / 2.4, yaw: z > 0 ? 0 : Math.PI });
  for (const x of [-3.6, -1.9, 1.9]) lamp(ho, D, lens.white, x, hy1 - 0.35, hz1 + 0.18, { w: 0.55, h: 0.35, pitch: 0.38 });

  // operator cab at the front left, overhanging the deck edge, on a dark sub-frame
  const cab = node(ho, 4.35, 7.1, 4.8);
  box(cab, H, 1.5, 2.8, 1.2, -0.45, -1.4, -0.75, { r: 0.08 });
  beam(cab, D, v3(0.6, -2.75, -0.3), v3(0.6, -0.1, 1.1), 0.22, 0.22);
  beam(cab, D, v3(-0.9, -2.75, -0.3), v3(-0.9, -0.1, 1.1), 0.22, 0.22);
  box(cab, H, 2.6, 2.75, 2.6, 0, 1.38, 0, { r: 0.12, seg: 2 });
  box(cab, H, 2.75, 0.14, 2.75, 0, 2.78, 0.02, { r: 0.04 });
  box(cab, G, 2.25, 1.75, 0.05, 0, 1.5, 1.31, { r: 0.015 });
  box(cab, G, 1.9, 0.7, 0.05, 0, 0.35, 1.25, { rx: 0.5, r: 0.015 });
  for (const s of [-1, 1]) {
    box(cab, G, 0.05, 1.6, 1.15, s * 1.31, 1.55, 0.5, { r: 0.015 });
    box(cab, G, 0.05, 1.6, 0.8, s * 1.31, 1.55, -0.65, { r: 0.015 });
  }
  box(cab, H, 2.75, 0.12, 0.6, 0, 2.55, 1.55, { rx: -0.15, r: 0.03 }); // sun visor
  bar(cab, K, v3(-0.6, 0.65, 1.36), v3(-0.25, 2.2, 1.36), 0.02, 5);
  bar(cab, K, v3(0.45, 0.65, 1.36), v3(0.8, 2.2, 1.36), 0.02, 5);
  for (const s of [-1, 1]) lamp(cab, D, lens.white, s * 0.95, 2.95, 1.2, { w: 0.3, h: 0.2, pitch: 0.2 });
  beacon(cab, K, lens.amber, -0.9, 2.85, -0.9, 1.1);
  mirror(cab, K, mats.chrome(), v3(1.32, 1.9, 1.0), v3(1.75, 2.1, 1.5), 0.45, 0.4);
  // cab walkway and rail on the outboard side
  deck(ho, GR, 0.9, 3.0, 6.0, 7.1, 4.8, 0.08);
  rail(ho, YL, [v3(6.4, 7.1, 3.4), v3(6.4, 7.1, 6.25), v3(5.65, 7.1, 6.25)], { h: 1.05 });
  ladder(ho, YL, v3(5.95, dy, 3.35), v3(5.95, 7.1, 3.35), 0.5);
  deck(ho, GR, 0.9, 0.9, 5.95, dy, 3.35, 0.1);

  // boom foot brackets at the front of the revolving frame
  for (const s of [-1, 1]) {
    plate(ho, D, [v3(s * 1.75, RS.ringY, 2.6), v3(s * 1.75, RS.ringY, 5.0), v3(s * 1.75, RS.foot.y + 0.6, 5.0), v3(s * 1.75, RS.foot.y + 0.6, 3.6)], 0.18, { out: v3(s, 0, 0) });
    cyl(ho, S, 0.36, 0.5, s * 1.85, RS.foot.y, RS.foot.z, { axis: "x", seg: 16 });
  }

  // boom: tapered box girder in a frame along the boom axis (local +z), top face toward local +y
  const bm = node(ho, 0, RS.foot.y, RS.foot.z, { rx: -RS.boomAng });
  prism(
    bm,
    H,
    [
      [0.7, -1.2],
      [19.3, -0.8],
      [19.9, -0.55],
      [19.9, 0.55],
      [19.3, 0.85],
      [0.7, 1.15],
      [0.2, 0.6],
      [0.2, -0.6],
    ],
    1.9,
    "zy",
    0,
    { bevel: 0.05 },
  );
  for (const s of [-1, 1]) {
    // foot legs splaying out to the pins
    prism(bm, H, [[-0.45, -0.55], [1.6, -1.1], [1.6, 1.1], [-0.45, 0.55]], 0.35, "zy", s * 1.55, { bevel: 0.03 });
    cyl(bm, S, 0.42, 0.42, s * 1.55, 0, 0, { axis: "x", seg: 18 });
    // side stiffeners
    beam(bm, H, v3(s * 0.98, 0.0, 1.5), v3(s * 0.98, 0.05, 18.8), 0.08, 0.22);
    beam(bm, H, v3(s * 0.98, 0.7, 2.0), v3(s * 0.98, 0.5, 18.5), 0.06, 0.14);
    beam(bm, H, v3(s * 0.98, -0.7, 2.0), v3(s * 0.98, -0.5, 18.5), 0.06, 0.14);
    // point cheek plates and sheave shaft bosses
    prism(bm, H, [[18.6, -0.9], [21.0, -0.9], [21.6, 0], [21.0, 1.0], [18.6, 1.0]], 0.22, "zy", s * 1.35, { bevel: 0.03 });
    cyl(bm, D, 0.38, 0.3, s * 1.55, 0, RS.boomLen, { axis: "x", seg: 16 });
    // shipper shaft bosses
    cyl(bm, D, 0.62, 0.3, s * 1.05, 0, RS.shaftS, { axis: "x", seg: 20 });
  }
  // boom-point sheaves (rope grooves as darker rims)
  for (const s of [-1, 1]) {
    cyl(bm, D, RS.sheaveR, 0.42, s * 0.72, 0, RS.boomLen, { axis: "x", seg: 40 });
    cyl(bm, S, RS.sheaveR * 0.45, 0.48, s * 0.72, 0, RS.boomLen, { axis: "x", seg: 20 });
    for (let i = 0; i < 6; i++) {
      const a = (i / 6) * Math.PI * 2;
      cyl(bm, K, 0.22, 0.44, s * 0.72, Math.cos(a) * RS.sheaveR * 0.7, RS.boomLen + Math.sin(a) * RS.sheaveR * 0.7, { axis: "x", seg: 10 });
    }
  }
  cyl(bm, S, 0.26, 3.4, 0, 0, RS.boomLen, { axis: "x", seg: 14 });
  // shipper shaft through the boom, crowd machinery housing on top
  cyl(bm, S, 0.36, 3.7, 0, 0, RS.shaftS, { axis: "x", seg: 18 });
  box(bm, H, 1.7, 1.3, 2.4, 0, 1.6, RS.shaftS - 1.6, { r: 0.1 });
  box(bm, D, 1.5, 0.3, 1.8, 0, 2.3, RS.shaftS - 1.6, { r: 0.05 });
  // boom catwalk handrail and boom lights
  rail(bm, YL, [v3(0.7, 1.12, 2.2), v3(0.7, 0.92, 17.0)], { h: 0.9, posts: 2.2 });
  for (const s of [-1, 1]) lamp(bm, D, lens.white, s * 1.08, -0.3, 6.0, { w: 0.45, h: 0.3, yaw: s * 0.25, pitch: 0.9 });
  lamp(bm, D, lens.white, 0, -0.95, 15.0, { w: 0.5, h: 0.3, pitch: 1.4 });

  // gantry (A-frame) on the house roof, and the pendant ropes to the boom point
  const ap = RS.apex;
  for (const s of [-1, 1]) {
    const top = v3(s * 1.6, ap.y, ap.z);
    beam(ho, H, v3(s * 3.2, hy1 + 0.1, 1.9), top, 0.7, 0.7, Y, 0.06);
    beam(ho, H, v3(s * 3.2, hy1 + 0.1, -7.4), top, 0.5, 0.5, Y, 0.05);
    box(ho, H, 1.2, 0.6, 1.4, s * 3.2, hy1 + 0.35, 1.9, { r: 0.05 });
    box(ho, H, 1.0, 0.5, 1.0, s * 3.2, hy1 + 0.3, -7.4, { r: 0.05 });
    beam(ho, H, v3(s * 2.7, 13.4, 0.1), v3(s * 2.7, 13.4, -5.3), 0.3, 0.3);
  }
  beam(ho, H, v3(-2.0, ap.y + 0.1, ap.z), v3(2.0, ap.y + 0.1, ap.z), 0.8, 0.9, Y, 0.06);
  beam(ho, H, v3(-2.8, 13.4, 0.1), v3(2.8, 13.4, 0.1), 0.35, 0.35);
  for (const s of [-1, 1]) {
    cyl(ho, D, 0.55, 0.5, s * 1.2, ap.y + 0.1, ap.z + 0.3, { axis: "x", seg: 18 });
    for (const dx of [-0.18, 0.18]) {
      const from = v3(s * 1.2 + dx, ap.y + 0.3, ap.z + 0.5);
      const t = atBoom(19.2, 1.0);
      bar(ho, ropeMat(), from, v3(s * 1.05 + dx, t.y, t.x), 0.07, 6);
    }
  }
  // hoist ropes' back legs: over the top of the point sheaves, down along the boom into the house
  for (const s of [-1, 1]) {
    const t = atBoom(RS.boomLen, RS.sheaveR);
    for (const dx of [-0.08, 0.08]) bar(ho, ropeMat(), v3(s * 0.72 + dx, t.y, t.x), v3(s * 0.72 + dx, 9.95, hz1 + 0.05), 0.045, 6);
  }
  const house = bakeFree(ho, { mudTop: 5.5, mudBottom: 2.5, mud: 0.8, dust: 1 });

  // ───────── saddle blocks (rotate with the handle about the shipper shaft) ─────────
  const sa = new THREE.Group();
  for (const s of [-1, 1]) {
    box(sa, D, 0.95, 1.05, 2.0, s * RS.handleX, 0, 0, { r: 0.1 });
    box(sa, W, 0.8, 0.14, 1.9, s * RS.handleX, 0.55, 0, { r: 0.03 });
    cyl(sa, D, 0.55, 0.3, s * (RS.handleX - 0.55), 0, 0, { axis: "x", seg: 18 });
  }
  const saddle = bakeFree(sa, { mudTop: 2, mudBottom: -3, mud: 0.4 });

  // ───────── handle + dipper (one rigid body in the slide frame) ─────────
  const sl = new THREE.Group();
  const L = RS.handleLen;
  for (const s of [-1, 1]) {
    cyl(sl, H, 0.33, L - 0.3, s * RS.handleX, 0, -L / 2 - 0.15, { axis: "z", seg: 20, wear: { mud: 0.3 } });
    cyl(sl, D, 0.34, 0.25, s * RS.handleX, 0, -L + 0.12, { axis: "z", seg: 20 });
    box(sl, W, 0.22, 0.12, L - 2.0, s * RS.handleX, -0.34, -L / 2 - 0.6, { r: 0.02 }); // crowd rack
    // pitch braces from the handle to the dipper's lower back
    beam(sl, D, v3(s * RS.handleX, -0.2, -2.6), v3(s * 1.6, DIP.yBot + 0.6, DIP.zBack), 0.22, 0.26, Y, 0.03);
  }
  box(sl, H, RS.handleX * 2 + 0.6, 0.7, 0.7, 0, 0, -0.35, { r: 0.08 });
  box(sl, D, 3.2, 1.6, 0.4, 0, 0.1, 0.05, { r: 0.06 });
  // dipper shell
  const dm = dipperMat();
  const wr = { mud: 1.6, dust: 1.2 };
  const dz0 = DIP.zBack;
  const dz1 = DIP.zFront;
  const sideProfile = (x: number) => [
    v3(x, DIP.yBot, dz0),
    v3(x, DIP.yBot, dz1 - DIP.chamfer),
    v3(x, DIP.yBot + DIP.chamfer, dz1),
    v3(x, DIP.yTop, dz1),
    v3(x, DIP.yBack, dz0),
  ];
  for (const s of [-1, 1]) {
    plate(sl, dm, sideProfile(s * DIP.x), 0.14, { out: v3(s, 0, 0), wear: wr });
    // wear bars and stiffeners on the side walls
    for (const yy of [-1.2, 0.0, 1.1])
      beam(sl, W, v3(s * (DIP.x + 0.05), yy, dz0 + 0.2), v3(s * (DIP.x + 0.05), yy + 0.1, dz1 - 0.2), 0.1, 0.16);
    // bail pin bosses
    cyl(sl, D, 0.32, 0.36, s * (DIP.x + 0.12), BAIL_PIN.y, BAIL_PIN.z, { axis: "x", seg: 16 });
    // door snubber housings
    box(sl, D, 0.4, 0.5, 0.6, s * 1.9, DIP.yBot + 0.4, dz0 - 0.25, { r: 0.05 });
  }
  plate(sl, dm, [v3(-DIP.x, DIP.yBot, dz0), v3(DIP.x, DIP.yBot, dz0), v3(DIP.x, DIP.yBack, dz0), v3(-DIP.x, DIP.yBack, dz0)], 0.14, { out: v3(0, 0, -1), wear: wr });
  plate(sl, dm, [v3(-DIP.x, DIP.yBot + DIP.chamfer, dz1), v3(DIP.x, DIP.yBot + DIP.chamfer, dz1), v3(DIP.x, DIP.yTop, dz1), v3(-DIP.x, DIP.yTop, dz1)], 0.14, { out: v3(0, 0, 1), wear: wr });
  plate(sl, dm, [v3(-DIP.x, DIP.yBot, dz1 - DIP.chamfer), v3(DIP.x, DIP.yBot, dz1 - DIP.chamfer), v3(DIP.x, DIP.yBot + DIP.chamfer, dz1), v3(-DIP.x, DIP.yBot + DIP.chamfer, dz1)], 0.14, { out: v3(0, -1, 1), wear: wr });
  // horizontal ribs on the front wall, the heavy lip and teeth
  for (const yy of [-1.0, 0.5]) beam(sl, W, v3(-DIP.x + 0.1, yy, dz1 + 0.06), v3(DIP.x - 0.1, yy, dz1 + 0.06), 0.14, 0.12);
  box(sl, W, DIP.x * 2 + 0.3, 0.32, 0.55, 0, DIP.yTop + 0.06, dz1 - 0.1, { rx: 0.2, r: 0.06 });
  const nT = 9;
  for (let i = 0; i < nT; i++) {
    const x = lerp(-DIP.x + 0.35, DIP.x - 0.35, i / (nT - 1));
    const t = node(sl, x, DIP.yTop + 0.15, dz1 - 0.05, { rx: 0.3 });
    box(t, W, 0.34, 0.55, 0.4, 0, 0.2, 0, { r: 0.05 });
    const tip = cyl(t, S, 0.2, 0.55, 0, 0.72, 0, { seg: 4, r2: 0.05 });
    tip.rotation.y = Math.PI / 4;
    tip.scale.set(1, 1, 0.75);
    if (i < nT - 1) box(t, W, 0.22, 0.32, 0.3, (DIP.x * 2 - 0.7) / (nT - 1) / 2, 0.08, 0.02, { r: 0.04 });
  }
  const slide = bakeFree(sl, { mudTop: 2.6, mudBottom: -2.4, mud: 0.6, dust: 1 });

  // ───────── trip door (hinged at the back-bottom edge) ─────────
  const dr = new THREE.Group();
  const dl = dz1 - DIP.chamfer - dz0;
  plate(dr, dm, [v3(-DIP.x + 0.05, 0, 0), v3(DIP.x - 0.05, 0, 0), v3(DIP.x - 0.05, 0, dl), v3(-DIP.x + 0.05, 0, dl)], 0.16, { out: v3(0, -1, 0), wear: { mud: 1.6 } });
  for (const x of [-1.8, -0.6, 0.6, 1.8]) beam(dr, W, v3(x, -0.2, 0.2), v3(x, -0.2, dl - 0.2), 0.16, 0.22);
  for (const s of [-1, 1]) cyl(dr, D, 0.2, 0.6, s * 1.9, -0.05, 0, { axis: "x", seg: 14 });
  box(dr, D, 1.2, 0.3, 0.4, 0, -0.25, dl - 0.25, { r: 0.05 }); // latch
  const doorG = bakeFree(dr, { mudTop: 2, mudBottom: -2, mud: 1.2 });

  // ───────── bail (pinned to the dipper sides, swings to follow the hoist ropes) ─────────
  const bl = new THREE.Group();
  for (const s of [-1, 1]) {
    beam(bl, D, v3(s * (DIP.x + 0.28), 0, 0), v3(s * 0.85, BAIL_LEN, 0), 0.28, 0.4, v3(0, 0, 1), 0.05);
    cyl(bl, D, 0.36, 0.25, s * (DIP.x + 0.3), 0, 0, { axis: "x", seg: 16 });
  }
  box(bl, D, 2.3, 0.6, 0.7, 0, BAIL_LEN, 0, { r: 0.08 }); // equalizer
  for (const s of [-1, 1]) cyl(bl, S, 0.18, 0.5, s * 0.72, BAIL_LEN + 0.45, 0, { seg: 12 }); // rope sockets
  const bail = bakeFree(bl, { mudTop: 0, mudBottom: -1, mud: 0.3 });

  return { lower, house, saddle, slide, door: doorG, bail, lens };
}

/**
 * Builds an electric rope shovel. Geometry and materials are shared between shovels of the same
 * colour; tracks, ropes and lenses are per shovel.
 */
export function makeRopeShovel(opts: RopeShovelOptions = {}): RopeShovel {
  const colour = opts.colour ?? COLOURS.cream;
  let tpl = ropeTemplates.get(colour);
  if (!tpl) {
    tpl = buildRopeTemplate(colour);
    ropeTemplates.set(colour, tpl);
  }
  const lens = lenses();
  const group = new THREE.Group();
  group.name = "rope-shovel";
  const lower = instance(tpl.lower);
  swapLenses(lower, tpl.lens, lens);
  group.add(lower);
  const tracks: Track[] = [];
  for (const s of [-1, 1]) {
    const t = makeTrack(RS.track);
    t.group.position.x = s * RS.trackX;
    group.add(t.group);
    tracks.push(t);
  }
  const house = new THREE.Group();
  house.add(instance(tpl.house));
  swapLenses(house, tpl.lens, lens);
  group.add(house);
  // handle pivot at the shipper shaft → slide (crowd) → dipper parts
  const pivot = new THREE.Group();
  pivot.position.set(0, SHAFT.y, SHAFT.x);
  pivot.add(instance(tpl.saddle));
  house.add(pivot);
  const slide = new THREE.Group();
  slide.add(instance(tpl.slide));
  pivot.add(slide);
  const doorHinge = new THREE.Group();
  doorHinge.position.set(0, DIP.yBot, DIP.zBack);
  doorHinge.add(instance(tpl.door));
  slide.add(doorHinge);
  const bail = new THREE.Group();
  bail.position.copy(BAIL_PIN);
  bail.add(instance(tpl.bail));
  slide.add(bail);
  const sockets = [-1, 1].map((s) => {
    const o = new THREE.Object3D();
    o.position.set(s * 0.72, BAIL_LEN + 0.7, 0);
    bail.add(o);
    return o;
  });
  // hoist ropes (two parts each, sheave → socket), a shared unit cylinder scaled per frame
  const unit = new THREE.CylinderGeometry(1, 1, 1, 7, 1, true);
  unit.translate(0, 0.5, 0);
  const ropes = [0, 1, 2, 3].map(() => {
    const m = new THREE.Mesh(unit, ropeMat());
    m.castShadow = true;
    house.add(m);
    return m;
  });
  const dipperAnchor = new THREE.Object3D();
  dipperAnchor.position.set(0, 0.1, (DIP.zBack + DIP.zFront) / 2);
  slide.add(dipperAnchor);
  const cableAnchor = new THREE.Object3D();
  cableAnchor.position.set(0, 0, -4.6);
  group.add(cableAnchor);

  const state: RopeShovelState = { swing: 0, crowd: 0.6, hoist: 0.3, door: 0, lights: false, distance: 0 };
  const v = new THREE.Vector3();
  const sheave = new THREE.Vector3();
  const tangent = new THREE.Vector3();
  const apply = () => {
    house.rotation.y = state.swing;
    for (const t of tracks) t.setDistance(state.distance);
    const crowd = clamp01(state.crowd);
    const e = lerp(RS.crowd[0], RS.crowd[1], crowd);
    const h = lerp(RS.hoistY[0], RS.hoistY[1], clamp01(state.hoist));
    const maxUp = lerp(deg(-2), deg(36), crowd);
    const sn = THREE.MathUtils.clamp((h - SHAFT.y) / e, Math.sin(deg(-72)), Math.sin(maxUp));
    const phi = Math.asin(sn);
    pivot.rotation.x = -phi;
    slide.position.z = e;
    doorHinge.rotation.x = clamp01(state.door) * 1.2;
    // bail follows the rope pull: aim its local +y at the sheave centre
    group.updateMatrixWorld(true);
    sheave.set(0, POINT.y, POINT.x);
    v.copy(sheave);
    house.localToWorld(v);
    slide.worldToLocal(v);
    v.sub(BAIL_PIN);
    bail.rotation.x = Math.atan2(v.z, v.y);
    bail.updateMatrixWorld(true);
    // ropes: from the front tangent of each sheave to its socket on the equalizer
    sockets.forEach((so, k) => {
      so.getWorldPosition(v);
      house.worldToLocal(v);
      const x = (k === 0 ? -1 : 1) * 0.72;
      const dz = v.z - POINT.x;
      const dyy = v.y - POINT.y;
      const d = Math.hypot(dz, dyy);
      const beta = Math.atan2(dyy, dz);
      const alpha = Math.acos(Math.min(1, RS.sheaveR / Math.max(d, RS.sheaveR + 1e-3)));
      const a1 = beta + alpha;
      const a2 = beta - alpha;
      const pick = Math.cos(a1) > Math.cos(a2) ? a1 : a2;
      for (const j of [0, 1]) {
        const dx = j ? 0.08 : -0.08;
        tangent.set(x + dx, POINT.y + Math.sin(pick) * RS.sheaveR, POINT.x + Math.cos(pick) * RS.sheaveR);
        const end = v.clone().setX(v.x + dx);
        const rope = ropes[k * 2 + j]!;
        const dir = end.sub(tangent);
        const len = dir.length();
        rope.position.copy(tangent);
        rope.quaternion.setFromUnitVectors(Y, dir.normalize());
        rope.scale.set(0.045, len, 0.045);
      }
    });
    lens.set(state.lights);
  };
  const shovel: RopeShovel = {
    group,
    state,
    lenses: lens,
    anchors: { dipper: dipperAnchor, cable: cableAnchor, house },
    set(s) {
      Object.assign(state, s);
      apply();
    },
    update() {
      /* no autonomous motion; scenes drive the state */
    },
  };
  void opts.seed;
  apply();
  return shovel;
}

// ───────────────────────────── hydraulic face shovel ─────────────────────────────

export const FACE_SHOVEL = {
  trackX: 3.45,
  track: { length: 7.2, sprocketR: 0.85, idlerR: 0.78, shoeW: 1.75, pitch: 0.42 },
  deckY: 3.6,
  foot: { y: 4.35, z: 2.6 },
  boomLen: 7.8,
  boom: [deg(12), deg(62)] as const,
  stickLen: 4.6,
  stick: [deg(-142), deg(-38)] as const,
  bucket: [deg(-45), deg(55)] as const,
  clam: deg(50),
} as const;
const FS = FACE_SHOVEL;

export interface FaceShovelState {
  swing: number;
  /** Boom raise 0..1. */
  boom: number;
  /** Stick out 0..1 (0 tucked under the boom, 1 reaching forward). */
  stick: number;
  /** Bucket curl 0..1 (0 lip down, 1 curled back to hold the load). */
  bucket: number;
  /** Bottom-dump clam 0 shut .. 1 open. */
  clam: number;
  lights: boolean;
  distance: number;
}

export interface FaceShovel {
  group: THREE.Group;
  state: FaceShovelState;
  set(s: Partial<FaceShovelState>): void;
  update(dt: number): void;
  lenses: Lenses;
  anchors: { bucket: THREE.Object3D; upper: THREE.Object3D };
}

interface FaceTemplate {
  lower: THREE.Group;
  upper: THREE.Group;
  boom: THREE.Group;
  stick: THREE.Group;
  bucket: THREE.Group;
  clam: THREE.Group;
  paint: THREE.Material;
  rams: Map<string, ReturnType<typeof ramParts>>;
  lens: Lenses;
}
const faceTemplates = new Map<string, FaceTemplate>();

// Bucket side profile in the bucket frame (origin at the stick pin, +z down the stick at curl 0, +y toward
// the face): A top-back, B bottom-back, C floor front, D lip, E top of the mouth (the clam hinge).
const BK = { x: 2.45, A: [-0.35, -0.4], B: [-0.35, 3.2], C: [2.4, 3.2], D: [3.3, 2.65], E: [1.6, -0.4] } as const;
const HINGE = v3(0, BK.E[0], BK.E[1]);

// pin positions (in their parent's frame) shared by template and instance
const PIN = {
  boomBase: (s: number) => v3(s * 1.2, 2.95, 3.4), // upper frame
  boomRod: (s: number) => v3(s * 1.2, -1.2, 3.0), // boom frame
  stickBase: (s: number) => v3(s * 0.5, 1.45, 2.4), // boom frame
  stickRod: (s: number) => v3(s * 0.5, 0.35, -1.35), // stick frame
  bucketBase: (s: number) => v3(s * 0.95, 0.95, 0.4), // stick frame
  bucketRod: (s: number) => v3(s * 0.95, 1.3, -0.05), // bucket frame
  clamBase: (s: number) => v3(s * 2.62, 0.25, 0.25), // bucket frame
  clamRod: (s: number) => v3(s * 2.62, 0.3, 2.3), // clam frame (relative to the hinge)
};

function buildFaceTemplate(colour: string): FaceTemplate {
  const P = paint(colour);
  const D = mats.dark();
  const K = mats.black();
  const S = mats.steel();
  const G = mats.glass();
  const GR = mats.grate();
  const RED = mats.red();
  const W = wearSteel();
  const lens = lenses();
  const dy = FS.deckY;

  // lower works
  const lo = new THREE.Group();
  box(lo, D, 4.6, 1.5, 4.8, 0, 1.85, 0, { r: 0.15, seg: 2 });
  for (const s of [-1, 1]) {
    box(lo, D, 1.3, 1.0, 3.4, s * 2.7, 1.75, 0, { r: 0.1 });
    box(lo, D, 0.9, 1.0, 1.2, s * 2.3, 1.45, -3.2, { r: 0.1 });
    cyl(lo, D, 0.42, 0.7, s * 2.25, 1.55, -3.2, { axis: "x", seg: 16 });
  }
  cyl(lo, D, 2.7, 0.3, 0, 2.7, 0, { seg: 40 });
  cyl(lo, S, 2.55, 0.12, 0, 2.88, 0, { seg: 40 });
  const lower = bakeFree(lo, { mudTop: 2.8, mudBottom: 0.5, mud: 1.1 });

  // upper: frame, counterweight, engine house, tank module, cab module and cab, walkways, stair
  const up = new THREE.Group();
  box(up, D, 7.2, 0.7, 8.2, 0, 3.2, -1.5, { r: 0.1 });
  deck(up, GR, 7.8, 8.6, 0, dy, -1.5, 0.12);
  box(up, P, 7.6, 2.6, 1.5, 0, 3.9, -6.3, { r: 0.35, seg: 2 }); // counterweight
  box(up, P, 6.6, 3.3, 4.6, 0, dy + 1.65, -3.6, { r: 0.12, seg: 2 }); // engine house
  box(up, P, 6.8, 0.14, 4.8, 0, dy + 3.33, -3.6, { r: 0.04 });
  for (const s of [-1, 1]) {
    louvre(up, P, D, 3.2, 1.6, s * 3.32, dy + 1.9, -3.6, s * Math.PI / 2);
    door(up, P, S, s * 3.32, dy, -1.7, s * Math.PI / 2, 0.8, 1.9);
  }
  louvre(up, P, D, 2.6, 1.6, -1.5, dy + 1.8, -5.92, Math.PI);
  louvre(up, P, D, 2.6, 1.6, 1.5, dy + 1.8, -5.92, Math.PI);
  for (const x of [-1.6, 1.6]) {
    cyl(up, K, 0.18, 1.1, x, dy + 3.85, -4.6, { seg: 12 });
    cyl(up, K, 0.22, 0.2, x, dy + 4.4, -4.6, { seg: 12, r2: 0.18 });
  }
  box(up, P, 2.6, 0.6, 1.6, 0, dy + 3.6, -2.6, { r: 0.08 });
  rail(up, mats.yellow(), [v3(-3.25, dy + 3.4, -1.4), v3(-3.25, dy + 3.4, -5.8), v3(3.25, dy + 3.4, -5.8), v3(3.25, dy + 3.4, -1.4)], { h: 1.0 });
  // hydraulic tank / cooler module front right
  box(up, P, 2.3, 2.7, 3.3, -2.45, dy + 1.35, 0.55, { r: 0.12 });
  louvre(up, P, D, 1.8, 1.4, -2.45, dy + 1.6, 2.22);
  // cab module front left and the cab on top
  box(up, P, 2.3, 3.6, 2.7, 2.45, dy + 1.8, 0.75, { r: 0.12 });
  door(up, P, S, 3.62, dy, 0.8, Math.PI / 2, 0.8, 1.9);
  const cab = node(up, 2.5, dy + 3.6, 1.15);
  box(cab, P, 2.4, 2.5, 2.5, 0, 1.25, 0, { r: 0.12, seg: 2 });
  box(cab, P, 2.55, 0.12, 2.65, 0, 2.52, 0.02, { r: 0.04 });
  box(cab, G, 2.1, 1.65, 0.05, 0, 1.38, 1.26, { r: 0.015 });
  box(cab, G, 1.7, 0.55, 0.05, 0, 0.32, 1.2, { rx: 0.45, r: 0.015 });
  for (const s of [-1, 1]) {
    box(cab, G, 0.05, 1.55, 1.0, s * 1.21, 1.42, 0.55, { r: 0.015 });
    box(cab, G, 0.05, 1.55, 0.8, s * 1.21, 1.42, -0.6, { r: 0.015 });
  }
  box(cab, P, 2.55, 0.1, 0.5, 0, 2.35, 1.45, { rx: -0.15, r: 0.03 }); // visor
  bar(cab, K, v3(-0.5, 0.6, 1.31), v3(-0.2, 2.05, 1.31), 0.02, 5);
  bar(cab, K, v3(0.4, 0.6, 1.31), v3(0.7, 2.05, 1.31), 0.02, 5);
  for (const s of [-1, 1]) lamp(cab, D, lens.white, s * 0.85, 2.7, 1.15, { w: 0.3, h: 0.2, pitch: 0.25 });
  beacon(cab, K, lens.amber, -0.8, 2.58, -0.8, 1.1);
  mirror(cab, K, mats.chrome(), v3(1.22, 1.7, 1.0), v3(1.6, 1.95, 1.45), 0.4, 0.4);
  // walkway round the cab and a rail on the roof edge of the modules
  deck(up, GR, 0.9, 3.4, 4.1, dy + 3.6, 1.0, 0.08);
  rail(up, mats.yellow(), [v3(4.5, dy + 3.6, -0.6), v3(4.5, dy + 3.6, 2.65), v3(3.75, dy + 3.6, 2.65)], { h: 1.05 });
  ladder(up, mats.yellow(), v3(4.2, dy, -0.75), v3(4.2, dy + 3.6, -0.75), 0.5);
  // deck rails
  const ex = 3.85;
  rail(up, mats.yellow(), [v3(ex, dy, 2.6), v3(ex, dy, -5.6), v3(-ex, dy, -5.6), v3(-ex, dy, 2.6)], { h: 1.05 });
  // access stairway hanging at the left front, from the ground to the deck
  stairs(up, D, GR, mats.yellow(), v3(4.45, 0.35, 5.6), v3(4.45, dy, 2.5), 0.85);
  deck(up, GR, 1.0, 1.0, 4.45, dy, 2.1, 0.1);
  // boom foot brackets, boom-ram base lugs
  for (const s of [-1, 1]) {
    plate(up, D, [v3(s * 1.0, 2.9, 1.2), v3(s * 1.0, 2.9, 3.4), v3(s * 1.0, FS.foot.y + 0.55, 3.2), v3(s * 1.0, FS.foot.y + 0.55, 1.8)], 0.2, { out: v3(s, 0, 0) });
    cyl(up, S, 0.3, 0.3, s * 1.1, FS.foot.y, FS.foot.z, { axis: "x", seg: 14 });
    box(up, D, 0.5, 0.6, 0.8, s * 1.2, 2.95, 3.35, { r: 0.05 });
  }
  bottle(up, RED, S, 3.5, dy, -5.2, 0.85, 0.12);
  bottle(up, RED, S, 3.2, dy, -5.2, 0.85, 0.12);
  for (const x of [-2.4, -1.0]) lamp(up, D, lens.white, x, dy + 2.95, 2.25, { w: 0.45, h: 0.3, pitch: 0.3 });
  lamp(up, D, lens.red, 3.0, dy + 0.4, -7.06, { w: 0.3, h: 0.18, yaw: Math.PI });
  lamp(up, D, lens.red, -3.0, dy + 0.4, -7.06, { w: 0.3, h: 0.18, yaw: Math.PI });
  const upper = bakeFree(up, { mudTop: 4.6, mudBottom: 2.4, mud: 0.8 });

  // boom (frame at the foot pin, local +z along the boom, +y its top)
  const bm = new THREE.Group();
  const BL = FS.boomLen;
  prism(
    bm,
    P,
    [
      [-0.55, -0.7],
      [2.9, -1.35],
      [5.5, -1.0],
      [BL, -0.7],
      [BL + 0.55, 0],
      [BL, 0.7],
      [4.6, 1.05],
      [1.4, 1.1],
      [-0.55, 0.7],
    ],
    1.7,
    "zy",
    0,
    { bevel: 0.05 },
  );
  for (const s of [-1, 1]) {
    cyl(bm, S, 0.36, 0.3, s * 0.95, 0, 0, { axis: "x", seg: 16 });
    cyl(bm, S, 0.36, 0.3, s * 0.95, 0, BL, { axis: "x", seg: 16 });
    box(bm, D, 0.35, 0.6, 0.8, PIN.boomRod(s).x, PIN.boomRod(s).y + 0.05, PIN.boomRod(s).z, { r: 0.05 });
    box(bm, D, 0.3, 0.55, 0.7, PIN.stickBase(s).x, PIN.stickBase(s).y - 0.1, PIN.stickBase(s).z, { r: 0.05 });
    beam(bm, P, v3(s * 0.87, 0.05, 0.6), v3(s * 0.87, 0.0, BL - 0.4), 0.06, 0.2);
    beam(bm, P, v3(s * 0.87, -0.65, 1.2), v3(s * 0.87, -0.45, BL - 0.8), 0.05, 0.12);
    bar(bm, mats.hose(), v3(s * 0.6, 1.12, 0.4), v3(s * 0.6, 0.78, BL - 0.1), 0.05, 6);
    bar(bm, mats.hose(), v3(s * 0.4, 1.12, 0.4), v3(s * 0.4, 0.8, BL - 0.1), 0.045, 6);
  }
  for (const s of [-1, 1]) lamp(bm, D, lens.white, s * 0.9, 0.6, 2.0, { w: 0.4, h: 0.26, yaw: s * 0.2, pitch: 0.3 });
  const boom = bakeFree(bm, { mudTop: -2, mudBottom: -4, mud: 0.4 });

  // stick (frame at the boom tip pin, +z along the stick)
  const st = new THREE.Group();
  const SL = FS.stickLen;
  prism(
    st,
    P,
    [
      [-1.65, -0.2],
      [-1.6, 0.6],
      [-0.6, 0.85],
      [SL - 0.6, 0.6],
      [SL + 0.35, 0.2],
      [SL + 0.35, -0.45],
      [SL - 0.4, -0.65],
      [0.3, -0.7],
      [-0.6, -0.6],
    ],
    1.3,
    "zy",
    0,
    { bevel: 0.05 },
  );
  for (const s of [-1, 1]) {
    cyl(st, S, 0.33, 0.28, s * 0.72, 0, SL, { axis: "x", seg: 14 });
    cyl(st, S, 0.34, 0.25, s * 0.72, 0, 0, { axis: "x", seg: 14 });
    box(st, D, 0.3, 0.5, 0.6, PIN.stickRod(s).x, PIN.stickRod(s).y, PIN.stickRod(s).z, { r: 0.04 });
    box(st, D, 0.32, 0.45, 0.55, PIN.bucketBase(s).x, PIN.bucketBase(s).y - 0.1, PIN.bucketBase(s).z, { r: 0.04 });
    beam(st, P, v3(s * 0.67, 0.0, -0.4), v3(s * 0.67, -0.05, SL - 0.3), 0.05, 0.16);
  }
  const stick = bakeFree(st, { mudTop: -1, mudBottom: -4, mud: 0.6 });

  // bucket body (back plate, roof, upper sides; fixed to the stick) and the clam (floor, lip, lower sides)
  const yz = (x: number, p: readonly [number, number]) => v3(x, p[0], p[1]);
  const bw = { mud: 1.5, dust: 1.2 };
  const X = BK.x;
  const bk = new THREE.Group();
  plate(bk, P, [yz(-X, BK.A), yz(X, BK.A), yz(X, BK.B), yz(-X, BK.B)], 0.14, { out: v3(0, -1, 0), wear: bw });
  plate(bk, P, [yz(-X, BK.A), yz(X, BK.A), yz(X, BK.E), yz(-X, BK.E)], 0.12, { out: v3(0, 0, -1), wear: bw });
  for (const s of [-1, 1]) {
    plate(bk, P, [yz(s * X, BK.A), yz(s * X, BK.B), yz(s * X, [0.25, 3.2]), yz(s * X, BK.E)], 0.12, { out: v3(s, 0, 0), wear: bw });
    cyl(bk, S, 0.33, 0.3, s * 0.72, 0, 0, { axis: "x", seg: 14 });
    box(bk, D, 0.3, 0.5, 0.5, PIN.bucketRod(s).x, PIN.bucketRod(s).y, PIN.bucketRod(s).z, { r: 0.04 });
    box(bk, D, 0.3, 0.45, 0.5, PIN.clamBase(s).x, PIN.clamBase(s).y, PIN.clamBase(s).z, { r: 0.04 });
    cyl(bk, D, 0.28, 0.3, s * (X + 0.05), HINGE.y, HINGE.z, { axis: "x", seg: 14 });
  }
  for (const zz of [0.8, 2.0]) beam(bk, W, v3(-X + 0.1, BK.A[0] - 0.08, zz), v3(X - 0.1, BK.A[0] - 0.08, zz), 0.14, 0.12);
  box(bk, D, 2.1, 0.9, 0.9, 0, -0.1, -0.15, { r: 0.06 });
  const bucket = bakeFree(bk, { mudTop: 2, mudBottom: -2, mud: 0.8 });

  const cl = new THREE.Group();
  const rel = (p: readonly [number, number]): [number, number] => [p[0] - BK.E[0], p[1] - BK.E[1]];
  const Xc = X + 0.12;
  for (const s of [-1, 1])
    plate(cl, P, [yz(s * Xc, rel(BK.E)), yz(s * Xc, rel([-0.2, 3.2])), yz(s * Xc, rel(BK.C)), yz(s * Xc, rel(BK.D))], 0.12, { out: v3(s, 0, 0), wear: bw });
  plate(cl, P, [yz(-Xc, rel([-0.3, 3.2])), yz(Xc, rel([-0.3, 3.2])), yz(Xc, rel(BK.C)), yz(-Xc, rel(BK.C))], 0.14, { out: v3(0, 0, 1), wear: bw });
  plate(cl, P, [yz(-Xc, rel(BK.C)), yz(Xc, rel(BK.C)), yz(Xc, rel(BK.D)), yz(-Xc, rel(BK.D))], 0.14, { out: v3(0, 1, 1), wear: bw });
  // wear bars under the floor, the cast lip and the teeth
  for (const yy of [0.4, 1.3]) beam(cl, W, v3(-X, rel([yy, 3.28])[0], rel([yy, 3.28])[1]), v3(X, rel([yy, 3.28])[0], rel([yy, 3.28])[1]), 0.14, 0.12);
  const lip = rel(BK.D);
  box(cl, W, X * 2 + 0.4, 0.45, 0.5, 0, lip[0] - 0.05, lip[1], { rx: -0.5, r: 0.06 });
  const nT = 7;
  for (let i = 0; i < nT; i++) {
    const x = lerp(-X + 0.4, X - 0.4, i / (nT - 1));
    const t = node(cl, x, lip[0] + 0.1, lip[1] - 0.05, { rx: -0.35 });
    box(t, W, 0.3, 0.5, 0.36, 0, 0.18, 0, { r: 0.04 });
    const tip = cyl(t, S, 0.18, 0.5, 0, 0.62, 0, { seg: 4, r2: 0.04 });
    tip.rotation.y = Math.PI / 4;
    tip.scale.set(1, 1, 0.75);
  }
  for (const s of [-1, 1]) box(cl, D, 0.3, 0.45, 0.5, PIN.clamRod(s).x, PIN.clamRod(s).y, PIN.clamRod(s).z, { r: 0.04 });
  const clam = bakeFree(cl, { mudTop: 2, mudBottom: -3, mud: 0.9 });

  return { lower, upper, boom, stick, bucket, clam, paint: P, rams: new Map(), lens };
}

/** Builds a hydraulic face shovel with a bottom-dump bucket. */
export function makeFaceShovel(opts: { colour?: string } = {}): FaceShovel {
  const colour = opts.colour ?? COLOURS.yellow;
  let tpl = faceTemplates.get(colour);
  if (!tpl) {
    tpl = buildFaceTemplate(colour);
    faceTemplates.set(colour, tpl);
  }
  const T = tpl;
  const lens = lenses();
  const group = new THREE.Group();
  group.name = "face-shovel";
  group.add(instance(T.lower));
  const tracks: Track[] = [];
  for (const s of [-1, 1]) {
    const t = makeTrack(FS.track);
    t.group.position.x = s * FS.trackX;
    group.add(t.group);
    tracks.push(t);
  }
  const upper = new THREE.Group();
  upper.add(instance(T.upper));
  group.add(upper);
  const boom = new THREE.Group();
  boom.position.set(0, FS.foot.y, FS.foot.z);
  boom.add(instance(T.boom));
  upper.add(boom);
  const stick = new THREE.Group();
  stick.position.set(0, 0, FS.boomLen);
  stick.add(instance(T.stick));
  boom.add(stick);
  const bucket = new THREE.Group();
  bucket.position.set(0, 0, FS.stickLen);
  bucket.add(instance(T.bucket));
  stick.add(bucket);
  const clamG = new THREE.Group();
  clamG.position.copy(HINGE);
  clamG.add(instance(T.clam));
  bucket.add(clamG);
  swapLenses(group, T.lens, lens);
  const pin = (parent: THREE.Object3D, p: V3) => {
    const o = new THREE.Object3D();
    o.position.copy(p);
    parent.add(o);
    return o;
  };
  const state: FaceShovelState = { swing: 0, boom: 0.45, stick: 0.5, bucket: 0.5, clam: 0, lights: false, distance: 0 };
  const pose = () => {
    upper.rotation.y = state.swing;
    boom.rotation.x = -lerp(FS.boom[0], FS.boom[1], clamp01(state.boom));
    stick.rotation.x = -lerp(FS.stick[0], FS.stick[1], clamp01(state.stick));
    bucket.rotation.x = -lerp(FS.bucket[0], FS.bucket[1], clamp01(state.bucket));
    clamG.rotation.x = -clamp01(state.clam) * FS.clam;
    group.updateMatrixWorld(true);
  };
  // each ram is sized from its own stroke: sweep the control that moves it, measure pin distances
  const rams: Ram[] = [];
  const pa = new THREE.Vector3();
  const pb = new THREE.Vector3();
  const addRam = (name: string, key: "boom" | "stick" | "bucket" | "clam", r: number, a: THREE.Object3D, b: THREE.Object3D) => {
    let parts = T.rams.get(name);
    if (!parts) {
      let lo = Infinity;
      let hi = 0;
      for (let i = 0; i <= 24; i++) {
        state[key] = i / 24;
        pose();
        a.getWorldPosition(pa);
        b.getWorldPosition(pb);
        const d = pa.distanceTo(pb);
        lo = Math.min(lo, d);
        hi = Math.max(hi, d);
      }
      state[key] = key === "clam" ? 0 : 0.5;
      const lb = Math.max(0.4, lo * 0.92 - r * 1.2);
      const lr = Math.min(lo * 0.9, Math.max(hi + 0.35 - lb - r * 1.2, 0.4));
      parts = ramParts(T.paint, r, lb, lr);
      T.rams.set(name, parts);
    }
    const barrel = instance(parts.barrel);
    const rod = instance(parts.rod);
    upper.add(barrel, rod);
    rams.push({ a, b, barrel, rod });
  };
  for (const s of [-1, 1]) {
    addRam(`boom${s}`, "boom", 0.3, pin(upper, PIN.boomBase(s)), pin(boom, PIN.boomRod(s)));
    addRam(`stick${s}`, "stick", 0.25, pin(boom, PIN.stickBase(s)), pin(stick, PIN.stickRod(s)));
    addRam(`bucket${s}`, "bucket", 0.24, pin(stick, PIN.bucketBase(s)), pin(bucket, PIN.bucketRod(s)));
    addRam(`clam${s}`, "clam", 0.15, pin(bucket, PIN.clamBase(s)), pin(clamG, PIN.clamRod(s)));
  }
  const bucketAnchor = pin(bucket, v3(0, 1.3, 1.6));
  const apply = () => {
    pose();
    for (const t of tracks) t.setDistance(state.distance);
    for (const r of rams) placeRam(r, upper);
    lens.set(state.lights);
  };
  apply();
  return {
    group,
    state,
    lenses: lens,
    anchors: { bucket: bucketAnchor, upper },
    set(s) {
      Object.assign(state, s);
      apply();
    },
    update() {
      /* driven by set() */
    },
  };
}

// ───────────────────────────── trailing cable ─────────────────────────────

export type TrailingCableSpec = { points: THREE.Vector3[] } | { from: THREE.Vector3; to: THREE.Vector3; slack?: number; seed?: number };

/**
 * A heavy trailing cable lying on the ground in lazy S-curves (with the odd loop), rising at `from`
 * into a coupler sled and at `to` into the machine's plug. With `points`, the curve follows them.
 */
export function makeTrailingCable(o: TrailingCableSpec, radius = 0.06): { group: THREE.Group } {
  const src = new THREE.Group();
  const r0 = radius;
  let pts: V3[];
  let a: V3;
  let b: V3;
  if ("points" in o) {
    pts = o.points.map((p) => p.clone());
    a = pts[0]!.clone();
    b = pts[pts.length - 1]!.clone();
  } else {
    const r = rng(o.seed ?? 1);
    a = o.from.clone().setY(0);
    b = o.to.clone().setY(0);
    const d = b.clone().sub(a);
    const len = d.length();
    const dir = d.clone().normalize();
    const side = new THREE.Vector3(-dir.z, 0, dir.x);
    const slack = o.slack ?? 0.25;
    const amp = Math.min(6, len * 0.09) * (0.6 + slack * 1.6);
    const f1 = 0.8 + r() * 1.2;
    const f2 = 2.5 + r() * 2;
    const p1 = r() * Math.PI * 2;
    const p2 = r() * Math.PI * 2;
    const n = Math.max(10, Math.ceil(len / 1.2));
    const loopAt = len > 25 ? 0.35 + r() * 0.3 : -1;
    pts = [];
    for (let i = 0; i <= n; i++) {
      const t = i / n;
      const env = Math.sin(Math.PI * t) ** 0.6;
      const off = (Math.sin(t * Math.PI * 2 * f1 + p1) * amp + Math.sin(t * Math.PI * 2 * f2 + p2) * amp * 0.25) * env;
      const p = a.clone().addScaledVector(dir, len * t).addScaledVector(side, off);
      p.y = r0;
      pts.push(p);
      if (loopAt > 0 && t < loopAt && (i + 1) / n >= loopAt) {
        // a flat loop: a circle tangent to the path, the second pass lifted over the first
        const R = 1.1 + r() * 0.5;
        const c = p.clone().addScaledVector(side, R * (r() < 0.5 ? 1 : -1));
        const start = Math.atan2(p.z - c.z, p.x - c.x);
        const sgn = (c.clone().sub(p).dot(side) > 0 ? -1 : 1) * 1;
        for (let k = 1; k <= 11; k++) {
          const ang = start + sgn * (k / 12) * Math.PI * 2;
          const q = new THREE.Vector3(c.x + Math.cos(ang) * R, r0, c.z + Math.sin(ang) * R);
          if (k >= 10) q.y = r0 * 2.6;
          q.addScaledVector(dir, (k / 12) * 0.9);
          pts.push(q);
        }
      }
    }
  }
  // ends: rise into the coupler sled at a and the machine plug at b
  const da = (pts[1] ?? b).clone().sub(a).setY(0).normalize();
  const db = b.clone().sub(pts[pts.length - 2] ?? a).setY(0).normalize();
  const head = [a.clone().addScaledVector(da, -0.5).setY(0.45), a.clone().addScaledVector(da, 0.2).setY(0.25)];
  const tail = [b.clone().addScaledVector(db, -0.4).setY(0.35), b.clone().setY(1.0)];
  const all = [...head, ...pts.slice(1, -1), ...tail];
  tube(src, cableMat(), all, r0, { seg: all.length * 6, radial: 10 });
  const D = mats.dark();
  // black rubber strain-relief boots where the cable leaves the coupler and enters the plug
  tube(src, D, [head[0]!, head[1]!, a.clone().addScaledVector(da, 1.4).setY(r0 * 1.3)], r0 * 1.4, { seg: 16, radial: 10 });
  tube(src, D, [b.clone().addScaledVector(db, -1.4).setY(r0 * 1.3), tail[0]!, tail[1]!], r0 * 1.4, { seg: 16, radial: 10 });
  // coupler sled at the source end
  const sled = node(src, a.x - da.x * 1.1, 0, a.z - da.z * 1.1, { ry: Math.atan2(da.x, da.z) });
  box(sled, mats.yellow(), 1.2, 0.18, 2.0, 0, 0.09, 0, { r: 0.04 });
  for (const s of [-1, 1]) box(sled, D, 0.12, 0.2, 2.1, s * 0.55, 0.08, 0, { r: 0.03 });
  box(sled, cableMat(), 0.7, 0.55, 0.9, 0, 0.45, 0.3, { r: 0.08 });
  cyl(sled, D, 0.14, 0.4, 0, 0.45, 0.85, { axis: "z", seg: 12 });
  box(sled, mats.yellow(), 0.5, 0.5, 0.5, 0, 0.43, -0.5, { r: 0.06 });
  // plug at the machine end
  const plug = node(src, b.x, 1.0, b.z, { ry: Math.atan2(db.x, db.z) });
  cyl(plug, D, r0 * 2.2, 0.7, 0, 0, 0.1, { axis: "z", seg: 14 });
  cyl(plug, mats.steel(), r0 * 2.5, 0.12, 0, 0, 0.5, { axis: "z", seg: 14 });
  const group = bakeFree(src, { mudTop: 0.3, mudBottom: 0, mud: 0.4, dust: 0.7 });
  group.name = "trailing-cable";
  return { group };
}
