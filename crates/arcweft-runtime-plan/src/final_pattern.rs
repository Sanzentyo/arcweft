//! Runtime-pattern seed projection from accepted final HIR.

use std::collections::BTreeMap;

use arcweft_core::plan::{
    RuntimeLocalSeedId, RuntimePatternRestSeed, RuntimePatternSeed, RuntimePatternSeedKind,
    RuntimeRecordFieldSeedId, RuntimeRecordPatternFieldSeed,
};
use arcweft_core::value::{RuntimeEntityReference, RuntimeValue};
use arcweft_lang_hir::identity::{LocalId, PatternId};
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::pattern::{
    HirPatternBinding, HirPatternKind, HirPatternSequenceRest, HirVariantPatternPayload,
};

use crate::semantic_facts::{
    RuntimeExecutableSemanticFactView, RuntimePlanSemanticFacts,
    RuntimeProjectFunctionInstanceSemanticFacts, RuntimeProjectItem, RuntimeRecordPatternFact,
    RuntimeRecordPatternRest, RuntimeRecordPatternSource, RuntimeResolvedVariant,
};

pub(crate) struct FinalPatternLowerer<'hir> {
    module: &'hir HirModule,
    semantic_facts: RuntimeExecutableSemanticFactView<'hir>,
    locals: &'hir BTreeMap<LocalId, RuntimeLocalSeedId>,
}

impl<'hir> FinalPatternLowerer<'hir> {
    pub(crate) const fn new(
        module: &'hir HirModule,
        facts: &'hir RuntimePlanSemanticFacts,
        locals: &'hir BTreeMap<LocalId, RuntimeLocalSeedId>,
    ) -> Self {
        Self {
            module,
            semantic_facts: RuntimeExecutableSemanticFactView::global(facts),
            locals,
        }
    }

    pub(crate) fn with_semantic_facts(
        mut self,
        semantic_facts: RuntimeExecutableSemanticFactView<'hir>,
    ) -> Self {
        self.semantic_facts = semantic_facts;
        self
    }

    pub(crate) fn with_project_semantics(
        self,
        semantics: &'hir RuntimeProjectFunctionInstanceSemanticFacts,
    ) -> Self {
        self.with_semantic_facts(RuntimeExecutableSemanticFactView::project_instance(
            semantics,
        ))
    }

    fn pattern_literal(&self, id: PatternId) -> Option<&RuntimeValue> {
        self.semantic_facts.pattern_literal(id)
    }

    fn pattern_item(&self, id: PatternId) -> Option<&RuntimeProjectItem> {
        self.semantic_facts.pattern_item(id)
    }

    fn pattern_nominal_record(&self, id: PatternId) -> Option<&RuntimeRecordPatternFact> {
        self.semantic_facts.pattern_nominal_record(id)
    }

    fn pattern_variant(&self, id: PatternId) -> Option<&RuntimeResolvedVariant> {
        self.semantic_facts.pattern_variant(id)
    }

    fn pattern_type_ref(
        &self,
        id: PatternId,
    ) -> Option<&crate::semantic_facts::RuntimeNormalizedType> {
        self.semantic_facts.pattern_type(id)
    }

    fn local_type_ref(&self, id: LocalId) -> Option<&crate::semantic_facts::RuntimeNormalizedType> {
        self.semantic_facts.local_type(id)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one closed final-HIR pattern projection"
    )]
    pub(crate) fn lower(&self, id: PatternId) -> Result<RuntimePatternSeed, String> {
        let pattern = self
            .module
            .resolve_pattern(id)
            .map_err(|error| format!("cannot resolve final-HIR pattern {id:?}: {error}"))?;
        if pattern.is_poisoned() {
            return Err(format!(
                "final-HIR pattern {id:?} contains recovery and is not executable"
            ));
        }
        let kind = match pattern.kind() {
            HirPatternKind::Binding(binding) => self.binding(binding, false)?,
            HirPatternKind::MutableBinding(binding) => self.binding(binding, true)?,
            HirPatternKind::Literal(_) => RuntimePatternSeedKind::Literal(
                self.pattern_literal(id)
                    .cloned()
                    .ok_or_else(|| format!("checked literal fact is missing for pattern {id:?}"))?,
            ),
            HirPatternKind::EntityReference(_) => RuntimePatternSeedKind::Entity(
                project_entity_reference(self.pattern_item(id).ok_or_else(|| {
                    format!("checked project-item fact is missing for entity pattern {id:?}")
                })?),
            ),
            HirPatternKind::Variant(variant) => {
                let selected = self
                    .pattern_variant(id)
                    .ok_or_else(|| format!("checked variant fact is missing for pattern {id:?}"))?;
                let payload = match (
                    variant.payload(),
                    selected
                        .selected_payload_type()
                        .map_err(|error| error.to_string())?,
                ) {
                    (HirVariantPatternPayload::Absent, None) => None,
                    (HirVariantPatternPayload::Pattern(payload), Some(expected)) => {
                        let payload_type = self.pattern_type_ref(*payload);
                        if payload_type != Some(expected) {
                            return Err(format!(
                                "variant payload pattern {payload:?} does not match its selected normalized payload type"
                            ));
                        }
                        Some(Box::new(self.lower(*payload)?))
                    }
                    (HirVariantPatternPayload::Recovered { .. }, _) => {
                        return Err(format!("variant pattern {id:?} has a recovered payload"));
                    }
                    _ => {
                        return Err(format!(
                            "variant pattern at {id:?} has incompatible payload presence"
                        ));
                    }
                };
                RuntimePatternSeedKind::Variant {
                    ordinal: selected
                        .checked_selection()
                        .map_err(|error| error.to_string())?
                        .ordinal(),
                    payload,
                }
            }
            HirPatternKind::Discard => RuntimePatternSeedKind::Discard,
            HirPatternKind::Tuple { elements } => RuntimePatternSeedKind::Tuple(
                elements
                    .iter()
                    .map(|element| self.lower(*element))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            HirPatternKind::Record { .. } => {
                let record = self
                    .pattern_nominal_record(id)
                    .ok_or_else(|| format!("checked nominal fact is missing for pattern {id:?}"))?;
                let lowered = record
                    .fields()
                    .iter()
                    .map(|field| {
                        let pattern = match field.source() {
                            RuntimeRecordPatternSource::Pattern(pattern) => self.lower(pattern)?,
                            RuntimeRecordPatternSource::Binding(local) => RuntimePatternSeed::new(
                                self.local_type(local)?,
                                RuntimePatternSeedKind::Bind {
                                    mutable: false,
                                    local: self.local(local)?,
                                },
                            ),
                        };
                        Ok(RuntimeRecordPatternFieldSeed::new(
                            RuntimeRecordFieldSeedId::from_zero_based(field.field().zero_based()),
                            pattern,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let rest = match record.rest() {
                    RuntimeRecordPatternRest::Absent => RuntimePatternRestSeed::Exact,
                    RuntimeRecordPatternRest::Ignore => RuntimePatternRestSeed::Ignore,
                    RuntimeRecordPatternRest::Binding(local) => {
                        RuntimePatternRestSeed::Bind(self.local(local)?)
                    }
                };
                RuntimePatternSeedKind::Record {
                    fields: lowered.into_boxed_slice(),
                    rest,
                }
            }
            HirPatternKind::BracketSequence { elements, rest } => {
                RuntimePatternSeedKind::Sequence {
                    items: elements
                        .iter()
                        .map(|element| self.lower(*element))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                    rest: match rest {
                        HirPatternSequenceRest::Absent => RuntimePatternRestSeed::Exact,
                        HirPatternSequenceRest::Unbound => RuntimePatternRestSeed::Ignore,
                        HirPatternSequenceRest::Bound(local) => {
                            RuntimePatternRestSeed::Bind(self.local(*local)?)
                        }
                        HirPatternSequenceRest::Recovered(_) => {
                            return Err(format!(
                                "bracket-sequence pattern {id:?} has a recovered rest"
                            ));
                        }
                    },
                }
            }
            HirPatternKind::WholeBinding { binding, pattern } => RuntimePatternSeedKind::Whole {
                local: self.bound_local(binding)?,
                pattern: Box::new(self.lower(*pattern)?),
            },
            HirPatternKind::TypedBinding { binding, .. } => RuntimePatternSeedKind::Typed {
                local: self.bound_local(binding)?,
            },
            HirPatternKind::Or { .. } => {
                return Err(format!(
                    "or-pattern {id:?} must be expanded before runtime-plan lowering"
                ));
            }
            HirPatternKind::Error(_) => {
                return Err(format!("error pattern {id:?} is not executable"));
            }
        };
        Ok(RuntimePatternSeed::new(self.pattern_type(id)?, kind))
    }

    fn binding(
        &self,
        binding: &HirPatternBinding,
        mutable: bool,
    ) -> Result<RuntimePatternSeedKind, String> {
        Ok(RuntimePatternSeedKind::Bind {
            mutable,
            local: self.bound_local(binding)?,
        })
    }

    fn bound_local(&self, binding: &HirPatternBinding) -> Result<RuntimeLocalSeedId, String> {
        let HirPatternBinding::Bound { local, .. } = binding else {
            return Err("recovered pattern binding cannot enter runtime lowering".to_owned());
        };
        self.local(*local)
    }

    fn pattern_type(
        &self,
        pattern: PatternId,
    ) -> Result<arcweft_core::pattern::RuntimeSemanticTypeId, String> {
        self.pattern_type_ref(pattern)
            .map(crate::semantic_facts::RuntimeNormalizedType::identity)
            .ok_or_else(|| format!("accepted type is missing for pattern {pattern:?}"))
    }

    fn local_type(
        &self,
        local: LocalId,
    ) -> Result<arcweft_core::pattern::RuntimeSemanticTypeId, String> {
        self.local_type_ref(local)
            .map(crate::semantic_facts::RuntimeNormalizedType::identity)
            .ok_or_else(|| format!("accepted type is missing for local {local:?}"))
    }

    fn local(&self, local: LocalId) -> Result<RuntimeLocalSeedId, String> {
        self.locals.get(&local).cloned().ok_or_else(|| {
            format!("runtime local seed handle is missing for accepted local {local:?}")
        })
    }
}

pub(crate) fn project_entity_reference(item: &RuntimeProjectItem) -> RuntimeEntityReference {
    RuntimeEntityReference::Project {
        family: item.family(),
        public_id: item.public_id().clone(),
    }
}
