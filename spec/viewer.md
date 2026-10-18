# Viewer, language server and editor integration

Status: normative for `tcad serve`; informative for the user interface (Phase 1).

## `tcad serve <path> [--port N]`

Starts an HTTP server on `127.0.0.1` (default port 8765; `0` picks a free port and the chosen
address is printed as `serving <path> at http://127.0.0.1:PORT/`). `<path>` is a bundle directory or
a `.tcad` file. Only `GET` is accepted. Responses are never cached.

| Route | Content |
|---|---|
| `/` | The viewer page. |
| `/viewer.js` | The viewer bundle (built from `viewer/`). |
| `/api/parts` | `{"parts": ["parts/a.tcad", ...]}`: paths relative to the bundle. |
| `/api/part?file=F&config=C` | `{ok, source, configs, build}` where `build` is the output of `tcad build --json`; on failure `{ok: false, error, diagnostics}`. |
| `/api/model.glb?file=F&config=C` | The result as GLB (spec/io.md); primitive extras carry `id`, `aliases` and `face`. |
| `/api/version` | `{"version": H}`; `H` changes when a source file is saved, added or removed. |

`F` must be one of the paths listed by `/api/parts`; anything else is 404. `C` is optional.

## Viewer behaviour

- The feature tree lists the features of the last successful build with their keyword.
- Selecting a feature (tree or statement) highlights its faces on the model and its statement in
  the source. Picking a face on the model selects the feature in whose namespace the face's
  persistent name lies (`body.cap_end` belongs to `body`), highlights the statement and shows the
  full name and its aliases.
- The page polls `/api/version` once a second and reloads source, tree and model after a change,
  keeping the camera and the selection when the feature still exists.
- A failing build leaves the previous model on screen and shows the error.

## In-browser mode (Phase 1b)

A page with `<body data-mode="browser">` (`viewer/web/index.html`, assembled with the WebAssembly
module by `tools/wasm/site.sh`) needs no server: the source pane is an editor, and every edit
(after 300 ms without typing) regenerates the part in a Web Worker running `crates/tcad-web`. The
configuration list comes from the build; a source that does not build keeps the last model on screen
and shows the errors with their codes and lines. The part shown first is `?part=URL`, by default the
rod clevis. Text and geometry stay linked both ways: the statement under the caret selects its
feature, and selecting a feature or picking a face selects its statement.

## Language server (`tcad-lsp`)

JSON-RPC over standard input and output with `Content-Length` framing. Capabilities: full-text
document sync; diagnostics (codes, related locations, UTF-16 positions); completion (keywords by
scope, parameters and declared names, units, functions, sketch entities and their points, and the
persistent names of the last build after `feature.`); hover (parameter value, feature keyword,
persistent name with geometry and size, keyword help); go to definition (parameters, features,
sketch entities); rename (see ADR 0013); formatting (canonical form); quick fixes from the fix hints
of diagnostics; document symbols.
