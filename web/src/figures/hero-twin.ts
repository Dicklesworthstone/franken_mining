/**
 * What the opening scene's twin draws, from what hero-solve.ts computes:
 * - the drillholes, coloured by assay down each hole;
 * - the block model's ore still in the ground inside the re-solved pit;
 * - that pit's limit as bench strings, the way a planner's design shows it.
 * Where the ground hides them they show through it faintly, as in a mine planning package, and solidly where they
 * are in view. hero.ts fades each layer in and out by lens.
 */
import * as THREE from "three";
import { LineMaterial } from "three/examples/jsm/lines/LineMaterial.js";
import { LineSegments2 } from "three/examples/jsm/lines/LineSegments2.js";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";
import { gradeColour } from "./drill-body";
import type { Twin } from "./hero-solve";
import { clamp } from "./lib";
import { B, COLS, H, NX, NY, NZ, TOP, centreX, centreY, centreZ } from "./pit-model";
import type { Pit } from "./terrain";

export type Layer = "holes" | "blocks" | "limit";
export interface TwinView {
  group: THREE.Group;
  /** The result, once the engine has it; null if it could not run here. */
  readonly twin: Twin | null;
  readonly state: "idle" | "solving" | "ready" | "failed";
  /** Starts the solve; calls back when it lands (or fails). */
  start(done: (t: Twin | null) => void): void;
  /** Target strength of each layer, 0–1; layers ease towards it. */
  show(o: Partial<Record<Layer, number>>): void;
  /** Eases the layers; returns true while anything is still moving. */
  update(dt: number, width: number, height: number): boolean;
  /** A point at the heart of the ore left in the pit, for a label. */
  readonly heart: THREE.Vector3;
  /** The engine's estimate for the block holding a point: expected grade (g/t) and sd of ln(grade); null if none. */
  at(x: number, y: number, z: number): { g: number; sd: number } | null;
}

const LIMIT = new THREE.Color("#60c3f0");

/** A ghosted twin of a material: drawn over everything, faint. */
function pair<M extends THREE.Material>(front: M, ghost: M) {
  ghost.depthTest = false;
  ghost.depthWrite = false;
  ghost.transparent = true;
  front.transparent = true;
  return [front, ghost] as const;
}

/** Block faces with bright edges, from the instance colour, so a block model reads as blocks. */
function blockMaterial(ghost: boolean) {
  return new THREE.ShaderMaterial({
    uniforms: { uOpacity: { value: 0 } },
    transparent: true,
    depthTest: !ghost,
    depthWrite: false,
    vertexShader: /* glsl */ `
      varying vec2 vUv;
      varying vec3 vCol;
      varying float vShade;
      void main() {
        vUv = uv;
        #ifdef USE_INSTANCING_COLOR
          vCol = instanceColor;
        #else
          vCol = vec3(1.0);
        #endif
        vec3 n = normalize(mat3(instanceMatrix) * normal);
        vShade = 0.72 + 0.28 * max(n.y, 0.0) + 0.12 * n.x;
        gl_Position = projectionMatrix * modelViewMatrix * instanceMatrix * vec4(position, 1.0);
      }`,
    fragmentShader: /* glsl */ `
      uniform float uOpacity;
      varying vec2 vUv;
      varying vec3 vCol;
      varying float vShade;
      void main() {
        vec2 e = min(vUv, 1.0 - vUv);
        float edge = 1.0 - smoothstep(0.02, 0.09, min(e.x, e.y));
        vec3 c = vCol * vShade;
        gl_FragColor = vec4(mix(c, c * 1.35 + 0.08, edge), uOpacity * (${ghost ? "0.38 + 0.62 * edge" : "0.82 + 0.18 * edge"}));
        #include <tonemapping_fragment>
        #include <colorspace_fragment>
      }`,
  });
}

export function twinView(pit: Pit, solve: (ground: Float32Array) => Promise<Twin>): TwinView {
  const group = new THREE.Group();
  group.name = "twin";
  const want: Record<Layer, number> = { holes: 0, blocks: 0, limit: 0 };
  const now: Record<Layer, number> = { holes: 0, blocks: 0, limit: 0 };
  const parts: Record<Layer, THREE.Object3D[]> = { holes: [], blocks: [], limit: [] };
  const setters: Record<Layer, ((v: number) => void)[]> = { holes: [], blocks: [], limit: [] };
  const lineMats: LineMaterial[] = [];
  let twin: Twin | null = null;
  let state: TwinView["state"] = "idle";
  const heart = new THREE.Vector3(0, pit.floor.y - 40, 0);

  /** The ground today over each column of the block grid. */
  const ground = () => {
    const g = new Float32Array(COLS);
    for (let j = 0; j < NZ; j++) for (let i = 0; i < NX; i++) g[i + NX * j] = pit.heightAt(centreX(i), centreZ(j));
    return g;
  };

  /* ---------- Drillholes: rods, one per 10 m composite; barren core pale and thin, mineralised core in grade colours ---------- */
  const drawHoles = (t: Twin) => {
    const segs: { a: THREE.Vector3; b: THREE.Vector3; g: number }[] = [];
    const BARREN = 0.18;
    for (const h of t.holes) {
      const [cx, cy, cz] = h.collar;
      const [dx, dy, dz] = h.dir;
      for (let s = 0; s < h.g.length; s++) {
        const a = new THREE.Vector3(cx + dx * s * 10, cy + dy * s * 10, cz + dz * s * 10);
        const b = new THREE.Vector3(cx + dx * (s + 1) * 10, cy + dy * (s + 1) * 10, cz + dz * (s + 1) * 10);
        // The stretch already mined out is gone with the rock; only the hole still in the ground is drawn.
        if (b.y > pit.heightAt(b.x, b.z) - 0.5) continue;
        if (a.y > pit.heightAt(a.x, a.z) - 0.5) a.lerp(b, 0.5);
        segs.push({ a, b, g: h.g[s]! });
      }
    }
    const geo = new THREE.CylinderGeometry(1, 1, 1, 6, 1, true);
    geo.translate(0, 0.5, 0);
    const up = new THREE.Vector3(0, 1, 0);
    const m4 = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const dir = new THREE.Vector3();
    const c = new THREE.Color();
    const pale = new THREE.Color("#d9d2c4");
    for (const ore of [false, true]) {
      const list = segs.filter((s) => (s.g >= BARREN) === ore);
      const mats = pair(new THREE.MeshBasicMaterial({ opacity: 0, fog: false }), new THREE.MeshBasicMaterial({ opacity: 0, fog: false }));
      mats.forEach((mat, ghost) => {
        const mesh = new THREE.InstancedMesh(geo, mat, Math.max(1, list.length));
        mesh.count = list.length;
        list.forEach((s, i) => {
          dir.subVectors(s.b, s.a);
          const len = dir.length();
          q.setFromUnitVectors(up, dir.normalize());
          // Higher grades are drawn thicker, as a strip log does it.
          const r = ore ? 1.6 + 1.6 * clamp(Math.log(s.g / BARREN) / Math.log(20), 0, 1) : 0.7;
          m4.compose(s.a, q, new THREE.Vector3(r, len, r));
          mesh.setMatrixAt(i, m4);
          mesh.setColorAt(i, ore ? gradeColour(s.g, c) : pale);
        });
        mesh.renderOrder = ghost ? 4 : 5;
        mesh.frustumCulled = false;
        group.add(mesh);
        parts.holes.push(mesh);
        const k = ore ? (ghost ? 0.6 : 1) : ghost ? 0.12 : 0.45;
        setters.holes.push((v) => (mat.opacity = v * k));
      });
    }
  };

  /* ---------- The ore still in the ground inside the re-solved pit ---------- */
  const drawBlocks = (t: Twin) => {
    const list: number[] = [];
    let sx = 0;
    let sy = 0;
    let sz = 0;
    let w = 0;
    for (let b = 0; b < t.grade.length; b++) {
      if (!t.inPit[b] || t.air[b] || !t.ore[b]) continue;
      list.push(b);
      const i = b % NX;
      const j = Math.floor(b / NX) % NZ;
      const k = Math.floor(b / COLS);
      const g = t.grade[b]!;
      sx += centreX(i) * g;
      sy += centreY(k) * g;
      sz += centreZ(j) * g;
      w += g;
    }
    if (w > 0) heart.set(sx / w, sy / w, sz / w);
    const geo = new THREE.BoxGeometry(B * 0.94, H * 0.94, B * 0.94);
    const m4 = new THREE.Matrix4();
    const c = new THREE.Color();
    [false, true].forEach((ghost) => {
      const mat = blockMaterial(ghost);
      const mesh = new THREE.InstancedMesh(geo, mat, Math.max(1, list.length));
      mesh.count = list.length;
      list.forEach((b, n) => {
        const i = b % NX;
        const j = Math.floor(b / NX) % NZ;
        const k = Math.floor(b / COLS);
        m4.makeTranslation(centreX(i), centreY(k), centreZ(j));
        mesh.setMatrixAt(n, m4);
        mesh.setColorAt(n, gradeColour(t.grade[b]!, c));
      });
      mesh.renderOrder = ghost ? 3 : 5;
      mesh.frustumCulled = false;
      group.add(mesh);
      parts.blocks.push(mesh);
      setters.blocks.push((v) => (mat.uniforms.uOpacity!.value = v * (ghost ? 0.34 : 0.95)));
    });
  };

  /* ---------- The pit limit, bench by bench: marching squares on each column's depth ---------- */
  const drawLimit = (t: Twin) => {
    const depth = new Float32Array(COLS);
    for (let c = 0; c < COLS; c++) {
      let k = 0;
      while (k < NY && t.inPit[c + k * COLS]) k++;
      depth[c] = k;
    }
    const at = (i: number, j: number) => (i < 0 || j < 0 || i >= NX || j >= NZ ? 0 : depth[i + NX * j]!);
    const pts: number[] = [];
    const cols: number[] = [];
    const c = new THREE.Color();
    for (let L = 1; L <= NY; L++) {
      const th = L - 0.5;
      const y = TOP - L * H;
      // Deeper benches are drawn a little brighter, so the stack of strings reads as a hole going down.
      c.copy(LIMIT).multiplyScalar(0.75 + 0.35 * (L / NY));
      const edge = (i0: number, j0: number, i1: number, j1: number) => {
        const a = at(i0, j0);
        const b = at(i1, j1);
        const f = (th - a) / (b - a);
        return [centreX(0) + (i0 + (i1 - i0) * f) * B, centreZ(0) + (j0 + (j1 - j0) * f) * B] as const;
      };
      for (let j = -1; j < NZ; j++) {
        for (let i = -1; i < NX; i++) {
          const v0 = at(i, j) >= th ? 1 : 0;
          const v1 = at(i + 1, j) >= th ? 2 : 0;
          const v2 = at(i + 1, j + 1) >= th ? 4 : 0;
          const v3 = at(i, j + 1) >= th ? 8 : 0;
          const code = v0 | v1 | v2 | v3;
          if (code === 0 || code === 15) continue;
          const E = [
            () => edge(i, j, i + 1, j),
            () => edge(i + 1, j, i + 1, j + 1),
            () => edge(i, j + 1, i + 1, j + 1),
            () => edge(i, j, i, j + 1),
          ];
          const SEG: Record<number, [number, number][]> = {
            1: [[0, 3]], 2: [[0, 1]], 3: [[1, 3]], 4: [[1, 2]], 5: [[0, 1], [2, 3]], 6: [[0, 2]], 7: [[2, 3]],
            8: [[2, 3]], 9: [[0, 2]], 10: [[0, 3], [1, 2]], 11: [[1, 2]], 12: [[1, 3]], 13: [[0, 1]], 14: [[0, 3]],
          };
          for (const [e0, e1] of SEG[code]!) {
            const [x0, z0] = E[e0]!();
            const [x1, z1] = E[e1]!();
            // Only where the bench would cut rock: inside today's void the limit has already been passed.
            if (y > pit.heightAt(x0, z0) - 1 || y > pit.heightAt(x1, z1) - 1) continue;
            pts.push(x0, y, z0, x1, y, z1);
            cols.push(c.r, c.g, c.b, c.r, c.g, c.b);
          }
        }
      }
    }
    if (!pts.length) return;
    const geo = new LineSegmentsGeometry();
    geo.setPositions(pts);
    geo.setColors(cols);
    [false, true].forEach((ghost) => {
      const mat = new LineMaterial({ vertexColors: true, linewidth: ghost ? 1.2 : 1.8, transparent: true, opacity: 0, depthTest: !ghost, depthWrite: false });
      lineMats.push(mat);
      const line = new LineSegments2(geo, mat);
      line.renderOrder = ghost ? 4 : 6;
      line.frustumCulled = false;
      group.add(line);
      parts.limit.push(line);
      setters.limit.push((v) => (mat.opacity = v * (ghost ? 0.5 : 1)));
    });
  };

  const apply = () => {
    for (const k of Object.keys(now) as Layer[]) {
      for (const s of setters[k]) s(now[k]);
      for (const p of parts[k]) p.visible = now[k] > 0.004;
    }
  };

  return {
    group,
    heart,
    get twin() {
      return twin;
    },
    get state() {
      return state;
    },
    at(x, y, z) {
      if (!twin) return null;
      const i = Math.floor((x - (centreX(0) - B / 2)) / B);
      const j = Math.floor((z - (centreZ(0) - B / 2)) / B);
      const k = Math.floor((TOP - y) / H);
      if (i < 0 || j < 0 || k < 0 || i >= NX || j >= NZ || k >= NY) return null;
      const b = i + NX * (j + NZ * k);
      return { g: twin.grade[b]!, sd: twin.sd[b]! };
    },
    start(done) {
      if (state !== "idle") return;
      state = "solving";
      solve(ground())
        .then((t) => {
          twin = t;
          drawHoles(t);
          drawBlocks(t);
          drawLimit(t);
          apply();
          state = "ready";
          done(t);
        })
        .catch(() => {
          state = "failed";
          done(null);
        });
    },
    show(o) {
      Object.assign(want, o);
    },
    update(dt, width, height) {
      let moving = false;
      const k = 1 - Math.exp(-dt * 5);
      for (const key of Object.keys(now) as Layer[]) {
        const d = want[key] - now[key];
        if (Math.abs(d) > 0.002) {
          now[key] += d * k;
          moving = true;
        } else now[key] = want[key];
      }
      for (const m of lineMats) m.resolution.set(width, height);
      apply();
      return moving;
    },
  };
}
