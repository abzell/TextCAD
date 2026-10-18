//! Binary STL.

use tcad_kernel::Mesh;

/// Binary STL of the whole mesh (units: millimetres). `name` goes into the 80-byte header.
pub fn stl_binary(mesh: &Mesh, name: &str) -> Vec<u8> {
    let count: usize = mesh.faces.iter().map(|f| f.triangles.len() / 3).sum();
    let mut out = Vec::with_capacity(84 + 50 * count);
    let mut header = format!("TextCAD {name}").into_bytes();
    header.resize(80, b' ');
    header.truncate(80);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(count as u32).to_le_bytes());
    for f in &mesh.faces {
        for t in f.triangles.chunks_exact(3) {
            let p = |i: u32| {
                let j = 3 * i as usize;
                [f.positions[j], f.positions[j + 1], f.positions[j + 2]]
            };
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let mut n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 0.0 {
                n = [n[0] / len, n[1] / len, n[2] / len];
            }
            for v in n.iter().chain(&a).chain(&b).chain(&c) {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&[0, 0]);
        }
    }
    out
}
