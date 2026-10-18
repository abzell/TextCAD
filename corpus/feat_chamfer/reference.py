"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

shape = Box(50, 30, 12, align=(Align.MIN, Align.MIN, Align.MIN))
edges = shape.edges().filter_by(Axis.X).filter_by_position(Axis.Z, 11.9, 12.1)
shape = chamfer(edges, 3)

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
