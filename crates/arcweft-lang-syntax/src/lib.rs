use arcweft_source::SourceAnchor;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTree {
    source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxError {
    pub message: String,
    pub anchor: SourceAnchor,
}

pub fn parse_stub(source: impl Into<String>) -> Result<SyntaxTree, SyntaxError> {
    Ok(SyntaxTree {
        source: source.into(),
    })
}

impl SyntaxTree {
    pub fn source(&self) -> &str {
        &self.source
    }
}

#[cfg(test)]
mod tests {
    use super::parse_stub;

    #[test]
    fn stub_preserves_source_text() {
        let tree = parse_stub("alice: おはよう。[p]").expect("stub parser succeeds");
        assert_eq!(tree.source(), "alice: おはよう。[p]");
    }
}
