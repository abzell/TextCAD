//! Number normalization for the canonical form.

/// Normalizes a numeric literal's text without going through floating point: leading zeros of the
/// integer part and trailing zeros of the fraction are dropped, the exponent is lowercase `e`
/// without `+` or leading zeros, and a zero exponent is removed. `1E-06` becomes `1e-6`.
pub fn normalize_number(text: &str) -> String {
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(i) => (&text[..i], Some(&text[i + 1..])),
        None => (text, None),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let int = int.trim_start_matches('0');
    let frac = frac.trim_end_matches('0');
    let mut out = String::new();
    out.push_str(if int.is_empty() { "0" } else { int });
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    if let Some(exp) = exponent {
        let (neg, digits) = match exp.strip_prefix('-') {
            Some(d) => (true, d),
            None => (false, exp.strip_prefix('+').unwrap_or(exp)),
        };
        let digits = digits.trim_start_matches('0');
        if !digits.is_empty() {
            out.push('e');
            if neg {
                out.push('-');
            }
            out.push_str(digits);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::normalize_number;

    #[test]
    fn normalizes() {
        for (input, want) in [
            ("25", "25"),
            ("25.0", "25"),
            ("25.50", "25.5"),
            ("007", "7"),
            ("0.5", "0.5"),
            ("00.50", "0.5"),
            ("1e-6", "1e-6"),
            ("1E-06", "1e-6"),
            ("1.0e+3", "1e3"),
            ("2e0", "2"),
            ("2e-0", "2"),
            ("0", "0"),
            ("0.0", "0"),
        ] {
            assert_eq!(normalize_number(input), want, "{input}");
        }
    }

    #[test]
    fn idempotent() {
        for s in ["1e-6", "25.5", "0", "3e10", "0.001"] {
            assert_eq!(normalize_number(&normalize_number(s)), normalize_number(s));
        }
    }
}
