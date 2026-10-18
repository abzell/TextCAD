// Language-server checks inside VS Code: diagnostics, hover, completion, formatting, definition.
const assert = require("node:assert/strict");
const path = require("node:path");
const vscode = require("vscode");

async function waitFor(what, fn, ms = 60000) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 250));
  }
}

async function open(name) {
  const ws = vscode.workspace.workspaceFolders[0].uri.fsPath;
  const doc = await vscode.workspace.openTextDocument(path.join(ws, name));
  await vscode.window.showTextDocument(doc);
  assert.equal(doc.languageId, "textcad");
  return doc;
}

exports.run = async function run() {
  // diagnostics with stable codes
  const broken = await open("broken.tcad");
  const diags = await waitFor("diagnostics", () => {
    const d = vscode.languages.getDiagnostics(broken.uri);
    return d.length ? d : null;
  });
  const codes = diags.map((d) => String(typeof d.code === "object" ? d.code.value : d.code));
  assert.ok(codes.some((c) => /^E\d{4}$/.test(c)), `codes: ${codes}`);
  assert.ok(diags.some((d) => d.range.start.line === 4), "error on the line of the bad reference");

  // hover shows the parameter's value
  const good = await open("good.tcad");
  const use = good.positionAt(good.getText().indexOf("w + 1"));
  const hovers = await waitFor("hover", async () => {
    const h = await vscode.commands.executeCommand("vscode.executeHoverProvider", good.uri, use);
    return h && h.length ? h : null;
  });
  const text = hovers
    .flatMap((h) => h.contents)
    .map((c) => (typeof c === "string" ? c : c.value))
    .join("\n");
  assert.match(text, /w/);
  assert.match(text, /10/);

  // go to definition lands on the declaration
  const defs = await vscode.commands.executeCommand("vscode.executeDefinitionProvider", good.uri, use);
  assert.ok(defs && defs.length, "a definition");
  const target = defs[0].range || defs[0].targetRange;
  assert.equal(target.start.line, 3);

  // completion offers parameters
  const end = good.positionAt(good.getText().indexOf("w + 1") + 4);
  const list = await vscode.commands.executeCommand("vscode.executeCompletionItemProvider", good.uri, end);
  const labels = list.items.map((i) => (typeof i.label === "string" ? i.label : i.label.label));
  assert.ok(labels.includes("w"), `labels: ${labels.slice(0, 20)}`);

  // formatting produces the canonical form
  const messy = await open("messy.tcad");
  const edits = await vscode.commands.executeCommand("vscode.executeFormatDocumentProvider", messy.uri, {
    tabSize: 2,
    insertSpaces: true,
  });
  assert.ok(edits && edits.length, "formatting edits");
  // VS Code turns the server's whole-document edit into minimal edits: apply them and compare
  const we = new vscode.WorkspaceEdit();
  we.set(messy.uri, edits);
  assert.ok(await vscode.workspace.applyEdit(we), "edits apply");
  assert.ok(messy.getText().includes("  param w = 10 mm\n"), messy.getText());
  console.log("VS Code language server checks passed");
};
