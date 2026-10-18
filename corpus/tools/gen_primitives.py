#!/usr/bin/env python3
"""Regenerates the eight primitive corpus parts (sources, analytic expectations, build123d references).

Usage: python3 corpus/tools/gen_primitives.py   (then: tcad fmt corpus)

The expected values are closed-form, computed here, never read back from a kernel.
"""
import json
import math
import os

PI = math.pi
ROOT = os.path.join(os.path.dirname(__file__), "..")


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)


def part_source(name, title, body):
    return f'tcad 0.1\n\npart {name} "{title}" {{\n{body}}}\n'


def reference(body):
    return (
        '"""Independent build123d model of the same part (CI only; never part of the product)."""\n'
        "import json\n"
        "from build123d import *\n\n"
        f"{body}\n"
        "bb = shape.bounding_box()\n"
        "c = shape.center()\n"
        "print(json.dumps({\n"
        '    "volume_mm3": shape.volume, "area_mm2": shape.area,\n'
        '    "center_of_mass_mm": [c.X, c.Y, c.Z],\n'
        '    "bbox_min_mm": [bb.min.X, bb.min.Y, bb.min.Z], "bbox_max_mm": [bb.max.X, bb.max.Y, bb.max.Z],\n'
        '    "topology": {"solids": len(shape.solids()), "faces": len(shape.faces()),\n'
        '                 "edges": len(shape.edges()), "vertices": len(shape.vertices())},\n'
        "}))\n"
    )


def emit(name, title, body, expected, ref):
    f, e, v = TOPOLOGY[name]
    expected["topology"] = {"solids": 1, "faces": f, "edges": e, "vertices": v}
    d = os.path.join(ROOT, name)
    write(os.path.join(d, "bundle.toml"), f'format = "0.1"\nname = "{name}"\nentry = "parts/{name}.tcad"\nunits = "mm"\nlicense = "Apache-2.0"\n')
    write(os.path.join(d, "parts", f"{name}.tcad"), part_source(name, title, body))
    write(os.path.join(d, "expected.json"), json.dumps(expected, indent=2) + "\n")
    write(os.path.join(d, "reference.py"), reference(ref))


# OpenCASCADE's conventions (a cylinder has one seam edge, a sphere's poles are degenerate edges).
# These are the only values not derived in closed form; they are a regression baseline.
TOPOLOGY = {
    "prim_block": (6, 12, 8), "prim_cylinder": (3, 3, 2), "prim_cone": (2, 3, 2), "prim_sphere": (1, 3, 2),
    "prim_torus": (1, 2, 1), "prim_wedge": (5, 9, 6), "prim_tube": (4, 6, 4), "prim_hex_prism": (8, 18, 12),
}


def exp(volume, area, com, lo, hi, note):
    return {
        "note": note,
        "volume_mm3": volume,
        "area_mm2": area,
        "center_of_mass_mm": com,
        "bbox_min_mm": lo,
        "bbox_max_mm": hi,
    }


# 1. block 40 x 25 x 10 with its corner at the origin
emit(
    "prim_block", "Block",
    "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=40 mm h=25 mm\n  }\n  extrude e from s.r distance=10 mm\n",
    exp(40 * 25 * 10, 2 * (40 * 25 + 40 * 10 + 25 * 10), [20, 12.5, 5], [0, 0, 0], [40, 25, 10],
        "A rectangle extruded: the simplest solid."),
    "shape = Box(40, 25, 10, align=(Align.MIN, Align.MIN, Align.MIN))",
)

# 2. cylinder r = 12.5, h = 30 standing on the XY plane
r, h = 12.5, 30
emit(
    "prim_cylinder", "Cylinder",
    "  sketch s on plane.XY {\n    circle c center=(0 mm, 0 mm) r=12.5 mm\n  }\n  extrude e from s distance=30 mm\n",
    exp(PI * r * r * h, 2 * PI * r * (r + h), [0, 0, h / 2], [-r, -r, 0], [r, r, h],
        "A circle extruded."),
    "shape = Cylinder(12.5, 30, align=(Align.CENTER, Align.CENTER, Align.MIN))",
)

# 3. cone r = 15, h = 40 by revolving a right triangle about the Z axis
r, h = 15, 40
slant = math.hypot(r, h)
emit(
    "prim_cone", "Cone",
    "  sketch s on plane.XZ {\n    line a start=(0 mm, 0 mm) end=(15 mm, 0 mm)\n    line b start=(15 mm, 0 mm) end=(0 mm, 40 mm)\n    line c start=(0 mm, 40 mm) end=(0 mm, 0 mm)\n  }\n  revolve v from s axis=axis.Z\n",
    exp(PI * r * r * h / 3, PI * r * (r + slant), [0, 0, h / 4], [-r, -r, 0], [r, r, h],
        "A triangle revolved about the axis along one of its sides."),
    "shape = Cone(15, 0, 40, align=(Align.CENTER, Align.CENTER, Align.MIN))",
)

# 4. sphere R = 20 by revolving a half disk
R = 20
emit(
    "prim_sphere", "Sphere",
    "  sketch s on plane.XZ {\n    arc a start=(0 mm, -20 mm) end=(0 mm, 20 mm) through=(20 mm, 0 mm)\n    line l start=(0 mm, 20 mm) end=(0 mm, -20 mm)\n  }\n  revolve v from s axis=axis.Z\n",
    exp(4 / 3 * PI * R**3, 4 * PI * R * R, [0, 0, 0], [-R, -R, -R], [R, R, R],
        "A half disk revolved: a sphere from an arc and a line."),
    "shape = Sphere(20)",
)

# 5. torus: major radius 30, minor radius 8
Rt, rt = 30, 8
emit(
    "prim_torus", "Torus",
    "  sketch s on plane.XZ {\n    circle c center=(30 mm, 0 mm) r=8 mm\n  }\n  revolve v from s axis=axis.Z\n",
    exp(2 * PI**2 * Rt * rt**2, 4 * PI**2 * Rt * rt, [0, 0, 0], [-(Rt + rt), -(Rt + rt), -rt], [Rt + rt, Rt + rt, rt],
        "A circle revolved about an axis that does not touch it."),
    "shape = Torus(30, 8)",
)

# 6. wedge: right triangle 30 x 20, extruded 15
emit(
    "prim_wedge", "Wedge",
    "  sketch s on plane.XY {\n    line a start=(0 mm, 0 mm) end=(30 mm, 0 mm)\n    line b start=(30 mm, 0 mm) end=(0 mm, 20 mm)\n    line c start=(0 mm, 20 mm) end=(0 mm, 0 mm)\n  }\n  extrude e from s distance=15 mm\n",
    exp(0.5 * 30 * 20 * 15, 2 * 300 + (30 + 20 + math.hypot(30, 20)) * 15, [10, 20 / 3, 7.5], [0, 0, 0], [30, 20, 15],
        "A closed chain of three lines extruded."),
    "shape = extrude(Polygon((0, 0), (30, 0), (0, 20), align=None), amount=15)",
)

# 7. tube: outer r 20, inner r 14, length 50
Ro, ri, L = 20, 14, 50
emit(
    "prim_tube", "Tube",
    "  sketch s on plane.XY {\n    circle outer center=(0 mm, 0 mm) r=20 mm\n    circle inner center=(0 mm, 0 mm) r=14 mm\n  }\n  extrude e from s distance=50 mm\n",
    exp(PI * (Ro**2 - ri**2) * L, 2 * PI * (Ro + ri) * L + 2 * PI * (Ro**2 - ri**2), [0, 0, L / 2], [-Ro, -Ro, 0], [Ro, Ro, L],
        "Two nested circles: the inner one is a hole."),
    "shape = Cylinder(20, 50, align=(Align.CENTER, Align.CENTER, Align.MIN)) - Cylinder(14, 50, align=(Align.CENTER, Align.CENTER, Align.MIN))",
)

# 8. hexagonal prism, circumradius 12, height 20 (a vertex on +X)
rc, hh = 12, 20
side_area = 3 * math.sqrt(3) / 2 * rc * rc
emit(
    "prim_hex_prism", "Hex prism",
    "  sketch s on plane.XY {\n    polygon g center=(0 mm, 0 mm) sides=6 r=12 mm\n  }\n  extrude e from s distance=20 mm\n",
    exp(side_area * hh, 2 * side_area + 6 * rc * hh, [0, 0, hh / 2], [-rc, -rc * math.sin(PI / 3), 0], [rc, rc * math.sin(PI / 3), hh],
        "A regular polygon extruded."),
    "shape = extrude(RegularPolygon(radius=12, side_count=6), amount=20)",
)
