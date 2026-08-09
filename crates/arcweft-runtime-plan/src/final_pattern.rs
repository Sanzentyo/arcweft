//! Runtime-pattern projection from the accepted final-HIR generation.

use arcweft_core::pattern::{RuntimePattern, RuntimeRecordPatternField};
use arcweft_lang_hir::identity::{LocalId, PatternId};
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::pattern::{
    HirPatternBinding, HirPatternField, HirPatternKind, HirPatternRecordPath,
    HirPatternSequenceRest, HirVariantPatternPayload,
};

use crate::semantic_facts::RuntimePlanSemanticFacts;

pub(crate) struct FinalPatternLowerer<'hir> {
    module: &'hir HirModule,
    facts: &'hir RuntimePlanSemanticFacts,
}

impl<'hir> FinalPatternLowerer<'hir> {
    pub(crate) const fn new(
        module: &'hir HirModule,
        facts: &'hir RuntimePlanSemanticFacts,
    ) -> Self {
        Self { module, facts }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive final-HIR pattern projection keeps accepted and rejected pattern families in one closed match"
    )]
    pub(crate) fn lower(&self, id: PatternId) -> Result<RuntimePattern, String> {
        let pattern = self
            .module
            .resolve_pattern(id)
            .map_err(|error| format!("cannot resolve final-HIR pattern {id:?}: {error}"))?;
        if pattern.is_poisoned() {
            return Err(format!(
                "final-HIR pattern {id:?} contains recovery and is not executable"
            ));
        }
        match pattern.kind() {
            HirPatternKind::Binding(binding) => self.binding(binding, false),
            HirPatternKind::MutableBinding(binding) => self.binding(binding, true),
            HirPatternKind::Literal(_) => self
                .facts
                .pattern_literal(id)
                .cloned()
                .map(RuntimePattern::Literal)
                .ok_or_else(|| format!("checked literal fact is missing for pattern {id:?}")),
            HirPatternKind::EntityReference(_) => self
                .facts
                .pattern_item(id)
                .map(|item| RuntimePattern::Entity(item.public_id().as_str().to_owned()))
                .ok_or_else(|| {
                    format!("checked project-item fact is missing for entity pattern {id:?}")
                }),
            HirPatternKind::Variant(variant) => {
                let selected = self
                    .facts
                    .pattern_variant(id)
                    .ok_or_else(|| format!("checked variant fact is missing for pattern {id:?}"))?;
                let payload = match variant.payload() {
                    HirVariantPatternPayload::Absent => None,
                    HirVariantPatternPayload::Pattern(payload) => {
                        Some(Box::new(self.lower(*payload)?))
                    }
                    HirVariantPatternPayload::Recovered { .. } => {
                        return Err(format!("variant pattern {id:?} has a recovered payload"));
                    }
                };
                Ok(RuntimePattern::Variant {
                    owner: selected.owner().checked_type()?,
                    ordinal: selected.ordinal(),
                    name: selected.name().to_owned(),
                    payload,
                })
            }
            HirPatternKind::Discard => Ok(RuntimePattern::Discard),
            HirPatternKind::Tuple { elements } => elements
                .iter()
                .map(|element| self.lower(*element))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimePattern::Tuple),
            HirPatternKind::Record { path, fields } => {
                let owner = match path {
                    HirPatternRecordPath::Absent => None,
                    HirPatternRecordPath::Resolved(_) => Some(
                        self.facts
                            .pattern_nominal(id)
                            .ok_or_else(|| {
                                format!("checked nominal fact is missing for pattern {id:?}")
                            })?
                            .checked_type()?,
                    ),
                    HirPatternRecordPath::Recovered(_) => {
                        return Err(format!("record pattern {id:?} has a recovered path"));
                    }
                };
                let mut lowered = Vec::with_capacity(fields.len());
                let mut rest = false;
                for field in fields {
                    match field {
                        HirPatternField::Explicit { name, pattern } => {
                            lowered.push(RuntimeRecordPatternField {
                                name: name.as_str().to_owned(),
                                pattern: self.lower(*pattern)?,
                            });
                        }
                        HirPatternField::Shorthand { name, local } => {
                            lowered.push(RuntimeRecordPatternField {
                                name: name.as_str().to_owned(),
                                pattern: RuntimePattern::Ident(self.local_name(*local)?),
                            });
                        }
                        HirPatternField::Rest { binding } => {
                            if binding.is_some() {
                                return Err(format!(
                                    "record pattern {id:?} has a bound rest, which the runtime pattern algebra cannot preserve"
                                ));
                            }
                            rest = true;
                        }
                        HirPatternField::Invalid { .. } => {
                            return Err(format!("record pattern {id:?} has an invalid field"));
                        }
                    }
                }
                Ok(RuntimePattern::Record {
                    owner,
                    fields: lowered,
                    rest,
                })
            }
            HirPatternKind::BracketSequence { elements, rest } => {
                let items = elements
                    .iter()
                    .map(|element| self.lower(*element))
                    .collect::<Result<Vec<_>, _>>()?;
                let rest = match rest {
                    HirPatternSequenceRest::Absent | HirPatternSequenceRest::Unbound => None,
                    HirPatternSequenceRest::Bound(local) => Some(self.local_name(*local)?),
                    HirPatternSequenceRest::Recovered(_) => {
                        return Err(format!(
                            "bracket-sequence pattern {id:?} has a recovered rest"
                        ));
                    }
                };
                Ok(RuntimePattern::BracketSeq { items, rest })
            }
            HirPatternKind::WholeBinding { binding, pattern } => {
                let name = self.bound_name(binding)?;
                Ok(RuntimePattern::Whole {
                    name,
                    pattern: Box::new(self.lower(*pattern)?),
                })
            }
            HirPatternKind::TypedBinding { binding, ty } => {
                let name = self.bound_name(binding)?;
                let ty = self
                    .facts
                    .ty(*ty)
                    .ok_or_else(|| format!("checked type fact is missing for {ty:?}"))?;
                Ok(RuntimePattern::Typed {
                    name,
                    ty: ty.checked_type()?,
                })
            }
            HirPatternKind::Or { .. } => Err(format!(
                "or-pattern {id:?} must be expanded before runtime-plan lowering"
            )),
            HirPatternKind::Error(_) => Err(format!("error pattern {id:?} is not executable")),
        }
    }

    fn binding(
        &self,
        binding: &HirPatternBinding,
        mutable: bool,
    ) -> Result<RuntimePattern, String> {
        let name = self.bound_name(binding)?;
        Ok(if mutable {
            RuntimePattern::MutIdent(name)
        } else {
            RuntimePattern::Ident(name)
        })
    }

    fn bound_name(&self, binding: &HirPatternBinding) -> Result<String, String> {
        let HirPatternBinding::Bound { name, local } = binding else {
            return Err("recovered pattern binding cannot enter runtime lowering".to_owned());
        };
        let resolved = self.local_name(*local)?;
        debug_assert_eq!(resolved, name.as_str());
        Ok(resolved)
    }

    fn local_name(&self, local: LocalId) -> Result<String, String> {
        self.module
            .resolve_local(local)
            .map(|local| local.name().as_str().to_owned())
            .map_err(|error| format!("cannot resolve final-HIR local {local:?}: {error}"))
    }
}
