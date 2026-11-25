/**
 * Effects for the blast figure: soft dust and fume (instanced billboards lit as if they were round puffs, shadowed by
 * the rest of the cloud on the side away from the sun, sorted back to front, carried by the wind and lifted by their
 * own heat), additive flashes at the collars, and the delay tags that light hole by hole. No DOM at import.
 */
import * as THREE from "three";
import { clamp, mix, rng, smooth } from "./lib";
import { noise2 } from "./stage";

/* ---------- A soft puff texture: four different puffs in a 2 × 2 atlas ---------- */
let puffTex: THREE.DataTexture | null = null;
export function puffTexture() {
  if (puffTex) return puffTex;
  const S = 128;
  const W = S * 2;
  const d = new Uint8Array(W * W * 4);
  for (let q = 0; q < 4; q++) {
    const { fbm } = noise2(101 + q * 17);
    const ox = (q % 2) * S;
    const oy = Math.floor(q / 2) * S;
    for (let y = 0; y < S; y++) {
      for (let x = 0; x < S; x++) {
        const u = (x + 0.5) / S - 0.5;
        const v = (y + 0.5) / S - 0.5;
        const r = Math.hypot(u, v) * 2;
        const ang = Math.atan2(v, u);
        // Soft from the middle out, with a lumpy edge and a billowy, cauliflower inside, so neighbours merge.
        const edge = 0.8 + 0.2 * fbm(Math.cos(ang) * 1.6 + q, Math.sin(ang) * 1.6, 3);
        const rr = r / edge;
        const body = Math.exp(-rr * rr * 2.6) * (1 - smooth((rr - 0.82) / 0.18));
        const billow = clamp(0.5 + fbm(u * 4.2 + q * 3, v * 4.2, 4) * 1.6, 0, 1);
        const a = body * (0.3 + 0.7 * billow) * 1.25;
        const k = ((oy + y) * W + ox + x) * 4;
        d[k] = Math.round(255 * clamp(0.5 + fbm(u * 9 + 7, v * 9 - q, 3) * 1.4, 0, 1));
        d[k + 1] = 255;
        d[k + 2] = 255;
        d[k + 3] = Math.round(255 * clamp(a, 0, 1));
      }
    }
  }
  puffTex = new THREE.DataTexture(d, W, W, THREE.RGBAFormat);
  puffTex.magFilter = THREE.LinearFilter;
  puffTex.minFilter = THREE.LinearMipmapLinearFilter;
  puffTex.generateMipmaps = true;
  puffTex.needsUpdate = true;
  return puffTex;
}

/* ---------- Dust ---------- */
export interface Puff {
  x: number;
  y: number;
  z: number;
  vx?: number;
  vy?: number;
  vz?: number;
  /** Seconds it lives. */
  life: number;
  /** Size at birth and at the end of its life, m across. */
  size: [number, number];
  alpha: number;
  colour: THREE.Color;
  /** How fast it comes to move with the wind (1/s). */
  drag?: number;
  /** Upward acceleration from heat, m/s², fading over its life. */
  lift?: number;
  spin?: number;
  /** Fraction of its life spent fading in. */
  fadeIn?: number;
  /** Whether it belongs to the cloud that shades itself (default true). */
  cloud?: boolean;
}

const VERT = /* glsl */ `
attribute vec3 iPos;
attribute vec4 iData; // size, alpha, rotation, seed
attribute vec3 iCol;
attribute float iShade;
varying vec2 vUv;
varying vec2 vC;
varying float vAlpha;
varying vec3 vCol;
varying float vSeed;
varying float vShade;
#include <fog_pars_vertex>
void main() {
  vec4 mvPosition = modelViewMatrix * vec4(iPos, 1.0);
  float c = cos(iData.z);
  float s = sin(iData.z);
  vec2 q = vec2(c * position.x - s * position.y, s * position.x + c * position.y);
  mvPosition.xy += q * iData.x;
  vUv = uv;
  vC = q * 2.0;
  vAlpha = iData.y;
  vCol = iCol;
  vSeed = iData.w;
  vShade = iShade;
  gl_Position = projectionMatrix * mvPosition;
  #include <fog_vertex>
}`;
const FRAG = /* glsl */ `
uniform sampler2D map;
uniform vec3 sunDir;
uniform vec3 sunCol;
uniform vec3 ambCol;
uniform float lit;
varying vec2 vUv;
varying vec2 vC;
varying float vAlpha;
varying vec3 vCol;
varying float vSeed;
varying float vShade;
#include <fog_pars_fragment>
void main() {
  vec2 cell = vec2(step(0.5, fract(vSeed * 7.31)), step(0.5, fract(vSeed * 13.7))) * 0.5;
  vec4 t = texture2D(map, vUv * 0.5 + cell);
  float a = t.a * vAlpha;
  if (a < 0.004) discard;
  vec3 col = vCol;
  if (lit > 0.5) {
    // Light the puff as a ball: bright on the sun's side, darker underneath and inside, darker still where the
    // rest of the cloud stands between it and the sun.
    vec3 n = normalize(vec3(vC, sqrt(max(0.0, 1.0 - dot(vC, vC))) + 0.6));
    float l = clamp(dot(n, sunDir) * 0.3 + 0.62, 0.0, 1.0);
    float self = mix(0.78, 1.0, t.r) * mix(1.0, 0.86, t.a);
    float under = mix(0.88, 1.0, clamp(vC.y * 0.5 + 0.5, 0.0, 1.0));
    col = vCol * (ambCol * under + sunCol * l * vShade) * self;
  }
  gl_FragColor = vec4(col, a);
  #include <fog_fragment>
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}`;

export class Dust {
  readonly mesh: THREE.Mesh;
  readonly material: THREE.ShaderMaterial;
  private readonly max: number;
  private n = 0;
  private readonly f: Float32Array; // per particle: x y z vx vy vz age life s0 s1 a0 rot spin drag lift fadeIn r g b seed cloud
  private static readonly K = 21;
  private readonly iPos: THREE.InstancedBufferAttribute;
  private readonly iData: THREE.InstancedBufferAttribute;
  private readonly iCol: THREE.InstancedBufferAttribute;
  private readonly iShade: THREE.InstancedBufferAttribute;
  private readonly geo: THREE.InstancedBufferGeometry;
  private order: Uint16Array;
  private depth: Float32Array;
  private r = rng(9);
  private readonly sun = new THREE.Vector3(0, 1, 0);

  constructor(max: number, o: { additive?: boolean; lit?: boolean } = {}) {
    this.max = max;
    this.f = new Float32Array(max * Dust.K);
    this.order = new Uint16Array(max);
    this.depth = new Float32Array(max);
    const geo = new THREE.InstancedBufferGeometry();
    const quad = new THREE.PlaneGeometry(1, 1);
    geo.index = quad.index;
    geo.setAttribute("position", quad.getAttribute("position"));
    geo.setAttribute("uv", quad.getAttribute("uv"));
    this.iPos = new THREE.InstancedBufferAttribute(new Float32Array(max * 3), 3);
    this.iData = new THREE.InstancedBufferAttribute(new Float32Array(max * 4), 4);
    this.iCol = new THREE.InstancedBufferAttribute(new Float32Array(max * 3), 3);
    this.iShade = new THREE.InstancedBufferAttribute(new Float32Array(max).fill(1), 1);
    for (const a of [this.iPos, this.iData, this.iCol, this.iShade]) a.setUsage(THREE.DynamicDrawUsage);
    geo.setAttribute("iPos", this.iPos);
    geo.setAttribute("iData", this.iData);
    geo.setAttribute("iCol", this.iCol);
    geo.setAttribute("iShade", this.iShade);
    geo.instanceCount = 0;
    this.geo = geo;
    this.material = new THREE.ShaderMaterial({
      uniforms: THREE.UniformsUtils.merge([
        THREE.UniformsLib.fog,
        {
          map: { value: null },
          sunDir: { value: new THREE.Vector3(0, 1, 0) },
          sunCol: { value: new THREE.Color(1, 1, 1) },
          ambCol: { value: new THREE.Color(0.4, 0.4, 0.4) },
          lit: { value: o.lit === false ? 0 : 1 },
        },
      ]),
      vertexShader: VERT,
      fragmentShader: FRAG,
      transparent: true,
      depthWrite: false,
      fog: true,
      blending: o.additive ? THREE.AdditiveBlending : THREE.NormalBlending,
    });
    this.material.uniforms.map!.value = puffTexture();
    this.mesh = new THREE.Mesh(geo, this.material);
    this.mesh.frustumCulled = false;
    this.mesh.renderOrder = o.additive ? 12 : 10;
  }

  get count() {
    return this.n;
  }

  emit(p: Puff) {
    if (this.n >= this.max) return;
    const k = this.n++ * Dust.K;
    const f = this.f;
    f[k] = p.x;
    f[k + 1] = p.y;
    f[k + 2] = p.z;
    f[k + 3] = p.vx ?? 0;
    f[k + 4] = p.vy ?? 0;
    f[k + 5] = p.vz ?? 0;
    f[k + 6] = 0;
    f[k + 7] = p.life;
    f[k + 8] = p.size[0];
    f[k + 9] = p.size[1];
    f[k + 10] = p.alpha;
    f[k + 11] = this.r() * Math.PI * 2;
    f[k + 12] = p.spin ?? (this.r() - 0.5) * 0.3;
    f[k + 13] = p.drag ?? 0.6;
    f[k + 14] = p.lift ?? 0;
    f[k + 15] = p.fadeIn ?? 0.08;
    f[k + 16] = p.colour.r;
    f[k + 17] = p.colour.g;
    f[k + 18] = p.colour.b;
    f[k + 19] = this.r();
    f[k + 20] = p.cloud === false ? 0 : 1;
  }

  clear() {
    this.n = 0;
    this.geo.instanceCount = 0;
  }

  /** Advances by dt seconds; `wind(y)` is the air's velocity at a height, `floor(x, z)` the ground under a point. */
  update(dt: number, wind: (y: number, out: [number, number, number]) => void, floor: (x: number, z: number) => number) {
    const f = this.f;
    const K = Dust.K;
    const w: [number, number, number] = [0, 0, 0];
    let i = 0;
    while (i < this.n) {
      const k = i * K;
      f[k + 6] = f[k + 6]! + dt;
      if (f[k + 6]! >= f[k + 7]!) {
        // Swap the last one in.
        const last = (this.n - 1) * K;
        if (last !== k) f.copyWithin(k, last, last + K);
        this.n--;
        continue;
      }
      wind(f[k + 1]!, w);
      const a = f[k + 6]! / f[k + 7]!;
      const drag = 1 - Math.exp(-f[k + 13]! * dt);
      f[k + 3] = f[k + 3]! + (w[0] - f[k + 3]!) * drag;
      f[k + 4] = f[k + 4]! + (w[1] - f[k + 4]!) * drag + f[k + 14]! * (1 - a) * dt;
      f[k + 5] = f[k + 5]! + (w[2] - f[k + 5]!) * drag;
      f[k] = f[k]! + f[k + 3]! * dt;
      f[k + 1] = f[k + 1]! + f[k + 4]! * dt;
      f[k + 2] = f[k + 2]! + f[k + 5]! * dt;
      const size = mix(f[k + 8]!, f[k + 9]!, 1 - (1 - a) ** 2);
      const g = floor(f[k]!, f[k + 2]!) + size * 0.22;
      if (f[k + 1]! < g) {
        f[k + 1] = g;
        if (f[k + 4]! < 0) f[k + 4] = 0;
      }
      f[k + 11] = f[k + 11]! + f[k + 12]! * dt;
      i++;
    }
    this.geo.instanceCount = this.n;
  }

  /** Writes the live particles to the GPU, farthest first, each shaded by how much cloud lies towards the sun. */
  draw(camera: THREE.Camera) {
    const f = this.f;
    const K = Dust.K;
    const n = this.n;
    if (!n) return;
    const e = camera.matrixWorldInverse.elements;
    // The cloud's centre and spread, weighted by how much each puff blocks.
    let wsum = 0;
    let cx = 0;
    let cy = 0;
    let cz = 0;
    for (let i = 0; i < n; i++) {
      const k = i * K;
      this.order[i] = i;
      // View-space z (more negative is farther).
      this.depth[i] = e[2]! * f[k]! + e[6]! * f[k + 1]! + e[10]! * f[k + 2]! + e[14]!;
      if (!f[k + 20]) continue;
      const a = f[k + 6]! / f[k + 7]!;
      const size = mix(f[k + 8]!, f[k + 9]!, 1 - (1 - a) ** 2);
      const w = f[k + 10]! * size * size * (1 - a);
      wsum += w;
      cx += w * f[k]!;
      cy += w * f[k + 1]!;
      cz += w * f[k + 2]!;
    }
    let spread = 1;
    if (wsum > 0) {
      cx /= wsum;
      cy /= wsum;
      cz /= wsum;
      let v = 0;
      for (let i = 0; i < n; i++) {
        const k = i * K;
        if (!f[k + 20]) continue;
        const a = f[k + 6]! / f[k + 7]!;
        const size = mix(f[k + 8]!, f[k + 9]!, 1 - (1 - a) ** 2);
        const w = f[k + 10]! * size * size * (1 - a);
        v += w * ((f[k]! - cx) ** 2 + (f[k + 1]! - cy) ** 2 + (f[k + 2]! - cz) ** 2);
      }
      spread = Math.max(6, Math.sqrt(v / wsum));
    }
    const ord = this.order.subarray(0, n);
    const dep = this.depth;
    ord.sort((a, b) => dep[a]! - dep[b]!);
    const P = this.iPos.array as Float32Array;
    const D = this.iData.array as Float32Array;
    const C = this.iCol.array as Float32Array;
    const S = this.iShade.array as Float32Array;
    const sx = this.sun.x;
    const sy = this.sun.y;
    const sz = this.sun.z;
    for (let j = 0; j < n; j++) {
      const k = ord[j]! * K;
      const a = f[k + 6]! / f[k + 7]!;
      const size = mix(f[k + 8]!, f[k + 9]!, 1 - (1 - a) ** 2);
      const fi = f[k + 15]!;
      const alpha = f[k + 10]! * smooth(a / Math.max(0.001, fi)) * (1 - smooth((a - 0.4) / 0.6));
      P[j * 3] = f[k]!;
      P[j * 3 + 1] = f[k + 1]!;
      P[j * 3 + 2] = f[k + 2]!;
      D[j * 4] = size;
      D[j * 4 + 1] = alpha;
      D[j * 4 + 2] = f[k + 11]!;
      D[j * 4 + 3] = f[k + 19]!;
      C[j * 3] = f[k + 16]!;
      C[j * 3 + 1] = f[k + 17]!;
      C[j * 3 + 2] = f[k + 18]!;
      // How far into the cloud the puff sits, seen from the sun: the far side and the base are in its shadow.
      if (f[k + 20] && wsum > 0) {
        const toward = ((f[k]! - cx) * sx + (f[k + 1]! - cy) * sy + (f[k + 2]! - cz) * sz) / spread;
        const low = (f[k + 1]! - cy) / spread;
        S[j] = clamp(0.62 + 0.38 * toward + 0.12 * low, 0.14, 1.05);
      } else S[j] = 1;
    }
    for (const a of [this.iPos, this.iData, this.iCol, this.iShade]) {
      a.clearUpdateRanges();
      a.addUpdateRange(0, n * a.itemSize);
      a.needsUpdate = true;
    }
  }

  /** Points the shading at the sun (in view space) and sets the light's colours. */
  light(camera: THREE.Camera, sunWorld: THREE.Vector3, sun: THREE.Color, amb: THREE.Color) {
    const u = this.material.uniforms;
    this.sun.copy(sunWorld).normalize();
    (u.sunDir!.value as THREE.Vector3).copy(sunWorld).transformDirection(camera.matrixWorldInverse);
    (u.sunCol!.value as THREE.Color).copy(sun);
    (u.ambCol!.value as THREE.Color).copy(amb);
  }
}

/* ---------- Delay tags: one billboard per hole, from a canvas atlas ---------- */
const TAG_VERT = /* glsl */ `
attribute vec3 iPos;
attribute vec2 iCell;
attribute float iState;
uniform vec2 grid;
uniform vec2 size;
uniform float ref;
varying vec2 vUv;
varying float vState;
void main() {
  vec4 mv = modelViewMatrix * vec4(iPos, 1.0);
  float grow = 1.0 + 0.35 * clamp(1.0 - abs(iState - 1.0), 0.0, 1.0);
  // Full size from ref metres out; nearer, they shrink with distance, so they hold their size on screen.
  float near = clamp(-mv.z / ref, 0.2, 1.0);
  mv.xy += position.xy * size * grow * near;
  vUv = vec2((iCell.x + uv.x) / grid.x, (iCell.y + 1.0 - uv.y) / grid.y);
  vState = iState;
  gl_Position = projectionMatrix * mv;
}`;
const TAG_FRAG = /* glsl */ `
uniform sampler2D map;
uniform float fade;
varying vec2 vUv;
varying float vState;
void main() {
  vec4 t = texture2D(map, vec2(vUv.x, 1.0 - vUv.y));
  // State: 0 waiting (white on dark), 1 firing (hot), 2 fired (dim amber).
  float hot = clamp(1.0 - abs(vState - 1.0), 0.0, 1.0);
  float done = clamp(vState - 1.0, 0.0, 1.0);
  vec3 ink = mix(vec3(0.95, 0.93, 0.9), vec3(1.0, 0.72, 0.3), done);
  ink = mix(ink, vec3(1.0, 0.95, 0.75), hot);
  vec3 bg = mix(vec3(0.05, 0.04, 0.035), vec3(0.95, 0.45, 0.08), hot * 0.9);
  bg = mix(bg, vec3(0.16, 0.08, 0.02), done * (1.0 - hot));
  vec3 col = mix(bg, ink, t.r);
  float a = t.a * mix(0.88, 1.0, hot) * mix(1.0, 0.72, done * (1.0 - hot)) * fade;
  if (a < 0.01) discard;
  gl_FragColor = vec4(col, a);
  #include <colorspace_fragment>
}`;

export class Tags {
  readonly mesh: THREE.Mesh;
  private readonly state: THREE.InstancedBufferAttribute;
  readonly material: THREE.ShaderMaterial;

  constructor(items: { text: string; at: THREE.Vector3 }[], o: { width: number; height: number; ref?: number }) {
    const cols = 8;
    const rows = Math.ceil(items.length / cols);
    const cw = 128;
    const ch = 52;
    const canvas = document.createElement("canvas");
    canvas.width = cols * cw;
    canvas.height = rows * ch;
    const g = canvas.getContext("2d")!;
    // Background pill in alpha; ink in the red channel.
    items.forEach((it, i) => {
      const x = (i % cols) * cw;
      const y = Math.floor(i / cols) * ch;
      g.fillStyle = "rgba(0,0,0,1)";
      g.beginPath();
      g.roundRect(x + 4, y + 4, cw - 8, ch - 8, (ch - 8) / 2);
      g.fill();
      g.fillStyle = "rgba(255,0,0,1)";
      g.font = "600 30px 'JetBrains Mono', ui-monospace, monospace";
      g.textAlign = "center";
      g.textBaseline = "middle";
      g.fillText(it.text, x + cw / 2, y + ch / 2 + 1);
    });
    const tex = new THREE.CanvasTexture(canvas);
    tex.anisotropy = 4;
    tex.colorSpace = THREE.NoColorSpace;
    const geo = new THREE.InstancedBufferGeometry();
    const quad = new THREE.PlaneGeometry(1, 1);
    geo.index = quad.index;
    geo.setAttribute("position", quad.getAttribute("position"));
    geo.setAttribute("uv", quad.getAttribute("uv"));
    const pos = new Float32Array(items.length * 3);
    const cell = new Float32Array(items.length * 2);
    items.forEach((it, i) => {
      pos.set([it.at.x, it.at.y, it.at.z], i * 3);
      cell.set([i % cols, Math.floor(i / cols)], i * 2);
    });
    geo.setAttribute("iPos", new THREE.InstancedBufferAttribute(pos, 3));
    geo.setAttribute("iCell", new THREE.InstancedBufferAttribute(cell, 2));
    this.state = new THREE.InstancedBufferAttribute(new Float32Array(items.length), 1);
    this.state.setUsage(THREE.DynamicDrawUsage);
    geo.setAttribute("iState", this.state);
    geo.instanceCount = items.length;
    this.material = new THREE.ShaderMaterial({
      uniforms: {
        map: { value: tex },
        grid: { value: new THREE.Vector2(cols, rows) },
        size: { value: new THREE.Vector2(o.width, o.height) },
        ref: { value: o.ref ?? 150 },
        fade: { value: 1 },
      },
      vertexShader: TAG_VERT,
      fragmentShader: TAG_FRAG,
      transparent: true,
      depthWrite: false,
      depthTest: true,
    });
    this.mesh = new THREE.Mesh(geo, this.material);
    this.mesh.frustumCulled = false;
    this.mesh.renderOrder = 20;
  }

  set(i: number, v: number) {
    (this.state.array as Float32Array)[i] = v;
    this.state.needsUpdate = true;
  }
  fade(v: number) {
    this.material.uniforms.fade!.value = v;
    this.mesh.visible = v > 0.01;
  }
}
