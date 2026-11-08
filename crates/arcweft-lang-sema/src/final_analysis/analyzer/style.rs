//! Source ownership and contextual expectations for specified Style values.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    identity::{ExprId, HirModuleId, ItemId, TypeId},
    item::{HirItemKind, HirStyleBodyItem},
    module::HirModule,
};
use arcweft_view::style::{ViewPropertyKind, ViewStyleValueKind};

use crate::{
    nominal::{ResolvedTypeRefOutcome, TypeResolutionReport},
    types::{CompileTimeStyleType, TypeKind},
};

use super::super::FinalSemanticAnalysisError;

#[derive(Clone, Copy)]
struct StyleTokenAnnotationOwner {
    style: ItemId,
    ordinal: u32,
    value: ExprId,
}

pub(super) struct StyleAnalysisContext {
    values: BTreeMap<ExprId, ViewStyleValueKind>,
    annotations: BTreeMap<TypeId, StyleTokenAnnotationOwner>,
}

impl StyleAnalysisContext {
    pub(super) fn from_modules(
        modules: &BTreeMap<HirModuleId, &HirModule>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut context = Self {
            values: BTreeMap::new(),
            annotations: BTreeMap::new(),
        };
        for module in modules.values() {
            for (owner, item) in module.items() {
                let HirItemKind::Style(style) = item.kind() else {
                    continue;
                };
                for (ordinal, token) in style.tokens().iter().enumerate() {
                    let Some(ty) = token.value_type() else {
                        continue;
                    };
                    let ordinal = u32::try_from(ordinal)
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
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
}
