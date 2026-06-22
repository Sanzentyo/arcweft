use crate::types::{ContentDigest, ContentId, PolicyError, TextRange};
use serde::{Deserialize, Serialize};

/// Authored or derived text submitted to the policy engine.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextArtifact {
    pub id: ContentId,
    pub text: String,
}

/// One applied redaction in original UTF-8 byte coordinates.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextRedaction {
    pub original: TextRange,
    pub replacement_bytes: usize,
}

/// Sanitized text plus an auditable range map.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextSanitization {
    pub artifact: TextArtifact,
    pub redactions: Vec<TextRedaction>,
}

impl TextArtifact {
    pub fn new(id: ContentId, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
        }
    }

    pub fn content_digest(&self) -> ContentDigest {
        ContentDigest::from_bytes(self.text.as_bytes())
    }

    pub fn redacted(
        &self,
        ranges: impl IntoIterator<Item = TextRange>,
        replacement: &str,
        whole_if_empty: bool,
    ) -> Result<TextSanitization, PolicyError> {
        let mut ranges = ranges
            .into_iter()
            .filter(|range| !range.is_empty())
            .collect::<Vec<_>>();
        if ranges.is_empty() && whole_if_empty && !self.text.is_empty() {
            ranges.push(TextRange::new(0, self.text.len()));
        }
        ranges.iter().try_for_each(|range| {
            if range.end <= self.text.len()
                && self.text.is_char_boundary(range.start)
                && self.text.is_char_boundary(range.end)
            {
                Ok(())
            } else {
                Err(PolicyError::InvalidTextRange)
            }
        })?;
        ranges.sort_unstable_by_key(|range| (range.start, range.end));
        let merged = ranges
            .into_iter()
            .fold(Vec::<TextRange>::new(), |mut output, range| {
                match output.last_mut() {
                    Some(previous) if range.start <= previous.end => {
                        previous.end = previous.end.max(range.end);
                    }
                    _ => output.push(range),
                }
                output
            });
        let mut cursor = 0;
        let mut text = String::with_capacity(self.text.len());
        let redactions = merged
            .iter()
            .map(|range| {
                text.push_str(&self.text[cursor..range.start]);
                text.push_str(replacement);
                cursor = range.end;
                TextRedaction {
                    original: *range,
                    replacement_bytes: replacement.len(),
                }
            })
            .collect::<Vec<_>>();
        text.push_str(&self.text[cursor..]);
        Ok(TextSanitization {
            artifact: TextArtifact::new(self.id.clone(), text),
            redactions,
        })
    }
}
