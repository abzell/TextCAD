#!/usr/bin/env python3
"""Corpus cases for connectors (spec/features.md § Connectors and § Published geometry; issue #28).

Writes `con_*` cases: three parts with geometry whose expected connector frames are computed here in
closed form and checked against frames that each case's independent build123d model derives from its
own geometry, and small invalid parts that must fail with exactly one diagnostic.

Usage: B123_PYTHON=/path/to/python-with-build123d python3 corpus/tools/gen_connectors.py
       (then bring the files into canonical form, spec/canonical-form.md)
"""
import itertools
import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from corpuslib import *  # noqa: F401,F403

from framelib import *  # noqa: F401,F403


def emit_connectors(name, title, body, note, volume, lo, hi, ref, frames_code, connectors, absent,
                    sweeps=None, published=None, configs=None):
    code = REF_HEAD + REF_FRAMES + REF_PUBLISHED + ref + "\n" + ref_tail(frames_code)
    out = subprocess.run([PY, "-c", code], capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"{name}: build123d failed:\n{out.stderr}")
    got = json.loads(out.stdout.strip().splitlines()[-1])
    baseline = ["area_mm2", "center_of_mass_mm"]
    if volume is None:
        volume, lo, hi = got["volume_mm3"], got["bbox_min_mm"], got["bbox_max_mm"]
        baseline = ["volume_mm3", "area_mm2", "center_of_mass_mm", "bbox_min_mm", "bbox_max_mm"]
    if abs(got["volume_mm3"] - volume) > 1e-6 * volume:
        sys.exit(f"{name}: closed-form volume {volume} disagrees with build123d {got['volume_mm3']}")
    for a, b in zip(got["bbox_min_mm"] + got["bbox_max_mm"], list(lo) + list(hi)):
        if abs(a - b) > 1e-5:
            sys.exit(f"{name}: closed-form bounding box disagrees with build123d")
    explicit = {k: v for k, v in connectors.items() if k != "origin"}
    check_frames(name, explicit, got["connectors"])
    if published:
        check_published(name, published, got["published"])
    expected = {
        "note": note,
        "volume_mm3": volume,
        "area_mm2": got["area_mm2"],
        "center_of_mass_mm": got["center_of_mass_mm"],
        "bbox_min_mm": list(lo),
        "bbox_max_mm": list(hi),
        "topology": {"solids": 1},
        "connectors": {k: clean(v) for k, v in connectors.items()},
        "absent": absent,
        "baseline": baseline,
    }
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
    if sweeps:
        expected["sweeps"] = sweeps
    d = os.path.join(ROOT, name)
    write(os.path.join(d, "bundle.toml"),
          f'format = "0.1"\nname = "{name}"\nentry = "parts/{name}.tcad"\nunits = "mm"\nlicense = "Apache-2.0"\n')
    write(os.path.join(d, "parts", f"{name}.tcad"), f'tcad 0.2\n\npart {name} "{title}" {{\n{body}}}\n')
    write(os.path.join(d, "expected.json"), json.dumps(expected, indent=2) + "\n")
    write(os.path.join(d, "reference.py"), code)
    print("wrote", name)


def emit_invalid(name, title, body, code, note, version="0.2"):
    d = os.path.join(ROOT, name)
    write(os.path.join(d, "parts", f"{name}.tcad"), f'tcad {version}\n\npart {name} "{title}" {{\n{body}}}\n')
    write(os.path.join(d, "expected.json"),
          json.dumps({"codes": [code], "fails": True, "note": note}, indent=2) + "\n")
    print("wrote", name)


import subprocess  # noqa: E402  (after corpuslib, which defines PY)

# 1. connectors from holes -------------------------------------------------------------------------

def hole_frames(t):
    top = [0, 0, 1]
    c = {
        "pin.p1": frame([10, 10, t], top),
        "pin.p2": frame([50, 10, t], top),
        "pin_under.p1": frame([10, 10, t], top, flip=True),
        "pin_under.p2": frame([50, 10, t], top, flip=True),
        "seat": frame([45, 30, t], top),
        "bolt_seat.i1": frame([10, 30, t], top),
        "bolt_seat.i3": frame([26, 30, t], top),
        "port_face": frame([20, 0, t / 2], [0, -1, 0]),
        "origin": IDENTITY,
    }
    return c


def hole_volume(t):
    return (60 * 40 * t - 2 * PI * 9 * t - (PI * 9 * (t - 3) + PI * 30.25 * 3) - 2 * PI * 4 * t
            - PI * 6.25 * 40)


hole_sweeps = []
for t in (6, 10, 14):
    hole_sweeps.append({
        "set": {"t": f"{t} mm"},
        "volume_mm3": hole_volume(t),
        "bbox_min_mm": [0, 0, 0],
        "bbox_max_mm": [60, 40, t],
        "connectors": {k: clean(v) for k, v in hole_frames(t).items()},
    })

emit_connectors(
    "con_hole_frames", "Plate with connectors on holes",
    "  param t = 10 mm range(6 mm, 14 mm)\n\n"
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=60 mm h=40 mm\n  }\n"
    "  extrude plate from s.r distance=t\n"
    "  hole pins on=plate.cap_end at=[(10 mm, 10 mm), (50 mm, 10 mm)] d=6 mm end=through_all\n"
    "  hole cb on=plate.cap_end at=(45 mm, 30 mm) d=6 mm cbore_d=11 mm cbore_depth=3 mm end=through_all\n"
    "  hole bolt on=plate.cap_end at=(10 mm, 30 mm) d=4 mm end=through_all\n"
    "  linear_pattern bolts of=bolt dir=+X count=3 spacing=8 mm skip=[2]\n"
    "  hole port on=plate.side(s.r.bottom) at=(20 mm, t / 2) d=5 mm end=through_all\n\n"
    "  connector pin from=pins\n"
    "  connector pin_under from=pins flip\n"
    "  connector seat from=cb\n"
    "  connector bolt_seat from=bolts\n"
    "  connector port_face from=port\n",
    "Connectors from holes: one per position (.p1, .p2), one per pattern instance (.i1, .i3; the "
    "skipped .i2 does not exist), at the top of a counterbore, on a side face (z out of the material, "
    "x by the hole-frame rule), and flipped. The thickness sweep moves every origin with the top face.",
    hole_volume(10), [0, 0, 0], [60, 40, 10],
    "shape = Box(60, 40, 10, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "for x, y in ((10, 10), (50, 10)):\n"
    "    shape = shape - Pos(x, y, 5) * Cylinder(3, 10)\n"
    "shape = shape - Pos(45, 30, 5) * Cylinder(3, 10)\n"
    "shape = shape - Pos(45, 30, 8.5) * Cylinder(5.5, 3)\n"
    "for x in (10, 26):\n"
    "    shape = shape - Pos(x, 30, 5) * Cylinder(2, 10)\n"
    "shape = shape - Pos(20, 20, 5) * Rot(90, 0, 0) * Cylinder(2.5, 40)\n",
    "_o, _n = face_frame(planar_face(shape, [0, 0, 1], (30, 20, 10)))\n"
    "for key, near, r in (('pin.p1', (10, 10, 10), 3), ('pin.p2', (50, 10, 10), 3)):\n"
    "    connectors[key] = _frame(circle_centre(shape, r, near), _n)\n"
    "    connectors[key.replace('pin', 'pin_under')] = _frame(circle_centre(shape, r, near), _n, flip=True)\n"
    "connectors['seat'] = _frame(circle_centre(shape, 5.5, (45, 30, 10)), _n)\n"
    "connectors['bolt_seat.i1'] = _frame(circle_centre(shape, 2, (10, 30, 10)), _n)\n"
    "connectors['bolt_seat.i3'] = _frame(circle_centre(shape, 2, (26, 30, 10)), _n)\n"
    "_o, _side = face_frame(planar_face(shape, [0, -1, 0], (30, 0, 5)))\n"
    "connectors['port_face'] = _frame(circle_centre(shape, 2.5, (20, 0, 5)), _side)\n",
    hole_frames(10), ["pin", "pin_under", "bolt_seat", "bolt_seat.i2"], hole_sweeps,
)

# 2. connectors from explicit arguments -------------------------------------------------------------

explicit = {
    "top": frame([20, 15, 20], [0, 0, 1]),
    "front": frame([20, 0, 10], [0, -1, 0]),
    "corner": frame([40, 30, 0], [0, 0, -1], x=[1, 1, 0]),
    "tilt": frame([0, 0, 0], [0, 1, 1], x=[1, 0, 0]),
    "seat": frame([10, 5, 20], [0, 0, 1], x=[1, 0, 0], rotate_deg=90, offset=(0, 0, 2)),
    "probe": frame([0, 0, 20], [0, 0, 1], flip=True, rotate_deg=90, offset=(5, 0, 0)),
    "origin": IDENTITY,
}
emit_connectors(
    "con_explicit", "Block with connectors from explicit arguments",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=40 mm h=30 mm\n  }\n"
    "  extrude base from s.r distance=20 mm\n"
    "  csys seat_csys origin=(10 mm, 5 mm, 20 mm) x=(1, 0, 0) y=(0, 1, 0)\n\n"
    "  connector top origin=base.cap_end z=base.cap_end\n"
    "  connector front origin=base.side(s.r.bottom) z=base.side(s.r.bottom)\n"
    "  connector corner origin=(40 mm, 30 mm, 0 mm) z=(0, 0, -1) x=(1, 1, 0)\n"
    "  connector tilt origin=(0 mm, 0 mm, 0 mm) z=(0, 1, 1) x=+X\n"
    "  connector seat csys=seat_csys rotate=90 deg offset=(0 mm, 0 mm, 2 mm)\n"
    "  connector probe origin=(0 mm, 0 mm, 20 mm) z=+Z rotate=90 deg offset=(5 mm, 0 mm, 0 mm) flip\n",
    "Frames from a face centroid and normal (top, and front with x by the default rule), from points "
    "and directions (an explicit direction keeps its sign; x is normalized and projected), from a csys "
    "rotated and offset, and a frame that is flipped, then rotated, then offset in its own axes. Every "
    "part also has the implicit connector origin.",
    40 * 30 * 20, [0, 0, 0], [40, 30, 20],
    "shape = Box(40, 30, 20, align=(Align.MIN, Align.MIN, Align.MIN))\n",
    "_o, _n = face_frame(planar_face(shape, [0, 0, 1], (20, 15, 20)))\n"
    "connectors['top'] = _frame(_o, _n)\n"
    "_o, _n = face_frame(planar_face(shape, [0, -1, 0], (20, 0, 10)))\n"
    "connectors['front'] = _frame(_o, _n)\n"
    "connectors['corner'] = _frame([40, 30, 0], [0, 0, -1], x=[1, 1, 0])\n"
    "connectors['tilt'] = _frame([0, 0, 0], [0, 1, 1], x=[1, 0, 0])\n"
    "connectors['seat'] = _frame([10, 5, 20], [0, 0, 1], x=[1, 0, 0], rotate=90, offset=(0, 0, 2))\n"
    "connectors['probe'] = _frame([0, 0, 20], [0, 0, 1], flip=True, rotate=90, offset=(5, 0, 0))\n",
    explicit, [],
)

# 3. connectors on faces and edges of revolution ----------------------------------------------------

revolution = {
    "boss_start": frame([15, 20, 30], [0, 0, 1]),
    "boss_mid": frame([15, 20, 40], [0, 0, 1]),
    "boss_end": frame([15, 20, 50], [0, 0, 1]),
    "boss_rim": frame([15, 20, 50], [0, 0, 1]),
    "cross_axis": frame([40, 0, 15], [0, 1, 0]),
    "cross_end": frame([40, 40, 15], [0, 1, 0]),
    "cross_entry": frame([40, 40, 15], [0, 1, 0]),
    "cross_down": frame([40, 0, 15], [0, 1, 0], flip=True),
    "origin": IDENTITY,
}
emit_connectors(
    "con_revolution", "Block with a boss and a cross hole: connectors on axes",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=60 mm h=40 mm\n  }\n"
    "  extrude base from s.r distance=30 mm\n"
    "  sketch t on base.cap_end {\n    circle c center=(15 mm, 20 mm) r=8 mm\n  }\n"
    "  extrude boss from t.c distance=20 mm\n"
    "  hole cross on=base.side(s.r.top) at=(-40 mm, 15 mm) d=8 mm end=through_all\n\n"
    "  connector boss_start origin=boss.side(t.c) z=boss.side(t.c)\n"
    "  connector boss_mid origin=boss.side(t.c) z=boss.side(t.c) origin_at=middle\n"
    "  connector boss_end origin=boss.side(t.c) z=boss.side(t.c) origin_at=end\n"
    "  connector boss_rim origin=edge(boss.cap_end & boss.side(t.c)) z=+Z\n"
    "  connector cross_axis origin=cross.wall z=cross.wall\n"
    "  connector cross_end origin=cross.wall z=cross.wall origin_at=end\n"
    "  connector cross_entry from=cross\n"
    "  connector cross_down origin=cross.wall z=cross.wall flip\n",
    "Axes of a cylindrical boss and of a hole drilled in -Y from the back face. An axis of revolution "
    "has no sign of its own: both point along +Y or +Z (first non-zero component positive), so the "
    "hole axis starts at the exit side (y = 0) while the hole connector (from=) sits at the entry with z "
    "out of the material. origin_at picks start, middle or end along the oriented axis; flip turns the "
    "frame in place and does not move the origin.",
    60 * 40 * 30 + PI * 64 * 20 - PI * 16 * 40, [0, 0, 0], [60, 40, 50],
    "shape = Box(60, 40, 30, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "shape = shape + Pos(15, 20, 40) * Cylinder(8, 20)\n"
    "shape = shape - Pos(40, 20, 15) * Rot(90, 0, 0) * Cylinder(4, 40)\n",
    "for key, at in (('boss_start', 'start'), ('boss_mid', 'middle'), ('boss_end', 'end')):\n"
    "    connectors[key] = _frame(*cylinder_axis(shape, 8, (15, 20, 40), at))\n"
    "connectors['boss_rim'] = _frame(circle_centre(shape, 8, (15, 20, 50)), [0, 0, 1])\n"
    "connectors['cross_axis'] = _frame(*cylinder_axis(shape, 4, (40, 20, 15), 'start'))\n"
    "connectors['cross_end'] = _frame(*cylinder_axis(shape, 4, (40, 20, 15), 'end'))\n"
    "_o, _back = face_frame(planar_face(shape, [0, 1, 0], (30, 40, 15)))\n"
    "connectors['cross_entry'] = _frame(circle_centre(shape, 4, (40, 40, 15)), _back)\n"
    "connectors['cross_down'] = _frame(*cylinder_axis(shape, 4, (40, 20, 15), 'start'), flip=True)\n",
    revolution, [],
)

# 4. a split face of revolution, mirrored holes, holes on a datum plane --------------------------------

split = {
    "pin_mid": frame([30, 20, 25], [0, 1, 0]),
    "pin_start": frame([30, 0, 25], [0, 1, 0]),
    "pin_piece_end": frame([30, 12, 25], [0, 1, 0]),
    "pin_entry": frame([30, 0, 25], [0, -1, 0]),
    "origin": IDENTITY,
}
emit_connectors(
    "con_split_axis", "Fork with a pin bore split by the slot",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=60 mm h=40 mm\n  }\n"
    "  extrude base from s.r distance=40 mm\n"
    "  sketch u on base.cap_end {\n    rect r corner=(0 mm, 12 mm) w=60 mm h=16 mm\n  }\n"
    "  extrude slot from u.r distance=-30 mm op=subtract\n"
    "  hole pin on=base.side(s.r.bottom) at=(30 mm, 25 mm) d=8 mm end=through_all\n\n"
    "  connector pin_mid origin=pin.wall z=pin.wall origin_at=middle\n"
    "  connector pin_start origin=pin.wall z=pin.wall\n"
    "  connector pin_piece_end origin=pin.wall#1 z=pin.wall#1 origin_at=end\n"
    "  connector pin_entry from=pin\n",
    "The slot splits the pin bore's wall into two pieces on one axis. Without an ordinal the connector "
    "takes both, so origin_at=middle is the middle of the fork gap (y = 20); with #1 it takes the first "
    "piece only (y = 0 to 12).",
    60 * 40 * 40 - 60 * 16 * 30 - PI * 16 * 24, [0, 0, 0], [60, 40, 40],
    "shape = Box(60, 40, 40, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "shape = shape - Pos(30, 20, 25) * Box(60, 16, 30)\n"
    "shape = shape - Pos(30, 20, 25) * Rot(90, 0, 0) * Cylinder(4, 40)\n",
    "connectors['pin_mid'] = _frame(*cylinder_axis_all(shape, 4, [30, 0, 25], [0, 1, 0], 'middle'))\n"
    "connectors['pin_start'] = _frame(*cylinder_axis_all(shape, 4, [30, 0, 25], [0, 1, 0], 'start'))\n"
    "connectors['pin_piece_end'] = _frame(*cylinder_axis(shape, 4, (30, 6, 25), 'end'))\n"
    "_o, _front = face_frame(planar_face(shape, [0, -1, 0], (30, 0, 20)))\n"
    "connectors['pin_entry'] = _frame(circle_centre(shape, 4, (30, 0, 25)), _front)\n",
    split, ["pin_mid.p1"],
)

mirrored = {
    "v4.m1.m1": frame([20, 10, 20], [0, 0, 1]),
    "v4.m1.m2": frame([-20, 10, 20], [0, 0, 1]),
    "v4.m2.m1": frame([20, -10, 20], [0, 0, 1]),
    "v4.m2.m2": frame([-20, -10, 20], [0, 0, 1]),
    "side.m1": frame([30, 0, 10], [1, 0, 0]),
    "side.m2": frame([-30, 0, 10], [-1, 0, 0]),
    "origin": IDENTITY,
}
emit_connectors(
    "con_mirror_holes", "Plate with mirrored holes",
    "  sketch s on plane.XY {\n    rect r corner=(-30 mm, -20 mm) w=60 mm h=40 mm\n  }\n"
    "  extrude plate from s.r distance=20 mm\n"
    "  hole v on=plate.cap_end at=(20 mm, 10 mm) d=6 mm end=through_all\n"
    "  mirror vx of=v plane=plane.YZ\n"
    "  mirror vxy of=vx plane=plane.XZ\n"
    "  hole sp on=plate.side(s.r.right) at=(0 mm, 10 mm) d=5 mm depth=8 mm\n"
    "  mirror sm of=sp plane=plane.YZ\n\n"
    "  connector v4 from=vxy\n"
    "  connector side from=sm\n",
    "A mirror gives .m1 (the original) and .m2 (the copy); a mirror of a mirror nests them, outermost "
    "first. The mirrored side hole's connector points out of the opposite face (z = -X) and stays "
    "right-handed (x by the default rule, not mirrored).",
    60 * 40 * 20 - 4 * PI * 9 * 20 - 2 * PI * 6.25 * 8, [-30, -20, 0], [30, 20, 20],
    "shape = Box(60, 40, 20, align=(Align.CENTER, Align.CENTER, Align.MIN))\n"
    "for x in (20, -20):\n"
    "    for y in (10, -10):\n"
    "        shape = shape - Pos(x, y, 10) * Cylinder(3, 20)\n"
    "shape = shape - Pos(26, 0, 10) * Rot(0, 90, 0) * Cylinder(2.5, 8)\n"
    "shape = shape - Pos(-26, 0, 10) * Rot(0, 90, 0) * Cylinder(2.5, 8)\n",
    "_o, _top = face_frame(planar_face(shape, [0, 0, 1], (0, 0, 20)))\n"
    "for key, x, y in (('v4.m1.m1', 20, 10), ('v4.m1.m2', -20, 10), ('v4.m2.m1', 20, -10), ('v4.m2.m2', -20, -10)):\n"
    "    connectors[key] = _frame(circle_centre(shape, 3, (x, y, 20)), _top)\n"
    "_o, _r = face_frame(planar_face(shape, [1, 0, 0], (30, 0, 10)))\n"
    "connectors['side.m1'] = _frame(circle_centre(shape, 2.5, (30, 0, 10)), _r)\n"
    "_o, _l = face_frame(planar_face(shape, [-1, 0, 0], (-30, 0, 10)))\n"
    "connectors['side.m2'] = _frame(circle_centre(shape, 2.5, (-30, 0, 10)), _l)\n",
    mirrored, ["v4", "v4.m1", "v4.m2", "side"],
)

datum = {
    "c_entry": frame([10, 0, 10], [0, -1, 0]),
    "cf_entry": frame([30, 0, 20], [0, 1, 0]),
    "origin": IDENTITY,
}
emit_connectors(
    "con_datum_hole", "Block with holes drilled from a datum plane",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, -20 mm) w=40 mm h=40 mm\n  }\n"
    "  extrude block from s.r distance=30 mm\n"
    "  hole c on=plane.XZ at=(10 mm, 10 mm) d=6 mm depth=25 mm\n"
    "  hole cf on=plane.XZ at=(30 mm, 20 mm) d=6 mm depth=25 mm flip\n\n"
    "  connector c_entry from=c\n"
    "  connector cf_entry from=cf\n",
    "Holes that start on plane.XZ inside the block and break out through a side. The connector sits on "
    "the plane with z opposite to the drilling direction: the plane's normal (-Y) for c, reversed (+Y) "
    "for the flipped cf.",
    40 * 40 * 30 - 2 * PI * 9 * 20, [0, -20, 0], [40, 20, 30],
    "shape = Box(40, 40, 30, align=(Align.MIN, Align.CENTER, Align.MIN))\n"
    "shape = shape - Pos(10, 12.5, 10) * Rot(90, 0, 0) * Cylinder(3, 25)\n"
    "shape = shape - Pos(30, -12.5, 20) * Rot(90, 0, 0) * Cylinder(3, 25)\n",
    "_p, _ax = cylinder_axis(shape, 3, (10, 10, 10), 'start')\n"
    "connectors['c_entry'] = _frame(_p, [-c for c in _ax])\n"
    "_p, _ax = cylinder_axis(shape, 3, (30, -10, 20), 'end')\n"
    "connectors['cf_entry'] = _frame(_p, _ax)\n",
    datum, [],
)

# 5. published geometry --------------------------------------------------------------------------

BLOCK = "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=60 mm h=40 mm\n  }\n  extrude base from s.r distance=20 mm\n"
BOSS = "  sketch t on base.cap_end {\n    circle c center=(15 mm, 20 mm) r=8 mm\n  }\n  extrude boss from t.c distance=15 mm\n"
BLOCK_REF = ("shape = Box(60, 40, 20, align=(Align.MIN, Align.MIN, Align.MIN))\n"
             "shape = shape + Pos(15, 20, 27.5) * Cylinder(8, 15)\n")


def plane(normal, point, area):
    return {"type": "plane", "normal": normal, "point_mm": point, "area_mm2": area}


top_area = 2400 - 100 * PI
bottom_area = 2400 - 9 * PI
surfaces = {
    "top": [plane([0, 0, 1], [(72000 - 2580 * PI) / top_area, 20, 20], top_area)],
    "boss_side": [{"type": "cylinder", "axis": [0, 0, 1], "axis_point_mm": [15, 20, 0], "radius_mm": 8,
                   "span_mm": [20, 35]}],
    "seat": [{"type": "cone", "axis": [0, 0, 1], "apex_mm": [45, 20, 14], "half_angle_deg": 45.0,
              "span_mm": [3, 6]}],
    "caps": [plane([0, 0, -1], [(72000 - 405 * PI) / bottom_area, 20, 0], bottom_area),
             plane([0, 0, 1], [15, 20, 35], 64 * PI)],
}
emit_connectors(
    "pub_surfaces", "Block with published faces",
    BLOCK + BOSS
    + "  hole h on=base.cap_end at=(45 mm, 20 mm) d=6 mm csk_d=12 mm end=through_all\n\n"
    "  publish top = base.cap_end\n"
    "  publish boss_side = boss.side(t.c)\n"
    "  publish seat = h.csk\n"
    "  publish caps = base.cap_start, boss.cap_end\n",
    "Published faces with their analytic parameters: a plane (outward normal, centroid, area), a "
    "cylinder (oriented axis, the axis point nearest the origin, radius, extent along the axis), a "
    "countersink cone (apex, half angle, extent from the apex) and a set of two planes in the order "
    "written.",
    48000 + 744 * PI, [0, 0, 0], [60, 40, 35],
    BLOCK_REF
    + "shape = shape - Pos(45, 20, 10) * Cylinder(3, 20)\n"
    "shape = shape - Pos(45, 20, 18.5) * Cone(3, 6, 3)\n",
    "published['top'] = [pub_plane(planar_face(shape, [0, 0, 1], (30, 20, 20)))]\n"
    "_cyl = [f for f in shape.faces() if f.geom_type == GeomType.CYLINDER\n"
    "        and abs(BRepAdaptor_Surface(f.wrapped).Cylinder().Radius() - 8) < 1e-9]\n"
    "published['boss_side'] = [pub_face(nearest(_cyl, (15, 20, 27.5)))]\n"
    "_cones = [f for f in shape.faces() if f.geom_type == GeomType.CONE]\n"
    "published['seat'] = [pub_face(nearest(_cones, (45, 20, 18.5)))]\n"
    "published['caps'] = [pub_plane(planar_face(shape, [0, 0, -1], (30, 20, 0))),\n"
    "                     pub_plane(planar_face(shape, [0, 0, 1], (15, 20, 35)))]\n",
    {"origin": IDENTITY}, [], published=surfaces,
)

edges = {
    "front_top": [{"type": "line", "ends_mm": [[0, 0, 20], [60, 0, 20]]}],
    "rim": [{"type": "line", "ends_mm": [[0, 0, 20], [60, 0, 20]]},
            {"type": "line", "ends_mm": [[60, 0, 20], [60, 40, 20]]}],
    "boss_rim": [{"type": "circle", "centre_mm": [15, 20, 35], "axis": [0, 0, 1], "radius_mm": 8}],
}
emit_connectors(
    "pub_edges", "Block with published edges",
    BLOCK + BOSS + "\n"
    "  publish front_top = edge(base.cap_end & base.side(s.r.bottom))\n"
    "  publish rim = edge(base.cap_end & base.side(s.r.bottom)), edge(base.cap_end & base.side(s.r.right))\n"
    "  publish boss_rim = edge(boss.cap_end & boss.side(t.c))\n",
    "A published line, a chain of two lines that share a corner (in the order written) and a full "
    "circle (centre, oriented axis, radius).",
    48000 + 960 * PI, [0, 0, 0], [60, 40, 35],
    BLOCK_REF,
    "_lines = [e for e in shape.edges() if e.geom_type == GeomType.LINE]\n"
    "published['front_top'] = [pub_edge(nearest(_lines, (30, 0, 20)))]\n"
    "published['rim'] = [pub_edge(nearest(_lines, (30, 0, 20))), pub_edge(nearest(_lines, (60, 20, 20)))]\n"
    "_circles = [e for e in shape.edges() if e.geom_type == GeomType.CIRCLE]\n"
    "published['boss_rim'] = [pub_edge(nearest(_circles, (15, 20, 35)))]\n",
    {"origin": IDENTITY}, [], published=edges,
)

bore = {"type": "cylinder", "axis": [0, 1, 0], "axis_point_mm": [30, 0, 25], "radius_mm": 4}
emit_connectors(
    "pub_split", "Fork with a published bore split by the slot",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=60 mm h=40 mm\n  }\n"
    "  extrude base from s.r distance=40 mm\n"
    "  sketch u on base.cap_end {\n    rect r corner=(0 mm, 12 mm) w=60 mm h=16 mm\n  }\n"
    "  extrude slot from u.r distance=-30 mm op=subtract\n"
    "  hole pin on=base.side(s.r.bottom) at=(30 mm, 25 mm) d=8 mm end=through_all\n\n"
    "  publish bore = pin.wall\n"
    "  publish bore_a = pin.wall#1\n",
    "A face split into pieces, published without an ordinal, gives all its pieces in ordinal order "
    "(the same axis, different extents); with #1 only the first.",
    60 * 40 * 40 - 60 * 16 * 30 - PI * 16 * 24, [0, 0, 0], [60, 40, 40],
    "shape = Box(60, 40, 40, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "shape = shape - Pos(30, 20, 25) * Box(60, 16, 30)\n"
    "shape = shape - Pos(30, 20, 25) * Rot(90, 0, 0) * Cylinder(4, 40)\n",
    "_cyl = sorted([f for f in shape.faces() if f.geom_type == GeomType.CYLINDER],\n"
    "              key=lambda f: f.center(CenterOf.MASS).Y)\n"
    "published['bore'] = [pub_face(f) for f in _cyl]\n"
    "published['bore_a'] = [pub_face(_cyl[0])]\n",
    {"origin": IDENTITY}, [],
    published={"bore": [dict(bore, span_mm=[0, 12]), dict(bore, span_mm=[28, 40])],
               "bore_a": [dict(bore, span_mm=[0, 12])]},
)

emit_connectors(
    "pub_sphere", "Sphere made by revolving an arc",
    "  sketch s on plane.XZ {\n"
    "    arc a start=(0 mm, -25 mm) end=(0 mm, 25 mm) through=(25 mm, 0 mm)\n"
    "    line l start=(0 mm, 25 mm) end=(0 mm, -25 mm)\n  }\n"
    "  revolve ball from s axis=axis.Z\n\n"
    "  publish ball_face = ball.side(s.a)\n",
    "A face of revolution is described by its geometry, not by how it was made: the revolved arc is "
    "a sphere.",
    4 / 3 * PI * 25 ** 3, [-25, -25, -25], [25, 25, 25],
    "shape = Sphere(25)\n",
    "published['ball_face'] = [pub_face(f) for f in shape.faces() if f.geom_type == GeomType.SPHERE]\n",
    {"origin": IDENTITY}, [],
    published={"ball_face": [{"type": "sphere", "centre_mm": [0, 0, 0], "radius_mm": 25}]},
)

emit_connectors(
    "pub_torus", "Torus made by revolving a circle",
    "  sketch k on plane.XZ {\n    circle c center=(30 mm, 0 mm) r=8 mm\n  }\n"
    "  revolve ring from k axis=axis.Z\n\n"
    "  publish ring_face = ring.side(k.c)\n",
    "The revolved circle is a torus: centre, oriented axis, major and minor radius.",
    2 * PI ** 2 * 30 * 64, [-38, -38, -8], [38, 38, 8],
    "shape = Torus(30, 8)\n",
    "published['ring_face'] = [pub_face(f) for f in shape.faces() if f.geom_type == GeomType.TORUS]\n",
    {"origin": IDENTITY}, [],
    published={"ring_face": [{"type": "torus", "centre_mm": [0, 0, 0], "axis": [0, 0, 1],
                              "major_radius_mm": 30, "minor_radius_mm": 8}]},
)

# 6. suppression ---------------------------------------------------------------------------------

sup_top = frame([30, 20, 20], [0, 0, 1])
emit_connectors(
    "con_suppressed", "Plate whose hole is suppressed in one configuration",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=60 mm h=40 mm\n  }\n"
    "  extrude base from s.r distance=20 mm\n"
    "  hole h on=base.cap_end at=(45 mm, 20 mm) d=10 mm end=through_all\n\n"
    "  connector seat from=h\n"
    "  connector top origin=base.cap_end z=base.cap_end\n"
    "  publish bore = h.wall\n\n"
    "  config plain { suppress h }\n",
    "A connector and a published face that refer to a suppressed feature are suppressed with it in that "
    "configuration (information I6014 for each), while the others stay. Without the configuration the "
    "part has all three.",
    48000 - PI * 25 * 20, [0, 0, 0], [60, 40, 20],
    "shape = Box(60, 40, 20, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "shape = shape - Pos(45, 20, 10) * Cylinder(5, 20)\n",
    "_o, _top = face_frame(planar_face(shape, [0, 0, 1], (30, 20, 20)))\n"
    "connectors['seat'] = _frame(circle_centre(shape, 5, (45, 20, 20)), _top)\n"
    "connectors['top'] = _frame(_o, _top)\n"
    "published['bore'] = [pub_face(f) for f in shape.faces() if f.geom_type == GeomType.CYLINDER]\n",
    {"seat": frame([45, 20, 20], [0, 0, 1]),
     "top": frame([(72000 - 1125 * PI) / (2400 - 25 * PI), 20, 20], [0, 0, 1]),
     "origin": IDENTITY},
    [],
    published={"bore": [{"type": "cylinder", "axis": [0, 0, 1], "axis_point_mm": [45, 20, 0],
                         "radius_mm": 5, "span_mm": [0, 20]}]},
    configs={"plain": {"connectors": {"top": sup_top, "origin": IDENTITY}, "published": {},
                       "info": ["I6014", "I6014"]}},
)

# 7. connectors through the naming sweeps ----------------------------------------------------------
# The four naming-robustness parts of gen_naming_parts.py (format 0.1 cases nam_*), raised to format
# 0.2 with connectors. Every swept parameter set lists its connectors: they must keep resolving and
# move with the geometry. Volumes are the closed forms of gen_naming_parts.py.

def sweep_frames(names, ranges, volume, bbox, frames):
    out = []
    for vals in itertools.product(*ranges):
        p = dict(zip(names, vals))
        lo, hi = bbox(**p)
        out.append({
            "set": {k: f"{v} mm" for k, v in p.items()},
            "volume_mm3": volume(**p),
            "bbox_min_mm": lo,
            "bbox_max_mm": hi,
            "connectors": {k: clean(v) for k, v in frames(**p).items()},
        })
    return out


def _chamfer_ring(r, d):
    return 2 * PI * (r + d / 3) * d * d / 2


def flange_volume(r_out, t, rb):
    return (PI * r_out**2 * t - PI * 100 * t - 6 * PI * 9 * t - _chamfer_ring(10, 1.5)
            - _chamfer_ring(3, 1.5))


def flange_frames(r_out, t, rb):
    c = {f"bolt_seat.i{k + 1}": frame([rb * math.cos(k * PI / 3), rb * math.sin(k * PI / 3), t], [0, 0, 1])
         for k in range(6)}
    c["bore_mid"] = frame([0, 0, (t - 1.5) / 2], [0, 0, 1])
    c["bore_top"] = frame([0, 0, t - 1.5], [0, 0, 1])
    c["origin"] = IDENTITY
    return c


emit_connectors(
    "con_sweep_flange", "Flange with a bolt circle and connectors",
    "  param r_out = 50 mm range(40 mm, 70 mm)\n"
    "  param t = 10 mm range(6 mm, 14 mm)\n"
    "  param rb = 30 mm range(25 mm, 35 mm)\n\n"
    "  sketch s on plane.XY {\n    circle c center=(0 mm, 0 mm) r=r_out\n  }\n"
    "  extrude disc from s.c distance=t\n"
    "  hole bore on=disc.cap_end at=(0 mm, 0 mm) d=20 mm end=through_all\n"
    "  hole bolt on=disc.cap_end at=(rb, 0 mm) d=6 mm end=through_all\n"
    "  circular_pattern bolts of=bolt axis=axis.Z count=6\n"
    "  chamfer rims edges=[edge(disc.cap_end & bore.wall), edge(disc.cap_end & bolts.i3.wall)] d=1.5 mm\n\n"
    "  connector bolt_seat from=bolts\n"
    "  connector bore_mid origin=bore.wall z=bore.wall origin_at=middle\n"
    "  connector bore_top origin=bore.wall z=bore.wall origin_at=end\n"
    "  publish bore_surface = bore.wall\n",
    "nam_flange_pattern with connectors: one per bolt-circle instance (the chamfered .i3 included, its "
    "origin on the hole's entry plane), and two on the bore wall, which the chamfer shortens. Every swept "
    "radius, thickness and bolt circle moves them with the geometry.",
    flange_volume(50, 10, 30), [-50, -50, 0], [50, 50, 10],
    "r_out, t, rb = 50, 10, 30\n"
    "shape = Pos(0, 0, t / 2) * Cylinder(r_out, t)\n"
    "shape = shape - Pos(0, 0, t / 2) * Cylinder(10, t)\n"
    "for i in range(6):\n"
    "    shape = shape - Pos(rb * cos(i * pi / 3), rb * sin(i * pi / 3), t / 2) * Cylinder(3, t)\n"
    "top = shape.edges().filter_by(GeomType.CIRCLE).filter_by_position(Axis.Z, t - 0.01, t + 0.01)\n"
    "ch = [e for e in top if abs(e.radius - 10) < 1e-6 or (abs(e.radius - 3) < 1e-6 and abs(e.arc_center.X - rb * cos(2 * pi / 3)) < 1e-6 and abs(e.arc_center.Y - rb * sin(2 * pi / 3)) < 1e-6)]\n"
    "shape = chamfer(ch, 1.5)\n",
    "_o, _top = face_frame(planar_face(shape, [0, 0, 1], (0, 40, 10)))\n"
    "for k in range(6):\n"
    "    r = 4.5 if k == 2 else 3\n"
    "    connectors[f'bolt_seat.i{k + 1}'] = _frame(circle_centre(shape, r, (30 * cos(k * pi / 3), 30 * sin(k * pi / 3), 10)), _top)\n"
    "connectors['bore_mid'] = _frame(*cylinder_axis(shape, 10, (0, 10, 4), 'middle'))\n"
    "connectors['bore_top'] = _frame(*cylinder_axis(shape, 10, (0, 10, 4), 'end'))\n"
    "published['bore_surface'] = [pub_face(f) for f in shape.faces() if f.geom_type == GeomType.CYLINDER\n"
    "                             and abs(BRepAdaptor_Surface(f.wrapped).Cylinder().Radius() - 10) < 1e-9]\n",
    flange_frames(50, 10, 30), ["bolt_seat", "bolt_seat.i7"],
    sweep_frames(["r_out", "t", "rb"], [[40, 50, 70], [6, 10, 14], [25, 30, 35]], flange_volume,
                 lambda r_out, t, rb: ([-r_out, -r_out, 0], [r_out, r_out, t]), flange_frames),
    published={"bore_surface": [{"type": "cylinder", "axis": [0, 0, 1], "axis_point_mm": [0, 0, 0],
                                 "radius_mm": 10, "span_mm": [0, 8.5]}]},
)


def pocket_volume(w, d, h, dp):
    return w * d * h - (w - 30) * (d - 20) * dp - PI * 9 * (h - dp) - 0.5 * 1 * 1 * (d - 20)


def pocket_frames(w, d, h, dp):
    return {
        "drain_seat": frame([w / 2, d / 2, h - dp], [0, 0, 1]),
        "pocket_floor": frame([w / 2, d / 2, h - dp], [0, 0, 1]),
        "origin": IDENTITY,
    }


emit_connectors(
    "con_sweep_pocket", "Pocketed plate with connectors through tags",
    "  param w = 80 mm range(60 mm, 120 mm)\n"
    "  param d = 50 mm range(40 mm, 80 mm)\n"
    "  param h = 20 mm range(14 mm, 30 mm)\n"
    "  param dp = 8 mm range(4 mm, 12 mm)\n\n"
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=w h=d\n  }\n"
    "  extrude plate from s.r distance=h\n\n"
    "  sketch cut on plate.cap_end {\n    rect r corner=(15 mm, 10 mm) w=w - 30 mm h=d - 20 mm\n  }\n"
    "  extrude pocket from cut.r distance=-dp op=subtract\n\n"
    "  tag floor_face = pocket.cap_end\n"
    "  tag rim = edge(plate.cap_end & pocket.side(cut.r.left))\n\n"
    "  hole drain on=floor_face at=(w / 2, d / 2) d=6 mm end=through_all\n"
    "  chamfer rim_chamfer edges=[rim] d=1 mm\n\n"
    "  connector drain_seat from=drain\n"
    "  connector pocket_floor origin=floor_face z=floor_face\n",
    "nam_pocket_tags with connectors: one from a hole drilled on a tagged face, one on the tagged pocket "
    "floor itself (its centroid, which the centred drain hole does not move). Every swept plate size and "
    "pocket depth keeps them on the floor.",
    pocket_volume(80, 50, 20, 8), [0, 0, 0], [80, 50, 20],
    "w, d, h, dp = 80, 50, 20, 8\n"
    "shape = Box(w, d, h, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "shape = shape - Pos(15 + (w - 30) / 2, 10 + (d - 20) / 2, h - dp / 2) * Box(w - 30, d - 20, dp)\n"
    "shape = shape - Pos(w / 2, d / 2, (h - dp) / 2) * Cylinder(3, h - dp)\n"
    "rim = shape.edges().filter_by(Axis.Y).filter_by_position(Axis.X, 14.99, 15.01).filter_by_position(Axis.Z, h - 0.01, h + 0.01)\n"
    "shape = chamfer(rim, 1)\n",
    "_o, _n = face_frame(planar_face(shape, [0, 0, 1], (40, 25, 12)))\n"
    "connectors['drain_seat'] = _frame(circle_centre(shape, 3, (40, 25, 12)), _n)\n"
    "connectors['pocket_floor'] = _frame(_o, _n)\n",
    pocket_frames(80, 50, 20, 8), [],
    sweep_frames(["w", "d", "h", "dp"], [[60, 80, 120], [40, 50, 80], [14, 20, 30], [4, 8, 12]],
                 pocket_volume, lambda w, d, h, dp: ([0, 0, 0], [w, d, h]), pocket_frames),
)


def cup_volume(w, d, h, tw):
    return w * d * h - (w - 2 * tw) * (d - 2 * tw) * (h - tw) - 2 * PI * 16 * tw


def cup_frames(w, d, h, tw):
    return {
        "port_face": frame([0, d / 2, h / 2], [-1, 0, 0]),
        "base_face": frame([w / 2, d / 2, 0], [0, 0, -1]),
        "origin": IDENTITY,
    }


emit_connectors(
    "con_sweep_shell", "Open box with connectors",
    "  param w = 80 mm range(60 mm, 100 mm)\n"
    "  param d = 50 mm range(40 mm, 70 mm)\n"
    "  param h = 40 mm range(30 mm, 50 mm)\n"
    "  param tw = 3 mm range(2 mm, 4 mm)\n\n"
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=w h=d\n  }\n"
    "  extrude body from s.r distance=h\n"
    "  shell cup faces=[body.cap_end] thickness=tw\n"
    "  hole port on=body.side(s.r.left) at=(-(d / 2), h / 2) d=8 mm end=through_all\n\n"
    "  connector port_face from=port\n"
    "  connector base_face origin=body.cap_start z=body.cap_start\n",
    "nam_shell_box with connectors: the cross hole's entry on the outer left wall (z = -X, x by the "
    "hole-frame rule) and the bottom face after the shell. Every swept size and wall thickness keeps them.",
    cup_volume(80, 50, 40, 3), [0, 0, 0], [80, 50, 40],
    "w, d, h, tw = 80, 50, 40, 3\n"
    "shape = Box(w, d, h, align=(Align.MIN, Align.MIN, Align.MIN))\n"
    "shape = shape - Pos(w / 2, d / 2, tw + (h - tw) / 2) * Box(w - 2 * tw, d - 2 * tw, h - tw)\n"
    "wall = Pos(w / 2, d / 2, h / 2) * (Rot(0, 90, 0) * Cylinder(4, w + 2))\n"
    "shape = shape - wall\n",
    "_o, _left = face_frame(planar_face(shape, [-1, 0, 0], (0, 25, 20)))\n"
    "connectors['port_face'] = _frame(circle_centre(shape, 4, (0, 25, 20)), _left)\n"
    "_o, _n = face_frame(planar_face(shape, [0, 0, -1], (40, 25, 0)))\n"
    "connectors['base_face'] = _frame(_o, _n)\n",
    cup_frames(80, 50, 40, 3), [],
    sweep_frames(["w", "d", "h", "tw"], [[60, 80, 100], [40, 50, 70], [30, 40, 50], [2, 3, 4]],
                 cup_volume, lambda w, d, h, tw: ([0, 0, 0], [w, d, h]), cup_frames),
)


def mirror_volume(w, c):
    return w * 30 * 12 - 2 * PI * 9 * 12 - 2 * 0.5 * c * c * w - 2 * _chamfer_ring(3, c)


def mirror_frames(w, c):
    return {
        "hole_seat.m1": frame([w / 4, 15, 12], [0, 0, 1]),
        "hole_seat.m2": frame([-w / 4, 15, 12], [0, 0, 1]),
        "origin": IDENTITY,
    }


emit_connectors(
    "con_sweep_mirror", "Symmetric plate with connectors on mirrored holes",
    "  param w = 90 mm range(60 mm, 140 mm)\n"
    "  param c = 2 mm range(1 mm, 3 mm)\n\n"
    "  sketch s on plane.XY {\n    rect r corner=(-w / 2, 0 mm) w=w h=30 mm\n  }\n"
    "  extrude plate from s.r distance=12 mm\n"
    "  hole h on=plate.cap_end at=(w / 4, 15 mm) d=6 mm end=through_all\n"
    "  mirror twin of=h plane=plane.YZ\n"
    "  chamfer edges_ch edges=[edge(plate.cap_end & plate.side(s.r.top)), edge(plate.cap_end & plate.side(s.r.bottom)), edge(plate.cap_end & h.wall), edge(plate.cap_end & twin.wall)] d=c\n\n"
    "  connector hole_seat from=twin\n",
    "nam_mirror_chamfer with connectors on the hole (.m1) and its mirror image (.m2), on the hole's "
    "entry plane under the chamfers. Every swept width and chamfer size moves them apart symmetrically.",
    mirror_volume(90, 2), [-45, 0, 0], [45, 30, 12],
    "w, c = 90, 2\n"
    "shape = Box(w, 30, 12, align=(Align.CENTER, Align.MIN, Align.MIN))\n"
    "for x in (w / 4, -w / 4):\n"
    "    shape = shape - Pos(x, 15, 6) * Cylinder(3, 12)\n"
    "top = [e for e in shape.edges().filter_by_position(Axis.Z, 11.99, 12.01)\n"
    "       if not (e.geom_type == GeomType.LINE and abs(e.start_point().X - e.end_point().X) < 1e-6)]\n"
    "shape = chamfer(top, c)\n",
    "_o, _top = face_frame(planar_face(shape, [0, 0, 1], (0, 15, 12)))\n"
    "connectors['hole_seat.m1'] = _frame(circle_centre(shape, 5, (22.5, 15, 12)), _top)\n"
    "connectors['hole_seat.m2'] = _frame(circle_centre(shape, 5, (-22.5, 15, 12)), _top)\n",
    mirror_frames(90, 2), ["hole_seat"],
    sweep_frames(["w", "c"], [[60, 90, 140], [1, 2, 3]], mirror_volume,
                 lambda w, c: ([-w / 2, 0, 0], [w / 2, 30, 12]), mirror_frames),
)


# 8. invalid connectors: each part must fail with exactly the one code -----------------------------

BASE = "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=40 mm h=30 mm\n  }\n  extrude base from s.r distance=20 mm\n"
emit_invalid("con_err_missing_z", "Connector without z",
             BASE + "  connector a origin=(0 mm, 0 mm, 0 mm)\n", "E5001",
             "origin= needs z= unless from= or csys= gives the whole frame.")
emit_invalid("con_err_conflict", "Connector with csys and origin",
             BASE + "  csys k origin=(0 mm, 0 mm, 0 mm) x=(1, 0, 0) y=(0, 1, 0)\n"
             "  connector a csys=k origin=(0 mm, 0 mm, 0 mm) z=+Z\n", "E5001",
             "csys= gives the whole frame; origin= and z= with it contradict it.")
emit_invalid("con_err_parallel", "Connector with x parallel to z",
             BASE + "  connector a origin=(0 mm, 0 mm, 0 mm) z=+Z x=(0, 0, 2)\n", "E6011",
             "x parallel to z leaves nothing to project: degenerate frame.")
emit_invalid("con_err_zero_dir", "Connector with a zero direction",
             BASE + "  connector a origin=(0 mm, 0 mm, 0 mm) z=(0, 0, 0)\n", "E6011",
             "A zero direction has no axis: degenerate frame.")
emit_invalid("con_err_not_hole", "Connector from a feature that is not a hole",
             BASE + "  connector a from=base\n", "E5002",
             "from= accepts a hole or a pattern of a hole; an extrude is the wrong kind of geometry.")
emit_invalid("con_err_unknown_ref", "Connector on a face that does not exist",
             BASE + "  connector a origin=base.cap_middle z=+Z\n", "E3001",
             "The reference does not resolve; the message names the nearest roles (cap_start, cap_end).")
emit_invalid("con_err_duplicate", "Connector named like a feature",
             BASE + "  connector base origin=(0 mm, 0 mm, 0 mm) z=+Z\n", "E2101",
             "Connectors share the part's name scope with parameters, sketches, features, datums and tags.")
emit_invalid("con_err_version", "Connector under a format 0.1 header",
             BASE + "  connector a origin=(0 mm, 0 mm, 0 mm) z=+Z\n", "E0103",
             "connector is a keyword of format 0.2 (spec/versioning.md); under a 0.1 header the statement "
             "is an error whose fix changes the header.", version="0.1")
emit_invalid("con_err_connector_ref", "Connector that refers to another connector",
             BASE + "  connector a origin=(0 mm, 0 mm, 0 mm) z=+Z\n  connector b origin=a z=+Z\n", "E5002",
             "Connectors refer to geometry, datums, points and holes, not to other connectors; share a "
             "frame through a csys.")
emit_invalid("pub_err_mixed", "Published faces and edges together",
             BASE + "  publish m = base.cap_end, edge(base.cap_end & base.side(s.r.bottom))\n", "E3005",
             "A published entity is faces or edges, not both.")
emit_invalid("pub_err_unknown_ref", "Publishing a face that does not exist",
             BASE + "  publish u = base.cap_middle\n", "E3001",
             "The reference does not resolve; the message names the nearest roles.")
emit_invalid("pub_err_chain", "Published edges that do not form a chain",
             BASE + "  publish c = edge(base.cap_end & base.side(s.r.bottom)), edge(base.cap_start & base.side(s.r.top))\n",
             "E5002", "Published edges must connect end to end in the order written.")
emit_invalid("pub_err_duplicate", "Published name that clashes with a feature",
             BASE + "  publish base = base.cap_end\n", "E2101",
             "Published names share the part's name scope.")
emit_invalid("pub_err_version", "publish under a format 0.1 header",
             BASE + "  publish top = base.cap_end\n", "E0103",
             "publish is a keyword of format 0.2.", version="0.1")
