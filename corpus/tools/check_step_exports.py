#!/usr/bin/env python3
"""Reads every corpus part's STEP export with an independent reader and compares it with expected.json.

For each corpus/<name>/ with an expected.json: `tcad export <dir> --to step`, then read the file with
build123d (`import_step`, a separate OpenCASCADE-based reader in Python) and compare volume and area
(1e-6 relative) and the bounding box (1e-5 mm) with the expected values: determinism tier T2.

Usage: python3 corpus/tools/check_step_exports.py <path-to-tcad-binary> [corpus_dir]
Needs `pip install build123d`. Run in CI only.
"""
import json
import os
import subprocess
import sys
import tempfile

from build123d import import_step
from OCP.BRepGProp import BRepGProp
from OCP.GProp import GProp_GProps


def precise(shape):
    """Volume and area integrated to a relative precision of 1e-12 (the default is only good to ~1e-6)."""
    v, a = GProp_GProps(), GProp_GProps()
    BRepGProp.VolumeProperties_s(shape.wrapped, v, 1e-12, False)
    BRepGProp.SurfaceProperties_s(shape.wrapped, a, 1e-12)
    return v.Mass(), a.Mass()


def main():
    tcad = sys.argv[1]
    root = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(__file__), "..")
    failures = checked = 0
    with tempfile.TemporaryDirectory() as tmp:
        for name in sorted(os.listdir(root)):
            exp_path = os.path.join(root, name, "expected.json")
            if not os.path.isfile(exp_path) or not os.path.isfile(os.path.join(root, name, "bundle.toml")):
                continue
            with open(exp_path) as f:
                exp = json.load(f)
            if "volume_mm3" not in exp:
                continue
            out = os.path.join(tmp, name + ".step")
            run = subprocess.run([tcad, "export", os.path.join(root, name), "--to", "step", "--out", out],
                                 capture_output=True, text=True)
            if run.returncode != 0:
                print(f"FAIL {name}: tcad export failed\n{run.stderr}")
                failures += 1
                continue
            shape = import_step(out)
            bb = shape.bounding_box()
            problems = []
            volume, area = precise(shape)
            if abs(volume - exp["volume_mm3"]) > 1e-6 * max(1.0, exp["volume_mm3"]):
                problems.append(f"volume {volume} vs {exp['volume_mm3']}")
            if abs(area - exp["area_mm2"]) > 1e-6 * max(1.0, exp["area_mm2"]):
                problems.append(f"area {area} vs {exp['area_mm2']}")
            for got, want in zip([bb.min.X, bb.min.Y, bb.min.Z, bb.max.X, bb.max.Y, bb.max.Z],
                                 exp["bbox_min_mm"] + exp["bbox_max_mm"]):
                if abs(got - want) > 1e-5:
                    problems.append("bounding box differs")
                    break
            checked += 1
            if problems:
                failures += 1
                print(f"FAIL {name}: " + "; ".join(problems))
            else:
                print(f"ok   {name}")
    print(f"{checked} STEP export(s) read back, {failures} failure(s)")
    sys.exit(1 if failures or checked == 0 else 0)


if __name__ == "__main__":
    main()
