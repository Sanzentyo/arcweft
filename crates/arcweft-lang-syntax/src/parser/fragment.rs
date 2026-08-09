/// Parser options shared by full documents and standalone fragments.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ParseOptions {}

/// Completion state produced by one exact standalone grammar entrypoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseCompletion {
    Complete,
    Incomplete { expected: Vec<ExpectedToken> },
    Invalid,
}

/// A syntax token or fragment expected at the parse boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedToken {
    text: String,
}

impl ExpectedToken {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}
