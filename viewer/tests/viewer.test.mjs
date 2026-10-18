// End-to-end test of the viewer: starts `tcad serve` on a copy of the rod clevis, opens it in
// Chromium and checks the text <-> geometry links in both directions and re-render on save.
// Usage: TCAD=path/to/tcad node tests/viewer.test.mjs   (the binary must be built with `occt`)

import { spawn } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromium } from "playwright-core";
import assert from "node:assert/strict";

const tcad = process.env.TCAD ?? resolve("../target/debug/tcad");
const work = mkdtempSync(join(tmpdir(), "tcad-viewer-"));
cpSync(resolve("../corpus/hyd_rod_clevis"), work, { recursive: true });
const partFile = join(work, "parts", readdirSync(join(work, "parts")).find((f) => f.endsWith(".tcad")));

const server = spawn(tcad, ["serve", work, "--port", "0"], { stdio: ["ignore", "pipe", "inherit"] });
const url = await new Promise((ok, fail) => {
  let text = "";
  server.stdout.on("data", (d) => {
    text += d;
    const m = /(http:\/\/127\.0\.0\.1:\d+\/)/.exec(text);
    if (m) ok(m[1]);
  });
  server.on("exit", (c) => fail(new Error(`tcad serve exited with ${c}`)));
});

const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM,
  args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
});
let failed = false;
try {
  const page = await browser.newPage({ viewport: { width: 1400, height: 800 } });
  page.on("pageerror", (e) => console.error("page error:", e.message));
  await page.goto(url);
  await page.waitForFunction(() => window.__tcad?.ready && window.__tcad.faces().length > 0, null, { timeout: 60000 });

  // the model carries persistent names
  const ids = await page.evaluate(() => window.__tcad.faces().map((f) => f.id));
  assert.ok(ids.some((id) => id.startsWith("body.")), `face names: ${ids.slice(0, 5)}`);

  // geometry -> text: picking a face selects its feature and highlights the statement
  const clicked = await page.evaluate(() => {
    const c = document.getElementById("view").getBoundingClientRect();
    return { x: c.left + c.width / 2, y: c.top + c.height / 2, w: c.width, h: c.height };
  });
  let picked = null;
  for (const [fx, fy] of [[0, 0], [0.1, 0], [-0.1, 0], [0, 0.1], [0, -0.1], [0.2, 0.1], [-0.2, -0.1], [0.15, -0.15]]) {
    await page.mouse.click(clicked.x + fx * clicked.w, clicked.y + fy * clicked.h);
    picked = await page.evaluate(() => window.__tcad.pickedFace());
    if (picked) break;
  }
  assert.ok(picked, "a click on the model picks a face");
  const feature = picked.split(".")[0];
  assert.equal(await page.evaluate(() => window.__tcad.selected()), feature);
  const lines = await page.evaluate(() => window.__tcad.highlightedLines());
  assert.equal(lines.length, 1, "the statement of the picked face is highlighted");
  const src = readFileSync(partFile, "utf8").split("\n");
  assert.match(src[lines[0] - 1], new RegExp(`\\b${feature}\\b`));

  // text -> geometry: clicking a statement highlights the feature in the tree and on the model
  const target = await page.evaluate(() => window.__tcad.features().find((n) => n !== "body" && document.querySelector(`#source .line[data-feature="${n}"]`)));
  assert.ok(target, "a second feature exists");
  await page.click(`#source .line[data-feature="${target}"]`);
  assert.equal(await page.evaluate(() => window.__tcad.selected()), target);
  assert.ok(await page.evaluate((t) => document.querySelector(`#tree li[data-name="${t}"]`).classList.contains("sel"), target));
  const status = await page.textContent("#status");
  assert.match(status, new RegExp(target));

  // the tree selects too
  await page.click(`#tree li[data-name="body"]`);
  assert.equal(await page.evaluate(() => window.__tcad.selected()), "body");

  // re-render on save
  const before = await page.evaluate(() => document.getElementById("source").textContent);
  writeFileSync(partFile, readFileSync(partFile, "utf8").replace("param jaw_gap = 26 mm", "param jaw_gap = 28 mm"));
  await page.waitForFunction((b) => document.getElementById("source").textContent !== b, before, { timeout: 30000 });
  assert.ok((await page.textContent("#source")).includes("jaw_gap = 28 mm"));
  await page.waitForFunction(() => window.__tcad.faces().length > 0);
  console.log("viewer ok");
} catch (e) {
  failed = true;
  console.error(e);
} finally {
  await browser.close();
  server.kill();
}
process.exit(failed ? 1 : 0);
