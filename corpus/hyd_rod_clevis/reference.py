"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

body = Box(70, 58, 40, align=(Align.MIN, Align.CENTER, Align.CENTER))
shape = body - Pos(30, 0, 0) * Box(45, 26, 100, align=(Align.MIN, Align.CENTER, Align.CENTER))
shape = fillet([e for e in shape.edges() if e.geom_type == GeomType.LINE and abs(e.start_point().X - (30)) < 1e-6 and abs(e.end_point().X - (30)) < 1e-6 and abs(e.start_point().Y - (13)) < 1e-6 and abs(e.end_point().Y - (13)) < 1e-6] + [e for e in shape.edges() if e.geom_type == GeomType.LINE and abs(e.start_point().X - (30)) < 1e-6 and abs(e.end_point().X - (30)) < 1e-6 and abs(e.start_point().Y - (-13)) < 1e-6 and abs(e.end_point().Y - (-13)) < 1e-6], 4)
shape = fillet([e for e in shape.edges() if e.geom_type == GeomType.LINE and abs(e.start_point().X - (70)) < 1e-6 and abs(e.end_point().X - (70)) < 1e-6 and abs(e.start_point().Y - (29)) < 1e-6 and abs(e.end_point().Y - (29)) < 1e-6] + [e for e in shape.edges() if e.geom_type == GeomType.LINE and abs(e.start_point().X - (70)) < 1e-6 and abs(e.end_point().X - (70)) < 1e-6 and abs(e.start_point().Y - (-29)) < 1e-6 and abs(e.end_point().Y - (-29)) < 1e-6], 4)
shape = shape - Pos(50, 0, 0) * (Rot(90, 0, 0) * Cylinder(10, 100))
shape = shape - Pos(14, 0, 0) * (Rot(0, 90, 0) * Cylinder(11, 28))
rim = [e for e in shape.edges() if e.geom_type == GeomType.CIRCLE and abs(e.radius - 10) < 1e-6 and abs(abs(e.arc_center.Y) - 29) < 1e-6]
shape = chamfer(rim, 1.5)

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
