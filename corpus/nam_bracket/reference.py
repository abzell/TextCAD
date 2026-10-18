"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

w, t, h, l = 40, 6, 50, 60
pts = [(0, 0), (l, 0), (l, t), (t, t), (t, h), (0, h)]
shape = extrude(Plane.XZ * Polygon(*pts, align=None), amount=w / 2, both=True)
shape = shape - Pos(l - 15, 0, t / 2) * Cylinder(3, t + 2)
shape = shape - Pos(t / 2, 0, h - 12) * (Rot(0, 90, 0) * Cylinder(3, t + 2))
inner = shape.edges().filter_by(Axis.Y).filter_by_position(Axis.Z, t - 0.01, t + 0.01).filter_by_position(Axis.X, t - 0.01, t + 0.01)
shape = fillet(inner, 3)

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
