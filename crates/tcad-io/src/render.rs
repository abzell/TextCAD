//! A small software renderer: orthographic views of a mesh as PNG, for agents and previews.

use crate::png::png_rgb;
use tcad_kernel::Mesh;

type V = [f64; 3];

fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: V) -> V {
    let l = dot(a, a).sqrt();
    if l == 0.0 {
        a
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

/// The names of the views: `iso`, `front`, `back`, `left`, `right`, `top`, `bottom`.
pub const VIEWS: &[&str] = &["iso", "front", "back", "left", "right", "top", "bottom"];

/// `(right, up, toward the viewer)` of a named view; the world is Z up, front looks along +Y.
fn camera(view: &str) -> Option<(V, V, V)> {
    Some(match view {
        "front" => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
        "back" => ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        "right" => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        "left" => ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0]),
        "top" => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        "bottom" => ([1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]),
        "iso" => {
            let to = unit([1.0, -1.0, 0.8]);
            let right = unit(cross([0.0, 0.0, 1.0], to));
            let up = cross(to, right);
            (right, up, to)
        }
        _ => return None,
    })
}

/// Renders the mesh from a named view into a PNG of `size` x `size` pixels. Faces for which
/// `highlight(face index)` is true are orange; face boundaries are drawn as dark lines.
pub fn render_png(
    mesh: &Mesh,
    view: &str,
    size: u32,
    highlight: &dyn Fn(u32) -> bool,
) -> Result<Vec<u8>, String> {
    let (right, up, to) = camera(view)
        .ok_or_else(|| format!("unknown view `{view}`; use one of {}", VIEWS.join(", ")))?;
    let size = size.clamp(16, 4096);
    let n = size as usize;
    // project every vertex
    let mut lo = [f64::MAX; 2];
    let mut hi = [f64::MIN; 2];
    for f in &mesh.faces {
        for p in f.positions.chunks_exact(3) {
            let p = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])];
            let uv = [dot(p, right), dot(p, up)];
            for k in 0..2 {
                lo[k] = lo[k].min(uv[k]);
                hi[k] = hi[k].max(uv[k]);
            }
        }
    }
    if lo[0] > hi[0] {
        return Err("the mesh is empty".into());
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-9);
    let margin = 0.08 * f64::from(size);
    let scale = (f64::from(size) - 2.0 * margin) / span;
    let cx = (lo[0] + hi[0]) / 2.0;
    let cy = (lo[1] + hi[1]) / 2.0;
    let to_px = |p: V| -> (f64, f64, f64) {
        (
            f64::from(size) / 2.0 + (dot(p, right) - cx) * scale,
            f64::from(size) / 2.0 - (dot(p, up) - cy) * scale,
            dot(p, to),
        )
    };
    let mut depth = vec![f64::MIN; n * n];
    let mut face_id = vec![u32::MAX; n * n];
    let mut shade = vec![0.0f64; n * n];
    for f in &mesh.faces {
        for t in f.triangles.chunks_exact(3) {
            let v = |i: u32| {
                let j = 3 * i as usize;
                [
                    f64::from(f.positions[j]),
                    f64::from(f.positions[j + 1]),
                    f64::from(f.positions[j + 2]),
                ]
            };
            let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
            let normal = unit(cross(
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            ));
            let facing = dot(normal, to);
            if facing <= 1e-9 {
                continue; // back face
            }
            let (pa, pb, pc) = (to_px(a), to_px(b), to_px(c));
            let area = (pb.0 - pa.0) * (pc.1 - pa.1) - (pb.1 - pa.1) * (pc.0 - pa.0);
            if area.abs() < 1e-12 {
                continue;
            }
            let x0 = pa.0.min(pb.0).min(pc.0).floor().max(0.0) as usize;
            let x1 = (pa.0.max(pb.0).max(pc.0).ceil() as usize).min(n - 1);
            let y0 = pa.1.min(pb.1).min(pc.1).floor().max(0.0) as usize;
            let y1 = (pa.1.max(pb.1).max(pc.1).ceil() as usize).min(n - 1);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                    let w0 = ((pb.0 - px) * (pc.1 - py) - (pb.1 - py) * (pc.0 - px)) / area;
                    let w1 = ((pc.0 - px) * (pa.1 - py) - (pc.1 - py) * (pa.0 - px)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 {
                        continue;
                    }
                    let z = w0 * pa.2 + w1 * pb.2 + w2 * pc.2;
                    let i = y * n + x;
                    if z > depth[i] {
                        depth[i] = z;
                        face_id[i] = f.face;
                        shade[i] = 0.3 + 0.7 * facing;
                    }
                }
            }
        }
    }
    let mut rgb = vec![0u8; n * n * 3];
    for y in 0..n {
        for x in 0..n {
            let i = y * n + x;
            let color: [f64; 3] = if face_id[i] == u32::MAX {
                [245.0, 245.0, 248.0]
            } else {
                // a boundary pixel: a neighbor shows another face (or the background)
                let edge = [(1i64, 0i64), (0, 1), (-1, 0), (0, -1)]
                    .iter()
                    .any(|(dx, dy)| {
                        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                        nx >= 0
                            && ny >= 0
                            && (nx as usize) < n
                            && (ny as usize) < n
                            && face_id[ny as usize * n + nx as usize] != face_id[i]
                            && face_id[ny as usize * n + nx as usize] != u32::MAX
                    });
                if edge {
                    [50.0, 55.0, 65.0]
                } else if highlight(face_id[i]) {
                    [230.0 * shade[i], 120.0 * shade[i], 30.0 * shade[i]]
                } else {
                    [178.0 * shade[i], 184.0 * shade[i], 196.0 * shade[i]]
                }
            };
            for k in 0..3 {
                rgb[3 * i + k] = color[k].clamp(0.0, 255.0) as u8;
            }
        }
    }
    Ok(png_rgb(size, size, &rgb))
}
