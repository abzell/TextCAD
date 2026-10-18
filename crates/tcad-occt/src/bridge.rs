//! The `cxx` bridge to the C++ shim (`cpp/shim.cc`).

#[cxx::bridge(namespace = "tcad")]
pub mod ffi {
    /// A 3D vector or point.
    #[derive(Clone, Copy, Debug, Default)]
    struct Vec3 {
        x: f64,
        y: f64,
        z: f64,
    }

    /// An orthonormal frame.
    #[derive(Clone, Copy, Debug, Default)]
    struct Frame3 {
        o: Vec3,
        x: Vec3,
        y: Vec3,
        z: Vec3,
    }

    /// One 2D segment. `kind`: 0 line (`p[0..2]` to `p[2..4]`), 1 arc (start, middle, end in
    /// `p`), 2 circle (center `p[0..2]`, radius `p[2]`), 3 ellipse (center, a, b, theta in `p`),
    /// 4 spline (`pts` flat x,y pairs).
    #[derive(Clone, Debug, Default)]
    struct Seg {
        kind: u32,
        p: [f64; 6],
        pts: Vec<f64>,
        interpolated: bool,
        closed: bool,
    }

    /// A closed wire.
    #[derive(Clone, Debug, Default)]
    struct LoopData {
        segs: Vec<Seg>,
    }

    /// A planar face: `loops[0]` is the outer boundary, the rest are holes.
    #[derive(Clone, Debug, Default)]
    struct ProfileData {
        frame: Frame3,
        loops: Vec<LoopData>,
    }

    /// An open or closed chain.
    #[derive(Clone, Debug, Default)]
    struct PathData {
        frame: Frame3,
        segs: Vec<Seg>,
    }

    /// Measurements of a shape.
    #[derive(Clone, Debug, Default)]
    struct Props {
        volume: f64,
        area: f64,
        com: Vec3,
        inertia: [f64; 9],
        bmin: Vec3,
        bmax: Vec3,
        solids: u32,
        faces: u32,
        edges: u32,
        vertices: u32,
    }

    unsafe extern "C++" {
        include!("cpp/shim.h");

        type Shape;

        fn occt_version() -> String;
        fn clone_shape(s: &Shape) -> Result<UniquePtr<Shape>>;
        fn extrude(
            p: &ProfileData,
            dir: Vec3,
            distance: f64,
            draft: f64,
        ) -> Result<UniquePtr<Shape>>;
        fn revolve(
            p: &ProfileData,
            origin: Vec3,
            dir: Vec3,
            angle: f64,
        ) -> Result<UniquePtr<Shape>>;
        fn sweep(
            p: &ProfileData,
            path: &PathData,
            guide: &PathData,
            has_guide: bool,
        ) -> Result<UniquePtr<Shape>>;
        fn loft(sections: &Vec<ProfileData>, ruled: bool) -> Result<UniquePtr<Shape>>;
        fn boolean_op(op: u32, a: &Shape, b: &Shape, tolerance: f64) -> Result<UniquePtr<Shape>>;
        fn properties(s: &Shape) -> Result<Props>;
        fn write_brep(s: &Shape) -> Result<Vec<u8>>;
        fn read_brep(bytes: &[u8]) -> Result<UniquePtr<Shape>>;
    }
}
