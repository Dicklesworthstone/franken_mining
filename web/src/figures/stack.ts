/**
 * Alongside your systems: six systems on the left feed FrankenMining through a read-only gate, each into the
 * subsystems that take it; readers on the right; the sibling engines beneath. Picking any box lights its flows and
 * says what FrankenMining takes and gives back. Control systems get a lock: nothing flows back to them.
 */
import { esc } from "./claim-ev";
import { $, $$, reduced } from "./lib";
import { ENGINES, LEDGER, READERS, SUBS, SYSTEMS, type Posture } from "./stack-data";

const LOCK = `<svg class="st-lock" viewBox="0 0 12 14" aria-hidden="true"><path d="M3.2 6V4.3a2.8 2.8 0 0 1 5.6 0V6" fill="none" stroke="currentColor" stroke-width="1.5"/><rect x="1.5" y="6" width="9" height="7" rx="1.6" fill="currentColor"/></svg>`;
const ICON: Record<Posture, string> = {
  ot: LOCK,
  recon: `<svg class="st-lock" viewBox="0 0 14 14" aria-hidden="true"><path d="M2 5h8.5l-2-2M12 9H3.5l2 2" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
  read: `<svg class="st-lock" viewBox="0 0 14 14" aria-hidden="true"><path d="M1.5 7s2-4 5.5-4 5.5 4 5.5 4-2 4-5.5 4S1.5 7 1.5 7z" fill="none" stroke="currentColor" stroke-width="1.4"/><circle cx="7" cy="7" r="1.7" fill="currentColor"/></svg>`,
  public: `<svg class="st-lock" viewBox="0 0 14 14" aria-hidden="true"><path d="M3 1.5h5.5L11 4v8.5H3z M8.5 1.5V4H11" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/></svg>`,
};
const BYSUB = new Map(SUBS.map((s) => [s.id, s]));
const feedsOf = (sub: string) => SYSTEMS.filter((s) => s.feeds.includes(sub)).map((s) => s.id);
const readsFrom = (reader: string) => SUBS.filter((s) => s.to?.includes(reader)).map((s) => s.id);

/** Edges: system → subsystem, and subsystem → reader. */
const EDGES: [from: string, to: string][] = [
  ...SYSTEMS.flatMap((s) => s.feeds.map((f) => [s.id, f] as [string, string])),
  ...SUBS.flatMap((s) => (s.to ?? []).map((r) => [s.id, r] as [string, string])),
];

function lit(sel: string): Set<string> {
  const on = new Set([sel]);
  const sys = SYSTEMS.find((s) => s.id === sel);
  if (sys) for (const f of sys.feeds) on.add(f);
  if (BYSUB.has(sel)) {
    for (const s of feedsOf(sel)) on.add(s);
    for (const r of BYSUB.get(sel)!.to ?? []) on.add(r);
  }
  if (READERS.some((r) => r.id === sel)) for (const s of readsFrom(sel)) on.add(s);
  if (sel === "fa") on.add("erp");
  if (sel === "erp") on.add("fa");
  return on;
}

function detail(sel: string) {
  const sys = SYSTEMS.find((s) => s.id === sel);
  const sub = BYSUB.get(sel);
  const rd = READERS.find((r) => r.id === sel);
  const en = ENGINES.find((e) => e.id === sel);
  const list = (xs: string[]) => `<ul>${xs.map((x) => `<li>${esc(x)}</li>`).join("")}</ul>`;
  const names = (ids: string[]) => ids.map((i) => `<span class="st-mini">${esc(BYSUB.get(i)?.name ?? SYSTEMS.find((s) => s.id === i)?.name ?? i)}</span>`).join("");
  if (sys)
    return `<div class="st-dh"><h4>${esc(sys.name)}</h4><span class="st-badge p-${sys.posture}">${ICON[sys.posture]}${esc(sys.badge)}</span></div>
<div class="st-cols">
  <section><p class="micro">FrankenMining takes</p>${list(sys.takes)}</section>
  <section><p class="micro">It gives back</p>${list(sys.gives)}</section>
  <section><p class="micro">Into</p><p class="st-minis">${names(sys.feeds)}</p>${
    sys.posture === "ot"
      ? `<p class="st-note">A control system. No write path exists to it: the gate is one way.</p>`
      : sys.posture === "recon"
        ? `<p class="st-note">The ERP stays the ledger of record. FrankenMining never posts to it.</p>`
        : ""
  }</section>
</div>`;
  if (sub) {
    const from = feedsOf(sub.id);
    return `<div class="st-dh"><h4>${esc(sub.name)}</h4><span class="st-badge sub">Subsystem</span></div>
<div class="st-cols">
  <section><p class="micro">Bounded context</p><p>${esc(sub.what)}.</p></section>
  <section><p class="micro">Fed by</p><p class="st-minis">${from.length ? names(from) : `<span class="st-none">Other subsystems in the ledger</span>`}</p></section>
  <section><p class="micro">Serves</p><p class="st-minis">${(sub.to ?? []).map((r) => `<span class="st-mini">${esc(READERS.find((x) => x.id === r)!.name)}</span>`).join("") || `<span class="st-none">Every reader, through Adit</span>`}</p></section>
</div>`;
  }
  if (rd)
    return `<div class="st-dh"><h4>${esc(rd.name)}</h4><span class="st-badge reader">${esc(rd.sub)}</span></div>
<div class="st-cols">
  <section><p class="micro">What reaches it</p>${list(rd.gets)}</section>
  <section><p class="micro">The limit</p><p>${esc(rd.limit)}</p></section>
  <section><p class="micro">From</p><p class="st-minis">${names(readsFrom(rd.id))}</p></section>
</div>`;
  if (en)
    return `<div class="st-dh"><h4>${esc(en.name)}</h4><span class="st-badge dev">Under active development</span></div>
<div class="st-cols"><section><p class="micro">Role</p><p>${esc(en.role)}.</p></section><section><p class="micro">How FrankenMining uses it</p><p>${esc(en.how)}</p></section></div>`;
  return "";
}

export default function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("st");
  host.innerHTML = `<div class="st-stage" data-stage>
  <svg class="st-flows" aria-hidden="true"></svg>
  <div class="st-sys">
    <p class="micro st-col-h">Your systems · unchanged</p>
    ${SYSTEMS.map(
      (s) => `<button type="button" class="st-box st-s p-${s.posture}" data-id="${s.id}" aria-pressed="false">
  <span class="st-name">${esc(s.name)}</span><span class="st-kind">${esc(s.kind)}</span>
  <span class="st-badge p-${s.posture}">${ICON[s.posture]}${esc(s.badge)}</span>
</button>`,
    ).join("")}
  </div>
  <div class="st-flow down" aria-hidden="true"><span>Read-only, one way · into quarantine first</span></div>
  <div class="st-core">
    <div class="st-gate" aria-hidden="true"><span>Portal · read-only import · quarantine first</span></div>
    <div class="st-in">
      <div class="st-hd"><h4>FrankenMining</h4><p>One ledger beside your systems</p></div>
      <ol class="st-ledger">${LEDGER.map((l, i) => `<li style="--i:${i}">${esc(l)}</li>`).join("")}</ol>
      <div class="st-subs">${SUBS.map(
        (s) => `<button type="button" class="st-sub" data-id="${s.id}" aria-pressed="false"><b>${esc(s.name)}</b><span>${esc(s.what)}</span></button>`,
      ).join("")}</div>
      <p class="st-base"><b>Plumb</b> evidence classes and calibration · <b>Bedrock</b> append-only ledger</p>
    </div>
  </div>
  <div class="st-flow down" aria-hidden="true"><span>Cited numbers, typed refusals</span></div>
  <div class="st-read">
    <p class="micro st-col-h">Who reads it</p>
    ${READERS.map(
      (r) => `<button type="button" class="st-box st-r" data-id="${r.id}" aria-pressed="false"><span class="st-name">${esc(r.name)}</span><span class="st-kind">${esc(r.sub)}</span></button>`,
    ).join("")}
  </div>
  <div class="st-eng">
    <p class="micro st-col-h">Underneath · sibling engines</p>
    <div class="st-engs">${ENGINES.map(
      (e) => `<button type="button" class="st-box st-e" data-id="${e.id}" aria-pressed="false"><span class="st-name">${esc(e.name)}</span><span class="st-kind">${esc(e.role)}</span><span class="st-how">${esc(e.how)}</span><span class="st-dev">Under active development</span></button>`,
    ).join("")}</div>
  </div>
  <div class="st-detail" data-detail aria-live="polite"></div>
</div>`;

  const stage = $(host, "[data-stage]");
  const svg = $<SVGSVGElement>(stage, ".st-flows");
  const panel = $(host, "[data-detail]");
  const boxes = $$<HTMLButtonElement>(stage, "[data-id]");
  let sel = "erp";

  const node = (id: string) => stage.querySelector<HTMLElement>(`.st-s[data-id="${id}"], .st-sub[data-id="${id}"], .st-r[data-id="${id}"]`)!;

  const draw = () => {
    if (getComputedStyle(svg).display === "none") return;
    const S = stage.getBoundingClientRect();
    svg.setAttribute("viewBox", `0 0 ${S.width} ${S.height}`);
    const r = (el: Element) => {
      const b = el.getBoundingClientRect();
      return { l: b.left - S.left, r: b.right - S.left, t: b.top - S.top, b: b.bottom - S.top, y: (b.top + b.bottom) / 2 - S.top };
    };
    const on = lit(sel);
    let out = "";
    let dots = "";
    let marks = "";
    const core = r($(stage, ".st-core"));
    EDGES.forEach(([a, b], n) => {
      const A = r(node(a));
      const B = r(node(b));
      const sys = SYSTEMS.find((s) => s.id === a);
      // Feeds stop at the gate; reads leave from the card's far edge.
      const x1 = sys ? A.r : core.r;
      const x2 = sys ? core.l : B.l;
      const dx = (x2 - x1) * 0.5;
      const d = `M${x1.toFixed(1)} ${A.y.toFixed(1)}C${(x1 + dx).toFixed(1)} ${A.y.toFixed(1)} ${(x2 - dx).toFixed(1)} ${B.y.toFixed(1)} ${x2.toFixed(1)} ${B.y.toFixed(1)}`;
      const hot = on.has(a) && on.has(b);
      out += `<path id="st-p${n}" class="${hot ? "hot" : ""} p-${sys?.posture ?? "out"}" d="${d}"/>`;
      if (hot && !reduced) {
        const len = Math.hypot(x2 - x1, B.y - A.y);
        const dur = (len / 60).toFixed(2);
        for (let k = 0; k < 3; k++)
          dots += `<circle class="st-dot" r="2.6"><animateMotion dur="${dur}s" begin="${((-k * Number(dur)) / 3).toFixed(2)}s" repeatCount="indefinite"><mpath href="#st-p${n}"/></animateMotion></circle>`;
      }
    });
    // A lock sits on every control-system feed, like a one-way valve: nothing flows back.
    for (const s of SYSTEMS.filter((x) => x.posture === "ot")) {
      const A = r(node(s.id));
      const hot = on.has(s.id);
      marks += `<g class="st-mark${hot ? " hot" : ""}" transform="translate(${(A.r + 16).toFixed(1)} ${(A.y - 10).toFixed(1)})"><title>Read-only, one way: no write path to a control system</title><circle cx="7" cy="10" r="11"/><path d="M4 9V6.6a3 3 0 0 1 6 0V9" fill="none" stroke-width="1.6"/><rect class="b" x="2.4" y="9" width="9.2" height="7" rx="1.6"/></g>`;
    }
    svg.innerHTML = `${out}${marks}${dots}`;
  };

  const pick = (id: string) => {
    sel = id;
    const on = lit(id);
    // An engine sits under everything, so picking one dims nothing.
    const engine = ENGINES.some((e) => e.id === id);
    for (const b of boxes) {
      const k = b.dataset.id!;
      b.setAttribute("aria-pressed", String(k === id));
      b.classList.toggle("on", on.has(k) && k !== id);
      b.classList.toggle("off", !engine && !on.has(k) && !b.classList.contains("st-e"));
    }
    panel.innerHTML = detail(id);
    draw();
  };
  stage.addEventListener("click", (e) => {
    const b = (e.target as Element).closest<HTMLButtonElement>("[data-id]");
    if (!b) return;
    pick(b.dataset.id!);
    if (matchMedia("(max-width: 999px)").matches && !b.classList.contains("st-s"))
      panel.scrollIntoView({ block: "nearest", behavior: reduced ? "auto" : "smooth" });
  });

  pick(sel);
  new ResizeObserver(() => draw()).observe(stage);
}
