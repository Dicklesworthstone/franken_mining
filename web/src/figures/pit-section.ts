/**
 * The nested shells drawn on the section: each shell's profile where the section plane cuts it, and the band
 * between one shell and the next filled faintly in that shell's colour, so the pushbacks read as they would on a
 * planner's cross-section. Inner shells are smaller pits, so their profiles stand in the air above the ultimate
 * pit's floor; the bands of shells beyond the one drawn as ground fall on the rock face, where later pushbacks
 * would cut.
 */
import * as THREE from "three";
import { CUT_Z0 } from "./pit-model";
import { SHELL_COLOURS } from "./pit-blocks";

export interface Section {
  group: THREE.Group;
  /**
   * Dimension lines on the section for the engineering view: crest to crest above the surface, and the depth from
   * the surface to the floor beside the wall. Null hides them.
   */
  dims(d: { xa: number; ya: number; xz: number; yz: number; xd: number; yFloor: number; xDeep: number } | null): void;
  /** New profiles: the ground, then one per shell, inner first, as heights at the section's x stations. */
  set(ground: Float32Array, shells: Float32Array[], seconds: number, highlight: number): void;
  update(dt: number): boolean;
}

export function makeSection(xs: Float32Array): Section {
  const P = xs.length;
  const S = SHELL_COLOURS.length;
  const group = new THREE.Group();
  group.name = "pit-section";
  const z = CUT_Z0 + 0.6;
  // Bands: S strips of P × 2 vertices. Lines: S ribbons of P × 2 vertices.
  const bandPos = new Float32Array(S * P * 2 * 3);
  const bandCol = new Float32Array(S * P * 2 * 4);
  const linePos = new Float32Array(S * P * 2 * 3);
  const lineCol = new Float32Array(S * P * 2 * 4);
  const idx: number[] = [];
  for (let s = 0; s < S; s++)
    for (let p = 0; p < P - 1; p++) {
      const a = (s * P + p) * 2;
      idx.push(a, a + 1, a + 2, a + 2, a + 1, a + 3);
    }
  const mk = (pos: Float32Array, col: Float32Array, order: number, additive: boolean) => {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(pos, 3).setUsage(THREE.DynamicDrawUsage));
    g.setAttribute("color", new THREE.BufferAttribute(col, 4).setUsage(THREE.DynamicDrawUsage));
    g.setIndex(idx);
    g.boundingSphere = new THREE.Sphere(new THREE.Vector3(0, -120, CUT_Z0), 600);
    const m = new THREE.Mesh(
      g,
      new THREE.MeshBasicMaterial({
        vertexColors: true,
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
        blending: additive ? THREE.AdditiveBlending : THREE.NormalBlending,
        fog: true,
        // In front of the rock face it is drawn on, never sharing its depth.
        polygonOffset: true,
        polygonOffsetFactor: -2,
        polygonOffsetUnits: -4 - order,
      }),
    );
    m.renderOrder = order;
    group.add(m);
    return g;
  };
  const bands = mk(bandPos, bandCol, 3, false);
  const lines = mk(linePos, lineCol, 4, false);
  const colours = SHELL_COLOURS.map((c) => new THREE.Color(c));

  // Profiles now and the morph between them.
  const from = Array.from({ length: S + 1 }, () => new Float32Array(P));
  const to = Array.from({ length: S + 1 }, () => new Float32Array(P));
  const now = Array.from({ length: S + 1 }, () => new Float32Array(P));
  let t = 1;
  let span = 1;
  let lit = -1;
  let fresh = true;
  const ease = (x: number) => x * x * (3 - 2 * x);

  const draw = () => {
    for (let s = 0; s < S; s++) {
      const upper = now[s]!;
      const lower = now[s + 1]!;
      const c = colours[s]!;
      const on = s === lit;
      const bandA = on ? 0.34 : 0.2;
      const lineA = on ? 1 : 0.75;
      for (let p = 0; p < P; p++) {
        const x = xs[p]!;
        const top = upper[p]!;
        // A shell's band runs from the shell inside it down to its own profile; nothing where they meet.
        const bot = Math.min(lower[p]!, top);
        const v = (s * P + p) * 2;
        const gap = top - bot;
        const a = gap > 0.4 ? bandA : 0;
        bandPos.set([x, top, z, x, bot, z], v * 3);
        bandCol.set([c.r, c.g, c.b, a * 0.8, c.r, c.g, c.b, a], v * 4);
        // The profile line: 2 m tall, centred on the shell's profile, hidden where it lies on the next one in.
        const la = gap > 0.4 ? lineA : 0;
        linePos.set([x, bot + 1.6, z + 0.2, x, bot - 1.6, z + 0.2], v * 3);
        lineCol.set([c.r, c.g, c.b, la, c.r, c.g, c.b, la], v * 4);
      }
    }
    for (const g of [bands, lines]) {
      (g.getAttribute("position") as THREE.BufferAttribute).needsUpdate = true;
      (g.getAttribute("color") as THREE.BufferAttribute).needsUpdate = true;
    }
  };

  // Dimension lines, a little in front of the face.
  const dimGeo = new THREE.BufferGeometry();
  const dimPos = new Float32Array(64 * 3);
  dimGeo.setAttribute("position", new THREE.BufferAttribute(dimPos, 3).setUsage(THREE.DynamicDrawUsage));
  dimGeo.boundingSphere = new THREE.Sphere(new THREE.Vector3(0, -120, CUT_Z0), 700);
  const dimLines = new THREE.LineSegments(dimGeo, new THREE.LineBasicMaterial({ color: "#f4efe6", transparent: true, opacity: 0.92, fog: true }));
  dimLines.renderOrder = 5;
  dimLines.visible = false;
  group.add(dimLines);

  return {
    group,
    dims(d) {
      if (!d) {
        dimLines.visible = false;
        return;
      }
      const zz = CUT_Z0 + 1.6;
      const v: number[] = [];
      const seg = (x0: number, y0: number, x1: number, y1: number) => v.push(x0, y0, zz, x1, y1, zz);
      const yd = Math.max(d.ya, d.yz) + 34;
      // Crest to crest: extension lines up from each crest, the dimension, and ticks at its ends.
      seg(d.xa, d.ya + 3, d.xa, yd + 6);
      seg(d.xz, d.yz + 3, d.xz, yd + 6);
      seg(d.xa, yd, d.xz, yd);
      seg(d.xa - 4, yd - 4, d.xa + 4, yd + 4);
      seg(d.xz - 4, yd - 4, d.xz + 4, yd + 4);
      // Depth: from the surface beside the wall down to the floor, with an extension along the floor.
      const top = Math.max(d.ya, d.yz);
      seg(d.xd, top, d.xd, d.yFloor);
      seg(d.xDeep, d.yFloor, d.xd + Math.sign(d.xd - d.xDeep) * 6, d.yFloor);
      seg(d.xd - 4, top - 4, d.xd + 4, top + 4);
      seg(d.xd - 4, d.yFloor - 4, d.xd + 4, d.yFloor + 4);
      dimPos.fill(0);
      dimPos.set(v.slice(0, dimPos.length));
      dimGeo.setDrawRange(0, v.length / 3);
      (dimGeo.getAttribute("position") as THREE.BufferAttribute).needsUpdate = true;
      dimLines.visible = true;
    },
    set(ground, shells, seconds, highlight) {
      lit = highlight;
      for (let s = 0; s <= S; s++) {
        from[s]!.set(now[s]!);
        to[s]!.set(s === 0 ? ground : (shells[s - 1] ?? ground));
      }
      if (fresh || seconds <= 0) {
        for (let s = 0; s <= S; s++) now[s]!.set(to[s]!);
        fresh = false;
        t = 1;
        draw();
        return;
      }
      t = 0;
      span = seconds;
    },
    update(dt) {
      if (t >= 1) return false;
      t = Math.min(1, t + dt / span);
      const e = ease(t);
      for (let s = 0; s <= S; s++) {
        const a = from[s]!;
        const b = to[s]!;
        const n = now[s]!;
        for (let p = 0; p < P; p++) n[p] = a[p]! + (b[p]! - a[p]!) * e;
      }
      draw();
      return t < 1;
    },
  };
}
