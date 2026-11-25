/**
 * The ground for the blast figure: a curved pushback in a benched pit at real scale, built as one heightfield on a
 * rectilinear grid that is half a metre apart over the blast and widens with distance (with a second fine patch
 * where the blast crew waits at the firing point). The blast zone carries two surfaces, the bench as drilled and the
 * muck pile after the blast, and `blast(t)` morphs between them hole by hole as the pattern fires.
 *
 * The rock takes the pit's strata colours from terrain.ts (oxidised near the rim, grey fresh rock at the working
 * level), darkened in hollows and at the toes of faces, and a material of its own that adds broken-rock relief from
 * every side (triplanar), so faces read as blocky rock rather than stretched texture.
 *
 * Units are metres, y up. The working bench top is y = 0, the floor below it y = −15, the pit rim y = 75.
 */
import * as THREE from "three";
import { BACK, BENCH, holes, PATTERN, SPAN, TOE, toBench } from "./blast-plan";
import { noiseTexture } from "./fleet-kit";
import { clamp, mix, rng, smooth } from "./lib";
import { noise2 } from "./stage";
import { strata } from "./terrain";

/** Crest positions (s) of the faces from the rim down: five benches of wall behind the working bench, its face, then the benches below the floor. */
const CRESTS = [-118, -104, -90, -76, -62, 0, 95, 128, 160, 190];
export const RIM = 75;
const RUN = 5.2;
/** The ramp from the working bench down to the floor, east of the blast. */
const RAMP = { u0: 150, u1: 300, inner: -24, outer: 6 };

const fn = noise2(19);
const bn = noise2(23);

/** Fraction of the way down a face, 0 at the crest, 1 at the toe, sharp at both with a steep middle. */
const faceFrac = (t: number) => {
  const f = clamp(t, 0, 1);
  return f * f * (3 - 2 * f) * 0.35 + f * 0.65;
};

/** The pit as drilled: benches round the curve, the ramp, a windrow on the floor's edge, the land beyond the rim. */
export function preAt(x: number, z: number): number {
  const [u, s] = toBench(x, z);
  let y = RIM;
  for (let k = 0; k < CRESTS.length; k++) {
    const c0 = CRESTS[k]!;
    if (s < c0 - 3.5) break;
    if (s > c0 + RUN + 3.5) {
      y -= 15;
      continue;
    }
    // Crests wander and the faces are broken, as blasted faces are: a long sway, blocky steps, and small breakage.
    const block = Math.round(bn.fbm(u / 9 + k * 5.3, k * 2.1, 2) * 4) / 4;
    const c = c0 + 1.4 * fn.fbm(u / 26 + k * 7.1, k * 3.3, 3) + 0.7 * block + 0.45 * fn.fbm(u / 4.5, k * 1.7 + 4, 2) + 0.3 * bn.fbm(u / 1.6, k * 4.1, 2);
    // The face is not one plane: it steps in and out down its height, so it varies with depth on the face too.
    const t0 = (s - c) / RUN;
    const t = t0 + (t0 > 0 && t0 < 1 ? (0.22 * bn.fbm(u / 3.4 + k * 3, t0 * 2.6, 3) + 0.1 * bn.fbm(u / 1.3, t0 * 6 + k, 2)) * Math.sin(Math.PI * t0) : 0);
    let d = t > 0 ? 15 * faceFrac(t) : 0;
    // Ledges and bulges down the face, strongest mid-face.
    if (t > -0.15 && t < 1.15) d -= 0.9 * fn.fbm(u / 3.1, (s + k * 40) / 2.4, 3) * Math.sin(Math.PI * clamp(t, 0, 1));
    // Scree: loose rock fallen from the face lies in a wedge at its toe, at the angle of repose.
    const toe = c + RUN;
    const hs = 2 + 1.4 * fn.fbm(u / 8 + k, 9.1, 2) - Math.max(0, s - toe) * 0.75 + Math.min(0, s - toe) * 0.3;
    if (hs > 0 && s > toe - 4) d = Math.min(d, 15 - hs);
    y -= clamp(d, 0, 15);
  }
  if (s < CRESTS[0]! - 4) y += 3.5 * fn.fbm(x / 260, z / 260, 3) * smooth((CRESTS[0]! - 4 - s) / 60);
  // The ramp: level across, at a 10% grade along, cut into the bench and built out over the face.
  if (u > RAMP.u0 - 20) {
    const ry = -15 * clamp((u - RAMP.u0) / (RAMP.u1 - RAMP.u0), 0, 1);
    const band = smooth((s - (RAMP.inner - 3)) / 3) * (1 - smooth((s - RAMP.outer) / 4)) * smooth((u - (RAMP.u0 - 20)) / 15);
    y = mix(y, ry, band);
    // A windrow along the open edge of the ramp.
    if (ry < -0.5) y += 1.6 * Math.exp(-(((s - (RAMP.outer - 1.4)) / 1.1) ** 2)) * smooth((u - RAMP.u0) / 15);
  }
  // A safety windrow along the floor's open edge.
  if (s > 86 && s < 96) y += (1.5 + 0.3 * fn.fbm(u / 6, 3.3, 2)) * Math.exp(-(((s - 92.2) / 1.05) ** 2));
  // Bench tops and floors are not quite flat.
  y += 0.1 * fn.fbm(x / 7, z / 7, 2) + 0.05 * fn.fbm(x / 1.7, z / 1.7, 2);
  // Hills on the horizon.
  const rc = Math.hypot(x, z - 420);
  if (rc > 1100) y += 140 * Math.max(0, fn.fbm(x / 1900 + 3, z / 1900 - 1, 4) + 0.15) * smooth((rc - 1100) / 1500);
  return y;
}

/* ---------- The muck pile ---------- */
const mn = noise2(77);
const toeAt = (u: number) => TOE + 3.5 * mn.fbm(u / 30, 2.2, 2);
/** Half the blast's width at a distance s from the crest: the ends hold behind the crest and spread in front of it. */
const halfAt = (s: number) => SPAN.half + (s > 0 ? 6 * (s / TOE) : 0);
/** Profile of the muck from the back break (v = 0) to the toe (v = 1): level over the old bench, then down. */
const shape = (v: number) => {
  if (v < 0.1) return 0.86 + 0.14 * smooth(v / 0.1);
  if (v < 0.42) return 1 - 0.05 * ((v - 0.1) / 0.32);
  // A rounded crest, a slope near the angle of repose, a long thin toe.
  return 0.95 * (1 - smooth((v - 0.42) / 0.58));
};
/** Crest height above the floor that holds a 30% swell over the bench's cross-section. */
const MUCK_A = (() => {
  const L = TOE - BACK;
  let area = 0;
  const n = 400;
  for (let i = 0; i < n; i++) area += shape((i + 0.5) / n) * (L / n);
  const before = BENCH.H * -BACK + 0.5 * BENCH.H * RUN;
  return (1.3 * before) / area;
})();

/** Whether a bench point lies in the blasted rock (behind the crest) or under the thrown muck (in front of it). */
export function inBlast(u: number, s: number) {
  return s >= BACK && s <= toeAt(u) && Math.abs(u - SPAN.mid) <= halfAt(s);
}

export function postAt(x: number, z: number, pre = preAt(x, z)): number {
  const [u, s] = toBench(x, z);
  const back = BACK + 0.6 * mn.fbm(u / 9, 0.5, 2);
  if (s < back || s > toeAt(u) + 2) return pre;
  const half = halfAt(s);
  const d = Math.abs(u - SPAN.mid);
  if (s <= 0.5 && d > half) return pre;
  const v = clamp((s - back) / (toeAt(u) - back), 0, 1);
  // Behind the crest the unblasted rock walls the muck in; in front it spreads and thins out to the sides.
  const lat = s <= 0.5 ? 0.86 + 0.14 * smooth((half - d) / 9) : smooth((half - d) / 12);
  // Lumps at every scale: swells where a row threw harder, then blocks and rubble.
  const lumps = 0.9 * mn.fbm(u / 7, s / 7, 3) + 0.35 * mn.fbm(u / 2.2, s / 2.2, 2) + 0.22 * mn.fbm(u / 1.05, s / 1.05, 2);
  const h = MUCK_A * shape(v) * lat;
  const y = -BENCH.H + h + lumps * Math.min(1, h / 3);
  // A crack opens at the back break where the muck pulls away from the wall behind.
  const crack = 0.9 * Math.exp(-(((s - back - 0.6) / 0.7) ** 2));
  return s > 0.5 ? Math.max(pre, y) : y - crack;
}

/* ---------- The grid ---------- */
/** One axis: fine and even over [a, b], widening outwards, with extra detail in the focus intervals. */
function axis(a: number, b: number, cell: number, lo: number, hi: number, focus: [number, number, number][], g: number, q: number, cap: number) {
  const pts: number[] = [];
  const n = Math.round((b - a) / cell);
  for (let i = 0; i <= n; i++) pts.push(a + (i * (b - a)) / n);
  const stepAt = (p: number) => {
    const d = p < a ? a - p : p > b ? p - b : 0;
    let st = Math.min(cap, cell + g * d + q * d * d);
    for (const [fa, fb, fc] of focus) {
      const e = p < fa ? fa - p : p > fb ? p - fb : 0;
      st = Math.min(st, fc + 0.08 * e);
    }
    return st;
  };
  for (let p = b; p < hi; ) {
    p += stepAt(p);
    pts.push(Math.min(p, hi));
  }
  const left: number[] = [];
  for (let p = a; p > lo; ) {
    p -= stepAt(p);
    left.push(Math.max(p, lo));
  }
  return left.reverse().concat(pts);
}

/** For each grid line, the index of the line about `r` metres either side. */
function reach(arr: number[], r: number) {
  const lo = new Int32Array(arr.length);
  const hi = new Int32Array(arr.length);
  let a = 0;
  let b = 0;
  for (let i = 0; i < arr.length; i++) {
    while (a < i && arr[i]! - arr[a + 1]! >= r) a++;
    while (b < arr.length - 1 && arr[b]! - arr[i]! < r) b++;
    lo[i] = Math.min(a, Math.max(0, i - 1));
    hi[i] = Math.max(b, Math.min(arr.length - 1, i + 1));
  }
  return { lo, hi };
}

/* ---------- Broken rock, as a normal map ---------- */
let rubbleTex: THREE.DataTexture | null = null;
/** Periodic gradient noise on an S×S texture with `per` cells across, in [−1, 1]. */
function periodicNoise(S: number, per: number, r: () => number) {
  const gx = new Float32Array(per * per);
  const gy = new Float32Array(per * per);
  for (let i = 0; i < per * per; i++) {
    const a = r() * Math.PI * 2;
    gx[i] = Math.cos(a);
    gy[i] = Math.sin(a);
  }
  const out = new Float32Array(S * S);
  const fade = (t: number) => t * t * t * (t * (t * 6 - 15) + 10);
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const fx = (x / S) * per;
      const fy = (y / S) * per;
      const ix = Math.floor(fx);
      const iy = Math.floor(fy);
      const tx = fx - ix;
      const ty = fy - iy;
      const dot = (cx: number, cy: number, dx: number, dy: number) => {
        const k = (((cy % per) + per) % per) * per + (((cx % per) + per) % per);
        return gx[k]! * dx + gy[k]! * dy;
      };
      const a = dot(ix, iy, tx, ty);
      const b = dot(ix + 1, iy, tx - 1, ty);
      const c = dot(ix, iy + 1, tx, ty - 1);
      const d = dot(ix + 1, iy + 1, tx - 1, ty - 1);
      const u = fade(tx);
      const v = fade(ty);
      out[y * S + x] = mix(mix(a, b, u), mix(c, d, u), v) * 1.4;
    }
  }
  return out;
}
/**
 * A tileable map of broken rock: angular facets (Voronoi cells, each a tilted plane) under a turbulence of creases
 * and fractures. Red and green hold the normal (x, y), blue a tone that changes from block to block and patch to
 * patch, alpha how far a texel is from a crease (0 in one).
 */
export function rubbleNormal() {
  if (rubbleTex) return rubbleTex;
  const S = 256;
  const cells = 9;
  const r = rng(91);
  const pts = new Float32Array(cells * cells * 6);
  for (let i = 0; i < cells * cells; i++) {
    pts[i * 6] = 0.04 + r() * 0.92;
    pts[i * 6 + 1] = 0.04 + r() * 0.92;
    pts[i * 6 + 2] = r() * 0.4;
    pts[i * 6 + 3] = (r() - 0.5) * 1.6;
    pts[i * 6 + 4] = (r() - 0.5) * 1.6;
    pts[i * 6 + 5] = r();
  }
  const oct = [4, 8, 16, 32].map((per) => periodicNoise(S, per, r));
  const patch = periodicNoise(S, 3, r);
  const h = new Float32Array(S * S);
  const gap = new Float32Array(S * S);
  const tone = new Float32Array(S * S);
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const i = y * S + x;
      const fx = (x / S) * cells;
      const fy = (y / S) * cells;
      const cx = Math.floor(fx);
      const cy = Math.floor(fy);
      let d1 = 9;
      let d2 = 9;
      let best = 0;
      let bx = 0;
      let by = 0;
      for (let oy = -1; oy <= 1; oy++) {
        for (let ox = -1; ox <= 1; ox++) {
          const gx = (cx + ox + cells) % cells;
          const gy = (cy + oy + cells) % cells;
          const k = (gy * cells + gx) * 6;
          const px = cx + ox + pts[k]!;
          const py = cy + oy + pts[k + 1]!;
          const dd = Math.hypot(px - fx, py - fy);
          if (dd < d1) {
            d2 = d1;
            d1 = dd;
            best = k;
            bx = px;
            by = py;
          } else if (dd < d2) d2 = dd;
        }
      }
      const edge = smooth((d2 - d1) / 0.12);
      const facet = pts[best + 2]! + pts[best + 3]! * (fx - bx) * 0.32 + pts[best + 4]! * (fy - by) * 0.32;
      // Turbulence: sharp V-shaped creases where each octave crosses zero.
      let turb = 0;
      let amp = 1;
      for (const o of oct) {
        turb += amp * Math.abs(o[i]!) ** 0.85;
        amp *= 0.5;
      }
      h[i] = facet * (0.6 + 0.4 * edge) + turb * 0.55;
      gap[i] = smooth(Math.abs(oct[0]![i]!) / 0.09) * smooth(Math.abs(oct[1]![i]!) / 0.07) * (0.5 + 0.5 * edge);
      tone[i] = clamp(0.5 + 0.55 * patch[i]! + (pts[best + 5]! - 0.5) * 0.35, 0, 1);
    }
  }
  const d = new Uint8Array(S * S * 4);
  const k = 4.2;
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const hx = h[y * S + ((x + 1) % S)]! - h[y * S + ((x - 1 + S) % S)]!;
      const hy = h[((y + 1) % S) * S + x]! - h[((y - 1 + S) % S) * S + x]!;
      const nx = -hx * k;
      const ny = -hy * k;
      const l = Math.hypot(nx, ny, 1);
      const i = (y * S + x) * 4;
      d[i] = Math.round(((nx / l) * 0.5 + 0.5) * 255);
      d[i + 1] = Math.round(((ny / l) * 0.5 + 0.5) * 255);
      d[i + 2] = Math.round(tone[y * S + x]! * 255);
      d[i + 3] = Math.round(gap[y * S + x]! * 255);
    }
  }
  const t = new THREE.DataTexture(d, S, S, THREE.RGBAFormat);
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.magFilter = THREE.LinearFilter;
  t.minFilter = THREE.LinearMipmapLinearFilter;
  t.generateMipmaps = true;
  t.anisotropy = 8;
  t.colorSpace = THREE.NoColorSpace;
  t.needsUpdate = true;
  rubbleTex = t;
  return t;
}

/**
 * The bench's rock material: vertex colours from the strata, broken-rock relief from every side at two scales
 * (blocks a couple of metres across, taller than wide where joints run down the faces, and rubble a hand or two
 * across), each block its own shade, cracks darkened, dust on the flats, and the muck (attribute `aMuck`) coarser and
 * darker than the weathered bench.
 */
export function benchMaterial() {
  const mat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.95, metalness: 0 });
  const uniforms = {
    tNoise: { value: noiseTexture() },
    tRubble: { value: rubbleNormal() },
    uDust: { value: new THREE.Color("#ad9f8a") },
  };
  mat.customProgramCacheKey = () => "fm-blast-bench-v4";
  mat.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, uniforms);
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nattribute float aMuck;\nvarying vec3 vRW;\nvarying vec3 vRN;\nvarying float vMuck;")
      .replace(
        "#include <worldpos_vertex>",
        "#include <worldpos_vertex>\nvRW = (modelMatrix * vec4(transformed, 1.0)).xyz;\nvRN = normalize(mat3(modelMatrix) * objectNormal);\nvMuck = aMuck;",
      );
    shader.fragmentShader = shader.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
uniform sampler2D tNoise;
uniform sampler2D tRubble;
uniform vec3 uDust;
varying vec3 vRW;
varying vec3 vRN;
varying float vMuck;
vec3 rubN;
float rubTone;
float rubCrack;
vec3 benchN;
vec3 triW(vec3 n) { vec3 w = pow(abs(n), vec3(4.0)); return w / (w.x + w.y + w.z + 1e-4); }
vec4 triN(vec3 p, vec3 w) {
  return texture2D(tNoise, p.zy) * w.x + texture2D(tNoise, p.xz) * w.y + texture2D(tNoise, p.xy) * w.z;
}
// Triplanar blocks: a whiteout blend of the facet normals onto n (strength k), and the blended tone and crack mask.
void triR(vec3 p, vec3 n, vec3 w, float k) {
  vec4 ax = texture2D(tRubble, p.zy);
  vec4 ay = texture2D(tRubble, p.xz);
  vec4 az = texture2D(tRubble, p.xy);
  vec3 tx = vec3((ax.xy * 2.0 - 1.0) * k + n.zy, n.x);
  vec3 ty = vec3((ay.xy * 2.0 - 1.0) * k + n.xz, n.y);
  vec3 tz = vec3((az.xy * 2.0 - 1.0) * k + n.xy, n.z);
  rubN = normalize(tx.zyx * w.x + ty.xzy * w.y + tz.xyz * w.z);
  rubTone = ax.z * w.x + ay.z * w.y + az.z * w.z;
  rubCrack = ax.w * w.x + ay.w * w.y + az.w * w.z;
}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec3 rn = normalize(vRN);
vec3 tw = triW(rn);
float steep = 1.0 - smoothstep(0.5, 0.88, rn.y);
float camD = length(vRW - cameraPosition);
float near = 1.0 - smoothstep(260.0, 1100.0, camD);
vec3 q = vec3(vRW.x * 0.8 + vRW.z * 0.6, vRW.y, vRW.z * 0.8 - vRW.x * 0.6);
vec4 fine = mix(triN(vRW * 0.31, tw), triN(q * 0.197 + 0.43, tw), 0.5);
vec4 coarse = mix(triN(vRW * 0.021, tw), triN(q * 0.0131 + 0.17, tw), 0.5);
vec4 mid = mix(triN(vRW * 0.085 + 0.31, tw), triN(q * 0.0537 + 0.71, tw), 0.5);
float rock = max(steep, vMuck);
// Where fines and dust have washed over the face the blocks show less.
float show = near * mix(0.55, 1.0, smoothstep(0.3, 0.7, mid.g + 0.25 * vMuck));
// Big blocks: on the faces taller than wide (joints run down them); in the muck, round about.
vec3 pA = mix(vec3(vRW.x / 17.0, vRW.y / 24.0, vRW.z / 17.0), vRW / 11.0, vMuck);
triR(pA, rn, tw, near * mix(mix(0.15, 1.15, steep), 1.0, vMuck));
vec3 nA = rubN;
float toneA = rubTone;
float crackA = rubCrack;
vec3 pB = mix(vec3(q.x / 4.2, q.y / 5.4, q.z / 4.2), q / 3.1, vMuck) + 0.37;
triR(pB, nA, tw, near * mix(0.45, 1.0, rock));
benchN = rubN;
diffuseColor.rgb *= mix(1.0, 0.84 + 0.3 * toneA, rock * show * 0.8);
diffuseColor.rgb *= mix(1.0, 0.88 + 0.24 * rubTone, (0.3 + 0.7 * rock) * show);
// Open cracks only here and there; elsewhere the blocks are tight or filled with fines.
float open = smoothstep(0.42, 0.68, mid.g * 0.7 + fine.b * 0.5) * (0.4 + 0.6 * vMuck);
float crack = mix(1.0, crackA, rock * show * open) * mix(1.0, rubCrack, (0.15 + 0.6 * rock) * show * (0.3 + 0.7 * open));
diffuseColor.rgb *= 0.55 + 0.45 * crack;
diffuseColor.rgb *= mix(1.0, 0.82 + 0.3 * mid.b * coarse.a + 0.1 * fine.g, rock * 0.7);
diffuseColor.rgb *= 0.84 + 0.3 * mid.r * (0.6 + 0.4 * fine.g);
// Layering across the faces.
diffuseColor.rgb *= mix(1.0, 0.88 + 0.24 * texture2D(tNoise, vec2(vRW.y * 0.05, (vRW.x + vRW.z) * 0.0021)).r, steep);
// Dust settles on the flats, unevenly; fresh muck has none.
float dust = (1.0 - steep) * (1.0 - vMuck) * smoothstep(0.32, 0.72, coarse.g * 0.7 + fine.r * 0.4);
diffuseColor.rgb = mix(diffuseColor.rgb, uDust * (0.9 + 0.2 * fine.a), dust * 0.34);`,
      )
      .replace(
        "#include <normal_fragment_maps>",
        `#include <normal_fragment_maps>
normal = normalize((viewMatrix * vec4(benchN, 0.0)).xyz);`,
      )
      .replace(
        "#include <roughnessmap_fragment>",
        "#include <roughnessmap_fragment>\nroughnessFactor = clamp(roughnessFactor - 0.06 * (1.0 - smoothstep(0.5, 0.88, normalize(vRN).y)), 0.6, 1.0);",
      );
  };
  return mat;
}

export interface Ground {
  mesh: THREE.Mesh;
  geometry: THREE.BufferGeometry;
  xs: number[];
  zs: number[];
  /** Fine region: column and row ranges, inclusive. */
  fine: { i0: number; i1: number; j0: number; j1: number };
  /** Current surface height under a point (bilinear on the grid). */
  heightAt(x: number, z: number): number;
  /** The surface before and after the blast, for placing things on the bench or the muck. */
  preHeightAt(x: number, z: number): number;
  postHeightAt(x: number, z: number): number;
  /** Blast time (s) at which the rock at a point starts to move, Infinity outside the blast. */
  fireAt(x: number, z: number): number;
  /** Poses the blast at time t seconds after the first hole (Infinity for the finished muck pile, −1 for none). */
  blast(t: number): void;
  /** Blast time after which the ground no longer moves. */
  end: number;
}

export interface GroundOptions {
  /** Fine cell size over the blast, m. */
  cell: number;
  /** Where the firing point is, for a second fine patch. */
  focus: [number, number];
  /** Yields to the browser between chunks of work. */
  pause: () => Promise<void>;
}

/** How the rock moves at a point that starts at tFire: a dome lifts, then the rock slumps into the muck. */
export function morph(tau: number, bulge: number) {
  if (tau <= 0) return { rise: 0, go: 0 };
  if (bulge > 0) return { rise: bulge * smooth(tau / 0.16), go: smooth((tau - 0.14) / 1.05) };
  return { rise: 0, go: smooth(tau / 0.85) };
}

export async function makeGround(o: GroundOptions): Promise<Ground> {
  const [fx, fz] = o.focus;
  const coarse = o.cell > 0.6;
  const xs = axis(-70, 70, o.cell, -2400, 2400, [[fx - 50, fx + 50, 2.2]], coarse ? 0.045 : 0.03, coarse ? 0.00008 : 0.00005, 36);
  const zs = axis(-46, 42, o.cell, -2400, 2600, [[fz - 50, fz + 50, 2.2]], coarse ? 0.045 : 0.03, coarse ? 0.00008 : 0.00005, 36);
  const nx = xs.length;
  const nz = zs.length;
  const N = nx * nz;
  const pre = new Float32Array(N);
  const post = new Float32Array(N);
  const now = new Float32Array(N);
  const i0 = xs.indexOf(-70);
  const i1 = xs.indexOf(70);
  const j0 = zs.indexOf(-46);
  const j1 = zs.indexOf(42);

  // Heights, a chunk of rows at a time.
  for (let j = 0; j < nz; j++) {
    const z = zs[j]!;
    const fineRow = j >= j0 && j <= j1;
    for (let i = 0; i < nx; i++) {
      const x = xs[i]!;
      const k = j * nx + i;
      const h = preAt(x, z);
      pre[k] = h;
      now[k] = h;
      post[k] = fineRow && i >= i0 && i <= i1 ? postAt(x, z, h) : h;
    }
    if (j % 50 === 49) await o.pause();
  }

  const pos = new Float32Array(N * 3);
  const nrm = new Float32Array(N * 3);
  const uv = new Float32Array(N * 2);
  for (let j = 0; j < nz; j++) {
    for (let i = 0; i < nx; i++) {
      const k = j * nx + i;
      pos[k * 3] = xs[i]!;
      pos[k * 3 + 1] = pre[k]!;
      pos[k * 3 + 2] = zs[j]!;
      uv[k * 2] = xs[i]! * 0.12;
      uv[k * 2 + 1] = zs[j]! * 0.12;
    }
  }
  const normalsFor = (h: Float32Array, ia: number, ib: number, ja: number, jb: number) => {
    for (let j = ja; j <= jb; j++) {
      const jm = Math.max(0, j - 1);
      const jp = Math.min(nz - 1, j + 1);
      const dz = zs[jp]! - zs[jm]!;
      for (let i = ia; i <= ib; i++) {
        const im = Math.max(0, i - 1);
        const ip = Math.min(nx - 1, i + 1);
        const dx = xs[ip]! - xs[im]!;
        const gx = (h[j * nx + ip]! - h[j * nx + im]!) / dx;
        const gz = (h[jp * nx + i]! - h[jm * nx + i]!) / dz;
        const l = 1 / Math.hypot(gx, 1, gz);
        const k = (j * nx + i) * 3;
        nrm[k] = -gx * l;
        nrm[k + 1] = l;
        nrm[k + 2] = -gz * l;
      }
    }
  };
  normalsFor(pre, 0, nx - 1, 0, nz - 1);
  await o.pause();

  /* ---------- Hollows and crests: a cheap ambient occlusion from the heightfield ---------- */
  const rx3 = reach(xs, 3);
  const rz3 = reach(zs, 3);
  const rx9 = reach(xs, 9);
  const rz9 = reach(zs, 9);
  const occlusion = (h: Float32Array, i: number, j: number) => {
    const k = j * nx + i;
    const me = h[k]!;
    const a3 = (h[j * nx + rx3.lo[i]!]! + h[j * nx + rx3.hi[i]!]! + h[rz3.lo[j]! * nx + i]! + h[rz3.hi[j]! * nx + i]!) / 4 - me;
    const a9 = (h[j * nx + rx9.lo[i]!]! + h[j * nx + rx9.hi[i]!]! + h[rz9.lo[j]! * nx + i]! + h[rz9.hi[j]! * nx + i]!) / 4 - me;
    // Hollows (neighbours higher) darken; crests and lumps catch a little more light.
    return clamp(1 - 0.075 * a3 - 0.022 * a9, 0.5, 1.12);
  };

  /* ---------- Colours: the rock by depth, dust on the flats, the drill pad and the muck ---------- */
  const col = new Float32Array(N * 3);
  const colPost = new Float32Array((i1 - i0 + 1) * (j1 - j0 + 1) * 3);
  const colPre = new Float32Array(colPost.length);
  const c = new THREE.Color();
  const tmp = new THREE.Color();
  const DUST = new THREE.Color("#ab9d87");
  const WARM = new THREE.Color("#8a735c");
  const PAD = new THREE.Color("#9a9083");
  const ROAD = new THREE.Color("#a3978a");
  const MUCK = [new THREE.Color("#5e5852"), new THREE.Color("#6f675d"), new THREE.Color("#57514b"), new THREE.Color("#7a7064"), new THREE.Color("#665c50")];
  const cn = noise2(5);
  const rockColour = (x: number, z: number, y: number, ny: number, out: THREE.Color) => {
    const v = clamp(0.5 + 0.9 * cn.fbm(x / 45, z / 45, 3), 0, 0.999);
    strata(Math.max(0, RIM - y), v, out);
    // A little warmer than the hero's fresh rock: iron staining on the working benches.
    out.lerp(WARM, 0.22 * smooth((RIM - y - 40) / 30));
    const flat = smooth((ny - 0.75) / 0.2);
    out.lerp(DUST, flat * 0.16);
    const [u, s] = toBench(x, z);
    // The drill pad: cuttings and tyre marks over the pattern.
    if (s < 1 && s > BACK - 8 && Math.abs(u - SPAN.mid) < SPAN.half + 10 && y > -2)
      out.lerp(PAD, 0.3 * flat * (1 - smooth((Math.abs(u - SPAN.mid) - SPAN.half) / 10)));
    // The ramp and the floor's travel lanes.
    if (u > RAMP.u0 - 10 && s > RAMP.inner && s < RAMP.outer) out.lerp(ROAD, 0.45 * flat);
    if (flat > 0.5 && s > 8 && s < 90) out.multiplyScalar(1 - 0.06 * Math.max(0, Math.sin(s * 1.7 + cn.fbm(u / 40, 1, 2) * 6)));
    // Patches: damp ground, fresh spill, old dust.
    out.multiplyScalar(1 + 0.16 * flat * cn.fbm(x / 11 + 4, z / 11 - 2, 3));
    return out;
  };
  for (let j = 0; j < nz; j++) {
    for (let i = 0; i < nx; i++) {
      const k = j * nx + i;
      rockColour(xs[i]!, zs[j]!, pre[k]!, nrm[k * 3 + 1]!, c);
      c.multiplyScalar(occlusion(pre, i, j));
      col[k * 3] = c.r;
      col[k * 3 + 1] = c.g;
      col[k * 3 + 2] = c.b;
    }
    if (j % 90 === 89) await o.pause();
  }
  // The muck: freshly broken rock, darker and less dusty than the weathered bench.
  const fw = i1 - i0 + 1;
  const fh = j1 - j0 + 1;
  const muckPost = new Float32Array(fw * fh);
  for (let j = j0; j <= j1; j++) {
    for (let i = i0; i <= i1; i++) {
      const k = j * nx + i;
      const f = (j - j0) * fw + (i - i0);
      const f3 = f * 3;
      colPre[f3] = col[k * 3]!;
      colPre[f3 + 1] = col[k * 3 + 1]!;
      colPre[f3 + 2] = col[k * 3 + 2]!;
      const changed = Math.abs(post[k]! - pre[k]!) > 0.05;
      if (changed) {
        const x = xs[i]!;
        const z = zs[j]!;
        const v = clamp(0.5 + 0.9 * cn.fbm(x / 5, z / 5, 3), 0, 0.999);
        const g = clamp(v * (MUCK.length - 1), 0, MUCK.length - 1.001);
        c.copy(MUCK[Math.floor(g)]!).lerp(MUCK[Math.floor(g) + 1]!, g % 1);
        // Ore-bearing rock is a little warmer; thin muck at the toe shows the floor through it.
        const [, s] = toBench(x, z);
        const thin = s > 0 ? clamp((post[k]! - pre[k]!) / 2.5, 0, 1) : 1;
        tmp.setRGB(colPre[f3]!, colPre[f3 + 1]!, colPre[f3 + 2]!);
        c.lerp(tmp, 0.12 + 0.6 * (1 - thin));
        c.multiplyScalar(occlusion(post, i, j));
        muckPost[f] = 0.35 + 0.65 * thin;
      } else c.setRGB(colPre[f3]!, colPre[f3 + 1]!, colPre[f3 + 2]!);
      colPost[f3] = c.r;
      colPost[f3 + 1] = c.g;
      colPost[f3 + 2] = c.b;
    }
  }
  await o.pause();

  const index: number[] = [];
  for (let j = 0; j < nz - 1; j++) {
    for (let i = 0; i < nx - 1; i++) {
      const a = j * nx + i;
      // Split each quad along the diagonal that follows the surface better.
      const d1 = Math.abs(pre[a]! - pre[a + nx + 1]!);
      const d2 = Math.abs(pre[a + 1]! - pre[a + nx]!);
      if (d1 <= d2) index.push(a, a + nx, a + nx + 1, a, a + nx + 1, a + 1);
      else index.push(a, a + nx, a + 1, a + 1, a + nx, a + nx + 1);
    }
  }
  const muck = new Float32Array(N);
  const geometry = new THREE.BufferGeometry();
  const P = new THREE.BufferAttribute(pos, 3);
  const Nn = new THREE.BufferAttribute(nrm, 3);
  const C = new THREE.BufferAttribute(col, 3);
  const M = new THREE.BufferAttribute(muck, 1);
  for (const a of [P, Nn, C, M]) a.setUsage(THREE.DynamicDrawUsage);
  geometry.setAttribute("position", P);
  geometry.setAttribute("normal", Nn);
  geometry.setAttribute("color", C);
  geometry.setAttribute("aMuck", M);
  geometry.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  geometry.setIndex(N > 65535 ? new THREE.Uint32BufferAttribute(index, 1) : new THREE.Uint16BufferAttribute(index, 1));
  geometry.computeBoundingSphere();
  const mesh = new THREE.Mesh(geometry, benchMaterial());
  mesh.receiveShadow = true;
  mesh.castShadow = true;
  mesh.name = "blast-ground";

  /* ---------- Firing times and heave amplitude over the blast ---------- */
  const tFire = new Float32Array(fw * fh).fill(Infinity);
  const bulge = new Float32Array(fw * fh);
  const front = holes.filter((h) => h.row === 0);
  const fireFor = (u: number, s: number) => {
    if (s <= 0.5) {
      let best = Infinity;
      let t = 0;
      for (const h of holes) {
        const d = (u - h.u) ** 2 + ((s - h.s) * 1.15) ** 2;
        if (d < best) {
          best = d;
          t = h.ms / 1000;
        }
      }
      // Domes over each hole, highest near the free face.
      const near = Math.sqrt(best) / (PATTERN.spacing * 0.75);
      return { t: t + 0.004 * Math.sqrt(best), bulge: (1.1 + 1.6 * clamp((s + 20) / 20, 0, 1)) * (1 - 0.55 * clamp(near, 0, 1) ** 2) };
    }
    let best = Infinity;
    let t = 0;
    for (const h of front) {
      const d = Math.abs(u - h.u);
      if (d < best) {
        best = d;
        t = h.ms / 1000;
      }
    }
    // The thrown rock arrives later the further out it lands.
    return { t: t + 0.14 + s / 24, bulge: 0 };
  };
  for (let j = j0; j <= j1; j++) {
    for (let i = i0; i <= i1; i++) {
      const k = j * nx + i;
      const f = (j - j0) * fw + (i - i0);
      if (Math.abs(post[k]! - pre[k]!) < 0.02) continue;
      const [u, s] = toBench(xs[i]!, zs[j]!);
      const r = fireFor(u, s);
      tFire[f] = r.t;
      bulge[f] = r.bulge;
    }
  }

  let end = 0;
  for (const t of tFire) if (Number.isFinite(t)) end = Math.max(end, t);
  end += 1.3;

  /* ---------- Queries ---------- */
  const find = (arr: number[], v: number) => {
    let lo = 0;
    let hi = arr.length - 1;
    if (v <= arr[0]!) return 0;
    if (v >= arr[hi]!) return hi - 1;
    while (hi - lo > 1) {
      const m = (lo + hi) >> 1;
      if (arr[m]! <= v) lo = m;
      else hi = m;
    }
    return lo;
  };
  const sample = (h: Float32Array, x: number, z: number) => {
    const i = find(xs, x);
    const j = find(zs, z);
    const tx = clamp((x - xs[i]!) / (xs[i + 1]! - xs[i]!), 0, 1);
    const tz = clamp((z - zs[j]!) / (zs[j + 1]! - zs[j]!), 0, 1);
    const a = h[j * nx + i]!;
    const b = h[j * nx + i + 1]!;
    const cc = h[(j + 1) * nx + i]!;
    const d = h[(j + 1) * nx + i + 1]!;
    return mix(mix(a, b, tx), mix(cc, d, tx), tz);
  };

  /* ---------- The blast ---------- */
  let shown = -1;
  const blast = (t: number) => {
    if (t === shown) return;
    shown = t;
    for (let j = j0; j <= j1; j++) {
      for (let i = i0; i <= i1; i++) {
        const k = j * nx + i;
        const f = (j - j0) * fw + (i - i0);
        const tf = tFire[f]!;
        let y = pre[k]!;
        let go = 0;
        if (t >= tf) {
          const m = morph(t - tf, bulge[f]!);
          go = m.go;
          y = mix(pre[k]! + m.rise, post[k]!, go);
        }
        now[k] = y;
        pos[k * 3 + 1] = y;
        const fc = f * 3;
        col[k * 3] = mix(colPre[fc]!, colPost[fc]!, go);
        col[k * 3 + 1] = mix(colPre[fc + 1]!, colPost[fc + 1]!, go);
        col[k * 3 + 2] = mix(colPre[fc + 2]!, colPost[fc + 2]!, go);
        muck[k] = muckPost[f]! * smooth(go * 1.6);
      }
    }
    normalsFor(now, i0, i1, j0, j1);
    for (const attr of [P, Nn, C, M]) {
      attr.clearUpdateRanges();
      for (let j = j0; j <= j1; j++) attr.addUpdateRange((j * nx + i0) * attr.itemSize, fw * attr.itemSize);
      attr.needsUpdate = true;
    }
  };

  return {
    mesh,
    geometry,
    xs,
    zs,
    fine: { i0, i1, j0, j1 },
    heightAt: (x, z) => sample(now, x, z),
    preHeightAt: (x, z) => sample(pre, x, z),
    postHeightAt: (x, z) => sample(post, x, z),
    fireAt: (x, z) => {
      const [u, s] = toBench(x, z);
      return inBlast(u, s) ? fireFor(u, s).t : Infinity;
    },
    blast,
    end,
  };
}
