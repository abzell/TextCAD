"""Shared helpers of the corpus generators: write parts, run the independent build123d model."""
import json
import math
import os
import subprocess
import sys

PI = math.pi
ROOT = os.path.join(os.path.dirname(__file__), "..")
PY = os.environ.get("B123_PYTHON", "/tmp/b123/bin/python")


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)


def part_source(name, title, body):
    return f'tcad 0.1\n\npart {name} "{title}" {{\n{body}}}\n'


REF_HEAD = (
    '"""Independent build123d model of the same part (CI only; never part of the product)."""\n'
    "import json\n"
    "from math import cos, sin, pi, tan, radians\n"
    "from build123d import *\n\n"
)
REF_TAIL = (
    "from OCP.BRepGProp import BRepGProp\n"
    "from OCP.GProp import GProp_GProps\n"
    "# integrate to a relative precision of 1e-12 (the default quadrature is only good to about 1e-6)\n"
    "_v, _a = GProp_GProps(), GProp_GProps()\n"
    "BRepGProp.VolumeProperties_s(shape.wrapped, _v, 1e-12, False)\n"
    "BRepGProp.SurfaceProperties_s(shape.wrapped, _a, 1e-12)\n"
    "bb = shape.bounding_box()\n"
    "c = _v.CentreOfMass()\n"
    "print(json.dumps({\n"
    '    "volume_mm3": _v.Mass(), "area_mm2": _a.Mass(),\n'
    '    "center_of_mass_mm": [c.X(), c.Y(), c.Z()],\n'
    '    "bbox_min_mm": [bb.min.X, bb.min.Y, bb.min.Z], "bbox_max_mm": [bb.max.X, bb.max.Y, bb.max.Z],\n'
    '    "topology": {"solids": len(shape.solids()), "faces": len(shape.faces()),\n'
    '                 "edges": len(shape.edges()), "vertices": len(shape.vertices())},\n'
    "}))\n"
)


def run_reference(code):
    out = subprocess.run([PY, "-c", REF_HEAD + code + "\n" + REF_TAIL], capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"build123d failed:\n{out.stderr}")
    return json.loads(out.stdout.strip().splitlines()[-1])


def emit(name, title, body, note, volume, lo, hi, ref, sweeps=None, solids=1, closed_volume=None, source=None):
    """Writes a corpus part. With `volume` None the values come from the independent build123d model
    (listed under "baseline"), otherwise volume and box are closed-form and checked against it."""
    got = run_reference(ref)
    if volume is not None:
        if abs(got["volume_mm3"] - volume) > 1e-6 * max(1.0, volume):
            sys.exit(f"{name}: closed-form volume {volume} disagrees with build123d {got['volume_mm3']}")
        for a, b in zip(got["bbox_min_mm"] + got["bbox_max_mm"], list(lo) + list(hi)):
            if abs(a - b) > 1e-5:
                sys.exit(f"{name}: closed-form bounding box disagrees with build123d")
        expected = {
            "note": note,
            "volume_mm3": volume,
            "area_mm2": got["area_mm2"],
            "center_of_mass_mm": got["center_of_mass_mm"],
            "bbox_min_mm": list(lo),
            "bbox_max_mm": list(hi),
            "topology": {"solids": solids},
            "baseline": ["area_mm2", "center_of_mass_mm"],
        }
    else:
        if closed_volume is not None and abs(got["volume_mm3"] - closed_volume) > 1e-6 * closed_volume:
            sys.exit(f"{name}: closed-form volume {closed_volume} disagrees with build123d {got['volume_mm3']}")
        expected = {
            "note": note,
            "volume_mm3": got["volume_mm3"],
            "area_mm2": got["area_mm2"],
            "center_of_mass_mm": got["center_of_mass_mm"],
            "bbox_min_mm": got["bbox_min_mm"],
            "bbox_max_mm": got["bbox_max_mm"],
            "topology": {"solids": solids},
            "baseline": ["volume_mm3", "area_mm2", "center_of_mass_mm", "bbox_min_mm", "bbox_max_mm"],
        }
    if sweeps:
        expected["sweeps"] = sweeps
    d = os.path.join(ROOT, name)
    write(os.path.join(d, "bundle.toml"), f'format = "0.1"\nname = "{name}"\nentry = "parts/{name}.tcad"\nunits = "mm"\nlicense = "Apache-2.0"\n')
    write(os.path.join(d, "parts", f"{name}.tcad"), source if source is not None else part_source(name, title, body))
    write(os.path.join(d, "expected.json"), json.dumps(expected, indent=2) + "\n")
    write(os.path.join(d, "reference.py"), REF_HEAD + ref + "\n" + REF_TAIL)
    print("wrote", name)


