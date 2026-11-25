/**
 * Plan: the certified ultimate pit. The Explore scene's synthetic orebody, as a block model, under a desert; the pit
 * FrankenMining's pseudoflow solver finds for it (fmine-optimize, compiled to WebAssembly and run in a worker), cut
 * into the ground as 15 m benches with 72° faces and the berms the overall slope leaves; and a cutaway in front of
 * a section line where the blocks show: ore inside the pit glowing in its grade's (or value's) colour, ore left in
 * the ground dull, the pit's own surface as glass tinted by the nested shell that first takes each block.
 *
 * Move the gold price, the overall slope or the mining cost and the engine solves the pit again, checks its own
 * certificate independently, and solves six nested shells; the ground morphs to the new pit and the blocks that
 * change sides flash. Every figure on the cards comes from those calls (or counts the engine's block sets against
 * the block model). The engineering view greys the ground back and draws the crest and toe strings and the
 * section's dimensions; the section sheet under the scene draws section A–A′ at true scale. An agent reads what
 * moved and drafts a pushback sequence for the mine planner to review.
 */
import * as THREE from "three";
import { LineMaterial } from "three/examples/jsm/lines/LineMaterial.js";
import { LineSegments2 } from "three/examples/jsm/lines/LineSegments2.js";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";
import type { PitResult, ShellsResult } from "./engine";
import { $, clamp, fmt, range, reduced, seg } from "./lib";
import { makeBlocks, oreColour, oreGradient, SHELL_COLOURS, valueColour, valueGradient, valuePos, type Blocks } from "./pit-blocks";
import { FACE_DEG, bermWidth, makeGround, type Ground } from "./pit-ground";
import {
  B,
  BLOCK_T,
  centreX,
  columnDepths,
  CUT,
  CUT_X0,
  CUT_X1,
  CUT_Z0,
  cutoff,
  FACTORS,
  H,
  N,
  NX,
  NY,
  NZ,
  START,
  tally,
  type Econ,
  type Tally,
} from "./pit-model";
import { makeSection, type Section } from "./pit-section";
import { benchDetail, sheet, SHEET_VIEWBOX, type SheetBlock } from "./pit-sheet";
import type { Solver } from "./pit-solve";
import { stage, type Stage } from "./stage";
import { range as rangeHtml, seg as segHtml } from "../parts";

const S = FACTORS.length;
const PIT = "pit";
const MORPH = 1.5;

const money = (v: number) => {
  const a = Math.abs(v);
  const s = a >= 1e9 ? `$${fmt(a / 1e9, 2)}B` : a >= 1e6 ? `$${fmt(a / 1e6, 1)}M` : `$${fmt(a / 1e3, 0)}k`;
  return v < 0 ? `−${s}` : s;
};
const mt = (t: number) => `${fmt(t / 1e6, 1)} Mt`;
const koz = (oz: number) => `${fmt(oz / 1000, 0)} koz`;
const pct = (f: number) => `${fmt(f * 100, 0)}%`;
const priceText = (p: number) => `$${fmt(p)}/oz`;

interface Solved {
  econ: Econ;
  pit: PitResult;
  ore: Uint8Array;
  value: Float64Array;
  tally: Tally;
  shells?: ShellsResult;
  /** Per shell: its tally and its value at today's price (the sum of the engine's block set's values). */
  shellTally?: Tally[];
  shellValue?: number[];
}

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  const tick = (v: number) => `${(((valuePos(v) + 1) / 2) * 100).toFixed(1)}%`;
  host.innerHTML = `<div class="pit">
<div class="scene-view pit-view">
  <div class="hud-card hud-tl pit-cert">
    <p class="micro">Certified ultimate pit</p>
    <p class="pit-big"><b data-p="value">…</b><span class="pit-ok" data-p="ok">solving</span></p>
    <p class="pit-sub" data-p="sub">Loading FrankenMining’s solver (WebAssembly)…</p>
    <dl class="pit-dl" data-p="dl">
      <dt>Ore</dt><dd data-p="ore">…</dd>
      <dt>Waste</dt><dd data-p="waste">…</dd>
      <dt>Strip ratio</dt><dd data-p="strip">…</dd>
      <dt>Gold recovered</dt><dd data-p="oz">…</dd>
    </dl>
  </div>
  <div class="hud-card hud-tr pit-shells">
    <p class="micro">Nested shells · pit by pit</p>
    <svg class="pit-chart" viewBox="0 0 280 132" role="img" aria-label="Tonnes and value of each nested shell" data-p="chart"></svg>
    <p class="pit-chart-key"><i class="o"></i>ore <i class="w"></i>waste <i class="v"></i>value at today’s price</p>
  </div>
  <div class="hud-card hud-bl agent-note pit-agent" aria-live="polite">
    <p class="micro">Agent · price sensitivity</p>
    <div data-p="agent"><p>Waiting for the first pit.</p></div>
  </div>
  <div class="pit-legend hud-br" aria-hidden="true">
    <div class="pit-by-grade">
      <p>Ore grade, g/t</p>
      <div class="pit-ramp" style="background:linear-gradient(90deg, ${oreGradient(0.15, 4)})"><i data-p="cutmark"></i></div>
      <p class="pit-ticks"><span>0.15</span><span>0.5</span><span>1</span><span>2</span><span>4</span></p>
      <p class="pit-cut" data-p="cut"></p>
    </div>
    <div class="pit-by-value">
      <p>Block value at this price</p>
      <div class="pit-ramp" style="background:linear-gradient(90deg, ${valueGradient()})"></div>
      <p class="pit-vticks"><span style="left:${tick(-1e5)}">−$100k</span><span style="left:50%">0</span><span style="left:${tick(1e5)}">$100k</span><span style="left:${tick(1e6)}">$1M</span></p>
    </div>
    <ul class="pit-keys">
      <li class="pk-in">Ore in the pit</li>
      <li class="pk-left">Ore left in the ground</li>
      <li class="pk-glass">The pit’s surface, as glass</li>
      <li class="pk-crest">Bench crest</li>
      <li class="pk-toe">Bench toe</li>
    </ul>
  </div>
  <p class="pit-status" data-p="status" aria-live="polite"></p>
</div>
<div class="scene-bar pit-bar">
  ${rangeHtml("price", "Gold price", 1200, 4000, 50, START.price, priceText(START.price))}
  ${rangeHtml("slope", "Overall slope", 35, 55, 1, START.slope, `${START.slope}°`)}
  ${rangeHtml("mining", "Mining cost", 1.5, 4.5, 0.1, START.mining, `$${START.mining.toFixed(2)}/t`)}
  <div class="pit-show">
    <span class="micro">Draw as ground</span>
    ${segHtml("shell", "Which pit the ground shows", [[PIT, "Pit"], ...FACTORS.slice(0, -1).map((f, s) => [String(s), pct(f)] as [string, string]).reverse()], PIT)}
  </div>
  <div class="pit-show">
    <span class="micro">View</span>
    ${segHtml("view", "View", [["render", "Rendered"], ["eng", "Engineering"]], "render")}
  </div>
  <div class="pit-show">
    <span class="micro">Colour blocks by</span>
    ${segHtml("by", "Colour blocks by", [["grade", "Grade"], ["value", "Value"]], "grade")}
  </div>
</div>
<div class="pit-below">
  <div class="pit-sheet-card">
    <div class="pit-sheet-head">
      <p class="micro">Section A–A′ · as cut in the scene · true scale</p>
      <p class="pit-sheet-key"><i class="d"></i>shown pit, benched <i class="n"></i>natural surface <i class="s"></i>shells 50–90% <i class="b"></i>block set</p>
    </div>
    <svg class="pit-sheet" viewBox="${SHEET_VIEWBOX}" role="img" aria-label="Section A–A′ through the pit at true scale, with dimensions" data-p="sheet"></svg>
    <div class="pit-sheet-foot">
      <div data-p="bench"></div>
      <p data-p="design"></p>
    </div>
  </div>
  <div class="term pit-term">
    <div class="term-bar"><i></i><i></i><i></i><span>engine.pit · engine.shells · WebAssembly</span></div>
    <pre data-p="term">Loading…</pre>
  </div>
</div>
<p class="pit-note">Synthetic: the orebody is the one drilled in Explore, generated in your browser; prices and costs are illustrative. ${fmt(N)} blocks of ${B} × ${B} × ${H} m (${NX} × ${NZ} × ${NY}), so each re-solve takes milliseconds on a phone too. In the browser the limit is WebAssembly’s memory; natively the solver has run 2.1 million blocks in about 1.2 s. Block value is the recovered gold at the price less processing ($18/t, 90% recovery) for ore, less mining for every block. Shells scale the price with the ore and waste split held. The benches are drawn from the solved block set at ${H} m with ${FACE_DEG}° faces and the berm the overall slope leaves; the shell has no haul ramp yet, and designing one adds waste.</p>
</div>`;
  const view = $(host, ".pit-view");
  const below = $(host, ".pit-below");
  const ui = (k: string) => $(host, `[data-p="${k}"]`);
  const phone = matchMedia("(max-width: 699px)");
  const small = phone.matches;
  const root = $(host, ".pit");

  /* ---------- Status ---------- */
  let statusTimer = 0;
  const status = (html: string, on = true) => {
    const el = ui("status");
    el.innerHTML = html;
    el.classList.toggle("on", on);
  };
  const fail = (html: string) => {
    const p = document.createElement("p");
    p.className = "pit-fallback";
    p.innerHTML = html;
    view.append(p);
  };

  /* ---------- Stage, if this browser can draw it ---------- */
  let s: Stage | undefined;
  try {
    s = stage(view, {
      clock: 17.25,
      fog: 0.00034,
      sunBearing: 40,
      shadowRadius: 440,
      shadowAt: [0, -90, 20],
      near: 8,
      far: 16000,
      fov: small ? 40 : 30,
      drift: 0,
      maxPixelRatio: small ? 1.5 : 1.75,
      label: "A synthetic gold orebody as a block model under a desert, with the certified ultimate pit cut into the ground as benches and a cutaway showing the blocks",
      shift: () => (phone.matches ? [0, 0.1] : [0, 0.03]),
      labelArea: () => (phone.matches ? [0.03, 0.3, 0.97, 0.97] : [0.2, 0.08, 0.8, 0.92]),
    });
  } catch (err) {
    console.error(err);
    s = undefined;
    fail("This browser has WebGL turned off, so the 3D pit cannot draw here. The engine still runs: the certificate, the shells and the section below update as you move the sliders.");
  }
  const home = { target: [0, -120, 30] as [number, number, number], distance: small ? 1780 : 1330, azimuth: small ? 18 : 24, elevation: small ? 38 : 33 };
  let ground: Ground | undefined;
  let section: Section | undefined;
  if (s) {
    s.orbit.minDistance = 300;
    s.orbit.maxDistance = 2800;
    s.orbit.minElevation = 12;
    s.orbit.maxElevation = 80;
    s.orbit.set(home);
    ground = makeGround({ cell: small ? 4 : 2.5, scatter: small ? 900 : 2600 });
    s.scene.add(ground.group);
    section = makeSection(ground.sectionX);
    s.scene.add(section.group);
    s.start();
  }

  /* ---------- The block model's extents, for the engineering view: its box, with a tick every 100 m ---------- */
  const extents = (() => {
    const v: number[] = [];
    const x0 = -(NX * B) / 2;
    const x1 = -x0;
    const z0 = -(NZ * B) / 2;
    const z1 = -z0;
    const y0 = 0.6;
    const y1 = -NY * H;
    const e = (a: number[], b: number[]) => v.push(...a, ...b);
    for (const y of [y0, y1]) {
      e([x0, y, z0], [x1, y, z0]);
      e([x1, y, z0], [x1, y, z1]);
      e([x1, y, z1], [x0, y, z1]);
      e([x0, y, z1], [x0, y, z0]);
    }
    for (const [x, z] of [
      [x0, z0],
      [x1, z0],
      [x1, z1],
      [x0, z1],
    ] as const)
      e([x, y0, z], [x, y1, z]);
    for (let x = Math.ceil(x0 / 100) * 100; x <= x1; x += 100) {
      e([x, y0, z1], [x, y0, z1 + 8]);
      e([x, y0, z0], [x, y0, z0 - 8]);
    }
    for (let z = Math.ceil(z0 / 100) * 100; z <= z1; z += 100) {
      e([x0, y0, z], [x0 - 8, y0, z]);
      e([x1, y0, z], [x1 + 8, y0, z]);
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(v, 3));
    const m = new THREE.LineSegments(g, new THREE.LineBasicMaterial({ color: "#cfe7ff", transparent: true, opacity: 0, depthWrite: false, fog: true }));
    m.visible = false;
    m.renderOrder = 6;
    return m;
  })();
  if (s) s.scene.add(extents);

  /* ---------- Crest and toe strings ---------- */
  const lineMat = (color: string, dashed: boolean) =>
    new LineMaterial({ color, linewidth: small ? 1.3 : 1.6, worldUnits: false, transparent: true, opacity: 0, depthWrite: false, dashed, dashSize: 5, gapSize: 3.5 });
  const crestMat = lineMat("#ffd98a", false);
  const toeMat = lineMat("#6fd6f2", true);
  const crestLine = new LineSegments2(new LineSegmentsGeometry(), crestMat);
  const toeLine = new LineSegments2(new LineSegmentsGeometry(), toeMat);
  crestLine.renderOrder = 6;
  toeLine.renderOrder = 6;
  let hasStrings = false;
  if (s) s.scene.add(crestLine, toeLine);
  const setStrings = () => {
    if (!ground) return;
    const st = ground.strings();
    hasStrings = st.crest.length > 0 || st.toe.length > 0;
    for (const [line, arr] of [
      [crestLine, st.crest],
      [toeLine, st.toe],
    ] as const) {
      line.geometry.dispose();
      const g = new LineSegmentsGeometry();
      if (arr.length) g.setPositions(arr);
      line.geometry = g;
      if (arr.length) line.computeLineDistances();
    }
  };

  /* ---------- The solver ---------- */
  status("Loading FrankenMining’s engine (WebAssembly)…");
  let sv: Solver;
  let grade: Float32Array;
  try {
    const { solver } = await import("./pit-solve");
    sv = await solver();
    grade = (await sv.model()).grade;
  } catch (err) {
    console.error(err);
    const msg = String(err instanceof Error ? err.message : err).slice(0, 140);
    ui("sub").textContent = `FrankenMining’s engine did not load here (${msg}). Reloading the page usually brings it back.`;
    ui("ok").textContent = "no engine";
    ui("ok").classList.add("bad");
    status(`The engine did not load: ${msg}`);
    return;
  }
  let blocks: Blocks | undefined;
  if (s) {
    blocks = makeBlocks(grade, { reduced });
    s.scene.add(blocks.group);
  }

  /* ---------- Labels ---------- */
  const tag = (cls: string) => s?.label("", new THREE.Vector3(), { className: `pit-tag ${cls}` });
  const floorTag = tag("stem tall");
  const leftTag = tag("left down");
  const widthTag = tag("dim");
  const depthTag = tag("dim");
  const modelTag = tag("dim");
  modelTag?.anchor(new THREE.Vector3((NX * B) / 2, 0, (NZ * B) / 2), { y: 4 });
  modelTag?.html(`Block model <b>${NX * B} × ${NZ * B} × ${NY * H} m</b> · ${fmt(N)} blocks`);
  s?.label(`Section <b>A–A′</b>`, new THREE.Vector3(CUT_X1 - 40, 0, CUT_Z0), { y: 8, className: "pit-tag sec" });
  for (const t of [floorTag, leftTag, widthTag, depthTag, modelTag]) t?.show(false);

  /* ---------- State ---------- */
  let want: Econ = { ...START };
  let now: Solved | undefined;
  let prev: Solved | undefined;
  let shown: string = PIT;
  let mode: "render" | "eng" = "render";
  let by: "grade" | "value" = "grade";
  let engT = 0;
  let running = false;
  let timer = 0;
  const xs = ground?.sectionX ?? Float32Array.from({ length: Math.round((CUT_X1 - CUT_X0) / 5) + 1 }, (_, i) => CUT_X0 + i * 5);
  const profiles = Array.from({ length: S }, () => new Float32Array(xs.length));
  const design = new Float32Array(xs.length);
  const natural = new Float32Array(xs.length);
  xs.forEach((x, p) => (natural[p] = ground ? ground.surface(x, CUT_Z0) : 0));

  /** The block set the ground shows: the certified pit, or a shell. */
  const shownSet = (sol: Solved): ((b: number) => boolean) => {
    if (shown === PIT || !sol.shells) return (b) => sol.pit.inPit[b] === 1;
    const top = Number(shown);
    const first = sol.shells.firstShell;
    return (b) => first[b]! >= 0 && first[b]! <= top;
  };
  /** A block set's benched profile along the section; without the 3D ground, its plain block outline. */
  const profileOf = (cols: Uint8Array, slope: number, out: Float32Array) => {
    if (ground) return ground.profile(cols, slope, out);
    for (let p = 0; p < xs.length; p++) {
      const i = clamp(Math.floor((xs[p]! - (CUT_X0 - CUT.i0 * B)) / B), 0, NX - 1);
      out[p] = -cols[i + NX * CUT.j0]! * H;
    }
    return out;
  };

  /* ---------- Drawing a solution ---------- */
  const faceRow = CUT.j0 - 1;
  const paintFace = (sol: Solved, inSet: (b: number) => boolean) => {
    if (!ground) return;
    const data = ground.faceGrades.image.data as Uint8Array;
    const c = new THREE.Color();
    for (let k = 0; k < NY; k++)
      for (let i = 0; i < NX; i++) {
        const b = i + NX * (faceRow + NZ * k);
        const g = grade[b]!;
        const o = (k * NX + i) * 4;
        if (g < 0.1 || inSet(b)) {
          data[o + 3] = 0;
          continue;
        }
        if (by === "value" && sol.ore[b]) valueColour(sol.value[b]!, c);
        else oreColour(Math.max(g, 0.15), c);
        c.convertLinearToSRGB();
        data[o] = Math.round(c.r * 255);
        data[o + 1] = Math.round(c.g * 255);
        data[o + 2] = Math.round(c.b * 255);
        data[o + 3] = sol.ore[b] ? 230 : Math.round(70 * clamp((g - 0.1) / 0.15, 0, 1));
      }
    ground.faceGrades.needsUpdate = true;
  };

  /** The section sheet, and the 3D dimensions and their labels from the same profile. */
  const drawSheet = (sol: Solved, inSet: (b: number) => boolean) => {
    const rows: SheetBlock[] = [];
    for (let k = 0; k < CUT.k1; k++)
      for (let i = CUT.i0; i < CUT.i1; i++) {
        const b = i + NX * (faceRow + NZ * k);
        rows.push({ i: i - CUT.i0, k, grade: grade[b]!, ore: sol.ore[b] === 1, inSet: inSet(b), value: sol.value[b]! });
      }
    const shownIdx = shown === PIT ? S - 1 : Number(shown);
    const berm = bermWidth(sol.econ.slope);
    ui("sheet").innerHTML = sheet({ xs, ground: natural, design, shells: profiles, shown: shownIdx, blocks: rows, by, slope: sol.econ.slope, berm, face: FACE_DEG });
    ui("bench").innerHTML = benchDetail(sol.econ.slope, berm, FACE_DEG);
    ui("design").innerHTML = `Benches ${H} m high with ${FACE_DEG}° faces; at <b>${sol.econ.slope}°</b> overall each berm is <b>${fmt(berm, 1)} m</b> wide. The thin stepped line is the engine’s block set on this row; the benched line is the surface drawn from it, and the angles run crest to floor as this section cuts each wall. A wall flatter than the slope is set by the ore, or the section crosses it at a skew.`;
    // Crests, floor and the wall the depth stands beside, for the 3D dimensions.
    let a = -1;
    let z = -1;
    let deep = 0;
    let at = 0;
    for (let p = 0; p < xs.length; p++) {
      const cut = natural[p]! - design[p]!;
      if (cut > 0.5) {
        if (a < 0) a = p;
        z = p;
      }
      if (cut > deep) {
        deep = cut;
        at = p;
      }
    }
    if (a < 0 || !section || !widthTag || !depthTag) return;
    const right = xs[z]! + 70 < CUT_X1;
    const xd = right ? xs[z]! + 24 : xs[a]! - 24;
    const dims = { xa: xs[a]!, ya: natural[a]!, xz: xs[z]!, yz: natural[z]!, xd, yFloor: design[at]!, xDeep: xs[at]! };
    const on = mode === "eng";
    section.dims(on ? dims : null);
    widthTag.anchor(new THREE.Vector3((dims.xa + dims.xz) / 2, Math.max(dims.ya, dims.yz) + 34, CUT_Z0 + 2), { y: 6 });
    widthTag.html(`Crest to crest <b>${fmt(dims.xz - dims.xa)} m</b>`);
    depthTag.anchor(new THREE.Vector3(xd, (Math.max(dims.ya, dims.yz) + dims.yFloor) / 2, CUT_Z0 + 2));
    depthTag.html(`Depth <b>${fmt(deep)} m</b>`);
    widthTag.show(on);
    depthTag.show(on);
    modelTag?.show(on);
  };

  let drawing = 0;
  const draw = (sol: Solved, seconds: number) => {
    const inSet = shownSet(sol);
    const cols = columnDepths(inSet);
    ground?.shape(cols, sol.econ.slope, seconds);
    setStrings();
    // The blocks, the section and the sheet a frame later.
    const id = ++drawing;
    requestAnimationFrame(() => {
      if (id === drawing) drawRest(sol, seconds, inSet, cols);
    });
  };
  const drawRest = (sol: Solved, seconds: number, inSet: (b: number) => boolean, cols: Uint8Array) => {
    blocks?.set({ inPit: inSet, ore: sol.ore, firstShell: sol.shells?.firstShell ?? null, value: sol.value, by }, seconds);
    paintFace(sol, inSet);
    profileOf(cols, sol.econ.slope, design);
    if (sol.shells) {
      const first = sol.shells.firstShell;
      for (let sh = 0; sh < S; sh++) profileOf(columnDepths((b) => first[b]! >= 0 && first[b]! <= sh), sol.econ.slope, profiles[sh]!);
      section?.set(natural, profiles, seconds, shown === PIT ? S - 1 : Number(shown));
    }
    drawSheet(sol, inSet);
    // The floor at the section, and the ore left behind in the cutaway.
    let deep = 0;
    let at = 0;
    for (let i = CUT.i0; i < CUT.i1; i++) {
      const d = cols[i + NX * CUT.j0]!;
      if (d > deep) {
        deep = d;
        at = i;
      }
    }
    if (floorTag) {
      if (deep > 0 && mode === "render" && !phone.matches) {
        floorTag.anchor(new THREE.Vector3(centreX(at), -deep * H, CUT_Z0 + 2), { y: 70 });
        floorTag.html(`Floor <b>${fmt(deep * H)} m</b> down · ${deep} benches`);
        floorTag.show(true);
      } else floorTag.show(false);
    }
    if (blocks && leftTag) {
      const left = blocks.left;
      if (left.count > 0 && mode === "render" && !phone.matches) {
        leftTag.anchor(left.at.clone(), { y: -8 });
        leftTag.html(`Ore left in the ground here <b>${fmt(left.count)}</b> blocks · ${fmt(left.grade, 2)} g/t`);
        leftTag.show(true);
      } else leftTag.show(false);
    }
    s?.loop.poke();
  };

  /* ---------- Cards ---------- */
  const setText = (k: string, v: string) => {
    const el = ui(k);
    if (el.textContent !== v) el.textContent = v;
  };
  const cert = (sol: Solved) => {
    const t = sol.tally;
    setText("value", money(sol.pit.value));
    const ok = ui("ok");
    ok.textContent = sol.pit.verified ? "verified" : "not verified";
    ok.classList.toggle("bad", !sol.pit.verified);
    ok.classList.add("on");
    ui("sub").innerHTML = `<b>${fmt(sol.pit.blocks)}</b> of ${fmt(N)} blocks · solved in <b>${fmt(sol.pit.ms, 0)} ms</b> in your browser${small ? "" : ", then checked by an independent verifier"}`;
    setText("ore", `${mt(t.oreT)} · ${fmt(t.grade, 2)} g/t`);
    setText("waste", mt(t.wasteT));
    setText("strip", `${fmt(t.strip, 2)} : 1`);
    setText("oz", koz(t.oz));
    const cut = cutoff(sol.econ);
    setText("cut", `Cut-off ${fmt(cut, 2)} g/t at ${priceText(sol.econ.price)}`);
    ui("cutmark").style.left = `${clamp((Math.log(cut) - Math.log(0.15)) / (Math.log(4) - Math.log(0.15)), 0, 1) * 100}%`;
  };

  const term = (sol: Solved) => {
    const p = sol.pit;
    const e = sol.econ;
    const lines = [
      `<span class="dim">$</span> pit   ${NX} × ${NZ} × ${NY} blocks · ${B} × ${B} × ${H} m · slope ${e.slope}° · ${priceText(e.price)} · mining $${e.mining.toFixed(2)}/t`,
      `  value      <span class="ok">$${fmt(p.value, 0)}</span>   the most any pit the slope allows is worth`,
      `  verified   <span class="${p.verified ? "ok" : "bad"}">${p.verified ? "true" : "false"}</span>   a checker separate from the solver, run on its flow certificate`,
      `  blocks     ${fmt(p.blocks)} of ${fmt(N)}`,
      `  arcs       ${fmt(p.arcs)} kept · ${fmt(p.fullConeArcs)} in the full slope cones`,
      `  pseudoflow ${fmt(p.stats.mergers)} mergers · ${fmt(p.stats.splits)} splits · ${fmt(p.stats.relabels)} relabels`,
      `  scale      ${fmt(p.scale)} integer units per dollar (exact for the rounded values)`,
      `  time       ${fmt(p.ms, 1)} ms ${sv.where === "worker" ? "in a Web Worker" : "in the page"}`,
    ];
    if (sol.shells && sol.shellTally && sol.shellValue) {
      lines.push(`<span class="dim">$</span> shells  ${S} revenue factors · ${fmt(sol.shells.ms, 1)} ms · ${fmt(sol.shells.arcs)} arcs`);
      sol.shells.shells.forEach((sh, i) => {
        lines.push(
          `  ${pct(sh.factor).padStart(4)}  ${fmt(sh.blocks).padStart(6)} blocks  ${money(sh.value).padStart(8)} at ${priceText(Math.round(e.price * sh.factor))}  <span class="dim">${money(sol.shellValue![i]!)} at ${priceText(e.price)}</span>`,
        );
      });
    }
    // One block per line with a hanging indent at the value column, so a line too long for a phone wraps under its own
    // value instead of running off the edge.
    ui("term").innerHTML = lines
      .map((line) => {
        const plain = line.replace(/<[^>]+>/g, "");
        const ind = plain.startsWith("$") ? 2 : (/^\s+\S+\s+/.exec(plain)?.[0].length ?? 0);
        return `<span class="ln" style="--ind:${ind}">${line}</span>`;
      })
      .join("");
  };

  /** The pit-by-pit chart: cumulative tonnes of each shell (ore and waste) and its value at today's price. */
  const chart = (sol: Solved) => {
    const svg = ui("chart");
    if (!sol.shellTally || !sol.shellValue) return;
    const W = 280;
    const left = 30;
    const right = 46;
    const top = 8;
    const base = 108;
    const tMax = Math.max(1, ...sol.shellTally.map((t) => t.oreT + t.wasteT));
    const vMax = Math.max(1, ...sol.shellValue);
    const slot = (W - left - right) / S;
    const bw = slot * 0.58;
    const pick = shown === PIT ? S - 1 : Number(shown);
    let bars = "";
    const pts: string[] = [];
    sol.shellTally.forEach((t, i) => {
      const x = left + slot * i + (slot - bw) / 2;
      const hw = ((t.wasteT + t.oreT) / tMax) * (base - top);
      const ho = (t.oreT / tMax) * (base - top);
      const sel = i === pick;
      bars += `<g class="bar${sel ? " sel" : ""}" data-shell="${i === S - 1 ? PIT : i}">`;
      bars += `<rect class="hit" x="${x - 4}" y="${top}" width="${bw + 8}" height="${base - top + 20}"/>`;
      bars += `<rect class="w" x="${x}" y="${base - hw}" width="${bw}" height="${hw - ho}"/>`;
      bars += `<rect class="o" x="${x}" y="${base - ho}" width="${bw}" height="${ho}"/>`;
      bars += `<rect class="edge" x="${x - 1.5}" y="${base - hw - 1.5}" width="${bw + 3}" height="${hw + 3}" style="stroke:${SHELL_COLOURS[i]}"/>`;
      bars += `<text x="${x + bw / 2}" y="${base + 12}">${pct(FACTORS[i]!)}</text></g>`;
      const v = sol.shellValue![i]!;
      pts.push(`${(x + bw / 2).toFixed(1)},${(base - (Math.max(0, v) / vMax) * (base - top)).toFixed(1)}`);
    });
    const last = sol.shellTally[S - 1]!;
    svg.innerHTML =
      `<line class="ax" x1="${left - 4}" x2="${W - right + 4}" y1="${base}" y2="${base}"/>` +
      `<text class="yl" x="${left - 6}" y="${top + 6}" text-anchor="end">${fmt((last.oreT + last.wasteT) / 1e6, 0)}</text>` +
      `<text class="yl" x="${left - 6}" y="${top + 16}" text-anchor="end">Mt</text>` +
      bars +
      `<polyline class="vl" points="${pts.join(" ")}"/>` +
      pts.map((p) => `<circle class="vd" cx="${p.split(",")[0]}" cy="${p.split(",")[1]}" r="2.6"/>`).join("") +
      `<text class="yr" x="${W - right + 6}" y="${top + 6}">${money(vMax)}</text>`;
  };
  ui("chart").addEventListener("click", (e) => {
    const g = (e.target as Element).closest<SVGGElement>("[data-shell]");
    if (!g) return;
    pickShell(g.dataset.shell!);
    shellSeg.set(g.dataset.shell!);
  });

  /* ---------- The agent's note ---------- */
  const agent = (sol: Solved) => {
    const out: string[] = [];
    const e = sol.econ;
    const cut = cutoff(e);
    if (prev && prev.econ !== sol.econ) {
      const a = prev;
      let toOre = 0;
      let toWaste = 0;
      let joined = 0;
      let leftPit = 0;
      let gLo = Infinity;
      let gHi = 0;
      for (let b = 0; b < N; b++) {
        if (sol.ore[b] && !a.ore[b]) {
          toOre++;
          gLo = Math.min(gLo, grade[b]!);
          gHi = Math.max(gHi, grade[b]!);
        }
        if (!sol.ore[b] && a.ore[b]) {
          toWaste++;
          gLo = Math.min(gLo, grade[b]!);
          gHi = Math.max(gHi, grade[b]!);
        }
        const p1 = sol.pit.inPit[b] === 1;
        const p0 = a.pit.inPit[b] === 1;
        if (p1 && !p0) joined++;
        if (!p1 && p0) leftPit++;
      }
      const what =
        a.econ.price !== e.price
          ? `From ${priceText(a.econ.price)} to ${priceText(e.price)}`
          : a.econ.slope !== e.slope
            ? `From ${a.econ.slope}° to ${e.slope}°`
            : `From $${a.econ.mining.toFixed(2)} to $${e.mining.toFixed(2)}/t mining`;
      const flips = toOre + toWaste;
      const flipText = flips
        ? `<b>${fmt(flips)}</b> blocks turn from ${toOre >= toWaste ? "waste to ore" : "ore to waste"} (${fmt(gLo, 2)}–${fmt(gHi, 2)} g/t, around the ${fmt(cut, 2)} g/t cut-off)`
        : "no block changes from ore to waste";
      const net = joined - leftPit;
      const dBench = sol.tally.benches - a.tally.benches;
      const pitText =
        joined || leftPit
          ? `the pit ${net >= 0 ? "takes" : "gives up"} <b>${fmt(Math.abs(net))}</b> blocks (${mt(Math.abs(net) * BLOCK_T)})${dBench ? ` and goes ${Math.abs(dBench)} ${Math.abs(dBench) === 1 ? "bench" : "benches"} ${dBench > 0 ? "deeper" : "shallower"}` : ""}, and its value ${sol.pit.value >= a.pit.value ? "rises" : "falls"} ${money(Math.abs(sol.pit.value - a.pit.value))}`
          : `the pit does not move; its value ${sol.pit.value >= a.pit.value ? "rises" : "falls"} ${money(Math.abs(sol.pit.value - a.pit.value))}`;
      out.push(`<p>${what}: ${flipText}; ${pitText}.</p>`);
    } else {
      let inPit = 0;
      let all = 0;
      for (let b = 0; b < N; b++)
        if (sol.ore[b]) {
          all++;
          if (sol.pit.inPit[b]) inPit++;
        }
      out.push(
        `<p>At ${priceText(e.price)} the cut-off is <b>${fmt(cut, 2)} g/t</b>: ${fmt(all)} blocks are ore and the pit takes <b>${fmt(inPit)}</b> of them. Move the price to see which blocks change sides.</p>`,
      );
    }
    if (sol.shellTally && sol.shellValue && sol.shells) {
      const T = sol.shellTally.map((t) => t.oreT + t.wasteT);
      const V = sol.shellValue;
      const total = T[S - 1]!;
      const vTot = V[S - 1]!;
      // The biggest step out between neighbouring shells.
      let big = 1;
      for (let i = 2; i < S; i++) if (T[i]! - T[i - 1]! > T[big]! - T[big - 1]!) big = i;
      if (T[0]! > T[big]! - T[big - 1]!) big = 0;
      const stepText =
        big === 0
          ? `The shells start big: the ${pct(FACTORS[0]!)} shell already holds ${pct(T[0]! / total)} of the pit’s tonnes`
          : `The shells step out most from ${pct(FACTORS[big - 1]!)} to ${pct(FACTORS[big]!)}: <b>+${mt(T[big]! - T[big - 1]!)}</b> for +${money(V[big]! - V[big - 1]!)}`;
      // The starter pit: the shell that front-loads the most value for its tonnes.
      let starter = -1;
      let best = 0.04;
      for (let i = 0; i < S - 1; i++) {
        if (T[i]! <= 0 || vTot <= 0) continue;
        const gain = V[i]! / vTot - T[i]! / total;
        if (gain > best) {
          best = gain;
          starter = i;
        }
      }
      out.push(`<p>${stepText}.</p>`);
      if (starter >= 0 && vTot > 0) {
        const st = sol.shellTally[starter]!;
        out.push(
          `<p class="pit-draft"><b>Draft:</b> a starter pit on the ${pct(FACTORS[starter]!)} shell (${pct(V[starter]! / vTot)} of the value in ${pct(T[starter]! / total)} of the tonnes, strip ${fmt(st.strip, 1)}), then one pushback to the final wall.</p>`,
        );
      } else if (vTot > 0) {
        out.push(`<p class="pit-draft"><b>Draft:</b> no shell front-loads the value enough for a separate starter pit; mine to the final wall in one push.</p>`);
      } else {
        out.push(`<p class="pit-draft"><b>Draft:</b> at this price no pit pays; hold the design.</p>`);
      }
      out.push(`<p class="act">Drafted for the mine planner to review</p>`);
    }
    ui("agent").innerHTML = out.join("");
  };

  /* ---------- Solving ---------- */
  const frame = () => new Promise<void>((done) => requestAnimationFrame(() => setTimeout(done, 0)));
  const same = (a: Econ, b: Econ) => a.price === b.price && a.slope === b.slope && a.mining === b.mining;
  const kick = async () => {
    if (running) return;
    running = true;
    const econ = { ...want };
    clearTimeout(statusTimer);
    status(`Solving at ${priceText(econ.price)} · ${econ.slope}° …`);
    try {
      let sol: Solved | undefined;
      await sv.solve(
        econ,
        (p, ore, value) => {
          sol = { econ, pit: p, ore, value, tally: tally(grade, ore, (b) => p.inPit[b] === 1) };
          cert(sol);
          status(`Solved in ${fmt(p.ms, 0)} ms · ${p.verified ? "verified" : "check failed"} · shells…`);
        },
        (sh) => {
          if (!sol) return;
          sol.shells = sh;
          const first = sh.firstShell;
          sol.shellTally = FACTORS.map((_, i) => tally(grade, sol!.ore, (b) => first[b]! >= 0 && first[b]! <= i));
          sol.shellValue = FACTORS.map((_, i) => {
            let v = 0;
            for (let b = 0; b < N; b++) if (first[b]! >= 0 && first[b]! <= i) v += sol!.value[b]!;
            return v;
          });
        },
      );
      if (sol) {
        prev = now ?? undefined;
        now = sol;
        // Spread over frames, so no single task holds the page: the scene first, then the cards and the note.
        draw(sol, prev ? MORPH : 0);
        await frame();
        term(sol);
        chart(sol);
        await frame();
        agent(sol);
        status(`Solved in ${fmt(sol.pit.ms, 0)} ms · ${sol.pit.verified ? "verified" : "check failed"} · ${S} shells in ${fmt(sol.shells?.ms ?? 0, 0)} ms`);
        statusTimer = window.setTimeout(() => status(ui("status").innerHTML, false), 2600);
      }
    } catch (err) {
      status(`The engine refused: ${String(err instanceof Error ? err.message : err).slice(0, 120)}`);
      console.error(err);
    }
    running = false;
    if (!same(econ, want)) void kick();
  };
  const request = () => {
    clearTimeout(timer);
    timer = window.setTimeout(() => void kick(), running ? 0 : 40);
  };

  /* ---------- Controls ---------- */
  range(host, "price", priceText, (v) => {
    want = { ...want, price: v };
    request();
  });
  range(host, "slope", (v) => `${v}°`, (v) => {
    want = { ...want, slope: v };
    request();
  });
  range(host, "mining", (v) => `$${v.toFixed(2)}/t`, (v) => {
    want = { ...want, mining: Math.round(v * 10) / 10 };
    request();
  });
  const pickShell = (v: string) => {
    shown = v;
    if (now) {
      draw(now, reduced ? 0 : 1.1);
      chart(now);
    }
  };
  const shellSeg = seg(host, "shell", pickShell);
  seg(host, "view", (v) => {
    mode = v === "eng" ? "eng" : "render";
    root.classList.toggle("eng", mode === "eng");
    if (now) draw(now, 0);
  });
  seg(host, "by", (v) => {
    by = v === "value" ? "value" : "grade";
    root.classList.toggle("by-value", by === "value");
    if (now) draw(now, reduced ? 0 : 0.5);
  });

  /* ---------- Phones: the certificate over the scene, the rest below ---------- */
  const place = () => {
    const shellsCard = $(host, ".pit-shells");
    const agentCard = $(host, ".pit-agent");
    if (phone.matches) below.prepend(shellsCard, agentCard);
    else view.append(shellsCard, agentCard);
  };
  place();
  phone.addEventListener("change", place);

  /* ---------- Frames ---------- */
  s?.frame((dt) => {
    const a = ground?.update(dt) ?? false;
    const b = blocks?.update(dt) ?? false;
    const c = section?.update(dt) ?? false;
    // The engineering view eases in; the strings show once the ground has stopped moving under them.
    const goal = mode === "eng" ? 1 : 0;
    const was = engT;
    engT = clamp(engT + Math.sign(goal - engT) * dt * 3, 0, 1);
    const e3 = engT * engT * (3 - 2 * engT);
    ground?.setEngineering(e3);
    blocks?.glass(0.55 + 0.45 * e3);
    extents.material.opacity = 0.55 * e3;
    extents.visible = e3 > 0.01;
    const aim = a || !hasStrings ? 0 : engT;
    const op = crestMat.opacity + clamp(aim - crestMat.opacity, -dt * 6, dt * 2);
    crestMat.opacity = op;
    toeMat.opacity = op * 0.95;
    crestLine.visible = op > 0.01;
    toeLine.visible = op > 0.01;
    if (s) {
      crestMat.resolution.set(s.width, s.height);
      toeMat.resolution.set(s.width, s.height);
    }
    if (a || b || c || was !== engT || Math.abs(op - aim) > 0.005) s?.loop.poke();
  });
  // The figure counts as drawn once the first solve has filled the cards: the page keeps the figure's room until then,
  // so nothing below it moves while the engine works.
  await kick();
}
