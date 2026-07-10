//! Dialogue identity rules shared by HIR lowering and source materialization.
//!
//! These rules live in HIR because they normalize language-surface spellings
//! into semantic ID segments. Syntax must preserve authored text, while
//! tooling above HIR must observe the same identities as compilation.

/// ID families owned by dialogue line normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DialogueIdFamily {
    Line,
    Text,
}

impl DialogueIdFamily {
    pub(crate) const fn prefix(self) -> &'static str {
        match self {
            Self::Line => "say",
            Self::Text => "text",
        }
    }

    pub(crate) fn contains(self, body: &str) -> bool {
        self.tail(body).is_some()
    }

    fn tail(self, body: &str) -> Option<&str> {
        let tail = body.strip_prefix(self.prefix())?.strip_prefix('.')?;
        (!tail.is_empty()).then_some(tail)
    }
}

/// A normalized dialogue line ID known to belong to the `say` family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DialogueLineId<'a> {
    tail: &'a str,
}

impl<'a> DialogueLineId<'a> {
    pub(crate) fn parse(body: &'a str) -> Option<Self> {
        DialogueIdFamily::Line.tail(body).map(|tail| Self { tail })
    }

    /// Derives the localization identity by replacing only the owned family.
    pub(crate) fn generated_text_key(self) -> String {
        format!("{}.{}", DialogueIdFamily::Text.prefix(), self.tail)
    }
}

/// Canonical speaker segment inserted into generated dialogue identities.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DialogueSpeakerSlug(String);

impl DialogueSpeakerSlug {
    /// Normalizes bare, method-call, and entity-reference speaker spellings.
    ///
    /// Narrator aliases are deliberately case- and punctuation-insensitive.
    /// Other speaker segments preserve authored case because Arcweft entity
    /// identities are case-sensitive and must not be collapsed by tooling.
    pub(crate) fn from_callee(callee: &str) -> Option<Self> {
        let trimmed = callee.trim();
        let without_method = trimmed.strip_suffix(".say").unwrap_or(trimmed).trim();
        let unwrapped = without_method
            .strip_prefix("@<")
            .and_then(|inner| inner.strip_suffix('>'))
            .or_else(|| without_method.strip_prefix('@'))
            .unwrap_or(without_method);
        let identity = unwrapped.split('@').next().unwrap_or(unwrapped);
        if is_narrator_alias(identity) {
            return Some(Self("narrator".to_owned()));
        }
        let segment = identity
            .rsplit(['.', ':'])
            .next()
            .map(str::trim)
            .filter(|segment| !segment.is_empty())?;
        let normalized = if is_narrator_alias(segment) {
            "narrator"
        } else {
            segment
        };
        Some(Self(normalized.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_narrator_alias(segment: &str) -> bool {
    let normalized = segment
        .chars()
        .filter(|character| !matches!(character, '.' | '_'))
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "地" | "地の文"
            | "地文"
            | "ナレーター"
            | "ナレータ"
            | "ナレーション"
            | "語り"
            | "語り手"
            | "narrator"
            | "narration"
            | "voiceover"
            | "vo"
            | "off"
            | "offscreen"
            | "os"
            | "script"
            | "stagedirection"
            | "ト書き"
            | "脚本"
    )
}

#[cfg(test)]
mod tests {
    use super::{DialogueIdFamily, DialogueLineId, DialogueSpeakerSlug};

    #[test]
    fn speaker_slug_normalizes_every_callee_surface_without_collapsing_case() {
        for (source, expected) in [
            (" Alice ", "Alice"),
            ("Alice.say", "Alice"),
            ("@character:Alice.say", "Alice"),
            ("@<character.Alice>.say", "Alice"),
            ("@<character.Alice@sem:stable>.say", "Alice"),
        ] {
            assert_eq!(
                DialogueSpeakerSlug::from_callee(source)
                    .as_ref()
                    .map(DialogueSpeakerSlug::as_str),
                Some(expected),
                "speaker spelling {source:?}"
            );
        }
    }

    #[test]
    fn narrator_aliases_share_one_reserved_slug() {
        for alias in [
            "地",
            "地の文",
            "地文",
            "ナレーション",
            "Narration",
            "voice_over",
            "V.O.",
            "o.s.",
            "Offscreen",
            "stage_direction",
            "ト書き",
        ] {
            assert_eq!(
                DialogueSpeakerSlug::from_callee(alias)
                    .as_ref()
                    .map(DialogueSpeakerSlug::as_str),
                Some("narrator"),
                "narrator alias {alias:?}"
            );
        }
        assert_eq!(
            DialogueSpeakerSlug::from_callee("@<character.Narrator>.say")
                .as_ref()
                .map(DialogueSpeakerSlug::as_str),
            Some("narrator")
        );
    }

    #[test]
    fn generated_text_key_replaces_only_a_valid_line_family() {
        let line = DialogueLineId::parse("say.Opening.Alice.001").expect("valid line ID");
        assert_eq!(line.generated_text_key(), "text.Opening.Alice.001");
        assert!(DialogueLineId::parse("text.Opening.Alice.001").is_none());
        assert!(DialogueLineId::parse("say.").is_none());
        assert!(DialogueLineId::parse("say").is_none());
        assert!(DialogueIdFamily::Text.contains("text.Opening.Alice.001"));
        assert!(!DialogueIdFamily::Text.contains("say.Opening.Alice.001"));
    }
}
