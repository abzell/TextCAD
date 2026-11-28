#!/usr/bin/env python3
"""Corpus cases for the classic mates and the solving rules (spec/assembly.md § Classic mates,
§ Solving; issue #33).

Every case is a bundle like the other `asm_*` cases. The expected poses are constructed here in
closed form, case by case (the frames the mates make coincide; the nearest solution where some freedom
is left), and then checked independently of that construction:

- every mate equation of spec/assembly.md § Classic mates holds at the expected poses (a residual
  per pair, written from the table);
- the remaining degrees of freedom (total, and translations and rotations per instance) are the rank
  figures of a numerical Jacobian of those residuals, as § Degrees of freedom defines them;
- for an under-constrained mate, the expected pose moves and turns the block the least: random
  solutions nearby (start poses projected onto the mate) never move it less *and* turn it less.

Usage: python3 corpus/tools/gen_mates.py   (needs numpy; no build123d)
"""
import json
import math
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
from gen_assemblies import BOX, asm, bundle  # noqa: E402
from framelib import clean  # noqa: E402

HYD = os.path.join(os.path.dirname(__file__), "hydraulic")

# --- poses and frames ----------------------------------------------------------------------------
# A pose is (R, t): a point p of the file is at R p + t. A frame is (o, x, y, z) as numpy vectors.


def rot(axis, deg):
    a = np.asarray(axis, float)
    a = a / np.linalg.norm(a)
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    k = np.array([[0, -a[2], a[1]], [a[2], 0, -a[0]], [-a[1], a[0], 0]])
    return np.eye(3) + s * k + (1 - c) * (k @ k)


def placement(rotate=(), at=(0, 0, 0)):
    r = np.eye(3)
    for axis, deg in rotate:
        r = rot(axis, deg) @ r
    return r, np.asarray(at, float)


def default_x(z):
    x = np.cross([0.0, 0.0, 1.0], z)
    return np.array([1.0, 0.0, 0.0]) if np.linalg.norm(x) < 1e-9 else x / np.linalg.norm(x)


def frame(o, z, x=None):
    z = np.asarray(z, float) / np.linalg.norm(z)
    x = default_x(z) if x is None else np.asarray(x, float)
    return (np.asarray(o, float), x, np.cross(z, x), z)


def place(pose, f):
    r, t = pose
    return (r @ f[0] + t, r @ f[1], r @ f[2], r @ f[3])


def frame_matrix(f):
    """4x4 matrix of a frame (columns x, y, z, origin)."""
    m = np.eye(4)
    m[:3, 0], m[:3, 1], m[:3, 2], m[:3, 3] = f[1], f[2], f[3], f[0]
    return m


def pose_for(local, world):
    """The pose that puts the file's frame `local` at `world`."""
    m = frame_matrix(world) @ np.linalg.inv(frame_matrix(local))
    return m[:3, :3], m[:3, 3]


def geodesic(u, v):
    """The least rotation taking unit vector u to unit vector v."""
    axis = np.cross(u, v)
    s, c = np.linalg.norm(axis), float(np.dot(u, v))
    if s < 1e-15:
        if c > 0:
            return np.eye(3)
        sys.exit("geodesic: opposite vectors have no unique least rotation; change the case")
    return rot(axis, math.degrees(math.atan2(s, c)))


def as_json(pose):
    r, t = pose
    return clean({"origin_mm": list(map(float, t)), "x": list(map(float, r[:, 0])),
                  "y": list(map(float, r[:, 1])), "z": list(map(float, r[:, 2]))})


# --- the parts ---------------------------------------------------------------------------------

SLAB = """tcad 0.2

part slab "Slab, 40 x 30 x 10" {
  sketch s on plane.XY {
    rect r corner=(-20 mm, -15 mm) w=40 mm h=30 mm
  }
  extrude body from s.r distance=10 mm

  connector top origin=body.cap_end z=body.cap_end
  connector bottom origin=body.cap_start z=body.cap_start
  connector east origin=body.side(s.r.right) z=body.side(s.r.right)
  connector north origin=body.side(s.r.top) z=body.side(s.r.top)
}
"""

SLAB_FRAMES = {
    "origin": frame([0, 0, 0], [0, 0, 1]),
    "top": frame([0, 0, 10], [0, 0, 1]),
    "bottom": frame([0, 0, 0], [0, 0, -1]),
    "east": frame([20, 0, 5], [1, 0, 0]),
    "north": frame([0, 15, 5], [0, 1, 0]),
}

KNOB = """tcad 0.2

part knob "Knob" {
  sketch s on plane.XY {
    circle c center=(0 mm, 0 mm) r=4 mm
  }
  extrude body from s.c distance=30 mm

  connector tip origin=body.cap_end z=body.cap_end
}
"""

KNOB_FRAMES = {"origin": frame([0, 0, 0], [0, 0, 1]), "tip": frame([0, 0, 30], [0, 0, 1])}


def hyd(name):
    with open(os.path.join(HYD, name)) as f:
        return f.read()


# The hydraulic parts' frames (corpus/hyd_*/expected.json), for the overrides used below: the
# clevis's pin bore is at x = body_len - 20 mm whatever its diameter; the grease pin's axis frame is at
# half its length.
CLEVIS_FRAMES = {
    "origin": frame([0, 0, 0], [0, 0, 1]),
    "pin": frame([50, 0, 0], [0, 1, 0]),
    "rod": frame([0, 0, 0], [-1, 0, 0]),
}
PIN_LEN = 100.0
PIN_FRAMES = {
    "origin": frame([0, 0, 0], [0, 0, 1]),
    "pin_axis": frame([0, 0, PIN_LEN / 2], [0, 0, 1]),
}
EYE_FRAMES = {
    "origin": frame([0, 0, 0], [0, 0, 1]),
    "bearing": frame([0, 0, 0], [0, 1, 0]),
    "rod": frame([50, 0, 0], [1, 0, 0]),
}

# --- primitives and the mate equations (spec/assembly.md § Classic mates) ---------------------------

DEFAULT = {"fastened": None, "coincident": "xy", "concentric": "z", "parallel": "z",
           "perpendicular": "z", "distance": "xy", "angle": "z"}


def primitive(f, suffix):
    """(kind, point, direction) of a frame's derived primitive; a frame is ("frame", f)."""
    o, x, y, z = f
    if suffix is None:
        return ("frame", f)
    if suffix == "origin":
        return ("point", o, None)
    if suffix in ("x", "y", "z"):
        return ("line", o, {"x": x, "y": y, "z": z}[suffix])
    return ("plane", o, {"xy": z, "yz": x, "xz": y}[suffix])


def residual(kind, a, b, value=None, flags=(), about=None, rotate=0.0, offset=(0, 0, 0)):  # noqa: C901
    """The mate's equations at the current poses, as a vector that is zero exactly when it holds."""
    sign = -1.0 if "opposed" in flags else 1.0
    if kind == "fastened":
        fb = b[1]
        o = fb[0] + offset[0] * fb[1] + offset[1] * fb[2] + offset[2] * fb[3]
        c, s = math.cos(math.radians(rotate)), math.sin(math.radians(rotate))
        x = c * fb[1] + s * fb[2]
        fa = a[1]
        return np.concatenate([fa[0] - o, np.cross(fa[1], x), np.cross(fa[3], fb[3]),
                               [np.dot(fa[1], x) - 1, np.dot(fa[3], fb[3]) - 1]])
    ka, kb = a[0], b[0]
    pair = (ka, kb)

    def on_line(p, line):
        return np.cross(p - line[1], line[2])

    def on_plane(p, plane):
        return [np.dot(p - plane[1], plane[2])]

    if kind in ("coincident", "concentric"):
        if pair == ("point", "point"):
            return a[1] - b[1]
        if pair == ("point", "line"):
            return on_line(a[1], b)
        if pair == ("line", "point"):
            return on_line(b[1], a)
        if pair == ("point", "plane"):
            return on_plane(a[1], b)
        if pair == ("plane", "point"):
            return on_plane(b[1], a)
        if pair == ("line", "line"):
            return np.concatenate([a[2] - sign * b[2], on_line(a[1], b)])
        if pair in (("line", "plane"), ("plane", "line")):
            ln, pl = (a, b) if ka == "line" else (b, a)
            return np.concatenate([[np.dot(ln[2], pl[2])], on_plane(ln[1], pl)])
        if pair == ("plane", "plane"):
            sign = 1.0 if "aligned" in flags else -1.0
            return np.concatenate([a[2] - sign * b[2], on_plane(a[1], b)])
    if kind == "parallel":
        if pair in (("line", "line"), ("plane", "plane")):
            return a[2] - sign * b[2]
        ln, pl = (a, b) if ka == "line" else (b, a)
        return [np.dot(ln[2], pl[2])]
    if kind == "perpendicular":
        if pair in (("line", "line"), ("plane", "plane")):
            return [np.dot(a[2], b[2])]
        ln, pl = (a, b) if ka == "line" else (b, a)
        return np.cross(ln[2], pl[2])
    if kind == "distance":
        if pair == ("point", "point"):
            return [np.linalg.norm(a[1] - b[1]) - value]
        if pair in (("point", "line"), ("line", "point")):
            p, ln = (a, b) if ka == "point" else (b, a)
            return [np.linalg.norm(on_line(p[1], ln)) - value]
        if pair in (("point", "plane"), ("plane", "point")):
            p, pl = (a, b) if ka == "point" else (b, a)
            return [np.dot(p[1] - pl[1], pl[2]) - value]
        if pair == ("plane", "plane"):
            sign = 1.0 if "aligned" in flags else -1.0
            return np.concatenate([a[2] - sign * b[2], [np.dot(a[1] - b[1], b[2]) - value]])
    if kind == "angle":
        w = about[2]
        u = a[2] - np.dot(a[2], w) * w
        v = b[2] - np.dot(b[2], w) * w
        theta = math.atan2(np.dot(w, np.cross(v, u)), np.dot(v, u))
        d = (theta - math.radians(value) + math.pi) % (2 * math.pi) - math.pi
        return [d]
    sys.exit(f"no equation for {kind} {pair}")


# --- an assembly: instances, mates, the check of a solution ---------------------------------------


class Asm:
    def __init__(self):
        self.inst = {}      # name -> (frames, pose, grounded)
        self.mates = []     # (name, kind, ref a, ref b, kwargs)

    def add(self, name, frames, pose, grounded=False):
        self.inst[name] = [frames, pose, grounded]

    def ref(self, poses, text, kind):
        parts = text.split(".")
        name, conn = parts[0], parts[1]
        suffix = parts[2] if len(parts) > 2 else DEFAULT[kind]
        frames = self.inst[name][0]
        return primitive(place(poses[name], frames[conn]), suffix)

    def mate(self, name, kind, a, b, **kw):
        self.mates.append((name, kind, a, b, kw))

    def residuals(self, poses, only=None):
        out = []
        for name, kind, a, b, kw in self.mates:
            if only is not None and name not in only:
                continue
            kw = dict(kw)
            if "about" in kw:
                kw["about"] = self.ref(poses, kw["about"], "angle")
            out.append(np.atleast_1d(residual(kind, self.ref(poses, a, kind), self.ref(poses, b, kind), **kw)))
        return np.concatenate(out) if out else np.zeros(0)

    def check(self, case, poses):
        r = self.residuals(poses)
        if r.size and np.max(np.abs(r)) > 1e-12:
            sys.exit(f"{case}: a mate does not hold at the expected poses: {r}")

    def freedom(self, poses):
        """Degrees of freedom by § Degrees of freedom: the null space of the Jacobian of all mate
        equations with respect to (translation of the origin, rotation) of every free body."""
        free = [n for n, v in self.inst.items() if not v[2]]
        mated = {m[2].split(".")[0] for m in self.mates} | {m[3].split(".")[0] for m in self.mates}

        def moved(x):
            p = dict(poses)
            for i, n in enumerate(free):
                r, t = poses[n]
                w = x[6 * i + 3:6 * i + 6]
                ang = np.linalg.norm(w)
                q = rot(w, math.degrees(ang)) if ang > 0 else np.eye(3)
                # turn about the body's frame origin (its translation t), then move it
                p[n] = (q @ r, t + x[6 * i:6 * i + 3])
            return p

        n = 6 * len(free)
        h = 1e-6
        cols = []
        for k in range(n):
            e = np.zeros(n)
            e[k] = h
            cols.append((self.residuals(moved(e)) - self.residuals(moved(-e))) / (2 * h))
        jac = np.array(cols).T
        _, s, vt = np.linalg.svd(jac)
        rank = int(np.sum(s > 1e-7))
        null = vt[rank:].T  # columns span the null space
        per = {}
        for i, nm in enumerate(free):
            if nm not in mated:
                continue
            block = null[6 * i:6 * i + 6]
            dof = np.linalg.matrix_rank(block, tol=1e-7) if block.size else 0
            rots = np.linalg.matrix_rank(block[3:], tol=1e-7) if block.size else 0
            if dof:
                per[nm] = {"translations": int(dof - rots), "rotations": int(rots)}
        return n - rank, per


def instances_json(a, poses, files, configs=None, nested=None):
    out = {}
    for name in a.inst:
        out[name] = {"file": files[name], "config": (configs or {}).get(name), **as_json(poses[name])}
    for path, (file, pose) in (nested or {}).items():
        out[path] = {"file": file, "config": None, **as_json(pose)}
    return out


# --- 1. asm_pin_clevis ---------------------------------------------------------------------------

PIN_CLEVIS = """tcad 0.2

assembly asm_pin_clevis "Rod clevis, pin and rod eye" {
  param tilt = 150 deg

  instance clevis = part("../parts/rod_clevis.tcad") { pin_dia = 32 mm }
  instance pin = part("../parts/grease_pin.tcad") at=(45 mm, 5 mm, 3 mm) rotate=[(+X, -80 deg)] {
    pin_d = 32 mm
    pin_len = 100 mm
  }
  instance eye = part("../parts/rod_eye.tcad") at=(60 mm, 2 mm, -20 mm) rotate=[(+Y, -20 deg)]

  ground clevis

  mate pin_seat fastened(pin.pin_axis, clevis.pin) rotate=90 deg
  mate eye_on concentric(eye.bearing, pin.pin_axis)
  mate eye_mid coincident(eye.bearing.xy, pin.pin_axis.xy) aligned
  mate eye_tilt angle(eye.rod, clevis.rod) = tilt about=clevis.pin
}
"""


def pin_clevis():
    a = Asm()
    a.add("clevis", CLEVIS_FRAMES, placement(), grounded=True)
    a.add("pin", PIN_FRAMES, placement([((1, 0, 0), -80)], (45, 5, 3)))
    a.add("eye", EYE_FRAMES, placement([((0, 1, 0), -20)], (60, 2, -20)))
    a.mate("pin_seat", "fastened", "pin.pin_axis", "clevis.pin", rotate=90.0)
    a.mate("eye_on", "concentric", "eye.bearing", "pin.pin_axis")
    a.mate("eye_mid", "coincident", "eye.bearing.xy", "pin.pin_axis.xy", flags=("aligned",))
    a.mate("eye_tilt", "angle", "eye.rod", "clevis.rod", value=150.0, about="clevis.pin")
    poses = {n: v[1] for n, v in a.inst.items()}
    # the pin: its axis frame is the clevis's pin frame turned 90 deg about its z
    cp = CLEVIS_FRAMES["pin"]
    target = (cp[0], math.cos(math.pi / 2) * cp[1] + math.sin(math.pi / 2) * cp[2],
              None, cp[3])
    target = (target[0], target[1], np.cross(target[3], target[1]), target[3])
    poses["pin"] = pose_for(PIN_FRAMES["pin_axis"], target)
    # the eye: its bearing frame on the pin's axis frame's origin with the same z (concentric and the
    # aligned planes), turned about that z so that the shank makes `tilt` with the clevis's rod axis.
    # The eye's bearing z is its part's y, and the clevis's pin z is world y: the eye turns about y by
    # phi, its rod axis (part x) becomes (cos phi, 0, -sin phi), and the angle from the clevis's rod
    # axis (-x) to it about +y is phi - 180 deg.
    phi = 150.0 + 180.0
    r = rot((0, 1, 0), phi)
    poses["eye"] = (r, place(poses["pin"], PIN_FRAMES["pin_axis"])[0] - r @ EYE_FRAMES["bearing"][0])
    a.check("asm_pin_clevis", poses)
    dof, per = a.freedom(poses)
    assert dof == 0 and not per, (dof, per)
    files = {"clevis": "parts/rod_clevis.tcad", "pin": "parts/grease_pin.tcad", "eye": "parts/rod_eye.tcad"}
    expected = {
        "note": "The rod clevis, a grease pin and a rod eye from the hydraulic parts (with overrides that make "
                "them fit). The pin is fastened in the clevis's bore, turned 90 deg; the eye turns on the pin "
                "(concentric, its mid-plane on the pin's) and is set by an angle about the bore axis, from a "
                "parameter. Fully constrained: the start poses do not matter.",
        "codes": [],
        "fails": False,
        "dof": 0,
        "instances": instances_json(a, poses, files),
    }
    bundle("asm_pin_clevis",
           {"parts/rod_clevis.tcad": hyd("rod_clevis.tcad"), "parts/grease_pin.tcad": hyd("grease_pin.tcad"),
            "parts/rod_eye.tcad": hyd("rod_eye.tcad"), "assemblies/asm_pin_clevis.tcad": PIN_CLEVIS},
           "assemblies/asm_pin_clevis.tcad", expected)


# --- 2. asm_plate_stack ----------------------------------------------------------------------------

CAPPED = """tcad 0.2

assembly capped "Slab with a knob" {
  instance plate = part("../parts/slab.tcad")
  instance knob = part("../parts/knob.tcad") at=(3 mm, -2 mm, 14 mm) rotate=[(+X, 10 deg)]

  ground plate

  mate knob_seat fastened(knob.origin, plate.top)
}
"""

PLATE_STACK = """tcad 0.2

assembly asm_plate_stack "Three slabs and a capped slab, stacked" {
  param gap = 4 mm

  instance base = part("../parts/slab.tcad")
  instance mid = part("../parts/slab.tcad") at=(3 mm, -4 mm, 12 mm) rotate=[(+Z, 5 deg)]
  instance top = part("../parts/slab.tcad") at=(-2 mm, 1 mm, 23 mm) rotate=[(+Z, 25 deg), (+X, 3 deg)]
  instance cap = assembly("capped.tcad") at=(0 mm, 0 mm, 35 mm) rotate=[(+Z, -80 deg)]

  ground base

  mate mid_on coincident(mid.bottom, base.top)
  mate mid_east distance(mid.east, base.east) = gap aligned
  mate mid_north distance(mid.north.origin, base.north) = -5 mm
  mate top_on coincident(top.bottom, mid.top)
  mate top_axis concentric(top.origin, mid.origin)
  mate top_turn angle(top.east, mid.east) = 30 deg about=mid.top
  mate cap_on coincident(cap.plate.bottom, top.top)
  mate cap_axis concentric(cap.knob.origin, top.origin)
  mate cap_turn angle(cap.plate.north, top.north) = -90 deg about=top.top
}
"""


def plate_stack():
    a = Asm()
    a.add("base", SLAB_FRAMES, placement(), grounded=True)
    a.add("mid", SLAB_FRAMES, placement([((0, 0, 1), 5)], (3, -4, 12)))
    a.add("top", SLAB_FRAMES, placement([((0, 0, 1), 25), ((1, 0, 0), 3)], (-2, 1, 23)))
    # the capped sub-assembly, solved on its own: the knob's origin frame on the plate's top frame
    knob_in_cap = pose_for(KNOB_FRAMES["origin"], SLAB_FRAMES["top"])
    cap_frames = {"plate.bottom": SLAB_FRAMES["bottom"], "plate.north": SLAB_FRAMES["north"],
                  "knob.origin": place(knob_in_cap, KNOB_FRAMES["origin"])}
    a.add("cap", {k.replace(".", "_"): v for k, v in cap_frames.items()}, placement([((0, 0, 1), -80)], (0, 0, 35)))
    a.mate("mid_on", "coincident", "mid.bottom", "base.top")
    a.mate("mid_east", "distance", "mid.east", "base.east", value=4.0, flags=("aligned",))
    a.mate("mid_north", "distance", "mid.north.origin", "base.north", value=-5.0)
    a.mate("top_on", "coincident", "top.bottom", "mid.top")
    a.mate("top_axis", "concentric", "top.origin", "mid.origin")
    a.mate("top_turn", "angle", "top.east", "mid.east", value=30.0, about="mid.top")
    a.mate("cap_on", "coincident", "cap.plate_bottom", "top.top")
    a.mate("cap_axis", "concentric", "cap.knob_origin", "top.origin")
    a.mate("cap_turn", "angle", "cap.plate_north", "top.north", value=-90.0, about="top.top")
    poses = {n: v[1] for n, v in a.inst.items()}
    # mid: on the base's top face, east faces 4 mm apart (+x), north face centre 5 mm behind the base's
    poses["mid"] = (np.eye(3), np.array([4.0, -5.0, 10.0]))
    # top: on mid's top, centred on mid's axis, turned 30 deg about +z
    poses["top"] = (rot((0, 0, 1), 30), np.array([4.0, -5.0, 20.0]))
    # cap: its plate on top's top, its knob on top's axis, its north normal -90 deg from top's
    poses["cap"] = (rot((0, 0, 1), 30 - 90), np.array([4.0, -5.0, 30.0]))
    a.check("asm_plate_stack", poses)
    dof, per = a.freedom(poses)
    assert dof == 0 and not per, (dof, per)
    files = {"base": "parts/slab.tcad", "mid": "parts/slab.tcad", "top": "parts/slab.tcad",
             "cap": "assemblies/capped.tcad"}
    nested = {
        "cap.plate": ("parts/slab.tcad", poses["cap"]),
        "cap.knob": ("parts/knob.tcad", (poses["cap"][0] @ knob_in_cap[0],
                                         poses["cap"][0] @ knob_in_cap[1] + poses["cap"][1])),
    }
    expected = {
        "note": "Slabs stacked by coincident faces, a signed distance between aligned planes (from a "
                "parameter), a signed point-to-plane distance, a concentric axis and angles about a face "
                "normal; on top, a rigid sub-assembly whose own fastened mate is solved first (its knob starts "
                "off its seat), mated through nested references. Fully constrained.",
        "codes": [],
        "fails": False,
        "dof": 0,
        "instances": instances_json(a, poses, files, nested=nested),
    }
    bundle("asm_plate_stack",
           {"parts/slab.tcad": SLAB, "parts/knob.tcad": KNOB, "assemblies/capped.tcad": CAPPED,
            "assemblies/asm_plate_stack.tcad": PLATE_STACK},
           "assemblies/asm_plate_stack.tcad", expected)


# --- 3. asm_mate_pairs: every pair of every classic kind, one block each -----------------------------
# Each block is mated to the grounded base by one mate whose block-side primitive passes through the
# block's frame origin, so what the mate fixes of its position and of its turn are independent: the
# nearest solution moves the origin to the nearest point that the mate allows and turns the block by the
# least rotation that the mate allows, and no other solution moves it less or turns it less.

PAIRS = [
    # name, mate text (kind, a, b, rest), kwargs, start (rotate, at), solve(start) -> pose
    ("fast", "fastened(fast.origin, base.top) rotate=30 deg offset=(5 mm, 0 mm, 2 mm)", "fastened",
     "fast.origin", "base.top", {"rotate": 30.0, "offset": (5, 0, 2)}, ([((0, 0, 1), 12)], (60, 0, 0))),
    ("pt_pt", "coincident(pt_pt.origin.origin, base.top.origin)", "coincident", "pt_pt.origin.origin",
     "base.top.origin", {}, ([((1, 0, 0), 20)], (0, 70, 40))),
    ("pt_line", "coincident(pt_line.origin.origin, base.east.z)", "coincident", "pt_line.origin.origin",
     "base.east.z", {}, ([((0, 1, 0), 15)], (90, 8, -6))),
    ("line_pt", "coincident(base.east.z, line_pt.origin.origin)", "coincident", "base.east.z",
     "line_pt.origin.origin", {}, ([((0, 0, 1), 40)], (70, -3, 9))),
    ("pt_plane", "coincident(pt_plane.origin.origin, base.top.xy)", "coincident", "pt_plane.origin.origin",
     "base.top.xy", {}, ([((1, 1, 0), 30)], (-60, 20, 35))),
    ("line_line", "coincident(line_line.origin.z, base.east.z)", "coincident", "line_line.origin.z",
     "base.east.z", {}, ([((0, 1, 0), 75)], (100, 6, 12))),
    ("line_line_op", "coincident(line_line_op.origin.z, base.north.z) opposed", "coincident",
     "line_line_op.origin.z", "base.north.z", {"flags": ("opposed",)}, ([((1, 0, 0), 100)], (5, 80, 2))),
    ("line_plane", "coincident(line_plane.origin.x, base.top.xy)", "coincident", "line_plane.origin.x",
     "base.top.xy", {}, ([((0, 1, 0), -25)], (-40, -50, 30))),
    ("plane_plane", "coincident(plane_plane.origin, base.top)", "coincident", "plane_plane.origin",
     "base.top", {}, ([((1, 0, 0), 170)], (30, -60, 25))),
    ("plane_plane_al", "coincident(plane_plane_al.origin, base.top) aligned", "coincident",
     "plane_plane_al.origin", "base.top", {"flags": ("aligned",)}, ([((1, 0, 0), 15)], (-30, -60, 25))),
    ("conc", "concentric(conc.origin, base.north)", "concentric", "conc.origin", "base.north", {},
     ([((0, 0, 1), -70), ((1, 0, 0), -80)], (-8, 60, 15))),
    ("par_line", "parallel(par_line.origin.x, base.east)", "parallel", "par_line.origin.x", "base.east", {},
     ([((0, 0, 1), 20)], (0, -90, 0))),
    ("par_plane", "parallel(par_plane.origin.xy, base.top.xy) opposed", "parallel", "par_plane.origin.xy",
     "base.top.xy", {"flags": ("opposed",)}, ([((0, 1, 0), 160)], (50, -90, 0))),
    ("par_mixed", "parallel(par_mixed.origin.z, base.top.xy)", "parallel", "par_mixed.origin.z", "base.top.xy",
     {}, ([((1, 0, 0), 70)], (-50, -90, 0))),
    ("perp_line", "perpendicular(perp_line.origin, base.top)", "perpendicular", "perp_line.origin",
     "base.top", {}, ([((1, 0, 0), 60)], (0, 120, 0))),
    ("perp_plane", "perpendicular(perp_plane.origin.xy, base.east.xy)", "perpendicular",
     "perp_plane.origin.xy", "base.east.xy", {}, ([((0, 1, 0), 30)], (50, 120, 0))),
    ("perp_mixed", "perpendicular(perp_mixed.origin.z, base.east.xy)", "perpendicular",
     "perp_mixed.origin.z", "base.east.xy", {}, ([((0, 1, 0), -110)], (-50, 120, 0))),
    ("dist_pt", "distance(dist_pt.origin.origin, base.top.origin) = 25 mm", "distance",
     "dist_pt.origin.origin", "base.top.origin", {"value": 25.0}, ([((0, 0, 1), 10)], (10, -20, 40))),
    ("dist_line", "distance(dist_line.origin.origin, base.east.z) = 12 mm", "distance",
     "dist_line.origin.origin", "base.east.z", {"value": 12.0}, ([((0, 0, 1), -10)], (80, 30, 45))),
    ("dist_plane", "distance(dist_plane.origin.origin, base.top.xy) = 8 mm", "distance",
     "dist_plane.origin.origin", "base.top.xy", {"value": 8.0}, ([((1, 0, 0), 5)], (-90, 0, 30))),
    ("dist_plane_pt", "distance(base.top.xy, dist_plane_pt.origin.origin) = -8 mm", "distance", "base.top.xy",
     "dist_plane_pt.origin.origin", {"value": -8.0}, ([((1, 0, 0), -5)], (-90, 40, -10))),
    ("dist_planes", "distance(dist_planes.origin, base.top) = 5 mm", "distance", "dist_planes.origin",
     "base.top", {"value": 5.0}, ([((0, 1, 0), 172)], (-120, 0, 30))),
    ("dist_planes_al", "distance(dist_planes_al.origin, base.top) = -3 mm aligned", "distance",
     "dist_planes_al.origin", "base.top", {"value": -3.0, "flags": ("aligned",)},
     ([((0, 1, 0), 6)], (-120, 60, 30))),
    ("ang_line", "angle(ang_line.origin.x, base.east) = 30 deg about=base.top", "angle", "ang_line.origin.x",
     "base.east", {"value": 30.0, "about": "base.top"}, ([((0, 0, 1), 75)], (120, -60, 0))),
    ("ang_plane", "angle(ang_plane.origin.xz, base.north.xy) = -45 deg about=base.top", "angle",
     "ang_plane.origin.xz", "base.north.xy", {"value": -45.0, "about": "base.top"},
     ([((0, 0, 1), -10)], (120, 60, 0))),
]

BASE_PRIM = {}


def nearest(kind, a_text, b_text, kw, start):
    """The nearest solution of one pair mate on a block whose primitive passes through its origin."""
    r0, t0 = start
    block_first = not a_text.startswith("base.")
    blk_ref, base_ref = (a_text, b_text) if block_first else (b_text, a_text)
    suffix = blk_ref.split(".")[2] if blk_ref.count(".") == 2 else DEFAULT[kind]
    base = primitive(place(placement(), SLAB_FRAMES[base_ref.split(".")[1]]),
                     base_ref.split(".")[2] if base_ref.count(".") == 2 else DEFAULT[kind])
    if kind == "fastened":
        fb = base[1]
        c, s = math.cos(math.radians(kw["rotate"])), math.sin(math.radians(kw["rotate"]))
        off = kw["offset"]
        o = fb[0] + off[0] * fb[1] + off[1] * fb[2] + off[2] * fb[3]
        x = c * fb[1] + s * fb[2]
        return pose_for(SLAB_FRAMES["origin"], (o, x, np.cross(fb[3], x), fb[3]))
    local = {"x": np.array([1.0, 0, 0]), "y": np.array([0, 1.0, 0]), "z": np.array([0, 0, 1.0]),
             "xy": np.array([0, 0, 1.0]), "yz": np.array([1.0, 0, 0]), "xz": np.array([0, 1.0, 0])}
    u = r0 @ local[suffix] if suffix in local else None
    bk, bp, bd = base[0], base[1], base[2]
    flags = kw.get("flags", ())
    # the turn
    q = np.eye(3)
    if kind in ("coincident", "concentric", "parallel", "distance") and u is not None and bk in ("line", "plane") \
            and (suffix in ("x", "y", "z")) == (bk == "line"):
        if kind == "distance" or (kind == "coincident" and bk == "plane"):
            sign = 1.0 if "aligned" in flags else -1.0
        else:
            sign = -1.0 if "opposed" in flags else 1.0
        q = geodesic(u, sign * bd)
    elif kind == "coincident" and u is not None:      # a line in a plane
        q = geodesic(u, (u - np.dot(u, bd) * bd) / np.linalg.norm(u - np.dot(u, bd) * bd))
    elif kind == "parallel":                           # line/plane
        q = geodesic(u, (u - np.dot(u, bd) * bd) / np.linalg.norm(u - np.dot(u, bd) * bd))
    elif kind == "perpendicular":
        if (suffix in ("x", "y", "z")) == (bk == "line"):
            q = geodesic(u, (u - np.dot(u, bd) * bd) / np.linalg.norm(u - np.dot(u, bd) * bd))
        else:
            q = geodesic(u, math.copysign(1.0, np.dot(u, bd)) * bd)
    elif kind == "angle":
        w = primitive(place(placement(), SLAB_FRAMES[kw["about"].split(".")[1]]), "z")[2]
        if abs(np.dot(u, w)) > 1e-12 or abs(np.dot(bd, w)) > 1e-12:
            sys.exit("angle pair: keep both directions normal to `about`")
        theta = math.degrees(math.atan2(np.dot(w, np.cross(bd, u)), np.dot(bd, u)))
        d = (kw["value"] - theta + 180) % 360 - 180
        q = rot(w, d)
    r = q @ r0
    # the move of the origin (the block's primitive point)
    o = t0
    if kind == "distance":
        s = kw["value"]
        if bk == "point":
            d = o - bp
            o = bp + s * d / np.linalg.norm(d)
        elif bk == "line":
            foot = bp + np.dot(o - bp, bd) * bd
            d = o - foot
            o = foot + s * d / np.linalg.norm(d)
        else:
            o = o - (np.dot(o - bp, bd) - s) * bd
    elif kind in ("coincident", "concentric"):
        if bk == "point":
            o = bp
        elif bk == "line":
            o = bp + np.dot(o - bp, bd) * bd
        else:
            o = o - np.dot(o - bp, bd) * bd
    return r, o


def check_nearest(case, a, name, poses, kind, kw):
    """No solution near the start moves the block's origin less *and* turns it less."""
    rng = np.random.default_rng(33)
    r0, t0 = a.inst[name][1]
    r, t = poses[name]
    best_move = np.linalg.norm(t - t0)
    best_turn = math.acos(max(-1.0, min(1.0, (np.trace(r @ r0.T) - 1) / 2)))
    only = {m[0] for m in a.mates if name in (m[2].split(".")[0], m[3].split(".")[0])}
    for _ in range(300):
        # a random solution: perturb the expected pose and pull it back onto the mate (Gauss-Newton)
        w = rng.normal(size=3) * 0.3
        rr = rot(w, math.degrees(np.linalg.norm(w))) @ r
        tt = t + rng.normal(size=3) * 5
        for _it in range(60):
            p = dict(poses)
            p[name] = (rr, tt)
            res = a.residuals(p, only)
            if np.max(np.abs(res)) < 1e-11:
                break
            jac = []
            for k in range(6):
                e = np.zeros(6)
                e[k] = 1e-7
                q = dict(p)
                ang = np.linalg.norm(e[3:])
                q[name] = ((rot(e[3:], math.degrees(ang)) if ang else np.eye(3)) @ rr, tt + e[:3])
                jac.append((a.residuals(q, only) - res) / 1e-7)
            step = np.linalg.lstsq(np.array(jac).T, -res, rcond=None)[0]
            ang = np.linalg.norm(step[3:])
            rr = (rot(step[3:], math.degrees(ang)) if ang else np.eye(3)) @ rr
            tt = tt + step[:3]
        else:
            continue
        move = np.linalg.norm(tt - t0)
        turn = math.acos(max(-1.0, min(1.0, (np.trace(rr @ r0.T) - 1) / 2)))
        if move < best_move - 1e-9 and turn < best_turn - 1e-9:
            sys.exit(f"{case}: {name} has a nearer solution: move {move} < {best_move}, turn {turn} < {best_turn}")


def mate_pairs():
    a = Asm()
    a.add("base", SLAB_FRAMES, placement(), grounded=True)
    lines = ["  instance base = part(\"../parts/slab.tcad\")\n"]
    mates = []
    for name, text, kind, ra, rb, kw, (rotate, at) in PAIRS:
        a.add(name, SLAB_FRAMES, placement(rotate, at))
        a.mate(name, kind, ra, rb, **kw)
        rot_txt = ", ".join(f"({axis_text(ax)}, {fmt(deg)} deg)" for ax, deg in rotate)
        lines.append(f"  instance {name} = part(\"../parts/slab.tcad\") at=({', '.join(fmt(c) + ' mm' for c in at)})"
                     f" rotate=[{rot_txt}]\n")
        mates.append(f"  mate {name}_m {text}\n")
    poses = {n: v[1] for n, v in a.inst.items()}
    for name, text, kind, ra, rb, kw, _ in PAIRS:
        poses[name] = nearest(kind, ra, rb, kw, a.inst[name][1])
    a.check("asm_mate_pairs", poses)
    dof, per = a.freedom(poses)
    for name, text, kind, ra, rb, kw, _ in PAIRS:
        if name != "fast":
            check_nearest("asm_mate_pairs", a, name, poses, kind, kw)
    src = ("tcad 0.2\n\nassembly asm_mate_pairs \"Every pair of every classic mate\" {\n" + "".join(lines)
           + "\n  ground base\n\n" + "".join(mates) + "}\n")
    files = {n: "parts/slab.tcad" for n in a.inst}
    expected = {
        "note": "One block per pair of primitives of every classic mate kind (and both orders of the mixed "
                "pairs, and the orientation flags), each mated to the grounded base by that mate alone. Each "
                "block's primitive passes through its frame origin, so the nearest solution is exact: the "
                "origin moves to the nearest point the mate allows and the block turns by the least rotation "
                "it allows. I6201 lists every block but the fastened one.",
        "codes": [],
        "info": ["I6201"],
        "fails": False,
        "dof": dof,
        "free": per,
        "instances": instances_json(a, poses, files),
    }
    bundle("asm_mate_pairs", {"parts/slab.tcad": SLAB, "assemblies/asm_mate_pairs.tcad": src},
           "assemblies/asm_mate_pairs.tcad", expected)


def fmt(v):
    v = float(v)
    return str(int(v)) if v == int(v) else repr(v)


def axis_text(ax):
    named = {(1, 0, 0): "+X", (0, 1, 0): "+Y", (0, 0, 1): "+Z", (-1, 0, 0): "-X", (0, -1, 0): "-Y",
             (0, 0, -1): "-Z"}
    return named.get(tuple(ax)) or "(" + ", ".join(fmt(c) for c in ax) + ")"


# --- 4. diagnostics -----------------------------------------------------------------------------

def slab_asm(name, body, title="Invalid"):
    return asm(name, "  instance base = part(\"../parts/slab.tcad\")\n"
                     "  instance a = part(\"../parts/slab.tcad\") at=(0 mm, 0 mm, 10 mm)\n\n  ground base\n\n"
               + body).replace('"Invalid"', f'"{title}"')


def diagnostics():
    cases = [
        ("asm_err_mate_kinds", ["E6101"], "  mate m concentric(a.top.xy, base.top.xy)\n",
         "`concentric` on two planes: the message lists the primitives each argument offers."),
        ("asm_err_mate_flag", ["E5001"], "  mate m perpendicular(a.east, base.east) opposed\n",
         "`perpendicular` has no orientation choice, so a flag is an error."),
        ("asm_err_mate_flags", ["E5001"], "  mate m coincident(a.bottom, base.top) aligned opposed\n",
         "Both orientation flags at once."),
        ("asm_err_mate_value", ["E5001"], "  mate m distance(a.bottom, base.top)\n",
         "`distance` without its value."),
        ("asm_err_mate_extra_value", ["E5001"], "  mate m coincident(a.bottom, base.top) = 2 mm\n",
         "A value on a kind that takes none."),
        ("asm_err_mate_about", ["E5001"], "  mate m angle(a.east, base.east) = 30 deg\n",
         "`angle` without `about=`."),
        ("asm_err_mate_dimension", ["E2202"], "  mate m distance(a.bottom, base.top) = 30 deg\n",
         "A distance given as an angle."),
        ("asm_err_mate_negative", ["E5001"], "  mate m distance(a.origin.origin, base.origin.origin) = -2 mm\n",
         "A negative distance between two points."),
        ("asm_err_mate_same_body", ["E5001"], "  mate m coincident(a.top, a.bottom)\n",
         "Both arguments on the same body."),
        ("asm_err_mate_kind", ["E5001"], "  mate m glued(a.bottom, base.top)\n",
         "An unknown mate kind; the message gives the nearest names."),
        ("asm_err_mate_ref", ["E6001"], "  mate m coincident(a.bottm, base.top)\n",
         "A mate argument names a connector the part does not have (did you mean `bottom`)."),
        ("asm_err_mate_name", ["E2101"], "  mate a coincident(a.bottom, base.top)\n",
         "A mate with the name of an instance."),
        ("asm_err_mate_conflict", ["E6102"],
         "  instance other = part(\"../parts/slab.tcad\") at=(80 mm, 0 mm, 0 mm)\n\n"
         "  mate seat coincident(a.bottom, base.top)\n"
         "  mate d1 distance(a.east, base.east) = 4 mm aligned\n"
         "  mate other_on coincident(other.bottom, base.top)\n"
         "  mate d2 distance(a.east, base.east) = 9 mm aligned\n",
         "Two distances between the same planes that cannot both hold: the minimal conflicting set is d1 and "
         "d2 (d2 the primary location); `seat` and the other block's mate are innocent."),
    ]
    for name, codes, body, note in cases:
        files = {"parts/slab.tcad": SLAB, f"assemblies/{name}.tcad": slab_asm(name, body)}
        expected = {"codes": codes, "fails": True, "note": note}
        if name == "asm_err_mate_conflict":
            expected["conflict"] = ["d1", "d2"]
            expected["not_named"] = ["seat", "other_on"]
            # move the extra instance line above `ground`
            files[f"assemblies/{name}.tcad"] = files[f"assemblies/{name}.tcad"].replace(
                "\n  ground base\n\n  instance other = part(\"../parts/slab.tcad\") at=(80 mm, 0 mm, 0 mm)\n",
                "\n  instance other = part(\"../parts/slab.tcad\") at=(80 mm, 0 mm, 0 mm)\n\n  ground base\n")
        bundle(name, files, f"assemblies/{name}.tcad", expected)

    # a redundant mate: solved, with a warning
    name = "asm_mate_redundant"
    a = Asm()
    a.add("base", SLAB_FRAMES, placement(), grounded=True)
    a.add("a", SLAB_FRAMES, placement(at=(0, 0, 10)))
    a.mate("seat", "fastened", "a.origin", "base.top")
    a.mate("again", "concentric", "a.origin", "base.origin")
    poses = {"base": a.inst["base"][1], "a": pose_for(SLAB_FRAMES["origin"], SLAB_FRAMES["top"])}
    a.check(name, poses)
    body = "  mate seat fastened(a.origin, base.top)\n  mate again concentric(a.origin, base.origin)\n"
    bundle(name, {"parts/slab.tcad": SLAB, f"assemblies/{name}.tcad": slab_asm(name, body, "A redundant mate")},
           f"assemblies/{name}.tcad",
           {"codes": ["W6103"], "fails": False, "dof": 0,
            "instances": instances_json(a, poses, {"base": "parts/slab.tcad", "a": "parts/slab.tcad"}),
            "note": "`again` removes nothing that `seat` has not removed and is consistent with it (the block "
                    "is fastened on the base's top, on the base's axis): W6103, and the assembly solves."})

    # a mate suppressed with its instance
    name = "asm_mate_suppressed"
    src = ("tcad 0.2\n\nassembly asm_mate_suppressed \"A mate on a suppressed instance\" {\n"
           "  param with_a = false\n\n"
           "  instance base = part(\"../parts/slab.tcad\")\n"
           "  instance a = part(\"../parts/slab.tcad\") at=(0 mm, 0 mm, 10 mm) suppress if not with_a\n\n"
           "  ground base\n\n"
           "  mate seat coincident(a.bottom, base.top)\n}\n")
    bundle(name, {"parts/slab.tcad": SLAB, f"assemblies/{name}.tcad": src}, f"assemblies/{name}.tcad",
           {"codes": [], "info": ["I6202"], "fails": False, "dof": 0,
            "instances": {"base": {"file": "parts/slab.tcad", "config": None, **as_json(placement())}},
            "note": "The instance is suppressed, so its mate is suppressed with it (I6202) and only the base "
                    "remains."})


def main():
    pin_clevis()
    plate_stack()
    mate_pairs()
    diagnostics()


if __name__ == "__main__":
    main()
