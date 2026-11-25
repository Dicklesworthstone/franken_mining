/**
 * Water for the tailings scene: the pond as a planar mirror (it reflects the embankment, the hills and the sky) with
 * wind ripples, rain rings and a shallow edge that shows the beach through it; the slurry that runs from the open
 * spigots down the beach in wet channels; and the small lined seepage pond at the toe.
 */
import * as THREE from "three";
import { Reflector } from "three/examples/jsm/objects/Reflector.js";
import { noiseTexture } from "./fleet-kit";
import { mix, rng } from "./lib";
import { bed, crestZ, DAM, POND, SHORE, tailings } from "./tailings-model";
import type { Weather } from "./tailings-ground";

/** The pond's extent: every place it could reach in the largest storm. */
const BOX = { x0: -620, x1: 320, z0: -980, z1: -60 };

/** The pond's bed as a texture, so the water knows how deep it is everywhere: (bed − today's level), metres. */
function bedTexture(cell: number) {
  const w = Math.round((BOX.x1 - BOX.x0) / cell) + 1;
  const h = Math.round((BOX.z1 - BOX.z0) / cell) + 1;
  const data = new Uint16Array(w * h);
  for (let j = 0; j < h; j++) {
    for (let i = 0; i < w; i++) {
      const x = BOX.x0 + (i / (w - 1)) * (BOX.x1 - BOX.x0);
      const z = BOX.z0 + (j / (h - 1)) * (BOX.z1 - BOX.z0);
      data[j * w + i] = THREE.DataUtils.toHalfFloat(Math.max(-8, Math.min(8, bed(x, z) - POND)));
    }
  }
  const t = new THREE.DataTexture(data, w, h, THREE.RedFormat, THREE.HalfFloatType);
  t.magFilter = THREE.LinearFilter;
  t.minFilter = THREE.LinearFilter;
  t.wrapS = t.wrapT = THREE.ClampToEdgeWrapping;
  t.needsUpdate = true;
  return t;
}

const WATER_VERT = /* glsl */ `
uniform mat4 textureMatrix;
varying vec4 vUv;
varying vec3 vW;
#include <common>
#include <fog_pars_vertex>
void main() {
  vUv = textureMatrix * vec4(position, 1.0);
  vec4 wp = modelMatrix * vec4(position, 1.0);
  vW = wp.xyz;
  vec4 mvPosition = viewMatrix * wp;
  gl_Position = projectionMatrix * mvPosition;
  #include <fog_vertex>
}`;

const WATER_FRAG = /* glsl */ `
uniform vec3 color;
uniform sampler2D tDiffuse;
uniform sampler2D tNoise;
uniform sampler2D tBed;
uniform vec4 uBox;
uniform float uLevel;
uniform float uRef;
uniform float uTime;
uniform float uRain;
uniform float uStorm;
uniform vec3 uSunDir;
uniform vec3 uSunCol;
uniform vec3 uDeep;
uniform vec3 uShallow;
uniform vec2 uTexel;
varying vec4 vUv;
varying vec3 vW;
#include <common>
#include <fog_pars_fragment>
vec2 wHash(vec2 p) {
  p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
  return fract(sin(p) * 43758.5453);
}
// Rings spreading from raindrops: one drop per cell, each on its own clock.
vec2 rings(vec2 p, float t) {
  vec2 n = floor(p);
  vec2 f = fract(p);
  vec2 acc = vec2(0.0);
  for (int j = -1; j <= 1; j++)
    for (int i = -1; i <= 1; i++) {
      vec2 g = vec2(float(i), float(j));
      vec2 h = wHash(n + g);
      vec2 c = f - (g + 0.15 + 0.7 * h);
      float life = fract(t * (0.55 + 0.6 * h.x) + h.y * 7.0);
      float r = life * 0.85;
      float d = length(c);
      float x = d - r;
      float ring = sin(x * 38.0) * exp(-x * x * 140.0) * (1.0 - life) * (1.0 - life);
      acc += c / max(d, 1e-3) * ring;
    }
  return acc;
}
void main() {
  vec3 toEye = cameraPosition - vW;
  float dist = length(toEye);
  vec3 V = toEye / dist;
  vec2 p = vW.xz;
  // Depth over the bed.
  vec2 bt = (p - uBox.xy) * uBox.zw;
  float depth = uLevel - (uRef + texture2D(tBed, bt).r);
  if (depth < -0.02) discard;
  // Wind ripples.
  // Cat's paws: patches where the wind ruffles the water, between glassy streaks where it does not.
  vec4 paw = texture2D(tNoise, p * 0.0031 + vec2(uTime * 0.0012, uTime * 0.0004));
  float ruffle = mix(0.25, 1.35, smoothstep(0.38, 0.68, paw.r * 0.7 + paw.b * 0.4));
  float wind = (0.6 + 1.6 * uStorm) * mix(ruffle, 1.2, uStorm);
  vec4 a = texture2D(tNoise, p * 0.017 + vec2(uTime * 0.0045, uTime * 0.0031));
  vec4 b = texture2D(tNoise, p * 0.061 - vec2(uTime * 0.0102, -uTime * 0.0066));
  vec4 c = texture2D(tNoise, p * vec2(0.23, 0.31) + vec2(uTime * 0.021, uTime * 0.017));
  vec4 e = texture2D(tNoise, p * vec2(0.71, 0.93) - vec2(uTime * 0.04, uTime * 0.013));
  float pxw = length(fwidth(p));
  vec2 tilt = ((a.rg - 0.5) * 0.45 + (b.ba - 0.5) * 0.4 + (c.gr - 0.5) * 0.35 + (e.rb - 0.5) * 0.3 * (1.0 - smoothstep(0.3, 1.2, pxw))) * 0.075 * wind;
  float px = length(fwidth(p));
  if (uRain > 0.01) {
    float near = 1.0 - smoothstep(0.15, 0.6, px);
    float mid = 1.0 - smoothstep(0.6, 2.4, px);
    vec2 r1 = near > 0.0 ? rings(p * 1.1, uTime * 1.3) : vec2(0.0);
    vec2 r2 = mid > 0.0 ? rings(p * 0.31 + 7.3, uTime * 0.9) : vec2(0.0);
    tilt += (r1 * near * 0.6 + r2 * mid * 0.4) * uRain;
    // Further off, the rain just roughens the water.
    tilt += ((c.ba - 0.5) * 0.5 + (b.rg - 0.5) * 0.4) * 0.22 * uRain * smoothstep(0.3, 2.5, px);
  }
  vec3 N = normalize(vec3(tilt.x, 1.0, tilt.y));
  // The reflection, bent by the ripples and blurred by the rain.
  vec2 ruv = vUv.xy / vUv.w + N.xz * (0.0015 + 1.0 / max(dist, 1.0)) * 18.0;
  vec3 refl = texture2D(tDiffuse, ruv).rgb;
  // Ruffled water mirrors sky from higher up, bluer and darker than the horizon the glassy water shows.
  float rough = smoothstep(0.38, 0.68, paw.r * 0.7 + paw.b * 0.4);
  refl = mix(refl, refl * vec3(0.58, 0.66, 0.76), rough * 0.65 * (1.0 - uStorm));
  if (uRain > 0.01) {
    float blur = uRain * (2.0 + 6.0 * uStorm);
    refl = refl * 0.4
      + texture2D(tDiffuse, ruv + vec2(blur, 0.4 * blur) * uTexel).rgb * 0.15
      + texture2D(tDiffuse, ruv - vec2(0.4 * blur, blur) * uTexel).rgb * 0.15
      + texture2D(tDiffuse, ruv + vec2(-blur, 0.5 * blur) * uTexel).rgb * 0.15
      + texture2D(tDiffuse, ruv + vec2(0.3 * blur, -blur) * uTexel).rgb * 0.15;
  }
  float cosT = max(dot(N, V), 0.0);
  float F = 0.02 + 0.98 * pow(1.0 - cosT, 5.0);
  // Murky supernatant: grey-green in depth, milky where fines are still settling near the beach, the bed showing
  // through where it is shallowest.
  float shallow = 1.0 - smoothstep(0.0, 1.4, depth);
  float milky = 1.0 - smoothstep(0.0, 0.9, depth);
  vec3 body = mix(uDeep, uShallow, milky * 0.55 + shallow * 0.35);
  body *= 0.92 + 0.16 * a.b;
  vec3 col = mix(body, refl, clamp(F * 0.95, 0.0, 1.0));
  // The sun's glints on the ripples.
  vec3 H = normalize(uSunDir + V);
  float spec = pow(max(dot(N, H), 0.0), 520.0) * 3.0 + pow(max(dot(N, H), 0.0), 60.0) * 0.06;
  col += uSunCol * spec * (1.0 - uStorm * 0.92);
  float alpha = smoothstep(-0.02, 0.09, depth);
  gl_FragColor = vec4(col, alpha);
  #include <fog_fragment>
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}`;

export interface Pond {
  mesh: Reflector;
  /** Copies the sun and the light into the water each frame. */
  update(sun: THREE.DirectionalLight, light: number): void;
  resize(w: number, h: number): void;
}

/** The pond: a mirror at the water's level that only shows where the water is deeper than the bed. */
export function makePond(o: { small: boolean; weather: Weather; size: [number, number]; hide: THREE.Object3D[] }): Pond {
  const scale = o.small ? 0.4 : 0.6;
  const geo = new THREE.PlaneGeometry(BOX.x1 - BOX.x0, BOX.z1 - BOX.z0);
  const tw = Math.max(256, Math.round(o.size[0] * scale));
  const th = Math.max(256, Math.round(o.size[1] * scale));
  const mesh = new Reflector(geo, {
    textureWidth: tw,
    textureHeight: th,
    clipBias: 0.002,
    multisample: o.small ? 0 : 4,
    shader: {
      name: "fm-pond",
      uniforms: {
        color: { value: new THREE.Color("#7f7f7f") },
        tDiffuse: { value: null },
        textureMatrix: { value: null },
      },
      vertexShader: WATER_VERT,
      fragmentShader: WATER_FRAG,
    },
  });
  const mat = mesh.material as THREE.ShaderMaterial;
  Object.assign(mat.uniforms, THREE.UniformsUtils.clone(THREE.UniformsLib.fog), {
    tNoise: { value: noiseTexture() },
    tBed: { value: bedTexture(o.small ? 6 : 4) },
    uBox: { value: new THREE.Vector4(BOX.x0, BOX.z0, 1 / (BOX.x1 - BOX.x0), 1 / (BOX.z1 - BOX.z0)) },
    uLevel: o.weather.level,
    uRef: { value: POND },
    uTime: o.weather.time,
    uRain: o.weather.wet,
    uStorm: o.weather.storm,
    uSunDir: { value: new THREE.Vector3(0, 1, 0) },
    uSunCol: { value: new THREE.Color("#ffffff") },
    uDeep: { value: new THREE.Color("#2f4744") },
    uShallow: { value: new THREE.Color("#7b7c73") },
    uTexel: { value: new THREE.Vector2(1 / tw, 1 / th) },
  });
  mat.fog = true;
  mat.transparent = true;
  mat.depthWrite = false;
  // Pulled a little towards the camera, so the water wins where it meets the nearly flat beach at a shallow angle;
  // its own alpha fades to nothing there.
  mat.polygonOffset = true;
  mat.polygonOffsetFactor = -1;
  mat.polygonOffsetUnits = -4;
  mesh.rotation.x = -Math.PI / 2;
  mesh.position.set((BOX.x0 + BOX.x1) / 2, POND, (BOX.z0 + BOX.z1) / 2);
  mesh.renderOrder = 2;
  mesh.name = "pond";
  // Rain, labels in 3D and the x-ray section stay out of the reflection.
  const inner = mesh.onBeforeRender;
  mesh.onBeforeRender = function (renderer, scene, camera, geometry, material, group) {
    const was = o.hide.map((h) => h.visible);
    for (const h of o.hide) h.visible = false;
    inner.call(this, renderer, scene, camera, geometry, material, group);
    o.hide.forEach((h, i) => (h.visible = was[i]!));
  };
  const deep = new THREE.Color("#2b3632");
  const shallow = new THREE.Color("#6d6c65");
  const storm = new THREE.Color("#262d2c");
  return {
    mesh,
    update(sun, light) {
      mat.uniforms.uSunDir!.value.copy(sun.position).sub(sun.target.position).normalize();
      mat.uniforms.uSunCol!.value.copy(sun.color).multiplyScalar(sun.intensity * 0.25);
      const s = o.weather.storm.value;
      mat.uniforms.uDeep!.value.copy(deep).lerp(storm, s).multiplyScalar(light);
      mat.uniforms.uShallow!.value.copy(shallow).multiplyScalar(light * mix(1, 0.6, s));
    },
    resize(w, h) {
      const tw2 = Math.max(256, Math.round(w * scale));
      const th2 = Math.max(256, Math.round(h * scale));
      mesh.getRenderTarget().setSize(tw2, th2);
      mat.uniforms.uTexel!.value.set(1 / tw2, 1 / th2);
    },
  };
}

/* ---------- Slurry ---------- */
/** Wet slurry running in a channel: dark, glossy, its surface sliding downstream. */
export function slurryMaterial(w: Weather) {
  const mat = new THREE.MeshStandardMaterial({ color: "#5c5a55", roughness: 0.16, metalness: 0, transparent: true, depthWrite: false });
  mat.polygonOffset = true;
  mat.polygonOffsetFactor = -2;
  mat.polygonOffsetUnits = -2;
  const uniforms = { tNoise: { value: noiseTexture() }, uTime: w.time };
  mat.customProgramCacheKey = () => "fm-slurry-v1";
  mat.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, uniforms);
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nattribute vec2 flow;\nvarying vec2 vFlow;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\nvFlow = flow;");
    sh.fragmentShader = sh.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform sampler2D tNoise;\nuniform float uTime;\nvarying vec2 vFlow;")
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
float across = vFlow.x;
float along = vFlow.y;
vec4 fl = texture2D(tNoise, vec2(across * 0.35 + along * 0.002, along * 0.045 - uTime * 0.32));
vec4 fl2 = texture2D(tNoise, vec2(across * 0.9, along * 0.12 - uTime * 0.75));
float edge = 1.0 - smoothstep(0.55, 1.0, abs(across));
diffuseColor.rgb *= 0.82 + 0.3 * fl.r + 0.12 * fl2.g;
diffuseColor.a *= edge * (0.85 + 0.15 * fl2.b);`,
      )
      .replace("#include <roughnessmap_fragment>", "#include <roughnessmap_fragment>\nroughnessFactor = 0.1 + 0.25 * texture2D(tNoise, vec2(vFlow.x * 0.5, vFlow.y * 0.08 - uTime * 0.4)).g;");
  };
  return mat;
}

/** The channel each open spigot's slurry cuts down the beach to the water, as a draped ribbon. */
export function makeChannels(spigots: number[], w: Weather, seed = 3) {
  const r = rng(seed);
  const pos: number[] = [];
  const flow: number[] = [];
  const index: number[] = [];
  for (const x0 of spigots) {
    const phase = r() * 6;
    const amp = 10 + r() * 18;
    const pts: { x: number; d: number; w: number }[] = [];
    const end = SHORE + 25;
    for (let d = 0.5; d <= end; d += 2) {
      const x = x0 + amp * Math.sin(d * 0.019 + phase) * Math.min(1, d / 90) + 6 * Math.sin(d * 0.071 + phase * 2);
      // Narrow under the spigot, braiding wider down the beach, spreading into a fan at the water.
      const width = 1.6 + d * 0.012 + 18 * Math.max(0, (d - SHORE + 70) / 95) ** 2;
      pts.push({ x, d, w: width });
    }
    const base = pos.length / 3;
    let along = 0;
    for (let i = 0; i < pts.length; i++) {
      const p = pts[i]!;
      const q = pts[Math.min(pts.length - 1, i + 1)]!;
      const o2 = pts[Math.max(0, i - 1)]!;
      const tx = q.x - o2.x;
      const td = q.d - o2.d;
      const l = Math.hypot(tx, td) || 1;
      // Across the channel, in (x, d).
      const nx = td / l;
      const nd = -tx / l;
      if (i > 0) along += Math.hypot(p.x - pts[i - 1]!.x, p.d - pts[i - 1]!.d);
      for (const side of [-1, 1]) {
        const x = p.x + nx * side * p.w * 0.5;
        const d = p.d + nd * side * p.w * 0.5;
        const z = crestZ(x) - DAM.crestHalf - d;
        const y = tailings(x, z) + 0.05;
        pos.push(x, y, z);
        flow.push(side, along);
      }
      if (i > 0) {
        const a = base + (i - 1) * 2;
        index.push(a, a + 2, a + 1, a + 1, a + 2, a + 3);
      }
    }
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  geo.setAttribute("flow", new THREE.Float32BufferAttribute(flow, 2));
  geo.setIndex(index);
  geo.computeVertexNormals();
  const mesh = new THREE.Mesh(geo, slurryMaterial(w));
  mesh.receiveShadow = true;
  mesh.renderOrder = 1;
  mesh.name = "slurry";
  return mesh;
}

/** A small still water surface for the seepage pond: dark, glossy, catching the sky. */
export function stillWater() {
  return new THREE.MeshPhysicalMaterial({ color: "#4d5f57", roughness: 0.12, metalness: 0, clearcoat: 1, clearcoatRoughness: 0.05, envMapIntensity: 2.2 });
}
