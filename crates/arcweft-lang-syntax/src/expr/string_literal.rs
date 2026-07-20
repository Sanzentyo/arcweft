/// Decoded semantic value of a quoted Arcweft string literal body.
///
/// The expression lexer retains the source body without its delimiters so
/// source-aware tooling can preserve authored spelling. Consumers that need
/// the semantic string value use this type instead of independently decoding
/// escape sequences.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedStringLiteral(String);

impl DecodedStringLiteral {
    /// Decodes the raw body retained by [`super::Literal::String`].
    #[must_use]
    pub fn from_raw_body(raw: &str) -> Self {
        let mut decoded = String::with_capacity(raw.len());
        let mut chars = raw.chars();
        while let Some(ch) = chars.next() {
            if ch != '\\' {
                decoded.push(ch);
                continue;
            }
            match chars.next() {
                Some('"') => decoded.push('"'),
                Some('\\') | None => decoded.push('\\'),
                Some('n') => decoded.push('\n'),
                Some('r') => decoded.push('\r'),
                Some('t') => decoded.push('\t'),
                Some('u') => decode_unicode_escape(&mut chars, &mut decoded),
                Some(other) => {
                    decoded.push('\\');
                    decoded.push(other);
                }
            }
        }
        Self(decoded)
    }

    /// Returns the decoded semantic value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes this value and returns its owned string.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

fn decode_unicode_escape(chars: &mut core::str::Chars<'_>, decoded: &mut String) {
    match chars.next() {
        Some('{') => {}
        Some(other) => {
            decoded.push_str("\\u");
            decoded.push(other);
            return;
        }
        None => {
            decoded.push_str("\\u");
            return;
        }
    }
    let mut digits = String::new();
    for ch in chars.by_ref() {
        if ch == '}' {
            if let Some(ch) = u32::from_str_radix(&digits, 16)
                .ok()
                .and_then(char::from_u32)
            {
                decoded.push(ch);
            } else {
                decoded.push_str("\\u{");
                decoded.push_str(&digits);
                decoded.push('}');
            }
            return;
        }
        digits.push(ch);
    }
    decoded.push_str("\\u{");
    decoded.push_str(&digits);
}

#[cfg(test)]
mod tests {
    use super::DecodedStringLiteral;

    #[test]
    fn decodes_runtime_string_escapes_without_normalizing_text() {
        let decoded = DecodedStringLiteral::from_raw_body(r"line\nreview\t\u{732b}\\");
        assert_eq!(decoded.as_str(), "line\nreview\t猫\\");
    }

    #[test]
    fn preserves_unknown_and_malformed_escapes_losslessly() {
        for raw in [r"\q", r"\u1234", r"\u{not-hex}", "\\"] {
            assert_eq!(DecodedStringLiteral::from_raw_body(raw).as_str(), raw);
        }
    }
}
