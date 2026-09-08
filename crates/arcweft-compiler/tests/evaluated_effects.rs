use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_compiler::{
    lower::{RuntimeEmissionMode, project_runtime_reachability, project_runtime_semantic_facts},
    project::{
        AcceptedLaunchProfileInput, CompiledProject, ProjectCompilationContext,
        ProjectCompilationSession, ProjectCompileError, compile_project,
    },
    source::compile_source,
};
use arcweft_core::{
    effect::{RuntimeDropPolicyExpr, RuntimeDropPolicyKind, RuntimeEffectExpr},
    line_task::{LineTaskGroup, LineTaskNode, LineTaskTrigger},
    plan::{
        FlowOp, RuntimeDialogueContentEffectTrigger, RuntimeDialogueContentPlan,
        RuntimeFunctionSiteBody, RuntimePlan,
    },
    runtime_id::{RuntimeDialogueMarkId, RuntimeLineTaskNodeId},
    time::LogicalDuration,
    value::{RuntimeExprKind, RuntimeValue},
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{
    env::TypeCheckEnv, final_analysis::CheckedExpressionResolution,
    registration::ProjectRegistrationFacts,
};
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
use arcweft_text_model::{
    RichTextNode, RichTextObjectProxy, RichTextStyle, RichTextTextProxyFieldKind,
    RichTextTextProxyScalar,
};

fn object_proxy(compiled: &CompiledProject) -> &RichTextObjectProxy {
    fn find_object_proxy(nodes: &[RichTextNode]) -> Option<&RichTextObjectProxy> {
        nodes.iter().find_map(|node| match node {
            RichTextNode::Scope { style, body } => match style.as_ref() {
                RichTextStyle::Object { proxy } => Some(proxy),
                _ => find_object_proxy(body),
            },
            RichTextNode::Ruby { body, .. } => find_object_proxy(body),
            _ => None,
        })
    }

    let [template] = compiled.runtime_plan().dialogue_content_catalog.templates() else {
        panic!("one dialogue content template")
    };
    find_object_proxy(&template.content().nodes).expect("compiler emits one object style")
}

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
fn dialogue_content_callbacks_and_delays_reach_their_exact_runtime_owners_and_awbc() {
    let compiled = compile_attached_dialogue_project(
        r#"
pub character alice { display = "Alice" }

flow main() -> Unit {
    alice[Hello [call log.info("content")] [at 120ms call=log.info("delay")]]
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
    assert_dialogue_effect_sites(&runtime_plan.plan, content, 2);
    let manifest = runtime_plan
        .plan
        .dialogue_content()
        .template(content.template())
        .expect("dialogue template manifest");
    assert!(matches!(
        manifest.effects()[0].trigger(),
        RuntimeDialogueContentEffectTrigger::Content
    ));
    assert!(matches!(
        manifest.effects()[1].trigger(),
        RuntimeDialogueContentEffectTrigger::Delay { duration }
            if duration == LogicalDuration::from_nanos(120_000_000)
    ));

    AwbcLowerer::new(
        &runtime_plan.plan,
        &runtime_plan.dialogue_content_catalog,
        "dialogue_evaluated_effects.arcw",
    )
    .lower()
    .expect("dialogue content and delay effects lower to verified product AWBC");
}

#[test]
fn nested_modifier_effects_use_the_body_content_effect_plan() {
    let compiled = compile_attached_dialogue_project(
        r#"
pub character alice { display = "Alice" }

flow main() -> Unit {
    alice[#strong()[nested [call log.info("modifier-content")] [at 120ms call=log.info("modifier-delay")]]]
}

entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("nested modifier effects compile from the body-local effect plan");
    let [content] = compiled.runtime_plan().plan.dialogue_content().rows() else {
        panic!("one dialogue content plan")
    };
    assert_dialogue_effect_sites(&compiled.runtime_plan().plan, content, 2);
    assert_eq!(
        dialogue_effect_triggers(&compiled.runtime_plan().plan, content),
        [
            RuntimeDialogueContentEffectTrigger::Content,
            RuntimeDialogueContentEffectTrigger::Delay {
                duration: LogicalDuration::from_nanos(120_000_000),
            },
        ]
    );
}

#[test]
fn nested_fx_effects_use_the_body_content_effect_plan() {
    let compiled = compile_attached_dialogue_project(
        r#"
pub character alice { display = "Alice" }

flow main() -> Unit {
    alice[#fx(shake(amplitude=1px))[nested [call log.info("fx-content")] [at 120ms call=log.info("fx-delay")]]]
}

entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("nested Fx effects compile from the body-local effect plan");
    let [content] = compiled.runtime_plan().plan.dialogue_content().rows() else {
        panic!("one dialogue content plan")
    };
    assert_dialogue_effect_sites(&compiled.runtime_plan().plan, content, 2);
    assert_eq!(
        dialogue_effect_triggers(&compiled.runtime_plan().plan, content),
        [
            RuntimeDialogueContentEffectTrigger::Content,
            RuntimeDialogueContentEffectTrigger::Delay {
                duration: LogicalDuration::from_nanos(120_000_000),
            },
        ]
    );
}

#[test]
fn parent_and_nested_effect_ordinals_remain_local_before_runtime_rebasing() {
    let compiled = compile_attached_dialogue_project(
        r#"
pub character alice { display = "Alice" }

flow main() -> Unit {
    alice[root [call log.info("root-content")] #strong()[nested [call log.info("nested-content")] [at 60ms call=log.info("nested-delay")]] [at 120ms call=log.info("root-delay")]]
}

entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("parent and nested effect plans rebase from distinct local ordinal domains");
    let [content] = compiled.runtime_plan().plan.dialogue_content().rows() else {
        panic!("one dialogue content plan")
    };
    assert_eq!(content.effect_site_count().get(), 4);
    assert_dialogue_effect_sites(&compiled.runtime_plan().plan, content, 4);
    assert_eq!(
        dialogue_effect_triggers(&compiled.runtime_plan().plan, content),
        [
            RuntimeDialogueContentEffectTrigger::Content,
            RuntimeDialogueContentEffectTrigger::Content,
            RuntimeDialogueContentEffectTrigger::Delay {
                duration: LogicalDuration::from_nanos(60_000_000),
            },
            RuntimeDialogueContentEffectTrigger::Delay {
                duration: LogicalDuration::from_nanos(120_000_000),
            },
        ]
    );
}

#[test]
fn explicit_typed_text_proxy_materializes_only_from_final_sema_authority() {
    let compiled = compile_attached_dialogue_project(
        r#"
#[text_proxy(role = "keyword", hit_test = true, channel = "fallback")]
pub struct KeywordHit {
    channel: String
    weight: Option<i64>
}

pub character alice { display = "Alice" }

flow main() -> Unit {
    alice[#object(id = @.hotspot, type = KeywordHit, weight = 3)[typed]]
}

entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("explicit typed text proxy compiles from final sema authority");
    let proxy = object_proxy(&compiled);
    assert_eq!(proxy.id, "hotspot");
    assert_eq!(proxy.role.as_deref(), Some("keyword"));
    assert!(proxy.hit_test);
    let declaration = proxy.declaration.as_ref().expect("typed provenance");
    assert_eq!(declaration.struct_name, "KeywordHit");
    assert_eq!(declaration.attribute, "text_proxy");
    assert_eq!(proxy.type_name.as_deref(), Some("KeywordHit"));
    let schema = proxy.schema.as_ref().expect("typed schema DTO");
    assert_eq!(schema.id, "KeywordHit");
    assert_eq!(schema.declaration.struct_name, "KeywordHit");
    assert_eq!(schema.declaration.attribute, "text_proxy");
    assert_eq!(schema.fields.len(), 2);
    assert_eq!(schema.fields[0].id, 0);
    assert_eq!(schema.fields[0].name, "channel");
    assert!(matches!(
        schema.fields[0].kind,
        RichTextTextProxyFieldKind::Text
    ));
    assert!(!schema.fields[0].optional);
    assert!(matches!(
        &schema.fields[0].default,
        Some(RichTextTextProxyScalar::Text { value }) if value == "fallback"
    ));
    assert_eq!(schema.fields[1].id, 1);
    assert_eq!(schema.fields[1].name, "weight");
    assert!(matches!(
        schema.fields[1].kind,
        RichTextTextProxyFieldKind::Int
    ));
    assert!(schema.fields[1].optional);
    assert!(schema.fields[1].default.is_none());
    assert_eq!(proxy.fields[0].id, 0);
    assert_eq!(proxy.fields[0].name, "channel");
    assert!(matches!(
        proxy.fields[0].value,
        RichTextTextProxyScalar::Text { ref value } if value == "fallback"
    ));
    assert_eq!(proxy.fields[1].id, 1);
    assert_eq!(proxy.fields[1].name, "weight");
    assert!(matches!(
        proxy.fields[1].value,
        RichTextTextProxyScalar::Int { value: 3 }
    ));

    let executable = compiled
        .hir_project()
        .executable_view()
        .expect("compiled project has executable HIR");
    let runtime_owners = project_runtime_reachability(
        executable,
        compiled.project_symbols(),
        compiled.final_analysis(),
        compiled.checked_entries(),
        RuntimeEmissionMode::CheckAll,
    )
    .expect("Object runtime reachability");
    let runtime_facts = project_runtime_semantic_facts(
        executable,
        compiled.project_symbols(),
        compiled.registered_world(),
        compiled.final_analysis(),
        &runtime_owners,
        Some((
            compiled.dialogue_profile().presentation(),
            compiled.dialogue_profile().revision(),
        )),
        None,
        &arcweft_compiler::lower::ProjectInstantiationControl::default(),
    )
    .expect("Object runtime semantic facts")
    .0;
    let object_owner = compiled
        .final_analysis()
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(
                expression.resolution(),
                CheckedExpressionResolution::ContentApplication(_)
            )
            .then_some(owner)
        })
        .expect("checked Object application owner");
    assert!(
        runtime_owners.contains_expression(object_owner),
        "Object remains structurally reachable for body traversal"
    );
    assert!(
        !runtime_owners
            .selected_expression_type_owners()
            .expect("Object runtime type owner inventory")
            .contains(&object_owner)
    );
    assert!(runtime_facts.expression_type(object_owner).is_none());
    assert!(
        runtime_facts
            .calls()
            .all(|(owner, _)| owner != object_owner)
    );
}

#[test]
fn typed_text_proxy_schema_preserves_enum_defaults_order_and_optional_fields() {
    let compiled = compile_attached_dialogue_project(
        r#"
enum KeywordKind {
    Plain
    Emphasis
}

#[text_proxy(role = "keyword", kind = .Plain)]
pub struct KeywordHit {
    kind: KeywordKind
    note: Option<String>
}

pub character alice { display = "Alice" }

flow main() -> Unit {
    alice[#object(id = @.hotspot, type = KeywordHit, kind = .Emphasis)[enum]]
}

entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("enum-backed text proxy compiles at the object boundary");
    let proxy = object_proxy(&compiled);
    let schema = proxy.schema.as_ref().expect("typed schema DTO");
    assert_eq!(schema.id, "KeywordHit");
    assert_eq!(proxy.type_name.as_deref(), Some(schema.id.as_str()));
    assert_eq!(schema.fields.len(), 2);
    assert_eq!(schema.fields[0].id, 0);
    assert_eq!(schema.fields[0].name, "kind");
    assert!(!schema.fields[0].optional);
    assert!(matches!(
        &schema.fields[0].kind,
        RichTextTextProxyFieldKind::ClosedEnum { enum_id, variants }
            if enum_id == "KeywordKind" && variants == &["Plain", "Emphasis"]
    ));
    assert!(matches!(
        &schema.fields[0].default,
        Some(RichTextTextProxyScalar::ClosedEnum { enum_id, variant })
            if enum_id == "KeywordKind" && *variant == 0
    ));
    assert_eq!(schema.fields[1].id, 1);
    assert_eq!(schema.fields[1].name, "note");
    assert!(schema.fields[1].optional);
    assert!(schema.fields[1].default.is_none());
    assert_eq!(proxy.fields.len(), 1);
    assert_eq!(proxy.fields[0].id, 0);
    assert_eq!(proxy.fields[0].name, "kind");
    assert!(matches!(
        &proxy.fields[0].value,
        RichTextTextProxyScalar::ClosedEnum { enum_id, variant }
            if enum_id == "KeywordKind" && *variant == 1
    ));
}

#[test]
fn dialogue_mark_projection_is_content_ordered_and_uses_the_exact_checked_trigger() {
    let compiled = compile_attached_dialogue_project(
        r#"
pub character alice { display = "Alice" }

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
pub character alice { display = "Alice" }

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

fn assert_dialogue_effect_sites(
    plan: &RuntimePlan,
    content: &RuntimeDialogueContentPlan,
    expected: usize,
) {
    assert_eq!(content.effect_sites().len(), expected);
    let manifest = plan
        .dialogue_content()
        .template(content.template())
        .expect("dialogue template manifest");
    assert_eq!(manifest.effects().len(), expected);
    for (index, effect) in content.effect_sites().iter().enumerate() {
        let declared = &manifest.effects()[index];
        assert_eq!(effect.site().index(), index);
        assert_eq!(declared.site(), effect.site());
        let function = plan
            .function_sites()
            .get(effect.function())
            .expect("dialogue callback function site");
        assert!(function.parameter_inputs().next().is_none());
        let RuntimeFunctionSiteBody::Executable(body) = function.body() else {
            panic!("dialogue effect site must reference an executable function site");
        };
        assert!(!body.effects().is_empty());
        assert!(!body.ops().is_empty());
    }
}

fn dialogue_effect_triggers(
    plan: &RuntimePlan,
    content: &RuntimeDialogueContentPlan,
) -> Vec<RuntimeDialogueContentEffectTrigger> {
    plan.dialogue_content()
        .template(content.template())
        .expect("dialogue template manifest")
        .effects()
        .iter()
        .map(|effect| effect.trigger())
        .collect()
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
