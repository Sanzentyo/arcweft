//! Complete type projection from the selected executable's sealed partition.

use arcweft_lang_hir::identity::HirValueTypeOwner;
use std::collections::BTreeMap;

use arcweft_lang_hir::{identity::TypeId, symbol::ProjectSymbolTable};
use arcweft_lang_sema::{
    final_analysis::{CheckedExecutableRuntimeFactPartition, FinalSemanticAnalysis},
    registration::RegisteredSemanticWorld,
};
use arcweft_runtime_plan::semantic_facts::{
    RuntimeExecutableTypeProjection, RuntimeNormalizedType,
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
        source_type_abis: impl IntoIterator<Item = (TypeId, &'abi RuntimeNormalizedType)>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<Box<[RuntimeExecutableTypeProjection]>, RuntimeSemanticProjectionError> {
        // A checked parameter ABI can close an inferred callback effect row
        // which remains omitted at its authored TypeId. Preserve that exact
        // owner substitution rather than normalizing the annotation again.
        let source_type_abis = source_type_abis.into_iter().collect::<BTreeMap<_, _>>();
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
        projection.sort_by_key(RuntimeExecutableTypeProjection::owner);
        Ok(projection.into_boxed_slice())
    }
}
