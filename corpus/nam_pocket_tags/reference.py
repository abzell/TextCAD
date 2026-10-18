"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

w, d, h, dp = 80, 50, 20, 8
shape = Box(w, d, h, align=(Align.MIN, Align.MIN, Align.MIN))
shape = shape - Pos(15 + (w - 30) / 2, 10 + (d - 20) / 2, h - dp / 2) * Box(w - 30, d - 20, dp)
shape = shape - Pos(w / 2, d / 2, (h - dp) / 2) * Cylinder(3, h - dp)
rim = shape.edges().filter_by(Axis.Y).filter_by_position(Axis.X, 14.99, 15.01).filter_by_position(Axis.Z, h - 0.01, h + 0.01)
shape = chamfer(rim, 1)

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
