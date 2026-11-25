/**
 * The words and panels of the haul scene: its markup, the custody card for a truck's load, the dispatch agent's
 * note, the engineering card and the small formatters they share. Strings only; nothing touches the DOM at import.
 */
import { fmt } from "./lib";
import type { CycleParts, PayloadStats, Sizing } from "./haul-agent";
import type { BlockEstimate } from "./haul-grade";
import type { Custody, Loader, Truck } from "./opencut";

/** Mine seconds to a clock reading, given the clock (hours) at mine second 0. */
export const clockAt = (start: number, t: number, seconds = true) => {
  const h = (((start + t / 3600) % 24) + 24) % 24;
  const H = Math.floor(h);
  const M = Math.floor((h - H) * 60);
  const S = Math.floor(((h - H) * 60 - M) * 60);
  const two = (n: number) => String(n).padStart(2, "0");
  return seconds ? `${two(H)}:${two(M)}:${two(S)}` : `${two(H)}:${two(M)}`;
};
/** A span of seconds as "4 min 06 s", "38 s" or "1 h 12 min". */
export const span = (s: number) => {
  const v = Math.max(0, Math.round(s));
  if (v < 60) return `${v} s`;
  if (v < 3600) return `${Math.floor(v / 60)} min ${String(v % 60).padStart(2, "0")} s`;
  return `${Math.floor(v / 3600)} h ${String(Math.floor((v % 3600) / 60)).padStart(2, "0")} min`;
};
/** Minutes and seconds, "3:42". */
const ms = (s: number) => {
  const v = Math.max(0, Math.round(s));
  return `${Math.floor(v / 60)}:${String(v % 60).padStart(2, "0")}`;
};
export const tonnes = (t: number) => `${fmt(t)} t`;
export const DEST: Record<string, string> = { crusher: "Crusher CR1", dump: "Waste dump WD2" };
export const LOADER: Record<string, string> = { S1: "S1 rope shovel", L2: "L2 face shovel" };

export const markup = () => `
<div class="hl-stage">
  <div class="scene-view hl-view">
    <div class="hud-card hud-tl hl-shift">
      <p class="micro">Day shift · synthetic mine</p>
      <p class="hl-clock"><b data-clock>16:40</b><span data-left></span></p>
      <dl class="hl-tally">
        <dt>Ore to crusher CR1</dt><dd data-t="ore">—</dd>
        <dt>Waste to dump WD2</dt><dd data-t="waste">—</dd>
        <dt>Loads tipped, 30 min</dt><dd data-t="loads">—</dd>
      </dl>
      <div class="hl-q">
        <p class="hl-qh"><span>Queued at S1</span><b data-t="queue">—</b></p>
        <svg class="hl-spark" viewBox="0 0 220 40" preserveAspectRatio="none" aria-hidden="true" data-spark></svg>
        <p class="hl-qf"><span>30 min ago</span><span>now</span></p>
      </div>
      <p class="hl-src">Last 30 minutes of the fleet system’s time records <span class="ev measured">measured</span></p>
    </div>
    <p class="hl-hint" data-hint data-avoid>Tap a truck to follow it</p>
    <p class="hl-msg" data-msg hidden></p>
  </div>
  <aside class="hl-side">
    <div class="hud-card hl-load" data-load aria-live="polite"></div>
    <div class="hud-card agent-note hl-agent" data-agent></div>
    <div class="hud-card hl-eng" data-eng hidden></div>
  </aside>
</div>
<div class="scene-bar hl-bar">
  <div class="hl-fleet" role="group" aria-label="Follow a truck" data-fleet></div>
  <div class="hl-ctl">
    <div class="seg" role="group" aria-label="View" data-seg="view"><button type="button" data-v="bay" aria-pressed="true">Loading area</button><button type="button" data-v="ramp" aria-pressed="false">Ramp</button><button type="button" data-v="crusher" aria-pressed="false">Crusher</button></div>
    <div class="seg" role="group" aria-label="Speed" data-seg="pace"><button type="button" data-v="1" aria-pressed="true">Real time</button><button type="button" data-v="4" aria-pressed="false">×4</button><button type="button" data-v="16" aria-pressed="false">×16</button></div>
    <button type="button" class="hl-engbtn" aria-pressed="false" data-eng-toggle><i></i>Engineering view</button>
  </div>
  <p class="hl-note">Synthetic mine. Trucks run on rimpull and retarding physics; grades come from FrankenMining’s engine, compiled to WebAssembly and running in your browser. Drag to look round.</p>
</div>`;

/** What a truck is doing, as a state for its chip and dot. */
export const stateOf = (t: Truck) =>
  t.act === "load" ? "load" : t.act === "queue" && (t.stretch === "in" || t.stretch === "transfer") ? "queue" : t.act === "dump" ? "tip" : t.payloadT > 0 ? "full" : "empty";

const ev = (k: string, text = k) => `<span class="ev ${k}">${text}</span>`;
const row = (state: "done" | "now" | "", label: string, value: string, extra = "") =>
  `<li class="${state}"><i></i><span>${label}</span><b>${value}</b>${extra}</li>`;

/** The engine's grade for a block, as the custody card shows it. */
const gradeText = (c: Custody, est: BlockEstimate | null | undefined, cutoff: number) => {
  if (!est) return `${fmt(c.grade.mean, 2)} g/t · block model; engine loading`;
  if (c.material === "ore") return `${fmt(est.mean, 2)} g/t Au · P10–P90 ${fmt(est.p10, 2)}–${fmt(est.p90, 2)}`;
  return `${fmt(est.mean, 2)} g/t · ${fmt(Math.max(0.1, est.pCut * 100), est.pCut < 0.01 ? 1 : 0)}% chance of ≥ ${fmt(cutoff, 2)}`;
};

/** The chain of custody of a truck's load: the one it carries, or the last it delivered. */
export function custody(
  t: Truck,
  start: number,
  now: number,
  eta: number | null,
  X: Loader,
  o: { watching: boolean; pending: string | null; est?: BlockEstimate | null; rated: number; cutoff: number; engineMs?: number },
) {
  const c: Custody | null = t.load ?? t.last;
  const kmh = Math.abs(t.speed) * 3.6;
  const moving = kmh >= 1 ? ` · ${fmt(kmh)} km/h` : "";
  const head = `<header class="hl-lh"><span class="hl-id" data-state="${stateOf(t)}">${t.id}</span><div><b>Haul truck, 360 t class</b><small>${t.doing}${moving}</small></div>${
    o.watching ? "" : `<button type="button" class="hl-x" data-unfollow aria-label="Stop following ${t.id}">×</button>`
  }</header>`;
  const lead = "";
  const assign = `<p class="hl-asg">Assigned to <b>${LOADER[t.loader] ?? t.loader}</b>${o.pending ? ` · moves to <b>${o.pending}</b> after this load` : ""}</p>`;
  if (!c) return `${head}${lead}${assign}<p class="hl-src">No load yet in this view.</p>`;
  const open = c === t.load;
  const planned = X.id === c.loader && X.loading === t ? X.passes : c.passes.length || X.plan || X.passes;
  const passes = Array.from({ length: Math.max(planned, c.passes.length) }, (_, i) => {
    const p = c.passes[i];
    const top = i >= (X.plan ?? 99);
    return `<em class="${p ? "on" : ""}${top ? " top" : ""}" title="${top ? "Top-up pass" : `Pass ${i + 1}`}">${p ? fmt(p.tonnes) : "·"}</em>`;
  }).join("");
  const under = c.tUnder ?? null;
  const loaded = c.tLoaded !== null;
  const weighed = c.tDepart !== null;
  const tipped = c.tTip !== null;
  const dest = tipped
    ? `${DEST[c.dest]} · tipped ${clockAt(start, c.tTip!)}`
    : weighed && eta !== null
      ? `${DEST[c.dest]} · due ${clockAt(start, now + eta, false)}`
      : DEST[c.dest]!;
  const pct = weighed ? ` · ${fmt((c.payloadT / o.rated) * 100)}% of rated` : "";
  return `${head}${lead}
<p class="micro">Load ${String(c.n).padStart(4, "0")} · chain of custody${open ? "" : " · delivered"}${o.watching ? ` · the truck under ${X.id}` : ""}</p>
<ol class="hl-chain">
${row("done", "Face", `${c.face} · block ${c.block}`, ev("estimated", c.material))}
${row("done", `Queued at ${c.loader}`, `from ${clockAt(start, c.tArrive, false)} · ${c.queued > 20 ? span(c.queued) : "no wait"}`, ev("measured"))}
${row(under !== null ? "done" : "now", "Spotted", under !== null ? `reversed in, ${span(under - c.tSpot)}` : "reversing in")}
${row(loaded ? "done" : under !== null ? "now" : "", "Passes, t", `<span class="hl-passes">${passes}</span>`)}
${row(weighed ? "done" : "", "Weighed payload", weighed ? `${tonnes(c.payloadT)}${pct}` : "at departure", weighed ? ev("measured") : "")}
${row("done", "Grade", gradeText(c, o.est, o.cutoff), ev("estimated"))}
${row(tipped ? "done" : weighed ? "now" : "", "Destination", dest)}
${row(tipped ? "done" : "", "Ledger", tipped ? `closed ${clockAt(start, c.tTip!, false)}` : "open")}
</ol>
${assign}
<p class="hl-src">Payload: the truck’s payload monitor. Times: the fleet system. Grade: FrankenMining’s spatial predictor (fmine-calib) on the face’s blast-hole samples${o.engineMs !== undefined ? `, ${fmt(o.engineMs, 1)} ms in your browser` : ""}.</p>`;
}

export interface Draft {
  truck: string;
  place: string;
}

export interface Tail {
  mean: number;
  var: number;
  cvar: number;
  ms: number;
  n: number;
}

/** The dispatch agent's note: what it saw in the ledger, what it is worth, the change it drafts. */
export function agentNote(z: Sizing, o: { queuedFor: number; left: number; draft: Draft | null; at: string; cost: number; end: string; tail: Tail | null; open?: boolean }) {
  const lost = Math.round(z.lost / 10) * 10;
  const toEnd = Math.round((z.lost * Math.max(0, o.left)) / 3600 / 10) * 10;
  const k2 = Math.round(z.truckRate2 / 10) * 10;
  const cap = Math.round((z.idle2 * z.rate2) / 10) * 10;
  const tail = o.tail
    ? `<p class="hl-math">Minute by minute the loss ran at ${fmt(Math.round(o.tail.mean / 10) * 10)} t/h; in the worst tenth of minutes, ${fmt(Math.round(o.tail.cvar / 10) * 10)} t/h (CVaR 90%, FrankenMining’s risk kernel, ${o.tail.n} minutes, ${fmt(o.tail.ms, 1)} ms).</p>`
    : "";
  return `<p class="micro">Agent · dispatch · ${o.at}</p>
<p>A queue has stood at <b>S1</b> for <b>${o.queuedFor} min</b>, <b>${fmt(z.qAvg, 1)}</b> trucks on average over the last half hour (${fmt(z.qMin)} truck-minutes). <b>L2</b> on the waste face stood waiting <b>${fmt(z.idle2 * 100)}%</b> of that time.</p>
<div class="hl-sum">
  <div><small>Haulage lost</small><b>${fmt(lost)} t/h</b>${ev("estimated")}</div>
  <div><small>To ${o.end}</small><b>${fmt(toEnd)} t</b></div>
  <div><small>Trucks standing</small><b>$${fmt(Math.round((z.qAvg * o.cost) / 10) * 10)}/h</b>${ev("assumed")}</div>
</div>
${
  o.draft
    ? `<div class="hl-act"><span class="act">Draft dispatch change: move ${o.draft.truck} (${o.draft.place}) from S1 to L2 until ${o.end}. For the shift supervisor.</span><button type="button" class="btn sm go" data-approve>Approve</button></div>
<p class="hl-why">${z.fed ? "S1 stays fed: more than one truck waits there on average." : "S1 may wait a little between trucks: under one truck waits there on average."}</p>`
    : ""
}
<details class="hl-how"${o.open ? " open" : ""}><summary>How the agent sized it</summary>
<p class="hl-math">min(${fmt(z.qAvg, 1)}, 1) × min(${fmt(k2)}, ${fmt(z.idle2, 2)} × ${fmt(Math.round(z.rate2 / 10) * 10)}) t/h: one truck at L2 moves ${fmt(Math.round(z.payload))} t every ${fmt(z.cycle2 / 60, 1)} min; L2 has room for ${fmt(cap)} t/h. Trucks at $${fmt(o.cost)} an hour.</p>
${tail}</details>`;
}

export function approvedNote(o: { truck: string; at: string; q0: number; qNow: number; qAvg: number | null; l2: number; mins: number; gain: number | null }) {
  return `<p class="micro">Agent · dispatch · approved ${o.at}</p>
<p>The shift supervisor approved the change at <b>${o.at}</b> and the dispatcher set it in the fleet system: <b>${o.truck}</b> now works for <b>L2</b>. The agent drafted it; it cannot dispatch a truck.</p>
<div class="hl-sum">
  <div><small>Queued at S1</small><b>${o.q0} → ${o.qNow}</b></div>
  <div><small>Since approval</small><b>${o.qAvg === null ? "—" : fmt(o.qAvg, 1)}</b><span class="hl-unit">trucks waiting, mean</span></div>
  <div><small>Loaded at L2</small><b>${fmt(o.l2)} t</b><span class="hl-unit">in ${o.mins} min</span></div>
</div>
<p class="hl-math">${o.gain === null ? "" : `L2 is loading ${fmt(Math.round(o.gain / 10) * 10)} t/h more than before the change. `}Every load ${o.truck} takes at L2 joins the ledger with its face, payload and destination, like any other.</p>`;
}

const PARTS: [keyof CycleParts, string][] = [
  ["queue", "Queued"],
  ["spot", "Spotting"],
  ["load", "Loading"],
  ["haul", "Hauling loaded"],
  ["tip", "Tipping"],
  ["back", "Returning empty"],
];

const bar = (c: CycleParts | null, scale: number, name: string) => {
  if (!c) return `<div class="hl-cyc"><p><b>${name}</b><span>no loads in the window yet</span></p></div>`;
  const seg = PARTS.map(([k, label]) => {
    const v = c[k] as number;
    return `<i class="p-${k}" style="flex:${Math.max(0, v).toFixed(1)}" title="${label} ${ms(v)}"></i>`;
  }).join("");
  return `<div class="hl-cyc"><p><b>${name}</b><span>${ms(c.total)} a cycle · ${fmt(c.passes, 1)} passes · ${c.loads} loads</span></p><div class="hl-bar2" style="width:${Math.min(100, (c.total / scale) * 100).toFixed(1)}%">${seg}</div><p class="hl-cv">${PARTS.map(([k]) => `<span class="p-${k}">${ms(c[k] as number)}</span>`).join("")}</p></div>`;
};

export interface Governing {
  emptyT: number;
  ratedT: number;
  wheelKW: number;
  retardKW: number;
  rr: number;
  ramp: { grade: number; steepest: number; vLoaded: number; govLoaded: string; vEmpty: number; govEmpty: string; retardEmpty: number } | null;
  dipper: { m3: number; density: number; fill: [number, number]; passT: [number, number]; plan: number } | null;
  bucket2: { m3: number; passT: [number, number]; plan: number } | null;
  payload: PayloadStats | null;
  grades: { ms: number; queries: number; samples: number } | null;
}

/** The engineering card: the cycle at each loader from the ledger and the numbers that govern it. */
export function engCard(S1: CycleParts | null, L2: CycleParts | null, g: Governing) {
  const scale = Math.max(S1?.total ?? 0, L2?.total ?? 0, 60);
  const p = g.payload;
  const r = g.ramp;
  const d = g.dipper;
  return `<p class="micro">Engineering view · truck cycle and what sets it</p>
${bar(S1, scale, "S1 · ore to the crusher")}
${bar(L2, scale, "L2 · waste to the dump")}
<p class="hl-legend">${PARTS.map(([k, label]) => `<span class="p-${k}"><i></i>${label}</span>`).join("")}</p>
<dl class="hl-gov">
  <dt>Truck ${ev("assumed")}</dt><dd>${fmt(g.emptyT)} t + ${fmt(g.ratedT)} t rated · ${fmt(g.wheelKW)} kW at the wheels · rolling resistance ${fmt(g.rr * 100, 1)}%</dd>
  ${
    r
      ? `<dt>Ramp ${ev("estimated")}</dt><dd>up loaded ${fmt(r.vLoaded, 1)} km/h on ${fmt(r.grade * 100, 1)}% (${r.govLoaded}) · down empty ${fmt(r.vEmpty, 0)} km/h (${r.govEmpty}; retarder allows ${fmt(r.retardEmpty, 0)})</dd>`
      : ""
  }
  ${
    d
      ? `<dt>Passes ${ev("assumed")}</dt><dd>S1 ${fmt(d.m3)} m³ × ${fmt(d.density, 2)} t/m³ × fill ${fmt(d.fill[0], 2)}–${fmt(d.fill[1], 2)} = ${fmt(d.passT[0])}–${fmt(d.passT[1])} t, ${d.plan} and a top-up when light${g.bucket2 ? ` · L2 ${fmt(g.bucket2.m3)} m³, ${fmt(g.bucket2.passT[0])}–${fmt(g.bucket2.passT[1])} t, ${g.bucket2.plan}` : ""}</dd>`
      : ""
  }
  ${p ? `<dt>Payloads ${ev("measured")}</dt><dd>${fmt(p.mean)} ± ${fmt(p.sd)} t · ${fmt(p.min)}–${fmt(p.max)} t · ${fmt(p.over110 * 100)}% over 110%, ${fmt(p.over120 * 100)}% over 120% of rated · ${p.n} loads</dd>` : ""}
  ${g.grades ? `<dt>Grades ${ev("estimated")}</dt><dd>${g.grades.queries} blocks from ${g.grades.samples} blast-hole samples, fmine-calib, ${fmt(g.grades.ms, 1)} ms</dd>` : ""}
</dl>
<p class="hl-src">Cycle parts from the last 40 minutes of custody records. Speeds from rimpull, retarding and site limits on each stretch’s grade; truck figures typical of the class.</p>`;
}
