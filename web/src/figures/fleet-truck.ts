/**
 * Ultra-class rigid-frame haul truck, 360–400 t payload. Generic yellow, no lettering.
 *
 * Proportions follow the published envelope of the class (59/80R63 tyres ≈ 4.03 m, length ≈ 15 m,
 * width over the canopy ≈ 9.75 m, canopy ≈ 7.6 m, wheelbase ≈ 7.2 m, body raised ≈ 15.5 m at ≈ 45°).
 * Frame: metres, y up, +z forward, origin on the ground under the truck's centre. The truck's left is +x.
 */
import * as THREE from "three";
import {
  bake,
  bakeFree,
  bar,
  beacon,
  beam,
  bottle,
  box,
  clamp01,
  colourHeap,
  cyl,
  deck,
  heapMaterial,
  instance,
  ladder,
  lamp,
  lenses,
  makeRocks,
  mats,
  mirror,
  node,
  Noise3,
  paint,
  plate,
  rail,
  rim,
  ROCK_PALETTES,
  smoothstep,
  sphere,
  stairs,
  tyreGeometry,
  v3,
  COLOURS,
  type Lenses,
  type RockPalette,
} from "./fleet-kit";

/** Key dimensions, metres. */
export const HAUL = {
  tyre: { od: 4.03, width: 1.47, rim: 1.6 },
  axleY: 1.98,
  frontZ: 3.35,
  rearZ: -3.85,
  frontX: 3.25,
  rearInX: 2.2,
  rearOutX: 3.82,
  deckY: 4.45,
  hinge: { y: 3.05, z: -5.55 },
  dumpMax: (45 * Math.PI) / 180,
  halfW: 4.875,
  length: 15.1,
} as const;

const H = HAUL.hinge;
const floorY = (z: number) => (z < -6.3 ? 3.88 + ((-6.3 - z) / 1.25) * 0.24 : 4.35 + 0.105 * (z + 1.8));
const railY = (z: number) => 6.75 + ((z + 7.0) / 10.1) * 0.22;
const wallX = (y: number, z: number) => 4.15 + clamp01((y - floorY(z)) / (railY(z) - floorY(z))) * 0.62;

export interface HaulTruckOptions {
  /** Build the visible ore heap (default true). */
  payload?: boolean;
  /** Paint colour (default industrial yellow). */
  colour?: string;
  /** Rock palette of the load. */
  ore?: RockPalette;
  /** Seed for the load's shape. */
  seed?: number;
}

export interface HaulTruckState {
  /** Ground speed, m/s, integrated by update(dt) into wheel spin. */
  speed: number;
  /** Distance travelled, m (sets the wheel angle directly). */
  distance: number;
  /** Steering angle of the inside-average front wheel, rad; positive turns left (+x). */
  steer: number;
  /** Body tilt 0..1 about the rear hinge (1 = fully raised, 45°). */
  dump: number;
  /** Visible load 0..1. */
  payload: number;
  /** Head, tail and beacon lights. */
  lights: boolean;
}

export interface HaulTruck {
  group: THREE.Group;
  state: HaulTruckState;
  set(s: Partial<HaulTruckState>): void;
  update(dt: number): void;
  lenses: Lenses;
  /** World-space anchors for scene lights: headlamps (two) at the deck front. */
  anchors: { headlights: THREE.Object3D[]; dumpPoint: THREE.Object3D };
}

interface Template {
  chassis: THREE.Group;
  body: THREE.Group;
  frontWheel: THREE.Group;
  rearWheels: THREE.Group;
  barrel: THREE.Group;
  stage1: THREE.Group;
  stage2: THREE.Group;
  lens: Lenses;
}

const templates = new Map<string, Template>();

function wheelParts(parent: THREE.Object3D, dish: number) {
  const t = HAUL.tyre;
  const tyre = new THREE.Mesh(tyreGeometry({ od: t.od, width: t.width, rim: t.rim, lugs: 30, depth: 0.1 }), mats.rubber());
  tyre.userData.keepWear = true;
  parent.add(tyre);
  rim(parent, mats.rimGrey(), mats.dark(), mats.steel(), { rim: t.rim, width: t.width, dish, bolts: 30, hubR: 0.5 });
}

function buildTemplate(colour: string): Template {
  const P = paint(colour);
  const D = mats.dark();
  const K = mats.black();
  const S = mats.steel();
  const C = mats.chrome();
  const G = mats.glass();
  const GR = mats.grate();
  const RED = mats.red();
  const lens = lenses();
  const T = HAUL;
  const ay = T.axleY;

  // ───────── chassis (static, vehicle frame) ─────────
  const ch = new THREE.Group();
  // main frame rails and cross members
  for (const s of [-1, 1]) {
    box(ch, P, 0.5, 1.15, 11.4, s * 0.78, 2.3, -0.05, { r: 0.06 });
    // front horns rising to the deck
    box(ch, P, 0.42, 1.6, 1.6, s * 0.95, 3.2, 4.4, { r: 0.05 });
    // hinge bosses at the rear
    cyl(ch, P, 0.24, 0.34, s * 1.02, H.y, H.z, { axis: "x", seg: 18 });
    box(ch, P, 0.36, 0.9, 0.7, s * 1.02, H.y - 0.45, H.z, { r: 0.05 });
  }
  for (const z of [-5.2, -2.9, -0.4, 1.7]) box(ch, P, 1.2, 0.55, 0.5, 0, 2.25, z, { r: 0.05 });
  box(ch, P, 2.4, 0.9, 0.5, 0, 2.35, -5.95, { r: 0.06 });
  // rear axle housing: central banjo with diff cover, axle tubes out to the final drives
  cyl(ch, P, 0.95, 1.7, 0, ay, T.rearZ, { axis: "x", seg: 28 });
  sphere(ch, P, 0.85, 0, ay, T.rearZ - 0.35, { seg: 20, sy: 1 });
  for (const s of [-1, 1]) {
    cyl(ch, P, 0.62, 0.75, s * 1.22, ay, T.rearZ, { axis: "x", seg: 24 });
    // rear suspension cylinder: yellow body, chrome rod
    cyl(ch, P, 0.22, 1.0, s * 1.15, 3.1, T.rearZ + 0.9, { seg: 16 });
    cyl(ch, C, 0.13, 0.6, s * 1.15, 2.35, T.rearZ + 0.9, { seg: 12 });
    // A-frame link from axle to frame
    beam(ch, P, v3(s * 0.55, ay - 0.1, T.rearZ + 0.3), v3(s * 0.7, 1.9, 0.9), 0.3, 0.35, undefined, 0.04);
  }
  // tail lights and a rear ladder on the frame end
  for (const s of [-1, 1]) {
    lamp(ch, D, lens.red, s * 0.95, 2.55, -6.21, { w: 0.3, h: 0.16, yaw: Math.PI });
    lamp(ch, D, lens.amber, s * 0.6, 2.55, -6.21, { w: 0.2, h: 0.16, yaw: Math.PI });
  }
  // front suspension struts (inside the front wheels) and spindles
  for (const s of [-1, 1]) {
    cyl(ch, P, 0.32, 1.7, s * 2.1, 3.45, T.frontZ, { seg: 18 });
    cyl(ch, C, 0.2, 0.9, s * 2.1, 2.25, T.frontZ, { seg: 16 });
    cyl(ch, D, 0.42, 0.5, s * 2.15, ay, T.frontZ, { axis: "x", seg: 20 });
    box(ch, P, 0.5, 0.7, 1.3, s * 1.55, 3.9, T.frontZ, { r: 0.06 });
  }
  // steering tie rod
  bar(ch, D, v3(-2.0, 2.0, T.frontZ - 0.8), v3(2.0, 2.0, T.frontZ - 0.8), 0.08, 10);
  // engine and transmission (mostly in shadow, seen between the wheels)
  box(ch, K, 2.0, 1.5, 3.4, 0, 2.75, 3.3, { r: 0.12 });
  box(ch, D, 1.4, 0.8, 2.6, 0, 2.1, 0.1, { r: 0.1 });
  for (let i = 0; i < 5; i++) cyl(ch, S, 0.1, 0.7, (i - 2) * 0.35, 3.8, 1.6 + i * 0.05, { seg: 8 });
  // driveshaft to the rear axle
  bar(ch, S, v3(0, 2.1, -1.2), v3(0, ay + 0.1, T.rearZ + 0.9), 0.13, 12);
  // fuel tank (left) and hydraulic tank (right), between the wheels
  box(ch, P, 1.15, 1.6, 2.5, 2.35, 2.05, -0.35, { r: 0.28, seg: 2 });
  cyl(ch, K, 0.12, 0.12, 2.35, 2.9, 0.35, { seg: 12 });
  box(ch, G, 0.04, 0.9, 0.08, 2.94, 2.0, -0.9, { r: 0.01 });
  box(ch, P, 1.05, 1.35, 2.3, -2.3, 2.25, -0.35, { r: 0.24, seg: 2 });
  for (let i = 0; i < 3; i++) bar(ch, mats.hose(), v3(-1.75, 1.9 + i * 0.12, -1.3), v3(-1.0, 2.0 + i * 0.1, -1.6), 0.04, 6);
  // batteries/steps box on the right ahead of the tank
  box(ch, D, 0.9, 0.7, 0.9, -2.3, 1.55, 1.0, { r: 0.04 });

  // radiator housing, grille and bumper
  box(ch, P, 3.3, 2.75, 1.2, 0, 2.95, 5.75, { r: 0.1, seg: 2 });
  const gz = 6.36;
  box(ch, K, 2.7, 2.2, 0.12, 0, 3.0, gz - 0.02, { r: 0.02 }); // radiator core (dark, recessed)
  for (let i = 0; i < 11; i++) box(ch, D, 2.68, 0.05, 0.12, 0, 2.0 + i * 0.2, gz + 0.06, { r: 0.01 });
  for (const x of [-0.9, 0, 0.9]) box(ch, D, 0.07, 2.2, 0.14, x, 3.0, gz + 0.07, { r: 0.01 });
  // grille surround
  box(ch, P, 3.1, 0.22, 0.24, 0, 4.19, gz + 0.06, { r: 0.04 });
  box(ch, P, 3.1, 0.22, 0.24, 0, 1.81, gz + 0.06, { r: 0.04 });
  for (const s of [-1, 1]) box(ch, P, 0.22, 2.6, 0.24, s * 1.45, 3.0, gz + 0.06, { r: 0.04 });
  // bumper
  box(ch, P, 4.6, 0.85, 0.65, 0, 1.38, 6.5, { r: 0.1, seg: 2 });
  for (const s of [-1, 1]) {
    box(ch, D, 0.42, 0.42, 0.3, s * 1.6, 1.2, 6.9, { r: 0.05 }); // tow hooks
    cyl(ch, D, 0.08, 0.5, s * 1.6, 1.2, 6.95, { axis: "x", seg: 10 });
    lamp(ch, D, lens.white, s * 0.95, 1.5, 6.84, { w: 0.42, h: 0.2 });
  }

  // upper deck over the front wheels: plate, fenders, fascia
  const dy = T.deckY;
  deck(ch, GR, 9.1, 3.3, 0, dy, 4.9, 0.12);
  box(ch, P, 9.2, 0.55, 0.2, 0, dy - 0.33, 6.55, { r: 0.05 }); // front fascia
  for (const s of [-1, 1]) {
    box(ch, P, 0.18, 0.5, 3.4, s * 4.56, dy - 0.3, 4.85, { r: 0.05 }); // side fascia
    // fender skirts curving down behind the front tyres
    box(ch, P, 1.7, 0.12, 0.9, s * 3.25, dy - 0.75, 1.2, { rx: -0.9, r: 0.03 });
    // fascia headlights, two each side
    lamp(ch, D, lens.white, s * 2.55, dy - 0.33, 6.66, { w: 0.5, h: 0.24 });
    lamp(ch, D, lens.white, s * 3.25, dy - 0.33, 6.66, { w: 0.5, h: 0.24 });
    // turn/marker lights at the deck corners
    lamp(ch, D, lens.amber, s * 4.15, dy - 0.33, 6.66, { w: 0.2, h: 0.14 });
  }
  // hood behind the radiator
  box(ch, P, 3.2, 0.32, 1.95, 0, dy + 0.1, 4.25, { r: 0.08 });
  for (let i = 0; i < 6; i++) box(ch, D, 2.6, 0.03, 0.12, 0, dy + 0.27, 3.55 + i * 0.25, { r: 0.01 });

  // cab on the left of the deck
  const cab = node(ch, 2.95, dy, 4.85);
  box(cab, P, 2.0, 2.12, 2.3, 0, 1.06, 0, { r: 0.12, seg: 2 });
  box(cab, P, 2.12, 0.1, 2.42, 0, 2.12, 0, { r: 0.04 }); // roof lip
  // glass: front (raked a touch), sides split by the door pillar, rear
  box(cab, G, 1.78, 1.2, 0.05, 0, 1.38, 1.16, { r: 0.015 });
  for (const s of [-1, 1]) {
    box(cab, G, 0.05, 1.18, 1.0, s * 1.01, 1.38, 0.55, { r: 0.015 });
    box(cab, G, 0.05, 1.18, 0.85, s * 1.01, 1.38, -0.55, { r: 0.015 });
  }
  box(cab, G, 1.6, 0.8, 0.05, 0, 1.55, -1.16, { r: 0.015 });
  // wipers, door handle, grab rails
  bar(cab, K, v3(-0.5, 0.8, 1.2), v3(-0.25, 1.75, 1.2), 0.018, 5);
  bar(cab, K, v3(0.35, 0.8, 1.2), v3(0.6, 1.75, 1.2), 0.018, 5);
  box(cab, S, 0.05, 0.05, 0.2, 1.04, 1.0, 0.0, { r: 0.01 });
  bar(cab, P, v3(1.08, 0.5, 1.05), v3(1.08, 1.7, 1.05), 0.025, 6);
  bar(cab, P, v3(1.08, 0.5, -1.05), v3(1.08, 1.7, -1.05), 0.025, 6);
  // roof: beacon, aircon unit, antenna stub
  beacon(cab, K, lens.amber, -0.55, 2.17, -0.6, 1.1);
  box(cab, P, 1.0, 0.22, 0.8, 0.25, 2.28, -0.5, { r: 0.05 });
  bar(cab, K, v3(0.8, 2.17, 0.85), v3(0.8, 2.45, 0.85), 0.015, 5);
  // work lights on the cab roof front corners
  for (const s of [-1, 1]) lamp(cab, D, lens.white, s * 0.8, 2.25, 1.15, { w: 0.22, h: 0.14, pitch: 0.15 });

  // right deck: air cleaners, HVAC/electrical cabinet, exhaust stacks
  for (const x of [-2.75, -3.8]) {
    cyl(ch, P, 0.38, 1.25, x, dy + 0.63, 5.75, { seg: 24 });
    cyl(ch, K, 0.4, 0.12, x, dy + 1.3, 5.75, { seg: 24 });
    sphere(ch, K, 0.3, x, dy + 1.38, 5.75, { seg: 16, sy: 0.45 });
    bar(ch, K, v3(x, dy + 0.4, 5.4), v3(-1.6, dy + 0.4, 4.6), 0.16, 10);
  }
  box(ch, P, 2.0, 1.35, 1.4, -3.3, dy + 0.68, 4.05, { r: 0.08 });
  for (let i = 0; i < 7; i++) box(ch, D, 1.6, 0.04, 0.03, -3.3, dy + 0.3 + i * 0.13, 4.76, { r: 0.008 });
  for (const x of [-1.25, -0.6]) {
    cyl(ch, K, 0.17, 1.5, x, dy + 0.85, 3.55, { seg: 14 });
    cyl(ch, K, 0.2, 0.3, x, dy + 1.65, 3.55, { seg: 14, r2: 0.15 });
  }
  // fire suppression bottles behind the cab, and on the right deck
  bottle(ch, RED, S, 1.55, dy, 3.75);
  bottle(ch, RED, S, 1.8, dy, 3.75);
  bottle(ch, RED, S, -4.25, dy, 5.0, 0.65, 0.1);

  // diagonal access stairway across the front, landing on the deck by the cab
  const sz = 7.12;
  stairs(ch, P, GR, P, v3(-3.75, 0.72, sz), v3(0.35, dy, sz), 0.78);
  deck(ch, GR, 1.35, 0.95, 0.95, dy, 6.98, 0.1);
  box(ch, P, 1.4, 0.3, 0.12, 0.95, dy - 0.2, 7.46, { r: 0.03 });
  ladder(ch, P, v3(-4.15, 0.22, sz), v3(-4.05, 0.72, sz), 0.7, 0.24);
  deck(ch, GR, 0.6, 0.78, -3.95, 0.72, sz, 0.06);
  // handrails round the deck
  rail(ch, P, [v3(1.55, dy, 7.4), v3(1.55, dy, 6.5), v3(4.5, dy, 6.5), v3(4.5, dy, 3.3)], { h: 1.1 });
  rail(ch, P, [v3(0.35, dy, 7.4), v3(1.55, dy, 7.4)], { h: 1.1 });
  rail(ch, P, [v3(-0.1, dy, 6.5), v3(-4.5, dy, 6.5), v3(-4.5, dy, 3.3)], { h: 1.1 });
  // mirrors: left on a long arm ahead of the cab, right at the deck corner, a convex one centre-front
  mirror(ch, K, C, v3(4.45, dy + 0.9, 6.45), v3(4.95, dy + 1.25, 6.9), 0.6, 0.25);
  mirror(ch, K, C, v3(-4.45, dy + 0.9, 6.45), v3(-4.95, dy + 1.25, 6.9), 0.6, -0.25);
  bar(ch, K, v3(-0.1, dy + 1.1, 6.5), v3(-0.1, dy + 1.5, 6.95), 0.025, 5);
  sphere(ch, C, 0.2, -0.1, dy + 1.55, 7.02, { seg: 14, sy: 0.6 });

  const chassis = bakeFree(ch, { mudTop: 3.4, mudBottom: 0.5, mud: 1, dust: 1 });

  // ───────── dump body (baked in the hinge frame) ─────────
  const bodyRoot = new THREE.Group();
  const b = node(bodyRoot, 0, -H.y, -H.z);
  const t = 0.09;
  const zF = 3.1;
  const zT = -6.3;
  const zE = -7.55;
  const fl = (z: number) => floorY(z);
  // floor and tail chute
  plate(b, P, [v3(-4.15, fl(zF), zF), v3(4.15, fl(zF), zF), v3(4.15, fl(zT), zT), v3(-4.15, fl(zT), zT)], t, { out: v3(0, 1, 0) });
  plate(b, P, [v3(-4.2, fl(zT), zT), v3(4.2, fl(zT), zT), v3(4.25, fl(zE), zE), v3(-4.25, fl(zE), zE)], t, { out: v3(0, 1, 0) });
  // side walls, flared outward toward the top rail
  for (const s of [-1, 1]) {
    const pts = [v3(s * 4.15, fl(zF), zF), v3(s * 4.8, railY(zF) + 0.03, zF), v3(s * 4.8, railY(-6.95), -6.95), v3(s * 4.25, fl(zE) + 0.02, zE), v3(s * 4.2, fl(zT), zT)];
    plate(b, P, pts, t, { out: v3(s, 0, 0) });
    // top rail (box beam), lower rail
    beam(b, P, v3(s * 4.86, railY(zF) + 0.12, zF + 0.05), v3(s * 4.86, railY(-7.0) + 0.12, -7.05), 0.3, 0.34, undefined, 0.05);
    beam(b, P, v3(s * 4.28, fl(zF) + 0.1, zF), v3(s * 4.28, fl(zT) + 0.1, zT), 0.22, 0.26, undefined, 0.04);
    beam(b, P, v3(s * 4.28, fl(zT) + 0.1, zT), v3(s * 4.3, fl(zE) + 0.12, zE + 0.05), 0.22, 0.26, undefined, 0.04);
    // vertical external ribs
    for (const z of [-5.6, -3.6, -1.6, 0.4, 2.3]) {
      const yb = fl(z) + 0.05;
      const yt = railY(z) - 0.02;
      beam(b, P, v3(s * (4.15 + 0.12), yb, z), v3(s * (4.8 + 0.12), yt, z), 0.24, 0.26, v3(0, 0, 1), 0.04);
    }
    // rock ejector bars hanging between the rear duals
    bar(b, K, v3(s * 3.01, fl(-4.8) - 0.05, -4.8), v3(s * 3.01, 2.85, -4.6), 0.06, 8);
    // hinge brackets down to the pivot
    plate(b, P, [v3(s * 1.2, fl(-4.4), -4.4), v3(s * 1.2, fl(-6.3), -6.3), v3(s * 1.2, H.y - 0.25, H.z - 0.35), v3(s * 1.2, H.y - 0.25, H.z + 0.35)], 0.1, { out: v3(s, 0, 0) });
    cyl(b, P, 0.26, 0.2, s * 1.25, H.y, H.z, { axis: "x", seg: 18 });
    // hoist brackets
    plate(b, P, [v3(s * 1.4, fl(0.3), 0.3), v3(s * 1.4, fl(-0.9), -0.9), v3(s * 1.4, 4.05, -0.45), v3(s * 1.4, 4.05, -0.15)], 0.12, { out: v3(s, 0, 0) });
    cyl(b, P, 0.2, 0.36, s * 1.4, 4.15, -0.3, { axis: "x", seg: 16 });
  }
  // front wall, leaning forward to the canopy
  plate(b, P, [v3(-4.15, fl(zF), zF), v3(-4.8, 7.0, zF + 0.25), v3(4.8, 7.0, zF + 0.25), v3(4.15, fl(zF), zF)], t, { out: v3(0, 0, 1) });
  for (const y of [5.5, 6.3]) beam(b, P, v3(-4.5, y, zF + 0.17 + (y - 4.9) * 0.1), v3(4.5, y, zF + 0.17 + (y - 4.9) * 0.1), 0.22, 0.26, undefined, 0.04);
  // canopy over the cab and deck
  const cz0 = zF + 0.2;
  const cz1 = 7.58;
  plate(b, P, [v3(-4.875, 7.0, cz0), v3(-4.875, 7.32, cz1), v3(4.875, 7.32, cz1), v3(4.875, 7.0, cz0)], 0.1, { out: v3(0, 1, 0) });
  beam(b, P, v3(-4.9, 7.36, cz1), v3(4.9, 7.36, cz1), 0.2, 0.32, undefined, 0.04);
  for (const s of [-1, 1]) beam(b, P, v3(s * 4.9, 7.22, cz0), v3(s * 4.9, 7.4, cz1), 0.2, 0.3, undefined, 0.04);
  for (const x of [-3.6, -1.8, 0, 1.8, 3.6]) beam(b, P, v3(x, 6.82, cz0), v3(x, 7.08, cz1 - 0.15), 0.16, 0.26, undefined, 0.03);
  beam(b, P, v3(-4.7, 6.85, cz0 + 2.3), v3(4.7, 6.98, cz0 + 2.3), 0.14, 0.22, undefined, 0.03);
  // under-floor structure: keel beams sitting on the frame, outer stringers, cross members
  for (const s of [-1, 1]) {
    plate(b, P, [v3(s * 0.75, fl(zF), zF), v3(s * 0.75, fl(-5.0), -5.0), v3(s * 0.75, 3.0, -5.0), v3(s * 0.75, 3.05, 0.6), v3(s * 0.75, 3.6, 1.5), v3(s * 0.75, 4.3, zF)], 0.14, { out: v3(s, 0, 0) });
    for (const x of [2.4, 3.6]) beam(b, P, v3(s * x, fl(zF) - 0.2, zF), v3(s * x, fl(zE) - 0.15, zE + 0.3), 0.18, 0.36, undefined, 0.03);
  }
  for (let z = -6.8; z < 3; z += 1.45) beam(b, P, v3(-4.1, fl(z) - 0.17, z), v3(4.1, fl(z) - 0.17, z), 0.2, 0.32, undefined, 0.03);
  box(b, P, 1.7, 0.25, 5.4, 0, 3.08, -2.2, { r: 0.05 }); // body pads on the frame
  const body = bakeFree(bodyRoot, { mudTop: 4.6, mudBottom: 2.6, mud: 0.75, dust: 1 }, new THREE.Matrix4().makeTranslation(0, H.y, H.z));

  // ───────── wheels ─────────
  const fw = new THREE.Group();
  wheelParts(fw, 0.14);
  const frontWheel = bakeKeepTyre(fw);
  const rw = new THREE.Group();
  for (const s of [-1, 1]) {
    const inner = node(rw, s * T.rearInX, 0, 0, { ry: s > 0 ? 0 : Math.PI });
    wheelParts(inner, 0.1);
    const outer = node(rw, s * T.rearOutX, 0, 0, { ry: s > 0 ? 0 : Math.PI });
    wheelParts(outer, 0.36);
  }
  const rearWheels = bakeKeepTyre(rw);

  // ───────── hoist cylinder stages (along +y from the lower pin) ─────────
  const mk = (r: number, len: number, mat: THREE.Material, eye: boolean) => {
    const g = new THREE.Group();
    cyl(g, mat, r, len, 0, len / 2, 0, { seg: 18 });
    if (eye) {
      cyl(g, P, r * 1.05, 0.12, 0, 0.06, 0, { seg: 18 });
      cyl(g, P, 0.16, r * 1.6, 0, 0, 0, { axis: "x", seg: 14 });
    }
    return bakeFree(g, { mudTop: 2.5, mudBottom: 0, mud: 0.5 });
  };
  const barrel = mk(0.26, 2.4, P, true);
  const stage1 = mk(0.2, 2.35, C, false);
  const stage2 = (() => {
    const g = new THREE.Group();
    cyl(g, C, 0.15, 2.3, 0, 1.15, 0, { seg: 16 });
    cyl(g, P, 0.2, 0.18, 0, 2.3, 0, { seg: 16 });
    cyl(g, P, 0.16, 0.36, 0, 2.3, 0, { axis: "x", seg: 14 });
    return bakeFree(g, { mudTop: 0, mudBottom: -1, mud: 0 });
  })();
  return { chassis, body, frontWheel, rearWheels, barrel, stage1, stage2, lens };
}

/** Bake a wheel: the rim parts merge; the tyre keeps its own (cached) geometry and wear. */
function bakeKeepTyre(root: THREE.Group): THREE.Group {
  const tyres: THREE.Mesh[] = [];
  root.updateMatrixWorld(true);
  root.traverse((o) => {
    if ((o as THREE.Mesh).isMesh && o.userData.keepWear) tyres.push(o as THREE.Mesh);
  });
  // merge tyres into one geometry preserving their wear attribute
  const out = new THREE.Group();
  const geos = tyres.map((m) => {
    const g = m.geometry.clone();
    g.applyMatrix4(m.matrixWorld);
    return g;
  });
  for (const m of tyres) m.parent?.remove(m);
  const rims = bake(root, { mudTop: 0.2, mudBottom: -2, mud: 0.6, dust: 0.5 });
  out.add(rims);
  if (geos.length) {
    let n = 0;
    for (const g of geos) n += g.getAttribute("position").count;
    const attrs = ["position", "normal", "uv", "wear"] as const;
    const merged = new THREE.BufferGeometry();
    for (const a of attrs) {
      const size = geos[0]!.getAttribute(a).itemSize;
      const arr = new Float32Array(n * size);
      let o = 0;
      for (const g of geos) {
        const src = g.getAttribute(a).array as Float32Array;
        arr.set(src, o);
        o += src.length;
      }
      merged.setAttribute(a, new THREE.BufferAttribute(arr, size));
    }
    merged.computeBoundingSphere();
    for (const g of geos) g.dispose();
    const mesh = new THREE.Mesh(merged, mats.rubber());
    mesh.castShadow = mesh.receiveShadow = true;
    out.add(mesh);
  }
  return out;
}

function swapLenses(root: THREE.Object3D, from: Lenses, to: Lenses) {
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh) return;
    if (m.material === from.white) m.material = to.white;
    else if (m.material === from.amber) m.material = to.amber;
    else if (m.material === from.red) m.material = to.red;
  });
}

/** Payload heap surface inside the body, in the hinge frame, rebuilt as the load changes. */
function makeLoad(seed: number, ore: RockPalette) {
  const nx = 26;
  const nz = 46;
  const noise = new Noise3(seed);
  const g = new THREE.BufferGeometry();
  const pos = new Float32Array((nx + 1) * (nz + 1) * 3);
  g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  const idx: number[] = [];
  for (let j = 0; j < nz; j++)
    for (let i = 0; i < nx; i++) {
      const a = j * (nx + 1) + i;
      idx.push(a, a + nx + 1, a + 1, a + 1, a + nx + 1, a + nx + 2);
    }
  g.setIndex(idx);
  const z0 = -7.35;
  const z1 = 3.12;
  const full = (x: number, z: number) => {
    const base = railY(z) - 0.15 - smoothstep(-3.8, -7.3, z) * (railY(z) - 4.35);
    const crest = 1.55 * Math.exp(-(((z + 1.0) / 3.7) ** 2));
    const across = Math.max(0, 1 - (x / 4.8) ** 2) ** 0.75;
    return base + crest * across + noise.fbm(x * 0.35, 0, z * 0.35, 3) * 0.22 + noise.noise(x * 1.7, 3, z * 1.7) * 0.06;
  };
  const surface = (u: number, z: number, p: number) => {
    const y = Math.max(floorY(z) + 0.04, full(u * 4.6, z) - (1 - p) * 4.2);
    return y;
  };
  const shape = (p: number) => {
    for (let j = 0; j <= nz; j++) {
      const z = z0 + ((z1 - z0) * j) / nz;
      for (let i = 0; i <= nx; i++) {
        const u = (i / nx) * 2 - 1;
        const y = surface(u, z, p);
        const zz = j === nz ? Math.min(z, 3.1 + (y - floorY(3.1)) * 0.11 - 0.05) : z;
        const k = (j * (nx + 1) + i) * 3;
        pos[k] = u * (wallX(y, zz) - 0.07);
        pos[k + 1] = y - H.y;
        pos[k + 2] = zz - H.z;
      }
    }
    g.getAttribute("position").needsUpdate = true;
    g.computeVertexNormals();
    g.computeBoundingSphere();
  };
  shape(1);
  g.translate(0, H.y, H.z);
  colourHeap(g, ROCK_PALETTES[ore], seed, 0.75);
  g.translate(0, -H.y, -H.z);
  const mesh = new THREE.Mesh(g, heapMaterial(`load:${ore}`));
  mesh.castShadow = mesh.receiveShadow = true;
  // boulders sitting on the heap
  const spots: { u: number; z: number; s: number }[] = [];
  const rocks = makeRocks({
    count: 70,
    seed: seed + 5,
    shapes: 3,
    detail: 1,
    palette: ore,
    place: (_i, r) => {
      const u = (r() * 2 - 1) * 0.9;
      const z = z0 + 0.6 + r() * (z1 - z0 - 1.2);
      const s = 0.3 + r() ** 2 * 0.85;
      spots.push({ u, z, s });
      return { x: 0, y: 0, z: 0, size: s };
    },
  });
  // keep each rock's random orientation and scale, re-seat its position as the load changes
  const base = rocks.meshes.map((m) => {
    const arr: THREE.Matrix4[] = [];
    for (let i = 0; i < m.count; i++) {
      const mm = new THREE.Matrix4();
      m.getMatrixAt(i, mm);
      arr.push(mm);
    }
    return arr;
  });
  // rocks were assigned to meshes in bucket order; rebuild the id → (mesh, slot) mapping by replaying placement order
  const slots: { mesh: number; slot: number }[] = [];
  {
    const counters = rocks.meshes.map(() => 0);
    // makeRocks fills buckets in the order of ids; spots[] is in call order, which is bucket order
    rocks.meshes.forEach((m, k) => {
      for (let i = 0; i < m.count; i++) slots.push({ mesh: k, slot: counters[k]!++ });
    });
  }
  const m4 = new THREE.Matrix4();
  const tr = new THREE.Vector3();
  const q = new THREE.Quaternion();
  const sc = new THREE.Vector3();
  const setLoad = (p: number) => {
    const vis = p > 0.02;
    mesh.visible = vis;
    rocks.group.visible = vis;
    if (!vis) return;
    shape(p);
    spots.forEach((sp, i) => {
      const sl = slots[i]!;
      const bm = base[sl.mesh]![sl.slot]!;
      bm.decompose(tr, q, sc);
      const y = surface(sp.u, sp.z, p);
      const buried = y <= floorY(sp.z) + 0.06;
      tr.set(sp.u * (wallX(y, sp.z) - 0.35 - sp.s * 0.4), y - H.y + sp.s * 0.12, sp.z - H.z);
      if (buried) sc.setScalar(0);
      m4.compose(tr, q, sc);
      rocks.meshes[sl.mesh]!.setMatrixAt(sl.slot, m4);
    });
    for (const m of rocks.meshes) {
      m.instanceMatrix.needsUpdate = true;
      m.computeBoundingSphere();
    }
  };
  const group = new THREE.Group();
  group.add(mesh, rocks.group);
  return { group, setLoad };
}

/**
 * Builds an ultra-class haul truck. Geometry and materials are shared between trucks of the same
 * colour; lenses (lights) and the load are per truck.
 */
export function makeHaulTruck(opts: HaulTruckOptions = {}): HaulTruck {
  const colour = opts.colour ?? COLOURS.yellow;
  let tpl = templates.get(colour);
  if (!tpl) {
    tpl = buildTemplate(colour);
    templates.set(colour, tpl);
  }
  const lens = lenses();
  const group = new THREE.Group();
  group.name = "haul-truck";
  const chassis = instance(tpl.chassis);
  swapLenses(chassis, tpl.lens, lens);
  group.add(chassis);
  // body on its hinge
  const pivot = new THREE.Group();
  pivot.position.set(0, H.y, H.z);
  pivot.add(instance(tpl.body));
  group.add(pivot);
  let load: ReturnType<typeof makeLoad> | null = null;
  if (opts.payload !== false) {
    load = makeLoad(opts.seed ?? 7, opts.ore ?? "ore");
    pivot.add(load.group);
  }
  // front wheels: steer pivot → (mirror for the right) → spin
  const fronts: { steer: THREE.Group; spin: THREE.Group; side: number }[] = [];
  for (const s of [1, -1]) {
    const steer = new THREE.Group();
    steer.position.set(s * HAUL.frontX, HAUL.axleY, HAUL.frontZ);
    const mirrorG = new THREE.Group();
    mirrorG.rotation.y = s > 0 ? 0 : Math.PI;
    const spin = new THREE.Group();
    spin.add(instance(tpl.frontWheel));
    mirrorG.add(spin);
    steer.add(mirrorG);
    group.add(steer);
    fronts.push({ steer, spin, side: s });
  }
  const rear = new THREE.Group();
  rear.position.set(0, HAUL.axleY, HAUL.rearZ);
  rear.add(instance(tpl.rearWheels));
  group.add(rear);
  // hoist cylinders
  const hoists: { root: THREE.Group; s1: THREE.Object3D; s2: THREE.Object3D; x: number }[] = [];
  for (const s of [-1, 1]) {
    const root = new THREE.Group();
    root.position.set(s * 1.4, 1.75, 0.3);
    const brl = instance(tpl.barrel);
    const s1 = instance(tpl.stage1);
    const s2 = instance(tpl.stage2);
    root.add(brl, s1, s2);
    group.add(root);
    hoists.push({ root, s1, s2, x: s * 1.4 });
  }
  // lower hoist mounts on the frame
  const anchors = { headlights: [] as THREE.Object3D[], dumpPoint: new THREE.Object3D() };
  for (const s of [-1, 1]) {
    const a = new THREE.Object3D();
    a.position.set(s * 2.9, HAUL.deckY - 0.33, 6.75);
    group.add(a);
    anchors.headlights.push(a);
  }
  anchors.dumpPoint.position.set(0, 0, -8.5);
  group.add(anchors.dumpPoint);

  const state: HaulTruckState = { speed: 0, distance: 0, steer: 0, dump: 0, payload: opts.payload === false ? 0 : 1, lights: false };
  let shownLoad = -1;
  const upper = new THREE.Vector3();
  const dir = new THREE.Vector3();
  const Y = new THREE.Vector3(0, 1, 0);
  const apply = () => {
    const ang = clamp01(state.dump) * HAUL.dumpMax;
    pivot.rotation.x = -ang;
    // wheels
    const roll = state.distance / (HAUL.tyre.od / 2);
    rear.rotation.x = roll;
    const L = HAUL.frontZ - HAUL.rearZ;
    const tw = HAUL.frontX * 2;
    const st = state.steer;
    for (const f of fronts) {
      let a = st;
      if (Math.abs(st) > 1e-4) {
        const R = L / Math.tan(Math.abs(st));
        const inner = Math.sign(st) === f.side;
        a = Math.sign(st) * Math.atan(L / (R + (inner ? -tw / 2 : tw / 2)));
      }
      f.steer.rotation.y = a;
      f.spin.rotation.x = f.side > 0 ? roll : -roll;
    }
    // hoists: upper pin rides on the body
    const c = Math.cos(ang);
    const sn = Math.sin(ang);
    const uy = 4.15 - H.y;
    const uz = -0.3 - H.z;
    for (const h of hoists) {
      upper.set(h.x, H.y + uy * c + uz * sn, H.z + uz * c - uy * sn);
      dir.copy(upper).sub(h.root.position);
      const len = dir.length();
      h.root.quaternion.setFromUnitVectors(Y, dir.normalize());
      const s1 = Math.min(2.15, Math.max(0.05, len - 4.4));
      h.s1.position.y = s1;
      h.s2.position.y = len - 2.3;
    }
    if (load && state.payload !== shownLoad) {
      shownLoad = state.payload;
      load.setLoad(clamp01(state.payload));
    }
    lens.set(state.lights);
  };
  const truck: HaulTruck = {
    group,
    state,
    lenses: lens,
    anchors,
    set(s) {
      Object.assign(state, s);
      apply();
    },
    update(dt) {
      if (state.speed) {
        state.distance += state.speed * dt;
        apply();
      }
    },
  };
  apply();
  return truck;
}
