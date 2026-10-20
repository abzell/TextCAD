// Runs the extension inside a real VS Code (downloaded by @vscode/test-electron) against
// test/fixture, with the language server from TCAD_LSP. Usage: TCAD_LSP=/path/tcad-lsp npm test
import { runTests } from "@vscode/test-electron";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const fixture = join(root, "test", "fixture");
const lsp = process.env.TCAD_LSP;
if (!lsp) {
  console.error("set TCAD_LSP to the tcad-lsp executable");
  process.exit(2);
}
mkdirSync(join(fixture, ".vscode"), { recursive: true });
writeFileSync(
  join(fixture, ".vscode", "settings.json"),
  JSON.stringify({ "textcad.languageServerPath": resolve(lsp) }, null, 2),
);
try {
  await runTests({
    extensionDevelopmentPath: root,
    extensionTestsPath: join(root, "test", "suite", "index.js"),
    launchArgs: [fixture, "--disable-extensions", "--disable-workspace-trust"],
  });
} catch (e) {
  console.error(e);
  process.exit(1);
}
