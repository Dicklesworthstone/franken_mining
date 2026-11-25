/**
 * Pieces shared by the plant and tailings scenes: a desert ground with hills to the horizon, corrugated cladding,
 * pipes with rounded bends and their supports, round handrails, and the animated surfaces of a process plant (froth
 * spilling from a flotation cell, slurry turning in a leach tank, water in a thickener or a pond, molten gold), plus
 * the glowing flow overlay that shows which way each stream runs. Units metres, y up. No DOM access at import.
 */
import * as THREE from "three";
import { bar, grime, noiseTexture, paint, type V3 } from "./fleet-kit";
import { clamp, mix, smooth } from "./lib";
import { noise2 } from "./stage";
import { blend, rockSurfaceMaterial } from "./terrain";

const C = (hex: string) => new THREE.Color(hex);
export const SOIL = [C("#a48a68"), C("#937a5b"), C("#b09878"), C("#9c8463")];
export const GRAVEL = [C("#b4a68c"), C("#a89a80"), C("#bdb096")];
export const ROAD = C("#b8ab92");

/* ---------- Ground ---------- */

export interface GroundOptions {
  /** Centre of the detailed square, x and z. */
  centre: [number, number];
  extent: number;
  cells: number;
  /** Ground height inside the square. */
  height(x: number, z: number): number;
  /** Ground colour at a point, given its height and the up-component of its normal. */
  colour(x: number, z: number, y: number, ny: number, out: THREE.Color): void;
  /** Ground height beyond the square (defaults to a gentle plain rising into hills and ranges). */
  far?(x: number, z: number): number;
  seed?: number;
}

export interface Ground {
  group: THREE.Group;
  heightAt(x: number, z: number): number;
  material: THREE.MeshStandardMaterial;
  mesh: THREE.Mesh;
}

/** A heightfield square with vertex colours, ringed by a coarser grid out to hills on the horizon. */
export function makeGround(o: GroundOptions): Ground {
  const [cx, cz] = o.centre;
  const { extent, cells } = o;
  const n = cells + 1;
  const cell = extent / cells;
  const half = extent / 2;
  const far = noise2((o.seed ?? 5) + 11);
  const heights = new Float32Array(n * n);
  for (let j = 0; j < n; j++) for (let k = 0; k < n; k++) heights[j * n + k] = o.height(cx - half + k * cell, cz - half + j * cell);
  const heightAt = (x: number, z: number) => {
    const fx = (x - cx + half) / cell;
    const fz = (z - cz + half) / cell;
    if (fx < 0 || fz < 0 || fx > cells || fz > cells) return outer(x, z);
    const k = Math.min(cells - 1, Math.floor(fx));
    const j = Math.min(cells - 1, Math.floor(fz));
    const tx = fx - k;
    const tz = fz - j;
    return mix(mix(heights[j * n + k]!, heights[j * n + k + 1]!, tx), mix(heights[(j + 1) * n + k]!, heights[(j + 1) * n + k + 1]!, tx), tz);
  };
  const outer =
    o.far ??
    ((x: number, z: number) => {
      const r = Math.hypot(x - cx, z - cz);
      const base = o.height(clamp(x, cx - half, cx + half), clamp(z, cz - half, cz + half));
      const hills = 24 * far.fbm(x / 900, z / 900, 4) * smooth((r - half) / 900) + 10 * smooth((r - half) / 1400);
      const ranges = 300 * Math.max(0, far.fbm(x / 2600 + 4, z / 2600 - 2, 5) + 0.14) * smooth((r - 2400) / 2600);
      return base + hills + ranges;
    });

  const geo = new THREE.PlaneGeometry(extent, extent, cells, cells);
  geo.rotateX(-Math.PI / 2);
  geo.translate(cx, 0, cz);
  const pos = geo.getAttribute("position") as THREE.BufferAttribute;
  for (let i = 0; i < pos.count; i++) pos.setY(i, heights[i]!);
  geo.computeVertexNormals();
  const nrm = geo.getAttribute("normal") as THREE.BufferAttribute;
  const uv = geo.getAttribute("uv") as THREE.BufferAttribute;
  const col = new Float32Array(pos.count * 3);
  const c = new THREE.Color();
  for (let i = 0; i < pos.count; i++) {
    const x = pos.getX(i);
    const z = pos.getZ(i);
    // Fine tiling with a soft normal: at a few metres the pad reads as compacted gravel, not as cracked mud.
    uv.setXY(i, x * 0.37, z * 0.37);
    o.colour(x, z, pos.getY(i), nrm.getY(i), c);
    col[i * 3] = c.r;
    col[i * 3 + 1] = c.g;
    col[i * 3 + 2] = c.b;
  }
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
  const material = rockSurfaceMaterial();
  material.normalScale.set(0.36, 0.36);
  const mesh = new THREE.Mesh(geo, material);
  mesh.receiveShadow = true;
  const group = new THREE.Group();
  group.add(mesh);

  // The ring out to the horizon: lines that start at twice the detailed spacing and widen with distance.
  const axis = (centre: number) => {
    const a: number[] = [];
    const inner = Math.max(2, Math.round(cells / 2));
    for (let i = 0; i <= inner; i++) a.push(centre - half + (i * extent) / inner);
    let d = 0;
    while (d < 9000) {
      d += Math.min(480, cell * 2 + 0.08 * d);
      a.push(centre + half + d);
      a.unshift(centre - half - d);
    }
    return a;
  };
  const ax = axis(cx);
  const az = axis(cz);
  const mx = ax.length;
  const mz = az.length;
  const verts = new Float32Array(mx * mz * 3);
  const cols = new Float32Array(mx * mz * 3);
  const uvs = new Float32Array(mx * mz * 2);
  const index: number[] = [];
  for (let j = 0; j < mz; j++) {
    for (let k = 0; k < mx; k++) {
      const x = ax[k]!;
      const z = az[j]!;
      const i = j * mx + k;
      const inside = Math.abs(x - cx) <= half + 0.01 && Math.abs(z - cz) <= half + 0.01;
      const y = inside ? heightAt(x, z) - 0.3 : outer(x, z);
      verts[i * 3] = x;
      verts[i * 3 + 1] = y;
      verts[i * 3 + 2] = z;
      uvs[i * 2] = x * 0.12;
      uvs[i * 2 + 1] = z * 0.12;
      blend(SOIL, clamp(0.5 + 0.9 * far.fbm(x / 400, z / 400, 3), 0, 0.999), c);
      if (y > 60) c.lerp(C("#8d7560"), smooth((y - 60) / 160) * 0.6);
      cols[i * 3] = c.r;
      cols[i * 3 + 1] = c.g;
      cols[i * 3 + 2] = c.b;
    }
  }
  for (let j = 0; j < mz - 1; j++) {
    for (let k = 0; k < mx - 1; k++) {
      if (ax[k]! >= cx - half - 0.01 && ax[k + 1]! <= cx + half + 0.01 && az[j]! >= cz - half - 0.01 && az[j + 1]! <= cz + half + 0.01) continue;
      const a = j * mx + k;
      index.push(a, a + mx, a + 1, a + 1, a + mx, a + mx + 1);
    }
  }
  const g2 = new THREE.BufferGeometry();
  g2.setAttribute("position", new THREE.BufferAttribute(verts, 3));
  g2.setAttribute("color", new THREE.BufferAttribute(cols, 3));
  g2.setAttribute("uv", new THREE.BufferAttribute(uvs, 2));
  g2.setIndex(index);
  g2.computeVertexNormals();
  const ring = new THREE.Mesh(g2, material);
  ring.receiveShadow = true;
  group.add(ring);
  return { group, heightAt, material, mesh };
}

/* ---------- Cladding ---------- */

let ribTex: THREE.DataTexture | null = null;
/** Trapezoidal sheet ribs, 5 per metre along u. */
function ribNormal() {
  if (ribTex) return ribTex;
  const S = 128;
  const d = new Uint8Array(S * S * 4);
  for (let x = 0; x < S; x++) {
    const u = (x / S) * 5;
    const f = u - Math.floor(u);
    // A flat pan, a sloped flank, a flat crest, a sloped flank.
    const slope = f < 0.55 ? 0 : f < 0.63 ? 0.9 : f < 0.85 ? 0 : f < 0.93 ? -0.9 : 0;
    const l = Math.hypot(slope, 1);
    for (let y = 0; y < S; y++) {
      const k = (y * S + x) * 4;
      d[k] = Math.round((slope / l) * 0.5 * 255 + 127.5);
      d[k + 1] = 128;
      d[k + 2] = Math.round((1 / l) * 0.5 * 255 + 127.5);
      d[k + 3] = 255;
    }
  }
  ribTex = new THREE.DataTexture(d, S, S, THREE.RGBAFormat);
  ribTex.wrapS = ribTex.wrapT = THREE.RepeatWrapping;
  ribTex.minFilter = THREE.LinearMipmapLinearFilter;
  ribTex.magFilter = THREE.LinearFilter;
  ribTex.generateMipmaps = true;
  ribTex.needsUpdate = true;
  return ribTex;
}

const cladCache = new Map<string, THREE.MeshStandardMaterial>();
/** Ribbed sheet cladding in a colour, weathered. Map UVs in metres (u across the ribs). */
export function cladding(colour: string, metal = 0.15) {
  const key = `${colour}:${metal}`;
  let m = cladCache.get(key);
  if (!m) {
    m = grime(new THREE.MeshStandardMaterial({ color: colour, roughness: 0.55, metalness: metal, normalMap: ribNormal(), normalScale: new THREE.Vector2(0.9, 0.9) }), {
      dustAmount: 0.8,
      mudAmount: 0.7,
      scale: 0.06,
    });
    m.name = `clad:${key}`;
    cladCache.set(key, m);
  }
  return m;
}

/** A wall panel in the vertical plane from a to b (bottom corners), h tall, with UVs in metres. */
export function wall(parent: THREE.Object3D, mat: THREE.Material, a: V3, b: V3, h: number) {
  const w = a.distanceTo(b);
  const g = new THREE.PlaneGeometry(w, h);
  const uv = g.getAttribute("uv") as THREE.BufferAttribute;
  for (let i = 0; i < uv.count; i++) uv.setXY(i, uv.getX(i) * w, uv.getY(i) * h);
  const m = new THREE.Mesh(g, mat);
  m.position.copy(a).lerp(b, 0.5).setY(a.y + h / 2);
  m.rotation.y = Math.atan2(b.x - a.x, b.z - a.z) - Math.PI / 2;
  parent.add(m);
  return m;
}

/** Four clad walls round a rectangle (centre x, z; ground y), outward-facing, with a mono-pitch or flat roof. */
export function shed(
  parent: THREE.Object3D,
  o: { x: number; y: number; z: number; w: number; d: number; h: number; ry?: number; colour: string; roof?: string; pitch?: number; skip?: ("n" | "s" | "e" | "w" | "roof")[] },
) {
  const g = new THREE.Group();
  g.position.set(o.x, o.y, o.z);
  g.rotation.y = o.ry ?? 0;
  parent.add(g);
  const mat = cladding(o.colour);
  const w = o.w / 2;
  const d = o.d / 2;
  const skip = new Set(o.skip ?? []);
  const P = (x: number, z: number) => new THREE.Vector3(x, 0, z);
  if (!skip.has("s")) wall(g, mat, P(-w, d), P(w, d), o.h);
  if (!skip.has("e")) wall(g, mat, P(w, d), P(w, -d), o.h);
  if (!skip.has("n")) wall(g, mat, P(w, -d), P(-w, -d), o.h);
  if (!skip.has("w")) wall(g, mat, P(-w, -d), P(-w, d), o.h);
  if (!skip.has("roof")) {
    const rise = (o.pitch ?? 0.05) * o.d;
    const roof = cladding(o.roof ?? "#9ea3a0", 0.35);
    const rg = new THREE.PlaneGeometry(o.w + 0.6, Math.hypot(o.d + 0.6, rise));
    const uv = rg.getAttribute("uv") as THREE.BufferAttribute;
    for (let i = 0; i < uv.count; i++) uv.setXY(i, uv.getY(i) * o.d, uv.getX(i) * o.w);
    const rm = new THREE.Mesh(rg, roof);
    rm.rotation.x = -Math.PI / 2 + Math.atan2(rise, o.d);
    rm.position.set(0, o.h + rise / 2, 0);
    g.add(rm);
    // Gable infill under a pitched roof.
    if (rise > 0.05) {
      for (const s of [-1, 1]) {
        const tri = new THREE.BufferGeometry().setFromPoints([P(s * w, d), P(s * w, -d).setY(rise), P(s * w, -d)]);
        tri.computeVertexNormals();
        const tm = new THREE.Mesh(tri, mat);
        tm.position.y = o.h;
        if (s < 0) tm.scale.x = 1;
        g.add(tm);
      }
    }
  }
  return g;
}

/* ---------- Pipes ---------- */

/** A polyline with rounded corners of radius `bend` (clamped to half of each run), as a curve. */
export function bendPath(pts: V3[], bend: number): THREE.CurvePath<THREE.Vector3> {
  const path = new THREE.CurvePath<THREE.Vector3>();
  let from = pts[0]!.clone();
  for (let i = 1; i < pts.length - 1; i++) {
    const a = pts[i - 1]!;
    const b = pts[i]!;
    const c = pts[i + 1]!;
    const r = Math.min(bend, a.distanceTo(b) / 2, b.distanceTo(c) / 2);
    const p = b.clone().addScaledVector(a.clone().sub(b).normalize(), r);
    const q = b.clone().addScaledVector(c.clone().sub(b).normalize(), r);
    if (from.distanceTo(p) > 1e-3) path.add(new THREE.LineCurve3(from, p));
    path.add(new THREE.QuadraticBezierCurve3(p, b.clone(), q));
    from = q;
  }
  path.add(new THREE.LineCurve3(from, pts[pts.length - 1]!.clone()));
  return path;
}

/** Points along a curve every `step` metres, with their running distance. */
export function sample(path: THREE.Curve<THREE.Vector3>, step = 1) {
  const L = path.getLength();
  const n = Math.max(2, Math.ceil(L / step));
  const pts = path.getSpacedPoints(n);
  const s = pts.map((_, i) => (i / n) * L);
  return { pts, s, length: L };
}

/** A pipe through points with rounded bends, flanges every ~12 m, as a tube in `parent`. */
export function pipe(parent: THREE.Object3D, mat: THREE.Material, pts: V3[], r: number, o: { bend?: number; flange?: THREE.Material; radial?: number } = {}) {
  const path = bendPath(pts, o.bend ?? Math.max(0.6, r * 4));
  const L = path.getLength();
  const g = new THREE.TubeGeometry(path, Math.max(4, Math.ceil(L / 1.5) + pts.length * 6), r, o.radial ?? 10, false);
  const m = new THREE.Mesh(g, mat);
  parent.add(m);
  if (o.flange) {
    const n = Math.floor(L / 12);
    for (let i = 1; i <= n; i++) {
      const t = (i * 12) / L;
      const p = path.getPointAt(t);
      const d = path.getTangentAt(t);
      bar(parent, o.flange, p.clone().addScaledVector(d, -0.06), p.clone().addScaledVector(d, 0.06), r * 1.35, 12, undefined, false);
    }
  }
  return { mesh: m, path, length: L };
}

/** Steel supports under a pipe: a post and a cross-head every `span` metres where the pipe is off the ground. */
export function pipeSupports(parent: THREE.Object3D, mat: THREE.Material, path: THREE.Curve<THREE.Vector3>, ground: (x: number, z: number) => number, span = 7, r = 0.3) {
  const L = path.getLength();
  for (let s = span / 2; s < L; s += span) {
    const p = path.getPointAt(s / L);
    const gy = ground(p.x, p.z);
    if (p.y - gy < 0.9) continue;
    const d = path.getTangentAt(s / L).setY(0).normalize();
    const side = new THREE.Vector3(-d.z, 0, d.x);
    const top = p.clone().setY(p.y - r - 0.05);
    bar(parent, mat, new THREE.Vector3(p.x, gy, p.z), top, 0.09, 6);
    bar(parent, mat, top.clone().addScaledVector(side, -0.45), top.clone().addScaledVector(side, 0.45), 0.06, 6);
  }
}

/** A round handrail at height y on radius r (with a knee rail and posts every ~1.8 m). */
export function ringRail(parent: THREE.Object3D, mat: THREE.Material, cx: number, y: number, cz: number, r: number, o: { from?: number; to?: number } = {}) {
  const a0 = o.from ?? 0;
  const a1 = o.to ?? Math.PI * 2;
  const n = Math.max(6, Math.ceil(((a1 - a0) * r) / 1.8));
  const at = (k: number, h: number) => {
    const a = a0 + ((a1 - a0) * k) / n;
    return new THREE.Vector3(cx + Math.cos(a) * r, y + h, cz + Math.sin(a) * r);
  };
  for (let k = 0; k < n; k++) {
    bar(parent, mat, at(k, 1.1), at(k + 1, 1.1), 0.025, 5);
    bar(parent, mat, at(k, 0.55), at(k + 1, 0.55), 0.02, 5);
    bar(parent, mat, at(k, 0), at(k, 1.1), 0.026, 5);
  }
  if (a1 - a0 < Math.PI * 2 - 1e-3) bar(parent, mat, at(n, 0), at(n, 1.1), 0.026, 5);
}

/* ---------- Animated surfaces ---------- */

/** A shared clock for every animated surface. */
export const surfaceTime = { value: 0 };

const BUMP = `
vec3 fmBump(vec3 pos, vec3 n, float h, float k) {
  vec3 sx = dFdx(pos);
  vec3 sy = dFdy(pos);
  vec3 r1 = cross(sy, n);
  vec3 r2 = cross(n, sx);
  float det = dot(sx, r1);
  vec2 dh = vec2(dFdx(h), dFdy(h)) * k;
  vec3 grad = sign(det) * (dh.x * r1 + dh.y * r2);
  return normalize(abs(det) * n - grad);
}`;

/**
 * Mineral froth on a flotation cell: bubbles a few centimetres to fifteen across, drifting from the centre to the
 * lip. Put it on a disc in the xz plane whose local (x, z) are metres from the cell's axis; `spill` makes the same
 * froth fall down a lip band instead (local y down).
 */
export function frothMaterial(o: { colour?: string; speed?: number; spill?: boolean } = {}) {
  const uniforms = { uT: surfaceTime, uV: { value: o.speed ?? 0.12 }, uSpill: { value: o.spill ? 1 : 0 } };
  const m = new THREE.MeshStandardMaterial({ color: o.colour ?? "#8f8b7a", roughness: 0.35, metalness: 0.15 });
  m.customProgramCacheKey = () => `fm-froth-${o.spill ? 1 : 0}`;
  m.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, uniforms);
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vFL;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\nvFL = position;");
    sh.fragmentShader = sh.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
uniform float uT; uniform float uV; uniform float uSpill;
varying vec3 vFL;
float gH; float gEdge;
vec2 fmHash(vec2 p) { p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3))); return fract(sin(p) * 43758.5453); }
vec3 fmCells(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p);
  float d1 = 8.0; float d2 = 8.0; float id = 0.0;
  for (int y = -1; y <= 1; y++) for (int x = -1; x <= 1; x++) {
    vec2 g = vec2(float(x), float(y));
    vec2 h = fmHash(i + g);
    vec2 o = g + 0.5 + 0.42 * sin(6.2831 * h + uT * 0.35);
    float d = length(o - f);
    if (d < d1) { d2 = d1; d1 = d; id = h.x; } else if (d < d2) d2 = d;
  }
  return vec3(d1, d2, id);
}
${BUMP}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec2 q;
if (uSpill > 0.5) {
  float a = atan(vFL.z, vFL.x);
  q = vec2(a * 18.0, vFL.y * 9.0 + uT * 1.4);
} else {
  float r = length(vFL.xz);
  float a = atan(vFL.z, vFL.x);
  q = vec2(a * max(r, 0.4) * 7.0, (r - uT * uV) * 7.0);
}
vec3 c1 = fmCells(q);
vec3 c2 = fmCells(q * 2.3 + 7.1);
float big = smoothstep(0.0, 0.5, c1.y - c1.x);
float small = smoothstep(0.0, 0.35, c2.y - c2.x);
gEdge = 1.0 - min(big, mix(1.0, small, 0.55));
gH = (1.0 - c1.x * c1.x) * 0.7 + (1.0 - c2.x * c2.x) * 0.3;
float tone = 0.78 + 0.32 * c1.z;
diffuseColor.rgb *= tone * mix(1.0, 0.42, gEdge);
// Brassy sulphide shine on the bubble tops.
diffuseColor.rgb = mix(diffuseColor.rgb, vec3(0.72, 0.64, 0.42), smoothstep(0.82, 0.98, gH) * 0.35);`,
      )
      .replace("#include <roughnessmap_fragment>", "#include <roughnessmap_fragment>\nroughnessFactor = mix(0.18, 0.75, gEdge);")
      .replace("#include <normal_fragment_maps>", "#include <normal_fragment_maps>\nnormal = fmBump(-vViewPosition, normal, gH, 0.08);");
  };
  return m;
}

/**
 * Liquid in a tank or a pond: slurry or water, its surface stirred by an agitator (`swirl` radians per second at
 * the axis) or ruffled by wind and rain (`ripple`). Local (x, z) of the mesh are metres from the axis.
 */
export function liquidMaterial(o: { colour: string; rough?: number; swirl?: number; ripple?: number; foam?: string; turbid?: string; opacity?: number }) {
  const uniforms = {
    uT: surfaceTime,
    tN: { value: noiseTexture() },
    uSwirl: { value: o.swirl ?? 0 },
    uRipple: { value: o.ripple ?? 0.3 },
    uRain: { value: 0 },
    uFoam: { value: new THREE.Color(o.foam ?? "#d8d2c2") },
    uTurbid: { value: new THREE.Color(o.turbid ?? o.colour) },
  };
  const m = new THREE.MeshPhysicalMaterial({
    color: o.colour,
    roughness: o.rough ?? 0.12,
    metalness: 0,
    clearcoat: 0.6,
    clearcoatRoughness: 0.18,
    transparent: (o.opacity ?? 1) < 1,
    opacity: o.opacity ?? 1,
  });
  m.userData.liquid = uniforms;
  m.customProgramCacheKey = () => "fm-liquid-v3";
  m.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, uniforms);
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vLL;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\nvLL = position;");
    sh.fragmentShader = sh.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
uniform float uT; uniform sampler2D tN; uniform float uSwirl; uniform float uRipple; uniform float uRain;
uniform vec3 uFoam; uniform vec3 uTurbid;
varying vec3 vLL;
float gLH;
float gLod;
${BUMP}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec2 lp = vLL.xz;
float lr = length(lp);
// Ripples fade out once they are smaller than a pixel or two: past that they only alias into glitter.
gLod = 1.0 - smoothstep(0.04, 0.3, length(fwidth(lp)));
float la = atan(lp.y, lp.x) + uSwirl * uT / (0.25 + lr * 0.35);
vec2 sw = vec2(cos(la), sin(la)) * lr;
vec2 drift = vec2(uT * 0.013, uT * 0.008);
float n1 = texture2D(tN, sw * 0.045 + drift).r;
float n2 = texture2D(tN, sw * 0.19 - drift * 2.0).b;
float n3 = texture2D(tN, lp * 0.6 + vec2(uT * 0.31, -uT * 0.27)).b;
gLH = n1 * 0.55 + n2 * 0.35 + n3 * 0.25 * (uRipple + uRain * 2.5);
// Turbid water near the feed, foam streaks where the slurry turns.
diffuseColor.rgb = mix(diffuseColor.rgb, uTurbid, smoothstep(0.55, 0.0, lr / 6.0) * 0.6);
float streak = smoothstep(0.62, 0.8, texture2D(tN, vec2(la * 0.6, lr * 0.08)).a) * step(0.001, uSwirl);
diffuseColor.rgb = mix(diffuseColor.rgb, uFoam, streak * 0.35);`,
      )
      .replace("#include <roughnessmap_fragment>", "#include <roughnessmap_fragment>\nroughnessFactor = clamp(roughnessFactor + uRain * 0.25, 0.02, 1.0);")
      .replace("#include <normal_fragment_maps>", "#include <normal_fragment_maps>\nnormal = fmBump(-vViewPosition, normal, gLH, 0.6 * (0.4 + uRipple + uRain * 1.5) * gLod);");
  };
  return m;
}

/** Molten metal: hot orange-white with a slow flicker. `heat` (0..1) cools it to a dull crust. */
export function moltenMaterial() {
  const heat = { value: 1 };
  const m = new THREE.MeshStandardMaterial({ color: "#5a3a18", roughness: 0.25, metalness: 0.8, emissive: "#ff9a3a", emissiveIntensity: 3 });
  m.userData.heat = heat;
  m.customProgramCacheKey = () => "fm-molten";
  m.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, { uT: surfaceTime, uHeat: heat, tN: { value: noiseTexture() } });
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vML;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\nvML = position;");
    sh.fragmentShader = sh.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform float uT; uniform float uHeat; uniform sampler2D tN; varying vec3 vML;")
      .replace(
        "#include <emissivemap_fragment>",
        `#include <emissivemap_fragment>
float mf = texture2D(tN, vML.xz * 1.7 + vec2(uT * 0.21, uT * 0.17)).r;
vec3 hot = mix(vec3(1.0, 0.42, 0.08), vec3(1.0, 0.86, 0.55), smoothstep(0.35, 0.8, mf));
totalEmissiveRadiance = hot * mix(0.0, 3.2, uHeat) * (0.85 + 0.3 * mf);`,
      );
  };
  return m;
}

/* ---------- The flow overlay ---------- */

export interface FlowLine {
  /** Corner points; bends are rounded. */
  pts: V3[];
  colour: string;
  /** How fast the pulses travel, m/s. */
  speed?: number;
  bend?: number;
}

/**
 * Glowing pulses running along every stream, drawn as a ribbon tube that keeps a couple of pixels wide at any
 * distance. One draw call for every stream together.
 */
export function makeFlows(lines: FlowLine[]) {
  const P: number[] = [];
  const N: number[] = [];
  const S: number[] = [];
  const V: number[] = [];
  const K: number[] = [];
  const I: number[] = [];
  const radial = 6;
  const col = new THREE.Color();
  for (const line of lines) {
    const path = bendPath(line.pts, line.bend ?? 3);
    const { pts, s } = sample(path, 1.2);
    col.set(line.colour);
    const base = P.length / 3;
    const frames = path.computeFrenetFrames(pts.length - 1, false);
    for (let i = 0; i < pts.length; i++) {
      const p = pts[i]!;
      // Frenet frames of a CurvePath can flip at straight runs; build a stable frame from the tangent instead.
      const t = (i < pts.length - 1 ? pts[i + 1]!.clone().sub(p) : p.clone().sub(pts[i - 1]!)).normalize();
      const up = Math.abs(t.y) > 0.95 ? new THREE.Vector3(1, 0, 0) : new THREE.Vector3(0, 1, 0);
      const nx = new THREE.Vector3().crossVectors(t, up).normalize();
      const ny = new THREE.Vector3().crossVectors(nx, t).normalize();
      void frames;
      for (let k = 0; k < radial; k++) {
        const a = (k / radial) * Math.PI * 2;
        const d = nx.clone().multiplyScalar(Math.cos(a)).addScaledVector(ny, Math.sin(a));
        P.push(p.x, p.y, p.z);
        N.push(d.x, d.y, d.z);
        S.push(s[i]!);
        V.push(line.speed ?? 3);
        K.push(col.r, col.g, col.b);
      }
    }
    for (let i = 0; i < pts.length - 1; i++) {
      for (let k = 0; k < radial; k++) {
        const a = base + i * radial + k;
        const b = base + i * radial + ((k + 1) % radial);
        const c = a + radial;
        const d = b + radial;
        I.push(a, c, b, b, c, d);
      }
    }
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(P, 3));
  g.setAttribute("aN", new THREE.Float32BufferAttribute(N, 3));
  g.setAttribute("aS", new THREE.Float32BufferAttribute(S, 1));
  g.setAttribute("aV", new THREE.Float32BufferAttribute(V, 1));
  g.setAttribute("aC", new THREE.Float32BufferAttribute(K, 3));
  g.setIndex(I);
  const uniforms = { uT: surfaceTime, uOn: { value: 1 }, uGap: { value: 9 } };
  const material = new THREE.ShaderMaterial({
    uniforms,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    vertexShader: `attribute vec3 aN; attribute float aS; attribute float aV; attribute vec3 aC;
varying float vS; varying float vV; varying vec3 vC; varying float vD;
void main() {
  vec4 wc = modelMatrix * vec4(position, 1.0);
  float dist = length(cameraPosition - wc.xyz);
  float r = max(0.1, dist * 0.0021);
  vec4 mv = viewMatrix * vec4(wc.xyz + aN * r, 1.0);
  vS = aS; vV = aV; vC = aC; vD = dist;
  gl_Position = projectionMatrix * mv;
}`,
    fragmentShader: `uniform float uT; uniform float uOn; uniform float uGap;
varying float vS; varying float vV; varying vec3 vC; varying float vD;
void main() {
  float gap = uGap * max(1.0, vD / 260.0);
  float p = fract((vS - uT * vV * max(1.0, vD / 260.0)) / gap);
  float pulse = smoothstep(0.0, 0.75, p) * (1.0 - smoothstep(0.75, 0.86, p));
  // Fades out close to the camera, where an additive tube would only wash the view out.
  float a = (0.16 + 0.84 * pulse) * uOn * clamp(1.25 - vD / 2600.0, 0.35, 1.0) * smoothstep(6.0, 40.0, vD);
  gl_FragColor = vec4(vC * (0.55 + 0.9 * pulse), a);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}`,
  });
  const mesh = new THREE.Mesh(g, material);
  mesh.frustumCulled = false;
  mesh.renderOrder = 5;
  return { mesh, uniforms };
}

/* ---------- Small parts ---------- */

/** A cached weathered paint, re-exported for the scenes. */
export const coat = (colour: string, o: Parameters<typeof paint>[1] = {}) => paint(colour, o);

/** A concrete slab, top at y. */
export function slab(parent: THREE.Object3D, mat: THREE.Material, w: number, d: number, x: number, y: number, z: number, t = 0.4) {
  const m = new THREE.Mesh(new THREE.BoxGeometry(w, t, d), mat);
  m.position.set(x, y - t / 2, z);
  parent.add(m);
  return m;
}
