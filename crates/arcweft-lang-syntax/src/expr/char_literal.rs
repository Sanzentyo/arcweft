pub(super) fn suffix_boundary(tail: &str) -> bool {
    tail.chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || matches!(ch, ')' | ']' | '}' | ',' | ';'))
}

pub(super) fn decode(source: &str) -> Result<char, String> {
    let mut chars = source.chars();
    let value = match chars.next() {
        Some('\\') => decode_escape(&mut chars)?,
        Some(value) => value,
        None => return Err("char literal must contain exactly one Unicode scalar value".to_owned()),
    };
    if chars.next().is_some() {
        return Err("char literal must contain exactly one Unicode scalar value".to_owned());
    }
    Ok(value)
}

fn decode_escape(chars: &mut core::str::Chars<'_>) -> Result<char, String> {
    match chars.next() {
        Some('n') => Ok('\n'),
        Some('r') => Ok('\r'),
        Some('t') => Ok('\t'),
        Some('0') => Ok('\0'),
        Some('\\') => Ok('\\'),
        Some('"') => Ok('"'),
        Some('u') => decode_unicode_escape(chars),
        Some(other) => Err(format!("unsupported char escape `\\{other}`")),
        None => Err("unterminated char escape".to_owned()),
    }
}

fn decode_unicode_escape(chars: &mut core::str::Chars<'_>) -> Result<char, String> {
    if chars.next() != Some('{') {
        return Err("unicode char escape must use `\\u{...}`".to_owned());
    }
    let mut digits = String::new();
    for ch in chars.by_ref() {
        if ch == '}' {
            let value = u32::from_str_radix(&digits, 16)
                .map_err(|_| "invalid unicode char escape".to_owned())?;
            return char::from_u32(value)
                .ok_or_else(|| "unicode char escape is not a valid scalar value".to_owned());
        }
        digits.push(ch);
    }
    Err("unterminated unicode char escape".to_owned())
}
