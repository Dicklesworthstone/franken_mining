/**
 * What stands on and around the tailings facility: the spigot line along the crest with its offtakes (a few open,
 * pouring slurry onto the beach), the survey prisms, the piezometer heads and their loggers at section A–A, the decant
 * tower in the pond with its bridge and causeway, the pipelines that carry tailings in and water back, and the lined
 * seepage pond at the toe with its weir and pump. Metres, y up.
 */
import * as THREE from "three";
import { bake, bar, box, cyl, grime, mats, paint, rail, v3 } from "./fleet-kit";
import { mix } from "./lib";
import { CREST, crestZ, DAM, damTop, HOLES, INCLINOMETERS, natural, POND, PRISMS, SECTION_X, SEEP, seepDepth, settle, tailings } from "./tailings-model";
import type { Road } from "./tailings-ground";
import { stillWater } from "./tailings-water";

/** The spigot offtakes along the crest; slurry runs from these. */
export const OPEN_SPIGOTS = [-216, -120, -24, 72, 168];
const SPIGOT_S = -6.25;
const PIPE_R = 0.28;

const hdpe = () => grime(new THREE.MeshStandardMaterial({ color: "#161616", roughness: 0.48, metalness: 0 }), { dust: "#b3ab9c", dustAmount: 0.8, mudAmount: 0.2, scale: 0.3 });
const crestY = (x: number, s: number) => damTop(s) - settle(x);
const P = (x: number, s: number, y: number) => v3(x, y, crestZ(x) + s);

/** A pipe lying along a polyline of points (already at the pipe's centre height). */
function pipe(pts: THREE.Vector3[], r: number, mat: THREE.Material) {
  const curve = new THREE.CatmullRomCurve3(pts, false, "centripetal");
  const g = new THREE.TubeGeometry(curve, Math.max(8, Math.round(curve.getLength() / 2)), r, 8, false);
  const m = new THREE.Mesh(g, mat);
  m.castShadow = true;
  m.receiveShadow = true;
  return m;
}

/* ---------- The spigot line ---------- */
export function makeSpigotLine() {
  const root = new THREE.Group();
  const black = hdpe();
  const steel = mats.galv();
  const concrete = mats.concrete();
  const valve = paint("#2f5d8a", { coat: 0.3 });
  const pts: THREE.Vector3[] = [];
  for (let x = -306; x <= 306; x += 6) pts.push(P(x, SPIGOT_S, crestY(x, SPIGOT_S) + 0.16 + PIPE_R));
  root.add(pipe(pts, PIPE_R, black));
  // Sleepers under the pipe.
  for (let x = -300; x <= 300; x += 9) {
    const y = crestY(x, SPIGOT_S);
    const b = box(root, concrete, 0.3, 0.16, 0.9, x, y + 0.08, crestZ(x) + SPIGOT_S, { r: 0.02 });
    b.rotation.y = -Math.atan(2e-4 * x);
  }
  const outlets: THREE.Vector3[] = [];
  const streams = new THREE.Group();
  const slurry = new THREE.MeshStandardMaterial({ color: "#5d5a54", roughness: 0.14 });
  for (let x = -288; x <= 288; x += 24) {
    const y = crestY(x, SPIGOT_S) + 0.16 + PIPE_R;
    const zc = crestZ(x);
    // Tee off the side, a knife-gate valve, then a short drop over the upstream edge.
    bar(root, black, v3(x, y, zc + SPIGOT_S - PIPE_R * 0.8), v3(x, y, zc - 7.8), 0.1, 8);
    box(root, valve, 0.34, 0.5, 0.18, x, y + 0.08, zc - 6.95, { r: 0.03 });
    cyl(root, steel, 0.02, 0.36, x, y + 0.42, zc - 6.95, { seg: 6 });
    cyl(root, valve, 0.13, 0.025, x, y + 0.6, zc - 6.95, { seg: 14 });
    bar(root, black, v3(x, y + 0.02, zc - 7.8), v3(x, y - 0.75, zc - 8.25), 0.1, 8);
    const end = v3(x, y - 0.78, zc - 8.3);
    if (OPEN_SPIGOTS.includes(x)) {
      outlets.push(end.clone());
      // Slurry pours from the open ones onto the beach below.
      const beach = tailings(x, zc - 10.2);
      const fall = new THREE.QuadraticBezierCurve3(end, v3(x, end.y - 0.1, zc - 9.2), v3(x, beach + 0.05, zc - 10.4));
      const g = new THREE.TubeGeometry(fall, 10, 0.12, 6, false);
      streams.add(new THREE.Mesh(g, slurry));
    }
  }
  const baked = bake(root, { mudTop: -1, dust: 1.2 });
  baked.add(streams);
  return { group: baked, outlets };
}

/* ---------- Survey prisms ---------- */
export function makePrisms() {
  const root = new THREE.Group();
  const pole = mats.galv();
  const head = paint("#e8620f", { coat: 0.4 });
  const glass = new THREE.MeshStandardMaterial({ color: "#c9d2d6", metalness: 0.9, roughness: 0.08 });
  const at: Record<string, THREE.Vector3> = {};
  for (const p of PRISMS) {
    const y = crestY(p.x, p.s);
    const z = crestZ(p.x) + p.s;
    cyl(root, pole, 0.035, 1.5, p.x, y + 0.75, z, { seg: 6 });
    cyl(root, mats.concrete(), 0.16, 0.25, p.x, y + 0.1, z, { seg: 10 });
    // The prism faces downstream, towards the robotic total station on the hill.
    cyl(root, head, 0.085, 0.09, p.x, y + 1.55, z + 0.02, { axis: "z", seg: 14 });
    cyl(root, glass, 0.06, 0.02, p.x, y + 1.55, z + 0.075, { axis: "z", seg: 14 });
    at[p.id] = v3(p.x, y + 1.6, z);
  }
  return { group: bake(root, { mudTop: 0.2 }), at };
}

/* ---------- Piezometer heads at section A–A ---------- */
export function makeHeads() {
  const root = new THREE.Group();
  const casing = paint("#b23a1e", { coat: 0.3 });
  const bollard = paint("#e6b417", { coat: 0.4 });
  const logger = paint("#c9ccc8", { coat: 0.3, rough: 0.4 });
  const pole = mats.galv();
  const solar = new THREE.MeshStandardMaterial({ color: "#1d2a3a", metalness: 0.3, roughness: 0.18 });
  const at: Record<string, THREE.Vector3> = {};
  const x0 = SECTION_X;
  for (const [hole, s] of Object.entries(HOLES)) {
    const x = x0;
    const z = crestZ(x) + s;
    const y = s < -DAM.crestHalf - 1 ? tailings(x, z) : crestY(x, s);
    cyl(root, casing, 0.11, 0.85, x, y + 0.42, z, { seg: 10 });
    cyl(root, casing, 0.14, 0.06, x, y + 0.86, z, { seg: 10 });
    for (let k = 0; k < 3; k++) {
      const a = (k / 3) * Math.PI * 2 + 0.4;
      cyl(root, bollard, 0.07, 1.0, x + Math.cos(a) * 0.75, y + 0.5, z + Math.sin(a) * 0.75, { seg: 8 });
    }
    // The logger on its post, with a small solar panel above it.
    const lx = x + 1.6;
    cyl(root, pole, 0.04, 2.2, lx, y + 1.1, z, { seg: 6 });
    box(root, logger, 0.42, 0.52, 0.2, lx, y + 1.25, z + 0.12, { r: 0.02 });
    const panel = box(root, solar, 0.62, 0.03, 0.48, lx, y + 2.25, z + 0.05, { r: 0.01 });
    panel.rotation.x = -0.5;
    at[hole] = v3(x, y, z);
  }
  // Inclinometer casings on the berms: a lockable steel cover over the grooved casing, in a ring of bollards.
  const cover = paint("#5b6b78", { coat: 0.3, rough: 0.5 });
  for (const inc of INCLINOMETERS) {
    const z = crestZ(inc.x) + inc.s + 1.6;
    const y = crestY(inc.x, inc.s + 1.6);
    cyl(root, cover, 0.16, 0.9, inc.x, y + 0.45, z, { seg: 10 });
    cyl(root, cover, 0.19, 0.08, inc.x, y + 0.92, z, { seg: 10 });
    for (let k = 0; k < 3; k++) {
      const a = (k / 3) * Math.PI * 2 + 1.1;
      cyl(root, bollard, 0.07, 1.0, inc.x + Math.cos(a) * 0.8, y + 0.5, z + Math.sin(a) * 0.8, { seg: 8 });
    }
    at[inc.id] = v3(inc.x, y, z);
  }
  return { group: bake(root, { mudTop: 0.3 }), at };
}

/* ---------- The decant tower, its bridge and the causeway ---------- */
export const TOWER = v3(-118, 0, -656);
const CAUSEWAY: [number, number][] = [
  [72, -612],
  [20, -626],
  [-40, -640],
  [-93, -651],
];
const CAUSEWAY_Y = POND + 2.25;

export function makeDecant(o: { material: THREE.Material }) {
  const root = new THREE.Group();
  const concrete = grime(new THREE.MeshStandardMaterial({ color: "#a9a69e", roughness: 0.88 }), { scale: 0.2, mudAmount: 0.6, dust: "#bdb5a5" });
  const steel = paint("#2f4d63", { coat: 0.2, rough: 0.5 });
  const railM = paint("#e2b21b", { coat: 0.4 });
  const grate = mats.grate();
  const clad = paint("#d9dcd6", { coat: 0.3, rough: 0.45 });
  const dark = new THREE.MeshStandardMaterial({ color: "#151719", roughness: 0.7 });
  const bedY = POND - 4.2;
  const topY = CREST + 0.6;
  const tx = TOWER.x;
  const tz = TOWER.z;
  // The tower: a concrete shaft with stop-log slots on the pond side.
  cyl(root, concrete, 2.4, topY - bedY, tx, (topY + bedY) / 2, tz, { seg: 28, r2: 2.3 });
  for (let k = 0; k < 4; k++) {
    const a = (k / 4) * Math.PI * 2 + 0.3;
    const sx = tx + Math.cos(a) * 2.36;
    const sz = tz + Math.sin(a) * 2.36;
    const slot = box(root, dark, 0.7, 4.6, 0.12, sx, topY - 3.2, sz, { r: 0.01 });
    slot.rotation.y = -a + Math.PI / 2;
  }
  // The deck, its rail and the pump and valve house.
  box(root, grate, 6.2, 0.12, 6.2, tx, topY + 0.06, tz, { r: 0.01 });
  rail(
    root,
    railM,
    [v3(tx - 3.1, topY + 0.12, tz - 3.1), v3(tx + 3.1, topY + 0.12, tz - 3.1), v3(tx + 3.1, topY + 0.12, tz + 3.1), v3(tx - 3.1, topY + 0.12, tz + 3.1)],
    { closed: true, posts: 1.6 },
  );
  box(root, clad, 2.6, 2.3, 2.2, tx - 1.2, topY + 1.27, tz - 1.1, { r: 0.04 });
  box(root, steel, 2.8, 0.12, 2.4, tx - 1.2, topY + 2.46, tz - 1.1, { r: 0.02 });
  cyl(root, mats.galv(), 0.05, 4.2, tx + 2.6, topY + 2.2, tz + 2.6, { seg: 6 });
  box(root, mats.white(), 0.5, 0.18, 0.3, tx + 2.6, topY + 4.3, tz + 2.6, { r: 0.02 });
  // The bridge from the end of the causeway.
  const end = CAUSEWAY[CAUSEWAY.length - 1]!;
  const a = v3(end[0], CAUSEWAY_Y + 0.4, end[1]);
  const b = v3(tx + 2.2, topY + 0.06, tz + 0.4);
  const dir = b.clone().sub(a).setY(0).normalize();
  const side = v3(-dir.z, 0, dir.x);
  for (const k of [-1, 1]) {
    const o2 = side.clone().multiplyScalar(k * 0.8);
    bar(root, steel, a.clone().add(o2), b.clone().add(o2), 0.12, 6);
    bar(root, railM, a.clone().add(o2).setY(a.y + 1.1), b.clone().add(o2).setY(b.y + 1.1), 0.03, 6);
    const n = Math.ceil(a.distanceTo(b) / 1.8);
    for (let i = 0; i <= n; i++) {
      const p = a.clone().lerp(b, i / n).add(o2);
      bar(root, railM, p, p.clone().setY(p.y + 1.1), 0.025, 6);
    }
  }
  const span = a.distanceTo(b);
  const deck = box(root, grate, 1.7, 0.08, span, (a.x + b.x) / 2, (a.y + b.y) / 2 + 0.1, (a.z + b.z) / 2, { r: 0.01 });
  deck.lookAt(b.clone().setY(deck.position.y + (b.y - a.y) / 2));
  // Piles under the bridge.
  for (const t of [0.35, 0.7]) {
    const p = a.clone().lerp(b, t);
    cyl(root, steel, 0.25, p.y - bedY, p.x, (p.y + bedY) / 2, p.z, { seg: 10 });
  }
  const tower = bake(root, { mudTop: POND + 0.4, mudBottom: POND - 1, mud: 1.4 });

  // The causeway: rockfill with a gravel running surface, 7 m wide, sides at 1.5 to 1.
  const pts = CAUSEWAY.map(([x, z]) => new THREE.Vector2(x, z));
  const prof: [number, number][] = [
    [-12.3, bedY],
    [-4.3, CAUSEWAY_Y - 0.1],
    [-3.5, CAUSEWAY_Y],
    [3.5, CAUSEWAY_Y],
    [4.3, CAUSEWAY_Y - 0.1],
    [12.3, bedY],
  ];
  const curve = new THREE.SplineCurve(pts);
  const N = 60;
  const posA: number[] = [];
  const colA: number[] = [];
  const uvA: number[] = [];
  const index: number[] = [];
  const c = new THREE.Color();
  for (let i = 0; i <= N; i++) {
    const t = i / N;
    const p = curve.getPoint(t);
    const tg = curve.getTangent(t);
    const nx = -tg.y;
    const nz = tg.x;
    for (const [u, y] of prof) {
      posA.push(p.x + nx * u, y + (Math.abs(u) > 4 ? 0.25 * Math.sin(i * 1.7 + u) : 0), p.y + nz * u);
      c.set(Math.abs(u) <= 3.5 ? "#b2a690" : "#867f73");
      colA.push(c.r, c.g, c.b);
      uvA.push(u * 0.2, t * 30);
    }
  }
  const k = prof.length;
  for (let i = 0; i < N; i++)
    for (let j = 0; j < k - 1; j++) {
      const q = i * k + j;
      index.push(q, q + k, q + 1, q + 1, q + k, q + k + 1);
    }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(posA, 3));
  g.setAttribute("color", new THREE.Float32BufferAttribute(colA, 3));
  g.setAttribute("uv", new THREE.Float32BufferAttribute(uvA, 2));
  g.setIndex(index);
  g.computeVertexNormals();
  const causeway = new THREE.Mesh(g, o.material);
  causeway.castShadow = true;
  causeway.receiveShadow = true;
  const group = new THREE.Group();
  group.add(tower, causeway);
  return { group, top: v3(tx, topY + 2.6, tz), causewayEnd: v3(end[0], CAUSEWAY_Y, end[1]) };
}

/* ---------- Pipelines ---------- */
/** A pipe laid beside a road, `off` metres to its side, on short sleepers. */
function besideRoad(r: Road, off: number, from: number, to: number, heightAt: (x: number, z: number) => number) {
  const pts: THREE.Vector3[] = [];
  const list = r.pts.slice(from, to < 0 ? r.pts.length : to);
  for (let i = 0; i < list.length; i += 3) {
    const p = list[i]!;
    const q = list[Math.min(list.length - 1, i + 1)]!;
    const o2 = list[Math.max(0, i - 1)]!;
    const d = q.clone().sub(o2).setY(0).normalize();
    const x = p.x - d.z * off;
    const z = p.z + d.x * off;
    pts.push(v3(x, heightAt(x, z) + 0.4, z));
  }
  return pts;
}
export function makePipelines(roads: Road[], heightAt: (x: number, z: number) => number) {
  const group = new THREE.Group();
  const black = hdpe();
  // Water back from the decant: along the causeway's edge, round the side road to the crest, then out to the plant.
  const cw: THREE.Vector3[] = [];
  for (let i = CAUSEWAY.length - 1; i >= 0; i--) {
    const [x, z] = CAUSEWAY[i]!;
    const [nx, nz] = CAUSEWAY[Math.max(0, i - 1)]!;
    const [px, pz] = CAUSEWAY[Math.min(CAUSEWAY.length - 1, i + 1)]!;
    const dx = px - nx;
    const dz = pz - nz;
    const l = Math.hypot(dx, dz) || 1;
    cw.push(v3(x - (dz / l) * 2.7, CAUSEWAY_Y + 0.3, z + (dx / l) * 2.7));
  }
  const side = besideRoad(roads[2]!, -6.5, 0, -1, heightAt).reverse();
  group.add(pipe([...cw, ...side], 0.22, black));
  group.add(pipe(besideRoad(roads[0]!, 6.5, 6, -1, heightAt), 0.22, black));
  // Tailings in from the plant: beside the access road, then up onto the crest to feed the spigot line.
  const feed = besideRoad(roads[0]!, 8.2, 10, -1, heightAt).reverse();
  const join = [
    v3(330, mix(crestY(330, SPIGOT_S), natural(330, crestZ(330) + SPIGOT_S), 0.5) + 0.44, crestZ(330) + SPIGOT_S - 1),
    v3(306, crestY(306, SPIGOT_S) + 0.44, crestZ(306) + SPIGOT_S),
  ];
  group.add(pipe([...feed, ...join], PIPE_R, black));
  return group;
}

/* ---------- The seepage collection pond ---------- */
export function makeSeepage() {
  const group = new THREE.Group();
  // The liner follows the dug pond and laps over the rim into its anchor trench.
  const liner = new THREE.MeshStandardMaterial({ color: "#1b1c1c", roughness: 0.38, metalness: 0 });
  const hx = SEEP.hx + 2;
  const hz = SEEP.hz + 2;
  const nx = 64;
  const nz = 38;
  const geo = new THREE.PlaneGeometry(hx * 2, hz * 2, nx, nz);
  geo.rotateX(-Math.PI / 2);
  const pos = geo.getAttribute("position") as THREE.BufferAttribute;
  let rimLow = Infinity;
  for (let i = 0; i < pos.count; i++) {
    const x = pos.getX(i) + SEEP.x;
    const z = pos.getZ(i) + SEEP.z;
    const edge = Math.min(SEEP.hx - Math.abs(x - SEEP.x), SEEP.hz - Math.abs(z - SEEP.z));
    const y = natural(x, z) - seepDepth(x, z) + 0.06 + (edge < 0 ? 0.12 * Math.max(0, 1 + edge / 2) : 0) + 0.04 * Math.sin(x * 1.7) * Math.sin(z * 2.3);
    if (edge >= 0 && edge < 0.3) rimLow = Math.min(rimLow, y);
    pos.setXYZ(i, x, y, z);
  }
  geo.computeVertexNormals();
  const linerMesh = new THREE.Mesh(geo, liner);
  linerMesh.receiveShadow = true;
  group.add(linerMesh);
  const level = rimLow - 1.15;
  const water = new THREE.Mesh(new THREE.PlaneGeometry(SEEP.hx * 2 - 4, SEEP.hz * 2 - 4), stillWater());
  water.rotation.x = -Math.PI / 2;
  water.position.set(SEEP.x, level, SEEP.z);
  water.receiveShadow = true;
  group.add(water);
  // The V-notch weir box where the toe drain comes in, and the pump that sends the water back up.
  const root = new THREE.Group();
  const conc = mats.concrete();
  const wx = SEEP.x - 14;
  const wz = SEEP.z - SEEP.hz - 2.4;
  const wy = natural(wx, wz);
  box(root, conc, 3.2, 1.0, 1.6, wx, wy + 0.3, wz, { r: 0.03 });
  box(root, mats.galv(), 0.04, 0.6, 1.4, wx + 1.3, wy + 0.6, wz, { r: 0.01 });
  const px = SEEP.x + SEEP.hx + 3;
  const pz = SEEP.z + 4;
  const py = natural(px, pz);
  box(root, mats.dark(), 3.2, 0.2, 1.6, px, py + 0.1, pz, { r: 0.02 });
  cyl(root, paint("#2f5d8a", { coat: 0.4 }), 0.35, 1.2, px - 0.5, py + 0.55, pz, { axis: "x", seg: 16 });
  box(root, paint("#2f5d8a", { coat: 0.4 }), 0.8, 0.8, 0.7, px + 0.8, py + 0.6, pz, { r: 0.05 });
  bar(root, hdpe(), v3(px - 1.1, py + 0.5, pz), v3(SEEP.x + SEEP.hx - 4, level + 0.2, pz), 0.12, 8);
  group.add(bake(root, { mudTop: 0.4 }));
  return { group, level, weir: v3(wx, wy + 1, wz) };
}
