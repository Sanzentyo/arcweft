//! Complete type projection from the selected executable's sealed partition.

use arcweft_lang_hir::identity::HirValueTypeOwner;
use std::collections::BTreeMap;

use arcweft_lang_hir::{
    identity::TypeId, module::HirModule, stmt::HirStmtKind, symbol::ProjectSymbolTable,
    type_ref::HirTypeKind,
};
use arcweft_lang_sema::{
    final_analysis::{
        CheckedExecutableRuntimeFactPartition, CheckedExecutableRuntimePatternFactFamily,
        CheckedPatternResolution, FinalSemanticAnalysis,
    },
    registration::RegisteredSemanticWorld,
    types::TypeKind,
};
use arcweft_runtime_plan::semantic_facts::{
    RuntimeExecutableTypeProjection, RuntimeNormalizedType, RuntimeSequenceKind, RuntimeTypeShape,
};

use super::{
    ProjectInstantiationOrigin, RuntimeExecutableInstantiation, RuntimeSemanticProjectionError,
    RuntimeTypeProjectionPath, runtime_type_scoped_at,
};

impl RuntimeExecutableInstantiation<'_> {
    pub(super) fn runtime_type(
        self,
        ty: &arcweft_lang_sema::types::TypeKind,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<RuntimeNormalizedType, RuntimeSemanticProjectionError> {
        let ty = self.instantiate_type(ty)?;
        let scope = match self {
            Self::Program { environment, .. } => environment.type_scope(),
            _ => arcweft_lang_sema::types::GenericScope::default(),
        };
        runtime_type_scoped_at(
            &ty,
            symbols,
            world,
            analysis,
            &RuntimeTypeProjectionPath::root(),
            &scope,
        )
    }
    pub(super) fn type_projection<'abi>(
        self,
        origin: ProjectInstantiationOrigin,
        partition: &CheckedExecutableRuntimeFactPartition,
        source: (
            &HirModule,
            impl IntoIterator<Item = (TypeId, &'abi RuntimeNormalizedType)>,
        ),
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<Box<[RuntimeExecutableTypeProjection]>, RuntimeSemanticProjectionError> {
        let (module, callable_abis) = source;
        let binding_abis = binding_annotation_abis(module, partition, analysis, origin, |ty| {
            self.runtime_type(ty, symbols, world, analysis)
        })?;
        let mut roots = binding_abis
            .iter()
            .map(|(owner, ty)| (*owner, ty))
            .collect::<Vec<_>>();
        for root in callable_abis {
            roots.push(root);
        }
        let source_type_abis = source_type_abis(module, origin, roots)?;
        let mut projection = Vec::new();
        for expected in partition.expressions() {
            let owner = expected.owner();
            if let Some(proof) =
                partition.semantic_value_projection(HirValueTypeOwner::Expression(owner))
            {
                projection.push(RuntimeExecutableTypeProjection::semantic_value(proof));
                continue;
            }
            if !expected.has_runtime_type() {
                projection.push(RuntimeExecutableTypeProjection::semantic_only_expression(
                    owner,
                ));
                continue;
            }
            let ty = analysis
                .expression(owner)
                .and_then(|checked| checked.value_type())
                .ok_or_else(|| origin.error("runtime expression has no checked value type"))?;
            projection.push(RuntimeExecutableTypeProjection::value(
                HirValueTypeOwner::Expression(owner),
                self.runtime_type(ty, symbols, world, analysis)?,
            ));
        }
        for expected in partition.patterns() {
            let owner = expected.owner();
            if let Some(proof) =
                partition.semantic_value_projection(HirValueTypeOwner::Pattern(owner))
            {
                projection.push(RuntimeExecutableTypeProjection::semantic_value(proof));
                continue;
            }
            let checked = analysis
                .pattern(owner)
                .ok_or_else(|| origin.error("runtime pattern has no checked semantic fact"))?;
            projection.push(RuntimeExecutableTypeProjection::value(
                HirValueTypeOwner::Pattern(owner),
                self.runtime_type(checked.ty(), symbols, world, analysis)?,
            ));
        }
        for owner in partition.locals().iter().chain(partition.input_locals()) {
            if let Some(proof) =
                partition.semantic_value_projection(HirValueTypeOwner::Local(*owner))
            {
                projection.push(RuntimeExecutableTypeProjection::semantic_value(proof));
                continue;
            }
            let checked = analysis
                .local(*owner)
                .ok_or_else(|| origin.error("runtime local has no checked semantic fact"))?;
            projection.push(RuntimeExecutableTypeProjection::local(
                *owner,
                self.runtime_type(checked.ty(), symbols, world, analysis)?,
                analysis.local_binding_origin(*owner).map_err(|_| {
                    origin.error("runtime local has no accepted lexical binding origin")
                })?,
            ));
        }
        for expected in partition.types() {
            let owner = expected.owner();
            if let Some(proof) = partition.semantic_value_projection(HirValueTypeOwner::Type(owner))
            {
                projection.push(RuntimeExecutableTypeProjection::semantic_value(proof));
                continue;
            }
            if !expected.has_runtime_type() {
                projection.push(RuntimeExecutableTypeProjection::semantic_only_type(
                    owner,
                    expected.purpose().clone(),
                ));
                continue;
            }
            let ty = if let Some(abi) = source_type_abis.get(&owner) {
                (*abi).clone()
            } else {
                let checked = analysis.ty(owner).ok_or_else(|| {
                    origin.error("runtime type root has no checked semantic fact")
                })?;
                self.runtime_type(checked, symbols, world, analysis)?
            };
            projection.push(RuntimeExecutableTypeProjection::value(
                HirValueTypeOwner::Type(owner),
                ty,
            ));
        }
        for owner in partition.captures() {
            if let Some(proof) =
                partition.semantic_value_projection(HirValueTypeOwner::Capture(*owner))
            {
                projection.push(RuntimeExecutableTypeProjection::semantic_value(proof));
            }
        }
        projection.sort_by_key(RuntimeExecutableTypeProjection::owner);
        Ok(projection.into_boxed_slice())
    }
}

/// A binding annotation gets its executable type from the checked binding.
/// The authored type remains available separately for source analysis.
fn binding_annotation_abis(
    module: &HirModule,
    partition: &CheckedExecutableRuntimeFactPartition,
    analysis: &FinalSemanticAnalysis,
    origin: ProjectInstantiationOrigin,
    mut project: impl FnMut(&TypeKind) -> Result<RuntimeNormalizedType, RuntimeSemanticProjectionError>,
) -> Result<Vec<(TypeId, RuntimeNormalizedType)>, RuntimeSemanticProjectionError> {
    let mut annotations = Vec::new();
    for expected in partition.statements() {
        let statement = module.resolve_stmt(expected.owner()).map_err(|_| {
            origin.error("binding annotation has no statement in its executable partition")
        })?;
        let (HirStmtKind::Let {
            annotation: Some(annotation),
            pattern,
            ..
        }
        | HirStmtKind::LetElse {
            annotation: Some(annotation),
            pattern,
            ..
        }) = statement.kind()
        else {
            continue;
        };
        let checked = analysis
            .pattern(*pattern)
            .ok_or_else(|| origin.error("binding annotation has no checked binding pattern"))?;
        annotations.push((*annotation, project(checked.ty())?));
    }
    for expected in partition.patterns() {
        if expected.family() != CheckedExecutableRuntimePatternFactFamily::TypedBinding {
            continue;
        }
        let owner = expected.owner();
        let annotation = module
            .resolve_pattern(owner)
            .ok()
            .and_then(|pattern| pattern.kind().authored_type())
            .ok_or_else(|| origin.error("typed binding has no authored annotation"))?;
        let Some(CheckedPatternResolution::TypedBinding(binding)) =
            analysis.pattern(owner).map(|pattern| pattern.resolution())
        else {
            return Err(origin.error("typed binding has no checked annotation"));
        };
        annotations.push((annotation, project(binding.annotation())?));
    }
    Ok(annotations)
}

/// Associate authored structural type occurrences with the corresponding
/// fields of the selected callable ABI. An omitted effect in a nested type
/// annotation is completed by the same instance that owns the enclosing ABI.
fn source_type_abis<'a>(
    module: &HirModule,
    origin: ProjectInstantiationOrigin,
    mut pending: Vec<(TypeId, &'a RuntimeNormalizedType)>,
) -> Result<BTreeMap<TypeId, &'a RuntimeNormalizedType>, RuntimeSemanticProjectionError> {
    let mut types = BTreeMap::new();
    while let Some((owner, abi)) = pending.pop() {
        if let Some(previous) = types.insert(owner, abi) {
            if previous != abi {
                return Err(origin.error("source type has conflicting executable ABI fields"));
            }
            continue;
        }
        let ty = module.resolve_type(owner).map_err(|_| {
            origin.error("executable ABI field has no matching authored type occurrence")
        })?;
        let mismatch =
            || origin.error("authored structural type does not match its executable ABI");
        let (children, fields) = match (ty.kind(), abi.shape()) {
            (HirTypeKind::Tuple(children), RuntimeTypeShape::Tuple(fields))
            | (HirTypeKind::Choice(children), RuntimeTypeShape::Choice(fields)) => {
                (children.as_ref(), fields.as_ref())
            }
            (
                HirTypeKind::Function(source),
                RuntimeTypeShape::Function {
                    parameters, result, ..
                },
            ) => {
                pending.push((source.return_type(), result.as_ref()));
                (source.parameters(), parameters.as_ref())
            }
            (HirTypeKind::Reference(source), RuntimeTypeShape::Reference(target)) => {
                pending.push((source.referent(), target.as_ref()));
                continue;
            }
            (
                HirTypeKind::Slice(source),
                RuntimeTypeShape::Sequence {
                    kind: RuntimeSequenceKind::Slice,
                    item,
                },
            ) => {
                pending.push((*source, item.as_ref()));
                continue;
            }
            (HirTypeKind::Tuple(children), RuntimeTypeShape::Unit) if children.is_empty() => {
                continue;
            }
            (
                HirTypeKind::Tuple(_)
                | HirTypeKind::Choice(_)
                | HirTypeKind::Function(_)
                | HirTypeKind::Reference(_)
                | HirTypeKind::Slice(_),
                _,
            ) => return Err(mismatch()),
            _ => continue,
        };
        if children.len() != fields.len() {
            return Err(mismatch());
        }
        pending.extend(children.iter().copied().zip(fields));
    }
    Ok(types)
}
