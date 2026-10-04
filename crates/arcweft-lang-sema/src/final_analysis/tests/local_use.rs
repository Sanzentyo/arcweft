use arcweft_lang_hir::expr::HirExprKind;

use crate::final_analysis::{
    CheckedLocalCopyEvidence, CheckedLocalReadMode, CheckedLocalUseError,
    CheckedLocalUseInstantiation, CheckedLocalUseSite, FinalSemanticAnalysisError,
};
use crate::types::TypeKind;

use super::{analyze, character_nominal_fixture, fixture};

#[test]
fn closed_instance_from_an_equivalent_rebuild_cannot_issue_local_use_evidence() {
    let source = "fn pair<T>(value: T) -> (T, T) { (value, value) }\nfn caller() -> (i64, i64) { pair(1i64) }";
    let first = fixture(source, None);
    let second = fixture(source, None);
    let first_report = analyze(&first).unwrap();
    let second_report = analyze(&second).unwrap();
    let foreign = super::project_specialization::selections(&first_report, "pair")
        .remove(0)
        .close_instance(None)
        .unwrap();
    let current = super::project_specialization::selections(&second_report, "pair")
        .remove(0)
        .close_instance(None)
        .unwrap();
    assert_eq!(foreign.declaration(), current.declaration());
    assert_eq!(foreign.instantiation(), current.instantiation());
    assert_eq!(foreign.callable_type(), current.callable_type());
    assert_ne!(
        foreign, current,
        "equal semantic identities do not share admission leases"
    );
    assert!(matches!(
        second_report.checked_local_uses_for_instance(
            second.project.analysis_view().unwrap(),
            &second.symbols,
            CheckedLocalUseInstantiation::ProjectFunction(&foreign),
        ),
        Err(CheckedLocalUseError::ForeignInstance)
    ));
    second_report
        .checked_local_uses_for_instance(
            second.project.analysis_view().unwrap(),
            &second.symbols,
            CheckedLocalUseInstantiation::ProjectFunction(&current),
        )
        .unwrap();
}

#[test]
fn field_transfers_preserve_initialized_siblings() {
    for source in [
        "struct Pair { style: RichTextStyle, other: i64 }\nfn inspect(pair: Pair) -> (i64, RichTextStyle) { (pair.other, pair.style) }",
        "struct Pair { style: RichTextStyle, other: i64 }\nfn inspect(pair: Pair) -> (RichTextStyle, i64) { (pair.style, pair.other) }",
    ] {
        let fixture = fixture(source, None);
        analyze(&fixture).expect("a field transfer preserves initialized siblings");
    }
}

#[test]
fn moved_field_assignment_restores_its_aggregate() {
    let fixture = fixture(
        r#"
struct Pair { style: RichTextStyle, other: i64 }
fn replace(first: RichTextStyle, second: RichTextStyle) -> Pair {
    let mut pair = Pair { style: first, other: 42i64 }
    let moved = pair.style
    pair.style = second
    pair
}

"#,
        None,
    );
    analyze(&fixture).expect("field assignment restores the moved child before whole-place use");
}

#[test]
fn mutable_field_receiver_allows_moving_a_disjoint_sibling_operand() {
    let fixture = fixture(
        r#"
struct Pair { items: Vec<RichTextStyle>, other: RichTextStyle }
fn push(input: Pair) -> Option<RichTextStyle> {
    let mut pair = input
    pair.items.push(value=pair.other)
    pair.items.pop()
}
"#,
        None,
    );
    analyze(&fixture)
        .expect("the receiver reservation protects its field while siblings remain independent");
}

#[test]
fn partial_move_rejects_whole_use_and_reading_the_moved_child() {
    for tail in ["pair", "pair.style"] {
        let result = if tail == "pair" {
            "Pair"
        } else {
            "RichTextStyle"
        };
        let source = format!(
            r#"
struct Pair {{ style: RichTextStyle, other: i64 }}
fn consume(first: RichTextStyle) -> {result} {{
    let pair = Pair {{ style: first, other: 42i64 }}
    let moved = pair.style
    {tail}
}}
"#
        );
        let fixture = fixture(&source, None);
        assert!(matches!(
            analyze(&fixture),
            Err(FinalSemanticAnalysisError::LocalUse(
                CheckedLocalUseError::Unavailable { .. }
            ))
        ));
    }
}

#[test]
fn indexing_requires_a_copyable_selected_item() {
    let copyable = fixture(
        r#"
fn first(items: Vec<i64>) -> i64 { items[0] }
"#,
        None,
    );
    analyze(&copyable).expect("indexing a Copy item is admitted");

    let affine = fixture(
        r#"
fn first(items: Vec<VoiceHandle>) -> VoiceHandle { items[0] }
"#,
        None,
    );
    assert!(matches!(
        analyze(&affine),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::IndexRequiresCopy { .. }
        ))
    ));
}

#[test]
fn generic_index_copy_requirement_closes_for_each_selected_instance() {
    let fixture = fixture(
        r#"
fn first<T>(items: Vec<T>) -> T { items[0] }
fn numeric(items: Vec<i64>) -> i64 { first(items) }
fn voiced(items: Vec<VoiceHandle>) -> VoiceHandle { first(items) }
"#,
        None,
    );
    let report = analyze(&fixture).expect("open generic index defers its Copy proof");
    let selections = super::project_specialization::selections(&report, "first");
    assert_eq!(selections.len(), 2);
    let mut accepted = 0;
    let mut rejected = 0;
    for selection in selections {
        let instance = selection
            .close_instance(None)
            .expect("closed index instance");
        match report.checked_local_uses_for_instance(
            fixture.project.analysis_view().expect("executable HIR"),
            &fixture.symbols,
            CheckedLocalUseInstantiation::ProjectFunction(&instance),
        ) {
            Ok(_) => accepted += 1,
            Err(CheckedLocalUseError::IndexRequiresCopy { .. }) => rejected += 1,
            Err(other) => panic!("unexpected closed index error: {other:?}"),
        }
    }
    assert_eq!((accepted, rejected), (1, 1));
}

#[test]
fn generic_local_reads_close_under_the_selected_instance() {
    let fixture = fixture(
        r#"
fn pair<T>(value: T) -> (T, T) { (value, value) }
flow main() -> (i64, i64) { return pair(1i64) }
"#,
        None,
    );
    let report = analyze(&fixture).expect("open generic body defers transfer modes");
    let selection = super::project_specialization::selections(&report, "pair").remove(0);
    let instance = selection.close_instance(None).expect("closed pair<i64>");
    let closed = report
        .checked_local_uses_for_instance(
            fixture.project.analysis_view().expect("executable HIR"),
            &fixture.symbols,
            CheckedLocalUseInstantiation::ProjectFunction(&instance),
        )
        .expect("closed i64 value is deeply Copy");
    assert_eq!(
        closed
            .value_transfers()
            .filter(|(_, row)| row.mode() == CheckedLocalReadMode::Copy)
            .count(),
        2
    );
    assert!(closed.value_transfers().all(|(site, _)| {
        report
            .checked_local_uses()
            .value_transfer_at(site)
            .is_none()
    }));
}

#[test]
fn generic_affine_instance_rejects_duplicate_local_transfer() {
    let fixture = fixture(
        r#"
fn pair<T>(value: T) -> (T, T) { (value, value) }
fn caller(voice: VoiceHandle) -> (VoiceHandle, VoiceHandle) { pair(voice) }
"#,
        None,
    );
    let report = analyze(&fixture).expect("generic declaration defers ownership to its instance");
    let selection = super::project_specialization::selections(&report, "pair").remove(0);
    let instance = selection
        .close_instance(None)
        .expect("closed pair<VoiceHandle>");
    let actual = report.checked_local_uses_for_instance(
        fixture.project.analysis_view().expect("executable HIR"),
        &fixture.symbols,
        CheckedLocalUseInstantiation::ProjectFunction(&instance),
    );
    assert!(matches!(
        actual,
        Err(CheckedLocalUseError::Unavailable { .. })
    ));
}

#[test]
fn generic_guard_copy_obligation_closes_per_selected_instance() {
    let fixture = fixture(
        r#"
fn check<T>(value: T) -> bool { true }
fn guarded<T>(candidate: Option<T>) {
    match candidate {
        .Some(value) when check(value) => ()
        _ => ()
    }
}
fn numeric(candidate: Option<i64>) { guarded(candidate) }
fn voiced(candidate: Option<VoiceHandle>) { guarded(candidate) }
"#,
        None,
    );
    let report = analyze(&fixture).expect("open generic guard defers ownership to its instance");
    let selections = super::project_specialization::selections(&report, "guarded");
    assert_eq!(selections.len(), 2);
    let project = fixture.project.analysis_view().expect("executable HIR");
    let module = project.modules().next().expect("root module").1;
    let guard = module
        .expressions()
        .find_map(|(_, expression)| match expression.kind() {
            arcweft_lang_hir::expr::HirExprKind::Match(value) => value.arms()[0].guard(),
            _ => None,
        })
        .expect("generic selected guard");
    let mut accepted = 0;
    let mut rejected = 0;
    for selection in selections {
        let instance = selection
            .close_instance(None)
            .expect("closed guarded instance");
        match report.checked_local_uses_for_instance(
            fixture.project.analysis_view().expect("executable HIR"),
            &fixture.symbols,
            CheckedLocalUseInstantiation::ProjectFunction(&instance),
        ) {
            Ok(catalog) => {
                assert_eq!(catalog.guard_copy_locals(guard).count(), 1);
                accepted += 1;
            }
            Err(CheckedLocalUseError::GuardBoundAffineRead { .. }) => rejected += 1,
            Err(other) => panic!("unexpected closed guard error: {other:?}"),
        }
    }
    assert_eq!((accepted, rejected), (1, 1));
}

#[test]
fn generic_implicit_callable_owner_placeholder_seals_each_closed_parameter_use() {
    let fixture = fixture(
        r#"
fn make<T>(sample: T) -> (T -> T effects {}) { _ }
flow main() -> i64 {
    let numeric = make(0i64)
    let text = make("")
    text("word")
    return numeric(3i64)
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("generic implicit callable is accepted");
    let selections = super::project_specialization::selections(&report, "make");
    assert_eq!(selections.len(), 2);
    let mut identities = Vec::new();
    let mut sites = Vec::new();
    for selection in selections {
        let instance = selection
            .close_instance(None)
            .expect("closed make instance");
        let closed = report
            .checked_local_uses_for_instance(
                fixture.project.analysis_view().expect("executable HIR"),
                &fixture.symbols,
                CheckedLocalUseInstantiation::ProjectFunction(&instance),
            )
            .expect("closed implicit parameter use is sealed");
        identities.push(closed.identity().clone());
        let rows = closed.synthetic_rows().collect::<Vec<_>>();
        let [(site, use_row)] = rows.as_slice() else {
            panic!("each closed body has exactly one implicit parameter use")
        };
        assert_eq!(use_row.mode(), CheckedLocalReadMode::Copy);
        assert!(matches!(
            use_row.owner(),
            crate::final_analysis::CheckedSyntheticUseOwner::ImplicitParameter(_)
        ));
        sites.push(*site);
    }
    assert_ne!(identities[0], identities[1]);
    assert_eq!(sites[0], sites[1]);
}

#[test]
fn repeated_function_parameter_use_seals_exact_ingress_copy_requirement() {
    let fixture = fixture(
        r#"
fn twice(callback: i64 -> i64, value: i64) -> (i64, i64) {
    (callback(value), callback(value))
}
fn caller() -> (i64, i64) { twice(|value: i64| value, 42i64) }
"#,
        None,
    );
    let report = analyze(&fixture).expect("copy demand is deferred to selected ingress");
    let requirements = report
        .checked_local_uses()
        .copy_requirements()
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        panic!("one exact Function parameter requires deep Copy ingress")
    };
    assert!(matches!(
        requirement.owner(),
        crate::final_analysis::CheckedLocalCopyIngressOwner::Declaration {
            parameter: crate::final_analysis::CheckedIngressParameterCoordinate::Parameter {
                group: 0,
                parameter: 0,
            },
            ..
        }
    ));
    assert!(matches!(
        report
            .local(requirement.local())
            .map(|binding| binding.ty()),
        Some(TypeKind::Function { .. })
    ));
    assert_eq!(
        report
            .checked_local_uses()
            .value_transfers()
            .filter(|(_, row)| row.local() == requirement.local()
                && row.mode() == CheckedLocalReadMode::Copy)
            .count(),
        2
    );
}

#[test]
fn repeated_plain_opaque_parameter_requires_deep_copy_at_ingress() {
    let fixture = fixture(
        r#"
fn twice(value: RichTextStyle) -> (RichTextStyle, RichTextStyle) {
    (value, value)
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("plain opaque reuse requires its exact ingress value");
    let requirements = report
        .checked_local_uses()
        .copy_requirements()
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        panic!("one plain opaque parameter requires a deep Copy ingress proof")
    };
    assert!(matches!(
        requirement.owner(),
        crate::final_analysis::CheckedLocalCopyIngressOwner::Declaration {
            parameter: crate::final_analysis::CheckedIngressParameterCoordinate::Parameter {
                group: 0,
                parameter: 0,
            },
            ..
        }
    ));
    assert!(matches!(
        report
            .local(requirement.local())
            .map(|binding| binding.ty()),
        Some(TypeKind::AcceptedNominal(_))
    ));
    assert_eq!(
        report
            .checked_local_uses()
            .value_transfers()
            .filter(|(_, row)| row.local() == requirement.local()
                && row.mode() == CheckedLocalReadMode::Copy)
            .count(),
        2
    );
}

#[test]
fn immutable_function_alias_traces_copy_demand_to_its_ingress() {
    let fixture = fixture(
        r#"
fn twice(callback: i64 -> i64) -> (i64, i64) {
    let alias = callback
    (alias(1i64), alias(2i64))
}
fn caller() -> (i64, i64) { twice(|value: i64| value) }
"#,
        None,
    );
    let report = analyze(&fixture).expect("immutable alias shares its selected ingress proof");
    let requirements = report
        .checked_local_uses()
        .copy_requirements()
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        panic!("alias must demand one exact ingress Function proof")
    };
    assert!(matches!(
        requirement.owner(),
        crate::final_analysis::CheckedLocalCopyIngressOwner::Declaration {
            parameter: crate::final_analysis::CheckedIngressParameterCoordinate::Parameter {
                group: 0,
                parameter: 0,
            },
            ..
        }
    ));
    let alias = report
        .locals()
        .find_map(|(local, binding)| {
            (local != requirement.local() && matches!(binding.ty(), TypeKind::Function { .. }))
                .then_some(local)
        })
        .expect("Function alias binding");
    assert_eq!(
        report
            .checked_local_uses()
            .value_transfers()
            .filter(|(_, row)| row.local() == alias && row.mode() == CheckedLocalReadMode::Copy)
            .count(),
        2
    );
}

#[test]
fn destructured_function_copy_requirement_does_not_cover_affine_sibling() {
    let valid = fixture(
        r#"
fn twice((callback, voice): (i64 -> i64, VoiceHandle)) -> (i64, i64) {
    (callback(1i64), callback(2i64))
}
"#,
        None,
    );
    let report = analyze(&valid).expect("only the destructured Function binding needs Copy");
    let requirements = report
        .checked_local_uses()
        .copy_requirements()
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        panic!("one exact nested parameter binding requirement")
    };
    assert!(matches!(
        report
            .local(requirement.local())
            .map(|binding| binding.ty()),
        Some(TypeKind::Function { .. })
    ));
    assert!(report.locals().any(|(local, binding)| {
        binding.ty() == &TypeKind::VoiceHandle
            && report
                .checked_local_uses()
                .copy_requirement(local)
                .is_none()
    }));

    let invalid = fixture(
        r#"
fn consume(voice: VoiceHandle) {}
fn twice((callback, voice): (i64 -> i64, VoiceHandle)) {
    callback(1i64)
    callback(2i64)
    consume(voice)
    consume(voice)
}
"#,
        None,
    );
    assert!(matches!(
        analyze(&invalid),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::Unavailable { .. }
        ))
    ));
}

#[test]
fn repeated_explicit_closure_parameter_has_closure_owned_requirement() {
    let fixture = fixture(
        r#"
fn caller() {
    let twice = |callback: i64 -> i64 effects {}| (callback(1i64), callback(2i64))
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("closure parameter Copy demand is explicit");
    let requirements = report
        .checked_local_uses()
        .copy_requirements()
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        panic!("one explicit closure parameter demand")
    };
    assert!(matches!(
        requirement.owner(),
        crate::final_analysis::CheckedLocalCopyIngressOwner::Closure { parameter: 0, .. }
    ));
}

#[test]
fn repeated_implicit_callable_function_parameter_has_synthetic_requirement() {
    let fixture = fixture(
        r#"
fn caller() {
    let twice: (i64 -> i64 effects {}) -> ((i64 -> i64 effects {}), (i64 -> i64 effects {})) = (_, _)
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("implicit callable parameter demands deep Copy");
    let requirements = report
        .checked_local_uses()
        .synthetic_copy_requirements()
        .collect::<Vec<_>>();
    assert_eq!(
        requirements.len(),
        1,
        "synthetic rows: {:?}",
        report
            .checked_local_uses()
            .synthetic_rows()
            .map(|(site, row)| (site, row.owner(), row.mode()))
            .collect::<Vec<_>>()
    );
    assert!(
        report
            .checked_local_uses()
            .synthetic_rows()
            .all(|(_, row)| { row.mode() == CheckedLocalReadMode::Copy })
    );
}

#[test]
fn capture_free_function_local_has_selected_copy_evidence() {
    let fixture = fixture(
        r#"
fn identity(value: i32) -> i32 { value }
fn reuse() -> (i32, i32) {
    let callback = identity
    (callback(1i32), callback(2i32))
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("capture-free callable is reusable");
    let function_local = report
        .locals()
        .find_map(|(local, binding)| {
            matches!(binding.ty(), TypeKind::Function { .. }).then_some(local)
        })
        .expect("function local");
    assert!(matches!(
        report.checked_local_uses().copy_evidence(function_local),
        Some(CheckedLocalCopyEvidence::ImmutableInitializer { .. })
    ));
    assert_eq!(
        report
            .checked_local_uses()
            .value_transfers()
            .filter(|(_, row)| row.local() == function_local
                && row.mode() == CheckedLocalReadMode::Copy)
            .count(),
        2
    );
}

#[test]
fn pipe_reuses_a_capture_free_function_from_its_selected_left_source() {
    let fixture = fixture(
        r#"
fn identity(value: i32) -> i32 { value }
fn apply(callback: i32 -> i32, value: i32) -> i32 { callback(value) }
fn reuse() -> (i32, i32) { identity |> (apply(^, 1i32), apply(^, 2i32)) }
"#,
        None,
    );
    let report = analyze(&fixture).expect("pipe left has selected capture-free function proof");
    let uses = report
        .checked_local_uses()
        .synthetic_rows()
        .collect::<Vec<_>>();
    assert_eq!(uses.len(), 2);
    assert!(
        uses.iter()
            .all(|(_, row)| row.mode() == CheckedLocalReadMode::Copy)
    );
}

#[test]
fn scheduled_line_closure_capture_has_exact_move_site() {
    let fixture = character_nominal_fixture(concat!(
        "pub character akane {}\n",
        "flow line_handles() -> String {\n",
        "    let (_, cue) = akane(voice=auto)[聞いて。[p]]\n",
        "    with:\n",
        "        let actor = akane.stage.acquire(scope=line)\n",
        "        let cue = at(0.42s):\n",
        "            actor.look(.normal)\n",
        "        let voice = line.voice_handle()\n",
        "        out (voice, cue)\n",
        "    return \"done\"\n",
        "}\n",
    ));
    let report = analyze(&fixture).expect("one scheduled actor capture");
    let executable = fixture.project.analysis_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    let callback = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Closure(closure) if !closure.captures().is_empty() => Some(owner),
            _ => None,
        })
        .expect("scheduled closure");
    let observed = report
        .checked_local_uses()
        .value_transfers()
        .filter_map(|(site, row)| match site {
            CheckedLocalUseSite::Capture { owner, local } => Some((
                owner,
                local,
                row.mode(),
                report.local(local).map(|binding| binding.ty().clone()),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let line_owners = module
        .expressions()
        .filter_map(|(owner, expression)| {
            expression.kind().expression_owned_child_edges().ok().and_then(|edges| {
                let line_statements = edges.iter().filter_map(|edge| {
                    matches!(edge.role(), arcweft_lang_hir::expr::HirExpressionOwnedBodyRole::DialogueLinePlanStatement { .. }).then_some(match edge.child() {
                        arcweft_lang_hir::expr::HirExpressionOwnedChild::Statement(statement) => (Some(statement), report.statement(statement).is_some()),
                        _ => (None, false),
                    })
                }).collect::<Vec<_>>();
                (!line_statements.is_empty()).then_some((owner, expression.kind().semantic_transcript_tag(), report.expression(owner).is_some_and(|value| matches!(value.resolution(), crate::final_analysis::CheckedExpressionResolution::DialogueApplication { .. })), line_statements))
            })
        })
        .collect::<Vec<_>>();
    assert!(
        report
            .checked_local_uses()
            .value_transfers()
            .any(|(site, row)| {
                site == CheckedLocalUseSite::Capture {
                    owner: callback,
                    local: row.local(),
                } && row.mode() == CheckedLocalReadMode::Move
                    && report.local(row.local()).is_some_and(|binding| {
                        matches!(binding.ty(), TypeKind::StageActorHandle(_))
                    })
            }),
        "callback {callback:?}, capture rows {observed:?}, line owners {line_owners:?}"
    );
}

#[test]
fn stage_actor_look_borrows_one_affine_receiver_twice_before_transfer() {
    let fixture = character_nominal_fixture(concat!(
        "pub character akane {}\n",
        "flow repeated_look() -> String {\n",
        "    let (retained, _) = akane(voice=auto)[聞いて。[p]]\n",
        "    with:\n",
        "        let actor = akane.stage.acquire(scope=line)\n",
        "        actor.look(.normal)\n",
        "        actor.look(.normal)\n",
        "        out (actor, ())\n",
        "    return \"done\"\n",
        "}\n",
    ));
    let report = analyze(&fixture).expect("Look borrows the live StageActor receiver");
    let actor_uses = report
        .checked_local_uses()
        .value_transfers()
        .filter_map(|(_, row)| {
            report
                .local(row.local())
                .is_some_and(|binding| matches!(binding.ty(), TypeKind::StageActorHandle(_)))
                .then_some(row.mode())
        })
        .collect::<Vec<_>>();
    assert_eq!(actor_uses.len(), 3);
    assert_eq!(
        actor_uses
            .iter()
            .filter(|mode| **mode == CheckedLocalReadMode::Borrow)
            .count(),
        2
    );
    assert_eq!(
        actor_uses
            .iter()
            .filter(|mode| **mode == CheckedLocalReadMode::Move)
            .count(),
        1
    );
}

#[test]
fn stage_actor_look_cannot_borrow_after_owner_transfer() {
    let fixture = character_nominal_fixture(concat!(
        "pub character akane {}\n",
        "flow moved_actor() -> String {\n",
        "    let (retained, _) = akane(voice=auto)[聞いて。[p]]\n",
        "    with:\n",
        "        let actor = akane.stage.acquire(scope=line)\n",
        "        let moved = actor\n",
        "        actor.look(.normal)\n",
        "        out (moved, ())\n",
        "    return \"done\"\n",
        "}\n",
    ));
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::Unavailable { .. }
        ))
    ));
}

#[test]
fn repeated_pop_reads_an_affine_vec_receiver_as_a_mutable_place() {
    let fixture = fixture(
        r#"
fn pop_twice(input: Vec<Need<i64>>) {
    let mut items = input
    items.pop();
    items.pop();
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("both pop calls borrow one mutable Vec place");
    let executable = fixture.project.analysis_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    let mutable_vec = report
        .locals()
        .find_map(|(local, binding)| {
            (matches!(binding.ty(), TypeKind::Vec(_))
                && module
                    .resolve_local(local)
                    .is_ok_and(|local| local.is_mutable_binding()))
            .then_some(local)
        })
        .expect("mutable Vec local");
    assert!(
        report
            .checked_local_uses()
            .value_transfers()
            .all(|(_, row)| row.local() != mutable_vec)
    );
    let places = report
        .checked_local_uses()
        .rows()
        .filter_map(|(_, access)| access.place_access())
        .collect::<Vec<_>>();
    assert_eq!(places.len(), 2);
    assert!(
        places
            .iter()
            .all(|access| access.place().local_id() == mutable_vec
                && access.mode() == crate::final_analysis::CheckedLocalPlaceMode::Mutate)
    );
}

#[test]
fn mutation_requires_the_affine_owner_to_remain_available() {
    for source in [
        "fn root(input: Vec<Need<i64>>) { let mut items = input; let moved = items; items.pop(); () }",
    ] {
        let world = fixture(source, None);
        assert!(
            matches!(
                analyze(&world),
                Err(FinalSemanticAnalysisError::LocalUse(
                    CheckedLocalUseError::Unavailable {
                        site: CheckedLocalUseSite::Place(_),
                        ..
                    }
                ))
            ),
            "mutation after owner transfer: {source}"
        );
    }
}

#[test]
fn whole_local_assignment_initializes_and_mutation_requires_a_live_owner() {
    let world = fixture(
        "fn root(input: Vec<Need<i64>>, replacement: Vec<Need<i64>>) -> Option<Need<i64>> { let mut items = input; items = replacement; items.pop() }",
        None,
    );
    let report = analyze(&world).expect("replacement keeps a live owner available");
    let modes = report
        .checked_local_uses()
        .rows()
        .filter_map(|(_, access)| access.place_access().map(|access| access.mode()))
        .collect::<Vec<_>>();
    assert_eq!(
        modes,
        [
            crate::final_analysis::CheckedLocalPlaceMode::Assign,
            crate::final_analysis::CheckedLocalPlaceMode::Mutate
        ]
    );
}

#[test]
fn assignment_reinitializes_after_move_and_reachable_branch_joins() {
    use crate::final_analysis::{CheckedPlaceDisplacement, CheckedPlaceInitialization as Init};
    for (body, expected) in [
        ("items = replacement; ()", Init::Initialized),
        (
            "let moved = items; items = replacement; ()",
            Init::Uninitialized,
        ),
        (
            "let moved = items; let ignored = { let marker = 0i64; items = replacement; () }; ()",
            Init::Uninitialized,
        ),
        (
            "let ignored = if condition { let moved = items; () } else { () }; items = replacement; ()",
            Init::Conditional,
        ),
        ("items = identity(items); ()", Init::Uninitialized),
        (
            "items = if condition { identity(items) } else { replacement }; ()",
            Init::Conditional,
        ),
    ] {
        let source = format!(
            "fn identity(input: Vec<Need<i64>>) -> Vec<Need<i64>> {{ input }}\nfn root(input: Vec<Need<i64>>, replacement: Vec<Need<i64>>, condition: bool) {{ let mut items = input; {body} }}"
        );
        let world = fixture(&source, None);
        let report = analyze(&world)
            .unwrap_or_else(|error| panic!("Rust-compatible initialization: {source}: {error:?}"));
        let assignments = report
            .checked_local_uses()
            .rows()
            .filter_map(|(_, access)| access.place_access())
            .filter(|access| access.mode() == crate::final_analysis::CheckedLocalPlaceMode::Assign)
            .collect::<Vec<_>>();
        let [assignment] = assignments.as_slice() else {
            panic!("one write boundary")
        };
        let Some(CheckedPlaceDisplacement::Reachable {
            initialization,
            fields,
        }) = assignment.displacement()
        else {
            panic!("sealed post-RHS cleanup contour")
        };
        assert_eq!(*initialization, expected, "{source}");
        assert!(fields.is_empty());
    }
}

#[test]
fn shadowing_after_move_creates_a_new_owner_generation() {
    let world = fixture(
        "fn root(input: Vec<Need<i64>>, replacement: Vec<Need<i64>>) -> Option<Need<i64>> { let items = input; let moved = items; let items = replacement; items.pop() }",
        None,
    );
    analyze(&world).expect("shadowing initializes a distinct declaration");
}

#[test]
fn generic_place_access_is_issued_only_by_the_selected_closed_instance() {
    let world = fixture(
        "fn root<T>(input: Vec<T>) -> Option<T> { let mut items = input; items.pop() }\nfn caller(input: Vec<i64>) -> Option<i64> { root(input) }",
        None,
    );
    let report = analyze(&world).unwrap();
    let selection = super::project_specialization::selections(&report, "root").remove(0);
    let instance = selection.close_instance(None).unwrap();
    let catalog = report
        .checked_local_uses_for_instance(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            CheckedLocalUseInstantiation::ProjectFunction(&instance),
        )
        .unwrap();
    let (site, access) = catalog
        .rows()
        .find(|(_, access)| access.place_access().is_some())
        .unwrap();
    assert!(report.checked_local_uses().access_at(site).is_none());
    assert_eq!(
        access.place_access().unwrap().mode(),
        crate::final_analysis::CheckedLocalPlaceMode::Mutate
    );
    assert!(catalog.value_transfer_at(site).is_none());
}

#[test]
fn closed_generic_instance_initializes_after_affine_transfer() {
    let world = fixture(
        "fn root<T>(input: Vec<T>, replacement: Vec<T>) { let mut items = input; let moved = items; items = replacement; () }\nfn caller(input: Vec<Need<i64>>, replacement: Vec<Need<i64>>) { root(input, replacement); () }",
        None,
    );
    let report =
        analyze(&world).expect("the open body defers local access until its selected instance");
    let selection = super::project_specialization::selections(&report, "root").remove(0);
    let instance = selection.close_instance(None).unwrap();
    let catalog = report
        .checked_local_uses_for_instance(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            CheckedLocalUseInstantiation::ProjectFunction(&instance),
        )
        .expect("closed affine instance initializes the existing declaration");
    assert!(
        catalog
            .rows()
            .any(|(_, access)| access.place_access().is_some_and(
                |access| access.mode() == crate::final_analysis::CheckedLocalPlaceMode::Assign
            ))
    );
}

#[test]
fn in_place_call_rechecks_owner_after_its_value_operands() {
    let world = fixture(
        "fn take(input: Vec<Need<i64>>, item: Need<i64>) -> Need<i64> { item }\nfn root(input: Vec<Need<i64>>, item: Need<i64>) { let mut items = input; items.push(take(items, item)); () }",
        None,
    );
    assert!(matches!(
        analyze(&world),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::BorrowedReceiverInvalidation { .. }
        ))
    ));
}

#[test]
fn mutable_receiver_reservation_survives_operand_move_and_restoration() {
    for operand in [
        "{ let taken = items; items = taken; item }",
        "{ let marker = 0i64; items = replacement; item }",
    ] {
        let source = format!(
            "fn root(input: Vec<Need<i64>>, replacement: Vec<Need<i64>>, item: Need<i64>) {{ let mut items = input; items.push(value={operand}); () }}"
        );
        assert!(
            matches!(
                analyze(&fixture(&source, None)),
                Err(FinalSemanticAnalysisError::LocalUse(
                    CheckedLocalUseError::BorrowedReceiverInvalidation { .. }
                ))
            ),
            "receiver reservation: {source}"
        );
    }
}

#[test]
fn latent_callable_body_has_its_own_loan_and_control_frame() {
    let source = "fn root(input: Vec<i64>) { let mut items = input; items.push(value=(|| { items.pop(); 0i64 })()); () }";
    analyze(&fixture(source, None)).expect("a Copy capture is owned by the callback frame; mutating that copy does not invalidate the caller's reservation");
}

#[test]
fn initialization_is_solved_over_real_loop_backedges_and_exits() {
    for body in [
        "while condition { items = identity(items); }
return items.pop()",
        "while condition { items = identity(items); continue }
return items.pop()",
        "let ignored = loop { let moved = items; break }
items = replacement; return items.pop()",
        "let ignored = loop { items = identity(items); break }
return items.pop()",
        "while ({ let marker = 0i64; items = identity(items); condition }) { }
return items.pop()",
        "while condition { let ignored = loop { items = identity(items); break }; }
return items.pop()",
    ] {
        let source = format!(
            "fn identity(input: Vec<Need<i64>>) -> Vec<Need<i64>> {{ input }}\nflow root(input: Vec<Need<i64>>, replacement: Vec<Need<i64>>, condition: bool) -> Option<Need<i64>> {{ \nlet mut items = input\n{body}\n }}"
        );
        analyze(&fixture(&source, None))
            .unwrap_or_else(|error| panic!("valid loop initialization: {source}: {error:?}"));
    }
}

#[test]
fn missing_restoration_on_continue_break_or_zero_iteration_is_rejected() {
    for body in [
        "while condition { let moved = items; continue
items = replacement; }
return items.pop()",
        "let ignored = loop { let moved = items; break }
return items.pop()",
        "let moved = items; while condition { items = replacement; break }
return items.pop()",
        "while condition { let moved = items; if condition { continue }
items = replacement; }
return items.pop()",
        "while condition { let moved = items; if condition { break }
items = replacement; }
return items.pop()",
    ] {
        let source = format!(
            "flow root(input: Vec<Need<i64>>, replacement: Vec<Need<i64>>, condition: bool) -> Option<Need<i64>> {{ \nlet mut items = input\n{body}\n }}"
        );
        assert!(
            matches!(
                analyze(&fixture(&source, None)),
                Err(FinalSemanticAnalysisError::LocalUse(
                    CheckedLocalUseError::Unavailable { .. }
                        | CheckedLocalUseError::RepeatedLoopMove { .. }
                ))
            ),
            "uninitialized reachable path: {source}"
        );
    }
}

#[test]
fn try_residual_cannot_skip_required_initialization_in_a_carrier_block() {
    let source = "fn root(input: Vec<Need<i64>>, candidate: Option<Vec<Need<i64>>>) -> Option<Need<i64>> { let mut items = input; let moved = items; let ignored: Option<Unit> = option { let restored = try candidate; items = restored; () }; items.pop() }";
    assert!(matches!(
        analyze(&fixture(source, None)),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::Unavailable { .. }
        ))
    ));
    let restored = source.replace("}; items.pop()", "}; items = moved; items.pop()");
    analyze(&fixture(&restored, None))
        .expect("unconditional assignment restores both carrier exits");
}

#[test]
fn callable_try_exit_does_not_poison_its_successful_initialization_path() {
    let source = "fn root(input: Vec<Need<i64>>, candidate: Option<Vec<Need<i64>>>) -> Option<Need<i64>> { let mut items = input; let moved = items; items = try candidate; items.pop() }";
    analyze(&fixture(source, None))
        .expect("residual returns from the callable; only success continues to the write");
}

#[test]
fn borrowed_receiver_cannot_be_moved_or_replaced_by_a_later_operand() {
    for operand in [
        "{ let moved = actor; 0ms }",
        "{ let marker = 0i64; actor = replacement; 0ms }",
    ] {
        let source = format!(
            "pub character akane {{}}\nflow root() -> String {{\n    let (retained, _) = akane(voice=auto)[聞いて。[p]]\n    with:\n        let mut actor = akane.stage.acquire(scope=line)\n        let replacement = akane.stage.acquire(scope=line)\n        actor.look(.normal, crossfade={operand})\n        out (actor, ())\n    return \"done\"\n}}"
        );
        let world = character_nominal_fixture(&source);
        let result = analyze(&world);
        assert!(
            matches!(
                result,
                Err(FinalSemanticAnalysisError::LocalUse(
                    CheckedLocalUseError::BorrowedReceiverInvalidation { .. }
                ))
            ),
            "receiver loan protects operand evaluation: {result:?}"
        );
    }
}

#[test]
fn affine_use_in_one_if_arm_is_unavailable_after_the_join() {
    let fixture = fixture(
        r#"
fn consume(voice: VoiceHandle) {}
fn speak(voice: VoiceHandle, enabled: bool) {
    if enabled { consume(voice); }
    consume(voice);
}
"#,
        None,
    );
    let actual = analyze(&fixture).err();
    assert!(
        matches!(
            actual,
            Some(FinalSemanticAnalysisError::LocalUse(
                CheckedLocalUseError::Unavailable { .. }
            ))
        ),
        "one-sided move must make the joined local unavailable: {actual:?}"
    );
}

#[test]
fn guard_fallthrough_cannot_reuse_an_affine_local() {
    let fixture = fixture(
        r#"
fn check(voice: VoiceHandle) -> bool { false }
fn consume(voice: VoiceHandle) {}
flow guarded(voice: VoiceHandle, candidate: Option<i64>) {
    if let .Some(value) = candidate when check(voice) {
        let observed = value
    } else {
        consume(voice)
    }
}
"#,
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::Unavailable { .. }
        ))
    ));
}

#[test]
fn match_guard_fallthrough_cannot_reuse_an_affine_local() {
    let fixture = fixture(
        r#"
fn check(voice: VoiceHandle) -> bool { false }
fn consume(voice: VoiceHandle) {}
fn guarded(voice: VoiceHandle, enabled: bool) {
    match enabled {
        true when check(voice) => ()
        _ => consume(voice)
    }
}
"#,
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::Unavailable { .. }
        ))
    ));
}

#[test]
fn guard_cannot_move_an_affine_pattern_binding() {
    let fixture = fixture(
        r#"
fn check(voice: VoiceHandle) -> bool { false }
fn guarded(candidate: Option<VoiceHandle>) {
    match candidate {
        .Some(voice) when check(voice) => ()
        _ => ()
    }
}
"#,
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::GuardBoundAffineRead { .. }
        ))
    ));
}

#[test]
fn if_let_guard_cannot_move_an_affine_pattern_binding() {
    let fixture = fixture(
        r#"
fn check(voice: VoiceHandle) -> bool { false }
fn guarded(candidate: Option<VoiceHandle>) -> bool {
    if let .Some(voice) = candidate when check(voice) { true } else { false }
}
"#,
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::GuardBoundAffineRead { .. }
        ))
    ));
}

#[test]
fn while_let_guard_cannot_move_an_affine_pattern_binding() {
    let fixture = fixture(
        r#"
fn check(voice: VoiceHandle) -> bool { false }
flow guarded(queue: Vec<VoiceHandle>) {
    while let .Some(voice) = queue.pop_front() when check(voice) {}
}
"#,
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::GuardBoundAffineRead { .. }
        ))
    ));
}

#[test]
fn guard_cannot_mutate_a_pattern_binding_through_a_selected_place() {
    let fixture = fixture(
        r#"
fn guarded(candidate: Option<Vec<i64>>) -> bool {
    match candidate {
        .Some(items) when match items.pop_front() {
            .Some(_) => true
            .None => false
        } => true
        _ => false
    }
}
"#,
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::GuardBoundMutation { .. }
        ))
    ));
}

#[test]
fn guard_copies_only_the_referenced_unrestricted_pattern_binding() {
    let fixture = fixture(
        r#"
fn check(count: i64) -> bool { count > 0i64 }
fn consume(voice: VoiceHandle) {}
fn guarded(candidate: (VoiceHandle, i64)) {
    match candidate {
        (voice, count) when check(count) => consume(voice)
        _ => ()
    }
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("guard copies only the i64 binding");
    let project = fixture.project.analysis_view().expect("accepted HIR");
    let module = project.modules().next().expect("root module").1;
    let guard = module
        .expressions()
        .find_map(|(_, expression)| match expression.kind() {
            arcweft_lang_hir::expr::HirExprKind::Match(value) => value.arms()[0].guard(),
            _ => None,
        })
        .expect("selected guard");
    let locals = report
        .checked_local_uses()
        .guard_copy_locals(guard)
        .collect::<Vec<_>>();
    let [local] = locals.as_slice() else {
        panic!("only the read count binding has a Copy obligation")
    };
    assert_eq!(
        report.local(*local).map(|value| value.ty()),
        Some(&TypeKind::I64)
    );
    assert!(
        report
            .checked_local_uses()
            .value_transfers()
            .any(|(_, row)| row.local() == *local && row.mode() == CheckedLocalReadMode::Copy)
    );
}

#[test]
fn guard_function_binding_has_an_exact_runtime_copy_obligation() {
    let fixture = fixture(
        r#"
fn check(callback: (i64 -> bool effects {})) -> bool { callback(1i64) }
fn guarded(candidate: Option<(i64 -> bool effects {})>) {
    match candidate {
        .Some(callback) when check(callback) => ()
        _ => ()
    }
}
"#,
        None,
    );
    let report =
        analyze(&fixture).expect("Function binding requires a selected runtime Copy proof");
    let project = fixture.project.analysis_view().expect("accepted HIR");
    let module = project.modules().next().expect("root module").1;
    let guard = module
        .expressions()
        .find_map(|(_, expression)| match expression.kind() {
            arcweft_lang_hir::expr::HirExprKind::Match(value) => value.arms()[0].guard(),
            _ => None,
        })
        .expect("selected guard");
    let locals = report
        .checked_local_uses()
        .guard_copy_locals(guard)
        .collect::<Vec<_>>();
    let [local] = locals.as_slice() else {
        panic!("one exact Function binding requires Copy")
    };
    assert!(matches!(
        report.local(*local).map(|binding| binding.ty()),
        Some(TypeKind::Function { .. })
    ));
    assert!(
        report
            .checked_local_uses()
            .value_transfers()
            .any(|(_, row)| row.local() == *local && row.mode() == CheckedLocalReadMode::Copy)
    );
}

#[test]
fn mutually_exclusive_affine_uses_are_admitted() {
    let fixture = fixture(
        r#"
fn consume(voice: VoiceHandle) {}
fn speak(voice: VoiceHandle, enabled: bool) {
    if enabled { consume(voice); } else { consume(voice); }
}
"#,
        None,
    );
    analyze(&fixture).expect("one move on each exclusive path is valid");
}

#[test]
fn affine_use_from_an_outer_binding_is_rejected_in_a_repeating_loop() {
    let fixture = fixture(
        r#"
fn consume(voice: VoiceHandle) {}
flow speak(voice: VoiceHandle, enabled: bool) {
    while enabled { consume(voice); }
}
"#,
        None,
    );
    let actual = analyze(&fixture).err();
    assert!(
        matches!(
            actual,
            Some(FinalSemanticAnalysisError::LocalUse(
                CheckedLocalUseError::RepeatedLoopMove { .. }
            ))
        ),
        "a loop cannot consume an outer affine owner on repeated iterations: {actual:?}"
    );
}

#[test]
fn affine_use_in_a_repeated_condition_is_rejected() {
    let fixture = fixture(
        r#"
fn consume(voice: VoiceHandle) -> bool { true }
flow speak(voice: VoiceHandle) {
    while consume(voice) {}
}
"#,
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::LocalUse(
            CheckedLocalUseError::RepeatedLoopMove { .. }
        ))
    ));
}

#[test]
fn deep_copy_carrier_allows_repeated_string_but_not_content() {
    let string = fixture(
        r#"
fn twice(value: String) -> (String, String) { (value, value) }
"#,
        None,
    );
    let report = analyze(&string).expect("String has an unrestricted runtime carrier");
    let copies = report
        .checked_local_uses()
        .value_transfers()
        .filter(|(_, row)| {
            report
                .local(row.local())
                .is_some_and(|binding| binding.ty() == &TypeKind::String)
        })
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 2);
    assert!(
        copies
            .iter()
            .all(|(_, row)| row.mode() == CheckedLocalReadMode::Copy)
    );

    let content = fixture(
        r#"
fn twice(value: Content) -> (Content, Content) { (value, value) }
"#,
        None,
    );
    let actual = analyze(&content).err();
    assert!(
        matches!(
            actual,
            Some(FinalSemanticAnalysisError::LocalUse(
                CheckedLocalUseError::Unavailable { .. }
            ))
        ),
        "Content may retain an affine callback and cannot be duplicated: {actual:?}"
    );
}

#[test]
fn explicit_closure_capture_moves_affine_local_at_construction() {
    let fixture = fixture(
        r#"
fn consume(voice: VoiceHandle) {}
fn speak(voice: VoiceHandle) {
    let callback = || consume(voice)
    consume(voice)
}
"#,
        None,
    );
    let actual = analyze(&fixture).err();
    assert!(
        matches!(
            actual,
            Some(FinalSemanticAnalysisError::LocalUse(
                CheckedLocalUseError::Unavailable { .. }
            ))
        ),
        "a second use after closure capture must fail: {actual:?}"
    );
}

#[test]
fn repeated_pipe_placeholder_of_affine_content_is_rejected() {
    let fixture = fixture(
        r#"
fn pair(value: Content) -> (Content, Content) { value |> (^, ^) }
"#,
        None,
    );
    let actual = analyze(&fixture).err();
    assert!(
        matches!(
            actual,
            Some(FinalSemanticAnalysisError::LocalUse(
                CheckedLocalUseError::SyntheticUnavailable { .. }
            ))
        ),
        "a pipe binding with an affine carrier is single-use: {actual:?}"
    );
}

#[test]
fn whole_pattern_binding_cannot_alias_an_affine_subbinding() {
    let fixture = fixture(
        r#"
fn speak(pair: (VoiceHandle, i32)) {
    let whole (voice, _) = pair
}
"#,
        None,
    );
    let actual = analyze(&fixture).err();
    assert!(
        matches!(
            actual,
            Some(FinalSemanticAnalysisError::LocalUse(
                CheckedLocalUseError::PatternOverlap { .. }
            ))
        ),
        "whole and nested affine bindings cannot own the same handle: {actual:?}"
    );
}

#[test]
fn capacity_receiver_admission_preserves_complete_places_and_computed_diagnostics() {
    for (index, source) in [
        r#"
struct Queue { items: Vec<i64> }
struct Boxed { queue: Queue }
flow main() -> i64 {
    let boxed = Boxed { queue = Queue { items = [1i64] } }
    let popped = boxed.queue.items.pop_front()
    return 42i64
}
"#,
        r#"
struct Queue { items: Vec<i64> }
flow main() -> i64 {
    let queues = [Queue { items = [1i64] }]
    let popped = queues[0i64].items.pop_front()
    return 42i64
}
"#,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = fixture(source, None);
        let report = analyze(&fixture).expect("receiver analysis keeps valid local-use topology");
        if index == 0 {
            let access = report
                .checked_local_uses()
                .rows()
                .filter_map(|(_, access)| access.place_access())
                .find(|access| {
                    access.mode() == crate::final_analysis::CheckedLocalPlaceMode::Mutate
                })
                .expect("stored receiver has a sealed mutation address");
            let fields = access.place().fields();
            assert_eq!(fields.len(), 2);
            assert!(
                fields
                    .iter()
                    .all(|field| field.runtime_field().unwrap().zero_based() == 0)
            );
            assert_eq!(fields[0].field_type(), fields[1].owner_type());
        }
    }
}

#[test]
fn whole_record_replacement_retains_static_moved_child_cleanup_contours() {
    use crate::final_analysis::{CheckedPlaceDisplacement, CheckedPlaceInitialization};
    let world = fixture(
        "struct Pair { style: RichTextStyle, other: i64 }\nfn root(input: Pair, replacement: Pair) { let mut pair = input; let moved = pair.style; pair = replacement; () }",
        None,
    );
    let report = analyze(&world).unwrap();
    let assignment = report
        .checked_local_uses()
        .rows()
        .filter_map(|(_, access)| access.place_access())
        .find(|access| access.mode() == crate::final_analysis::CheckedLocalPlaceMode::Assign)
        .unwrap();
    let Some(CheckedPlaceDisplacement::Reachable {
        initialization,
        fields,
    }) = assignment.displacement()
    else {
        panic!("post-RHS contour")
    };
    assert_eq!(*initialization, CheckedPlaceInitialization::Initialized);
    let [field] = fields.as_ref() else {
        panic!("one moved child")
    };
    assert_eq!(
        field.initialization(),
        CheckedPlaceInitialization::Uninitialized
    );
    assert_eq!(field.fields().len(), 1);
    assert_eq!(field.fields()[0].runtime_field().unwrap().zero_based(), 0);
}
