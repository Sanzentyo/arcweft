//! Authored body order and source-provenanced native environment conjunctions.

use arcweft_bundle::resource_codec::SourceRangeRef;
use arcweft_lang_hir::{
    identity::ItemId,
    item::{HirStyleAssignOperation, HirStyleBodyItem, HirStyleRule},
    module::HirModule,
    source_index::{HirStyleBodyPath, HirStyleBodySourcePart, HirStyleSourceRole},
};
use arcweft_lang_sema::{
    final_analysis::{CheckedExpressionResolution, CheckedStyleOperand, FinalSemanticAnalysis},
    types::{CompileTimeStyleType, TypeKind},
};
use arcweft_source::ProductSourceRef;
use arcweft_view::style::{
    ViewEnvironmentClause, ViewEnvironmentCondition, ViewEnvironmentWrapperIndex,
    ViewEnvironmentWrapperSource, ViewPropertyKind, ViewStyleAssignOp, ViewStyleDeclaration,
    ViewStyleRule, ViewStyleSourceId,
};

use super::{ViewStyleLowerError, lower_selector, style_product_source};

/// One sheet's lowering owns source order and the resource's shared source tables.
pub(super) struct StyleBodyLowerer<'a> {
    module: &'a HirModule,
    owner: ItemId,
    analysis: &'a FinalSemanticAnalysis,
    product_sources: &'a mut Vec<ProductSourceRef>,
    source_ranges: &'a mut Vec<SourceRangeRef>,
    source_order: u32,
}

#[derive(Clone, Default)]
struct EnvironmentFrames {
    wrappers: Vec<ViewEnvironmentWrapperSource>,
    clauses: Vec<ViewEnvironmentClause>,
}

impl<'a> StyleBodyLowerer<'a> {
    pub(super) fn new(
        module: &'a HirModule,
        owner: ItemId,
        analysis: &'a FinalSemanticAnalysis,
        product_sources: &'a mut Vec<ProductSourceRef>,
        source_ranges: &'a mut Vec<SourceRangeRef>,
    ) -> Self {
        Self {
            module,
            owner,
            analysis,
            product_sources,
            source_ranges,
            source_order: 0,
        }
    }

    pub(super) fn lower(
        mut self,
        body: &[HirStyleBodyItem],
    ) -> Result<Vec<ViewStyleRule>, ViewStyleLowerError> {
        let mut rules = Vec::new();
        self.lower_body(
            body,
            &HirStyleBodyPath::root(),
            &EnvironmentFrames::default(),
            &mut rules,
        )?;
        Ok(rules)
    }

    fn error(&self) -> ViewStyleLowerError {
        ViewStyleLowerError::MissingCheckedStyleProjection { owner: self.owner }
    }

    fn source(
        &mut self,
        path: &HirStyleBodyPath,
        part: HirStyleBodySourcePart,
        role: &'static str,
    ) -> Result<ViewStyleSourceId, ViewStyleLowerError> {
        style_product_source(
            self.module,
            self.owner,
            HirStyleSourceRole::Body {
                path: path.clone(),
                part,
            },
            role,
            self.product_sources,
            self.source_ranges,
        )
    }

    fn lower_body(
        &mut self,
        body: &[HirStyleBodyItem],
        path: &HirStyleBodyPath,
        frames: &EnvironmentFrames,
        rules: &mut Vec<ViewStyleRule>,
    ) -> Result<(), ViewStyleLowerError> {
        for (ordinal, item) in body.iter().enumerate() {
            let ordinal = u32::try_from(ordinal).map_err(|_| self.error())?;
            match item {
                HirStyleBodyItem::Rule(rule) => {
                    rules.push(self.lower_rule(rule, ordinal, path, frames)?)
                }
                HirStyleBodyItem::Environment(environment) => {
                    if environment.clauses().is_empty()
                        || frames.wrappers.len() >= ViewEnvironmentCondition::MAX_WRAPPERS
                        || frames
                            .clauses
                            .len()
                            .checked_add(environment.clauses().len())
                            .is_none_or(|count| count > ViewEnvironmentCondition::MAX_CLAUSES)
                    {
                        return Err(self.error());
                    }
                    let wrapper = ViewEnvironmentWrapperIndex::new(
                        u8::try_from(frames.wrappers.len()).map_err(|_| self.error())?,
                    );
                    let mut nested = frames.clone();
                    let predicate_source = self.source(
                        path,
                        HirStyleBodySourcePart::EnvironmentCondition {
                            environment: ordinal,
                        },
                        "environment predicate",
                    )?;
                    let body_source = self.source(
                        path,
                        HirStyleBodySourcePart::EnvironmentBody {
                            environment: ordinal,
                        },
                        "environment body",
                    )?;
                    let scope_source = self.source(
                        path,
                        HirStyleBodySourcePart::EnvironmentWhole {
                            environment: ordinal,
                        },
                        "environment scope",
                    )?;
                    nested.wrappers.push(ViewEnvironmentWrapperSource::new(
                        predicate_source,
                        body_source,
                        scope_source,
                    ));
                    for (clause, original) in environment.clauses().iter().enumerate() {
                        let clause = u32::try_from(clause).map_err(|_| self.error())?;
                        let checked = self
                            .analysis
                            .expression(original.value())
                            .ok_or_else(|| self.error())?;
                        let CheckedExpressionResolution::StyleOperand(
                            CheckedStyleOperand::Environment(checked),
                        ) = checked.resolution()
                        else {
                            return Err(self.error());
                        };
                        if !checked.matches_clause(
                            self.owner,
                            path,
                            ordinal,
                            clause,
                            original.value(),
                        ) {
                            return Err(self.error());
                        }
                        let test = checked.test();
                        let source = self.source(
                            path,
                            HirStyleBodySourcePart::ClauseWhole {
                                environment: ordinal,
                                clause,
                            },
                            "environment clause",
                        )?;
                        nested
                            .clauses
                            .push(ViewEnvironmentClause::new(test, wrapper, source));
                    }
                    // Validate the merged conjunction even if this wrapper has no rules.
                    ViewEnvironmentCondition::try_new(
                        nested.wrappers.clone(),
                        nested.clauses.clone(),
                    )?;
                    let mut nested_path = path.ordinals().to_vec();
                    nested_path.push(ordinal);
                    self.lower_body(
                        environment.body(),
                        &HirStyleBodyPath::from_ordinals(nested_path.into_boxed_slice()),
                        &nested,
                        rules,
                    )?;
                }
                HirStyleBodyItem::Recovered(_) => return Err(self.error()),
            }
        }
        Ok(())
    }

    fn lower_rule(
        &mut self,
        rule: &HirStyleRule,
        ordinal: u32,
        path: &HirStyleBodyPath,
        frames: &EnvironmentFrames,
    ) -> Result<ViewStyleRule, ViewStyleLowerError> {
        let selector = lower_selector(rule.selector()).ok_or_else(|| self.error())?;
        let source = self.source(
            path,
            HirStyleBodySourcePart::RuleSelector { rule: ordinal },
            "rule selector",
        )?;
        let mut declarations = Vec::new();
        for (declaration_ordinal, declaration) in rule.declarations().iter().enumerate() {
            let property = declaration
                .property()
                .as_str()
                .and_then(ViewPropertyKind::from_source_name)
                .ok_or_else(|| self.error())?;
            let checked = self
                .analysis
                .expression(declaration.value())
                .ok_or_else(|| self.error())?;
            let value = match checked.resolution() {
                CheckedExpressionResolution::StyleValue(value)
                    if value.kind() == property.value_kind()
                        && checked.value_type()
                            == Some(&TypeKind::CompileTimeStyle(CompileTimeStyleType::Value(
                                value.kind(),
                            ))) =>
                {
                    value.clone()
                }
                _ => return Err(self.error()),
            };
            let declaration_ordinal =
                u32::try_from(declaration_ordinal).map_err(|_| self.error())?;
            let declaration_source = self.source(
                path,
                HirStyleBodySourcePart::DeclarationWhole {
                    rule: ordinal,
                    declaration: declaration_ordinal,
                },
                "declaration",
            )?;
            let op = match declaration.operation() {
                HirStyleAssignOperation::Replace => ViewStyleAssignOp::Replace,
                HirStyleAssignOperation::Append => ViewStyleAssignOp::Append,
                HirStyleAssignOperation::Recovered(_) => return Err(self.error()),
            };
            declarations.push(ViewStyleDeclaration::new(
                property,
                value,
                op,
                declaration_source,
            )?);
        }
        let environment = if frames.wrappers.is_empty() {
            None
        } else {
            Some(ViewEnvironmentCondition::try_new(
                frames.wrappers.clone(),
                frames.clauses.clone(),
            )?)
        };
        let rule = ViewStyleRule::new(
            selector,
            environment,
            declarations,
            self.source_order,
            source,
        )?;
        self.source_order = self
            .source_order
            .checked_add(1)
            .ok_or_else(|| self.error())?;
        Ok(rule)
    }
}
