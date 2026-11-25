/**
 * The orebody and its stopes: a translucent lens of ore, the stope solids cut from it between levels, the planned
 * outlines, the drilled ring fans, the paste-filled blocks, and one fired stope as its cavity-monitoring scan sees it,
 * coloured by overbreak and underbreak against the design. Volumes and tonnes are measured off the same meshes the
 * figure draws, so every number on the stope card matches the picture.
 */
import * as THREE from "three";
import { colourHeap, heapMaterial, makeRocks, Noise3, noiseTexture } from "./fleet-kit";
import { clamp, smooth } from "./lib";
import type { Solid } from "./underground-rock";
import { DENSITY, ORE_RANGE, oreThick, zFW, zHW, type Stope } from "./underground-plan";

/* ---------- The lens ---------- */

/** A closed lens between the footwall and hanging-wall contacts (they meet where the ore pinches out). */
export function lensGeometry(nu = 70, nv = 56) {
  const { x0, x1, top, bottom } = ORE_RANGE;
  const pos: number[] = [];
  const idx: number[] = [];
  const sheet = (side: 1 | -1) => {
    const base = pos.length / 3;
    for (let j = 0; j <= nv; j++)
      for (let i = 0; i <= nu; i++) {
        const x = x0 + ((x1 - x0) * i) / nu;
        const y = bottom + ((top - bottom) * j) / nv;
        pos.push(x, y, side > 0 ? zFW(x, y) : zHW(x, y));
      }
    for (let j = 0; j < nv; j++)
      for (let i = 0; i < nu; i++) {
        const a = base + j * (nu + 1) + i;
        const b = a + 1;
        const c = a + nu + 1;
        const d = c + 1;
        if (side > 0) idx.push(a, b, c, b, d, c);
        else idx.push(a, c, b, b, c, d);
      }
  };
  sheet(1);
  sheet(-1);
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  g.setIndex(idx);
  g.computeVertexNormals();
  return g;
}

/** Translucent ore with a bright rim, so the lens reads as a volume and the stopes inside it show through. */
export function lensMaterial() {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    side: THREE.DoubleSide,
    fog: true,
    uniforms: THREE.UniformsUtils.merge([
      THREE.UniformsLib.fog,
      { uColour: { value: new THREE.Color("#e2a640") }, uAlpha: { value: 1 }, tNoise: { value: noiseTexture() } },
    ]),
    vertexShader: `varying vec3 vN;
varying vec3 vV;
varying vec3 vW;
#include <common>
#include <fog_pars_vertex>
void main() {
  vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
  vN = normalize(normalMatrix * normal);
  vV = -mvPosition.xyz;
  vW = (modelMatrix * vec4(position, 1.0)).xyz;
  gl_Position = projectionMatrix * mvPosition;
  #include <fog_vertex>
}`,
    fragmentShader: `uniform vec3 uColour;
uniform float uAlpha;
uniform sampler2D tNoise;
varying vec3 vN;
varying vec3 vV;
varying vec3 vW;
#include <common>
#include <fog_pars_fragment>
void main() {
  float f = 1.0 - abs(dot(normalize(vN), normalize(vV)));
  vec4 n = texture2D(tNoise, vW.xy * 0.02);
  float grade = 0.75 + 0.5 * n.r;
  // Fades out close to the camera, so in a close-up the lens's edge does not cut across the tunnels.
  float a = (0.07 + pow(f, 3.0) * 0.5) * grade * smoothstep(45.0, 170.0, length(vV));
  vec3 c = uColour * (0.8 + 0.5 * pow(f, 2.0)) * grade;
  gl_FragColor = vec4(c, a * uAlpha);
  #include <fog_fragment>
}`,
  });
}

/* ---------- Stope solids ---------- */

/** Which face of a stope: footwall, hanging wall, west, east, floor, back. */
type Face = "fw" | "hw" | "w" | "e" | "floor" | "back";

export interface StopeShape {
  geometry: THREE.BufferGeometry;
  volume: number;
  /** For a scanned stope: metres of overbreak (+) and underbreak (−) at each vertex, and the shell volumes. */
  over?: number;
  under?: number;
  hwElos?: number;
}

/**
 * The stope as a hexahedron that follows the ore contacts: x from xa to xb, y from the extraction level's floor to
 * the drill level's, z from the footwall to the hanging wall. `disp` moves each face point along its outward
 * direction (a scan's overbreak); `inward` turns the faces to look into the opening.
 */
export function stopeShape(s: Stope, o: { inset?: number; disp?: (f: Face, x: number, y: number, z: number) => number; inward?: boolean; heat?: boolean } = {}): StopeShape {
  const inset = o.inset ?? 0;
  const xa = s.xa + inset;
  const xb = s.xb - inset;
  const ya = s.level + inset * 0.2;
  const yb = s.level + 25 - inset;
  const zf = (x: number, y: number) => zFW(x, y) - inset;
  const zh = (x: number, y: number) => zHW(x, y) + inset;
  const pos: number[] = [];
  const col: number[] = [];
  const dv: number[] = [];
  const idx: number[] = [];
  const c = new THREE.Color();
  const P = new THREE.Vector3();
  const NX = 10;
  const NY = 12;
  const NZ = 6;
  /** Grid on one face: (a, b) in [0,1]² mapped to a point and its outward direction. */
  const face = (f: Face, na: number, nb: number, at: (a: number, b: number) => [number, number, number], out: [number, number, number], flip: boolean) => {
    const base = pos.length / 3;
    for (let j = 0; j <= nb; j++)
      for (let i = 0; i <= na; i++) {
        const [x, y, z] = at(i / na, j / nb);
        const d = o.disp ? o.disp(f, x, y, z) : 0;
        P.set(x + out[0] * d, y + out[1] * d, z + out[2] * d);
        pos.push(P.x, P.y, P.z);
        dv.push(d);
        if (o.heat) {
          // Underbreak sky blue, on-design grey-green, overbreak coral to red-orange.
          if (d < -0.3) c.set("#6cc8f7").lerp(new THREE.Color("#2f7fd6"), clamp((-d - 0.3) / 1.4, 0, 1));
          else if (d > 0.3) c.set("#fb9a7a").lerp(new THREE.Color("#e2412f"), clamp((d - 0.3) / 1.6, 0, 1));
          else c.set("#97a69a");
          col.push(c.r, c.g, c.b);
        } else col.push(1, 1, 1);
      }
    for (let j = 0; j < nb; j++)
      for (let i = 0; i < na; i++) {
        const a = base + j * (na + 1) + i;
        const b = a + 1;
        const cc = a + na + 1;
        const d = cc + 1;
        if (flip !== !!o.inward) idx.push(a, cc, b, b, cc, d);
        else idx.push(a, b, cc, b, d, cc);
      }
  };
  const X = (a: number) => xa + (xb - xa) * a;
  const Yv = (b: number) => ya + (yb - ya) * b;
  face("fw", NX, NY, (a, b) => [X(a), Yv(b), zf(X(a), Yv(b))], [0, 0, 1], false);
  face("hw", NX, NY, (a, b) => [X(a), Yv(b), zh(X(a), Yv(b))], [0, 0, -1], true);
  // The grid runs from footwall to hanging wall (−z) on these four, which turns their winding the other way.
  face("w", NZ, NY, (a, b) => [xa, Yv(b), zf(xa, Yv(b)) + (zh(xa, Yv(b)) - zf(xa, Yv(b))) * a], [-1, 0, 0], true);
  face("e", NZ, NY, (a, b) => [xb, Yv(b), zf(xb, Yv(b)) + (zh(xb, Yv(b)) - zf(xb, Yv(b))) * a], [1, 0, 0], false);
  face("floor", NX, NZ, (a, b) => [X(a), ya, zf(X(a), ya) + (zh(X(a), ya) - zf(X(a), ya)) * b], [0, -1, 0], true);
  face("back", NX, NZ, (a, b) => [X(a), yb, zf(X(a), yb) + (zh(X(a), yb) - zf(X(a), yb)) * b], [0, 1, 0], false);
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute("color", new THREE.Float32BufferAttribute(col, 3));
  g.setIndex(idx);
  g.computeVertexNormals();
  // Volume by the divergence theorem, and the over- and underbreak shells (area × depth).
  let vol = 0;
  let over = 0;
  let under = 0;
  let hwArea = 0;
  let hwOver = 0;
  const A = new THREE.Vector3();
  const Bv = new THREE.Vector3();
  const C = new THREE.Vector3();
  for (let i = 0; i < idx.length; i += 3) {
    const a = idx[i]!;
    const b = idx[i + 1]!;
    const cc = idx[i + 2]!;
    A.fromArray(pos, a * 3);
    Bv.fromArray(pos, b * 3);
    C.fromArray(pos, cc * 3);
    vol += A.dot(new THREE.Vector3().crossVectors(Bv, C)) / 6;
    const area = new THREE.Vector3().crossVectors(Bv.clone().sub(A), C.clone().sub(A)).length() / 2;
    const d = (dv[a]! + dv[b]! + dv[cc]!) / 3;
    if (d > 0) over += area * d;
    else under -= area * d;
    // The hanging wall's faces: the second NY×NX block of triangles.
    const hwStart = NX * NY * 6;
    if (i >= hwStart && i < hwStart * 2) {
      hwArea += area;
      hwOver += area * Math.max(0, d);
    }
  }
  return { geometry: g, volume: Math.abs(vol), over, under, hwElos: hwArea ? hwOver / hwArea : 0 };
}

export const tonnes = (v: number) => v * DENSITY;

/** A stope's 12 edges as line segments, following the curved contacts. */
export function stopeEdges(s: Stope, inset = 0.05) {
  const xa = s.xa + inset;
  const xb = s.xb - inset;
  const ya = s.level + inset;
  const yb = s.level + 25 - inset;
  const out: number[] = [];
  const line = (f: (t: number) => [number, number, number], n: number) => {
    for (let i = 0; i < n; i++) out.push(...f(i / n), ...f((i + 1) / n));
  };
  for (const y of [ya, yb])
    for (const side of [zFW, zHW]) line((t) => [xa + (xb - xa) * t, y, side(xa + (xb - xa) * t, y)], 10);
  for (const x of [xa, xb])
    for (const side of [zFW, zHW]) line((t) => [x, ya + (yb - ya) * t, side(x, ya + (yb - ya) * t)], 10);
  for (const x of [xa, xb]) for (const y of [ya, yb]) line((t) => [x, y, zFW(x, y) + (zHW(x, y) - zFW(x, y)) * t], 1);
  return out;
}

/**
 * Long-hole rings for a drilled stope: fans drilled down from the crosscut on the level above, one ring every 2.6 m
 * across the ore, each hole running out to the stope's design boundary.
 */
export function ringLines(s: Stope) {
  const out: number[] = [];
  const xc = (s.xa + s.xb) / 2;
  const yTop = s.level + 25 + 1.2;
  const ya = s.level + 0.6;
  const zf = zFW(xc, s.level + 12);
  const zh = zHW(xc, s.level + 12);
  const half = (s.xb - s.xa) / 2 - 0.3;
  const rings = Math.max(3, Math.floor((zf - zh) / 2.6));
  for (let r = 0; r <= rings; r++) {
    const z = zf - 1.2 - ((zf - zh - 2.4) * r) / rings;
    const n = 11;
    for (let h = 0; h < n; h++) {
      // From well out to the west, through straight down, to well out to the east.
      const phi = ((h - (n - 1) / 2) / ((n - 1) / 2)) * 1.3;
      const dx = Math.sin(phi);
      const dy = -Math.cos(phi);
      let t = (yTop - 0.3 - ya) / -dy;
      if (Math.abs(dx) > 1e-3) t = Math.min(t, half / Math.abs(dx));
      out.push(xc, yTop - 0.3, z, xc + dx * t, yTop - 0.3 + dy * t, z);
    }
  }
  return out;
}

/* ---------- The fired stope ---------- */

const scan = new Noise3(5);
/**
 * What the scan found, in metres along each face's outward direction: a sloughing hanging wall (about a metre, more
 * near the crown), a little footwall overbreak, a rib of ore left on the west end low down, and a near-design floor.
 */
export function scanDisp(s: Stope) {
  const xa = s.xa;
  const xb = s.xb;
  return (f: Face, x: number, y: number, z: number) => {
    const u = (x - xa) / (xb - xa);
    const v = (y - s.level) / 25;
    const n = scan.noise(x * 0.16, y * 0.16, z * 0.16) * 0.55 + scan.noise(x * 0.5, y * 0.5, z * 0.5) * 0.18;
    // Vanishes at the edges of each face so the faces still meet.
    switch (f) {
      case "hw":
        return (1.05 + 0.9 * smooth(v) + n * 1.3) * Math.min(1, Math.sin(Math.PI * clamp(u, 0, 1)) * 2.2) * Math.min(1, Math.sin(Math.PI * v) * 2.5);
      case "fw":
        return (0.32 + n * 0.6) * Math.min(1, Math.sin(Math.PI * clamp(u, 0, 1)) * 2) * Math.min(1, Math.sin(Math.PI * v) * 2.5);
      case "w": {
        const rib = Math.exp(-(((v - 0.22) / 0.16) ** 2)) * 1.9;
        return (0.25 + n * 0.5 - rib) * Math.min(1, Math.sin(Math.PI * v) * 2.5) * Math.min(1, Math.sin(Math.PI * ((z - zFW(x, y)) / (zHW(x, y) - zFW(x, y)))) * 2.5);
      }
      case "e":
        return (0.4 + n * 0.5) * Math.min(1, Math.sin(Math.PI * v) * 2.5) * Math.min(1, Math.sin(Math.PI * ((z - zFW(x, y)) / (zHW(x, y) - zFW(x, y)))) * 2.5);
      case "back":
        return (0.6 + n * 0.7) * Math.min(1, Math.sin(Math.PI * u) * 2.5);
      case "floor":
        return n * 0.15;
    }
  };
}

/** The open stope as a solid that cuts the tunnels meeting it (its brows and drawpoint). */
export function stopeSolid(s: Stope): Solid {
  const d = scanDisp(s);
  const box = new THREE.Box3(new THREE.Vector3(s.xa - 2, s.level - 0.4, -200), new THREE.Vector3(s.xb + 2, s.level + 26.5, 200));
  return {
    rank: 6,
    box,
    inside(p) {
      if (p.x < s.xa - 0.2 || p.x > s.xb + 0.2 || p.y < s.level - 0.2 || p.y > s.level + 25 + Math.max(0, d("back", p.x, s.level + 25, p.z))) return false;
      const f = zFW(p.x, p.y) + Math.max(0, d("fw", p.x, p.y, p.z));
      const h = zHW(p.x, p.y) - Math.max(0, d("hw", p.x, p.y, p.z));
      return p.z < f + 0.1 && p.z > h - 0.1;
    },
  };
}

/** Broken ore in the bottom of the fired stope, spilling through the drawpoint into the crosscut. */
export function stopeMuck(s: Stope, xcX: number, brow: number, detail = 2) {
  const g = new THREE.Group();
  const ya = s.level;
  const xc = (s.xa + s.xb) / 2;
  const zf = zFW(xc, ya + 4);
  const zh = zHW(xc, ya + 4);
  const W = s.xb - s.xa - 1.2;
  const D = zf - zh + 9;
  const n = new Noise3(9);
  const geo = new THREE.PlaneGeometry(W, D, 44, 60);
  geo.rotateX(-Math.PI / 2);
  geo.translate(xc, ya, (zf + zh) / 2 + 4.5);
  const p = geo.getAttribute("position");
  const height = (x: number, z: number) => {
    const inStope = z < zFW(x, ya + 3) - 0.6 && z > zHW(x, ya + 3) + 0.6 && x > s.xa + 0.8 && x < s.xb - 0.8;
    const inXc = Math.abs(x - xcX) < 2.2 && z >= zFW(x, ya + 3) - 0.6 && z < brow + 4.5;
    if (!inStope && !inXc) return -1;
    // Highest against the hanging wall, at its angle of repose down to the brow; a tongue into the drawpoint.
    const fromHw = (z - zHW(x, ya + 4)) / (zFW(x, ya + 4) - zHW(x, ya + 4));
    let h = 8.5 * (1 - smooth(fromHw * 0.95)) + 1.2;
    if (inXc) h = Math.max(0, 1.6 - (z - zFW(x, ya + 3)) * 0.42) * (1 - ((x - xcX) / 2.4) ** 2);
    h += n.fbm(x * 0.3, 1, z * 0.3, 3) * 0.7;
    const edge = inStope ? Math.min(1, (x - s.xa - 0.8) / 2, (s.xb - 0.8 - x) / 2) : 1;
    return Math.max(0.05, h * clamp(edge, 0.25, 1));
  };
  for (let i = 0; i < p.count; i++) p.setY(i, ya + height(p.getX(i), p.getZ(i)));
  // Drop triangles that are entirely outside the opening.
  const index = geo.getIndex()!;
  const keep: number[] = [];
  for (let i = 0; i < index.count; i += 3) {
    const a = index.getX(i);
    const b = index.getX(i + 1);
    const c = index.getX(i + 2);
    if (p.getY(a) < ya && p.getY(b) < ya && p.getY(c) < ya) continue;
    keep.push(a, b, c);
  }
  for (let i = 0; i < p.count; i++) if (p.getY(i) < ya) p.setY(i, ya + 0.02);
  geo.setIndex(keep);
  geo.computeVertexNormals();
  colourHeap(geo, ["#4a4139", "#5c5045", "#3e3731", "#6a5a48", "#54483c"], 3, 1.2);
  const heap = new THREE.Mesh(geo, heapMaterial("ug-muck"));
  heap.castShadow = heap.receiveShadow = true;
  g.add(heap);
  const rocks = makeRocks({
    count: 140,
    seed: 31,
    detail,
    palette: ["#4f4842", "#5a524b", "#463f39", "#6b5a45", "#3f3a35"],
    place: (_i, r) => {
      for (let k = 0; k < 20; k++) {
        const x = s.xa + 1 + r() * (s.xb - s.xa - 2);
        const z = zh + 1 + r() * (brow + 3 - zh);
        const h = height(x, z);
        if (h > 0.1) return { x, y: ya + h + 0.05, z, size: 0.25 + r() ** 2.2 * 0.9 };
      }
      return { x: xc, y: ya + 0.2, z: zf, size: 0.3 };
    },
  });
  g.add(rocks.group);
  return { group: g, height: (x: number, z: number) => ya + Math.max(0, height(x, z)) };
}

/** Material for the inside of the scanned opening: dark broken rock, or the scan's colours. */
export function cavityMaterial(heat: { value: number }) {
  const m = new THREE.MeshStandardMaterial({ color: "#5d554c", roughness: 0.95, vertexColors: false });
  const tex = noiseTexture();
  m.customProgramCacheKey = () => "ug-cavity-v1";
  m.onBeforeCompile = (sh) => {
    sh.uniforms.uHeat = heat;
    sh.uniforms.tNoise = { value: tex };
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nattribute vec3 color;\nvarying vec3 vHeat;\nvarying vec3 vCw;")
      .replace("#include <worldpos_vertex>", "#include <worldpos_vertex>\nvHeat = color;\nvCw = (modelMatrix * vec4(transformed, 1.0)).xyz;");
    sh.fragmentShader = sh.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform float uHeat;\nuniform sampler2D tNoise;\nvarying vec3 vHeat;\nvarying vec3 vCw;")
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec4 nn = texture2D(tNoise, vCw.xy * 0.21 + vCw.z * 0.13);
vec4 nb = texture2D(tNoise, vCw.zy * 0.05);
diffuseColor.rgb *= 0.7 + 0.5 * nn.b * nb.r + 0.2 * nb.g;
diffuseColor.rgb = mix(diffuseColor.rgb, vHeat * (0.85 + 0.25 * nn.b), uHeat);`,
      )
      .replace("#include <emissivemap_fragment>", "#include <emissivemap_fragment>\ntotalEmissiveRadiance += diffuseColor.rgb * (0.05 + 0.32 * uHeat);");
  };
  return m;
}

/** Cured paste: pale grey with the pour's layering; curing paste is darker and wet. */
export function pasteMaterial(wet: boolean) {
  const m = new THREE.MeshStandardMaterial({ color: wet ? "#8e887d" : "#b8b2a6", roughness: wet ? 0.42 : 0.9 });
  const tex = noiseTexture();
  m.customProgramCacheKey = () => `ug-paste-${wet}`;
  m.onBeforeCompile = (sh) => {
    sh.uniforms.tNoise = { value: tex };
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vPw;")
      .replace("#include <worldpos_vertex>", "#include <worldpos_vertex>\nvPw = (modelMatrix * vec4(transformed, 1.0)).xyz;");
    sh.fragmentShader = sh.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform sampler2D tNoise;\nvarying vec3 vPw;")
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec4 nn = texture2D(tNoise, vec2(vPw.x * 0.03 + vPw.z * 0.03, vPw.y * 0.7));
diffuseColor.rgb *= 0.86 + 0.2 * nn.r;
diffuseColor.rgb *= 0.94 + 0.06 * sin(vPw.y * 2.6 + nn.g * 3.0);`,
      )
      .replace("#include <emissivemap_fragment>", "#include <emissivemap_fragment>\ntotalEmissiveRadiance += diffuseColor.rgb * 0.06;");
  };
  return m;
}

export { oreThick };
