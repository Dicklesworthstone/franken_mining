const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];
const calm = matchMedia("(prefers-reduced-motion: reduce)").matches;

function el(tag, attributes = {}, ...children) {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes))
    if (value !== undefined && value !== false) node.setAttribute(name, value === true ? "" : value);
  node.append(...children.filter((c) => c !== undefined));
  return node;
}
const SEARCH_ICON =
  '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.6-3.6"/></svg>';

/* ---------- Header ---------- */
const bar = $(".progress i");
// The headframe's sheave wheel turns as the page scrolls; without scripts it stands still.
const wheels = $$(".top .mark .wheel");
for (const wheel of wheels) {
  wheel.style.transformBox = "view-box";
  wheel.style.transformOrigin = "21px 8px";
}
let queued = 0;
function scrolled() {
  queued = 0;
  document.body.classList.toggle("scrolled", scrollY > 24);
  const room = document.documentElement.scrollHeight - innerHeight;
  if (bar) bar.style.transform = `scaleX(${room > 0 ? Math.min(1, scrollY / room) : 0})`;
  if (calm) return;
  for (const wheel of wheels) wheel.style.transform = `rotate(${(scrollY / 3).toFixed(1)}deg)`;
}
addEventListener("scroll", () => (queued ||= requestAnimationFrame(scrolled)), { passive: true });
scrolled();

const menu = $(".menu");
if (menu) {
  addEventListener("click", (event) => {
    if (menu.open && (!menu.contains(event.target) || event.target.closest("a"))) menu.open = false;
  });
  addEventListener("keydown", (event) => {
    if (event.key !== "Escape" || !menu.open) return;
    menu.open = false;
    $("summary", menu).focus();
  });
}

/* ---------- Search engine ---------- */
let engine;
let attempts = 0;
// Some browsers remember a failed module fetch for the life of the page, so each retry asks for a new address.
function pagefind() {
  engine ??= import(`/pagefind/pagefind.js${attempts ? `?r=${attempts}` : ""}`).then(async (module) => {
    await module.options({ excerptLength: 24 });
    await module.init();
    return module;
  });
  return engine;
}
async function find(term) {
  try {
    const module = await pagefind();
    const response = await module.search(term);
    return response.results;
  } catch (error) {
    engine = undefined;
    attempts++;
    throw error;
  }
}
async function load(results, from, to) {
  try {
    return await Promise.all(results.slice(from, to).map((r) => r.data()));
  } catch (error) {
    engine = undefined;
    attempts++;
    throw error;
  }
}
let headings;
const jumps = () =>
  (headings ??= fetch("/jump.json")
    .then((r) => (r.ok ? r.json() : Promise.reject(Error(r.statusText))))
    .catch(() => {
      headings = undefined;
      return { sections: [], ids: {} };
    }));
/** Sections and identifiers the term names outright: 25.13, C-17, GATE-SPARK-21, "clearance". */
async function direct(term, limit) {
  const { sections, ids } = await jumps();
  const clean = term.replace(/^§\s*/, "").toLowerCase();
  if (clean.length < 2) return [];
  const scored = [];
  for (const [label, title, url] of sections) {
    const l = label.replace(/^§/, "").toLowerCase();
    const t = title.toLowerCase();
    const score = l === clean ? 5 : t === clean ? 4 : l.startsWith(clean) && /\d/.test(clean) ? 3 : t.startsWith(clean) ? 2 : t.includes(clean) ? 1 : 0;
    if (score) scored.push({ score: score + 1 / title.length, label, title, url });
  }
  const out = scored.sort((a, b) => b.score - a.score).slice(0, limit);
  const id = clean.toUpperCase();
  if (ids[id]) out.unshift({ label: id, title: "Go to its definition", url: ids[id] });
  return out.slice(0, limit);
}
const place = (url) => {
  const id = new URL(url, location.href).pathname.split("/")[2] ?? "";
  return id.startsWith("appendix-") ? `Appendix ${id.at(-1).toUpperCase()}` : /^\d/.test(id) ? `§${id}` : "Plan";
};
/** Pagefind escapes the source text and wraps matching words in mark elements. */
function excerpt(html) {
  const fragment = document.createDocumentFragment();
  const decoder = document.createElement("textarea");
  let parent = fragment;
  for (const part of html.split(/(<\/?mark>)/i)) {
    if (part === "<mark>") {
      parent = document.createElement("mark");
      fragment.append(parent);
    } else if (part === "</mark>") parent = fragment;
    else {
      decoder.innerHTML = part.replaceAll("<", "&lt;").replaceAll(">", "&gt;");
      parent.append(document.createTextNode(decoder.value));
    }
  }
  return fragment;
}
function hit(doc) {
  const best = doc.sub_results?.[0];
  const heading = best?.title && best.title !== doc.meta.title ? best.title : doc.meta.title;
  const text = el("p");
  text.append(excerpt(best?.excerpt || doc.excerpt));
  return el(
    "a",
    { class: "hit", href: best?.url || doc.url },
    el("span", { class: "where" }, el("b", {}, place(doc.url)), el("span", {}, doc.meta.title)),
    el("strong", {}, heading),
    text,
  );
}
const jump = (j) =>
  el("a", { class: "hit", href: j.url }, el("span", { class: "where" }, el("b", {}, j.label), el("span", {}, "Section")), el("strong", {}, j.title));
function failure(retry) {
  const again = el("button", { type: "button", class: "btn sm go" }, "Try again");
  again.addEventListener("click", retry);
  const reload = el("button", { type: "button", class: "btn sm" }, "Reload the page");
  reload.addEventListener("click", () => location.reload());
  return el("div", { class: "note" }, el("p", {}, "The search index could not be fetched."), el("div", { class: "btnrow" }, again, reload));
}

/* ---------- Palette ---------- */
let palette;
function buildPalette() {
  const input = el("input", {
    type: "search",
    "aria-label": "Search the plan",
    role: "combobox",
    "aria-expanded": "true",
    "aria-controls": "palette-hits",
    "aria-autocomplete": "list",
    placeholder: matchMedia("(max-width: 599px)").matches ? "Search the plan" : "Search the plan, or type a section such as 25.13",
    autocomplete: "off",
    spellcheck: "false",
    maxlength: "240",
  });
  const hits = el("div", { class: "hits", id: "palette-hits", role: "listbox", "aria-label": "Results" });
  const all = el("a", { href: "/search/" }, "All results");
  const q = el("div", { class: "q" });
  q.innerHTML = SEARCH_ICON;
  q.append(input);
  const close = el("button", { type: "button", class: "esc", "aria-label": "Close search" }, matchMedia("(hover: none)").matches ? "Close" : "Esc");
  q.append(close);
  const key = (k, what) => el("span", {}, el("kbd", {}, k), ` ${what}`);
  const dialog = el(
    "dialog",
    { class: "palette", "aria-label": "Search the plan" },
    q,
    hits,
    el("div", { class: "keys" }, key("↑ ↓", "choose"), key("↵", "open"), all),
  );
  document.body.append(dialog);
  let revision = 0;
  let timer = 0;
  let active = -1;
  const options = () => $$(".hit", hits);
  const choose = (i) => {
    const list = options();
    if (!list.length) return;
    active = (i + list.length) % list.length;
    list.forEach((o, j) => o.setAttribute("aria-selected", String(j === active)));
    input.setAttribute("aria-activedescendant", list[active].id);
    list[active].scrollIntoView({ block: "nearest" });
  };
  const show = (nodes) => {
    hits.replaceChildren(...nodes);
    options().forEach((o, i) => {
      o.id = `hit-${i}`;
      o.setAttribute("role", "option");
      o.setAttribute("aria-selected", "false");
      o.tabIndex = -1;
      o.addEventListener("pointermove", () => {
        if (active !== i) choose(i);
      });
    });
    active = -1;
    if (options().length) choose(0);
    else input.removeAttribute("aria-activedescendant");
  };
  const idle = () => {
    const row = el("div", { class: "suggest" });
    for (const term of ["mission query", "clearance", "refusal", "ten-fold", "placebo", "symmetry", "film friction", "Cayley"]) {
      const b = el("button", { type: "button" }, term);
      b.addEventListener("click", () => {
        input.value = term;
        input.focus();
        run();
      });
      row.append(b);
    }
    show([el("p", { class: "note" }, "Every section, table and identifier of the plan."), row]);
  };
  async function search(term, mine) {
    const quick = (await direct(term, 4)).map(jump);
    if (mine !== revision) return;
    show(quick.length ? quick : [el("p", { class: "note" }, "Searching…")]);
    try {
      const results = await find(term);
      const docs = await load(results, 0, 10);
      if (mine !== revision) return;
      const nodes = [...quick, ...docs.map(hit)];
      show(nodes.length ? nodes : [el("p", { class: "note" }, "Nothing matches. Try a shorter phrase.")]);
    } catch {
      if (mine === revision) show([...quick, failure(run)]);
    }
  }
  function run() {
    const term = input.value.trim();
    const mine = ++revision;
    clearTimeout(timer);
    all.href = term ? `/search/?q=${encodeURIComponent(term)}` : "/search/";
    if (!term) return idle();
    timer = setTimeout(() => search(term, mine), 130);
  }
  input.addEventListener("input", run);
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      choose(active + (e.key === "ArrowDown" ? 1 : -1));
    } else if (e.key === "Escape") {
      // A search field takes the first Escape for itself and only clears its text.
      e.preventDefault();
      dialog.close();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const target = options()[active];
      if (target) target.click();
      else if (input.value.trim()) location.assign(all.href);
    }
  });
  hits.addEventListener("click", (e) => {
    if (e.target.closest(".hit")) dialog.close();
  });
  close.addEventListener("click", () => dialog.close());
  dialog.addEventListener("click", (e) => {
    if (e.target === dialog) dialog.close();
  });
  idle();
  return { dialog, input };
}
function openPalette() {
  palette ??= buildPalette();
  if (palette.dialog.open) return;
  palette.dialog.showModal();
  palette.input.focus();
  palette.input.select();
  void jumps();
}
const query = $("#query");
for (const link of $$("[data-palette]"))
  link.addEventListener("click", (e) => {
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.button) return;
    e.preventDefault();
    if (query) query.focus();
    else openPalette();
  });
addEventListener("keydown", (e) => {
  const slash = e.key === "/" && !e.altKey && !e.metaKey && !e.ctrlKey && !e.shiftKey;
  const k = e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey;
  if (!slash && !k) return;
  if (slash && e.target.closest?.("input, textarea, select, [contenteditable]")) return;
  e.preventDefault();
  if (query) query.focus();
  else openPalette();
});

/* ---------- Search page ---------- */
if (query) {
  const results = $("#results");
  const status = $("#status");
  let revision = 0;
  let timer = 0;
  async function search(term, mine) {
    if (!term) return;
    try {
      const quick = (await direct(term, 3)).map(jump);
      const list = await find(term);
      if (mine !== revision) return;
      results.replaceChildren(...quick);
      let offset = 0;
      const more = async () => {
        const docs = await load(list, offset, offset + 12);
        if (mine !== revision) return;
        results.append(...docs.map(hit));
        offset += docs.length;
        status.textContent = list.length
          ? `${offset} of ${list.length} matching sections`
          : "No matching sections. Try a shorter phrase, or browse the library.";
        if (offset < list.length) {
          const button = el("button", { type: "button", class: "btn" }, "Show more");
          button.addEventListener("click", () => {
            button.remove();
            more().catch(() => fail(mine));
          });
          results.append(button);
        }
      };
      await more();
    } catch {
      fail(mine);
    }
  }
  function fail(mine) {
    if (mine !== revision) return;
    status.textContent = "";
    results.replaceChildren(failure(() => update(true)));
  }
  function update(now = false) {
    const term = query.value.trim();
    const mine = ++revision;
    clearTimeout(timer);
    if (!term) results.replaceChildren();
    status.textContent = term ? "Searching…" : "";
    if (now) void search(term, mine);
    else timer = setTimeout(() => search(term, mine), 160);
  }
  function remember() {
    const url = new URL(location.href);
    if (query.value.trim()) url.searchParams.set("q", query.value.trim());
    else url.searchParams.delete("q");
    history.replaceState(null, "", url);
  }
  query.addEventListener("input", () => {
    remember();
    update();
  });
  query.form.addEventListener("submit", (e) => {
    e.preventDefault();
    remember();
    update(true);
  });
  const restore = () => {
    query.value = new URL(location.href).searchParams.get("q") ?? "";
    update(true);
  };
  addEventListener("popstate", restore);
  restore();
}

/* ---------- Library ---------- */
const filter = $("[data-filter]");
if (filter) {
  filter.hidden = false;
  const input = $("input", filter);
  const output = $("output", filter);
  const cards = $$(".card").map((card) => ({ card, text: card.textContent.toLowerCase() }));
  input.addEventListener("input", () => {
    const words = input.value.toLowerCase().split(/\s+/).filter(Boolean);
    let shown = 0;
    for (const { card, text } of cards) {
      const on = words.every((w) => text.includes(w));
      card.hidden = !on;
      if (on) shown++;
    }
    for (const shelf of $$(".shelf")) shelf.hidden = !$(".card:not([hidden])", shelf);
    output.textContent = words.length ? `${shown} of ${cards.length}` : `${cards.length}`;
  });
}

/* ---------- Reader ---------- */
const article = $(".reading");
if (article) {
  for (const button of $$(".code .copy")) {
    button.hidden = false;
    button.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText($("code", button.parentElement).textContent);
        button.textContent = "Copied";
      } catch {
        button.textContent = "Copy failed";
      }
      setTimeout(() => (button.textContent = "Copy"), 1600);
    });
  }

  // Contents: the section being read, kept in view inside its rail.
  const rail = $(".rail.toc");
  const links = rail ? $$("a", rail) : [];
  const marks = links.map((a) => document.getElementById(decodeURIComponent(a.hash.slice(1))));
  let now = -1;
  let waiting = 0;
  const follow = () => {
    waiting = 0;
    let i = -1;
    for (let j = 0; j < marks.length; j++) {
      if (!marks[j] || marks[j].getBoundingClientRect().top > 150) break;
      i = j;
    }
    if (i === now) return;
    links[now]?.removeAttribute("aria-current");
    links[i]?.setAttribute("aria-current", "true");
    now = i;
    if (i >= 0) keep(rail, links[i]);
  };
  const keep = (box, link) => {
    const b = box.getBoundingClientRect();
    const l = link.getBoundingClientRect();
    if (l.top < b.top + 40 || l.bottom > b.bottom - 60) box.scrollTop += l.top - b.top - b.height * 0.35;
  };
  if (links.length) {
    addEventListener("scroll", () => (waiting ||= requestAnimationFrame(follow)), { passive: true });
    follow();
  }
  const part = $(".rail.part");
  const here = part && $("[aria-current]", part);
  if (here) keep(part, here);

  // Definitions: what an identifier means, without leaving the page.
  let cards;
  let card;
  let timer = 0;
  const hide = () => {
    clearTimeout(timer);
    card?.remove();
    card = undefined;
  };
  const show = async (link) => {
    cards ??= fetch(new URL("definitions.json", location.origin + location.pathname))
      .then((r) => (r.ok ? r.json() : {}))
      .catch(() => {
        cards = undefined;
        return {};
      });
    const found = (await cards)[link.dataset.id];
    if (!found || !(link.matches(":hover") || link === document.activeElement)) return;
    card?.remove();
    card = el("div", { class: "defn", role: "tooltip" }, el("b", {}, `${link.dataset.id} · ${found[0]}`), found[1]);
    document.body.append(card);
    const box = link.getBoundingClientRect();
    const left = Math.max(16, Math.min(box.left, innerWidth - card.offsetWidth - 16));
    const below = box.bottom + card.offsetHeight + 16 < innerHeight;
    card.style.left = `${left + scrollX}px`;
    card.style.top = `${(below ? box.bottom + 8 : box.top - card.offsetHeight - 8) + scrollY}px`;
  };
  article.addEventListener("pointerover", (e) => {
    if (e.pointerType === "touch") return;
    const link = e.target.closest?.("a.id");
    if (!link) return;
    clearTimeout(timer);
    timer = setTimeout(() => show(link), 160);
  });
  article.addEventListener("pointerout", (e) => {
    if (e.target.closest?.("a.id")) hide();
  });
  article.addEventListener("focusin", (e) => {
    if (e.target.matches?.("a.id:focus-visible")) show(e.target);
  });
  article.addEventListener("focusout", hide);
  addEventListener("keydown", (e) => {
    if (e.key === "Escape") hide();
  });
  addEventListener("scroll", hide, { passive: true });

  // Phones: the contents in a sheet above the dock.
  let sheet;
  for (const opener of $$("[data-sheet]"))
    opener.addEventListener("click", (e) => {
      e.preventDefault();
      if (!sheet) {
        const nav = el("nav", { "aria-label": "Contents" });
        for (const [title, from] of [
          ["On this page", ".inline-toc > div"],
          [$(".rail.part .micro")?.textContent ?? "", ".rail.part nav"],
        ]) {
          const source = $(from);
          if (!source) continue;
          nav.append(el("p", { class: "micro" }, title), ...$$("a", source).map((a) => a.cloneNode(true)));
        }
        const close = el("button", { type: "button", class: "btn sm" }, "Close");
        sheet = el("dialog", { class: "sheet", "aria-label": "Contents" }, el("header", {}, "Contents", close), nav);
        close.addEventListener("click", () => sheet.close());
        sheet.addEventListener("click", (event) => {
          if (event.target === sheet || event.target.closest("a")) sheet.close();
        });
        document.body.append(sheet);
      }
      sheet.showModal();
    });
}
