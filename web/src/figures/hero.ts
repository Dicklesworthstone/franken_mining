/**
 * The opening scene: a working open pit at the end of the afternoon. Labels over the machines show the claims
 * FrankenMining keeps about each one, and the console switches whose desk they are read from: the supervisor's,
 * the geologist's, the metallurgist's, the CFO's or an AI agent's. The mine and every number on it are synthetic.
 */
import * as THREE from "three";
import { $, breathe, clamp, fmt, range, reduced, seg } from "./lib";
import { twinView, type Layer } from "./hero-twin";
import { bake, truckDetail } from "./haul-lod";
import { buildMineAsync, type Truck } from "./opencut";
import { stage, type Label } from "./stage";

type Lens = "operator" | "geology" | "plant" | "cfo" | "agent";
type Ev = "measured" | "derived" | "estimated" | "assumed" | "stale" | "refused";
/** What a label says: its words, its value and what the value rests on. Empty hides it. */
type Say = [text: string, value: string, ev: Ev] | null;

const pill = (s: Say) => (s ? `<span class="t">${s[0]}</span> <b>${s[1]}</b><span class="ev ${s[2]}">${s[2]}</span>` : "");
/** On a phone the scene keeps its four most telling labels for each desk, in this order. */
const PHONE_ORDER = ["twin", "shovel", "truck", "wall", "crusher", "drill"] as const;
const clockText = (h: number) => `${String(Math.floor(h)).padStart(2, "0")}:${String(Math.round((h % 1) * 60)).padStart(2, "0")}`;

export default async function mount(fig: HTMLElement) {
  const view = $(fig, ".lineup");
  const wide = () => matchMedia("(min-width: 1000px)").matches;
  const small = !wide();
  /** The bottom of the buttons over the scene, as a fraction of the scene's height. */
  const ctaLine = () => {
    const v = view.getBoundingClientRect();
    const cta = fig.querySelector(".cta")?.getBoundingClientRect();
    return cta && v.height ? clamp((cta.bottom - v.top) / v.height, 0.05, 0.6) : 0.05;
  };
  const s = stage(view, {
    clock: 17.25,
    fog: 0.00042,
    shadowRadius: 620,
    shadowAt: [0, -60, 80],
    fov: small ? 42 : 36,
    drift: 0.9,
    maxPixelRatio: small ? 1.5 : 1.6,
    sunBearing: -35,
    label: "A live 3D open-pit mine",
    shift: () => (wide() ? [0.21, 0] : [0, -0.02]),
    labelArea: () => (wide() ? [0.47, 0.12, 0.88, 0.76] : [0.03, ctaLine() + 0.03, 0.97, 0.62]),
    // On a phone the scene runs on under the headline: no label reaches over the buttons.
    labelCeiling: () => (wide() ? -Infinity : ctaLine() + 0.01),
  });
  s.orbit.minDistance = 260;
  s.orbit.maxDistance = 2200;
  s.orbit.minElevation = 12;
  s.orbit.maxElevation = 70;

  // Built a piece at a time, so the page can paint the headline and answer a tap while the mine goes up.
  await breathe();
  const mine = await buildMineAsync({ cells: small ? 288 : 480, trucks: small ? 5 : 7, people: true, pace: 2.4 });
  s.scene.add(mine.group);
  // The light towers stay still from here on: one mesh per material each (the same materials, so the lamps still
  // switch at dusk). A far truck's small parts (hoist rams, hoses, grates, chrome) would not cover a pixel.
  for (const tw of mine.towers) bake(tw);
  const details = new Map(mine.trucks.map((t) => [t, truckDetail(t.model.group)]));
  let detailT = 1;
  // The drill works a bench on the far wall, where the camera sees it, with the blaster beside it.
  const drillAt = mine.pit.berm(1.3, 30) ?? mine.pit.berm(1.3, 45);
  if (drillAt) {
    mine.drill.group.position.copy(drillAt);
    mine.drill.group.rotation.y = -1.3;
    const blaster = mine.people.find((m) => m.role === "blaster");
    if (blaster) {
      const inward = new THREE.Vector3(-Math.cos(1.3), 0, -Math.sin(1.3));
      const side = new THREE.Vector3(-inward.z, 0, inward.x);
      const at = drillAt.clone().addScaledVector(side, 9).addScaledVector(inward, 3);
      at.y = mine.pit.heightAt(at.x, at.z);
      blaster.group.position.copy(at);
      blaster.face(drillAt.clone());
    }
  }
  // The twin: drillholes, block model and pit limit, computed by the engine in a worker once the scene is up.
  const twin = twinView(mine.pit, async (ground) => (await import("./hero-solve")).solveTwin(ground));
  s.scene.add(twin.group);

  // The whole pit in view beside the headline, seen from the south-west with the low sun across it.
  s.orbit.set({ target: [-30, -90, 10], distance: small ? 1500 : 1350, azimuth: 212, elevation: 34 });

  /* ---------- Labels ---------- */
  const tally = mine.tally;
  const followed = () =>
    mine.trucks.find((t) => t.doing.startsWith("Climbing the ramp, loaded")) ??
    mine.trucks.find((t) => t.payloadT > 0) ??
    mine.trucks[0]!;
  let truck: Truck = followed();
  const ozToPlant = () => (tally.ore * 1.42) / 31.1035;
  const wall = new THREE.Vector3(230, -60, 170);
  wall.y = mine.pit.heightAt(wall.x, wall.z) + 6;
  const anchors = {
    shovel: [mine.shovel.group, 24],
    truck: [truck.model.group, 11],
    crusher: [mine.crusher.group, 26],
    drill: [mine.drill.group, 20],
    wall: [wall, 0],
    twin: [twin.heart, 0],
  } as const;
  type Key = keyof typeof anchors;
  /** The engine's grade for the block the shovel is digging (the one under its feet). */
  const face = () => {
    const p = mine.shovel.group.position;
    return twin.at(p.x, p.y - 7.5, p.z);
  };
  const n1 = (v: number) => v.toFixed(1);
  /** The twin's label: what the engine has re-solved, read from each desk. */
  const twinSays = (l: Lens): Say => {
    const t = twin.twin;
    if (twin.state === "idle") return null;
    if (twin.state === "solving") return ["Re-solving the pit from today’s survey", "estimating blocks…", "estimated"];
    if (!t) return null;
    switch (l) {
      case "operator":
        return ["Pit limit", `${t.benches} benches below the floor`, "estimated"];
      case "geology":
        return [`Block model from ${fmt(t.samples)} assays`, `${fmt(t.queries)} blocks in ${n1(t.estimateMs / 1000)} s`, "estimated"];
      case "plant":
        return [`Ore left in the pit above ${t.cutoff.toFixed(2)} g/t`, `${n1(t.oreT / 1e6)} Mt at ${t.oreGrade.toFixed(2)} g/t`, "estimated"];
      case "cfo":
        return [`Pit value at $${fmt(t.price)}/oz (not a reserve)`, `$${fmt(t.value / 1e6)}M`, "estimated"];
      case "agent":
        // A model output, however well checked, is never "measured" (INV-006).
        return t.verified
          ? ["Pit re-solved, certificate re-checked", `verified · ${fmt(t.solveMs)} ms`, "estimated"]
          : ["Pit re-solved, certificate re-checked", "check failed", "refused"];
    }
  };
  /** How strongly each desk shows the drillholes, the block model and the pit limit. */
  const layers: Record<Lens, Partial<Record<Layer, number>>> = {
    operator: { holes: 0, blocks: 0, limit: 0.6 },
    geology: { holes: 1, blocks: 0.9, limit: 0.25 },
    plant: { holes: 0, blocks: 0.85, limit: 0 },
    cfo: { holes: 0, blocks: 0.55, limit: 1 },
    agent: { holes: 0.6, blocks: 0.35, limit: 0.9 },
  };
  const lenses: Record<Lens, (k: Key) => Say> = {
    operator: (k) =>
      ({
        shovel: ["Shovel S1 queue", `${tally.queue} ${tally.queue === 1 ? "truck" : "trucks"}`, "measured"] as Say,
        truck: [`${truck.id} · ${truck.doing.toLowerCase()}`, `${fmt(truck.payloadT)} t`, "measured"] as Say,
        crusher: ["Crusher feed", "4,180 t/h", "measured"] as Say,
        drill: ["Drill D2, pattern 240-14", "hole 14 of 60", "measured"] as Say,
        wall: ["Waste moved this shift", `${fmt(18_400 + tally.waste)} t`, "derived"] as Say,
        twin: twinSays("operator"),
      })[k],
    geology: (k) =>
      ({
        shovel: (() => {
          const f = face();
          return (f ? ["Face N-07 grade, from the block model", `${f.g.toFixed(2)} g/t ± ${Math.round(Math.sqrt(Math.expm1(f.sd * f.sd)) * 100)}%`, "estimated"] : ["Face N-07 grade", "1.42 g/t ± 9%", "estimated"]) as Say;
        })(),
        truck: [`${truck.id} carries`, `${fmt((truck.payloadT * (face()?.g ?? 1.42)) / 1000, 2)} kg Au`, "estimated"] as Say,
        drill: ["Blasthole assays back", "12 of 60", "measured"] as Say,
        wall: ["South wall factor of safety", "1.34", "estimated"] as Say,
        crusher: ["Stockpile SP-2 grade, sampled 31 days ago", "0.68 g/t", "stale"] as Say,
        twin: twinSays("geology"),
      })[k],
    plant: (k) =>
      ({
        crusher: ["Feed hardness", "BWi 14.8 kWh/t", "estimated"] as Say,
        shovel: ["Ore type at the face", "transitional", "estimated"] as Say,
        truck: ["Recovery forecast for this load", "88.1%", "estimated"] as Say,
        drill: null,
        wall: null,
        twin: twinSays("plant"),
      })[k],
    cfo: (k) =>
      ({
        wall: twin.twin
          ? (["Gold left in the re-solved pit (not a reserve)", `${fmt(twin.twin.oz / 1000)} koz`, "estimated"] as Say)
          : (["Gold in the pit model (not a reserve)", "1.21 Moz", "estimated"] as Say),
        truck: ["Cost per tonne moved", "$2.84", "estimated"] as Say,
        crusher: ["Gold to the plant today", `${fmt(1_640 + ozToPlant())} oz`, "estimated"] as Say,
        drill: ["Drill and blast cost", "$0.38/t", "estimated"] as Say,
        shovel: null,
        twin: twinSays("cfo"),
      })[k],
    agent: (k) =>
      ({
        shovel: ["Queue at S1 costs 410 t/h", "dispatch change drafted", "estimated"] as Say,
        drill: ["Grade model asks for assays not yet back", "FMINE-REF-108", "refused"] as Say,
        crusher: ["Hardness forecast frozen at 06:00", "scored at 18:00", "estimated"] as Say,
        wall: ["Wall monitor", "no alarm", "measured"] as Say,
        truck: null,
        twin: twinSays("agent"),
      })[k],
  };
  const labels = new Map<Key, Label>();
  for (const [k, [obj, y]] of Object.entries(anchors) as [Key, (typeof anchors)[Key]][]) {
    labels.set(k, s.label("", obj as THREE.Object3D | THREE.Vector3, { y, className: "stem" }));
  }
  let lens: Lens = "operator";
  const paint = () => {
    const now = followed();
    if (now !== truck) {
      truck = now;
      labels.get("truck")!.anchor(truck.model.group, { y: 11 });
    }
    const keep = new Set<Key>();
    if (!wide()) for (const k of PHONE_ORDER) if (keep.size < 4 && lenses[lens](k)) keep.add(k);
    for (const [k, l] of labels) {
      const say = lenses[lens](k);
      const on = !!say && (wide() || keep.has(k));
      l.show(on);
      if (say && on) l.html(pill(say));
    }
  };
  seg(fig, "lens", (v) => {
    lens = v as Lens;
    twin.show(layers[lens]);
    paint();
    s.loop.poke();
  });

  /* ---------- The console ---------- */
  const out = {
    loads: $(fig, '[data-n="loads"]'),
    ore: $(fig, '[data-n="ore"]'),
    waste: $(fig, '[data-n="waste"]'),
    open: $(fig, '[data-n="open"]'),
  };
  const bars = {
    ore: $(fig, '[data-bar="ore"]'),
    waste: $(fig, '[data-bar="waste"]'),
    open: $(fig, '[data-bar="open"]'),
  };
  // The shift opened with some work already done. The page prints these opening numbers (src/home.ts), so the
  // console keeps its size when the scene starts.
  const base = { loads: 112, ore: 26_150, waste: 18_400 };
  const console = () => {
    const ore = base.ore + tally.ore;
    const waste = base.waste + tally.waste;
    const open = 3 + tally.queue;
    out.loads.textContent = fmt(base.loads + tally.loads);
    out.ore.textContent = fmt(ore);
    out.waste.textContent = fmt(waste);
    out.open.textContent = fmt(open);
    const total = ore + waste;
    bars.ore.style.flexGrow = String(ore / total);
    bars.waste.style.flexGrow = String(waste / total);
    bars.open.style.flexGrow = String(0.06);
  };
  const clock = range(fig, "clock", clockText, (h) => {
    s.setClock(h);
    s.loop.poke();
  });
  s.onNight((n) => mine.lights(n > 0.35));
  const pause = fig.querySelector<HTMLButtonElement>("[data-pause]");
  pause?.addEventListener("click", () => {
    const on = !s.loop.running;
    s.loop.running = on;
    pause.setAttribute("aria-pressed", String(!on));
    pause.setAttribute("aria-label", on ? "Pause" : "Play");
  });

  let since = 0;
  s.frame((dt) => {
    mine.update(dt);
    twin.update(dt, s.width, s.height);
    detailT += dt;
    if (detailT > 0.5) {
      detailT = 0;
      const far = (small ? 230 : 300) * (34 / s.camera.fov);
      for (const [t, d] of details) d.set(t.model.group.position.distanceTo(s.camera.position) > far);
    }
    since += dt;
    if (since > 0.25) {
      since = 0;
      paint();
      console();
    }
  });
  twin.show(layers[lens]);
  // The solve starts once the scene has drawn, so the first frames are not shared with the worker's start-up.
  setTimeout(
    () =>
      twin.start(() => {
        paint();
        s.loop.poke();
        // For anything waiting on the engine's result (the share-card photographer, tests): ready or failed.
        fig.dataset.twin = twin.state;
      }),
    reduced ? 0 : 1200,
  );
  paint();
  console();
  clock.set(s.clock);
  s.start();
  if (reduced) s.render();
  // The still picture gives way to the live scene once it has drawn.
  s.firstFrame(() => (fig.dataset.live = ""));
}
