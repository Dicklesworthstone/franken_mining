import { blurbs } from "./blurbs";
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { home, homeNav } from "./home";
import { keep } from "./html";
import { origin, shell, type Shell } from "./layout";
import { bytes, definitions, entries, ref, render, version, words } from "./plan";
import { library, missing, reader, readerDock, search } from "./reader";

const root = resolve(import.meta.dir, "..");
const out = process.env.OUT_DIR ? resolve(process.env.OUT_DIR) : join(root, "out");
// Pagefind leaves content-addressed shards from previous runs in its output directory.
rmSync(out, { recursive: true, force: true });

function emit(path: string, data: string | Uint8Array) {
  const dest = join(out, path);
  mkdirSync(dirname(dest), { recursive: true });
  writeFileSync(dest, data);
}
const hash = (data: string | Uint8Array) => Bun.hash(data).toString(16).padStart(16, "0").slice(0, 10);
/** Files the build writes in place of their copy in static/. */
const built = new Map<string, string | Uint8Array>();
// The home page's stylesheet, followed by each figure's own (static/fig/*.css), as one file.
const figDir = join(root, "static/fig");
built.set(
  "/home.css",
  [
    readFileSync(join(root, "static/home.css"), "utf8"),
    ...(existsSync(figDir) ? readdirSync(figDir).filter((f) => f.endsWith(".css")).sort() : []).map((f) =>
      readFileSync(join(figDir, f), "utf8"),
    ),
  ].join("\n"),
);
const versions = new Map<string, string>();
/** A static file's address, with its content hash so that a deploy never mixes two versions. */
function asset(file: string) {
  if (!versions.has(file))
    versions.set(file, `${file}?v=${hash(built.get(file) ?? readFileSync(join(root, "static", file)))}`);
  return versions.get(file)!;
}

/* ---------- The engine ---------- */
// FrankenMining's own Rust, compiled to WebAssembly by `bun run wasm`. The figures find it through a meta tag.
const wasmFile = join(root, "wasm/pkg/engine.wasm");
if (!existsSync(wasmFile)) throw Error("The engine is not built: run `bun run wasm` first.");
const wasmBytes = readFileSync(wasmFile);
const engine = `/assets/engine-${hash(wasmBytes)}.wasm`;

/* ---------- Scripts ---------- */
// The agent figure quotes the CLI's registries: their rows are counted here, so the page follows the CLI.
const rows = (file: string, table: string) =>
  readFileSync(join(root, "..", "registries", file), "utf8").split("\n").filter((l) => l.trim() === `[[${table}]]`).length;
const bundle = await Bun.build({
  entrypoints: [join(root, "src/client/home.ts")],
  define: { __FM_VERBS__: String(rows("verbs.toml", "verb")), __FM_REFUSALS__: String(rows("refusals.toml", "refusal")) },
  target: "browser",
  format: "esm",
  splitting: true,
  // FM_PROFILE=1 keeps names readable in a CPU profile.
  minify: !process.env.FM_PROFILE,
  naming: { entry: "[name]-[hash].[ext]", chunk: "[name]-[hash].[ext]" },
});
if (!bundle.success) throw new AggregateError(bundle.logs, "The figures did not bundle");
let client = "";
for (const file of bundle.outputs) {
  const path = `/assets/${file.path.replace(/^\.?\//, "")}`;
  if (file.kind === "entry-point") client = path;
  emit(path, new Uint8Array(await file.arrayBuffer()));
}

/* ---------- Pages ---------- */
/** A page's share card, drawn by `bun run stills`. A page without one yet shows the home card. */
function share(id: string, alt: string): Shell["share"] {
  const file = (name: string) => {
    const path = `/media/share/${name}.jpg`;
    const disk = join(root, path);
    return existsSync(disk) ? `${path}?v=${hash(readFileSync(disk))}` : undefined;
  };
  const home = {
    src: file("home")!,
    wide: file("home-wide"),
    alt: "An open-pit mine at golden hour, haul trucks on the ramp, with labels showing the evidence behind each number. Headline: Every number in the mine, with its evidence.",
  };
  if (id === "home") return home;
  const src = file(id);
  // X shows a 1200×600 card; every page has its own beside the 1200×630 one.
  return src ? { src, alt, wide: file(`${id}-wide`) } : home;
}
const page = (o: Omit<Shell, "version">) =>
  emit(`${o.path}index.html`, shell({ ...o, version: asset }));
const round = (v: number) => (Math.round(v / 1000) * 1000).toLocaleString("en-US");
// The class is set before the first paint, so that content waits for its entrance only where scripts run.
const js = `<script>document.documentElement.classList.add("js")</script>`;
page({
  path: "/",
  share: share("home", ""),
  title: "FrankenMining · Every number in the mine, with its evidence",
  body: keep(home({ ref, sections: entries.length - 1, words: round(words) })),
  nav: homeNav,
  head: `${js}<meta name="fmine-engine" content="${engine}"><link rel="stylesheet" href="${asset("/home.css")}"><link rel="modulepreload" href="${client}"><script type="module" src="${client}" onerror="document.documentElement.classList.remove('js')"></script>`,
});
for (const entry of entries) {
  const { html, used } = render(entry);
  const cards: Record<string, [string, string]> = {};
  for (const id of used) {
    const d = definitions.get(id)!;
    cards[id] = [d.entry.label, d.text];
  }
  emit(`${entry.url}definitions.json`, JSON.stringify(cards));
  page({
    path: entry.url,
    share: share(entry.id, `${entry.label}, ${entry.title}. A headframe over a dark mine beside the section's name.`),
    title: `${entry.label} · ${entry.title}`,
    description: blurbs[entry.id] ?? (entry.summary || undefined),
    body: reader(entry, html),
    dock: readerDock(entry),
    head: js,
    solid: true,
  });
}
page({ path: "/plan/", share: share("library", "Every section of the plan, over a terraced open pit."), title: "Plan library", body: library(), head: js, solid: true });
page({ path: "/search/", share: share("search", "Find anything in the plan, over a cutaway of a mine."), title: "Search the plan", body: search, head: js, solid: true });
emit("404.html", shell({ path: "/404/", share: share("home", ""), title: "Page not found", body: missing, head: js, solid: true, version: asset }));

/* ---------- Files ---------- */
emit("plan.md", bytes);
// What the search palette can jump to without reading the index.
const NUMBERED = /^(\d+(?:\.\d+)+|[A-O]\.\d+(?:\.\d+)*|C-\d+)\.?\s+(.+)$/;
const jumps = entries.flatMap((e) => [
  [e.label, e.title, e.url],
  ...e.sub.flatMap((s) => {
    const m = s.title.match(NUMBERED);
    return m ? [[/^\d/.test(m[1]!) ? `§${m[1]}` : m[1]!, m[2]!, `${e.url}#${s.id}`]] : [];
  }),
]);
emit("jump.json", JSON.stringify({ sections: jumps, ids: Object.fromEntries([...definitions].map(([id, d]) => [id, d.href])) }));
cpSync(join(root, "static"), out, { recursive: true, filter: (from) => !from.startsWith(figDir) });
for (const [file, data] of built) emit(file, data);
emit(engine, wasmBytes);
if (existsSync(join(root, "media"))) cpSync(join(root, "media"), join(out, "media"), { recursive: true });
for (const [font, pkg, file] of [
  ["inter-latin", "inter", "inter-latin-opsz-normal"],
  ["inter-greek", "inter", "inter-greek-opsz-normal"],
  ["mono-latin", "jetbrains-mono", "jetbrains-mono-latin-wght-normal"],
  ["mono-greek", "jetbrains-mono", "jetbrains-mono-greek-wght-normal"],
] as const) {
  const dir = join(root, "node_modules/@fontsource-variable", pkg);
  emit(`fonts/${font}.woff2`, readFileSync(join(dir, "files", `${file}.woff2`)));
  if (!existsSync(join(out, `fonts/${pkg}-license.txt`))) emit(`fonts/${pkg}-license.txt`, readFileSync(join(dir, "LICENSE")));
}
emit(
  "sitemap.xml",
  `<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${[
    "/",
    "/plan/",
    "/search/",
    ...entries.map((e) => e.url),
  ]
    .map((p) => `<url><loc>${origin}${p}</loc></url>`)
    .join("")}</urlset>`,
);
emit("robots.txt", `User-agent: *\nAllow: /\nSitemap: ${origin}/sitemap.xml\n`);
// Read by hosts that take a headers file; everything under these paths has a hash in its name.
emit(
  "_headers",
  "/assets/*\n  Cache-Control: public, max-age=31536000, immutable\n/assets/*.wasm\n  Content-Type: application/wasm\n/fonts/*\n  Cache-Control: public, max-age=31536000, immutable\n",
);
console.log(
  `Built plan ${version}: ${entries.length} reader entries, ${definitions.size} linked identifiers, ${bytes.length} Markdown bytes.`,
);
