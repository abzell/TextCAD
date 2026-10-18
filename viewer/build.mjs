// Bundles the viewer into the file `tcad serve` embeds (../crates/tcad-cli/assets/viewer.js) and the
// worker of the in-browser mode (dist/worker.js, docs/adr/0014).
import { build } from "esbuild";

const common = { bundle: true, minify: true, format: "iife", target: "es2022", legalComments: "none" };
await build({ ...common, entryPoints: ["src/viewer.ts"], outfile: "../crates/tcad-cli/assets/viewer.js" });
await build({ ...common, entryPoints: ["src/worker.ts"], outfile: "dist/worker.js" });
