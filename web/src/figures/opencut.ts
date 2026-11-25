/**
 * A working open pit: the terrain, a rope shovel digging a muck pile on the floor, haul trucks that queue, turn and
 * reverse in to be loaded, climb the ramp and tip at the crusher or the waste dump, a drill on a bench, the crusher's
 * conveyor out to a stockpile, light towers, a pickup, and a few people. It runs itself; scenes add cameras, labels
 * and words.
 *
 * The simulation keeps what a fleet system and a payload monitor would record: every load's face, passes, weighed
 * payload, grade estimate, destination and times (its chain of custody), a ledger of the loads delivered, and a
 * minute-by-minute record of the time trucks spent queued and loaders spent waiting. Trucks drive in lanes, keep
 * their distance and queue nose to tail. With `second`, a hydraulic face shovel (L2) works a waste face on the same
 * floor and each truck belongs to one loader; `assign` moves a truck between them, as a dispatcher would.
 */
import * as THREE from "three";
import {
  HAUL,
  makeConveyor,
  makeCrusher,
  makeDrillRig,
  makeFaceShovel,
  makeHaulTruck,
  makeLightTower,
  makeMuckPile,
  makePickup,
  makeRopeShovel,
  makeStockpile,
  makeTrailingCable,
  type FaceShovel,
  type HaulTruck,
  type RopeShovel,
} from "./fleet";
import { clamp, finish, mix, pace, rng, smooth } from "./lib";
import { makeMiner, type Miner } from "./miners";
import { pitSteps, type Pit } from "./terrain";

/* ---------- Routes ---------- */
type Kind = "out" | "up" | "road" | "pad" | "platform" | "down" | "in" | "spot" | "tip" | "transfer";

interface Seg {
  pts: THREE.Vector3[];
  /** Cumulative length at each point. */
  len: number[];
  /** +1 drives forward along the points, −1 reverses along them. */
  dir: 1 | -1;
  speed: number;
  /** Speed with a load on, where it differs. */
  loadedSpeed?: number;
  kind: Kind;
  /** A resource the truck must hold before starting this segment, and releases after `release`. */
  claim?: string;
  /** What happens at the end. */
  then?: "load" | "dump";
  release?: string;
  /** Use the points' own heights instead of the ground's. */
  fixedY?: boolean;
  /** Drive this far to the left of the centreline, easing in over `a` metres at the start and out over `b` at the end. */
  lane?: { off: number; a: number; b: number };
  /** The loader an approach or a spotting run leads to. */
  loader?: string;
}

const seg = (pts: THREE.Vector3[], dir: 1 | -1, speed: number, kind: Kind, extra: Partial<Seg> = {}): Seg => {
  const len = [0];
  for (let i = 1; i < pts.length; i++) len.push(len[i - 1]! + pts[i]!.distanceTo(pts[i - 1]!));
  return { pts, len, dir, speed, kind, ...extra };
};
const bez = (a: THREE.Vector3, c: THREE.Vector3, b: THREE.Vector3, step = 1.5) =>
  new THREE.QuadraticBezierCurve3(a, c, b).getSpacedPoints(Math.max(4, Math.ceil(a.distanceTo(b) / step)));
const cubic = (a: THREE.Vector3, c1: THREE.Vector3, c2: THREE.Vector3, b: THREE.Vector3, step = 1.5) =>
  new THREE.CubicBezierCurve3(a, c1, c2, b).getSpacedPoints(Math.max(6, Math.ceil(a.distanceTo(b) / step)));
const lengthOf = (s: Seg) => s.len[s.len.length - 1]!;

/** Point and level tangent at distance `d` along a segment (binary search on the cumulative lengths). */
function at(s: Seg, d: number, out: THREE.Vector3, tan: THREE.Vector3) {
  const L = lengthOf(s);
  const x = clamp(d, 0, L);
  let lo = 0;
  let hi = s.len.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (s.len[mid]! < x) lo = mid;
    else hi = mid;
  }
  const a = s.pts[lo]!;
  const b = s.pts[hi]!;
  const span = s.len[hi]! - s.len[lo]! || 1;
  out.lerpVectors(a, b, (x - s.len[lo]!) / span);
  tan.subVectors(b, a).setY(0).normalize();
  return L;
}
/** Left of a level heading (the truck's left is +x when it faces +z). */
const leftOf = (h: THREE.Vector3, out = new THREE.Vector3()) => out.set(h.z, 0, -h.x);

/* ---------- What the mine records ---------- */
/** One load's chain of custody, as the fleet system and the truck's payload monitor record it. Times are mine seconds. */
export interface Custody {
  /** Load number, in the order loads began. */
  n: number;
  truck: string;
  loader: string;
  face: string;
  material: "ore" | "waste";
  /** "crusher" or "dump". */
  dest: "crusher" | "dump";
  /** The grade-control block the load came from. */
  block: string;
  /** Grade estimate from the grade-control model, g/t, with one standard deviation. */
  grade: { mean: number; sd: number };
  /** Each pass: when it landed and the tonnes the loader's own scale put in. */
  passes: { t: number; tonnes: number }[];
  /** Weighed payload from the truck's payload monitor, tonnes (0 until the truck leaves the loader). */
  payloadT: number;
  /** Joined the queue (or arrived, if there was none), began reversing in, first and last pass, left, tipped. */
  tArrive: number;
  tSpot: number;
  tFirst: number | null;
  /** When the truck stopped under the loader, spotted. */
  tUnder?: number | null;
  tLoaded: number | null;
  tDepart: number | null;
  tTip: number | null;
  /** Seconds it waited in the queue. */
  queued: number;
}

/** A minute of the shift: truck-seconds queued and seconds idle at each loader, tonnes loaded, tonnes delivered. */
export interface Minute {
  t: number;
  queue: Record<string, number>;
  idle: Record<string, number>;
  loaded: Record<string, number>;
  ore: number;
  waste: number;
}

export interface MineEvent {
  type: "pass" | "spot" | "depart" | "tip" | "assign";
  truck: Truck;
  loader?: string;
  /** Where it happened, in the mine's frame. */
  at: THREE.Vector3;
}

export interface Truck {
  id: string;
  model: HaulTruck;
  route: "ore" | "waste";
  /** Where it is in its cycle, in words. */
  doing: string;
  payloadT: number;
  loads: number;
  /** Seconds waiting in the current queue. */
  waited: number;
  /** The loader it works for. */
  loader: string;
  /** A move to another loader that applies when the truck next returns empty. */
  pending: string | null;
  /** The load it is taking on or carrying, with its chain of custody; null when empty. */
  load: Custody | null;
  /** The last load it delivered. */
  last: Custody | null;
  /** Ground speed, m/s (negative when reversing). */
  speed: number;
  /** "drive", "queue" (stopped behind another truck or for a loader, crusher or dump), "spot", "load", "dump". */
  act: "drive" | "queue" | "spot" | "load" | "dump";
  /** What kind of road it is on. */
  stretch: Kind;
}

export interface Loader {
  id: string;
  kind: "rope" | "face";
  group: THREE.Group;
  face: string;
  material: "ore" | "waste";
  dest: "crusher" | "dump";
  passes: number;
  /** Tonnes a pass, low and high. */
  passT: [number, number];
  /** Mine seconds a pass takes. */
  passTime: number;
  /** Dipper or bucket, when passes follow from it: heaped volume, loose density and fill factor range. */
  bucket?: { m3: number; density: number; fill: [number, number] };
  /** Passes planned for a truck before topping up or stopping short, and the tonnes a top-up pass is after. */
  plan?: number;
  topUp?: number;
  /** Where a truck stands to be loaded, and where the first truck in the queue waits. */
  spot: THREE.Vector3;
  queueAt: THREE.Vector3;
  /** The truck under it now, if any. */
  loading: Truck | null;
  /** Trucks waiting for it, next first. */
  queue: Truck[];
  /** Seconds since the start spent waiting for trucks, and seconds loading or spotting them. */
  idle: number;
  busy: number;
  /** Seconds the present wait for a truck has lasted (0 while loading). */
  waiting: number;
  /** Passes into the truck under it. */
  passesDone: number;
  /** Where the bucket or dipper empties into a truck. */
  dumpPoint: THREE.Vector3;
  muck: THREE.Object3D;
}

export interface Mine {
  group: THREE.Group;
  pit: Pit;
  trucks: Truck[];
  shovel: ReturnType<typeof makeRopeShovel>;
  drill: ReturnType<typeof makeDrillRig>;
  crusher: ReturnType<typeof makeCrusher>;
  stockpile: THREE.Object3D;
  muck: THREE.Object3D;
  dumpPoint: THREE.Vector3;
  people: Miner[];
  /** Totals since the start, in tonnes, and loads delivered. */
  tally: { ore: number; waste: number; loads: number; queue: number };
  update(dt: number): void;
  lights(on: boolean): void;
  /** Mine seconds since the start. */
  readonly time: number;
  /** Mine seconds per second of real time. */
  pace: number;
  loaders: Loader[];
  /** Every load delivered, oldest first. */
  ledger: Custody[];
  /** One row per minute of mine time. */
  minutes: Minute[];
  /** The floor roads between the ramp and the loaders, for scenes that draw them. */
  floorRoads: THREE.Vector3[][];
  /** The light towers. */
  towers: THREE.Object3D[];
  /** The haul roads with their grades and the speeds the truck physics gives on them. */
  roads: Road[];
  /** Moves a truck to another loader (a dispatch change). A truck queued or on its way in turns off at once. */
  assign(truck: string, loader: string): void;
  /** The truck a ray hits, if any. */
  pick(ray: THREE.Ray): Truck | null;
  /** Runs the mine forward without drawing every step (for a warm start); poses everything at the end. */
  advance(seconds: number): void;
  /** Listens for passes, spotting, departures, tips and dispatch changes. */
  on(fn: (e: MineEvent) => void): void;
  /** Mine seconds of a cycle on a loader's route with no waiting: driving, spotting, loading and tipping. */
  cycle(loader: string): number;
  /** Mine seconds until a truck tips (with a load on or being loaded) or reaches its loader (empty), with no waiting. */
  eta(truck: string): number | null;
}

export interface MineOptions {
  trucks?: number;
  cells?: number;
  people?: boolean;
  seed?: number;
  /** Seconds of mine time per second of real time. */
  pace?: number;
  /** A second loading unit: a hydraulic face shovel (L2) on a waste face. Each truck then belongs to one loader. */
  second?: boolean;
  /** Trucks on each loader when `second` is set. */
  fleet?: { S1: number; L2: number };
  /** The pickup that patrols the surface roads (default true). */
  pickup?: boolean;
  /** Mine seconds a rope-shovel pass takes (default 26). */
  passTime?: number;
  /** Passes the rope shovel takes to fill a truck (default 3). */
  passes?: number;
  /**
   * The rope shovel's dipper, m³. When set, a pass weighs dipper × loose density × fill (random), the shovel plans
   * the passes a rated load needs and tops up or stops short by the loader scale, so trucks take 4 or 5; L2's
   * bucket is 38 m³. Without it every truck takes `passes`.
   */
  dipper?: number;
}

const GAP = 21; // metres nose to tail in a queue
/**
 * The haul truck as the speed model sees it: the ultra-class, 360 t mechanical-drive truck that fleet-truck.ts draws
 * (15.1 m long, 9.75 m wide, 4.03 m tyres). Typical figures for the class, not one maker's data sheet.
 */
export const TRUCK = {
  /** Empty operating mass and rated payload, t. */
  emptyT: 260,
  ratedT: 363,
  /** Power reaching the wheels: about 2,800 kW net at 82% through the drivetrain. */
  wheelKW: 2300,
  /** Continuous retarding power, kW. */
  retardKW: 2600,
  /** Rimpull ceiling set by traction, kN. */
  tractionKN: 1550,
  /** Rolling resistance of a well-kept haul road, as a fraction of weight. */
  rr: 0.025,
  /** Sideways acceleration allowed in bends, m/s². */
  lat: 1.2,
  /** Service braking used to plan stops, m/s². */
  brake: 0.8,
  /** Strut travel a rated load takes up, m. */
  squat: 0.14,
} as const;
/** Site speed limits by kind of road, km/h. */
export const LIMITS: Record<string, number> = { out: 18, in: 22, transfer: 15, up: 35, down: 35, road: 50, pad: 20, platform: 10, spot: 7, tip: 7 };
const G = 9.81;
/** How far ahead (m) a driver looks for a bend, a limit or a stop. */
const AHEAD = [0, 8, 20, 40, 70];

/** A stretch of haul road as the speed model reads it, for scenes that draw the network. */
export interface Road {
  name: string;
  kind: string;
  pts: THREE.Vector3[];
  length: number;
  /** Rise over run from end to end, in the direction trucks drive it. */
  grade: number;
  /** Steepest grade along it. */
  steepest: number;
  /** Steady speeds, km/h, empty and with a rated load, and what sets each. */
  vEmpty: number;
  vLoaded: number;
  govEmpty: string;
  govLoaded: string;
  /** Whether trucks drive it loaded. */
  loaded: boolean;
}
const TIP = 24; // seconds to raise, empty and lower the body
/** Two-way roads keep right: a lane is this far to the left of the centreline (negative is right). */
const LANE = -6.5;

export function buildMine(o: MineOptions = {}): Mine {
  return finish(mineSteps(o));
}

/** The mine a piece at a time, pausing for the page between pieces: the same mine as `buildMine`. */
export function buildMineAsync(o: MineOptions = {}): Promise<Mine> {
  return pace(mineSteps(o));
}

function* mineSteps(o: MineOptions = {}): Generator<void, Mine> {
  const r = rng(o.seed ?? 11);
  const pit = yield* pitSteps({ cells: o.cells ?? 512, seed: o.seed ?? 7 });
  yield;
  const group = new THREE.Group();
  group.add(pit.group);
  const H = pit.heightAt;
  const ground = (p: THREE.Vector3) => p.setY(H(p.x, p.z));
  const second = !!o.second;

  /* ---------- The loading area on the floor ---------- */
  const rampEnd = pit.ramp[pit.ramp.length - 1]!.clone();
  const inward = new THREE.Vector3(-rampEnd.x, 0, -rampEnd.z).normalize();
  const side = new THREE.Vector3(-inward.z, 0, inward.x);
  // The shovel faces a muck pile against the far part of the floor.
  const shovelAt = ground(rampEnd.clone().addScaledVector(inward, 95).addScaledVector(side, -10));
  const faceDir = inward.clone();
  const shovel = makeRopeShovel({ seed: 3 });
  shovel.group.position.copy(shovelAt);
  shovel.group.rotation.y = Math.atan2(faceDir.x, faceDir.z);
  group.add(shovel.group);
  const muckPile = makeMuckPile({ length: 70, width: 30, height: 9, palette: "ore", seed: 5 });
  muckPile.group.position.copy(shovelAt).addScaledVector(faceDir, 26);
  muckPile.group.position.y = H(muckPile.group.position.x, muckPile.group.position.z) - 0.4;
  muckPile.group.rotation.y = Math.atan2(faceDir.x, faceDir.z) + Math.PI;
  group.add(muckPile.group);
  const cable = makeTrailingCable({ from: shovelAt.clone().addScaledVector(faceDir, -6).setY(shovelAt.y + 0.3), to: ground(shovelAt.clone().addScaledVector(side, 55).addScaledVector(faceDir, -40)).setY(shovelAt.y + 0.2), slack: 0.15, seed: 2 });
  group.add(cable.group);

  yield;
  /** A loader whose passes follow from its bucket: tonnes a pass, and the passes a rated load needs. */
  const bucketed = (b: { m3: number; density: number; fill: [number, number] }) => {
    const passT: [number, number] = [b.m3 * b.density * b.fill[0], b.m3 * b.density * b.fill[1]];
    const plan = Math.round(TRUCK.ratedT / ((passT[0] + passT[1]) / 2));
    return { bucket: b, passT, plan, passes: plan };
  };
  // Where the dipper dumps when the house swings a quarter turn to the right.
  const ROPE_SWING = -Math.PI / 2;
  shovel.set({ swing: ROPE_SWING, crowd: 0.55, hoist: 1, door: 0 });
  shovel.group.updateMatrixWorld(true);
  const dipper = new THREE.Vector3();
  shovel.anchors.dipper.getWorldPosition(dipper);
  shovel.set({ swing: 0, crowd: 0.4, hoist: 0.2 });

  /* ---------- The crusher on the run-of-mine pad ---------- */
  const crusher = makeCrusher({ platform: 7, seed: 4 });
  const padYaw = Math.atan2(pit.pad.x - pit.toPad[pit.toPad.length - 8]!.x, pit.pad.z - pit.toPad[pit.toPad.length - 8]!.z);
  crusher.group.position.copy(pit.pad).addScaledVector(new THREE.Vector3(Math.sin(padYaw), 0, Math.cos(padYaw)), 72);
  crusher.group.position.y = pit.pad.y;
  crusher.group.rotation.y = padYaw;
  group.add(crusher.group);
  crusher.group.updateMatrixWorld(true);
  const local = (x: number, y: number, z: number) => new THREE.Vector3(x, y, z).applyMatrix4(crusher.group.matrixWorld);
  const anchorT = new THREE.Vector3();
  crusher.anchors.truck.getWorldPosition(anchorT);
  const anchorLocal = crusher.group.worldToLocal(anchorT.clone());
  const P = 7;
  const rampTopZ = anchorLocal.z - 24;
  const R = 11;
  const rampFoot = local(0, 0, rampTopZ - 70);
  const rampTop = local(0, P, rampTopZ);
  const kTurn = local(R, P, anchorLocal.z - R);
  const crushIn = [
    ...bez(rampFoot, rampFoot.clone().lerp(rampTop, 0.5), rampTop),
    ...bez(rampTop, local(0, P, anchorLocal.z - R), kTurn).slice(1),
  ];
  const crushBack = bez(kTurn, local(anchorLocal.x, P, anchorLocal.z - R), local(anchorLocal.x, P, anchorLocal.z));
  const crushOut = [...bez(local(0, P, anchorLocal.z), local(0, P, rampTopZ + 4), rampTop), ...bez(rampTop, rampTop.clone().lerp(rampFoot, 0.5), rampFoot).slice(1)];
  const crushTip = local(anchorLocal.x, P + 2, anchorLocal.z - 9);

  yield;
  // Overland conveyor from the crusher's discharge to a stockpile.
  const head = new THREE.Vector3();
  crusher.conveyor?.anchors.head.getWorldPosition(head);
  const pileAt = local(0, 0, anchorLocal.z + 230);
  pileAt.y = H(pileAt.x, pileAt.z);
  const conveyor = makeConveyor(
    { points: [head.clone().add(new THREE.Vector3(0, -0.5, 0)), local(0, 14, anchorLocal.z + 150), pileAt.clone().setY(pileAt.y + 34)] },
    { width: 1.6, withGantry: true, covered: false, load: 0.8, speed: 3.5, ore: "ore" },
  );
  group.add(conveyor.group);
  yield;
  const stock = makeStockpile({ radius: 46, height: 31, shape: "cone", palette: "ore", seed: 9 });
  stock.group.position.copy(pileAt).add(new THREE.Vector3(0, -0.3, 0));
  group.add(stock.group);

  yield;
  /* ---------- The waste dump ---------- */
  const tip = pit.toDump[pit.toDump.length - 1]!.clone();
  const tipDir = new THREE.Vector3().subVectors(tip, pit.toDump[pit.toDump.length - 10]!).setY(0).normalize();
  const dumpPoint = ground(tip.clone().addScaledVector(tipDir, 14));

  /* ---------- The second loader: a face shovel on a waste face at the wall ---------- */
  type LoaderSim = Loader & {
    phase: number;
    hold: number;
    dumpAt: number;
    sign: number;
    pose(phase: number, loading: boolean): void;
    update(dt: number): void;
    lights(on: boolean): void;
    departAt: number;
  };
  const loaders: LoaderSim[] = [];
  const ropePose = (p: number, loading: boolean) => {
    // One pass: dig (0–0.42), swing out (0.42–0.62), dump (0.62–0.74), swing back (0.74–1).
    if (!loading && p <= 0.42) {
      const dig = smooth(p / 0.42);
      shovel.set({ swing: 0, crowd: mix(0.25, 0.9, dig), hoist: mix(0.05, 0.8, dig), door: 0 });
      return;
    }
    if (p < 0.42) {
      const dig = smooth(p / 0.42);
      shovel.set({ swing: 0, crowd: mix(0.25, 0.9, dig), hoist: mix(0.05, 0.8, dig), door: 0 });
    } else if (p < 0.62) {
      const s = smooth((p - 0.42) / 0.2);
      shovel.set({ swing: ROPE_SWING * s, crowd: mix(0.9, 0.55, s), hoist: mix(0.8, 1, s), door: 0 });
    } else if (p < 0.74) {
      shovel.set({ swing: ROPE_SWING, crowd: 0.55, hoist: 1, door: smooth((p - 0.62) / 0.06) });
    } else {
      const s = smooth((p - 0.74) / 0.26);
      shovel.set({ swing: ROPE_SWING * (1 - s), crowd: mix(0.55, 0.25, s), hoist: mix(1, 0.05, s), door: 1 - smooth((p - 0.74) / 0.1) });
    }
  };
  const S1: LoaderSim = {
    id: "S1",
    kind: "rope",
    group: shovel.group,
    face: "N-07",
    material: "ore",
    dest: "crusher",
    passes: o.passes ?? 3,
    passT: [354 / (o.passes ?? 3), 372 / (o.passes ?? 3)],
    passTime: o.passTime ?? 26,
    ...(o.dipper ? bucketed({ m3: o.dipper, density: 2.05, fill: [0.9, 1.08] }) : {}),
    spot: new THREE.Vector3(),
    queueAt: new THREE.Vector3(),
    loading: null,
    queue: [],
    idle: 0,
    busy: 0,
    waiting: 0,
    passesDone: 0,
    dumpPoint: dipper.clone(),
    muck: muckPile.group,
    phase: 0.42,
    hold: 0.42,
    dumpAt: 0.66,
    departAt: 0.8,
    sign: -1,
    pose: ropePose,
    update: (dt) => shovel.update(dt),
    lights: (on) => shovel.set({ lights: on }),
  };
  loaders.push(S1);

  let face: FaceShovel | null = null;
  let L2: LoaderSim | null = null;
  const extraTowers: THREE.Vector3[] = [];
  if (second) {
    // Walk out from the middle of the floor on a bearing until the ground starts to climb: the toe of the wall.
    const floorY = pit.floor.y;
    const phi = Math.atan2(-side.z, -side.x);
    const out = new THREE.Vector3(Math.cos(phi), 0, Math.sin(phi));
    let toe = 60;
    for (let d = 40; d < 400; d += 2) {
      const q = shovelAt.clone().setY(0).addScaledVector(out, d);
      if (H(q.x, q.z) > floorY + 2.5) {
        toe = d;
        break;
      }
    }
    const fsAt = ground(shovelAt.clone().setY(0).addScaledVector(out, toe - 36).addScaledVector(inward, 30));
    face = makeFaceShovel();
    face.group.position.copy(fsAt);
    const fsDir = out.clone();
    face.group.rotation.y = Math.atan2(fsDir.x, fsDir.z);
    group.add(face.group);
    const muck2 = makeMuckPile({ length: 54, width: 24, height: 8, palette: "waste", seed: 8 });
    muck2.group.position.copy(fsAt).addScaledVector(fsDir, 20);
    muck2.group.position.y = H(muck2.group.position.x, muck2.group.position.z) - 0.4;
    muck2.group.rotation.y = Math.atan2(fsDir.x, fsDir.z) + Math.PI;
    group.add(muck2.group);
    // The bucket empties over the truck when the upper swings a quarter turn towards the ramp.
    const towardRamp = new THREE.Vector3().subVectors(rampEnd, fsAt).setY(0).normalize();
    const leftW = leftOf(fsDir);
    const sign = leftW.dot(towardRamp) > 0 ? 1 : -1;
    const FS_SWING = (sign * Math.PI) / 2;
    face.set({ swing: FS_SWING, boom: 0.9, stick: 0.55, bucket: 0.85, clam: 0 });
    face.group.updateMatrixWorld(true);
    const bucketAt = new THREE.Vector3();
    face.anchors.bucket.getWorldPosition(bucketAt);
    const fs = face;
    const facePose = (p: number, loading: boolean) => {
      // One pass: crowd into the face (0–0.38), swing (0.38–0.6), open the clam (0.6–0.7), swing back (0.7–1).
      const dig = (k: number) =>
        fs.set({ swing: 0, boom: mix(0.12, 0.62, smooth(k)), stick: mix(0.95, 0.42, k), bucket: mix(0.08, 0.92, smooth(k * 1.15)), clam: 0 });
      if (p < 0.38 || (!loading && p <= 0.38)) dig(p / 0.38);
      else if (p < 0.6) {
        const s = smooth((p - 0.38) / 0.22);
        fs.set({ swing: FS_SWING * s, boom: mix(0.62, 0.9, s), stick: mix(0.42, 0.55, s), bucket: mix(0.92, 0.85, s), clam: 0 });
      } else if (p < 0.7) fs.set({ swing: FS_SWING, boom: 0.9, stick: 0.55, bucket: 0.85, clam: smooth((p - 0.6) / 0.05) });
      else {
        const s = smooth((p - 0.7) / 0.3);
        fs.set({ swing: FS_SWING * (1 - s), boom: mix(0.9, 0.12, s), stick: mix(0.55, 0.95, s), bucket: mix(0.85, 0.08, s), clam: 1 - smooth((p - 0.7) / 0.12) });
      }
    };
    L2 = {
      id: "L2",
      kind: "face",
      group: face.group,
      face: "W-12",
      material: "waste",
      dest: "dump",
      passes: 5,
      passT: [66, 74],
      passTime: 27,
      ...(o.dipper ? bucketed({ m3: 38, density: 2.1, fill: [0.86, 1.04] }) : {}),
      spot: new THREE.Vector3(),
      queueAt: new THREE.Vector3(),
      loading: null,
      queue: [],
      idle: 0,
      busy: 0,
      waiting: 0,
      passesDone: 0,
      dumpPoint: bucketAt.clone(),
      muck: muck2.group,
      phase: 0.38,
      hold: 0.38,
      dumpAt: 0.64,
      departAt: 0.78,
      sign,
      pose: facePose,
      update: (dt) => fs.update(dt),
      lights: (on) => fs.set({ lights: on }),
    };
    loaders.push(L2);
    extraTowers.push(ground(fsAt.clone().addScaledVector(towardRamp, 30).addScaledVector(fsDir, -34)));
  }
  const loaderOf = (id: string) => loaders.find((l) => l.id === id) ?? S1;

  /* ---------- Roads ---------- */
  // Two-way roads keep traffic to the right; the floor roads meet the ramp's two lanes where it reaches the floor.
  const rampUp = [...pit.ramp].reverse();
  const rampDown = pit.ramp;
  const downEnd = rampEnd.clone().add(leftOf(inward).multiplyScalar(LANE));
  const upStart = rampEnd.clone().sub(leftOf(inward).multiplyScalar(LANE));
  const toPad = pit.toPad;
  const padJoin = bez(toPad[toPad.length - 1]!, toPad[toPad.length - 1]!.clone().lerp(rampFoot, 0.6), rampFoot);
  const fromPad = [...padJoin].reverse();
  const roadBack = [...toPad].reverse();
  const toDump = pit.toDump;
  const tipBack = bez(tip, tip.clone().addScaledVector(tipDir, 7), dumpPoint);
  const tipOut = [...tipBack].reverse();
  const dumpBack = [...toDump].reverse();

  const V = { floor: 6.2, approach: 5, up: 3.3, upEmpty: 6.2, road: 11, roadLoaded: 9, down: 7, reverse: 1.9, platform: 3.4 };
  const floorRoads: THREE.Vector3[][] = [];

  /**
   * A loading bay: the truck stands with its body under the bucket, nose towards the ramp. Trucks come in on a lane
   * wide of the exit, swing round in a U-turn to stop at the queue point, then reverse in.
   */
  const bay = (X: LoaderSim) => {
    const heading = new THREE.Vector3().subVectors(rampEnd, X.group.position).setY(0);
    // Square to the loader's side, nose roughly towards the ramp.
    const fromLoader = new THREE.Vector3().subVectors(X.dumpPoint, X.group.position).setY(0).normalize();
    const u = new THREE.Vector3(-fromLoader.z, 0, fromLoader.x);
    if (u.dot(heading) < 0) u.negate();
    const v = fromLoader.clone();
    const bodyOffset = -1.2; // body centre sits behind the truck's origin
    const spot = ground(X.dumpPoint.clone().setY(0).addScaledVector(u, -bodyOffset));
    const P2 = (a: number, b: number) => ground(spot.clone().addScaledVector(u, a).addScaledVector(v, b));
    const loopR = 14;
    const turn = P2(19, 13);
    const entry = P2(19, 13 + 2 * loopR);
    const centre = P2(19, 13 + loopR);
    const arc: THREE.Vector3[] = [];
    for (let k = 1; k <= 24; k++) {
      const a = (k / 24) * Math.PI;
      // From the entry (heading −u) round through the far side to the turn (heading +u).
      arc.push(ground(centre.clone().addScaledVector(v, Math.cos(a) * loopR).addScaledVector(u, -Math.sin(a) * loopR)));
    }
    const lead = cubic(downEnd, downEnd.clone().addScaledVector(inward, 38), entry.clone().addScaledVector(u, 42), entry).map(ground);
    const inPts = [...lead, ...arc];
    const backPts = cubic(turn, P2(11, 13), P2(8, 1.5), spot).map(ground);
    const outAll = cubic(spot, P2(34, -2), upStart.clone().addScaledVector(inward, 40), upStart).map(ground);
    // Split the exit so the loader is free once the truck has pulled clear.
    let cut = 1;
    let run = 0;
    while (cut < outAll.length - 2 && run < 26) {
      run += outAll[cut]!.distanceTo(outAll[cut - 1]!);
      cut++;
    }
    X.spot.copy(spot);
    X.queueAt.copy(turn);
    floorRoads.push(inPts, backPts, outAll);
    return {
      inn: seg(inPts, 1, V.approach, "in", { loader: X.id }),
      back: seg(backPts, -1, V.reverse, "spot", { claim: X.id, then: "load", loader: X.id }),
      out1: seg(outAll.slice(0, cut), 1, V.floor * 0.6, "out", { release: X.id, loader: X.id }),
      out2: seg(outAll.slice(cut - 1), 1, V.floor, "out", { loader: X.id }),
    };
  };
  const bays = new Map(loaders.map((X) => [X.id, bay(X)]));

  const lane = (a: number, b: number) => ({ off: LANE, a, b });
  const S = {
    rampUp: seg(rampUp, 1, V.up, "up", { lane: lane(0, 40) }),
    rampDown: seg(rampDown, 1, V.down, "down", { lane: lane(40, 0) }),
    toPad: seg(toPad, 1, V.road, "road", { loadedSpeed: V.roadLoaded, lane: lane(40, 40) }),
    padJoin: seg(padJoin, 1, V.floor, "pad"),
    crushIn: seg(crushIn, 1, V.platform, "platform", { claim: "crusher", fixedY: true }),
    crushBack: seg(crushBack, -1, V.reverse, "tip", { fixedY: true, then: "dump" }),
    crushOut: seg(crushOut, 1, V.platform, "platform", { fixedY: true, release: "crusher" }),
    fromPad: seg(fromPad, 1, V.floor, "pad"),
    roadBack: seg(roadBack, 1, V.road, "road", { lane: lane(40, 40) }),
    toDump: seg(toDump, 1, V.road, "road", { loadedSpeed: V.roadLoaded, lane: lane(40, 20) }),
    tipBack: seg(tipBack, -1, V.reverse, "tip", { claim: "dump", then: "dump" }),
    tipOut: seg(tipOut, 1, V.floor * 0.6, "pad", { release: "dump" }),
    dumpBack: seg(dumpBack, 1, V.road, "road", { lane: lane(20, 40) }),
  };
  const routeFor = (X: LoaderSim, dest: "crusher" | "dump"): Seg[] => {
    const b = bays.get(X.id)!;
    const haul =
      dest === "crusher"
        ? [S.toPad, S.padJoin, S.crushIn, S.crushBack, S.crushOut, S.fromPad, S.roadBack]
        : [S.toDump, S.tipBack, S.tipOut, S.dumpBack];
    return [b.out1, b.out2, S.rampUp, ...haul, S.rampDown, b.inn, b.back];
  };
  const routes = new Map<string, Seg[]>();
  const routeKey = (X: string, dest: string) => `${X}:${dest}`;
  for (const X of loaders) for (const d of ["crusher", "dump"] as const) routes.set(routeKey(X.id, d), routeFor(X, d));
  // Which stretches carry a load: from leaving the loader to tipping.
  const loadedOn = (segs: Seg[], i: number) => i <= segs.findIndex((s) => s.then === "dump");
  // Which stretches lie between taking a resource and letting it go.
  const claimed = (segs: Seg[]) => {
    const inside = segs.map(() => false);
    segs.forEach((s0, k) => {
      if (!s0.claim) return;
      for (let j = k, n = 0; n < segs.length; j = (j + 1) % segs.length, n++) {
        inside[j] = true;
        if (segs[j]!.release === s0.claim) break;
      }
    });
    return inside;
  };

  /* ---------- Truck physics on the roads ---------- */
  interface Prof {
    sin: Float32Array;
    cos: Float32Array;
    /** Speed cap from bends and the site limit, m/s. */
    cap: Float32Array;
    /** Steady speeds empty and with a rated load, m/s, and what sets them. */
    vE: number;
    vL: number;
    govE: string;
    govL: string;
    grade: number;
    steepest: number;
  }
  const profs = new Map<Seg, Prof>();
  /** The point at or before distance d. */
  const idxAt = (s: Seg, d: number) => {
    let lo = 0;
    let hi = s.len.length - 1;
    if (d <= 0) return 0;
    if (d >= s.len[hi]!) return hi;
    while (hi - lo > 1) {
      const mid = (lo + hi) >> 1;
      if (s.len[mid]! <= d) lo = mid;
      else hi = mid;
    }
    return lo;
  };
  /** Steady speed at a point for a mass (kg) and what sets it: rimpull up a grade, the retarder down one, a bend or the limit. */
  const steadyAt = (P: { sin: Float32Array; cos: Float32Array; cap: Float32Array }, k: number, m: number, lim: number): [number, string] => {
    const F = m * G * (P.sin[k]! + TRUCK.rr * P.cos[k]!);
    let v = P.cap[k]!;
    let why = v < lim - 1e-3 ? "bend" : "site limit";
    if (F > 0) {
      const w = F > TRUCK.tractionKN * 1000 ? 0.3 : (TRUCK.wheelKW * 1000) / F;
      if (w < v) [v, why] = [w, "rimpull"];
    } else if (F < 0) {
      const w = (TRUCK.retardKW * 1000) / -F;
      if (w < v) [v, why] = [w, "retarder"];
    }
    return [v, why];
  };
  const prof = (s: Seg): Prof => {
    const got = profs.get(s);
    if (got) return got;
    const n = s.pts.length;
    const y = s.pts.map((p) => (s.fixedY ? p.y : H(p.x, p.z)));
    const sin = new Float32Array(n);
    const cos = new Float32Array(n);
    const cap = new Float32Array(n);
    const lim = (LIMITS[s.kind] ?? 25) / 3.6;
    const flat = (a: number, b: number) => Math.hypot(s.pts[b]!.x - s.pts[a]!.x, s.pts[b]!.z - s.pts[a]!.z);
    let steepest = 0;
    for (let i = 0; i < n; i++) {
      const a = idxAt(s, s.len[i]! - 10);
      const b = Math.min(n - 1, idxAt(s, s.len[i]! + 10) + 1);
      let run = 0;
      for (let j = a + 1; j <= b; j++) run += flat(j - 1, j);
      const g = run > 0.5 ? (y[b]! - y[a]!) / run : 0;
      steepest = Math.max(steepest, Math.abs(g));
      const th = Math.atan(g);
      sin[i] = Math.sin(th);
      cos[i] = Math.cos(th);
      // Bend radius from the turn in heading across the window.
      const a1 = Math.min(a + 1, n - 1);
      const b0 = Math.max(b - 1, 0);
      const h0 = Math.atan2(s.pts[a1]!.x - s.pts[a]!.x, s.pts[a1]!.z - s.pts[a]!.z);
      const h1 = Math.atan2(s.pts[b]!.x - s.pts[b0]!.x, s.pts[b]!.z - s.pts[b0]!.z);
      let dh = Math.abs(h1 - h0);
      if (dh > Math.PI) dh = 2 * Math.PI - dh;
      const R = dh > 1e-3 ? run / dh : 1e6;
      cap[i] = Math.min(lim, Math.sqrt(TRUCK.lat * R));
    }
    const P0 = { sin, cos, cap };
    const steady = (mT: number): [number, string] => {
      const m = mT * 1000;
      let T = 0;
      const why = new Map<string, number>();
      for (let i = 1; i < n; i++) {
        const ds = s.len[i]! - s.len[i - 1]!;
        const [v, w] = steadyAt(P0, i, m, lim);
        T += ds / Math.max(0.3, v);
        why.set(w, (why.get(w) ?? 0) + ds);
      }
      const gov = [...why].sort((p, q) => q[1] - p[1])[0]?.[0] ?? "site limit";
      return [T > 0 ? lengthOf(s) / T : lim, gov];
    };
    const [vE, govE] = steady(TRUCK.emptyT);
    const [vL, govL] = steady(TRUCK.emptyT + TRUCK.ratedT);
    let run = 0;
    for (let i = 1; i < n; i++) run += flat(i - 1, i);
    const P: Prof = { sin, cos, cap, vE, vL, govE, govL, grade: run > 1 ? (y[n - 1]! - y[0]!) / run : 0, steepest };
    profs.set(s, P);
    return P;
  };
  /** The highest speed allowed at a point for a truck of mass m (kg): the bend or limit cap, and the retarder downhill. */
  const capAt = (s: Seg, k: number, m: number) => {
    const P = prof(s);
    let c = P.cap[k]!;
    const down = -m * G * (P.sin[k]! + TRUCK.rr * P.cos[k]!);
    if (down > 0) c = Math.min(c, (TRUCK.retardKW * 1000) / down);
    return c;
  };
  // Cycle estimates use the steady speeds.
  const timed = (q: Seg) => {
    const P = prof(q);
    q.speed = P.vE;
    q.loadedSpeed = P.vL;
    return q;
  };
  for (const q of Object.values(S)) timed(q);
  for (const b of bays.values()) for (const q of Object.values(b)) timed(q);
  const road = (name: string, q: Seg, loaded: boolean, pts = q.pts): Road => {
    const P = prof(q);
    return {
      name,
      kind: q.kind,
      pts,
      length: lengthOf(q),
      grade: P.grade,
      steepest: P.steepest,
      vEmpty: P.vE * 3.6,
      vLoaded: P.vL * 3.6,
      govEmpty: P.govE,
      govLoaded: P.govL,
      loaded,
    };
  };
  const roads: Road[] = [
    road("Ramp, up", S.rampUp, true),
    road("Ramp, down", S.rampDown, false),
    road("Road to the crusher", S.toPad, true),
    road("Crusher ramp", S.crushIn, true),
    road("Road back from the crusher", S.roadBack, false),
    ...(second ? [road("Road to the dump", S.toDump, true), road("Road back from the dump", S.dumpBack, false)] : []),
    ...[...bays].flatMap(([id, b]) => [
      road(`Approach to ${id}`, b.inn, false),
      road(`Reversing in at ${id}`, b.back, false),
      road(`Pulling out of ${id}`, b.out2, true, [...b.out1.pts, ...b.out2.pts.slice(1)]),
    ]),
  ];

  yield;
  /* ---------- Trucks ---------- */
  type Run = Truck & {
    segs: Seg[];
    i: number;
    d: number;
    t: number;
    detour: Seg | null;
    /** Where the truck joins its next stretch, after a detour. */
    enter: number;
    lastHeading: THREE.Vector3;
    dest: "crusher" | "dump";
    arrived: number | null;
    /** Ground speed, unsigned, for the acceleration limit. */
    v: number;
    dump: number;
    /** The wheel groups (they stay on the ground as the frame squats) and the struts' travel and its rate. */
    wheels: THREE.Object3D[];
    sq: number;
    sqv: number;
  };
  const held: Record<string, Run | null> = { crusher: null, dump: null };
  for (const X of loaders) held[X.id] = null;
  const runs: Run[] = [];
  const names = ["T03", "T07", "T11", "T14", "T18", "T22", "T25", "T29", "T31", "T34", "T38", "T41", "T45", "T47", "T52"];
  const plan: { loader: LoaderSim; dest: "crusher" | "dump" }[] = [];
  if (second) {
    const f = o.fleet ?? { S1: 10, L2: 2 };
    for (let k = 0; k < f.S1; k++) plan.push({ loader: S1, dest: "crusher" });
    for (let k = 0; k < f.L2; k++) plan.push({ loader: L2!, dest: "dump" });
  } else {
    const count = o.trucks ?? 7;
    for (let k = 0; k < count; k++) plan.push({ loader: S1, dest: k % 3 === 2 ? "dump" : "crusher" });
  }
  const timeOf = (s: Seg) => lengthOf(s) / s.speed;
  for (const [k, { loader: X, dest }] of plan.entries()) {
    yield;
    const route = dest === "crusher" ? "ore" : "waste";
    const model = makeHaulTruck({ payload: true, ore: route === "ore" ? "ore" : "waste", seed: 20 + k });
    group.add(model.group);
    const segs = routes.get(routeKey(X.id, dest))!;
    // Spread the trucks of each loader round its cycle by driving time, off the claimed stretches.
    const mates = plan.filter((p) => p.loader === X && p.dest === dest);
    const nth = plan.slice(0, k).filter((p) => p.loader === X && p.dest === dest).length;
    const total = segs.reduce((s, q) => s + timeOf(q), 0);
    let goal = ((nth + 0.5) / mates.length) * total;
    let i = 0;
    while (i < segs.length - 1 && goal > timeOf(segs[i]!)) {
      goal -= timeOf(segs[i]!);
      i++;
    }
    let d = Math.min(goal * segs[i]!.speed, lengthOf(segs[i]!) - 1);
    // Not inside a stretch someone must hold: move on to the open road beyond it, behind anyone already there.
    const inside = claimed(segs);
    while (inside[i]) {
      i = (i + 1) % segs.length;
      d = 0;
    }
    for (const u of runs) if (u.segs[u.i]!.pts === segs[i]!.pts && Math.abs(u.d - d) < GAP + 4) d = Math.min(lengthOf(segs[i]!) - 1, u.d + GAP + 4);
    const loaded = loadedOn(segs, i) && i > 0;
    model.set({ payload: loaded ? 1 : 0 });
    runs.push({
      id: names[k] ?? `T${k}`,
      model,
      route,
      doing: "",
      payloadT: loaded ? Math.round(X.passes * mix(X.passT[0], X.passT[1], r())) : 0,
      loads: 0,
      waited: 0,
      loader: X.id,
      pending: null,
      load: null,
      last: null,
      speed: 0,
      act: "drive",
      stretch: segs[i]!.kind,
      segs,
      i,
      d,
      t: 0,
      detour: null,
      enter: 0,
      lastHeading: new THREE.Vector3(0, 0, 1),
      dest,
      arrived: null,
      v: 0,
      dump: 0,
      wheels: model.group.children.filter((c) => Math.abs(c.position.y - HAUL.axleY) < 1e-3),
      sq: 0,
      sqv: 0,
    });
  }

  /* ---------- Records ---------- */
  let time = 0;
  let loadNo = 0;
  const ledger: Custody[] = [];
  const minutes: Minute[] = [];
  const listeners: ((e: MineEvent) => void)[] = [];
  const emit = (type: MineEvent["type"], truck: Run, at: THREE.Vector3, loader?: string) => {
    for (const fn of listeners) fn({ type, truck, at: at.clone(), loader });
  };
  const zero = () => Object.fromEntries(loaders.map((X) => [X.id, 0]));
  const minute = () => {
    const m = Math.floor(time / 60);
    let row = minutes[minutes.length - 1];
    if (!row || row.t < m * 60) {
      row = { t: m * 60, queue: zero(), idle: zero(), loaded: zero(), ore: 0, waste: 0 };
      minutes.push(row);
    }
    return row;
  };
  const tally = { ore: 0, waste: 0, loads: 0, queue: 0 };
  // Grade-control blocks along each face: the ore face carries a smooth trend, the waste face sits below cut-off.
  const blockGrade = (X: LoaderSim, n: number) => {
    if (X.material === "ore") return { mean: 1.42 + 0.21 * Math.sin(n * 0.61) + 0.07 * (r() - 0.5), sd: 0.07 + 0.04 * r() };
    return { mean: 0.09 + 0.07 * r(), sd: 0.03 + 0.02 * r() };
  };
  const blocks: Record<string, number> = {};
  const openLoad = (t: Run, X: LoaderSim): Custody => {
    blocks[X.id] = (blocks[X.id] ?? 13) + (r() < 0.55 ? 1 : 0);
    const b = blocks[X.id]!;
    return {
      n: ++loadNo,
      truck: t.id,
      loader: X.id,
      face: X.face,
      material: t.route,
      dest: t.dest,
      block: `${X.face.replace("-", "")}-${String(b).padStart(2, "0")}`,
      grade: blockGrade(X, b),
      passes: [],
      payloadT: 0,
      tArrive: t.arrived ?? time,
      tSpot: time,
      tFirst: null,
      tLoaded: null,
      tDepart: null,
      tTip: null,
      queued: t.arrived === null ? 0 : time - t.arrived,
    };
  };

  /* ---------- Placing a truck ---------- */
  const pos = new THREE.Vector3();
  const tan = new THREE.Vector3();
  const p2 = new THREE.Vector3();
  const t2 = new THREE.Vector3();
  const off = new THREE.Vector3();
  const cur = (t: Run) => t.detour ?? t.segs[t.i]!;
  /** Where a truck at distance d along a stretch actually drives: the centreline, or its lane. */
  const where = (s: Seg, d: number, out: THREE.Vector3) => {
    const L = at(s, d, out, tan);
    if (s.lane) {
      const { off: o2, a, b } = s.lane;
      const w = (a ? smooth(d / a) : 1) * (b ? smooth((L - d) / b) : 1);
      // The lane is to the left of the way the points run, whichever way the truck faces.
      out.add(leftOf(tan, off).multiplyScalar(o2 * w));
    }
    return out;
  };
  const place = (t: Run) => {
    const s = cur(t);
    const L = lengthOf(s);
    where(s, t.d, pos);
    // Heading from just behind to just ahead, so lane changes and curves turn the truck smoothly.
    const a = Math.max(0, t.d - 1.5);
    const b = Math.min(L, t.d + 1.5);
    where(s, b, p2);
    where(s, a, t2);
    const heading = p2.sub(t2).setY(0);
    if (heading.lengthSq() < 1e-4) heading.copy(t.lastHeading);
    else heading.normalize();
    if (s.dir < 0) heading.negate();
    // Steering from how fast the heading turns.
    const ahead = new THREE.Vector3();
    const behind = new THREE.Vector3();
    where(s, clamp(t.d + 5 * s.dir, 0, L), ahead);
    where(s, clamp(t.d + 3 * s.dir, 0, L), behind);
    const h2 = ahead.sub(behind).setY(0);
    if (h2.lengthSq() > 1e-4) {
      h2.normalize();
      if (s.dir < 0) h2.negate();
    } else h2.copy(heading);
    const turn = Math.atan2(heading.x * h2.z - heading.z * h2.x, heading.dot(h2));
    t.lastHeading.copy(heading);
    const front = pos.clone().addScaledVector(heading, HAUL.frontZ);
    const rear = pos.clone().addScaledVector(heading, HAUL.rearZ);
    const yf = s.fixedY ? front.y : H(front.x, front.z);
    const yr = s.fixedY ? rear.y : H(rear.x, rear.z);
    const y = s.fixedY ? pos.y : (yf * -HAUL.rearZ + yr * HAUL.frontZ) / (HAUL.frontZ - HAUL.rearZ);
    t.model.group.position.set(pos.x, y, pos.z);
    t.model.group.rotation.set(0, 0, 0);
    t.model.group.rotation.y = Math.atan2(heading.x, heading.z);
    t.model.group.rotateX(-Math.atan2(yf - yr, HAUL.frontZ - HAUL.rearZ));
    const steer = clamp(-turn * 1.6 * s.dir, -0.6, 0.6);
    t.model.state.steer = mix(t.model.state.steer, steer, 0.15);
  };

  yield;
  /* ---------- The drill on a bench ---------- */
  const drill = makeDrillRig();
  const drillAng = Math.atan2(rampEnd.z, rampEnd.x) + 2.1;
  const drillAt =
    pit.berm(drillAng, 45) ?? pit.berm(drillAng, 30) ?? ground(new THREE.Vector3(Math.cos(drillAng) * 250, 0, Math.sin(drillAng) * 250));
  drill.group.position.copy(drillAt);
  drill.group.rotation.y = drillAng + Math.PI / 2;
  drill.set({ mast: 1, jacks: 1, feed: 0.6, rodSpin: 6, dust: 0.7 });
  group.add(drill.group);

  yield;
  /* ---------- Light towers and a pickup ---------- */
  const towers = [
    ground(shovelAt.clone().addScaledVector(side, 34).addScaledVector(faceDir, -18)),
    ground(shovelAt.clone().addScaledVector(side, -30).addScaledVector(faceDir, -26)),
    ground(local(-22, 0, rampTopZ - 40)),
    ground(drillAt.clone().addScaledVector(new THREE.Vector3(Math.cos(drillAng), 0, Math.sin(drillAng)), -14)),
    ...extraTowers,
  ].map((p, i) => {
    const t = makeLightTower();
    t.group.position.copy(p);
    t.group.rotation.y = i * 1.7;
    t.set({ mast: 1, yaw: 0 });
    group.add(t.group);
    return t;
  });
  if (second) {
    // Aim the loading-area towers at the bays.
    const aim = (tw: (typeof towers)[number], to: THREE.Vector3) => {
      tw.group.rotation.y = Math.atan2(to.x - tw.group.position.x, to.z - tw.group.position.z);
    };
    aim(towers[0]!, S1.spot);
    aim(towers[1]!, S1.spot);
    if (L2 && towers[4]) aim(towers[4], L2.spot);
  }
  const pickup = o.pickup === false ? null : makePickup();
  if (pickup) group.add(pickup.group);
  const pickupRoute = seg([...pit.toDump].reverse().concat(pit.toPad.slice(1)), 1, 13, "road");
  let pickupD = 0;
  let pickupDir = 1;

  /* ---------- People ---------- */
  const people: Miner[] = [];
  if (o.people !== false) {
    const crew: [Parameters<typeof makeMiner>[0], THREE.Vector3, THREE.Vector3][] = [
      [{ role: "supervisor", seed: 1, detail: 1 }, ground(shovelAt.clone().addScaledVector(side, 32).addScaledVector(faceDir, -22)), shovelAt],
      [{ role: "geologist", seed: 2, detail: 1 }, ground(muckPile.group.position.clone().addScaledVector(side, -38).addScaledVector(faceDir, -4)), muckPile.group.position],
      [{ role: "blaster", seed: 3, detail: 1 }, ground(drillAt.clone().addScaledVector(new THREE.Vector3(Math.cos(drillAng), 0, Math.sin(drillAng)), -8).addScaledVector(new THREE.Vector3(-Math.sin(drillAng), 0, Math.cos(drillAng)), 6)), drillAt],
      [{ role: "operator", seed: 4, detail: 1 }, ground(local(-14, 0, rampTopZ - 78)), crusher.group.position],
    ];
    for (const [opts, p, look] of crew) {
      yield;
      const m = makeMiner(opts);
      m.group.position.copy(p);
      m.face(look.clone());
      m.work(opts.role === "geologist" ? "inspect" : opts.role === "supervisor" ? "radio" : opts.role === "blaster" ? "tablet" : "point", look.clone());
      group.add(m.group);
      people.push(m);
    }
  }

  /* ---------- The simulation ---------- */
  const next = (t: Run) => (t.detour ? t.segs[t.i]! : t.segs[(t.i + 1) % t.segs.length]!);
  /** Distance to the nearest truck ahead on this stretch or the next, nose to nose. */
  const gapAhead = (t: Run) => {
    const s = cur(t);
    const n = next(t);
    const L = lengthOf(s);
    const entry = t.detour ? t.enter : 0;
    let gap = Infinity;
    for (const u of runs) {
      if (u === t) continue;
      const us = cur(u);
      if (us.pts === s.pts && (u.d > t.d || (u.d === t.d && runs.indexOf(u) < runs.indexOf(t)))) gap = Math.min(gap, u.d - t.d);
      else if (us.pts === n.pts && u.d >= entry) gap = Math.min(gap, L - t.d + (u.d - entry));
    }
    return gap;
  };
  const switchRoute = (t: Run, X: LoaderSim) => {
    t.segs = routes.get(routeKey(X.id, X.dest))!;
    t.dest = X.dest;
    t.pending = null;
  };
  /** A truck on its way in, or queued, turns off for its new loader: a short run across the floor joins that loader's approach. */
  const transfer = (t: Run, X: LoaderSim) => {
    const b = bays.get(X.id)!;
    place(t);
    const from = t.model.group.position.clone();
    const h = t.lastHeading.clone();
    // Join the new approach a third of the way along, where it is clear of the ramp.
    const join = lengthOf(b.inn) * 0.42;
    const jp = new THREE.Vector3();
    const jt = new THREE.Vector3();
    at(b.inn, join, jp, jt);
    const k = from.distanceTo(jp) * 0.4;
    const pts = cubic(from, from.clone().addScaledVector(h, k), jp.clone().addScaledVector(jt, -k), jp).map(ground);
    switchRoute(t, X);
    t.i = t.segs.indexOf(b.inn);
    t.detour = timed(seg(pts, 1, V.approach, "transfer", { loader: X.id }));
    t.enter = join;
    t.d = 0;
    t.act = "drive";
    t.arrived = null;
  };
  const assign = (id: string, to: string) => {
    const t = runs.find((x) => x.id === id);
    const X = loaders.find((l) => l.id === to);
    if (!t || !X || (t.loader === to && !t.pending)) return;
    t.loader = to;
    const s = cur(t);
    if (s.kind === "in" || s.kind === "transfer" || (s.kind === "down" && !t.payloadT)) {
      if (s.kind === "down") {
        switchRoute(t, X);
        t.i = t.segs.indexOf(S.rampDown);
      } else transfer(t, X);
    } else t.pending = to;
    emit("assign", t, t.model.group.position, to);
  };

  const step = (dt: number) => {
    time += dt;
    const row = minute();
    let queue = 0;
    for (const t of runs) {
      const s = cur(t);
      const L = lengthOf(s);
      t.stretch = s.kind;
      if (t.act === "load") {
        t.speed = 0;
        const X = loaderOf(t.loader);
        t.doing = X.id === "S1" && !second ? `Loading, pass ${Math.min(X.passes, X.passesDone + 1)} of ${X.passes}` : `Loading at ${X.id}, pass ${Math.min(X.passes, X.passesDone + 1)} of ${X.passes}`;
        continue;
      }
      if (t.act === "dump") {
        t.t += dt;
        t.speed = 0;
        const k = t.t / TIP;
        t.dump = k < 0.4 ? smooth(k / 0.4) : k < 0.66 ? 1 : 1 - smooth((k - 0.66) / 0.34);
        t.doing = t.dest === "crusher" ? "Tipping at the crusher" : "Tipping at the dump";
        if (k >= 0.38 && t.payloadT > 0) {
          if (t.dest === "crusher") {
            tally.ore += t.payloadT;
            row.ore += t.payloadT;
          } else {
            tally.waste += t.payloadT;
            row.waste += t.payloadT;
          }
          tally.loads++;
          t.loads++;
          if (t.load) {
            t.load.tTip = time;
            ledger.push(t.load);
            t.last = t.load;
            t.load = null;
          }
          t.payloadT = 0;
          emit("tip", t, t.dest === "crusher" ? crushTip : t.model.group.position);
        }
        if (k >= 1) {
          t.act = "drive";
          t.dump = 0;
          t.i = (t.i + 1) % t.segs.length;
          t.d = 0;
        }
        continue;
      }
      // Driving: slow into stops, keep a gap to the truck ahead, wait for a loader, the crusher or the dump.
      const n = next(t);
      const blocked = !!n.claim && !t.detour && !!held[n.claim] && held[n.claim] !== t;
      // Stop at the end before reversing, before a resource someone else holds, and behind the truck ahead.
      let stopAt = s.then || blocked || (!t.detour && n.dir !== s.dir) ? L : Infinity;
      const gap = gapAhead(t);
      if (gap < GAP + 60) stopAt = Math.min(stopAt, t.d + gap - GAP);
      const loaded = t.payloadT > 0;
      const room = stopAt - t.d;
      // Rimpull against grade and rolling resistance; the retarder holds it downhill; bends and limits cap it.
      const m = (TRUCK.emptyT + t.payloadT) * 1000;
      const P = prof(s);
      const k = idxAt(s, t.d);
      const resist = m * G * (P.sin[k]! + TRUCK.rr * P.cos[k]!);
      const drive = Math.min(TRUCK.tractionKN * 1000, (TRUCK.wheelKW * 1000) / Math.max(t.v, 0.4));
      const a = Math.min((drive - resist) / m, loaded ? 0.4 : 0.7);
      let v = Math.max(0, t.v + a * dt);
      // The driver looks ahead and slows before a bend, a lower limit or the end of the stretch, not in it.
      for (const x of AHEAD) {
        let c: number;
        if (t.d + x <= L) c = capAt(s, idxAt(s, t.d + x), m);
        else if (!t.detour && n.dir === s.dir) c = capAt(n, idxAt(n, t.d + x - L), m);
        else c = capAt(s, s.pts.length - 1, m);
        v = Math.min(v, Math.sqrt(c * c + 2 * TRUCK.brake * x));
      }
      if (room !== Infinity) v = Math.min(v, Math.max(0, Math.sqrt(Math.max(0, 2 * TRUCK.brake * room)) * 0.9 + (room > 0.3 ? 0.3 : 0)));
      if (s.dir < 0) v = Math.min(v, Math.max(0.6, (L - t.d) * 0.35));
      v = Math.max(0, Math.min(v, room === Infinity ? v : Math.max(0, room) / dt));
      t.v = v;
      t.d += v * dt;
      t.speed = v * s.dir;
      const waiting = v < 0.05 && room < 1.5;
      if (waiting) {
        if (t.act !== "queue") t.waited = 0;
        t.act = "queue";
        t.waited += dt;
        queue++;
        if ((s.kind === "in" || s.kind === "transfer") && t.arrived === null) t.arrived = time;
        if (s.kind === "in" || s.kind === "transfer") row.queue[s.loader!] = (row.queue[s.loader!] ?? 0) + dt;
      } else if (t.act === "queue") t.act = "drive";
      if (s.kind === "spot") t.act = "spot";
      else if (t.act === "spot") t.act = "drive";
      const X = loaderOf(t.loader);
      t.doing =
        t.act === "queue"
          ? s.kind === "in" || s.kind === "transfer"
            ? second ? `Queued at ${X.id}` : "Queued at the shovel"
            : s.kind === "pad" || s.kind === "platform"
              ? "Queued at the crusher"
              : s.kind === "road" && t.dest === "dump" && loaded
                ? "Queued at the dump"
                : "Waiting behind a truck"
          : s.kind === "spot"
            ? "Reversing in"
            : s.kind === "tip"
              ? "Reversing to tip"
              : s.kind === "transfer"
                ? `Reassigned, heading to ${X.id}`
                : s.kind === "up"
                  ? loaded
                    ? "Climbing the ramp, loaded"
                    : "Climbing the ramp"
                  : s.kind === "down"
                    ? "Descending the ramp"
                    : s.kind === "in"
                      ? second ? `Heading in to ${X.id}` : "Heading in to the shovel"
                      : s.kind === "out"
                        ? "Pulling away loaded"
                        : loaded
                          ? t.dest === "crusher"
                            ? "Hauling to the crusher"
                            : "Hauling to the dump"
                          : "Returning empty";
      if (t.d >= L - 1e-3) {
        t.d = L;
        if (s.release && held[s.release] === t) held[s.release] = null;
        if (t.detour) {
          t.detour = null;
          t.d = t.enter;
          t.enter = 0;
          continue;
        }
        t.v = 0;
        if (s.then === "load") {
          const Y = loaderOf(s.loader!);
          t.act = "load";
          Y.loading = t;
          Y.passesDone = 0;
          if (t.load) t.load.tUnder = time;
          if (Y.plan) Y.passes = Y.plan;
          if (t.load) t.load.tFirst = null;
          continue;
        }
        if (s.then === "dump") {
          t.act = "dump";
          t.t = 0;
          continue;
        }
        if (n.claim) {
          if (held[n.claim] && held[n.claim] !== t) continue;
          held[n.claim] = t;
          if (n.kind === "spot") {
            const Y = loaderOf(n.loader!);
            t.load = openLoad(t, Y);
            t.arrived = null;
            emit("spot", t, Y.spot, Y.id);
          }
        }
        t.i = (t.i + 1) % t.segs.length;
        t.d = 0;
        if (n.dir !== s.dir) t.v = 0;
        // A dispatch change waiting for this truck takes effect as it starts back down to the floor.
        if (t.pending && t.segs[t.i]!.kind === "down") {
          switchRoute(t, loaderOf(t.pending));
          t.i = t.segs.indexOf(S.rampDown);
        }
      }
    }
    // The loaders load whoever stands under them.
    for (const X of loaders) {
      const truck = X.loading as Run | null;
      // Waiting for trucks means none under it, none reversing in and none queued: the change-over counts as work.
      const spotting = !truck && runs.some((t) => cur(t).kind === "spot" && cur(t).loader === X.id);
      if (truck || spotting || X.queue.length > 0) {
        X.busy += dt;
        X.waiting = 0;
      } else {
        X.idle += dt;
        X.waiting += dt;
        row.idle[X.id] = (row.idle[X.id] ?? 0) + dt;
      }
      const before = X.phase;
      if (truck) {
        X.phase += dt / X.passTime;
        if (before < X.dumpAt && X.phase >= X.dumpAt && X.passesDone < X.passes) {
          X.passesDone++;
          const mean = (X.passT[0] + X.passT[1]) / 2;
          // A top-up pass is a part-filled dipper, sized by eye to the shortfall the loader scale shows.
          const tonnes = Math.round(X.topUp ? clamp(X.topUp * mix(0.8, 1.15, r()), 12, X.passT[1]) : mix(X.passT[0], X.passT[1], r()));
          X.topUp = 0;
          truck.payloadT += tonnes;
          if (X.bucket) {
            // The operator reads the loader scale: tops up a light truck, and stops short rather than overload one.
            const short = TRUCK.ratedT - truck.payloadT;
            if (X.passesDone >= X.passes && short > 0.25 * mean && X.passes < 6) {
              X.passes++;
              X.topUp = short;
            } else if (X.passesDone < X.passes && truck.payloadT + mean > TRUCK.ratedT * 1.1) X.passes = X.passesDone;
          }
          row.loaded[X.id] = (row.loaded[X.id] ?? 0) + tonnes;
          if (truck.load) {
            truck.load.passes.push({ t: time, tonnes });
            truck.load.tFirst ??= time;
            if (X.passesDone === X.passes) truck.load.tLoaded = time;
          }
          emit("pass", truck, X.dumpPoint, X.id);
        }
        if (X.passesDone >= X.passes && X.phase >= X.departAt && before < X.departAt + 1) {
          // The truck pulls away as the bucket swings clear; its payload monitor weighs the load.
          truck.act = "drive";
          truck.i = (truck.i + 1) % truck.segs.length;
          truck.d = 0;
          truck.v = 0;
          if (second) {
            truck.route = X.material;
            truck.dest = X.dest;
          }
          if (truck.load) {
            const sum = truck.load.passes.reduce((a, p) => a + p.tonnes, 0);
            truck.load.payloadT = Math.round(sum * (1 + (r() - 0.5) * 0.024));
            truck.load.tDepart = time;
            truck.load.material = truck.route;
            truck.load.dest = truck.dest;
            truck.payloadT = truck.load.payloadT;
          }
          emit("depart", truck, truck.model.group.position, X.id);
          X.loading = null;
          X.passesDone = 0;
        }
        if (X.phase >= 1) X.phase -= 1;
      } else if (X.phase < X.hold) X.phase = Math.min(X.hold, X.phase + dt / X.passTime);
      else if (X.phase > X.hold) {
        X.phase += dt / X.passTime;
        if (X.phase >= 1) X.phase = Math.min(X.hold, X.phase - 1);
      }
      X.queue = runs
        .filter((t) => t.act === "queue" && (cur(t).kind === "in" || cur(t).kind === "transfer") && t.loader === X.id)
        .sort((a, b) => (cur(b) === cur(a) ? b.d - a.d : cur(a).kind === "transfer" ? 1 : -1));
    }
    tally.queue = queue;
  };

  const pose = (dtReal: number) => {
    for (const t of runs) {
      place(t);
      // The heap grows as each pass lands and goes as the body tips.
      const X = loaderOf(t.loader);
      let shown = t.payloadT > 0 ? 1 : 0;
      if (t.act === "load" && X.loading === t) {
        // The pass in the air lands over the last few hundredths of the cycle before the count ticks.
        const landing = X.passesDone < X.passes && X.phase > X.dumpAt - 0.05 && X.phase < X.dumpAt ? smooth((X.phase - (X.dumpAt - 0.05)) / 0.05) : 0;
        shown = clamp((X.passesDone + landing) / X.passes, 0, 1);
      }
      if (t.act === "dump") shown = t.payloadT > 0 ? 1 - smooth(t.t / (TIP * 0.4)) : 0;
      // The struts take up the load and bounce a little as a pass lands; the wheels stay on the ground.
      const want = (TRUCK.squat * (t.act === "dump" ? shown : 1) * t.payloadT) / TRUCK.ratedT;
      const h = Math.min(0.04, dtReal * mine.pace);
      if (h <= 0) {
        t.sq = want;
        t.sqv = 0;
      } else {
        for (let k = 0, n = Math.ceil((dtReal * mine.pace) / 0.04); k < Math.min(n, 8); k++) {
          t.sqv += (55 * (want - t.sq) - 4 * t.sqv) * h;
          t.sq += t.sqv * h;
        }
      }
      t.model.group.translateY(-t.sq);
      for (const w of t.wheels) w.position.y = HAUL.axleY + t.sq;
      t.model.set({ speed: t.speed, dump: t.dump, payload: shown });
      t.model.update(dtReal);
    }
    for (const X of loaders) {
      X.pose(X.phase, !!X.loading);
      X.update(dtReal);
    }
  };

  /* ---------- A truck already standing under the shovel when the scene opens ---------- */
  {
    const b = bays.get("S1")!;
    const first = runs.find((t) => t.loader === "S1" && (cur(t) === b.inn || cur(t) === S.rampDown)) ?? runs.find((t) => t.loader === "S1")!;
    first.i = first.segs.indexOf(b.back);
    first.d = lengthOf(b.back);
    first.act = "load";
    first.model.set({ payload: 0 });
    first.payloadT = 0;
    held.S1 = first;
    S1.loading = first;
    S1.phase = S1.hold;
    first.load = openLoad(first, S1);
  }
  const pace0 = o.pace ?? 2.2;
  const mine: Mine = {
    group,
    pit,
    trucks: runs,
    shovel,
    drill,
    crusher,
    stockpile: stock.group,
    muck: muckPile.group,
    dumpPoint,
    people,
    tally,
    get time() {
      return time;
    },
    pace: pace0,
    loaders,
    ledger,
    minutes,
    floorRoads,
    towers: towers.map((t) => t.group),
    roads,
    update(dtReal: number) {
      const dt = dtReal * mine.pace;
      // Big steps are cut up so trucks never jump past a stop.
      const n = Math.max(1, Math.ceil(dt / 0.25));
      for (let k = 0; k < n; k++) step(dt / n);
      pose(dtReal);
      drill.update(dtReal);
      crusher.update(dtReal);
      conveyor.update(dtReal);
      for (const tw of towers) tw.update(dtReal);
      for (const m of people) m.update(dtReal);
      if (!pickup) return;
      // The pickup patrols the surface roads.
      pickupD += pickupDir * 13 * dt * 0.5;
      const PL = lengthOf(pickupRoute);
      if (pickupD > PL || pickupD < 0) {
        pickupDir *= -1;
        pickupD = clamp(pickupD, 0, PL);
      }
      at(pickupRoute, pickupD, pos, tan);
      const ph = pickupDir > 0 ? tan : tan.clone().negate();
      pickup.group.position.set(pos.x + ph.z * 9, 0, pos.z - ph.x * 9);
      pickup.group.position.y = H(pickup.group.position.x, pickup.group.position.z);
      pickup.group.rotation.y = Math.atan2(ph.x, ph.z);
      pickup.set({ speed: 13 * 0.5 * mine.pace });
      pickup.update(dtReal);
    },
    lights(on) {
      for (const t of runs) t.model.set({ lights: on });
      for (const X of loaders) X.lights(on);
      drill.set({ lights: on });
      crusher.set({ lights: on });
      conveyor.set({ lights: on });
      pickup?.set({ lights: on });
      for (const tw of towers) tw.set({ lights: on });
    },
    assign,
    pick(ray) {
      const inv = new THREE.Matrix4();
      const local = new THREE.Ray();
      const box = new THREE.Box3(new THREE.Vector3(-HAUL.halfW, 0, -7.7), new THREE.Vector3(HAUL.halfW, 7.8, 7.7));
      const hit = new THREE.Vector3();
      let best: Run | null = null;
      let bestD = Infinity;
      for (const t of runs) {
        t.model.group.updateMatrixWorld();
        inv.copy(t.model.group.matrixWorld).invert();
        local.copy(ray).applyMatrix4(inv);
        if (!local.intersectBox(box, hit)) continue;
        hit.applyMatrix4(t.model.group.matrixWorld);
        const d = hit.distanceTo(ray.origin);
        if (d < bestD) {
          bestD = d;
          best = t;
        }
      }
      return best;
    },
    advance(seconds) {
      const n = Math.ceil(seconds / 0.5);
      for (let k = 0; k < n; k++) step(seconds / n);
      pose(0);
    },
    on(fn) {
      listeners.push(fn);
    },
    cycle(id) {
      const X = loaderOf(id);
      const segs = routes.get(routeKey(X.id, X.dest))!;
      let s = 0;
      segs.forEach((q, i) => {
        const v = loadedOn(segs, i) && q.loadedSpeed ? q.loadedSpeed : q.speed;
        // Reversing and the stops at either end of a stretch cost about a third again.
        s += lengthOf(q) / (q.dir < 0 || q.claim || q.release ? v * 0.6 : v * 0.95);
      });
      return s + X.passes * X.passTime + TIP;
    },
    eta(id) {
      const t = runs.find((x) => x.id === id);
      if (!t) return null;
      const X = loaderOf(t.loader);
      const full = t.payloadT > 0 || t.act === "load";
      if (t.act === "dump") return 0;
      const target = t.segs.findIndex((q) => (full ? q.then === "dump" : q.then === "load"));
      let s = 0;
      let i = t.i;
      let d = t.d;
      if (t.detour) {
        s += Math.max(0, lengthOf(t.detour) - t.d) / t.detour.speed;
        d = t.enter;
      }
      if (t.act === "load" && X.loading === t) {
        s += Math.max(0, X.passes - X.passesDone) * X.passTime + (X.departAt - X.dumpAt) * X.passTime;
        i = (i + 1) % t.segs.length;
        d = 0;
      }
      for (let n = 0; n < t.segs.length; n++) {
        const q = t.segs[i]!;
        const v = full && q.loadedSpeed ? q.loadedSpeed : q.speed;
        s += Math.max(0, lengthOf(q) - d) / (q.dir < 0 || q.claim || q.release ? v * 0.6 : v * 0.95);
        if (i === target) break;
        i = (i + 1) % t.segs.length;
        d = 0;
      }
      return s;
    },
  };
  // Settle everyone where they start.
  for (const t of runs) place(t);
  pose(0);
  return mine;
}
