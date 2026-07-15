//! Canonical native Style selector state inventory.

use super::value::{ViewLengthMilli, ViewRatioMilli};
use crate::{ViewElementKind, ViewPartName};
use arcweft_presentation::appearance::{ColorScheme, ContrastPreference};
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::interaction::InteractionState;
use serde::{Deserialize, Serialize};

mod codec;

/// Pseudo-state selector evaluated from the shared presentation interaction state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewInteractionSelector {
    Hovered,
    Focused,
    Pressed,
    Disabled,
}

/// Element-owned pseudo-state used by native Style selectors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewElementState {
    FocusVisible,
    ReadOnly,
    Invalid,
    Composing,
    PlaceholderShown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewStyleCombinator {
    Descendant,
    Child,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewStyleComparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewEnvironmentPredicate {
    ReduceMotion(bool),
    ColorScheme(ViewStyleComparison, ColorScheme),
    Contrast(ViewStyleComparison, ContrastPreference),
    TextScale(ViewStyleComparison, ViewRatioMilli),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewContainerAxis {
    InlineSize,
    BlockSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewContainerPredicate {
    axis: ViewContainerAxis,
    comparison: ViewStyleComparison,
    threshold: ViewLengthMilli,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewStylePredicate {
    Interaction(ViewInteractionSelector),
    ElementState(ViewElementState),
    Environment(ViewEnvironmentPredicate),
    Container(ViewContainerPredicate),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewStyleSelectorSequence {
    relation_to_previous: Option<ViewStyleCombinator>,
    element: Option<ViewElementKind>,
    part: Option<ViewPartName>,
    predicates: Vec<ViewStylePredicate>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewStyleSelector {
    sequences: Vec<ViewStyleSelectorSequence>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewStyleSpecificity {
    predicates: u16,
    elements: u16,
}

impl ViewInteractionSelector {
    pub const ALL: &'static [Self] = &[Self::Hovered, Self::Focused, Self::Pressed, Self::Disabled];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Hovered => "hover",
            Self::Focused => "focus",
            Self::Pressed => "active",
            Self::Disabled => "disabled",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|selector| selector.source_name() == value)
    }

    pub const fn cascade() -> [Self; 4] {
        [Self::Hovered, Self::Focused, Self::Pressed, Self::Disabled]
    }

    pub fn matches(
        self,
        target: Option<&InteractionTarget>,
        enabled: bool,
        interaction: &InteractionState,
    ) -> bool {
        match self {
            Self::Hovered => target.is_some_and(|target| interaction.is_hovered(target)),
            Self::Focused => target.is_some_and(|target| interaction.is_focused(target)),
            Self::Pressed => target.is_some_and(|target| interaction.is_pressed(target)),
            Self::Disabled => !enabled,
        }
    }
}

impl ViewElementState {
    pub const ALL: &'static [Self] = &[
        Self::FocusVisible,
        Self::ReadOnly,
        Self::Invalid,
        Self::Composing,
        Self::PlaceholderShown,
    ];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::FocusVisible => "focus-visible",
            Self::ReadOnly => "read-only",
            Self::Invalid => "invalid",
            Self::Composing => "composing",
            Self::PlaceholderShown => "placeholder-shown",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|state| state.source_name() == value)
    }
}

impl ViewContainerPredicate {
    pub const fn new(
        axis: ViewContainerAxis,
        comparison: ViewStyleComparison,
        threshold: ViewLengthMilli,
    ) -> Self {
        Self {
            axis,
            comparison,
            threshold,
        }
    }

    pub const fn axis(self) -> ViewContainerAxis {
        self.axis
    }

    pub const fn comparison(self) -> ViewStyleComparison {
        self.comparison
    }

    pub const fn threshold(self) -> ViewLengthMilli {
        self.threshold
    }
}

impl ViewStyleSelectorSequence {
    pub fn new(
        relation_to_previous: Option<ViewStyleCombinator>,
        element: Option<ViewElementKind>,
        part: Option<ViewPartName>,
        predicates: Vec<ViewStylePredicate>,
    ) -> Option<Self> {
        (element.is_some() || part.is_some() || !predicates.is_empty()).then_some(Self {
            relation_to_previous,
            element,
            part,
            predicates,
        })
    }

    pub const fn relation_to_previous(&self) -> Option<ViewStyleCombinator> {
        self.relation_to_previous
    }

    pub const fn element(&self) -> Option<ViewElementKind> {
        self.element
    }

    pub const fn part(&self) -> Option<&ViewPartName> {
        self.part.as_ref()
    }

    pub fn predicates(&self) -> &[ViewStylePredicate] {
        &self.predicates
    }
}

impl ViewStyleSelector {
    pub fn new(sequences: Vec<ViewStyleSelectorSequence>) -> Option<Self> {
        let valid_relations = sequences.first().is_some_and(|first| {
            first.relation_to_previous().is_none()
                && sequences
                    .iter()
                    .skip(1)
                    .all(|sequence| sequence.relation_to_previous().is_some())
        });
        valid_relations.then_some(Self { sequences })
    }

    pub fn sequences(&self) -> &[ViewStyleSelectorSequence] {
        &self.sequences
    }

    /// Returns the exact cascade specificity when both typed counters fit.
    ///
    /// Callers must reject an unrepresentable selector instead of allowing
    /// saturation to collapse two distinct priorities.
    pub fn specificity(&self) -> Option<ViewStyleSpecificity> {
        self.sequences
            .iter()
            .try_fold(ViewStyleSpecificity::default(), |specificity, sequence| {
                let sequence_predicates = u16::try_from(sequence.predicates.len())
                    .ok()?
                    .checked_add(u16::from(sequence.part.is_some()))?;
                Some(ViewStyleSpecificity {
                    predicates: specificity.predicates.checked_add(sequence_predicates)?,
                    elements: specificity
                        .elements
                        .checked_add(u16::from(sequence.element.is_some()))?,
                })
            })
    }

    /// Number of selector sequences crossed from the scoped root to the target.
    pub const fn max_depth(&self) -> usize {
        self.sequences.len()
    }

    /// Element constrained by the final selector sequence, when explicit.
    pub fn target_element(&self) -> Option<ViewElementKind> {
        self.sequences
            .last()
            .and_then(ViewStyleSelectorSequence::element)
    }
}

impl ViewStyleSpecificity {
    pub const fn predicates(self) -> u16 {
        self.predicates
    }

    pub const fn elements(self) -> u16 {
        self.elements
    }
}
