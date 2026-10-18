//! The [`Kernel`] implementation over the cxx bridge.

use crate::bridge::ffi;
use cxx::UniquePtr;
use tcad_kernel::geometry::{Frame, Path, Profile, Segment, V3};
use tcad_kernel::{BoolOp, KResult, Kernel, KernelError, Properties, Topology};

/// The OpenCASCADE backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct OcctKernel;

/// A shape owned by OpenCASCADE.
pub struct OcctShape(UniquePtr<ffi::Shape>);

impl Clone for OcctShape {
    fn clone(&self) -> Self {
        // TopoDS shapes are reference-counted handles, so this is cheap.
        OcctShape(ffi::clone_shape(&self.0).expect("cloning a shape cannot fail"))
    }
}

impl OcctKernel {
    /// Creates the backend.
    pub fn new() -> Self {
        OcctKernel
    }
}

fn err(e: cxx::Exception) -> KernelError {
    KernelError(e.what().to_string())
}

fn v3(v: V3) -> ffi::Vec3 {
    ffi::Vec3 {
        x: v[0],
        y: v[1],
        z: v[2],
    }
}

fn frame(f: &Frame) -> ffi::Frame3 {
    ffi::Frame3 {
        o: v3(f.origin),
        x: v3(f.x),
        y: v3(f.y),
        z: v3(f.z),
    }
}

fn seg(s: &Segment) -> ffi::Seg {
    let mut out = ffi::Seg {
        kind: 0,
        p: [0.0; 6],
        pts: Vec::new(),
        interpolated: false,
        closed: false,
    };
    match s {
        Segment::Line { a, b } => out.p = [a[0], a[1], b[0], b[1], 0.0, 0.0],
        Segment::Arc {
            start, mid, end, ..
        } => {
            out.kind = 1;
            out.p = [start[0], start[1], mid[0], mid[1], end[0], end[1]];
        }
        Segment::Circle { center, r } => {
            out.kind = 2;
            out.p = [center[0], center[1], *r, 0.0, 0.0, 0.0];
        }
        Segment::Ellipse {
            center,
            a,
            b,
            theta,
        } => {
            out.kind = 3;
            out.p = [center[0], center[1], *a, *b, *theta, 0.0];
        }
        Segment::Spline {
            points,
            interpolated,
            closed,
        } => {
            out.kind = 4;
            out.pts = points.iter().flat_map(|p| [p[0], p[1]]).collect();
            out.interpolated = *interpolated;
            out.closed = *closed;
        }
    }
    out
}

fn profile(p: &Profile) -> ffi::ProfileData {
    let mut loops = vec![ffi::LoopData {
        segs: p.outer.segments.iter().map(seg).collect(),
    }];
    loops.extend(p.holes.iter().map(|h| ffi::LoopData {
        segs: h.segments.iter().map(seg).collect(),
    }));
    ffi::ProfileData {
        frame: frame(&p.frame),
        loops,
    }
}

fn path(p: &Path) -> ffi::PathData {
    ffi::PathData {
        frame: frame(&p.frame),
        segs: p.segments.iter().map(seg).collect(),
    }
}

fn shape(r: Result<UniquePtr<ffi::Shape>, cxx::Exception>) -> KResult<OcctShape> {
    r.map(OcctShape).map_err(err)
}

impl Kernel for OcctKernel {
    type Shape = OcctShape;

    fn id(&self) -> String {
        format!("occt-{}", ffi::occt_version())
    }

    fn extrude(&self, p: &Profile, dir: V3, distance: f64, draft: f64) -> KResult<OcctShape> {
        shape(ffi::extrude(&profile(p), v3(dir), distance, draft))
    }

    fn revolve(
        &self,
        p: &Profile,
        axis_origin: V3,
        axis_dir: V3,
        angle: f64,
    ) -> KResult<OcctShape> {
        shape(ffi::revolve(
            &profile(p),
            v3(axis_origin),
            v3(axis_dir),
            angle,
        ))
    }

    fn sweep(&self, p: &Profile, spine: &Path, guide: Option<&Path>) -> KResult<OcctShape> {
        let empty = ffi::PathData::default();
        let g = guide.map(path);
        shape(ffi::sweep(
            &profile(p),
            &path(spine),
            g.as_ref().unwrap_or(&empty),
            g.is_some(),
        ))
    }

    fn loft(&self, sections: &[Profile], ruled: bool) -> KResult<OcctShape> {
        let data: Vec<ffi::ProfileData> = sections.iter().map(profile).collect();
        shape(ffi::loft(&data, ruled))
    }

    fn boolean(
        &self,
        op: BoolOp,
        a: &OcctShape,
        b: &OcctShape,
        tolerance: f64,
    ) -> KResult<OcctShape> {
        let code = match op {
            BoolOp::Union => 0,
            BoolOp::Subtract => 1,
            BoolOp::Intersect => 2,
        };
        shape(ffi::boolean_op(code, &a.0, &b.0, tolerance))
    }

    fn properties(&self, s: &OcctShape) -> KResult<Properties> {
        let p = ffi::properties(&s.0).map_err(err)?;
        let v = |v: &ffi::Vec3| [v.x, v.y, v.z];
        let i = p.inertia;
        Ok(Properties {
            volume: p.volume,
            area: p.area,
            center_of_mass: v(&p.com),
            inertia_com: Some([[i[0], i[1], i[2]], [i[3], i[4], i[5]], [i[6], i[7], i[8]]]),
            bbox_min: v(&p.bmin),
            bbox_max: v(&p.bmax),
            topology: Topology {
                solids: p.solids as usize,
                faces: p.faces as usize,
                edges: p.edges as usize,
                vertices: p.vertices as usize,
            },
        })
    }

    fn canonical_brep(&self, s: &OcctShape) -> KResult<Vec<u8>> {
        ffi::write_brep(&s.0).map_err(err)
    }

    fn from_brep(&self, bytes: &[u8]) -> KResult<OcctShape> {
        shape(ffi::read_brep(bytes))
    }
}
