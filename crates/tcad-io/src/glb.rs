//! GLB (binary glTF 2.0) for the viewer. Every face is its own primitive and carries its semantic
//! ID in `extras`, so the viewer can highlight a name and name a picked face.

use crate::ModelInfo;
use serde_json::{Value, json};
use tcad_kernel::Mesh;

/// The persistent name of a face for the viewer.
#[derive(Clone, Debug, Default)]
pub struct FaceLabel {
    /// Primary semantic ID.
    pub id: String,
    /// Other IDs of the same face.
    pub aliases: Vec<String>,
}

fn pad4(v: &mut Vec<u8>, fill: u8) {
    while !v.len().is_multiple_of(4) {
        v.push(fill);
    }
}

/// A GLB with one primitive per face. `labels[i]` names face `i` of the shape (missing labels are
/// left out).
pub fn glb(mesh: &Mesh, labels: &[FaceLabel], info: &ModelInfo<'_>) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let mut views: Vec<Value> = Vec::new();
    let mut accessors: Vec<Value> = Vec::new();
    let mut primitives: Vec<Value> = Vec::new();
    for f in &mesh.faces {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for v in f.positions.chunks_exact(3) {
            for k in 0..3 {
                min[k] = min[k].min(v[k]);
                max[k] = max[k].max(v[k]);
            }
        }
        let mut add_view = |bytes: &[u8], target: u32, bin: &mut Vec<u8>| -> usize {
            pad4(bin, 0);
            let offset = bin.len();
            bin.extend_from_slice(bytes);
            views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len(), "target": target}));
            views.len() - 1
        };
        let pos: Vec<u8> = f.positions.iter().flat_map(|v| v.to_le_bytes()).collect();
        let nor: Vec<u8> = f.normals.iter().flat_map(|v| v.to_le_bytes()).collect();
        let idx: Vec<u8> = f.triangles.iter().flat_map(|v| v.to_le_bytes()).collect();
        let (vp, vn, vi) = (
            add_view(&pos, 34962, &mut bin),
            add_view(&nor, 34962, &mut bin),
            add_view(&idx, 34963, &mut bin),
        );
        let n = f.positions.len() / 3;
        let base = accessors.len();
        accessors.push(json!({"bufferView": vp, "componentType": 5126, "count": n, "type": "VEC3", "min": min, "max": max}));
        accessors
            .push(json!({"bufferView": vn, "componentType": 5126, "count": n, "type": "VEC3"}));
        accessors.push(json!({"bufferView": vi, "componentType": 5125, "count": f.triangles.len(), "type": "SCALAR"}));
        let mut extras = json!({"face": f.face});
        if let Some(l) = labels.get(f.face as usize) {
            extras["id"] = json!(l.id);
            if !l.aliases.is_empty() {
                extras["aliases"] = json!(l.aliases);
            }
        }
        primitives.push(json!({
            "attributes": {"POSITION": base, "NORMAL": base + 1},
            "indices": base + 2,
            "material": 0,
            "mode": 4,
            "extras": extras,
        }));
    }
    pad4(&mut bin, 0);
    let mut model = json!({"name": info.name, "generator": "TextCAD"});
    if let Some(c) = info.configuration {
        model["configuration"] = json!(c);
    }
    let doc = json!({
        "asset": {"version": "2.0", "generator": "TextCAD"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": info.name, "mesh": 0}],
        "meshes": [{"name": info.name, "primitives": primitives, "extras": model}],
        "materials": [{
            "name": info.material.unwrap_or("default"),
            "pbrMetallicRoughness": {"baseColorFactor": [0.7, 0.72, 0.75, 1.0], "metallicFactor": 0.4, "roughnessFactor": 0.5},
        }],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": views,
        "accessors": accessors,
    });
    let mut json_bytes = serde_json::to_vec(&doc).unwrap_or_default();
    pad4(&mut json_bytes, b' ');
    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}
