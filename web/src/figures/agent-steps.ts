/**
 * The session the agent figure plays: a question from the site GM, answered with real `fmine` verbs (registries/
 * verbs.toml) and envelopes shaped like FMT-ENV-1. Payloads are abridged and every value is synthetic. The decision
 * numbers are computed below, not typed in.
 */
import { FLAGS, MORE, claimId, group, inline, json, shortId, type Ev } from "./claim-ev";

/** Counted from registries/verbs.toml and refusals.toml when the site is built (build.ts). */
declare const __FM_VERBS__: number;
declare const __FM_REFUSALS__: number;

/* ---------- The decision: four weeks while stockpile SP-2 feeds the mill, in USD ---------- */
const P_FRESH = 0.6; // the metallurgist's prior, Elicited
const ACTIONS = [
  { id: "blend", label: "Change the blend: less N-07, more SP-2", fresh: 1_440_000, trans: -80_000 },
  { id: "regrind", label: "Run the regrind mill on rougher concentrate", fresh: 560_000, trans: 660_000 },
  { id: "hold", label: "Hold the current plan", fresh: 0, trans: 0 },
];
const SAMPLE = { accuracy: 0.9, test: 6_000, delay: 60_000, days: 2 };

function decide() {
  const mean = (a: (typeof ACTIONS)[number], p = P_FRESH) => p * a.fresh + (1 - p) * a.trans;
  const bestF = Math.max(...ACTIONS.map((a) => a.fresh));
  const bestT = Math.max(...ACTIONS.map((a) => a.trans));
  const rows = ACTIONS.map((a) => ({
    ...a,
    mean: mean(a),
    meanRegret: P_FRESH * (bestF - a.fresh) + (1 - P_FRESH) * (bestT - a.trans),
    maxRegret: Math.max(bestF - a.fresh, bestT - a.trans),
  }));
  const base = Math.max(...rows.map((r) => r.mean));
  const evpi = P_FRESH * bestF + (1 - P_FRESH) * bestT - base;
  const sayF = P_FRESH * SAMPLE.accuracy + (1 - P_FRESH) * (1 - SAMPLE.accuracy);
  const postF = (P_FRESH * SAMPLE.accuracy) / sayF;
  const postT = (P_FRESH * (1 - SAMPLE.accuracy)) / (1 - sayF);
  const gross = sayF * Math.max(...ACTIONS.map((a) => mean(a, postF))) + (1 - sayF) * Math.max(...ACTIONS.map((a) => mean(a, postT)));
  const evsi = gross - base;
  const cost = SAMPLE.test + SAMPLE.delay;
  const [a, b] = [rows[0]!, rows[1]!];
  const flip = (b.trans - a.trans) / (a.fresh - a.trans - (b.fresh - b.trans));
  return { rows, base, evpi, gross, evsi, cost, net: evsi - cost, flip };
}
export const D = decide();
const usd = (v: number) => String(Math.round(v));
export const k = (v: number) => `$${(v / 1000).toLocaleString("en-US", { maximumFractionDigits: 1 })}k`;

/* ---------- Plant change: Shapley split over the six factor orders, in kg of gold ---------- */
const WK42 = { t: 117_400, g: 1.43, r: 0.893 };
const WK43 = { t: 116_900, g: 1.42, r: 0.862 };
function explain() {
  const kg = (x: typeof WK42) => (x.t * x.g * x.r) / 1000;
  const keys = ["t", "g", "r"] as const;
  const eff = { t: 0, g: 0, r: 0 };
  const orders = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
  for (const order of orders) {
    const cur = { ...WK42 };
    for (const i of order) {
      const key = keys[i]!;
      const before = kg(cur);
      cur[key] = WK43[key];
      eff[key] += (kg(cur) - before) / 6;
    }
  }
  return { base: kg(WK42), now: kg(WK43), change: kg(WK43) - kg(WK42), eff };
}
export const X = explain();
const f2 = (v: number) => v.toFixed(2);

/* ---------- Envelopes ---------- */
const id = (s: string) => claimId(`synthetic:${s}`);
export const REC = id("recovery");
const FLAGS_SHORT = inline({ execution_authorized: false, more: MORE });
function env(verb: string, payload: unknown, o: { refusal?: Record<string, unknown>; exit?: number; next?: [string, string][]; full?: boolean; warnings?: string[] } = {}) {
  const refused = !!o.refusal;
  return {
    a: group({ schema_version: "fmine.env.v1", verb, status: refused ? "refused" : "ok", exit_code: o.exit ?? 0, anchor_id: null }),
    flags: o.full ? FLAGS : FLAGS_SHORT,
    b: group({ certificate: null, warnings: inline(o.warnings ?? []) }),
    next_actions: (o.next ?? []).map(([command, why]) => ({ a: group({ command }), b: group({ why }) })),
    payload,
    refusal: o.refusal ?? null,
  };
}
const head = "b3f1c07e…9a2d";

export interface Learned {
  text: string;
  head: string;
  src: string;
  ev?: Ev;
  tone?: "warn" | "refused" | "draft" | "ok";
}
export interface Step {
  rail: string;
  who?: string;
  say: string[];
  cmd?: string;
  out?: string;
  learned: Learned;
}

export const STEPS: Step[] = [
  {
    rail: "Question",
    who: "Why did flotation recovery drop 3.1 points this week?",
    say: ["First, what the tool can do. Then the number, then where it came from."],
    learned: { text: "Flotation recovery fell from 89.3% in week 42 to 86.2% in week 43. Find the cause and what to do.", src: "site GM", head: "Recovery 89.3% → 86.2% in week 43" },
  },
  {
    rail: "Docs",
    say: ["Reading the robot docs, so I use the tool as written."],
    cmd: "fmine robot-docs",
    out: json(
      env(
        "robot-docs",
        {
          a: group({ binary: "fmine", envelope: "FMT-ENV-1" }),
          verbs: inline([MORE, "claim why <id> […]", "forecast journal inspect --journal <path> [--head <hex>]", MORE]),
          exit_codes: { "0": "ok; lint verdicts and track-record misses are data", "2": "usage error (unknown verb, flag, issuer or selection); the envelope still prints" },
          claim_inspection: {
            why: "Recorded evidence, interpretation, computed color and Claim admission guards; absent assessments remain absent",
            trace: "Available source anchors and registered provenance; unresolved parents are explicit",
          },
          refusals: [MORE, inline({ code: "FMINE-REF-202", exit_code: 3, status: "active", summary: "Not observable: …" }), MORE],
          more: MORE,
        },
        { full: true },
      ),
    ),
    learned: { text: `One envelope for every reply, ${__FM_VERBS__} registered verbs and ${__FM_REFUSALS__} typed refusal codes. Four flags are false in every reply, execution_authorized among them.`, src: "robot-docs", head: `One envelope, ${__FM_VERBS__} verbs, ${__FM_REFUSALS__} refusal codes` },
  },
  {
    rail: "Schema",
    say: ["Taking the verb names from the schema."],
    cmd: "fmine robot schema verbs",
    out: json(
      env("robot schema", {
        a: group({ schema: "fmine.robot-schema.v1", target: "verbs" }),
        document: {
          $schema: "https://json-schema.org/draft/2020-12/schema",
          a: group({ $id: "urn:fmine:schema:verbs:v1", title: "Registered CLI Verbs", type: "string" }),
          enum: inline(["decide card", "decide production", "claim show", "claim why", MORE]),
        },
      }),
    ),
    learned: { text: "Commands come from a JSON Schema, so the agent never invents a verb or a flag.", src: "robot schema verbs", head: "Verb names from a JSON Schema" },
  },
  {
    rail: "Why",
    say: ["Is the drop real, or noise in the assays?"],
    cmd: `fmine claim why ${REC}`,
    out: json(
      env(
        "claim why",
        {
          a: group({ schema: "fmine.claim.why.v1", claim: inline({ claim_id: shortId(REC), more: MORE }) }),
          b: group({ evidence_class: "Derived", knowledge_state: "Estimated", additivity: "NonAdditive" }),
          representation: "Gaussian { mean: 86.2, std_dev: 0.85 }",
          color: { a: group({ status: "available", refusal: null }), value: inline({ kind: "validated", dataset: "qaqc:plant-balance-wk43" }) },
          uu_guard: inline({ status: "passed", refusal: null }),
          decision_admission: inline({ scope: "Claim color and UU guards only", claim_guards_pass: true, more: MORE }),
          more: MORE,
        },
        {
          next: [[`fmine claim trace ${REC}`, "the source anchors and available provenance"]],
        },
      ),
    ),
    learned: { text: "Recovery 86.2% (80% interval 85.1–87.3%) is derived from assays and a balance that closes. Week 42 was 89.3%, well outside it: the drop is real.", src: "claim why", ev: "derived", head: "The drop is real; the claim’s guards pass" },
  },
  {
    rail: "Trace",
    say: ["Following the claim to its sources."],
    cmd: `fmine claim trace ${REC}`,
    out: json(
      env("claim trace", {
        a: group({ schema: "fmine.claim.trace.v1", root: shortId(REC), complete_for_claim: true }),
        nodes: [
          inline({ claim_id: shortId(REC), category: "recovery", parents: [shortId(id("feed-assay")), MORE] }),
          inline({ claim_id: shortId(id("feed-assay")), category: "feed_assay", parents: [shortId(id("feed-lot"))] }),
          inline({ claim_id: shortId(id("feed-lot")), category: "feed_lot", value: { face: "N-07", share: "0.41", wk42: "0.18" } }),
          inline({ claim_id: shortId(id("ore-type")), category: "ore_type", value: { face: "N-07", type: "transitional" } }),
          inline({ claim_id: shortId(id("bwi")), category: "hardness", value: { unit: "kWh/t", wk43: "16.4", wk42: "14.0" } }),
          inline({ claim_id: shortId(id("grind")), category: "grind_p80", value: { unit: "µm", wk43: "131", wk42: "106" } }),
          MORE,
        ],
        b: group({ unresolved_parents: inline([]), unanchored_leaves: inline([]) }),
      }),
    ),
    learned: {
      text: "The feed changed. Face N-07 went from 18% to 41% of mill feed. Its ore is transitional and harder (BWi 14.0 → 16.4 kWh/t), and the grind coarsened (P80 106 → 131 µm).",
      src: "claim trace",
      ev: "measured",
      head: "Face N-07: 18% → 41% of feed, harder ore",
    },
  },
  {
    rail: "Change",
    say: ["How much of the lost gold is recovery, and how much tonnes or grade?"],
    cmd: `fmine forecast journal explain-plant-change --journal plant.journal --head ${head} --baseline wk42 --current wk43`,
    out: json(
      env("forecast journal explain-plant-change", {
        a: group({ schema: "fmine.plant-change.v1", head, head_pinned: true }),
        b: group({ circuit: "flotation", element: "Au", change_kg: f2(X.change) }),
        effects_kg: inline({ dry_tonnes: f2(X.eff.t), head_grade: f2(X.eff.g), recovery: f2(X.eff.r) }),
        c: group({ interaction_allocation: "mean_of_six_factor_orders", source_class: "source_derived_unverified" }),
        more: MORE,
      }),
    ),
    learned: {
      text: `Gold recovered fell ${f2(-X.change)} kg. Recovery explains ${f2(-X.eff.r)} kg of it; grade ${f2(-X.eff.g)} kg and tonnes ${f2(-X.eff.t)} kg.`,
      src: "explain-plant-change",
      ev: "derived",
      head: `Recovery: ${f2(-X.eff.r)} of ${f2(-X.change)} kg lost`,
    },
  },
  {
    rail: "Track record",
    say: ["Can the geometallurgy model be trusted on this ore?"],
    cmd: "fmine forecast journal inspect --journal journals/geomet-recovery.journal",
    out: json(
      env("forecast journal inspect", {
        a: group({ schema: "fmine.forecast-journal.v1", head: "5e02a9d1…c47b", expected_head_checked: false }),
        b: group({ n_forecasts: 28, n_cohorts: 2, n_outcomes: 28, n_assessments: 2 }),
        cohorts: [inline({ cohort: "fresh", inside_80: "18/22" }), inline({ cohort: "transitional", inside_80: "2/6" })],
        more: MORE,
      }),
    ),
    learned: {
      text: "Its frozen forecasts were scored: on fresh ore 18 of 22 weeks fell inside the 80% interval; on transitional ore, 2 of 6. The model is not to be relied on for this feed.",
      src: "forecast journal inspect",
      tone: "warn",
      head: "Model weak on transitional ore (2 of 6)",
    },
  },
  {
    rail: "Refusal",
    say: ["Would blending in stockpile SP-2 recover the loss? Asking for its recovery."],
    cmd: "fmine plant recovery --input cards/sp2-blend.toml",
    out: json(
      env("plant recovery", null, {
        exit: 3,
        refusal: {
          a: group({ code: "FMINE-REF-202", exit_code: 3 }),
          what: "stockpile SP-2 ore type and gold grade are not observable: the last assay is 41 days old against a 14-day validity window",
          fix: "Restore or replace the instrument or source; a quantity that is not observable is never reported as stable.",
        },
      }),
    ),
    learned: { text: "SP-2 cannot be assessed: its last assay is 41 days old, against a 14-day window. A typed refusal, FMINE-REF-202, so no number is invented.", src: "plant recovery", tone: "refused", head: "SP-2 not observable: FMINE-REF-202" },
  },
  {
    rail: "Card",
    say: ["Drafting the card, with the value of sampling SP-2."],
    cmd: "fmine decide card cards/recovery-wk43.toml",
    out: json(
      env("decide card", {
        a: group({ schema: "fmine.decision-card.v1", mode: "finite_scenario_draft" }),
        summary: {
          a: group({ model_key: "recovery-wk43", currency: "USD" }),
          question: "How should the plant respond to the week-43 recovery drop?",
          alternatives: D.rows.map((r) => inline({ action_id: r.id, mean: usd(r.mean), max_regret: usd(r.maxRegret), more: MORE })),
          information: {
            a: group({ baseline_primary: "blend", evpi: usd(D.evpi) }),
            options: [
              {
                a: group({ option_id: "sample-sp2", delay_days: SAMPLE.days, test_cost: usd(SAMPLE.test), delay_cost: usd(SAMPLE.delay) }),
                b: group({ evsi: usd(D.evsi), net_voi: usd(D.net), status: "positive" }),
              },
            ],
            more: MORE,
          },
        },
        more: MORE,
      }),
    ),
    learned: {
      text: `Over four weeks: change the blend ${k(D.rows[0]!.mean)}, regrind ${k(D.rows[1]!.mean)}, on a ${Math.round(P_FRESH * 100)}% chance SP-2 is fresh ore. A two-day sample of SP-2 is worth ${k(D.net)} net. The best choice flips at ${Math.round(D.flip * 100)}%.`,
      src: "decide card",
      ev: "elicited",
      tone: "draft",
      head: `Draft: sample SP-2 first, ${k(D.net)} net`,
    },
  },
  {
    rail: "Approval",
    say: [
      "Draft card routed to the plant metallurgist for approval.",
      `Recommendation: sample SP-2 first. If it is fresh, change the blend. If it is transitional, run the regrind mill.`,
      "I have changed nothing: no blend, no setpoint, no record. Recording a choice is for a person.",
    ],
    learned: { text: "Routed to the plant metallurgist. The agent executed nothing.", src: "hand-off", tone: "ok", head: "Routed to the plant metallurgist" },
  },
];
