"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

w, d, h, tw = 80, 50, 40, 3
shape = Box(w, d, h, align=(Align.MIN, Align.MIN, Align.MIN))
shape = shape - Pos(w / 2, d / 2, tw + (h - tw) / 2) * Box(w - 2 * tw, d - 2 * tw, h - tw)
# the hole goes through the two walls only: the cavity is air already
wall = Pos(w / 2, d / 2, h / 2) * (Rot(0, 90, 0) * Cylinder(4, w + 2))
shape = shape - wall

from OCP.BRepGProp import BRepGProp
from OCP.GProp import GProp_GProps
# integrate to a relative precision of 1e-12 (the default quadrature is only good to about 1e-6)
_v, _a = GProp_GProps(), GProp_GProps()
BRepGProp.VolumeProperties_s(shape.wrapped, _v, 1e-12, False)
BRepGProp.SurfaceProperties_s(shape.wrapped, _a, 1e-12)
bb = shape.bounding_box()
c = _v.CentreOfMass()
print(json.dumps({
    "volume_mm3": _v.Mass(), "area_mm2": _a.Mass(),
    "center_of_mass_mm": [c.X(), c.Y(), c.Z()],
    "bbox_min_mm": [bb.min.X, bb.min.Y, bb.min.Z], "bbox_max_mm": [bb.max.X, bb.max.Y, bb.max.Z],
    "topology": {"solids": len(shape.solids()), "faces": len(shape.faces()),
                 "edges": len(shape.edges()), "vertices": len(shape.vertices())},
}))
