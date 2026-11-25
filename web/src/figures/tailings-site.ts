/**
 * The site around the facility: saltbush scattered over the desert (thicker in the washes), the 22 kV line on its
 * poles that feeds the decant pumps, and the pad at the toe with the tailings control room, a generator and the
 * fence round the seepage pond. They give the scene its scale. Pure factories; nothing touches the DOM.
 */
import * as THREE from "three";
import { mergeVertices } from "three/examples/jsm/utils/BufferGeometryUtils.js";
import { bake, box, cyl, grime, mats, paint, v3 } from "./fleet-kit";
import { clamp, rng } from "./lib";
import { noise2 } from "./stage";
import type { Road } from "./tailings-ground";
import { crestZ, damAt, SEEP, tailings, upstream } from "./tailings-model";

/** Saltbush and spinifex: squat grey-green clumps, as one instanced mesh. */
export function makeShrubs(o: { small: boolean; heightAt: (x: number, z: number) => number; roads: Road[] }) {
  const count = o.small ? 2200 : 7000;
  const ico = new THREE.IcosahedronGeometry(1, 2);
  ico.deleteAttribute("normal");
  ico.deleteAttribute("uv");
  const geo = mergeVertices(ico);
  ico.dispose();
  // Lumpy, flat-bottomed clumps.
  const p = geo.getAttribute("position") as THREE.BufferAttribute;
  const r0 = rng(5);
  for (let i = 0; i < p.count; i++) {
    const k = 0.78 + 0.4 * r0();
    // Twiggy and uneven: a few lobes pushed out, the rest pulled in.
    const lobe = 0.75 + 0.35 * Math.sin(p.getX(i) * 4.1 + p.getZ(i) * 3.3) * Math.cos(p.getY(i) * 3.7);
    p.setXYZ(i, p.getX(i) * k * lobe, Math.max(-0.1, p.getY(i)) * k * 0.7, p.getZ(i) * k * lobe);
  }
  geo.computeVertexNormals();
  const mat = new THREE.MeshStandardMaterial({ color: "#ffffff", roughness: 0.92, metalness: 0 });
  const mesh = new THREE.InstancedMesh(geo, mat, count);
  mesh.name = "shrubs";
  const wash = noise2(141);
  const r = rng(29);
  const m = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const sc = new THREE.Vector3();
  const pos = new THREE.Vector3();
  const c = new THREE.Color();
  // Saltbush is a silvery grey-green; bluebush bluer; dead clumps straw.
  const greens = [new THREE.Color("#8a8f74"), new THREE.Color("#7c8468"), new THREE.Color("#959a80"), new THREE.Color("#7f8a7c"), new THREE.Color("#a39a78")];
  // Road centrelines, sampled coarsely, to keep the clumps off them.
  const roadPts = o.roads.flatMap((rd) => rd.pts.filter((_, i) => i % 3 === 0).map((pt) => [pt.x, pt.z, rd.width / 2 + 4] as const));
  let n = 0;
  for (let tries = 0; n < count && tries < count * 8; tries++) {
    // Mostly near the facility, thinning out with distance.
    const a = r() * Math.PI * 2;
    const d = 60 + 1300 * r() ** 1.6;
    const x = 40 + Math.cos(a) * d * 1.2;
    const z = -200 + Math.sin(a) * d;
    // Off the impoundment, the embankment, the seepage pond and the roads.
    if (upstream(x, z) > -14 && tailings(x, z) > -1e9 && x > -640 && x < 380) continue;
    const g = o.heightAt(x, z);
    if (damAt(x, z) > g - 0.5 && Math.abs(z - crestZ(x)) < 140 && Math.abs(x) < 400) continue;
    if (Math.abs(x - SEEP.x) < SEEP.hx + 30 && Math.abs(z - SEEP.z) < SEEP.hz + 25) continue;
    let onRoad = false;
    for (const [rx, rz, w] of roadPts)
      if (Math.abs(rx - x) < w + 6 && Math.abs(rz - z) < w + 6) {
        onRoad = true;
        break;
      }
    if (onRoad) continue;
    // Thicker along the washes (the noise's zero lines), sparse on the open plain.
    const wv = Math.abs(wash.fbm(x / 180, z / 180, 3));
    if (r() > 0.18 + 0.82 * clamp(1 - wv * 5, 0, 1)) continue;
    const s = 0.5 + r() ** 2 * 1.1;
    pos.set(x, g - 0.05, z);
    q.setFromAxisAngle(new THREE.Vector3(0, 1, 0), r() * 6.28);
    sc.set(s * (0.8 + 0.4 * r()), s, s * (0.8 + 0.4 * r()));
    m.compose(pos, q, sc);
    mesh.setMatrixAt(n, m);
    c.copy(greens[Math.floor(r() * greens.length)]!).multiplyScalar(0.85 + 0.3 * r());
    mesh.setColorAt(n, c);
    n++;
  }
  mesh.count = n;
  mesh.instanceMatrix.needsUpdate = true;
  if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  mesh.castShadow = !o.small;
  mesh.receiveShadow = true;
  mesh.computeBoundingSphere();
  return mesh;
}

/**
 * The 22 kV line to the decant pumps: steel poles with a crossarm and three insulators, every 75 m along the side
 * road and down to the causeway, the conductors sagging between them.
 */
export function makePowerLine(road: Road, heightAt: (x: number, z: number) => number) {
  const root = new THREE.Group();
  const pole = grime(new THREE.MeshStandardMaterial({ color: "#8d8c86", roughness: 0.6, metalness: 0.5 }), { scale: 0.3 });
  const arm = mats.galv();
  const insul = paint("#6b4a3a", { coat: 0.6, rough: 0.3 });
  const tops: THREE.Vector3[][] = [];
  const pts = road.pts;
  // Poles stand 9 m off the road's centreline, on the uphill side.
  let along = 0;
  let next = 20;
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1]!;
    const b = pts[i]!;
    along += a.distanceTo(b);
    if (along < next) continue;
    next += 75;
    const dir = b.clone().sub(a).setY(0).normalize();
    const side = new THREE.Vector3(-dir.z, 0, dir.x);
    const x = b.x + side.x * (road.width / 2 + 7);
    const z = b.z + side.z * (road.width / 2 + 7);
    const y = heightAt(x, z);
    const H = 11;
    cyl(root, pole, 0.16, H, x, y + H / 2, z, { seg: 8, r2: 0.11 });
    const yaw = Math.atan2(side.x, side.z);
    const armM = box(root, arm, 2.6, 0.12, 0.12, x, y + H - 0.5, z, { r: 0.01 });
    armM.rotation.y = yaw;
    const set: THREE.Vector3[] = [];
    for (const k of [-1.15, 0, 1.15]) {
      const ix = x + side.x * k;
      const iz = z + side.z * k;
      const top = k === 0 ? H + 0.35 : H - 0.2;
      cyl(root, insul, 0.07, 0.35, ix, y + top, iz, { seg: 8 });
      set.push(v3(ix, y + top + 0.2, iz));
    }
    tops.push(set);
  }
  const group = bake(root, { mudTop: 0.5 });
  // Conductors: a catenary-like sag of 1.6 m mid-span.
  const wire: number[] = [];
  for (let i = 1; i < tops.length; i++) {
    for (let k = 0; k < 3; k++) {
      const a = tops[i - 1]![k]!;
      const b = tops[i]![k]!;
      const N = 10;
      for (let j = 0; j < N; j++) {
        const t0 = j / N;
        const t1 = (j + 1) / N;
        const sag = (t: number) => 1.6 * 4 * t * (1 - t);
        wire.push(a.x + (b.x - a.x) * t0, a.y + (b.y - a.y) * t0 - sag(t0), a.z + (b.z - a.z) * t0);
        wire.push(a.x + (b.x - a.x) * t1, a.y + (b.y - a.y) * t1 - sag(t1), a.z + (b.z - a.z) * t1);
      }
    }
  }
  const wg = new THREE.BufferGeometry();
  wg.setAttribute("position", new THREE.Float32BufferAttribute(wire, 3));
  const wires = new THREE.LineSegments(wg, new THREE.LineBasicMaterial({ color: "#2a2826", transparent: true, opacity: 0.75 }));
  wires.name = "conductors";
  group.add(wires);
  return group;
}

/** The pad at the toe: levelled fill, the control room, a generator, and the fence round the seepage pond. */
export function makePad(heightAt: (x: number, z: number) => number) {
  const root = new THREE.Group();
  const x0 = 34;
  const x1 = 78;
  const z0 = 150;
  const z1 = 178;
  let hi = -Infinity;
  let lo = Infinity;
  for (let x = x0; x <= x1; x += 4)
    for (let z = z0; z <= z1; z += 4) {
      const g = heightAt(x, z);
      hi = Math.max(hi, g);
      lo = Math.min(lo, g);
    }
  const top = hi + 0.4;
  const fill = grime(new THREE.MeshStandardMaterial({ color: "#a89a82", roughness: 0.95 }), { scale: 0.15, dust: "#c2b091" });
  // The pad: a flat top with battered sides down to the ground.
  const padH = top - lo + 1;
  const pad = new THREE.Mesh(new THREE.BoxGeometry(x1 - x0, padH, z1 - z0), fill);
  pad.position.set((x0 + x1) / 2, top - padH / 2, (z0 + z1) / 2);
  pad.receiveShadow = true;
  root.add(pad);
  // Control room: a white 12 m site office with windows and an air conditioner, on sleepers.
  const white = paint("#e8e6df", { coat: 0.4, rough: 0.45 });
  const dark = mats.dark();
  const glass = mats.glass();
  const cx = 48;
  const cz = 160;
  box(root, white, 12, 2.9, 3.2, cx, top + 0.25 + 1.45, cz, { r: 0.04 });
  for (let k = -2; k <= 2; k++) box(root, glass, 1.1, 0.8, 0.04, cx + k * 2.2, top + 2.1, cz + 1.62, { r: 0.01 });
  box(root, dark, 0.9, 2.0, 0.05, cx + 5.2, top + 1.25, cz + 1.62, { r: 0.01 });
  box(root, paint("#c9c8c2", { coat: 0.3 }), 0.9, 0.6, 0.7, cx - 3, top + 3.5, cz, { r: 0.04 });
  for (const k of [-5.5, 0, 5.5]) box(root, dark, 0.25, 0.25, 3.4, cx + k, top + 0.12, cz, { r: 0.01 });
  // Steps up to the door.
  box(root, mats.galv(), 1.2, 0.2, 0.9, cx + 5.2, top + 0.35, cz + 2.1, { r: 0.01 });
  // Generator set: a yellow canopy on a skid, its exhaust stack.
  box(root, paint("#e0b120", { coat: 0.4 }), 4.2, 2.1, 1.6, 66, top + 1.05 + 0.2, 158, { r: 0.05 });
  box(root, dark, 4.6, 0.2, 1.8, 66, top + 0.1, 158, { r: 0.01 });
  cyl(root, mats.steel(), 0.1, 1.2, 67.4, top + 2.9, 158.3, { seg: 8 });
  // A water tank on a stand.
  cyl(root, paint("#2f6b4f", { coat: 0.2 }), 1.4, 2.6, 72, top + 1.5, 170, { seg: 18 });
  const group = bake(root, { mudTop: 0.4 });
  // The fence round the seepage pond: posts every 3 m, three strands and the mesh as a faint skin.
  const fx0 = SEEP.x - SEEP.hx - 4;
  const fx1 = SEEP.x + SEEP.hx + 4;
  const fz0 = SEEP.z - SEEP.hz - 4;
  const fz1 = SEEP.z + SEEP.hz + 4;
  const ring: [number, number][] = [
    [fx0, fz0],
    [fx1, fz0],
    [fx1, fz1],
    [fx0, fz1],
    [fx0, fz0],
  ];
  const posts = new THREE.Group();
  const postM = mats.galv();
  const wire: number[] = [];
  const skin: number[] = [];
  for (let i = 1; i < ring.length; i++) {
    const [ax, az] = ring[i - 1]!;
    const [bx, bz] = ring[i]!;
    const len = Math.hypot(bx - ax, bz - az);
    const n = Math.ceil(len / 3);
    let prev: [number, number, number] | null = null;
    for (let k = 0; k <= n; k++) {
      const t = k / n;
      const x = ax + (bx - ax) * t;
      const z = az + (bz - az) * t;
      const y = heightAt(x, z);
      if (k < n) cyl(posts, postM, 0.035, 2.1, x, y + 1.05, z, { seg: 6 });
      if (prev) {
        for (const h of [0.15, 1.05, 2.0]) wire.push(prev[0], prev[1] + h, prev[2], x, y + h, z);
        skin.push(prev[0], prev[1] + 0.1, prev[2], x, y + 0.1, z, prev[0], prev[1] + 2.0, prev[2], x, y + 0.1, z, x, y + 2.0, z, prev[0], prev[1] + 2.0, prev[2]);
      }
      prev = [x, y, z];
    }
  }
  group.add(bake(posts, { mudTop: 0.3 }));
  const wg = new THREE.BufferGeometry();
  wg.setAttribute("position", new THREE.Float32BufferAttribute(wire, 3));
  group.add(new THREE.LineSegments(wg, new THREE.LineBasicMaterial({ color: "#8f918e", transparent: true, opacity: 0.8 })));
  const sg = new THREE.BufferGeometry();
  sg.setAttribute("position", new THREE.Float32BufferAttribute(skin, 3));
  sg.computeVertexNormals();
  group.add(new THREE.Mesh(sg, new THREE.MeshBasicMaterial({ color: "#9a9c98", transparent: true, opacity: 0.16, side: THREE.DoubleSide, depthWrite: false })));
  return { group, top, light: v3(40, top, 172), park: v3(58, top, 170) };
}
