/**
 * Small helpers shared by the underground machines and the light vehicles: wheel bakes that keep the
 * tyres' own wear, hydraulic rams that stretch between two points, and fender/arch bands.
 */
import * as THREE from "three";
import { bake, bakeFree, cyl, mats, plate, smoothstep, tyreGeometry, v3, type TyreSpec, type V3 } from "./fleet-kit";

/** Adds a tyre mesh (cached geometry, own wear) to be merged by {@link bakeWheels}. */
export function addTyre(parent: THREE.Object3D, spec: TyreSpec, x = 0, flip = false) {
  const m = new THREE.Mesh(tyreGeometry(spec), mats.rubber());
  m.position.x = x;
  if (flip) m.rotation.y = Math.PI;
  m.userData.keepWear = true;
  parent.add(m);
  return m;
}

/** Bakes a wheel set: tyres merge keeping their wear attribute; everything else bakes normally. */
export function bakeWheels(root: THREE.Object3D): THREE.Group {
  root.updateMatrixWorld(true);
  const tyres: THREE.Mesh[] = [];
  root.traverse((o) => {
    if ((o as THREE.Mesh).isMesh && o.userData.keepWear) tyres.push(o as THREE.Mesh);
  });
  const inv = root.matrixWorld.clone().invert();
  const geos = tyres.map((m) => {
    const g = m.geometry.clone();
    g.applyMatrix4(new THREE.Matrix4().multiplyMatrices(inv, m.matrixWorld));
    return g;
  });
  for (const m of tyres) m.parent?.remove(m);
  const out = new THREE.Group();
  out.add(bake(root, { mudTop: 0.5, mudBottom: -1, mud: 0.8, dust: 0.5 }));
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (m.isMesh) m.geometry.dispose();
  });
  if (geos.length) {
    let n = 0;
    for (const g of geos) n += g.getAttribute("position").count;
    const merged = new THREE.BufferGeometry();
    for (const a of ["position", "normal", "uv", "wear"]) {
      const size = geos[0]!.getAttribute(a).itemSize;
      const arr = new Float32Array(n * size);
      let o = 0;
      for (const g of geos) {
        const src = g.getAttribute(a).array as Float32Array;
        arr.set(src, o);
        o += src.length;
      }
      merged.setAttribute(a, new THREE.BufferAttribute(arr, size));
    }
    merged.computeBoundingSphere();
    for (const g of geos) g.dispose();
    const mesh = new THREE.Mesh(merged, mats.rubber());
    mesh.castShadow = mesh.receiveShadow = true;
    out.add(mesh);
  }
  return out;
}

export interface RamSpec {
  /** Barrel radius. */
  r: number;
  /** Shortest and longest pin-to-pin lengths the ram must cover. */
  min: number;
  max: number;
  barrelMat?: THREE.Material;
  rodMat?: THREE.Material;
}

interface RamGeo {
  barrel: THREE.Group;
  rod: THREE.Group;
  rodLen: number;
}
const ramCache = new Map<string, RamGeo>();

function ramGeo(s: RamSpec): RamGeo {
  const key = `${s.r}:${s.min}:${s.max}:${s.barrelMat?.uuid}:${s.rodMat?.uuid}`;
  const hit = ramCache.get(key);
  if (hit) return hit;
  const barrelLen = Math.max(0.2, s.min - 0.06);
  const rodLen = Math.min(barrelLen - 0.02, Math.max(s.max - barrelLen + 0.18, 0.3));
  const bm = s.barrelMat ?? mats.yellow();
  const rm = s.rodMat ?? mats.chrome();
  const b = new THREE.Group();
  cyl(b, bm, s.r, barrelLen - s.r, 0, s.r + (barrelLen - s.r) / 2, 0, { seg: 14 });
  cyl(b, bm, s.r * 1.12, 0.06, 0, barrelLen - 0.03, 0, { seg: 14 }); // gland
  cyl(b, bm, s.r * 0.75, s.r * 2.2, 0, 0, 0, { axis: "x", seg: 12 }); // pin eye
  cyl(b, mats.steel(), s.r * 0.35, s.r * 2.6, 0, 0, 0, { axis: "x", seg: 8 });
  const r = new THREE.Group();
  cyl(r, rm, s.r * 0.55, rodLen, 0, rodLen / 2, 0, { seg: 12 });
  cyl(r, bm, s.r * 0.7, s.r * 2.0, 0, rodLen, 0, { axis: "x", seg: 12 });
  const g: RamGeo = {
    barrel: bakeFree(b, { mudTop: 1, mudBottom: -1, mud: 0.3 }),
    rod: bakeFree(r, { mudTop: 0, mudBottom: -1, mud: 0 }),
    rodLen,
  };
  ramCache.set(key, g);
  return g;
}

export interface Ram {
  root: THREE.Group;
  /** Stretch the ram from a to b (both in the ram's parent frame). */
  set(a: V3, b: V3): void;
}

/** Hydraulic ram (barrel + chrome rod with pin eyes) that spans two points each frame. */
export function makeRam(parent: THREE.Object3D, s: RamSpec): Ram {
  const g = ramGeo(s);
  const root = new THREE.Group();
  const barrel = g.barrel.clone(true);
  const rod = g.rod.clone(true);
  root.add(barrel, rod);
  parent.add(root);
  const Y = new THREE.Vector3(0, 1, 0);
  const d = new THREE.Vector3();
  return {
    root,
    set(a, b) {
      root.position.copy(a);
      d.copy(b).sub(a);
      const len = d.length();
      if (len > 1e-6) root.quaternion.setFromUnitVectors(Y, d.multiplyScalar(1 / len));
      rod.position.y = len - g.rodLen;
    },
  };
}

/**
 * Curved band (fender, wheel-arch flare) round an axle along x: from angle a0 to a1 (radians, 0 = +z
 * forward, π/2 = up), radius r, across x from x0 to x1.
 */
export function arcBand(
  parent: THREE.Object3D,
  mat: THREE.Material,
  cy: number,
  cz: number,
  r: number,
  x0: number,
  x1: number,
  a0: number,
  a1: number,
  t = 0.05,
  n = 10,
) {
  for (let i = 0; i < n; i++) {
    const p = a0 + ((a1 - a0) * i) / n;
    const q = a0 + ((a1 - a0) * (i + 1)) / n;
    const P = (a: number, x: number) => v3(x, cy + Math.sin(a) * r, cz + Math.cos(a) * r);
    plate(parent, mat, [P(p, x0), P(p, x1), P(q, x1), P(q, x0)], t, { out: v3(0, Math.sin((p + q) / 2), Math.cos((p + q) / 2)) });
  }
}

/** A side flare (arch lip) round a wheel opening in the x = xs plane: a ring sector of width w. */
export function archLip(parent: THREE.Object3D, mat: THREE.Material, xs: number, cy: number, cz: number, r: number, w: number, a0: number, a1: number, depth = 0.05, n = 12) {
  for (let i = 0; i < n; i++) {
    const p = a0 + ((a1 - a0) * i) / n;
    const q = a0 + ((a1 - a0) * (i + 1)) / n;
    const P = (a: number, rr: number) => v3(xs, cy + Math.sin(a) * rr, cz + Math.cos(a) * rr);
    plate(parent, mat, [P(p, r), P(q, r), P(q, r + w), P(p, r + w)], depth, { out: v3(Math.sign(xs) || 1, 0, 0) });
  }
}

/** Ackermann split of an average steer angle for a wheel on side (+1 left / -1 right). */
export function ackermann(steer: number, side: number, wheelbase: number, track: number) {
  if (Math.abs(steer) < 1e-4) return steer;
  const R = wheelbase / Math.tan(Math.abs(steer));
  const inner = Math.sign(steer) === side;
  return Math.sign(steer) * Math.atan(wheelbase / (R + (inner ? -track / 2 : track / 2)));
}

export { smoothstep };
