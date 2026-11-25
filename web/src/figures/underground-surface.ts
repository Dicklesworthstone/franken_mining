/**
 * The ground surface over the underground mine: a thin slab of ground you can see down through when you look from
 * above, the box cut and portal where the decline starts, the headframe over the shaft, the main exhaust fan on the
 * return-air raise with its plume, the paste plant over its borehole, offices, a change house, a workshop and the
 * ROM pad. All generic and synthetic.
 */
import * as THREE from "three";
import { mergeGeometries } from "three/examples/jsm/utils/BufferGeometryUtils.js";
import { makeLightTower, makePickup, makeStockpile } from "./fleet";
import { noiseTexture } from "./fleet-kit";
import { rng } from "./lib";
import { BOX, BOXCUT, DECLINE, PASTE_HOLE, RAR, SHAFT } from "./underground-plan";

/** Merges every mesh in a group by material, so a building is a few draw calls. */
export function mergeByMaterial(root: THREE.Object3D, shadows = true): THREE.Group {
  root.updateMatrixWorld(true);
  const sets = new Map<THREE.Material, THREE.BufferGeometry[]>();
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh || Array.isArray(m.material)) return;
    let g = m.geometry.clone().applyMatrix4(m.matrixWorld);
    if (g.index) g = g.toNonIndexed();
    for (const k of Object.keys(g.attributes)) if (k !== "position" && k !== "normal" && k !== "uv") g.deleteAttribute(k);
    if (!g.getAttribute("uv")) g.setAttribute("uv", new THREE.Float32BufferAttribute(new Float32Array(g.getAttribute("position").count * 2), 2));
    const list = sets.get(m.material);
    if (list) list.push(g);
    else sets.set(m.material, [g]);
  });
  const out = new THREE.Group();
  for (const [mat, geos] of sets) {
    const g = mergeGeometries(geos);
    for (const x of geos) x.dispose();
    if (!g) continue;
    const mesh = new THREE.Mesh(g, mat);
    mesh.castShadow = shadows;
    mesh.receiveShadow = true;
    out.add(mesh);
  }
  return out;
}

/**
 * Merges the static meshes under `root` that share a material and the same vertex attributes into one mesh each,
 * keeping every attribute (the fleet kit's baked parts carry their own). Instanced and skinned meshes, sprites, lines
 * and lights are moved across unchanged. For parked vehicles and fixed plant that never animate.
 */
export function mergeStatic(root: THREE.Object3D): THREE.Group {
  root.updateMatrixWorld(true);
  const out = new THREE.Group();
  const sets = new Map<string, { mat: THREE.Material; geos: THREE.BufferGeometry[]; cast: boolean; src: THREE.Mesh[] }>();
  const move: THREE.Object3D[] = [];
  const shown = (o: THREE.Object3D) => {
    for (let p: THREE.Object3D | null = o; p && p !== root; p = p.parent) if (!p.visible) return false;
    return true;
  };
  const ids = new Map<THREE.Material, number>();
  root.traverse((o) => {
    if (o === root || !shown(o)) return;
    const m = o as THREE.Mesh;
    if ((o as THREE.Light).isLight || (o as THREE.Sprite).isSprite || (o as THREE.Line).isLine || (o as THREE.Points).isPoints) {
      move.push(o);
      return;
    }
    if (!m.isMesh) return;
    const special = (m as unknown as THREE.InstancedMesh).isInstancedMesh || (m as unknown as THREE.SkinnedMesh).isSkinnedMesh;
    if (special || Array.isArray(m.material) || Object.keys(m.geometry.morphAttributes).length) {
      move.push(o);
      return;
    }
    let g = m.geometry.clone().applyMatrix4(m.matrixWorld);
    if (g.index) g = g.toNonIndexed();
    const sig = Object.keys(g.attributes)
      .sort()
      .map((k) => k + g.getAttribute(k).itemSize)
      .join(",");
    if (!ids.has(m.material)) ids.set(m.material, ids.size);
    const key = ids.get(m.material) + "|" + sig;
    const set = sets.get(key) ?? { mat: m.material, geos: [], cast: false, src: [] };
    set.geos.push(g);
    set.src.push(m);
    set.cast ||= m.castShadow;
    sets.set(key, set);
  });
  for (const { mat, geos, cast, src } of sets.values()) {
    const g = geos.length > 1 ? mergeGeometries(geos) : geos[0]!;
    if (!g) {
      for (const x of geos) x.dispose();
      move.push(...src);
      continue;
    }
    if (geos.length > 1) for (const x of geos) x.dispose();
    const mesh = new THREE.Mesh(g, mat);
    mesh.castShadow = cast;
    mesh.receiveShadow = true;
    out.add(mesh);
  }
  for (const o of move) out.attach(o);
  return out;
}

const M = {
  steel: new THREE.MeshStandardMaterial({ color: "#7b2f22", roughness: 0.62, metalness: 0.35 }),
  grey: new THREE.MeshStandardMaterial({ color: "#7f8487", roughness: 0.55, metalness: 0.5 }),
  galv: new THREE.MeshStandardMaterial({ color: "#a5a9a8", roughness: 0.42, metalness: 0.7 }),
  clad: new THREE.MeshStandardMaterial({ color: "#c9c4b7", roughness: 0.8 }),
  cladG: new THREE.MeshStandardMaterial({ color: "#8f9a8c", roughness: 0.75 }),
  roof: new THREE.MeshStandardMaterial({ color: "#6d7276", roughness: 0.6, metalness: 0.3 }),
  white: new THREE.MeshStandardMaterial({ color: "#dcdad3", roughness: 0.6 }),
  concrete: new THREE.MeshStandardMaterial({ color: "#a29d93", roughness: 0.92 }),
  dark: new THREE.MeshStandardMaterial({ color: "#2a2a28", roughness: 0.7 }),
  glass: new THREE.MeshStandardMaterial({ color: "#1d2a33", roughness: 0.15, metalness: 0.4, emissive: "#ffcf8a", emissiveIntensity: 0.12 }),
  yellow: new THREE.MeshStandardMaterial({ color: "#d79a1c", roughness: 0.6 }),
  rock: new THREE.MeshStandardMaterial({ color: "#7b6c5b", roughness: 0.95 }),
  road: new THREE.MeshStandardMaterial({ color: "#8b7e6b", roughness: 0.97 }),
};

const box = (p: THREE.Object3D, m: THREE.Material, w: number, h: number, d: number, x: number, y: number, z: number, ry = 0) => {
  const b = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), m);
  b.position.set(x, y + h / 2, z);
  b.rotation.y = ry;
  p.add(b);
  return b;
};
const cylY = (p: THREE.Object3D, m: THREE.Material, r: number, h: number, x: number, y: number, z: number, seg = 20, r2 = r) => {
  const c = new THREE.Mesh(new THREE.CylinderGeometry(r2, r, h, seg), m);
  c.position.set(x, y + h / 2, z);
  p.add(c);
  return c;
};
/** A strut between two points. */
const strut = (p: THREE.Object3D, m: THREE.Material, a: THREE.Vector3, b: THREE.Vector3, s: number) => {
  const d = b.clone().sub(a);
  const g = new THREE.Mesh(new THREE.BoxGeometry(s, d.length(), s), m);
  g.position.copy(a).add(b).multiplyScalar(0.5);
  g.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), d.normalize());
  p.add(g);
  return g;
};

/** A simple building: walls, a low-pitch roof, a band of windows and a door. */
function building(w: number, d: number, h: number, wall: THREE.Material, o: { windows?: boolean; storeys?: number; door?: boolean } = {}) {
  const g = new THREE.Group();
  box(g, wall, w, h, d, 0, 0, 0);
  // Roof with a slight fall and overhang.
  const roof = new THREE.Mesh(new THREE.BoxGeometry(w + 0.8, 0.35, d + 0.8), M.roof);
  roof.position.set(0, h + 0.2, 0);
  roof.rotation.x = 0.03;
  g.add(roof);
  if (o.windows) {
    const n = o.storeys ?? 1;
    for (let s = 0; s < n; s++)
      for (const side of [-1, 1]) {
        const y = 1.1 + s * (h / n);
        const win = new THREE.Mesh(new THREE.BoxGeometry(w * 0.86, 1.1, 0.08), M.glass);
        win.position.set(0, y + 0.55, side * (d / 2 + 0.02));
        g.add(win);
      }
  }
  if (o.door) box(g, M.dark, Math.min(4.5, w * 0.3), Math.min(4.6, h * 0.7), 0.1, w * 0.25, 0, d / 2 + 0.03);
  return g;
}

/* ---------- The ground slab ---------- */

/** The site plan painted onto the slab: ground, scrub, gravel pads and roads. */
function sitePlan() {
  const W = 1024;
  const H = Math.round((W * (BOX.z1 - BOX.z0)) / (BOX.x1 - BOX.x0));
  const c = document.createElement("canvas");
  c.width = W;
  c.height = H;
  const x = c.getContext("2d")!;
  const sx = (v: number) => ((v - BOX.x0) / (BOX.x1 - BOX.x0)) * W;
  const sz = (v: number) => ((v - BOX.z0) / (BOX.z1 - BOX.z0)) * H;
  const m = W / (BOX.x1 - BOX.x0);
  x.fillStyle = "#7d6c57";
  x.fillRect(0, 0, W, H);
  const r = rng(12);
  // Blotches of soil colour and scrub.
  for (let i = 0; i < 900; i++) {
    const px = r() * W;
    const pz = r() * H;
    const rad = 6 + r() * 40;
    x.fillStyle = `rgba(${90 + r() * 50},${74 + r() * 40},${56 + r() * 30},${0.08 + r() * 0.12})`;
    x.beginPath();
    x.arc(px, pz, rad, 0, Math.PI * 2);
    x.fill();
  }
  for (let i = 0; i < 1500; i++) {
    x.fillStyle = `rgba(${52 + r() * 20},${56 + r() * 20},${38 + r() * 12},${0.5 + r() * 0.4})`;
    x.beginPath();
    x.arc(r() * W, r() * H, 0.6 + r() * 2.2, 0, Math.PI * 2);
    x.fill();
  }
  const pad = (x0: number, z0: number, x1: number, z1: number, col = "#a3957f") => {
    x.fillStyle = col;
    x.fillRect(sx(x0), sz(z0), sx(x1) - sx(x0), sz(z1) - sz(z0));
  };
  const road = (pts: [number, number][], w: number, col = "#a99b85") => {
    x.strokeStyle = col;
    x.lineWidth = w * m;
    x.lineCap = "round";
    x.lineJoin = "round";
    x.beginPath();
    pts.forEach(([px, pz], i) => (i ? x.lineTo(sx(px), sz(pz)) : x.moveTo(sx(px), sz(pz))));
    x.stroke();
  };
  // Pads: portal and office area, ROM pad, workshop, paste plant, headframe, fan.
  pad(-52, 52, 40, 112);
  pad(40, 40, 110, 100, "#9a8b74");
  pad(18, -10, 56, 26);
  pad(-128, -4, -66, 32);
  pad(160, -28, 196, 6);
  road(
    [
      [22, BOXCUT.z],
      [60, 70],
      [130, 70],
      [178, 20],
    ],
    9,
  );
  road(
    [
      [30, 70],
      [36, 26],
    ],
    7,
  );
  road(
    [
      [-40, 52],
      [-70, 30],
      [-95, 18],
    ],
    7,
  );
  road(
    [
      [130, 70],
      [205, 70],
    ],
    9,
  );
  // Car park lines.
  x.strokeStyle = "rgba(235,228,210,0.55)";
  x.lineWidth = 1;
  for (let i = 0; i < 14; i++) {
    const px = sx(-46 + i * 3);
    x.beginPath();
    x.moveTo(px, sz(100));
    x.lineTo(px, sz(106));
    x.stroke();
  }
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  t.anisotropy = 8;
  return t;
}

/**
 * The slab: the site plan on top, more see-through the more steeply you look down on it, with the box cut left open.
 */
function slab(holes: ((x: number, z: number) => boolean)[]) {
  const step = 3;
  const nx = Math.round((BOX.x1 - BOX.x0) / step);
  const nz = Math.round((BOX.z1 - BOX.z0) / step);
  const pos: number[] = [];
  const uv: number[] = [];
  const idx: number[] = [];
  for (let j = 0; j <= nz; j++)
    for (let i = 0; i <= nx; i++) {
      const x = BOX.x0 + i * step;
      const z = BOX.z0 + j * step;
      pos.push(x, 0, z);
      uv.push(i / nx, 1 - j / nz);
    }
  for (let j = 0; j < nz; j++)
    for (let i = 0; i < nx; i++) {
      const cx = BOX.x0 + (i + 0.5) * step;
      const cz = BOX.z0 + (j + 0.5) * step;
      if (holes.some((h) => h(cx, cz))) continue;
      const a = j * (nx + 1) + i;
      idx.push(a, a + nx + 1, a + 1, a + 1, a + nx + 1, a + nx + 2);
    }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute("uv", new THREE.Float32BufferAttribute(uv, 2));
  g.setIndex(idx);
  g.computeVertexNormals();
  const mat = new THREE.MeshStandardMaterial({ map: sitePlan(), roughness: 0.96, transparent: true, depthWrite: false });
  const fade = { value: 1 };
  mat.customProgramCacheKey = () => "ug-slab-v1";
  mat.onBeforeCompile = (sh) => {
    sh.uniforms.uFade = fade;
    sh.uniforms.tNoise = { value: noiseTexture() };
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vSw;")
      .replace("#include <worldpos_vertex>", "#include <worldpos_vertex>\nvSw = (modelMatrix * vec4(transformed, 1.0)).xyz;");
    sh.fragmentShader = sh.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform float uFade;\nuniform sampler2D tNoise;\nvarying vec3 vSw;")
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec4 gn = texture2D(tNoise, vSw.xz * 0.05);
diffuseColor.rgb *= 0.85 + 0.3 * gn.b;
vec3 vd = normalize(cameraPosition - vSw);
float graze = 1.0 - abs(vd.y);
// More solid at a glance, see-through from above; solid within a few metres of the edge, so the slab has a lip.
float edge = min(min(vSw.x - ${BOX.x0.toFixed(1)}, ${BOX.x1.toFixed(1)} - vSw.x), min(vSw.z - ${BOX.z0.toFixed(1)}, ${BOX.z1.toFixed(1)} - vSw.z));
float a = mix(0.32, 0.96, pow(graze, 1.6));
a = max(a, 1.0 - smoothstep(0.0, 2.5, edge));
diffuseColor.a = a * uFade;`,
      );
  };
  const mesh = new THREE.Mesh(g, mat);
  mesh.receiveShadow = true;
  mesh.renderOrder = 5;
  return { mesh, fade };
}

/* ---------- Pieces ---------- */

function boxCut() {
  const g = new THREE.Group();
  const { x0, x1, z, w } = BOXCUT;
  const floorY = (x: number) => DECLINE.portalY * (1 - (x - x0) / (x1 - x0));
  // Floor (road) and side walls, falling from the surface to the portal.
  const n = 16;
  const fl: number[] = [];
  const wl: number[] = [];
  for (let i = 0; i < n; i++) {
    const xa = x0 + ((x1 - x0) * i) / n;
    const xb = x0 + ((x1 - x0) * (i + 1)) / n;
    const ya = floorY(xa);
    const yb = floorY(xb);
    fl.push(xa, ya, z - w / 2, xa, ya, z + w / 2, xb, yb, z - w / 2, xb, yb, z - w / 2, xa, ya, z + w / 2, xb, yb, z + w / 2);
    for (const s of [-1, 1]) {
      const zz = z + (s * w) / 2;
      if (s < 0) wl.push(xa, ya, zz, xb, yb, zz, xa, 0, zz, xb, yb, zz, xb, 0, zz, xa, 0, zz);
      else wl.push(xa, ya, zz, xa, 0, zz, xb, yb, zz, xb, yb, zz, xa, 0, zz, xb, 0, zz);
    }
  }
  const tri = (a: number[], m: THREE.Material) => {
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.Float32BufferAttribute(a, 3));
    geo.computeVertexNormals();
    const mesh = new THREE.Mesh(geo, m);
    mesh.receiveShadow = true;
    g.add(mesh);
    return mesh;
  };
  tri(fl, M.road);
  const walls = tri(wl, M.rock);
  (walls.material as THREE.Material).side = THREE.DoubleSide;
  // The portal face above and beside the tunnel, a steel culvert canopy, and a concrete collar.
  const py = DECLINE.portalY;
  box(g, M.rock, 0.8, -py - DECLINE.h, w, x0 - 0.4, py + DECLINE.h, z);
  for (const s of [-1, 1]) box(g, M.rock, 0.8, DECLINE.h, (w - DECLINE.w) / 2, x0 - 0.4, py, z + (s * (w + DECLINE.w)) / 4);
  const canopy = new THREE.Mesh(new THREE.CylinderGeometry(3.15, 3.15, 7, 28, 1, true, 0, Math.PI), M.galv);
  canopy.rotation.z = Math.PI / 2;
  canopy.rotation.y = Math.PI / 2;
  canopy.position.set(x0 + 3.3, py + 2.7, z);
  canopy.material.side = THREE.DoubleSide;
  g.add(canopy);
  for (let i = 0; i < 8; i++) {
    const rib = new THREE.Mesh(new THREE.TorusGeometry(3.2, 0.07, 6, 28, Math.PI), M.grey);
    rib.rotation.y = Math.PI / 2;
    rib.position.set(x0 + 0.2 + i * 0.95, py + 2.7, z);
    g.add(rib);
  }
  for (const s of [-1, 1]) box(g, M.rock, 0.1, 2.7, 0.3, x0 + 6.8, py, z + s * 3.15);
  return g;
}

function headframe() {
  const g = new THREE.Group();
  const H = 30;
  const s = 3.2;
  const L = [
    [-s, -s],
    [s, -s],
    [s, s],
    [-s, s],
  ] as const;
  for (const [x, z] of L) strut(g, M.steel, new THREE.Vector3(x * 1.25, 0, z * 1.25), new THREE.Vector3(x * 0.7, H, z * 0.7), 0.55);
  // Bracing at each panel.
  for (let k = 0; k < 5; k++) {
    const y0 = (k * H) / 5;
    const y1 = ((k + 1) * H) / 5;
    const f0 = 1.25 - (0.55 * k) / 5;
    const f1 = 1.25 - (0.55 * (k + 1)) / 5;
    for (let i = 0; i < 4; i++) {
      const a = L[i]!;
      const b = L[(i + 1) % 4]!;
      strut(g, M.steel, new THREE.Vector3(a[0] * f1, y1, a[1] * f1), new THREE.Vector3(b[0] * f1, y1, b[1] * f1), 0.3);
      strut(g, M.steel, new THREE.Vector3(a[0] * f0, y0, a[1] * f0), new THREE.Vector3(b[0] * f1, y1, b[1] * f1), 0.18);
    }
  }
  // Back legs to the winder (west).
  for (const z of [-2.6, 2.6]) strut(g, M.steel, new THREE.Vector3(-17, 0, z * 1.4), new THREE.Vector3(-1.8, H - 1, z), 0.6);
  // Sheave deck and wheels facing the winder.
  box(g, M.steel, 7, 0.5, 7, 0, H, 0);
  for (const z of [-1.3, 1.3]) {
    const wheel = new THREE.Mesh(new THREE.TorusGeometry(2.1, 0.16, 8, 32), M.grey);
    wheel.position.set(-0.5, H + 2.4, z);
    g.add(wheel);
    for (let i = 0; i < 4; i++) {
      const sp = new THREE.Mesh(new THREE.BoxGeometry(4.1, 0.1, 0.1), M.grey);
      sp.position.copy(wheel.position);
      sp.rotation.z = (i * Math.PI) / 4;
      g.add(sp);
    }
  }
  box(g, M.steel, 5, 2.2, 1.2, -0.5, H + 0.4, 0);
  // Ropes down to the winder house's drums.
  for (const z of [-1.3, 1.3]) strut(g, M.dark, new THREE.Vector3(-2.6, H + 2.4, z), new THREE.Vector3(-34, 6.5, z * 0.8), 0.07);
  // A bin on the frame and the collar.
  box(g, M.grey, 5, 4, 4, 3.4, 9, 0);
  box(g, M.concrete, 10, 0.6, 10, 0, -0.1, 0);
  const house = building(16, 12, 11, M.cladG, { door: true });
  house.position.set(-38, 0, 0);
  g.add(house);
  return g;
}

function fan() {
  const g = new THREE.Group();
  box(g, M.concrete, 7, 3, 7, 0, 0, 0);
  const housing = new THREE.Mesh(new THREE.CylinderGeometry(2, 2, 7, 24), M.galv);
  housing.rotation.z = Math.PI / 2;
  housing.position.set(-6.5, 2.5, 0);
  g.add(housing);
  box(g, M.grey, 3, 2.6, 2.8, -11.5, 1.2, 0);
  box(g, M.concrete, 12, 0.5, 6, -6, 0, 0);
  // The evasé: a widening stack that slows the air before it leaves.
  const ev = new THREE.Mesh(new THREE.CylinderGeometry(3.6, 2.1, 9, 24, 1, true), M.galv);
  ev.position.set(0, 7.5, 0);
  ev.material.side = THREE.DoubleSide;
  g.add(ev);
  for (const y of [3.2, 6, 9, 12]) {
    const r = 2.1 + ((y - 3) / 9) * 1.5;
    const ring = new THREE.Mesh(new THREE.TorusGeometry(r + 0.04, 0.08, 6, 24), M.grey);
    ring.rotation.x = Math.PI / 2;
    ring.position.set(0, y, 0);
    g.add(ring);
  }
  return g;
}

function pastePlant() {
  const g = new THREE.Group();
  const b = building(18, 13, 15, M.clad, { door: true });
  g.add(b);
  for (const [x, z] of [
    [13, -4],
    [13, 4],
  ] as const) {
    cylY(g, M.white, 2.6, 13, x, 7, z);
    const cone = new THREE.Mesh(new THREE.ConeGeometry(2.6, 3.4, 20), M.white);
    cone.rotation.x = Math.PI;
    cone.position.set(x, 5.3, z);
    g.add(cone);
    for (const [dx, dz] of [
      [-1.7, -1.7],
      [1.7, -1.7],
      [1.7, 1.7],
      [-1.7, 1.7],
    ] as const)
      box(g, M.grey, 0.3, 7, 0.3, x + dx, 0, z + dz);
  }
  // The mixer gallery and the borehole collar.
  box(g, M.grey, 6, 3, 3, -1, 15, 0);
  cylY(g, M.yellow, 0.4, 1.2, PASTE_HOLE.x - 36, 0, PASTE_HOLE.z - 8.25 + 8.5, 12);
  return g;
}

export interface Surface {
  group: THREE.Group;
  slab: THREE.Mesh;
  /** Fade the slab out (0) or in (1). */
  fade: { value: number };
  plume: THREE.Object3D[];
  lamps: THREE.Object3D[];
  update(dt: number, t: number): void;
}

export function makeSurface(small: boolean): Surface {
  const group = new THREE.Group();
  const cut = (x: number, z: number) => x > BOXCUT.x0 - 1 && x < BOXCUT.x1 + 2 && Math.abs(z - BOXCUT.z) < BOXCUT.w / 2 + 0.5;
  const shaft = (x: number, z: number) => Math.hypot(x - SHAFT.x, z - SHAFT.z) < SHAFT.r + 1;
  const rar = (x: number, z: number) => Math.hypot(x - RAR.x, z - RAR.z) < RAR.r + 1;
  const { mesh: slabMesh, fade } = slab([cut, shaft, rar]);
  group.add(slabMesh);
  // Everything that never moves goes into one static set, merged by material.
  const parked = new THREE.Group();
  parked.add(boxCut());

  const statics = new THREE.Group();
  const hf = headframe();
  hf.position.set(SHAFT.x, 0, SHAFT.z);
  statics.add(hf);
  const f = fan();
  f.position.set(RAR.x, 0, RAR.z);
  statics.add(f);
  const pp = pastePlant();
  pp.position.set(36, 0, 8);
  statics.add(pp);
  const office = building(30, 12, 7.2, M.white, { windows: true, storeys: 2, door: true });
  office.position.set(-8, 0, 96);
  statics.add(office);
  const change = building(22, 11, 4.6, M.clad, { windows: true, door: true });
  change.position.set(28, 0, 98);
  statics.add(change);
  const shop = building(30, 20, 12, M.cladG, { door: true });
  shop.position.set(80, 0, 88);
  statics.add(shop);
  const sub = new THREE.Group();
  box(sub, M.concrete, 12, 0.4, 8, 0, 0, 0);
  for (const x of [-3, 2]) box(sub, M.grey, 3, 3.4, 2.4, x, 0.4, 0);
  sub.position.set(150, 0, 2);
  statics.add(sub);
  const merged = mergeByMaterial(statics);
  group.add(merged);

  // The ROM pad, a light tower at the portal and one on the pad, and pickups at the office.
  if (!small) {
    const pile = makeStockpile({ radius: 14, height: 6.5, shape: "dump", palette: "ore", seed: 4, boulders: 40 });
    pile.group.position.set(78, 0, 60);
    parked.add(pile.group);
  }
  const lamps: THREE.Object3D[] = [];
  for (const [x, z, ry] of [
    [-18, 76, -2.2],
    [62, 46, 0.7],
  ] as const) {
    const t = makeLightTower();
    t.set({ mast: 0.9, lights: true, yaw: 0 });
    t.group.position.set(x, 0, z);
    t.group.rotation.y = ry;
    parked.add(t.group);
    lamps.push(...t.anchors.lamps);
  }
  for (const [x, z, ry] of [
    [-38, 104, 0],
    [-32, 104, 0],
    [-23, 104, Math.PI],
    [50, 72, 1.4],
  ] as const) {
    const p = makePickup();
    p.group.position.set(x, 0, z);
    p.group.rotation.y = ry;
    parked.add(p.group);
  }
  group.add(mergeStatic(parked));

  // The fan's plume: warm, wet exhaust air condensing as it meets the cooler air (one draw: soft points).
  const n = small ? 6 : 10;
  const ppos = new Float32Array(n * 3);
  const pdat = new Float32Array(n * 2);
  const pg = new THREE.BufferGeometry();
  pg.setAttribute("position", new THREE.BufferAttribute(ppos, 3));
  pg.setAttribute("puff", new THREE.BufferAttribute(pdat, 2));
  const pmat = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    fog: true,
    uniforms: THREE.UniformsUtils.merge([THREE.UniformsLib.fog, { uView: { value: 900 } }]),
    vertexShader: `attribute vec2 puff;
uniform float uView;
varying float vA;
#include <common>
#include <fog_pars_vertex>
void main() {
  vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
  gl_PointSize = uView * puff.x / -mvPosition.z;
  vA = puff.y;
  gl_Position = projectionMatrix * mvPosition;
  #include <fog_vertex>
}`,
    fragmentShader: `varying float vA;
#include <common>
#include <fog_pars_fragment>
void main() {
  float r = length(gl_PointCoord - 0.5) * 2.0;
  float a = (1.0 - smoothstep(0.0, 1.0, r)) * (0.9 - 0.55 * smoothstep(0.0, 0.5, r)) * vA;
  gl_FragColor = vec4(vec3(0.9, 0.89, 0.86), a);
  #include <fog_fragment>
}`,
  });
  const plumePts = new THREE.Points(pg, pmat);
  plumePts.frustumCulled = false;
  group.add(plumePts);
  const plume: THREE.Object3D[] = [plumePts];
  const update = (_dt: number, t: number) => {
    pmat.uniforms.uView!.value = (typeof window === "undefined" ? 900 : window.innerHeight) * 0.75;
    for (let i = 0; i < n; i++) {
      const ph = i / n;
      const k = (t * 0.07 + ph) % 1;
      ppos[i * 3] = RAR.x - 2 + k * 14;
      ppos[i * 3 + 1] = 12.5 + k * 22;
      ppos[i * 3 + 2] = RAR.z + Math.sin(k * 5 + ph * 9) * 2;
      pdat[i * 2] = 5 + k * 16;
      pdat[i * 2 + 1] = 0.32 * Math.sin(Math.PI * Math.min(1, k * 1.2)) * (1 - k * 0.5);
    }
    pg.attributes.position!.needsUpdate = true;
    pg.attributes.puff!.needsUpdate = true;
  };
  update(0, 0);
  return { group, slab: slabMesh, fade, plume, lamps, update };
}
