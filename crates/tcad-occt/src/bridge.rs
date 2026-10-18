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

    /// One face. `kind`: 0 plane, 1 cylinder, 2 cone, 3 sphere, 4 torus, 5 other. `has` bit 0:
    /// normal valid, bit 1: axis valid, bit 2: radius valid.
    #[derive(Clone, Debug, Default)]
    struct FaceRec {
        kind: u32,
        has: u32,
        normal: Vec3,
        axis_origin: Vec3,
        axis_dir: Vec3,
        radius: f64,
        centroid: Vec3,
        area: f64,
        edges: Vec<u32>,
    }

    /// One edge. `kind`: 0 line, 1 circle, 2 ellipse, 3 other. `has` bit 0: direction valid,
    /// bit 1: radius valid.
    #[derive(Clone, Debug, Default)]
    struct EdgeRec {
        kind: u32,
        has: u32,
        start: Vec3,
        end: Vec3,
        mid: Vec3,
        length: f64,
        radius: f64,
        direction: Vec3,
        faces: Vec<u32>,
    }

    /// The triangles of one face.
    #[derive(Clone, Debug, Default)]
    struct MeshRec {
        face: u32,
        positions: Vec<f32>,
        normals: Vec<f32>,
        triangles: Vec<u32>,
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
        fn fillet(s: &Shape, edges: &[u32], radii: &[f64]) -> Result<UniquePtr<Shape>>;
        fn chamfer(
            s: &Shape,
            edges: &[u32],
            faces: &[u32],
            dists: &[f64],
        ) -> Result<UniquePtr<Shape>>;
        fn shell(
            s: &Shape,
            remove: &[u32],
            thickness: f64,
            outward: bool,
            tolerance: f64,
        ) -> Result<UniquePtr<Shape>>;
        fn draft(
            s: &Shape,
            faces: &[u32],
            pull: Vec3,
            angle: f64,
            neutral_origin: Vec3,
            neutral_normal: Vec3,
        ) -> Result<UniquePtr<Shape>>;
        fn translate(s: &Shape, v: Vec3) -> Result<UniquePtr<Shape>>;
        fn rotate(s: &Shape, origin: Vec3, dir: Vec3, angle: f64) -> Result<UniquePtr<Shape>>;
        fn mirror(s: &Shape, origin: Vec3, normal: Vec3) -> Result<UniquePtr<Shape>>;
        fn faces_of(s: &Shape) -> Result<Vec<FaceRec>>;
        fn edges_of(s: &Shape) -> Result<Vec<EdgeRec>>;
        fn lineage_inherited(s: &Shape) -> Vec<u32>;
        fn lineage_created(s: &Shape) -> Vec<u32>;
        fn export_step(
            s: &Shape,
            name: &str,
            configuration: &str,
            material: &str,
            density_g_cm3: f64,
        ) -> Result<Vec<u8>>;
        fn import_step(bytes: &[u8]) -> Result<UniquePtr<Shape>>;
        fn mesh_of(s: &Shape, linear: f64, angular: f64) -> Result<Vec<MeshRec>>;
        fn properties(s: &Shape) -> Result<Props>;
        fn write_brep(s: &Shape) -> Result<Vec<u8>>;
        fn read_brep(bytes: &[u8]) -> Result<UniquePtr<Shape>>;
    }
}
