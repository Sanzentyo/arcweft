//! Effect projection for retained expression types and nominal metadata.

use super::*;
use crate::types::constraints::{ConstraintDomain, RetainedSourceCompletion, TypeConstraintError};

#[derive(Debug, Eq, PartialEq)]
pub(in crate::final_analysis) struct PreparedExpressionTypeUpdates {
    result: Option<TypeKind<PreparedEffects>>,
    nominal: Option<TypeKind<PreparedEffects>>,
    fields: Vec<FieldTypeUpdate>,
}

#[derive(Debug, Eq, PartialEq)]
struct FieldTypeUpdate {
    index: usize,
    nominal: Option<TypeKind<PreparedEffects>>,
    value: Option<TypeKind<PreparedEffects>>,
}

impl PreparedExpressionTypeUpdates {
    pub(in crate::final_analysis) fn resolved_type(ty: TypeKind<PreparedEffects>) -> Self {
        Self {
            result: Some(ty),
            nominal: None,
            fields: Vec::new(),
        }
    }

    pub(in crate::final_analysis) fn is_empty(&self) -> bool {
        self.result.is_none() && self.nominal.is_none() && self.fields.is_empty()
    }

    pub(in crate::final_analysis) fn matches_result(
        &self,
        result: Option<&TypeKind<PreparedEffects>>,
    ) -> bool {
        self.result.as_ref() == result
    }
}

impl PreparedExpressionFact {
    pub(in crate::final_analysis) fn supports_primary_type_update(&self) -> bool {
        matches!(
            self,
            Self::Resolved(_)
                | Self::ProjectField(_)
                | Self::ProjectRecord(_)
                | Self::ProjectNominalTypeValue(_)
        ) && self.result().is_plain_value()
    }

    pub(in crate::final_analysis) fn prepare_type_updates<D, C>(
        &self,
        completion: &RetainedSourceCompletion<'_, D>,
        control: &mut C,
    ) -> Result<Option<PreparedExpressionTypeUpdates>, TypeConstraintError>
    where
        D: ConstraintDomain,
        C: TypeProjectionControl<Error = TypeConstraintError>,
    {
        let mut updates = PreparedExpressionTypeUpdates {
            result: None,
            nominal: None,
            fields: Vec::new(),
        };
        let (result, nominal, fields) = match self {
            Self::Resolved(value) => (value.result(), None, None),
            Self::ProjectField(value) => (
                value.shell.result(),
                None,
                Some(NominalFields::Selection(&value.fields)),
            ),
            Self::ProjectRecord(value) => (
                value.shell.result(),
                Some(&value.nominal),
                Some(NominalFields::Record(&value.fields)),
            ),
            Self::ProjectNominalTypeValue(value) => {
                (value.shell.result(), Some(&value.nominal), None)
            }
            _ => {
                let mut unchanged = true;
                self.visit_types(&mut |ty| {
                    unchanged &= completion
                        .project_declaration_type(ty, control)?
                        .is_some_and(|ty| ty.into_replacement().is_none());
                    Ok::<_, TypeConstraintError>(())
                })?;
                return Ok(unchanged.then_some(updates));
            }
        };
        if let PreparedExpressionResult::Value(value) = result {
            let Some(source) = completion.project_declaration_type(value.source_type(), control)?
            else {
                return Ok(None);
            };
            updates.result = source.into_replacement();
            if let Some(specialized) = &value.specialization {
                let scope = &GenericScope::default();
                // A checked function-use witness retains both arrows in its
                // own root representation. Only the prepared source can
                // change, and it must agree with that exact witness.
                let source = specialized
                    .witness
                    .source_type()
                    .prepare_effects(scope, control)?;
                let use_type = specialized
                    .witness
                    .specialized_type()
                    .prepare_effects(scope, control)?;
                if updates.result.as_ref().unwrap_or(value.source_type()) != &source
                    || specialized.ty != use_type
                {
                    return Ok(None);
                }
            }
        }
        if let Some(nominal) = nominal {
            let Some(projected) = completion.project_declaration_type(nominal.ty(), control)?
            else {
                return Ok(None);
            };
            updates.nominal = projected.into_replacement();
        }
        if let Some(fields) = fields {
            for (index, (nominal, value)) in fields.types().enumerate() {
                let nominal = if let Some(nominal) = nominal {
                    let Some(projected) =
                        completion.project_declaration_type(nominal.ty(), control)?
                    else {
                        return Ok(None);
                    };
                    projected.into_replacement()
                } else {
                    None
                };
                let Some(projected) = completion.project_declaration_type(value, control)? else {
                    return Ok(None);
                };
                let value = projected.into_replacement();
                if nominal.is_some() || value.is_some() {
                    control.visit_binding()?;
                    updates.fields.push(FieldTypeUpdate {
                        index,
                        nominal,
                        value,
                    });
                }
            }
        }
        Ok(Some(updates))
    }

    pub(in crate::final_analysis) fn apply_type_updates(
        &mut self,
        updates: PreparedExpressionTypeUpdates,
    ) -> bool {
        let PreparedExpressionTypeUpdates {
            result,
            nominal,
            fields,
        } = updates;
        match self {
            Self::Resolved(value) => {
                nominal.is_none() && fields.is_empty() && update_result(value.result_mut(), result)
            }
            Self::ProjectField(value) => {
                nominal.is_none()
                    && update_result(&mut value.shell.result, result)
                    && fields.into_iter().all(|update| {
                        let Some(field) = value.fields.get_mut(update.index) else {
                            return false;
                        };
                        update_type(&mut field.field_type, update.value);
                        update_nominal(&mut field.nominal, update.nominal)
                    })
                    && value.shell.source_value_type()
                        == value.fields.last().map(|field| &field.field_type)
            }
            Self::ProjectRecord(value) => {
                update_result(&mut value.shell.result, result)
                    && update_nominal(&mut value.nominal, nominal)
                    && fields.into_iter().all(|update| {
                        let Some(field) = value.fields.get_mut(update.index) else {
                            return false;
                        };
                        update_type(&mut field.field_type, update.value);
                        update.nominal.is_none()
                    })
                    && value.shell.source_value_type() == Some(value.nominal.ty())
            }
            Self::ProjectNominalTypeValue(value) => {
                fields.is_empty()
                    && update_result(&mut value.shell.result, result)
                    && update_nominal(&mut value.nominal, nominal)
                    && matches!(value.shell.source_value_type(), Some(TypeKind::MetaType(inner)) if inner.as_ref() == value.nominal.ty())
            }
            _ => result.is_none() && nominal.is_none() && fields.is_empty(),
        }
    }
}

enum NominalFields<'a> {
    Selection(&'a [PreparedProjectFieldSelection]),
    Record(&'a [PreparedProjectRecordExpressionField]),
}

impl<'a> NominalFields<'a> {
    fn types(
        self,
    ) -> impl Iterator<
        Item = (
            Option<&'a PreparedProjectNominal>,
            &'a TypeKind<PreparedEffects>,
        ),
    > {
        let (selections, fields) = match self {
            Self::Selection(fields) => (fields, &[][..]),
            Self::Record(fields) => (&[][..], fields),
        };
        selections
            .iter()
            .map(|field| (Some(&field.nominal), &field.field_type))
            .chain(fields.iter().map(|field| (None, &field.field_type)))
    }
}

fn update_result(
    result: &mut PreparedExpressionResult,
    update: Option<TypeKind<PreparedEffects>>,
) -> bool {
    let Some(update) = update else {
        return true;
    };
    let PreparedExpressionResult::Value(value) = result else {
        return false;
    };
    value.source = update;
    true
}

fn update_nominal(
    nominal: &mut PreparedProjectNominal,
    update: Option<TypeKind<PreparedEffects>>,
) -> bool {
    let Some(update) = update else {
        return true;
    };
    if !matches!(&update, TypeKind::ProjectNominal(value) if value.declaration() == nominal.declaration())
    {
        return false;
    }
    nominal.ty = update;
    true
}

fn update_type(ty: &mut TypeKind<PreparedEffects>, update: Option<TypeKind<PreparedEffects>>) {
    if let Some(update) = update {
        *ty = update;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        effect_row::{
            EffectPredicate, EffectRow,
            prepared::{PreparedEffectPredicate, PreparedEffectRow},
        },
        types::{GenericBinder, ProjectNominalType},
    };

    #[test]
    fn record_effect_updates_preserve_field_coordinates() {
        let fixture = super::super::super::tests::fixture(
            "struct Holder<T> { value: T }\nfn source() { 1; }",
            None,
        );
        let declaration = fixture
            .symbols
            .nominal_symbols()
            .next()
            .unwrap()
            .id()
            .clone();
        let module = fixture
            .project
            .analysis_view()
            .unwrap()
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .unwrap();
        let source = PreparedRecordValueSource::Expression(module.expressions().next().unwrap().0);
        let callback = |labels| {
            TypeKind::function_with_contract(
                GenericBinder::EMPTY,
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                [],
                TypeKind::I64,
                PreparedEffectRow::Canonical(EffectRow::closed(labels)),
            )
        };
        let before = callback(EffectSet::new());
        let after = callback(EffectSet::from_labels(["record.read"]).unwrap());
        let nominal = |arg| ProjectNominalType::new(declaration.clone(), vec![arg]);
        let old = nominal(before.clone());
        let new = TypeKind::ProjectNominal(nominal(after.clone()));
        let mut fact = PreparedExpressionFact::from(PreparedProjectRecordExpression::new(
            PreparedExpressionShell::value(
                TypeKind::ProjectNominal(old.clone()),
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
            ),
            PreparedProjectNominal::new(old),
            Box::new([PreparedProjectRecordExpressionField::new(
                0, 0, before, source,
            )]),
        ));
        assert!(fact.apply_type_updates(PreparedExpressionTypeUpdates {
            result: Some(new.clone()),
            nominal: Some(new.clone()),
            fields: vec![FieldTypeUpdate {
                index: 0,
                nominal: None,
                value: Some(after.clone())
            }],
        }));
        let PreparedExpressionFact::ProjectRecord(value) = fact else {
            panic!("record fact")
        };
        assert_eq!(value.shell().value_type(), Some(&new));
        assert_eq!(value.nominal().ty(), &new);
        assert_eq!(value.fields()[0].field_type(), &after);
        assert_eq!(value.fields()[0].source(), source);
        assert_eq!(value.fields()[0].declaration_ordinal(), 0);
        assert_eq!(value.fields()[0].source_ordinal(), 0);
    }
}
