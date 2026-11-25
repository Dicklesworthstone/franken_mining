/**
 * Shared kit for the fleet figures: seeded noise, the weathered-paint shader patch, a bake that merges
 * a part tree into one mesh per material, and helpers for the parts every machine shares (rounded
 * boxes, plates, bars, handrails, stairs, ladders, lamps, beacons, mirrors).
 *
 * Nothing here touches the DOM: textures are DataTextures, so Bun scripts can import and even build
 * the models without a window. Units are metres, y up.
 */
import * as THREE from "three";
import { RoundedBoxGeometry } from "three/examples/jsm/geometries/RoundedBoxGeometry.js";

export type V3 = THREE.Vector3;
export const v3 = (x = 0, y = 0, z = 0) => new THREE.Vector3(x, y, z);
const UP = new THREE.Vector3(0, 1, 0);

// ───────────────────────────── randomness and noise ─────────────────────────────

/** mulberry32: repeatable from its seed. */
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const fade = (t: number) => t * t * t * (t * (t * 6 - 15) + 10);
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
export const clamp01 = (v: number) => (v < 0 ? 0 : v > 1 ? 1 : v);
export const smoothstep = (a: number, b: number, v: number) => {
  const t = clamp01((v - a) / (b - a));
  return t * t * (3 - 2 * t);
};

/** Improved Perlin noise in 3D, seeded. Output roughly in [-1, 1]. */
export class Noise3 {
  private readonly p = new Uint8Array(512);
  constructor(seed = 1) {
    const r = rng(seed);
    const q = new Uint8Array(256);
    for (let i = 0; i < 256; i++) q[i] = i;
    for (let i = 255; i > 0; i--) {
      const j = Math.floor(r() * (i + 1));
      const t = q[i]!;
      q[i] = q[j]!;
      q[j] = t;
    }
    for (let i = 0; i < 512; i++) this.p[i] = q[i & 255]!;
  }
  private static grad(h: number, x: number, y: number, z: number) {
    const k = h & 15;
    const u = k < 8 ? x : y;
    const v = k < 4 ? y : k === 12 || k === 14 ? x : z;
    return ((k & 1) === 0 ? u : -u) + ((k & 2) === 0 ? v : -v);
  }
  noise(x: number, y: number, z: number) {
    const P = this.p;
    const X = Math.floor(x) & 255;
    const Y = Math.floor(y) & 255;
    const Z = Math.floor(z) & 255;
    x -= Math.floor(x);
    y -= Math.floor(y);
    z -= Math.floor(z);
    const u = fade(x);
    const v = fade(y);
    const w = fade(z);
    const A = P[X]! + Y;
    const AA = P[A]! + Z;
    const AB = P[A + 1]! + Z;
    const B = P[X + 1]! + Y;
    const BA = P[B]! + Z;
    const BB = P[B + 1]! + Z;
    const g = Noise3.grad;
    return lerp(
      lerp(lerp(g(P[AA]!, x, y, z), g(P[BA]!, x - 1, y, z), u), lerp(g(P[AB]!, x, y - 1, z), g(P[BB]!, x - 1, y - 1, z), u), v),
      lerp(
        lerp(g(P[AA + 1]!, x, y, z - 1), g(P[BA + 1]!, x - 1, y, z - 1), u),
        lerp(g(P[AB + 1]!, x, y - 1, z - 1), g(P[BB + 1]!, x - 1, y - 1, z - 1), u),
        v,
      ),
      w,
    );
  }
  fbm(x: number, y: number, z: number, octaves = 4, gain = 0.5) {
    let a = 1;
    let f = 1;
    let s = 0;
    let n = 0;
    for (let i = 0; i < octaves; i++) {
      s += a * this.noise(x * f, y * f, z * f);
      n += a;
      a *= gain;
      f *= 2.03;
    }
    return s / n;
  }
}

/** Tileable value-noise fBm on a size×size grid, normalised to 0..1. */
function tileFbm(size: number, period: number, octaves: number, seed: number, gain = 0.55): Float32Array {
  const r = rng(seed);
  const out = new Float32Array(size * size);
  let amp = 1;
  for (let o = 0; o < octaves; o++) {
    const per = period << o;
    const g = new Float32Array(per * per);
    for (let i = 0; i < g.length; i++) g[i] = r();
    for (let y = 0; y < size; y++) {
      const fy = (y / size) * per;
      const iy = Math.floor(fy);
      const ty = fade(fy - iy);
      const y0 = (iy % per) * per;
      const y1 = ((iy + 1) % per) * per;
      for (let x = 0; x < size; x++) {
        const fx = (x / size) * per;
        const ix = Math.floor(fx);
        const tx = fade(fx - ix);
        const x0 = ix % per;
        const x1 = (ix + 1) % per;
        const v = lerp(lerp(g[y0 + x0]!, g[y0 + x1]!, tx), lerp(g[y1 + x0]!, g[y1 + x1]!, tx), ty);
        const k = y * size + x;
        out[k] = out[k]! + v * amp;
      }
    }
    amp *= gain;
  }
  let lo = Infinity;
  let hi = -Infinity;
  for (const v of out) {
    lo = Math.min(lo, v);
    hi = Math.max(hi, v);
  }
  for (let i = 0; i < out.length; i++) out[i] = (out[i]! - lo) / (hi - lo || 1);
  return out;
}

function dataTexture(data: Uint8Array, size: number, colour = false): THREE.DataTexture {
  const t = new THREE.DataTexture(data, size, size, THREE.RGBAFormat);
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.magFilter = THREE.LinearFilter;
  t.minFilter = THREE.LinearMipmapLinearFilter;
  t.generateMipmaps = true;
  t.anisotropy = 4;
  t.colorSpace = colour ? THREE.SRGBColorSpace : THREE.NoColorSpace;
  t.needsUpdate = true;
  return t;
}

let noiseTex: THREE.DataTexture | null = null;
/** RGBA tileable noise: r broad blotches, g a second blotch field, b fine grain, a streaks. */
export function noiseTexture(): THREE.DataTexture {
  if (noiseTex) return noiseTex;
  const S = 256;
  const r = tileFbm(S, 4, 5, 11);
  const g = tileFbm(S, 6, 4, 23);
  const b = tileFbm(S, 32, 3, 37);
  const a = tileFbm(S, 16, 3, 41);
  const d = new Uint8Array(S * S * 4);
  for (let i = 0; i < S * S; i++) {
    d[i * 4] = Math.round(r[i]! * 255);
    d[i * 4 + 1] = Math.round(g[i]! * 255);
    d[i * 4 + 2] = Math.round(b[i]! * 255);
    d[i * 4 + 3] = Math.round(a[i]! * 255);
  }
  noiseTex = dataTexture(d, S);
  return noiseTex;
}

let gravelTex: THREE.DataTexture | null = null;
/** Tileable normal map of packed pebbles and grit, for rock piles, ROM pads and haul roads. */
export function gravelNormal(): THREE.DataTexture {
  if (gravelTex) return gravelTex;
  const S = 256;
  const cells = 22;
  const r = rng(77);
  const pts = new Float32Array(cells * cells * 3);
  for (let i = 0; i < cells * cells; i++) {
    pts[i * 3] = r();
    pts[i * 3 + 1] = r();
    pts[i * 3 + 2] = 0.55 + r() * 0.6;
  }
  const grit = tileFbm(S, 64, 2, 5);
  const h = new Float32Array(S * S);
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const fx = (x / S) * cells;
      const fy = (y / S) * cells;
      const cx = Math.floor(fx);
      const cy = Math.floor(fy);
      let d1 = 9;
      let d2 = 9;
      let hs = 1;
      for (let oy = -1; oy <= 1; oy++) {
        for (let ox = -1; ox <= 1; ox++) {
          const gx = (cx + ox + cells) % cells;
          const gy = (cy + oy + cells) % cells;
          const k = (gy * cells + gx) * 3;
          const px = cx + ox + pts[k]!;
          const py = cy + oy + pts[k + 1]!;
          const dd = Math.hypot(px - fx, py - fy);
          if (dd < d1) {
            d2 = d1;
            d1 = dd;
            hs = pts[k + 2]!;
          } else if (dd < d2) d2 = dd;
        }
      }
      const edge = Math.min(1, (d2 - d1) * 3.2);
      h[y * S + x] = Math.sqrt(edge) * hs + grit[y * S + x]! * 0.35;
    }
  }
  const d = new Uint8Array(S * S * 4);
  const k = 2.2;
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const hx = h[y * S + ((x + 1) % S)]! - h[y * S + ((x - 1 + S) % S)]!;
      const hy = h[((y + 1) % S) * S + x]! - h[((y - 1 + S) % S) * S + x]!;
      const nx = -hx * k;
      const ny = -hy * k;
      const l = Math.hypot(nx, ny, 1);
      const i = (y * S + x) * 4;
      d[i] = Math.round((nx / l * 0.5 + 0.5) * 255);
      d[i + 1] = Math.round((ny / l * 0.5 + 0.5) * 255);
      d[i + 2] = Math.round((1 / l * 0.5 + 0.5) * 255);
      d[i + 3] = 255;
    }
  }
  gravelTex = dataTexture(d, S);
  return gravelTex;
}

// ───────────────────────────── weathered materials ─────────────────────────────

export interface Grime {
  /** Colour of caked mud and road grime. */
  mud?: THREE.ColorRepresentation;
  /** Colour of dust settled on up-facing surfaces. */
  dust?: THREE.ColorRepresentation;
  /** 0..1 multiplier on the baked mud mask. */
  mudAmount?: number;
  /** 0..1 multiplier on the baked dust mask. */
  dustAmount?: number;
  /** Noise frequency in cycles per metre for the blotches. */
  scale?: number;
  /** 0..1 strength of the mottled colour and roughness variation of the base surface. */
  variation?: number;
}

export const SITE = { mud: "#6e5c48", dust: "#b9a586" } as const;

const GRIME_KEY = "fleet-grime-v3";

/**
 * Patches a standard/physical material so it reads as worn site equipment. The mesh geometry carries a
 * `wear` attribute (x = mud mask, y = dust mask, z = cavity darkening) baked by {@link bake}; the shader
 * breaks those masks up with tri-planar noise, adds drips on vertical faces, dulls the clearcoat and
 * roughens the surface where dirt sits.
 */
export function grime<M extends THREE.MeshStandardMaterial>(mat: M, o: Grime = {}): M {
  const uniforms = {
    gNoise: { value: noiseTexture() },
    gMudC: { value: new THREE.Color(o.mud ?? SITE.mud) },
    gDustC: { value: new THREE.Color(o.dust ?? SITE.dust) },
    gAmt: { value: new THREE.Vector4(o.mudAmount ?? 1, o.dustAmount ?? 1, o.scale ?? 0.16, o.variation ?? 1) },
  };
  mat.userData.grime = uniforms;
  mat.customProgramCacheKey = () => GRIME_KEY;
  mat.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, uniforms);
    shader.vertexShader = shader.vertexShader
      .replace(
        "#include <common>",
        `#include <common>
attribute vec3 wear;
varying vec3 vWear;
varying vec3 vGPos;
varying vec3 vGNrm;`,
      )
      .replace(
        "#include <begin_vertex>",
        `#include <begin_vertex>
vWear = wear;
vec4 gP = vec4(position, 1.0);
vec3 gNo = normal;
#ifdef USE_INSTANCING
gP = instanceMatrix * gP;
gNo = mat3(instanceMatrix) * gNo;
#endif
vGPos = gP.xyz;
vGNrm = gNo;`,
      );
    shader.fragmentShader = shader.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
uniform sampler2D gNoise;
uniform vec3 gMudC;
uniform vec3 gDustC;
uniform vec4 gAmt;
varying vec3 vWear;
varying vec3 vGPos;
varying vec3 vGNrm;
vec4 gTri(vec3 p, vec3 n) {
  vec3 w = pow(abs(n), vec3(4.0));
  w /= (w.x + w.y + w.z + 1e-4);
  return texture2D(gNoise, p.zy) * w.x + texture2D(gNoise, p.xz) * w.y + texture2D(gNoise, p.xy) * w.z;
}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec3 gN = normalize(vGNrm);
vec4 gA = gTri(vGPos * gAmt.z, gN);
vec4 gB = gTri(vGPos * (gAmt.z * 5.3) + 0.37, gN);
float gVert = 1.0 - abs(gN.y);
float gDrip = texture2D(gNoise, vec2((vGPos.x + vGPos.z) * gAmt.z * 9.0, vGPos.y * gAmt.z * 0.45)).a;
float gM = vWear.x * gAmt.x;
float gSpl = gA.r * 0.55 + gB.g * 0.45;
float gMud = smoothstep(0.42, 0.78, gM * 0.95 + (gSpl - 0.5) * 1.0 + (gDrip - 0.5) * 0.22 * gVert * step(0.02, gM));
vec3 gMudCol = mix(gMudC * 0.55, gMudC * 1.2, gB.b);
float gD = vWear.y * gAmt.y;
float gDust = smoothstep(0.12, 0.85, gD * (0.55 + 0.9 * gA.g) + (gB.r - 0.5) * 0.3) * min(1.0, gD * 1.6);
diffuseColor.rgb *= mix(1.0, 0.93 + 0.12 * gB.b, gAmt.w);
diffuseColor.rgb = mix(diffuseColor.rgb, diffuseColor.rgb * gMudC * 1.7, clamp(gM * 0.55, 0.0, 0.55));
diffuseColor.rgb = mix(diffuseColor.rgb, gMudCol, gMud * 0.9);
diffuseColor.rgb = mix(diffuseColor.rgb, gDustC * (0.85 + 0.25 * gB.g), gDust * 0.85);
diffuseColor.rgb *= 1.0 - vWear.z;
float gDirt = max(gMud, gDust);`,
      )
      .replace(
        "#include <roughnessmap_fragment>",
        `#include <roughnessmap_fragment>
roughnessFactor = clamp(roughnessFactor + (gB.r - 0.5) * 0.14 * gAmt.w + gM * 0.25, 0.03, 1.0);
roughnessFactor = mix(roughnessFactor, 0.96, gDirt);`,
      )
      .replace(
        "#include <metalnessmap_fragment>",
        `#include <metalnessmap_fragment>
metalnessFactor *= 1.0 - gDirt;`,
      )
      .replace(
        "#include <lights_physical_fragment>",
        `#include <lights_physical_fragment>
#ifdef USE_CLEARCOAT
material.clearcoat *= 1.0 - gDirt;
#endif`,
      );
  };
  return mat;
}

const matCache = new Map<string, THREE.Material>();
function cached<M extends THREE.Material>(key: string, make: () => M): M {
  const hit = matCache.get(key);
  if (hit) return hit as M;
  const m = make();
  m.name = key;
  matCache.set(key, m);
  return m;
}

/** Industrial enamel with a light clearcoat, weathered by {@link grime}. Cached per colour and options. */
export function paint(colour: string, o: Grime & { rough?: number; coat?: number; metal?: number } = {}) {
  return cached(`paint:${colour}:${JSON.stringify(o)}`, () =>
    grime(
      new THREE.MeshPhysicalMaterial({
        color: colour,
        roughness: o.rough ?? 0.48,
        metalness: o.metal ?? 0,
        clearcoat: o.coat ?? 0.45,
        clearcoatRoughness: 0.32,
      }),
      o,
    ),
  );
}

export const COLOURS = {
  yellow: "#eca614",
  white: "#e9e8e2",
  cream: "#ddd6c6",
  dark: "#2b2b29",
  grey: "#8c8e8b",
  red: "#b0241c",
  orange: "#e8620f",
} as const;

/** The shared material set. Lenses are per instance (see {@link lenses}) so lights switch independently. */
export const mats = {
  yellow: () => paint(COLOURS.yellow),
  dark: () => paint(COLOURS.dark, { rough: 0.62, coat: 0.12 }),
  black: () => paint("#161615", { rough: 0.7, coat: 0.05, variation: 0.6 }),
  rimGrey: () => paint("#9a9992", { rough: 0.55, coat: 0.2 }),
  red: () => paint(COLOURS.red, { coat: 0.6 }),
  white: () => paint(COLOURS.white, { coat: 0.6, rough: 0.38 }),
  orange: () => paint(COLOURS.orange, { coat: 0.4 }),
  steel: () =>
    cached("steel", () => grime(new THREE.MeshStandardMaterial({ color: "#8d9093", metalness: 0.85, roughness: 0.42 }), { mudAmount: 0.8 })),
  galv: () =>
    cached("galv", () => grime(new THREE.MeshStandardMaterial({ color: "#a3a7a6", metalness: 0.7, roughness: 0.5 }), { variation: 1 })),
  track: () =>
    cached("track", () => grime(new THREE.MeshStandardMaterial({ color: "#4b4a47", metalness: 0.75, roughness: 0.55 }), { mudAmount: 0.7, dustAmount: 0.8 })),
  chrome: () => cached("chrome", () => new THREE.MeshStandardMaterial({ color: "#e4e6e8", metalness: 1, roughness: 0.1 })),
  rubber: () =>
    cached("rubber", () =>
      grime(new THREE.MeshStandardMaterial({ color: "#1d1d1c", metalness: 0, roughness: 0.9 }), {
        dust: "#a08d73",
        mud: "#4d4034",
        variation: 0.5,
        scale: 0.5,
      }),
    ),
  hose: () => cached("hose", () => new THREE.MeshStandardMaterial({ color: "#151515", roughness: 0.55 })),
  glass: () =>
    cached(
      "glass",
      () =>
        new THREE.MeshPhysicalMaterial({
          color: "#1a242b",
          metalness: 0.1,
          roughness: 0.04,
          clearcoat: 1,
          clearcoatRoughness: 0.02,
          envMapIntensity: 3.2,
          reflectivity: 0.75,
        }),
    ),
  grate: () =>
    cached("grate", () => grime(new THREE.MeshStandardMaterial({ color: "#555853", metalness: 0.6, roughness: 0.55 }), { dustAmount: 1.3 })),
  concrete: () =>
    cached("concrete", () => grime(new THREE.MeshStandardMaterial({ color: "#a9a59c", roughness: 0.93 }), { scale: 0.08, mudAmount: 0.8 })),
  reflector: () =>
    cached("reflector", () => new THREE.MeshStandardMaterial({ color: "#d9d4c7", roughness: 0.25, metalness: 0.3 })),
};

export interface Lenses {
  white: THREE.MeshStandardMaterial;
  amber: THREE.MeshStandardMaterial;
  red: THREE.MeshStandardMaterial;
  /** Lights on or off: emissive at night, a dull lens by day. */
  set(on: boolean, strength?: number): void;
}
/** A fresh set of lamp lens materials for one machine, so each can switch its lights on its own. */
export function lenses(): Lenses {
  const mk = (colour: string, glow: string) =>
    new THREE.MeshStandardMaterial({ color: colour, roughness: 0.12, metalness: 0.25, emissive: glow, emissiveIntensity: 0 });
  const l: Lenses = {
    white: mk("#dfe6ea", "#fff4dc"),
    amber: mk("#d88a1a", "#ffa21a"),
    red: mk("#9a1712", "#ff2a14"),
    set(on, strength = 4) {
      l.white.emissiveIntensity = on ? strength : 0.06;
      l.amber.emissiveIntensity = on ? strength * 0.7 : 0.08;
      l.red.emissiveIntensity = on ? strength * 0.6 : 0.05;
    },
  };
  l.white.name = "lens:white";
  l.amber.name = "lens:amber";
  l.red.name = "lens:red";
  l.set(false);
  return l;
}

// ───────────────────────────── bake: merge a part tree by material ─────────────────────────────

export interface Wear {
  /** Height (in the wear frame) above which there is no road mud. */
  mudTop?: number;
  /** Height at and below which the mud mask is full. */
  mudBottom?: number;
  /** Mud multiplier. */
  mud?: number;
  /** Dust multiplier for up-facing surfaces. */
  dust?: number;
  /** Extra darkening on surfaces that face the ground. */
  under?: number;
}

/** Per-mesh override read by {@link bake} from `mesh.userData.wear`. */
export interface PartWear {
  mud?: number;
  dust?: number;
  ao?: number;
}

const _m = new THREE.Matrix4();
const _n = new THREE.Matrix3();
const _p = new THREE.Vector3();
const _q = new THREE.Vector3();

/**
 * Merges every mesh under `root` into one mesh per material, in `root`'s frame. Each vertex gets a
 * `wear` attribute from its height and facing in the wear frame (`frame` maps root space into it,
 * default identity), so mud climbs from the ground and dust settles on tops, whatever moves later.
 */
export function bake(root: THREE.Object3D, wear: Wear = {}, frame?: THREE.Matrix4): THREE.Group {
  const mudTop = wear.mudTop ?? 3;
  const mudBottom = wear.mudBottom ?? 0.3;
  const mudK = wear.mud ?? 1;
  const dustK = wear.dust ?? 1;
  const under = wear.under ?? 0.22;
  root.updateMatrixWorld(true);
  const inv = root.matrixWorld.clone().invert();
  const fN = new THREE.Matrix3();
  if (frame) fN.getNormalMatrix(frame);
  const groups = new Map<THREE.Material, { geo: THREE.BufferGeometry; m: THREE.Matrix4; w: PartWear; shadow: boolean }[]>();
  root.traverse((o) => {
    const mesh = o as THREE.Mesh;
    if (!mesh.isMesh || (o as THREE.InstancedMesh).isInstancedMesh) return;
    const mat = mesh.material as THREE.Material;
    const list = groups.get(mat) ?? [];
    list.push({
      geo: mesh.geometry,
      m: new THREE.Matrix4().multiplyMatrices(inv, mesh.matrixWorld),
      w: (mesh.userData.wear as PartWear | undefined) ?? {},
      shadow: mesh.userData.noShadow !== true,
    });
    groups.set(mat, list);
  });
  const out = new THREE.Group();
  for (const [mat, list] of groups) {
    let nv = 0;
    let ni = 0;
    for (const { geo } of list) {
      const c = geo.getAttribute("position").count;
      nv += c;
      ni += geo.index ? geo.index.count : c;
    }
    const pos = new Float32Array(nv * 3);
    const nrm = new Float32Array(nv * 3);
    const uv = new Float32Array(nv * 2);
    const wr = new Float32Array(nv * 3);
    const idx = nv > 65535 ? new Uint32Array(ni) : new Uint16Array(ni);
    let vo = 0;
    let io = 0;
    for (const { geo, m, w } of list) {
      if (!geo.getAttribute("normal")) geo.computeVertexNormals();
      const P = geo.getAttribute("position");
      const N = geo.getAttribute("normal");
      const U = geo.getAttribute("uv");
      _n.getNormalMatrix(m);
      const flip = m.determinant() < 0;
      _m.copy(m);
      if (frame) _m.premultiply(frame);
      const nm = frame ? new THREE.Matrix3().multiplyMatrices(fN, _n) : _n;
      for (let i = 0; i < P.count; i++) {
        _p.fromBufferAttribute(P, i).applyMatrix4(m);
        pos[(vo + i) * 3] = _p.x;
        pos[(vo + i) * 3 + 1] = _p.y;
        pos[(vo + i) * 3 + 2] = _p.z;
        _q.fromBufferAttribute(N, i).applyMatrix3(_n).normalize();
        nrm[(vo + i) * 3] = _q.x;
        nrm[(vo + i) * 3 + 1] = _q.y;
        nrm[(vo + i) * 3 + 2] = _q.z;
        if (U) {
          uv[(vo + i) * 2] = U.getX(i);
          uv[(vo + i) * 2 + 1] = U.getY(i);
        }
        // wear in the wear frame
        _p.fromBufferAttribute(P, i).applyMatrix4(_m);
        _q.fromBufferAttribute(N, i).applyMatrix3(nm).normalize();
        const mud = smoothstep(mudTop, mudBottom, _p.y) * mudK + (_q.y < -0.4 ? 0.25 * mudK : 0);
        const dust = smoothstep(0.35, 0.92, _q.y) * dustK * 0.85 + 0.04 * dustK;
        const ao = (_q.y < -0.3 ? under : 0) + smoothstep(1.2, 0, _p.y) * 0.12;
        wr[(vo + i) * 3] = clamp01(mud * (w.mud ?? 1));
        wr[(vo + i) * 3 + 1] = clamp01(dust * (w.dust ?? 1));
        wr[(vo + i) * 3 + 2] = Math.min(0.6, ao + (w.ao ?? 0));
      }
      if (geo.index) {
        const I = geo.index;
        for (let t = 0; t < I.count; t += 3) {
          const a = I.getX(t);
          const b = I.getX(t + 1);
          const c = I.getX(t + 2);
          idx[io++] = vo + a;
          idx[io++] = vo + (flip ? c : b);
          idx[io++] = vo + (flip ? b : c);
        }
      } else {
        for (let t = 0; t < P.count; t += 3) {
          idx[io++] = vo + t;
          idx[io++] = vo + (flip ? t + 2 : t + 1);
          idx[io++] = vo + (flip ? t + 1 : t + 2);
        }
      }
      vo += P.count;
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    g.setAttribute("normal", new THREE.BufferAttribute(nrm, 3));
    g.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
    g.setAttribute("wear", new THREE.BufferAttribute(wr, 3));
    g.setIndex(new THREE.BufferAttribute(idx, 1));
    g.computeBoundingSphere();
    g.computeBoundingBox();
    const mesh = new THREE.Mesh(g, mat);
    mesh.name = mat.name;
    mesh.castShadow = list.some((l) => l.shadow);
    mesh.receiveShadow = true;
    out.add(mesh);
  }
  return out;
}

/** Disposes the temporary source geometries of a part tree after baking. */
export function discard(root: THREE.Object3D) {
  root.traverse((o) => {
    const mesh = o as THREE.Mesh;
    if (mesh.isMesh) mesh.geometry.dispose();
  });
}

/** Bakes then frees the source tree. */
export function bakeFree(root: THREE.Object3D, wear?: Wear, frame?: THREE.Matrix4) {
  const g = bake(root, wear, frame);
  discard(root);
  return g;
}

/** Draw calls and triangles of an object tree as the renderer will see them. */
export function stats(root: THREE.Object3D): { drawCalls: number; triangles: number } {
  let drawCalls = 0;
  let triangles = 0;
  root.traverse((o) => {
    const mesh = o as THREE.Mesh;
    if (!mesh.isMesh || !o.visible) return;
    let vis = true;
    for (let p: THREE.Object3D | null = o; p; p = p.parent) if (!p.visible) vis = false;
    if (!vis) return;
    const g = mesh.geometry;
    const n = (g.index ? g.index.count : g.getAttribute("position").count) / 3;
    const inst = (o as THREE.InstancedMesh).isInstancedMesh ? (o as THREE.InstancedMesh).count : 1;
    drawCalls += 1;
    triangles += Math.round(n * inst);
  });
  return { drawCalls, triangles };
}

/** Sets castShadow/receiveShadow on every mesh. */
export function shadows(root: THREE.Object3D, cast = true, receive = true) {
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (m.isMesh) {
      m.castShadow = cast;
      m.receiveShadow = receive;
    }
  });
  return root;
}

/** Clones an object tree sharing geometry and materials (cheap instances of a built template). */
export function instance<T extends THREE.Object3D>(template: T): T {
  return template.clone(true);
}

// ───────────────────────────── part helpers ─────────────────────────────

export interface Place {
  rx?: number;
  ry?: number;
  rz?: number;
  wear?: PartWear;
}

function place<T extends THREE.Object3D>(o: T, parent: THREE.Object3D, x: number, y: number, z: number, p?: Place): T {
  o.position.set(x, y, z);
  if (p) {
    o.rotation.set(p.rx ?? 0, p.ry ?? 0, p.rz ?? 0);
    if (p.wear) o.userData.wear = p.wear;
  }
  parent.add(o);
  return o;
}

/** A group placed in a parent, for authoring sub-assemblies. */
export function node(parent: THREE.Object3D, x = 0, y = 0, z = 0, p?: Place): THREE.Group {
  return place(new THREE.Group(), parent, x, y, z, p);
}

/** Box centred at (x, y, z). `r` > 0 rounds the edges (default a small chamfer so nothing looks primitive). */
export function box(
  parent: THREE.Object3D,
  mat: THREE.Material,
  w: number,
  h: number,
  d: number,
  x: number,
  y: number,
  z: number,
  p: Place & { r?: number; seg?: number } = {},
): THREE.Mesh {
  const r = p.r ?? Math.min(0.04, Math.min(w, h, d) * 0.2);
  const g = r > 0.001 ? new RoundedBoxGeometry(w, h, d, p.seg ?? 1, r) : new THREE.BoxGeometry(w, h, d);
  return place(new THREE.Mesh(g, mat), parent, x, y, z, p);
}

/** Cylinder along an axis, centred at (x, y, z). */
export function cyl(
  parent: THREE.Object3D,
  mat: THREE.Material,
  r: number,
  len: number,
  x: number,
  y: number,
  z: number,
  p: Place & { axis?: "x" | "y" | "z"; seg?: number; r2?: number; open?: boolean } = {},
): THREE.Mesh {
  const g = new THREE.CylinderGeometry(p.r2 ?? r, r, len, p.seg ?? 16, 1, p.open ?? false);
  if (p.axis === "x") g.rotateZ(-Math.PI / 2);
  else if (p.axis === "z") g.rotateX(Math.PI / 2);
  return place(new THREE.Mesh(g, mat), parent, x, y, z, p);
}

export function sphere(parent: THREE.Object3D, mat: THREE.Material, r: number, x: number, y: number, z: number, p: Place & { seg?: number; sy?: number } = {}) {
  const s = p.seg ?? 12;
  const g = new THREE.SphereGeometry(r, s, Math.max(4, Math.round(s * 0.6)));
  if (p.sy) g.scale(1, p.sy, 1);
  return place(new THREE.Mesh(g, mat), parent, x, y, z, p);
}

/** Round bar between two points (radius r at a, r2 at b). */
export function bar(parent: THREE.Object3D, mat: THREE.Material, a: V3, b: V3, r: number, seg = 8, r2?: number, open = true): THREE.Mesh {
  const d = b.clone().sub(a);
  const len = d.length();
  const g = new THREE.CylinderGeometry(r2 ?? r, r, len, seg, 1, open);
  const m = new THREE.Mesh(g, mat);
  m.position.copy(a).addScaledVector(d, 0.5);
  if (len > 1e-6) m.quaternion.setFromUnitVectors(UP, d.normalize());
  parent.add(m);
  return m;
}

/** Square-section member between two points (for frames, trusses, booms). `up` sets the roll. */
export function beam(parent: THREE.Object3D, mat: THREE.Material, a: V3, b: V3, w: number, h = w, up: V3 = UP, r = 0): THREE.Mesh {
  const d = b.clone().sub(a);
  const len = d.length();
  const g = r > 0 ? new RoundedBoxGeometry(w, h, len, 1, r) : new THREE.BoxGeometry(w, h, len);
  const m = new THREE.Mesh(g, mat);
  m.position.copy(a).addScaledVector(d, 0.5);
  const z = d.clone().normalize();
  const upv = Math.abs(z.dot(up)) > 0.99 ? new THREE.Vector3(1, 0, 0) : up;
  const x = new THREE.Vector3().crossVectors(upv, z).normalize();
  const y = new THREE.Vector3().crossVectors(z, x);
  m.quaternion.setFromRotationMatrix(new THREE.Matrix4().makeBasis(x, y, z));
  parent.add(m);
  return m;
}

/** Tube through points (smoothed by a centripetal Catmull–Rom unless `sharp`). */
export function tube(parent: THREE.Object3D, mat: THREE.Material, pts: V3[], r: number, p: { seg?: number; radial?: number; closed?: boolean } = {}) {
  const curve = new THREE.CatmullRomCurve3(pts, p.closed ?? false, "centripetal");
  const g = new THREE.TubeGeometry(curve, p.seg ?? Math.max(8, pts.length * 6), r, p.radial ?? 8, p.closed ?? false);
  const m = new THREE.Mesh(g, mat);
  parent.add(m);
  return m;
}

/**
 * Flat plate of thickness t from a planar polygon (corners in order). The polygon is the plate's
 * outer face; the plate grows against its normal (`out` picks which side is outer). Flat-shaded.
 */
export function plate(parent: THREE.Object3D, mat: THREE.Material, pts: V3[], t: number, o: { wear?: PartWear; out?: V3 } = {}): THREE.Mesh {
  let corners = pts;
  const n = new THREE.Vector3();
  for (let i = 0; i < corners.length; i++) {
    const a = corners[i]!;
    const b = corners[(i + 1) % corners.length]!;
    n.x += (a.y - b.y) * (a.z + b.z);
    n.y += (a.z - b.z) * (a.x + b.x);
    n.z += (a.x - b.x) * (a.y + b.y);
  }
  n.normalize();
  if (o.out && n.dot(o.out) < 0) {
    corners = [...corners].reverse();
    n.negate();
  }
  const back = corners.map((c) => c.clone().addScaledVector(n, -t));
  const pos: number[] = [];
  const tri = (a: V3, b: V3, c: V3) => pos.push(a.x, a.y, a.z, b.x, b.y, b.z, c.x, c.y, c.z);
  const k = corners.length;
  for (let i = 1; i < k - 1; i++) {
    tri(corners[0]!, corners[i]!, corners[i + 1]!);
    tri(back[0]!, back[i + 1]!, back[i]!);
  }
  for (let i = 0; i < k; i++) {
    const a = corners[i]!;
    const b = corners[(i + 1) % k]!;
    const c = back[(i + 1) % k]!;
    const d = back[i]!;
    tri(a, d, c);
    tri(a, c, b);
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  g.computeVertexNormals();
  const m = new THREE.Mesh(g, mat);
  if (o.wear) m.userData.wear = o.wear;
  parent.add(m);
  return m;
}

/**
 * Prism from a 2D outline extruded with chamfered edges. `plane` says which world axes the outline's
 * (u, v) map to: "zy" side profile extruded across x, "xy" front profile extruded along z, "xz" plan
 * extruded up y. `at` is the centre of the extrusion along the remaining axis.
 */
export function prism(
  parent: THREE.Object3D,
  mat: THREE.Material,
  outline: [number, number][],
  depth: number,
  plane: "zy" | "xy" | "xz",
  at: number,
  p: { bevel?: number; curve?: number; holes?: [number, number][][]; wear?: PartWear } = {},
): THREE.Mesh {
  const shape = new THREE.Shape(outline.map(([u, v]) => new THREE.Vector2(u, v)));
  for (const h of p.holes ?? []) shape.holes.push(new THREE.Path(h.map(([u, v]) => new THREE.Vector2(u, v))));
  const bevel = p.bevel ?? Math.min(0.03, depth * 0.2);
  const g = new THREE.ExtrudeGeometry(shape, {
    depth: Math.max(0.001, depth - 2 * bevel),
    bevelEnabled: bevel > 0,
    bevelThickness: bevel,
    bevelSize: bevel,
    bevelOffset: -bevel,
    bevelSegments: 1,
    curveSegments: p.curve ?? 8,
  });
  g.translate(0, 0, -(depth - 2 * bevel) / 2);
  const m4 = new THREE.Matrix4();
  if (plane === "zy") m4.makeBasis(new THREE.Vector3(0, 0, 1), new THREE.Vector3(0, 1, 0), new THREE.Vector3(-1, 0, 0));
  else if (plane === "xz") m4.makeBasis(new THREE.Vector3(1, 0, 0), new THREE.Vector3(0, 0, 1), new THREE.Vector3(0, -1, 0));
  g.applyMatrix4(m4);
  if (plane === "zy") g.translate(at, 0, 0);
  else if (plane === "xy") g.translate(0, 0, at);
  else g.translate(0, at, 0);
  const m = new THREE.Mesh(g, mat);
  if (p.wear) m.userData.wear = p.wear;
  parent.add(m);
  return m;
}

/** Lathe around the local y axis from (radius, height) pairs, placed and optionally laid along x or z. */
export function lathe(
  parent: THREE.Object3D,
  mat: THREE.Material,
  profile: [number, number][],
  x: number,
  y: number,
  z: number,
  p: Place & { axis?: "x" | "y" | "z"; seg?: number } = {},
) {
  const g = new THREE.LatheGeometry(
    profile.map(([r, h]) => new THREE.Vector2(r, h)),
    p.seg ?? 24,
  );
  if (p.axis === "x") g.rotateZ(-Math.PI / 2);
  else if (p.axis === "z") g.rotateX(Math.PI / 2);
  return place(new THREE.Mesh(g, mat), parent, x, y, z, p);
}

/**
 * Handrail along deck-level points: top rail, knee rail and stanchions, with an optional toe board.
 * Points are where the stanchions stand.
 */
export function rail(
  parent: THREE.Object3D,
  mat: THREE.Material,
  pts: V3[],
  p: { h?: number; r?: number; posts?: number; mid?: boolean; toe?: THREE.Material; closed?: boolean } = {},
) {
  const h = p.h ?? 1.1;
  const r = p.r ?? 0.024;
  const span = p.posts ?? 1.8;
  const list = p.closed ? [...pts, pts[0]!] : pts;
  for (let i = 0; i < list.length - 1; i++) {
    const a = list[i]!;
    const b = list[i + 1]!;
    const top = (q: V3, k: number) => q.clone().add(new THREE.Vector3(0, h * k, 0));
    bar(parent, mat, top(a, 1), top(b, 1), r, 6);
    if (p.mid !== false) bar(parent, mat, top(a, 0.5), top(b, 0.5), r * 0.85, 6);
    const n = Math.max(1, Math.ceil(a.distanceTo(b) / span));
    for (let k = 0; k <= n; k++) {
      if (k === n && i < list.length - 2) continue;
      const s = a.clone().lerp(b, k / n);
      bar(parent, mat, s, top(s, 1), r * 1.05, 6);
    }
    if (p.toe) beam(parent, p.toe, top(a, 0.05), top(b, 0.05), 0.01, 0.1);
  }
}

/** Vertical or raked ladder with two stiles and round rungs. */
export function ladder(parent: THREE.Object3D, mat: THREE.Material, bottom: V3, top: V3, w = 0.5, step = 0.3) {
  const d = top.clone().sub(bottom);
  const len = d.length();
  const side = new THREE.Vector3().crossVectors(d, UP);
  if (side.lengthSq() < 1e-6) side.set(1, 0, 0);
  side.normalize().multiplyScalar(w / 2);
  // stile direction: if vertical, put stiles along x unless told otherwise via the top/bottom offset
  bar(parent, mat, bottom.clone().add(side), top.clone().add(side), 0.028, 6);
  bar(parent, mat, bottom.clone().sub(side), top.clone().sub(side), 0.028, 6);
  const n = Math.floor(len / step);
  for (let i = 1; i <= n; i++) {
    const c = bottom.clone().addScaledVector(d, i / (n + 0.5));
    bar(parent, mat, c.clone().add(side), c.clone().sub(side), 0.018, 6);
  }
}

/**
 * Stairway from bottom to top (both at tread level), treads of steel grating between channel
 * stringers, handrails on the chosen sides. Returns the group in case the caller animates it.
 */
export function stairs(
  parent: THREE.Object3D,
  frame: THREE.Material,
  tread: THREE.Material,
  railMat: THREE.Material,
  bottom: V3,
  top: V3,
  width = 0.8,
  p: { left?: boolean; right?: boolean; rise?: number } = {},
): THREE.Group {
  const g = new THREE.Group();
  g.position.copy(bottom);
  const dx = top.x - bottom.x;
  const dz = top.z - bottom.z;
  const run = Math.hypot(dx, dz);
  const rise = top.y - bottom.y;
  g.rotation.y = Math.atan2(dx, dz);
  parent.add(g);
  const n = Math.max(2, Math.round(rise / (p.rise ?? 0.22)));
  const sl = Math.hypot(run, rise);
  const ang = Math.atan2(rise, run);
  for (const s of [-1, 1]) {
    const st = box(g, frame, 0.06, 0.26, sl + 0.25, (s * width) / 2, rise / 2 - 0.12, run / 2, { rx: -ang, r: 0.01 });
    st.userData.wear = { dust: 0.6 };
  }
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    box(g, tread, width - 0.04, 0.04, Math.max(0.26, run / n + 0.04), 0, rise * t - 0.02, run * t, { r: 0.008 });
  }
  const sides = [p.left !== false ? 1 : 0, p.right !== false ? -1 : 0].filter((s) => s !== 0);
  for (const s of sides) {
    const x = (s * width) / 2 + s * 0.02;
    const a = new THREE.Vector3(x, 0, 0);
    const b = new THREE.Vector3(x, rise, run);
    const posts = Math.max(2, Math.ceil(sl / 1.6));
    for (let i = 0; i <= posts; i++) {
      const q = a.clone().lerp(b, i / posts);
      bar(g, railMat, q, q.clone().setY(q.y + 1.0), 0.024, 6);
    }
    bar(g, railMat, a.clone().setY(1.0), b.clone().setY(rise + 1.0), 0.024, 6);
    bar(g, railMat, a.clone().setY(0.5), b.clone().setY(rise + 0.5), 0.02, 6);
  }
  return g;
}

/** Rectangular lamp: housing with a lens face pointing along the local +z, aimed by yaw/pitch. */
export function lamp(
  parent: THREE.Object3D,
  housing: THREE.Material,
  lens: THREE.Material,
  x: number,
  y: number,
  z: number,
  p: { w?: number; h?: number; d?: number; yaw?: number; pitch?: number; round?: boolean } = {},
): THREE.Group {
  const w = p.w ?? 0.36;
  const h = p.h ?? 0.22;
  const d = p.d ?? 0.16;
  const g = node(parent, x, y, z, { ry: p.yaw ?? 0, rx: p.pitch ?? 0 });
  if (p.round) {
    cyl(g, housing, w / 2, d, 0, 0, -d / 2, { axis: "z", seg: 16 });
    cyl(g, lens, w / 2 - 0.02, 0.02, 0, 0, 0.005, { axis: "z", seg: 16 });
  } else {
    box(g, housing, w, h, d, 0, 0, -d / 2, { r: 0.025 });
    box(g, lens, w - 0.05, h - 0.05, 0.03, 0, 0, 0.005, { r: 0.01 });
  }
  return g;
}

/** Rotating-beacon housing on a short base. */
export function beacon(parent: THREE.Object3D, base: THREE.Material, lens: THREE.Material, x: number, y: number, z: number, s = 1) {
  cyl(parent, base, 0.09 * s, 0.06 * s, x, y + 0.03 * s, z, { seg: 12 });
  cyl(parent, lens, 0.075 * s, 0.16 * s, x, y + 0.14 * s, z, { seg: 12, r2: 0.065 * s });
  sphere(parent, lens, 0.065 * s, x, y + 0.22 * s, z, { seg: 10, sy: 0.6 });
}

/** Rear-view mirror on a tubular arm from `mount` to `head`, glass facing back along -z (rotate by yaw). */
export function mirror(parent: THREE.Object3D, arm: THREE.Material, glass: THREE.Material, mount: V3, head: V3, size = 0.5, yaw = 0) {
  bar(parent, arm, mount, head, 0.03, 6);
  bar(parent, arm, mount.clone().setY(mount.y - 0.35), head.clone().setY(head.y - 0.15), 0.025, 6);
  const g = node(parent, head.x, head.y, head.z, { ry: yaw });
  box(g, arm, size * 0.62, size, 0.09, 0, 0, 0, { r: 0.03 });
  box(g, glass, size * 0.62 - 0.05, size - 0.05, 0.02, 0, 0, -0.05, { r: 0.008 });
}

/** Fire-suppression bottle with valve head, standing upright. */
export function bottle(parent: THREE.Object3D, red: THREE.Material, steel: THREE.Material, x: number, y: number, z: number, h = 0.75, r = 0.11) {
  cyl(parent, red, r, h - r, x, y + (h - r) / 2, z, { seg: 14 });
  sphere(parent, red, r, x, y + h - r, z, { seg: 14, sy: 0.7 });
  cyl(parent, steel, 0.035, 0.12, x, y + h - r * 0.3 + 0.05, z, { seg: 8 });
}

/** Bevelled steel walkway plate with a slight raised edge, top at y. */
export function deck(parent: THREE.Object3D, mat: THREE.Material, w: number, d: number, x: number, y: number, z: number, t = 0.06) {
  return box(parent, mat, w, t, d, x, y - t / 2, z, { r: 0.012 });
}

// ───────────────────────────── tyres and rims ─────────────────────────────

export interface TyreSpec {
  /** Overall diameter, m. */
  od: number;
  /** Section width, m. */
  width: number;
  /** Rim diameter, m. */
  rim: number;
  /** Lugs per side around the circumference. */
  lugs?: number;
  /** Tread depth, m. */
  depth?: number;
  /** "rock" (deep transverse lugs, haul trucks and loaders), "l5" (smooth-ish underground). */
  pattern?: "rock" | "smooth" | "road";
  segments?: number;
}

const tyreCache = new Map<string, THREE.BufferGeometry>();

/**
 * Off-the-road radial tyre around the x axis, centred at the origin: a lathed carcass with a bulged
 * sidewall and a rim-guard rib, and drafted transverse tread lugs that wrap over the shoulder. Cached.
 */
export function tyreGeometry(s: TyreSpec): THREE.BufferGeometry {
  const key = JSON.stringify(s);
  const hit = tyreCache.get(key);
  if (hit) return hit;
  const R = s.od / 2;
  const depth = s.depth ?? Math.max(0.03, s.od * 0.026);
  const hw = s.width / 2;
  const rr = s.rim / 2;
  const crown = R - depth;
  const seg = s.segments ?? 72;
  // carcass profile (axial a, radius r) from inner bead round the tread to outer bead
  const half: [number, number][] = [
    [hw * 0.8, rr - 0.01],
    [hw * 0.84, rr + 0.04],
    [hw * 0.93, rr + 0.07],
    [hw * 0.91, rr + 0.1],
    [hw * 0.97, rr + (crown - rr) * 0.3],
    [hw * 1.0, rr + (crown - rr) * 0.5],
    [hw * 0.985, rr + (crown - rr) * 0.68],
    [hw * 0.94, rr + (crown - rr) * 0.84],
    [hw * 0.86, crown - (crown - rr) * 0.05],
    [hw * 0.72, crown - 0.004],
    [hw * 0.4, crown],
    [0, crown + 0.003],
  ];
  const prof: [number, number][] = [...half.map(([a, r]) => [-a, r] as [number, number]), ...half.slice(0, -1).reverse()];
  const pos: number[] = [];
  const uv: number[] = [];
  const idx: number[] = [];
  const cols = prof.length;
  for (let i = 0; i <= seg; i++) {
    const th = (i / seg) * Math.PI * 2;
    const c = Math.cos(th);
    const sn = Math.sin(th);
    for (let j = 0; j < cols; j++) {
      const [a, r] = prof[j]!;
      pos.push(a, r * c, r * sn);
      uv.push(i / seg, j / (cols - 1));
    }
  }
  for (let i = 0; i < seg; i++) {
    for (let j = 0; j < cols - 1; j++) {
      const a = i * cols + j;
      const b = (i + 1) * cols + j;
      idx.push(a, b, a + 1, b, b + 1, a + 1);
    }
  }
  const carcass = new THREE.BufferGeometry();
  carcass.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  carcass.setAttribute("uv", new THREE.Float32BufferAttribute(uv, 2));
  carcass.setIndex(idx);
  carcass.computeVertexNormals();
  const parts: THREE.BufferGeometry[] = [carcass.toNonIndexed()];
  carcass.dispose();
  if (s.pattern !== "smooth") {
    // lugs: a strip from near the centreline out over the shoulder, drafted, at an angle
    const n = s.lugs ?? Math.round((Math.PI * 2 * R) / (s.od * 0.105));
    const lp: number[] = [];
    const radiusAt = (a: number) => {
      // tread surface radius drops over the shoulder
      const t = Math.max(0, (a - hw * 0.62) / (hw * 0.4));
      return R - t * t * depth * 2.4;
    };
    const baseAt = (a: number) => {
      const t = Math.max(0, (a - hw * 0.62) / (hw * 0.4));
      return crown - 0.01 - t * t * depth * 2.6;
    };
    const pitch = (Math.PI * 2) / n;
    const lugW = s.pattern === "road" ? 0.5 : 0.56; // fraction of pitch that is rubber
    const steps = 5;
    for (let side = -1; side <= 1; side += 2) {
      for (let k = 0; k < n; k++) {
        const th0 = k * pitch + (side > 0 ? pitch / 2 : 0);
        const a0 = s.pattern === "road" ? 0.03 : -hw * 0.06;
        const a1 = hw * 1.0;
        const ring: { a: number; th: number; w: number }[] = [];
        for (let q = 0; q <= steps; q++) {
          const t = q / steps;
          const a = lerp(a0, a1, t);
          // lug centreline sweeps back as it goes outboard (the classic rock-lug chevron)
          const th = th0 + t * pitch * 0.45;
          const w = pitch * lugW * (0.82 + 0.3 * t);
          ring.push({ a: a * side, th, w });
        }
        const P = (a: number, th: number, r: number) => [a, r * Math.cos(th), r * Math.sin(th)] as const;
        const quad = (p0: readonly number[], p1: readonly number[], p2: readonly number[], p3: readonly number[]) => {
          lp.push(...p0, ...p1, ...p2, ...p0, ...p2, ...p3);
        };
        const draft = 0.12;
        for (let q = 0; q < steps; q++) {
          const A = ring[q]!;
          const B = ring[q + 1]!;
          const ra = radiusAt(Math.abs(A.a));
          const rb = radiusAt(Math.abs(B.a));
          const ba = baseAt(Math.abs(A.a));
          const bb = baseAt(Math.abs(B.a));
          const tA0 = P(A.a, A.th - (A.w / 2) * (1 - draft), ra);
          const tA1 = P(A.a, A.th + (A.w / 2) * (1 - draft), ra);
          const tB0 = P(B.a, B.th - (B.w / 2) * (1 - draft), rb);
          const tB1 = P(B.a, B.th + (B.w / 2) * (1 - draft), rb);
          const bA0 = P(A.a, A.th - A.w / 2, ba);
          const bA1 = P(A.a, A.th + A.w / 2, ba);
          const bB0 = P(B.a, B.th - B.w / 2, bb);
          const bB1 = P(B.a, B.th + B.w / 2, bb);
          const flip = side > 0;
          const Q = flip
            ? (p0: readonly number[], p1: readonly number[], p2: readonly number[], p3: readonly number[]) => quad(p0, p3, p2, p1)
            : quad;
          Q(tA0, tB0, tB1, tA1); // top
          Q(bA0, bB0, tB0, tA0); // leading wall
          Q(tA1, tB1, bB1, bA1); // trailing wall
          if (q === 0) Q(bA1, bA0, tA0, tA1); // inboard end
          if (q === steps - 1) Q(tB1, tB0, bB0, bB1); // outboard end (over the shoulder)
        }
      }
    }
    const lugs = new THREE.BufferGeometry();
    lugs.setAttribute("position", new THREE.Float32BufferAttribute(lp, 3));
    lugs.setAttribute("uv", new THREE.Float32BufferAttribute(new Float32Array((lp.length / 3) * 2), 2));
    lugs.computeVertexNormals();
    parts.push(lugs);
  }
  // merge (all non-indexed) and add a wear attribute: dust on the tread and lower sidewall
  let total = 0;
  for (const p of parts) total += p.getAttribute("position").count;
  const P = new Float32Array(total * 3);
  const N = new Float32Array(total * 3);
  const U = new Float32Array(total * 2);
  const W = new Float32Array(total * 3);
  let o = 0;
  for (const p of parts) {
    const pp = p.getAttribute("position");
    const nn = p.getAttribute("normal");
    const uu = p.getAttribute("uv");
    for (let i = 0; i < pp.count; i++) {
      const x = pp.getX(i);
      const y = pp.getY(i);
      const z = pp.getZ(i);
      P.set([x, y, z], (o + i) * 3);
      N.set([nn.getX(i), nn.getY(i), nn.getZ(i)], (o + i) * 3);
      U.set([uu ? uu.getX(i) : 0, uu ? uu.getY(i) : 0], (o + i) * 2);
      const r = Math.hypot(y, z);
      const tread = smoothstep(crown - (crown - rr) * 0.25, R, r);
      const side = Math.abs(x) / hw;
      W.set([0.22 + tread * 0.3, 0.12 + tread * 0.28 + (side > 0.9 ? 0.06 : 0), 0], (o + i) * 3);
    }
    o += pp.count;
    p.dispose();
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(P, 3));
  g.setAttribute("normal", new THREE.BufferAttribute(N, 3));
  g.setAttribute("uv", new THREE.BufferAttribute(U, 2));
  g.setAttribute("wear", new THREE.BufferAttribute(W, 3));
  g.computeBoundingSphere();
  tyreCache.set(key, g);
  return g;
}

/**
 * Wheel rim, disc and hub for an OTR tyre, around the x axis, outer face toward +x. Adds into `parent`
 * (to be baked). `dish` pushes the hub inboard (rear outer duals show a deep dish).
 */
export function rim(
  parent: THREE.Object3D,
  rimMat: THREE.Material,
  hubMat: THREE.Material,
  boltMat: THREE.Material,
  s: { rim: number; width: number; dish?: number; bolts?: number; hubR?: number; cap?: boolean },
) {
  const rr = s.rim / 2;
  const hw = s.width / 2;
  const dish = s.dish ?? 0.15;
  const hubR = s.hubR ?? rr * 0.55;
  // rim barrel: outer flange lip, flange face, then the barrel's inside (the outside is inside the tyre).
  // Lathe height maps to +x; normals follow (dh, -dr), so this order faces the visible sides outward.
  const barrel: [number, number][] = [
    [rr + 0.075, hw * 0.78],
    [rr + 0.075, hw * 0.845],
    [rr + 0.03, hw * 0.88],
    [rr - 0.06, hw * 0.88],
    [rr - 0.075, hw * 0.83],
    [rr - 0.075, -hw * 0.83],
    [rr - 0.06, -hw * 0.88],
    [rr + 0.05, -hw * 0.86],
  ];
  lathe(parent, rimMat, barrel, 0, 0, 0, { axis: "x", seg: 40 });
  // disc: dished cone from the barrel to the hub flange, facing outboard
  const disc: [number, number][] = [
    [rr - 0.07, hw * 0.55],
    [rr * 0.8, hw * 0.55 - dish * 0.35],
    [hubR + 0.08, hw * 0.55 - dish],
    [hubR - 0.01, hw * 0.55 - dish],
  ];
  lathe(parent, rimMat, disc, 0, 0, 0, { axis: "x", seg: 32 });
  // hub (final-drive housing) with a domed cap and bolt circle
  const hubX = hw * 0.55 - dish;
  const hub: [number, number][] = [
    [hubR, 0],
    [hubR, 0.05],
    [hubR * 0.92, 0.09],
    [hubR * 0.85, 0.12],
    [hubR * 0.6, s.cap === false ? 0.12 : 0.22],
    [hubR * 0.3, s.cap === false ? 0.12 : 0.27],
    [0.001, s.cap === false ? 0.12 : 0.28],
  ];
  lathe(parent, hubMat, hub, hubX, 0, 0, { axis: "x", seg: 28 });
  const nb = s.bolts ?? 24;
  for (let i = 0; i < nb; i++) {
    const th = (i / nb) * Math.PI * 2;
    const rb = hubR + 0.035;
    cyl(parent, boltMat, 0.028, 0.06, hubX + 0.02, Math.cos(th) * rb, Math.sin(th) * rb, { axis: "x", seg: 6 });
  }
  // rim clamp lugs round the disc
  const nc = 12;
  for (let i = 0; i < nc; i++) {
    const th = (i / nc) * Math.PI * 2;
    const rc = rr - 0.12;
    box(parent, hubMat, 0.08, 0.12, 0.1, hw * 0.55 - 0.03, Math.cos(th) * rc, Math.sin(th) * rc, { rx: th, r: 0.015 });
  }
}

// ───────────────────────────── crawler tracks ─────────────────────────────

export interface TrackSpec {
  /** Centre distance between sprocket (rear, -z) and idler (front, +z). */
  length: number;
  /** Sprocket and idler radii. */
  sprocketR: number;
  idlerR: number;
  /** Height of the axle centres above the ground. */
  axleY?: number;
  /** Track shoe width (x) and pitch (along the chain). */
  shoeW: number;
  pitch: number;
  /** Shoe thickness. */
  shoeT?: number;
  /** Bottom rollers count. */
  rollers?: number;
  rollerR?: number;
  /** Carrier (top) rollers count. */
  carriers?: number;
  frameMat?: THREE.Material;
  shoeMat?: THREE.Material;
  /** Raise the sprocket (elevated drive) by this much. */
  sprocketRise?: number;
}

export interface Track {
  group: THREE.Group;
  /** Distance the machine has travelled on this track (m); positive moves the bottom run backwards. */
  setDistance(d: number): void;
}

/**
 * A crawler track: shoes running round a sprocket and idler on an InstancedMesh, the roller frame with
 * bottom and carrier rollers, a toothed sprocket and an idler. Local frame: track centred on x = 0,
 * running along z, ground at y = 0.
 */
export function makeTrack(s: TrackSpec): Track {
  const group = new THREE.Group();
  const frameMat = s.frameMat ?? mats.dark();
  const shoeMat = s.shoeMat ?? mats.track();
  const t = s.shoeT ?? s.pitch * 0.22;
  const ay = s.axleY ?? Math.max(s.sprocketR, s.idlerR) + t;
  const half = s.length / 2;
  const sp = new THREE.Vector2(-half, ay + (s.sprocketRise ?? 0)); // (z, y) sprocket centre
  const id = new THREE.Vector2(half, ay);
  const rs = s.sprocketR + t / 2;
  const ri = s.idlerR + t / 2;
  // loop path in (z, y): bottom run on the ground (y = t/2), wrap idler, top run, wrap sprocket
  const path: THREE.Vector2[] = [];
  const arc = (c: THREE.Vector2, r: number, a0: number, a1: number, n: number) => {
    for (let i = 0; i <= n; i++) {
      const a = lerp(a0, a1, i / n);
      path.push(new THREE.Vector2(c.x + Math.cos(a) * r, c.y + Math.sin(a) * r));
    }
  };
  const groundY = t / 2;
  // bottom run from below the sprocket to below the idler, approaching tangentially
  const bz0 = sp.x + rs * 0.35;
  const bz1 = id.x - ri * 0.35;
  path.push(new THREE.Vector2(bz0, groundY));
  path.push(new THREE.Vector2(bz1, groundY));
  arc(id, ri, -Math.PI / 2 + 0.35, Math.PI / 2, 14);
  // top run with a little sag
  const topA = new THREE.Vector2(id.x, id.y + ri);
  const topB = new THREE.Vector2(sp.x, sp.y + rs);
  for (let i = 1; i < 10; i++) {
    const q = topA.clone().lerp(topB, i / 10);
    q.y -= Math.sin((i / 10) * Math.PI) * 0.06 * s.length * 0.05;
    path.push(q);
  }
  arc(sp, rs, Math.PI / 2, Math.PI * 1.5 - 0.35, 14);
  path.push(new THREE.Vector2(bz0, groundY));
  const cum = [0];
  for (let i = 1; i < path.length; i++) cum.push(cum[i - 1]! + path[i]!.distanceTo(path[i - 1]!));
  const total = cum[cum.length - 1]!;
  const nShoes = Math.round(total / s.pitch);
  const pitch = total / nShoes;
  // shoe: plate with a grouser bar and two link lugs
  const shoeParts = new THREE.Group();
  box(shoeParts, shoeMat, s.shoeW, t * 0.45, pitch * 0.94, 0, 0, 0, { r: 0.015 });
  box(shoeParts, shoeMat, s.shoeW * 0.98, t * 0.5, pitch * 0.16, 0, -t * 0.42, pitch * 0.1, { r: 0.01 });
  for (const sx of [-0.18, 0.18]) box(shoeParts, shoeMat, s.shoeW * 0.12, t * 0.6, pitch * 0.9, sx * s.shoeW, t * 0.45, 0, { r: 0.01 });
  const shoeGeo = bakeFree(shoeParts, { mudTop: 10, mudBottom: 0, mud: 0.8 }).children[0] as THREE.Mesh;
  const shoes = new THREE.InstancedMesh(shoeGeo.geometry, shoeMat, nShoes);
  shoes.castShadow = true;
  shoes.receiveShadow = true;
  shoes.frustumCulled = false;
  group.add(shoes);
  const m4 = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const e = new THREE.Euler();
  const one = new THREE.Vector3(1, 1, 1);
  const pv = new THREE.Vector3();
  const sample = (d: number) => {
    const s0 = ((d % total) + total) % total;
    let lo = 0;
    let hi = cum.length - 1;
    while (hi - lo > 1) {
      const mid = (lo + hi) >> 1;
      if (cum[mid]! <= s0) lo = mid;
      else hi = mid;
    }
    const a = path[lo]!;
    const b = path[hi]!;
    const seg = cum[hi]! - cum[lo]!;
    const f = seg > 0 ? (s0 - cum[lo]!) / seg : 0;
    return { z: lerp(a.x, b.x, f), y: lerp(a.y, b.y, f), ang: Math.atan2(b.y - a.y, b.x - a.x) };
  };
  // static parts: roller frame, rollers, idler, sprocket
  const st = new THREE.Group();
  const frameH = Math.min(s.idlerR * 1.3, ay * 1.1);
  box(st, frameMat, s.shoeW * 0.55, frameH, s.length - s.idlerR * 0.9, 0, ay - frameH * 0.12, -(s.sprocketR - s.idlerR) * 0.4, { r: 0.05 });
  const nr = s.rollers ?? Math.max(4, Math.round(s.length / (s.sprocketR * 1.1)));
  const rR = s.rollerR ?? Math.max(0.12, t * 1.2);
  for (let i = 0; i < nr; i++) {
    const z = lerp(bz0 + rR, bz1 - rR, i / (nr - 1));
    for (const sx of [-0.2, 0.2]) cyl(st, mats.steel(), rR, s.shoeW * 0.16, sx * s.shoeW, t + rR, z, { axis: "x", seg: 14 });
  }
  for (let i = 0; i < (s.carriers ?? 2); i++) {
    const z = lerp(sp.x + rs * 1.5, id.x - ri * 1.5, (i + 1) / ((s.carriers ?? 2) + 1));
    const y = lerp(sp.y + rs, id.y + ri, (z - sp.x) / (id.x - sp.x)) - t / 2 - rR * 0.8;
    cyl(st, mats.steel(), rR * 0.8, s.shoeW * 0.3, 0, y, z, { axis: "x", seg: 14 });
  }
  const baked = bakeFree(st, { mudTop: ay * 1.6, mudBottom: 0, mud: 1 });
  group.add(baked);
  // idler and sprocket wheels rotate
  const idler = new THREE.Group();
  idler.position.set(0, id.y, id.x);
  const ig = new THREE.Group();
  cyl(ig, mats.steel(), s.idlerR * 0.98, s.shoeW * 0.28, 0, 0, 0, { axis: "x", seg: 28 });
  cyl(ig, frameMat, s.idlerR * 0.55, s.shoeW * 0.36, 0, 0, 0, { axis: "x", seg: 20 });
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2;
    cyl(ig, frameMat, s.idlerR * 0.12, s.shoeW * 0.3, 0, Math.cos(a) * s.idlerR * 0.7, Math.sin(a) * s.idlerR * 0.7, { axis: "x", seg: 8 });
  }
  idler.add(bakeFree(ig, { mudTop: s.idlerR, mudBottom: -s.idlerR, mud: 0.8 }));
  group.add(idler);
  const sprocket = new THREE.Group();
  sprocket.position.set(0, sp.y, sp.x);
  const sg = new THREE.Group();
  cyl(sg, frameMat, s.sprocketR * 0.8, s.shoeW * 0.3, 0, 0, 0, { axis: "x", seg: 28 });
  cyl(sg, mats.steel(), s.sprocketR * 0.35, s.shoeW * 0.5, 0, 0, 0, { axis: "x", seg: 16 });
  const teeth = Math.max(9, Math.round((Math.PI * 2 * s.sprocketR) / pitch));
  for (let i = 0; i < teeth; i++) {
    const a = (i / teeth) * Math.PI * 2;
    box(sg, mats.steel(), s.shoeW * 0.2, s.sprocketR * 0.26, pitch * 0.38, 0, Math.cos(a) * s.sprocketR * 0.92, Math.sin(a) * s.sprocketR * 0.92, {
      rx: -a,
      r: 0.02,
    });
  }
  sprocket.add(bakeFree(sg, { mudTop: s.sprocketR, mudBottom: -s.sprocketR, mud: 0.7 }));
  group.add(sprocket);
  const setDistance = (d: number) => {
    for (let i = 0; i < nShoes; i++) {
      const p = sample(i * pitch - d);
      e.set(-p.ang, 0, 0);
      q.setFromEuler(e);
      pv.set(0, p.y, p.z);
      m4.compose(pv, q, one);
      shoes.setMatrixAt(i, m4);
    }
    shoes.instanceMatrix.needsUpdate = true;
    idler.rotation.x = -d / s.idlerR;
    sprocket.rotation.x = -d / s.sprocketR;
  };
  // A shoe's local +y is the loop's inside (tangent +z on the bottom run), so its grouser at -y faces out.
  setDistance(0);
  return { group, setDistance };
}

// ───────────────────────────── rocks ─────────────────────────────

const rockCache = new Map<string, THREE.BufferGeometry>();

/**
 * An angular blasted rock: a noisy icosahedron cut by a few random fracture planes, flat-shaded.
 * Unit size (about 1 m across before scaling). Cached per (seed, detail).
 */
export function rockGeometry(seed: number, detail = 2): THREE.BufferGeometry {
  const key = `${seed}:${detail}`;
  const hit = rockCache.get(key);
  if (hit) return hit;
  const r = rng(seed * 7919 + 13);
  const n = new Noise3(seed + 101);
  const g = new THREE.IcosahedronGeometry(0.5, detail);
  const P = g.getAttribute("position");
  const sx = 0.8 + r() * 0.45;
  const sy = 0.55 + r() * 0.35;
  const sz = 0.75 + r() * 0.45;
  const planes: { n: V3; d: number }[] = [];
  const np = 3 + Math.floor(r() * 4);
  for (let i = 0; i < np; i++) {
    const v = new THREE.Vector3(r() * 2 - 1, r() * 2 - 1, r() * 2 - 1).normalize();
    planes.push({ n: v, d: 0.3 + r() * 0.14 });
  }
  const p = new THREE.Vector3();
  for (let i = 0; i < P.count; i++) {
    p.fromBufferAttribute(P, i);
    const k = 1 + 0.32 * n.noise(p.x * 2.1, p.y * 2.1, p.z * 2.1) + 0.1 * n.noise(p.x * 6, p.y * 6, p.z * 6);
    p.multiplyScalar(k);
    for (const pl of planes) {
      const dd = p.dot(pl.n);
      if (dd > pl.d) p.addScaledVector(pl.n, pl.d - dd);
    }
    p.set(p.x * sx, p.y * sy, p.z * sz);
    P.setXYZ(i, p.x, p.y, p.z);
  }
  g.computeVertexNormals();
  const N = g.getAttribute("normal");
  const W = new Float32Array(P.count * 3);
  for (let i = 0; i < P.count; i++) {
    W[i * 3] = 0.05;
    W[i * 3 + 1] = smoothstep(0.1, 0.9, N.getY(i)) * 0.8 + 0.1;
    W[i * 3 + 2] = N.getY(i) < -0.3 ? 0.25 : 0;
  }
  g.setAttribute("wear", new THREE.BufferAttribute(W, 3));
  g.computeBoundingSphere();
  rockCache.set(key, g);
  return g;
}

/** Rock material: matte, mottled, dusty on top; colours come per instance. */
export function rockMaterial(colour = "#ffffff", dust: string = SITE.dust) {
  return cached(`rock:${colour}:${dust}`, () =>
    grime(new THREE.MeshStandardMaterial({ color: colour, roughness: 0.94, metalness: 0, flatShading: false }), {
      dust,
      mudAmount: 0,
      dustAmount: 0.8,
      scale: 0.9,
      variation: 1,
    }),
  );
}

/** Ore and waste palettes (sRGB). Each rock picks one and jitters it. */
export const ROCK_PALETTES = {
  ore: ["#6d6358", "#7b6f62", "#5a524b", "#857a6b", "#6a5e50", "#4f4842"],
  oxide: ["#8a5a3c", "#9b6b48", "#7a4c34", "#a8805c", "#6e4630", "#8d6a52"],
  waste: ["#8e877c", "#9d968a", "#777168", "#aaa397", "#837c71", "#6b665e"],
  coal: ["#232220", "#2c2a27", "#1a1918", "#34312d", "#282624"],
  granite: ["#a39e98", "#8f8a84", "#b5afa6", "#7d7873", "#9a928a"],
} as const;
export type RockPalette = keyof typeof ROCK_PALETTES;

export interface RockSet {
  group: THREE.Group;
  meshes: THREE.InstancedMesh[];
  /** Number of rocks placed. */
  count: number;
}

/**
 * Instanced boulders on a few rock shapes. `place(i, r)` returns position, size (m across) and an
 * optional yaw; rocks get random tumbles and palette colours. One draw call per shape (`shapes`).
 */
export function makeRocks(o: {
  count: number;
  seed?: number;
  shapes?: number;
  detail?: number;
  palette?: RockPalette | string[];
  place: (i: number, r: () => number) => { x: number; y: number; z: number; size: number; flat?: number };
}): RockSet {
  const seed = o.seed ?? 1;
  const r = rng(seed);
  const nShapes = o.shapes ?? 4;
  const pal = typeof o.palette === "string" || o.palette === undefined ? ROCK_PALETTES[(o.palette as RockPalette | undefined) ?? "ore"] : o.palette;
  const geos = Array.from({ length: nShapes }, (_, k) => rockGeometry(((seed * 31 + k) % 64) + 1, o.detail ?? 1));
  const buckets: number[][] = geos.map(() => []);
  for (let i = 0; i < o.count; i++) buckets[Math.floor(r() * nShapes) % nShapes]!.push(i);
  const mat = rockMaterial();
  const group = new THREE.Group();
  const meshes: THREE.InstancedMesh[] = [];
  const m4 = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const e = new THREE.Euler();
  const s = new THREE.Vector3();
  const pv = new THREE.Vector3();
  const c = new THREE.Color();
  buckets.forEach((ids, k) => {
    if (!ids.length) return;
    const im = new THREE.InstancedMesh(geos[k]!, mat, ids.length);
    ids.forEach((id, j) => {
      const p = o.place(id, r);
      e.set((r() - 0.5) * 0.8, r() * Math.PI * 2, (r() - 0.5) * 0.8);
      q.setFromEuler(e);
      const f = p.flat ?? 1;
      s.set(p.size * (0.9 + r() * 0.2), p.size * f * (0.85 + r() * 0.3), p.size * (0.9 + r() * 0.2));
      pv.set(p.x, p.y, p.z);
      m4.compose(pv, q, s);
      im.setMatrixAt(j, m4);
      c.set(pal[Math.floor(r() * pal.length)]!);
      c.offsetHSL((r() - 0.5) * 0.02, (r() - 0.5) * 0.06, (r() - 0.5) * 0.08);
      im.setColorAt(j, c);
    });
    im.castShadow = true;
    im.receiveShadow = true;
    im.instanceMatrix.needsUpdate = true;
    if (im.instanceColor) im.instanceColor.needsUpdate = true;
    im.computeBoundingSphere();
    meshes.push(im);
    group.add(im);
  });
  return { group, meshes, count: o.count };
}

/**
 * Material for heaped broken rock surfaces (stockpiles, muck piles, payloads): vertex-coloured, with a
 * pebble normal map in world-planar UVs so the surface reads granular up close.
 */
export function heapMaterial(key = "heap") {
  return cached(`heapmat:${key}`, () => {
    const m = new THREE.MeshStandardMaterial({
      color: "#ffffff",
      vertexColors: true,
      roughness: 0.97,
      metalness: 0,
      normalMap: gravelNormal(),
      normalScale: new THREE.Vector2(1.1, 1.1),
    });
    return grime(m, { mudAmount: 0, dustAmount: 0.7, scale: 0.12, variation: 1 });
  });
}

/** Writes vertex colours and planar uvs (period `uvScale` m) on a heap surface from a palette. */
export function colourHeap(g: THREE.BufferGeometry, palette: readonly string[], seed: number, uvScale = 1.6) {
  const n = new Noise3(seed);
  const P = g.getAttribute("position");
  const N = g.getAttribute("normal");
  const col = new Float32Array(P.count * 3);
  const uv = new Float32Array(P.count * 2);
  const W = new Float32Array(P.count * 3);
  const cs = palette.map((p) => new THREE.Color(p));
  const c = new THREE.Color();
  for (let i = 0; i < P.count; i++) {
    const x = P.getX(i);
    const y = P.getY(i);
    const z = P.getZ(i);
    const t = clamp01(0.5 + 0.8 * n.fbm(x * 0.08, y * 0.08, z * 0.08, 3));
    const k = Math.min(cs.length - 1, Math.floor(t * cs.length));
    const t2 = clamp01(0.5 + 0.7 * n.noise(x * 0.45 + 9, y * 0.45, z * 0.45));
    c.copy(cs[k]!).lerp(cs[(k + 1) % cs.length]!, t2 * 0.6);
    col[i * 3] = c.r;
    col[i * 3 + 1] = c.g;
    col[i * 3 + 2] = c.b;
    uv[i * 2] = x / uvScale;
    uv[i * 2 + 1] = z / uvScale;
    const ny = N ? N.getY(i) : 1;
    W[i * 3] = 0;
    W[i * 3 + 1] = 0.15 + smoothstep(0.6, 1, ny) * 0.5;
    W[i * 3 + 2] = 0;
  }
  g.setAttribute("color", new THREE.BufferAttribute(col, 3));
  g.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  g.setAttribute("wear", new THREE.BufferAttribute(W, 3));
}
