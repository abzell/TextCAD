"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

shape = Box(60, 20, 10, align=(Align.CENTER, Align.MIN, Align.MIN))
for x in (15, -15):
    shape = shape - Pos(x, 10, 5) * Cylinder(3, 10)

bb = shape.bounding_box()
c = shape.center()
print(json.dumps({
    "volume_mm3": shape.volume, "area_mm2": shape.area,
    "center_of_mass_mm": [c.X, c.Y, c.Z],
    "bbox_min_mm": [bb.min.X, bb.min.Y, bb.min.Z], "bbox_max_mm": [bb.max.X, bb.max.Y, bb.max.Z],
    "topology": {"solids": len(shape.solids()), "faces": len(shape.faces()),
                 "edges": len(shape.edges()), "vertices": len(shape.vertices())},
}))
