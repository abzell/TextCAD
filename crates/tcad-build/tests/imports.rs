//! STEP import as a dead solid, combined into a body, with names and patterns.
#![cfg(feature = "occt")]

use tcad_build::{build, build_with};
use tcad_kernel::{Kernel, StepMeta};
use tcad_model::analyze;
use tcad_occt::OcctKernel;

fn part(body: &str) -> String {
    format!("tcad 0.1\n\npart p {{\n{body}\n}}\n")
}

/// STEP bytes of a 20 x 10 x 6 block with a 4 mm hole.
fn step_block(k: &OcctKernel) -> Vec<u8> {
    let a = analyze(&part(
        "
  sketch s on plane.XY {
    rect r corner=(0 mm, 0 mm) w=20 mm h=10 mm
  }
  extrude blk from s.r distance=6 mm
  hole h on=blk.cap_end at=(10 mm, 5 mm) d=4 mm end=through_all
",
    ));
    let built = build(k, &a.model.unwrap(), None).unwrap();
    k.export_step(
        built.shape.as_ref().unwrap(),
        &StepMeta {
            name: "blk",
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn an_imported_solid_is_combined_named_and_patterned() {
    let k = OcctKernel::new();
    let bytes = step_block(&k);
    let loader = |p: &str| -> Result<Vec<u8>, String> {
        if p == "imports/blk.step" {
            Ok(bytes.clone())
        } else {
            Err("no such file".into())
        }
    };
    let src = part(
        "
  import blk = step(\"imports/blk.step\")
  combine base with=blk
  sketch top on blk.face#2 {
    circle c center=(0 mm, 0 mm) r=1 mm
  }
",
    );
    let a = analyze(&src);
    assert!(!a.has_errors(), "{:#?}", a.diagnostics);
    // the face names are blk.face#1 .. #7 (6 sides + the hole wall), ordered by position
    let (res, refs) = tcad_build::build_with(&k, &a.model.unwrap(), None, Some(&loader));
    let built = res.unwrap_or_else(|d| panic!("{d:#?}\n{refs:#?}"));
    let p = k.properties(built.shape.as_ref().unwrap()).unwrap();
    let want = 20.0 * 10.0 * 6.0 - std::f64::consts::PI * 4.0 * 6.0;
    assert!(
        (p.volume - want).abs() < 1e-6 * want,
        "{} vs {want}",
        p.volume
    );
    let ids: Vec<String> = built
        .features
        .last()
        .unwrap()
        .topology_map
        .iter()
        .filter(|e| e.kind == "face")
        .map(|e| e.id.clone())
        .collect();
    assert!(ids.iter().all(|i| i.starts_with("blk.face#")), "{ids:?}");
    assert_eq!(ids.len(), 7);

    // without a loader an import cannot work, and a missing file is reported
    let a = analyze(&src);
    let d = build(&k, &a.model.unwrap(), None).err().unwrap();
    assert_eq!(d[0].code.to_string(), "E5004");
    let missing = |_: &str| -> Result<Vec<u8>, String> { Err("no such file".into()) };
    let a = analyze(&src);
    let d = build_with(&k, &a.model.unwrap(), None, Some(&missing))
        .0
        .err()
        .unwrap();
    assert!(d[0].message.contains("no such file"), "{}", d[0].message);
}

#[test]
fn an_imported_solid_can_be_a_boolean_tool() {
    let k = OcctKernel::new();
    let bytes = step_block(&k);
    let loader = move |_: &str| -> Result<Vec<u8>, String> { Ok(bytes.clone()) };
    let src = part(
        "
  import blk = step(\"x.step\")
  sketch s on plane.XY {
    rect r corner=(0 mm, 0 mm) w=40 mm h=20 mm
  }
  extrude plate from s.r distance=6 mm
  combine pocket with=blk at=(5 mm, 5 mm, 0 mm) op=subtract
",
    );
    let a = analyze(&src);
    assert!(!a.has_errors(), "{:#?}", a.diagnostics);
    let built = build_with(&k, &a.model.unwrap(), None, Some(&loader))
        .0
        .unwrap();
    let p = k.properties(built.shape.as_ref().unwrap()).unwrap();
    // the block with its hole is removed from the plate: the hole's column stays as a pin
    let want = 40.0 * 20.0 * 6.0 - (20.0 * 10.0 * 6.0 - std::f64::consts::PI * 4.0 * 6.0);
    assert!(
        (p.volume - want).abs() < 1e-6 * want,
        "{} vs {want}",
        p.volume
    );
}
