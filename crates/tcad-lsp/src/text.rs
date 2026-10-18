//! Document text with LSP positions (line, UTF-16 column) and byte offsets.

/// An open document.
pub struct Doc {
    /// The text.
    pub text: String,
    line_starts: Vec<usize>,
}

impl Doc {
    /// Indexes a text.
    pub fn new(text: &str) -> Doc {
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Doc {
            text: text.to_string(),
            line_starts,
        }
    }

    /// `(line, utf16 column)` of a byte offset.
    pub fn position(&self, offset: usize) -> (usize, usize) {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.line_starts.partition_point(|&s| s <= offset) - 1;
        let col = self.text[self.line_starts[line]..offset]
            .chars()
            .map(char::len_utf16)
            .sum();
        (line, col)
    }

    /// Byte offset of `(line, utf16 column)`, clamped to the line.
    pub fn offset(&self, line: usize, col: usize) -> usize {
        let Some(&start) = self.line_starts.get(line) else {
            return self.text.len();
        };
        let end = self
            .line_starts
            .get(line + 1)
            .map_or(self.text.len(), |&e| e.saturating_sub(1));
        let mut units = 0;
        for (i, c) in self.text[start..end].char_indices() {
            if units >= col {
                return start + i;
            }
            units += c.len_utf16();
        }
        end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_round_trip_with_wide_characters() {
        let d = Doc::new("a\nb\u{1F600}c\nlast");
        // the emoji takes two UTF-16 units
        assert_eq!(d.position(d.text.find('c').unwrap()), (1, 3));
        assert_eq!(d.offset(1, 3), d.text.find('c').unwrap());
        assert_eq!(d.offset(2, 99), d.text.len());
        assert_eq!(d.position(0), (0, 0));
    }
}
