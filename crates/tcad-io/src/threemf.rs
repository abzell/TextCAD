//! 3MF (3D Manufacturing Format): a ZIP with one model, in millimetres.

use crate::ModelInfo;
use crate::zip::zip;
use std::fmt::Write;
use tcad_kernel::Mesh;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A 3MF package with the mesh as one object, the part name as title, the part number and
/// revision as metadata and the material name (with the part's display colour) as a base
/// material.
pub fn threemf(mesh: &Mesh, info: &ModelInfo<'_>) -> Vec<u8> {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n");
    let _ = writeln!(
        xml,
        " <metadata name=\"Title\">{}</metadata>",
        esc(info.name)
    );
    if let Some(c) = info.configuration {
        let _ = writeln!(
            xml,
            " <metadata name=\"Configuration\">{}</metadata>",
            esc(c)
        );
    }
    if let Some(n) = info.number {
        let _ = writeln!(xml, " <metadata name=\"PartNumber\">{}</metadata>", esc(n));
    }
    if let Some(r) = info.revision {
        let _ = writeln!(xml, " <metadata name=\"Revision\">{}</metadata>", esc(r));
    }
    xml.push_str(" <metadata name=\"Application\">TextCAD</metadata>\n <resources>\n");
    let material = info.material.is_some() || info.color.is_some();
    if material {
        let color = info
            .color
            .map_or_else(|| "#B4B4B4".to_string(), crate::hex_color);
        let _ = writeln!(
            xml,
            "  <basematerials id=\"1\"><base name=\"{}\" displaycolor=\"{color}\"/></basematerials>",
            esc(info.material.unwrap_or("default"))
        );
    }
    let pid = if material {
        " pid=\"1\" pindex=\"0\""
    } else {
        ""
    };
    let _ = writeln!(
        xml,
        "  <object id=\"2\" type=\"model\" name=\"{}\"{pid}>\n   <mesh>\n    <vertices>",
        esc(info.name)
    );
    let mut triangles = String::new();
    let mut base = 0u32;
    for f in &mesh.faces {
        for v in f.positions.chunks_exact(3) {
            let _ = writeln!(
                xml,
                "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>",
                v[0], v[1], v[2]
            );
        }
        for t in f.triangles.chunks_exact(3) {
            let _ = writeln!(
                triangles,
                "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>",
                base + t[0],
                base + t[1],
                base + t[2]
            );
        }
        base += (f.positions.len() / 3) as u32;
    }
    xml.push_str("    </vertices>\n    <triangles>\n");
    xml.push_str(&triangles);
    xml.push_str("    </triangles>\n   </mesh>\n  </object>\n </resources>\n <build>\n  <item objectid=\"2\"/>\n </build>\n</model>\n");
    let types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/></Types>\n";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/></Relationships>\n";
    zip(&[
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("3D/3dmodel.model", xml.as_bytes()),
    ])
}
