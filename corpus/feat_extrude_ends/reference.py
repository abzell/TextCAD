"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

shape = Box(60, 40, 10, align=(Align.MIN, Align.MIN, Align.MIN))
shape = shape + Pos(10, 10, 0) * Cylinder(5, 30)
shape = shape + Pos(30, 10, 7.5) * Cylinder(4, 25)
shape = shape + Pos(50, 10, 12.5) * Cylinder(3, 25)
shape = shape - Pos(10, 30, 5) * Cylinder(3, 10)
shape = shape + Pos(30, 27, 10) * extrude(Rectangle(10, 10), amount=10, taper=5)

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
