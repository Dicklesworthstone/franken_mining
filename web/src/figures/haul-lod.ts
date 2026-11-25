/**
 * Draw-call thrift for the haul scene. `bake` merges a model that no longer moves (a light tower with its mast up, a
 * parked pickup) into one mesh per material, in the model's own frame, keeping the same material objects so lamps
 * still switch. `truckDetail` hides a truck's small parts (hoist rams, hoses, grates, chrome) when it is too far
 * away for them to cover a pixel. Nothing touches the DOM.
 */
import * as THREE from "three";
import { mergeGeometries } from "three/examples/jsm/utils/BufferGeometryUtils.js";

/** Merges every mesh under `root` by material and shadow flags into meshes directly under `root`. */
export function bake(root: THREE.Object3D) {
  root.updateMatrixWorld(true);
  const inv = new THREE.Matrix4().copy(root.matrixWorld).invert();
  const groups = new Map<string, { mat: THREE.Material; cast: boolean; receive: boolean; geos: THREE.BufferGeometry[]; meshes: THREE.Mesh[] }>();
  root.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh || (m as unknown as THREE.InstancedMesh).isInstancedMesh || (m as unknown as THREE.SkinnedMesh).isSkinnedMesh || Array.isArray(m.material) || !m.visible) return;
    const key = `${m.material.uuid}|${m.castShadow}|${m.receiveShadow}|${m.renderOrder}`;
    let g = groups.get(key);
    if (!g) groups.set(key, (g = { mat: m.material, cast: m.castShadow, receive: m.receiveShadow, geos: [], meshes: [] }));
    const geo = (m.geometry.index ? m.geometry.toNonIndexed() : m.geometry.clone()) as THREE.BufferGeometry;
    geo.applyMatrix4(new THREE.Matrix4().multiplyMatrices(inv, m.matrixWorld));
    g.geos.push(geo);
    g.meshes.push(m);
  });
  const made: THREE.Mesh[] = [];
  for (const g of groups.values()) {
    if (g.meshes.length < 2) {
      for (const geo of g.geos) geo.dispose();
      continue;
    }
    // Keep only the attributes every part has.
    const names = Object.keys(g.geos[0]!.attributes).filter((n) => g.geos.every((q) => q.attributes[n] && q.attributes[n]!.itemSize === g.geos[0]!.attributes[n]!.itemSize));
    for (const q of g.geos) {
      for (const n of Object.keys(q.attributes)) if (!names.includes(n)) q.deleteAttribute(n);
      q.morphAttributes = {};
      q.clearGroups();
    }
    const merged = names.includes("position") && names.includes("normal") ? mergeGeometries(g.geos, false) : null;
    for (const q of g.geos) q.dispose();
    if (!merged) continue;
    merged.computeBoundingSphere();
    const mesh = new THREE.Mesh(merged, g.mat);
    mesh.castShadow = g.cast;
    mesh.receiveShadow = g.receive;
    mesh.renderOrder = g.meshes[0]!.renderOrder;
    mesh.name = `baked:${g.mat.name || g.mat.type}`;
    for (const m of g.meshes) m.visible = false;
    made.push(mesh);
  }
  for (const m of made) root.add(m);
  return made.length;
}

/** The small parts of a haul truck worth hiding at a distance: everything but the frame, the body, the load and the wheels. */
export function truckDetail(group: THREE.Object3D) {
  const small: THREE.Object3D[] = [];
  const kids = group.children;
  // The hoist rams hang off two groups of their own; the chassis carries the hose, grate and chrome as meshes of their own.
  for (const k of kids) {
    if (k.type === "Group" && Math.abs(k.position.y - 1.75) < 1e-3) small.push(k);
  }
  kids[0]?.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh || Array.isArray(m.material)) return;
    if (/^(hose|grate|chrome)$/.test(m.name)) small.push(m);
  });
  let far = false;
  return {
    set(on: boolean) {
      if (on === far) return;
      far = on;
      for (const o of small) o.visible = !on;
    },
    count: small.length,
  };
}
