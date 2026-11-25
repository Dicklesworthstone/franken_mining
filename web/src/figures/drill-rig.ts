/**
 * A track-mounted diamond core rig and the kit on its pad, at real scale in metres.
 *
 * The rig: steel crawler tracks under a 5.6 m carrier; a diesel power pack with louvres, radiator, exhaust and
 * air cleaner; a wireline winch, a triplex mud pump and the hydraulic tank on the deck; the driller's station with
 * gauges, levers, handrails and a sunshade; four levelling jacks; an 11 m box-section mast on a cradle that pivots
 * at 2.9 m and slides along its own length, raised by two rams; a top-drive head with two motors, a water swivel
 * and hose loops running on guide rails; the foot clamp at the collar, the crown sheave and the wireline over it,
 * and the NQ rod string turning down the hole through the casing.
 *
 * The pad: a rod rack whose rods go down the hole as it deepens, core trays stacking up as runs come out, a
 * logging bench under shade cloth, two mud tanks with the return line from the casing, a water tank trailer, a
 * fuel pod in its bund and a light tower.
 *
 * Frame: origin on the ground under the mast pivot, +z the hole's heading, y up. The mast's angle is the hole's
 * dip (0 stowed back over the carrier, 90 vertical); its rod line meets the ground `collarAhead(dip)` in front.
 * Parts that touch are embedded a little in each other, never flush, so no two faces share a plane.
 */
import * as THREE from "three";
import { makeLightTower } from "./fleet";
import {
  bakeFree,
  bar,
  beacon,
  beam,
  bottle,
  box,
  COLOURS,
  cyl,
  deck,
  lamp,
  lathe,
  lenses,
  makeTrack,
  mats,
  paint,
  prism,
  rail,
  rng,
  sphere,
  tube,
  v3,
  type Lenses,
  type Track,
} from "./fleet-kit";
import { makeCoreTray } from "./miners";

export const CORE_RIG = {
  pivotY: 2.9,
  /** The rod line runs this far in front of the mast's centre line. */
  rodZ: 0.5,
  /** The mast body along its own length, from the pivot (before sliding). */
  foot: -3.6,
  top: 7.4,
  /** The head's travel along the mast body (its chuck). */
  head: { lo: -2.3, hi: 3.4 },
  deckY: 1.05,
  track: { z: -2.6, gauge: 0.95, length: 3.3 },
  jack: { x: 1.12, z: [0.3, -4.95] as const, travel: 0.42 },
  ram: { base: v3(0.3, 1.12, -3.05), mast: v3(0.3, 1.6, -0.33) },
  winch: v3(-0.86, 1.6, -2.35),
  /** NQ rods: 69.9 mm outside diameter, 3 m long. */
  rodR: 0.035,
} as const;

const rad = THREE.MathUtils.degToRad;

/** How far ahead of the rig's origin the rod line meets the ground, for a hole at this dip (degrees). */
export function collarAhead(dip: number) {
  const d = rad(Math.max(dip, 10));
  return (CORE_RIG.rodZ + CORE_RIG.pivotY * Math.cos(d)) / Math.sin(d);
}
/** Where the rod line meets the ground, along the mast from the pivot. */
const groundAlong = (dip: number) => {
  const d = rad(Math.max(dip, 10));
  return -(CORE_RIG.pivotY + CORE_RIG.rodZ * Math.cos(d)) / Math.sin(d);
};
/** The mast's slide that sets its foot clamp just over the collar. */
const slideFor = (dip: number) => groundAlong(dip) + 0.55 / Math.sin(rad(Math.max(dip, 10))) - CORE_RIG.foot;

export interface CoreRigState {
  /** Mast angle from horizontal, degrees: 0 stowed, the hole's dip when drilling. */
  mast: number;
  /** The hole's dip, which sets how far the mast slides down to the collar. */
  dip: number;
  /** 0 slid up for travel, 1 slid down to the collar. */
  slide: number;
  /** The head's chuck along its travel, 0 bottom .. 1 top. */
  head: number;
  /** Rod rotation shown, rad/s. */
  spin: number;
  /** Levelling jacks 0 up .. 1 on the ground. */
  jacks: number;
  /** The rod string from the chuck into the hole. */
  rods: boolean;
  /** Distance tracked, m (turns the track shoes). */
  track: number;
  lights: boolean;
}

export interface CoreRig {
  group: THREE.Group;
  state: CoreRigState;
  set(s: Partial<CoreRigState>): void;
  update(dt: number): void;
  lenses: Lenses;
  /** Points for labels and the camera: the head, the crown and the driller's station. */
  anchors: { head: THREE.Object3D; crown: THREE.Object3D; panel: THREE.Object3D; foot: THREE.Object3D };
}

const UNIT_Y = new THREE.CylinderGeometry(1, 1, 1, 14, 1, false).translate(0, 0.5, 0);
const _a = new THREE.Vector3();
const _b = new THREE.Vector3();
const _q = new THREE.Quaternion();
const Y = new THREE.Vector3(0, 1, 0);

/** Stretches a unit cylinder (radius 1, y 0..1) from a to b with radius r. */
function span(m: THREE.Mesh, a: THREE.Vector3, b: THREE.Vector3, r: number) {
  const d = _b.copy(b).sub(a);
  const len = Math.max(1e-3, d.length());
  m.position.copy(a);
  m.quaternion.copy(_q.setFromUnitVectors(Y, d.multiplyScalar(1 / len)));
  m.scale.set(r, len, r);
}

export function makeCoreRig(): CoreRig {
  const R = CORE_RIG;
  const W = paint(COLOURS.cream, { coat: 0.55, rough: 0.4 });
  const O = paint(COLOURS.orange, { coat: 0.5 });
  const D = mats.dark();
  const K = mats.black();
  const S = mats.steel();
  const C = mats.chrome();
  const GR = mats.grate();
  const RB = mats.rubber();
  const H = mats.hose();
  const lens = lenses();
  const dy = R.deckY;
  const group = new THREE.Group();
  group.name = "core-rig";

  /* ---------- Carrier, power pack, deck machinery, the driller's station ---------- */
  const h = new THREE.Group();
  box(h, D, 1.3, 0.46, 5.6, 0, 0.79, -2.4, { r: 0.05 });
  deck(h, GR, 2.5, 5.6, 0, dy, -2.4, 0.06);
  for (const s of [-1, 1]) box(h, W, 0.08, 0.24, 5.62, s * 1.27, dy - 0.07, -2.4, { r: 0.02 });
  box(h, W, 2.62, 0.24, 0.08, 0, dy - 0.07, 0.42, { r: 0.02 });
  // power pack: enclosure, roof, louvres, rear radiator, exhaust, air cleaner, beacon
  box(h, W, 2.2, 1.45, 1.8, 0, dy + 0.71, -4.25, { r: 0.08, seg: 2 });
  box(h, W, 2.1, 0.08, 1.7, 0, dy + 1.46, -4.25, { r: 0.03 });
  for (const s of [-1, 1]) {
    box(h, D, 0.04, 0.82, 1.24, s * 1.1, dy + 0.86, -4.25, { r: 0.012 });
    for (let k = 0; k < 7; k++) box(h, W, 0.05, 0.035, 1.18, s * 1.13, dy + 0.53 + k * 0.11, -4.25, { rz: s * 0.55, r: 0.006 });
    box(h, O, 0.03, 0.1, 1.84, s * 1.115, dy + 1.3, -4.25, { r: 0.008 });
    box(h, S, 0.04, 0.04, 0.16, s * 1.13, dy + 0.45, -3.6, { r: 0.008 });
  }
  box(h, D, 1.6, 0.95, 0.05, 0, dy + 0.72, -5.17, { r: 0.01 });
  for (let k = 0; k < 11; k++) box(h, W, 0.025, 0.95, 0.03, -0.75 + k * 0.15, dy + 0.72, -5.2, { r: 0.004 });
  cyl(h, K, 0.065, 0.95, 0.7, dy + 1.9, -3.75, { seg: 12 });
  cyl(h, K, 0.09, 0.02, 0.7, dy + 2.39, -3.72, { seg: 14, rx: 0.5 });
  cyl(h, S, 0.16, 0.7, 0.7, dy + 1.65, -4.45, { axis: "z", seg: 16 });
  cyl(h, D, 0.16, 0.55, -0.55, dy + 1.65, -4.0, { axis: "x", seg: 16 });
  cyl(h, D, 0.08, 0.2, -0.85, dy + 1.82, -4.0, { seg: 12 });
  beacon(h, K, lens.amber, -0.85, dy + 1.49, -4.9);
  for (const s of [-1, 1]) lamp(h, D, lens.white, s * 0.62, dy + 1.38, -3.32, { w: 0.3, h: 0.18, pitch: 0.42 });
  // mast rest on the roof
  for (const s of [-1, 1]) box(h, W, 0.12, 0.27, 0.14, s * 0.3, dy + 1.6, -5.0, { r: 0.02 });
  box(h, RB, 0.78, 0.07, 0.26, 0, dy + 1.71, -5.0, { r: 0.015 });
  // A-frame and pivot
  for (const s of [-1, 1]) {
    prism(h, W, [[-1.3, dy - 0.02], [0.45, dy - 0.02], [0.24, R.pivotY + 0.16], [-0.26, R.pivotY + 0.16]], 0.08, "zy", s * 0.5, { bevel: 0.015 });
    cyl(h, W, 0.15, 0.14, s * 0.5, R.pivotY, 0, { axis: "x", seg: 18 });
    box(h, W, 0.12, 0.3, 0.3, s * R.ram.base.x, R.ram.base.y + 0.02, R.ram.base.z, { r: 0.02 });
  }
  cyl(h, S, 0.055, 1.18, 0, R.pivotY, 0, { axis: "x", seg: 12 });
  // wireline winch (left)
  for (const x of [-0.5, -1.22]) box(h, W, 0.06, 0.9, 0.9, x, dy + 0.44, R.winch.z, { r: 0.02 });
  cyl(h, K, 0.24, 0.6, R.winch.x, R.winch.y, R.winch.z, { axis: "x", seg: 20 });
  for (const x of [-0.56, -1.16]) cyl(h, W, 0.4, 0.04, x, R.winch.y, R.winch.z, { axis: "x", seg: 24 });
  cyl(h, D, 0.11, 0.3, -1.36, R.winch.y, R.winch.z, { axis: "x", seg: 14 });
  bar(h, S, v3(-0.56, R.winch.y + 0.34, R.winch.z + 0.34), v3(-1.16, R.winch.y + 0.34, R.winch.z + 0.34), 0.03, 8);
  // triplex mud pump and hydraulic oil tank (right)
  box(h, O, 0.7, 0.45, 0.6, 0.78, dy + 0.24, -1.6, { r: 0.05 });
  for (const x of [0.62, 0.78, 0.94]) cyl(h, S, 0.06, 0.42, x, dy + 0.27, -1.13, { axis: "z", seg: 12 });
  box(h, S, 0.56, 0.16, 0.14, 0.78, dy + 0.27, -0.88, { r: 0.02 });
  sphere(h, D, 0.13, 0.78, dy + 0.6, -1.0, { seg: 14 });
  cyl(h, K, 0.05, 0.03, 0.62, dy + 0.48, -0.96, { axis: "z", seg: 12 });
  box(h, W, 0.55, 0.75, 0.9, 0.86, dy + 0.36, -2.75, { r: 0.04 });
  cyl(h, S, 0.06, 0.08, 0.86, dy + 0.77, -2.6, { seg: 12 });
  box(h, mats.glass(), 0.02, 0.32, 0.06, 1.14, dy + 0.4, -2.75, { r: 0.004 });
  // driller's station: platform, handrail, steps, console with gauges and levers, sunshade
  const px = -1.85;
  const py = 0.72;
  deck(h, GR, 1.0, 1.5, px, py, -0.45, 0.05);
  for (const z of [-1.1, 0.2]) beam(h, D, v3(-0.6, 0.78, z), v3(px - 0.45, py - 0.05, z), 0.08, 0.08);
  rail(h, W, [v3(-1.4, py, -1.18), v3(-2.33, py, -1.18), v3(-2.33, py, 0.28), v3(-1.4, py, 0.28)], { h: 1.0, r: 0.022 });
  for (const k of [1, 2]) box(h, GR, 0.4, 0.04, 0.7, px - 0.15 + k * 0.0, py - k * 0.24, 0.62 + k * 0.22, { r: 0.01 });
  box(h, D, 0.22, 0.7, 0.9, -1.45, py + 0.38, -0.45, { r: 0.03 });
  box(h, D, 0.3, 0.55, 1.12, -1.48, py + 0.98, -0.45, { r: 0.04, rz: -0.28 });
  box(h, K, 0.05, 0.38, 0.98, -1.58, py + 1.18, -0.45, { rz: -0.28, r: 0.01 });
  const face = mats.white();
  for (let i = 0; i < 4; i++)
    for (let j = 0; j < 2; j++) {
      const z = -0.84 + i * 0.26;
      const y = py + 1.08 + j * 0.17;
      cyl(h, face, 0.048, 0.03, -1.62 + j * 0.05, y, z, { axis: "x", seg: 14, rz: -0.28 });
    }
  const knob = mats.red();
  for (let i = 0; i < 6; i++) {
    const z = -0.88 + i * 0.17;
    bar(h, S, v3(-1.62, py + 0.78, z), v3(-1.78, py + 0.98, z + 0.02), 0.012, 6);
    sphere(h, i % 3 === 0 ? knob : K, 0.03, -1.78, py + 0.99, z + 0.02, { seg: 10 });
  }
  for (const z of [-1.12, 0.22]) bar(h, W, v3(-2.3, py, z), v3(-2.3, py + 2.15, z), 0.035, 8);
  bar(h, W, v3(-2.3, py + 2.15, -1.12), v3(-2.3, py + 2.15, 0.22), 0.035, 8);
  box(h, W, 1.15, 0.05, 1.6, -1.8, py + 2.2, -0.45, { r: 0.02, rz: 0.08 });
  bar(h, W, v3(-1.32, py + 2.2, -1.12), v3(-1.32, dy, -1.12), 0.03, 8);
  // jack barrels on their outriggers
  for (const s of [-1, 1])
    for (const z of R.jack.z) {
      cyl(h, W, 0.11, 0.78, s * R.jack.x, 0.92, z, { seg: 16 });
      box(h, W, 0.5, 0.22, 0.32, s * (R.jack.x - 0.25), 0.96, z, { r: 0.03 });
    }
  // hoses from the pump and the console to the mast foot, a tool box and the extinguisher
  tube(h, H, [v3(0.78, dy + 0.3, -0.84), v3(0.6, dy + 0.15, -0.2), v3(0.3, dy + 0.4, 0.3), v3(0.32, R.pivotY - 0.4, 0.18)], 0.035, { radial: 8 });
  for (let i = 0; i < 3; i++)
    tube(h, H, [v3(-1.45, py + 0.3, -0.2 + i * 0.08), v3(-0.9, dy + 0.06, -0.1 + i * 0.06), v3(-0.36, dy + 0.5, 0.1), v3(-0.34, R.pivotY - 0.5, 0.1 + i * 0.04)], 0.022, { radial: 6 });
  box(h, O, 0.6, 0.36, 0.4, 0.98, dy + 0.2, -0.25, { r: 0.04 });
  bottle(h, mats.red(), S, 1.05, dy, -3.55);
  const body = bakeFree(h, { mudTop: 1.4, mudBottom: 0.2 });
  group.add(body);

  // tracks
  const tracks: Track[] = [];
  for (const s of [-1, 1]) {
    const t = makeTrack({ length: R.track.length, sprocketR: 0.3, idlerR: 0.28, shoeW: 0.45, pitch: 0.16, rollers: 5, carriers: 1, frameMat: D });
    t.group.position.set(s * R.track.gauge, 0, R.track.z);
    group.add(t.group);
    tracks.push(t);
  }

  /* ---------- Jacks: rods and pads, all four together ---------- */
  const jk = new THREE.Group();
  for (const s of [-1, 1])
    for (const z of R.jack.z) {
      cyl(jk, C, 0.07, 0.62, s * R.jack.x, 0.37, z, { seg: 14 });
      box(jk, D, 0.46, 0.06, 0.46, s * R.jack.x, 0.01, z, { r: 0.015 });
    }
  const jacks = bakeFree(jk, { mudTop: 0.5, mudBottom: 0.1 });
  group.add(jacks);

  /* ---------- Mast: cradle (pivots), body (slides), head (feeds), rods (turn) ---------- */
  const mastG = new THREE.Group();
  mastG.position.set(0, R.pivotY, 0);
  group.add(mastG);
  const cr = new THREE.Group();
  for (const s of [-1, 1]) {
    box(cr, W, 0.07, 3.0, 0.52, s * 0.37, 0.7, 0, { r: 0.02 });
    box(cr, W, 0.12, 0.26, 0.26, s * R.ram.mast.x, R.ram.mast.y, R.ram.mast.z, { r: 0.02 });
  }
  for (const y of [-0.75, 2.15]) box(cr, W, 0.82, 0.12, 0.12, 0, y, -0.25, { r: 0.02 });
  cyl(cr, W, 0.17, 0.66, 0, 0, 0, { axis: "x", seg: 18 });
  cyl(cr, W, 0.06, 1.7, 0.12, 0.1, -0.33, { seg: 12 });
  const cradle = bakeFree(cr, { mudTop: -5, mudBottom: -6, mud: 0 });
  mastG.add(cradle);

  const slideG = new THREE.Group();
  mastG.add(slideG);
  const mb = new THREE.Group();
  for (const s of [-1, 1]) {
    beam(mb, O, v3(s * 0.24, R.foot, 0), v3(s * 0.24, R.top, 0), 0.16, 0.34, v3(0, 0, 1), 0.02);
    beam(mb, S, v3(s * 0.2, R.foot + 0.6, 0.195), v3(s * 0.2, R.top - 0.6, 0.195), 0.06, 0.06, v3(0, 0, 1), 0.006);
  }
  const bays = 10;
  for (let i = 0; i <= bays; i++) {
    const y = R.foot + 0.3 + (i * (R.top - R.foot - 0.6)) / bays;
    beam(mb, O, v3(-0.25, y, -0.12), v3(0.25, y, -0.12), 0.07, 0.07, v3(0, 1, 0), 0.008);
    if (i < bays) {
      const y1 = R.foot + 0.3 + ((i + 1) * (R.top - R.foot - 0.6)) / bays;
      const f = i % 2 ? 1 : -1;
      bar(mb, O, v3(-0.22 * f, y, -0.14), v3(0.22 * f, y1, -0.14), 0.026, 6);
    }
  }
  // foot clamp at the collar, crown and sheave at the top, feed cylinder, floodlights, hose run
  box(mb, D, 0.62, 0.46, 0.7, 0, R.foot + 0.3, R.rodZ, { r: 0.04 });
  for (const s of [-1, 1]) cyl(mb, S, 0.08, 0.3, s * 0.24, R.foot + 0.3, R.rodZ, { axis: "x", seg: 14 });
  box(mb, O, 0.62, 0.36, 0.8, 0, R.top + 0.12, 0.12, { r: 0.04 });
  for (const s of [-1, 1]) box(mb, O, 0.05, 0.62, 0.66, s * 0.16, R.top + 0.4, R.rodZ - 0.3, { r: 0.015 });
  lathe(mb, S, [[0.05, -0.04], [0.29, -0.04], [0.26, 0], [0.29, 0.04], [0.05, 0.04]], 0, R.top + 0.42, R.rodZ - 0.3, { axis: "x", seg: 28 });
  cyl(mb, D, 0.09, 2.4, 0, R.top - 1.6, 0.05, { seg: 14 });
  for (const s of [-1, 1]) lamp(mb, D, lens.white, s * 0.36, R.top - 0.5, 0.24, { w: 0.3, h: 0.2, pitch: 1.15, yaw: s * 0.15 });
  for (const zz of [0.03, -0.05]) bar(mb, H, v3(0.35, R.foot + 1.0, zz), v3(0.35, R.top - 1.8, zz), 0.028, 6);
  for (let y = R.foot + 1.2; y < R.top - 1.8; y += 1.4) box(mb, S, 0.06, 0.05, 0.16, 0.33, y, -0.01, { r: 0.008 });
  const mastBody = bakeFree(mb, { mudTop: -5, mudBottom: -6, mud: 0, dust: 0.7 });
  slideG.add(mastBody);

  const hd = new THREE.Group();
  box(hd, O, 0.5, 0.46, 0.48, 0, 0.42, 0, { r: 0.06, seg: 2 });
  box(hd, O, 0.64, 0.92, 0.08, 0, 0.45, -0.25, { r: 0.02 });
  for (const s of [-1, 1]) {
    box(hd, S, 0.13, 0.82, 0.11, s * 0.2, 0.45, -0.3, { r: 0.015 });
    cyl(hd, D, 0.09, 0.3, s * 0.13, 0.8, 0.04, { seg: 14 });
    cyl(hd, S, 0.06, 0.06, s * 0.13, 0.98, 0.04, { seg: 12 });
    tube(hd, H, [v3(s * 0.13, 1.0, 0.04), v3(s * 0.3, 1.15, -0.05), v3(0.5, 0.85, -0.3), v3(0.42, 0.25, -0.42)], 0.024, { radial: 6 });
  }
  lathe(hd, S, [[0.0, -0.2], [0.1, -0.2], [0.13, -0.08], [0.13, 0.22], [0.0, 0.22]], 0, 0, 0, { seg: 20 });
  lathe(hd, S, [[0.0, 0.64], [0.06, 0.64], [0.06, 0.96], [0.04, 1.06], [0.0, 1.06]], 0, 0, 0, { seg: 14 });
  tube(hd, H, [v3(0, 1.02, 0), v3(0.06, 1.18, 0.06), v3(0.24, 1.12, 0.1), v3(0.4, 0.7, -0.12), v3(0.4, 0.2, -0.36)], 0.03, { radial: 8 });
  const headMesh = bakeFree(hd, { mudTop: -5, mudBottom: -6, mud: 0 });
  const head = new THREE.Group();
  head.add(headMesh);
  slideG.add(head);

  // the rod string: a unit cylinder down from the chuck, with two flats so it reads turning
  const rodGeo = new THREE.CylinderGeometry(R.rodR, R.rodR, 1, 12, 1, true).translate(0, -0.5, 0);
  const rod = new THREE.Mesh(rodGeo, S);
  const flats = new THREE.Mesh(new THREE.BoxGeometry(0.012, 1, 0.074).translate(0, -0.5, 0), D);
  rod.add(flats);
  rod.castShadow = true;
  slideG.add(rod);

  /* ---------- Dynamic parts: raise rams, wireline ---------- */
  const rams = [-1, 1].map(() => {
    const barrel = new THREE.Mesh(UNIT_Y, W);
    const rodM = new THREE.Mesh(UNIT_Y, C);
    barrel.castShadow = rodM.castShadow = true;
    group.add(barrel, rodM);
    return { barrel, rod: rodM };
  });
  const wire = new THREE.Mesh(UNIT_Y, K);
  const wireDown = new THREE.Mesh(UNIT_Y, K);
  group.add(wire, wireDown);

  // anchors
  const crown = new THREE.Object3D();
  crown.position.set(0, R.top + 0.7, 0.12);
  slideG.add(crown);
  const panel = new THREE.Object3D();
  panel.position.set(-1.85, 1.9, -0.45);
  group.add(panel);
  const foot = new THREE.Object3D();
  foot.position.set(0, R.foot + 0.3, R.rodZ);
  slideG.add(foot);
  const headAnchor = new THREE.Object3D();
  headAnchor.position.set(0, 1.2, 0);
  head.add(headAnchor);

  group.traverse((ob) => {
    const m = ob as THREE.Mesh;
    if (m.isMesh) {
      m.castShadow = true;
      m.receiveShadow = true;
    }
  });
  wire.castShadow = wireDown.castShadow = false;

  const state: CoreRigState = { mast: 0, dip: 60, slide: 0, head: 1, spin: 0, jacks: 0, rods: false, track: 0, lights: false };
  let angle = 0;
  const m1 = new THREE.Vector3();
  const m2 = new THREE.Vector3();
  const apply = () => {
    mastG.rotation.x = rad(state.mast - 90);
    const off = state.slide * slideFor(state.dip);
    slideG.position.y = off;
    const hy = R.head.lo + (R.head.hi - R.head.lo) * THREE.MathUtils.clamp(state.head, 0, 1);
    head.position.set(0, hy, R.rodZ);
    rod.visible = state.rods;
    if (state.rods) {
      // From the chuck down through the foot clamp and 2 m into the casing.
      const bottom = groundAlong(state.dip) - 2 - off;
      rod.position.set(0, hy - 0.2, R.rodZ);
      rod.scale.set(1, Math.max(0.1, hy - 0.2 - bottom), 1);
      rod.rotation.y = angle;
    }
    jacks.position.y = (1 - THREE.MathUtils.clamp(state.jacks, 0, 1)) * R.jack.travel;
    for (const t of tracks) t.setDistance(state.track);
    group.updateMatrixWorld(true);
    // Rams from the deck lugs to the cradle, barrel then rod.
    const inv = group.matrixWorld.clone().invert();
    rams.forEach((rm, i) => {
      const s = i ? 1 : -1;
      const base = _a.set(s * R.ram.base.x, R.ram.base.y, R.ram.base.z);
      const top = m1.set(s * R.ram.mast.x, R.ram.mast.y, R.ram.mast.z).applyMatrix4(mastG.matrixWorld).applyMatrix4(inv);
      const len = base.distanceTo(top);
      const bl = Math.min(1.95, len - 0.12);
      const mid = m2.copy(top).sub(base).multiplyScalar(bl / len).add(base);
      span(rm.barrel, base, mid, 0.08);
      span(rm.rod, mid.clone().addScaledVector(top.clone().sub(base).normalize(), -0.1), top, 0.045);
    });
    // Wireline from the drum over the crown sheave, then down the rod line to the head.
    const drum = _a.set(R.winch.x * 0.5, R.winch.y + 0.24, R.winch.z);
    const sheaveBack = m1.set(0, R.top + 0.42 + 0.06, R.rodZ - 0.59).applyMatrix4(slideG.matrixWorld).applyMatrix4(inv);
    span(wire, drum, sheaveBack, 0.008);
    const sheaveFront = m2.set(0, R.top + 0.42, R.rodZ - 0.01).applyMatrix4(slideG.matrixWorld).applyMatrix4(inv);
    const headTop = _b.set(0, hy + 1.06, R.rodZ).applyMatrix4(slideG.matrixWorld).applyMatrix4(inv).clone();
    span(wireDown, sheaveFront, headTop, 0.008);
    lens.set(state.lights);
  };
  apply();
  return {
    group,
    state,
    lenses: lens,
    anchors: { head: headAnchor, crown, panel, foot },
    set(s) {
      Object.assign(state, s);
      apply();
    },
    update(dt) {
      if (state.spin) {
        angle += state.spin * dt;
        rod.rotation.y = angle;
      }
    },
  };
}

/* ---------- The pad's kit ---------- */

export interface PadKit {
  group: THREE.Group;
  /** Rods still in the rack. */
  rods(n: number): void;
  /** Core trays filled so far. */
  trays(n: number): void;
  /** Where the geologist logs, the tray stacks, and the light tower (pad frame). */
  spots: { bench: THREE.Vector3; offsider: THREE.Vector3; driller: THREE.Vector3 };
  /** Sets the casing and the return line for a hole at this dip. */
  collar(dip: number): void;
  update(dt: number, night: boolean): void;
}

export const RACK_RODS = 112;
const MAX_TRAYS = 84;

export function makePadKit(): PadKit {
  const group = new THREE.Group();
  group.name = "pad-kit";
  const W = paint(COLOURS.cream, { coat: 0.55, rough: 0.4 });
  const D = mats.dark();
  const S = mats.steel();
  const G = mats.galv();
  const H = mats.hose();
  const r = rng(77);
  const st = new THREE.Group();

  // Rod rack: two steel trestles, rods along z in nested rows.
  const RX = 2.85;
  const RZ = -1.3;
  for (const z of [-0.95, 0.95]) {
    for (const s of [-1, 1]) bar(st, G, v3(RX + s * 0.95, 0, RZ + z), v3(RX + s * 0.8, 0.62, RZ + z), 0.04, 8);
    beam(st, G, v3(RX - 0.95, 0.64, RZ + z), v3(RX + 0.95, 0.64, RZ + z), 0.1, 0.08, v3(0, 1, 0), 0.01);
    for (const s of [-1, 1]) bar(st, G, v3(RX + s * 0.88, 0.62, RZ + z), v3(RX + s * 0.88, 1.35, RZ + z), 0.03, 8);
  }
  // Logging bench: two trestle tables under a shade cloth; the tray stacks on pallets beside it.
  const BX = 6.6;
  const BZ = 2.4;
  for (const dz of [-0.6, 0.6]) {
    box(st, W, 2.4, 0.05, 0.8, BX, 0.86, BZ + dz, { r: 0.01 });
    for (const sx of [-1.1, 1.1]) for (const sz of [-0.32, 0.32]) bar(st, G, v3(BX + sx, 0, BZ + dz + sz), v3(BX + sx, 0.84, BZ + dz + sz), 0.022, 6);
  }
  for (const sx of [-1.75, 1.75]) for (const sz of [-1.5, 1.5]) bar(st, G, v3(BX + sx, 0, BZ + sz), v3(BX + sx, 2.45 + (sz > 0 ? 0.15 : 0), BZ + sz), 0.03, 8);
  const shade = new THREE.MeshStandardMaterial({ color: "#2e4a3c", roughness: 0.92, side: THREE.DoubleSide });
  box(st, shade, 3.7, 0.02, 3.2, BX, 2.55, BZ, { r: 0, rx: -0.05 });
  for (let i = 0; i < 6; i++) {
    const t = makeCoreTray(i + 1);
    t.position.set(BX - 0.8 + (i % 3) * 0.8, 0.95, BZ + (i < 3 ? -0.55 : 0.55));
    t.rotation.y = Math.PI / 2;
    st.add(t);
  }
  const PX = 6.6;
  const PZ = 5.6;
  for (const dx of [-0.7, 0.7]) box(st, mats.concrete(), 1.15, 0.12, 1.15, PX + dx, 0.055, PZ, { r: 0.01 });
  // Mud tanks: two open steel tanks, muddy water inside; a pump between them.
  const water = new THREE.MeshPhysicalMaterial({ color: "#5d4c3b", roughness: 0.06, metalness: 0, clearcoat: 1, clearcoatRoughness: 0.05 });
  const TX = -3.7;
  for (const tz of [2.6, 5.4]) {
    const w = 1.3;
    const d = 2.5;
    const hh = 0.95;
    box(st, S, w, 0.06, d, TX, 0.02, tz, { r: 0.01 });
    for (const s of [-1, 1]) {
      box(st, S, 0.05, hh, d + 0.04, TX + (s * (w - 0.05)) / 2, hh / 2, tz, { r: 0.008 });
      box(st, S, w, hh, 0.05, TX, hh / 2, tz + (s * (d - 0.05)) / 2, { r: 0.008 });
    }
    box(st, water, w - 0.12, 0.02, d - 0.12, TX, hh - 0.22, tz, { r: 0 });
    beam(st, G, v3(TX - w / 2, hh + 0.02, tz), v3(TX + w / 2, hh + 0.02, tz), 0.05, 0.05);
  }
  box(st, paint("#1f5f8b", { coat: 0.4 }), 0.6, 0.45, 0.5, TX + 1.2, 0.25, 4.0, { r: 0.05 });
  tube(st, H, [v3(TX + 1.2, 0.4, 3.75), v3(TX + 1.9, 0.12, 2.0), v3(0.6, 0.1, -0.2), v3(0.78, 1.2, -1.0)], 0.045, { radial: 8 });
  // Water tank trailer: a poly tank on a single axle.
  const poly = paint("#e6e1d3", { rough: 0.62, coat: 0.08 });
  const WX = 3.5;
  const WZ = -7.0;
  box(st, D, 1.5, 0.14, 3.0, WX, 0.62, WZ, { r: 0.02 });
  cyl(st, poly, 0.78, 2.7, WX, 1.48, WZ, { axis: "z", seg: 28 });
  for (const s of [-1, 1]) sphere(st, poly, 0.78, WX, 1.48, WZ + s * 1.35, { seg: 20, sy: 0.35, rx: Math.PI / 2 });
  for (let k = -1; k <= 1; k++) box(st, G, 1.66, 0.05, 0.08, WX, 1.5 + 0.02, WZ + k * 0.9, { r: 0.01, rx: 0 });
  for (const s of [-1, 1]) {
    cyl(st, mats.rubber(), 0.36, 0.22, WX + s * 0.86, 0.36, WZ - 0.2, { axis: "x", seg: 20 });
    cyl(st, G, 0.2, 0.235, WX + s * 0.86, 0.36, WZ - 0.2, { axis: "x", seg: 14 });
  }
  bar(st, D, v3(WX, 0.6, WZ + 1.5), v3(WX, 0.45, WZ + 3.1), 0.05, 8);
  cyl(st, D, 0.05, 0.4, WX, 0.22, WZ + 3.0, { seg: 8 });
  tube(st, H, [v3(WX - 0.2, 0.8, WZ + 1.6), v3(WX - 1.0, 0.1, WZ + 4.0), v3(0.9, 0.1, -2.0), v3(0.78, 1.25, -1.9)], 0.04, { radial: 8 });
  // Fuel pod in a bund, with the bowser hose.
  const FX = -3.2;
  const FZ = -6.4;
  box(st, S, 1.8, 0.06, 1.8, FX, 0.02, FZ, { r: 0.01 });
  for (const s of [-1, 1]) {
    box(st, S, 0.05, 0.3, 1.84, FX + s * 0.875, 0.15, FZ, { r: 0.008 });
    box(st, S, 1.8, 0.3, 0.05, FX, 0.15, FZ + s * 0.875, { r: 0.008 });
  }
  box(st, paint("#b9241b", { coat: 0.5 }), 1.2, 1.2, 1.2, FX, 0.7, FZ, { r: 0.08, seg: 2 });
  cyl(st, D, 0.08, 0.1, FX + 0.3, 1.34, FZ - 0.3, { seg: 12 });
  box(st, D, 0.3, 0.25, 0.12, FX + 0.61, 0.95, FZ, { r: 0.02 });
  const kit = bakeFree(st, { mudTop: 0.5, mudBottom: 0.05, mud: 0.8 });
  group.add(kit);
  group.traverse((ob) => {
    const m = ob as THREE.Mesh;
    if (m.isMesh) {
      m.castShadow = true;
      m.receiveShadow = true;
    }
  });

  // Rods in the rack, instanced: rows of 14 nested, up to 8 rows.
  const rodGeo = new THREE.CylinderGeometry(CORE_RIG.rodR, CORE_RIG.rodR, 3.0, 10, 1).rotateX(Math.PI / 2);
  const rodsM = new THREE.InstancedMesh(rodGeo, S, RACK_RODS);
  {
    const m4 = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const one = new THREE.Vector3(1, 1, 1);
    const p = new THREE.Vector3();
    const pitch = CORE_RIG.rodR * 2 + 0.004;
    let n = 0;
    for (let row = 0; n < RACK_RODS; row++) {
      const per = 14 - (row % 2);
      for (let i = 0; i < per && n < RACK_RODS; i++, n++) {
        p.set(RX - ((per - 1) * pitch) / 2 + i * pitch, 0.68 + CORE_RIG.rodR + row * pitch * 0.87, RZ + (r() - 0.5) * 0.08);
        rodsM.setMatrixAt(n, m4.compose(p, q, one));
      }
    }
  }
  rodsM.castShadow = rodsM.receiveShadow = true;
  group.add(rodsM);

  // Core trays filled, instanced: six stacks fourteen high on two pallets.
  const tpl = makeCoreTray(1);
  const trayM = new THREE.InstancedMesh(tpl.geometry, tpl.material, MAX_TRAYS);
  {
    const m4 = new THREE.Matrix4();
    const q = new THREE.Quaternion().setFromAxisAngle(Y, Math.PI / 2);
    const one = new THREE.Vector3(1, 1, 1);
    const p = new THREE.Vector3();
    for (let n = 0; n < MAX_TRAYS; n++) {
      const stack = Math.floor(n / 14);
      const lvl = n % 14;
      const pallet = stack % 2;
      const col = Math.floor(stack / 2);
      p.set(PX + (pallet ? 0.7 : -0.7) + (col - 1) * 0.34, 0.115 + 0.058 + lvl * 0.068, PZ);
      trayM.setMatrixAt(n, m4.compose(p, q, one));
    }
  }
  trayM.castShadow = trayM.receiveShadow = true;
  trayM.count = 0;
  group.add(trayM);

  // Casing at the collar and the return line to the first mud tank.
  const casing = new THREE.Group();
  const cs = new THREE.Group();
  cyl(cs, S, 0.075, 0.95, 0, 0.0, 0, { seg: 16 });
  cyl(cs, S, 0.095, 0.08, 0, 0.44, 0, { seg: 16 });
  cyl(cs, S, 0.05, 0.36, 0.18, 0.28, 0, { axis: "x", seg: 12 });
  cyl(cs, mats.red(), 0.035, 0.12, 0.3, 0.38, 0, { seg: 10 });
  casing.add(bakeFree(cs));
  group.add(casing);
  const ret = new THREE.Mesh(new THREE.BufferGeometry(), H);
  ret.castShadow = true;
  group.add(ret);

  // The light tower behind the rig, aimed at the collar.
  const tower = makeLightTower();
  tower.group.position.set(-4.6, 0, -2.4);
  tower.group.rotation.y = 2.2;
  tower.set({ mast: 1, yaw: 0.4 });
  group.add(tower.group);

  return {
    group,
    spots: { bench: v3(BX - 0.1, 0, BZ - 1.25), offsider: v3(0.95, 0, 1.0), driller: v3(-1.95, 0.72, -0.45) },
    rods(n) {
      rodsM.count = THREE.MathUtils.clamp(Math.round(n), 0, RACK_RODS);
    },
    trays(n) {
      trayM.count = THREE.MathUtils.clamp(Math.round(n), 0, MAX_TRAYS);
    },
    collar(dip) {
      const c = collarAhead(dip);
      casing.position.set(0, 0, c);
      casing.rotation.x = rad(90 - dip);
      const top = new THREE.Vector3(0.3, 0.28, 0).applyEuler(casing.rotation).add(casing.position);
      ret.geometry.dispose();
      ret.geometry = new THREE.TubeGeometry(
        new THREE.CatmullRomCurve3([top, top.clone().add(v3(-0.4, -0.1, 0.3)), v3(-1.8, 0.08, c + 0.5), v3(TX + 0.4, 0.3, 2.0), v3(TX + 0.2, 1.0, 2.0)], false, "centripetal"),
        40,
        0.05,
        8,
      );
    },
    update(dt, night) {
      tower.set({ lights: night });
      tower.update(dt);
    },
  };
}
