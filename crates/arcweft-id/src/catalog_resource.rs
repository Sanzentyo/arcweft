use super::{AssetId, AssetIdError, AssetVirtualPath, DeclarationIdentityFamily, PublicId};

/// Public identity of a recorded voice resource in an authored asset catalog.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VoiceResourceId(PublicId);

/// Closed source and runtime identity domain for loadable catalog resources.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CatalogResourceId {
    Asset(AssetId),
    Voice(VoiceResourceId),
}

impl VoiceResourceId {
    pub const VIRTUAL_PATH_PREFIX: &'static str = "audio/voice/";
    const BUNDLE_ASSET_PREFIX: &'static str = "asset.";

    /// Derives a recording's identity from its asset-space path. The caller
    /// selects the file's supported audio format separately.
    pub fn from_virtual_path(path: &AssetVirtualPath) -> Result<Option<Self>, AssetIdError> {
        path.as_str()
            .strip_prefix(Self::VIRTUAL_PATH_PREFIX)
            .map(|local_path| {
                derive_catalog_public_id(path, local_path, DeclarationIdentityFamily::Voice)
                    .map(Self)
            })
            .transpose()
    }

    pub const fn as_public_id(&self) -> &PublicId {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Projects a recorded voice to the physical asset identity used by a
    /// bundle request, audio graph entry, and retained audio handle.
    pub fn bundle_asset_id(&self) -> AssetId {
        AssetId(PublicId(format!(
            "{}{}",
            Self::BUNDLE_ASSET_PREFIX,
            self.as_str()
        )))
    }

    /// Tests the recording's physical asset identity using borrowed IDs.
    pub fn matches_bundle_asset(voice: &PublicId, asset: &AssetId) -> bool {
        DeclarationIdentityFamily::Voice.accepts_public_id_text(voice.as_str())
            && asset.as_str().strip_prefix(Self::BUNDLE_ASSET_PREFIX) == Some(voice.as_str())
    }
}

impl TryFrom<PublicId> for VoiceResourceId {
    type Error = super::PublicIdFamilyError;

    fn try_from(id: PublicId) -> Result<Self, Self::Error> {
        DeclarationIdentityFamily::Voice.validate_public_id(&id)?;
        Ok(Self(id))
    }
}

impl CatalogResourceId {
    /// Checks a borrowed ID before a retained value copies its text.
    pub fn family_for_text(text: &str) -> Option<DeclarationIdentityFamily> {
        let prefix = text.split_once('.')?.0;
        let family = DeclarationIdentityFamily::from_prefix(prefix)?;
        match family {
            DeclarationIdentityFamily::Asset | DeclarationIdentityFamily::Voice
                if family.accepts_public_id_text(text) =>
            {
                Some(family)
            }
            _ => None,
        }
    }

    pub const fn family(&self) -> DeclarationIdentityFamily {
        match self {
            Self::Asset(_) => DeclarationIdentityFamily::Asset,
            Self::Voice(_) => DeclarationIdentityFamily::Voice,
        }
    }

    pub fn as_public_id(&self) -> &PublicId {
        match self {
            Self::Asset(id) => id.as_public_id(),
            Self::Voice(id) => id.as_public_id(),
        }
    }

    pub fn as_str(&self) -> &str {
        self.as_public_id().as_str()
    }
}

impl TryFrom<PublicId> for CatalogResourceId {
    type Error = super::PublicIdFamilyError;

    fn try_from(id: PublicId) -> Result<Self, Self::Error> {
        match Self::family_for_text(id.as_str()) {
            Some(DeclarationIdentityFamily::Asset) => Ok(Self::Asset(AssetId(id))),
            Some(DeclarationIdentityFamily::Voice) => Ok(Self::Voice(VoiceResourceId(id))),
            _ => Err(super::PublicIdFamilyError::WrongFamily {
                expected: "asset or voice",
                id,
            }),
        }
    }
}

pub(super) fn derive_catalog_public_id(
    path: &AssetVirtualPath,
    local_path: &str,
    family: DeclarationIdentityFamily,
) -> Result<PublicId, AssetIdError> {
    let without_extension = local_path
        .rsplit_once('.')
        .map_or(local_path, |(stem, _)| stem);
    if without_extension.is_empty() {
        return Err(AssetIdError::NoEffectiveComponents { path: path.clone() });
    }

    let components = without_extension
        .split('/')
        .map(|component| {
            component
                .chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() {
                        Some(character.to_ascii_lowercase())
                    } else if matches!(character, '_' | '-') {
                        Some('_')
                    } else {
                        None
                    }
                })
                .collect::<Option<String>>()
                .filter(|normalized| !normalized.is_empty())
                .ok_or_else(|| AssetIdError::InvalidComponent {
                    component: component.to_owned(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PublicId::try_new(format!(
        "{}.{}",
        family.prefix(),
        components.join(".")
    ))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_identity_uses_the_recording_root_and_catalog_component_rules() {
        let voice = AssetVirtualPath::try_new("audio/voice/Alice/Opening-01.wav").unwrap();
        let id = VoiceResourceId::from_virtual_path(&voice).unwrap().unwrap();
        assert_eq!(id.as_str(), "voice.alice.opening_01");
        let asset = id.bundle_asset_id();
        assert_eq!(asset.as_str(), "asset.voice.alice.opening_01");
        assert!(VoiceResourceId::matches_bundle_asset(
            id.as_public_id(),
            &asset
        ));
        assert_eq!(
            CatalogResourceId::family_for_text(id.as_str()),
            Some(DeclarationIdentityFamily::Voice)
        );

        let effect = AssetVirtualPath::try_new("audio/se/Alice/Opening-01.wav").unwrap();
        assert_eq!(VoiceResourceId::from_virtual_path(&effect), Ok(None));
        let invalid = AssetVirtualPath::try_new("audio/voice/Alice/Opening 01.wav").unwrap();
        assert!(matches!(
            VoiceResourceId::from_virtual_path(&invalid),
            Err(AssetIdError::InvalidComponent { .. })
        ));
    }
}
