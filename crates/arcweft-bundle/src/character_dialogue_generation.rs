//! Bounded version-one AWFB framing for one Dialogue generation and its
//! referenced Character package metadata.

use crate::{
    ArcweftBundle, BundleCodecError, BundleVirtualFileSpace,
    character_package::BundleCharacterPackage,
};
use arcweft_character::package::{
    CHARACTER_PACKAGE_MANIFEST_PATH, CharacterLayerPayload, CharacterPackage,
};
use arcweft_core::awbc::schema::AwbcRuntimeTypeShape;
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::value::RuntimeCharacterDialogueProducerId;
use arcweft_dialogue::{CharacterDialogueGenerationDeclaration, CharacterDialogueVisualType};
use std::sync::Arc;

const MAGIC: &[u8; 8] = b"AWDG\r\n\x1a\n";
const VERSION: u32 = 1;
const MAX_SECTION_BYTES: usize = 64 * 1024 * 1024;
const MAX_PACKAGES: usize = 4_096;

pub(crate) type RuntimeGeneration = CharacterDialogueGenerationDeclaration<RuntimeSemanticTypeId>;

/// Checks the bundled bytes against the typed package metadata and every
/// logical Character's accepted visual declaration. Package metadata alone is
/// insufficient because an AWFB may carry mismatched manifest or PNG bytes.
pub(crate) fn validate_binding(bundle: &ArcweftBundle) -> Result<(), BundleCodecError> {
    let producer = RuntimeCharacterDialogueProducerId::get();
    let program = bundle.product_awbc_program();
    if program.runtime_types.iter().any(|ty| {
        let AwbcRuntimeTypeShape::Opaque { producer: name, .. } = ty.shape() else {
            return false;
        };
        program
            .strings
            .get(name.index())
            .is_some_and(|name| name == producer.as_str())
    }) && bundle.character_dialogue_generation.is_none()
    {
        return Err(invalid_generation(
            "executable CharacterDialogue type has no generation declaration",
        ));
    }
    for metadata in &bundle.character_packages {
        let id = &metadata.character;
        let suffix = format!("/{CHARACTER_PACKAGE_MANIFEST_PATH}");
        let root = metadata
            .manifest
            .path
            .strip_suffix(&suffix)
            .ok_or_else(|| invalid_package(id, "manifest path is not a Character package root"))?;
        if root.is_empty() || metadata.manifest.space != BundleVirtualFileSpace::Asset {
            return Err(invalid_package(
                id,
                "manifest is not under an asset package root",
            ));
        }
        let manifest = bundle
            .virtual_files
            .iter()
            .find(|file| {
                file.space == metadata.manifest.space && file.path == metadata.manifest.path
            })
            .ok_or_else(|| invalid_package(id, "manifest file is missing"))?;
        let mut layers = Vec::with_capacity(metadata.layers.len());
        for layer in &metadata.layers {
            if layer.file.space != BundleVirtualFileSpace::Asset
                || layer.file.path != format!("{root}/{}", layer.asset_path.as_str())
            {
                return Err(invalid_package(
                    id,
                    "layer reference differs from its package path",
                ));
            }
            let file = bundle
                .virtual_files
                .iter()
                .find(|file| file.space == layer.file.space && file.path == layer.file.path)
                .ok_or_else(|| invalid_package(id, "layer file is missing"))?;
            layers.push(CharacterLayerPayload::new(
                layer.asset_path.clone(),
                Arc::<[u8]>::from(file.bytes.as_slice()),
            ));
        }
        let package = CharacterPackage::from_manifest_bytes(
            Arc::<[u8]>::from(manifest.bytes.as_slice()),
            layers,
        )
        .map_err(|error| invalid_package(id, error.to_string()))?;
        let (rebuilt, _) = BundleCharacterPackage::from_character_package(&package, root)?;
        if rebuilt != *metadata || package.manifest().character().as_str() != id {
            return Err(invalid_package(
                id,
                "metadata differs from the validated package",
            ));
        }
        if let Some(generation) = &bundle.character_dialogue_generation {
            let row = generation
                .characters()
                .iter()
                .find(|(character, _)| character.as_str() == id)
                .map(|(_, row)| row)
                .ok_or_else(|| invalid_generation("package has no logical Character row"))?;
            match row.visual() {
                CharacterDialogueVisualType::Present { manifest, .. }
                    if manifest.as_bytes()
                        == package.manifest().semantic_fingerprint_v1().as_bytes() => {}
                CharacterDialogueVisualType::Present { .. } => {
                    return Err(invalid_generation(
                        "package manifest fingerprint differs from declaration",
                    ));
                }
                CharacterDialogueVisualType::Absent => {
                    return Err(invalid_generation(
                        "package exists for a visually absent Character",
                    ));
                }
            }
        }
    }
    if let Some(generation) = &bundle.character_dialogue_generation {
        for (character, row) in generation.characters() {
            if matches!(row.visual(), CharacterDialogueVisualType::Present { .. })
                && !bundle
                    .character_packages
                    .iter()
                    .any(|package| package.character == character.as_str())
            {
                return Err(invalid_generation(format!(
                    "visual Character `{character}` has no bundled package"
                )));
            }
        }
    }
    Ok(())
}

fn invalid_package(character_id: &str, message: impl Into<String>) -> BundleCodecError {
    BundleCodecError::InvalidCharacterPackage {
        character_id: character_id.to_owned(),
        message: message.into(),
    }
}

fn invalid_generation(message: impl Into<String>) -> BundleCodecError {
    BundleCodecError::InvalidCharacterDialogueGeneration {
        message: message.into(),
    }
}

pub(crate) fn encode_section(
    generation: Option<&RuntimeGeneration>,
    packages: &[BundleCharacterPackage],
) -> Result<Vec<u8>, BundleCodecError> {
    if packages.len() > MAX_PACKAGES {
        return Err(encode_error("too many Character package rows"));
    }
    let declaration = generation
        .map(|generation| {
            generation
                .encode_canonical_bytes()
                .map_err(|error| encode_error(error.to_string()))
        })
        .transpose()?;
    let mut ordered = packages.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.character.cmp(&right.character));
    if ordered
        .windows(2)
        .any(|window| window[0].character == window[1].character)
    {
        return Err(encode_error("duplicate Character package row"));
    }
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(ordered.len())
            .map_err(|_| encode_error("Character package count exceeds u32"))?
            .to_le_bytes(),
    );
    append_blob(&mut out, declaration.as_deref().unwrap_or_default())?;
    for package in ordered {
        let bytes = serde_json::to_vec(package).map_err(|error| encode_error(error.to_string()))?;
        append_blob(&mut out, &bytes)?;
    }
    Ok(out)
}

pub(crate) fn decode_section(
    bytes: &[u8],
) -> Result<(Option<RuntimeGeneration>, Vec<BundleCharacterPackage>), BundleCodecError> {
    if bytes.len() > MAX_SECTION_BYTES {
        return Err(decode_error(
            "CharacterDialogue generation section exceeds 64 MiB",
        ));
    }
    let mut cursor = Cursor::new(bytes);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(decode_error(
            "CharacterDialogue generation magic does not match",
        ));
    }
    if cursor.u32()? != VERSION {
        return Err(decode_error(
            "unsupported CharacterDialogue generation section version",
        ));
    }
    let count = usize::try_from(cursor.u32()?)
        .map_err(|_| decode_error("Character package count overflow"))?;
    if count > MAX_PACKAGES {
        return Err(decode_error("too many Character package rows"));
    }
    let declaration = cursor.blob()?;
    let generation = (!declaration.is_empty())
        .then(|| RuntimeGeneration::decode_canonical_bytes(declaration))
        .transpose()
        .map_err(|error| decode_error(error.to_string()))?;
    let mut packages = Vec::with_capacity(count);
    let mut previous = None::<String>;
    for _ in 0..count {
        let encoded = cursor.blob()?;
        let package: BundleCharacterPackage =
            serde_json::from_slice(encoded).map_err(|error| decode_error(error.to_string()))?;
        let canonical =
            serde_json::to_vec(&package).map_err(|error| decode_error(error.to_string()))?;
        if canonical != encoded {
            return Err(decode_error("Character package metadata is not canonical"));
        }
        if previous
            .as_ref()
            .is_some_and(|previous| previous >= &package.character)
        {
            return Err(decode_error(
                "Character package rows are not strictly ordered",
            ));
        }
        previous = Some(package.character.clone());
        packages.push(package);
    }
    if !cursor.is_empty() {
        return Err(decode_error(
            "CharacterDialogue generation section has trailing bytes",
        ));
    }
    Ok((generation, packages))
}

fn append_blob(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), BundleCodecError> {
    let required = out
        .len()
        .checked_add(8)
        .and_then(|value| value.checked_add(bytes.len()))
        .ok_or_else(|| encode_error("CharacterDialogue generation length overflow"))?;
    if required > MAX_SECTION_BYTES {
        return Err(encode_error(
            "CharacterDialogue generation section exceeds 64 MiB",
        ));
    }
    out.extend_from_slice(
        &u64::try_from(bytes.len())
            .map_err(|_| encode_error("CharacterDialogue row length exceeds u64"))?
            .to_le_bytes(),
    );
    out.extend_from_slice(bytes);
    Ok(())
}

struct Cursor<'a> {
    remaining: &'a [u8],
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], BundleCodecError> {
        let (head, tail) = self
            .remaining
            .split_at_checked(len)
            .ok_or_else(|| decode_error("truncated CharacterDialogue generation section"))?;
        self.remaining = tail;
        Ok(head)
    }

    fn u32(&mut self) -> Result<u32, BundleCodecError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("four bytes")))
    }

    fn blob(&mut self) -> Result<&'a [u8], BundleCodecError> {
        let bytes = self.take(8)?;
        let len = u64::from_le_bytes(bytes.try_into().expect("eight bytes"));
        let len = usize::try_from(len)
            .map_err(|_| decode_error("CharacterDialogue row length overflow"))?;
        self.take(len)
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
}

fn encode_error(message: impl Into<String>) -> BundleCodecError {
    BundleCodecError::EncodeAwfb {
        message: message.into(),
    }
}

fn decode_error(message: impl Into<String>) -> BundleCodecError {
    BundleCodecError::DecodeAwfb {
        message: message.into(),
    }
}
