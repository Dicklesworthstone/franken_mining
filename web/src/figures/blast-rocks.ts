/**
 * Broken rock for the blast figure, all instanced: the rubble that rides the heave from the bench into the muck pile,
 * the rock thrown off the face and out of the collars on true ballistic arcs (g = 9.81 m/s², real metres and seconds),
 * a little fly rock, and the loose boulders already lying on the benches and at the toes of the faces. No DOM at import.
 */
import * as THREE from "three";
import { morph } from "./blast-ground";
import { rockGeometry } from "./fleet-kit";
import { clamp, rng, smooth } from "./lib";

const G = 9.81;

/** Freshly broken rock: grey to grey-brown, a few warmer pieces. */
export const FRESH = ["#6b645c", "#7a7268", "#5c564f", "#857b6f", "#6e6253", "#504a45", "#8c8273", "#736a5f"];
/** Weathered rock lying on the benches: paler, dustier. */
export const WEATHERED = ["#8d8274", "#9a8f80", "#7f7568", "#a39784", "#857a6c"];

const rockMats: THREE.MeshStandardMaterial[] = [];
/** Matte and faceted, so each block catches the sun on its own planes; variant 1 is freshly broken rock, a little
 * smoother on its fracture faces, so a pile of it does not read as one uniform material. */
export function rubbleMaterial(variant = 0) {
  rockMats[variant] ??= new THREE.MeshStandardMaterial({ roughness: variant ? 0.78 : 0.94, metalness: 0, flatShading: true, envMapIntensity: variant ? 1.15 : 0.9 });
  return rockMats[variant]!;
}

type HeightFn = (x: number, z: number) => number;

export interface Rider {
  kind: "ride";
  /** Where the piece starts on the bench, and where it ends on the muck (x, z). */
  x0: number;
  z0: number;
  x1: number;
  z1: number;
  /** Blast time its rock starts to move, and the dome height there. */
  t0: number;
  bulge: number;
  size: number;
}
export interface Flyer {
  kind: "fly";
  from: THREE.Vector3;
  /** Landing point (y on the finished surface). */
  to: THREE.Vector3;
  t0: number;
  /** Launch speed upwards, m/s. */
  vy: number;
  size: number;
}
export type Piece = Rider | Flyer;

/** The time a ballistic piece takes from launch at y0 with upward speed vy to land at y1. */
export const flightTime = (vy: number, y0: number, y1: number) => (vy + Math.sqrt(Math.max(0, vy * vy + 2 * G * (y0 - y1)))) / G;

export class Rubble {
  readonly group = new THREE.Group();
  private readonly meshes: THREE.InstancedMesh[] = [];
  private readonly items: (Piece & { mesh: number; slot: number; axis: THREE.Vector3; spin: number; q: THREE.Quaternion; T: number; flat: number })[] = [];
  private readonly m4 = new THREE.Matrix4();
  private readonly qa = new THREE.Quaternion();
  private readonly qb = new THREE.Quaternion();
  private readonly p = new THREE.Vector3();
  private readonly sc = new THREE.Vector3();
  private readonly zero = new THREE.Matrix4().makeScale(0, 0, 0);
  /** Blast time after which nothing moves. */
  readonly end: number;
  private posed = NaN;

  constructor(pieces: Piece[], o: { seed?: number; detail?: number; palette?: string[] } = {}) {
    const seed = o.seed ?? 5;
    const r = rng(seed);
    const shapes = 5;
    const buckets: number[][] = Array.from({ length: shapes }, () => []);
    pieces.forEach((_, i) => buckets[Math.floor(r() * shapes)]!.push(i));
    const pal = (o.palette ?? FRESH).map((c) => new THREE.Color(c));
    const c = new THREE.Color();
    let end = 0;
    buckets.forEach((ids, b) => {
      const mesh = new THREE.InstancedMesh(rockGeometry(((seed * 31 + b * 11) % 64) + 1, o.detail ?? 1), rubbleMaterial(b % 2), Math.max(1, ids.length));
      mesh.castShadow = true;
      mesh.receiveShadow = true;
      mesh.frustumCulled = false;
      ids.forEach((id, slot) => {
        const pc = pieces[id]!;
        const axis = new THREE.Vector3(r() - 0.5, r() - 0.5, r() - 0.5).normalize();
        const q = new THREE.Quaternion().setFromEuler(new THREE.Euler((r() - 0.5) * 0.9, r() * Math.PI * 2, (r() - 0.5) * 0.9));
        const T = pc.kind === "fly" ? flightTime(pc.vy, pc.from.y, pc.to.y) : 1.25;
        end = Math.max(end, pc.t0 + T + 0.2);
        this.items.push({ ...pc, mesh: b, slot, axis, spin: 2 + r() * 7, q, T, flat: 0.75 + r() * 0.3 });
        c.copy(pal[Math.floor(r() * pal.length)]!);
        c.offsetHSL((r() - 0.5) * 0.015, (r() - 0.5) * 0.05, (r() - 0.5) * 0.07);
        mesh.setColorAt(slot, c);
      });
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
      this.meshes.push(mesh);
      this.group.add(mesh);
    });
    this.end = end;
  }

  get count() {
    return this.items.length;
  }

  /** Places every piece for blast time t; `surface` is the ground as it is at t, `post` the finished muck. */
  pose(t: number, surface: HeightFn, post: HeightFn) {
    const tt = Math.min(t, this.end + 1);
    if (tt === this.posed) return;
    this.posed = tt;
    for (const it of this.items) {
      const mesh = this.meshes[it.mesh]!;
      if (t < it.t0) {
        mesh.setMatrixAt(it.slot, this.zero);
        continue;
      }
      const tau = t - it.t0;
      let grow = 1;
      let turn = 0;
      if (it.kind === "ride") {
        // Breaks out of the bench as the dome lifts, rides the swell and slumps forward onto the muck.
        const m = morph(tau, it.bulge);
        const g = m.go;
        this.p.set(it.x0 + (it.x1 - it.x0) * g, 0, it.z0 + (it.z1 - it.z0) * g);
        this.p.y = (g >= 1 ? post(this.p.x, this.p.z) : surface(this.p.x, this.p.z)) + it.size * 0.1;
        grow = smooth((tau - 0.02) / 0.22);
        turn = g * 0.9;
      } else {
        const f = Math.min(tau, it.T);
        const k = f / it.T;
        this.p.lerpVectors(it.from, it.to, k);
        // True ballistic height: launch speed up, gravity down, landing on the muck.
        this.p.y = it.from.y + it.vy * f - 0.5 * G * f * f;
        if (tau >= it.T) this.p.y = it.to.y;
        grow = 0.4 + 0.6 * smooth(tau / 0.05);
        turn = Math.min(tau, it.T) * 1.2;
      }
      if (grow <= 0.001) {
        mesh.setMatrixAt(it.slot, this.zero);
        continue;
      }
      this.qa.setFromAxisAngle(it.axis, turn * it.spin * 0.35);
      this.qb.copy(it.q).premultiply(this.qa);
      this.sc.set(it.size * grow, it.size * grow * it.flat, it.size * grow);
      this.m4.compose(this.p, this.qb, this.sc);
      mesh.setMatrixAt(it.slot, this.m4);
    }
    for (const m of this.meshes) m.instanceMatrix.needsUpdate = true;
  }
}

/** Loose boulders lying still: spillage on the benches, scree at the toes of faces. */
export function scatter(points: { x: number; y: number; z: number; size: number }[], o: { seed?: number; detail?: number; palette?: string[] } = {}) {
  const seed = o.seed ?? 13;
  const r = rng(seed);
  const shapes = 4;
  const buckets: number[][] = Array.from({ length: shapes }, () => []);
  points.forEach((_, i) => buckets[Math.floor(r() * shapes)]!.push(i));
  const pal = (o.palette ?? WEATHERED).map((c) => new THREE.Color(c));
  const group = new THREE.Group();
  const m4 = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const e = new THREE.Euler();
  const v = new THREE.Vector3();
  const s = new THREE.Vector3();
  const c = new THREE.Color();
  buckets.forEach((ids, b) => {
    if (!ids.length) return;
    const mesh = new THREE.InstancedMesh(rockGeometry(((seed * 17 + b * 5) % 64) + 1, o.detail ?? 1), rubbleMaterial(), ids.length);
    ids.forEach((id, slot) => {
      const p = points[id]!;
      e.set((r() - 0.5) * 0.6, r() * Math.PI * 2, (r() - 0.5) * 0.6);
      q.setFromEuler(e);
      v.set(p.x, p.y, p.z);
      s.set(p.size, p.size * (0.7 + r() * 0.3), p.size);
      m4.compose(v, q, s);
      mesh.setMatrixAt(slot, m4);
      c.copy(pal[Math.floor(r() * pal.length)]!);
      c.offsetHSL(0, (r() - 0.5) * 0.04, (r() - 0.5) * 0.06);
      mesh.setColorAt(slot, c);
    });
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    mesh.computeBoundingSphere();
    group.add(mesh);
  });
  return group;
}

export const sizeOf = (r: () => number, lo: number, hi: number, k = 2.8) => lo + (hi - lo) * clamp(r(), 0, 1) ** k;
