use arcweft_data::{DecodeBudget, DecodeLimits, Result};

pub(crate) fn preflight_yaml_source_scalars(source: &str, limits: &DecodeLimits) -> Result<()> {
    YamlSourceScalarScanner::new(source, limits)?.scan()
}

struct YamlSourceScalarScanner<'source, 'limits> {
    source: &'source str,
    index: usize,
    budget: DecodeBudget<'limits>,
}

impl<'source, 'limits> YamlSourceScalarScanner<'source, 'limits> {
    fn new(source: &'source str, limits: &'limits DecodeLimits) -> Result<Self> {
        Ok(Self {
            source,
            index: 0,
            budget: DecodeBudget::new(source.len(), limits)?,
        })
    }

    fn scan(mut self) -> Result<()> {
        while self.index < self.source.len() {
            let Some(ch) = self.current_char() else {
                break;
            };
            match ch {
                '#' => self.skip_comment(),
                ' ' | '\t' | '\r' | '\n' | ':' | ',' | '[' | ']' | '{' | '}' => {
                    self.advance_char(ch);
                }
                '-' if self.is_sequence_indicator() => self.advance_char(ch),
                '"' => self.scan_double_quoted_scalar()?,
                '\'' => self.scan_single_quoted_scalar()?,
                '|' | '>' if self.is_block_scalar_indicator() => self.scan_block_scalar()?,
                _ => self.scan_plain_scalar()?,
            }
        }
        Ok(())
    }

    fn current_char(&self) -> Option<char> {
        self.source[self.index..].chars().next()
    }

    fn advance_char(&mut self, ch: char) {
        self.index += ch.len_utf8();
    }

    fn skip_comment(&mut self) {
        while let Some(ch) = self.current_char() {
            self.advance_char(ch);
            if ch == '\n' {
                break;
            }
        }
    }

    fn scan_single_quoted_scalar(&mut self) -> Result<()> {
        let mut decoded_len = 0_usize;
        self.index += 1;
        while let Some(ch) = self.current_char() {
            if ch == '\'' {
                self.index += 1;
                if self.current_char() == Some('\'') {
                    decoded_len = decoded_len.saturating_add(1);
                    self.budget.string_len(decoded_len)?;
                    self.index += 1;
                    continue;
                }
                return self.budget.string_len(decoded_len);
            }
            decoded_len = decoded_len.saturating_add(ch.len_utf8());
            self.budget.string_len(decoded_len)?;
            self.advance_char(ch);
        }
        self.budget.string_len(decoded_len)
    }

    fn scan_double_quoted_scalar(&mut self) -> Result<()> {
        let mut decoded_len = 0_usize;
        self.index += 1;
        while let Some(ch) = self.current_char() {
            if ch == '"' {
                self.index += 1;
                return self.budget.string_len(decoded_len);
            }
            if ch == '\\' {
                decoded_len = decoded_len.saturating_add(self.scan_double_escape());
            } else {
                decoded_len = decoded_len.saturating_add(ch.len_utf8());
                self.advance_char(ch);
            }
            self.budget.string_len(decoded_len)?;
        }
        self.budget.string_len(decoded_len)
    }

    fn scan_double_escape(&mut self) -> usize {
        self.index += 1;
        let Some(ch) = self.current_char() else {
            return 0;
        };
        match ch {
            'x' => self.scan_hex_escape(2),
            'u' => self.scan_hex_escape(4),
            'U' => self.scan_hex_escape(8),
            '\r' | '\n' => {
                self.skip_escaped_line_break();
                0
            }
            _ => {
                self.advance_char(ch);
                1
            }
        }
    }

    fn scan_hex_escape(&mut self, digits: usize) -> usize {
        self.index += 1;
        let end = self.index.saturating_add(digits);
        let bytes = self.source.as_bytes();
        if end > bytes.len() || !bytes[self.index..end].iter().all(u8::is_ascii_hexdigit) {
            self.index = self.source.len();
            return 0;
        }
        let value = u32::from_str_radix(&self.source[self.index..end], 16).ok();
        self.index = end;
        value.and_then(char::from_u32).map_or(0, char::len_utf8)
    }

    fn skip_escaped_line_break(&mut self) {
        if self.current_char() == Some('\r') {
            self.index += 1;
        }
        if self.current_char() == Some('\n') {
            self.index += 1;
        }
        while let Some(ch) = self.current_char() {
            match ch {
                ' ' | '\t' => self.advance_char(ch),
                _ => break,
            }
        }
    }

    fn scan_block_scalar(&mut self) -> Result<()> {
        let indicator_indent = self.current_line_indent();
        self.index += 1;
        self.skip_to_next_line();
        let Some(content_indent) = self.next_block_content_indent(indicator_indent) else {
            return Ok(());
        };
        let mut decoded_len = 0_usize;
        while self.index < self.source.len() {
            let line_start = self.index;
            let indent = self.line_indent_at(line_start);
            let line_end = self.line_end_from(line_start);
            if line_start != line_end && indent < content_indent {
                break;
            }
            let content_start =
                line_start.saturating_add(content_indent.min(line_end - line_start));
            decoded_len = decoded_len
                .saturating_add(line_end.saturating_sub(content_start))
                .saturating_add(1);
            self.budget.string_len(decoded_len)?;
            self.index = line_end;
            if self.current_char() == Some('\r') {
                self.index += 1;
            }
            if self.current_char() == Some('\n') {
                self.index += 1;
            }
        }
        Ok(())
    }

    fn scan_plain_scalar(&mut self) -> Result<()> {
        let mut decoded_len = 0_usize;
        let mut trailing_space_len = 0_usize;
        while let Some(ch) = self.current_char() {
            if self.ends_plain_scalar(ch) {
                break;
            }
            decoded_len = decoded_len.saturating_add(ch.len_utf8());
            if matches!(ch, ' ' | '\t' | '\r') {
                trailing_space_len = trailing_space_len.saturating_add(ch.len_utf8());
            } else {
                trailing_space_len = 0;
            }
            self.budget
                .string_len(decoded_len.saturating_sub(trailing_space_len))?;
            self.advance_char(ch);
        }
        self.budget
            .string_len(decoded_len.saturating_sub(trailing_space_len))
    }

    fn ends_plain_scalar(&self, ch: char) -> bool {
        match ch {
            '\n' | ',' | '[' | ']' | '{' | '}' | '"' | '\'' | '#' => true,
            ':' => {
                let next = self.index + ch.len_utf8();
                self.source[next..]
                    .chars()
                    .next()
                    .is_none_or(|next| matches!(next, ' ' | '\t' | '\r' | '\n' | ',' | ']' | '}'))
            }
            _ => false,
        }
    }

    fn is_sequence_indicator(&self) -> bool {
        let line_start = self.source[..self.index]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        self.source[line_start..self.index].trim().is_empty()
            && self.source[self.index + 1..]
                .chars()
                .next()
                .is_none_or(|ch| matches!(ch, ' ' | '\t' | '\r' | '\n'))
    }

    fn is_block_scalar_indicator(&self) -> bool {
        let line_start = self.source[..self.index]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let prefix = self.source[line_start..self.index].trim_end();
        matches!(prefix.chars().last(), Some(':' | '-'))
    }

    fn current_line_indent(&self) -> usize {
        let line_start = self.source[..self.index]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        self.line_indent_at(line_start)
    }

    fn line_indent_at(&self, line_start: usize) -> usize {
        self.source[line_start..]
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count()
    }

    fn line_end_from(&self, line_start: usize) -> usize {
        self.source[line_start..]
            .find(['\r', '\n'])
            .map_or(self.source.len(), |offset| line_start + offset)
    }

    fn skip_to_next_line(&mut self) {
        while let Some(ch) = self.current_char() {
            self.advance_char(ch);
            if ch == '\n' {
                break;
            }
        }
    }

    fn next_block_content_indent(&self, parent_indent: usize) -> Option<usize> {
        let mut index = self.index;
        while index < self.source.len() {
            let line_end = self.line_end_from(index);
            if self.source[index..line_end].trim().is_empty() {
                index = line_end.saturating_add(self.line_break_len_at(line_end));
                continue;
            }
            let indent = self.line_indent_at(index);
            return (indent > parent_indent).then_some(indent);
        }
        None
    }

    fn line_break_len_at(&self, index: usize) -> usize {
        match self.source.as_bytes().get(index..) {
            Some([b'\r', b'\n', ..]) => 2,
            Some([b'\r' | b'\n', ..]) => 1,
            _ => 0,
        }
    }
}
