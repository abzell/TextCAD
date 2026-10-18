"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *

w, gx, gw = 60, 20, 8
shape = Box(w, 30, 12, align=(Align.MIN, Align.MIN, Align.MIN))
shape = shape - Pos(gx + gw / 2, 15, 12 - 2) * Box(gw, 40, 4)
floor_edges = shape.edges().filter_by(Axis.Y).filter_by_position(Axis.Z, 7.99, 8.01)
shape = fillet(floor_edges, 1.5)
shape = shape - Pos(gx + gw + (w - gx - gw) / 2, 15, 6) * Cylinder(3, 12)

bb = shape.bounding_box()
c = shape.center()
print(json.dumps({
    "volume_mm3": shape.volume, "area_mm2": shape.area,
    "center_of_mass_mm": [c.X, c.Y, c.Z],
    "bbox_min_mm": [bb.min.X, bb.min.Y, bb.min.Z], "bbox_max_mm": [bb.max.X, bb.max.Y, bb.max.Z],
    "topology": {"solids": len(shape.solids()), "faces": len(shape.faces()),
                 "edges": len(shape.edges()), "vertices": len(shape.vertices())},
}))
