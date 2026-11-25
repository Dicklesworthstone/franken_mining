/**
 * "Bring in the storm": a tailings storage facility at real scale in the same desert as the opening pit. The
 * embankment is raised on the centreline in lifts, with rockfill faces and berms; a spigot line on the crest pours
 * slurry onto a beach of grey tailings that falls to the pond; a decant tower stands in the pond at the end of its
 * causeway; a lined seepage pond catches the drains at the toe; survey prisms stand along the crest, inclinometers on
 * the berms; and section A–A shows, as an x-ray through the embankment, the piezometers, their trigger levels, the
 * phreatic surface and the freeboard as a dimension against its criteria.
 *
 * The storm control sets a 72-hour rainfall from the site's rainfall-frequency curve. The pond then follows a water
 * balance hour by hour (rain, runoff, slurry water, the decant pumps, evaporation, seepage) over the storm's
 * hyetograph, on the stage–storage curve of the bed itself, and the freeboard counts down. FrankenMining's own
 * kernels, compiled to wasm, compute three numbers live: the tail of the freeboard over an ensemble of storms
 * (CVaR), an estimate of the stale piezometer from its neighbours (which still cannot stand in for a reading:
 * FMINE-REF-404), and the value of reading it by hand before the rain (EVSI). The agent drafts the field reading; a
 * person approves it. Nothing here says the dam is safe or signs it off. Everything is synthetic.
 */
import * as THREE from "three";
import { makeDozer, makeLightTower, makePickup } from "./fleet";
import { $, clamp, fmt, range, reduced, seg, toggle } from "./lib";
import { makeMiner, type Miner } from "./miners";
import { stage, type Label } from "./stage";
import { at, CRITERIA, designBasis, ensemble, HOURS, RATES, simulate, STORM_HOURS, type Run, type Terms } from "./tailings-balance";
import { freeboardTail, readingValue, staleEstimate, type StaleEstimate } from "./tailings-engine";
import { makeBeach, makeEmbankment, makeGround, weather, wetRock } from "./tailings-ground";
import { makeDecant, makeHeads, makePipelines, makePrisms, makeSeepage, makeSpigotLine, OPEN_SPIGOTS } from "./tailings-kit";
import {
  ageText,
  balance,
  CREST_LOW,
  crestZ,
  damTop,
  DATES,
  edgeAt,
  HILLS_M2,
  INCLINOMETERS,
  kPa,
  PIEZOMETERS,
  PMP,
  POND,
  RL,
  SECTION_X,
  settle,
  yearsFor,
  headKPa,
} from "./tailings-model";
import { makeSection } from "./tailings-section";
import { makeSky } from "./tailings-sky";
import { makePad, makePowerLine, makeShrubs } from "./tailings-site";
import { makeChannels, makePond } from "./tailings-water";

const m2 = (v: number) => `${fmt(v, 2)} m`;
const k3 = (v: number) => `${v < 0 ? "−" : "+"}${fmt(Math.abs(v) / 1000, 1)}k`;
const NOW_FB = CREST_LOW - POND;
const NOW_BEACH = edgeAt(POND);
const STORM100 = 186;
const STALE = PIEZOMETERS.find((p) => p.stale)!;
/** The hand reading the technician brings back (synthetic). */
const HAND_KPA = 249;
/** Gauge: metres below the crest shown, and its height in SVG units. */
const G_M = 3;
const G_H = 150;

/** A return period in words. */
function returnText(mm: number) {
  if (mm <= 0) return "No storm";
  if (mm >= PMP - 0.5) return "Probable maximum";
  const y = yearsFor(mm);
  if (y < 2) return "Less than 1 in 2 years";
  const nice = y < 10 ? Math.round(y) : y < 100 ? Math.round(y / 5) * 5 : y < 1000 ? Math.round(y / 50) * 50 : Math.round(y / 500) * 500;
  return `1 in ${fmt(nice)} years`;
}

const gy = (fb: number) => 8 + (clamp(fb, 0, G_M) / G_M) * G_H;
const gaugeSvg = (storage: number) => {
  const allow = CRITERIA.wave + CRITERIA.residual;
  return `<svg class="tl-gauge" viewBox="0 0 80 ${G_H + 22}" aria-hidden="true">
<rect x="22" y="8" width="22" height="${G_H}" rx="2" class="g-well"/>
<rect x="22" y="8" width="22" height="${gy(allow) - 8}" class="g-red"/>
<rect x="22" y="${gy(allow)}" width="22" height="${gy(allow + storage) - gy(allow)}" class="g-amber"/>
<rect x="22" y="${gy(NOW_FB)}" width="22" height="${8 + G_H - gy(NOW_FB)}" class="g-now"/>
<rect x="22" data-g="live" y="${gy(NOW_FB)}" width="22" height="0" class="g-live"/>
<path data-g="low" d="M18 0H48" class="g-low" visibility="hidden"/>
<path data-g="tail" d="M18 0H48" class="g-tail" visibility="hidden"/>
<path d="M16 8H50" class="g-crest"/><text x="52" y="11">crest</text>
<text x="52" y="${gy(allow) + 3}" class="g-r">1.00</text>
<text x="52" y="${gy(allow + storage) + 3}" class="g-a">${fmt(allow + storage, 2)}</text>
<path data-g="bracket" d="M12 8V${gy(NOW_FB)}" class="g-br"/>
<text x="0" y="${G_H + 20}" class="g-scale">metres below the crest</text>
</svg>`;
};

const markup = (storage: number, pmp: ReturnType<typeof designBasis>["pmp"]) => `
<div class="tl-stage">
  <div class="scene-view tl-view"></div>
  <div class="tl-clock" data-clock aria-live="polite"></div>
  <div class="tl-mini" aria-hidden="true"><small>Freeboard</small><b data-mini>${m2(NOW_FB)}</b><span data-mini-low></span></div>
  <div class="hud-card hud-tl tl-fb">
    <p class="micro">Freeboard · TSF-1 main embankment</p>
    <div class="tl-fb-row">${gaugeSvg(storage)}
      <dl>
        <dt>Now</dt><dd><b data-fb-now>${m2(NOW_FB)}</b><span class="ev derived">derived</span></dd>
        <dt>Lowest in this storm</dt><dd><b data-fb-low>–</b><span class="ev estimated">estimated</span></dd>
        <dt>95% tail (CVaR)</dt><dd><b data-fb-tail>–</b><span class="ev estimated">estimated</span></dd>
      </dl>
    </div>
    <p class="tl-crit"><span><i class="c-a"></i>Operating minimum <b>${m2(CRITERIA.wave + CRITERIA.residual + storage)}</b></span><span><i class="c-r"></i>After a storm <b>${m2(CRITERIA.wave + CRITERIA.residual)}</b></span></p>
    <p class="tl-beach">Beach, crest to water <b data-beach>${fmt(NOW_BEACH)} m</b><span data-beach-low></span> · minimum ${CRITERIA.beach} m</p>
    <p class="tl-spill">No spillway in operation: the PMP, ${fmt(pmp.mm)} mm in 72 h with the pumps stopped, ${pmp.stored ? "is stored" : "is <em>not</em> stored"} with <b>${m2(pmp.freeboard)}</b> to the crest (needs ${m2(CRITERIA.wave + CRITERIA.residual)}).</p>
    <p class="tl-warn" data-warn hidden></p>
    <p class="tl-eng" data-eng-tail>CVaR of the freeboard: bring in a storm.</p>
    <p class="tl-src">Crest low point RL ${fmt(RL(CREST_LOW), 2)}, prism P-07, survey of ${DATES.crestSurvey}. Pond RL ${fmt(RL(POND), 2)}, radar gauge at ${DATES.pondRead}. Criteria are the engineer of record’s (illustrative).</p>
  </div>
  <div class="hud-card hud-tr tl-wb">
    <p class="micro">Water balance · forecast<span class="ev estimated">estimated</span></p>
    <p class="tl-sname"><b data-sname>No storm</b><span data-smm></span></p>
    <svg class="tl-hyeto" viewBox="0 0 240 64" preserveAspectRatio="none" aria-hidden="true"><g data-bars></g><path data-lvl class="h-lvl" d=""/><path data-cur class="h-cur" d="M0 0V64"/></svg>
    <p class="tl-hour"><span data-hour>hour 0 of ${HOURS}</span><span data-peak></span></p>
    <table class="tl-terms"><tbody>
      <tr><th>Rain on the pond</th><td data-t="rainPond">0</td></tr>
      <tr><th>Runoff, beach</th><td data-t="beach">0</td></tr>
      <tr><th>Runoff, ${fmt(HILLS_M2 / 1e6, 2)} km² of hills</th><td data-t="hills">0</td></tr>
      <tr><th>Slurry water</th><td data-t="slurry">0</td></tr>
      <tr><th data-pumps-name>Decant pumps</th><td data-t="pumped">0</td></tr>
      <tr><th>Evaporation</th><td data-t="evaporation">0</td></tr>
      <tr><th>Seepage</th><td data-t="seepage">0</td></tr>
      <tr class="sum"><th>Stored in the pond, m³</th><td data-t="net">0</td></tr>
    </tbody></table>
    <p class="tl-src">Rates illustrative; seepage as measured at the weir. Stage–storage from the bed surveyed ${DATES.bathymetry}.</p>
  </div>
  <div class="hud-card hud-bl tl-inst">
    <p class="micro">Instruments · section A–A</p>
    <table class="tl-pz">
      <thead><tr><th>Tip</th><th>In</th><th>Reading</th><th>L1</th><th>Age</th></tr></thead>
      <tbody>${PIEZOMETERS.map(
        (p) =>
          `<tr class="${p.stale ? "stale" : ""}" data-pz="${p.id}"><td>${p.id}</td><td>${p.zone}</td><td data-kpa>${fmt(kPa(p))} kPa</td><td>${fmt(headKPa(p, p.l1))}</td><td data-age>${ageText(p.age)}<span class="ev ${p.stale ? "stale" : "measured"}">${p.stale ? "stale" : "measured"}</span></td></tr>${
            p.stale ? `<tr class="est" data-est><td colspan="5"><span data-est-text>Engine estimate loading…</span></td></tr>` : ""
          }`,
      ).join("")}${INCLINOMETERS.map(
        (i, k) =>
          `<tr class="inc"><td>${i.id}</td><td>berm ${k + 1}</td><td>${fmt(i.mm, 1)} mm</td><td>25</td><td>${i.days} days<span class="ev measured">measured</span></td></tr>`,
      ).join("")}</tbody>
    </table>
    <p class="tl-refuse" data-refuse><s>“Every piezometer is within its TARP”</s><span class="ev refused">refused</span><code>FMINE-REF-404</code></p>
    <p class="tl-src" data-refuse-why>Stale TSF instrumentation: no “within TARP” statement. A stale tip cannot be reported as stable.</p>
  </div>
  <div class="hud-card hud-br agent-note tl-agent">
    <p class="micro">Agent · tailings evidence</p>
    <p data-agent-est>VWP-04, in the foundation under the crest, last reported <b>19 days ago</b>.</p>
    <p data-agent-voi></p>
    <div class="tl-act" data-act><span class="act">Draft · read VWP-04 by hand at the BH-B logger before the rain; send it with this forecast to the engineer of record</span><button type="button" class="btn sm go" data-approve>Approve the field reading</button></div>
    <p class="tl-done" data-done hidden></p>
  </div>
</div>
<div class="scene-bar tl-bar">
  <button type="button" class="btn go" data-play><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 16a4.5 4.5 0 1 1 1.6-8.7A6 6 0 0 1 20 10a3.5 3.5 0 0 1-1 6.8"/><path d="m9 19-1 2M13 19l-1 2M17 19l-1 2"/></svg><span>Bring in the storm</span></button>
  <label class="range" data-range="storm"><span><b>Storm, 72-hour rain</b><output aria-live="off">no storm</output></span><input type="range" aria-label="Storm, 72-hour rainfall in millimetres" min="0" max="${PMP}" step="2" value="0" style="--p:0%"></label>
  <button type="button" class="toggle" data-toggle="pumps" aria-pressed="true"><i></i>Decant pumps</button>
  <button type="button" class="toggle" data-toggle="overlay" aria-pressed="true"><i></i>Engineering overlay</button>
  <div class="seg" role="group" aria-label="View" data-seg="view"><button type="button" data-v="dam" aria-pressed="true">Section</button><button type="button" data-v="pond" aria-pressed="false">Pond</button><button type="button" data-v="whole" aria-pressed="false">Facility</button></div>
  <p class="tl-note">Synthetic facility; rates and criteria illustrative. Drag to look around.</p>
</div>`;

type View = "dam" | "pond" | "whole";

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("tailings");
  const small = matchMedia("(max-width: 699px)").matches;
  const wide = () => matchMedia("(min-width: 1000px) and (min-height: 860px)").matches;
  const pause = () => new Promise<void>((r) => setTimeout(r, 0));
  // The stage–storage curve and the design basis come first: the criteria in the markup depend on them.
  const bal = balance(small ? 6 : 4);
  const basis = designBasis(bal);
  host.innerHTML = markup(basis.storage, basis.pmp);

  const s = stage($(host, ".tl-view"), {
    clock: 17.1,
    sunBearing: 57,
    fog: 0.00034,
    shadowRadius: 340,
    shadowAt: [20, 14, 20],
    shadowMap: small ? 2048 : 4096,
    fov: small ? 44 : 30,
    near: 4,
    far: 16000,
    maxPixelRatio: small ? 1.5 : 1.75,
    label: "A tailings storage facility in a desert valley: the embankment, the beach and the pond, with an x-ray section through the embankment showing its piezometers and its freeboard. A storm can be brought in to raise the pond.",
    labelArea: () => (wide() ? [0.25, 0.1, 0.76, 0.95] : [0.02, 0.12, 0.98, 0.97]),
  });
  s.orbit.minDistance = 60;
  s.orbit.maxDistance = 3200;
  s.orbit.minElevation = 3;
  s.orbit.maxElevation = 75;

  const views: Record<View, { target: [number, number, number]; distance: number; azimuth: number; elevation: number }> = {
    dam: small
      ? { target: [SECTION_X - 8, 12, -20], distance: 430, azimuth: 48, elevation: 21 }
      : { target: [SECTION_X - 22, 13, -40], distance: 540, azimuth: 44, elevation: 17 },
    pond: { target: [-140, 34, -520], distance: small ? 620 : 520, azimuth: 55, elevation: 9 },
    whole: { target: [-90, 20, -320], distance: small ? 2300 : 1750, azimuth: 30, elevation: 30 },
  };
  s.orbit.set(views.dam, 0);

  /* ---------- The world ---------- */
  const w = weather(POND);
  const world = new THREE.Group();
  s.scene.add(world);
  const ground = await makeGround({ small, pause, weather: w });
  world.add(ground.mesh);
  await pause();
  const dam = await makeEmbankment({ small, pause, weather: w, groundAt: ground.heightAt });
  world.add(dam.mesh, dam.rocks);
  await pause();
  const beach = await makeBeach({ small, pause, weather: w, spigots: OPEN_SPIGOTS });
  world.add(beach.mesh);
  await pause();
  const spigots = makeSpigotLine();
  world.add(spigots.group, makeChannels(OPEN_SPIGOTS, w));
  const prisms = makePrisms();
  const heads = makeHeads();
  world.add(prisms.group, heads.group);
  const decant = makeDecant({ material: wetRock(w, "causeway") });
  world.add(decant.group);
  world.add(makePipelines(ground.roads, (x, z) => Math.max(ground.heightAt(x, z), dam.top(x, z))));
  const seep = makeSeepage();
  world.add(seep.group);
  await pause();
  world.add(makeShrubs({ small, heightAt: ground.heightAt, roads: ground.roads }));
  if (ground.roads[2]) world.add(makePowerLine(ground.roads[2], ground.heightAt));
  const pad = makePad(ground.heightAt);
  world.add(pad.group);
  const tower = makeLightTower();
  tower.group.position.copy(pad.light);
  tower.group.rotation.y = -2.4;
  tower.set({ mast: 1, lights: false, yaw: 0.4 });
  world.add(tower.group);
  const parked = makePickup({ colour: "#e9e8e2" });
  parked.group.position.copy(pad.park);
  parked.group.rotation.y = 1.4;
  world.add(parked.group);
  // A dozer that has been trimming the crest road near the west abutment.
  const dozer = makeDozer();
  const dzX = -170;
  dozer.group.position.set(dzX, damTop(1.5) - settle(dzX), crestZ(dzX) + 1.5);
  dozer.group.rotation.y = Math.PI / 2 - Math.atan(2e-4 * dzX);
  dozer.set({ blade: 0.1 });
  world.add(dozer.group);
  await pause();
  const section = await makeSection({ small, storage: basis.storage, pause });
  world.add(section.group);
  const sky = makeSky(s, { small });
  const px = () => Math.min(small ? 1.5 : 1.75, devicePixelRatio);
  const pond = makePond({ small, weather: w, size: [s.view.clientWidth * px(), s.view.clientHeight * px()], hide: [sky.rain, section.group] });
  world.add(pond.mesh);
  new ResizeObserver(() => pond.resize(s.view.clientWidth * px(), s.view.clientHeight * px())).observe(s.view);
  await pause();

  /* ---------- A pickup on the crest, and the technician it will carry ---------- */
  const pickup = makePickup({ colour: "#e9e8e2" });
  const parkX = 140;
  const crestAt = (x: number, off: number) => damTop(off) - settle(x);
  const placePickup = (x: number) => {
    pickup.group.position.set(x, crestAt(x, 2.6), crestZ(x) + 2.6);
    pickup.group.rotation.y = -Math.PI / 2 - Math.atan(2e-4 * x);
  };
  placePickup(parkX);
  world.add(pickup.group);
  // The technician who will read VWP-04: built now, so his materials compile with the rest, and kept out of sight.
  const techModel: Miner = makeMiner({ role: "engineer", seed: 6, board: "tablet" });
  const techX = SECTION_X + 10.8;
  techModel.group.position.set(techX, crestAt(techX, 4.4), crestZ(techX) + 4.4);
  world.add(techModel.group);
  let tech: Miner | null = null;

  /* ---------- Labels ---------- */
  const chip = (ev: string) => `<span class="ev ${ev}">${ev}</span>`;
  const labels: Label[] = [];
  const lab = (html: string, where: THREE.Vector3 | THREE.Object3D, y: number, cls = "stem") => {
    const l = s.label(html, where, { y, className: cls });
    labels.push(l);
    return l;
  };
  // On a phone the cards below carry these; over the scene only the pond and the freeboard stay.
  if (!small) {
    lab(`Crest low point <b>RL ${fmt(RL(CREST_LOW), 2)}</b>${chip("measured")}<i class="tl-age">P-07 · ${DATES.crestSurvey}</i>`, prisms.at["P-07"]!, 9);
    lab(`Spigots <b>${OPEN_SPIGOTS.length} of 25 open</b>`, spigots.outlets[2] ?? new THREE.Vector3(), 9);
    lab(`Seepage at the weir <b>6.4 L/s</b>${chip("measured")}`, seep.weir, 7);
  }
  const pondLabel = lab("", new THREE.Vector3(-150, POND, -380), 6);
  const decantLabel = small ? null : lab("", decant.top, 8);
  const fbLabel = lab("", section.dimAt(POND), 0, "tl-dim");

  /* ---------- Piezometer callouts beside the section ---------- */
  const view = s.view;
  const calls = document.createElement("div");
  calls.className = "tl-calls";
  calls.setAttribute("aria-hidden", "true");
  const CALLED = small ? PIEZOMETERS.filter((p) => p.stale) : PIEZOMETERS;
  calls.innerHTML = `<svg class="tl-leads"></svg>${CALLED.map(
    (p) =>
      `<div class="tl-call ${p.stale ? "stale" : ""}" data-call="${p.id}"><i></i>${p.id} <b>${fmt(kPa(p))} kPa</b><span>${ageText(p.age)}</span></div>`,
  ).join("")}`;
  view.append(calls);
  const leads = $(calls, ".tl-leads") as unknown as SVGSVGElement;
  const callEls = CALLED.map((p) => $(calls, `[data-call="${p.id}"]`));
  let overlay = true;
  const placeCalls = () => {
    if (!overlay) {
      calls.hidden = true;
      return;
    }
    const pts = CALLED.map((p) => s.project(section.tips[p.id]!));
    // Only when the section is in view and near enough to read: from afar, or with a tip off screen, the cards
    // below carry the readings.
    const off = pts.some((q) => !q || q.x < 0 || q.x > s.width || q.y < 0 || q.y > s.height);
    const all = PIEZOMETERS.map((p) => s.project(section.tips[p.id]!)?.x ?? 0);
    if (off || Math.max(...all) - Math.min(...all) < 90) {
      calls.hidden = true;
      return;
    }
    calls.hidden = false;
    const W = s.width;
    const Hh = s.height;
    const xs = pts.map((q) => q!.x);
    const ys = pts.map((q) => q!.y);
    const area = wide() ? [0.25, 0.12, 0.76, 0.92] : [0.02, 0.14, 0.98, 0.96];
    const rowH = small ? 20 : 24;
    const colW = small ? 146 : 150;
    // A column clear of the tips, on whichever side has room.
    let cx = Math.max(...xs) + 36;
    let side = 1;
    if (cx + colW > area[2]! * W) {
      cx = Math.min(...xs) - 36 - colW;
      side = -1;
    }
    cx = clamp(cx, area[0]! * W, area[2]! * W - colW);
    const order = ys.map((y, i) => [y, i] as const).sort((a, b) => a[0] - b[0]);
    const mid = (Math.min(...ys) + Math.max(...ys)) / 2;
    let y0 = clamp(mid - (order.length * rowH) / 2, area[1]! * Hh, area[3]! * Hh - order.length * rowH);
    let path = "";
    for (const [, i] of order) {
      const el = callEls[i]!;
      const ty = y0 + rowH / 2;
      el.style.transform = `translate3d(${cx.toFixed(1)}px, ${(ty - rowH / 2 + 2).toFixed(1)}px, 0)`;
      const ex = side > 0 ? cx : cx + colW;
      const cls = CALLED[i]!.stale && !handRead ? "stale" : "";
      path += `<path d="M${ex.toFixed(1)} ${ty.toFixed(1)}H${(ex - side * 12).toFixed(1)}L${xs[i]!.toFixed(1)} ${ys[i]!.toFixed(1)}" class="${cls}"/><circle cx="${xs[i]!.toFixed(1)}" cy="${ys[i]!.toFixed(1)}" r="2.4" class="${cls}"/>`;
      y0 += rowH;
    }
    leads.innerHTML = path;
    calls.style.setProperty("--colw", `${colW}px`);
  };

  /* ---------- The storm, hour by hour ---------- */
  const q = <T extends Element = HTMLElement>(sel: string) => $<T>(host, sel);
  const out = {
    clock: q("[data-clock]"),
    mini: q("[data-mini]"),
    miniLow: q("[data-mini-low]"),
    low: q("[data-fb-low]"),
    tail: q("[data-fb-tail]"),
    beach: q("[data-beach]"),
    beachLow: q("[data-beach-low]"),
    warn: q("[data-warn]"),
    engTail: q("[data-eng-tail]"),
    sname: q("[data-sname]"),
    smm: q("[data-smm]"),
    bars: q<SVGGElement>("[data-bars]"),
    lvl: q<SVGPathElement>("[data-lvl]"),
    cur: q<SVGPathElement>("[data-cur]"),
    hour: q("[data-hour]"),
    peak: q("[data-peak]"),
    pumpsName: q("[data-pumps-name]"),
    play: q<HTMLButtonElement>("[data-play]"),
    gLive: host.querySelector<SVGRectElement>('[data-g="live"]')!,
    gLow: host.querySelector<SVGPathElement>('[data-g="low"]')!,
    gTail: host.querySelector<SVGPathElement>('[data-g="tail"]')!,
    gBr: host.querySelector<SVGPathElement>('[data-g="bracket"]')!,
    estText: q("[data-est-text]"),
    agentEst: q("[data-agent-est]"),
    agentVoi: q("[data-agent-voi]"),
  };
  const termEls = Object.fromEntries(
    [...host.querySelectorAll<HTMLElement>("[data-t]")].map((el) => [el.dataset.t!, el]),
  ) as Record<keyof Terms | "net", HTMLElement>;

  let mm = 0;
  let pumps = true;
  let run: Run = simulate(bal, 0, pumps);
  let hour = 0;
  let playing = false;
  let level = POND;
  let storm = 0;
  let rainVis = 0;
  let tailFb: number | null = null;
  let est: StaleEstimate | null = null;
  let handRead = false;
  const allow = CRITERIA.wave + CRITERIA.residual;

  /** The hyetograph and the pond's level for the run, drawn once per run. */
  const drawHyeto = () => {
    const maxI = Math.max(1, ...run.rain);
    let bars = "";
    const per = 4; // one bar an hour
    for (let i = 0; i < run.rain.length - 1; i += per) {
      let v = 0;
      for (let k = 0; k < per; k++) v += run.rain[i + k] ?? 0;
      v /= per;
      if (v <= 0) continue;
      const x = (run.hour[i]! / HOURS) * 240;
      const h = (v / maxI) * 40;
      bars += `<rect x="${x.toFixed(1)}" y="0" width="2" height="${h.toFixed(1)}"/>`;
    }
    out.bars.innerHTML = bars || `<text x="120" y="30" text-anchor="middle" class="h-hint">Pick a storm: its rain, hour by hour</text>`;
    const lo = POND;
    const hi = POND + Math.max(0.5, run.peak.rise * 1.1);
    let d = "";
    for (let i = 0; i < run.level.length; i += 4) {
      const x = (i / (run.level.length - 1)) * 240;
      const y = 62 - ((run.level[i]! - lo) / (hi - lo)) * 40;
      d += `${i ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`;
    }
    out.lvl.setAttribute("d", d);
  };

  const setRun = () => {
    run = simulate(bal, mm, pumps);
    drawHyeto();
    tailFb = null;
    scheduleEngine();
    if (mm > 0) {
      hour = 0;
      playing = true;
    } else {
      // No storm: the pond as it stands now.
      hour = 0;
      playing = false;
    }
    s.loop.poke();
  };

  const slider = range(
    host,
    "storm",
    (v) => (v <= 0 ? "no storm" : `${fmt(v)} mm · ${returnText(v).replace(" years", " yr")}`),
    (v) => {
      mm = v;
      setRun();
    },
  );
  out.play.addEventListener("click", () => {
    const to = mm > 0 && !playing ? 0 : mm > 0 ? mm : STORM100;
    slider.set(to);
    mm = to;
    setRun();
  });
  toggle(host, "pumps", (on) => {
    pumps = on;
    setRun();
  });
  seg(host, "view", (v) => {
    s.orbit.set(views[v as View], 2.4);
    s.loop.poke();
  });
  toggle(host, "overlay", (on) => {
    overlay = on;
    section.group.visible = on;
    fbLabel.show(on);
    s.loop.poke();
  });

  /* ---------- FrankenMining's kernels: the tail, the stale tip, the value of reading it ---------- */
  let engineTimer = 0;
  let engineSeq = 0;
  const engineError = (e: unknown) => `FrankenMining engine unavailable (${e instanceof Error ? e.message.slice(0, 60) : "error"}); no number shown in its place.`;
  const runVoi = async () => {
    if (!est) return;
    try {
      const v = await readingValue(est.pAboveL1, mm);
      if (handRead) return;
      out.agentVoi.innerHTML = `${mm > 0 ? "Before this storm" : "Today"}, reading it by hand is worth about <b>$${fmt(v.evsi, 0)}k</b> in expected loss avoided, for a $${v.cost}k visit${chip("estimated")}<small class="tl-engline">EVSI, illustrative payoffs · fmine-numerics in wasm · ${v.ms.toFixed(2)} ms</small>`;
    } catch (e) {
      out.agentVoi.textContent = engineError(e);
    }
  };
  const scheduleEngine = () => {
    clearTimeout(engineTimer);
    const seq = ++engineSeq;
    out.tail.textContent = mm > 0 ? "…" : "–";
    engineTimer = window.setTimeout(async () => {
      void runVoi();
      if (mm <= 0) {
        out.engTail.textContent = "CVaR of the freeboard: bring in a storm.";
        out.gTail.setAttribute("visibility", "hidden");
        return;
      }
      const n = small ? 300 : 600;
      const t0 = performance.now();
      const en = ensemble(bal, mm, pumps, n, 11);
      const tEns = performance.now() - t0;
      try {
        const t = await freeboardTail(en.rise, 0.95);
        if (seq !== engineSeq) return;
        tailFb = CREST_LOW - (POND + t.cvar);
        out.tail.textContent = m2(tailFb);
        out.tail.classList.toggle("below", tailFb < allow);
        out.engTail.innerHTML = `CVaR<sub>95</sub> of the peak rise, ${n} storms with depth, runoff, storage and pumps drawn at random: <b>+${fmt(t.cvar, 2)} m</b> · fmine-numerics in wasm ${t.ms.toFixed(2)} ms · runs ${fmt(tEns, 0)} ms`;
        out.gTail.setAttribute("d", `M16 ${gy(tailFb).toFixed(1)}H50`);
        out.gTail.setAttribute("visibility", "visible");
      } catch (e) {
        if (seq !== engineSeq) return;
        out.tail.textContent = "–";
        out.engTail.textContent = engineError(e);
      }
    }, 180);
  };
  void (async () => {
    try {
      est = await staleEstimate();
      // A time of zero means under one tick of the clock, a whole millisecond in Safari.
      out.estText.innerHTML = `Engine estimate from the five live tips: <b>${fmt(est.lo)}–${fmt(est.hi)} kPa</b> (90%), ${fmt(est.pAboveL1 * 100)}% above L1${chip("estimated")}<small class="tl-engline">fmine-calib Gaussian predictor in wasm · ${est.ms < 0.05 ? "under 1" : est.ms.toFixed(1)} ms · not a reading</small>`;
      out.agentEst.innerHTML = `VWP-04, in the foundation under the crest, last reported <b>19 days ago</b>. Its neighbours put it at ${fmt(est.lo)}–${fmt(est.hi)} kPa against an L1 of ${fmt(est.l1)} kPa. An estimate cannot replace a reading, so the TARP statement stays refused.`;
      void runVoi();
    } catch (e) {
      out.estText.textContent = engineError(e);
      out.agentVoi.textContent = "";
    }
  })();

  /* ---------- The field reading ---------- */
  const approve = q<HTMLButtonElement>("[data-approve]");
  const done = q("[data-done]");
  let drive: { x: number; to: number } | null = null;
  let readAt = 0;
  approve.addEventListener("click", () => {
    approve.disabled = true;
    q("[data-act]").hidden = true;
    done.hidden = false;
    done.textContent = "Approved by the tailings engineer. A technician is driving to BH-B to read VWP-04.";
    drive = { x: parkX, to: SECTION_X + 12 };
    s.loop.poke();
  });
  const finishReading = () => {
    handRead = true;
    const row = q(`[data-pz="${STALE.id}"]`);
    row.classList.remove("stale");
    row.classList.add("fresh");
    $(row, "[data-kpa]").textContent = `${HAND_KPA} kPa`;
    $(row, "[data-age]").innerHTML = `just now${chip("measured")}`;
    q("[data-est]").hidden = true;
    q("[data-refuse]").innerHTML = `“All six piezometers read below L1”${chip("measured")}<code>for the engineer of record</code>`;
    q("[data-refuse-why]").textContent = "Every tip now has a current reading, so FMINE-REF-404 no longer applies. What the readings mean for the dam is the engineer of record’s call.";
    done.textContent = `VWP-04 read by hand: ${HAND_KPA} kPa, below its L1 of ${fmt(headKPa(STALE, STALE.l1))} kPa. Sent with the storm forecast to the engineer of record.`;
    out.agentVoi.textContent = "";
    out.agentEst.innerHTML = est
      ? `VWP-04 had last reported <b>19 days ago</b>. The engine put it at ${fmt(est.lo)}–${fmt(est.hi)} kPa from its neighbours, an estimate; the hand reading of ${HAND_KPA} kPa now stands in its place.`
      : `VWP-04 had last reported <b>19 days ago</b>; the hand reading of ${HAND_KPA} kPa now stands in its place.`;
    const call = callEls[CALLED.indexOf(STALE)]!;
    call.classList.remove("stale");
    call.innerHTML = `<i></i>${STALE.id} <b>${HAND_KPA} kPa</b><span>now</span>`;
  };

  /* ---------- HUD ---------- */
  let lastHud = "";
  const hud = () => {
    const fb = CREST_LOW - level;
    const key = `${mm}|${pumps}|${hour.toFixed(1)}|${fb.toFixed(3)}|${tailFb}`;
    if (key === lastHud) return;
    lastHud = key;
    const name = returnText(mm);
    out.sname.textContent = name;
    out.smm.textContent = mm > 0 ? `${fmt(mm)} mm in 72 h · peak ${fmt(Math.max(...run.rain), 1)} mm/h` : "";
    out.pumpsName.textContent = pumps ? `Decant pumps, ${fmt(RATES.pumps, 2)} m³/s` : "Decant pumps, stopped";
    const h = Math.min(hour, HOURS);
    for (const k of Object.keys(run.cum) as (keyof Terms)[]) {
      const v = at(run, run.cum[k], h);
      const out_ = k === "pumped" || k === "evaporation" || k === "seepage";
      termEls[k].textContent = Math.abs(v) < 50 ? "0" : k3(out_ ? -v : v);
    }
    const stored = (Object.keys(run.cum) as (keyof Terms)[]).reduce((a, k) => {
      const v = at(run, run.cum[k], h);
      return a + (k === "pumped" || k === "evaporation" || k === "seepage" ? -v : v);
    }, 0);
    termEls.net.textContent = Math.abs(stored) < 50 ? "0" : k3(stored);
    out.hour.textContent = mm > 0 ? `hour ${Math.floor(h)} of ${HOURS}${h < STORM_HOURS ? ", raining" : ", rain over"}` : `no storm`;
    out.peak.textContent = mm > 0 ? `highest at hour ${fmt(run.peak.hour)}` : "";
    out.cur.setAttribute("d", `M${((h / HOURS) * 240).toFixed(1)} 0V64`);
    out.cur.setAttribute("visibility", mm > 0 ? "visible" : "hidden");
    // The freeboard now, counting down, and the lowest the storm takes it.
    out.mini.textContent = m2(fb);
    out.mini.classList.toggle("below", fb < allow);
    const low = run.peak.freeboard;
    out.low.textContent = mm > 0 ? m2(low) : "–";
    out.low.classList.toggle("below", mm > 0 && low < allow);
    out.miniLow.textContent = mm > 0 ? `lowest ${m2(low)}` : "";
    const bNow = edgeAt(level);
    out.beach.textContent = `${fmt(bNow)} m`;
    out.beach.classList.toggle("below", bNow < CRITERIA.beach);
    out.beachLow.textContent = mm > 0 ? ` (lowest ${fmt(run.peak.beach)} m)` : "";
    out.beachLow.classList.toggle("below", mm > 0 && run.peak.beach < CRITERIA.beach);
    const warns: string[] = [];
    if (mm > 0 && low < allow) warns.push(`The storm would take the freeboard ${m2(allow - low)} into the wave and residual allowance.`);
    if (mm > 0 && run.peak.beach < CRITERIA.beach) warns.push(`The water would come within ${fmt(run.peak.beach)} m of the crest, inside the ${CRITERIA.beach} m minimum beach: a TARP trigger.`);
    out.warn.hidden = warns.length === 0;
    out.warn.textContent = warns.join(" ");
    // Gauge: the pond now (live), the lowest mark, the bracket from the crest.
    out.gLive.setAttribute("y", gy(fb).toFixed(1));
    out.gLive.setAttribute("height", Math.max(0, gy(NOW_FB) - gy(fb)).toFixed(1));
    out.gLive.classList.toggle("below", fb < allow);
    out.gBr.setAttribute("d", `M12 8V${gy(fb).toFixed(1)}`);
    out.gLow.setAttribute("d", `M16 ${gy(low).toFixed(1)}H50`);
    out.gLow.setAttribute("visibility", mm > 0 ? "visible" : "hidden");
    out.clock.textContent =
      mm > 0
        ? `What-if: ${name.toLowerCase()}, ${fmt(mm)} mm in 72 h · hour ${Math.floor(h)} of ${HOURS} · freeboard ${m2(fb)}. A water-balance forecast.`
        : "";
    out.clock.classList.toggle("on", mm > 0);
    pondLabel.html(
      mm > 0
        ? `Pond${small ? "" : ", forecast"} <b>RL ${fmt(RL(level), 2)}</b>${chip("estimated")}${small ? "" : `<i class="tl-age">hour ${Math.floor(h)}</i>`}`
        : `Pond <b>RL ${fmt(RL(POND), 2)}</b>${chip("measured")}${small ? "" : `<i class="tl-age">${DATES.pondRead}</i>`}`,
    );
    decantLabel?.html(pumps ? `Decant pumps <b>${fmt(RATES.pumps, 2)} m³/s</b>${chip("measured")}` : `Decant pumps <b>stopped</b>${chip("assumed")}`);
    fbLabel.html(`Freeboard <b>${m2(fb)}</b>${chip(mm > 0 ? "estimated" : "derived")}`);
    out.play.querySelector("span")!.textContent = mm > 0 && !playing ? "Clear the sky" : mm > 0 ? "Storm running…" : "Bring in the storm";
  };

  /* ---------- Frame ---------- */
  const maxRainOf = () => Math.max(1e-6, ...run.rain);
  const severity = (v: number) => (v <= 0 ? 0 : 0.42 + 0.58 * Math.min(1, v / PMP) ** 0.75);
  let since = 1;
  s.frame((dt) => {
    w.time.value += dt;
    if (playing) {
      hour = Math.min(HOURS, hour + dt * (HOURS / 11));
      if (hour >= HOURS) playing = false;
      s.loop.poke();
    }
    // The pond from the run, eased only where the run restarts.
    const target = at(run, run.level, hour);
    level += (target - level) * (1 - Math.exp(-dt * 6));
    const rise = level - POND;
    w.level.value = level;
    pond.mesh.position.y = level;
    // Sky and rain from the hyetograph: cloud builds before the rain and lingers a little after it.
    const iNow = mm > 0 ? at(run, run.rain, hour) / maxRainOf() : 0;
    const during = mm > 0 && hour < STORM_HOURS + 2 ? 1 : mm > 0 ? Math.exp(-(hour - STORM_HOURS - 2) / 5) : 0;
    const want = severity(mm) * during * (0.62 + 0.38 * iNow);
    storm += (want - storm) * (1 - Math.exp(-dt * 2.2));
    const rainWant = mm > 0 && hour < STORM_HOURS ? (0.25 + 0.75 * iNow) * (0.45 + 0.55 * Math.min(1, mm / 300)) : 0;
    rainVis += (rainWant - rainVis) * (1 - Math.exp(-dt * 3));
    const soFar = at(run, run.rainSoFar, hour);
    const wetWant = mm > 0 ? Math.min(1, soFar / 25) * (hour < STORM_HOURS ? 1 : Math.exp(-(hour - STORM_HOURS) / 20)) : 0;
    w.wet.value += (wetWant - w.wet.value) * (1 - Math.exp(-dt * 2));
    w.storm.value = storm;
    sky.update(dt, storm, rainVis);
    const lit = storm > 0.5;
    if (lit !== tower.state.lights) tower.set({ lights: lit });
    pond.update(s.sun, sky.light);
    section.update(level, soFar * (rise > 0 ? 1 : 0), s.camera.position.distanceTo(section.dimAt(level)));
    fbLabel.anchor(section.dimAt(level));
    pickup.update(dt);
    if (drive) {
      const d = drive.to - drive.x;
      const v = Math.sign(d) * Math.min(12, Math.abs(d) * 0.6 + 0.5);
      drive.x += v * dt;
      pickup.set({ speed: Math.abs(v) });
      placePickup(drive.x);
      if (Math.abs(d) < 0.15) {
        drive = null;
        pickup.set({ speed: 0 });
        tech = techModel;
        tech.group.visible = true;
        const head = heads.at["BH-B"]!;
        tech.walkTo([new THREE.Vector3(head.x + 2.2, 0, head.z + 1.4)], 1.2);
        tech.work("tablet", head);
        readAt = 7;
      }
      s.loop.poke();
    }
    if (readAt > 0) {
      readAt -= dt;
      if (readAt <= 0) finishReading();
      s.loop.poke();
    }
    if (tech) {
      tech.update(dt);
      s.loop.poke();
    }
    since += dt;
    if (since > 0.1 || reduced) {
      since = 0;
      hud();
    }
    placeCalls();
  });
  drawHyeto();
  hud();
  void runVoi();
  // Compile every program before the first frame, in parallel where the driver allows (KHR_parallel_shader_compile),
  // so the first frame does not stall the page linking them one by one.
  // The pond's reflection renders the scene into a target, untoned and linear: those variants are compiled too.
  const prevTarget = s.renderer.getRenderTarget();
  s.renderer.setRenderTarget(pond.mesh.getRenderTarget());
  // One after the other: each wait checks the program a material holds last, so the second compile would hide the first.
  const warmReflection = s.renderer.compileAsync(s.scene, s.camera).catch(() => undefined);
  s.renderer.setRenderTarget(prevTarget);
  await warmReflection;
  await s.renderer.compileAsync(s.scene, s.camera).catch(() => undefined);
  // Some drivers still build their pipelines at the first draw. Bring the world in a few pieces a frame, so that
  // work, and the buffer uploads, come in small steps rather than one long stall.
  const parts = world.children.filter((c) => c.visible);
  for (const c of parts) c.visible = false;
  for (let i = 0; i < parts.length; i += 3) {
    for (const c of parts.slice(i, i + 3)) c.visible = true;
    s.render();
    await new Promise((r) => requestAnimationFrame(r));
  }
  techModel.group.visible = false;
  s.start();
  if (reduced) s.render();
  s.firstFrame(() => (fig.dataset.live = ""));
}
