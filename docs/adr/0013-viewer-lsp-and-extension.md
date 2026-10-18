# 0013. Viewer, language server and editor extension

Status: accepted (M8)

## Decision
- **The language server is a library plus a thin binary.** `tcad-lsp` has no file-system access in
  its core (`Server<P: Provider>`); the persistent names of the last build come through the
  `Provider` trait, which the binary implements by reading the bundle's cache. The core therefore
  builds for WebAssembly (the `wasm` CI job builds it) and can run in the browser.
- **Rename is token based and verified.** Only the declared name, its uses as an expression,
  `sketch.entity` paths and `config` keys change; argument names (`d=`), fields after a dot and
  names inside strings do not. The renamed text is re-analyzed and the edit is refused if it adds
  errors.
- **The viewer is served by the CLI.** `tcad serve` is a loopback HTTP server inside `tcad-cli`. It
  builds through the same commands as the command line (shared cache), exports GLB with the
  persistent name of every face in the primitive extras, and exposes a version hash that the page
  polls to re-render after a save. It serves only the `.tcad` files of the bundle it was started on.
- **three.js is bundled, not loaded from a CDN.** `viewer/` (TypeScript) is bundled with esbuild into
  `crates/tcad-cli/assets/viewer.js`, which is checked in and embedded into the binary, so the
  viewer works offline and the Rust build needs no Node. CI rebuilds the bundle and fails if it is
  stale, and runs a Chromium test (Playwright) of both highlight directions and of re-render on save.
- **One extension for VS Code and Theia.** `editors/vscode` uses only the VS Code extension API
  (language client, webview), which Theia implements. The viewer opens as an iframe of the local
  server, mapped with `asExternalUri` so remote workspaces work.

## Consequences
The viewer needs a `tcad` built with the OpenCASCADE kernel. In-browser regeneration is Phase 1b. A
change to `viewer/src` needs `npm run build` and the regenerated bundle committed.
