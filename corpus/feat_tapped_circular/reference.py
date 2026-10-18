"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

shape = Pos(0, 0, 4) * Cylinder(30, 8)
shape = shape - Pos(0, 0, 4) * Cylinder(5, 8)
for i in range(6):
    shape = shape - Pos(20 * cos(i * pi / 3), 20 * sin(i * pi / 3), 5) * Cylinder(2.5, 6)

bb = shape.bounding_box()
c = shape.center()
print(json.dumps({
    "volume_mm3": shape.volume, "area_mm2": shape.area,
    "center_of_mass_mm": [c.X, c.Y, c.Z],
    "bbox_min_mm": [bb.min.X, bb.min.Y, bb.min.Z], "bbox_max_mm": [bb.max.X, bb.max.Y, bb.max.Z],
    "topology": {"solids": len(shape.solids()), "faces": len(shape.faces()),
                 "edges": len(shape.edges()), "vertices": len(shape.vertices())},
}))
