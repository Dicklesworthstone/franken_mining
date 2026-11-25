/**
 * Renders the pictures that have to exist before a browser can draw anything: the hero's first frame and one share
 * card per page. The mines in them are the site's own 3D scenes, photographed in Chrome from a local build.
 *
 *   bun run build && bun run stills          # whatever is missing or whose text has changed
 *   bun run stills --all                     # everything, scenes photographed again
 */
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { chromium, type Page } from "playwright-core";
import { escape, mark } from "./layout";
import { entries, groups } from "./plan";

const root = resolve(import.meta.dir, "..");
const out = join(root, "out");
const media = join(root, "media");
const dir = join(media, "share");
// Photographs of the scenes are scratch: they live beside the other previews, never in out/ or media/.
const art = join(root, ".preview", "art");
mkdirSync(dir, { recursive: true });
mkdirSync(art, { recursive: true });
const all = process.argv.includes("--all");

const server = Bun.serve({
  port: 0,
  hostname: "127.0.0.1",
  fetch(req) {
    const pathname = decodeURIComponent(new URL(req.url).pathname);
    let path = pathname.startsWith("/.art/") ? join(art, pathname.slice(6)) : join(out, pathname);
    if (statSync(path, { throwIfNoEntry: false })?.isDirectory()) path = join(path, "index.html");
    return statSync(path, { throwIfNoEntry: false })?.isFile()
      ? new Response(Bun.file(path), { headers: { "access-control-allow-origin": "*" } })
      : new Response("Not found", { status: 404 });
  },
});
const base = `http://127.0.0.1:${server.port}`;
const browser = await chromium.launch({
  channel: "chrome",
  args: ["--use-gl=angle", "--use-angle=metal", "--enable-gpu", "--ignore-gpu-blocklist"],
});

const HIDE = `header.top,.dock,.hero-copy,.console-wrap,.lineup::after,.skip,.pins,.scene-labels,.label3d{display:none!important}html{scroll-behavior:auto!important;scroll-padding:0!important}body::before,body::after{display:none!important}`;
/** Opens the home page (with its stages reachable) with everything but the pictures out of the way. */
async function open(w: number, h: number, scale: number, css: string) {
  const page = await browser.newPage({ viewport: { width: w, height: h }, deviceScaleFactor: scale });
  page.on("pageerror", (e) => console.log("pageerror:", String(e).slice(0, 200)));
  await page.goto(`${base}/?stage`, { waitUntil: "load" });
  await page.addStyleTag({ content: HIDE + css });
  return page;
}
const webp = async (page: Page, png: Buffer, path: string, quality: number) => {
  const data = await page.evaluate(
    async ([src, q]) => {
      const img = new Image();
      img.src = `data:image/png;base64,${src}`;
      await img.decode();
      const c = document.createElement("canvas");
      c.width = img.width;
      c.height = img.height;
      c.getContext("2d")!.drawImage(img, 0, 0);
      return c.toDataURL("image/webp", q).split(",")[1]!;
    },
    [png.toString("base64"), quality] as const,
  );
  writeFileSync(path, Buffer.from(data, "base64"));
};
const HERO_FULL = (h: number) =>
  `.hero{min-height:${h}px!important;height:${h}px;padding:0!important}.lineup{position:absolute!important;inset:0!important;margin:0!important;aspect-ratio:auto!important}`;

/* ---------- The hero's first frame ---------- */
for (const [name, w, h, scale] of [
  ["lineup", 1920, 1056, 1],
  ["lineup-tall", 540, 590, 2],
] as const) {
  const path = join(media, `${name}.webp`);
  if (!all && existsSync(path)) continue;
  const page = await open(w, h, scale, HERO_FULL(h));
  await page.waitForSelector("#fig-hero[data-live]");
  await page.waitForTimeout(2200);
  await webp(page, await page.screenshot({ type: "png" }), path, 0.8);
  await page.close();
  console.log("poster", name, `${(statSync(path).size / 1024).toFixed(0)} KB`);
}

/* ---------- Pictures of the scenes ---------- */
type View = { target: [number, number, number]; distance: number; azimuth: number; elevation: number; lens?: string };
/** Views of the hero's working pit; a lens shows the engine's drillholes, block model or pit limit through the rock. */
const PIT: Record<string, View> = {
  "pit-wide": { target: [-30, -90, 10], distance: 1350, azimuth: 212, elevation: 34 },
  "pit-geology": { target: [10, -185, 30], distance: 780, azimuth: 204, elevation: 31, lens: "geology" },
  "pit-limit": { target: [0, -160, 0], distance: 1000, azimuth: 222, elevation: 42, lens: "cfo" },
  "pit-agent": { target: [-20, -140, 20], distance: 1150, azimuth: 198, elevation: 36, lens: "agent" },
  "pit-floor": { target: [-80, -146, 20], distance: 210, azimuth: 150, elevation: 22 },
  "pit-crusher": { target: [225, 10, 540], distance: 300, azimuth: 232, elevation: 24 },
  "pit-top": { target: [0, -80, 0], distance: 1150, azimuth: 20, elevation: 62 },
  "pit-drill": { target: [82, -40, -260], distance: 150, azimuth: 12, elevation: 20 },
};
/** The other scenes, photographed as they open. A scene that does not draw falls back to a view of the pit. */
/** A scene's own camera can be replaced for its card, to stand its subject to the right of the card's words. */
const SCENES: Record<string, { fig: string; fallback: string; view?: View; wait?: number }> = {
  drill: { fig: "fig-drill", fallback: "pit-top" },
  pit: { fig: "fig-pit", fallback: "pit-wide", wait: 10000, view: { target: [-80, -130, 66], distance: 1050, azimuth: 28, elevation: 30 } },
  blast: { fig: "fig-blast", fallback: "pit-drill" },
  haul: { fig: "fig-haul", fallback: "pit-floor" },
  underground: { fig: "fig-underground", fallback: "pit-top" },
  plant: { fig: "fig-plant", fallback: "pit-crusher", wait: 9000, view: { target: [-80, 0, -200], distance: 430, azimuth: -108, elevation: 25 } },
  tailings: { fig: "fig-tailings", fallback: "pit-wide" },
};
const shot = (name: string) => join(art, `${name}.png`);
const have = new Set<string>();

/** A label the scene showed when it was photographed: its words, number and evidence class, and where it stood. */
interface Chip {
  t: string;
  b: string;
  ev: string;
  x: number;
  y: number;
}
const chipsFile = (name: string) => join(art, `${name}.json`);
/** The labels a figure is showing (they are hidden from the photograph, but each keeps its words and position). */
const chipsOf = (page: Page, fig: string) =>
  page.evaluate((sel) => {
    const out: { t: string; b: string; ev: string; x: number; y: number }[] = [];
    for (const el of document.querySelectorAll<HTMLElement>(`${sel} .label3d`)) {
      if (el.classList.contains("off") || el.style.visibility === "hidden") continue;
      const m = el.style.transform.match(/translate3d\(([-\d.]+)px,\s*([-\d.]+)px/);
      if (!m) continue;
      const c = el.cloneNode(true) as HTMLElement;
      const ev = [...(c.querySelector(".ev")?.classList ?? [])].find((k) => k !== "ev") ?? "";
      c.querySelectorAll(".ev").forEach((x) => x.remove());
      // A label is words then a number; one whose number sits inside a sentence is kept whole, as words.
      const bold = c.querySelector("b");
      const inline = !!bold?.nextSibling?.textContent?.trim();
      const b = inline ? "" : (bold?.textContent?.trim() ?? "");
      if (!inline) bold?.remove();
      const t = (c.textContent ?? "").replace(/\s+/g, " ").trim();
      if (t || b) out.push({ t, b, ev, x: Number(m[1]), y: Number(m[2]) });
    }
    return out;
  }, `#${fig}`);
const keepChips = async (page: Page, fig: string, name: string) => writeFileSync(chipsFile(name), JSON.stringify(await chipsOf(page, fig)));

async function pitViews(w: number, h: number, names: string[]) {
  const page = await open(w, h, 2, HERO_FULL(h));
  await page.waitForSelector("#fig-hero[data-live]");
  // The engine's drillholes, block model and pit limit, solved in a worker after the scene comes up.
  await page.waitForSelector("#fig-hero[data-twin]", { timeout: 40000 }).catch(() => console.log("the twin did not finish; photographing without it"));
  await page.waitForTimeout(1500);
  for (const name of names) {
    const v = PIT[name]!;
    await page.evaluate((lens) => document.querySelector<HTMLButtonElement>(`#fig-hero [data-seg="lens"] [data-v="${lens}"]`)?.click(), v.lens ?? "operator");
    await page.evaluate((view) => {
      const i = Number(document.querySelector<HTMLElement>("#fig-hero [data-stage]")?.dataset.stage ?? 0);
      (window as unknown as { __stages: { orbit: { set(v: unknown, s: number): void } }[] }).__stages[i]!.orbit.set(view, 0);
    }, v);
    await page.waitForTimeout(2600);
    await page.screenshot({ path: shot(name) });
    await keepChips(page, "fig-hero", name);
    have.add(name);
  }
  await page.close();
}
async function sceneShot(name: string, w: number, h: number) {
  const { fig, view, wait } = SCENES[name]!;
  const page = await open(
    w,
    h,
    2,
    `#${fig} .scene-view{position:fixed!important;inset:0!important;z-index:9999;width:${w}px!important;height:${h}px!important;min-height:0!important;border:0!important;border-radius:0!important}#${fig} .scene-view>:not(canvas){display:none!important}.reveal,.fig,.fig *{transform:none!important;filter:none!important;backdrop-filter:none!important;contain:none!important}.fig,.fig-body{overflow:visible!important}`,
  );
  const ok = await page
    .evaluate(async (id) => {
      const el = document.getElementById(id);
      if (!el) return false;
      el.scrollIntoView();
      for (let i = 0; i < 60; i++) {
        if (el.querySelector(".scene-view canvas")) return true;
        await new Promise((r) => setTimeout(r, 250));
      }
      return false;
    }, fig)
    .catch(() => false);
  if (ok) {
    await page.waitForTimeout(wait ?? 6000);
    if (view) {
      await page.evaluate(
        ([id, v]) => {
          const i = Number(document.querySelector<HTMLElement>(`#${id} [data-stage]`)?.dataset.stage ?? -1);
          if (i >= 0) (window as unknown as { __stages: { orbit: { set(v: unknown, s: number): void } }[] }).__stages[i]!.orbit.set(v, 0);
        },
        [fig, view] as const,
      );
      await page.waitForTimeout(2600);
    }
    await page.screenshot({ path: shot(name) });
    await keepChips(page, fig, name);
    have.add(name);
  } else console.log("no scene for", name, "- using", SCENES[name]!.fallback);
  await page.close();
}
const artOf = (name: string) => (have.has(name) || existsSync(shot(name)) ? name : (SCENES[name]?.fallback ?? "pit-wide"));

/* ---------- Cards ---------- */
// One colour per part of the plan, from the chapters' palette.
const GROUP = ["#f2a33c", "#ffd166", "#f2a33c", "#b794f6", "#34d3b5", "#60c3f0", "#f78fb3", "#8be27a", "#ef7f4c"];
/** Sections with an obvious scene. The rest take a view of the pit in turn. */
const CAST: Record<string, string> = {
  introduction: "pit-wide", "1": "pit-wide", "6": "pit-geology", "7": "pit-agent", "8": "pit-floor", "9": "pit-geology",
  "10": "pit-agent", "11": "drill", "12": "drill", "13": "pit-geology", "14": "blast", "15": "pit", "16": "haul",
  "17": "plant", "18": "tailings", "19": "underground", "20": "pit-crusher", "21": "pit-limit", "22": "pit-limit",
  "23": "pit-limit", "24": "pit-geology", "30": "pit-agent", "31": "underground",
};
const turn = ["pit-wide", "pit-geology", "pit-top", "pit-limit", "pit-crusher", "pit-agent", "pit-floor", "pit-drill"];
interface Card {
  id: string;
  eyebrow: string;
  title: string;
  sub: string;
  tag: string;
  numeral: string;
  colour: string;
  art: string;
}
const cards: Card[] = entries.map((e, i) => {
  const [, name, rest] = e.title.match(/^([A-Z][\w-]*(?: [IVX]+)?): (.+)$/) ?? [, e.title, ""];
  const appendix = e.id.match(/^appendix-(.)$/)?.[1]?.toUpperCase();
  return {
    id: e.id,
    eyebrow: e.group === "Overview" ? "The plan" : e.group.replace(/ \(.+\)$/, ""),
    title: e.id === "introduction" ? "The whole plan" : name!.replace(/ \(.+\)$/, ""),
    sub: rest ? rest[0]!.toUpperCase() + rest.slice(1) : e.id === "introduction" ? "Contents, scope and how to read it" : "",
    tag: e.id === "introduction" ? "Introduction" : e.label,
    numeral: appendix ?? (e.id === "introduction" ? "0" : e.id),
    colour: GROUP[groups.indexOf(e.group)] ?? "#f2a33c",
    art: CAST[e.id] ?? turn[i % turn.length]!,
  };
});
cards.push(
  { id: "library", eyebrow: "The plan", title: "The full design", sub: `${entries.length - 1} sections and appendices, cross-linked`, tag: "Library", numeral: "", colour: "#f2a33c", art: "pit-top" },
  { id: "search", eyebrow: "Search", title: "Find anything", sub: "Claims, refusals, kriging, tailings, AISC and the maths", tag: "Search", numeral: "", colour: "#60c3f0", art: "underground" },
);

const css = `@font-face{font-family:Inter;src:url(${base}/fonts/inter-latin.woff2);font-weight:100 900}@font-face{font-family:Mono;src:url(${base}/fonts/mono-latin.woff2);font-weight:100 800}
*{box-sizing:border-box;margin:0}body{width:1200px;overflow:hidden;background:#070505;color:#fff;font-family:Inter;font-optical-sizing:auto;-webkit-font-smoothing:antialiased}
.card{position:relative;width:1200px;height:var(--h);overflow:hidden;background:radial-gradient(900px 620px at 84% 46%,color-mix(in srgb,var(--c) 18%,transparent),transparent 70%),#070505}
.brand{position:absolute;left:64px;top:50px;display:flex;align-items:center;gap:15px;font-size:29px;font-weight:800;letter-spacing:-.03em;z-index:2}
.mark{width:52px;height:42px;overflow:visible}.mark .frame,.mark .ground,.mark .wheel{fill:none;stroke:#f2a33c;stroke-width:2.2;stroke-linecap:round;stroke-linejoin:round}.mark .ground{opacity:.5}.mark .hub{fill:#f2a33c}
.where{position:absolute;right:54px;top:54px;font:500 19px Mono;color:rgb(255 255 255/.85);letter-spacing:-.01em;padding:8px 15px;border:1px solid rgb(255 255 255/.16);border-radius:12px;background:rgb(7 5 5/.62);z-index:2}
.eyebrow{display:flex;align-items:center;gap:13px;font:600 18px/1.2 Mono;letter-spacing:.13em;text-transform:uppercase;color:var(--c)}
.eyebrow::before{content:"";width:11px;height:11px;border-radius:50%;background:var(--c);box-shadow:0 0 0 5px color-mix(in srgb,var(--c) 22%,transparent)}
.line{position:absolute;inset:auto 0 0;height:7px;background:linear-gradient(90deg,#ffe39a,#ffc35c 18%,#f2913a 38%,#e2603b 54%,#b794f6 72%,#60c3f0 86%,#8be27a)}
.tick{position:absolute;width:20px;height:20px;border:2px solid rgb(255 255 255/.22);z-index:2}
.tick.a{left:24px;top:24px;border-right:0;border-bottom:0}.tick.b{right:24px;top:24px;border-left:0;border-bottom:0}`;

/**
 * The labels the card's scene was showing when it was photographed, placed where they stood: at most three, clear of
 * the card's words and of each other. Each is the scene's own claim with its evidence class, never one written for
 * the card.
 */
function chips(artName: string, h: number, left = 700) {
  const file = chipsFile(artName);
  if (!existsSync(file)) return "";
  const dy = (630 - h) / 2;
  const kept: Chip[] = [];
  for (const k of (JSON.parse(readFileSync(file, "utf8")) as Chip[]).sort((a, b) => a.y - b.y)) {
    const y = k.y - dy;
    if (k.x < left - 60 || k.x > 1160 || y < 128 || y > h - 104) continue;
    // Width at the card's type sizes: words in Inter 17px, number in mono 18px, evidence badge, padding and dot.
    const w = k.t.length * 8.6 + k.b.length * 11 + (k.ev ? k.ev.length * 8.4 + 40 : 0) + 64;
    const flip = k.x > 860;
    const x0 = flip ? k.x + 11 - w : k.x - 11;
    if (x0 < left || x0 + w > 1188) continue;
    if (kept.some((o) => Math.abs(o.y - dy - y) < 50 && Math.abs(o.x - k.x) < 330)) continue;
    kept.push({ ...k, flip } as Chip & { flip: boolean });
    if (kept.length === 3) break;
  }
  return kept
    .map((k) => {
      const flip = (k as Chip & { flip?: boolean }).flip;
      const at = flip ? `right:${(1200 - k.x - 11).toFixed(0)}px` : `left:${(k.x - 11).toFixed(0)}px`;
      return `<div class="chip${flip ? " flip" : ""}" style="${at};top:${(k.y - dy).toFixed(0)}px">${k.t ? `<span class="ct">${escape(k.t)}</span>` : ""}${k.b ? `<b>${escape(k.b)}</b>` : ""}${k.ev ? `<i class="ev ${k.ev}">${k.ev}</i>` : ""}</div>`;
    })
    .join("");
}
const chipCss = `.chip{position:absolute;z-index:2;display:flex;align-items:center;gap:10px;transform:translateY(-100%);padding:9px 15px 9px 12px;border:1px solid rgb(255 255 255/.2);border-radius:99px;background:rgb(12 10 9/.8);backdrop-filter:blur(10px);font:500 17px/1.2 Inter;color:#e9dfd5;white-space:nowrap;box-shadow:0 10px 30px rgb(0 0 0/.35)}
.chip::before{content:"";width:11px;height:11px;border-radius:50%;background:var(--c);box-shadow:0 0 0 4px rgb(255 255 255/.07),0 0 14px var(--c)}
.chip::after{content:"";position:absolute;left:16px;top:100%;width:1.5px;height:30px;background:linear-gradient(rgb(255 255 255/.5),transparent)}
.chip.flip{padding:9px 12px 9px 15px}.chip.flip::before{order:9}.chip.flip::after{left:auto;right:16px}
.chip b{font:650 18px/1 Mono;color:#fff;letter-spacing:-.01em}
.chip .ev{font:650 12px/1.4 Mono;font-style:normal;letter-spacing:.07em;text-transform:uppercase;padding:2px 9px;border-radius:99px;border:1px solid currentColor}
.ev.measured{color:#5fd38d;background:rgb(95 211 141/.12)}.ev.derived{color:#2dd4bf;background:rgb(45 212 191/.12)}.ev.estimated{color:#6cc8f7;background:rgb(108 200 247/.12)}.ev.assumed{color:#fbbf24;background:rgb(251 191 36/.12)}.ev.stale{color:#f2913a;background:rgb(242 145 58/.12)}.ev.refused{color:#fb7185;background:rgb(251 113 133/.12)}`;

const portraitCard = (c: Card, h: number) => `<div class="card" style="--c:${c.colour};--h:${h}px"><style>${chipCss}
.art{position:absolute;inset:0;width:1200px;height:100%;object-fit:cover;filter:saturate(1.05) contrast(1.04)}
.num{position:absolute;right:${c.numeral.length > 2 ? 44 : 56}px;bottom:-34px;font-size:${c.numeral.length > 3 ? 300 : c.numeral.length > 2 ? 350 : 430}px;line-height:1;font-weight:800;letter-spacing:-.07em;color:transparent;-webkit-text-stroke:2px color-mix(in srgb,var(--c) 42%,transparent);z-index:1}
.veil{position:absolute;inset:0;background:linear-gradient(90deg,#070505 10%,rgb(7 5 5/.9) 36%,rgb(7 5 5/.35) 60%,transparent 78%),linear-gradient(0deg,rgb(7 5 5/.8),transparent 34%),linear-gradient(180deg,rgb(7 5 5/.55),transparent 22%)}
.copy{position:absolute;left:64px;top:122px;bottom:118px;width:640px;display:flex;flex-direction:column;justify-content:center;z-index:2}h1 span.k{white-space:nowrap}
h1{margin-top:22px;font-weight:800;letter-spacing:-.052em;line-height:.99;text-wrap:balance;padding-bottom:.08em}
h1 em{font-style:normal;background:linear-gradient(100deg,color-mix(in srgb,var(--c) 60%,#fff),var(--c) 60%);-webkit-background-clip:text;color:transparent;padding-right:.06em;margin-right:-.06em}
.sub{margin-top:20px;font-size:29px;line-height:1.34;font-weight:450;letter-spacing:-.018em;color:#ddd0c4;text-wrap:balance}
.foot{position:absolute;left:64px;bottom:46px;display:flex;gap:12px;z-index:2}
.pill{font:600 19px/1 Mono;padding:12px 17px;border-radius:13px;border:1px solid color-mix(in srgb,var(--c) 44%,transparent);background:color-mix(in srgb,var(--c) 14%,#070505);color:color-mix(in srgb,var(--c) 50%,#fff)}
.pill.b{border-color:rgb(255 255 255/.14);background:rgb(12 10 9/.8);color:#d6c8bb;font-weight:500}
</style><img class="art" src="${base}/.art/${c.art}.png"><div class="veil"></div>${chips(c.art, h)}${c.numeral ? `<span class="num">${c.numeral}</span>` : ""}<i class="tick a"></i><i class="tick b"></i><div class="brand">${mark}FrankenMining</div><div class="copy"><p class="eyebrow">${escape(c.eyebrow)}</p><h1 data-fit>${escape(c.title).replace(/(\S+)$/, "<em>$1</em>").replace(/(?<![<\/\w])\w+-\w+(?![^<]*>)/g, '<span class="k">$&</span>')}<span style="margin-left:-.02em">.</span></h1>${c.sub ? `<p class="sub">${escape(c.sub)}</p>` : ""}</div><div class="foot"><span class="pill">${escape(c.tag)}</span><span class="pill b">System of record for mining</span><span class="pill b">frankenmining.com</span></div><i class="line"></i></div>`;

const homeCard = (h: number) => `<div class="card" style="--c:#f2a33c;--h:${h}px"><style>${chipCss}
.art{position:absolute;inset:0;width:1200px;height:100%;object-fit:cover;filter:brightness(1.08) saturate(1.08)}
.veil{position:absolute;inset:0;background:linear-gradient(90deg,#070505 3%,rgb(7 5 5/.94) 28%,rgb(7 5 5/.42) 48%,transparent 62%),linear-gradient(0deg,rgb(7 5 5/.92),transparent 32%),linear-gradient(180deg,rgb(7 5 5/.6),transparent 20%)}
.copy{position:absolute;left:64px;top:${h > 615 ? 136 : 124}px;width:760px;z-index:2}
h1{margin-top:24px;font-size:84px;font-weight:800;letter-spacing:-.058em;line-height:.95;text-shadow:0 6px 40px rgb(7 5 5/.8)}
h1 em{font-style:normal;background:linear-gradient(100deg,#ffe39a,#ffc35c 30%,#f2913a 62%,#e2603b);-webkit-background-clip:text;color:transparent;padding:0 .06em .12em 0;margin-right:-.06em;text-shadow:none}
.foot{position:absolute;left:64px;bottom:${h > 615 ? 50 : 40}px;display:flex;gap:12px;z-index:2}
.pill{font:600 18px/1 Mono;padding:12px 17px;border-radius:13px;border:1px solid rgb(242 163 60/.4);background:rgb(30 18 8/.88);color:#ffd9a6}
.hud{position:absolute;right:54px;bottom:${h > 615 ? 48 : 38}px;display:flex;align-items:center;gap:10px;font:500 18px/1.35 Mono;color:#f3ebe3;padding:10px 15px;border:1px solid rgb(255 255 255/.16);border-radius:99px;background:rgb(12 10 9/.82);z-index:2}
.hud::before{content:"";width:11px;height:11px;border-radius:50%;background:#f2a33c}
.hud b{font-weight:700;color:#fff}.hud i{font:600 13px/1.4 Mono;font-style:normal;letter-spacing:.06em;color:#5fd38d;border:1px solid currentColor;border-radius:99px;padding:1px 8px;background:rgb(95 211 141/.1)}
</style><img class="art" src="${base}/.art/hero-${h}.png"><div class="veil"></div>${chips(`hero-${h}`, h, 800)}<div class="brand">${mark}FrankenMining</div><span class="where">frankenmining.com</span><div class="copy"><p class="eyebrow">System of record for mining</p><h1>Every number<br>in the mine,<br>with its <em>evidence</em>.</h1></div><div class="foot"><span class="pill">11 of 11 pits exact</span><span class="pill">22,732 figures as claims</span><span class="pill">0 third-party crates</span></div><i class="line"></i></div>`;

const manifestPath = join(dir, "manifest.json");
const manifest: Record<string, string> = existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, "utf8")) : {};
// A card is redrawn when its markup changes. A change to the scenes themselves needs --all.
const key = (html: string) => Bun.hash((css + html).replaceAll(base, "")).toString(16);
const html: Record<string, string> = { home: homeCard(630), "home-wide": homeCard(600) };
for (const c of cards) html[c.id] = portraitCard(c, 630);
const stale = (id: string) =>
  all || manifest[id] !== key(html[id]!) || !existsSync(join(dir, `${id}.jpg`)) || (!id.startsWith("home") && !existsSync(join(dir, `${id}-wide.jpg`)));
const due = cards.filter((c) => stale(c.id));
const homeDue = stale("home") || stale("home-wide");

if (due.length || homeDue) {
  // The hero, full frame, for the home card.
  if (homeDue || all)
    for (const h of [630, 600]) {
      if (!all && existsSync(shot(`hero-${h}`))) continue;
      const page = await open(1200, h, 2, HERO_FULL(h));
      await page.waitForSelector("#fig-hero[data-live]");
      // The home card shows the geologist's desk: the drillholes and the engine's block model glowing through the rock.
      await page.waitForSelector("#fig-hero[data-twin]", { timeout: 40000 }).catch(() => console.log("the twin did not finish"));
      await page.evaluate(() => document.querySelector<HTMLButtonElement>('#fig-hero [data-seg="lens"] [data-v="geology"]')?.click());
      await page.waitForTimeout(2500);
      await page.screenshot({ path: shot(`hero-${h}`) });
      await keepChips(page, "fig-hero", `hero-${h}`);
      await page.close();
    }
  const wanted = new Set(due.map((c) => c.art));
  const views = Object.keys(PIT).filter((v) => all || !existsSync(shot(v)));
  // Every fallback must exist, whichever scenes draw.
  for (const s of Object.keys(SCENES)) if (wanted.has(s) && !existsSync(shot(SCENES[s]!.fallback))) views.push(SCENES[s]!.fallback);
  if (views.length) await pitViews(1200, 630, [...new Set(views)]);
  for (const s of Object.keys(SCENES)) if (wanted.has(s) && (all || !existsSync(shot(s)))) await sceneShot(s, 1200, 630);
  for (const c of due) c.art = artOf(c.art);

  const page = await browser.newPage({ viewport: { width: 1200, height: 630 }, deviceScaleFactor: 1 });
  const draw = async (name: string, h: number, of?: Card) => {
    await page.setViewportSize({ width: 1200, height: h });
    const c = of ?? cards.find((x) => x.id === name);
    await page.setContent(`<!doctype html><meta charset="utf-8"><style>${css}</style>${c ? portraitCard(c, h) : html[name]}`, { waitUntil: "load" });
    await page.evaluate(async () => {
      await document.fonts.ready;
      await Promise.all([...document.images].map((i) => i.decode()));
      // The largest title that keeps its longest word whole and the block inside its column.
      const t = document.querySelector<HTMLElement>("[data-fit]");
      const box = t?.parentElement;
      if (t && box)
        for (let s = 128; s >= 44; s -= 2) {
          t.style.fontSize = `${s}px`;
          if (t.scrollWidth <= box.clientWidth && box.scrollHeight <= box.clientHeight) break;
        }
    });
    await page.waitForTimeout(60);
    await page.screenshot({ path: join(dir, `${name}.jpg`), type: "jpeg", quality: name.startsWith("home") ? 90 : 84 });
    if (html[name]) manifest[name] = key(html[name]!);
  };
  if (homeDue) {
    await draw("home", 630);
    await draw("home-wide", 600);
  }
  for (const c of due) {
    await draw(c.id, 630);
    await draw(`${c.id}-wide`, 600, c);
  }
  writeFileSync(manifestPath, JSON.stringify(Object.fromEntries(Object.entries(manifest).sort()), null, 1) + "\n");
}
const size = [...cards.flatMap((c) => [c.id, `${c.id}-wide`]), "home", "home-wide"].reduce((n, id) => n + (statSync(join(dir, `${id}.jpg`), { throwIfNoEntry: false })?.size ?? 0), 0);
console.log(`${due.length + (homeDue ? 2 : 0)} cards drawn; ${cards.length + 2} in media/share, ${(size / 1e6).toFixed(1)} MB.`);
await browser.close();
server.stop(true);
