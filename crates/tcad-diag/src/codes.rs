//! The diagnostic code registry. Codes are stable: never renumber or reuse one.
//!
//! Ranges: `E0xxx` general, `E1xxx` lexical and syntax, `E2xxx` types, units and parameters,
//! `E3xxx` naming and references, `E4xxx` sketch solver, `E5xxx` kernel. `W` codes are warnings.

use serde::{Serialize, Serializer};

/// A stable diagnostic code such as `E1102`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Code(pub &'static str);

impl Code {
    /// The code as text.
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl Serialize for Code {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.0)
    }
}

macro_rules! registry {
    ($($name:ident = $text:literal, $title:literal;)*) => {
        $(
            #[doc = $title]
            pub const $name: Code = Code($text);
        )*
        /// Every registered code with its title, in numeric order.
        pub const REGISTRY: &[(Code, &str)] = &[$(($name, $title)),*];
    };
}

registry! {
    E0001 = "E0001", "feature not yet supported";
    E0101 = "E0101", "unsupported format major version";
    W0102 = "W0102", "format minor version is newer than supported";
    E1001 = "E1001", "unexpected character";
    E1002 = "E1002", "unterminated string";
    E1003 = "E1003", "unterminated block comment";
    E1004 = "E1004", "invalid escape sequence in string";
    E1005 = "E1005", "carriage return or byte order mark (files must be UTF-8 with LF line endings)";
    E1006 = "E1006", "identifiers must be lowercase";
    E1101 = "E1101", "missing or malformed file header";
    E1102 = "E1102", "expected a specific token";
    E1103 = "E1103", "unexpected token";
    E1104 = "E1104", "reserved word used as a name";
    E1201 = "E1201", "unknown statement";
    E1202 = "E1202", "unknown axis or plane name";
    E1203 = "E1203", "expected end of statement";
    E1204 = "E1204", "expected an expression";
    E1205 = "E1205", "unclosed block";
    E2101 = "E2101", "name declared more than once";
    E2102 = "E2102", "undefined name";
    E2103 = "E2103", "name used before its declaration";
    E2104 = "E2104", "name does not denote a value";
    E2201 = "E2201", "dimension mismatch";
    E2202 = "E2202", "type mismatch";
    E2204 = "E2204", "invalid exponent or root of a dimensional quantity";
    E2205 = "E2205", "arithmetic error (division by zero or non-finite result)";
    E2206 = "E2206", "wrong function or arguments";
    E2301 = "E2301", "value outside the declared range";
    E2302 = "E2302", "invalid tolerance";
    E2303 = "E2303", "duplicate declaration of a single-use statement";
    E2304 = "E2304", "invalid range";
    E2305 = "E2305", "invalid material";
    E2401 = "E2401", "configuration overrides an unknown parameter";
    E2402 = "E2402", "override does not match the parameter's type or dimension";
    E2403 = "E2403", "configuration suppresses an unknown feature";
    E2404 = "E2404", "more than one default configuration";
    W2405 = "W2405", "configurations exist but none is marked default";
    E2501 = "E2501", "unknown value for an enumerated argument";
    E2601 = "E2601", "suppress condition must be a boolean";
    E2701 = "E2701", "a file must contain exactly one part";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_well_formed_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for (code, title) in REGISTRY {
            let s = code.as_str();
            assert!(s.starts_with('E') || s.starts_with('W'), "{s}");
            assert!(
                s.len() == 5 && s[1..].bytes().all(|b| b.is_ascii_digit()),
                "{s}"
            );
            assert!(!title.is_empty());
            assert!(seen.insert(s), "duplicate code {s}");
        }
        let sorted = REGISTRY.windows(2).all(|w| w[0].0.0[1..] < w[1].0.0[1..]);
        assert!(sorted, "registry must be in numeric order");
    }
}
