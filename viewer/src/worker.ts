// Runs the WebAssembly build (crates/tcad-web, docs/adr/0014) off the main thread. The page posts
// `{id, src, config}`; the worker answers `{id, json, glb}` with the build's JSON and the GLB.

declare function importScripts(...urls: string[]): void;
declare const createTcad: (options: object) => Promise<TcadModule>;

interface TcadModule {
  HEAPU8: Uint8Array;
  _malloc(n: number): number;
  _free(p: number): void;
  _tcad_build(src: number, srcLen: number, config: number, configLen: number): number;
  _tcad_free(p: number): void;
  _tcad_glb_ptr(): number;
  _tcad_glb_len(): number;
  UTF8ToString(p: number): string;
}

importScripts("tcad-web.js");
const ready = createTcad({});

function bytes(M: TcadModule, text: string): [number, number] {
  const data = new TextEncoder().encode(text);
  const p = M._malloc(Math.max(1, data.length));
  M.HEAPU8.set(data, p);
  return [p, data.length];
}

self.onmessage = async (e: MessageEvent<{ id: number; src: string; config: string }>) => {
  const M = await ready;
  const { id, src, config } = e.data;
  const [ps, ns] = bytes(M, src);
  const [pc, nc] = bytes(M, config);
  const out = M._tcad_build(ps, ns, pc, nc);
  const json = JSON.parse(M.UTF8ToString(out));
  M._tcad_free(out);
  M._free(ps);
  M._free(pc);
  const n = M._tcad_glb_len();
  const glb = json.ok && n > 0 ? M.HEAPU8.slice(M._tcad_glb_ptr(), M._tcad_glb_ptr() + n).buffer : null;
  (self as unknown as Worker).postMessage({ id, json, glb }, glb ? [glb] : []);
};
