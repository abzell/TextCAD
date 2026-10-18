#!/usr/bin/env python3
"""Cross-checks each corpus part's expected.json against its independent build123d model.

For every corpus/<name>/reference.py: run it, and compare the printed properties with
expected.json at determinism tier T2 (requirements 8.2): volume and area to 1e-6 relative,
center of mass and bounding box to 1e-5 mm (10 x the default modeling tolerance of 1e-6 mm).

Usage: python3 corpus/tools/compare_reference.py [corpus_dir]
Needs `pip install build123d`. Run in CI only; build123d is never part of the product.
"""
import json
import os
import subprocess
import sys

REL = 1e-6
ABS = 1e-5


def close_rel(a, b):
    return abs(a - b) <= REL * max(1.0, abs(b))


def close_abs(a, b):
    return abs(a - b) <= ABS


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..")
    failures = 0
    checked = 0
    for name in sorted(os.listdir(root)):
        ref = os.path.join(root, name, "reference.py")
        exp_path = os.path.join(root, name, "expected.json")
        if not (os.path.isfile(ref) and os.path.isfile(exp_path)):
            continue
        with open(exp_path) as f:
            exp = json.load(f)
        out = subprocess.run([sys.executable, ref], capture_output=True, text=True)
        if out.returncode != 0:
            print(f"FAIL {name}: reference.py failed\n{out.stderr}")
            failures += 1
            continue
        got = json.loads(out.stdout.strip().splitlines()[-1])
        problems = []
        for key in ("volume_mm3", "area_mm2"):
            if not close_rel(got[key], exp[key]):
                problems.append(f"{key}: build123d {got[key]!r} vs expected {exp[key]!r}")
        for key in ("center_of_mass_mm", "bbox_min_mm", "bbox_max_mm"):
            for a, b in zip(got[key], exp[key]):
                if not close_abs(a, b):
                    problems.append(f"{key}: build123d {got[key]} vs expected {exp[key]}")
                    break
        if exp.get("topology") and got["topology"]["solids"] != exp["topology"]["solids"]:
            problems.append(f"solids: build123d {got['topology']['solids']} vs {exp['topology']['solids']}")
        checked += 1
        if problems:
            failures += 1
            print(f"FAIL {name}:\n  " + "\n  ".join(problems))
        else:
            print(f"ok   {name}")
    print(f"{checked} part(s) cross-checked, {failures} failure(s)")
    return 1 if failures or checked == 0 else 0


if __name__ == "__main__":
    sys.exit(main())
