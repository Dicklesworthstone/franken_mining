/**
 * A development check for z-fighting, loaded only with ?zcheck in the address: it walks the unbaked plant and lists
 * every pair of faces that lie in one plane, face the same way and overlap (box faces and cylinder end caps), plus
 * upward faces that sit within 6 cm of the ground. No DOM access at module level.
 */
import * as THREE from "three";

interface Face {
  /** Axis of the normal (0 x, 1 y, 2 z), its sign, and the plane's offset along it. */
  axis: 0 | 1 | 2;
  sign: 1 | -1;
  at: number;
  /** Extent in the two other axes. */
  a0: number;
  a1: number;
  b0: number;
  b1: number;
  who: string;
}

const AX = [new THREE.Vector3(1, 0, 0), new THREE.Vector3(0, 1, 0), new THREE.Vector3(0, 0, 1)];

/** Faces of axis-aligned boxes and cylinder caps (after their world transform) under `root`. */
function faces(root: THREE.Object3D): Face[] {
  const out: Face[] = [];
  const box = new THREE.Box3();
  const n = new THREE.Vector3();
  const q = new THREE.Quaternion();
  const p = new THREE.Vector3();
  const sc = new THREE.Vector3();
  root.updateMatrixWorld(true);
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh || (m as THREE.InstancedMesh).isInstancedMesh) return;
    const g = m.geometry as THREE.BufferGeometry & { parameters?: Record<string, number> };
    const name = `${m.name || g.type}@${m.getWorldPosition(p).toArray().map((v) => v.toFixed(1)).join(",")}`;
    m.matrixWorld.decompose(p, q, sc);
    if (g.type === "BoxGeometry" || g.type === "RoundedBoxGeometry") {
      // Only boxes whose axes stay aligned with the world (a turn of a multiple of 90 degrees).
      const ok = [0, 1, 2].every((i) => {
        n.copy(AX[i]!).applyQuaternion(q);
        return Math.max(Math.abs(n.x), Math.abs(n.y), Math.abs(n.z)) > 0.9999;
      });
      if (!ok) return;
      g.computeBoundingBox();
      box.copy(g.boundingBox!).applyMatrix4(m.matrixWorld);
      const lo = box.min.toArray();
      const hi = box.max.toArray();
      for (const axis of [0, 1, 2] as const) {
        const [u, v] = [0, 1, 2].filter((k) => k !== axis) as [number, number];
        for (const sign of [-1, 1] as const)
          out.push({ axis, sign, at: sign > 0 ? hi[axis]! : lo[axis]!, a0: lo[u]!, a1: hi[u]!, b0: lo[v]!, b1: hi[v]!, who: name });
      }
    } else if (g.type === "CylinderGeometry") {
      const pr = g.parameters!;
      n.set(0, 1, 0).applyQuaternion(q);
      const ax = Math.abs(n.x) > 0.9999 ? 0 : Math.abs(n.y) > 0.9999 ? 1 : Math.abs(n.z) > 0.9999 ? 2 : -1;
      if (ax < 0 || (pr.openEnded as unknown as boolean)) return;
      const axis = ax as 0 | 1 | 2;
      const h = (pr.height! * sc.y) / 2;
      const c = p.toArray();
      const [u, v] = [0, 1, 2].filter((k) => k !== axis) as [number, number];
      for (const [end, r] of [
        [1, pr.radiusTop!],
        [-1, pr.radiusBottom!],
      ] as const) {
        const s = r * sc.x * 0.7;
        if (s <= 0) continue;
        const sign = (end * Math.sign(n.getComponent(axis))) as 1 | -1;
        out.push({ axis, sign, at: c[axis]! + sign * h, a0: c[u]! - s, a1: c[u]! + s, b0: c[v]! - s, b1: c[v]! + s, who: name });
      }
    }
  });
  return out;
}

/** Pairs of coplanar, same-facing, overlapping faces, and upward faces too close to the ground. */
export function coplanar(root: THREE.Object3D, ground: (x: number, z: number) => number, tol = 0.004): string[] {
  // Downward faces resting on the ground can never be seen from above it: leave them out.
  const list = faces(root).filter((f) => !(f.axis === 1 && f.sign < 0 && f.at <= ground((f.a0 + f.a1) / 2, (f.b0 + f.b1) / 2) + 0.06));
  const found: string[] = [];
  const buckets = new Map<string, Face[]>();
  for (const f of list) {
    const k = `${f.axis}${f.sign}${Math.round(f.at / tol)}`;
    (buckets.get(k) ?? buckets.set(k, []).get(k)!).push(f);
  }
  // A face pressed against an opposite-facing face in the same plane (a box resting on a slab, a wall on its
  // footing) is hidden there; two same-facing faces only fight where neither is covered like that.
  const covered = (axis: number, sign: number, at: number, u0: number, u1: number, v0: number, v1: number) => {
    for (const d of [-1, 0, 1]) {
      const fs = buckets.get(`${axis}${-sign}${Math.round(at / tol) + d}`);
      if (!fs) continue;
      for (const f of fs) if (Math.abs(f.at - at) <= tol && f.a0 <= u0 + 0.005 && f.a1 >= u1 - 0.005 && f.b0 <= v0 + 0.005 && f.b1 >= v1 - 0.005) return true;
    }
    return false;
  };
  for (const [, fs] of buckets) {
    for (let i = 0; i < fs.length; i++)
      for (let j = i + 1; j < fs.length; j++) {
        const a = fs[i]!;
        const b = fs[j]!;
        if (Math.abs(a.at - b.at) > tol) continue;
        const u0 = Math.max(a.a0, b.a0);
        const u1 = Math.min(a.a1, b.a1);
        const v0 = Math.max(a.b0, b.b0);
        const v1 = Math.min(a.b1, b.b1);
        if (u1 - u0 <= 0.01 || v1 - v0 <= 0.01) continue;
        if (covered(a.axis, a.sign, a.at, u0, u1, v0, v1)) continue;
        found.push(`${"xyz"[a.axis]}${a.sign > 0 ? "+" : "-"} ${a.at.toFixed(3)}: ${a.who} | ${b.who} (${(u1 - u0).toFixed(2)}×${(v1 - v0).toFixed(2)})`);
      }
  }
  for (const f of list) {
    if (f.axis !== 1 || f.sign < 0) continue;
    const cx = (f.a0 + f.a1) / 2;
    const cz = (f.b0 + f.b1) / 2;
    const g = ground(cx, cz);
    if (Math.abs(f.at - g) < 0.03) found.push(`ground ${f.at.toFixed(3)} vs ${g.toFixed(3)}: ${f.who}`);
  }
  return found;
}
