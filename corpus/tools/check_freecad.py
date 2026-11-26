"""Opens the STEP exports of the hydraulic parts in FreeCAD and checks them against expected.json.

Run with FreeCAD's command-line interpreter, never with plain python:

    STEP_DIR=out CORPUS_DIR=corpus freecadcmd corpus/tools/check_freecad.py

STEP_DIR holds <corpus name>.step for the hyd_* parts (CI creates them with `tcad export --to step`).
A part passes if FreeCAD reads it, finds exactly the expected solids, a valid shape, and a volume and
bounding box within 1e-5 relative / 1e-3 mm of expected.json (FreeCAD integrates less precisely than
the kernel tests do). The part number and revision of the part's `meta` statement must be the STEP
product id and version (FreeCAD labels objects by product name, so this is checked in the file text).
"""
import glob
import json
import os
import re
import sys

import FreeCAD  # noqa: F401  (provided by freecadcmd)
import Part

step_dir = os.environ["STEP_DIR"]
corpus = os.environ["CORPUS_DIR"]
failed = []
files = sorted(glob.glob(os.path.join(step_dir, "hyd_*.step")))
if len(files) < 6:
    failed.append(f"expected 6 hydraulic STEP files in {step_dir}, found {len(files)}")
for path in files:
    name = os.path.basename(path)[: -len(".step")]
    with open(os.path.join(corpus, name, "expected.json")) as f:
        exp = json.load(f)
    shape = Part.read(path)
    problems = []
    if not shape.isValid():
        problems.append("shape is not valid")
    if len(shape.Solids) != exp["topology"]["solids"]:
        problems.append(f"{len(shape.Solids)} solids, expected {exp['topology']['solids']}")
    if abs(shape.Volume - exp["volume_mm3"]) > 1e-5 * exp["volume_mm3"]:
        problems.append(f"volume {shape.Volume} vs {exp['volume_mm3']}")
    bb = shape.BoundBox
    for got, want in zip((bb.XMin, bb.YMin, bb.ZMin, bb.XMax, bb.YMax, bb.ZMax), exp["bbox_min_mm"] + exp["bbox_max_mm"]):
        if abs(got - want) > 1e-3:
            problems.append(f"bounding box {got} vs {want}")
            break
    source = glob.glob(os.path.join(corpus, name, "parts", "*.tcad"))[0]
    with open(source) as f:
        meta = re.search(r'^\s*meta number="([^"]*)" revision="([^"]*)"', f.read(), re.M)
    with open(path, errors="replace") as f:
        text = f.read()
    if meta:
        number, revision = meta.groups()
        if f"PRODUCT('{number}'" not in text:
            problems.append(f"product id {number} not in the STEP file")
        if not re.search(rf"PRODUCT_DEFINITION_FORMATION\w*\('{re.escape(revision)}'", text):
            problems.append(f"product version {revision} not in the STEP file")
    print(f"{name}: {'ok' if not problems else '; '.join(problems)}")
    failed += [f"{name}: {p}" for p in problems]
if failed:
    print("FreeCAD check failed:\n" + "\n".join(failed))
    sys.exit(1)
print(f"FreeCAD opened {len(files)} STEP files")
