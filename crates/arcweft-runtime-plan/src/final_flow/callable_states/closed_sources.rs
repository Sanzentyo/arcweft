//! Closed state identity includes the complete body substitution and producing
//! root, even when either contains types erased from the remaining arrow.

use super::*;
use arcweft_lang_sema::callable::{CallableGroupIndex, CallableParameterCoordinate};

type ClosedLayout = (
    RuntimeProjectCallableSourceKey,
    RuntimeProjectFunctionInstanceKey,
    CallableParameterCoordinate,
);
pub(super) type ClosedCallableStates = BTreeMap<ClosedLayout, RuntimeCallableStateSeedId>;

pub(super) fn materialize<'a>(
    facts: &'a RuntimePlanSemanticFacts,
    instances: &ProjectCallableStates,
    sources: &mut ProjectCallableSourceStates,
    sites: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    defaults: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    builder: &mut RuntimePlanBuilder,
) -> Result<ClosedCallableStates, RuntimePlanLowerError> {
    let error = |message: &str| RuntimePlanLowerError::new(message);
    let mut requests = BTreeMap::<ClosedLayout, &RuntimeNormalizedType>::new();
    let mut request = |root: &RuntimeProjectCallableSourceKey,
                       body: &RuntimeProjectFunctionInstanceKey,
                       position: CallableParameterCoordinate,
                       ty: &'a RuntimeNormalizedType| {
        let key = (root.clone(), body.clone(), position);
        match requests.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(ty);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(entry) if *entry.get() == ty => Ok(()),
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(error("closed callable position has conflicting types"))
            }
        }
    };
    for source in facts.callable_sources() {
        if let Some(body) = source.body() {
            request(
                source.root(),
                body,
                source.position(),
                source.function_type(),
            )?;
        }
    }
    for specialization in facts.callable_specializations() {
        for (source, target) in specialization.selections() {
            if let RuntimeProjectCallableValueTarget::Closed { instance, position } = target {
                let source = facts
                    .callable_source(source)
                    .ok_or_else(|| error("closed specialization source is absent"))?;
                request(source.root(), instance, *position, specialization.target())?;
            }
        }
    }
    let families = requests
        .keys()
        .map(|(root, body, _)| (root.clone(), body.clone()))
        .collect::<BTreeSet<_>>();
    let mut chains = BTreeMap::new();
    let mut states = BTreeMap::new();
    for (root, body) in families {
        let origin = sources
            .get(&root)
            .ok_or_else(|| error("closed callable root state is absent"))?;
        let instance = facts
            .project_function_instance(&body)
            .ok_or_else(|| error("closed callable body is absent"))?;
        let root_fact = facts
            .callable_source(&root)
            .ok_or_else(|| error("closed callable root fact is absent"))?;
        let chain = if root_fact.body() == Some(&body) {
            instances
                .get(&body)
                .cloned()
                .ok_or_else(|| error("closed root chain is absent"))?
        } else {
            materialize_instance_from(
                instance,
                RuntimeCallableParameterCoordinate {
                    group: 0,
                    parameter: 0,
                },
                instance.callable_type(),
                Some(origin.clone()),
                None,
                None,
                sites,
                defaults,
                builder,
            )?
        };
        for (group, state) in chain.iter().enumerate() {
            let group = CallableGroupIndex::try_from_usize(group)
                .map_err(|_| error("closed callable group exceeds coordinate"))?;
            states.insert(
                (
                    root.clone(),
                    body.clone(),
                    CallableParameterCoordinate::group_start(group),
                ),
                state.clone(),
            );
        }
        chains.insert((root, body), chain);
    }
    for ((root, body, position), ty) in requests {
        let key = (root.clone(), body.clone(), position);
        if states.contains_key(&key) {
            // Group starts use the declaration's exact closed arrow. The
            // partial constructor checks suffixes at nonzero coordinates.
            let instance = facts
                .project_function_instance(&body)
                .ok_or_else(|| error("closed callable body is absent"))?;
            let mut expected = instance.callable_type();
            for _ in 0..position.group().get() {
                let RuntimeTypeShape::Function { result, .. } = expected.shape() else {
                    return Err(error("closed callable group chain is incomplete"));
                };
                expected = result;
            }
            if expected != ty {
                return Err(error("closed group source differs from its body"));
            }
            continue;
        }
        let chain = chains
            .get(&(root.clone(), body.clone()))
            .ok_or_else(|| error("closed family is absent"))?;
        let origin = sources
            .get(&root)
            .ok_or_else(|| error("closed root state is absent"))?;
        let instance = facts
            .project_function_instance(&body)
            .ok_or_else(|| error("closed callable body is absent"))?;
        let following = chain
            .get(position.group().get() + 1..)
            .ok_or_else(|| error("closed following groups are absent"))?;
        let state = builder
            .reserve_callable_state_seed()
            .map_err(|failure| error(&failure.to_string()))?;
        materialize_instance_from(
            instance,
            RuntimeCallableParameterCoordinate {
                group: u32::try_from(position.group().get())
                    .map_err(|_| error("closed group exceeds coordinate"))?,
                parameter: u32::try_from(position.parameter().get())
                    .map_err(|_| error("closed parameter exceeds coordinate"))?,
            },
            ty,
            Some(origin.clone()),
            Some(state.clone()),
            Some(following),
            sites,
            defaults,
            builder,
        )?;
        states.insert(key, state);
    }
    for source in facts.callable_sources() {
        if let Some(body) = source.body() {
            let state = states
                .get(&(source.root().clone(), body.clone(), source.position()))
                .ok_or_else(|| error("closed source layout is absent"))?;
            sources.insert(source.key().clone(), state.clone());
        }
    }
    Ok(states)
}
