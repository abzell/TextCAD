// OpenCASCADE shim for tcad-occt. Everything here is deterministic: no parallel algorithms, fixed
// tolerances taken from the caller, no use of global state.

// MSVC only defines M_PI when asked to.
#ifndef _USE_MATH_DEFINES
#define _USE_MATH_DEFINES
#endif

#include "cpp/shim.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <sstream>
#include <stdexcept>
#include <string>

#include "tcad-occt/src/bridge.rs.h"

#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepGProp.hxx>
#include <BRepOffsetAPI_MakePipe.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <Bnd_Box.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <Geom_BSplineCurve.hxx>
#include <LocOpe_DPrism.hxx>
#include <Precision.hxx>
#include <ShapeFix_Face.hxx>
#include <ShapeFix_Shape.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Version.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <TColgp_HArray1OfPnt.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Ax1.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
#include <gp_Dir.hxx>
#include <gp_Elips.hxx>
#include <gp_Mat.hxx>
#include <gp_Pln.hxx>
#include <gp_Pnt.hxx>
#include <gp_Vec.hxx>

namespace tcad {
namespace {

// Run a block and turn OpenCASCADE exceptions into std::runtime_error (which cxx maps to Result).
template <typename F>
auto guarded(const char* what, F&& f) -> decltype(f()) {
  try {
    return f();
  } catch (const Standard_Failure& e) {
    std::string msg = std::string(what) + ": " + e.DynamicType()->Name();
    if (e.GetMessageString() != nullptr && e.GetMessageString()[0] != '\0') {
      msg += std::string(" (") + e.GetMessageString() + ")";
    }
    throw std::runtime_error(msg);
  }
}

gp_Pnt P(const Vec3& v) { return gp_Pnt(v.x, v.y, v.z); }
gp_Dir D(const Vec3& v) { return gp_Dir(v.x, v.y, v.z); }

gp_Pnt at(const Frame3& f, double x, double y) {
  return gp_Pnt(f.o.x + x * f.x.x + y * f.y.x, f.o.y + x * f.x.y + y * f.y.y,
                f.o.z + x * f.x.z + y * f.y.z);
}

gp_Ax2 axis_in(const Frame3& f, double cx, double cy, double theta) {
  gp_Pnt c = at(f, cx, cy);
  gp_Dir z = D(f.z);
  double ct = std::cos(theta), st = std::sin(theta);
  gp_Dir x(ct * f.x.x + st * f.y.x, ct * f.x.y + st * f.y.y, ct * f.x.z + st * f.y.z);
  return gp_Ax2(c, z, x);
}

TopoDS_Edge make_edge(const Frame3& f, const Seg& s) {
  switch (s.kind) {
    case 0: {
      gp_Pnt a = at(f, s.p[0], s.p[1]), b = at(f, s.p[2], s.p[3]);
      if (a.Distance(b) < Precision::Confusion()) throw std::runtime_error("zero-length line");
      return BRepBuilderAPI_MakeEdge(a, b).Edge();
    }
    case 1: {
      GC_MakeArcOfCircle arc(at(f, s.p[0], s.p[1]), at(f, s.p[2], s.p[3]), at(f, s.p[4], s.p[5]));
      if (!arc.IsDone()) throw std::runtime_error("degenerate arc");
      return BRepBuilderAPI_MakeEdge(arc.Value()).Edge();
    }
    case 2: {
      gp_Circ c(axis_in(f, s.p[0], s.p[1], 0.0), s.p[2]);
      return BRepBuilderAPI_MakeEdge(c).Edge();
    }
    case 3: {
      double a = s.p[2], b = s.p[3], theta = s.p[4];
      if (a < b) {  // OCCT wants the major radius first
        std::swap(a, b);
        theta += M_PI / 2.0;
      }
      gp_Elips e(axis_in(f, s.p[0], s.p[1], theta), a, b);
      return BRepBuilderAPI_MakeEdge(e).Edge();
    }
    case 4: {
      size_t n = s.pts.size() / 2;
      if (n < 2) throw std::runtime_error("spline needs two points");
      if (s.interpolated) {
        Handle(TColgp_HArray1OfPnt) pts = new TColgp_HArray1OfPnt(1, (Standard_Integer)n);
        for (size_t i = 0; i < n; i++) pts->SetValue((Standard_Integer)i + 1, at(f, s.pts[2 * i], s.pts[2 * i + 1]));
        GeomAPI_Interpolate interp(pts, s.closed ? Standard_True : Standard_False, Precision::Confusion());
        interp.Perform();
        if (!interp.IsDone()) throw std::runtime_error("spline interpolation failed");
        return BRepBuilderAPI_MakeEdge(interp.Curve()).Edge();
      }
      // control-point spline: clamped, uniform knots, degree up to 3; closed = repeat the first
      size_t m = n + (s.closed ? 1 : 0);
      TColgp_Array1OfPnt poles(1, (Standard_Integer)m);
      for (size_t i = 0; i < m; i++) {
        size_t j = i % n;
        poles.SetValue((Standard_Integer)i + 1, at(f, s.pts[2 * j], s.pts[2 * j + 1]));
      }
      Standard_Integer degree = (Standard_Integer)std::min<size_t>(3, m - 1);
      Standard_Integer inner = (Standard_Integer)m - degree;  // number of spans
      TColStd_Array1OfReal knots(1, inner + 1);
      TColStd_Array1OfInteger mults(1, inner + 1);
      for (Standard_Integer i = 1; i <= inner + 1; i++) {
        knots.SetValue(i, (double)(i - 1));
        mults.SetValue(i, (i == 1 || i == inner + 1) ? degree + 1 : 1);
      }
      Handle(Geom_BSplineCurve) curve = new Geom_BSplineCurve(poles, knots, mults, degree);
      return BRepBuilderAPI_MakeEdge(curve).Edge();
    }
    default:
      throw std::runtime_error("unknown segment kind");
  }
}

TopoDS_Wire make_wire(const Frame3& f, const rust::Vec<Seg>& segs) {
  BRepBuilderAPI_MakeWire mk;
  for (const Seg& s : segs) {
    mk.Add(make_edge(f, s));
    if (!mk.IsDone()) throw std::runtime_error("segments do not connect into a wire");
  }
  if (!mk.IsDone()) throw std::runtime_error("empty wire");
  return mk.Wire();
}

TopoDS_Face make_face(const ProfileData& p) {
  if (p.loops.empty()) throw std::runtime_error("profile has no outer loop");
  gp_Pln plane(gp_Ax3(P(p.frame.o), D(p.frame.z), D(p.frame.x)));
  BRepBuilderAPI_MakeFace mk(plane, make_wire(p.frame, p.loops[0].segs), Standard_True);
  if (!mk.IsDone()) throw std::runtime_error("cannot build a face from the outer loop");
  // all loops arrive counterclockwise in the profile plane; a hole runs the other way round
  for (size_t i = 1; i < p.loops.size(); i++) {
    mk.Add(TopoDS::Wire(make_wire(p.frame, p.loops[i].segs).Reversed()));
  }
  ShapeFix_Face fix(mk.Face());
  fix.Perform();
  return fix.Face();
}

TopoDS_Wire outer_wire(const ProfileData& p) {
  if (p.loops.empty()) throw std::runtime_error("profile has no outer loop");
  return make_wire(p.frame, p.loops[0].segs);
}

std::unique_ptr<Shape> wrap(TopoDS_Shape s) {
  if (s.IsNull()) throw std::runtime_error("the operation produced no shape");
  std::unique_ptr<Shape> out(new Shape());
  out->s = s;
  return out;
}

// A solid with negative volume is inside out; turn it around.
TopoDS_Shape orient_positive(TopoDS_Shape s) {
  GProp_GProps g;
  BRepGProp::VolumeProperties(s, g);
  if (g.Mass() < 0.0) s.Reverse();
  return s;
}

TopoDS_Shape unify(const TopoDS_Shape& s) {
  ShapeUpgrade_UnifySameDomain u(s, Standard_True, Standard_True, Standard_False);
  u.Build();
  return u.Shape();
}

uint32_t count(const TopoDS_Shape& s, TopAbs_ShapeEnum kind) {
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s, kind, m);
  return (uint32_t)m.Extent();
}

}  // namespace

rust::String occt_version() { return rust::String(OCC_VERSION_STRING_EXT); }

std::unique_ptr<Shape> clone_shape(const Shape& s) { return wrap(s.s); }

std::unique_ptr<Shape> extrude(const ProfileData& p, Vec3 dir, double distance, double draft) {
  return guarded("extrude", [&]() {
    TopoDS_Face face = make_face(p);
    if (draft == 0.0) {
      gp_Vec v(dir.x * distance, dir.y * distance, dir.z * distance);
      BRepPrimAPI_MakePrism mk(face, v);
      mk.Build();
      if (!mk.IsDone()) throw std::runtime_error("prism failed");
      return wrap(orient_positive(mk.Shape()));
    }
    // A drafted extrusion goes along the face normal; the direction must be that normal.
    double along = dir.x * p.frame.z.x + dir.y * p.frame.z.y + dir.z * p.frame.z.z;
    if (std::fabs(std::fabs(along) - 1.0) > 1e-9) {
      throw std::runtime_error("a drafted extrusion must be normal to the profile");
    }
    // LocOpe_DPrism measures the height along the slanted side edges; the format says along the
    // extrusion direction.
    double height = std::fabs(distance) / std::cos(draft);
    if (along * distance < 0.0) face.Reverse();
    LocOpe_DPrism dp(face, height, draft);
    return wrap(orient_positive(dp.Shape()));
  });
}

std::unique_ptr<Shape> revolve(const ProfileData& p, Vec3 origin, Vec3 dir, double angle) {
  return guarded("revolve", [&]() {
    TopoDS_Face face = make_face(p);
    gp_Ax1 axis(P(origin), D(dir));
    BRepPrimAPI_MakeRevol mk = (angle >= 2.0 * M_PI - 1e-12) ? BRepPrimAPI_MakeRevol(face, axis)
                                                              : BRepPrimAPI_MakeRevol(face, axis, angle);
    mk.Build();
    if (!mk.IsDone()) throw std::runtime_error("revolve failed");
    return wrap(orient_positive(mk.Shape()));
  });
}

std::unique_ptr<Shape> sweep(const ProfileData& p, const PathData& path, const PathData& guide,
                             bool has_guide) {
  return guarded("sweep", [&]() {
    TopoDS_Wire spine = make_wire(path.frame, path.segs);
    if (!has_guide) {
      BRepOffsetAPI_MakePipe mk(spine, make_face(p));
      mk.Build();
      if (!mk.IsDone()) throw std::runtime_error("pipe failed");
      return wrap(orient_positive(mk.Shape()));
    }
    if (p.loops.size() != 1) throw std::runtime_error("a guided sweep needs a profile without holes");
    TopoDS_Wire aux = make_wire(guide.frame, guide.segs);
    BRepOffsetAPI_MakePipeShell mk(spine);
    mk.SetMode(aux, Standard_True, BRepFill_NoContact);
    mk.Add(outer_wire(p));
    mk.Build();
    if (!mk.IsDone()) throw std::runtime_error("guided pipe failed");
    if (!mk.MakeSolid()) throw std::runtime_error("guided pipe did not close into a solid");
    return wrap(orient_positive(mk.Shape()));
  });
}

std::unique_ptr<Shape> loft(const rust::Vec<ProfileData>& sections, bool ruled) {
  return guarded("loft", [&]() {
    BRepOffsetAPI_ThruSections gen(Standard_True, ruled ? Standard_True : Standard_False, Precision::Confusion());
    gen.CheckCompatibility(Standard_False);
    for (const ProfileData& s : sections) {
      if (s.loops.size() != 1) throw std::runtime_error("loft sections must not have holes");
      gen.AddWire(outer_wire(s));
    }
    gen.Build();
    if (!gen.IsDone()) throw std::runtime_error("loft failed");
    return wrap(orient_positive(gen.Shape()));
  });
}

std::unique_ptr<Shape> boolean_op(uint32_t op, const Shape& a, const Shape& b, double tolerance) {
  return guarded("boolean", [&]() {
    TopTools_ListOfShape args, tools;
    args.Append(a.s);
    tools.Append(b.s);
    auto run = [&](BRepAlgoAPI_BooleanOperation& alg) {
      alg.SetArguments(args);
      alg.SetTools(tools);
      alg.SetRunParallel(Standard_False);
      alg.SetFuzzyValue(tolerance);
      alg.SetNonDestructive(Standard_True);
      alg.Build();
      if (alg.HasErrors() || !alg.IsDone()) throw std::runtime_error("boolean operation failed");
      return wrap(unify(alg.Shape()));
    };
    if (op == 0) {
      BRepAlgoAPI_Fuse alg;
      return run(alg);
    }
    if (op == 1) {
      BRepAlgoAPI_Cut alg;
      return run(alg);
    }
    BRepAlgoAPI_Common alg;
    return run(alg);
  });
}

Props properties(const Shape& sh) {
  return guarded("properties", [&]() {
    const TopoDS_Shape& s = sh.s;
    Props out;
    GProp_GProps v;
    BRepGProp::VolumeProperties(s, v);
    out.volume = v.Mass();
    gp_Pnt c = v.CentreOfMass();
    out.com = Vec3{c.X(), c.Y(), c.Z()};
    // MatrixOfInertia is taken about the center of mass (verified by the tests).
    gp_Mat m = v.MatrixOfInertia();
    for (int i = 0; i < 3; i++)
      for (int j = 0; j < 3; j++) out.inertia[i * 3 + j] = m(i + 1, j + 1);
    GProp_GProps sp;
    BRepGProp::SurfaceProperties(s, sp);
    out.area = sp.Mass();
    Bnd_Box box;
    BRepBndLib::AddOptimal(s, box, Standard_False, Standard_False);
    double x0, y0, z0, x1, y1, z1;
    box.Get(x0, y0, z0, x1, y1, z1);
    out.bmin = Vec3{x0, y0, z0};
    out.bmax = Vec3{x1, y1, z1};
    out.solids = count(s, TopAbs_SOLID);
    out.faces = count(s, TopAbs_FACE);
    out.edges = count(s, TopAbs_EDGE);
    out.vertices = count(s, TopAbs_VERTEX);
    return out;
  });
}

rust::Vec<uint8_t> write_brep(const Shape& s) {
  return guarded("write_brep", [&]() {
    std::ostringstream os;
    BRepTools::Write(s.s, os, Standard_False, Standard_False, TopTools_FormatVersion_VERSION_1);
    std::string text = os.str();
    rust::Vec<uint8_t> out;
    out.reserve(text.size());
    for (unsigned char ch : text) out.push_back(ch);
    return out;
  });
}

std::unique_ptr<Shape> read_brep(rust::Slice<const uint8_t> bytes) {
  return guarded("read_brep", [&]() {
    std::string text(reinterpret_cast<const char*>(bytes.data()), bytes.size());
    std::istringstream is(text);
    TopoDS_Shape s;
    BRep_Builder b;
    BRepTools::Read(s, is, b);
    return wrap(s);
  });
}

}  // namespace tcad
