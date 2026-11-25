/**
 * The ground of the tailings scene: the desert valley out to the horizon with its roads, the embankment raised on
 * the centreline with its rockfill faces and berms, and the beach of grey tailings that falls to the pond. Heights
 * come from `tailings-model.ts`; the meshes are built here across several frames so the page never stalls.
 */
import * as THREE from "three";
import { gravelNormal, makeRocks, noiseTexture } from "./fleet-kit";
import { clamp, mix, smooth } from "./lib";
import { noise2 } from "./stage";
import { blend, rockSurfaceMaterial, strata } from "./terrain";
import {
  BEACH_TOP,
  crestZ,
  DAM,
  damTop,
  ground as groundAt,
  natural,
  SEEP,
  seepDepth,
  settle,
  tailings,
  TOE_S,
  upstream,
} from "./tailings-model";

export type Pause = () => Promise<void>;

/** Uniforms the ground, the beach and the water share: rain wetness, the pond's level, time and the storm's darkness. */
export interface Weather {
  wet: { value: number };
  level: { value: number };
  time: { value: number };
  storm: { value: number };
}
export const weather = (level: number): Weather => ({ wet: { value: 0 }, level: { value: level }, time: { value: 0 }, storm: { value: 0 } });

const C = (hex: string) => new THREE.Color(hex);
const ROAD = C("#b7ab93");
const ROCKFILL = [C("#6e675e"), C("#625c54"), C("#7a7268"), C("#58534d"), C("#6b6459")];
const FRESHFILL = [C("#77756f"), C("#6b6964"), C("#827f78")];
const CLAY = C("#8e7a62");
const TAILDUST = C("#a9a59c");

/** The desert rock material, darkened and given a sheen where rain wets it. */
export function wetRock(w: Weather, key: string) {
  const mat = rockSurfaceMaterial();
  const base = mat.onBeforeCompile;
  mat.onBeforeCompile = (sh, r) => {
    base.call(mat, sh, r);
    sh.uniforms.uWet = w.wet;
    sh.fragmentShader = sh.fragmentShader.replace("#include <common>", "#include <common>\nuniform float uWet;").replace(
      "#include <normal_fragment_begin>",
      `{
  float wFlat = smoothstep(0.35, 0.95, normalize(vRN).y);
  float wk = uWet * (0.55 + 0.45 * wFlat);
  diffuseColor.rgb *= 1.0 - 0.42 * wk;
  roughnessFactor = mix(roughnessFactor, 0.42, wk * wFlat * 0.85);
}
#include <normal_fragment_begin>`,
    );
  };
  mat.customProgramCacheKey = () => `fm-rock-v4-wet-${key}`;
  return mat;
}

/**
 * The desert floor and the hills: colour from the vertex (laid down at many scales, so nothing repeats), fine grit
 * and pebbles near the camera that fade out with distance before any texture could tile, banded rock on the
 * steep ground, and rain that darkens it all.
 */
export function desertMaterial(w: Weather) {
  const mat = new THREE.MeshStandardMaterial({
    vertexColors: true,
    roughness: 0.97,
    metalness: 0,
    normalMap: gravelNormal(),
    normalScale: new THREE.Vector2(0.6, 0.6),
  });
  const uniforms = { tNoise: { value: noiseTexture() }, uWet: w.wet };
  mat.customProgramCacheKey = () => "fm-desert-v1";
  mat.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, uniforms);
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vDW;\nvarying vec3 vDN;")
      .replace(
        "#include <worldpos_vertex>",
        "#include <worldpos_vertex>\nvDW = (modelMatrix * vec4(transformed, 1.0)).xyz;\nvDN = normalize(mat3(modelMatrix) * objectNormal);",
      );
    sh.fragmentShader = sh.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
uniform sampler2D tNoise;
uniform float uWet;
varying vec3 vDW;
varying vec3 vDN;
float dNear;`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
{
  float dist = length(cameraPosition - vDW);
  dNear = 1.0 - smoothstep(150.0, 1100.0, dist);
  vec2 p = vDW.xz;
  vec2 r = vec2(p.x * 0.8 - p.y * 0.6, p.x * 0.6 + p.y * 0.8);
  vec4 f1 = texture2D(tNoise, p * 0.093 + 0.21);
  vec4 f2 = texture2D(tNoise, r * 0.0277 + 0.73);
  diffuseColor.rgb *= mix(1.0, 0.86 + 0.13 * f1.r + 0.11 * f2.g, dNear);
  // Pebbles and grit, only where a pixel is small enough to hold them.
  float px = length(fwidth(p));
  vec4 f3 = texture2D(tNoise, r * 0.57 + 0.37);
  float speck = smoothstep(0.74, 0.86, f3.b) * (1.0 - smoothstep(0.08, 0.5, px));
  diffuseColor.rgb *= 1.0 - 0.3 * speck + 0.12 * smoothstep(0.8, 0.9, f3.r) * (1.0 - smoothstep(0.08, 0.5, px));
  // Rock on the steep ground, in beds that follow the height.
  vec3 n = normalize(vDN);
  float steep = 1.0 - smoothstep(0.62, 0.9, n.y);
  float bed = texture2D(tNoise, vec2(vDW.y * 0.071 + p.x * 0.0009, 0.37 + p.y * 0.0009)).r;
  diffuseColor.rgb *= mix(1.0, 0.72 + 0.42 * bed, steep * 0.85);
  float wk = uWet * (0.55 + 0.45 * smoothstep(0.35, 0.95, n.y));
  diffuseColor.rgb *= 1.0 - 0.42 * wk;
}`,
      )
      .replace("#include <normal_fragment_maps>", "vec3 nGeo = normal;\n#include <normal_fragment_maps>\nnormal = normalize(mix(nGeo, normal, dNear));")
      .replace(
        "#include <roughnessmap_fragment>",
        "#include <roughnessmap_fragment>\nroughnessFactor = mix(roughnessFactor, 0.5, uWet * smoothstep(0.6, 0.95, normalize(vDN).y) * 0.8);",
      );
  };
  return mat;
}

/**
 * Rockfill and graded crest: the embankment's own material. On the faces, angular rocks of about a metre with dark
 * voids between them and a facet each that catches the sun differently; on the crest and the berms, graded gravel.
 * The `face` attribute says which (1 face, 0 graded). Rain darkens it and gives the flats a sheen.
 */
export function rockfillMaterial(w: Weather) {
  const mat = new THREE.MeshStandardMaterial({
    vertexColors: true,
    roughness: 0.95,
    metalness: 0,
    normalMap: gravelNormal(),
    normalScale: new THREE.Vector2(0.9, 0.9),
  });
  const uniforms = { tNoise: { value: noiseTexture() }, uWet: w.wet };
  mat.customProgramCacheKey = () => "fm-rockfill-v4";
  mat.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, uniforms);
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nattribute float face;\nvarying float vFace;\nvarying vec3 vRW;\nvarying vec3 vRN;")
      .replace(
        "#include <worldpos_vertex>",
        "#include <worldpos_vertex>\nvFace = face;\nvRW = (modelMatrix * vec4(transformed, 1.0)).xyz;\nvRN = normalize(mat3(modelMatrix) * objectNormal);",
      );
    sh.fragmentShader = sh.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
uniform sampler2D tNoise;
uniform float uWet;
varying float vFace;
varying vec3 vRW;
varying vec3 vRN;
vec2 rfHash(vec2 p) {
  p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
  return fract(sin(p) * 43758.5453);
}
// Nearest and second-nearest distances of a jittered grid, and the nearest cell's hash: rocks and their voids.
vec2 rfNear;
vec4 rfVoro(vec2 x) {
  vec2 n = floor(x);
  vec2 f = fract(x);
  float d1 = 8.0;
  float d2 = 8.0;
  vec2 id = vec2(0.0);
  vec2 near = vec2(0.0);
  for (int j = -1; j <= 1; j++)
    for (int i = -1; i <= 1; i++) {
      vec2 g = vec2(float(i), float(j));
      vec2 o = rfHash(n + g);
      vec2 r = g + 0.5 + 0.4 * sin(6.2831 * o) - f;
      float d = dot(r, r);
      if (d < d1) { d2 = d1; d1 = d; id = n + g; near = r; }
      else if (d < d2) d2 = d;
    }
  rfNear = near;
  return vec4(sqrt(d1), sqrt(d2) - sqrt(d1), rfHash(id + 7.3));
}
vec3 rfTilt;
float rfFlat;
float rfFace;`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
{
  vec3 rn = normalize(vRN);
  // Coordinates on the face: along the crest, and down the slope.
  vec2 q = vec2(vRW.x, vRW.z * 0.93 - vRW.y * 0.37);
  float px = length(fwidth(q));
  float fc = smoothstep(0.3, 0.7, vFace);
  vec4 big = rfVoro(q / 1.15);
  vec2 bigNear = rfNear;
  vec4 sml = rfVoro(q / 0.42 + 17.0);
  float seeBig = 1.0 - smoothstep(0.35, 1.1, px);
  float seeSml = 1.0 - smoothstep(0.12, 0.4, px);
  // Each rock its own tone; the voids between them dark.
  float tone = 0.78 + 0.42 * big.z * seeBig + (0.21 * (1.0 - seeBig));
  float tone2 = 0.9 + 0.2 * sml.w * seeSml + 0.1 * (1.0 - seeSml);
  float gap = (1.0 - smoothstep(0.03, 0.09, big.y)) * seeBig;
  float gap2 = (1.0 - smoothstep(0.03, 0.11, sml.y)) * seeSml * 0.6;
  vec4 mottle = texture2D(tNoise, q * 0.013 + 0.4);
  // Rounded shoulders: each rock darkens towards its edges, as the voids shade them.
  float ao = mix(1.0, 0.6 + 0.4 * smoothstep(0.0, 0.34, big.y), seeBig);
  float rock = tone * tone2 * ao * (1.0 - 0.6 * gap) * (1.0 - 0.45 * gap2) * (0.9 + 0.2 * mottle.r);
  // Graded gravel on the crest and berms: fine and even, with darker tyre lines from the vertex colours.
  vec4 fine = texture2D(tNoise, vRW.xz * 0.37);
  float gravel = 0.93 + 0.12 * fine.g + 0.05 * mottle.b;
  diffuseColor.rgb *= mix(gravel, rock, fc);
  // Each rock a rounded block: its surface tilts away from its centre, so the sun lights one shoulder and the
  // other falls into shade; plus a facet of its own.
  vec2 dome = -bigNear * 1.5;
  rfTilt = (vec3(dome.x, 0.0, dome.y * 0.93) + vec3(big.z - 0.5, 0.0, big.w - 0.5) * 0.6) * seeBig * fc;
  rfFlat = smoothstep(0.6, 0.95, rn.y);
  rfFace = fc;
  float wk = uWet * (0.6 + 0.4 * rfFlat);
  diffuseColor.rgb *= 1.0 - 0.4 * wk;
}`,
      )
      .replace(
        "#include <normal_fragment_maps>",
        `#include <normal_fragment_maps>
// The graded gravel on the crest and berms is fine and even: only a trace of the coarse normal map.
normal = normalize(mix(nonPerturbedNormal, normal, 0.05 + 0.95 * rfFace));
normal = normalize(normal + (viewMatrix * vec4(rfTilt, 0.0)).xyz * 0.9);`,
      )
      .replace(
        "#include <roughnessmap_fragment>",
        "#include <roughnessmap_fragment>\nroughnessFactor = mix(roughnessFactor, 0.45, uWet * rfFlat * 0.8);",
      );
  };
  return mat;
}

/* ---------- Roads ---------- */
/** A road along points, with a running height smoothed from the ground and pinned where asked. */
export interface Road {
  pts: THREE.Vector3[];
  width: number;
}
function resample(pts: [number, number][], step = 2) {
  const out: [number, number][] = [];
  const curve = new THREE.CatmullRomCurve3(
    pts.map(([x, z]) => new THREE.Vector3(x, 0, z)),
    false,
    "centripetal",
  );
  const n = Math.max(2, Math.ceil(curve.getLength() / step));
  for (const p of curve.getSpacedPoints(n)) out.push([p.x, p.z]);
  return out;
}
function road(pts: [number, number][], width: number, pin: { start?: number; end?: number } = {}, ground = natural): Road {
  const line = resample(pts);
  const raw = line.map(([x, z]) => ground(x, z));
  const k = 14;
  const ys = raw.map((_, i) => {
    let s = 0;
    let n = 0;
    for (let j = Math.max(0, i - k); j <= Math.min(raw.length - 1, i + k); j++) {
      s += raw[j]!;
      n++;
    }
    return s / n;
  });
  // Ease into the pinned heights over the first and last stretch.
  const ease = 40;
  for (let i = 0; i < ys.length; i++) {
    if (pin.start !== undefined) ys[i] = mix(pin.start, ys[i]!, smooth(i / ease));
    if (pin.end !== undefined) ys[i] = mix(pin.end, ys[i]!, smooth((ys.length - 1 - i) / ease));
  }
  return { pts: line.map(([x, z], i) => new THREE.Vector3(x, ys[i]!, z)), width };
}
/** The site's roads: up the right abutment to the crest, along the toe, and round the right side to the causeway. */
export function siteRoads(): Road[] {
  const crestEnd = (x: number) => [x, crestZ(x)] as [number, number];
  return [
    road([crestEnd(338), [366, 46], [372, 96], [346, 152], [282, 196], [196, 208], [100, 198], [20, 196], [-90, 200], [-240, 214], [-460, 250], [-760, 310]], 11, {
      start: 36.05,
    }),
    road([[-262, crestZ(-262) + TOE_S + 12], [-150, crestZ(-150) + TOE_S + 9], [-40, TOE_S + 9], [60, crestZ(60) + TOE_S + 10], [150, crestZ(150) + TOE_S + 14], [228, 196]], 8),
    road([crestEnd(344), [338, -60], [318, -170], [282, -300], [232, -420], [170, -520], [96, -598], [72, -612]], 9, { start: 36.05, end: 35.6 }),
  ];
}

/* ---------- The valley out to the horizon ---------- */
export interface Ground {
  mesh: THREE.Mesh;
  heightAt(x: number, z: number): number;
  roads: Road[];
  material: THREE.MeshStandardMaterial;
}

/** Grid lines `cell` apart near the centre, widening with distance out to the horizon. */
function axis(centre: number, half: number, cell: number, reach: number) {
  const right: number[] = [];
  let u = 0;
  while (u < reach) {
    u += u < half ? cell : Math.min(420, cell + (u - half) * 0.065);
    right.push(u);
  }
  return [...right.map((v) => centre - v).reverse(), centre, ...right.map((v) => centre + v)];
}
const find = (a: number[], v: number) => {
  let lo = 0;
  let hi = a.length - 1;
  if (v <= a[0]!) return 0;
  if (v >= a[hi]!) return hi - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (a[mid]! <= v) lo = mid;
    else hi = mid;
  }
  return lo;
};

export async function makeGround(o: { small: boolean; pause: Pause; weather: Weather }): Promise<Ground> {
  const far = noise2(12);
  const tone = noise2(5);
  const xs = axis(0, 470, o.small ? 6 : 3.2, 9000);
  const zs = axis(-210, 560, o.small ? 6 : 3.2, 9000);
  const nx = xs.length;
  const nz = zs.length;
  const count = nx * nz;
  const pos = new Float32Array(count * 3);
  const roadD = new Float32Array(count).fill(1e9);
  const roadY = new Float32Array(count);
  const roads = siteRoads();
  const mountains = (x: number, z: number) => {
    const r = Math.hypot(x, z + 200);
    return 280 * Math.max(0, far.fbm(x / 2600 + 4, z / 2600 - 2, 5) + 0.1) * smooth((r - 2400) / 2600) + 30 * far.fbm(x / 700, z / 700, 3) * smooth((r - 1200) / 1500);
  };
  for (let j = 0; j < nz; j++) {
    const z = zs[j]!;
    for (let k = 0; k < nx; k++) {
      const x = xs[k]!;
      const i = j * nx + k;
      pos[i * 3] = x;
      const cell = Math.max(xs[Math.min(k + 1, nx - 1)]! - xs[Math.max(k - 1, 0)]!, zs[Math.min(j + 1, nz - 1)]! - zs[Math.max(j - 1, 0)]!) / 2;
      pos[i * 3 + 1] = groundAt(x, z, cell) + mountains(x, z);
      pos[i * 3 + 2] = z;
    }
    if (j % 60 === 59) await o.pause();
  }
  // Roads: cut and fill to the road's running height, with soft shoulders.
  for (const r of roads) {
    const reach = r.width / 2 + 9;
    for (const p of r.pts) {
      const k0 = find(xs, p.x - reach);
      const k1 = find(xs, p.x + reach) + 1;
      const j0 = find(zs, p.z - reach);
      const j1 = find(zs, p.z + reach) + 1;
      for (let j = j0; j <= j1; j++) {
        for (let k = k0; k <= k1; k++) {
          const i = j * nx + k;
          const d = Math.hypot(xs[k]! - p.x, zs[j]! - p.z) - r.width / 2;
          if (d < roadD[i]!) {
            roadD[i] = d;
            roadY[i] = p.y;
          }
        }
      }
    }
  }
  for (let i = 0; i < count; i++) {
    const d = roadD[i]!;
    if (d < 9) pos[i * 3 + 1] = mix(pos[i * 3 + 1]!, roadY[i]! - 0.05, 1 - smooth(d / 9));
  }
  // Under the seepage pond's liner the ground sits a little lower, so the coarse grid never pokes through.
  for (let j = 0; j < nz; j++) {
    for (let k = 0; k < nx; k++) {
      const x = xs[k]!;
      const z = zs[j]!;
      if (Math.abs(x - SEEP.x) < SEEP.hx + 1 && Math.abs(z - SEEP.z) < SEEP.hz + 1) pos[(j * nx + k) * 3 + 1]! -= 0.7 * smooth(seepDepth(x, z) / 0.4 + 0.3);
    }
  }
  await o.pause();

  const index = new Uint32Array((nx - 1) * (nz - 1) * 6);
  let t = 0;
  for (let j = 0; j < nz - 1; j++) {
    for (let k = 0; k < nx - 1; k++) {
      const a = j * nx + k;
      index[t++] = a;
      index[t++] = a + nx;
      index[t++] = a + 1;
      index[t++] = a + 1;
      index[t++] = a + nx;
      index[t++] = a + nx + 1;
    }
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setIndex(new THREE.BufferAttribute(index, 1));
  geo.computeVertexNormals();
  await o.pause();

  const nrm = geo.getAttribute("normal") as THREE.BufferAttribute;
  const col = new Float32Array(count * 3);
  const uv = new Float32Array(count * 2);
  const c = new THREE.Color();
  const tmp = new THREE.Color();
  for (let j = 0; j < nz; j++) {
    for (let k = 0; k < nx; k++) {
      const i = j * nx + k;
      const x = xs[k]!;
      const z = zs[j]!;
      const y = pos[i * 3 + 1]!;
      uv[i * 2] = x * 0.12;
      uv[i * 2 + 1] = z * 0.12;
      const cell = Math.max(xs[Math.min(k + 1, nx - 1)]! - xs[Math.max(k - 1, 0)]!, zs[Math.min(j + 1, nz - 1)]! - zs[Math.max(j - 1, 0)]!) / 2;
      const coarse = smooth((cell - 12) / 30);
      const v = clamp(0.5 + 0.9 * mix(tone.fbm(x / 60, z / 60, 3), tone.fbm(x / 500, z / 500, 2), coarse), 0, 0.999);
      const steep = 1 - smooth((nrm.getY(i) - 0.8) / 0.17);
      strata(0, v, c);
      // Desert pavement: darker, redder gravel over much of the plain, sand gathered in the low ground.
      const pave = smooth((tone.fbm(x / 340 + 9, z / 340 - 4, 3) + 0.05) / 0.3);
      c.lerp(tmp.set("#8c7058"), pave * 0.45 * (1 - steep));
      c.lerp(tmp.set("#d2bf9c"), smooth((tone.fbm(x / 120 - 3, z / 120 + 6, 2) - 0.25) / 0.25) * 0.3 * (1 - pave));
      // Rock shows through on the steeper hillsides; the high ground is darker and redder.
      c.lerp(strata(22, clamp(v * 1.2, 0, 0.999), tmp), steep * 0.75);
      if (y > 60) c.lerp(tmp.set("#7d6553"), smooth((y - 60) / 160) * 0.7);
      c.lerp(tmp.set("#6e5a4b"), steep * smooth((y - 50) / 60) * 0.5);
      // Fine tailings dust blows out over the ground around the impoundment.
      const d = upstream(x, z);
      if (d > -20 && d < 900) {
        const near = tailings(x, z) - natural(x, z);
        c.lerp(TAILDUST, smooth((near + 6) / 6) * 0.35);
      }
      // Where the toe and the drains were dug, the ground is darker and compacted.
      const s = z - crestZ(x);
      if (s > 100 && s < 135 && Math.abs(x) < 300) c.multiplyScalar(1 - 0.08 * smooth(1 - Math.abs(s - 113) / 6));
      const rd = roadD[i]!;
      if (rd < 5) {
        c.lerp(ROAD, (1 - smooth((rd + 2) / 6)) * 0.85);
        c.multiplyScalar(1 - 0.05 * Math.max(0, Math.sin((rd + 6) * 2.3)) * (1 - smooth((rd + 2) / 6)));
      }
      col[i * 3] = c.r;
      col[i * 3 + 1] = c.g;
      col[i * 3 + 2] = c.b;
    }
    if (j % 90 === 89) await o.pause();
  }
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
  geo.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  const material = desertMaterial(o.weather);
  const mesh = new THREE.Mesh(geo, material);
  mesh.receiveShadow = true;
  mesh.castShadow = true;
  mesh.name = "ground";

  const heightAt = (x: number, z: number) => {
    const k = find(xs, x);
    const j = find(zs, z);
    const tx = clamp((x - xs[k]!) / (xs[k + 1]! - xs[k]!), 0, 1);
    const tz = clamp((z - zs[j]!) / (zs[j + 1]! - zs[j]!), 0, 1);
    const h = (a: number, b: number) => pos[(b * nx + a) * 3 + 1]!;
    return mix(mix(h(k, j), h(k + 1, j), tx), mix(h(k, j + 1), h(k + 1, j + 1), tx), tz);
  };
  return { mesh, heightAt, roads, material };
}

/* ---------- The embankment ---------- */
/** Profile stations across the embankment, with every break in slope kept sharp. */
function stations(step: number) {
  const h = DAM.crestHalf;
  const run = DAM.face * DAM.down;
  const breaks = [-h, h, h + run, h + run + DAM.berm, h + 2 * run + DAM.berm, h + 2 * run + 2 * DAM.berm];
  const out = new Set<number>();
  for (let s = -36; s <= 150; s += step) out.add(Math.round(s * 100) / 100);
  for (const b of breaks) {
    out.add(b - 0.08);
    out.add(b + 0.08);
  }
  return [...out].sort((a, b) => a - b);
}

export interface Embankment {
  mesh: THREE.Mesh;
  rocks: THREE.Group;
  /** Surface height at a point on the embankment. */
  top(x: number, z: number): number;
}

export async function makeEmbankment(o: { small: boolean; pause: Pause; weather: Weather; groundAt: (x: number, z: number) => number }): Promise<Embankment> {
  const nz = noise2(91);
  const ss = stations(o.small ? 2 : 1.1);
  const xs: number[] = [];
  for (let x = -392; x <= 392; x += o.small ? 4 : 2.2) xs.push(x);
  const nx = xs.length;
  const ns = ss.length;
  const pos = new Float32Array(nx * ns * 3);
  const uv = new Float32Array(nx * ns * 2);
  const col = new Float32Array(nx * ns * 3);
  const faceA = new Float32Array(nx * ns);
  const c = new THREE.Color();
  const tmp = new THREE.Color();
  const h = DAM.crestHalf;
  // How much of a point is face (rough rockfill) rather than crest or berm (graded).
  const faceness = (s: number) => {
    if (s < -h) return 0.5;
    if (s <= h + 0.1) return 0;
    const run = DAM.face * DAM.down;
    const u = s - h;
    const k = Math.floor(u / (run + DAM.berm));
    const f = u - k * (run + DAM.berm);
    return k >= 2 || f < run ? 1 : 0;
  };
  // Along-profile distance, for the gravel texture's v.
  const along = new Float32Array(ns);
  for (let i = 1; i < ns; i++) along[i] = along[i - 1]! + Math.hypot(ss[i]! - ss[i - 1]!, damTop(ss[i]!) - damTop(ss[i - 1]!));
  for (let a = 0; a < nx; a++) {
    const x = xs[a]!;
    for (let b = 0; b < ns; b++) {
      const s = ss[b]!;
      const i = a * ns + b;
      const z = crestZ(x) + s;
      const f = faceness(s);
      faceA[i] = f;
      // Rockfill sits lumpy on the faces; the crest and berms are graded.
      const lump = f * (0.32 * nz.fbm(x / 7, s / 5, 3) + 0.18 * nz.fbm(x / 2.1, s / 1.7, 2));
      const edge = smooth(Math.min(...[-h, h, 37, 43, 73, 79].map((e) => Math.abs(s - e))) / 1.4);
      const y = damTop(s) - settle(x) + lump * edge;
      pos[i * 3] = x;
      pos[i * 3 + 1] = y;
      pos[i * 3 + 2] = z;
      uv[i * 2] = x * 0.085;
      uv[i * 2 + 1] = along[b]! * 0.085;
      const v = clamp(0.5 + 0.9 * nz.fbm(x / 30 + 3, s / 14, 3), 0, 0.999);
      if (s < -h) c.copy(CLAY).lerp(tmp.set("#9d917f"), v * 0.5);
      else if (f < 0.5) {
        // Crest road and berms: graded gravel, wheel tracks along the crest.
        c.copy(ROAD).lerp(blend(ROCKFILL, v, tmp), s > h ? 0.35 : 0.12);
        if (Math.abs(s) < h) c.multiplyScalar(1 - 0.06 * Math.max(0, Math.sin(s * 2.1 + 0.4)) * smooth(1 - Math.abs(s) / 5));
      } else {
        blend(ROCKFILL, v, c);
        // The newest raise is fresher, greyer rock; the older faces carry more dust.
        const yy = damTop(s);
        if (yy > 28) c.lerp(blend(FRESHFILL, v, tmp), 0.45);
        else c.lerp(tmp.set("#a29780"), 0.12 + 0.12 * smooth((20 - yy) / 12));
      }
      col[i * 3] = c.r;
      col[i * 3 + 1] = c.g;
      col[i * 3 + 2] = c.b;
    }
    if (a % 80 === 79) await o.pause();
  }
  const index = new Uint32Array((nx - 1) * (ns - 1) * 6);
  let t = 0;
  for (let a = 0; a < nx - 1; a++) {
    for (let b = 0; b < ns - 1; b++) {
      const p = a * ns + b;
      index[t++] = p;
      index[t++] = p + 1;
      index[t++] = p + ns;
      index[t++] = p + ns;
      index[t++] = p + 1;
      index[t++] = p + ns + 1;
    }
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
  geo.setIndex(new THREE.BufferAttribute(index, 1));
  geo.computeVertexNormals();
  geo.setAttribute("face", new THREE.BufferAttribute(faceA, 1));
  const mat = rockfillMaterial(o.weather);
  const mesh = new THREE.Mesh(geo, mat);
  mesh.castShadow = true;
  mesh.receiveShadow = true;
  mesh.name = "embankment";
  await o.pause();

  // Coarse rock on the faces, half buried, wherever the face stands above the natural ground.
  const top = (x: number, z: number) => damTop(z - crestZ(x)) - settle(x);
  const rocks = makeRocks({
    count: o.small ? 1400 : 4200,
    seed: 17,
    shapes: 5,
    detail: 1,
    palette: ["#857e72", "#77716a", "#958c7e", "#6a655d", "#8c8478", "#9b9283"],
    place(_, r) {
      for (let tries = 0; tries < 12; tries++) {
        const face = Math.floor(r() * 3);
        const s = h + face * (DAM.face * DAM.down + DAM.berm) + 0.8 + r() * (DAM.face * DAM.down - 1.6);
        const x = -330 + r() * 660;
        const z = crestZ(x) + s;
        const y = top(x, z);
        if (y < o.groundAt(x, z) + 0.6) continue;
        const size = 0.45 + r() ** 2.2 * 1.5;
        return { x, y: y - size * 0.22, z, size, flat: 0.72 };
      }
      return { x: 0, y: -50, z: 0, size: 0.1 };
    },
  });
  for (const m of rocks.meshes) m.castShadow = !o.small;
  return { mesh, rocks: rocks.group, top };
}

/* ---------- The beach ---------- */
export interface Beach {
  mesh: THREE.Mesh;
  material: THREE.MeshStandardMaterial;
}

/**
 * Wet grey silt: paler and cracked where it has dried, dark and glossy where the slurry runs and near the water,
 * white salt where it has dried longest, and the whole of it darkening as the rain soaks it.
 */
export function beachMaterial(w: Weather, spigots: number[]) {
  const mat = new THREE.MeshStandardMaterial({
    color: "#ffffff",
    roughness: 0.92,
    metalness: 0,
    normalMap: gravelNormal(),
    normalScale: new THREE.Vector2(0.12, 0.12),
  });
  const sp = new Float32Array(8).fill(1e5);
  spigots.slice(0, 8).forEach((x, i) => (sp[i] = x));
  const uniforms = {
    tNoise: { value: noiseTexture() },
    uLevel: w.level,
    uWet: w.wet,
    uTime: w.time,
    uSpig: { value: sp },
    cDry: { value: C("#b9b5aa") },
    cDamp: { value: C("#9c9992") },
    cWet: { value: C("#6c6a66") },
    cSalt: { value: C("#d9d6cc") },
    cTop: { value: BEACH_TOP },
  };
  mat.customProgramCacheKey = () => "fm-beach-v3";
  mat.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, uniforms);
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vBW;")
      .replace("#include <worldpos_vertex>", "#include <worldpos_vertex>\nvBW = (modelMatrix * vec4(transformed, 1.0)).xyz;");
    sh.fragmentShader = sh.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
uniform sampler2D tNoise;
uniform float uLevel;
uniform float uWet;
uniform float uTime;
uniform float uSpig[8];
uniform vec3 cDry;
uniform vec3 cDamp;
uniform vec3 cWet;
uniform vec3 cSalt;
uniform float cTop;
varying vec3 vBW;
vec2 bHash(vec2 p) {
  p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
  return fract(sin(p) * 43758.5453);
}
// Distance to the nearest edge of a Voronoi cell: the pattern of drying cracks.
float bCrack(vec2 x) {
  vec2 n = floor(x);
  vec2 f = fract(x);
  vec2 mg = vec2(0.0);
  vec2 mr = vec2(0.0);
  float md = 8.0;
  for (int j = -1; j <= 1; j++)
    for (int i = -1; i <= 1; i++) {
      vec2 g = vec2(float(i), float(j));
      vec2 r = g + bHash(n + g) - f;
      float d = dot(r, r);
      if (d < md) { md = d; mr = r; mg = g; }
    }
  md = 8.0;
  for (int j = -1; j <= 1; j++)
    for (int i = -1; i <= 1; i++) {
      vec2 g = mg + vec2(float(i), float(j));
      vec2 r = g + bHash(n + g) - f;
      if (dot(mr - r, mr - r) > 1e-5) md = min(md, dot(0.5 * (mr + r), normalize(r - mr)));
    }
  return md;
}
float bMoist;`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
{
  vec2 p = vBW.xz;
  float d = 1e-4 * p.x * p.x - 7.0 - p.y;
  vec4 broad = texture2D(tNoise, p * 0.0019 + 0.3);
  vec4 mid = texture2D(tNoise, p * 0.017 + 0.61);
  vec4 fine = texture2D(tNoise, p * 0.19);
  // Deposition streaks run down the beach, away from the spigots.
  vec4 streak = texture2D(tNoise, vec2(p.x * 0.013, d * 0.0028) + 0.17);
  vec4 lobe = texture2D(tNoise, vec2(p.x * 0.006, d * 0.004) + 0.53);
  // Mostly dry: older deposits, with long faint streaks where earlier flows ran.
  float m = 0.06 + 0.16 * smoothstep(0.45, 0.8, streak.r) + 0.12 * smoothstep(0.55, 0.85, lobe.b);
  // The active fans below the open spigots: wet, fresh, darker, with ragged edges, widening down the beach.
  float fan = 0.0;
  float rill = 0.0;
  for (int i = 0; i < 8; i++) {
    float w = 7.0 + 0.17 * d;
    float wander = 22.0 * sin(d * 0.021 + float(i) * 1.7) * smoothstep(0.0, 120.0, d);
    float dx = (p.x - uSpig[i] - wander) / w;
    float g = exp(-dx * dx) * smoothstep(-2.0, 8.0, d) * (1.0 - smoothstep(200.0, 320.0, d));
    float f = smoothstep(0.4, 0.5, g + 0.26 * (mid.r - 0.5) + 0.05 * (fine.b - 0.5));
    // Braided rills inside the fan, where the slurry runs thickest.
    float braid = abs(texture2D(tNoise, vec2((p.x - uSpig[i] - wander) * 0.045, d * 0.006 + float(i) * 0.13)).r - 0.5);
    f *= 0.82 + 0.18 * smoothstep(0.02, 0.07, braid);
    rill = max(rill, (1.0 - smoothstep(0.012, 0.035, braid)) * step(0.5, f));
    fan = max(fan, f);
    // An older lobe beside each, abandoned and drying.
    float dx2 = (p.x - uSpig[i] - wander + 46.0) / (w * 0.8);
    float g2 = exp(-dx2 * dx2) * smoothstep(10.0, 40.0, d) * (1.0 - smoothstep(160.0, 260.0, d));
    m = max(m, 0.42 * smoothstep(0.35, 0.5, g2 + 0.25 * (mid.g - 0.5)));
  }
  m = max(m, fan * (0.66 + 0.3 * streak.g));
  float above = vBW.y - uLevel;
  // Saturated near the water, and wetter towards it as the beach flattens.
  m += 0.85 * (1.0 - smoothstep(0.0, 0.12, above)) + 0.3 * (1.0 - smoothstep(0.08, 0.45, above));
  m = max(m, rill);
  m = clamp(m, 0.0, 1.0);
  m = max(m, uWet * (0.82 + 0.18 * fine.g));
  bMoist = m;
  vec3 col = mix(cDry, cDamp, smoothstep(0.25, 0.6, m));
  col = mix(col, cWet, smoothstep(0.62, 0.95, m));
  col *= 0.9 + 0.16 * mid.b + 0.08 * broad.r - 0.04;
  // Salt crust where it has dried longest.
  float salt = (1.0 - smoothstep(0.15, 0.4, m)) * smoothstep(0.58, 0.82, mid.g * 0.65 + broad.b * 0.55);
  col = mix(col, cSalt, salt * 0.55);
  // Drying cracks, in two sizes, fading out once a pixel covers more than a crack.
  float px = length(fwidth(p));
  float dryK = 1.0 - smoothstep(0.2, 0.55, m);
  float big = 1.0 - smoothstep(0.03, 0.09, bCrack(p * 0.55 + 3.1) / max(1.0, px * 1.2));
  float small = 1.0 - smoothstep(0.03, 0.1, bCrack(p * 1.9) / max(1.0, px * 2.5));
  float cr = (big * (1.0 - smoothstep(0.6, 2.2, px)) + small * 0.7 * (1.0 - smoothstep(0.18, 0.7, px))) * dryK;
  col *= 1.0 - 0.45 * clamp(cr, 0.0, 1.0);
  // A thin dark line where the water meets the beach.
  col *= 1.0 - 0.25 * (1.0 - smoothstep(0.0, 0.06, abs(above)));
  diffuseColor.rgb = col;
}`,
      )
      .replace(
        "#include <roughnessmap_fragment>",
        `#include <roughnessmap_fragment>
roughnessFactor = mix(0.94, 0.3, smoothstep(0.6, 1.0, bMoist));
roughnessFactor = mix(roughnessFactor, 0.12, (1.0 - smoothstep(0.0, 0.12, vBW.y - uLevel)) * step(0.0, vBW.y - uLevel + 0.02));`,
      );
  };
  return mat;
}

export async function makeBeach(o: { small: boolean; pause: Pause; weather: Weather; spigots: number[] }): Promise<Beach> {
  const step = o.small ? 5 : 2.8;
  const xs: number[] = [];
  for (let x = -600; x <= 370; x += step) xs.push(x);
  const ds: number[] = [-1.6];
  for (let d = 0; d <= 980; d += d < 30 ? step * 0.6 : step) ds.push(d);
  const nx = xs.length;
  const nd = ds.length;
  const pos = new Float32Array(nx * nd * 3);
  const uv = new Float32Array(nx * nd * 2);
  for (let a = 0; a < nx; a++) {
    const x = xs[a]!;
    for (let b = 0; b < nd; b++) {
      const z = crestZ(x) - DAM.crestHalf - ds[b]!;
      let y = tailings(x, z);
      // Far below the hillside the tailings are hidden; keep the triangles short there.
      const g = natural(x, z);
      if (y < g - 6) y = g - 6;
      const i = a * nd + b;
      pos[i * 3] = x;
      pos[i * 3 + 1] = y;
      pos[i * 3 + 2] = z;
      uv[i * 2] = x * 0.6;
      uv[i * 2 + 1] = z * 0.6;
    }
    if (a % 60 === 59) await o.pause();
  }
  const index = new Uint32Array((nx - 1) * (nd - 1) * 6);
  let t = 0;
  for (let a = 0; a < nx - 1; a++) {
    for (let b = 0; b < nd - 1; b++) {
      const p = a * nd + b;
      index[t++] = p;
      index[t++] = p + nd;
      index[t++] = p + 1;
      index[t++] = p + 1;
      index[t++] = p + nd;
      index[t++] = p + nd + 1;
    }
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  geo.setIndex(new THREE.BufferAttribute(index, 1));
  geo.computeVertexNormals();
  const material = beachMaterial(o.weather, o.spigots);
  const mesh = new THREE.Mesh(geo, material);
  mesh.receiveShadow = true;
  mesh.name = "beach";
  return { mesh, material };
}
