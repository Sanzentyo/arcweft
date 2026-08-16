//! Canonical identity of a fully accepted semantic environment.

use arcweft_lang_syntax::ast::module_path::ModulePathRoot;
use arcweft_source::SourceSpan;

use crate::callable::{EnvironmentCallableOwner, ProjectCallablePath, StandardEnvironmentId};

use super::{
    AcceptedNominalSource, AcceptedNominalVisibilityIndex, AcceptedNominalWorld,
    CharacterInventoryDigest, CharacterInventoryRevision, EnvironmentPublicationItemId,
    ProjectRegistrationFacts, RegisteredEnvironmentDigest,
};

const ENVIRONMENT_DOMAIN: &[u8] = b"arcweft.registered-semantic-environment.v1\0";
const VISIBILITY_DOMAIN: &[u8] = b"arcweft.accepted-nominal-visibility.v1\0";
#[cfg(test)]
const TEST_CALLABLE_REPLACEMENT_DOMAIN: &[u8] =
    b"arcweft.registered-semantic-environment.test-callable-replacement.v1\0";

pub(super) fn derive(
    world: &AcceptedNominalWorld,
    rust_metadata_digest: &[u8; 32],
    callable_catalog_digest: &[u8; 32],
    character_dialogue_fields_digest: &[u8; 32],
    facts: &ProjectRegistrationFacts,
    character_digest: CharacterInventoryDigest,
    character_revision: CharacterInventoryRevision,
) -> RegisteredEnvironmentDigest {
    let mut encoder = Encoder::new(ENVIRONMENT_DOMAIN);
    encoder.string(world.world().package().as_str());
    encoder.string(world.world().root_document().as_str());
    encoder.string(world.world().profile());
    encoder.bytes(world.symbol_revision().as_source_set().as_bytes());
    encoder.bytes(world.nominal_catalog().digest().as_bytes());
    encoder.bytes(&visibility_digest(world.visibility()));
    encoder.bytes(rust_metadata_digest);
    encoder.bytes(callable_catalog_digest);
    encoder.bytes(character_dialogue_fields_digest);

    let mut manifests = facts
        .environment_inputs()
        .map(|input| (input.input().owner(), input.input().manifest_digest()))
        .collect::<Vec<_>>();
    manifests.sort_by(|(left_owner, left_digest), (right_owner, right_digest)| {
        left_owner
            .cmp(right_owner)
            .then_with(|| left_digest.cmp(right_digest))
    });
    encoder.len(manifests.len());
    for (owner, digest) in manifests {
        encoder.environment_owner(owner);
        encoder.bytes(digest.as_bytes());
    }

    encoder.bytes(character_digest.as_bytes());
    encoder.u64(character_revision.get());
    RegisteredEnvironmentDigest::from_bytes(encoder.finish())
}

#[cfg(test)]
pub(super) fn derive_test_callable_replacement(
    previous: RegisteredEnvironmentDigest,
    callable_catalog_digest: &[u8; 32],
) -> RegisteredEnvironmentDigest {
    let mut encoder = Encoder::new(TEST_CALLABLE_REPLACEMENT_DOMAIN);
    encoder.bytes(previous.as_bytes());
    encoder.bytes(callable_catalog_digest);
    RegisteredEnvironmentDigest::from_bytes(encoder.finish())
}

fn visibility_digest(index: &AcceptedNominalVisibilityIndex) -> [u8; 32] {
    let mut encoder = Encoder::new(VISIBILITY_DOMAIN);
    encoder.len(index.visible_entries().len());
    for (id, source) in index.visible_entries() {
        encoder.byte(0);
        encoder.accepted_nominal_id(id);
        encoder.nominal_source(source);
    }
    encoder.len(index.inaccessible_entries().len());
    for (id, source) in index.inaccessible_entries() {
        encoder.byte(1);
        encoder.accepted_nominal_id(id);
        encoder.nominal_source(source);
    }
    encoder.finish()
}

struct Encoder(blake3::Hasher);

impl Encoder {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        Self(hasher)
    }

    fn finish(self) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }

    fn byte(&mut self, value: u8) {
        self.0.update(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.0.update(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.update(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(&value.to_le_bytes());
    }

    fn len(&mut self, value: usize) {
        self.u32(
            u32::try_from(value)
                .expect("accepted environment sequences fit the checked u32 contract"),
        );
    }

    fn bytes(&mut self, value: &[u8]) {
        self.len(value.len());
        self.0.update(value);
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn accepted_nominal_id(&mut self, id: &crate::env::nominal::AcceptedNominalId) {
        let digest = crate::types::accepted_nominal_semantic_identity_digest(id, &[]);
        self.bytes(digest.as_bytes());
    }

    fn nominal_source(&mut self, source: &AcceptedNominalSource) {
        self.source(source.declaration());
        self.publication_item(source.item());
    }

    fn source(&mut self, source: &SourceSpan) {
        self.string(source.source().id().as_str());
        self.bytes(source.source().revision().as_bytes());
        self.u64(source.source().source_len());
        self.u64(u64::try_from(source.range().start()).expect("source offset fits u64"));
        self.u64(u64::try_from(source.range().end()).expect("source offset fits u64"));
    }

    fn publication_item(&mut self, item: &EnvironmentPublicationItemId) {
        match item {
            EnvironmentPublicationItemId::AdapterSymbol { owner, path } => {
                self.u16(0);
                self.environment_owner(owner);
                self.project_symbol_path(path);
            }
            EnvironmentPublicationItemId::AdapterNominal { owner, path } => {
                self.u16(1);
                self.environment_owner(owner);
                self.project_symbol_path(path.path());
            }
            EnvironmentPublicationItemId::AdapterHostCall { owner, path } => {
                self.u16(6);
                self.environment_owner(owner);
                self.callable_path_raw(path);
            }
            EnvironmentPublicationItemId::AdapterFunction {
                owner,
                path,
                overload,
            } => {
                self.u16(2);
                self.environment_owner(owner);
                self.callable_path(path);
                self.u64(u64::try_from(overload.get()).expect("overload fits u64"));
            }
            EnvironmentPublicationItemId::AdapterMethod {
                owner,
                receiver,
                method,
                overload,
                declaration_order,
            } => {
                self.u16(3);
                self.environment_owner(owner);
                self.bytes(receiver.as_bytes());
                self.string(method.as_str());
                self.u64(u64::try_from(overload.get()).expect("overload fits u64"));
                self.u64(
                    u64::try_from(declaration_order.get()).expect("declaration order fits u64"),
                );
            }
            EnvironmentPublicationItemId::RustType {
                adapter,
                package,
                rust_item,
                accepted_path,
            } => {
                self.u16(4);
                self.string(adapter.as_str());
                self.string(package.as_str());
                self.string(rust_item.as_str());
                self.project_symbol_path(accepted_path.path());
            }
            EnvironmentPublicationItemId::RustFunction {
                adapter,
                package,
                rust_item,
                callable_path,
                overload,
            } => {
                self.u16(5);
                self.string(adapter.as_str());
                self.string(package.as_str());
                self.string(rust_item.as_str());
                self.callable_path(callable_path);
                self.u64(u64::try_from(overload.get()).expect("overload fits u64"));
            }
        }
    }

    fn environment_owner(&mut self, owner: &EnvironmentCallableOwner) {
        match owner {
            EnvironmentCallableOwner::Standard(owner) => {
                self.byte(0);
                self.byte(match owner {
                    StandardEnvironmentId::Core => 0,
                    StandardEnvironmentId::SansIo => 1,
                    StandardEnvironmentId::NativeHttp => 2,
                    StandardEnvironmentId::InferenceTensor => 3,
                    StandardEnvironmentId::SystemInfo => 4,
                    StandardEnvironmentId::NativeFile => 5,
                    StandardEnvironmentId::Math => 6,
                });
            }
            EnvironmentCallableOwner::Adapter(owner) => {
                self.byte(1);
                self.string(owner.as_str());
            }
        }
    }

    fn callable_path(&mut self, path: &ProjectCallablePath) {
        self.string(path.package().as_str());
        self.len(path.module().segments().len());
        for segment in path.module().segments() {
            self.string(segment.as_str());
        }
        self.callable_path_raw(path.path());
    }

    fn callable_path_raw(&mut self, path: &crate::callable::CallablePath) {
        self.len(path.segments().len());
        for segment in path.segments() {
            self.string(segment.as_str());
        }
    }

    fn project_symbol_path(
        &mut self,
        path: &arcweft_lang_syntax::ast::symbol_path::ProjectSymbolPath,
    ) {
        match path.root() {
            ModulePathRoot::ImplicitCrate => self.byte(0),
            ModulePathRoot::Crate => self.byte(1),
            ModulePathRoot::SelfModule => self.byte(2),
            ModulePathRoot::Super(depth) => {
                self.byte(3);
                self.u64(u64::try_from(depth).expect("module depth fits u64"));
            }
        }
        self.len(path.segments().len());
        for segment in path.segments() {
            self.string(segment.as_str());
        }
    }
}
