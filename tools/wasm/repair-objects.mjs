// Emscripten 4.0.9 emits invalid code for a few OpenCASCADE functions in the Release build
// ("br_table: label arity inconsistent"; ShapeCustom_BSplineRestriction and ShapeUpgrade_ShapeDivide
// as of 8.0.1). This validates every object of a CMake build and recompiles each invalid one with
// its own compile command at the next lower optimisation level until the engine accepts it. Lower
// levels change speed only: the code is IEEE arithmetic with -ffp-contract=off either way.
//
//   node tools/wasm/repair-objects.mjs BUILD_DIR    (BUILD_DIR/compile_commands.json is required)
import { execSync } from "node:child_process";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const build = resolve(process.argv[2]);
const commands = JSON.parse(readFileSync(join(build, "compile_commands.json"), "utf8"));
const byOutput = new Map(commands.map((c) => [resolve(c.directory, c.output), c]));

function* objects(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* objects(p);
    else if (name.endsWith(".o")) yield p;
  }
}

async function valid(path) {
  try {
    await WebAssembly.compile(readFileSync(path));
    return true;
  } catch {
    return false;
  }
}

let checked = 0;
let repaired = 0;
for (const obj of objects(build)) {
  checked++;
  if (await valid(obj)) continue;
  const entry = byOutput.get(obj);
  if (!entry) throw new Error(`invalid ${obj} and no compile command for it`);
  let fixed = false;
  for (const level of ["-O2", "-O1", "-O0"]) {
    const cmd = entry.command.replace(/(^|\s)-O[0-3sz](?=\s|$)/g, "$1") + ` ${level}`;
    execSync(cmd, { cwd: entry.directory, stdio: "inherit" });
    if (await valid(obj)) {
      console.log(`repaired ${obj.slice(build.length + 1)} at ${level}`);
      fixed = true;
      break;
    }
  }
  if (!fixed) throw new Error(`${obj} is invalid at every optimisation level`);
  repaired++;
}
console.log(`${checked} objects checked, ${repaired} recompiled`);
