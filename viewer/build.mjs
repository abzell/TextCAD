// Bundles the viewer into the file `tcad serve` embeds: ../crates/tcad-cli/assets/viewer.js
import { build } from "esbuild";

await build({
  entryPoints: ["src/viewer.ts"],
  bundle: true,
  minify: true,
  format: "iife",
  target: "es2022",
  legalComments: "none",
  outfile: "../crates/tcad-cli/assets/viewer.js",
});
