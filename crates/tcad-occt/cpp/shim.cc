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

#include <APIHeaderSection_MakeHeader.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Interface_Static.hxx>
#include <Poly_Triangulation.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_StepModelType.hxx>
#include <StepData_StepModel.hxx>
#include <TCollection_AsciiString.hxx>
#include <TCollection_ExtendedString.hxx>
#include <TCollection_HAsciiString.hxx>
#include <TDF_Label.hxx>
#include <TDataStd_Name.hxx>
#include <TDocStd_Document.hxx>
#include <TopLoc_Location.hxx>
#include <XCAFApp_Application.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_MaterialTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <mutex>
#include <random>
#include <TopoDS_Compound.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepOffsetAPI_DraftAngle.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
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
#include <GCPnts_AbscissaPoint.hxx>
#include <GProp_GProps.hxx>
#include <GeomAbs_CurveType.hxx>
#include <GeomAbs_SurfaceType.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <Geom_BSplineCurve.hxx>
#include <LocOpe_DPrism.hxx>
#include <Precision.hxx>
#include <ShapeBuild_ReShape.hxx>
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
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_ListIteratorOfListOfShape.hxx>
#include <BRep_Tool.hxx>
#include <gp_Trsf.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Wire.hxx>
#include <BRepOffset_Mode.hxx>
#include <GeomAbs_JoinType.hxx>
#include <gp_Ax1.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Sphere.hxx>
#include <gp_Torus.hxx>
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

// Edges of a profile in (loop, segment) order, so history can be traced back to segments.
using EdgeGrid = std::vector<std::vector<TopoDS_Edge>>;

TopoDS_Wire make_wire(const Frame3& f, const rust::Vec<Seg>& segs, std::vector<TopoDS_Edge>* edges = nullptr) {
  BRepBuilderAPI_MakeWire mk;
  for (const Seg& s : segs) {
    TopoDS_Edge e = make_edge(f, s);
    if (edges != nullptr) edges->push_back(e);
    mk.Add(e);
    if (!mk.IsDone()) throw std::runtime_error("segments do not connect into a wire");
  }
  if (!mk.IsDone()) throw std::runtime_error("empty wire");
  return mk.Wire();
}

TopoDS_Face make_face(const ProfileData& p, EdgeGrid* grid = nullptr) {
  if (p.loops.empty()) throw std::runtime_error("profile has no outer loop");
  gp_Pln plane(gp_Ax3(P(p.frame.o), D(p.frame.z), D(p.frame.x)));
  EdgeGrid local;
  local.resize(p.loops.size());
  BRepBuilderAPI_MakeFace mk(plane, make_wire(p.frame, p.loops[0].segs, &local[0]), Standard_True);
  if (!mk.IsDone()) throw std::runtime_error("cannot build a face from the outer loop");
  // all loops arrive counterclockwise in the profile plane; a hole runs the other way round
  for (size_t i = 1; i < p.loops.size(); i++) {
    mk.Add(TopoDS::Wire(make_wire(p.frame, p.loops[i].segs, &local[i]).Reversed()));
  }
  ShapeFix_Face fix(mk.Face());
  fix.Perform();
  if (grid != nullptr) {
    // Fixing may rebuild edges; find each segment's edge in the final face by its midpoint.
    TopTools_IndexedMapOfShape fe;
    TopExp::MapShapes(fix.Face(), TopAbs_EDGE, fe);
    auto mid = [](const TopoDS_Edge& e) {
      BRepAdaptor_Curve c(e);
      return c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
    };
    for (auto& loop : local) {
      for (auto& e : loop) {
        gp_Pnt m = mid(e);
        double best = 1e300;
        TopoDS_Edge pick_e = e;
        for (int k = 1; k <= fe.Extent(); k++) {
          TopoDS_Edge cand = TopoDS::Edge(fe(k));
          double d = mid(cand).Distance(m);
          if (d < best) {
            best = d;
            pick_e = cand;
          }
        }
        e = pick_e;
      }
    }
    *grid = local;
  }
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


// ---- topology indices and history ----

TopTools_IndexedMapOfShape face_map(const TopoDS_Shape& s) {
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s, TopAbs_FACE, m);
  return m;
}

TopTools_IndexedMapOfShape edge_map(const TopoDS_Shape& s) {
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s, TopAbs_EDGE, m);
  return m;
}

// 0-based index of `f` in `m`, or -1.
int index_in(const TopTools_IndexedMapOfShape& m, const TopoDS_Shape& f) {
  int i = m.FindIndex(f);
  return i == 0 ? -1 : i - 1;
}

enum RoleCode : uint32_t { kCapStart = 0, kCapEnd = 1, kSide = 2, kFromEdge = 3, kFromFace = 4 };

void add_created(Shape& out, const TopTools_IndexedMapOfShape& faces, const TopoDS_Shape& f,
                 uint32_t role, uint32_t a, uint32_t b) {
  if (f.IsNull()) return;
  int i = index_in(faces, f);
  if (i >= 0) out.created.push_back({(uint32_t)i, role, a, b});
}

void add_created_list(Shape& out, const TopTools_IndexedMapOfShape& faces,
                      const TopTools_ListOfShape& l, uint32_t role, uint32_t a, uint32_t b) {
  for (TopTools_ListIteratorOfListOfShape it(l); it.More(); it.Next()) {
    if (it.Value().ShapeType() == TopAbs_FACE) add_created(out, faces, it.Value(), role, a, b);
  }
}

// Runs `maker` history on every input face: result faces that are the (possibly modified) input
// face are recorded as inherited from `operand`.
template <typename Maker>
void inherit_from(Shape& out, Maker& mk, const TopoDS_Shape& input, uint32_t operand,
                  const TopTools_IndexedMapOfShape& result_faces) {
  TopTools_IndexedMapOfShape in_faces = face_map(input);
  for (int i = 1; i <= in_faces.Extent(); i++) {
    const TopoDS_Shape& f = in_faces(i);
    if (mk.IsDeleted(f)) continue;
    const TopTools_ListOfShape& mod = mk.Modified(f);
    if (mod.IsEmpty()) {
      int r = index_in(result_faces, f);
      if (r >= 0) out.inherited.push_back({(uint32_t)r, operand, (uint32_t)(i - 1)});
    } else {
      for (TopTools_ListIteratorOfListOfShape it(mod); it.More(); it.Next()) {
        int r = index_in(result_faces, it.Value());
        if (r >= 0) out.inherited.push_back({(uint32_t)r, operand, (uint32_t)(i - 1)});
      }
    }
  }
}

// The lowest-numbered face of `faces` that contains three sample points of `e`, or a null shape.
TopoDS_Shape face_containing(const TopTools_IndexedMapOfShape& faces, const TopoDS_Edge& e) {
  BRepAdaptor_Curve c(e);
  double t0 = c.FirstParameter(), t1 = c.LastParameter();
  gp_Pnt samples[3] = {c.Value(t0 + 0.25 * (t1 - t0)), c.Value(t0 + 0.5 * (t1 - t0)),
                       c.Value(t0 + 0.75 * (t1 - t0))};
  for (int i = 1; i <= faces.Extent(); i++) {
    bool all = true;
    for (const gp_Pnt& p : samples) {
      BRepExtrema_DistShapeShape d(BRepBuilderAPI_MakeVertex(p).Vertex(), faces(i));
      if (!d.IsDone() || d.Value() > 1e-6) {
        all = false;
        break;
      }
    }
    if (all) return faces(i);
  }
  return TopoDS_Shape();
}

// Roles of the sides of a swept profile: `Generated` of each profile edge.
template <typename Maker>
void created_sides(Shape& out, Maker& mk, const EdgeGrid& grid,
                   const TopTools_IndexedMapOfShape& faces) {
  for (size_t l = 0; l < grid.size(); l++) {
    for (size_t g = 0; g < grid[l].size(); g++) {
      const TopTools_ListOfShape& gl = mk.Generated(grid[l][g]);
      if (!gl.IsEmpty()) {
        add_created_list(out, faces, gl, kSide, (uint32_t)l, (uint32_t)g);
      } else {
        // some sweeps (a segment perpendicular to the axis of a revolution) report nothing:
        // find the face the swept segment lies on
        add_created(out, faces, face_containing(faces, grid[l][g]), kSide, (uint32_t)l, (uint32_t)g);
      }
    }
  }
}

}  // namespace

rust::String occt_version() { return rust::String(OCC_VERSION_STRING_EXT); }

std::unique_ptr<Shape> clone_shape(const Shape& s) {
  auto out = wrap(s.s);
  out->inherited = s.inherited;
  out->created = s.created;
  return out;
}

std::unique_ptr<Shape> extrude(const ProfileData& p, Vec3 dir, double distance, double draft) {
  return guarded("extrude", [&]() {
    EdgeGrid grid;
    TopoDS_Face face = make_face(p, &grid);
    if (draft == 0.0) {
      gp_Vec v(dir.x * distance, dir.y * distance, dir.z * distance);
      BRepPrimAPI_MakePrism mk(face, v);
      mk.Build();
      if (!mk.IsDone()) throw std::runtime_error("prism failed");
      auto out = wrap(orient_positive(mk.Shape()));
      auto faces = face_map(out->s);
      add_created(*out, faces, mk.FirstShape(), kCapStart, 0, 0);
      add_created(*out, faces, mk.LastShape(), kCapEnd, 0, 0);
      created_sides(*out, mk, grid, faces);
      return out;
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
    auto out = wrap(orient_positive(dp.Shape()));
    auto faces = face_map(out->s);
    add_created(*out, faces, dp.FirstShape(), kCapStart, 0, 0);
    add_created(*out, faces, dp.LastShape(), kCapEnd, 0, 0);
    for (size_t l = 0; l < grid.size(); l++) {
      for (size_t g = 0; g < grid[l].size(); g++) {
        add_created_list(*out, faces, dp.Shapes(grid[l][g]), kSide, (uint32_t)l, (uint32_t)g);
      }
    }
    return out;
  });
}

std::unique_ptr<Shape> revolve(const ProfileData& p, Vec3 origin, Vec3 dir, double angle) {
  return guarded("revolve", [&]() {
    EdgeGrid grid;
    TopoDS_Face face = make_face(p, &grid);
    gp_Ax1 axis(P(origin), D(dir));
    bool full = angle >= 2.0 * M_PI - 1e-12;
    BRepPrimAPI_MakeRevol mk = full ? BRepPrimAPI_MakeRevol(face, axis)
                                    : BRepPrimAPI_MakeRevol(face, axis, angle);
    mk.Build();
    if (!mk.IsDone()) throw std::runtime_error("revolve failed");
    auto out = wrap(orient_positive(mk.Shape()));
    auto faces = face_map(out->s);
    if (!full) {
      add_created(*out, faces, mk.FirstShape(), kCapStart, 0, 0);
      add_created(*out, faces, mk.LastShape(), kCapEnd, 0, 0);
    }
    created_sides(*out, mk, grid, faces);
    return out;
  });
}

std::unique_ptr<Shape> sweep(const ProfileData& p, const PathData& path, const PathData& guide,
                             bool has_guide) {
  return guarded("sweep", [&]() {
    TopoDS_Wire spine = make_wire(path.frame, path.segs);
    EdgeGrid grid;
    if (!has_guide) {
      BRepOffsetAPI_MakePipe mk(spine, make_face(p, &grid));
      mk.Build();
      if (!mk.IsDone()) throw std::runtime_error("pipe failed");
      auto out = wrap(orient_positive(mk.Shape()));
      auto faces = face_map(out->s);
      add_created(*out, faces, mk.FirstShape(), kCapStart, 0, 0);
      add_created(*out, faces, mk.LastShape(), kCapEnd, 0, 0);
      created_sides(*out, mk, grid, faces);
      return out;
    }
    if (p.loops.size() != 1) throw std::runtime_error("a guided sweep needs a profile without holes");
    TopoDS_Wire aux = make_wire(guide.frame, guide.segs);
    grid.resize(1);
    TopoDS_Wire prof = make_wire(p.frame, p.loops[0].segs, &grid[0]);
    BRepOffsetAPI_MakePipeShell mk(spine);
    mk.SetMode(aux, Standard_True, BRepFill_NoContact);
    mk.Add(prof);
    mk.Build();
    if (!mk.IsDone()) throw std::runtime_error("guided pipe failed");
    if (!mk.MakeSolid()) throw std::runtime_error("guided pipe did not close into a solid");
    auto out = wrap(orient_positive(mk.Shape()));
    auto faces = face_map(out->s);
    add_created(*out, faces, mk.FirstShape(), kCapStart, 0, 0);
    add_created(*out, faces, mk.LastShape(), kCapEnd, 0, 0);
    created_sides(*out, mk, grid, faces);
    return out;
  });
}

std::unique_ptr<Shape> loft(const rust::Vec<ProfileData>& sections, bool ruled) {
  return guarded("loft", [&]() {
    BRepOffsetAPI_ThruSections gen(Standard_True, ruled ? Standard_True : Standard_False, Precision::Confusion());
    gen.CheckCompatibility(Standard_False);
    EdgeGrid grid;
    grid.resize(1);
    bool first = true;
    for (const ProfileData& s : sections) {
      if (s.loops.size() != 1) throw std::runtime_error("loft sections must not have holes");
      if (first) {
        gen.AddWire(make_wire(s.frame, s.loops[0].segs, &grid[0]));
        first = false;
      } else {
        gen.AddWire(outer_wire(s));
      }
    }
    gen.Build();
    if (!gen.IsDone()) throw std::runtime_error("loft failed");
    auto out = wrap(orient_positive(gen.Shape()));
    auto faces = face_map(out->s);
    add_created(*out, faces, gen.FirstShape(), kCapStart, 0, 0);
    add_created(*out, faces, gen.LastShape(), kCapEnd, 0, 0);
    for (size_t g = 0; g < grid[0].size(); g++) {
      add_created(*out, faces, gen.GeneratedFace(grid[0][g]), kSide, 0, (uint32_t)g);
    }
    return out;
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
      ShapeUpgrade_UnifySameDomain u(alg.Shape(), Standard_True, Standard_True, Standard_False);
      u.Build();
      auto out = wrap(u.Shape());
      auto faces = face_map(out->s);
      Handle(BRepTools_History) uh = u.History();
      // input face -> result faces, through the boolean and then the face merge
      auto trace = [&](const TopoDS_Shape& input, uint32_t operand) {
        TopTools_IndexedMapOfShape in_faces = face_map(input);
        for (int i = 1; i <= in_faces.Extent(); i++) {
          const TopoDS_Shape& f = in_faces(i);
          if (alg.IsDeleted(f)) continue;
          std::vector<TopoDS_Shape> cur;
          const TopTools_ListOfShape& mod = alg.Modified(f);
          if (mod.IsEmpty()) {
            cur.push_back(f);
          } else {
            for (TopTools_ListIteratorOfListOfShape it(mod); it.More(); it.Next()) cur.push_back(it.Value());
          }
          for (const TopoDS_Shape& c : cur) {
            std::vector<TopoDS_Shape> fin;
            if (!uh.IsNull() && uh->IsRemoved(c)) continue;
            if (!uh.IsNull() && !uh->Modified(c).IsEmpty()) {
              for (TopTools_ListIteratorOfListOfShape it(uh->Modified(c)); it.More(); it.Next()) fin.push_back(it.Value());
            } else {
              fin.push_back(c);
            }
            for (const TopoDS_Shape& r : fin) {
              int ri = index_in(faces, r);
              if (ri >= 0) out->inherited.push_back({(uint32_t)ri, (uint8_t)operand, (uint32_t)(i - 1)});
            }
          }
        }
      };
      trace(a.s, 0);
      trace(b.s, 1);
      return out;
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

// ---- detail features ----

TopoDS_Shape pick(const TopTools_IndexedMapOfShape& m, uint32_t i, const char* what) {
  if ((int)i >= m.Extent()) throw std::runtime_error(std::string(what) + " index out of range");
  return m((int)i + 1);
}

std::unique_ptr<Shape> fillet(const Shape& sh, rust::Slice<const uint32_t> edges,
                              rust::Slice<const double> radii) {
  return guarded("fillet", [&]() {
    TopTools_IndexedMapOfShape em = edge_map(sh.s);
    BRepFilletAPI_MakeFillet mk(sh.s);
    for (size_t i = 0; i < edges.size(); i++) {
      TopoDS_Edge e = TopoDS::Edge(pick(em, edges[i], "edge"));
      double r1 = radii[2 * i], r2 = radii[2 * i + 1];
      if (std::fabs(r1 - r2) < 1e-12) mk.Add(r1, e); else mk.Add(r1, r2, e);
    }
    mk.Build();
    if (!mk.IsDone()) throw std::runtime_error("fillet failed");
    auto out = wrap(mk.Shape());
    auto faces = face_map(out->s);
    inherit_from(*out, mk, sh.s, 0, faces);
    for (size_t i = 0; i < edges.size(); i++) {
      add_created_list(*out, faces, mk.Generated(pick(em, edges[i], "edge")), kFromEdge, edges[i], 0);
    }
    return out;
  });
}

std::unique_ptr<Shape> chamfer(const Shape& sh, rust::Slice<const uint32_t> edges,
                               rust::Slice<const uint32_t> faces_in, rust::Slice<const double> dists) {
  return guarded("chamfer", [&]() {
    TopTools_IndexedMapOfShape em = edge_map(sh.s), fm = face_map(sh.s);
    BRepFilletAPI_MakeChamfer mk(sh.s);
    for (size_t i = 0; i < edges.size(); i++) {
      mk.Add(dists[2 * i], dists[2 * i + 1], TopoDS::Edge(pick(em, edges[i], "edge")),
             TopoDS::Face(pick(fm, faces_in[i], "face")));
    }
    mk.Build();
    if (!mk.IsDone()) throw std::runtime_error("chamfer failed");
    auto out = wrap(mk.Shape());
    auto faces = face_map(out->s);
    inherit_from(*out, mk, sh.s, 0, faces);
    for (size_t i = 0; i < edges.size(); i++) {
      add_created_list(*out, faces, mk.Generated(pick(em, edges[i], "edge")), kFromEdge, edges[i], 0);
    }
    return out;
  });
}

std::unique_ptr<Shape> shell(const Shape& sh, rust::Slice<const uint32_t> remove, double thickness,
                             bool outward, double tolerance) {
  return guarded("shell", [&]() {
    TopTools_IndexedMapOfShape fm = face_map(sh.s);
    TopTools_ListOfShape closing;
    for (uint32_t i : remove) closing.Append(pick(fm, i, "face"));
    BRepOffsetAPI_MakeThickSolid mk;
    mk.MakeThickSolidByJoin(sh.s, closing, outward ? thickness : -thickness, tolerance,
                            BRepOffset_Skin, Standard_True, Standard_False, GeomAbs_Intersection);
    mk.Build();
    if (!mk.IsDone()) throw std::runtime_error("shell failed");
    auto out = wrap(mk.Shape());
    auto faces = face_map(out->s);
    inherit_from(*out, mk, sh.s, 0, faces);
    // the walls on the inside are generated from the faces that stayed
    std::vector<bool> removed(fm.Extent(), false);
    for (uint32_t i : remove) removed[i] = true;
    for (int i = 1; i <= fm.Extent(); i++) {
      if (removed[i - 1]) continue;
      add_created_list(*out, faces, mk.Generated(fm(i)), kFromFace, (uint32_t)(i - 1), 0);
    }
    TopTools_IndexedMapOfShape em = edge_map(sh.s);
    for (int i = 1; i <= em.Extent(); i++) {
      add_created_list(*out, faces, mk.Generated(em(i)), kFromEdge, (uint32_t)(i - 1), 0);
    }
    return out;
  });
}

std::unique_ptr<Shape> draft(const Shape& sh, rust::Slice<const uint32_t> faces_in, Vec3 pull,
                             double angle, Vec3 neutral_origin, Vec3 neutral_normal) {
  return guarded("draft", [&]() {
    TopTools_IndexedMapOfShape fm = face_map(sh.s);
    BRepOffsetAPI_DraftAngle mk(sh.s);
    gp_Pln neutral(P(neutral_origin), D(neutral_normal));
    for (uint32_t i : faces_in) {
      mk.Add(TopoDS::Face(pick(fm, i, "face")), D(pull), angle, neutral);
      if (!mk.AddDone()) throw std::runtime_error("a face cannot be drafted");
    }
    mk.Build();
    if (!mk.IsDone()) throw std::runtime_error("draft failed");
    auto out = wrap(mk.Shape());
    auto faces = face_map(out->s);
    inherit_from(*out, mk, sh.s, 0, faces);
    return out;
  });
}

std::unique_ptr<Shape> moved(const Shape& sh, const gp_Trsf& t) {
  BRepBuilderAPI_Transform mk(sh.s, t, Standard_True);
  mk.Build();
  if (!mk.IsDone()) throw std::runtime_error("transform failed");
  auto out = wrap(mk.Shape());
  auto faces = face_map(out->s);
  TopTools_IndexedMapOfShape in_faces = face_map(sh.s);
  for (int i = 1; i <= in_faces.Extent(); i++) {
    int r = index_in(faces, mk.ModifiedShape(in_faces(i)));
    if (r >= 0) out->inherited.push_back({(uint32_t)r, 0, (uint32_t)(i - 1)});
  }
  return out;
}

std::unique_ptr<Shape> translate(const Shape& sh, Vec3 v) {
  return guarded("translate", [&]() {
    gp_Trsf t;
    t.SetTranslation(gp_Vec(v.x, v.y, v.z));
    return moved(sh, t);
  });
}

std::unique_ptr<Shape> rotate(const Shape& sh, Vec3 origin, Vec3 dir, double angle) {
  return guarded("rotate", [&]() {
    gp_Trsf t;
    t.SetRotation(gp_Ax1(P(origin), D(dir)), angle);
    return moved(sh, t);
  });
}

std::unique_ptr<Shape> mirror(const Shape& sh, Vec3 origin, Vec3 normal) {
  return guarded("mirror", [&]() {
    gp_Trsf t;
    t.SetMirror(gp_Ax2(P(origin), D(normal)));
    return moved(sh, t);
  });
}

// ---- topology listing ----

rust::Vec<FaceRec> faces_of(const Shape& sh) {
  return guarded("faces", [&]() {
    TopTools_IndexedMapOfShape fm = face_map(sh.s), em = edge_map(sh.s);
    rust::Vec<FaceRec> out;
    for (int i = 1; i <= fm.Extent(); i++) {
      TopoDS_Face f = TopoDS::Face(fm(i));
      FaceRec r;
      r.has = 0;
      r.radius = 0;
      r.normal = Vec3{0, 0, 0};
      r.axis_origin = Vec3{0, 0, 0};
      r.axis_dir = Vec3{0, 0, 0};
      BRepAdaptor_Surface surf(f);
      switch (surf.GetType()) {
        case GeomAbs_Plane: {
          r.kind = 0;
          gp_Dir n = surf.Plane().Axis().Direction();
          if (f.Orientation() == TopAbs_REVERSED) n.Reverse();
          r.normal = Vec3{n.X(), n.Y(), n.Z()};
          r.has |= 1;
          break;
        }
        case GeomAbs_Cylinder: {
          r.kind = 1;
          gp_Cylinder c = surf.Cylinder();
          r.radius = c.Radius();
          r.has |= 4;
          gp_Ax1 ax = c.Axis();
          r.axis_origin = Vec3{ax.Location().X(), ax.Location().Y(), ax.Location().Z()};
          r.axis_dir = Vec3{ax.Direction().X(), ax.Direction().Y(), ax.Direction().Z()};
          r.has |= 2;
          break;
        }
        case GeomAbs_Cone: {
          r.kind = 2;
          gp_Cone c = surf.Cone();
          r.radius = c.RefRadius();
          r.has |= 4;
          gp_Ax1 ax = c.Axis();
          r.axis_origin = Vec3{ax.Location().X(), ax.Location().Y(), ax.Location().Z()};
          r.axis_dir = Vec3{ax.Direction().X(), ax.Direction().Y(), ax.Direction().Z()};
          r.has |= 2;
          break;
        }
        case GeomAbs_Sphere: {
          r.kind = 3;
          r.radius = surf.Sphere().Radius();
          r.has |= 4;
          gp_Pnt c = surf.Sphere().Location();
          r.axis_origin = Vec3{c.X(), c.Y(), c.Z()};
          break;
        }
        case GeomAbs_Torus: {
          r.kind = 4;
          gp_Torus t = surf.Torus();
          r.radius = t.MajorRadius();
          r.has |= 4;
          gp_Ax1 ax = t.Axis();
          r.axis_origin = Vec3{ax.Location().X(), ax.Location().Y(), ax.Location().Z()};
          r.axis_dir = Vec3{ax.Direction().X(), ax.Direction().Y(), ax.Direction().Z()};
          r.has |= 2;
          break;
        }
        default:
          r.kind = 5;
      }
      GProp_GProps g;
      BRepGProp::SurfaceProperties(f, g);
      r.area = g.Mass();
      gp_Pnt c = g.CentreOfMass();
      r.centroid = Vec3{c.X(), c.Y(), c.Z()};
      TopTools_IndexedMapOfShape fe;
      TopExp::MapShapes(f, TopAbs_EDGE, fe);
      for (int k = 1; k <= fe.Extent(); k++) {
        int ei = index_in(em, fe(k));
        if (ei >= 0) r.edges.push_back((uint32_t)ei);
      }
      out.push_back(std::move(r));
    }
    return out;
  });
}

rust::Vec<EdgeRec> edges_of(const Shape& sh) {
  return guarded("edges", [&]() {
    TopTools_IndexedMapOfShape fm = face_map(sh.s), em = edge_map(sh.s);
    TopTools_IndexedDataMapOfShapeListOfShape anc;
    TopExp::MapShapesAndAncestors(sh.s, TopAbs_EDGE, TopAbs_FACE, anc);
    rust::Vec<EdgeRec> out;
    for (int i = 1; i <= em.Extent(); i++) {
      TopoDS_Edge e = TopoDS::Edge(em(i));
      EdgeRec r;
      r.has = 0;
      r.radius = 0;
      r.direction = Vec3{0, 0, 0};
      BRepAdaptor_Curve c(e);
      double t0 = c.FirstParameter(), t1 = c.LastParameter();
      gp_Pnt a = c.Value(t0), b = c.Value(t1), m = c.Value(0.5 * (t0 + t1));
      r.start = Vec3{a.X(), a.Y(), a.Z()};
      r.end = Vec3{b.X(), b.Y(), b.Z()};
      r.mid = Vec3{m.X(), m.Y(), m.Z()};
      r.length = GCPnts_AbscissaPoint::Length(c);
      switch (c.GetType()) {
        case GeomAbs_Line: {
          r.kind = 0;
          gp_Dir d = c.Line().Direction();
          if (e.Orientation() == TopAbs_REVERSED) d.Reverse();
          r.direction = Vec3{d.X(), d.Y(), d.Z()};
          r.has |= 1;
          break;
        }
        case GeomAbs_Circle:
          r.kind = 1;
          r.radius = c.Circle().Radius();
          r.has |= 2;
          break;
        case GeomAbs_Ellipse:
          r.kind = 2;
          break;
        default:
          r.kind = 3;
      }
      if (anc.Contains(e)) {
        std::vector<uint32_t> seen;
        for (TopTools_ListIteratorOfListOfShape it(anc.FindFromKey(e)); it.More(); it.Next()) {
          int fi = index_in(fm, it.Value());
          if (fi >= 0 && std::find(seen.begin(), seen.end(), (uint32_t)fi) == seen.end()) seen.push_back((uint32_t)fi);
        }
        std::sort(seen.begin(), seen.end());
        for (uint32_t fi : seen) r.faces.push_back(fi);
      }
      out.push_back(std::move(r));
    }
    return out;
  });
}

rust::Vec<uint32_t> lineage_inherited(const Shape& sh) {
  rust::Vec<uint32_t> out;
  for (const auto& t : sh.inherited) for (uint32_t v : t) out.push_back(v);
  return out;
}

rust::Vec<uint32_t> lineage_created(const Shape& sh) {
  rust::Vec<uint32_t> out;
  for (const auto& t : sh.created) for (uint32_t v : t) out.push_back(v);
  return out;
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

// ---- STEP and meshing ----

namespace {

// A scratch file name unique per call (OpenCASCADE's STEP writer works on files).
std::string scratch_path(const char* ext) {
  static unsigned counter = 0;
  std::random_device rd;
  std::filesystem::path dir = std::filesystem::temp_directory_path();
  return (dir / ("tcad-" + std::to_string(rd()) + "-" + std::to_string(counter++) + ext)).string();
}

// OpenCASCADE's STEP and XDE machinery keeps global state (interface parameters, the shared
// application object), so concurrent exports and imports corrupt each other. One at a time.
std::mutex& step_mutex() {
  static std::mutex m;
  return m;
}

struct Scratch {
  std::string path;
  explicit Scratch(const char* ext) : path(scratch_path(ext)) {}
  ~Scratch() {
    std::error_code ec;
    std::filesystem::remove(path, ec);
  }
};

}  // namespace

rust::Vec<uint8_t> export_step(const Shape& sh, rust::Str name, rust::Str configuration,
                               rust::Str material, double density_g_cm3) {
  return guarded("export_step", [&]() {
    std::lock_guard<std::mutex> lock(step_mutex());
    Handle(XCAFApp_Application) app = XCAFApp_Application::GetApplication();
    Handle(TDocStd_Document) doc;
    app->NewDocument("MDTV-XCAF", doc);
    Handle(XCAFDoc_ShapeTool) st = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
    // work on a copy: the writer may adjust tolerances, and the caller's shape must stay intact
    BRepBuilderAPI_Copy copy(sh.s);
    TDF_Label label = st->AddShape(copy.Shape(), Standard_False);
    std::string part(name.data(), name.size());
    TDataStd_Name::Set(label, TCollection_ExtendedString(part.c_str()));
    if (material.size() > 0) {
      std::string mat(material.data(), material.size());
      Handle(XCAFDoc_MaterialTool) mt = XCAFDoc_DocumentTool::MaterialTool(doc->Main());
      TDF_Label m = mt->AddMaterial(new TCollection_HAsciiString(mat.c_str()),
                                    new TCollection_HAsciiString(""), density_g_cm3,
                                    new TCollection_HAsciiString("density"),
                                    new TCollection_HAsciiString("g/cm^3"));
      mt->SetMaterial(label, m);
    }
    Interface_Static::SetCVal("write.step.schema", "AP242DIS");
    Interface_Static::SetCVal("write.step.unit", "MM");
    STEPCAFControl_Writer writer;
    writer.SetNameMode(Standard_True);
    writer.SetMaterialMode(Standard_True);
    if (!writer.Transfer(doc, STEPControl_AsIs)) throw std::runtime_error("STEP transfer failed");
    Handle(StepData_StepModel) model = writer.ChangeWriter().Model();
    if (!model.IsNull()) {
      APIHeaderSection_MakeHeader header(model);
      std::string desc = "TextCAD part " + part;
      if (configuration.size() > 0) desc += ", configuration " + std::string(configuration.data(), configuration.size());
      header.SetDescriptionValue(1, new TCollection_HAsciiString(desc.c_str()));
    }
    Scratch tmp(".step");
    if (writer.Write(tmp.path.c_str()) != IFSelect_RetDone) throw std::runtime_error("STEP write failed");
    std::ifstream in(tmp.path, std::ios::binary);
    std::string text((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
    rust::Vec<uint8_t> out;
    out.reserve(text.size());
    for (unsigned char ch : text) out.push_back(ch);
    app->Close(doc);
    return out;
  });
}

std::unique_ptr<Shape> import_step(rust::Slice<const uint8_t> bytes) {
  return guarded("import_step", [&]() {
    std::lock_guard<std::mutex> lock(step_mutex());
    std::string text(reinterpret_cast<const char*>(bytes.data()), bytes.size());
    std::istringstream is(text);
    STEPControl_Reader reader;
    if (reader.ReadStream("import.step", is) != IFSelect_RetDone) {
      throw std::runtime_error("not a readable STEP file");
    }
    reader.TransferRoots();
    TopoDS_Shape all = reader.OneShape();
    if (all.IsNull()) throw std::runtime_error("the STEP file contains no shape");
    TopTools_IndexedMapOfShape solids;
    TopExp::MapShapes(all, TopAbs_SOLID, solids);
    if (solids.Extent() == 0) throw std::runtime_error("the STEP file contains no solid");
    if (solids.Extent() == 1) return wrap(solids(1));
    BRep_Builder b;
    TopoDS_Compound comp;
    b.MakeCompound(comp);
    for (int i = 1; i <= solids.Extent(); i++) b.Add(comp, solids(i));
    return wrap(comp);
  });
}

rust::Vec<MeshRec> mesh_of(const Shape& sh, double linear, double angular) {
  return guarded("mesh", [&]() {
    // mesh a copy: triangulations are attached to the shape, which must stay as it was built
    BRepBuilderAPI_Copy copy(sh.s);
    TopoDS_Shape work = copy.Shape();
    BRepMesh_IncrementalMesh mesher(work, linear, Standard_False, angular, Standard_False);
    TopTools_IndexedMapOfShape fm = face_map(work);
    rust::Vec<MeshRec> out;
    for (int i = 1; i <= fm.Extent(); i++) {
      TopoDS_Face face = TopoDS::Face(fm(i));
      TopLoc_Location loc;
      Handle(Poly_Triangulation) tri = BRep_Tool::Triangulation(face, loc);
      if (tri.IsNull() || tri->NbTriangles() == 0) continue;
      if (!tri->HasNormals()) tri->ComputeNormals();
      gp_Trsf t = loc.Transformation();
      bool reversed = face.Orientation() == TopAbs_REVERSED;
      MeshRec r;
      r.face = (uint32_t)(i - 1);
      for (int n = 1; n <= tri->NbNodes(); n++) {
        gp_Pnt p = tri->Node(n).Transformed(t);
        r.positions.push_back((float)p.X());
        r.positions.push_back((float)p.Y());
        r.positions.push_back((float)p.Z());
        gp_Dir d = tri->Normal(n);
        d.Transform(t);
        if (reversed) d.Reverse();
        r.normals.push_back((float)d.X());
        r.normals.push_back((float)d.Y());
        r.normals.push_back((float)d.Z());
      }
      for (int k = 1; k <= tri->NbTriangles(); k++) {
        int a, b, c;
        tri->Triangle(k).Get(a, b, c);
        if (reversed) std::swap(b, c);
        r.triangles.push_back((uint32_t)(a - 1));
        r.triangles.push_back((uint32_t)(b - 1));
        r.triangles.push_back((uint32_t)(c - 1));
      }
      out.push_back(std::move(r));
    }
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
