"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

from OCP.BRepFilletAPI import BRepFilletAPI_MakeFillet
box = Box(50, 30, 12, align=(Align.MIN, Align.MIN, Align.MIN))
edge = [e for e in box.edges() if abs(e.center().Y) < 1e-6 and abs(e.center().Z - 12) < 1e-6][0]
mk = BRepFilletAPI_MakeFillet(box.wrapped)
first = edge.start_point()
mk.Add(2.0, 5.0, edge.wrapped) if first.X < 1e-6 else mk.Add(5.0, 2.0, edge.wrapped)
mk.Build()
shape = Solid(mk.Shape())

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
