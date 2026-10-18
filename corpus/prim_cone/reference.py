"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from build123d import *

shape = Cone(15, 0, 40, align=(Align.CENTER, Align.CENTER, Align.MIN))
bb = shape.bounding_box()
c = shape.center()
print(json.dumps({
    "volume_mm3": shape.volume, "area_mm2": shape.area,
    "center_of_mass_mm": [c.X, c.Y, c.Z],
    "bbox_min_mm": [bb.min.X, bb.min.Y, bb.min.Z], "bbox_max_mm": [bb.max.X, bb.max.Y, bb.max.Z],
    "topology": {"solids": len(shape.solids()), "faces": len(shape.faces()),
                 "edges": len(shape.edges()), "vertices": len(shape.vertices())},
}))
