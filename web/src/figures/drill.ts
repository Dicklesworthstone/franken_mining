/**
 * Explore: a synthetic gold prospect under a desert plain at the golden hour. A constellation of diamond drillholes
 * glows under a window of glass ground, each trace coloured and thickened by its assays; between them sits the
 * block model, estimated by FrankenMining's own conditional Gaussian predictor (fmine-calib, compiled to
 * WebAssembly) and shown as the estimate, as its uncertainty (a fog that clears along every hole) or as the hidden
 * truth. A rig and its crew stand at the newest pad. The agent ranks the next collar, has the engine work out how
 * much the hole would narrow the gold interval of the ground it tests, and prices that with FrankenMining's
 * value-of-information kernel; the exploration manager approves, or sites a hole by clicking the ground.
 *
 * Everything is synthetic: the orebody is generated in the browser, and the true grades are shown only on request.
 */
import * as THREE from "three";
import { mergeGeometries } from "three/examples/jsm/utils/BufferGeometryUtils.js";
import { gradeColour, gradeGradient } from "./drill-body";
import { makeFog } from "./drill-fog";
import { boxDist, makeGround, X1, Z1 } from "./drill-ground";
import { tags, type Tag } from "./drill-labels";
import {
  assay,
  B,
  BEARING,
  blockLn,
  blocksNear,
  compositeCentres,
  containedOz,
  COST_PER_M,
  CUTOFF,
  designHole,
  DIP,
  drillOutCost,
  gridE,
  gridN,
  holeDecision,
  makeProspect,
  NB,
  rankCandidates,
  samplesOf,
  SD_MAX,
  VARIOGRAM,
  widthShare,
  X0,
  Z0,
  type Hole,
  type Interval,
} from "./drill-model";
import { EngineError, estimate, monitor, ready, voi, type Sample } from "./engine";
import { makePickup } from "./fleet";
import { collarAhead, makeCoreRig, makePadKit, RACK_RODS } from "./drill-rig";
import { clockOf, depthAt, OPS, plan, type Plan } from "./drill-ops";
import { $, clamp, fmt, mix, reduced, seg, smooth } from "./lib";
import { makeCoreTray, makeMiner } from "./miners";
import { stage, type StageOptions } from "./stage";
import { seg as segHtml } from "../parts";

type Mode = "estimate" | "sd" | "truth";

const AGENT = "#8be27a";
const BARREN = new THREE.Color("#a99f96");
/** The tint an ore block takes for a moment when an estimate changes it. */
const FLASH_SOLID = new THREE.Color("#d9f4ff");
/** Composites drawn at most. */
const MAX_SEGS = 2000;
const CHUNK = 900;
const SD_MIN = Math.sqrt(VARIOGRAM.nugget);
/** How long the shown model takes to settle on a new estimate, seconds. */
const SETTLE = 1.4;

const koz = (oz: number) => `${fmt(oz / 1000)} koz`;
const money = (v: number) => (Math.abs(v) >= 1e6 ? `$${fmt(v / 1e6, 2)}M` : `$${fmt(Math.round(v / 1000))}k`);
const nextFrame = () => new Promise<void>((r) => requestAnimationFrame(() => r()));
const deg3 = (b: number) => `${String(Math.round(b)).padStart(3, "0")}°`;

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.innerHTML = `<div class="drill">
<div class="scene-view drill-view">
  <div class="hud-card hud-tl drill-metal">
    <p class="micro">Gold above ${CUTOFF} g/t</p>
    <p class="drill-big"><b data-d="p50">…</b><span class="ev estimated">estimated</span></p>
    <p class="drill-sub">P10–P90 <span data-d="int">…</span></p>
    <div class="drill-band" aria-hidden="true"><i data-d="band"></i><u data-d="truth"></u></div>
    <figure class="drill-fan">
      <div data-d="fan"></div>
      <figcaption>P10–P90 and median as holes are added, log scale</figcaption>
    </figure>
    <p class="drill-truth" data-d="truthline" hidden></p>
  </div>
  <div class="hud-card hud-tr drill-stats">
    <p class="micro">Synthetic programme</p>
    <dl>
      <dt>Holes</dt><dd data-d="holes">…</dd>
      <dt>Metres drilled</dt><dd data-d="metres">…</dd>
      <dt>Assays</dt><dd data-d="assays">…</dd>
      <dt>Spent</dt><dd data-d="spent">…</dd>
      <dt>Rig days</dt><dd data-d="days">…</dd>
      <dt>Blocks estimated</dt><dd data-d="blocks">…</dd>
    </dl>
    <p class="drill-engine" data-d="engine">Loading FrankenMining’s predictor…</p>
    <p class="drill-calib" data-d="calib" hidden></p>
  </div>
  <div class="hud-card hud-bl agent-note drill-agent" aria-live="polite">
    <p class="micro">Agent · next hole</p>
    <div data-d="agent"><p>Waiting for the first estimate.</p></div>
  </div>
  <div class="drill-legend hud-br" aria-hidden="true">
    <p data-d="legend-title">Estimated grade, g/t</p>
    <div class="drill-ramp" data-d="ramp"></div>
    <p class="drill-ticks" data-d="ticks"></p>
  </div>
  <p class="drill-status" data-d="status" aria-live="polite"></p>
</div>
<div class="scene-bar drill-bar">
  ${segHtml("mode", "What the blocks show", [["estimate", "Estimate"], ["sd", "Uncertainty"], ["truth", "Truth"]], "estimate")}
  <div class="btnrow">
    <button type="button" class="btn sm go" data-drill disabled>Approve and drill</button>
    <button type="button" class="btn sm" data-section aria-pressed="false" title="A 60 m slice of the model along the newest hole, seen square on">Section</button>
    <button type="button" class="btn sm" data-reset disabled>Reset</button>
  </div>
  <p class="drill-hint" data-d="hint" data-avoid></p>
</div>
<div class="drill-below"></div>
</div>`;
  const view = $(host, ".drill-view");
  const below = $(host, ".drill-below");
  const ui = (k: string) => $(host, `[data-d="${k}"]`);
  const drillBtn = $<HTMLButtonElement>(host, "[data-drill]");
  const resetBtn = $<HTMLButtonElement>(host, "[data-reset]");
  const sectionBtn = $<HTMLButtonElement>(host, "[data-section]");
  const phone = matchMedia("(max-width: 699px)");
  const small = phone.matches;
  ui("hint").textContent =
    (small
      ? "Or tap the ground inside the outline to site your own hole. Drag to turn, pinch to zoom."
      : "Or click the ground inside the outline to site your own hole. Drag to turn; ctrl and scroll to zoom.") +
    " Rig rates and timings are illustrative; the estimates, prices and scores come from the engine.";

  /* ---------- Stage ---------- */
  const opts: StageOptions = {
    clock: 17.6,
    fog: 0.00022,
    sunBearing: -100,
    shadowRadius: 460,
    shadowAt: [0, 0, 0],
    fov: small ? 40 : 30,
    drift: 0,
    maxPixelRatio: small ? 1.5 : 1.6,
    label: "A synthetic gold prospect under a desert plain: drillholes and the estimated block model under a window of glass ground",
    shift: () => (phone.matches ? [0, 0.1] : [0, 0.12]),
    // Labels place themselves clear of the cards (drill-labels.ts); the stage need not hide them.
    labelArea: () => [-0.5, -0.5, 1.5, 1.5],
  };
  const s = stage(view, opts);
  s.renderer.localClippingEnabled = true;
  s.orbit.minDistance = 25;
  s.orbit.maxDistance = 2600;
  s.orbit.minElevation = 6;
  s.orbit.maxElevation = 80;
  const HOME = () =>
    phone.matches
      ? { target: [10, -150, 0] as [number, number, number], distance: 1320, azimuth: 30, elevation: 22 }
      : { target: [24, -165, 0] as [number, number, number], distance: 1150, azimuth: 28, elevation: 15 };
  s.orbit.set(HOME());
  s.start();

  /* ---------- Ground and prospect, built across frames ---------- */
  // Build timings, for the long-task budget (read with ?stage).
  const tm: [string, number][] = [];
  let tt = performance.now();
  const lap = (k: string) => {
    const n = performance.now();
    tm.push([k, Math.round(n - tt)]);
    tt = n;
  };
  await nextFrame();
  tt = performance.now();
  const G = makeGround({ small, glass: 0.3 });
  lap("ground");
  s.scene.add(G.group);
  const ground = G.height;
  /** The hole being drilled, if any (declared here: the frame below reads it from the first frame). */
  let anim: Anim | null = null;
  // Up close the ground is solid and real; pulled back, it turns to glass over the block.
  s.frame(() => {
    G.update(s.camera);
    const o = s.orbit;
    const c = s.camera.position;
    const d = c.distanceTo(o.target);
    G.glass(mix(0.92, 0.3, smooth((d - 70) / 160)));
    // Never under the ground: the lowest elevation that keeps the camera some metres over the land beneath it.
    const clear = clamp(o.distance * 0.04, 1.6, 12);
    const need = THREE.MathUtils.radToDeg(Math.asin(clamp((ground(c.x, c.z) + clear - o.target.y) / o.distance, -1, 0.97)));
    o.minElevation = Math.max(6, need);
    // The near plane as far out as the view allows, so depth stays precise for pads and tracks at any range.
    const near = clamp((c.y - ground(c.x, c.z)) * 0.35, 0.3, 60);
    if (Math.abs(s.camera.near - near) > near * 0.08) {
      s.camera.near = near;
      s.camera.updateProjectionMatrix();
    }
    if (!anim && !o.busy && o.elevation < o.minElevation - 0.01) o.elevation = o.minElevation;
  });
  await nextFrame();
  tt = performance.now();
  const P = makeProspect(11);
  lap("prospect");
  await nextFrame();
  tt = performance.now();

  /* ---------- The block model ---------- */
  // Ore blocks are solid, lit by baked face shading (top bright, sides and bottom darker); lower grades glow.
  const boxGeo = new THREE.BoxGeometry(1, 1, 1);
  {
    const n = boxGeo.getAttribute("normal") as THREE.BufferAttribute;
    const shade = new Float32Array(n.count * 3);
    for (let i = 0; i < n.count; i++) {
      const k = n.getY(i) > 0.5 ? 1 : n.getY(i) < -0.5 ? 0.42 : n.getX(i) !== 0 ? 0.72 : 0.58;
      shade.set([k, k, k], i * 3);
    }
    boxGeo.setAttribute("color", new THREE.BufferAttribute(shade, 3));
  }
  const solid = new THREE.InstancedMesh(boxGeo, new THREE.MeshBasicMaterial({ vertexColors: true, toneMapped: false, fog: false }), NB);
  const glow = new THREE.InstancedMesh(
    new THREE.BoxGeometry(1, 1, 1),
    new THREE.MeshBasicMaterial({ transparent: true, blending: THREE.AdditiveBlending, depthWrite: false, fog: false, toneMapped: false }),
    NB,
  );
  for (const m of [solid, glow]) {
    m.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    m.frustumCulled = false;
    s.scene.add(m);
  }
  glow.renderOrder = 2;
  const flash = new Float32Array(NB);
  const fog = makeFog({ steps: small ? 56 : 96 });
  fog.mesh.visible = false;
  s.scene.add(fog.mesh);

  /* ---------- Holes ---------- */
  const segGeo = new THREE.CylinderGeometry(1, 1, 1, 12, 1, true);
  const traces = new THREE.InstancedMesh(segGeo, new THREE.MeshBasicMaterial({ toneMapped: false, fog: false }), MAX_SEGS);
  const halos = new THREE.InstancedMesh(
    segGeo,
    new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.3, blending: THREE.AdditiveBlending, depthWrite: false, toneMapped: false, fog: false }),
    MAX_SEGS,
  );
  // The same traces again, faintly, wherever a block hides them: the constellation never disappears.
  const ghosts = new THREE.InstancedMesh(
    segGeo,
    new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.5, depthWrite: false, depthFunc: THREE.GreaterDepth, toneMapped: false, fog: false }),
    MAX_SEGS,
  );
  traces.renderOrder = 1;
  ghosts.renderOrder = 3;
  halos.renderOrder = 4;
  const collarGeo = new THREE.RingGeometry(0.9, 1.9, 28);
  collarGeo.rotateX(-Math.PI / 2);
  const collars = new THREE.InstancedMesh(
    collarGeo,
    new THREE.MeshBasicMaterial({ color: "#fff1dc", toneMapped: false, transparent: true, opacity: 0.95, depthWrite: false, polygonOffset: true, polygonOffsetFactor: -4, polygonOffsetUnits: -4 }),
    120,
  );
  collars.renderOrder = 8;
  /*
   * A finished hole as the rig leaves it: a cement plug round the collar, and the PVC collar pipe standing out of it
   * along the hole's line, capped, for the downhole survey crew to find.
   */
  const plugs = new THREE.InstancedMesh(
    new THREE.CylinderGeometry(0.34, 0.4, 0.2, 18).translate(0, 0.02, 0),
    new THREE.MeshStandardMaterial({ color: "#a8a49b", roughness: 0.95 }),
    120,
  );
  const pipes = (() => {
    const tint = (g: THREE.BufferGeometry, hex: string) => {
      const c = new THREE.Color(hex);
      const n = g.getAttribute("position").count;
      g.setAttribute("color", new THREE.BufferAttribute(new Float32Array(n * 3).map((_, i) => [c.r, c.g, c.b][i % 3]!), 3));
      return g.toNonIndexed();
    };
    const pipe = tint(new THREE.CylinderGeometry(0.057, 0.057, 0.7, 14).translate(0, 0.25, 0), "#e9e6dc");
    const cap = tint(new THREE.CylinderGeometry(0.066, 0.066, 0.07, 14).translate(0, 0.62, 0), "#2f6fb0");
    return new THREE.InstancedMesh(mergeGeometries([pipe, cap])!, new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.55 }), 120);
  })();
  for (const m of [plugs, pipes]) {
    m.count = 0;
    m.castShadow = m.receiveShadow = true;
    m.frustumCulled = false;
    s.scene.add(m);
  }
  for (const m of [traces, ghosts, halos, collars]) {
    m.count = 0;
    m.frustumCulled = false;
    s.scene.add(m);
  }

  const m4 = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const v3 = new THREE.Vector3();
  const sc = new THREE.Vector3();
  const UP = new THREE.Vector3(0, 1, 0);

  /** The proposed hole: marching dashes in the agent's green down the planned trace, and a ring at its collar. */
  const DASH = 9;
  const GAP = 7;
  const dashes = new THREE.InstancedMesh(
    new THREE.CylinderGeometry(1, 1, 1, 8, 1, false),
    new THREE.MeshBasicMaterial({ color: new THREE.Color(AGENT).multiplyScalar(1.3), toneMapped: false, fog: false }),
    80,
  );
  dashes.count = 0;
  dashes.frustumCulled = false;
  s.scene.add(dashes);
  const propRing = new THREE.Mesh(
    new THREE.RingGeometry(7, 9, 48).rotateX(-Math.PI / 2),
    new THREE.MeshBasicMaterial({ color: AGENT, toneMapped: false, transparent: true, opacity: 0.95, depthWrite: false, polygonOffset: true, polygonOffsetFactor: -4, polygonOffsetUnits: -4 }),
  );
  propRing.renderOrder = 9;
  propRing.visible = false;
  s.scene.add(propRing);

  /* ---------- The rig, the kit on its pad, and the crew ---------- */
  lap("blocks+holes");
  await nextFrame();
  tt = performance.now();
  const rig = makeCoreRig();
  lap("rig");
  await nextFrame();
  tt = performance.now();
  const kit = makePadKit();
  lap("kit");
  await nextFrame();
  tt = performance.now();
  // The pad's frame: origin on the ground under the mast pivot, +z the hole's heading.
  const padG = new THREE.Group();
  padG.add(kit.group);
  const geologist = makeMiner({ role: "geologist", seed: 4, detail: small ? 1 : 2, board: "tablet" });
  const driller = makeMiner({ role: "operator", seed: 9, detail: small ? 1 : 2 });
  const offsider = makeMiner({ role: "operator", seed: 3, detail: small ? 1 : 2 });
  const pickup = makePickup();
  pickup.group.traverse((o) => {
    if ((o as THREE.Mesh).isMesh) o.castShadow = true;
  });
  // Compile their shaders off the main thread where the browser can, before they join the scene.
  const arrivals = [rig.group, padG, geologist.group, driller.group, offsider.group, pickup.group];
  {
    const holder = new THREE.Group();
    holder.add(...arrivals);
    lap("crew");
    await s.renderer.compileAsync(holder, s.camera, s.scene).catch(() => {});
    tt = performance.now();
    holder.remove(...arrivals);
  }
  s.scene.add(...arrivals);
  lap("compile");

  const headingOf = (h: Hole) => {
    const b = THREE.MathUtils.degToRad(h.bearing);
    return new THREE.Vector3(Math.sin(b), 0, -Math.cos(b));
  };
  /** Where the rig's origin stands for a hole, and its yaw: the rod line through the collar at the hole's dip. */
  const padFrame = (h: Hole) => {
    const head = headingOf(h);
    const at = h.collar.clone().addScaledVector(head, -collarAhead(h.dip));
    at.y = h.collar.y - 0.01;
    return { at, yaw: Math.atan2(head.x, head.z), head };
  };
  const inPad = (x: number, y: number, z: number) => padG.localToWorld(new THREE.Vector3(x, y, z));
  /** Sets the kit and the crew on a hole's pad, around the rig. */
  const setPad = (h: Hole) => {
    const f = padFrame(h);
    padG.position.copy(f.at);
    padG.rotation.set(0, f.yaw, 0);
    padG.updateMatrixWorld(true);
    kit.collar(h.dip);
    const c = collarAhead(h.dip);
    driller.group.position.copy(inPad(-1.95, 0.72, -0.5));
    driller.face(inPad(-1.0, 0.72, -0.5));
    driller.work("stand");
    offsider.group.position.copy(inPad(1.05, 0, c - 0.5));
    offsider.face(inPad(0, 0, c));
    offsider.work("stand");
    geologist.group.position.copy(inPad(6.5, 0, 1.15));
    geologist.face(inPad(6.5, 0, 2.4));
    geologist.work("inspect", inPad(6.5, 0.95, 1.85));
    pickup.group.position.copy(inPad(-8.6, 0, -5.2));
    pickup.group.rotation.y = f.yaw + 0.5;
    kit.rods(RACK_RODS);
    kit.trays(Math.floor(h.length / OPS.trayM));
  };
  /** Puts the rig over a collar, set up and drilling-ready (or with its mast at `mast` degrees). */
  const placeRig = (h: Hole, mast = h.dip) => {
    const f = padFrame(h);
    rig.group.position.copy(f.at);
    rig.group.rotation.set(0, f.yaw, 0);
    rig.set({ dip: h.dip, mast, slide: mast >= h.dip ? 1 : 0, jacks: 1, head: 0.75, rods: mast >= h.dip, spin: 0, lights: s.night > 0.3 });
  };

  /* ---------- Labels ---------- */
  const T = tags(s, () => {
    const list: Element[] = [];
    for (const el of view.querySelectorAll(".hud-card, .drill-legend, .drill-status.on")) if (el.parentElement === view) list.push(el);
    return list;
  });
  for (const dy of [100, 200, 300]) T.add(`${dy} m`, new THREE.Vector3(X0, -dy, Z1), { prio: 4, className: "drill-depth" });

  /* ---------- State ---------- */
  type Est = { mean: Float64Array; sd: Float64Array };
  let holes: Hole[] = [];
  let est: Est = { mean: Float64Array.from(P.prior), sd: new Float64Array(NB).fill(SD_MAX) };
  // What the blocks show: eases from `from` to `est` after each update.
  const shown: Est = { mean: Float64Array.from(P.prior), sd: new Float64Array(NB).fill(SD_MAX) };
  const from: Est = { mean: Float64Array.from(P.prior), sd: new Float64Array(NB).fill(SD_MAX) };
  let settle = 1;
  let iv: Interval = containedOz(est.mean, est.sd);
  const priorIv = containedOz(P.prior, new Float64Array(NB).fill(SD_MAX));
  let hist: { n: number; iv: Interval }[] = [];
  let mode: Mode = "estimate";
  let busy = false;
  let job = 0;
  let engineMs = 0;
  let priceMs = 0;
  let refused = 0;
  type Proposal = { hole: Hole; zoneNow: Interval; zoneAfter: Interval; evsi: number; net: number; cost: number };
  let proposal: Proposal | null = null;
  let idTags: Tag[] = [];
  let newest: Tag | null = null;
  let propTag: Tag | null = null;
  const prior = (x: number, y: number, z: number) => P.body.prior(x, y, z);

  const startHoles = () => {
    const off = 160 / Math.tan(THREE.MathUtils.degToRad(DIP));
    const dx = -Math.sin(THREE.MathUtils.degToRad(BEARING)) * off;
    const dz = Math.cos(THREE.MathUtils.degToRad(BEARING)) * off;
    const at = (x: number, z: number) => new THREE.Vector3(x, ground(x, z), z);
    const list = [
      designHole(1, at(-170, -120), BEARING, DIP, "plan"),
      designHole(2, at(36 + dx, 13 + dz), BEARING, DIP, "plan"),
      designHole(3, at(96 + dx, -27 + dz), BEARING, DIP, "plan"),
      designHole(4, at(-24 + dx, 53 + dz), BEARING, DIP, "plan"),
      designHole(5, at(0, -40), 0, 90, "plan"),
    ];
    list.forEach((h, i) => assay(h, P.body, i + 1));
    return list;
  };

  /* ---------- Drawing ---------- */
  const col = new THREE.Color();
  /** Rebuilds the traces, to `upto` metres down the last hole (all of every other). */
  const drawHoles = (upto = Infinity) => {
    let n = 0;
    let c = 0;
    let p = 0;
    holes.forEach((h, hi) => {
      const last = hi === holes.length - 1;
      q.setFromUnitVectors(UP, h.dir);
      for (const cmp of h.composites) {
        if (last && cmp.from >= upto) break;
        if (n >= MAX_SEGS) break;
        const to = last ? Math.min(cmp.to, upto) : cmp.to;
        const len = to - cmp.from;
        v3.copy(h.collar).addScaledVector(h.dir, (cmp.from + to) / 2);
        // A downhole log: thin and grey in barren rock, thicker and brighter with grade.
        const g = cmp.g;
        const ore = g >= CUTOFF;
        const lowish = g >= 0.15;
        const r = ore ? clamp(1.7 + 1.1 * Math.log2(g / CUTOFF), 1.7, 4.2) : lowish ? 1.15 : 0.75;
        m4.compose(v3, q, sc.set(r, len, r));
        traces.setMatrixAt(n, m4);
        gradeColour(g, col);
        if (!lowish) col.lerp(BARREN, 0.75);
        const away = offSlab(v3) ? 0.22 : 1;
        traces.setColorAt(n, col.clone().multiplyScalar((ore ? 2 : lowish ? 1.2 : 0.55) * away));
        const hr = ore ? r * 3 : lowish ? 3.4 : 2;
        m4.compose(v3, q, sc.set(hr, len, hr));
        halos.setMatrixAt(n, m4);
        halos.setColorAt(n, col.multiplyScalar((ore ? 0.9 : lowish ? 0.22 : 0.04) * away));
        n++;
      }
      m4.compose(v3.copy(h.collar).setY(h.collar.y + 0.3), q.identity(), sc.setScalar(last ? 1.4 : 1));
      collars.setMatrixAt(c++, m4);
      // The rig stands over the newest hole, with its own casing; the others are plugged and capped.
      if (!last) {
        plugs.setMatrixAt(p, m4.compose(h.collar, q.identity(), sc.setScalar(1)));
        q.setFromUnitVectors(UP, v3.copy(h.dir).negate());
        pipes.setMatrixAt(p++, m4.compose(h.collar, q, sc.setScalar(1)));
      }
    });
    plugs.count = pipes.count = p;
    plugs.instanceMatrix.needsUpdate = pipes.instanceMatrix.needsUpdate = true;
    traces.count = halos.count = ghosts.count = n;
    collars.count = c;
    ghosts.instanceMatrix = traces.instanceMatrix;
    ghosts.instanceColor = traces.instanceColor;
    for (const m of [traces, halos, collars]) {
      m.instanceMatrix.needsUpdate = true;
      if (m.instanceColor) m.instanceColor.needsUpdate = true;
    }
    s.loop.poke();
  };

  /** Pads, tracks and hole tags for the holes as they stand. */
  const site = () => {
    const route = G.site(holes.map((h, i) => ({ x: h.collar.x, z: h.collar.z, y: h.collar.y, bearing: h.bearing, newest: i === holes.length - 1 })));
    for (const t of idTags) t.remove();
    idTags = holes.slice(0, -1).map((h) => T.add(h.id, h.collar.clone(), { prio: 10 + Number(h.id.slice(3)) / 100, className: "drill-id" }));
    return route;
  };

  /*
   * The section: a slab of the model 60 m thick along the newest hole, its plane holding the hole and the vertical,
   * looked at square on, as a geologist draws a cross-section. Blocks and fog outside it are left out, and holes
   * away from it fade.
   */
  const SLAB = 30;
  let sectionOn = false;
  const slab = { n: new THREE.Vector3(1, 0, 0), d: 0, hole: null as Hole | null };
  const inSlab = new Uint8Array(NB).fill(1);
  const setSlab = () => {
    const h = holes[holes.length - 1];
    if (!h) return;
    const head = headingOf(h);
    slab.n.set(-head.z, 0, head.x);
    slab.d = slab.n.dot(h.collar);
    slab.hole = h;
    for (let b = 0; b < NB; b++) {
      const dd = slab.n.x * P.centres[b * 3]! + slab.n.z * P.centres[b * 3 + 2]! - slab.d;
      inSlab[b] = !sectionOn || Math.abs(dd) <= SLAB ? 1 : 0;
    }
    fog.slab(sectionOn ? slab.n : null, slab.d, SLAB);
  };
  const offSlab = (p: THREE.Vector3) => sectionOn && Math.abs(slab.n.dot(p) - slab.d) > SLAB + 10;
  const sectionView = (h = slab.hole ?? holes[holes.length - 1]!) => {
    const head = headingOf(h);
    let az = THREE.MathUtils.radToDeg(Math.atan2(head.z === 0 && head.x === 0 ? 1 : -head.z, head.x));
    const off = ((((az - HOME().azimuth + 180) % 360) + 360) % 360) - 180;
    if (Math.abs(off) > 90) az += 180;
    const tgt = h.collar.clone().addScaledVector(h.dir, h.length * 0.5);
    const dist = small ? 980 : 780;
    const lowest = THREE.MathUtils.radToDeg(Math.asin(clamp((h.collar.y + 30 - tgt.y) / dist, 0, 0.95)));
    return { target: tgt, distance: dist, elevation: Math.max(12, lowest), azimuth: az };
  };

  const colour = new THREE.Color();
  const zero = new THREE.Matrix4().makeScale(0, 0, 0);
  const black = new THREE.Color(0, 0, 0);
  const lnLo = Math.log(0.25);
  const lnHi = Math.log(4);
  const uOf = (sd: number) => clamp((sd - SD_MIN) / (SD_MAX - SD_MIN), 0, 1);
  const uBuf = new Float32Array(NB);
  /** Paints the model for the current mode: ore as solid blocks and lower grades as a glow, or uncertainty as fog. */
  const paintBlocks = () => {
    const fogMode = mode === "sd";
    fog.mesh.visible = fogMode;
    solid.visible = glow.visible = !fogMode;
    if (fogMode) {
      for (let b = 0; b < NB; b++) uBuf[b] = uOf(shown.sd[b]!);
      fog.set(uBuf);
      s.loop.poke();
      return;
    }
    for (let b = 0; b < NB; b++) {
      if (!inSlab[b]) {
        solid.setMatrixAt(b, zero);
        glow.setMatrixAt(b, zero);
        continue;
      }
      v3.set(P.centres[b * 3]!, P.centres[b * 3 + 1]!, P.centres[b * 3 + 2]!);
      const f = flash[b]!;
      let glowSize = 0;
      let solidSize = 0;
      // The block's median grade: the truth, or the estimate at block support.
      const g = mode === "truth" ? Math.exp(P.truth[b]!) : Math.exp(blockLn(shown.mean[b]!, shown.sd[b]!)[0]);
      const k = clamp((Math.log(g) - lnLo) / (lnHi - lnLo), 0, 1);
      if (g >= CUTOFF) {
        solidSize = B * (0.5 + 0.32 * Math.sqrt(k));
        gradeColour(g, colour).multiplyScalar(0.75 + 0.5 * k);
        if (f > 0) colour.lerp(FLASH_SOLID, f * 0.35);
        solid.setColorAt(b, colour);
        glowSize = B * 1.05;
        gradeColour(g, colour).multiplyScalar(0.05 + 0.08 * k);
      } else if (g >= 0.25) {
        glowSize = B * (0.55 + 0.4 * k);
        gradeColour(g, colour).multiplyScalar(0.035 + 0.1 * k);
      }
      solid.setMatrixAt(b, solidSize > 0 ? m4.compose(v3, q.identity(), sc.setScalar(solidSize)) : zero);
      if (solidSize <= 0) solid.setColorAt(b, black);
      glow.setMatrixAt(b, glowSize > 0 ? m4.compose(v3, q.identity(), sc.setScalar(glowSize)) : zero);
      glow.setColorAt(b, glowSize > 0 ? colour : black);
    }
    for (const m of [solid, glow]) {
      m.instanceMatrix.needsUpdate = true;
      if (m.instanceColor) m.instanceColor.needsUpdate = true;
    }
    s.loop.poke();
  };
  /** Starts easing what is shown towards the estimate as it now stands. */
  const ease = () => {
    from.mean.set(shown.mean);
    from.sd.set(shown.sd);
    settle = reduced ? 1 : 0;
    if (reduced) {
      shown.mean.set(est.mean);
      shown.sd.set(est.sd);
    }
    paintBlocks();
  };

  /* ---------- HUD ---------- */
  const legend = () => {
    const ramp = ui("ramp");
    const sec = sectionOn && slab.hole ? `<span class="drill-sec">Section along <span>${slab.hole.id}</span>, ${2 * SLAB} m thick</span>` : "";
    if (mode === "sd") {
      ui("legend-title").innerHTML = `Uncertainty<span class="why">: clear near the holes, fog far from them</span>` + sec;
      ramp.style.background = "linear-gradient(90deg, #0d1024 0%, #9fdcff 22%, #3a2c8a 48%, #4a33b4 72%, #8f68f0 100%)";
      ui("ticks").innerHTML = `<span>known</span><span class="far"><i>as </i>unknown<i> as the prior</i></span>`;
    } else {
      ui("legend-title").innerHTML = (mode === "truth" ? "True grade, g/t (hidden from the model)" : "Estimated grade, g/t") + sec;
      ramp.style.background = `linear-gradient(90deg, ${gradeGradient(0.15, 4)})`;
      ui("ticks").innerHTML = `<span>0.15</span><span>0.3</span><span class="cut">${CUTOFF} cut-off</span><span>1</span><span>2</span><span>4</span>`;
    }
  };

  /** The interval's history: P10–P90 band and median against holes drilled, on a log scale, with the prior at 0. */
  const fan = () => {
    const Wv = 300;
    const Hv = 78;
    const L = 6;
    const R = 34;
    const Tp = 6;
    const Bt = 16;
    const lo = Math.log(8);
    const hi = Math.log(900);
    const last = hist[hist.length - 1];
    const n = Math.max(14, (last?.n ?? 0) + 3);
    const X = (k: number) => L + (k / n) * (Wv - L - R);
    const Y = (oz: number) => Tp + (1 - (Math.log(clamp(oz / 1000, 8, 900)) - lo) / (hi - lo)) * (Hv - Tp - Bt);
    const pts = [{ n: 0, iv: priorIv }, ...hist];
    const top = pts.map((p) => `${X(p.n).toFixed(1)},${Y(p.iv.p90).toFixed(1)}`);
    const bot = pts.map((p) => `${X(p.n).toFixed(1)},${Y(p.iv.p10).toFixed(1)}`).reverse();
    const mid = pts.map((p) => `${X(p.n).toFixed(1)},${Y(p.iv.p50).toFixed(1)}`).join(" ");
    const grid = [10, 100]
      .map((v) => `<line x1="${L}" x2="${Wv - R + 4}" y1="${Y(v * 1000)}" y2="${Y(v * 1000)}" class="g"/><text x="${Wv - R + 7}" y="${Y(v * 1000) + 3}">${v}</text>`)
      .join("");
    const ticks = [0, 5, 10, 15, 20, 25, 30]
      .filter((k) => k <= n)
      .map((k) => `<text x="${X(k)}" y="${Hv - 3}" text-anchor="middle">${k === 0 ? "prior" : k}</text>`)
      .join("");
    const dots = pts.map((p) => `<circle cx="${X(p.n).toFixed(1)}" cy="${Y(p.iv.p50).toFixed(1)}" r="${p.n === 0 ? 2 : 2.4}"/>`).join("");
    const truth =
      mode === "truth"
        ? `<line x1="${L}" x2="${Wv - R + 4}" y1="${Y(P.truthOz)}" y2="${Y(P.truthOz)}" class="t"/><text x="${L + 2}" y="${Y(P.truthOz) - 3}" class="t">true ${fmt(P.truthOz / 1000)} koz</text>`
        : "";
    ui("fan").innerHTML = `<svg viewBox="0 0 ${Wv} ${Hv}" class="chart" role="img" aria-label="Contained gold, P10 to P90, as holes are added">
${grid}<text x="${Wv - R + 7}" y="${Tp + 4}">koz</text>${ticks}
<polygon points="${top.concat(bot).join(" ")}" class="band"/>
<polyline points="${mid}" class="mid"/>${dots}${truth}</svg>`;
  };

  const SCALE_OZ = 500_000;
  const hud = () => {
    ui("p50").textContent = koz(iv.p50);
    ui("int").textContent = `${koz(iv.p10)} to ${koz(iv.p90)}`;
    const band = ui("band");
    band.style.left = `${clamp(iv.p10 / SCALE_OZ, 0, 1) * 100}%`;
    band.style.width = `${clamp((iv.p90 - iv.p10) / SCALE_OZ, 0, 1 - iv.p10 / SCALE_OZ) * 100}%`;
    band.style.setProperty("--mid", `${clamp((iv.p50 - iv.p10) / Math.max(1, iv.p90 - iv.p10), 0, 1) * 100}%`);
    const truth = ui("truth");
    truth.style.left = `${clamp(P.truthOz / SCALE_OZ, 0, 1) * 100}%`;
    truth.hidden = mode !== "truth";
    const tl = ui("truthline");
    tl.hidden = mode !== "truth";
    const inside = P.truthOz >= iv.p10 && P.truthOz <= iv.p90;
    tl.innerHTML = `True gold, hidden from the model: <b>${koz(P.truthOz)}</b>, ${inside ? "inside" : "outside"} the P10–P90.`;
    const metres = holes.reduce((a, h) => a + h.length, 0);
    const assays = holes.reduce((a, h) => a + h.composites.length, 0);
    ui("holes").textContent = fmt(holes.length);
    ui("metres").textContent = `${fmt(metres)} m`;
    ui("assays").textContent = fmt(assays);
    ui("spent").textContent = money(metres * COST_PER_M);
    ui("days").textContent = fmt(rigDays, 1);
    ui("blocks").textContent = fmt(NB);
    fan();
  };
  const status = (s2: string) => {
    ui("status").textContent = s2;
    ui("status").classList.toggle("on", !!s2);
  };

  /** "18 m at 2.4 g/t from 212 m": the best run of composites above the cut-off, by gram-metres. */
  const intercept = (h: Hole) => {
    let best = { gm: 0, from: 0, len: 0, g: 0 };
    let run = { gm: 0, from: 0, len: 0 };
    for (const c of h.composites) {
      if (c.g >= CUTOFF) {
        if (!run.len) run = { gm: 0, from: c.from, len: 0 };
        run.gm += c.g * (c.to - c.from);
        run.len += c.to - c.from;
        if (run.gm > best.gm) best = { ...run, g: run.gm / run.len };
      } else run = { gm: 0, from: 0, len: 0 };
    }
    return best.len ? `<b>${fmt(best.len)} m at ${fmt(best.g, 2)} g/t</b> from ${fmt(best.from)} m` : `<b>nothing above ${CUTOFF} g/t</b>`;
  };

  const pinNewest = () => {
    newest?.remove();
    newest = null;
    const h = holes[holes.length - 1];
    if (!h) return;
    newest = T.add(`${h.id}${h.by === "person" ? " (yours)" : ""} · ${intercept(h)}`, h.collar.clone(), { prio: 80, lift: 30, className: "stem drill-pin" });
  };

  const agentHtml = () => {
    const el = ui("agent");
    if (!proposal) {
      el.innerHTML = `<p>Ranking collars by how much of the gold interval lies near each one…</p>`;
      drillBtn.textContent = "Approve and drill";
      return;
    }
    const p = proposal;
    const h = p.hole;
    const w = (iv2: Interval) => koz(iv2.p90 - iv2.p10);
    const worth = p.net > 0;
    drillBtn.textContent = `Approve and drill ${h.id}`;
    el.innerHTML = `<p class="drill-next"><b>${h.id}</b> · ${h.dip === 90 ? "vertical" : `−${h.dip}° to ${deg3(h.bearing)}`} · ${fmt(h.length)} m · ${fmt(gridE(h.collar.x))} E ${fmt(gridN(h.collar.z))} N</p>
<p>Aimed where the model is least sure: it narrows the P10–P90 of the ground it tests from <b>${w(p.zoneNow)}</b> to <b>${w(p.zoneAfter)}</b> wide.</p>
<p>${
      worth
        ? `That sharpens the call on drilling the ground out; the engine prices it at <b>${money(p.evsi)}</b>, for <b>${money(p.cost)}</b> of drilling.`
        : `That sharpens the call on drilling the ground out, but the engine prices it at only <b>${money(p.evsi)}</b>, less than the <b>${money(p.cost)}</b> it costs. I would pause here.`
    }</p>
<span class="act">${worth ? "Drafted for the exploration manager to approve" : "Drafted: pause the programme, for the exploration manager"}</span>`;
  };

  /* ---------- Engine work ---------- */
  const queryOf = (list: ArrayLike<number>) => {
    const out = new Float64Array(list.length * 3);
    for (let i = 0; i < list.length; i++) out.set(P.centres.subarray(list[i]! * 3, list[i]! * 3 + 3), i * 3);
    return out;
  };
  /** Estimates the listed blocks against `samples` in chunks across frames, writing into `into`. */
  const estimateBlocks = async (list: ArrayLike<number>, samples: Sample[], into: Est, token: number, each?: (done: number, from: number) => void): Promise<number | null> => {
    let ms = 0;
    let refusedNow = 0;
    // Each block is predicted on its own, so the chunks only set how long the page waits: each is sized to take about
    // 24 ms on this machine, so the page never stalls for long.
    let size = 96;
    // An engine that cannot load fails here, as before; only a chunk the crate refused outright is caught below.
    await ready();
    if (token !== job) return null;
    let first: unknown = null;
    let answered = false;
    for (let o = 0; o < list.length; ) {
      const part = Array.from({ length: Math.min(size, list.length - o) }, (_, i) => list[o + i]!);
      const t0 = performance.now();
      let r: Awaited<ReturnType<typeof estimate>> | null = null;
      try {
        r = await estimate({ ...VARIOGRAM, samples, prior: part.map((b) => P.prior[b]!), queries: queryOf(part) });
      } catch (error) {
        // Every block in this chunk refused (the crate fails a call only then): they keep their last values.
        if (!(error instanceof EngineError)) throw error;
        first ??= error;
      }
      answered ||= r !== null;
      if (token !== job) return null;
      ms += r ? r.ms : performance.now() - t0;
      refusedNow += r ? r.refused : part.length;
      if (r)
        part.forEach((b, i) => {
          const m = r.mean[i]!;
          const sd = r.sd[i]!;
          if (Number.isFinite(m) && Number.isFinite(sd)) {
            into.mean[b] = m;
            into.sd[b] = sd;
          }
        });
      each?.(o + part.length, o);
      o += part.length;
      size = Math.max(32, Math.min(CHUNK, Math.round((part.length * 24) / Math.max(1, performance.now() - t0))));
      await nextFrame();
      if (token !== job) return null;
    }
    // Refused everywhere is a failure, as one big call would report it.
    if (!answered && first) throw first;
    refused = refusedNow;
    return ms;
  };

  const share = new Float64Array(NB);
  /** Ranks collars, then has the engine price the best two and keeps the one worth more. */
  const propose = async (token: number) => {
    proposal = null;
    agentHtml();
    showProposal();
    widthShare(est.mean, est.sd, share);
    await nextFrame();
    const cands = rankCandidates(share, P.centres, holes, ground, 2);
    const samples = samplesOf(holes, prior);
    let best = null as Proposal | null;
    let spent = 0;
    for (const c of cands) {
      await nextFrame();
      if (token !== job) return;
      const h = designHole(holes.length + 1, c.collar, c.bearing, c.dip, "agent");
      const pts = compositeCentres(h);
      // The hole's assays are not known yet; the predictor's sd does not depend on them, so stand in the mean.
      const pm = await estimate({ ...VARIOGRAM, samples, prior: pts.map((p) => prior(...p)), queries: pts.flat() });
      if (token !== job) return;
      spent += pm.ms;
      const hyp: Sample[] = pts.map((p, i) => ({ x: p[0], y: p[1], z: p[2], v: pm.mean[i]!, prior: prior(...p) }));
      const near = blocksNear(h, P.centres, 2);
      const afterSd = Float64Array.from(est.sd);
      const ms2 = await estimateBlocks(near, samples.concat(hyp), { mean: new Float64Array(NB), sd: afterSd }, token);
      if (ms2 === null) return;
      spent += ms2;
      const zone = blocksNear(h, P.centres, 1);
      const zoneNow = containedOz(est.mean, est.sd, zone);
      const zoneAfter = containedOz(est.mean, afterSd, zone);
      const cost = h.length * COST_PER_M;
      const tv = performance.now();
      const v = await voi(holeDecision(zoneNow, zoneAfter, cost, drillOutCost(zone.length)));
      spent += performance.now() - tv;
      if (token !== job) return;
      const evsi = v.evsi ?? 0;
      const cand: Proposal = { hole: h, zoneNow, zoneAfter, evsi, net: v.netVoi ?? evsi - cost, cost };
      if (!best || cand.net > best.net) best = cand;
    }
    proposal = best;
    priceMs = spent;
    agentHtml();
    showProposal();
    engineLine(lastN);
    drillBtn.disabled = !proposal || busy;
  };

  const showProposal = () => {
    propTag?.remove();
    propTag = null;
    if (!proposal) {
      propRing.visible = false;
      dashes.count = 0;
      return;
    }
    const h = proposal.hole;
    propRing.position.copy(h.collar).setY(h.collar.y + 0.5);
    propRing.visible = true;
    propTag = T.add(`Proposed ${h.id} · <b>worth ${money(proposal.evsi)}</b>`, h.collar.clone(), { prio: 90, lift: 44, className: "stem drill-prop" });
    s.loop.poke();
  };
  const drawDashes = (t: number) => {
    if (!proposal) return;
    const h = proposal.hole;
    const step = DASH + GAP;
    const off = (t * 14) % step;
    let n = 0;
    q.setFromUnitVectors(UP, h.dir);
    for (let a = off - step; a < h.length && n < 80; a += step) {
      const s0 = Math.max(0, a);
      const s1 = Math.min(h.length, a + DASH);
      if (s1 <= s0) continue;
      v3.copy(h.collar).addScaledVector(h.dir, (s0 + s1) / 2);
      dashes.setMatrixAt(n++, m4.compose(v3, q, sc.set(1.3, s1 - s0, 1.3)));
    }
    dashes.count = n;
    dashes.instanceMatrix.needsUpdate = true;
  };

  /* ---------- Drilling, as the rig really does it, shown faster ---------- */
  type Phase = "tram" | "setup" | "drill" | "done";
  type Anim = {
    hole: Hole;
    plan: Plan;
    phase: Phase;
    t: number;
    /** Tram path (world xz) and its length. */
    path: THREE.Vector2[];
    len: number;
    tramS: number;
    yaw0: number;
    yaw1: number;
    /** The rig's yaw on arriving at the pad, before it turns onto the bearing. */
    yaw0Pad?: number;
    sectioned?: boolean;
    /** Real seconds for the coring, and the curve that maps it onto rig hours. */
    coreS: number;
    alpha: number;
    depth: number;
  };
  let rigTag: Tag | null = null;
  /** The bit, as a spark at the bottom of the hole, for when the camera has pulled back to the section. */
  const bit = new THREE.Mesh(
    new THREE.SphereGeometry(1, 16, 12),
    new THREE.MeshBasicMaterial({ color: "#fff2cf", transparent: true, opacity: 0.95, depthTest: false, depthWrite: false, toneMapped: false, fog: false }),
  );
  bit.renderOrder = 9;
  bit.visible = false;
  s.scene.add(bit);
  let since = 0;
  let rigDays = 0;
  const TRAM_GAP = 150;
  /** The close-up looks at the rig from in front and to the right of its heading, past the logging bench. */
  const CLOSE_AZ = 36;

  /** Arc length along a polyline to a point and its heading. */
  const along = (pts: THREE.Vector2[], d: number) => {
    for (let i = 0; i < pts.length - 1; i++) {
      const a = pts[i]!;
      const b = pts[i + 1]!;
      const l = a.distanceTo(b);
      if (d <= l || i === pts.length - 2) {
        const f = clamp(d / Math.max(l, 1e-6), 0, 1);
        return { x: mix(a.x, b.x, f), z: mix(a.y, b.y, f), yaw: Math.atan2(b.x - a.x, b.y - a.y) };
      }
      d -= l;
    }
    return { x: pts[0]!.x, z: pts[0]!.y, yaw: 0 };
  };
  const turn = (a: number, b: number, f: number) => a + ((((b - a + Math.PI) % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI) - Math.PI) * f;
  /** The exponential that spends the first 30% of the coring's real time on the first run, the rest on all others. */
  const alphaFor = (first: number) => {
    let lo = 0.1;
    let hi = 60;
    for (let i = 0; i < 60; i++) {
      const a = (lo + hi) / 2;
      if ((Math.exp(0.3 * a) - 1) / (Math.exp(a) - 1) > first) lo = a;
      else hi = a;
    }
    return (lo + hi) / 2;
  };
  const hoursAt = (a: Anim, u: number) => (a.plan.coreH * (Math.exp(a.alpha * u) - 1)) / (Math.exp(a.alpha) - 1);

  const drill = async (h: Hole) => {
    if (busy) return;
    busy = true;
    drillBtn.disabled = resetBtn.disabled = true;
    const token = ++job;
    proposal = null;
    agentHtml();
    showProposal();
    assay(h, P.body, holes.length + 1);
    const from = rig.group.position.clone();
    holes.push(h);
    const route = site();
    if (sectionOn) {
      setSlab();
      paintBlocks();
      legend();
    }
    const f = padFrame(h);
    const path = [new THREE.Vector2(from.x, from.z), ...route.slice(0, -1), new THREE.Vector2(f.at.x, f.at.z)];
    let len = 0;
    for (let i = 0; i < path.length - 1; i++) len += path[i]!.distanceTo(path[i + 1]!);
    const pl = plan(h, len);
    rigDays += pl.days;
    rigTag?.remove();
    rigTag = T.add(`${h.id}`, rig.anchors.crown, { prio: 100, lift: 30, className: "stem drill-rig" });
    newest?.remove();
    newest = null;
    if (reduced) {
      anim = null;
      setPad(h);
      placeRig(h);
      kit.rods(RACK_RODS - Math.ceil(h.length / OPS.rodM));
      drawHoles();
      status(`${h.id}: ${fmt(h.length)} m cored in ${fmt(pl.days, 1)} rig days`);
    } else {
      // Mast down and jacks up for the tram; the hole's trace stays hidden until the bit cuts it.
      rig.set({ mast: 0, slide: 0, jacks: 0, rods: false, spin: 0, head: 1 });
      drawHoles(0);
      const first = pl.runs[0]!.hours / pl.coreH;
      const yaw0 = rig.group.rotation.y;
      anim = {
        hole: h,
        plan: pl,
        phase: "tram",
        t: 0,
        path,
        len,
        tramS: clamp(len / 110, 1.4, 2.8),
        yaw0,
        yaw1: f.yaw,
        coreS: 5.2,
        alpha: alphaFor(first),
        depth: 0,
      };
      s.shadowBox(from, 60);
      while (anim && anim.phase !== "done") {
        await nextFrame();
        if (token !== job) return;
      }
    }
    s.shadowBox([0, 0, 0], 460);
    bit.visible = false;
    rigTag?.remove();
    rigTag = null;
    pinNewest();
    hud();
    // The assays are back: the blocks within two ranges of the hole are estimated again.
    status(`Assays back: ${fmt(h.composites.length)} composites; estimating the blocks near ${h.id}`);
    await calibrate(h).catch(() => (ui("calib").hidden = true));
    if (token !== job) return;
    if (!reduced) s.orbit.set(sectionOn ? sectionView() : HOME(), 2.4);
    const near = blocksNear(h, P.centres, 2);
    const before = Float64Array.from(est.sd);
    const ms = await estimateBlocks(near, samplesOf(holes, prior), est, token, (done, from) => {
      for (let i = from; i < done; i++) {
        const b = near[i]!;
        flash[b] = clamp((before[b]! - est.sd[b]!) * 3, 0, 1);
      }
      ease();
    });
    if (ms === null) return;
    engineMs = ms;
    iv = containedOz(est.mean, est.sd);
    hist.push({ n: holes.length, iv });
    hud();
    engineLine(near.length);
    status("");
    busy = false;
    resetBtn.disabled = false;
    await propose(token);
  };

  /*
   * Was the model honest about the hole? Its assays, top down, are each scored against the forecast the model made
   * for that spot before it was cut (from the holes before it and the assays above it), by fmine-calib's
   * SpatialFrontPitMonitor: where each assay falls in its forecast (the PIT), and two e-processes betting that the
   * forecasts are off in centre or in spread, either way. An e-process that reaches 2/α (40) raises the drift alarm.
   */
  const calibrate = async (h: Hole) => {
    const t0 = performance.now();
    const mon = await monitor({ ...VARIOGRAM, alpha: 0.05 });
    let n = 0;
    let inside = 0;
    let maxLog = 0;
    let logT = Math.log(40);
    let alarm = false;
    let refusedN = 0;
    let code: string | undefined;
    try {
      mon.condition(samplesOf(holes.filter((k) => k !== h), prior));
      for (const smp of samplesOf([h], prior)) {
        try {
          const o = mon.observe(smp.x, smp.y, smp.z, smp.prior!, smp.v);
          if (!Number.isFinite(o.pit)) continue;
          n++;
          if (o.pit > 0.1 && o.pit < 0.9) inside++;
          maxLog = Math.max(maxLog, o.maxLogELocation, o.maxLogEDispersion);
          logT = o.logThreshold;
          alarm ||= o.alarm;
        } catch (e) {
          if (!(e instanceof EngineError)) throw e;
          refusedN++;
          code ??= e.code;
        }
      }
    } finally {
      mon.free();
    }
    const ms = performance.now() - t0;
    const el = ui("calib");
    el.hidden = n === 0;
    const ref = refusedN ? ` ${fmt(refusedN)} refused${code ? ` (${code})` : ""}.` : "";
    const share = inside / Math.max(1, n);
    const read = share > 0.9 ? "too wide" : share < 0.7 ? "too narrow" : "about right";
    el.innerHTML =
      `${h.id}’s ${fmt(n)} assays against the forecast made before each was cut: <span class="dc-in">${fmt(inside)} inside their P10–P90</span>, against ${fmt(0.8 * n)} for a calibrated model, so here its intervals are ${read}. ` +
      (alarm
        ? `<span class="dc-bad">Drift alarm</span> (e-value ${fmt(Math.exp(maxLog))} of ${fmt(Math.exp(logT))}): flagged for the geologist to ${share > 0.9 ? "refit the variogram" : "review the model"}.`
        : `No drift alarm (e-value ${fmt(Math.exp(maxLog), 1)} of ${fmt(Math.exp(logT))}).`) +
      `${ref} Calibration monitor, ${fmt(ms, ms < 10 ? 1 : 0)} ms.`;
  };

  let lastN = NB;
  const engineLine = (n: number) => {
    lastN = n;
    const ref = refused ? ` (${fmt(refused)} refused: samples at one point)` : "";
    const priced = proposal ? ` ${proposal.hole.id} priced in ${fmt(priceMs, priceMs < 10 ? 1 : 0)} ms by its value-of-information kernel.` : "";
    ui("engine").innerHTML = `${fmt(n)} blocks estimated in ${fmt(engineMs)} ms by FrankenMining’s predictor${ref}.${priced} Computed in your browser.`;
  };

  const reset = async () => {
    const token = ++job;
    busy = true;
    drillBtn.disabled = resetBtn.disabled = true;
    anim = null;
    bit.visible = false;
    rigTag?.remove();
    rigTag = null;
    flash.fill(0);
    holes = startHoles();
    const last = holes[holes.length - 1]!;
    rigDays = holes.reduce((a, h) => a + plan(h, TRAM_GAP).days, 0);
    drawHoles();
    site();
    setPad(last);
    placeRig(last);
    pinNewest();
    proposal = null;
    agentHtml();
    showProposal();
    hist = [];
    est = { mean: Float64Array.from(P.prior), sd: new Float64Array(NB).fill(SD_MAX) };
    iv = containedOz(est.mean, est.sd);
    hud();
    status("Estimating the block model");
    setSlab();
    legend();
    s.orbit.set(sectionOn ? sectionView() : HOME(), reduced ? 0 : 1.2);
    const all = Array.from({ length: NB }, (_, b) => b);
    const ms = await estimateBlocks(all, samplesOf(holes, prior), est, token, () => ease());
    if (ms === null) return;
    engineMs = ms;
    iv = containedOz(est.mean, est.sd);
    hist = [{ n: holes.length, iv }];
    hud();
    engineLine(NB);
    await calibrate(holes[holes.length - 1]!).catch(() => (ui("calib").hidden = true));
    if (token !== job) return;
    status("");
    busy = false;
    resetBtn.disabled = false;
    await propose(token);
  };

  /* ---------- Controls ---------- */
  seg(host, "mode", (v) => {
    mode = v as Mode;
    legend();
    hud();
    paintBlocks();
  });
  drillBtn.addEventListener("click", () => {
    if (proposal) void drill(proposal.hole).catch(fail);
  });
  resetBtn.addEventListener("click", () => void reset().catch(fail));
  sectionBtn.addEventListener("click", () => {
    sectionOn = !sectionOn;
    sectionBtn.setAttribute("aria-pressed", String(sectionOn));
    setSlab();
    paintBlocks();
    drawHoles(anim ? anim.depth : Infinity);
    legend();
    if (!anim) s.orbit.set(sectionOn ? sectionView() : HOME(), reduced ? 0 : 1.6);
  });
  // A click (not a drag) on the ground inside the outline sites a hole there, −60° towards the usual bearing.
  {
    let down: { x: number; y: number; t: number } | null = null;
    const ray = new THREE.Raycaster();
    s.canvas.addEventListener("pointerdown", (e) => (down = { x: e.clientX, y: e.clientY, t: performance.now() }));
    s.canvas.addEventListener("pointerup", (e) => {
      if (!down) return;
      const moved = Math.hypot(e.clientX - down.x, e.clientY - down.y);
      const quick = performance.now() - down.t < 450;
      down = null;
      if (moved > 8 || !quick || busy) return;
      const r = s.canvas.getBoundingClientRect();
      ray.setFromCamera(new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1), s.camera);
      const hit = ray.intersectObjects(G.pickable, false)[0];
      if (!hit) return;
      if (boxDist(hit.point.x, hit.point.z) > 40) {
        status("Site holes inside the prospect’s outline");
        setTimeout(() => !busy && status(""), 1800);
        return;
      }
      const x = clamp(hit.point.x, X0 + 25, X1 - 25);
      const z = clamp(hit.point.z, Z0 + 25, Z1 - 25);
      if (holes.some((h) => Math.hypot(h.collar.x - x, h.collar.z - z) < 12)) {
        status("Too close to a hole already drilled");
        setTimeout(() => !busy && status(""), 1800);
        return;
      }
      void drill(designHole(holes.length + 1, new THREE.Vector3(x, ground(x, z), z), BEARING, DIP, "person")).catch(fail);
    });
  }
  const placeCards = () => {
    const note = $(host, ".drill-agent");
    const stats = $(host, ".drill-stats");
    const fanFig = $(host, ".drill-fan");
    if (phone.matches) {
      below.append(note, stats);
      stats.append(fanFig);
    } else {
      view.append(stats, note);
      $(host, ".drill-metal").insertBefore(fanFig, ui("truthline"));
    }
  };
  phone.addEventListener("change", () => {
    placeCards();
    s.orbit.set(HOME(), 0.6);
  });
  placeCards();

  function fail(e: unknown) {
    busy = false;
    resetBtn.disabled = false;
    const msg = e instanceof EngineError ? e.message : String(e);
    ui("engine").textContent = `The engine refused: ${msg}`;
    status("");
  }

  /* ---------- Frame ---------- */
  s.frame((dt, t) => {
    geologist.update(dt);
    driller.update(dt);
    offsider.update(dt);
    if (propRing.visible) {
      drawDashes(t);
      const k = 1 + 0.14 * Math.sin(t * 3);
      propRing.scale.set(k, 1, k);
    }
    // The model eases onto each new estimate, so the change is seen where it happens.
    if (settle < 1) {
      settle = Math.min(1, settle + dt / SETTLE);
      const e = smooth(settle);
      for (let b = 0; b < NB; b++) {
        shown.mean[b] = mix(from.mean[b]!, est.mean[b]!, e);
        shown.sd[b] = mix(from.sd[b]!, est.sd[b]!, e);
      }
      paintBlocks();
    }
    since += dt;
    if (since > 0.1) {
      let any = false;
      for (let b = 0; b < NB; b++)
        if (flash[b]! > 0) {
          flash[b] = Math.max(0, flash[b]! - since * 0.4);
          any = true;
        }
      if (any && settle >= 1 && mode !== "sd") paintBlocks();
      since = 0;
    }
    if (anim) step(anim, dt);
    // Collar rings mark holes from afar; up close the casing and the pad do.
    (collars.material as THREE.MeshBasicMaterial).opacity = 0.95 * smooth((s.camera.position.distanceTo(s.orbit.target) - 90) / 160);
    rig.update(dt);
    kit.update(dt, s.night > 0.3);
  });

  /** Advances the drilling: tram, set up, raise the mast, core; the camera follows and the status gives the numbers. */
  const step = (a: Anim, dt: number) => {
    a.t += dt;
    const h = a.hole;
    const c = collarAhead(h.dip);
    const yawDeg = THREE.MathUtils.radToDeg(a.yaw1);
    const x = (v: number) => (v >= 100 ? fmt(Math.round(v / 10) * 10) : fmt(v));
    if (a.phase === "tram") {
      const WAIT = 1.1;
      if (a.t - dt <= 0) {
        s.orbit.set({ target: rig.group.position.clone().add(new THREE.Vector3(0, 2.6, 0)), distance: small ? 64 : 46, elevation: 11, azimuth: THREE.MathUtils.radToDeg(rig.group.rotation.y) + 65 }, 1.3);
        status(`Lowering the mast to tram to ${h.id}`);
      }
      if (a.t < WAIT) {
        const k = a.t / WAIT;
        rig.set({ mast: mix(rig.state.mast, 0, Math.min(1, dt / Math.max(0.05, WAIT - a.t + dt))), slide: 0, jacks: 1 - k, rods: false, spin: 0, head: 1 });
        return;
      }
      const f = clamp((a.t - WAIT) / a.tramS, 0, 1);
      const p = along(a.path, a.len * f);
      rig.group.position.set(p.x, ground(p.x, p.z) - 0.01, p.z);
      rig.group.rotation.y = turn(rig.group.rotation.y, p.yaw, Math.min(1, dt * 5));
      rig.set({ track: a.len * f, jacks: 0 });
      const mins = (a.len * f) / ((OPS.trammKmh * 1000) / 60);
      const rate = (a.len / ((OPS.trammKmh * 1000) / 3600)) / a.tramS;
      status(small ? `Tramming ${fmt(a.len * f)} of ${fmt(a.len)} m · ${OPS.trammKmh} km/h · ${fmt(mins)} min` : `Tramming to ${h.id}: ${fmt(a.len * f)} of ${fmt(a.len)} m at ${OPS.trammKmh} km/h, ${fmt(mins)} min · shown ${x(rate)}× faster`);
      rigTag?.html(`${h.id} · <b>tramming</b>`);
      s.orbit.set({ target: rig.group.position.clone().add(new THREE.Vector3(0, 2.6, 0)), distance: small ? 64 : 46, elevation: 11, azimuth: THREE.MathUtils.radToDeg(rig.group.rotation.y) + 65 }, 0.4);
      if (f >= 1) {
        a.phase = "setup";
        a.t = 0;
        setPad(h);
        s.shadowBox(padG.position, 40);
        s.orbit.set({ target: inPad(0.4, 4.8, c * 0.5 - 0.6), distance: small ? 54 : 38, elevation: 8, azimuth: yawDeg + CLOSE_AZ }, 1.1);
      }
      return;
    }
    if (a.phase === "setup") {
      // Turn on the spot onto the bearing, put the jacks down, raise the mast at its real rate, slide it to the collar.
      const TURN = 0.55;
      const JACK = 0.45;
      const MAST = 1.5;
      const SLIDE = 0.35;
      const t = a.t;
      if (t < TURN) {
        rig.group.rotation.y = turn(a.yaw0Pad ?? rig.group.rotation.y, a.yaw1, smooth(t / TURN));
        const f = padFrame(h);
        rig.group.position.lerp(f.at, Math.min(1, dt * 8));
      } else {
        const f = padFrame(h);
        rig.group.position.copy(f.at);
        rig.group.rotation.y = a.yaw1;
      }
      a.yaw0Pad ??= rig.group.rotation.y;
      const jack = clamp((t - TURN) / JACK, 0, 1);
      const mt = clamp((t - TURN - JACK) / MAST, 0, 1);
      const sl = clamp((t - TURN - JACK - MAST) / SLIDE, 0, 1);
      rig.set({ jacks: jack, mast: h.dip * mt, slide: sl, rods: sl >= 1, head: 1, spin: 0 });
      const secs = h.dip / OPS.mastDegS;
      status(
        mt < 1
          ? small
            ? `Raising the mast to ${h.dip}° at ${OPS.mastDegS}°/s · ${fmt(secs * mt)} of ${fmt(secs)} s`
            : `Levelled on four jacks; raising the mast to ${h.dip}° at ${OPS.mastDegS}°/s: ${fmt(secs * mt)} of ${fmt(secs)} s · shown ${x(secs / MAST)}× faster`
          : `Mast at ${h.dip}° on ${deg3(h.bearing)}; foot clamp over the collar`,
      );
      rigTag?.html(`${h.id} · <b>mast ${fmt(h.dip * mt)}°</b>`);
      if (t >= TURN + JACK + MAST + SLIDE) {
        a.phase = "drill";
        a.t = 0;
        driller.work("radio");
      }
      return;
    }
    if (a.phase === "drill") {
      const u = clamp(a.t / a.coreS, 0, 1);
      const hours = hoursAt(a, u);
      const pl = a.plan;
      const at = depthAt(pl, hours);
      const run = pl.runs[at.run]!;
      a.depth = at.depth;
      // Rig hours per real second now, and so how long the run under way lasts on screen.
      const dhdu = (pl.coreH * a.alpha * Math.exp(a.alpha * u)) / (Math.exp(a.alpha) - 1);
      const rate = (dhdu / a.coreS) * 3600;
      const runReal = (run.hours / dhdu) * a.coreS;
      const cutShare = run.cut / OPS.availability / run.hours;
      if (runReal > 0.35) {
        const cutting = at.inRun < cutShare;
        const head = cutting ? 1 - 0.55 * (at.inRun / cutShare) : 0.45 + 0.55 * ((at.inRun - cutShare) / (1 - cutShare));
        rig.set({ head, spin: cutting ? 9 : 0 });
      } else rig.set({ head: 0.62 + 0.04 * Math.sin(a.t * 17), spin: 9 });
      drawHoles(at.depth);
      kit.rods(RACK_RODS - Math.ceil(at.depth / OPS.rodM));
      kit.trays(Math.floor(at.depth / OPS.trayM));
      const pre = pl.tramH + pl.setupH + pl.mastH;
      rigTag?.html(`${h.id} · <b>${fmt(at.depth)} m</b>`);
      status(
        small
          ? `${fmt(at.depth)} of ${fmt(h.length)} m · ${at.rock.name}, ${fmt(at.rock.rop, 1)} m/h · ${clockOf(pre + hours)}`
          : `${h.id} · ${fmt(at.depth)} of ${fmt(h.length)} m · ${at.rock.name} at ${fmt(at.rock.rop, 1)} m/h · ${clockOf(pre + hours)} · shown ${x(rate)}× faster`,
      );
      ui("metres").textContent = `${fmt(holes.slice(0, -1).reduce((m, k) => m + k.length, 0) + Math.round(at.depth))} m`;
      // The camera stays on the rig for the first run, then flies back to a section square across the hole.
      if (u >= 0.3 && !a.sectioned) {
        a.sectioned = true;
        s.shadowBox([0, 0, 0], 460);
        s.orbit.set(sectionView(h), 1.9);
        // From afar, the tag follows the bit down the hole.
        rigTag?.anchor(bit);
        bit.visible = true;
      }
      if (a.sectioned) {
        bit.position.copy(h.collar).addScaledVector(h.dir, at.depth);
        bit.scale.setScalar(s.orbit.distance * 0.0055 * (1 + 0.18 * Math.sin(a.t * 9)));
      }
      if (u >= 1) {
        a.phase = "done";
        bit.visible = false;
        rig.set({ spin: 0, head: 0.8 });
        driller.work("stand");
        drawHoles();
        anim = null;
      }
    }
  };
  const turnDeg = (a: number, b: number, f: number) => a + (((((b - a + 180) % 360) + 360) % 360) - 180) * f;
  s.onNight((n) => rig.set({ lights: n > 0.3 }));
  if (location.search.includes("stage")) (window as unknown as { __drill?: unknown }).__drill = { tm, rig, holes: () => holes, s, opts, G, fog, HOME, solid, glow, shown, est: () => est, settle: () => settle, at: (x: number, z: number) => s.project(new THREE.Vector3(x, ground(x, z), z)) };

  legend();
  hud();
  try {
    await ready();
  } catch (e) {
    fail(e);
    return;
  }
  await reset().catch(fail);
}
