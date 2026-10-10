//! Authored Choice identities and static targets shared by analysis and sealing.

use arcweft_id::PublicId;
use arcweft_lang_hir::{
    identity::{ExprId, ScopeId},
    leaf::{HirIdRef, HirIdRefValue},
    module::HirModule,
    scope::HirScopeOwner,
    source_index::{
        HirChoiceCompactArmSourcePart, HirExprSourceRole, HirSourcePresence, HirSourceQuery,
        HirSourceSite,
    },
    symbol::{
        CallableDeclarationKey, CallableDeclarationOwner, ProjectSymbolTable, ResolvedProjectSymbol,
    },
};

use super::{CheckedProjectItem, FinalSemanticAnalysisError};

fn checked_id(value: &str) -> Result<PublicId, FinalSemanticAnalysisError> {
    let id = PublicId::try_new(value.to_owned())
        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
    (id.as_str().split('.').next() == Some("choice"))
        .then_some(id)
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
}

pub(super) fn public_id(
    symbols: &ProjectSymbolTable,
    module: &HirModule,
    mut scope: ScopeId,
    value: &HirIdRefValue,
) -> Result<PublicId, FinalSemanticAnalysisError> {
    let reference = value
        .as_resolved()
        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
    if let HirIdRef::Absolute(value) = reference {
        return checked_id(value.as_str());
    }
    let mut named_scopes = Vec::new();
    let item = loop {
        let node = module
            .resolve_scope(scope)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if let HirScopeOwner::Item(owner) = node.owner() {
            break *owner;
        }
        if let Some(named) = module
            .scope_namespace(scope)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
        {
            named_scopes.push(named.name().as_str());
        }
        scope = node
            .parent()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
    };
    named_scopes.reverse();
    let symbol = symbols
        .flow_symbol_for_item(item)
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
    let CallableDeclarationKey::Flow(flow) = symbol.declaration() else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    let flow_path = flow
        .public_id()
        .as_str()
        .strip_prefix("flow.")
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
    let relative = match reference {
        HirIdRef::Relative(relative) => relative,
        HirIdRef::FamilyRelative(relative) if relative.family().as_str() == "choice" => {
            relative.relative()
        }
        HirIdRef::FamilyRelative(_) | HirIdRef::Absolute(_) => {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    };
    if relative.parent_depth() > named_scopes.len() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    let mut id = String::from("choice.");
    id.push_str(flow_path);
    for name in &named_scopes[..named_scopes.len() - relative.parent_depth()] {
        id.push('.');
        id.push_str(name);
    }
    id.push('.');
    id.push_str(relative.suffix().as_str());
    checked_id(&id)
}

pub(super) fn option_id(
    value: &HirIdRefValue,
    parent: Option<&PublicId>,
) -> Result<PublicId, FinalSemanticAnalysisError> {
    let reference = value
        .as_resolved()
        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
    if let HirIdRef::Absolute(value) = reference {
        return checked_id(value.as_str());
    }
    let relative = match reference {
        HirIdRef::Relative(relative) => relative,
        HirIdRef::FamilyRelative(relative) if relative.family().as_str() == "choice" => {
            relative.relative()
        }
        HirIdRef::FamilyRelative(_) | HirIdRef::Absolute(_) => {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    };
    let parent = parent.ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
    let mut segments = parent.as_str().split('.').collect::<Vec<_>>();
    if relative.parent_depth() >= segments.len() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    segments.truncate(segments.len() - relative.parent_depth());
    segments.extend(relative.suffix().as_str().split('.'));
    checked_id(&segments.join("."))
}

pub(super) fn goto(
    symbols: &ProjectSymbolTable,
    module: &HirModule,
    owner: ExprId,
    arm: u32,
    value: &HirIdRefValue,
) -> Result<CheckedProjectItem, FinalSemanticAnalysisError> {
    let reference = value
        .as_resolved()
        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
    let source = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::ChoiceCompactArm {
                    arm,
                    part: HirChoiceCompactArmSourcePart::GotoTarget,
                },
            },
        )
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let HirSourcePresence::Present(HirSourceSite::Span(source)) = source.presence() else {
        return Err(FinalSemanticAnalysisError::RecoveredOwner);
    };
    let target = symbols
        .resolve_entity_reference(module.key().path(), reference, source.clone())
        .map_err(|_| FinalSemanticAnalysisError::ValueResolutionFailed { owner })?;
    match target {
        ResolvedProjectSymbol::StructuralCallable(symbol)
            if symbol.owner() == CallableDeclarationOwner::Flow =>
        {
            CheckedProjectItem::new_flow(symbol.declaration().clone(), symbol.source_item())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
        }
        _ => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
    }
}
