/**
 * Play the session: an AI agent answers a site GM's question with `fmine` verbs, step by step. The terminal shows each
 * command and an abridged FMT-ENV-1 envelope; the side panel keeps what the agent has learned, ending in a draft
 * DecisionCard routed to a person. Nothing is executed. Synthetic data throughout.
 */
import { STEPS, D, k } from "./agent-steps";
import { chip, esc } from "./claim-ev";
import { $, $$, reduced, seen } from "./lib";

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

function block(i: number) {
  const s = STEPS[i]!;
  const lines = (s.out ?? "").split("\n");
  const exit = s.out ? (s.learned.tone === "refused" ? 3 : 0) : null;
  return `<div class="ag-blk" data-i="${i}">
${s.who ? `<p class="ag-who"><span class="ag-tag">site GM ›</span> ${esc(s.who)}</p>` : ""}
${s.say.map((t) => `<p class="ag-say"><span class="ag-tag">agent ›</span> ${esc(t)}</p>`).join("")}
${s.cmd ? `<p class="ag-cmd"><span class="ag-ps">$</span> <span data-cmd></span><i class="ag-caret" aria-hidden="true"></i></p>` : ""}
${s.out ? `<pre class="ag-out" data-out>${lines.map((l, n) => {
    // Leading spaces become a hanging indent, so a wrapped line stays under its key.
    const body = l.replace(/^ +/, "");
    return `<span class="ln" style="--i:${n};--ind:${l.length - body.length}">${body}</span>`;
  }).join("")}</pre>` : ""}
${exit === null ? "" : `<p class="ag-exit ${exit ? "bad" : "ok"}" data-exit>exit ${exit}${exit ? " · refused, typed, with a fix" : " · ok"}</p>`}
</div>`;
}

function hist(i: number) {
  const s = STEPS[i]!;
  if (!s.cmd) return "";
  const bad = s.learned.tone === "refused";
  return `<p class="ag-hist"><span class="ag-ps">$</span> <span class="c">${esc(s.cmd)}</span> <span class="ag-st ${bad ? "bad" : "ok"}">${bad ? "refused · 3" : "ok · 0"}</span></p>`;
}

export default function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("ag");
  const N = STEPS.length;
  host.innerHTML = `<div class="ag-top">
  <p class="micro ag-label">Illustrative session · synthetic data</p>
  <p class="ag-surface"><span class="ag-pill now">Today</span> fmine CLI, FMT‑ENV‑1 JSON envelopes, JSON Schemas <span class="ag-nw"><span class="ag-pill">In design</span> MCP server (fmine‑mcp)</span></p>
</div>
<div class="ag-ctl">
  <div class="ag-btns">
    <button type="button" class="btn sm" data-back aria-label="Back one step">←</button>
    <button type="button" class="btn sm go" data-play>Play</button>
    <button type="button" class="btn sm" data-step aria-label="Forward one step">→</button>
  </div>
  <ol class="ag-rail" aria-label="Steps">${STEPS.map(
    (s, i) => `<li><button type="button" data-to="${i}" aria-label="Step ${i + 1}: ${esc(s.rail)}"><i></i><span>${esc(s.rail)}</span></button></li>`,
  ).join("")}</ol>
  <span class="ag-count num" data-count></span>
</div>
<div class="ag-main">
  <div class="term ag-term">
    <div class="term-bar"><i></i><i></i><i></i><span>agent session · fmine · read-only</span></div>
    <div class="ag-body" data-body aria-live="polite"></div>
  </div>
  <aside class="ag-side">
    <p class="micro">What the agent has learned</p>
    <ol class="ag-learned" data-learned></ol>
    <div data-hand></div>
    <dl class="ag-tally" data-tally></dl>
  </aside>
</div>`;

  const body = $(host, "[data-body]");
  const learned = $(host, "[data-learned]");
  const hand = $(host, "[data-hand]");
  const tally = $(host, "[data-tally]");
  const play = $<HTMLButtonElement>(host, "[data-play]");
  const count = $(host, "[data-count]");
  const rail = $$<HTMLButtonElement>(host, ".ag-rail button");

  let cur = 0;
  let playing = false;
  let gen = 0;

  const side = (i: number, animate: boolean) => {
    learned.innerHTML = STEPS.slice(0, i + 1)
      .map((s, n) => {
        const l = s.learned;
        if (n < i)
          return `<li class="ag-l past ${l.tone ?? ""}"><span class="ag-n num">${n + 1}</span><p>${esc(l.head)}</p></li>`;
        return `<li class="ag-l ${l.tone ?? ""}${animate ? " new" : ""} cur">
  <span class="ag-n num">${n + 1}</span>
  <div><p>${esc(l.text)}</p><p class="ag-src"><code>${esc(l.src)}</code>${l.ev ? chip(l.ev) : ""}${l.tone === "refused" ? `<span class="ev k-refused">Refused</span>` : ""}</p></div>
</li>`;
      })
      .join("");
    hand.innerHTML =
      i >= 8
        ? `<div class="ag-card${i === 8 && animate ? " new" : ""}">
  <p class="micro">DecisionCard · draft · four weeks, USD</p>
  <table class="data ag-table">
    <thead><tr><th>Option</th><th class="r">Mean</th><th class="r">Max regret</th></tr></thead>
    <tbody>${D.rows
      .map((r) => `<tr><td>${esc(r.label)}</td><td class="r num">${k(r.mean)}</td><td class="r num">${k(r.maxRegret)}</td></tr>`)
      .join("")}
    <tr class="voi"><td>Sample SP-2 first: 2 days, $6k test, $60k delay</td><td class="r num" colspan="2">net value ${k(D.net)}</td></tr></tbody>
  </table>
  ${
    i >= 9
      ? `<p class="ag-route"><span>Routed to</span> <b>Plant metallurgist</b> <span class="ag-pill wait">awaiting approval</span></p>
  <p class="ag-fine">Only a person can approve it. Every reply carried <code>execution_authorized: false</code>.</p>`
      : `<p class="ag-fine">A draft. Nothing in it has been done.</p>`
  }
</div>`
        : "";
    const run = STEPS.slice(1, i + 1).filter((s) => s.cmd).length;
    const refused = STEPS.slice(0, i + 1).filter((s) => s.learned.tone === "refused").length;
    tally.innerHTML = [
      ["Commands run", run],
      ["Typed refusals", refused],
      ["Writes", 0],
      ["Control actions", 0],
    ]
      .map(([a, b]) => `<div><dt>${a}</dt><dd class="num">${b}</dd></div>`)
      .join("");
  };

  const chrome = () => {
    count.textContent = `${cur + 1} / ${N}`;
    rail.forEach((b, i) => {
      b.classList.toggle("done", i < cur);
      b.setAttribute("aria-current", String(i === cur));
    });
    play.textContent = playing ? "Pause" : cur >= N - 1 ? "Replay" : "Play";
    play.setAttribute("aria-pressed", String(playing));
  };

  /** Draws the session up to step i. Animated, it types the command and reveals the reply; it returns false if interrupted. */
  async function show(i: number, animate: boolean): Promise<boolean> {
    const my = ++gen;
    cur = i;
    const history = STEPS.slice(0, i)
      .map((_, n) => hist(n))
      .join("");
    const q = i > 0 ? `<p class="ag-who ag-old"><span class="ag-tag">site GM ›</span> ${esc(STEPS[0]!.who!)}</p>` : "";
    body.innerHTML = `${q}${history}${block(i)}`;
    // Where the terminal keeps one height and scrolls (phones and tablets), the step being run sits at its top.
    if (body.scrollHeight > body.clientHeight) body.scrollTop += $(body, ".ag-blk").getBoundingClientRect().top - body.getBoundingClientRect().top - 6;
    // While a step plays, the panel waits for its reply before it learns anything from it.
    const deferred = animate && !reduced && i > 0;
    side(deferred ? i - 1 : i, false);
    chrome();
    const s = STEPS[i]!;
    const blk = $(body, ".ag-blk");
    const cmd = blk.querySelector<HTMLElement>("[data-cmd]");
    const out = blk.querySelector<HTMLElement>("[data-out]");
    const exit = blk.querySelector<HTMLElement>("[data-exit]");
    if (!animate || reduced) {
      if (cmd) cmd.textContent = s.cmd!;
      blk.classList.add("done");
      return true;
    }
    blk.classList.add("run");
    await sleep(500);
    if (cmd) {
      const text = s.cmd!;
      for (let n = 1; n <= text.length; n += 2) {
        if (my !== gen) return false;
        cmd.textContent = text.slice(0, n);
        await sleep(16);
      }
      cmd.textContent = text;
      await sleep(260);
    }
    if (my !== gen) return false;
    blk.classList.add("done");
    if (out) {
      const n = out.querySelectorAll(".ln").length;
      await sleep(n * 26 + 300);
    }
    exit?.classList.add("on");
    if (my !== gen) return false;
    side(i, true);
    return true;
  }

  async function run() {
    if (playing) return;
    playing = true;
    if (cur >= N - 1) await show(0, true);
    chrome();
    const mine = gen;
    let token = mine;
    while (playing && cur < N - 1) {
      const ok = await show(cur + 1, true);
      token = gen;
      if (!ok || !playing) return;
      const s = STEPS[cur]!;
      await sleep(1600 + Math.min(2600, s.learned.text.length * 14));
      if (token !== gen || !playing) return;
    }
    playing = false;
    chrome();
  }
  const stop = () => {
    playing = false;
    gen++;
    chrome();
  };

  play.addEventListener("click", () => (playing ? stop() : void run()));
  $(host, "[data-step]").addEventListener("click", () => {
    stop();
    if (cur < N - 1) void show(cur + 1, false);
  });
  $(host, "[data-back]").addEventListener("click", () => {
    stop();
    if (cur > 0) void show(cur - 1, false);
  });
  rail.forEach((b, i) =>
    b.addEventListener("click", () => {
      stop();
      void show(i, false);
    }),
  );

  // A session left playing off screen would keep lengthening the panel above or below the reader: it pauses there,
  // and in view again it finishes the step it was on and carries on.
  let parked = false;
  new IntersectionObserver(([e]) => {
    if (!e) return;
    if (!e.isIntersecting && playing) {
      parked = true;
      stop();
    } else if (e.isIntersecting && parked) {
      parked = false;
      void show(cur, false);
      void run();
    }
  }).observe(host);
  void show(0, false);
  // Starts on its own once the controls are in view; the figure as a whole can be taller than a phone screen.
  if (!reduced) seen($(host, ".ag-ctl"), () => void run());
}
