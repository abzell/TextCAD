//! Integration tests for semantic analysis.

use tcad_expr::Value;
use tcad_model::{Analysis, ItemKind, analyze};

const CLEVIS: &str = include_str!("../../../spec/examples/rod_clevis.tcad");

fn codes(a: &Analysis) -> Vec<&'static str> {
    a.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

/// Wraps statements in a part.
fn part(body: &str) -> String {
    format!("tcad 0.1\n\npart p {{\n{body}\n}}\n")
}

fn check(body: &str) -> Vec<&'static str> {
    codes(&analyze(&part(body)))
}

#[test]
fn clevis_has_no_diagnostics() {
    let a = analyze(CLEVIS);
    assert!(a.diagnostics.is_empty(), "{:#?}", a.diagnostics);
    let m = a.model.expect("model");
    assert_eq!(m.part.name, "rod_clevis");
    assert_eq!(
        m.part.display_name.as_deref(),
        Some("Rod clevis, 20 mm pin")
    );
    assert_eq!(m.part.tolerance, 1e-6);
    assert_eq!(m.part.params.len(), 5);
    assert_eq!(m.part.items.len(), 9);
    assert!(matches!(m.part.items[0].kind, ItemKind::Sketch(_)));
}

#[test]
fn material_density_is_canonical() {
    let m = analyze(CLEVIS).model.expect("model");
    let mat = m.part.material.expect("material");
    assert_eq!(mat.name, "AISI 1045");
    // 7.85 g/cm^3 = 7.85e-6 kg/mm^3
    assert!((mat.density.expect("density") - 7.85e-6).abs() < 1e-18);
}

#[test]
fn resolve_base_and_configs() {
    let m = analyze(CLEVIS).model.expect("model");
    let base = m.resolve(None).expect("base");
    assert_eq!(base.length_mm("pin_dia"), Some(20.0));
    assert_eq!(base.param("thread"), Some(&Value::Str("M24x2".into())));
    let heavy = m.resolve(Some("heavy")).expect("heavy");
    assert_eq!(heavy.length_mm("pin_dia"), Some(25.0));
    assert_eq!(heavy.length_mm("jaw_thk"), Some(20.0));
    assert_eq!(heavy.length_mm("jaw_gap"), Some(26.0));
    assert_eq!(heavy.param("thread"), Some(&Value::Str("M30x2".into())));
    assert_eq!(
        m.default_config().map(|c| c.name.as_str()),
        Some("standard")
    );
    assert!(m.resolve(Some("missing")).is_err());
}

#[test]
fn parameters_evaluate_with_units() {
    let a = analyze(&part(
        "param a = 1 in\nparam b = a + 5.4 mm\nparam c = 90 deg\nparam d = sin(c)\nparam e = 2 * (b - a) / 1 mm",
    ));
    assert!(a.diagnostics.is_empty(), "{:#?}", a.diagnostics);
    let r = a.model.expect("model").resolve(None).expect("resolve");
    assert!((r.length_mm("a").expect("a") - 25.4).abs() < 1e-12);
    assert!((r.length_mm("b").expect("b") - 30.8).abs() < 1e-12);
    let Some(Value::Num(d)) = r.param("d") else {
        panic!()
    };
    assert_eq!(d.value, 1.0);
    let Some(Value::Num(e)) = r.param("e") else {
        panic!()
    };
    assert!((e.value - 10.8).abs() < 1e-12 && e.dim.is_none());
}

#[test]
fn dimension_errors() {
    assert_eq!(check("param a = 1 mm + 1 deg"), ["E2201"]);
    assert_eq!(check("param a = 1 mm + 1"), ["E2201"]);
    assert_eq!(check("param a = sin(30)"), ["E2201"]);
    assert_eq!(check("param a = 1 mm / 0"), ["E2205"]);
    assert_eq!(check("param a = sqrt(1 mm)"), ["E2204"]);
    assert_eq!(check("param a = 1 mm ^ 0.5"), ["E2204"]);
    assert_eq!(check("param a = 1 mm\nparam b = a < 2 deg"), ["E2201"]);
    assert_eq!(check("param a = true + 1"), ["E2202"]);
}

#[test]
fn unit_errors_in_features_are_found() {
    let body = "param w = 5 mm\nextrude e from s.o distance=w + 3 deg";
    // `s` is undefined too, but the unit error must be reported regardless.
    let c = check(body);
    assert!(c.contains(&"E2201"), "{c:?}");
    assert!(c.contains(&"E2102"), "{c:?}");
}

#[test]
fn scoping() {
    assert_eq!(check("param a = a + 1 mm"), ["E2103"]);
    assert_eq!(check("param a = b\nparam b = 1 mm"), ["E2103"]);
    assert_eq!(check("param a = nope"), ["E2102"]);
    assert_eq!(check("param a = 1 mm\nparam a = 2 mm"), ["E2101"]);
    assert_eq!(check("param a = 1 mm\ntag a = x.y"), ["E2101", "E2102"]);
    let a = analyze(&part("param jaw_gap = 1 mm\nparam b = jaw_gp * 2"));
    assert_eq!(codes(&a), ["E2102"]);
    let d = &a.diagnostics[0];
    assert!(
        d.message.contains("did you mean `jaw_gap`"),
        "{}",
        d.message
    );
    assert_eq!(
        d.hint.as_ref().expect("hint").edits[0].replacement,
        "jaw_gap"
    );
    // features may reference earlier names only
    assert_eq!(
        check("tag t = later.x\nextrude later distance=1 mm"),
        ["E2103"]
    );
    // a feature is not a value
    assert_eq!(
        check("extrude e distance=1 mm\nparam a = e + 1 mm"),
        ["E2104"]
    );
}

#[test]
fn sketch_scope() {
    let ok = "sketch s on plane.XY {\n  circle c center=(0 mm, 0 mm) d=1 mm\n  circle e center=c.center d=1 mm\n  constrain concentric(c, e)\n}";
    assert_eq!(check(ok), Vec::<&str>::new());
    let bad = "sketch s on plane.XY {\n  circle e center=c.center d=1 mm\n  circle c d=1 mm\n}";
    assert_eq!(check(bad), ["E2103"]);
    let dup = "sketch s on plane.XY {\n  circle c d=1 mm\n  circle c d=2 mm\n}";
    assert_eq!(check(dup), ["E2101"]);
    let bad_constraint = "sketch s on plane.XY {\n  circle c d=1 mm\n  constrain equal(c, d)\n}";
    assert_eq!(check(bad_constraint), ["E2102"]);
    // entities may share a name with part-level names
    let shadow = "param c = 1 mm\nsketch s on plane.XY {\n  circle c d=c\n}";
    assert_eq!(check(shadow), Vec::<&str>::new());
}

#[test]
fn ranges() {
    assert_eq!(
        check("param a = 5 mm range(1 mm, 10 mm)"),
        Vec::<&str>::new()
    );
    assert_eq!(check("param a = 50 mm range(1 mm, 10 mm)"), ["E2301"]);
    assert_eq!(check("param a = 5 mm range(10 mm, 1 mm)"), ["E2304"]);
    assert_eq!(check("param a = 5 mm range(1 deg, 10 deg)"), ["E2304"]);
    assert_eq!(check("param a = \"x\" range(1, 2)"), ["E2304"]);
}

#[test]
fn singletons() {
    assert_eq!(check("tolerance 0 mm"), ["E2302"]);
    assert_eq!(check("tolerance 1 deg"), ["E2302"]);
    assert_eq!(check("tolerance 1e-3 mm\ntolerance 1e-4 mm"), ["E2303"]);
    assert_eq!(check("material \"x\" density=1 mm"), ["E2305"]);
    assert_eq!(
        check("material \"x\" density=1 g/cm^3\nmaterial \"y\""),
        ["E2303"]
    );
    let m = analyze(&part("tolerance 1e-3 mm")).model.expect("model");
    assert_eq!(m.part.tolerance, 1e-3);
}

#[test]
fn configurations() {
    let base = "param a = 1 mm\nparam t = \"x\"\nextrude e distance=1 mm\n";
    assert_eq!(
        check(&format!(
            "{base}config c {{ a = 2 mm }}\nconfig default d {{ }}"
        )),
        Vec::<&str>::new()
    );
    assert_eq!(
        check(&format!(
            "{base}config c {{ zz = 2 mm }}\nconfig default d {{ }}"
        )),
        ["E2401"]
    );
    assert_eq!(
        check(&format!(
            "{base}config c {{ a = 2 deg }}\nconfig default d {{ }}"
        )),
        ["E2402"]
    );
    assert_eq!(
        check(&format!(
            "{base}config c {{ t = 2 mm }}\nconfig default d {{ }}"
        )),
        ["E2402"]
    );
    assert_eq!(
        check(&format!(
            "{base}config c {{ suppress zz }}\nconfig default d {{ }}"
        )),
        ["E2403"]
    );
    assert_eq!(
        check(&format!(
            "{base}config c {{ suppress a }}\nconfig default d {{ }}"
        )),
        ["E2403"]
    );
    assert_eq!(
        check(&format!(
            "{base}config default c {{ }}\nconfig default d {{ }}"
        )),
        ["E2404"]
    );
    assert_eq!(check(&format!("{base}config c {{ }}")), ["W2405"]);
    assert_eq!(
        check(&format!("{base}config c {{ }}\nconfig default c {{ }}")),
        ["E2101"]
    );
    // an override that violates the parameter's range
    let ranged = "param a = 5 mm range(1 mm, 10 mm)\nconfig default big { a = 20 mm }";
    let a = analyze(&part(ranged));
    assert_eq!(codes(&a), ["E2301"]);
    assert!(a.diagnostics[0].message.contains("configuration `big`"));
    // overrides propagate to dependent parameters
    let dep = "param a = 1 mm\nparam b = a * 2\nconfig default c { a = 3 mm }";
    let m = analyze(&part(dep)).model.expect("model");
    assert_eq!(m.resolve(Some("c")).expect("c").length_mm("b"), Some(6.0));
    assert_eq!(m.resolve(None).expect("base").length_mm("b"), Some(2.0));
    // an override cannot use a parameter declared after its target
    assert_eq!(
        check("param a = 1 mm\nparam b = 2 mm\nconfig default c { a = b }"),
        ["E2103"]
    );
}

#[test]
fn suppression() {
    let src = "param n = 1\nextrude e distance=1 mm suppress if n > 1\nextrude f distance=1 mm\n\
               config default big { n = 2 }\nconfig other { suppress f }";
    let m = analyze(&part(src)).model.expect("model");
    assert!(m.resolve(None).expect("base").suppressed.is_empty());
    let big = m.resolve(Some("big")).expect("big");
    assert_eq!(big.suppressed.iter().collect::<Vec<_>>(), ["e"]);
    let other = m.resolve(Some("other")).expect("other");
    assert_eq!(other.suppressed.iter().collect::<Vec<_>>(), ["f"]);
    assert_eq!(
        check("param n = 1\nextrude e distance=1 mm suppress if n + 1"),
        ["E2601"]
    );
    assert_eq!(
        check("extrude e distance=1 mm suppress if e"),
        ["E2103", "E2104"]
            .iter()
            .copied()
            .take(1)
            .collect::<Vec<_>>()
    );
}

#[test]
fn enumerated_arguments() {
    assert_eq!(
        check("extrude e distance=1 mm op=subtract"),
        Vec::<&str>::new()
    );
    assert_eq!(check("extrude e distance=1 mm op=cut"), ["E2501"]);
    assert_eq!(check("hole h type=bogus end=through_all"), ["E2501"]);
    assert_eq!(
        check("hole h type=simple end=through_all"),
        Vec::<&str>::new()
    );
    assert_eq!(
        check("fillet f edges=query(edges, type=cylinder, largest) r=1 mm"),
        Vec::<&str>::new()
    );
    assert_eq!(
        check("fillet f edges=query(edges, type=sphere) r=1 mm"),
        ["E2501"]
    );
    assert_eq!(
        check("extrude e from rect(w=1 mm, h=1 mm) distance=1 mm"),
        Vec::<&str>::new()
    );
}

#[test]
fn part_count() {
    assert_eq!(codes(&analyze("tcad 0.1\n")), ["E2701"]);
    assert_eq!(
        codes(&analyze("tcad 0.1\npart a { }\npart b { }\n")),
        ["E2701"]
    );
    let a = analyze("tcad 0.1\npart a { }\npart b { }\n");
    assert!(a.model.is_some());
}

#[test]
fn syntax_errors_stop_semantic_analysis() {
    let a = analyze("tcad 0.1\npart p {\n param a = = 1\n param b = nope\n}\n");
    assert!(a.model.is_none());
    assert!(
        codes(&a).iter().all(|c| c.starts_with("E1")),
        "{:?}",
        codes(&a)
    );
}

#[test]
fn diagnostics_are_sorted_and_unique() {
    let a = analyze(&part(
        "param a = 1 mm + 1 deg\nparam b = a\nconfig default c { a = 1 mm + 1 deg }\nconfig d { }",
    ));
    let starts: Vec<usize> = a.diagnostics.iter().map(|d| d.span.start).collect();
    assert!(starts.windows(2).all(|w| w[0] <= w[1]));
    let n = a
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E2201")
        .count();
    assert_eq!(
        n, 2,
        "one per source location, not per configuration: {:#?}",
        a.diagnostics
    );
}
