//! Integration tests for the parser and formatter.

use tcad_diag::codes;
use tcad_syntax::{format_source, parse};

const COMMENTED: &str = "# top\ntcad 0.1 # v\n\n/* a */ part p { # open\n  # lead\n  param a = (1 mm, # c\n    2) # t\n  config x {\n    a = 1; b = 2 # q\n  }\n  sketch s on plane.XY { # k\n    line l start=(0 mm, 0 mm) end=(1 mm, 1 mm) @hint(1, 2)\n  }\n  reserved_like thing\n}\n";

const CLEVIS: &str = include_str!("../../../spec/examples/rod_clevis.tcad");

fn fmt(src: &str) -> String {
    format_source(src).unwrap_or_else(|e| panic!("format failed: {:?}\n{src}", e.diagnostics))
}

fn codes_of(src: &str) -> Vec<&'static str> {
    parse(src)
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect()
}

#[test]
fn rod_clevis_parses_without_diagnostics() {
    let p = parse(CLEVIS);
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
}

#[test]
fn rod_clevis_is_canonical() {
    assert_eq!(fmt(CLEVIS), CLEVIS);
}

#[test]
fn tree_is_lossless() {
    for src in [
        CLEVIS,
        "",
        "tcad 0.1",
        "\r\n$$$",
        "tcad 0.1\npart x {\n/* c */ param a = 1 # t\n",
    ] {
        assert_eq!(parse(src).root.to_source(), src);
    }
}

#[test]
fn normalizes_spacing_numbers_and_argument_order() {
    let src = "tcad 0.1\npart p{\nparam a=25.0mm\nparam b = 1E-06 mm   range(0.50 mm,1.0 mm)\n\
               extrude e from s.o symmetric op=add distance=040 mm\n}\n";
    let want = "tcad 0.1\n\npart p {\n  param a = 25 mm\n  param b = 1e-6 mm range(0.5 mm, 1 mm)\n  \
                extrude e from s.o distance=40 mm op=add symmetric\n}\n";
    assert_eq!(fmt(src), want);
}

#[test]
fn expression_formatting() {
    let src = "tcad 0.1\npart p {\nparam a = -(b+c)*2^-1/ d\nparam c = min(a,b)\n}\n";
    let want = "tcad 0.1\n\npart p {\n  param a = -(b + c) * 2^-1 / d\n  param c = min(a, b)\n}\n";
    assert_eq!(fmt(src), want);
}

#[test]
fn composite_units_and_division() {
    let out = fmt(
        "tcad 0.1\npart p {\nparam a = 7.85 g/cm^3\nparam b = 10 mm / 2\nparam c = 4 kg/m^3\n}\n",
    );
    assert!(out.contains("param a = 7.85 g/cm^3"));
    assert!(out.contains("param b = 10 mm / 2"));
    assert!(out.contains("param c = 4 kg/m^3"));
}

#[test]
fn comments_are_preserved() {
    let src = "# top\ntcad 0.1 # ver\n\n# before part\npart p {\n  # lead\n  param a = 1 # trail\n\n\n  /* blk */ param b = 2\n  # end\n}\n# last\n";
    assert_eq!(
        fmt(src),
        "# top\ntcad 0.1  # ver\n\n# before part\npart p {\n  # lead\n  param a = 1  # trail\n\n  /* blk */ param b = 2\n  # end\n}\n# last\n"
    );
}

#[test]
fn comment_inside_brackets_survives() {
    let src = "tcad 0.1\npart p {\n  tag t = a.b(c, # why\n    d)\n}\n";
    let once = fmt(src);
    assert!(once.contains("# why"));
    assert_eq!(fmt(&once), once);
}

#[test]
fn config_one_line_and_multi_line() {
    let src = "tcad 0.1\npart p {\n  config a {\n    x = 1\n    y = 2\n  }\n  config b {\n  }\n}\n";
    assert_eq!(
        fmt(src),
        "tcad 0.1\n\npart p {\n  config a { x = 1; y = 2 }\n  config b { }\n}\n"
    );
    let long = format!(
        "tcad 0.1\npart p {{\n  config c {{ {} }}\n}}\n",
        (0..12)
            .map(|i| format!("param_number_{i} = {i} mm"))
            .collect::<Vec<_>>()
            .join("; ")
    );
    let out = fmt(&long);
    assert!(
        out.contains("  config c {\n    param_number_0 = 0 mm\n"),
        "{out}"
    );
    assert_eq!(fmt(&out), out);
}

#[test]
fn ordinal_suffix_and_directions() {
    let src = "tcad 0.1\npart p {\n tag t = boss.side(profile.c1)#2\n tag u = f.face(a.b & c.d)\n tag v = s.side(-Z)\n}\n";
    let out = fmt(src);
    assert!(out.contains("tag t = boss.side(profile.c1)#2"));
    assert!(out.contains("tag u = f.face(a.b & c.d)"));
    assert!(out.contains("tag v = s.side(-Z)"));
}

#[test]
fn sketch_hint_and_construction_order() {
    let src = "tcad 0.1\npart p {\n sketch s on plane.XY {\n  circle c @hint(1 mm, 2 mm) construction d=3 mm center=(0 mm, 0 mm)\n }\n}\n";
    let out = fmt(src);
    assert!(
        out.contains("circle c center=(0 mm, 0 mm) d=3 mm construction @hint(1 mm, 2 mm)"),
        "{out}"
    );
}

#[test]
fn reserved_keywords_are_rejected_with_a_message() {
    for kw in tcad_syntax::keywords::RESERVED_KW {
        let top = format!("tcad 0.1\n{kw} thing {{ a b }}\n");
        assert_eq!(codes_of(&top), ["E0001"], "{kw} at top level");
        let inner = format!("tcad 0.1\npart p {{\n  {kw} x = 1\n  param a = 1\n}}\n");
        assert_eq!(codes_of(&inner), ["E0001"], "{kw} in a part");
    }
}

#[test]
fn version_checks() {
    assert_eq!(codes_of("tcad 1.0\n"), ["E0101"]);
    assert_eq!(codes_of("tcad 0.9\n"), ["W0102"]);
    assert!(codes_of("tcad 0.1\n").is_empty());
    assert_eq!(codes_of("part p { }\n")[0], "E1101");
    assert_eq!(codes_of("")[0], "E1101");
    assert_eq!(codes_of("tcad\n")[0], "E1101");
}

#[test]
fn syntax_errors_have_codes_spans_and_hints() {
    let p = parse("tcad 0.1\npart p {\n  param a 5 mm\n}\n");
    let d = &p.diagnostics[0];
    assert_eq!(d.code, codes::E1102);
    assert_eq!(d.span.start, "tcad 0.1\npart p {\n  param a ".len());
    assert!(d.hint.is_some());

    assert_eq!(codes_of("tcad 0.1\npart p {\n  param a = \n}\n"), ["E1204"]);
    assert_eq!(
        codes_of("tcad 0.1\npart p {\n  param a = 1 2\n}\n"),
        ["E1203"]
    );
    assert_eq!(
        codes_of("tcad 0.1\npart p {\n  param plane = 1\n}\n"),
        ["E1104"]
    );
    assert_eq!(
        codes_of("tcad 0.1\npart p {\n  frobnicate x\n}\n"),
        ["E1201"]
    );
    assert_eq!(codes_of("tcad 0.1\npart p {\n  param a = 1\n"), ["E1205"]);
    assert_eq!(codes_of("tcad 0.1\n}\n"), ["E1103"]);
    assert_eq!(codes_of("tcad 0.1\npart p {\n tag t = a.Q\n}\n"), ["E1202"]);
    assert_eq!(
        codes_of("tcad 0.1\npart p {\n  param a = $\n}\n"),
        ["E1001", "E1204"]
    );
}

#[test]
fn recovery_reports_later_errors_too() {
    let src = "tcad 0.1\npart p {\n  param a = \n  param b = 2\n  param c =\n}\n";
    let c = codes_of(src);
    assert_eq!(c, ["E1204", "E1204"]);
}

#[test]
fn format_refuses_invalid_input() {
    assert!(format_source("tcad 0.1\npart p {\n param = 1\n}\n").is_err());
}

#[test]
fn multi_line_brackets_are_allowed() {
    let src = "tcad 0.1\npart p {\n  chamfer c edges=[\n    a.b,\n    c.d\n  ] d=1 mm\n}\n";
    assert_eq!(
        fmt(src),
        "tcad 0.1\n\npart p {\n  chamfer c edges=[a.b, c.d] d=1 mm\n}\n"
    );
}

/// A tiny deterministic PRNG so the mutation test needs no dependencies.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn mutate(rng: &mut Rng, src: &str) -> String {
    const SNIPPETS: &[&str] = &[
        "{", "}", "(", ")", "[", "]", ",", ";", "=", "\n", " ", "#", "/*", "*/", "\"", "\\", "@",
        "+Y", "-", "^", "&", "1e", "mm", "part", "param", "config", "plane.XZ", "\u{feff}", "é",
        "\r",
    ];
    let mut chars: Vec<char> = src.chars().collect();
    for _ in 0..1 + rng.below(6) {
        if chars.is_empty() {
            chars.push(' ');
        }
        let at = rng.below(chars.len() + 1).min(chars.len());
        match rng.below(3) {
            0 if at < chars.len() => {
                chars.remove(at);
            }
            1 => {
                let s = SNIPPETS[rng.below(SNIPPETS.len())];
                for (i, c) in s.chars().enumerate() {
                    chars.insert((at + i).min(chars.len()), c);
                }
            }
            _ => {
                let from = rng.below(chars.len());
                let len = rng.below(20).min(chars.len() - from);
                let piece: Vec<char> = chars[from..from + len].to_vec();
                for (i, c) in piece.into_iter().enumerate() {
                    chars.insert((at + i).min(chars.len()), c);
                }
            }
        }
    }
    chars.into_iter().collect()
}

#[test]
fn mutation_fuzz_never_panics_and_stays_lossless() {
    let iters = std::env::var("TCAD_FUZZ_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..iters {
        let seed = if rng.below(4) == 0 { COMMENTED } else { CLEVIS };
        let src = mutate(&mut rng, seed);
        let p = parse(&src);
        assert_eq!(p.root.to_source(), src, "lossless");
        if let Ok(out) = format_source(&src) {
            let again = format_source(&out).expect("formatted output must parse");
            assert_eq!(again, out, "idempotent for {src:?}");
        }
    }
}
