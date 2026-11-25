/**
 * Shared by the claim, ladder, agent and stack figures: the plan's evidence classes (§7.1) with their weakest-input
 * order (NL-2), escaping, synthetic claim ids, and a JSON renderer styled like the CLI's FMT-ENV-1 envelopes.
 * No DOM access at the top level.
 */

/** Evidence classes as plan §7.1 names them, strongest first. Rank follows NL-2's partial order. */
export const EV = {
  measured: { name: "Measured (instrument)", short: "Measured", code: "Measured", rank: 0 },
  derived: { name: "Derived", short: "Derived", code: "Derived", rank: 1 },
  calibrated: { name: "Modelled, calibrated", short: "Modelled · cal.", code: "ModelledCalibrated", rank: 2 },
  uncalibrated: { name: "Modelled, uncalibrated", short: "Modelled · uncal.", code: "ModelledUncalibrated", rank: 3 },
  transferred: { name: "Transferred", short: "Transferred", code: "Transferred", rank: 3 },
  elicited: { name: "Elicited", short: "Elicited", code: "Elicited", rank: 3 },
  assumed: { name: "Assumed", short: "Assumed", code: "Assumed", rank: 4 },
  assertion: { name: "Assertion", short: "Assertion", code: "Assertion", rank: 5 },
} as const;
export type Ev = keyof typeof EV;
export const EV_ORDER: Ev[] = ["measured", "derived", "calibrated", "uncalibrated", "transferred", "elicited", "assumed", "assertion"];

/** NL-2: a derived claim is never stronger than its weakest input. All-Measured inputs make it Derived. */
export function weakest(inputs: Ev[]): Ev {
  let worst: Ev = "derived";
  for (const k of inputs) if (EV[k].rank > EV[worst].rank) worst = k;
  return worst;
}

export const esc = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** An evidence chip. `short` keeps it narrow inside tight nodes. */
export const chip = (k: Ev, short = false) =>
  `<span class="ev k-${k}" title="Evidence class: ${esc(EV[k].name)}">${esc(short ? EV[k].short : EV[k].name)}</span>`;

/** A repeatable 32-hex claim id, `clm_…`, the shape `fmine` prints (16 bytes, content-addressed). */
export function claimId(seed: string) {
  let h = 2166136261 >>> 0;
  for (const c of seed) h = Math.imul(h ^ c.charCodeAt(0), 16777619) >>> 0;
  let out = "clm_";
  for (let i = 0; i < 4; i++) {
    h = Math.imul(h ^ (h >>> 15), 2246822507) >>> 0;
    h = Math.imul(h ^ (h >>> 13), 3266489909) >>> 0;
    h = (h ^ (h >>> 16)) >>> 0;
    out += h.toString(16).padStart(8, "0");
  }
  return out;
}
export const shortId = (id: string) => `${id.slice(0, 10)}…${id.slice(-4)}`;

/* ---------- JSON, the way the robot envelope reads when indented ---------- */

/** Renders on one line. */
export class Inline {
  constructor(readonly v: unknown) {}
}
export const inline = (v: unknown) => new Inline(v);
/** Several short fields of one object, set on one line. Its key in the parent is ignored. */
export class Group {
  constructor(readonly v: Record<string, unknown>) {}
}
export const group = (v: Record<string, unknown>) => new Group(v);
/** An abridged tail: `…` in place of the rest. */
export const MORE = Symbol("more");
/** Keeps ids such as CV-3 or FMINE-REF-103 whole: a non-breaking hyphen between capitals and digits. */
export const tidy = (s: string) => s.replace(/(?<=[A-Z0-9])-(?=[A-Z0-9])/g, "‑");

const p = (s: string) => `<span class="j-p">${s}</span>`;
function atom(v: unknown): string {
  if (v === null) return `<span class="j-z">null</span>`;
  if (typeof v === "boolean") return `<span class="j-b">${v}</span>`;
  if (typeof v === "number") return `<span class="j-n">${v}</span>`;
  if (typeof v === "string") return `<span class="j-s">"${esc(v)}"</span>`;
  return "";
}
function one(v: unknown): string {
  if (v === MORE) return `<span class="j-m">…</span>`;
  if (v instanceof Inline) return one(v.v);
  if (Array.isArray(v)) return `${p("[")}${v.map(one).join(p(", "))}${p("]")}`;
  if (v && typeof v === "object")
    return `${p("{ ")}${Object.entries(v)
      .map(([k, x]) => (x === MORE ? one(MORE) : `<span class="j-k">"${esc(k)}"</span>${p(": ")}${one(x)}`))
      .join(p(", "))}${p(" }")}`;
  return atom(v);
}
/** Indented JSON as markup. Objects wrapped in `inline()` stay on one line; `MORE` abridges. */
export function json(v: unknown, depth = 0): string {
  const pad = "  ".repeat(depth + 1);
  const end = "  ".repeat(depth);
  if (v instanceof Inline || v === MORE) return one(v);
  if (Array.isArray(v)) {
    if (!v.length) return p("[]");
    return `${p("[")}\n${v.map((x) => pad + json(x, depth + 1)).join(`${p(",")}\n`)}\n${end}${p("]")}`;
  }
  if (v && typeof v === "object") {
    const rows = Object.entries(v).map(([k, x]) =>
      x === MORE
        ? pad + one(MORE)
        : x instanceof Group
          ? pad +
            Object.entries(x.v)
              .map(([gk, gx]) => (gx === MORE ? one(MORE) : `<span class="j-k">"${esc(gk)}"</span>${p(": ")}${one(gx)}`))
              .join(p(", "))
          : `${pad}<span class="j-k">"${esc(k)}"</span>${p(": ")}${json(x, depth + 1)}`,
    );
    return `${p("{")}\n${rows.join(`${p(",")}\n`)}\n${end}${p("}")}`;
  }
  return atom(v);
}

/** The four INV-002 flags. They are false in every envelope and cannot be set. */
export const FLAGS = inline({
  reserve_disclosure_claim: false,
  execution_authorized: false,
  geotechnical_signoff: false,
  regulatory_compliance_claim: false,
});
