// A guard for the OpenCASCADE WebAssembly build: Emscripten 4.0.9 emitted invalid code
// ("br_table: label arity inconsistent") for ShapeCustom_BSplineRestriction and
// ShapeUpgrade_ShapeDivide while OCC_CONVERT_SIGNALS was defined (build-occt.sh now undefines it).
// This validates every
// object of a CMake build and recompiles each invalid one: at a lower optimisation level, else
// without one of its options (reporting which), else with only its includes, definitions and our
// flags. None of these changes the arithmetic (IEEE, -ffp-contract=off).
//
//   node tools/wasm/repair-objects.mjs BUILD_DIR    (BUILD_DIR/compile_commands.json is required)
import { execSync } from "node:child_process";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join as joinPath, resolve } from "node:path";

const build = resolve(process.argv[2]);
const commands = JSON.parse(readFileSync(joinPath(build, "compile_commands.json"), "utf8"));
const byOutput = new Map(commands.map((c) => [resolve(c.directory, c.output), c]));

function* objects(dir) {
  for (const name of readdirSync(dir)) {
    const p = joinPath(dir, name);
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

// Options that take the next token as their argument.
const WITH_ARG = new Set(["-o", "-c", "-MF", "-MT", "-MQ", "-isystem", "-include", "-x", "-Xclang", "-iquote"]);

// The command as a list of options (an option and its argument stay together).
function options(command) {
  const tokens = command.trim().split(/\s+/);
  const out = [];
  for (let i = 1; i < tokens.length; i++) {
    if (WITH_ARG.has(tokens[i]) && i + 1 < tokens.length) out.push([tokens[i], tokens[++i]]);
    else out.push([tokens[i]]);
  }
  return [tokens[0], out];
}

const join = (compiler, opts) => [compiler, ...opts.flat()].join(" ");
// Never removed: inputs/outputs, includes and definitions, and the flags every object must share
// (exception ABI, position independence, floating-point model).
const REQUIRED = ["-o", "-c", "-fwasm-exceptions", "-fPIC", "-ffp-contract=off"];
const essential = (o) => REQUIRED.includes(o[0]) || /^-[ID]/.test(o[0]);

async function compiles(cmd, obj, cwd) {
  try {
    execSync(cmd, { cwd, stdio: ["ignore", "ignore", "pipe"] });
  } catch {
    return false;
  }
  return valid(obj);
}

let checked = 0;
let repaired = 0;
for (const obj of objects(build)) {
  checked++;
  if (await valid(obj)) continue;
  const entry = byOutput.get(obj);
  if (!entry) throw new Error(`invalid ${obj} and no compile command for it`);
  const short = obj.slice(build.length + 1);
  console.log(`invalid: ${short}\n  command: ${entry.command}`);
  const [compiler, opts] = options(entry.command);
  let fix = null;
  // 1. a lower optimisation level
  for (const level of ["-O2", "-O1", "-O0"]) {
    const o = opts.filter((x) => !/^-O[0-3sz]$/.test(x[0])).concat([[level]]);
    if (await compiles(join(compiler, o), obj, entry.directory)) {
      fix = `at ${level}`;
      break;
    }
  }
  // 2. without one of the command's options (names the option that causes it)
  for (let i = 0; !fix && i < opts.length; i++) {
    if (essential(opts[i])) continue;
    const o = opts.filter((_, j) => j !== i);
    if (await compiles(join(compiler, o), obj, entry.directory)) fix = `without ${opts[i].join(" ")}`;
  }
  // 3. only the includes, definitions and our flags (the form that compiles correctly standalone)
  if (!fix) {
    const o = opts.filter(essential).concat([["-O2"]]);
    if (await compiles(join(compiler, o), obj, entry.directory)) fix = "with only includes, definitions and our flags";
  }
  if (!fix) throw new Error(`${short} is invalid with every variant tried`);
  console.log(`  repaired ${fix}`);
  repaired++;
}
console.log(`${checked} objects checked, ${repaired} recompiled`);
