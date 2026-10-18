// End-to-end test of the in-browser mode (Phase 1b, docs/adr/0014): serves a site assembled by
// tools/wasm/site.sh, opens it in Chromium, waits for the WebAssembly build of the example part,
// then edits the text and checks that the model, the tree and the links follow.
//
//   node tests/browser.test.mjs SITE_DIR       (CHROMIUM=/path/chrome optional)
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, resolve } from "node:path";
import { chromium } from "playwright-core";
import assert from "node:assert/strict";

const site = resolve(process.argv[2] ?? "site");
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm", ".tcad": "text/plain" };
const server = createServer(async (req, res) => {
  const path = join(site, decodeURIComponent(new URL(req.url, "http://x").pathname).replace(/^\/$/, "/index.html"));
  try {
    const body = await readFile(path);
    res.writeHead(200, { "content-type": types[extname(path)] ?? "application/octet-stream" });
    res.end(body);
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const url = `http://127.0.0.1:${server.address().port}/`;

const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM,
  args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
});
let failed = false;
try {
  const page = await browser.newPage({ viewport: { width: 1400, height: 800 } });
  page.on("pageerror", (e) => console.error("page error:", e.message));
  page.on("console", (m) => m.type() === "error" && console.error("console:", m.text()));
  await page.goto(url);
  const t0 = Date.now();
  await page.waitForFunction(() => window.__tcad?.ready && window.__tcad.faces().length > 0, null, { timeout: 300000 });
  const first = await page.evaluate(() => window.__tcad.lastResult());
  assert.equal(first.ok, true);
  assert.match(first.kernel, /^occt-/);
  console.log(`first build in the browser: ${Date.now() - t0} ms, ${first.result.faces} faces, ${first.result.volume_mm3.toFixed(3)} mm^3`);
  assert.ok((await page.evaluate(() => window.__tcad.faces())).some((f) => f.id.startsWith("body.")));

  // text -> geometry and geometry -> text
  await page.evaluate(() => window.__tcad.selectFeature("body"));
  const [line] = await page.evaluate(() => window.__tcad.highlightedLines());
  const text = await page.inputValue("#editor");
  assert.match(text.split("\n")[line - 1], /\bbody\b/);

  // an edit regenerates in the browser
  const n = await page.evaluate(() => window.__tcad.regenerations());
  await page.evaluate((t) => window.__tcad.setSource(t), text.replace("param jaw_gap = 26 mm", "param jaw_gap = 30 mm"));
  await page.waitForFunction((k) => window.__tcad.regenerations() > k, n, { timeout: 120000 });
  const second = await page.evaluate(() => window.__tcad.lastResult());
  assert.equal(second.ok, true);
  assert.ok(second.result.volume_mm3 < first.result.volume_mm3, "a wider gap removes material");

  // a broken edit keeps the last model and names the problem
  const m = await page.evaluate(() => window.__tcad.regenerations());
  await page.evaluate((t) => window.__tcad.setSource(t), text.replace("d=pin_dia", "d=pin_diameter"));
  await page.waitForFunction((k) => window.__tcad.regenerations() > k, m, { timeout: 120000 });
  assert.match(await page.textContent("#banner"), /E\d{4} line \d+/);
  assert.ok((await page.evaluate(() => window.__tcad.faces())).length > 0, "the last model stays");
  console.log("in-browser viewer ok");
} catch (e) {
  failed = true;
  console.error(e);
} finally {
  await browser.close();
  server.close();
}
process.exit(failed ? 1 : 0);
