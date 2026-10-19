// Compiles a WebAssembly module and, when the engine rejects a function, names it (from the module's
// name section; link with --profiling-funcs to keep one).
//
//   node tools/wasm/validate.mjs path/to/module.wasm
import { readFileSync } from "node:fs";

const [path] = process.argv.slice(2);
const bytes = readFileSync(path);

function leb(buf, pos) {
  let result = 0;
  let shift = 0;
  for (;;) {
    const b = buf[pos++];
    result += (b & 0x7f) * 2 ** shift;
    shift += 7;
    if (!(b & 0x80)) return [result, pos];
  }
}

function functionNames(buf) {
  const names = new Map();
  let pos = 8;
  while (pos < buf.length) {
    const id = buf[pos++];
    let size;
    [size, pos] = leb(buf, pos);
    const end = pos + size;
    if (id === 0) {
      let [len, p] = leb(buf, pos);
      const name = buf.toString("utf8", p, p + len);
      p += len;
      if (name === "name") {
        while (p < end) {
          const sub = buf[p++];
          let subSize;
          [subSize, p] = leb(buf, p);
          if (sub === 1) {
            let count;
            let q = p;
            [count, q] = leb(buf, q);
            for (let i = 0; i < count; i++) {
              let idx, n;
              [idx, q] = leb(buf, q);
              [n, q] = leb(buf, q);
              names.set(idx, buf.toString("utf8", q, q + n));
              q += n;
            }
          }
          p += subSize;
        }
      }
    }
    pos = end;
  }
  return names;
}

try {
  await WebAssembly.compile(bytes);
  console.log(`${path}: valid`);
} catch (e) {
  console.log(`${path}: ${e.message}`);
  const m = /function #(\d+)/.exec(e.message);
  if (m) {
    const names = functionNames(bytes);
    console.log(`function #${m[1]} is ${names.get(Number(m[1])) ?? "(no name section)"}`);
  }
  process.exit(1);
}
