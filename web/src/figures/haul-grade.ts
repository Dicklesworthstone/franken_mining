/**
 * Grades for the haul scene from FrankenMining's own engine (engine.ts, the Rust kernels compiled to wasm): each
 * face's blast-hole samples condition fmine-calib's spatial predictor, which gives every grade-control block a mean,
 * a P10–P90 range and the chance it is above the cut-off. The samples are synthetic (a made-up face on a made-up
 * mine); the estimates are the engine's, computed in the page. Nothing here touches the DOM.
 */
import * as THREE from "three";
import { cvar, estimate, ready } from "./engine";
import { rng } from "./lib";
import type { Loader } from "./opencut";

export const CUTOFF = 0.5;
/** The registered variogram for both faces, in ln(g/t)². */
export const VARIOGRAM = { nugget: 0.04, sill: 0.2, rangeM: 35, neighbors: 12 };

export interface BlockEstimate {
  /** Mean grade (lognormal mean), and the P10 and P90, g/t. */
  mean: number;
  p10: number;
  p90: number;
  /** Chance the block is at or above the cut-off. */
  pCut: number;
  /** Where the block is, and the face it belongs to. */
  at: THREE.Vector3;
  face: string;
}

export interface FaceGrades {
  blocks: Map<string, BlockEstimate>;
  holes: { at: THREE.Vector3; g: number; face: string }[];
  /** Wall time of the engine calls, ms, and how many blocks and samples went in. */
  ms: number;
  queries: number;
  samples: number;
}

const BLOCK = 12;
const PER_ROW = 6;
const FIRST = 13;

/** The face's frame: its centre, the way along it and the way into it. */
function frame(X: Loader) {
  const c = X.muck.position.clone();
  const into = c.clone().sub(X.group.position).setY(0).normalize();
  const along = new THREE.Vector3(into.z, 0, -into.x);
  return { c, into, along };
}

/** Where grade-control block `n` of a face lies: six 12 m blocks across the face, rows going into it. */
export function blockAt(X: Loader, n: number, H: (x: number, z: number) => number) {
  const { c, into, along } = frame(X);
  const i = Math.max(0, n - FIRST);
  const p = c
    .clone()
    .addScaledVector(along, ((i % PER_ROW) - (PER_ROW - 1) / 2) * BLOCK)
    .addScaledVector(into, Math.floor(i / PER_ROW) * 10 - 6);
  return p.setY(H(p.x, p.z));
}

export const blockName = (X: Loader, n: number) => `${X.face.replace("-", "")}-${String(n).padStart(2, "0")}`;

// Standard normal CDF (Abramowitz and Stegun 7.1.26), enough for a chance to two figures.
const phi = (z: number) => {
  const t = 1 / (1 + 0.3275911 * Math.abs(z) / Math.SQRT2);
  const y = 1 - ((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * Math.exp(-(z * z) / 2);
  return z >= 0 ? (1 + y) / 2 : (1 - y) / 2;
};

/**
 * Blast-hole samples for each face (synthetic: a smooth field plus a nugget), then one engine call per face for the
 * blocks the shovel will dig this shift.
 */
export async function gradeFaces(faces: Loader[], H: (x: number, z: number) => number, count = 60): Promise<FaceGrades> {
  const blocks = new Map<string, BlockEstimate>();
  const holes: FaceGrades["holes"] = [];
  let ms = 0;
  let queries = 0;
  let samples = 0;
  for (const X of faces) {
    const r = rng(X.id === "S1" ? 31 : 47);
    const { c, into, along } = frame(X);
    const base = Math.log(X.material === "ore" ? 1.45 : 0.11);
    const field = (a: number, o: number) => 0.34 * Math.sin(0.07 * a + 1.3) + 0.24 * Math.cos(0.11 * o - 0.4) + 0.14 * Math.sin(0.05 * (a + o));
    const gauss = () => Math.sqrt(-2 * Math.log(Math.max(1e-9, r()))) * Math.cos(2 * Math.PI * r());
    const list = [];
    // An 8 m by 9 m pattern over the blasted face and the next blast behind it.
    for (let a = -40; a <= 40; a += 8) {
      for (let o = -14; o <= 46; o += 9) {
        const p = c.clone().addScaledVector(along, a + (r() - 0.5) * 1.2).addScaledVector(into, o + (r() - 0.5) * 1.2);
        p.y = H(p.x, p.z);
        const v = base + field(a, o) + gauss() * Math.sqrt(VARIOGRAM.nugget);
        list.push({ x: p.x, y: -p.z, z: p.y, v });
        holes.push({ at: p, g: Math.exp(v), face: X.face });
      }
    }
    const q: number[] = [];
    const names: string[] = [];
    const where: THREE.Vector3[] = [];
    for (let n = FIRST; n < FIRST + count; n++) {
      const p = blockAt(X, n, H);
      q.push(p.x, -p.z, p.y);
      names.push(blockName(X, n));
      where.push(p);
    }
    const res = await estimate({ ...VARIOGRAM, samples: list, prior: base, queries: q });
    ms += res.ms;
    queries += names.length;
    samples += list.length;
    names.forEach((name, i) => {
      const m = res.mean[i]!;
      const sd = res.sd[i]!;
      if (!Number.isFinite(m) || !Number.isFinite(sd)) return;
      blocks.set(name, {
        mean: Math.exp(m + (sd * sd) / 2),
        p10: Math.exp(m - 1.2816 * sd),
        p90: Math.exp(m + 1.2816 * sd),
        pCut: 1 - phi((Math.log(CUTOFF) - m) / Math.max(1e-6, sd)),
        at: where[i]!,
        face: X.face,
      });
    });
  }
  return { blocks, holes, ms, queries, samples };
}

/** The tail of a series of losses from the engine (fmine-numerics): mean, VaR and CVaR at `alpha`, and the call's time. */
export async function lossTail(losses: number[], alpha = 0.9) {
  await ready();
  const t = performance.now();
  const r = await cvar(losses, alpha);
  return { ...r, ms: performance.now() - t, n: losses.length };
}
