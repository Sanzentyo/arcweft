use crate::raster::RgbaImage;
use crate::scene::RenderedScene;
use crate::types::{
    ClassificationReport, ClassifierIdentity, ClassifierRun, Completeness, FindingTarget,
    PolicyCategory, PolicyError, PolicyFinding, TextRange,
};
use serde::{Deserialize, Serialize};

/// Borrowed policy input. The classifier never owns filesystem or network access.
#[derive(Clone, Copy, Debug)]
pub enum PolicyInputRef<'a> {
    Text(&'a str),
    Image(&'a RgbaImage),
    RenderedScene(&'a RenderedScene),
}

/// Classification backend boundary. Model loading and execution remain host concerns.
pub trait ContentClassifier {
    fn identity(&self) -> ClassifierIdentity;

    fn classify(&self, input: PolicyInputRef<'_>) -> Result<ClassificationReport, PolicyError>;
}

/// Redaction locality produced by a deterministic text rule.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRuleScope {
    #[default]
    Match,
    Line,
    Whole,
}

impl TextRuleScope {
    fn range(self, text: &str, start: usize, matched_len: usize) -> TextRange {
        match self {
            Self::Match => TextRange::new(start, start + matched_len),
            Self::Line => {
                let line_start = text[..start].rfind('\n').map_or(0, |index| index + 1);
                let suffix_start = start + matched_len;
                let line_end = text[suffix_start..]
                    .find('\n')
                    .map_or(text.len(), |offset| suffix_start + offset);
                TextRange::new(line_start, line_end)
            }
            Self::Whole => TextRange::new(0, text.len()),
        }
    }
}

/// Deterministic exact-text rule used for secret markers and untrusted instructions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextRule {
    pub needle: String,
    pub category: PolicyCategory,
    pub score_milli: u16,
    #[serde(default)]
    pub scope: TextRuleScope,
}

impl TextRule {
    pub fn new(needle: impl Into<String>, category: PolicyCategory, score_milli: u16) -> Self {
        Self {
            needle: needle.into(),
            category,
            score_milli: score_milli.min(1000),
            scope: TextRuleScope::Match,
        }
    }

    #[must_use]
    pub fn with_scope(mut self, scope: TextRuleScope) -> Self {
        self.scope = scope;
        self
    }
}

/// Built-in deterministic classifier. It intentionally reports image/scene
/// modalities as unsupported instead of silently allowing them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuleClassifier {
    identity: ClassifierIdentity,
    rules: Vec<TextRule>,
}

impl RuleClassifier {
    pub fn new(identity: ClassifierIdentity, rules: Vec<TextRule>) -> Self {
        Self { identity, rules }
    }

    pub fn strict_builtin() -> Self {
        Self::new(
            ClassifierIdentity::new("arcweft.rules", "2026-06-22"),
            vec![
                TextRule::new("BEGIN PRIVATE KEY", PolicyCategory::security_secret(), 1000)
                    .with_scope(TextRuleScope::Whole),
                TextRule::new("api_key=", PolicyCategory::security_secret(), 950)
                    .with_scope(TextRuleScope::Line),
                TextRule::new(
                    "ignore previous instructions",
                    PolicyCategory::untrusted_instruction(),
                    900,
                )
                .with_scope(TextRuleScope::Line),
                TextRule::new("社外秘", PolicyCategory::security_confidential(), 900),
            ],
        )
    }

    fn classify_text(&self, text: &str) -> ClassificationReport {
        let findings = self
            .rules
            .iter()
            .flat_map(|rule| {
                text.match_indices(rule.needle.as_str())
                    .map(move |(start, matched)| {
                        PolicyFinding::new(
                            rule.category.clone(),
                            rule.score_milli,
                            FindingTarget::Text {
                                range: rule.scope.range(text, start, matched.len()),
                            },
                        )
                    })
            })
            .collect();
        ClassificationReport {
            findings,
            runs: vec![ClassifierRun::complete(self.identity.clone())],
        }
    }
}

impl ContentClassifier for RuleClassifier {
    fn identity(&self) -> ClassifierIdentity {
        self.identity.clone()
    }

    fn classify(&self, input: PolicyInputRef<'_>) -> Result<ClassificationReport, PolicyError> {
        Ok(match input {
            PolicyInputRef::Text(text) => self.classify_text(text),
            PolicyInputRef::Image(_) | PolicyInputRef::RenderedScene(_) => ClassificationReport {
                findings: Vec::new(),
                runs: vec![ClassifierRun::not_applicable(self.identity.clone())],
            },
        })
    }
}

/// Runs two independent classifiers and merges their typed reports.
#[derive(Clone, Debug)]
pub struct CompositeClassifier<A, B> {
    first: A,
    second: B,
}

impl<A, B> CompositeClassifier<A, B> {
    pub const fn new(first: A, second: B) -> Self {
        Self { first, second }
    }
}

impl<A, B> ContentClassifier for CompositeClassifier<A, B>
where
    A: ContentClassifier,
    B: ContentClassifier,
{
    fn identity(&self) -> ClassifierIdentity {
        let first = self.first.identity();
        let second = self.second.identity();
        ClassifierIdentity::new(
            format!("{}+{}", first.id, second.id),
            format!("{}+{}", first.revision, second.revision),
        )
    }

    fn classify(&self, input: PolicyInputRef<'_>) -> Result<ClassificationReport, PolicyError> {
        let first = self
            .first
            .classify(input)
            .unwrap_or_else(|error| ClassificationReport {
                findings: Vec::new(),
                runs: vec![ClassifierRun::incomplete(
                    self.first.identity(),
                    Completeness::Failed,
                    error.receipt_code(),
                )],
            });
        let second = self
            .second
            .classify(input)
            .unwrap_or_else(|error| ClassificationReport {
                findings: Vec::new(),
                runs: vec![ClassifierRun::incomplete(
                    self.second.identity(),
                    Completeness::Failed,
                    error.receipt_code(),
                )],
            });
        Ok(first.merge(second))
    }
}
