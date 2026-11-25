/**
 * Weather for the tailings scene: a cloud deck that thickens from a few clouds to a low grey overcast, rain that
 * falls in streaks around the camera, the light that dims and greys under it, haze that closes in, and the odd flash
 * of lightning in the heaviest storms.
 */
import * as THREE from "three";
import { noiseTexture } from "./fleet-kit";
import { mix, rng } from "./lib";
import type { Stage } from "./stage";

const CLOUD_VERT = /* glsl */ `
varying vec3 vD;
void main() {
  vD = normalize(position);
  vec4 p = projectionMatrix * mat4(mat3(modelViewMatrix)) * vec4(position, 1.0);
  gl_Position = p.xyww;
}`;
const CLOUD_FRAG = /* glsl */ `
uniform sampler2D tNoise;
uniform float uCover;
uniform float uStorm;
uniform float uTime;
uniform float uFlash;
uniform vec3 uSun;
uniform vec3 uSunCol;
uniform vec3 uLit;
uniform vec3 uDark;
uniform vec3 uHaze;
varying vec3 vD;
void main() {
  vec3 d = normalize(vD);
  float h = d.y;
  vec2 uv = d.xz / (max(h, 0.0) + 0.09) * 0.21 + vec2(uTime * 0.0032, uTime * 0.0013);
  float n = texture2D(tNoise, uv * 0.6).r * 0.5 + texture2D(tNoise, uv * 1.7 + 0.37).g * 0.3 + texture2D(tNoise, uv * 4.3 + 0.71).b * 0.2;
  float cov = mix(0.66, 0.2, uCover);
  float a = smoothstep(cov, cov + 0.2, n);
  a = max(a, smoothstep(0.35, 1.0, uCover) * 0.96);
  float dens = smoothstep(cov - 0.05, cov + 0.45, n);
  float sunSide = pow(max(dot(d, uSun), 0.0), 6.0);
  vec3 col = mix(uLit, uDark, clamp(dens * 0.55 + uStorm * 0.75, 0.0, 1.0));
  // Silver edges towards the sun when the cloud is thin.
  col += uSunCol * sunSide * (1.0 - dens) * (1.0 - uStorm) * 0.6;
  // Darker bands of rain hanging under the deck.
  float shaft = texture2D(tNoise, vec2(atan(d.x, d.z) * 1.7 + uTime * 0.002, 0.3)).r;
  col *= 1.0 - uStorm * 0.25 * smoothstep(0.45, 0.75, shaft) * (1.0 - smoothstep(0.0, 0.25, h));
  col += vec3(0.75, 0.8, 0.95) * uFlash * (0.35 + 0.65 * n);
  // Towards the horizon the deck sinks into the haze; in a storm the haze closes over it.
  float toHaze = 1.0 - smoothstep(-0.02, 0.2, h);
  col = mix(col, uHaze, toHaze * (0.6 + 0.4 * uStorm));
  a *= smoothstep(-0.06, 0.06, h);
  a = max(a, uStorm * 0.97 * smoothstep(0.5, 1.0, uCover) * (1.0 - smoothstep(-0.35, -0.2, -h - 0.4)));
  gl_FragColor = vec4(col, a);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}`;

const RAIN_VERT = /* glsl */ `
attribute vec2 corner;
attribute vec4 seed;
uniform float uTime;
uniform float uDensity;
uniform float uLen;
uniform vec3 uBox;
uniform vec3 uCentre;
uniform vec3 uFall;
varying float vA;
varying float vV;
void main() {
  if (seed.w > uDensity) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); vA = 0.0; vV = 0.0; return; }
  vec3 p = seed.xyz * uBox + uFall * uTime;
  p = uCentre + mod(p - uCentre, uBox) - uBox * 0.5;
  vec3 dir = normalize(uFall);
  vec3 toCam = cameraPosition - p;
  float dist = length(toCam);
  vec3 side = normalize(cross(dir, toCam / dist));
  // At least a pixel wide wherever it is, and longer further off, as a camera's shutter would smear it.
  float width = max(0.012, dist * 0.0009);
  vec3 wp = p + side * corner.x * width + dir * corner.y * uLen * (1.0 + dist * 0.006);
  vec3 q = abs(p - uCentre) / (uBox * 0.5);
  vA = (1.0 - smoothstep(0.7, 1.0, max(q.x, q.z))) * smoothstep(1.5, 8.0, dist);
  vV = corner.y;
  gl_Position = projectionMatrix * viewMatrix * vec4(wp, 1.0);
}`;
const RAIN_FRAG = /* glsl */ `
uniform vec3 uColor;
uniform float uOpacity;
varying float vA;
varying float vV;
void main() {
  float a = vA * uOpacity * sin(vV * 3.14159);
  if (a < 0.004) discard;
  gl_FragColor = vec4(uColor, a);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}`;

export interface Sky {
  clouds: THREE.Mesh;
  rain: THREE.Mesh;
  /** Sets the weather: storm 0 (fair) to 1 (the heaviest), rain 0 to 1, and advances the clocks. */
  update(dt: number, storm: number, rain: number): void;
  /** 0..1 brightness of the light this frame, for the water's own colour. */
  readonly light: number;
}

export function makeSky(s: Stage, o: { small: boolean }): Sky {
  /* ---------- Clouds ---------- */
  const cu = {
    tNoise: { value: noiseTexture() },
    uCover: { value: 0.1 },
    uStorm: { value: 0 },
    uTime: { value: 0 },
    uFlash: { value: 0 },
    uSun: { value: new THREE.Vector3(0, 1, 0) },
    uSunCol: { value: new THREE.Color("#ffe2b8") },
    uLit: { value: new THREE.Color("#f1ece4") },
    uDark: { value: new THREE.Color("#4a5059") },
    uHaze: { value: new THREE.Color("#cdbfa9") },
  };
  const clouds = new THREE.Mesh(
    new THREE.SphereGeometry(1, 48, 24),
    new THREE.ShaderMaterial({
      side: THREE.BackSide,
      depthWrite: false,
      depthTest: false,
      fog: false,
      transparent: false,
      blending: THREE.CustomBlending,
      blendSrc: THREE.SrcAlphaFactor,
      blendDst: THREE.OneMinusSrcAlphaFactor,
      uniforms: cu,
      vertexShader: CLOUD_VERT,
      fragmentShader: CLOUD_FRAG,
    }),
  );
  clouds.frustumCulled = false;
  clouds.renderOrder = -999;
  clouds.scale.setScalar(s.camera.far * 0.85);
  clouds.name = "clouds";
  s.scene.add(clouds);

  /* ---------- Rain ---------- */
  const count = o.small ? 9000 : 26000;
  const g = new THREE.InstancedBufferGeometry();
  g.setAttribute("corner", new THREE.Float32BufferAttribute([-0.5, 0, 0.5, 0, -0.5, 1, 0.5, 1], 2));
  g.setIndex([0, 1, 2, 2, 1, 3]);
  const seeds = new Float32Array(count * 4);
  const r = rng(77);
  for (let i = 0; i < count * 4; i++) seeds[i] = r();
  g.setAttribute("seed", new THREE.InstancedBufferAttribute(seeds, 4));
  g.instanceCount = count;
  const ru = {
    uTime: { value: 0 },
    uDensity: { value: 0 },
    uLen: { value: 1.6 },
    uBox: { value: new THREE.Vector3(420, 260, 420) },
    uCentre: { value: new THREE.Vector3() },
    uFall: { value: new THREE.Vector3(2.2, -9, 1.2) },
    uColor: { value: new THREE.Color("#b9c2cc") },
    uOpacity: { value: 0.32 },
  };
  const rain = new THREE.Mesh(
    g,
    new THREE.ShaderMaterial({
      uniforms: ru,
      vertexShader: RAIN_VERT,
      fragmentShader: RAIN_FRAG,
      transparent: true,
      depthWrite: false,
      fog: false,
    }),
  );
  rain.frustumCulled = false;
  rain.renderOrder = 20;
  rain.visible = false;
  rain.name = "rain";
  s.scene.add(rain);

  /* ---------- Light ---------- */
  const fog = s.scene.fog as THREE.FogExp2;
  const base = {
    sun: s.sun.intensity,
    sunCol: s.sun.color.clone(),
    hemi: s.hemi.intensity,
    hemiCol: s.hemi.color.clone(),
    hemiGround: s.hemi.groundColor.clone(),
    env: s.scene.environmentIntensity,
    fog: fog.density,
    fogCol: fog.color.clone(),
    exposure: s.renderer.toneMappingExposure,
  };
  const grey = { sun: new THREE.Color("#c4ccd6"), hemi: new THREE.Color("#a9b2bd"), ground: new THREE.Color("#45423d"), fog: new THREE.Color("#717980") };
  let flash = 0;
  let nextFlash = 4;
  let time = 0;
  let light = 1;
  const fwd = new THREE.Vector3();
  const sky: Sky = {
    clouds,
    rain,
    get light() {
      return light;
    },
    update(dt, storm, wet) {
      time += dt;
      // Lightning only in the heaviest storms: a double flicker every few seconds.
      if (storm > 0.72) {
        nextFlash -= dt;
        if (nextFlash < 0) {
          flash = 1;
          nextFlash = 3 + r() * 6;
        }
      }
      flash = Math.max(0, flash - dt * 5.5);
      const f = flash * (0.6 + 0.4 * Math.sin(time * 70)) * Math.min(1, (storm - 0.6) * 3);
      cu.uCover.value = mix(0.12, 1, Math.min(1, storm * 1.35));
      cu.uStorm.value = storm;
      cu.uTime.value = time;
      cu.uFlash.value = Math.max(0, f);
      cu.uSun.value.copy(s.sun.position).sub(s.sun.target.position).normalize();
      s.sun.intensity = base.sun * (1 - 0.9 * Math.min(1, storm * 1.25));
      s.sun.color.copy(base.sunCol).lerp(grey.sun, storm);
      s.hemi.intensity = mix(base.hemi, base.hemi * 1.15, storm) + Math.max(0, f) * 2.4;
      s.hemi.color.copy(base.hemiCol).lerp(grey.hemi, storm);
      s.hemi.groundColor.copy(base.hemiGround).lerp(grey.ground, storm);
      s.scene.environmentIntensity = mix(base.env, base.env * 0.6, storm);
      fog.density = base.fog * (1 + 2.6 * storm * storm);
      fog.color.copy(base.fogCol).lerp(grey.fog, Math.min(1, storm * 1.2)).multiplyScalar(1 - 0.25 * storm);
      cu.uHaze.value.copy(fog.color);
      s.renderer.toneMappingExposure = base.exposure * (1 - 0.1 * storm);
      light = mix(1, 0.45, storm);
      // Rain around the camera, thickest between it and what it looks at.
      rain.visible = wet > 0.005;
      ru.uTime.value = time;
      ru.uDensity.value = wet;
      ru.uOpacity.value = 0.18 + 0.2 * wet;
      ru.uColor.value.set("#aeb7c1").multiplyScalar(mix(1, 0.75, storm));
      s.camera.getWorldDirection(fwd);
      const dist = s.camera.position.distanceTo(s.orbit.target);
      const reach = Math.min(dist * 0.55, 260);
      ru.uCentre.value.copy(s.camera.position).addScaledVector(fwd, reach);
      const w = Math.min(520, Math.max(120, dist * 0.9));
      ru.uBox.value.set(w, Math.max(160, w * 0.6), w);
    },
  };
  return sky;
}
