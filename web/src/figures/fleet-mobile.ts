/**
 * Mobile pit equipment: a large wheel loader (≈19 m³ bucket, 3.8 m tyres), a large crawler dozer with
 * a U-blade and single-shank ripper, and a tracked rotary blasthole drill with a 20 m mast.
 * Generic paint, no lettering. Metres, y up, +z forward, origin on the ground under the machine.
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
  rim,
  rng,
  sphere,
  stairs,
  tyreGeometry,
  v3,
  COLOURS,
  type Lenses,
  type PartWear,
  type Track,
  type TyreSpec,
  type V3,
} from "./fleet-kit";

const D2R = Math.PI / 180;
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const Y = new THREE.Vector3(0, 1, 0);

// ───────────────────────────── shared pieces ─────────────────────────────

/** Replaces a template's lens materials with a machine's own. */
function swapLenses(root: THREE.Object3D, from: Lenses, to: Lenses) {
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh) return;
    if (m.material === from.white) m.material = to.white;
    else if (m.material === from.amber) m.material = to.amber;
    else if (m.material === from.red) m.material = to.red;
  });
}

/** Rims baked by material plus all tyres merged into one rubber mesh (tyres keep their own wear). */
function wheelSet(spec: TyreSpec, wheels: { x: number; flip: boolean; dish: number }[]): THREE.Group {
  const src = new THREE.Group();
  const tyres: THREE.BufferGeometry[] = [];
  const m4 = new THREE.Matrix4();
  for (const w of wheels) {
    const n = node(src, w.x, 0, 0, { ry: w.flip ? Math.PI : 0 });
    rim(n, mats.rimGrey(), mats.dark(), mats.steel(), { rim: spec.rim, width: spec.width, dish: w.dish, bolts: 28, hubR: 0.46 });
    const g = tyreGeometry(spec).clone();
    m4.makeRotationY(w.flip ? Math.PI : 0).setPosition(w.x, 0, 0);
    g.applyMatrix4(m4);
    tyres.push(g);
  }
  const out = new THREE.Group();
  out.add(bakeFree(src, { mudTop: 0.4, mudBottom: -1.8, mud: 0.6, dust: 0.5 }));
  let n = 0;
  for (const g of tyres) n += g.getAttribute("position").count;
  const merged = new THREE.BufferGeometry();
  for (const a of ["position", "normal", "uv", "wear"] as const) {
    const size = tyres[0]!.getAttribute(a).itemSize;
    const arr = new Float32Array(n * size);
    let o = 0;
    for (const g of tyres) {
      const s = g.getAttribute(a).array as Float32Array;
      arr.set(s, o);
      o += s.length;
    }
    merged.setAttribute(a, new THREE.BufferAttribute(arr, size));
  }
  merged.computeBoundingSphere();
  for (const g of tyres) g.dispose();
  const mesh = new THREE.Mesh(merged, mats.rubber());
  mesh.castShadow = mesh.receiveShadow = true;
  out.add(mesh);
  return out;
}

interface RamTpl {
  stages: THREE.Group[];
  len: number;
}
const ramCache = new Map<string, RamTpl>();

/**
 * A telescoping hydraulic ram sized for a pin-to-pin range: painted barrel with a base eye, chrome
 * stages, and a painted rod-end eye. Enough stages that every overlap stays ≥ 0.25 m at full stroke.
 */
function ramTemplate(colour: string, r0: number, lmin: number, lmax: number): RamTpl {
  const key = `${colour}:${r0}:${lmin.toFixed(2)}:${lmax.toFixed(2)}`;
  const hit = ramCache.get(key);
  if (hit) return hit;
  const P = paint(colour);
  const C = mats.chrome();
  const len = Math.max(0.6, lmin - 0.14);
  const n = Math.max(2, 1 + Math.ceil((lmax - len) / Math.max(0.2, len - 0.25)));
  const stages: THREE.Group[] = [];
  for (let i = 0; i < n; i++) {
    const g = new THREE.Group();
    const r = r0 * Math.pow(0.78, i);
    const last = i === n - 1;
    cyl(g, i === 0 ? P : C, r, len, 0, len / 2, 0, { seg: 16 });
    if (i === 0) {
      cyl(g, P, r * 1.12, 0.1, 0, len - 0.05, 0, { seg: 16 }); // gland
      cyl(g, P, r * 1.08, 0.16, 0, 0.08, 0, { seg: 16 });
      cyl(g, P, r * 0.7, r * 2.1, 0, 0, 0, { axis: "x", seg: 12 }); // base eye
      // hose fitting on the barrel
      box(g, mats.dark(), r * 0.5, r * 0.5, r * 0.9, 0, len * 0.85, r * 1.05, { r: 0.01 });
    }
    if (last) {
      cyl(g, P, r * 1.15, r * 1.3, 0, len, 0, { seg: 14 });
      cyl(g, P, r * 0.8, r * 1.9, 0, len, 0, { axis: "x", seg: 12 }); // rod eye
    }
    stages.push(bakeFree(g, { mudTop: 0.5, mudBottom: -2, mud: 0.4, dust: 0.6 }));
  }
  const tpl = { stages, len };
  ramCache.set(key, tpl);
  return tpl;
}

interface Ram {
  root: THREE.Group;
  /** Places the ram between two pins given in its parent's frame. */
  set(a: V3, b: V3): void;
}
function makeRam(parent: THREE.Object3D, tpl: RamTpl): Ram {
  const root = new THREE.Group();
  const stages = tpl.stages.map((s) => instance(s));
  root.add(...stages);
  parent.add(root);
  const d = new THREE.Vector3();
  const n = stages.length;
  return {
    root,
    set(a, b) {
      root.position.copy(a);
      d.copy(b).sub(a);
      const L = d.length();
      if (L < 1e-6) return;
      root.quaternion.setFromUnitVectors(Y, d.multiplyScalar(1 / L));
      for (let i = 0; i < n; i++) stages[i]!.position.y = Math.max(0, (i * (L - tpl.len)) / (n - 1));
    },
  };
}

/** Range of a pin-to-pin distance over a sampled pose grid. */
function range(f: (a: number, b: number) => number, na = 16, nb = 16): [number, number] {
  let lo = Infinity;
  let hi = 0;
  for (let i = 0; i <= na; i++)
    for (let j = 0; j <= nb; j++) {
      const v = f(i / na, j / nb);
      lo = Math.min(lo, v);
      hi = Math.max(hi, v);
    }
  return [lo, hi];
}

/** Box with a grille face: dark core, horizontal louvres and a painted surround, facing +z (local). */
function grille(parent: THREE.Object3D, frame: THREE.Material, w: number, h: number, x: number, y: number, z: number, ry = 0, bars = 0.14) {
  const g = node(parent, x, y, z, { ry });
  box(g, mats.black(), w - 0.12, h - 0.12, 0.08, 0, 0, -0.05, { r: 0.01 });
  const n = Math.floor((h - 0.2) / bars);
  for (let i = 0; i < n; i++) box(g, mats.dark(), w - 0.14, 0.035, 0.09, 0, -h / 2 + 0.14 + i * bars, 0.0, { rx: -0.5, r: 0.008 });
  box(g, frame, w, 0.09, 0.16, 0, h / 2 - 0.045, 0.02, { r: 0.02 });
  box(g, frame, w, 0.09, 0.16, 0, -h / 2 + 0.045, 0.02, { r: 0.02 });
  for (const s of [-1, 1]) box(g, frame, 0.09, h, 0.16, (s * (w - 0.09)) / 2, 0, 0.02, { r: 0.02 });
  return g;
}

/** Vertical louvre band on a side panel (local +x faces out). */
function louvres(parent: THREE.Object3D, mat: THREE.Material, x: number, y: number, z: number, h: number, d: number, side: number, step = 0.12) {
  const n = Math.floor(h / step);
  for (let i = 0; i < n; i++) box(parent, mat, 0.04, 0.03, d, x + side * 0.02, y - h / 2 + (i + 0.5) * step, z, { rz: side * 0.5, r: 0.006 });
}

/** Operator cab: painted shell, dark glass on every face, roof lip, door seam, wipers. Faces +z. */
function cab(parent: THREE.Object3D, P: THREE.Material, w: number, h: number, d: number, x: number, y: number, z: number, o: { door?: number; glassTop?: number } = {}) {
  const c = node(parent, x, y, z);
  const G = mats.glass();
  const K = mats.black();
  box(c, P, w, h, d, 0, h / 2, 0, { r: 0.1, seg: 2 });
  box(c, P, w + 0.12, 0.1, d + 0.12, 0, h + 0.03, 0, { r: 0.04 });
  const gb = h * 0.36;
  const gt = h - (o.glassTop ?? 0.18);
  const gh = gt - gb;
  const gy = (gt + gb) / 2;
  box(c, G, w - 0.22, gh, 0.05, 0, gy, d / 2 + 0.005, { r: 0.015 });
  box(c, G, w - 0.3, gh * 0.85, 0.05, 0, gy + gh * 0.07, -d / 2 - 0.005, { r: 0.015 });
  for (const s of [-1, 1]) {
    box(c, G, 0.05, gh, d * 0.48 - 0.12, (s * w) / 2 + s * 0.005, gy, d * 0.24, { r: 0.015 });
    box(c, G, 0.05, gh, d * 0.48 - 0.12, (s * w) / 2 + s * 0.005, gy, -d * 0.24, { r: 0.015 });
  }
  // lower glass in the front for the view down to the bucket/blade
  box(c, G, w - 0.5, gb * 0.55, 0.05, 0, gb * 0.55, d / 2 + 0.005, { r: 0.015 });
  bar(c, K, v3(-w * 0.2, gb + 0.05, d / 2 + 0.04), v3(-w * 0.1, gt - 0.1, d / 2 + 0.04), 0.016, 5);
  bar(c, K, v3(w * 0.15, gb + 0.05, d / 2 + 0.04), v3(w * 0.25, gt - 0.1, d / 2 + 0.04), 0.016, 5);
  const ds = o.door ?? 1;
  box(c, mats.steel(), 0.04, 0.04, 0.18, (ds * w) / 2 + ds * 0.02, h * 0.45, 0.0, { r: 0.008 });
  bar(c, P, v3((ds * w) / 2 + ds * 0.06, h * 0.15, d / 2 - 0.12), v3((ds * w) / 2 + ds * 0.06, h * 0.8, d / 2 - 0.12), 0.022, 6);
  return c;
}

/** Arc fender over a wheel, side profile around the axle, extruded across x. */
function fender(parent: THREE.Object3D, mat: THREE.Material, r: number, a0: number, a1: number, x: number, width: number, ay: number, az: number) {
  const pts: [number, number][] = [];
  const n = 14;
  for (let i = 0; i <= n; i++) {
    const a = lerp(a0, a1, i / n);
    pts.push([az + Math.cos(a) * (r + 0.12), ay + Math.sin(a) * (r + 0.12)]);
  }
  for (let i = n; i >= 0; i--) {
    const a = lerp(a0, a1, i / n);
    pts.push([az + Math.cos(a) * r, ay + Math.sin(a) * r]);
  }
  prism(parent, mat, pts, width, "zy", x, { bevel: 0.02 });
}

// ───────────────────────────── wheel loader ─────────────────────────────

/** Loader dimensions (front-frame z/y plane, metres). */
export const LOADER = {
  tyre: { od: 3.83, width: 1.47, rim: 1.45, lugs: 28, depth: 0.095 } satisfies TyreSpec,
  axleY: 1.9,
  frontZ: 3.3,
  rearZ: -3.3,
  trackX: 2.45,
  arm: { y: 4.25, z: 1.6, len: 6.2, x: 1.42 },
  liftLo: -34.4 * D2R,
  liftHi: 52 * D2R,
  bell: { s: 4.6, off: 0.7, r1: 0.85, r2: 1.1, link: 1.68 },
  linkPin: { z: -0.3, y: 0.7 },
  tiltF: { z: 1.4, y: 3.4 },
  liftF: { z: 1.0, y: 1.35, x: 0.98 },
  liftOnArm: { s: 3.7, off: -0.45 },
} as const;

type P2 = { z: number; y: number };
const WL = LOADER;
const armAngle = (lift: number) => lerp(WL.liftLo, WL.liftHi, clamp01(lift));
const bucketPitch = (tilt: number) => {
  const t = clamp01(tilt);
  return (t < 0.5 ? lerp(-45, 8, t / 0.5) : lerp(8, 42, (t - 0.5) / 0.5)) * D2R;
};
const onArm = (th: number, s: number, off: number): P2 => ({
  z: WL.arm.z + s * Math.cos(th) - off * Math.sin(th),
  y: WL.arm.y + s * Math.sin(th) + off * Math.cos(th),
});
const rotP = (p: P2, pitch: number): P2 => ({ y: p.y * Math.cos(pitch) + p.z * Math.sin(pitch), z: -p.y * Math.sin(pitch) + p.z * Math.cos(pitch) });

/** Z-bar pose: arm angle, bucket pitch, bellcrank pivot and ends, link and ram pins. */
function loaderPose(lift: number, tilt: number) {
  const th = armAngle(lift);
  const pitch = bucketPitch(tilt);
  const pin = onArm(th, WL.arm.len, 0);
  const lp = rotP(WL.linkPin, pitch);
  const B = { z: pin.z + lp.z, y: pin.y + lp.y };
  const P = onArm(th, WL.bell.s, WL.bell.off);
  const dz = B.z - P.z;
  const dy = B.y - P.y;
  const d = Math.hypot(dz, dy);
  const { r1, link: Lk, r2 } = WL.bell;
  const a = (r1 * r1 - Lk * Lk + d * d) / (2 * d);
  const h = Math.sqrt(Math.max(0, r1 * r1 - a * a));
  const E = { z: P.z + (dz * a) / d + (dy / d) * h, y: P.y + (dy * a) / d - (dz / d) * h };
  const bz = (E.z - P.z) / r1;
  const by = (E.y - P.y) / r1;
  const U = { z: P.z - r2 * bz, y: P.y - r2 * by };
  const liftPin = onArm(th, WL.liftOnArm.s, WL.liftOnArm.off);
  return { th, pitch, pin, B, P, E, U, beta: Math.atan2(by, bz), liftPin };
}

export interface WheelLoaderState {
  /** Lift arms 0 (bucket on the ground) .. 1 (full height). */
  lift: number;
  /** Bucket 0 dumped (lip down), ~0.45 level on the ground, 0.5 carry, 1 racked back. */
  tilt: number;
  /** Articulation angle, rad; positive turns left (+x). */
  steer: number;
  speed: number;
  distance: number;
  lights: boolean;
}
export interface WheelLoader {
  group: THREE.Group;
  state: WheelLoaderState;
  set(s: Partial<WheelLoaderState>): void;
  update(dt: number): void;
  lenses: Lenses;
}

interface LoaderTpl {
  rear: THREE.Group;
  front: THREE.Group;
  arms: THREE.Group;
  bucket: THREE.Group;
  bell: THREE.Group;
  link: THREE.Group;
  wheels: THREE.Group;
  tilt: RamTpl;
  liftRam: RamTpl;
  lens: Lenses;
}
const loaderTpls = new Map<string, LoaderTpl>();

function buildLoader(colour: string): LoaderTpl {
  const P = paint(colour);
  const D = mats.dark();
  const K = mats.black();
  const S = mats.steel();
  const C = mats.chrome();
  const GR = mats.grate();
  const lens = lenses();
  const ay = WL.axleY;
  const tx = WL.trackX;

  // ── rear frame: engine module, radiator, counterweight, cab and platform ──
  const r = new THREE.Group();
  box(r, P, 1.5, 1.4, 6.9, 0, 1.85, -3.85, { r: 0.08 });
  for (const yy of [1.45, 2.75]) box(r, P, 1.2, 0.3, 0.7, 0, yy, -0.3, { r: 0.05 }); // hitch ears
  cyl(r, S, 0.16, 1.9, 0, 2.1, 0, { seg: 12 }); // articulation pin
  // rear axle and oscillation
  cyl(r, P, 0.55, 3.4, 0, ay, WL.rearZ, { axis: "x", seg: 22 });
  sphere(r, P, 0.75, 0, ay, WL.rearZ, { seg: 18 });
  for (const s of [-1, 1]) {
    fender(r, P, 2.07, 0.15, Math.PI - 0.12, s * tx, 1.6, ay, WL.rearZ);
    // engine enclosure side walkway over the fender
    deck(r, GR, 1.75, 6.5, s * 2.4, 4.3, -3.6, 0.1);
    box(r, P, 0.14, 0.32, 6.5, s * 3.24, 4.13, -3.6, { r: 0.03 });
  }
  // engine enclosure with side doors and louvres
  box(r, P, 3.0, 3.4, 3.9, 0, 4.1, -4.55, { r: 0.14, seg: 2 });
  box(r, P, 2.7, 0.18, 3.6, 0, 5.85, -4.55, { r: 0.06 });
  for (const s of [-1, 1]) {
    for (const z of [-5.9, -4.6, -3.3]) {
      box(r, D, 0.03, 1.3, 0.02, s * 1.505, 5.0, z - 0.62, { r: 0.004 });
      louvres(r, D, s * 1.5, 5.05, z, 0.8, 0.9, s);
      box(r, S, 0.04, 0.05, 0.16, s * 1.53, 4.6, z + 0.35, { r: 0.008 });
    }
    // hinges on the door edges
    for (const z of [-6.5, -5.2, -3.9]) for (const yy of [4.55, 5.45]) box(r, D, 0.05, 0.12, 0.08, s * 1.52, yy, z + 0.06, { r: 0.01 });
  }
  // exhaust stacks and pre-cleaners on the hood
  for (const x of [-0.55, 0.55]) {
    cyl(r, K, 0.17, 1.0, x, 6.4, -5.35, { seg: 14 });
    cyl(r, K, 0.2, 0.2, x, 6.95, -5.35, { seg: 14, r2: 0.16 });
  }
  for (const x of [-0.9, 0.9]) {
    cyl(r, P, 0.28, 0.5, x, 6.15, -3.4, { seg: 18 });
    sphere(r, K, 0.24, x, 6.42, -3.4, { seg: 14, sy: 0.5 });
  }
  // engine-hood handrail
  rail(r, P, [v3(-1.25, 5.94, -6.2), v3(-1.25, 5.94, -2.9)], { h: 0.5, posts: 1.65, mid: false });
  // radiator module with the big rear grille, counterweight below
  box(r, P, 3.6, 4.3, 1.05, 0, 3.65, -6.95, { r: 0.12, seg: 2 });
  grille(r, P, 3.2, 3.6, 0, 3.7, -7.58, Math.PI, 0.15);
  box(r, P, 3.8, 1.05, 1.15, 0, 1.25, -7.15, { r: 0.18, seg: 2 });
  for (const s of [-1, 1]) {
    lamp(r, D, lens.red, s * 1.5, 1.45, -7.73, { w: 0.3, h: 0.16, yaw: Math.PI });
    lamp(r, D, lens.white, s * 1.45, 5.65, -7.5, { w: 0.34, h: 0.22, yaw: Math.PI, pitch: 0.2 });
    lamp(r, D, lens.amber, s * 1.1, 1.45, -7.73, { w: 0.18, h: 0.16, yaw: Math.PI });
  }
  // rear access stairway on the left, landing onto the walkway
  stairs(r, P, GR, P, v3(3.68, 0.5, -0.75), v3(3.68, 4.3, -4.95), 0.72);
  ladder(r, P, v3(3.68, 0.05, -0.5), v3(3.68, 0.5, -0.7), 0.6, 0.22);
  deck(r, GR, 0.75, 0.9, 3.66, 4.3, -5.35, 0.1);
  // cab platform and cab over the hitch
  deck(r, GR, 6.4, 3.0, 0, 4.3, -1.35, 0.12);
  box(r, P, 6.4, 0.35, 0.16, 0, 4.12, 0.12, { r: 0.04 });
  const cb = cab(r, P, 2.3, 2.6, 2.3, 0, 4.3, -1.5, { door: 1 });
  beacon(cb, K, lens.amber, 0.75, 2.68, -0.8);
  box(cb, P, 1.2, 0.25, 0.9, -0.3, 2.78, -0.6, { r: 0.05 }); // aircon pod
  for (const s of [-1, 1]) {
    lamp(cb, D, lens.white, s * 0.85, 2.72, 1.12, { w: 0.26, h: 0.16, pitch: 0.18 });
    lamp(cb, D, lens.white, s * 0.4, 2.72, 1.12, { w: 0.26, h: 0.16, pitch: 0.18 });
    lamp(cb, D, lens.white, s * 0.8, 2.72, -1.12, { w: 0.26, h: 0.16, yaw: Math.PI, pitch: 0.2 });
  }
  mirror(r, K, C, v3(1.2, 6.2, -0.45), v3(1.75, 6.25, -0.1), 0.42, 0.35);
  mirror(r, K, C, v3(-1.2, 6.2, -0.45), v3(-1.75, 6.25, -0.1), 0.42, -0.35);
  bottle(r, mats.red(), S, -1.6, 4.3, -2.55);
  bottle(r, mats.red(), S, -1.85, 4.3, -2.55);
  // handrails: platform front and the walkway outer edges
  rail(r, P, [v3(-3.15, 4.3, -7.0), v3(-3.15, 4.3, 0.05), v3(-1.3, 4.3, 0.05)]);
  rail(r, P, [v3(1.3, 4.3, 0.05), v3(3.15, 4.3, 0.05), v3(3.15, 4.3, -4.85)]);
  rail(r, P, [v3(4.0, 4.3, -4.9), v3(4.0, 4.3, -5.8), v3(3.15, 4.3, -5.8), v3(3.15, 4.3, -7.0), v3(1.6, 4.3, -7.0)]);
  rail(r, P, [v3(-1.6, 4.3, -7.0), v3(-3.15, 4.3, -7.0)]);
  // hydraulic tank and battery box on the right
  box(r, P, 1.1, 1.0, 1.6, -2.4, 4.8, -5.2, { r: 0.1 });
  box(r, D, 0.7, 0.6, 1.0, -2.0, 2.6, -1.0, { r: 0.04 });
  const rear = bakeFree(r, { mudTop: 3.0, mudBottom: 0.3 });

  // ── front frame: plates, tower with the arm pivots, front axle, fenders, lights ──
  const f = new THREE.Group();
  for (const s of [-1, 1]) {
    prism(f, P, [[0.25, 1.05], [4.8, 1.05], [4.9, 2.35], [3.2, 2.95], [0.25, 3.0]], 0.3, "zy", s * 0.72, { bevel: 0.03 });
    prism(f, P, [[0.5, 1.4], [2.9, 1.4], [2.9, 3.0], [2.15, 4.75], [1.15, 4.75], [0.5, 3.2]], 0.26, "zy", s * 1.08, { bevel: 0.03 });
    cyl(f, P, 0.28, 0.2, s * 1.2, WL.arm.y, WL.arm.z, { axis: "x", seg: 16 });
    // lift-ram base lugs
    cyl(f, P, 0.24, 0.5, s * WL.liftF.x, WL.liftF.y, WL.liftF.z, { axis: "x", seg: 14 });
    fender(f, P, 2.05, 0.35, Math.PI - 0.55, s * tx, 1.6, ay, WL.frontZ);
    box(f, D, 1.25, 0.1, 0.34, s * tx, ay + 1.99, WL.frontZ + 0.9, { r: 0.02 });
    lamp(f, D, lens.white, s * (tx + 0.35), ay + 2.15, WL.frontZ + 0.95, { w: 0.36, h: 0.22, pitch: 0.1 });
    lamp(f, D, lens.white, s * (tx - 0.35), ay + 2.15, WL.frontZ + 0.95, { w: 0.36, h: 0.22, pitch: 0.1 });
    lamp(f, D, lens.amber, s * (tx + 0.75), ay + 2.0, WL.frontZ + 0.85, { w: 0.16, h: 0.12 });
  }
  cyl(f, S, 0.14, 3.3, 0, WL.arm.y, WL.arm.z, { axis: "x", seg: 10 }); // arm pivot pin
  for (const yy of [1.1, 3.1]) box(f, P, 1.0, 0.28, 0.75, 0, yy, 0.25, { r: 0.05 });
  box(f, P, 1.9, 0.6, 1.0, 0, 1.35, 4.5, { r: 0.08 });
  box(f, P, 0.8, 0.5, 0.6, 0, WL.tiltF.y - 0.15, WL.tiltF.z, { r: 0.05 }); // tilt-ram base
  cyl(f, P, 0.55, 3.4, 0, ay, WL.frontZ, { axis: "x", seg: 22 });
  sphere(f, P, 0.78, 0, ay, WL.frontZ, { seg: 18 });
  const front = bakeFree(f, { mudTop: 3.2, mudBottom: 0.3 });

  // ── lift arms with the cross tube (arm frame: origin at the pivot, +z along the arm) ──
  const a = new THREE.Group();
  const L = WL.arm.len;
  for (const s of [-1, 1]) {
    prism(a, P, [[-0.42, 0.42], [L * 0.3, 0.6], [L * 0.62, 0.52], [L + 0.3, 0.34], [L + 0.38, 0], [L + 0.3, -0.34], [L * 0.62, -0.46], [L * 0.3, -0.52], [-0.42, -0.42]], 0.44, "zy", s * WL.arm.x, { bevel: 0.04 });
    cyl(a, P, 0.36, 0.52, s * WL.arm.x, 0, 0, { axis: "x", seg: 18 });
    cyl(a, P, 0.33, 0.52, s * WL.arm.x, 0, L, { axis: "x", seg: 18 });
    // lift-ram lugs under the arm
    const lz = WL.liftOnArm.s;
    box(a, P, 0.32, 0.6, 0.6, s * (WL.arm.x - 0.38), WL.liftOnArm.off + 0.15, lz, { r: 0.04 });
    cyl(a, S, 0.1, 0.7, s * (WL.arm.x - 0.3), WL.liftOnArm.off, lz, { axis: "x", seg: 10 });
  }
  // cross tube carrying the bellcrank
  cyl(a, P, 0.34, WL.arm.x * 2, 0, WL.bell.off, WL.bell.s, { axis: "x", seg: 18 });
  for (const s of [-1, 1]) box(a, P, 0.3, 0.8, 0.9, s * 0.3, WL.bell.off * 0.5, WL.bell.s, { r: 0.05 });
  const arms = bakeFree(a, { mudTop: 0.2, mudBottom: -3.5, mud: 0.6 });

  // ── bucket (origin at the hinge pins, +z toward the lip) ──
  const bk = new THREE.Group();
  const BZ = 0.5; // shell offset ahead of the hinge pins
  const prof: [number, number][] = (
    [
      [2.95, -0.62],
      [2.0, -0.7],
      [1.0, -0.74],
      [0.25, -0.66],
      [-0.25, -0.26],
      [-0.42, 0.3],
      [-0.38, 0.9],
      [-0.12, 1.42],
      [0.35, 1.72],
      [1.15, 1.84],
    ] as [number, number][]
  ).map(([z, y]) => [z + BZ, y]);
  const W = 3.3;
  for (let i = 0; i < prof.length - 1; i++) {
    const [z0, y0] = prof[i]!;
    const [z1, y1] = prof[i + 1]!;
    const nz = -(y1 - y0);
    const ny = z1 - z0;
    const nl = Math.hypot(nz, ny);
    const o = 0.07 / nl;
    plate(bk, P, [v3(-W, y0 + ny * o, z0 + nz * o), v3(W, y0 + ny * o, z0 + nz * o), v3(W, y1 + ny * o, z1 + nz * o), v3(-W, y1 + ny * o, z1 + nz * o)], 0.07, {
      out: v3(0, ny, nz),
    });
    const q = -0.012 / nl;
    plate(bk, S, [v3(-W + 0.06, y0 + ny * q, z0 + nz * q), v3(W - 0.06, y0 + ny * q, z0 + nz * q), v3(W - 0.06, y1 + ny * q, z1 + nz * q), v3(-W + 0.06, y1 + ny * q, z1 + nz * q)], 0.03, {
      out: v3(0, -ny, -nz),
      wear: { mud: 0.6 },
    });
  }
  // side plates and side cutters
  const outline = prof.map(([z, y]) => [z, y] as [number, number]);
  for (const s of [-1, 1]) {
    prism(bk, P, [...outline.map(([z, y]) => [z + (z > 2 + BZ ? 0.08 : -0.06), y + (y < 0 ? -0.08 : 0.08)] as [number, number])], 0.1, "zy", s * (W + 0.05), { bevel: 0.02 });
    beam(bk, D, v3(s * (W + 0.1), 1.86, 1.15 + BZ), v3(s * (W + 0.1), -0.6, 3.0 + BZ), 0.12, 0.22, v3(1, 0, 0), 0.02);
    box(bk, D, 0.14, 0.25, 0.7, s * (W + 0.08), -0.6, 2.65 + BZ, { r: 0.03 });
  }
  // cutting edge, teeth with adapters, lip shrouds
  box(bk, D, W * 2 + 0.1, 0.14, 0.55, 0, -0.62, 2.82 + BZ, { r: 0.04 });
  const nT = 9;
  for (let i = 0; i < nT; i++) {
    const x = lerp(-W + 0.35, W - 0.35, i / (nT - 1));
    const t = node(bk, x, -0.6, 3.0 + BZ);
    prism(t, D, [[-0.25, -0.1], [0.25, -0.08], [0.55, -0.03], [0.62, 0], [0.55, 0.04], [0.25, 0.1], [-0.25, 0.12]], 0.22, "zy", 0, { bevel: 0.02 });
    prism(t, S, [[0.45, -0.05], [0.78, -0.01], [0.8, 0.01], [0.62, 0.05], [0.45, 0.06]], 0.18, "zy", 0, { bevel: 0.015 });
    if (i < nT - 1) box(bk, D, 0.34, 0.12, 0.3, x + (W * 2 - 0.7) / (nT - 1) / 2, -0.55, 3.08 + BZ, { r: 0.03 });
  }
  // back: hinge bosses for the arms, link lugs, stiffeners, top spill guard
  for (const s of [-1, 1]) {
    for (const dx of [-0.32, 0.32]) prism(bk, P, [[-0.5, -0.35], [0.3, -0.55], [0.05, 0.3], [-0.38, 0.5]], 0.09, "zy", s * WL.arm.x + dx, { bevel: 0.01 });
    cyl(bk, S, 0.12, 0.9, s * WL.arm.x, 0, 0, { axis: "x", seg: 10 });
    // wear bars under the floor and a back rib behind each hinge
    for (const x of [0.9, 1.9, 2.8]) beam(bk, D, v3(s * x, -0.82, 0.85 + BZ * 0.4), v3(s * x, -0.79, 2.6 + BZ), 0.12, 0.1, Y, 0.02);
    beam(bk, P, v3(s * (WL.arm.x + 0.5), -0.35, -0.05), v3(s * (WL.arm.x + 0.5), 1.0, -0.15), 0.16, 0.2, v3(1, 0, 0), 0.02);
  }
  for (const dx of [-0.24, 0.24]) prism(bk, P, [[-0.48, 0.35], [0.03, 0.3], [0.05, 1.1], [-0.42, 1.0]], 0.08, "zy", dx, { bevel: 0.01 });
  cyl(bk, S, 0.1, 0.6, 0, WL.linkPin.y, WL.linkPin.z, { axis: "x", seg: 10 });
  beam(bk, P, v3(-W, 1.9, 1.15 + BZ), v3(W, 1.9, 1.15 + BZ), 0.14, 0.2, Y, 0.02);
  const bucket = bakeFree(bk, { mudTop: 0.4, mudBottom: -0.8, mud: 0.8 }, new THREE.Matrix4().makeTranslation(0, 0.8, 0));

  // ── bellcrank (pivot at origin, lower end toward +z at r1, upper end at -r2) ──
  const bc = new THREE.Group();
  for (const dx of [-0.2, 0.2]) prism(bc, P, [[-WL.bell.r2 - 0.25, -0.2], [WL.bell.r1 + 0.22, -0.16], [WL.bell.r1 + 0.22, 0.16], [-WL.bell.r2 - 0.25, 0.22]], 0.12, "zy", dx, { bevel: 0.02 });
  cyl(bc, S, 0.13, 0.62, 0, 0, 0, { axis: "x", seg: 12 });
  cyl(bc, P, 0.2, 0.52, 0, 0, WL.bell.r1, { axis: "x", seg: 12 });
  cyl(bc, P, 0.2, 0.52, 0, 0, -WL.bell.r2, { axis: "x", seg: 12 });
  const bell = bakeFree(bc, { mudTop: 0, mudBottom: -2, mud: 0.3 });
  // ── link (along +y, length Lk) ──
  const lk = new THREE.Group();
  for (const dx of [-0.22, 0.22]) box(lk, P, 0.1, WL.bell.link + 0.3, 0.32, dx, WL.bell.link / 2, 0, { r: 0.04 });
  const link = bakeFree(lk, { mudTop: 0, mudBottom: -2, mud: 0.3 });

  // ── wheels (both front, both rear: they spin together on each axle) ──
  const spec: TyreSpec = WL.tyre;
  const wheels = wheelSet(spec, [
    { x: tx, flip: false, dish: 0.22 },
    { x: -tx, flip: true, dish: 0.22 },
  ]);

  // ── rams sized from the kinematics ──
  const tiltR = range((i, j) => {
    const p = loaderPose(i, j);
    return Math.hypot(p.U.z - WL.tiltF.z, p.U.y - WL.tiltF.y);
  });
  const liftR = range((i) => {
    const p = loaderPose(i, 0.5);
    return Math.hypot(p.liftPin.z - WL.liftF.z, p.liftPin.y - WL.liftF.y);
  });
  const tilt = ramTemplate(colour, 0.24, tiltR[0], tiltR[1]);
  const liftRam = ramTemplate(colour, 0.26, liftR[0], liftR[1]);
  return { rear, front, arms, bucket, bell, link, wheels, tilt, liftRam, lens };
}

/** A large articulated mining wheel loader (≈19 m³ rock bucket, 3.8 m tyres). */
export function makeWheelLoader(opts: { colour?: string } = {}): WheelLoader {
  const colour = opts.colour ?? COLOURS.yellow;
  let tpl = loaderTpls.get(colour);
  if (!tpl) {
    tpl = buildLoader(colour);
    loaderTpls.set(colour, tpl);
  }
  const lens = lenses();
  const group = new THREE.Group();
  group.name = "wheel-loader";
  const rear = instance(tpl.rear);
  swapLenses(rear, tpl.lens, lens);
  group.add(rear);
  const rearWheels = instance(tpl.wheels);
  rearWheels.position.set(0, WL.axleY, WL.rearZ);
  group.add(rearWheels);
  const front = new THREE.Group();
  group.add(front);
  const fbody = instance(tpl.front);
  swapLenses(fbody, tpl.lens, lens);
  front.add(fbody);
  const frontWheels = instance(tpl.wheels);
  frontWheels.position.set(0, WL.axleY, WL.frontZ);
  front.add(frontWheels);
  const arms = instance(tpl.arms);
  arms.position.set(0, WL.arm.y, WL.arm.z);
  front.add(arms);
  const bucket = instance(tpl.bucket);
  front.add(bucket);
  const bell = instance(tpl.bell);
  front.add(bell);
  const link = instance(tpl.link);
  front.add(link);
  const tiltRam = makeRam(front, tpl.tilt);
  const liftRams = [-1, 1].map((s) => ({ s, ram: makeRam(front, tpl.liftRam) }));
  const state: WheelLoaderState = { lift: 0, tilt: 0.5, steer: 0, speed: 0, distance: 0, lights: false };
  const a = new THREE.Vector3();
  const b = new THREE.Vector3();
  const apply = () => {
    const p = loaderPose(state.lift, state.tilt);
    arms.rotation.x = -p.th;
    bucket.position.set(0, p.pin.y, p.pin.z);
    bucket.rotation.x = -p.pitch;
    bell.position.set(0, p.P.y, p.P.z);
    bell.rotation.x = -p.beta;
    link.position.set(0, p.E.y, p.E.z);
    b.set(0, p.B.y - p.E.y, p.B.z - p.E.z).normalize();
    link.quaternion.setFromUnitVectors(Y, b);
    tiltRam.set(a.set(0, WL.tiltF.y, WL.tiltF.z), b.set(0, p.U.y, p.U.z));
    for (const { s, ram } of liftRams) ram.set(a.set(s * WL.liftF.x, WL.liftF.y, WL.liftF.z), b.set(s * WL.liftF.x, p.liftPin.y, p.liftPin.z));
    front.rotation.y = state.steer;
    const roll = state.distance / (WL.tyre.od / 2);
    frontWheels.rotation.x = roll;
    rearWheels.rotation.x = roll;
    lens.set(state.lights);
  };
  apply();
  return {
    group,
    state,
    lenses: lens,
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
}

// ───────────────────────────── crawler dozer ─────────────────────────────

export const DOZER = {
  /** Half the track gauge (track centre to machine centre). */
  gauge: 1.45,
  trackZ: -0.3,
  track: { length: 5.0, sprocketR: 0.86, idlerR: 0.8, shoeW: 0.76, pitch: 0.32, rollers: 8, rollerR: 0.2, carriers: 2 },
  /** Push-arm trunnions on the track roller frames (the blade's pivot). */
  trunnion: { x: 2.02, y: 0.95, z: -0.6 },
  edgeZ: 4.55,
  liftMax: 0.3,
  liftF: { x: 1.25, y: 3.35, z: 1.75 },
  liftB: { x: 1.25, y: 2.3, z: 3.62 },
  ripF: { x: 0.55, y: 3.1, z: -3.45 },
  ripper: { up: { y: 2.15, z: -3.5 }, dy: 1.0, link: 1.7, hi: 32 * D2R, lo: -30 * D2R },
} as const;

export interface DozerState {
  /** Blade lift 0 (on the ground) .. 1 (raised ≈1.5 m). */
  blade: number;
  /** Ripper 0 raised .. 1 shank in the ground. */
  ripper: number;
  speed: number;
  distance: number;
  lights: boolean;
}
export interface Dozer {
  group: THREE.Group;
  state: DozerState;
  set(s: Partial<DozerState>): void;
  update(dt: number): void;
  lenses: Lenses;
}

interface DozerTpl {
  body: THREE.Group;
  blade: THREE.Group;
  carriage: THREE.Group;
  upperLinks: THREE.Group;
  lowerLinks: THREE.Group;
  liftRam: RamTpl;
  ripRam: RamTpl;
  lens: Lenses;
}
const dozerTpls = new Map<string, DozerTpl>();

const bladeAngle = (t: number) => clamp01(t) * DOZER.liftMax;
const rotAbout = (y: number, z: number, cy: number, cz: number, ang: number): P2 => {
  // rotate (y, z) about (cy, cz) so that +z goes toward +y by ang (front up)
  const dy = y - cy;
  const dz = z - cz;
  const c = Math.cos(ang);
  const s = Math.sin(ang);
  return { y: cy + dy * c + dz * s, z: cz + dz * c - dy * s };
};
const ripAngle = (t: number) => lerp(DOZER.ripper.hi, DOZER.ripper.lo, clamp01(t));

// ── small geometry builders for curved plate, tapered hoods and mud ──

/** Triangle collector with explicit normals; every triangle is wound to face `want`. */
function soup() {
  const pos: number[] = [];
  const nrm: number[] = [];
  const e1 = new THREE.Vector3();
  const e2 = new THREE.Vector3();
  const fn = new THREE.Vector3();
  const tri = (a: THREE.Vector3, b: THREE.Vector3, c: THREE.Vector3, na: THREE.Vector3, nb: THREE.Vector3, nc: THREE.Vector3, want: THREE.Vector3) => {
    fn.crossVectors(e1.subVectors(b, a), e2.subVectors(c, a));
    const list: [THREE.Vector3, THREE.Vector3][] =
      fn.dot(want) < 0
        ? [
            [a, na],
            [c, nc],
            [b, nb],
          ]
        : [
            [a, na],
            [b, nb],
            [c, nc],
          ];
    for (const [v, n] of list) {
      pos.push(v.x, v.y, v.z);
      nrm.push(n.x, n.y, n.z);
    }
  };
  return {
    /** A convex polygon; flat-shaded unless per-vertex normals are given. */
    face(v: THREE.Vector3[], want: THREE.Vector3, ns?: THREE.Vector3[]) {
      let flat: THREE.Vector3 | undefined;
      if (!ns) {
        flat = new THREE.Vector3();
        for (let i = 0; i < v.length; i++) {
          const a = v[i]!;
          const b = v[(i + 1) % v.length]!;
          flat.x += (a.y - b.y) * (a.z + b.z);
          flat.y += (a.z - b.z) * (a.x + b.x);
          flat.z += (a.x - b.x) * (a.y + b.y);
        }
        flat.normalize();
        if (flat.dot(want) < 0) flat.negate();
      }
      for (let i = 1; i < v.length - 1; i++) tri(v[0]!, v[i]!, v[i + 1]!, flat ?? ns![0]!, flat ?? ns![i]!, flat ?? ns![i + 1]!, want);
    },
    mesh(mat: THREE.Material) {
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
      g.setAttribute("normal", new THREE.Float32BufferAttribute(nrm, 3));
      return new THREE.Mesh(g, mat);
    },
  };
}

/**
 * A plate bent along a (z, y) profile (listed bottom to top, front face on the profile, thickness
 * behind it), spanning x from xa to xb. oa/ob push the profile forward at each end (blade wings).
 * Shading is smooth along the bends.
 */
function bentPlate(parent: THREE.Object3D, mat: THREE.Material, prof: [number, number][], xa: number, xb: number, t: number, oa = 0, ob = 0, wear?: PartWear) {
  const k = (ob - oa) / (xb - xa);
  const n = prof.length;
  const fr: THREE.Vector3[][] = [[], []];
  const bk: THREE.Vector3[][] = [[], []];
  const nn: THREE.Vector3[] = [];
  for (let i = 0; i < n; i++) {
    const p0 = prof[Math.max(0, i - 1)]!;
    const p1 = prof[Math.min(n - 1, i + 1)]!;
    const nv = new THREE.Vector3(-k * (p1[1] - p0[1]), -(p1[0] - p0[0]), p1[1] - p0[1]).normalize();
    nn.push(nv);
    for (const e of [0, 1]) {
      const f = new THREE.Vector3(e ? xb : xa, prof[i]![1], prof[i]![0] + (e ? ob : oa));
      fr[e]!.push(f);
      bk[e]!.push(f.clone().addScaledVector(nv, -t));
    }
  }
  const [F0, F1] = fr as [THREE.Vector3[], THREE.Vector3[]];
  const [B0, B1] = bk as [THREE.Vector3[], THREE.Vector3[]];
  const s = soup();
  for (let i = 0; i < n - 1; i++) {
    const na = nn[i]!;
    const nb = nn[i + 1]!;
    s.face([F0[i]!, F1[i]!, F1[i + 1]!, F0[i + 1]!], na.clone().add(nb), [na, na, nb, nb]);
    const ra = na.clone().negate();
    const rb = nb.clone().negate();
    s.face([B0[i]!, B1[i]!, B1[i + 1]!, B0[i + 1]!], ra.clone().add(rb), [ra, ra, rb, rb]);
    s.face([F0[i]!, F0[i + 1]!, B0[i + 1]!, B0[i]!], new THREE.Vector3(-1, 0, -k));
    s.face([F1[i]!, F1[i + 1]!, B1[i + 1]!, B1[i]!], new THREE.Vector3(1, 0, k));
  }
  const tan = (i: number, j: number) => new THREE.Vector3(0, prof[j]![1] - prof[i]![1], prof[j]![0] - prof[i]![0]);
  s.face([F0[0]!, F1[0]!, B1[0]!, B0[0]!], tan(1, 0));
  s.face([F0[n - 1]!, F1[n - 1]!, B1[n - 1]!, B0[n - 1]!], tan(n - 2, n - 1));
  const m = s.mesh(mat);
  if (wear) m.userData.wear = wear;
  parent.add(m);
  return m;
}

/** A closed, flat-shaded loft through convex (x, y) sections stacked along z. */
function loft(parent: THREE.Object3D, mat: THREE.Material, secs: { z: number; pts: [number, number][] }[]) {
  const s = soup();
  const V = secs.map(({ z, pts }) => pts.map(([x, y]) => new THREE.Vector3(x, y, z)));
  const centre = (vs: THREE.Vector3[]) => vs.reduce((acc, v) => acc.add(v), new THREE.Vector3()).multiplyScalar(1 / vs.length);
  for (let k = 0; k < V.length - 1; k++) {
    const a = V[k]!;
    const b = V[k + 1]!;
    const c = centre(a).add(centre(b)).multiplyScalar(0.5);
    for (let j = 0; j < a.length; j++) {
      const j1 = (j + 1) % a.length;
      const q = [a[j]!, a[j1]!, b[j1]!, b[j]!];
      const mid = centre(q);
      s.face(q, mid.sub(c).setZ(0));
    }
  }
  s.face(V[0]!, new THREE.Vector3(0, 0, -1));
  s.face(V[V.length - 1]!, new THREE.Vector3(0, 0, 1));
  const m = s.mesh(mat);
  parent.add(m);
  return m;
}

let lumpGeo: THREE.BufferGeometry | undefined;
/** A lumpy clod: an icosphere with a position-hashed radial jitter (so shared corners stay closed). */
function lumpGeometry() {
  if (lumpGeo) return lumpGeo;
  const g = new THREE.IcosahedronGeometry(1, 1);
  const p = g.getAttribute("position");
  for (let i = 0; i < p.count; i++) {
    const x = p.getX(i);
    const y = p.getY(i);
    const z = p.getZ(i);
    const h = Math.sin(x * 12.9898 + y * 78.233 + z * 37.719) * 43758.5453;
    const f = 0.76 + (h - Math.floor(h)) * 0.42;
    p.setXYZ(i, x * f, y * f, z * f);
  }
  g.computeVertexNormals();
  lumpGeo = g;
  return g;
}
/** Dried mud packed onto a part: a flattened clod of size s. */
function clod(parent: THREE.Object3D, mat: THREE.Material, r: () => number, x: number, y: number, z: number, s: number, flat = 0.55) {
  const m = new THREE.Mesh(lumpGeometry(), mat);
  m.position.set(x, y, z);
  m.scale.set(s * (0.8 + r() * 0.6), s * flat * (0.8 + r() * 0.4), s * (0.8 + r() * 0.6));
  m.rotation.set((r() - 0.5) * 0.5, r() * Math.PI * 2, (r() - 0.5) * 0.5);
  parent.add(m);
}
const mudMat = () => paint("#5c4a37", { rough: 0.97, coat: 0, variation: 1, mud: "#4e3e2d" });
const underSteel = () => paint("#34332f", { metal: 0.55, rough: 0.6, coat: 0, mudAmount: 1 });

/** U-blade moldboard section (z back, y up) from the cutting-edge bottom: 0.9 m flat, then a 1.6 m arc. */
function bladeProfile(): [number, number][] {
  const pts: [number, number][] = [
    [0, 0],
    [-0.173, 0.3],
    [-0.45, 0.78],
  ];
  const R = 1.6;
  const a0 = 60 * D2R;
  const a1 = 135 * D2R;
  const cz = -0.45 + R * Math.sin(a0);
  const cy = 0.78 + R * Math.cos(a0);
  for (let i = 1; i <= 14; i++) {
    const a = lerp(a0, a1, i / 14);
    pts.push([cz - R * Math.sin(a), cy - R * Math.cos(a)]);
  }
  return pts;
}

function buildDozer(colour: string): DozerTpl {
  const P = paint(colour);
  const D = mats.dark();
  const K = mats.black();
  const S = mats.steel();
  const C = mats.chrome();
  const GR = mats.grate();
  const U = underSteel();
  const M = mudMat();
  const lens = lenses();
  const g = DOZER.gauge;
  const tz = DOZER.trackZ;
  const TR = DOZER.track;
  const T = DOZER.trunnion;
  const r = rng(23);
  const ay = Math.max(TR.sprocketR, TR.idlerR) + TR.pitch * 0.22; // track axle height (kit default)
  const spZ = tz - TR.length / 2;
  const idZ = tz + TR.length / 2;

  // ── main case, final drives, roller-frame guards, trunnions, equaliser bar ──
  const h = new THREE.Group();
  box(h, P, 1.9, 1.35, 5.6, 0, 1.28, -0.5, { r: 0.08 });
  box(h, U, 1.7, 0.14, 4.4, 0, 0.56, 0.0, { r: 0.03 }); // belly guard
  cyl(h, U, 0.16, 2.6, 0, ay, 1.45, { axis: "x", seg: 14 }); // equaliser bar
  for (const s of [-1, 1]) {
    const xc = s * g;
    cyl(h, U, 0.56, 0.34, s * 1.1, ay, spZ, { axis: "x", seg: 22 }); // final drive
    cyl(h, D, 0.3, 0.08, s * 1.28, ay, spZ, { axis: "x", seg: 16 });
    cyl(h, U, 0.22, 0.32, s * 1.1, ay, -1.75, { axis: "x", seg: 14 }); // pivot shaft housing
    // track roller-frame: outer guard plate between the wheels, top cover, idler recoil housing
    prism(h, U, [[-2.3, 0.3], [1.55, 0.3], [1.85, 0.62], [1.7, 0.86], [-2.15, 0.86], [-2.45, 0.6]], 0.07, "zy", xc + s * 0.3, { bevel: 0.015 });
    for (const zz of [-1.6, -0.6, 0.4, 1.3]) box(h, U, 0.1, 0.36, 0.12, xc + s * 0.33, 0.56, zz, { r: 0.02 }); // stiffeners
    box(h, U, 0.5, 0.09, 3.9, xc, 1.38, tz + 0.05, { r: 0.02 });
    box(h, U, 0.46, 0.5, 0.75, xc, ay + 0.05, idZ - 0.85, { r: 0.05 });
    // push-arm trunnion stub and ball
    cyl(h, U, 0.17, 0.5, s * (T.x - 0.17), T.y, T.z, { axis: "x", seg: 14 });
    sphere(h, U, 0.24, s * (T.x - 0.02), T.y, T.z, { seg: 14 });
    // mud packed along the roller frame, on the guard ledge and round the final drive
    for (let i = 0; i < 9; i++) clod(h, M, r, xc + s * (0.22 + r() * 0.1), 0.86 + r() * 0.04, lerp(-2.0, 1.5, (i + r() * 0.6) / 9), 0.16 + r() * 0.08);
    for (let i = 0; i < 7; i++) clod(h, M, r, xc + s * (0.12 + r() * 0.1), 1.42, lerp(-1.9, 1.7, (i + r()) / 7), 0.15 + r() * 0.07, 0.35);
    for (let i = 0; i < 3; i++) clod(h, M, r, s * (0.92 + r() * 0.06), 0.64 + r() * 0.08, spZ + 0.8 + r() * 0.8, 0.16, 0.7);
    clod(h, M, r, s * 1.1, ay + 0.12, 1.45, 0.17);
  }

  // ── fenders sitting on the tracks, steps and rails ──
  for (const s of [-1, 1]) {
    const xc = s * g;
    deck(h, GR, 0.98, 5.5, xc, 2.02, tz - 0.15, 0.1);
    box(h, P, 0.1, 0.3, 5.5, xc + s * 0.5, 1.93, tz - 0.15, { r: 0.025 });
    box(h, P, 0.98, 0.3, 0.1, xc, 1.88, tz - 2.9, { r: 0.025 });
    prism(h, P, [[2.55, 1.98], [2.25, 1.98], [2.25, 1.86], [2.62, 1.86]], 0.98, "zy", xc, { bevel: 0.015 }); // front lip
    rail(h, P, s > 0 ? [v3(xc + s * 0.45, 2.02, -1.85), v3(xc + s * 0.45, 2.02, -0.2)] : [v3(xc + s * 0.45, 2.02, -2.85), v3(xc + s * 0.45, 2.02, -0.2)], { h: 1.0, posts: 1.0 });
  }
  // two-step ladder up the left track fender, behind the push arm
  for (const [yy, zz] of [
    [1.42, -2.45],
    [0.88, -2.55],
  ] as const) {
    box(h, GR, 0.36, 0.05, 0.42, g + 0.6, yy, zz, { r: 0.01 });
    for (const dz of [-0.2, 0.2]) bar(h, P, v3(g + 0.6, yy, zz + dz), v3(g + 0.48, 1.95, zz + dz * 0.6), 0.022, 6);
  }
  bar(h, P, v3(g + 0.52, 2.1, -2.2), v3(g + 0.52, 3.0, -2.2), 0.025, 6); // grab handle

  // ── engine hood: tapers toward the front, chamfered top, hinged side doors ──
  const hz0 = -0.15;
  const hz1 = 2.32;
  const hood = (w: number, top: number, c: number): [number, number][] => [
    [-w / 2, 1.95],
    [w / 2, 1.95],
    [w / 2, top - c],
    [w / 2 - c, top],
    [-w / 2 + c, top],
    [-w / 2, top - c],
  ];
  loft(h, P, [
    { z: hz0, pts: hood(1.86, 3.36, 0.22) },
    { z: hz1, pts: hood(1.62, 3.12, 0.2) },
  ]);
  const hw = (z: number) => lerp(0.93, 0.81, (z - hz0) / (hz1 - hz0));
  const htop = (z: number) => lerp(3.36, 3.12, (z - hz0) / (hz1 - hz0));
  const taper = Math.atan2(0.12, hz1 - hz0);
  for (const s of [-1, 1]) {
    for (const [z0, z1] of [
      [0.0, 1.05],
      [1.12, 2.2],
    ] as const) {
      const zm = (z0 + z1) / 2;
      const side = node(h, s * (hw(zm) + 0.004), 0, zm, { ry: -s * taper });
      box(side, D, 0.02, 0.8, 0.02, 0, 2.48, -(z1 - z0) / 2, { r: 0.004 }); // door seams
      box(side, D, 0.02, 0.8, 0.02, 0, 2.48, (z1 - z0) / 2, { r: 0.004 });
      box(side, D, 0.02, 0.02, z1 - z0, 0, 2.08, 0, { r: 0.004 });
      box(side, D, 0.02, 0.02, z1 - z0, 0, 2.88, 0, { r: 0.004 });
      louvres(side, D, 0, 2.62, 0, 0.42, (z1 - z0) * 0.7, s, 0.1);
      for (const yy of [2.25, 2.72]) cyl(side, S, 0.03, 0.14, s * 0.02, yy, (z1 - z0) / 2 - 0.06, { seg: 8 }); // hinges
      box(side, S, 0.05, 0.05, 0.16, s * 0.03, 2.45, -(z1 - z0) / 2 + 0.16, { r: 0.01 }); // latch
    }
    bar(h, P, v3(s * 0.66, htop(0.2) + 0.03, 0.2), v3(s * 0.62, htop(2.0) + 0.03, 2.0), 0.022, 6); // hood grab rail
  }
  box(h, D, 0.03, 0.02, hz1 - hz0, 0, htop((hz0 + hz1) / 2) + 0.012, (hz0 + hz1) / 2, { r: 0.004, rx: Math.atan2(0.24, hz1 - hz0) });
  // precleaner and exhaust on the hood
  cyl(h, K, 0.24, 0.12, 0.42, htop(1.5) + 0.04, 1.5, { seg: 16 });
  cyl(h, K, 0.13, 0.95, 0.42, htop(1.5) + 0.55, 1.5, { seg: 14 });
  cyl(h, K, 0.155, 0.14, 0.42, htop(1.5) + 1.07, 1.5, { seg: 14, r2: 0.12 });
  cyl(h, P, 0.21, 0.48, -0.42, htop(1.85) + 0.24, 1.85, { seg: 18 });
  cyl(h, D, 0.215, 0.06, -0.42, htop(1.85) + 0.32, 1.85, { seg: 18 });
  sphere(h, K, 0.2, -0.42, htop(1.85) + 0.5, 1.85, { seg: 14, sy: 0.55 });

  // ── radiator guard: heavy box, slotted cover, lift-ram brackets, lights, bumper ──
  box(h, P, 2.0, 2.55, 0.66, 0, 2.08, 2.62, { r: 0.1, seg: 2 });
  grille(h, P, 1.62, 1.8, 0, 2.3, 3.05, 0, 0.1);
  for (const yy of [1.1, 3.2]) box(h, P, 2.04, 0.12, 0.3, 0, yy, 3.0, { r: 0.03 }); // guard bars
  for (const s of [-1, 1]) {
    box(h, P, 0.12, 2.4, 0.3, s * 0.96, 2.1, 3.0, { r: 0.03 });
    // lift-ram brackets reaching back from the guard over the hood sides
    for (const dx of [-0.24, 0.24]) prism(h, P, [[2.4, 2.7], [2.4, 3.42], [1.5, 3.55], [1.42, 3.12]], 0.14, "zy", s * (DOZER.liftF.x + dx), { bevel: 0.02 });
    cyl(h, S, 0.09, 0.56, s * DOZER.liftF.x, DOZER.liftF.y, DOZER.liftF.z, { axis: "x", seg: 12 });
    lamp(h, D, lens.white, s * 0.62, 3.47, 2.78, { w: 0.32, h: 0.2, pitch: 0.15 });
    box(h, D, 0.16, 0.1, 0.16, s * 0.3, 3.4, 2.6, { r: 0.02 }); // lifting eye
  }
  box(h, U, 1.4, 0.36, 0.34, 0, 0.78, 3.1, { r: 0.05 }); // bumper / pull hook
  cyl(h, U, 0.12, 0.6, 0, 0.78, 3.3, { axis: "x", seg: 10 });

  // ── cab on a raised base behind the hood, ROPS frame, lights, mirrors ──
  box(h, P, 2.05, 0.32, 2.55, 0, 2.11, -1.4, { r: 0.05 });
  const cb = cab(h, P, 2.05, 2.25, 2.15, 0, 2.27, -1.4, { door: 1, glassTop: 0.22 });
  for (const s of [-1, 1])
    for (const zz of [-1.12, 1.12]) box(cb, P, 0.17, 2.7, 0.17, s * 1.13, 1.07, zz, { r: 0.03 }); // ROPS posts
  box(cb, P, 2.5, 0.2, 2.46, 0, 2.45, 0, { r: 0.05 });
  beacon(cb, K, lens.amber, -0.8, 2.55, -0.8);
  for (const s of [-1, 1]) {
    lamp(cb, D, lens.white, s * 0.7, 2.6, 1.15, { w: 0.24, h: 0.15, pitch: 0.2 });
    lamp(cb, D, lens.white, s * 0.7, 2.6, -1.15, { w: 0.24, h: 0.15, yaw: Math.PI, pitch: 0.3 });
  }
  mirror(h, K, C, v3(1.15, 3.95, -0.3), v3(1.45, 4.0, 0.0), 0.32, 0.4);
  mirror(h, K, C, v3(-1.15, 3.95, -0.3), v3(-1.45, 4.0, 0.0), 0.32, -0.4);
  bottle(h, mats.red(), S, -1.55, 2.02, -2.3, 0.6, 0.09);

  // ── fuel tank, tail lights, ripper mount ──
  box(h, P, 2.1, 1.05, 0.75, 0, 2.45, -3.0, { r: 0.12 });
  cyl(h, K, 0.1, 0.1, 0.6, 3.0, -3.0, { seg: 10 });
  for (const s of [-1, 1]) {
    lamp(h, D, lens.red, s * 0.85, 2.3, -3.385, { w: 0.22, h: 0.14, yaw: Math.PI });
    lamp(h, D, lens.white, s * 0.5, 2.62, -3.385, { w: 0.24, h: 0.16, yaw: Math.PI });
    prism(h, P, [[-3.1, 0.6], [-3.8, 0.6], [-3.8, 2.45], [-3.1, 2.62]], 0.22, "zy", s * 0.92, { bevel: 0.02 });
    for (const dx of [-0.24, 0.24]) prism(h, P, [[-3.1, 2.75], [-3.7, 2.8], [-3.65, 3.35], [-3.2, 3.38]], 0.16, "zy", s * (DOZER.ripF.x + dx), { bevel: 0.02 });
    cyl(h, S, 0.13, 0.4, s * 0.92, DOZER.ripper.up.y, DOZER.ripper.up.z, { axis: "x", seg: 12 });
    cyl(h, S, 0.13, 0.4, s * 0.92, DOZER.ripper.up.y - DOZER.ripper.dy, DOZER.ripper.up.z, { axis: "x", seg: 12 });
    cyl(h, S, 0.12, 0.5, s * DOZER.ripF.x, DOZER.ripF.y, DOZER.ripF.z, { axis: "x", seg: 12 });
  }
  const body = bakeFree(h, { mudTop: 1.9, mudBottom: 0.25, mud: 1.1 });

  // ── blade (origin at the trunnion axis): curved moldboard, wings, cutting edge, end bits, frame ──
  const bl = new THREE.Group();
  const EZ = DOZER.edgeZ - T.z;
  const EY = -T.y;
  const prof = bladeProfile().map(([z, y]) => [EZ + z, EY + y] as [number, number]);
  const top = prof[prof.length - 1]!;
  const W0 = 2.3;
  const W1 = 3.2;
  const WO = 0.42;
  const wing = (x: number) => Math.max(0, Math.abs(x) - W0) * (WO / (W1 - W0));
  const beta = Math.atan2(WO, W1 - W0);
  const panels: [number, number][] = [
    [-W1, -W0],
    [-W0, W0],
    [W0, W1],
  ];
  for (const [xa, xb] of panels) {
    bentPlate(bl, P, prof.slice(1), xa, xb, 0.06, wing(xa), wing(xb));
    bentPlate(bl, D, prof.slice(0, 2), xa, xb, 0.1, wing(xa), wing(xb));
    // heavy top edge bar and bottom box beam behind the plate
    beam(bl, P, v3(xa, top[1] - 0.09, top[0] - 0.3 + wing(xa)), v3(xb, top[1] - 0.09, top[0] - 0.3 + wing(xb)), 0.2, 0.18, Y, 0.03);
    beam(bl, P, v3(xa, EY + 0.45, EZ - 0.63 + wing(xa)), v3(xb, EY + 0.45, EZ - 0.63 + wing(xb)), 0.42, 0.42, Y, 0.04);
  }
  // ribs following the back of the moldboard
  const rib = (x: number, t: number) => {
    const o = wing(x);
    const front = prof.slice(1).map(([z, y]) => [z - 0.1 + o, y] as [number, number]);
    prism(bl, P, [...front, [EZ - 0.98 + o, top[1] - 0.12], [EZ - 0.98 + o, EY + 0.3]], t, "zy", x, { bevel: 0.015 });
  };
  for (const x of [-2.75, -2.25, -1.62, -0.6, 0.6, 1.62, 2.25, 2.75]) rib(x, 0.1);
  for (const s of [-1, 1]) {
    // end plates closing the wing tips
    const o = WO + 0.02;
    prism(bl, P, [...prof.map(([z, y]) => [z + o, y] as [number, number]), [EZ - 0.95 + o, top[1]], [EZ - 0.95 + o, EY + 0.12]], 0.08, "zy", s * (W1 + 0.04), { bevel: 0.015 });
    // end bits bolted to the wing corners, lying along the face
    const eb = node(bl, s * 2.76, EY, EZ + wing(2.76), { ry: -s * beta });
    box(eb, D, 0.92, 0.42, 0.13, 0, 0.18, -0.04, { rx: -30 * D2R, r: 0.025 });
    // push arm from the trunnion to the blade's lower back, with a gusset at the blade
    beam(bl, P, v3(s * T.x, 0, 0), v3(s * T.x, EY + 0.55, EZ - 0.9), 0.34, 0.5, Y, 0.05);
    cyl(bl, P, 0.32, 0.44, s * T.x, 0, 0, { axis: "x", seg: 18 });
    prism(bl, P, [[EZ - 1.6, EY + 0.25], [EZ - 0.65, EY + 0.25], [EZ - 0.65, EY + 1.3], [EZ - 1.15, EY + 0.85]], 0.3, "zy", s * T.x, { bevel: 0.02 });
    // lift-ram clevis on the blade back
    for (const dx of [-0.17, 0.17]) prism(bl, P, [[EZ - 1.15, EY + 2.0], [EZ - 0.74, EY + 1.95], [EZ - 0.62, EY + 2.7], [EZ - 1.0, EY + 2.6]], 0.08, "zy", s * DOZER.liftB.x + dx, { bevel: 0.01 });
    cyl(bl, S, 0.09, 0.5, s * DOZER.liftB.x, DOZER.liftB.y - T.y, DOZER.liftB.z - T.z, { axis: "x", seg: 10 });
    // mud packed on the push arm, the bottom beam and the lower face
    for (let i = 0; i < 4; i++) {
      const z = EZ - 1.2 - i * 0.7;
      clod(bl, M, r, s * (T.x + (r() - 0.5) * 0.15), (EY + 0.55) * (z / (EZ - 0.9)) + 0.24, z, 0.17);
    }
    for (let i = 0; i < 4; i++) {
      const x = s * lerp(0.4, 2.9, (i + r()) / 4);
      clod(bl, M, r, x, EY + 0.68, EZ - 0.62 + wing(x), 0.2, 0.5);
    }
    for (let i = 0; i < 6; i++) {
      const x = s * lerp(0.2, 3.0, (i + r() * 0.7) / 6);
      clod(bl, M, r, x, EY + 0.33 + r() * 0.12, EZ - 0.2 + wing(x) - 0.04, 0.16 + r() * 0.1, 0.5);
    }
  }
  // tilt cylinder (right) and tilt brace (left) from the push arms to the blade's upper back
  const tl0 = v3(-T.x + 0.06, 0.02, 3.0);
  const tl1 = v3(-1.82, 2.35 - T.y, 3.55 - T.z);
  bar(bl, P, tl0, tl0.clone().lerp(tl1, 0.56), 0.16, 14, undefined, false);
  bar(bl, C, tl0.clone().lerp(tl1, 0.5), tl1, 0.095, 12, undefined, false);
  bar(bl, P, v3(T.x - 0.06, 0.02, 3.0), v3(1.82, 2.35 - T.y, 3.55 - T.z), 0.12, 12, undefined, false);
  for (const s of [-1, 1]) {
    box(bl, P, 0.3, 0.26, 0.36, s * 1.85, 2.35 - T.y, 3.55 - T.z, { r: 0.03 });
    box(bl, P, 0.3, 0.2, 0.4, s * (T.x - 0.06), 0.1, 3.0, { r: 0.03 });
  }
  const blade = bakeFree(bl, { mudTop: 1.4, mudBottom: 0.0, mud: 1 }, new THREE.Matrix4().makeTranslation(0, T.y, T.z));

  // ── ripper: links (parallelogram) and carriage with the shank ──
  const R = DOZER.ripper;
  const ul = new THREE.Group();
  for (const s of [-1, 1]) box(ul, P, 0.22, 0.3, R.link + 0.35, s * 0.92, 0, -R.link / 2, { r: 0.05 });
  const upperLinks = bakeFree(ul, { mudTop: 0, mudBottom: -2, mud: 0.6 });
  const ll = new THREE.Group();
  for (const s of [-1, 1]) box(ll, P, 0.22, 0.3, R.link + 0.35, s * 0.92, 0, -R.link / 2, { r: 0.05 });
  const lowerLinks = bakeFree(ll, { mudTop: 0, mudBottom: -2, mud: 0.9 });
  const ca = new THREE.Group();
  for (const s of [-1, 1]) {
    // side plates joining the upper and lower link pins
    prism(ca, P, [[0.28, 0.28], [-0.4, 0.22], [-0.5, -R.dy - 0.2], [0.28, -R.dy - 0.26]], 0.12, "zy", s * 0.85, { bevel: 0.02 });
    cyl(ca, S, 0.12, 0.34, s * 0.92, 0, 0, { axis: "x", seg: 10 });
    cyl(ca, S, 0.12, 0.34, s * 0.92, -R.dy, 0, { axis: "x", seg: 10 });
  }
  box(ca, P, 1.6, 0.55, 0.6, 0, -0.45, -0.05, { r: 0.05 }); // cross beam
  box(ca, P, 0.6, 0.95, 0.72, 0, -0.38, 0.02, { r: 0.05 }); // shank pocket
  cyl(ca, S, 0.09, 0.75, 0, -0.6, 0.02, { axis: "x", seg: 10 }); // depth pin
  box(ca, P, 0.6, 0.45, 0.45, 0, 0.3, -0.25, { r: 0.04 }); // ram clevis
  // shank: deep curved bar, tip forward
  prism(
    ca,
    U,
    [[0.2, 0.55], [0.26, -0.6], [0.45, -1.78], [0.78, -2.42], [0.58, -2.52], [0.1, -1.92], [-0.18, -0.7], [-0.2, 0.55]],
    0.26,
    "zy",
    0,
    { bevel: 0.03 },
  );
  prism(ca, S, [[0.62, -2.33], [1.05, -2.58], [1.02, -2.68], [0.56, -2.54]], 0.2, "zy", 0, { bevel: 0.02 });
  for (const yy of [0.25, 0.0]) cyl(ca, D, 0.06, 0.28, 0, yy, 0.02, { axis: "x", seg: 8 }); // depth holes
  for (let i = 0; i < 3; i++) clod(ca, M, r, (r() - 0.5) * 1.2, -0.15, -0.1 + (r() - 0.5) * 0.3, 0.14, 0.5);
  const carriage = bakeFree(ca, { mudTop: -1.0, mudBottom: -2.4, mud: 1 }, new THREE.Matrix4().makeTranslation(0, 1.6, -5.0));

  const liftR = range((i) => {
    const p = rotAbout(DOZER.liftB.y, DOZER.liftB.z, T.y, T.z, bladeAngle(i));
    return Math.hypot(p.y - DOZER.liftF.y, p.z - DOZER.liftF.z);
  });
  const ripR = range((i) => {
    const a = ripAngle(i);
    const cy = R.up.y + R.link * Math.sin(a) + 0.35;
    const cz = R.up.z - R.link * Math.cos(a) - 0.25;
    return Math.hypot(cy - DOZER.ripF.y, cz - DOZER.ripF.z);
  });
  const liftRam = ramTemplate(colour, 0.16, liftR[0], liftR[1]);
  const ripRam = ramTemplate(colour, 0.17, ripR[0], ripR[1]);
  return { body, blade, carriage, upperLinks, lowerLinks, liftRam, ripRam, lens };
}

/** A large crawler dozer with a U-blade and a single-shank ripper. */
export function makeDozer(opts: { colour?: string } = {}): Dozer {
  const colour = opts.colour ?? COLOURS.yellow;
  let tpl = dozerTpls.get(colour);
  if (!tpl) {
    tpl = buildDozer(colour);
    dozerTpls.set(colour, tpl);
  }
  const lens = lenses();
  const group = new THREE.Group();
  group.name = "dozer";
  const body = instance(tpl.body);
  swapLenses(body, tpl.lens, lens);
  group.add(body);
  const tracks: Track[] = [];
  for (const s of [-1, 1]) {
    const t = makeTrack({ ...DOZER.track, frameMat: mats.dark(), shoeMat: underSteel() });
    t.group.position.set(s * DOZER.gauge, 0, DOZER.trackZ);
    group.add(t.group);
    tracks.push(t);
  }
  const T = DOZER.trunnion;
  const blade = new THREE.Group();
  blade.position.set(0, T.y, T.z);
  blade.add(instance(tpl.blade));
  group.add(blade);
  const R = DOZER.ripper;
  const upper = instance(tpl.upperLinks);
  upper.position.set(0, R.up.y, R.up.z);
  const lower = instance(tpl.lowerLinks);
  lower.position.set(0, R.up.y - R.dy, R.up.z);
  const carriage = instance(tpl.carriage);
  group.add(upper, lower, carriage);
  const liftRams = [-1, 1].map((s) => ({ s, ram: makeRam(group, tpl.liftRam) }));
  const ripRams = [-1, 1].map((s) => ({ s, ram: makeRam(group, tpl.ripRam) }));
  const state: DozerState = { blade: 0, ripper: 0, speed: 0, distance: 0, lights: false };
  const a = new THREE.Vector3();
  const b = new THREE.Vector3();
  const apply = () => {
    const ang = bladeAngle(state.blade);
    blade.rotation.x = -ang;
    const bp = rotAbout(DOZER.liftB.y, DOZER.liftB.z, T.y, T.z, ang);
    for (const { s, ram } of liftRams) ram.set(a.set(s * DOZER.liftF.x, DOZER.liftF.y, DOZER.liftF.z), b.set(s * DOZER.liftB.x, bp.y, bp.z));
    const ra = ripAngle(state.ripper);
    // links point backward (-z) and up by ra
    upper.rotation.x = ra;
    lower.rotation.x = ra;
    const cy = R.up.y + R.link * Math.sin(ra);
    const cz = R.up.z - R.link * Math.cos(ra);
    carriage.position.set(0, cy, cz);
    for (const { s, ram } of ripRams) ram.set(a.set(s * DOZER.ripF.x, DOZER.ripF.y, DOZER.ripF.z), b.set(s * 0.2, cy + 0.35, cz - 0.25));
    for (const t of tracks) t.setDistance(state.distance);
    lens.set(state.lights);
  };
  apply();
  return {
    group,
    state,
    lenses: lens,
    set(s) {
      Object.assign(state, s);
      apply();
    },
    update(dt) {
      if (state.speed) {
        state.distance += state.speed * dt;
        for (const t of tracks) t.setDistance(state.distance);
      }
    },
  };
}

// ───────────────────────────── blasthole drill ─────────────────────────────

export const DRILL = {
  gauge: 2.0,
  trackZ: -0.6,
  track: { length: 5.8, sprocketR: 0.62, idlerR: 0.55, shoeW: 0.8, pitch: 0.3, rollers: 8, carriers: 2 },
  deckY: 2.15,
  pivot: { x: -0.4, y: 6.0, z: 4.8 },
  mast: { bottom: -3.7, top: 16.0, hx: 0.6, hz: 0.5, bay: 1.3 },
  rodZ: 0.72,
  raiseDeck: { y: 2.3, z: 1.3, dx: 0.85 },
  raiseMast: { y: 3.2, z: -0.55 },
  jack: { xs: 2.35, zf: 5.3, zr: -5.5, travel: 0.55 },
} as const;

export interface DrillRigState {
  /** Mast 0 lowered (horizontal, for tramming) .. 1 vertical. */
  mast: number;
  /** Rotary head position on the mast 0 (bottom) .. 1 (top). */
  feed: number;
  /** Drill-rod rotation rate, rad/s (applied by update). */
  rodSpin: number;
  /** Levelling jacks 0 up .. 1 on the ground. */
  jacks: number;
  speed: number;
  distance: number;
  lights: boolean;
  /** Dust from the hole collar, 0..1. */
  dust: number;
}
export interface DrillRig {
  group: THREE.Group;
  state: DrillRigState;
  set(s: Partial<DrillRigState>): void;
  update(dt: number): void;
  lenses: Lenses;
}

interface DrillTpl {
  body: THREE.Group;
  mast: THREE.Group;
  head: THREE.Group;
  rod: THREE.Group;
  jacks: THREE.Group;
  raise: RamTpl;
  lens: Lenses;
}
const drillTpls = new Map<string, DrillTpl>();
const mastAngle = (t: number) => -(1 - clamp01(t)) * (Math.PI / 2);

function buildDrill(colour: string): DrillTpl {
  const P = paint(colour);
  const D = mats.dark();
  const K = mats.black();
  const S = mats.steel();
  const C = mats.chrome();
  const GR = mats.grate();
  const RB = mats.rubber();
  const lens = lenses();
  const dy = DRILL.deckY;
  const pv = DRILL.pivot;

  // ── carrier: main frame, deck, machinery house, cab, mast support, curtain, tanks ──
  const h = new THREE.Group();
  box(h, P, 2.4, 1.1, 7.6, 0, 1.25, -0.6, { r: 0.08 });
  box(h, P, 5.2, 0.4, 12.5, 0, dy - 0.2, 0.2, { r: 0.06 });
  deck(h, GR, 5.0, 12.3, 0, dy + 0.01, 0.2, 0.04);
  // machinery house (engine + compressor) with doors, louvres and a rear radiator grille
  box(h, P, 4.4, 2.7, 6.6, 0, dy + 1.35, -2.5, { r: 0.12, seg: 2 });
  box(h, P, 4.2, 0.14, 6.4, 0, dy + 2.74, -2.5, { r: 0.05 });
  for (const s of [-1, 1]) {
    for (const z of [-4.9, -3.3, -1.7, -0.1]) {
      box(h, D, 0.03, 2.3, 0.02, s * 2.205, dy + 1.3, z - 0.78, { r: 0.004 });
      louvres(h, D, s * 2.2, dy + 1.95, z, 0.8, 1.1, s);
      box(h, S, 0.04, 0.05, 0.18, s * 2.23, dy + 1.2, z + 0.5, { r: 0.008 });
    }
  }
  grille(h, P, 3.6, 2.2, 0, dy + 1.35, -5.92, Math.PI, 0.14);
  // roof: exhaust stacks, air cleaners, mast rest
  for (const x of [1.1, 1.6]) {
    cyl(h, K, 0.15, 0.9, x, dy + 3.2, -4.2, { seg: 12 });
    cyl(h, K, 0.17, 0.16, x, dy + 3.7, -4.2, { seg: 12, r2: 0.13 });
  }
  for (const x of [1.15, 1.75]) {
    cyl(h, P, 0.26, 0.6, x, dy + 3.1, -1.9, { seg: 16 });
    sphere(h, K, 0.22, x, dy + 3.42, -1.9, { seg: 12, sy: 0.5 });
  }
  box(h, P, 1.5, 0.5, 1.2, -1.6, dy + 3.0, -1.0, { r: 0.06 }); // aircon / filter pod
  // mast rest cradle on the roof
  for (const s of [-1, 1]) box(h, P, 0.2, 0.5, 0.4, pv.x + s * 0.75, dy + 3.05, -4.6, { r: 0.03 });
  box(h, P, 1.7, 0.14, 0.5, pv.x, dy + 3.27, -4.6, { r: 0.03 });
  // mast support A-frame at the front with the pivot pin
  for (const s of [-1, 1]) {
    prism(h, P, [[3.3, dy], [6.0, dy], [5.25, pv.y + 0.35], [4.35, pv.y + 0.35]], 0.22, "zy", pv.x + s * 0.82, { bevel: 0.03 });
    cyl(h, P, 0.3, 0.3, pv.x + s * 0.82, pv.y, pv.z, { axis: "x", seg: 16 });
    // raise-ram base lugs
    box(h, P, 0.3, 0.4, 0.5, pv.x + s * DRILL.raiseDeck.dx, DRILL.raiseDeck.y, DRILL.raiseDeck.z, { r: 0.03 });
  }
  cyl(h, S, 0.14, 1.9, pv.x, pv.y, pv.z, { axis: "x", seg: 10 });
  // drill table, breakout wrench and the dust curtain round the hole collar
  box(h, S, 1.6, 0.12, 1.4, pv.x, dy + 0.08, pv.z + DRILL.rodZ, { r: 0.02 });
  box(h, D, 0.7, 0.25, 0.4, pv.x + 0.6, dy + 0.25, pv.z + DRILL.rodZ, { r: 0.03 });
  const cx = pv.x;
  const cz = pv.z + DRILL.rodZ;
  const r = rng(91);
  const cw = 1.8;
  const cd = 1.3;
  const cTop = dy - 0.4;
  box(h, S, cw + 0.1, 0.12, 0.1, cx, cTop, cz + cd / 2, { r: 0.01 });
  box(h, S, cw + 0.1, 0.12, 0.1, cx, cTop, cz - cd / 2, { r: 0.01 });
  for (const s of [-1, 1]) box(h, S, 0.1, 0.12, cd, cx + (s * cw) / 2, cTop, cz, { r: 0.01 });
  const flap = (x: number, z: number, w: number, ry: number) => {
    const hh = cTop - 0.08 - r() * 0.06;
    box(h, RB, w, hh, 0.025, x, hh / 2 + 0.06, z, { ry, rx: (r() - 0.5) * 0.06, r: 0.006, wear: { mud: 1, dust: 1.3 } });
  };
  for (let i = 0; i < 8; i++) {
    const x = cx - cw / 2 + (i + 0.5) * (cw / 8);
    flap(x, cz + cd / 2, cw / 8 - 0.02, 0);
    flap(x, cz - cd / 2, cw / 8 - 0.02, 0);
  }
  for (let i = 0; i < 7; i++) {
    const z = cz - cd / 2 + (i + 0.5) * (cd / 7);
    flap(cx + cw / 2, z, cd / 7 - 0.02, Math.PI / 2);
    flap(cx - cw / 2, z, cd / 7 - 0.02, Math.PI / 2);
  }
  // cab at the front-left corner, facing the mast
  const cb = cab(h, P, 1.55, 2.6, 2.3, 1.85, dy, 4.65, { door: 1 });
  beacon(cb, K, lens.amber, 0.4, 2.68, -0.7);
  for (const s of [-1, 1]) lamp(cb, D, lens.white, s * 0.45, 2.72, 1.12, { w: 0.24, h: 0.15, pitch: 0.25 });
  mirror(h, K, C, v3(2.62, dy + 2.2, 5.6), v3(2.95, dy + 2.3, 5.95), 0.36, 0.35);
  // compressor receiver, hydraulic tank, pipe/hose runs on the deck
  cyl(h, P, 0.42, 1.7, -1.95, dy + 0.5, 2.7, { axis: "z", seg: 20 });
  sphere(h, P, 0.42, -1.95, dy + 0.5, 3.55, { seg: 16, sy: 1 });
  sphere(h, P, 0.42, -1.95, dy + 0.5, 1.85, { seg: 16, sy: 1 });
  for (const z of [2.0, 3.4]) box(h, P, 0.7, 0.3, 0.2, -1.95, dy + 0.12, z, { r: 0.02 });
  box(h, P, 1.2, 1.3, 1.4, 1.65, dy + 0.65, 2.3, { r: 0.08 });
  for (let i = 0; i < 4; i++) bar(h, mats.hose(), v3(-0.9 + i * 0.12, dy + 0.12, 0.9), v3(pv.x - 0.2 + i * 0.1, dy + 0.12, 4.0), 0.035, 6);
  // walkway handrails and the rear stair
  rail(h, P, [v3(2.55, dy, 3.3), v3(2.55, dy, -6.0), v3(1.0, dy, -6.0)]);
  rail(h, P, [v3(-0.3, dy, -6.0), v3(-2.55, dy, -6.0), v3(-2.55, dy, 6.4), v3(-1.45, dy, 6.4)]);
  rail(h, P, [v3(0.65, dy, 6.4), v3(2.55, dy, 6.4), v3(2.55, dy, 4.0)]);
  stairs(h, P, GR, P, v3(0.35, 0.4, -7.75), v3(0.35, dy, -6.05), 0.7);
  // rear and front lights, fire bottles, deck jacks' barrels
  for (const s of [-1, 1]) {
    lamp(h, D, lens.red, s * 2.1, dy - 0.15, -6.06, { w: 0.24, h: 0.14, yaw: Math.PI });
    lamp(h, D, lens.white, s * 1.6, dy + 2.6, -5.9, { w: 0.3, h: 0.2, yaw: Math.PI, pitch: 0.25 });
    lamp(h, D, lens.white, s * 2.2, dy - 0.15, 6.46, { w: 0.3, h: 0.2 });
  }
  bottle(h, mats.red(), S, -2.2, dy, 0.6);
  bottle(h, mats.red(), S, -2.2, dy, 0.9);
  const J = DRILL.jack;
  for (const s of [-1, 1])
    for (const z of [J.zf, J.zr]) {
      cyl(h, P, 0.24, 1.25, s * J.xs, 1.425, z, { seg: 16 });
      box(h, P, 0.6, 0.35, 0.6, s * (J.xs - 0.15), dy - 0.15, z, { r: 0.04 });
    }
  const body = bakeFree(h, { mudTop: 2.0, mudBottom: 0.3 });

  // ── mast (origin at the pivot, local +y along the mast, +z toward the rod side) ──
  const M = DRILL.mast;
  const m = new THREE.Group();
  const chords: [number, number][] = [
    [M.hx, M.hz],
    [-M.hx, M.hz],
    [-M.hx, -M.hz],
    [M.hx, -M.hz],
  ];
  for (const [x, z] of chords) beam(m, P, v3(x, M.bottom, z), v3(x, M.top, z), 0.2, 0.2, v3(0, 0, 1), 0.02);
  const bays = Math.round((M.top - M.bottom) / M.bay);
  const bh = (M.top - M.bottom) / bays;
  for (let i = 0; i < bays; i++) {
    const y0 = M.bottom + i * bh;
    const y1 = y0 + bh;
    for (let k = 0; k < 4; k++) {
      const [xa, za] = chords[k]!;
      const [xb, zb] = chords[(k + 1) % 4]!;
      bar(m, P, v3(xa, y0, za), v3(xb, y0, zb), 0.05, 6);
      const flip = (i + k) % 2 === 0;
      bar(m, P, v3(flip ? xa : xb, y0, flip ? za : zb), v3(flip ? xb : xa, y1, flip ? zb : za), 0.045, 6);
    }
  }
  // head guide rails on the rod face, crown at the top, centraliser at the bottom
  for (const s of [-1, 1]) beam(m, S, v3(s * 0.42, M.bottom + 0.5, M.hz + 0.13), v3(s * 0.42, M.top - 0.5, M.hz + 0.13), 0.12, 0.09, v3(0, 0, 1), 0.01);
  box(m, P, 1.5, 0.9, 1.6, 0, M.top + 0.35, 0.1, { r: 0.06 });
  cyl(m, S, 0.45, 0.25, 0, M.top + 0.55, 0.3, { axis: "x", seg: 18 });
  box(m, P, 1.4, 0.4, 1.4, 0, M.bottom + 0.2, 0.15, { r: 0.04 });
  box(m, D, 0.6, 0.3, 0.5, 0, M.bottom + 0.55, DRILL.rodZ, { r: 0.04 });
  // floodlights up the mast
  for (const yy of [M.top - 0.3, 8]) for (const s of [-1, 1]) lamp(m, D, lens.white, s * 0.75, yy, M.hz + 0.15, { w: 0.42, h: 0.28, pitch: 0.7, yaw: s * 0.3 });
  // pipe carousel on the right side: four rods in a rack
  const px = -M.hx - 0.55;
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2 + 0.4;
    cyl(m, S, 0.12, 10.4, px + Math.cos(a) * 0.28, 4.3, Math.sin(a) * 0.28, { seg: 10 });
  }
  cyl(m, S, 0.05, 11, px, 4.3, 0, { seg: 8 });
  for (const yy of [-1.0, 4.3, 9.5]) {
    cyl(m, P, 0.48, 0.14, px, yy, 0, { seg: 18 });
    beam(m, P, v3(px + 0.4, yy, 0), v3(-M.hx, yy, 0), 0.12, 0.12, Y, 0.01);
  }
  // hose and cable runs down the mast
  bar(m, mats.hose(), v3(M.hx + 0.12, M.bottom + 0.5, 0.2), v3(M.hx + 0.12, M.top - 1, 0.2), 0.05, 6);
  bar(m, mats.hose(), v3(M.hx + 0.12, M.bottom + 0.5, -0.05), v3(M.hx + 0.12, M.top - 1, -0.05), 0.04, 6);
  // raise-ram lugs on the back face
  for (const s of [-1, 1]) box(m, P, 0.3, 0.45, 0.4, s * DRILL.raiseDeck.dx, DRILL.raiseMast.y, DRILL.raiseMast.z, { r: 0.03 });
  const mast = bakeFree(m, { mudTop: 3.6, mudBottom: 2.0, mud: 0.6, dust: 0.9 }, new THREE.Matrix4().makeTranslation(pv.x, pv.y, pv.z));

  // ── rotary head (local origin on the rod axis at the head's bottom) ──
  const hd = new THREE.Group();
  box(hd, P, 1.2, 0.95, 0.95, 0, 0.75, -0.08, { r: 0.1, seg: 2 });
  box(hd, D, 1.0, 0.3, 0.8, 0, 0.18, -0.08, { r: 0.05 });
  box(hd, P, 1.3, 0.22, 1.0, 0, 1.3, -0.08, { r: 0.05 });
  cyl(hd, S, 0.16, 0.35, 0, 1.55, 0, { seg: 14 }); // air swivel
  for (const s of [-1, 1]) {
    cyl(hd, D, 0.2, 0.6, s * 0.3, 1.85, 0, { seg: 14 });
    box(hd, S, 0.14, 1.1, 0.12, s * 0.42, 0.75, -0.5, { r: 0.02 }); // guide shoes
  }
  cyl(hd, S, 0.24, 0.25, 0, 0.12, 0, { seg: 16 });
  for (let i = 0; i < 4; i++) bar(hd, mats.hose(), v3(-0.4 + i * 0.25, 1.7, -0.2), v3(-0.6 + i * 0.25, 2.4, -0.7), 0.04, 6);
  const head = bakeFree(hd, { mudTop: 0, mudBottom: -1, mud: 0.2 });

  // ── drill rod: unit length along -y from the head, with tool joint and weld seams so it reads spinning ──
  const rd = new THREE.Group();
  cyl(rd, S, 0.135, 1, 0, -0.5, 0, { seg: 16 });
  for (const a of [0, Math.PI]) box(rd, D, 0.04, 1, 0.03, Math.cos(a) * 0.13, -0.5, Math.sin(a) * 0.13, { r: 0 });
  const rod = bakeFree(rd, { mudTop: -2, mudBottom: -1, mud: 0, dust: 0.6 });
  // ── jacks' moving rods and pads, all four together ──
  const jk = new THREE.Group();
  for (const s of [-1, 1])
    for (const z of [J.zf, J.zr]) {
      cyl(jk, C, 0.16, 1.4, s * J.xs, 1.25, z, { seg: 14 });
      cyl(jk, P, 0.22, 0.25, s * J.xs, 0.67, z, { seg: 14 });
      box(jk, D, 0.75, 0.14, 0.75, s * J.xs, 0.62, z, { r: 0.03 });
    }
  const jacks = bakeFree(jk, { mudTop: 1.2, mudBottom: 0.5, mud: 1 });

  const raiseR = range((i) => {
    const ang = mastAngle(i);
    const p = rotAbout(pv.y + DRILL.raiseMast.y, pv.z + DRILL.raiseMast.z, pv.y, pv.z, -ang);
    return Math.hypot(p.y - DRILL.raiseDeck.y, p.z - DRILL.raiseDeck.z);
  }, 24, 0);
  const raise = ramTemplate(colour, 0.2, raiseR[0], raiseR[1]);
  return { body, mast, head, rod, jacks, raise, lens };
}

/** Dust puffs at the hole collar: soft points that rise and spread, one draw call. */
function dustCloud(n: number, seed: number) {
  const r = rng(seed);
  const pos = new Float32Array(n * 3);
  const size = new Float32Array(n);
  const alpha = new Float32Array(n);
  const life = new Float32Array(n);
  const vel = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) life[i] = r();
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  g.setAttribute("size", new THREE.BufferAttribute(size, 1));
  g.setAttribute("alpha", new THREE.BufferAttribute(alpha, 1));
  const mat = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    uniforms: { colour: { value: new THREE.Color("#cdbd9f") }, amount: { value: 0 }, scale: { value: 500 } },
    vertexShader: `attribute float size; attribute float alpha; varying float vA;
      uniform float scale;
      void main(){ vA = alpha; vec4 mv = modelViewMatrix * vec4(position,1.0); gl_PointSize = size * projectionMatrix[1][1] * scale / -mv.z; gl_Position = projectionMatrix * mv; }`,
    fragmentShader: `uniform vec3 colour; uniform float amount; varying float vA;
      void main(){ vec2 c = gl_PointCoord - 0.5; float d = dot(c,c)*4.0; float a = exp(-d*3.0) * (1.0 - d) * vA * amount; if (a < 0.003) discard;
        gl_FragColor = vec4(colour, a);
        #include <tonemapping_fragment>
        #include <colorspace_fragment>
      }`,
  });
  const pts = new THREE.Points(g, mat);
  pts.frustumCulled = false;
  const vp = new THREE.Vector2();
  pts.onBeforeRender = (renderer) => {
    renderer.getDrawingBufferSize(vp);
    mat.uniforms.scale!.value = vp.y * 0.5;
  };
  const spawn = (i: number) => {
    const a = r() * Math.PI * 2;
    pos[i * 3] = Math.cos(a) * 0.9;
    pos[i * 3 + 1] = 0.2 + r() * 0.3;
    pos[i * 3 + 2] = Math.sin(a) * 0.9;
    vel[i * 3] = Math.cos(a) * (0.6 + r() * 0.8);
    vel[i * 3 + 1] = 0.5 + r() * 0.9;
    vel[i * 3 + 2] = Math.sin(a) * (0.6 + r() * 0.8);
  };
  for (let i = 0; i < n; i++) spawn(i);
  const step = (dt: number) => {
    for (let i = 0; i < n; i++) {
      let l = life[i]! + dt * 0.25;
      if (l > 1) {
        l -= 1;
        spawn(i);
      }
      life[i] = l;
      pos[i * 3] = pos[i * 3]! + vel[i * 3]! * dt;
      pos[i * 3 + 1] = pos[i * 3 + 1]! + vel[i * 3 + 1]! * dt;
      pos[i * 3 + 2] = pos[i * 3 + 2]! + vel[i * 3 + 2]! * dt;
      size[i] = 1.2 + l * 4.5;
      alpha[i] = Math.sin(l * Math.PI) * 0.55;
    }
    g.getAttribute("position").needsUpdate = true;
    g.getAttribute("size").needsUpdate = true;
    g.getAttribute("alpha").needsUpdate = true;
  };
  // pre-warm so a still frame shows a developed cloud
  for (let k = 0; k < 120; k++) step(1 / 30);
  return { points: pts, mat, step };
}

/** A tracked rotary blasthole drill with a 20 m lattice mast, rotary head, carousel and dust curtain. */
export function makeDrillRig(opts: { colour?: string } = {}): DrillRig {
  const colour = opts.colour ?? COLOURS.yellow;
  let tpl = drillTpls.get(colour);
  if (!tpl) {
    tpl = buildDrill(colour);
    drillTpls.set(colour, tpl);
  }
  const lens = lenses();
  const group = new THREE.Group();
  group.name = "drill-rig";
  const body = instance(tpl.body);
  swapLenses(body, tpl.lens, lens);
  group.add(body);
  const tracks: Track[] = [];
  for (const s of [-1, 1]) {
    const t = makeTrack({ ...DRILL.track, frameMat: mats.dark() });
    t.group.position.set(s * DRILL.gauge, 0, DRILL.trackZ);
    group.add(t.group);
    tracks.push(t);
  }
  const pv = DRILL.pivot;
  const mastG = new THREE.Group();
  mastG.position.set(pv.x, pv.y, pv.z);
  const mastBody = instance(tpl.mast);
  swapLenses(mastBody, tpl.lens, lens);
  mastG.add(mastBody);
  const head = instance(tpl.head);
  mastG.add(head);
  const rod = instance(tpl.rod);
  mastG.add(rod);
  group.add(mastG);
  const jacks = instance(tpl.jacks);
  group.add(jacks);
  const raise = [-1, 1].map((s) => ({ s, ram: makeRam(group, tpl.raise) }));
  const dust = dustCloud(90, 5);
  dust.points.position.set(pv.x, 0, pv.z + DRILL.rodZ);
  group.add(dust.points);
  const state: DrillRigState = { mast: 1, feed: 0.6, rodSpin: 0, jacks: 1, speed: 0, distance: 0, lights: false, dust: 0 };
  let rodAngle = 0;
  const a = new THREE.Vector3();
  const b = new THREE.Vector3();
  const M = DRILL.mast;
  const apply = () => {
    const ang = mastAngle(state.mast);
    mastG.rotation.x = ang;
    const hy = lerp(M.bottom + 1.0, M.top - 2.6, clamp01(state.feed));
    head.position.set(0, hy, DRILL.rodZ);
    // the rod reaches the ground through the table when the mast is up; otherwise it stops at the mast foot
    const drilling = state.mast > 0.995;
    const bottom = drilling ? -pv.y - 0.3 : M.bottom + 0.4;
    const len = Math.max(0.1, hy - bottom);
    rod.position.set(0, hy, DRILL.rodZ);
    rod.scale.set(1, len, 1);
    rod.rotation.y = rodAngle;
    const mp = rotAbout(pv.y + DRILL.raiseMast.y, pv.z + DRILL.raiseMast.z, pv.y, pv.z, -ang);
    for (const { s, ram } of raise) ram.set(a.set(pv.x + s * DRILL.raiseDeck.dx, DRILL.raiseDeck.y, DRILL.raiseDeck.z), b.set(pv.x + s * DRILL.raiseDeck.dx, mp.y, mp.z));
    jacks.position.y = -clamp01(state.jacks) * DRILL.jack.travel;
    for (const t of tracks) t.setDistance(state.distance);
    dust.points.visible = state.dust > 0.01 && drilling;
    dust.mat.uniforms.amount!.value = clamp01(state.dust);
    lens.set(state.lights);
  };
  apply();
  return {
    group,
    state,
    lenses: lens,
    set(s) {
      Object.assign(state, s);
      apply();
    },
    update(dt) {
      if (state.rodSpin) {
        rodAngle = (rodAngle + state.rodSpin * dt) % (Math.PI * 2);
        rod.rotation.y = rodAngle;
      }
      if (state.speed) {
        state.distance += state.speed * dt;
        for (const t of tracks) t.setDistance(state.distance);
      }
      if (dust.points.visible) dust.step(dt);
    },
  };
}
