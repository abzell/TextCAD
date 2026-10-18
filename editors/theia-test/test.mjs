// End-to-end check that the TextCAD extension and its language server work in Eclipse Theia:
// starts this minimal Theia app with the extension from ../vscode as a plugin, opens a part with
// an error in Chromium and waits for the language server's diagnostic in the editor.
// Usage: TCAD_LSP=/path/tcad-lsp [CHROMIUM=/path/chrome] node test.mjs
import { spawn } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const here = dirname(fileURLToPath(import.meta.url));
const ext = resolve(here, "..", "vscode");
const lsp = process.env.TCAD_LSP;
if (!lsp) {
  console.error("set TCAD_LSP to the tcad-lsp executable");
  process.exit(2);
}
const work = mkdtempSync(join(tmpdir(), "tcad-theia-"));
const ws = join(work, "ws");
cpSync(join(ext, "test", "fixture"), ws, { recursive: true });
mkdirSync(join(ws, ".vscode"), { recursive: true });
writeFileSync(join(ws, ".vscode", "settings.json"), JSON.stringify({ "textcad.languageServerPath": resolve(lsp) }));
const plugins = join(work, "plugins", "textcad");
mkdirSync(plugins, { recursive: true });
for (const f of ["package.json", "language-configuration.json", "dist", "syntaxes"]) {
  cpSync(join(ext, f), join(plugins, f), { recursive: true });
}

const server = spawn(
  process.execPath,
  [join(here, "src-gen", "backend", "main.js"), ws, "--hostname", "127.0.0.1", "--port", "0",
    `--plugins=local-dir:${join(work, "plugins")}`],
  { cwd: here, stdio: ["ignore", "pipe", "pipe"] },
);
let log = "";
const url = await new Promise((ok, fail) => {
  const timer = setTimeout(() => fail(new Error(`Theia did not start:\n${log}`)), 120000);
  const on = (d) => {
    log += d;
    const m = /listening on (http:\/\/[0-9.]+:\d+)/i.exec(log);
    if (m) {
      clearTimeout(timer);
      ok(m[1].replace(/\/?$/, "/"));
    }
  };
  server.stdout.on("data", on);
  server.stderr.on("data", on);
  server.on("exit", (c) => fail(new Error(`Theia exited with ${c}:\n${log}`)));
});

const browser = await chromium.launch({ executablePath: process.env.CHROMIUM });
let failed = false;
try {
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
  await page.goto(url);
  await page.waitForSelector("#theia-app-shell", { timeout: 120000 });
  // the workspace is the test's own temporary copy: trust it so the extension may run
  const trust = page.getByRole("button", { name: /Yes, I trust/ });
  await trust.waitFor({ timeout: 60000 });
  await trust.click();
  // open the broken part with quick open
  await page.waitForTimeout(2000);
  await page.keyboard.press("Control+P");
  const input = page.locator(".quick-input-widget input");
  await input.waitFor({ timeout: 60000 });
  await input.fill("broken.tcad");
  await page.locator(".quick-input-list .monaco-list-row", { hasText: "broken.tcad" }).first().waitFor({ timeout: 60000 });
  await page.keyboard.press("Enter");
  await page.waitForSelector(".monaco-editor .view-lines", { timeout: 60000 });
  // the language server's error shows as a squiggle
  await page.waitForSelector(".monaco-editor .squiggly-error", { timeout: 120000 });
  // hovering the squiggle shows the diagnostic with its stable code
  await page.locator(".monaco-editor .view-line", { hasText: "nosuch" }).getByText("nosuch").hover();
  const hover = page.locator(".monaco-hover").filter({ hasText: /E\d{4}/ }).first();
  await hover.waitFor({ timeout: 60000 });
  console.log("Theia language server check passed:", (await hover.textContent())?.trim().slice(0, 160));
} catch (e) {
  failed = true;
  await browser.contexts()[0]?.pages()[0]?.screenshot({ path: join(work, "failure.png") }).catch(() => {});
  console.error("screenshot:", join(work, "failure.png"));
  console.error(e);
  console.error(log.slice(-4000));
} finally {
  await browser.close();
  server.kill();
}
process.exit(failed ? 1 : 0);
