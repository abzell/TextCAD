//! A mock backend for fast unit tests and for WASM builds before the in-browser kernel exists.
//!
//! It handles only what can be computed in closed form: [`Kernel::extrude`] of a profile whose
//! single outer loop consists of straight lines or one circle, with no holes and no draft. Anything
//! else is a [`KernelError`]. Booleans, revolves, sweeps and lofts are not available.

use crate::geometry::{Frame, Profile, Segment, V3, add, scale};
use crate::{
    BoolOp, ChamferEdge, FilletEdge, KResult, Kernel, KernelError, Lineage, Path, Properties,
    TopoInfo, Topology, Transform,
};
use std::f64::consts::PI;

/// The mock backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct MockKernel;

/// A prism: a profile swept along a direction.
#[derive(Clone, Debug, PartialEq)]
pub struct MockShape {
    frame: Frame,
    segments: Vec<Segment>,
    dir: V3,
    distance: f64,
}

fn unsupported<T>(what: &str) -> KResult<T> {
    Err(KernelError(format!(
        "the mock kernel does not support {what}"
    )))
}

impl Kernel for MockKernel {
    type Shape = MockShape;

    fn id(&self) -> String {
        "mock-0.1".to_string()
    }

    fn extrude(&self, profile: &Profile, dir: V3, distance: f64, draft: f64) -> KResult<MockShape> {
        if !profile.holes.is_empty() {
            return unsupported("profiles with holes");
        }
        if draft != 0.0 {
            return unsupported("draft");
        }
        let single_circle = matches!(profile.outer.segments.as_slice(), [Segment::Circle { .. }]);
        if !single_circle
            && !profile
                .outer
                .segments
                .iter()
                .all(|s| matches!(s, Segment::Line { .. }))
        {
            return unsupported("anything but straight-line or circular profiles");
        }
        Ok(MockShape {
            frame: profile.frame,
            segments: profile.outer.segments.clone(),
            dir,
            distance,
        })
    }

    fn revolve(&self, _: &Profile, _: V3, _: V3, _: f64) -> KResult<MockShape> {
        unsupported("revolve")
    }

    fn sweep(&self, _: &Profile, _: &Path, _: Option<&Path>) -> KResult<MockShape> {
        unsupported("sweep")
    }

    fn loft(&self, _: &[Profile], _: bool) -> KResult<MockShape> {
        unsupported("loft")
    }

    fn boolean(&self, _: BoolOp, _: &MockShape, _: &MockShape, _: f64) -> KResult<MockShape> {
        unsupported("boolean operations")
    }

    fn fillet(&self, _: &MockShape, _: &[FilletEdge]) -> KResult<MockShape> {
        unsupported("fillet")
    }

    fn chamfer(&self, _: &MockShape, _: &[ChamferEdge]) -> KResult<MockShape> {
        unsupported("chamfer")
    }

    fn shell(&self, _: &MockShape, _: &[u32], _: f64, _: bool) -> KResult<MockShape> {
        unsupported("shell")
    }

    fn draft(&self, _: &MockShape, _: &[u32], _: V3, _: f64, _: V3, _: V3) -> KResult<MockShape> {
        unsupported("draft")
    }

    fn transform(&self, _: &MockShape, _: &Transform) -> KResult<MockShape> {
        unsupported("transform")
    }

    fn topology(&self, _: &MockShape) -> KResult<TopoInfo> {
        unsupported("topology queries")
    }

    fn lineage(&self, _: &MockShape) -> Lineage {
        Lineage::default()
    }

    fn properties(&self, s: &MockShape) -> KResult<Properties> {
        let h = s.distance.abs();
        let (area, perimeter, centroid, corners, counts) = match s.segments.as_slice() {
            [Segment::Circle { center, r }] => (
                PI * r * r,
                2.0 * PI * r,
                *center,
                Vec::new(),
                Topology {
                    solids: 1,
                    faces: 3,
                    edges: 3,
                    vertices: 2,
                },
            ),
            segs => {
                let pts: Vec<[f64; 2]> = segs.iter().map(Segment::start).collect();
                let n = pts.len();
                let mut a2 = 0.0;
                let (mut cx, mut cy) = (0.0, 0.0);
                let mut per = 0.0;
                for i in 0..n {
                    let (p, q) = (pts[i], pts[(i + 1) % n]);
                    let w = p[0] * q[1] - q[0] * p[1];
                    a2 += w;
                    cx += (p[0] + q[0]) * w;
                    cy += (p[1] + q[1]) * w;
                    per += (q[0] - p[0]).hypot(q[1] - p[1]);
                }
                let area = a2 / 2.0;
                let c = [cx / (6.0 * area), cy / (6.0 * area)];
                (
                    area.abs(),
                    per,
                    c,
                    pts,
                    Topology {
                        solids: 1,
                        faces: n + 2,
                        edges: 3 * n,
                        vertices: 2 * n,
                    },
                )
            }
        };
        let base = s.frame.to_world(centroid);
        let center_of_mass = add(base, scale(s.dir, s.distance / 2.0));
        // bounding box of the two end faces
        let far = scale(s.dir, s.distance);
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        let mut grow = |p: V3| {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        };
        match s.segments.as_slice() {
            [Segment::Circle { center, r }] => {
                let c = s.frame.to_world(*center);
                for end in [c, add(c, far)] {
                    let ext: V3 = std::array::from_fn(|i| {
                        r * (s.frame.x[i].powi(2) + s.frame.y[i].powi(2)).sqrt()
                    });
                    grow([end[0] - ext[0], end[1] - ext[1], end[2] - ext[2]]);
                    grow([end[0] + ext[0], end[1] + ext[1], end[2] + ext[2]]);
                }
            }
            _ => {
                for p in &corners {
                    let w = s.frame.to_world(*p);
                    grow(w);
                    grow(add(w, far));
                }
            }
        }
        Ok(Properties {
            volume: area * h,
            area: 2.0 * area + perimeter * h,
            center_of_mass,
            inertia_com: None,
            bbox_min: lo,
            bbox_max: hi,
            topology: counts,
        })
    }

    fn canonical_brep(&self, s: &MockShape) -> KResult<Vec<u8>> {
        Ok(format!("mock-prism\n{s:?}\n").into_bytes())
    }

    fn from_brep(&self, _: &[u8]) -> KResult<MockShape> {
        unsupported("reading shapes back")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Loop;

    fn rect(w: f64, h: f64) -> Profile {
        let p = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
        Profile {
            frame: Frame::xy(),
            outer: Loop {
                segments: (0..4)
                    .map(|i| Segment::Line {
                        a: p[i],
                        b: p[(i + 1) % 4],
                    })
                    .collect(),
            },
            holes: vec![],
        }
    }

    #[test]
    fn block() {
        let k = MockKernel;
        let s = k
            .extrude(&rect(2.0, 3.0), [0.0, 0.0, 1.0], 4.0, 0.0)
            .unwrap();
        let p = k.properties(&s).unwrap();
        assert!((p.volume - 24.0).abs() < 1e-12);
        assert!((p.area - (2.0 * 6.0 + 10.0 * 4.0)).abs() < 1e-12);
        assert_eq!(p.center_of_mass, [1.0, 1.5, 2.0]);
        assert_eq!((p.bbox_min, p.bbox_max), ([0.0; 3], [2.0, 3.0, 4.0]));
        assert_eq!(
            p.topology,
            Topology {
                solids: 1,
                faces: 6,
                edges: 12,
                vertices: 8
            }
        );
    }

    #[test]
    fn cylinder_on_a_tilted_plane() {
        let k = MockKernel;
        let prof = Profile {
            frame: Frame::xz(),
            outer: Loop {
                segments: vec![Segment::Circle {
                    center: [0.0, 0.0],
                    r: 5.0,
                }],
            },
            holes: vec![],
        };
        // the XZ plane's normal is -Y; extrude along it
        let s = k.extrude(&prof, Frame::xz().z, 10.0, 0.0).unwrap();
        let p = k.properties(&s).unwrap();
        assert!((p.volume - PI * 250.0).abs() < 1e-9);
        assert!((p.bbox_min[1] + 10.0).abs() < 1e-9 && p.bbox_max[1].abs() < 1e-9);
        assert!((p.bbox_max[0] - 5.0).abs() < 1e-9 && (p.bbox_max[2] - 5.0).abs() < 1e-9);
    }

    #[test]
    fn unsupported_operations_are_errors() {
        let k = MockKernel;
        let s = k
            .extrude(&rect(1.0, 1.0), [0.0, 0.0, 1.0], 1.0, 0.0)
            .unwrap();
        assert!(k.boolean(BoolOp::Union, &s, &s, 1e-6).is_err());
        assert!(
            k.extrude(&rect(1.0, 1.0), [0.0, 0.0, 1.0], 1.0, 0.1)
                .is_err()
        );
        assert!(
            k.revolve(&rect(1.0, 1.0), [0.0; 3], [0.0, 1.0, 0.0], 1.0)
                .is_err()
        );
    }
}
