/**
 * Underground, in the same ledger: a cutaway of a synthetic underground mine you can turn and zoom. The ground is a
 * thin slab over the workings; under it a decline spirals down past six levels, footwall drives and crosscuts reach a
 * dipping lens of ore, and its stopes stand in every state: planned, drilled, fired and being mucked, filled with
 * paste. An LHD mucks the fired stope and tips into the ore pass, a jumbo drills the 175 heading, crews work with cap
 * lamps on. Three layers put FrankenMining's claims on it: stopes (planned against scanned, overbreak and underbreak,
 * tonnes), ground support (each heading's critical-control state) and people (every tag against the shift's
 * register). An agent surfaces a refused blast plan (FMINE-REF-504) and the re-sequenced draft it prepares instead.
 * The mine, its people and every number are synthetic.
 */
import * as THREE from "three";
import { escape } from "../html";
import { read as readHtml, seg as segHtml } from "../parts";
import { makeJumbo, makeLHD, makePickup } from "./fleet";
import { $, $$, clamp, fmt, mix, reduced, seg, smooth } from "./lib";
import { makeMiner, type Miner } from "./miners";
import { stage, type Label, type Stage } from "./stage";
import {
  BOXCUT,
  CLOCK,
  CREWS,
  DECLINE,
  DECLINE_END,
  declineAt,
  driveZ,
  FEATURE,
  HEADINGS,
  L175_FACE,
  LEVELS,
  OREPASS,
  orepassZ,
  ozOf,
  AIR,
  airNeed,
  AIR_PER_KW,
  CURE,
  INSTRUMENTS,
  PASTE,
  PRICE,
  RAR,
  REFUGE,
  REGISTER,
  SHAFT,
  stope,
  STOPES,
  stopeMidX,
  SUBSTATION,
  TAGS,
  usdOf,
  zFW,
  zHW,
  type Crew,
  type Heading,
  type Stope,
} from "./underground-plan";
import { route } from "./underground-rock";
import { mergeStatic } from "./underground-surface";
import { EngineError } from "./engine";
import { drillComposites, muckWatch, stopeGrades, type Grades, type MuckWatch, type StopeGrade } from "./underground-model";
import { makeTwin, type Grade } from "./underground-twin";
import { buildWorld, glowTexture, halo, lightShaft, type World } from "./underground-world";

type Layer = "mine" | "stopes" | "support" | "people";
type Ev = "measured" | "derived" | "estimated" | "assumed" | "stale" | "refused";
const V = (x: number, y: number, z: number) => new THREE.Vector3(x, y, z);
/** The middle of the ore lens at (x, y). */
const oreZ = (x: number, y: number) => (zFW(x, y) + zHW(x, y)) / 2;
const ev = (e: Ev, text: string = e) => `<span class="ev ${e}">${escape(text)}</span>`;
const t0 = (n: number) => fmt(Math.round(n / 10) * 10);
const SUPPORT_TEXT = { verified: "verified", due: "due", overdue: "overdue" } as const;
const SUPPORT_EV: Record<Heading["status"], Ev> = { verified: "measured", due: "stale", overdue: "refused" };

const markup = () => `<div class="ug">
<div class="scene-view ug-view">
  <div class="hud-card hud-tl ug-card" data-card aria-live="polite"></div>
  <div class="hud-card hud-br agent-note ug-agent" data-agent></div>
  <div class="hud-card hud-bl ug-legend" data-legend></div>
  <p class="ug-hint" aria-hidden="true" data-avoid>Drag to turn · pinch or Ctrl-scroll to zoom · tap a label</p>
  <p class="ug-synth" data-avoid>Synthetic mine · illustrative</p>
</div>
<div class="scene-bar ug-bar">
  ${segHtml("layer", "Layer", [
    ["mine", "The mine"],
    ["stopes", "Stopes"],
    ["support", "Ground support"],
    ["people", "People"],
  ], "mine")}
  ${segHtml("view", "Go to", [
    ["whole", "Whole mine"],
    ["muck", "Mucking"],
    ["face", "Heading"],
    ["refuge", "Refuge"],
  ], "whole")}
  <button type="button" class="ug-twin-btn" data-twin aria-pressed="false">Engineering plan</button>
  <div class="reads">${readHtml("mucked", "Mucked this shift", "–", "bucket weigher")}${readHtml("tags", "Tags underground", `${TAGS} of ${REGISTER}`, "against the register")}</div>
</div>
<div class="ug-below" data-below></div>
</div>`;

/* ---------- Views ---------- */

interface ViewSpec {
  target: [number, number, number];
  distance: number;
  azimuth: number;
  elevation: number;
}

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.innerHTML = markup();
  const view = $(host, ".scene-view");
  const small = matchMedia("(max-width: 699px)").matches;
  const s: Stage = stage(view, {
    sky: "studio",
    fog: 0.00042,
    fov: small ? 40 : 30,
    near: 0.4,
    far: 6000,
    exposure: 1.12,
    drift: 0.5,
    maxPixelRatio: small ? 1.5 : 1.6,
    shadowMap: 2048,
    label: "A cutaway of a synthetic underground mine: drag to look around",
    labelArea: () => [0.01, 0.02, 0.99, 0.985],
  });
  s.orbit.minDistance = 12;
  s.orbit.maxDistance = 1300;
  s.orbit.minElevation = -10;
  s.orbit.maxElevation = 82;

  const VIEWS: Record<string, ViewSpec> = {
    whole: small ? { target: [22, -92, -6], distance: 705, azimuth: 24, elevation: 12 } : { target: [12, -118, -6], distance: 590, azimuth: 26, elevation: 17 },
    muck: { target: [24, -97, -14], distance: small ? 120 : 82, azimuth: 32, elevation: 24 },
    face: { target: [L175_FACE - 9, -172.5, driveZ(-175)], distance: small ? 52 : 36, azimuth: -52, elevation: 20 },
    refuge: { target: [REFUGE.x + 2, REFUGE.level + 2, driveZ(REFUGE.level) + 5], distance: small ? 60 : 40, azimuth: 28, elevation: 26 },
    // Long sections from the hanging-wall side (north), so no drive stands in front of the stopes.
    refusal: { target: [84, -111, oreZ(84, -111)], distance: small ? 220 : 158, azimuth: 196, elevation: 9 },
    draft: { target: [21, -111, oreZ(21, -111)], distance: small ? 220 : 158, azimuth: 164, elevation: 9 },
    decline: { target: [-60, -176, 8], distance: small ? 70 : 48, azimuth: 60, elevation: 22 },
    survey: { target: [stopeMidX(3), -74, zFW(stopeMidX(3), -74)], distance: small ? 60 : 42, azimuth: 30, elevation: 26 },
    paste: { target: [stopeMidX(5), -123, zFW(stopeMidX(5), -123) + 4], distance: small ? 56 : 38, azimuth: 26, elevation: 24 },
  };
  const go = (v: ViewSpec, seconds = 1.6) => s.orbit.set(v, seconds);
  go(VIEWS.whole!, 0);

  /* ---------- Light: dark rock, lamps, a key and a rim so the whole mine still reads from outside ---------- */
  s.sun.intensity = 1.3;
  s.sun.color.set("#ffe9d0");
  s.hemi.intensity = 0.28;
  s.hemi.color.set("#b9c7dc");
  s.hemi.groundColor.set("#2c221b");
  s.scene.environmentIntensity = 0.2;
  const rim = new THREE.DirectionalLight("#9fbbe6", 0.75);
  rim.position.set(-260, 140, -300);
  s.scene.add(rim);
  s.shadowBox([20, 0, 30], 175);
  // Only the surface casts the sun's shadow: the frustum stops a few metres under the ground.
  s.sun.shadow.camera.far = 175 * 3 + 300 + 14 / 0.7;
  s.sun.shadow.camera.updateProjectionMatrix();

  const step = () => new Promise<void>((r) => requestAnimationFrame(() => r()));
  const world: World = await buildWorld(small, step);
  s.scene.add(world.group);
  /** Stope grades from the engine (fmine-calib in WebAssembly), filled in when it answers. */
  const grades = new Map<string, Grade>();
  const engine: { grades: Grades | null; watch: MuckWatch | null; error: string } = { grades: null, watch: null, error: "" };
  const twin = makeTwin({ s, view, world, small, grade: (id) => grades.get(id) });
  s.scene.add(twin.group);
  await step();

  /* ---------- Machines ---------- */
  const lhd = makeLHD();
  lhd.set({ lights: true });
  world.group.add(lhd.group);
  const jumbo = makeJumbo();
  jumbo.set({ lights: true, stabilisers: 1, drilling: true });
  const j175 = driveZ(-175);
  jumbo.group.position.set(L175_FACE - 9.2, -175, j175);
  jumbo.group.rotation.y = Math.PI / 2;
  world.group.add(jumbo.group);
  const spot = (anchor: THREE.Object3D, colour: string, intensity: number, angle: number, dist: number, shadow: boolean) => {
    const l = new THREE.SpotLight(colour, intensity, dist, angle, 0.55, 2);
    l.target.position.set(0, -0.6, 8);
    anchor.add(l, l.target);
    if (shadow && !small) {
      l.castShadow = true;
      l.shadow.mapSize.set(1024, 1024);
      l.shadow.bias = -0.0006;
      l.shadow.camera.near = 0.5;
    }
    const h = halo("#fff6e8", 1.1, 0.9);
    h.position.z = 0.1;
    anchor.add(h);
    // The beam in the dust: a soft cone along the light, its motes drifting with world.tick.
    if (shadow) {
      const cone = lightShaft({ length: Math.min(dist * 0.3, 11), angle: angle * 0.8, colour, strength: 0.26 });
      cone.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), new THREE.Vector3(0, -0.6, 8).normalize());
      anchor.add(cone);
    }
    return l;
  };
  /** Lights whose shadows are drawn only while the camera is close enough to see them (each is a pass over the scene). */
  const shadowed: { light: THREE.Light; near: THREE.Object3D }[] = [];
  shadowed.push({ light: spot(lhd.anchors.front[0]!, "#f1f4ff", 80, 0.7, 40, true), near: lhd.group });
  spot(lhd.anchors.rear[0]!, "#f1f4ff", 45, 0.75, 30, false);
  for (const a of [lhd.anchors.front[1]!, lhd.anchors.rear[1]!]) {
    const h = halo("#fff6e8", 1.1, 0.9);
    h.position.z = 0.1;
    a.add(h);
  }
  shadowed.push({ light: spot(jumbo.anchors.front[0]!, "#eef2ff", 75, 0.8, 30, true), near: jumbo.group });
  spot(jumbo.anchors.front[1]!, "#eef2ff", 55, 0.8, 30, false);
  /**
   * A merged stand-in for a machine seen from far off: its opaque meshes as a few draws, riding in the machine's own
   * group. The detailed meshes step onto camera layer 1 rather than hiding, so the lights under them stay counted and no
   * shader recompiles.
   */
  const standIn = (g: THREE.Object3D) => {
    const c = g.clone(true);
    c.position.set(0, 0, 0);
    c.quaternion.identity();
    c.scale.set(1, 1, 1);
    const solid = (o: THREE.Object3D) => {
      const m = o as THREE.Mesh;
      return m.isMesh && !Array.isArray(m.material) && !m.material.transparent && !(m.material as THREE.ShaderMaterial).isShaderMaterial;
    };
    const drop: THREE.Object3D[] = [];
    c.traverse((o) => {
      if (o !== c && !solid(o) && !(o.type === "Object3D" || (o as THREE.Group).isGroup || (o as THREE.Mesh).isMesh)) drop.push(o);
      else if ((o as THREE.Mesh).isMesh && !solid(o)) drop.push(o);
    });
    for (const o of drop) o.removeFromParent();
    const proxy = mergeStatic(c);
    proxy.visible = false;
    const parts: THREE.Object3D[] = [];
    g.traverse((o) => {
      if (solid(o)) parts.push(o);
    });
    g.add(proxy);
    let far = false;
    return (isFar: boolean) => {
      if (isFar === far) return;
      far = isFar;
      proxy.visible = far;
      for (const p of parts) p.layers.set(far ? 1 : 0);
    };
  };
  const lhdFar = standIn(lhd.group);
  const jumboFar = standIn(jumbo.group);
  const point = (colour: string, intensity: number, dist: number, p: THREE.Vector3) => {
    const l = new THREE.PointLight(colour, intensity, dist, 2);
    l.position.copy(p);
    world.group.add(l);
    return l;
  };
  // The working faces glow warm under their lights.
  const faceGlow = point("#ffb36b", 36, 24, V(L175_FACE - 3, -171.5, j175));
  const decEnd = world.end("decline");
  point("#ffc184", 22, 18, decEnd.p.clone().addScaledVector(decEnd.dir, -4).add(V(0, 4, 0)));
  point("#ffd3a0", 16, 16, V(OREPASS.x, -96, orepassZ(-100) + 6));
  point("#9dffc6", 12, 14, V(REFUGE.x, REFUGE.level + 4, driveZ(REFUGE.level) + 4));
  if (!small) {
    point("#ffcf9a", 14, 14, V(stopeMidX(5), -121, zFW(stopeMidX(5), -123) + 6));
    point("#ffcf9a", 12, 14, V(stopeMidX(3), -71, zFW(stopeMidX(3), -73) + 4));
    point("#ffe0b8", 10, 12, V(SUBSTATION.x, SUBSTATION.level + 4, driveZ(SUBSTATION.level) + 4));
    point("#ffd7a8", 12, 14, V(DECLINE.cx, -96, 20));
    point("#ffd7a8", 10, 14, V(103, -46, driveZ(-50)));
  }

  /* ---------- People ---------- */
  type Placed = { miner: Miner; crew: Crew; tag: string; lamp: THREE.Sprite; marker: THREE.Sprite };
  const placed: Placed[] = [];
  const markerTex = (() => {
    const c = document.createElement("canvas");
    c.width = c.height = 64;
    const x = c.getContext("2d")!;
    x.strokeStyle = "rgba(95,211,141,1)";
    x.lineWidth = 6;
    x.beginPath();
    x.arc(32, 32, 22, 0, Math.PI * 2);
    x.stroke();
    x.fillStyle = "rgba(95,211,141,1)";
    x.beginPath();
    x.arc(32, 32, 10, 0, Math.PI * 2);
    x.fill();
    return new THREE.CanvasTexture(c);
  })();
  const marker = () => {
    const m = new THREE.Sprite(new THREE.SpriteMaterial({ map: markerTex, sizeAttenuation: false, depthTest: false, transparent: true, opacity: 0 }));
    m.scale.setScalar(small ? 0.045 : 0.026);
    m.renderOrder = 20;
    return m;
  };
  const markers: THREE.Sprite[] = [];
  /** Where each person stands and what they do there. */
  const decFace = declineAt(DECLINE_END - 6.5);
  const decTo = declineAt(DECLINE_END);
  const spots: Record<string, { p: THREE.Vector3; face: THREE.Vector3; work: "stand" | "tablet" | "radio" | "inspect" | "point" | "crouch"; target?: THREE.Vector3 }[]> = {
    dev: [{ p: V(L175_FACE - 24, -175, j175 + 1.9), face: V(L175_FACE, -175, j175), work: "radio" }],
    geo: [{ p: V(decFace.x, decFace.y, decFace.z), face: V(decTo.x, decTo.y, decTo.z), work: "point", target: V(decTo.x, decTo.y + 4.6, decTo.z) }],
    survey: [{ p: V(stopeMidX(3) - 1.3, -75, zFW(stopeMidX(3), -73) + 4.2), face: V(stopeMidX(3), -75, zFW(stopeMidX(3), -73) - 6), work: "tablet" }],
    paste: [
      { p: V(stopeMidX(5) + 1.0, -125, zFW(stopeMidX(5), -123) + 5.0), face: V(stopeMidX(5), -125, zFW(stopeMidX(5), -123)), work: "inspect", target: V(stopeMidX(5) + 0.6, -124.6, zFW(stopeMidX(5), -123) + 3.4) },
      { p: V(stopeMidX(5) - 1.3, -125, zFW(stopeMidX(5), -123) + 6.6), face: V(stopeMidX(5), -125, zFW(stopeMidX(5), -123)), work: "tablet" },
    ],
    super: [{ p: V(DECLINE.cx + 1.7, -100, 10.5), face: V(DECLINE.cx - 4, -100, SHAFT.z), work: "tablet" }],
    services: [
      { p: V(101, -50, driveZ(-50) - 1.4), face: V(112, -50, driveZ(-50)), work: "point", target: V(108, -45.6, driveZ(-50) - 1.2) },
      { p: V(104.5, -50, driveZ(-50) + 1.5), face: V(100, -50, driveZ(-50)), work: "radio" },
    ],
    elec: [{ p: V(SUBSTATION.x + 1.2, -125, driveZ(-125) + 3.1), face: V(SUBSTATION.x, -125, driveZ(-125) + 9), work: "inspect", target: V(SUBSTATION.x + 0.4, -123.8, driveZ(-125) + 5.1) }],
  };
  const lampTex = glowTexture();
  let seed = 3;
  const crewAnchor = new Map<string, THREE.Object3D>();
  for (const crew of CREWS) {
    const list = spots[crew.id] ?? [];
    let k = 0;
    for (const person of crew.people) {
      if (person.at === "lhd" || person.at === "jumbo") {
        const m = marker();
        m.position.set(0, 4.6, person.at === "lhd" ? -0.6 : -1.5);
        (person.at === "lhd" ? lhd.group : jumbo.group).add(m);
        markers.push(m);
        if (!crewAnchor.has(crew.id)) crewAnchor.set(crew.id, person.at === "lhd" ? lhd.group : jumbo.group);
        continue;
      }
      const at = list[k++];
      if (!at) continue;
      const role = person.role === "blaster" ? "blaster" : person.role;
      const miner = makeMiner({ role, seed: seed++ * 7 + 1, lamp: true, respirator: role === "underground", detail: small ? 0 : 1 });
      miner.group.position.copy(at.p);
      world.group.add(miner.group);
      miner.face(at.face);
      if (at.work !== "stand") miner.work(at.work, at.target ?? at.face);
      const head = miner.staff.bones.get("head");
      const lamp = new THREE.Sprite(new THREE.SpriteMaterial({ map: lampTex, color: "#fff2d6", blending: THREE.AdditiveBlending, depthWrite: false, transparent: true }));
      lamp.scale.setScalar(0.55);
      lamp.position.set(0, 0.11, 0.13);
      head?.add(lamp);
      const mk = marker();
      mk.position.set(0, 2.5, 0);
      miner.group.add(mk);
      markers.push(mk);
      placed.push({ miner, crew, tag: person.tag, lamp, marker: mk });
      if (!crewAnchor.has(crew.id)) crewAnchor.set(crew.id, miner.group);
      if (placed.length % 3 === 0) await step();
    }
  }
  // The surveyor's scanner on its boom, reaching out over the stope brow; and the supervisor's pickup.
  {
    const x = stopeMidX(3);
    const zb = zFW(x, -73);
    const g = new THREE.Group();
    const grey = new THREE.MeshStandardMaterial({ color: "#3a3f44", metalness: 0.5, roughness: 0.5 });
    for (let i = 0; i < 3; i++) {
      const a = (i / 3) * Math.PI * 2;
      const leg = new THREE.Mesh(new THREE.CylinderGeometry(0.025, 0.025, 1.5, 6), grey);
      leg.position.set(Math.cos(a) * 0.32, 0.72, Math.sin(a) * 0.32);
      leg.rotation.set(Math.sin(a) * 0.22, 0, -Math.cos(a) * 0.22);
      g.add(leg);
    }
    const boom = new THREE.Mesh(new THREE.CylinderGeometry(0.04, 0.04, 6, 8), new THREE.MeshStandardMaterial({ color: "#d9a12b", roughness: 0.5 }));
    boom.rotation.x = Math.PI / 2 - 0.12;
    boom.position.set(0, 1.75, -2.9);
    g.add(boom);
    const head = new THREE.Mesh(new THREE.SphereGeometry(0.16, 12, 10), new THREE.MeshStandardMaterial({ color: "#e8e8e4", emissive: "#ff3b2f", emissiveIntensity: 0.6 }));
    head.position.set(0, 1.4, -5.8);
    g.add(head);
    const dot = halo("#ff4a3a", 1.2, 0.9);
    dot.position.copy(head.position);
    g.add(dot);
    g.position.set(x + 0.4, -75, zb + 2.4);
    world.group.add(g);
  }
  {
    const p = makePickup();
    p.set({ lights: true });
    p.group.position.set(DECLINE.cx - 1.2, -100, 19);
    p.group.rotation.y = Math.PI;
    const parked = new THREE.Group();
    parked.add(p.group);
    world.group.add(mergeStatic(parked));
  }

  /* ---------- The LHD's cycle: load at the drawpoint, back out, tram to the ore pass, tip, return ---------- */
  const yL = -100;
  const dz = driveZ(yL);
  const xd = stopeMidX(3);
  const loadZ = zFW(xd, yL + 2) + 3.0 + 5.4;
  const tipX = OREPASS.x;
  const tipZ = orepassZ(yL) + 7.2;
  type Leg = { pts: THREE.Vector3[]; len: number[]; dir: 1 | -1; v: number; loaded: boolean; then?: "tip" | "load" };
  const leg = (ctrl: THREE.Vector3[], dir: 1 | -1, v: number, loaded: boolean, then?: "tip" | "load"): Leg => {
    const pts = route(ctrl, 7.5, 0.4);
    const len = [0];
    for (let i = 1; i < pts.length; i++) len.push(len[i - 1]! + pts[i]!.distanceTo(pts[i - 1]!));
    return { pts, len, dir, v, loaded, then };
  };
  const legs: Leg[] = [
    leg([V(xd, yL, loadZ), V(xd, yL, dz), V(xd + 15, yL, dz)], -1, 2.4, true),
    leg([V(xd + 15, yL, dz), V(tipX, yL, dz), V(tipX, yL, tipZ)], 1, 4.6, true, "tip"),
    leg([V(tipX, yL, tipZ), V(tipX, yL, dz), V(tipX - 13, yL, dz)], -1, 2.4, false),
    leg([V(tipX - 13, yL, dz), V(xd, yL, dz), V(xd, yL, loadZ)], 1, 4.8, false, "load"),
  ];
  const cyc = { leg: 0, d: 0, dwell: 0, phase: "load" as "drive" | "load" | "tip", t: 0, steer: 0 };
  // This shift (from 06:40, about ten buckets an hour) and the stope since it was fired.
  const tally = { buckets: 46, tonnes: 46 * 17.2, stope: 14_460 };
  const PAYLOAD = 17.2;
  const at = (l: Leg, d: number) => {
    let i = l.len.findIndex((x) => x >= d);
    if (i <= 0) i = 1;
    const a = l.pts[i - 1]!;
    const b = l.pts[i]!;
    const f = clamp((d - l.len[i - 1]!) / Math.max(1e-6, l.len[i]! - l.len[i - 1]!), 0, 1);
    const p = a.clone().lerp(b, f);
    const tan = b.clone().sub(a).setY(0).normalize();
    return { p, tan };
  };
  const heading = (l: Leg, d: number) => {
    const { tan } = at(l, d);
    return Math.atan2(tan.x * l.dir, tan.z * l.dir);
  };
  const placeLHD = (l: Leg, d: number) => {
    const { p } = at(l, d);
    lhd.group.position.copy(p);
    lhd.group.rotation.y = heading(l, d);
  };
  placeLHD(legs[3]!, legs[3]!.len[legs[3]!.len.length - 1]!);
  const ease = (x: number) => smooth(x);
  const lhdStep = (dt: number) => {
    if (cyc.phase === "load") {
      cyc.t += dt;
      const t = cyc.t;
      // Into the muck, crowd, break out and rack back.
      lhd.set({ lift: mix(0.02, 0.24, ease((t - 3.4) / 2)), tilt: mix(0, -0.4, ease((t - 1.6) / 2.2)), speed: t < 1.4 ? 0.6 : 0 });
      if (t > 6.2) {
        cyc.phase = "drive";
        cyc.leg = 0;
        cyc.d = 0;
      }
      lhd.update(dt);
      return;
    }
    if (cyc.phase === "tip") {
      cyc.t += dt;
      const t = cyc.t;
      lhd.set({ lift: t < 4.2 ? mix(0.24, 0.95, ease(t / 1.6)) : mix(0.95, 0.08, ease((t - 4.2) / 1.6)), tilt: t < 4.2 ? mix(-0.4, 1, ease((t - 1.6) / 1.2)) : mix(1, -0.05, ease((t - 4.2) / 1.2)), speed: 0 });
      if (t > 5.9) {
        tally.buckets += 1;
        tally.tonnes += PAYLOAD;
        tally.stope += PAYLOAD;
        // The bucket's face sample is bet against the model by the drift monitor (one engine call).
        engine.watch?.tip();
        cyc.phase = "drive";
        cyc.leg = 2;
        cyc.d = 0;
      }
      lhd.update(dt);
      return;
    }
    const l = legs[cyc.leg]!;
    const L = l.len[l.len.length - 1]!;
    const a = 1.3;
    const v = Math.min(l.v, Math.sqrt(2 * a * Math.max(cyc.d, 0) + 0.3), Math.sqrt(2 * a * Math.max(L - cyc.d, 0) + 0.05));
    cyc.d += v * dt;
    const h0 = heading(l, Math.max(0, cyc.d - 0.6));
    const h1 = heading(l, Math.min(L, cyc.d + 0.6));
    let dh = h1 - h0;
    dh = Math.atan2(Math.sin(dh), Math.cos(dh));
    cyc.steer = mix(cyc.steer, clamp((dh / 1.2) * 4.2, -0.62, 0.62), 1 - Math.exp(-dt * 4));
    lhd.set({ steer: cyc.steer, speed: v * l.dir, lift: l.loaded ? 0.24 : 0.08, tilt: l.loaded ? -0.4 : -0.05 });
    placeLHD(l, Math.min(cyc.d, L));
    lhd.update(dt);
    if (cyc.d >= L) {
      if (l.then === "tip") {
        cyc.phase = "tip";
        cyc.t = 0;
      } else if (l.then === "load") {
        cyc.phase = "load";
        cyc.t = 0;
      } else {
        cyc.leg = (cyc.leg + 1) % legs.length;
        cyc.d = 0;
      }
    }
  };

  /* ---------- The jumbo drills its round ---------- */
  const holes: [number, number][] = [];
  for (let y = 0.9; y <= 4.9; y += 0.62)
    for (let x = -2.2; x <= 2.21; x += 0.55) {
      if (y > 4.0 && Math.abs(x) > 2.4 - (y - 4.0) * 1.6) continue;
      holes.push([x, y]);
    }
  const booms = [0, 1].map((b) => ({ queue: holes.filter(([x]) => (b === 0 ? x >= 0 : x < 0)), i: 0, t: 0, from: [0, 0.06], to: [0, 0.06] }));
  const aim = (b: number, h: [number, number]) => {
    const side = b === 0 ? 0.62 : -0.62;
    const reach = 8.4;
    return [Math.atan2(h[0] - side, reach), Math.atan2(h[1] - 1.85, reach)] as [number, number];
  };
  const jumboStep = (dt: number) => {
    const st: [{ yaw: number; pitch: number; feed: number }, { yaw: number; pitch: number; feed: number }] = [
      { yaw: 0, pitch: 0, feed: 0 },
      { yaw: 0, pitch: 0, feed: 0 },
    ];
    booms.forEach((b, i) => {
      b.t += dt * (1 + i * 0.13);
      const T = [2.2, 7.5, 1.6];
      if (b.t > T[0]! + T[1]! + T[2]!) {
        b.t = 0;
        b.i = (b.i + 1) % b.queue.length;
        b.from = b.to;
        b.to = aim(i, b.queue[b.i]!);
      }
      const m = ease(b.t / T[0]!);
      const feed = b.t < T[0]! ? 0 : b.t < T[0]! + T[1]! ? (b.t - T[0]!) / T[1]! : 1 - (b.t - T[0]! - T[1]!) / T[2]!;
      st[i]!.yaw = mix(b.from[0]!, b.to[0]!, m);
      st[i]!.pitch = mix(b.from[1]!, b.to[1]!, m);
      st[i]!.feed = feed * 0.92;
    });
    jumbo.set({ booms: st });
    jumbo.update(dt);
  };
  booms.forEach((b, i) => (b.to = aim(i, b.queue[0]!)));

  /* ---------- Labels ---------- */
  /** A label, the layer it belongs to, what it selects, and its rank when labels crowd (lower wins). */
  type Pin = { label: Label; layer: Layer | "all"; id?: string; prio: number };
  const pins: Pin[] = [];
  const pin = (layer: Layer | "all", html: string, at: THREE.Vector3 | THREE.Object3D, o: { y?: number; className?: string; id?: string; prio?: number } = {}) => {
    // One span inside, so a label that hangs to the left of its point (the stage flips the row) keeps its word order.
    const label = s.label(`<span class="ug-l">${html}</span>`, at, { y: o.y ?? 0, className: o.className ?? "stem" });
    if (o.id) {
      label.el.classList.add("ug-pick");
      label.el.dataset.pick = o.id;
      label.el.addEventListener("click", () => pick(o.id!));
    }
    pins.push({ label, layer, id: o.id, prio: o.prio ?? 10 + pins.length * 0.01 });
    return label;
  };
  const chip = (e: Ev, t: string) => `<span class="ev ${e}">${escape(t)}</span>`;
  // The mine.
  pin("mine", "Portal · decline at 1 in 8", V(BOXCUT.x0 + 4, DECLINE.portalY + 6, BOXCUT.z), { y: 4, prio: 4 });
  pin("mine", "Shaft and headframe", V(SHAFT.x, 33, SHAFT.z), { y: 4, prio: 6 });
  pin("mine", "Main fan on the return-air raise", V(RAR.x, 14, RAR.z), { y: 4, prio: 5 });
  pin("mine", "Paste plant", V(36, 16, 8), { y: 4, prio: 7 });
  pin("mine", `LHD 04 · mucking ${FEATURE.mucking}`, lhd.group, { y: 4, id: "lhd", prio: 1 });
  pin("mine", "Jumbo 02 · 175 heading", jumbo.group, { y: 4, id: "jumbo", prio: 2 });
  pin("mine", "Refuge chamber", world.refuge.group, { y: 3.4, id: "refuge", prio: 3 });
  pin("mine", "Ore pass", V(OREPASS.x, -138, orepassZ(-138)), { y: 0, prio: 8 });
  for (const L of LEVELS) pin("all", `${-L}`, V(-150, L + 2.5, 118), { className: "ug-lvl", prio: 50 });
  // Stopes.
  const stopeLabel = (id: string, html: string, dy = 3, prio = 10) => {
    const st = stope(id);
    const x = (st.xa + st.xb) / 2;
    const y = st.level + 25;
    pin("stopes", html, V(x, y + dy, (zFW(x, y) + zHW(x, y)) / 2), { id: `stope:${id}`, y: 0, prio });
  };
  const scan = world.scan;
  const overPct = ((scan.surveyed - scan.design) / scan.design) * 100;
  stopeLabel(FEATURE.mucking, `${FEATURE.mucking} · scanned <b>${overPct >= 0 ? "+" : "−"}${fmt(Math.abs(overPct), 1)}%</b>${chip("measured", "measured")}`, 3, 3);
  stopeLabel("P100-05", `P100-05 · drilled`, 3, 6);
  stopeLabel(FEATURE.curing, `${FEATURE.curing} · paste <b>${fmt(stope(FEATURE.curing).fill!.mpa, 2)} MPa</b>${chip("measured", "7-day")}`, -2, 2);
  stopeLabel(FEATURE.refused, `${FEATURE.refused} · blast plan${chip("refused", "FMINE-REF-504")}`, -8, 1);
  stopeLabel(FEATURE.draft, `${FEATURE.draft} · drafted to fire`, -2, 4);
  stopeLabel("S150-04", `150 block · paste-filled, cured`, -6, 7);
  // Ground support: one label per heading.
  const headingAt = (h: Heading) => {
    if (h.at === "decline") return world.end("decline").p;
    if (h.at === "l175") return world.end("drive-175").p;
    return world.end(`xc${h.at.level}-${h.at.k}`).p;
  };
  for (const h of HEADINGS)
    pin("support", `${escape(h.name)}${chip(SUPPORT_EV[h.status], SUPPORT_TEXT[h.status])}`, headingAt(h).clone().add(V(0, 5.6, 0)), {
      id: `head:${h.id}`,
      prio: { overdue: 1, due: 2, verified: 3 }[h.status],
    });
  // Ground instruments: extensometers and convergence stations, their rate against the trigger.
  const instrumentAt = (i: (typeof INSTRUMENTS)[number]) => {
    const placed = world.instruments.get(i.id);
    if (placed) return placed.clone();
    const z = i.k ? zFW(i.x, i.level) + 4 : driveZ(i.level);
    return V(i.x, i.level + 5, z);
  };
  for (const i of INSTRUMENTS) {
    const over = i.rate > i.trigger;
    pin("support", `${i.id} · <b>${fmt(i.rate, 1)} mm/day</b>${chip(over ? "refused" : "measured", over ? "over trigger" : "measured")}`, instrumentAt(i).add(V(0, 1.2, 0)), {
      id: `inst:${i.id}`,
      prio: over ? 1.5 : 4,
    });
  }
  // People: a label for each crew and the refuge.
  for (const crew of CREWS) {
    const a = crewAnchor.get(crew.id);
    if (!a) continue;
    pin("people", `${escape(crew.name)} · <b>${crew.people.length}</b>`, a, { y: crew.id === "prod" || crew.id === "dev" ? 4.6 : 2.6, id: `crew:${crew.id}` });
  }
  pin("people", `Refuge RC-125 · <b>0 of 16</b> inside`, world.refuge.group, { y: 3.4, id: "refuge", prio: 5 });

  /* ---------- HUD ---------- */
  const card = $(host, "[data-card]");
  const agent = $(host, "[data-agent]");
  const legend = $(host, "[data-legend]");
  const below = $(host, "[data-below]");
  const refused = world.stopes.get(FEATURE.refused)!;
  const draft = world.stopes.get(FEATURE.draft)!;
  const curing = stope(FEATURE.curing);
  const p1 = stope("P125-01");
  const p3 = stope("P125-03");
  const rT = refused.tonnes;
  const dT = draft.tonnes;
  const musd = (v: number) => `$${fmt(v / 1e6, 1)}M`;
  const gradeOf = (id: string): StopeGrade | undefined => engine.grades?.byStope.get(id);
  const pending = `<span class="ug-pending">${engine.error ? `engine: ${escape(engine.error)}` : "computing"}</span>`;
  const today = curing.fill!.days;
  const paintAgent = () => {
    const rg = gradeOf(FEATURE.refused);
    const dg = gradeOf(FEATURE.draft);
    const money =
      rg && dg
        ? `The week keeps <b>${t0(dT)} of ${t0(rT)} t</b> and <b>${musd(usdOf(dT, dg.mean))} of ${musd(usdOf(rT, rg.mean))}</b> of recovered gold ${ev("estimated")} at $${fmt(PRICE.usdPerOz)}/oz ${ev("assumed")}; the rest is deferred.`
        : `The week keeps <b>${t0(dT)} of ${t0(rT)} t</b>; its gold is ${pending}.`;
    agent.innerHTML = `<p class="micro">Agent · stope sequence</p>
<p class="ug-lead"><b>Blast plan for ${FEATURE.refused} refused</b> ${ev("refused", "FMINE-REF-504")}</p>
<p>Its west wall is ${FEATURE.curing} paste, poured ${PASTE.poured}: <b>${fmt(curing.fill!.mpa, 2)} MPa</b> at 7 days ${ev("measured")}, near ${fmt(CURE.at(today), 2)} today on the cure curve. A 25 m wall needs <b>${fmt(PASTE.design, 2)} MPa</b> ${ev("estimated")}, about 30 Nov.</p>
<p><b>Draft:</b> fire ${FEATURE.draft} instead: drilled, both fill walls cured (${fmt(p1.fill!.mpa, 2)} and ${fmt(p3.fill!.mpa, 2)} MPa). ${money} ${FEATURE.refused} follows the 14-day cylinders on ${PASTE.cylinders14}.</p>
<div class="ug-acts"><button type="button" class="btn sm" data-go="refusal">See the refusal</button><button type="button" class="btn sm" data-go="draft">See the draft</button></div>
<span class="act">Draft for the underground manager to approve · not sent</span>`;
    for (const b of $$<HTMLButtonElement>(agent, "[data-go]")) b.addEventListener("click", () => goTo(b.dataset.go!));
  };

  let layer: Layer = "mine";
  const sel: Record<Layer, string | null> = { mine: null, stopes: `stope:${FEATURE.mucking}`, support: null, people: null };
  let focus: string[] = [];

  /** A claim row; a trailing <small> (an interval, the rule behind a number) goes on its own line under it. */
  const row = (k: string, v: string, e?: Ev, et?: string) => {
    const m = /^(.*?)\s*<small([^>]*)>(.*)<\/small>$/s.exec(v);
    const main = m ? m[1]! : v;
    return `<dt>${k}</dt><dd>${main}${e ? ` ${ev(e, et ?? e)}` : ""}</dd>${m ? `<dd class="ug-sub-dd"${m[2]}>${m[3]}</dd>` : ""}`;
  };
  const mineCard = () => `<p class="micro">Synthetic mine · day shift · ${CLOCK}</p>
<dl>${row("Mucked this shift", `<span data-live="mucked">${fmt(tally.tonnes)}</span> t`, "derived")}
${row(`${FEATURE.mucking} vs design`, `${overPct >= 0 ? "+" : "−"}${fmt(Math.abs(overPct), 1)}%`, "measured", "scan")}
${row("Headings verified", `${HEADINGS.filter((h) => h.status === "verified").length} of ${HEADINGS.length}`, "measured")}
${row("People accounted for", `${TAGS} of ${REGISTER}`, "measured")}
${row("Air, 100 level", `${fmt(air100.measured, 1)} m³/s <small>needs ${fmt(airNeed(air100), 1)}: ${fmt(air100.diesel[0]!.kw)} kW × ${fmt(AIR_PER_KW, 2)}</small>`, "measured")}
${row("Gold, drilled stopes", goldDrilled(), "estimated")}</dl>
<p class="ug-src">${engineLine()} Every number is a claim with its evidence; pick a layer to open them.</p>`;
  const air100 = AIR.find((a) => a.level === -100)!;
  const goldDrilled = () => {
    const drilled = [...world.stopes.values()].filter((v) => v.stope.state === "drilled");
    const gs = drilled.map((v) => ({ t: v.tonnes, g: gradeOf(v.stope.id) }));
    if (gs.some((x) => !x.g)) return pending;
    const sum = (k: "mean" | "lo" | "hi") => gs.reduce((a, x) => a + ozOf(x.t, x.g![k]), 0);
    return `${fmt(sum("mean"))} oz <small>${fmt(Math.round(sum("lo") / 100) * 100)}–${fmt(Math.round(sum("hi") / 100) * 100)}</small>`;
  };
  const engineLine = () => {
    const g = engine.grades;
    if (engine.error) return `The engine refused: ${escape(engine.error)}.`;
    if (!g) return "Grades are being computed by FrankenMining’s engine in this browser.";
    return `Grades: fmine-calib’s conditional Gaussian predictor, compiled to WebAssembly and run in this browser: ${fmt(g.points)} points on ${fmt(g.composites)} drill composites in ${fmt(g.ms, 0)} ms. Intervals are 80%.`;
  };

  const scanBar = () => {
    const W = 260;
    const max = Math.max(scan.surveyed + scan.under, scan.design) * 1.02;
    const x = (t: number) => (t / max) * W;
    const kept = scan.design - scan.under;
    return `<svg class="ug-scanbar" viewBox="0 0 ${W} 34" role="img" aria-label="Design ${t0(scan.design)} t against scanned ${t0(scan.surveyed)} t">
<rect x="0" y="2" width="${x(scan.design).toFixed(1)}" height="11" rx="2" class="pl"/>
<rect x="0" y="19" width="${x(kept).toFixed(1)}" height="11" rx="2" class="ok"/>
<rect x="${x(kept).toFixed(1)}" y="19" width="${x(scan.over).toFixed(1)}" height="11" class="ob"/>
<rect x="${x(kept - scan.under).toFixed(1)}" y="19" width="${x(scan.under).toFixed(1)}" height="11" class="ub"/>
</svg>`;
  };
  /** The drift monitor's answer: the ratio, and its line underneath (buckets, alarm or e-value). */
  const monitorText = (): [string, string] => {
    const w = engine.watch;
    if (!w?.last) return [pending, ""];
    const sub = `${fmt(w.buckets)} buckets · ${w.alarmAt ? `drift alarm since bucket ${w.alarmAt}` : `e-value ${fmt(w.last.eLocation, 1)} of 40`}`;
    return [`${fmt(w.ratio, 2)}×`, sub];
  };
  /** The curing fill's strength against time: the fitted curve, the cylinder break, today and the design line. */
  const cureChart = (days: number) => {
    const W = 260;
    const H = 84;
    const x = (d: number) => 24 + (d / 30) * (W - 32);
    const y = (m: number) => H - 16 - (m / 1.6) * (H - 24);
    let d = "";
    for (let t = 0; t <= 30; t += 0.5) d += `${t ? "L" : "M"}${x(t).toFixed(1)} ${y(CURE.at(t)).toFixed(1)}`;
    const ticks = [0, 7, 14, 21].map((t) => `<text x="${x(t)}" y="${H - 3}" class="ax" text-anchor="middle">${t}</text>`).join("");
    const yt = [0.5, 1].map((m) => `<text x="19" y="${y(m) + 3}" class="ax" text-anchor="end">${fmt(m, 1)}</text>`).join("");
    const cyl = CURE.cylinders.map((c) => `<circle cx="${x(c.days)}" cy="${y(c.mpa)}" r="3.2" class="cyl"/>`).join("");
    return `<svg class="ug-cure" viewBox="0 0 ${W} ${H}" role="img" aria-label="Paste strength against cure time: ${fmt(CURE.at(days), 2)} MPa today on day ${days}, design ${fmt(PASTE.design, 2)} MPa on day ${PASTE.reachDays}">
<line x1="${x(0)}" x2="${x(30)}" y1="${y(PASTE.design)}" y2="${y(PASTE.design)}" class="des"/>
<line x1="${x(days)}" x2="${x(days)}" y1="${y(1.6)}" y2="${y(0)}" class="now"/>
<path d="${d}" class="crv"/>${cyl}
<circle cx="${x(PASTE.reachDays)}" cy="${y(PASTE.design)}" r="3" class="hit"/>
<text x="${x(days) + 4}" y="${y(1.5)}" class="lb">today, day ${days}</text>
<text x="${x(30)}" y="${y(PASTE.design) - 4}" class="lb" text-anchor="end">design ${fmt(PASTE.design, 2)} MPa</text>
${ticks}${yt}<text x="${x(30)}" y="${H - 3}" class="ax" text-anchor="end">28 days</text><text x="19" y="${y(1.6) + 3}" class="ax" text-anchor="end">MPa</text></svg>`;
  };
  const stopeCard = (id: string) => {
    const v = world.stopes.get(id);
    if (!v) return "";
    const st = v.stope;
    const sg = gradeOf(st.id);
    const g = sg ? `${fmt(sg.mean, 2)} g/t <small>${fmt(sg.lo, 1)}–${fmt(sg.hi, 1)}</small>` : pending;
    const oz = sg ? `${fmt(ozOf(v.tonnes, sg.mean))} oz <small>${fmt(Math.round(ozOf(v.tonnes, sg.lo) / 10) * 10)}–${fmt(Math.round(ozOf(v.tonnes, sg.hi) / 10) * 10)}</small>` : pending;
    const head = (state: string) => `<p class="micro">Stope ${st.id} · ${state}</p>`;
    if (st.state === "fired")
      return `${head("fired, mucking · planned against scanned")}${scanBar()}
<dl>${row("Planned (design solid)", `${t0(scan.design)} t`, "estimated")}
${row("Scanned opening", `${t0(scan.surveyed)} t`, "measured")}
${row("Overbreak", `+${t0(scan.over)} t`, "measured")}
${row("Underbreak (west rib)", `−${t0(scan.under)} t`, "measured")}
${row("Hanging-wall slough", `${fmt(scan.elos, 1)} m`, "measured")}
${row("Slough forecast at design", "0.5 m", "estimated")}
${row("Mucked since firing", `<span data-live="stope">${fmt(tally.stope)}</span> t`, "measured")}
${row("Still in the stope", `<span data-live="left">${t0(Math.max(0, scan.surveyed - tally.stope))}</span> t`, "estimated")}
${row("Buckets against the model", `<span data-live="ratio">${monitorText()[0]}</span> <small data-live="ratio-sub">${monitorText()[1]}</small>`, "measured", "fmine-calib")}</dl>
<p class="ug-src">Each bucket’s face sample is bet against the grade model by fmine-calib’s drift monitor (two e-processes, alarm at 2/α = 40) in this browser. Cavity scan 23 Nov 21:40 from 075 XC-03, compared with the design on firing. Density ${fmt(2.8, 1)} t/m³ ${ev("assumed")}. The slough forecast is scored against the scan.</p>`;
    if (st.state === "curing")
      return `${head("paste fill, curing")}${cureChart(st.fill!.days)}
<dl>${row("Poured", `${PASTE.poured} · ${st.fill!.days} days`)}
${row("Binder", `${fmt(PASTE.binder, 1)}%`, "measured")}
${row("7-day cylinders (3 of 3)", `${fmt(st.fill!.mpa, 2)} MPa`, "measured")}
${row("Design, 25 m exposed wall", `${fmt(PASTE.design, 2)} MPa`, "estimated")}
${row(`Strength today, day ${st.fill!.days}`, `${fmt(CURE.at(st.fill!.days), 2)} MPa`, "estimated")}
${row("Reaches design", `day ${PASTE.reachDays}, about 30 Nov`, "estimated")}
${row("14-day cylinders", PASTE.cylinders14)}</dl>
<p class="ug-src">The curve is a hyperbolic strength gain fitted in the page through the cylinder breaks and the mix design. While this fill is under design strength, a blast plan for a stope that exposes it is refused (FMINE-REF-504).</p>`;
    if (st.state === "filled")
      return `${head("paste-filled")}
<dl>${row("Filled", `${st.fill!.days} days ago`)}
${row(st.fill!.test, `${fmt(st.fill!.mpa, 2)} MPa`, "measured")}
${row("Design", `${fmt(PASTE.design, 2)} MPa`, "estimated")}
${row("Mined", `${t0(v.tonnes)} t`, "measured")}
${row("Grade", g, "estimated")}</dl>`;
    // The walls it would expose: the stopes either side along strike, when they are paste.
    const sides = ([[-1, "West"], [1, "East"]] as const)
      .map(([dk, side]) => ({ side, n: STOPES.find((x) => x.level === st.level && x.k === st.k + dk) }))
      .filter((w): w is { side: "West" | "East"; n: Stope } => !!w.n && (w.n.state === "filled" || w.n.state === "curing"));
    const weak = sides.find((w) => w.n.fill!.mpa < PASTE.design);
    const plan = weak ? "refused" : id === FEATURE.draft ? "drafted, to approve" : "can be issued";
    return `${head(st.state === "drilled" ? "drilled" : "planned")}
<dl>${row("Design tonnes", `${t0(v.tonnes)} t`, "estimated")}
${row("Grade", g, "estimated")}
${row("Gold in place", oz, "estimated")}
${sides.map((w) => row(`${w.side} wall, ${w.n.id} paste`, `${fmt(w.n.fill!.mpa, 2)} MPa`, "measured")).join("")}
${sides.length ? row("Design for a 25 m wall", `${fmt(PASTE.design, 2)} MPa`, "estimated") : ""}
${st.state === "drilled" ? row("Blast plan", plan, weak ? "refused" : undefined, weak ? "REF-504" : undefined) : ""}</dl>
${weak ? cureChart(weak.n.fill!.days) : ""}${weak ? `<p class="ug-src">FMINE-REF-504: a secondary stope’s blast plan is refused while adjacent paste is uncured or under design strength. ${weak.n.id} is at ${fmt(weak.n.fill!.mpa, 2)} of ${fmt(PASTE.design, 2)} MPa. The rule and its code are from FrankenMining’s refusal catalogue; the page applies it to the synthetic cylinder breaks. The agent’s draft re-sequences the week.</p>` : st.state === "drilled" ? `<p class="ug-src">${sides.length ? "Its fill walls are over design strength, so the blast plan passes the backfill check." : "A primary between unmined stopes: no fill walls exposed."}</p>` : ""}`;
  };
  const supportCard = (id: string | null) => {
    const counts = { verified: 0, due: 0, overdue: 0 };
    for (const h of HEADINGS) counts[h.status]++;
    const h = id ? HEADINGS.find((x) => `head:${x.id}` === id) : null;
    const detail = h
      ? `<div class="ug-detail"><p><b>${escape(h.name)}</b> ${ev(SUPPORT_EV[h.status], SUPPORT_TEXT[h.status])}</p><dl>${row("Support installed", escape(h.installed))}${row("Last verified", escape(h.checked))}${row("By", escape(h.by))}</dl><p class="ug-src">${escape(h.note)}</p></div>`
      : "";
    return `<p class="micro">Ground support · a verified critical control</p>
<p class="ug-sum"><span class="dot v"></span><b>${counts.verified}</b> verified <span class="dot d"></span><b>${counts.due}</b> due <span class="dot o"></span><b>${counts.overdue}</b> overdue</p>
${detail}<ul class="ug-list">${HEADINGS.map((x) => `<li><button type="button" data-pick="head:${x.id}" aria-pressed="${id === `head:${x.id}`}"><span class="dot ${x.status[0]}"></span><span>${escape(x.name)}</span><small>${escape(x.checked)}</small></button></li>`).join("")}</ul>
<p class="micro ug-sub">Ground movement · rate against the trigger</p>
<ul class="ug-inst">${INSTRUMENTS.map((i) => `<li title="${escape(i.where)} · ${fmt(i.total, 1)} mm since installed"><code>${i.id}</code><span>${escape(i.where.split(",")[0]!)}</span><b class="${i.rate > i.trigger ? "hot" : ""}">${fmt(i.rate, 1)}</b><small>of ${fmt(i.trigger, 1)} mm/day</small></li>`).join("")}</ul>
<p class="ug-src">Each verification is a record: who, when, where, the answers and photos. Rates are measured; triggers are the geotechnical engineer's. The state does not rate safety.</p>`;
  };
  const ages = new Map<string, number>();
  for (const c of CREWS) for (const p of c.people) ages.set(p.tag, p.read);
  const age = (sec: number) => (sec < 90 ? `${Math.round(sec)} s ago` : `${Math.round(sec / 60)} min ago`);
  const peopleCard = (id: string | null) => {
    const crew = id?.startsWith("crew:") ? CREWS.find((c) => `crew:${c.id}` === id) : null;
    const detail = crew
      ? `<div class="ug-detail"><p><b>${escape(crew.name)}</b> · ${escape(crew.place)}</p><ul class="ug-tags">${crew.people
          .map((p) => `<li><span>${escape(p.job)}</span><code>${p.tag}</code><small data-age="${p.tag}">${age(ages.get(p.tag)!)}</small></li>`)
          .join("")}</ul></div>`
      : id === "refuge"
        ? `<div class="ug-detail"><p><b>Refuge chamber RC-125</b> · 125 level</p><dl>${row("Inside now", "0 of 16", "measured")}${row("Oxygen and scrubbers", "36 h supply", "measured")}${row("Last inspection", "22 Nov", "measured")}${row("Nearest crews", "Paste fill, Electrical")}</dl></div>`
        : "";
    return `<p class="micro">People underground · tags against the shift register</p>
<p class="ug-big"><b>${TAGS} of ${REGISTER}</b> accounted for ${ev("measured")}</p>
${detail}<ul class="ug-list">${CREWS.map(
      (c) =>
        `<li><button type="button" data-pick="crew:${c.id}" aria-pressed="${id === `crew:${c.id}`}"><span class="dot v"></span><span>${escape(c.name)} · ${escape(c.place)}</span><small>${c.people.length}</small></button></li>`,
    ).join("")}<li><button type="button" data-pick="refuge" aria-pressed="${id === "refuge"}"><span class="dot r"></span><span>Refuge RC-125 · 125 level</span><small>0 / 16</small></button></li></ul>
<p class="ug-src">Every tag read by the level readers in the last two minutes, checked against who badged in for the shift.</p>`;
  };
  const legends: Record<Layer, string> = {
    mine: `<ul><li><i class="sw ore"></i>Ore</li><li><i class="sw paste"></i>Paste fill</li><li><i class="sw open"></i>Open stope</li><li><i class="sw ring"></i>Drill rings</li><li><i class="sw plan"></i>Planned stope</li></ul>`,
    stopes: `<ul><li><i class="sw over"></i>Overbreak</li><li><i class="sw on"></i>On design</li><li><i class="sw under"></i>Underbreak</li><li><i class="sw ring"></i>Drilled</li><li><i class="sw wet"></i>Paste curing</li></ul>`,
    support: `<ul><li><i class="sw v"></i>Verified</li><li><i class="sw d"></i>Due</li><li><i class="sw o"></i>Overdue</li></ul>`,
    people: `<ul><li><i class="sw tagread"></i>Tag read</li><li><i class="sw refuge"></i>Refuge</li></ul>`,
  };
  const paintCard = () => {
    const id = sel[layer];
    card.innerHTML =
      layer === "mine" ? mineCard() : layer === "stopes" ? stopeCard((id ?? `stope:${FEATURE.mucking}`).replace("stope:", "")) : layer === "support" ? supportCard(id) : peopleCard(id);
    legend.innerHTML = legends[layer];
    for (const b of $$<HTMLButtonElement>(card, "[data-pick]")) b.addEventListener("click", () => pick(b.dataset.pick!));
    live();
  };
  const live = () => {
    for (const el of $$(host, '[data-live="mucked"]')) el.textContent = fmt(tally.tonnes);
    for (const el of $$(host, '[data-live="stope"]')) el.textContent = fmt(tally.stope);
    for (const el of $$(host, '[data-live="left"]')) el.textContent = t0(Math.max(0, scan.surveyed - tally.stope));
    const [ratio, ratioSub] = monitorText();
    for (const el of $$(host, '[data-live="ratio"]')) el.innerHTML = ratio;
    for (const el of $$(host, '[data-live="ratio-sub"]')) el.textContent = ratioSub;
    for (const el of $$<HTMLElement>(host, "[data-age]")) el.textContent = age(ages.get(el.dataset.age!) ?? 0);
    $(host, '[data-read="mucked"] b').textContent = `${fmt(tally.tonnes)} t`;
  };
  const showPins = () => {
    for (const p of pins) {
      const on = p.layer === "all" || p.layer === layer;
      p.label.show(on);
      p.label.el.classList.toggle("on", !!p.id && (sel[layer] === p.id || focus.includes(p.id)));
    }
  };

  const layerSeg = seg(host, "layer", (v) => setLayer(v as Layer));
  const twinBtn = $<HTMLButtonElement>(host, "[data-twin]");
  const setTwin = (v: boolean) => {
    if (v === twin.on) return;
    twin.set(v);
    twinBtn.setAttribute("aria-pressed", String(v));
    view.classList.toggle("ug-twin-on", v);
    go(v ? twin.view() : VIEWS[viewSeg.value || "whole"]!);
    showPins();
    s.loop.poke();
  };
  twinBtn.addEventListener("click", () => setTwin(!twin.on));
  const viewSeg = seg(host, "view", (v) => {
    focus = [];
    if (twin.on) setTwin(false);
    go(VIEWS[v]!);
    showPins();
  });
  const setLayer = (l: Layer) => {
    layer = l;
    layerSeg.set(l);
    paintCard();
    showPins();
    s.loop.poke();
  };
  /** Selecting something: switch to its layer, open its card and fly to it. */
  function pick(id: string) {
    viewSeg.set("");
    if (id === "lhd" || id === "crew:prod") {
      if (layer === "people") sel.people = "crew:prod";
      else {
        sel.stopes = `stope:${FEATURE.mucking}`;
        if (layer !== "stopes") setLayer("stopes");
      }
      go(VIEWS.muck!);
    } else if (id === "jumbo" || id === "crew:dev") {
      sel.support = "head:H-175E";
      if (layer === "people") sel.people = "crew:dev";
      else if (layer !== "support") setLayer("support");
      go(VIEWS.face!);
    } else if (id === "refuge") {
      sel.people = "refuge";
      if (layer !== "people") setLayer("people");
      go(VIEWS.refuge!);
    } else if (id.startsWith("crew:")) {
      sel.people = id;
      if (layer !== "people") setLayer("people");
      const crewId = id.slice(5);
      const v =
        crewId === "geo" ? VIEWS.decline : crewId === "survey" ? VIEWS.survey : crewId === "paste" ? VIEWS.paste : crewId === "elec" ? VIEWS.refuge : crewId === "super" ? VIEWS.muck : null;
      const a = crewAnchor.get(crewId);
      if (v) go(v);
      else if (a) go({ target: a.getWorldPosition(new THREE.Vector3()).toArray() as [number, number, number], distance: small ? 60 : 40, azimuth: 30, elevation: 24 });
    } else if (id.startsWith("stope:")) {
      sel.stopes = id;
      if (layer !== "stopes") setLayer("stopes");
      const st = stope(id.slice(6));
      const x = (st.xa + st.xb) / 2;
      const y = st.level + 12.5;
      go({ target: [x, y, (zFW(x, y) + zHW(x, y)) / 2], distance: small ? 150 : 100, azimuth: 16, elevation: 12 });
    } else if (id.startsWith("head:")) {
      sel.support = id;
      if (layer !== "support") setLayer("support");
      const h = HEADINGS.find((x) => `head:${x.id}` === id)!;
      const p = headingAt(h);
      go({ target: [p.x, p.y + 2.5, p.z], distance: small ? 70 : 46, azimuth: h.at === "l175" ? -52 : h.at === "decline" ? 60 : 24, elevation: 22 });
    }
    focus = [];
    paintCard();
    showPins();
  }
  function goTo(which: string) {
    if (twin.on) setTwin(false);
    sel.stopes = `stope:${which === "refusal" ? FEATURE.refused : FEATURE.draft}`;
    if (layer !== "stopes") setLayer("stopes");
    focus = which === "refusal" ? [`stope:${FEATURE.refused}`, `stope:${FEATURE.curing}`] : [`stope:${FEATURE.draft}`, "stope:P125-01", "stope:P125-03"];
    viewSeg.set("");
    go(VIEWS[which]!);
    paintCard();
    showPins();
  }
  paintAgent();

  // Phones: the claims card and the agent's note go under the scene.
  const narrow = matchMedia("(max-width: 699px)");
  const place = () => {
    if (narrow.matches) {
      below.append(card, agent);
      card.classList.remove("hud-card", "hud-tl");
      agent.classList.remove("hud-card", "hud-br");
    } else {
      view.append(card, agent);
      card.classList.add("hud-card", "hud-tl");
      agent.classList.add("hud-card", "hud-br");
    }
  };
  narrow.addEventListener("change", place);
  place();

  /* ---------- Picking in 3D ---------- */
  const ray = new THREE.Raycaster();
  const targets: THREE.Object3D[] = [];
  const proxy = (id: string, size: [number, number, number], parent: THREE.Object3D, at = V(0, 0, 0)) => {
    const m = new THREE.Mesh(new THREE.BoxGeometry(...size), new THREE.MeshBasicMaterial());
    m.visible = false;
    m.position.copy(at);
    m.userData.pick = id;
    parent.add(m);
    targets.push(m);
  };
  proxy("lhd", [4, 4, 12], lhd.group, V(0, 2, 0));
  proxy("jumbo", [4, 4, 14], jumbo.group, V(0, 2, 0));
  proxy("refuge", [4, 4, 8], world.refuge.group, V(0, 2, 0));
  for (const crew of CREWS) {
    const a = crewAnchor.get(crew.id);
    if (a && crew.id !== "prod" && crew.id !== "dev") proxy(`crew:${crew.id}`, [5, 4, 5], a, V(0, 1.6, 0));
  }
  for (const v of world.stopes.values()) {
    v.design.userData.pick = `stope:${v.stope.id}`;
    targets.push(v.design);
  }
  let down: { x: number; y: number } | null = null;
  s.canvas.addEventListener("pointerdown", (e) => (down = { x: e.clientX, y: e.clientY }));
  s.canvas.addEventListener("pointerup", (e) => {
    if (!down || Math.hypot(e.clientX - down.x, e.clientY - down.y) > 6) return;
    const r = s.canvas.getBoundingClientRect();
    ray.setFromCamera(new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1), s.camera);
    const hit = ray.intersectObjects(targets, false).find((h) => {
      const id = h.object.userData.pick as string;
      return !id.startsWith("stope:") || layer === "stopes";
    });
    if (hit) pick(hit.object.userData.pick as string);
  });

  /* ---------- Layers, animated ---------- */
  const want = { support: 0, heat: 0, lens: 1, shell: 1, edges: 0.3, rings: 0.4, tags: 0, slab: 1 };
  const now = { ...want };
  const coral = new THREE.Color("#fb7185");
  const green = new THREE.Color("#8be27a");
  const amber = new THREE.Color("#ffb347");
  const ivory = new THREE.Color("#f3dca6");
  const paint = (t: number, dt: number) => {
    want.support = layer === "support" ? 1 : 0;
    want.heat = layer === "stopes" ? 1 : 0;
    want.lens = layer === "mine" ? 1 : layer === "stopes" ? 0.7 : 0.32;
    want.shell = layer === "stopes" ? 0.65 : layer === "people" ? 0.8 : 1;
    // Outside the stopes layer the outlines and ring fans give way in a close-up, where they would cross the rock.
    const far = layer === "stopes" ? 1 : smooth((s.orbit.distance - 45) / 140);
    want.edges = (layer === "stopes" ? 0.75 : layer === "mine" ? 0.3 : 0.12) * far;
    want.rings = (layer === "stopes" ? 0.9 : layer === "mine" ? 0.45 : 0.15) * mix(0.35, 1, far);
    want.tags = layer === "people" ? 1 : 0;
    const k = 1 - Math.exp(-dt * 5);
    for (const key of Object.keys(want) as (keyof typeof want)[]) now[key] = mix(now[key], want[key], reduced ? 1 : k);
    world.uniforms.uSupport.value = now.support;
    world.uniforms.uTime.value = t;
    world.heat.value = now.heat;
    world.lensMat.uniforms.uAlpha!.value = now.lens;
    world.shellMat.uniforms.uOpacity!.value = now.shell;
    const pulse = 0.5 + 0.5 * Math.sin(t * 3);
    const selected = sel.stopes?.replace("stope:", "");
    for (const v of world.stopes.values()) {
      const st = v.stope;
      const id = `stope:${st.id}`;
      const lit = layer === "stopes" && (focus.includes(id) || (!focus.length && selected === st.id));
      (v.edges.material as THREE.LineBasicMaterial).opacity = now.edges * (lit ? 1.4 : st.state === "planned" ? 0.8 : 1);
      if (v.rings) {
        const m = v.rings.material as THREE.LineBasicMaterial;
        m.opacity = now.rings;
        m.color.copy(st.id === FEATURE.refused && layer === "stopes" ? coral : st.id === FEATURE.draft && focus.length && layer === "stopes" ? green : amber);
      }
      const dm = v.design.material as THREE.MeshBasicMaterial;
      let o = 0;
      if (layer === "stopes") {
        if (st.state === "planned") o = 0.03;
        if (st.state === "drilled") o = 0.07;
        if (lit) o = 0.1 + 0.12 * pulse;
      }
      dm.opacity = mix(dm.opacity, o, k);
      dm.color.copy(st.id === FEATURE.refused || st.id === FEATURE.curing ? coral : st.id === FEATURE.draft || (focus.length && focus.includes(id)) ? green : st.state === "drilled" ? amber : ivory);
      dm.visible = dm.opacity > 0.004;
      if (v.paste && st.state === "curing") {
        const pm = v.paste.material as THREE.MeshStandardMaterial;
        pm.emissive.copy(coral);
        pm.emissiveIntensity = now.heat * (0.18 + 0.2 * pulse);
      }
    }
    for (const m of markers) {
      (m.material as THREE.SpriteMaterial).opacity = now.tags * (0.75 + 0.25 * pulse);
      m.visible = now.tags > 0.02;
    }
    world.refuge.strobe.material.opacity = 0.5 + 0.5 * Math.max(0, Math.sin(t * 5));
    faceGlow.intensity = 30 + 8 * Math.sin(t * 13) * Math.sin(t * 7.3);
  };

  /* ---------- Frame ---------- */
  let since = 0;
  let underClock = 0;
  /**
   * Labels may use the whole view, but one that would sit under a HUD card, or over a label that ranks above it, steps
   * back until there is room (the selected label ranks first).
   */
  const declutter = () => {
    const panels = [card, agent, legend].filter((p) => p.parentElement === view && p.offsetParent);
    const taken = panels.map((p) => p.getBoundingClientRect());
    const cards = taken.length;
    const rank = (p: Pin) => (p.id && (sel[layer] === p.id || focus.includes(p.id)) ? -1 : p.prio);
    const vr = view.getBoundingClientRect();
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
    for (const p of [...pins].sort((a, b) => rank(a) - rank(b))) {
      const el = p.label.el;
      if (el.classList.contains("off") || el.style.visibility === "hidden") continue;
      // The stage hangs labels to the left of their point only near the right edge; a long label that would still
      // run off the view hangs left here too.
      const shifted = el.classList.contains("ug-flip");
      let r = el.getBoundingClientRect();
      const rightIfUnshifted = shifted ? r.left + r.width * 2 - 1.36 * rem : r.right;
      const wantFlip = !el.classList.contains("flip") && rightIfUnshifted > vr.right - 4;
      if (wantFlip !== shifted) {
        el.classList.toggle("ug-flip", wantFlip);
        r = el.getBoundingClientRect();
      }
      const hit = taken.some((c, i) => {
        const pad = i < cards ? 6 : 2;
        return r.right > c.left - pad && r.left < c.right + pad && r.bottom > c.top - pad && r.top < c.bottom + pad;
      });
      el.classList.toggle("ug-under", hit);
      if (!hit) taken.push(r);
    }
  };
  // The surface never moves: draw the sun's shadow once.
  s.sun.shadow.autoUpdate = false;
  s.sun.shadow.needsUpdate = true;
  const camAt = new THREE.Vector3();
  const nearAt = new THREE.Vector3();
  const capLamps = placed.map((p) => p.lamp);
  s.frame((dt, t) => {
    const step = Math.min(dt, 0.05);
    lhdStep(step);
    jumboStep(step);
    camAt.copy(s.camera.position);
    world.tick(step, t, camAt, capLamps);
    // The near plane as far out as the view allows: depth precision for the cutaway's thin films and the services.
    const near = clamp(s.orbit.distance * 0.008, 0.15, 4);
    if (Math.abs(near - s.camera.near) > near * 0.05) {
      s.camera.near = near;
      s.camera.updateProjectionMatrix();
    }
    if (twin.on) twin.update(step, t);
    for (const sh of shadowed) {
      if (!sh.light.shadow) continue;
      sh.light.shadow.autoUpdate = sh.near.getWorldPosition(nearAt).distanceTo(camAt) < 160;
    }
    // From far off the machines draw as merged stand-ins and the crews (a few pixels each) only as their cap lamps.
    const far = s.orbit.distance > 240;
    lhdFar(far);
    jumboFar(far);
    for (const p of placed) {
      p.miner.group.visible = !far;
      if (!far) p.miner.update(step);
    }
    world.surface.update(step, t);
    paint(t, step);
    since += step;
    underClock += dt;
    if (underClock > 0.12) {
      underClock = 0;
      declutter();
    }
    if (since > 0.5) {
      since = 0;
      // Tags are read every minute or so: ages climb, then reset as a reader sees them again.
      for (const [tag, a] of ages) ages.set(tag, a + 0.5 > 70 + (tag.charCodeAt(4) % 30) ? 3 : a + 0.5);
      live();
    }
  });
  paintCard();
  showPins();
  s.start();
  if (reduced) s.render();
  // FrankenMining's engine, in WebAssembly: the stope grades from the drill composites, then the drift monitor over
  // every bucket since P100-03 was fired. Each runs in its own frame; the HUD repaints when they answer.
  void (async () => {
    try {
      await step();
      const samples = drillComposites();
      engine.grades = await stopeGrades(samples, step);
      for (const [id, g] of engine.grades.byStope) grades.set(id, { mean: g.mean, lo: g.lo, hi: g.hi });
      twin.refresh();
      paintCard();
      paintAgent();
      await step();
      engine.watch = await muckWatch(samples, stope(FEATURE.mucking), scan.over / scan.surveyed, Math.round(tally.stope / PAYLOAD), step);
      live();
    } catch (e) {
      engine.error = e instanceof EngineError ? (e.code ?? e.message) : String((e as Error)?.message ?? e);
      paintCard();
      paintAgent();
    }
  })();
  s.firstFrame(() => (fig.dataset.live = ""));
}
