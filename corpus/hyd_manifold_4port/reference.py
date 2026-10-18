"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

shape = Box(120, 60, 40, align=(Align.MIN, Align.MIN, Align.MIN))
shape = shape - Pos(55, 30, 20) * (Rot(0, 90, 0) * Cylinder(6, 110))
shape = shape - Pos(60, 25, 20) * (Rot(90, 0, 0) * Cylinder(4, 50))
for i in range(4):
    shape = shape - Pos(20 + i * 80 / 3, 30, 27) * Cylinder(8.25, 26)
for x in (8, 112):
    for y in (8, 52):
        shape = shape - Pos(x, y, 20) * Cylinder(4.5, 40)

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
