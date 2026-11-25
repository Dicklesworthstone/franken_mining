/**
 * The blocks in the cutaway, in front of the section: what the engine's pit takes and what it leaves.
 *
 * - Ore inside the pit glows in its grade's colour: deep copper near the cut-off, copper at half a gram, gold and
 *   white-hot above.
 * - Ore the pit leaves in the ground is drawn small and dull.
 * - The pit's own surface is glass: the faces where a block in the pit meets rock it leaves, seen from inside the
 *   pit and tinted by the shell that first takes the block, so the stepped bowl shows the pushbacks and the ore
 *   inside shows through.
 *
 * Each change of pit is a transition: blocks joining the pit arrive bench by bench from the top, blocks leaving go
 * from the bottom up, and blocks that change from ore to waste or back flash as they turn.
 */
import * as THREE from "three";
import { B, centreX, centreY, centreZ, COLS, CUT, H, index, NX, NZ } from "./pit-model";

/** Shell colours, inner (50% of the price) to outer (the pit): teal to sky, the Plan chapter's colour. */
export const SHELL_COLOURS = ["#3fd6a8", "#2ec9c4", "#3ab8e4", "#56a5f0", "#7b95f3", "#9fb8ff"];

/**
 * Ore's colour by grade, g/t, on a log scale: deep copper at the cut-off's end of the range, copper at half a gram,
 * then amber, gold and white-hot, the Explore scene's ramp above half a gram.
 */
const ORE_STOPS: [number, string][] = [
  [0.15, "#5e3020"],
  [0.3, "#9a4724"],
  [0.5, "#d0662a"],
  [1.0, "#f2a33c"],
  [2.0, "#ffd166"],
  [4.0, "#fff4d6"],
];
const oreStops = ORE_STOPS.map(([g, c]) => [Math.log(g), new THREE.Color(c)] as const);
export function oreColour(g: number, out = new THREE.Color()) {
  const l = Math.log(Math.max(g, 1e-6));
  const first = oreStops[0]!;
  const last = oreStops[oreStops.length - 1]!;
  if (l <= first[0]) return out.copy(first[1]);
  if (l >= last[0]) return out.copy(last[1]);
  for (let i = 1; i < oreStops.length; i++) {
    const [b, cb] = oreStops[i]!;
    if (l <= b) {
      const [a, ca] = oreStops[i - 1]!;
      return out.copy(ca).lerp(cb, (l - a) / (b - a));
    }
  }
  return out.copy(last[1]);
}
/**
 * A block's value colour, dollars per block (price, recovery and costs as set): losses in brick red, break-even in
 * grey, then green to pale gold as the block pays more. On a signed log scale, so a $50k block and a $2M one both read.
 */
const VALUE_STOPS: [number, string][] = [
  [-1, "#b4472f"],
  [-0.2, "#8a5a48"],
  [0, "#7d8188"],
  [0.25, "#2f9e74"],
  [0.55, "#6cc552"],
  [0.8, "#d4e86a"],
  [1, "#fff7c2"],
];
const valueStops = VALUE_STOPS.map(([v, c]) => [v, new THREE.Color(c)] as const);
/** Signed log position of a block value on [−1, 1]: ±$5M at the ends. */
export const valuePos = (v: number) => Math.sign(v) * Math.min(1, Math.log10(1 + Math.abs(v) / 5000) / Math.log10(1 + 5e6 / 5000));
export function valueColour(v: number, out = new THREE.Color()) {
  const t = valuePos(v);
  for (let i = 1; i < valueStops.length; i++) {
    const [b, cb] = valueStops[i]!;
    if (t <= b) {
      const [a, ca] = valueStops[i - 1]!;
      return out.copy(ca).lerp(cb, (t - a) / (b - a));
    }
  }
  return out.copy(valueStops[valueStops.length - 1]![1]);
}
/** CSS stops for the value ramp from −$v to +$v on the same scale. */
export function valueGradient() {
  return VALUE_STOPS.map(([t, c]) => `${c} ${(((t + 1) / 2) * 100).toFixed(1)}%`).join(", ");
}

/** CSS stops for the ore ramp between two grades, as percentages of a log axis. */
export function oreGradient(lo: number, hi: number) {
  const span = Math.log(hi) - Math.log(lo);
  return ORE_STOPS.map(([g, c]) => `${c} ${(((Math.log(g) - Math.log(lo)) / span) * 100).toFixed(1)}%`).join(", ");
}

export interface BlockState {
  /** Per block: in the set drawn as the pit, ore at this price, first shell (−1 for none). */
  inPit: (b: number) => boolean;
  ore: Uint8Array;
  firstShell: Int32Array | null;
  /** Each block's value at this price, dollars, as the engine was given it. */
  value: Float64Array;
  /** What colours the ore: its grade, or its value. */
  by: "grade" | "value";
}

export interface Blocks {
  group: THREE.Group;
  /** Moves to a new state; `flips` marks blocks whose ore or waste call changed. */
  set(state: BlockState, seconds: number): void;
  update(dt: number): boolean;
  /** Centroid and count of ore left in the ground inside the cutaway, for a label. */
  left: { count: number; at: THREE.Vector3; grade: number };
  /** How strongly the glass draws: fainter in the rendered view, full in the engineering one. */
  glass(strength: number): void;
}

/* The cutaway's blocks, indexed locally. */
const CI = CUT.i1 - CUT.i0;
const CJ = CUT.j1 - CUT.j0;
const CK = CUT.k1;
const LOCAL = CI * CJ * CK;
const globalOf = (l: number) => {
  const i = CUT.i0 + (l % CI);
  const j = CUT.j0 + (Math.floor(l / CI) % CJ);
  const k = Math.floor(l / (CI * CJ));
  return index(i, j, k);
};

/** Ore: solid, lit, with its own glow. */
function oreMaterial() {
  const m = new THREE.MeshStandardMaterial({ roughness: 0.55, metalness: 0.05 });
  m.customProgramCacheKey = () => "fm-pit-ore-v2";
  m.onBeforeCompile = (s) => {
    s.uniforms.uT = uniforms.uT;
    s.vertexShader = s.vertexShader
      .replace(
        "#include <common>",
        `#include <common>
uniform float uT;
attribute vec4 aFrom;
attribute vec4 aTo;
attribute vec4 aAnim;
varying vec3 vTint;
varying float vGlow;
varying vec3 vBox;`,
      )
      .replace(
        "#include <begin_vertex>",
        `#include <begin_vertex>
vBox = position / vec3(${B.toFixed(1)}, ${H.toFixed(1)}, ${B.toFixed(1)});
float pr = clamp((uT - aAnim.x) / aAnim.y, 0.0, 1.0);
pr = pr * pr * (3.0 - 2.0 * pr);
float sc = mix(aFrom.w, aTo.w, pr);
transformed *= sc;
vTint = mix(aFrom.rgb, aTo.rgb, pr);
// A target glow above 10 marks a block that changed from ore to waste or back: it flashes as it turns.
float flip = step(10.0, aAnim.w);
vGlow = mix(aAnim.z, aAnim.w - 10.0 * flip, pr);
float flash = flip * sin(pr * 3.14159);
vGlow += flash * 1.6;
vTint = mix(vTint, vec3(1.0, 0.9, 0.72), flash * 0.35);`,
      );
    s.fragmentShader = s.fragmentShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vTint;\nvarying float vGlow;\nvarying vec3 vBox;")
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
// A block model's cubes: each face framed by a dark edge, as a planning package draws them.
vec3 ab = abs(vBox);
float bhi = max(ab.x, max(ab.y, ab.z));
float blo = min(ab.x, min(ab.y, ab.z));
float bmid = ab.x + ab.y + ab.z - bhi - blo;
float bw = fwidth(bmid) * 1.6 + 1e-4;
float bEdge = smoothstep(0.5 - bw - 0.012, 0.5 - 0.004, bmid);
diffuseColor.rgb = vTint * (1.0 - 0.6 * bEdge);`,
      )
      .replace("#include <emissivemap_fragment>", "#include <emissivemap_fragment>\ntotalEmissiveRadiance += vTint * vGlow * (1.0 - 0.75 * bEdge);");
  };
  return m;
}

const uniforms = { uT: { value: 0 }, uGlass: { value: 0.6 } };

/**
 * Glass: faint faces and brighter edges, blended over what is behind. Each block carries a mask of the faces it
 * shows (bit 0 −x, 1 +x, 2 down, 3 up, 4 −z, 5 +z), before and after the transition; only the back faces draw,
 * so the near wall of the pit never hides the ore and the far wall and floor read as a bowl.
 */
function glassMaterial() {
  return new THREE.ShaderMaterial({
    uniforms: { uT: uniforms.uT, uGlass: uniforms.uGlass, ...THREE.UniformsLib.fog },
    transparent: true,
    depthWrite: false,
    blending: THREE.NormalBlending,
    side: THREE.BackSide,
    fog: true,
    vertexShader: `uniform float uT;
attribute vec4 aFrom;
attribute vec4 aTo;
attribute vec4 aAnim;
attribute vec2 aMask;
varying vec3 vLocal;
varying vec3 vTint;
varying float vAlpha;
#include <fog_pars_vertex>
float bit(float m, float f) { return mod(floor(m / exp2(f) + 0.001), 2.0); }
void main() {
  float pr = clamp((uT - aAnim.x) / aAnim.y, 0.0, 1.0);
  pr = pr * pr * (3.0 - 2.0 * pr);
  float f = abs(normal.x) > 0.5 ? (normal.x > 0.0 ? 1.0 : 0.0) : abs(normal.y) > 0.5 ? (normal.y > 0.0 ? 3.0 : 2.0) : (normal.z > 0.0 ? 5.0 : 4.0);
  vAlpha = mix(aFrom.w * bit(aMask.x, f), aTo.w * bit(aMask.y, f), pr);
  vTint = mix(aFrom.rgb, aTo.rgb, pr);
  vLocal = position / vec3(${B.toFixed(1)}, ${H.toFixed(1)}, ${B.toFixed(1)});
  // Inset a few centimetres, so no glass face shares a plane with the cutaway's walls or the section.
  vec3 transformed = position * (vAlpha > 0.001 ? 0.988 : 0.0);
  vec4 mvPosition = modelViewMatrix * instanceMatrix * vec4(transformed, 1.0);
  gl_Position = projectionMatrix * mvPosition;
  #include <fog_vertex>
}`,
    fragmentShader: `uniform float uGlass;
varying vec3 vLocal;
varying vec3 vTint;
varying float vAlpha;
#include <fog_pars_fragment>
void main() {
  vec3 a = abs(vLocal);
  // The second-largest coordinate: 0.5 on an edge of the cube.
  float hi = max(a.x, max(a.y, a.z));
  float lo = min(a.x, min(a.y, a.z));
  float mid = a.x + a.y + a.z - hi - lo;
  float w = fwidth(mid) * 1.1 + 1e-4;
  float edge = smoothstep(0.5 - w, 0.5, mid);
  // Glass: a faint body and brighter edges, layered over what is behind.
  gl_FragColor = vec4(vTint * (0.85 + 0.5 * edge), (0.06 + 0.5 * edge) * vAlpha * uGlass);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
  #ifdef USE_FOG
    #ifdef FOG_EXP2
      float fogFactor = 1.0 - exp( - fogDensity * fogDensity * vFogDepth * vFogDepth );
    #else
      float fogFactor = smoothstep( fogNear, fogFar, vFogDepth );
    #endif
    gl_FragColor.a *= 1.0 - fogFactor;
  #endif
}`,
  });
}

export function makeBlocks(grade: Float32Array, o: { reduced: boolean }): Blocks {
  const group = new THREE.Group();
  group.name = "pit-blocks";
  const box = new THREE.BoxGeometry(B, H, B);

  // One instance per cutaway block, in both meshes; a zero scale or alpha hides it.
  const mk = (mat: THREE.Material) => {
    const geo = box.clone();
    const from = new THREE.InstancedBufferAttribute(new Float32Array(LOCAL * 4), 4).setUsage(THREE.DynamicDrawUsage);
    const to = new THREE.InstancedBufferAttribute(new Float32Array(LOCAL * 4), 4).setUsage(THREE.DynamicDrawUsage);
    const anim = new THREE.InstancedBufferAttribute(new Float32Array(LOCAL * 4), 4).setUsage(THREE.DynamicDrawUsage);
    geo.setAttribute("aFrom", from);
    geo.setAttribute("aTo", to);
    geo.setAttribute("aAnim", anim);
    const mesh = new THREE.InstancedMesh(geo, mat, LOCAL);
    const m = new THREE.Matrix4();
    for (let l = 0; l < LOCAL; l++) {
      const b = globalOf(l);
      const k = Math.floor(b / COLS);
      const c = b % COLS;
      m.makeTranslation(centreX(c % NX), centreY(k), centreZ(Math.floor(c / NX)));
      mesh.setMatrixAt(l, m);
    }
    mesh.frustumCulled = false;
    group.add(mesh);
    return { mesh, from, to, anim };
  };
  const ore = mk(oreMaterial());
  // Ore casts shadows into the cutaway: its depth pass scales each block as the colour pass does.
  {
    const depth = new THREE.MeshDepthMaterial({ depthPacking: THREE.RGBADepthPacking });
    depth.onBeforeCompile = (sh) => {
      sh.uniforms.uT = uniforms.uT;
      sh.vertexShader = sh.vertexShader
        .replace("#include <common>", "#include <common>\nuniform float uT;\nattribute vec4 aFrom;\nattribute vec4 aTo;\nattribute vec4 aAnim;")
        .replace(
          "#include <begin_vertex>",
          "#include <begin_vertex>\nfloat pr = clamp((uT - aAnim.x) / aAnim.y, 0.0, 1.0);\npr = pr * pr * (3.0 - 2.0 * pr);\ntransformed *= mix(aFrom.w, aTo.w, pr);",
        );
    };
    ore.mesh.customDepthMaterial = depth;
    ore.mesh.castShadow = true;
    ore.mesh.receiveShadow = true;
  }
  const glass = mk(glassMaterial());
  glass.mesh.renderOrder = 2;
  const mask = new THREE.InstancedBufferAttribute(new Float32Array(LOCAL * 2), 2).setUsage(THREE.DynamicDrawUsage);
  glass.mesh.geometry.setAttribute("aMask", mask);
  const maskNow = new Float32Array(LOCAL);

  const colour = new THREE.Color();
  const tint = SHELL_COLOURS.map((c) => new THREE.Color(c));
  // What each instance shows now, to start the next transition from (mid-flight when a new pit lands early).
  const oreNow = new Float32Array(LOCAL * 4);
  const glowNow = new Float32Array(LOCAL);
  const glassNow = new Float32Array(LOCAL * 4);
  const progress = (anim: Float32Array, l: number) => {
    const p = Math.min(1, Math.max(0, (uniforms.uT.value - anim[l * 4]!) / anim[l * 4 + 1]!));
    return p * p * (3 - 2 * p);
  };
  const capture = () => {
    const oa = ore.anim.array as Float32Array;
    const ga = glass.anim.array as Float32Array;
    const of = ore.from.array as Float32Array;
    const ot = ore.to.array as Float32Array;
    const gf = glass.from.array as Float32Array;
    const gt = glass.to.array as Float32Array;
    for (let l = 0; l < LOCAL; l++) {
      const p = progress(oa, l);
      const q = progress(ga, l);
      for (let c = 0; c < 4; c++) {
        oreNow[l * 4 + c] = of[l * 4 + c]! + (ot[l * 4 + c]! - of[l * 4 + c]!) * p;
        glassNow[l * 4 + c] = gf[l * 4 + c]! + (gt[l * 4 + c]! - gf[l * 4 + c]!) * q;
      }
      const w = oa[l * 4 + 3]!;
      const to = w >= 10 ? w - 10 : w;
      glowNow[l] = oa[l * 4 + 2]! + (to - oa[l * 4 + 2]!) * p;
      // Faces cannot blend between masks: past halfway, the new one stands.
      const ma = mask.array as Float32Array;
      maskNow[l] = q < 0.5 ? ma[l * 2]! : ma[l * 2 + 1]!;
    }
  };
  const wasPit = new Uint8Array(LOCAL);
  const wasOre = new Uint8Array(LOCAL);
  let first = true;
  let clock = 0;
  let until = 0;
  const left = { count: 0, at: new THREE.Vector3(), grade: 0 };

  return {
    group,
    left,
    glass(strength) {
      uniforms.uGlass.value = strength;
    },
    set(state, seconds) {
      const dur = o.reduced || first ? 0 : seconds;
      if (!first) capture();
      clock = 0;
      uniforms.uT.value = 0;
      until = 0;
      let lx = 0;
      let ly = 0;
      let lz = 0;
      let lg = 0;
      left.count = 0;
      const inside = (i: number, j: number, k: number) =>
        i >= 0 && i < NX && j >= 0 && j < NZ && k >= 0 && state.inPit(index(i, j, k));
      for (let l = 0; l < LOCAL; l++) {
        const b = globalOf(l);
        const k = Math.floor(b / COLS);
        const c = b % COLS;
        const i = c % NX;
        const j = Math.floor(c / NX);
        const pit = state.inPit(b);
        const isOre = state.ore[b] === 1;
        const g = grade[b]!;
        // The ore mesh.
        let r = 0;
        let gg = 0;
        let bb = 0;
        let s = 0;
        let glow = 0;
        if (isOre) {
          if (state.by === "value") valueColour(state.value[b]!, colour);
          else oreColour(g, colour);
          if (pit) {
            r = colour.r;
            gg = colour.g;
            bb = colour.b;
            s = 0.86;
            glow = state.by === "value" ? 0.2 + 0.6 * Math.max(0, valuePos(state.value[b]!)) : 0.35 + 0.55 * Math.min(1, Math.log(1 + g) / Math.log(5));
          } else {
            // Left in the ground: small, dull, the colour drained towards grey. By value, a block that would pay for
            // itself keeps some colour: the pit leaves it because the waste above it costs more.
            const keepC = state.by === "value" && state.value[b]! > 0 ? 0.75 : 0.45;
            const grey = (colour.r + colour.g + colour.b) / 3;
            r = (colour.r * keepC + grey * (1 - keepC)) * 0.6;
            gg = (colour.g * keepC + grey * (1 - keepC)) * 0.6;
            bb = (colour.b * keepC + grey * (1 - keepC)) * 0.6;
            s = 0.62;
            glow = state.by === "value" && state.value[b]! > 0 ? 0.12 : 0.03;
            left.count++;
            lx += centreX(i);
            ly += centreY(k);
            lz += centreZ(j);
            lg += g;
          }
        }
        const flipped = !first && isOre !== (wasOre[l] === 1) && (isOre || wasOre[l] === 1);
        // Joining the pit arrives from the top bench down; leaving goes from the bottom up.
        const joins = pit && !wasPit[l];
        const leaves = !pit && wasPit[l];
        const delay = dur ? (joins ? (k / CK) * dur * 0.55 : leaves ? ((CK - k) / CK) * dur * 0.45 : 0) : 0;
        const span = dur ? dur * 0.45 : 1e-3;
        until = Math.max(until, delay + span);
        const o4 = l * 4;
        ore.from.array.set(first ? [r, gg, bb, s] : oreNow.subarray(o4, o4 + 4), o4);
        ore.to.array.set([r, gg, bb, s], o4);
        // Glow from, glow to; +10 on the target marks a flip, which flashes.
        ore.anim.array.set([delay, span, first ? glow : glowNow[l]!, glow + (flipped ? 10 : 0)], o4);

        // The glass bowl: each face where a block in the pit meets rock the pit leaves.
        let a = 0;
        let tr = 0;
        let tg = 0;
        let tb = 0;
        let faces = 0;
        if (pit) {
          if (!inside(i - 1, j, k)) faces |= 1;
          if (!inside(i + 1, j, k)) faces |= 2;
          if (!inside(i, j, k + 1)) faces |= 4;
          if (!inside(i, j - 1, k)) faces |= 16;
          if (!inside(i, j + 1, k)) faces |= 32;
          if (faces) {
            const sh = state.firstShell ? state.firstShell[b]! : -1;
            const t = tint[sh >= 0 ? Math.min(sh, tint.length - 1) : tint.length - 1]!;
            tr = t.r;
            tg = t.g;
            tb = t.b;
            a = 1;
          }
        }
        const ma = mask.array as Float32Array;
        ma[l * 2] = first ? faces : maskNow[l]!;
        ma[l * 2 + 1] = faces;
        glass.from.array.set(first ? [tr, tg, tb, a] : glassNow.subarray(o4, o4 + 4), o4);
        glass.to.array.set([tr, tg, tb, a], o4);
        glass.anim.array.set([delay, span, 0, 0], o4);
        wasPit[l] = pit ? 1 : 0;
        wasOre[l] = isOre ? 1 : 0;
      }
      mask.needsUpdate = true;
      for (const m of [ore, glass]) {
        m.from.needsUpdate = true;
        m.to.needsUpdate = true;
        m.anim.needsUpdate = true;
      }
      if (left.count) left.at.set(lx / left.count, ly / left.count, lz / left.count);
      left.grade = left.count ? lg / left.count : 0;
      first = false;
      if (!dur) uniforms.uT.value = 1e6;
    },
    update(dt) {
      if (clock > until + 0.1) return false;
      clock += dt;
      uniforms.uT.value = clock;
      return true;
    },
  };
}
