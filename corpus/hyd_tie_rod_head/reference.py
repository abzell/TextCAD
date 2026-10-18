"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

shape = Box(100, 100, 40, align=(Align.CENTER, Align.CENTER, Align.MIN))
shape = fillet(shape.edges().filter_by(Axis.Z), 8)
shape = shape - Pos(0, 0, 20) * Cylinder(25, 40)
for sx in (36, -36):
    for sy in (36, -36):
        shape = shape - Pos(sx, sy, 20) * Cylinder(7, 40)
ring = Pos(0, 0, 20) * (Cylinder(27.5, 6) - Cylinder(24.5, 6))
shape = shape - ring
shape = shape - Pos(25, 0, 20) * (Rot(0, 90, 0) * Cylinder(8.25, 50))

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
