//! The mesh writers on a single triangle and on a tetrahedron.

use serde_json::Value;
use tcad_io::{FaceLabel, ModelInfo, glb, stl_binary, threemf};
use tcad_kernel::{Mesh, MeshFace};

fn tetra() -> Mesh {
    // four faces of a tetrahedron with vertices at the origin and on the axes
    let v = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let tris = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
    Mesh {
        faces: tris
            .iter()
            .enumerate()
            .map(|(i, t)| MeshFace {
                face: i as u32,
                positions: t.iter().flat_map(|&k| v[k]).collect(),
                normals: vec![0.0; 9],
                triangles: vec![0, 1, 2],
            })
            .collect(),
    }
}

#[test]
fn stl_has_header_count_and_records() {
    let b = stl_binary(&tetra(), "tet");
    assert_eq!(&b[..13], b"TextCAD tet  ");
    assert_eq!(u32::from_le_bytes(b[80..84].try_into().unwrap()), 4);
    assert_eq!(b.len(), 84 + 4 * 50);
    // the first face lies in the z = 0 plane and faces down
    let n = |i: usize| f32::from_le_bytes(b[84 + 4 * i..88 + 4 * i].try_into().unwrap());
    assert!((n(2) + 1.0).abs() < 1e-6, "{} {} {}", n(0), n(1), n(2));
    // reproducible
    assert_eq!(b, stl_binary(&tetra(), "tet"));
}

#[test]
fn threemf_is_a_zip_with_the_model() {
    let b = threemf(
        &tetra(),
        &ModelInfo {
            name: "tet & co",
            configuration: Some("heavy"),
            material: Some("steel"),
        },
    );
    assert_eq!(&b[..4], b"PK\x03\x04");
    let text = String::from_utf8_lossy(&b);
    assert!(text.contains("unit=\"millimeter\""));
    assert!(text.contains("tet &amp; co"));
    assert!(text.contains("<base name=\"steel\""));
    assert!(text.contains("3D/3dmodel.model"));
    assert_eq!(text.matches("<triangle ").count(), 4);
    assert_eq!(text.matches("<vertex ").count(), 12);
}

#[test]
fn glb_carries_face_ids_in_extras() {
    let labels: Vec<FaceLabel> = (0..4)
        .map(|i| FaceLabel {
            id: format!("boss.side({i})"),
            aliases: if i == 1 {
                vec!["other.cap".into()]
            } else {
                vec![]
            },
        })
        .collect();
    let b = glb(
        &tetra(),
        &labels,
        &ModelInfo {
            name: "tet",
            configuration: None,
            material: Some("steel"),
        },
    );
    assert_eq!(&b[..4], b"glTF");
    assert_eq!(
        u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize,
        b.len()
    );
    assert_eq!(b.len() % 4, 0);
    let json_len = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    assert_eq!(&b[16..20], b"JSON");
    let doc: Value = serde_json::from_slice(&b[20..20 + json_len]).unwrap();
    let prims = doc["meshes"][0]["primitives"].as_array().unwrap();
    assert_eq!(prims.len(), 4);
    assert_eq!(prims[2]["extras"]["id"], "boss.side(2)");
    assert_eq!(prims[1]["extras"]["aliases"][0], "other.cap");
    assert_eq!(doc["materials"][0]["name"], "steel");
    let bin_len = doc["buffers"][0]["byteLength"].as_u64().unwrap() as usize;
    assert_eq!(&b[20 + json_len + 4..20 + json_len + 8], b"BIN\0");
    assert_eq!(b.len(), 20 + json_len + 8 + bin_len);
}

#[test]
fn png_and_views() {
    let png = tcad_io::png_rgb(2, 1, &[255, 0, 0, 0, 255, 0]);
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    assert!(png.ends_with(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82]));
    // a tetrahedron seen from every view covers some pixels and the highlight changes the picture
    for view in tcad_io::VIEWS {
        let a = tcad_io::render_png(&tetra(), view, 64, &|_| false).expect("render");
        let b = tcad_io::render_png(&tetra(), view, 64, &|_| true).expect("render");
        assert_eq!(&a[..4], &[0x89, b'P', b'N', b'G']);
        assert!(a != b, "{view}: highlight");
    }
    assert!(tcad_io::render_png(&tetra(), "sideways", 64, &|_| false).is_err());
}
