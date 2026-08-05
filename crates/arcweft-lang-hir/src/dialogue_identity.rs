//! HIR-only dialogue speaker normalization.
//!
//! Durable dialogue line and text identities are owned by `arcweft-id`.
//! Speaker surface parsing remains here until syntax exposes a typed callee
//! boundary instead of a source string.

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
    use super::DialogueSpeakerSlug;

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
}
