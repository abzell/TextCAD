// TextCAD for VS Code and Eclipse Theia: the language server plus the viewer in a webview.

import * as vscode from "vscode";
import { spawn, ChildProcess } from "node:child_process";
import { LanguageClient, LanguageClientOptions, ServerOptions } from "vscode-languageclient/node";

let client: LanguageClient | undefined;
let viewerServer: ChildProcess | undefined;
let viewerPanel: vscode.WebviewPanel | undefined;

function startClient(): void {
  const command = vscode.workspace.getConfiguration("textcad").get<string>("languageServerPath", "tcad-lsp");
  const server: ServerOptions = { command, args: [] };
  const options: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "textcad" }],
  };
  client = new LanguageClient("textcad", "TextCAD Language Server", server, options);
  void client.start().catch((e) => {
    void vscode.window.showWarningMessage(`TextCAD: cannot start ${command}: ${e}`);
  });
}

/** Starts `tcad serve` on the workspace (once) and resolves to its URL. */
function startViewerServer(folder: string): Promise<string> {
  const cli = vscode.workspace.getConfiguration("textcad").get<string>("cliPath", "tcad");
  return new Promise((resolve, reject) => {
    const child = spawn(cli, ["serve", folder, "--port", "0"], { stdio: ["ignore", "pipe", "pipe"] });
    viewerServer = child;
    let text = "";
    let errors = "";
    child.stdout.on("data", (d: Buffer) => {
      text += d.toString();
      const m = /(http:\/\/127\.0\.0\.1:\d+\/)/.exec(text);
      if (m) resolve(m[1]);
    });
    child.stderr.on("data", (d: Buffer) => (errors += d.toString()));
    child.on("error", (e) => reject(new Error(`cannot run ${cli}: ${e.message}`)));
    child.on("exit", (code) => {
      viewerServer = undefined;
      reject(new Error(errors.trim() || `${cli} serve exited with ${code}`));
    });
  });
}

async function openViewer(): Promise<void> {
  const folder =
    vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ??
    (vscode.window.activeTextEditor?.document.uri.fsPath
      ? vscode.window.activeTextEditor.document.uri.fsPath
      : undefined);
  if (!folder) {
    void vscode.window.showWarningMessage("TextCAD: open a folder or a .tcad file first.");
    return;
  }
  if (viewerPanel) {
    viewerPanel.reveal();
    return;
  }
  let url: string;
  try {
    url = await startViewerServer(folder);
  } catch (e) {
    void vscode.window.showErrorMessage(`TextCAD viewer: ${(e as Error).message}`);
    return;
  }
  // The page is served from loopback; map it through the host so remote workspaces work too.
  const external = (await vscode.env.asExternalUri(vscode.Uri.parse(url))).toString();
  const panel = vscode.window.createWebviewPanel("textcadViewer", "TextCAD", vscode.ViewColumn.Beside, {
    enableScripts: true,
    retainContextWhenHidden: true,
  });
  panel.webview.html = `<!doctype html><html><body style="margin:0;height:100vh">
<iframe src="${external}" style="border:0;width:100%;height:100%" allow="clipboard-read; clipboard-write"></iframe>
</body></html>`;
  panel.onDidDispose(() => {
    viewerPanel = undefined;
    viewerServer?.kill();
  });
  viewerPanel = panel;
}

export function activate(context: vscode.ExtensionContext): void {
  startClient();
  context.subscriptions.push(
    vscode.commands.registerCommand("textcad.openViewer", openViewer),
    vscode.commands.registerCommand("textcad.restartServer", async () => {
      await client?.stop();
      startClient();
    }),
  );
}

export async function deactivate(): Promise<void> {
  viewerServer?.kill();
  await client?.stop();
}
