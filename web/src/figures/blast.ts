/**
 * "Fire the pattern": a production blast on a working bench, close up and at real scale. The drilled and charged
 * pattern sits on the bench with its collars, stemming and harness; the ore boundary from the grade-control model is
 * painted and flagged on it; the drill is parked back, the shovel waits on the floor and the blast crew stands at the
 * firing point beyond the exclusion zone. Fire runs the electronic sequence hole by hole and row by row in slow
 * motion: the collars vent, the bench domes and heaves, the face bursts and throws its rock onto the floor on true
 * ballistic arcs, and a brown dust cloud rolls up and drifts off on the wind. Eight buried monitors then report how
 * far the rock moved; the dig lines are redrawn where the ore landed, and the dilution and ore loss of digging to
 * each line are counted from the scene's own geometry. The grade-control model and the heave field between the
 * monitors are estimated live by FrankenMining's engine in the browser (blast-engine.ts); the rock leaves the face and
 * the collars at speeds worked out from the charge (Richards & Moore). Everything is synthetic.
 */
import * as THREE from "three";
import { Dust, Tags } from "./blast-fx";
import { makeGround, preAt } from "./blast-ground";
import { arcArrows, arrowMaterial, flatArrows, lineMaterial, makeCollars, makeHarness, makeOverlay, makePegs, pullToCamera, ribbon } from "./blast-marks";
import { gradeModel, heaveModel, type GradeModel, type HeaveModel } from "./blast-engine";
import { adopt, BACK, BENCH, count, ECON, facts, heave, heaveEstimate, holes, inside, lastMs, monitors, moved, ORE, ORE_MOVED, outcome, PATTERN, SPAN, THROW, TOE, toWorld } from "./blast-plan";
import { Rubble, scatter, sizeOf, type Piece } from "./blast-rocks";
import { makeDrillRig, makePickup, makeRopeShovel, makeTrailingCable } from "./fleet";
import { $, clamp, fmt, mix, reduced, rng, smooth } from "./lib";
import { makeMiner, type Miner } from "./miners";
import { stage } from "./stage";

type Phase = "ready" | "arming" | "firing" | "throw" | "plume" | "settled";
type View = { target: [number, number, number]; distance: number; azimuth: number; elevation: number };

const pct = (v: number) => `${fmt(v * 100, 1)}%`;
const kt = (t: number) => `${fmt(t / 1000, 1)}&nbsp;kt`;
const money = (v: number) => `$${fmt(v / 1e6, 2)}M`;
const W = (u: number, s: number, y = 0) => {
  const [x, z] = toWorld(u, s);
  return new THREE.Vector3(x, y, z);
};

/** The hole's section: stemming over the charge, subdrill below the floor. */
const holeSvg = () => {
  const top = 10;
  const H = 120;
  const m = H / facts.depth;
  const stem = top + BENCH.stem * m;
  const floor = top + 15 * m;
  const bot = top + facts.depth * m;
  return `<svg class="bl-hole" viewBox="0 0 96 ${bot + 8}" aria-hidden="true">
<path d="M0 ${top}H96" class="bench"/><path d="M0 ${floor}H96" class="floor"/>
<rect x="20" y="${top}" width="9" height="${stem - top}" class="stem"/>
<rect x="20" y="${stem}" width="9" height="${bot - stem}" class="charge"/>
<path d="M20 ${top}V${bot}H29V${top}" class="wall"/>
<text x="36" y="${(top + stem) / 2 + 3}">${fmt(BENCH.stem, 1)} m stem</text>
<text x="36" y="${(stem + floor) / 2 + 3}">${fmt(facts.column, 1)} m charge</text>
<text x="36" y="${(floor + bot) / 2 + 4}">${fmt(BENCH.sub, 1)} m sub</text>
</svg>`;
};

/** The timing plan in plan view: each hole a dot, lit as it fires. */
const timingSvg = () => {
  const us = holes.map((h) => h.u);
  const u0 = Math.min(...us) - 4;
  const u1 = Math.max(...us) + 4;
  const s0 = BACK - 1;
  const w = 240;
  const h = 74;
  const X = (u: number) => ((u - u0) / (u1 - u0)) * w;
  const Y = (s: number) => ((0 - s) / (0 - s0)) * (h - 16) + 10;
  const ore = ORE.map(([u, s], i) => `${i ? "L" : "M"}${X(u).toFixed(1)} ${Y(s).toFixed(1)}`).join("") + "Z";
  return `<svg class="bl-timing" viewBox="-4 0 ${w + 8} ${h}" aria-hidden="true">
<path d="M${X(u0)} ${Y(0) - 3}H${X(u1)}" class="crest"/><text x="${X(u0)}" y="${Y(0) - 6}">crest · free face</text>
<path d="${ore}" class="ore"/>
${holes.map((ho, i) => `<circle cx="${X(ho.u).toFixed(1)}" cy="${Y(ho.s).toFixed(1)}" r="3.1" data-h="${i}"/>`).join("")}
</svg>`;
};

/** What the engine answered, for the cards. */
interface Eng {
  gm: GradeModel;
  hm: HeaveModel;
  /** Value of the moved line if the field is off by ±1.28 sd (its 80% band), and the 90% band at the contacts, m. */
  lo: number;
  hi: number;
  band: number;
  /** Ore tonnes inside the 90%- and 10%-sure lines. */
  sureT: number;
  maybeT: number;
}

const markup = (eng: Eng | null) => `
<div class="bl-stage">
  <div class="scene-view bl-view"></div>
  <div class="bl-count" aria-live="polite"></div>
  <div class="bl-mini" aria-hidden="true"><b data-mini>Ready to fire</b><span data-mini-rate></span></div>
  <div class="hud-card hud-tl bl-pat">
    <p class="micro">Pattern ${PATTERN.name} · as charged</p>
    <div class="bl-pat-row">${holeSvg()}<dl>
      <dt>Burden × spacing</dt><dd>${PATTERN.burden} × ${PATTERN.spacing}&nbsp;m</dd>
      <dt>Hole</dt><dd>${fmt(BENCH.dia * 1000)}&nbsp;mm × ${fmt(facts.depth, 1)}&nbsp;m</dd>
      <dt>Stemming</dt><dd>${fmt(BENCH.stem, 1)}&nbsp;m · 25&nbsp;d</dd>
      <dt>Charge a hole</dt><dd>${fmt(facts.kg)}&nbsp;kg</dd>
      <dt>Powder factor</dt><dd>${fmt(facts.pf, 2)}&nbsp;kg/m³</dd>
      <dt>Rock broken</dt><dd>${fmt(facts.tonnes / 1000)}&nbsp;kt · swell ${fmt(THROW.swell, 2)}</dd>
    </dl></div>
    <p class="bl-src"><span class="ev measured">measured</span> collar survey and loading record, ${facts.holes} holes</p>
  </div>
  <div class="hud-card hud-tr bl-fire">
    <p class="micro">Firing sequence · electronic detonators</p>
    <p class="bl-clock"><b data-ms>0</b><small>ms</small><span data-rate>ready</span></p>
    ${timingSvg()}
    <p class="bl-sub"><b data-fired>0</b> of ${facts.holes} fired · ${PATTERN.holeMs}&nbsp;ms between holes, ${PATTERN.rowMs}&nbsp;ms between rows · never two holes within ${facts.gap}&nbsp;ms</p>
    <p class="bl-throw">Face bursts at about <b>${fmt(THROW.face)}&nbsp;m/s</b>, collars vent at <b>${fmt(THROW.collar)}&nbsp;m/s</b>; worst case reaches ${fmt(THROW.reach)}&nbsp;m, inside the 500&nbsp;m zone. Richards &amp; Moore, site constant ${fmt(THROW.k.typical, 1)} <span class="ev assumed">assumed</span></p>
  </div>
  <div class="hud-card hud-bl bl-dig">
    <div data-dig="pre">
      <p class="micro">Grade-control model · before the blast</p>
      <div class="bl-ramp"><i></i><span>0</span><span class="cut">0.5&nbsp;g/t cut-off</span><span>2.5&nbsp;g/t</span></div>
      <p>The ore boundary is painted and flagged where the model crosses the cut-off. <span class="ev estimated">estimated</span></p>
      <p class="bl-engine">${
        eng
          ? `${eng.gm.samples} grade-control samples; ${fmt(eng.gm.blocks)} one-metre blocks estimated in ${fmt(eng.gm.ms, 1)}&nbsp;ms by FrankenMining’s spatial predictor, in your browser. ${kt(outcome.ore)} of ore inside the line; ${fmt(eng.sureT / 1000, 1)}–${kt(eng.maybeT)} between its 90%- and 10%-sure lines.`
          : `FrankenMining’s predictor did not run here, so the line shown is the synthetic ore body the samples are drawn from.`
      }</p>
      <p class="bl-wait">${monitors.length} movement monitors are buried in the pattern near the ore contacts. They are found and read after the blast.</p>
    </div>
    <div data-dig="post" hidden>
      <p class="micro">Dilution and ore loss · illustrative</p>
      <table class="bl-cmp">
        <thead><tr><th></th><th>Dilution</th><th>Ore loss</th></tr></thead>
        <tbody>
          <tr class="pre"><td>Pre-blast line</td><td><i style="--w:${outcome.pre.dilution / 0.2}"></i>${pct(outcome.pre.dilution)}</td><td><i style="--w:${outcome.pre.loss / 0.2}"></i>${pct(outcome.pre.loss)}</td></tr>
          <tr class="post"><td>Moved line</td><td><i style="--w:${outcome.post.dilution / 0.2}"></i>${pct(outcome.post.dilution)}</td><td><i style="--w:${outcome.post.loss / 0.2}"></i>${pct(outcome.post.loss)}</td></tr>
        </tbody>
      </table>
      <p class="bl-key"><span><i class="k-ore"></i>ore the pre-blast line leaves behind</span><span><i class="k-waste"></i>waste the pre-blast line takes in</span></p>
      <p class="bl-src">Heave ${fmt(outcome.out[0], 1)}–${fmt(outcome.out[1], 1)}&nbsp;m towards the pit <span class="ev measured">measured</span> by ${monitors.length} monitors. ${
        eng
          ? `Between them the same predictor estimates the field, ${fmt(eng.hm.blocks)} values in ${fmt(eng.hm.ms, 1)}&nbsp;ms, to ±${fmt(eng.band, 1)}&nbsp;m (90%) at the contacts`
          : `Between them the field is interpolated`
      } <span class="ev estimated">estimated</span>. Mixing at the contacts stays as dilution on either line.</p>
    </div>
  </div>
  <div class="hud-card hud-br agent-note bl-agent">
    <p class="micro">Agent · grade control</p>
    <div data-agent="pre"><p>Waiting for the blast. When the monitors report, I move the dig line to where the ore landed and count what that is worth. Until then the shovel’s line is the pre-blast one.</p></div>
    <div data-agent="post" hidden>
      <p>Digging to the pre-blast line sends <b>${kt(outcome.pre.lost)}</b> of ore to the waste dump and <b>${kt(outcome.pre.waste)}</b> of waste to the mill. The moved line recovers ${kt(outcome.kept)} of that ore and keeps ${kt(outcome.avoided)} of that waste out: about <b class="bl-money">${money(outcome.value)}</b> on this blast${
        eng ? `, ${money(eng.lo)}–${money(eng.hi)} across the heave field’s 80% band` : ""
      }.</p>
      <p class="bl-terms">At $${fmt(ECON.price)}/oz, ${ECON.recovery * 100}% recovery and $${ECON.process}/t to process. <span class="ev estimated">estimated</span></p>
      <div class="bl-act" data-act><span class="act">Draft dig line DL-${PATTERN.name}-r1 · for the pit supervisor</span><button type="button" class="btn sm go" data-approve>Approve dig line</button></div>
      <p class="bl-done" data-done hidden>Approved by the pit supervisor. Surveyors peg the moved line for the shovel; the agent only drafted it.</p>
    </div>
  </div>
</div>
<div class="scene-bar bl-bar">
  <button type="button" class="btn go" data-fire><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m13 2-7 11h5l-1 9 8-12h-5.5z"/></svg>Fire</button>
  <button type="button" class="btn sm" data-reset disabled>Reset the bench</button>
  <button type="button" class="toggle" data-toggle="model" aria-pressed="true"><i></i>Grade model</button>
  <button type="button" class="toggle" data-toggle="delays" aria-pressed="true"><i></i>Delays</button>
  <button type="button" class="toggle" data-toggle="heave" aria-pressed="true"><i></i>Heave</button>
  <button type="button" class="toggle" data-toggle="survey" aria-pressed="true"><i></i>Survey</button>
  <p class="bl-note">Synthetic bench, true scale. Drag to look around.</p>
</div>`;

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("blast");

  /* ---------- FrankenMining's engine: the grade-control model and the heave field, estimated live ---------- */
  const eng: Eng | null = await (async () => {
    try {
      const [gm, hm] = await Promise.all([gradeModel(), heaveModel()]);
      const field = (u: number, sb: number, z = 0): [number, number] => [hm.along.at(u, sb), hm.out.at(u, sb) + z * hm.sdOut.at(u, sb)];
      adopt({ ore: gm.ore, grade: (u, sb) => gm.grade.at(u, sb), field: (u, sb) => field(u, sb) });
      const values = [outcome.value, ...[-1.28, 1.28].map((z) => count(ORE, moved(ORE, (u, sb) => field(u, sb, z))).value)];
      const band = ORE.reduce((a, [u, sb]) => a + 1.645 * hm.sdOut.at(u, sb), 0) / ORE.length;
      const tonnes = (poly: [number, number][]) => {
        let t = 0;
        for (let u = SPAN.uMin; u <= SPAN.uMax; u += 0.5) for (let sb = BACK; sb <= 0; sb += 0.5) if (inside(poly, u, sb)) t += 0.25 * BENCH.H * BENCH.rock;
        return t;
      };
      return { gm, hm, lo: Math.min(...values), hi: Math.max(...values), band, sureT: tonnes(gm.sure), maybeT: tonnes(gm.maybe) };
    } catch (e) {
      console.warn("blast: FrankenMining's engine did not answer; showing the synthetic stand-ins.", e);
      return null;
    }
  })();
  host.innerHTML = markup(eng);
  const small = matchMedia("(max-width: 699px)").matches;
  const wide = () => matchMedia("(min-width: 1000px)").matches;
  const pause = () => new Promise<void>((r) => setTimeout(r, 0));

  const s = stage($(host, ".bl-view"), {
    clock: 16.9,
    sunBearing: -141,
    fog: 0.00052,
    exposure: 1,
    shadowRadius: 130,
    shadowAt: [6, -6, -4],
    shadowMap: small ? 2048 : 4096,
    fov: small ? 44 : 34,
    maxPixelRatio: small ? 1.5 : 1.75,
    near: 1.5,
    label: "A blast on a benched open-pit wall: the drilled pattern fires hole by hole, the rock heaves into a muck pile and the dig lines move with it",
    labelArea: () => (wide() ? [0.24, 0.05, 0.76, 0.8] : [0.02, 0.14, 0.98, 0.97]),
  });
  s.orbit.minDistance = 40;
  s.orbit.maxDistance = 1400;
  s.orbit.minElevation = 2;
  s.orbit.maxElevation = 82;

  const world = new THREE.Group();
  s.scene.add(world);

  /* ---------- Where the crew waits: on the working bench, well round the pit ---------- */
  const crewAt = W(-514, -30);
  const ground = await makeGround({ cell: small ? 0.85 : 0.5, focus: [crewAt.x, crewAt.z], pause });
  world.add(ground.mesh);
  const H = ground.heightAt;
  const on = (v: THREE.Vector3, lift = 0) => v.setY(H(v.x, v.z) + lift);
  crewAt.y = H(crewAt.x, crewAt.z);
  await pause();

  /* ---------- Loose rock already lying about: spillage on the floor and bench, scree at the toes ---------- */
  {
    const r = rng(71);
    const pts: { x: number; y: number; z: number; size: number }[] = [];
    const add = (u: number, sb: number, size: number) => {
      const [x, z] = toWorld(u, sb);
      pts.push({ x, y: preAt(x, z) + size * 0.08, z, size });
    };
    const inMuck = (u: number, sb: number) => sb > BACK - 2 && sb < TOE + 10 && Math.abs(u - SPAN.mid) < SPAN.half + 12;
    for (const [c, n] of [[-118, 50], [-104, 60], [-90, 70], [-76, 80], [-62, 110], [0, 90], [95, 70]] as const) {
      for (let k = 0; k < (small ? n * 0.5 : n); k++) {
        const u = -230 + r() * 460;
        const sb = c + 5.2 + 0.3 + r() ** 1.6 * 4.2;
        if (inMuck(u, sb)) continue;
        add(u, sb, sizeOf(r, 0.3, 1.6, 2.4));
      }
    }
    for (let k = 0; k < (small ? 80 : 170); k++) {
      const u = -200 + r() * 400;
      const sb = 8 + r() * 80;
      if (inMuck(u, sb)) continue;
      add(u, sb, sizeOf(r, 0.2, 0.9, 2.2));
    }
    world.add(scatter(pts, { detail: small ? 0 : 1 }));
  }

  /* ---------- The data overlay, collars, harness, paint, pegs, monitors ---------- */
  const overlay = makeOverlay(ground);
  world.add(overlay.mesh);
  overlay.set(0, 1);
  const collars = makeCollars(H);
  world.add(collars);
  const harness = makeHarness(H, crewAt);
  world.add(harness);
  const oreWorld = ORE.map(([u, sb]) => toWorld(u, sb));
  const paint = new THREE.Mesh(ribbon(oreWorld, 0.42, H, { lift: 0.05, closed: true, step: 0.4 }), lineMaterial("#ff4f93", 0.92, true));
  paint.renderOrder = 3;
  world.add(paint);
  const pegsAlong = (poly: [number, number][], every: number, height: (x: number, z: number) => number) => {
    const out: THREE.Vector3[] = [];
    let run = every;
    for (let i = 0; i < poly.length; i++) {
      const [ax, az] = poly[i]!;
      const [bx, bz] = poly[(i + 1) % poly.length]!;
      run += Math.hypot(bx - ax, bz - az);
      if (run > every) {
        run = 0;
        out.push(new THREE.Vector3(ax + 0.5, height(ax + 0.5, az - 0.4), az - 0.4));
      }
    }
    return out;
  };
  const pegs = makePegs(pegsAlong(oreWorld, 9, H));
  world.add(pegs);
  const monCaps = new THREE.InstancedMesh(
    new THREE.CylinderGeometry(0.22, 0.26, 0.5, 10).translate(0, 0.25, 0),
    new THREE.MeshStandardMaterial({ color: "#3fb6f0", emissive: "#3fb6f0", emissiveIntensity: 0.35, roughness: 0.5 }),
    monitors.length,
  );
  {
    const m4 = new THREE.Matrix4();
    monitors.forEach((m, i) => {
      const p = on(W(m.u, m.s));
      m4.makeTranslation(p.x, p.y - 0.05, p.z);
      monCaps.setMatrixAt(i, m4);
    });
    monCaps.castShadow = true;
  }
  world.add(monCaps);

  /* ---------- Machines and people ---------- */
  const tangent = (u: number, sign = 1) => {
    const phi = u / 420;
    return Math.atan2(Math.cos(phi) * sign, Math.sin(phi) * sign);
  };
  const drill = makeDrillRig();
  drill.group.position.copy(on(W(150, -44)));
  drill.group.rotation.y = tangent(150, 1);
  drill.set({ mast: 0, jacks: 0, feed: 0.2, rodSpin: 0, dust: 0 });
  world.add(drill.group);
  const shovel = makeRopeShovel({ seed: 3 });
  const shovelAt = on(W(140, 50));
  shovel.group.position.copy(shovelAt);
  shovel.group.rotation.y = tangent(140, -1);
  shovel.set({ swing: 0, crowd: 0.3, hoist: 0.08, door: 0 });
  world.add(shovel.group);
  {
    const back = W(178, 72);
    const cable = makeTrailingCable({ from: on(W(147, 52)).add(new THREE.Vector3(0, 0.3, 0)), to: on(back).add(new THREE.Vector3(0, 0.2, 0)), slack: 0.2, seed: 4 });
    world.add(cable.group);
  }
  const pickup = makePickup();
  pickup.group.position.copy(on(W(-506, -36)));
  pickup.group.rotation.y = tangent(-482, -1);
  world.add(pickup.group);
  const people: Miner[] = [];
  const centre = W(SPAN.mid, -17);
  const crew: [Parameters<typeof makeMiner>[0], number, number, "tablet" | "radio" | "stand" | "point"][] = [
    [{ role: "blaster", seed: 3, detail: 2 }, -515, -27, "tablet"],
    [{ role: "supervisor", seed: 5, detail: 2 }, -518.5, -28.5, "radio"],
    [{ role: "blaster", seed: 8, detail: 1 }, -511, -25.5, "point"],
  ];
  for (const [opts, u, sb, work] of crew) {
    const m = makeMiner(opts);
    m.group.position.copy(on(W(u, sb)));
    world.add(m.group);
    m.face(centre);
    m.work(work, centre);
    people.push(m);
  }
  // The exclusion zone, 500 m round the pattern.
  const ring: [number, number][] = [];
  for (let a = 0; a <= 360; a += 0.6) {
    const r = 500;
    ring.push([centre.x + Math.sin((a * Math.PI) / 180) * r, centre.z + Math.cos((a * Math.PI) / 180) * r]);
  }
  const zone = new THREE.Mesh(ribbon(ring, 1.1, H, { lift: 0.15, dash: [6, 5], step: 1.5 }), lineMaterial("#fb7185", 0.6));
  world.add(zone);
  await pause();

  /* ---------- Delay tags ---------- */
  const tags = new Tags(
    holes.map((h) => ({ text: String(h.ms), at: new THREE.Vector3(h.x, H(h.x, h.z) + 2.1, h.z) })),
    small ? { width: 4.6, height: 1.87 } : { width: 3.7, height: 1.5 },
  );
  world.add(tags.mesh);

  /* ---------- The broken rock: rubble that rides the heave, rock thrown off the face and out of the collars ---------- */
  const r = rng(41);
  const holeTime = (u: number, sb: number) => {
    let best = Infinity;
    let t = 0;
    for (const h of holes) {
      const d = (u - h.u) ** 2 + (sb - h.s) ** 2;
      if (d < best) {
        best = d;
        t = h.ms / 1000;
      }
    }
    return t;
  };
  const P = ground.postHeightAt;
  const pieces: Piece[] = [];
  {
    const nRide = small ? 1300 : 3600;
    const nFace = small ? 600 : 1600;
    const nCollar = small ? 2 : 4;
    for (let k = 0; k < nRide; k++) {
      // Riders: the bench breaks into blocks that are carried a few metres by the heave onto the top of the muck.
      const u = SPAN.uMin + 1 + r() * (SPAN.half * 2 - 2);
      const sb = BACK + 0.8 + r() * (-BACK - 1.2);
      const [du, ds] = heave(u, sb);
      const [x0, z0] = toWorld(u, sb);
      const [x1, z1] = toWorld(u + du + (r() - 0.5) * 1.4, sb + ds + (r() - 0.5) * 1.4);
      const t0 = ground.fireAt(x0, z0);
      if (!Number.isFinite(t0)) continue;
      const bulge = (1.1 + 1.6 * clamp((sb + 20) / 20, 0, 1)) * 0.8;
      pieces.push({ kind: "ride", x0, z0, x1, z1, t0, bulge, size: sizeOf(r, 0.35, 2.0, 2.6) });
    }
    /** A piece thrown at speed v (m/s), angle th (rad above level) and heading (unit x, z), flown under gravity until
     * it comes down onto the finished muck: its landing found by stepping the arc and bisecting the last step. */
    const G = 9.81;
    const lob = (from: THREE.Vector3, v: number, th: number, hx: number, hz: number, t0: number, size: number) => {
      const vy = v * Math.sin(th);
      const vh = v * Math.cos(th);
      const at = (t: number) => [from.x + hx * vh * t, from.y + vy * t - 0.5 * G * t * t, from.z + hz * vh * t] as const;
      const gap = (t: number) => {
        const [x, y, z] = at(t);
        return y - (P(x, z) + size * 0.12);
      };
      // The muck it lands on is not there yet when it leaves, so look for it only once the piece is above where the
      // muck's surface will be; a piece that never gets above it simply goes with the bulk of the heave.
      let t = Math.max(0.1, vy / G);
      while (t < 4 && gap(t) <= 0) t += 0.04;
      if (gap(t) <= 0) return;
      let a = t;
      let b = t;
      while (b < 14 && gap(b) > 0) {
        a = b;
        b += 0.04;
      }
      for (let k = 0; k < 10; k++) {
        const m = (a + b) / 2;
        if (gap(m) > 0) a = m;
        else b = m;
      }
      const [x, y, z] = at(b);
      pieces.push({ kind: "fly", from, to: new THREE.Vector3(x, y, z), t0, vy, size });
    };
    const outDir = (u: number, sb: number, spread: number) => {
      const a = W(u, sb);
      const b = W(u, sb + 10);
      const ang = Math.atan2(b.z - a.z, b.x - a.x) + (r() - 0.5) * spread;
      return [Math.cos(ang), Math.sin(ang)] as const;
    };
    for (let k = 0; k < nFace; k++) {
      // Face burst: the front row's burden leaves the face at up to the Richards & Moore speed, nearly level, the
      // bigger blocks a little slower, and comes down on the floor in front of the old toe.
      const size = sizeOf(r, 0.35, 2.4, 2.4);
      const u = SPAN.uMin + r() * SPAN.half * 2;
      const sb = -1.5 + r() * 2.5;
      const from = W(u, sb);
      from.y = (sb < 0 ? 0 : preAt(from.x, from.z)) - r() * 6;
      const v = THROW.face * (0.3 + 0.7 * r() ** 0.6) * (1 - 0.22 * (size / 2.4));
      const [hx, hz] = outDir(u, sb, 0.35);
      lob(from, v, ((-6 + r() * 28) * Math.PI) / 180, hx, hz, holeTime(u, -5) + 0.025 + r() * 0.08, size);
    }
    for (const h of holes) {
      // Stemming ejection: rock torn from the collar, steeply up through the dust, back down onto the muck.
      for (let k = 0; k < nCollar; k++) {
        const size = sizeOf(r, 0.2, 0.6, 1.5);
        const from = new THREE.Vector3(h.x, H(h.x, h.z) + 0.4, h.z);
        const v = THROW.collar * (0.35 + 0.65 * r() ** 0.8);
        const [hx, hz] = outDir(h.u, h.s, 2.4);
        lob(from, v, ((58 + r() * 30) * Math.PI) / 180, hx, hz, h.ms / 1000 + 0.004 + r() * 0.02, size);
      }
    }
    for (let k = 0; k < (small ? 8 : 14); k++) {
      // A little fly rock off the face at the worst-case speed: tens of metres, short of the zone by hundreds.
      const u = SPAN.uMin + r() * SPAN.half * 2;
      const from = W(u, -1, -2 - r() * 6);
      const v = THROW.faceWorst * (0.75 + 0.25 * r());
      const [hx, hz] = outDir(u, 0, 0.5);
      lob(from, v, ((18 + r() * 22) * Math.PI) / 180, hx, hz, holeTime(u, -5) + 0.02, 0.3 + r() * 0.45);
    }
  }
  const rubble = new Rubble(pieces, { detail: small ? 0 : 1 });
  world.add(rubble.group);

  /* ---------- Dust, fume and flashes ---------- */
  const dust = new Dust(small ? 1700 : 3400);
  const glow = new Dust(260, { additive: true, lit: false });
  world.add(dust.mesh, glow.mesh);
  const flash = new THREE.PointLight("#ffb36a", 0, 220, 0);
  flash.position.copy(W(SPAN.mid, -14, 8));
  world.add(flash);
  const DUSTC = ["#8c7c69", "#7d6f60", "#998771", "#6f6356", "#a28f77", "#84776a"].map((c) => new THREE.Color(c));
  const JET = ["#4a423b", "#574d44", "#3f3832", "#625649"].map((c) => new THREE.Color(c));
  const FUME = new THREE.Color("#9c6a4b");
  const HOT = new THREE.Color("#ffc074");
  const CORE = new THREE.Color("#fff0c8");
  const pick = (set: THREE.Color[]) => set[Math.floor(r() * set.length)]!;
  const wind = (y: number, out: [number, number, number]) => {
    out[0] = 4.2 + 0.05 * Math.max(0, y);
    out[1] = 0;
    out[2] = 0.9 + 0.012 * Math.max(0, y);
  };
  const share = small ? 0.5 : 1;

  /* ---------- After the blast: lines, arrows and the monitors found ---------- */
  const post = new THREE.Group();
  post.visible = false;
  world.add(post);
  const preLine = new THREE.Mesh(ribbon(oreWorld, 0.6, P, { lift: 0.3, closed: true, dash: [2.2, 1.6] }), lineMaterial("#ffffff", 0.95));
  const movedWorld = ORE_MOVED.map(([u, sb]) => toWorld(u, sb));
  const movedMat = lineMaterial("#9be15d", 1);
  const movedLine = new THREE.Mesh(ribbon(movedWorld, 0.6, P, { lift: 0.34, closed: true }), movedMat);
  // Drawn over the rubble, so the lines read through the rocks lying on them.
  for (const m of [preLine.material, movedMat] as THREE.Material[]) m.depthTest = false;
  preLine.renderOrder = 6;
  movedLine.renderOrder = 7;
  post.add(preLine, movedLine);
  const was = monitors.map((m) => {
    const p = W(m.u, m.s);
    p.y = P(p.x, p.z) + 0.5;
    return p;
  });
  const found = monitors.map((m) => {
    const p = W(m.u + m.du, m.s + m.ds);
    p.y = P(p.x, p.z) + 0.75;
    return p;
  });
  const arrowsMat = arrowMaterial("#5ccbff");
  arrowsMat.depthTest = false;
  const arrows = new THREE.Mesh(
    arcArrows(
      monitors.map((_, i) => [was[i]!, found[i]!]),
      { r: 0.48, head: 2.1, rise: 2.4 },
    ),
    arrowsMat,
  );
  arrows.renderOrder = 9;
  // Where each monitor was buried: a ring on the muck above the spot.
  const ringsMat = lineMaterial("#ffffff", 0.9);
  ringsMat.depthTest = false;
  const rings = new THREE.Mesh(
    (() => {
      const parts: THREE.BufferGeometry[] = [];
      for (const p of was) {
        const g = new THREE.RingGeometry(0.75, 1.0, 28).rotateX(-Math.PI / 2);
        const a = g.getAttribute("position");
        for (let i = 0; i < a.count; i++) a.setY(i, P(p.x + a.getX(i), p.z + a.getZ(i)) + 0.2 - p.y);
        g.translate(p.x, p.y, p.z);
        parts.push(g);
      }
      const merged = new THREE.BufferGeometry();
      const pos: number[] = [];
      const idx: number[] = [];
      let base = 0;
      for (const g of parts) {
        const a = g.getAttribute("position");
        for (let i = 0; i < a.count; i++) pos.push(a.getX(i), a.getY(i), a.getZ(i));
        for (const v of g.index!.array) idx.push(v + base);
        base += a.count;
      }
      merged.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
      merged.setIndex(idx);
      return merged;
    })(),
    ringsMat,
  );
  rings.renderOrder = 8;
  const fieldPairs: [THREE.Vector3, THREE.Vector3][] = [];
  for (let u = SPAN.uMin + 5; u < SPAN.uMax - 3; u += 8.5)
    for (let sb = BACK + 4; sb < -2; sb += 6.4) {
      if (monitors.some((m) => Math.hypot(m.u - u, m.s - sb) < 5)) continue;
      const [du, ds] = heaveEstimate(u, sb);
      fieldPairs.push([W(u, sb), W(u + du, sb + ds)]);
    }
  const fieldMat = arrowMaterial("#a9e1ff", 0.55);
  fieldMat.depthTest = false;
  const field = new THREE.Mesh(flatArrows(fieldPairs, P, 0.34), fieldMat);
  field.renderOrder = 8;
  const gems = new THREE.InstancedMesh(
    new THREE.OctahedronGeometry(0.7, 0),
    new THREE.MeshBasicMaterial({ color: "#8fdcff", depthTest: false, transparent: true, toneMapped: false }),
    monitors.length,
  );
  gems.renderOrder = 10;
  found.forEach((p, i) => gems.setMatrixAt(i, new THREE.Matrix4().makeTranslation(p.x, p.y + 0.2, p.z)));
  const heaveGroup = new THREE.Group();
  heaveGroup.add(arrows, rings, field, gems);
  post.add(heaveGroup);
  // Once approved, the surveyors' pegs go in along the moved line.
  const newPegs = makePegs(pegsAlong(movedWorld, 9, P), "#7ee36b");
  newPegs.visible = false;
  post.add(newPegs);

  /* ---------- Survey overlay: dimensions, hole IDs, the model's confidence lines, the moved line's band ---------- */
  const surveyPre = new THREE.Group();
  const surveyPost = new THREE.Group();
  world.add(surveyPre);
  post.add(surveyPost);
  const surveyLabels: { l: ReturnType<typeof s.label>; post: boolean }[] = [];
  const dimMat = lineMaterial("#f6f1e7", 0.92, false, 0.006);
  const dimMatOver = lineMaterial("#f6f1e7", 0.95, false, 0.006);
  dimMatOver.depthTest = false;
  /** A surveyor's dimension: a line with end ticks draped on the ground, and its length or note at the middle. */
  const dim = (a: THREE.Vector3, b: THREE.Vector3, text: string, isPost: boolean, hf: (x: number, z: number) => number) => {
    const group = isPost ? surveyPost : surveyPre;
    const mat = isPost ? dimMatOver : dimMat;
    const l = Math.hypot(b.x - a.x, b.z - a.z) || 1;
    const nx = -(b.z - a.z) / l;
    const nz = (b.x - a.x) / l;
    const lay = (path: [number, number][]) => {
      const m = new THREE.Mesh(ribbon(path, 0.16, hf, { lift: 0.3, step: 0.3 }), mat);
      m.renderOrder = 11;
      group.add(m);
    };
    lay([[a.x, a.z], [b.x, b.z]]);
    for (const p of [a, b]) lay([[p.x - nx, p.z - nz], [p.x + nx, p.z + nz]]);
    const mid = new THREE.Vector3((a.x + b.x) / 2, 0, (a.z + b.z) / 2);
    mid.y = hf(mid.x, mid.z) + 0.4;
    surveyLabels.push({ l: s.label(text, mid, { className: "bl-dim" }), post: isPost });
  };
  {
    const row = (k: number) => holes.filter((h) => h.row === k).sort((a, b) => a.u - b.u);
    const r0 = row(0);
    const r1 = row(1);
    // Spacing along the front row, burden between the first two rows (outside the pattern's west end).
    const a = r0[4]!;
    const b = r0[5]!;
    dim(W(a.u, a.s + 2.2), W(b.u, b.s + 2.2), `Spacing <b>${fmt(PATTERN.spacing, 1)}&nbsp;m</b>`, false, H);
    const uw = Math.min(r0[0]!.u, r1[0]!.u) - 4;
    dim(W(uw, r0[0]!.s), W(uw, r1[0]!.s), `Burden <b>${fmt(PATTERN.burden, 1)}&nbsp;m</b>`, false, H);
    // The bench height, standing in front of the face at the pattern's west end.
    const foot = W(SPAN.uMin + 5, 8);
    foot.y = H(foot.x, foot.z);
    const top = BENCH.H + foot.y;
    const post3 = new THREE.Mesh(new THREE.CylinderGeometry(0.09, 0.09, top - foot.y, 6).translate(0, (top - foot.y) / 2, 0), new THREE.MeshBasicMaterial({ color: "#f6f1e7", toneMapped: false }));
    post3.position.copy(foot);
    surveyPre.add(post3);
    for (const y of [foot.y, top]) {
      const tick = new THREE.Mesh(new THREE.CylinderGeometry(0.07, 0.07, 1.8, 6).rotateZ(Math.PI / 2), post3.material);
      tick.position.set(foot.x, y, foot.z);
      surveyPre.add(tick);
    }
    surveyLabels.push({ l: s.label(`Bench <b>${fmt(BENCH.H)}&nbsp;m</b>`, foot.clone().setY((foot.y + top) / 2), { className: "bl-dim" }), post: false });
    // The model's 90%- and 10%-sure lines either side of the painted one.
    if (eng) {
      const conf = lineMaterial("#ffc2da", 0.62, false, 0.0045);
      for (const poly of [eng.gm.sure, eng.gm.maybe])
        if (poly.length > 8) surveyPre.add(new THREE.Mesh(ribbon(poly.map(([u, sb]) => toWorld(u, sb)), 0.11, H, { lift: 0.06, closed: true, dash: [0.45, 0.75], step: 0.15 }), conf));
    }
  }
  // Hole IDs as a pattern plan marks them: every hole of the front row, and the first hole of each row behind it.
  const ids = new Tags(
    holes
      .filter((h) => h.row === 0 || h.i === 0)
      .map((h) => ({ text: `${h.row + 1}-${String(h.i + 1).padStart(2, "0")}`, at: new THREE.Vector3(h.x, H(h.x, h.z) + 0.8, h.z) })),
    small ? { width: 3.2, height: 1.3 } : { width: 2.5, height: 1.02 },
  );
  ids.material.uniforms.fade!.value = 0.8;
  surveyPre.add(ids.mesh);
  if (eng) {
    // The moved line's 90% band from the engine's heave field.
    const lo = moved(ORE, (u, sb) => [eng.hm.along.at(u, sb), eng.hm.out.at(u, sb) - 1.645 * eng.hm.sdOut.at(u, sb)]);
    const hi = moved(ORE, (u, sb) => [eng.hm.along.at(u, sb), eng.hm.out.at(u, sb) + 1.645 * eng.hm.sdOut.at(u, sb)]);
    const pos: number[] = [];
    const n = lo.length;
    for (let i = 0; i < n; i++) {
      const j = (i + 1) % n;
      const q = [lo[i]!, hi[i]!, lo[j]!, hi[j]!].map(([u, sb]) => {
        const [x, z] = toWorld(u, sb);
        return [x, P(x, z) + 0.28, z];
      });
      pos.push(...q[0]!, ...q[1]!, ...q[2]!, ...q[1]!, ...q[3]!, ...q[2]!);
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
    const bandMat = lineMaterial("#9be15d", 0.22, false, 0.004);
    bandMat.depthTest = false;
    const band = new THREE.Mesh(g, bandMat);
    band.renderOrder = 6;
    surveyPost.add(band);
  }
  {
    // How far the line moved at the front of the ore, measured square to it.
    let best = 0;
    ORE.forEach(([u, sb], i) => {
      if (Math.abs(u - SPAN.mid - 6) < 10 && sb > ORE[best]![1]) best = i;
    });
    const a = W(ORE[best]![0], ORE[best]![1]);
    const b = W(ORE_MOVED[best]![0], ORE_MOVED[best]![1]);
    dim(a, b, `Moved <b>${fmt(Math.hypot(b.x - a.x, b.z - a.z), 1)}&nbsp;m</b>`, true, P);
  }
  const movedAnchor = (() => {
    let best = 0;
    ORE_MOVED.forEach(([u, sb], i) => {
      if (Math.abs(u - SPAN.mid - 22) < 8 && sb > ORE_MOVED[best]![1]) best = i;
    });
    const [u, sb] = ORE_MOVED[best]!;
    return W(u, sb).setY(P(...toWorld(u, sb)));
  })();

  /* ---------- Labels ---------- */
  const tagL = (html: string, at: THREE.Vector3 | THREE.Object3D, y: number) => s.label(html, at, { y, className: "stem" });
  const ev = (k: string) => `<span class="ev ${k}">${k}</span>`;
  const labels = {
    pattern: tagL(`Pattern ${PATTERN.name} <b>${facts.holes} holes</b>`, on(W(-8, -30)), 6),
    ore: tagL(`Ore boundary, from the model ${ev("estimated")}`, on(W(-46, -9)), 2.5),
    drill: tagL(`Drill D2 <b>parked back</b>`, drill.group, 8),
    shovel: tagL(`Shovel S1 <b>waits to dig</b>`, shovel.group, 22),
    crew: tagL(`Firing point <b>${fmt(crewAt.distanceTo(centre))}&nbsp;m</b> zone clear ${ev("measured")}`, people[0]!.group, 3),
    monitors: tagL(`${monitors.length} movement monitors buried`, on(W(monitors[2]!.u, monitors[2]!.s)), 2.4),
    muck: tagL(`Muck pile <b>${fmt(facts.tonnes / 1000)}&nbsp;kt</b> broken`, W(SPAN.mid + 26, 16, P(...toWorld(SPAN.mid + 26, 16))), 4),
    moved: tagL(`Moved dig line ${ev("estimated")}`, movedAnchor, 3),
    pre: tagL(`Pre-blast line`, W(-30, -18).setY(P(...toWorld(-30, -18))), 3),
    m: [1, 3, 6].map((i) => {
      const m = monitors[i]!;
      return tagL(`${m.id} moved <b>${fmt(Math.hypot(m.du, m.ds), 1)}&nbsp;m</b> ${ev("measured")}`, found[i]!, 1.8);
    }),
  };
  const preLabels = [labels.pattern, labels.ore, labels.monitors];
  /** Labels in the order they matter; a label that would overlap one before it, or run off the view, stands down. */
  const byPriority = () => [labels.crew, labels.moved, labels.pre, labels.pattern, labels.ore, ...labels.m, labels.muck, labels.monitors, ...surveyLabels.map((x) => x.l), labels.shovel, labels.drill];
  const declutter = () => {
    const view = s.view.getBoundingClientRect();
    const taken: DOMRect[] = [];
    for (const l of byPriority()) {
      const e = l.el;
      if (e.classList.contains("off") || e.style.visibility === "hidden") {
        e.classList.remove("bl-crowd");
        continue;
      }
      const b = e.getBoundingClientRect();
      const out = b.left < view.left + 4 || b.right > view.right - 4 || b.top < view.top + 4 || b.bottom > view.bottom - 4;
      const hit = taken.some((t) => b.left < t.right + 4 && b.right > t.left - 4 && b.top < t.bottom + 3 && b.bottom > t.top - 3);
      e.classList.toggle("bl-crowd", out || hit);
      if (!out && !hit) taken.push(b);
    }
  };
  const postLabels = [labels.moved, labels.pre, labels.muck, ...labels.m];
  /** Which labels stand in each phase: the plan before, nothing over the blast itself, the verdicts after. */
  const showLabels = (p: Phase) => {
    // A phone has room for the line labels only; the cards under the scene carry the rest.
    const narrow = s.view.clientWidth < 560;
    // On a phone the pattern's label hangs from its west end, so it stays inside the narrow view.
    labels.pattern.anchor(on(W(narrow ? -34 : -8, -30)), { y: 6 });
    for (const l of preLabels) l.show(p === "ready" && (!narrow || l === labels.pattern));
    for (const l of postLabels) l.show(p === "settled" && (!narrow || l === labels.moved || l === labels.pre));
    labels.crew.show(p === "ready" || p === "arming");
    // The planning marks lie on rock that is about to move; once it has, they are gone with it.
    collars.visible = harness.visible = pegs.visible = paint.visible = monCaps.visible = p === "ready" || p === "arming" || p === "firing";
    labels.drill.show(p !== "arming" && p !== "firing" && p !== "throw");
    labels.shovel.show(p !== "arming" && p !== "firing" && p !== "throw");
    applySurvey(p);
  };
  const applySurvey = (p: Phase) => {
    const pre = show.survey && p === "ready";
    const after = show.survey && p === "settled";
    surveyPre.visible = pre;
    surveyPost.visible = after;
    const narrow = s.view.clientWidth < 560;
    for (const { l, post: isPost } of surveyLabels) l.show(!narrow && (isPost ? after : pre));
  };

  /* ---------- Controls and HUD ---------- */
  const el = {
    fire: $<HTMLButtonElement>(host, "[data-fire]"),
    reset: $<HTMLButtonElement>(host, "[data-reset]"),
    ms: $(host, "[data-ms]"),
    rate: $(host, "[data-rate]"),
    fired: $(host, "[data-fired]"),
    count: $(host, ".bl-count"),
    mini: $(host, "[data-mini]"),
    miniRate: $(host, "[data-mini-rate]"),
    digPre: $(host, '[data-dig="pre"]'),
    digPost: $(host, '[data-dig="post"]'),
    agentPre: $(host, '[data-agent="pre"]'),
    agentPost: $(host, '[data-agent="post"]'),
    act: $(host, "[data-act]"),
    approve: $<HTMLButtonElement>(host, "[data-approve]"),
    done: $(host, "[data-done]"),
    dots: [...host.querySelectorAll<SVGCircleElement>(".bl-timing circle")],
  };
  const show = { model: true, delays: true, heave: true, survey: !small };
  $(host, '[data-toggle="survey"]').setAttribute("aria-pressed", String(show.survey));
  for (const b of host.querySelectorAll<HTMLButtonElement>("[data-toggle]")) {
    b.addEventListener("click", () => {
      const k = b.dataset.toggle as keyof typeof show;
      show[k] = !show[k];
      b.setAttribute("aria-pressed", String(show[k]));
      if (k === "survey") applySurvey(phase);
      s.loop.poke();
      if (reduced) paintStill();
    });
  }

  /* ---------- Camera ---------- */
  const far = small ? 1.45 : 1;
  const views: Record<"ready" | "firing" | "firing2" | "throw" | "plume" | "settled", View> = {
    ready: { target: [6, -5, -12], distance: 190 * far, azimuth: 26, elevation: 25 },
    firing: { target: [0, -6, -8], distance: 150 * far, azimuth: 40, elevation: 22 },
    firing2: { target: [2, -4, -6], distance: 128 * far, azimuth: 30, elevation: 20 },
    throw: { target: [8, 6, 0], distance: 215 * far, azimuth: 36, elevation: 15 },
    plume: { target: [40, 38, -10], distance: 410 * (small ? 1.25 : 1), azimuth: 34, elevation: 11 },
    settled: { target: [0, -7, -8], distance: small ? 196 : 158, azimuth: 10, elevation: small ? 60 : 50 },
  };
  const firingPoint = (): View => {
    const d = crewAt.distanceTo(centre) + 38;
    const az = (Math.atan2(crewAt.x - centre.x, crewAt.z - centre.z) * 180) / Math.PI;
    const elv = (Math.atan2(crewAt.y + 8 - centre.y, d) * 180) / Math.PI;
    return { target: [centre.x, centre.y, centre.z], distance: d, azimuth: az, elevation: elv };
  };
  s.orbit.set(views.ready);

  /* ---------- The timeline ---------- */
  let phase: Phase = "ready";
  let T = -1; // blast time, s
  let since = 0; // real seconds in this phase
  let rate = 0;
  let firedCount = 0;
  let shake = 0;
  let flashI = 0;
  let plumeOn = 0;
  let skirtOn = 0;
  let reveal = 0;
  const firedAt = new Float32Array(holes.length).fill(-1);
  const go = (p: Phase) => {
    phase = p;
    since = 0;
    if (p === "arming") s.orbit.set(firingPoint(), 0);
    if (p === "firing") {
      s.orbit.set(views.firing, 0);
      s.orbit.set(views.firing2, 10);
    }
    if (p === "throw") s.orbit.set(views.throw, 4.5);
    if (p === "plume") s.orbit.set(views.plume, 8);
    if (p === "settled") {
      s.orbit.set(views.settled, 3.5);
      el.digPre.hidden = true;
      el.digPost.hidden = false;
      el.agentPre.hidden = true;
      el.agentPost.hidden = false;
      el.reset.disabled = false;
    }
    host.dataset.phase = p;
    showLabels(p);
  };

  const emitHole = (i: number) => {
    const h = holes[i]!;
    const y = H(h.x, h.z);
    const front = h.row === 0;
    // A brief flash at the collar: the stemming lets go.
    glow.emit({ x: h.x, y: y + 0.8, z: h.z, life: 0.04, size: [1.4, 3.4], alpha: 1, colour: CORE, drag: 0, fadeIn: 0.02, cloud: false });
    glow.emit({ x: h.x, y: y + 2.5, z: h.z, vy: 40, life: 0.06, size: [2.2, 6], alpha: 0.5, colour: HOT, drag: 0, fadeIn: 0.02, cloud: false });
    // Dust lifting off the cracking bench round the hole.
    for (let k = 0; k < 4; k++)
      dust.emit({
        x: h.x + (r() - 0.5) * 7,
        y: y + 1,
        z: h.z + (r() - 0.5) * 6,
        vy: 1.5 + r() * 3,
        life: 7 + r() * 5,
        size: [3, 12 + r() * 8],
        alpha: 0.38,
        colour: pick(DUSTC),
        drag: 0.8,
        lift: 0.5,
        fadeIn: 0.12,
      });
    if (r() < 0.18)
      dust.emit({ x: h.x, y: y + 3, z: h.z, vy: 6 + r() * 6, life: 9 + r() * 4, size: [2, 12 + r() * 6], alpha: 0.3, colour: FUME, drag: 1, lift: 1.6, fadeIn: 0.05 });
    if (front) {
      // The face bursts: a wall of dust rolls out across the floor.
      const out = W(h.u, 30, 0).sub(W(h.u, 0, 0)).normalize();
      for (let k = 0; k < 12; k++) {
        const p = W(h.u + (r() - 0.5) * 7, 0.5 + r() * 4, -1 - r() * 12);
        const v = 9 + r() * 15;
        dust.emit({
          x: p.x,
          y: p.y,
          z: p.z,
          vx: out.x * v + (r() - 0.5) * 3,
          vy: -1 + r() * 5,
          vz: out.z * v,
          life: 8 + r() * 6,
          size: [3, 16 + r() * 12],
          alpha: 0.5,
          colour: pick(DUSTC),
          drag: 0.9,
          lift: 0.5,
          fadeIn: 0.04,
        });
      }
    }
    flashI = Math.min(1.4, flashI + 0.45);
    shake = Math.max(shake, front ? 0.3 : 0.16);
  };
  /** The stemming and gas vent from each collar for a moment after it fires: a dark jet tens of metres up. */
  let ventOn = 0;
  const vent = (bdt: number) => {
    ventOn += bdt * 110;
    const n = Math.floor(ventOn);
    ventOn -= n;
    for (let k = 0; k < n; k++) {
      for (let i = 0; i < holes.length; i++) {
        const age = T - (firedAt[i] ?? -1);
        if (firedAt[i]! < 0 || age > 0.16) continue;
        const h = holes[i]!;
        const y = H(h.x, h.z);
        const fade = 1 - age / 0.16;
        dust.emit({
          x: h.x + (r() - 0.5) * 0.6,
          y: y + 0.8,
          z: h.z + (r() - 0.5) * 0.6,
          vx: (r() - 0.5) * 7,
          vy: (16 + r() * 34) * (0.5 + 0.5 * fade),
          vz: (r() - 0.5) * 7 + (h.row === 0 ? 2.5 : 0),
          life: 5 + r() * 4,
          size: [1.8 + r() * 1.4, 12 + r() * 11],
          alpha: 0.5,
          colour: r() < 0.05 ? FUME : pick(JET),
          drag: 1.4,
          lift: 0.9,
          fadeIn: 0.012,
        });
      }
    }
  };
  const plumePuff = (n: number) => {
    for (let k = 0; k < n; k++) {
      // Rising from rock that has already fired: most of it a broad, low, dense cloud that spreads and drifts, some
      // of it a hot column that climbs before the wind takes it.
      let h = holes[Math.floor(r() * holes.length)]!;
      for (let g = 0; g < 4 && firedAt[holes.indexOf(h)]! < 0; g++) h = holes[Math.floor(r() * holes.length)]!;
      const sb = h.s + (r() - 0.35) * 9;
      const p = W(h.u + (r() - 0.5) * 8, sb);
      const column = r() < 0.26;
      const a = r() * Math.PI * 2;
      const spread = column ? 1.5 + r() * 2 : 2 + r() * 5;
      dust.emit({
        x: p.x,
        y: H(p.x, p.z) + 2 + r() * (column ? 7 : 4),
        z: p.z,
        vx: Math.cos(a) * spread,
        vy: column ? 6 + r() * 9 : 2 + r() * 4,
        vz: Math.sin(a) * spread + 1,
        life: 18 + r() * 12,
        size: column ? [6 + r() * 5, 34 + r() * 28] : [8 + r() * 5, 30 + r() * 22],
        alpha: column ? 0.42 : 0.46,
        colour: r() < 0.03 ? FUME : pick(DUSTC),
        drag: column ? 0.42 : 0.28,
        lift: column ? 0.8 + r() * 1.4 : 0.2 + r() * 0.6,
        fadeIn: 0.05,
      });
    }
  };
  const skirtPuff = (n: number) => {
    for (let k = 0; k < n; k++) {
      // The low cloud along the front of the muck, spreading over the floor.
      const u = SPAN.uMin - 4 + r() * (SPAN.half * 2 + 8);
      const sb = 4 + r() * (TOE - 4);
      const p = W(u, sb);
      const out = W(u, sb + 10).sub(p).normalize();
      const v = 2 + r() * 5;
      dust.emit({
        x: p.x,
        y: H(p.x, p.z) + 1.5,
        z: p.z,
        vx: out.x * v,
        vy: 0.6 + r() * 2,
        vz: out.z * v,
        life: 13 + r() * 8,
        size: [8, 30 + r() * 18],
        alpha: 0.4,
        colour: pick(DUSTC),
        drag: 0.6,
        lift: 0.35,
        fadeIn: 0.08,
      });
    }
  };

  const setPost = (on2: boolean) => {
    post.visible = on2;
    collars.visible = harness.visible = pegs.visible = paint.visible = monCaps.visible = !on2;
  };

  const fire = () => {
    if (phase !== "ready") return;
    el.fire.disabled = true;
    el.reset.disabled = true;
    if (reduced) {
      T = 999;
      ground.blast(Infinity);
      rubble.pose(999, H, P);
      for (let i = 0; i < holes.length; i++) {
        tags.set(i, 2);
        firedAt[i] = 1;
      }
      firedCount = holes.length;
      reveal = 1;
      go("settled");
      s.orbit.set(views.settled, 0);
      paintStill();
      return;
    }
    go("arming");
  };
  const reset = () => {
    phase = "ready";
    host.dataset.phase = "ready";
    showLabels("ready");
    T = -1;
    since = 0;
    firedCount = 0;
    firedAt.fill(-1);
    plumeOn = 0;
    skirtOn = 0;
    ventOn = 0;
    reveal = 0;
    ground.blast(-1);
    rubble.pose(-1, H, P);
    dust.clear();
    glow.clear();
    for (let i = 0; i < holes.length; i++) tags.set(i, 0);
    setPost(false);
    newPegs.visible = false;
    movedMat.color.set("#9be15d");
    el.digPre.hidden = false;
    el.digPost.hidden = true;
    el.agentPre.hidden = false;
    el.agentPost.hidden = true;
    el.act.hidden = false;
    el.done.hidden = true;
    el.fire.disabled = false;
    el.reset.disabled = true;
    el.count.textContent = "";
    el.count.classList.remove("on");
    labels.shovel.html(`Shovel S1 <b>waits to dig</b>`);
    s.orbit.set(views.ready, reduced ? 0 : 2);
    hud();
    if (reduced) paintStill();
  };
  el.fire.addEventListener("click", fire);
  el.reset.addEventListener("click", reset);
  el.approve.addEventListener("click", () => {
    el.act.hidden = true;
    el.done.hidden = false;
    labels.shovel.html(`Shovel S1 <b>digs to the moved line</b>`);
    movedMat.color.set("#5fd38d");
    newPegs.visible = true;
    s.loop.poke();
    if (reduced) paintStill();
  });

  const hud = () => {
    const ms = T < 0 ? 0 : Math.min(T * 1000, 99_999);
    el.ms.textContent = phase === "ready" || phase === "arming" ? "0" : fmt(phase === "firing" || phase === "settled" ? Math.min(ms, lastMs) : ms);
    el.fired.textContent = String(firedCount);
    const r2 =
      phase === "ready" ? "ready" : phase === "arming" ? "zone clear" : rate < 0.2 ? `slow motion ×1/${Math.round(1 / rate)}` : rate < 1.4 ? "real time" : `time-lapse ×${fmt(rate, 1)}`;
    el.rate.textContent = phase === "settled" ? (firedCount === holes.length ? "all fired, no misfires" : `${firedCount} fired`) : r2;
    el.mini.textContent =
      phase === "ready" ? "Ready to fire" : phase === "arming" ? "Zone clear" : phase === "settled" ? "Settled · heave measured" : `${fmt(ms)} ms · ${firedCount}/${holes.length}`;
    el.miniRate.textContent = phase === "settled" || phase === "ready" || phase === "arming" ? "" : r2;
    el.dots.forEach((d, i) => d.classList.toggle("on", firedAt[i]! >= 0));
  };

  /* ---------- Each frame ---------- */
  const sunDir = new THREE.Vector3();
  const sunC = new THREE.Color();
  const ambC = new THREE.Color();
  let hudT = 0;
  let crowdT = 0;
  const step = (dt: number) => {
    since += dt;
    // How fast blast time runs: slow through the sequence, real time for the throw, a time-lapse for the plume.
    rate =
      phase === "firing" ? 0.09 : phase === "throw" ? mix(0.09, 1, smooth(since / 1.4)) : phase === "plume" ? mix(1, 2.6, smooth(since / 2)) : phase === "settled" ? 2.6 : 0;
    if (phase === "arming") {
      const left = 3 - Math.floor(since / 0.85);
      el.count.classList.add("on");
      el.count.innerHTML = since < 0.85 ? `<small>Siren sounded · guards posted</small>All clear` : left > 0 ? `<small>Firing in</small>${left}` : "";
      if (since > 3.4) {
        el.count.classList.remove("on");
        T = -0.004;
        go("firing");
      }
    }
    if (phase === "firing" || phase === "throw" || phase === "plume" || phase === "settled") {
      const bdt = dt * rate;
      const prev = T;
      T += bdt;
      holes.forEach((h, i) => {
        const t = h.ms / 1000;
        if (prev < t && T >= t) {
          firedAt[i] = T;
          firedCount++;
          emitHole(i);
        }
        if (firedAt[i]! >= 0) tags.set(i, T - t < 0.06 ? 1 : 1 + smooth((T - t - 0.06) / 0.08));
      });
      vent(bdt);
      if (T > 0.12 && T < 3.8) {
        plumeOn += bdt * (T < 1.3 ? 230 : 90) * share;
        const n = Math.floor(plumeOn);
        plumeOn -= n;
        plumePuff(n);
      }
      if (T > 0.45 && T < 2.6) {
        skirtOn += bdt * 70 * share;
        const n = Math.floor(skirtOn);
        skirtOn -= n;
        skirtPuff(n);
      }
      ground.blast(Math.min(T, ground.end));
      rubble.pose(T, H, P);
      dust.update(bdt, wind, H);
      glow.update(bdt, wind, H);
      if (phase === "firing" && T > lastMs / 1000 + 0.05) go("throw");
      if (phase === "throw" && T > 2.6) go("plume");
      if (phase === "plume" && (T > 16 || (T > 11 && dust.count < 500))) go("settled");
      if (phase === "settled") {
        reveal = Math.min(1, reveal + dt / 1.8);
        if (!post.visible) setPost(true);
      }
    }
    // Light from the explosions, and a tremor in the ground.
    flashI *= Math.exp(-dt * (phase === "firing" ? 2.6 : 6));
    flash.intensity = flashI * 1.6;
    shake *= Math.exp(-dt * 7);
    world.position.set((r() - 0.5) * shake, (r() - 0.5) * shake * 0.6, (r() - 0.5) * shake);
    // Fade the planning marks as the rock moves; bring the verdicts in after.
    const moving = phase === "firing" || phase === "throw" ? 1 - smooth((T - 0.05) / 0.6) : phase === "plume" ? 0 : 1;
    overlay.set(post.visible ? 1 : 0, (post.visible ? reveal : moving) * (show.model ? 1 : 0));
    tags.fade(show.delays ? (phase === "ready" || phase === "arming" || phase === "firing" ? 1 : phase === "throw" ? 1 - smooth(since / 2) : 0) : 0);
    arrowsMat.uniforms.reveal!.value = reveal;
    fieldMat.uniforms.reveal!.value = clamp(reveal * 1.4 - 0.4, 0, 1);
    ringsMat.opacity = 0.9 * smooth(reveal * 3);
    heaveGroup.visible = show.heave;
    for (const m of people) m.update(dt);
    shovel.update(dt);
    drill.update(dt);
    hudT += dt;
    if (hudT > 0.06) {
      hudT = 0;
      hud();
    }
    // The near plane as far out as the view allows: depth precision is what keeps close surfaces from fighting.
    const cam = s.camera;
    const above = cam.position.y - H(cam.position.x, cam.position.z);
    const near = clamp(Math.min(cam.position.distanceTo(s.orbit.target) * 0.03, above * 0.5), 0.5, 12);
    if (Math.abs(near / cam.near - 1) > 0.08) {
      cam.near = near;
      cam.updateProjectionMatrix();
    }
    crowdT += dt;
    if (crowdT > 0.2) {
      crowdT = 0;
      declutter();
    }
  };

  s.frame((dt) => {
    step(dt);
    // Light the dust with the scene's own sun and sky.
    sunDir.copy(s.sun.position).sub(s.sun.target.position).normalize();
    sunC.copy(s.sun.color).multiplyScalar((s.sun.intensity / Math.PI) * 1.05);
    ambC.copy(s.hemi.color).multiplyScalar(s.hemi.intensity * 0.42).add(new THREE.Color(0.035, 0.032, 0.03));
    s.camera.updateMatrixWorld();
    dust.light(s.camera, sunDir, sunC, ambC);
    dust.draw(s.camera);
    glow.draw(s.camera);
  });

  // A still for reduced motion: pose everything for the current state and draw once.
  const paintStill = () => {
    step(0);
    s.render();
  };

  setPost(false);
  showLabels("ready");
  hud();
  s.start();
  if (reduced) paintStill();
}
