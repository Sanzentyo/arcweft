use arcweft_lang_hir::symbol::CallablePackageId;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::SourceDocument;

use crate::{
    callable::{
        CallableLookupKey, CallableParameter, CallableParameterGroup, CallableParameterType,
        EnvironmentCallableOwner, EnvironmentCallablePublicationRecord, ProjectCallablePath,
    },
    registration::{
        EnvironmentCallableLookupInput, EnvironmentCallablePublicationMetadataInput,
        EnvironmentCallablePublicationRecordInput, EnvironmentCallableSignatureInput,
        EnvironmentManifestDigest, EnvironmentParameterGroupInput, EnvironmentParameterInput,
        EnvironmentParameterMetadataInput, EnvironmentParameterTypeInput,
        EnvironmentPublicationItemId, EnvironmentTypeInputDigest, EnvironmentTypeProjectionKind,
        EnvironmentTypeProjectionNode, SourceBackedEnvironmentRegistrationInput,
    },
    types::TypeKind,
};

pub(crate) fn source_backed_callable_input(
    owner: EnvironmentCallableOwner,
    source: &SourceDocument,
    records: impl IntoIterator<Item = EnvironmentCallablePublicationRecord>,
) -> SourceBackedEnvironmentRegistrationInput {
    let span = source
        .span(arcweft_source::SourceRange::new(0, source.text().len()))
        .expect("test source has a complete span");
    let package = CallablePackageId::try_new(match &owner {
        EnvironmentCallableOwner::Adapter(owner) => owner.as_str(),
        EnvironmentCallableOwner::Standard(_) => "standard-test-environment",
    })
    .expect("test environment owner is a valid callable package");
    let records = records
        .into_iter()
        .map(|record| callable_record_input(&owner, &package, &span, &record))
        .collect::<Vec<_>>();

    SourceBackedEnvironmentRegistrationInput::new(
        owner,
        source.identity().clone(),
        EnvironmentManifestDigest::from_bytes(*blake3::hash(source.text().as_bytes()).as_bytes()),
        [],
        [],
        [],
        records,
    )
}

fn callable_record_input(
    owner: &EnvironmentCallableOwner,
    package: &CallablePackageId,
    span: &arcweft_source::SourceSpan,
    record: &EnvironmentCallablePublicationRecord,
) -> EnvironmentCallablePublicationRecordInput {
    let key = match record.key() {
        CallableLookupKey::Free(path) => {
            EnvironmentCallableLookupInput::Free(ProjectCallablePath::new(
                package.clone(),
                CanonicalModulePath::crate_root(),
                path.clone(),
            ))
        }
        CallableLookupKey::Method(method) => EnvironmentCallableLookupInput::Method {
            receiver: projection(method.receiver(), span),
            method: method.method().clone(),
        },
    };
    let item = publication_item(owner, &key, record);
    let groups = record
        .schema()
        .groups()
        .iter()
        .map(|group| parameter_group_input(group, span))
        .collect::<Vec<_>>();
    let schema = EnvironmentCallableSignatureInput::new(
        groups,
        projection(record.schema().result(), span),
        record.schema().effects().declared().clone(),
        record.schema().argument_policy(),
        record.schema().validator().clone(),
    );
    EnvironmentCallablePublicationRecordInput::new(
        item,
        record.kind(),
        key,
        record.overload(),
        schema,
        record.declaration_order(),
        EnvironmentCallablePublicationMetadataInput::new(
            record.documentation().clone(),
            record.source().cloned(),
            record.rust().cloned(),
        ),
    )
}

fn publication_item(
    owner: &EnvironmentCallableOwner,
    key: &EnvironmentCallableLookupInput,
    record: &EnvironmentCallablePublicationRecord,
) -> EnvironmentPublicationItemId {
    match key {
        EnvironmentCallableLookupInput::Free(path) => {
            EnvironmentPublicationItemId::AdapterFunction {
                owner: owner.clone(),
                path: path.clone(),
                overload: record.overload(),
            }
        }
        EnvironmentCallableLookupInput::Method { receiver, method } => {
            EnvironmentPublicationItemId::AdapterMethod {
                owner: owner.clone(),
                receiver: EnvironmentTypeInputDigest::from_bytes(
                    *method_receiver_digest(receiver).as_bytes(),
                ),
                method: method.clone(),
                overload: record.overload(),
                declaration_order: record.declaration_order(),
            }
        }
    }
}

fn parameter_group_input(
    group: &CallableParameterGroup,
    span: &arcweft_source::SourceSpan,
) -> EnvironmentParameterGroupInput {
    EnvironmentParameterGroupInput::new(
        group.index(),
        group.kind(),
        group
            .parameters()
            .iter()
            .map(|parameter| parameter_input(parameter, span))
            .collect::<Vec<_>>(),
    )
}

fn parameter_input(
    parameter: &CallableParameter,
    span: &arcweft_source::SourceSpan,
) -> EnvironmentParameterInput {
    let ty = match parameter.ty() {
        CallableParameterType::Exact(ty) => {
            EnvironmentParameterTypeInput::Exact(projection(ty, span))
        }
        CallableParameterType::Unchecked => EnvironmentParameterTypeInput::Unchecked {
            source: span.clone(),
        },
    };
    EnvironmentParameterInput::new(
        parameter.index(),
        parameter.name().cloned(),
        ty,
        parameter.passing(),
        parameter.presence(),
        EnvironmentParameterMetadataInput::new(
            parameter.documentation().map(Into::into),
            parameter.source().cloned(),
        ),
    )
}

fn method_receiver_digest(
    receiver: &EnvironmentTypeProjectionNode,
) -> crate::types::SemanticTypeDigest {
    match receiver.kind() {
        EnvironmentTypeProjectionKind::Unit => TypeKind::Unit.semantic_identity_digest(),
        EnvironmentTypeProjectionKind::Bool => TypeKind::Bool.semantic_identity_digest(),
        EnvironmentTypeProjectionKind::I32 => TypeKind::I32.semantic_identity_digest(),
        EnvironmentTypeProjectionKind::String => TypeKind::String.semantic_identity_digest(),
        EnvironmentTypeProjectionKind::AcceptedNominal { id, arguments } => {
            TypeKind::AcceptedNominal(crate::types::AcceptedNominalType::new(
                id.clone(),
                arguments.iter().map(projected_type).collect::<Vec<_>>(),
            ))
            .semantic_identity_digest()
        }
        _ => projected_type(receiver).semantic_identity_digest(),
    }
}

fn projected_type(node: &EnvironmentTypeProjectionNode) -> TypeKind {
    match node.kind() {
        EnvironmentTypeProjectionKind::Unit => TypeKind::Unit,
        EnvironmentTypeProjectionKind::Bool => TypeKind::Bool,
        EnvironmentTypeProjectionKind::I8 => TypeKind::I8,
        EnvironmentTypeProjectionKind::I16 => TypeKind::I16,
        EnvironmentTypeProjectionKind::I32 => TypeKind::I32,
        EnvironmentTypeProjectionKind::I64 => TypeKind::I64,
        EnvironmentTypeProjectionKind::I128 => TypeKind::I128,
        EnvironmentTypeProjectionKind::ISize => TypeKind::ISize,
        EnvironmentTypeProjectionKind::U8 => TypeKind::U8,
        EnvironmentTypeProjectionKind::U16 => TypeKind::U16,
        EnvironmentTypeProjectionKind::U32 => TypeKind::U32,
        EnvironmentTypeProjectionKind::U64 => TypeKind::U64,
        EnvironmentTypeProjectionKind::U128 => TypeKind::U128,
        EnvironmentTypeProjectionKind::USize => TypeKind::USize,
        EnvironmentTypeProjectionKind::F32 => TypeKind::F32,
        EnvironmentTypeProjectionKind::F64 => TypeKind::F64,
        EnvironmentTypeProjectionKind::String => TypeKind::String,
        EnvironmentTypeProjectionKind::Char => TypeKind::Char,
        EnvironmentTypeProjectionKind::Vec(item) => TypeKind::Vec(Box::new(projected_type(item))),
        EnvironmentTypeProjectionKind::Seq(item) => TypeKind::Seq(Box::new(projected_type(item))),
        EnvironmentTypeProjectionKind::Option(item) => {
            TypeKind::Option(Box::new(projected_type(item)))
        }
        EnvironmentTypeProjectionKind::Result { ok, error } => TypeKind::Result {
            ok: Box::new(projected_type(ok)),
            error: Box::new(projected_type(error)),
        },
        EnvironmentTypeProjectionKind::Tuple(items) => {
            TypeKind::Tuple(items.iter().map(projected_type).collect())
        }
        EnvironmentTypeProjectionKind::Need { ready, error } => TypeKind::Need {
            ready: Box::new(projected_type(ready)),
            error: Box::new(projected_type(error)),
        },
        EnvironmentTypeProjectionKind::CharacterNominal(nominal) => {
            TypeKind::CharacterNominal(nominal.clone())
        }
        EnvironmentTypeProjectionKind::AcceptedNominal { id, arguments } => {
            TypeKind::AcceptedNominal(crate::types::AcceptedNominalType::new(
                id.clone(),
                arguments.iter().map(projected_type).collect::<Vec<_>>(),
            ))
        }
        EnvironmentTypeProjectionKind::TypeParameter { .. } => {
            panic!("test callable publications cannot contain free type parameters")
        }
    }
}

fn projection(ty: &TypeKind, source: &arcweft_source::SourceSpan) -> EnvironmentTypeProjectionNode {
    let kind = match ty {
        TypeKind::Unit => EnvironmentTypeProjectionKind::Unit,
        TypeKind::Bool => EnvironmentTypeProjectionKind::Bool,
        TypeKind::I8 => EnvironmentTypeProjectionKind::I8,
        TypeKind::I16 => EnvironmentTypeProjectionKind::I16,
        TypeKind::I32 => EnvironmentTypeProjectionKind::I32,
        TypeKind::I64 => EnvironmentTypeProjectionKind::I64,
        TypeKind::I128 => EnvironmentTypeProjectionKind::I128,
        TypeKind::ISize => EnvironmentTypeProjectionKind::ISize,
        TypeKind::U8 => EnvironmentTypeProjectionKind::U8,
        TypeKind::U16 => EnvironmentTypeProjectionKind::U16,
        TypeKind::U32 => EnvironmentTypeProjectionKind::U32,
        TypeKind::U64 => EnvironmentTypeProjectionKind::U64,
        TypeKind::U128 => EnvironmentTypeProjectionKind::U128,
        TypeKind::USize => EnvironmentTypeProjectionKind::USize,
        TypeKind::F32 => EnvironmentTypeProjectionKind::F32,
        TypeKind::F64 => EnvironmentTypeProjectionKind::F64,
        TypeKind::String => EnvironmentTypeProjectionKind::String,
        TypeKind::Char => EnvironmentTypeProjectionKind::Char,
        TypeKind::Vec(item) => {
            EnvironmentTypeProjectionKind::Vec(Box::new(projection(item, source)))
        }
        TypeKind::Seq(item) => {
            EnvironmentTypeProjectionKind::Seq(Box::new(projection(item, source)))
        }
        TypeKind::Option(item) => {
            EnvironmentTypeProjectionKind::Option(Box::new(projection(item, source)))
        }
        TypeKind::Result { ok, error } => EnvironmentTypeProjectionKind::Result {
            ok: Box::new(projection(ok, source)),
            error: Box::new(projection(error, source)),
        },
        TypeKind::Tuple(items) => EnvironmentTypeProjectionKind::Tuple(
            items.iter().map(|item| projection(item, source)).collect(),
        ),
        TypeKind::Need { ready, error } => EnvironmentTypeProjectionKind::Need {
            ready: Box::new(projection(ready, source)),
            error: Box::new(projection(error, source)),
        },
        TypeKind::CharacterNominal(nominal) => {
            EnvironmentTypeProjectionKind::CharacterNominal(nominal.clone())
        }
        TypeKind::AcceptedNominal(nominal) => EnvironmentTypeProjectionKind::AcceptedNominal {
            id: nominal.declaration().clone(),
            arguments: nominal
                .arguments()
                .iter()
                .map(|argument| projection(argument, source))
                .collect(),
        },
        unsupported => panic!("unsupported test publication type: {unsupported:?}"),
    };
    EnvironmentTypeProjectionNode::new(source.clone(), kind)
}
