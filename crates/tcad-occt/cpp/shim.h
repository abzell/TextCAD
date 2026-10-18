// C++ side of the OpenCASCADE backend. The shape handle is a thin wrapper around TopoDS_Shape.
#pragma once

#include <cstdint>
#include <memory>

#include "rust/cxx.h"
#include <TopoDS_Shape.hxx>

namespace tcad {

struct Vec3;
struct Seg;
struct ProfileData;
struct PathData;
struct Props;

struct Shape {
  TopoDS_Shape s;
};

rust::String occt_version();

std::unique_ptr<Shape> clone_shape(const Shape& s);
std::unique_ptr<Shape> extrude(const ProfileData& p, Vec3 dir, double distance, double draft);
std::unique_ptr<Shape> revolve(const ProfileData& p, Vec3 origin, Vec3 dir, double angle);
std::unique_ptr<Shape> sweep(const ProfileData& p, const PathData& path, const PathData& guide,
                             bool has_guide);
std::unique_ptr<Shape> loft(const rust::Vec<ProfileData>& sections, bool ruled);
std::unique_ptr<Shape> boolean_op(uint32_t op, const Shape& a, const Shape& b, double tolerance);
Props properties(const Shape& s);
rust::Vec<uint8_t> write_brep(const Shape& s);
std::unique_ptr<Shape> read_brep(rust::Slice<const uint8_t> bytes);

}  // namespace tcad
