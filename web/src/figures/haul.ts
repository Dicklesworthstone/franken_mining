/**
 * "Watch the shift; pick a truck to see its load": the load-and-haul cycle on the floor of the synthetic pit, close
 * up, late in the day shift. A rope shovel (S1) loads ore four passes to a truck from a 44 m³ dipper while trucks
 * queue, swing round and reverse in; a face shovel (L2) on a waste face waits for its one truck. Trucks run on
 * rimpull and retarding physics; their struts squat as each pass lands. Pick a truck and the camera follows it up the
 * ramp to the crusher while a card shows its load's chain of custody: face and block, queue, spotting, passes,
 * weighed payload, the block's grade from FrankenMining's engine (wasm, in the page), destination and times.
 *
 * The dispatch agent reads the queue and L2's idle time in the mine's own records, sizes the haulage lost (the
 * engine takes the tail of it) and drafts a dispatch change; the shift supervisor approves it, the truck turns off for
 * L2 and the queue thins. The engineering view draws the road network with its grades and speeds and breaks each
 * loader's cycle into its parts. Every number comes from the simulation or the engine running in the page.
 */
import * as THREE from "three";
import { makePickup } from "./fleet";
import { cycleParts, lossByMinute, payloadStats, queueTrace, size, type Sizing } from "./haul-agent";
import { makeDust } from "./haul-dust";
import { makeRoadOverlay } from "./haul-eng";
import { CUTOFF, gradeFaces, lossTail, type FaceGrades } from "./haul-grade";
import { makeApron, makeFloorRoads, makeSpill } from "./haul-ground";
import { bake, truckDetail } from "./haul-lod";
import { agentNote, approvedNote, clockAt, custody, engCard, markup, stateOf, type Draft, type Tail } from "./haul-ui";
import { $, clamp, fmt, reduced, seg, smooth } from "./lib";
import { makeMiner } from "./miners";
import { buildMineAsync, TRUCK, type Truck } from "./opencut";
import { stage, type Label } from "./stage";

/** The clock when the scene opens, and the change-over; what a truck and its operator cost an hour (assumed). */
const OPEN = 16 + 15 / 60;
const SHIFT_END = 18;
const COST = 610;
const deg = THREE.MathUtils.radToDeg;

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("haul");
  try {
    await run(host);
  } catch (e) {
    console.error("haul figure:", e);
    fail(host, e);
  }
}

/** Says plainly why there is no scene, instead of leaving an empty box. */
function fail(host: HTMLElement, e: unknown) {
  const webgl = (() => {
    try {
      const c = document.createElement("canvas");
      return !!(c.getContext("webgl2") ?? c.getContext("webgl"));
    } catch {
      return false;
    }
  })();
  host.innerHTML = `<div class="hl-fail"><p class="micro">Live 3D could not start</p><p><b>${
    webgl ? "The scene stopped while it was being built." : "This browser has WebGL switched off or unavailable."
  }</b></p><p>The scene is a working pit at the end of a day shift: a shovel loading trucks pass by pass, a queue at the shovel, and a dispatch agent that sizes the haulage the queue costs and drafts a change for the shift supervisor to approve. ${
    webgl ? `Reload the page to try again. (${String((e as Error)?.message ?? e).slice(0, 140).replace(/</g, "&lt;")})` : "It needs WebGL to draw."
  }</p></div>`;
}

async function run(host: HTMLElement) {
  host.innerHTML = markup();
  const small = matchMedia("(max-width: 699px)").matches;
  const nextFrame = () => new Promise<void>((r) => requestAnimationFrame(() => r()));
  // How long each step of the mount holds the main thread, ms (read with ?stage as __haul.phases).
  const phases: [string, number][] = [];
  let tPhase = performance.now();
  const phase = (name: string) => {
    const now = performance.now();
    phases.push([name, Math.round(now - tPhase)]);
    tPhase = now;
  };

  const s = stage($(host, ".hl-view"), {
    clock: OPEN,
    sunBearing: 208,
    fog: small ? 0.0006 : 0.00052,
    shadowRadius: 170,
    shadowAt: [-92, -150, -10],
    shadowMap: small ? 2048 : 4096,
    fov: small ? 48 : 34,
    near: 2.5,
    far: 6000,
    maxPixelRatio: small ? 1.6 : 1.75,
    exposure: 1.0,
    label: "The floor of an open pit late in the day shift: a shovel loads haul trucks while others queue, and trucks climb the ramp to the crusher",
    labelArea: () => [0.03, 0.05, 0.97, 0.95],
  });
  phase("stage");
  s.orbit.minDistance = 25;
  s.orbit.maxDistance = 1700;
  s.orbit.minElevation = 3;
  s.orbit.maxElevation = 80;

  /* ---------- The mine, run forward so its records hold the half hour before the scene opens ---------- */
  const mine = await buildMineAsync({
    cells: small ? 320 : 512,
    second: true,
    fleet: { S1: 12, L2: 1 },
    passes: 4,
    passTime: 28,
    dipper: 44,
    people: false,
    pickup: false,
    pace: 1,
  });
  s.scene.add(mine.group);
  phase("buildMine");
  const H = mine.pit.heightAt;
  const S1 = mine.loaders.find((l) => l.id === "S1")!;
  const L2 = mine.loaders.find((l) => l.id === "L2")!;
  await nextFrame();
  tPhase = performance.now();
  // The same steps as one run; a pause for the page after each, as nothing draws or moves the mine until s.start().
  for (let k = 0; k < 6; k++) {
    mine.advance(330);
    if (k % 2) phase("advance 11 min");
    await nextFrame();
    tPhase = performance.now();
  }
  // Open as a truck has just spotted under S1 with two waiting behind it.
  for (let k = 0; k < 240 && !(S1.queue.length >= 2 && S1.loading && S1.passesDone === 0); k++) {
    mine.advance(5);
    if (k % 40 === 39) await nextFrame();
  }
  const start = OPEN - mine.time / 3600;
  phase("advance to open");
  await nextFrame();
  tPhase = performance.now();

  /* ---------- Close-range ground: floor roads and spilled rock ---------- */
  mine.group.add(makeApron(S1.spot.clone().lerp(L2.spot, 0.4), small ? 140 : 170, H, small ? 3.2 : 2.5));
  mine.group.add(makeFloorRoads(mine.floorRoads, H));
  mine.group.add(makeSpill(S1.spot, H, { count: small ? 40 : 90, radius: 13, palette: "ore", seed: 3 }));
  mine.group.add(makeSpill(L2.spot, H, { count: small ? 30 : 60, radius: 11, palette: "waste", seed: 5 }));

  /* ---------- People: the shift supervisor at a pickup, a spotter at L2 ---------- */
  const u1 = new THREE.Vector3().subVectors(S1.spot, mine.shovel.group.position).setY(0).normalize();
  const f1 = new THREE.Vector3(u1.z, 0, -u1.x);
  const on = (v: THREE.Vector3) => v.setY(H(v.x, v.z));
  const pickup = makePickup();
  const pickAt = on(mine.shovel.group.position.clone().addScaledVector(u1, -44).addScaledVector(f1, -12));
  pickup.group.position.copy(pickAt);
  pickup.group.rotation.y = Math.atan2(mine.shovel.group.position.x - pickAt.x, mine.shovel.group.position.z - pickAt.z) + 0.5;
  mine.group.add(pickup.group);
  const boss = makeMiner({ role: "supervisor", seed: 6, detail: 2, board: "tablet" });
  const side = new THREE.Vector3(1, 0, 0).applyAxisAngle(new THREE.Vector3(0, 1, 0), pickup.group.rotation.y);
  boss.group.position.copy(on(pickAt.clone().addScaledVector(side, 1.9)));
  boss.face(mine.shovel.group.position.clone());
  boss.work("tablet");
  mine.group.add(boss.group);
  const uL = new THREE.Vector3().subVectors(L2.spot, L2.group.position).setY(0).normalize();
  const fL = new THREE.Vector3(uL.z, 0, -uL.x);
  const spotter = makeMiner({ role: "operator", seed: 9, detail: 2 });
  const spotAt = on(L2.spot.clone().addScaledVector(uL, 24).addScaledVector(fL, fL.dot(new THREE.Vector3().subVectors(L2.queueAt, L2.spot)) > 0 ? -16 : 16));
  spotter.group.position.copy(spotAt);
  spotter.face(L2.spot.clone());
  spotter.work("radio");
  mine.group.add(spotter.group);

  // Things that stay still from here on draw as one mesh per material.
  for (const tw of mine.towers) bake(tw);
  bake(pickup.group);
  const details = new Map(mine.trucks.map((t) => [t, truckDetail(t.model.group)]));

  /* ---------- Sunlight: what the sun reaches over the pit walls ---------- */
  const sunDir = new THREE.Vector3(0, 1, 0);
  const readSun = () => sunDir.subVectors(s.sun.position, s.sun.target.position).normalize();
  /** Whether the sun reaches a point `top` metres above the ground, marching towards it over the terrain. */
  const sunlit = (p: THREE.Vector3, top: number) => {
    if (sunDir.y <= 0.01 || s.night > 0.6) return false;
    const h = Math.hypot(sunDir.x, sunDir.z) || 1;
    const dx = sunDir.x / h;
    const dz = sunDir.z / h;
    const rise = sunDir.y / h;
    const y0 = p.y + top;
    for (let d = 6; d < 2600; d += 7) {
      const y = y0 + d * rise;
      if (y > 140) return true;
      if (H(p.x + dx * d, p.z + dz * d) > y) return false;
    }
    return true;
  };
  // Shadow casters: a thing in the pit wall's shadow throws none, so it skips the shadow pass.
  const casters: { root: THREE.Object3D; top: number; meshes: THREE.Mesh[]; moving: boolean; lit: boolean }[] = [];
  const caster = (root: THREE.Object3D, top: number, moving = false) => {
    const meshes: THREE.Mesh[] = [];
    root.traverse((m) => {
      if ((m as THREE.Mesh).isMesh && m.castShadow) meshes.push(m as THREE.Mesh);
    });
    casters.push({ root, top, meshes, moving, lit: true });
  };
  for (const t of mine.trucks) caster(t.model.group, 7, true);
  caster(mine.shovel.group, 20);
  caster(L2.group, 12);
  for (const tw of mine.towers) caster(tw, 9);
  caster(mine.crusher.group, 18);
  caster(mine.drill.group, 15);
  caster(pickup.group, 2);
  caster(boss.group, 2);
  caster(spotter.group, 2);
  const truckLit = new Map<Truck, boolean>();
  const where = new THREE.Vector3();
  const shade = (all: boolean) => {
    readSun();
    for (const c of casters) {
      if (!all && !c.moving) continue;
      const lit = sunlit(c.root.getWorldPosition(where), c.top);
      if (lit !== c.lit) {
        c.lit = lit;
        for (const m of c.meshes) m.castShadow = lit;
      }
    }
    for (const c of casters) if (c.moving) truckLit.set(mine.trucks.find((t) => t.model.group === c.root)!, c.lit);
  };

  /* ---------- Dust ---------- */
  const dust = makeDust(small ? 700 : 1500);
  s.scene.add(dust.mesh);
  const emit = new Map<Truck, number>();
  const wheel = new THREE.Vector3();
  const back = new THREE.Vector3();
  mine.on((e) => {
    const lit = truckLit.get(e.truck) ? 1 : 0;
    if (e.type === "pass") dust.puff(e.at.clone().add(new THREE.Vector3(0, -2.5, 0)), { n: small ? 5 : 9, size: 3, grow: 7, life: 5, alpha: 0.2, vel: new THREE.Vector3(0, -0.8, 0), spread: 1.1, scatter: 2, lit });
    if (e.type === "tip") dust.puff(e.at, { n: small ? 10 : 20, size: 5, grow: 15, life: 9, alpha: 0.18, vel: new THREE.Vector3(0, 0.6, 0), spread: 1.4, scatter: 4, lit });
  });

  /* ---------- Lights for the dusk ---------- */
  const lamps: { l: THREE.SpotLight; power: number }[] = [];
  const lamp = (from: THREE.Vector3, to: THREE.Vector3, power: number, angle = 0.7) => {
    const l = new THREE.SpotLight("#ffdcae", 0, 0, angle, 0.6, 1.6);
    l.position.copy(from);
    l.target.position.copy(to);
    s.scene.add(l, l.target);
    lamps.push({ l, power });
  };
  const [tw0, tw1, , , tw4] = mine.towers;
  if (tw0) lamp(tw0.position.clone().add(new THREE.Vector3(0, 10, 0)), S1.spot, 2600);
  if (tw1) lamp(tw1.position.clone().add(new THREE.Vector3(0, 10, 0)), S1.queueAt, 2200);
  if (tw4) lamp(tw4.position.clone().add(new THREE.Vector3(0, 10, 0)), L2.spot, 2200);
  lamp(mine.shovel.group.position.clone().add(new THREE.Vector3(0, 16, 0)).addScaledVector(u1, 4), S1.spot, 1500, 0.9);
  const head = new THREE.SpotLight("#fff1d6", 0, 140, 0.5, 0.5, 1.5);
  const headAim = new THREE.Object3D();
  head.position.set(0, 4.6, 7.2);
  headAim.position.set(0, 0, 40);
  let lightsOn = false;
  let lampK = 0;
  const dusk = () => {
    // Lamps come on as the wall's shadow takes the floor, and brighten into the dusk.
    const k = smooth(clamp((s.clock - 16.7) / 0.7, 0, 1));
    lampK = k;
    const want = k > 0.02;
    if (want !== lightsOn) {
      lightsOn = want;
      mine.lights(want);
      pickup.set({ lights: want });
    }
    for (const { l, power } of lamps) l.intensity = power * k;
    head.intensity = 900 * k;
  };
  s.onNight(dusk);

  /* ---------- Camera views ---------- */
  const rampEnd = mine.pit.ramp[mine.pit.ramp.length - 1]!;
  const azTo = (v: THREE.Vector3) => deg(Math.atan2(v.x, v.z));
  const bayAt = S1.spot.clone().lerp(S1.queueAt, 0.35).lerp(L2.spot, 0.22);
  const rampMid = mine.pit.ramp[Math.floor(mine.pit.ramp.length * 0.62)]!;
  const crusherAt = mine.crusher.group.position;
  const views = {
    bay: { target: [bayAt.x, bayAt.y + 5, bayAt.z] as [number, number, number], distance: small ? 180 : 205, azimuth: -18, elevation: small ? 25 : 14 },
    ramp: { target: [rampMid.x * 0.7, rampMid.y + 4, rampMid.z * 0.7] as [number, number, number], distance: small ? 820 : 600, azimuth: azTo(new THREE.Vector3(-rampMid.x, 0, -rampMid.z)) + 6, elevation: 20 },
    crusher: {
      target: [crusherAt.x, crusherAt.y + 6, crusherAt.z] as [number, number, number],
      distance: small ? 240 : 175,
      azimuth: azTo(new THREE.Vector3().subVectors(rampEnd, crusherAt).setY(0)) + 50,
      elevation: 20,
    },
  };
  type View = keyof typeof views;
  let viewNow: View = "bay";
  s.orbit.set(views.bay, 0);
  const view = seg(host, "view", (v) => {
    select(null, false);
    viewNow = v as View;
    s.orbit.set(views[viewNow], 1.6);
  });
  seg(host, "pace", (v) => {
    mine.pace = Number(v);
  });

  /* ---------- Labels ---------- */
  const lbl = {
    s1: s.label("", mine.shovel.group, { y: 25, className: "stem hl-l" }),
    l2: s.label("", L2.group, { y: 19, className: "stem hl-l" }),
    queue: s.label("", S1.queueAt, { y: 12, className: "stem hl-l hl-lq" }),
    sel: s.label("", mine.shovel.group, { y: 11, className: "stem hl-l hl-ls" }),
    boss: s.label("", boss.group, { y: 2.6, className: "stem hl-l hl-lp" }),
  };
  lbl.sel.show(false);
  const tags = new Map<Truck, Label>(mine.trucks.map((t) => [t, s.label(t.id, t.model.group, { y: 9.6, className: "hl-tag" })]));

  /* ---------- The engineering view ---------- */
  const shownRoads = new Set(["Ramp, up", "Ramp, down", "Road to the crusher", "Approach to S1", "Reversing in at S1", "Pulling out of S1", "Approach to L2"]);
  const overlay = makeRoadOverlay(mine.roads.filter((r) => r.kind !== "platform"), H, small ? 1.4 : 1.0);
  overlay.group.visible = false;
  mine.group.add(overlay.group);
  const roadLabels = overlay.anchors
    .filter((a) => shownRoads.has(a.road.name) && !(small && /Reversing|Pulling out of L2/.test(a.road.name)))
    .map((a) => {
      const r = a.road;
      const g = Math.abs(r.grade) < 0.005 ? "level" : `${r.grade >= 0 ? "+" : "−"}${fmt(Math.abs(r.grade) * 100, 1)}%`;
      const v = r.loaded ? `${fmt(r.vLoaded, 1)} km/h loaded · ${r.govLoaded}` : `${fmt(r.vEmpty, 0)} km/h empty · ${r.govEmpty}`;
      const l = s.label(small ? `<b>${g}</b> ${r.loaded ? fmt(r.vLoaded, 1) : fmt(r.vEmpty, 0)} km/h` : `<b>${g}</b> ${r.name} · ${v}`, a.at, { y: 3, className: "hl-rl" });
      l.show(false);
      return l;
    });
  const faceLabel = s.label("", S1.muck, { y: 14, className: "stem hl-l hl-lf" });
  faceLabel.show(false);
  // The draft dispatch change, drawn from the truck to L2's queue point.
  const arc = new THREE.Mesh(new THREE.BufferGeometry(), new THREE.MeshBasicMaterial({ color: "#8be27a", transparent: true, opacity: 0.8, depthWrite: false, toneMapped: false }));
  arc.renderOrder = 8;
  arc.frustumCulled = false;
  arc.visible = false;
  s.scene.add(arc);
  const arcLabel = s.label("", L2.queueAt, { y: 16, className: "stem hl-l hl-la" });
  arcLabel.show(false);
  let eng = false;
  const engBtn = $(host, "[data-eng-toggle]");
  const engCardEl = $(host, "[data-eng]");
  engBtn.addEventListener("click", () => {
    eng = !eng;
    engBtn.setAttribute("aria-pressed", String(eng));
    overlay.group.visible = eng;
    for (const l of roadLabels) l.show(eng);
    faceLabel.show(eng);
    engCardEl.hidden = !eng;
    card.hidden = eng && !sel;
    host.classList.toggle("hl-eng-on", eng);
    if (eng && !sel && viewNow === "bay") s.orbit.set({ elevation: small ? 34 : 30, distance: small ? 330 : 270 }, 1.4);
    paint(true);
    s.loop.poke();
  });

  /* ---------- Trucks to follow ---------- */
  const fleet = $(host, "[data-fleet]");
  fleet.innerHTML = mine.trucks.map((t) => `<button type="button" class="hl-chip" data-id="${t.id}" aria-pressed="false"><i data-state="${stateOf(t)}"></i>${t.id}</button>`).join("");
  const chips = new Map(mine.trucks.map((t) => [t, $(fleet, `[data-id="${t.id}"]`)]));
  let sel: Truck | null = null;
  let selSince = 0;
  const yawOf = (t: Truck) => {
    const f = new THREE.Vector3(0, 0, 1).applyQuaternion(t.model.group.quaternion);
    return deg(Math.atan2(f.x, f.z));
  };
  const select = (t: Truck | null, fly = true) => {
    if (sel === t) return;
    sel = t;
    for (const [u, b] of chips) b.setAttribute("aria-pressed", String(u === t));
    head.removeFromParent();
    headAim.removeFromParent();
    if (t) {
      view.set("");
      t.model.group.add(head, headAim);
      head.target = headAim;
      lbl.sel.anchor(t.model.group, { y: 11 });
      $(host, "[data-hint]").classList.add("off");
      if (fly) {
        const p = t.model.group.position;
        s.orbit.set({ target: [p.x, p.y + 4, p.z], distance: small ? 54 : 64, elevation: small ? 18 : 15, azimuth: yawOf(t) + 125 }, 1.4);
      }
      selSince = performance.now();
    } else if (fly) {
      view.set(viewNow);
      s.orbit.set(views[viewNow], 1.4);
    }
    lbl.sel.show(!!t);
    card.hidden = eng && !t;
    paint(true);
    s.loop.poke();
  };
  fleet.addEventListener("click", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-id]");
    const t = b && mine.trucks.find((u) => u.id === b.dataset.id);
    if (t) select(t === sel ? null : t);
  });
  host.addEventListener("click", (e) => {
    if ((e.target as Element).closest("[data-unfollow]")) select(null);
  });
  window.addEventListener("keydown", (e: KeyboardEvent) => {
    if (e.key === "Escape" && sel) select(null);
  });

  // A tap or a click on a truck follows it; a drag turns the view.
  const ray = new THREE.Raycaster();
  const ndc = new THREE.Vector2();
  const hitAt = (x: number, y: number) => {
    const r = s.canvas.getBoundingClientRect();
    ndc.set(((x - r.left) / r.width) * 2 - 1, -((y - r.top) / r.height) * 2 + 1);
    ray.setFromCamera(ndc, s.camera);
    return mine.pick(ray.ray);
  };
  let down: { x: number; y: number; t: number } | null = null;
  let touched = 0;
  s.canvas.addEventListener("pointerdown", (e) => {
    down = { x: e.clientX, y: e.clientY, t: performance.now() };
    touched = performance.now();
  });
  s.canvas.addEventListener("pointerup", (e) => {
    if (!down) return;
    const moved = Math.hypot(e.clientX - down.x, e.clientY - down.y);
    const quick = performance.now() - down.t < 600;
    down = null;
    if (moved > 7 || !quick) return;
    const t = hitAt(e.clientX, e.clientY);
    if (t) select(t);
  });
  let hoverAt = 0;
  s.canvas.addEventListener("pointermove", (e) => {
    if (e.pointerType !== "mouse" || down) return;
    const now = performance.now();
    if (now - hoverAt < 90) return;
    hoverAt = now;
    s.canvas.style.cursor = hitAt(e.clientX, e.clientY) ? "pointer" : "";
  });

  /* ---------- The engine: block grades, and the tail of the loss ---------- */
  let grades: FaceGrades | null = null;
  let tail: Tail | null = null;
  let tailAt = -1e9;
  const engineNote = (e: unknown) => console.warn("haul figure: engine", e);
  gradeFaces([S1, L2], H)
    .then((g) => {
      grades = g;
      paint(true);
    })
    .catch(engineNote);
  const refreshTail = (z: Sizing) => {
    if (mine.time - tailAt < 45) return;
    tailAt = mine.time;
    const xs = lossByMinute(mine, z, 30);
    if (xs.length < 5) return;
    lossTail(xs, 0.9)
      .then((r) => {
        tail = { mean: r.meanLoss, var: r.var, cvar: r.cvar, ms: r.ms, n: r.n };
      })
      .catch(engineNote);
  };

  /* ---------- The agent and the supervisor ---------- */
  const note = $(host, "[data-agent]");
  const card = $(host, "[data-load]");
  let approved: { truck: string; at: number; q0: number; l2Before: number } | null = null;
  let draft: Draft | null = null;
  let howOpen = false;
  note.addEventListener("toggle", (e) => {
    if ((e.target as Element).matches?.("details")) howOpen = (e.target as HTMLDetailsElement).open;
  }, true);
  const queuedFor = () => {
    // Whole minutes, newest first, in which a truck stood in S1's queue for more than a few seconds; the minute
    // under way counts when its part so far does.
    const rows = mine.minutes;
    let m = 0;
    for (let i = rows.length - 1; i >= 0; i--) {
      const r = rows[i]!;
      const part = Math.min(60, mine.time - r.t);
      if ((r.queue.S1 ?? 0) > Math.min(5, part * 0.1)) m++;
      else if (i < rows.length - 1) break;
    }
    return Math.max(0, m - 1);
  };
  const ordinal = (n: number) => ["first", "second", "third", "fourth", "fifth"][n] ?? `${n + 1}th`;
  const draftFor = (): Draft | null => {
    const q = S1.queue;
    if (q.length) {
      const t = q[q.length - 1]!;
      return { truck: t.id, place: q.length === 1 ? "waiting at S1" : `${ordinal(q.length - 1)} in the queue` };
    }
    const next = mine.trucks.find((t) => t.loader === "S1" && t.stretch === "in");
    return next ? { truck: next.id, place: "on its way in" } : null;
  };
  const l2Rate = (from: number, to: number) => {
    const rows = mine.minutes.filter((r) => r.t >= from && r.t < to);
    const secs = rows.reduce((a, r) => a + Math.min(60, mine.time - r.t), 0);
    return secs > 120 ? (rows.reduce((a, r) => a + (r.loaded.L2 ?? 0), 0) / secs) * 3600 : 0;
  };
  note.addEventListener("click", (e) => {
    if (!(e.target as Element).closest("[data-approve]") || approved || !draft) return;
    approved = { truck: draft.truck, at: mine.time, q0: S1.queue.length, l2Before: l2Rate(mine.time - 1800, mine.time) };
    mine.assign(draft.truck, "L2");
    if (reduced) mine.advance(150);
    paint(true);
    s.loop.poke();
  });

  /* ---------- Panels ---------- */
  const out = {
    clock: $(host, "[data-clock]"),
    left: $(host, "[data-left]"),
    ore: $(host, '[data-t="ore"]'),
    waste: $(host, '[data-t="waste"]'),
    loads: $(host, '[data-t="loads"]'),
    queue: $(host, '[data-t="queue"]'),
    spark: $(host, "[data-spark]"),
  };
  const setHtml = (el: Element, html: string) => {
    if ((el as HTMLElement).dataset.html !== html) {
      el.innerHTML = html;
      (el as HTMLElement).dataset.html = html;
    }
  };
  const spark = () => {
    const tr = queueTrace(mine, "S1", 30);
    const n = tr.length;
    if (n < 2) return "";
    const top = Math.max(3, ...tr);
    const w = 220 / n;
    const Y = (v: number) => 38 - (v / top) * 34;
    const first = mine.minutes[mine.minutes.length - n]!.t;
    const mark = approved ? clamp((Math.floor(approved.at / 60) * 60 - first) / 60, 0, n - 1) : -1;
    const bars = tr
      .map((v, i) => `<rect class="${mark >= 0 && i >= mark ? "after" : ""}" x="${(i * w + 0.6).toFixed(1)}" y="${Y(v).toFixed(1)}" width="${(w - 1.2).toFixed(1)}" height="${(38 - Y(v)).toFixed(1)}"/>`)
      .join("");
    const one = Y(1).toFixed(1);
    return `${bars}<path class="one" d="M0 ${one}H220"/><text x="2" y="${Number(one) - 2}">1 truck</text>${
      mark >= 0 ? `<path class="mark" d="M${(mark * w).toFixed(1)} 0V38"/><text class="mk" x="${(mark * w - 3).toFixed(1)}" y="8" text-anchor="end">approved</text>` : ""
    }`;
  };
  const paint = (force = false) => {
    const now = mine.time;
    const clockNow = start + now / 3600;
    out.clock.textContent = clockAt(start, now);
    const left = (SHIFT_END - clockNow) * 3600;
    out.left.textContent = left > 0 ? `${Math.floor(left / 3600) ? `${Math.floor(left / 3600)} h ` : ""}${Math.floor((left % 3600) / 60)} min to the change-over` : "night shift";
    const z = size(mine);
    refreshTail(z);
    const recent = mine.ledger.filter((c) => (c.tTip ?? 0) >= now - 1800);
    out.ore.textContent = `${fmt(Math.round(z.ore / 10) * 10)} t/h`;
    out.waste.textContent = `${fmt(Math.round(z.waste / 10) * 10)} t/h`;
    out.loads.textContent = fmt(recent.length);
    out.queue.textContent = `${S1.queue.length} now`;
    setHtml(out.spark, spark());
    // The custody card: the truck followed, or the one under S1.
    const watched = sel ?? (S1.loading as Truck | null) ?? S1.queue[0] ?? null;
    if (watched) {
      const c = watched.load ?? watched.last;
      const X = mine.loaders.find((l) => l.id === (c?.loader ?? watched.loader)) ?? S1;
      setHtml(
        card,
        custody(watched, start, now, mine.eta(watched.id), X, {
          watching: !sel,
          pending: watched.pending,
          est: c ? grades?.blocks.get(c.block) ?? null : null,
          rated: TRUCK.ratedT,
          cutoff: CUTOFF,
          engineMs: grades?.ms,
        }),
      );
    }
    // The agent.
    const end = clockAt(0, SHIFT_END * 3600, false);
    if (!approved) {
      draft = draftFor();
      setHtml(note, agentNote(z, { queuedFor: queuedFor(), left: Math.max(0, left), draft, at: clockAt(start, now, false), cost: COST, end, tail, open: howOpen }));
    } else {
      const rows = mine.minutes.filter((r) => r.t >= approved!.at);
      const spanS = rows.reduce((a, r) => a + Math.min(60, now - r.t), 0);
      const qAvg = spanS > 90 ? rows.reduce((a, r) => a + (r.queue.S1 ?? 0), 0) / spanS : null;
      const l2 = rows.reduce((a, r) => a + (r.loaded.L2 ?? 0), 0);
      const after = l2Rate(approved.at, now + 1);
      const gain = now - approved.at > 900 ? after - approved.l2Before : null;
      setHtml(note, approvedNote({ truck: approved.truck, at: clockAt(start, approved.at, false), q0: approved.q0, qNow: S1.queue.length, qAvg, l2, mins: Math.round((now - approved.at) / 60), gain }));
    }
    // The engineering card and the draft drawn in the scene.
    if (eng) {
      const up = mine.roads.find((r) => r.name === "Ramp, up") ?? null;
      const dn = mine.roads.find((r) => r.name === "Ramp, down") ?? null;
      const th = Math.atan(Math.abs(dn?.grade ?? 0.09));
      const downForce = TRUCK.emptyT * 1000 * 9.81 * (Math.sin(th) - TRUCK.rr * Math.cos(th));
      setHtml(
        engCardEl,
        engCard(cycleParts(mine, "S1"), cycleParts(mine, "L2"), {
          emptyT: TRUCK.emptyT,
          ratedT: TRUCK.ratedT,
          wheelKW: TRUCK.wheelKW,
          retardKW: TRUCK.retardKW,
          rr: TRUCK.rr,
          ramp: up && dn ? { grade: up.grade, steepest: up.steepest, vLoaded: up.vLoaded, govLoaded: up.govLoaded, vEmpty: dn.vEmpty, govEmpty: dn.govEmpty, retardEmpty: downForce > 0 ? ((TRUCK.retardKW * 1000) / downForce) * 3.6 : 0 } : null,
          dipper: S1.bucket ? { m3: S1.bucket.m3, density: S1.bucket.density, fill: S1.bucket.fill, passT: S1.passT, plan: S1.plan ?? S1.passes } : null,
          bucket2: L2.bucket ? { m3: L2.bucket.m3, passT: L2.passT, plan: L2.plan ?? L2.passes } : null,
          payload: payloadStats(mine, TRUCK.ratedT),
          grades: grades ? { ms: grades.ms, queries: grades.queries, samples: grades.samples } : null,
        }),
      );
      const blk = S1.loading?.load?.block ?? mine.ledger.filter((c) => c.loader === "S1").at(-1)?.block;
      const est = blk ? grades?.blocks.get(blk) : undefined;
      faceLabel.html(est ? (small ? `<b>N-07</b> ${fmt(est.mean, 2)} g/t` : `<b>N-07</b> ore · block ${blk}: ${fmt(est.mean, 2)} g/t, P10–P90 ${fmt(est.p10, 2)}–${fmt(est.p90, 2)}`) : `<b>N-07</b> ore face`);
    }
    const drafted = !approved && draft ? mine.trucks.find((t) => t.id === draft!.truck) : undefined;
    arc.visible = eng && !!drafted;
    arcLabel.show(eng && !!drafted);
    if (eng && drafted) {
      const a = drafted.model.group.position.clone().add(new THREE.Vector3(0, 9, 0));
      const b = L2.queueAt.clone().add(new THREE.Vector3(0, 9, 0));
      const mid = a.clone().lerp(b, 0.5).add(new THREE.Vector3(0, 26, 0));
      arc.geometry.dispose();
      arc.geometry = new THREE.TubeGeometry(new THREE.QuadraticBezierCurve3(a, mid, b), 48, small ? 0.9 : 0.6, 6, false);
      arcLabel.html(small ? `Draft <b>+${fmt(Math.round(z.lost / 10) * 10)} t/h</b>` : `Draft: ${drafted.id} to L2 · <b>+${fmt(Math.round(z.lost / 10) * 10)} t/h</b>`);
    }
    // Labels in the scene.
    const sl = S1.loading;
    lbl.s1.html(
      small
        ? `<b>S1</b> ${sl ? `${sl.id} · pass ${Math.min(S1.passes, S1.passesDone + 1)}/${S1.passes}` : "spotting"}`
        : `<b>S1</b> rope shovel · ${sl ? `loading ${sl.id}, pass ${Math.min(S1.passes, S1.passesDone + 1)} of ${S1.passes}` : S1.waiting > 20 ? `waiting ${Math.round(S1.waiting / 60)} min` : "spotting a truck"}`,
    );
    const ll = L2.loading;
    lbl.l2.html(
      small
        ? `<b>L2</b> ${ll ? `${ll.id} · pass ${Math.min(L2.passes, L2.passesDone + 1)}/${L2.passes}` : L2.waiting > 30 ? `idle ${Math.max(1, Math.round(L2.waiting / 60))} min` : "spotting"}`
        : `<b>L2</b> face shovel · ${ll ? `loading ${ll.id}, pass ${Math.min(L2.passes, L2.passesDone + 1)} of ${L2.passes}` : L2.waiting > 30 ? `waiting for a truck, ${Math.max(1, Math.round(L2.waiting / 60))} min` : "spotting a truck"}`,
    );
    const q = S1.queue;
    lbl.queue.show(q.length > 0);
    if (q.length) {
      lbl.queue.anchor(q[q.length - 1]!.model.group, { y: 10 });
      const longest = Math.max(...q.map((t) => t.waited));
      lbl.queue.html(small ? `Queue <b>${q.length}</b> · ${Math.max(1, Math.round(longest / 60))} min` : `Queue at S1: <b>${q.length} ${q.length === 1 ? "truck" : "trucks"}</b>, longest ${Math.max(1, Math.round(longest / 60))} min`);
    }
    if (sel) lbl.sel.html(`<b>${sel.id}</b> ${sel.payloadT > 0 ? `${fmt(sel.payloadT)} t` : "empty"} · ${fmt(Math.abs(sel.speed) * 3.6)} km/h`);
    lbl.boss.show(!small || !!approved);
    lbl.boss.html(approved ? `Shift supervisor · <b>approved</b>` : "Shift supervisor");
    // Id tags on the nearer trucks, but not on the ones another label already names.
    const named = new Set<Truck>([...(sel ? [sel] : []), ...q, ...(sl ? [sl as Truck] : [])]);
    const cam = s.camera.position;
    let shown = 0;
    for (const [t, tag] of tags) {
      const d = t.model.group.position.distanceTo(cam);
      const showIt = !named.has(t) && d < (small ? 240 : 320) && shown < (small ? 4 : 7);
      if (showIt) shown++;
      tag.show(showIt);
      tag.el.dataset.state = stateOf(t);
    }
    for (const [t, b] of chips) b.querySelector("i")!.dataset.state = stateOf(t);
    if (force && reduced) s.render();
  };

  /* ---------- Frame ---------- */
  let hud = 0;
  let shadeT = 0;
  let shadowAt = new THREE.Vector3(...views.bay.target);
  let shadowR = 170;
  let idle = 0;
  let lastClock = mine.time;
  s.frame((dt) => {
    if (!reduced) mine.update(dt);
    const sim = dt * mine.pace;
    // The sky follows the shift clock; things the wall's shadow reaches stop casting their own.
    if (Math.abs(mine.time - lastClock) > 20) {
      lastClock = mine.time;
      s.setClock(start + mine.time / 3600);
      shade(true);
    }
    shadeT += dt;
    if (shadeT > 0.5) {
      shadeT = 0;
      shade(false);
      // Small parts of far trucks would not cover a pixel.
      const far = (small ? 230 : 300) * (34 / s.camera.fov);
      for (const [t, d] of details) d.set(t !== sel && t.model.group.position.distanceTo(s.camera.position) > far);
    }
    // Trails behind moving trucks: more dust the faster they go, lit where the sun reaches them.
    for (const t of mine.trucks) {
      const v = Math.abs(t.speed);
      if (v < 0.8 || reduced) continue;
      let acc = (emit.get(t) ?? 0) + sim * v * (small ? 0.9 : 1.6);
      const g = t.model.group;
      const lit = truckLit.get(t) ? 1 : 0.15;
      back.set(0, 0, -Math.sign(t.speed)).applyQuaternion(g.quaternion);
      while (acc >= 1) {
        acc -= 1;
        const sideX = Math.random() < 0.5 ? -3.9 : 3.9;
        g.localToWorld(wheel.set(sideX, 0.7, t.speed > 0 ? -4.4 : 4.4));
        dust.puff(wheel, { size: 2.4, grow: v > 6 ? 11 : 7, life: v > 6 ? 8 : 6, alpha: v > 6 ? 0.11 : 0.085, vel: back.clone().multiplyScalar(v * 0.18).setY(0.7), spread: 0.5, scatter: 0.8, lit });
      }
      emit.set(t, acc);
    }
    dust.update(reduced ? 0 : sim);
    dust.light(s.sun.color, s.sun.intensity * (1 - s.night), s.hemi.color, s.hemi.intensity, s.hemi.groundColor, sunDir, lampK);
    // The followed truck: the camera keeps it in view, slowly turning round it while nobody drags.
    if (sel) {
      const p = sel.model.group.position;
      const settling = performance.now() - selSince < 1500;
      s.orbit.set({ target: [p.x, p.y + 4, p.z] }, settling ? 1.2 : 0.3);
    }
    idle = performance.now() - touched > 5000 ? idle + dt : 0;
    if (idle > 0 && !reduced) s.orbit.azimuth = s.orbit.azimuth + dt * (sel ? 2.2 : 0.6) * Math.min(1, idle / 3);
    // Keep the sun's shadow box on what the camera looks at, sized to it.
    const tgt = s.orbit.target;
    const want = clamp(s.orbit.distance * 0.75, 70, 320);
    if (tgt.distanceTo(shadowAt) > want * 0.15 || Math.abs(want - shadowR) > want * 0.25) {
      shadowAt = tgt.clone();
      shadowR = want;
      s.shadowBox(shadowAt, shadowR);
    }
    hud += dt;
    if (hud > 0.25) {
      hud = 0;
      paint();
    }
  });
  s.setClock(start + mine.time / 3600);
  shade(true);
  dusk();
  paint(true);
  phase("scene, labels, overlay");
  s.start();
  if (reduced) s.render();
  if (location.search.includes("stage")) (window as unknown as { __haul?: unknown }).__haul = { mine, s, select, dust, views, phases };
}
