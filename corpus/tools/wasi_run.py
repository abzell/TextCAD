#!/usr/bin/env python3
"""Runs a wasm32-wasip1 test binary under wasmtime (Python bindings); used as a cargo runner.

    CARGO_TARGET_WASM32_WASIP1_RUNNER="python3 corpus/tools/wasi_run.py" \
        cargo test --target wasm32-wasip1 -p tcad-model --test determinism

The repository root is mapped to the same absolute path inside the guest, so paths baked in at
compile time (CARGO_MANIFEST_DIR) work unchanged. Needs `pip install wasmtime`.
"""
import os
import sys

from wasmtime import Engine, Linker, Module, Store, WasiConfig


def main() -> int:
    wasm, args = sys.argv[1], sys.argv[1:]
    root = os.path.realpath(os.path.join(os.path.dirname(__file__), "..", ".."))
    engine = Engine()
    linker = Linker(engine)
    linker.define_wasi()
    store = Store(engine)
    wasi = WasiConfig()
    wasi.argv = args
    wasi.env = [(k, v) for k, v in os.environ.items() if k.startswith(("RUST_", "UPDATE_"))]
    wasi.preopen_dir(root, root)
    wasi.inherit_stdout()
    wasi.inherit_stderr()
    store.set_wasi(wasi)
    module = Module.from_file(engine, wasm)
    instance = linker.instantiate(store, module)
    try:
        instance.exports(store)["_start"](store)
    except Exception as e:  # a WASI exit(code) surfaces as an exception with an exit status
        code = getattr(e, "code", None)
        if code is None and "exit status" in str(e):
            code = int(str(e).split("exit status")[-1].split()[0])
        if code == 0:
            return 0
        print(e, file=sys.stderr)
        return code if isinstance(code, int) and code else 1
    return 0


sys.exit(main())
