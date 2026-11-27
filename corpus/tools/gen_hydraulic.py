#!/usr/bin/env python3
"""Corpus parts of milestone M8, part 3: six hydraulic components.

Their expected values (volume, area, center of mass, box) come from the independent build123d model
of each part: there is no short closed form for fillets, cross drills and threads' tap drills.
`spec/examples/rod_clevis.tcad` is the same text as `corpus/hyd_rod_clevis`.

Usage: B123_PYTHON=/path/to/python-with-build123d python3 corpus/tools/gen_hydraulic.py
       (then bring the files into canonical form, spec/canonical-form.md)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from corpuslib import *  # noqa: F401,F403

SRC = os.path.join(os.path.dirname(__file__), "hydraulic")
REFS = {}
_emit = emit


def emit(name, *args, **kw):  # noqa: F811  keeps each part's build123d model for its interface
    REFS[name] = args[6]
    _emit(name, *args, **kw)


def source(name):
    with open(os.path.join(SRC, name + ".tcad")) as f:
        return f.read()


def vertical_edges_at(x, y):
    return (
        f"[e for e in shape.edges() if e.geom_type == GeomType.LINE "
        f"and abs(e.start_point().X - ({x})) < 1e-6 and abs(e.end_point().X - ({x})) < 1e-6 "
        f"and abs(e.start_point().Y - ({y})) < 1e-6 and abs(e.end_point().Y - ({y})) < 1e-6]"
    )


# 1. rod clevis (also the example in spec/examples)
text = source("rod_clevis")
emit(
    "hyd_rod_clevis", "Rod clevis", None,
    "A clevis: a block with a fork slot, filleted gap root and nose corners, a 20 mm pin bore through both jaws "
    "with chamfered rims and an M24x2 tapped hole for the rod. Faces are found by name, tags and ordinals.",
    None, None, None,
    "body = Box(70, 58, 40, align=(Align.MIN, Align.CENTER, Align.CENTER))\n"
    "shape = body - Pos(30, 0, 0) * Box(45, 26, 100, align=(Align.MIN, Align.CENTER, Align.CENTER))\n"
    f"shape = fillet({vertical_edges_at(30, 13)} + {vertical_edges_at(30, -13)}, 4)\n"
    f"shape = fillet({vertical_edges_at(70, 29)} + {vertical_edges_at(70, -29)}, 4)\n"
    "shape = shape - Pos(50, 0, 0) * (Rot(90, 0, 0) * Cylinder(10, 100))\n"
    "shape = shape - Pos(14, 0, 0) * (Rot(0, 90, 0) * Cylinder(11, 28))\n"
    "rim = [e for e in shape.edges() if e.geom_type == GeomType.CIRCLE and abs(e.radius - 10) < 1e-6 "
    "and abs(abs(e.arc_center.Y) - 29) < 1e-6]\n"
    "shape = chamfer(rim, 1.5)\n",
    source=text,
)
with open(os.path.join(os.path.dirname(__file__), "..", "..", "spec", "examples", "rod_clevis.tcad"), "w") as f:
    f.write(text)

# 2. rod eye with a spherical bearing bore
emit(
    "hyd_rod_eye", "Rod eye", None,
    "A 50 mm eye, 20 mm wide, with a 32 mm bore for a spherical plain bearing (chamfered both sides) and an "
    "M12x1.25 tapped hole in the shank.",
    None, None, None,
    "shape = Rot(90, 0, 0) * Cylinder(25, 20)\n"
    "shape = shape + Pos(25, 0, 0) * (Rot(0, 90, 0) * Cylinder(9, 50))\n"
    "shape = shape - Rot(90, 0, 0) * Cylinder(16, 100)\n"
    "shape = shape - Pos(38, 0, 0) * (Rot(0, 90, 0) * Cylinder(10.75 / 2, 24))\n"
    "rim = [e for e in shape.edges() if e.geom_type == GeomType.CIRCLE and abs(e.radius - 16) < 1e-6]\n"
    "shape = chamfer(rim, 1.5)\n",
    source=source("rod_eye"),
)

# 3. cylinder end cap with an SAE O-ring port
drill = (0.875 - 1 / 14) * 25.4
emit(
    "hyd_end_cap", "Cylinder end cap", None,
    "A 120 mm cap with an 80 mm spigot, a 7/8-14 UNF O-ring port (tap drill, counterbore as the seal face), four "
    "tie-rod holes on a 100 mm circle and a chamfered rim.",
    None, None, None,
    "shape = Pos(0, 0, 12.5) * Cylinder(60, 25)\n"
    "shape = shape + Pos(0, 0, -3) * Cylinder(40, 6)\n"
    f"shape = shape - Pos(0, 0, 17) * Cylinder({drill / 2!r}, 16)\n"
    "shape = shape - Pos(0, 0, 24) * Cylinder(15, 2)\n"
    "for i in range(4):\n"
    "    shape = shape - Pos(50 * cos(i * pi / 2), 50 * sin(i * pi / 2), 12.5) * Cylinder(7, 60)\n"
    "rim = [e for e in shape.edges() if e.geom_type == GeomType.CIRCLE and abs(e.radius - 60) < 1e-6 "
    "and abs(e.arc_center.Z - 25) < 1e-6]\n"
    "shape = chamfer(rim, 2)\n",
    source=source("end_cap"),
)

# 4. tie-rod cylinder head
emit(
    "hyd_tie_rod_head", "Tie-rod cylinder head", None,
    "A 100 mm square head with rounded corners, a 50 mm rod bore with a seal groove, four tie-rod holes (one "
    "hole mirrored twice) and a side port into the bore.",
    None, None, None,
    "shape = Box(100, 100, 40, align=(Align.CENTER, Align.CENTER, Align.MIN))\n"
    "shape = fillet(shape.edges().filter_by(Axis.Z), 8)\n"
    "shape = shape - Pos(0, 0, 20) * Cylinder(25, 40)\n"
    "for sx in (36, -36):\n"
    "    for sy in (36, -36):\n"
    "        shape = shape - Pos(sx, sy, 20) * Cylinder(7, 40)\n"
    "ring = Pos(0, 0, 20) * (Cylinder(27.5, 6) - Cylinder(24.5, 6))\n"
    "shape = shape - ring\n"
    "shape = shape - Pos(25, 0, 20) * (Rot(0, 90, 0) * Cylinder(8.25, 50))\n",
    source=source("tie_rod_head"),
)

# 5. four-port manifold block
emit(
    "hyd_manifold_4port", "Four-port manifold block", None,
    "A 120 x 60 x 40 block with a 12 mm gallery drilled from one end, an 8 mm cross drill, four M18x1.5 ports "
    "(a linear pattern) that meet the gallery, and four mounting holes (a pattern of a pattern).",
    None, None, None,
    "shape = Box(120, 60, 40, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "shape = shape - Pos(55, 30, 20) * (Rot(0, 90, 0) * Cylinder(6, 110))\n"
    "shape = shape - Pos(60, 25, 20) * (Rot(90, 0, 0) * Cylinder(4, 50))\n"
    "for i in range(4):\n"
    "    shape = shape - Pos(20 + i * 80 / 3, 30, 27) * Cylinder(8.25, 26)\n"
    "for x in (8, 112):\n"
    "    for y in (8, 52):\n"
    "        shape = shape - Pos(x, y, 20) * Cylinder(4.5, 40)\n",
    source=source("manifold"),
)

# 6. pin with a grease groove and cross hole
emit(
    "hyd_grease_pin", "Pin with a grease groove", None,
    "A 40 mm pin with an axial grease hole, an annular groove and a cross hole that joins them, and chamfered ends.",
    None, None, None,
    "shape = Pos(0, 0, 60) * Cylinder(20, 120)\n"
    "shape = shape - Pos(0, 0, 30) * Cylinder(4, 60)\n"
    "shape = shape - Pos(0, 0, 60) * (Cylinder(21.5, 8) - Cylinder(18.5, 8))\n"
    "shape = shape - Pos(0, 11, 60) * (Rot(90, 0, 0) * Cylinder(3, 22))\n"
    "ends = [e for e in shape.edges() if e.geom_type == GeomType.CIRCLE and abs(e.radius - 20) < 1e-6 "
    "and (abs(e.arc_center.Z) < 1e-6 or abs(e.arc_center.Z - 120) < 1e-6)]\n"
    "shape = chamfer(ends, 2)\n",
    source=source("grease_pin"),
)


# --- interfaces: connectors and published faces (spec/assembly.md, draft) -------------------------
import json  # noqa: E402
import subprocess  # noqa: E402

from framelib import (  # noqa: E402
    IDENTITY, REF_FRAMES, REF_PUBLISHED, check_frames, check_published, clean, clean_entity, frame,
    ref_tail,
)


def cyl(axis, point, r, span):
    return {"type": "cylinder", "axis": axis, "axis_point_mm": point, "radius_mm": r, "span_mm": span}


def interface(name, frames_code, connectors, published=None, configs=None):
    """Adds the part's connectors and published entities to its expected.json, after checking them
    against the frames and entities its build123d model derives from its own geometry."""
    code = REF_HEAD + REF_FRAMES + REF_PUBLISHED + REFS[name] + "\n" + ref_tail(frames_code)
    out = subprocess.run([PY, "-c", code], capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"{name}: build123d failed:\n{out.stderr}")
    got = json.loads(out.stdout.strip().splitlines()[-1])
    check_frames(name, {k: v for k, v in connectors.items() if k != "origin"}, got["connectors"])
    if published:
        check_published(name, published, got["published"])
    d = os.path.join(ROOT, name)
    with open(os.path.join(d, "expected.json")) as f:
        expected = json.load(f)
    expected["connectors"] = {k: clean(v) for k, v in connectors.items()}
    if published:
        expected["published"] = {k: [clean_entity(e) for e in v] for k, v in published.items()}
    if configs:
        expected["configs"] = {
            c: {
                "connectors": {k: clean(v) for k, v in spec["connectors"].items()},
                "published": {k: [clean_entity(e) for e in v] for k, v in spec.get("published", {}).items()},
                "info": spec.get("info", []),
            }
            for c, spec in configs.items()
        }
    write(os.path.join(d, "expected.json"), json.dumps(expected, indent=2) + "\n")
    write(os.path.join(d, "reference.py"), code)
    print("interface", name)


PIN = "\n".join([
    "_cyl = sorted([f for f in shape.faces() if f.geom_type == GeomType.CYLINDER",
    "               and abs(BRepAdaptor_Surface(f.wrapped).Cylinder().Radius() - {r}) < 1e-9], key=lambda f: {key})",
    "",
])

# 1. rod clevis: the pin joint at the middle of the fork, the rod thread, the pin surface
clevis_conn = {
    "pin": frame([50, 0, 0], [0, 1, 0]),
    "rod": frame([0, 0, 0], [-1, 0, 0]),
    "origin": IDENTITY,
}
interface(
    "hyd_rod_clevis",
    PIN.format(r=10, key="f.center(CenterOf.MASS).Y")
    + "connectors['pin'] = _frame(*cylinder_axis_all(shape, 10, [50, 0, 0], [0, 1, 0], 'middle'))\n"
    "_o, _left = face_frame(planar_face(shape, [-1, 0, 0], (0, 0, 0)))\n"
    "connectors['rod'] = _frame(circle_centre(shape, 11, (0, 0, 0)), _left)\n"
    "published['pin_surface'] = [pub_face(f) for f in _cyl]\n",
    clevis_conn,
    {"pin_surface": [cyl([0, 1, 0], [50, 0, 0], 10, [-27.5, -13]), cyl([0, 1, 0], [50, 0, 0], 10, [13, 27.5])]},
    configs={"heavy": {
        "connectors": clevis_conn,
        "published": {"pin_surface": [cyl([0, 1, 0], [50, 0, 0], 12.5, [-31.5, -13]),
                                      cyl([0, 1, 0], [50, 0, 0], 12.5, [13, 31.5])]},
    }},
)

# 2. rod eye: the bearing at the eye's centre, the rod thread at the shank's end
interface(
    "hyd_rod_eye",
    PIN.format(r=16, key="f.center(CenterOf.MASS).Y")
    + "connectors['bearing'] = _frame(*cylinder_axis_all(shape, 16, [0, 0, 0], [0, 1, 0], 'middle'))\n"
    "_o, _end = face_frame(planar_face(shape, [1, 0, 0], (50, 0, 0)))\n"
    "connectors['rod'] = _frame(circle_centre(shape, 10.75 / 2, (50, 0, 0)), _end)\n"
    "published['bearing_bore'] = [pub_face(f) for f in _cyl]\n",
    {"bearing": frame([0, 0, 0], [0, 1, 0]), "rod": frame([50, 0, 0], [1, 0, 0]), "origin": IDENTITY},
    {"bearing_bore": [cyl([0, 1, 0], [0, 0, 0], 16, [-8.5, 8.5])]},
)

# 3. end cap: the port, the four tie rods, the spigot that seats in the barrel
interface(
    "hyd_end_cap",
    "_o, _top = face_frame(planar_face(shape, [0, 0, 1], (0, 0, 25)))\n"
    "connectors['port_face'] = _frame(circle_centre(shape, 15, (0, 0, 25)), _top)\n"
    "for i in range(4):\n"
    "    connectors[f'tie_rod.i{i + 1}'] = _frame(circle_centre(shape, 7, (50 * cos(i * pi / 2), 50 * sin(i * pi / 2), 25)), _top)\n"
    "_o, _n = face_frame(planar_face(shape, [0, 0, -1], (0, 0, -6)))\n"
    "connectors['pilot_face'] = _frame(_o, _n)\n"
    + PIN.format(r=40, key="f.center(CenterOf.MASS).Z")
    + "published['spigot'] = [pub_face(f) for f in _cyl]\n",
    {"port_face": frame([0, 0, 25], [0, 0, 1]),
     "tie_rod.i1": frame([50, 0, 25], [0, 0, 1]),
     "tie_rod.i2": frame([0, 50, 25], [0, 0, 1]),
     "tie_rod.i3": frame([-50, 0, 25], [0, 0, 1]),
     "tie_rod.i4": frame([0, -50, 25], [0, 0, 1]),
     "pilot_face": frame([0, 0, -6], [0, 0, -1]),
     "origin": IDENTITY},
    {"spigot": [cyl([0, 0, 1], [0, 0, 0], 40, [-6, 0])]},
)

# 4. tie-rod head: the rod axis through the split bore, the tie rods (a mirror of a mirror), the port
interface(
    "hyd_tie_rod_head",
    "connectors['rod_axis'] = _frame(*cylinder_axis_all(shape, 25, [0, 0, 0], [0, 0, 1], 'middle'))\n"
    "_o, _top = face_frame(planar_face(shape, [0, 0, 1], (0, 0, 40)))\n"
    "for key, x, y in (('tie_rod.m1.m1', 36, 36), ('tie_rod.m1.m2', -36, 36), ('tie_rod.m2.m1', 36, -36), ('tie_rod.m2.m2', -36, -36)):\n"
    "    connectors[key] = _frame(circle_centre(shape, 7, (x, y, 40)), _top)\n"
    "_o, _side = face_frame(planar_face(shape, [1, 0, 0], (50, 0, 20)))\n"
    "connectors['port_face'] = _frame(circle_centre(shape, 8.25, (50, 0, 20)), _side)\n"
    "_o, _n = face_frame(planar_face(shape, [0, 0, -1], (0, 0, 0)))\n"
    "connectors['seat'] = _frame(_o, _n)\n",
    {"rod_axis": frame([0, 0, 20], [0, 0, 1]),
     "tie_rod.m1.m1": frame([36, 36, 40], [0, 0, 1]),
     "tie_rod.m1.m2": frame([-36, 36, 40], [0, 0, 1]),
     "tie_rod.m2.m1": frame([36, -36, 40], [0, 0, 1]),
     "tie_rod.m2.m2": frame([-36, -36, 40], [0, 0, 1]),
     "port_face": frame([50, 0, 20], [1, 0, 0]),
     "seat": frame([0, 0, 0], [0, 0, -1]),
     "origin": IDENTITY},
)

# 5. manifold: four ports (a pattern), the gallery and cross-drill plugs, four mounting bolts
manifold = {
    f"port_face.i{i + 1}": frame([20 + i * 80 / 3, 30, 40], [0, 0, 1]) for i in range(4)
}
manifold.update({
    "gallery_plug": frame([0, 30, 20], [-1, 0, 0]),
    "cross_plug": frame([60, 0, 20], [0, -1, 0]),
    "mount_bolt.i1.i1": frame([8, 8, 40], [0, 0, 1]),
    "mount_bolt.i1.i2": frame([112, 8, 40], [0, 0, 1]),
    "mount_bolt.i2.i1": frame([8, 52, 40], [0, 0, 1]),
    "mount_bolt.i2.i2": frame([112, 52, 40], [0, 0, 1]),
    "base": frame([60, 30, 0], [0, 0, -1]),
    "origin": IDENTITY,
})
interface(
    "hyd_manifold_4port",
    "_o, _top = face_frame(planar_face(shape, [0, 0, 1], (60, 30, 40)))\n"
    "for i in range(4):\n"
    "    connectors[f'port_face.i{i + 1}'] = _frame(circle_centre(shape, 8.25, (20 + i * 80 / 3, 30, 40)), _top)\n"
    "for key, x, y in (('mount_bolt.i1.i1', 8, 8), ('mount_bolt.i1.i2', 112, 8), ('mount_bolt.i2.i1', 8, 52), ('mount_bolt.i2.i2', 112, 52)):\n"
    "    connectors[key] = _frame(circle_centre(shape, 4.5, (x, y, 40)), _top)\n"
    "_o, _left = face_frame(planar_face(shape, [-1, 0, 0], (0, 30, 20)))\n"
    "connectors['gallery_plug'] = _frame(circle_centre(shape, 6, (0, 30, 20)), _left)\n"
    "_o, _front = face_frame(planar_face(shape, [0, -1, 0], (60, 0, 20)))\n"
    "connectors['cross_plug'] = _frame(circle_centre(shape, 4, (60, 0, 20)), _front)\n"
    "_o, _n = face_frame(planar_face(shape, [0, 0, -1], (60, 30, 0)))\n"
    "connectors['base'] = _frame(_o, _n)\n",
    manifold,
)

# 6. grease pin: the pin axis across the groove, the grease nipple, the cross hole, both ends
interface(
    "hyd_grease_pin",
    "connectors['pin_axis'] = _frame(*cylinder_axis_all(shape, 20, [0, 0, 0], [0, 0, 1], 'middle'))\n"
    "_o, _n = face_frame(planar_face(shape, [0, 0, -1], (0, 0, 0)))\n"
    "connectors['grease'] = _frame(circle_centre(shape, 4, (0, 0, 0)), _n)\n"
    "connectors['end_a'] = _frame(_o, _n)\n"
    "_o, _n = face_frame(planar_face(shape, [0, 0, 1], (0, 0, 120)))\n"
    "connectors['end_b'] = _frame(_o, _n)\n"
    "_p, _ax = cylinder_axis(shape, 3, (0, 11, 60), 'start')\n"
    "connectors['cross_in'] = _frame(_p, [-c for c in _ax])\n"
    + PIN.format(r=20, key="f.center(CenterOf.MASS).Z")
    + "published['bearing_surface'] = [pub_face(f) for f in _cyl]\n",
    {"pin_axis": frame([0, 0, 60], [0, 0, 1]),
     "grease": frame([0, 0, 0], [0, 0, -1]),
     "cross_in": frame([0, 0, 60], [0, -1, 0]),
     "end_a": frame([0, 0, 0], [0, 0, -1]),
     "end_b": frame([0, 0, 120], [0, 0, 1]),
     "origin": IDENTITY},
    {"bearing_surface": [cyl([0, 0, 1], [0, 0, 0], 20, [2, 56]), cyl([0, 0, 1], [0, 0, 0], 20, [64, 118])]},
)
