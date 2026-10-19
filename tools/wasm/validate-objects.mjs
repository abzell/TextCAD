// Prints every WebAssembly object file under a directory that the engine rejects, with the reason.
//
//   node tools/wasm/validate-objects.mjs DIR
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else if (name.endsWith(".o")) yield p;
  }
}

let checked = 0;
for (const path of walk(process.argv[2])) {
  checked++;
  try {
    await WebAssembly.compile(readFileSync(path));
  } catch (e) {
    console.log(`${path.split("/").pop()} ${e.message.replace(/^WebAssembly\.compile\(\): /, "")}`);
  }
}
console.error(`${checked} object files checked`);
