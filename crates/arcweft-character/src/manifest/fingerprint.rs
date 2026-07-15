use super::{
    CharacterLook, CharacterManifest, CharacterPart, CharacterPartSelection, CharacterVariant,
};

/// Domain-separated digest of the canonical semantic manifest fields.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterManifestFingerprint([u8; 32]);

impl CharacterManifestFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl CharacterManifest {
    pub fn semantic_fingerprint_v1(&self) -> CharacterManifestFingerprint {
        let mut encoder = ManifestFingerprintEncoder::new();
        encode_manifest(&mut encoder, self);
        CharacterManifestFingerprint(*encoder.finish().as_bytes())
    }
}

fn encode_manifest(encoder: &mut ManifestFingerprintEncoder, manifest: &CharacterManifest) {
    encoder.string(manifest.character.as_str());
    encoder.u32(manifest.canvas.width);
    encoder.u32(manifest.canvas.height);
    encoder.i32(manifest.anchor.x);
    encoder.i32(manifest.anchor.y);
    encoder.string(manifest.default_look.as_str());

    let mut parts = manifest.parts.iter().collect::<Vec<_>>();
    parts.sort_by_key(|part| &part.id);
    encoder.list_len(parts.len());
    for part in parts {
        encode_part(encoder, part);
    }

    let mut looks = manifest.looks.iter().collect::<Vec<_>>();
    looks.sort_by_key(|look| &look.id);
    encoder.list_len(looks.len());
    for look in looks {
        encode_look(encoder, look);
    }
}

#[cfg(test)]
pub(super) fn canonical_len_v1(manifest: &CharacterManifest) -> usize {
    let mut encoder = ManifestFingerprintEncoder::new();
    encode_manifest(&mut encoder, manifest);
    encoder.encoded_len
}

fn encode_part(encoder: &mut ManifestFingerprintEncoder, part: &CharacterPart) {
    encoder.string(part.id.as_str());
    encoder.i32(part.z);
    let mut variants = part.variants.iter().collect::<Vec<_>>();
    variants.sort_by_key(|variant| &variant.id);
    encoder.list_len(variants.len());
    for variant in variants {
        encode_variant(encoder, variant);
    }
}

fn encode_variant(encoder: &mut ManifestFingerprintEncoder, variant: &CharacterVariant) {
    encoder.string(variant.id.as_str());
    encoder.string(variant.asset.as_str());
    encoder.i32(variant.rect.x);
    encoder.i32(variant.rect.y);
    encoder.u32(variant.rect.width);
    encoder.u32(variant.rect.height);
    encoder.u8(variant.opacity);
    encoder.u32(variant.blend.stable_code());
    encoder.bool(variant.clipping);
}

fn encode_look(encoder: &mut ManifestFingerprintEncoder, look: &CharacterLook) {
    encoder.string(look.id.as_str());
    let mut selections = look.select.iter().collect::<Vec<_>>();
    selections.sort_by_key(|selection| &selection.part);
    encoder.list_len(selections.len());
    for selection in selections {
        encode_selection(encoder, selection);
    }
}

fn encode_selection(encoder: &mut ManifestFingerprintEncoder, selection: &CharacterPartSelection) {
    encoder.string(selection.part.as_str());
    encoder.string(selection.variant.as_str());
}

struct ManifestFingerprintEncoder {
    hasher: blake3::Hasher,
    #[cfg(test)]
    encoded_len: usize,
}

impl ManifestFingerprintEncoder {
    fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft-character-manifest-fingerprint-v1\0");
        hasher.update(&1_u32.to_le_bytes());
        Self {
            hasher,
            #[cfg(test)]
            encoded_len: b"arcweft-character-manifest-fingerprint-v1\0".len() + size_of::<u32>(),
        }
    }

    fn finish(self) -> blake3::Hash {
        self.hasher.finalize()
    }

    fn u8(&mut self, value: u8) {
        self.update(&[value]);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.update(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.update(&value.to_le_bytes());
    }

    fn list_len(&mut self, value: usize) {
        self.u32(u32::try_from(value).expect("validated manifest list length fits in u32"));
    }

    fn string(&mut self, value: &str) {
        self.list_len(value.len());
        self.update(value.as_bytes());
    }

    fn update(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
        #[cfg(test)]
        {
            self.encoded_len = self
                .encoded_len
                .checked_add(bytes.len())
                .expect("validated manifest canonical length fits in usize");
        }
    }
}
