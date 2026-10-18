#!/usr/bin/env python3
"""Corpus parts of milestone M8, part 1: extrude end conditions, variable fillet, guided and ruled
lofts, sweep along an arc and boolean operations.

Usage: B123_PYTHON=/path/to/python-with-build123d python3 corpus/tools/gen_more_features.py
       (then: tcad fmt corpus)
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from corpuslib import *  # noqa: F401,F403

# 1. every extrude end condition
w2 = 10 - 2 * 10 * math.tan(math.radians(5))
frustum = 10 / 3 * (100 + w2 * w2 + 10 * w2)
emit(
    "feat_extrude_ends", "Plate with one boss for every extrude end condition",
    "  sketch base on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=60 mm h=40 mm\n  }\n"
    "  extrude plate from base.r distance=10 mm\n"
    "  sketch boss1 on plane.XY {\n    circle c center=(10 mm, 10 mm) r=5 mm\n  }\n"
    "  extrude centered from boss1.c distance=30 mm symmetric\n"
    "  sketch boss2 on plane.XY {\n    circle c center=(30 mm, 10 mm) r=4 mm\n  }\n"
    "  extrude two_sided from boss2.c distance=20 mm distance2=5 mm\n"
    "  plane stop from=plane.XY offset=25 mm\n"
    "  sketch boss3 on plane.XY {\n    circle c center=(50 mm, 10 mm) r=3 mm\n  }\n"
    "  extrude up_to from boss3.c to=stop\n"
    "  sketch bore on plane.XY {\n    circle c center=(10 mm, 30 mm) r=3 mm\n  }\n"
    "  extrude clear from bore.c through_all op=subtract\n"
    "  sketch tower on plate.cap_end {\n    rect r corner=(25 mm, 22 mm) w=10 mm h=10 mm\n  }\n"
    "  extrude tapered from tower.r distance=10 mm draft=5 deg\n",
    "Blind, symmetric, two-sided, to a plane, through all (subtract) and drafted, the last on a sketch on a face.",
    60 * 40 * 10 + 500 * PI + 240 * PI + 135 * PI - 90 * PI + frustum, [0, 0, -15], [60, 40, 25],
    "shape = Box(60, 40, 10, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "shape = shape + Pos(10, 10, 0) * Cylinder(5, 30)\n"
    "shape = shape + Pos(30, 10, 7.5) * Cylinder(4, 25)\n"
    "shape = shape + Pos(50, 10, 12.5) * Cylinder(3, 25)\n"
    "shape = shape - Pos(10, 30, 5) * Cylinder(3, 10)\n"
    "shape = shape + Pos(30, 27, 10) * extrude(Rectangle(10, 10), amount=10, taper=5)\n",
)

# 2. variable-radius fillet (the independent model drives OpenCASCADE directly: build123d has none)
emit(
    "feat_fillet_variable", "Block with a variable-radius fillet",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=50 mm h=30 mm\n  }\n"
    "  extrude boss from s.r distance=12 mm\n"
    "  fillet taper edges=[edge(boss.cap_end & boss.side(s.r.bottom))] r=(2 mm, 5 mm)\n",
    "One edge rounded with a radius growing from 2 mm at its start to 5 mm at its end.",
    None, None, None,
    "from OCP.BRepFilletAPI import BRepFilletAPI_MakeFillet\n"
    "box = Box(50, 30, 12, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "edge = [e for e in box.edges() if abs(e.center().Y) < 1e-6 and abs(e.center().Z - 12) < 1e-6][0]\n"
    "mk = BRepFilletAPI_MakeFillet(box.wrapped)\n"
    "first = edge.start_point()\n"
    "mk.Add(2.0, 5.0, edge.wrapped) if first.X < 1e-6 else mk.Add(5.0, 2.0, edge.wrapped)\n"
    "mk.Build()\n"
    "shape = Solid(mk.Shape())\n",
)

# 3. guided loft: two circles along a quarter-circle guide (closed form: integral of pi r(s)^2 ds)
L = PI / 2 * 50
emit(
    "feat_loft_guided", "Tapered tube along a curved guide",
    "  plane stop from=plane.XZ offset=-50 mm\n"
    "  sketch start on plane.YZ {\n    circle c center=(0 mm, 0 mm) r=10 mm\n  }\n"
    "  sketch end on stop {\n    circle c center=(50 mm, 0 mm) r=5 mm\n  }\n"
    "  sketch spine on plane.XY {\n    arc a center=(0 mm, 50 mm) start=(0 mm, 0 mm) end=(50 mm, 50 mm)\n  }\n"
    "  loft tube sections=[start.c, end.c] guides=[spine.a]\n",
    "A radius shrinking from 10 mm to 5 mm over a quarter circle of radius 50: volume = pi L (r0^2 + r0 r1 + r1^2) / 3.",
    None, None, None,
    "arc = CenterArc((0, 50), 50, -90, 90)\n"
    "c1 = Plane.YZ * Circle(10)\n"
    "c2 = Pos(50, 50, 0) * (Plane.XZ * Circle(5))\n"
    "shape = sweep(sections=[c1, c2], path=arc, multisection=True)\n",
    closed_volume=PI * L * (100 + 50 + 25) / 3,
)

# 4. ruled loft: a square frustum
emit(
    "feat_loft_ruled", "Ruled loft between two squares",
    "  plane upper from=plane.XY offset=30 mm\n"
    "  sketch bottom on plane.XY {\n    rect r center=(0 mm, 0 mm) w=40 mm h=40 mm\n  }\n"
    "  sketch top on upper {\n    rect r center=(0 mm, 0 mm) w=20 mm h=20 mm\n  }\n"
    "  loft frustum sections=[bottom.r, top.r] ruled\n",
    "Straight-sided loft: a square frustum, volume h/3 (A1 + A2 + sqrt(A1 A2)).",
    30 / 3 * (1600 + 400 + 800), [-20, -20, 0], [20, 20, 30],
    "bottom = Rectangle(40, 40)\n"
    "top = Pos(0, 0, 30) * Rectangle(20, 20)\n"
    "shape = loft([bottom, top], ruled=True)\n",
)

# 5. sweep along an arc
emit(
    "feat_sweep_arc", "Pipe bend",
    "  sketch profile_sk on plane.YZ {\n    circle c center=(0 mm, 0 mm) r=6 mm\n  }\n"
    "  sketch path on plane.XY {\n    arc a center=(0 mm, 40 mm) start=(0 mm, 0 mm) end=(40 mm, 40 mm)\n  }\n"
    "  sweep elbow from profile_sk.c path=path.a\n",
    "A circle of radius 6 swept along a quarter circle of radius 40: volume pi r^2 * arc length (Pappus).",
    PI * 36 * (PI / 2 * 40), [0, -6, -6], [46, 40, 6],
    "shape = sweep(Plane.YZ * Circle(6), path=CenterArc((0, 40), 40, -90, 90))\n",
)

# 6. union, subtract and intersect
emit(
    "feat_boolean_ops", "Union, subtract and intersect",
    "  sketch a on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=40 mm h=30 mm\n  }\n"
    "  extrude one from a.r distance=20 mm\n"
    "  plane lift from=plane.XY offset=5 mm\n"
    "  sketch b on lift {\n    rect r corner=(20 mm, 10 mm) w=40 mm h=30 mm\n  }\n"
    "  extrude two from b.r distance=20 mm\n"
    "  sketch c on plane.XY {\n    circle k center=(30 mm, 20 mm) r=5 mm\n  }\n"
    "  extrude drill from c.k through_all op=subtract\n"
    "  sketch d on plane.XY {\n    rect r corner=(-10 mm, -10 mm) w=60 mm h=60 mm\n  }\n"
    "  extrude clip from d.r distance=40 mm op=intersect\n",
    "Two overlapping blocks united, a column drilled through both, then clipped to x <= 50.",
    36000 - 625 * PI, [0, 0, 0], [50, 40, 25],
    "a = Box(40, 30, 20, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "b = Pos(20, 10, 5) * Box(40, 30, 20, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "shape = a + b\n"
    "shape = shape - Pos(30, 20, 12.5) * Cylinder(5, 25)\n"
    "shape = shape & Pos(20, 20, 20) * Box(60, 60, 40)\n",
)
