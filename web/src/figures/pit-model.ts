/**
 * The Plan scene's block model and its economics, free of any drawing so that the solver worker can run it: the
 * same synthetic gold orebody the Explore scene drills (drill-model's prospect, seed 11), cut into a wider and
 * deeper grid so a pit has room for its walls, and the dollar value of every block at a gold price, a slope and a
 * mining cost. Nothing here is measured at any real place; every cost is an assumption a planner would replace.
 *
 * Coordinates are three.js world metres (x east, y up, z south). Block (i, j, k) has i east, j south and k down
 * from the top bench, and sits at index i + NX·(j + NZ·k), the layout the engine's grid expects (its "north" axis
 * is our j; the slope rule is the same in every direction, so the pit does not care which way j points).
 */
import { OZ, type Body } from "./drill-body";
import { DENSITY } from "./drill-model";

/** Block edge east and south, metres: the Explore grid's 20 m, so the two scenes share their block boundaries. */
export const B = 20;
/** Bench height: one block high, metres. */
export const H = 15;
/** Blocks east, south and down: 880 × 760 m, 330 m deep. */
export const NX = 44;
export const NZ = 38;
export const NY = 22;
export const N = NX * NZ * NY;
export const COLS = NX * NZ;
export const X0 = -(NX * B) / 2;
export const Z0 = -(NZ * B) / 2;
export const TOP = 0;
export const BOTTOM = TOP - NY * H;
/** Tonnes in a block. */
export const BLOCK_T = B * B * H * DENSITY;

/** Assumed economics (illustrative): metallurgical recovery, processing with overheads, and the haul's cost of depth. */
export const RECOVERY = 0.9;
export const PROCESS = 18;
export const PER_METRE = 0.01;
/** Revenue factors for the nested shells: the best pit if gold sold for 50% … 100% of the price. */
export const FACTORS = [0.5, 0.6, 0.7, 0.8, 0.9, 1.0];

export interface Econ {
  /** Gold price, $/oz. */
  price: number;
  /** Overall slope angle, degrees from horizontal. */
  slope: number;
  /** Mining cost at surface, $/t. */
  mining: number;
}
export const START: Econ = { price: 2600, slope: 45, mining: 2.2 };

/** The engine's grid for a slope. */
export const grid = (slopeDeg: number) => ({ nx: NX, ny: NZ, nz: NY, dx: B, dy: B, dz: H, slopeDeg });

export const index = (i: number, j: number, k: number) => i + NX * (j + NZ * k);
export const centreX = (i: number) => X0 + (i + 0.5) * B;
export const centreZ = (j: number) => Z0 + (j + 0.5) * B;
export const centreY = (k: number) => TOP - (k + 0.5) * H;

/**
 * The cutaway: the blocks in front of the section (south of z = 0) inside this box are drawn, and the ground over
 * them is lifted away. Wide and deep enough for the largest pit the controls can ask for.
 */
export const CUT = { i0: 4, i1: 40, j0: 19, j1: 32, k1: 20 } as const;
export const CUT_X0 = X0 + CUT.i0 * B;
export const CUT_X1 = X0 + CUT.i1 * B;
export const CUT_Z0 = Z0 + CUT.j0 * B;
export const CUT_Z1 = Z0 + CUT.j1 * B;
export const CUT_Y = TOP - CUT.k1 * H;

/** Each block's grade, g/t: the orebody averaged over four points inside the block, so grades have block support. */
export function grades(body: Body): Float32Array {
  const g = new Float32Array(N);
  const q = [
    [-1, -1, -1],
    [1, 1, -1],
    [1, -1, 1],
    [-1, 1, 1],
  ] as const;
  for (let k = 0; k < NY; k++)
    for (let j = 0; j < NZ; j++)
      for (let i = 0; i < NX; i++) {
        const x = centreX(i);
        const y = centreY(k);
        const z = centreZ(j);
        let s = 0;
        for (const [a, b, c] of q) s += body.grade(x + (a * B) / 4, y + (b * H) / 4, z + (c * B) / 4);
        g[index(i, j, k)] = s / 4;
      }
  return g;
}

/** The marginal cut-off: the grade whose recovered gold pays for processing it, g/t. */
export const cutoff = (e: Econ) => PROCESS / ((RECOVERY * e.price) / OZ);

export interface Values {
  /** Dollars per block if mined, the engine's input to the pit. */
  value: Float64Array;
  /** For the shells: revenue per block at factor 1 (recovered gold of blocks above the cut-off) and cost. */
  revenue: Float64Array;
  cost: Float64Array;
  /** 1 where a block is ore at this price (grade above the cut-off). */
  ore: Uint8Array;
}

/**
 * Block values: ore pays its recovered gold less processing and mining; waste costs its mining. Mining costs more
 * with depth, for the longer haul. The shells scale the revenue with the ore and waste split held at this price.
 */
export function values(grade: Float32Array, e: Econ): Values {
  const value = new Float64Array(N);
  const revenue = new Float64Array(N);
  const cost = new Float64Array(N);
  const ore = new Uint8Array(N);
  const perGram = (RECOVERY * e.price) / OZ;
  for (let k = 0; k < NY; k++) {
    const mine = (e.mining + PER_METRE * (k + 0.5) * H) * BLOCK_T;
    const base = k * COLS;
    for (let c = 0; c < COLS; c++) {
      const b = base + c;
      const r = grade[b]! * perGram * BLOCK_T;
      const p = PROCESS * BLOCK_T;
      const isOre = r > p;
      ore[b] = isOre ? 1 : 0;
      revenue[b] = isOre ? r : 0;
      cost[b] = mine + (isOre ? p : 0);
      value[b] = revenue[b]! - cost[b]!;
    }
  }
  return { value, revenue, cost, ore };
}

export interface Tally {
  oreBlocks: number;
  wasteBlocks: number;
  oreT: number;
  wasteT: number;
  /** Waste tonnes per ore tonne. */
  strip: number;
  /** Mean grade of the ore mined, g/t, and the gold it recovers, oz. */
  grade: number;
  oz: number;
  /** Benches deep at the deepest column. */
  benches: number;
}

/** Tonnes, grade and gold in a set of blocks (the engine's pit or a shell), from the block model. */
export function tally(grade: Float32Array, ore: Uint8Array, inSet: (b: number) => boolean): Tally {
  let oreBlocks = 0;
  let wasteBlocks = 0;
  let grams = 0;
  let deepest = 0;
  for (let b = 0; b < N; b++) {
    if (!inSet(b)) continue;
    const k = Math.floor(b / COLS);
    if (k + 1 > deepest) deepest = k + 1;
    if (ore[b]) {
      oreBlocks++;
      grams += grade[b]! * BLOCK_T;
    } else wasteBlocks++;
  }
  const oreT = oreBlocks * BLOCK_T;
  const wasteT = wasteBlocks * BLOCK_T;
  return {
    oreBlocks,
    wasteBlocks,
    oreT,
    wasteT,
    strip: oreT > 0 ? wasteT / oreT : 0,
    grade: oreT > 0 ? grams / oreT : 0,
    oz: (grams * RECOVERY) / OZ,
    benches: deepest,
  };
}

/** Benches deep in each column (i, j) of a block set: the set is closed under the slope, so a column's mined blocks
 * run from the top down without a gap. */
export function columnDepths(inSet: (b: number) => boolean, out = new Uint8Array(COLS)): Uint8Array {
  out.fill(0);
  for (let c = 0; c < COLS; c++) {
    let k = 0;
    while (k < NY && inSet(c + k * COLS)) k++;
    out[c] = k;
  }
  return out;
}
