/**
 * The engineering plan: the digital-twin reading of the underground figure. One level seen from above, drawn the way
 * a mine planner's level plan is: the as-built wall (a section of the excavated network 1.5 m above the floor)
 * against the design outline, the stopes coloured by the engine's estimated grade (paste-filled ones hatched, planned
 * ones dashed), the ventilation network with the airflow each airway carries against what its diesels need, a scale
 * bar and a north arrow. While it is on, the 3D mine is cut to a slab around the level and dimmed underneath the
 * drawing, so the plan reads as a section through the model rather than a picture laid on top of it.
 */
import * as THREE from "three";
import { reduced } from "./lib";
import type { Stage } from "./stage";
import {
  AIR,
  AIR_FLOOR,
  AIR_PER_KW,
  airNeed,
  DRIVE_AREA,
  DUCT,
  FAN,
  LEVELS,
  OREPASS,
  orepassZ,
  RAR,
  SHAFT,
  STOPES,
  zFW,
  zHW,
  type Level,
  type Stope,
} from "./underground-plan";
import type { World } from "./underground-world";

/** A stope's grade from the engine: mean and 80% interval, g/t. */
export interface Grade {
  mean: number;
  lo: number;
  hi: number;
}
export interface TwinInput {
  s: Stage;
  /** The figure's `.scene-view`: the twin puts its own panel in here. */
  view: HTMLElement;
  world: World;
  small: boolean;
  /** The engine's grade for a stope id, once computed (undefined before). */
  grade: (id: string) => Grade | undefined;
}
export interface Twin {
  /** Overlay geometry; the caller adds it to the scene. Hidden while off. */
  group: THREE.Group;
  readonly on: boolean;
  level: Level;
  /** Turns the plan on or off (geometry, the slab cut and dimming of the 3D mine, and the twin's panel). */
  set(on: boolean): void;
  /** Where the camera goes for the plan of the current level: centred in the view's free area, uncropped. */
  view(): { target: [number, number, number]; distance: number; azimuth: number; elevation: number };
  update(dt: number, t: number): void;
  /** Grades have changed (the engine answered): recolour. */
  refresh(): void;
}

/* ---------- Drawing conventions ---------- */

/** The plan's section height above the level floor, m (where the as-built wall is cut). */
const SECTION = 1.5;
/** The 3D mine is cut to this slab around the level while the plan is on. */
const CUT_TOP = 4.2;
const CUT_BOTTOM = 3;
/** Chevrons on the airways run at this multiple of the true air speed, so the slow drives still visibly move. */
const TIME_SCALE = 6;
/** Grade ramp, g/t, in sRGB: dark ochre (poor) to pale gold (rich). */
const RAMP: [number, string][] = [
  [3.6, "#6e3a1c"],
  [4.2, "#a9561f"],
  [4.8, "#de8a2d"],
  [5.4, "#f4c25b"],
  [6.0, "#fff0b8"],
];
const INK = {
  asBuilt: "#f3e9d8",
  design: "#9fb1ff",
  air: "#55dcc6",
  duct: "#ffb547",
  low: "#fb7185",
  outline: "#fff4dc",
  pending: "#8a8178",
};
const NB = " ";

/** sRGB components of a CSS hex colour (the plan's shaders write sRGB straight to the canvas). */
const srgb = (hex: string) => new THREE.Color(hex).convertLinearToSRGB();
function rampColour(g: number) {
  const out = new THREE.Color();
  if (g <= RAMP[0]![0]) return srgb(RAMP[0]![1]);
  for (let k = 1; k < RAMP.length; k++) {
    const [g1, c1] = RAMP[k]!;
    if (g <= g1) {
      const [g0, c0] = RAMP[k - 1]!;
      return out.copy(srgb(c0)).lerp(srgb(c1), (g - g0) / (g1 - g0));
    }
  }
  return srgb(RAMP[RAMP.length - 1]![1]);
}
const f1 = (v: number) => v.toFixed(1);
const f2 = (v: number) => v.toFixed(2);
const lvl = (L: number) => String(-L).padStart(3, "0");
const signed = (v: number) => `${v >= 0 ? "+" : "−"}${Math.abs(v).toFixed(2)}${NB}m`;

/* ---------- Ink: every stroke of the drawing as screen-width ribbons in one mesh ---------- */

interface Stroke {
  /** Width in CSS pixels. */
  w: number;
  c: THREE.Color;
  a: number;
  /** Dash period (m) and the drawn fraction of it; 0 for a solid line. */
  dash?: number;
  fill?: number;
  /** Chevrons: period (m) and their speed along the stroke (m/s on the plan). */
  chevron?: number;
  speed?: number;
}

class Ink {
  pos: number[] = [];
  nrm: number[] = [];
  side: number[] = [];
  wid: number[] = [];
  col: number[] = [];
  dist: number[] = [];
  sty: number[] = [];
  idx: number[] = [];
  private vert(x: number, y: number, z: number, nx: number, nz: number, s: number, d: number, o: Stroke) {
    this.pos.push(x, y, z);
    this.nrm.push(nx, nz);
    this.side.push(s);
    this.wid.push(o.w);
    this.col.push(o.c.r, o.c.g, o.c.b, o.a);
    this.dist.push(d);
    this.sty.push(o.chevron ?? o.dash ?? 0, o.fill ?? 1, o.speed ?? 0, o.chevron ? 1 : 0);
  }
  /** A polyline in plan (x, z) at height y, with mitred joins. */
  line(pts: { x: number; z: number }[], y: number, o: Stroke, d0 = 0) {
    const n = pts.length;
    if (n < 2) return;
    const base = this.pos.length / 3;
    let d = d0;
    for (let k = 0; k < n; k++) {
      const p = pts[k]!;
      const a = pts[Math.max(0, k - 1)]!;
      const b = pts[Math.min(n - 1, k + 1)]!;
      // Normals of the segments either side; the mitre is their bisector, lengthened (capped) to keep the width.
      let ax = p.x - a.x;
      let az = p.z - a.z;
      let bx = b.x - p.x;
      let bz = b.z - p.z;
      const la = Math.hypot(ax, az) || 1;
      const lb = Math.hypot(bx, bz) || 1;
      if (k === 0) {
        ax = bx;
        az = bz;
      }
      if (k === n - 1) {
        bx = ax;
        bz = az;
      }
      const n1x = -az / (k === 0 ? lb : la);
      const n1z = ax / (k === 0 ? lb : la);
      const n2x = -bz / (k === n - 1 ? la : lb);
      const n2z = bx / (k === n - 1 ? la : lb);
      let mx = n1x + n2x;
      let mz = n1z + n2z;
      const ml = Math.hypot(mx, mz) || 1;
      mx /= ml;
      mz /= ml;
      const cos = Math.max(0.5, mx * n1x + mz * n1z);
      if (k > 0) d += la;
      this.vert(p.x, y, p.z, mx / cos, mz / cos, -1, d, o);
      this.vert(p.x, y, p.z, mx / cos, mz / cos, 1, d, o);
    }
    for (let k = 0; k < n - 1; k++) {
      const i = base + k * 2;
      this.idx.push(i, i + 1, i + 2, i + 1, i + 3, i + 2);
    }
  }
  mesh(mat: THREE.Material) {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(this.pos, 3));
    g.setAttribute("aN", new THREE.Float32BufferAttribute(this.nrm, 2));
    g.setAttribute("aSide", new THREE.Float32BufferAttribute(this.side, 1));
    g.setAttribute("aW", new THREE.Float32BufferAttribute(this.wid, 1));
    g.setAttribute("aCol", new THREE.Float32BufferAttribute(this.col, 4));
    g.setAttribute("aDist", new THREE.Float32BufferAttribute(this.dist, 1));
    g.setAttribute("aSty", new THREE.Float32BufferAttribute(this.sty, 4));
    g.setIndex(this.idx);
    const m = new THREE.Mesh(g, mat);
    m.frustumCulled = false;
    m.renderOrder = 22;
    return m;
  }
}

function inkMaterial(u: { uPx: { value: number }; uTime: { value: number } }) {
  return new THREE.ShaderMaterial({
    uniforms: { uPx: u.uPx, uTime: u.uTime },
    vertexShader: /* glsl */ `
      attribute vec2 aN;
      attribute float aSide;
      attribute float aW;
      attribute vec4 aCol;
      attribute float aDist;
      attribute vec4 aSty;
      uniform float uPx;
      varying vec4 vCol;
      varying float vSide;
      varying float vDist;
      varying vec4 vSty;
      varying float vW;
      void main() {
        vec3 p = position;
        p.xz += aN * aSide * (aW + 1.0) * 0.5 * uPx;
        vCol = aCol;
        vSide = aSide;
        vDist = aDist;
        vSty = aSty;
        vW = aW + 1.0;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(p, 1.0);
      }`,
    fragmentShader: /* glsl */ `
      uniform float uTime;
      varying vec4 vCol;
      varying float vSide;
      varying float vDist;
      varying vec4 vSty;
      varying float vW;
      void main() {
        float a = vCol.a;
        // Antialiased edges: the ribbon is drawn a pixel wider and fades over its outer half-pixels.
        float edge = (1.0 - abs(vSide)) * vW * 0.5;
        a *= clamp(edge, 0.0, 1.0);
        float u = vDist - uTime * vSty.z;
        if (vSty.w > 0.5) {
          // Chevrons pointing along the airway over a faint band: the wings trail the tip.
          float s = u / vSty.x + abs(vSide) * 0.3;
          float f = fract(s);
          float fw = max(fwidth(s), 1e-4);
          float on = smoothstep(0.0, fw, f) * (1.0 - smoothstep(0.2 - fw, 0.2, f));
          a *= mix(0.2, 1.0, on);
        } else if (vSty.x > 0.0) {
          if (fract(u / vSty.x) > vSty.y) discard;
        }
        if (a < 0.004) discard;
        gl_FragColor = vec4(vCol.rgb, a);
      }`,
    transparent: true,
    side: THREE.DoubleSide,
    depthTest: false,
    depthWrite: false,
  });
}

function fillMaterial() {
  return new THREE.ShaderMaterial({
    vertexShader: /* glsl */ `
      attribute vec4 aCol;
      attribute float aHatch;
      varying vec4 vCol;
      varying float vHatch;
      varying vec2 vXZ;
      void main() {
        vCol = aCol;
        vHatch = aHatch;
        vXZ = position.xz;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }`,
    fragmentShader: /* glsl */ `
      varying vec4 vCol;
      varying float vHatch;
      varying vec2 vXZ;
      void main() {
        vec3 c = vCol.rgb;
        float a = vCol.a;
        if (vHatch > 0.5) {
          // Paste fill: the drafting convention, diagonal hatching every 2.4 m.
          float s = (vXZ.x + vXZ.y) / 2.4;
          float f = abs(fract(s) - 0.5);
          float fw = max(fwidth(s), 1e-4);
          float h = 1.0 - smoothstep(0.08 - fw, 0.08 + fw, f);
          c = mix(c, c * 0.25, h * 0.85);
          a = mix(a, min(1.0, a + 0.25), h);
        }
        gl_FragColor = vec4(c, a);
      }`,
    transparent: true,
    side: THREE.DoubleSide,
    depthTest: false,
    depthWrite: false,
  });
}

/* ---------- The as-built section: the excavated network cut by a horizontal plane ---------- */

/** Segments (x0, z0, x1, z1) where the plane y = y0 cuts the triangles of a mesh. */
function slice(geo: THREE.BufferGeometry, y0: number): number[] {
  const P = geo.getAttribute("position");
  const I = geo.getIndex();
  const out: number[] = [];
  const n = I ? I.count : P.count;
  const vi = (k: number) => (I ? I.getX(k) : k);
  const xs = [0, 0, 0];
  const ys = [0, 0, 0];
  const zs = [0, 0, 0];
  const y = y0 + 1e-4;
  for (let t = 0; t < n; t += 3) {
    let above = 0;
    for (let c = 0; c < 3; c++) {
      const v = vi(t + c);
      ys[c] = P.getY(v);
      if (ys[c]! > y) above++;
    }
    if (above === 0 || above === 3) continue;
    for (let c = 0; c < 3; c++) {
      const v = vi(t + c);
      xs[c] = P.getX(v);
      zs[c] = P.getZ(v);
    }
    let m = 0;
    for (let c = 0; c < 3 && m < 2; c++) {
      const d = (c + 1) % 3;
      const ya = ys[c]!;
      const yb = ys[d]!;
      if (ya > y === yb > y) continue;
      const f = (y - ya) / (yb - ya);
      out.push(xs[c]! + (xs[d]! - xs[c]!) * f, zs[c]! + (zs[d]! - zs[c]!) * f);
      m++;
    }
    if (m !== 2) out.length -= m * 2;
  }
  return out;
}

/** Distance from (x, z) to a polyline in plan, and the parameter of the nearest point along it (m from its start). */
function nearest(pts: { x: number; z: number }[], x: number, z: number) {
  let best = Infinity;
  let along = 0;
  let total = 0;
  let at = 0;
  for (let k = 0; k < pts.length - 1; k++) {
    const a = pts[k]!;
    const b = pts[k + 1]!;
    const dx = b.x - a.x;
    const dz = b.z - a.z;
    const l2 = dx * dx + dz * dz || 1;
    const f = Math.max(0, Math.min(1, ((x - a.x) * dx + (z - a.z) * dz) / l2));
    const d = Math.hypot(a.x + dx * f - x, a.z + dz * f - z);
    const l = Math.sqrt(l2);
    if (d < best) {
      best = d;
      at = total + f * l;
    }
    total += l;
  }
  along = at;
  return { d: best, along, total };
}

/* ---------- One level's drawing ---------- */

/** An annotation on the drawing: centred on its point, placed by priority, never over another or off the view. */
interface Tag {
  el: HTMLElement;
  p: THREE.Vector3;
  prio: number;
  stope?: Stope;
  w: number;
  h: number;
  x: number;
  y: number;
  vis: boolean;
}

interface Built {
  group: THREE.Group;
  fill: THREE.Mesh | null;
  fillCol: THREE.BufferAttribute | null;
  /** Stope fills: vertex ranges per stope, to recolour when grades arrive. */
  stopeVerts: { s: Stope; from: number; to: number }[];
  labels: Tag[];
  ext: { x0: number; x1: number; z0: number; z1: number };
  dev: { mean: number; abs: number; over: number; under: number; n: number };
}

export function makeTwin(i: TwinInput): Twin {
  const { s, world, small } = i;
  const group = new THREE.Group();
  group.visible = false;
  group.renderOrder = 20;
  const uPx = { value: 0.3 };
  const uTime = { value: 0 };
  const inkMat = inkMaterial({ uPx, uTime });
  const fillMat = fillMaterial();
  /** Plans are drawn north-up on a wide view; on a phone the long east-west axis runs down the screen (north right). */
  const AZ = small ? 90 : 0;
  const ELEVATION = 88.5;

  // Darkens the 3D mine under the drawing: a full-screen quad drawn after the scene, before the ink.
  const dim = new THREE.Mesh(
    new THREE.PlaneGeometry(2, 2),
    new THREE.ShaderMaterial({
      uniforms: { uA: { value: 0.5 } },
      vertexShader: "void main() { gl_Position = vec4(position.xy, 0.0, 1.0); }",
      fragmentShader: "uniform float uA; void main() { gl_FragColor = vec4(0.03, 0.026, 0.024, uA); }",
      transparent: true,
      depthTest: false,
      depthWrite: false,
    }),
  );
  dim.frustumCulled = false;
  dim.renderOrder = 18;
  group.add(dim);

  /* ---------- Panel, scale bar and north arrow ---------- */
  const panel = document.createElement("div");
  panel.className = "hud-card tw-panel";
  panel.setAttribute("aria-live", "polite");
  // On a phone the panel sits under the view, so the plan has all of it.
  if (small) i.view.after(panel);
  else i.view.append(panel);
  const scale = document.createElement("div");
  scale.className = "tw-scale";
  scale.setAttribute("aria-hidden", "true");
  scale.innerHTML = `<svg class="tw-north" viewBox="-12 -14 24 28" width="22" height="26"><path d="M0 -12 L6 6 L0 2 L-6 6 Z" fill="currentColor"/><text x="0" y="13.5" text-anchor="middle">N</text></svg><div class="tw-bar"><i></i><span>50 m</span></div>`;
  i.view.append(scale);
  const synth = i.view.querySelector<HTMLElement>(".ug-synth");
  const hint = i.view.querySelector<HTMLElement>(".ug-hint");
  const tagLayer = document.createElement("div");
  tagLayer.className = "tw-labels";
  tagLayer.setAttribute("aria-hidden", "true");
  i.view.append(tagLayer);
  const north = scale.querySelector<SVGElement>(".tw-north")!;
  const bar = scale.querySelector<HTMLElement>(".tw-bar i")!;
  const barText = scale.querySelector<HTMLElement>(".tw-bar span")!;

  /* ---------- Building a level ---------- */
  const built = new Map<number, Built>();
  const lineList = () => [...world.net.lines.values()];

  function buildLevel(L: Level): Built {
    const y0 = L + SECTION;
    const g = new THREE.Group();
    const ink = new Ink();
    const yInk = L + CUT_TOP + 0.05;
    const ext = { x0: Infinity, x1: -Infinity, z0: Infinity, z1: -Infinity };
    const grow = (x: number, z: number) => {
      ext.x0 = Math.min(ext.x0, x);
      ext.x1 = Math.max(ext.x1, x);
      ext.z0 = Math.min(ext.z0, z);
      ext.z1 = Math.max(ext.z1, z);
    };

    // Which excavations the section cuts: their floor is between the bottom of the arch and 0.3 m below the plane.
    const cut = lineList()
      .map((l) => {
        const hw = l.spec.h - 0.32 * l.spec.w;
        const inRange = (p: THREE.Vector3) => p.y <= y0 - 0.3 && p.y >= y0 - hw;
        const runs: THREE.Vector3[][] = [];
        let run: THREE.Vector3[] = [];
        for (const p of l.pts) {
          if (inRange(p)) run.push(p);
          else if (run.length) {
            runs.push(run);
            run = [];
          }
        }
        if (run.length) runs.push(run);
        return { l, runs: runs.filter((r) => r.length > 1) };
      })
      .filter((c) => c.runs.length);
    const insideOther = (x: number, z: number, self: string) =>
      cut.some((c) => c.l.spec.id !== self && c.runs.some((r) => nearest(r, x, z).d < c.l.spec.w / 2 - 0.08));

    // As-built wall: the network sliced at the section height.
    const segs = slice(world.net.geometry, y0);
    const asBuilt: Stroke = { w: 1.6, c: srgb(INK.asBuilt), a: 0.95 };
    for (let k = 0; k < segs.length; k += 4) {
      ink.line(
        [
          { x: segs[k]!, z: segs[k + 1]! },
          { x: segs[k + 2]!, z: segs[k + 3]! },
        ],
        yInk,
        asBuilt,
      );
      grow(segs[k]!, segs[k + 1]!);
    }

    // Design outline: each centreline offset by half the design width, trimmed where it runs inside another opening.
    const design: Stroke = { w: 1.2, c: srgb(INK.design), a: 0.95, dash: 2.2, fill: 0.58 };
    for (const { l, runs } of cut) {
      const hw = l.spec.w / 2;
      for (const run of runs) {
        for (const sgn of [-1, 1]) {
          let piece: { x: number; z: number }[] = [];
          let d0 = 0;
          let d = 0;
          const flush = () => {
            if (piece.length > 1) ink.line(piece, yInk, design, d0);
            piece = [];
          };
          for (let k = 0; k < run.length - 1; k++) {
            const a = run[k]!;
            const b = run[k + 1]!;
            const len = Math.hypot(b.x - a.x, b.z - a.z);
            const nx = -(b.z - a.z) / (len || 1);
            const nz = (b.x - a.x) / (len || 1);
            const steps = Math.max(1, Math.ceil(len / 0.5));
            for (let q = k === 0 ? 0 : 1; q <= steps; q++) {
              const f = q / steps;
              const x = a.x + (b.x - a.x) * f + nx * hw * sgn;
              const z = a.z + (b.z - a.z) * f + nz * hw * sgn;
              const dd = d + len * f;
              if (insideOther(x, z, l.spec.id)) flush();
              else {
                if (!piece.length) d0 = dd;
                piece.push({ x, z });
              }
            }
            d += len;
          }
          flush();
        }
        // A face or barricade at the end of the run, when the run reaches the excavation's end.
        const last = run[run.length - 1]!;
        if ((l.spec.end === "face" || l.spec.end === "barricade") && last === l.pts[l.pts.length - 1]) {
          const prev = run[run.length - 2]!;
          const len = Math.hypot(last.x - prev.x, last.z - prev.z) || 1;
          const nx = -(last.z - prev.z) / len;
          const nz = (last.x - prev.x) / len;
          ink.line(
            [
              { x: last.x + nx * hw, z: last.z + nz * hw },
              { x: last.x - nx * hw, z: last.z - nz * hw },
            ],
            yInk,
            design,
          );
        }
      }
    }

    // Wall against design: each as-built segment's distance from the nearest centreline, less the half width.
    let sum = 0;
    let abs = 0;
    let n = 0;
    let over = { v: -Infinity, x: 0, z: 0 };
    let under = { v: Infinity, x: 0, z: 0 };
    for (let k = 0; k < segs.length; k += 4) {
      const x = (segs[k]! + segs[k + 2]!) / 2;
      const z = (segs[k + 1]! + segs[k + 3]!) / 2;
      let best: { d: number; c: (typeof cut)[number]; along: number; total: number; run: THREE.Vector3[] } | null = null;
      for (const c of cut)
        for (const run of c.runs) {
          const r = nearest(run, x, z);
          if (!best || r.d < best.d) best = { d: r.d, c, along: r.along, total: r.total, run };
        }
      if (!best) continue;
      const dev = best.d - best.c.l.spec.w / 2;
      // Side walls only: away from the ends of the run and from junctions.
      if (Math.abs(dev) > 1.2 || best.along < 3 || best.along > best.total - 3) continue;
      if (cut.some((c) => c !== best!.c && c.runs.some((r) => nearest(r, x, z).d < c.l.spec.w / 2 + 2.5))) continue;
      sum += dev;
      abs += Math.abs(dev);
      n++;
      if (dev > over.v) over = { v: dev, x, z };
      if (dev < under.v) under = { v: dev, x, z };
    }

    // Stopes on the level, coloured by grade; paste-filled hatched, planned dashed.
    const fill = { pos: [] as number[], col: [] as number[], hatch: [] as number[], idx: [] as number[] };
    const stopeVerts: Built["stopeVerts"] = [];
    const yFill = L + CUT_TOP + 0.02;
    const stopes = STOPES.filter((st) => st.level === L);
    for (const st of stopes) {
      const from = fill.pos.length / 3;
      const xs: number[] = [];
      for (let x = st.xa; x < st.xb - 0.01; x += 1.5) xs.push(x);
      xs.push(st.xb);
      const hwE = xs.map((x) => ({ x, z: zHW(x, y0) }));
      const fwE = xs.map((x) => ({ x, z: zFW(x, y0) }));
      if (hwE.some((p, k) => !(fwE[k]!.z - p.z > 0.5))) continue;
      const hatched = st.state === "filled" || st.state === "curing";
      for (let k = 0; k < xs.length; k++) {
        for (const p of [hwE[k]!, fwE[k]!]) {
          fill.pos.push(p.x, yFill, p.z);
          fill.col.push(0, 0, 0, 0);
          fill.hatch.push(hatched ? 1 : 0);
          grow(p.x, p.z);
        }
        if (k < xs.length - 1) {
          const v = from + k * 2;
          fill.idx.push(v, v + 2, v + 1, v + 1, v + 2, v + 3);
        }
      }
      stopeVerts.push({ s: st, from, to: fill.pos.length / 3 });
      const outline: Stroke =
        st.state === "planned"
          ? { w: 1.3, c: srgb(INK.outline), a: 0.75, dash: 1.6, fill: 0.5 }
          : { w: 1.5, c: srgb(st.state === "fired" ? "#ffb070" : INK.outline), a: 0.9 };
      ink.line([...hwE, ...fwE.slice().reverse(), hwE[0]!], yInk, outline);
    }
    let fillMesh: THREE.Mesh | null = null;
    let fillCol: THREE.BufferAttribute | null = null;
    if (fill.idx.length) {
      const fg = new THREE.BufferGeometry();
      fg.setAttribute("position", new THREE.Float32BufferAttribute(fill.pos, 3));
      fillCol = new THREE.Float32BufferAttribute(fill.col, 4);
      fg.setAttribute("aCol", fillCol);
      fg.setAttribute("aHatch", new THREE.Float32BufferAttribute(fill.hatch, 1));
      fg.setIndex(fill.idx);
      fillMesh = new THREE.Mesh(fg, fillMat);
      fillMesh.frustumCulled = false;
      fillMesh.renderOrder = 20;
      g.add(fillMesh);
    }

    // Ventilation: the level's airway from the decline to the return raise, ducts to dead-end headings, the decline.
    const labels: Built["labels"] = [];
    const lab = (html: string, x: number, z: number, cls: string, stope?: Stope, prio = 2) => {
      const el = document.createElement("div");
      el.className = `tw-tag ${cls}`;
      el.innerHTML = html;
      el.hidden = true;
      tagLayer.append(el);
      labels.push({ el, p: new THREE.Vector3(x, yInk, z), prio, stope, w: 0, h: 0, x: NaN, y: NaN, vis: true });
      grow(x, z);
    };
    const air = AIR.find((a) => a.level === L);
    const lines = world.net.lines;
    const flat = (pts: THREE.Vector3[]) => pts.map((p) => ({ x: p.x, z: p.z }));
    const access = lines.get(`access${L}`);
    const drive = lines.get(`drive${L}`);
    const ax = access ? access.pts[0]!.x : -58;
    const airStroke = (q: number, area: number, colour: string, w: number): Stroke => ({
      w,
      c: srgb(colour),
      a: 0.92,
      chevron: 6,
      speed: reduced ? 0 : (q / area) * TIME_SCALE,
    });
    if (air && access && drive) {
      const low = air.measured < airNeed(air);
      const col = low ? INK.low : INK.air;
      const main = airStroke(air.measured, DRIVE_AREA, col, small ? 7 : 9);
      ink.line(flat(access.pts), yInk + 0.01, main);
      const east = drive.pts.filter((p) => p.x >= ax - 0.1);
      const reachesRaise = east.some((p) => Math.hypot(p.x - RAR.x, p.z - RAR.z) < 4);
      // Through-flow to the return raise, or (a dead-end level) the face's return air coming back along the drive.
      const path = reachesRaise ? east : air.duct ? east.slice().reverse() : east;
      const q = reachesRaise || !air.duct ? air.measured : air.duct.measured;
      ink.line(flat(path), yInk + 0.01, airStroke(q, DRIVE_AREA, col, small ? 7 : 9));
      const mid = east[Math.floor(east.length * 0.62)] ?? east[0]!;
      lab(
        `<b>${f1(q)}${NB}m³/s</b><span>${f2(q / DRIVE_AREA)}${NB}m/s${reachesRaise ? "" : " · face return"}</span>`,
        mid.x,
        mid.z + 9,
        low ? "tw-air tw-low" : "tw-air",
      );
      if (air.duct) {
        // A forcing fan in the drive pushing fresh air down a 1.4 m duct to the heading.
        const target = air.duct.to.startsWith("XC") ? lines.get(`xc${L}-${air.duct.to.match(/XC-0?(\d+)/)?.[1] ?? ""}`) : null;
        let duct: { x: number; z: number }[] = [];
        if (target) {
          const a = target.pts[0]!;
          const b = target.pts[target.pts.length - 1]!;
          const side = 1.6;
          duct = [
            { x: a.x - 15, z: a.z - 1.6 },
            { x: a.x - side, z: a.z - 1.6 },
            { x: b.x - side, z: b.z + 3 * Math.sign(a.z - b.z) },
          ];
        } else {
          const end = east[east.length - 1]!;
          duct = [
            { x: ax + 8, z: end.z - 1.6 },
            { x: end.x - 3, z: end.z - 1.6 },
          ];
        }
        ink.line(duct, yInk + 0.02, { w: 1.6, c: srgb(INK.duct), a: 0.95 });
        ink.line(duct, yInk + 0.02, airStroke(air.duct.measured, DUCT.area, INK.duct, small ? 5 : 7));
        const d0 = duct[0]!;
        lab(
          `<b>Duct ${f1(air.duct.measured)}${NB}m³/s</b><span>${f1(air.duct.measured / DUCT.area)}${NB}m/s in ${DUCT.diameter}${NB}m</span>`,
          d0.x + 4,
          d0.z + 9,
          "tw-duct",
        );
      }
    }
    // The decline through the slab: intake for this level and everything below it, then for the levels below.
    const dec = lines.get("decline");
    if (dec) {
      const slab = dec.pts.filter((p) => p.y <= L + CUT_TOP && p.y >= L - CUT_BOTTOM);
      if (slab.length > 1) {
        const decArea = DECLINE_AREA(dec.spec.w, dec.spec.h);
        const below = (lev: number) => AIR.filter((a) => a.level <= lev).reduce((t, a) => t + a.measured, 0);
        const split = slab.findIndex((p) => p.y < L + 0.05);
        const upper = split > 0 ? slab.slice(0, split + 1) : slab;
        const lower = split > 0 ? slab.slice(split) : [];
        const qU = below(L);
        const qL = below(L - 1);
        ink.line(flat(upper), yInk, airStroke(qU, decArea, INK.air, small ? 7 : 9));
        if (lower.length > 1 && qL > 0) ink.line(flat(lower), yInk, airStroke(qL, decArea, INK.air, small ? 6 : 8));
        const m = upper[Math.floor(upper.length * 0.35)]!;
        lab(`<b>Decline ${f1(qU)}${NB}m³/s</b><span>${f2(qU / decArea)}${NB}m/s down</span>`, m.x, m.z + 10, "tw-air");
      }
    }
    // Raises: the return raise to the main fan, the shaft, the ore pass.
    if (L >= RAR.bottom) lab(`<b>Return raise</b><span>to the fan: ${f1(FAN.measured)}${NB}m³/s, ${FAN.kPa}${NB}kPa</span>`, RAR.x - 14, RAR.z - 11, "tw-raise", undefined, 3);
    if (L >= SHAFT.bottom) lab(`<b>Shaft</b>`, SHAFT.x - 4, SHAFT.z - 8, "tw-raise", undefined, 3);
    if (L <= OREPASS.top && L >= OREPASS.bottom) lab(`<b>Ore pass</b>`, OREPASS.x - 8, orepassZ(y0) - 6, "tw-raise", undefined, 3);
    // Where the wall is furthest outside and inside the design.
    if (n > 0) {
      if (over.v > 0.05) lab(`overbreak <b>${signed(over.v)}</b>`, over.x, over.z + 7, "tw-dev", undefined, 4);
      if (under.v < -0.05) lab(`underbreak <b>${signed(under.v)}</b>`, under.x, under.z + 7, "tw-dev", undefined, 4);
    }
    // Stope labels in two rows north of the stopes, so neighbours do not collide.
    for (const st of stopes) {
      const xm = (st.xa + st.xb) / 2;
      // A phone stacks one compact row (its stopes are a label's height apart); a wide view alternates two rows.
      const row = small ? 0 : st.k % 2 ? 0 : 1;
      lab("", xm, zHW(xm, y0) - (small ? 30 : 7) - row * 14, "tw-stope", st, 1);
    }

    g.add(ink.mesh(inkMat));
    return { group: g, fill: fillMesh, fillCol, stopeVerts, labels, ext, dev: { mean: n ? sum / n : 0, abs: n ? abs / n : 0, over: over.v, under: under.v, n } };
  }

  /* ---------- Grades: fills and stope labels ---------- */
  // The fill style already says filled, drilled or planned; the two states that matter to a planner today are named.
  const stateWord: Partial<Record<Stope["state"], string>> = { fired: " · open", curing: " · curing" };
  function colour(b: Built) {
    if (b.fillCol) {
      for (const { s: st, from, to } of b.stopeVerts) {
        const gr = i.grade(st.id);
        const c = gr ? rampColour(gr.mean) : srgb(INK.pending);
        const a = gr ? (st.state === "planned" ? 0.42 : st.state === "fired" ? 0.5 : 0.7) : 0.22;
        for (let v = from; v < to; v++) b.fillCol.setXYZW(v, c.r, c.g, c.b, a);
      }
      b.fillCol.needsUpdate = true;
    }
    for (const tag of b.labels) {
      const st = tag.stope;
      if (!st) continue;
      const gr = i.grade(st.id);
      const word = stateWord[st.state] ?? "";
      tag.w = 0;
      tag.el.innerHTML = (
        gr
          ? `<b>${st.id}${word}</b><span class="g"><i style="background:#${rampColour(gr.mean).clone().convertSRGBToLinear().getHexString()}"></i>${f1(gr.mean)}${NB}g/t${small ? ` <span class="r">${f1(gr.lo)}–${f1(gr.hi)}</span>` : ""}</span>${small ? "" : `<span class="r">${f1(gr.lo)}–${f1(gr.hi)}</span>`}`
          : `<b>${st.id}${word}</b><span class="r">grade pending</span>`
      );
    }
  }

  /* ---------- The panel ---------- */
  function paintPanel(b: Built) {
    const L = twin.level;
    const air = AIR.find((a) => a.level === L);
    const kw = air ? air.diesel.reduce((t, d) => t + d.kw, 0) : 0;
    const need = air ? airNeed(air) : 0;
    const byRule = kw * AIR_PER_KW;
    const needWhy = byRule >= AIR_FLOOR ? `${kw}${NB}kW × ${AIR_PER_KW}` : `floor for a level with people${kw ? ` (${kw}${NB}kW × ${AIR_PER_KW} = ${f1(byRule)})` : ""}`;
    const low = air ? air.measured < need : false;
    const graded = b.stopeVerts.filter((v) => i.grade(v.s.id)).length;
    const levels = LEVELS.map((x) => `<button type="button" data-level="${x}" aria-pressed="${x === L}">${lvl(x)}</button>`).join("");
    panel.innerHTML = `<p class="micro">Engineering plan · ${lvl(L)} level · synthetic mine</p>
<div class="tw-levels" role="group" aria-label="Level">${levels}</div>
<ul class="tw-key">
  <li><i class="k asb"></i>As-built wall at ${SECTION}${NB}m <span class="ev measured">measured</span></li>
  <li><i class="k des"></i>Design outline <span class="ev plan">plan</span></li>
  <li><i class="k grade"></i>Stope grade <span class="ev estimated">estimated</span></li>
  <li class="tw-ramp"><span>${RAMP[0]![0]}</span><i></i><span>${RAMP[RAMP.length - 1]![0]}${NB}g/t</span></li>
  <li><i class="k hatch"></i>Paste-filled <i class="k dash"></i>Planned</li>
  <li><i class="k air"></i>Airflow <span class="ev measured">measured</span></li>
  ${air?.duct ? `<li><i class="k duct"></i>Vent duct, ${DUCT.diameter}${NB}m</li>` : ""}
</ul>
${
  air
    ? `<dl class="tw-air">
  <dt>Air on the level</dt><dd>${f1(air.measured)}${NB}m³/s ${low ? '<span class="ev refused">below need</span>' : '<span class="ev measured">measured</span>'}</dd>
  <dt>Needed: ${needWhy}</dt><dd>${f1(need)}${NB}m³/s <span class="ev estimated">estimated</span></dd>
  <dt>Air speed in the drive</dt><dd>${f2(air.measured / DRIVE_AREA)}${NB}m/s</dd>
  ${b.dev.n ? `<dt>Wall off design, mean</dt><dd>${f2(b.dev.abs)}${NB}m <span class="ev measured">measured</span></dd><dt>Most over / under</dt><dd>${signed(b.dev.over)} / ${signed(b.dev.under)}</dd>` : ""}
</dl>`
    : ""
}
<p class="tw-src">${
      b.stopeVerts.length
        ? graded
          ? `Grades: fmine-calib’s Gaussian predictor in WebAssembly, ${graded} stopes, 80% intervals.`
          : "Grades: waiting for the engine."
        : "No stopes are drawn from this level."
    } Need is ${AIR_PER_KW}${NB}m³/s per kW of diesel. Chevrons run at ${TIME_SCALE}× the air speed.</p>`;
  }
  panel.addEventListener("click", (e) => {
    const b = (e.target as Element).closest<HTMLButtonElement>("button[data-level]");
    if (!b) return;
    const L = Number(b.dataset.level) as Level;
    if (L === twin.level) return;
    show(L);
    s.orbit.set(twin.view(), 1.1);
    s.loop.poke();
  });

  /* ---------- The slab cut: clip what crosses it, hide what lies outside it ----------
   * The cut is a discard in a copy of each material (not three's clipping planes, whose shader variant depends on
   * render state and so cannot be compiled ahead): the copies compile in the background before the plan opens. */
  const slab = { top: { value: 0 }, bottom: { value: 0 } };
  const clones = new Map<THREE.Material, THREE.Material>();
  const swapped = new Map<THREE.Object3D & { material: THREE.Material | THREE.Material[] }, THREE.Material | THREE.Material[]>();
  const hidden = new Map<THREE.Object3D, number>();
  const box = new THREE.Box3();
  const clipped = (m: THREE.Material) => {
    let c = clones.get(m);
    if (!c) {
      const copy = m.clone();
      const before = m.onBeforeCompile;
      const key = m.customProgramCacheKey();
      copy.onBeforeCompile = (sh, r) => {
        before.call(m, sh, r);
        sh.uniforms.uTwTop = slab.top;
        sh.uniforms.uTwBot = slab.bottom;
        sh.vertexShader = sh.vertexShader
          .replace("#include <common>", "#include <common>\nvarying float vTwY;")
          .replace(
            "#include <project_vertex>",
            "#include <project_vertex>\n{\n  vec4 twP = vec4(transformed, 1.0);\n#ifdef USE_INSTANCING\n  twP = instanceMatrix * twP;\n#endif\n  vTwY = (modelMatrix * twP).y;\n}",
          );
        sh.fragmentShader = sh.fragmentShader
          .replace("#include <common>", "#include <common>\nuniform float uTwTop;\nuniform float uTwBot;\nvarying float vTwY;")
          .replace("#include <clipping_planes_fragment>", "#include <clipping_planes_fragment>\nif (vTwY > uTwTop || vTwY < uTwBot) discard;");
      };
      copy.customProgramCacheKey = () => `${key}|tw-slab`;
      c = copy;
      clones.set(m, c);
    }
    return c;
  };
  const renderables = () => {
    const out: (THREE.Object3D & { material: THREE.Material | THREE.Material[]; geometry: THREE.BufferGeometry })[] = [];
    world.group.updateMatrixWorld(true);
    world.group.traverse((o) => {
      const r = o as THREE.Mesh;
      if ((r.isMesh || (o as THREE.Line).isLine || (o as THREE.Points).isPoints || (o as THREE.Sprite).isSprite) && r.geometry) out.push(r);
    });
    return out;
  };
  const worldBox = (o: THREE.Object3D & { geometry: THREE.BufferGeometry }) => {
    const im = o as THREE.InstancedMesh;
    if (im.isInstancedMesh) {
      if (!im.boundingBox) im.computeBoundingBox();
      return box.copy(im.boundingBox!).applyMatrix4(o.matrixWorld);
    }
    if (!o.geometry.boundingBox) o.geometry.computeBoundingBox();
    return box.copy(o.geometry.boundingBox!).applyMatrix4(o.matrixWorld);
  };
  /** What the slab around level L does to an object: keep it, clip it to the slab, or hide it. */
  const fate = (o: THREE.Object3D & { material: THREE.Material | THREE.Material[]; geometry: THREE.BufferGeometry }, L: number) => {
    if (o === world.block) return "keep";
    if (o === world.shell || o === world.lens) return "hide";
    const b = worldBox(o);
    if (b.isEmpty()) return "keep";
    const top = L + CUT_TOP;
    const bot = L - CUT_BOTTOM;
    if (b.min.y > top || b.max.y < bot) return "hide";
    if (b.max.y <= top && b.min.y >= bot) return "keep";
    const mats = Array.isArray(o.material) ? o.material : [o.material];
    // Shader materials do not all carry the clipping chunks: hide those rather than leave them uncut.
    return mats.some((m) => (m as THREE.ShaderMaterial).isShaderMaterial) ? "hide" : "clip";
  };
  const cutTo = (L: number) => {
    uncut();
    slab.top.value = L + CUT_TOP;
    slab.bottom.value = L - CUT_BOTTOM;
    // Stopes of other blocks (the filled ones below whose tops reach this floor) belong to other plans.
    const others = new Set<THREE.Object3D>();
    for (const v of world.stopes.values())
      if (v.stope.level !== L) for (const o of [v.design, v.edges, v.rings, v.paste]) if (o) o.traverse((c) => others.add(c));
    for (const o of renderables()) {
      const f = others.has(o) ? "hide" : fate(o, L);
      if (f === "hide") {
        hidden.set(o, o.layers.mask);
        o.layers.set(31);
      } else if (f === "clip") {
        swapped.set(o, o.material);
        o.material = Array.isArray(o.material) ? o.material.map(clipped) : clipped(o.material);
      }
    }
  };
  const uncut = () => {
    for (const [o, m] of swapped) o.material = m;
    swapped.clear();
    for (const [o, mask] of hidden) o.layers.mask = mask;
    hidden.clear();
  };
  /** Compiles the clipped materials in the background (parallel shader compile) before the plan is first opened. */
  // The clipped variants and the drawing's own shaders are compiled ahead (in the driver's background threads where
  // the browser offers parallel compiling): on hovering or touching the button, else at the first opening, when the
  // drawing shows at once and the cut follows as soon as its shaders are ready.
  let warm: Promise<void> | null = null;
  let warmAt = 0;
  const prewarm = (): Promise<void> => {
    if (warm) return warm;
    try {
      // The drawing's own shaders first (small and quick), then stand-ins carrying the cut copies of the materials of
      // everything the slab can cut, so only those compile (not every material in the mine).
      const ink = new THREE.Scene();
      const tiny = new THREE.BufferGeometry();
      tiny.setAttribute("position", new THREE.Float32BufferAttribute([0, 0, 0, 0, 0, 0, 0, 0, 0], 3));
      for (const m of [inkMat, fillMat, dim.material as THREE.Material]) ink.add(new THREE.Mesh(tiny, m));
      const a = s.renderer.compileAsync(ink, s.camera, s.scene);
      const proxies = new THREE.Scene();
      for (const o of renderables()) {
        if ((o as THREE.SkinnedMesh).isSkinnedMesh || !LEVELS.some((L) => fate(o, L) === "clip")) continue;
        const mats = (Array.isArray(o.material) ? o.material : [o.material]).map(clipped);
        const one = mats[0]!;
        const p = (o as THREE.InstancedMesh).isInstancedMesh
          ? new THREE.InstancedMesh(o.geometry, one, 1)
          : (o as THREE.LineSegments).isLineSegments
            ? new THREE.LineSegments(o.geometry, one)
            : (o as THREE.Line).isLine
              ? new THREE.Line(o.geometry, one)
              : (o as THREE.Points).isPoints
                ? new THREE.Points(o.geometry, one)
                : new THREE.Mesh(o.geometry, mats.length > 1 ? mats : one);
        p.castShadow = o.castShadow;
        p.receiveShadow = o.receiveShadow;
        proxies.add(p);
      }
      const b = s.renderer.compileAsync(proxies, s.camera, s.scene);
      warmAt = performance.now();
      warm = Promise.all([a, b]).then(
        () => {},
        () => {},
      );
    } catch {
      warm = Promise.resolve();
    }
    return warm;
  };
  const btn = i.view.parentElement?.querySelector<HTMLElement>("[data-twin]");
  for (const ev of ["pointerenter", "focus", "touchstart"]) btn?.addEventListener(ev, () => void prewarm(), { once: true, passive: true });
  // And once the mine has settled, in an idle moment, so the plan usually opens warm.
  setTimeout(() => {
    const go = () => void prewarm();
    if ("requestIdleCallback" in window) requestIdleCallback(go, { timeout: 4000 });
    else go();
  }, 6000);

  /* ---------- Annotations: centred on their points, by priority; one that would collide tries above and below,
   * else waits until the view has room (zoom in) ---------- */
  const placeTags = () => {
    if (!current) return;
    const W = s.width;
    const H = s.height;
    const vr = i.view.getBoundingClientRect();
    const taken: [number, number, number, number][] = [];
    for (const el of [small ? null : panel, scale, synth, hint]) {
      const r = el?.getBoundingClientRect();
      if (!r || !el?.offsetParent) continue;
      if (r.width) taken.push([r.left - vr.left, r.top - vr.top, r.right - vr.left, r.bottom - vr.top]);
    }
    const tags = [...current.labels].sort((a, b) => a.prio - b.prio);
    const gap = small ? 1.5 : 3;
    for (const t of tags) {
      if (!t.w) {
        t.w = t.el.offsetWidth;
        t.h = t.el.offsetHeight;
      }
      const q = s.project(t.p);
      let ok = false;
      let x = 0;
      let y = 0;
      if (q && t.w)
        for (const dy of [0, -(t.h + 4), t.h + 4]) {
          x = q.x - t.w / 2;
          y = q.y - t.h / 2 + dy;
          if (x < 4 || y < 4 || x + t.w > W - 4 || y + t.h > H - 4) continue;
          if (taken.some(([l, tp, r, b]) => x < r + gap && x + t.w > l - gap && y < b + gap && y + t.h > tp - gap)) continue;
          ok = true;
          break;
        }
      if (ok) taken.push([x, y, x + t.w, y + t.h]);
      if (ok !== t.vis) {
        t.vis = ok;
        t.el.style.visibility = ok ? "" : "hidden";
      }
      if (ok && !(Math.abs(x - t.x) <= 0.3 && Math.abs(y - t.y) <= 0.3)) {
        t.x = x;
        t.y = y;
        t.el.style.transform = `translate3d(${x.toFixed(1)}px, ${y.toFixed(1)}px, 0)`;
      }
    }
  };

  /* ---------- Showing a level ---------- */
  let current: Built | null = null;
  function show(L: Level) {
    if (current) {
      current.group.removeFromParent();
      for (const t of current.labels) t.el.hidden = true;
    }
    twin.level = L;
    let b = built.get(L);
    if (!b) {
      b = buildLevel(L);
      built.set(L, b);
    }
    current = b;
    group.add(b.group);
    colour(b);
    paintPanel(b);
    if (on) {
      for (const t of b.labels) {
        t.el.hidden = false;
        t.w = 0;
      }
      // Some drivers keep compiling after reporting a shader ready: give a fresh compile a moment before using it.
      void prewarm().then(() =>
        setTimeout(
          () => {
            if (on && twin.level === L) {
              cutTo(L);
              s.loop.poke();
            }
          },
          Math.max(0, warmAt + 1800 - performance.now()),
        ),
      );
    }
  }

  let on = false;
  let maxElevation = s.orbit.maxElevation;
  let lastBar = -1;
  let lastAngle = 999;
  const tA = new THREE.Vector3();
  const tB = new THREE.Vector3();
  const twin: Twin = {
    group,
    get on() {
      return on;
    },
    level: -100,
    set(v: boolean) {
      if (v === on) return;
      on = v;
      group.visible = v;
      if (v) {
        maxElevation = s.orbit.maxElevation;
        s.orbit.maxElevation = 89.5;
        show(twin.level);
      } else {
        s.orbit.maxElevation = maxElevation;
        uncut();
        if (current) for (const t of current.labels) t.el.hidden = true;
      }
      s.loop.poke();
    },
    view() {
      if (!current) show(twin.level);
      const b = current!;
      const W = s.width || i.view.clientWidth;
      const H = s.height || i.view.clientHeight;
      const vr = i.view.getBoundingClientRect();
      const pr = panel.getBoundingClientRect();
      const pad = small ? 10 : 18;
      // The free area: right of the panel on a wide view, below the strip on a phone.
      // Labels are centred on their points, so the drawing keeps half a label's width clear of each side.
      const free = small
        ? { x0: pad + 46, y0: 52, x1: W - pad - 40, y1: H - 64 }
        : { x0: Math.max(pad, pr.right - vr.left + 56), y0: 56, x1: W - 44, y1: H - 56 };
      const a = THREE.MathUtils.degToRad(AZ);
      const R = { x: Math.cos(a), z: -Math.sin(a) };
      const D = { x: Math.sin(a), z: Math.cos(a) };
      let r0 = Infinity;
      let r1 = -Infinity;
      let d0 = Infinity;
      let d1 = -Infinity;
      const { x0, x1, z0, z1 } = b.ext;
      for (const [x, z] of [
        [x0, z0],
        [x1, z0],
        [x0, z1],
        [x1, z1],
      ] as const) {
        const r = x * R.x + z * R.z;
        const d = x * D.x + z * D.z;
        r0 = Math.min(r0, r);
        r1 = Math.max(r1, r);
        d0 = Math.min(d0, d);
        d1 = Math.max(d1, d);
      }
      const fw = Math.max(80, free.x1 - free.x0);
      const fh = Math.max(80, free.y1 - free.y0);
      const m = Math.max((r1 - r0) / fw, (d1 - d0) / fh) * 1.06;
      const fov = THREE.MathUtils.degToRad(s.camera.fov);
      const distance = (m * H) / (2 * Math.tan(fov / 2));
      const ox = (free.x0 + free.x1) / 2 - W / 2;
      const oy = (free.y0 + free.y1) / 2 - H / 2;
      const tr = (r0 + r1) / 2 - ox * m;
      const td = (d0 + d1) / 2 - oy * m;
      return { target: [tr * R.x + td * D.x, twin.level + CUT_TOP, tr * R.z + td * D.z], distance, azimuth: AZ, elevation: ELEVATION };
    },
    update(dt: number) {
      if (!reduced) uTime.value += dt;
      placeTags();
      // A plan keeps its orientation: turning is locked (tilting and zooming still work).
      if (s.orbit.azimuth !== AZ) s.orbit.azimuth = AZ;
      // Screen scale at the plan: metres per CSS pixel, for stroke widths and the scale bar.
      const t = s.orbit.target;
      tA.set(t.x, twin.level + CUT_TOP, t.z);
      const a = THREE.MathUtils.degToRad(AZ);
      tB.set(tA.x + Math.cos(a) * 20, tA.y, tA.z - Math.sin(a) * 20);
      const pa = s.project(tA);
      const pb = s.project(tB);
      if (!pa || !pb) return;
      const pxPerM = Math.hypot(pb.x - pa.x, pb.y - pa.y) / 20;
      if (!(pxPerM > 0)) return;
      uPx.value = 1 / pxPerM;
      const len = [5, 10, 20, 25, 50, 100, 200].reduce((best, l) => (l * pxPerM <= (small ? 90 : 120) ? l : best), 5);
      const px = len * pxPerM;
      if (Math.abs(px - lastBar) > 0.5) {
        lastBar = px;
        bar.style.width = `${px.toFixed(1)}px`;
        barText.textContent = `${len}${NB}m`;
      }
      tB.set(tA.x, tA.y, tA.z - 20);
      const pn = s.project(tB);
      if (pn) {
        const ang = (Math.atan2(pn.x - pa.x, -(pn.y - pa.y)) * 180) / Math.PI;
        if (Math.abs(ang - lastAngle) > 0.5) {
          lastAngle = ang;
          north.style.transform = `rotate(${ang.toFixed(1)}deg)`;
        }
      }
    },
    refresh() {
      for (const b of built.values()) colour(b);
      if (current) paintPanel(current);
    },
  };
  return twin;
}

/** Open cross-section of an arched excavation, m²: vertical walls to the springline, then a half-ellipse back. */
function DECLINE_AREA(w: number, h: number) {
  const R = 0.32 * w;
  return w * (h - R) + (Math.PI * (w / 2) * R) / 2;
}
