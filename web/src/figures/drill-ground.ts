/**
 * The ground of the drill scene: a desert plain of soil, gibber and dry creek lines out to hazy ranges, saltbush
 * scrub, an access track, the drill pads and the tracks between them, and over the prospect a window of glass
 * ground through which the block model and the holes show, with the strata on its far walls and dark rock below.
 *
 * Units are metres, y up, the natural surface near y = 0. `height` answers for any point so collars, the rig and
 * the crew stand on the ground. Nothing here touches the DOM at import time.
 */
import * as THREE from "three";
import { B, BOTTOM, NX, NZ, X0, Z0 } from "./drill-model";
import { makeRocks } from "./fleet";
import { clamp, mix, rng, smooth } from "./lib";
import { noise2 } from "./stage";
import { blend, rockSurfaceMaterial, strata } from "./terrain";

export const W = NX * B;
export const D = NZ * B;
export const X1 = X0 + W;
export const Z1 = Z0 + D;

const C = (hex: string) => new THREE.Color(hex);
const SOIL = [C("#a48a68"), C("#937a5b"), C("#b09878"), C("#9c8466")];
const GIBBER = C("#76594a");
const SAND = C("#cdb795");
const RANGE = C("#8d7560");
const BUSH = ["#6d6b4c", "#7b7552", "#5e5f45", "#86805f", "#6a604a"].map(C);

/** How far a point lies outside the prospect's footprint (0 inside). */
export const boxDist = (x: number, z: number) => Math.hypot(Math.max(X0 - x, 0, x - X1), Math.max(Z0 - z, 0, z - Z1));

/**
 * Patches a standard material so that its fragments vanish wherever the line of sight goes on into the block: the
 * scrub and stones on the plain do not float over the X-ray. `bMin`/`bMax` bound the block.
 */
function xrayHide(mat: THREE.MeshStandardMaterial, bMin: THREE.Vector3, bMax: THREE.Vector3, key: string) {
  mat.onBeforeCompile = (shader) => {
    shader.uniforms.bMin = { value: bMin };
    shader.uniforms.bMax = { value: bMax };
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vXW;")
      .replace(
        "#include <project_vertex>",
        `#include <project_vertex>
vec4 xw = vec4(transformed, 1.0);
#ifdef USE_INSTANCING
xw = instanceMatrix * xw;
#endif
vXW = (modelMatrix * xw).xyz;`,
      );
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vXW;\nuniform vec3 bMin;\nuniform vec3 bMax;")
      .replace(
        "#include <clipping_planes_fragment>",
        `#include <clipping_planes_fragment>
{
  vec3 rd = normalize(vXW - cameraPosition);
  vec3 inv = 1.0 / rd;
  vec3 t0 = (bMin - vXW) * inv;
  vec3 t1 = (bMax - vXW) * inv;
  vec3 tn = min(t0, t1);
  vec3 tf = max(t0, t1);
  if (min(min(tf.x, tf.y), tf.z) - max(max(max(tn.x, tn.y), tn.z), 0.0) > 4.0) discard;
}`,
      );
  };
  mat.customProgramCacheKey = () => key;
  return mat;
}

export interface Pad {
  x: number;
  z: number;
  /** Bearing of the hole, degrees: the pad lines up with it. */
  bearing: number;
  newest: boolean;
  /** The pad's level (default the natural surface at its centre). */
  y?: number;
}

export interface Ground {
  group: THREE.Group;
  /** What a click on the ground can hit. */
  pickable: THREE.Object3D[];
  height(x: number, z: number): number;
  /**
   * Lays out the pads for these collars (newest last), grading each flat, with the tracks between them and the
   * access track. Returns the track that reaches the newest pad, from where it leaves the network.
   */
  site(pads: Pad[]): THREE.Vector2[];
  /** Shows only the walls of the window that lie behind the model from where the camera stands. */
  update(camera: THREE.Camera): void;
  /** How much of the ground shows over the block: 0 clear, 1 solid. */
  glass(a: number): void;
}

export interface GroundOptions {
  small?: boolean;
  /** How much of the glass shows over the prospect (0 clear, 1 solid ground). */
  glass?: number;
}

/** A soft-edged strip for tracks: two wheel ruts in pale graded dirt, tileable along v. */
function trackTexture() {
  const cv = document.createElement("canvas");
  cv.width = 64;
  cv.height = 256;
  const g = cv.getContext("2d")!;
  const img = g.createImageData(64, 256);
  const r = rng(5);
  for (let y = 0; y < 256; y++)
    for (let x = 0; x < 64; x++) {
      const u = (x + 0.5) / 64;
      const edge = smooth((0.5 - Math.abs(u - 0.5)) / 0.16);
      const rut = Math.exp(-(((Math.abs(u - 0.5) - 0.2) / 0.05) ** 2));
      const grain = 0.92 + 0.16 * r();
      const k = (1 - 0.22 * rut) * grain;
      const i = (y * 64 + x) * 4;
      img.data[i] = 214 * k;
      img.data[i + 1] = 196 * k;
      img.data[i + 2] = 164 * k;
      img.data[i + 3] = 200 * edge * (0.86 + 0.14 * r());
    }
  g.putImageData(img, 0, 0);
  const t = new THREE.CanvasTexture(cv);
  t.wrapT = THREE.RepeatWrapping;
  t.colorSpace = THREE.SRGBColorSpace;
  t.anisotropy = 4;
  return t;
}

/** A scraped, levelled drill pad with soft edges and tyre marks. */
function padTexture() {
  const S = 128;
  const cv = document.createElement("canvas");
  cv.width = cv.height = S;
  const g = cv.getContext("2d")!;
  const img = g.createImageData(S, S);
  const r = rng(9);
  const { fbm } = noise2(71);
  for (let y = 0; y < S; y++)
    for (let x = 0; x < S; x++) {
      const u = (x + 0.5) / S - 0.5;
      const v = (y + 0.5) / S - 0.5;
      const edge = smooth((0.5 - Math.max(Math.abs(u), Math.abs(v))) / 0.12);
      const mottle = 0.9 + 0.22 * fbm(x / 18, y / 18, 3) + 0.08 * r();
      const marks = 1 - 0.12 * Math.exp(-(((Math.abs(u) - 0.18) / 0.03) ** 2)) * smooth((0.4 - Math.abs(v)) / 0.1);
      const k = mottle * marks;
      const i = (y * S + x) * 4;
      img.data[i] = 205 * k;
      img.data[i + 1] = 186 * k;
      img.data[i + 2] = 152 * k;
      img.data[i + 3] = 255 * edge;
    }
  g.putImageData(img, 0, 0);
  const t = new THREE.CanvasTexture(cv);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

export function makeGround(o: GroundOptions = {}): Ground {
  const small = !!o.small;
  const GLASS = o.glass ?? 0.3;
  const { fbm } = noise2(23);
  const far = noise2(41);
  const near = (x: number, z: number) => 3.2 * fbm(x / 380, z / 380, 4) + 1.1 * fbm(x / 70, z / 70, 3);
  const natural = (x: number, z: number) => {
    const r = Math.hypot(x, z);
    if (r < 800) return near(x, z);
    const hills = 16 * far.fbm(x / 900, z / 900, 4) * smooth((r - 800) / 900);
    const ranges = 240 * Math.max(0, far.fbm(x / 2600 + 4, z / 2600 - 2, 5) + 0.1) * smooth((r - 2600) / 2400);
    return near(x, z) + hills + ranges;
  };
  /*
   * Graded pads: each is dead level over its rectangle and an apron wider than a mesh cell, so every triangle under
   * the pad is flat and the pad's surface can lie a few centimetres over it without ever touching; beyond the
   * apron the cut or fill blends back into the plain.
   */
  type Flat = { x: number; z: number; ax: number; az: number; hw: number; hl: number; y: number };
  let flats: Flat[] = [];
  const APRON = small ? 12 : 8;
  const BLEND = 9;
  const height = (x: number, z: number) => {
    let y = natural(x, z);
    for (const f of flats) {
      const dx = x - f.x;
      const dz = z - f.z;
      const c = dx * f.ax + dz * f.az;
      const a = -dx * f.az + dz * f.ax;
      const d = Math.hypot(Math.max(Math.abs(a) - f.hw, 0), Math.max(Math.abs(c) - f.hl, 0));
      if (d < APRON + BLEND) y = mix(y, f.y, 1 - smooth((d - APRON) / BLEND));
    }
    return y;
  };
  const group = new THREE.Group();
  group.name = "drill-ground";
  const tmp = new THREE.Color();

  /* ---------- The surface, out to the horizon ---------- */
  const colourAt = (x: number, z: number, y: number, out: THREE.Color) => {
    blend(SOIL, clamp(0.5 + 0.9 * fbm(x / 60, z / 60, 3), 0, 0.999), out);
    // Stony gibber flats, darker and redder; pale sand along dry creek lines.
    out.lerp(GIBBER, smooth((fbm(x / 170 + 11, z / 170 - 5, 3) - 0.06) / 0.22) * 0.5);
    const ch = Math.abs(fbm(x / 460 - 3, z / 460 + 8, 4) + 0.15 * fbm(x / 60, z / 60, 2));
    out.lerp(SAND, (1 - smooth(ch / 0.045)) * 0.6);
    if (y > 60) out.lerp(RANGE, smooth((y - 60) / 140) * 0.6);
    return out;
  };
  const axis: number[] = [];
  {
    const fine = small ? 8 : 5;
    for (let v = -320; v <= 320; v += fine) axis.push(v);
    for (let v = 320 + (small ? 16 : 10); v <= 1000; v += small ? 16 : 10) {
      axis.push(v);
      axis.unshift(-v);
    }
    let d = 0;
    while (d < 8200) {
      d += Math.min(420, 20 + 0.08 * d);
      axis.push(1000 + d);
      axis.unshift(-1000 - d);
    }
  }
  const m = axis.length;
  const pos = new Float32Array(m * m * 3);
  const col = new Float32Array(m * m * 3);
  const uv = new Float32Array(m * m * 2);
  for (let j = 0; j < m; j++)
    for (let k = 0; k < m; k++) {
      const x = axis[k]!;
      const z = axis[j]!;
      const i = j * m + k;
      const y = height(x, z);
      pos[i * 3] = x;
      pos[i * 3 + 1] = y;
      pos[i * 3 + 2] = z;
      uv[i * 2] = x * 0.12;
      uv[i * 2 + 1] = z * 0.12;
      colourAt(x, z, y, tmp);
      col.set([tmp.r, tmp.g, tmp.b], i * 3);
    }
  const idx = new Uint32Array((m - 1) * (m - 1) * 6);
  for (let j = 0, o = 0; j < m - 1; j++)
    for (let k = 0; k < m - 1; k++, o += 6) {
      const a = j * m + k;
      idx.set([a, a + m, a + 1, a + 1, a + m, a + m + 1], o);
    }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
  geo.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  geo.setIndex(new THREE.BufferAttribute(idx, 1));
  geo.computeVertexNormals();
  /*
   * X-ray ground: wherever the line of sight through a point of the ground goes on into the prospect's block, the
   * ground there turns to glass, softly at the silhouette's edge; everywhere else it is solid. So the block shows
   * whole from any side the camera turns to, and the plain around it stays real. The mesh is drawn after the
   * things under it and still writes depth, so hills hide hills.
   */
  const mat = rockSurfaceMaterial();
  mat.transparent = true;
  const xu = {
    bMin: { value: new THREE.Vector3(X0, BOTTOM - 10, Z0) },
    bMax: { value: new THREE.Vector3(X1, -2, Z1) },
    glass: { value: GLASS },
  };
  const baseCompile = mat.onBeforeCompile;
  mat.onBeforeCompile = (shader, r) => {
    baseCompile.call(mat, shader, r);
    Object.assign(shader.uniforms, xu);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform vec3 bMin;\nuniform vec3 bMax;\nuniform float glass;")
      .replace(
        "#include <alphamap_fragment>",
        `#include <alphamap_fragment>
{
  vec3 rd = normalize(vRW - cameraPosition);
  vec3 inv = 1.0 / rd;
  vec3 t0 = (bMin - vRW) * inv;
  vec3 t1 = (bMax - vRW) * inv;
  vec3 tn = min(t0, t1);
  vec3 tf = max(t0, t1);
  float a = max(max(max(tn.x, tn.y), tn.z), 0.0);
  float b = min(min(tf.x, tf.y), tf.z);
  diffuseColor.a = mix(1.0, glass, smoothstep(0.0, 26.0, b - a));
}`,
      );
  };
  mat.customProgramCacheKey = () => "fm-rock-v4-xray";
  const surface = new THREE.Mesh(geo, mat);
  surface.receiveShadow = true;
  surface.renderOrder = 5;
  group.add(surface);

  /* ---------- Under the glass: dark rock below the model, and the strata on the far walls ---------- */
  {
    const floorGeo = new THREE.PlaneGeometry(W, D, 48, 48);
    floorGeo.rotateX(-Math.PI / 2);
    floorGeo.translate((X0 + X1) / 2, BOTTOM - 10, (Z0 + Z1) / 2);
    const fp = floorGeo.getAttribute("position") as THREE.BufferAttribute;
    const fc = new Float32Array(fp.count * 3);
    for (let i = 0; i < fp.count; i++) {
      const x = fp.getX(i);
      const z = fp.getZ(i);
      strata(400, clamp(0.5 + 0.5 * fbm(x / 140, z / 140, 2), 0, 0.999), tmp).multiplyScalar(0.26);
      fc.set([tmp.r, tmp.g, tmp.b], i * 3);
    }
    floorGeo.setAttribute("color", new THREE.BufferAttribute(fc, 3));
    const fuv = floorGeo.getAttribute("uv") as THREE.BufferAttribute;
    for (let i = 0; i < fp.count; i++) fuv.setXY(i, fp.getX(i) * 0.12, fp.getZ(i) * 0.12);
    const floorMat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 1, metalness: 0, fog: false });
    const floor = new THREE.Mesh(floorGeo, floorMat);
    floor.receiveShadow = true;
    group.add(floor);
  }
  const walls: THREE.Mesh[] = [];
  {
    const mat = rockSurfaceMaterial();
    mat.side = THREE.DoubleSide;
    mat.fog = false;
    const corners = [
      [X0, Z0],
      [X1, Z0],
      [X1, Z1],
      [X0, Z1],
    ] as const;
    for (let c = 0; c < 4; c++) {
      const [ax, az] = corners[c]!;
      const [bx, bz] = corners[(c + 1) % 4]!;
      const nu = 48;
      const nv = 64;
      const p: number[] = [];
      const cl: number[] = [];
      const idx: number[] = [];
      for (let v = 0; v <= nv; v++)
        for (let u = 0; u <= nu; u++) {
          const x = mix(ax, bx, u / nu);
          const z = mix(az, bz, u / nu);
          const top = near(x, z);
          const y = mix(top, BOTTOM - 10, v / nv);
          p.push(x, y, z);
          // Bands that wander a little, as real contacts do.
          const depth = top - y + 6 * fbm(x / 80 + z / 80, y / 40, 2);
          strata(depth, clamp(0.5 + 0.9 * fbm(x / 30 + y / 22, z / 30 - y / 37, 3), 0, 0.999), tmp);
          tmp.multiplyScalar(mix(0.82, 0.36, v / nv));
          cl.push(tmp.r, tmp.g, tmp.b);
        }
      for (let v = 0; v < nv; v++)
        for (let u = 0; u < nu; u++) {
          const a = v * (nu + 1) + u;
          idx.push(a, a + nu + 1, a + 1, a + 1, a + nu + 1, a + nu + 2);
        }
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.Float32BufferAttribute(p, 3));
      g.setAttribute("color", new THREE.Float32BufferAttribute(cl, 3));
      const wuv: number[] = [];
      for (let i = 0; i < p.length; i += 3) wuv.push((p[i]! + p[i + 2]!) * 0.12, p[i + 1]! * 0.12);
      g.setAttribute("uv", new THREE.Float32BufferAttribute(wuv, 2));
      g.setIndex(idx);
      g.computeVertexNormals();
      const wall = new THREE.Mesh(g, mat);
      wall.receiveShadow = true;
      const mid = new THREE.Vector3((ax + bx) / 2 - (X0 + X1) / 2, 0, (az + bz) / 2 - (Z0 + Z1) / 2);
      wall.userData.half = mid.length();
      wall.userData.out = mid.normalize();
      walls.push(wall);
      group.add(wall);
    }
  }
  // The window's rim and its depths, as faint lines.
  {
    const lines: number[] = [];
    const corners = [
      [X0, Z0],
      [X1, Z0],
      [X1, Z1],
      [X0, Z1],
    ] as const;
    for (let c = 0; c < 4; c++) {
      const [ax, az] = corners[c]!;
      const [bx, bz] = corners[(c + 1) % 4]!;
      // The rim follows the ground.
      for (let s = 0; s < 60; s++) {
        const x0 = mix(ax, bx, s / 60);
        const z0 = mix(az, bz, s / 60);
        const x1 = mix(ax, bx, (s + 1) / 60);
        const z1 = mix(az, bz, (s + 1) / 60);
        lines.push(x0, near(x0, z0) + 0.45, z0, x1, near(x1, z1) + 0.45, z1);
      }
      for (const dy of [100, 200, 300]) lines.push(ax, -dy, az, bx, -dy, bz);
    }
    for (const [x, z] of corners) lines.push(x, near(x, z), z, x, BOTTOM, z);
    const lg = new THREE.BufferGeometry();
    lg.setAttribute("position", new THREE.Float32BufferAttribute(lines, 3));
    const outline = new THREE.LineSegments(
      lg,
      new THREE.LineBasicMaterial({ color: "#f6e6d2", transparent: true, opacity: 0.3, depthWrite: false, fog: false }),
    );
    outline.renderOrder = 4;
    group.add(outline);
  }

  /* ---------- Saltbush and stones on the plain ---------- */
  {
    const r = rng(17);
    const n = small ? 1400 : 3600;
    const geo = new THREE.IcosahedronGeometry(1, 1);
    const p = geo.getAttribute("position") as THREE.BufferAttribute;
    for (let i = 0; i < p.count; i++) {
      const x = p.getX(i);
      const y = p.getY(i);
      const z = p.getZ(i);
      const k = 0.82 + 0.3 * Math.abs(Math.sin(x * 5.1 + z * 3.7 + y * 2.3));
      p.setXYZ(i, x * k, Math.max(-0.2, y) * 0.62 * k + 0.28, z * k);
    }
    geo.computeVertexNormals();
    const bushes = new THREE.InstancedMesh(geo, xrayHide(new THREE.MeshStandardMaterial({ roughness: 1, metalness: 0 }), xu.bMin.value, xu.bMax.value, "drill-bush"), n);
    const m4 = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const s = new THREE.Vector3();
    const v = new THREE.Vector3();
    let placed = 0;
    for (let i = 0; i < n * 3 && placed < n; i++) {
      const a = r() * Math.PI * 2;
      const d = 60 + Math.pow(r(), 0.7) * 1150;
      const x = Math.cos(a) * d * 1.1;
      const z = Math.sin(a) * d;
      if (boxDist(x, z) < 14) continue;
      // Scrub grows in clumps, thicker along the creek lines.
      const clump = fbm(x / 90 - 4, z / 90 + 2, 3) + 0.5 * (1 - smooth(Math.abs(fbm(x / 460 - 3, z / 460 + 8, 4)) / 0.08));
      if (r() > 0.25 + clump) continue;
      const size = 0.5 + Math.pow(r(), 2) * 1.3;
      v.set(x, height(x, z), z);
      q.setFromAxisAngle(new THREE.Vector3(0, 1, 0), r() * 6.28);
      m4.compose(v, q, s.set(size, size * (0.7 + 0.5 * r()), size));
      bushes.setMatrixAt(placed, m4);
      bushes.setColorAt(placed, tmp.copy(BUSH[Math.floor(r() * BUSH.length)]!).multiplyScalar(0.8 + 0.3 * r()));
      placed++;
    }
    bushes.count = placed;
    bushes.castShadow = true;
    bushes.receiveShadow = true;
    group.add(bushes);
    const rr = rng(29);
    const stones = makeRocks({
      count: small ? 300 : 900,
      seed: 6,
      shapes: 3,
      detail: 1,
      palette: ["#7a5e4c", "#6b5546", "#8a6d57", "#5e4b40"],
      place: () => {
        for (;;) {
          const x = (rr() - 0.5) * 1700;
          const z = (rr() - 0.5) * 1500;
          if (boxDist(x, z) < 10) continue;
          return { x, y: height(x, z) - 0.1, z, size: 0.25 + Math.pow(rr(), 3) * 1.4, flat: 0.55 };
        }
      },
    });
    const stoneMat = xrayHide(new THREE.MeshStandardMaterial({ roughness: 0.95, metalness: 0 }), xu.bMin.value, xu.bMax.value, "drill-stone");
    for (const mm of stones.meshes) {
      mm.material = stoneMat;
      mm.castShadow = true;
      mm.receiveShadow = true;
    }
    group.add(stones.group);
  }

  /* ---------- Pads and tracks ---------- */
  const trackMat = new THREE.MeshStandardMaterial({
    map: trackTexture(),
    transparent: true,
    depthWrite: false,
    roughness: 1,
    polygonOffset: true,
    polygonOffsetFactor: -2,
    polygonOffsetUnits: -2,
  });
  const padMat = new THREE.MeshStandardMaterial({
    map: padTexture(),
    transparent: true,
    depthWrite: false,
    roughness: 1,
    polygonOffset: true,
    polygonOffsetFactor: -3,
    polygonOffsetUnits: -3,
  });
  const tracks = new THREE.Mesh(new THREE.BufferGeometry(), trackMat);
  const pads = new THREE.Mesh(new THREE.BufferGeometry(), padMat);
  tracks.receiveShadow = pads.receiveShadow = true;
  tracks.renderOrder = 6;
  pads.renderOrder = 7;
  group.add(tracks, pads);

  /** A ribbon draped on the ground along a polyline. */
  const ribbon = (pts: THREE.Vector2[], width: number, out: { p: number[]; uv: number[]; idx: number[] }) => {
    const dense: THREE.Vector2[] = [];
    for (let i = 0; i < pts.length - 1; i++) {
      const a = pts[i]!;
      const b = pts[i + 1]!;
      const n = Math.max(1, Math.ceil(a.distanceTo(b) / 3));
      for (let s = 0; s < n; s++) dense.push(a.clone().lerp(b, s / n));
    }
    dense.push(pts[pts.length - 1]!.clone());
    let along = 0;
    const base = out.p.length / 3;
    for (let i = 0; i < dense.length; i++) {
      const c = dense[i]!;
      const prev = dense[Math.max(0, i - 1)]!;
      const next = dense[Math.min(dense.length - 1, i + 1)]!;
      const t = next.clone().sub(prev).normalize();
      if (i > 0) along += c.distanceTo(prev);
      for (const sgn of [-1, 1]) {
        const x = c.x - t.y * sgn * width * 0.5;
        const z = c.y + t.x * sgn * width * 0.5;
        out.p.push(x, height(x, z) + 0.07, z);
        out.uv.push(sgn < 0 ? 0 : 1, along / 24);
      }
      if (i > 0) {
        const a = base + (i - 1) * 2;
        out.idx.push(a, a + 1, a + 2, a + 1, a + 3, a + 2);
      }
    }
  };
  const meshOf = (target: THREE.Mesh, o2: { p: number[]; uv: number[]; idx: number[] }) => {
    target.geometry.dispose();
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(o2.p, 3));
    g.setAttribute("uv", new THREE.Float32BufferAttribute(o2.uv, 2));
    g.setIndex(o2.idx);
    g.computeVertexNormals();
    target.geometry = g;
  };
  // A gentle wander, so bulldozed tracks do not look ruled.
  const wander = (a: THREE.Vector2, b: THREE.Vector2, seed: number) => {
    const len = a.distanceTo(b);
    const n = Math.max(2, Math.ceil(len / 30));
    const t = b.clone().sub(a).normalize();
    const pts: THREE.Vector2[] = [];
    for (let i = 0; i <= n; i++) {
      const f = i / n;
      const off = Math.sin(f * Math.PI) * 7 * fbm(seed * 3.1 + f * 2.2, seed, 2) * Math.min(1, len / 80);
      pts.push(a.clone().lerp(b, f).add(new THREE.Vector2(-t.y * off, t.x * off)));
    }
    return pts;
  };
  const ENTRY = new THREE.Vector2(X1 + 30, Z0 - 20);
  const access = [new THREE.Vector2(3400, -3000), new THREE.Vector2(1900, -1500), ...wander(new THREE.Vector2(900, -760), ENTRY, 9)];

  const posAttr = geo.getAttribute("position") as THREE.BufferAttribute;
  const nrmAttr = geo.getAttribute("normal") as THREE.BufferAttribute;
  /** Re-levels the surface mesh around the pads, normals from the height field itself. */
  const regrade = (around: Flat[]) => {
    const nrm = new THREE.Vector3();
    for (const f of around) {
      const reach = Math.hypot(f.hw, f.hl) + APRON + BLEND + 6;
      for (let j = 0; j < m; j++) {
        const z = axis[j]!;
        if (Math.abs(z - f.z) > reach) continue;
        for (let k = 0; k < m; k++) {
          const x = axis[k]!;
          if (Math.abs(x - f.x) > reach) continue;
          const i = j * m + k;
          posAttr.setY(i, height(x, z));
          const e = 1.5;
          nrm.set(height(x - e, z) - height(x + e, z), 2 * e, height(x, z - e) - height(x, z + e)).normalize();
          nrmAttr.setXYZ(i, nrm.x, nrm.y, nrm.z);
        }
      }
    }
    posAttr.needsUpdate = true;
    nrmAttr.needsUpdate = true;
    geo.computeBoundingSphere();
  };

  const site = (list: Pad[]) => {
    // Grade the pads first, so tracks and everything standing on a pad sit on the level.
    const before = flats;
    flats = list.map((pd) => {
      const b = THREE.MathUtils.degToRad(pd.bearing);
      const big = pd.newest;
      return { x: pd.x, z: pd.z, ax: Math.sin(b), az: -Math.cos(b), hw: big ? 11 : 7, hl: big ? 14 : 8, y: pd.y ?? natural(pd.x, pd.z) };
    });
    regrade([...before, ...flats]);
    const t = { p: [] as number[], uv: [] as number[], idx: [] as number[] };
    ribbon(access, 5, t);
    // Each pad is reached from the nearest earlier pad, or from the entry.
    const done: THREE.Vector2[] = [ENTRY];
    let route: THREE.Vector2[] = [];
    list.forEach((pd, i) => {
      const at = new THREE.Vector2(pd.x, pd.z);
      let best = done[0]!;
      for (const d of done) if (d.distanceTo(at) < best.distanceTo(at)) best = d;
      const path = wander(best, at, i + 1);
      ribbon(path, 3.6, t);
      if (pd.newest) route = path;
      done.push(at);
    });
    meshOf(tracks, t);
    // Each pad's surface: a level sheet 5 cm over its graded flat.
    const p = { p: [] as number[], uv: [] as number[], idx: [] as number[] };
    flats.forEach((f) => {
      const base = p.p.length / 3;
      const N = 4;
      for (let v = 0; v <= N; v++)
        for (let u = 0; u <= N; u++) {
          const a = (u / N - 0.5) * 2 * f.hw;
          const c = (v / N - 0.5) * 2 * f.hl;
          p.p.push(f.x - f.az * a + f.ax * c, f.y + 0.05, f.z + f.ax * a + f.az * c);
          p.uv.push(u / N, v / N);
        }
      for (let v = 0; v < N; v++)
        for (let u = 0; u < N; u++) {
          const a = base + v * (N + 1) + u;
          p.idx.push(a, a + N + 1, a + 1, a + 1, a + N + 1, a + N + 2);
        }
    });
    meshOf(pads, p);
    return route;
  };

  const camFlat = new THREE.Vector3();
  return {
    group,
    pickable: [surface],
    height,
    site,
    glass(a) {
      xu.glass.value = a;
    },
    update(camera) {
      camFlat.set(camera.position.x - (X0 + X1) / 2, 0, camera.position.z - (Z0 + Z1) / 2);
      // A wall is a backdrop wherever the camera stands on its inner side, including side-on within its span.
      for (const w of walls) w.visible = (w.userData.out as THREE.Vector3).dot(camFlat) < (w.userData.half as number);
    },
  };
}
