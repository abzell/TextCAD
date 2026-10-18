#!/usr/bin/env python3
"""Corpus parts of milestone M8, part 2: four more naming-robustness parts (volumes closed-form,
every swept parameter set listed with its closed-form volume and bounding box).

Usage: B123_PYTHON=/path/to/python-with-build123d python3 corpus/tools/gen_naming_parts.py
       (then: tcad fmt corpus)
"""
import itertools
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from corpuslib import *  # noqa: F401,F403


def sweep_list(names, ranges, volume, bbox):
    out = []
    for vals in itertools.product(*ranges):
        p = dict(zip(names, vals))
        lo, hi = bbox(**p)
        out.append({
            "set": {k: f"{v} mm" for k, v in p.items()},
            "volume_mm3": volume(**p),
            "bbox_min_mm": lo,
            "bbox_max_mm": hi,
        })
    return out


# ---- 1. flange with a bolt circle pattern -------------------------------------------------------
def flange_volume(r_out, t, rb):
    d = 1.5
    chamfer = lambda r: 2 * PI * (r + d / 3) * d * d / 2
    return PI * r_out**2 * t - PI * 100 * t - 6 * PI * 9 * t - chamfer(10) - chamfer(3)


flange_body = """  param r_out = 50 mm range(40 mm, 70 mm)
  param t = 10 mm range(6 mm, 14 mm)
  param rb = 30 mm range(25 mm, 35 mm)

  sketch s on plane.XY {
    circle c center=(0 mm, 0 mm) r=r_out
  }
  extrude disc from s.c distance=t
  hole bore on=disc.cap_end at=(0 mm, 0 mm) d=20 mm end=through_all
  hole bolt on=disc.cap_end at=(rb, 0 mm) d=6 mm end=through_all
  circular_pattern bolts of=bolt axis=axis.Z count=6
  chamfer rims edges=[edge(disc.cap_end & bore.wall), edge(disc.cap_end & bolts.i3.wall)] d=1.5 mm
"""


def flange_ref(r_out, t, rb):
    return (
        f"r_out, t, rb = {r_out}, {t}, {rb}\n"
        "shape = Pos(0, 0, t / 2) * Cylinder(r_out, t)\n"
        "shape = shape - Pos(0, 0, t / 2) * Cylinder(10, t)\n"
        "for i in range(6):\n"
        "    shape = shape - Pos(rb * cos(i * pi / 3), rb * sin(i * pi / 3), t / 2) * Cylinder(3, t)\n"
        "top = shape.edges().filter_by(GeomType.CIRCLE).filter_by_position(Axis.Z, t - 0.01, t + 0.01)\n"
        "ch = [e for e in top if abs(e.radius - 10) < 1e-6 or (abs(e.radius - 3) < 1e-6 and abs(e.arc_center.X - rb * cos(2 * pi / 3)) < 1e-6 and abs(e.arc_center.Y - rb * sin(2 * pi / 3)) < 1e-6)]\n"
        "shape = chamfer(ch, 1.5)\n"
    )


emit(
    "nam_flange_pattern", "Flange with a bolt circle (naming robustness)",
    flange_body,
    "A bolt hole patterned six times; the chamfer refers to the bore and to the third instance by name. "
    "Swept outer radius, thickness and bolt circle radius must keep all names.",
    flange_volume(50, 10, 30), [-50, -50, 0], [50, 50, 10],
    flange_ref(50, 10, 30),
    sweep_list(["r_out", "t", "rb"], [[40, 50, 70], [6, 10, 14], [25, 30, 35]], flange_volume,
               lambda r_out, t, rb: ([-r_out, -r_out, 0], [r_out, r_out, t])),
)


# ---- 2. plate with a pocket, tags and a hole in the pocket floor ------------------------------
def pocket_volume(w, d, h, dp):
    return w * d * h - (w - 30) * (d - 20) * dp - PI * 9 * (h - dp) - 0.5 * 1 * 1 * (d - 20)


pocket_body = """  param w = 80 mm range(60 mm, 120 mm)
  param d = 50 mm range(40 mm, 80 mm)
  param h = 20 mm range(14 mm, 30 mm)
  param dp = 8 mm range(4 mm, 12 mm)

  sketch s on plane.XY {
    rect r corner=(0 mm, 0 mm) w=w h=d
  }
  extrude plate from s.r distance=h

  sketch cut on plate.cap_end {
    rect r corner=(15 mm, 10 mm) w=w - 30 mm h=d - 20 mm
  }
  extrude pocket from cut.r distance=-dp op=subtract

  tag floor_face = pocket.cap_end
  tag rim = edge(plate.cap_end & pocket.side(cut.r.left))

  hole drain on=floor_face at=(w / 2, d / 2) d=6 mm end=through_all
  chamfer rim_chamfer edges=[rim] d=1 mm
"""


def pocket_ref(w, d, h, dp):
    return (
        f"w, d, h, dp = {w}, {d}, {h}, {dp}\n"
        "shape = Box(w, d, h, align=(Align.MIN, Align.MIN, Align.MIN))\n"
        "shape = shape - Pos(15 + (w - 30) / 2, 10 + (d - 20) / 2, h - dp / 2) * Box(w - 30, d - 20, dp)\n"
        "shape = shape - Pos(w / 2, d / 2, (h - dp) / 2) * Cylinder(3, h - dp)\n"
        "rim = shape.edges().filter_by(Axis.Y).filter_by_position(Axis.X, 14.99, 15.01).filter_by_position(Axis.Z, h - 0.01, h + 0.01)\n"
        "shape = chamfer(rim, 1)\n"
    )


emit(
    "nam_pocket_tags", "Pocketed plate (naming robustness)",
    pocket_body,
    "A sketch on the top face cuts a pocket; two tags name the pocket floor and its rim edge; a drain hole "
    "and a chamfer use the tags. Swept plate size and pocket depth keep every reference valid.",
    pocket_volume(80, 50, 20, 8), [0, 0, 0], [80, 50, 20],
    pocket_ref(80, 50, 20, 8),
    sweep_list(["w", "d", "h", "dp"], [[60, 80, 120], [40, 50, 80], [14, 20, 30], [4, 8, 12]], pocket_volume,
               lambda w, d, h, dp: ([0, 0, 0], [w, d, h])),
)


# ---- 3. a shelled box with holes through its walls --------------------------------------------
def cup_volume(w, d, h, tw):
    return w * d * h - (w - 2 * tw) * (d - 2 * tw) * (h - tw) - 2 * PI * 16 * tw


cup_body = """  param w = 80 mm range(60 mm, 100 mm)
  param d = 50 mm range(40 mm, 70 mm)
  param h = 40 mm range(30 mm, 50 mm)
  param tw = 3 mm range(2 mm, 4 mm)

  sketch s on plane.XY {
    rect r corner=(0 mm, 0 mm) w=w h=d
  }
  extrude body from s.r distance=h
  shell cup faces=[body.cap_end] thickness=tw
  hole port on=body.side(s.r.left) at=(-(d / 2), h / 2) d=8 mm end=through_all
"""


def cup_ref(w, d, h, tw):
    return (
        f"w, d, h, tw = {w}, {d}, {h}, {tw}\n"
        "shape = Box(w, d, h, align=(Align.MIN, Align.MIN, Align.MIN))\n"
        "shape = shape - Pos(w / 2, d / 2, tw + (h - tw) / 2) * Box(w - 2 * tw, d - 2 * tw, h - tw)\n"
        "# the hole goes through the two walls only: the cavity is air already\n"
        "wall = Pos(w / 2, d / 2, h / 2) * (Rot(0, 90, 0) * Cylinder(4, w + 2))\n"
        "shape = shape - wall\n"
    )


emit(
    "nam_shell_box", "Open box with a cross hole (naming robustness)",
    cup_body,
    "A box is shelled open at the top and a hole is drilled through two walls from the outer left face. Swept "
    "size and wall thickness must keep the shell faces and the hole faces named the same.",
    cup_volume(80, 50, 40, 3), [0, 0, 0], [80, 50, 40],
    cup_ref(80, 50, 40, 3),
    sweep_list(["w", "d", "h", "tw"], [[60, 80, 100], [40, 50, 70], [30, 40, 50], [2, 3, 4]], cup_volume,
               lambda w, d, h, tw: ([0, 0, 0], [w, d, h])),
)


# ---- 4. a mirrored hole and chamfers by name ---------------------------------------------------
def mirror_volume(w, c):
    ch = lambda r: 2 * PI * (r + c / 3) * c * c / 2
    return w * 30 * 12 - 2 * PI * 9 * 12 - 2 * 0.5 * c * c * w - ch(3) - ch(3)


mirror_body = """  param w = 90 mm range(60 mm, 140 mm)
  param c = 2 mm range(1 mm, 3 mm)

  sketch s on plane.XY {
    rect r corner=(-w / 2, 0 mm) w=w h=30 mm
  }
  extrude plate from s.r distance=12 mm
  hole h on=plate.cap_end at=(w / 4, 15 mm) d=6 mm end=through_all
  mirror twin of=h plane=plane.YZ
  chamfer edges_ch edges=[edge(plate.cap_end & plate.side(s.r.top)), edge(plate.cap_end & plate.side(s.r.bottom)), edge(plate.cap_end & h.wall), edge(plate.cap_end & twin.wall)] d=c
"""


def mirror_ref(w, c):
    return (
        f"w, c = {w}, {c}\n"
        "shape = Box(w, 30, 12, align=(Align.CENTER, Align.MIN, Align.MIN))\n"
        "for x in (w / 4, -w / 4):\n"
        "    shape = shape - Pos(x, 15, 6) * Cylinder(3, 12)\n"
        "top = [e for e in shape.edges().filter_by_position(Axis.Z, 11.99, 12.01)\n"
        "       if not (e.geom_type == GeomType.LINE and abs(e.start_point().X - e.end_point().X) < 1e-6)]\n"
        "shape = chamfer(top, c)\n"
    )


emit(
    "nam_mirror_chamfer", "Symmetric plate with mirrored holes (naming robustness)",
    mirror_body,
    "A hole and its mirror image, and four chamfers that name the top face with the long sides and with both hole "
    "walls. Swept width and chamfer size must keep every name.",
    mirror_volume(90, 2), [-45, 0, 0], [45, 30, 12],
    mirror_ref(90, 2),
    sweep_list(["w", "c"], [[60, 90, 140], [1, 2, 3]], mirror_volume,
               lambda w, c: ([-w / 2, 0, 0], [w / 2, 30, 12])),
)
