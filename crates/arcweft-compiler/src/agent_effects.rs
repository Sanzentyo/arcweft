use arcweft_agent_protocol::artifact::EffectCapability as AgentEffectCapability;
use arcweft_lang_hir::model::HirAgent;
use arcweft_lang_sema::types::EntityKind;
use arcweft_lang_syntax::ast::flow::ContractClause;
use arcweft_lang_syntax::expr::{CallArg, Expr};

pub(crate) fn declared_agent_effects(agent: &HirAgent) -> Vec<AgentEffectCapability> {
    let mut effects = agent
        .item()
        .contracts()
        .iter()
        .filter_map(|contract| match contract {
            ContractClause::Effects(effects) => Some(effects),
            _ => None,
        })
        .flat_map(|effects| effects.iter().filter_map(effect_label))
        .map(AgentEffectCapability::new)
        .collect::<Vec<_>>();
    effects.sort();
    effects.dedup();
    effects
}

fn effect_label(expr: &Expr) -> Option<String> {
    if let Expr::Call { callee, args } = expr
        && effect_path_label(callee).as_deref() == Some("state.write")
    {
        return state_write_effect_label(args);
    }
    if let Expr::MethodCall {
        receiver,
        method,
        args,
    } = expr
        && method == "write"
        && effect_path_label(receiver).as_deref() == Some("state")
    {
        return state_write_effect_label(args);
    }
    match expr {
        Expr::MethodCall {
            receiver, method, ..
        } => effect_path_label(receiver).map(|receiver| format!("{receiver}.{method}")),
        Expr::Call { callee, .. } => effect_label(callee),
        _ => effect_path_label(expr),
    }
}

fn effect_path_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => Some(path.clone()),
        Expr::Field { target, field } => {
            effect_path_label(target).map(|target| format!("{target}.{field}"))
        }
        _ => None,
    }
}

fn state_write_effect_label(args: &[CallArg]) -> Option<String> {
    args.first().and_then(|arg| match arg.value() {
        Expr::LifetimePath { key, .. } => Some(format!("state.write({})", key.scope().as_str())),
        Expr::Path(path) => path
            .strip_prefix('\'')
            .map(|scope| format!("state.write({scope})")),
        _ => None,
    })
}

pub(crate) fn entity_kind_label(kind: &EntityKind) -> &str {
    match kind {
        EntityKind::Agent => "agent",
        EntityKind::Entry => "entry",
        EntityKind::Flow => "flow",
        EntityKind::Fragment => "fragment",
        EntityKind::Choice => "choice",
        EntityKind::ChoiceOption => "choice_option",
        EntityKind::Character => "character",
        EntityKind::Component => "component",
        EntityKind::Activity => "activity",
        EntityKind::Textbox => "textbox",
        EntityKind::DialogueLine => "dialogue_line",
        EntityKind::Text => "text",
        EntityKind::Asset => "asset",
        EntityKind::Image => "image",
        EntityKind::Animation => "animation",
        EntityKind::Capture => "capture",
        EntityKind::Hook => "hook",
        EntityKind::Signal => "signal",
        EntityKind::Metric => "metric",
        EntityKind::Scene => "scene",
        EntityKind::Source => "source",
        EntityKind::Test => "test",
        EntityKind::Bench => "bench",
        EntityKind::Layer => "layer",
        EntityKind::Voice => "voice",
        EntityKind::Se => "se",
        EntityKind::Bgm => "bgm",
        EntityKind::AudioBus => "audio_bus",
        EntityKind::MixerSnapshot => "mixer_snapshot",
        EntityKind::Ducking => "ducking",
        EntityKind::Motion => "motion",
        EntityKind::Rig => "rig",
        EntityKind::Slot => "slot",
        EntityKind::Target => "target",
        EntityKind::Other(value) => value.as_str(),
    }
}
