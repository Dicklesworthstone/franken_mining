/**
 * Terrain for the open-pit scenes, built the way a mine surveyor's surface is: a pit cut in horizontal benches at
 * fixed elevations, each a steep blasted face over a talus apron and a flat catch berm; a haul ramp cut into the
 * wall at a constant grade, crossing the bench levels as it spirals down; a sump at the floor; waste dumps built in
 * lifts at the angle of repose; a run-of-mine pad; haul roads with safety windrows; and a desert out to the hills.
 *
 * The pit and each dump are polar strip meshes whose rows lie exactly on every crest and toe, so bench edges are
 * crisp lines rather than the saw-tooth a square grid gives a stepped surface. Where the ramp passes a bench level
 * the row structure shifts across flat ground only (berm into ramp), so nothing tears. The base ground is a square
 * grid with the pit and dumps cut out; where it overlaps their outer bands it wins the depth test by polygon offset,
 * so no two surfaces fight over the same pixels.
 *
 * Geometry (DESIGN, typical of a large open pit, not one site's design): 15 m benches; 70° bench faces with a talus
 * apron at 34° over the lowest 14% of each face; catch berms about 7 m; 45° inter-ramp slope; a 30 m two-way ramp at
 * 10% with a windrow half a tyre high on its open edge; dumps in 20 m lifts at 37° with 14 m setbacks.
 *
 * `heightAt` answers for any point from the same rows the meshes are built from, so trucks and people stand on
 * what is drawn. Units are metres, y up, the natural surface near y = 0.
 */
import * as THREE from "three";
import { gravelNormal, noiseTexture } from "./fleet-kit";
import { clamp, finish, mix, smooth } from "./lib";
import { noise2 } from "./stage";

export interface PitOptions {
  seed?: number;
  /** Side of the square of detailed base ground, centred on the pit. */
  extent?: number;
  /** Grid cells along a side of that square. */
  cells?: number;
  /** Angular columns in the pit's own mesh (fewer on phones). */
  columns?: number;
  /** Semi-axes of the pit at surface, along x and z. */
  rx?: number;
  rz?: number;
  depth?: number;
  /** Bench height. */
  bench?: number;
  /** Inter-ramp slope, degrees (crest to crest). */
  overall?: number;
  /** Bench face angle, degrees. */
  faceAngle?: number;
  /** Kept for older callers; the berm now follows from bench height, face angle and inter-ramp slope. */
  berm?: number;
  /** Haul ramp: running width, grade, where it leaves the rim (degrees from +x towards +z), and which way it turns. */
  ramp?: { width?: number; grade?: number; start?: number; turn?: 1 | -1 };
  /** Waste dumps, as centre, radius and height. */
  dumps?: { x: number; z: number; r: number; h: number }[];
  /** Where the run-of-mine pad and crusher sit. */
  pad?: { x: number; z: number; r: number };
  /** Draw the hills to the horizon. */
  surround?: boolean;
}

export interface Pit {
  group: THREE.Group;
  /** Ground height under a point, anywhere. */
  heightAt(x: number, z: number): number;
  /** The ramp's centreline, 2 m apart, from the surface road (index 0) down to the floor. */
  ramp: THREE.Vector3[];
  /** Road from the top of the ramp to the pad, and from the top of the ramp to the first dump. */
  toPad: THREE.Vector3[];
  toDump: THREE.Vector3[];
  floor: { y: number; centre: THREE.Vector3; radius: number };
  pad: THREE.Vector3;
  /** Elevation of each bench floor, from the top bench down. */
  benches: number[];
  depth: number;
  rx: number;
  rz: number;
  /** Bench height and the elevation of the top bench (the datum the benches are cut from). */
  bench: number;
  datum: number;
  /** Is a point inside the pit's surface outline? */
  inside(x: number, z: number): boolean;
  /** The middle of the berm `depth` metres down on a bearing (radians from +x towards +z), or null if none. */
  berm(phi: number, depth: number): THREE.Vector3 | null;
  /** Design strings as a surveyor draws them: each bench's crest and toe, closed loops of points. */
  strings: { crests: THREE.Vector3[][]; toes: THREE.Vector3[][] };
  /** Windrow centrelines (for scenes that want to label or avoid them). */
  windrows: THREE.Vector3[][];
  material: THREE.MeshStandardMaterial;
}

/* ---------- Rock colours ---------- */
const C = (hex: string) => new THREE.Color(hex);
const SOIL = [C("#a48a68"), C("#937a5b"), C("#b09878")];
const OXIDE = [C("#8c6449"), C("#9d7556"), C("#7f5a42"), C("#a27d5c")];
const TRANS = [C("#a08c6f"), C("#948066"), C("#ab9a7e")];
const FRESH = [C("#7b7872"), C("#6c6a66"), C("#878279"), C("#706f6c")];
const ROAD = C("#b0a48d");
const DUST = C("#c2b091");
const WASTE = [C("#99907f"), C("#8a8172"), C("#a69c8a"), C("#7f776b")];
/** Rock units a pit wall cuts through, as tints over the weathering colours. */
const UNITS = [C("#ffffff"), C("#d9e0dc"), C("#f3d9c4"), C("#e8dcc0")];

/** A colour from a palette, blended smoothly by `v` in [0, 1]. */
export function blend(set: THREE.Color[], v: number, out = new THREE.Color()) {
  const f = clamp(v, 0, 0.999) * (set.length - 1);
  const i = Math.floor(f);
  return out.copy(set[i]!).lerp(set[Math.min(set.length - 1, i + 1)]!, f - i);
}

const t2 = new THREE.Color();
/** The colour of rock at a depth below surface, with a little variety from `v` in [0, 1]. */
export function strata(depth: number, v: number, out = new THREE.Color()) {
  // Blend smoothly through each palette so neighbouring vertices never jump from one colour to another.
  if (depth < 4) return blend(SOIL, v, out);
  if (depth < 34) return blend(OXIDE, v, out).lerp(blend(SOIL, v, t2), smooth((8 - depth) / 4) * 0.6);
  if (depth < 70) return blend(TRANS, v, out).lerp(blend(OXIDE, v, t2), smooth((40 - depth) / 6));
  return blend(FRESH, v, out).lerp(blend(TRANS, v, t2), smooth((78 - depth) / 8));
}

/**
 * A rock-and-dirt material for heightfields: vertex colours, a gravel normal map in world space, fine and coarse
 * noise, broken rock on steep faces, pale dust on the flats, and, where a mesh carries an `aRoad` attribute
 * (across-road offset in metres, weight 0–1), compacted running surface with darker wheel paths.
 */
export function rockSurfaceMaterial() {
  const mat = new THREE.MeshStandardMaterial({
    vertexColors: true,
    roughness: 0.96,
    metalness: 0,
    normalMap: gravelNormal(),
    normalScale: new THREE.Vector2(0.7, 0.7),
  });
  const uniforms = { tNoise: { value: noiseTexture() }, uDust: { value: DUST.clone() } };
  mat.customProgramCacheKey = () => "fm-rock-v5";
  mat.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, uniforms);
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nattribute vec2 aRoad;\nvarying vec3 vRW;\nvarying vec3 vRN;\nvarying vec2 vRoad;")
      .replace(
        "#include <worldpos_vertex>",
        "#include <worldpos_vertex>\nvRW = (modelMatrix * vec4(transformed, 1.0)).xyz;\nvRN = normalize(mat3(modelMatrix) * objectNormal);\nvRoad = aRoad;",
      );
    shader.fragmentShader = shader.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
uniform sampler2D tNoise;
uniform vec3 uDust;
varying vec3 vRW;
varying vec3 vRN;
varying vec2 vRoad;
vec4 triN(vec3 p, vec3 n) {
  vec3 w = pow(abs(n), vec3(4.0));
  w /= (w.x + w.y + w.z + 1e-4);
  return texture2D(tNoise, p.zy) * w.x + texture2D(tNoise, p.xz) * w.y + texture2D(tNoise, p.xy) * w.z;
}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec3 rn = normalize(vRN);
float steep = 1.0 - smoothstep(0.55, 0.9, rn.y);
// Two samples at unrelated scales and a skewed frame each, so the tiles never line up into a visible grid.
vec3 q = vec3(vRW.x * 0.8 + vRW.z * 0.6, vRW.y, vRW.z * 0.8 - vRW.x * 0.6);
vec4 fine = mix(triN(vRW * 0.31, rn), triN(q * 0.197 + 0.43, rn), 0.5);
vec4 coarse = mix(triN(vRW * 0.021, rn), triN(q * 0.0131 + 0.17, rn), 0.5);
vec4 mid = mix(triN(vRW * 0.085 + 0.31, rn), triN(q * 0.0537 + 0.71, rn), 0.5);
// Broken rock on the faces: blotchy tone, dark joints and blast half-barrels running down the face.
float joint = smoothstep(0.62, 0.8, fine.b) * steep;
float barrel = smoothstep(0.86, 0.97, texture2D(tNoise, vec2(atan(vRW.z, vRW.x) * 61.0, vRW.y * 0.012)).r) * steep;
diffuseColor.rgb *= mix(1.0, 0.8 + 0.32 * mid.b * coarse.a + 0.1 * fine.g, steep * 0.8);
diffuseColor.rgb *= 1.0 - joint * 0.35 - barrel * 0.12;
diffuseColor.rgb *= 0.86 + 0.24 * mid.r * (0.6 + 0.4 * fine.g);
// Dust settles on the flats, unevenly.
float dust = (1.0 - steep) * smoothstep(0.35, 0.75, coarse.g * 0.7 + fine.r * 0.4);
diffuseColor.rgb = mix(diffuseColor.rgb, uDust * (0.9 + 0.2 * fine.a), dust * 0.38 * (1.0 - vRoad.y * 0.7));
// Haul road: graded, compacted and swept by tyres. Two lanes; each truck's dual tyres run 2.6 m either side of its
// lane's centre, so four wheel paths show darker and smoother.
float lane = abs(abs(vRoad.x) - 7.5);
float path = smoothstep(1.25, 0.35, abs(lane - 2.6)) * vRoad.y;
diffuseColor.rgb *= 1.0 - 0.15 * path * (0.75 + 0.25 * fine.g);
diffuseColor.rgb = mix(diffuseColor.rgb, diffuseColor.rgb * (0.92 + 0.12 * coarse.b), vRoad.y * 0.5);`,
      )
      .replace(
        "#include <roughnessmap_fragment>",
        `#include <roughnessmap_fragment>
roughnessFactor = clamp(roughnessFactor - 0.08 * (1.0 - smoothstep(0.55, 0.9, normalize(vRN).y)) - 0.12 * smoothstep(1.25, 0.35, abs(abs(abs(vRoad.x) - 7.5) - 2.6)) * vRoad.y, 0.55, 1.0);`,
      );
  };
  return mat;
}

/* ---------- Polar strip meshes (the pit and the dumps) ---------- */

/** What a stretch of a radial profile is; it sets how its rows are spaced and whether the mesh creases at its ends. */
const enum Kind {
  Band = 0,
  Berm = 1,
  Face = 2,
  Ramp = 3,
  Floor = 4,
}
/** Every interval of a profile carries five rows: its two ends and three between. */
const PER = 5;
const SMOOTH_KIND = (k: number) => k === Kind.Band || k === Kind.Floor;

/**
 * A radial profile for one column: for each row, distance in from the outline (D), elevation (Y), the interval's
 * kind and, on a road, the across-road offset (U, NaN elsewhere).
 */
interface Rows {
  D: Float64Array;
  Y: Float64Array;
  K: Uint8Array;
  U: Float64Array;
}

interface PolarSpec {
  cx: number;
  cz: number;
  columns: number;
  rows: number;
  /** Bearing of column c, 0 ≤ c ≤ columns (the last column repeats the first bearing: the strip is not closed). */
  phi(c: number): number;
  /** Outline radius on a bearing. */
  R(phi: number): number;
  /** Fill one column's rows. Rows must not decrease in D. */
  fill(c: number, phi: number, R: number, out: Rows): void;
  /** Colour of a vertex. */
  colour(x: number, y: number, z: number, ny: number, kind: number, out: THREE.Color): void;
  /** How strongly a column's face noise applies (0 at a seam). */
  material: THREE.Material;
}

interface PolarMesh {
  mesh: THREE.Mesh;
  /** Height at a point inside the outline band, or NaN outside it. */
  heightAt(x: number, z: number): number;
  /** Column profile access, for strings and berms. */
  column(c: number): Rows;
  /** Column nearest a bearing. */
  nearest(phi: number): number;
  phiOf(c: number): number;
}

/** A polar strip mesh, built as steps a few dozen columns at a time (see `pace`). */
function* polarSteps(s: PolarSpec): Generator<void, PolarMesh> {
  const NC = s.columns;
  const NR = s.rows;
  const cols: Rows[] = [];
  const phis = new Float64Array(NC + 1);
  const Rs = new Float64Array(NC + 1);
  for (let c = 0; c <= NC; c++) {
    const phi = s.phi(c);
    const R = s.R(phi);
    phis[c] = phi;
    Rs[c] = R;
    const rows: Rows = { D: new Float64Array(NR), Y: new Float64Array(NR), K: new Uint8Array(NR), U: new Float64Array(NR).fill(NaN) };
    s.fill(c, phi, R, rows);
    cols.push(rows);
    if ((c & 63) === 63) yield;
  }
  yield;
  // Positions.
  const nv = (NC + 1) * NR;
  const pos = new Float32Array(nv * 3);
  const roadA = new Float32Array(nv * 2);
  for (let c = 0; c <= NC; c++) {
    const cs = Math.cos(phis[c]!);
    const sn = Math.sin(phis[c]!);
    const R = Rs[c]!;
    const rows = cols[c]!;
    for (let r = 0; r < NR; r++) {
      const i = c * NR + r;
      const rad = Math.max(0, R - rows.D[r]!);
      pos[i * 3] = s.cx + cs * rad;
      pos[i * 3 + 1] = rows.Y[r]!;
      pos[i * 3 + 2] = s.cz + sn * rad;
      const u = rows.U[r]!;
      if (Number.isFinite(u)) {
        roadA[i * 2] = u;
        roadA[i * 2 + 1] = 1;
      }
    }
  }
  yield;
  // Normals: across the profile from the row's own interval (so crests and toes stay sharp), along the wall from
  // the neighbouring columns. Smooth kinds (band, floor) difference across interval ends so they show no seam.
  const nrm = new Float32Array(nv * 3);
  const P = (c: number, r: number, k: 0 | 1 | 2) => pos[(c * NR + r) * 3 + k]!;
  const tp = new THREE.Vector3();
  const tc = new THREE.Vector3();
  const n = new THREE.Vector3();
  // Which way round the cross product points up depends on the direction the columns run.
  let sign = 1;
  {
    const c = Math.floor(NC / 2);
    const r = NR - 2;
    tp.set(P(c, r + 1, 0) - P(c, r - 1, 0), 0, P(c, r + 1, 2) - P(c, r - 1, 2));
    tc.set(P(c + 1, r, 0) - P(c - 1, r, 0), 0, P(c + 1, r, 2) - P(c - 1, r, 2));
    sign = n.crossVectors(tp, tc).y >= 0 ? 1 : -1;
  }
  for (let c = 0; c <= NC; c++) {
    const rows = cols[c]!;
    const c0 = Math.max(0, c - 1);
    const c1 = Math.min(NC, c + 1);
    for (let r = 0; r < NR; r++) {
      const j = r % PER;
      const kind = rows.K[r]!;
      let a = r;
      let b = r;
      if (j > 0) a = r - 1;
      else if (SMOOTH_KIND(kind) && r > 0 && SMOOTH_KIND(rows.K[r - 1]!)) a = r - 2 >= 0 ? r - 2 : r - 1;
      if (j < PER - 1) b = r + 1;
      else if (SMOOTH_KIND(kind) && r + 1 < NR && SMOOTH_KIND(rows.K[r + 1]!)) b = Math.min(NR - 1, r + 2);
      tp.set(P(c, b, 0) - P(c, a, 0), P(c, b, 1) - P(c, a, 1), P(c, b, 2) - P(c, a, 2));
      // A collapsed interval (a ramp of no width, a face of no height): look further along the profile.
      if (tp.lengthSq() < 1e-6) {
        let lo = r;
        let hi = r;
        while (lo > 0 && Math.abs(P(c, lo, 0) - P(c, r, 0)) + Math.abs(P(c, lo, 2) - P(c, r, 2)) < 1e-3) lo--;
        while (hi < NR - 1 && Math.abs(P(c, hi, 0) - P(c, r, 0)) + Math.abs(P(c, hi, 2) - P(c, r, 2)) < 1e-3) hi++;
        tp.set(P(c, hi, 0) - P(c, lo, 0), P(c, hi, 1) - P(c, lo, 1), P(c, hi, 2) - P(c, lo, 2));
      }
      tc.set(P(c1, r, 0) - P(c0, r, 0), P(c1, r, 1) - P(c0, r, 1), P(c1, r, 2) - P(c0, r, 2));
      n.crossVectors(tp, tc).multiplyScalar(sign);
      if (n.lengthSq() < 1e-12 || !Number.isFinite(n.x)) n.set(0, 1, 0);
      n.normalize();
      if (n.y < -0.05) n.negate();
      const i = c * NR + r;
      nrm[i * 3] = n.x;
      nrm[i * 3 + 1] = n.y;
      nrm[i * 3 + 2] = n.z;
    }
  }
  // Colours.
  const col = new Float32Array(nv * 3);
  const cc = new THREE.Color();
  for (let i = 0; i < nv; i++) {
    s.colour(pos[i * 3]!, pos[i * 3 + 1]!, pos[i * 3 + 2]!, nrm[i * 3 + 1]!, cols[Math.floor(i / NR)]!.K[i % NR]!, cc);
    col[i * 3] = cc.r;
    col[i * 3 + 1] = cc.g;
    col[i * 3 + 2] = cc.b;
  }
  // Triangles: each interval's own strip, with the shorter diagonal of each quad.
  const index: number[] = [];
  const d2 = (i: number, j: number) => {
    const dx = pos[i * 3]! - pos[j * 3]!;
    const dy = pos[i * 3 + 1]! - pos[j * 3 + 1]!;
    const dz = pos[i * 3 + 2]! - pos[j * 3 + 2]!;
    return dx * dx + dy * dy + dz * dz;
  };
  for (let c = 0; c < NC; c++) {
    for (let r = 0; r < NR - 1; r++) {
      if (r % PER === PER - 1) continue; // the next row starts the next interval at the same place
      const a = c * NR + r;
      const b = (c + 1) * NR + r;
      const a1 = a + 1;
      const b1 = b + 1;
      if (sign > 0) {
        if (d2(a, b1) <= d2(a1, b)) index.push(a, a1, b1, a, b1, b);
        else index.push(a, a1, b, a1, b1, b);
      } else if (d2(a, b1) <= d2(a1, b)) index.push(a, b1, a1, a, b, b1);
      else index.push(a, b, a1, a1, b, b1);
    }
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setAttribute("normal", new THREE.BufferAttribute(nrm, 3));
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
  geo.setAttribute("aRoad", new THREE.BufferAttribute(roadA, 2));
  const uv = new Float32Array(nv * 2);
  for (let i = 0; i < nv; i++) {
    uv[i * 2] = pos[i * 3]! * 0.12;
    uv[i * 2 + 1] = pos[i * 3 + 2]! * 0.12;
  }
  geo.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  geo.setIndex(nv > 65535 ? new THREE.Uint32BufferAttribute(index, 1) : new THREE.Uint16BufferAttribute(index, 1));
  geo.computeBoundingSphere();
  const mesh = new THREE.Mesh(geo, s.material);
  mesh.castShadow = true;
  mesh.receiveShadow = true;

  // Lookup: the strip's columns are equally spaced in bearing from phi(0).
  const phi0 = phis[0]!;
  const dir = Math.sign(phis[1]! - phis[0]!) || 1;
  const span = (Math.PI * 2) / NC;
  const colAt = (phi: number) => {
    let u = (phi - phi0) * dir;
    u = ((u % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2);
    return u / span;
  };
  const sample = (c: number, D: number) => {
    const rows = cols[c]!;
    const Dd = rows.D;
    if (D <= Dd[0]!) return NaN;
    if (D >= Dd[NR - 1]!) return rows.Y[NR - 1]!;
    let lo = 0;
    let hi = NR - 1;
    while (hi - lo > 1) {
      const m = (lo + hi) >> 1;
      if (Dd[m]! <= D) lo = m;
      else hi = m;
    }
    const span2 = Dd[hi]! - Dd[lo]!;
    return span2 < 1e-9 ? rows.Y[hi]! : mix(rows.Y[lo]!, rows.Y[hi]!, (D - Dd[lo]!) / span2);
  };
  return {
    mesh,
    heightAt(x, z) {
      const dx = x - s.cx;
      const dz = z - s.cz;
      const phi = Math.atan2(dz, dx);
      const f = colAt(phi);
      const c0 = Math.min(NC - 1, Math.floor(f));
      const t = f - c0;
      const r = Math.hypot(dx, dz);
      const h0 = sample(c0, Rs[c0]! - r);
      const h1 = sample(c0 + 1, Rs[c0 + 1]! - r);
      if (!Number.isFinite(h0) || !Number.isFinite(h1)) return NaN;
      return mix(h0, h1, t);
    },
    column: (c) => cols[c]!,
    nearest: (phi) => Math.min(NC, Math.round(colAt(phi))),
    phiOf: (c) => phis[c]!,
  };
}

/** A safety windrow along a path: a trapezoid of loose rock, sunk a little so it meets the ground without a seam. */
function windrowMesh(paths: THREE.Vector3[][], material: THREE.Material, seed: number, height = 1.8, base = 4.8) {
  const { fbm } = noise2(seed);
  const pos: number[] = [];
  const col: number[] = [];
  const index: number[] = [];
  const prof: [number, number][] = [
    [-base / 2, -0.5],
    [-base * 0.22, height * 0.92],
    [0, height],
    [base * 0.22, height * 0.92],
    [base / 2, -0.5],
  ];
  const cc = new THREE.Color();
  for (const path of paths) {
    if (path.length < 2) continue;
    const start = pos.length / 3;
    for (let i = 0; i < path.length; i++) {
      const p = path[i]!;
      const a = path[Math.max(0, i - 1)]!;
      const b = path[Math.min(path.length - 1, i + 1)]!;
      const tx = b.x - a.x;
      const tz = b.z - a.z;
      const tl = Math.hypot(tx, tz) || 1;
      const sx = -tz / tl;
      const sz = tx / tl;
      // Taper the ends into the ground.
      const end = Math.min(i, path.length - 1 - i);
      const k = smooth(end / 3);
      const lump = 1 + 0.22 * fbm(p.x / 3.1, p.z / 3.1, 2);
      for (const [o, h] of prof) {
        pos.push(p.x + sx * o, p.y + (h < 0 ? h : h * k * lump), p.z + sz * o);
        blend(WASTE, clamp(0.5 + 1.2 * fbm(p.x / 9 + o, p.z / 9, 2), 0, 0.999), cc).multiplyScalar(0.92);
        col.push(cc.r, cc.g, cc.b);
      }
    }
    const m = prof.length;
    for (let i = 0; i < path.length - 1; i++) {
      for (let j = 0; j < m - 1; j++) {
        const a = start + i * m + j;
        const b = start + (i + 1) * m + j;
        index.push(a, a + 1, b, a + 1, b + 1, b);
      }
    }
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  geo.setAttribute("color", new THREE.Float32BufferAttribute(col, 3));
  geo.setIndex(index);
  geo.computeVertexNormals();
  const mesh = new THREE.Mesh(geo, material);
  mesh.castShadow = true;
  mesh.receiveShadow = true;
  return mesh;
}

/* ---------- The pit ---------- */
export function makePit(o: PitOptions = {}): Pit {
  return finish(pitSteps(o));
}

/** The pit, built as steps so a caller can pause between them (see `pace`). */
export function* pitSteps(o: PitOptions = {}): Generator<void, Pit> {
  const seed = o.seed ?? 7;
  const { fbm } = noise2(seed);
  const wob = noise2(seed + 3);
  const geo = noise2(seed + 5);
  const extent = o.extent ?? 1150;
  const cells = o.cells ?? 384;
  const columns = o.columns ?? Math.round(cells * 2.3);
  const rx = o.rx ?? 430;
  const rz = o.rz ?? 330;
  const H = o.bench ?? 15;
  const K = Math.max(2, Math.round((o.depth ?? 150) / H));
  const depth = K * H;
  const deg = THREE.MathUtils.degToRad;
  const run = H / Math.tan(deg(o.overall ?? 45));
  const TALUS = 0.14;
  const BFA = deg(o.faceAngle ?? 70);
  const REPOSE = deg(34);
  const runPerH = (1 - TALUS) / Math.tan(BFA) + TALUS / Math.tan(REPOSE);
  /** Horizontal run of a face `h` high, talus included. */
  const faceRun = (h: number) => Math.max(0, h) * runPerH;
  /** Where along a face's run the blasted face meets its talus. */
  const BREAK = (1 - TALUS) / Math.tan(BFA) / runPerH;
  const bw = Math.max(3, run - faceRun(H));
  const W = o.ramp?.width ?? 30;
  const grade = o.ramp?.grade ?? 0.1;
  const turn = o.ramp?.turn ?? 1;
  const start = deg(o.ramp?.start ?? 205);
  const dumps = o.dumps ?? [
    { x: -760, z: -330, r: 300, h: 62 },
    { x: 690, z: 520, r: 240, h: 44 },
  ];
  const padAt = o.pad ?? { x: 160, z: 520, r: 78 };
  const TAU = Math.PI * 2;
  const BAND = [-70, -48, -32, -20, -11, -4.5, 0];
  const E = -BAND[0]!;
  const NF = 8;

  // The pit's outline at surface: an ellipse with a gently irregular edge.
  const rimR = (phi: number) => {
    const c = Math.cos(phi);
    const s = Math.sin(phi);
    const e = 1 / Math.sqrt((c * c) / (rx * rx) + (s * s) / (rz * rz));
    return e * (1 + 0.055 * fbm(c * 1.6 + 3.1, s * 1.6 - 1.7, 3) + 0.02 * Math.sin(phi * 5 + 1.3));
  };
  // The natural surface: gentle swells, and the same with the small ones graded out (what a road is built to).
  const surf = (x: number, z: number) => 5.5 * fbm(x / 420, z / 420, 4) + 1.6 * fbm(x / 90, z / 90, 3);
  const graded = (x: number, z: number) => 5.5 * fbm(x / 420, z / 420, 4) + 0.15;
  /** How a crest wanders off its design line: blasting never breaks exactly to the string. Periodic in bearing. */
  const crestOff = (k: number, phi: number) => 1.8 * wob.fbm(Math.cos(phi) * 9 + k * 3.1, Math.sin(phi) * 9 - k * 1.7, 2);
  const crest = (k: number, phi: number) => k * run + bw + crestOff(k, phi);
  const toe = (k: number, phi: number) => (k === 0 ? 0 : crest(k - 1, phi) + faceRun(H));
  /** Distance in from the rim of the ramp's wall-side edge, when the ramp is `a` metres down on a bearing. */
  const rampOuter = (phi: number, a: number) => {
    const k = Math.min(K - 1, Math.floor(a / H));
    return crest(k, phi) + faceRun(a - k * H);
  };

  /* ---------- The ramp: a spiral down the wall at constant grade ---------- */
  const rampPts: THREE.Vector3[] = [];
  const rampU: number[] = [];
  const rampA: number[] = [];
  const rampPhi: number[] = [];
  {
    let phi = start;
    let a = 0;
    let u = 0;
    const step = 2;
    let guard = 0;
    for (;;) {
      const r = rimR(phi) - (rampOuter(phi, a) + W / 2);
      rampPts.push(new THREE.Vector3(Math.cos(phi) * r, 0, Math.sin(phi) * r));
      rampU.push(u);
      rampA.push(a);
      rampPhi.push(phi);
      if (a >= depth - 1e-6 || guard++ > 6000) break;
      const dphi = step / Math.max(20, r);
      phi += turn * dphi;
      u += dphi;
      a = Math.min(depth, a + grade * step);
    }
  }
  const uEnd = rampU[rampU.length - 1]!;
  // Before the ramp's head the wall eases back to make room for it, over this much of a turn.
  const lead = Math.min(0.42, Math.max(0.05, TAU - uEnd - 0.12));
  const uSeam = uEnd + (TAU - lead - uEnd) / 2;
  const rampState = (u: number): { k: number; a: number; S: number; road: boolean } => {
    if (u <= uEnd) {
      let lo = 0;
      let hi = rampU.length - 1;
      while (hi - lo > 1) {
        const m = (lo + hi) >> 1;
        if (rampU[m]! <= u) lo = m;
        else hi = m;
      }
      const t = (u - rampU[lo]!) / (rampU[hi]! - rampU[lo]! || 1);
      const a = mix(rampA[lo]!, rampA[hi]!, clamp(t, 0, 1));
      return { k: Math.min(K, Math.floor(a / H + 1e-9)), a, S: W, road: true };
    }
    if (u >= TAU - lead) return { k: 0, a: 0, S: W * smooth((u - (TAU - lead)) / lead), road: false };
    // The stretch with no ramp: parked at the floor up to the seam, at the rim (and of no width) after it.
    return u < uSeam ? { k: K, a: depth, S: W, road: false } : { k: 0, a: 0, S: 0, road: false };
  };

  // Out over the rim to the surface road.
  const top = rampPts[0]!;
  const out = new THREE.Vector3(top.x, 0, top.z).normalize();
  const tangent = rampPts[1]!.clone().sub(top).setY(0).normalize();
  const exitDir = out.clone().multiplyScalar(0.55).addScaledVector(tangent, -0.45).normalize();
  const approach: THREE.Vector3[] = [];
  for (let i = 40; i >= 1; i--) approach.push(top.clone().addScaledVector(exitDir, i * 2));
  // Onto the floor, towards the middle.
  const last = rampPts[rampPts.length - 1]!;
  const toward = new THREE.Vector3(-last.x, 0, -last.z).normalize();
  const onFloor: THREE.Vector3[] = [];
  for (let i = 1; i <= 30; i++) onFloor.push(last.clone().addScaledVector(toward, i * 2));
  const ramp = [...approach, ...rampPts, ...onFloor];

  yield;
  /* ---------- Surface roads ---------- */
  const curve = (a: THREE.Vector3, b: THREE.Vector3, bend: THREE.Vector3) =>
    new THREE.QuadraticBezierCurve3(a, bend, b).getSpacedPoints(Math.ceil(a.distanceTo(b) / 2));
  const head = ramp[0]!.clone();
  const pad = new THREE.Vector3(padAt.x, 0, padAt.z);
  // A road that keeps a set distance outside the rim, from one bearing to another the short way round.
  const rimRoad = (from: number, to: number, off: number) => {
    let d = to - from;
    while (d > Math.PI) d -= TAU;
    while (d < -Math.PI) d += TAU;
    const pts: THREE.Vector3[] = [];
    const steps = Math.max(8, Math.ceil((Math.abs(d) * (rx + rz)) / 2 / 2));
    for (let i = 0; i <= steps; i++) {
      const phi = from + (d * i) / steps;
      const r = rimR(phi) + off;
      pts.push(new THREE.Vector3(Math.cos(phi) * r, 0, Math.sin(phi) * r));
    }
    return pts;
  };
  const join = (...parts: THREE.Vector3[][]) => parts.reduce((all, p) => (all.length ? all.concat(p.slice(1)) : p.slice()), []);
  const headPhi = Math.atan2(head.z, head.x);
  const padPhi = Math.atan2(pad.z, pad.x);
  const ringA = rimRoad(headPhi - 0.05, padPhi + 0.12, 46);
  const ringStart = ringA[0]!;
  const ringEnd = ringA[ringA.length - 1]!;
  const padGate = pad.clone().add(new THREE.Vector3(-padAt.r * 0.7, 0, -padAt.r * 0.2));
  const toPad = join(
    curve(head, ringStart, head.clone().lerp(ringStart, 0.5)),
    ringA,
    curve(ringEnd, padGate, ringEnd.clone().lerp(padGate, 0.5).add(ringEnd.clone().setY(0).normalize().multiplyScalar(25))),
  );
  const d0 = dumps[0]!;
  const dumpFoot = new THREE.Vector3(d0.x, 0, d0.z).add(new THREE.Vector3(-d0.x, 0, -d0.z).normalize().multiplyScalar(d0.r + 20));
  const outward = head.clone().setY(0).normalize();
  const toDump = curve(head, dumpFoot, head.clone().lerp(dumpFoot, 0.5).addScaledVector(outward, 45));
  const surfaceRoads = [approach.concat([top]), toPad, toDump];

  // A coarse spatial hash of road centreline points, for distance-to-road anywhere.
  const HC = 12;
  const hash = new Map<number, number[]>();
  const roadXY: number[] = [];
  const roadId: number[] = [];
  surfaceRoads.forEach((pts, id) => {
    for (const p of pts) {
      const i = roadXY.length / 2;
      roadXY.push(p.x, p.z);
      roadId.push(id);
      const key = Math.floor(p.x / HC) * 100003 + Math.floor(p.z / HC);
      const b = hash.get(key);
      if (b) b.push(i);
      else hash.set(key, [i]);
    }
  });
  // The cells whose search reaches any road: everywhere else answers at once (most of the ground is far from roads).
  const near = new Set<number>();
  for (const key of hash.keys()) {
    const gx = Math.round(key / 100003);
    const gz = key - gx * 100003;
    for (let a = -3; a <= 3; a++) for (let b = -3; b <= 3; b++) near.add((gx + a) * 100003 + gz + b);
  }
  /** Distance to the nearest surface-road centreline (Infinity beyond 40 m), skipping road `skip`. */
  const roadDist = (x: number, z: number, skip = -1) => {
    const gx = Math.floor(x / HC);
    const gz = Math.floor(z / HC);
    if (!near.has(gx * 100003 + gz)) return Infinity;
    let best = Infinity;
    for (let a = -3; a <= 3; a++) {
      for (let b = -3; b <= 3; b++) {
        const list = hash.get((gx + a) * 100003 + gz + b);
        if (!list) continue;
        for (const i of list) {
          if (roadId[i] === skip) continue;
          const d = Math.hypot(roadXY[i * 2]! - x, roadXY[i * 2 + 1]! - z);
          if (d < best) best = d;
        }
      }
    }
    return best;
  };
  /** The ground with no pit and no dumps: natural surface, graded roads and the levelled pad. */
  /** The road distance `base` last found, so a caller colouring the same point need not search again. */
  let lastRoad = Infinity;
  const base = (x: number, z: number) => {
    let y = surf(x, z);
    const d = (lastRoad = roadDist(x, z));
    if (d < 40) y = mix(y, graded(x, z), 1 - smooth((d - W / 2 - 2) / 9));
    const pr = Math.hypot(x - padAt.x, z - padAt.z);
    if (pr < padAt.r + 30) y = mix(y, 0.4, 1 - smooth((pr - padAt.r) / 30));
    return y;
  };
  /** The elevation the benches are cut from: level with the road at the ramp's head. */
  const Y0 = base(top.x, top.z) - 0.15;
  const floorY = Y0 - depth;

  yield;
  /* ---------- The pit's mesh ---------- */
  const material = rockSurfaceMaterial();
  const NB = BAND.length - 1;
  const NSEG = NB + 2 * K + 3 + NF;
  const NR = NSEG * PER;
  const FR: Record<number, number[]> = {
    [Kind.Band]: [0, 0.25, 0.5, 0.75, 1],
    [Kind.Berm]: [0, 0.1, 0.5, 0.9, 1],
    [Kind.Face]: [0, 0.3, BREAK, (BREAK + 1) / 2, 1],
    [Kind.Ramp]: [0, 0.25, 0.5, 0.75, 1],
    [Kind.Floor]: [0, 0.25, 0.5, 0.75, 1],
  };
  // The sump: a dished hollow on the floor, opposite the ramp's foot.
  const sumpAt = last.clone().multiplyScalar(-0.55);
  const SUMP = { x: sumpAt.x, z: sumpAt.z, rx: 38, rz: 26, d: 3.2 };
  const sumpDepth = (x: number, z: number) => {
    const q = Math.hypot((x - SUMP.x) / SUMP.rx, (z - SUMP.z) / SUMP.rz);
    return q >= 1 ? 0 : SUMP.d * smooth((1 - q) / 0.55);
  };
  const floorPath = [last, ...onFloor];
  const floorRoad = (x: number, z: number) => {
    let best = Infinity;
    for (let i = 0; i < floorPath.length; i += 2) best = Math.min(best, Math.hypot(floorPath[i]!.x - x, floorPath[i]!.z - z));
    return best;
  };
  const columnsN = columns;
  const pitMesh = yield* polarSteps({
    cx: 0,
    cz: 0,
    columns: columnsN,
    rows: NR,
    phi: (c) => start + turn * (uSeam + (c * TAU) / columnsN),
    R: rimR,
    material,
    fill(c, phi, R, out) {
      const u = uSeam + (c * TAU) / columnsN;
      const st = rampState(u % TAU);
      const cs = Math.cos(phi);
      const sn = Math.sin(phi);
      const xz = (D: number) => [cs * (R - D), sn * (R - D)] as const;
      // Face noise fades to nothing at the strip's two ends, so both ends of the seam are the same plain profile.
      const taper = smooth(Math.min(c, columnsN - c) / 10);
      let r = 0;
      const put = (kind: Kind, D: number, Y: number, U = NaN) => {
        out.D[r] = D;
        out.Y[r] = Y;
        out.K[r] = kind;
        out.U[r] = U;
        r++;
      };
      // Near the rim the cut cannot stand above the ground it is cut from.
      const capped = (D: number, Y: number) => {
        if (D > 30) return Y;
        const [x, z] = xz(D);
        return Math.min(Y, base(x, z));
      };
      const band = (D0: number, D1: number) => {
        for (const f of FR[Kind.Band]!) {
          const D = mix(D0, D1, f);
          const [x, z] = xz(D);
          put(Kind.Band, D, base(x, z));
        }
      };
      const rimY = (() => {
        const [x, z] = xz(0);
        return base(x, z);
      })();
      const berm = (D0: number, D1: number, y: number, first: boolean) => {
        const fr = FR[Kind.Berm]!;
        // The top berm is a setback that steps down from the ground at the rim to the first bench.
        const slope = first ? Math.max(0.9, (rimY - y) / Math.max(1, (D1 - D0) * 0.85)) : 0;
        for (let i = 0; i < PER; i++) {
          const f = fr[i]!;
          const D = mix(D0, D1, f);
          let Y = y + (first ? slope * (D1 - D) : 0);
          if (i > 0 && i < PER - 1) {
            const [x, z] = xz(D);
            Y += 0.18 * fbm(x / 6.5, z / 6.5, 2) - (i === 3 ? 0.07 : 0);
          }
          put(Kind.Berm, D, capped(D, Y));
        }
      };
      const face = (D0: number, yTop: number, h: number) => {
        const fr = FR[Kind.Face]!;
        const L = faceRun(h);
        for (let i = 0; i < PER; i++) {
          const f = fr[i]!;
          let D = D0 + L * f;
          let Y = f <= BREAK ? yTop - h * (1 - TALUS) * (f / BREAK) : yTop - h * (1 - TALUS) - h * TALUS * ((f - BREAK) / (1 - BREAK));
          if (i > 0 && i < PER - 1 && h > 0.5) {
            // Broken rock: the face bulges and recedes along the wall; the talus is lumpy.
            const arc = phi * 330;
            const nn = fbm(arc / 11 + yTop * 0.13, Y / 7, 3);
            const amp = (i === 1 ? 1.1 : i === 2 ? 0.55 : 0.35) * taper * Math.min(1, h / 6);
            D += nn * amp * 1.6;
            Y += (i === 3 ? 0.35 : 0.15) * fbm(arc / 5, Y / 3, 2) * taper;
          }
          put(Kind.Face, D, capped(D, Y));
        }
      };
      /** One half of the ramp's width; `Do` and `S` are the whole ramp's wall-side edge and width. */
      const rampSeg = (D0: number, D1: number, y: number, Do: number, S: number, road: boolean) => {
        for (const f of FR[Kind.Ramp]!) {
          const D = mix(D0, D1, f);
          const across = D - (Do + S / 2);
          // Crowned 0.3 m at the centre to shed water to both edges; level with the ground at each edge.
          const crown = S > 2 ? 0.3 * (1 - ((2 * across) / S) ** 2) : 0;
          put(Kind.Ramp, D, capped(D, y + crown), road ? across : NaN);
        }
      };
      const floor = (D0: number, D1: number) => {
        const fr = FR[Kind.Floor]!;
        for (let j = 0; j < NF; j++) {
          const a0 = mix(D0, D1, (j / NF) ** 1.45);
          const a1 = mix(D0, D1, ((j + 1) / NF) ** 1.45);
          for (let i = 0; i < PER; i++) {
            const D = mix(a0, a1, fr[i]!);
            const [x, z] = xz(D);
            const edge = smooth((D - D0) / 12);
            // The haul road across the floor from the ramp's foot is graded flat and carries wheel paths.
            const fr2 = floorRoad(x, z);
            const graded2 = 1 - smooth((fr2 - W / 2) / 6);
            put(Kind.Floor, D, floorY + edge * ((1 - graded2) * 0.3 * fbm(x / 21, z / 21, 3) - sumpDepth(x, z)), fr2 < W / 2 + 2 ? fr2 : NaN);
          }
        }
      };

      for (let j = 0; j < NB; j++) band(BAND[j]!, BAND[j + 1]!);
      const { k, a, S } = st;
      const kk = Math.min(k, K);
      for (let j = 0; j < Math.min(kk, K); j++) {
        berm(toe(j, phi), crest(j, phi), Y0 - j * H, j === 0);
        face(crest(j, phi), Y0 - j * H, H);
      }
      if (kk < K) {
        // Berm k, the face above the ramp cut down to it, the ramp, the face below it, then the rest shifted in by S.
        const Dc = crest(kk, phi);
        berm(toe(kk, phi), Dc, Y0 - kk * H, kk === 0);
        const hUp = a - kk * H;
        face(Dc, Y0 - kk * H, hUp);
        const Do = Dc + faceRun(hUp);
        rampSeg(Do, Do + S / 2, Y0 - a, Do, S, st.road);
        rampSeg(Do + S / 2, Do + S, Y0 - a, Do, S, st.road);
        face(Do + S, Y0 - a, (kk + 1) * H - a);
        for (let j = kk + 1; j < K; j++) {
          berm(toe(j, phi) + S, crest(j, phi) + S, Y0 - j * H, false);
          face(crest(j, phi) + S, Y0 - j * H, H);
        }
        floor(toe(K, phi) + S, R - 6);
      } else {
        // The ramp has reached the floor: three stretches of floor where it lands, then the floor.
        const D0 = toe(K, phi);
        rampSeg(D0, D0 + S / 2, floorY, D0, 0, false);
        rampSeg(D0 + S / 2, D0 + S, floorY, D0, 0, false);
        face(D0 + S, floorY, 0);
        floor(D0 + S, R - 6);
      }
      if (r !== NR) throw new Error(`pit column ${c}: ${r} rows, expected ${NR}`);
    },
    colour(x, y, z, ny, kind, out) {
      const v = clamp(0.5 + 0.9 * fbm(x / 55, z / 55, 3), 0, 0.999);
      if (kind === Kind.Band) {
        blend(SOIL, v, out);
      } else {
        strata(Math.max(0, Y0 + 2 - y), v, out);
        // Rock units dip across the wall, so the same unit shows on bench after bench.
        const unit = (x * 0.8 + z * 0.6) / 95 + y / 70 + 0.9 * geo.fbm(x / 260, z / 260, 3);
        const ui = ((Math.floor(unit) % UNITS.length) + UNITS.length) % UNITS.length;
        const uf = unit - Math.floor(unit);
        t2.copy(UNITS[ui]!).lerp(UNITS[(ui + 1) % UNITS.length]!, smooth((uf - 0.85) / 0.15));
        out.multiply(t2);
      }
      // Berms and floor gather dust; faces stay the colour of the rock.
      if (kind !== Kind.Face) out.lerp(DUST, smooth((ny - 0.8) / 0.15) * 0.3);
      if (kind === Kind.Ramp) out.lerp(ROAD, 0.75);
      if (kind === Kind.Floor) {
        // Damp near the sump.
        const q = Math.hypot((x - SUMP.x) / (SUMP.rx * 1.6), (z - SUMP.z) / (SUMP.rz * 1.6));
        out.lerp(t2.set("#5d5247"), 0.42 * (1 - smooth((q - 0.6) / 0.5)) + 0.12);
      }
    },
  });
  const group = new THREE.Group();
  group.name = "pit";
  group.add(pitMesh.mesh);

  yield;
  /* ---------- Waste dumps, in lifts ---------- */
  const dumpMeshes: { mesh: PolarMesh; d: (typeof dumps)[number]; R: (phi: number) => number }[] = [];
  const DUMP_BAND = [-28, -16, -8, -3, 0];
  const REPOSE_D = Math.tan(deg(37));
  for (const [di, d] of dumps.entries()) {
    yield;
    const lifts = Math.max(1, Math.round(d.h / 20));
    const hl = d.h / lifts;
    const fr = hl / REPOSE_D;
    const setback = 14;
    const Rd = (phi: number) => d.r * (1 + 0.07 * fbm(Math.cos(phi) * 1.6 + di * 7.7, Math.sin(phi) * 1.6 - di * 3.3, 3));
    const nInt = DUMP_BAND.length - 1 + lifts + (lifts - 1) + 4;
    const cols = Math.max(240, Math.round((TAU * d.r) / 2.6));
    const dm = yield* polarSteps({
      cx: d.x,
      cz: d.z,
      columns: cols,
      rows: nInt * PER,
      phi: (c) => (c * TAU) / cols,
      R: Rd,
      material,
      fill(_c, phi, R, out) {
        let r = 0;
        const cs = Math.cos(phi);
        const sn = Math.sin(phi);
        const put = (kind: Kind, D: number, rise: number) => {
          const x = d.x + cs * (R - D);
          const z = d.z + sn * (R - D);
          out.D[r] = D;
          out.Y[r] = base(x, z) + rise;
          out.K[r] = kind;
          r++;
        };
        for (let j = 0; j < DUMP_BAND.length - 1; j++) for (const f of FR[Kind.Band]!) put(Kind.Band, mix(DUMP_BAND[j]!, DUMP_BAND[j + 1]!, f), 0);
        let D = 0;
        for (let L = 0; L < lifts; L++) {
          for (let i = 0; i < PER; i++) {
            const f = i / (PER - 1);
            // Tipped faces stand at the angle of repose, a little concave at the foot.
            const lump = i > 0 && i < PER - 1 ? 0.5 * fbm((cs * d.r) / 7 + L * 5 + f * 3, (sn * d.r) / 7, 2) : 0;
            put(Kind.Face, D + fr * f, L * hl + hl * f * (0.92 + 0.08 * f) + lump);
          }
          D += fr;
          if (L < lifts - 1) {
            for (const f of FR[Kind.Berm]!) put(Kind.Berm, D + setback * f, (L + 1) * hl);
            D += setback;
          }
        }
        const end = R - 5;
        for (let j = 0; j < 4; j++) {
          for (const f of FR[Kind.Floor]!) {
            const DD = mix(D, end, (j + f) / 4);
            const x = d.x + cs * (R - DD);
            const z = d.z + sn * (R - DD);
            // The top is graded to shed water off its edges, with low mounds where loads were paddock-dumped.
            const crown = 0.012 * (DD - D);
            const mounds = (j + f) / 4 > 0.02 ? 1.3 * Math.max(0, fbm(x / 9, z / 9, 2)) : 0;
            out.D[r] = DD;
            out.Y[r] = base(x, z) + d.h + crown + mounds;
            out.K[r] = Kind.Floor;
            r++;
          }
        }
      },
      colour(x, _y, z, ny, kind, out) {
        const v = clamp(0.5 + 1.1 * fbm(x / 23, z / 23, 3), 0, 0.999);
        if (kind === Kind.Band) blend(SOIL, clamp(0.5 + 0.9 * fbm(x / 55, z / 55, 3), 0, 0.999), out);
        else {
          blend(WASTE, v, out);
          // Each truckload tips its own colour down the face: streaks run downhill.
          if (kind === Kind.Face) {
            const a = Math.atan2(z - d.z, x - d.x);
            out.multiplyScalar(0.9 + 0.2 * fbm(Math.cos(a) * d.r * 0.25, Math.sin(a) * d.r * 0.25, 2));
          }
          else out.lerp(DUST, smooth((ny - 0.85) / 0.1) * 0.25);
        }
      },
    });
    group.add(dm.mesh);
    dumpMeshes.push({ mesh: dm, d, R: Rd });
  }

  yield;
  /* ---------- The base ground, with the pit and dumps cut out ---------- */
  const baseMat = rockSurfaceMaterial();
  // Where the base overlaps the pit's or a dump's outer band, the base wins the depth test.
  baseMat.polygonOffset = true;
  baseMat.polygonOffsetFactor = -1;
  baseMat.polygonOffsetUnits = -2;
  // The outline's radius stays within these bounds, so most points are settled without evaluating it.
  const rimLo = Math.min(rx, rz) * 0.9;
  const rimHi = Math.max(rx, rz) * 1.1;
  const pitCore = (x: number, z: number) => {
    const h = Math.hypot(x, z);
    if (h > rimHi + E) return false;
    if (h < rimLo + E - 4) return true;
    return rimR(Math.atan2(z, x)) - h > -E + 4;
  };
  const dumpCore = (x: number, z: number) =>
    dumpMeshes.some(({ d, R }) => {
      const h = Math.hypot(x - d.x, z - d.z);
      const reach = -DUMP_BAND[0]! - 4;
      if (h > d.r * 1.12 + reach) return false;
      if (h < d.r * 0.88 + reach) return true;
      return h < R(Math.atan2(z - d.z, x - d.x)) + reach;
    });
  const cored = (x: number, z: number) => pitCore(x, z) || dumpCore(x, z);

  // The base ground needs no finer cells than about 3 m: the pit, dumps and roads carry their own detail.
  const baseCells = Math.min(cells, 384);
  const n1 = baseCells + 1;
  const cell = extent / baseCells;
  const half = extent / 2;
  const heights = new Float32Array(n1 * n1);
  {
    const pos = new Float32Array(n1 * n1 * 3);
    const col = new Float32Array(n1 * n1 * 3);
    const roadA = new Float32Array(n1 * n1 * 2);
    const uv = new Float32Array(n1 * n1 * 2);
    const core = new Uint8Array(n1 * n1);
    const c = new THREE.Color();
    for (let j = 0; j < n1; j++) {
      for (let k = 0; k < n1; k++) {
        const x = k * cell - half;
        const z = j * cell - half;
        const i = j * n1 + k;
        const y = base(x, z);
        heights[i] = y;
        core[i] = cored(x, z) ? 1 : 0;
        pos[i * 3] = x;
        pos[i * 3 + 1] = y;
        pos[i * 3 + 2] = z;
        uv[i * 2] = x * 0.12;
        uv[i * 2 + 1] = z * 0.12;
        blend(SOIL, clamp(0.5 + 0.9 * fbm(x / 55, z / 55, 3), 0, 0.999), c);
        const rd = lastRoad;
        if (rd < W / 2 + 6) {
          const w = 1 - smooth((rd - W / 2 + 1) / 5);
          c.lerp(ROAD, w * 0.8);
          roadA[i * 2] = rd;
          roadA[i * 2 + 1] = w;
        }
        const pr = Math.hypot(x - padAt.x, z - padAt.z);
        if (pr < padAt.r + 10) c.lerp(ROAD, (1 - smooth((pr - padAt.r + 6) / 12)) * 0.55);
        col[i * 3] = c.r;
        col[i * 3 + 1] = c.g;
        col[i * 3 + 2] = c.b;
      }
    }
    const index: number[] = [];
    for (let j = 0; j < baseCells; j++) {
      for (let k = 0; k < baseCells; k++) {
        const a = j * n1 + k;
        const b = a + n1;
        if (core[a] && core[a + 1] && core[b] && core[b + 1]) continue;
        index.push(a, b, a + 1, a + 1, b, b + 1);
      }
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    g.setAttribute("color", new THREE.BufferAttribute(col, 3));
    g.setAttribute("aRoad", new THREE.BufferAttribute(roadA, 2));
    g.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
    g.setIndex(index);
    g.computeVertexNormals();
    const m = new THREE.Mesh(g, baseMat);
    m.receiveShadow = true;
    m.castShadow = true;
    group.add(m);
  }

  yield;
  /* ---------- Out to the horizon ---------- */
  const far = noise2(seed + 11);
  const outer = (x: number, z: number) => {
    const r = Math.hypot(x, z);
    // Hills grow with distance; mountains stand on the horizon.
    const hills = 18 * far.fbm(x / 900, z / 900, 4) * smooth((r - 700) / 900);
    const ranges = 260 * Math.max(0, far.fbm(x / 2600 + 4, z / 2600 - 2, 5) + 0.12) * smooth((r - 2600) / 2400);
    return base(x, z) + hills + ranges;
  };
  const squareAt = (x: number, z: number) => {
    const fx = (x + half) / cell;
    const fz = (z + half) / cell;
    const k = Math.min(baseCells - 1, Math.floor(fx));
    const j = Math.min(baseCells - 1, Math.floor(fz));
    const tx = fx - k;
    const tz = fz - j;
    return mix(mix(heights[j * n1 + k]!, heights[j * n1 + k + 1]!, tx), mix(heights[(j + 1) * n1 + k]!, heights[(j + 1) * n1 + k + 1]!, tx), tz);
  };
  const heightAt = (x: number, z: number) => {
    if (pitCore(x, z) || rimR(Math.atan2(z, x)) - Math.hypot(x, z) > -E + 1) {
      const h = pitMesh.heightAt(x, z);
      if (Number.isFinite(h)) return h;
    }
    for (const { mesh, d, R } of dumpMeshes) {
      const rr = Math.hypot(x - d.x, z - d.z);
      if (rr < R(Math.atan2(z - d.z, x - d.x)) - DUMP_BAND[0]! - 1) {
        const h = mesh.heightAt(x, z);
        if (Number.isFinite(h)) return h;
      }
    }
    if (Math.abs(x) > half || Math.abs(z) > half) return outer(x, z);
    return squareAt(x, z);
  };
  if (o.surround !== false) {
    // One grid out to the horizon whose lines start at twice the detailed spacing at the square's edge and widen
    // with distance, so the waste dumps and the pad keep their shape and the seam with the square stays tight.
    const axis: number[] = [];
    {
      const inner = Math.max(2, Math.round(baseCells / 2));
      for (let i = 0; i <= inner; i++) axis.push(-half + (i * extent) / inner);
      let d = 0;
      const s0 = cell * 2;
      while (d < 8200) {
        d += Math.min(420, s0 + 0.075 * d);
        axis.push(half + d);
        axis.unshift(-half - d);
      }
    }
    const m = axis.length;
    const verts = new Float32Array(m * m * 3);
    const cols = new Float32Array(m * m * 3);
    const uvs = new Float32Array(m * m * 2);
    const core = new Uint8Array(m * m);
    const index: number[] = [];
    const c = new THREE.Color();
    const tmp = new THREE.Color();
    for (let j = 0; j < m; j++) {
      for (let k = 0; k < m; k++) {
        const x = axis[k]!;
        const z = axis[j]!;
        const i = j * m + k;
        const edge = Math.abs(x) <= half + 0.01 && Math.abs(z) <= half + 0.01;
        // Points strictly inside the detailed square belong to no drawn cell; they only hold the grid together.
        const within = Math.abs(x) < half - 0.01 && Math.abs(z) < half - 0.01;
        const y = edge ? squareAt(clamp(x, -half, half), clamp(z, -half, half)) - 0.35 : outer(x, z);
        core[i] = !within && cored(x, z) ? 1 : 0;
        verts[i * 3] = x;
        verts[i * 3 + 1] = y;
        verts[i * 3 + 2] = z;
        uvs[i * 2] = x * 0.12;
        uvs[i * 2 + 1] = z * 0.12;
        if (within) c.copy(SOIL[0]!);
        else blend(SOIL, clamp(0.5 + 0.9 * far.fbm(x / 400, z / 400, 3), 0, 0.999), c);
        if (y > 70) c.lerp(tmp.set("#8d7560"), smooth((y - 70) / 140) * 0.6);
        cols[i * 3] = c.r;
        cols[i * 3 + 1] = c.g;
        cols[i * 3 + 2] = c.b;
      }
    }
    for (let j = 0; j < m - 1; j++) {
      for (let k = 0; k < m - 1; k++) {
        // Skip the cells the detailed square already covers, and those deep inside a dump.
        if (axis[k]! >= -half - 0.01 && axis[k + 1]! <= half + 0.01 && axis[j]! >= -half - 0.01 && axis[j + 1]! <= half + 0.01) continue;
        const a = j * m + k;
        if (core[a] && core[a + 1] && core[a + m] && core[a + m + 1]) continue;
        index.push(a, a + m, a + 1, a + 1, a + m, a + m + 1);
      }
    }
    const g2 = new THREE.BufferGeometry();
    g2.setAttribute("position", new THREE.BufferAttribute(verts, 3));
    g2.setAttribute("color", new THREE.BufferAttribute(cols, 3));
    g2.setAttribute("uv", new THREE.BufferAttribute(uvs, 2));
    g2.setIndex(index);
    g2.computeVertexNormals();
    const ring = new THREE.Mesh(g2, baseMat);
    ring.receiveShadow = true;
    group.add(ring);
  }

  yield;
  /* ---------- The sump ---------- */
  {
    // Water stands 1.3 m deep at the middle of the hollow; its shore is where the hollow's side is steep, so the
    // surface never lies along the ground.
    const water = new THREE.Mesh(
      new THREE.CircleGeometry(1, 72),
      // Pit water: silty, so it is dull green-brown and reflects only a little sky.
      new THREE.MeshPhysicalMaterial({ color: "#4a5a48", roughness: 0.22, metalness: 0, clearcoat: 0.4, clearcoatRoughness: 0.3, transparent: true, opacity: 0.93, depthWrite: false, envMapIntensity: 0.35 }),
    );
    water.rotation.x = -Math.PI / 2;
    water.position.set(SUMP.x, floorY - SUMP.d + 1.3, SUMP.z);
    water.scale.set(SUMP.rx * 0.9, SUMP.rz * 0.9, 1);
    water.renderOrder = 2;
    water.receiveShadow = true;
    group.add(water);
  }

  /* ---------- Windrows ---------- */
  const windrows: THREE.Vector3[][] = [];
  {
    // On the ramp's open edge, its centre 2.9 m in from it.
    const rampRow: THREE.Vector3[] = [];
    for (let i = 0; i < rampPts.length; i++) {
      const phi = rampPhi[i]!;
      const a = rampA[i]!;
      const Di = rampOuter(phi, a) + W - 2.9;
      const R = rimR(phi);
      const x = Math.cos(phi) * (R - Di);
      const z = Math.sin(phi) * (R - Di);
      const across = W / 2 - 2.9;
      rampRow.push(new THREE.Vector3(x, Y0 - a + 0.3 * (1 - ((2 * across) / W) ** 2), z));
    }
    // The last few metres before the floor stay open.
    windrows.push(rampRow.slice(8, Math.max(9, rampRow.length - 10)));
    // Along both edges of the surface roads, open where roads meet, at the pad and near the rim.
    const edge = (pts: THREE.Vector3[], id: number, side: 1 | -1) => {
      const runs: THREE.Vector3[][] = [];
      let cur: THREE.Vector3[] = [];
      for (let i = 0; i < pts.length; i++) {
        const p = pts[i]!;
        const q = pts[Math.min(pts.length - 1, i + 1)]!;
        const pp = pts[Math.max(0, i - 1)]!;
        const tx = q.x - pp.x;
        const tz = q.z - pp.z;
        const tl = Math.hypot(tx, tz) || 1;
        const x = p.x + (-tz / tl) * side * (W / 2 - 2.4);
        const z = p.z + (tx / tl) * side * (W / 2 - 2.4);
        const ok =
          roadDist(x, z, id) > W / 2 + 4 &&
          Math.hypot(x - padAt.x, z - padAt.z) > padAt.r + 26 &&
          rimR(Math.atan2(z, x)) - Math.hypot(x, z) < -18 &&
          !dumpMeshes.some(({ d, R }) => Math.hypot(x - d.x, z - d.z) < R(Math.atan2(z - d.z, x - d.x)) + 26);
        if (ok) cur.push(new THREE.Vector3(x, 0, z));
        else if (cur.length) {
          if (cur.length > 6) runs.push(cur);
          cur = [];
        }
      }
      if (cur.length > 6) runs.push(cur);
      return runs;
    };
    for (const [id, pts] of [toPad, toDump].entries()) {
      for (const side of [1, -1] as const) for (const run of edge(pts, id + 1, side)) windrows.push(run.filter((_, i) => i % 2 === 0));
    }
    // A bund around the pit, 14 m back from the crest, open where the ramp road crosses it.
    const bund: THREE.Vector3[] = [];
    const runs: THREE.Vector3[][] = [];
    const steps = 900;
    for (let i = 0; i <= steps; i++) {
      const phi = (i / steps) * TAU;
      const R = rimR(phi) + 14;
      const x = Math.cos(phi) * R;
      const z = Math.sin(phi) * R;
      if (roadDist(x, z) > W / 2 + 5) bund.push(new THREE.Vector3(x, 0, z));
      else if (bund.length) {
        runs.push(bund.splice(0));
      }
    }
    if (bund.length) runs.push(bund);
    for (const r of runs) if (r.length > 6) windrows.push(r);
  }
  const lift = (pts: THREE.Vector3[]) => pts.map((p) => new THREE.Vector3(p.x, heightAt(p.x, p.z), p.z));
  for (let i = 1; i < windrows.length; i++) windrows[i] = lift(windrows[i]!);
  group.add(windrowMesh(windrows, material, seed + 17));

  /* ---------- Design strings ---------- */
  const crests: THREE.Vector3[][] = [];
  const toes: THREE.Vector3[][] = [];
  {
    const step = 4;
    for (let k = 0; k < K; k++) {
      const cr: THREE.Vector3[] = [];
      const to: THREE.Vector3[] = [];
      for (let c = 0; c <= columnsN; c += step) {
        const rows = pitMesh.column(c);
        const phi = pitMesh.phiOf(c);
        const R = rimR(phi);
        // The crest of bench k is the first row at its elevation that starts a face; the toe, the last face row
        // that reaches the bench below.
        for (let rr = 0; rr < NR; rr++) {
          if (rows.K[rr] === Kind.Face && rr % PER === 0 && Math.abs(rows.Y[rr]! - (Y0 - k * H)) < 0.6 && rows.Y[rr + PER - 1]! < rows.Y[rr]! - 1) {
            const D = rows.D[rr]!;
            cr.push(new THREE.Vector3(Math.cos(phi) * (R - D), rows.Y[rr]!, Math.sin(phi) * (R - D)));
            break;
          }
        }
        for (let rr = NR - 1; rr >= 0; rr--) {
          if (rows.K[rr] === Kind.Face && rr % PER === PER - 1 && Math.abs(rows.Y[rr]! - (Y0 - (k + 1) * H)) < 0.6 && rows.Y[rr - PER + 1]! > rows.Y[rr]! + 1) {
            const D = rows.D[rr]!;
            to.push(new THREE.Vector3(Math.cos(phi) * (R - D), rows.Y[rr]!, Math.sin(phi) * (R - D)));
            break;
          }
        }
      }
      crests.push(cr);
      toes.push(to);
    }
  }

  const benches: number[] = [];
  for (let k = 0; k < K; k++) benches.push(Y0 - k * H);
  return {
    group,
    heightAt,
    ramp: lift(ramp),
    toPad: lift(toPad),
    toDump: lift(toDump),
    floor: { y: floorY, centre: new THREE.Vector3(0, floorY, 0), radius: Math.min(rx, rz) - depth / Math.tan(deg(o.overall ?? 45)) - W },
    pad: new THREE.Vector3(padAt.x, heightAt(padAt.x, padAt.z), padAt.z),
    benches,
    depth,
    rx,
    rz,
    bench: H,
    datum: Y0,
    inside: (x, z) => Math.hypot(x, z) < rimR(Math.atan2(z, x)),
    berm(phi, d) {
      const c = pitMesh.nearest(phi);
      const rows = pitMesh.column(c);
      const want = Y0 - Math.round(d / H) * H;
      for (let rr = 0; rr < NR; rr += PER) {
        if (rows.K[rr] !== Kind.Berm || Math.abs(rows.Y[rr]! - want) > 0.8) continue;
        const D = (rows.D[rr]! + rows.D[rr + PER - 1]!) / 2;
        const p = pitMesh.phiOf(c);
        const R = rimR(p);
        const x = Math.cos(p) * (R - D);
        const z = Math.sin(p) * (R - D);
        return new THREE.Vector3(x, heightAt(x, z), z);
      }
      return null;
    },
    strings: { crests, toes },
    windrows,
    material,
  };
}
