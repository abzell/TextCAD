#!/usr/bin/env python3
"""Corpus parts of milestone M8, part 3: six hydraulic components.

Their expected values (volume, area, center of mass, box) come from the independent build123d model
of each part: there is no short closed form for fillets, cross drills and threads' tap drills.
`spec/examples/rod_clevis.tcad` is the same text as `corpus/hyd_rod_clevis`.

Usage: B123_PYTHON=/path/to/python-with-build123d python3 corpus/tools/gen_hydraulic.py
       (then: tcad fmt corpus spec/examples)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from corpuslib import *  # noqa: F401,F403

SRC = os.path.join(os.path.dirname(__file__), "hydraulic")


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
