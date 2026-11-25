/**
 * Follow the ore through the plant: a synthetic gold concentrator at true scale in late-afternoon desert light.
 * Trucks tip at the primary crusher; a covered conveyor stacks the coarse-ore stockpile; a SAG mill and a ball mill
 * turn; cyclones classify; flotation cells spill froth into their launders; the concentrate is thickened under a
 * turning rake and leached in agitated tanks; carbon goes to elution and the gold room, where doré is poured. Every
 * stream carries its reading (instrument, age, evidence class) or is inferred, and "Reconcile" runs a small weighted
 * least-squares balance (plant-balance.ts) that conserves solids and gold at every node and names the reading it
 * trusts least. The balance is written for the page; the plant and its numbers are synthetic.
 */
import * as THREE from "three";
import { makeConveyor, makeCrusher, makeHaulTruck, makePickup } from "./fleet";
import { bake, bar, beam, box, cyl } from "./fleet-kit";
import { EngineError, grind, type GrindResult } from "./engine";
import { $, clamp, fmt, mix, reduced, seg, smooth, toggle } from "./lib";
import { makeMiner, type Miner } from "./miners";
import { balance, CHI2_95, G_PER_OZ, IX, QUANTITIES } from "./plant-balance";
import { GRAVEL, ROAD, SOIL, makeFlows, makeGround, pipe, pipeSupports, slab, surfaceTime, type FlowLine } from "./plant-kit";
import { bakeAsync, compileGradually, finish, makeScrub } from "./plant-look";
import { building, bund, container, fence, lightPole, pipeRack, silo, stockpileDome, storageTank, transformer } from "./plant-site";
import { makeCilTank, makeCyclones, makeElution, makeFlotationCell, makeGoldRoom, makeMill, makeThickener, palette, platform, slurryPump, stairTower, walkway } from "./plant-units";
import { seg as segHtml, toggle as toggleHtml } from "../parts";
import { noise2, stage, type Label } from "./stage";
import { blend } from "./terrain";

type Ev = "measured" | "derived" | "estimated" | "assumed" | "stale" | "refused";
type StageId = "plant" | "crush" | "grind" | "float" | "leach" | "pour" | "tails";

const P = (x: number, y: number, z: number) => new THREE.Vector3(x, y, z);
const nextFrame = () => new Promise<void>((r) => requestAnimationFrame(() => r()));
/** Mount timings, kept only with ?stage in the address, for checking that no step stalls the page. */
const mark = (k: string) => {
  if (typeof location !== "undefined" && location.search.includes("stage")) ((window as unknown as { __plt?: [string, number][] }).__plt ??= []).push([k, Math.round(performance.now())]);
};

/* ---------- Words and numbers ---------- */

const B = balance();
const Q = QUANTITIES;
const raw = (id: string) => Q[IX[id]!]!.value;
const fit = (id: string) => B.fit.x[IX[id]!]!;
const tph = (v: number) => `${fmt(v)} t/h`;
const gpt = (v: number) => `${v < 0.2 ? v.toFixed(3) : v.toFixed(2)} g/t`;
const pct = (v: number, d = 1) => `${(v * 100).toFixed(d)}%`;
const doreKg = (gph: number) => (gph * 24) / 1000;
const WORST = Q[B.worst]!;
const NEXT = Q[B.next]!;
const chip = (ev: Ev, text: string = ev) => `<span class="ev ${ev}">${text}</span>`;
const tag = (id: string) => (id === "g5" ? "TS-301" : id === "g7" ? "TS-501" : Q[IX[id]!]!.label);

interface Stream {
  id: string;
  /** Stream number on the flowsheet. */
  sid: string;
  name: string;
  /** Quantities shown on the label, in order. */
  q: string[];
  /** Short instrument line. */
  inst: string;
  at: THREE.Vector3;
  stage: StageId;
  /** Shown on the whole-plant view. */
  key?: boolean;
}

/* ---------- The figure ---------- */

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  const small = matchMedia("(max-width: 699px)").matches;
  host.innerHTML = `<div class="pl">
<div class="scene-view pl-view">
  <div class="hud-card pl-step"></div>
  <div class="pl-col">
    <div class="hud-card pl-kpi" aria-live="polite"></div>
    <div class="hud-card pl-bal"></div>
    <div class="hud-card agent-note pl-agent"></div>
  </div>
</div>
<div class="scene-bar pl-bar">
  <button type="button" class="btn sm go pl-tour" data-tour aria-pressed="false"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13l10.5-6.5z" fill="currentColor"/></svg><span>Follow the ore</span></button>
  ${segHtml("stage", "Stage of the plant", [["plant", "Plant"], ["crush", "Crush"], ["grind", "Grind"], ["float", "Float"], ["leach", "Leach"], ["pour", "Pour"], ["tails", "Tailings"]], "plant")}
  <div class="toggles">${toggleHtml("reconcile", "Reconcile")}${toggleHtml("sheet", "Flowsheet")}</div>
</div>
<div class="pl-below"></div>
<p class="pl-foot">Synthetic plant; every number is illustrative. The balance is a small weighted least-squares fit written for this page: it moves each reading by as little as its own uncertainty allows until every node conserves solids and gold, names the reading with the largest normalised adjustment, and sets it aside. FrankenMining’s plant reconciliation, in design, fits declared worksheets with their full error covariance and reports adjustments and gross-error diagnostics.</p>
</div>`;
  const view = $(host, ".pl-view");
  const kpi = $(host, ".pl-kpi");
  const bal = $(host, ".pl-bal");
  const step = $(host, ".pl-step");
  const agent = $(host, ".pl-agent");
  const below = $(host, ".pl-below");

  const root0 = $(host, ".pl");
  const col = $(host, ".pl-col");
  const wide = () => view.clientWidth >= 1000;
  /** Layout class from the view's own width (not the window's), so the cards and the 3D framing always agree. */
  const layout = () => {
    const w = view.clientWidth;
    const k = w >= 1000 ? "wide" : w >= 700 ? "mid" : "phone";
    for (const c of ["wide", "mid", "phone"]) root0.classList.toggle(`pl-${c}`, c === k);
    return k;
  };
  layout();
  /** The clear part of the view between the cards, as fractions of its width [left, right]. */
  const clear = (): [number, number] => {
    const w = view.clientWidth || 1;
    if (!wide()) return [0, 1];
    const v = view.getBoundingClientRect();
    return [(step.getBoundingClientRect().right - v.left + 6) / w, (col.getBoundingClientRect().left - v.left - 6) / w];
  };
  const FOG = 0.00062;
  const s = stage(view, {
    clock: 16.75,
    fog: FOG,
    // The figure sets its own near plane each frame (below), from the distance and the height above the ground.
    near: 0.5,
    sunBearing: 345,
    shadowRadius: 420,
    shadowAt: [-160, 0, 30],
    fov: small ? 42 : 30,
    drift: 0,
    maxPixelRatio: small ? 1.5 : 1.6,
    label: "A live 3D gold concentrator",
    // A lens shift puts the middle of the clear part of the view where the camera looks.
    shift: () => {
      layout();
      const [l, r] = clear();
      return [(l + r) / 2 - 0.5, view.clientWidth < 700 ? -0.02 : 0];
    },
    labelArea: () => {
      const [l, r] = clear();
      return wide() ? [l + 0.01, 0.05, r - 0.01, 0.96] : view.clientWidth < 700 ? [0.03, 0.2, 0.97, 0.78] : [0.04, 0.05, 0.72, 0.62];
    },
  });
  s.orbit.minDistance = 6;
  s.orbit.maxDistance = 2400;
  s.orbit.minElevation = 5;
  s.orbit.maxElevation = 72;
  s.renderer.shadowMap.autoUpdate = true;
  // The SAG mill is drawn as a section at the grinding stop (a clipping plane on its own materials).
  s.renderer.localClippingEnabled = true;
  const fog = s.scene.fog instanceof THREE.FogExp2 ? s.scene.fog : null;

  /* ---------- Ground ---------- */
  const { fbm } = noise2(31);
  const PAD = { x0: -600, x1: 262, z0: -80, z1: 132 };
  const roads: THREE.Vector3[][] = [
    [P(-530, 0, 30), P(-640, 0, 40), P(-1100, 0, 120)],
    [P(-620, 0, 118), P(262, 0, 118)],
    [P(-132, 0, -66), P(250, 0, -66)],
    [P(-132, 0, -66), P(-132, 0, 118)],
    [P(250, 0, -66), P(250, 0, 118)],
    [P(-20, 0, 98), P(220, 0, 98)],
    [P(250, 0, 40), P(700, 0, 120), P(1400, 0, 160)],
  ];
  const segDist = (x: number, z: number, a: THREE.Vector3, b: THREE.Vector3) => {
    const dx = b.x - a.x;
    const dz = b.z - a.z;
    const t = clamp(((x - a.x) * dx + (z - a.z) * dz) / (dx * dx + dz * dz), 0, 1);
    return Math.hypot(x - (a.x + dx * t), z - (a.z + dz * t));
  };
  const roadD = (x: number, z: number) => {
    let d = Infinity;
    for (const r of roads) for (let i = 0; i < r.length - 1; i++) d = Math.min(d, segDist(x, z, r[i]!, r[i + 1]!));
    return d;
  };
  const padOut = (x: number, z: number) => Math.hypot(Math.max(PAD.x0 - x, 0, x - PAD.x1), Math.max(PAD.z0 - z, 0, z - PAD.z1));
  const natural = (x: number, z: number) => 3.2 * fbm(x / 380, z / 380, 4) + 0.8 * fbm(x / 70, z / 70, 3) + 1.4;
  const height = (x: number, z: number) => {
    const t = smooth(padOut(x, z) / 45);
    const rd = roadD(x, z);
    const y = mix(0.02 * fbm(x / 9, z / 9, 2), natural(x, z), t);
    return rd < 18 && t > 0 ? mix(y, Math.min(y, 0.6 + 0.5 * t), 1 - smooth((rd - 9) / 9)) : y;
  };
  const tmpC = new THREE.Color();
  const WET = [
    [-50, 46, 34, 26],
    [32, 54, 42, 20],
    [140, 34, 46, 14],
    [30, -12, 30, 30],
  ] as const;
  mark("start");
  const ground = makeGround({
    centre: [-170, 30],
    extent: 1500,
    cells: small ? 200 : 260,
    seed: 31,
    height,
    colour(x, z, _y, ny, c) {
      const v = clamp(0.5 + 0.9 * fbm(x / 60, z / 60, 3), 0, 0.999);
      blend(SOIL, v, c);
      // Red-brown laterite in broad patches, paler calcrete in others.
      const iron = fbm(x / 240 + 3, z / 240 - 1, 3);
      c.lerp(tmpC.set("#9b6b4a"), clamp(iron * 1.4, 0, 0.45));
      c.lerp(tmpC.set("#c2b293"), clamp(-iron * 1.2, 0, 0.35));
      const pad = 1 - smooth(padOut(x, z) / 25);
      if (pad > 0) c.lerp(blend(GRAVEL, clamp(0.5 + fbm(x / 25, z / 25, 2), 0, 0.999), tmpC), pad * 0.85);
      const rd = roadD(x, z);
      if (rd < 14) c.lerp(ROAD, (1 - smooth((rd - 6) / 7)) * 0.8);
      // Tyre-darkened wheel paths on the roads.
      if (rd < 6) c.multiplyScalar(1 - 0.06 * Math.exp(-(((rd - 2.6) / 0.9) ** 2)));
      // Spillage and wet ground round the wet plant.
      for (const [wx, wz, rx, rz] of WET) {
        const wet = Math.exp(-(((x - wx) / rx) ** 2) - (((z - wz) / rz) ** 2));
        if (wet > 0.01) c.lerp(tmpC.set("#6d6455"), wet * 0.5 * clamp(0.45 + fbm(x / 9, z / 9, 3) * 1.8, 0, 1));
      }
      if (ny < 0.9) c.multiplyScalar(0.92);
    },
  });
  s.scene.add(ground.group);
  mark("ground");
  await nextFrame();
  const H = ground.heightAt;

  const M = palette();
  const root = new THREE.Group(); // every fixed part of the plant, baked at the end
  const moving: { update(dt: number): void }[] = [];
  // Belts run at 4.5 m/s; the ore on each metre of belt follows from the tonnage: kg/m = (t/h) / 3.6 / v.
  const BELT_V = 4.5;
  const BELT_W = 1.4;
  const kgPerM = (tph: number) => tph / 3.6 / BELT_V;
  // Visible fill of the troughed section: the ore's cross-section (bulk density 1.7 t/m³) over a full 1.4 m trough.
  const beltFill = (tph: number) => clamp(kgPerM(tph) / 1700 / 0.19, 0.15, 1);
  const DOME = { r: 46, h: 31 };

  /* ---------- Crushing and the covered stockpile ---------- */
  const crusher = makeCrusher({ platform: 7, seed: 4 });
  crusher.group.position.set(-420, 0, 30);
  crusher.group.rotation.y = Math.PI / 2;
  s.scene.add(crusher.group);
  crusher.group.updateMatrixWorld(true);
  const local = (x: number, y: number, z: number) => P(x, y, z).applyMatrix4(crusher.group.matrixWorld);
  const crHead = new THREE.Vector3();
  crusher.conveyor?.anchors.head.getWorldPosition(crHead);
  crusher.set({ running: true, load: 0.85 });
  moving.push(crusher);
  mark("c1");
  await nextFrame();

  const pile = P(-205, 0, 30);
  const dome = stockpileDome(root, M, { x: pile.x, z: pile.z, r: DOME.r, h: DOME.h, doorAt: Math.PI * 0.62 });
  const cv1Pts = [crHead.clone().add(P(0, -0.5, 0)), P(-262, 24.5, 30), P(pile.x - 1, DOME.h + 4.2, 30)];
  const cv1 = makeConveyor({ points: cv1Pts }, { width: BELT_W, covered: true, withGantry: true, load: beltFill(raw("t1")), speed: BELT_V, ore: "ore", seed: 3 });
  s.scene.add(cv1.group);
  // Reclaim tunnel under the pile: its portal and wing walls east of the dome, an escape shaft, and the mill feed
  // conveyor rising out of it.
  const RT = -150;
  box(root, M.concrete, 10, 6.4, 8.6, RT - 4, 2.6, 30, { r: 0.08 });
  box(root, M.dark, 0.3, 4.2, 5.2, RT + 1.02, 2.2, 30, { r: 0 });
  for (const sz of [-1, 1]) beam(root, M.concrete, P(RT + 0.6, 0.3, 30 + sz * 4.6), P(RT + 6, 0.3, 30 + sz * 6.6), 0.5, 5.2);
  box(root, M.concrete, 11, 0.7, 9.6, RT - 4.2, 6.1, 30, { r: 0.04 });
  cyl(root, M.concrete, 1.5, 4, RT - 4, 2, 30 + 10, { seg: 20 });
  box(root, M.galv, 2.2, 2.6, 2.2, RT - 4, 5.3, 30 + 10, { r: 0.05 });

  await nextFrame();
  /* ---------- Grinding and classification ---------- */
  // SAG mill 11.6 m by 6.1 m and two ball mills 7.3 m by 11 m, each at a fraction of its critical speed.
  const sag = makeMill(root, s.scene, M, { x: -64, y: 0, z: 30, D: 11.6, L: 6.1, head: 2.4, trunnion: 1.6, phi: 0.75, drive: 1, sag: true, charge: 0.3, small });
  const cv2End = sag.feed.clone().add(P(-1.0, 2.4, 0));
  const cv2 = makeConveyor({ points: [P(RT + 1, 1.2, 30), cv2End] }, { width: BELT_W, covered: false, withGantry: false, walkway: "both", load: beltFill(raw("t2")), speed: BELT_V, ore: "ore", seed: 5 });
  s.scene.add(cv2.group);
  moving.push(cv1, cv2);
  mark("c2");
  await nextFrame();
  // One mill a frame: each builds its shell, liners, drive and charge.
  const balls: ReturnType<typeof makeMill>[] = [];
  for (const z of [64, 94]) {
    balls.push(makeMill(root, s.scene, M, { x: -55, y: 0, z, D: 7.3, L: 11, head: 1.3, trunnion: 1.1, phi: 0.74, drive: 1 }));
    await nextFrame();
  }
  moving.push(sag, ...balls);
  await nextFrame();
  // A cyclone cluster for each ball mill, each fed from its own pump box with a duty and a standby pump.
  const cycs = [47, 78].map((z) => makeCyclones(root, M, { x: -30, y: 0, z, deck: 18 }));
  await nextFrame();
  const PB = [P(-43, 0, 46), P(-41, 0, 80)];
  const pumps = PB.map((pb) => {
    box(root, M.steel, 6, 4.4, 5, pb.x, 2.2, pb.z, { r: 0.08 });
    box(root, M.steelDark, 6.3, 0.25, 5.3, pb.x, 4.53, pb.z, { r: 0.02 });
    platform(root, M, pb.x, 4.66, pb.z + 3.4, 6, 1.6, "sew", false);
    return [slurryPump(root, M, pb.x + 5.2, 0.12, pb.z - 1.4, 0, 1.25), slurryPump(root, M, pb.x + 5.2, 0.12, pb.z + 1.5, 0, 1.25)];
  });
  slab(root, M.concreteDark, 30, 52, -37, 0.32, 63, 0.4);
  await nextFrame();
  // An electrical room on piers beside the SAG mill.
  for (const sx of [-1, 1]) for (const sz of [-1, 1]) box(root, M.concrete, 0.6, 2.2, 0.6, -98 + sx * 6, 1.1, 44 + sz * 2.5, { r: 0.02 });
  building(root, M, { x: -98, y: 2.2, z: 44, w: 14, d: 6.4, h: 3.6, wall: "#dcd9ce", roof: "#b0b5b2", trim: "#4e5a55", roofType: "mono", pitch: 0.08, ac: [{ side: "e", at: 0 }], doors: [{ side: "e", at: 1.6, w: 1.0, h: 2.2, kind: "door" }], louvres: [{ side: "s", at: -3, y: 1.6, w: 1.4, h: 0.9 }] });
  platform(root, M, -90, 2.2, 45.6, 2.2, 2.4, "nse", false);
  stairTower(root, M, { x0: -85.2, x1: -88.9, zA: 45.6, zB: 45.6, y0: 0, y1: 2.2, flights: 1 });
  mark("c3");
  await nextFrame();

  /* ---------- Flotation ---------- */
  const rough: ReturnType<typeof makeFlotationCell>[] = [];
  // Six 320 m³ rougher tank cells stepping down the row, three 130 m³ cleaners.
  for (let i = 0; i < 6; i++) rough.push(makeFlotationCell(root, s.scene, M, { x: 6 + i * 9.6, y: 0, z: 60, r: 3.75, h: 7.3 - i * 0.4, launder: -1 }));
  await nextFrame();
  const clean: ReturnType<typeof makeFlotationCell>[] = [];
  for (let i = 0; i < 3; i++) clean.push(makeFlotationCell(root, s.scene, M, { x: 18 + i * 7.6, y: 0, z: 40, r: 2.75, h: 5.6 - i * 0.3, launder: 1 }));
  // Walkways joining the rougher decks, and stairs down at the feed end.
  for (let i = 0; i < 5; i++) {
    const a = rough[i]!.deck;
    const b = rough[i + 1]!.deck;
    // 1.04 m wide, so the stringers miss the faces of the cell-top beams they meet.
    walkway(root, M, a.clone().add(P(1.3, 0, 0)), b.clone().add(P(-1.3, 0, 0)).setY(a.y), 1.04);
    // The riser between two deck levels stops just under the upper grating.
    if (a.y - b.y > 0.05) box(root, M.grate, 0.5, a.y - b.y - 0.07, 1.0, b.x - 1.55, b.y + (a.y - b.y - 0.07) / 2, b.z, { r: 0 });
  }
  stairTower(root, M, { x0: -5.6, x1: -0.8, zA: 60, zB: 61.2, y0: 0, y1: rough[0]!.deck.y, flights: 3 });
  walkway(root, M, P(-0.8, rough[0]!.deck.y, 60), P(rough[0]!.deck.x - 1.3, rough[0]!.deck.y, 60), 1.04);
  // An air header along the rougher row, a drop to each mechanism.
  pipe(root, M.airPipe, [P(-6, 1.2, 66.6), P(-2, 1.2, 66.6), P(-2, 9.2, 66.6), P(60, 9.2, 66.6)], 0.28, { flange: M.steelLight });
  for (const c of rough) bar(root, M.airPipe, P(c.lip.x - 0.6, 9.2, 66.6), P(c.lip.x - 0.6, c.top + 2.5, c.lip.z + 0.6), 0.09, 6);
  // Concentrate launders along the rows, falling to the concentrate pump box.
  beam(root, M.launder, P(2, 5.6, 54.2), P(58, 4.0, 54.2), 1.0, 0.75);
  beam(root, M.launder, P(14, 4.1, 45.1), P(42, 3.4, 45.1), 0.8, 0.6);
  for (let x = 5; x < 58; x += 6) box(root, M.steel, 0.2, 5.2 - (x - 2) * 0.029, 0.2, x, (5.2 - (x - 2) * 0.029) / 2, 54.2, { r: 0 });
  box(root, M.steel, 4, 3.2, 3.6, 46, 1.6, 47, { r: 0.06 });
  slurryPump(root, M, 50.5, 0.12, 47, 0, 0.9);
  await nextFrame();
  // Trash screen on a stand ahead of the first cell.
  platform(root, M, -10, 9.6, 58, 6, 5, "nse");
  box(root, M.steelLight, 3.6, 1.6, 2.4, -10, 10.6, 58, { r: 0.06, rz: -0.18 });
  box(root, M.motor, 0.8, 0.6, 0.6, -11.6, 11.8, 59.4, { r: 0.04 });
  // Tailings sampler TS-301: a sampler house on the tailings line after the last cell.
  const ts301 = P(66, 0, 66);
  box(root, M.steelLight, 4.2, 5.6, 3.4, ts301.x, 2.8, ts301.z, { r: 0.06 });
  box(root, M.guard, 4.6, 0.3, 3.8, ts301.x, 5.75, ts301.z, { r: 0.03 });
  box(root, M.motor, 1.2, 0.8, 0.8, ts301.x + 1.2, 6.3, ts301.z, { r: 0.04 });
  box(root, M.dark, 0.06, 1.6, 1.0, ts301.x - 2.13, 2.2, ts301.z + 0.6, { r: 0 });
  slab(root, M.concreteDark, 74, 40, 28, 0.26, 54, 0.4);
  slab(root, M.concreteDark, 20, 52, -30, 0.2, 64, 0.4);
  // Reagent tanks in a bund, a reagent shed and a blower house beside flotation.
  for (let i = 0; i < 4; i++) {
    cyl(root, M.frp, 1.6, 4.2, 10 + i * 4.4, 2.4, 86, { seg: 24 });
    cyl(root, M.frp, 1.65, 0.1, 10 + i * 4.4, 4.5, 86, { seg: 24 });
    cyl(root, M.motor, 0.25, 0.7, 10 + i * 4.4, 4.9, 86, { seg: 10 });
  }
  bund(root, M, 16.6, 86, 18, 6, 1.0);
  slab(root, M.concrete, 18, 6, 16.6, 0.24, 86, 0.4);
  building(root, M, { x: 40, z: 88, w: 16, d: 10, h: 6.5, wall: "#c9bfa4", roof: "#a4a9a6", trim: "#5a5442", doors: [{ side: "s", at: -3, w: 3.6, h: 4, kind: "roller" }, { side: "s", at: 4, w: 1, h: 2.2, kind: "door" }], louvres: [{ side: "w", at: 0, y: 4.2, w: 2, h: 1.2 }] });
  building(root, M, { x: -8, z: 88, w: 16, d: 10, h: 7, wall: "#7c846a", roof: "#a4a9a6", trim: "#3f463b", doors: [{ side: "s", at: 3, w: 4, h: 4.5, kind: "roller" }], louvres: [{ side: "e", at: -2, y: 4.4, w: 2.4, h: 1.4 }, { side: "e", at: 2, y: 4.4, w: 2.4, h: 1.4 }] });
  mark("c4");
  await nextFrame();

  /* ---------- Thickeners, leach and gold room ---------- */
  // Concentrate thickener 22 m across with a traction rake; tailings thickener 40 m across with a centre drive.
  const CT = P(80, 0, 36);
  const TT = P(30, 0, -14);
  const conc = makeThickener(root, s.scene, M, { x: CT.x, y: 0, z: CT.z, r: 11, wall: 4.2, traction: true, speed: 0.5 });
  const tailsT = makeThickener(root, s.scene, M, { x: TT.x, y: 0, z: TT.z, r: 20, wall: 4.8, speed: 0.3, bridge: 0, outlet: Math.PI });
  moving.push(conc, tailsT);
  const CIL0 = 114;
  const CILD = 12.4;
  const cil: ReturnType<typeof makeCilTank>[] = [];
  for (let i = 0; i < 6; i++) cil.push(makeCilTank(root, s.scene, M, { x: CIL0 + i * CILD, y: 0.6, z: 34, r: 5.2, h: 11.5, spin: 3.6 + (i % 3) * 0.4 }));
  moving.push(...cil);
  // The bund the leach tanks stand in, the walkways joining their bridges, and a stair tower.
  slab(root, M.concrete, 6 * CILD + 8, 20, CIL0 + 2.5 * CILD, 0.6, 34, 0.9);
  bund(root, M, CIL0 + 2.5 * CILD, 34, 6 * CILD + 8, 20, 1.4, 0.35);
  for (let i = 0; i < 5; i++) {
    const a = cil[i]!.deck;
    const b = cil[i + 1]!.deck;
    walkway(root, M, a.clone().add(P(1.4, 0, 0)), b.clone().add(P(-1.4, 0, 0)), 1.2);
  }
  stairTower(root, M, { x0: CIL0 - 16.2, x1: CIL0 - 7.2, zA: 34, zB: 35.2, y0: 0, y1: cil[0]!.deck.y, flights: 3 });
  walkway(root, M, P(CIL0 - 7.2, cil[0]!.deck.y, 34), P(CIL0 - 1.4, cil[0]!.deck.y, 34), 1.2);
  // Carbon safety screen and the cyanide destruction tank for the leach tailings.
  const cilEnd = CIL0 + 5 * CILD;
  platform(root, M, cilEnd + 12, 8, 34, 5, 4, "nse");
  box(root, M.steelLight, 3.6, 1.4, 2.4, cilEnd + 12, 9, 34, { r: 0.06, rz: 0.15 });
  cyl(root, M.tankDark, 3.6, 7, cilEnd + 20, 3.5, 34, { seg: 40 });
  // Top ring stands proud of the shell top, so the two never share a plane.
  cyl(root, M.tankDark, 3.66, 0.24, cilEnd + 20, 6.94, 34, { seg: 40 });
  box(root, M.gearbox, 1.2, 0.9, 1.2, cilEnd + 20, 7.6, 34, { r: 0.06 });
  cyl(root, M.motor, 0.42, 1.2, cilEnd + 20, 8.6, 34, { seg: 16 });
  const elu = makeElution(root, M, { x: 128, y: 0, z: 2 });
  const gold = makeGoldRoom(root, s.scene, M, { x: 164, y: 0, z: -10, w: 20, d: 13, h: 7 });
  moving.push(gold);
  fence(root, M, [P(150, 0, -22), P(180, 0, -22), P(180, 0, 2), P(150, 0, 2), P(150, 0, -22)]);
  silo(root, M, { x: 100, z: 4, r: 2.6, h: 8, legs: 6, mat: M.white });
  // Tailings pump station and sampler TS-501 at the thickener underflow.
  const ts501 = P(30, 0, -44);
  building(root, M, { x: ts501.x, z: ts501.z, w: 10, d: 7, h: 5, wall: "#b9bcb6", roof: "#a4a9a6", trim: "#4e5a55", roofType: "mono", pitch: 0.08, doors: [{ side: "e", at: 0, w: 3, h: 3.4, kind: "roller" }], louvres: [{ side: "s", at: 2.5, y: 3.4, w: 1.6, h: 1 }] });
  box(root, M.steelLight, 3.2, 4.0, 2.8, ts501.x + 7.5, 2.0, ts501.z, { r: 0.06 });
  box(root, M.guard, 3.6, 0.3, 3.2, ts501.x + 7.5, 4.15, ts501.z, { r: 0.03 });
  // Water: the process water tank, a raw and fire water tank, and the thickener overflow to the tank.
  storageTank(root, M, { x: -24, z: -14, r: 11, h: 8, mat: finish("#8ea494", { rough: 0.5, coat: 0.3, rust: 0.2, streak: 0.5, streakColour: "#5e5a4c" }) });
  storageTank(root, M, { x: -54, z: -10, r: 7, h: 9, mat: M.white, band: M.red });
  for (let i = 0; i < 3; i++) slurryPump(root, M, -40 + i * 3.2, 0.24, 2, Math.PI / 2, 0.8);
  slab(root, M.concrete, 12, 5, -37, 0.24, 2, 0.4);
  mark("c5");
  await nextFrame();

  /* ---------- Buildings, power and the laydown ---------- */
  building(root, M, { x: -104, z: 86, w: 24, d: 12, h: 7.5, wall: "#dcd9ce", roof: "#b0b5b2", trim: "#4e5a55", dado: { colour: "#56665d", h: 1.2 }, windows: [{ side: "s", at: -2, y: 4.2, w: 2.2, h: 1.3, n: 5, gap: 0.6 }], doors: [{ side: "s", at: 8.5, w: 1.0, h: 2.2, kind: "door" }], ac: [{ side: "e", at: -2 }, { side: "e", at: 2 }], vents: 2 });
  building(root, M, { x: 200, z: 82, w: 36, d: 20, h: 12, wall: "#b9bcb6", roof: "#a4a9a6", trim: "#3f463b", dado: { colour: "#3f4a52", h: 2.4 }, doors: [{ side: "s", at: -9, w: 6, h: 7, kind: "roller" }, { side: "s", at: 2, w: 6, h: 7, kind: "roller" }, { side: "s", at: 12, w: 1, h: 2.2, kind: "door" }], windows: [{ side: "w", at: 0, y: 9.5, w: 2, h: 1, n: 4, gap: 2 }], vents: 4 });
  building(root, M, { x: 150, z: 80, w: 28, d: 16, h: 9, wall: "#c9bfa4", roof: "#a4a9a6", trim: "#5a5442", doors: [{ side: "s", at: -6, w: 5, h: 5.5, kind: "roller" }, { side: "s", at: 6, w: 5, h: 5.5, kind: "roller" }], vents: 3 });
  building(root, M, { x: -140, z: 84, w: 22, d: 11, h: 5, wall: "#dcd9ce", roof: "#b0b5b2", trim: "#4e5a55", roofType: "mono", pitch: 0.06, windows: [{ side: "s", at: 0, y: 2.6, w: 1.6, h: 1.1, n: 6, gap: 1.2 }], doors: [{ side: "e", at: 0, w: 1, h: 2.2, kind: "door" }], ac: [{ side: "n", at: -4 }, { side: "n", at: 4 }] });
  building(root, M, { x: -170, z: 110, w: 34, d: 11, h: 4.2, wall: "#dcd9ce", roof: "#b0b5b2", trim: "#5a5442", roofType: "mono", pitch: 0.05, windows: [{ side: "s", at: 0, y: 2.3, w: 1.4, h: 1.0, n: 10, gap: 1.6 }], doors: [{ side: "s", at: -15, w: 1, h: 2.2, kind: "door" }], ac: [{ side: "n", at: -10 }, { side: "n", at: 0 }, { side: "n", at: 10 }] });
  // Substation: transformers behind a fence, a switch room.
  for (let i = 0; i < 3; i++) transformer(root, M, -160 + i * 9, -30, 0);
  building(root, M, { x: -151, z: -44, w: 22, d: 7, h: 4.4, wall: "#dcd9ce", roof: "#a4a9a6", trim: "#4e5a55", roofType: "mono", pitch: 0.06, doors: [{ side: "s", at: -8, w: 1.2, h: 2.2, kind: "door" }], louvres: [{ side: "s", at: 4, y: 2.6, w: 2.4, h: 1.2 }], ac: [{ side: "s", at: 0 }] });
  fence(root, M, [P(-170, 0, -52), P(-130, 0, -52), P(-130, 0, -20), P(-170, 0, -20), P(-170, 0, -52)]);
  // Laydown yard: containers and pipe spools east of the workshop road.
  const lay: { x: number; z: number; ry: number; c: number; y?: number; long?: boolean }[] = [
    { x: 228, z: 30, ry: 0.04, c: 0 },
    { x: 228, z: 33, ry: 0.04, c: 2 },
    { x: 228, z: 30, ry: 0.04, c: 4, y: 2.59 },
    { x: 236, z: 44, ry: 1.6, c: 1 },
    { x: 240, z: 44, ry: 1.6, c: 5 },
    { x: 214, z: 52, ry: 0, c: 3, long: true },
    { x: 214, z: 55, ry: 0, c: 6, long: true },
    { x: 232, z: 62, ry: 0.3, c: 0 },
  ];
  for (const c of lay) container(root, M, c.x, c.y ?? 0, c.z, c.ry, c.c, !!c.long);
  for (let i = 0; i < 6; i++) {
    const py = 0.45 + Math.floor(i / 3) * 0.9;
    cyl(root, M.pipe, 0.45, 11.5, 222 + (i % 3) * 0.95 + (i >= 3 ? 0.47 : 0), py, 76, { axis: "z", seg: 14 });
  }
  for (let i = 0; i < 5; i++) cyl(root, M.waterPipe, 0.3, 6, 232 + i * 0.62, 0.3, 78, { axis: "z", seg: 12 });
  // Light poles round the plant.
  for (const [x, z, a] of [
    [-80, 24, 0.6],
    [-20, 72, 2.4],
    [70, 76, 2.8],
    [104, 58, -2.6],
    [190, 18, -2.2],
    [-130, 60, 1.6],
    [10, 27, -0.5],
    [140, -26, -0.6],
  ] as const)
    lightPole(root, M, x, z, 18, a);
  mark("c6");
  await nextFrame();

  /* ---------- Pipes ---------- */
  // The main pipe rack from the grinding area east to the leach, carrying water, air and reagent lines.
  pipeRack(root, M, [P(-40, 0, 20), P(98, 0, 20), P(98, 0, 52)], {
    h: 7.2,
    w: 3.2,
    span: 7,
    tiers: 2,
    pipes: [
      { dx: -1.1, r: 0.32, mat: M.waterPipe },
      { dx: -0.3, r: 0.22, mat: M.waterPipe },
      { dx: 0.5, r: 0.26, mat: M.airPipe },
      { dx: 1.2, r: 0.1, mat: M.reagentPipe },
      { dx: -0.8, r: 0.18, mat: M.steelLight, tier: 1 },
      { dx: 0.4, r: 0.14, mat: M.reagentPipe, tier: 1 },
      { dx: 1.1, r: 0.12, mat: M.red, tier: 1 },
    ],
  });
  const pb = PB.map((p) => p.clone());
  const R = {
    sagOut: [sag.discharge.clone().setY(1.1), P(-48, 1.1, 30), P(-48, 1.1, 43.8), P(pb[0]!.x - 3, 1.1, 43.8)],
    bm1Out: [balls[0]!.discharge.clone().setY(1.1), P(balls[0]!.discharge.x, 1.1, 49.8), P(-44, 1.1, 48.7)],
    bm2Out: [balls[1]!.discharge.clone().setY(1.1), P(balls[1]!.discharge.x, 1.1, 84.2), P(-42, 1.1, 82.7)],
    cycFeed1: [pumps[0]![0]!.discharge, P(pb[0]!.x + 4.95, 4.2, pb[0]!.z - 1.08), P(-35.6, 4.2, pb[0]!.z - 1.08), P(-35.6, 19.6, pb[0]!.z - 1.08), P(-31.3, 19.6, 47)],
    cycFeed2: [pumps[1]![0]!.discharge, P(pb[1]!.x + 4.95, 4.2, pb[1]!.z - 1.08), P(-35.6, 4.2, pb[1]!.z - 1.08), P(-35.6, 19.6, pb[1]!.z - 1.08), P(-31.3, 19.6, 78)],
    under1: [cycs[0]!.underflow, P(-36, 17.8, 47), P(-50, 14.5, 58), P(-64, 11.2, 64), P(-66.1, 10.8, 64)],
    under2c: [cycs[1]!.underflow, P(-36, 17.8, 78), P(-50, 14.5, 88), P(-64, 11.2, 94), P(-66.1, 10.8, 94)],
    over1: [cycs[0]!.overflow, P(-23.2, 22.1, 47), P(-21, 13.5, 52), P(-12.5, 11.6, 57)],
    over2: [cycs[1]!.overflow, P(-23.2, 22.1, 78), P(-20, 14, 70), P(-12.5, 11.8, 59)],
    screen: [P(-8, 9.6, 58), P(-3, 8.6, 60), P(1.8, 7.6, 60)],
    roughTails: [P(58.5, 1.2, 60), P(62, 1.2, 64), P(ts301.x - 2.1, 1.2, ts301.z)],
    toThick: [P(ts301.x - 2.1, 1.0, ts301.z + 1), P(61, 1.0, 67), P(61, 1.0, TT.z), P(TT.x + 24, 1.0, TT.z), P(TT.x + 22, 6.0, TT.z), P(TT.x + 3, 6.0, TT.z)],
    concOut: [P(48, 3.0, 47), P(60, 3.0, 47), P(70, 5.4, 38), P(CT.x, 5.8, CT.z)],
    concUnder: [P(CT.x, 0.6, CT.z), P(CT.x + 18, 0.6, 28), P(106, 0.6, 28), P(106, 12.6, 28), P(110, 12.6, 30)],
    cilTails: [P(cilEnd + 6.3, 11.6, 31.9), P(cilEnd + 10, 9.6, 33), P(cilEnd + 16.4, 6.8, 34)],
    detox: [P(cilEnd + 20, 0.6, 30.2), P(cilEnd + 26, 0.6, 24), P(cilEnd + 26, 0.6, -40), P(44, 0.6, -52), P(36, 0.6, -62), P(36, 0.6, -1400)],
    under2: [P(TT.x, 0.8, TT.z), P(TT.x, 0.8, -40)],
    toDam: [P(31, 0.8, -48), P(31, 0.8, -60), P(31, 0.8, -1400)],
    carbon: [P(CIL0, 12.8, 29), P(CIL0, 12.8, 10), P(126, 9.6, 4)],
    eluate: [P(130, 1.0, 6.2), P(146, 1.0, 6.2), P(152.4, 1.0, -6.5), P(154.4, 1.0, -6.5)],
    water: [P(-24 + 11.6, 0.8, -14), P(0, 0.8, -14), P(0, 0.8, 14), P(-30, 0.8, 14), P(-30, 0.8, 38)],
    overflow: [P(TT.x - 21.6, 3.2, TT.z), P(-10, 1.0, TT.z), P(-12.4, 1.0, -14)],
  };
  const pipeRuns: [THREE.Vector3[], number, THREE.Material][] = [
    [R.sagOut, 0.36, M.pipe],
    [R.bm1Out, 0.32, M.pipe],
    [R.bm2Out, 0.32, M.pipe],
    [R.cycFeed1, 0.36, M.pipe],
    [R.cycFeed2, 0.36, M.pipe],
    [R.under1, 0.3, M.pipe],
    [R.under2c, 0.3, M.pipe],
    [R.over1, 0.4, M.pipe],
    [R.over2, 0.4, M.pipe],
    [R.screen, 0.36, M.pipe],
    [R.roughTails, 0.34, M.pipe],
    [R.toThick, 0.36, M.pipe],
    [R.concOut, 0.2, M.pipe],
    [R.concUnder, 0.18, M.pipe],
    [R.cilTails, 0.18, M.pipe],
    [R.detox, 0.16, M.pipe],
    [R.under2, 0.32, M.pipe],
    [R.toDam, 0.36, M.pipe],
    [R.carbon, 0.13, M.steelLight],
    [R.eluate, 0.1, M.stainless],
    [R.water, 0.3, M.waterPipe],
    [R.overflow, 0.4, M.waterPipe],
  ];
  for (const [pts, r, m] of pipeRuns) {
    const p = pipe(root, m, pts, r, { flange: M.steelLight });
    pipeSupports(root, M.steel, p.path, H, 6, r);
  }
  // Sleepers under the long tailings lines out to the dam.
  for (let z = -70; z > -700; z -= 9) for (const x of [31, 36]) box(root, M.concrete, 1.6, 0.25, 0.4, x, H(x, z) + 0.1, z, { r: 0.02 });
  mark("c7");
  await nextFrame();

  if (location.search.includes("zcheck")) {
    const { coplanar } = await import("./plant-zcheck");
    const found = coplanar(root, H);
    (window as unknown as { __zcheck: string[] }).__zcheck = found;
  }

  /* ---------- Bake the fixed plant ---------- */
  const baked = await bakeAsync(root, { mudTop: 1.2, mudBottom: 0.1, mud: 0.55, dust: 0.9 });
  baked.traverse((m) => {
    m.castShadow = true;
    m.receiveShadow = true;
  });
  s.scene.add(baked);
  mark("c8");
  await nextFrame();

  /* ---------- Scrub, and the dam and waste dump on the horizon ---------- */
  const scrub = makeScrub({
    n: small ? 2600 : 6000,
    centre: [-170, 30],
    radius: 1400,
    seed: 12,
    height: H,
    keep: (x, z) => {
      const pd = padOut(x, z);
      if (pd < 14) return 0;
      if (roadD(x, z) < 11) return 0;
      if (Math.abs(x - 33) < 8 && z < -60) return 0;
      if (z < -1290 || Math.hypot((x + 1180) / 1.6, z + 420) < 270) return 0;
      return clamp((pd - 14) / 60, 0, 1) * clamp(0.45 + fbm(x / 120, z / 120, 3) * 1.6, 0.1, 1);
    },
  });
  s.scene.add(scrub);
  {
    // On the horizon: a waste dump in three benched lifts to the west and the tailings dam's embankment to the north,
    // built as heightfields on the ground's own material, so they take its light, haze and texture.
    const dumpH = (x: number, z: number) => {
      const dx = (x + 1180) / 1.6;
      const dz = z + 420;
      const a = Math.atan2(dz, dx);
      const r = Math.hypot(dx, dz) / (1 + 0.1 * Math.sin(a * 3 + 1) + 0.05 * Math.sin(a * 7 + 2));
      let h = 0;
      for (const [R, lh] of [
        [250, 22],
        [208, 22],
        [168, 20],
      ] as const)
        h += lh * clamp((R - r) / (lh * 1.5), 0, 1);
      return h;
    };
    const damH = (x: number, z: number) => {
      const ends = smooth((x + 1150) / 160) * smooth((1150 - x) / 160);
      return 26 * clamp((-1300 - z) / 58, 0, 1) * ends;
    };
    const tmp = new THREE.Color();
    const land = (x0: number, x1: number, z0: number, z1: number, nx: number, nz: number, f: (x: number, z: number) => number, paint: (x: number, z: number, h: number, ny: number, c: THREE.Color) => void) => {
      const g = new THREE.PlaneGeometry(x1 - x0, z1 - z0, nx, nz).rotateX(-Math.PI / 2).translate((x0 + x1) / 2, 0, (z0 + z1) / 2);
      const pos = g.getAttribute("position") as THREE.BufferAttribute;
      const hs = new Float32Array(pos.count);
      for (let i = 0; i < pos.count; i++) {
        const x = pos.getX(i);
        const z = pos.getZ(i);
        const h = f(x, z);
        hs[i] = h;
        // The skirt dives under the ground steeply, so the toe is a clean crossing and never a sliver lying in the
        // ground's own plane (where the two surfaces would fight for depth).
        pos.setY(i, H(x, z) + h + 0.6 * fbm(x / 40, z / 40, 2) * Math.min(1, h / 4) - 2.5 * Math.max(0, 1 - h / 2));
      }
      g.computeVertexNormals();
      const nrm = g.getAttribute("normal") as THREE.BufferAttribute;
      const uv = g.getAttribute("uv") as THREE.BufferAttribute;
      const col = new Float32Array(pos.count * 3);
      for (let i = 0; i < pos.count; i++) {
        uv.setXY(i, pos.getX(i) * 0.12, pos.getZ(i) * 0.12);
        paint(pos.getX(i), pos.getZ(i), hs[i]!, nrm.getY(i), tmp);
        tmp.toArray(col, i * 3);
      }
      g.setAttribute("color", new THREE.BufferAttribute(col, 3));
      const m = new THREE.Mesh(g, ground.material);
      m.receiveShadow = true;
      s.scene.add(m);
    };
    land(-1700, -660, -820, -20, 150, 110, dumpH, (x, z, h, ny, c) => {
      blend(SOIL, clamp(0.5 + 0.9 * fbm(x / 400, z / 400, 3), 0, 0.999), c);
      if (h > 0.05) {
        // Waste rock: greyer and darker than the plain, with paler crests and a fresher, redder top lift.
        c.lerp(tmp.set(h > 60 ? "#8c6f58" : "#857a6c"), 0.55 + 0.2 * fbm(x / 30, z / 30, 2));
        if (ny > 0.97) c.lerp(tmp.set("#a29483"), 0.4);
      }
    });
    land(-1500, 1500, -1500, -1270, 260, 46, damH, (x, z, h, ny, c) => {
      blend(SOIL, clamp(0.5 + 0.9 * fbm(x / 400, z / 400, 3), 0, 0.999), c);
      if (h > 0.05) c.lerp(tmp.set(ny > 0.97 ? "#c9c0ab" : "#8d8069"), ny > 0.97 ? 0.75 : 0.6);
    });
  }

  mark("land");
  await nextFrame();

  /* ---------- A haul truck tipping at the crusher ---------- */
  const truck = small ? null : makeHaulTruck({ payload: true, ore: "ore", seed: 21 });
  const anchorT = new THREE.Vector3();
  crusher.anchors.truck.getWorldPosition(anchorT);
  const aLocal = crusher.group.worldToLocal(anchorT.clone());
  const bez = (a: THREE.Vector3, c: THREE.Vector3, b: THREE.Vector3) => new THREE.QuadraticBezierCurve3(a, c, b).getSpacedPoints(Math.max(6, Math.ceil(a.distanceTo(b) / 1.5)));
  const PL = 7;
  const rampTopZ = aLocal.z - 24;
  const rampFoot = local(0, 0, rampTopZ - 70);
  const rampTop = local(0, PL, rampTopZ);
  const kTurn = local(11, PL, aLocal.z - 11);
  const far = P(-760, 0, 64);
  type Leg = { pts: THREE.Vector3[]; len: number[]; dir: 1 | -1; v: number; fixed?: boolean; then?: "dump" };
  const leg = (pts: THREE.Vector3[], dir: 1 | -1, v: number, extra: Partial<Leg> = {}): Leg => {
    const len = [0];
    for (let i = 1; i < pts.length; i++) len.push(len[i - 1]! + pts[i]!.distanceTo(pts[i - 1]!));
    return { pts, len, dir, v, ...extra };
  };
  const legs: Leg[] = [
    leg([...bez(far, P(-640, 0, 44), rampFoot)], 1, 9),
    leg([...bez(rampFoot, rampFoot.clone().lerp(rampTop, 0.5), rampTop), ...bez(rampTop, local(0, PL, aLocal.z - 11), kTurn).slice(1)], 1, 3.4, { fixed: true }),
    leg(bez(kTurn, local(aLocal.x, PL, aLocal.z - 11), local(aLocal.x, PL, aLocal.z)), -1, 2.2, { fixed: true, then: "dump" }),
    leg([...bez(local(0, PL, aLocal.z), local(0, PL, rampTopZ + 4), rampTop), ...bez(rampTop, rampTop.clone().lerp(rampFoot, 0.5), rampFoot).slice(1)], 1, 3.4, { fixed: true }),
    leg(bez(rampFoot, P(-640, 0, 44), far), 1, 10),
  ];
  const tr = { i: 2, d: 6, act: "drive" as "drive" | "dump" | "away", t: 0, heading: P(0, 0, 1) };
  const tpos = new THREE.Vector3();
  const placeTruck = () => {
    if (!truck) return;
    const L = legs[tr.i]!;
    const total = L.len[L.len.length - 1]!;
    const d = clamp(tr.d, 0, total);
    let k = 1;
    while (k < L.len.length - 1 && L.len[k]! < d) k++;
    const a = L.pts[k - 1]!;
    const b = L.pts[k]!;
    tpos.lerpVectors(a, b, (d - L.len[k - 1]!) / (L.len[k]! - L.len[k - 1]! || 1));
    const dir = b.clone().sub(a).setY(0).normalize();
    if (L.dir < 0) dir.negate();
    if (dir.lengthSq() > 0.5) tr.heading.lerp(dir, 0.25).normalize();
    const y = L.fixed ? tpos.y : H(tpos.x, tpos.z);
    truck.group.position.set(tpos.x, y, tpos.z);
    truck.group.rotation.set(0, Math.atan2(tr.heading.x, tr.heading.z), 0);
    if (L.fixed && tpos.y > 0.2 && tpos.y < PL - 0.2) truck.group.rotateX(-Math.atan2(PL, 70) * (L.dir > 0 && tr.i === 1 ? 1 : -1));
  };
  if (truck) {
    truck.set({ payload: 1 });
    s.scene.add(truck.group);
    // Start facing the pocket, reversing in.
    tr.heading.copy(local(aLocal.x, PL, aLocal.z).sub(local(aLocal.x, PL, aLocal.z - 11)).setY(0).normalize().negate());
    placeTruck();
    moving.push({
      update(dt) {
        const L = legs[tr.i]!;
        const total = L.len[L.len.length - 1]!;
        if (tr.act === "dump") {
          tr.t += dt;
          const k = tr.t / 16;
          const up = k < 0.45 ? smooth(k / 0.45) : k < 0.7 ? 1 : 1 - smooth((k - 0.7) / 0.3);
          truck.set({ dump: up, payload: k < 0.5 ? 1 - smooth(k / 0.5) : 0, speed: 0 });
          crusher.set({ breaker: Math.max(0, Math.sin(Math.min(1, k) * Math.PI)) * 0.4 });
          if (k >= 1) {
            tr.act = "drive";
            tr.i++;
            tr.d = 0;
          }
        } else {
          const remain = total - tr.d;
          const v = L.v * (L.then || L.dir < 0 ? clamp(remain / 8, 0.25, 1) : 1);
          tr.d += v * dt;
          truck.set({ speed: v * L.dir });
          if (tr.d >= total) {
            if (L.then === "dump") {
              tr.act = "dump";
              tr.t = 0;
            } else {
              tr.i = (tr.i + 1) % legs.length;
              tr.d = 0;
              if (tr.i === 0) truck.set({ payload: 1 });
            }
          }
        }
        placeTruck();
        truck.update(dt);
      },
    });
  }

  await nextFrame();
  /* ---------- Light vehicles ---------- */
  const parked: [number, number, number, string?][] = [
    [-112, 99, 0],
    [-107, 99, 0, "#dcdcd6"],
    [-176, 100, Math.PI],
    [-166, 100, Math.PI, "#c9c9c2"],
    [190, 64, 0.2],
    [146, 22, 1.4],
    [40, 76, Math.PI / 2],
  ];
  if (!small) {
    // Parked vehicles never move: merged into one mesh per material, they cost a handful of draw calls, not 200.
    const lot = new THREE.Group();
    for (const [x, z, ry, c] of parked) {
      const p = makePickup(c ? { colour: c } : {});
      p.group.position.set(x, H(x, z), z);
      p.group.rotation.y = ry;
      lot.add(p.group);
    }
    await nextFrame();
  // The pickups each carry their own copies of the lamp lenses: share one material per look, so the baked lot is
  // one draw call per finish rather than one per pickup.
  {
    const seen = new Map<string, THREE.Material>();
    lot.traverse((o) => {
      const m = o as THREE.Mesh;
      if (!m.isMesh || Array.isArray(m.material)) return;
      const mt = m.material as THREE.MeshStandardMaterial;
      const key = `${mt.type}|${mt.name}|${mt.color?.getHexString()}|${mt.roughness}|${mt.metalness}|${mt.map?.uuid}|${mt.emissive?.getHexString()}|${mt.transparent}`;
      const got = seen.get(key);
      if (got) m.material = got;
      else seen.set(key, mt);
    });
  }
  const merged = bake(lot, { mud: 0.6, dust: 0.8 });
    merged.name = "parked";
    merged.traverse((m) => {
      m.castShadow = true;
      m.receiveShadow = true;
    });
    s.scene.add(merged);
  }
  // One pickup on the plant road loop.
  const loopPts = [P(-120, 0, 118), P(240, 0, 118), P(240, 0, -66), P(-120, 0, -66)];
  const loopLen = loopPts.map((p, i) => p.distanceTo(loopPts[(i + 1) % loopPts.length]!));
  await nextFrame();
  const drive = small ? null : makePickup({ colour: "#e8e6df" });
  if (drive) {
    s.scene.add(drive.group);
    let d = 120;
    const total = loopLen.reduce((a, b) => a + b, 0);
    const at = new THREE.Vector3();
    moving.push({
      update(dt) {
        d = (d + dt * 9) % total;
        let k = 0;
        let rem = d;
        while (rem > loopLen[k]!) rem -= loopLen[k++]!;
        const a = loopPts[k]!;
        const b = loopPts[(k + 1) % loopPts.length]!;
        at.lerpVectors(a, b, rem / loopLen[k]!);
        const dir = b.clone().sub(a).normalize();
        // Keep left of the centreline.
        at.addScaledVector(P(dir.z, 0, -dir.x), 2.2);
        drive.group.position.set(at.x, H(at.x, at.z), at.z);
        drive.group.rotation.y = Math.atan2(dir.x, dir.z);
        drive.set({ speed: 9 });
        drive.update(dt);
      },
    });
  }

  await nextFrame();

  /* ---------- People ---------- */
  const people: { m: Miner; st: StageId[] }[] = [];
  const person = (o: Parameters<typeof makeMiner>[0], at: THREE.Vector3, work: Parameters<Miner["work"]>[0], look: THREE.Vector3, st: StageId[]) => {
    const m = makeMiner({ detail: 1, ...o });
    m.group.position.copy(at);
    m.face(look);
    m.work(work, look);
    s.scene.add(m.group);
    people.push({ m, st });
    return m;
  };
  person({ role: "operator", seed: 4 }, local(-7, PL, aLocal.z - 3), "radio", local(0, PL, aLocal.z + 6), ["crush", "plant"]);
  person({ role: "engineer", seed: 7 }, sag.walk.clone().add(P(0.6, 0, 0)), "inspect", P(sag.walk.x + 2.5, sag.axisY, 30), ["grind"]);
  person({ role: "operator", seed: 12 }, cycs[0]!.deck.clone(), "point", cycs[0]!.top.clone().setY(cycs[0]!.top.y - 1.5), ["grind"]);
  person({ role: "operator", seed: 14 }, balls[0]!.walk.clone().add(P(0.4, 0, 0)), "inspect", P(balls[0]!.walk.x + 2.5, balls[0]!.axisY, 64), ["grind"]);
  person({ role: "operator", seed: 13 }, P(sag.motor.x - 1, 0.05, sag.motor.z + 3.4), "tablet", sag.motor.clone(), ["grind"]);
  person({ role: "operator", seed: 2 }, P(rough[1]!.lip.x + 0.4, rough[1]!.deck.y, 61.55), "point", P(rough[1]!.lip.x - 1.6, rough[1]!.top, 61), ["float"]);
  person({ role: "operator", seed: 15 }, P(rough[3]!.lip.x - 0.6, rough[3]!.deck.y, rough[3]!.lip.z + 0.8), "inspect", P(rough[3]!.lip.x - 1.8, rough[3]!.top, rough[3]!.lip.z + 2.4), ["float"]);
  person({ role: "engineer", seed: 11, board: "tablet" }, P(ts301.x - 1.4, 0.32, ts301.z + 3.2), "tablet", ts301.clone().setY(2), ["float", "plant"]);
  if (!small) {
    const walker = person({ role: "operator", seed: 9 }, P(CIL0 + 22, cil[2]!.deck.y, 34), "stand", P(CIL0 + 30, cil[2]!.deck.y, 34), ["leach"]);
    const route = [P(CIL0 + 1.6 * CILD, cil[1]!.deck.y, 33.7), P(CIL0 + 3.4 * CILD, cil[3]!.deck.y, 33.7)];
    walker.walkTo([route[1]!], 0.9);
    let back = false;
    let tw = 0;
    moving.push({
      update(dt) {
        tw += dt;
        if (tw > 26) {
          tw = 0;
          back = !back;
          walker.walkTo([route[back ? 0 : 1]!], 0.9);
        }
      },
    });
  }
  person({ role: "operator", seed: 17 }, P(cilEnd + 12 - 1.6, 8.05, 35.4), "inspect", P(cilEnd + 12, 9, 34), ["leach"]);
  person({ role: "operator", seed: 5 }, P(gold.furnace.x - 0.4, 0.38, gold.furnace.z + 1.5), "stand", gold.moulds, ["pour"]);
  person({ role: "operator", seed: 6 }, P(gold.moulds.x + 0.4, 0.38, gold.moulds.z + 1.3), "inspect", gold.moulds, ["pour"]);
  person({ role: "supervisor", seed: 3 }, P(gold.scale.x - 0.5, 0.38, gold.scale.z + 1.4), "tablet", gold.scale, ["pour"]);
  person({ role: "operator", seed: 8 }, P(ts501.x + 7.5, 0.05, ts501.z + 3.2), "inspect", P(ts501.x + 7.5, 1, ts501.z + 1.4), ["tails"]);
  person({ role: "engineer", seed: 19 }, P(TT.x + 10, tailsT.top + 1.06, TT.z + 0.4), "point", P(TT.x + 4, tailsT.top, TT.z - 6), ["tails"]);
  for (const p of people) moving.push(p.m);

  mark("people");
  await nextFrame();

  /* ---------- Streams: the flow overlay ---------- */
  const COL = { ore: "#f2a33c", slurry: "#34d3b5", conc: "#ffd166", tails: "#b7a2fb", solution: "#6cc8f7", gold: "#ffe08a", water: "#5fd38d" };
  const up = (pts: THREE.Vector3[], dy = 0.05) => pts.map((p) => p.clone().setY(p.y + dy));
  const flows: FlowLine[] = [
    { pts: [crHead.clone().add(P(0, 0.6, 0)), P(-262, 25.6, 30), P(pile.x - 1, DOME.h + 5.3, 30)], colour: COL.ore, speed: BELT_V, bend: 1 },
    { pts: [P(RT + 1, 2.4, 30), cv2End.clone().add(P(0, 1.2, 0))], colour: COL.ore, speed: BELT_V, bend: 1 },
    { pts: up(R.sagOut), colour: COL.slurry, speed: 2.5 },
    { pts: up(R.bm1Out), colour: COL.slurry, speed: 2.5 },
    { pts: up(R.bm2Out), colour: COL.slurry, speed: 2.5 },
    { pts: up(R.cycFeed1), colour: COL.slurry, speed: 3 },
    { pts: up(R.cycFeed2), colour: COL.slurry, speed: 3 },
    { pts: up(R.under1), colour: COL.slurry, speed: 2.5 },
    { pts: up(R.under2c), colour: COL.slurry, speed: 2.5 },
    { pts: [...up(R.over1), ...up(R.screen)], colour: COL.slurry, speed: 2.5 },
    { pts: up(R.over2), colour: COL.slurry, speed: 2.5 },
    { pts: rough.map((c) => c.lip.clone().setY(c.top + 0.5)), colour: COL.slurry, speed: 1.5, bend: 2 },
    { pts: [P(2, 6.1, 55.2), P(56, 4.5, 55.2), P(58, 4, 50), P(30, 4.5, 40), P(16, 4.5, 40)], colour: COL.conc, speed: 1.6, bend: 2 },
    { pts: [P(15, 4.3, 43.8), P(40, 3.7, 43.8), P(46, 3.6, 47), ...up(R.concOut)], colour: COL.conc, speed: 1.6 },
    { pts: up(R.concUnder), colour: COL.conc, speed: 1.4 },
    { pts: cil.map((c) => c.centre.clone().setY(c.top + 0.3)), colour: COL.conc, speed: 1.2, bend: 2 },
    { pts: [...up(R.cilTails), ...up(R.detox.slice(0, -1)), P(36, 0.65, -240)], colour: COL.tails, speed: 2 },
    { pts: [...up(R.roughTails), ...up(R.toThick)], colour: COL.tails, speed: 2.4 },
    { pts: [...up(R.under2), ...up(R.toDam.slice(0, -1)), P(31, 0.85, -240)], colour: COL.tails, speed: 2.4 },
    { pts: [...up(R.carbon), ...up(R.eluate), P(gold.furnace.x, 1.6, gold.furnace.z)], colour: COL.gold, speed: 1.2 },
    { pts: up(R.water), colour: COL.water, speed: 2 },
    { pts: up(R.overflow), colour: COL.water, speed: 2 },
  ];
  const flow = makeFlows(flows);
  s.scene.add(flow.mesh);

  /* ---------- Stream labels ---------- */
  const streams: Stream[] = [
    { id: "h", sid: "S-01", name: "Ore tipped at the crusher", q: ["h"], inst: "Grade-control model", at: local(0, 16, aLocal.z + 8), stage: "crush" },
    { id: "t1", sid: "S-02", name: "CV-01 · crusher product", q: ["t1"], inst: "Belt scale WT-101", at: P(-300, 21, 30), stage: "crush" },
    { id: "dS", sid: "S-03", name: "Stockpile change", q: ["dS"], inst: "Drone survey of the dome’s pile", at: dome.crown.clone(), stage: "crush" },
    { id: "t2", sid: "S-04", name: "CV-02 · mill feed", q: ["t2"], inst: "Belt scale WT-201", at: sag.feed.clone().add(P(-3.5, 2.2, 0)), stage: "grind", key: true },
    { id: "t3", sid: "S-05", name: "Flotation feed", q: ["t3", "g3"], inst: "FT-301, DT-301, cyclone-overflow sampler", at: P(-17, 13, 54.5), stage: "grind" },
    { id: "t4", sid: "S-06", name: "Concentrate", q: ["t4", "g4"], inst: "FT-341, DT-341, concentrate sampler", at: P(60, 3.6, 47), stage: "float" },
    { id: "g5", sid: "S-07", name: "Flotation tailings · TS-301", q: ["t5", "g5"], inst: "Tonnes by difference; sampler TS-301", at: ts301.clone().setY(6.6), stage: "float", key: true },
    { id: "g6", sid: "S-08", name: "Leach tailings", q: ["t6", "g6"], inst: "Tonnes inferred; CIL tailings sampler", at: P(cilEnd + 12, 10.4, 34), stage: "leach" },
    { id: "D", sid: "S-09", name: "Doré, gold room", q: ["D"], inst: "Bar weights and fire assays", at: gold.moulds.clone().add(P(0, 0.5, 0)), stage: "pour", key: true },
    { id: "g7", sid: "S-10", name: "To the dam · TS-501", q: ["t7", "g7"], inst: "FT-501, DT-501, sampler TS-501", at: P(ts501.x + 7.5, 4.6, ts501.z), stage: "tails", key: true },
  ];
  const labels = new Map<string, Label>();
  for (const st of streams) labels.set(st.id, s.label("", st.at, { className: "stem pl-tag" }));
  // On the SAG mill's section: where the outer layer of the charge leaves the shell, and where it lands.
  const SP = sag.physics;
  // Two tags on the cut-open SAG mill; a phone gets the short form (its step card carries the speed), so the tags
  // do not cover the charge they point at.
  const millTagText = [
    { at: sag.shoulderAt, html: `<span class="pl-n">Shoulder · ${Math.round(SP.shoulder)}° above level</span><b>${SP.rpm.toFixed(1)} rpm = ${Math.round(SP.phi * 100)}% of critical</b>`, short: `<span class="pl-n">Shoulder · ${Math.round(SP.shoulder)}°</span>` },
    { at: sag.toeAt, html: `<span class="pl-n">Toe · ${Math.round(SP.toe)}° below level</span><b>${SP.flight.toFixed(1)} s in flight, ${Math.round(SP.fill * 100)}% charge</b>`, short: `<span class="pl-n">Toe · ${SP.flight.toFixed(1)} s in flight</span>` },
  ];
  const millTags = millTagText.map((t) => s.label("", t.at, { className: "stem pl-tag pl-phystag" }));
  if (location.search.includes("stage")) (window as unknown as { __plant: unknown }).__plant = { streams, s, sag };

  /* ---------- State ---------- */
  let reconciled = false;
  let sheet = false;
  let current: StageId = "plant";

  const valueOf = (id: string, v: number) => {
    const q = Q[IX[id]!]!;
    if (id === "D") return `${doreKg(v).toFixed(1)} kg/d`;
    if (q.unit === "t/h") return tph(v);
    return gpt(v);
  };
  /** The value used for anything derived: reconciled when Reconcile is on, else the reading (or the first fit's fill). */
  const val = (id: string) => {
    const i = IX[id]!;
    if (reconciled) return B.fit.x[i]!;
    return Q[i]!.sd === null ? B.first.x[i]! : Q[i]!.value;
  };
  const shown = (id: string) => {
    const i = IX[id]!;
    const q = Q[i]!;
    const r = q.value;
    const f = B.fit.x[i]!;
    if (!reconciled) return q.sd === null ? `<i class="pl-inf">${valueOf(id, B.first.x[i]!)}</i>` : valueOf(id, r);
    if (q.sd === null) return `<i class="pl-inf">${valueOf(id, f)}</i>`;
    const moved = Math.abs(f / r - 1) > 0.004;
    return moved ? `<s>${valueOf(id, r).replace(/ .*/, "")}</s> <em class="pl-new">${valueOf(id, f)}</em>` : valueOf(id, f);
  };
  const evOf = (st: Stream): string => {
    const qs = st.q.map((id) => Q[IX[id]!]!);
    if (reconciled && st.q.includes(WORST.id)) return chip("refused", "set aside");
    if (reconciled) return chip("estimated", "reconciled");
    if (qs.some((q) => q.sd === null)) return chip("measured") + chip("estimated", "inferred");
    return chip(qs[0]!.ev);
  };
  const paintLabels = () => {
    const phone = view.clientWidth < 700;
    for (const st of streams) {
      const l = labels.get(st.id)!;
      const worst = reconciled && st.q.includes(WORST.id);
      // On a phone the grinding stop keeps to the mill feed and the two charge tags; flotation feed waits for Float.
      const crowd = phone && current === "grind" && st.id === "t3";
      l.show(sheet || (current === "plant" ? !!st.key : st.stage === current && !crowd));
      l.el.classList.toggle("worst", worst);
      l.el.classList.toggle("rec", reconciled);
      l.el.classList.toggle("fs", sheet);
      const age = st.q.map((id) => Q[IX[id]!]!).find((q) => q.sd !== null)?.age ?? "";
      if (sheet) {
        // Flowsheet overlay: stream number, values, and an evidence mark (M measured, I inferred, R reconciled).
        const mk = worst ? `<span class="pl-m bad">trusted least</span>` : reconciled ? `<span class="pl-m r">R</span>` : st.q.some((id) => Q[IX[id]!]!.sd === null) ? `<span class="pl-m i">M+I</span>` : `<span class="pl-m m">M</span>`;
        // On a phone each flowsheet label keeps to one line: the current values only, without the struck-out reading.
        const vals = st.q.map(shown).join(" · ");
        l.html(`<span class="pl-id">${st.sid}</span><b>${phone ? vals.replace(/<s>.*?<\/s> /g, "") : vals}</b>${mk}`);
      } else if (current === "plant" || phone)
        l.html(`<span class="pl-n">${st.name}</span><b>${st.q.map(shown).join(" · ")}</b>${phone && current !== "plant" ? `<span class="pl-ev">${evOf(st)}</span>` : ""}`);
      else l.html(`<span class="pl-n">${st.sid} · ${st.name}</span><span class="pl-ev">${evOf(st)}</span><b class="pl-v">${st.q.map(shown).join(" · ")}</b><small>${st.inst}${age && age !== "no meter" ? ` · ${age} ago` : ""}</small>`);
    }
    millTags.forEach((l, i) => {
      l.html(phone ? millTagText[i]!.short : millTagText[i]!.html);
      l.show(current === "grind" && !sheet);
    });
  };

  /* ---------- Governing numbers: physics from the balance and the vessel sizes ---------- */
  const RHO_S = 2.75; // ore solids, t/m³
  const slurry = (t: number, solids: number) => t / RHO_S + (t * (1 - solids)) / solids; // m³/h
  const cellVol = (r: number, h: number) => Math.PI * r * r * h * 0.85; // active volume
  const roughV = Array.from({ length: 6 }, (_, i) => cellVol(3.75, 7.3 - i * 0.4)).reduce((a, b) => a + b, 0);
  const cleanV = Array.from({ length: 3 }, (_, i) => cellVol(2.75, 5.6 - i * 0.3)).reduce((a, b) => a + b, 0);
  const cilV = 6 * Math.PI * 5.2 * 5.2 * 11.5 * 0.95;
  const pileT = ((Math.PI * (26 / Math.tan((37 * Math.PI) / 180)) ** 2 * 26) / 3) * 1.7;
  const f0 = (v: number, d = 0) => v.toLocaleString("en-GB", { maximumFractionDigits: d, minimumFractionDigits: d });
  const row = (k: string, v: string, note = "") => `<tr><th>${k}</th><td>${v}</td>${note ? `<td class="n">${note}</td>` : "<td></td>"}</tr>`;
  // The FrankenMining engine's Bond grinding (fmine-plant, compiled to wasm) for the two ball mills, over the blend's
  // work-index scenarios; and the same engine asked about the SAG feed, which its declared domain refuses.
  const BALL = { wi: [11.6, 12.9, 14.1, 15.3, 16.8], weights: [0.1, 0.22, 0.36, 0.22, 0.1], f80: 2400, p80: 106, powerKw: 2 * 10500, utilization: 0.93 };
  let eng: GrindResult | null = null;
  let engFail = "";
  let refusal: { code: string; detail: string } | null = null;
  /** What a person can change: the grind target and how hard the ore is (a multiplier on every scenario's Wi). */
  const ballIn = { p80: BALL.p80, hard: 1 };
  const WI_MEAN = BALL.wi.reduce((a, w, i) => a + w * BALL.weights[i]!, 0);
  const engOut = () => {
    if (engFail) return `The engine did not load here (${engFail}); nothing stands in for it.`;
    if (!eng) return "Loading FrankenMining’s engine…";
    const t = eng.throughput;
    const feed = val("t2");
    const load = feed / t.p50;
    const ms = eng.ms < 1 ? eng.ms.toFixed(2) : eng.ms.toFixed(1);
    return `<b>${f0(t.p50)} t/h</b> median; p10–p90 ${f0(t.p10)}–${f0(t.p90)}; ${f0(eng.energy.p50, 1)} kWh/t.
The plant feeds ${f0(feed)} t/h: ${load <= 1 ? `<span class="pl-ok">${f0((1 - load) * 100, load > 0.98 ? 1 : 0)}% headroom</span>` : `<span class="pl-bad">${f0((load - 1) * 100, load < 1.02 ? 1 : 0)}% over capacity</span>`} at the median${t.p10 < feed ? `, and in the hardest tenth of the ore the mills take only ${f0(t.p10)} t/h` : ""}.
Averaging the hardness first would say ${f0(eng.atMeanWi.throughput)} t/h. ${chip("estimated", "modelled, uncalibrated")} <span class="pl-ms">computed in your browser in ${ms} ms</span>`;
  };
  const engine = () => `<div class="pl-eng"><span class="micro">FrankenMining engine · Bond grinding · wasm, live</span>
<p>Two ball mills, ${f0(BALL.powerKw / 1000)} MW at ${f0(BALL.utilization * 100)}% run time, ${f0(BALL.f80 / 1000, 1)} mm cyclone underflow, five hardness cases. Move either input and the engine runs again.</p>
<label class="pl-sl"><span>Grind P80</span><input type="range" data-eng="p80" min="75" max="180" step="1" value="${ballIn.p80}" aria-label="Grind target P80, micrometres"><output>${ballIn.p80} µm</output></label>
<label class="pl-sl"><span>Ore hardness</span><input type="range" data-eng="hard" min="0.8" max="1.3" step="0.01" value="${ballIn.hard}" aria-label="Ore hardness as a multiple of the blend’s work index"><output>Wi ${f0(WI_MEAN * ballIn.hard, 1)}</output></label>
<p data-eng-out>${engOut()}</p>
${refusal ? `<p class="pl-ref">Asked about the SAG mill’s own feed (F80 150 mm), the engine refuses rather than extrapolate: <code>${refusal.code}</code> ${refusal.detail}</p>` : ""}</div>`;
  let engBusy = false;
  let engAgain = false;
  const runEngine = () => {
    if (engBusy) {
      engAgain = true;
      return;
    }
    engBusy = true;
    grind({ ...BALL, p80: ballIn.p80, wi: BALL.wi.map((w) => w * ballIn.hard) })
      .then((r) => {
        eng = r;
        engFail = "";
      })
      .catch((e: unknown) => {
        engFail = e instanceof EngineError ? `${e.code ?? "refused"}` : e instanceof Error ? e.message.slice(0, 80) : "unknown";
      })
      .finally(() => {
        engBusy = false;
        for (const el of host.querySelectorAll("[data-eng-out]")) el.innerHTML = engOut();
        if (engAgain) {
          engAgain = false;
          runEngine();
        }
      });
  };
  host.addEventListener("input", (e) => {
    const t = e.target as HTMLInputElement;
    const k = t.dataset.eng as keyof typeof ballIn | undefined;
    if (!k) return;
    ballIn[k] = Number(t.value);
    const out = t.nextElementSibling;
    if (out) out.textContent = k === "p80" ? `${ballIn.p80} µm` : `Wi ${f0(WI_MEAN * ballIn.hard, 1)}`;
    runEngine();
  });
  const physics = (id: StageId) => {
    const t2 = val("t2");
    const t3 = val("t3");
    const t4 = val("t4");
    const t7 = val("t7");
    if (id === "crush")
      return `<table class="pl-phys">${row("CV-01", `${BELT_V} m/s, ${f0(kgPerM(val("t1")))} kg/m`, `= ${f0(val("t1"))} t/h`)}${row("Stockpile", `≈ ${f0(Math.round(pileT / 1000))} kt`, `${f0(pileT / t2)} h of mill feed`)}${row("Dome", `${DOME.r * 2} m across, ${DOME.h} m high`)}</table>`;
    if (id === "grind") {
      const S = sag.physics;
      const Bm = balls[0]!.physics;
      return `<table class="pl-phys">${row("SAG 11.6 × 6.1 m", `${f0(S.rpm, 1)} rpm = ${f0(S.phi * 100)}% of Nc`, `Nc = 42.3/√${f0(S.D, 1)} = ${f0(S.Nc, 1)} rpm`)}${row("Charge", `${f0(S.fill * 100)}% full`, `leaves ${f0(S.shoulder)}° above level, ${f0(S.flight, 1)} s in flight`)}${row("Ball mills 7.3 × 11 m", `${f0(Bm.rpm, 1)} rpm = ${f0(Bm.phi * 100)}% of Nc`, `Nc = ${f0(Bm.Nc, 1)} rpm`)}${row("CV-02", `${BELT_V} m/s, ${f0(kgPerM(t2))} kg/m`, `= ${f0(t2)} t/h`)}</table>${engine()}`;
    }
    if (id === "float") {
      const q = slurry(t3, 0.33);
      return `<table class="pl-phys">${row("Feed slurry", `${f0(q)} m³/h`, "33% solids")}${row("Roughers", `6 × ${f0(cellVol(3.75, 7.3) / 0.85)} m³`, `τ = V/Q = ${f0((roughV / q) * 60)} min`)}${row("Mass pull", `${f0((t4 / t3) * 100, 1)}%`, `${f0(t4)} t/h of solids over the lips`)}${row("Cleaners", `3 × ${f0(cellVol(2.75, 5.6) / 0.85)} m³`, `τ = ${f0((cleanV / slurry(t4 * 1.6, 0.25)) * 60)} min`)}</table>`;
    }
    if (id === "leach") {
      const q = slurry(t4, 0.45);
      return `<table class="pl-phys">${row("Thickener 22 m", `${f0((Math.PI * 121) / (t4 * 24), 2)} m²/(t/d)`, "underflow 60% solids")}${row("CIL", `6 × ${f0(cilV / 6 / 0.95)} m³`, `τ = V/Q = ${f0(cilV / q, 1)} h at 45% solids`)}${row("Leach tails", `${f0(val("t6"))} t/h, ${gpt(val("g6"))}`)}</table>`;
    }
    if (id === "pour") return `<table class="pl-phys">${row("Doré", `${f0(doreKg(val("D")), 1)} kg/d`, `${f0((doreKg(val("D")) * 1000) / G_PER_OZ)} oz/d`)}${row("Bars", `≈ ${f0(doreKg(val("D")) / 22)} a day`, "about 22 kg each")}${row("Assay", "fire assay, ± 0.4%", "the plant’s most precise number")}</table>`;
    if (id === "tails") {
      const qIn = slurry(t3 - t4, 0.31);
      const qU = slurry(t7, 0.62);
      return `<table class="pl-phys">${row("Thickener 40 m", `${f0((Math.PI * 400) / (t7 * 24), 3)} m²/(t/d)`, "high-rate")}${row("Underflow", `${f0(qU)} m³/h`, "62% solids, to the dam")}${row("Water back", `${f0(qIn - qU)} m³/h`, "overflow to process water")}</table>`;
    }
    return `<table class="pl-phys">${row("Throughput", `${f0(t2)} t/h`, `${f0((t2 * 24) / 1000, 1)} kt a day`)}${row("Gold in feed", `${f0((t2 * val("g3") * 24) / G_PER_OZ)} oz a day`)}</table>`;
  };
  const NOTE = `<p class="pl-src">Computed on this page from the balance and the vessel sizes (residence is V/Q); illustrative.</p>`;
  const paintStep = () => {
    const st = STAGES[current];
    const phone = view.clientWidth < 700;
    step.innerHTML = `<p class="micro">${st.title}</p><p>${phone ? st.short : st.text}</p>${wide() ? physics(current) + NOTE : ""}`;
    const card = below.querySelector(".pl-physcard");
    if (card) card.innerHTML = `<p class="micro">${st.title} · the numbers</p>${physics(current)}${NOTE}`;
  };

  /* ---------- HUD ---------- */
  const zText = (z: number) => `${Math.abs(z).toFixed(1)}σ`;
  const first = B.first;
  const limit1 = CHI2_95[first.dof]!;
  const limit2 = CHI2_95[B.fit.dof]!;
  const r80 = 1.2816 * B.recoverySd;
  const swap = (a: string, b: string) => (reconciled ? `<s>${a}</s> <b class="pl-new">${b}</b>` : `<b>${a}</b>`);
  const paintHud = () => {
    const phone = view.clientWidth < 700;
    // One evidence chip for the card on wide screens (the rows have no room for three); one per figure on phones.
    const ev0 = chip(reconciled ? "estimated" : "measured", reconciled ? "reconciled" : "measured");
    const kv = (k: string, v: string, sub: string, ev: string) => `<div class="pl-k"><span>${k}</span><span class="pl-kv">${v}</span>${phone ? ev : ""}<small>${sub}</small></div>`;
    kpi.innerHTML = `<p class="micro pl-kh"><span>Plant · last 24 h · synthetic</span>${ev0}</p><div class="pl-ks">
${kv("Throughput", swap(tph(raw("t2")), tph(fit("t2"))), reconciled ? "reconciled; WT-201 read" : "belt scale WT-201", chip(reconciled ? "estimated" : "measured", reconciled ? "reconciled" : "measured"))}
${kv("Head grade", swap(`${B.rawHead.toFixed(2)} g/t`, `${B.head.toFixed(2)} g/t`), reconciled ? `± ${B.headSd.toFixed(2)} g/t, from the balance` : "feed sampler", chip(reconciled ? "estimated" : "measured", reconciled ? "reconciled" : "measured"))}
${kv("Recovery", swap(pct(B.rawRecovery), pct(B.recovery)), reconciled ? `80%: ${pct(B.recovery - r80)}–${pct(B.recovery + r80)}` : "as the samplers report it", chip(reconciled ? "estimated" : "derived", reconciled ? "reconciled" : "derived"))}
</div>`;
    const nodes = [
      ["Crush, stockpile", 0, -1],
      ["Grind", 1, 2],
      ["Float", 3, 4],
      ["Thicken, leach", 5, 6],
      ["Tails thickener", 7, 8],
    ] as const;
    // Relative closure of each node's equation: solids against the flow in, gold against the gold in.
    const scale = [raw("t1"), raw("t2"), raw("t2") * raw("g3"), raw("t3"), raw("t3") * raw("g3"), raw("t4"), raw("t4") * raw("g4"), raw("t3"), raw("t3") * raw("g5")];
    const cell = (k: number) => {
      if (k < 0) return `<td class="na">–</td>`;
      const res = reconciled ? B.fit.residual[k]! : B.first.rawResidual[k]!;
      const rel = res / scale[k]!;
      const ok = Math.abs(rel) < 0.0005;
      return `<td class="${ok ? "ok" : Math.abs(rel) > 0.05 ? "bad" : "off"}">${ok ? "closes" : `${rel > 0 ? "+" : "−"}${Math.abs(rel * 100).toFixed(1)}%`}</td>`;
    };
    const table = `<table class="pl-nodes"><thead><tr><th>Node</th><th>Solids</th><th>Gold</th></tr></thead><tbody>${nodes.map(([n, a, b]) => `<tr><th>${n}</th>${cell(a)}${cell(b)}</tr>`).join("")}</tbody></table>`;
    const balHtml = reconciled
      ? `<p class="micro">Balance · reconciled</p>${table}
<p class="pl-pass">All readings in: χ² <b>${first.chi2.toFixed(1)}</b>, over the ${limit1.toFixed(1)} limit. Trusted least: <b class="pl-bad">${tag(WORST.id)}</b>, adjusted ${zText(first.z[B.worst]!)}; next ${tag(NEXT.id)}, ${zText(first.z[B.next]!)}.</p>
<p class="pl-pass">${tag(WORST.id)} set aside: χ² <b>${B.fit.chi2.toFixed(1)}</b>, under ${limit2.toFixed(1)}. Every node conserves solids and gold.</p>`
      : `<p class="micro">Balance · raw readings</p>${table}
<p class="pl-pass">${Q.filter((q) => q.sd !== null).length} readings, ${Q.filter((q) => q.sd === null).length} inferred. As read, the gold does not close at flotation or at the tailings thickener.</p>`;
    const over = B.overstatedOzMonth;
    const agentHtml = reconciled
      ? `<p class="micro">Agent · metallurgical accounting</p>
<p>${tag(WORST.id)} reads low: its cutter was last inspected 94 days ago, ${tag(NEXT.id)}’s 6 days ago. Reported recovery is ${((B.rawRecovery - B.recovery) * 100).toFixed(1)} points high, about <b>${fmt(Math.round(over / 10) * 10)} oz a month</b> (${"$"}${((over * 2800) / 1e6).toFixed(1)}M at a $2,800/oz deck) that is in the tailings.</p>
<p>Drafted: inspect the cutter, take duplicate tailings samples, report recovery from the balance until it is fixed.</p>
<span class="act">For the plant metallurgist to approve</span>`
      : `<p class="micro">Agent · metallurgical accounting</p>
<p>The gold does not close: flotation has gold left over and the tailings thickener sends out more than it receives. The two tailings samplers, ${tag("g5")} and ${tag("g7")}, disagree by a third.</p>
<p>Turn on <b>Reconcile</b> to see which reading the balance trusts least, and what it is worth.</p>`;
    bal.innerHTML = balHtml;
    agent.innerHTML = agentHtml;
    if (!wide()) below.innerHTML = `<div class="pl-card pl-physcard"></div><div class="pl-card">${balHtml}</div><div class="pl-card agent-note">${agentHtml}</div>`;
    else below.innerHTML = "";
    paintStep();
    if (reconciled) {
      for (const el of [kpi, bal]) {
        el.classList.remove("flash");
        void el.offsetWidth;
        el.classList.add("flash");
      }
    }
  };

  /* ---------- Stages and the tour ---------- */
  interface View {
    title: string;
    text: string;
    /** One line for phones. */
    short: string;
    target: [number, number, number];
    distance: number;
    azimuth: number;
    elevation: number;
    shadow: [number, number, number];
    radius: number;
    /** Phone framing, if it differs. */
    phone?: Partial<Pick<View, "target" | "distance" | "azimuth" | "elevation">>;
  }
  const STAGES: Record<StageId, View> = {
    plant: {
      title: "The whole plant",
      short: `A synthetic gold plant, about ${tph(1800)}. Follow the ore from crusher to doré.`,
      text: `A synthetic gold concentrator treating about ${tph(1800)}. Follow the ore from the crusher to the doré bar: every stream carries a reading or is inferred from the others.`,
      target: [-170, 0, 28],
      distance: 900,
      azimuth: -100,
      elevation: 42,
      shadow: [-120, 0, 25],
      radius: 420,
      phone: { target: [-125, 0, 28], distance: 1150, elevation: 46 },
    },
    crush: {
      title: "1 · Crush and stockpile",
      short: "Trucks tip into the gyratory crusher; a covered belt fills the stockpile dome.",
      text: "Trucks tip run-of-mine ore into the primary gyratory crusher. A covered conveyor carries it into the stockpile dome; a reclaim tunnel under the pile feeds the mill. Belt scale WT-101 weighs it; a drone survey measures the pile.",
      target: [-365, 8, 30],
      distance: 285,
      azimuth: -86,
      elevation: 22,
      shadow: [-320, 0, 30],
      radius: 170,
      phone: { target: [-350, 8, 30], distance: 430 },
    },
    grind: {
      title: "2 · Grind and classify",
      short: `The SAG mill, cut open: at ${SP.rpm.toFixed(1)} rpm (${Math.round(SP.phi * 100)}% of critical) its lifters carry the charge to the shoulder; it falls to the toe.`,
      text: "The SAG mill is drawn cut open at mid-length. Its lifters carry the charge up to the shoulder, where it leaves the shell and falls to the toe; the speed sets both. Two ball mills grind the cyclone underflow, and the cyclones send the fines on to flotation.",
      target: [-63, 8, 33],
      distance: 58,
      azimuth: -122,
      elevation: 20,
      shadow: [-58, 0, 42],
      radius: 70,
      phone: { target: [-63, 8, 32], distance: 82 },
    },
    float: {
      title: "3 · Float",
      short: "Froth carries the gold-bearing sulphides over the lips into the launders.",
      text: "Air bubbles carry gold-bearing sulphides into the froth, which spills over the lips into the launders. A few per cent of the mass and most of the gold go on as concentrate; the rest is tailings, sampled by TS-301.",
      target: [36, 4, 54],
      distance: 112,
      azimuth: -62,
      elevation: 34,
      shadow: [36, 0, 54],
      radius: 70,
      phone: { target: [36, 4, 54], distance: 150 },
    },
    leach: {
      title: "4 · Thicken and leach",
      short: "Concentrate is thickened, then leached in six agitated tanks with carbon.",
      text: "The concentrate is thickened under a turning rake, then leached with cyanide in six agitated tanks, where activated carbon takes up the gold. The leach tailings go to cyanide destruction.",
      target: [140, 6, 26],
      distance: 140,
      azimuth: -66,
      elevation: 28,
      shadow: [136, 0, 26],
      radius: 85,
      phone: { target: [140, 6, 26], distance: 200 },
    },
    pour: {
      title: "5 · Elute and pour",
      short: "Gold is stripped, plated and smelted; the bars are weighed and assayed.",
      text: "Gold is stripped from the carbon, plated in electrowinning cells and smelted. The bars are weighed and fire-assayed: the plant’s most precise number.",
      target: [168.6, 1.4, -10.6],
      distance: 15,
      azimuth: 14,
      elevation: 34,
      shadow: [164, 0, -10],
      radius: 22,
      phone: { target: [168.4, 1.4, -10.4], distance: 21 },
    },
    tails: {
      title: "6 · Tailings",
      short: "Tailings are thickened and pumped to the dam; the water comes back.",
      text: "Flotation tailings are thickened and pumped to the dam, sampled again by TS-501. Thickener overflow returns as process water. Leach tailings go through cyanide destruction on their own line.",
      target: [30, 2, -26],
      distance: 128,
      azimuth: -30,
      elevation: 30,
      shadow: [30, 0, -24],
      radius: 75,
      phone: { target: [30, 2, -26], distance: 180 },
    },
  };
  const ORDER: StageId[] = ["plant", "crush", "grind", "float", "leach", "pour", "tails"];
  const stageSeg = seg(host, "stage", (v) => go(v as StageId, true));
  const go = (id: StageId, byHand = false) => {
    current = id;
    stageSeg.set(id);
    const st = STAGES[id];
    const phone = view.clientWidth < 700;
    const v = { target: st.target, distance: st.distance, azimuth: st.azimuth, elevation: st.elevation, ...(phone ? st.phone : {}) };
    s.orbit.set(v, reduced ? 0 : 2.6);
    s.shadowBox(st.shadow, st.radius);
    paintStep();
    gold.cutaway(id === "pour" ? 1 : 0);
    // Arriving at the gold room, the furnace tilts within a second or two and pours while the stop lasts.
    if (id === "pour") gold.restart(1.6);
    sag.ghost(id === "grind" ? 1 : 0);
    // People are a few pixels tall from the whole-plant view; they appear where the camera goes.
    for (const p of people) p.m.group.visible = id !== "plant" && p.st.includes(id);
    // So are the moving vehicles; leaving them out of the overview keeps it under 300 draw calls.
    if (truck) truck.group.visible = id !== "plant";
    if (drive) drive.group.visible = id !== "plant" && id !== "crush";
    if (byHand) stopTour();
    paintLabels();
    s.loop.poke();
  };

  const tourBtn = $<HTMLButtonElement>(host, "[data-tour]");
  let touring = false;
  let tourT = 0;
  const stopTour = () => {
    touring = false;
    tourBtn.setAttribute("aria-pressed", "false");
    tourBtn.querySelector("span")!.textContent = "Follow the ore";
  };
  tourBtn.addEventListener("click", () => {
    if (touring) return stopTour();
    touring = true;
    tourT = 0;
    tourBtn.setAttribute("aria-pressed", "true");
    tourBtn.querySelector("span")!.textContent = "Stop";
    const at = ORDER.indexOf(current);
    go(ORDER[at <= 0 || at === ORDER.length - 1 ? 1 : at + 1]!);
    s.loop.running = true;
  });

  toggle(host, "reconcile", (on) => {
    reconciled = on;
    paintLabels();
    paintHud();
    s.loop.poke();
  });
  toggle(host, "sheet", (on) => {
    sheet = on;
    view.classList.toggle("pl-sheet", on);
    flow.uniforms.uGap.value = on ? 6 : 9;
    paintLabels();
    s.loop.poke();
  });

  /* ---------- Frame ---------- */
  s.onNight((n) => {
    crusher.set({ lights: n > 0.35 });
    cv1.set({ lights: n > 0.35 });
  });
  s.frame((dt) => {
    surfaceTime.value += dt;
    for (const m of moving) m.update(dt);
    // The flow lines show the route from the whole-plant view, and everywhere when the flowsheet is on.
    flow.uniforms.uOn.value += ((sheet ? 1.25 : current === "plant" ? 0.8 : 0) - flow.uniforms.uOn.value) * Math.min(1, dt * 6);
    flow.mesh.visible = flow.uniforms.uOn.value > 0.01;
    // Near plane as far out as the view allows: depth precision is what keeps close faces from fighting.
    const cd = s.camera.position.distanceTo(s.orbit.target);
    const near = clamp(Math.min(cd * 0.035, (s.camera.position.y - H(s.camera.position.x, s.camera.position.z)) * 0.5), 0.25, 40);
    if (Math.abs(near / s.camera.near - 1) > 0.08) {
      s.camera.near = near;
      s.camera.updateProjectionMatrix();
    }
    // Haze thins as the camera pulls back, so the far whole-plant view (the phone's especially) keeps the plant
    // legible instead of fading it into the plain; close views keep the full density.
    if (fog) fog.density = FOG * clamp(720 / cd, 0.5, 1) * (sheet ? 0.6 : 1);
    if (touring) {
      tourT += dt;
      if (tourT > (current === "plant" ? 5 : 9)) {
        tourT = 0;
        const next = ORDER[(ORDER.indexOf(current) + 1) % ORDER.length]!;
        if (next === "plant") {
          stopTour();
          go("plant");
        } else go(next);
      }
    }
  });
  let lastLayout = layout();
  new ResizeObserver(() => {
    const k = layout();
    paintLabels();
    paintHud();
    // Crossing a layout boundary changes the framing (phones look from further out).
    if (k !== lastLayout) {
      lastLayout = k;
      go(current);
    }
  }).observe(view);

  paintHud();
  go("plant");
  runEngine();
  grind({ ...BALL, f80: 150000 }).catch((e: unknown) => {
    if (!(e instanceof EngineError)) return;
    // The crate's own words, from the clause that names the input.
    const detail = e.message.replace(/^.*?Bond grinding:\s*/, "").replace(/\s*\|.*$/, "").replace(/\s+/g, " ").trim();
    refusal = { code: e.code ?? "refused", detail: detail.length > 150 ? `${detail.slice(0, 148)}…` : detail };
    paintStep();
  });
  mark("before-compile");
  // Compile every shader before the first frame, a few at a time, so the page never stalls.
  await compileGradually(s.renderer, s.scene, s.camera);
  mark("compiled");
  s.start();
  requestAnimationFrame(() => requestAnimationFrame(() => mark("first-frames")));
  if (reduced) {
    // Still picture: one frame with everything posed.
    for (const m of moving) m.update(0.016);
    s.render();
  }
  void elu;
}
