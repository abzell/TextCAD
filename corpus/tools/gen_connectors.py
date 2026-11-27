#!/usr/bin/env python3
"""Corpus cases for connectors (spec/assembly.md § `connector`, draft; issue #28).

Writes `con_*` cases: three parts with geometry whose expected connector frames are computed here in
closed form and checked against frames that each case's independent build123d model derives from its
own geometry, and small invalid parts that must fail with exactly one diagnostic.

Usage: B123_PYTHON=/path/to/python-with-build123d python3 corpus/tools/gen_connectors.py
       (then bring the files into canonical form, spec/canonical-form.md)
"""
import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from corpuslib import *  # noqa: F401,F403

ORIGIN_TOL = 1e-5  # mm, tier T2 (spec/kernel.md)
AXIS_TOL = 1e-9


# --- closed-form frame helpers (the rules of spec/assembly.md § `connector`) -------------------------

def unit(v):
    n = math.sqrt(sum(c * c for c in v))
    return [c / n for c in v]


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def default_x(z):
    """x without `x=`: world Z cross z, or world X when z is parallel to world Z."""
    x = cross([0, 0, 1], z)
    return [1.0, 0.0, 0.0] if math.sqrt(sum(c * c for c in x)) < 1e-9 else unit(x)


def frame(origin, z, x=None, flip=False, rotate_deg=0.0, offset=(0, 0, 0)):
    """Base frame, then flip (180 deg about x), then rotate (about z, right-hand rule), then the
    offset in the resulting frame."""
    z = unit(z)
    if x is None:
        x = default_x(z)
    else:
        d = sum(a * b for a, b in zip(x, z))
        x = unit([a - d * b for a, b in zip(x, z)])
    y = cross(z, x)
    if flip:
        y, z = [-c for c in y], [-c for c in z]
    if rotate_deg:
        c, s = math.cos(math.radians(rotate_deg)), math.sin(math.radians(rotate_deg))
        x, y = [c * a + s * b for a, b in zip(x, y)], [-s * a + c * b for a, b in zip(x, y)]
    o = [origin[i] + offset[0] * x[i] + offset[1] * y[i] + offset[2] * z[i] for i in range(3)]
    return {"origin_mm": o, "x": x, "y": y, "z": z}


def clean(f):
    """Rounds away floating-point noise so that the expected values read as the exact values they are."""
    return {k: [0.0 if abs(c) < 1e-15 else round(c, 15) for c in v] for k, v in f.items()}


IDENTITY = {"origin_mm": [0.0, 0.0, 0.0], "x": [1.0, 0.0, 0.0], "y": [0.0, 1.0, 0.0], "z": [0.0, 0.0, 1.0]}


# --- independent frames from the build123d geometry (copied into each reference.py) -----------------

REF_FRAMES = '''
from OCP.BRepAdaptor import BRepAdaptor_Surface


def _unit(v):
    n = (v[0] ** 2 + v[1] ** 2 + v[2] ** 2) ** 0.5
    return [c / n for c in v]


def _cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def _dot(a, b):
    return sum(p * q for p, q in zip(a, b))


def _orient(d):
    """An axis of revolution has no sign: make its first non-zero component positive."""
    for c in d:
        if abs(c) > 1e-9:
            return d if c > 0 else [-v for v in d]
    return d


def _frame(o, z, x=None, flip=False, rotate=0.0, offset=(0, 0, 0)):
    z = _unit(z)
    if x is None:
        x = _cross([0, 0, 1], z)
        x = [1.0, 0.0, 0.0] if _dot(x, x) < 1e-18 else _unit(x)
    else:
        x = _unit([a - _dot(x, z) * b for a, b in zip(x, z)])
    y = _cross(z, x)
    if flip:
        y, z = [-c for c in y], [-c for c in z]
    if rotate:
        c, s = cos(radians(rotate)), sin(radians(rotate))
        x, y = [c * a + s * b for a, b in zip(x, y)], [-s * a + c * b for a, b in zip(x, y)]
    o = [o[i] + offset[0] * x[i] + offset[1] * y[i] + offset[2] * z[i] for i in range(3)]
    return {"origin_mm": o, "x": x, "y": y, "z": z}


def planar_face(shape, normal, near):
    """The planar face with this outward normal whose centroid is nearest to `near`."""
    best = None
    for f in shape.faces():
        if f.geom_type != GeomType.PLANE:
            continue
        n = f.normal_at(f.center())
        if _dot([n.X, n.Y, n.Z], normal) < 1 - 1e-9:
            continue
        c = f.center(CenterOf.MASS)
        d = (c - Vector(*near)).length
        if best is None or d < best[0]:
            best = (d, f)
    return best[1]


def face_frame(face):
    """Centroid and outward normal of a planar face."""
    c = face.center(CenterOf.MASS)
    n = face.normal_at(c)
    return [c.X, c.Y, c.Z], [n.X, n.Y, n.Z]


def circle_centre(shape, radius, near):
    """Centre of the circular edge of this radius nearest to `near`."""
    best = None
    for e in shape.edges():
        if e.geom_type != GeomType.CIRCLE or abs(e.radius - radius) > 1e-9:
            continue
        c = e.arc_center
        d = (c - Vector(*near)).length
        if best is None or d < best[0]:
            best = (d, c)
    return [best[1].X, best[1].Y, best[1].Z]


def cylinder_axis(shape, radius, near, at="start"):
    """Oriented axis of the cylindrical face of this radius nearest to `near`, and the point on it at
    the face's start, middle or end along that axis."""
    best = None
    for f in shape.faces():
        if f.geom_type != GeomType.CYLINDER:
            continue
        cyl = BRepAdaptor_Surface(f.wrapped).Cylinder()
        if abs(cyl.Radius() - radius) > 1e-9:
            continue
        d = (f.center(CenterOf.MASS) - Vector(*near)).length
        if best is None or d < best[0]:
            best = (d, f, cyl)
    _, f, cyl = best
    ax = cyl.Axis()
    p = [ax.Location().X(), ax.Location().Y(), ax.Location().Z()]
    z = _orient([ax.Direction().X(), ax.Direction().Y(), ax.Direction().Z()])
    ts = [_dot([v.X - p[0], v.Y - p[1], v.Z - p[2]], z) for v in f.vertices()]
    t = {"start": min(ts), "end": max(ts), "middle": (min(ts) + max(ts)) / 2}[at]
    return [p[i] + t * z[i] for i in range(3)], z


def cylinder_axis_all(shape, radius, axis_point, axis_dir, at="start"):
    """Like cylinder_axis, over every cylindrical face of this radius on the given axis (the pieces of a
    split face)."""
    z = _orient(_unit(axis_dir))
    ts = []
    for f in shape.faces():
        if f.geom_type != GeomType.CYLINDER:
            continue
        cyl = BRepAdaptor_Surface(f.wrapped).Cylinder()
        if abs(cyl.Radius() - radius) > 1e-9:
            continue
        ax = cyl.Axis()
        q = [ax.Location().X() - axis_point[0], ax.Location().Y() - axis_point[1], ax.Location().Z() - axis_point[2]]
        d = [ax.Direction().X(), ax.Direction().Y(), ax.Direction().Z()]
        if abs(abs(_dot(d, z)) - 1) > 1e-9 or _dot(_cross(q, z), _cross(q, z)) > 1e-12:
            continue
        ts += [_dot([v.X - axis_point[0], v.Y - axis_point[1], v.Z - axis_point[2]], z) for v in f.vertices()]
    t = {"start": min(ts), "end": max(ts), "middle": (min(ts) + max(ts)) / 2}[at]
    return [axis_point[i] + t * z[i] for i in range(3)], z

'''


def ref_tail(frames_code):
    """reference.py ending: the usual properties plus the connector frames derived from the geometry."""
    return (
        REF_TAIL.replace("print(json.dumps({\n", "connectors = {}\n" + frames_code + "print(json.dumps({\n", 1)
        .replace("\n}))\n", '\n    "connectors": connectors,\n}))\n', 1)
    )


def check_frames(name, expected, got):
    for key, f in expected.items():
        g = got.get(key)
        if g is None:
            sys.exit(f"{name}: build123d model has no frame for {key}")
        for a, b in zip(f["origin_mm"], g["origin_mm"]):
            if abs(a - b) > ORIGIN_TOL:
                sys.exit(f"{name}: {key} origin {f['origin_mm']} disagrees with build123d {g['origin_mm']}")
        for axis in "xyz":
            for a, b in zip(f[axis], g[axis]):
                if abs(a - b) > AXIS_TOL:
                    sys.exit(f"{name}: {key}.{axis} {f[axis]} disagrees with build123d {g[axis]}")


def emit_connectors(name, title, body, note, volume, lo, hi, ref, frames_code, connectors, absent,
                    sweeps=None):
    code = REF_HEAD + REF_FRAMES + ref + "\n" + ref_tail(frames_code)
    out = subprocess.run([PY, "-c", code], capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"{name}: build123d failed:\n{out.stderr}")
    got = json.loads(out.stdout.strip().splitlines()[-1])
    if abs(got["volume_mm3"] - volume) > 1e-6 * volume:
        sys.exit(f"{name}: closed-form volume {volume} disagrees with build123d {got['volume_mm3']}")
    for a, b in zip(got["bbox_min_mm"] + got["bbox_max_mm"], list(lo) + list(hi)):
        if abs(a - b) > 1e-5:
            sys.exit(f"{name}: closed-form bounding box disagrees with build123d")
    explicit = {k: v for k, v in connectors.items() if k != "origin"}
    check_frames(name, explicit, got["connectors"])
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
        "baseline": ["area_mm2", "center_of_mass_mm"],
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

# 5. invalid connectors: each part must fail with exactly the one code -----------------------------

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
