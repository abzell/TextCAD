// The Phase 1b gate (docs/adr/0014): every measured corpus part regenerates in the browser module
// and its canonical B-rep has the SHA-256 listed in corpus/brep-t1.txt (written by the native
// OpenCASCADE builds of the same version). Runs under Node; the same module runs in the browser.
//
//   node tools/wasm/gate.mjs path/to/tcad-web.js [corpus]
//
// occt.wasm must sit next to tcad-web.js. BREP_T1_DUMP=DIR keeps the canonical B-reps, to compare
// with the native builds line by line (tools/brep-diff.py).
import { readFileSync, readdirSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";

const [modulePath, corpusArg] = process.argv.slice(2);
if (!modulePath) {
  console.error("usage: node tools/wasm/gate.mjs path/to/tcad-web.js [corpus]");
  process.exit(2);
}
const corpus = resolve(corpusArg ?? "corpus");
const dump = process.env.BREP_T1_DUMP;
if (dump) mkdirSync(dump, { recursive: true });
const createTcad = createRequire(import.meta.url)(resolve(modulePath));
const M = await createTcad();

function build(src, config = "") {
  const enc = new TextEncoder();
  const s = enc.encode(src);
  const c = enc.encode(config);
  const ps = M._malloc(s.length || 1);
  const pc = M._malloc(c.length || 1);
  M.HEAPU8.set(s, ps);
  M.HEAPU8.set(c, pc);
  const out = M._tcad_build(ps, s.length, pc, c.length);
  const json = JSON.parse(M.UTF8ToString(out));
  M._tcad_free(out);
  M._free(ps);
  M._free(pc);
  return json;
}

const golden = readFileSync(join(corpus, "brep-t1.txt"), "utf8").trim().split("\n");
const kernel = golden[0].replace(/^# /, "");
const want = new Map(golden.slice(1).map((l) => l.split(" ")));
let failed = 0;
let checked = 0;
const started = Date.now();
for (const name of [...want.keys()].sort()) {
  const file = join(corpus, name, "parts", `${name}.tcad`);
  if (!existsSync(file)) {
    console.log(`MISSING ${name}`);
    failed++;
    continue;
  }
  const t0 = Date.now();
  const r = build(readFileSync(file, "utf8"));
  const ms = Date.now() - t0;
  if (dump && r.ok) {
    const brep = M.HEAPU8.slice(M._tcad_brep_ptr(), M._tcad_brep_ptr() + M._tcad_brep_len());
    writeFileSync(join(dump, `${name}.brep`), brep);
  }
  if (!r.ok) {
    console.log(`FAIL ${name}: ${JSON.stringify(r.diagnostics).slice(0, 300)}`);
    failed++;
  } else if (r.kernel !== kernel) {
    console.log(`FAIL ${name}: kernel ${r.kernel}, brep-t1.txt is for ${kernel}`);
    failed++;
  } else if (r.brep_sha256 !== want.get(name)) {
    console.log(`DIFF ${name}: ${r.brep_sha256} (browser) vs ${want.get(name)} (native)`);
    failed++;
  } else {
    console.log(`ok   ${name} (${ms} ms, ${r.result.faces} faces, ${r.result.volume_mm3.toFixed(6)} mm^3)`);
  }
  checked++;
}
const parts = readdirSync(corpus).filter((d) => existsSync(join(corpus, d, "expected.json"))).length;
console.log(`${checked} parts regenerated in WebAssembly (${parts} corpus entries) in ${((Date.now() - started) / 1000).toFixed(1)} s; ${failed} differ from ${kernel}`);
process.exit(failed ? 1 : 0);
