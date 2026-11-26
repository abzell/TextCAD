//! Part metadata (`meta`, format 0.2): parsing, formatting, configurations and diagnostics.

use tcad_model::{MetaKind, analyze};
use tcad_syntax::format_source;

fn part(body: &str) -> String {
    format!("tcad 0.2\n\npart p \"Piston rod, 50 mm\" {{\n{body}\n}}\n")
}

fn codes(src: &str) -> Vec<String> {
    analyze(src)
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| d.code.as_str().to_string())
        .collect()
}

const FULL: &str = "tcad 0.2

part rod \"Piston rod, 50 mm\" {
  param num = \"HC-110-014\"
  meta number=num revision=\"B\" kind=purchased vendor=\"Acme\" vendor_number=\"A-1\" mass=0.84 kg com=(0 mm, 0 mm, 12 mm) color=(0.7, 0.7, 0.72)
  config heavy { meta.number = \"HC-110-015\"; meta.kind = made }
  config default standard { }
}
";

#[test]
fn full_meta_resolves_and_configurations_override_it() {
    let a = analyze(FULL);
    assert!(!a.has_errors(), "{:?}", a.diagnostics);
    let m = a.model.expect("model");
    let base = m.resolve(None).expect("base").meta;
    assert_eq!(base.number.as_deref(), Some("HC-110-014"));
    assert_eq!(base.revision.as_deref(), Some("B"));
    assert_eq!(base.kind, MetaKind::Purchased);
    assert_eq!(base.vendor.as_deref(), Some("Acme"));
    assert_eq!(base.vendor_number.as_deref(), Some("A-1"));
    assert_eq!(base.mass_kg, Some(0.84));
    assert_eq!(base.com_mm, Some([0.0, 0.0, 12.0]));
    assert_eq!(base.color, Some([0.7, 0.7, 0.72]));
    let heavy = m.resolve(Some("heavy")).expect("heavy").meta;
    assert_eq!(heavy.number.as_deref(), Some("HC-110-015"));
    assert_eq!(heavy.kind, MetaKind::Made);
    assert_eq!(
        heavy.revision.as_deref(),
        Some("B"),
        "keys not overridden are kept"
    );
    let standard = m.resolve(Some("standard")).expect("standard").meta;
    assert_eq!(standard, base);
}

#[test]
fn units_are_converted_and_defaults_apply() {
    let a = analyze(&part("  meta mass=840 g com=(1 cm, 0 mm, 0 mm)"));
    assert!(!a.has_errors(), "{:?}", a.diagnostics);
    let meta = a.model.expect("model").resolve(None).expect("ok").meta;
    assert!((meta.mass_kg.expect("mass") - 0.84).abs() < 1e-12);
    assert_eq!(meta.com_mm, Some([10.0, 0.0, 0.0]));
    assert_eq!(meta.kind, MetaKind::Made);
    let none = analyze(&part("  param a = 1 mm"));
    assert!(
        none.model
            .expect("model")
            .resolve(None)
            .expect("ok")
            .meta
            .is_empty()
    );
}

#[test]
fn canonical_form_orders_keys() {
    let src =
        part("  meta color=(1, 0, 0) kind=made number=\"X\"\n  config c { meta.number = \"Y\" }");
    let want =
        part("  meta number=\"X\" kind=made color=(1, 0, 0)\n  config c { meta.number = \"Y\" }");
    assert_eq!(format_source(&src).expect("formats"), want);
    assert_eq!(format_source(&want).expect("formats"), want);
}

#[test]
fn diagnostics() {
    for (body, code) in [
        ("  meta number=\"A\"\n  meta revision=\"B\"", "E2303"),
        ("  meta part_no=\"A\"", "E5001"),
        ("  meta purchased", "E5001"),
        ("  meta number=\"A\" number=\"B\"", "E5001"),
        ("  meta com=(0 mm, 0 mm, 0 mm)", "E5001"),
        ("  meta kind=bought", "E2501"),
        ("  meta number=3", "E2202"),
        ("  meta mass=3 mm", "E2202"),
        ("  meta mass=0 kg", "E2301"),
        ("  meta color=(1.2, 0, 0)", "E2301"),
        ("  meta color=(1, 0)", "E2202"),
        ("  meta number=nope", "E2102"),
        (
            "  meta number=\"A\"\n  config c { meta.part = \"B\" }",
            "E2401",
        ),
        (
            "  meta number=\"A\"\n  config c { meta.com = (0 mm, 0 mm, 0 mm) }",
            "E5001",
        ),
        (
            "  meta number=\"A\"\n  config c { meta.revision = 2 }",
            "E2202",
        ),
    ] {
        let src = part(body);
        assert_eq!(codes(&src), [code], "{body}");
    }
}

#[test]
fn meta_needs_format_0_2() {
    let old = "tcad 0.1\n\npart p {\n  meta number=\"A\"\n}\n";
    assert_eq!(codes(old), ["E0103"]);
    let old_cfg = "tcad 0.1\n\npart p {\n  param meta = 1\n  config c { meta.number = \"A\" }\n}\n";
    assert_eq!(codes(old_cfg), ["E0103"]);
    // and in 0.1 `meta` is still a fine name
    assert!(
        codes("tcad 0.1\n\npart p {\n  param meta = 1\n  config c { meta = 2 }\n}\n").is_empty()
    );
}

#[test]
fn plain_overrides_with_a_name_value_are_not_meta() {
    let src = part("  param a = 1 mm\n  param b = 2 mm\n  meta number=\"A\"\n  config c { b = a }");
    let a = analyze(&src);
    assert!(!a.has_errors(), "{:?}", a.diagnostics);
    let m = a.model.expect("model");
    let c = m.resolve(Some("c")).expect("c");
    assert_eq!(c.length_mm("b"), Some(1.0));
    assert_eq!(c.meta.number.as_deref(), Some("A"));
}
