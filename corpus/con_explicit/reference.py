"""Independent build123d model of the same part (CI only; never part of the product)."""
import json
from math import cos, sin, pi, tan, radians
from build123d import *


from OCP.BRepAdaptor import BRepAdaptor_Surface


def _unit(v):
    n = (v[0] ** 2 + v[1] ** 2 + v[2] ** 2) ** 0.5
    return [c / n for c in v]


def _cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def _dot(a, b):
    return sum(p * q for p, q in zip(a, b))


def _orient(d):
    """An axis of revolution has no sign: make its first non-zero component positive."""
    for c in d:
        if abs(c) > 1e-9:
            return d if c > 0 else [-v for v in d]
    return d


def _frame(o, z, x=None, flip=False, rotate=0.0, offset=(0, 0, 0)):
    z = _unit(z)
    if x is None:
        x = _cross([0, 0, 1], z)
        x = [1.0, 0.0, 0.0] if _dot(x, x) < 1e-18 else _unit(x)
    else:
        x = _unit([a - _dot(x, z) * b for a, b in zip(x, z)])
    y = _cross(z, x)
    if flip:
        y, z = [-c for c in y], [-c for c in z]
    if rotate:
        c, s = cos(radians(rotate)), sin(radians(rotate))
        x, y = [c * a + s * b for a, b in zip(x, y)], [-s * a + c * b for a, b in zip(x, y)]
    o = [o[i] + offset[0] * x[i] + offset[1] * y[i] + offset[2] * z[i] for i in range(3)]
    return {"origin_mm": o, "x": x, "y": y, "z": z}


def planar_face(shape, normal, near):
    """The planar face with this outward normal whose centroid is nearest to `near`."""
    best = None
    for f in shape.faces():
        if f.geom_type != GeomType.PLANE:
            continue
        n = f.normal_at(f.center())
        if _dot([n.X, n.Y, n.Z], normal) < 1 - 1e-9:
            continue
        c = f.center(CenterOf.MASS)
        d = (c - Vector(*near)).length
        if best is None or d < best[0]:
            best = (d, f)
    return best[1]


def face_frame(face):
    """Centroid and outward normal of a planar face."""
    c = face.center(CenterOf.MASS)
    n = face.normal_at(c)
    return [c.X, c.Y, c.Z], [n.X, n.Y, n.Z]


def circle_centre(shape, radius, near):
    """Centre of the circular edge of this radius nearest to `near`."""
    best = None
    for e in shape.edges():
        if e.geom_type != GeomType.CIRCLE or abs(e.radius - radius) > 1e-9:
            continue
        c = e.arc_center
        d = (c - Vector(*near)).length
        if best is None or d < best[0]:
            best = (d, c)
    return [best[1].X, best[1].Y, best[1].Z]


def cylinder_axis(shape, radius, near, at="start"):
    """Oriented axis of the cylindrical face of this radius nearest to `near`, and the point on it at
    the face's start, middle or end along that axis."""
    best = None
    for f in shape.faces():
        if f.geom_type != GeomType.CYLINDER:
            continue
        cyl = BRepAdaptor_Surface(f.wrapped).Cylinder()
        if abs(cyl.Radius() - radius) > 1e-9:
            continue
        d = (f.center(CenterOf.MASS) - Vector(*near)).length
        if best is None or d < best[0]:
            best = (d, f, cyl)
    _, f, cyl = best
    ax = cyl.Axis()
    p = [ax.Location().X(), ax.Location().Y(), ax.Location().Z()]
    z = _orient([ax.Direction().X(), ax.Direction().Y(), ax.Direction().Z()])
    ts = [_dot([v.X - p[0], v.Y - p[1], v.Z - p[2]], z) for v in f.vertices()]
    t = {"start": min(ts), "end": max(ts), "middle": (min(ts) + max(ts)) / 2}[at]
    return [p[i] + t * z[i] for i in range(3)], z


def cylinder_axis_all(shape, radius, axis_point, axis_dir, at="start"):
    """Like cylinder_axis, over every cylindrical face of this radius on the given axis (the pieces of a
    split face)."""
    z = _orient(_unit(axis_dir))
    ts = []
    for f in shape.faces():
        if f.geom_type != GeomType.CYLINDER:
            continue
        cyl = BRepAdaptor_Surface(f.wrapped).Cylinder()
        if abs(cyl.Radius() - radius) > 1e-9:
            continue
        ax = cyl.Axis()
        q = [ax.Location().X() - axis_point[0], ax.Location().Y() - axis_point[1], ax.Location().Z() - axis_point[2]]
        d = [ax.Direction().X(), ax.Direction().Y(), ax.Direction().Z()]
        if abs(abs(_dot(d, z)) - 1) > 1e-9 or _dot(_cross(q, z), _cross(q, z)) > 1e-12:
            continue
        ts += [_dot([v.X - axis_point[0], v.Y - axis_point[1], v.Z - axis_point[2]], z) for v in f.vertices()]
    t = {"start": min(ts), "end": max(ts), "middle": (min(ts) + max(ts)) / 2}[at]
    return [axis_point[i] + t * z[i] for i in range(3)], z

shape = Box(40, 30, 20, align=(Align.MIN, Align.MIN, Align.MIN))

from OCP.BRepGProp import BRepGProp
from OCP.GProp import GProp_GProps
# integrate to a relative precision of 1e-12 (the default quadrature is only good to about 1e-6)
_v, _a = GProp_GProps(), GProp_GProps()
BRepGProp.VolumeProperties_s(shape.wrapped, _v, 1e-12, False)
BRepGProp.SurfaceProperties_s(shape.wrapped, _a, 1e-12)
bb = shape.bounding_box()
c = _v.CentreOfMass()
connectors = {}
_o, _n = face_frame(planar_face(shape, [0, 0, 1], (20, 15, 20)))
connectors['top'] = _frame(_o, _n)
_o, _n = face_frame(planar_face(shape, [0, -1, 0], (20, 0, 10)))
connectors['front'] = _frame(_o, _n)
connectors['corner'] = _frame([40, 30, 0], [0, 0, -1], x=[1, 1, 0])
connectors['tilt'] = _frame([0, 0, 0], [0, 1, 1], x=[1, 0, 0])
connectors['seat'] = _frame([10, 5, 20], [0, 0, 1], x=[1, 0, 0], rotate=90, offset=(0, 0, 2))
connectors['probe'] = _frame([0, 0, 20], [0, 0, 1], flip=True, rotate=90, offset=(5, 0, 0))
print(json.dumps({
    "volume_mm3": _v.Mass(), "area_mm2": _a.Mass(),
    "center_of_mass_mm": [c.X(), c.Y(), c.Z()],
    "bbox_min_mm": [bb.min.X, bb.min.Y, bb.min.Z], "bbox_max_mm": [bb.max.X, bb.max.Y, bb.max.Z],
    "topology": {"solids": len(shape.solids()), "faces": len(shape.faces()),
                 "edges": len(shape.edges()), "vertices": len(shape.vertices())},
    "connectors": connectors,
}))
