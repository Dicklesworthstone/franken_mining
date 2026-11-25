/**
 * The services of the underground figure, hung off the real (rough) walls and backs the rock mesh draws: compressed
 * air and water lines on brackets, power cables on hooks, 1.4 m vent ducts from auxiliary fans on a messenger wire,
 * and the ground instruments (extensometer heads, convergence targets). All of it merged into two draw calls. Also
 * the air you can see moving (dust motes at the true airway speed, from the plan's airflow and drive area) and the
 * dust cones of machine headlights.
 */
import * as THREE from "three";
import { noiseTexture } from "./fleet-kit";
import { AIR, DECLINE, DRIVE_AREA, DUCT, INSTRUMENTS, L175_FACE, LEVELS, stopeMidX } from "./underground-plan";
import { backAt, ringAt, wallAt, type Network } from "./underground-rock";

const V = (x: number, y: number, z: number) => new THREE.Vector3(x, y, z);
const Y = new THREE.Vector3(0, 1, 0);

/** Accumulates lit, vertex-coloured geometry. */
class Kit {
  pos: number[] = [];
  nor: number[] = [];
  col: number[] = [];
  uv: number[] = [];
  mnt: number[] = [];
  idx: number[] = [];
  /** The inward normal of the wall the next items hang on (null: always drawn). */
  mount: THREE.Vector3 | null = null;
  private v(p: THREE.Vector3, n: THREE.Vector3, c: THREE.Color, u = 0, w = 0) {
    const m = this.mount;
    this.mnt.push(m ? m.x : 0, m ? m.y : 0, m ? m.z : 0, m ? 1 : 0);
    this.pos.push(p.x, p.y, p.z);
    this.nor.push(n.x, n.y, n.z);
    this.col.push(c.r, c.g, c.b);
    this.uv.push(u, w);
    return this.pos.length / 3 - 1;
  }
  /** A tube along a polyline (radius r, S sides); uv.x is metres along; `mounts` the wall's inward normal at each point. */
  tube(pts: THREE.Vector3[], r: number, S: number, c: THREE.Color, cap = false, mounts?: THREE.Vector3[]) {
    if (pts.length < 2) return;
    const base = this.pos.length / 3;
    const t = new THREE.Vector3();
    const a = new THREE.Vector3();
    const b = new THREE.Vector3();
    const n = new THREE.Vector3();
    const p = new THREE.Vector3();
    let along = 0;
    let prevA: THREE.Vector3 | null = null;
    for (let i = 0; i < pts.length; i++) {
      const p0 = pts[Math.max(0, i - 1)]!;
      const p1 = pts[Math.min(pts.length - 1, i + 1)]!;
      t.copy(p1).sub(p0).normalize();
      if (i > 0) along += pts[i]!.distanceTo(pts[i - 1]!);
      // A frame that turns as little as it can from the last one.
      if (!prevA) a.copy(Math.abs(t.y) > 0.9 ? new THREE.Vector3(1, 0, 0) : Y).cross(t).normalize();
      else a.copy(prevA).addScaledVector(t, -prevA.dot(t)).normalize();
      prevA = a.clone();
      b.crossVectors(t, a).normalize();
      if (mounts) this.mount = mounts[i]!;
      for (let j = 0; j <= S; j++) {
        const ang = (j / S) * Math.PI * 2;
        n.copy(a).multiplyScalar(Math.cos(ang)).addScaledVector(b, Math.sin(ang));
        p.copy(pts[i]!).addScaledVector(n, r);
        this.v(p, n, c, along, j / S);
      }
    }
    for (let i = 0; i < pts.length - 1; i++)
      for (let j = 0; j < S; j++) {
        const q = base + i * (S + 1) + j;
        const w = q + S + 1;
        this.idx.push(q, q + 1, w, q + 1, w + 1, w);
      }
    if (cap) {
      for (const end of [0, pts.length - 1]) {
        const c0 = this.v(pts[end]!, t.copy(pts[end === 0 ? 0 : end]!).sub(pts[end === 0 ? 1 : end - 1]!).normalize(), c);
        const ring = base + end * (S + 1);
        for (let j = 0; j < S; j++) this.idx.push(c0, ring + j + (end === 0 ? 1 : 0), ring + j + (end === 0 ? 0 : 1));
      }
    }
  }
  /** A box of size (sx, sy, sz) at p, its z axis along `fwd` and y up as far as it can be. */
  box(p: THREE.Vector3, sx: number, sy: number, sz: number, fwd: THREE.Vector3, c: THREE.Color, up: THREE.Vector3 = Y) {
    const z = fwd.clone().normalize();
    const x = new THREE.Vector3().crossVectors(up, z);
    if (x.lengthSq() < 1e-6) x.set(1, 0, 0);
    x.normalize();
    const y = new THREE.Vector3().crossVectors(z, x).normalize();
    const faces: [THREE.Vector3, THREE.Vector3, THREE.Vector3, number, number, number][] = [
      [x, y, z, sx, sy, sz],
      [x.clone().negate(), y, z.clone().negate(), sx, sy, sz],
      [y, z, x, sy, sz, sx],
      [y.clone().negate(), z, x.clone().negate(), sy, sz, sx],
      [z, x, y, sz, sx, sy],
      [z.clone().negate(), x, y.clone().negate(), sz, sx, sy],
    ];
    const q = new THREE.Vector3();
    for (const [n, u, w, sn, su, sw] of faces) {
      const b0 = this.pos.length / 3;
      for (const [cu, cw] of [
        [-1, -1],
        [1, -1],
        [1, 1],
        [-1, 1],
      ] as const) {
        q.copy(p)
          .addScaledVector(n, sn / 2)
          .addScaledVector(u, (cu * su) / 2)
          .addScaledVector(w, (cw * sw) / 2);
        this.v(q, n, c);
      }
      // Wound so the face looks along n.
      const flip = new THREE.Vector3().crossVectors(u, w).dot(n) < 0;
      if (flip) this.idx.push(b0, b0 + 2, b0 + 1, b0, b0 + 3, b0 + 2);
      else this.idx.push(b0, b0 + 1, b0 + 2, b0, b0 + 2, b0 + 3);
    }
  }
  geometry() {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(this.pos, 3));
    g.setAttribute("normal", new THREE.Float32BufferAttribute(this.nor, 3));
    g.setAttribute("color", new THREE.Float32BufferAttribute(this.col, 3));
    g.setAttribute("uv", new THREE.Float32BufferAttribute(this.uv, 2));
    g.setAttribute("mount", new THREE.Float32BufferAttribute(this.mnt, 4));
    g.setIndex(this.idx);
    g.computeBoundingSphere();
    return g;
  }
}

/** A tunnel's centreline with its frames and the rough ring at each point, and a running min/smooth helper. */
function frames(net: Network, id: string) {
  const l = net.lines.get(id);
  if (!l) return null;
  const rings = l.pts.map((o) => ringAt(l.spec.w, l.spec.h, o));
  const tang = l.pts.map((_, i) => l.pts[Math.min(l.pts.length - 1, i + 1)]!.clone().sub(l.pts[Math.max(0, i - 1)]!).setY(0).normalize());
  return { ...l, rings, tang };
}
/** The smallest value within ±w samples, then a moving average: a line that clears every bulge but still follows the wall. */
function clearLine(v: number[], w: number) {
  const m = v.map((_, i) => Math.min(...v.slice(Math.max(0, i - w), i + w + 1).filter(Number.isFinite)));
  return m.map((_, i) => {
    const s = m.slice(Math.max(0, i - w), i + w + 1);
    return s.reduce((a, b) => a + b, 0) / s.length;
  });
}

const C = (hex: string) => new THREE.Color(hex);
const GALV = C("#8d9296");
const WATER = C("#3f6f9a");
const STEEL = C("#5a5f63");
const CABLE = C("#d0641c");
const CABLE_B = C("#1d1d1f");
const DUCT_C = C("#9c8040");
const FAN = C("#3e5a4a");
const TARGET = C("#f0d26a");
const LOGGER = C("#c9c6bd");

/** Every drive, access and the decline get services; the decline's run stops at its face. */
function serviceLines() {
  const ids = ["decline"];
  for (const L of LEVELS) ids.push(`access${L}`, `drive${L}`);
  return ids;
}

export interface Services {
  group: THREE.Group;
  /** Where each instrument's reading head is, by id. */
  instruments: Map<string, THREE.Vector3>;
}

/** Pipes, cables, ducts, fans and instruments for the whole mine, in two merged meshes. */
export function services(net: Network, small: boolean): Services {
  const K = new Kit();
  const S = small ? 5 : 8;
  for (const id of serviceLines()) {
    const f = frames(net, id);
    if (!f) continue;
    const { pts, right, rings } = f;
    const n = pts.length;
    // Air (150 mm) at 3.2 m and water (100 mm) at 2.9 m on the right wall; power cables at 3.8 m on the left.
    const lines: { y: number; r: number; side: 1 | -1; c: THREE.Color; sag?: number }[] = [
      { y: 3.2, r: 0.075, side: 1, c: GALV },
      { y: 2.86, r: 0.055, side: 1, c: WATER },
      { y: 3.82, r: 0.03, side: -1, c: CABLE, sag: 0.14 },
      { y: 3.74, r: 0.024, side: -1, c: CABLE_B, sag: 0.16 },
    ];
    for (const ln of lines) {
      const wall = rings.map((ring) => wallAt(ring, ln.side, ln.y));
      const lat = clearLine(wall, 3).map((w) => w - 0.13 - ln.r);
      const path: THREE.Vector3[] = [];
      const mounts: THREE.Vector3[] = [];
      // Cables hang in a catenary between hooks every 3 m (every second ring), so they are sampled finer.
      const sub = ln.sag ? 4 : 1;
      for (let i = 0; i < n; i++)
        for (let k = 0; k < (i < n - 1 ? sub : 1); k++) {
          const f = k / sub;
          const j = Math.min(n - 1, i + 1);
          const u = ((i % 2) + f) / 2;
          const sag = ln.sag ? ln.sag * 4 * u * (1 - u) : 0;
          const o = pts[i]!.clone().lerp(pts[j]!, f);
          const rr = right[i]!.clone().lerp(right[j]!, f).normalize();
          path.push(o.addScaledVector(rr, ln.side * (lat[i]! + (lat[j]! - lat[i]!) * f)).add(V(0, ln.y - sag, 0)));
          mounts.push(rr.clone().multiplyScalar(-ln.side));
        }
      K.tube(path, ln.r, ln.sag ? 4 : S, ln.c, false, mounts);
      // Brackets (pipes) or hooks (cables) from the line to the wall every 3 m.
      for (let i = 1; i < n - 1; i += 2) {
        const w = wall[i]!;
        if (!Number.isFinite(w)) continue;
        const len = w - lat[i]! + 0.06;
        const mid = pts[i]!.clone().addScaledVector(right[i]!, ln.side * (lat[i]! + len / 2 - 0.02)).add(V(0, ln.y - (ln.sag ? 0 : ln.r + 0.03), 0));
        K.mount = right[i]!.clone().multiplyScalar(-ln.side);
        K.box(mid, ln.sag ? 0.018 : 0.06, ln.sag ? 0.018 : 0.05, len, right[i]!, STEEL);
      }
    }
  }

  K.mount = null;
  /* Vent ducts: 1.4 m flexible bag from an auxiliary fan, on a messenger wire, to the dead-end headings. */
  const D = new Kit();
  const R = DUCT.diameter / 2;
  const ductRun = (legs: { id: string; from: number; to: number; lat: number }[]) => {
    const path: THREE.Vector3[] = [];
    for (const leg of legs) {
      const f = frames(net, leg.id);
      if (!f) continue;
      const top = clearLine(
        f.rings.map((ring) => backAt(ring, leg.lat)),
        2,
      );
      const sel: THREE.Vector3[] = [];
      for (let i = 0; i < f.pts.length; i++) {
        const o = f.pts[i]!;
        const a = (leg.id.startsWith("drive") ? o.x : leg.id.startsWith("xc") ? -o.z : i);
        if (a < Math.min(leg.from, leg.to) || a > Math.max(leg.from, leg.to)) continue;
        sel.push(o.clone().addScaledVector(f.right[i]!, leg.lat).add(V(0, top[i]! - R - 0.18, 0)));
      }
      if (leg.from > leg.to) sel.reverse();
      path.push(...sel);
    }
    if (path.length < 2) return;
    const curve = new THREE.CatmullRomCurve3(path, false, "centripetal");
    const length = curve.getLength();
    const pts = curve.getSpacedPoints(Math.max(8, Math.round(length / 0.9)));
    D.tube(pts, R, small ? 12 : 18, DUCT_C);
    // The messenger wire over it and a hanger every 3 m.
    const wire = pts.map((p) => p.clone().add(V(0, R + 0.16, 0)));
    K.tube(wire, 0.012, 3, STEEL);
    for (let i = 0; i < pts.length; i += 3) K.box(pts[i]!.clone().add(V(0, R + 0.08, 0)), 0.04, 0.18, 0.02, V(1, 0, 0), STEEL);
    // The fan at the start: a 1.4 m casing with a bell mouth, on a stand.
    const dir = pts[1]!.clone().sub(pts[0]!).normalize();
    const f0 = pts[0]!.clone().addScaledVector(dir, -1.4);
    K.tube([f0.clone().addScaledVector(dir, -1.3), f0.clone().addScaledVector(dir, 1.3)], R + 0.08, S + 4, FAN, true);
    K.tube([f0.clone().addScaledVector(dir, -1.65), f0.clone().addScaledVector(dir, -1.3)], R + 0.28, S + 4, FAN, true);
    K.box(f0.clone().add(V(0, -R - 0.5, 0)), 0.2, 1.0, 1.6, dir, STEEL);
  };
  // 100: the access to the fired stope's draw point (XC-03), along the north side of the drive.
  ductRun([
    { id: "drive-100", from: DECLINE.cx + 6, to: stopeMidX(3) - 5, lat: -0.85 },
    { id: "xc-100-3", from: -(net.lines.get("xc-100-3")?.pts[0]!.z ?? 0) + 5, to: -(net.lines.get("xc-100-3")?.pts.at(-1)!.z ?? 0) - 7, lat: 0.85 },
  ]);
  // 175: along the drive to 10 m short of the jumbo's face.
  ductRun([{ id: "drive-175", from: DECLINE.cx + 6, to: L175_FACE - 12, lat: -0.85 }]);
  // Down the decline to its face, from just above the 175 access.
  {
    const f = net.lines.get("decline");
    if (f) {
      const n = f.pts.length;
      const tops = f.pts.map((o) => backAt(ringAt(f.spec.w, f.spec.h, o), -0.85));
      const top = clearLine(tops, 2);
      const run: THREE.Vector3[] = [];
      for (let i = Math.max(0, n - 36); i < n - 5; i++) run.push(f.pts[i]!.clone().addScaledVector(f.right[i]!, -0.85).add(V(0, top[i]! - R - 0.18, 0)));
      if (run.length > 2) {
        const curve = new THREE.CatmullRomCurve3(run, false, "centripetal");
        const pts = curve.getSpacedPoints(Math.max(8, Math.round(curve.getLength() / 0.9)));
        D.tube(pts, R, small ? 12 : 18, DUCT_C);
        K.tube(
          pts.map((p) => p.clone().add(V(0, R + 0.16, 0))),
          0.012,
          3,
          STEEL,
        );
        for (let i = 0; i < pts.length; i += 3) K.box(pts[i]!.clone().add(V(0, R + 0.08, 0)), 0.04, 0.18, 0.02, V(1, 0, 0), STEEL);
      }
    }
  }

  /* Instruments: extensometer heads in the back with a cable to a logger; convergence targets on both walls. */
  const instruments = new Map<string, THREE.Vector3>();
  for (const ins of INSTRUMENTS) {
    const id = ins.k ? `xc${ins.level}-${ins.k}` : `drive${ins.level}`;
    const f = frames(net, id);
    if (!f) continue;
    let i = 0;
    if (ins.k) i = Math.max(1, f.pts.length - 3);
    else {
      let best = Infinity;
      f.pts.forEach((p, j) => {
        const d = Math.abs(p.x - ins.x);
        if (d < best) {
          best = d;
          i = j;
        }
      });
    }
    const o = f.pts[i]!;
    const r = f.right[i]!;
    const t = f.tang[i]!;
    const ring = f.rings[i]!;
    if (ins.kind === "extensometer") {
      const top = backAt(ring, 0.4);
      const head = o.clone().addScaledVector(r, 0.4).add(V(0, top - 0.14, 0));
      K.box(head, 0.32, 0.2, 0.22, t, C("#7d8a92"));
      K.box(head.clone().add(V(0, -0.14, 0)), 0.1, 0.1, 0.1, t, C("#d8b93a"));
      const wl = wallAt(ring, -1, 1.7);
      const logger = o.clone().addScaledVector(r, -(wl - 0.14)).add(V(0, 1.7, 0));
      K.box(logger, 0.3, 0.42, 0.18, r, LOGGER, Y);
      // Its cable: up the wall from the logger and over the back to the head.
      const at = (lat: number, y: number) => o.clone().addScaledVector(r, lat).add(V(0, y, 0)).addScaledVector(t, 0.12);
      const w34 = wallAt(ring, -1, 3.4);
      const cab = [at(-(wl - 0.1), 1.95), at(-(w34 - 0.07), 3.4)];
      for (let k = 1; k <= 8; k++) {
        const lat = -(w34 - 0.07) + ((0.4 + w34 - 0.07) * k) / 8;
        cab.push(at(lat, backAt(ring, lat) - 0.07));
      }
      K.tube(cab, 0.015, 3, CABLE_B);
      instruments.set(ins.id, head);
    } else {
      for (const side of [1, -1] as const) {
        const w = wallAt(ring, side, 1.8);
        K.tube([o.clone().addScaledVector(r, side * (w + 0.02)).add(V(0, 1.8, 0)), o.clone().addScaledVector(r, side * (w - 0.1)).add(V(0, 1.8, 0))], 0.06, 8, TARGET, true);
      }
      const top = backAt(ring, 0);
      const crown = o.clone().add(V(0, top - 0.1, 0));
      K.tube([crown.clone().add(V(0, 0.12, 0)), crown], 0.06, 8, TARGET, true);
      instruments.set(ins.id, crown);
    }
  }

  const group = new THREE.Group();
  const misc = new THREE.Mesh(K.geometry(), servicesMaterial());
  misc.name = "services";
  const duct = new THREE.Mesh(D.geometry(), ductMaterial());
  duct.name = "ducts";
  duct.castShadow = !small;
  group.add(misc, duct);
  return { group, instruments };
}

/**
 * Pipes, cables and fittings. The near wall of a tunnel drops away when seen from outside, so whatever hangs on a wall
 * the camera is behind is not drawn (it would float in the air); things in the back and the instruments always are.
 */
function servicesMaterial() {
  const m = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.55, metalness: 0.35, envMapIntensity: 0.3 });
  m.customProgramCacheKey = () => "ug-services-v1";
  m.onBeforeCompile = (sh) => {
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nattribute vec4 mount;\nvarying vec4 vMount;\nvarying vec3 vSw;")
      .replace("#include <worldpos_vertex>", "#include <worldpos_vertex>\nvMount = mount;\nvSw = (modelMatrix * vec4(transformed, 1.0)).xyz;");
    sh.fragmentShader = sh.fragmentShader
      .replace("#include <common>", "#include <common>\nvarying vec4 vMount;\nvarying vec3 vSw;")
      .replace("#include <clipping_planes_fragment>", "#include <clipping_planes_fragment>\nif (vMount.w > 0.5 && dot(cameraPosition - vSw, vMount.xyz) < 0.0) discard;");
  };
  return m;
}

/** Flexible vent bag: dusty yellow fabric pulled taut over a spiral wire every 0.5 m (uv.x is metres along). */
export function ductMaterial() {
  const m = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.9, emissive: "#20160a", emissiveIntensity: 0.12 });
  const tex = noiseTexture();
  m.customProgramCacheKey = () => "ug-duct-v3";
  m.onBeforeCompile = (sh) => {
    sh.uniforms.tNoise = { value: tex };
    sh.vertexShader = sh.vertexShader.replace("#include <common>", "#include <common>\nvarying vec2 vDu;").replace("#include <uv_vertex>", "#include <uv_vertex>\nvDu = uv;");
    sh.fragmentShader = sh.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform sampler2D tNoise;\nvarying vec2 vDu;")
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
float rr = vDu.x / 0.5 + vDu.y;
float ring = abs(fract(rr) - 0.5) * 2.0;
float fw = fwidth(rr) * 2.0;
float rib = smoothstep(0.0, 0.25 + fw, ring);
diffuseColor.rgb *= mix(0.84, 1.0, mix(rib, 0.92, smoothstep(0.3, 1.0, fw)));
// Grime and wear in patches; dust settles on the top of the bag; seams every 20 m.
vec4 nz = texture2D(tNoise, vec2(vDu.x * 0.05, vDu.y * 0.7));
vec4 nf = texture2D(tNoise, vec2(vDu.x * 0.4, vDu.y * 2.0));
diffuseColor.rgb *= 0.78 + 0.3 * nz.r + 0.12 * nf.b;
diffuseColor.rgb = mix(diffuseColor.rgb, vec3(0.3, 0.25, 0.19), smoothstep(0.55, 0.8, nz.g) * 0.45);
float top = smoothstep(0.45, 0.95, cos((vDu.y - 0.25) * 6.2832));
diffuseColor.rgb = mix(diffuseColor.rgb, vec3(0.42, 0.38, 0.32), (0.3 + 0.3 * nf.a) * top);
diffuseColor.rgb *= 1.0 - 0.3 * (1.0 - smoothstep(0.05, 0.12, abs(fract(vDu.x / 20.0) - 0.5) * 20.0 - 9.8));`,
      );
  };
  return m;
}

/* ---------- Air you can see ---------- */

/** Air speed in a level's drives, m/s: the anemometer airflow over the drive's open area. */
export const airSpeed = (level: number) => (AIR.find((a) => a.level === level)?.measured ?? 0) / DRIVE_AREA;
const DECLINE_AREA = DECLINE.w * DECLINE.h * 0.93;
/** Air speed down the decline at height y: the air for every level below y, over the decline's area. */
export const declineAirSpeed = (y: number) => AIR.filter((a) => a.level < y + 0.5).reduce((n, a) => n + a.measured, 0) / DECLINE_AREA;

export interface Airflow {
  points: THREE.Points;
  update(dt: number): void;
}

/**
 * Dust motes carried by the ventilation: fresh air down the decline (slowing as each level draws its share), along
 * each level's access and footwall drive west to east to the return raise, each at the airway's true speed. They
 * catch the light near the wall lamps and fade with distance, so they only show close in.
 */
export function airflow(net: Network, small: boolean, still: boolean): Airflow {
  type Path = { pts: THREE.Vector3[]; cum: number[]; length: number; speed: (p: THREE.Vector3) => number; light: Float32Array };
  const paths: Path[] = [];
  const mk = (pts: THREE.Vector3[], speed: (p: THREE.Vector3) => number) => {
    if (pts.length < 2) return;
    const cum = [0];
    for (let i = 1; i < pts.length; i++) cum.push(cum[i - 1]! + pts[i]!.distanceTo(pts[i - 1]!));
    const length = cum[cum.length - 1]!;
    // The lamps' light along the path, every metre.
    const light = new Float32Array(Math.ceil(length) + 1);
    const q = new THREE.Vector3();
    for (let m = 0; m < light.length; m++) {
      at({ pts, cum, length } as Path, m, q, { i: 0 });
      let e = 0;
      for (const l of net.lamps) {
        const d2 = l.p.distanceToSquared(q.clone().add(V(0, 2.2, 0)));
        if (d2 < 200) e += 6 / (d2 + 3);
      }
      light[m] = Math.min(1.5, e);
    }
    paths.push({ pts, cum, length, speed, light });
  };
  const at = (p: Pick<Path, "pts" | "cum" | "length">, s: number, out: THREE.Vector3, hint: { i: number }) => {
    let i = Math.min(hint.i, p.cum.length - 2);
    while (i > 0 && p.cum[i]! > s) i--;
    while (i < p.cum.length - 2 && p.cum[i + 1]! < s) i++;
    hint.i = i;
    const a = p.pts[i]!;
    const b = p.pts[i + 1]!;
    const seg = p.cum[i + 1]! - p.cum[i]!;
    return out.copy(a).lerp(b, seg > 0 ? (s - p.cum[i]!) / seg : 0);
  };
  const dec = net.lines.get("decline");
  if (dec) {
    // To the 175 access: below it the decline is a dead end on its duct.
    const stop = dec.pts.findIndex((p) => p.y < -175.5);
    mk(dec.pts.slice(0, stop < 0 ? dec.pts.length : stop), (p) => declineAirSpeed(p.y));
  }
  for (const a of AIR) {
    const acc = net.lines.get(`access${a.level}`);
    const dr = net.lines.get(`drive${a.level}`);
    if (!acc || !dr) continue;
    const ax = acc.pts[0]!.x;
    const pts = [...acc.pts, ...dr.pts.filter((p) => p.x > ax + 1)];
    const v = airSpeed(a.level);
    mk(pts, () => v);
  }
  const per = small ? 0.9 : 2.6;
  const motes: { path: number; s: number; lat: number; up: number; ph: number; hint: { i: number } }[] = [];
  let seed = 11;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  paths.forEach((p, k) => {
    const count = Math.round(p.length * per * (k === 0 ? 0.6 : 1));
    for (let i = 0; i < count; i++) motes.push({ path: k, s: rnd() * p.length, lat: (rnd() - 0.5) * 4.4, up: 0.4 + rnd() * 4.4, ph: rnd() * 100, hint: { i: 0 } });
  });
  const n = motes.length;
  const pos = new Float32Array(n * 3);
  const lit = new Float32Array(n);
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(pos, 3).setUsage(THREE.DynamicDrawUsage));
  g.setAttribute("lit", new THREE.BufferAttribute(lit, 1).setUsage(THREE.DynamicDrawUsage));
  const mat = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    fog: true,
    uniforms: THREE.UniformsUtils.merge([THREE.UniformsLib.fog, { uScale: { value: 600 } }]),
    vertexShader: `attribute float lit;
uniform float uScale;
varying float vA;
#include <common>
#include <fog_pars_vertex>
void main() {
  vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
  float d = -mvPosition.z;
  gl_PointSize = clamp(uScale * 0.04 / d, 1.5, 7.0);
  vA = (0.3 + 1.4 * lit) * (1.0 - smoothstep(18.0, 60.0, d)) * smoothstep(0.8, 2.5, d);
  gl_Position = projectionMatrix * mvPosition;
  #include <fog_vertex>
}`,
    fragmentShader: `varying float vA;
#include <common>
#include <fog_pars_fragment>
void main() {
  vec2 c = gl_PointCoord - 0.5;
  float a = smoothstep(0.5, 0.0, length(c)) * vA;
  gl_FragColor = vec4(vec3(1.0, 0.86, 0.66) * a, a);
  #include <fog_fragment>
}`,
  });
  const points = new THREE.Points(g, mat);
  points.frustumCulled = false;
  points.renderOrder = 7;
  const q = new THREE.Vector3();
  const tan = new THREE.Vector3();
  let clock = 0;
  const place = () => {
    for (let k = 0; k < n; k++) {
      const m = motes[k]!;
      const p = paths[m.path]!;
      at(p, m.s, q, m.hint);
      const i = m.hint.i;
      tan.copy(p.pts[i + 1]!).sub(p.pts[i]!).setY(0).normalize();
      // Eddies: a slow wander across the airway.
      const w = Math.sin(clock * 0.7 + m.ph) * 0.25;
      pos[k * 3] = q.x - tan.z * (m.lat + w);
      pos[k * 3 + 1] = q.y + m.up + Math.sin(clock * 0.5 + m.ph * 1.7) * 0.15;
      pos[k * 3 + 2] = q.z + tan.x * (m.lat + w);
      lit[k] = p.light[Math.min(p.light.length - 1, Math.round(m.s))]! * (0.5 + 0.5 * Math.sin(m.ph));
    }
    g.attributes.position!.needsUpdate = true;
    g.attributes.lit!.needsUpdate = true;
  };
  place();
  return {
    points,
    update(dt) {
      mat.uniforms.uScale!.value = window.innerHeight * Math.min(2, window.devicePixelRatio || 1);
      if (still) return;
      clock += dt;
      for (const m of motes) {
        const p = paths[m.path]!;
        m.s += p.speed(at(p, m.s, q, m.hint)) * dt;
        if (m.s > p.length) {
          m.s -= p.length;
          m.hint.i = 0;
        }
      }
      place();
    },
  };
}

/* ---------- Headlight dust cones ---------- */

const shaftTime = { value: 0 };
/** Advance the dust in every light shaft (call once a frame). */
export const tickShafts = (t: number) => (shaftTime.value = t);

/**
 * A volumetric dust cone for a headlight or lamp: apex at the origin, opening along +z for `length` metres at the
 * half-angle `angle` (radians). Additive, soft at its edges and along its length, with slow drifting dust; fades when
 * seen end-on. Attach it to the light's parent and aim it like the light.
 */
export function lightShaft(opts: { length: number; angle: number; colour: string; strength?: number }): THREE.Mesh {
  const m = new THREE.Mesh(shaftGeometry(opts.length, opts.angle), shaftMaterial(opts));
  m.renderOrder = 8;
  return m;
}
function shaftGeometry(length: number, angle: number) {
  const g = new THREE.ConeGeometry(Math.tan(angle) * length, length, 28, 6, true);
  g.translate(0, -length / 2, 0);
  g.rotateX(-Math.PI / 2);
  return g;
}
/** The same dust cones, one per transform, in a single draw call (for the wall lamps). */
export function lightShafts(count: number, opts: { length: number; angle: number; colour: string; strength?: number }) {
  const m = new THREE.InstancedMesh(shaftGeometry(opts.length, opts.angle), shaftMaterial(opts), count);
  m.renderOrder = 8;
  m.frustumCulled = false;
  return m;
}
function shaftMaterial(opts: { length: number; colour: string; strength?: number }) {
  const mat = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    side: THREE.DoubleSide,
    blending: THREE.AdditiveBlending,
    fog: true,
    uniforms: THREE.UniformsUtils.merge([
      THREE.UniformsLib.fog,
      { uColour: { value: new THREE.Color(opts.colour) }, uLen: { value: opts.length }, uK: { value: opts.strength ?? 0.32 }, tNoise: { value: noiseTexture() } },
    ]),
    vertexShader: `varying vec3 vN;
varying vec3 vV;
varying vec3 vW;
varying float vZ;
varying vec3 vAxis;
#include <common>
#include <fog_pars_vertex>
void main() {
  mat4 im = mat4(1.0);
  #ifdef USE_INSTANCING
  im = instanceMatrix;
  #endif
  vec4 mvPosition = modelViewMatrix * im * vec4(position, 1.0);
  vN = normalize(normalMatrix * mat3(im) * normal);
  vV = normalize(-mvPosition.xyz);
  vW = (modelMatrix * im * vec4(position, 1.0)).xyz;
  vZ = position.z;
  vAxis = normalize((viewMatrix * vec4(mat3(modelMatrix) * mat3(im) * vec3(0.0, 0.0, 1.0), 0.0)).xyz);
  gl_Position = projectionMatrix * mvPosition;
  #include <fog_vertex>
}`,
    fragmentShader: `uniform vec3 uColour;
uniform float uLen;
uniform float uK;
uniform float uTime;
uniform sampler2D tNoise;
varying vec3 vN;
varying vec3 vV;
varying vec3 vW;
varying float vZ;
varying vec3 vAxis;
#include <common>
#include <fog_pars_fragment>
void main() {
  float u = clamp(vZ / uLen, 0.0, 1.0);
  float along = pow(1.0 - u, 1.6) * smoothstep(0.0, 0.08, u);
  float edge = pow(abs(dot(normalize(vN), vV)), 1.4);
  float dust = 0.55 + 0.9 * texture2D(tNoise, vW.xz * 0.21 + vec2(uTime * 0.013, uTime * 0.007)).b * texture2D(tNoise, vW.zy * 0.33 - vec2(uTime * 0.01, 0.0)).g;
  float endOn = 1.0 - 0.8 * pow(abs(dot(vAxis, vV)), 5.0);
  float a = along * edge * dust * endOn * uK;
  gl_FragColor = vec4(uColour * a, a);
  #include <fog_fragment>
}`,
  });
  mat.uniforms.uTime = shaftTime;
  return mat;
}
