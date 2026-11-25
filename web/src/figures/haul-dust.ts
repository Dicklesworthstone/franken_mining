/**
 * Dust for the haul scene: soft, lit, fogged billboards in one instanced draw. Trucks raise trails from their rear
 * wheels in proportion to speed; a pass into a truck and a tip at the crusher or the dump throw up a puff. Particles
 * drift downwind, rise a little, grow and thin out. Nothing touches the DOM.
 */
import * as THREE from "three";

export interface PuffOptions {
  n?: number;
  /** Starting size and how much it grows over its life, metres. */
  size?: number;
  grow?: number;
  life?: number;
  alpha?: number;
  /** Starting velocity, m/s, and the random spread added to it. */
  vel?: THREE.Vector3;
  spread?: number;
  /** Random scatter of the start point, metres. */
  scatter?: number;
  /** How much sun reaches it: 1 in sunlight, 0 in the pit wall's shadow. */
  lit?: number;
}

export interface Dust {
  mesh: THREE.Mesh;
  puff(at: THREE.Vector3, o?: PuffOptions): void;
  update(dt: number): void;
  /** Lights the dust: the sun's colour and strength, and the sky's. */
  /** `lamp` (0–1): how far the loading-area lamps are up; dust round the loaders glows warm in them after dusk. */
  light(sun: THREE.Color, sunI: number, sky: THREE.Color, skyI: number, ground: THREE.Color, sunDir?: THREE.Vector3, lamp?: number): void;
  /** Wind, m/s. */
  wind: THREE.Vector3;
}

function puffTexture() {
  const N = 96;
  const data = new Uint8Array(N * N * 4);
  // A few overlapping soft lobes with fine grain, so no two puffs read as the same disc.
  const lobes: [number, number, number][] = [
    [0.5, 0.5, 0.36],
    [0.38, 0.56, 0.22],
    [0.62, 0.44, 0.24],
    [0.55, 0.64, 0.2],
    [0.42, 0.4, 0.2],
  ];
  let seed = 7;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const grain = Array.from({ length: 64 }, () => rnd());
  for (let y = 0; y < N; y++) {
    for (let x = 0; x < N; x++) {
      const u = x / (N - 1);
      const v = y / (N - 1);
      let d = 0;
      for (const [cx, cy, r] of lobes) {
        const q = Math.hypot(u - cx, v - cy) / r;
        d += Math.max(0, 1 - q * q) ** 2;
      }
      const g = grain[(x * 7 + y * 13) & 63]! * 0.25 + 0.75;
      const edge = Math.max(0, 1 - Math.hypot(u - 0.5, v - 0.5) * 2);
      const a = Math.min(1, d * 0.85) * g * Math.min(1, edge * 2.2);
      const i = (y * N + x) * 4;
      data[i] = data[i + 1] = data[i + 2] = 255;
      data[i + 3] = Math.round(a * 255);
    }
  }
  const tex = new THREE.DataTexture(data, N, N, THREE.RGBAFormat);
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  tex.needsUpdate = true;
  return tex;
}

export function makeDust(max = 1400): Dust {
  const geo = new THREE.InstancedBufferGeometry();
  const quad = new THREE.PlaneGeometry(1, 1);
  geo.index = quad.index;
  geo.setAttribute("position", quad.getAttribute("position"));
  geo.setAttribute("uv", quad.getAttribute("uv"));
  const iPos = new THREE.InstancedBufferAttribute(new Float32Array(max * 3), 3);
  const iSize = new THREE.InstancedBufferAttribute(new Float32Array(max), 1);
  const iAlpha = new THREE.InstancedBufferAttribute(new Float32Array(max), 1);
  const iRot = new THREE.InstancedBufferAttribute(new Float32Array(max), 1);
  const iLit = new THREE.InstancedBufferAttribute(new Float32Array(max), 1);
  for (const a of [iPos, iSize, iAlpha, iRot, iLit]) a.setUsage(THREE.DynamicDrawUsage);
  geo.setAttribute("iPos", iPos);
  geo.setAttribute("iSize", iSize);
  geo.setAttribute("iAlpha", iAlpha);
  geo.setAttribute("iRot", iRot);
  geo.setAttribute("iLit", iLit);
  geo.instanceCount = max;

  const uniforms = THREE.UniformsUtils.merge([
    THREE.UniformsLib.fog,
    {
      tPuff: { value: null as THREE.Texture | null },
      uLit: { value: new THREE.Color("#b9a07c") },
      uShade: { value: new THREE.Color("#6d5d4a") },
      uSunDir: { value: new THREE.Vector3(0, 1, 0) },
    },
  ]);
  uniforms.tPuff!.value = puffTexture();
  const mat = new THREE.ShaderMaterial({
    uniforms,
    fog: true,
    transparent: true,
    depthWrite: false,
    vertexShader: `attribute vec3 iPos; attribute float iSize; attribute float iAlpha; attribute float iRot; attribute float iLit;
uniform vec3 uSunDir;
varying vec2 vUv; varying float vA; varying float vUp; varying float vLit; varying float vFwd;
#include <fog_pars_vertex>
void main() {
  vec4 mvPosition = modelViewMatrix * vec4(iPos, 1.0);
  float c = cos(iRot), s = sin(iRot);
  vec2 q = position.xy;
  q = vec2(c * q.x - s * q.y, s * q.x + c * q.y);
  mvPosition.xy += q * iSize;
  gl_Position = projectionMatrix * mvPosition;
  vUv = uv;
  vA = iAlpha;
  vUp = position.y + 0.5;
  vLit = iLit;
  // Dust scatters sunlight forwards: it glows when the camera looks towards the sun through it.
  vec3 wp = (modelMatrix * vec4(iPos, 1.0)).xyz;
  vFwd = pow(max(0.0, dot(normalize(wp - cameraPosition), uSunDir)), 5.0);
  #include <fog_vertex>
}`,
    fragmentShader: `uniform sampler2D tPuff; uniform vec3 uLit; uniform vec3 uShade;
varying vec2 vUv; varying float vA; varying float vUp; varying float vLit; varying float vFwd;
#include <fog_pars_fragment>
void main() {
  float a = texture2D(tPuff, vUv).a * vA;
  if (a < 0.003) discard;
  // Lit on top, in its own shade underneath.
  vec3 col = mix(uShade, uLit * (1.0 + 2.2 * vFwd), smoothstep(0.1, 0.9, vUp) * vLit);
  gl_FragColor = vec4(col, a);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
  #include <fog_fragment>
}`,
  });
  const mesh = new THREE.Mesh(geo, mat);
  mesh.frustumCulled = false;
  mesh.renderOrder = 5;

  // Particle state.
  const P = new Float32Array(max * 3);
  const Vv = new Float32Array(max * 3);
  const age = new Float32Array(max);
  const life = new Float32Array(max).fill(-1);
  const s0 = new Float32Array(max);
  const gr = new Float32Array(max);
  const a0 = new Float32Array(max);
  const rot = new Float32Array(max);
  const spin = new Float32Array(max);
  const lit = new Float32Array(max);
  let head = 0;
  let seed = 12345;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const wind = new THREE.Vector3(1.1, 0, 0.5);

  const dust: Dust = {
    mesh,
    wind,
    puff(at, o = {}) {
      const n = o.n ?? 1;
      const sc = o.scatter ?? 0.6;
      const sp = o.spread ?? 0.5;
      for (let k = 0; k < n; k++) {
        const i = head;
        head = (head + 1) % max;
        P[i * 3] = at.x + (rnd() - 0.5) * sc * 2;
        P[i * 3 + 1] = at.y + (rnd() - 0.5) * sc;
        P[i * 3 + 2] = at.z + (rnd() - 0.5) * sc * 2;
        Vv[i * 3] = (o.vel?.x ?? 0) + (rnd() - 0.5) * sp * 2;
        Vv[i * 3 + 1] = (o.vel?.y ?? 0.4) + rnd() * sp;
        Vv[i * 3 + 2] = (o.vel?.z ?? 0) + (rnd() - 0.5) * sp * 2;
        age[i] = 0;
        life[i] = (o.life ?? 6) * (0.75 + rnd() * 0.5);
        s0[i] = (o.size ?? 2.5) * (0.8 + rnd() * 0.4);
        gr[i] = (o.grow ?? 8) * (0.7 + rnd() * 0.6);
        a0[i] = (o.alpha ?? 0.14) * (0.7 + rnd() * 0.6);
        rot[i] = rnd() * Math.PI * 2;
        spin[i] = (rnd() - 0.5) * 0.3;
        lit[i] = o.lit ?? 1;
      }
    },
    update(dt) {
      const drag = Math.exp(-dt * 0.9);
      const pa = iPos.array as Float32Array;
      const sa = iSize.array as Float32Array;
      const aa = iAlpha.array as Float32Array;
      const ra = iRot.array as Float32Array;
      const la = iLit.array as Float32Array;
      for (let i = 0; i < max; i++) {
        if (life[i]! < 0) {
          sa[i] = 0;
          aa[i] = 0;
          continue;
        }
        age[i] = age[i]! + dt;
        const k = age[i]! / life[i]!;
        if (k >= 1) {
          life[i] = -1;
          sa[i] = 0;
          aa[i] = 0;
          continue;
        }
        const j = i * 3;
        Vv[j] = Vv[j]! * drag + wind.x * (1 - drag);
        Vv[j + 1] = Vv[j + 1]! * drag + 0.25 * (1 - drag);
        Vv[j + 2] = Vv[j + 2]! * drag + wind.z * (1 - drag);
        P[j] = P[j]! + Vv[j]! * dt;
        P[j + 1] = P[j + 1]! + Vv[j + 1]! * dt;
        P[j + 2] = P[j + 2]! + Vv[j + 2]! * dt;
        pa[j] = P[j]!;
        pa[j + 1] = P[j + 1]!;
        pa[j + 2] = P[j + 2]!;
        sa[i] = s0[i]! + gr[i]! * Math.sqrt(k);
        // Quick to appear, slow to thin out.
        aa[i] = a0[i]! * Math.min(1, k / 0.08) * (1 - k) ** 1.6;
        rot[i] = rot[i]! + spin[i]! * dt;
        ra[i] = rot[i]!;
        la[i] = lit[i]!;
      }
      iPos.needsUpdate = true;
      iSize.needsUpdate = true;
      iAlpha.needsUpdate = true;
      iRot.needsUpdate = true;
      iLit.needsUpdate = true;
    },
    light(sun, sunI, sky, skyI, ground, sunDir, lamp = 0) {
      if (sunDir) uniforms.uSunDir!.value.copy(sunDir).normalize();
      // Dust is pale and scatters forward; its lit side takes the sun, its shade the sky and the ground.
      const base = new THREE.Color("#c4ad8a");
      uniforms.uLit!.value.copy(base).multiply(new THREE.Color().copy(sky).multiplyScalar(skyI * 0.9).add(new THREE.Color().copy(sun).multiplyScalar(sunI * 0.16)));
      uniforms.uShade!.value.copy(base).multiply(new THREE.Color().copy(sky).multiplyScalar(skyI * 0.55).add(new THREE.Color().copy(ground).multiplyScalar(skyI * 0.35)));
      if (lamp > 0) {
        const glow = new THREE.Color("#ffd8a2").multiply(base).multiplyScalar(lamp * 0.55);
        uniforms.uLit!.value.add(glow);
        uniforms.uShade!.value.add(glow);
      }
    },
  };
  return dust;
}
