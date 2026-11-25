import { escape } from "./html";
import { icon } from "./parts";

export { escape };
export const origin = "https://frankenmining.com";
/** Where this build is served. Link previews are fetched from here; the canonical address is always `origin`. */
export const host = process.env.SITE_ORIGIN?.replace(/\/$/, "") || origin;
export const repo = "https://github.com/Dicklesworthstone/franken_mining";

/** A headframe over its shaft. site.js turns the sheave wheel as the page scrolls; without scripts it stands still. */
export const mark = `<svg class="mark" viewBox="0 0 42 34" aria-hidden="true"><path class="ground" d="M3 32.5h36"/><path class="frame" d="M12 32 19 12.5M30 32 23 12.5M14.6 25h12.8M16.9 18.5h8.2M27 32l8.5-15.5-10.3-4"/><g class="wheel"><circle cx="21" cy="8" r="5.4"/><path d="M21 2.6v10.8M15.6 8h10.8"/></g><circle class="hub" cx="21" cy="8" r="1.6"/></svg>`;

export interface Shell {
  title: string;
  path: string;
  body: string;
  description?: string;
  /** The middle of the header: the chapters of the home page, or the site's sections. */
  nav?: string;
  dock?: string;
  head?: string;
  solid?: boolean;
  /** The page's share card, with its content hash, and what it shows. */
  share: { src: string; wide?: string; alt: string };
  version: (file: string) => string;
}

const site = (path: string, inMenu = false) =>
  [
    ["/", "Overview"],
    ["/plan/", "The plan"],
    ["/search/", "Search"],
  ]
    .filter(([to]) => !(inMenu && path === "" && to === "/"))
    .map(
      ([to, label]) =>
        `<a href="${to}"${path === to || (to === "/plan/" && path.startsWith("/plan/")) ? ' aria-current="page"' : ""}><span class="joint" aria-hidden="true"></span>${label}</a>`,
    )
    .join("");

export const DESCRIPTION =
  "A system of record for mining companies and their AI agents, in which every assay, truckload, recovery and cost per ounce carries its evidence, its uncertainty and its track record.";

export function shell(o: Shell) {
  const description = o.description ?? DESCRIPTION;
  const full = o.path === "/" ? o.title : `${o.title} · FrankenMining`;
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"><meta name="theme-color" content="#0a0807"><title>${escape(full)}</title><meta name="description" content="${escape(description)}"><link rel="canonical" href="${origin + o.path}"><meta property="og:type" content="website"><meta property="og:site_name" content="FrankenMining"><meta property="og:title" content="${escape(full)}"><meta property="og:description" content="${escape(description)}"><meta property="og:url" content="${host + o.path}"><meta property="og:image" content="${host + o.share.src}"><meta property="og:image:type" content="image/jpeg"><meta property="og:image:width" content="1200"><meta property="og:image:height" content="630"><meta property="og:image:alt" content="${escape(o.share.alt)}"><meta name="twitter:card" content="summary_large_image"><meta name="twitter:title" content="${escape(full)}"><meta name="twitter:description" content="${escape(description)}"><meta name="twitter:image" content="${host + (o.share.wide ?? o.share.src)}"><meta name="twitter:image:alt" content="${escape(o.share.alt)}"><link rel="icon" href="/icon.svg" type="image/svg+xml"><link rel="preload" href="/fonts/inter-latin.woff2" as="font" type="font/woff2" crossorigin><link rel="stylesheet" href="${o.version("/site.css")}">${o.head ?? ""}<script type="module" src="${o.version("/site.js")}"></script></head>
<body class="${[o.solid && "solid", o.dock && "docked"].filter(Boolean).join(" ")}"><a class="skip" href="#main">Skip to content</a><header class="top"><div class="bar"><a href="/" class="brand">${mark}FrankenMining</a><nav class="chapters" aria-label="${o.nav ? "Chapters" : "Site"}">${o.nav ?? site(o.path)}</nav><div class="tools"><a class="find" href="/search/" data-palette aria-label="Search the plan">${icon("search")}<span>Search</span><kbd>/</kbd></a><a class="btn go sm" href="${repo}">GitHub${icon("out")}</a><details class="menu"><summary aria-label="Menu">${icon("menu")}</summary><nav aria-label="Menu">${o.nav ? o.nav + site("", true) : site(o.path, true)}<a class="btn go" href="${repo}">GitHub${icon("out")}</a></nav></details></div></div><span class="progress" aria-hidden="true"><i></i></span></header>
<main id="main">${o.body}</main><footer class="end"><div class="wrap"><a href="/" class="brand">${mark}FrankenMining</a><span>Designed in the open. Every mine on this site is synthetic; the public-assurance counts come from published workbooks.</span><a href="/plan/">The plan</a><a href="/plan.md" download>Markdown</a><a href="${repo}">GitHub</a></div></footer>${o.dock ? `<nav class="dock" aria-label="Quick navigation"><ul>${o.dock}</ul></nav>` : ""}</body></html>`;
}
