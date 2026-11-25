/**
 * The rock of the underground figure: every excavation swept from its centreline into a rough arched tunnel whose
 * inside faces inwards, so from outside the ground you look straight into it (the near wall drops away, the far wall
 * and floor stay), with a faint film over the near wall that outlines each tube. Junctions are opened by dropping the
 * triangles of one excavation that fall inside another. Also the raises, the cutaway block of ground behind the
 * mine, and the materials (rock with bolts and mesh drawn in the shader, and the ground-support colours).
 */
import * as THREE from "three";
import { Noise3, noiseTexture } from "./fleet-kit";
import { clamp } from "./lib";
import { BOX } from "./underground-plan";

/* ---------- Excavations ---------- */

export type Kind = "decline" | "access" | "drive" | "xc" | "stub" | "raise";
const RANK: Record<Kind, number> = { decline: 0, access: 1, drive: 2, xc: 3, stub: 4, raise: 5 };

export interface TunnelSpec {
  id: string;
  kind: Kind;
  /** Floor centreline, in order; corners are rounded by `route`. */
  pts: THREE.Vector3[];
  w: number;
  h: number;
  end?: "face" | "barricade" | "open";
  /** Ground-support status code over the last `length` metres (1 verified, 2 due, 3 overdue) and the heading's index. */
  zone?: { status: number; length: number; heading: number };
  /** Tint the walls as ore between these z (for crosscuts that cross the lens). */
  ore?: (p: THREE.Vector3) => boolean;
  /** No bolts or mesh (old, shotcreted, or raise-bored). */
  bare?: boolean;
  /** Fibre-reinforced shotcrete over the bolts (the decline and level accesses). */
  shotcrete?: boolean;
}

export interface RaiseSpec {
  id: string;
  top: THREE.Vector3;
  bottom: THREE.Vector3;
  r: number;
  lined?: boolean;
}

/** Something that can contain a point: a tunnel, a raise, an open stope. */
export interface Solid {
  rank: number;
  box: THREE.Box3;
  inside(p: THREE.Vector3, floorOnly?: boolean): boolean;
}

/** Rounds the corners of a polyline with quadratic curves of radius `r` and resamples it every `step` metres. */
export function route(pts: THREE.Vector3[], r = 6, step = 1.6): THREE.Vector3[] {
  if (pts.length < 2) return pts.map((p) => p.clone());
  const path: THREE.Vector3[] = [pts[0]!.clone()];
  const curve = (a: THREE.Vector3, b: THREE.Vector3, c: THREE.Vector3) => {
    const ab = a.distanceTo(b);
    const bc = b.distanceTo(c);
    const k = Math.min(r, ab * 0.45, bc * 0.45);
    const p0 = b.clone().lerp(a, k / ab);
    const p2 = b.clone().lerp(c, k / bc);
    path.push(p0);
    const n = Math.max(3, Math.ceil(k / 0.9));
    for (let i = 1; i < n; i++) {
      const t = i / n;
      path.push(
        new THREE.Vector3()
          .addScaledVector(p0, (1 - t) ** 2)
          .addScaledVector(b, 2 * (1 - t) * t)
          .addScaledVector(p2, t * t),
      );
    }
    path.push(p2);
  };
  for (let i = 1; i < pts.length - 1; i++) curve(pts[i - 1]!, pts[i]!, pts[i + 1]!);
  path.push(pts[pts.length - 1]!.clone());
  // Resample evenly.
  const out: THREE.Vector3[] = [path[0]!.clone()];
  let carry = 0;
  for (let i = 1; i < path.length; i++) {
    const a = path[i - 1]!;
    const b = path[i]!;
    const d = a.distanceTo(b);
    let at = step - carry;
    while (at < d) {
      out.push(a.clone().lerp(b, at / d));
      at += step;
    }
    carry = d - (at - step);
  }
  const last = path[path.length - 1]!;
  if (out[out.length - 1]!.distanceTo(last) > step * 0.35) out.push(last.clone());
  else out[out.length - 1]!.copy(last);
  return out;
}

/** The arched profile: floor, walls, a 0.32-width arch. Points go round once; normals point out of the rock-free space. */
function profile(w: number, h: number) {
  const R = 0.32 * w;
  const hw = h - R;
  const pts: { x: number; y: number; nx: number; ny: number }[] = [];
  const add = (x: number, y: number, nx: number, ny: number) => {
    const l = Math.hypot(nx, ny);
    pts.push({ x, y, nx: nx / l, ny: ny / l });
  };
  add(w / 2 - 0.22, 0, 0, -1);
  add(w / 2, 0.3, 1, 0);
  add(w / 2, hw * 0.5, 1, 0);
  add(w / 2, hw, 1, 0);
  const A = 12;
  for (let i = 1; i < A; i++) {
    const t = (i / A) * Math.PI;
    add((w / 2) * Math.cos(t), hw + R * Math.sin(t), Math.cos(t) / (w / 2), Math.sin(t) / R);
  }
  add(-w / 2, hw, -1, 0);
  add(-w / 2, hw * 0.5, -1, 0);
  add(-w / 2, 0.3, -1, 0);
  add(-w / 2 + 0.22, 0, 0, -1);
  add(-w / 6, 0.02, 0, -1);
  add(w / 6, 0.02, 0, -1);
  // Arc length round the ring, for the bolt and mesh patterns.
  const v: number[] = [0];
  for (let i = 1; i <= pts.length; i++) {
    const a = pts[i - 1]!;
    const b = pts[i % pts.length]!;
    v.push(v[i - 1]! + Math.hypot(b.x - a.x, b.y - a.y));
  }
  const roof = (lat: number) => {
    const u = Math.abs(lat) / (w / 2);
    return u >= 1 ? 0 : hw + R * Math.sqrt(1 - u * u);
  };
  return { pts, v, hw, R, roof };
}

/** How far the rough wall stands out (+) or in (−) from the clean profile at profile point q of the ring at o. */
function wallDisp(o: THREE.Vector3, q: { x: number; y: number }) {
  return noise.noise(o.x * 0.35 + q.x * 0.35, o.y * 0.35 + q.y * 0.35, o.z * 0.35) * 0.32 + noise.noise(o.x * 1.3, (o.y + q.y) * 1.3, o.z * 1.3 + q.x) * 0.08;
}

/**
 * The rough ring of an excavation at floor-centreline point o, as the rock mesh draws it: (lateral, height) points
 * round the profile, lateral positive on the `right` side. For hanging services off the real wall and back.
 */
export function ringAt(w: number, h: number, o: THREE.Vector3) {
  const prof = profile(w, h);
  return prof.pts.map((q) => {
    const d = q.y > 0.05 ? wallDisp(o, q) : 0;
    return { x: q.x + q.nx * d, y: q.y + q.ny * d };
  });
}
/** On the rough ring: the wall's lateral distance from the centreline at height y on one side (side ±1). */
export function wallAt(ring: { x: number; y: number }[], side: 1 | -1, y: number) {
  let best = Infinity;
  for (let i = 0; i < ring.length; i++) {
    const a = ring[i]!;
    const b = ring[(i + 1) % ring.length]!;
    if (Math.sign(a.x + b.x) !== side || (a.y - y) * (b.y - y) > 0 || a.y === b.y) continue;
    const t = (y - a.y) / (b.y - a.y);
    best = Math.min(best, Math.abs(a.x + (b.x - a.x) * t));
  }
  return best;
}
/** On the rough ring: the height of the back (roof) at lateral x. */
export function backAt(ring: { x: number; y: number }[], x: number) {
  let best = Infinity;
  for (let i = 0; i < ring.length; i++) {
    const a = ring[i]!;
    const b = ring[(i + 1) % ring.length]!;
    if (a.y < 1 || b.y < 1 || (a.x - x) * (b.x - x) > 0 || a.x === b.x) continue;
    const t = (x - a.x) / (b.x - a.x);
    best = Math.min(best, a.y + (b.y - a.y) * t);
  }
  return best;
}

/** Accumulates the merged geometry of every excavation. */
class Builder {
  pos: number[] = [];
  uv: number[] = [];
  aux: number[] = [];
  col: number[] = [];
  idx: number[] = [];
  vertex(p: THREE.Vector3, u: number, v: number, a: [number, number, number, number], c: THREE.Color) {
    this.pos.push(p.x, p.y, p.z);
    this.uv.push(u, v);
    this.aux.push(...a);
    this.col.push(c.r, c.g, c.b);
    return this.pos.length / 3 - 1;
  }
  geometry() {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(this.pos, 3));
    g.setAttribute("uv", new THREE.Float32BufferAttribute(this.uv, 2));
    g.setAttribute("aux", new THREE.Float32BufferAttribute(this.aux, 4));
    g.setAttribute("color", new THREE.Float32BufferAttribute(this.col, 3));
    g.setIndex(this.idx);
    g.computeVertexNormals();
    g.computeBoundingSphere();
    return g;
  }
}

interface Swept {
  spec: TunnelSpec;
  pts: THREE.Vector3[];
  right: THREE.Vector3[];
  roof: (lat: number) => number;
  solid: Solid;
}

const noise = new Noise3(23);
const ROCK = new THREE.Color("#57524c");
const ROCK_B = new THREE.Color("#403c38");
const FLOOR = new THREE.Color("#3a332c");
const OREC = new THREE.Color("#735d3c");
const FACE = new THREE.Color("#665d53");
const PASTE = new THREE.Color("#a29d92");
const SHOT = new THREE.Color("#8a8780");

/** A uniform grid over many solids' segments, so a point finds the few that could hold it. */
class Grid {
  cells = new Map<string, { s: number; i: number }[]>();
  constructor(readonly size: number) {}
  key(x: number, y: number, z: number) {
    return `${Math.floor(x / this.size)},${Math.floor(y / this.size)},${Math.floor(z / this.size)}`;
  }
  add(box: THREE.Box3, entry: { s: number; i: number }) {
    const S = this.size;
    for (let x = Math.floor(box.min.x / S); x <= Math.floor(box.max.x / S); x++)
      for (let y = Math.floor(box.min.y / S); y <= Math.floor(box.max.y / S); y++)
        for (let z = Math.floor(box.min.z / S); z <= Math.floor(box.max.z / S); z++) {
          const k = `${x},${y},${z}`;
          const list = this.cells.get(k);
          if (list) list.push(entry);
          else this.cells.set(k, [entry]);
        }
  }
  at(p: THREE.Vector3) {
    return this.cells.get(this.key(p.x, p.y, p.z));
  }
}

/** Prepares a tunnel: its resampled centreline, frames and the solid that tests points against it. */
export function prepare(spec: TunnelSpec): Swept {
  const pts = route(spec.pts, spec.kind === "decline" ? 2 : 7, 1.5);
  const right: THREE.Vector3[] = [];
  const Y = new THREE.Vector3(0, 1, 0);
  for (let i = 0; i < pts.length; i++) {
    const a = pts[Math.max(0, i - 1)]!;
    const b = pts[Math.min(pts.length - 1, i + 1)]!;
    const t = b.clone().sub(a);
    t.y = 0;
    t.normalize();
    right.push(new THREE.Vector3().crossVectors(t, Y).normalize());
  }
  const { roof } = profile(spec.w, spec.h);
  const box = new THREE.Box3();
  for (const p of pts) box.expandByPoint(p);
  box.min.addScalar(-spec.w);
  box.max.addScalar(spec.w);
  box.max.y += spec.h;
  const tmp = new THREE.Vector3();
  const segInside = (p: THREE.Vector3, i: number, floorOnly: boolean) => {
    const a = pts[i]!;
    const b = pts[i + 1]!;
    const ab = tmp.copy(b).sub(a);
    const l2 = ab.lengthSq();
    const t = l2 > 0 ? ((p.x - a.x) * ab.x + (p.y - a.y) * ab.y + (p.z - a.z) * ab.z) / l2 : 0;
    // Only a face end is closed; an open end lets the volume run on a little.
    const lo = i === 0 ? (spec.kind === "access" || spec.kind === "xc" || spec.kind === "stub" ? 0 : -0.02) : -0.02;
    const hi = i === pts.length - 2 ? (spec.end === "open" ? 1.3 : 1.0) : 1.02;
    if (t < lo || t > hi) return false;
    const px = a.x + ab.x * t;
    const py = a.y + ab.y * t;
    const pz = a.z + ab.z * t;
    const r = right[i]!;
    const lat = (p.x - px) * r.x + (p.z - pz) * r.z;
    const up = p.y - py;
    if (Math.abs(lat) > spec.w / 2 - 0.12) return false;
    if (floorOnly) return up > -0.6 && up < 0.6;
    return up > -0.6 && up < roof(lat) - 0.1;
  };
  const solid: Solid = {
    rank: RANK[spec.kind],
    box,
    inside(p, floorOnly = false) {
      if (!box.containsPoint(p)) return false;
      for (let i = 0; i < pts.length - 1; i++) if (segInside(p, i, floorOnly)) return true;
      return false;
    },
  };
  // A faster test through the network's grid is set up by `network`.
  (solid as Solid & { seg: typeof segInside }).seg = segInside;
  return { spec, pts, right, roof, solid };
}

function raiseSolid(r: RaiseSpec): Solid {
  const box = new THREE.Box3().setFromPoints([r.top, r.bottom]);
  box.min.addScalar(-r.r);
  box.max.addScalar(r.r);
  const axis = r.bottom.clone().sub(r.top);
  const L = axis.length();
  axis.normalize();
  const d = new THREE.Vector3();
  return {
    rank: RANK.raise,
    box,
    inside(p) {
      d.copy(p).sub(r.top);
      const t = d.dot(axis);
      if (t < -0.3 || t > L + 0.3) return false;
      d.addScaledVector(axis, -t);
      return d.length() < r.r - 0.08;
    },
  };
}

/**
 * Opens a swept grid surface (rows × cols vertices, columns wrapping round) where another excavation passes through
 * it: the vertices inside are pulled onto that excavation's boundary by bisection along a grid edge to their nearest
 * outside neighbour, so the opening has a clean edge instead of a saw-tooth of whole triangles. Returns each vertex's
 * state: 0 outside, 1 pulled onto the boundary, 2 inside (drop every triangle that uses one, and those with all three
 * on the boundary).
 */
function openGrid(B: Builder, ids: number[], rows: number, cols: number, inside: (p: THREE.Vector3, k: number) => boolean) {
  const n = ids.length;
  const state = new Uint8Array(n);
  const va = new THREE.Vector3();
  const vb = new THREE.Vector3();
  const vm = new THREE.Vector3();
  const at = (k: number, out: THREE.Vector3) => out.fromArray(B.pos, ids[k]! * 3);
  for (let k = 0; k < n; k++) state[k] = inside(at(k, va), k) ? 2 : 0;
  const pulls: { k: number; p: THREE.Vector3 }[] = [];
  for (let k = 0; k < n; k++) {
    if (state[k] !== 2) continue;
    const i = Math.floor(k / cols);
    const j = k % cols;
    at(k, va);
    let best: THREE.Vector3 | null = null;
    let bestD = Infinity;
    for (const [di, dj] of [
      [-1, 0],
      [1, 0],
      [0, -1],
      [0, 1],
    ] as const) {
      const ii = i + di;
      if (ii < 0 || ii >= rows) continue;
      const kk = ii * cols + ((j + dj + cols) % cols);
      if (state[kk] !== 0) continue;
      at(kk, vb);
      let lo = 0;
      let hi = 1;
      for (let it = 0; it < 9; it++) {
        const mid = (lo + hi) / 2;
        if (inside(vm.copy(va).lerp(vb, mid), k)) lo = mid;
        else hi = mid;
      }
      const d = hi * va.distanceTo(vb);
      if (d < bestD) {
        bestD = d;
        best = va.clone().lerp(vb, hi);
      }
    }
    if (best) pulls.push({ k, p: best });
  }
  for (const { k, p } of pulls) {
    const v = ids[k]! * 3;
    B.pos[v] = p.x;
    B.pos[v + 1] = p.y;
    B.pos[v + 2] = p.z;
    state[k] = 1;
  }
  return state;
}

export interface Network {
  geometry: THREE.BufferGeometry;
  /** Each excavation's centreline (resampled), by id. */
  lines: Map<string, { pts: THREE.Vector3[]; right: THREE.Vector3[]; spec: TunnelSpec }>;
  solids: Solid[];
  /** Where to put lamps along the main drives: position and the wall's inward normal. */
  lamps: { p: THREE.Vector3; n: THREE.Vector3; floor: number }[];
  /** Whether a point is in the open: inside any excavation, raise or open stope. */
  open(p: THREE.Vector3): boolean;
}

/**
 * Builds every tunnel and raise into one geometry. `extra` are solids that cut the tunnels (open stopes) but are drawn
 * elsewhere; `cut` receives the network's own solids so the open stopes can be opened where tunnels meet them.
 */
export function network(tunnels: TunnelSpec[], raises: RaiseSpec[], extra: Solid[]): Network {
  const swept = tunnels.map(prepare);
  const rs = raises.map(raiseSolid);
  const solids: Solid[] = [...swept.map((s) => s.solid), ...rs, ...extra];
  // Grid of tunnel segments for quick lookups.
  const grid = new Grid(10);
  swept.forEach((sw, s) => {
    const b = new THREE.Box3();
    for (let i = 0; i < sw.pts.length - 1; i++) {
      b.makeEmpty();
      b.expandByPoint(sw.pts[i]!);
      b.expandByPoint(sw.pts[i + 1]!);
      b.min.addScalar(-sw.spec.w);
      b.max.addScalar(sw.spec.w);
      b.max.y += sw.spec.h;
      grid.add(b, { s, i });
    }
  });
  type Seg = (p: THREE.Vector3, i: number, floorOnly: boolean) => boolean;
  /** Whether p lies inside any excavation other than `self` (with the rank rule for floors). */
  const insideOther = (p: THREE.Vector3, self: number, floor: boolean, rank: number) => {
    const list = grid.at(p);
    if (list)
      for (const { s, i } of list) {
        if (s === self) continue;
        const other = swept[s]!;
        // Floors: the lower-ranked excavation keeps its floor where two meet.
        if (floor && other.solid.rank >= rank) continue;
        if ((other.solid as Solid & { seg: Seg }).seg(p, i, false)) return true;
      }
    for (const r of rs) if (r.box.containsPoint(p) && r.inside(p)) return true;
    for (const e of extra) if (e.box.containsPoint(p) && e.inside(p)) return true;
    return false;
  };

  const B = new Builder();
  const lines = new Map<string, { pts: THREE.Vector3[]; right: THREE.Vector3[]; spec: TunnelSpec }>();
  const lamps: { p: THREE.Vector3; n: THREE.Vector3; floor: number }[] = [];
  const c = new THREE.Color();
  const p = new THREE.Vector3();

  swept.forEach((sw, self) => {
    const { spec, pts, right } = sw;
    lines.set(spec.id, { pts, right, spec });
    const prof = profile(spec.w, spec.h);
    const N = prof.pts.length;
    const rank = RANK[spec.kind];
    // Distance along, and from the far end (for the face's support zone).
    const along: number[] = [0];
    for (let i = 1; i < pts.length; i++) along.push(along[i - 1]! + pts[i]!.distanceTo(pts[i - 1]!));
    const total = along[along.length - 1]!;
    const local: number[] = [];
    const first = B.pos.length / 3;
    for (let i = 0; i < pts.length; i++) {
      const o = pts[i]!;
      const r = right[i]!;
      const toFace = spec.end === "face" || spec.end === "barricade" ? total - along[i]! : 99;
      const zone = spec.zone && total - along[i]! <= spec.zone.length ? spec.zone.status : 0;
      for (let j = 0; j < N; j++) {
        const q = prof.pts[j]!;
        const wall = q.y > 0.05 ? 1 : 0;
        const d = wall ? wallDisp(o, q) : 0;
        p.copy(o).addScaledVector(r, q.x + q.nx * d);
        p.y += q.y + q.ny * d + (wall ? 0 : 0.015);
        const n = noise.noise(p.x * 0.08, p.y * 0.11, p.z * 0.08);
        c.copy(wall ? ROCK : FLOOR).lerp(ROCK_B, clamp(0.5 + n, 0, 1) * (wall ? 0.6 : 0.3));
        if (spec.ore?.(p) && wall) c.lerp(OREC, 0.75);
        // Shotcrete covers the walls and backs; the bottom half-metre of the walls stays rock and mud.
        if (spec.shotcrete && q.y > 0.2) c.copy(SHOT).multiplyScalar(0.88 + n * 0.2);
        local.push(B.vertex(p, along[i]!, prof.v[j]!, [q.y, zone, toFace, spec.bare ? 3 : spec.shotcrete && q.y > 0.2 ? 5 : 0], c));
      }
    }
    // Where another excavation opens through this one, the opening gets a clean edge (see `openGrid`).
    const state = openGrid(B, local, pts.length, N, (q, k) => insideOther(q, self, B.aux[local[k]! * 4]! < 0.1, rank));
    const tri = (ka: number, kb: number, kc: number) => {
      const sa = state[ka]!;
      const sb = state[kb]!;
      const sc = state[kc]!;
      if (sa === 2 || sb === 2 || sc === 2 || (sa && sb && sc)) return;
      B.idx.push(local[ka]!, local[kb]!, local[kc]!);
    };
    for (let i = 0; i < pts.length - 1; i++)
      for (let j = 0; j < N; j++) {
        const a = i * N + j;
        const b = i * N + ((j + 1) % N);
        const cc = (i + 1) * N + j;
        const dd = (i + 1) * N + ((j + 1) % N);
        tri(a, b, cc);
        tri(b, dd, cc);
      }
    // The end: a rough rock face, or a fill barricade.
    if (spec.end === "face" || spec.end === "barricade") {
      const i = pts.length - 1;
      const o = pts[i]!;
      const t = o.clone().sub(pts[i - 1]!).setY(0).normalize();
      const rock = spec.end === "face";
      const K = 5;
      const centre = o.clone().add(new THREE.Vector3(0, spec.h * 0.42, 0));
      const ring0: THREE.Vector3[] = [];
      for (let j = 0; j < N; j++) ring0.push(new THREE.Vector3(B.pos[local[i * N + j]! * 3]!, B.pos[local[i * N + j]! * 3 + 1]!, B.pos[local[i * N + j]! * 3 + 2]!));
      const rings: number[][] = [];
      for (let k = 0; k <= K; k++) {
        const s = 1 - k / K;
        const ring: number[] = [];
        for (let j = 0; j < N; j++) {
          p.copy(centre).lerp(ring0[j]!, s);
          const e = 1 - s;
          const push = rock ? noise.noise(p.x * 0.6, p.y * 0.6, p.z * 0.6) * 0.45 * Math.sin(Math.PI * Math.min(1, e * 1.4)) + e * 0.25 : -e * 0.12;
          p.addScaledVector(t, push);
          c.copy(rock ? FACE : PASTE).multiplyScalar(0.9 + noise.noise(p.x * 0.9, p.y * 0.9, p.z) * 0.25);
          if (rock && spec.ore?.(p)) c.lerp(OREC, 0.7);
          ring.push(B.vertex(p, along[i]! + push, prof.v[j]! * s, [Math.max(0.2, p.y - o.y), spec.zone?.status ?? 0, 0, rock ? 1 : 2], c));
          if (k === K) break;
        }
        rings.push(ring);
      }
      for (let k = 0; k < K; k++) {
        const a = rings[k]!;
        const b = rings[k + 1]!;
        for (let j = 0; j < N; j++) {
          const j1 = (j + 1) % N;
          // Wound like the walls, so a face shows from inside the tunnel and drops away from behind.
          if (b.length === 1) B.idx.push(a[j]!, a[j1]!, b[0]!);
          else {
            B.idx.push(a[j]!, a[j1]!, b[j]!);
            B.idx.push(b[j]!, a[j1]!, b[j1]!);
          }
        }
      }
    }
    // Make sure the faces point into the tunnel (the floor's normal should be up).
    void first;
    // Lamps on the main drives and the decline.
    if (spec.kind === "drive" || spec.kind === "decline" || spec.kind === "access") {
      const every = spec.kind === "decline" ? 46 : spec.kind === "access" ? 26 : 30;
      for (let s = 8; s < total - 6; s += every) {
        let i = along.findIndex((a) => a >= s);
        if (i < 0) i = pts.length - 1;
        const o = pts[i]!;
        const r = right[i]!;
        // On the vertical part of the wall, which is never culled when seen from inside; never where another
        // excavation opens through that wall (the lamp would hang in the junction).
        const p = o.clone().addScaledVector(r, -(spec.w / 2 - 0.3)).add(new THREE.Vector3(0, Math.min(3.0, prof.hw - 0.4), 0));
        const t = (i + 1 < pts.length ? pts[i + 1]!.clone().sub(o) : o.clone().sub(pts[i - 1]!)).setY(0).normalize();
        const open = [-2.5, 0, 2.5].some((k) => insideOther(p.clone().addScaledVector(r, -0.7).addScaledVector(t, k), self, false, rank));
        if (!open) lamps.push({ p, n: r.clone(), floor: o.y });
      }
    }
  });

  // Raises: a circle swept down the axis, faces inward.
  raises.forEach((r, ri) => {
    const axis = r.bottom.clone().sub(r.top);
    const L = axis.length();
    axis.normalize();
    const a1 = new THREE.Vector3(1, 0, 0).addScaledVector(axis, -axis.x).normalize();
    const a2 = new THREE.Vector3().crossVectors(axis, a1).normalize();
    const M = 18;
    const rows = Math.ceil(L / 2);
    const base: number[] = [];
    for (let i = 0; i <= rows; i++) {
      const t = (i / rows) * L;
      for (let j = 0; j < M; j++) {
        const ang = (j / M) * Math.PI * 2;
        const rr = r.r * (1 + (r.lined ? 0 : noise.noise(t * 0.4, j * 0.7, ri * 3) * 0.12));
        p.copy(r.top).addScaledVector(axis, t).addScaledVector(a1, Math.cos(ang) * rr).addScaledVector(a2, Math.sin(ang) * rr);
        c.copy(r.lined ? new THREE.Color("#8e8a83") : ROCK).multiplyScalar(0.85 + noise.noise(p.x * 0.1, p.y * 0.1, p.z * 0.1) * 0.3);
        base.push(B.vertex(p, t, ang * r.r, [3, 0, 99, r.lined ? 4 : 3], c));
      }
    }
    // Where a tunnel meets the raise, the raise's wall opens with a clean edge, like the tunnels' junctions.
    const inTunnel = (q: THREE.Vector3) => {
      const list = grid.at(q);
      if (list) for (const { s, i: k } of list) if ((swept[s]!.solid as Solid & { seg: Seg }).seg(q, k, false)) return true;
      return false;
    };
    const state = openGrid(B, base, rows + 1, M, (q) => inTunnel(q));
    const tri = (ka: number, kb: number, kc: number) => {
      const sa = state[ka]!;
      const sb = state[kb]!;
      const sc = state[kc]!;
      if (sa === 2 || sb === 2 || sc === 2 || (sa && sb && sc)) return;
      B.idx.push(base[ka]!, base[kb]!, base[kc]!);
    };
    for (let i = 0; i < rows; i++)
      for (let j = 0; j < M; j++) {
        const a = i * M + j;
        const b = i * M + ((j + 1) % M);
        const cc = (i + 1) * M + j;
        const dd = (i + 1) * M + ((j + 1) % M);
        tri(a, cc, b);
        tri(b, cc, dd);
      }
  });

  const geometry = B.geometry();
  // Check the winding on a floor: the shader expects normals pointing into the open space.
  const n = geometry.getAttribute("normal");
  const aux = geometry.getAttribute("aux");
  let up = 0;
  for (let i = 0; i < Math.min(n.count, 4000); i++) if (aux.getX(i) < 0.05) up += n.getY(i);
  if (up < 0) {
    const ix = geometry.getIndex()!;
    const arr = ix.array as Uint32Array | Uint16Array;
    for (let i = 0; i < arr.length; i += 3) {
      const t = arr[i]!;
      arr[i] = arr[i + 2]!;
      arr[i + 2] = t;
    }
    ix.needsUpdate = true;
    geometry.computeVertexNormals();
  }
  // Filled in by `bakeLamps`.
  geometry.setAttribute("lampL", new THREE.Float32BufferAttribute(new Float32Array(geometry.getAttribute("position").count * 4), 4));
  const open = (q: THREE.Vector3) => {
    const list = grid.at(q);
    if (list) for (const { s, i } of list) if ((swept[s]!.solid as Solid & { seg: Seg }).seg(q, i, false)) return true;
    for (const r of rs) if (r.box.containsPoint(q) && r.inside(q)) return true;
    for (const e of extra) if (e.box.containsPoint(q) && e.inside(q)) return true;
    return false;
  };
  return { geometry, lines, solids, lamps, open };
}

/**
 * Bakes the wall lamps' light into the rock, per vertex: irradiance from each lamp within 15 m (inverse square, the
 * cosine at the rock, the fitting's own spread into the tunnel), counted only where the straight line from the lamp
 * stays in the open, so no light leaks through rock. Stores the brightest lamp's position for the wet highlights.
 */
export function bakeLamps(net: Network) {
  const g = net.geometry;
  const pos = g.getAttribute("position");
  const nor = g.getAttribute("normal");
  const out = g.getAttribute("lampL") as THREE.BufferAttribute;
  const arr = out.array as Float32Array;
  const best = new Float32Array(pos.count);
  const R = 15;
  const v = new THREE.Vector3();
  const n = new THREE.Vector3();
  const L = new THREE.Vector3();
  const tgt = new THREE.Vector3();
  const q = new THREE.Vector3();
  // Bucket the vertices on a 15 m grid so each lamp looks only at its neighbours.
  const cells = new Map<string, number[]>();
  const key = (x: number, y: number, z: number) => `${x},${y},${z}`;
  for (let i = 0; i < pos.count; i++) {
    const k = key(Math.floor(pos.getX(i) / R), Math.floor(pos.getY(i) / R), Math.floor(pos.getZ(i) / R));
    const c = cells.get(k);
    if (c) c.push(i);
    else cells.set(k, [i]);
  }
  for (const lamp of net.lamps) {
    const cx = Math.floor(lamp.p.x / R);
    const cy = Math.floor(lamp.p.y / R);
    const cz = Math.floor(lamp.p.z / R);
    for (let dx = -1; dx <= 1; dx++)
      for (let dy = -1; dy <= 1; dy++)
        for (let dz = -1; dz <= 1; dz++) {
          const list = cells.get(key(cx + dx, cy + dy, cz + dz));
          if (!list) continue;
          for (const i of list) {
            v.fromBufferAttribute(pos, i);
            L.copy(lamp.p).sub(v);
            const d2 = L.lengthSq();
            if (d2 > R * R) continue;
            const d = Math.sqrt(d2);
            L.divideScalar(d);
            n.fromBufferAttribute(nor, i);
            const ndl = n.dot(L);
            if (ndl <= 0) continue;
            const spread = Math.max(0.2, -L.dot(lamp.n));
            const fade = (1 - d2 / (R * R)) ** 2;
            const e = (22 * ndl * spread * fade) / (d2 + 2.5);
            if (e < 0.004) continue;
            // In the open all the way from the lamp to just off the rock?
            tgt.copy(v).addScaledVector(n, 0.45);
            let seen = true;
            for (const t of [0.3, 0.6, 0.85])
              if (!net.open(q.copy(lamp.p).lerp(tgt, t))) {
                seen = false;
                break;
              }
            if (!seen) continue;
            arr[i * 4 + 3]! += e;
            if (e > best[i]!) {
              best[i] = e;
              arr[i * 4] = lamp.p.x;
              arr[i * 4 + 1] = lamp.p.y;
              arr[i * 4 + 2] = lamp.p.z;
            }
          }
        }
  }
  for (let i = 0; i < pos.count; i++) {
    arr[i * 4 + 3] = Math.min(arr[i * 4 + 3]!, 3);
    if (best[i] === 0) {
      arr[i * 4] = pos.getX(i);
      arr[i * 4 + 1] = pos.getY(i) + 3;
      arr[i * 4 + 2] = pos.getZ(i);
    }
  }
  out.needsUpdate = true;
}

/* ---------- Materials ---------- */

/**
 * GLSL: bends a view-space normal by the slope of a height field given in metres, using screen derivatives scaled by
 * the size of a pixel on the surface, so the bump is the same at any distance (and fades as the noise mips away).
 */
export const BUMP_GLSL = `vec3 ugBump(vec3 n, vec3 pos, float h, float faceDir) {
  vec3 dpx = dFdx(pos);
  vec3 dpy = dFdy(pos);
  float lx = max(length(dpx), 1e-5);
  float ly = max(length(dpy), 1e-5);
  vec3 sx = dpx / lx;
  vec3 sy = dpy / ly;
  vec2 dh = clamp(vec2(dFdx(h) / lx, dFdy(h) / ly), -1.5, 1.5);
  vec3 r1 = cross(sy, n);
  vec3 r2 = cross(n, sx);
  float det = dot(sx, r1) * faceDir;
  vec3 g = sign(det) * (dh.x * r1 + dh.y * r2);
  return normalize(abs(det) * n - g);
}`;

export const STATUS_COLOURS = ["#000000", "#5fd38d", "#fbbf24", "#fb7185"] as const;

export interface RockUniforms {
  uSupport: { value: number };
  uTime: { value: number };
  uWarm: { value: number };
  /** Cap lamps: world positions and unit directions (up to CAPS), how many are on, and the wall lamps' gain. */
  uCapP: { value: THREE.Vector3[] };
  uCapD: { value: THREE.Vector3[] };
  uCapN: { value: number };
  uLampGain: { value: number };
}
/** The most cap lamps the rock shader lights with. */
export const CAPS = 12;
export const rockUniforms = (): RockUniforms => ({
  uSupport: { value: 0 },
  uTime: { value: 0 },
  uWarm: { value: 1 },
  uCapP: { value: Array.from({ length: CAPS }, () => new THREE.Vector3(0, 1e4, 0)) },
  uCapD: { value: Array.from({ length: CAPS }, () => new THREE.Vector3(0, 0, 1)) },
  uCapN: { value: 0 },
  uLampGain: { value: 1 },
});

/**
 * The inside of the excavations: rough rock (vertex colour, tri-planar grain) banded with the host rock's lithology
 * and stained round the ore, drill half-barrels, wet lower walls and puddles, welded mesh down to 1.5 m off the floor
 * with its sheet laps, square bolt plates with domed washers on 1.5 m rings, sprayed shotcrete, the light of the wall
 * lamps (baked per vertex with an occlusion test) and of the crew's cap lamps (live), and the ground-support colours
 * when that layer is on.
 */
export function rockMaterial(u: RockUniforms) {
  const mat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.92, metalness: 0, envMapIntensity: 0.12 });
  const tex = noiseTexture();
  mat.customProgramCacheKey = () => "ug-rock-v6";
  mat.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, u, {
      tNoise: { value: tex },
      uS1: { value: new THREE.Color(STATUS_COLOURS[1]) },
      uS2: { value: new THREE.Color(STATUS_COLOURS[2]) },
      uS3: { value: new THREE.Color(STATUS_COLOURS[3]) },
    });
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nattribute vec4 aux;\nattribute vec4 lampL;\nvarying vec4 vAux;\nvarying vec4 vLamp;\nvarying vec2 vUg;\nvarying vec3 vW;\nvarying vec3 vWN;")
      .replace(
        "#include <worldpos_vertex>",
        "#include <worldpos_vertex>\nvAux = aux;\nvLamp = lampL;\nvUg = uv;\nvW = (modelMatrix * vec4(transformed, 1.0)).xyz;\nvWN = normalize(mat3(modelMatrix) * objectNormal);",
      );
    sh.fragmentShader = sh.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
#define CAPS ${CAPS}
uniform sampler2D tNoise;
uniform float uSupport;
uniform float uTime;
uniform float uWarm;
uniform vec3 uS1;
uniform vec3 uS2;
uniform vec3 uS3;
uniform vec3 uCapP[CAPS];
uniform vec3 uCapD[CAPS];
uniform int uCapN;
uniform float uLampGain;
varying vec4 vAux;
varying vec4 vLamp;
varying vec2 vUg;
varying vec3 vW;
varying vec3 vWN;
vec4 triN(vec3 p, vec3 n) {
  vec3 w = pow(abs(n), vec3(4.0));
  w /= (w.x + w.y + w.z + 1e-4);
  return texture2D(tNoise, p.zy) * w.x + texture2D(tNoise, p.xz) * w.y + texture2D(tNoise, p.xy) * w.z;
}
${BUMP_GLSL}
float hash2(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}
float gridLine(vec2 g) {
  vec2 fw = max(fwidth(g), vec2(1e-4));
  vec2 d = abs(fract(g - 0.5) - 0.5) / fw;
  float l = 1.0 - min(min(d.x, d.y), 1.0);
  return l * (1.0 - smoothstep(0.1, 0.32, max(fw.x, fw.y)));
}`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec3 nW = normalize(vWN);
float hgt = vAux.x;
float kind = vAux.w;
float shot = step(4.5, kind) * step(kind, 5.5);
float wallK = step(kind, 0.5);
vec4 fine = triN(vW * 0.42, nW);
vec4 mid = triN(vW * 0.09 + 0.3, nW);
vec4 big = triN(vW * 0.021 + 0.7, nW);
float isFloor = 1.0 - smoothstep(0.05, 0.35, hgt);
float bw = max(max(fwidth(vUg.x), fwidth(vUg.y)), 1e-4);
// Host rock in bands parallel to the orebody (striking east, dipping north at about 72 degrees), stained by
// oxidation in a halo round the ore.
float strat = vW.z + 35.0 - (vW.y + 100.0) * 0.325 + (big.r - 0.5) * 9.0;
float band = fract(strat / 8.5 + big.b * 0.35);
vec3 litho = mix(vec3(0.9, 0.96, 0.95), vec3(1.1, 1.01, 0.9), smoothstep(0.28, 0.42, band) - smoothstep(0.72, 0.86, band));
litho = mix(litho, vec3(0.78, 0.82, 0.86), smoothstep(0.56, 0.64, mid.b) * 0.55);
diffuseColor.rgb *= mix(vec3(1.0), litho, 1.0 - shot);
float halo = (1.0 - smoothstep(9.0, 24.0, abs(strat))) * smoothstep(0.38, 0.72, mid.a + 0.3 * (fine.r - 0.5)) * (1.0 - shot) * (1.0 - isFloor);
diffuseColor.rgb = mix(diffuseColor.rgb, diffuseColor.rgb * vec3(1.38, 0.94, 0.6), halo * 0.6);
// Blocky jointed rock on the walls; shotcrete is sprayed, mottled and a little lumpy.
float joint = smoothstep(0.62, 0.8, fine.b) * (1.0 - isFloor) * (1.0 - shot);
diffuseColor.rgb *= mix((0.74 + 0.5 * mid.r) * (0.86 + 0.28 * fine.g), (0.84 + 0.22 * mid.r) * (0.88 + 0.2 * fine.g) * (0.92 + 0.16 * fine.b), shot);
diffuseColor.rgb *= 1.0 - joint * 0.35;
diffuseColor.rgb *= 0.86 + 0.28 * big.g;
// Drill half-barrels: each 3.8 m round's perimeter holes leave half-pipes along the walls and back, about 0.75 m
// apart, drifting outwards across the round (the look-out).
float per = vUg.y / 0.75;
float rnd = floor(vUg.x / 3.8);
float fx = fract(vUg.x / 3.8);
float hbOff = hash2(vec2(rnd, floor(per + 0.5)));
float hbD = abs(fract(per + 0.5 + (hbOff - 0.5) * 0.3 + fx * 0.14) - 0.5) * 0.75;
float hbW = max(fwidth(per) * 0.75, 1e-4);
float hb = (1.0 - smoothstep(0.018, 0.018 + hbW * 1.5, hbD)) * step(0.3, hbOff) * smoothstep(0.03, 0.1, fx) * (1.0 - smoothstep(0.78, 0.92, fx));
hb *= wallK * (1.0 - shot) * smoothstep(0.7, 1.2, hgt) * (1.0 - smoothstep(0.03, 0.08, hbW));
// A groove: dark in its trough, a lit lip on one side.
float hbLip = (1.0 - smoothstep(0.018 + hbW, 0.03 + hbW * 2.0, hbD)) * step(0.3, hbOff) * wallK * (1.0 - shot) * smoothstep(0.7, 1.2, hgt) * (1.0 - smoothstep(0.03, 0.08, hbW)) * smoothstep(0.03, 0.1, fx) * (1.0 - smoothstep(0.78, 0.92, fx));
diffuseColor.rgb *= 1.0 - hb * 0.28 + (hbLip - hb) * 0.1;
// Wet: the bottom of the walls, mud splashed up them, streaks where water runs down, puddles on the floor.
float wetWall = (1.0 - isFloor) * (1.0 - smoothstep(0.25, 1.1, hgt)) * (0.55 + 0.45 * mid.g);
float streak = smoothstep(0.6, 0.78, texture2D(tNoise, vec2(vUg.x * 0.45, vUg.y * 0.018 + 0.3)).r) * (1.0 - isFloor) * (1.0 - smoothstep(2.5, 4.6, hgt)) * smoothstep(0.45, 0.6, mid.a);
float wet = max(wetWall, streak * 0.85);
diffuseColor.rgb *= mix(0.68, 1.0, smoothstep(0.1, 1.3, hgt) * (1.0 - isFloor) + isFloor);
diffuseColor.rgb *= 1.0 - 0.3 * wet;
float puddle = isFloor * smoothstep(0.6, 0.7, mid.g) * (0.6 + 0.4 * big.r);
diffuseColor.rgb *= 1.0 - isFloor * 0.18;
diffuseColor.rgb = mix(diffuseColor.rgb, diffuseColor.rgb * vec3(0.48, 0.44, 0.4), puddle);
wet = max(wet, puddle);
// Support: galvanised welded mesh (100 mm aperture) down to 1.5 m off the floor, the sheets lapping every 2.4 m;
// square 150 mm bolt plates with domed washers on 1.5 m rings, 1.3 m apart round the ring, staggered; none within
// 2.2 m of a face (the unsupported span). Under shotcrete only the plates show.
float supH = smoothstep(1.4, 1.6, hgt);
float rockSup = (kind < 0.5) ? step(2.2, vAux.z) : 0.0;
float lapD = abs(fract(vUg.x / 2.4 + 0.5) - 0.5) * 2.4;
float lapZ = 1.0 - smoothstep(0.08, 0.1, lapD);
float meshM = rockSup * supH * max(gridLine(vUg / 0.1), lapZ * gridLine((vUg + 0.05) / 0.1));
vec3 wire = mix(vec3(0.34, 0.34, 0.33), vec3(0.3, 0.21, 0.14), smoothstep(0.5, 0.8, fine.a));
diffuseColor.rgb = mix(diffuseColor.rgb, wire, meshM * 0.38);
diffuseColor.rgb *= 1.0 - rockSup * supH * lapZ * 0.12 * (1.0 - smoothstep(0.02, 0.06, bw));
vec2 bc = vec2(vUg.x / 1.5, vUg.y / 1.3);
bc.y += 0.5 * mod(floor(bc.x), 2.0);
vec2 cell = floor(bc);
vec2 jit = (vec2(hash2(cell), hash2(cell + 7.3)) - 0.5) * vec2(0.1, 0.12);
vec2 bd = (fract(bc) - 0.5) * vec2(1.5, 1.3) - jit;
float cov = clamp((0.075 - max(abs(bd.x), abs(bd.y))) / bw + 0.5, 0.0, 1.0);
float plate = max(rockSup, shot * step(2.2, vAux.z)) * supH * cov * (1.0 - smoothstep(0.05, 0.14, bw));
float rb = length(bd) / 0.045;
float dome = (1.0 - smoothstep(0.85, 1.0, rb)) * plate;
float nut = (1.0 - smoothstep(0.85, 1.0, max(abs(bd.x), abs(bd.y)) / 0.016)) * plate;
vec3 plateC = mix(vec3(0.3, 0.29, 0.28), vec3(0.36, 0.23, 0.14), smoothstep(0.5, 0.78, fine.a));
diffuseColor.rgb = mix(diffuseColor.rgb, plateC, plate * 0.85);
diffuseColor.rgb = mix(diffuseColor.rgb, vec3(0.5, 0.48, 0.45), nut * 0.6);
// The ground-support layer: tint by the heading's state.
float z = vAux.y;
vec3 sc = z > 2.5 ? uS3 : z > 1.5 ? uS2 : uS1;
float zoneOn = step(0.5, z) * uSupport;
float faded = uSupport * (1.0 - step(0.5, z));
diffuseColor.rgb = mix(diffuseColor.rgb, diffuseColor.rgb * 0.55, faded * 0.6);
diffuseColor.rgb = mix(diffuseColor.rgb, sc * 0.6, zoneOn * 0.42);`,
      )
      .replace(
        "#include <roughnessmap_fragment>",
        "#include <roughnessmap_fragment>\nroughnessFactor = mix(roughnessFactor, 0.8, isFloor);\nroughnessFactor = mix(roughnessFactor, 0.62, wetWall);\nroughnessFactor = mix(roughnessFactor, 0.66, streak * 0.8);\nroughnessFactor = mix(roughnessFactor, 0.58, puddle);\nroughnessFactor = mix(roughnessFactor, 0.5, plate);",
      )
      .replace(
        "#include <metalnessmap_fragment>",
        "#include <metalnessmap_fragment>\nmetalnessFactor = mix(metalnessFactor, 0.35, plate);\nmetalnessFactor = mix(metalnessFactor, 0.35, meshM);",
      )
      .replace(
        "#include <normal_fragment_maps>",
        `#include <normal_fragment_maps>
// Rough blasted rock: a few centimetres of relief over the half-metre lumps; shotcrete follows the rock's lumps with
// its own sprayed texture; puddles are flat; half-barrels are grooves; washers are domed.
float relief = (triN(vW * 0.42, nW).r * 0.12 * mix(1.0, 0.6, shot) + triN(vW * 1.37 + 0.5, nW).g * mix(0.035, 0.02, shot) + triN(vW * 3.1, nW).b * 0.012 * shot) * (1.0 - puddle) * mix(1.0, 0.45, isFloor);
relief -= hb * 0.02;
relief += plate * 0.006 + dome * 0.012 * (1.0 - rb * rb) + nut * 0.01;
normal = ugBump(normal, -vViewPosition, relief, faceDirection);`,
      )
      .replace(
        "#include <emissivemap_fragment>",
        `#include <emissivemap_fragment>
// A faint bounce so the dark rock still reads from outside, the warm glow of a working face, and the support colours.
totalEmissiveRadiance += diffuseColor.rgb * vec3(1.0, 0.84, 0.66) * 0.06;
float faceGlow = (kind > 0.5 && kind < 1.5) ? 1.0 : exp(-vAux.z / 7.0) * step(kind, 0.5);
totalEmissiveRadiance += vec3(1.0, 0.62, 0.3) * faceGlow * uWarm * 0.05;
float pulse = 0.75 + 0.25 * sin(uTime * 3.2);
totalEmissiveRadiance += sc * zoneOn * (z > 2.5 ? pulse : 1.0) * 0.22;
// The wall lamps' light, baked per vertex (irradiance in w, the brightest lamp's position in xyz), with a highlight
// on wet rock; and the cap lamps, live: 18-degree cones with a soft edge and inverse-square falloff.
vec3 nV = inverseTransformDirection(normal, viewMatrix);
vec3 toCam = normalize(cameraPosition - vW);
vec3 warm = vec3(1.0, 0.8, 0.56);
float lampE = vLamp.w * uLampGain;
totalEmissiveRadiance += diffuseColor.rgb * warm * lampE;
vec3 lH = normalize(normalize(vLamp.xyz - vW) + toCam);
totalEmissiveRadiance += warm * pow(max(dot(nV, lH), 0.0), mix(14.0, 80.0, wet)) * lampE * (0.03 + 0.55 * wet);
for (int i = 0; i < CAPS; i++) {
  if (i >= uCapN) break;
  vec3 L = uCapP[i] - vW;
  float d2 = dot(L, L);
  if (d2 > 1600.0) continue;
  float dl = sqrt(d2);
  L /= dl;
  float cone = smoothstep(0.927, 0.972, dot(-L, uCapD[i]));
  float att = 34.0 / (d2 + 1.0) * (1.0 - smoothstep(26.0, 40.0, dl));
  vec3 capC = vec3(1.0, 0.95, 0.86) * cone * att;
  totalEmissiveRadiance += capC * (diffuseColor.rgb * max(dot(nV, L), 0.0) + vec3(0.9, 0.8, 0.66) * pow(max(dot(nV, normalize(L + toCam)), 0.0), 40.0) * wet * 0.22);
}`,
      );
  };
  return mat;
}

/** The film over each tube's near wall: nearly clear face-on, a soft outline at the silhouette. */
export function shellMaterial(u: RockUniforms, colour = "#c9b8a2") {
  const m = new THREE.ShaderMaterial({
    side: THREE.BackSide,
    transparent: true,
    depthWrite: false,
    fog: true,
    uniforms: THREE.UniformsUtils.merge([
      THREE.UniformsLib.fog,
      {
        uColour: { value: new THREE.Color(colour) },
        uS1: { value: new THREE.Color(STATUS_COLOURS[1]) },
        uS2: { value: new THREE.Color(STATUS_COLOURS[2]) },
        uS3: { value: new THREE.Color(STATUS_COLOURS[3]) },
        uOpacity: { value: 1 },
      },
    ]),
    vertexShader: `attribute vec4 aux;
varying vec3 vN;
varying vec3 vV;
varying float vZone;
#include <common>
#include <fog_pars_vertex>
void main() {
  vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
  vN = normalize(normalMatrix * normal);
  vV = -mvPosition.xyz;
  vZone = aux.y;
  gl_Position = projectionMatrix * mvPosition;
  #include <fog_vertex>
}`,
    fragmentShader: `uniform vec3 uColour;
uniform vec3 uS1;
uniform vec3 uS2;
uniform vec3 uS3;
uniform float uOpacity;
uniform float uSupport;
varying vec3 vN;
varying vec3 vV;
varying float vZone;
#include <common>
#include <fog_pars_fragment>
void main() {
  float f = 1.0 - abs(dot(normalize(vN), normalize(vV)));
  float a = pow(f, 3.0) * 0.62 + 0.035;
  vec3 c = uColour;
  if (vZone > 0.5) {
    vec3 sc = vZone > 2.5 ? uS3 : vZone > 1.5 ? uS2 : uS1;
    c = mix(c, sc * 1.1, uSupport);
    a = mix(a, a * 1.25 + 0.05, uSupport);
  } else a *= 1.0 - 0.45 * uSupport;
  gl_FragColor = vec4(c, a * uOpacity);
  #include <fog_fragment>
}`,
  });
  m.uniforms.uSupport = u.uSupport;
  return m;
}

/* ---------- The cutaway block ---------- */

/**
 * The block of ground the mine is cut out of, drawn from inside: only its far walls and floor show from any angle,
 * so the mine always stands in front of rock. Soil and weathered rock near the top, fresh rock below, faint level
 * lines every 25 m.
 */
export function cutawayBlock() {
  const { x0, x1, z0, z1, y0 } = BOX;
  const g = new THREE.BoxGeometry(x1 - x0, -y0, z1 - z0, 48, 40, 32);
  g.translate((x0 + x1) / 2, y0 / 2, (z0 + z1) / 2);
  // Drop the top face (the surface is drawn separately).
  const pos = g.getAttribute("position");
  const index = g.getIndex()!;
  const keep: number[] = [];
  for (let i = 0; i < index.count; i += 3) {
    const a = index.getX(i);
    const b = index.getX(i + 1);
    const c = index.getX(i + 2);
    if (pos.getY(a) > -0.01 && pos.getY(b) > -0.01 && pos.getY(c) > -0.01) continue;
    keep.push(a, b, c);
  }
  g.setIndex(keep);
  const mat = new THREE.MeshStandardMaterial({ color: "#ffffff", roughness: 0.98, side: THREE.BackSide });
  const tex = noiseTexture();
  mat.customProgramCacheKey = () => "ug-block-v2";
  mat.onBeforeCompile = (sh) => {
    sh.uniforms.tNoise = { value: tex };
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vB;")
      .replace("#include <worldpos_vertex>", "#include <worldpos_vertex>\nvB = (modelMatrix * vec4(transformed, 1.0)).xyz;");
    sh.fragmentShader = sh.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform sampler2D tNoise;\nvarying vec3 vB;")
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
float d = -vB.y;
vec2 q = abs(vB.x - ${x0.toFixed(1)}) < 0.5 || abs(vB.x - ${x1.toFixed(1)}) < 0.5 ? vB.zy : vB.xy;
if (vB.y < ${(y0 + 0.5).toFixed(1)}) q = vB.xz;
vec4 n1 = texture2D(tNoise, q * 0.012);
vec4 n2 = texture2D(tNoise, q * 0.05 + 0.37);
vec4 n3 = texture2D(tNoise, vec2(q.x * 0.004, d * 0.06));
// Soil, oxidised and fresh rock, with bedding that wavers.
float w = d + (n1.r - 0.5) * 9.0;
vec3 soil = vec3(0.27, 0.215, 0.165);
vec3 ox = vec3(0.235, 0.165, 0.125);
vec3 fresh = vec3(0.125, 0.12, 0.115);
vec3 c = mix(soil, ox, smoothstep(5.0, 9.0, w));
c = mix(c, fresh, smoothstep(30.0, 52.0, w));
c *= 0.78 + 0.34 * n2.g + 0.18 * (n3.b - 0.5);
c *= 0.92 + 0.1 * sin(w * 0.9 + n2.r * 4.0);
// Level lines every 25 m.
float lv = abs(fract((d + 0.0) / 25.0 + 0.5) - 0.5) * 25.0;
float lineW = fwidth(d) * 1.2;
c += vec3(0.10, 0.08, 0.06) * (1.0 - smoothstep(lineW * 0.5, lineW * 1.5, lv)) * step(40.0, d);
// Darker towards the bottom, so the eye stays on the mine.
c *= mix(1.0, 0.55, smoothstep(60.0, 215.0, d));
diffuseColor.rgb = c;`,
      )
      .replace(
        "#include <emissivemap_fragment>",
        "#include <emissivemap_fragment>\ntotalEmissiveRadiance += diffuseColor.rgb * 0.12;",
      );
  };
  const mesh = new THREE.Mesh(g, mat);
  mesh.receiveShadow = false;
  mesh.renderOrder = -10;
  return mesh;
}
