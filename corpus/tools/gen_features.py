#!/usr/bin/env python3
"""Regenerates the detail-feature and naming-robustness corpus parts (milestone M6).

Usage: B123_PYTHON=/path/to/python-with-build123d python3 corpus/tools/gen_features.py
       (then: tcad fmt corpus)

Volumes and bounding boxes are closed-form, computed here. Surface area and center of mass of
parts with fillets and tapers have no short closed form; those two values are taken from the
independent build123d model of each part (listed under "baseline" in expected.json) and are then
checked against our own results at tier T2 in CI. Sweeps list, for every swept parameter set, the
closed-form volume and bounding box.
"""
import itertools
import math
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))

from corpuslib import *  # noqa: F401,F403


PLATE = "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=60 mm h=40 mm\n  }\n  extrude plate from s.r distance=10 mm\n"
PLATE_REF = "shape = Box(60, 40, 10, align=(Align.MIN, Align.MIN, Align.MIN))\n"

# 1. simple holes
emit(
    "feat_hole_simple", "Plate with a through hole and a blind hole",
    PLATE
    + "  hole through_hole on=plate.cap_end at=(15 mm, 20 mm) d=8 mm end=through_all\n"
    + "  hole blind_hole on=plate.cap_end at=(45 mm, 20 mm) d=6 mm depth=5 mm\n",
    "Holes cut from a named face; the through hole passes all the way, the blind hole stops at its floor.",
    60 * 40 * 10 - PI * 16 * 10 - PI * 9 * 5, [0, 0, 0], [60, 40, 10],
    PLATE_REF
    + "shape = shape - Pos(15, 20, 5) * Cylinder(4, 10)\n"
    + "shape = shape - Pos(45, 20, 7.5) * Cylinder(3, 5)\n",
)

# 2. counterbore and countersink
emit(
    "feat_hole_cbore_csk", "Plate with a counterbore and a countersink",
    PLATE
    + "  hole cbore on=plate.cap_end at=(15 mm, 20 mm) d=6 mm cbore_d=12 mm cbore_depth=4 mm end=through_all\n"
    + "  hole csk on=plate.cap_end at=(45 mm, 20 mm) d=6 mm csk_d=12 mm end=through_all\n",
    "A counterbored hole and a 90 degree countersunk hole.",
    60 * 40 * 10 - 198 * PI - 126 * PI, [0, 0, 0], [60, 40, 10],
    PLATE_REF
    + "shape = shape - Pos(15, 20, 5) * Cylinder(3, 10)\n"
    + "shape = shape - Pos(15, 20, 8) * Cylinder(6, 4)\n"
    + "shape = shape - Pos(45, 20, 5) * Cylinder(3, 10)\n"
    + "shape = shape - Pos(45, 20, 8.5) * Cone(3, 6, 3)\n",
)

# 3. disc with tapped holes on a bolt circle
emit(
    "feat_tapped_circular", "Flange with six tapped holes",
    "  sketch s on plane.XY {\n    circle c center=(0 mm, 0 mm) r=30 mm\n  }\n"
    "  extrude disc from s.c distance=8 mm\n"
    "  hole bore on=disc.cap_end at=(0 mm, 0 mm) d=10 mm end=through_all\n"
    "  hole tap on=disc.cap_end at=(20 mm, 0 mm) thread=\"M6\" depth=6 mm\n"
    "  circular_pattern bolts of=tap axis=axis.Z count=6\n",
    "M6 tap drill is 5 mm; the six holes are a circular pattern of the first.",
    PI * 900 * 8 - PI * 25 * 8 - 6 * PI * 6.25 * 6, [-30, -30, 0], [30, 30, 8],
    "shape = Pos(0, 0, 4) * Cylinder(30, 8)\n"
    "shape = shape - Pos(0, 0, 4) * Cylinder(5, 8)\n"
    "for i in range(6):\n"
    "    shape = shape - Pos(20 * cos(i * pi / 3), 20 * sin(i * pi / 3), 5) * Cylinder(2.5, 6)\n",
)

BLOCK = "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=50 mm h=30 mm\n  }\n  extrude boss from s.r distance=12 mm\n"
BLOCK_REF = "shape = Box(50, 30, 12, align=(Align.MIN, Align.MIN, Align.MIN))\n"

# 4. fillet the vertical edges
emit(
    "feat_fillet_vertical", "Block with rounded vertical edges",
    BLOCK + "  fillet corners edges=query(edges, parallel_to=+Z) r=5 mm\n",
    "The four edges parallel to Z found by a geometric query, rounded with r=5.",
    50 * 30 * 12 - (4 - PI) * 25 * 12, [0, 0, 0], [50, 30, 12],
    BLOCK_REF + "shape = fillet(shape.edges().filter_by(Axis.Z), 5)\n",
)

# 5. chamfer two named edges
emit(
    "feat_chamfer", "Block with two chamfered edges",
    BLOCK
    + "  chamfer bevel edges=[edge(boss.cap_end & boss.side(s.r.bottom)), edge(boss.cap_end & boss.side(s.r.top))] d=3 mm\n",
    "Equal-distance chamfers on the two long top edges, named by the faces they join.",
    50 * 30 * 12 - 2 * 0.5 * 9 * 50, [0, 0, 0], [50, 30, 12],
    BLOCK_REF
    + "edges = shape.edges().filter_by(Axis.X).filter_by_position(Axis.Z, 11.9, 12.1)\n"
    + "shape = chamfer(edges, 3)\n",
)

# 6. shell
emit(
    "feat_shell", "Open box",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=50 mm h=30 mm\n  }\n"
    "  extrude boss from s.r distance=20 mm\n"
    "  shell box faces=[boss.cap_end] thickness=2 mm\n",
    "A block hollowed from its top face with 2 mm walls.",
    50 * 30 * 20 - 46 * 26 * 18, [0, 0, 0], [50, 30, 20],
    "shape = Box(50, 30, 20, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "shape = shape - Pos(25, 15, 2 + 9) * Box(46, 26, 18)\n",
)

# 7. draft on the side faces
w2 = 40 - 2 * 20 * math.tan(math.radians(3))
emit(
    "feat_draft_faces", "Tapered block",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=40 mm h=40 mm\n  }\n"
    "  extrude boss from s.r distance=20 mm\n"
    "  draft taper faces=[boss.side(s.r.bottom), boss.side(s.r.right), boss.side(s.r.top), boss.side(s.r.left)] angle=3 deg pull=+Z neutral=boss.cap_start\n",
    "All four sides drafted by 3 degrees about the bottom: a square frustum.",
    20 / 3 * (1600 + w2 * w2 + 40 * w2), [0, 0, 0], [40, 40, 20],
    "shape = Pos(20, 20, 0) * extrude(Rectangle(40, 40), amount=20, taper=3)\n",
)

# 8. rib
emit(
    "feat_rib", "Plate with a triangular rib",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=60 mm h=40 mm\n  }\n"
    "  extrude base from s.r distance=5 mm\n"
    "  plane top from=plane.XY offset=5 mm\n"
    "  plane mid from=plane.XZ offset=-20 mm\n"
    "  sketch web on mid {\n    line l start=(10 mm, 5 mm) end=(50 mm, 25 mm)\n  }\n"
    "  rib stiffener profile=web thickness=4 mm direction=-Z to=top\n",
    "A 4 mm rib: the region between the centerline and the plate, 40 mm long and 20 mm high.",
    60 * 40 * 5 + 0.5 * 40 * 20 * 4, [0, 0, 0], [60, 40, 25],
    "shape = Box(60, 40, 5, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "sk = Plane.XZ.offset(-20) * Polygon((10, 5), (50, 5), (50, 25), align=None)\n"
    "shape = shape + extrude(sk, amount=2, both=True)\n",
)

# 9. linear pattern with a skipped instance
emit(
    "feat_pattern_linear", "Bar with a row of holes",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=60 mm h=20 mm\n  }\n"
    "  extrude bar from s.r distance=8 mm\n"
    "  hole h1 on=bar.cap_end at=(5 mm, 10 mm) d=4 mm end=through_all\n"
    "  linear_pattern row of=h1 dir=+X spacing=10 mm count=5 skip=[3]\n",
    "Five instances of a hole, instance 3 skipped.",
    60 * 20 * 8 - 4 * PI * 4 * 8, [0, 0, 0], [60, 20, 8],
    "shape = Box(60, 20, 8, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "for x in (5, 15, 35, 45):\n"
    "    shape = shape - Pos(x, 10, 4) * Cylinder(2, 8)\n",
)

# 10. pattern of a pattern
emit(
    "feat_pattern_of_pattern", "Plate with a 4 x 3 grid of holes",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=80 mm h=60 mm\n  }\n"
    "  extrude plate from s.r distance=6 mm\n"
    "  hole h on=plate.cap_end at=(10 mm, 10 mm) d=5 mm end=through_all\n"
    "  linear_pattern cols of=h dir=+X spacing=20 mm count=4\n"
    "  linear_pattern rows of=cols dir=+Y spacing=15 mm count=3\n",
    "A row of four holes, repeated in three rows.",
    80 * 60 * 6 - 12 * PI * 6.25 * 6, [0, 0, 0], [80, 60, 6],
    "shape = Box(80, 60, 6, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "for x in (10, 30, 50, 70):\n"
    "    for y in (10, 25, 40):\n"
    "        shape = shape - Pos(x, y, 3) * Cylinder(2.5, 6)\n",
)

# 11. mirror
emit(
    "feat_mirror", "Bar with a mirrored hole",
    "  sketch s on plane.XY {\n    rect r corner=(-30 mm, 0 mm) w=60 mm h=20 mm\n  }\n"
    "  extrude bar from s.r distance=10 mm\n"
    "  hole h on=bar.cap_end at=(15 mm, 10 mm) d=6 mm end=through_all\n"
    "  mirror m of=h plane=plane.YZ\n",
    "The hole at x=15 and its mirror image in the YZ plane at x=-15.",
    60 * 20 * 10 - 2 * PI * 9 * 10, [-30, 0, 0], [30, 20, 10],
    "shape = Box(60, 20, 10, align=(Align.CENTER, Align.MIN, Align.MIN))\n"
    "for x in (15, -15):\n"
    "    shape = shape - Pos(x, 10, 5) * Cylinder(3, 10)\n",
)


# 12. L bracket with named faces, a tag, an edge fillet and a parameter sweep
def bracket_volume(w, t, h, l):
    r = 3.0
    area = l * t + (h - t) * t
    return area * w - 2 * PI * 9 * t + (1 - PI / 4) * r * r * w


bracket_body = """  param w = 40 mm range(20 mm, 100 mm)
  param t = 6 mm range(4 mm, 10 mm)
  param h = 50 mm range(30 mm, 100 mm)
  param l = 60 mm range(40 mm, 100 mm)

  sketch prof on plane.XZ {
    line l0 start=(0 mm, 0 mm) end=(l, 0 mm)
    line l1 start=(l, 0 mm) end=(l, t)
    line l2 start=(l, t) end=(t, t)
    line l3 start=(t, t) end=(t, h)
    line l4 start=(t, h) end=(0 mm, h)
    line l5 start=(0 mm, h) end=(0 mm, 0 mm)
    constrain horizontal(l0)
    constrain vertical(l1)
    constrain horizontal(l2)
    constrain vertical(l3)
    constrain horizontal(l4)
    constrain vertical(l5)
  }
  extrude body from prof distance=w symmetric

  tag deck = body.side(prof.l2)
  tag wall = body.side(prof.l3)

  hole deck_hole on=deck at=(l - 15 mm, 0 mm) d=6 mm end=through_all
  hole wall_hole on=wall at=(0 mm, h - 12 mm) d=6 mm end=through_all
  fillet inner_corner edges=[edge(deck & wall)] r=3 mm
"""
def bracket_ref(w, t, h, l):
    return (
        f"w, t, h, l = {w}, {t}, {h}, {l}\n"
        "pts = [(0, 0), (l, 0), (l, t), (t, t), (t, h), (0, h)]\n"
        "shape = extrude(Plane.XZ * Polygon(*pts, align=None), amount=w / 2, both=True)\n"
        "shape = shape - Pos(l - 15, 0, t / 2) * Cylinder(3, t + 2)\n"
        "shape = shape - Pos(t / 2, 0, h - 12) * (Rot(0, 90, 0) * Cylinder(3, t + 2))\n"
        "inner = shape.edges().filter_by(Axis.Y).filter_by_position(Axis.Z, t - 0.01, t + 0.01).filter_by_position(Axis.X, t - 0.01, t + 0.01)\n"
        "shape = fillet(inner, 3)\n"
    )

sweeps = []
for w, t, h, l in itertools.product([30, 40, 70], [4, 6, 9], [40, 50, 80], [50, 70]):
    sweeps.append({
        "set": {"w": f"{w} mm", "t": f"{t} mm", "h": f"{h} mm", "l": f"{l} mm"},
        "volume_mm3": bracket_volume(w, t, h, l),
        "bbox_min_mm": [0, -w / 2, 0],
        "bbox_max_mm": [l, w / 2, h],
    })
emit(
    "nam_bracket", "L bracket (naming robustness)",
    bracket_body,
    "Faces named by profile segments, two tags, holes placed on tagged faces and a fillet on the edge between them; "
    "every swept parameter set must resolve all references and keep the same names.",
    bracket_volume(40, 6, 50, 60), [0, -20, 0], [60, 20, 50],
    bracket_ref(40, 6, 50, 60),
    sweeps,
)


# 13. block with a groove that splits the top face
def groove_volume(w, gx, gw):
    r = 1.5
    return w * 30 * 12 - gw * 30 * 4 + 2 * (1 - PI / 4) * r * r * 30 - PI * 9 * 12


groove_body = """  param w = 60 mm range(40 mm, 120 mm)
  param gx = 20 mm range(8 mm, 60 mm)
  param gw = 8 mm range(4 mm, 20 mm)

  sketch s on plane.XY {
    rect r corner=(0 mm, 0 mm) w=w h=30 mm
  }
  extrude boss from s.r distance=12 mm

  sketch cut on boss.cap_end {
    rect c corner=(gx, -5 mm) w=gw h=40 mm
  }
  extrude groove from cut.c distance=-4 mm op=subtract

  fillet floor_round edges=[edge(groove.cap_end & groove.side(cut.c.left)), edge(groove.cap_end & groove.side(cut.c.right))] r=1.5 mm
  hole through_hole on=boss.cap_end#2 at=(gx + gw + (w - gx - gw) / 2, 15 mm) d=6 mm end=through_all
"""
def groove_ref(w, gx, gw):
    return (
        f"w, gx, gw = {w}, {gx}, {gw}\n"
        "shape = Box(w, 30, 12, align=(Align.MIN, Align.MIN, Align.MIN))\n"
        "shape = shape - Pos(gx + gw / 2, 15, 12 - 2) * Box(gw, 40, 4)\n"
        "floor_edges = shape.edges().filter_by(Axis.Y).filter_by_position(Axis.Z, 7.99, 8.01)\n"
        "shape = fillet(floor_edges, 1.5)\n"
        "shape = shape - Pos(gx + gw + (w - gx - gw) / 2, 15, 6) * Cylinder(3, 12)\n"
    )

sweeps = []
for w, gx, gw in itertools.product([60, 90], [12, 20, 30], [6, 8, 12]):
    sweeps.append({
        "set": {"w": f"{w} mm", "gx": f"{gx} mm", "gw": f"{gw} mm"},
        "volume_mm3": groove_volume(w, gx, gw),
        "bbox_min_mm": [0, 0, 0],
        "bbox_max_mm": [w, 30, 12],
    })
emit(
    "nam_split_merge", "Grooved block (naming robustness)",
    groove_body,
    "A groove splits the top face in two; the hole refers to the right-hand piece by ordinal (#2) and the groove "
    "floor edges by the faces they join. Swept groove position and width must keep every reference valid.",
    groove_volume(60, 20, 8), [0, 0, 0], [60, 30, 12],
    groove_ref(60, 20, 8),
    sweeps,
)
