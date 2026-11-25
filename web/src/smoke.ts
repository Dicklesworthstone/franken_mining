/**
 * Drives a running site in Chrome: every figure mounts, the controls change what they should, search and the palette
 * work, nothing overflows on a phone, and every share card is an image of the size it claims.
 *
 *   bun run smoke                                  # the local preview
 *   bun run smoke https://frankenmining.com        # a deployment
 */
import assert from "node:assert/strict";
import { chromium, type Page } from "playwright-core";

const base = new URL(process.argv[2] ?? "http://localhost:8787").origin;
const browser = await chromium.launch({
  channel: "chrome",
  headless: true,
  args: ["--use-gl=angle", "--use-angle=metal", "--enable-gpu", "--ignore-gpu-blocklist"],
});
const errors: string[] = [];
const watch = (page: Page) => {
  page.on("pageerror", (e) => errors.push(`${page.url()}: ${e.message}`));
  page.on("response", (r) => {
    if (r.status() >= 400 && !r.url().endsWith("/no-such-page/")) errors.push(`${r.status()} ${r.url()}`);
  });
  return page;
};
const overflow = (page: Page) => page.evaluate(() => document.documentElement.scrollWidth - innerWidth);

try {
  const page = watch(await browser.newPage({ baseURL: base, viewport: { width: 1440, height: 900 } }));

  /* The hero: live, its tallies are numbers, and its labels follow the desk they are read from. */
  await page.goto("/");
  await page.locator("#fig-hero[data-live]").waitFor({ timeout: 20_000 });
  const loads = Number((await page.locator("#fig-hero [data-n=loads]").innerText()).replace(/\D/g, ""));
  assert(loads > 0, `The hero counts ${loads} loads`);
  await page.locator("#fig-hero [data-seg=lens] button", { hasText: "AI agent" }).click();
  await page.locator("#fig-hero .label3d", { hasText: "FMINE-REF-108" }).waitFor({ timeout: 5_000 });
  await page.locator("#fig-hero [data-seg=lens] button", { hasText: "CFO" }).click();
  await page.locator("#fig-hero .label3d", { hasText: "not a reserve" }).waitFor({ timeout: 5_000 });

  /* Every figure mounts when it comes on screen, without an error. */
  // Drawn by the server, with nothing to run.
  const still: string[] = [];
  const ids = (await page.locator("[data-fig]").evaluateAll((all) => all.map((e) => [e.id, (e as HTMLElement).dataset.fig!])))
    .filter(([, fig]) => !still.includes(fig!))
    .map(([id]) => id!);
  assert.equal(ids.length, 20);
  for (const id of ids) {
    await page.evaluate((x) => document.getElementById(x)!.scrollIntoView({ block: "center", behavior: "instant" }), id);
    await page.locator(`#${id}[data-ready]`).waitFor({ timeout: 20_000 }).catch(() => {
      throw Error(`${id} did not mount`);
    });
  }

  assert.equal(new URL(page.url()).pathname, "/");

  // Only the chrome is fixed to the window.
  const fixed = await page.evaluate(() =>
    [...document.querySelectorAll("main *")].filter((e) => getComputedStyle(e).position === "fixed").map((e) => e.className),
  );
  assert.deepEqual(fixed, []);

  // An output is labelable, so a slider inside the same label needs its own name.
  const unnamed = await page.evaluate(() =>
    [...document.querySelectorAll("input")].filter((e) => !e.getAttribute("aria-label") && !e.labels?.length).length,
  );
  assert.equal(unnamed, 0, "Unnamed controls");

  /* The palette, from the keyboard. */
  await page.keyboard.press("/");
  const palette = page.locator("dialog.palette[open]");
  await palette.waitFor();
  await page.keyboard.type("Touchstone");
  await palette.locator('[role="option"]').first().waitFor({ timeout: 20_000 });
  await page.keyboard.press("Escape");
  await palette.waitFor({ state: "hidden" });

  /* The library and the search page. */
  await page.goto("/plan/");
  assert((await page.locator('a[href^="/plan/"]').count()) >= 58);
  await page.goto("/search/?q=pseudoflow");
  const hit = page.locator("#results a").first();
  await hit.waitFor({ timeout: 20_000 });
  assert((await page.locator("#results mark").count()) > 0);
  await hit.click();
  await page.waitForURL((u) => /^\/plan\/.+/.test(u.pathname));
  const hash = decodeURIComponent(new URL(page.url()).hash.slice(1));
  if (hash) assert(await page.evaluate((id) => !!document.getElementById(id), hash), `No #${hash}`);
  await page.goto('/search/?q="jzqxvfjzqxvf"');
  await page.getByRole("status").filter({ hasText: /no (matching|sections)/i }).waitFor({ timeout: 20_000 });
  const missing = await page.goto("/no-such-page/");
  assert.equal(missing!.status(), 404);

  /* Headings step down one level at a time. */
  for (const path of ["/", "/plan/13/", "/plan/20/", "/plan/appendix-k/"]) {
    await page.goto(path);
    const skip = await page.evaluate(() => {
      let last = 0;
      for (const h of document.querySelectorAll("main h1, main h2, main h3, main h4, main h5, main h6")) {
        const n = Number(h.tagName[1]);
        if (last && n > last + 1) return `h${last} to h${n}: ${h.textContent}`;
        last = n;
      }
      return "";
    });
    assert.equal(skip, "", `${path} skips a heading level, ${skip}`);
  }

  /* Share cards. */
  for (const path of ["/", "/plan/", "/search/", "/plan/introduction/", "/plan/13/", "/plan/20/", "/plan/appendix-k/"]) {
    await page.goto(path);
    const meta = await page.evaluate(() =>
      Object.fromEntries(
        [...document.querySelectorAll<HTMLMetaElement>('meta[property^="og:"],meta[name^="twitter:"]')].map((m) => [
          m.getAttribute("property") ?? m.name,
          m.content,
        ]),
      ),
    );
    for (const key of ["og:title", "og:description", "og:image:alt", "twitter:title", "twitter:image:alt"])
      assert(meta[key], `${path} has no ${key}`);
    for (const [key, height] of [["og:image", 630], ["twitter:image", 600]] as const) {
      const at = new URL(meta[key]!);
      assert(at.searchParams.get("v"), `${path} ${key} has no content hash`);
      const image = await page.evaluate(async (src) => {
        const r = await fetch(src);
        const blob = await r.blob();
        const bitmap = await createImageBitmap(blob);
        return { type: r.headers.get("content-type"), bytes: blob.size, w: bitmap.width, h: bitmap.height };
      }, at.pathname + at.search);
      assert.equal(image.type, "image/jpeg");
      assert(image.bytes > 20_000, `${path} ${key} is ${image.bytes} bytes`);
      assert.deepEqual([image.w, image.h], [1200, height], `${path} ${key}`);
    }
  }

  /* Phones: no sideways scrolling anywhere, the menu opens, and the reader has its dock. */
  const phone = watch(
    await browser.newPage({ baseURL: base, viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }),
  );
  for (const path of ["/", "/plan/", "/plan/13/", "/plan/appendix-k/", "/search/"]) {
    await phone.goto(path);
    await phone.evaluate(() => document.fonts.ready);
    const end = await phone.evaluate(() => document.documentElement.scrollHeight);
    for (let y = 0; y < Math.min(end, 40_000); y += 2500) {
      await phone.evaluate((to) => scrollTo({ top: to, behavior: "instant" }), y);
      await phone.waitForTimeout(60);
    }
    assert((await overflow(phone)) <= 0, `${path} scrolls sideways by ${await overflow(phone)} px`);
    assert.equal(await phone.locator(".dock").isVisible(), path.startsWith("/plan/") && path !== "/plan/");
    await phone.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
    await phone.locator(".menu summary").click();
    assert(await phone.locator(".menu nav").isVisible());
  }

  /* Without JavaScript the text is all there. */
  const plain = await browser.newContext({ javaScriptEnabled: false, baseURL: base });
  const text = await plain.newPage();
  await text.goto("/");
  assert((await text.locator("main").innerText()).length > 8000);
  assert(await text.locator('img[src*="/media/lineup"]').first().isVisible());
  await text.goto("/plan/13/");
  assert((await text.locator("main").innerText()).length > 30_000);
  await plain.close();

  assert.deepEqual(errors, []);
  console.log(`Verified ${ids.length} figures, the palette, search, share cards, headings and phone layout at ${base}`);
} finally {
  await browser.close();
}
