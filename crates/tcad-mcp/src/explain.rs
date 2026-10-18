//! Longer explanations of diagnostic codes for agents: what it means, how to fix it.

/// `(code, what it means, how to fix it)`.
const NOTES: &[(&str, &str, &str)] = &[
    (
        "E0001",
        "The statement uses a construct that is reserved for a later phase of the format (assemblies, drawings, sheet metal ...).",
        "Remove it; Phase 1 describes single parts only.",
    ),
    (
        "E0101",
        "The file header names a format version whose major number this tool does not know.",
        "Write `tcad 0.1` on the first line, or run `tcad migrate`.",
    ),
    (
        "W0102",
        "The file was written for a newer minor version of the format.",
        "Update tcad, or lower the version if the file does not use newer constructs.",
    ),
    (
        "E1001",
        "A character that is not part of the language.",
        "Remove or replace it; text belongs in strings or comments.",
    ),
    (
        "E1002",
        "A string literal has no closing quote on its line.",
        "Add the closing `\"`.",
    ),
    (
        "E1003",
        "A `/* ... */` comment is never closed.",
        "Add `*/`.",
    ),
    (
        "E1004",
        "A string contains an escape the language does not know.",
        "Use \\\" \\\\ \\n \\t or remove the backslash.",
    ),
    (
        "E1005",
        "The file contains carriage returns or a byte order mark.",
        "Save the file as UTF-8 with LF line endings; `tcad fmt` fixes it.",
    ),
    (
        "E1006",
        "Names are lowercase with digits and underscores.",
        "Rename, for example `Rod_Eye` to `rod_eye`.",
    ),
    (
        "E1101",
        "The first line must be `tcad <major>.<minor>`.",
        "Add `tcad 0.1` as the first line.",
    ),
    (
        "E1102",
        "A specific token was expected here and something else was found.",
        "Compare with the grammar in spec/grammar.ebnf; the message names the expected token.",
    ),
    (
        "E1103",
        "A token that cannot appear at this position.",
        "Check for a missing `=`, bracket or comma just before the marked token.",
    ),
    (
        "E1104",
        "The name is reserved (a keyword or function such as `round`, `floor`, `flange`, `bend`, `section`).",
        "Choose another name.",
    ),
    (
        "E1201",
        "The statement starts with a word that is not a statement keyword in this scope.",
        "Check the spelling; features are extrude, revolve, sweep, loft, hole, fillet, chamfer, shell, draft, rib, linear_pattern, circular_pattern, mirror, combine.",
    ),
    (
        "E1202",
        "An axis or plane name that does not exist.",
        "Use X, Y, Z, plane.XY, plane.YZ or plane.XZ (uppercase).",
    ),
    (
        "E1203",
        "Something follows a complete statement on the same line.",
        "Put it on a new line or separate statements with `;`.",
    ),
    (
        "E1204",
        "An expression was expected.",
        "Add the missing value after `=` or inside the brackets.",
    ),
    (
        "E1205",
        "A `{` block is not closed.",
        "Add the closing `}`.",
    ),
    (
        "E2101",
        "Two statements, parameters or sketch entities have the same name.",
        "Rename one of them; names are unique in their scope.",
    ),
    (
        "E2102",
        "The name is not defined before this point; the message may suggest the closest name.",
        "Declare it earlier or fix the spelling.",
    ),
    (
        "E2103",
        "References point backward only: a name is used before its declaration.",
        "Move the declaration above its first use.",
    ),
    (
        "E2104",
        "The name exists but is not a value (it is a feature or a sketch) where a value is needed.",
        "Use a parameter, or a property of the entity.",
    ),
    (
        "E2201",
        "Quantities with different dimensions are combined (for example a length plus an angle).",
        "Check the units: `10 mm + 5 deg` is an error; `10 mm * 2` is fine.",
    ),
    (
        "E2202",
        "A value of the wrong type (number, string, boolean, tuple).",
        "Check the argument's type in spec/features.md or spec/sketch.md.",
    ),
    (
        "E2204",
        "A power or root of a quantity does not produce a whole dimension.",
        "Use integer powers of dimensional values.",
    ),
    (
        "E2205",
        "Division by zero or a non-finite result.",
        "Check the parameters that feed the expression.",
    ),
    (
        "E2206",
        "A function was called with the wrong number or kind of arguments.",
        "Functions: min max abs sqrt sin cos tan asin acos atan atan2 floor ceil round.",
    ),
    (
        "E2301",
        "A parameter's value is outside its declared `range(lo, hi)`.",
        "Change the value or widen the range.",
    ),
    (
        "E2302",
        "The tolerance must be a positive length.",
        "Write for example `tolerance 1e-6 mm`.",
    ),
    (
        "E2303",
        "A single-use declaration (`tolerance`, `material`) appears twice.",
        "Keep one.",
    ),
    (
        "E2304",
        "A range's bounds are wrong (not lengths of the same dimension as the value, or low above high).",
        "Write `range(10 mm, 60 mm)` with the same dimension as the parameter.",
    ),
    (
        "E2305",
        "The material is invalid (for example a density that is not a mass per volume).",
        "Write `material \"AISI 1045\" density=7.85 g/cm^3`.",
    ),
    (
        "E2401",
        "A configuration overrides something that is not a parameter.",
        "Override only declared parameters.",
    ),
    (
        "E2402",
        "The override has another type or dimension than the parameter.",
        "Give a value of the same dimension.",
    ),
    (
        "E2403",
        "A configuration suppresses a feature that does not exist.",
        "Use the name of an existing feature.",
    ),
    (
        "E2404",
        "Only one configuration can be the default.",
        "Mark one with `default`.",
    ),
    (
        "W2405",
        "Configurations exist but none is the default; the base part is built.",
        "Mark one `default` if a variant should build by default.",
    ),
    (
        "E2501",
        "An enumerated argument got a word it does not accept; the message lists the choices.",
        "Use one of the listed words.",
    ),
    (
        "E2601",
        "`suppress if` needs a boolean expression.",
        "Use a comparison, for example `suppress if pin_dia < 12 mm`.",
    ),
    (
        "E2701",
        "A file holds exactly one part.",
        "Split the file or remove the extra `part`.",
    ),
    (
        "E3001",
        "A face or edge reference does not exist in the current shape; the message gives the nearest names and a fix hint.",
        "Use `resolve_reference` or `query_topology` to see the names that exist; names come from `<feature>.<role>`, for example `body.cap_end`.",
    ),
    (
        "E3002",
        "A name matches several faces because a later feature split one (for example a slot cut a face in two).",
        "Add the ordinal the message lists: `body.cap_end#2`.",
    ),
    (
        "E3003",
        "A query matched nothing, or a different number of entities than needed.",
        "Loosen or add filters; run `query_topology` to see candidates.",
    ),
    (
        "E3004",
        "A query filter is unknown or its value is invalid.",
        "Filters: type, normal, parallel_to, axis, radius, created_by, adjacent_to, nearest, largest, smallest.",
    ),
    (
        "E3005",
        "The reference is an edge where a face is needed, or the reverse.",
        "Use `edge(a & b)` for edges and plain names for faces.",
    ),
    (
        "E4001",
        "A sketch entity has a missing, unknown or conflicting argument.",
        "See the entity table in spec/sketch.md.",
    ),
    (
        "E4002",
        "No such sketch constraint.",
        "Constraints: coincident horizontal vertical parallel perpendicular tangent equal symmetric concentric midpoint fix on_curve distance hdistance vdistance radius diameter angle.",
    ),
    (
        "E4003",
        "A constraint has the wrong arguments or a dimension without a value (or a value where none is expected).",
        "Dimensions need `= value`, other constraints take none.",
    ),
    (
        "E4101",
        "Constraints contradict each other; the message names the smallest conflicting set.",
        "Remove or relax one of the named constraints.",
    ),
    (
        "W4102",
        "A constraint repeats what other constraints already fix.",
        "Remove it, or keep it as a harmless duplicate.",
    ),
    (
        "I4201",
        "The sketch still has degrees of freedom (information only).",
        "Add constraints if the shape must not move; it is fine for the solver to keep the written coordinates.",
    ),
    (
        "E4301",
        "The solver cannot satisfy the constraints near the written coordinates.",
        "Move the starting coordinates closer to the intended solution.",
    ),
    (
        "W4601",
        "`project` geometry is only known once the kernel runs.",
        "Informational; build the part to use it.",
    ),
    (
        "E5001",
        "A feature argument is unknown, missing, misplaced or conflicting.",
        "The message lists the accepted arguments; see spec/features.md.",
    ),
    (
        "E5002",
        "A reference does not denote the kind of geometry the feature needs (a plane, an axis, a sketch).",
        "Use plane.XY / a `plane` feature / a planar face for planes; a sketch name for profiles.",
    ),
    (
        "E5003",
        "The construct is not supported yet.",
        "Use a supported alternative or see docs/progress.md.",
    ),
    (
        "E5004",
        "The geometry kernel could not make the feature (invalid geometry, a radius that is too large, a path that does not match the profile ...).",
        "Check dimensions against the part (a fillet radius must be smaller than the faces it rounds; sweep profiles must sit at the path start).",
    ),
    (
        "E5005",
        "The profile cannot be made into a face: it is not closed (the message gives the gap), it crosses itself, or the sketch is empty.",
        "Close the loop or constrain the end points to coincide.",
    ),
    (
        "E5006",
        "The feature needs a body that does not exist yet (for example `subtract` or `hole` before the first `extrude`).",
        "Add a solid first; the first solid feature must add.",
    ),
];

/// The explanation of a code as `(title, meaning, fix)`.
pub fn explain(code: &str) -> Option<(&'static str, &'static str, &'static str)> {
    let title = tcad_diag::codes::REGISTRY
        .iter()
        .find(|(c, _)| c.as_str() == code)
        .map(|(_, t)| *t)?;
    let (_, meaning, fix) = NOTES.iter().find(|(c, _, _)| *c == code)?;
    Some((title, meaning, fix))
}

/// Every code that has an explanation.
pub fn codes() -> impl Iterator<Item = &'static str> {
    NOTES.iter().map(|(c, _, _)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every registered diagnostic code has an explanation.
    #[test]
    fn every_code_is_explained() {
        let missing: Vec<&str> = tcad_diag::codes::REGISTRY
            .iter()
            .map(|(c, _)| c.as_str())
            .filter(|c| explain(c).is_none())
            .collect();
        assert!(missing.is_empty(), "no explanation for {missing:?}");
    }
}
