/**
 * The ground for the Plan scene: a desert to the horizon with the solved pit cut into it as benches and berms, and a
 * cutaway in front of the section line, whose rock walls show the strata.
 *
 * The pit's shape comes from the engine's block set. Each column's mined benches give a depth on the 20 m block
 * grid, smoothed a little and sampled smoothly onto a fine grid. That smooth depth is then terraced: per 15 m bench
 * a flat berm and a planar 72° face, the berm as wide as the overall slope leaves beside the face. So the crest of
 * each bench follows the edge of the blocks mined below it, and the walls stand at the slope the engine was given.
 *
 * The crests and toes are sharp: every fine-grid vertex within half a cell of a crest or toe line moves onto it
 * (along the slope; along the wall where it is on the cutaway's edge), so the mesh's edges run along the design
 * strings rather than across them, and in the pit the faces are lit flat. The same lines come out as the crest and
 * toe strings of the engineering view.
 *
 * Changing the pit morphs the fine grid from the old shape to the new one; the cutaway's walls follow.
 */
import * as THREE from "three";
import { clamp, mix, smooth } from "./lib";
import { B, CUT_X0, CUT_X1, CUT_Y, CUT_Z0, CUT_Z1, H, NX, NY, NZ, TOP, X0, Z0 } from "./pit-model";
import { noise2 } from "./stage";
import { rockSurfaceMaterial, strata } from "./terrain";

/** Detailed square around the block model. */
const GX0 = -480;
const GX1 = 480;
const GZ0 = -420;
const GZ1 = 420;
/** The bench face angle the berms are sized against. */
export const FACE_DEG = 72;
const FACE = THREE.MathUtils.degToRad(FACE_DEG);

/** Berm width, in metres, beside a 72° face for a 15 m bench at an overall slope. */
export const bermWidth = (slopeDeg: number) => H / Math.tan(THREE.MathUtils.degToRad(slopeDeg)) - H / Math.tan(FACE);

/* ---------- Colours ---------- */
const LUT_D = 361;
const LUT_V = 16;
/** strata(depth, v) for whole metres and sixteen variants, so the per-frame colouring allocates nothing. */
const lut = (() => {
  const out = new Float32Array(LUT_D * LUT_V * 3);
  const c = new THREE.Color();
  for (let d = 0; d < LUT_D; d++)
    for (let v = 0; v < LUT_V; v++) {
      strata(d, (v + 0.5) / LUT_V, c);
      out.set([c.r, c.g, c.b], (d * LUT_V + v) * 3);
    }
  return out;
})();
const DUST = new THREE.Color("#c4b294");
const DAMP = new THREE.Color("#544a40");
const SCREE = new THREE.Color("#6e6155");
const SOILS = ["#a8906d", "#94795a", "#b39b7b", "#9d8566", "#8a7458"].map((h) => new THREE.Color(h));
const GRAVEL = new THREE.Color("#6f6152");
const CALICHE = new THREE.Color("#cdbfa3");

function rock(depth: number, v: number, out: Float32Array, o: number) {
  const d = clamp(depth, 0, LUT_D - 1.001);
  const d0 = Math.floor(d);
  const f = d - d0;
  const vi = clamp(Math.floor(v * LUT_V), 0, LUT_V - 1);
  const a = (d0 * LUT_V + vi) * 3;
  const b = ((d0 + 1) * LUT_V + vi) * 3;
  out[o] = mix(lut[a]!, lut[b]!, f);
  out[o + 1] = mix(lut[a + 1]!, lut[b + 1]!, f);
  out[o + 2] = mix(lut[a + 2]!, lut[b + 2]!, f);
}
const lerpInto = (out: Float32Array, o: number, c: THREE.Color, w: number) => {
  out[o] = mix(out[o]!, c.r, w);
  out[o + 1] = mix(out[o + 1]!, c.g, w);
  out[o + 2] = mix(out[o + 2]!, c.b, w);
};

/* ---------- The pit's shape ---------- */
/**
 * Depth below the natural surface from benches mined per block column: `field3` the smooth depth, `terrace` the
 * benched one. `breakAt` names the crest or toe line nearest a smooth depth, for snapping.
 */
export class Shaper {
  /** Column depths in metres, smoothed, padded by two blocks of zero on every side. */
  private field = new Float32Array((NX + 4) * (NZ + 4));
  private tmp = new Float32Array((NX + 4) * (NZ + 4));
  /** Each bench's share of flat ground, for the slope. */
  berm = 0.5;
  /** The pit floor's depth: the deepest whole bench the smooth depth reaches. */
  floor = 0;
  /** Benches the solver's block set reaches (its deepest column). */
  benches = 0;

  set(cols: Uint8Array, slopeDeg: number) {
    const W = NX + 4;
    const f = this.field;
    const t = this.tmp;
    f.fill(0);
    let deepest = 0;
    for (let j = 0; j < NZ; j++)
      for (let i = 0; i < NX; i++) {
        const d = cols[i + NX * j]!;
        f[i + 2 + W * (j + 2)] = d * H;
        if (d > deepest) deepest = d;
      }
    this.benches = deepest;
    // Two passes of a 1-2-1 blur each way: rounds the block corners off without moving the walls, so the benches
    // run in smooth arcs as a designed pit's do.
    for (let pass = 0; pass < 2; pass++) {
      for (let j = 0; j < NZ + 4; j++)
        for (let i = 0; i < W; i++) {
          const l = f[Math.max(0, i - 1) + W * j]!;
          const r = f[Math.min(W - 1, i + 1) + W * j]!;
          t[i + W * j] = (l + 2 * f[i + W * j]! + r) / 4;
        }
      for (let j = 0; j < NZ + 4; j++)
        for (let i = 0; i < W; i++) {
          const u = t[i + W * Math.max(0, j - 1)]!;
          const d = t[i + W * Math.min(NZ + 3, j + 1)]!;
          f[i + W * j] = (u + 2 * t[i + W * j]! + d) / 4;
        }
    }
    let top = 0;
    for (let c = 0; c < f.length; c++) if (f[c]! > top) top = f[c]!;
    this.berm = clamp(bermWidth(slopeDeg) / (H / Math.tan(THREE.MathUtils.degToRad(slopeDeg))), 0.12, 0.85);
    this.floor = deepest === 0 ? 0 : Math.max(H, H * Math.floor(top / H + 1e-6));
  }

  /** The smoothed depth (before terracing) at a point: Catmull-Rom over the column grid, kept within its cell. */
  field3(x: number, z: number) {
    const W = NX + 4;
    const u = (x - X0) / B - 0.5 + 2;
    const v = (z - Z0) / B - 0.5 + 2;
    if (u < 1 || v < 1 || u > NX + 2 || v > NZ + 2) return 0;
    const i = Math.floor(u);
    const j = Math.floor(v);
    const fx = u - i;
    const fz = v - j;
    const f = this.field;
    const at = (a: number, b: number) => f[clamp(a, 0, W - 1) + W * clamp(b, 0, NZ + 3)]!;
    const cr = (p0: number, p1: number, p2: number, p3: number, s: number) =>
      p1 + 0.5 * s * (p2 - p0 + s * (2 * p0 - 5 * p1 + 4 * p2 - p3 + s * (3 * (p1 - p2) + p3 - p0)));
    const row = (b: number) => cr(at(i - 1, b), at(i, b), at(i + 1, b), at(i + 2, b), fx);
    const d = cr(row(j - 1), row(j), row(j + 1), row(j + 2), fz);
    const hi = Math.max(at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
    return clamp(d, 0, hi);
  }

  /** Benched depth for a smoothed depth: a flat berm, then a planar face down to the next bench; flat at the floor. */
  terrace(d: number) {
    if (d <= 0.01 || this.floor <= 0) return 0;
    const u = d / H;
    const k = Math.floor(u);
    const p = clamp((u - k - this.berm) / (1 - this.berm), 0, 1);
    return Math.min(this.floor, H * (k + p));
  }

  /** The crest or toe line (as a smooth depth) nearest `d`, or −1. Crests sit at (k + berm)·H, toes at k·H. */
  breakAt(d: number) {
    if (d <= 0.01 || this.floor <= 0) return -1;
    const k = Math.floor(d / H);
    let best = -1;
    let gap = Infinity;
    const consider = (t: number) => {
      if (t <= 0 || t > this.floor + 1e-6) return;
      const g = Math.abs(t - d);
      if (g < gap) {
        gap = g;
        best = t;
      }
    };
    consider(k * H);
    consider((k + this.berm) * H);
    consider((k + 1) * H);
    return best;
  }

  /** Crest and toe depths down to the floor, for the strings. */
  lines() {
    const crests: number[] = [];
    const toes: number[] = [];
    for (let k = 0; k * H < this.floor - 1e-6; k++) {
      crests.push((k + this.berm) * H);
      toes.push((k + 1) * H);
    }
    return { crests, toes };
  }

  depth(x: number, z: number) {
    return this.terrace(this.field3(x, z));
  }
}

export interface Strings {
  /** Line-segment pairs (x, y, z) along each bench crest and toe, 0.8 m above the ground. */
  crest: Float32Array;
  toe: Float32Array;
}

export interface Ground {
  group: THREE.Group;
  /** The natural ground before any mining. */
  surface(x: number, z: number): number;
  /** The ground as drawn now (mid-morph included). */
  heightAt(x: number, z: number): number;
  /** Morphs to the pit given by column depths (benches mined per column) at a slope, over `seconds`. */
  shape(cols: Uint8Array, slopeDeg: number, seconds: number): void;
  /** Advances the morph; true while it is still moving. */
  update(dt: number): boolean;
  /** The section's top edge (ground or pit) at x along z = CUT_Z0, for overlays: as drawn now. */
  readonly sectionX: Float32Array;
  readonly sectionY: Float32Array;
  /** The block-grade texture painted on the section face: NX × NY texels, rgb the colour, alpha its strength. */
  faceGrades: THREE.DataTexture;
  /** Where the cutaway's walls are, for labels. */
  floorY: number;
  /** Profile of a block set along the section (its benched top edge), at `sectionX`. */
  profile(cols: Uint8Array, slopeDeg: number, out: Float32Array): Float32Array;
  /** The crest and toe strings of the pit shaped last. */
  strings(): Strings;
  /** The floor depth, deepest column in benches, and berm width of the pit shaped last. */
  readonly design: { floor: number; benches: number; berm: number };
  /** 0 for the rendered view, 1 for the engineering view (the ground greys back so the strings read). */
  setEngineering(t: number): void;
  material: THREE.MeshStandardMaterial;
}

export function makeGround(o: { cell: number; seed?: number; surround?: boolean; scatter?: number }): Ground {
  const { fbm } = noise2(o.seed ?? 23);
  const cell = o.cell;
  const cx = Math.round((GX1 - GX0) / cell);
  const cz = Math.round((GZ1 - GZ0) / cell);
  const nxv = cx + 1;
  const nzv = cz + 1;
  const count = nxv * nzv;

  // The natural surface: gentle swells, flatter over the block model.
  const surface = (x: number, z: number) => {
    const r = Math.max(Math.abs(x) - 470, Math.abs(z) - 410, 0);
    const amp = mix(0.5, 1, smooth(r / 400));
    return amp * (3.2 * fbm(x / 420, z / 420, 4) + 0.9 * fbm(x / 90, z / 90, 3)) + 0.5 * fbm(x / 23, z / 23, 2);
  };
  /** Desert soil colour: patches of gravel and pale caliche, dry washes, at several scales. */
  const soil = (x: number, z: number, out: Float32Array, o: number) => {
    const v = clamp(0.5 + 0.9 * fbm(x / 55, z / 55, 3), 0, 0.999) * (SOILS.length - 1);
    const a = SOILS[Math.floor(v)]!;
    const b = SOILS[Math.min(SOILS.length - 1, Math.floor(v) + 1)]!;
    const f = v - Math.floor(v);
    out[o] = mix(a.r, b.r, f);
    out[o + 1] = mix(a.g, b.g, f);
    out[o + 2] = mix(a.b, b.b, f);
    const gravel = smooth((fbm(x / 140 + 7, z / 140 - 3, 4) - 0.08) / 0.3);
    lerpInto(out, o, GRAVEL, gravel * 0.45);
    const pale = smooth((fbm(x / 70 - 5, z / 70 + 9, 3) - 0.18) / 0.22);
    lerpInto(out, o, CALICHE, pale * 0.35);
    // Washes: thin winding lines where a noise crosses zero.
    const w = 1 - Math.abs(fbm(x / 260 + 2, z / 260 + 5, 3)) * 7;
    lerpInto(out, o, CALICHE, smooth(w) * 0.25);
    const fine = 0.94 + 0.12 * fbm(x / 6, z / 6, 2);
    out[o]! *= fine;
    out[o + 1]! *= fine;
    out[o + 2]! *= fine;
  };

  /* ---------- Fixed per-vertex data ---------- */
  const xs = new Float32Array(count);
  const zs = new Float32Array(count);
  const base = new Float32Array(count);
  const vary = new Float32Array(count);
  const breakage = new Float32Array(count);
  const rough = new Float32Array(count);
  /** 1 where a vertex lies on a cutaway edge running along x, 2 along z, 3 at a corner: its snapping is held to it. */
  const edge = new Uint8Array(count);
  const soilCol = new Float32Array(count * 3);
  const on = (a: number, b: number) => Math.abs(a - b) < 1e-3;
  for (let j = 0; j < nzv; j++)
    for (let k = 0; k < nxv; k++) {
      const i = j * nxv + k;
      const x = GX0 + k * cell;
      const z = GZ0 + j * cell;
      xs[i] = x;
      zs[i] = z;
      base[i] = surface(x, z);
      vary[i] = clamp(0.5 + 0.9 * fbm(x / 55, z / 55, 3), 0, 0.999);
      // Overbreak at the crests: the lines wander a metre or so, as blasted crests do.
      breakage[i] = 1.4 * fbm(x / 17 + 4, z / 17 - 2, 3);
      rough[i] = 0.35 * fbm(x / 7, z / 7, 2);
      const inX = x >= CUT_X0 - 1e-3 && x <= CUT_X1 + 1e-3;
      const inZ = z >= CUT_Z0 - 1e-3 && z <= CUT_Z1 + 1e-3;
      if (inX && (on(z, CUT_Z0) || on(z, CUT_Z1))) edge[i]! |= 1;
      if (inZ && (on(x, CUT_X0) || on(x, CUT_X1))) edge[i]! |= 2;
      soil(x, z, soilCol, i * 3);
    }

  /* ---------- Morph state ---------- */
  const depthFrom = new Float32Array(count);
  const depthTo = new Float32Array(count);
  const depthNow = new Float32Array(count);
  const offFrom = new Float32Array(count * 2);
  const offTo = new Float32Array(count * 2);
  const offNow = new Float32Array(count * 2);
  /** Per vertex: rubble below a face (0–1), at the target and now. */
  const screeFrom = new Float32Array(count);
  const screeTo = new Float32Array(count);
  const screeNow = new Float32Array(count);
  /** The smooth depth at the target, for the strings. */
  const smoothTo = new Float32Array(count);
  /** Unit down-slope direction at the target: the texture is unrolled along it, so faces are not streaked. */
  const slopeDir = new Float32Array(count * 2);
  /** The smooth depth before overbreak, per vertex, for the target. */
  const rawAt = new Float32Array(count);
  const shaper = new Shaper();
  let morph = 1;
  let span = 1;
  // The box of vertices the morph touches.
  let box = { k0: 0, k1: cx, j0: 0, j1: cz };

  /* ---------- The detailed mesh ---------- */
  const pos = new Float32Array(count * 3);
  const nrm = new Float32Array(count * 3);
  const col = new Float32Array(count * 3);
  const mined = new Float32Array(count);
  const uv = new Float32Array(count * 2);
  for (let i = 0; i < count; i++) {
    pos[i * 3] = xs[i]!;
    pos[i * 3 + 1] = base[i]!;
    pos[i * 3 + 2] = zs[i]!;
    uv[i * 2] = xs[i]! * 0.12;
    uv[i * 2 + 1] = zs[i]! * 0.12;
  }
  const inCut = (x0: number, x1: number, z0: number, z1: number) =>
    x0 >= CUT_X0 - 1e-6 && x1 <= CUT_X1 + 1e-6 && z0 >= CUT_Z0 - 1e-6 && z1 <= CUT_Z1 + 1e-6;
  // Each quad's place in the index (−1 inside the cutaway), so its diagonal can be chosen again for each pit.
  const quadAt = new Int32Array(cx * cz).fill(-1);
  let quads = 0;
  for (let j = 0; j < cz; j++)
    for (let k = 0; k < cx; k++) {
      const x0 = GX0 + k * cell;
      const z0 = GZ0 + j * cell;
      if (!inCut(x0, x0 + cell, z0, z0 + cell)) quadAt[j * cx + k] = quads++;
    }
  const index = new Uint32Array(quads * 6);
  /** The line (crest or toe, by depth) each vertex was moved onto for the target pit, or −1. */
  const snapped = new Float32Array(count).fill(-1);
  /**
   * Splits a quad along the diagonal that follows the ground: the one joining two vertices on the same crest or
   * toe line if there is one, else the one with the smaller drop, so no triangle cuts across a bench edge.
   */
  const split = (k: number, j: number) => {
    const q = quadAt[j * cx + k]!;
    if (q < 0) return;
    const a = j * nxv + k;
    const b = a + 1;
    const c = a + nxv;
    const d = c + 1;
    const sa = snapped[a]!;
    const sb = snapped[b]!;
    const sc = snapped[c]!;
    const sd = snapped[d]!;
    let ad: boolean;
    if (sa >= 0 && sa === sd) ad = true;
    else if (sb >= 0 && sb === sc) ad = false;
    else ad = Math.abs(depthTo[a]! - depthTo[d]!) <= Math.abs(depthTo[b]! - depthTo[c]!);
    const o = q * 6;
    if (ad) {
      index[o] = a;
      index[o + 1] = c;
      index[o + 2] = d;
      index[o + 3] = a;
      index[o + 4] = d;
      index[o + 5] = b;
    } else {
      index[o] = a;
      index[o + 1] = c;
      index[o + 2] = b;
      index[o + 3] = b;
      index[o + 4] = c;
      index[o + 5] = d;
    }
  };
  for (let j = 0; j < cz; j++) for (let k = 0; k < cx; k++) split(k, j);
  const geo = new THREE.BufferGeometry();
  const posAttr = new THREE.BufferAttribute(pos, 3);
  const nrmAttr = new THREE.BufferAttribute(nrm, 3);
  const colAttr = new THREE.BufferAttribute(col, 3);
  const minedAttr = new THREE.BufferAttribute(mined, 1);
  const uvAttr = new THREE.BufferAttribute(uv, 2);
  for (const a of [posAttr, nrmAttr, colAttr, minedAttr, uvAttr]) a.setUsage(THREE.DynamicDrawUsage);
  geo.setAttribute("position", posAttr);
  geo.setAttribute("normal", nrmAttr);
  geo.setAttribute("color", colAttr);
  geo.setAttribute("aMined", minedAttr);
  geo.setAttribute("uv", uvAttr);
  const indexAttr = new THREE.BufferAttribute(index, 1).setUsage(THREE.DynamicDrawUsage);
  geo.setIndex(indexAttr);
  geo.boundingSphere = new THREE.Sphere(new THREE.Vector3(0, -100, 0), 900);

  const uEng = { value: 0 };
  /**
   * The rock material, with the pit lit flat: where the ground is mined, the shading normal (and the rock shader's
   * test for a face) comes from the triangle itself, so crests and toes stay sharp; the undisturbed desert stays
   * smooth. In the engineering view the ground greys back.
   */
  const pitMaterial = (flat: boolean) => {
    const m = rockSurfaceMaterial();
    const baseCompile = m.onBeforeCompile;
    m.customProgramCacheKey = () => `fm-pit-ground-v2-${flat ? 1 : 0}`;
    m.onBeforeCompile = (shader, renderer) => {
      baseCompile.call(m, shader, renderer);
      shader.uniforms.uEng = uEng;
      if (flat) {
        shader.vertexShader = shader.vertexShader
          .replace("#include <common>", "#include <common>\nattribute float aMined;\nvarying float vMined;")
          .replace("#include <begin_vertex>", "#include <begin_vertex>\nvMined = aMined;");
        shader.fragmentShader = shader.fragmentShader
          .replace("#include <common>", "#include <common>\nvarying float vMined;")
          .replace(
            "vec3 rn = normalize(vRN);",
            "vec3 rn = normalize(mix(normalize(vRN), normalize(cross(dFdx(vRW), dFdy(vRW))), vMined));",
          )
          .replace(
            "#include <normal_fragment_begin>",
            THREE.ShaderChunk.normal_fragment_begin.replace(
              "vec3 normal = normalize( vNormal );",
              "vec3 normal = normalize( vNormal );\n\tnormal = normalize( mix( normal, normalize( cross( dFdx( vViewPosition ), dFdy( vViewPosition ) ) ), vMined ) );",
            ),
          );
      }
      shader.fragmentShader = shader.fragmentShader
        .replace("#include <common>", "#include <common>\nuniform float uEng;")
        .replace(
          "#include <roughnessmap_fragment>",
          `{
  float lum = dot(diffuseColor.rgb, vec3(0.3, 0.59, 0.11));
  diffuseColor.rgb = mix(diffuseColor.rgb, vec3(lum * 0.78 + 0.02), uEng * 0.75);
}
#include <roughnessmap_fragment>`,
        );
    };
    return m;
  };
  const material = pitMaterial(true);
  const mesh = new THREE.Mesh(geo, material);
  mesh.receiveShadow = true;
  mesh.castShadow = true;
  const group = new THREE.Group();
  group.name = "pit-ground";
  group.add(mesh);

  /** Normals and colours for vertices in [k0, k1] × [j0, j1], from the current positions. */
  const shade = (k0: number, k1: number, j0: number, j1: number, floorDepth: number) => {
    for (let j = j0; j <= j1; j++)
      for (let k = k0; k <= k1; k++) {
        const i = j * nxv + k;
        const l = (j * nxv + Math.max(0, k - 1)) * 3;
        const r = (j * nxv + Math.min(cx, k + 1)) * 3;
        const d = (Math.max(0, j - 1) * nxv + k) * 3;
        const u = (Math.min(cz, j + 1) * nxv + k) * 3;
        // The cross product of the grid's two directions through this vertex, with the vertices as they stand.
        const ax = pos[r]! - pos[l]!;
        const ay = pos[r + 1]! - pos[l + 1]!;
        const az = pos[r + 2]! - pos[l + 2]!;
        const bx = pos[u]! - pos[d]!;
        const by = pos[u + 1]! - pos[d + 1]!;
        const bz = pos[u + 2]! - pos[d + 2]!;
        let nx = by * az - bz * ay;
        let ny = bz * ax - bx * az;
        let nz = bx * ay - by * ax;
        const len = Math.hypot(nx, ny, nz) || 1;
        nx /= len;
        ny /= len;
        nz /= len;
        if (ny < 0) {
          nx = -nx;
          ny = -ny;
          nz = -nz;
        }
        nrm[i * 3] = nx;
        nrm[i * 3 + 1] = ny;
        nrm[i * 3 + 2] = nz;
        const below = depthNow[i]!;
        mined[i] = smooth((below - 0.4) / 1.6);
        const o = i * 3;
        if (below < 0.6) {
          col[o] = soilCol[o]!;
          col[o + 1] = soilCol[o + 1]!;
          col[o + 2] = soilCol[o + 2]!;
          continue;
        }
        rock(below, vary[i]!, col, o);
        const flat = smooth((ny - 0.8) / 0.15);
        // Berms gather dust; rubble lies at the toe of the face above; faces keep the colour of the rock.
        lerpInto(col, o, DUST, flat * 0.32);
        lerpInto(col, o, SCREE, screeNow[i]! * flat * 0.55);
        // Damp ground on the floor.
        if (floorDepth > 0 && below > floorDepth - 1.5) lerpInto(col, o, DAMP, 0.4 * flat);
      }
  };
  const flush = (k0: number, k1: number, j0: number, j1: number) => {
    const first = j0 * nxv + k0;
    const last = j1 * nxv + k1;
    for (const a of [posAttr, nrmAttr, colAttr]) {
      a.clearUpdateRanges();
      a.addUpdateRange(first * 3, (last - first + 1) * 3);
      a.needsUpdate = true;
    }
    minedAttr.clearUpdateRanges();
    minedAttr.addUpdateRange(first, last - first + 1);
    minedAttr.needsUpdate = true;
    uvAttr.clearUpdateRanges();
    uvAttr.addUpdateRange(first * 2, (last - first + 1) * 2);
    uvAttr.needsUpdate = true;
  };
  shade(0, cx, 0, cz, 0);
  flush(0, cx, 0, cz);

  /* ---------- The cutaway's walls ---------- */
  // Rows of the walls: the top edge, then fixed levels every 5 m down to the floor of the cut.
  const LEVELS: number[] = [];
  for (let y = TOP - 5; y > CUT_Y + 0.01; y -= 5) LEVELS.push(y);
  LEVELS.push(CUT_Y);
  const rowsN = LEVELS.length + 1;
  type Wall = { pts: [number, number][]; at: Int32Array; geo: THREE.BufferGeometry; pos: Float32Array; col: Float32Array; vary: Float32Array };
  const kOf = (x: number) => Math.round((x - GX0) / cell);
  const jOf = (z: number) => Math.round((z - GZ0) / cell);
  const line = (x0: number, z0: number, x1: number, z1: number) => {
    const n = Math.max(1, Math.round(Math.hypot(x1 - x0, z1 - z0) / cell));
    return Array.from({ length: n + 1 }, (_, s) => [mix(x0, x1, s / n), mix(z0, z1, s / n)] as [number, number]);
  };
  const wallMaterial = rockSurfaceMaterial();
  const faceMaterial = rockSurfaceMaterial();
  const walls: Wall[] = [];
  const makeWall = (pts: [number, number][], normal: THREE.Vector3, mat: THREE.Material) => {
    const n = pts.length;
    const p = new Float32Array(n * rowsN * 3);
    const c = new Float32Array(n * rowsN * 3);
    const nn = new Float32Array(n * rowsN * 3);
    const u = new Float32Array(n * rowsN * 2);
    const vr = new Float32Array(n * rowsN);
    const at = new Int32Array(n);
    const idx: number[] = [];
    for (let s = 0; s < n; s++) {
      const [x, z] = pts[s]!;
      at[s] = jOf(z) * nxv + kOf(x);
      for (let r = 0; r < rowsN; r++) {
        const i = s * rowsN + r;
        p[i * 3] = x;
        p[i * 3 + 2] = z;
        nn.set([normal.x, normal.y, normal.z], i * 3);
        u[i * 2] = (x + z) * 0.12;
        u[i * 2 + 1] = (r ? LEVELS[r - 1]! : 0) * 0.12;
        vr[i] = clamp(0.5 + 0.9 * fbm(x / 55 + 3, z / 55 - 2 + (r ? LEVELS[r - 1]! : 0) / 70, 3), 0, 0.999);
      }
    }
    for (let s = 0; s < n - 1; s++)
      for (let r = 0; r < rowsN - 1; r++) {
        const a = s * rowsN + r;
        const b = a + 1;
        const cc = a + rowsN;
        const d = cc + 1;
        idx.push(a, b, cc, cc, b, d);
      }
    // Wind the triangles to face `normal`.
    const [x0, z0] = pts[0]!;
    const [x1, z1] = pts[1]!;
    const along = new THREE.Vector3(x1 - x0, 0, z1 - z0);
    const facing = new THREE.Vector3(0, -1, 0).cross(along);
    if (facing.dot(normal) < 0)
      for (let t = 0; t < idx.length; t += 3) {
        const tmp = idx[t + 1]!;
        idx[t + 1] = idx[t + 2]!;
        idx[t + 2] = tmp;
      }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(p, 3).setUsage(THREE.DynamicDrawUsage));
    g.setAttribute("color", new THREE.BufferAttribute(c, 3).setUsage(THREE.DynamicDrawUsage));
    g.setAttribute("normal", new THREE.BufferAttribute(nn, 3));
    g.setAttribute("uv", new THREE.BufferAttribute(u, 2));
    g.setIndex(idx);
    g.boundingSphere = new THREE.Sphere(new THREE.Vector3(0, -150, 130), 700);
    const m = new THREE.Mesh(g, mat);
    m.receiveShadow = true;
    m.castShadow = true;
    group.add(m);
    const w: Wall = { pts, at, geo: g, pos: p, col: c, vary: vr };
    walls.push(w);
    return w;
  };
  const front = makeWall(line(CUT_X0, CUT_Z0, CUT_X1, CUT_Z0), new THREE.Vector3(0, 0, 1), faceMaterial);
  makeWall(line(CUT_X0, CUT_Z0, CUT_X0, CUT_Z1), new THREE.Vector3(1, 0, 0), wallMaterial);
  makeWall(line(CUT_X1, CUT_Z0, CUT_X1, CUT_Z1), new THREE.Vector3(-1, 0, 0), wallMaterial);
  makeWall(line(CUT_X0, CUT_Z1, CUT_X1, CUT_Z1), new THREE.Vector3(0, 0, -1), wallMaterial);

  // The floor of the cut.
  {
    const fw = CUT_X1 - CUT_X0;
    const fd = CUT_Z1 - CUT_Z0;
    const fg = new THREE.PlaneGeometry(fw, fd, Math.round(fw / 20), Math.round(fd / 20));
    fg.rotateX(-Math.PI / 2);
    fg.translate((CUT_X0 + CUT_X1) / 2, CUT_Y, (CUT_Z0 + CUT_Z1) / 2);
    const p = fg.getAttribute("position") as THREE.BufferAttribute;
    const c = new Float32Array(p.count * 3);
    for (let i = 0; i < p.count; i++) {
      const x = p.getX(i);
      const z = p.getZ(i);
      rock(TOP - CUT_Y, clamp(0.5 + 0.9 * fbm(x / 40, z / 40, 3), 0, 0.999), c, i * 3);
      // Darker into the corners, where little light reaches.
      const e = Math.min(x - CUT_X0, CUT_X1 - x, z - CUT_Z0, CUT_Z1 - z);
      const kk = 0.62 + 0.25 * smooth(e / 60);
      c[i * 3]! *= kk;
      c[i * 3 + 1]! *= kk;
      c[i * 3 + 2]! *= kk;
    }
    fg.setAttribute("color", new THREE.BufferAttribute(c, 3));
    const floor = new THREE.Mesh(fg, wallMaterial);
    floor.receiveShadow = true;
    group.add(floor);
  }

  /** Heights from the detailed grid as drawn now (at the grid's own points; close enough off them). */
  const heightNow = (x: number, z: number) => {
    const fx = clamp((x - GX0) / cell, 0, cx);
    const fz = clamp((z - GZ0) / cell, 0, cz);
    const k = Math.min(cx - 1, Math.floor(fx));
    const j = Math.min(cz - 1, Math.floor(fz));
    const tx = fx - k;
    const tz = fz - j;
    const h = (kk: number, jj: number) => pos[(jj * nxv + kk) * 3 + 1]!;
    return mix(mix(h(k, j), h(k + 1, j), tx), mix(h(k, j + 1), h(k + 1, j + 1), tx), tz);
  };
  const sectionX = new Float32Array(front.pts.length);
  const sectionY = new Float32Array(front.pts.length);
  front.pts.forEach(([x], s) => (sectionX[s] = x));
  const placeWalls = () => {
    for (const w of walls) {
      for (let s = 0; s < w.pts.length; s++) {
        // The wall's top is the ground's edge vertex itself, wherever its snapping moved it along the wall.
        const g = w.at[s]!;
        const x = pos[g * 3]!;
        const z = pos[g * 3 + 2]!;
        const top = pos[g * 3 + 1]!;
        const nat = surface(x, z);
        if (w === front) sectionY[s] = top;
        for (let r = 0; r < rowsN; r++) {
          const i = s * rowsN + r;
          const y = r === 0 ? top : Math.min(top - 0.01 * r, LEVELS[r - 1]!);
          w.pos[i * 3] = x;
          w.pos[i * 3 + 1] = y;
          w.pos[i * 3 + 2] = z;
          rock(nat - y, w.vary[i]!, w.col, i * 3);
          // A little darker towards the floor of the cut.
          const k2 = 0.78 + 0.22 * smooth((y - CUT_Y) / 90);
          w.col[i * 3]! *= k2;
          w.col[i * 3 + 1]! *= k2;
          w.col[i * 3 + 2]! *= k2;
        }
      }
      (w.geo.getAttribute("position") as THREE.BufferAttribute).needsUpdate = true;
      (w.geo.getAttribute("color") as THREE.BufferAttribute).needsUpdate = true;
    }
  };
  placeWalls();

  /* ---------- The section face: the block grid and the grades of the row behind it ---------- */
  const gradeData = new Uint8Array(NX * NY * 4);
  const faceGrades = new THREE.DataTexture(gradeData, NX, NY, THREE.RGBAFormat);
  faceGrades.magFilter = THREE.NearestFilter;
  faceGrades.minFilter = THREE.NearestFilter;
  faceGrades.colorSpace = THREE.SRGBColorSpace;
  faceGrades.needsUpdate = true;
  {
    const baseCompile = faceMaterial.onBeforeCompile;
    faceMaterial.customProgramCacheKey = () => "fm-pit-face-v2";
    faceMaterial.onBeforeCompile = (shader, renderer) => {
      baseCompile.call(faceMaterial, shader, renderer);
      shader.uniforms.tGrade = { value: faceGrades };
      shader.uniforms.uEng = uEng;
      shader.fragmentShader = shader.fragmentShader
        .replace("#include <common>", "#include <common>\nuniform sampler2D tGrade;\nuniform float uEng;")
        .replace(
          "#include <emissivemap_fragment>",
          `#include <emissivemap_fragment>
{
  vec2 cellF = vec2((vRW.x - (${X0.toFixed(1)})) / ${B.toFixed(1)}, ((${TOP.toFixed(1)}) - vRW.y) / ${H.toFixed(1)});
  vec2 fw = max(fwidth(cellF), vec2(1e-4));
  vec2 gl = abs(fract(cellF + 0.5) - 0.5) / fw;
  float lineA = 1.0 - min(min(gl.x, gl.y) / 1.2, 1.0);
  vec4 gt = texture2D(tGrade, (floor(cellF) + 0.5) / vec2(${NX.toFixed(1)}, ${NY.toFixed(1)}));
  float lum = dot(diffuseColor.rgb, vec3(0.3, 0.59, 0.11));
  diffuseColor.rgb = mix(diffuseColor.rgb, vec3(lum * 0.7), uEng * 0.6);
  diffuseColor.rgb = mix(diffuseColor.rgb, gt.rgb * 0.9, gt.a * mix(0.7, 0.9, uEng));
  totalEmissiveRadiance += gt.rgb * gt.a * mix(0.16, 0.3, uEng);
  diffuseColor.rgb = mix(diffuseColor.rgb, vec3(0.06, 0.05, 0.045), lineA * mix(0.32, 0.55, uEng));
}`,
        );
    };
  }

  /* ---------- Scrub and boulders on the undisturbed desert ---------- */
  const scatter = o.scatter ?? 2400;
  const scattered: { x: number; z: number; mesh: THREE.InstancedMesh; at: number; m: THREE.Matrix4 }[] = [];
  const scatterMeshes: THREE.InstancedMesh[] = [];
  if (scatter > 0) {
    let seed = 1234567;
    const rnd = () => (seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff;
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const e = new THREE.Euler();
    const sc = new THREE.Vector3();
    const p = new THREE.Vector3();
    const c = new THREE.Color();
    const place = (geo: THREE.BufferGeometry, n: number, bush: boolean) => {
      const mat = new THREE.MeshStandardMaterial({ roughness: bush ? 1 : 0.92, metalness: 0, flatShading: true });
      const mesh = new THREE.InstancedMesh(geo, mat, n);
      for (let t = 0; t < n; t++) {
        let x = 0;
        let z = 0;
        for (let tries = 0; tries < 20; tries++) {
          // Denser near the pit, out to about a kilometre and a half; clumped where a slow noise says so.
          const r = 60 + 1500 * Math.pow(rnd(), 1.6);
          const a = rnd() * Math.PI * 2;
          x = Math.cos(a) * r;
          z = Math.sin(a) * r;
          const clear = !(x > CUT_X0 - 6 && x < CUT_X1 + 6 && z > CUT_Z0 - 6 && z < CUT_Z1 + 6);
          if (clear && fbm(x / 90 + 1, z / 90 - 4, 2) > (bush ? -0.05 : 0.12)) break;
        }
        const s = bush ? 0.7 + 1.3 * rnd() : 0.4 + 1.5 * rnd() ** 2;
        sc.set(s * (0.9 + 0.5 * rnd()), s * (bush ? 0.55 + 0.25 * rnd() : 0.5 + 0.4 * rnd()), s * (0.9 + 0.5 * rnd()));
        e.set(rnd() * 0.3, rnd() * Math.PI * 2, rnd() * 0.3);
        q.setFromEuler(e);
        p.set(x, surface(x, z) + sc.y * (bush ? 0.35 : 0.2), z);
        m.compose(p, q, sc);
        mesh.setMatrixAt(t, m);
        if (bush) c.setHSL(0.13 + 0.05 * rnd(), 0.16 + 0.12 * rnd(), 0.2 + 0.1 * rnd());
        else c.setHSL(0.07 + 0.03 * rnd(), 0.14 + 0.1 * rnd(), 0.3 + 0.16 * rnd());
        mesh.setColorAt(t, c);
        if (Math.abs(x) < GX1 && Math.abs(z) < GZ1) scattered.push({ x, z, mesh, at: t, m: m.clone() });
      }
      mesh.instanceMatrix.needsUpdate = true;
      mesh.castShadow = true;
      mesh.receiveShadow = true;
      group.add(mesh);
      scatterMeshes.push(mesh);
    };
    place(new THREE.IcosahedronGeometry(1, 1), scatter, true);
    place(new THREE.DodecahedronGeometry(1, 0), Math.round(scatter * 0.35), false);
  }
  /** Hides the scrub and boulders the pit digs out (zero scale), and brings back the rest. */
  const zero = new THREE.Matrix4().makeScale(0, 0, 0);
  const clearScatter = () => {
    for (const s of scattered) s.mesh.setMatrixAt(s.at, shaper.field3(s.x, s.z) > 0.3 ? zero : s.m);
    for (const m of scatterMeshes) m.instanceMatrix.needsUpdate = true;
  };

  /* ---------- The horizon ---------- */
  if (o.surround !== false) {
    const far = noise2((o.seed ?? 23) + 11);
    const outer = (x: number, z: number) => {
      const r = Math.hypot(x, z);
      const hills = 22 * far.fbm(x / 900, z / 900, 4) * smooth((r - 800) / 900);
      const ranges = 280 * Math.max(0, far.fbm(x / 2600 + 4, z / 2600 - 2, 5) + 0.12) * smooth((r - 2600) / 2400);
      return surface(x, z) + hills + ranges;
    };
    const axisX: number[] = [];
    const axisZ: number[] = [];
    const build = (lo: number, hi: number, axis: number[]) => {
      const inner = Math.max(2, Math.round((hi - lo) / (cell * 2)));
      for (let i = 0; i <= inner; i++) axis.push(lo + ((hi - lo) * i) / inner);
      let d = 0;
      const s0 = cell * 2;
      while (d < 8200) {
        d += Math.min(420, s0 + 0.075 * d);
        axis.push(hi + d);
        axis.unshift(lo - d);
      }
    };
    build(GX0, GX1, axisX);
    build(GZ0, GZ1, axisZ);
    const mX = axisX.length;
    const mZ = axisZ.length;
    const verts = new Float32Array(mX * mZ * 3);
    const cols = new Float32Array(mX * mZ * 3);
    const uvs = new Float32Array(mX * mZ * 2);
    const idx: number[] = [];
    const tmp = new THREE.Color("#8d7560");
    for (let j = 0; j < mZ; j++)
      for (let k = 0; k < mX; k++) {
        const x = axisX[k]!;
        const z = axisZ[j]!;
        const i = j * mX + k;
        const inner = x >= GX0 - 0.01 && x <= GX1 + 0.01 && z >= GZ0 - 0.01 && z <= GZ1 + 0.01;
        // Under the detailed square's edge by a few centimetres, so the two never share a plane.
        const y = inner ? surface(x, z) - 0.35 : outer(x, z);
        verts.set([x, y, z], i * 3);
        uvs.set([x * 0.12, z * 0.12], i * 2);
        soil(x, z, cols, i * 3);
        if (y > 70) lerpInto(cols, i * 3, tmp, smooth((y - 70) / 140) * 0.6);
      }
    for (let j = 0; j < mZ - 1; j++)
      for (let k = 0; k < mX - 1; k++) {
        if (axisX[k]! >= GX0 - 0.01 && axisX[k + 1]! <= GX1 + 0.01 && axisZ[j]! >= GZ0 - 0.01 && axisZ[j + 1]! <= GZ1 + 0.01) continue;
        const a = j * mX + k;
        idx.push(a, a + mX, a + 1, a + 1, a + mX, a + mX + 1);
      }
    const g2 = new THREE.BufferGeometry();
    g2.setAttribute("position", new THREE.BufferAttribute(verts, 3));
    g2.setAttribute("color", new THREE.BufferAttribute(cols, 3));
    g2.setAttribute("uv", new THREE.BufferAttribute(uvs, 2));
    g2.setIndex(idx);
    g2.computeVertexNormals();
    const ring = new THREE.Mesh(g2, pitMaterial(false));
    ring.receiveShadow = true;
    group.add(ring);
  }

  /* ---------- Strings ---------- */
  let lastStrings: Strings = { crest: new Float32Array(0), toe: new Float32Array(0) };
  /** Marching squares over the target's smooth depth at each crest and toe depth. */
  const trace = (k0: number, k1: number, j0: number, j1: number) => {
    const { crests, toes } = shaper.lines();
    const run = (levels: number[]) => {
      const out: number[] = [];
      if (!levels.length) return new Float32Array(0);
      const pts: number[] = [];
      for (let j = j0; j < j1; j++)
        for (let k = k0; k < k1; k++) {
          const x0 = GX0 + k * cell;
          const z0 = GZ0 + j * cell;
          if (inCut(x0, x0 + cell, z0, z0 + cell)) continue;
          const a = j * nxv + k;
          const v00 = smoothTo[a]!;
          const v10 = smoothTo[a + 1]!;
          const v01 = smoothTo[a + nxv]!;
          const v11 = smoothTo[a + nxv + 1]!;
          const lo = Math.min(v00, v10, v01, v11);
          const hi = Math.max(v00, v10, v01, v11);
          for (const t of levels) {
            if (t <= lo || t > hi) continue;
            const y = (xx: number, zz: number) => surface(xx, zz) - shaper.terrace(t) + 0.8;
            pts.length = 0;
            const cross = (va: number, vb: number, xa: number, za: number, xb: number, zb: number) => {
              if (va < t === vb < t) return;
              const f = (t - va) / (vb - va);
              pts.push(mix(xa, xb, f), mix(za, zb, f));
            };
            cross(v00, v10, x0, z0, x0 + cell, z0);
            cross(v10, v11, x0 + cell, z0, x0 + cell, z0 + cell);
            cross(v01, v11, x0, z0 + cell, x0 + cell, z0 + cell);
            cross(v00, v01, x0, z0, x0, z0 + cell);
            for (let p = 0; p + 3 < pts.length; p += 4) {
              const xa = pts[p]!;
              const za = pts[p + 1]!;
              const xb = pts[p + 2]!;
              const zb = pts[p + 3]!;
              out.push(xa, y(xa, za), za, xb, y(xb, zb), zb);
            }
          }
        }
      return new Float32Array(out);
    };
    lastStrings = { crest: run(crests), toe: run(toes) };
  };

  /* ---------- Shaping and morphing ---------- */
  const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);
  let floorDepth = 0;
  const snapR = cell * 0.5;
  const design = { floor: 0, benches: 0, berm: 0 };
  const ground: Ground = {
    group,
    surface,
    heightAt: heightNow,
    sectionX,
    sectionY,
    faceGrades,
    floorY: CUT_Y,
    material,
    design,
    shape(cols, slopeDeg, seconds) {
      shaper.set(cols, slopeDeg);
      floorDepth = shaper.floor;
      design.floor = shaper.floor;
      design.benches = shaper.benches;
      design.berm = bermWidth(slopeDeg);
      // Start from what is drawn now.
      depthFrom.set(depthNow);
      offFrom.set(offNow);
      screeFrom.set(screeNow);
      // Only the block model's footprint (and a margin for the smoothing) can change.
      const kx0 = Math.max(1, kOf(X0 - 2 * B));
      const kx1 = Math.min(cx - 1, kOf(X0 + NX * B + 2 * B));
      const jz0 = Math.max(1, jOf(Z0 - 2 * B));
      const jz1 = Math.min(cz - 1, jOf(Z0 + NZ * B + 2 * B));
      let k0 = cx;
      let k1 = 0;
      let j0 = cz;
      let j1 = 0;
      // The smooth depth once per vertex (and a ring round the footprint), then slopes from the neighbours.
      for (let j = jz0 - 1; j <= jz1 + 1; j++)
        for (let k = kx0 - 1; k <= kx1 + 1; k++) {
          const i = j * nxv + k;
          rawAt[i] = shaper.field3(xs[i]!, zs[i]!);
        }
      const inv2 = 1 / (2 * cell);
      for (let j = jz0; j <= jz1; j++)
        for (let k = kx0; k <= kx1; k++) {
          const i = j * nxv + k;
          const raw = rawAt[i]!;
          let d = 0;
          let ox = 0;
          let oz = 0;
          let scree = 0;
          let sd = raw;
          snapped[i] = -1;
          if (raw > 0.05) {
            sd = raw + breakage[i]! * smooth(raw / 5);
            const gx = (rawAt[i + 1]! - rawAt[i - 1]!) * inv2;
            const gz = (rawAt[i + nxv]! - rawAt[i - nxv]!) * inv2;
            const gl = Math.hypot(gx, gz);
            slopeDir[i * 2] = gl > 1e-4 ? gx / gl : 0;
            slopeDir[i * 2 + 1] = gl > 1e-4 ? gz / gl : 0;
            d = shaper.terrace(sd);
            const t = shaper.breakAt(sd);
            if (t > 0) {
              // Along the slope, or along the cutaway's edge where the vertex is on one.
              let dx = gx;
              let dz = gz;
              const e = edge[i]!;
              if (e & 1) dz = 0;
              if (e & 2) dx = 0;
              const rate = Math.hypot(dx, dz);
              if (rate > 0.06 && e !== 3) {
                const dist = (t - sd) / rate;
                if (Math.abs(dist) < snapR) {
                  ox = (dx / rate) * dist;
                  oz = (dz / rate) * dist;
                  d = shaper.terrace(t);
                  snapped[i] = t;
                }
              }
            }
            // Rubble on the berm just below each face's toe.
            const g = Math.hypot(gx, gz);
            const u = sd / H;
            const kk = Math.floor(u);
            const f = u - kk;
            if (kk >= 1 && f < shaper.berm && g > 0.05 && d < shaper.floor - 0.5) {
              const fromToe = (f * H) / g;
              scree = (1 - smooth(fromToe / 3.2)) * (0.6 + 0.4 * smooth(breakage[i]! + 0.5));
            }
            d += rough[i]! * smooth(d / 4);
          }
          smoothTo[i] = sd;
          depthTo[i] = d;
          offTo[i * 2] = ox;
          offTo[i * 2 + 1] = oz;
          screeTo[i] = scree;
          if (Math.abs(d - depthFrom[i]!) > 1e-3 || Math.abs(ox - offFrom[i * 2]!) > 1e-3 || Math.abs(oz - offFrom[i * 2 + 1]!) > 1e-3) {
            if (k < k0) k0 = k;
            if (k > k1) k1 = k;
            if (j < j0) j0 = j;
            if (j > j1) j1 = j;
          }
        }
      // New diagonals for the footprint's quads, then one upload of the index.
      for (let j = jz0; j < jz1; j++) for (let k = kx0; k < kx1; k++) split(k, j);
      indexAttr.needsUpdate = true;
      trace(kx0, kx1, jz0, jz1);
      clearScatter();
      if (k1 < k0) return;
      box = { k0: Math.max(0, k0 - 1), k1: Math.min(cx, k1 + 1), j0: Math.max(0, j0 - 1), j1: Math.min(cz, j1 + 1) };
      span = Math.max(seconds, 1e-3);
      morph = seconds > 0 ? 0 : 1;
      ground.update(0);
    },
    update(dt) {
      if (morph >= 1 && dt > 0) return false;
      morph = Math.min(1, morph + dt / span);
      const e = ease(morph);
      const { k0, k1, j0, j1 } = box;
      for (let j = j0; j <= j1; j++)
        for (let k = k0; k <= k1; k++) {
          const i = j * nxv + k;
          const d = mix(depthFrom[i]!, depthTo[i]!, e);
          depthNow[i] = d;
          offNow[i * 2] = mix(offFrom[i * 2]!, offTo[i * 2]!, e);
          offNow[i * 2 + 1] = mix(offFrom[i * 2 + 1]!, offTo[i * 2 + 1]!, e);
          screeNow[i] = mix(screeFrom[i]!, screeTo[i]!, e);
          pos[i * 3] = xs[i]! + offNow[i * 2]!;
          pos[i * 3 + 1] = base[i]! - d;
          pos[i * 3 + 2] = zs[i]! + offNow[i * 2 + 1]!;
          // The texture runs down each face as far as the face is long, not squeezed into its plan width.
          uv[i * 2] = (pos[i * 3]! + slopeDir[i * 2]! * d) * 0.12;
          uv[i * 2 + 1] = (pos[i * 3 + 2]! + slopeDir[i * 2 + 1]! * d) * 0.12;
        }
      shade(Math.max(0, k0 - 1), Math.min(cx, k1 + 1), Math.max(0, j0 - 1), Math.min(cz, j1 + 1), floorDepth);
      flush(Math.max(0, k0 - 1), Math.min(cx, k1 + 1), Math.max(0, j0 - 1), Math.min(cz, j1 + 1));
      placeWalls();
      return morph < 1;
    },
    profile(cols, slopeDeg, out) {
      const s = new Shaper();
      s.set(cols, slopeDeg);
      for (let p = 0; p < sectionX.length; p++) {
        const x = sectionX[p]!;
        out[p] = surface(x, CUT_Z0) - s.depth(x, CUT_Z0);
      }
      return out;
    },
    strings: () => lastStrings,
    setEngineering(t) {
      uEng.value = t;
    },
  };
  return ground;
}
