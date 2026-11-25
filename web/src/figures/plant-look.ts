/**
 * The look of the plant scene: weathered finishes (painted steel that rusts at its edges and streaks below its
 * seams, galvanised steel, tank coatings stained by slurry), a bake that merges a part tree by material across
 * several frames so mounting never stalls the page, a soft glow sprite for hot metal and lamps, and the desert
 * scrub round the pad. No DOM access at import.
 */
import * as THREE from "three";
import { grime, noiseTexture, type PartWear, type Wear } from "./fleet-kit";
import { surfaceTime } from "./plant-kit";

/* ---------- Finishes ---------- */

export interface Finish {
  rough?: number;
  coat?: number;
  metal?: number;
  /** Rust blooms at edges and low points, 0..1. */
  rust?: number;
  /** Vertical streaks below seams and bolts on vertical faces, 0..1. */
  streak?: number;
  /** Colour of the streaks (rust by default; slurry grey on tanks). */
  streakColour?: string;
  /** Dust and mud multipliers, and blotch scale (cycles per metre). */
  dust?: number;
  mud?: number;
  scale?: number;
  variation?: number;
  /** A normal map and its scale (cladding ribs, block courses). */
  normalMap?: THREE.Texture;
  normalScale?: number;
  map?: THREE.Texture;
  transparent?: boolean;
}

const finishCache = new Map<string, THREE.MeshPhysicalMaterial>();

/**
 * A weathered finish: industrial paint (or bare metal) with the fleet's mud and dust, plus rust and streaks that
 * hang below seams on vertical faces. Cached by colour and options; each call with a new key is its own material.
 */
export function finish(colour: string, o: Finish = {}): THREE.MeshPhysicalMaterial {
  const key = `${colour}|${o.rough ?? ""}|${o.coat ?? ""}|${o.metal ?? ""}|${o.rust ?? ""}|${o.streak ?? ""}|${o.streakColour ?? ""}|${o.dust ?? ""}|${o.mud ?? ""}|${o.scale ?? ""}|${o.variation ?? ""}|${o.normalMap?.uuid ?? ""}|${o.map?.uuid ?? ""}|${o.transparent ?? ""}`;
  const hit = finishCache.get(key);
  if (hit) return hit;
  const m = new THREE.MeshPhysicalMaterial({
    color: colour,
    roughness: o.rough ?? 0.55,
    metalness: o.metal ?? 0,
    clearcoat: o.coat ?? 0.2,
    clearcoatRoughness: 0.4,
    normalMap: o.normalMap ?? null,
    normalScale: new THREE.Vector2(o.normalScale ?? 1, o.normalScale ?? 1),
    map: o.map ?? null,
    transparent: o.transparent ?? false,
  });
  grime(m, { dustAmount: o.dust ?? 1, mudAmount: o.mud ?? 1, scale: o.scale ?? 0.12, variation: o.variation ?? 1 });
  const base = m.onBeforeCompile;
  const u = {
    wAmt: { value: new THREE.Vector4(o.rust ?? 0.25, o.streak ?? 0.35, 0.22, 0) },
    wRustC: { value: new THREE.Color("#7b4a2b") },
    wStreakC: { value: new THREE.Color(o.streakColour ?? "#6a4a33") },
  };
  m.onBeforeCompile = (sh, r) => {
    base.call(m, sh, r);
    Object.assign(sh.uniforms, u);
    sh.fragmentShader = sh.fragmentShader
      .replace("uniform vec4 gAmt;", "uniform vec4 gAmt;\nuniform vec4 wAmt; uniform vec3 wRustC; uniform vec3 wStreakC;")
      .replace(
        "float gDirt = max(gMud, gDust);",
        `float gDirt = max(gMud, gDust);
{
  // Streaks: thin vertical runs below seams, in columns that come and go along the face.
  float hz = (vGPos.x * 0.83 + vGPos.z * 0.61);
  float sN = texture2D(gNoise, vec2(hz * wAmt.z, vGPos.y * 0.03)).a;
  float sC = texture2D(gNoise, vec2(hz * wAmt.z * 0.17, 0.37)).r;
  float sk = smoothstep(0.56, 0.84, sN) * smoothstep(0.38, 0.78, sC) * gVert * wAmt.y;
  // Rust: blooms where the paint has failed, more on edges the fleet's cavity mask darkens and low down.
  float rk = smoothstep(0.66, 0.86, gB.g * 0.55 + gA.r * 0.45 + vWear.z * 0.6 + vWear.x * 0.25) * wAmt.x;
  diffuseColor.rgb = mix(diffuseColor.rgb, wStreakC * (0.75 + 0.4 * gB.b), clamp(sk, 0.0, 0.7));
  diffuseColor.rgb = mix(diffuseColor.rgb, wRustC * (0.65 + 0.55 * gB.b), clamp(rk, 0.0, 0.8));
  gDirt = max(gDirt, rk * 0.8);
}`,
      );
  };
  m.customProgramCacheKey = () => `pl-finish-v1-${o.normalMap ? 1 : 0}${o.map ? 1 : 0}`;
  m.name = `finish:${key}`;
  finishCache.set(key, m);
  return m;
}

/* ---------- Procedural textures ---------- */

function dataTex(d: Uint8Array, w: number, h: number, srgb = false) {
  const t = new THREE.DataTexture(d, w, h, THREE.RGBAFormat);
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.minFilter = THREE.LinearMipmapLinearFilter;
  t.magFilter = THREE.LinearFilter;
  t.generateMipmaps = true;
  t.anisotropy = 4;
  if (srgb) t.colorSpace = THREE.SRGBColorSpace;
  t.needsUpdate = true;
  return t;
}

let blockN: THREE.DataTexture | null = null;
/** Concrete block courses (400 × 200 mm, running bond) as a normal map; one tile is 2 m by 1 m (set repeat 0.5, 1). */
export function blockNormal() {
  if (blockN) return blockN;
  const W = 256;
  const H = 128;
  const h = new Float32Array(W * H);
  for (let y = 0; y < H; y++)
    for (let x = 0; x < W; x++) {
      const course = Math.floor((y / H) * 5);
      const off = course % 2 ? 0.5 : 0;
      const u = ((x / W) * 5 + off) % 1;
      const v = ((y / H) * 5) % 1;
      const joint = Math.min(u, 1 - u) * 5 < 0.06 || Math.min(v, 1 - v) * 5 < 0.12 ? 0 : 1;
      h[y * W + x] = joint;
    }
  const d = new Uint8Array(W * H * 4);
  for (let y = 0; y < H; y++)
    for (let x = 0; x < W; x++) {
      const hx = h[y * W + ((x + 1) % W)]! - h[y * W + ((x - 1 + W) % W)]!;
      const hy = h[((y + 1) % H) * W + x]! - h[((y - 1 + H) % H) * W + x]!;
      const nx = -hx * 0.9;
      const ny = -hy * 0.9;
      const l = Math.hypot(nx, ny, 1);
      const k = (y * W + x) * 4;
      d[k] = Math.round((nx / l) * 127.5 + 127.5);
      d[k + 1] = Math.round((ny / l) * 127.5 + 127.5);
      d[k + 2] = Math.round((1 / l) * 127.5 + 127.5);
      d[k + 3] = 255;
    }
  blockN = dataTex(d, W, H);
  blockN.repeat.set(0.5, 1);
  return blockN;
}

let glowT: THREE.DataTexture | null = null;
/** A soft round glow, white in the centre, for additive sprites. */
export function glowTexture() {
  if (glowT) return glowT;
  const S = 64;
  const d = new Uint8Array(S * S * 4);
  for (let y = 0; y < S; y++)
    for (let x = 0; x < S; x++) {
      const r = Math.hypot(x + 0.5 - S / 2, y + 0.5 - S / 2) / (S / 2);
      const a = Math.max(0, 1 - r) ** 2.2;
      const k = (y * S + x) * 4;
      d[k] = d[k + 1] = d[k + 2] = 255;
      d[k + 3] = Math.round(a * 255);
    }
  glowT = new THREE.DataTexture(d, S, S, THREE.RGBAFormat);
  glowT.magFilter = THREE.LinearFilter;
  glowT.minFilter = THREE.LinearFilter;
  glowT.needsUpdate = true;
  return glowT;
}

/** An additive glow sprite of a colour and size in metres. */
export function glow(colour: string, size: number, opacity = 1) {
  const m = new THREE.SpriteMaterial({ map: glowTexture(), color: colour, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending, opacity, fog: false });
  const s = new THREE.Sprite(m);
  s.scale.set(size, size, 1);
  s.renderOrder = 6;
  return s;
}

/* ---------- Bake across frames ---------- */

const nextFrame = () => new Promise<void>((r) => (typeof requestAnimationFrame === "function" ? requestAnimationFrame(() => r()) : setTimeout(r, 0)));
const ss = (a: number, b: number, v: number) => {
  const t = Math.min(1, Math.max(0, (v - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

/**
 * Merges every mesh under `root` into one mesh per material, like the fleet kit's bake (same `wear` attribute:
 * mud from the ground up, dust on tops, cavities darkened), but yields to the browser whenever a slice of work
 * passes `budget` milliseconds.
 */
export async function bakeAsync(root: THREE.Object3D, wear: Wear = {}, budget = 12): Promise<THREE.Group> {
  const mudTop = wear.mudTop ?? 3;
  const mudBottom = wear.mudBottom ?? 0.3;
  const mudK = wear.mud ?? 1;
  const dustK = wear.dust ?? 1;
  const under = wear.under ?? 0.22;
  root.updateMatrixWorld(true);
  const inv = root.matrixWorld.clone().invert();
  const groups = new Map<THREE.Material, { geo: THREE.BufferGeometry; m: THREE.Matrix4; w: PartWear; shadow: boolean }[]>();
  root.traverse((o) => {
    const mesh = o as THREE.Mesh;
    if (!mesh.isMesh || (o as THREE.InstancedMesh).isInstancedMesh) return;
    const mat = mesh.material as THREE.Material;
    const list = groups.get(mat) ?? [];
    list.push({ geo: mesh.geometry, m: new THREE.Matrix4().multiplyMatrices(inv, mesh.matrixWorld), w: (mesh.userData.wear as PartWear | undefined) ?? {}, shadow: mesh.userData.noShadow !== true });
    groups.set(mat, list);
  });
  const out = new THREE.Group();
  const p = new THREE.Vector3();
  const q = new THREE.Vector3();
  const nm = new THREE.Matrix3();
  let t0 = performance.now();
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
      nm.getNormalMatrix(m);
      const flip = m.determinant() < 0;
      for (let i = 0; i < P.count; i++) {
        p.fromBufferAttribute(P, i).applyMatrix4(m);
        const k3 = (vo + i) * 3;
        pos[k3] = p.x;
        pos[k3 + 1] = p.y;
        pos[k3 + 2] = p.z;
        q.fromBufferAttribute(N, i).applyMatrix3(nm).normalize();
        nrm[k3] = q.x;
        nrm[k3 + 1] = q.y;
        nrm[k3 + 2] = q.z;
        if (U) {
          uv[(vo + i) * 2] = U.getX(i);
          uv[(vo + i) * 2 + 1] = U.getY(i);
        }
        const mud = ss(mudTop, mudBottom, p.y) * mudK + (q.y < -0.4 ? 0.25 * mudK : 0);
        const dust = ss(0.35, 0.92, q.y) * dustK * 0.85 + 0.04 * dustK;
        const ao = (q.y < -0.3 ? under : 0) + ss(1.2, 0, p.y) * 0.12;
        wr[k3] = Math.min(1, Math.max(0, mud * (w.mud ?? 1)));
        wr[k3 + 1] = Math.min(1, Math.max(0, dust * (w.dust ?? 1)));
        wr[k3 + 2] = Math.min(0.6, ao + (w.ao ?? 0));
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
      geo.dispose();
      if (performance.now() - t0 > budget) {
        await nextFrame();
        t0 = performance.now();
      }
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

/* ---------- Desert scrub ---------- */

/**
 * Low saltbush and spinifex clumps scattered over the ground, thinning near roads and the pad: one instanced mesh.
 * `keep(x, z)` returns 0 where nothing may grow and up to 1 where scrub is thickest.
 */
export function makeScrub(o: { n: number; centre: [number, number]; radius: number; inner?: number; seed: number; height: (x: number, z: number) => number; keep: (x: number, z: number) => number }) {
  const geo = new THREE.IcosahedronGeometry(1, 1);
  // Flatten and roughen the clump.
  const pos = geo.getAttribute("position") as THREE.BufferAttribute;
  for (let i = 0; i < pos.count; i++) {
    const y = pos.getY(i);
    const n = 0.82 + 0.3 * Math.sin(pos.getX(i) * 7.1 + pos.getZ(i) * 5.3);
    pos.setXYZ(i, pos.getX(i) * n, Math.max(-0.15, y) * 0.62, pos.getZ(i) * n);
  }
  geo.computeVertexNormals();
  const mat = new THREE.MeshStandardMaterial({ color: "#ffffff", roughness: 0.95, metalness: 0 });
  const mesh = new THREE.InstancedMesh(geo, mat, o.n);
  let s = o.seed >>> 0 || 1;
  const r = () => {
    s = (s + 0x6d2b79f5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const cols = ["#6f6a45", "#7d7650", "#5e5b3c", "#8a7f5a", "#6b6f4c", "#9b8d66"].map((c) => new THREE.Color(c));
  const m = new THREE.Matrix4();
  const qn = new THREE.Quaternion();
  const sc = new THREE.Vector3();
  const at = new THREE.Vector3();
  let k = 0;
  let tries = 0;
  while (k < o.n && tries < o.n * 8) {
    tries++;
    const a = r() * Math.PI * 2;
    const rr = (o.inner ?? 0) + Math.sqrt(r()) * (o.radius - (o.inner ?? 0));
    const x = o.centre[0] + Math.cos(a) * rr;
    const z = o.centre[1] + Math.sin(a) * rr;
    const keep = o.keep(x, z);
    if (keep <= 0 || r() > keep) continue;
    const size = (0.35 + r() * r() * 1.1) * (rr > 600 ? 1.6 : 1);
    at.set(x, o.height(x, z) - 0.08 * size, z);
    qn.setFromAxisAngle(new THREE.Vector3(0, 1, 0), r() * 6.28);
    sc.set(size * (0.8 + r() * 0.5), size * (0.55 + r() * 0.5), size * (0.8 + r() * 0.5));
    m.compose(at, qn, sc);
    mesh.setMatrixAt(k, m);
    mesh.setColorAt(k, cols[Math.floor(r() * cols.length)]!.clone().multiplyScalar(0.85 + r() * 0.3));
    k++;
  }
  mesh.count = k;
  mesh.instanceMatrix.needsUpdate = true;
  if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  mesh.castShadow = true;
  mesh.receiveShadow = true;
  mesh.frustumCulled = false;
  return mesh;
}

/**
 * Mineral froth seen from a walkway or from a hundred metres: bubble cells that move towards the lip (or down a
 * spill), lit with a wet sulphide sheen, and fading to their average tone once a bubble is smaller than a pixel, so
 * a distant cell reads as froth rather than gravel. Local (x, z) of the mesh are metres from the cell axis.
 */
const frothCache = new Map<string, THREE.MeshStandardMaterial>();
export function froth(o: { colour: string; speed?: number; spill?: boolean; size?: number }) {
  const key = `${o.colour}|${o.speed ?? 0.12}|${o.spill ? 1 : 0}|${o.size ?? 9}`;
  const hit = frothCache.get(key);
  if (hit) return hit;
  const uniforms = { uT: surfaceTime, uV: { value: o.speed ?? 0.12 }, uSpill: { value: o.spill ? 1 : 0 }, uSize: { value: o.size ?? 9 } };
  const m = new THREE.MeshStandardMaterial({ color: o.colour, roughness: 0.3, metalness: 0.1 });
  m.customProgramCacheKey = () => `pl-froth-v2-${o.spill ? 1 : 0}`;
  m.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, uniforms);
    sh.vertexShader = sh.vertexShader.replace("#include <common>", "#include <common>\nvarying vec3 vFL;").replace("#include <begin_vertex>", "#include <begin_vertex>\nvFL = position;");
    sh.fragmentShader = sh.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
uniform float uT; uniform float uV; uniform float uSpill; uniform float uSize;
varying vec3 vFL;
float gH; float gEdge;
vec2 frHash(vec2 p) { p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3))); return fract(sin(p) * 43758.5453); }
vec3 frCells(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p);
  float d1 = 8.0; float d2 = 8.0; float id = 0.0;
  for (int y = -1; y <= 1; y++) for (int x = -1; x <= 1; x++) {
    vec2 g = vec2(float(x), float(y));
    vec2 h = frHash(i + g);
    vec2 o = g + 0.5 + 0.4 * sin(6.2831 * h + uT * 0.3);
    float d = length(o - f);
    if (d < d1) { d2 = d1; d1 = d; id = h.x; } else if (d < d2) d2 = d;
  }
  return vec3(d1, d2, id);
}
vec3 frBump(vec3 pos, vec3 n, float h, float k) {
  vec3 sx = dFdx(pos); vec3 sy = dFdy(pos);
  vec3 r1 = cross(sy, n); vec3 r2 = cross(n, sx);
  float det = dot(sx, r1);
  vec2 dh = vec2(dFdx(h), dFdy(h)) * k;
  return normalize(abs(det) * n - sign(det) * (dh.x * r1 + dh.y * r2));
}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec2 q;
float r = length(vFL.xz);
float a = atan(vFL.z, vFL.x);
if (uSpill > 0.5) q = vec2(a * max(r, 0.4) * uSize, vFL.y * uSize + uT * 1.2);
else q = vec2(a * max(r, 0.4) * uSize, (r - uT * uV) * uSize);
// Bubbles smaller than about a pixel fade to their mean tone.
float lod = clamp(1.6 - length(fwidth(q)) * 2.2, 0.0, 1.0);
vec3 c1 = frCells(q);
vec3 c2 = frCells(q * 2.4 + 7.1);
float big = smoothstep(0.0, 0.45, c1.y - c1.x);
float small = smoothstep(0.0, 0.3, c2.y - c2.x);
gEdge = (1.0 - min(big, mix(1.0, small, 0.5))) * lod;
gH = mix(0.55, (1.0 - c1.x * c1.x) * 0.7 + (1.0 - c2.x * c2.x) * 0.3, lod);
float tone = mix(1.0, 0.86 + 0.24 * c1.z, lod);
// Loaded, darker froth in broad patches; cleaner, paler froth near the lip.
float patchy = 0.9 + 0.1 * sin(vFL.x * 1.7 + uT * 0.05) * sin(vFL.z * 1.3);
diffuseColor.rgb *= tone * patchy * mix(1.0, 0.55, gEdge);
diffuseColor.rgb = mix(diffuseColor.rgb, vec3(0.8, 0.74, 0.55), smoothstep(0.8, 0.98, gH) * 0.3 * lod);`,
      )
      .replace("#include <roughnessmap_fragment>", "#include <roughnessmap_fragment>\nroughnessFactor = mix(0.22, 0.7, gEdge);")
      .replace("#include <normal_fragment_maps>", "#include <normal_fragment_maps>\nnormal = frBump(-vViewPosition, normal, gH, 0.06 * lod);");
  };
  frothCache.set(key, m);
  return m;
}

/**
 * Compile the scene's shaders a few at a time, yielding a frame whenever a slice passes `budget` ms, then wait for
 * the driver to finish linking them all, so the first rendered frame does not stall the page.
 */
export async function compileGradually(renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.Camera, budget = 20) {
  const objs: THREE.Object3D[] = [];
  scene.traverse((o) => {
    const m = o as THREE.Mesh & { isPoints?: boolean; isSprite?: boolean; isLine?: boolean };
    if (m.isMesh || m.isPoints || m.isSprite || m.isLine) objs.push(o);
  });
  let t0 = performance.now();
  for (const o of objs) {
    renderer.compile(o, camera, scene);
    if (performance.now() - t0 > budget) {
      await new Promise<void>((r) => requestAnimationFrame(() => r()));
      t0 = performance.now();
    }
  }
  await renderer.compileAsync(scene, camera).catch(() => undefined);
}
