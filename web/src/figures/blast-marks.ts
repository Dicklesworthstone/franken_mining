/**
 * What is drawn on and over the bench in the blast figure: the drill-cuttings collars with their stemming, the
 * detonator harness, the surveyor's pegs and flags along the ore boundary, the buried movement monitors, lines draped
 * over the ground, the measured heave as arrows, and a data overlay that paints the grade-control model before the
 * blast and the dig-line verdicts after it. No DOM at import.
 */
import * as THREE from "three";
import { mergeGeometries } from "three/examples/jsm/utils/BufferGeometryUtils.js";
import { holes, inside, ORE, ORE_MOVED, grade, SPAN, BACK, toBench } from "./blast-plan";
import { clamp, rng } from "./lib";

type HeightFn = (x: number, z: number) => number;

/* ---------- Collars ---------- */
/** A cone of drill cuttings round each hole, darker stemming aggregate in the middle. One draw call. */
export function makeCollars(heightAt: HeightFn) {
  const prof: THREE.Vector2[] = [];
  const R = 1.05;
  for (let i = 0; i <= 14; i++) {
    const t = i / 14;
    const r = 0.13 + (R - 0.13) * t;
    // A low cone with a slight crater at the stemming.
    const h = 0.34 * (1 - t) ** 1.4 - (t < 0.12 ? 0.05 * (1 - t / 0.12) : 0);
    prof.push(new THREE.Vector2(r, Math.max(0, h)));
  }
  prof.unshift(new THREE.Vector2(0, 0.24));
  const g = new THREE.LatheGeometry(prof.reverse(), 18);
  const P = g.getAttribute("position");
  const col = new Float32Array(P.count * 3);
  const r = rng(7);
  for (let i = 0; i < P.count; i++) {
    const d = Math.hypot(P.getX(i), P.getZ(i));
    // Stemming aggregate is grey and coarse; cuttings are pale rock flour.
    const stem = d < 0.16 ? 1 : 0;
    const k = 0.92 + r() * 0.1;
    col.set(stem ? [0.32 * k, 0.31 * k, 0.3 * k] : [0.62 * k, 0.58 * k, 0.52 * k], i * 3);
  }
  g.setAttribute("color", new THREE.BufferAttribute(col, 3));
  const mat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 1, metalness: 0 });
  const mesh = new THREE.InstancedMesh(g, mat, holes.length);
  const m4 = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  holes.forEach((h, i) => {
    const s = 0.85 + r() * 0.35;
    q.setFromAxisAngle(new THREE.Vector3(0, 1, 0), r() * 6.28);
    m4.compose(new THREE.Vector3(h.x, heightAt(h.x, h.z) - 0.04, h.z), q, new THREE.Vector3(s, 0.8 + r() * 0.4, s));
    mesh.setMatrixAt(i, m4);
  });
  mesh.receiveShadow = true;
  mesh.castShadow = true;
  return mesh;
}

/* ---------- The harness ---------- */
/** Detonator leads out of each collar and the surface wire joining them, row by row, with the lead-in back. */
export function makeHarness(heightAt: HeightFn, leadTo: THREE.Vector3) {
  const pts: number[] = [];
  const on = (x: number, z: number, lift = 0.07) => [x, heightAt(x, z) + lift, z];
  const wire = (ax: number, az: number, bx: number, bz: number) => {
    const n = Math.max(2, Math.ceil(Math.hypot(bx - ax, bz - az) / 1.5));
    for (let k = 0; k < n; k++) {
      const t0 = k / n;
      const t1 = (k + 1) / n;
      // A little slack between holes.
      const sag = (t: number) => 0.25 * Math.sin(Math.PI * t);
      pts.push(...on(ax + (bx - ax) * t0 + sag(t0), az + (bz - az) * t0), ...on(ax + (bx - ax) * t1 + sag(t1), az + (bz - az) * t1));
    }
  };
  const byRow = new Map<number, typeof holes>();
  for (const h of holes) byRow.set(h.row, [...(byRow.get(h.row) ?? []), h]);
  for (const [row, hs] of byRow) {
    hs.sort((a, b) => a.u - b.u);
    for (let i = 0; i < hs.length - 1; i++) wire(hs[i]!.x + 0.5, hs[i]!.z + 0.35, hs[i + 1]!.x - 0.5, hs[i + 1]!.z + 0.35);
    // Each lead loops up out of its collar to the wire.
    for (const h of hs) pts.push(...on(h.x, h.z, 0.28), ...on(h.x + 0.4, h.z + 0.35));
    const next = byRow.get(row + 1);
    if (next) wire(hs[0]!.x - 0.5, hs[0]!.z + 0.35, next[0]!.x - 0.5, next[0]!.z + 0.35);
  }
  // The lead-in runs back from the first hole towards the firing point, as far as the bench goes.
  const first = holes[0]!;
  const dir = new THREE.Vector3(leadTo.x - first.x, 0, leadTo.z - first.z).normalize();
  let x = first.x - 0.5;
  let z = first.z + 0.35;
  for (let k = 0; k < 40; k++) {
    const nx = x + dir.x * 3 + Math.sin(k * 0.7) * 0.2;
    const nz = z + dir.z * 3;
    const [, s] = toBench(nx, nz);
    if (s < -66) break;
    wire(x, z, nx, nz);
    x = nx;
    z = nz;
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
  const lines = new THREE.LineSegments(g, pullToCamera(new THREE.LineBasicMaterial({ color: "#f3c34a", transparent: true, opacity: 0.95 }), 0.003));
  lines.renderOrder = 2;
  return lines;
}

/* ---------- Pegs and flags along the painted boundary ---------- */
export function makePegs(points: THREE.Vector3[], colour = "#ff3f8a") {
  const peg = new THREE.CylinderGeometry(0.035, 0.045, 1.15, 6);
  peg.translate(0, 0.575, 0);
  const flag = new THREE.PlaneGeometry(0.42, 0.26);
  flag.translate(0.21, 1.0, 0);
  const pegs = new THREE.InstancedMesh(peg, new THREE.MeshStandardMaterial({ color: "#d9cbb3", roughness: 0.8 }), points.length);
  const flags = new THREE.InstancedMesh(
    flag,
    new THREE.MeshStandardMaterial({ color: colour, emissive: colour, emissiveIntensity: 0.25, roughness: 0.7, side: THREE.DoubleSide }),
    points.length,
  );
  const m4 = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const r = rng(12);
  points.forEach((p, i) => {
    q.setFromEuler(new THREE.Euler((r() - 0.5) * 0.08, r() * 6.28, (r() - 0.5) * 0.08));
    m4.compose(p, q, new THREE.Vector3(1, 1, 1));
    pegs.setMatrixAt(i, m4);
    flags.setMatrixAt(i, m4);
  });
  pegs.castShadow = flags.castShadow = true;
  const group = new THREE.Group();
  group.add(pegs, flags);
  return group;
}

/* ---------- Ribbons draped over the ground ---------- */
export function ribbon(
  path: [number, number][],
  width: number,
  heightAt: HeightFn,
  o: { lift?: number; dash?: [number, number]; closed?: boolean; step?: number } = {},
) {
  const step = o.step ?? 0.5;
  const pts = o.closed ? [...path, path[0]!] : path;
  // Resample evenly.
  const xs: [number, number][] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const [ax, az] = pts[i]!;
    const [bx, bz] = pts[i + 1]!;
    const n = Math.max(1, Math.ceil(Math.hypot(bx - ax, bz - az) / step));
    for (let k = 0; k < n; k++) xs.push([ax + ((bx - ax) * k) / n, az + ((bz - az) * k) / n]);
  }
  xs.push(pts[pts.length - 1]!);
  const pos: number[] = [];
  const idx: number[] = [];
  const lift = o.lift ?? 0.12;
  let run = 0;
  for (let i = 0; i < xs.length; i++) {
    const [x, z] = xs[i]!;
    const [px, pz] = xs[Math.max(0, i - 1)]!;
    const [nx, nz] = xs[Math.min(xs.length - 1, i + 1)]!;
    let tx = nx - px;
    let tz = nz - pz;
    const l = Math.hypot(tx, tz) || 1;
    tx /= l;
    tz /= l;
    const ox = -tz * width * 0.5;
    const oz = tx * width * 0.5;
    pos.push(x + ox, heightAt(x + ox, z + oz) + lift, z + oz, x - ox, heightAt(x - ox, z - oz) + lift, z - oz);
    if (i > 0) {
      run += Math.hypot(x - xs[i - 1]![0], z - xs[i - 1]![1]);
      const on = !o.dash || run % (o.dash[0] + o.dash[1]) < o.dash[0];
      if (on) {
        const a = (i - 1) * 2;
        idx.push(a, a + 1, a + 2, a + 1, a + 3, a + 2);
      }
    }
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  g.setIndex(idx);
  return g;
}
/**
 * Pulls a surface's fragments toward the camera along their own view rays by a fraction of their distance: where it
 * lies on screen does not change, only its depth, and by more the further away it is. Marks draped on the ground then
 * never share its depth, at any range or grazing angle, so they cannot fight it.
 */
export function pullToCamera<M extends THREE.Material>(m: M, k: number): M {
  m.onBeforeCompile = (sh) => {
    sh.vertexShader = sh.vertexShader.replace(
      "#include <project_vertex>",
      `#include <project_vertex>
      mvPosition.xyz *= ${(1 - k).toFixed(5)};
      gl_Position = projectionMatrix * mvPosition;`,
    );
  };
  m.customProgramCacheKey = () => `pull-${k}`;
  return m;
}
export function lineMaterial(colour: string, opacity = 1, toneMapped = false, pull = 0.004) {
  return pullToCamera(
    new THREE.MeshBasicMaterial({
      color: colour,
      transparent: true,
      opacity,
      depthWrite: false,
      toneMapped,
      side: THREE.DoubleSide,
    }),
    pull,
  );
}

/* ---------- Heave arrows ---------- */
const ARROW_VERT = /* glsl */ `
attribute float along;
varying float vAlong;
#include <fog_pars_vertex>
void main() {
  vAlong = along;
  vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
  mvPosition.xyz *= 0.996;
  gl_Position = projectionMatrix * mvPosition;
  #include <fog_vertex>
}`;
const ARROW_FRAG = /* glsl */ `
uniform vec3 colour;
uniform float reveal;
uniform float opacity;
varying float vAlong;
#include <fog_pars_fragment>
void main() {
  if (vAlong > reveal) discard;
  gl_FragColor = vec4(colour, opacity);
  #include <fog_fragment>
  #include <colorspace_fragment>
}`;
export function arrowMaterial(colour: string, opacity = 1) {
  return new THREE.ShaderMaterial({
    uniforms: THREE.UniformsUtils.merge([THREE.UniformsLib.fog, { colour: { value: new THREE.Color(colour) }, reveal: { value: 0 }, opacity: { value: opacity } }]),
    vertexShader: ARROW_VERT,
    fragmentShader: ARROW_FRAG,
    transparent: opacity < 1,
    fog: true,
    side: THREE.DoubleSide,
    depthWrite: opacity >= 1,
  });
}
/** Arcing 3D arrows from where each monitor was buried to where it was found. */
export function arcArrows(pairs: [THREE.Vector3, THREE.Vector3][], o: { r: number; head: number; rise: number }) {
  const parts: THREE.BufferGeometry[] = [];
  for (const [a, b] of pairs) {
    const mid = a.clone().lerp(b, 0.5);
    mid.y += o.rise;
    const curve = new THREE.QuadraticBezierCurve3(a, mid, b);
    const L = a.distanceTo(b);
    const pts = curve.getSpacedPoints(24);
    // Stop the shaft where the head begins.
    const cut = Math.max(0.3, 1 - (o.head * 1.1) / Math.max(L, 0.1));
    const shaft = new THREE.TubeGeometry(new THREE.CatmullRomCurve3(pts.filter((_, i) => i / 24 <= cut)), 20, o.r, 6, false);
    const al = new Float32Array(shaft.getAttribute("position").count);
    for (let i = 0; i < al.length; i++) al[i] = (Math.floor(i / 7) / 20) * cut;
    shaft.setAttribute("along", new THREE.BufferAttribute(al, 1));
    const tip = curve.getPoint(1);
    const dir = curve.getTangent(1);
    const head = new THREE.ConeGeometry(o.head * 0.42, o.head, 10);
    head.translate(0, -o.head / 2, 0);
    head.applyQuaternion(new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir));
    head.translate(tip.x, tip.y, tip.z);
    head.deleteAttribute("uv");
    head.deleteAttribute("normal");
    shaft.deleteAttribute("uv");
    shaft.deleteAttribute("normal");
    head.setAttribute("along", new THREE.BufferAttribute(new Float32Array(head.getAttribute("position").count).fill(0.999), 1));
    parts.push(shaft.index ? shaft.toNonIndexed() : shaft, head.index ? head.toNonIndexed() : head);
  }
  return mergeGeometries(parts)!;
}
/** Flat arrows lying on the muck: the heave field between the monitors. */
export function flatArrows(pairs: [THREE.Vector3, THREE.Vector3][], heightAt: HeightFn, w = 0.32) {
  const pos: number[] = [];
  const along: number[] = [];
  for (const [a, b] of pairs) {
    const dx = b.x - a.x;
    const dz = b.z - a.z;
    const L = Math.hypot(dx, dz);
    if (L < 0.3) continue;
    const tx = dx / L;
    const tz = dz / L;
    const nx = -tz;
    const nz = tx;
    const hl = Math.min(1.3, L * 0.45);
    const y = (x: number, z: number) => heightAt(x, z) + 0.18;
    const quad = (p: number[][]) => {
      for (const v of [p[0]!, p[1]!, p[2]!, p[0]!, p[2]!, p[3]!]) {
        pos.push(v[0]!, y(v[0]!, v[1]!), v[1]!);
        along.push(v[2]!);
      }
    };
    const sx = a.x;
    const sz = a.z;
    const ex = b.x - tx * hl;
    const ez = b.z - tz * hl;
    quad([
      [sx + nx * w * 0.5, sz + nz * w * 0.5, 0],
      [sx - nx * w * 0.5, sz - nz * w * 0.5, 0],
      [ex - nx * w * 0.5, ez - nz * w * 0.5, 0.8],
      [ex + nx * w * 0.5, ez + nz * w * 0.5, 0.8],
    ]);
    const hw = hl * 0.55;
    for (const v of [
      [ex + nx * hw, ez + nz * hw, 0.9],
      [ex - nx * hw, ez - nz * hw, 0.9],
      [b.x, b.z, 0.99],
    ]) {
      pos.push(v[0]!, y(v[0]!, v[1]!), v[1]!);
      along.push(v[2]!);
    }
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute("along", new THREE.Float32BufferAttribute(along, 1));
  return g;
}

/* ---------- The data overlay on the blast area ---------- */
const OV_VERT = /* glsl */ `
attribute float og;
attribute float oa;
attribute float ob;
attribute vec2 obs;
varying float vG;
varying float vA;
varying float vB;
varying vec2 vBS;
varying vec3 vW;
#include <fog_pars_vertex>
void main() {
  vG = og;
  vA = oa;
  vB = ob;
  vBS = obs;
  vec4 w = modelMatrix * vec4(position, 1.0);
  vW = w.xyz;
  vec4 mvPosition = viewMatrix * w;
  // It shares the ground's vertices: pull it a little toward the camera so the two never share a depth.
  mvPosition.xyz *= 0.9975;
  gl_Position = projectionMatrix * mvPosition;
  #include <fog_vertex>
}`;
const OV_FRAG = /* glsl */ `
uniform float mode;
uniform float alpha;
varying float vG;
varying float vA;
varying float vB;
varying vec2 vBS;
varying vec3 vW;
#include <fog_pars_fragment>
void main() {
  vec4 c = vec4(0.0);
  if (mode < 0.5) {
    if (vG < 0.0) discard;
    float g = vG;
    vec3 lo = mix(vec3(0.3, 0.38, 0.5), vec3(0.45, 0.5, 0.56), clamp(g / 0.5, 0.0, 1.0));
    vec3 hi = mix(vec3(1.0, 0.8, 0.32), vec3(0.95, 0.3, 0.14), clamp((g - 0.5) / 2.0, 0.0, 1.0));
    vec3 ramp = g < 0.5 ? lo : hi;
    // 5 m model cells: a thin grid over the pattern, the ore cells filled by grade.
    vec2 f = abs(fract(vBS / 5.0) - 0.5);
    float line = smoothstep(0.46, 0.5, max(f.x, f.y));
    c = vec4(ramp, g < 0.5 ? 0.05 : 0.4);
    c.rgb = mix(c.rgb, vec3(1.0), line * 0.4);
    c.a = max(c.a, line * 0.3);
  } else {
    float hatch = step(0.5, fract((vW.x + vW.z) / 1.8));
    if (vA > 0.5 && vB > 0.5) discard;
    else if (vB > 0.5) c = vec4(1.0, 0.74, 0.2, mix(0.3, 0.7, hatch));
    else if (vA > 0.5) c = vec4(1.0, 0.33, 0.5, mix(0.28, 0.66, hatch));
  }
  c.a *= alpha;
  if (c.a < 0.01) discard;
  gl_FragColor = c;
  #include <fog_fragment>
  #include <colorspace_fragment>
}`;

/** An overlay sharing the ground's positions over its fine region. */
export function makeOverlay(ground: { geometry: THREE.BufferGeometry; xs: number[]; zs: number[]; fine: { i0: number; i1: number; j0: number; j1: number } }) {
  const { xs, zs, fine } = ground;
  const nx = xs.length;
  const N = nx * zs.length;
  const og = new Float32Array(N).fill(-1);
  const oa = new Float32Array(N);
  const ob = new Float32Array(N);
  const obs = new Float32Array(N * 2);
  const idx: number[] = [];
  for (let j = fine.j0; j <= fine.j1; j++) {
    for (let i = fine.i0; i <= fine.i1; i++) {
      const k = j * nx + i;
      const [u, s] = toBench(xs[i]!, zs[j]!);
      obs[k * 2] = u;
      obs[k * 2 + 1] = s;
      // The model shows over the pattern: 5 m cells, each with the grade at its centre.
      if (s >= BACK - 2.5 && s <= 0 && Math.abs(u - SPAN.mid) <= SPAN.half + 2.5) {
        const cu = (Math.floor(u / 5) + 0.5) * 5;
        const cs = (Math.floor(s / 5) + 0.5) * 5;
        og[k] = grade(cu, cs);
      }
      oa[k] = inside(ORE, u, s) ? 1 : 0;
      ob[k] = inside(ORE_MOVED, u, s) ? 1 : 0;
      if (i < fine.i1 && j < fine.j1) {
        const a = k;
        idx.push(a, a + nx, a + nx + 1, a, a + nx + 1, a + 1);
      }
    }
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", ground.geometry.getAttribute("position"));
  g.setAttribute("og", new THREE.BufferAttribute(og, 1));
  g.setAttribute("oa", new THREE.BufferAttribute(oa, 1));
  g.setAttribute("ob", new THREE.BufferAttribute(ob, 1));
  g.setAttribute("obs", new THREE.BufferAttribute(obs, 2));
  g.setIndex(N > 65535 ? new THREE.Uint32BufferAttribute(idx, 1) : new THREE.Uint16BufferAttribute(idx, 1));
  g.boundingSphere = new THREE.Sphere(new THREE.Vector3(0, 0, -5), 120);
  const material = new THREE.ShaderMaterial({
    uniforms: THREE.UniformsUtils.merge([THREE.UniformsLib.fog, { mode: { value: 0 }, alpha: { value: 1 } }]),
    vertexShader: OV_VERT,
    fragmentShader: OV_FRAG,
    transparent: true,
    depthWrite: false,
    fog: true,
  });
  const mesh = new THREE.Mesh(g, material);
  mesh.renderOrder = 1;
  return {
    mesh,
    set(mode: 0 | 1, alpha: number) {
      material.uniforms.mode!.value = mode;
      material.uniforms.alpha!.value = clamp(alpha, 0, 1);
      mesh.visible = alpha > 0.01;
    },
  };
}
