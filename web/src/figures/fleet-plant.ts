/**
 * Plant pieces: a troughed belt conveyor on idlers (ground trestles or an elevated truss gantry, optional
 * hood) whose ore visibly moves, and a primary gyratory crusher station with a raised tipping platform,
 * a lined dump pocket, a pedestal rock breaker and a clad crusher building.
 *
 * Units metres, y up. Conveyor points are the belt centreline at the top of the carrying idlers.
 * Sizes follow common practice for a 1200 mm belt: 152 mm rolls, carrying sets every 1.2 m, return sets
 * every 3 m, walkway about 0.95 m below the belt with 1.1 m handrails, trestles every 6 m.
 */
import * as THREE from "three";
import {
  bake,
  bakeFree,
  bar,
  beam,
  box,
  clamp01,
  cyl,
  deck,
  grime,
  gravelNormal,
  lamp,
  lenses,
  mats,
  node,
  Noise3,
  paint,
  rng,
  rockGeometry,
  rockMaterial,
  ROCK_PALETTES,
  colourHeap,
  heapMaterial,
  smoothstep,
  sphere,
  v3,
  type Lenses,
  type RockPalette,
  type V3,
} from "./fleet-kit";

const UP = new THREE.Vector3(0, 1, 0);

// ───────────────────────────── shared plant materials ─────────────────────────────

const plantCache = new Map<string, unknown>();
function once<T>(key: string, make: () => T): T {
  if (!plantCache.has(key)) plantCache.set(key, make());
  return plantCache.get(key) as T;
}

export const plantMats = {
  /** Conveyor steelwork: hot-dip galvanised, dusty. */
  galv: () => mats.galv(),
  /** Painted structural steel for the crusher building frame and trims. */
  frame: () => paint("#3f4a52", { rough: 0.6, coat: 0.15 }),
  /** Corrugated wall cladding. */
  clad: (colour = "#c3c8c4") => paint(colour, { rough: 0.5, coat: 0.25, scale: 0.05 }),
  roof: () => paint("#9ea3a0", { rough: 0.55, coat: 0.15, scale: 0.05 }),
  /** Belt rubber. */
  belt: () => once("belt", () => grime(new THREE.MeshStandardMaterial({ color: "#1b1b1a", roughness: 0.82 }), { dust: "#8e7f69", mudAmount: 0.3, dustAmount: 0.45, scale: 0.4 })),
  /** Gearbox / motor enamel. */
  drive: () => paint("#3c5566", { coat: 0.5, rough: 0.4 }),
  /** Wear-plate liners in the pocket: bare, scoured steel. */
  liner: () => once("liner", () => grime(new THREE.MeshStandardMaterial({ color: "#5f5c57", metalness: 0.55, roughness: 0.62 }), { mudAmount: 1, dustAmount: 1.2, scale: 0.12 })),
  /** Compacted crushed-rock fill for the tipping platform and ramp. */
  fill: () =>
    once("fill", () =>
      grime(
        new THREE.MeshStandardMaterial({ color: "#9a8466", roughness: 0.97, normalMap: gravelNormal(), normalScale: new THREE.Vector2(0.6, 0.6) }),
        { dust: "#c1ae8f", mud: "#7b6850", scale: 0.05, variation: 1 },
      ),
    ),
  /** Recessed panel joints in precast walls. */
  joint: () => paint("#6f6b63", { rough: 0.95, coat: 0 }),
  translucent: () =>
    once("translucent", () => new THREE.MeshStandardMaterial({ color: "#dfe4df", roughness: 0.35, metalness: 0, emissive: "#fff3d6", emissiveIntensity: 0.05 })),
  hazardBlack: () => paint("#1c1c1b", { coat: 0.2 }),
  /** Open steel bar grating: the top and bottom faces are see-through bars up close, solid far away. */
  grate: () =>
    once("grate-open", () => {
      const m = grime(new THREE.MeshStandardMaterial({ color: "#6e706b", metalness: 0.6, roughness: 0.55 }), { dustAmount: 1.1 });
      const base = m.onBeforeCompile;
      m.onBeforeCompile = (sh, r) => {
        base.call(m, sh, r);
        sh.vertexShader = sh.vertexShader
          .replace("#include <common>", "#include <common>\nvarying vec3 vGrP;\nvarying vec3 vGrN;")
          .replace("#include <begin_vertex>", "#include <begin_vertex>\nvGrP = position;\nvGrN = normal;");
        sh.fragmentShader = sh.fragmentShader
          .replace("#include <common>", "#include <common>\nvarying vec3 vGrP;\nvarying vec3 vGrN;")
          .replace(
            "#include <clipping_planes_fragment>",
            `#include <clipping_planes_fragment>
{
  vec2 gq = vGrP.xz;
  float gfw = max(fwidth(gq.x), fwidth(gq.y));
  if (abs(vGrN.y) > 0.6 && gfw < 0.009) {
    float b1 = abs(fract(gq.x * 30.0) - 0.5);
    float b2 = abs(fract(gq.y * 10.0) - 0.5);
    if (b1 < 0.32 && b2 < 0.43) discard;
  }
}`,
          );
      };
      m.customProgramCacheKey = () => "fleet-plant-grate-v1";
      return m;
    }),
  /** Chain-link fence mesh: alpha-tested diamond grid, double-sided. */
  mesh: () =>
    once("fence-mesh", () => {
      const S = 64;
      const d = new Uint8Array(S * S * 4);
      for (let y = 0; y < S; y++)
        for (let x = 0; x < S; x++) {
          const u = (x + y) % 32;
          const v = (x - y + 64) % 32;
          const on = u < 3 || v < 3;
          const k = (y * S + x) * 4;
          // alphaMap samples the green channel
          d[k] = d[k + 1] = d[k + 2] = on ? 255 : 0;
          d[k + 3] = 255;
        }
      const t = new THREE.DataTexture(d, S, S, THREE.RGBAFormat);
      t.wrapS = t.wrapT = THREE.RepeatWrapping;
      t.repeat.set(26, 10);
      t.magFilter = THREE.LinearFilter;
      t.minFilter = THREE.LinearMipmapLinearFilter;
      t.generateMipmaps = true;
      t.needsUpdate = true;
      return new THREE.MeshStandardMaterial({ color: "#9ea29d", metalness: 0.6, roughness: 0.5, alphaMap: t, transparent: true, depthWrite: false, alphaTest: 0.02, side: THREE.DoubleSide });
    }),
  /** Arched conveyor hood: galvanised sheet with circumferential corrugations (normal map along the run). */
  hood: () =>
    once("hood", () =>
      grime(
        new THREE.MeshStandardMaterial({
          color: "#c2c6c3",
          metalness: 0.55,
          roughness: 0.42,
          normalMap: corrugationNormal(),
          normalScale: new THREE.Vector2(1, 1),
        }),
        { dustAmount: 1.0, mudAmount: 0.6, scale: 0.12 },
      ),
    ),
};

let corrTex: THREE.DataTexture | null = null;
/** Normal map of 13 sinusoidal corrugations per texture tile along v (tile = 1 m on the hood). */
function corrugationNormal(): THREE.DataTexture {
  if (corrTex) return corrTex;
  const S = 128;
  const d = new Uint8Array(S * S * 4);
  for (let y = 0; y < S; y++) {
    const slope = Math.cos((y / S) * Math.PI * 2 * 13) * 0.75;
    const l = Math.hypot(slope, 1);
    for (let x = 0; x < S; x++) {
      const k = (y * S + x) * 4;
      d[k] = 128;
      d[k + 1] = Math.round((slope / l * 0.5 + 0.5) * 255);
      d[k + 2] = Math.round((1 / l * 0.5 + 0.5) * 255);
      d[k + 3] = 255;
    }
  }
  corrTex = new THREE.DataTexture(d, S, S, THREE.RGBAFormat);
  corrTex.wrapS = corrTex.wrapT = THREE.RepeatWrapping;
  corrTex.minFilter = THREE.LinearMipmapLinearFilter;
  corrTex.magFilter = THREE.LinearFilter;
  corrTex.generateMipmaps = true;
  corrTex.needsUpdate = true;
  return corrTex;
}

// ───────────────────────────── path frames and sweeps ─────────────────────────────

interface Stn {
  p: V3;
  f: V3;
  X: V3;
  Y: V3;
  s: number;
}

function frameOf(f: V3) {
  const X = new THREE.Vector3().crossVectors(UP, f);
  if (X.lengthSq() < 1e-8) X.set(1, 0, 0);
  X.normalize();
  const Y = new THREE.Vector3().crossVectors(f, X).normalize();
  return { X, Y };
}

interface Seg {
  a: V3;
  b: V3;
  len: number;
  f: V3;
  X: V3;
  Y: V3;
  s0: number;
}

function segmentsOf(points: V3[]): { segs: Seg[]; total: number } {
  const pts: V3[] = [];
  for (const p of points) if (!pts.length || pts[pts.length - 1]!.distanceTo(p) > 0.05) pts.push(p.clone());
  if (pts.length < 2) throw Error("makeConveyor needs two distinct points");
  const segs: Seg[] = [];
  let s = 0;
  for (let i = 0; i < pts.length - 1; i++) {
    const a = pts[i]!;
    const b = pts[i + 1]!;
    const d = b.clone().sub(a);
    const len = d.length();
    const f = d.normalize();
    const { X, Y } = frameOf(f);
    segs.push({ a, b, len, f, X, Y, s0: s });
    s += len;
  }
  return { segs, total: s };
}

/** Stations every ~`step` m along the path with mitred frames at the joints. */
function stationsOf(segs: Seg[], step: number): Stn[] {
  const out: Stn[] = [];
  segs.forEach((g, k) => {
    const m = Math.max(1, Math.ceil(g.len / step));
    for (let j = k === 0 ? 0 : 1; j <= m; j++) {
      const t = j / m;
      const p = g.a.clone().lerp(g.b, t);
      let f = g.f.clone();
      if (j === m && k < segs.length - 1) f = g.f.clone().add(segs[k + 1]!.f).normalize();
      const { X, Y } = frameOf(f);
      out.push({ p, f, X, Y, s: g.s0 + t * g.len });
    }
  });
  return out;
}

function sampleStn(st: Stn[], s: number, out: { p: V3; X: V3; Y: V3; f: V3 }) {
  let lo = 0;
  let hi = st.length - 1;
  if (s <= st[0]!.s) hi = 1;
  else if (s >= st[hi]!.s) lo = hi - 1;
  else
    while (hi - lo > 1) {
      const mid = (lo + hi) >> 1;
      if (st[mid]!.s <= s) lo = mid;
      else hi = mid;
    }
  const A = st[lo]!;
  const B = st[hi]!;
  const t = B.s > A.s ? clamp01((s - A.s) / (B.s - A.s)) : 0;
  out.p.copy(A.p).lerp(B.p, t);
  out.X.copy(A.X).lerp(B.X, t).normalize();
  out.Y.copy(A.Y).lerp(B.Y, t).normalize();
  out.f.copy(A.f).lerp(B.f, t).normalize();
  return out;
}

/**
 * Sweeps a cross-section (local x across, y up) along stations. Profiles traversed left (−x) to right
 * (+x) across the top face outward-up. `v` runs in metres / vPeriod along the path.
 */
function sweep(st: Stn[], profile: (s: Stn) => [number, number][], o: { closed?: boolean; vPeriod?: number; wear?: [number, number, number] } = {}) {
  const rows = st.length;
  const cols = profile(st[0]!).length;
  const pos = new Float32Array(rows * cols * 3);
  const uv = new Float32Array(rows * cols * 2);
  const vp = o.vPeriod ?? 1;
  st.forEach((S, i) => {
    const pr = profile(S);
    for (let j = 0; j < cols; j++) {
      const [x, y] = pr[j]!;
      const k = (i * cols + j) * 3;
      pos[k] = S.p.x + S.X.x * x + S.Y.x * y;
      pos[k + 1] = S.p.y + S.X.y * x + S.Y.y * y;
      pos[k + 2] = S.p.z + S.X.z * x + S.Y.z * y;
      uv[(i * cols + j) * 2] = j / (cols - 1);
      uv[(i * cols + j) * 2 + 1] = S.s / vp;
    }
  });
  const idx: number[] = [];
  const cc = o.closed ? cols : cols - 1;
  for (let i = 0; i < rows - 1; i++)
    for (let j = 0; j < cc; j++) {
      const j2 = (j + 1) % cols;
      const a = i * cols + j;
      const b = (i + 1) * cols + j;
      const c = i * cols + j2;
      const d = (i + 1) * cols + j2;
      idx.push(a, b, c, c, b, d);
    }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  g.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  g.setIndex(idx);
  g.computeVertexNormals();
  if (o.wear) {
    const w = new Float32Array(rows * cols * 3);
    for (let i = 0; i < rows * cols; i++) w.set(o.wear, i * 3);
    g.setAttribute("wear", new THREE.BufferAttribute(w, 3));
  }
  g.computeBoundingSphere();
  return g;
}

/** A group whose local frame is (X across, Y up-normal, f along) at p. */
function frameNode(parent: THREE.Object3D, p: V3, X: V3, Y: V3, f: V3) {
  const g = new THREE.Group();
  const m = new THREE.Matrix4().makeBasis(X, Y, f).setPosition(p);
  m.decompose(g.position, g.quaternion, g.scale);
  parent.add(g);
  return g;
}

/** Corrugated sheet in the xy plane facing +z: x ∈ [−w/2, w/2], y ∈ [0, h], ribs running along y. */
export function corrugated(w: number, h: number, pitch = 0.2, depth = 0.032, rows = 1): THREE.BufferGeometry {
  const n = Math.max(1, Math.round(w / pitch));
  const p = w / n;
  const prof: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    const x0 = -w / 2 + i * p;
    prof.push([x0, 0], [x0 + p * 0.2, 0], [x0 + p * 0.36, depth], [x0 + p * 0.64, depth], [x0 + p * 0.8, 0]);
  }
  prof.push([w / 2, 0]);
  const cols = prof.length;
  const pos = new Float32Array(cols * (rows + 1) * 3);
  const uv = new Float32Array(cols * (rows + 1) * 2);
  for (let r = 0; r <= rows; r++)
    for (let j = 0; j < cols; j++) {
      const [x, z] = prof[j]!;
      const k = r * cols + j;
      pos.set([x, (h * r) / rows, z], k * 3);
      uv.set([(x + w / 2) / w, r / rows], k * 2);
    }
  const idx: number[] = [];
  for (let r = 0; r < rows; r++)
    for (let j = 0; j < cols - 1; j++) {
      const a = r * cols + j;
      const b = a + 1;
      const c = a + cols;
      const d = c + 1;
      idx.push(a, b, c, c, b, d);
    }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  g.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  g.setIndex(idx);
  g.computeVertexNormals();
  return g;
}

/** Adds a corrugated panel to `parent`, bottom-centre at (x, y, z), facing yaw (0 = +z). */
function panel(parent: THREE.Object3D, mat: THREE.Material, w: number, h: number, x: number, y: number, z: number, yaw: number, pitch = 0.2, depth = 0.032) {
  const m = new THREE.Mesh(corrugated(w, h, pitch, depth), mat);
  m.position.set(x, y, z);
  m.rotation.y = yaw;
  parent.add(m);
  return m;
}

// ───────────────────────────── ore textures ─────────────────────────────

const oreTexCache = new Map<string, THREE.DataTexture>();
/** Tileable granular ore-bed colour texture (pebbles from a palette, shaded, with fines between). */
function oreTexture(pal: readonly string[], seed = 9): THREE.DataTexture {
  const key = pal.join(",");
  const hit = oreTexCache.get(key);
  if (hit) return hit;
  const S = 256;
  const r = rng(seed);
  const cells = 26;
  const pts: { x: number; y: number; c: THREE.Color; h: number }[] = [];
  const cs = pal.map((p) => new THREE.Color(p));
  for (let i = 0; i < cells * cells; i++) {
    const c = cs[Math.floor(r() * cs.length)]!.clone();
    c.offsetHSL(0, (r() - 0.5) * 0.05, (r() - 0.5) * 0.12);
    pts.push({ x: r(), y: r(), c, h: 0.6 + r() * 0.5 });
  }
  const fines = cs[0]!.clone().multiplyScalar(0.55);
  const d = new Uint8Array(S * S * 4);
  const tmp = new THREE.Color();
  for (let y = 0; y < S; y++)
    for (let x = 0; x < S; x++) {
      const fx = (x / S) * cells;
      const fy = (y / S) * cells;
      const cx = Math.floor(fx);
      const cy = Math.floor(fy);
      let d1 = 9;
      let d2 = 9;
      let best = pts[0]!;
      let bx = 0;
      let by = 0;
      for (let oy = -1; oy <= 1; oy++)
        for (let ox = -1; ox <= 1; ox++) {
          const gx = (cx + ox + cells) % cells;
          const gy = (cy + oy + cells) % cells;
          const q = pts[gy * cells + gx]!;
          const px = cx + ox + q.x;
          const py = cy + oy + q.y;
          const dd = Math.hypot(px - fx, py - fy);
          if (dd < d1) {
            d2 = d1;
            d1 = dd;
            best = q;
            bx = px - fx;
            by = py - fy;
          } else if (dd < d2) d2 = dd;
        }
      const edge = Math.min(1, (d2 - d1) * 4);
      // light from the top-left of each pebble
      const shade = 0.75 + 0.35 * clamp01(0.5 + (bx + by) * 0.9) * best.h;
      tmp.copy(best.c).multiplyScalar(shade);
      tmp.lerp(fines, 1 - smoothstep(0.05, 0.4, edge));
      const k = (y * S + x) * 4;
      // the colour is linear in THREE.Color; write sRGB bytes
      const sr = tmp.clone().convertLinearToSRGB();
      d[k] = Math.round(clamp01(sr.r) * 255);
      d[k + 1] = Math.round(clamp01(sr.g) * 255);
      d[k + 2] = Math.round(clamp01(sr.b) * 255);
      d[k + 3] = 255;
    }
  const t = new THREE.DataTexture(d, S, S, THREE.RGBAFormat);
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.minFilter = THREE.LinearMipmapLinearFilter;
  t.magFilter = THREE.LinearFilter;
  t.generateMipmaps = true;
  t.anisotropy = 8;
  t.colorSpace = THREE.SRGBColorSpace;
  t.needsUpdate = true;
  oreTexCache.set(key, t);
  return t;
}

// ───────────────────────────── conveyor ─────────────────────────────

export interface ConveyorOptions {
  /** Belt width, m (default 1.2). */
  width?: number;
  /** Troughing angle of the wing rolls, degrees (default 35). */
  troughDeg?: number;
  /** Carry the run in a box-truss gantry on tower trestles instead of ground trestles. */
  withGantry?: boolean;
  /** Semicircular corrugated hood over the carrying belt. */
  covered?: boolean;
  /** Belt speed, m/s (default 3). */
  speed?: number;
  /** Ore colours (default "ore"). */
  ore?: RockPalette;
  /** Visible load 0..1 (default 0.8). */
  load?: number;
  /** Walkway side: +x of the path frame ("left", default), "right", "both" or "none". */
  walkway?: "left" | "right" | "both" | "none";
  /** Head drive and discharge chute at the last point (default true). */
  head?: boolean;
  /** Tail pulley and loading chute at the first point (default true). */
  tail?: boolean;
  seed?: number;
}

export interface ConveyorState {
  /** Belt speed, m/s. */
  speed: number;
  /** Visible ore load 0..1. */
  load: number;
  /** Whether the belt runs (update(dt) moves the ore). */
  running: boolean;
  /** Walkway lights. */
  lights: boolean;
}

export interface Conveyor {
  group: THREE.Group;
  state: ConveyorState;
  set(s: Partial<ConveyorState>): void;
  update(dt: number): void;
  /** Path length, m. */
  length: number;
  lenses: Lenses;
  /** Loading point over the tail and discharge point under the head chute. */
  anchors: { tail: THREE.Object3D; head: THREE.Object3D };
}

let rollGeo: THREE.BufferGeometry | null = null;
/** Unit-length idler roll along x (radius 0.076): a 10-sided steel tube, scaled per instance to the roll face. */
function rollGeometry(): THREE.BufferGeometry {
  if (rollGeo) return rollGeo;
  const g = new THREE.Group();
  cyl(g, mats.steel(), 0.076, 1, 0, 0, 0, { axis: "x", seg: 10 });
  const b = bakeFree(g, { mudTop: 1, mudBottom: -1, mud: 0.25, dust: 0.9 });
  rollGeo = (b.children[0] as THREE.Mesh).geometry;
  return rollGeo;
}

/**
 * Troughed belt conveyor along a polyline. Returns the group (static steel baked by material, idler rolls
 * instanced, the ore bed and ore lumps animated by update(dt)).
 */
export function makeConveyor(path: { points: V3[] } | { from: V3; to: V3 }, opts: ConveyorOptions = {}): Conveyor {
  const points = "points" in path ? path.points : [path.from, path.to];
  const { segs, total } = segmentsOf(points);
  const w = opts.width ?? 1.2;
  const theta = ((opts.troughDeg ?? 35) * Math.PI) / 180;
  const pal = ROCK_PALETTES[opts.ore ?? "ore"];
  const r = rng(opts.seed ?? 17);
  const R = 0.076; // roll radius
  const c = w * 0.36; // centre roll length
  const wl = (w - c) / 2; // wing length
  const tb = 0.016; // belt thickness
  const xs = w / 2 + 0.32; // stringer centreline offset
  const yStrTop = -0.33;
  const yStrBot = -0.63;
  const yRet = -0.86; // return belt top
  const yWalk = -0.95; // walkway floor
  const walkIn = xs + 0.1;
  const walkOut = walkIn + 0.9;
  const gantry = opts.withGantry === true;
  const gw = w + 2.5; // gantry inside width
  const yChordB = -1.05;
  const yChordT = 1.6;
  const T = Math.min(2.6, total / 4);
  const thetaAt = (s: number) => theta * smoothstep(0, T, s) * smoothstep(total, total - T, s);
  const troughY = (x: number, th: number) => {
    const a = Math.abs(x);
    return a <= c / 2 ? 0 : Math.min(wl * Math.cos(th), a - c / 2) * Math.tan(th);
  };
  const walkSides: number[] =
    opts.walkway === "none" ? [] : opts.walkway === "right" ? [-1] : opts.walkway === "both" || gantry ? [1, -1] : [1];

  const G = mats.galv();
  const GR = plantMats.grate();
  const Y = mats.yellow();
  const D = mats.dark();
  const S = mats.steel();
  const CON = mats.concrete();
  const BELT = plantMats.belt();
  const DRV = plantMats.drive();
  const lens = lenses();

  const st = stationsOf(segs, 1.0);
  const root = new THREE.Group(); // static parts, baked at the end
  const rolls: THREE.Matrix4[] = [];
  const m4 = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const qz = new THREE.Quaternion();
  const sc = new THREE.Vector3();
  const tmp = { p: new THREE.Vector3(), X: new THREE.Vector3(), Y: new THREE.Vector3(), f: new THREE.Vector3() };
  const at = (s: number) => sampleStn(st, s, { p: new THREE.Vector3(), X: new THREE.Vector3(), Y: new THREE.Vector3(), f: new THREE.Vector3() });
  const local = (fr: { p: V3; X: V3; Y: V3 }, x: number, y: number) => fr.p.clone().addScaledVector(fr.X, x).addScaledVector(fr.Y, y);
  const addRoll = (fr: { p: V3; X: V3; Y: V3; f: V3 }, x: number, y: number, len: number, roll: number) => {
    const basis = new THREE.Matrix4().makeBasis(fr.X, fr.Y, fr.f);
    q.setFromRotationMatrix(basis);
    qz.setFromAxisAngle(new THREE.Vector3(0, 0, 1), roll);
    q.multiply(qz);
    sc.set(len, 1, 1);
    m4.compose(local(fr, x, y), q, sc);
    rolls.push(m4.clone());
  };

  // ── belts
  const beltProfile = (S0: Stn): [number, number][] => {
    const th = thetaAt(S0.s);
    const ex = c / 2 + wl * Math.cos(th);
    const ey = wl * Math.sin(th);
    return [
      [-ex - 0.01, ey + tb],
      [-c / 2, tb],
      [c / 2, tb],
      [ex + 0.01, ey + tb],
      [ex + 0.01, ey],
      [c / 2, 0],
      [-c / 2, 0],
      [-ex - 0.01, ey],
    ];
  };
  root.add(new THREE.Mesh(sweep(st, beltProfile, { closed: true, vPeriod: 4 }), BELT));
  const retProfile = (): [number, number][] => [
    [-w / 2, yRet],
    [w / 2, yRet],
    [w / 2, yRet - tb],
    [-w / 2, yRet - tb],
  ];
  root.add(new THREE.Mesh(sweep(st, retProfile, { closed: true, vPeriod: 4 }), BELT));

  // ── per-segment steelwork in segment frames
  const ylwOrGalv = gantry ? G : Y;
  segs.forEach((g) => {
    const fr = frameNode(root, g.a, g.X, g.Y, g.f);
    const L = g.len;
    // stringers (channels: web + flanges)
    for (const s of [-1, 1]) {
      box(fr, G, 0.012, 0.3, L, s * xs, (yStrTop + yStrBot) / 2, L / 2, { r: 0 });
      box(fr, G, 0.09, 0.012, L, s * (xs - 0.04), yStrTop - 0.006, L / 2, { r: 0 });
      box(fr, G, 0.09, 0.012, L, s * (xs - 0.04), yStrBot + 0.006, L / 2, { r: 0 });
    }
    // carrying idler sets every 1.2 m
    const n = Math.max(1, Math.round(L / 1.2));
    for (let i = 0; i < n; i++) {
      const z = ((i + 0.5) * L) / n;
      const sAbs = g.s0 + z;
      const th = thetaAt(sAbs);
      const set = node(fr, 0, 0, z);
      box(set, G, 2 * xs + 0.12, 0.07, 0.09, 0, yStrTop + 0.035, 0, { r: 0 });
      box(set, G, 2 * xs + 0.12, 0.012, 0.16, 0, yStrTop + 0.07, 0, { r: 0 });
      for (const s of [-1, 1]) {
        box(set, D, 0.012, 0.16, 0.07, s * (c / 2 + 0.012), -R - 0.03, 0, { r: 0 });
        const ox = c / 2 + wl * Math.cos(th) + R * Math.sin(th) + 0.012;
        const oy = wl * Math.sin(th) - R * Math.cos(th);
        const hgt = oy - (yStrTop + 0.07);
        box(set, D, 0.012, hgt + 0.06, 0.07, s * ox, yStrTop + 0.07 + hgt / 2, 0, { r: 0 });
      }
      const frA = { p: g.a.clone().addScaledVector(g.f, z), X: g.X, Y: g.Y, f: g.f };
      addRoll(frA, 0, -R, c - 0.02, 0);
      const mx = c / 2 + (wl / 2) * Math.cos(th) + R * Math.sin(th);
      const my = (wl / 2) * Math.sin(th) - R * Math.cos(th);
      addRoll(frA, mx, my, wl - 0.04, th);
      addRoll(frA, -mx, my, wl - 0.04, -th);
    }
    // return idlers every 3 m, hung from the stringers
    const nr = Math.max(1, Math.round(L / 3));
    for (let i = 0; i < nr; i++) {
      const z = ((i + 0.5) * L) / nr;
      const frA = { p: g.a.clone().addScaledVector(g.f, z), X: g.X, Y: g.Y, f: g.f };
      addRoll(frA, 0, yRet - tb - R, w + 0.1, 0);
      for (const s of [-1, 1]) box(fr, D, 0.012, yStrBot - (yRet - tb - R) + 0.05, 0.08, s * (w / 2 + 0.11), (yStrBot + yRet - tb - R) / 2, z, { r: 0 });
    }
    // walkways: grating on cantilever brackets, toe board (handrails are added vertical, in the root frame)
    for (const side of walkSides) {
      const wx = gantry ? gw / 2 - 0.05 - walkIn : walkOut - walkIn;
      const xc = side * (walkIn + wx / 2);
      box(fr, GR, wx, 0.035, L, xc, yWalk - 0.0175, L / 2, { r: 0 });
      box(fr, G, 0.012, 0.08, L, side * walkIn, yWalk - 0.04, L / 2, { r: 0 });
      if (!gantry) {
        box(fr, G, 0.012, 0.1, L, side * walkOut, yWalk + 0.05, L / 2, { r: 0 });
        const nb = Math.max(1, Math.round(L / 3));
        for (let i = 0; i <= nb; i++) {
          const z = (i * L) / nb;
          box(fr, G, walkOut - xs, 0.08, 0.06, side * (xs + walkOut) / 2, yWalk - 0.08, z, { r: 0 });
          bar(fr, G, v3(side * xs, yStrBot, z), v3(side * (walkOut - 0.2), yWalk - 0.1, z), 0.022, 5);
        }
      }
    }
    // gantry truss
    if (gantry) {
      const bays = Math.max(1, Math.round(L / 3));
      const bl = L / bays;
      for (const s of [-1, 1]) {
        const x = (s * gw) / 2;
        beam(fr, G, v3(x, yChordB, 0), v3(x, yChordB, L), 0.2, 0.2, UP);
        beam(fr, G, v3(x, yChordT, 0), v3(x, yChordT, L), 0.18, 0.18, UP);
        // knee and hand rails inside the truss plane
        bar(fr, Y, v3(x - s * 0.12, yWalk + 1.1, 0), v3(x - s * 0.12, yWalk + 1.1, L), 0.025, 6);
        bar(fr, Y, v3(x - s * 0.12, yWalk + 0.55, 0), v3(x - s * 0.12, yWalk + 0.55, L), 0.022, 6);
        for (let i = 0; i <= bays; i++) {
          const z = i * bl;
          beam(fr, G, v3(x, yChordB, z), v3(x, yChordT, z), 0.14, 0.14);
          if (i < bays) {
            const up = i % 2 === 0;
            bar(fr, G, v3(x, up ? yChordB : yChordT, z), v3(x, up ? yChordT : yChordB, z + bl), 0.05, 6);
          }
        }
      }
      for (let i = 0; i <= bays; i++) {
        const z = i * bl;
        beam(fr, G, v3(-gw / 2, yChordB, z), v3(gw / 2, yChordB, z), 0.16, 0.16, UP);
        beam(fr, G, v3(-gw / 2, yChordT, z), v3(gw / 2, yChordT, z), 0.14, 0.14, UP);
        // stringer posts off the floor beams
        for (const s of [-1, 1]) box(fr, G, 0.1, yStrBot - yChordB, 0.1, s * xs, (yStrBot + yChordB) / 2, z, { r: 0 });
        if (i < bays) {
          bar(fr, G, v3(-gw / 2, yChordT, z), v3(gw / 2, yChordT, z + bl), 0.035, 5);
          bar(fr, G, v3(gw / 2, yChordT, z), v3(-gw / 2, yChordT, z + bl), 0.035, 5);
          bar(fr, G, v3(-gw / 2, yChordB, z), v3(gw / 2, yChordB, z + bl), 0.035, 5);
        }
      }
      // roof sheeting over the gantry when covered
      if (opts.covered) {
        const rf = new THREE.Mesh(corrugated(gw + 0.4, L, 0.25, 0.035), plantMats.roof());
        rf.rotation.x = -Math.PI / 2;
        rf.position.set(0, yChordT + 0.12, 0);
        fr.add(rf);
      }
    }
  });

  // ── handrails, pull-wire and walkway lights (vertical posts: in the root frame)
  if (!gantry)
    for (const side of walkSides)
      segs.forEach((g) => {
        const fa = { p: g.a, X: g.X, Y: g.Y };
        const fb = { p: g.b, X: g.X, Y: g.Y };
        const pa = local(fa, side * (walkOut - 0.03), yWalk);
        const pb = local(fb, side * (walkOut - 0.03), yWalk);
        const len = pa.distanceTo(pb);
        const n = Math.max(1, Math.ceil(len / 1.8));
        for (let i = 0; i <= n; i++) {
          const p = pa.clone().lerp(pb, i / n);
          bar(root, Y, p, p.clone().setY(p.y + 1.1), 0.026, 6);
        }
        bar(root, Y, pa.clone().setY(pa.y + 1.1), pb.clone().setY(pb.y + 1.1), 0.026, 6);
        bar(root, Y, pa.clone().setY(pa.y + 0.55), pb.clone().setY(pb.y + 0.55), 0.022, 6);
        // emergency pull-wire along the belt side on short posts, switch boxes every ~40 m
        const wa = local(fa, side * (walkIn + 0.06), yWalk);
        const wb = local(fb, side * (walkIn + 0.06), yWalk);
        const np = Math.max(1, Math.ceil(len / 3));
        for (let i = 0; i <= np; i++) {
          const p = wa.clone().lerp(wb, i / np);
          bar(root, G, p, p.clone().setY(p.y + 0.8), 0.012, 4);
        }
        bar(root, mats.red(), wa.clone().setY(wa.y + 0.75), wb.clone().setY(wb.y + 0.75), 0.005, 3);
        const ns = Math.floor(len / 40);
        for (let i = 1; i <= ns; i++) {
          const p = wa.clone().lerp(wb, i / (ns + 1));
          box(root, Y, 0.14, 0.2, 0.12, p.x, p.y + 0.72, p.z, { r: 0.015 });
        }
        // walkway light fittings on the handrail posts every ~15 m
        const nl = Math.max(1, Math.round(len / 15));
        for (let i = 0; i < nl; i++) {
          const p = pa.clone().lerp(pb, (i + 0.5) / nl);
          bar(root, G, p, p.clone().setY(p.y + 2.1), 0.03, 6);
          const lp = v3(p.x - g.X.x * side * 0.15, p.y + 2.15, p.z - g.X.z * side * 0.15);
          aim(lamp(root, D, lens.white, lp.x, lp.y, lp.z, { w: 0.36, h: 0.12, d: 0.1 }), lp.clone().addScaledVector(g.X, -side * 1.2).setY(lp.y - 1.6));
        }
      });

  // ── supports
  const footing = (p: V3) => {
    box(root, CON, 0.75, 0.45, 0.75, p.x, 0.12, p.z, { r: 0.03, wear: { mud: 1.2 } });
    box(root, G, 0.42, 0.03, 0.42, p.x, 0.36, p.z, { r: 0 });
  };
  if (!gantry) {
    const sts: number[] = [];
    segs.forEach((g, k) => {
      const n = Math.max(1, Math.round(g.len / 6));
      for (let i = k === 0 ? 0 : 1; i <= n; i++) sts.push(g.s0 + (i * g.len) / n);
    });
    for (const s of sts) {
      const fr = at(Math.min(total - 0.4, Math.max(0.4, s)));
      const tops = [-1, 1].map((sd) => local(fr, sd * xs, yStrBot));
      const h = Math.min(tops[0]!.y, tops[1]!.y);
      if (h < 0.45) continue;
      for (const t of tops) {
        beam(root, G, t, t.clone().setY(0.37), 0.16, 0.16, fr.X);
        footing(t.clone().setY(0));
      }
      beam(root, G, tops[0]!.clone().setY(tops[0]!.y - 0.08), tops[1]!.clone().setY(tops[1]!.y - 0.08), 0.12, 0.14, UP);
      // walkway leg on taller trestles
      if (h > 2.2 && walkSides.length) {
        for (const side of walkSides) {
          const wt = local(fr, side * (walkOut - 0.1), yWalk - 0.12);
          beam(root, G, wt, wt.clone().setY(0.37), 0.12, 0.12, fr.X);
          footing(wt.clone().setY(0));
          const inner = tops[side > 0 ? 1 : 0]!;
          beam(root, G, inner.clone().setY(wt.y), wt, 0.1, 0.12, UP);
        }
      }
      // ties and bracing on tall legs
      const levels = Math.floor(h / 3);
      for (let l = 1; l <= levels; l++) {
        const y = h - l * 3 < 0.6 ? 0.6 : h - l * 3;
        const a = tops[0]!.clone().setY(y);
        const b = tops[1]!.clone().setY(y);
        beam(root, G, a, b, 0.1, 0.1, UP);
        const yUp = Math.min(h, y + 3);
        bar(root, G, a, tops[1]!.clone().setY(yUp - 0.1), 0.035, 5);
        bar(root, G, b, tops[0]!.clone().setY(yUp - 0.1), 0.035, 5);
      }
    }
  } else {
    // tower trestles under the gantry at the ends and every ≤ 20 m
    const sts: number[] = [];
    segs.forEach((g, k) => {
      const n = Math.max(1, Math.ceil(g.len / 20));
      for (let i = k === 0 ? 0 : 1; i <= n; i++) sts.push(g.s0 + (i * g.len) / n);
    });
    for (const s of sts) {
      const fr = at(Math.min(total - 1.2, Math.max(1.2, s)));
      const corners: V3[] = [];
      for (const dz of [-1, 1]) for (const sd of [-1, 1]) corners.push(local(fr, (sd * gw) / 2, yChordB).addScaledVector(fr.f, dz * 1.1));
      const h = Math.min(...corners.map((p) => p.y));
      if (h < 0.5) continue;
      const splay = 0.05 * h;
      const feet = corners.map((p, i) => {
        const sd = i % 2 === 0 ? -1 : 1;
        return p.clone().setY(0.37).addScaledVector(fr.X, sd * splay);
      });
      corners.forEach((p, i) => {
        beam(root, G, p.clone().setY(p.y - 0.1), feet[i]!, 0.24, 0.24, fr.X);
        footing(feet[i]!.clone().setY(0));
      });
      const levels = Math.max(1, Math.round(h / 4));
      const lv = (i: number, t: number) => corners[i]!.clone().setY(corners[i]!.y - 0.1).lerp(feet[i]!, t);
      for (let l = 0; l < levels; l++) {
        const t0 = l / levels;
        const t1 = (l + 1) / levels;
        for (const [i, j] of [
          [0, 1],
          [2, 3],
          [0, 2],
          [1, 3],
        ] as const) {
          beam(root, G, lv(i, t1), lv(j, t1), 0.12, 0.12, UP);
          bar(root, G, lv(i, t0), lv(j, t1), 0.04, 5);
          bar(root, G, lv(j, t0), lv(i, t1), 0.04, 5);
        }
      }
    }
  }

  // ── hood over the carrying belt
  if (opts.covered && !gantry) {
    const rh = w / 2 + 0.34;
    const s0 = Math.min(5, total * 0.2);
    const s1 = total - Math.min(1.5, total * 0.1);
    const hst = st.filter((x) => x.s >= s0 && x.s <= s1);
    if (hst.length >= 2) {
      const nA = 24;
      const hoodProfile = (): [number, number][] => {
        const out: [number, number][] = [];
        for (let i = 0; i <= nA; i++) {
          const a = Math.PI - (i / nA) * Math.PI;
          out.push([Math.cos(a) * rh, 0.06 + Math.sin(a) * rh]);
        }
        for (let i = nA; i >= 0; i--) {
          const a = Math.PI - (i / nA) * Math.PI;
          out.push([Math.cos(a) * (rh - 0.015), 0.06 + Math.sin(a) * (rh - 0.015)]);
        }
        return out;
      };
      root.add(new THREE.Mesh(sweep(hst, hoodProfile, { closed: true, vPeriod: 1 }), plantMats.hood()));
      // hoops every 1.2 m
      for (let s = s0; s <= s1; s += 1.2) {
        const fr = at(s);
        const hoop = frameNode(root, fr.p, fr.X, fr.Y, fr.f);
        const tor = new THREE.Mesh(new THREE.TorusGeometry(rh + 0.035, 0.018, 4, 20, Math.PI), G);
        tor.position.y = 0.06;
        hoop.add(tor);
        for (const sd of [-1, 1]) box(hoop, G, 0.05, 0.1, 0.05, sd * (rh + 0.02), 0.02, 0, { r: 0 });
      }
    }
  }

  // ── head end: pulley, drive, discharge chute
  const anchors = { tail: new THREE.Object3D(), head: new THREE.Object3D() };
  const endF = at(total);
  const startF = at(0);
  if (opts.head !== false) {
    const hf = frameNode(root, endF.p, endF.X, endF.Y, endF.f);
    const Rh = 0.42;
    cyl(hf, BELT, Rh, w + 0.08, 0, -Rh - tb, 0, { axis: "x", seg: 28 });
    for (const sd of [-1, 1]) {
      cyl(hf, S, Rh * 0.98, 0.04, sd * (w / 2 + 0.06), -Rh - tb, 0, { axis: "x", seg: 28 });
      cyl(hf, S, 0.09, 0.6, sd * (w / 2 + 0.36), -Rh - tb, 0, { axis: "x", seg: 10 });
      box(hf, D, 0.26, 0.24, 0.42, sd * (w / 2 + 0.5), -Rh - tb - 0.04, 0, { r: 0.03 });
      // head frame posts down to the stringers / trestle
      box(hf, G, 0.2, 1.2, 0.2, sd * (w / 2 + 0.5), -Rh - 0.8, 0, { r: 0 });
    }
    // belt wrap round the front of the drum
    const wrap = new THREE.Mesh(new THREE.CylinderGeometry(Rh + tb, Rh + tb, w, 24, 1, true, -Math.PI / 2, Math.PI).rotateZ(-Math.PI / 2), BELT);
    wrap.position.set(0, -Rh - tb, 0);
    hf.add(wrap);
    // discharge hood and chute
    const hw = w + 0.7;
    box(hf, D, hw, 0.08, 2.0, 0, 0.75, 0.55, { r: 0.02 });
    for (const sd of [-1, 1]) box(hf, D, 0.06, 1.0, 2.0, sd * hw / 2, 0.27, 0.55, { r: 0.015 });
    box(hf, D, hw, 1.0, 0.06, 0, 0.27, 1.55, { r: 0.015 });
    box(hf, D, hw, 2.2, 0.06, 0, -1.3, 1.45, { rx: 0.18, r: 0.015 });
    for (const sd of [-1, 1]) box(hf, D, 0.06, 2.0, 1.4, sd * hw / 2, -1.25, 0.8, { r: 0.015 });
    box(hf, D, hw, 1.4, 0.06, 0, -1.6, 0.08, { rx: -0.25, r: 0.015 });
    // stiffeners on the chute
    for (const y of [-0.6, -1.4, -2.0]) box(hf, D, hw + 0.08, 0.06, 0.08, 0, y, 1.5 - (y + 1.3) * 0.18, { r: 0 });
    // belt scraper under the drum
    box(hf, D, w, 0.08, 0.08, 0, -Rh * 1.6, 0.25, { r: 0.01 });
    // drive on the +X side: coupling guard, right-angle gearbox, motor along the belt, baseplate
    const dx = w / 2 + 1.05;
    box(hf, Y, 0.45, 0.42, 0.5, dx - 0.15, -Rh - tb, 0, { r: 0.04 });
    box(hf, DRV, 0.7, 0.75, 0.8, dx + 0.45, -Rh - 0.05, 0, { r: 0.06 });
    cyl(hf, DRV, 0.3, 1.1, dx + 0.45, -Rh - 0.1, -1.05, { axis: "z", seg: 20 });
    for (let i = 0; i < 9; i++) cyl(hf, DRV, 0.32, 0.03, dx + 0.45, -Rh - 0.1, -0.65 - i * 0.09, { axis: "z", seg: 20 });
    cyl(hf, D, 0.27, 0.2, dx + 0.45, -Rh - 0.1, -1.72, { axis: "z", seg: 18 });
    box(hf, DRV, 0.22, 0.22, 0.3, dx + 0.45, -Rh + 0.28, -0.95, { r: 0.03 });
    box(hf, G, 1.3, 0.12, 2.6, dx + 0.35, -Rh - 0.55, -0.6, { r: 0 });
    // drive platform with rail
    deck(hf, GR, 1.7, 3.0, dx + 0.3, -Rh - 0.62, -0.6, 0.04);
    for (const z of [-2.0, 0.85]) box(hf, G, 0.12, 2.0, 0.12, dx + 1.1, -Rh - 1.6, z, { r: 0 });
    anchors.head.position.copy(local(endF, 0, -3.0).addScaledVector(endF.f, 1.0));
    // legs under the head frame
    const hl = [-1, 1].map((sd) => local(endF, sd * (w / 2 + 0.5), -Rh - 1.4));
    for (const t of hl)
      if (t.y > 0.6) {
        beam(root, G, t, t.clone().setY(0.37), 0.2, 0.2, endF.X);
        footing(t.clone().setY(0));
      }
  }
  // ── tail end: pulley with take-up, skirtboards and feed chute
  if (opts.tail !== false) {
    const tf = frameNode(root, startF.p, startF.X, startF.Y, startF.f);
    const Rt = 0.32;
    cyl(tf, BELT, Rt, w + 0.06, 0, -Rt - tb, 0, { axis: "x", seg: 24 });
    const wrap = new THREE.Mesh(new THREE.CylinderGeometry(Rt + tb, Rt + tb, w, 20, 1, true, Math.PI / 2, Math.PI).rotateZ(-Math.PI / 2), BELT);
    wrap.position.set(0, -Rt - tb, 0);
    tf.add(wrap);
    for (const sd of [-1, 1]) {
      cyl(tf, S, 0.07, 0.5, sd * (w / 2 + 0.3), -Rt - tb, 0, { axis: "x", seg: 10 });
      box(tf, D, 0.22, 0.22, 0.38, sd * (w / 2 + 0.44), -Rt - tb, 0, { r: 0.03 });
      // screw take-up rods back to the end frame
      bar(tf, S, v3(sd * (w / 2 + 0.44), -Rt - tb, -0.2), v3(sd * (w / 2 + 0.44), -Rt - tb, -1.0), 0.03, 6);
      box(tf, G, 0.16, 1.0, 0.16, sd * (w / 2 + 0.44), -Rt - 0.6, -1.05, { r: 0 });
      box(tf, G, 0.14, 0.14, 1.4, sd * (w / 2 + 0.44), -Rt - tb - 0.2, -0.4, { r: 0 });
      // skirtboards along the loading zone
      box(tf, D, 0.03, 0.5, 4.2, sd * w * 0.36, 0.33, 2.6, { r: 0.008 });
      box(tf, mats.rubber(), 0.02, 0.12, 4.2, sd * w * 0.355, 0.07, 2.6, { r: 0 });
      for (let z = 0.8; z < 4.6; z += 0.9) box(tf, D, 0.05, 0.5, 0.06, sd * (w * 0.36 + 0.04), 0.33, z, { r: 0 });
    }
    // feed chute above the loading zone
    const cw = w * 0.72 + 0.08;
    box(tf, D, cw, 1.6, 0.05, 0, 1.35, 0.55, { rx: -0.35, r: 0.01 });
    for (const sd of [-1, 1]) box(tf, D, 0.05, 1.7, 2.0, sd * cw / 2, 1.3, 1.5, { r: 0.01 });
    box(tf, D, cw, 1.2, 0.05, 0, 1.55, 2.5, { r: 0.01 });
    box(tf, D, cw + 0.1, 0.06, 2.2, 0, 2.18, 1.5, { r: 0.01 });
    anchors.tail.position.copy(local(startF, 0, 2.4).addScaledVector(startF.f, 1.5));
    const tl = [-1, 1].map((sd) => local(startF, sd * (w / 2 + 0.44), -Rt - 1.1).addScaledVector(startF.f, -1.05));
    for (const t of tl)
      if (t.y > 0.6) {
        beam(root, G, t, t.clone().setY(0.37), 0.16, 0.16, startF.X);
        footing(t.clone().setY(0));
      }
  }

  const group = new THREE.Group();
  group.name = "conveyor";
  const baked = bake(root, { mudTop: 1.4, mudBottom: 0, mud: 0.8, dust: 1.1 });
  discardTree(root);
  group.add(baked);
  group.add(anchors.tail, anchors.head);

  // ── instanced rolls (one draw call)
  const rollMesh = new THREE.InstancedMesh(rollGeometry(), mats.steel(), rolls.length);
  rolls.forEach((m, i) => rollMesh.setMatrixAt(i, m));
  rollMesh.castShadow = true;
  rollMesh.receiveShadow = true;
  rollMesh.computeBoundingSphere();
  group.add(rollMesh);

  // ── ore bed on the carrying belt (scrolling texture) and lumps riding on it
  const sLoad = Math.min(opts.tail === false ? 0.3 : 3.2, total * 0.2);
  const sEnd = total - 0.15;
  const oreSt = st.filter((x) => x.s > sLoad && x.s < sEnd);
  oreSt.unshift({ ...at(sLoad), s: sLoad });
  oreSt.push({ ...at(sEnd), s: sEnd });
  const Tm = 0.1 + w * 0.12;
  const xeOf = (th: number) => c / 2 + wl * Math.cos(th) * 0.66;
  const oreTop = (x: number, th: number, load: number, ramp: number) => {
    const xe = xeOf(th);
    const t = Math.max(0, 1 - (x / xe) ** 2);
    return troughY(x, th) + tb + 0.004 + load * ramp * Tm * t ** 0.6;
  };
  const rampAt = (s: number) => smoothstep(sLoad, sLoad + 1.6, s) * smoothstep(sEnd, sEnd - 0.6, s);
  const NX = 14;
  const map = oreTexture(pal).clone();
  const nmap = gravelNormal().clone();
  map.repeat.set(1, 1);
  const oreMat = grime(
    new THREE.MeshStandardMaterial({ map, normalMap: nmap, normalScale: new THREE.Vector2(0.9, 0.9), roughness: 0.95 }),
    { mudAmount: 0, dustAmount: 0.3, scale: 0.3, variation: 0.3 },
  );
  const buildOre = (load: number) =>
    sweep(
      oreSt,
      (S0) => {
        const th = thetaAt(S0.s);
        const xe = xeOf(th);
        const rp = rampAt(S0.s);
        const out: [number, number][] = [];
        for (let i = 0; i <= NX; i++) {
          const x = -xe + (2 * xe * i) / NX;
          out.push([x, oreTop(x, th, load, rp)]);
        }
        return out;
      },
      { vPeriod: 1.1, wear: [0, 0.25, 0] },
    );
  const oreMesh = new THREE.Mesh(buildOre(opts.load ?? 0.8), oreMat);
  oreMesh.castShadow = true;
  oreMesh.receiveShadow = true;
  group.add(oreMesh);

  const span = sEnd - sLoad - 0.4;
  const maxLumps = Math.max(1, Math.floor(span / 0.45));
  const lumpShapes = [rockGeometry(21, 1), rockGeometry(34, 1)];
  const lumpMeshes = lumpShapes.map((g) => {
    const im = new THREE.InstancedMesh(g, rockMaterial(), Math.ceil(maxLumps / 2));
    im.castShadow = true;
    im.receiveShadow = true;
    im.frustumCulled = false;
    group.add(im);
    return im;
  });
  const lumps = Array.from({ length: maxLumps }, (_, i) => {
    const size = (0.08 + r() ** 2 * 0.26) * (w / 1.2);
    const rot = new THREE.Quaternion().setFromEuler(new THREE.Euler(r() * 6.3, r() * 6.3, r() * 6.3));
    const col = new THREE.Color(pal[Math.floor(r() * pal.length)]!).offsetHSL(0, 0, (r() - 0.3) * 0.06);
    return { s0: r() * span, u: (r() * 2 - 1) * 0.72, size, rot, mesh: i % 2, slot: Math.floor(i / 2), col };
  });
  for (const l of lumps) lumpMeshes[l.mesh]!.setColorAt(l.slot, l.col);
  for (const im of lumpMeshes) if (im.instanceColor) im.instanceColor.needsUpdate = true;

  const state: ConveyorState = { speed: opts.speed ?? 3, load: opts.load ?? 0.8, running: true, lights: false };
  let travel = 0;
  let shownLoad = state.load;
  const pv = new THREE.Vector3();
  const svec = new THREE.Vector3();
  const placeLumps = () => {
    const load = clamp01(state.load);
    const n = Math.round(maxLumps * Math.min(1, load * 1.15));
    const counts = [0, 0];
    for (let i = 0; i < maxLumps; i++) {
      const l = lumps[i]!;
      if (i >= n) continue;
      const s = sLoad + 0.2 + ((((l.s0 + travel) % span) + span) % span);
      const fr = sampleStn(st, s, tmp);
      const th = thetaAt(s);
      const x = l.u * xeOf(th);
      const y = oreTop(x, th, load, rampAt(s)) + l.size * 0.18;
      pv.copy(fr.p).addScaledVector(fr.X, x).addScaledVector(fr.Y, y);
      svec.setScalar(l.size);
      m4.compose(pv, l.rot, svec);
      lumpMeshes[l.mesh]!.setMatrixAt(l.slot, m4);
      counts[l.mesh] = Math.max(counts[l.mesh]!, l.slot + 1);
    }
    lumpMeshes.forEach((im, k) => {
      im.count = counts[k]!;
      im.instanceMatrix.needsUpdate = true;
    });
  };
  const apply = () => {
    if (state.load !== shownLoad) {
      shownLoad = state.load;
      oreMesh.geometry.dispose();
      oreMesh.geometry = buildOre(clamp01(state.load));
    }
    oreMesh.visible = state.load > 0.01;
    map.offset.y = -travel / 1.1;
    nmap.offset.y = map.offset.y;
    placeLumps();
    lens.set(state.lights);
  };
  apply();
  return {
    group,
    state,
    length: total,
    lenses: lens,
    anchors,
    set(s) {
      Object.assign(state, s);
      apply();
    },
    update(dt) {
      if (!state.running || !state.speed) return;
      travel += state.speed * dt;
      apply();
    },
  };
}

// ───────────────────────────── primary crusher station ─────────────────────────────

export interface CrusherOptions {
  /** Height of the tipping platform above the surrounding ground (default 7 m). */
  platform?: number;
  /** Access ramp up to the platform (default true). */
  ramp?: boolean;
  /** Discharge conveyor leaving the bottom of the building (default true). */
  conveyor?: boolean;
  /** Wall cladding colour. */
  cladding?: string;
  ore?: RockPalette;
  seed?: number;
}

export interface CrusherState {
  /** Rock breaker: 0 parked beside the pocket, 1 reaching over it. */
  breaker: number;
  /** Building and platform floodlights. */
  lights: boolean;
  /** Discharge conveyor running. */
  running: boolean;
  /** Discharge conveyor load 0..1. */
  load: number;
}

export interface Crusher {
  group: THREE.Group;
  state: CrusherState;
  set(s: Partial<CrusherState>): void;
  update(dt: number): void;
  lenses: Lenses;
  conveyor: Conveyor | null;
  anchors: {
    /** On the platform where a haul truck's rear axle stops against the bund; rotation faces the truck away from the pocket. */
    dumpPoint: THREE.Object3D;
    /** Where a haul truck's origin goes (position and rotation) when dumping at the pocket. */
    truck: THREE.Object3D;
    /** The crusher throat at the bottom of the pocket. */
    feed: THREE.Object3D;
  };
}

/**
 * Primary gyratory crusher station. Frame: x across the pocket, +z from the pocket into the crusher
 * building, −z toward the tipping platform and ramp; ground at y = 0, platform at y = `platform`.
 */
export function makeCrusher(opts: CrusherOptions = {}): Crusher {
  const P = opts.platform ?? 7;
  const lens = lenses();
  const CON = mats.concrete();
  const JT = plantMats.joint();
  const FILL = plantMats.fill();
  const CLAD = plantMats.clad(opts.cladding);
  const ROOF = plantMats.roof();
  const FR = plantMats.frame();
  const G = mats.galv();
  const GR = plantMats.grate();
  const Y = mats.yellow();
  const D = mats.dark();
  const S = mats.steel();
  const LIN = plantMats.liner();
  const TR = plantMats.translucent();
  const HB = plantMats.hazardBlack();
  const root = new THREE.Group();
  const fillRoot = new THREE.Group();

  // pocket opening at platform level
  const px = 7.5;
  const pz0 = -4;
  const pz1 = 3;
  const throatY = 2.2;
  // building
  const bx = 11;
  const bz0 = pz1;
  const bz1 = 19;
  const H = 30;

  // ── tipping platform: three fill blocks round the pocket, precast panel faces, compacted surface
  const pl = { x: 24, zf: -40 };
  const block = (x0: number, x1: number, z0: number, z1: number) => {
    // top surface
    const top = new THREE.Mesh(new THREE.PlaneGeometry(x1 - x0, z1 - z0).rotateX(-Math.PI / 2), FILL);
    top.position.set((x0 + x1) / 2, P, (z0 + z1) / 2);
    fillRoot.add(top);
  };
  block(-pl.x, -px, pl.zf, pz1);
  block(px, pl.x, pl.zf, pz1);
  block(-px, px, pl.zf, pz0 - 1.8);
  // the strip in front of the bund (concrete slab where wheels stop)
  box(root, CON, 2 * px, 0.3, 1.8, 0, P - 0.15, pz0 - 0.9, { r: 0.02 });
  // MSE wall faces with panel joints (sides and the building-side return walls)
  const wallFace = (a: V3, b: V3, h: number) => {
    const len = a.distanceTo(b);
    const n = Math.max(1, Math.round(len / 1.5));
    const d = b.clone().sub(a).normalize();
    const nrm = new THREE.Vector3(d.z, 0, -d.x);
    const mesh = new THREE.Mesh(new THREE.PlaneGeometry(len, h), CON);
    mesh.position.copy(a).lerp(b, 0.5).setY(h / 2);
    mesh.lookAt(mesh.position.clone().add(nrm));
    root.add(mesh);
    for (let i = 1; i < n; i++) {
      const p = a.clone().lerp(b, i / n).addScaledVector(nrm, 0.01);
      beam(root, JT, p.clone().setY(0), p.clone().setY(h), 0.03, 0.012, nrm);
    }
    for (let y = 1.5; y < h - 0.3; y += 1.5) {
      const off = nrm.clone().multiplyScalar(0.01);
      beam(root, JT, a.clone().setY(y).add(off), b.clone().setY(y).add(off), 0.012, 0.03, UP);
    }
    // coping beam
    beam(root, CON, a.clone().setY(h + 0.15).addScaledVector(nrm, -0.15), b.clone().setY(h + 0.15).addScaledVector(nrm, -0.15), 0.5, 0.3, UP);
  };
  wallFace(v3(pl.x, 0, pl.zf), v3(pl.x, 0, pz1), P); // +x side, faces +x
  wallFace(v3(-pl.x, 0, pz1), v3(-pl.x, 0, pl.zf), P); // −x side
  wallFace(v3(-bx - 0.3, 0, pz1), v3(-pl.x, 0, pz1), P); // building-side returns, facing +z
  wallFace(v3(pl.x, 0, pz1), v3(bx + 0.3, 0, pz1), P);
  // front face toward the ramp, either side of it
  const rw = 11;
  if (opts.ramp !== false) {
    wallFace(v3(-pl.x, 0, pl.zf), v3(-rw, 0, pl.zf), P);
    wallFace(v3(rw, 0, pl.zf), v3(pl.x, 0, pl.zf), P);
  } else wallFace(v3(-pl.x, 0, pl.zf), v3(pl.x, 0, pl.zf), P);
  // safety windrows along the platform edges (crushed-rock berms, ~half a truck tyre high)
  const bermRng = rng((opts.seed ?? 5) + 17);
  const bermNoise = new Noise3((opts.seed ?? 5) + 3);
  const berm = (a: V3, b: V3, h = 1.5) => {
    // rounded-trapezoid windrow with tapered ends and a lumpy crest, swept from a to b
    const d = b.clone().sub(a);
    const len = d.length();
    const f = d.clone().normalize();
    const X = new THREE.Vector3().crossVectors(UP, f).normalize();
    const prof: [number, number][] = [[-2.1, 0], [-1.5, 0.55], [-0.85, 0.92], [-0.3, 1], [0.3, 1], [0.85, 0.92], [1.5, 0.55], [2.1, 0]];
    const rows = Math.max(3, Math.round(len / 1.2));
    const pos: number[] = [];
    const idx: number[] = [];
    for (let r = 0; r <= rows; r++) {
      const t = r / rows;
      const sAlong = t * len;
      const taper = Math.min(1, Math.min(sAlong, len - sAlong) / 2.2) * 0.85 + 0.15;
      const c = a.clone().addScaledVector(f, sAlong);
      prof.forEach(([u, w], j) => {
        const lump = 1 + (bermNoise.noise(c.x * 0.6, j * 1.7, c.z * 0.6) * 0.18 + (bermRng() - 0.5) * 0.06) * w;
        const q = c.clone().addScaledVector(X, u * (0.6 + 0.4 * taper)).add(new THREE.Vector3(0, w * h * taper * lump, 0));
        pos.push(q.x, q.y, q.z);
      });
    }
    const n = prof.length;
    for (let r = 0; r < rows; r++)
      for (let j = 0; j < n - 1; j++) {
        const i0 = r * n + j;
        idx.push(i0, i0 + 1, i0 + n, i0 + n, i0 + 1, i0 + n + 1);
      }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
    g.setAttribute("uv", new THREE.Float32BufferAttribute(new Array((pos.length / 3) * 2).fill(0), 2));
    g.setIndex(idx);
    // winding: profile runs −X→+X, rows run along +f; (+X)×(+f) points down, so flip
    g.setIndex(idx.map((_, i) => idx[i - (i % 3) + (2 - (i % 3))]!));
    g.computeVertexNormals();
    fillRoot.add(new THREE.Mesh(g, FILL));
  };
  berm(v3(pl.x - 2.4, P, pl.zf + 1.5), v3(pl.x - 2.4, P, pz1 - 1));
  berm(v3(-pl.x + 2.4, P, pz1 - 1), v3(-pl.x + 2.4, P, pl.zf + 1.5));
  if (opts.ramp !== false) {
    berm(v3(-pl.x + 1, P, pl.zf + 2.4), v3(-rw - 1, P, pl.zf + 2.4));
    berm(v3(rw + 1, P, pl.zf + 2.4), v3(pl.x - 1, P, pl.zf + 2.4));
  } else berm(v3(-pl.x + 1, P, pl.zf + 2.4), v3(pl.x - 1, P, pl.zf + 2.4));
  // ramp down to the ground with fill side slopes and berms
  if (opts.ramp !== false) {
    const L = P * 10;
    const z0 = pl.zf - L;
    const top = (x: number, z: number, y: number) => v3(x, y, z);
    const slope = 1.5;
    const verts = [top(-rw, z0, 0), top(rw, z0, 0), top(rw, pl.zf, P), top(-rw, pl.zf, P)];
    const g = new THREE.BufferGeometry().setFromPoints([verts[0]!, verts[2]!, verts[1]!, verts[0]!, verts[3]!, verts[2]!]);
    g.computeVertexNormals();
    fillRoot.add(new THREE.Mesh(g, FILL));
    for (const s of [-1, 1]) {
      const a = top(s * rw, z0, 0);
      const b = top(s * rw, pl.zf, P);
      const c = top(s * (rw + P * slope), pl.zf, 0);
      const tri = s > 0 ? [a, b, c] : [a, c, b];
      const gs = new THREE.BufferGeometry().setFromPoints(tri);
      gs.computeVertexNormals();
      fillRoot.add(new THREE.Mesh(gs, FILL));
      // ramp-edge windrow, following the grade
      berm(s > 0 ? v3(rw - 1.8, 0.05, z0 + 2) : v3(-rw + 1.8, P + 0.05, pl.zf + 2.4), s > 0 ? v3(rw - 1.8, P + 0.05, pl.zf + 2.4) : v3(-rw + 1.8, 0.05, z0 + 2), 1.2);
    }
  }
  // planar uvs on the fill so the gravel normal map reads
  const fillBaked = bake(fillRoot, { mudTop: 0.5, mudBottom: 0, mud: 0.5, dust: 1 });
  fillBaked.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh) return;
    const P2 = m.geometry.getAttribute("position");
    const U = m.geometry.getAttribute("uv");
    for (let i = 0; i < P2.count; i++) U.setXY(i, P2.getX(i) / 2.2, P2.getZ(i) / 2.2 + P2.getY(i) / 2.2);
    U.needsUpdate = true;
  });
  fillRoot.traverse((o) => {
    const m = o as THREE.Mesh;
    if (m.isMesh) m.geometry.dispose();
  });

  // ── wheel-stop bund at the pocket edge (sloped face toward the trucks) with hazard banding
  const bz = pz0 - 1.8;
  const bundOutline: [number, number][] = [
    [bz, P],
    [pz0, P],
    [pz0, P + 1.6],
    [pz0 - 0.5, P + 1.6],
    [bz, P + 0.5],
  ];
  const bund = new THREE.Group();
  const shape = new THREE.Shape(bundOutline.map(([u, vv]) => new THREE.Vector2(u, vv)));
  const bg = new THREE.ExtrudeGeometry(shape, { depth: 2 * px, bevelEnabled: true, bevelSize: 0.05, bevelThickness: 0.05, bevelOffset: -0.05, bevelSegments: 1 });
  bg.applyMatrix4(new THREE.Matrix4().makeBasis(v3(0, 0, 1), v3(0, 1, 0), v3(-1, 0, 0)));
  bg.translate(px, 0, 0);
  bund.add(new THREE.Mesh(bg, CON));
  for (let i = 0; i < 10; i++) {
    if (i % 2) continue;
    const x = -px + (i + 0.5) * (2 * px / 10);
    box(bund, Y, 2 * px / 10, 0.25, 0.52, x, P + 1.47, pz0 - 0.26, { r: 0 });
    box(bund, HB, 2 * px / 10, 0.25, 0.52, x + 2 * px / 10, P + 1.47, pz0 - 0.26, { r: 0 });
  }
  root.add(bund);
  // concrete barrier walls along the pocket sides on the platform
  for (const s of [-1, 1]) box(root, CON, 1.0, 2.4, pz1 - pz0, s * (px + 0.5), P + 1.2, (pz0 + pz1) / 2, { r: 0.05 });

  // ── dump pocket: sloped liner walls down to the throat, wear plates with seams
  const pocket = new THREE.Group();
  const tx = 3.2;
  const tz0 = -0.8;
  const tz1 = 2.2;
  const quad = (a: V3, b: V3, c: V3, d: V3, mat: THREE.Material) => {
    const g = new THREE.BufferGeometry().setFromPoints([a, b, c, a, c, d]);
    g.computeVertexNormals();
    pocket.add(new THREE.Mesh(g, mat));
  };
  // front (truck side), sides, back wall (vertical, part of the building plinth)
  const A = v3(-px, P, pz0);
  const B = v3(px, P, pz0);
  const C = v3(px, P, pz1);
  const Dd = v3(-px, P, pz1);
  const a = v3(-tx, throatY, tz0);
  const b = v3(tx, throatY, tz0);
  const cc = v3(tx, throatY, tz1);
  const d = v3(-tx, throatY, tz1);
  quad(A, a, b, B, LIN);
  quad(B, b, cc, C, LIN);
  quad(Dd, d, a, A, LIN);
  quad(C, cc, d, Dd, CON);
  // liner plate seams (raised plates) on the front and side walls
  const plates = (p0: V3, p1: V3, p2: V3, p3: V3, nu: number, nv: number) => {
    // p0-p1 top edge, p3-p2 bottom edge
    for (let i = 0; i < nu; i++)
      for (let j = 0; j < nv; j++) {
        const lerp2 = (u: number, vv: number) => {
          const top = p0.clone().lerp(p1, u);
          const bot = p3.clone().lerp(p2, u);
          return top.lerp(bot, vv);
        };
        const g0 = 0.04;
        const q0 = lerp2(i / nu + g0 / nu, j / nv + g0 / nv);
        const q1 = lerp2((i + 1) / nu - g0 / nu, j / nv + g0 / nv);
        const q2 = lerp2((i + 1) / nu - g0 / nu, (j + 1) / nv - g0 / nv);
        const q3 = lerp2(i / nu + g0 / nu, (j + 1) / nv - g0 / nv);
        const n = new THREE.Vector3().subVectors(q1, q0).cross(new THREE.Vector3().subVectors(q3, q0)).normalize().multiplyScalar(-0.05);
        const pg = new THREE.BufferGeometry().setFromPoints([q0, q2, q1, q0, q3, q2].map((p) => p.clone().add(n)));
        pg.computeVertexNormals();
        pocket.add(new THREE.Mesh(pg, LIN));
      }
  };
  plates(A, B, b, a, 8, 4);
  plates(B, C, cc, b, 4, 4);
  plates(Dd, A, a, d, 4, 4);
  // crusher top in the throat: spider cap and arms over the mantle
  sphere(pocket, D, 1.6, 0, throatY - 0.4, (tz0 + tz1) / 2, { seg: 20, sy: 0.55 });
  cyl(pocket, S, 0.55, 0.5, 0, throatY + 0.35, (tz0 + tz1) / 2, { seg: 16 });
  box(pocket, D, 6.6, 0.6, 0.7, 0, throatY + 0.05, (tz0 + tz1) / 2, { r: 0.06 });
  box(pocket, D, 0.7, 0.6, 3.4, 0, throatY + 0.05, (tz0 + tz1) / 2, { r: 0.06 });
  root.add(pocket);

  // ── crusher building: concrete plinth, steel frame, corrugated cladding, roof, windows, doors
  const bw = 2 * bx;
  const bd = bz1 - bz0;
  const plinth = P + 6; // concrete up to the top of the rock-box wall
  box(root, CON, bw, plinth, 0.6, 0, plinth / 2, bz0 + 0.3, { r: 0.04 });
  box(root, CON, 0.6, P + 1, bd, -bx + 0.3, (P + 1) / 2, (bz0 + bz1) / 2, { r: 0.04 });
  box(root, CON, 0.6, P + 1, bd, bx - 0.3, (P + 1) / 2, (bz0 + bz1) / 2, { r: 0.04 });
  box(root, CON, bw, P + 1, 0.6, 0, (P + 1) / 2, bz1 - 0.3, { r: 0.04 });
  // cladding: front above the rock box, sides and back above the plinth
  const clad0 = P + 1;
  panel(root, CLAD, bw, H - plinth, 0, plinth, bz0 - 0.02, Math.PI);
  panel(root, CLAD, bw, H - clad0, 0, clad0, bz1 + 0.02, 0);
  panel(root, CLAD, bd, H - clad0, bx + 0.02, clad0, (bz0 + bz1) / 2, Math.PI / 2);
  panel(root, CLAD, bd, H - clad0, -bx - 0.02, clad0, (bz0 + bz1) / 2, -Math.PI / 2);
  // corner trims, girt lines, eave gutters
  for (const [x, z] of [
    [-bx, bz0],
    [bx, bz0],
    [-bx, bz1],
    [bx, bz1],
  ] as const)
    box(root, FR, 0.32, H - clad0 + 0.3, 0.32, x, (H + clad0) / 2, z, { r: 0.03 });
  for (const s of [-1, 1]) box(root, FR, 0.25, 0.3, bd + 0.6, s * (bx + 0.1), H + 0.15, (bz0 + bz1) / 2, { r: 0.03 });
  // translucent light band near the top of the long walls and a window strip lower down
  for (const s of [-1, 1]) {
    box(root, TR, 0.06, 1.4, bd - 1.2, s * (bx + 0.06), H - 2.6, (bz0 + bz1) / 2, { r: 0.01 });
    for (let i = 0; i < 4; i++) {
      const z = bz0 + 2.5 + i * 3.6;
      box(root, FR, 0.1, 1.3, 2.2, s * (bx + 0.06), P + 9, z, { r: 0.02 });
      box(root, mats.glass(), 0.1, 1.1, 2.0, s * (bx + 0.08), P + 9, z, { r: 0.01 });
    }
  }
  box(root, TR, bw - 1.2, 1.4, 0.06, 0, H - 2.6, bz1 + 0.06, { r: 0.01 });
  // gable roof with a ridge vent, corrugations running down the slope
  const pitch = 0.12;
  const half = bd / 2 + 0.6;
  const rise = half * Math.tan(pitch);
  for (const s of [-1, 1]) {
    const rf = new THREE.Mesh(corrugated(bw + 0.8, half / Math.cos(pitch), 0.25, 0.035), ROOF);
    rf.rotation.order = "YXZ";
    rf.rotation.y = s > 0 ? 0 : Math.PI;
    rf.rotation.x = -Math.PI / 2 + pitch;
    rf.position.set(0, H + 0.3, (bz0 + bz1) / 2 + s * half);
    root.add(rf);
  }
  box(root, ROOF, bw + 0.8, 0.5, 1.2, 0, H + 0.3 + rise + 0.2, (bz0 + bz1) / 2, { r: 0.05 });
  // gable ends
  for (const z of [bz0 - 0.03, bz1 + 0.03]) {
    const gg = new THREE.BufferGeometry().setFromPoints([v3(-bx, H + 0.3, z), v3(0, H + 0.3 + rise, z), v3(bx, H + 0.3, z)]);
    gg.computeVertexNormals();
    root.add(new THREE.Mesh(gg, CLAD));
    const gg2 = new THREE.BufferGeometry().setFromPoints([v3(-bx, H + 0.3, z), v3(bx, H + 0.3, z), v3(0, H + 0.3 + rise, z)]);
    gg2.computeVertexNormals();
    root.add(new THREE.Mesh(gg2, CLAD));
    box(root, CLAD, bw, 0.32, 0.1, 0, H + 0.15, z, { r: 0 });
  }
  // roller door and personnel doors at ground level on the +x side, and on the back for the conveyor
  box(root, FR, 0.2, 5.6, 5.6, bx + 0.15, 2.8, bz1 - 5, { r: 0.03 });
  const rd = new THREE.Mesh(corrugated(5.0, 5.0, 0.12, 0.02), plantMats.clad("#9aa29f"));
  rd.rotation.set(0, Math.PI / 2, Math.PI / 2);
  rd.position.set(bx + 0.3, 2.5, bz1 - 5);
  root.add(rd);
  box(root, D, 0.4, 0.5, 5.4, bx + 0.35, 5.3, bz1 - 5, { r: 0.04 });
  for (const y of [0, P + 1, H - 2.4]) {
    box(root, FR, 0.15, 2.3, 1.1, -bx - 0.08, y + 1.15, bz0 + 4, { r: 0.02 });
    box(root, S, 0.06, 0.04, 0.16, -bx - 0.18, y + 1.1, bz0 + 4.35, { r: 0.01 });
  }
  box(root, FR, 0.15, 2.3, 1.1, bx + 0.08, 1.15, bz1 - 9.5, { r: 0.02 });
  box(root, S, 0.06, 0.04, 0.16, bx + 0.18, 1.1, bz1 - 9.15, { r: 0.01 });
  // the conveyor opening at the back
  box(root, D, 3.8, 3.6, 0.3, 0, 1.8, bz1 + 0.05, { r: 0.04 });

  // ── overhead crane: runway beams out over the pocket under a canopy, bridge girder and hoist
  const craneY = P + 19;
  const rx = 7.9;
  const czF = pz0 - 1.6; // front portal line of the canopy and crane runway
  for (const s of [-1, 1]) beam(root, FR, v3(s * rx, craneY, czF), v3(s * rx, craneY, bz1 - 0.5), 0.5, 0.9, UP, 0.03);
  // canopy over the pocket, sloping up slightly from the building front to the portal
  const canL = pz1 - czF + 0.6;
  for (const gq of twoSided(corrugated(bw + 0.4, canL, 0.25, 0.035))) {
    const can = new THREE.Mesh(gq, ROOF);
    can.rotation.x = -Math.PI / 2 + 0.05;
    can.position.set(0, craneY + 2.4, pz1);
    root.add(can);
  }
  const canY = craneY + 2.4 + Math.sin(0.05) * canL;
  // portal: two columns on footings on the platform, a header carrying the runway and the canopy edge
  for (const s of [-1, 1]) {
    const x = s * (px + 1.2);
    beam(root, FR, v3(x, P + 0.4, czF), v3(x, canY - 0.3, czF), 0.6, 0.6, v3(0, 0, 1), 0.03);
    box(root, CON, 1.4, 0.8, 1.4, x, P + 0.4, czF, { r: 0.05 });
    // knee braces and canopy rafters back to the building
    bar(root, FR, v3(x, craneY - 3.5, czF), v3(x - s * 2.5, craneY - 0.6, czF), 0.12, 6);
    beam(root, FR, v3(s * bx, canY - 0.2, czF + 0.2), v3(s * bx, craneY + 2.25, pz1), 0.35, 0.55, UP, 0.02);
  }
  beam(root, FR, v3(-px - 1.5, craneY - 0.55, czF), v3(px + 1.5, craneY - 0.55, czF), 0.5, 0.7, UP, 0.03);
  beam(root, FR, v3(-bx, canY - 0.25, czF + 0.1), v3(bx, canY - 0.25, czF + 0.1), 0.35, 0.6, UP, 0.02);
  // crane bridge (yellow box girders) with trolley and hook block, parked over the pocket
  const cz = pz0 + 2.5;
  for (const dz of [-0.9, 0.9]) beam(root, Y, v3(-rx - 0.4, craneY + 0.75, cz + dz), v3(rx + 0.4, craneY + 0.75, cz + dz), 0.45, 1.0, UP, 0.04);
  for (const s of [-1, 1]) box(root, Y, 0.7, 0.8, 2.6, s * rx, craneY + 0.85, cz, { r: 0.05 });
  box(root, D, 2.4, 1.1, 2.4, 1.5, craneY + 1.7, cz, { r: 0.06 });
  for (const dx of [-0.25, 0.25]) bar(root, S, v3(1.5 + dx, craneY + 1.2, cz), v3(1.5 + dx, craneY - 5.5, cz), 0.025, 5);
  box(root, Y, 0.9, 1.0, 0.5, 1.5, craneY - 6.0, cz, { r: 0.08 });
  // ── stair tower on the −x side up to the platform level and to the roof
  const sx = -bx - 2.2;
  const flight = 3.4;
  const nFlights = Math.ceil(H / flight);
  const tower = new THREE.Group();
  for (let i = 0; i < nFlights; i++) {
    const y0 = i * flight;
    const y1 = Math.min(H, y0 + flight);
    const up = i % 2 === 0;
    const z0 = bz0 + (up ? 5 : 10);
    const z1 = bz0 + (up ? 10 : 5);
    const sb = v3(sx + (up ? -0.55 : 0.55), y0, z0);
    const stp = v3(sx + (up ? -0.55 : 0.55), y1, z1);
    // stairs helper builds treads/stringers/rails between bottom and top tread points
    stairsFlight(tower, G, GR, Y, sb, stp);
    deck(tower, GR, 2.4, 1.6, sx, y1, z1 + (up ? 0.8 : -0.8), 0.05);
  }
  // tower columns and bracing
  for (const dx of [-1.25, 1.25])
    for (const z of [bz0 + 2.9, bz0 + 12.1]) {
      beam(tower, G, v3(sx + dx, 0.3, z), v3(sx + dx, H + 1.1, z), 0.18, 0.18, v3(1, 0, 0));
      box(tower, CON, 0.7, 0.4, 0.7, sx + dx, 0.1, z, { r: 0.03 });
    }
  for (let y = 3.4; y <= H; y += 6.8)
    for (const dx of [-1.25, 1.25]) {
      bar(tower, G, v3(sx + dx, y - 3.4, bz0 + 2.9), v3(sx + dx, y + 3.4, bz0 + 12.1), 0.04, 5);
      bar(tower, G, v3(sx + dx, y + 3.4, bz0 + 2.9), v3(sx + dx, y - 3.4, bz0 + 12.1), 0.04, 5);
    }
  // outer handrails of the stair tower landings
  root.add(tower);
  // walkway from the tower into the building at platform level and roof level
  for (const y of [P + 1 + 0.0, H]) {
    deck(root, GR, 1.4, 1.2, -bx - 0.7, y, bz0 + 4, 0.05);
  }

  // ── dust extraction: hood over the pocket's back wall, duct up and round to a baghouse on the +x side
  const DU = paint("#a9b1ad", { rough: 0.45, coat: 0.3 });
  box(root, DU, 6, 1.6, 1.4, 0, P + 4.6, bz0 - 0.75, { r: 0.08 });
  const duct = [v3(3.5, P + 4.6, bz0 - 0.8), v3(bx + 2.4, P + 4.6, bz0 - 0.8), v3(bx + 2.4, P + 4.6, bz0 + 4.5), v3(bx + 2.4, 9.5, bz0 + 7.5), v3(bx + 3.2, 9.5, bz0 + 9.5)];
  for (let i = 0; i < duct.length - 1; i++) {
    bar(root, DU, duct[i]!, duct[i + 1]!, 0.6, 18, undefined, true);
    const n = Math.floor(duct[i]!.distanceTo(duct[i + 1]!) / 3);
    for (let k = 1; k <= n; k++) {
      const p = duct[i]!.clone().lerp(duct[i + 1]!, k / (n + 1));
      const dir = duct[i + 1]!.clone().sub(duct[i]!);
      const fl = new THREE.Mesh(new THREE.CylinderGeometry(0.66, 0.66, 0.08, 18).applyQuaternion(new THREE.Quaternion().setFromUnitVectors(UP, dir.normalize())), DU);
      fl.position.copy(p);
      root.add(fl);
    }
  }
  for (const p of duct.slice(1, -1)) sphere(root, DU, 0.6, p.x, p.y, p.z, { seg: 14 });
  // baghouse: housing on legs with hoppers, fan and stack
  const bhx = bx + 5.2;
  const bhz = bz0 + 9.5;
  box(root, DU, 4.2, 5.0, 5.0, bhx, 9.5, bhz, { r: 0.08 });
  box(root, DU, 4.6, 0.6, 5.4, bhx, 12.2, bhz, { r: 0.05 });
  for (const dz of [-1.2, 1.2]) {
    const hop = new THREE.Mesh(new THREE.CylinderGeometry(1.35, 0.25, 2.6, 4, 1).rotateY(Math.PI / 4), DU);
    hop.position.set(bhx, 5.7, bhz + dz);
    root.add(hop);
    cyl(root, S, 0.18, 1.0, bhx, 3.9, bhz + dz, { seg: 10 });
  }
  for (const dx of [-1.9, 1.9]) for (const dz of [-2.3, 2.3]) beam(root, FR, v3(bhx + dx, 0.2, bhz + dz), v3(bhx + dx, 7.1, bhz + dz), 0.22, 0.22, v3(1, 0, 0));
  box(root, DU, 1.5, 1.6, 1.8, bhx + 3.0, 1.0, bhz + 3.6, { r: 0.08 });
  cyl(root, DU, 0.45, 9, bhx + 3.0, 6.3, bhz + 3.6, { seg: 16 });
  bar(root, DU, v3(bhx, 7.4, bhz + 2.4), v3(bhx + 3.0, 1.6, bhz + 3.6), 0.4, 14);
  // ladder and handrail on the baghouse roof
  rail2(root, Y, [v3(bhx - 2.2, 12.5, bhz - 2.6), v3(bhx + 2.2, 12.5, bhz - 2.6), v3(bhx + 2.2, 12.5, bhz + 2.6), v3(bhx - 2.2, 12.5, bhz + 2.6)]);

  // ── floodlights: on the building front aimed at the pocket and on poles at the platform corners
  for (const s of [-1, 1]) {
    aim(lamp(root, D, lens.white, s * 4, P + 14, bz0 - 0.4, { w: 0.6, h: 0.45, d: 0.2 }), v3(0, P, pz0 - 2));
    const pole = v3(s * (pl.x - 3), P, -20);
    cyl(root, G, 0.14, 14, pole.x, P + 7, pole.z, { seg: 10, r2: 0.09 });
    box(root, CON, 1.0, 0.5, 1.0, pole.x, P + 0.25, pole.z, { r: 0.04 });
    for (const dx of [-0.5, 0.5]) aim(lamp(root, D, lens.white, pole.x + dx, P + 14.2, pole.z + 0.2, { w: 0.55, h: 0.4 }), v3(dx * 8, P, -6));
    box(root, G, 1.4, 0.1, 0.1, pole.x, P + 14.0, pole.z, { r: 0 });
  }
  // the rotating beacon on the pocket wall (amber) and a red/green dump signal
  // dump signal (red/amber) on a post at the bund end, facing the reversing truck
  box(root, D, 0.4, 0.9, 0.3, -px - 0.5, P + 3.0, pz0 - 1.2, { r: 0.04 });
  bar(root, G, v3(-px - 0.5, P, pz0 - 1.0), v3(-px - 0.5, P + 2.6, pz0 - 1.0), 0.05, 8);
  lamp(root, D, lens.red, -px - 0.5, P + 3.2, pz0 - 1.36, { w: 0.26, h: 0.26, yaw: Math.PI, round: true });
  lamp(root, D, lens.amber, -px - 0.5, P + 2.85, pz0 - 1.36, { w: 0.26, h: 0.26, yaw: Math.PI, round: true });

  // ── electrical room (E-house) on piers behind the building, A/C units, cable ladder, transformer yard
  {
    const ex0 = -11.8;
    const ex1 = -3.6;
    const ez0 = bz1 + 1.6;
    const ez1 = bz1 + 5.4;
    const ey = 1.1;
    const eh = 3.4;
    const EH = plantMats.clad("#d6d2c4");
    const ecx = (ex0 + ex1) / 2;
    const ecz = (ez0 + ez1) / 2;
    for (const x of [ex0 + 0.4, ecx, ex1 - 0.4]) for (const z of [ez0 + 0.4, ez1 - 0.4]) box(root, CON, 0.5, ey, 0.5, x, ey / 2, z, { r: 0.02 });
    beam(root, FR, v3(ex0, ey + 0.15, ecz), v3(ex1, ey + 0.15, ecz), 0.25, 0.3, UP);
    box(root, FR, ex1 - ex0 + 0.1, 0.3, ez1 - ez0 + 0.1, ecx, ey + 0.15, ecz, { r: 0.02 });
    // walls: ribbed panels on the long faces, plain ends
    panel(root, EH, ex1 - ex0, eh, ecx, ey + 0.3, ez1, 0, 0.6, 0.025);
    panel(root, EH, ex1 - ex0, eh, ecx, ey + 0.3, ez0, Math.PI, 0.6, 0.025);
    panel(root, EH, ez1 - ez0, eh, ex0, ey + 0.3, ecz, -Math.PI / 2, 0.6, 0.025);
    panel(root, EH, ez1 - ez0, eh, ex1, ey + 0.3, ecz, Math.PI / 2, 0.6, 0.025);
    box(root, EH, ex1 - ex0 + 0.3, 0.18, ez1 - ez0 + 0.3, ecx, ey + 0.3 + eh + 0.09, ecz, { r: 0.03 });
    for (const x of [ex0, ex1]) for (const z of [ez0, ez1]) box(root, FR, 0.12, eh, 0.12, x, ey + 0.3 + eh / 2, z, { r: 0 });
    // door with landing and steps at the −x end
    box(root, D, 0.06, 2.2, 1.0, ex0 - 0.04, ey + 0.3 + 1.1, ecz, { r: 0.01 });
    box(root, S, 0.05, 0.04, 0.14, ex0 - 0.09, ey + 0.3 + 1.05, ecz + 0.36, { r: 0.01 });
    deck(root, GR, 1.4, 1.6, ex0 - 0.75, ey + 0.3, ecz, 0.05);
    stairsFlight(root, FR, GR, Y, v3(ex0 - 0.75, 0, ecz - 0.8 - (ey + 0.3) / 0.75), v3(ex0 - 0.75, ey + 0.3, ecz - 0.8), 0.9);
    rail2(root, Y, [v3(ex0 - 1.45, ey + 0.3, ecz - 0.8 + 0.9), v3(ex0 - 1.45, ey + 0.3, ecz + 0.8), v3(ex0 - 0.05, ey + 0.3, ecz + 0.8)]);
    // split-system A/C units on brackets along the back face
    for (const x of [ex0 + 1.8, ex0 + 4.2, ex1 - 1.4]) {
      box(root, mats.white(), 0.9, 0.75, 0.38, x, ey + 1.6, ez1 + 0.3, { r: 0.04 });
      cyl(root, D, 0.26, 0.04, x - 0.12, ey + 1.6, ez1 + 0.5, { axis: "z", seg: 16 });
      for (const dx of [-0.3, 0.3]) box(root, G, 0.05, 0.05, 0.42, x + dx, ey + 1.2, ez1 + 0.24, { r: 0 });
      bar(root, mats.hose(), v3(x + 0.35, ey + 1.6, ez1 + 0.12), v3(x + 0.35, ey + 2.9, ez1 + 0.06), 0.03, 6);
    }
    // louvres on the −x end
    for (let i = 0; i < 6; i++) box(root, G, 0.05, 0.04, 1.2, ex0 - 0.03, ey + 2.4 + i * 0.12, ecz - 1.2, { r: 0, rx: 0.5 });
    // cable ladder from the roof of the E-house to the building wall
    const ct0 = v3(ecx + 2, ey + eh + 0.6, ez0 + 0.5);
    const ct1 = v3(ecx + 2, ey + eh + 0.6, bz1 + 0.1);
    for (const dx of [-0.3, 0.3]) beam(root, G, ct0.clone().setX(ct0.x + dx), ct1.clone().setX(ct1.x + dx), 0.03, 0.1, UP);
    for (let z = ct1.z + 0.3; z < ct0.z; z += 0.3) box(root, G, 0.6, 0.02, 0.03, ct0.x, ct0.y - 0.04, z, { r: 0 });
    for (let i = 0; i < 4; i++) bar(root, D, v3(ct0.x - 0.18 + i * 0.12, ct0.y, ct1.z), v3(ct0.x - 0.18 + i * 0.12, ct0.y, ct0.z), 0.028, 6);
    bar(root, FR, v3(ct0.x, ey + eh + 0.5, ez0 + 0.3), v3(ct0.x, ey + eh + 0.48, ez0 + 0.3), 0.05, 6);
    // transformer on a bunded pad, behind a mesh fence
    const tx0 = ex0 - 5.2;
    const tz = ecz;
    box(root, CON, 4.2, 0.3, 4.2, tx0, 0.15, tz, { r: 0.03 });
    const TF = paint("#7b867a", { rough: 0.55, coat: 0.2 });
    box(root, TF, 1.8, 2.0, 1.4, tx0, 1.3, tz, { r: 0.05 });
    for (const s of [-1, 1]) for (let i = 0; i < 7; i++) box(root, TF, 0.04, 1.6, 0.5, tx0 - 0.7 + i * 0.23, 1.2, tz + s * 0.95, { r: 0 });
    box(root, TF, 1.6, 0.35, 0.6, tx0, 2.55, tz - 0.3, { r: 0.04 });
    for (let i = 0; i < 3; i++) {
      cyl(root, mats.white(), 0.08, 0.55, tx0 - 0.5 + i * 0.5, 2.6, tz + 0.3, { seg: 10 });
      for (let k = 0; k < 4; k++) cyl(root, mats.white(), 0.12, 0.04, tx0 - 0.5 + i * 0.5, 2.45 + k * 0.12, tz + 0.3, { seg: 10 });
    }
    box(root, D, 1.0, 0.6, 0.25, tx0 + 0.95, 1.3, tz, { r: 0.03, ry: Math.PI / 2 });
    const fz0 = tz - 2.6;
    const fz1 = tz + 2.6;
    const fx0 = tx0 - 2.6;
    const fx1 = tx0 + 2.6;
    const fence = [v3(fx1, 0, fz0), v3(fx0, 0, fz0), v3(fx0, 0, fz1), v3(fx1, 0, fz1)];
    for (let i = 0; i < fence.length - 1; i++) {
      const a = fence[i]!;
      const b = fence[i + 1]!;
      const n = Math.max(1, Math.round(a.distanceTo(b) / 2.5));
      for (let k = 0; k <= n; k++) bar(root, G, a.clone().lerp(b, k / n), a.clone().lerp(b, k / n).setY(2.2), 0.035, 6);
      for (const y of [0.1, 2.15]) bar(root, G, a.clone().setY(y), b.clone().setY(y), 0.02, 4);
      const meshG = new THREE.Mesh(new THREE.PlaneGeometry(a.distanceTo(b), 2.0), plantMats.mesh());
      meshG.position.copy(a).lerp(b, 0.5).setY(1.1);
      meshG.rotation.y = Math.atan2(b.x - a.x, b.z - a.z) + Math.PI / 2;
      root.add(meshG);
    }
  }

  const group = new THREE.Group();
  group.name = "crusher";
  group.add(bake(root, { mudTop: 1.2, mudBottom: 0, mud: 0.7, dust: 1.1 }));
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (m.isMesh) m.geometry.dispose();
  });
  group.add(fillBaked);

  // ── rock breaker on a pedestal beside the pocket (swings over it)
  const brk = new THREE.Group();
  const pedX = px + 3.0;
  const pedZ = 0.2;
  {
    const ped = new THREE.Group();
    box(ped, CON, 2.6, 1.2, 2.6, pedX, P + 0.6, pedZ, { r: 0.06 });
    cyl(ped, Y, 0.9, 1.2, pedX, P + 1.8, pedZ, { seg: 22 });
    cyl(ped, D, 1.05, 0.2, pedX, P + 2.45, pedZ, { seg: 22 });
    group.add(bake(ped, { mudTop: P + 1, mudBottom: P, mud: 0.6 }));
    discardTree(ped);
  }
  const slew = new THREE.Group();
  slew.position.set(pedX, P + 2.55, pedZ);
  brk.add(slew);
  {
    const arm = new THREE.Group();
    // boom: rising from the slew ring then stick angled down over the pocket (pointing −x at swing 1)
    box(arm, Y, 1.4, 0.9, 1.4, 0, 0.45, 0, { r: 0.08 });
    beam(arm, Y, v3(0, 0.8, 0), v3(-4.8, 3.6, 0), 0.55, 0.75, v3(0, 0, 1), 0.05);
    beam(arm, Y, v3(-4.8, 3.6, 0), v3(-8.2, 0.6, 0), 0.45, 0.6, v3(0, 0, 1), 0.05);
    // cylinders: boom lift and stick
    bar(arm, Y, v3(0.3, 0.6, 0.45), v3(-2.6, 2.7, 0.45), 0.12, 10, undefined, false);
    bar(arm, mats.chrome(), v3(-2.0, 2.25, 0.45), v3(-3.3, 3.2, 0.45), 0.06, 8, undefined, false);
    bar(arm, Y, v3(-3.6, 3.95, 0.0), v3(-6.0, 3.3, 0.0), 0.1, 10, undefined, false);
    bar(arm, mats.chrome(), v3(-5.6, 3.38, 0.0), v3(-6.9, 2.6, 0.0), 0.05, 8, undefined, false);
    // hammer and chisel
    beam(arm, D, v3(-8.2, 0.7, 0), v3(-8.45, -1.0, 0), 0.5, 0.45, v3(0, 0, 1), 0.04);
    bar(arm, S, v3(-8.45, -1.0, 0), v3(-8.52, -1.6, 0), 0.09, 8, 0.03, false);
    // hoses
    for (const dz of [-0.2, 0.2]) {
      const pts = [v3(-0.5, 1.2, dz), v3(-2.5, 2.6, dz * 1.5), v3(-4.8, 3.9, dz), v3(-7.0, 2.2, dz * 1.4), v3(-8.0, 0.9, dz)];
      const curve = new THREE.CatmullRomCurve3(pts);
      arm.add(new THREE.Mesh(new THREE.TubeGeometry(curve, 30, 0.035, 6), mats.hose()));
    }
    const armBaked = bake(arm, { mudTop: -1, mudBottom: -2, mud: 0.4, dust: 1 });
    discardTree(arm);
    slew.add(armBaked);
  }
  group.add(brk);

  // ── rocks resting in the pocket and on the platform apron
  const rset = (() => {
    const out = new THREE.Group();
    const r = rng(opts.seed ?? 5);
    const lumps: { x: number; y: number; z: number; s: number; k: number; rot: THREE.Euler }[] = [];
    for (let i = 0; i < 46; i++) {
      // in the pocket: on the sloped liner surfaces near the throat
      const u = r();
      const vv = r();
      const x = (u * 2 - 1) * (tx + 1.5);
      const z = tz0 - 0.5 + vv * (tz1 - tz0 + 1);
      const y = throatY + 0.6 + Math.max(0, Math.abs(x) - tx) * 1.1 + r() * 0.6;
      lumps.push({ x, y, z, s: 0.5 + r() * 1.1, k: i % 3, rot: new THREE.Euler(r() * 6, r() * 6, r() * 6) });
    }
    for (let i = 0; i < 14; i++) {
      lumps.push({ x: (r() * 2 - 1) * px, y: P + 0.15, z: pz0 - 2 - r() * 6, s: 0.25 + r() * 0.5, k: i % 3, rot: new THREE.Euler(r() * 6, r() * 6, r() * 6) });
    }
    const pal = ROCK_PALETTES[opts.ore ?? "ore"];
    const geos = [rockGeometry(44, 2), rockGeometry(45, 2), rockGeometry(46, 1)];
    geos.forEach((g, k) => {
      const mine = lumps.filter((l) => l.k === k);
      const im = new THREE.InstancedMesh(g, rockMaterial(), mine.length);
      const mm = new THREE.Matrix4();
      mine.forEach((l, i) => {
        mm.compose(v3(l.x, l.y, l.z), new THREE.Quaternion().setFromEuler(l.rot), v3(l.s, l.s * 0.8, l.s));
        im.setMatrixAt(i, mm);
        im.setColorAt(i, new THREE.Color(pal[i % pal.length]!));
      });
      im.castShadow = im.receiveShadow = true;
      im.computeBoundingSphere();
      out.add(im);
    });
    return out;
  })();
  group.add(rset);

  // ── discharge conveyor out of the back of the building
  let conveyor: Conveyor | null = null;
  if (opts.conveyor !== false) {
    conveyor = makeConveyor(
      { points: [v3(0, 1.6, bz1 - 6), v3(0, 1.6, bz1 + 8), v3(0, 16, bz1 + 60)] },
      { width: 1.6, withGantry: false, covered: true, ore: opts.ore, load: 0.85, speed: 3.5, tail: true },
    );
    group.add(conveyor.group);
  }

  const anchors = { dumpPoint: new THREE.Object3D(), truck: new THREE.Object3D(), feed: new THREE.Object3D() };
  const axleZ = bz - 2.2; // rear tyres just touching the bund toe
  anchors.dumpPoint.position.set(0, P, axleZ);
  anchors.dumpPoint.rotation.y = Math.PI;
  anchors.truck.position.set(0, P, axleZ - 3.85);
  anchors.truck.rotation.y = Math.PI;
  anchors.feed.position.set(0, throatY, (tz0 + tz1) / 2);
  group.add(anchors.dumpPoint, anchors.truck, anchors.feed);

  const state: CrusherState = { breaker: 0, lights: false, running: true, load: 0.85 };
  const apply = () => {
    // parked: arm along −z beside the pocket; working: swung to reach over the pocket (−x)
    slew.rotation.y = (1 - clamp01(state.breaker)) * -Math.PI / 2 - 0.15;
    lens.set(state.lights);
    if (conveyor) {
      conveyor.set({ running: state.running, load: state.load, lights: state.lights });
    }
  };
  apply();
  return {
    group,
    state,
    lenses: lens,
    conveyor,
    anchors,
    set(s) {
      Object.assign(state, s);
      apply();
    },
    update(dt) {
      conveyor?.update(dt);
    },
  };
}

function discardTree(root: THREE.Object3D) {
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (m.isMesh) m.geometry.dispose();
  });
}

/** Handrail with vertical posts every ≤ 1.8 m through points (closed loop). */
function rail2(parent: THREE.Object3D, mat: THREE.Material, pts: V3[]) {
  const list = [...pts, pts[0]!];
  for (let i = 0; i < list.length - 1; i++) {
    const a = list[i]!;
    const b = list[i + 1]!;
    bar(parent, mat, a.clone().setY(a.y + 1.1), b.clone().setY(b.y + 1.1), 0.025, 6);
    bar(parent, mat, a.clone().setY(a.y + 0.55), b.clone().setY(b.y + 0.55), 0.02, 6);
    const n = Math.max(1, Math.ceil(a.distanceTo(b) / 1.8));
    for (let k = 0; k < n; k++) {
      const p = a.clone().lerp(b, k / n);
      bar(parent, mat, p, p.clone().setY(p.y + 1.1), 0.025, 6);
    }
  }
}

/** One straight stair flight between tread points (treads, stringers, both handrails). */
function stairsFlight(parent: THREE.Object3D, frame: THREE.Material, tread: THREE.Material, railMat: THREE.Material, bottom: V3, top: V3, width = 0.9) {
  const g = new THREE.Group();
  g.position.copy(bottom);
  const dx = top.x - bottom.x;
  const dz = top.z - bottom.z;
  const run = Math.hypot(dx, dz);
  const rise = top.y - bottom.y;
  g.rotation.y = Math.atan2(dx, dz);
  parent.add(g);
  const n = Math.max(2, Math.round(rise / 0.2));
  const sl = Math.hypot(run, rise);
  const ang = Math.atan2(rise, run);
  for (const s of [-1, 1]) box(g, frame, 0.05, 0.25, sl + 0.2, (s * width) / 2, rise / 2 - 0.12, run / 2, { rx: -ang, r: 0 });
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    box(g, tread, width - 0.04, 0.035, Math.max(0.25, run / n + 0.03), 0, rise * t - 0.02, run * t, { r: 0 });
  }
  for (const s of [-1, 1]) {
    const x = (s * width) / 2 + s * 0.03;
    const a = new THREE.Vector3(x, 0, 0);
    const b = new THREE.Vector3(x, rise, run);
    for (let i = 0; i <= 2; i++) {
      const q = a.clone().lerp(b, i / 2);
      bar(g, railMat, q, q.clone().setY(q.y + 1.0), 0.024, 6);
    }
    bar(g, railMat, a.clone().setY(1.0), b.clone().setY(rise + 1.0), 0.024, 6);
    bar(g, railMat, a.clone().setY(0.5), b.clone().setY(rise + 0.5), 0.02, 6);
  }
}

/** The geometry and a copy with reversed winding and normals, nudged 5 mm behind it. */
function twoSided(g: THREE.BufferGeometry): THREE.BufferGeometry[] {
  const b = g.clone();
  const ix = b.getIndex();
  if (ix) {
    const a = ix.array;
    for (let i = 0; i < a.length; i += 3) {
      const t = a[i + 1]!;
      a[i + 1] = a[i + 2]!;
      a[i + 2] = t;
    }
  }
  const n = b.getAttribute("normal");
  const p = b.getAttribute("position");
  for (let i = 0; i < n.count; i++) {
    p.setXYZ(i, p.getX(i) - n.getX(i) * 0.005, p.getY(i) - n.getY(i) * 0.005, p.getZ(i) - n.getZ(i) * 0.005);
    n.setXYZ(i, -n.getX(i), -n.getY(i), -n.getZ(i));
  }
  return [g, b];
}

/** Turns a lamp group (lens along local +z) to face a point given in its parent's frame. */
function aim(g: THREE.Object3D, target: V3) {
  g.updateMatrixWorld(true);
  g.lookAt(target);
  return g;
}
