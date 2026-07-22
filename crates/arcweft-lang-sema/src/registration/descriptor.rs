use std::collections::BTreeMap;

use arcweft_character::{id::CharacterId, manifest::CharacterManifest};
use arcweft_lang_hir::symbol::{
    ExternalDeclarationId, ProjectSymbolTable, ProjectSymbolTargetId, ResolvedProjectSymbol,
};
use arcweft_lang_syntax::ast::{module_path::CanonicalModulePath, symbol_path::SymbolPath};
use arcweft_source::SourceSpan;

use super::model::{
    AcceptedNominalWorld, CharacterInventoryDescriptorV1, CharacterInventoryDigest,
    CharacterInventoryIntegrityError, ExternalOwnerLookupError, RegisteredCharacterResolutionError,
    RegisteredExternalOwner, RegisteredExternalOwnerKind, RegisteredTypeCheckEnv,
};

#[allow(
    clippy::result_large_err,
    reason = "integrity errors retain complete typed world, revision, owner, and declaration evidence"
)]
pub(crate) fn build_descriptor(
    symbols: &ProjectSymbolTable,
    characters: &BTreeMap<CharacterId, CharacterManifest>,
    owners: &BTreeMap<ExternalDeclarationId, RegisteredExternalOwner>,
) -> Result<CharacterInventoryDescriptorV1, CharacterInventoryIntegrityError> {
    let characters = characters
        .iter()
        .map(|(owner, manifest)| (owner.clone(), manifest.semantic_fingerprint_v1()))
        .collect::<Vec<_>>();
    let mut externals = Vec::new();
    for (declaration, owner) in owners {
        let RegisteredExternalOwner::Character(owner) = owner else {
            continue;
        };
        let symbol = symbols.external(*declaration).ok_or(
            CharacterInventoryIntegrityError::MissingExternalSymbol {
                declaration: *declaration,
            },
        )?;
        if !characters.iter().any(|(candidate, _)| candidate == owner) {
            return Err(CharacterInventoryIntegrityError::OwnerMismatch {
                declaration: *declaration,
                expected: owner.clone(),
                actual: owner.clone(),
            });
        }
        externals.push((*declaration, symbol.canonical_path().clone(), owner.clone()));
    }
    externals.sort_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    Ok(CharacterInventoryDescriptorV1 {
        characters,
        externals,
    })
}

pub(crate) fn descriptor_digest(
    descriptor: &CharacterInventoryDescriptorV1,
) -> CharacterInventoryDigest {
    let mut encoder = DescriptorEncoder::new();
    encode_descriptor(&mut encoder, descriptor);
    CharacterInventoryDigest(*encoder.finish().as_bytes())
}

fn encode_descriptor(encoder: &mut DescriptorEncoder, descriptor: &CharacterInventoryDescriptorV1) {
    encoder.list_len(descriptor.characters.len());
    for (owner, fingerprint) in &descriptor.characters {
        encoder.string(owner.as_str());
        encoder.bytes(fingerprint.as_bytes());
    }
    encoder.list_len(descriptor.externals.len());
    for (_, path, owner) in &descriptor.externals {
        encoder.string(&path.canonical_string());
        encoder.string(owner.as_str());
    }
}

#[cfg(test)]
pub(super) fn descriptor_canonical_len(descriptor: &CharacterInventoryDescriptorV1) -> usize {
    let mut encoder = DescriptorEncoder::new();
    encode_descriptor(&mut encoder, descriptor);
    encoder.encoded_len
}

impl AcceptedNominalWorld {
    #[allow(
        clippy::result_large_err,
        reason = "owner lookup errors retain both complete typed world identities and revisions"
    )]
    pub fn external_owner(
        &self,
        symbols: &ProjectSymbolTable,
        declaration: ExternalDeclarationId,
        expected: RegisteredExternalOwnerKind,
    ) -> Result<&RegisteredExternalOwner, ExternalOwnerLookupError> {
        if symbols.world() != self.world() || symbols.revision() != self.symbol_revision() {
            return Err(ExternalOwnerLookupError::Stale {
                expected_world: self.world().clone(),
                actual_world: symbols.world().clone(),
                expected_revision: *self.symbol_revision(),
                actual_revision: *symbols.revision(),
            });
        }
        let owner = self
            .external_owners()
            .get(&declaration)
            .ok_or(ExternalOwnerLookupError::Unknown { declaration })?;
        let actual = owner.kind();
        if actual != expected {
            return Err(ExternalOwnerLookupError::WrongKind {
                declaration,
                expected,
                actual,
            });
        }
        Ok(owner)
    }
}

impl RegisteredTypeCheckEnv {
    #[allow(
        clippy::result_large_err,
        reason = "owner lookup errors retain both complete typed world identities and revisions"
    )]
    pub fn external_owner(
        &self,
        symbols: &ProjectSymbolTable,
        declaration: ExternalDeclarationId,
        expected: RegisteredExternalOwnerKind,
    ) -> Result<&RegisteredExternalOwner, ExternalOwnerLookupError> {
        self.nominal_world
            .external_owner(symbols, declaration, expected)
    }

    #[allow(
        clippy::result_large_err,
        reason = "resolution errors retain typed symbol, owner, world, revision, and source evidence"
    )]
    pub fn resolve_character_owner(
        &self,
        symbols: &ProjectSymbolTable,
        module: &CanonicalModulePath,
        reference: &SymbolPath,
        source: &SourceSpan,
    ) -> Result<CharacterId, RegisteredCharacterResolutionError> {
        let declaration = match symbols.resolve(module, reference, source)? {
            ResolvedProjectSymbol::External(symbol) => symbol.declaration(),
            ResolvedProjectSymbol::Callable(symbol) => {
                return Err(RegisteredCharacterResolutionError::NotExternal {
                    actual: ProjectSymbolTargetId::Callable(symbol.declaration().clone()),
                });
            }
            ResolvedProjectSymbol::Nominal(symbol) => {
                return Err(RegisteredCharacterResolutionError::NotExternal {
                    actual: ProjectSymbolTargetId::Nominal(symbol.id().clone()),
                });
            }
            ResolvedProjectSymbol::Module(module) => {
                return Err(RegisteredCharacterResolutionError::NotExternal {
                    actual: ProjectSymbolTargetId::Module(module.clone()),
                });
            }
        };
        match self
            .external_owner(symbols, declaration, RegisteredExternalOwnerKind::Character)
            .map_err(RegisteredCharacterResolutionError::Owner)?
        {
            RegisteredExternalOwner::Character(owner) => Ok(owner.clone()),
            RegisteredExternalOwner::Environment(_) => {
                unreachable!("typed external-owner lookup enforces the requested kind")
            }
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "integrity errors retain complete typed world, revision, owner, and declaration evidence"
    )]
    pub fn verify_character_inventory(
        &self,
        symbols: &ProjectSymbolTable,
    ) -> Result<(), CharacterInventoryIntegrityError> {
        if symbols.world() != self.world() || symbols.revision() != self.symbol_revision() {
            return Err(CharacterInventoryIntegrityError::Stale {
                expected_world: self.world().clone(),
                actual_world: symbols.world().clone(),
                expected_revision: *self.symbol_revision(),
                actual_revision: *symbols.revision(),
            });
        }
        let descriptor = build_descriptor(
            symbols,
            &self.characters,
            self.nominal_world.external_owners(),
        )?;
        for (declaration, _, expected) in &descriptor.externals {
            let actual = self
                .nominal_world
                .external_owners()
                .get(declaration)
                .ok_or(CharacterInventoryIntegrityError::MissingExternalSymbol {
                    declaration: *declaration,
                })?;
            match actual {
                RegisteredExternalOwner::Character(actual) if actual == expected => {}
                RegisteredExternalOwner::Character(actual) => {
                    return Err(CharacterInventoryIntegrityError::OwnerMismatch {
                        declaration: *declaration,
                        expected: expected.clone(),
                        actual: actual.clone(),
                    });
                }
                RegisteredExternalOwner::Environment(_) => {
                    return Err(CharacterInventoryIntegrityError::WrongOwnerKind {
                        declaration: *declaration,
                        actual: RegisteredExternalOwnerKind::Environment,
                    });
                }
            }
        }
        let actual = descriptor_digest(&descriptor);
        if descriptor != self.character_descriptor || actual != self.character_digest {
            return Err(CharacterInventoryIntegrityError::DescriptorTamper {
                expected: self.character_digest,
                actual,
            });
        }
        Ok(())
    }
}

struct DescriptorEncoder {
    hasher: blake3::Hasher,
    #[cfg(test)]
    encoded_len: usize,
}

impl DescriptorEncoder {
    fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft-character-inventory-descriptor-v1\0");
        hasher.update(&1_u32.to_le_bytes());
        Self {
            hasher,
            #[cfg(test)]
            encoded_len: b"arcweft-character-inventory-descriptor-v1\0".len() + 4,
        }
    }

    fn finish(self) -> blake3::Hash {
        self.hasher.finalize()
    }

    fn list_len(&mut self, value: usize) {
        let value = u32::try_from(value).expect("validated descriptor count fits u32");
        self.update(&value.to_le_bytes());
    }

    fn string(&mut self, value: &str) {
        self.list_len(value.len());
        self.update(value.as_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.update(value);
    }

    fn update(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
        #[cfg(test)]
        {
            self.encoded_len = self
                .encoded_len
                .checked_add(bytes.len())
                .expect("validated descriptor canonical length fits in usize");
        }
    }
}
