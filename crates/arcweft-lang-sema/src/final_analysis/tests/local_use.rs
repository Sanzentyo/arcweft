use arcweft_lang_hir::expr::HirExprKind;

use crate::final_analysis::{
    CheckedLocalCopyEvidence, CheckedLocalReadMode, CheckedLocalUseError,
    CheckedLocalUseInstantiation, CheckedLocalUseSite, FinalSemanticAnalysisError,
};
use crate::types::TypeKind;

use super::{analyze, character_nominal_fixture, fixture};

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
            .rows()
            .filter(|(_, row)| row.mode() == CheckedLocalReadMode::Copy)
            .count(),
        2
    );
    assert!(
        closed
            .rows()
            .all(|(site, _)| report.checked_local_uses().use_at(site).is_none())
    );
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
            .rows()
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
            .rows()
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
            .rows()
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
            .rows()
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
        .rows()
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
        report.checked_local_uses().rows().any(|(site, row)| {
            site == CheckedLocalUseSite::Capture {
                owner: callback,
                local: row.local(),
            } && row.mode() == CheckedLocalReadMode::Move
                && report
                    .local(row.local())
                    .is_some_and(|binding| matches!(binding.ty(), TypeKind::StageActorHandle(_)))
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
        .rows()
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
    items.pop()
    items.pop()
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
            .rows()
            .all(|(_, row)| row.local() != mutable_vec)
    );
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
            .rows()
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
            .rows()
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
        .rows()
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
fn unsupported_capacity_receivers_keep_source_diagnostic_authority() {
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
        let actual = analyze(&fixture).err();
        assert!(
            matches!(
                (index, actual.as_ref()),
                (
                    0,
                    Some(FinalSemanticAnalysisError::UnknownCallTarget { .. })
                ) | (1, None)
            ),
            "unsupported receiver {index} must not poison final local-use topology: {actual:?}"
        );
    }
}
