use arcweft_data::{DecodeBudget, DecodeLimits, Result};

pub(crate) fn preflight_toml_source_budget(source: &str, limits: &DecodeLimits) -> Result<()> {
    TomlSourceBudgetScanner::new(source, limits)?.scan()
}

struct TomlSourceBudgetScanner<'source, 'limits> {
    source: &'source str,
    index: usize,
    budget: DecodeBudget<'limits>,
    collections: Vec<TomlSourceCollection>,
    root_items: usize,
    after_key_value_separator: bool,
    line_allows_table_header: bool,
}

impl<'source, 'limits> TomlSourceBudgetScanner<'source, 'limits> {
    fn new(source: &'source str, limits: &'limits DecodeLimits) -> Result<Self> {
        Ok(Self {
            source,
            index: 0,
            budget: DecodeBudget::new(source.len(), limits)?,
            collections: Vec::new(),
            root_items: 0,
            after_key_value_separator: false,
            line_allows_table_header: true,
        })
    }

    fn scan(mut self) -> Result<()> {
        self.budget.enter_node()?;
        while self.index < self.source.len() {
            let Some(ch) = self.current_char() else {
                break;
            };
            match ch {
                ' ' | '\t' | '\r' => self.advance_char(ch),
                '\n' => self.on_newline(),
                '#' => self.skip_comment(),
                '=' => self.on_key_value_separator()?,
                ',' => self.on_value_separator(),
                '[' if self.line_allows_table_header && !self.after_key_value_separator => {
                    self.scan_table_header()?;
                }
                '[' => self.open_array()?,
                ']' => self.close_array(),
                '{' => self.open_inline_table()?,
                '}' => self.close_inline_table(),
                '"' | '\'' => self.scan_string(ch)?,
                _ => {
                    if self.after_key_value_separator || self.array_expects_value() {
                        self.scan_bare_value()?;
                    } else {
                        self.scan_bare_key_or_token()?;
                    }
                }
            }
        }
        self.budget.exit_node();
        Ok(())
    }

    fn current_char(&self) -> Option<char> {
        self.source[self.index..].chars().next()
    }

    fn peek_char(&self, offset: usize) -> Option<char> {
        self.source[self.index + offset..].chars().next()
    }

    fn advance_char(&mut self, ch: char) {
        self.index += ch.len_utf8();
    }

    fn on_newline(&mut self) {
        self.index += 1;
        self.line_allows_table_header = true;
        self.after_key_value_separator = false;
    }

    fn skip_comment(&mut self) {
        self.line_allows_table_header = false;
        while let Some(ch) = self.current_char() {
            self.advance_char(ch);
            if ch == '\n' {
                self.line_allows_table_header = true;
                self.after_key_value_separator = false;
                break;
            }
        }
    }

    fn on_key_value_separator(&mut self) -> Result<()> {
        self.line_allows_table_header = false;
        self.after_key_value_separator = true;
        if let Some(TomlSourceCollection::InlineTable { items }) = self.collections.last_mut() {
            *items = items.saturating_add(1);
            self.budget.map_item(*items)?;
        } else if self.collections.is_empty() {
            self.root_items = self.root_items.saturating_add(1);
            self.budget.map_item(self.root_items)?;
        }
        self.index += 1;
        Ok(())
    }

    fn on_value_separator(&mut self) {
        self.line_allows_table_header = false;
        self.after_key_value_separator = false;
        if let Some(TomlSourceCollection::Array {
            expecting_value, ..
        }) = self.collections.last_mut()
        {
            *expecting_value = true;
        }
        self.index += 1;
    }

    fn open_array(&mut self) -> Result<()> {
        self.start_value()?;
        self.enter_collection(TomlSourceCollection::Array {
            items: 0,
            expecting_value: true,
        })
    }

    fn close_array(&mut self) {
        self.line_allows_table_header = false;
        self.after_key_value_separator = false;
        if matches!(
            self.collections.last(),
            Some(TomlSourceCollection::Array { .. })
        ) {
            self.collections.pop();
            self.budget.exit_node();
        }
        self.index += 1;
    }

    fn open_inline_table(&mut self) -> Result<()> {
        self.start_value()?;
        self.enter_collection(TomlSourceCollection::InlineTable { items: 0 })
    }

    fn close_inline_table(&mut self) {
        self.line_allows_table_header = false;
        self.after_key_value_separator = false;
        if matches!(
            self.collections.last(),
            Some(TomlSourceCollection::InlineTable { .. })
        ) {
            self.collections.pop();
            self.budget.exit_node();
        }
        self.index += 1;
    }

    fn enter_collection(&mut self, collection: TomlSourceCollection) -> Result<()> {
        self.line_allows_table_header = false;
        self.after_key_value_separator = false;
        self.budget.enter_node()?;
        self.collections.push(collection);
        self.index += 1;
        Ok(())
    }

    fn scan_table_header(&mut self) -> Result<()> {
        self.root_items = self.root_items.saturating_add(1);
        self.budget.map_item(self.root_items)?;
        self.budget.enter_node()?;
        let is_array_table = self.peek_char(1) == Some('[');
        self.index += if is_array_table { 2 } else { 1 };
        while self.index < self.source.len() {
            let Some(ch) = self.current_char() else {
                break;
            };
            match ch {
                '"' | '\'' => self.scan_string(ch)?,
                ']' => {
                    self.index += 1;
                    if is_array_table && self.current_char() == Some(']') {
                        self.index += 1;
                    }
                    break;
                }
                '\n' => {
                    self.on_newline();
                    break;
                }
                _ => self.advance_char(ch),
            }
        }
        self.budget.exit_node();
        self.line_allows_table_header = false;
        Ok(())
    }

    fn scan_string(&mut self, quote: char) -> Result<()> {
        self.start_value()?;
        self.line_allows_table_header = false;
        let delimiter_len = if (matches!(quote, '"')
            && self.source[self.index..].starts_with("\"\"\""))
            || (matches!(quote, '\'') && self.source[self.index..].starts_with("'''"))
        {
            3
        } else {
            1
        };
        if quote == '"' {
            self.scan_basic_string(delimiter_len)
        } else {
            self.scan_literal_string(delimiter_len)
        }
    }

    fn scan_literal_string(&mut self, delimiter_len: usize) -> Result<()> {
        let delimiter = "'".repeat(delimiter_len);
        let mut content_len = 0_usize;
        self.index += delimiter_len;
        while self.index < self.source.len() {
            if self.source[self.index..].starts_with(&delimiter) {
                self.index += delimiter_len;
                return self.consume_string_node(content_len);
            }
            let Some(ch) = self.current_char() else {
                break;
            };
            content_len = content_len.saturating_add(ch.len_utf8());
            self.budget.string_len(content_len)?;
            self.advance_char(ch);
        }
        self.consume_string_node(content_len)
    }

    fn scan_basic_string(&mut self, delimiter_len: usize) -> Result<()> {
        let delimiter = "\"".repeat(delimiter_len);
        let mut decoded_len = 0_usize;
        self.index += delimiter_len;
        while self.index < self.source.len() {
            if self.source[self.index..].starts_with(&delimiter) {
                self.index += delimiter_len;
                return self.consume_string_node(decoded_len);
            }
            let Some(ch) = self.current_char() else {
                break;
            };
            if ch == '\\' {
                decoded_len = decoded_len.saturating_add(self.scan_basic_escape(delimiter_len));
            } else {
                decoded_len = decoded_len.saturating_add(ch.len_utf8());
                self.advance_char(ch);
            }
            self.budget.string_len(decoded_len)?;
        }
        self.consume_string_node(decoded_len)
    }

    fn scan_basic_escape(&mut self, delimiter_len: usize) -> usize {
        self.index += 1;
        let Some(ch) = self.current_char() else {
            return 0;
        };
        match ch {
            'u' => self.scan_unicode_escape(4),
            'U' => self.scan_unicode_escape(8),
            '\n' if delimiter_len == 3 => {
                self.index += 1;
                self.skip_multiline_escape_whitespace();
                0
            }
            '\r' if delimiter_len == 3 => {
                self.index += 1;
                if self.current_char() == Some('\n') {
                    self.index += 1;
                }
                self.skip_multiline_escape_whitespace();
                0
            }
            _ => {
                self.advance_char(ch);
                1
            }
        }
    }

    fn scan_unicode_escape(&mut self, digits: usize) -> usize {
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

    fn skip_multiline_escape_whitespace(&mut self) {
        while let Some(ch) = self.current_char() {
            match ch {
                ' ' | '\t' | '\n' => self.advance_char(ch),
                '\r' => {
                    self.index += 1;
                    if self.current_char() == Some('\n') {
                        self.index += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn scan_bare_value(&mut self) -> Result<()> {
        self.start_value()?;
        self.consume_scalar_node()?;
        while let Some(ch) = self.current_char() {
            match ch {
                ',' | ']' | '}' | '#' | '\n' => break,
                _ => self.advance_char(ch),
            }
        }
        Ok(())
    }

    fn scan_bare_key_or_token(&mut self) -> Result<()> {
        self.line_allows_table_header = false;
        let mut token_len = 0_usize;
        while let Some(ch) = self.current_char() {
            match ch {
                '=' | ',' | '[' | ']' | '{' | '}' | '"' | '\'' | '#' | '\n' | ' ' | '\t' | '\r' => {
                    break;
                }
                _ => {
                    token_len = token_len.saturating_add(ch.len_utf8());
                    self.budget.string_len(token_len)?;
                    self.advance_char(ch);
                }
            }
        }
        if token_len > 0 {
            self.consume_string_node(token_len)?;
        }
        Ok(())
    }

    fn start_value(&mut self) -> Result<()> {
        self.line_allows_table_header = false;
        if let Some(TomlSourceCollection::Array {
            items,
            expecting_value,
        }) = self.collections.last_mut()
            && *expecting_value
        {
            *items = items.saturating_add(1);
            self.budget.sequence_item(*items)?;
            *expecting_value = false;
        }
        self.after_key_value_separator = false;
        Ok(())
    }

    fn array_expects_value(&self) -> bool {
        matches!(
            self.collections.last(),
            Some(TomlSourceCollection::Array {
                expecting_value: true,
                ..
            })
        )
    }

    fn consume_scalar_node(&mut self) -> Result<()> {
        self.budget.enter_node()?;
        self.budget.exit_node();
        Ok(())
    }

    fn consume_string_node(&mut self, len: usize) -> Result<()> {
        self.budget.enter_node()?;
        if let Err(error) = self.budget.string_len(len) {
            self.budget.exit_node();
            return Err(error);
        }
        self.budget.exit_node();
        Ok(())
    }
}

enum TomlSourceCollection {
    Array { items: usize, expecting_value: bool },
    InlineTable { items: usize },
}
