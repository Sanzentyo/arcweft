//! Source ownership and contextual expectations for specified Style values.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    expr::{HirExprKind, HirSelectedMember},
    identity::{ExprId, HirModuleId, ItemId, TypeId},
    item::{HirItemKind, HirStyleBodyItem},
    leaf::{HirPathRoot, HirPathSegment},
    module::HirModule,
};
use arcweft_view::style::{ViewPropertyKind, ViewStyleTokenId, ViewStyleValueKind};

use crate::{
    nominal::{ResolvedTypeRefOutcome, TypeResolutionReport},
    types::{CompileTimeStyleType, TypeKind},
};

use super::super::CheckedStyleTokenReference;
use super::super::FinalSemanticAnalysisError;

struct StyleTokenDeclaration {
    ordinal: u32,
    value: ExprId,
    id: ViewStyleTokenId,
}

#[derive(Default)]
struct StyleTokenScope {
    declarations: BTreeMap<Vec<String>, StyleTokenDeclaration>,
    max_path_segments: usize,
}

#[derive(Clone, Copy)]
struct StyleTokenAnnotationOwner {
    style: ItemId,
    ordinal: u32,
    value: ExprId,
}

pub(super) struct StyleAnalysisContext {
    values: BTreeMap<ExprId, ViewStyleValueKind>,
    annotations: BTreeMap<TypeId, StyleTokenAnnotationOwner>,
    tokens: BTreeMap<ItemId, StyleTokenScope>,
}

impl StyleAnalysisContext {
    pub(super) fn from_modules(
        modules: &BTreeMap<HirModuleId, &HirModule>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut context = Self {
            values: BTreeMap::new(),
            annotations: BTreeMap::new(),
            tokens: BTreeMap::new(),
        };
        for module in modules.values() {
            for (owner, item) in module.items() {
                let HirItemKind::Style(style) = item.kind() else {
                    continue;
                };
                for (ordinal, token) in style.tokens().iter().enumerate() {
                    let ordinal = u32::try_from(ordinal)
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                    let id = arcweft_id::PublicId::try_from(token)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                    let name: Vec<String> = id.as_str().split('.').map(str::to_owned).collect();
                    let scope = context.tokens.entry(owner).or_default();
                    scope.max_path_segments = scope.max_path_segments.max(name.len());
                    if scope
                        .declarations
                        .insert(
                            name,
                            StyleTokenDeclaration {
                                ordinal,
                                value: token.value(),
                                id: ViewStyleTokenId::from_public_id(id),
                            },
                        )
                        .is_some()
                    {
                        return Err(FinalSemanticAnalysisError::InvalidOwner);
                    }
                    let Some(ty) = token.value_type() else {
                        continue;
                    };
                    if context
                        .annotations
                        .insert(
                            ty,
                            StyleTokenAnnotationOwner {
                                style: owner,
                                ordinal,
                                value: token.value(),
                            },
                        )
                        .is_some()
                    {
                        return Err(FinalSemanticAnalysisError::InvalidOwner);
                    }
                }
                context.collect_body(style.body())?;
            }
        }
        Ok(context)
    }

    fn collect_body(
        &mut self,
        body: &[HirStyleBodyItem],
    ) -> Result<(), FinalSemanticAnalysisError> {
        for item in body {
            match item {
                HirStyleBodyItem::Rule(rule) => {
                    for declaration in rule.declarations() {
                        let Some(kind) = declaration
                            .property()
                            .as_str()
                            .and_then(ViewPropertyKind::from_source_name)
                        else {
                            continue;
                        };
                        self.insert_value(declaration.value(), kind.value_kind())?;
                    }
                }
                HirStyleBodyItem::Environment(environment) => {
                    self.collect_body(environment.body())?
                }
                HirStyleBodyItem::Recovered(_) => {}
            }
        }
        Ok(())
    }

    fn insert_value(
        &mut self,
        owner: ExprId,
        kind: ViewStyleValueKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if self.values.insert(owner, kind).is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(())
    }

    pub(super) fn annotation_owner(&self, owner: TypeId) -> Option<(ItemId, u32)> {
        self.annotations
            .get(&owner)
            .map(|annotation| (annotation.style, annotation.ordinal))
    }

    pub(super) fn complete_annotations(
        &mut self,
        reports: &BTreeMap<TypeId, TypeResolutionReport>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        for (owner, annotation) in &self.annotations {
            let Some(report) = reports.get(owner) else {
                return Err(FinalSemanticAnalysisError::TypeResolutionFailed { owner: *owner });
            };
            let ResolvedTypeRefOutcome::Complete(product) = report.outcome() else {
                return Err(FinalSemanticAnalysisError::TypeResolutionFailed { owner: *owner });
            };
            let TypeKind::CompileTimeStyle(CompileTimeStyleType::Value(kind)) = product.recovered()
            else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            if self.values.insert(annotation.value, *kind).is_some() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        Ok(())
    }

    pub(super) fn value_kind(&self, owner: ExprId) -> Option<ViewStyleValueKind> {
        self.values.get(&owner).copied()
    }

    pub(super) fn token(
        &self,
        module: &HirModule,
        style: ItemId,
        mut expression: ExprId,
    ) -> Option<CheckedStyleTokenReference> {
        let scope = self.tokens.get(&style)?;
        let mut suffix = Vec::new();
        // No declared name can match after this exact scope-derived bound.
        // Qualifiers remain HIR syntax and are never evaluated as values.
        let mut name = loop {
            match module.resolve_expr(expression).ok()?.kind() {
                HirExprKind::Select(select) => {
                    let HirSelectedMember::Name(member) = select.member() else {
                        return None;
                    };
                    if suffix.len() >= scope.max_path_segments {
                        return None;
                    }
                    suffix.push(member.as_str().to_owned());
                    expression = select.target();
                }
                HirExprKind::Path(path) => {
                    let path = path.as_resolved()?;
                    if path.root() != HirPathRoot::ImplicitCrate
                        || path.segments().len().checked_add(suffix.len())?
                            > scope.max_path_segments
                    {
                        return None;
                    }
                    break path
                        .segments()
                        .iter()
                        .map(|segment| match segment {
                            HirPathSegment::Identifier(name) => name.as_str().to_owned(),
                            HirPathSegment::ProjectSymbol(name) => name.as_str().to_owned(),
                        })
                        .collect::<Vec<_>>();
                }
                _ => return None,
            }
        };
        name.extend(suffix.into_iter().rev());
        let token = scope.declarations.get(&name)?;
        Some(CheckedStyleTokenReference::new(
            style,
            token.ordinal,
            token.value,
            token.id.clone(),
            self.value_kind(token.value)?,
        ))
    }
}
