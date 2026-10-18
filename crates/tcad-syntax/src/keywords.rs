//! Keyword tables. Keywords are contextual in the lexer (every word lexes as an identifier) and
//! are recognized by position in the parser. A *declared name* may never be a keyword.

/// Reference geometry statements.
pub const REF_GEOM_KW: &[&str] = &["plane", "axis", "point", "csys"];

/// Feature statements (provisional list; see `spec/features.md`).
pub const FEATURE_KW: &[&str] = &[
    "extrude",
    "revolve",
    "sweep",
    "loft",
    "hole",
    "fillet",
    "chamfer",
    "shell",
    "draft",
    "rib",
    "linear_pattern",
    "circular_pattern",
    "mirror",
];

/// Sketch entity statements (provisional list; see `spec/sketch.md`).
pub const ENTITY_KW: &[&str] = &[
    "point", "line", "arc", "circle", "ellipse", "spline", "slot", "rect", "polygon", "project",
];

/// Part-level declaration keywords.
pub const DECL_KW: &[&str] = &[
    "part",
    "tolerance",
    "material",
    "param",
    "import",
    "tag",
    "config",
    "sketch",
    "constrain",
];

/// Keywords reserved for later phases (requirements 4.8).
pub const RESERVED_KW: &[&str] = &[
    "assembly",
    "instance",
    "mate",
    "skeleton",
    "bom",
    "pmi",
    "datum_feature",
    "gdt",
    "drawing",
    "view",
    "section",
    "surface",
    "boundary",
    "fill",
    "sheet_metal",
    "flange",
    "bend",
    "flat_pattern",
    "weldment",
    "direct_edit",
];

/// Words with a fixed role inside statements.
pub const CLAUSE_KW: &[&str] = &[
    "tcad",
    "from",
    "on",
    "suppress",
    "if",
    "range",
    "doc",
    "default",
    "construction",
    "true",
    "false",
    "and",
    "or",
    "not",
    "query",
    "origin",
];

/// Built-in functions.
pub const FUNC_NAMES: &[&str] = &[
    "min", "max", "abs", "sqrt", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "floor",
    "ceil", "round",
];

/// Unit names (requirements 4.4). Composite units such as `kg/m^3` are built from these.
pub const UNIT_NAMES: &[&str] = &["mm", "cm", "m", "in", "ft", "deg", "rad", "kg", "g", "lb"];

/// Axis and plane names that appear in uppercase (`plane.XZ`, `axis.Y`, `+Z`).
pub const AXIS_NAMES: &[&str] = &["X", "Y", "Z", "XY", "YZ", "XZ"];

/// True if `word` is a unit name.
pub fn is_unit(word: &str) -> bool {
    UNIT_NAMES.contains(&word)
}

/// True if `word` is a valid uppercase axis or plane name.
pub fn is_axis_name(word: &str) -> bool {
    AXIS_NAMES.contains(&word)
}

/// True if `word` is a built-in function name.
pub fn is_func(word: &str) -> bool {
    FUNC_NAMES.contains(&word)
}

/// True if `word` is a reserved-for-later-phases keyword.
pub fn is_reserved(word: &str) -> bool {
    RESERVED_KW.contains(&word)
}

/// True if `word` may not be used as a declared name. Sketch entity words are only reserved
/// inside sketches (`slot` is a fine feature name); part-level words only outside them.
pub fn is_keyword(word: &str, in_sketch: bool) -> bool {
    let scoped: &[&str] = if in_sketch { ENTITY_KW } else { &[] };
    let part_level = |t: &[&str]| !in_sketch && t.contains(&word);
    scoped.contains(&word)
        || (in_sketch && word == "constrain")
        || part_level(REF_GEOM_KW)
        || part_level(FEATURE_KW)
        || part_level(DECL_KW)
        || RESERVED_KW.contains(&word)
        || CLAUSE_KW.contains(&word)
        || FUNC_NAMES.contains(&word)
}
