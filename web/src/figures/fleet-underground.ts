/**
 * Underground fleet: a 17-t-class load-haul-dump loader (LHD) and a two-boom face drill rig (jumbo).
 * Generic yellow, no lettering. Metres, y up, +z forward, origin on the ground under the machine's centre.
 * The machines' left is +x.
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
  COLOURS,
  cyl,
  deck,
  instance,
  ladder,
  lamp,
  lenses,
  mats,
  mirror,
  node,
  paint,
  plate,
  prism,
  rail,
  rim,
  sphere,
  tube,
  v3,
  type Lenses,
  type TyreSpec,
  type V3,
} from "./fleet-kit";
import { addTyre, arcBand, bakeWheels, makeRam, type Ram } from "./fleet-underground-parts";

function swapLenses(root: THREE.Object3D, from: Lenses, to: Lenses) {
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh) return;
    if (m.material === from.white) m.material = to.white;
    else if (m.material === from.amber) m.material = to.amber;
    else if (m.material === from.red) m.material = to.red;
  });
}

/** Reflective tape strip on a face: a thin plate proud of the surface. */
function tape(parent: THREE.Object3D, w: number, h: number, d: number, x: number, y: number, z: number) {
  box(parent, mats.reflector(), w, h, d, x, y, z, { r: 0.004, wear: { mud: 0.4, dust: 0.3 } });
}

/** Red/white chevron panel facing -z (rear marker) or +z. */
function chevrons(parent: THREE.Object3D, x: number, y: number, z: number, w: number, h: number, facing: 1 | -1) {
  const n = 5;
  const red = mats.red();
  const white = mats.reflector();
  const g = node(parent, x, y, z, { ry: facing > 0 ? 0 : Math.PI });
  box(g, mats.dark(), w + 0.04, h + 0.04, 0.02, 0, 0, -0.01, { r: 0.005 });
  // diagonal stripes clipped to the panel: each stripe is a parallelogram whose corners are clamped in x
  const sw = w / (n * 2);
  const lean = h * 0.9;
  const cx = (x: number) => Math.max(-w / 2, Math.min(w / 2, x));
  for (let i = -1; i <= n * 2; i++) {
    const x0 = -w / 2 + i * sw;
    const pts = [v3(cx(x0), -h / 2, 0.004), v3(cx(x0 + sw), -h / 2, 0.004), v3(cx(x0 + sw + lean), h / 2, 0.004), v3(cx(x0 + lean), h / 2, 0.004)];
    if (Math.abs(pts[0]!.x - pts[1]!.x) + Math.abs(pts[2]!.x - pts[3]!.x) < 1e-3) continue;
    plate(g, (i + 2) % 2 ? red : white, pts, 0.008, { out: v3(0, 0, 1) });
  }
}

/** Builds a side-profile bucket (shell plates, side plates, lip, wear bars, hinge brackets) into `b`. */
function bucket(
  b: THREE.Object3D,
  P: THREE.Material,
  S: THREE.Material,
  prof: [number, number][],
  top: [number, number],
  halfW: number,
  hingeX: number,
) {
  const cz = prof.reduce((a, p) => a + p[0], 0) / prof.length;
  const cy = prof.reduce((a, p) => a + p[1], 0) / prof.length;
  for (let i = 0; i < prof.length - 1; i++) {
    const [z0, y0] = prof[i]!;
    const [z1, y1] = prof[i + 1]!;
    const mz = (z0 + z1) / 2 - cz;
    const my = (y0 + y1) / 2 - cy;
    plate(b, P, [v3(-halfW, y0, z0), v3(halfW, y0, z0), v3(halfW, y1, z1), v3(-halfW, y1, z1)], 0.05, { out: v3(0, my, mz) });
  }
  const outline: [number, number][] = [...prof, top];
  for (const s of [-1, 1]) prism(b, P, outline, 0.07, "zy", s * (halfW + 0.03), { bevel: 0.015 });
  // cutting edge and corner shrouds
  const [lz, ly] = prof[0]!;
  box(b, S, halfW * 2 + 0.06, 0.08, 0.32, 0, ly + 0.04, lz - 0.12, { r: 0.015 });
  for (const s of [-1, 1]) box(b, S, 0.1, 0.22, 0.4, s * (halfW + 0.02), ly + 0.12, lz - 0.18, { r: 0.02 });
  // wear bars under the floor
  for (const x of [-0.6, 0, 0.6].map((k) => k * halfW * 1.2)) box(b, S, 0.1, 0.06, (prof[0]![0] - prof[1]![0]) * 0.9, x, ly - 0.03, (prof[0]![0] + prof[1]![0]) / 2, { r: 0.01 });
  // back ribs and hinge brackets
  for (const s of [-1, 1]) {
    plate(b, P, [v3(s * hingeX, -0.5, 0.42), v3(s * hingeX, 0.4, 0.3), v3(s * hingeX, 0.2, -0.06), v3(s * hingeX, -0.12, -0.14), v3(s * hingeX, -0.32, 0.05)], 0.1, { out: v3(s, 0, 0) });
    plate(b, P, [v3(s * (hingeX + 0.14), -0.5, 0.42), v3(s * (hingeX + 0.14), 0.4, 0.3), v3(s * (hingeX + 0.14), 0.2, -0.06), v3(s * (hingeX + 0.14), -0.12, -0.14), v3(s * (hingeX + 0.14), -0.32, 0.05)], 0.06, { out: v3(s, 0, 0) });
    cyl(b, S, 0.09, 0.32, s * (hingeX + 0.06), 0, 0, { axis: "x", seg: 12 });
  }
  // tilt-link bracket on the back
  for (const s of [-1, 1]) plate(b, P, [v3(s * 0.12, 0.92, 0.48), v3(s * 0.12, 0.98, 0.3), v3(s * 0.12, 0.62, 0.22), v3(s * 0.12, 0.55, 0.42)], 0.06, { out: v3(s, 0, 0) });
  cyl(b, S, 0.07, 0.34, 0, 0.92, 0.3, { axis: "x", seg: 10 });
}

// ───────────────────────────── LHD ─────────────────────────────

export interface LHDOptions {
  colour?: string;
}
export interface LHDState {
  /** Lift arms 0 (bucket on the floor) .. 1 (full height). */
  lift: number;
  /** Bucket 0 level, 1 fully dumped (≈55° forward); negative to −0.4 racks it back for carrying. */
  tilt: number;
  /** Articulation angle, rad; positive turns left (+x). */
  steer: number;
  /** Ground speed m/s (integrated in update). */
  speed: number;
  /** Distance travelled, m. */
  distance: number;
  lights: boolean;
}
export interface LHD {
  group: THREE.Group;
  state: LHDState;
  set(s: Partial<LHDState>): void;
  update(dt: number): void;
  lenses: Lenses;
  /** Head- and work-lamp anchors (local +z is the beam direction) for scene SpotLights. */
  anchors: { front: THREE.Object3D[]; rear: THREE.Object3D[] };
}

const LHD_DIM = {
  tyre: { od: 1.86, width: 0.72, rim: 0.74, pattern: "rock", lugs: 26, depth: 0.045, segments: 56 } as TyreSpec,
  axleY: 0.9,
  trackX: 1.13,
  frontAxle: 1.95,
  rearAxle: -2.35,
  arm: { y: 1.62, z: 0.85 },
  hingeAtLift0: { y: 0.64, z: 3.55 },
  liftRange: (64 * Math.PI) / 180,
  dump: (55 * Math.PI) / 180,
  /** Shift so the origin sits under the machine's middle (bucket lip ≈ +5.9, rear ≈ −5.55 before shift). */
  shiftZ: -0.2,
};

interface LHDTemplate {
  front: THREE.Group;
  rear: THREE.Group;
  arms: THREE.Group;
  bucket: THREE.Group;
  frontWheels: THREE.Group;
  rearWheels: THREE.Group;
  lens: Lenses;
}
const lhdTemplates = new Map<string, LHDTemplate>();

function buildLHD(colour: string): LHDTemplate {
  const P = paint(colour);
  const D = mats.dark();
  const K = mats.black();
  const S = mats.steel();
  const C = mats.chrome();
  const G = mats.glass();
  const RED = mats.red();
  const lens = lenses();
  const L = LHD_DIM;
  const ay = L.axleY;

  // ───── front frame (hinge at origin) ─────
  const f = new THREE.Group();
  box(f, P, 0.78, 0.95, 2.7, 0, 0.92, 1.72, { r: 0.06 }); // narrow box frame between the wheels
  box(f, P, 0.9, 0.35, 0.9, 0, 1.28, 2.85, { r: 0.05, rx: -0.25 });
  // hinge yoke
  for (const y of [0.6, 1.3]) box(f, P, 0.7, 0.14, 0.6, 0, y, 0.25, { r: 0.03 });
  cyl(f, S, 0.12, 0.95, 0, 0.95, 0.08, { seg: 14 });
  // arm towers
  for (const s of [-1, 1]) {
    plate(f, P, [v3(s * 0.4, 0.5, 0.5), v3(s * 0.4, 1.78, 0.62), v3(s * 0.4, 1.78, 1.12), v3(s * 0.4, 0.5, 1.55)], 0.06, { out: v3(s, 0, 0) });
    cyl(f, S, 0.11, 0.5, s * 0.55, L.arm.y, L.arm.z, { axis: "x", seg: 14 });
    // lift-ram base lugs
    box(f, P, 0.3, 0.26, 0.3, s * 0.5, 0.52, 1.15, { r: 0.03 });
  }
  box(f, P, 0.36, 0.3, 0.36, 0, 1.78, 1.05, { r: 0.04 }); // tilt-ram base
  // front axle housing with diff and planetary ends
  cyl(f, D, 0.3, 1.5, 0, ay, L.frontAxle, { axis: "x", seg: 20 });
  sphere(f, D, 0.36, 0, ay, L.frontAxle - 0.12, { seg: 16 });
  // fenders over the front tyres, with lamps and reflectors
  for (const s of [-1, 1]) {
    const x0 = s * (L.trackX - 0.42);
    const x1 = s * (L.trackX + 0.42);
    arcBand(f, P, ay, L.frontAxle, 1.02, Math.min(x0, x1), Math.max(x0, x1), 0.35, Math.PI - 0.35, 0.05, 10);
    box(f, P, 0.18, 0.5, 0.18, s * 0.6, 1.55, L.frontAxle, { r: 0.03 });
    box(f, D, 0.12, 0.26, 0.12, s * L.trackX, ay + 0.98, L.frontAxle + 0.47, { r: 0.02 });
    lamp(f, D, lens.white, s * L.trackX, ay + 1.12, L.frontAxle + 0.55, { w: 0.3, h: 0.18, pitch: 0.12 });
    tape(f, 0.02, 0.06, 0.5, s * (L.trackX + 0.43), ay + 0.75, L.frontAxle + 0.2);
  }
  // hoses along the frame
  for (let i = 0; i < 3; i++) tube(f, mats.hose(), [v3(-0.2 + i * 0.12, 1.4, 0.1), v3(-0.2 + i * 0.12, 1.55, 0.6), v3(-0.25 + i * 0.12, 1.5, 1.4), v3(-0.3 + i * 0.12, 1.1, 2.4)], 0.03, { radial: 6 });
  tape(f, 0.8, 0.06, 0.01, 0, 1.2, 3.08);
  const front = bakeFree(f, { mudTop: 1.4, mudBottom: 0.2, mud: 1.1, dust: 0.9 });

  // ───── lift arms (one group, rotating about the arm pivot) ─────
  const la = new THREE.Group();
  const hz = L.hingeAtLift0.z - L.arm.z;
  const hy = L.hingeAtLift0.y - L.arm.y;
  const armLen = Math.hypot(hz, hy);
  for (const s of [-1, 1]) {
    // arm built along +z from the pivot, deeper in the middle
    prism(la, P, [[-0.18, -0.16], [armLen * 0.45, -0.32], [armLen + 0.1, -0.12], [armLen + 0.1, 0.12], [armLen * 0.5, 0.2], [-0.18, 0.16]], 0.18, "zy", s * 0.58, { bevel: 0.02 });
    cyl(la, S, 0.1, 0.3, s * 0.58, 0, 0, { axis: "x", seg: 12 });
  }
  beam(la, P, v3(-0.6, -0.05, armLen * 0.62), v3(0.6, -0.05, armLen * 0.62), 0.24, 0.28, undefined, 0.03); // cross tube
  const arms = bakeFree(la, { mudTop: 0.6, mudBottom: -1.2, mud: 0.9 });

  // ───── bucket (hinge at origin, level) ─────
  const bk = new THREE.Group();
  const prof: [number, number][] = [
    [2.35, -0.62],
    [0.7, -0.62],
    [0.4, -0.5],
    [0.22, -0.2],
    [0.18, 0.28],
    [0.28, 0.72],
    [0.48, 0.92],
    [0.82, 0.97],
  ];
  bucket(bk, P, S, prof, [2.35, -0.38], 1.44, 0.58);
  // spill-guard top lip
  beam(bk, P, v3(-1.45, 0.98, 0.82), v3(1.45, 0.98, 0.82), 0.12, 0.1, undefined, 0.02);
  const bucketG = bakeFree(bk, { mudTop: 0.1, mudBottom: -0.65, mud: 1.2, dust: 1.1 });

  // ───── rear frame (hinge at origin, extends to -z) ─────
  const r = new THREE.Group();
  const ra = L.rearAxle;
  box(r, P, 1.3, 0.85, 1.4, 0, 0.88, -0.75, { r: 0.05 }); // hinge box
  for (const y of [0.55, 1.35]) box(r, P, 0.5, 0.12, 0.5, 0, y, -0.12, { r: 0.03 });
  box(r, P, 1.3, 1.35, 1.95, 0, 1.2, ra, { r: 0.06 }); // core between the rear tyres
  cyl(r, D, 0.3, 1.5, 0, ay, ra, { axis: "x", seg: 20 });
  // engine compartment behind the wheels, full width, with a sloped top rear
  prism(r, P, [[-3.3, 0.48], [-5.38, 0.48], [-5.38, 1.65], [-5.0, 2.12], [-3.3, 2.12]], 2.84, "zy", 0, { bevel: 0.06 });
  // hood shelf over the rear tyres and fenders
  box(r, P, 2.84, 0.22, 1.95, 0, 2.02, ra - 0.02, { r: 0.05 });
  for (const s of [-1, 1]) {
    arcBand(r, P, ay, ra, 1.0, Math.min(s * 0.72, s * 1.52), Math.max(s * 0.72, s * 1.52), 0.55, Math.PI - 0.5, 0.04, 8);
    // engine-bay side doors and louvres
    for (let i = 0; i < 6; i++) box(r, D, 0.02, 0.04, 1.2, s * 1.43, 1.0 + i * 0.13, -4.3, { r: 0.006 });
    box(r, D, 0.015, 1.3, 0.02, s * 1.43, 1.3, -3.55, { r: 0.004 });
    box(r, D, 0.015, 1.3, 0.02, s * 1.43, 1.3, -5.0, { r: 0.004 });
    tape(r, 0.01, 0.06, 1.6, s * 1.43, 0.62, -4.3);
    tape(r, 0.01, 0.06, 1.7, s * 1.43, 2.0, -2.4);
  }
  // hood panel seams, hinges, grab handles and lifting eyes
  for (const z of [-3.95, -4.65]) box(r, D, 2.7, 0.012, 0.025, 0, 2.125, z, { r: 0.004 });
  for (const s of [-1, 1]) {
    for (const z of [-3.6, -4.3, -4.95]) box(r, S, 0.08, 0.03, 0.12, s * 1.36, 2.13, z, { r: 0.008 });
    bar(r, S, v3(s * 1.43, 1.85, -3.75), v3(s * 1.47, 1.85, -3.75), 0.018, 6);
    bar(r, S, v3(s * 1.47, 1.85, -3.75), v3(s * 1.47, 1.85, -4.05), 0.018, 6);
    bar(r, S, v3(s * 1.47, 1.85, -4.05), v3(s * 1.43, 1.85, -4.05), 0.018, 6);
    box(r, D, 0.06, 0.14, 0.14, s * 1.3, 2.18, -5.25, { r: 0.02 });
    box(r, K, 0.5, 0.45, 0.02, s * 1.12, 0.32, ra - 1.02, { r: 0.004 }); // mud flaps behind the rear tyres
  }
  // radiator grille and guard at the back
  box(r, K, 2.3, 1.0, 0.06, 0, 1.08, -5.4, { r: 0.01 });
  for (let i = 0; i < 8; i++) box(r, D, 2.36, 0.04, 0.06, 0, 0.66 + i * 0.12, -5.43, { r: 0.008 });
  for (const x of [-0.8, 0, 0.8]) box(r, D, 0.05, 1.05, 0.07, x, 1.08, -5.44, { r: 0.008 });
  // counterweight bumper with chevrons, tail and work lamps
  box(r, P, 2.8, 0.42, 0.42, 0, 0.36, -5.5, { r: 0.06 });
  chevrons(r, 0, 0.36, -5.72, 1.8, 0.26, -1);
  for (const s of [-1, 1]) {
    lamp(r, D, lens.red, s * 1.2, 0.62, -5.6, { w: 0.18, h: 0.14, yaw: Math.PI });
    lamp(r, D, lens.white, s * 1.1, 2.2, -5.05, { w: 0.32, h: 0.2, yaw: Math.PI, pitch: 0.15 });
    box(r, D, 0.12, 0.25, 0.12, s * 1.1, 2.08, -5.0, { r: 0.02 });
  }
  // exhaust purifier and air cleaner on the hood
  cyl(r, K, 0.22, 1.1, 0.75, 2.42, -4.2, { axis: "z", seg: 16 });
  for (const z of [-3.62, -4.78]) box(r, D, 0.5, 0.2, 0.08, 0.75, 2.22, z, { r: 0.02 });
  bar(r, K, v3(0.75, 2.42, -3.6), v3(0.75, 2.6, -3.3), 0.07, 10);
  cyl(r, P, 0.26, 0.7, -0.75, 2.45, -4.4, { seg: 18 });
  cyl(r, K, 0.28, 0.1, -0.75, 2.84, -4.4, { seg: 18 });
  bar(r, K, v3(-0.75, 2.2, -3.95), v3(-0.75, 2.2, -3.4), 0.09, 10);
  // right side: hydraulic tank, fuel tank with fill point
  box(r, P, 0.95, 1.15, 1.15, -1.0, 1.25, -0.9, { r: 0.07 });
  box(r, K, 0.1, 0.3, 0.06, -1.49, 1.4, -0.7, { r: 0.01 });
  cyl(r, K, 0.07, 0.1, -1.0, 1.86, -1.2, { seg: 10 });
  // operator compartment on the left, sideways seated, beside the articulation
  const cab = node(r, 0.98, 0, -0.8);
  box(cab, P, 1.06, 0.3, 1.36, 0, 0.62, 0, { r: 0.04 }); // floor box
  // posts and glass
  for (const [x, z] of [[-0.48, 0.62], [0.48, 0.62], [-0.48, -0.62], [0.48, -0.62]] as const) box(cab, P, 0.09, 1.55, 0.09, x, 1.55, z, { r: 0.02 });
  box(cab, G, 0.04, 1.15, 1.12, 0.5, 1.42, 0, { r: 0.01 }); // door glass (outboard)
  box(cab, G, 0.92, 1.1, 0.04, 0, 1.45, 0.64, { r: 0.01 });
  box(cab, G, 0.92, 1.1, 0.04, 0, 1.45, -0.64, { r: 0.01 });
  box(cab, P, 0.04, 0.45, 1.2, 0.5, 0.95, 0, { r: 0.01 }); // door lower panel
  box(cab, S, 0.04, 0.04, 0.16, 0.53, 1.15, 0.35, { r: 0.008 });
  // FOPS roof: thick plate with an overhang and a visor
  box(cab, P, 1.22, 0.14, 1.52, 0, 2.38, 0, { r: 0.04 });
  box(cab, D, 1.12, 0.03, 1.4, 0, 2.3, 0, { r: 0.01 });
  beacon(cab, K, lens.amber, 0.3, 2.45, -0.45, 1.0);
  lamp(cab, D, lens.white, 0.3, 2.42, 0.72, { w: 0.26, h: 0.16, pitch: 0.1 });
  lamp(cab, D, lens.white, -0.3, 2.42, 0.72, { w: 0.26, h: 0.16, pitch: 0.1 });
  lamp(cab, D, lens.white, 0.0, 2.42, -0.74, { w: 0.26, h: 0.16, yaw: Math.PI, pitch: 0.1 });
  // access step and grab handle
  box(cab, D, 0.36, 0.04, 0.4, 0.66, 0.34, 0.1, { r: 0.01 });
  box(cab, D, 0.36, 0.04, 0.3, 0.62, 0.56, 0.1, { r: 0.01 });
  for (const z of [-0.08, 0.28]) for (const x of [0.5, 0.82]) box(cab, D, 0.04, 0.46, 0.04, x, 0.55, z, { r: 0.008 });
  bar(cab, P, v3(0.58, 0.9, 0.55), v3(0.58, 1.9, 0.55), 0.022, 6);
  mirror(r, K, C, v3(1.5, 1.2, -0.2), v3(1.62, 1.6, 0.25), 0.32, 0.3);
  tape(r, 0.01, 0.06, 1.2, 1.51, 0.75, -0.85);
  // fire suppression bottles behind the cab
  bottle(r, RED, S, 1.15, 2.13, -1.85, 0.55, 0.09);
  bottle(r, RED, S, 1.15, 2.13, -2.1, 0.55, 0.09);
  const rear = bakeFree(r, { mudTop: 1.4, mudBottom: 0.2, mud: 1.1, dust: 0.9 });

  // ───── wheels: front and rear axles ─────
  const wheelSet = () => {
    const w = new THREE.Group();
    for (const s of [-1, 1]) {
      const n = node(w, s * L.trackX, 0, 0, { ry: s > 0 ? 0 : Math.PI });
      addTyre(n, L.tyre);
      rim(n, mats.dark(), mats.dark(), S, { rim: L.tyre.rim, width: L.tyre.width, dish: 0.12, bolts: 20, hubR: 0.22 });
    }
    return bakeWheels(w);
  };
  return { front, rear, arms, bucket: bucketG, frontWheels: wheelSet(), rearWheels: wheelSet(), lens };
}

/** Builds a 17-t-class underground LHD loader. */
export function makeLHD(opts: LHDOptions = {}): LHD {
  const colour = opts.colour ?? COLOURS.yellow;
  let tpl = lhdTemplates.get(colour);
  if (!tpl) {
    tpl = buildLHD(colour);
    lhdTemplates.set(colour, tpl);
  }
  const L = LHD_DIM;
  const lens = lenses();
  const group = new THREE.Group();
  group.name = "lhd";
  const body = new THREE.Group();
  body.position.z = L.shiftZ;
  group.add(body);
  const front = new THREE.Group();
  const rear = new THREE.Group();
  body.add(front, rear);
  const fb = instance(tpl.front);
  const rb = instance(tpl.rear);
  swapLenses(fb, tpl.lens, lens);
  swapLenses(rb, tpl.lens, lens);
  front.add(fb);
  rear.add(rb);
  const fw = new THREE.Group();
  fw.position.set(0, L.axleY, L.frontAxle);
  fw.add(instance(tpl.frontWheels));
  front.add(fw);
  const rw = new THREE.Group();
  rw.position.set(0, L.axleY, L.rearAxle);
  rw.add(instance(tpl.rearWheels));
  rear.add(rw);
  const arm = new THREE.Group();
  arm.position.set(0, L.arm.y, L.arm.z);
  arm.add(instance(tpl.arms));
  front.add(arm);
  const bkt = new THREE.Group();
  bkt.add(instance(tpl.bucket));
  front.add(bkt);
  // rams: lift (×2) and tilt in the front frame; steering (×2) across the hinge
  const hz = L.hingeAtLift0.z - L.arm.z;
  const hy = L.hingeAtLift0.y - L.arm.y;
  const armLen = Math.hypot(hz, hy);
  const a0 = Math.atan2(hy, hz);
  const armPt = (alpha: number, d: number, off: number, out: V3) => {
    const c = Math.cos(alpha);
    const s = Math.sin(alpha);
    return out.set(out.x, L.arm.y + off * c + d * s, L.arm.z - off * s + d * c);
  };
  const liftBase = (s: number) => v3(s * 0.5, 0.5, 1.15);
  const liftArm = (alpha: number, s: number) => armPt(alpha, armLen * 0.48, -0.32, v3(s * 0.5));
  const tiltBase = v3(0, 1.85, 1.05);
  const tiltEnd = (alpha: number, tilt: number) => {
    const B = armPt(alpha, armLen, 0, v3());
    const c = Math.cos(tilt);
    const s = Math.sin(tilt);
    const ey = 0.92;
    const ez = 0.3;
    return v3(0, B.y + ey * c - ez * s, B.z + ey * s + ez * c);
  };
  // size rams over the full motion range
  const range = (f: (l: number, t: number) => number) => {
    let lo = Infinity;
    let hi = 0;
    for (let i = 0; i <= 10; i++)
      for (let j = 0; j <= 10; j++) {
        const v = f(i / 10, -0.4 + (1.4 * j) / 10);
        lo = Math.min(lo, v);
        hi = Math.max(hi, v);
      }
    return { lo, hi };
  };
  const lr = range((l) => liftBase(1).distanceTo(liftArm(a0 + l * L.liftRange, 1)));
  const tr = range((l, t) => tiltBase.distanceTo(tiltEnd(a0 + l * L.liftRange, t * L.dump)));
  const lifts: Ram[] = [-1, 1].map(() => makeRam(front, { r: 0.12, min: lr.lo, max: lr.hi }));
  const tiltRam = makeRam(front, { r: 0.13, min: tr.lo, max: tr.hi });
  const steerRams: Ram[] = [-1, 1].map(() => makeRam(body, { r: 0.08, min: 0.95, max: 1.45, barrelMat: mats.dark() }));
  const anchors = { front: [] as THREE.Object3D[], rear: [] as THREE.Object3D[] };
  for (const s of [-1, 1]) {
    const a = new THREE.Object3D();
    a.position.set(s * L.trackX, L.axleY + 1.12, L.frontAxle + 0.6);
    front.add(a);
    anchors.front.push(a);
    const b = new THREE.Object3D();
    b.position.set(s * 1.1, 2.2, -5.1);
    b.rotation.y = Math.PI;
    rear.add(b);
    anchors.rear.push(b);
  }

  const state: LHDState = { lift: 0, tilt: 0, steer: 0, speed: 0, distance: 0, lights: true };
  const pa = new THREE.Vector3();
  const pb = new THREE.Vector3();
  const apply = () => {
    const alpha = a0 + clamp01(state.lift) * L.liftRange;
    arm.rotation.x = -alpha;
    const B = armPt(alpha, armLen, 0, pa.set(0, 0, 0));
    bkt.position.set(0, B.y, B.z);
    const tilt = Math.max(-0.4, Math.min(1, state.tilt)) * L.dump;
    bkt.rotation.x = tilt;
    [-1, 1].forEach((s, i) => lifts[i]!.set(liftBase(s), liftArm(alpha, s)));
    tiltRam.set(tiltBase, tiltEnd(alpha, tilt));
    const st = Math.max(-0.75, Math.min(0.75, state.steer));
    front.rotation.y = st / 2;
    rear.rotation.y = -st / 2;
    [-1, 1].forEach((s, i) => {
      pa.set(s * 0.55, 0.95, -0.75).applyAxisAngle(Y, -st / 2);
      pb.set(s * 0.45, 0.95, 0.55).applyAxisAngle(Y, st / 2);
      steerRams[i]!.set(pa, pb);
    });
    const roll = state.distance / (L.tyre.od / 2);
    fw.rotation.x = roll;
    rw.rotation.x = roll;
    lens.set(state.lights);
  };
  const Y = new THREE.Vector3(0, 1, 0);
  const lhd: LHD = {
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
  return lhd;
}

// ───────────────────────────── drill jumbo ─────────────────────────────

export interface BoomState {
  /** Swing of the boom about its turntable, rad (positive toward +x). */
  yaw: number;
  /** Elevation of the boom, rad (positive raises the feed). */
  pitch: number;
  /** Rock-drill position along the feed 0 (back) .. 1 (fully fed, steel in the rock). */
  feed: number;
}
export interface JumboState {
  booms: [BoomState, BoomState];
  /** Rock drills rotating (update(dt) spins the steels). */
  drilling: boolean;
  /** Stabiliser jacks 0 (up) .. 1 (on the floor). */
  stabilisers: number;
  /** Articulation, rad. */
  steer: number;
  speed: number;
  distance: number;
  lights: boolean;
}
export interface Jumbo {
  group: THREE.Group;
  state: JumboState;
  set(s: Partial<Omit<JumboState, "booms">> & { booms?: [Partial<BoomState>, Partial<BoomState>] }): void;
  update(dt: number): void;
  lenses: Lenses;
  anchors: { front: THREE.Object3D[]; rear: THREE.Object3D[] };
}

const JUMBO = {
  tyre: { od: 1.22, width: 0.4, rim: 0.61, pattern: "rock", lugs: 20, depth: 0.05, segments: 48 } as TyreSpec,
  axleY: 0.6,
  trackX: 0.92,
  frontAxle: 1.45,
  rearAxle: -2.0,
  boomBase: { x: 0.62, y: 1.45, z: 3.45 },
  boomLen: 3.3,
  feed: { y: 0.36, z0: 1.1, z1: 7.1 },
  rodLen: 5.5,
  travel: 4.5,
  shiftZ: -3.4,
};

interface JumboTemplate {
  front: THREE.Group;
  rear: THREE.Group;
  frontWheels: THREE.Group;
  rearWheels: THREE.Group;
  turntable: THREE.Group;
  boom: THREE.Group;
  feed: THREE.Group;
  drifter: THREE.Group;
  rod: THREE.Group;
  jackLeg: THREE.Group;
  lens: Lenses;
}
const jumboTemplates = new Map<string, JumboTemplate>();

function buildJumbo(colour: string): JumboTemplate {
  const P = paint(colour);
  const D = mats.dark();
  const K = mats.black();
  const S = mats.steel();
  const C = mats.chrome();
  const GV = mats.galv();
  const GR = mats.grate();
  const RED = mats.red();
  const lens = lenses();
  const J = JUMBO;
  const ay = J.axleY;

  // ───── front frame (hinge at origin) ─────
  const f = new THREE.Group();
  box(f, P, 1.1, 0.75, 3.3, 0, 0.78, 1.8, { r: 0.05 });
  for (const y of [0.55, 1.1]) box(f, P, 0.6, 0.12, 0.5, 0, y, 0.2, { r: 0.03 });
  cyl(f, D, 0.22, 1.4, 0, ay, J.frontAxle, { axis: "x", seg: 18 });
  // deck over the front wheels, with the operator's canopy
  deck(f, GR, 2.5, 2.5, 0, 1.4, 1.35, 0.08);
  box(f, P, 2.52, 0.18, 2.52, 0, 1.28, 1.35, { r: 0.04 });
  for (const s of [-1, 1]) {
    arcBand(f, P, ay, J.frontAxle, 0.7, Math.min(s * 0.68, s * 1.18), Math.max(s * 0.68, s * 1.18), 0.7, Math.PI - 0.7, 0.04, 6);
    tape(f, 0.01, 0.06, 2.3, s * 1.265, 1.28, 1.35);
  }
  // canopy: four posts, FOPS roof with grille, side screens
  const posts: [number, number][] = [
    [-1.12, 0.25],
    [1.12, 0.25],
    [-1.12, 2.5],
    [1.12, 2.5],
  ];
  for (const [x, z] of posts) box(f, P, 0.1, 1.68, 0.1, x, 2.15, z, { r: 0.02 });
  box(f, P, 2.45, 0.12, 2.55, 0, 3.02, 1.38, { r: 0.04 });
  for (let i = 0; i < 7; i++) box(f, D, 2.3, 0.03, 0.04, 0, 3.1, 0.3 + i * 0.36, { r: 0.006 });
  for (const s of [-1, 1]) {
    for (let i = 0; i < 5; i++) bar(f, P, v3(s * 1.12, 1.45, 0.62 + i * 0.4), v3(s * 1.12, 2.0, 0.62 + i * 0.4), 0.015, 5);
    bar(f, P, v3(s * 1.12, 2.0, 0.3), v3(s * 1.12, 2.0, 2.45), 0.025, 6);
  }
  // operator seat and console looking at the booms
  box(f, K, 0.55, 0.12, 0.55, 0.3, 1.78, 1.1, { r: 0.04 });
  box(f, K, 0.55, 0.65, 0.12, 0.3, 2.05, 0.82, { r: 0.04, rx: -0.15 });
  box(f, D, 0.5, 0.5, 0.45, 0.3, 1.58, 1.1, { r: 0.03 });
  box(f, D, 1.6, 0.85, 0.4, 0, 1.78, 2.15, { r: 0.04 });
  box(f, mats.glass(), 0.45, 0.32, 0.03, 0.3, 2.12, 1.95, { rx: -0.5, r: 0.008 });
  for (const s of [-1, 1]) for (let k = 0; k < 2; k++) cyl(f, K, 0.02, 0.18, s * 0.25 + 0.3 + k * 0.1 * s, 2.28, 2.0, { seg: 6 });
  // roof lamps toward the face, beacon
  for (const x of [-0.95, -0.45, 0.45, 0.95]) lamp(f, D, lens.white, x, 3.14, 2.55, { w: 0.28, h: 0.18, pitch: 0.18 });
  beacon(f, K, lens.amber, -0.9, 3.08, 0.5, 1.0);
  // boom foundation block
  box(f, P, 2.1, 1.0, 0.95, 0, 1.12, 3.0, { r: 0.06 });
  box(f, D, 2.2, 0.16, 0.2, 0, 0.6, 3.45, { r: 0.03 });
  chevrons(f, 0, 0.6, 3.56, 1.6, 0.12, 1);
  // front stabiliser housings
  for (const s of [-1, 1]) {
    box(f, P, 0.3, 0.6, 0.3, s * 1.1, 0.95, 3.15, { r: 0.03 });
    cyl(f, P, 0.13, 0.75, s * 1.1, 0.88, 3.15, { seg: 14 });
  }
  // hose bundles from the block toward the booms
  for (const s of [-1, 1])
    for (let i = 0; i < 3; i++) tube(f, mats.hose(), [v3(s * (0.2 + i * 0.1), 1.62, 2.6), v3(s * (0.3 + i * 0.1), 1.9, 3.0), v3(s * (0.55 + i * 0.05), 1.85, 3.4)], 0.025, { radial: 6 });
  const front = bakeFree(f, { mudTop: 1.2, mudBottom: 0.15, mud: 1.1, dust: 0.9 });

  // ───── rear frame (hinge at origin, extends -z) ─────
  const r = new THREE.Group();
  const ra = J.rearAxle;
  box(r, P, 1.1, 0.8, 4.55, 0, 0.8, -2.33, { r: 0.05 });
  cyl(r, D, 0.22, 1.4, 0, ay, ra, { axis: "x", seg: 18 });
  // power pack covers: sloped hood over the wheels, cabinets either side
  prism(r, P, [[-0.4, 1.25], [-3.3, 1.25], [-3.3, 1.85], [-3.0, 2.0], [-0.8, 2.0], [-0.4, 1.7]], 2.4, "zy", 0, { bevel: 0.05 });
  box(r, P, 1.5, 0.7, 0.9, 0, 0.95, -3.0, { r: 0.05 }); // power pack below the covers' tail
  for (const s of [-1, 1]) {
    arcBand(r, P, ay, ra, 0.7, Math.min(s * 0.68, s * 1.18), Math.max(s * 0.68, s * 1.18), 0.5, Math.PI - 0.5, 0.04, 6);
    for (let i = 0; i < 5; i++) box(r, D, 0.02, 0.035, 0.7, s * 1.21, 1.4 + i * 0.1, -2.8, { r: 0.005 });
    box(r, D, 0.015, 0.75, 0.02, s * 1.21, 1.55, -1.2, { r: 0.004 });
    box(r, D, 0.015, 0.75, 0.02, s * 1.21, 1.55, -2.6, { r: 0.004 });
    box(r, S, 0.03, 0.04, 0.12, s * 1.22, 1.55, -1.9, { r: 0.006 });
    tape(r, 0.01, 0.06, 2.7, s * 1.21, 1.32, -1.9);
  }
  // electrical cabinet with gauges on the left
  box(r, D, 0.12, 0.5, 0.6, 1.25, 1.55, -0.9, { r: 0.02 });
  for (const z of [-0.75, -1.05]) cyl(r, mats.glass(), 0.06, 0.02, 1.32, 1.62, z, { axis: "x", seg: 12 });
  // cable reel at the back: drum with flanges and wound trailing cable
  const reelY = 1.6;
  const reelZ = -3.95;
  cyl(r, mats.orange(), 0.6, 1.5, 0, reelY, reelZ, { axis: "x", seg: 28 });
  for (let i = 0; i < 9; i++) cyl(r, mats.orange(), 0.63, 0.1, -0.7 + i * 0.175, reelY, reelZ, { axis: "x", seg: 28 });
  for (const s of [-1, 1]) {
    cyl(r, P, 0.82, 0.05, s * 0.8, reelY, reelZ, { axis: "x", seg: 32 });
    box(r, P, 0.12, 1.1, 0.22, s * 0.9, 1.15, reelZ, { r: 0.03 });
  }
  cyl(r, D, 0.12, 1.85, 0, reelY, reelZ, { axis: "x", seg: 12 });
  // cable guide arm down to the floor at the back
  bar(r, P, v3(0, 0.9, -4.5), v3(0, 0.55, -4.95), 0.06, 8);
  cyl(r, S, 0.12, 0.3, 0, 0.55, -4.95, { axis: "x", seg: 12 });
  tube(r, mats.orange(), [v3(0.3, reelY - 0.62, reelZ - 0.1), v3(0.2, 0.72, -4.7), v3(0.1, 0.45, -5.05), v3(0.0, 0.05, -5.9), v3(-0.2, 0.05, -7.5)], 0.045, { radial: 8 });
  // water hose reel on top, behind the canopy
  cyl(r, mats.hose(), 0.36, 0.8, 0, 2.38, -1.25, { axis: "x", seg: 22 });
  for (const s of [-1, 1]) cyl(r, P, 0.48, 0.04, s * 0.44, 2.38, -1.25, { axis: "x", seg: 24 });
  for (const s of [-1, 1]) box(r, P, 0.08, 0.5, 0.15, s * 0.5, 2.1, -1.25, { r: 0.02 });
  // rear stabilisers, rear lamps, chevrons, tail lights
  for (const s of [-1, 1]) {
    box(r, P, 0.3, 0.6, 0.3, s * 1.05, 0.95, -3.3, { r: 0.03 });
    cyl(r, P, 0.13, 0.75, s * 1.05, 0.88, -3.3, { seg: 14 });
    box(r, P, 0.1, 0.5, 0.1, s * 1.0, 1.95, -3.5, { r: 0.02 });
    lamp(r, D, lens.white, s * 1.0, 2.25, -3.56, { w: 0.26, h: 0.16, yaw: Math.PI, pitch: 0.1 });
    lamp(r, D, lens.red, s * 0.95, 0.62, -4.68, { w: 0.16, h: 0.1, yaw: Math.PI });
  }
  box(r, P, 2.2, 0.35, 0.25, 0, 0.45, -4.6, { r: 0.04 });
  chevrons(r, 0, 0.45, -4.74, 1.5, 0.2, -1);
  bottle(r, RED, S, -1.0, 2.0, -0.5, 0.5, 0.08);
  bottle(r, RED, S, -0.8, 2.0, -0.5, 0.5, 0.08);
  ladder(r, P, v3(-1.3, 0.3, -0.55), v3(-1.3, 1.2, -0.45), 0.4, 0.28);
  const rear = bakeFree(r, { mudTop: 1.2, mudBottom: 0.15, mud: 1.1, dust: 0.9 });

  // ───── booms: turntable (yaws), boom (pitches), feed (on the boom tip), drifter (slides), rod ─────
  const tt = new THREE.Group();
  cyl(tt, D, 0.3, 0.2, 0, 0, -0.05, { axis: "z", seg: 20 });
  box(tt, P, 0.5, 0.7, 0.45, 0, -0.05, 0.15, { r: 0.05 });
  cyl(tt, S, 0.1, 0.62, 0, 0, 0.25, { axis: "x", seg: 12 });
  const turntable = bakeFree(tt, { mudTop: 0, mudBottom: -1.5, mud: 0.6 });
  const bm = new THREE.Group();
  const BL = J.boomLen;
  prism(bm, P, [[0.0, -0.22], [BL, -0.16], [BL + 0.2, 0.0], [BL, 0.16], [0.0, 0.22], [-0.25, 0]], 0.32, "zy", 0, { bevel: 0.03 });
  cyl(bm, S, 0.12, 0.5, 0, 0, 0, { axis: "x", seg: 12 });
  // tip: feed holder/rotation unit
  box(bm, D, 0.42, 0.5, 0.5, 0, 0.18, BL, { r: 0.05 });
  cyl(bm, D, 0.2, 0.36, 0, 0.42, BL, { seg: 16 });
  // hoses running along the boom side
  for (let i = 0; i < 3; i++) tube(bm, mats.hose(), [v3(0.2 + i * 0.05, 0.05, 0.1), v3(0.24 + i * 0.05, 0.18, BL * 0.5), v3(0.22 + i * 0.05, 0.3, BL - 0.1), v3(0.18, J.feed.y, BL + 0.6)], 0.022, { radial: 5 });
  const boom = bakeFree(bm, { mudTop: 0.3, mudBottom: -1.5, mud: 0.6 });
  // feed beam: aluminium profile with rails, rear end cap, front centraliser and rod support
  const fd = new THREE.Group();
  const fz0 = J.feed.z0;
  const fz1 = J.feed.z1;
  const fy = J.feed.y;
  box(fd, GV, 0.2, 0.18, fz1 - fz0, 0, fy, (fz0 + fz1) / 2, { r: 0.02 });
  for (const s of [-1, 1]) box(fd, S, 0.05, 0.05, fz1 - fz0 - 0.1, s * 0.12, fy + 0.11, (fz0 + fz1) / 2, { r: 0.01 });
  box(fd, D, 0.32, 0.3, 0.2, 0, fy + 0.05, fz0 - 0.05, { r: 0.03 });
  box(fd, D, 0.36, 0.34, 0.3, 0, fy + 0.18, fz1 - 0.12, { r: 0.04 });
  cyl(fd, S, 0.09, 0.32, 0, fy + 0.3, fz1 - 0.12, { axis: "z", seg: 14 });
  box(fd, D, 0.28, 0.24, 0.2, 0, fy + 0.16, (fz0 + fz1) / 2 + 0.6, { r: 0.03 });
  // feed cylinder under the beam and the cradle on the boom tip
  cyl(fd, C, 0.04, fz1 - fz0 - 0.6, 0, fy - 0.13, (fz0 + fz1) / 2, { axis: "z", seg: 8 });
  box(fd, D, 0.34, 0.22, 0.7, 0, fy - 0.12, JUMBO.boomLen, { r: 0.03 });
  // hose guide loop at the back
  tube(fd, mats.hose(), [v3(0.05, fy + 0.2, fz0 + 0.1), v3(0.05, fy + 0.65, fz0 + 0.3), v3(0.05, fy + 0.6, fz0 + 1.2), v3(0.05, fy + 0.35, fz0 + 1.8)], 0.035, { radial: 6 });
  const feed = bakeFree(fd, { mudTop: 0, mudBottom: -1, mud: 0.4, dust: 0.8 });
  // rock drill (drifter) riding on the feed
  const dr = new THREE.Group();
  box(dr, K, 0.32, 0.3, 0.85, 0, 0.17, 0, { r: 0.05 });
  box(dr, RED, 0.26, 0.12, 0.6, 0, 0.36, -0.05, { r: 0.03 });
  cyl(dr, D, 0.09, 0.3, 0.14, 0.32, 0.12, { axis: "z", seg: 12 });
  cyl(dr, D, 0.09, 0.3, -0.14, 0.32, 0.12, { axis: "z", seg: 12 });
  box(dr, S, 0.36, 0.08, 0.7, 0, 0.0, 0, { r: 0.02 });
  const drifter = bakeFree(dr, { mudTop: 0, mudBottom: -1, mud: 0.3, dust: 0.8 });
  // drill steel with shank adapter, coupling and button bit, along +z from the drifter nose
  const rd = new THREE.Group();
  cyl(rd, S, 0.035, 0.32, 0, 0, 0.16, { axis: "z", seg: 10 });
  cyl(rd, S, 0.019, J.rodLen, 0, 0, 0.32 + J.rodLen / 2, { axis: "z", seg: 6 });
  cyl(rd, S, 0.028, 0.18, 0, 0, 0.36, { axis: "z", seg: 8 });
  cyl(rd, mats.steel(), 0.026, 0.1, 0, 0, 0.32 + J.rodLen, { axis: "z", seg: 10, r2: 0.022 });
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2;
    sphere(rd, D, 0.009, Math.cos(a) * 0.016, Math.sin(a) * 0.016, 0.37 + J.rodLen, { seg: 5 });
  }
  const rod = bakeFree(rd, { mudTop: 0, mudBottom: -1, mud: 0.2 });
  // stabiliser inner leg with foot (along -y from its top)
  const jl = new THREE.Group();
  cyl(jl, C, 0.085, 0.9, 0, -0.45, 0, { seg: 12 });
  cyl(jl, D, 0.2, 0.06, 0, -0.9, 0, { seg: 14 });
  const jackLeg = bakeFree(jl, { mudTop: -0.5, mudBottom: -1, mud: 0.9 });

  const wheelSet = () => {
    const w = new THREE.Group();
    for (const s of [-1, 1]) {
      const n = node(w, s * J.trackX, 0, 0, { ry: s > 0 ? 0 : Math.PI });
      addTyre(n, J.tyre);
      rim(n, mats.dark(), mats.dark(), S, { rim: J.tyre.rim, width: J.tyre.width, dish: 0.06, bolts: 12, hubR: 0.16, cap: false });
    }
    return bakeWheels(w);
  };
  return { front, rear, frontWheels: wheelSet(), rearWheels: wheelSet(), turntable, boom, feed, drifter, rod, jackLeg, lens };
}

/** Builds a two-boom electro-hydraulic face drill rig (jumbo). */
export function makeJumbo(opts: { colour?: string } = {}): Jumbo {
  const colour = opts.colour ?? COLOURS.yellow;
  let tpl = jumboTemplates.get(colour);
  if (!tpl) {
    tpl = buildJumbo(colour);
    jumboTemplates.set(colour, tpl);
  }
  const J = JUMBO;
  const lens = lenses();
  const group = new THREE.Group();
  group.name = "jumbo";
  const body = new THREE.Group();
  body.position.z = J.shiftZ;
  group.add(body);
  const front = new THREE.Group();
  const rear = new THREE.Group();
  body.add(front, rear);
  const fb = instance(tpl.front);
  const rb = instance(tpl.rear);
  swapLenses(fb, tpl.lens, lens);
  swapLenses(rb, tpl.lens, lens);
  front.add(fb);
  rear.add(rb);
  const fw = new THREE.Group();
  fw.position.set(0, J.axleY, J.frontAxle);
  fw.add(instance(tpl.frontWheels));
  front.add(fw);
  const rw = new THREE.Group();
  rw.position.set(0, J.axleY, J.rearAxle);
  rw.add(instance(tpl.rearWheels));
  rear.add(rw);
  // booms
  interface BoomRig {
    yaw: THREE.Group;
    pitch: THREE.Group;
    drifter: THREE.Group;
    rod: THREE.Group;
    ram: Ram;
  }
  const rigs: BoomRig[] = [];
  for (const s of [1, -1]) {
    const yaw = new THREE.Group();
    yaw.position.set(s * J.boomBase.x, J.boomBase.y, J.boomBase.z);
    yaw.add(instance(tpl.turntable));
    const pitch = new THREE.Group();
    pitch.position.set(0, 0, 0.25);
    pitch.add(instance(tpl.boom), instance(tpl.feed));
    const drifter = new THREE.Group();
    drifter.add(instance(tpl.drifter));
    const rod = new THREE.Group();
    rod.add(instance(tpl.rod));
    pitch.add(drifter, rod);
    yaw.add(pitch);
    front.add(yaw);
    const ram = makeRam(yaw, { r: 0.09, min: 1.05, max: 1.75 });
    rigs.push({ yaw, pitch, drifter, rod, ram });
  }
  // stabiliser legs
  const legs: THREE.Group[] = [];
  for (const [frame, z] of [
    [front, 3.15],
    [rear, -3.3],
  ] as const) {
    for (const s of [-1, 1]) {
      const g = new THREE.Group();
      g.position.set(s * (z > 0 ? 1.1 : 1.05), 0.95, z);
      g.add(instance(tpl.jackLeg));
      frame.add(g);
      legs.push(g);
    }
  }
  const anchors = { front: [] as THREE.Object3D[], rear: [] as THREE.Object3D[] };
  for (const x of [-0.7, 0.7]) {
    const a = new THREE.Object3D();
    a.position.set(x, 3.14, 2.6);
    a.rotation.x = 0.18;
    front.add(a);
    anchors.front.push(a);
    const b = new THREE.Object3D();
    b.position.set(x * 1.43, 2.25, -3.6);
    b.rotation.y = Math.PI;
    rear.add(b);
    anchors.rear.push(b);
  }

  const state: JumboState = {
    booms: [
      { yaw: 0.06, pitch: 0.06, feed: 0 },
      { yaw: -0.06, pitch: 0.06, feed: 0 },
    ],
    drilling: false,
    stabilisers: 0,
    steer: 0,
    speed: 0,
    distance: 0,
    lights: true,
  };
  let spin = 0;
  const pa = new THREE.Vector3();
  const pb = new THREE.Vector3();
  const apply = () => {
    rigs.forEach((rg, i) => {
      const b = state.booms[i]!;
      rg.yaw.rotation.y = b.yaw;
      rg.pitch.rotation.x = -b.pitch;
      const zD = J.feed.z0 + 0.5 + clamp01(b.feed) * J.travel;
      rg.drifter.position.set(0, J.feed.y + 0.14, zD);
      rg.rod.position.set(0, J.feed.y + 0.3, zD + 0.42);
      rg.rod.rotation.z = spin;
      // lift ram from under the turntable to the boom's belly
      const c = Math.cos(-b.pitch);
      const sn = Math.sin(-b.pitch);
      const by = -0.2;
      const bz = 1.5;
      pa.set(0, -0.5, 0.1);
      pb.set(0, by * c - bz * sn, 0.25 + by * sn + bz * c);
      rg.ram.set(pa, pb);
    });
    const ext = clamp01(state.stabilisers);
    for (const g of legs) g.children[0]!.position.y = 0.35 - ext * 0.38;
    const st = Math.max(-0.7, Math.min(0.7, state.steer));
    front.rotation.y = st / 2;
    rear.rotation.y = -st / 2;
    const roll = state.distance / (J.tyre.od / 2);
    fw.rotation.x = roll;
    rw.rotation.x = roll;
    lens.set(state.lights);
  };
  const jumbo: Jumbo = {
    group,
    state,
    lenses: lens,
    anchors,
    set(s) {
      const { booms, ...rest } = s;
      Object.assign(state, rest);
      if (booms) booms.forEach((b, i) => Object.assign(state.booms[i]!, b));
      apply();
    },
    update(dt) {
      let dirty = false;
      if (state.drilling) {
        spin = (spin + dt * 22) % (Math.PI * 2);
        dirty = true;
      }
      if (state.speed) {
        state.distance += state.speed * dt;
        dirty = true;
      }
      if (dirty) apply();
    },
  };
  apply();
  return jumbo;
}

