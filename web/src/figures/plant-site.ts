/**
 * The rest of a concentrator site: clad steel buildings (ribbed sheeting with laps and corner flashings, roof
 * sheeting with skylights, ridge capping and ventilators, gutters and downpipes, roller and personnel doors,
 * windows, louvres and air-conditioners), shipping containers, multi-tier pipe racks, storage tanks and a lime
 * silo, a substation with transformers, light poles and security fences. Everything goes into a root that the
 * scene bakes by material. Metres, y up. No DOM access.
 */
import * as THREE from "three";
import { bar, beam, box, cyl, ladder, lathe, rail, sphere, type V3 } from "./fleet-kit";
import { plantMats } from "./fleet-plant";
import { bendPath, pipe, ringRail } from "./plant-kit";
import { finish } from "./plant-look";
import type { Palette } from "./plant-units";

const P = (x: number, y: number, z: number) => new THREE.Vector3(x, y, z);

/** Ribbed sheeting in a colour: the cladding's rib normals, weathered, with streaks below the laps. */
export const sheet = (M: Palette, colour: string, roof = false) =>
  finish(colour, { normalMap: M.ribs, normalScale: 1, rough: roof ? 0.42 : 0.5, metal: roof ? 0.45 : 0.12, coat: 0.2, rust: roof ? 0.12 : 0.08, streak: roof ? 0.05 : 0.45, streakColour: "#6d6352", scale: 0.1 });

/**
 * The triangle that fills a gable end (apex over the middle) or a monopitch end (apex over −z), facing out of the
 * wall on side `s` (±x), with UVs in metres. The winding sets the facing, so it survives baking.
 */
function gable(D: number, rise: number, s: number, mono: boolean) {
  const apex = mono ? -D / 2 : 0;
  const pts = [P(0, 0, -D / 2), P(0, 0, D / 2), P(0, rise, apex)];
  const uv = [0, 0, D, 0, apex + D / 2, rise];
  if (s > 0) {
    [pts[0], pts[1]] = [pts[1]!, pts[0]!];
    uv.splice(0, 4, D, 0, 0, 0);
  }
  const tri = new THREE.BufferGeometry().setFromPoints(pts);
  tri.setAttribute("uv", new THREE.BufferAttribute(new Float32Array(uv), 2));
  tri.computeVertexNormals();
  return tri;
}

/** A plane in a parent's frame with UVs in metres; `rot` turns the UVs so ribs run the other way. */
function quad(parent: THREE.Object3D, mat: THREE.Material, w: number, h: number, rot = false) {
  const g = new THREE.PlaneGeometry(w, h);
  const uv = g.getAttribute("uv") as THREE.BufferAttribute;
  for (let i = 0; i < uv.count; i++) {
    const u = uv.getX(i) * w;
    const v = uv.getY(i) * h;
    if (rot) uv.setXY(i, v, u);
    else uv.setXY(i, u, v);
  }
  const m = new THREE.Mesh(g, mat);
  parent.add(m);
  return m;
}

/** A box whose six faces carry UVs in metres (horizontal u on the sides), for ribbed or coursed finishes. */
export function metreBox(parent: THREE.Object3D, mat: THREE.Material, w: number, h: number, d: number, x: number, y: number, z: number, ry = 0) {
  const g = new THREE.BoxGeometry(w, h, d);
  const uv = g.getAttribute("uv") as THREE.BufferAttribute;
  const n = g.getAttribute("normal") as THREE.BufferAttribute;
  for (let i = 0; i < uv.count; i++) {
    const nx = Math.abs(n.getX(i));
    const ny = Math.abs(n.getY(i));
    const across = nx > 0.5 ? d : w;
    const up = ny > 0.5 ? d : h;
    uv.setXY(i, uv.getX(i) * across, uv.getY(i) * up);
  }
  const m = new THREE.Mesh(g, mat);
  m.position.set(x, y, z);
  m.rotation.y = ry;
  parent.add(m);
  return m;
}

type Side = "n" | "s" | "e" | "w";

export interface BuildingOptions {
  x: number;
  z: number;
  y?: number;
  /** Size along local x (w) and z (d), and the eave height. */
  w: number;
  d: number;
  h: number;
  ry?: number;
  wall: string;
  roof?: string;
  trim?: string;
  roofType?: "gable" | "mono";
  /** Roof rise over half the depth (gable) or the whole depth (mono). */
  pitch?: number;
  /** Doors along a side: `at` is the centre along that wall, from its middle. */
  doors?: { side: Side; at: number; w: number; h: number; kind: "roller" | "door" }[];
  windows?: { side: Side; at: number; y: number; w: number; h: number; n?: number; gap?: number }[];
  louvres?: { side: Side; at: number; y: number; w: number; h: number }[];
  ac?: { side: Side; at: number }[];
  vents?: number;
  skylights?: number;
  /** A strip of a second colour along the base of the walls (dado), its height. */
  dado?: { colour: string; h: number };
}

/** A clad steel building, its base at y (default 0). Returns the group (already in `root`). */
export function building(root: THREE.Object3D, M: Palette, o: BuildingOptions) {
  const g = new THREE.Group();
  g.position.set(o.x, o.y ?? 0, o.z);
  g.rotation.y = o.ry ?? 0;
  root.add(g);
  const W = o.w;
  const D = o.d;
  const H = o.h;
  const plinth = 0.45;
  const wallM = sheet(M, o.wall);
  const roofM = sheet(M, o.roof ?? "#a4a9a6", true);
  const trimM = finish(o.trim ?? "#5d6462", { rough: 0.45, coat: 0.3, rust: 0.1, streak: 0.2 });
  // Plinth and a concrete apron.
  box(g, M.concrete, W + 0.2, plinth, D + 0.2, 0, plinth / 2, 0, { r: 0.03 });
  // Walls: one sheet per side, with the dado colour below if asked.
  const sideLen = (s: Side) => (s === "n" || s === "s" ? W : D);
  const place = (m: THREE.Object3D, s: Side, along: number, y: number, out = 0) => {
    const off = 0.001 + out;
    if (s === "s") m.position.set(along, y, D / 2 + off);
    if (s === "n") {
      m.position.set(-along, y, -D / 2 - off);
      m.rotation.y = Math.PI;
    }
    if (s === "e") {
      m.position.set(W / 2 + off, y, -along);
      m.rotation.y = Math.PI / 2;
    }
    if (s === "w") {
      m.position.set(-W / 2 - off, y, along);
      m.rotation.y = -Math.PI / 2;
    }
    return m;
  };
  const dadoH = o.dado?.h ?? 0;
  const dadoM = o.dado ? sheet(M, o.dado.colour) : null;
  for (const s of ["n", "s", "e", "w"] as Side[]) {
    const L = sideLen(s);
    place(quad(g, wallM, L, H - plinth - dadoH), s, 0, plinth + dadoH + (H - plinth - dadoH) / 2);
    if (dadoM) {
      place(quad(g, dadoM, L, dadoH), s, 0, plinth + dadoH / 2);
      place(box(g, trimM, L + 0.06, 0.08, 0.06, 0, 0, 0, { r: 0 }), s, 0, plinth + dadoH, 0.03);
    }
    // Horizontal lap every 3.6 m or so: a thin shadow line.
    for (let y = plinth + dadoH + 3.6; y < H - 0.8; y += 3.6) place(box(g, M.dark, L, 0.025, 0.02, 0, 0, 0, { r: 0 }), s, 0, y, 0.01);
    // Base flashing.
    place(box(g, trimM, L + 0.06, 0.1, 0.06, 0, 0, 0, { r: 0 }), s, 0, plinth + 0.05, 0.03);
  }
  // Corner flashings.
  for (const sx of [-1, 1]) for (const sz of [-1, 1]) box(g, trimM, 0.2, H - plinth, 0.2, (sx * W) / 2, plinth + (H - plinth) / 2, (sz * D) / 2, { r: 0 });

  // Roof.
  const type = o.roofType ?? "gable";
  const pitch = o.pitch ?? (type === "gable" ? 0.18 : 0.06);
  const ov = 0.45;
  if (type === "gable") {
    const rise = pitch * (D / 2);
    const a = Math.atan2(rise, D / 2);
    const sl = Math.hypot(D / 2 + ov, rise + ov * Math.tan(a));
    for (const s of [-1, 1]) {
      const m = quad(g, roofM, W + 2 * ov, sl);
      m.rotation.x = -Math.PI / 2 + s * a;
      m.position.set(0, H + rise / 2 - (ov * Math.tan(a)) / 2, (s * (D / 2 + ov)) / 2);
      // Gable infill.
      const tm = new THREE.Mesh(gable(D, rise, s, false), wallM);
      tm.position.set((s * W) / 2 + s * 0.002, H, 0);
      g.add(tm);
      // Barge capping along the rakes, and the gutter along each eave.
      for (const sz of [-1, 1]) beam(g, trimM, P(s * (W / 2 + ov), H - 0.05, sz * (D / 2 + ov)), P(s * (W / 2 + ov), H + rise, 0), 0.08, 0.25);
      box(g, trimM, W + 2 * ov, 0.18, 0.2, 0, H - 0.12 - ov * Math.tan(a), s * (D / 2 + ov), { r: 0 });
    }
    box(g, trimM, W + 2 * ov, 0.12, 0.6, 0, H + rise + 0.04, 0, { r: 0 });
    // Ridge ventilators.
    const nv = o.vents ?? Math.max(0, Math.floor(W / 9));
    for (let i = 0; i < nv; i++) {
      const vx = -W / 2 + ((i + 0.5) * W) / nv;
      cyl(g, M.galv, 0.35, 0.5, vx, H + rise + 0.3, 0, { seg: 14 });
      sphere(g, M.galv, 0.42, vx, H + rise + 0.62, 0, { seg: 12, sy: 0.6 });
    }
    // Skylights: translucent sheets in the roof.
    const ns = o.skylights ?? Math.max(0, Math.floor(W / 7));
    for (let i = 0; i < ns; i++)
      for (const s of [-1, 1]) {
        const m = quad(g, plantMats.translucent(), 0.76, sl * 0.6);
        m.rotation.x = -Math.PI / 2 + s * a;
        m.position.set(-W / 2 + ((i + 0.5) * W) / ns, H + rise / 2 + 0.03, (s * D) / 4);
      }
  } else {
    const rise = pitch * D;
    const a = Math.atan2(rise, D);
    const sl = Math.hypot(D + 2 * ov, rise);
    const m = quad(g, roofM, W + 2 * ov, sl);
    m.rotation.x = -Math.PI / 2 + a;
    m.position.set(0, H + rise / 2, 0);
    for (const s of [-1, 1]) {
      const tm = new THREE.Mesh(gable(D, rise, s, true), wallM);
      tm.position.set((s * W) / 2 + s * 0.002, H, 0);
      g.add(tm);
    }
    box(g, trimM, W + 2 * ov, 0.18, 0.2, 0, H - 0.1, D / 2 + ov, { r: 0 });
    box(g, trimM, W + 2 * ov, 0.2, 0.15, 0, H + rise + 0.02, -D / 2 - ov, { r: 0 });
    // Back wall above the eave line.
    const back = quad(g, wallM, W, rise);
    back.position.set(0, H + rise / 2, -D / 2 - 0.001);
    back.rotation.y = Math.PI;
    for (let i = 0; i < (o.skylights ?? 0); i++) {
      const sm = quad(g, plantMats.translucent(), 0.76, sl * 0.7);
      sm.rotation.x = -Math.PI / 2 + a;
      sm.position.set(-W / 2 + ((i + 0.5) * W) / (o.skylights ?? 1), H + rise / 2 + 0.03, 0);
    }
  }
  // Downpipes at the corners.
  for (const sx of [-1, 1]) for (const sz of [-1, 1]) bar(g, trimM, P(sx * (W / 2 + 0.25), plinth, sz * (D / 2 + 0.25)), P(sx * (W / 2 + 0.25), H - 0.1, sz * (D / 2 + 0.25)), 0.05, 6);

  // Doors.
  for (const d of o.doors ?? []) {
    if (d.kind === "roller") {
      const m = quad(g, sheet(M, "#b9bcb6"), d.w, d.h, true);
      place(m, d.side, d.at, plinth + d.h / 2, 0.04);
      place(box(g, trimM, d.w + 0.6, 0.6, 0.55, 0, 0, 0, { r: 0.03 }), d.side, d.at, plinth + d.h + 0.3, 0.28);
      for (const s of [-1, 1]) place(box(g, trimM, 0.14, d.h, 0.16, 0, 0, 0, { r: 0 }), d.side, d.at + (s * (d.w + 0.14)) / 2, plinth + d.h / 2, 0.08);
      // Bollards in safety yellow either side.
      for (const s of [-1, 1]) {
        const b = cyl(g, M.yellow, 0.12, 1.1, 0, 0, 0, { seg: 10 });
        place(b, d.side, d.at + s * (d.w / 2 + 0.6), 0.55, 0.7);
      }
    } else {
      place(box(g, M.steelDark, d.w, d.h, 0.08, 0, 0, 0, { r: 0.02 }), d.side, d.at, plinth + d.h / 2, 0.04);
      place(box(g, trimM, d.w + 0.6, 0.08, 0.9, 0, 0, 0, { r: 0 }), d.side, d.at, plinth + d.h + 0.25, 0.45);
      place(box(g, M.concrete, d.w + 1.2, 0.15, 1.4, 0, 0, 0, { r: 0.02 }), d.side, d.at, 0.08, 0.7);
    }
  }
  for (const wdw of o.windows ?? []) {
    const n = wdw.n ?? 1;
    for (let i = 0; i < n; i++) {
      const at = wdw.at + (i - (n - 1) / 2) * (wdw.w + (wdw.gap ?? 0.4));
      place(box(g, trimM, wdw.w + 0.12, wdw.h + 0.12, 0.08, 0, 0, 0, { r: 0 }), wdw.side, at, wdw.y, 0.03);
      place(box(g, M.glass, wdw.w, wdw.h, 0.06, 0, 0, 0, { r: 0 }), wdw.side, at, wdw.y, 0.06);
    }
  }
  for (const l of o.louvres ?? []) {
    place(box(g, M.steelDark, l.w, l.h, 0.12, 0, 0, 0, { r: 0 }), l.side, l.at, l.y, 0.05);
    for (let y = -l.h / 2 + 0.12; y < l.h / 2; y += 0.16) place(box(g, M.galv, l.w - 0.06, 0.04, 0.1, 0, 0, 0, { r: 0 }), l.side, l.at, l.y + y, 0.11);
  }
  for (const a of o.ac ?? []) {
    place(box(g, M.white, 0.9, 0.65, 0.35, 0, 0, 0, { r: 0.03 }), a.side, a.at, 1.4, 0.2);
    place(cyl(g, M.dark, 0.22, 0.02, 0, 0, 0, { axis: "z", seg: 14 }), a.side, a.at + 0.12, 1.4, 0.38);
  }
  return g;
}

/* ---------- Shipping containers ---------- */

// Four liveries: every colour is two draw calls (sheeting and frame) wherever the boxes stand.
const CONTAINER = ["#2f5d8c", "#8f3a2c", "#4b6a3d", "#d8d3c6"];

/** A 20-foot (or 40-foot) container on the ground at (x, y, z), turned ry, in a colour (index or hex). */
export function container(root: THREE.Object3D, M: Palette, x: number, y: number, z: number, ry: number, colour: number | string, long = false) {
  const c = typeof colour === "number" ? CONTAINER[colour % CONTAINER.length]! : colour;
  const L = long ? 12.19 : 6.06;
  const mat = finish(c, { normalMap: M.ribs, normalScale: 1.2, rough: 0.55, coat: 0.2, rust: 0.35, streak: 0.35, scale: 0.15 });
  const g = new THREE.Group();
  g.position.set(x, y, z);
  g.rotation.y = ry;
  root.add(g);
  metreBox(g, mat, L, 2.59, 2.44, 0, 1.295, 0);
  // Corner posts and rails, and the door end's locking bars.
  const frame = finish(c, { rough: 0.6, coat: 0.1, rust: 0.5, streak: 0.2 });
  for (const sx of [-1, 1]) for (const sz of [-1, 1]) box(g, frame, 0.18, 2.6, 0.18, (sx * L) / 2, 1.3, (sz * 2.44) / 2, { r: 0 });
  for (const y of [0.06, 2.55]) for (const sz of [-1, 1]) box(g, frame, L, 0.14, 0.12, 0, y, (sz * 2.44) / 2, { r: 0 });
  for (let i = 0; i < 4; i++) bar(g, M.steelLight, P(L / 2 + 0.03, 0.2, -0.9 + i * 0.6), P(L / 2 + 0.03, 2.4, -0.9 + i * 0.6), 0.025, 5);
  return g;
}

/* ---------- Pipe racks ---------- */

export interface RackPipe {
  /** Offset across the rack from its centreline (m), the pipe radius, and its finish. */
  dx: number;
  r: number;
  mat: THREE.Material;
  /** Which tier (0 the top). */
  tier?: number;
}

/**
 * A pipe rack along ground-level points: steel bents every `span` metres with knee braces, longitudinal ties, one
 * or two tiers of pipes resting on the cross beams, footings. Returns the path of the rack's centreline at the top.
 */
export function pipeRack(root: THREE.Object3D, M: Palette, pts: V3[], o: { h: number; w: number; span?: number; tiers?: number; pipes: RackPipe[]; ground?: (x: number, z: number) => number }) {
  const span = o.span ?? 6;
  const tiers = o.tiers ?? 1;
  const gy = o.ground ?? (() => 0);
  const tierY = (t: number) => o.h - t * 1.8;
  // Bents along each run, including the corners.
  const side = (a: V3, b: V3) => {
    const d = b.clone().sub(a).setY(0).normalize();
    return new THREE.Vector3(-d.z, 0, d.x);
  };
  const bents: { p: V3; n: V3 }[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const a = pts[i]!;
    const b = pts[i + 1]!;
    const n = side(a, b);
    const L = a.distanceTo(b);
    const k = Math.max(1, Math.round(L / span));
    for (let j = i === 0 ? 0 : 1; j <= k; j++) bents.push({ p: a.clone().lerp(b, j / k), n: j === k && i < pts.length - 2 ? n.clone().add(side(b, pts[i + 2]!)).normalize() : n });
  }
  for (const { p, n } of bents) {
    const y0 = gy(p.x, p.z);
    for (const s of [-1, 1]) {
      const c = p.clone().addScaledVector(n, (s * o.w) / 2);
      box(root, M.steel, 0.3, o.h - y0, 0.3, c.x, y0 + (o.h - y0) / 2, c.z, { r: 0 });
      box(root, M.concrete, 0.8, 0.5, 0.8, c.x, y0 + 0.15, c.z, { r: 0.02 });
    }
    for (let t = 0; t < tiers; t++) {
      const y = tierY(t);
      // The cross beam stops 3 cm inside the columns' outer faces, so its ends never sit flush with them.
      beam(root, M.steel, p.clone().addScaledVector(n, -o.w / 2 - 0.12).setY(y), p.clone().addScaledVector(n, o.w / 2 + 0.12).setY(y), 0.22, 0.3);
    }
    // Knee braces under the top beam.
    for (const s of [-1, 1]) {
      const c = p.clone().addScaledVector(n, (s * o.w) / 2);
      const inner = p.clone().addScaledVector(n, (s * o.w) / 2 - s * 0.9);
      bar(root, M.steel, c.clone().setY(o.h - 1.0), inner.setY(o.h - 0.15), 0.06, 5);
    }
  }
  // Longitudinal ties at the top on both sides.
  for (const s of [-1, 1]) {
    for (let i = 0; i < pts.length - 1; i++) {
      const a = pts[i]!;
      const b = pts[i + 1]!;
      const n = side(a, b);
      beam(root, M.steel, a.clone().addScaledVector(n, (s * o.w) / 2).setY(o.h + 0.05), b.clone().addScaledVector(n, (s * o.w) / 2).setY(o.h + 0.05), 0.18, 0.18);
    }
  }
  // Pipes, offset across the rack (mitred at the corners), resting on their tier.
  for (const pp of o.pipes) {
    const off: V3[] = [];
    for (let i = 0; i < pts.length; i++) {
      const n0 = i > 0 ? side(pts[i - 1]!, pts[i]!) : side(pts[0]!, pts[1]!);
      const n1 = i < pts.length - 1 ? side(pts[i]!, pts[i + 1]!) : n0;
      const nm = n0.clone().add(n1).normalize();
      const k = 1 / Math.max(0.5, nm.dot(n0));
      off.push(pts[i]!.clone().addScaledVector(nm, pp.dx * k).setY(tierY(pp.tier ?? 0) + 0.16 + pp.r));
    }
    pipe(root, pp.mat, off, pp.r, { bend: 1.2 + Math.abs(pp.dx), flange: M.steelLight });
  }
  return bendPath(pts.map((p) => p.clone().setY(o.h + 0.6)), 2);
}

/* ---------- Tanks and silos ---------- */

/** A flat-bottomed storage tank with a cone roof, ring stiffeners, a caged ladder and a roof rail. */
export function storageTank(root: THREE.Object3D, M: Palette, o: { x: number; z: number; y?: number; r: number; h: number; mat: THREE.Material; band?: THREE.Material }) {
  const y = o.y ?? 0;
  cyl(root, M.concrete, o.r + 0.6, 0.5, o.x, y + 0.25, o.z, { seg: 64 });
  cyl(root, o.mat, o.r, o.h, o.x, y + 0.5 + o.h / 2, o.z, { seg: 72 });
  for (let h = 2.2; h < o.h - 0.5; h += 2.4) cyl(root, o.band ?? o.mat, o.r + 0.05, 0.14, o.x, y + 0.5 + h, o.z, { seg: 72 });
  lathe(root, o.mat, [[o.r + 0.12, 0], [o.r * 0.12, o.r * 0.14], [0, o.r * 0.15]], o.x, y + 0.5 + o.h, o.z, { seg: 72 });
  ringRail(root, M.yellow, o.x, y + 0.5 + o.h + 0.05, o.z, o.r - 0.3, { from: 0.4, to: 1.9 });
  // Caged ladder up the side.
  const lx = o.x + Math.cos(0.4) * (o.r + 0.45);
  const lz = o.z + Math.sin(0.4) * (o.r + 0.45);
  ladder(root, M.yellow, P(lx, y + 0.5, lz), P(lx, y + 0.5 + o.h + 1.0, lz), 0.5);
  for (let h = 2.4; h < o.h + 0.9; h += 0.9) {
    const pts: V3[] = [];
    for (let k = 0; k <= 6; k++) {
      const a = 0.4 - 0.25 + (k / 6) * 0.5;
      pts.push(P(o.x + Math.cos(a) * (o.r + 0.4 + Math.sin((k / 6) * Math.PI) * 0.45), y + 0.5 + h, o.z + Math.sin(a) * (o.r + 0.4 + Math.sin((k / 6) * Math.PI) * 0.45)));
    }
    for (let k = 0; k < 6; k++) bar(root, M.yellow, pts[k]!, pts[k + 1]!, 0.02, 4);
  }
  // A nozzle and valve at the foot.
  cyl(root, M.steelLight, 0.25, 1.4, o.x + o.r + 0.6, y + 1.0, o.z, { axis: "x", seg: 12 });
  cyl(root, M.red, 0.28, 0.06, o.x + o.r + 1.0, y + 1.45, o.z, { axis: "y", seg: 12 });
  return { top: P(o.x, y + 0.5 + o.h, o.z) };
}

/** A lime silo on four legs with a cone, a roof rail, a dust filter and a caged ladder. */
export function silo(root: THREE.Object3D, M: Palette, o: { x: number; z: number; r: number; h: number; legs: number; mat: THREE.Material }) {
  const y0 = o.legs;
  for (const sx of [-1, 1])
    for (const sz of [-1, 1]) {
      box(root, M.steel, 0.4, y0 + 2, 0.4, o.x + sx * o.r * 0.72, (y0 + 2) / 2, o.z + sz * o.r * 0.72, { r: 0 });
      box(root, M.concrete, 1.0, 0.5, 1.0, o.x + sx * o.r * 0.72, 0.25, o.z + sz * o.r * 0.72, { r: 0.02 });
    }
  for (const s of [-1, 1]) {
    bar(root, M.steel, P(o.x - o.r * 0.72, 0.5, o.z + s * o.r * 0.72), P(o.x + o.r * 0.72, y0, o.z + s * o.r * 0.72), 0.07, 6);
    bar(root, M.steel, P(o.x + s * o.r * 0.72, 0.5, o.z - o.r * 0.72), P(o.x + s * o.r * 0.72, y0, o.z + o.r * 0.72), 0.07, 6);
  }
  lathe(root, o.mat, [[0.35, y0 - o.r * 0.9], [o.r, y0 + 0.5]], o.x, 0, o.z, { seg: 40 });
  cyl(root, o.mat, o.r, o.h, o.x, y0 + 0.5 + o.h / 2, o.z, { seg: 48 });
  lathe(root, o.mat, [[o.r + 0.1, 0], [0.3, o.r * 0.25]], o.x, y0 + 0.5 + o.h, o.z, { seg: 48 });
  const top = y0 + 0.5 + o.h + o.r * 0.25;
  ringRail(root, M.yellow, o.x, y0 + 0.5 + o.h + 0.1, o.z, o.r - 0.2);
  box(root, M.steelLight, 1.4, 1.6, 1.4, o.x + 0.6, top + 0.6, o.z, { r: 0.05 });
  bar(root, M.steelLight, P(o.x - o.r, y0 + 0.5 + o.h - 0.5, o.z), P(o.x - o.r - 0.6, 0.4, o.z), 0.12, 8);
  ladder(root, M.yellow, P(o.x + o.r + 0.4, y0, o.z), P(o.x + o.r + 0.4, y0 + 0.5 + o.h + 1, o.z), 0.5);
  return { top: P(o.x, top, o.z) };
}

/* ---------- Power ---------- */

/** A transformer: tank, radiator banks, conservator, bushings. */
export function transformer(root: THREE.Object3D, M: Palette, x: number, z: number, ry = 0) {
  const g = new THREE.Group();
  g.position.set(x, 0, z);
  g.rotation.y = ry;
  root.add(g);
  const body = finish("#7b8580", { rough: 0.5, coat: 0.3, rust: 0.25, streak: 0.4, streakColour: "#3f3a30" });
  box(g, M.concrete, 5.2, 0.4, 4.2, 0, 0.2, 0, { r: 0.02 });
  box(g, body, 3.0, 2.8, 2.0, 0, 1.8, 0, { r: 0.06 });
  for (const s of [-1, 1]) for (let i = 0; i < 7; i++) box(g, body, 0.05, 2.2, 0.7, -1.2 + i * 0.4, 1.7, s * 1.4, { r: 0 });
  cyl(g, body, 0.32, 2.2, 0, 3.7, -0.6, { axis: "x", seg: 14 });
  for (let i = 0; i < 3; i++) {
    cyl(g, M.white, 0.1, 0.8, -0.8 + i * 0.8, 3.6, 0.5, { seg: 8 });
    for (let k = 0; k < 4; k++) cyl(g, M.white, 0.16, 0.05, -0.8 + i * 0.8, 3.3 + k * 0.17, 0.5, { seg: 8 });
  }
  return g;
}

/** A steel lattice light pole with a floodlight head. */
export function lightPole(root: THREE.Object3D, M: Palette, x: number, z: number, h = 18, yaw = 0) {
  box(root, M.concrete, 1.0, 0.5, 1.0, x, 0.25, z, { r: 0.02 });
  bar(root, M.galv, P(x, 0.5, z), P(x, h, z), 0.16, 8, 0.1);
  const head = new THREE.Group();
  head.position.set(x, h, z);
  head.rotation.y = yaw;
  root.add(head);
  box(head, M.galv, 1.6, 0.12, 0.12, 0, 0.1, 0, { r: 0 });
  for (const s of [-1, 1]) {
    box(head, M.steelDark, 0.55, 0.45, 0.2, s * 0.55, -0.1, 0.18, { rx: 0.5, r: 0.03 });
    box(head, M.lamp, 0.45, 0.35, 0.03, s * 0.55, -0.15, 0.3, { rx: 0.5, r: 0 });
  }
}

/** A chain-link security fence along points, 2.4 m tall with three strands of barbed wire. */
export function fence(root: THREE.Object3D, M: Palette, pts: V3[], h = 2.4) {
  for (let i = 0; i < pts.length - 1; i++) {
    const a = pts[i]!;
    const b = pts[i + 1]!;
    const L = a.distanceTo(b);
    const n = Math.max(1, Math.round(L / 3));
    for (let k = 0; k <= n; k++) {
      const p = a.clone().lerp(b, k / n);
      bar(root, M.galv, p, p.clone().setY(p.y + h + 0.45), 0.04, 6);
    }
    for (const y of [0.1, h]) bar(root, M.galv, a.clone().setY(a.y + y), b.clone().setY(b.y + y), 0.025, 5);
    for (const y of [h + 0.15, h + 0.3, h + 0.45]) bar(root, M.galv, a.clone().setY(a.y + y), b.clone().setY(b.y + y), 0.008, 3);
    const m = new THREE.Mesh(new THREE.PlaneGeometry(L, h), plantMats.mesh());
    m.position.copy(a).lerp(b, 0.5).setY(a.y + h / 2 + 0.05);
    m.rotation.y = Math.atan2(b.x - a.x, b.z - a.z) - Math.PI / 2;
    m.userData.noShadow = true;
    root.add(m);
  }
}

/** A bund wall round a rectangle (centre, size), height h. */
export function bund(root: THREE.Object3D, M: Palette, x: number, z: number, w: number, d: number, h = 1.2, t = 0.3) {
  // Walls stand just outside the slab's footprint and overlap at the corners by different amounts, so no two faces
  // share a plane.
  for (const s of [-1, 1]) {
    box(root, M.concrete, w + t * 2 + 0.06, h, t, x, h / 2 - 0.05, z + s * (d / 2 + t / 2 + 0.03), { r: 0.02 });
    box(root, M.concrete, t, h + 0.02, d + 0.04, x + s * (w / 2 + t / 2 + 0.03), h / 2 - 0.04, z, { r: 0.02 });
  }
}

/**
 * A covered stockpile: a dome of ribbed panels on a concrete ring wall, its meridian ribs and rings, a head house at
 * the crown where the feed conveyor drops through, a dozer door and a ventilator. Centre (x, z), base radius r,
 * crown height h above y = 0.
 */
export function stockpileDome(root: THREE.Object3D, M: Palette, o: { x: number; z: number; r: number; h: number; wall?: number; doorAt?: number }) {
  const wall = o.wall ?? 3.2;
  const r = o.r;
  const h = o.h - wall;
  const Rs = (r * r + h * h) / (2 * h);
  const tMax = Math.acos((Rs - h) / Rs);
  const yc = wall - (Rs - h);
  // Panels: ribbed sheet with UVs in metres along the meridians, so ribs run from the crown to the eave.
  const g = new THREE.SphereGeometry(Rs, 96, 28, 0, Math.PI * 2, 0, tMax);
  const uv = g.getAttribute("uv") as THREE.BufferAttribute;
  const pos = g.getAttribute("position") as THREE.BufferAttribute;
  for (let i = 0; i < uv.count; i++) {
    const a = Math.atan2(pos.getZ(i), pos.getX(i));
    const rr = Math.hypot(pos.getX(i), pos.getZ(i));
    uv.setXY(i, a * Math.max(rr, 1), Math.acos(Math.min(1, pos.getY(i) / Rs)) * Rs);
  }
  const shell = new THREE.Mesh(g, finish("#c9ccc8", { normalMap: M.ribs, normalScale: 0.8, rough: 0.45, metal: 0.35, coat: 0.2, rust: 0.05, streak: 0.4, streakColour: "#8e8270", scale: 0.06, dust: 1.2 }));
  shell.position.set(o.x, yc, o.z);
  root.add(shell);
  // Ring wall, with a cap so the panels sit on a ledge.
  cyl(root, M.concrete, r + 0.35, wall, o.x, wall / 2 - 0.02, o.z, { seg: 96, open: true });
  lathe(root, M.concrete, [[r - 0.3, wall + 0.02], [r + 0.5, wall + 0.02]], o.x, 0, o.z, { seg: 96 });
  // Meridian ribs and latitude rings in galvanised steel, standing proud of the panels.
  const ribs = 32;
  for (let i = 0; i < ribs; i++) {
    const a = (i / ribs) * Math.PI * 2;
    const pts: THREE.Vector3[] = [];
    for (let k = 0; k <= 10; k++) {
      const t = (k / 10) * tMax;
      const rr = (Rs + 0.18) * Math.sin(t);
      pts.push(P(o.x + rr * Math.cos(a), yc + (Rs + 0.18) * Math.cos(t), o.z + rr * Math.sin(a)));
    }
    for (let k = 1; k < pts.length; k++) if (pts[k]!.distanceTo(P(o.x, pts[k]!.y, o.z)) > 3.5) bar(root, M.galv, pts[k - 1]!, pts[k]!, 0.12, 5);
  }
  for (const f of [0.35, 0.62, 0.86]) {
    const t = tMax * f;
    const rr = (Rs + 0.16) * Math.sin(t);
    cyl(root, M.galv, rr, 0.22, o.x, yc + (Rs + 0.16) * Math.cos(t), o.z, { seg: 96, open: true });
  }
  // Head house on the crown, a ventilator beside it.
  box(root, M.galv, 7, 5, 7, o.x, o.h + 2.2, o.z, { r: 0.08 });
  box(root, M.steelDark, 7.6, 0.3, 7.6, o.x, o.h + 4.85, o.z, { r: 0.03 });
  cyl(root, M.galv, 0.7, 2.2, o.x + 9, o.h - 0.4, o.z + 6, { seg: 16 });
  // Dozer door through the ring wall, framed, with a rolled-up curtain.
  const da = o.doorAt ?? Math.PI * 0.75;
  const dx = o.x + Math.cos(da) * (r + 0.4);
  const dz = o.z + Math.sin(da) * (r + 0.4);
  box(root, M.dark, 0.6, wall - 0.2, 6.5, dx, (wall - 0.2) / 2, dz, { ry: -da, r: 0 });
  box(root, M.yellow, 0.8, 0.5, 7.4, dx, wall + 0.1, dz, { ry: -da, r: 0.04 });
  return { crown: P(o.x, o.h + 4.8, o.z), Rs, yc };
}

export { rail };
