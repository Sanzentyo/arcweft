//! Canonical named argument paths, distinct from lexical identifiers.

use super::{SyntaxName, SyntaxNameIssue};

/// One or more admitted identifier segments joined by dots at a call site.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallArgumentName(Box<str>);

impl CallArgumentName {
    /// Admits an API or schema spelling using the shared identifier grammar.
    pub fn try_new(spelling: &str) -> Result<Self, SyntaxNameIssue> {
        if spelling.is_empty() {
            return Err(SyntaxNameIssue::Missing);
        }
        let mut segments = Vec::new();
        for (index, segment) in spelling.split('.').enumerate() {
            match SyntaxName::try_new(segment) {
                Ok(segment) => segments.push(segment),
                Err(SyntaxNameIssue::InvalidStart { .. } | SyntaxNameIssue::Missing)
                    if index == 0 =>
                {
                    return Err(SyntaxNameIssue::InvalidStart {
                        spelling: spelling.into(),
                    });
                }
                Err(_) => {
                    return Err(SyntaxNameIssue::InvalidContinuation {
                        spelling: spelling.into(),
                    });
                }
            }
        }
        Self::try_from_segments(segments)
    }

    /// Source grammar supplies its already-admitted lexer identifier segments.
    pub(crate) fn try_from_segments(
        segments: impl IntoIterator<Item = SyntaxName>,
    ) -> Result<Self, SyntaxNameIssue> {
        let mut spelling = String::new();
        for segment in segments {
            if !spelling.is_empty() {
                spelling.push('.');
            }
            spelling.push_str(segment.as_str());
        }
        if spelling.is_empty() {
            Err(SyntaxNameIssue::Missing)
        } else {
            Ok(Self(spelling.into_boxed_str()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Exact owner-issued identity bytes of the admitted segment path.
    #[must_use]
    pub fn canonical_identity_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Projects into the lexical formal-parameter namespace when applicable.
    #[must_use]
    pub fn single(&self) -> Option<&str> {
        (!self.0.contains('.')).then_some(self.as_str())
    }

    pub fn segments(&self) -> impl DoubleEndedIterator<Item = &str> {
        self.0.split('.')
    }
}

#[cfg(test)]
mod tests {
    use super::{CallArgumentName, SyntaxName, SyntaxNameIssue};

    #[test]
    fn canonical_paths_keep_segment_identity_and_lexical_names_remain_distinct() {
        for spelling in [
            "value",
            "playback.local_time",
            "proxy.param.channel",
            "配置.幅",
        ] {
            let name = CallArgumentName::try_new(spelling).expect("admitted argument path");
            assert_eq!(name.as_str(), spelling);
            assert_eq!(name.canonical_identity_bytes(), spelling.as_bytes());
            assert_eq!(name.segments().collect::<Vec<_>>().join("."), spelling);
            assert_eq!(name.single().is_some(), !spelling.contains('.'));
        }
        assert!(SyntaxName::try_new("playback.local_time").is_err());
    }

    #[test]
    fn missing_and_malformed_paths_do_not_publish_a_partial_identity() {
        assert_eq!(CallArgumentName::try_new(""), Err(SyntaxNameIssue::Missing));
        for spelling in [
            ".value",
            "1value",
            "value.",
            "value..member",
            "value.1member",
            "value/member",
            "value member",
        ] {
            assert!(CallArgumentName::try_new(spelling).is_err(), "{spelling}");
        }
    }
}
