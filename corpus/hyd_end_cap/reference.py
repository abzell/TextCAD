"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

shape = Pos(0, 0, 12.5) * Cylinder(60, 25)
shape = shape + Pos(0, 0, -3) * Cylinder(40, 6)
shape = shape - Pos(0, 0, 17) * Cylinder(10.205357142857142, 16)
shape = shape - Pos(0, 0, 24) * Cylinder(15, 2)
for i in range(4):
    shape = shape - Pos(50 * cos(i * pi / 2), 50 * sin(i * pi / 2), 12.5) * Cylinder(7, 60)
rim = [e for e in shape.edges() if e.geom_type == GeomType.CIRCLE and abs(e.radius - 60) < 1e-6 and abs(e.arc_center.Z - 25) < 1e-6]
shape = chamfer(rim, 2)

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
