// C++ side of the OpenCASCADE backend. The shape handle is a thin wrapper around TopoDS_Shape.
#pragma once

#include <array>
#include <cstdint>
#include <memory>
#include <vector>

#include "rust/cxx.h"
#include <TopoDS_Shape.hxx>

namespace tcad {

struct Vec3;
struct Seg;
struct ProfileData;
struct PathData;
struct Props;
struct FaceRec;
struct EdgeRec;
struct MeshRec;

struct Shape {
  TopoDS_Shape s;
  // Where the faces came from (see tcad_kernel::topo::Lineage): (result face, operand, input face)
  std::vector<std::array<uint32_t, 3>> inherited;
  // (result face, role, a, b)
  std::vector<std::array<uint32_t, 4>> created;
};

rust::String occt_version();

std::unique_ptr<Shape> clone_shape(const Shape& s);
std::unique_ptr<Shape> extrude(const ProfileData& p, Vec3 dir, double distance, double draft);
std::unique_ptr<Shape> revolve(const ProfileData& p, Vec3 origin, Vec3 dir, double angle);
std::unique_ptr<Shape> sweep(const ProfileData& p, const PathData& path, const PathData& guide,
                             bool has_guide);
std::unique_ptr<Shape> loft(const rust::Vec<ProfileData>& sections, bool ruled);
std::unique_ptr<Shape> loft_guided(const rust::Vec<ProfileData>& sections, const PathData& guide);
std::unique_ptr<Shape> boolean_op(uint32_t op, const Shape& a, const Shape& b, double tolerance);
std::unique_ptr<Shape> fillet(const Shape& s, rust::Slice<const uint32_t> edges,
                              rust::Slice<const double> radii);
std::unique_ptr<Shape> chamfer(const Shape& s, rust::Slice<const uint32_t> edges,
                               rust::Slice<const uint32_t> faces, rust::Slice<const double> dists);
std::unique_ptr<Shape> shell(const Shape& s, rust::Slice<const uint32_t> remove, double thickness,
                             bool outward, double tolerance);
std::unique_ptr<Shape> draft(const Shape& s, rust::Slice<const uint32_t> faces, Vec3 pull,
                             double angle, Vec3 neutral_origin, Vec3 neutral_normal);
std::unique_ptr<Shape> translate(const Shape& s, Vec3 v);
std::unique_ptr<Shape> rotate(const Shape& s, Vec3 origin, Vec3 q, double w);
std::unique_ptr<Shape> mirror(const Shape& s, Vec3 origin, Vec3 normal);
rust::Vec<FaceRec> faces_of(const Shape& s);
rust::Vec<EdgeRec> edges_of(const Shape& s);
rust::Vec<uint32_t> lineage_inherited(const Shape& s);
rust::Vec<uint32_t> lineage_created(const Shape& s);
rust::Vec<uint8_t> export_step(const Shape& s, rust::Str name, rust::Str configuration,
                               rust::Str material, double density_g_cm3);
std::unique_ptr<Shape> import_step(rust::Slice<const uint8_t> bytes);
rust::Vec<MeshRec> mesh_of(const Shape& s, double linear, double angular);
Props properties(const Shape& s);
rust::Vec<uint8_t> write_brep(const Shape& s);
std::unique_ptr<Shape> read_brep(rust::Slice<const uint8_t> bytes);

}  // namespace tcad
