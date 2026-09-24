//! Normalizes the domain-issued payload graph without inventing source types.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_core::plan::{
    MAX_RUNTIME_PLAN_TYPE_DEPTH, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use arcweft_dialogue::CharacterDialogueRolePayloadSchema;

use super::super::{
    RuntimeNormalizedType, RuntimeSemanticFactsError, RuntimeSemanticTypeId, RuntimeTypeShape,
};

impl RuntimeNormalizedType {
    /// Projects the complete private schema supplied by a bound dialogue role.
    /// Source nominal declarations are not reconstructed from private IDs.
    pub fn try_from_character_dialogue_payload(
        schema: &CharacterDialogueRolePayloadSchema,
    ) -> Result<Self, RuntimeSemanticFactsError> {
        let mut seeds = BTreeMap::new();
        for seed in schema.types() {
            if seeds.insert(seed.semantic_identity(), seed).is_some() {
                return Err(invalid("a role payload schema repeats a semantic identity"));
            }
        }
        let mut projection = PayloadProjection {
            seeds,
            active: BTreeSet::new(),
            completed: BTreeMap::new(),
        };
        let root = projection.project(schema.root(), 0)?;
        if projection.completed.len() != projection.seeds.len() {
            return Err(invalid("a role payload schema contains an unrooted type"));
        }
        Ok(root)
    }
}

struct PayloadProjection<'a> {
    seeds: BTreeMap<RuntimeSemanticTypeId, &'a RuntimePlanTypeSeed>,
    active: BTreeSet<RuntimeSemanticTypeId>,
    completed: BTreeMap<RuntimeSemanticTypeId, RuntimeNormalizedType>,
}

impl PayloadProjection<'_> {
    fn project(
        &mut self,
        identity: RuntimeSemanticTypeId,
        depth: usize,
    ) -> Result<RuntimeNormalizedType, RuntimeSemanticFactsError> {
        if depth >= MAX_RUNTIME_PLAN_TYPE_DEPTH {
            return Err(invalid(
                "a role payload schema exceeds the runtime type depth limit",
            ));
        }
        if let Some(ty) = self.completed.get(&identity) {
            return Ok(ty.clone());
        }
        if !self.active.insert(identity) {
            return Err(invalid(
                "a role payload schema contains a recursive structural type",
            ));
        }
        let seed = self
            .seeds
            .get(&identity)
            .ok_or_else(|| invalid("a role payload schema is missing a referenced type"))?;
        let projection = seed
            .projection()
            .clone()
            .try_map(&mut |child| self.project(child, depth + 1))?;
        let shape = match projection {
            RuntimePlanTypeProjection::Never => RuntimeTypeShape::Never,
            RuntimePlanTypeProjection::Unit => RuntimeTypeShape::Unit,
            RuntimePlanTypeProjection::Bool => RuntimeTypeShape::Bool,
            RuntimePlanTypeProjection::Signed(width) => RuntimeTypeShape::Signed(width),
            RuntimePlanTypeProjection::Unsigned(width) => RuntimeTypeShape::Unsigned(width),
            RuntimePlanTypeProjection::F32 => RuntimeTypeShape::F32,
            RuntimePlanTypeProjection::F64 => RuntimeTypeShape::F64,
            RuntimePlanTypeProjection::String => RuntimeTypeShape::String,
            RuntimePlanTypeProjection::Char => RuntimeTypeShape::Char,
            RuntimePlanTypeProjection::Bytes => RuntimeTypeShape::Bytes,
            RuntimePlanTypeProjection::Duration => RuntimeTypeShape::Duration,
            RuntimePlanTypeProjection::Progress => RuntimeTypeShape::Progress,
            RuntimePlanTypeProjection::Tuple(items) => RuntimeTypeShape::Tuple(items),
            RuntimePlanTypeProjection::Choice(items) => RuntimeTypeShape::Choice(items),
            RuntimePlanTypeProjection::Option { item, some_payload } => RuntimeTypeShape::Option {
                item: Box::new(item),
                some_payload: Box::new(some_payload),
            },
            _ => {
                return Err(invalid(
                    "a role payload schema has a shape outside the domain's structural value algebra",
                ));
            }
        };
        let ty = RuntimeNormalizedType::new(identity, shape);
        self.active.remove(&identity);
        self.completed.insert(identity, ty.clone());
        Ok(ty)
    }
}

fn invalid(reason: &'static str) -> RuntimeSemanticFactsError {
    RuntimeSemanticFactsError::InvalidCharacterDialogueGeneration { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_dialogue::CharacterDialogueRolePayloadCodec;

    #[test]
    fn rich_text_payload_projection_preserves_every_private_type_and_some_payload() {
        let schema = CharacterDialogueRolePayloadCodec::RichTextProperties
            .payload_schema()
            .unwrap();
        let normalized =
            RuntimeNormalizedType::try_from_character_dialogue_payload(schema).unwrap();
        assert_eq!(normalized.identity(), schema.root());
        let mut projected = Vec::new();
        normalized
            .append_runtime_plan_type_seeds(&mut projected)
            .unwrap();
        let projected = projected
            .into_iter()
            .map(|seed| (seed.semantic_identity(), seed))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(projected.len(), schema.types().len());
        for seed in schema.types() {
            assert_eq!(&projected[&seed.semantic_identity()], seed);
            if let RuntimePlanTypeProjection::Option { item, some_payload } = seed.projection() {
                assert!(matches!(
                    projected[some_payload].projection(),
                    RuntimePlanTypeProjection::Tuple(items) if items.as_ref() == [*item]
                ));
            }
        }
    }
}
