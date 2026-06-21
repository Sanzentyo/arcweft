use super::{
    AgentIntrinsic, CallableLowering, CallableSymbol, EffectCapability, EntityKind, FunctionParam,
    FunctionSignature, MapKind, QualifiedName, TypeCheckEnv, TypeKind,
};
use std::collections::BTreeMap;

pub fn agent_prelude_env() -> TypeCheckEnv {
    let mut env = TypeCheckEnv::standard();
    for (name, callable) in agent_prelude_callables() {
        env = env
            .with_function_signature(name.as_str(), callable.signature.clone())
            .with_function_effects(name.as_str(), callable.effects.clone());
    }
    env.with_method_signature(
        TypeKind::Probe(Box::new(TypeKind::Bool)),
        "eq",
        FunctionSignature::new(
            TypeKind::Predicate,
            [FunctionParam::required("expected", TypeKind::Bool)],
        ),
    )
    .with_method_signature(
        TypeKind::Named("Diagnostics".to_owned()),
        "has_error",
        FunctionSignature::return_only(TypeKind::Predicate),
    )
    .with_method_signature(
        TypeKind::RagContextPack,
        "summary",
        FunctionSignature::return_only(TypeKind::DisplayText),
    )
}

pub(super) fn agent_prelude_callables() -> BTreeMap<QualifiedName, CallableSymbol> {
    agent_observation_callables()
        .into_iter()
        .chain(agent_probe_callables())
        .chain(agent_predicate_callables())
        .chain(agent_action_callables())
        .chain(agent_capture_callables())
        .chain(agent_resource_callables())
        .chain(agent_record_callables())
        .chain(agent_rag_callables())
        .map(|(name, callable)| (QualifiedName::new(name), callable))
        .collect()
}

fn agent_observation_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "observe",
            CallableSymbol::new(
                FunctionSignature::new(agent_result(TypeKind::Observation), []),
                [EffectCapability::new("agent.observe")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Observe),
            ),
        ),
        (
            "expect",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [
                        FunctionParam::required("condition", TypeKind::Bool),
                        FunctionParam::required("message", TypeKind::String),
                    ],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Expect),
            ),
        ),
        (
            "deny",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [
                        FunctionParam::required("condition", TypeKind::Bool),
                        FunctionParam::required("message", TypeKind::String),
                    ],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Deny),
            ),
        ),
        (
            "wait",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Result {
                        ok: Box::new(TypeKind::Observation),
                        error: Box::new(TypeKind::Named("WaitError".to_owned())),
                    },
                    [
                        FunctionParam::required("predicate", TypeKind::Predicate),
                        FunctionParam::required("timeout", TypeKind::Duration),
                    ],
                ),
                [
                    EffectCapability::new("agent.wait"),
                    EffectCapability::new("agent.observe"),
                ],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Wait),
            ),
        ),
    ]
}

fn agent_probe_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "signal",
            CallableSymbol::new(
                FunctionSignature::return_only(TypeKind::Probe(Box::new(TypeKind::Named(
                    "_".to_owned(),
                )))),
                [EffectCapability::new("agent.observe")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::SignalProbe),
            ),
        ),
        (
            "metric",
            CallableSymbol::new(
                FunctionSignature::return_only(TypeKind::Probe(Box::new(TypeKind::Named(
                    "_".to_owned(),
                )))),
                [EffectCapability::new("agent.observe")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::MetricProbe),
            ),
        ),
        (
            "state_path",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::DebugStatePath,
                    [FunctionParam::required("path", TypeKind::String)],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::DebugStatePath),
            ),
        ),
        (
            "observation_path",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::ObservationFieldPath,
                    [FunctionParam::required("path", TypeKind::String)],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::ObservationFieldPath),
            ),
        ),
        (
            "state",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Probe(Box::new(TypeKind::AgentValue)),
                    [FunctionParam::required("path", TypeKind::DebugStatePath)],
                ),
                [EffectCapability::new("debug.read")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::StateProbe),
            ),
        ),
        (
            "observation",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Probe(Box::new(TypeKind::AgentValue)),
                    [FunctionParam::required(
                        "path",
                        TypeKind::ObservationFieldPath,
                    )],
                ),
                [EffectCapability::new("agent.observe")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::ObservationProbe),
            ),
        ),
        (
            "entity_meta",
            CallableSymbol::new(
                FunctionSignature::return_only(agent_result(TypeKind::AgentEntityMetadata)),
                [EffectCapability::new("debug.read")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::EntityMetadata),
            ),
        ),
        (
            "project_neighbors",
            CallableSymbol::new(
                FunctionSignature::return_only(agent_result(
                    TypeKind::AgentProjectGraphNeighborhood,
                )),
                [EffectCapability::new("debug.read")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::ProjectGraphNeighborhood),
            ),
        ),
        (
            "diagnostics",
            CallableSymbol::new(
                FunctionSignature::return_only(TypeKind::Named("Diagnostics".to_owned())),
                [EffectCapability::new("agent.observe")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Diagnostics),
            ),
        ),
    ]
}

fn agent_predicate_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "exists",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Predicate,
                    [FunctionParam::required(
                        "probe",
                        TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned()))),
                    )],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PredicateExists),
            ),
        ),
        (
            "action_enabled",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Predicate,
                    [FunctionParam::required("target", TypeKind::ActionTarget)],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PredicateActionEnabled),
            ),
        ),
        (
            "all",
            CallableSymbol::new(
                FunctionSignature::return_only(TypeKind::Predicate),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PredicateAll),
            ),
        ),
        (
            "any",
            CallableSymbol::new(
                FunctionSignature::return_only(TypeKind::Predicate),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PredicateAny),
            ),
        ),
        (
            "not",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Predicate,
                    [FunctionParam::required("predicate", TypeKind::Predicate)],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PredicateNot),
            ),
        ),
    ]
}

fn agent_action_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "choice_action",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::ActionTarget,
                    [FunctionParam::required(
                        "choice",
                        TypeKind::entity_ref(EntityKind::ChoiceOption),
                    )],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::ChoiceAction),
            ),
        ),
        (
            "viewport_point",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Named("ViewportPoint".to_owned()),
                    [
                        FunctionParam::required("x", TypeKind::U32),
                        FunctionParam::required("y", TypeKind::U32),
                    ],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::ViewportPoint),
            ),
        ),
        (
            "pointer.click",
            CallableSymbol::new(
                FunctionSignature::new(
                    agent_result(TypeKind::ActionResult),
                    [
                        FunctionParam::required(
                            "point",
                            TypeKind::Named("ViewportPoint".to_owned()),
                        ),
                        FunctionParam::defaulted("button", TypeKind::ActionName),
                    ],
                ),
                [EffectCapability::new("agent.act.physical")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PointerClick),
            ),
        ),
        (
            "advance_text",
            CallableSymbol::new(
                FunctionSignature::new(agent_result(TypeKind::ActionResult), []),
                [EffectCapability::new("agent.act.semantic")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::AdvanceText),
            ),
        ),
        (
            "choose",
            CallableSymbol::new(
                FunctionSignature::new(
                    agent_result(TypeKind::ActionResult),
                    [FunctionParam::required(
                        "choice",
                        TypeKind::entity_ref(EntityKind::ChoiceOption),
                    )],
                ),
                [EffectCapability::new("agent.act.semantic")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Choose),
            ),
        ),
        (
            "invoke",
            CallableSymbol::new(
                FunctionSignature::new(
                    agent_result(TypeKind::ActionResult),
                    [
                        FunctionParam::required(
                            "target",
                            TypeKind::entity_ref(EntityKind::Other("_".to_owned())),
                        ),
                        FunctionParam::required("action", TypeKind::ActionName),
                        FunctionParam::required(
                            "args",
                            TypeKind::Map {
                                kind: MapKind::Sorted,
                                key: Box::new(TypeKind::String),
                                value: Box::new(TypeKind::AgentValue),
                            },
                        ),
                    ],
                ),
                [EffectCapability::new("agent.act.semantic")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Invoke),
            ),
        ),
    ]
}

fn agent_capture_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "capture",
            CallableSymbol::new(
                FunctionSignature::new(
                    agent_result(TypeKind::CaptureRef),
                    [FunctionParam::required("target", TypeKind::CaptureTarget)],
                ),
                [EffectCapability::new("agent.capture")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Capture),
            ),
        ),
        (
            "viewport",
            CallableSymbol::new(
                FunctionSignature::new(TypeKind::CaptureTarget, []),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Capture),
            ),
        ),
        (
            "layer",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::CaptureTarget,
                    [FunctionParam::required(
                        "target",
                        TypeKind::entity_ref(EntityKind::Layer),
                    )],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Capture),
            ),
        ),
        (
            "object",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::CaptureTarget,
                    [FunctionParam::required(
                        "id",
                        TypeKind::Named("ObservedObjectId".to_owned()),
                    )],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Capture),
            ),
        ),
    ]
}

fn agent_resource_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![(
        "read_resource",
        CallableSymbol::new(
            FunctionSignature::new(
                agent_result(TypeKind::AgentResource),
                [FunctionParam::required("uri", TypeKind::String)],
            ),
            [EffectCapability::new("agent.resource.read")],
            CallableLowering::AgentIntrinsic(AgentIntrinsic::ReadResource),
        ),
    )]
}

fn agent_record_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "attach",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "resource",
                        agent_attach_resource_type(),
                    )],
                ),
                [EffectCapability::new("debug.record")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Attach),
            ),
        ),
        (
            "checkpoint",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required("name", TypeKind::String)],
                ),
                [EffectCapability::new("debug.record")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Checkpoint),
            ),
        ),
        (
            "note",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required("text", TypeKind::DisplayText)],
                ),
                [EffectCapability::new("debug.record")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Note),
            ),
        ),
    ]
}

fn agent_attach_resource_type() -> TypeKind {
    TypeKind::Choice(vec![TypeKind::CaptureRef, TypeKind::AgentResource])
}

pub(super) fn agent_result(ok: TypeKind) -> TypeKind {
    TypeKind::Result {
        ok: Box::new(ok),
        error: Box::new(TypeKind::Named("AgentError".to_owned())),
    }
}

fn agent_rag_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![(
        "rag.query",
        CallableSymbol::new(
            FunctionSignature::new(
                TypeKind::Result {
                    ok: Box::new(TypeKind::RagContextPack),
                    error: Box::new(TypeKind::Named("RagError".to_owned())),
                },
                [FunctionParam::required("query", TypeKind::String)],
            ),
            [EffectCapability::new("rag.query")],
            CallableLowering::AgentIntrinsic(AgentIntrinsic::RagQuery),
        ),
    )]
}
