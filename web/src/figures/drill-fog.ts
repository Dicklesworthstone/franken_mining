/**
 * Uncertainty as a fog in the ground: the block model's predictive spread, one value per block in a small 3D
 * texture, ray-marched through the prospect's volume. Where nothing is known the fog is thick and violet; along
 * each drillhole it clears to a tunnel, and the tunnel's wall glows where knowledge gives way to guesswork.
 *
 * The fog draws the volume's back faces with the depth test on, so anything solid in or in front of it (the
 * holes, the rig) is never fogged. Nothing here touches the DOM at import time.
 */
import * as THREE from "three";
import { B, NX, NY, NZ, TOP, X0, Z0 } from "./drill-model";

export interface Fog {
  mesh: THREE.Mesh;
  /** Sets the field from each block's uncertainty, 0 (known) to 1 (as unknown as the prior). */
  set(u: ArrayLike<number>): void;
  /** Keeps only the slab |n·p − d| ≤ w (a section), or the whole volume when `n` is null. */
  slab(n: THREE.Vector3 | null, d: number, w: number): void;
}

const vertexShader = /* glsl */ `
varying vec3 vW;
void main() {
  vec4 w = modelMatrix * vec4(position, 1.0);
  vW = w.xyz;
  gl_Position = projectionMatrix * viewMatrix * w;
}`;

const fragmentShader = /* glsl */ `
precision highp sampler3D;
uniform sampler3D field;
uniform vec3 bmin;
uniform vec3 bmax;
uniform float steps;
uniform float sigma;
uniform float rim;
uniform float lo;
uniform float hi;
uniform float pw;
uniform float known;
uniform vec4 slabP;
uniform float slabW;
uniform vec3 cKnown;
uniform vec3 cLow;
uniform vec3 cMid;
uniform vec3 cHigh;
uniform vec3 cRim;
varying vec3 vW;
void main() {
  vec3 o = cameraPosition;
  vec3 d = normalize(vW - o);
  vec3 inv = 1.0 / d;
  vec3 t0 = (bmin - o) * inv;
  vec3 t1 = (bmax - o) * inv;
  vec3 tn = min(t0, t1);
  vec3 tf = max(t0, t1);
  float a = max(max(max(tn.x, tn.y), tn.z), 0.0);
  float b = min(min(tf.x, tf.y), tf.z);
  if (a >= b) discard;
  float ds = (b - a) / steps;
  float j = fract(sin(dot(gl_FragCoord.xy, vec2(12.9898, 78.233))) * 43758.5453);
  vec3 size = bmax - bmin;
  vec3 col = vec3(0.0);
  float T = 1.0;
  for (int i = 0; i < 160; i++) {
    if (float(i) >= steps || T < 0.03) break;
    vec3 p = o + d * (a + (float(i) + j) * ds);
    vec3 q = (p - bmin) / size;
    if (slabW > 0.0 && abs(dot(p, slabP.xyz) - slabP.w) > slabW) continue;
    vec2 f = texture(field, vec3(q.x, q.z, 1.0 - q.y)).rg;
    float u = f.r;
    float haze = pow(smoothstep(lo, hi, u), pw);
    float k = known * (1.0 - smoothstep(0.2, 0.8, u));
    float dens = sigma * (haze + 0.35 * rim * f.g + 0.6 * k);
    float al = 1.0 - exp(-dens * ds);
    vec3 c = mix(mix(cLow, cMid, smoothstep(0.3, 0.8, u)), cHigh, smoothstep(0.8, 1.0, u)) * haze + cRim * f.g * rim + cKnown * k;
    c /= max(haze + 0.35 * rim * f.g + 0.6 * k, 1e-3);
    col += T * al * c;
    T *= 1.0 - al;
  }
  gl_FragColor = vec4(col, 1.0 - T);
  #include <colorspace_fragment>
}`;

export function makeFog(o: { steps?: number } = {}): Fog {
  const n = NX * NZ * NY;
  const data = new Uint8Array(n * 2);
  const tex = new THREE.Data3DTexture(data, NX, NZ, NY);
  tex.format = THREE.RGFormat;
  tex.type = THREE.UnsignedByteType;
  tex.minFilter = tex.magFilter = THREE.LinearFilter;
  tex.wrapS = tex.wrapT = tex.wrapR = THREE.ClampToEdgeWrapping;
  tex.unpackAlignment = 1;
  tex.needsUpdate = true;
  const inset = 0.6;
  const bmin = new THREE.Vector3(X0 + inset, TOP - NY * B + inset, Z0 + inset);
  const bmax = new THREE.Vector3(X0 + NX * B - inset, TOP - inset, Z0 + NZ * B - inset);
  const size = bmax.clone().sub(bmin);
  const geo = new THREE.BoxGeometry(size.x, size.y, size.z);
  geo.translate((bmin.x + bmax.x) / 2, (bmin.y + bmax.y) / 2, (bmin.z + bmax.z) / 2);
  const lin = (hex: string) => new THREE.Color(hex);
  const mat = new THREE.ShaderMaterial({
    vertexShader,
    fragmentShader,
    uniforms: {
      field: { value: tex },
      bmin: { value: bmin },
      bmax: { value: bmax },
      steps: { value: o.steps ?? 96 },
      // Optical depth per metre where nothing is known: about two across the volume.
      sigma: { value: 2.6 / 420 },
      rim: { value: 2.2 },
      lo: { value: 0.5 },
      hi: { value: 1 },
      pw: { value: 1.2 },
      known: { value: 0 },
      slabP: { value: new THREE.Vector4() },
      slabW: { value: 0 },
      cKnown: { value: lin("#7fe3ff").multiplyScalar(1.2) },
      cLow: { value: lin("#1d2a6b").multiplyScalar(0.6) },
      cMid: { value: lin("#4a33b4").multiplyScalar(0.75) },
      cHigh: { value: lin("#8f68f0").multiplyScalar(0.8) },
      cRim: { value: lin("#9fdcff").multiplyScalar(1.3) },
    },
    side: THREE.BackSide,
    transparent: true,
    depthWrite: false,
    depthTest: true,
    blending: THREE.CustomBlending,
    blendSrc: THREE.OneFactor,
    blendDst: THREE.OneMinusSrcAlphaFactor,
    fog: false,
    toneMapped: false,
  });
  const mesh = new THREE.Mesh(geo, mat);
  mesh.renderOrder = 3;
  mesh.frustumCulled = false;
  const grid = new Float32Array(n);
  const at = (i: number, j: number, k: number) =>
    grid[Math.min(NX - 1, Math.max(0, i)) + NX * (Math.min(NZ - 1, Math.max(0, j)) + NZ * Math.min(NY - 1, Math.max(0, k)))]!;
  return {
    mesh,
    slab(nv, d, w) {
      if (nv) mat.uniforms.slabP!.value.set(nv.x, nv.y, nv.z, d);
      mat.uniforms.slabW!.value = nv ? w : 0;
    },
    set(u) {
      for (let b = 0; b < n; b++) grid[b] = Math.min(1, Math.max(0, u[b]!));
      for (let k = 0; k < NY; k++)
        for (let j = 0; j < NZ; j++)
          for (let i = 0; i < NX; i++) {
            const b = i + NX * (j + NZ * k);
            const gx = at(i + 1, j, k) - at(i - 1, j, k);
            const gz = at(i, j + 1, k) - at(i, j - 1, k);
            const gy = at(i, j, k + 1) - at(i, j, k - 1);
            // The wall of a tunnel: a steep change, on the known side of half-way.
            const edge = Math.min(1, Math.hypot(gx, gy, gz) * 1.6) * (grid[b]! < 0.75 ? 1 : 0.35);
            data[b * 2] = Math.round(grid[b]! * 255);
            data[b * 2 + 1] = Math.round(edge * 255);
          }
      tex.needsUpdate = true;
    },
  };
}
