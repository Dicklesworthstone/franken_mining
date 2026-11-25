/**
 * Light vehicles and site lighting: a mine-spec dual-cab pickup and a trailer-mounted LED light tower.
 * Generic, no lettering. Metres, y up, +z forward, origin on the ground under the vehicle's centre.
 */
import * as THREE from "three";
import {
  bakeFree,
  bar,
  beam,
  bottle,
  box,
  clamp01,
  COLOURS,
  cyl,
  grime,
  instance,
  lamp,
  lathe,
  lenses,
  mats,
  mirror,
  node,
  paint,
  prism,
  sphere,
  tube,
  v3,
  type Lenses,
  type TyreSpec,
} from "./fleet-kit";
import { ackermann, addTyre, arcBand, archLip, bakeWheels } from "./fleet-underground-parts";

function swapLenses(root: THREE.Object3D, from: Lenses, to: Lenses) {
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh) return;
    if (m.material === from.white) m.material = to.white;
    else if (m.material === from.amber) m.material = to.amber;
    else if (m.material === from.red) m.material = to.red;
  });
}

let flagMat: THREE.MeshStandardMaterial | null = null;
function flagMaterial() {
  if (!flagMat) {
    flagMat = grime(new THREE.MeshStandardMaterial({ color: "#ff5a0a", roughness: 0.75, side: THREE.DoubleSide, emissive: "#ff4a00", emissiveIntensity: 0.08 }), {
      mudAmount: 0,
      dustAmount: 0.4,
      variation: 0.5,
    });
    flagMat.name = "flag";
  }
  return flagMat;
}

/** Pressed-steel light-truck wheel face around x, facing +x. */
function steelWheel(parent: THREE.Object3D, mat: THREE.Material, nut: THREE.Material, rimR: number, w: number) {
  const prof: [number, number][] = [
    [rimR + 0.025, w * 0.42],
    [rimR + 0.025, w * 0.5],
    [rimR - 0.005, w * 0.52],
    [rimR - 0.02, w * 0.44],
    [rimR * 0.82, w * 0.4],
    [rimR * 0.7, w * 0.3],
    [rimR * 0.45, w * 0.3],
    [rimR * 0.36, w * 0.38],
    [rimR * 0.2, w * 0.4],
    [0.001, w * 0.42],
  ];
  lathe(parent, mat, prof, 0, 0, 0, { axis: "x", seg: 28 });
  // inner barrel seen through the dish
  lathe(parent, mat, [[rimR - 0.02, w * 0.4], [rimR - 0.02, -w * 0.45]], 0, 0, 0, { axis: "x", seg: 24 });
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2;
    cyl(parent, nut, 0.013, 0.03, w * 0.41, Math.cos(a) * rimR * 0.27, Math.sin(a) * rimR * 0.27, { axis: "x", seg: 6 });
    // ventilation holes as dark ovals in the dish
    const b = a + Math.PI / 6;
    box(parent, mats.black(), 0.01, 0.05, 0.035, w * 0.3 + 0.005, Math.cos(b) * rimR * 0.58, Math.sin(b) * rimR * 0.58, { rx: b, r: 0.012 });
  }
  cyl(parent, nut, rimR * 0.16, 0.03, w * 0.43, 0, 0, { axis: "x", seg: 16 });
}

// ───────────────────────────── pickup ─────────────────────────────

export interface PickupState {
  speed: number;
  distance: number;
  /** Average front-wheel steer, rad; positive turns left (+x). */
  steer: number;
  lights: boolean;
}
export interface Pickup {
  group: THREE.Group;
  state: PickupState;
  set(s: Partial<PickupState>): void;
  update(dt: number): void;
  lenses: Lenses;
  anchors: { headlights: THREE.Object3D[] };
}

const PU = {
  tyre: { od: 0.79, width: 0.265, rim: 0.432, pattern: "road", lugs: 34, depth: 0.018, segments: 48 } as TyreSpec,
  axleY: 0.39,
  trackX: 0.775,
  frontZ: 1.6,
  rearZ: -1.58,
};

interface PickupTemplate {
  body: THREE.Group;
  wheel: THREE.Group;
  rearAxle: THREE.Group;
  lens: Lenses;
}
const pickupTemplates = new Map<string, PickupTemplate>();

function buildPickup(colour: string): PickupTemplate {
  const W = paint(colour, { coat: 0.8, rough: 0.32, variation: 0.5 });
  const K = mats.black();
  const D = mats.dark();
  const S = mats.steel();
  const C = mats.chrome();
  const G = mats.glass();
  const GV = mats.galv();
  const lens = lenses();
  const b = new THREE.Group();
  const fz = PU.frontZ;
  const ay = PU.axleY;
  const archR = 0.47;
  // lower body: bonnet, front guards and cab lower in one rounded side profile, cut for the front arch
  const arch: [number, number][] = [];
  const t0 = Math.PI - Math.asin(0.06 / archR);
  const t1 = Math.asin(0.06 / archR);
  for (let i = 0; i <= 12; i++) {
    const t = t0 + ((t1 - t0) * i) / 12;
    arch.push([fz + Math.cos(t) * archR, ay + Math.sin(t) * archR]);
  }
  const lower: [number, number][] = [
    [2.36, 0.45],
    [2.47, 0.52],
    [2.5, 0.62],
    [2.52, 0.86],
    [2.5, 0.98],
    [2.42, 1.04],
    [2.2, 1.09],
    [1.6, 1.13],
    [1.0, 1.16],
    [0.6, 1.18],
    [-0.62, 1.2],
    [-0.66, 1.16],
    [-0.67, 0.52],
    [-0.62, 0.45],
    ...arch,
  ];
  prism(b, W, lower, 1.86, "zy", 0, { bevel: 0.075 });
  // engine bay floor and wheel-well liners behind the arch cut
  box(b, D, 1.1, 0.5, 1.0, 0, 0.68, fz, { r: 0.04 });
  for (const s of [-1, 1]) arcBand(b, K, ay, fz, archR - 0.01, Math.min(s * 0.55, s * 0.92), Math.max(s * 0.55, s * 0.92), 0.08, Math.PI - 0.08, 0.02, 12);
  // greenhouse: dark glass, narrower than the body (tumblehome), raked screen
  const green: [number, number][] = [
    [0.98, 1.17],
    [0.34, 1.82],
    [0.24, 1.88],
    [-0.5, 1.9],
    [-0.62, 1.86],
    [-0.65, 1.2],
  ];
  prism(b, G, green, 1.64, "zy", 0, { bevel: 0.05 });
  // roof, pillars and frames in body colour
  prism(b, W, [[0.3, 1.86], [0.22, 1.935], [-0.52, 1.95], [-0.66, 1.92], [-0.66, 1.87], [-0.5, 1.89], [0.24, 1.875]], 1.69, "zy", 0, { bevel: 0.03 });
  for (const s of [-1, 1]) {
    beam(b, W, v3(s * 0.8, 1.17, 0.99), v3(s * 0.79, 1.89, 0.27), 0.07, 0.08, undefined, 0.02); // A pillar
    box(b, W, 0.06, 0.7, 0.12, s * 0.815, 1.55, -0.02, { r: 0.015 }); // B pillar
    box(b, W, 0.06, 0.72, 0.18, s * 0.815, 1.55, -0.6, { r: 0.02 }); // C pillar
    box(b, W, 0.05, 0.05, 1.6, s * 0.825, 1.215, 0.16, { r: 0.012 }); // window sill
    // door shut lines, handles, mirrors, step
    for (const z of [0.97, -0.02, -0.64]) box(b, K, 0.01, 0.68, 0.012, s * 0.935, 0.83, z, { r: 0.003 });
    for (const z of [0.55, -0.3]) box(b, K, 0.02, 0.035, 0.14, s * 0.94, 1.08, z, { r: 0.008 });
    mirror(b, K, C, v3(s * 0.86, 1.24, 0.9), v3(s * 1.05, 1.36, 0.86), 0.24, s * 0.08);
    box(b, K, 0.18, 0.05, 1.5, s * 0.95, 0.36, 0.18, { r: 0.015 }); // side step
    for (const z of [0.75, -0.4]) box(b, K, 0.08, 0.08, 0.06, s * 0.9, 0.4, z, { r: 0.01 });
    // flared arch lip on the front guard
    archLip(b, K, s * 0.935, ay, fz, archR, 0.06, 0.05, Math.PI - 0.05, 0.06, 12);
    // reflective strip along the doors
    box(b, mats.reflector(), 0.006, 0.05, 1.5, s * 0.936, 0.66, 0.16, { r: 0.002 });
  }
  // cab back panel and rear window
  box(b, W, 1.66, 0.7, 0.03, 0, 1.55, -0.665, { r: 0.012 });
  box(b, G, 1.1, 0.36, 0.02, 0, 1.62, -0.685, { r: 0.01 });
  // bonnet scoop and grille, headlights, indicators
  box(b, W, 0.62, 0.07, 0.46, 0, 1.165, 1.55, { r: 0.03, rx: -0.04 });
  box(b, K, 0.5, 0.025, 0.06, 0, 1.17, 1.79, { r: 0.008 });
  box(b, K, 0.86, 0.25, 0.04, 0, 0.87, 2.515, { r: 0.02 });
  for (let i = 0; i < 4; i++) box(b, D, 0.84, 0.018, 0.05, 0, 0.785 + i * 0.058, 2.525, { r: 0.005 });
  box(b, C, 0.88, 0.025, 0.03, 0, 0.995, 2.5, { r: 0.008 }); // grille top trim
  box(b, K, 1.2, 0.09, 0.03, 0, 0.66, 2.505, { r: 0.012 }); // lower intake
  for (const s of [-1, 1]) {
    // swept headlamp cluster wrapping round the corner, indicator below
    lamp(b, K, lens.white, s * 0.67, 0.905, 2.5, { w: 0.38, h: 0.15, d: 0.06, yaw: s * 0.15 });
    cyl(b, C, 0.04, 0.02, s * 0.6, 0.905, 2.52, { axis: "z", seg: 12 }); // projector bezels
    cyl(b, C, 0.035, 0.02, s * 0.74, 0.905, 2.51, { axis: "z", seg: 12 });
    lamp(b, D, lens.amber, s * 0.885, 0.9, 2.478, { w: 0.08, h: 0.1, d: 0.04, yaw: s * 0.78 });
    lamp(b, D, lens.white, s * 0.66, 0.7, 2.52, { w: 0.12, h: 0.05, d: 0.03 });
  }
  // steel bull bar with driving lights and a UHF whip
  const BB = mats.black();
  const bw = { mud: 0.35, dust: 0.6 };
  box(b, BB, 1.92, 0.15, 0.13, 0, 0.6, 2.66, { r: 0.035, wear: bw });
  for (const s of [-1, 1]) box(b, BB, 0.34, 0.13, 0.12, s * 0.92, 0.6, 2.55, { r: 0.03, ry: s * 0.5, wear: bw });
  box(b, BB, 1.3, 0.035, 0.4, 0, 0.42, 2.3, { r: 0.01, rx: 0.1, wear: bw }); // bash plate
  box(b, W, 1.82, 0.16, 0.1, 0, 0.55, 2.47, { r: 0.04 }); // body-colour bumper behind the bar
  for (const s of [-1, 1]) {
    bar(b, BB, v3(s * 0.32, 0.62, 2.68), v3(s * 0.3, 1.07, 2.63), 0.032, 10);
    tube(b, BB, [v3(s * 0.3, 0.98, 2.64), v3(s * 0.6, 0.97, 2.62), v3(s * 0.86, 0.86, 2.56), v3(s * 0.95, 0.66, 2.5)], 0.028, { radial: 8 });
    lamp(b, D, lens.white, s * 0.5, 1.08, 2.66, { w: 0.18, d: 0.08, round: true });
  }
  tube(b, BB, [v3(-0.3, 1.07, 2.63), v3(0, 1.09, 2.64), v3(0.3, 1.07, 2.63)], 0.032, { radial: 8 });
  for (const s of [-1, 1]) box(b, mats.red(), 0.05, 0.06, 0.1, s * 0.55, 0.5, 2.62, { r: 0.01 }); // recovery points
  bar(b, K, v3(-0.8, 0.98, 2.6), v3(-0.8, 2.3, 2.55), 0.006, 4, 0.003);
  // snorkel up the right A-pillar
  tube(b, K, [v3(-0.92, 0.98, 1.38), v3(-0.95, 1.12, 1.15), v3(-0.9, 1.5, 0.9), v3(-0.86, 1.86, 0.64)], 0.045, { radial: 10 });
  box(b, K, 0.13, 0.12, 0.22, -0.86, 1.95, 0.62, { r: 0.03 });
  // roof rack with an amber light bar and a strobe
  for (const z of [0.1, -0.5]) bar(b, K, v3(-0.78, 1.95, z), v3(0.78, 1.95, z), 0.015, 6);
  box(b, K, 1.25, 0.08, 0.12, 0, 2.0, 0.12, { r: 0.03 });
  for (let i = 0; i < 6; i++) box(b, i === 0 || i === 5 ? lens.amber : lens.white, 0.17, 0.05, 0.02, -0.5 + i * 0.2, 2.0, 0.19, { r: 0.008 });
  cyl(b, K, 0.06, 0.04, 0.55, 1.97, -0.4, { seg: 10 });
  cyl(b, lens.amber, 0.05, 0.08, 0.55, 2.03, -0.4, { seg: 10 });
  // chassis rails, fuel tank, exhaust (under the body)
  for (const s of [-1, 1]) box(b, D, 0.08, 0.18, 4.9, s * 0.45, 0.52, -0.1, { r: 0.01 });
  box(b, D, 0.55, 0.28, 0.9, -0.55, 0.42, -0.2, { r: 0.04 });
  bar(b, S, v3(0.3, 0.32, 1.0), v3(0.3, 0.32, -2.6), 0.03, 8);
  // flat steel tray with drop sides, headboard and a roll bar
  const tz0 = -0.73;
  const tz1 = -2.75;
  const tc = (tz0 + tz1) / 2;
  const tl = tz0 - tz1;
  box(b, GV, 1.94, 0.06, tl, 0, 0.98, tc, { r: 0.01 });
  for (const s of [-1, 1]) {
    box(b, GV, 0.03, 0.28, tl, s * 0.965, 1.15, tc, { r: 0.01 });
    box(b, mats.reflector(), 0.006, 0.06, tl - 0.3, s * 0.982, 1.1, tc, { r: 0.002 });
    lamp(b, D, lens.red, s * 0.78, 0.85, tz1 - 0.02, { w: 0.22, h: 0.1, d: 0.05, yaw: Math.PI });
    lamp(b, D, lens.amber, s * 0.56, 0.85, tz1 - 0.02, { w: 0.08, h: 0.08, d: 0.04, yaw: Math.PI });
    box(b, K, 0.32, 0.34, 0.02, s * 0.77, 0.38, -2.05, { r: 0.004 }); // mud flaps
  }
  box(b, GV, 1.94, 0.28, 0.03, 0, 1.15, tz1, { r: 0.01 });
  box(b, D, 1.9, 0.12, 0.12, 0, 0.88, tz1 + 0.06, { r: 0.02 });
  box(b, D, 0.12, 0.12, 0.3, 0, 0.48, tz1 - 0.1, { r: 0.02 }); // towbar
  for (let i = 0; i < 4; i++) box(b, D, 1.9, 0.08, 0.08, 0, 0.9, tz0 - 0.2 - i * 0.55, { r: 0.01 });
  // headboard with a mesh guard
  box(b, GV, 1.94, 0.05, 0.05, 0, 1.76, tz0, { r: 0.01 });
  for (let i = 0; i <= 10; i++) bar(b, GV, v3(-0.95 + i * 0.19, 1.0, tz0), v3(-0.95 + i * 0.19, 1.76, tz0), 0.008, 4);
  for (const s of [-1, 1]) box(b, GV, 0.05, 0.8, 0.05, s * 0.95, 1.38, tz0, { r: 0.01 });
  // roll bar
  tube(b, K, [v3(-0.86, 1.0, -0.95), v3(-0.86, 1.82, -0.95), v3(-0.74, 2.02, -0.95), v3(0.74, 2.02, -0.95), v3(0.86, 1.82, -0.95), v3(0.86, 1.0, -0.95)], 0.042, { radial: 10, seg: 40 });
  for (const s of [-1, 1]) bar(b, K, v3(s * 0.84, 1.75, -0.95), v3(s * 0.84, 1.02, -1.75), 0.03, 8);
  // fire extinguisher and a spare in the tray
  bottle(b, mats.red(), S, 0.7, 1.01, -1.2, 0.45, 0.07);
  // buggy whip: tall fibreglass pole on the right rear corner with an orange flag and a tip lamp
  const px = -0.92;
  const pz = tz1 + 0.12;
  box(b, K, 0.08, 0.14, 0.08, px, 1.3, pz, { r: 0.01 });
  bar(b, paint(COLOURS.orange, { coat: 0.6 }), v3(px, 1.36, pz), v3(px, 5.3, pz), 0.013, 6, 0.006);
  cyl(b, lens.amber, 0.018, 0.06, px, 5.33, pz, { seg: 8 });
  const flag = new THREE.BufferGeometry();
  const fp: number[] = [];
  const nx = 6;
  const top = 5.24;
  const wav = (u: number) => Math.sin(u * 5.5) * 0.035 * u;
  for (let i = 0; i < nx; i++) {
    const u0 = i / nx;
    const u1 = (i + 1) / nx;
    const A = [px + wav(u0), top, pz - u0 * 0.48];
    const B = [px + wav(u0), top - 0.3 * (1 - u0), pz - u0 * 0.48];
    const Cc = [px + wav(u1), top - 0.3 * (1 - u1) - 0.002, pz - u1 * 0.48];
    const Dd = [px + wav(u1), top, pz - u1 * 0.48];
    fp.push(...A, ...B, ...Cc, ...A, ...Cc, ...Dd);
  }
  flag.setAttribute("position", new THREE.Float32BufferAttribute(fp, 3));
  flag.computeVertexNormals();
  const fm = new THREE.Mesh(flag, flagMaterial());
  b.add(fm);
  const body = bakeFree(b, { mudTop: 0.75, mudBottom: 0.15, mud: 0.6, dust: 0.8 });

  // wheels: one front wheel (mirrored per side when placed), rear axle with both wheels and the diff
  const w = new THREE.Group();
  addTyre(w, PU.tyre);
  steelWheel(w, paint("#d9d8d2", { coat: 0.3 }), S, PU.tyre.rim / 2, PU.tyre.width);
  const wheel = bakeWheels(w);
  const ra = new THREE.Group();
  for (const s of [-1, 1]) {
    const n = node(ra, s * PU.trackX, 0, 0, { ry: s > 0 ? 0 : Math.PI });
    addTyre(n, PU.tyre);
    steelWheel(n, paint("#d9d8d2", { coat: 0.3 }), S, PU.tyre.rim / 2, PU.tyre.width);
  }
  const rearAxle = bakeWheels(ra);
  return { body, wheel, rearAxle, lens };
}

/** Builds a mine-spec dual-cab pickup with bull bar, snorkel, light bar, roll bar and buggy whip. */
export function makePickup(opts: { colour?: string } = {}): Pickup {
  const colour = opts.colour ?? COLOURS.white;
  let tpl = pickupTemplates.get(colour);
  if (!tpl) {
    tpl = buildPickup(colour);
    pickupTemplates.set(colour, tpl);
  }
  const lens = lenses();
  const group = new THREE.Group();
  group.name = "pickup";
  const body = instance(tpl.body);
  swapLenses(body, tpl.lens, lens);
  group.add(body);
  // static rear axle housing (doesn't spin)
  const fronts: { steer: THREE.Group; spin: THREE.Group; side: number }[] = [];
  for (const s of [1, -1]) {
    const steer = new THREE.Group();
    steer.position.set(s * PU.trackX, PU.axleY, PU.frontZ);
    const m = new THREE.Group();
    m.rotation.y = s > 0 ? 0 : Math.PI;
    const spin = new THREE.Group();
    spin.add(instance(tpl.wheel));
    m.add(spin);
    steer.add(m);
    group.add(steer);
    fronts.push({ steer, spin, side: s });
  }
  const rear = new THREE.Group();
  rear.position.set(0, PU.axleY, PU.rearZ);
  rear.add(instance(tpl.rearAxle));
  group.add(rear);
  const axleHousing = new THREE.Mesh(new THREE.CylinderGeometry(0.06, 0.06, 1.35, 10).rotateZ(Math.PI / 2), mats.dark());
  axleHousing.position.set(0, PU.axleY, PU.rearZ);
  axleHousing.castShadow = true;
  group.add(axleHousing);
  const anchors = { headlights: [] as THREE.Object3D[] };
  for (const s of [-1, 1]) {
    const a = new THREE.Object3D();
    a.position.set(s * 0.66, 0.84, 2.55);
    group.add(a);
    anchors.headlights.push(a);
  }
  const state: PickupState = { speed: 0, distance: 0, steer: 0, lights: false };
  const apply = () => {
    const roll = state.distance / (PU.tyre.od / 2);
    for (const f of fronts) {
      f.steer.rotation.y = ackermann(state.steer, f.side, PU.frontZ - PU.rearZ, PU.trackX * 2);
      f.spin.rotation.x = f.side > 0 ? roll : -roll;
    }
    rear.rotation.x = roll;
    lens.set(state.lights, 3);
  };
  const p: Pickup = {
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
  return p;
}

// ───────────────────────────── light tower ─────────────────────────────

export interface LightTowerState {
  lights: boolean;
  /** 0 stowed (mast folded along the canopy) .. 0.5 upright .. 1 fully telescoped (≈ 9 m). */
  mast: number;
  /** Lamp head rotation about the mast, rad. */
  yaw: number;
}
export interface LightTower {
  group: THREE.Group;
  state: LightTowerState;
  set(s: Partial<LightTowerState>): void;
  update(dt: number): void;
  lenses: Lenses;
  /** Lamp centres; local +z is each beam's direction. */
  anchors: { lamps: THREE.Object3D[] };
}

const LT = {
  pivot: { y: 2.0, z: -1.3 },
  sec: [0.17, 0.14, 0.11, 0.085],
  secLen: 2.0,
  overlap: 0.45,
  tyre: { od: 0.62, width: 0.185, rim: 0.33, pattern: "road", lugs: 26, depth: 0.012, segments: 40 } as TyreSpec,
};

interface TowerTemplate {
  base: THREE.Group;
  secs: THREE.Group[];
  head: THREE.Group;
  leg: THREE.Group;
  jack: THREE.Group;
  lens: Lenses;
}
const towerTemplates = new Map<string, TowerTemplate>();

function buildTower(colour: string): TowerTemplate {
  const P = paint(colour, { coat: 0.6 });
  const K = mats.black();
  const D = mats.dark();
  const S = mats.steel();
  const GV = mats.galv();
  const lens = lenses();
  const b = new THREE.Group();
  // trailer chassis and drawbar
  for (const s of [-1, 1]) box(b, GV, 0.1, 0.16, 2.8, s * 0.55, 0.48, -0.25, { r: 0.01 });
  for (const z of [1.1, 0.2, -0.8, -1.6]) box(b, GV, 1.2, 0.1, 0.1, 0, 0.48, z, { r: 0.01 });
  for (const s of [-1, 1]) beam(b, GV, v3(s * 0.55, 0.48, 1.1), v3(0, 0.45, 2.55), 0.1, 0.12, undefined, 0.01);
  cyl(b, D, 0.06, 0.25, 0, 0.45, 2.65, { axis: "z", seg: 12 });
  cyl(b, D, 0.075, 0.035, 0, 0.45, 2.8, { axis: "x", seg: 16 });
  // jockey wheel
  cyl(b, GV, 0.035, 0.55, 0.25, 0.42, 2.1, { seg: 10 });
  cyl(b, mats.rubber(), 0.08, 0.06, 0.25, 0.1, 2.1, { axis: "x", seg: 14 });
  // axle and mudguards
  cyl(b, GV, 0.04, 1.6, 0, 0.31, -0.2, { axis: "x", seg: 10 });
  for (const s of [-1, 1]) arcBand(b, K, 0.31, -0.2, 0.4, Math.min(s * 0.7, s * 0.95), Math.max(s * 0.7, s * 0.95), 0.2, Math.PI - 0.2, 0.02, 8);
  // generator canopy: rounded enclosure with louvres, doors, control panel and exhaust
  box(b, D, 1.42, 0.14, 2.56, 0, 0.6, -0.22, { r: 0.02 }); // bunded fuel-tank base
  box(b, P, 1.36, 1.2, 2.45, 0, 1.25, -0.22, { r: 0.07, seg: 2 });
  box(b, P, 1.4, 0.05, 2.5, 0, 1.85, -0.22, { r: 0.02 });
  for (const s of [-1, 1]) {
    // gull-wing service doors with seams, hinges and lockable handles
    for (const z of [0.95, -0.22, -1.39]) box(b, K, 0.01, 1.02, 0.012, s * 0.685, 1.24, z, { r: 0.003 });
    box(b, K, 0.01, 0.012, 2.36, s * 0.685, 1.76, -0.22, { r: 0.003 });
    for (const z of [0.36, -0.8]) {
      box(b, S, 0.03, 0.05, 0.14, s * 0.695, 1.32, z + 0.4, { r: 0.008 });
      box(b, K, 0.02, 0.03, 0.03, s * 0.705, 1.32, z + 0.48, { r: 0.004 });
    }
    // intake louvres at the front, exhaust louvres over the radiator at the back
    for (let i = 0; i < 7; i++) box(b, D, 0.02, 0.035, 0.7, s * 0.69, 0.98 + i * 0.085, 0.36, { r: 0.006 });
    for (let i = 0; i < 7; i++) box(b, D, 0.02, 0.035, 0.7, s * 0.69, 0.98 + i * 0.085, -0.8, { r: 0.006 });
    box(b, mats.reflector(), 0.006, 0.05, 2.2, s * 0.69, 0.75, -0.22, { r: 0.002 });
    // forklift pockets in the base
    for (const z of [0.4, -0.85]) box(b, K, 0.02, 0.08, 0.26, s * 0.715, 0.6, z, { r: 0.006 });
  }
  // rear: radiator grille
  box(b, D, 1.0, 0.7, 0.02, 0, 1.2, -1.45, { r: 0.01 });
  for (let i = 0; i < 9; i++) box(b, K, 0.96, 0.025, 0.03, 0, 0.9 + i * 0.075, -1.465, { r: 0.004 });
  // front: lockable control-panel door with a window, seam and hinges
  box(b, D, 0.62, 0.62, 0.02, 0.24, 1.28, 1.01, { r: 0.012 });
  box(b, mats.glass(), 0.34, 0.22, 0.01, 0.24, 1.38, 1.022, { r: 0.006 });
  box(b, S, 0.04, 0.12, 0.03, 0.5, 1.2, 1.025, { r: 0.008 });
  for (const y of [1.06, 1.5]) box(b, S, 0.03, 0.08, 0.03, -0.08, y, 1.02, { r: 0.006 });
  box(b, K, 0.012, 1.02, 0.01, -0.35, 1.24, 1.008, { r: 0.003 });
  box(b, mats.reflector(), 1.2, 0.05, 0.006, 0, 0.75, 1.008, { r: 0.002 });
  box(b, mats.red(), 0.08, 0.08, 0.03, -0.52, 1.6, 1.015, { r: 0.01 }); // e-stop
  cyl(b, K, 0.05, 0.3, -0.4, 1.95, -1.15, { seg: 10 });
  bar(b, K, v3(-0.4, 2.1, -1.15), v3(-0.4, 2.16, -1.25), 0.05, 8);
  cyl(b, K, 0.06, 0.05, 0.45, 1.86, -0.6, { seg: 10 }); // fuel cap
  sphere(b, S, 0.05, 0, 1.88, -0.3, { seg: 8 }); // lifting eye
  // mast pivot bracket and the forward rest
  for (const s of [-1, 1]) box(b, GV, 0.04, 0.3, 0.3, s * 0.13, LT.pivot.y - 0.05, LT.pivot.z, { r: 0.01 });
  box(b, GV, 0.3, 0.1, 0.08, 0, 1.88, 0.75, { r: 0.01 });
  box(b, D, 0.2, 0.04, 0.12, 0, 1.93, 0.75, { r: 0.01 });
  // winch on the pivot
  cyl(b, D, 0.09, 0.2, 0.25, 1.98, -1.05, { axis: "x", seg: 12 });
  // wheels (static on the trailer)
  for (const s of [-1, 1]) {
    const n = node(b, s * 0.82, 0.31, -0.2, { ry: s > 0 ? 0 : Math.PI });
    steelWheel(n, paint("#d9d8d2", { coat: 0.3 }), S, LT.tyre.rim / 2, LT.tyre.width);
  }
  const base = bakeFree(b, { mudTop: 0.8, mudBottom: 0.1, mud: 0.8, dust: 0.9 });
  // tyres keep their own wear
  const tw = new THREE.Group();
  for (const s of [-1, 1]) {
    const n = node(tw, s * 0.82, 0.31, -0.2, { ry: s > 0 ? 0 : Math.PI });
    addTyre(n, LT.tyre);
  }
  base.add(bakeWheels(tw));
  // mast sections (along +y from their base)
  const secs = LT.sec.map((w, i) => {
    const g = new THREE.Group();
    box(g, GV, w, LT.secLen, w, 0, LT.secLen / 2, 0, { r: 0.01 });
    box(g, GV, w + 0.03, 0.06, w + 0.03, 0, LT.secLen - 0.03, 0, { r: 0.008 });
    if (i === 0) {
      cyl(g, S, 0.05, 0.34, 0, 0.1, 0, { axis: "x", seg: 10 });
      // coiled cable down the mast
      tube(g, K, [v3(w / 2 + 0.03, 0.2, 0), v3(w / 2 + 0.05, 1.0, 0.02), v3(w / 2 + 0.03, 1.9, 0)], 0.015, { radial: 5 });
    }
    return bakeFree(g, { mudTop: 0.2, mudBottom: -0.5, mud: 0.3 });
  });
  // lamp head: T-bar with four LED floods facing +z, tilted down a little
  const h = new THREE.Group();
  box(h, GV, 0.12, 0.25, 0.12, 0, 0.12, 0, { r: 0.01 });
  box(h, GV, 1.7, 0.08, 0.08, 0, 0.28, 0, { r: 0.01 });
  for (const x of [-0.65, -0.22, 0.22, 0.65]) {
    const l = node(h, x, 0.42, 0.05, { rx: 0.5 });
    box(l, D, 0.36, 0.26, 0.1, 0, 0, 0, { r: 0.02 });
    box(l, lens.white, 0.32, 0.22, 0.02, 0, 0, 0.05, { r: 0.008 });
    for (let k = 0; k < 6; k++) box(l, D, 0.33, 0.24, 0.008, 0, 0, -0.06 - k * 0.018, { r: 0.002 });
    box(l, GV, 0.04, 0.16, 0.04, 0, -0.18, 0, { r: 0.005 });
  }
  const head = bakeFree(h, { mudTop: -1, mudBottom: -2, mud: 0 });
  // outrigger leg with a screw jack at its foot, pointing along +z from its pin
  const lg = new THREE.Group();
  box(lg, GV, 0.08, 0.08, 0.9, 0, 0, 0.45, { r: 0.01 });
  cyl(lg, GV, 0.06, 0.12, 0, 0, 0.88, { seg: 12 }); // jack sleeve
  const leg = bakeFree(lg, { mudTop: 0.3, mudBottom: -0.4, mud: 0.8 });
  // screw jack that winds down through the sleeve: origin at the sleeve centre
  const jk = new THREE.Group();
  cyl(jk, S, 0.035, 0.5, 0, -0.08, 0, { seg: 10 });
  cyl(jk, D, 0.09, 0.03, 0, -0.33, 0, { seg: 12 });
  cyl(jk, K, 0.015, 0.14, 0, 0.18, 0, { axis: "x", seg: 6 });
  cyl(jk, K, 0.02, 0.03, 0.07, 0.18, 0, { axis: "x", seg: 8 });
  const jack = bakeFree(jk, { mudTop: -0.1, mudBottom: -0.4, mud: 0.9 });
  return { base, secs, head, leg, jack, lens };
}

/** Builds a trailer-mounted LED light tower. */
export function makeLightTower(opts: { colour?: string } = {}): LightTower {
  const colour = opts.colour ?? COLOURS.yellow;
  let tpl = towerTemplates.get(colour);
  if (!tpl) {
    tpl = buildTower(colour);
    towerTemplates.set(colour, tpl);
  }
  const lens = lenses();
  const group = new THREE.Group();
  group.name = "light-tower";
  group.add(instance(tpl.base));
  const pivot = new THREE.Group();
  pivot.position.set(0, LT.pivot.y, LT.pivot.z);
  group.add(pivot);
  const secs = tpl.secs.map((s) => {
    const g = new THREE.Group();
    g.add(instance(s));
    pivot.add(g);
    return g;
  });
  const headYaw = new THREE.Group();
  const headI = instance(tpl.head);
  swapLenses(headI, tpl.lens, lens);
  headYaw.add(headI);
  pivot.add(headYaw);
  const anchors = { lamps: [] as THREE.Object3D[] };
  for (const x of [-0.65, -0.22, 0.22, 0.65]) {
    const a = new THREE.Object3D();
    a.position.set(x, 0.42, 0.11);
    a.rotation.x = 0.5;
    headYaw.add(a);
    anchors.lamps.push(a);
  }
  const legs: { g: THREE.Group; jack: THREE.Object3D; side: number; dir: number }[] = [];
  for (const [x, z, dir] of [
    [0.55, 0.95, 1],
    [-0.55, 0.95, 1],
    [0.55, -1.5, -1],
    [-0.55, -1.5, -1],
  ] as const) {
    const g = new THREE.Group();
    g.position.set(x, 0.42, z);
    g.add(instance(tpl.leg));
    const jack = instance(tpl.jack);
    jack.position.z = 0.88;
    g.add(jack);
    group.add(g);
    legs.push({ g, jack, side: Math.sign(x), dir });
  }
  const state: LightTowerState = { lights: false, mast: 0, yaw: 0 };
  const apply = () => {
    const m = clamp01(state.mast);
    const up = clamp01(m * 2);
    const tele = clamp01(m * 2 - 1);
    pivot.rotation.x = (1 - up) * (Math.PI / 2);
    // each section rides on the previous one's travel
    secs.forEach((g, i) => (g.position.y = i * tele * (LT.secLen - LT.overlap) + i * 0.02));
    headYaw.position.y = (secs.length - 1) * tele * (LT.secLen - LT.overlap) + LT.secLen + 0.06;
    headYaw.rotation.y = state.yaw;
    // outriggers swing out and jack down as the mast goes up
    const dep = clamp01(m * 4);
    for (const l of legs) {
      const base0 = l.dir > 0 ? 0 : Math.PI;
      l.g.rotation.y = base0 + l.side * l.dir * dep * 1.0;
      l.jack.position.y = (1 - dep) * 0.3 - dep * 0.06;
    }
    lens.set(state.lights, 8);
  };
  const t: LightTower = {
    group,
    state,
    lenses: lens,
    anchors,
    set(s) {
      Object.assign(state, s);
      apply();
    },
    update() {},
  };
  apply();
  return t;
}
