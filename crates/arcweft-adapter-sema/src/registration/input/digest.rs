//! Canonical typed digests for adapter registration inputs.

use arcweft_lang_sema::registration::{EnvironmentManifestDigest, EnvironmentTypeInputDigest};
use arcweft_rust_abi::{
    ArcweftRustField, ArcweftRustPurity, ArcweftRustStructShape, ArcweftRustTypeKind,
    ArcweftRustTypeRef, ArcweftRustVariant, ArcweftRustVariantPayload,
};

use arcweft_adapter_context::manifest::{
    AdapterCallablePath, AdapterFreeCallableKind, AdapterFunctionSignature, AdapterManifest,
    AdapterNominalPath, AdapterParameterPassing, AdapterParameterPresence, AdapterToolingSubject,
    AdapterTypeKind,
};

const MANIFEST_DOMAIN: &[u8] = b"arcweft.environment-manifest.v1\0";
const TYPE_DOMAIN: &[u8] = b"arcweft.environment-type-input.v1\0";

pub(super) fn manifest_digest(manifest: &AdapterManifest) -> EnvironmentManifestDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MANIFEST_DOMAIN);
    hash_manifest_identity(&mut hasher, manifest);
    hash_manifest_mounts(&mut hasher, manifest);
    hash_manifest_nominals(&mut hasher, manifest);
    hash_manifest_rust_metadata(&mut hasher, manifest);
    hash_manifest_symbols(&mut hasher, manifest);
    hash_manifest_methods(&mut hasher, manifest);
    hash_manifest_functions(&mut hasher, manifest);
    hash_manifest_rust_functions(&mut hasher, manifest);
    hash_manifest_effects_and_host_calls(&mut hasher, manifest);
    hash_manifest_tooling(&mut hasher, manifest);

    EnvironmentManifestDigest::from_bytes(*hasher.finalize().as_bytes())
}

fn hash_manifest_identity(hasher: &mut blake3::Hasher, manifest: &AdapterManifest) {
    section(hasher, 0);
    hash_str(hasher, manifest.id().as_str());
    hash_str(hasher, &format!("adapter:{}", manifest.id().as_str()));
    hash_str(hasher, manifest.display_name());
}

fn hash_manifest_mounts(hasher: &mut blake3::Hasher, manifest: &AdapterManifest) {
    section(hasher, 1);
    hash_len(hasher, manifest.rust_package_mounts().iter().len());
    for (package, prefix) in manifest.rust_package_mounts().iter() {
        hash_str(hasher, package.as_str());
        hash_segments(
            hasher,
            prefix
                .segments()
                .iter()
                .map(arcweft_adapter_context::manifest::AdapterNominalPathSegment::as_str),
        );
    }
}

fn hash_manifest_nominals(hasher: &mut blake3::Hasher, manifest: &AdapterManifest) {
    section(hasher, 2);
    let mut nominals = manifest.nominal_declarations().iter().collect::<Vec<_>>();
    nominals.sort_by(|left, right| left.path().segments().cmp(right.path().segments()));
    hash_len(hasher, nominals.len());
    for declaration in nominals {
        hash_nominal_path(hasher, declaration.path());
        hash_u16(hasher, declaration.arity());
        hash_str(hasher, declaration.opaque_producer().as_str());
        hash_u8(
            hasher,
            match declaration.visibility() {
                arcweft_adapter_context::manifest::AdapterNominalVisibility::Public => 0,
                arcweft_adapter_context::manifest::AdapterNominalVisibility::Private => 1,
            },
        );
        hash_str(hasher, declaration.source_label());
    }
}

fn hash_manifest_rust_metadata(hasher: &mut blake3::Hasher, manifest: &AdapterManifest) {
    section(hasher, 3);
    hash_len(hasher, manifest.rust_packages().len());
    for (package_id, package) in manifest.rust_packages() {
        hash_str(hasher, package_id.as_str());
        hash_str(hasher, &package.version);
        hash_option_str(hasher, package.metadata_hash.as_deref());
    }

    let mut rust_types = manifest.rust_types().iter().collect::<Vec<_>>();
    rust_types.sort_by(|left, right| {
        left.package().id.cmp(&right.package().id).then_with(|| {
            left.decl()
                .path
                .segments()
                .cmp(right.decl().path.segments())
        })
    });
    hash_len(hasher, rust_types.len());
    for rust_type in rust_types {
        hash_rust_package(hasher, rust_type.package());
        hash_nominal_path(hasher, rust_type.accepted_path());
        hash_str(hasher, rust_type.opaque_producer().as_str());
        hash_rust_path(hasher, &rust_type.decl().path);
        hash_str(hasher, &rust_type.decl().rust_path);
        hash_len(hasher, rust_type.decl().parameters.len());
        for parameter in &rust_type.decl().parameters {
            hash_len(hasher, parameter.index.get());
            hash_str(hasher, parameter.name.as_str());
        }
        hash_rust_type_kind(hasher, &rust_type.decl().kind);
    }
}

fn hash_manifest_symbols(hasher: &mut blake3::Hasher, manifest: &AdapterManifest) {
    section(hasher, 4);
    let mut symbols = manifest.symbols().iter().collect::<Vec<_>>();
    symbols.sort_by(|left, right| left.path().segments().cmp(right.path().segments()));
    hash_len(hasher, symbols.len());
    for symbol in symbols {
        hash_segments(
            hasher,
            symbol
                .path()
                .segments()
                .iter()
                .map(arcweft_adapter_context::manifest::AdapterSymbolSegment::as_str),
        );
        hash_adapter_type(hasher, symbol.ty());
    }
}

fn hash_manifest_methods(hasher: &mut blake3::Hasher, manifest: &AdapterManifest) {
    section(hasher, 5);
    let mut methods = manifest.methods().iter().collect::<Vec<_>>();
    methods.sort_by(|left, right| {
        type_digest_bytes(left.receiver())
            .cmp(&type_digest_bytes(right.receiver()))
            .then_with(|| left.callable_name().cmp(right.callable_name()))
            .then_with(|| left.overload().cmp(&right.overload()))
    });
    hash_len(hasher, methods.len());
    for method in methods {
        hash_adapter_type(hasher, method.receiver());
        hash_str(hasher, method.callable_name().as_str());
        hash_len(hasher, method.overload().get());
        hash_signature(hasher, method.signature());
        hash_effects(hasher, method.effects());
    }
}

fn hash_manifest_functions(hasher: &mut blake3::Hasher, manifest: &AdapterManifest) {
    section(hasher, 6);
    let mut functions = manifest.functions().iter().collect::<Vec<_>>();
    functions.sort_by(|left, right| {
        left.path()
            .segments()
            .cmp(right.path().segments())
            .then_with(|| left.overload().cmp(&right.overload()))
    });
    hash_len(hasher, functions.len());
    for function in functions {
        hash_callable_path(hasher, function.path());
        hash_len(hasher, function.overload().get());
        hash_signature(hasher, function.signature());
        hash_effects(hasher, function.effects());
    }
}

fn hash_manifest_rust_functions(hasher: &mut blake3::Hasher, manifest: &AdapterManifest) {
    section(hasher, 7);
    let mut rust_functions = manifest.rust_functions().iter().collect::<Vec<_>>();
    rust_functions.sort_by(|left, right| {
        left.package()
            .id
            .cmp(&right.package().id)
            .then_with(|| left.rust_path().cmp(right.rust_path()))
            .then_with(|| left.path().segments().cmp(right.path().segments()))
            .then_with(|| left.overload().cmp(&right.overload()))
    });
    hash_len(hasher, rust_functions.len());
    for function in rust_functions {
        hash_rust_package(hasher, function.package());
        hash_str(hasher, function.rust_path());
        hash_callable_path(hasher, function.path());
        hash_len(hasher, function.overload().get());
        hash_signature(hasher, function.signature());
        hash_u8(hasher, rust_purity_tag(function.purity()));
        hash_effects(hasher, function.effects());
    }
}

fn hash_manifest_effects_and_host_calls(hasher: &mut blake3::Hasher, manifest: &AdapterManifest) {
    section(hasher, 8);
    hash_effects(hasher, manifest.effects());
    let mut host_calls = manifest.host_calls().iter().collect::<Vec<_>>();
    host_calls.sort_by_key(|call| call.id());
    hash_len(hasher, host_calls.len());
    for call in host_calls {
        hash_str(hasher, call.id());
        hash_signature(hasher, call.signature());
        hash_effects(hasher, call.effects());
    }
}

fn hash_manifest_tooling(hasher: &mut blake3::Hasher, manifest: &AdapterManifest) {
    section(hasher, 9);
    let mut tooling_docs = manifest.tooling_docs().iter().collect::<Vec<_>>();
    tooling_docs.sort_by_key(|doc| tooling_subject_key(doc.subject()));
    hash_len(hasher, tooling_docs.len());
    for doc in tooling_docs {
        hash_tooling_subject(hasher, doc.subject());
        hash_option_str(hasher, doc.summary());
        hash_option_str(hasher, doc.details());
        hash_len(hasher, doc.parameters().len());
        for parameter in doc.parameters() {
            hash_len(hasher, parameter.group().get());
            hash_len(hasher, parameter.parameter().get());
            hash_str(hasher, parameter.text());
        }
    }
}

pub(super) fn type_digest(ty: &AdapterTypeKind) -> EnvironmentTypeInputDigest {
    EnvironmentTypeInputDigest::from_bytes(type_digest_bytes(ty))
}

fn type_digest_bytes(ty: &AdapterTypeKind) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(TYPE_DOMAIN);
    hash_adapter_type(&mut hasher, ty);
    *hasher.finalize().as_bytes()
}

fn hash_signature(hasher: &mut blake3::Hasher, signature: &AdapterFunctionSignature) {
    hash_len(hasher, signature.groups().len());
    for group in signature.groups() {
        hash_len(hasher, group.index().get());
        hash_len(hasher, group.parameters().len());
        for parameter in group.parameters() {
            hash_len(hasher, parameter.index().get());
            hash_option_str(
                hasher,
                parameter
                    .name()
                    .map(arcweft_adapter_context::manifest::AdapterCallableName::as_str),
            );
            hash_u8(hasher, passing_tag(parameter.passing()));
            hash_u8(hasher, presence_tag(parameter.presence()));
            hash_adapter_type(hasher, parameter.ty());
        }
    }
    hash_adapter_type(hasher, signature.return_type());
}

fn hash_effects(
    hasher: &mut blake3::Hasher,
    effects: &[arcweft_adapter_context::manifest::AdapterEffectCapability],
) {
    let mut effects = effects
        .iter()
        .map(arcweft_adapter_context::manifest::AdapterEffectCapability::as_str)
        .collect::<Vec<_>>();
    effects.sort_unstable();
    hash_len(hasher, effects.len());
    for effect in effects {
        hash_str(hasher, effect);
    }
}

fn hash_tooling_subject(hasher: &mut blake3::Hasher, subject: &AdapterToolingSubject) {
    match subject {
        AdapterToolingSubject::Free {
            kind,
            path,
            overload,
        } => {
            hash_u8(hasher, 0);
            hash_u8(
                hasher,
                match kind {
                    AdapterFreeCallableKind::Function => 0,
                    AdapterFreeCallableKind::RustFunction => 1,
                },
            );
            hash_callable_path(hasher, path);
            hash_len(hasher, overload.get());
        }
        AdapterToolingSubject::Method {
            receiver,
            name,
            overload,
        } => {
            hash_u8(hasher, 1);
            hash_adapter_type(hasher, receiver);
            hash_str(hasher, name.as_str());
            hash_len(hasher, overload.get());
        }
    }
}

fn tooling_subject_key(subject: &AdapterToolingSubject) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.environment-tooling-subject.v1\0");
    hash_tooling_subject(&mut hasher, subject);
    *hasher.finalize().as_bytes()
}

fn hash_rust_package(hasher: &mut blake3::Hasher, package: &arcweft_rust_abi::ArcweftRustPackage) {
    hash_str(hasher, package.id.as_str());
    hash_str(hasher, &package.version);
    hash_option_str(hasher, package.metadata_hash.as_deref());
}

fn hash_rust_type_kind(hasher: &mut blake3::Hasher, kind: &ArcweftRustTypeKind) {
    match kind {
        ArcweftRustTypeKind::Struct { shape } => {
            hash_u8(hasher, 0);
            hash_rust_struct_shape(hasher, shape);
        }
        ArcweftRustTypeKind::Enum { variants } => {
            hash_u8(hasher, 1);
            hash_len(hasher, variants.len());
            for variant in variants {
                hash_rust_variant(hasher, variant);
            }
        }
        ArcweftRustTypeKind::Newtype { inner } => {
            hash_u8(hasher, 2);
            hash_rust_type_ref(hasher, inner);
        }
    }
}

fn hash_rust_struct_shape(hasher: &mut blake3::Hasher, shape: &ArcweftRustStructShape) {
    match shape {
        ArcweftRustStructShape::Unit => hash_u8(hasher, 0),
        ArcweftRustStructShape::Tuple { fields } => {
            hash_u8(hasher, 1);
            hash_rust_type_refs(hasher, fields);
        }
        ArcweftRustStructShape::Record { fields } => {
            hash_u8(hasher, 2);
            hash_rust_fields(hasher, fields);
        }
    }
}

fn hash_rust_variant(hasher: &mut blake3::Hasher, variant: &ArcweftRustVariant) {
    hash_str(hasher, &variant.name);
    match &variant.payload {
        ArcweftRustVariantPayload::Unit => hash_u8(hasher, 0),
        ArcweftRustVariantPayload::Tuple { fields } => {
            hash_u8(hasher, 1);
            hash_rust_type_refs(hasher, fields);
        }
        ArcweftRustVariantPayload::Record { fields } => {
            hash_u8(hasher, 2);
            hash_rust_fields(hasher, fields);
        }
    }
}

fn hash_rust_fields(hasher: &mut blake3::Hasher, fields: &[ArcweftRustField]) {
    hash_len(hasher, fields.len());
    for field in fields {
        hash_str(hasher, &field.name);
        hash_rust_type_ref(hasher, &field.ty);
    }
}

fn hash_rust_type_refs(hasher: &mut blake3::Hasher, types: &[ArcweftRustTypeRef]) {
    hash_len(hasher, types.len());
    for ty in types {
        hash_rust_type_ref(hasher, ty);
    }
}

fn hash_rust_type_ref(hasher: &mut blake3::Hasher, ty: &ArcweftRustTypeRef) {
    let tag = match ty {
        ArcweftRustTypeRef::Unit => 0,
        ArcweftRustTypeRef::Bool => 1,
        ArcweftRustTypeRef::I8 => 2,
        ArcweftRustTypeRef::I16 => 3,
        ArcweftRustTypeRef::I32 => 4,
        ArcweftRustTypeRef::I64 => 5,
        ArcweftRustTypeRef::I128 => 6,
        ArcweftRustTypeRef::ISize => 7,
        ArcweftRustTypeRef::U8 => 8,
        ArcweftRustTypeRef::U16 => 9,
        ArcweftRustTypeRef::U32 => 10,
        ArcweftRustTypeRef::U64 => 11,
        ArcweftRustTypeRef::U128 => 12,
        ArcweftRustTypeRef::USize => 13,
        ArcweftRustTypeRef::F32 => 14,
        ArcweftRustTypeRef::F64 => 15,
        ArcweftRustTypeRef::String => 16,
        ArcweftRustTypeRef::Char => 17,
        ArcweftRustTypeRef::Vec { .. } => 18,
        ArcweftRustTypeRef::Seq { .. } => 19,
        ArcweftRustTypeRef::Option { .. } => 20,
        ArcweftRustTypeRef::Result { .. } => 21,
        ArcweftRustTypeRef::Tuple { .. } => 22,
        ArcweftRustTypeRef::Nominal { .. } => 23,
        ArcweftRustTypeRef::TypeParameter { .. } => 24,
    };
    hash_u16(hasher, tag);
    match ty {
        ArcweftRustTypeRef::Vec { item }
        | ArcweftRustTypeRef::Seq { item }
        | ArcweftRustTypeRef::Option { item } => hash_rust_type_ref(hasher, item),
        ArcweftRustTypeRef::Result { ok, error } => {
            hash_rust_type_ref(hasher, ok);
            hash_rust_type_ref(hasher, error);
        }
        ArcweftRustTypeRef::Tuple { items } => hash_rust_type_refs(hasher, items),
        ArcweftRustTypeRef::Nominal {
            package,
            path,
            arguments,
        } => {
            hash_str(hasher, package.as_str());
            hash_rust_path(hasher, path);
            hash_rust_type_refs(hasher, arguments);
        }
        ArcweftRustTypeRef::TypeParameter { index } => hash_len(hasher, index.get()),
        ArcweftRustTypeRef::Unit
        | ArcweftRustTypeRef::Bool
        | ArcweftRustTypeRef::I8
        | ArcweftRustTypeRef::I16
        | ArcweftRustTypeRef::I32
        | ArcweftRustTypeRef::I64
        | ArcweftRustTypeRef::I128
        | ArcweftRustTypeRef::ISize
        | ArcweftRustTypeRef::U8
        | ArcweftRustTypeRef::U16
        | ArcweftRustTypeRef::U32
        | ArcweftRustTypeRef::U64
        | ArcweftRustTypeRef::U128
        | ArcweftRustTypeRef::USize
        | ArcweftRustTypeRef::F32
        | ArcweftRustTypeRef::F64
        | ArcweftRustTypeRef::String
        | ArcweftRustTypeRef::Char => {}
    }
}

fn hash_rust_path(hasher: &mut blake3::Hasher, path: &arcweft_rust_abi::ArcweftRustTypePath) {
    hash_segments(
        hasher,
        path.segments()
            .iter()
            .map(arcweft_rust_abi::ArcweftRustTypePathSegment::as_str),
    );
}

fn hash_adapter_type(hasher: &mut blake3::Hasher, ty: &AdapterTypeKind) {
    let tag = match ty {
        AdapterTypeKind::Unit => 0,
        AdapterTypeKind::Bool => 1,
        AdapterTypeKind::I8 => 2,
        AdapterTypeKind::I16 => 3,
        AdapterTypeKind::I32 => 4,
        AdapterTypeKind::I64 => 5,
        AdapterTypeKind::I128 => 6,
        AdapterTypeKind::ISize => 7,
        AdapterTypeKind::U8 => 8,
        AdapterTypeKind::U16 => 9,
        AdapterTypeKind::U32 => 10,
        AdapterTypeKind::U64 => 11,
        AdapterTypeKind::U128 => 12,
        AdapterTypeKind::USize => 13,
        AdapterTypeKind::F32 => 14,
        AdapterTypeKind::F64 => 15,
        AdapterTypeKind::String => 16,
        AdapterTypeKind::Char => 17,
        AdapterTypeKind::Vec { .. } => 18,
        AdapterTypeKind::Seq { .. } => 19,
        AdapterTypeKind::Option { .. } => 20,
        AdapterTypeKind::Result { .. } => 21,
        AdapterTypeKind::Tuple { .. } => 22,
        AdapterTypeKind::Need { .. } => 23,
        AdapterTypeKind::Nominal { .. } => 24,
    };
    hash_u16(hasher, tag);
    match ty {
        AdapterTypeKind::Vec { item }
        | AdapterTypeKind::Seq { item }
        | AdapterTypeKind::Option { item } => hash_adapter_type(hasher, item),
        AdapterTypeKind::Result { ok, error } => {
            hash_adapter_type(hasher, ok);
            hash_adapter_type(hasher, error);
        }
        AdapterTypeKind::Tuple { items } => {
            hash_len(hasher, items.len());
            items
                .iter()
                .for_each(|item| hash_adapter_type(hasher, item));
        }
        AdapterTypeKind::Need { ready, error } => {
            hash_adapter_type(hasher, ready);
            hash_adapter_type(hasher, error);
        }
        AdapterTypeKind::Nominal { nominal } => {
            match nominal.owner() {
                arcweft_adapter_context::manifest::AdapterNominalOwner::Environment { owner } => {
                    hash_u8(hasher, 0);
                    hash_str(hasher, owner.as_str());
                }
                arcweft_adapter_context::manifest::AdapterNominalOwner::RustPackage { package } => {
                    hash_u8(hasher, 1);
                    hash_str(hasher, package.as_str());
                }
            }
            hash_nominal_path(hasher, nominal.path());
            hash_len(hasher, nominal.arguments().len());
            nominal
                .arguments()
                .iter()
                .for_each(|argument| hash_adapter_type(hasher, argument));
        }
        AdapterTypeKind::Unit
        | AdapterTypeKind::Bool
        | AdapterTypeKind::I8
        | AdapterTypeKind::I16
        | AdapterTypeKind::I32
        | AdapterTypeKind::I64
        | AdapterTypeKind::I128
        | AdapterTypeKind::ISize
        | AdapterTypeKind::U8
        | AdapterTypeKind::U16
        | AdapterTypeKind::U32
        | AdapterTypeKind::U64
        | AdapterTypeKind::U128
        | AdapterTypeKind::USize
        | AdapterTypeKind::F32
        | AdapterTypeKind::F64
        | AdapterTypeKind::String
        | AdapterTypeKind::Char => {}
    }
}

fn hash_nominal_path(hasher: &mut blake3::Hasher, path: &AdapterNominalPath) {
    hash_segments(
        hasher,
        path.segments()
            .iter()
            .map(arcweft_adapter_context::manifest::AdapterNominalPathSegment::as_str),
    );
}

fn hash_callable_path(hasher: &mut blake3::Hasher, path: &AdapterCallablePath) {
    hash_segments(
        hasher,
        path.segments()
            .iter()
            .map(arcweft_adapter_context::manifest::AdapterCallableName::as_str),
    );
}

fn hash_segments<'a>(
    hasher: &mut blake3::Hasher,
    segments: impl ExactSizeIterator<Item = &'a str>,
) {
    hash_len(hasher, segments.len());
    for segment in segments {
        hash_str(hasher, segment);
    }
}

fn rust_purity_tag(purity: ArcweftRustPurity) -> u8 {
    match purity {
        ArcweftRustPurity::External => 0,
        ArcweftRustPurity::Pure => 1,
        ArcweftRustPurity::Task => 2,
    }
}

fn passing_tag(passing: AdapterParameterPassing) -> u8 {
    match passing {
        AdapterParameterPassing::PositionalOrNamed => 0,
        AdapterParameterPassing::PositionalOnly => 1,
        AdapterParameterPassing::NamedOnly => 2,
        AdapterParameterPassing::RestPositional => 3,
        AdapterParameterPassing::RestNamed => 4,
    }
}

fn presence_tag(presence: AdapterParameterPresence) -> u8 {
    match presence {
        AdapterParameterPresence::Required => 0,
        AdapterParameterPresence::Defaulted => 1,
    }
}

fn section(hasher: &mut blake3::Hasher, tag: u8) {
    hash_u8(hasher, 0xff);
    hash_u8(hasher, tag);
}

fn hash_option_str(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hash_u8(hasher, 1);
            hash_str(hasher, value);
        }
        None => hash_u8(hasher, 0),
    }
}

fn hash_u8(hasher: &mut blake3::Hasher, value: u8) {
    hasher.update(&[value]);
}

fn hash_u16(hasher: &mut blake3::Hasher, value: u16) {
    hasher.update(&value.to_le_bytes());
}

fn hash_len(hasher: &mut blake3::Hasher, value: usize) {
    let value = u32::try_from(value)
        .expect("validated adapter manifest sequences fit the checked u32 digest contract");
    hasher.update(&value.to_le_bytes());
}

fn hash_str(hasher: &mut blake3::Hasher, value: &str) {
    hash_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::manifest::AdapterEffectCapability;

    #[test]
    fn manifest_digest_is_independent_of_set_insertion_order() {
        let left = AdapterManifest::new("host", "Host")
            .with_effect(AdapterEffectCapability::new("io.write"))
            .with_effect(AdapterEffectCapability::new("io.read"));
        let right = AdapterManifest::new("host", "Host")
            .with_effect(AdapterEffectCapability::new("io.read"))
            .with_effect(AdapterEffectCapability::new("io.write"));

        assert_eq!(manifest_digest(&left), manifest_digest(&right));
    }

    #[test]
    fn manifest_digest_changes_with_identity_bearing_content() {
        let left = AdapterManifest::new("host", "Host");
        let right = AdapterManifest::new("other", "Host");

        assert_ne!(manifest_digest(&left), manifest_digest(&right));
    }
}
