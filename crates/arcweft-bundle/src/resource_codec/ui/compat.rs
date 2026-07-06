use crate::container::BundleSectionKind;
use crate::patch::PatchCompatibility;
use crate::resource_codec::error::SectionCodecError;
use crate::resource_codec::kind::ProductSectionCodecKind;
use serde::{Deserialize, Serialize};

use super::model::{
    UiInputResource, UiStyleResource, UiTextResource, UiThemeResource, ViewProgramResource,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiResourceCompatibility {
    ContentOnly,
    CodeCompatible,
    CodeGenerational,
    RestartRequired,
}

impl UiResourceCompatibility {
    pub const fn patch_compatibility(self) -> PatchCompatibility {
        match self {
            Self::ContentOnly => PatchCompatibility::ContentOnly,
            Self::CodeCompatible => PatchCompatibility::CodeCompatible,
            Self::CodeGenerational => PatchCompatibility::CodeGenerational,
            Self::RestartRequired => PatchCompatibility::RestartRequired,
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::ContentOnly => 0,
            Self::CodeCompatible => 1,
            Self::CodeGenerational => 2,
            Self::RestartRequired => 3,
        }
    }

    pub(crate) fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }
}

/// Semantic patch compatibility for migrated UI compact sections.
pub fn migrated_ui_section_compatibility(
    kind: BundleSectionKind,
    old_bytes: &[u8],
    new_bytes: &[u8],
) -> Result<Option<PatchCompatibility>, SectionCodecError> {
    let Some(codec) = ProductSectionCodecKind::from_section_kind(kind) else {
        return Ok(None);
    };
    match codec {
        ProductSectionCodecKind::UiProgram => {
            let old = ViewProgramResource::decode_canonical_section(old_bytes)?;
            let new = ViewProgramResource::decode_canonical_section(new_bytes)?;
            Ok(Some(old.compatibility_with(&new).patch_compatibility()))
        }
        ProductSectionCodecKind::UiStyle => {
            let old = UiStyleResource::decode_canonical_section(old_bytes)?;
            let new = UiStyleResource::decode_canonical_section(new_bytes)?;
            Ok(Some(old.compatibility_with(&new).patch_compatibility()))
        }
        ProductSectionCodecKind::UiText => {
            let old = UiTextResource::decode_canonical_section(old_bytes)?;
            let new = UiTextResource::decode_canonical_section(new_bytes)?;
            Ok(Some(old.compatibility_with(&new).patch_compatibility()))
        }
        ProductSectionCodecKind::UiInput => {
            let old = UiInputResource::decode_canonical_section(old_bytes)?;
            let new = UiInputResource::decode_canonical_section(new_bytes)?;
            Ok(Some(old.compatibility_with(&new).patch_compatibility()))
        }
        ProductSectionCodecKind::UiTheme => {
            let old = UiThemeResource::decode_canonical_section(old_bytes)?;
            let new = UiThemeResource::decode_canonical_section(new_bytes)?;
            Ok(Some(old.compatibility_with(&new).patch_compatibility()))
        }
        _ => Ok(None),
    }
}
