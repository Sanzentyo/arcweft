//! Logical codec fields borrowed from the owning dialogue value algebra.

use super::{
    RUNTIME_DIALOGUE_CONTENT_VALUE_VERSION, RuntimeDialogueContentBinding,
    RuntimeDialogueContentEffectBinding, RuntimeDialogueContentValue,
    RuntimeDialogueFormattedFailureSelection, RuntimeDialogueFormattedOutcome,
    RuntimeDialogueFormattedSuccess, RuntimeDialogueFormattedValue, RuntimeDialogueOpaqueRole,
    RuntimeInlineTextValue, RuntimeOpaqueTypeOwner, RuntimeUInt,
};
use crate::value::{
    RuntimeOpaqueView, RuntimeScalarView, RuntimeSequenceView, RuntimeTupleView, RuntimeValueView,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeDialogueTupleView<'a> {
    Content(&'a RuntimeDialogueContentValue),
    Binding(&'a RuntimeDialogueContentBinding),
    Effect(&'a RuntimeDialogueContentEffectBinding),
    InlineText(&'a RuntimeInlineTextValue),
    Formatted(&'a RuntimeDialogueFormattedValue),
    Outcome(&'a RuntimeDialogueFormattedOutcome),
    Failure(&'a RuntimeDialogueFormattedFailureSelection),
    OptionalColor(Option<crate::value::RuntimeColor>),
    OptionalString(Option<&'a str>),
}

impl<'a> RuntimeDialogueTupleView<'a> {
    pub(crate) fn view(self) -> RuntimeValueView<'a> {
        RuntimeValueView::Tuple(RuntimeTupleView::Dialogue(self))
    }

    pub(crate) fn len(self) -> usize {
        match self {
            Self::Content(_) => 6,
            Self::Binding(_) => 4,
            Self::Outcome(_) => 3,
            Self::Failure(RuntimeDialogueFormattedFailureSelection::Inherit) => 1,
            Self::Effect(_) | Self::InlineText(_) | Self::Formatted(_) | Self::Failure(_) => 2,
            Self::OptionalColor(value) => usize::from(value.is_some()),
            Self::OptionalString(value) => usize::from(value.is_some()),
        }
    }

    pub(crate) fn get(self, index: usize) -> Option<RuntimeValueView<'a>> {
        use RuntimeScalarView as Scalar;
        use RuntimeValueView as View;
        let byte = |value| View::Scalar(Scalar::UInt(RuntimeUInt::U8(value)));
        let ordinal = |value| View::Scalar(Scalar::UInt(RuntimeUInt::U32(value)));
        let string = |value| View::Scalar(Scalar::String(value));
        let bytes = |value| View::Sequence(RuntimeSequenceView::Bytes(value));
        Some(match (self, index) {
            (Self::Content(_), 0) => byte(RUNTIME_DIALOGUE_CONTENT_VALUE_VERSION),
            (Self::Content(value), 1) => bytes(value.artifact.as_bytes()),
            (Self::Content(value), 2) => ordinal(value.template.get().get()),
            (Self::Content(value), 3) => bytes(value.template_digest.as_bytes()),
            (Self::Content(value), 4) => {
                View::Sequence(RuntimeSequenceView::DialogueBindings(&value.bindings))
            }
            (Self::Content(value), 5) => {
                View::Sequence(RuntimeSequenceView::DialogueEffects(&value.effects))
            }
            (Self::Binding(value), 0) => ordinal(value.slot().get().get()),
            (Self::Binding(value), 1) => byte(value.role().encoded()),
            (
                Self::Binding(
                    RuntimeDialogueContentBinding::Interpolation { semantic_type, .. }
                    | RuntimeDialogueContentBinding::Content { semantic_type, .. }
                    | RuntimeDialogueContentBinding::Formatted { semantic_type, .. },
                ),
                2,
            ) => bytes(semantic_type.as_bytes()),
            (Self::Binding(RuntimeDialogueContentBinding::Interpolation { value, .. }), 3) => {
                Self::InlineText(value).view()
            }
            (Self::Binding(RuntimeDialogueContentBinding::Content { value, .. }), 3) => {
                value.view()
            }
            (Self::Binding(RuntimeDialogueContentBinding::Formatted { value, .. }), 3) => {
                Self::Formatted(value).view()
            }
            (Self::Effect(value), 0) => ordinal(value.site.get().get()),
            (Self::Effect(value), 1) => {
                View::Callable(crate::value::RuntimeCallableView::Live(&value.callback))
            }
            (Self::InlineText(value), 0) => bytes(value.semantic_type.as_bytes()),
            (Self::InlineText(value), 1) => string(&value.text),
            (Self::Formatted(value), 0) => Self::Outcome(&value.outcome).view(),
            (Self::Formatted(value), 1) => Self::Failure(&value.failure).view(),
            (Self::Outcome(value), index) => return value.borrowed_field(index),
            (Self::Failure(RuntimeDialogueFormattedFailureSelection::Inherit), 0) => byte(0),
            (Self::Failure(RuntimeDialogueFormattedFailureSelection::OnError(_)), 0) => byte(1),
            (Self::Failure(RuntimeDialogueFormattedFailureSelection::Fallback(_)), 0) => byte(2),
            (Self::Failure(RuntimeDialogueFormattedFailureSelection::Discard(_)), 0) => byte(3),
            (Self::Failure(RuntimeDialogueFormattedFailureSelection::OnError(value)), 1) => {
                value.view()
            }
            (Self::Failure(RuntimeDialogueFormattedFailureSelection::Fallback(value)), 1) => {
                string(value)
            }
            (Self::Failure(RuntimeDialogueFormattedFailureSelection::Discard(value)), 1) => {
                View::Scalar(Scalar::Bool(*value))
            }
            (Self::OptionalColor(Some(value)), 0) => View::Scalar(Scalar::Color(value)),
            (Self::OptionalString(Some(value)), 0) => string(value),
            _ => return None,
        })
    }
}

impl RuntimeDialogueContentValue {
    pub(crate) fn view(&self) -> RuntimeValueView<'_> {
        RuntimeValueView::Opaque(RuntimeOpaqueView::DialogueContent(self))
    }

    pub(crate) fn borrowed_owner() -> &'static RuntimeOpaqueTypeOwner {
        static OWNER: std::sync::LazyLock<RuntimeOpaqueTypeOwner> =
            std::sync::LazyLock::new(|| RuntimeDialogueOpaqueRole::Content.exact_owner());
        &OWNER
    }
}

impl RuntimeDialogueFormattedOutcome {
    fn borrowed_field(&self, index: usize) -> Option<RuntimeValueView<'_>> {
        use RuntimeScalarView as Scalar;
        use RuntimeValueView as View;
        let byte = |value| View::Scalar(Scalar::UInt(RuntimeUInt::U8(value)));
        Some(match (self, index) {
            (
                Self::Success {
                    value: RuntimeDialogueFormattedSuccess::Text(_),
                    ..
                },
                0,
            ) => byte(0),
            (
                Self::Success {
                    value: RuntimeDialogueFormattedSuccess::Content(_),
                    ..
                },
                0,
            ) => byte(1),
            (Self::Failure { .. }, 0) => byte(2),
            (
                Self::Success {
                    value: RuntimeDialogueFormattedSuccess::Text(value),
                    ..
                },
                1,
            ) => View::Scalar(Scalar::String(value)),
            (
                Self::Success {
                    value: RuntimeDialogueFormattedSuccess::Content(value),
                    ..
                },
                1,
            ) => value.view(),
            (Self::Failure { reason, .. }, 1) => View::Scalar(Scalar::String(reason)),
            (Self::Success { color, .. }, 2) => {
                RuntimeDialogueTupleView::OptionalColor(*color).view()
            }
            (Self::Failure { value_plain, .. }, 2) => {
                RuntimeDialogueTupleView::OptionalString(value_plain.as_deref()).view()
            }
            _ => return None,
        })
    }
}
