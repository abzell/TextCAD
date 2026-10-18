"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

r_out, t, rb = 50, 10, 30
shape = Pos(0, 0, t / 2) * Cylinder(r_out, t)
shape = shape - Pos(0, 0, t / 2) * Cylinder(10, t)
for i in range(6):
    shape = shape - Pos(rb * cos(i * pi / 3), rb * sin(i * pi / 3), t / 2) * Cylinder(3, t)
top = shape.edges().filter_by(GeomType.CIRCLE).filter_by_position(Axis.Z, t - 0.01, t + 0.01)
ch = [e for e in top if abs(e.radius - 10) < 1e-6 or (abs(e.radius - 3) < 1e-6 and abs(e.arc_center.X - rb * cos(2 * pi / 3)) < 1e-6 and abs(e.arc_center.Y - rb * sin(2 * pi / 3)) < 1e-6)]
shape = chamfer(ch, 1.5)

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
