/**
 * The process equipment of a gold concentrator at true scale: grinding mills (shells with liner bolts, ribbed heads,
 * girth gears in their guards, pinions, gearboxes and motors, feed chutes and discharge hoods), a hydrocyclone
 * cluster on its tower, horizontal slurry pumps, flotation tank cells whose froth spills into peripheral launders,
 * thickeners with bridges, drives, rakes and overflow launders, carbon-in-leach tanks with agitator gearboxes,
 * the elution column and kiln, and a gold room where doré is poured. Each builder adds its fixed steel and concrete
 * to `root` (which the scene bakes into one mesh per material) and returns the parts that move. Metres, y up.
 * No DOM access.
 */
import * as THREE from "three";
import { bake, bar, beam, box, cyl, deck, ladder, lathe, mats, rail, sphere, stairs, v3, type V3 } from "./fleet-kit";
import { plantMats } from "./fleet-plant";
import { cladding, liquidMaterial, moltenMaterial, ringRail, slab } from "./plant-kit";
import { blockNormal, finish, froth, glow } from "./plant-look";
import { smooth } from "./lib";

export function palette() {
  const ribs = cladding("#ffffff").normalMap!;
  return {
    steel: finish("#4f5d57", { rough: 0.6, coat: 0.15, rust: 0.3, streak: 0.3 }),
    steelDark: finish("#39434a", { rough: 0.6, coat: 0.15, rust: 0.25, streak: 0.25 }),
    steelLight: finish("#8a9496", { rough: 0.45, coat: 0.2, metal: 0.35, rust: 0.2, streak: 0.15 }),
    yellow: finish("#e0a31a", { rough: 0.5, coat: 0.35, rust: 0.18, streak: 0.05 }),
    galv: mats.galv(),
    grate: plantMats.grate(),
    concrete: finish("#aba596", { rough: 0.95, coat: 0, rust: 0, streak: 0.3, streakColour: "#6f675b", scale: 0.07, mud: 0.9 }),
    concreteDark: finish("#918a7c", { rough: 0.95, coat: 0, rust: 0, streak: 0.35, streakColour: "#5f584d", scale: 0.07, mud: 1 }),
    floor: finish("#7d8378", { rough: 0.7, coat: 0.3, rust: 0, streak: 0, scale: 0.2, mud: 0.4 }),
    mill: finish("#58704f", { rough: 0.45, coat: 0.35, rust: 0.18, streak: 0.35, streakColour: "#3d3b33" }),
    millHead: finish("#4a5f44", { rough: 0.5, coat: 0.3, rust: 0.2, streak: 0.3 }),
    guard: finish("#dba11e", { rough: 0.45, coat: 0.4, rust: 0.22, streak: 0.25 }),
    motor: finish("#2d5a7c", { rough: 0.42, coat: 0.45, rust: 0.12, streak: 0.15 }),
    gearbox: finish("#55625d", { rough: 0.5, coat: 0.3, rust: 0.2, streak: 0.2 }),
    drive: plantMats.drive(),
    pump: finish("#3f5f52", { rough: 0.5, coat: 0.3, rust: 0.3, streak: 0.3 }),
    cell: finish("#7d949b", { rough: 0.5, coat: 0.25, rust: 0.12, streak: 0.55, streakColour: "#4c4a42" }),
    tank: finish("#b9bdb2", { rough: 0.5, coat: 0.25, rust: 0.15, streak: 0.6, streakColour: "#6b6659" }),
    tankDark: finish("#9ea395", { rough: 0.5, coat: 0.25, rust: 0.15, streak: 0.4, streakColour: "#5f5b50" }),
    tankInner: (() => {
      const m = new THREE.MeshStandardMaterial({ color: "#3a372f", roughness: 0.95, side: THREE.BackSide });
      return m;
    })(),
    launder: (() => {
      const m = finish("#5e6a64", { rough: 0.55, coat: 0.2, rust: 0.3, streak: 0.4, streakColour: "#3f3d35" });
      m.side = THREE.DoubleSide;
      return m;
    })(),
    pipe: finish("#1e1f1e", { rough: 0.6, coat: 0.15, rust: 0, streak: 0.15, streakColour: "#8c7f6a" }),
    waterPipe: finish("#3f7150", { rough: 0.5, coat: 0.3, rust: 0.15, streak: 0.1 }),
    airPipe: finish("#4f80a8", { rough: 0.5, coat: 0.3, rust: 0.12, streak: 0.1 }),
    reagentPipe: finish("#c6b04a", { rough: 0.5, coat: 0.3, rust: 0.1, streak: 0.1 }),
    cyclone: finish("#c2612b", { rough: 0.62, coat: 0.15, rust: 0, streak: 0.4, streakColour: "#5a4a3c" }),
    rubber: mats.rubber(),
    dark: mats.dark(),
    black: mats.black(),
    stainless: finish("#b8bcbc", { rough: 0.28, coat: 0.2, metal: 0.75, rust: 0, streak: 0.15 }),
    copper: finish("#b0663b", { rough: 0.35, coat: 0.1, metal: 0.85, rust: 0, streak: 0 }),
    refractory: finish("#7a5f4a", { rough: 0.9, coat: 0, rust: 0.2, streak: 0 }),
    frp: finish("#c9b78f", { rough: 0.5, coat: 0.3, rust: 0, streak: 0.2 }),
    insul: finish("#cfd3d4", { rough: 0.32, coat: 0.15, metal: 0.6, rust: 0, streak: 0.2 }),
    rust: finish("#7a4d33", { rough: 0.85, coat: 0, rust: 0.5, streak: 0.3 }),
    red: finish("#a8291f", { rough: 0.45, coat: 0.4, rust: 0.15, streak: 0.1 }),
    white: finish("#e2e0d6", { rough: 0.45, coat: 0.4, rust: 0.1, streak: 0.35 }),
    glass: mats.glass(),
    block: finish("#d3ccbb", { rough: 0.9, coat: 0, rust: 0, streak: 0.08, streakColour: "#8a7f6c", normalMap: blockNormal(), normalScale: 0.8, scale: 0.08, mud: 0.4, dust: 0.4 }),
    ribs,
    lamp: new THREE.MeshStandardMaterial({ color: "#fff6e0", emissive: "#fff1d0", emissiveIntensity: 1.4, roughness: 0.4 }),
  };
}
export type Palette = ReturnType<typeof palette>;

const P = (x: number, y: number, z: number) => new THREE.Vector3(x, y, z);

/** A grated deck with toe-plated handrails on the sides named (n −z, s +z, e +x, w −x), at height y. */
export function platform(root: THREE.Object3D, M: Palette, x: number, y: number, z: number, w: number, d: number, sides = "nsew", legs = true) {
  deck(root, M.grate, w, d, x, y, z, 0.05);
  beam(root, M.steel, P(x - w / 2, y - 0.17, z - d / 2), P(x + w / 2, y - 0.17, z - d / 2), 0.15, 0.25);
  beam(root, M.steel, P(x - w / 2, y - 0.17, z + d / 2), P(x + w / 2, y - 0.17, z + d / 2), 0.15, 0.25);
  beam(root, M.steel, P(x - w / 2, y - 0.17, z - d / 2), P(x - w / 2, y - 0.17, z + d / 2), 0.15, 0.25);
  beam(root, M.steel, P(x + w / 2, y - 0.17, z - d / 2), P(x + w / 2, y - 0.17, z + d / 2), 0.15, 0.25);
  const c = (sx: number, sz: number) => P(x + (sx * w) / 2, y, z + (sz * d) / 2);
  const o = { toe: M.yellow };
  if (sides.includes("n")) rail(root, M.yellow, [c(-1, -1), c(1, -1)], o);
  if (sides.includes("s")) rail(root, M.yellow, [c(-1, 1), c(1, 1)], o);
  if (sides.includes("e")) rail(root, M.yellow, [c(1, -1), c(1, 1)], o);
  if (sides.includes("w")) rail(root, M.yellow, [c(-1, -1), c(-1, 1)], o);
  if (legs && y > 0.8)
    for (const sx of [-1, 1])
      for (const sz of [-1, 1]) {
        box(root, M.steel, 0.24, y - 0.07, 0.24, x + sx * (w / 2 - 0.15), (y - 0.07) / 2, z + sz * (d / 2 - 0.15), { r: 0 });
        box(root, M.concrete, 0.6, 0.3, 0.6, x + sx * (w / 2 - 0.15), 0.15, z + sz * (d / 2 - 0.15), { r: 0.02 });
      }
}

/** A straight walkway between two deck-level points with rails on both sides. */
export function walkway(root: THREE.Object3D, M: Palette, a: V3, b: V3, w = 1.1, sides: "both" | "left" | "right" = "both") {
  const d = b.clone().sub(a).setY(0);
  const len = d.length();
  d.normalize();
  const mid = a.clone().lerp(b, 0.5);
  const g = new THREE.Group();
  g.position.copy(mid);
  g.rotation.y = Math.atan2(d.x, d.z);
  root.add(g);
  deck(g, M.grate, w, len, 0, 0, 0, 0.05);
  beam(g, M.steel, P(-w / 2, -0.17, -len / 2), P(-w / 2, -0.17, len / 2), 0.15, 0.25);
  beam(g, M.steel, P(w / 2, -0.17, -len / 2), P(w / 2, -0.17, len / 2), 0.15, 0.25);
  if (sides !== "right") rail(g, M.yellow, [P(-w / 2, 0, -len / 2), P(-w / 2, 0, len / 2)], { toe: M.yellow });
  if (sides !== "left") rail(g, M.yellow, [P(w / 2, 0, -len / 2), P(w / 2, 0, len / 2)], { toe: M.yellow });
}

/**
 * A stair tower: scissor flights that alternate between two lanes (z = zA, zB) running between x0 and x1, with a
 * landing on posts at each turn, from y0 up to y1.
 */
export function stairTower(root: THREE.Object3D, M: Palette, o: { x0: number; x1: number; zA: number; zB: number; y0: number; y1: number; flights: number; width?: number }) {
  const w = o.width ?? 1.0;
  const rise = (o.y1 - o.y0) / o.flights;
  const dir = Math.sign(o.x1 - o.x0);
  const zc = (o.zA + o.zB) / 2;
  const dz = Math.abs(o.zA - o.zB) + w + 0.2;
  for (let i = 0; i < o.flights; i++) {
    const odd = i % 2 === 1;
    const lane = odd ? o.zB : o.zA;
    const from = odd ? o.x1 : o.x0;
    const to = odd ? o.x0 : o.x1;
    const yA = o.y0 + i * rise;
    stairs(root, M.steel, M.grate, M.yellow, P(from, yA + 0.04, lane), P(to, yA + rise - 0.02, lane), w);
    if (i < o.flights - 1) {
      const lx = odd ? o.x0 - dir * 0.75 : o.x1 + dir * 0.75;
      const outer = (odd ? -dir : dir) > 0 ? "e" : "w";
      // Landings on one side stack over the same footprint: only the highest on each side carries legs.
      platform(root, M, lx, yA + rise, zc, 1.5, dz, outer + "ns", i >= o.flights - 3);
    }
  }
}

/* ---------- Pumps ---------- */

/**
 * A horizontal centrifugal slurry pump with an overhead-mounted motor and V-belt guard. The shaft runs along local
 * x; the suction faces −x and the discharge rises from the top of the volute. `s` scales the whole pump.
 */
export function slurryPump(root: THREE.Object3D, M: Palette, x: number, y: number, z: number, ry = 0, s = 1) {
  const g = new THREE.Group();
  g.position.set(x, y, z);
  g.rotation.y = ry;
  root.add(g);
  const c = 0.75 * s; // shaft centre height
  box(g, M.steelDark, 2.6 * s, 0.25 * s, 1.0 * s, 0.5 * s, 0.12 * s, 0, { r: 0.02 });
  cyl(g, M.pump, 0.62 * s, 0.55 * s, -0.25 * s, c, 0, { axis: "x", seg: 22 });
  cyl(g, M.pump, 0.7 * s, 0.12 * s, -0.5 * s, c, 0, { axis: "x", seg: 22 });
  cyl(g, M.pump, 0.7 * s, 0.12 * s, 0.02 * s, c, 0, { axis: "x", seg: 22 });
  // Suction spool and discharge spool with flanges.
  cyl(g, M.pump, 0.22 * s, 0.6 * s, -0.85 * s, c, 0, { axis: "x", seg: 14 });
  cyl(g, M.steelLight, 0.3 * s, 0.06 * s, -1.15 * s, c, 0, { axis: "x", seg: 14 });
  cyl(g, M.pump, 0.18 * s, 0.7 * s, -0.25 * s, c + 0.85 * s, 0.32 * s, { seg: 14 });
  cyl(g, M.steelLight, 0.26 * s, 0.06 * s, -0.25 * s, c + 1.2 * s, 0.32 * s, { seg: 14 });
  // Bearing frame, then the motor above it on a hinged base, and the belt guard at the far end.
  cyl(g, M.pump, 0.24 * s, 1.1 * s, 0.65 * s, c, 0, { axis: "x", seg: 14 });
  box(g, M.pump, 1.2 * s, 0.45 * s, 0.7 * s, 0.65 * s, c - 0.4 * s, 0, { r: 0.03 });
  box(g, M.steelDark, 1.4 * s, 0.12 * s, 0.9 * s, 0.75 * s, c + 0.42 * s, 0, { r: 0.01 });
  cyl(g, M.motor, 0.36 * s, 1.1 * s, 0.75 * s, c + 0.85 * s, 0, { axis: "x", seg: 18 });
  for (let i = 0; i < 6; i++) box(g, M.motor, 0.9 * s, 0.05 * s, 0.05 * s, 0.75 * s, c + 0.85 * s + Math.cos(i) * 0.36 * s, Math.sin(i) * 0.36 * s, { r: 0 });
  box(g, M.yellow, 0.22 * s, 1.5 * s, 0.9 * s, 1.45 * s, c + 0.42 * s, 0, { r: 0.04 });
  g.updateMatrix();
  return { suction: P(-1.15 * s, c, 0).applyMatrix4(g.matrix), discharge: P(-0.25 * s, c + 1.2 * s, 0.32 * s).applyMatrix4(g.matrix), group: g };
}

/* ---------- Grinding mills ---------- */

/** A mill's speed and charge motion: critical speed, the fraction of it, and where the charge leaves and lands. */
export interface MillPhysics {
  /** Inside-liner diameter, m; critical speed Nc = 42.3/√D, rpm; running speed, rpm; fraction of critical. */
  D: number;
  Nc: number;
  rpm: number;
  phi: number;
  /** Outer layer: departure angle above the horizontal (shoulder) and landing angle below it (toe), degrees. */
  shoulder: number;
  toe: number;
  /** Charge filling by volume, and the longest flight time, s. */
  fill: number;
  flight: number;
}

export interface Mill {
  spin: THREE.Group;
  physics: MillPhysics;
  /** Cut the mill open at mid-length to show its charge (0 whole, 1 section); only a mill made with `charge`. */
  ghost(a: number): void;
  /** Where the outer layer leaves the shell (shoulder) and lands again (toe), world, on the section plane. */
  shoulderAt: THREE.Vector3;
  toeAt: THREE.Vector3;
  /** Axis height, shell radius, and the feed hopper and discharge ends (world). */
  axisY: number;
  R: number;
  feed: THREE.Vector3;
  discharge: THREE.Vector3;
  /** The feed-end walkway beside the trunnion, and the drive side's motor. */
  walk: THREE.Vector3;
  motor: THREE.Vector3;
  update(dt: number): void;
}

/**
 * A grinding mill with its axis along x, feed end at −x. `drive` puts the pinion, gearbox and motor on ±z. A SAG
 * mill gets a trommel in a hood, a big motor with an air-to-air cooler and a feed hopper fed from above.
 */
export function makeMill(
  root: THREE.Object3D,
  scene: THREE.Object3D,
  M: Palette,
  o: { x: number; y: number; z: number; D: number; L: number; head: number; trunnion: number; phi: number; drive: 1 | -1; sag?: boolean; feedFrom?: V3; charge?: number; small?: boolean },
): Mill {
  const R = o.D / 2;
  // Speed from the critical speed of the inside-liner diameter: Nc = 42.3/√D rpm.
  const Din = o.D - 2 * 0.2;
  const Nc = 42.3 / Math.sqrt(Din);
  const rpm = o.phi * Nc;
  // A mill that shows its charge is drawn as a section: every turning part has its own material carrying one
  // clipping plane across the axis at mid-length. The plane is always there (parked far away when the mill is
  // whole), so cutting it open never compiles a new shader.
  const see = !!o.charge;
  const cut = new THREE.Plane(new THREE.Vector3(1, 0, 0), 1e5);
  const own = <T extends THREE.Material>(m: T): T => {
    if (!see) return m;
    const c = m.clone() as T;
    c.clippingPlanes = [cut];
    c.side = THREE.DoubleSide;
    return c;
  };
  const shellM = see ? finish("#5b7352", { rough: 0.45, coat: 0.35, rust: 0.18, streak: 0.35, streakColour: "#3d3b33" }) : M.mill;
  const headM = see ? finish("#4b6045", { rough: 0.5, coat: 0.3, rust: 0.2, streak: 0.3 }) : M.millHead;
  if (see)
    for (const m of [shellM, headM]) {
      m.clippingPlanes = [cut];
      m.side = THREE.DoubleSide;
    }
  const trunM = own(M.steelLight);
  const darkM = own(M.dark);
  const linerM = see ? new THREE.MeshStandardMaterial({ color: "#2b2a28", roughness: 0.9, metalness: 0, clippingPlanes: [cut], side: THREE.DoubleSide }) : M.tankInner;
  const lifterM = see ? new THREE.MeshStandardMaterial({ color: "#1f1f1e", roughness: 0.8, metalness: 0, clippingPlanes: [cut], side: THREE.DoubleSide }) : M.rubber;
  const Rt = o.trunnion;
  const axisY = o.y + R + (o.sag ? 2.3 : 1.9);
  const L = o.L;
  const Lh = o.head;
  const Ltr = 1.9;
  const spinSrc = new THREE.Group();
  // Shell, flanges, heads and trunnions turn together.
  cyl(spinSrc, shellM, R, L, 0, 0, 0, { axis: "x", seg: 72, open: true });
  for (const s of [-1, 1]) {
    // Shell flanges (a ring, so they never share a face with the heads) and the ribbed heads.
    lathe(spinSrc, headM, [[R + 0.005, -0.18], [R + 0.16, -0.18], [R + 0.16, 0.18], [R + 0.005, 0.18]], (s * L) / 2, 0, 0, { axis: "x", seg: 72 });
    lathe(spinSrc, headM, s < 0 ? [[Rt, -Lh], [R * 0.985, 0]] : [[R * 0.985, 0], [Rt, Lh]], (s * L) / 2, 0, 0, { axis: "x", seg: 56 });
    const n = o.sag ? 20 : 16;
    for (let i = 0; i < n; i++) {
      const a = (i / n) * Math.PI * 2;
      const c = Math.cos(a);
      const sn = Math.sin(a);
      const x0 = s * (L / 2 + 0.2);
      const x1 = s * (L / 2 + Lh - 0.05);
      const r0 = R * 0.985 - (R - Rt) * (0.2 / Lh) + 0.1;
      const r1 = Rt + 0.12;
      beam(spinSrc, headM, P(x0, c * r0, sn * r0), P(x1, c * r1, sn * r1), 0.16, 0.26, P(0, c, sn));
    }
    // Trunnion and its flange.
    cyl(spinSrc, trunM, Rt, Ltr, s * (L / 2 + Lh + Ltr / 2), 0, 0, { axis: "x", seg: 36 });
    cyl(spinSrc, headM, Rt + 0.3, 0.25, s * (L / 2 + Lh + 0.1), 0, 0, { axis: "x", seg: 36 });
  }
  if (see) {
    // Inside: a rubber-lined shell with high-low lifter bars, seen only when the shell goes see-through.
    cyl(spinSrc, linerM, R - 0.06, L - 0.1, 0, 0, 0, { axis: "x", seg: 72, open: true });
    const nl = Math.round(o.D * 5.5);
    for (let i = 0; i < nl; i++) {
      const a = (i / nl) * Math.PI * 2;
      const hl = i % 2 ? 0.22 : 0.34;
      box(spinSrc, lifterM, L - 0.2, hl, 0.24, 0, Math.cos(a) * (R - 0.06 - hl / 2), Math.sin(a) * (R - 0.06 - hl / 2), { rx: a, r: 0 });
    }
  }
  // Feed-end liner spout inside the trunnion, and the mill's two manholes.
  cyl(spinSrc, darkM, Rt * 0.72, 0.6, -(L / 2 + Lh + Ltr + 0.2), 0, 0, { axis: "x", seg: 24 });
  for (const a of [0.5, 0.5 + Math.PI]) box(spinSrc, headM, 1.3, 0.16, 0.95, -L * 0.18, Math.cos(a) * (R + 0.09), Math.sin(a) * (R + 0.09), { rx: a, r: 0.04 });
  // The girth gear, which shows through the guard's gap.
  const gx = L / 2 - 1.0;
  cyl(spinSrc, darkM, R + 0.82, 0.9, gx, 0, 0, { axis: "x", seg: 72 });
  cyl(spinSrc, trunM, R + 0.2, 0.3, gx, 0, 0, { axis: "x", seg: 72 });
  if (o.sag) {
    // Discharge trommel: a screen drum on the discharge trunnion.
    cyl(spinSrc, trunM, Rt * 0.95, 4.2, L / 2 + Lh + Ltr + 2.1, 0, 0, { axis: "x", seg: 28, open: true });
    for (let i = 0; i < 6; i++) cyl(spinSrc, trunM, Rt * 0.98, 0.12, L / 2 + Lh + Ltr + 0.2 + i * 0.8, 0, 0, { axis: "x", seg: 28 });
  }
  const spin = bake(spinSrc, { mudTop: -50, mudBottom: -60, dust: 0.45 });
  spin.position.set(o.x, axisY, o.z);
  spin.traverse((m) => {
    m.castShadow = true;
    m.receiveShadow = true;
  });
  // Liner bolts in rows along the shell: they show it turning.
  const rows = Math.round((Math.PI * 2 * R) / 0.5);
  const cols = Math.max(4, Math.round(L / 0.48));
  const boltM = see ? new THREE.MeshStandardMaterial({ color: "#26282a", roughness: 0.6, metalness: 0.4, clippingPlanes: [cut] }) : M.dark;
  const bolts = new THREE.InstancedMesh(new THREE.CylinderGeometry(0.06, 0.07, 0.11, 6), boltM, rows * cols);
  const mtx = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const one = new THREE.Vector3(1, 1, 1);
  let k = 0;
  for (let i = 0; i < rows; i++) {
    const a = (i / rows) * Math.PI * 2;
    q.setFromAxisAngle(new THREE.Vector3(1, 0, 0), a);
    for (let j = 0; j < cols; j++) {
      const x = -L / 2 + 0.4 + ((L - 0.8) * j) / (cols - 1);
      if (Math.abs(x - gx) < 0.8) continue;
      mtx.compose(new THREE.Vector3(x, Math.cos(a) * (R + 0.03), Math.sin(a) * (R + 0.03)), q, one);
      bolts.setMatrixAt(k++, mtx);
    }
  }
  bolts.count = k;
  bolts.castShadow = false;
  spin.add(bolts);
  scene.add(spin);

  /* Fixed: bearings on concrete piers, the gear guard, the drive train, feed and discharge, walkways. */
  const xF = o.x - L / 2 - Lh - Ltr / 2;
  const xD = o.x + L / 2 + Lh + Ltr / 2;
  const pierTop = axisY - Rt - 1.25;
  for (const xb of [xF, xD]) {
    box(root, M.concrete, 5.2, pierTop - o.y, 6.4, xb, o.y + (pierTop - o.y) / 2, o.z, { r: 0.12 });
    box(root, M.concrete, 6.0, 0.6, 7.2, xb, o.y + 0.3, o.z, { r: 0.06 });
    // Hydrostatic bearing housing, its cap, and the lube lines.
    box(root, M.gearbox, 3.0, 1.3, 3.8, xb, pierTop + 0.65, o.z, { r: 0.12 });
    lathe(root, M.gearbox, [[Rt + 0.45, -1.0], [Rt + 0.45, 1.0]], xb, axisY, o.z, { axis: "x", seg: 32 });
    for (const sz of [-1, 1]) bar(root, M.steelLight, P(xb + 0.8, pierTop + 0.3, o.z + sz * 1.6), P(xb + 0.8, o.y + 0.6, o.z + sz * 2.8), 0.04, 6);
  }
  // Girth gear guard: a ring of sheet steel round the gear, carried down to a sump on the floor.
  const ggx = o.x + gx;
  lathe(root, M.guard, [[R + 0.5, -0.65], [R + 1.35, -0.65], [R + 1.35, 0.65], [R + 0.5, 0.65]], ggx, axisY, o.z, { axis: "x", seg: 72 });
  for (const s of [-1, 1]) lathe(root, M.guard, [[R + 1.35, s * 0.65], [R + 0.5, s * 0.65]], ggx, axisY, o.z, { axis: "x", seg: 72 });
  box(root, M.guard, 1.5, axisY - R - 1.0 - o.y, 2 * R + 1.6, ggx, o.y + (axisY - R - 1.0 - o.y) / 2, o.z, { r: 0.06 });
  // Bolted inspection doors on the guard.
  for (const a of [0.7, 1.57, 2.45, -0.6, 3.75]) box(root, M.guard, 1.0, 0.07, 0.95, ggx, axisY + Math.sin(a) * (R + 1.38), o.z + Math.cos(a) * (R + 1.38), { rx: Math.PI / 2 - a, r: 0.02 });
  // Pinion, couplings, gearbox and motor in line, out on the drive side.
  const zP = o.z + o.drive * (R + 1.7);
  const shaftY = axisY - R * 0.35;
  box(root, M.guard, 1.7, 2.2, 1.9, ggx, shaftY, zP, { r: 0.1 });
  box(root, M.concrete, 17, shaftY - 1.1 - o.y, 4.4, ggx + 6.8, o.y + (shaftY - 1.1 - o.y) / 2, zP, { r: 0.1 });
  cyl(root, M.steelLight, 0.24, 2.2, ggx + 1.9, shaftY, zP, { axis: "x", seg: 14 });
  box(root, M.yellow, 1.0, 1.0, 1.0, ggx + 2.1, shaftY, zP, { r: 0.05 });
  // Gearbox: a ribbed casing with an oil cooler.
  box(root, M.gearbox, 3.2, 2.8, 2.8, ggx + 4.6, shaftY + 0.2, zP, { r: 0.14 });
  for (let i = 0; i < 5; i++) box(root, M.gearbox, 0.08, 2.6, 3.0, ggx + 3.3 + i * 0.65, shaftY + 0.2, zP, { r: 0 });
  box(root, M.steelLight, 1.2, 0.7, 0.6, ggx + 4.6, shaftY + 1.95, zP + o.drive * 0.8, { r: 0.04 });
  box(root, M.yellow, 1.2, 1.1, 1.1, ggx + 6.7, shaftY, zP, { r: 0.05 });
  const motorR = o.sag ? 1.6 : 1.25;
  const motorL = o.sag ? 4.4 : 3.4;
  const mx = ggx + 7.6 + motorL / 2;
  const my = shaftY;
  cyl(root, M.motor, motorR, motorL, mx, my, zP, { axis: "x", seg: 36 });
  for (const s of [-1, 1]) lathe(root, M.motor, [[motorR * 0.95, 0], [motorR * 0.55, 0.35], [0.25, 0.42]], mx + (s * motorL) / 2, my, zP, { axis: "x", seg: 28, rz: s < 0 ? Math.PI : 0 });
  // Cooling ribs along the frame, the terminal box and, on a SAG drive, an air-to-air cooler on top.
  for (let i = 0; i < 16; i++) {
    const a = -0.2 + (i / 15) * (Math.PI + 0.4);
    box(root, M.motor, motorL * 0.9, 0.2, 0.06, mx, my + Math.sin(a) * (motorR + 0.06), zP + Math.cos(a) * (motorR + 0.06), { rx: Math.PI / 2 - a, r: 0 });
  }
  box(root, M.motor, 1.0, 0.9, 0.8, mx - motorL * 0.2, my + 0.2, zP + o.drive * (motorR + 0.35), { r: 0.05 });
  for (const s of [-1, 1]) box(root, M.motor, motorL * 0.9, 0.5, 0.5, mx, my - motorR + 0.15, zP + s * motorR * 0.75, { r: 0.03 });
  if (o.sag) {
    box(root, M.motor, motorL * 0.85, 1.4, motorR * 1.7, mx, my + motorR + 0.65, zP, { r: 0.1 });
    for (let i = 0; i < 9; i++) box(root, M.dark, 0.06, 0.95, motorR * 1.72, mx - motorL * 0.38 + i * motorL * 0.095, my + motorR + 0.7, zP, { r: 0 });
  }
  // Lube skid with tanks and pumps beside the gear.
  box(root, M.steelDark, 3.6, 0.25, 2.0, ggx + 1.8, o.y + 0.13, zP + o.drive * 3.4, { r: 0.02 });
  box(root, M.gearbox, 2.0, 1.4, 1.6, ggx + 1.2, o.y + 0.95, zP + o.drive * 3.4, { r: 0.08 });
  for (const dx of [2.6, 3.2]) cyl(root, M.motor, 0.22, 0.6, ggx + dx, o.y + 0.6, zP + o.drive * 3.4, { seg: 12 });
  bar(root, M.steelLight, P(ggx + 1.2, o.y + 1.6, zP + o.drive * 3.4), P(ggx, axisY - R - 0.9, o.z + o.drive * (R + 0.4)), 0.05, 6);

  // Walkways at the trunnions, with stairs down to the floor.
  const wy = pierTop + 1.3;
  const wz = o.z - o.drive * (Rt + 2.6);
  const outer = o.drive > 0 ? "n" : "s";
  platform(root, M, xF - 0.3, wy, wz, 5.2, 2.4, "ew");
  stairs(root, M.steel, M.grate, M.yellow, P(xF - 0.3, o.y + 0.16, wz - o.drive * (1.2 + (wy - o.y) * 1.05)), P(xF - 0.3, wy, wz - o.drive * 1.2), 0.9);
  platform(root, M, xD + 0.3, wy, wz, 5.2, 2.4, outer + "ew");

  // Feed: a lined hopper under the feed conveyor's head chute, a sloped chute and the feed spout.
  const hx = xF - (o.sag ? 4.6 : 3.4);
  const hy = axisY + (o.sag ? 5.6 : 3.6);
  const hopper = o.feedFrom ?? P(hx, hy + 1.6, o.z);
  box(root, M.steel, 3.2, 2.4, 3.2, hx, hy, o.z, { r: 0.08 });
  box(root, M.steelDark, 3.4, 0.3, 3.4, hx, hy + 1.25, o.z, { r: 0.03 });
  beam(root, M.steel, P(hx + 0.6, hy - 1.0, o.z), P(xF - Ltr / 2 - 0.4, axisY + 0.3, o.z), 1.6, 1.4);
  cyl(root, M.dark, Rt * 0.68, 1.4, xF - Ltr / 2 - 0.6, axisY, o.z, { axis: "x", seg: 20 });
  for (const sx of [-1, 1])
    for (const sz of [-1, 1]) box(root, M.steel, 0.3, hy - 1.2 - o.y, 0.3, hx + sx * 1.4, o.y + (hy - 1.2 - o.y) / 2, o.z + sz * 1.4, { r: 0 });
  for (const h of [0.35, 0.7]) {
    beam(root, M.steel, P(hx - 1.4, o.y + (hy - 1.2) * h, o.z - 1.4), P(hx - 1.4, o.y + (hy - 1.2) * h, o.z + 1.4), 0.18, 0.18);
    beam(root, M.steel, P(hx + 1.4, o.y + (hy - 1.2) * h, o.z - 1.4), P(hx + 1.4, o.y + (hy - 1.2) * h, o.z + 1.4), 0.18, 0.18);
  }
  platform(root, M, hx - 0.2, hy + 1.4, o.z + 2.6, 3.6, 1.8, "sew", false);
  ladder(root, M.yellow, P(hx - 1.9, o.y, o.z + 3.2), P(hx - 1.9, hy + 1.4, o.z + 3.2), 0.5);

  // Discharge: a hood over the trommel (or the trunnion), venting up, and a chute into the pump box.
  const hw = o.sag ? 5.6 : 2.8;
  const xH = xD + Ltr / 2 + hw / 2 - 0.4;
  const hb = Rt * 2 + 2.6;
  box(root, M.steel, hw, hb, hb, xH, axisY, o.z, { r: 0.1 });
  box(root, M.steelDark, hw + 0.2, 0.25, hb + 0.2, xH, axisY + hb / 2, o.z, { r: 0.03 });
  for (let i = 1; i < (o.sag ? 4 : 2); i++) box(root, M.steelDark, 0.12, hb + 0.04, hb + 0.04, xH - hw / 2 + (i * hw) / (o.sag ? 4 : 2), axisY, o.z, { r: 0 });
  bar(root, M.steelLight, P(xH, axisY + hb / 2, o.z), P(xH, axisY + hb / 2 + 3.5, o.z), 0.3, 12);
  const pbH = axisY - hb / 2 - o.y;
  box(root, M.steel, hw * 0.8, pbH, hb * 0.8, xH, o.y + pbH / 2, o.z, { r: 0.08 });
  if (o.sag) {
    // Oversize (scats) chute off the end of the trommel.
    beam(root, M.steel, P(xH + hw / 2 - 0.2, axisY - 0.6, o.z), P(xH + hw / 2 + 2.6, o.y + 1.2, o.z + 1.8), 1.0, 1.0);
    box(root, M.steelDark, 2.8, 1.6, 2.8, xH + hw / 2 + 3.2, o.y + 0.8, o.z + 2.4, { r: 0.05 });
  }
  // Floor slab under the whole mill, and a drain trench.
  slab(root, M.concreteDark, L + 2 * (Lh + Ltr) + 30, 2 * R + 22, o.x + 6, o.y + 0.12, o.z + o.drive * 4, 0.5);
  box(root, M.dark, L + 2 * (Lh + Ltr) + 20, 0.05, 0.5, o.x + 6, o.y + 0.13, o.z - o.drive * (R + 6), { r: 0 });

  const omega = (rpm * Math.PI * 2) / 60;
  const dir = o.drive > 0 ? -1 : 1;
  const rising = o.drive > 0 ? 1 : -1;
  const charge = see ? makeCharge(scene, { R: R - 0.36, L: L - 0.5, omega, fill: o.charge!, rising, n: o.small ? 220 : 520, cut }) : null;
  if (charge) {
    charge.group.position.set(o.x, axisY, o.z);
    // The cut face of shell, liner and lifters, drawn as a section is on a drawing: one flat colour.
    const ring = new THREE.Mesh(new THREE.RingGeometry(R - 0.42, R + 0.17, 120, 1), new THREE.MeshStandardMaterial({ color: "#a4482a", roughness: 0.7, side: THREE.DoubleSide }));
    ring.rotation.y = -Math.PI / 2;
    ring.position.x = -0.01;
    charge.group.add(ring);
  }
  const shellMeshes: THREE.Mesh[] = [];
  spin.traverse((m) => {
    const mm = m as THREE.Mesh;
    if (mm.isMesh) shellMeshes.push(mm);
  });
  const onSection = (deg: number) => {
    const a = (deg * Math.PI) / 180;
    return P(o.x, axisY + (R - 0.36) * Math.sin(a), o.z + rising * (R - 0.36) * Math.cos(a));
  };
  let ghostA = 0;
  return {
    spin,
    physics: { D: Din, Nc, rpm, phi: o.phi, shoulder: charge?.shoulder ?? 0, toe: charge?.toe ?? 0, fill: o.charge ?? 0, flight: charge?.flight ?? 0 },
    shoulderAt: onSection(charge?.shoulder ?? 0),
    toeAt: onSection((charge?.toe ?? 0) + 180),
    ghost(a) {
      ghostA = a;
      if (!see) return;
      const open = a > 0.5;
      // Keep x > the mill's middle: the feed half goes, the discharge half and the charge in it stay.
      cut.constant = open ? -o.x : 1e5;
      // A section drawing is lit inside: the cut shell casts no shadow into it.
      for (const m of shellMeshes) m.castShadow = !open;
      charge!.group.visible = open;
    },
    axisY,
    R,
    feed: hopper,
    discharge: P(xH, o.y + 1.5, o.z),
    walk: P(xF - 0.3, wy, wz),
    motor: P(mx, my, zP),
    update(dt) {
      spin.rotation.x += omega * dt * dir;
      if (charge && ghostA > 0.01) charge.update(dt);
    },
  };
}

/** A tile of packed balls and ore lumps for the charge's cut face (drawn once, on first use). */
let pebbleTex: THREE.Texture | null = null;
function pebbles(): THREE.Texture {
  if (pebbleTex) return pebbleTex;
  const n = 256;
  const cv = document.createElement("canvas");
  cv.width = cv.height = n;
  const g = cv.getContext("2d")!;
  g.fillStyle = "#3b3631";
  g.fillRect(0, 0, n, n);
  let sd = 97;
  const rnd = () => ((sd = (sd * 16807) % 2147483647) / 2147483647);
  for (let i = 0; i < 230; i++) {
    const r = 5 + rnd() * 13;
    const x = rnd() * n;
    const y = rnd() * n;
    const ball = rnd() < 0.42;
    const base = ball ? [138, 142, 146] : rnd() < 0.5 ? [112, 98, 82] : [134, 118, 98];
    // Draw wrapped, so the tile repeats without seams.
    for (const dx of [-n, 0, n])
      for (const dy of [-n, 0, n]) {
        const gr = g.createRadialGradient(x + dx - r * 0.35, y + dy - r * 0.35, r * 0.1, x + dx, y + dy, r);
        gr.addColorStop(0, `rgb(${base.map((c) => Math.min(255, c + 45)).join(",")})`);
        gr.addColorStop(0.7, `rgb(${base.join(",")})`);
        gr.addColorStop(1, `rgb(${base.map((c) => c * 0.45).join(",")})`);
        g.fillStyle = gr;
        g.beginPath();
        if (ball) g.arc(x + dx, y + dy, r * 0.8, 0, Math.PI * 2);
        else g.ellipse(x + dx, y + dy, r, r * (0.6 + rnd() * 0.3), rnd() * Math.PI, 0, Math.PI * 2);
        g.fill();
      }
  }
  const t = new THREE.CanvasTexture(cv);
  t.colorSpace = THREE.SRGBColorSpace;
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  // The extrusion's cap UVs are in metres: one tile is 1.4 m of charge.
  t.repeat.set(1 / 1.4, 1 / 1.4);
  t.anisotropy = 4;
  pebbleTex = t;
  return t;
}

/**
 * The charge in a turning mill, by the Davis model: every particle turns with the shell at its own radius r until
 * the normal force vanishes, where cos α = (N/Nc)² r/R with α measured from the vertical, then flies free under
 * gravity until it reaches radius r again, at the toe. A layer's cycle is closed-form, so every particle is placed
 * from its phase each frame: no integration and no drift. The rigid part of every cycle, taken together, is the
 * en-masse bed, drawn as one solid; the particles show the cascade and the cataract off the shoulder.
 * In the mill's frame, u is along +z and v up; the shell turns towards +u at the bottom when `rising` is 1.
 */
function makeCharge(scene: THREE.Object3D, o: { R: number; L: number; omega: number; fill: number; rising: 1 | -1; n: number; cut: THREE.Plane }) {
  const g = 9.81;
  const { R, omega } = o;
  const k = (omega * omega) / g; // (N/Nc)² / R
  type Layer = { r: number; dep: number; land: number; tRot: number; tFly: number };
  const layer = (r: number): Layer => {
    const dep = Math.asin(Math.min(0.999, k * r)); // above the horizontal, on the rising side
    const p0x = r * Math.cos(dep);
    const p0y = r * Math.sin(dep);
    const vx = -omega * r * Math.sin(dep);
    const vy = omega * r * Math.cos(dep);
    let t = 0.02;
    let land = 0;
    for (; t < 6; t += 0.002) {
      const x = p0x + vx * t;
      const y = p0y + vy * t - 0.5 * g * t * t;
      if (x * x + y * y >= r * r) {
        land = Math.atan2(y, x);
        break;
      }
    }
    if (land < 0) land += Math.PI * 2;
    const tRot = (dep + Math.PI * 2 - land) / omega;
    return { r, dep, land, tRot, tFly: t };
  };
  // Inner radius of the charge chosen so the bed fills the asked fraction of the section.
  const area = (rMin: number) => {
    let A = 0;
    const n = 40;
    for (let i = 0; i < n; i++) {
      const r = rMin + ((i + 0.5) * (R - rMin)) / n;
      const L = layer(r);
      A += (L.dep + Math.PI * 2 - L.land) * r * ((R - rMin) / n);
    }
    return A / (Math.PI * R * R);
  };
  let lo = 0.05 * R;
  let hi = 0.98 * R;
  for (let i = 0; i < 30; i++) {
    const mid = (lo + hi) / 2;
    if (area(mid) > o.fill) lo = mid;
    else hi = mid;
  }
  const rMin = (lo + hi) / 2;
  const outer = layer(R);
  // The bed: outer shell arc from the toe round the bottom to the shoulder, the departure curve in, the inner arc
  // back and the landing curve out.
  const sh = new THREE.Shape();
  // Shape x is −u·rising so that a quarter turn about y puts u on +z·rising without mirroring the solid.
  const pt = (r: number, a: number) => new THREE.Vector2(-o.rising * r * Math.cos(a), r * Math.sin(a));
  const N = 28;
  const pts: THREE.Vector2[] = [];
  for (let i = 0; i <= 48; i++) pts.push(pt(R, outer.land + ((outer.dep + Math.PI * 2 - outer.land) * i) / 48));
  for (let i = 1; i <= N; i++) {
    const r = R - ((R - rMin) * i) / N;
    pts.push(pt(r, layer(r).dep));
  }
  const inner = layer(rMin);
  for (let i = 1; i < 24; i++) pts.push(pt(rMin, inner.dep + Math.PI * 2 - ((inner.dep + Math.PI * 2 - inner.land) * i) / 24));
  for (let i = 0; i <= N; i++) {
    const r = rMin + ((R - rMin) * i) / N;
    pts.push(pt(r, layer(r).land));
  }
  sh.setFromPoints(pts);
  // The bed runs from the section plane (the mill's middle) to the discharge end; its cap is the cut face.
  const bedG = new THREE.ExtrudeGeometry(sh, { depth: o.L / 2 - 0.05, bevelEnabled: false, curveSegments: 1 });
  bedG.translate(0, 0, 0.02);
  bedG.rotateY(Math.PI / 2);
  const bedM = new THREE.MeshStandardMaterial({ color: "#ffffff", map: pebbles(), roughness: 0.62, metalness: 0.12 });
  const group = new THREE.Group();
  const bed = new THREE.Mesh(bedG, bedM);
  bed.receiveShadow = true;
  group.add(bed);
  // Particles on the surface layers: steel balls and ore pieces, each with a layer and a phase.
  const geo = new THREE.IcosahedronGeometry(1, 1);
  const mat = new THREE.MeshStandardMaterial({ color: "#ffffff", roughness: 0.45, metalness: 0.35, clippingPlanes: [o.cut] });
  const inst = new THREE.InstancedMesh(geo, mat, o.n);
  inst.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
  const rnd = (() => {
    let s2 = 1234567;
    return () => ((s2 = (s2 * 16807) % 2147483647) / 2147483647);
  })();
  const parts: { L: Layer; x: number; ph: number; size: number }[] = [];
  // Only the visible part of each cycle is drawn: the last LEAD seconds on the bed's surface below the shoulder, and
  // the flight. Inside the bed a particle would be hidden by the solid anyway.
  const LEAD = 0.7;
  const c = new THREE.Color();
  for (let i = 0; i < o.n; i++) {
    // Most particles in the outer third, where the cataract is.
    const r = rMin + (R - rMin) * (1 - Math.pow(rnd(), 1.8) * 0.9);
    const L = layer(r);
    const ball = rnd() < 0.45;
    // Thickest at the section, where the eye is; the rest thin out towards the discharge end.
    parts.push({ L, x: 0.12 + Math.pow(rnd(), 1.6) * (o.L / 2 - 0.3), ph: rnd() * (LEAD + L.tFly), size: ball ? 0.13 : 0.1 + rnd() * 0.18 });
    inst.setColorAt(i, ball ? c.set("#8d9194") : c.set(rnd() < 0.5 ? "#6e6152" : "#857563"));
  }
  group.add(inst);
  group.visible = false;
  scene.add(group);
  const m4 = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const sc = new THREE.Vector3();
  const pos = new THREE.Vector3();
  let t = 0;
  return {
    group,
    shoulder: (outer.dep * 180) / Math.PI,
    toe: ((outer.land - Math.PI) * 180) / Math.PI,
    flight: outer.tFly,
    update(dt: number) {
      t += Math.min(dt, 0.05);
      for (let i = 0; i < parts.length; i++) {
        const p = parts[i]!;
        const L = p.L;
        const s2 = (t + p.ph) % (LEAD + L.tFly);
        let u: number;
        let v: number;
        if (s2 < LEAD) {
          // Turning with the shell up to the shoulder.
          const a = L.dep - omega * (LEAD - s2);
          u = L.r * Math.cos(a);
          v = L.r * Math.sin(a);
        } else {
          const tf = s2 - LEAD;
          const p0x = L.r * Math.cos(L.dep);
          const p0y = L.r * Math.sin(L.dep);
          u = p0x - omega * L.r * Math.sin(L.dep) * tf;
          v = p0y + omega * L.r * Math.cos(L.dep) * tf - 0.5 * g * tf * tf;
        }
        pos.set(p.x, v, o.rising * u);
        sc.setScalar(p.size);
        q.setFromAxisAngle(pos.clone().normalize(), (t + i) * 0.7);
        m4.compose(pos, q, sc);
        inst.setMatrixAt(i, m4);
      }
      inst.instanceMatrix.needsUpdate = true;
    },
  };
}

/* ---------- Hydrocyclone cluster ---------- */

export function makeCyclones(root: THREE.Object3D, M: Palette, o: { x: number; y: number; z: number; deck: number; n?: number }) {
  const H = o.y + o.deck;
  const n = o.n ?? 10;
  const w = 4.2;
  // Tower: four columns, bracing, an intermediate deck, the top deck with rails, and a stair.
  for (const sx of [-1, 1])
    for (const sz of [-1, 1]) {
      box(root, M.steel, 0.5, o.deck - 0.08, 0.5, o.x + sx * w, o.y + (o.deck - 0.08) / 2, o.z + sz * w, { r: 0.03 });
      box(root, M.concrete, 1.3, 0.6, 1.3, o.x + sx * w, o.y + 0.3, o.z + sz * w, { r: 0.04 });
    }
  for (const h of [o.deck * 0.34, o.deck * 0.67]) {
    for (const [a, b] of [
      [P(-w, 0, -w), P(w, 0, -w)],
      [P(-w, 0, w), P(w, 0, w)],
      [P(-w, 0, -w), P(-w, 0, w)],
      [P(w, 0, -w), P(w, 0, w)],
    ] as [V3, V3][])
      beam(root, M.steel, a.clone().add(P(o.x, o.y + h, o.z)), b.clone().add(P(o.x, o.y + h, o.z)), 0.22, 0.32);
    for (const s of [-1, 1]) {
      bar(root, M.steel, P(o.x - w, o.y + h - o.deck * 0.33, o.z + s * w), P(o.x + w, o.y + h, o.z + s * w), 0.07, 6);
      bar(root, M.steel, P(o.x + s * w, o.y + h - o.deck * 0.33, o.z - w), P(o.x + s * w, o.y + h, o.z + w), 0.07, 6);
    }
  }
  platform(root, M, o.x, o.y + o.deck * 0.5, o.z, 2 * w + 0.6, 2 * w + 0.6, "nse", false);
  platform(root, M, o.x, H, o.z, 2 * w + 1.4, 2 * w + 1.4, "nsew", false);
  // A stair tower on the −z side, arriving at the top deck.
  stairTower(root, M, { x0: o.x - w + 0.4, x1: o.x + w - 0.4, zA: o.z - w - 1.3, zB: o.z - w - 2.5, y0: o.y, y1: H, flights: 4 });

  // Distributor manifold, cyclones in a ring, valves, overflow pipes and launders.
  const yc = H + 2.6;
  cyl(root, M.steelLight, 1.15, 2.8, o.x, yc, o.z, { seg: 28 });
  lathe(root, M.steelLight, [[1.15, 0], [0.6, 0.6], [0, 0.75]], o.x, yc + 1.4, o.z, { seg: 28 });
  cyl(root, M.steelLight, 1.3, 0.14, o.x, yc - 1.0, o.z, { seg: 28 });
  cyl(root, M.steelLight, 1.3, 0.14, o.x, yc + 1.0, o.z, { seg: 28 });
  const rc = 3.3;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    const cx = o.x + ca * rc;
    const cz = o.z + sa * rc;
    // Body: a cylinder head and a long cone, tilted a little outward.
    const body = new THREE.Group();
    body.position.set(cx, yc + 0.2, cz);
    body.rotation.set(0, -a, 0);
    root.add(body);
    const tilt = new THREE.Group();
    tilt.rotation.z = -0.12;
    body.add(tilt);
    cyl(tilt, M.cyclone, 0.4, 1.1, 0, 0.3, 0, { seg: 16 });
    cyl(tilt, M.cyclone, 0.46, 0.12, 0, 0.86, 0, { seg: 16 });
    cyl(tilt, M.cyclone, 0.46, 0.12, 0, -0.24, 0, { seg: 16 });
    lathe(tilt, M.cyclone, [[0.4, -0.25], [0.11, -2.9]], 0, 0, 0, { seg: 16 });
    cyl(tilt, M.rubber, 0.13, 0.4, 0, -3.05, 0, { seg: 10 });
    // Inlet spool from the manifold with a knife-gate valve and its actuator.
    const inA = P(o.x + ca * 1.15, yc + 0.5, o.z + sa * 1.15);
    const inB = P(cx - ca * 0.42, yc + 0.6, cz - sa * 0.42);
    bar(root, M.rubber, inA, inB, 0.17, 10);
    const vp = inA.clone().lerp(inB, 0.45);
    box(root, M.steelDark, 0.25, 0.6, 0.5, vp.x, vp.y, vp.z, { ry: -a, r: 0.02 });
    cyl(root, M.yellow, 0.1, 0.8, vp.x, vp.y + 0.7, vp.z, { seg: 10 });
    // Vortex finder up and over into the overflow launder.
    bar(root, M.rubber, P(cx, yc + 1.05, cz), P(cx, yc + 1.9, cz), 0.17, 10);
    bar(root, M.rubber, P(cx, yc + 1.9, cz), P(o.x + ca * 4.55, yc + 1.75, o.z + sa * 4.55), 0.17, 10);
  }
  lathe(root, M.launder, [[4.0, yc + 2.0], [4.0, yc + 1.2], [5.0, yc + 1.2], [5.0, yc + 2.05]], o.x, 0, o.z, { seg: 48 });
  lathe(root, M.launder, [[2.6, H + 0.9], [2.6, H + 0.2], [4.1, H + 0.2], [4.1, H + 0.95]], o.x, 0, o.z, { seg: 48 });
  // Frame carrying the cluster off the deck.
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2;
    beam(root, M.steel, P(o.x + Math.cos(a) * 3.9, H, o.z + Math.sin(a) * 3.9), P(o.x + Math.cos(a) * 3.9, yc + 1.2, o.z + Math.sin(a) * 3.9), 0.2, 0.2);
  }
  return { top: P(o.x, yc + 2.2, o.z), overflow: P(o.x + 5.0, yc + 1.5, o.z), underflow: P(o.x - 4.1, H + 0.3, o.z), deck: P(o.x + 2.5, H, o.z + 3.5) };
}

/* ---------- Flotation tank cells ---------- */

let frothM: THREE.MeshStandardMaterial | null = null;
let spillM: THREE.MeshStandardMaterial | null = null;
let launderFrothM: THREE.MeshStandardMaterial | null = null;
const frothCache = () => (frothM ??= froth({ colour: "#b9b4a3", size: 8 }));
const spillCache = () => (spillM ??= froth({ colour: "#aea896", spill: true, size: 10 }));
const launderFroth = () => (launderFrothM ??= froth({ colour: "#a39d8a", speed: 0.4, size: 10 }));

/**
 * A flotation tank cell: the tank on its plinth, a peripheral froth launder with froth running in it, the bridge
 * with the mechanism's drive, and froth that domes over the lip and spills into the launder. `launder` says which
 * side (±z) the launder discharge faces.
 */
export function makeFlotationCell(root: THREE.Object3D, scene: THREE.Object3D, M: Palette, o: { x: number; y: number; z: number; r: number; h: number; launder: 1 | -1 }) {
  const top = o.y + o.h;
  slab(root, M.concrete, o.r * 2 + 1.4, o.r * 2 + 1.4, o.x, o.y + 0.5, o.z, 0.5);
  cyl(root, M.cell, o.r, o.h - 0.5, o.x, o.y + 0.5 + (o.h - 0.5) / 2, o.z, { seg: 56, open: true });
  for (let h = 1.7; h < o.h - 1.4; h += 1.6) cyl(root, M.cell, o.r + 0.06, 0.16, o.x, o.y + h, o.z, { seg: 56 });
  cyl(root, M.cell, o.r + 0.08, 0.3, o.x, o.y + 0.65, o.z, { seg: 56 });
  // Peripheral launder: inner lip at the tank wall, a sloped floor, an outer wall a little higher.
  const lw = Math.max(0.8, o.r * 0.24);
  lathe(root, M.launder, [[o.r - 0.02, top + 0.02], [o.r - 0.02, top - 1.0], [o.r + lw, top - 1.25], [o.r + lw, top + 0.18], [o.r + lw + 0.08, top + 0.18]], o.x, 0, o.z, { seg: 56 });
  for (let i = 0; i < 12; i++) {
    const a = (i / 12) * Math.PI * 2;
    beam(root, M.launder, P(o.x + Math.cos(a) * (o.r + 0.05), top - 1.6, o.z + Math.sin(a) * (o.r + 0.05)), P(o.x + Math.cos(a) * (o.r + lw), top - 1.2, o.z + Math.sin(a) * (o.r + lw)), 0.12, 0.12);
  }
  // Launder outlet box and its downcomer on the discharge side.
  const ox = o.x;
  const oz = o.z + o.launder * (o.r + lw + 0.6);
  box(root, M.launder, 1.4, 1.6, 1.3, ox, top - 0.75, oz, { r: 0.04 });
  // Bridge across the top with the mechanism's motor, gearbox and V-belt guard.
  for (const s of [-1, 1]) beam(root, M.steel, P(o.x - o.r - lw, top + 0.75, o.z + s * 0.6), P(o.x + o.r + lw, top + 0.75, o.z + s * 0.6), 0.25, 0.4);
  deck(root, M.grate, 2.6, 2.2, o.x, top + 1.0, o.z);
  box(root, M.gearbox, 1.2, 0.9, 1.2, o.x, top + 1.5, o.z, { r: 0.06 });
  cyl(root, M.motor, 0.5, 1.3, o.x + 0.95, top + 1.95, o.z, { seg: 20 });
  lathe(root, M.motor, [[0.5, 0], [0.32, 0.25], [0, 0.3]], o.x + 0.95, top + 2.6, o.z, { seg: 20 });
  box(root, M.yellow, 1.6, 0.6, 0.35, o.x + 0.45, top + 2.15, o.z - 0.55, { r: 0.05 });
  // Air down the mechanism's standpipe.
  bar(root, M.airPipe, P(o.x - 0.6, top + 1.0, o.z + 0.6), P(o.x - 0.6, top + 2.5, o.z + 0.6), 0.1, 8);
  bar(root, M.airPipe, P(o.x - 0.6, top + 2.5, o.z + 0.6), P(o.x - 0.1, top + 2.5, o.z + 0.1), 0.1, 8);
  // The froth: a slightly domed disc that drifts to the lip, the spill down the lip band, and froth in the launder.
  const fg = new THREE.CircleGeometry(o.r - 0.03, 64);
  const fp = fg.getAttribute("position") as THREE.BufferAttribute;
  for (let i = 0; i < fp.count; i++) {
    const rr = Math.hypot(fp.getX(i), fp.getY(i)) / o.r;
    fp.setZ(i, 0.32 * (1 - rr * rr));
  }
  fg.computeVertexNormals();
  fg.rotateX(-Math.PI / 2);
  const froth = new THREE.Mesh(fg, frothCache());
  froth.position.set(o.x, top + 0.06, o.z);
  froth.receiveShadow = true;
  scene.add(froth);
  const sg = new THREE.CylinderGeometry(o.r + 0.02, o.r + 0.45, 0.95, 64, 1, true);
  const spill = new THREE.Mesh(sg, spillCache());
  spill.position.set(o.x, top - 0.42, o.z);
  spill.receiveShadow = true;
  scene.add(spill);
  const lg = new THREE.RingGeometry(o.r + 0.02, o.r + lw - 0.02, 64, 1);
  lg.rotateX(-Math.PI / 2);
  const lf = new THREE.Mesh(lg, launderFroth());
  lf.position.set(o.x, top - 0.82, o.z);
  lf.receiveShadow = true;
  scene.add(lf);
  return { top, lip: P(o.x, top, o.z), launderOut: P(ox, top - 0.6, oz), lw, deck: P(o.x, top + 1.0, o.z) };
}

/* ---------- Thickeners ---------- */

let mudFloorM: THREE.MeshStandardMaterial | null = null;
const mudFloor = () => (mudFloorM ??= new THREE.MeshStandardMaterial({ color: "#2e2b25", roughness: 1, side: THREE.DoubleSide }));

/**
 * A thickener: the tank wall with its columns, the overflow launder with water running into it, a bridge from the
 * rim to the centre, the drive head, and rake arms that turn slowly under the clear water. A traction thickener's
 * bridge itself turns, carrying the rakes.
 */
export function makeThickener(root: THREE.Object3D, scene: THREE.Object3D, M: Palette, o: { x: number; y: number; z: number; r: number; wall: number; traction?: boolean; speed?: number; bridge?: number; outlet?: number }) {
  const top = o.y + o.wall;
  cyl(root, M.tank, o.r, o.wall, o.x, o.y + o.wall / 2, o.z, { seg: 112, open: true });
  cyl(root, M.tankInner, o.r - 0.05, o.wall, o.x, o.y + o.wall / 2, o.z, { seg: 112, open: true });
  cyl(root, M.concrete, o.r + 0.6, 0.7, o.x, o.y + 0.35, o.z, { seg: 112 });
  // Floor: a shallow cone down to the underflow at the centre.
  lathe(root, mudFloor(), [[0.8, o.y - 1.2], [o.r, o.y + 0.6]], o.x, 0, o.z, { seg: 64 });
  // Overflow launder outside the wall, with a weir and water in it, and the launder outlet.
  lathe(root, M.launder, [[o.r, top - 0.1], [o.r, top - 0.9], [o.r + 1.0, top - 0.9], [o.r + 1.0, top + 0.05]], o.x, 0, o.z, { seg: 112 });
  ringRail(root, M.yellow, o.x, top + 0.05, o.z, o.r + 1.05, { from: 0.25, to: Math.PI * 2 - 0.25 });
  const cols = Math.round(o.r * 1.6);
  for (let i = 0; i < cols; i++) {
    const a = (i / cols) * Math.PI * 2;
    box(root, M.tankDark, 0.35, o.wall - 0.5, 0.35, o.x + Math.cos(a) * (o.r + 0.18), o.y + 0.7 + (o.wall - 0.9) / 2, o.z + Math.sin(a) * (o.r + 0.18), { ry: -a, r: 0 });
  }
  const oa = o.outlet ?? 0;
  box(root, M.launder, 1.6, 1.4, 1.6, o.x + Math.cos(oa) * (o.r + 1.6), top - 0.9, o.z + Math.sin(oa) * (o.r + 1.6), { ry: -oa, r: 0.05 });
  // Centre pier and feedwell.
  cyl(root, M.concrete, 1.2, o.wall + 0.8, o.x, o.y + (o.wall + 0.8) / 2 - 0.6, o.z, { seg: 24 });
  const fw = Math.max(2.2, o.r * 0.16);
  cyl(root, M.tankDark, fw, 2.6, o.x, top - 0.7, o.z, { seg: 40, open: true });
  cyl(root, M.tankDark, fw + 0.06, 0.2, o.x, top + 0.55, o.z, { seg: 40 });
  // Water: clear near the rim, turbid round the feedwell, slightly transparent so the rakes show.
  const g = new THREE.RingGeometry(fw + 0.05, o.r - 0.06, 96, 2);
  g.rotateX(-Math.PI / 2);
  const water = new THREE.Mesh(g, liquidMaterial({ colour: "#3b4437", turbid: "#7c705a", rough: 0.1, ripple: 0.06, opacity: 0.84 }));
  water.position.set(o.x, top - 0.3, o.z);
  water.receiveShadow = true;
  water.renderOrder = 1;
  scene.add(water);
  const fwG = new THREE.CircleGeometry(fw - 0.05, 40);
  fwG.rotateX(-Math.PI / 2);
  const slurryIn = new THREE.Mesh(fwG, liquidMaterial({ colour: "#6c6250", turbid: "#857a63", rough: 0.3, swirl: 0.6, ripple: 0.4, foam: "#a39882" }));
  slurryIn.position.set(o.x, top - 0.1, o.z);
  scene.add(slurryIn);
  const launderWater = new THREE.Mesh(new THREE.RingGeometry(o.r + 0.05, o.r + 0.95, 96, 1).rotateX(-Math.PI / 2), liquidMaterial({ colour: "#4a5446", rough: 0.08, ripple: 0.9 }));
  launderWater.position.set(o.x, top - 0.6, o.z);
  scene.add(launderWater);

  // Bridge: a truss from the rim to the centre (a traction bridge turns, carrying its rakes).
  const turn = new THREE.Group();
  const span = o.r + 0.6;
  const src = new THREE.Group();
  const yb = 1.0;
  for (const s of [-1, 1]) {
    beam(src, M.steel, P(0, yb, s * 0.85), P(span, yb, s * 0.85), 0.2, 0.32);
    beam(src, M.steel, P(0, yb + 1.5, s * 0.85), P(span, yb + 1.5, s * 0.85), 0.15, 0.22);
    for (let d = 0; d < span - 0.1; d += 1.7) bar(src, M.steel, P(d, yb, s * 0.85), P(Math.min(span, d + 1.7), yb + 1.5, s * 0.85), 0.05, 5);
  }
  deck(src, M.grate, span, 1.5, span / 2, yb + 0.06, 0, 0.05);
  rail(src, M.yellow, [P(0.5, yb + 0.06, 0.82), P(span, yb + 0.06, 0.82)], { posts: 2.2 });
  rail(src, M.yellow, [P(0.5, yb + 0.06, -0.82), P(span, yb + 0.06, -0.82)], { posts: 2.2 });
  // Feed pipe along the bridge into the feedwell.
  bar(src, M.pipe, P(span, yb - 0.4, 1.2), P(fw * 0.8, yb - 0.4, 1.2), 0.22, 10);
  // Rake arms with blades, just under the water, and the torque cage.
  const rakes = new THREE.Group();
  const rk = new THREE.Group();
  for (const s of [0, Math.PI]) {
    const arm = new THREE.Group();
    arm.rotation.y = s;
    rk.add(arm);
    beam(arm, M.dark, P(0.6, -0.6, 0), P(o.r - 0.6, -0.95, 0), 0.32, 0.45);
    beam(arm, M.dark, P(0.6, -0.6, 0.4), P(o.r * 0.6, -0.8, 0.4), 0.15, 0.15);
    for (let d = 1.6; d < o.r - 0.8; d += 1.4) box(arm, M.dark, 0.12, 0.45, 1.5, d, -0.95 - d * 0.02, 0, { ry: 0.55, r: 0 });
  }
  for (let i = 0; i < 4; i++) box(rk, M.dark, 0.18, 1.6, 0.18, Math.cos(i * 1.57) * 0.9, 0.1, Math.sin(i * 1.57) * 0.9, { r: 0 });
  rakes.add(bake(rk, { mudTop: -40, mudBottom: -50, dust: 0.2 }));
  rakes.position.set(o.x, top - 0.25, o.z);
  scene.add(rakes);
  if (o.traction) {
    box(src, M.motor, 1.6, 1.2, 1.8, span - 0.4, yb + 0.6, 0, { r: 0.08 });
    cyl(src, M.rubber, 0.45, 0.4, span - 0.4, yb - 0.55, 0.7, { axis: "z", seg: 16 });
  }
  const baked = bake(src, { mudTop: -40, mudBottom: -50, dust: 0.6 });
  turn.add(baked);
  turn.position.set(o.x, o.y + o.wall, o.z);
  turn.rotation.y = o.bridge ?? 0;
  turn.traverse((m) => {
    m.castShadow = true;
    m.receiveShadow = true;
  });
  scene.add(turn);
  // Drive head on top of the pier: a slewing-ring drive, motors and a lifting device.
  cyl(root, M.gearbox, 1.7, 1.1, o.x, top + 1.45, o.z, { seg: 32 });
  cyl(root, M.gearbox, 1.85, 0.2, o.x, top + 2.0, o.z, { seg: 32 });
  for (const a of [0.4, 0.4 + Math.PI]) cyl(root, M.motor, 0.32, 0.9, o.x + Math.cos(a) * 1.4, top + 2.55, o.z + Math.sin(a) * 1.4, { seg: 14 });
  box(root, M.guard, 0.9, 1.2, 0.9, o.x, top + 2.6, o.z, { r: 0.06 });
  deck(root, M.grate, 4.6, 4.6, o.x, top + 1.0, o.z);
  const head = new THREE.Mesh(new THREE.CylinderGeometry(0.5, 0.5, 0.35, 6), M.yellow);
  head.position.set(o.x, top + 3.4, o.z);
  scene.add(head);
  const wRake = (((o.speed ?? 0.25) * Math.PI * 2) / 60) * 1;
  return {
    top,
    centre: P(o.x, top, o.z),
    rim: (a: number) => P(o.x + Math.cos(a) * (o.r + 0.5), top, o.z + Math.sin(a) * (o.r + 0.5)),
    update(dt: number) {
      rakes.rotation.y += wRake * dt;
      if (o.traction) turn.rotation.y = rakes.rotation.y;
      head.rotation.y += 0.4 * dt;
    },
  };
}

/* ---------- Carbon-in-leach tanks ---------- */

let slurryM: THREE.MeshPhysicalMaterial | null = null;
export function makeCilTank(root: THREE.Object3D, scene: THREE.Object3D, M: Palette, o: { x: number; y: number; z: number; r: number; h: number; spin?: number }) {
  const top = o.y + o.h;
  cyl(root, M.concrete, o.r + 0.7, 0.8, o.x, o.y + 0.4 - 0.6, o.z, { seg: 56 });
  cyl(root, M.tank, o.r, o.h, o.x, o.y + o.h / 2, o.z, { seg: 64, open: true });
  for (let h = 1.8; h < o.h - 0.5; h += 2.2) cyl(root, M.tank, o.r + 0.06, 0.18, o.x, o.y + h, o.z, { seg: 64 });
  cyl(root, M.tankDark, o.r + 0.1, 0.35, o.x, top - 0.17, o.z, { seg: 64 });
  // Manway and a drain valve at the foot.
  box(root, M.tankDark, 1.0, 1.0, 0.25, o.x, o.y + 1.2, o.z + o.r + 0.05, { r: 0.05 });
  cyl(root, M.steelLight, 0.18, 1.2, o.x + 1.6, o.y + 0.6, o.z + o.r + 0.4, { axis: "z", seg: 12 });
  // Agitator bridge across the tank, with the gearbox, a vertical motor and a coupling guard.
  for (const s of [-1, 1]) beam(root, M.steel, P(o.x + s * 0.8, top + 0.5, o.z - o.r - 0.35), P(o.x + s * 0.8, top + 0.5, o.z + o.r + 0.35), 0.3, 0.6);
  deck(root, M.grate, 2.8, o.r * 2 + 0.8, o.x, top + 0.82, o.z);
  for (const s of [-1, 1]) rail(root, M.yellow, [P(o.x + s * 1.4, top + 0.82, o.z - o.r - 0.4), P(o.x + s * 1.4, top + 0.82, o.z - 1.1)]);
  for (const s of [-1, 1]) rail(root, M.yellow, [P(o.x + s * 1.4, top + 0.82, o.z + 1.1), P(o.x + s * 1.4, top + 0.82, o.z + o.r + 0.4)]);
  box(root, M.gearbox, 1.7, 1.2, 1.7, o.x, top + 1.45, o.z, { r: 0.1 });
  for (let i = 0; i < 4; i++) box(root, M.gearbox, 1.8, 0.08, 0.08, o.x, top + 1.0 + i * 0.3, o.z + 0.86, { r: 0 });
  box(root, M.yellow, 0.9, 0.6, 0.9, o.x, top + 2.35, o.z, { r: 0.04 });
  cyl(root, M.motor, 0.55, 1.6, o.x, top + 3.45, o.z, { seg: 22 });
  lathe(root, M.motor, [[0.55, 0], [0.35, 0.25], [0, 0.3]], o.x, top + 4.25, o.z, { seg: 22 });
  box(root, M.motor, 0.4, 0.4, 0.3, o.x + 0.6, top + 3.3, o.z, { r: 0.03 });
  // An interstage screen in the slurry on the outlet side, with its drive, and the launder to the next tank.
  const sx = o.x + o.r - 1.3;
  const sz = o.z - o.r * 0.4;
  cyl(root, M.stainless, 0.7, 2.0, sx, top - 0.1, sz, { seg: 20 });
  cyl(root, M.motor, 0.28, 0.7, sx, top + 1.25, sz, { seg: 12 });
  box(root, M.steel, 2.0, 0.2, 0.3, sx - 0.6, top + 0.85, sz, { r: 0.02 });
  box(root, M.launder, 3.4, 0.7, 1.0, o.x + o.r + 1.1, top - 0.35, sz, { r: 0.04 });
  ringRail(root, M.yellow, o.x, top + 0.02, o.z, o.r + 0.05, { from: 0.35, to: Math.PI * 2 - 0.35 });
  // Recessed-impeller carbon transfer pump on the rim.
  cyl(root, M.pump, 0.35, 1.4, o.x - o.r + 1.0, top + 0.2, o.z + 1.2, { seg: 14 });
  cyl(root, M.motor, 0.26, 0.6, o.x - o.r + 1.0, top + 1.2, o.z + 1.2, { seg: 12 });
  const g = new THREE.CircleGeometry(o.r - 0.04, 64);
  slurryM ??= liquidMaterial({ colour: "#4a443b", swirl: 0.9, foam: "#a2977f", turbid: "#55504a", rough: 0.24, ripple: 0.55 });
  const slurry = new THREE.Mesh(g, slurryM);
  g.rotateX(-Math.PI / 2);
  slurry.position.set(o.x, top - 0.45, o.z);
  slurry.receiveShadow = true;
  scene.add(slurry);
  const shaft = new THREE.Group();
  cyl(shaft, M.steelLight, 0.16, 1.1, 0, -0.25, 0, { seg: 10 });
  box(shaft, M.guard, 0.8, 0.2, 0.2, 0, 0.22, 0, { r: 0.02 });
  shaft.position.set(o.x, top + 2.35, o.z);
  scene.add(shaft);
  const w = o.spin ?? 4.2;
  return {
    top,
    centre: P(o.x, top, o.z),
    deck: P(o.x, top + 0.82, o.z),
    update(dt: number) {
      shaft.rotation.y += dt * w;
    },
  };
}

/* ---------- Elution, carbon regeneration ---------- */

export function makeElution(root: THREE.Object3D, M: Palette, o: { x: number; y: number; z: number }) {
  // A steel frame with two platform levels, the elution column (insulated) and the acid wash column (FRP).
  const { x, y, z } = o;
  for (const sx of [-1, 1]) for (const sz of [-1, 1]) box(root, M.steel, 0.3, 13, 0.3, x + sx * 3.2, y + 6.5, z + sz * 2.7, { r: 0.02 });
  for (const h of [4.4, 9.0])
    for (const s of [-1, 1]) {
      bar(root, M.steel, P(x - 3.2, y + h - 4.2, z + s * 2.7), P(x + 3.2, y + h, z + s * 2.7), 0.06, 6);
    }
  platform(root, M, x, y + 4.4, z, 6.8, 5.8, "nsw", false);
  platform(root, M, x, y + 9.0, z, 6.8, 5.8, "nsew", false);
  stairs(root, M.steel, M.grate, M.yellow, P(x + 4.6, y + 0.04, z + 3.8), P(x + 4.6, y + 4.4, z + 0.4), 0.9);
  cyl(root, M.insul, 1.0, 12, x - 1.3, y + 6.6, z, { seg: 32 });
  for (let h = 1.5; h < 12; h += 1.2) cyl(root, M.insul, 1.03, 0.04, x - 1.3, y + h, z, { seg: 32 });
  lathe(root, M.insul, [[1.0, 0], [0.55, 0.8], [0, 1.0]], x - 1.3, y + 12.6, z, { seg: 32 });
  cyl(root, M.frp, 0.95, 7, x + 1.5, y + 4.3, z - 0.6, { seg: 28 });
  lathe(root, M.frp, [[0.95, 0], [0, 0.65]], x + 1.5, y + 7.8, z - 0.6, { seg: 28 });
  // Heat exchangers, a strip-solution heater and an eluate tank.
  for (const s of [0, 1]) cyl(root, M.stainless, 0.34, 4.4, x - 0.5, y + 1.2 + s * 0.85, z + 4.6, { axis: "x", seg: 16 });
  box(root, M.red, 2.0, 2.2, 1.6, x - 4.6, y + 1.1, z + 4.4, { r: 0.08 });
  cyl(root, M.tank, 2.2, 4.0, x + 7.0, y + 2.0, z + 3.6, { seg: 36 });
  lathe(root, M.tank, [[2.2, 0], [0, 0.45]], x + 7.0, y + 4.0, z + 3.6, { seg: 36 });
  // Carbon regeneration kiln: a rotary tube on rollers, a burner box, a dewatering screen over the feed end.
  const kx = x - 10;
  const ky = y + 3.4;
  cyl(root, M.rust, 0.75, 10, kx, ky, z + 1, { axis: "x", seg: 28, rz: 0.04 });
  for (let i = -3; i <= 3; i++) cyl(root, M.refractory, 0.8, 0.12, kx + i * 1.3, ky - i * 0.05, z + 1, { axis: "x", seg: 28 });
  for (const s of [-1, 1]) {
    box(root, M.steel, 0.6, ky - y - 0.4, 1.9, kx + s * 3.8, y + (ky - y - 0.4) / 2, z + 1, { r: 0.03 });
    cyl(root, M.steelLight, 0.9, 0.32, kx + s * 3.8, ky, z + 1, { axis: "x", seg: 28 });
  }
  box(root, M.refractory, 2.8, 3.2, 2.6, kx - 6.2, ky - 0.2, z + 1, { r: 0.06 });
  box(root, M.steel, 2.4, 1.3, 2.2, kx + 5.8, ky + 1.7, z + 1, { r: 0.05 });
  bar(root, M.steelLight, P(kx - 6.2, ky + 1.4, z + 1), P(kx - 6.2, ky + 9, z + 1), 0.28, 12);
  return { column: P(x - 1.3, y + 13.6, z), kiln: P(kx, ky + 1.2, z + 1), deck: P(x, y + 4.4, z) };
}

/* ---------- The gold room ---------- */

export interface GoldRoom {
  /** Walls and roof that fade so the camera can look in; they keep their shadow so the room stays dim. */
  cutaway(amount: number): void;
  update(dt: number): void;
  furnace: THREE.Vector3;
  moulds: THREE.Vector3;
  scale: THREE.Vector3;
  light: THREE.PointLight;
  /** Whether metal is flowing. */
  readonly pouring: boolean;
  /** Starts the pour cycle again from `at` seconds (the furnace tilts at 3 s, metal flows from 5.6 s to 15.2 s). */
  restart(at?: number): void;
}

/**
 * A gold room: a block-walled building (its front wall and roof fade for the view inside, still shading the room)
 * with electrowinning cells, a rectifier, a calcining oven, a tilting induction furnace under its fume hood that
 * pours doré into a cascade of moulds, a scale and a safe.
 */
export function makeGoldRoom(root: THREE.Object3D, scene: THREE.Object3D, M: Palette, o: { x: number; y: number; z: number; w: number; d: number; h: number }): GoldRoom {
  const { x, y, z, w, d, h } = o;
  slab(root, M.concrete, w + 2.4, d + 2.4, x, y + 0.3, z, 0.6);
  const fy = y + 0.3;
  slab(root, M.floor, w - 0.4, d - 0.4, x, fy + 0.02, z, 0.05);
  // Yellow walkway lines on the floor.
  for (const s of [-1, 1]) box(root, M.yellow, w - 1.2, 0.012, 0.08, x, fy + 0.03, z + d * 0.12 + s * 0.7, { r: 0 });
  // Back and side walls are fixed; the front (+z) wall and the roof go into their own group, to fade.
  const wallT = 0.3;
  const blk = (mat: THREE.Material, ww: number, hh: number, dd: number, px: number, pz: number) => {
    // A box with UVs in metres so the block courses read at true size.
    const gg = new THREE.BoxGeometry(ww, hh, dd);
    const uv = gg.getAttribute("uv") as THREE.BufferAttribute;
    const nn = gg.getAttribute("normal") as THREE.BufferAttribute;
    for (let i = 0; i < uv.count; i++) {
      const across = Math.abs(nn.getX(i)) > 0.5 ? dd : ww;
      uv.setXY(i, uv.getX(i) * across, uv.getY(i) * hh);
    }
    const m = new THREE.Mesh(gg, mat);
    m.position.set(px, fy + hh / 2, pz);
    return m;
  };
  root.add(blk(M.block, w, h, wallT, x, z - d / 2));
  root.add(blk(M.block, wallT, h, d, x - w / 2, z));
  root.add(blk(M.block, wallT, h, d, x + w / 2, z));
  // A bond beam course and parapet on the fixed walls.
  box(root, M.concrete, w + 0.4, 0.4, 0.4, x, fy + h + 0.2, z - d / 2, { r: 0.02 });
  for (const s of [-1, 1]) box(root, M.concrete, 0.36, 0.37, d + 0.36, x + (s * w) / 2, fy + h + 0.195, z, { r: 0.02 });
  const fade = new THREE.Group();
  const blockF = finish("#d3ccbb", { rough: 0.9, coat: 0, rust: 0, streak: 0.08, streakColour: "#8a7f6c", normalMap: blockNormal(), normalScale: 0.8, scale: 0.08, mud: 0.4, dust: 0.4, transparent: true });
  const roofM = finish("#a7aca8", { rough: 0.5, coat: 0.15, metal: 0.4, rust: 0.15, streak: 0, normalMap: M.ribs, transparent: true });
  const front = new THREE.Group();
  front.add(blk(blockF, w, h, wallT, x, z + d / 2));
  box(front, blockF, w + 0.4, 0.4, 0.4, x, fy + h + 0.2, z + d / 2, { r: 0.02 });
  // The roof is a slab with thickness, overlapping the parapets: a zero-thickness plane at the wall heads lets the
  // low sun leak through the joint in the shadow map as a bright wedge on the floor.
  const roofG = new THREE.BoxGeometry(w + 0.6, 0.3, d + 0.6);
  const ruv = roofG.getAttribute("uv") as THREE.BufferAttribute;
  const rn = roofG.getAttribute("normal") as THREE.BufferAttribute;
  for (let i = 0; i < ruv.count; i++) if (Math.abs(rn.getY(i)) > 0.5) ruv.setXY(i, ruv.getY(i) * d, ruv.getX(i) * w);
  const roof = new THREE.Mesh(roofG, roofM);
  roof.position.set(x, fy + h + 0.33, z);
  fade.add(bake(front, { mudTop: 1, dust: 0.4 }), roof);
  fade.traverse((m) => {
    m.castShadow = true;
    m.receiveShadow = true;
  });
  scene.add(fade);
  // A steel roller door in the east wall, under its hood, and a personnel door beside it.
  box(root, M.steelLight, 0.12, 3.6, 3.4, x + w / 2 + 0.2, fy + 1.8, z + d * 0.18, { r: 0.02 });
  for (let i = 0; i < 16; i++) box(root, M.steelLight, 0.05, 0.03, 3.4, x + w / 2 + 0.27, fy + 0.2 + i * 0.22, z + d * 0.18, { r: 0 });
  box(root, M.steelDark, 0.6, 0.5, 3.8, x + w / 2 + 0.4, fy + 3.9, z + d * 0.18, { r: 0.03 });
  box(root, M.steelDark, 0.1, 2.2, 1.0, x + w / 2 + 0.2, fy + 1.1, z - d * 0.2, { r: 0.02 });
  box(root, M.yellow, 0.4, 0.04, 1.6, x + w / 2 + 0.5, fy + 0.02, z + d * 0.18, { r: 0 });
  // Inside: two electrowinning cells with cathodes and a busbar, a rectifier and the calcining oven.
  for (const s of [-1, 1]) {
    const cz = z - d * 0.26 + s * 1.3;
    box(root, M.stainless, 4.8, 1.1, 1.3, x - w * 0.24, fy + 0.6, cz, { r: 0.05 });
    box(root, M.steelDark, 4.9, 0.4, 1.4, x - w * 0.24, fy + 0.2, cz, { r: 0.02 });
    for (let i = 0; i < 12; i++) box(root, M.dark, 0.04, 0.55, 1.0, x - w * 0.24 - 2.1 + i * 0.38, fy + 1.35, cz, { r: 0 });
    beam(root, M.copper, P(x - w * 0.24 - 2.4, fy + 1.66, cz - 0.55), P(x - w * 0.24 + 2.4, fy + 1.66, cz - 0.55), 0.08, 0.08);
  }
  box(root, M.steelLight, 1.6, 2.2, 0.9, x - w * 0.44, fy + 1.1, z - d * 0.42, { r: 0.04 });
  box(root, M.dark, 1.62, 0.5, 0.92, x - w * 0.44, fy + 1.8, z - d * 0.42, { r: 0.02 });
  box(root, M.steelDark, 2.0, 2.0, 1.8, x + w * 0.04, fy + 1.0, z - d * 0.34, { r: 0.06 });
  box(root, M.dark, 1.2, 1.1, 0.06, x + w * 0.04, fy + 1.1, z - d * 0.34 + 0.92, { r: 0.02 });
  // Fluorescent battens under the roof; they go with the roof in the cutaway, and a fill light stands in for them.
  const battenG = new THREE.Group();
  for (let i = 0; i < 3; i++) for (const s of [-1, 1]) box(battenG, M.lamp, 1.4, 0.06, 0.16, x - w * 0.3 + i * w * 0.3, fy + h - 0.3, z + s * d * 0.22, { r: 0 });
  const battens = bake(battenG, {});
  scene.add(battens);
  const fill = new THREE.PointLight("#e9eef5", 0, 24, 1.1);
  fill.position.set(x - w * 0.05, fy + h - 1.2, z + d * 0.15);
  scene.add(fill);
  // Furnace bay: a hood with its duct through the roof, the furnace on its tilting frame, and the mould cascade.
  const fx = x + w * 0.2;
  const fz = z - d * 0.08;
  box(root, M.stainless, 3.0, 1.2, 3.0, fx + 0.6, fy + h - 1.5, fz, { r: 0.05 });
  lathe(root, M.stainless, [[2.0, 0], [0.45, 0.9]], fx + 0.6, fy + h - 0.9, fz, { seg: 4, ry: Math.PI / 4 });
  bar(root, M.stainless, P(fx + 0.6, fy + h - 0.1, fz), P(fx + 0.6, fy + h + 3.2, fz), 0.42, 14);
  for (const s of [-1, 1]) box(root, M.steelDark, 0.26, 1.5, 0.26, fx, fy + 0.75, fz + s * 0.8, { r: 0.02 });
  box(root, M.steelDark, 1.6, 0.3, 2.0, fx, fy + 0.15, fz, { r: 0.03 });
  box(root, M.steelDark, 0.9, 0.9, 0.7, fx - 1.4, fy + 0.45, fz - 0.9, { r: 0.04 });
  const tilt = new THREE.Group();
  tilt.position.set(fx, fy + 1.45, fz);
  const pot = new THREE.Group();
  // An induction furnace: a painted steel shell on trunnions, a refractory top and pouring lip, the lid swung open.
  const shell = finish("#3d566b", { rough: 0.45, coat: 0.4, rust: 0.1, streak: 0.2, streakColour: "#2a2a26" });
  cyl(pot, shell, 0.64, 1.0, 0, 0.32, 0, { seg: 28 });
  for (const yy of [-0.12, 0.76]) cyl(pot, M.steelDark, 0.67, 0.08, 0, yy, 0, { seg: 28 });
  cyl(pot, M.refractory, 0.6, 0.1, 0, 0.86, 0, { seg: 28 });
  box(pot, M.refractory, 0.34, 0.12, 0.3, 0.66, 0.84, 0, { r: 0.02 });
  for (const s of [-1, 1]) cyl(pot, M.steelDark, 0.12, 0.22, 0, 0.45, s * 0.74, { axis: "z", seg: 12 });
  const lid = new THREE.Group();
  cyl(lid, M.refractory, 0.5, 0.12, 0, 0, 0, { seg: 24 });
  cyl(lid, shell, 0.54, 0.06, 0, 0.08, 0, { seg: 24 });
  lid.rotation.z = 1.75;
  lid.position.set(-0.68, 1.1, 0);
  pot.add(lid);
  // Water-cooled power leads down to the frame.
  for (const s of [-1, 1]) bar(pot, M.black, P(-0.5, -0.05, s * 0.3), P(-1.0, -1.2, s * 0.5), 0.05, 6);
  const potB = bake(pot, { mudTop: -9, dust: 0.3 });
  tilt.add(potB);
  const melt = moltenMaterial();
  const surf = new THREE.Mesh(new THREE.CircleGeometry(0.44, 24).rotateX(-Math.PI / 2), melt);
  surf.position.y = 0.8;
  tilt.add(surf);
  const mouth = glow("#ffb060", 1.6, 0.9);
  mouth.position.set(0, 1.1, 0);
  tilt.add(mouth);
  scene.add(tilt);
  // The cascade: five moulds stepping down towards +x on a trolley.
  const mx0 = fx + 1.0;
  const my0 = fy + 1.08;
  const steps: THREE.Vector3[] = [];
  for (let i = 0; i < 5; i++) {
    const p = P(mx0 + i * 0.44, my0 - i * 0.09, fz);
    steps.push(p);
    box(root, M.black, 0.4, 0.13, 0.3, p.x, p.y, p.z, { r: 0.01 });
  }
  box(root, M.steelDark, 2.6, 0.95, 0.7, mx0 + 0.9, fy + 0.48, fz, { r: 0.03 });
  for (const s of [-1, 1]) cyl(root, M.dark, 0.12, 0.1, mx0 + 0.9 + s * 1.1, fy + 0.12, fz + 0.36, { axis: "z", seg: 10 });
  // Scale and safe.
  box(root, M.steelLight, 1.4, 0.95, 0.8, x + w * 0.38, fy + 0.48, z + d * 0.02, { r: 0.04 });
  box(root, M.white, 0.6, 0.1, 0.5, x + w * 0.38, fy + 1.0, z + d * 0.02, { r: 0.02 });
  box(root, M.steelDark, 1.4, 1.9, 1.2, x + w * 0.42, fy + 0.95, z - d * 0.36, { r: 0.06 });
  // Cooled bars on the scale bench: three earlier bars, dull gold.
  const doreM = new THREE.MeshStandardMaterial({ color: "#c9a24a", metalness: 1, roughness: 0.32 });
  for (let i = 0; i < 3; i++) {
    const b = new THREE.Mesh(new THREE.BoxGeometry(0.28, 0.08, 0.15), doreM);
    b.position.set(x + w * 0.38 - 0.42 + i * 0.32, fy + 0.99, z + d * 0.02 + 0.22);
    root.add(b);
  }
  const barM = moltenMaterial();
  const bars = steps.map((p) => {
    const b = new THREE.Mesh(new THREE.BoxGeometry(0.3, 0.08, 0.2), barM);
    b.position.set(p.x, p.y + 0.07, p.z);
    b.scale.y = 0.001;
    scene.add(b);
    return b;
  });
  const barGlow = steps.map((p) => {
    const s = glow("#ff9a40", 0.9, 0);
    s.position.set(p.x, p.y + 0.2, p.z);
    scene.add(s);
    return s;
  });
  const stream = new THREE.Mesh(new THREE.CylinderGeometry(0.035, 0.028, 1, 8), melt);
  scene.add(stream);
  const streamGlow = glow("#ffb45a", 1.1, 0);
  scene.add(streamGlow);
  // Sparks where the metal lands.
  const NS = 48;
  const sparkPos = new Float32Array(NS * 3);
  const sparkVel = new Float32Array(NS * 3);
  const sparkLife = new Float32Array(NS);
  const sg = new THREE.BufferGeometry();
  sg.setAttribute("position", new THREE.BufferAttribute(sparkPos, 3));
  const sparks = new THREE.Points(sg, new THREE.PointsMaterial({ color: "#ffc46a", size: 0.05, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending }));
  sparks.frustumCulled = false;
  scene.add(sparks);
  const light = new THREE.PointLight("#ff9440", 0, 18, 1.4);
  light.position.set(fx + 1.0, fy + 2.0, fz + 0.9);
  scene.add(light);
  let t = 2;
  let pouring = false;
  const heat = melt.userData.heat as { value: number };
  const barHeat = barM.userData.heat as { value: number };
  const spout = new THREE.Vector3();
  let spark = 0;
  const update = (dt: number) => {
    t = (t + dt) % 26;
    // 0–3 upright; 3–6 tilt; 6–15 pour along the cascade; 15–18 back; 18–26 the bars cool.
    const tiltA = t < 3 ? 0 : t < 6 ? smooth((t - 3) / 3) : t < 15 ? 1 : t < 18 ? 1 - smooth((t - 15) / 3) : 0;
    tilt.rotation.z = -tiltA * 1.15;
    pouring = t > 5.6 && t < 15.2;
    const fill = (k: number) => Math.min(1, Math.max(0, (t - 5.8 - k * 1.8) / 1.7));
    bars.forEach((b, k) => (b.scale.y = t < 1 ? 0.001 : Math.max(0.001, fill(k))));
    barHeat.value = t < 15 ? 1 : 1 - smooth((t - 15) / 10) * 0.92;
    barGlow.forEach((g, k) => (g.material.opacity = (t < 1 ? 0 : Math.min(1, fill(k) * 1.5)) * barHeat.value * 0.75));
    heat.value = 1;
    tilt.updateMatrixWorld();
    spout.set(0.7, 0.82, 0).applyMatrix4(tilt.matrixWorld);
    const k = Math.min(4, Math.max(0, Math.floor((t - 5.8) / 1.8)));
    const target = steps[k]!;
    const len = Math.max(0.05, spout.y - (target.y + 0.06));
    stream.visible = pouring;
    stream.scale.set(1, len, 1);
    stream.position.set((spout.x + target.x) / 2, (spout.y + target.y + 0.06) / 2, fz);
    streamGlow.position.copy(stream.position);
    streamGlow.material.opacity = pouring ? 0.95 : 0;
    mouth.material.opacity = 0.35 + tiltA * 0.6;
    // Sparks: spawn while pouring, fall under gravity and fade.
    spark += dt * (pouring ? 60 : 0);
    for (let i = 0; i < NS; i++) {
      if (sparkLife[i]! <= 0 && spark >= 1) {
        spark -= 1;
        sparkLife[i] = 0.4 + Math.random() * 0.5;
        sparkPos.set([target.x, target.y + 0.08, target.z], i * 3);
        const a = Math.random() * Math.PI * 2;
        const v = 0.8 + Math.random() * 1.6;
        sparkVel.set([Math.cos(a) * v * 0.6, 1.2 + Math.random() * 1.8, Math.sin(a) * v * 0.6], i * 3);
      }
      if (sparkLife[i]! > 0) {
        sparkLife[i]! -= dt;
        sparkVel[i * 3 + 1]! -= 9.8 * dt;
        for (let c = 0; c < 3; c++) sparkPos[i * 3 + c]! += sparkVel[i * 3 + c]! * dt;
        if (sparkLife[i]! <= 0) sparkPos.set([0, -100, 0], i * 3);
      }
    }
    sg.attributes.position!.needsUpdate = true;
    light.intensity = pouring ? 70 + Math.sin(t * 23) * 8 : 14 * barHeat.value + 6;
  };
  return {
    cutaway(a) {
      // The front wall goes entirely (it still casts its shadow); the roof stays as a faint shade.
      blockF.opacity = 1 - a;
      roofM.opacity = 1 - a * 0.92;
      for (const m of [blockF, roofM]) m.depthWrite = a < 0.5;
      battens.visible = a < 0.5;
      fill.intensity = a * 46;
    },
    update,
    furnace: P(fx, fy + 1.45, fz),
    moulds: steps[2]!.clone(),
    scale: P(x + w * 0.38, fy + 1.0, z + d * 0.02),
    light,
    get pouring() {
      return pouring;
    },
    restart(at = 2) {
      t = at;
    },
  };
}

/** Points of a polyline offset vertically, for flows that run just above a belt or pipe. */
export const lift = (pts: V3[], dy: number) => pts.map((p) => p.clone().setY(p.y + dy));
export { v3, sphere };
