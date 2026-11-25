/**
 * Broken rock for the site scenes: conical and windrow stockpiles, truck-dumped ROM piles, blasted muck
 * piles and boulder scatters. Surfaces are noisy height fields (slump gullies down the slope, lumps,
 * a wavy toe) with a pebble normal map; boulders are instanced angular rocks, the big ones rolled to
 * the toe the way real piles segregate. Metres, y up, origin on the ground at the pile's centre.
 */
import * as THREE from "three";
import { colourHeap, heapMaterial, makeRocks, Noise3, rng, ROCK_PALETTES, smoothstep, type RockPalette, type RockSet } from "./fleet-kit";

export interface PileOptions {
  /** Toe radius (half-width for windrows), m. */
  radius: number;
  /** Crest height, m. Angle of repose for dry broken ore is ~37°, i.e. height ≈ 0.75 × radius. */
  height: number;
  /** "cone" (stacker cone), "ridge" (windrow along x of `length`), "dump" (truck-dumped, rounded top). */
  shape?: "cone" | "ridge" | "dump";
  /** Ridge length along x, m (shape "ridge"). */
  length?: number;
  /** Rock palette name, or explicit sRGB colours. */
  palette?: RockPalette | string[];
  /** Single colour shortcut: tints a grey-brown palette toward it. */
  colour?: string;
  seed?: number;
  /** Boulders round the toe and on the slopes (default scales with the pile). */
  boulders?: number;
  /** Grid cell size, m (default radius / 36). */
  cell?: number;
}

export interface Pile {
  group: THREE.Group;
  surface: THREE.Mesh;
  rocks: RockSet;
  /** Height of the pile surface at (x, z) in the pile's frame. */
  heightAt(x: number, z: number): number;
}

function paletteOf(o: { palette?: RockPalette | string[]; colour?: string }): string[] {
  if (Array.isArray(o.palette)) return o.palette;
  const base = [...ROCK_PALETTES[o.palette ?? "ore"]];
  if (!o.colour) return base;
  const tint = new THREE.Color(o.colour);
  return base.map((c) => "#" + new THREE.Color(c).lerp(tint, 0.55).getHexString());
}

/** Builds a height-field mesh over a rectangle, dropping triangles that are wholly below ground. */
function field(x0: number, x1: number, z0: number, z1: number, cell: number, h: (x: number, z: number) => number) {
  const nx = Math.max(2, Math.ceil((x1 - x0) / cell));
  const nz = Math.max(2, Math.ceil((z1 - z0) / cell));
  const pos = new Float32Array((nx + 1) * (nz + 1) * 3);
  for (let j = 0; j <= nz; j++) {
    for (let i = 0; i <= nx; i++) {
      const x = x0 + ((x1 - x0) * i) / nx;
      const z = z0 + ((z1 - z0) * j) / nz;
      const k = (j * (nx + 1) + i) * 3;
      pos[k] = x;
      pos[k + 1] = h(x, z);
      pos[k + 2] = z;
    }
  }
  const idx: number[] = [];
  const y = (i: number) => pos[i * 3 + 1]!;
  for (let j = 0; j < nz; j++) {
    for (let i = 0; i < nx; i++) {
      const a = j * (nx + 1) + i;
      const b = a + 1;
      const c = a + nx + 1;
      const d = c + 1;
      if (Math.max(y(a), y(b), y(c), y(d)) < -0.04) continue;
      idx.push(a, c, b, b, c, d);
    }
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  g.setIndex(idx);
  g.computeVertexNormals();
  return g;
}

const pileCache = new Map<string, THREE.BufferGeometry>();

/**
 * A stockpile of broken ore: a cone under a stacker, a windrow (`shape: "ridge"`) or a truck-dumped
 * pile with a rounded crest (`shape: "dump"`). Same options give the same (shared) surface geometry.
 */
export function makeStockpile(o: PileOptions): Pile {
  const seed = o.seed ?? 1;
  const R = o.radius;
  const H = o.height;
  const L = o.shape === "ridge" ? (o.length ?? R * 3) : 0;
  const n = new Noise3(seed);
  const cell = o.cell ?? Math.max(0.25, R / 36);
  const dist = (x: number, z: number) => {
    const dx = Math.max(0, Math.abs(x) - L / 2);
    return Math.hypot(dx, z);
  };
  const angle = (x: number, z: number) => Math.atan2(z, Math.abs(x) > L / 2 ? Math.abs(x) - L / 2 : 0.0001);
  const heightAt = (x: number, z: number) => {
    const th = angle(x, z);
    // wavy toe: the pile spreads unevenly
    const Rt = R * (1 + 0.07 * n.fbm(Math.cos(th) * 1.3 + x * 0.004, Math.sin(th) * 1.3, 0.5, 3));
    const t = dist(x, z) / Rt;
    let y: number;
    if (o.shape === "dump") {
      // rounded crest, steep sides
      y = H * Math.max(0, 1 - t * t) ** 0.62;
    } else {
      const a = 0.12;
      y = t < a ? H * (1 - a / 2 - (t * t) / (2 * a)) : H * (1 - t);
    }
    if (t > 1) y = -(t - 1) * R * 0.2;
    // slump gullies running down the slope, lumps, grit
    const inSlope = smoothstep(1.03, 0.85, t) * smoothstep(0, 0.25, t);
    const circ = Math.max(6, (Math.PI * 2 * R) / 7);
    const gul = n.noise(th * (circ / (Math.PI * 2)) + x * 0.05, t * 2.2, 7.3);
    const lumps = n.fbm(x * 0.22, z * 0.22, 3.1, 4);
    const grit = n.noise(x * 1.3, z * 1.3, 9.9);
    return y + inSlope * (gul * 0.035 * R + lumps * 0.05 * R * (0.4 + 0.6 * t) + grit * 0.06);
  };
  const key = JSON.stringify({ R, H, L, s: o.shape ?? "cone", seed, cell, p: paletteOf(o) });
  let g = pileCache.get(key);
  if (!g) {
    const m = R * 1.12;
    g = field(-L / 2 - m, L / 2 + m, -m, m, cell, heightAt);
    colourHeap(g, paletteOf(o), seed, 1.5);
    g.computeBoundingSphere();
    pileCache.set(key, g);
  }
  const surface = new THREE.Mesh(g, heapMaterial(`pile:${paletteOf(o).join(",")}`));
  surface.castShadow = true;
  surface.receiveShadow = true;
  // boulders: the coarse fraction rolls to the toe; a few sit half-buried on the slope
  const r = rng(seed * 13 + 5);
  const nToe = o.boulders ?? Math.round((Math.PI * 2 * R + 2 * L) / 1.6);
  const nSlope = Math.round(nToe * 0.45);
  const size = Math.min(1.8, Math.max(0.35, R * 0.05));
  const rocks = makeRocks({
    count: nToe + nSlope,
    seed: seed + 3,
    shapes: 4,
    detail: 1,
    palette: paletteOf(o),
    place: (i) => {
      const toe = i < nToe;
      const th = r() * Math.PI * 2;
      const t = toe ? 0.9 + r() * 0.2 : 0.25 + r() * 0.65;
      let x = Math.cos(th) * t * R;
      const z = Math.sin(th) * t * R;
      if (L > 0) x += (r() - 0.5) * L;
      const s = size * (toe ? 0.6 + r() ** 1.5 * 1.2 : 0.35 + r() * 0.5);
      return { x, y: Math.max(0, heightAt(x, z)) + s * (toe ? 0.12 : -0.08), z, size: s };
    },
  });
  // the fine fraction: small lumps sitting all over the surface, which is what makes it read as rock
  const nGrit = Math.round((Math.PI * R * R + 2 * R * L) / 9);
  const grit = makeRocks({
    count: nGrit,
    seed: seed + 11,
    shapes: 3,
    detail: 0,
    palette: paletteOf(o),
    place: () => {
      const th = r() * Math.PI * 2;
      const t = Math.sqrt(r()) * 0.98;
      let x = Math.cos(th) * t * R;
      const z = Math.sin(th) * t * R;
      if (L > 0) x += (r() - 0.5) * L;
      const s = size * (0.15 + r() * 0.3);
      return { x, y: heightAt(x, z) - s * 0.1, z, size: s };
    },
  });
  rocks.meshes.push(...grit.meshes);
  rocks.group.add(...grit.meshes);
  rocks.count += nGrit;
  const group = new THREE.Group();
  group.name = "stockpile";
  group.add(surface, rocks.group);
  return { group, surface, rocks, heightAt };
}

export interface MuckPileOptions {
  /** Along the face (x), m. */
  length: number;
  /** Throw from the face (z), m. */
  width: number;
  /** Crest height, m. */
  height: number;
  palette?: RockPalette | string[];
  colour?: string;
  seed?: number;
  /** Number of loose boulders on the pile (default from area). */
  boulders?: number;
}

/**
 * A blasted muck pile: broken rock thrown forward from a bench face (the face is at z = -width/2,
 * the pile slopes toward +z), lumpy and coarse, littered with boulders of mixed size.
 */
export function makeMuckPile(o: MuckPileOptions): Pile {
  const seed = o.seed ?? 2;
  const n = new Noise3(seed + 40);
  const hx = o.length / 2;
  const hz = o.width / 2;
  const H = o.height;
  const heightAt = (x: number, z: number) => {
    // superellipse footprint with a ragged edge; v runs 0 at the face to 1 at the far toe
    const e = n.fbm(x * 0.05, z * 0.05, 1.7, 3) * 0.12;
    const u = Math.abs(x) / (hx * (1 + e));
    const v = (z + hz) / (2 * hz * (1 + e));
    const side = Math.max(0, 1 - u ** 4);
    // steep against the face, crest a fifth of the way out, rolling off toward the toe
    const prof = v < 0.2 ? smoothstep(-0.12, 0.2, v) : Math.max(0, 1 - ((v - 0.2) / 0.8) ** 1.6);
    let y = H * prof * side ** 0.5;
    const lumps = n.fbm(x * 0.12, z * 0.12, 0.3, 4) * 0.22 * H + n.fbm(x * 0.5, z * 0.5, 4.4, 3) * 0.05 * H;
    y += lumps * Math.min(1, y / (H * 0.25));
    return y > 0.02 ? y : -0.25 + y;
  };
  const cell = Math.max(0.3, Math.min(o.length, o.width) / 40);
  const key = JSON.stringify({ muck: o, cell });
  let g = pileCache.get(key);
  if (!g) {
    g = field(-hx * 1.15, hx * 1.15, -hz * 1.3, hz * 1.4, cell, heightAt);
    colourHeap(g, paletteOf(o), seed, 1.3);
    g.computeBoundingSphere();
    pileCache.set(key, g);
  }
  const surface = new THREE.Mesh(g, heapMaterial(`pile:${paletteOf(o).join(",")}`));
  surface.castShadow = surface.receiveShadow = true;
  const r = rng(seed * 7 + 1);
  const count = o.boulders ?? Math.round((o.length * o.width) / 2.4);
  const rocks = makeRocks({
    count,
    seed: seed + 9,
    shapes: 5,
    detail: 1,
    palette: paletteOf(o),
    place: () => {
      for (let k = 0; k < 8; k++) {
        const x = (r() * 2 - 1) * hx;
        const z = (r() * 2 - 1) * hz;
        const y = heightAt(x, z);
        if (y > 0.05 || k === 7) {
          const s = 0.25 + r() ** 2.6 * Math.min(2.6, H * 0.38);
          return { x, y: Math.max(0, y) + s * 0.05, z, size: s };
        }
      }
      return { x: 0, y: 0, z: 0, size: 0.3 };
    },
  });
  const group = new THREE.Group();
  group.name = "muck-pile";
  group.add(surface, rocks.group);
  return { group, surface, rocks, heightAt };
}

/**
 * Loose boulders scattered on flat ground within a radius (spillage on haul roads, a blast's fly rock,
 * oversize set aside for the breaker). Sizes in metres across.
 */
export function makeBoulders(o: {
  count: number;
  radius: number;
  size?: [number, number];
  palette?: RockPalette | string[];
  seed?: number;
  detail?: number;
}): RockSet {
  const r = rng((o.seed ?? 4) * 31 + 7);
  const [lo, hi] = o.size ?? [0.3, 1.4];
  return makeRocks({
    count: o.count,
    seed: o.seed ?? 4,
    shapes: 5,
    detail: o.detail ?? 2,
    palette: o.palette ?? "ore",
    place: () => {
      const a = r() * Math.PI * 2;
      const d = Math.sqrt(r()) * o.radius;
      const s = lo + (hi - lo) * r() ** 2;
      return { x: Math.cos(a) * d, y: s * 0.22, z: Math.sin(a) * d, size: s };
    },
  });
}
