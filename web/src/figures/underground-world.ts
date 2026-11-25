/**
 * Puts the underground figure's world together from the plan: the excavation network, the orebody and its stopes,
 * the fired stope's scan and muck, the lamps along the drives, the ore pass grizzlies, the refuge chamber, vent duct
 * and the surface. Returns handles the page animates and the layers switch.
 */
import * as THREE from "three";
import {
  cavityMaterial,
  lensGeometry,
  lensMaterial,
  pasteMaterial,
  ringLines,
  scanDisp,
  stopeEdges,
  stopeMuck,
  stopeShape,
  stopeSolid,
  tonnes,
} from "./underground-ore";
import {
  CROSSCUTS,
  DECLINE,
  DECLINE_END,
  declineAt,
  driveEast,
  DRIVE_WEST,
  driveZ,
  FEATURE,
  HEADINGS,
  LEVELS,
  OREPASS,
  orepassZ,
  PASTE_HOLE,
  RAR,
  REFUGE,
  SHAFT,
  STOPES,
  stopeMidX,
  SUBSTATION,
  zFW,
  zHW,
  type Stope,
} from "./underground-plan";
import { reduced } from "./lib";
import { bakeLamps, CAPS, cutawayBlock, network, rockMaterial, rockUniforms, shellMaterial, type Network, type RaiseSpec, type RockUniforms, type Solid, type TunnelSpec } from "./underground-rock";
import { airflow, lightShafts, services, tickShafts } from "./underground-services";
import { makeSurface, mergeByMaterial, type Surface } from "./underground-surface";

export { airSpeed, declineAirSpeed, lightShaft } from "./underground-services";

const V = (x: number, y: number, z: number) => new THREE.Vector3(x, y, z);
const STATUS = { verified: 1, due: 2, overdue: 3 } as const;

/** Every excavation as a centreline, from the plan. */
export function tunnelSpecs(): TunnelSpec[] {
  const out: TunnelSpec[] = [];
  const headingZone = (match: (h: (typeof HEADINGS)[number]) => boolean, length: number) => {
    const i = HEADINGS.findIndex(match);
    return i < 0 ? undefined : { status: STATUS[HEADINGS[i]!.status], length, heading: i };
  };
  // The decline, from the portal to its face below 175.
  const dp: THREE.Vector3[] = [];
  for (let s = 0; s <= DECLINE_END; s += 2) {
    const d = declineAt(s);
    dp.push(V(d.x, d.y, d.z));
  }
  out.push({ id: "decline", kind: "decline", pts: dp, w: DECLINE.w, h: DECLINE.h, end: "face", shotcrete: true, zone: headingZone((h) => h.at === "decline", 20) });
  const inOre = (p: THREE.Vector3) => p.z < zFW(p.x, p.y + 2) && p.z > zHW(p.x, p.y + 2);
  for (const L of LEVELS) {
    const dz = driveZ(L);
    const ax = DECLINE.cx;
    out.push({ id: `access${L}`, kind: "access", pts: [V(ax, L, DECLINE.cz - DECLINE.r), V(ax, L, dz)], w: 5.4, h: 5.6, shotcrete: true });
    const east = driveEast(L);
    const drive = [V(DRIVE_WEST, L, dz), V(east, L, dz)];
    if (L !== -175) drive.push(V(RAR.x, L, RAR.z));
    out.push({
      id: `drive${L}`,
      kind: "drive",
      pts: drive,
      w: 5.4,
      h: 5.6,
      end: L === -175 ? "face" : "open",
      zone: L === -175 ? headingZone((h) => h.at === "l175", 22) : undefined,
    });
    out.push({ id: `station${L}`, kind: "stub", pts: [V(ax, L, SHAFT.z), V(SHAFT.x + 1.2, L, SHAFT.z)], w: 5.5, h: 5.5, end: "open", bare: true });
    if (L >= -150 && L <= -75) out.push({ id: `tip${L}`, kind: "stub", pts: [V(OREPASS.x, L, dz), V(OREPASS.x, L, orepassZ(L) + 0.6)], w: 5.5, h: 5.5, end: "open" });
    for (const [k, end] of CROSSCUTS[L] ?? []) {
      const x = stopeMidX(k);
      const zEnd = end === "face" ? zHW(x, L + 2) + 1.6 : end === "brow" ? zFW(x, L + 2) - 1.2 : zFW(x, L + 2) + 3.2;
      out.push({
        id: `xc${L}-${k}`,
        kind: "xc",
        pts: [V(x, L, dz), V(x, L, zEnd)],
        w: 5.5,
        h: 5.5,
        end: end === "brow" ? "open" : end,
        ore: inOre,
        zone: headingZone((h) => typeof h.at === "object" && h.at.level === L && h.at.k === k, 16),
      });
    }
  }
  const L125 = driveZ(REFUGE.level);
  out.push({ id: "refuge", kind: "stub", pts: [V(REFUGE.x, REFUGE.level, L125), V(REFUGE.x, REFUGE.level, L125 + 12)], w: 5.6, h: 5.5, end: "face", bare: true });
  out.push({ id: "substation", kind: "stub", pts: [V(SUBSTATION.x, SUBSTATION.level, L125), V(SUBSTATION.x, SUBSTATION.level, L125 + 10)], w: 5.5, h: 5.5, end: "face", bare: true });
  return out;
}

export function raiseSpecs(): RaiseSpec[] {
  return [
    { id: "shaft", top: V(SHAFT.x, 0, SHAFT.z), bottom: V(SHAFT.x, SHAFT.bottom, SHAFT.z), r: SHAFT.r, lined: true },
    { id: "rar", top: V(RAR.x, 0, RAR.z), bottom: V(RAR.x, RAR.bottom, RAR.z), r: RAR.r },
    { id: "orepass", top: V(OREPASS.x, OREPASS.top - 0.3, orepassZ(OREPASS.top)), bottom: V(OREPASS.x, OREPASS.bottom, orepassZ(OREPASS.bottom)), r: OREPASS.r },
  ];
}

/** Drops the triangles of a mesh whose centres fall inside any of these solids (where tunnels open into a stope). */
function trim(g: THREE.BufferGeometry, solids: Solid[]) {
  const p = g.getAttribute("position");
  const idx = g.getIndex()!;
  const keep: number[] = [];
  const c = new THREE.Vector3();
  for (let i = 0; i < idx.count; i += 3) {
    const a = idx.getX(i);
    const b = idx.getX(i + 1);
    const d = idx.getX(i + 2);
    c.set((p.getX(a) + p.getX(b) + p.getX(d)) / 3, (p.getY(a) + p.getY(b) + p.getY(d)) / 3, (p.getZ(a) + p.getZ(b) + p.getZ(d)) / 3);
    if (solids.some((s) => s.box.containsPoint(c) && s.inside(c))) continue;
    keep.push(a, b, d);
  }
  g.setIndex(keep);
}

/* ---------- Glows ---------- */

let glowTex: THREE.Texture | null = null;
/** A soft round glow, for lamp halos and light pools. */
export function glowTexture() {
  if (glowTex) return glowTex;
  const c = document.createElement("canvas");
  c.width = c.height = 128;
  const x = c.getContext("2d")!;
  const g = x.createRadialGradient(64, 64, 0, 64, 64, 64);
  g.addColorStop(0, "rgba(255,255,255,1)");
  g.addColorStop(0.14, "rgba(255,255,255,0.6)");
  g.addColorStop(0.42, "rgba(255,255,255,0.14)");
  g.addColorStop(1, "rgba(255,255,255,0)");
  x.fillStyle = g;
  x.fillRect(0, 0, 128, 128);
  glowTex = new THREE.CanvasTexture(c);
  return glowTex;
}
export function halo(colour: string, size: number, opacity = 1) {
  const s = new THREE.Sprite(new THREE.SpriteMaterial({ map: glowTexture(), color: colour, blending: THREE.AdditiveBlending, depthWrite: false, transparent: true, opacity }));
  s.scale.setScalar(size);
  return s;
}

/**
 * Lamp fittings on the walls (their light is baked into the rock by `bakeLamps`). The near wall of every tunnel drops
 * away when seen from outside, so `update` hides a fitting whose wall faces away from the camera (it would hang in
 * the air).
 */
function lampFittings(net: Network) {
  const n = net.lamps.length;
  const fit = new THREE.InstancedMesh(
    new THREE.BoxGeometry(0.12, 0.12, 1.2),
    new THREE.MeshStandardMaterial({ color: "#fff4e0", emissive: "#ffd9a8", emissiveIntensity: 4 }),
    n,
  );
  const Yv = new THREE.Vector3(0, 1, 0);
  const one = new THREE.Vector3(1, 1, 1);
  const fits = net.lamps.map((l) => {
    const along = new THREE.Vector3().crossVectors(Yv, l.n).normalize();
    const q = new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(l.n, Yv, along));
    return new THREE.Matrix4().compose(l.p, q, one);
  });
  // Each lamp's light falling through the dust: a wide cone down and across the drive.
  const shafts = lightShafts(n, { length: 4.6, angle: 0.62, colour: "#ffd29a", strength: 0.16 });
  const cones = net.lamps.map((l) => {
    const dir = l.n.clone().multiplyScalar(0.75).add(new THREE.Vector3(0, -1, 0)).normalize();
    const q = new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, 0, 1), dir);
    return new THREE.Matrix4().compose(l.p.clone().addScaledVector(l.n, 0.1), q, one);
  });
  const g = new THREE.Group();
  g.add(fit, shafts);
  const zero = new THREE.Matrix4().makeScale(0, 0, 0);
  const side: number[] = net.lamps.map(() => 0);
  const d = new THREE.Vector3();
  const update = (cam: THREE.Vector3) => {
    let changed = false;
    net.lamps.forEach((l, i) => {
      const far = d.copy(cam).sub(l.p).dot(l.n) > 0 ? 1 : 2;
      if (far === side[i]) return;
      side[i] = far;
      fit.setMatrixAt(i, far === 1 ? fits[i]! : zero);
      shafts.setMatrixAt(i, far === 1 ? cones[i]! : zero);
      changed = true;
    });
    if (changed) fit.instanceMatrix.needsUpdate = shafts.instanceMatrix.needsUpdate = true;
  };
  return { group: g, update };
}

/** A refuge chamber: a steel box on skids with a door, air bottles and a green strobe. */
function refugeChamber() {
  const g = new THREE.Group();
  const white = new THREE.MeshStandardMaterial({ color: "#e4e2da", roughness: 0.5, metalness: 0.2 });
  const green = new THREE.MeshStandardMaterial({ color: "#1f8a4c", roughness: 0.5 });
  const steel = new THREE.MeshStandardMaterial({ color: "#5e6266", roughness: 0.5, metalness: 0.6 });
  const b = (m: THREE.Material, w: number, h: number, d: number, x: number, y: number, z: number) => {
    const mesh = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), m);
    mesh.position.set(x, y, z);
    g.add(mesh);
  };
  b(steel, 2.5, 0.25, 6.6, 0, 0.12, 0);
  b(white, 2.4, 2.35, 6.2, 0, 1.45, 0);
  b(green, 2.42, 0.4, 6.22, 0, 2.2, 0);
  b(steel, 0.08, 1.8, 0.9, 1.22, 1.3, -2.2);
  const bottle = new THREE.MeshStandardMaterial({ color: "#2f6fb7", roughness: 0.4 });
  for (let i = 0; i < 6; i++) {
    const c = new THREE.Mesh(new THREE.CylinderGeometry(0.12, 0.12, 1.5, 10), bottle);
    c.position.set(-1.36, 0.95, -1.5 + i * 0.32);
    g.add(c);
  }
  const merged = mergeByMaterial(g);
  const strobe = new THREE.Mesh(new THREE.SphereGeometry(0.13, 10, 8), new THREE.MeshStandardMaterial({ color: "#43ff8a", emissive: "#2bff7a", emissiveIntensity: 5 }));
  strobe.position.set(0, 2.72, 2.6);
  merged.add(strobe);
  const glow = halo("#4dff98", 2.6, 0.8);
  glow.position.copy(strobe.position);
  merged.add(glow);
  return { group: merged, strobe: glow };
}

const GRIZ = { steel: new THREE.MeshStandardMaterial({ color: "#555b5e", roughness: 0.55, metalness: 0.7 }), bund: new THREE.MeshStandardMaterial({ color: "#8c877d", roughness: 0.9 }) };
/** A grizzly over the ore pass: steel bars on a frame, at a tip. */
function grizzly(x: number, y: number, z: number) {
  const g = new THREE.Group();
  const steel = GRIZ.steel;
  for (let i = -4; i <= 4; i++) {
    const bar = new THREE.Mesh(new THREE.BoxGeometry(0.16, 0.22, 3.8), steel);
    bar.position.set(x + i * 0.42, y + 0.05, z);
    g.add(bar);
  }
  for (const s of [-1, 1]) {
    const bar = new THREE.Mesh(new THREE.BoxGeometry(4, 0.3, 0.3), steel);
    bar.position.set(x, y + 0.1, z + s * 1.9);
    g.add(bar);
  }
  // A concrete bund across the tip.
  const bund = new THREE.Mesh(new THREE.BoxGeometry(4.8, 0.9, 0.6), GRIZ.bund);
  bund.position.set(x, y + 0.45, z + 2.4);
  g.add(bund);
  return g;
}

export interface StopeView {
  stope: Stope;
  /** The design solid, for highlights and picking. */
  design: THREE.Mesh;
  edges: THREE.LineSegments;
  rings?: THREE.LineSegments;
  paste?: THREE.Mesh;
  tonnes: number;
}

export interface World {
  group: THREE.Group;
  net: Network;
  uniforms: RockUniforms;
  rock: THREE.Mesh;
  shell: THREE.Mesh;
  shellMat: THREE.ShaderMaterial;
  lens: THREE.Mesh;
  lensMat: THREE.ShaderMaterial;
  stopes: Map<string, StopeView>;
  cavity: THREE.Mesh;
  heat: { value: number };
  scan: { design: number; surveyed: number; over: number; under: number; elos: number };
  muck: { group: THREE.Group; height: (x: number, z: number) => number };
  refuge: { group: THREE.Group; strobe: THREE.Sprite };
  surface: Surface;
  block: THREE.Mesh;
  /** Call when the camera moves: hides lamp fittings on walls the camera sees through. */
  lamps(cam: THREE.Vector3): void;
  /** Call once a frame: the camera, and each miner's cap lamp (an object on the hat whose +z is where it shines). */
  tick(dt: number, t: number, cam: THREE.Vector3, capLamps: readonly THREE.Object3D[]): void;
  /** Ground instruments' reading heads (extensometers in the back, convergence targets at the crown), by id. */
  instruments: Map<string, THREE.Vector3>;
  /** Where the tunnel floor is at an excavation's far end, and its heading direction. */
  end(id: string): { p: THREE.Vector3; dir: THREE.Vector3 };
}

/** Builds the world in steps, yielding between them so no single frame stalls. */
export async function buildWorld(small: boolean, step: () => Promise<void>): Promise<World> {
  const group = new THREE.Group();
  const uniforms: RockUniforms = rockUniforms();
  const block = cutawayBlock();
  group.add(block);

  const fired = STOPES.find((s) => s.id === FEATURE.mucking)!;
  const firedSolid = stopeSolid(fired);
  const specs = tunnelSpecs();
  await step();
  const net = network(specs, raiseSpecs(), [firedSolid]);
  const rock = new THREE.Mesh(net.geometry, rockMaterial(uniforms));
  rock.receiveShadow = true;
  const shellMat = shellMaterial(uniforms);
  const shell = new THREE.Mesh(net.geometry, shellMat);
  shell.renderOrder = 3;
  const lamps = lampFittings(net);
  group.add(rock, shell, lamps.group);
  await step();
  bakeLamps(net);
  const serv = services(net, small);
  group.add(serv.group);
  await step();
  const air = airflow(net, small, reduced);
  group.add(air.points);
  await step();

  /* ---------- Ore and stopes ---------- */
  const lensMat = lensMaterial();
  const lens = new THREE.Mesh(lensGeometry(small ? 50 : 70, small ? 40 : 56), lensMat);
  lens.renderOrder = 4;
  group.add(lens);
  const stopes = new Map<string, StopeView>();
  const hiMat = () =>
    new THREE.MeshBasicMaterial({ color: "#ffffff", transparent: true, opacity: 0, depthWrite: false, side: THREE.DoubleSide, blending: THREE.AdditiveBlending, fog: true });
  const pasteCured = pasteMaterial(false);
  const pasteWet = pasteMaterial(true);
  for (const s of STOPES) {
    const shape = stopeShape(s, { inset: 0.15 });
    const design = new THREE.Mesh(shape.geometry, hiMat());
    design.renderOrder = 4;
    design.userData.stope = s.id;
    group.add(design);
    const eg = new THREE.BufferGeometry();
    eg.setAttribute("position", new THREE.Float32BufferAttribute(stopeEdges(s), 3));
    const edges = new THREE.LineSegments(eg, new THREE.LineBasicMaterial({ color: "#f3dca6", transparent: true, opacity: 0.32, depthWrite: false, fog: true }));
    edges.renderOrder = 6;
    group.add(edges);
    const view: StopeView = { stope: s, design, edges, tonnes: tonnes(shape.volume) };
    if (s.state === "drilled") {
      const rg = new THREE.BufferGeometry();
      rg.setAttribute("position", new THREE.Float32BufferAttribute(ringLines(s), 3));
      view.rings = new THREE.LineSegments(rg, new THREE.LineBasicMaterial({ color: "#ffb347", transparent: true, opacity: 0.55, depthWrite: false, fog: true }));
      view.rings.renderOrder = 6;
      group.add(view.rings);
    }
    if (s.state === "filled" || s.state === "curing") {
      const ps = stopeShape(s, { inset: 0.35 });
      view.paste = new THREE.Mesh(ps.geometry, s.state === "curing" ? pasteWet : pasteCured);
      view.paste.receiveShadow = true;
      view.paste.userData.stope = s.id;
      group.add(view.paste);
      edges.visible = false;
    }
    stopes.set(s.id, view);
  }
  await step();

  /* ---------- The fired stope: its scan, the muck and the drawpoint ---------- */
  const heat = { value: 0 };
  const design = stopeShape(fired);
  const scanned = stopeShape(fired, { disp: scanDisp(fired), inward: true, heat: true });
  trim(scanned.geometry, net.solids.filter((s) => s.rank < 6));
  const cavity = new THREE.Mesh(scanned.geometry, cavityMaterial(heat));
  cavity.receiveShadow = true;
  cavity.userData.stope = fired.id;
  group.add(cavity);
  stopes.get(fired.id)!.edges.visible = true;
  const xcX = stopeMidX(fired.k);
  const muck = stopeMuck(fired, xcX, zFW(xcX, fired.level + 2), small ? 1 : 2);
  group.add(muck.group);

  /* ---------- Fittings underground ---------- */
  {
    const gz = new THREE.Group();
    for (const L of [-75, -100, -125, -150] as const) gz.add(grizzly(OREPASS.x, L, orepassZ(L) + 0.4));
    group.add(mergeByMaterial(gz));
  }
  const refuge = refugeChamber();
  const rz = driveZ(REFUGE.level) + 7.4;
  refuge.group.position.set(REFUGE.x, REFUGE.level, rz);
  group.add(refuge.group);
  // A substation in its cuddy.
  {
    const sub = new THREE.Group();
    const grey = new THREE.MeshStandardMaterial({ color: "#7a8086", roughness: 0.5, metalness: 0.5 });
    const orange = new THREE.MeshStandardMaterial({ color: "#d1661c", roughness: 0.6 });
    const t = new THREE.Mesh(new THREE.BoxGeometry(2.4, 2.2, 3.2), grey);
    t.position.set(0, 1.1, 1.5);
    sub.add(t);
    const p = new THREE.Mesh(new THREE.BoxGeometry(2.6, 2, 0.8), orange);
    p.position.set(0, 1, -1.2);
    sub.add(p);
    const m = mergeByMaterial(sub);
    m.position.set(SUBSTATION.x, SUBSTATION.level, driveZ(SUBSTATION.level) + 6.8);
    group.add(m);
  }
  // The paste borehole, from the plant down to 050.
  {
    const hole = new THREE.Mesh(new THREE.CylinderGeometry(0.22, 0.22, 50, 8), new THREE.MeshStandardMaterial({ color: "#c2c4c4", metalness: 0.7, roughness: 0.4, emissive: "#3c3c3c" }));
    hole.position.set(PASTE_HOLE.x, -25, PASTE_HOLE.z - 1.5);
    group.add(hole);
  }
  await step();

  const surface = makeSurface(small);
  group.add(surface.group);

  const end = (id: string) => {
    const l = net.lines.get(id)!;
    const n = l.pts.length;
    const p = l.pts[n - 1]!.clone();
    const dir = p.clone().sub(l.pts[n - 2]!).setY(0).normalize();
    return { p, dir };
  };
  const capUp = new THREE.Vector3();
  return {
    group,
    net,
    uniforms,
    rock,
    shell,
    shellMat,
    lens,
    lensMat,
    stopes,
    cavity,
    heat,
    // Underbreak is the shell of ore left inside the design; overbreak is whatever else the opening gained, so planned
    // plus overbreak less underbreak is exactly the scanned tonnage the card shows.
    scan: (() => {
      const d = tonnes(design.volume);
      const s = tonnes(scanned.volume);
      const under = tonnes(scanned.under ?? 0);
      return { design: d, surveyed: s, over: s - d + under, under, elos: scanned.hwElos ?? 0 };
    })(),
    muck,
    refuge,
    surface,
    block,
    lamps: lamps.update,
    tick(dt: number, t: number, cam: THREE.Vector3, capLamps: readonly THREE.Object3D[]) {
      lamps.update(cam);
      // Cap lamps light the rock: each lamp's position and the way its wearer faces.
      let n = 0;
      for (const l of capLamps) {
        if (n >= CAPS) break;
        if (!l.visible || !l.parent) continue;
        l.getWorldPosition(uniforms.uCapP.value[n]!);
        // The lamp shines along the head's forward axis, tipped a little down (as miners.ts aims its own spotlight).
        uniforms.uCapD.value[n]!.setFromMatrixColumn(l.matrixWorld, 2).normalize().addScaledVector(capUp.setFromMatrixColumn(l.matrixWorld, 1).normalize(), -0.25).normalize();
        n++;
      }
      uniforms.uCapN.value = n;
      air.update(dt);
      tickShafts(t);
      // Unlit stope highlights cost a draw call each: skip them while they are clear.
      for (const v of stopes.values()) {
        const m = v.design.material as THREE.MeshBasicMaterial;
        v.design.visible = m.opacity > 0.002;
      }
    },
    instruments: serv.instruments,
    end,
  };
}

