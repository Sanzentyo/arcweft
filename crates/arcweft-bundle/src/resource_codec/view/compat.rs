use crate::container::BundleSectionKind;
use crate::patch::PatchCompatibility;
use crate::resource_codec::error::SectionCodecError;
use crate::resource_codec::kind::ProductSectionCodecKind;
use serde::{Deserialize, Serialize};

use super::model::{
    ViewInputResource, ViewProgramResource, ViewStyleResource, ViewTextResource, ViewThemeResource,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewResourceCompatibility {
    ContentOnly,
    CodeCompatible,
    CodeGenerational,
    RestartRequired,
}

impl ViewResourceCompatibility {
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

/// Semantic patch compatibility for migrated View compact sections.
pub fn migrated_view_section_compatibility(
    kind: BundleSectionKind,
    old_bytes: &[u8],
    new_bytes: &[u8],
) -> Result<Option<PatchCompatibility>, SectionCodecError> {
    let Some(codec) = ProductSectionCodecKind::from_section_kind(kind) else {
        return Ok(None);
    };
    match codec {
        ProductSectionCodecKind::ViewProgram => {
            let old = ViewProgramResource::decode_canonical_section(old_bytes)?;
            let new = ViewProgramResource::decode_canonical_section(new_bytes)?;
            Ok(Some(old.compatibility_with(&new).patch_compatibility()))
        }
        ProductSectionCodecKind::ViewStyle => {
            let old = ViewStyleResource::decode_canonical_section(old_bytes)?;
            let new = ViewStyleResource::decode_canonical_section(new_bytes)?;
            Ok(Some(old.compatibility_with(&new).patch_compatibility()))
        }
        ProductSectionCodecKind::ViewText => {
            let old = ViewTextResource::decode_canonical_section(old_bytes)?;
            let new = ViewTextResource::decode_canonical_section(new_bytes)?;
            Ok(Some(old.compatibility_with(&new).patch_compatibility()))
        }
        ProductSectionCodecKind::ViewInput => {
            let old = ViewInputResource::decode_canonical_section(old_bytes)?;
            let new = ViewInputResource::decode_canonical_section(new_bytes)?;
            Ok(Some(old.compatibility_with(&new).patch_compatibility()))
        }
        ProductSectionCodecKind::ViewTheme => {
            let old = ViewThemeResource::decode_canonical_section(old_bytes)?;
            let new = ViewThemeResource::decode_canonical_section(new_bytes)?;
            Ok(Some(old.compatibility_with(&new).patch_compatibility()))
        }
        _ => Ok(None),
    }
}
