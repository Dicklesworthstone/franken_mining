/** Cost per ounce, built line by line (plan §21, Bullion): an all-in sustaining cost waterfall in which every bar can
 * say what it rests on, beside the two books it must agree with. The ERP stays the ledger of record; FrankenAccounting
 * keeps the reconciling book. A toggle books one accrual late, and the reconciling book finds and explains the
 * difference without posting anything. All amounts are illustrative. */
import { keep } from "../html";
import { icon, toggle as toggleMarkup } from "../parts";
import { $, $$, fmt, reduced, seen, toggle } from "./lib";

type Ev = "measured" | "estimated" | "assumed";
interface Line {
  id: string;
  label: string;
  /** USD million for the quarter; credits are negative. */
  m: number;
  kind: "cost" | "credit" | "sub" | "total" | "oz";
  ev?: Ev;
  account?: string;
  from: string;
  check: string;
  driver?: string;
  rule?: string;
}
const OZ = 120_000;
const LATE = 1.2; // USD million: the reagent accrual the ERP books in the next period
const perOz = (m: number) => (m * 1e6) / OZ;

const LINES: Line[] = [
  {
    id: "mining",
    label: "Mining",
    m: 62.4,
    kind: "cost",
    ev: "measured",
    account: "5100 Mining · CC-1100",
    from: "ERP cost centre CC-1100 Mining: drill and blast, load and haul, mine maintenance. Quarter actuals as booked.",
    check: "Reconciled to the ERP trial balance: difference $0.",
    driver: "CostDriverCard: 3.9 Mt moved × $16.00/t, fitted on the ledger’s own history.",
  },
  {
    id: "processing",
    label: "Processing",
    m: 48.0,
    kind: "cost",
    ev: "measured",
    account: "5200 Processing · CC-2100",
    from: "ERP cost centre CC-2100 Processing: crushing, grinding, leach, reagents and power.",
    check: "Reconciled to the ERP trial balance: difference $0.",
    driver: "CostDriverCard: 1.6 Mt milled × $30.00/t; reagent kg per tonne from the plant historian.",
  },
  {
    id: "ga",
    label: "Site G&A",
    m: 14.4,
    kind: "cost",
    ev: "measured",
    account: "5300 Site G&A · CC-3100",
    from: "ERP cost centre CC-3100 Site administration: camp, security, site services, community.",
    check: "Reconciled to the ERP trial balance: difference $0.",
    driver: "Mostly fixed: $4.8M a month.",
  },
  {
    id: "royalty",
    label: "Royalties and production taxes",
    m: 16.8,
    kind: "cost",
    ev: "estimated",
    account: "5400 Royalties and production taxes",
    from: "Computed by FrankenMining as a Workpaper: 120,000 oz sold × $4,000/oz settlement price × 3.5% under the royalty agreement, reviewed by the site controller and accrued in the ERP.",
    check: "The ERP accrual equals the Workpaper; the royalty holder’s statement will settle it.",
    driver: "Price-linked: it moves with the settlement price, as revenue does.",
  },
  {
    id: "credit",
    label: "By-product credits",
    m: -6.0,
    kind: "credit",
    ev: "estimated",
    account: "4200 By-product revenue, silver",
    from: "Silver in the doré: 240,000 oz × $25/oz on the refiner’s provisional statements, marked to market until final settlement.",
    check: "Reconciled to the ERP trial balance: difference $0.",
    rule: "Shown as a separate line (byproduct_credit_location = SeparateLine).",
  },
  {
    id: "tcc",
    label: "Total cash costs",
    m: 135.6,
    kind: "sub",
    from: "The sum of the five lines above, exactly. No balancing figure.",
    check: "Every input is cited to its ERP row or Workpaper.",
  },
  {
    id: "capex",
    label: "Sustaining capital",
    m: 30.0,
    kind: "cost",
    ev: "measured",
    account: "1610 Construction in progress, sustaining",
    from: "Sustaining leaves of construction in progress: fleet rebuilds, a tailings lift, production-phase stripping.",
    check: "Reconciled to the ERP trial balance: difference $0.",
    rule: "Each project is classed sustaining under the World Gold Council test by a named, versioned policy with an approver. Growth projects stay out.",
  },
  {
    id: "lease",
    label: "Sustaining leases",
    m: 3.6,
    kind: "cost",
    ev: "measured",
    account: "5450 Lease payments, sustaining",
    from: "Lease payments on leased mobile equipment, from accounts payable and the lease schedule.",
    check: "Reconciled to the ERP trial balance: difference $0.",
  },
  {
    id: "reclam",
    label: "Reclamation accretion and amortisation",
    m: 2.4,
    kind: "cost",
    ev: "estimated",
    account: "5700 Reclamation accretion and amortisation",
    from: "The closure provision unwinding at its discount rate, plus amortisation of the closure asset, from the site’s closure estimate as a reviewed Workpaper.",
    check: "Reconciled to the ERP trial balance: difference $0.",
    driver: "Moves with the closure estimate and the discount rate, both cited.",
  },
  {
    id: "expl",
    label: "Sustaining exploration",
    m: 3.0,
    kind: "cost",
    ev: "measured",
    account: "5600 Exploration, sustaining",
    from: "Minesite drilling that replaces reserves around the operation; cost centre CC-4100.",
    check: "Reconciled to the ERP trial balance: difference $0.",
    rule: "Greenfield exploration is not sustaining and stays out.",
  },
  {
    id: "corp",
    label: "Corporate G&A share",
    m: 6.0,
    kind: "cost",
    ev: "assumed",
    account: "5900 Corporate G&A allocation",
    from: "Corporate office costs, share-based pay included, allocated to this site by its share of ounces sold.",
    check: "The allocation key is a policy choice, so the line is marked assumed until the policy owner signs it.",
  },
  {
    id: "aisc",
    label: "All-in sustaining cost",
    m: 180.6,
    kind: "total",
    from: "Total cash costs plus the sustaining lines, divided by ounces sold.",
    check: "Leaves out interest, acquisitions, impairments, one-off severance, income tax and growth capital.",
    rule: "World Gold Council guidance (2013, updated 2018) · by-product basis · per ounce sold. A metric shown without its rules and basis is refused.",
  },
  {
    id: "oz",
    label: "Ounces sold",
    m: 0,
    kind: "oz",
    ev: "measured",
    from: "Refinery settlement statements for every doré shipment of the quarter: 120,000 oz of gold sold.",
    check: "Held in FrankenMining’s metal ledger and tied to each settlement. FrankenAccounting has no metal units, so ounces reach its book only as Workpaper rows.",
    rule: "Per ounce sold, not produced. 122,400 oz were poured; the difference is finished-goods inventory, a named layer.",
  },
];

const MAX = 1600;
/** "5100 Mining · CC-1100" → number, name and cost centre, so a phone can drop the cost centre. */
const acctCell = (a: string) => {
  const [head = "", cc] = a.split(" · ");
  const sp = head.indexOf(" ");
  return `<span class="lg-no">${head.slice(0, sp)}</span> ${head.slice(sp + 1)}${cc ? `<span class="lg-cc"> · ${cc}</span>` : ""}`;
};
const usd = (v: number) => `${v < 0 ? "−" : ""}$${fmt(Math.abs(v))}`;
const money = (m: number) => `${m < 0 ? "−" : ""}$${fmt(Math.abs(m), 1)}M`;

export default function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("lg");

  // Running position of each bar on the $/oz axis.
  let run = 0;
  const bars = LINES.map((l) => {
    // The ounces row carries no bar.
    const v = perOz(l.m);
    if (l.kind === "oz") return { l, a: 0, b: 0, v };
    if (l.kind === "sub" || l.kind === "total") return { l, a: 0, b: v, v };
    const a = run;
    run += v;
    return { l, a, b: run, v };
  });

  const row = ({ l, a, b, v }: (typeof bars)[number], i: number) => {
    const lo = Math.min(a, b);
    const w = Math.abs(b - a);
    const value =
      l.kind === "oz"
        ? `<b>${fmt(OZ)}</b><small>oz</small>`
        : `<b data-v>${usd(v)}</b><small data-m>${money(l.m)}</small>`;
    const bar =
      l.kind === "oz"
        ? `<span class="lg-div" aria-hidden="true">${icon("right")}every line ÷ ${fmt(OZ)} oz</span>`
        : `<span class="lg-bar ${l.kind}" style="left:${((lo / MAX) * 100).toFixed(3)}%;width:${((w / MAX) * 100).toFixed(3)}%;--i:${i};--late:${w ? ((10 / w) * 100).toFixed(3) : 0}%"><i class="late" aria-hidden="true"></i></span>`;
    return `<li class="lg-row ${l.kind}" data-id="${l.id}"><button type="button" aria-pressed="false" aria-label="${l.label}: ${l.kind === "oz" ? `${fmt(OZ)} ounces` : `${usd(v)} per ounce`}. Show its evidence"><span class="lg-lab">${l.label}</span><span class="lg-chip">${
      l.ev ? `<span class="ev ${l.ev}">${l.ev}</span>` : ""
    }</span><span class="lg-track">${bar}</span><span class="lg-val">${value}</span></button></li>`;
  };

  host.innerHTML = keep(`
<div class="lg-top">
  <p class="lg-tag"><span class="tag">Illustrative</span><span>Synthetic gold mine · one quarter · USD · by-product basis · per ounce sold</span></p>
  ${toggleMarkup("late", "Book a reagent accrual late")}
</div>
<div class="lg-main">
  <div class="lg-fall">
    <div class="lg-axis" aria-hidden="true"><span></span><span class="lg-ticks">${[0, 400, 800, 1200, 1600]
      .map((t) => `<i style="left:${(t / MAX) * 100}%">${t ? fmt(t) : "$0/oz"}</i>`)
      .join("")}</span><span></span></div>
    <ol class="lg-rows">${bars.map(row).join("")}</ol>
  </div>
  <aside class="lg-ev" aria-live="polite"></aside>
</div>
<div class="lg-books">
  <div class="lg-bhead">
    <div><p class="micro">Two books</p><p class="lg-bsub">The ERP stays the statutory ledger of record. FrankenAccounting keeps a reconciling book beside it, and the two must agree line for line.</p></div>
    <p class="lg-check" data-check></p>
  </div>
  <table class="data lg-table">
    <thead><tr><th>Account behind the line</th><th><span class="lg-book erp">ERP</span><small>ledger of record</small></th><th><span class="lg-book fa"><span class="lg-long">FrankenAccounting</span><span class="lg-short">Book</span></span><small>reconciling book</small></th><th><span class="lg-long">Difference</span><span class="lg-short">Diff.</span></th></tr></thead>
    <tbody>${LINES.filter((l) => l.account)
      .map(
        (l) =>
          `<tr data-acct="${l.id}"><td>${acctCell(l.account!)}</td><td data-erp>${fmt(l.m, 1)}</td><td>${fmt(l.m, 1)}</td><td data-diff>0.0</td></tr>`,
      )
      .join("")}<tr class="lg-sum"><td>Total, USD million</td><td data-erp-total>${fmt(180.6, 1)}</td><td>${fmt(180.6, 1)}</td><td data-diff-total>0.0</td></tr></tbody>
  </table>
  <div class="lg-item" data-item></div>
</div>
<p class="micro asis lg-note">All amounts, accounts and documents are illustrative. FrankenAccounting, version 0.0.1 and under active development, keeps the reconciling book’s journal. It has no mining semantics of its own: ounces, stockpiles and all-in sustaining cost are FrankenMining’s Bullion design on top of it. Neither book posts to the other, and agents draft entries without posting them.</p>`);

  const ev = $(host, ".lg-ev");
  const rows = $$<HTMLLIElement>(host, ".lg-row");
  const check = $(host, "[data-check]");
  const item = $(host, "[data-item]");
  let late = false;
  let picked = "processing";

  const show = (id: string) => {
    picked = id;
    const l = LINES.find((x) => x.id === id)!;
    for (const r of rows) $(r, "button").setAttribute("aria-pressed", String(r.dataset.id === id));
    const v = perOz(l.m);
    const head =
      l.kind === "oz"
        ? `<p class="lg-big"><b>${fmt(OZ)}</b> oz sold</p>`
        : `<p class="lg-big"><b>${usd(v)}</b>/oz <span>${money(l.m)} ÷ ${fmt(OZ)} oz</span></p>`;
    const lateNote =
      late && (id === "processing" || id === "aisc" || id === "tcc")
        ? `<p class="lg-flag">${icon("question")}<span>The reconciling book holds ${id === "processing" ? "$400/oz" : id === "tcc" ? "$1,130/oz" : "$1,505/oz"}, from the goods receipt and the plant’s consumption; the ERP shows ${id === "processing" ? "$390/oz" : id === "tcc" ? "$1,120/oz" : "$1,495/oz"}. The $10/oz between them is register item R-1, a reagent accrual the ERP booked in the next period.</span></p>`
        : "";
    const accountCheck = late && id === "processing" ? "ERP and reconciling book differ by $1.2M on this account: see register item R-1 below." : l.check;
    ev.innerHTML = keep(`<p class="micro">Evidence</p><h4>${l.label}${l.ev ? ` <span class="ev ${l.ev}">${l.ev}</span>` : ""}</h4>${head}${lateNote}
<dl>
  <div><dt>Comes from</dt><dd>${l.from}</dd></div>
  ${l.account ? `<div><dt>Account</dt><dd><code>${l.account}</code></dd></div>` : ""}
  <div><dt>Checked</dt><dd>${accountCheck}</dd></div>
  ${l.driver ? `<div><dt>Driver</dt><dd>${l.driver}</dd></div>` : ""}
  ${l.rule ? `<div><dt>Rule</dt><dd>${l.rule}</dd></div>` : ""}
  ${l.kind !== "oz" && l.kind !== "sub" && l.kind !== "total" ? `<div><dt>Per ounce</dt><dd>Divided by ${fmt(OZ)} oz sold, evidenced by refinery settlement. <button type="button" class="lg-jump" data-jump="oz">See the ounces</button></dd></div>` : ""}
</dl>`);
    const jump = ev.querySelector<HTMLButtonElement>("[data-jump]");
    jump?.addEventListener("click", () => show("oz"));
  };

  const setLate = (on: boolean) => {
    late = on;
    host.classList.toggle("is-late", on);
    const proc = rows.find((r) => r.dataset.id === "processing")!;
    const aisc = rows.find((r) => r.dataset.id === "aisc")!;
    const tcc = rows.find((r) => r.dataset.id === "tcc")!;
    for (const [r, erp, book] of [
      [proc, 390, 400],
      [tcc, 1120, 1130],
      [aisc, 1495, 1505],
    ] as const) {
      $(r, "[data-m]").innerHTML = on ? `ERP ${usd(erp)}<br>book ${usd(book)}` : money(LINES.find((l) => l.id === r.dataset.id)!.m);
    }
    const acct = $(host, '[data-acct="processing"]');
    $(acct, "[data-erp]").textContent = fmt(on ? 48 - LATE : 48, 1);
    $(acct, "[data-diff]").textContent = on ? fmt(LATE, 1) : "0.0";
    acct.classList.toggle("off", on);
    $(host, "[data-erp-total]").textContent = fmt(on ? 180.6 - LATE : 180.6, 1);
    $(host, "[data-diff-total]").textContent = on ? fmt(LATE, 1) : "0.0";
    $(host, ".lg-sum").classList.toggle("off", on);
    check.className = `lg-check ${on ? "warn" : "ok"}`;
    check.innerHTML = on
      ? `${icon("question")}<span><b>Trial balances differ by $1,200,000</b>One register item, explained</span>`
      : `${icon("check")}<span><b>Trial balances agree</b>Difference $0</span>`;
    item.innerHTML = keep(
      on
        ? `<div class="agent-note lg-note-in"><p class="micro">Reconciling book · register item R-1</p>
<p><b>$1,200,000 on 5200 Processing, this quarter.</b> A reagent delivery was received on 29 September (goods receipt GR-48211 in the warehouse system) and drawn into the leach circuit the same week (plant historian), but its accrual was posted in the ERP on 3 October, in the next period.</p>
<p>Effect on all-in sustaining cost: <b>+$10/oz</b> if the cost belongs in this quarter, $1,505/oz against $1,495/oz on the ERP’s figures. The metric shows both figures and the $10/oz gap between them.</p>
<p class="lg-nopost">Nothing is posted to the ERP. FrankenMining drafts a Workpaper and a proposed accrual; the site controller decides, and posts it in the ERP if they agree.</p>
<span class="act">Draft for the site controller · not posted</span></div>`
        : `<p class="lg-quiet">${icon("check")}No open register items. Every account above cites the ERP row it came from, and every computed line cites its Workpaper.</p>`,
    );
    show(on ? "processing" : picked);
    if (on) place();
  };

  // On a narrow screen the evidence opens under the row that was picked.
  const main = $(host, ".lg-main");
  const slot = document.createElement("li");
  slot.className = "lg-slot";
  const narrow = matchMedia("(max-width: 999px)");
  const place = () => {
    if (narrow.matches) {
      slot.append(ev);
      rows.find((r) => r.dataset.id === picked)!.after(slot);
    } else if (ev.parentElement !== main) {
      main.append(ev);
      slot.remove();
    }
  };
  narrow.addEventListener("change", place);
  for (const r of rows)
    $(r, "button").addEventListener("click", () => {
      show(r.dataset.id!);
      place();
    });
  toggle(host, "late", setLate);
  setLate(false);
  show("processing");
  place();

  const fall = $(host, ".lg-fall");
  if (reduced) fall.classList.add("go");
  else seen(fall, () => fall.classList.add("go"));
}
