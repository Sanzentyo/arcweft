use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_compiler::{
    project::{
        AcceptedLaunchProfileInput, CompiledProject, ProjectCompilationContext,
        ProjectCompilationSession, ProjectCompileError, compile_project,
    },
    source::compile_source,
};
use arcweft_core::{
    effect::{RuntimeDropPolicyExpr, RuntimeDropPolicyKind, RuntimeEffectExpr},
    line_task::{LineTaskGroup, LineTaskNode, LineTaskTrigger},
    plan::FlowOp,
    runtime_id::{RuntimeDialogueMarkId, RuntimeLineTaskNodeId},
    time::LogicalDuration,
    value::{RuntimeExprKind, RuntimeValue},
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    incremental::{ParsedSource, SyntaxDatabase},
    parser::ParseOptions,
};
use arcweft_launch::{LaunchProfileSelection, ProfileId, accepted::SourceBackedManifest};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceSetRevision, identity::SourceSnapshotId,
};

#[test]
fn evaluated_effect_operands_reach_awbc_from_final_checked_sources() {
    let compiled = compile_source(
        r#"
flow main() -> Unit {
    log.info("started", detail = "final source")
    drop(.Cancel)([1i64]...)
    drop(.Stop(fade = 120ms))([1i64]...)
}

entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("typed evaluated effects compile from final call operands");

    let evaluated_effects = compiled
        .plan
        .flows()
        .iter()
        .flat_map(|flow| flow.ops.iter())
        .filter_map(|operation| match operation {
            FlowOp::EvaluatedEffect(effect) => Some(effect),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(evaluated_effects.len(), 3);
    assert!(matches!(
        evaluated_effects[0],
        RuntimeEffectExpr::Log { .. }
    ));
    let drop_policies = evaluated_effects
        .iter()
        .filter_map(|effect| match effect {
            RuntimeEffectExpr::Drop { policy, .. } => Some(policy),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(drop_policies.len(), 2);
    assert_eq!(drop_policies[0].kind(), RuntimeDropPolicyKind::Cancel);
    assert_eq!(drop_policies[1].kind(), RuntimeDropPolicyKind::Stop);

    let RuntimeEffectExpr::Drop { target, policy } = evaluated_effects[1] else {
        panic!("the first drop effect remains a typed Drop expression")
    };
    assert_eq!(target.kind(), &RuntimeExprKind::Value(RuntimeValue::i64(1)));
    assert!(matches!(policy, RuntimeDropPolicyExpr::Cancel));

    let RuntimeEffectExpr::Drop { target, policy } = evaluated_effects[2] else {
        panic!("the second drop effect remains a typed Drop expression")
    };
    assert_eq!(target.kind(), &RuntimeExprKind::Value(RuntimeValue::i64(1)));
    let RuntimeDropPolicyExpr::Stop { fade } = policy else {
        panic!("the second drop effect retains its typed Stop policy")
    };
    assert_eq!(
        fade.kind(),
        &RuntimeExprKind::Value(RuntimeValue::Duration(LogicalDuration::from_nanos(
            120_000_000
        ),))
    );

    AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "evaluated_effects.arcw",
    )
    .lower()
    .expect("typed evaluated effects lower to verified product AWBC");
}

#[test]
fn dialogue_content_and_delay_effects_reach_the_line_task_graph_and_awbc() {
    let compiled = compile_attached_dialogue_project(
        r#"
pub character @character.alice Alice as alice {}

flow main() -> Unit {
    alice(id = @say.shared):
        Hello [call log.info("content")] [at 120ms call=log.info("delay")]
}

entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("dialogue content and delay effects compile from final call operands");
    let runtime_plan = compiled.runtime_plan();
    let [content] = runtime_plan.plan.dialogue_content().rows() else {
        panic!("fixture publishes one dialogue content plan")
    };
    assert_eq!(content.effect_site_count().get(), 2);
    let group = runtime_plan
        .plan
        .line_task_groups()
        .get(
            content
                .line_task_group()
                .expect("dialogue effects publish a line-task group")
                .index(),
        )
        .expect("dialogue content references its exact line-task group");
    let (content_trigger, scheduled_trigger, logs) = count_line_task_effects(group, group.root());
    assert_eq!((content_trigger, scheduled_trigger, logs), (1, 1, 2));

    AwbcLowerer::new(
        &runtime_plan.plan,
        &runtime_plan.dialogue_content_catalog,
        "dialogue_evaluated_effects.arcw",
    )
    .lower()
    .expect("dialogue content and delay effects lower to verified product AWBC");
}

#[test]
fn dialogue_mark_projection_is_content_ordered_and_uses_the_exact_checked_trigger() {
    let compiled = compile_attached_dialogue_project(
        r#"
pub character @character.alice Alice as alice {}

flow main() -> Unit {
    alice[before [mark @.first] middle [mark @.second] after] with {
        on mark(@.first) => return ()
        on mark(@.second) => return ()
    }
}

entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("source-ordered dialogue markers compile");

    let plan = &compiled.runtime_plan().plan;
    let [content] = plan.dialogue_content().rows() else {
        panic!("one dialogue content plan");
    };
    assert_eq!(
        content
            .marks()
            .iter()
            .map(|mark| (mark.id().index(), mark.label()))
            .collect::<Vec<_>>(),
        vec![(0, "first"), (1, "second")]
    );

    let group_id = content
        .line_task_group()
        .expect("marker handlers publish one line-task group");
    let group = plan
        .line_task_groups()
        .get(group_id.index())
        .expect("content line-task group");
    let mut triggers = Vec::new();
    collect_mark_triggers(group, group.root(), &mut triggers);
    assert_eq!(
        triggers,
        [
            RuntimeDialogueMarkId::from_zero_based(0).expect("first runtime mark"),
            RuntimeDialogueMarkId::from_zero_based(1).expect("second runtime mark"),
        ]
    );

    let [first, second] = content.marks() else {
        panic!("content mark inventory");
    };
    assert_eq!(first.id(), triggers[0]);
    assert_eq!(second.id(), triggers[1]);
}

#[test]
fn dialogue_mark_projection_keeps_equal_local_names_content_qualified() {
    let compiled = compile_attached_dialogue_project(
        r#"
pub character @character.alice Alice as alice {}

flow main() -> Unit {
    alice[first [mark @.same] end] with {
        on mark(@.same) => return ()
    }

    alice[second [mark @.same] end] with {
        on mark(@.same) => return ()
    }
}

entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("equal local marker names in distinct applications compile");

    let contents = compiled.runtime_plan().plan.dialogue_content().rows();
    assert_eq!(contents.len(), 2);
    assert!(contents.iter().all(|content| {
        content.marks().len() == 1
            && content.marks()[0].id() == RuntimeDialogueMarkId::from_zero_based(0).unwrap()
            && content.marks()[0].label() == "same"
    }));
    let groups = contents
        .iter()
        .map(|content| {
            let group_id = content.line_task_group().expect("marker line-task group");
            compiled
                .runtime_plan()
                .plan
                .line_task_groups()
                .get(group_id.index())
                .expect("content line-task group")
        })
        .collect::<Vec<_>>();
    let triggers = groups
        .into_iter()
        .map(|group| {
            let mut marks = Vec::new();
            collect_mark_triggers(group, group.root(), &mut marks);
            marks
        })
        .collect::<Vec<_>>();
    assert_eq!(
        triggers,
        vec![vec![RuntimeDialogueMarkId::from_zero_based(0).unwrap()]; 2]
    );
}

fn collect_mark_triggers(
    group: &LineTaskGroup,
    node_id: RuntimeLineTaskNodeId,
    output: &mut Vec<RuntimeDialogueMarkId>,
) {
    let node = group.node(node_id).expect("sealed line-task node");
    match node {
        LineTaskNode::Sequence(children)
        | LineTaskNode::Start(children)
        | LineTaskNode::Parallel { children, .. } => {
            for child in children {
                collect_mark_triggers(group, *child, output);
            }
        }
        LineTaskNode::Child { trigger, scope, .. } => {
            if let LineTaskTrigger::Mark(mark) = trigger {
                output.push(*mark);
            }
            collect_mark_triggers(group, *scope, output);
        }
        LineTaskNode::Action(_) => {}
    }
}

fn count_line_task_effects(
    group: &LineTaskGroup,
    node_id: RuntimeLineTaskNodeId,
) -> (usize, usize, usize) {
    let node = group
        .node(node_id)
        .expect("line-task child scope references a sealed node");
    match node {
        LineTaskNode::Sequence(children) | LineTaskNode::Start(children) => children
            .iter()
            .map(|child| count_line_task_effects(group, *child))
            .fold((0, 0, 0), sum_line_task_effect_counts),
        LineTaskNode::Parallel { children, .. } => children
            .iter()
            .map(|child| count_line_task_effects(group, *child))
            .fold((0, 0, 0), sum_line_task_effect_counts),
        LineTaskNode::Child { trigger, scope, .. } => {
            let (content, scheduled, logs) = count_line_task_effects(group, *scope);
            match trigger {
                LineTaskTrigger::ContentEffect(_) => (content + 1, scheduled, logs),
                LineTaskTrigger::Scheduled(_) => (content, scheduled + 1, logs),
                LineTaskTrigger::Immediate | LineTaskTrigger::Mark(_) => (content, scheduled, logs),
            }
        }
        LineTaskNode::Action(operations) => (
            0,
            0,
            operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        FlowOp::EvaluatedEffect(RuntimeEffectExpr::Log { .. })
                    )
                })
                .count(),
        ),
    }
}

fn sum_line_task_effect_counts(
    (content_left, scheduled_left, logs_left): (usize, usize, usize),
    (content_right, scheduled_right, logs_right): (usize, usize, usize),
) -> (usize, usize, usize) {
    (
        content_left + content_right,
        scheduled_left + scheduled_right,
        logs_left + logs_right,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "the end-to-end fixture keeps one explicit source-to-runtime publication pipeline"
)]
fn compile_attached_dialogue_project(source: &str) -> Result<CompiledProject, ProjectCompileError> {
    let source_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://evaluated-effects/source")
                .expect("dialogue source document ID"),
            SourceName::path("src/main.arcw"),
            source,
        )
        .expect("dialogue source document"),
    );
    let manifest_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://evaluated-effects/manifest")
                .expect("dialogue manifest document ID"),
            SourceName::path("arcw.toml"),
            "schema = 1\n\
[package]\n\
id = \"local.arcweft.evaluated-effects\"\n\
version = \"0.0.0\"\n\
\n\
[profiles.dev]\n\
kind = \"game\"\n\
source = \"src/main.arcw\"\n\
\n\
[profiles.dev.localization.character_names]\n\
active = \"ja-JP\"\n\
fallbacks = []\n",
        )
        .expect("dialogue manifest document"),
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("local.arcweft.evaluated-effects").expect("package ID"),
            version: PackageVersion::new("0.0.0").expect("package version"),
        },
        BuildSpec::default(),
        Arc::clone(&manifest_document),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            PathBuf::from("src/main.arcw"),
            Arc::clone(&source_document),
            [],
        )],
    )
    .expect("dialogue project sources");
    let package =
        CallablePackageId::try_new(project.package().id.as_str()).expect("callable package ID");
    let world = ProjectSymbolWorldId::try_new(
        package,
        source_document.identity().id().clone(),
        "evaluated-effects-test",
    )
    .expect("dialogue symbol world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&source_document), Arc::clone(&manifest_document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("dialogue registration facts");
    let resource_types = Arc::new(ResourceTypeRegistry::empty());
    let accepted = Arc::new(
        SourceBackedManifest::decode(Arc::clone(&manifest_document))
            .expect("accepted dialogue manifest"),
    );
    let profile_id = ProfileId::new("dev").expect("dialogue profile ID");
    let resolved = accepted
        .resolve_profile(LaunchProfileSelection::Explicit(profile_id.as_str()))
        .expect("resolved dialogue profile");
    let topology_revision = SourceSetRevision::try_for_identities([
        manifest_document.identity(),
        source_document.identity(),
    ])
    .expect("dialogue topology revision");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::clone(&resource_types),
        None,
        None,
    )
    .with_accepted_launch_profile(AcceptedLaunchProfileInput::new(
        accepted,
        profile_id,
        resolved,
        topology_revision,
        resource_types,
    ));
    let mut syntax = SyntaxDatabase::try_new().expect("dialogue syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(source_document.display_name().clone()),
            Arc::clone(&source_document),
            ParseOptions::default(),
        )
        .expect("dialogue attached source");
    let parsed_sources = BTreeMap::<CanonicalModulePath, ParsedSource>::from([(
        CanonicalModulePath::crate_root(),
        parsed,
    )]);
    let mut session = ProjectCompilationSession::try_new().expect("dialogue HIR database");
    compile_project(&mut session, &project, &parsed_sources, &context)
}
