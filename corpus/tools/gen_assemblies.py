#!/usr/bin/env python3
"""Corpus cases for assembly files (spec/assembly.md § Files and bundles, § Assembly file,
§ References, § Placement before solving; issue #29).

Every case is a small bundle: `bundle.toml` (entry: the top assembly), `parts/`, `assemblies/`,
`expected.json` and, for the valid case, a `reference.py` build123d model. The expected placements
are computed here in closed form (axis-angle rotations applied in order about the fixed axes, then
the translation; nested placements composed) and cross-checked against build123d, which places the
same parts with its own `rotate` and `Pos`.

Usage: B123_PYTHON=/path/to/python-with-build123d python3 corpus/tools/gen_assemblies.py
"""
import json
import math
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from corpuslib import PI, PY, REF_HEAD, ROOT, write  # noqa: E402
from framelib import REF_FRAMES, check_frames, clean, ref_tail  # noqa: E402

# --- closed-form placement (spec/assembly.md § Placement before solving) ---------------------------


def unit(v):
    n = math.sqrt(sum(c * c for c in v))
    return [c / n for c in v]


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def matmul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def apply(m, v):
    return [sum(m[i][k] * v[k] for k in range(3)) for i in range(3)]


IDENTITY = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]


def axis_angle(axis, deg):
    """Right-hand rotation about a unit axis through the origin (Rodrigues)."""
    x, y, z = unit(axis)
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    t = 1 - c
    return [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]


def placement(rotate=(), at=(0, 0, 0)):
    """(R, t, rotate) of `rotate=[...] at=(...)`: the rotations in the order written, each about the
    fixed axes, so R = Rn ... R1; then the translation. The list is kept for the build123d model."""
    r = IDENTITY
    for axis, deg in rotate:
        r = matmul(axis_angle(axis, deg), r)
    return r, list(at), [(list(axis), deg) for axis, deg in rotate]


def compose(outer, inner):
    """The placement of `inner` seen from the frame that contains `outer`."""
    (ro, to, _), (ri, ti, _) = outer, inner
    return matmul(ro, ri), [a + b for a, b in zip(apply(ro, ti), to)], None


def place_frame(p, f):
    r, t = p[0], p[1]
    return {
        "origin_mm": [a + b for a, b in zip(apply(r, f["origin_mm"]), t)],
        "x": apply(r, f["x"]),
        "y": apply(r, f["y"]),
        "z": apply(r, f["z"]),
    }


def frame(o, z, x=None):
    z = unit(z)
    if x is None:
        x = cross([0, 0, 1], z)
        x = [1.0, 0.0, 0.0] if sum(c * c for c in x) < 1e-18 else unit(x)
    return {"origin_mm": list(o), "x": x, "y": cross(z, x), "z": z}


IDENT_FRAME = frame([0, 0, 0], [0, 0, 1])

# --- the parts: source, closed-form volume, geometry for the box, frames, build123d model ----------

PLATE = """tcad 0.2

part plate "Base plate" {
  sketch s on plane.XY {
    rect r corner=(0 mm, 0 mm) w=120 mm h=80 mm
  }
  extrude body from s.r distance=10 mm
  hole sockets on=body.cap_end at=[(30 mm, 40 mm), (90 mm, 40 mm)] d=10 mm end=through_all

  connector socket from=sockets
  connector bottom origin=body.cap_start z=body.cap_start
}
"""

BLOCK = """tcad 0.2

part block "Post block" {
  param h = 40 mm range(20 mm, 80 mm)

  sketch s on plane.XY {
    rect r corner=(-10 mm, -10 mm) w=20 mm h=20 mm
  }
  extrude body from s.r distance=h

  connector top origin=body.cap_end z=body.cap_end
  connector side origin=body.side(s.r.right) z=body.side(s.r.right)

  config tall { h = 60 mm }
}
"""

PIN = """tcad 0.2

part pin "Pin" {
  sketch s on plane.XY {
    circle c center=(0 mm, 0 mm) r=4 mm
  }
  extrude body from s.c distance=30 mm

  connector tip origin=body.cap_end z=body.cap_end
}
"""


def plate_frames():
    top = [0, 0, 1]
    return {
        "origin": IDENT_FRAME,
        "socket.p1": frame([30, 40, 10], top),
        "socket.p2": frame([90, 40, 10], top),
        "bottom": frame([60, 40, 0], [0, 0, -1]),
    }


def block_frames(h):
    return {
        "origin": IDENT_FRAME,
        "top": frame([0, 0, h], [0, 0, 1]),
        "side": frame([10, 0, h / 2], [1, 0, 0]),
    }


def pin_frames():
    return {"origin": IDENT_FRAME, "tip": frame([0, 0, 30], [0, 0, 1])}


PARTS = {
    # name: (volume, points whose box is the part's box, frames, build123d expression, derivation)
    "plate": lambda: (
        120 * 80 * 10 - 2 * PI * 25 * 10,
        [("box", [0, 0, 0], [120, 80, 10])],
        plate_frames(),
        "plate()",
        "plate_frames(s)",
    ),
    "block": lambda h=40: (
        400 * h,
        [("box", [-10, -10, 0], [10, 10, h])],
        block_frames(h),
        f"block({h})",
        f"block_frames(s, {h})",
    ),
    "pin": lambda: (
        PI * 16 * 30,
        [("cyl", [0, 0, 0], [0, 0, 30], 4)],
        pin_frames(),
        "pin()",
        "pin_frames(s)",
    ),
}

REF_PARTS = '''
def plate():
    s = Box(120, 80, 10, align=(Align.MIN, Align.MIN, Align.MIN))
    for x in (30, 90):
        s = s - Pos(x, 40, 5) * Cylinder(5, 10)
    return s


def block(h):
    return Box(20, 20, h, align=(Align.CENTER, Align.CENTER, Align.MIN))


def pin():
    return Pos(0, 0, 15) * Cylinder(4, 30)


IDENT = _frame([0, 0, 0], [0, 0, 1])


def plate_frames(s):
    _o, _top = face_frame(planar_face(s, [0, 0, 1], (60, 40, 10)))
    _ob, _nb = face_frame(planar_face(s, [0, 0, -1], (60, 40, 0)))
    return {
        "origin": IDENT,
        "socket.p1": _frame(circle_centre(s, 5, (30, 40, 10)), _top),
        "socket.p2": _frame(circle_centre(s, 5, (90, 40, 10)), _top),
        "bottom": _frame(_ob, _nb),
    }


def block_frames(s, h):
    _o, _n = face_frame(planar_face(s, [0, 0, 1], (0, 0, h)))
    _os, _ns = face_frame(planar_face(s, [1, 0, 0], (10, 0, h / 2)))
    return {"origin": IDENT, "top": _frame(_o, _n), "side": _frame(_os, _ns)}


def pin_frames(s):
    _o, _n = face_frame(planar_face(s, [0, 0, 1], (0, 0, 30)))
    return {"origin": IDENT, "tip": _frame(_o, _n)}


def place(obj, steps):
    """Applies placements innermost first: each is (rotations about the fixed axes in order, at)."""
    for rot, at in steps:
        for axis, deg in rot:
            obj = obj.rotate(Axis((0, 0, 0), axis), deg)
        obj = Pos(*at) * obj
    return obj


def place_frame(f, steps):
    o = Vertex(*f["origin_mm"])
    px = Vertex(*[a + b for a, b in zip(f["origin_mm"], f["x"])])
    pz = Vertex(*[a + b for a, b in zip(f["origin_mm"], f["z"])])
    o, px, pz = (place(v, steps) for v in (o, px, pz))
    o, px, pz = ([v.X, v.Y, v.Z] for v in (o, px, pz))
    x = _unit([a - b for a, b in zip(px, o)])
    z = _unit([a - b for a, b in zip(pz, o)])
    return {"origin_mm": o, "x": x, "y": _cross(z, x), "z": z}
'''


def leaf_box(geom, p):
    """Bounding box of a placed box or cylinder, in closed form."""
    r, t = p[0], p[1]
    if geom[0] == "box":
        lo, hi = geom[1], geom[2]
        pts = [apply(r, [x, y, z]) for x in (lo[0], hi[0]) for y in (lo[1], hi[1]) for z in (lo[2], hi[2])]
        pts = [[a + b for a, b in zip(q, t)] for q in pts]
        return [min(q[i] for q in pts) for i in range(3)], [max(q[i] for q in pts) for i in range(3)]
    _, a, b, rad = geom
    a = [x + y for x, y in zip(apply(r, a), t)]
    b = [x + y for x, y in zip(apply(r, b), t)]
    d = unit([q - p_ for p_, q in zip(a, b)])
    ext = [rad * math.sqrt(max(0.0, 1 - c * c)) for c in d]
    return ([min(a[i], b[i]) - ext[i] for i in range(3)], [max(a[i], b[i]) + ext[i] for i in range(3)])


def bundle(name, files, entry, expected, reference=None):
    d = os.path.join(ROOT, name)
    shutil.rmtree(d, ignore_errors=True)
    write(os.path.join(d, "bundle.toml"),
          f'format = "0.1"\nname = "{name}"\nentry = "{entry}"\nunits = "mm"\nlicense = "Apache-2.0"\n')
    for path, text in files.items():
        write(os.path.join(d, path), text)
    write(os.path.join(d, "expected.json"), json.dumps(expected, indent=2) + "\n")
    if reference:
        write(os.path.join(d, "reference.py"), reference)
    print("wrote", name)


# --- 1. asm_placement ------------------------------------------------------------------------------

POST = """tcad 0.2

assembly post "Post with a pin" {
  param h = 40 mm range(20 mm, 80 mm)

  instance block = part("../parts/block.tcad") { h = h }
  instance pin = part("../parts/pin.tcad") at=(0 mm, 0 mm, h)

  ground block
}
"""

TOP = """tcad 0.2

assembly asm_placement "Parts and sub-assemblies placed by at= and rotate=" {
  instance base = part("../parts/plate.tcad")
  instance post_a = assembly("post.tcad") at=(30 mm, 40 mm, 10 mm)
  instance post_b = assembly("post.tcad") at=(90 mm, 40 mm, 10 mm) rotate=[(+Z, 90 deg)] {
    h = 50 mm
  }
  instance lying = part("../parts/block.tcad") config=tall at=(60 mm, -20 mm, 10 mm) rotate=[(+Y, 90 deg), (+Z, 30 deg)]
  instance loose = part("../parts/pin.tcad") at=(0 mm, 100 mm, 0 mm) rotate=[((1, 1, 0), 45 deg)]

  ground base

  tag pin_b_tip = post_b.pin.tip
  tag lying_side = lying.side
}
"""


def post(h):
    """The sub-assembly's instances: (name, part, args, placement in the post's frame)."""
    return [("block", "block", (h,), placement()), ("pin", "pin", (), placement(at=(0, 0, h)))]


TREE = [
    # (path, file, config, placement, part or None for a sub-assembly, part args, children)
    ("base", "parts/plate.tcad", None, placement(), "plate", (), None),
    ("post_a", "assemblies/post.tcad", None, placement(at=(30, 40, 10)), None, (), post(40)),
    ("post_b", "assemblies/post.tcad", None, placement([((0, 0, 1), 90)], (90, 40, 10)), None, (), post(50)),
    ("lying", "parts/block.tcad", "tall", placement([((0, 1, 0), 90), ((0, 0, 1), 30)], (60, -20, 10)),
     "block", (60,), None),
    ("loose", "parts/pin.tcad", None, placement([((1, 1, 0), 45)], (0, 100, 0)), "pin", (), None),
]

PART_FILE = {"plate": "parts/plate.tcad", "block": "parts/block.tcad", "pin": "parts/pin.tcad"}


def main():
    instances, connectors = {}, {}
    volume, lo, hi = 0.0, [math.inf] * 3, [-math.inf] * 3
    leaves = []  # (path, part, args, [placements innermost first], world placement)

    def note_frame(path, file, config, p):
        f = place_frame(p, IDENT_FRAME)
        instances[path] = {"file": file, "config": config, **clean(f)}

    for path, file, config, p, part, args, children in TREE:
        note_frame(path, file, config, p)
        if children is None:
            leaves.append((path, part, args, [p], p))
            continue
        for cname, cpart, cargs, cp in children:
            world = compose(p, cp)
            note_frame(f"{path}.{cname}", PART_FILE[cpart], None, world)
            leaves.append((f"{path}.{cname}", cpart, cargs, [cp, p], world))

    ref_build = []
    ref_frames = []
    for path, part, args, chain, world in leaves:
        vol, geoms, frames, expr, derive = PARTS[part](*args)
        volume += vol
        for g in geoms:
            a, b = leaf_box(g, world)
            lo = [min(x, y) for x, y in zip(lo, a)]
            hi = [max(x, y) for x, y in zip(hi, b)]
        for cname, f in frames.items():
            connectors[f"{path}.{cname}"] = clean(place_frame(world, f))
        steps = [(pl[2], pl[1]) for pl in chain]
        ref_build.append(f"_steps = {steps!r}\n"
                         f"s = {expr}\n"
                         f"solids.append(place(s, _steps))\n"
                         f"for _k, _f in {derive}.items():\n"
                         f"    connectors['{path}.' + _k] = place_frame(_f, _steps)\n")
    references = {
        "pin_b_tip": connectors["post_b.pin.tip"],
        "lying_side": connectors["lying.side"],
    }
    code = (REF_HEAD + REF_FRAMES + REF_PARTS + "\nsolids = []\nplaced = {}\n"
            + "".join(ref_build).replace("connectors[", "placed[")
            + "shape = Compound(children=solids)\n"
            + ref_tail("connectors.update(placed)\n"))
    out = subprocess.run([PY, "-c", code], capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"asm_placement: build123d failed:\n{out.stderr}")
    got = json.loads(out.stdout.strip().splitlines()[-1])
    if abs(got["volume_mm3"] - volume) > 1e-6 * volume:
        sys.exit(f"asm_placement: volume {volume} disagrees with build123d {got['volume_mm3']}")
    for a, b in zip(lo + hi, got["bbox_min_mm"] + got["bbox_max_mm"]):
        if abs(a - b) > 1e-5:
            sys.exit(f"asm_placement: box {lo} {hi} disagrees with build123d {got['bbox_min_mm']} {got['bbox_max_mm']}")
    check_frames("asm_placement", connectors, got["connectors"])
    expected = {
        "note": "Parts and a rigid sub-assembly (used twice, once with a parameter override) placed by at= and "
                "rotate=: a translation, a rotation about Z, two rotations applied in order about the fixed axes, "
                "a rotation about a skew axis, and nested placements. No mates: every instance stays where it is "
                "placed. Tags resolve paths into the sub-assembly.",
        "volume_mm3": volume,
        "area_mm2": got["area_mm2"],
        "center_of_mass_mm": got["center_of_mass_mm"],
        "bbox_min_mm": lo,
        "bbox_max_mm": hi,
        "topology": {"solids": len(leaves)},
        "baseline": ["area_mm2", "center_of_mass_mm"],
        "instances": instances,
        "connectors": connectors,
        "references": references,
    }
    bundle(
        "asm_placement",
        {"parts/plate.tcad": PLATE, "parts/block.tcad": BLOCK, "parts/pin.tcad": PIN,
         "assemblies/post.tcad": POST, "assemblies/asm_placement.tcad": TOP},
        "assemblies/asm_placement.tcad",
        expected,
        code,
    )
    invalid()


# --- 2. invalid assemblies: each must fail with exactly the listed codes ---------------------------

BOX = """tcad 0.2

part box "Box" {
  sketch s on plane.XY {
    rect r corner=(0 mm, 0 mm) w=10 mm h=10 mm
  }
  extrude body from s.r distance=10 mm
  config default big { }
}
"""


def asm(name, body, version="0.2"):
    return f'tcad {version}\n\nassembly {name} "Invalid" {{\n{body}}}\n'


def invalid():
    cases = [
        ("asm_err_unknown_ref", "E6001",
         "  instance a = part(\"../parts/box.tcad\")\n  ground a\n  tag t = a.nosuch\n",
         "A path names a connector the part does not have; the message gives the nearest names.", {}),
        ("asm_err_unknown_instance", "E6001",
         "  instance a = part(\"../parts/box.tcad\")\n  ground b\n",
         "`ground` names an instance that does not exist.", {}),
        ("asm_err_missing_file", "E6002",
         "  instance a = part(\"../parts/nope.tcad\")\n  ground a\n",
         "The referenced file does not exist.", {}),
        ("asm_err_outside", "E6002",
         "  instance a = part(\"../../prim_box/parts/prim_box.tcad\")\n  ground a\n",
         "The path leaves the bundle (the file exists, in another corpus case); only `lib:` libraries reach "
         "outside.", {}),
        ("asm_err_wrong_kind", "E6002",
         "  instance a = assembly(\"../parts/box.tcad\")\n  ground a\n",
         "`assembly(...)` names a part file.", {}),
        ("asm_err_cycle", "E6004",
         "  instance a = part(\"../parts/box.tcad\")\n  instance b = assembly(\"other.tcad\")\n  ground a\n",
         "The assembly refers to another that refers back to it; the message lists the cycle.",
         {"assemblies/other.tcad": asm("other",
                                       "  instance back = assembly(\"asm_err_cycle.tcad\")\n  ground back\n")}),
        ("asm_err_no_ground", "E6012",
         "  instance a = part(\"../parts/box.tcad\")\n",
         "No instance is grounded; the first is not grounded implicitly.", {}),
        ("asm_err_ground_at", "E5001",
         "  instance a = part(\"../parts/box.tcad\")\n  ground a at=(0 mm, 0 mm, 0 mm)\n",
         "`ground` takes no arguments: the placement is written once, on the instance.", {}),
        ("asm_err_override", "E2401",
         "  instance a = part(\"../parts/box.tcad\") { nosuch = 1 mm }\n  ground a\n",
         "The override block sets a parameter the part does not declare.", {}),
        ("asm_err_override_type", "E2402",
         "  instance a = assembly(\"sub.tcad\") { w = 5 deg }\n  ground a\n",
         "An override of a length parameter with an angle.",
         {"assemblies/sub.tcad": asm("sub", "  param w = 10 mm\n  instance b = part(\"../parts/box.tcad\")\n"
                                            "  ground b\n")}),
        ("asm_err_config", "E6006",
         "  instance a = part(\"../parts/box.tcad\") config=small\n  ground a\n",
         "`config=` names a configuration the part does not declare (it has `big`).", {}),
        ("asm_err_duplicate", "E2101",
         "  instance a = part(\"../parts/box.tcad\")\n  instance a = part(\"../parts/box.tcad\")\n  ground a\n",
         "Two instances with the same name.", {}),
        ("asm_err_zero_axis", "E6011",
         "  instance a = part(\"../parts/box.tcad\") rotate=[((0, 0, 0), 30 deg)]\n  ground a\n",
         "A rotation about a zero vector.", {}),
    ]
    for name, code, body, note, extra in cases:
        files = {"parts/box.tcad": BOX, f"assemblies/{name}.tcad": asm(name, body)}
        files.update(extra)
        bundle(name, files, f"assemblies/{name}.tcad", {"codes": [code], "fails": True, "note": note})
    # format 0.1: `assembly` is a reserved word there
    name = "asm_err_version"
    bundle(name, {"parts/box.tcad": BOX,
                  f"assemblies/{name}.tcad": asm(name, "  instance a = part(\"../parts/box.tcad\")\n  ground a\n",
                                                 version="0.1")},
           f"assemblies/{name}.tcad",
           {"codes": ["E0001"], "fails": True,
            "note": "Assemblies need format 0.2; under a 0.1 header `assembly` is a reserved word."})
    # one file, two top-level items
    name = "asm_err_two_items"
    body = "  instance a = part(\"../parts/box.tcad\")\n  ground a\n"
    bundle(name, {"parts/box.tcad": BOX,
                  f"assemblies/{name}.tcad": BOX.rstrip("\n") + "\n\n" + asm(name, body)[len("tcad 0.2\n\n"):]},
           f"assemblies/{name}.tcad",
           {"codes": ["E2701"], "fails": True, "note": "A file holds one part or one assembly, not both."})


if __name__ == "__main__":
    main()
