//! Launch-selected dialogue presentation policy.

use arcweft_view::{ViewId, ViewStyleSheetId};
use serde::{Deserialize, Serialize};

use crate::InlineFailurePolicy;

/// Immutable launch-level defaults applied before Character and line overrides.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialoguePresentationProfile {
    view: ViewId,
    style: Option<ViewStyleSheetId>,
    inline_failure: InlineFailurePolicy,
}

impl DialoguePresentationProfile {
    /// Engine fallback when a launch profile omits every dialogue field.
    ///
    /// # Panics
    ///
    /// Panics if the compile-time reserved `std.view.dialogue` identity stops
    /// satisfying the engine-owned `PublicId` contract.
    pub fn engine_default() -> Self {
        Self::new(
            ViewId::standard_dialogue(),
            None,
            InlineFailurePolicy::FailLine,
        )
    }

    pub const fn new(
        view: ViewId,
        style: Option<ViewStyleSheetId>,
        inline_failure: InlineFailurePolicy,
    ) -> Self {
        Self {
            view,
            style,
            inline_failure,
        }
    }

    pub const fn view(&self) -> &ViewId {
        &self.view
    }

    pub const fn style(&self) -> Option<&ViewStyleSheetId> {
        self.style.as_ref()
    }

    pub const fn inline_failure(&self) -> &InlineFailurePolicy {
        &self.inline_failure
    }
}

impl Default for DialoguePresentationProfile {
    fn default() -> Self {
        Self::engine_default()
    }
}

#[cfg(test)]
mod tests {
    use super::DialoguePresentationProfile;
    use crate::InlineFailurePolicy;

    #[test]
    fn engine_default_has_one_typed_view_and_no_base_style() {
        let profile = DialoguePresentationProfile::engine_default();

        assert_eq!(profile.view().as_str(), "std.view.dialogue");
        assert_eq!(profile.style(), None);
        assert_eq!(profile.inline_failure(), &InlineFailurePolicy::FailLine);
    }
}
