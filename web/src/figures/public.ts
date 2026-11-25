/** The public assurance lane, run on one large gold and copper producer's public quarterly workbooks: twenty files read,
 * their figures held as Claims, their cost identities checked and their changes between releases listed. The totals
 * are the README's and the delivery record's; the identity row is illustrative; the restatement is from the
 * registered corpus. The issuer is not named here. */
import { keep } from "../html";
import { icon, seg as segMarkup } from "../parts";
import { $, $$, fmt, q, reduced, seen, seg, smooth, type Q } from "./lib";

const QUARTERS = ["2024Q1", "2024Q2", "2024Q3", "2024Q4", "2025Q1", "2025Q2", "2025Q3", "2025Q4", "2026Q1", "2026Q2", "2026Q3"];
type Kind = "os" | "aisc";
interface File { kind: Kind; quarter: string; }
// Registered sources: operating statistics for every quarter, AISC reconciliations from 2024Q3.
const FILES: File[] = QUARTERS.flatMap((quarter, i) => [
  { kind: "os" as const, quarter },
  ...(i >= 2 ? [{ kind: "aisc" as const, quarter }] : []),
]);
const KIND: Record<Kind, string> = { os: "Operating statistics", aisc: "AISC reconciliation" };
const KIND_IN_LINE: Record<Kind, string> = { os: "operating statistics", aisc: "AISC reconciliation" };

type Scope = "os-q2" | "all";
const inScope = (f: File, s: Scope) => s === "all" || (f.kind === "os" && f.quarter !== "2026Q3");

interface Count { n: number; label: string; scope: Scope; where: string; means: string; dots?: boolean; tone: string; }
const COUNTS: Count[] = [
  {
    n: 22_732,
    label: "observations held as Claims",
    scope: "os-q2",
    where: "10 operating-statistics workbooks, 2024Q1 to 2026Q2",
    means:
      "Every printed number, each with its period, publication vintage, ownership basis, unit, raw token and source cell. No mapping refusals. Dashes, blanks and n/a stay distinct.",
    tone: "claims",
  },
  {
    n: 2_915,
    label: "AISC identities evaluated",
    scope: "all",
    where: "all 20 workbooks, through 2026Q3",
    means: "Every total and per-ounce rate in the cost reconciliations, checked exactly against its own printed lines.",
    tone: "ids",
  },
  {
    n: 9,
    label: "unexplained restatements",
    scope: "os-q2",
    where: "comparisons across releases, through 2026Q2",
    means:
      "A figure that changed between releases for the same period, unit and ownership basis, with no registered note that explains it.",
    dots: true,
    tone: "silent",
  },
  {
    n: 60,
    label: "ownership-conflict findings",
    scope: "os-q2",
    where: "comparisons across releases, through 2026Q2",
    means: "A printed ownership share that disagrees with the share registered from other evidence. Both are kept; neither is chosen.",
    dots: true,
    tone: "conflict",
  },
];

// docs/delivery/README.md: 2,915 instances; states i–v 2,737 / 3 / 4 / 101 / 0, plus 70 not published.
const STATES = [
  { code: "i", name: "consistent", n: 2737, tone: "s1", means: "The printed numbers satisfy the declared construction." },
  { code: "ii", name: "explained", n: 3, tone: "s2", means: "They close only with a registered reconstruction, re-verified on the run." },
  { code: "iii", name: "issuer discrepancy", n: 4, tone: "s3", means: "A printed total differs from the sum of its printed lines. An exact certificate shows it." },
  { code: "iv", name: "undecidable from public data", n: 101, tone: "s4", means: "The printed rate lies outside every value the printed inputs allow. The fact that would decide it is named." },
  { code: "v", name: "refused", n: 0, tone: "s5", means: "Cannot be scored from what is printed." },
  { code: "np", name: "not published", n: 70, tone: "np", means: "Nothing printed for the instance, so it is excluded by name." },
];

// The illustrative reconciliation: whole USD millions, thousands of ounces, whole USD per ounce.
const BLOCK1: [string, number][] = [
  ["Cost of sales, gold", 448],
  ["Depreciation", -122],
  ["By-product credits", -9],
];
const BLOCK2: [string, number][] = [
  ["General and administrative", 12],
  ["Minesite exploration and evaluation", 4],
  ["Minesite sustaining capital", 71],
  ["Reclamation accretion and amortisation", 3],
];
const TCC = 317;
const OZ = 287;
const CASES: Record<string, { aisc: number; rate: number; name: string }> = {
  ok: { aisc: 407, rate: 1418, name: "Consistent" },
  foot: { aisc: 417, rate: 1453, name: "A total that does not foot" },
  rate: { aisc: 407, rate: 1452, name: "A rate out of reach" },
};

const one = (x: Q, digits = 1) => fmt(x.value, digits);
/** Rounds outward, so a shown interval always contains the exact one. */
const down = (x: Q) => fmt(Math.floor(x.value * 10) / 10, 1);
const up = (x: Q) => fmt(Math.ceil(x.value * 10) / 10, 1);
const signed = (x: Q) => `${x.cmp(q(0)) < 0 ? "−" : "+"}${fmt(Math.abs(x.value), 2)}`;

interface Check { what: string; sum: string; result: string; code: string; label: string; tone: string; }
function checks(c: { aisc: number; rate: number }): Check[] {
  const half = q(1, 2);
  // TCC footing: three lines, each ± 1/2, against the printed total ± 1/2.
  const tccSum = BLOCK1.reduce((s, [, v]) => s + v, 0);
  const tcc: Check = {
    what: "Total cash costs foot",
    sum: `${BLOCK1.map(([, v], i) => (i ? (v < 0 ? ` − ${fmt(-v)}` : ` + ${fmt(v)}`) : fmt(v))).join("")} = ${fmt(tccSum)}`,
    result: `Printed ${fmt(TCC)}.`,
    code: "i",
    label: "consistent",
    tone: "s1",
  };
  // AISC footing: printed TCC plus the four block-2 lines, each ± 1/2.
  const lines = [TCC, ...BLOCK2.map(([, v]) => v)];
  const sum = lines.reduce((s, v) => s + v, 0);
  const slack = q(lines.length, 2);
  const lo = q(sum).sub(slack);
  const hi = q(sum).add(slack);
  const foots = q(c.aisc).add(half).cmp(lo) >= 0 && q(c.aisc).sub(half).cmp(hi) <= 0;
  const aisc: Check = {
    what: "All-in sustaining cost foots",
    sum: `${fmt(TCC)} + ${BLOCK2.map(([, v]) => fmt(v)).join(" + ")} = ${fmt(sum)}`,
    result: foots
      ? `Printed ${fmt(c.aisc)}.`
      : `Printed ${fmt(c.aisc)}. With every figure rounded, the lines allow ${one(lo)} to ${one(hi)} and the total ${one(q(c.aisc).sub(half))} to ${one(q(c.aisc).add(half))}. They cannot meet: a gap of at least ${one(q(c.aisc).sub(half).sub(hi))}, kept as an exact certificate.`,
    code: foots ? "i" : "iii",
    label: foots ? "consistent" : "issuer_discrepancy",
    tone: foots ? "s1" : "s3",
  };
  // AISC per ounce: r = 1000·N/Q with N = n ± k/2 (k = the 7 lines summed into N), Q = q ± 1/2, r ± 1/2.
  const k = BLOCK1.length + BLOCK2.length;
  const n = q(c.aisc);
  const rlo = q(1000).mul(n.sub(q(k, 2))).div(q(OZ).add(half));
  const rhi = q(1000).mul(n.add(q(k, 2))).div(q(OZ).sub(half));
  const point = q(1000).mul(n).div(q(OZ));
  const holds = q(c.rate).add(half).cmp(rlo) >= 0 && q(c.rate).sub(half).cmp(rhi) <= 0;
  const rate: Check = {
    what: "AISC per ounce, recomputed",
    sum: `1,000 × ${fmt(c.aisc)} ÷ ${fmt(OZ)} = ${one(point, 2)} USD/oz`,
    result: `Published ${fmt(c.rate)}, difference ${signed(q(c.rate).sub(point))}. The printed inputs allow ${down(rlo)} to ${up(rhi)}; ${
      holds ? "the published rate lies inside." : "the published rate lies outside, and no registered reconstruction explains it."
    }`,
    code: holds ? "i" : "iv",
    label: holds ? "consistent" : "undecidable_from_public_data",
    tone: holds ? "s1" : "s4",
  };
  return [tcc, aisc, rate];
}

export default function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("pb");

  const years = [
    ["2024", 4],
    ["2025", 4],
    ["2026", 3],
  ] as const;
  const row = (kind: Kind) =>
    `<div class="pb-row" data-kind="${kind}"><p class="pb-rlab">${KIND[kind]}<small>${kind === "os" ? "11 workbooks" : "9 workbooks"}</small></p><ol class="pb-cells">${QUARTERS.map((quarter) => {
      const f = FILES.find((x) => x.kind === kind && x.quarter === quarter);
      return f
        ? `<li class="pb-file" data-q="${quarter}" data-kind="${kind}" title="${KIND[kind]} ${quarter}"><span class="pb-sheet" aria-hidden="true"></span><b>${quarter.slice(4)}</b><i aria-hidden="true">${icon("check")}</i></li>`
        : `<li class="pb-file none" aria-hidden="true"><b>–</b></li>`;
    }).join("")}</ol></div>`;

  const total = STATES.reduce((s, x) => s + x.n, 0);

  host.innerHTML = keep(`
<div class="pb-top">
  <p class="pb-tag"><span class="tag">Public data</span><span>One large gold and copper producer’s public quarterly workbooks · 20 files · 2024Q1 through 2026Q3</span></p>
  <button type="button" class="btn sm go pb-run">${icon("redo")}<span>Run the check</span></button>
</div>
<div class="pb-files">
  <div class="pb-years" aria-hidden="true"><span></span><span class="pb-ycells">${years.map(([y, n]) => `<span style="grid-column:span ${n}">${y}</span>`).join("")}</span></div>
  ${row("os")}
  ${row("aisc")}
  <p class="pb-status" aria-live="polite"><code>fmine assure public --issuer …</code><span data-status></span></p>
</div>
<div class="pb-counts">${COUNTS.map(
    (c, i) =>
      `<button type="button" class="pb-count ${c.tone}" data-i="${i}" data-scope="${c.scope}" aria-pressed="false"><b class="num" data-n="${c.n}">0</b><span class="pb-clab">${c.label}</span>${
        c.dots ? `<span class="pb-dots" aria-hidden="true">${"<i></i>".repeat(c.n)}</span>` : ""
      }<small class="pb-where">${c.where}</small><span class="pb-means">${c.means}</span></button>`,
  ).join("")}</div>
<div class="pb-states">
  <p class="micro">${fmt(total)} AISC identity instances, each in exactly one state</p>
  <div class="pb-bar" role="img" aria-label="${STATES.map((s) => `${s.name} ${fmt(s.n)}`).join(", ")}">${STATES.filter((s) => s.n)
    .map((s) => `<i class="${s.tone}" style="--n:${s.n}"></i>`)
    .join("")}</div>
  <ul class="pb-legend">${STATES.map(
    (s) =>
      `<li class="${s.tone}"><span class="pb-code">${s.code}</span><span class="pb-sname">${s.name}</span><b data-n="${s.n}">${fmt(s.n)}</b><small>${s.means}</small></li>`,
  ).join("")}</ul>
</div>
<div class="pb-detail">
  <section class="pb-id">
    <div class="pb-dhead"><p class="micro">One identity, checked <span class="tag">Illustrative</span></p>${segMarkup(
      "case",
      "Printed figures",
      Object.entries(CASES).map(([v, c]) => [v, c.name] as [string, string]),
      "ok",
    )}</div>
    <div class="pb-idbody">
      <table class="data pb-lines">
        <thead><tr><th>Line, as printed</th><th>USD M</th></tr></thead>
        <tbody>
          ${BLOCK1.map(([l, v]) => `<tr><td>${l}</td><td>${fmt(v)}</td></tr>`).join("")}
          <tr class="sub"><td>Total cash costs</td><td>${fmt(TCC)}</td></tr>
          ${BLOCK2.map(([l, v]) => `<tr><td>${l}</td><td>${fmt(v)}</td></tr>`).join("")}
          <tr class="sub"><td>All-in sustaining cost</td><td data-aisc></td></tr>
          <tr><td>Ounces sold, thousands</td><td>${fmt(OZ)}</td></tr>
          <tr class="sub"><td>AISC, USD/oz</td><td data-rate></td></tr>
        </tbody>
      </table>
      <ol class="pb-checks" aria-live="polite"></ol>
    </div>
  </section>
  <section class="pb-re">
    <p class="micro">One restatement, illustrative</p>
    <p class="pb-rewhat">One printed cost per ounce: the same subject, period, unit and ownership basis, in two releases.</p>
    <div class="pb-pair">
      <div><small>First release · 2024Q1 · quarter column</small><b>1,262</b><span>USD/oz</span></div>
      <i aria-hidden="true">${icon("right")}</i>
      <div><small>A year later · 2025Q1 · prior-year column</small><b>1,257</b><span>USD/oz</span></div>
    </div>
    <p class="pb-state"><code>restated_silent</code><span>Value differs between vintages; no note found in registered documents.</span></p>
    <p class="pb-recap">Both Claims stay, each with its source cell and workbook hash. The difference does not say which release is right, and “unexplained” means only that no registered note covers it.</p>
    <p class="pb-recap">Each of the 9 restatements and 60 ownership conflicts becomes one review packet with its operands and the document that would close it. The human check stays empty until a person fills it.</p>
  </section>
</div>
<p class="micro asis pb-note">The counts are the command’s totals on the registered workbooks, which stay with their publisher and are checked against their registered SHA-256 hashes on every run. The observation count and the two finding counts describe the 2026Q2 epoch; the identity count covers all 20 files through 2026Q3. The identity row and the restatement pair are illustrative. Nothing here scores the issuer.</p>`);

  const files = $$<HTMLLIElement>(host, ".pb-file:not(.none)");
  const status = $(host, "[data-status]");
  const counts = $$<HTMLButtonElement>(host, ".pb-count");
  const run = $<HTMLButtonElement>(host, ".pb-run");
  const order = [...files].sort(
    (a, b) => a.dataset.q!.localeCompare(b.dataset.q!) || (a.dataset.kind === "os" ? -1 : 1),
  );

  // Hover, focus or press a count to see which files it covers.
  let pinned: number | null = null;
  const scope = (i: number | null) => {
    const s = i === null ? null : COUNTS[i]!.scope;
    host.dataset.scope = s ?? "";
    for (const f of files) {
      const file: File = { kind: f.dataset.kind as Kind, quarter: f.dataset.q! };
      f.classList.toggle("pb-dim", s !== null && !inScope(file, s));
    }
    for (const [j, c] of counts.entries()) c.classList.toggle("lit", j === i);
  };
  for (const [i, c] of counts.entries()) {
    c.addEventListener("mouseenter", () => scope(i));
    c.addEventListener("focus", () => scope(i));
    c.addEventListener("mouseleave", () => scope(pinned));
    c.addEventListener("blur", () => scope(pinned));
    c.addEventListener("click", () => {
      pinned = pinned === i ? null : i;
      for (const [j, d] of counts.entries()) d.setAttribute("aria-pressed", String(j === pinned));
      scope(pinned);
    });
  }

  // The identity row.
  const ids = $(host, ".pb-checks");
  const show = (v: string) => {
    const c = CASES[v]!;
    $(host, "[data-aisc]").textContent = fmt(c.aisc);
    $(host, "[data-rate]").textContent = fmt(c.rate);
    $(host, "[data-aisc]").classList.toggle("pb-hit", c.aisc !== CASES.ok!.aisc);
    $(host, "[data-rate]").classList.toggle("pb-hit", c.rate !== CASES.ok!.rate);
    ids.innerHTML = keep(
      checks(c)
        .map(
          (k) =>
            `<li class="${k.tone}"><p class="pb-cwhat">${k.what}<span class="pb-chip ${k.tone}"><b>${k.code}</b>${k.label}</span></p><p class="pb-sum">${k.sum}</p><p class="pb-res">${k.result}</p></li>`,
        )
        .join(""),
    );
  };
  seg(host, "case", show);
  show("ok");

  // The run: read every file in order of quarter, then count.
  const nums = $$<HTMLElement>(host, ".pb-count .num");
  const dots = $$<HTMLElement>(host, ".pb-dots i");
  const groups = $$<HTMLElement>(host, ".pb-dots").map((g) => $$<HTMLElement>(g, "i"));
  let timers: number[] = [];
  let frame = 0;
  const settle = () => {
    for (const f of files) f.classList.add("pb-done");
    for (const n of nums) n.textContent = fmt(Number(n.dataset.n));
    for (const d of dots) d.classList.add("on");
    host.classList.add("done");
    status.textContent = "20 of 20 files match their registered SHA-\u2060256. Every finding cites its source cell.";
    run.disabled = false;
  };
  const go = () => {
    for (const t of timers) clearTimeout(t);
    cancelAnimationFrame(frame);
    timers = [];
    if (reduced) {
      settle();
      return;
    }
    run.disabled = true;
    host.classList.remove("done");
    for (const f of files) f.classList.remove("pb-done", "pb-now");
    for (const n of nums) n.textContent = "0";
    for (const d of dots) d.classList.remove("on");
    const step = 110;
    for (const [i, f] of order.entries()) {
      timers.push(
        window.setTimeout(() => {
          for (const g of files) g.classList.remove("pb-now");
          f.classList.add("pb-now", "pb-done");
          status.textContent = `Reading ${f.dataset.q} ${KIND_IN_LINE[f.dataset.kind as Kind]} · SHA-\u2060256 matches`;
        }, i * step),
      );
    }
    const start = order.length * step + 150;
    timers.push(
      window.setTimeout(() => {
        for (const g of files) g.classList.remove("pb-now");
        status.textContent = "Holding observations as Claims, checking identities, comparing releases…";
        host.classList.add("done");
        const t0 = performance.now();
        const tick = (now: number) => {
          const t = smooth((now - t0) / 1600);
          for (const n of nums) n.textContent = fmt(Math.round(Number(n.dataset.n) * t));
          for (const group of groups) {
            const lit = Math.round(group.length * t);
            for (const [i, d] of group.entries()) d.classList.toggle("on", i < lit);
          }
          if (t < 1) frame = requestAnimationFrame(tick);
          else settle();
        };
        frame = requestAnimationFrame(tick);
      }, start),
    );
  };
  run.addEventListener("click", go);
  if (reduced) settle();
  else seen(host, go);
}
