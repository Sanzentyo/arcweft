//! Surface parser for `.awft` source files.
//!
//! This crate owns syntax-level parsing only. It keeps enough structure for
//! formatter, diagnostics, and later HIR lowering, while deliberately avoiding
//! type resolution or runtime semantics.

mod ast;
mod check;
mod expr;
mod lower;
mod parser;
mod resolve;
mod symbols;
mod text;
mod types;

pub use ast::{
    Attribute, AwaitBranch, AwaitBranchKind, BorrowBlock, CallableItem, CallableKind, ChoiceAction,
    ChoiceBlock, ChoiceItem, ChoiceMatchArm, ChoiceOption, ChoicePlan, ChoicePlanItem, ContentCall,
    ContractClause, DialogueToken, EntityDeclItem, EntityDeclKind, EntityRef, EnumItem,
    EnumVariant, ExternModItem, Flow, FlowItem, FlowKind, ForBlock, FunctionItem, FunctionKind,
    HookItem, IfBlock, IfLetBlock, ImplItem, ImplMember, Item, LineOptions, LinePlan, LinePlanItem,
    LoopBlock, MatchArm, MatchBlock, MemoFn, ModuleDecl, ParserItem, Pattern, RecordPatternField,
    ScenarioCommand, ScopeBlock, ScopeExprBlock, SelectBlock, SelectBranch, SelectBranchHead,
    SourceItem, SourceLocaleBlock, SpeakerLine, StateField, StateItem, Stmt, StructField,
    StructItem, SyntaxTree, TextRange, TraitItem, TraitMember, TypeAliasItem, UseItem,
    VariantPatternPayload, Visibility, WhileBlock, WhileLetBlock, WikiLink,
};
pub use check::{
    EntityKind, TypeCheckEnv, TypeCheckError, TypeCheckReadinessError, TypeKind, typecheck_hir,
    validate_typecheck_ready,
};
pub use expr::{BinaryOp, ComputationBlockKind, Expr, Literal, Placeholder, UnaryOp, parse_expr};
pub use lower::{
    HirAwait, HirAwaitBranch, HirBorrow, HirChoice, HirChoiceOption, HirDialogue, HirFlow,
    HirFlowItem, HirFor, HirIf, HirIfLet, HirLoop, HirLowerError, HirMatch, HirMatchArm, HirModule,
    HirScope, HirSelect, HirSelectBranch, HirTopLevelDecl, HirWhile, HirWhileLet, lower_to_hir,
};
pub use parser::{ParseError, RecoverySuggestion, parse_source, parse_stub};
pub use resolve::{NameRegistry, NameResolutionError, registry_from_hir, validate_hir_references};
pub use symbols::{SymbolUse, SymbolUseKind, collect_symbol_uses};
pub use text::parse_dialogue_tokens;
pub use types::{
    FnSignature, LifetimeName, TypeParseError, TypeRef, parse_fn_signature, parse_type_ref,
};

#[cfg(test)]
mod tests {
    use super::{
        AwaitBranchKind, BinaryOp, CallableKind, ChoiceAction, ChoiceItem, ChoicePlanItem,
        ComputationBlockKind, ContractClause, DialogueToken, EntityDeclKind, EntityKind, EntityRef,
        Expr, FlowItem, FlowKind, FunctionKind, HirFlowItem, HirTopLevelDecl, ImplMember, Item,
        LinePlanItem, Literal, NameRegistry, Pattern, Placeholder, SelectBranchHead, Stmt,
        SymbolUseKind, TraitMember, TypeCheckEnv, TypeKind, TypeRef, UnaryOp,
        VariantPatternPayload, Visibility, collect_symbol_uses, lower_to_hir,
        parse_dialogue_tokens, parse_fn_signature, parse_source, parse_stub, parse_type_ref,
        registry_from_hir, typecheck_hir, validate_hir_references, validate_typecheck_ready,
    };

    fn variant_tuple_binding(pattern: &Pattern, variant: &str, binding: &str) -> bool {
        matches!(
            pattern,
            Pattern::Variant {
                path: None,
                name,
                payload: Some(VariantPatternPayload::Tuple(items)),
            } if name == variant && matches!(items.as_slice(), [Pattern::Ident(name)] if name == binding)
        )
    }

    #[test]
    fn stub_is_now_real_source_parser() {
        let tree = parse_stub("alice: おはよう。[p]").expect("speaker line parses");
        assert_eq!(tree.items().len(), 1);
        assert!(matches!(
            &tree.items()[0],
            Item::FlowItem(FlowItem::SpeakerLine(_))
        ));
    }

    #[test]
    fn parses_module_use_and_pub_flow() {
        let tree = parse_source(
            r"
mod game::routes::opening

use game::prelude::*
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    @bg #asset.bg.room fade=300ms
    include #frag.alice_enters
}
",
        )
        .expect("module, use, and flow parse");

        assert_eq!(
            tree.module().expect("module").path(),
            "game::routes::opening"
        );
        assert_eq!(tree.uses().len(), 1);
        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow item");
        };
        assert_eq!(flow.visibility(), Some(Visibility::Public));
        assert_eq!(flow.kind(), FlowKind::Flow);
        assert_eq!(flow.id().expect("flow id").body(), "flow.opening");
        assert_eq!(flow.body().len(), 2);
        assert!(
            matches!(&flow.body()[0], FlowItem::ScenarioCommand(command) if command.args().len() == 2)
        );
        assert!(matches!(&flow.body()[1], FlowItem::Include(_)));
    }

    #[test]
    fn parses_flow_contracts_before_body_block() {
        let tree = parse_source(
            r"
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError>
requires delta >= -100 && delta <= 100
ensures check result.affection[character] >= 0
requires progress in 0.0..=1.0
effects { asset.read, ui.show }
ensures no_effect network.request
{
    goto #flow.title
}
",
        )
        .expect("flow contracts parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        assert_eq!(flow.contracts().len(), 5);
        assert!(matches!(
            &flow.contracts()[0],
            ContractClause::Requires {
                expr: Expr::Binary { .. },
                ..
            }
        ));
        assert!(matches!(
            &flow.contracts()[1],
            ContractClause::Ensures {
                mode: Some(mode),
                expr: Expr::Binary { .. },
            } if mode == "check"
        ));
        assert!(matches!(&flow.contracts()[4], ContractClause::NoEffect(_)));
        assert!(matches!(&flow.body()[0], FlowItem::Stmt(Stmt::Goto(_))));
    }

    #[test]
    fn parses_documented_contract_clauses_and_logical_ops() {
        let tree = parse_source(
            r"
pub fn add_affection(character: Ref<Character>, delta: i32)(state: GameState) -> GameState
requires delta >= -100 && delta <= 100
ensures no_effect network.request
invariant affection_bounds_ok
reads state.affection[character]
modifies state.affection[character]
assume external_plugin_is_deterministic
{
    state
}
",
        )
        .expect("documented contracts parse");

        let Item::Function(function) = &tree.items()[0] else {
            panic!("expected function item");
        };
        assert_eq!(function.contracts().len(), 6);
        assert!(matches!(
            &function.contracts()[0],
            ContractClause::Requires {
                expr: Expr::Binary {
                    op: BinaryOp::And,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            &function.contracts()[1],
            ContractClause::NoEffect(Expr::Path(path)) if path == "network.request"
        ));
        assert!(matches!(
            &function.contracts()[2],
            ContractClause::Invariant {
                expr: Expr::Path(path),
                ..
            } if path == "affection_bounds_ok"
        ));
        assert!(matches!(&function.contracts()[3], ContractClause::Reads(_)));
        assert!(matches!(
            &function.contracts()[5],
            ContractClause::Assume {
                expr: Expr::Path(path)
            } if path == "external_plugin_is_deterministic"
        ));
    }

    #[test]
    fn parses_scenario_command_args_as_expressions() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    @show alice normal at=right fade=220ms
}
",
        )
        .expect("scenario command args parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::ScenarioCommand(command) = &flow.body()[0] else {
            panic!("expected scenario command");
        };
        assert_eq!(command.name(), "show");
        assert!(matches!(&command.args()[0], Expr::Path(path) if path == "alice"));
        assert!(matches!(
            &command.args()[2],
            Expr::NamedArg { name, value } if name == "at" && matches!(value.as_ref(), Expr::Path(path) if path == "right")
        ));
        assert!(matches!(
            &command.args()[3],
            Expr::NamedArg { name, value } if name == "fade" && matches!(value.as_ref(), Expr::Literal(_))
        ));
    }

    #[test]
    fn parses_fragment_as_flow_like_body() {
        let tree = parse_source(
            r"
pub fragment #frag.alice_enters alice_enters: FlowFragment {
    @show alice normal at=right fade=220ms
    alice: おはよう。[p]
}
",
        )
        .expect("fragment parses");

        let Item::Flow(fragment) = &tree.items()[0] else {
            panic!("expected fragment as flow-like item");
        };
        assert_eq!(fragment.kind(), FlowKind::Fragment);
        assert_eq!(
            fragment.id().expect("fragment id").body(),
            "frag.alice_enters"
        );
        assert!(matches!(&fragment.body()[0], FlowItem::ScenarioCommand(_)));
        assert!(matches!(&fragment.body()[1], FlowItem::SpeakerLine(_)));
    }

    #[test]
    fn parses_choice_block_inside_flow() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }
}
"#,
        )
        .expect("choice block parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Choice(choice) = &flow.body()[0] else {
            panic!("expected choice");
        };
        assert_eq!(
            choice.id().expect("choice id").body(),
            "choice.opening.first"
        );
        assert_eq!(choice.options().len(), 2);
        assert_eq!(choice.options()[0].label(), "聞いてみる");
    }

    #[test]
    fn rejects_old_at_choice_syntax() {
        let errors = parse_source(
            r#"
flow #flow.opening opening {
    @choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    }
}
"#,
        )
        .expect_err("old @choice syntax is rejected");

        assert!(errors[0].message().contains("@choice"));
        assert_eq!(errors[0].expected(), &["choice #choice.id { ... }"]);
    }

    #[test]
    fn parses_delimited_entity_refs_with_semantic_hashes() {
        let tree = parse_source(
            r"
flow #<flow.alice_intro@sem:b3_9f2a1c> opening {
    include #<frag.alice_enters@sem:f0_00aa>
}
",
        )
        .expect("delimited entity refs parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let id = flow.id().expect("flow id");
        assert!(id.is_delimited());
        assert_eq!(id.body(), "flow.alice_intro@sem:b3_9f2a1c");
        let FlowItem::Include(fragment) = &flow.body()[0] else {
            panic!("expected include");
        };
        assert!(fragment.is_delimited());
        assert_eq!(fragment.body(), "frag.alice_enters@sem:f0_00aa");
    }

    #[test]
    fn parses_colon_speaker_with_indented_line_plan() {
        let tree = parse_source(
            r"
alice(voice=auto, face=smile):
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
with:
    at(0.42s): alice.stage.face(worried)
    cancel on input .SkipLine => continue
    out (actor, voice)
",
        )
        .expect("speaker line with line plan parses");

        let Item::FlowItem(FlowItem::SpeakerLine(line)) = &tree.items()[0] else {
            panic!("expected top-level speaker flow item");
        };
        assert_eq!(line.speaker(), "alice");
        assert_eq!(line.content().tokens().len(), 4);
        assert!(line.plan().is_some());
        let plan = line.plan().expect("line plan");
        assert!(matches!(&plan.items()[0], LinePlanItem::TimedCue { .. }));
        let LinePlanItem::CancelRule(rule) = &plan.items()[1] else {
            panic!("expected cancel rule");
        };
        assert_eq!(rule.trigger(), "input .SkipLine");
        assert!(matches!(rule.action(), [Stmt::Continue { label: None }]));
        assert!(matches!(&plan.items()[2], LinePlanItem::Out(_)));
    }

    #[test]
    fn parses_bracket_speaker_call_with_with_colon_plan() {
        let tree = parse_source(
            r"
alice[
    おはよう。[p]
]
with:
    at(0.42s): alice.stage.face(smile)
",
        )
        .expect("bracket speaker call with with-colon plan parses");

        let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
            panic!("expected content call");
        };
        assert_eq!(call.callee(), "alice");
        assert_eq!(call.content().raw(), "おはよう。[p]");
        let plan = call.plan().expect("line plan");
        assert!(matches!(
            &plan.items()[0],
            LinePlanItem::TimedCue {
                anchor: Expr::Literal(_),
                body: Expr::MethodCall { .. }
            }
        ));
    }

    #[test]
    fn parses_colon_form_with_inline_bracket_content() {
        let tree = parse_source("alice(voice=auto):[今日は少しだけ。[p]]")
            .expect("colon inline bracket content parses as dialogue text");

        let Item::FlowItem(FlowItem::SpeakerLine(line)) = &tree.items()[0] else {
            panic!("expected speaker line");
        };
        assert_eq!(line.speaker(), "alice");
        assert_eq!(line.args(), Some("voice=auto"));
        assert_eq!(line.content().raw(), "[今日は少しだけ。[p]]");
    }

    #[test]
    fn typechecks_character_method_and_speaker_preset_dialogue_callees() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    alice.say(voice=auto)[おはよう。[p]]
    #<character.alice>.say(voice=auto)[おはよう。[p]]
    alice2(voice=auto): おはよう。[p]
    alice2(voice=auto)[おはよう。[p]]
}
",
        )
        .expect("dialogue callee fixture parses");

        let hir = lower_to_hir(&tree).expect("dialogue callee fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
            .with_symbol("alice2", TypeKind::Named("SpeakerPreset".to_owned()));

        typecheck_hir(&hir, &env).expect("dialogue callee forms typecheck");
        let flow = &hir.flows()[0];
        let HirFlowItem::Dialogue(delimited) = &flow.body()[1] else {
            panic!("expected delimited character dialogue");
        };
        assert_eq!(
            delimited.id().expect("generated delimited line id").body(),
            "say.opening.alice.002"
        );
    }

    #[test]
    fn parses_bare_block_after_dialogue_as_lexical_scope() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    alice.say()[おはよう。[p]] {
        let tmp = route_title(state.route)
        log info "tmp={tmp}" { tmp = tmp }
    }
}
"#,
        )
        .expect("dialogue followed by bare lexical block parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let [FlowItem::ContentCall(call), FlowItem::Block(block)] = flow.body() else {
            panic!("expected dialogue call followed by lexical block");
        };
        assert!(call.plan().is_none());
        assert_eq!(block.body().len(), 2);

        let hir = lower_to_hir(&tree).expect("dialogue plus bare block lowers");
        let [HirFlowItem::Dialogue(dialogue), HirFlowItem::Block(block)] = hir.flows()[0].body()
        else {
            panic!("expected HIR dialogue followed by lexical block");
        };
        assert!(dialogue.plan().is_none());
        assert_eq!(block.body().len(), 2);
    }

    #[test]
    fn rejects_at_bracket_timed_cue_as_raw_line_plan_item() {
        let tree = parse_source(
            r"
alice[おはよう。[p]]
with:
    at(0.42s)[alice.stage.face(worried)]
",
        )
        .expect("old at bracket cue parses lossily");

        let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
            panic!("expected content call");
        };
        let plan = call.plan().expect("line plan");
        assert!(matches!(&plan.items()[0], LinePlanItem::Raw(_)));

        let hir = lower_to_hir(&tree).expect("lossy line plan still lowers");
        let errors = validate_typecheck_ready(&hir).expect_err("old at bracket cue is rejected");
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("raw expression")
                    && error.message().contains("at(0.42s)["))
        );
    }

    #[test]
    fn parses_choice_option_with_condition() {
        let tree = parse_source(
            r#"
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" if state.affection[#character.alice] >= 3 -> #flow.alice_intro
}
"#,
        )
        .expect("choice option condition parses");

        let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
            panic!("expected choice");
        };
        let option = &choice.options()[0];
        assert_eq!(
            option.id().expect("choice option id").body(),
            "choice.opening.listen"
        );
        assert!(matches!(option.condition(), Some(Expr::Binary { .. })));
        assert_eq!(
            option.target().expect("goto target").body(),
            "flow.alice_intro"
        );
    }

    #[test]
    fn parses_choice_option_block_and_value_output() {
        let tree = parse_source(
            r#"
choice #choice.opening.first {
    let can_enter_alice = state.affection[#character.alice] >= 3

    option #choice.opening.listen {
        label = "聞いてみる"
        enabled = can_enter_alice
        visible = true
        order = 10
        ui {
            disabled_reason = if can_enter_alice { None } else { Some("好感度が足りません") }
            badge = if can_enter_alice { None } else { Some("LOCKED") }
        }
        select {
            goto #flow.alice_intro
        }
    }

    #choice.opening.silent "黙っている" => #flow.quiet_intro
}
"#,
        )
        .expect("rich choice block parses");

        let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
            panic!("expected choice");
        };
        assert_eq!(choice.items().len(), 3);
        assert_eq!(choice.options().len(), 2);
        let option = &choice.options()[0];
        assert_eq!(option.label(), "聞いてみる");
        assert!(option.enabled().is_some());
        assert!(option.visible().is_some());
        assert!(option.order().is_some());
        assert_eq!(option.ui_fields().len(), 2);
        assert_eq!(
            option.target().expect("goto target").body(),
            "flow.alice_intro"
        );
        assert!(matches!(
            choice.options()[1].action(),
            ChoiceAction::Out(Expr::EntityRef(entity)) if entity.body() == "flow.quiet_intro"
        ));
    }

    #[test]
    fn parses_dynamic_choice_options_from_for_loop() {
        let tree = parse_source(
            r"
choice #choice.opening.routes {
    for route in opening_routes(state) {
        option route.choice_id {
            label = route.label
            enabled = route.enabled
            select { goto route.target }
        }
    }
}
",
        )
        .expect("dynamic choice options parse");

        let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
            panic!("expected choice");
        };
        assert!(matches!(&choice.items()[0], ChoiceItem::For { .. }));
        assert_eq!(choice.options().len(), 1);
        assert!(choice.options()[0].id_expr().is_some());
    }

    #[test]
    fn parses_choice_match_items_and_collects_arm_options() {
        let tree = parse_source(
            r#"
choice #choice.opening.first {
    match state.route_override {
        .Some(route) when route_enabled => {
            .listen "聞いてみる" -> #flow.alice_intro
        }
        _ => {
            .silent "黙っている" -> #flow.quiet_intro
        }
    }
}
"#,
        )
        .expect("choice match item parses");

        let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
            panic!("expected choice");
        };
        let ChoiceItem::Match { expr, arms } = &choice.items()[0] else {
            panic!("expected choice match item");
        };
        assert!(matches!(expr, Expr::Path(path) if path == "state.route_override"));
        assert_eq!(arms.len(), 2);
        assert!(arms[0].guard().is_some());
        assert!(matches!(
            arms[0].items().first(),
            Some(ChoiceItem::Option(option)) if option.label() == "聞いてみる"
        ));
        assert_eq!(choice.options().len(), 2);
        assert_eq!(
            choice.options()[0].target().expect("listen target").body(),
            "flow.alice_intro"
        );
        assert_eq!(
            choice.options()[1].target().expect("silent target").body(),
            "flow.quiet_intro"
        );

        let hir = lower_to_hir(&tree).expect("choice match lowers");
        validate_typecheck_ready(&hir).expect("choice match is typecheck-ready");
        let env = TypeCheckEnv::new()
            .with_symbol(
                "state.route_override",
                TypeKind::Named("Option<String>".to_owned()),
            )
            .with_symbol("route_enabled", TypeKind::Bool);
        typecheck_hir(&hir, &env).expect("choice match options typecheck");
    }

    #[test]
    fn choice_body_raw_items_are_not_typecheck_ready() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    choice #choice.opening.first {
        unknown choice body syntax
    }
}
",
        )
        .expect("raw choice body item is preserved");
        let hir = lower_to_hir(&tree).expect("choice with raw item lowers");
        let errors = validate_typecheck_ready(&hir).expect_err("raw choice item is rejected");

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("raw expression")
                    && error.message().contains("unknown choice body syntax"))
        );
    }

    #[test]
    fn parses_choice_plan_option_in_sugar_label_key_and_value() {
        let tree = parse_source(
            r#"
choice #choice.opening.routes {
    option route in opening_routes(state) {
        id = route.choice_id
        label(id=#text.choice.opening.route) = route.label
        value = route.target
        enabled = route.enabled
        select { out route.target }
    }
}
with {
    window = #choice_window.main
    layout = vertical
    default_focus = #choice.opening.listen
    timeout 10s { select #choice.opening.silent }
    cancel on input .BackToTitle { return Ok(FlowExit::Goto(#flow.title)) }
    on select selected { log info "selected {id:?}" { id = selected.id } }
}
"#,
        )
        .expect("choice plan and option-in sugar parse");

        let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
            panic!("expected choice");
        };
        let plan = choice.plan().expect("choice plan");
        assert!(
            matches!(&plan.items()[0], ChoicePlanItem::Option { name, .. } if name == "window")
        );
        assert!(matches!(&plan.items()[3], ChoicePlanItem::Timeout { .. }));
        assert!(matches!(&plan.items()[4], ChoicePlanItem::Cancel { .. }));
        assert!(matches!(&plan.items()[5], ChoicePlanItem::OnSelect { .. }));
        assert!(matches!(
            &plan.items()[3],
            ChoicePlanItem::Timeout { body, .. }
                if matches!(body.first(), Some(Stmt::Select(Expr::EntityRef(_))))
        ));
        assert!(matches!(
            &plan.items()[4],
            ChoicePlanItem::Cancel { body, .. }
                if matches!(body.first(), Some(Stmt::Return(Expr::Call { .. })))
        ));
        assert!(matches!(
            &plan.items()[5],
            ChoicePlanItem::OnSelect { body, .. }
                if matches!(body.first(), Some(Stmt::Expr(Expr::Call { .. })))
        ));
        assert!(matches!(&choice.items()[0], ChoiceItem::For { .. }));
        let option = &choice.options()[0];
        assert!(option.label_text_key().is_some());
        assert!(option.value().is_some());
        assert!(matches!(option.action(), ChoiceAction::Out(_)));
    }

    #[test]
    fn typechecks_dynamic_choice_option_fields_in_for_sugar() {
        let tree = parse_source(
            r"
choice #choice.opening.routes {
    option route in opening_routes(state) {
        id = route.choice_id
        label(id=#text.choice.opening.route) = route.label
        value = route.target
        enabled = route.enabled
        visible = route.visible
        order = route.order

        ui {
            disabled_reason = route.disabled_reason
            badge = route.badge
        }

        select { out route.target }
    }
}
",
        )
        .expect("dynamic choice option fixture parses");

        let hir = lower_to_hir(&tree).expect("dynamic choice option fixture lowers");
        validate_typecheck_ready(&hir).expect("dynamic choice option fixture is typecheck-ready");
        let env = TypeCheckEnv::new()
            .with_symbol("state", TypeKind::Named("GameState".to_owned()))
            .with_function(
                "opening_routes",
                TypeKind::Named("List<RouteChoice>".to_owned()),
            );
        typecheck_hir(&hir, &env).expect("dynamic choice option fields typecheck");
    }

    #[test]
    fn rejects_dynamic_id_in_compact_choice_arm() {
        let tree = parse_source(
            r#"
choice #choice.opening.routes {
    route.choice_id "Dynamic label" -> #flow.alice_intro
}
"#,
        )
        .expect("dynamic compact choice arm is preserved for recovery");

        let hir = lower_to_hir(&tree).expect("choice with dynamic compact arm lowers");
        let errors =
            validate_typecheck_ready(&hir).expect_err("dynamic compact arm is not typecheck-ready");

        assert!(
            errors.iter().any(|error| error
                .message()
                .contains("raw expression is not ready for type checking")),
            "expected raw choice item diagnostic, got {errors:?}"
        );
    }

    #[test]
    fn typechecks_choice_plan_structured_bodies() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    }
    with {
        timeout 10s { select #choice.opening.listen }
        cancel on input .BackToTitle { return Ok(FlowExit::Goto(#flow.title)) }
        on select selected { log info "selected {id:?}" { id = selected.id } }
    }
}
"#,
        )
        .expect("choice plan parses");
        let hir = lower_to_hir(&tree).expect("choice plan lowers");
        validate_typecheck_ready(&hir).expect("choice plan bodies have structured expressions");
        let env = TypeCheckEnv::new()
            .with_function("Ok", TypeKind::Named("Result".to_owned()))
            .with_function("FlowExit::Goto", TypeKind::Named("FlowExit".to_owned()))
            .with_function("log.info", TypeKind::Unit);
        typecheck_hir(&hir, &env).expect("choice plan bodies typecheck");
    }

    #[test]
    fn typechecks_choice_option_select_block_statements() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    choice #choice.opening.first {
        option #choice.opening.listen {
            label = "聞いてみる"
            select {
                if can_emit {
                    emit GameEvent::ChoiceSelected { id = #choice.opening.listen }
                }
                match selected_route {
                    .Some(route) => out route
                    _ => out #flow.title
                }
            }
        }
    }
}
"#,
        )
        .expect("choice option select block parses");
        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Choice(choice) = &flow.body()[0] else {
            panic!("expected choice");
        };
        assert!(matches!(
            choice.options()[0].action(),
            ChoiceAction::SelectBlock(statements)
                if matches!(
                    statements.as_slice(),
                    [Stmt::If { .. }, Stmt::Match { .. }]
                )
        ));

        let hir = lower_to_hir(&tree).expect("choice option select block lowers");
        validate_typecheck_ready(&hir).expect("choice option select block is typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol(
                    "GameEvent::ChoiceSelected",
                    TypeKind::Named("GameEvent".to_owned()),
                )
                .with_symbol("can_emit", TypeKind::Bool)
                .with_symbol(
                    "selected_route",
                    TypeKind::Named("Option<Ref<Flow>>".to_owned()),
                ),
        )
        .expect("choice option select block typechecks");
    }

    #[test]
    fn parses_source_locale_block() {
        let tree = parse_source(
            r"
source locale en-US {
    alice(id=#say.opening.alice.english_quote):
        Good morning.[p]
}
",
        )
        .expect("source locale block parses");

        let Item::FlowItem(FlowItem::SourceLocale(block)) = &tree.items()[0] else {
            panic!("expected source locale block");
        };
        assert_eq!(block.locale(), "en-US");
        assert_eq!(block.body().len(), 1);
    }

    #[test]
    fn lowers_named_scope_and_relative_choice_ids() {
        let tree = parse_source(
            r#"
mod crate::game::routes::opening
use self::characters::{alice}
use parent::common::{route_gate}

flow #flow.opening opening {
    scope dream {
        choice .first {
            .listen "聞いてみる" -> #flow.alice_intro
            .silent "黙っている" -> #flow.quiet_intro
        }
    }
}

flow #flow.alice_intro alice_intro {}
flow #flow.quiet_intro quiet_intro {}
"#,
        )
        .expect("named scope and relative choice ids parse");

        assert_eq!(
            tree.module().expect("module").path(),
            "crate::game::routes::opening"
        );
        assert_eq!(tree.uses()[0].tree(), "self::characters::{alice}");
        assert_eq!(tree.uses()[1].tree(), "super::common::{route_gate}");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Scope(scope) = &flow.body()[0] else {
            panic!("expected named scope");
        };
        assert_eq!(scope.name(), "dream");
        let FlowItem::Choice(choice) = &scope.body()[0] else {
            panic!("expected scoped choice");
        };
        assert!(choice.id().expect("choice id").is_relative());
        assert!(choice.options()[0].id().expect("option id").is_relative());

        let hir = lower_to_hir(&tree).expect("relative choice ids lower");
        let HirFlowItem::Scope(scope) = &hir.flows()[0].body()[0] else {
            panic!("expected HIR scope");
        };
        let HirFlowItem::Choice(choice) = &scope.body()[0] else {
            panic!("expected HIR choice");
        };
        assert_eq!(
            choice.id().expect("normalized choice id").body(),
            "choice.opening.dream.first"
        );
        assert_eq!(
            choice.options()[0]
                .id()
                .expect("normalized option id")
                .body(),
            "choice.opening.dream.first.listen"
        );

        let registry = registry_from_hir(&hir);
        validate_hir_references(&hir, &registry).expect("normalized scoped ids resolve");
        validate_typecheck_ready(&hir).expect("scoped relative ids are typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_symbol("can_enter", TypeKind::Bool),
        )
        .expect("scoped relative choice HIR typechecks");
    }

    #[test]
    fn rejects_relative_id_syntax_in_module_and_use_paths() {
        for source in ["mod .routes::opening", "use .characters::{alice}"] {
            let errors = parse_source(source).expect_err("relative module path is rejected");
            assert!(
                errors
                    .iter()
                    .any(|error| error.message().contains("relative `.suffix` ID syntax")),
                "expected relative-id diagnostic for {source:?}, got {errors:?}"
            );
        }
    }

    #[test]
    fn normalizes_parent_module_root_alias() {
        let tree = parse_source(
            r"
mod parent::shared
lazy use parent::common::{route_gate}
",
        )
        .expect("parent module root parses as alias");

        assert_eq!(tree.module().expect("module").path(), "super::shared");
        assert_eq!(tree.uses()[0].tree(), "super::common::{route_gate}");
    }

    #[test]
    fn lowers_relative_dialogue_line_options() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    scope rain {
        地の文(id=.sound):
            扉の向こうから、雨の音がした。[p]

        alice(id=.comment, text_key=.comment_text, source_locale=en-US):
            Good morning.[p]

        地の文:
            窓が小さく鳴った。[p]
    }
}
",
        )
        .expect("relative dialogue options parse");
        let hir = lower_to_hir(&tree).expect("relative dialogue options lower");
        let HirFlowItem::Scope(scope) = &hir.flows()[0].body()[0] else {
            panic!("expected HIR scope");
        };
        let HirFlowItem::Dialogue(narration) = &scope.body()[0] else {
            panic!("expected narration");
        };
        assert_eq!(
            narration.id().expect("narration id").body(),
            "say.opening.narrator.rain.sound"
        );
        assert_eq!(
            narration.text_key().expect("derived text key").body(),
            "text.opening.narrator.rain.sound"
        );

        let HirFlowItem::Dialogue(alice) = &scope.body()[1] else {
            panic!("expected alice line");
        };
        assert_eq!(
            alice.id().expect("alice line id").body(),
            "say.opening.alice.rain.comment"
        );
        assert_eq!(
            alice.text_key().expect("explicit text key").body(),
            "text.opening.alice.rain.comment_text"
        );
        assert_eq!(alice.source_locale(), Some("en-US"));

        let HirFlowItem::Dialogue(generated) = &scope.body()[2] else {
            panic!("expected generated-id narration");
        };
        assert_eq!(
            generated.id().expect("generated line id").body(),
            "say.opening.narrator.rain.001"
        );
        assert_eq!(
            generated.text_key().expect("generated text key").body(),
            "text.opening.narrator.rain.001"
        );

        let registry = registry_from_hir(&hir);
        validate_hir_references(&hir, &registry).expect("dialogue ids resolve");
        validate_typecheck_ready(&hir).expect("dialogue option IDs are typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol("地の文", TypeKind::Ref(EntityKind::Character))
                .with_symbol("alice", TypeKind::Ref(EntityKind::Character)),
        )
        .expect("relative dialogue options typecheck");
    }

    #[test]
    fn lowers_choice_expression_let_binding() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    let next_flow = choice .first {
        .listen "聞いてみる" => #flow.alice_intro
        .silent "黙っている" => #flow.quiet_intro
    }

    goto next_flow
}

flow #flow.alice_intro alice_intro {
}

flow #flow.quiet_intro quiet_intro {
}
"#,
        )
        .expect("choice expression fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected source flow");
        };
        let FlowItem::Stmt(Stmt::LetChoice { pattern, choice }) = &flow.body()[0] else {
            panic!("expected AST choice expression binding");
        };
        assert_eq!(pattern, &Pattern::Ident("next_flow".to_owned()));
        assert!(choice.id().expect("choice id").is_relative());

        let hir = lower_to_hir(&tree).expect("choice expression fixture lowers");
        let HirFlowItem::LetChoice { pattern, choice } = &hir.flows()[0].body()[0] else {
            panic!("expected HIR choice expression binding");
        };
        assert_eq!(pattern, &Pattern::Ident("next_flow".to_owned()));
        assert_eq!(
            choice.id().expect("normalized choice id").body(),
            "choice.opening.first"
        );
        assert_eq!(
            choice.options()[0]
                .id()
                .expect("normalized first option id")
                .body(),
            "choice.opening.first.listen"
        );
        assert!(matches!(
            choice.options()[0].action(),
            ChoiceAction::Out(Expr::EntityRef(entity)) if entity.body() == "flow.alice_intro"
        ));

        let registry = registry_from_hir(&hir);
        validate_hir_references(&hir, &registry).expect("choice expression refs resolve");
        validate_typecheck_ready(&hir).expect("choice expression is typecheck-ready");
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect("choice expression typechecks");
    }

    #[test]
    fn lowers_scope_expression_let_binding() {
        let tree = parse_source(
            r"
flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    let can_enter = scope alice_route_check {
        let affection_ok = state.affection[#character.alice] >= 3
        let has_key = state.inventory.contains(#item.alice_key)
        affection_ok && has_key
    }

    if can_enter {
        goto #flow.alice_intro
    }
}

flow #flow.alice_intro alice_intro {
}
",
        )
        .expect("scope expression fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected source flow");
        };
        let FlowItem::Stmt(Stmt::LetScope { pattern, scope }) = &flow.body()[0] else {
            panic!("expected AST scope expression binding");
        };
        assert_eq!(pattern, &Pattern::Ident("can_enter".to_owned()));
        assert_eq!(scope.name(), "alice_route_check");
        assert_eq!(scope.statements().len(), 2);
        assert!(scope.value().is_some());

        let hir = lower_to_hir(&tree).expect("scope expression fixture lowers");
        let HirFlowItem::LetScope { pattern, scope } = &hir.flows()[0].body()[0] else {
            panic!("expected HIR scope expression binding");
        };
        assert_eq!(pattern, &Pattern::Ident("can_enter".to_owned()));
        assert_eq!(scope.name(), "alice_route_check");
        assert_eq!(scope.statements().len(), 2);
        assert!(scope.value().is_some());

        let registry = registry_from_hir(&hir)
            .with_entity("character.alice", EntityKind::Character)
            .with_entity("item.alice_key", EntityKind::Other("item".to_owned()));
        validate_hir_references(&hir, &registry).expect("scope expression refs resolve");
        validate_typecheck_ready(&hir).expect("scope expression is typecheck-ready");
        let env = TypeCheckEnv::new()
            .with_symbol(
                "state.affection",
                TypeKind::Named("Map<Character, Int>".to_owned()),
            )
            .with_symbol("state.inventory", TypeKind::Named("Inventory".to_owned()))
            .with_method(
                TypeKind::Named("Inventory".to_owned()),
                "contains",
                TypeKind::Bool,
            )
            .with_index(
                TypeKind::Named("Map<Character, Int>".to_owned()),
                TypeKind::Int,
            );
        typecheck_hir(&hir, &env).expect("scope expression typechecks");
    }

    #[test]
    fn parses_and_typechecks_plain_block_expression_binding() {
        let tree = parse_source(
            r"
flow #flow.block_expr block_expr {
    let total = {
        let a = 1
        let b = 2
        a + b
    }
}
",
        )
        .expect("plain block expression fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::Let {
            pattern,
            expr: Expr::Block { statements, value },
        }) = &flow.body()[0]
        else {
            panic!("expected let binding with block expression");
        };
        assert_eq!(pattern, &Pattern::Ident("total".to_owned()));
        assert_eq!(statements.len(), 2);
        assert!(value.is_some());

        let hir = lower_to_hir(&tree).expect("plain block expression fixture lowers");
        validate_typecheck_ready(&hir).expect("plain block expression is typecheck-ready");
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect("plain block expression typechecks");
    }

    #[test]
    fn parses_and_typechecks_let_else_binding() {
        let tree = parse_source(
            r"
flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    let .Some(route) = state.route_override else {
        goto #flow.title
    }

    goto route
}

flow #flow.title title {
}
",
        )
        .expect("let-else fixture parses");
        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected source flow");
        };
        let FlowItem::Stmt(Stmt::LetElse {
            pattern,
            expr,
            else_body,
        }) = &flow.body()[0]
        else {
            panic!("expected structured let-else");
        };
        assert!(variant_tuple_binding(pattern, "Some", "route"));
        assert!(matches!(expr, Expr::Path(path) if path == "state.route_override"));
        assert!(matches!(else_body.as_slice(), [Stmt::Goto(_)]));

        let hir = lower_to_hir(&tree).expect("let-else fixture lowers");
        let registry = registry_from_hir(&hir);
        validate_hir_references(&hir, &registry).expect("let-else refs resolve");
        validate_typecheck_ready(&hir).expect("let-else is typecheck-ready");
        let env = TypeCheckEnv::new().with_symbol(
            "state.route_override",
            TypeKind::Named("Option<Ref<Flow>>".to_owned()),
        );
        typecheck_hir(&hir, &env).expect("let-else typechecks and binds route");
    }

    #[test]
    fn typecheck_rejects_non_diverging_let_else() {
        let tree = parse_source(
            r"
flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    let .Some(route) = state.route_override else {
        #flow.title
    }
}

flow #flow.title title {
}
",
        )
        .expect("non-diverging let-else fixture parses");
        let hir = lower_to_hir(&tree).expect("non-diverging let-else fixture lowers");
        let env = TypeCheckEnv::new().with_symbol(
            "state.route_override",
            TypeKind::Named("Option<Ref<Flow>>".to_owned()),
        );
        let errors = typecheck_hir(&hir, &env).expect_err("non-diverging let-else is rejected");
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("let-else else block must leave the current continuation")
        }));
    }

    #[test]
    fn typechecks_let_else_panic_and_fail_as_diverging() {
        for diverging in [
            r#"panic "missing route""#,
            "fail .MissingRoute",
            r#"bail "missing route""#,
        ] {
            let source = format!(
                r"
flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {{
    let .Some(route) = state.route_override else {{
        {diverging}
    }}

    goto route
}}
"
            );
            let tree = parse_source(source).expect("diverging let-else fixture parses");
            let hir = lower_to_hir(&tree).expect("diverging let-else fixture lowers");
            let env = TypeCheckEnv::new()
                .with_symbol(
                    "state.route_override",
                    TypeKind::Named("Option<Ref<Flow>>".to_owned()),
                )
                .with_symbol(".MissingRoute", TypeKind::Named("ErrorKind".to_owned()));
            typecheck_hir(&hir, &env).expect("panic/fail let-else branches diverge");
        }
    }

    #[test]
    fn parses_and_typechecks_bail_and_ensure_statements() {
        let tree = parse_source(
            r#"
flow #flow.validate validate {
    ensure score >= 0, "score must be non-negative"
    if !valid {
        bail "invalid score"
    }
    goto #flow.title
}
"#,
        )
        .expect("bail and ensure fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        assert!(matches!(
            &flow.body()[0],
            FlowItem::Stmt(Stmt::Ensure { .. })
        ));
        let hir = lower_to_hir(&tree).expect("bail and ensure fixture lowers");
        validate_typecheck_ready(&hir).expect("bail and ensure are typecheck-ready");
        let env = TypeCheckEnv::new()
            .with_symbol("score", TypeKind::Int)
            .with_symbol("valid", TypeKind::Bool);
        typecheck_hir(&hir, &env).expect("bail and ensure typecheck");
    }

    #[test]
    fn parses_and_typechecks_result_computation_block_binding() {
        let tree = parse_source(
            r#"
flow #flow.compute compute {
    let route = result {
        let id = parse_choice_id(raw)?
        ensure id_valid, "choice id must be valid"
        Ok(#flow.title)
    }
    goto #flow.title
}
"#,
        )
        .expect("result computation block fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::Let {
            expr:
                Expr::ComputationBlock {
                    kind,
                    statements,
                    value: Some(_),
                },
            ..
        }) = &flow.body()[0]
        else {
            panic!("expected result computation block binding");
        };
        assert_eq!(kind, &ComputationBlockKind::Result);
        assert_eq!(statements.len(), 2);

        let hir = lower_to_hir(&tree).expect("result computation block fixture lowers");
        validate_typecheck_ready(&hir).expect("result computation block is typecheck-ready");
        let env = TypeCheckEnv::new()
            .with_symbol("raw", TypeKind::String)
            .with_symbol("id_valid", TypeKind::Bool)
            .with_function(
                "parse_choice_id",
                TypeKind::Result {
                    ok: Box::new(TypeKind::String),
                    error: Box::new(TypeKind::Named("ParseError".to_owned())),
                },
            )
            .with_function(
                "Ok",
                TypeKind::Result {
                    ok: Box::new(TypeKind::Ref(EntityKind::Flow)),
                    error: Box::new(TypeKind::Named("ArcError".to_owned())),
                },
            );
        typecheck_hir(&hir, &env).expect("result computation block typechecks");
    }

    #[test]
    fn parses_and_typechecks_stream_computation_block_binding() {
        let tree = parse_source(
            r"
flow #flow.stream stream_example {
    let levels = stream {
        for frame in frames {
            yield rms(frame)
        }
    }
    goto #flow.title
}

flow #flow.title title {}
",
        )
        .expect("stream computation block fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::Let {
            expr:
                Expr::ComputationBlock {
                    kind,
                    statements,
                    value: None,
                },
            ..
        }) = &flow.body()[0]
        else {
            panic!("expected stream computation block binding");
        };
        assert_eq!(kind, &ComputationBlockKind::Stream);
        assert_eq!(statements.len(), 1);

        let hir = lower_to_hir(&tree).expect("stream computation block fixture lowers");
        validate_typecheck_ready(&hir).expect("stream computation block is typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol("frames", TypeKind::Named("Stream".to_owned()))
                .with_function("rms", TypeKind::Int),
        )
        .expect("stream computation block typechecks");
    }

    #[test]
    fn parses_and_typechecks_memo_expression_block_binding() {
        let tree = parse_source(
            r"
flow #flow.memo memo_example {
    let value = memo(scope=scene, key=(score)) {
        let next = score
        next
    }
    goto #flow.title
}

flow #flow.title title {}
",
        )
        .expect("memo expression block fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::Let {
            expr:
                Expr::MemoBlock {
                    options,
                    statements,
                    value: Some(_),
                },
            ..
        }) = &flow.body()[0]
        else {
            panic!("expected memo expression block binding");
        };
        assert_eq!(options.len(), 2);
        assert_eq!(statements.len(), 1);

        let hir = lower_to_hir(&tree).expect("memo expression block fixture lowers");
        validate_typecheck_ready(&hir).expect("memo expression block is typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol("score", TypeKind::Int)
                .with_symbol("scene", TypeKind::Named("MemoScope".to_owned())),
        )
        .expect("memo expression block typechecks");
    }

    #[test]
    fn typecheck_rejects_non_bool_ensure_condition() {
        let tree = parse_source(
            r#"
flow #flow.validate validate {
    ensure score, "score must be non-negative"
}
"#,
        )
        .expect("non-bool ensure fixture parses");
        let hir = lower_to_hir(&tree).expect("non-bool ensure fixture lowers");
        let errors = typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_symbol("score", TypeKind::Int),
        )
        .expect_err("non-bool ensure condition is rejected");

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("ensure condition"))
        );
    }

    #[test]
    fn parses_and_typechecks_while_loop() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    while loading {
        continue
    }
}
",
        )
        .expect("while fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::While(block) = &flow.body()[0] else {
            panic!("expected while block");
        };
        assert!(matches!(block.condition(), Expr::Path(path) if path == "loading"));

        let hir = lower_to_hir(&tree).expect("while fixture lowers");
        let HirFlowItem::While(block) = &hir.flows()[0].body()[0] else {
            panic!("expected HIR while block");
        };
        assert!(matches!(block.condition(), Expr::Path(path) if path == "loading"));

        validate_typecheck_ready(&hir).expect("while block is typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_symbol("loading", TypeKind::Bool),
        )
        .expect("while block typechecks");
    }

    #[test]
    fn parses_and_typechecks_if_let_guard_block() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    if let .Some(route) = state.route_override when route_available {
        goto route
    }
}
",
        )
        .expect("if-let fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::IfLet(block) = &flow.body()[0] else {
            panic!("expected if-let block");
        };
        assert!(variant_tuple_binding(block.pattern(), "Some", "route"));
        assert!(matches!(block.expr(), Expr::Path(path) if path == "state.route_override"));
        assert!(block.guard().is_some());

        let hir = lower_to_hir(&tree).expect("if-let fixture lowers");
        let HirFlowItem::IfLet(block) = &hir.flows()[0].body()[0] else {
            panic!("expected HIR if-let block");
        };
        assert!(variant_tuple_binding(block.pattern(), "Some", "route"));

        validate_typecheck_ready(&hir).expect("if-let block is typecheck-ready");
        let env = TypeCheckEnv::new()
            .with_symbol(
                "state.route_override",
                TypeKind::Named("Option<Ref<Flow>>".to_owned()),
            )
            .with_symbol("route_available", TypeKind::Bool);
        typecheck_hir(&hir, &env).expect("if-let block typechecks and binds route in body");
    }

    #[test]
    fn parses_and_typechecks_value_if_expression_binding() {
        let tree = parse_source(
            r#"
flow #flow.branching branching {
    let face = if ready {
        "smile"
    } else {
        "worried"
    }
}
"#,
        )
        .expect("value if expression fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::Let {
            pattern,
            expr:
                Expr::If {
                    condition,
                    then_branch,
                    else_branch: Some(_),
                },
        }) = &flow.body()[0]
        else {
            panic!("expected let binding with value if expression");
        };
        assert_eq!(pattern, &Pattern::Ident("face".to_owned()));
        assert!(matches!(condition.as_ref(), Expr::Path(path) if path == "ready"));
        assert!(matches!(
            then_branch.as_ref(),
            Expr::Block {
                value: Some(value),
                ..
            } if matches!(value.as_ref(), Expr::Literal(Literal::String(value)) if value == "smile")
        ));

        let hir = lower_to_hir(&tree).expect("value if expression fixture lowers");
        validate_typecheck_ready(&hir).expect("value if expression is typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_symbol("ready", TypeKind::Bool),
        )
        .expect("value if expression typechecks");
    }

    #[test]
    fn typecheck_rejects_value_if_branch_type_mismatch() {
        let tree = parse_source(
            r#"
flow #flow.branching branching {
    let face = if ready {
        "smile"
    } else {
        1
    }
}
"#,
        )
        .expect("mismatched value if fixture parses");
        let hir = lower_to_hir(&tree).expect("mismatched value if fixture lowers");
        let errors = typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_symbol("ready", TypeKind::Bool),
        )
        .expect_err("mismatched value if branches are rejected");
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("if expression branches must have the same type")
        }));
    }

    #[test]
    fn parses_and_typechecks_value_if_let_expression_binding() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    let route = if let .Some(route) = state.route_override when route_enabled {
        route
    } else {
        #flow.title
    }
}
",
        )
        .expect("value if-let expression fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::Let {
            pattern,
            expr:
                Expr::IfLet {
                    pattern: binding,
                    expr,
                    guard: Some(_),
                    then_branch,
                    else_branch: Some(_),
                },
        }) = &flow.body()[0]
        else {
            panic!("expected let binding with value if-let expression");
        };
        assert_eq!(pattern, &Pattern::Ident("route".to_owned()));
        assert!(variant_tuple_binding(binding.as_ref(), "Some", "route"));
        assert!(matches!(expr.as_ref(), Expr::Path(path) if path == "state.route_override"));
        assert!(matches!(
            then_branch.as_ref(),
            Expr::Block {
                value: Some(value),
                ..
            } if matches!(value.as_ref(), Expr::Path(path) if path == "route")
        ));

        let hir = lower_to_hir(&tree).expect("value if-let expression fixture lowers");
        validate_typecheck_ready(&hir).expect("value if-let expression is typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol(
                    "state.route_override",
                    TypeKind::Named("Option<Ref<Flow>>".to_owned()),
                )
                .with_symbol("route_enabled", TypeKind::Bool),
        )
        .expect("value if-let expression typechecks");
    }

    #[test]
    fn typecheck_rejects_value_if_let_non_bool_guard() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    let route = if let .Some(route) = state.route_override when route_count {
        route
    } else {
        #flow.title
    }
}
",
        )
        .expect("non-bool value if-let fixture parses");
        let hir = lower_to_hir(&tree).expect("non-bool value if-let fixture lowers");
        let errors = typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol(
                    "state.route_override",
                    TypeKind::Named("Option<Ref<Flow>>".to_owned()),
                )
                .with_symbol("route_count", TypeKind::Int),
        )
        .expect_err("non-bool value if-let guard is rejected");
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("if-let expression guard must have type Bool")
        }));
    }

    #[test]
    fn parses_and_typechecks_value_match_expression_binding() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    let route = match selected {
        #choice.opening.listen when can_listen => #flow.alice_intro
        #choice.opening.silent => #flow.quiet_intro
        _ => #flow.title
    }
}
",
        )
        .expect("value match expression fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::Let {
            pattern,
            expr: Expr::Match { scrutinee, arms },
        }) = &flow.body()[0]
        else {
            panic!("expected let binding with value match expression");
        };
        assert_eq!(pattern, &Pattern::Ident("route".to_owned()));
        assert!(matches!(scrutinee.as_ref(), Expr::Path(path) if path == "selected"));
        assert_eq!(arms.len(), 3);
        assert!(arms[0].guard().is_some());
        assert!(matches!(
            arms[0].value(),
            Expr::EntityRef(entity) if entity.body() == "flow.alice_intro"
        ));

        let hir = lower_to_hir(&tree).expect("value match expression fixture lowers");
        validate_typecheck_ready(&hir).expect("value match expression is typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol("selected", TypeKind::Ref(EntityKind::ChoiceOption))
                .with_symbol("can_listen", TypeKind::Bool),
        )
        .expect("value match expression typechecks");
    }

    #[test]
    fn typecheck_rejects_value_match_branch_type_mismatch() {
        let tree = parse_source(
            r#"
flow #flow.branching branching {
    let route = match selected {
        #choice.opening.listen => #flow.alice_intro
        _ => "fallback"
    }
}
"#,
        )
        .expect("mismatched value match fixture parses");
        let hir = lower_to_hir(&tree).expect("mismatched value match fixture lowers");
        let errors = typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_symbol("selected", TypeKind::Ref(EntityKind::ChoiceOption)),
        )
        .expect_err("mismatched value match arms are rejected");
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("match expression arms must have the same type")
        }));
    }

    #[test]
    fn parses_and_typechecks_postfix_try_expression() {
        let tree = parse_source(
            r"
flow #flow.trying trying {
    let config = load_config()?
}
",
        )
        .expect("postfix try fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::Let {
            pattern,
            expr: Expr::Try { expr },
        }) = &flow.body()[0]
        else {
            panic!("expected let binding with postfix try expression");
        };
        assert_eq!(pattern, &Pattern::Ident("config".to_owned()));
        assert!(matches!(expr.as_ref(), Expr::Call { .. }));

        let hir = lower_to_hir(&tree).expect("postfix try fixture lowers");
        validate_typecheck_ready(&hir).expect("postfix try expression is typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_function(
                "load_config",
                TypeKind::Result {
                    ok: Box::new(TypeKind::Named("Config".to_owned())),
                    error: Box::new(TypeKind::Named("ConfigError".to_owned())),
                },
            ),
        )
        .expect("postfix try expression typechecks");
    }

    #[test]
    fn parses_and_typechecks_prefix_try_expression() {
        let tree = parse_source(
            r"
flow #flow.trying trying {
    let config = try load_config()
}
",
        )
        .expect("prefix try fixture parses");
        let hir = lower_to_hir(&tree).expect("prefix try fixture lowers");
        validate_typecheck_ready(&hir).expect("prefix try expression is typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_function(
                "load_config",
                TypeKind::Named("Result<Config, Error>".to_owned()),
            ),
        )
        .expect("prefix try expression typechecks");
    }

    #[test]
    fn typecheck_rejects_try_on_non_result_expression() {
        let tree = parse_source(
            r"
flow #flow.trying trying {
    let bad = score?
}
",
        )
        .expect("bad try fixture parses");
        let hir = lower_to_hir(&tree).expect("bad try fixture lowers");
        let errors = typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_symbol("score", TypeKind::Int),
        )
        .expect_err("try on non-result expression is rejected");
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("`?` requires Result<T, E> or Option<T>")
        }));
    }

    #[test]
    fn typecheck_rejects_non_bool_if_let_guard() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    if let .Some(route) = state.route_override when route_count {
        goto route
    }
}
",
        )
        .expect("non-bool if-let guard fixture parses");
        let hir = lower_to_hir(&tree).expect("non-bool if-let guard fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol(
                "state.route_override",
                TypeKind::Named("Option<Ref<Flow>>".to_owned()),
            )
            .with_symbol("route_count", TypeKind::Int);
        let errors = typecheck_hir(&hir, &env).expect_err("non-bool if-let guard is rejected");
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("if-let guard must have type Bool"))
        );
    }

    #[test]
    fn parses_and_typechecks_while_let_loop() {
        let tree = parse_source(
            r"
flow #flow.events events {
    while let .Some(event) = next_event when event_ready {
        goto event
    }
}
",
        )
        .expect("while-let fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::WhileLet(block) = &flow.body()[0] else {
            panic!("expected while-let block");
        };
        assert!(variant_tuple_binding(block.pattern(), "Some", "event"));
        assert!(matches!(block.expr(), Expr::Path(path) if path == "next_event"));
        assert!(block.guard().is_some());

        let hir = lower_to_hir(&tree).expect("while-let fixture lowers");
        let HirFlowItem::WhileLet(block) = &hir.flows()[0].body()[0] else {
            panic!("expected HIR while-let block");
        };
        assert!(variant_tuple_binding(block.pattern(), "Some", "event"));

        validate_typecheck_ready(&hir).expect("while-let block is typecheck-ready");
        let env = TypeCheckEnv::new()
            .with_symbol(
                "next_event",
                TypeKind::Named("Option<Ref<Flow>>".to_owned()),
            )
            .with_symbol("event_ready", TypeKind::Bool);
        typecheck_hir(&hir, &env).expect("while-let block typechecks");
    }

    #[test]
    fn typecheck_rejects_non_bool_while_condition() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    while loading_count {
        continue
    }
}
",
        )
        .expect("non-bool while fixture parses");
        let hir = lower_to_hir(&tree).expect("non-bool while fixture lowers");
        let errors = typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_symbol("loading_count", TypeKind::Int),
        )
        .expect_err("non-bool while condition is rejected");
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("while condition must have type Bool")
        }));
    }

    #[test]
    fn parses_and_typechecks_loop_expression_binding() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    let next = 'events: loop {
        break 'events #flow.title
    }

    goto next
}

flow #flow.title title {
}
",
        )
        .expect("loop expression fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::LetLoop { pattern, block }) = &flow.body()[0] else {
            panic!("expected loop expression binding");
        };
        assert_eq!(pattern, &Pattern::Ident("next".to_owned()));
        assert_eq!(block.label(), Some("events"));
        assert!(matches!(
            block.body(),
            [FlowItem::Stmt(Stmt::Break { label: Some(label), expr: Some(Expr::EntityRef(entity)) })]
                if label == "events" && entity.body() == "flow.title"
        ));

        let hir = lower_to_hir(&tree).expect("loop expression fixture lowers");
        let HirFlowItem::LetLoop { pattern, block } = &hir.flows()[0].body()[0] else {
            panic!("expected HIR loop expression binding");
        };
        assert_eq!(pattern, &Pattern::Ident("next".to_owned()));
        assert_eq!(block.label(), Some("events"));
        assert!(matches!(
            block.body(),
            [HirFlowItem::Stmt(Stmt::Break { label: Some(label), expr: Some(Expr::EntityRef(entity)) })]
                if label == "events" && entity.body() == "flow.title"
        ));

        let registry = registry_from_hir(&hir);
        validate_hir_references(&hir, &registry).expect("loop expression refs resolve");
        validate_typecheck_ready(&hir).expect("loop expression is typecheck-ready");
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect("loop expression typechecks");
    }

    #[test]
    fn typecheck_rejects_break_value_in_while() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    while is_loading {
        break #flow.title
    }
}

flow #flow.title title {
}
",
        )
        .expect("while break-value fixture parses");
        let hir = lower_to_hir(&tree).expect("while break-value fixture lowers");
        let errors = typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_symbol("is_loading", TypeKind::Bool),
        )
        .expect_err("break expr in while is rejected");
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("break expr is allowed only in loop")
        }));
    }

    #[test]
    fn typecheck_rejects_break_outside_loop() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    break
}
",
        )
        .expect("bare break fixture parses");
        let hir = lower_to_hir(&tree).expect("bare break fixture lowers");
        let errors =
            typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("break outside loops is rejected");
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("break is only allowed inside loop")
        }));
    }

    #[test]
    fn typecheck_rejects_unresolved_control_transfer_labels() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    let next = 'events: loop {
        if done {
            break 'missing #flow.title
        }
        continue 'missing
    }

    alice[
        聞いて。[p]
    ]
    with 'line {
        cancel on input .SkipLine { out 'missing .Skipped }
    }
}

flow #flow.title title {}
",
        )
        .expect("unresolved label fixture parses");
        let hir = lower_to_hir(&tree).expect("unresolved label fixture lowers");
        validate_typecheck_ready(&hir).expect("unresolved label fixture is typecheck-ready");
        let errors = typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
                .with_symbol("done", TypeKind::Bool)
                .with_symbol(".Skipped", TypeKind::Named("LineExit".to_owned())),
        )
        .expect_err("unresolved labels are rejected");
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("break label `'missing` does not name an active loop")
        }));
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("continue label `'missing` does not name an active loop")
        }));
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("out label `'missing` does not name an active line-plan scope")
        }));
    }

    #[test]
    fn parses_character_content_call_with_brace_plan() {
        let tree = parse_source(
            r##"
alice.say(id=#say.opening.dream_hint, voice=auto)[
    今日は少しだけ、#[fmt("変な夢", color=rgb("#a8b5ff"))]を見たんだ。[p]
]
with {
    at(0.42s) { alice.stage.face(worried, crossfade=120ms) }
}
"##,
        )
        .expect("content call parses");

        let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
            panic!("expected content call");
        };
        assert_eq!(call.callee(), "alice.say");
        assert!(call.plan().is_some());
    }

    #[test]
    fn parses_documented_hook_memo_and_parser_items() {
        let tree = parse_source(
            r#"
hook #hook.choice_visible
on #choice.opening.listen
phase AfterLayout
check every frame
{
    signal #signal.choice_visible <- true
}

memo fn route_title(route: Ref<Flow>) -> String
scope = session
{
    registry.flow(route).title
}

pub parser parse_player_command: Parser<PlayerCommand, ParseError> {
    alt { "advance" => PlayerCommand::Advance }
}
"#,
        )
        .expect("hook, memo, and parser items parse");

        let Item::Hook(hook) = &tree.items()[0] else {
            panic!("expected hook item");
        };
        assert!(matches!(hook.body_statements(), [Stmt::Signal { .. }]));

        let Item::MemoFn(memo) = &tree.items()[1] else {
            panic!("expected memo item");
        };
        assert!(memo.body_statements().is_empty());
        assert!(matches!(memo.body_value(), Some(Expr::Field { .. })));

        let Item::Parser(parser) = &tree.items()[2] else {
            panic!("expected parser item");
        };
        assert!(parser.body_statements().is_empty());
        assert!(
            matches!(parser.body_value(), Some(Expr::NamedBlock { name, .. }) if name == "alt")
        );

        let hir = lower_to_hir(&tree).expect("hook, memo, and parser items lower");
        validate_typecheck_ready(&hir).expect("hook, memo, and parser bodies are structured");
    }

    #[test]
    fn rejects_old_memo_attribute_and_cache_option() {
        let errors = parse_source(
            r"
@memo(scope = scene)
fn route_title(route: Ref<Flow>) -> String {
    registry.flow(route).title
}

memo fn route_graph(root: Ref<Flow>) -> RouteGraph
cache session
{
    build_route_graph(root)
}
",
        )
        .expect_err("old memo syntax is rejected");

        assert!(errors.iter().any(|error| error.message().contains("@memo")));
        assert!(errors.iter().any(|error| error.message().contains("cache")));
    }

    #[test]
    fn rejects_old_hook_header_syntax() {
        let errors = parse_source(
            r"
hook #hook.choice_click
for #choice.opening.listen
on input target PointerClick
phase = input.target
{
    stop_propagation
}
",
        )
        .expect_err("old hook syntax is rejected");

        assert!(errors.iter().any(|error| error.message().contains("for")));
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("phase ="))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("on input target"))
        );
    }

    #[test]
    fn parses_documented_adt_items() {
        let tree = parse_source(
            r"
@derive(Clone, Debug, Format, Serialize, Eq)
pub enum GameEvent {
    StartGame,
    ChoiceSelected { id: Ref<ChoiceOption> },
}

pub struct SettingsInput {
    text_speed: f32,
    master_volume: f32,
}

pub type PlayerName = String
where len(self) >= 1
where len(self) <= 16
",
        )
        .expect("adt items parse");

        assert!(matches!(&tree.items()[0], Item::Attribute(_)));
        let Item::Enum(event) = &tree.items()[1] else {
            panic!("expected enum item");
        };
        assert_eq!(event.visibility(), Some(Visibility::Public));
        assert_eq!(event.name(), "GameEvent");
        assert_eq!(event.variants().len(), 2);
        assert_eq!(event.variants()[1].name(), "ChoiceSelected");
        assert_eq!(
            event.variants()[1].payload(),
            Some("{ id: Ref<ChoiceOption> }")
        );

        let Item::Struct(settings) = &tree.items()[2] else {
            panic!("expected struct item");
        };
        assert_eq!(settings.fields().len(), 2);
        assert_eq!(settings.fields()[0].name(), "text_speed");

        let Item::TypeAlias(alias) = &tree.items()[3] else {
            panic!("expected type alias item");
        };
        assert_eq!(alias.name(), "PlayerName");
        assert!(matches!(alias.target(), TypeRef::Path(path) if path == "String"));
        assert_eq!(alias.where_clauses().len(), 2);

        let hir = lower_to_hir(&tree).expect("syntax-only adt items do not block lowering");
        assert!(hir.flows().is_empty());
        assert!(matches!(
            hir.declarations(),
            [
                HirTopLevelDecl::Attribute(_),
                HirTopLevelDecl::Enum(_),
                HirTopLevelDecl::Struct(_),
                HirTopLevelDecl::TypeAlias(_)
            ]
        ));
    }

    #[test]
    fn parses_documented_state_reducer_and_view_items() {
        let tree = parse_source(
            r"
pub state GameState {
    pub route: Ref<Flow> = #flow.opening
    pub config: Config = Config {}
    pub flags: Set<Flag> = {}
    pub affection: Map<Ref<Character>, i32> = {}
    pub current_bg: Option<ImageHandle> = None
}

pub reducer update(state: GameState, event: GameEvent) -> Result<Update<GameState>, GameError>
requires state_is_valid
{
    match event {
        _ => Ok(state.to_update())
    }
}

pub view current_scene(state: GameState) -> Scene {
    scene {
        layer bg = image(#asset.bg.room)
    }
}
",
        )
        .expect("state, reducer, and view parse");

        let Item::State(state) = &tree.items()[0] else {
            panic!("expected state item");
        };
        assert_eq!(state.visibility(), Some(Visibility::Public));
        assert_eq!(state.name(), "GameState");
        assert_eq!(state.fields().len(), 5);
        assert_eq!(state.fields()[0].visibility(), Some(Visibility::Public));
        assert_eq!(state.fields()[0].name(), "route");
        assert!(
            matches!(state.fields()[0].default(), Expr::EntityRef(entity) if entity.body() == "flow.opening")
        );
        assert!(matches!(
            state.fields()[1].default(),
            Expr::Record { path, fields } if path == "Config" && fields.is_empty()
        ));

        let Item::Callable(reducer) = &tree.items()[1] else {
            panic!("expected reducer item");
        };
        assert_eq!(reducer.kind(), CallableKind::Reducer);
        assert_eq!(reducer.name(), "update");
        assert!(reducer.signature_tail().contains("GameEvent"));
        assert_eq!(reducer.contracts().len(), 1);
        assert!(reducer.body().contains("match event"));

        let Item::Callable(view) = &tree.items()[2] else {
            panic!("expected view item");
        };
        assert_eq!(view.kind(), CallableKind::View);
        assert_eq!(view.name(), "current_scene");

        let hir = lower_to_hir(&tree).expect("syntax-only state/callable items do not block HIR");
        assert!(hir.flows().is_empty());
        validate_typecheck_ready(&hir).expect("state defaults lower without raw expressions");
        assert!(matches!(
            hir.declarations(),
            [
                HirTopLevelDecl::State(_),
                HirTopLevelDecl::Callable(_),
                HirTopLevelDecl::Callable(_)
            ]
        ));
    }

    #[test]
    fn parses_documented_trait_and_impl_items() {
        let tree = parse_source(
            r"
pub trait Mappable {
    type Item
    type Mapped<B>
    fn map<B>(self, f: Self::Item -> B) -> Self::Mapped<B>
}

pub trait Ord: Eq {}

pub impl<T> Mappable for Option<T> {
    type Item = T
    type Mapped<B> = Option<B>

    fn map<B>(self, f: T -> B) -> Option<B> {
        match self {
            Some(x) => Some(f(x)),
            None => None,
        }
    }
}
",
        )
        .expect("trait and impl items parse");

        let Item::Trait(mappable) = &tree.items()[0] else {
            panic!("expected trait item");
        };
        assert_eq!(mappable.visibility(), Some(Visibility::Public));
        assert_eq!(mappable.name(), "Mappable");
        assert_eq!(mappable.members().len(), 3);
        assert!(matches!(
            &mappable.members()[0],
            TraitMember::AssociatedType { name, params, value: None }
                if name == "Item" && params.is_empty()
        ));
        assert!(matches!(
            &mappable.members()[1],
            TraitMember::AssociatedType { name, params, value: None }
                if name == "Mapped" && params == &["B".to_owned()]
        ));
        assert!(matches!(
            &mappable.members()[2],
            TraitMember::Function { signature }
                if signature.name() == "map"
                    && signature.params().first().is_some_and(|param| param.pattern() == "self")
        ));

        let Item::Trait(ord) = &tree.items()[1] else {
            panic!("expected second trait item");
        };
        assert_eq!(ord.supertraits(), &["Eq".to_owned()]);

        let Item::Impl(impl_item) = &tree.items()[2] else {
            panic!("expected impl item");
        };
        assert_eq!(impl_item.visibility(), Some(Visibility::Public));
        assert_eq!(impl_item.generics(), Some("<T>"));
        assert_eq!(impl_item.trait_name(), Some("Mappable"));
        assert_eq!(impl_item.target(), "Option<T>");
        assert_eq!(impl_item.members().len(), 3);
        assert!(matches!(
            &impl_item.members()[0],
            ImplMember::AssociatedType { name, params, value }
                if name == "Item" && params.is_empty() && matches!(value, TypeRef::Path(path) if path == "T")
        ));
        assert!(matches!(
            &impl_item.members()[1],
            ImplMember::AssociatedType { name, params, value }
                if name == "Mapped"
                    && params == &["B".to_owned()]
                    && matches!(value, TypeRef::Generic { base, args } if base == "Option" && args.len() == 1)
        ));
        assert!(matches!(
            &impl_item.members()[2],
            ImplMember::Function {
                signature,
                body,
                body_statements,
                body_value,
            }
                if signature.name() == "map"
                    && signature.params().first().is_some_and(|param| param.pattern() == "self")
                    && body.contains("match self")
                    && body_statements.is_empty()
                    && matches!(body_value, Some(Expr::Match { .. }))
        ));
        assert!(impl_item.body().contains("Some(x)"));

        let hir = lower_to_hir(&tree).expect("syntax-only trait/impl items do not block HIR");
        assert!(hir.flows().is_empty());
        assert!(matches!(
            hir.declarations(),
            [
                HirTopLevelDecl::Trait(_),
                HirTopLevelDecl::Trait(_),
                HirTopLevelDecl::Impl(_)
            ]
        ));
    }

    #[test]
    fn parses_attributes_and_wiki_links() {
        let tree = parse_source(
            r"
/// links to [[flow.alice_intro]]
@derive(Debug)
flow #flow.opening opening {}
",
        )
        .expect("attributes and wiki links parse");

        assert_eq!(tree.wiki_links()[0].body(), "flow.alice_intro");
        assert!(matches!(&tree.items()[0], Item::Attribute(attr) if attr.name() == "derive"));
        assert!(matches!(&tree.items()[1], Item::Flow(_)));
    }

    #[test]
    fn dialogue_tokenizer_covers_tags_ruby_expr_and_escapes() {
        let tokens = parse_dialogue_tokens(
            r#"今日は\｜少し、｜変な夢《へんなゆめ》#[fmt("夢", color=blue)][p][hook mark]"#,
        );

        assert!(tokens.iter().any(|token| matches!(token, DialogueToken::Ruby { base, ruby } if base == "変な夢" && ruby == "へんなゆめ")));
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token, DialogueToken::Expr(Expr::Call { .. })))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "p"))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "hook"))
        );
    }

    #[test]
    fn dialogue_tokenizer_preserves_escaped_brackets_hash_and_braces() {
        let tokens = parse_dialogue_tokens(r"\[literal\] \#not_expr \{cue\}");

        assert!(matches!(tokens[0], DialogueToken::Escape('[')));
        assert!(matches!(tokens[2], DialogueToken::Escape(']')));
        assert!(matches!(tokens[4], DialogueToken::Escape('#')));
        assert!(matches!(tokens[6], DialogueToken::Escape('{')));
        assert!(matches!(tokens[8], DialogueToken::Escape('}')));
        assert!(
            tokens
                .iter()
                .all(|token| !matches!(token, DialogueToken::Tag(_) | DialogueToken::Expr(_)))
        );
    }

    #[test]
    fn reports_unclosed_flow_block() {
        let errors = parse_source("flow #flow.bad bad {").expect_err("unclosed block fails");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("unclosed block"));
        assert!(!errors[0].recovery().is_empty());
    }

    #[test]
    fn reports_invalid_entity_reference() {
        let errors = parse_source("flow # bad { }").expect_err("invalid entity ref fails");
        assert!(errors[0].message().contains("entity reference"));
    }

    #[test]
    fn reports_unclosed_dialogue_content_block() {
        let errors = parse_source("alice[おはよう。[p]").expect_err("unclosed content fails");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("dialogue content"));
        assert_eq!(errors[0].expected(), &["]"]);
        assert!(!errors[0].recovery().is_empty());
    }

    #[test]
    fn reports_unclosed_line_plan_block_after_cue() {
        let errors = parse_source(
            r"
alice[おはよう。[p]]
with {
    at(0.42s) { alice.stage.face(worried)
",
        )
        .expect_err("unclosed line plan fails");

        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("line plan"));
        assert!(!errors[0].recovery().is_empty());
    }

    #[test]
    fn parses_expression_shapes_needed_by_hir_lowering() {
        let pipe = super::parse_expr("state |> has_affection_at_least(#character.alice, 3)")
            .expect("pipe expr parses");
        assert!(matches!(pipe, Expr::Pipe { .. }));

        let method = super::parse_expr("choices.filter(_.enabled).map(_.label)")
            .expect("method chain parses");
        assert!(matches!(method, Expr::MethodCall { .. }));
        let Expr::MethodCall { args, .. } = method else {
            panic!("expected outer map call");
        };
        assert!(matches!(
            args.as_slice(),
            [Expr::Field {
                target,
                field
            }] if matches!(target.as_ref(), Expr::Placeholder(Placeholder::Partial)) && field == "label"
        ));

        let indexed =
            super::parse_expr("state.affection[#character.alice]").expect("index expr parses");
        assert!(matches!(indexed, Expr::Index { .. }));

        let dialogue_call =
            super::parse_expr("alice.say()[聞いて。[p]]").expect("dialogue call expr parses");
        assert!(matches!(dialogue_call, Expr::DialogueCall { .. }));

        let timeline_offset = super::parse_expr("end-250ms").expect("timeline offset parses");
        assert!(matches!(timeline_offset, Expr::Binary { .. }));

        let placeholder = super::parse_expr("clamp(0, ^, 100)").expect("placeholder call parses");
        assert!(matches!(placeholder, Expr::Call { .. }));
        let partial =
            super::parse_expr("_.score >= ^").expect("partial comparison expression parses");
        assert!(matches!(partial, Expr::Binary { .. }));

        let list = super::parse_expr("[normal, smile, worried]").expect("list expression parses");
        assert!(matches!(list, Expr::List(items) if items.len() == 3));
        let empty_list = super::parse_expr("[]").expect("empty list expression parses");
        assert!(matches!(empty_list, Expr::List(items) if items.is_empty()));
        let nested_list = super::parse_expr("[#stem.piano, fade(0.2s, [slow, fast])]")
            .expect("nested list expression parses");
        assert!(matches!(nested_list, Expr::List(items) if items.len() == 2));
        let record_literal = super::parse_expr("{ player_name = state.player_name }")
            .expect("record literal parses");
        assert!(matches!(record_literal, Expr::RecordLiteral(fields) if fields.len() == 1));
        let empty_record = super::parse_expr("{}").expect("empty record literal parses");
        assert!(matches!(empty_record, Expr::RecordLiteral(fields) if fields.is_empty()));

        let generic_collect = super::parse_expr("visible_choices.collect<List<ChoiceView>>()")
            .expect("generic method call parses");
        assert!(matches!(
            generic_collect,
            Expr::MethodCall { method, .. } if method == "collect<List<ChoiceView>>"
        ));

        let context_closure = super::parse_expr(r#"load_bg(id).with_context(|| "failed")?"#)
            .expect("closure argument parses");
        assert!(matches!(context_closure, Expr::Try { .. }));

        let delimited = super::parse_expr("#<say.opening.dream_hint@sem:b3_9f2a1c>")
            .expect("delimited ref expr parses");
        assert!(matches!(delimited, Expr::EntityRef(entity) if entity.is_delimited()));

        let range = super::parse_expr("0.0..=1.0").expect("inclusive float range parses");
        assert!(matches!(
            range,
            Expr::Range {
                inclusive: true,
                ..
            }
        ));

        let membership =
            super::parse_expr("progress in 0.0..=1.0").expect("range membership parses");
        assert!(matches!(
            membership,
            Expr::Binary {
                op: BinaryOp::In,
                ..
            }
        ));

        let unary_not = super::parse_expr("!event.is_relevant()").expect("unary not expr parses");
        assert!(matches!(
            unary_not,
            Expr::Unary {
                op: UnaryOp::Not,
                ..
            }
        ));
    }

    #[test]
    fn parses_lifetime_type_syntax_for_borrow_checks() {
        let borrowed_slice = parse_type_ref("&'asset [Rgba8]").expect("borrowed slice type parses");
        assert!(matches!(
            borrowed_slice,
            TypeRef::Ref {
                lifetime: Some(ref lifetime),
                inner,
            } if lifetime.name() == "asset" && matches!(inner.as_ref(), TypeRef::Slice(_))
        ));

        let option_borrow =
            parse_type_ref("Option<&'a ChoiceView>").expect("generic borrowed type parses");
        assert!(matches!(option_borrow, TypeRef::Generic { .. }));

        let signature =
            parse_fn_signature("fn first<'a>(xs: &'a [ChoiceView]) -> Option<&'a ChoiceView>")
                .expect("fn signature lifetimes parse");
        assert_eq!(signature.name(), "first");
        assert_eq!(signature.lifetimes()[0].name(), "a");
        assert_eq!(signature.params()[0].pattern(), "xs");
        assert!(signature.return_type().is_some());
    }

    #[test]
    fn parses_self_receiver_and_function_type_parameters() {
        let signature =
            parse_fn_signature("fn map<B>(self, f: Self::Item -> B) -> Self::Mapped<B>")
                .expect("trait method signature parses");
        assert_eq!(signature.name(), "map");
        assert_eq!(signature.params()[0].pattern(), "self");
        assert_eq!(signature.params()[1].pattern(), "f");
        assert!(
            matches!(signature.return_type(), Some(TypeRef::Generic { base, .. }) if base == "Self::Mapped")
        );
    }

    #[test]
    fn parses_function_item_with_lifetimes_and_contracts() {
        let tree = parse_source(
            r"
pub fn first<'a>(xs: &'a [ChoiceView]) -> Option<&'a ChoiceView>
requires xs.len() > 0
ensures check result.is_some()
effects { asset.read }
{
    xs[0]
}
",
        )
        .expect("function item parses");

        let Item::Function(function) = &tree.items()[0] else {
            panic!("expected function item");
        };
        assert_eq!(function.visibility(), Some(Visibility::Public));
        assert_eq!(function.signature().name(), "first");
        assert_eq!(function.signature().lifetimes()[0].name(), "a");
        assert!(function.signature_text().contains("Option<&'a ChoiceView>"));
        assert_eq!(function.contracts().len(), 3);
        assert!(matches!(
            &function.contracts()[0],
            ContractClause::Requires {
                expr: Expr::Binary { .. },
                ..
            }
        ));
        assert!(function.body().contains("xs[0]"));
        assert!(function.body_statements().is_empty());
        assert!(matches!(function.body_value(), Some(Expr::Index { .. })));
    }

    #[test]
    fn parses_task_fn_as_structured_function_item() {
        let tree = parse_source(
            r"
task fn load_opening_assets() -> ArcResult<OpeningAssets> {
    let bg = try await load_bg()
    Ok(OpeningAssets { bg })
}
",
        )
        .expect("task function parses");

        let Item::Function(function) = &tree.items()[0] else {
            panic!("expected task function item");
        };
        assert_eq!(function.kind(), FunctionKind::Task);
        assert_eq!(function.signature().name(), "load_opening_assets");
        assert_eq!(
            function.signature_text(),
            "fn load_opening_assets() -> ArcResult<OpeningAssets>"
        );
        assert!(matches!(
            function.body_statements()[0],
            Stmt::Let {
                expr: Expr::Await {
                    applies_try: true,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            function.body_value(),
            Some(Expr::Call { callee, .. }) if matches!(callee.as_ref(), Expr::Path(path) if path == "Ok")
        ));

        let hir = lower_to_hir(&tree).expect("task function lowers");
        assert_eq!(hir.functions().len(), 1);
        assert_eq!(hir.functions()[0].kind(), FunctionKind::Task);
        validate_typecheck_ready(&hir).expect("task function body has structured expressions");
    }

    #[test]
    fn typechecks_task_fn_try_await_without_wait_view() {
        let tree = parse_source(
            r"
task fn load_bg_task() -> Image {
    let bg = try await load_bg()
    bg
}
",
        )
        .expect("task function parses");
        let hir = lower_to_hir(&tree).expect("task function lowers");
        validate_typecheck_ready(&hir).expect("try await expression is structured");

        let env = TypeCheckEnv::new().with_function(
            "load_bg",
            TypeKind::Need {
                ready: Box::new(TypeKind::Named("Image".to_owned())),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        );
        typecheck_hir(&hir, &env).expect("try await unwraps Need<T, E> to T");
    }

    #[test]
    fn plain_await_expression_returns_result_in_task_fn() {
        let tree = parse_source(
            r"
task fn load_bg_result() -> Result<Image, AssetError> {
    await load_bg()
}
",
        )
        .expect("task function parses");
        let hir = lower_to_hir(&tree).expect("task function lowers");
        assert!(matches!(
            hir.functions()[0].value(),
            Some(Expr::Await {
                applies_try: false,
                ..
            })
        ));

        let env = TypeCheckEnv::new().with_function(
            "load_bg",
            TypeKind::Need {
                ready: Box::new(TypeKind::Named("Image".to_owned())),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        );
        typecheck_hir(&hir, &env).expect("plain await returns Result<T, E>");
    }

    #[test]
    fn parses_dialogue_and_stream_function_kinds() {
        let tree = parse_source(
            r"
pub dialogue fn flash(color: Color) -> Content {
    Content::empty()
}

stream fn camera_frames() -> Source<VideoFrame, CameraError> {
    yield next_frame()
}
",
        )
        .expect("dialogue and stream functions parse");

        let Item::Function(dialogue) = &tree.items()[0] else {
            panic!("expected dialogue function");
        };
        assert_eq!(dialogue.kind(), FunctionKind::Dialogue);
        assert_eq!(
            dialogue.signature_text(),
            "fn flash(color: Color) -> Content"
        );

        let Item::Function(stream) = &tree.items()[1] else {
            panic!("expected stream function");
        };
        assert_eq!(stream.kind(), FunctionKind::Stream);
        assert_eq!(
            stream.signature_text(),
            "fn camera_frames() -> Source<VideoFrame, CameraError>"
        );
        assert!(matches!(stream.body_statements()[0], Stmt::Yield(_)));

        let hir = lower_to_hir(&tree).expect("dialogue and stream functions lower");
        assert_eq!(hir.functions()[0].kind(), FunctionKind::Dialogue);
        assert_eq!(hir.functions()[1].kind(), FunctionKind::Stream);
        validate_typecheck_ready(&hir).expect("function-kind bodies are structured");
    }

    #[test]
    fn top_level_function_items_do_not_block_hir_lowering() {
        let tree = parse_source(
            r"
fn label<'a>(choice: &'a ChoiceView) -> &'a DisplayText {
    choice.label
}

flow #flow.opening opening {
    goto #flow.title
}
",
        )
        .expect("function and flow parse");

        let hir = lower_to_hir(&tree).expect("function and flow lower");
        assert_eq!(hir.functions().len(), 1);
        assert_eq!(hir.functions()[0].name(), "label");
        assert_eq!(hir.flows().len(), 1);
        assert_eq!(hir.flows()[0].id().expect("flow id").body(), "flow.opening");
    }

    #[test]
    fn typechecks_structured_function_body_for_hir_readiness() {
        let tree = parse_source(
            r"
fn load_score() -> i32 {
    let score = read_score()?
    score
}
",
        )
        .expect("function body parses");
        let hir = lower_to_hir(&tree).expect("function lowers");

        assert_eq!(hir.functions().len(), 1);
        assert!(matches!(
            hir.functions()[0].statements()[0],
            Stmt::Let {
                expr: Expr::Try { .. },
                ..
            }
        ));
        validate_typecheck_ready(&hir).expect("function body has structured expressions");

        let env = TypeCheckEnv::new().with_function(
            "read_score",
            TypeKind::Result {
                ok: Box::new(TypeKind::Int),
                error: Box::new(TypeKind::Named("ScoreError".to_owned())),
            },
        );
        typecheck_hir(&hir, &env).expect("function body typechecks");
    }

    #[test]
    fn typecheck_rejects_function_return_type_mismatch() {
        let tree = parse_source(
            r"
fn bad_score() -> Bool {
    1
}
",
        )
        .expect("function body parses");
        let hir = lower_to_hir(&tree).expect("function lowers");
        let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("return mismatch");
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("function `bad_score` returns"))
        );
    }

    #[test]
    fn line_plan_items_keep_typed_expressions() {
        let tree = parse_source(
            r"
alice:
    聞いて。[p]
with:
    reveal = voice
    let voice = line.voice_handle()
    out (actor, voice)
",
        )
        .expect("line plan exprs parse");

        let Item::FlowItem(FlowItem::SpeakerLine(line)) = &tree.items()[0] else {
            panic!("expected speaker line");
        };
        let plan = line.plan().expect("line plan");
        assert!(matches!(
            &plan.items()[0],
            LinePlanItem::Option { value: Expr::Path(path), .. } if path == "voice"
        ));
        assert!(matches!(
            &plan.items()[1],
            LinePlanItem::Let {
                expr: Expr::MethodCall { .. },
                ..
            }
        ));
        assert!(matches!(
            &plan.items()[2],
            LinePlanItem::Out(Expr::Tuple(_))
        ));
    }

    #[test]
    fn line_plan_cancel_actions_keep_typed_statements() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    alice[
        聞いて。[p]
    ]
    with 'line {
        cancel on input .SkipLine { out 'line .Skipped }
        cancel on input .BackToTitle => goto #flow.title
    }
}
",
        )
        .expect("line plan cancel actions parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::ContentCall(call) = &flow.body()[0] else {
            panic!("expected content call");
        };
        let plan = call.plan().expect("line plan");
        assert_eq!(plan.label(), Some("line"));
        let LinePlanItem::CancelRule(skip_rule) = &plan.items()[0] else {
            panic!("expected skip cancel rule");
        };
        assert!(matches!(
            skip_rule.action(),
            [Stmt::Out { label: Some(label), expr: Expr::Path(path) }]
                if label == "line" && path == ".Skipped"
        ));
        let LinePlanItem::CancelRule(back_rule) = &plan.items()[1] else {
            panic!("expected back-to-title cancel rule");
        };
        assert!(matches!(
            back_rule.action(),
            [Stmt::Goto(Expr::EntityRef(target))] if target.body() == "flow.title"
        ));

        let hir = lower_to_hir(&tree).expect("line plan cancel actions lower");
        validate_typecheck_ready(&hir).expect("line plan cancel actions are typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
                .with_symbol(".Skipped", TypeKind::Named("LineExit".to_owned())),
        )
        .expect("line plan cancel actions typecheck");
    }

    #[test]
    fn line_plan_cancel_commands_keep_structured_arguments() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    alice[
        聞いて。[p]
    ]
    with {
        cancel on input .SkipLine {
            stop voice fade=40ms
            flush text instant
            continue
        }
    }
}
",
        )
        .expect("line plan cancel commands parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::ContentCall(call) = &flow.body()[0] else {
            panic!("expected content call");
        };
        let plan = call.plan().expect("line plan");
        let LinePlanItem::CancelRule(rule) = &plan.items()[0] else {
            panic!("expected cancel rule");
        };
        assert!(matches!(
            rule.action(),
            [Stmt::Command(stop), Stmt::Command(flush), Stmt::Continue { label: None }]
                if stop.name() == "stop" && flush.name() == "flush"
        ));

        let hir = lower_to_hir(&tree).expect("line plan cancel commands lower");
        validate_typecheck_ready(&hir).expect("line plan cancel commands are typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
                .with_symbol("voice", TypeKind::Named("VoiceHandle".to_owned()))
                .with_symbol("text", TypeKind::Named("DialogueText".to_owned()))
                .with_symbol("instant", TypeKind::Named("FlushPolicy".to_owned())),
        )
        .expect("line plan cancel commands typecheck");
    }

    #[test]
    fn line_plan_assertions_keep_typed_conditions() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    alice:
        聞いて。[p]
    with {
        assert textbox_ready
        debug_assert route_count > 0
    }
}
",
        )
        .expect("line plan assertions parse");
        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
            panic!("expected speaker line");
        };
        let plan = line.plan().expect("line plan");
        assert!(matches!(
            &plan.items()[0],
            LinePlanItem::Assert {
                debug: false,
                expr: Expr::Path(path)
            } if path == "textbox_ready"
        ));

        let hir = lower_to_hir(&tree).expect("line plan assertions lower");
        validate_typecheck_ready(&hir).expect("line plan assertions are typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
                .with_symbol("textbox_ready", TypeKind::Bool)
                .with_symbol("route_count", TypeKind::Int),
        )
        .expect("line plan assertions typecheck");
    }

    #[test]
    fn line_plan_parallel_groups_keep_typed_items() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    alice:
        走って！[p]
    with {
        start {
            together {
                cue_move()
                cue_face()
                cue_se()
            }
        }
    }
}
",
        )
        .expect("line plan parallel groups parse");
        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
            panic!("expected speaker line");
        };
        let plan = line.plan().expect("line plan");
        let [LinePlanItem::StartGroup(start_items)] = plan.items() else {
            panic!("expected start group");
        };
        let [LinePlanItem::TogetherGroup(together_items)] = start_items.as_slice() else {
            panic!("expected together group inside start group");
        };
        assert_eq!(together_items.len(), 3);
        assert!(matches!(
            &together_items[0],
            LinePlanItem::Expr(Expr::Call { .. })
        ));

        let hir = lower_to_hir(&tree).expect("line plan parallel groups lower");
        validate_typecheck_ready(&hir).expect("line plan parallel groups are typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
                .with_function("cue_move", TypeKind::Unit)
                .with_function("cue_face", TypeKind::Unit)
                .with_function("cue_se", TypeKind::Unit),
        )
        .expect("line plan parallel groups typecheck");
    }

    #[test]
    fn line_plan_memo_keeps_typed_options() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    alice:
        聞いて。[p]
    with {
        memo rich_text key=(line.id, locale, theme.text_hash) cache=flow
    }
}
",
        )
        .expect("line plan memo parses");
        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
            panic!("expected speaker line");
        };
        let plan = line.plan().expect("line plan");
        let [LinePlanItem::Memo { name, options }] = plan.items() else {
            panic!("expected memo item");
        };
        assert_eq!(name, "rich_text");
        assert_eq!(options.len(), 2);
        assert!(matches!(&options[0].1, Expr::Tuple(items) if items.len() == 3));

        let hir = lower_to_hir(&tree).expect("line plan memo lowers");
        validate_typecheck_ready(&hir).expect("line plan memo is typecheck-ready");
        typecheck_hir(
            &hir,
            &TypeCheckEnv::new()
                .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
                .with_symbol("line.id", TypeKind::Ref(EntityKind::DialogueLine))
                .with_symbol("locale", TypeKind::String)
                .with_symbol("theme.text_hash", TypeKind::Named("TextHash".to_owned()))
                .with_symbol("flow", TypeKind::Named("CacheScope".to_owned())),
        )
        .expect("line plan memo typechecks");
    }

    #[test]
    fn parses_multiline_timed_cue_body_as_expression() {
        let tree = parse_source(
            r"
alice[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.face(smile)
",
        )
        .expect("multiline timed cue parses");

        let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
            panic!("expected content call");
        };
        let plan = call.plan().expect("line plan");
        assert!(matches!(
            &plan.items()[0],
            LinePlanItem::TimedCue {
                anchor: Expr::Literal(_),
                body: Expr::MethodCall { .. }
            }
        ));
    }

    #[test]
    fn await_with_keeps_awaited_expression() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    try await load_opening_assets() with { pending p => scene #scene.loading { progress p.ratio } }
}
",
        )
        .expect("await with parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::AwaitWith(await_with) = &flow.body()[0] else {
            panic!("expected await with");
        };
        assert!(await_with.applies_try());
        assert!(matches!(
            await_with.expr(),
            Expr::Call { .. } | Expr::MethodCall { .. }
        ));
        let pending = await_with.pending().expect("pending branch");
        assert_eq!(pending.kind(), AwaitBranchKind::Pending);
        assert!(matches!(pending.body()[0], FlowItem::ScenarioCommand(_)));
    }

    #[test]
    fn await_with_keeps_wait_view_branches() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    await load_avatar() with {
        pending p => scene #scene.loading { progress p.ratio }
        ready img => Image(img)
        error _ => Icon(#asset.avatar_fallback)
        denied _ => return Ok(FlowExit::Goto(#flow.title))
    }
}
",
        )
        .expect("await branches parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::AwaitWith(await_with) = &flow.body()[0] else {
            panic!("expected await with");
        };
        assert_eq!(await_with.branches().len(), 4);
        assert!(matches!(
            await_with.branches()[0].kind(),
            AwaitBranchKind::Pending
        ));
        assert!(matches!(
            await_with.branches()[1].body()[0],
            FlowItem::Stmt(Stmt::Expr(Expr::Call { .. }))
        ));

        let hir = lower_to_hir(&tree).expect("await branches lower");
        assert!(matches!(
            &hir.flows()[0].body()[0],
            HirFlowItem::Await(await_with) if await_with.branches().len() == 4
        ));
    }

    #[test]
    fn try_await_accepts_indented_with_block() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    try await asset.image(#asset.bg.room) with:
        pending p:
            scene #scene.loading:
                progress p.ratio
}
",
        )
        .expect("try await with colon block parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::AwaitWith(await_with) = &flow.body()[0] else {
            panic!("expected await with");
        };
        assert!(await_with.applies_try());
        assert!(matches!(
            await_with.expr(),
            Expr::Call { .. } | Expr::MethodCall { .. }
        ));
        let pending = await_with.pending().expect("pending branch");
        assert_eq!(pending.body().len(), 1);
        assert!(matches!(pending.body()[0], FlowItem::ScenarioCommand(_)));
    }

    #[test]
    fn await_question_prefix_is_try_await_sugar() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    await? asset.image(#asset.bg.room) with { pending p => scene #scene.loading }
}
",
        )
        .expect("await? prefix sugar parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::AwaitWith(await_with) = &flow.body()[0] else {
            panic!("expected await with");
        };
        assert!(await_with.applies_try());
        assert!(matches!(
            await_with.expr(),
            Expr::Call { .. } | Expr::MethodCall { .. }
        ));
    }

    #[test]
    fn let_try_await_with_binds_ready_value_and_keeps_wait_view() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    let assets = try await load_opening_assets() with { pending p => p.ratio ready loaded => loaded.ready }
    let count = assets.count
}
",
        )
        .expect("bound try-await wait-view parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::LetAwait {
            pattern,
            await_with,
        }) = &flow.body()[0]
        else {
            panic!("expected let-await statement");
        };
        assert!(matches!(pattern, Pattern::Ident(name) if name == "assets"));
        assert!(await_with.applies_try());
        assert!(await_with.pending().is_some());

        let hir = lower_to_hir(&tree).expect("bound try-await lowers");
        assert!(matches!(
            &hir.flows()[0].body()[0],
            HirFlowItem::LetAwait {
                pattern: Pattern::Ident(name),
                await_with,
            } if name == "assets" && await_with.applies_try()
        ));
        validate_typecheck_ready(&hir).expect("bound try-await is typecheck-ready");

        let env = TypeCheckEnv::new().with_function(
            "load_opening_assets",
            TypeKind::Need {
                ready: Box::new(TypeKind::Named("OpeningAssets".to_owned())),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        );
        typecheck_hir(&hir, &env).expect("ready value and pending progress bind in scope");
    }

    #[test]
    fn let_plain_await_with_binds_result_value() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    let result = await load_opening_assets() with:
        pending p:
            p.ratio
    let display = result
}
",
        )
        .expect("bound plain await wait-view parses");
        let hir = lower_to_hir(&tree).expect("bound plain await lowers");

        let env = TypeCheckEnv::new().with_function(
            "load_opening_assets",
            TypeKind::Need {
                ready: Box::new(TypeKind::Named("OpeningAssets".to_owned())),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        );
        typecheck_hir(&hir, &env).expect("plain await binds Result<T, E>");
    }

    #[test]
    fn await_with_variant_pending_pattern_binds_payload() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    try await run_activity() with { pending .Realizing(p) => p.ratio pending .Running(p) => p.ratio }
}
",
        )
        .expect("variant pending patterns parse");
        let hir = lower_to_hir(&tree).expect("variant pending patterns lower");

        let env = TypeCheckEnv::new().with_function(
            "run_activity",
            TypeKind::Need {
                ready: Box::new(TypeKind::Named("ActivityOutput".to_owned())),
                error: Box::new(TypeKind::Named("ActivityError".to_owned())),
            },
        );
        typecheck_hir(&hir, &env).expect("variant payloads bind in wait-view branches");
    }

    #[test]
    fn let_try_await_with_accepts_multiline_context_before_with() {
        let tree = parse_source(
            r#"
flow #flow.loading loading {
    let bg = try await asset.image(#asset.bg.room)
        .context("opening background failed")
    with:
        pending p:
            p.ratio
    let display = bg.id
}
"#,
        )
        .expect("multiline contextual try-await parses");

        let hir = lower_to_hir(&tree).expect("multiline contextual try-await lowers");
        assert!(matches!(
            &hir.flows()[0].body()[0],
            HirFlowItem::LetAwait { await_with, .. }
                if matches!(await_with.expr(), Expr::MethodCall { method, .. } if method == "context")
        ));
        validate_typecheck_ready(&hir).expect("multiline contextual try-await is typecheck-ready");

        let need_type = TypeKind::Need {
            ready: Box::new(TypeKind::Named("Image".to_owned())),
            error: Box::new(TypeKind::Named("AssetError".to_owned())),
        };
        let env = TypeCheckEnv::new()
            .with_symbol("asset", TypeKind::Named("AssetApi".to_owned()))
            .with_method(
                TypeKind::Named("AssetApi".to_owned()),
                "image",
                need_type.clone(),
            )
            .with_method(need_type.clone(), "context", need_type);
        typecheck_hir(&hir, &env).expect("context-preserved try-await typechecks");
    }

    #[test]
    fn let_parenthesized_await_with_question_is_try_sugar() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    let bg = (await asset.image(#asset.bg.room) with:
        pending p:
            p.ratio
    )?
    let display = bg.id
}
",
        )
        .expect("parenthesized await-with try parses");

        let hir = lower_to_hir(&tree).expect("parenthesized await-with lowers");
        assert!(matches!(
            &hir.flows()[0].body()[0],
            HirFlowItem::LetAwait { await_with, .. } if await_with.applies_try()
        ));
        let need_type = TypeKind::Need {
            ready: Box::new(TypeKind::Named("Image".to_owned())),
            error: Box::new(TypeKind::Named("AssetError".to_owned())),
        };
        let env = TypeCheckEnv::new()
            .with_symbol("asset", TypeKind::Named("AssetApi".to_owned()))
            .with_method(TypeKind::Named("AssetApi".to_owned()), "image", need_type);
        typecheck_hir(&hir, &env).expect("parenthesized await-with unwraps Result");
    }

    #[test]
    fn let_parenthesized_await_with_context_after_block_typechecks() {
        let tree = parse_source(
            r#"
flow #flow.loading loading {
    let bg = (await asset.image(#asset.bg.room) with:
        pending p:
            p.ratio
    ).context("opening background failed")?
    let display = bg.id
}
"#,
        )
        .expect("post-await context parses");

        let hir = lower_to_hir(&tree).expect("post-await context lowers");
        assert!(matches!(
            &hir.flows()[0].body()[0],
            HirFlowItem::LetAwait { await_with, .. }
                if await_with.applies_try()
                    && matches!(await_with.expr(), Expr::MethodCall { method, .. } if method == "context")
        ));
        let need_type = TypeKind::Need {
            ready: Box::new(TypeKind::Named("Image".to_owned())),
            error: Box::new(TypeKind::Named("AssetError".to_owned())),
        };
        let env = TypeCheckEnv::new()
            .with_symbol("asset", TypeKind::Named("AssetApi".to_owned()))
            .with_method(
                TypeKind::Named("AssetApi".to_owned()),
                "image",
                need_type.clone(),
            )
            .with_method(need_type.clone(), "context", need_type);
        typecheck_hir(&hir, &env).expect("post-await context remains structured");
    }

    #[test]
    fn let_try_await_without_wait_view_stays_expression_await() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    let bg = try await load_bg()
}
",
        )
        .expect("plain try-await binding parses");
        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        assert!(matches!(
            &flow.body()[0],
            FlowItem::Stmt(Stmt::Let {
                expr: Expr::Await {
                    applies_try: true,
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn await_question_with_is_rejected_as_ambiguous() {
        let errors = parse_source(
            r"
flow #flow.loading loading {
    await load_opening_assets()? with { pending p => scene #scene.loading }
}
",
        )
        .expect_err("ambiguous await propagation is rejected");
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("await expr? with"))
        );
    }

    #[test]
    fn parses_for_and_select_flow_blocks() {
        let tree = parse_source(
            r"
flow #flow.stream stream {
    for c in choices {
        option c.id c.label
    }
    select {
        audio = frames.next? => {
            signal #signal.voice_level <- audio.rms
        }

        frame _ => {
            scene #scene.listening
            continue
        }

        event .Back => {
            close frames
            return Ok(FlowExit::Goto(#flow.title))
        }
    }
}
",
        )
        .expect("for and select parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::For(for_block) = &flow.body()[0] else {
            panic!("expected for block");
        };
        assert!(matches!(for_block.pattern(), Pattern::Ident(name) if name == "c"));
        assert!(matches!(for_block.source(), Expr::Path(path) if path == "choices"));
        assert!(matches!(
            &for_block.body()[0],
            FlowItem::ScenarioCommand(command) if command.name() == "option"
        ));

        let FlowItem::Select(select) = &flow.body()[1] else {
            panic!("expected select block");
        };
        assert_eq!(select.branches().len(), 3);
        assert!(matches!(
            select.branches()[0].head(),
            SelectBranchHead::Bind {
                name,
                propagates_error: true,
                ..
            } if name == "audio"
        ));
        assert!(matches!(
            select.branches()[1].head(),
            SelectBranchHead::Frame(Pattern::Discard)
        ));
        assert!(matches!(
            select.branches()[2].head(),
            SelectBranchHead::Event(Pattern::Variant { name, payload: None, .. }) if name == "Back"
        ));

        let hir = lower_to_hir(&tree).expect("for and select lower");
        assert!(matches!(&hir.flows()[0].body()[0], HirFlowItem::For(_)));
        assert!(matches!(&hir.flows()[0].body()[1], HirFlowItem::Select(_)));
        validate_typecheck_ready(&hir).expect("for and select are typecheck-ready");
    }

    #[test]
    fn flow_typed_statements_keep_patterns_and_exprs() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    let (actor, (_, voice)) = alice.say()[聞いて。[p]]
    return Ok(FlowExit::Done)
    goto #flow.title
}
",
        )
        .expect("typed flow statements parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        assert!(matches!(
            &flow.body()[0],
            FlowItem::Stmt(Stmt::Let {
                pattern: Pattern::Tuple(_),
                expr: Expr::DialogueCall { .. },
            })
        ));
        assert!(matches!(
            &flow.body()[1],
            FlowItem::Stmt(Stmt::Return(Expr::Call { .. }))
        ));
        assert!(matches!(
            &flow.body()[2],
            FlowItem::Stmt(Stmt::Goto(Expr::EntityRef(entity))) if entity.body() == "flow.title"
        ));
    }

    #[test]
    fn typed_patterns_keep_lifetime_borrow_types() {
        let tree = parse_source(
            r"
flow #flow.borrow borrow {
    let pixels: &'asset [Rgba8] = bg.pixels()
}
",
        )
        .expect("typed borrow pattern parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        assert!(matches!(
            &flow.body()[0],
            FlowItem::Stmt(Stmt::Let {
                pattern: Pattern::Typed {
                    ty: TypeRef::Ref { .. },
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn parses_documented_structured_pattern_shapes() {
        let tree = parse_source(
            r"
flow #flow.patterns patterns {
    let mut route = current_route
    let 42 = answer
    let #choice.opening.listen = selected
    let TruckResult { score, rank, .. } = result
    let [first, ..rest] = items
    let ev .ChoiceSelected { id } = event
}
",
        )
        .expect("structured pattern fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        assert!(matches!(
            &flow.body()[0],
            FlowItem::Stmt(Stmt::Let {
                pattern: Pattern::MutIdent(name),
                ..
            }) if name == "route"
        ));
        assert!(matches!(
            &flow.body()[1],
            FlowItem::Stmt(Stmt::Let {
                pattern: Pattern::Literal(Expr::Literal(_)),
                ..
            })
        ));
        assert!(matches!(
            &flow.body()[2],
            FlowItem::Stmt(Stmt::Let {
                pattern: Pattern::Entity(entity),
                ..
            }) if entity.body() == "choice.opening.listen"
        ));
        assert!(matches!(
            &flow.body()[3],
            FlowItem::Stmt(Stmt::Let {
                pattern: Pattern::Record {
                    path: Some(path),
                    fields,
                    rest: true,
                },
                ..
            }) if path == "TruckResult" && fields.len() == 2
        ));
        assert!(matches!(
            &flow.body()[4],
            FlowItem::Stmt(Stmt::Let {
                pattern: Pattern::List {
                    items,
                    rest: Some(rest),
                },
                ..
            }) if items.len() == 1 && rest == "rest"
        ));
        let FlowItem::Stmt(Stmt::Let {
            pattern: Pattern::Whole { name, pattern },
            ..
        }) = &flow.body()[5]
        else {
            panic!("expected whole-pattern variant binding");
        };
        assert_eq!(name, "ev");
        assert!(matches!(
            pattern.as_ref(),
            Pattern::Variant {
                name,
                payload: Some(VariantPatternPayload::Record { fields, rest: false }),
                ..
            } if name == "ChoiceSelected" && fields.len() == 1 && fields[0].name() == "id"
        ));

        let hir = lower_to_hir(&tree).expect("structured pattern fixture lowers");
        validate_typecheck_ready(&hir).expect("structured patterns do not introduce raw HIR");
    }

    #[test]
    fn parses_borrow_block_with_lifetime_binding() {
        let tree = parse_source(
            r"
flow #flow.borrow borrow {
    borrow bg.pixels() as pixels: &'asset [Rgba8] {
        let average = pixels.average_color()
    }
}
",
        )
        .expect("borrow block parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::BorrowBlock(block) = &flow.body()[0] else {
            panic!("expected borrow block");
        };
        assert!(matches!(block.source(), Expr::MethodCall { .. }));
        assert!(matches!(
            block.binding(),
            Pattern::Typed {
                name,
                ty: TypeRef::Ref { .. }
            } if name == "pixels"
        ));
        assert!(matches!(
            &block.body()[0],
            FlowItem::Stmt(Stmt::Let {
                expr: Expr::MethodCall { .. },
                ..
            })
        ));

        let hir = lower_to_hir(&tree).expect("borrow block lowers");
        assert!(matches!(&hir.flows()[0].body()[0], HirFlowItem::Borrow(_)));
    }

    #[test]
    fn typecheck_rejects_borrow_block_across_await_boundary() {
        let tree = parse_source(
            r"
flow #flow.borrow borrow {
    borrow bg.pixels() as pixels: &'asset [Rgba8] {
        try await load_avatar() with { pending p => scene #scene.loading { progress p.ratio } }
    }
}
",
        )
        .expect("borrow block await fixture parses");
        let hir = lower_to_hir(&tree).expect("borrow block await fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
            .with_method(
                TypeKind::Named("ImageHandle".to_owned()),
                "pixels",
                TypeKind::Named("&'asset [Rgba8]".to_owned()),
            )
            .with_function(
                "load_avatar",
                TypeKind::Need {
                    ready: Box::new(TypeKind::Unit),
                    error: Box::new(TypeKind::Named("AssetError".to_owned())),
                },
            );

        let errors = typecheck_hir(&hir, &env).expect_err("borrow block cannot cross await");
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("suspension boundary"))
        );
    }

    #[test]
    fn typecheck_rejects_borrow_across_await_boundary() {
        let tree = parse_source(
            r"
flow #flow.borrow borrow {
    let pixels: &'asset [Rgba8] = bg.pixels()
    try await load_avatar() with { pending p => scene #scene.loading { progress p.ratio } }
}
",
        )
        .expect("borrow across await fixture parses");
        let hir = lower_to_hir(&tree).expect("borrow across await fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
            .with_method(
                TypeKind::Named("ImageHandle".to_owned()),
                "pixels",
                TypeKind::Named("Pixels".to_owned()),
            )
            .with_function(
                "load_avatar",
                TypeKind::Need {
                    ready: Box::new(TypeKind::Unit),
                    error: Box::new(TypeKind::Named("AssetError".to_owned())),
                },
            );
        let errors = typecheck_hir(&hir, &env).expect_err("borrow cannot cross await");

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("suspension boundary"))
        );
    }

    #[test]
    fn typecheck_rejects_borrow_across_yield_spawn_and_defer_boundaries() {
        for boundary in ["yield frame", "spawn load_avatar()", "defer cleanup()"] {
            let tree = parse_source(format!(
                r"
flow #flow.borrow borrow {{
    let pixels: &'asset [Rgba8] = bg.pixels()
    {boundary}
}}
"
            ))
            .expect("borrow boundary fixture parses");
            let hir = lower_to_hir(&tree).expect("borrow boundary fixture lowers");
            let env = TypeCheckEnv::new()
                .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
                .with_symbol("frame", TypeKind::Named("Frame".to_owned()))
                .with_function("load_avatar", TypeKind::Named("Task".to_owned()))
                .with_function("cleanup", TypeKind::Unit)
                .with_method(
                    TypeKind::Named("ImageHandle".to_owned()),
                    "pixels",
                    TypeKind::Named("Pixels".to_owned()),
                );
            let errors = typecheck_hir(&hir, &env).expect_err("borrow cannot cross boundary");

            assert!(
                errors
                    .iter()
                    .any(|error| error.message().contains("suspension boundary")),
                "expected suspension-boundary error for {boundary}"
            );
        }
    }

    #[test]
    fn parses_if_and_match_flow_blocks_for_hir() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    if state.ready {
        goto #flow.ready
    }
    match next {
        None => goto #flow.title
        _ => goto #flow.fallback
    }
}
",
        )
        .expect("if and match parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        assert!(
            matches!(&flow.body()[0], FlowItem::If(block) if matches!(block.condition(), Expr::Path(path) if path == "state.ready"))
        );
        assert!(matches!(&flow.body()[1], FlowItem::Match(block) if block.arms().len() == 2));

        let hir = lower_to_hir(&tree).expect("if and match lower");
        assert!(matches!(&hir.flows()[0].body()[0], HirFlowItem::If(_)));
        assert!(matches!(&hir.flows()[0].body()[1], HirFlowItem::Match(_)));
    }

    #[test]
    fn typechecks_if_and_match_flow_blocks() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    if !state.ready {
        goto #flow.ready
    }
    match next {
        None => goto #flow.title
        _ => goto #flow.fallback
    }
}
",
        )
        .expect("if and match fixture parses");
        let hir = lower_to_hir(&tree).expect("if and match fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("state.ready", TypeKind::Bool)
            .with_symbol("next", TypeKind::Named("Option<Ref<Flow>>".to_owned()));

        typecheck_hir(&hir, &env).expect("if and match fixture typechecks");
    }

    #[test]
    fn typecheck_rejects_unary_not_on_non_bool_expression() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    if !state.count {
        goto #flow.ready
    }
}
",
        )
        .expect("unary not fixture parses");
        let hir = lower_to_hir(&tree).expect("unary not fixture lowers");
        let errors = typecheck_hir(
            &hir,
            &TypeCheckEnv::new().with_symbol("state.count", TypeKind::Int),
        )
        .expect_err("unary not on non-bool is rejected");

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("not operand"))
        );
    }

    #[test]
    fn typechecks_statement_match_arm_guards_and_bindings() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    match state.route_override {
        .Some(route) when route_enabled => goto route
        _ => goto #flow.title
    }
}
",
        )
        .expect("guarded match fixture parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Match(block) = &flow.body()[0] else {
            panic!("expected statement match block");
        };
        assert!(block.arms()[0].guard().is_some());

        let hir = lower_to_hir(&tree).expect("guarded match fixture lowers");
        validate_typecheck_ready(&hir).expect("guarded match is typecheck-ready");
        let env = TypeCheckEnv::new()
            .with_symbol(
                "state.route_override",
                TypeKind::Named("Option<Ref<Flow>>".to_owned()),
            )
            .with_symbol("route_enabled", TypeKind::Bool);
        typecheck_hir(&hir, &env).expect("guarded match binds route and typechecks goto");
    }

    #[test]
    fn typecheck_rejects_statement_match_non_bool_guard() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    match state.route_override {
        .Some(route) when route_count => goto route
        _ => goto #flow.title
    }
}
",
        )
        .expect("non-bool guarded match fixture parses");
        let hir = lower_to_hir(&tree).expect("non-bool guarded match fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol(
                "state.route_override",
                TypeKind::Named("Option<Ref<Flow>>".to_owned()),
            )
            .with_symbol("route_count", TypeKind::Int);
        let errors = typecheck_hir(&hir, &env).expect_err("non-bool match guard is rejected");
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("match arm guard must have type Bool")
        }));
    }

    #[test]
    fn typechecks_flow_contract_expressions() {
        let tree = parse_source(
            r"
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError>
requires delta >= -100 && delta <= 100
ensures check result.affection[character] >= 0
effects { asset.read, ui.show }
ensures no_effect network.request
{
    goto #flow.title
}
",
        )
        .expect("contract typecheck fixture parses");
        let hir = lower_to_hir(&tree).expect("contract typecheck fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("delta", TypeKind::Int)
            .with_symbol("progress", TypeKind::Float)
            .with_symbol(
                "result.affection",
                TypeKind::Named("Map<Character, Int>".to_owned()),
            )
            .with_symbol("character", TypeKind::Ref(EntityKind::Character))
            .with_symbol("asset.read", TypeKind::Named("Effect".to_owned()))
            .with_symbol("ui.show", TypeKind::Named("Effect".to_owned()))
            .with_symbol("network.request", TypeKind::Named("Effect".to_owned()))
            .with_index(
                TypeKind::Named("Map<Character, Int>".to_owned()),
                TypeKind::Int,
            );

        typecheck_hir(&hir, &env).expect("contract expressions typecheck");
    }

    #[test]
    fn validates_hir_entity_references_against_registry() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    choice #choice.opening.first {
        #choice.opening.listen "聞く" -> #flow.alice_intro
    }
}

flow #flow.alice_intro alice_intro {
    goto #flow.opening
}
"#,
        )
        .expect("registry fixture parses");
        let hir = lower_to_hir(&tree).expect("registry fixture lowers");
        let registry = registry_from_hir(&hir);

        validate_hir_references(&hir, &registry).expect("all local refs resolve");
    }

    #[test]
    fn reports_unresolved_hir_entity_reference() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    goto #flow.missing
}
",
        )
        .expect("missing ref fixture parses");
        let hir = lower_to_hir(&tree).expect("missing ref fixture lowers");
        let registry = NameRegistry::new().with_entity("flow.opening", EntityKind::Flow);
        let errors = validate_hir_references(&hir, &registry).expect_err("missing ref should fail");

        assert!(errors[0].message().contains("flow.missing"));
    }

    #[test]
    fn typechecks_await_wait_view_branches() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    try await load_avatar() with {
        pending p => scene #scene.loading { progress p.ratio }
        ready img => Image(img)
        error _ => Icon(#asset.avatar_fallback)
        denied _ => return Ok(FlowExit::Goto(#flow.title))
    }
}
",
        )
        .expect("await branch typecheck fixture parses");
        let hir = lower_to_hir(&tree).expect("await branch typecheck fixture lowers");
        let env = TypeCheckEnv::new()
            .with_function(
                "load_avatar",
                TypeKind::Need {
                    ready: Box::new(TypeKind::Named("Image".to_owned())),
                    error: Box::new(TypeKind::Named("AvatarError".to_owned())),
                },
            )
            .with_function("Image", TypeKind::Named("View".to_owned()))
            .with_function("Icon", TypeKind::Named("View".to_owned()))
            .with_function("Ok", TypeKind::Named("Result".to_owned()))
            .with_function("FlowExit::Goto", TypeKind::Named("FlowExit".to_owned()))
            .with_symbol("img", TypeKind::Named("Image".to_owned()));

        typecheck_hir(&hir, &env).expect("await wait-view branches typecheck");
    }

    #[test]
    fn lowers_edge_case_flow_to_hir_without_raw_reparse() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    @bg #asset.bg.room fade=300ms
    let (actor, (_, voice)) = alice.say()[聞いて。[p]]
    try await load_opening_assets() with { pending p => scene #scene.loading { progress p.ratio } }
    alice[
        今日は｜変な夢《へんなゆめ》を見たんだ。[p]
    ]
    with:
        at(end-250ms): alice.stage.face(worried)
    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" if state.affection[#character.alice] >= 3 -> #flow.alice_intro
    }
    goto #flow.title
}
"#,
        )
        .expect("edge flow parses");

        let hir = lower_to_hir(&tree).expect("edge flow lowers");
        let flow = &hir.flows()[0];
        assert!(
            flow.body()
                .iter()
                .any(|item| matches!(item, HirFlowItem::Stmt(Stmt::Let { .. })))
        );
        assert!(flow.body().iter().any(
            |item| matches!(item, HirFlowItem::Await(await_with) if await_with.applies_try())
        ));
        assert!(flow.body().iter().any(
            |item| matches!(item, HirFlowItem::Dialogue(dialogue) if dialogue.callee() == "alice")
        ));
        assert!(flow.body().iter().any(
            |item| matches!(item, HirFlowItem::Dialogue(dialogue) if dialogue.plan().is_some())
        ));
        assert!(flow
            .body()
            .iter()
            .any(|item| matches!(item, HirFlowItem::Choice(choice) if choice.options()[0].condition().is_some())));
    }

    #[test]
    fn collects_hir_symbol_uses_for_type_checking_without_reparsing() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    let (actor, (_, voice)) = alice.say()[聞いて。[p]]
    alice[
        #[fmt("夢", color=blue)]を見た。[p]
    ]
    with:
        at(0.42s): alice.stage.face(worried)
    choice #choice.opening.first {
        #choice.opening.listen "聞く" if state.affection[#character.alice] >= 3 -> #flow.alice_intro
    }
}
"#,
        )
        .expect("symbol fixture parses");
        let hir = lower_to_hir(&tree).expect("symbol fixture lowers");
        let uses = collect_symbol_uses(&hir);

        assert!(uses.iter().any(
            |symbol| symbol.kind() == SymbolUseKind::DialogueCallee && symbol.name() == "alice"
        ));
        assert!(
            uses.iter()
                .any(|symbol| symbol.kind() == SymbolUseKind::Method && symbol.name() == "face")
        );
        assert!(
            uses.iter()
                .any(|symbol| symbol.kind() == SymbolUseKind::EntityRef
                    && symbol.name() == "character.alice")
        );
        assert!(
            uses.iter()
                .all(|symbol| symbol.kind() != SymbolUseKind::RawExpr)
        );
        validate_typecheck_ready(&hir).expect("edge fixture is typecheck-ready");
    }

    #[test]
    fn typecheck_readiness_rejects_raw_dialogue_expressions() {
        let tree = parse_source(
            r#"
alice[
    #[fmt("夢", color=)]を見た。[p]
]
"#,
        )
        .expect("raw dialogue expression fixture parses lossily");
        let hir = lower_to_hir(&tree).expect("raw dialogue expression still lowers");
        let errors = validate_typecheck_ready(&hir).expect_err("raw expr blocks type checking");

        assert!(errors[0].message().contains("raw expression"));
    }

    #[test]
    fn typechecks_edge_case_hir_with_explicit_environment() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    @show alice normal at=right fade=220ms
    let (actor, (_, voice)) = alice.say(voice=auto)[聞いて。[p]]
    try await load_opening_assets() with { pending p => scene #scene.loading { progress p.ratio } }
    alice[
        #[fmt("夢", color=blue)]を見た。[p]
    ]
    with:
        at(0.42s): alice.stage.face(worried)
    choice #choice.opening.first {
        #choice.opening.listen "聞く" if state.affection[#character.alice] >= 3 -> #flow.alice_intro
    }
    goto #flow.title
}
"#,
        )
        .expect("typecheck fixture parses");
        let hir = lower_to_hir(&tree).expect("typecheck fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
            .with_symbol("alice.stage", TypeKind::Named("StageActor".to_owned()))
            .with_symbol("auto", TypeKind::Named("VoicePolicy".to_owned()))
            .with_symbol("blue", TypeKind::Named("Color".to_owned()))
            .with_symbol("normal", TypeKind::Named("Pose".to_owned()))
            .with_symbol("right", TypeKind::Named("StagePosition".to_owned()))
            .with_symbol("worried", TypeKind::Named("Face".to_owned()))
            .with_symbol("end", TypeKind::Duration)
            .with_symbol(
                "state.affection",
                TypeKind::Named("Map<Ref<Character>, Int>".to_owned()),
            )
            .with_function("fmt", TypeKind::DisplayText)
            .with_function(
                "load_opening_assets",
                TypeKind::Need {
                    ready: Box::new(TypeKind::Unit),
                    error: Box::new(TypeKind::Named("AssetError".to_owned())),
                },
            )
            .with_method(
                TypeKind::Ref(EntityKind::Character),
                "say",
                TypeKind::Named("SayBuilder".to_owned()),
            )
            .with_method(
                TypeKind::Named("StageActor".to_owned()),
                "face",
                TypeKind::Named("StageCue".to_owned()),
            )
            .with_index(
                TypeKind::Named("Map<Ref<Character>, Int>".to_owned()),
                TypeKind::Int,
            );

        typecheck_hir(&hir, &env).expect("edge fixture typechecks");
    }

    #[test]
    fn typechecks_fragment_hir_and_include_target() {
        let tree = parse_source(
            r"
pub fragment #frag.alice_enters alice_enters: FlowFragment {
    alice: おはよう。[p]
}

flow #flow.opening opening {
    include #frag.alice_enters
}
",
        )
        .expect("fragment include fixture parses");
        let hir = lower_to_hir(&tree).expect("fragment include fixture lowers");
        let env = TypeCheckEnv::new().with_symbol("alice", TypeKind::Ref(EntityKind::Character));

        assert_eq!(hir.flows()[0].kind(), FlowKind::Fragment);
        typecheck_hir(&hir, &env).expect("fragment include fixture typechecks");
    }

    #[test]
    fn parses_lowers_and_typechecks_documented_source_item() {
        let tree = parse_source(
            r#"
pub source #source.face_camera_frames: Source<VideoFrameHandle, CaptureError> {
    from capture.camera(#capture.face_camera)
    backpressure = latest
    replay = hash_only
    privacy = transient

    on item frame => yield frame
    on disconnected => emit signal #signal.camera_connected <- false
    on error e => log warn "camera stream error {err:?}" { err = e }
}
"#,
        )
        .expect("documented source item parses");
        let Item::Source(source) = &tree.items()[0] else {
            panic!("expected source item");
        };
        assert_eq!(
            source.id().map(EntityRef::body),
            Some("source.face_camera_frames")
        );
        assert!(source.signature_tail().contains("Source<VideoFrameHandle"));
        assert!(source.body_statements().iter().any(|stmt| matches!(
            stmt,
            Stmt::On { head, body }
                if head == "item frame" && matches!(body.as_slice(), [Stmt::Yield(_)])
        )));
        assert!(source.body_statements().iter().any(|stmt| matches!(
            stmt,
            Stmt::On { head, body }
                if head == "disconnected" && matches!(body.as_slice(), [Stmt::Signal { .. }])
        )));

        let hir = lower_to_hir(&tree).expect("documented source item lowers");
        assert!(matches!(
            hir.declarations(),
            [HirTopLevelDecl::Source(source)] if source.id().is_some()
        ));
        validate_typecheck_ready(&hir).expect("source item is typecheck-ready");
        let env = TypeCheckEnv::new()
            .with_symbol("capture", TypeKind::Named("CaptureApi".to_owned()))
            .with_symbol("latest", TypeKind::Named("BackpressurePolicy".to_owned()))
            .with_symbol("hash_only", TypeKind::Named("ReplayPolicy".to_owned()))
            .with_symbol("transient", TypeKind::Named("PrivacyPolicy".to_owned()))
            .with_method(
                TypeKind::Named("CaptureApi".to_owned()),
                "camera",
                TypeKind::Named("CaptureStream".to_owned()),
            )
            .with_function("log.warn", TypeKind::Unit);
        typecheck_hir(&hir, &env).expect("documented source item typechecks");
    }

    #[test]
    fn parses_function_like_source_with_loop_yield_body() {
        let tree = parse_source(
            r"
source camera_frames() -> Source<VideoFrame, CameraError> {
    loop {
        let frame = await camera.next_frame()
        yield frame
    }
}
",
        )
        .expect("function-like source parses");
        let Item::Source(source) = &tree.items()[0] else {
            panic!("expected source item");
        };
        assert_eq!(source.name(), Some("camera_frames"));
        assert!(matches!(
            source.body_statements(),
            [Stmt::Loop { body }] if matches!(body.as_slice(), [Stmt::Let { .. }, Stmt::Yield(_)])
        ));

        let hir = lower_to_hir(&tree).expect("function-like source lowers");
        validate_typecheck_ready(&hir).expect("function-like source is typecheck-ready");
        let env = TypeCheckEnv::new()
            .with_symbol("camera", TypeKind::Named("Camera".to_owned()))
            .with_method(
                TypeKind::Named("Camera".to_owned()),
                "next_frame",
                TypeKind::Need {
                    ready: Box::new(TypeKind::Named("VideoFrame".to_owned())),
                    error: Box::new(TypeKind::Named("CameraError".to_owned())),
                },
            );
        typecheck_hir(&hir, &env).expect("function-like source typechecks");
    }

    #[test]
    fn parses_entity_declarations_used_by_presentation_docs() {
        let tree = parse_source(
            r"
pub signal #signal.microphone_level: Watch<f32>

pub character #character.alice Alice {
    role = main
    nameplate = visible
}

pub layer #layer.ui.game: NativeUi {
    phase = Ui
    z = 500
}

activity #activity.truck_game TruckGame {
    mode = embedded
}

component #ui.settings SettingsPanel(config: Binding<Config>) -> View {
    SettingsView(config)
}
",
        )
        .expect("entity declarations parse");
        let kinds = tree
            .items()
            .iter()
            .map(|item| match item {
                Item::EntityDecl(item) => item.kind(),
                other => panic!("expected entity declaration, got {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                EntityDeclKind::Signal,
                EntityDeclKind::Character,
                EntityDeclKind::Layer,
                EntityDeclKind::Activity,
                EntityDeclKind::Component,
            ]
        );

        let hir = lower_to_hir(&tree).expect("entity declarations lower");
        validate_typecheck_ready(&hir).expect("entity declarations are typecheck-ready");
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect("entity declarations typecheck");

        let registry = registry_from_hir(&hir);
        validate_hir_references(&hir, &registry).expect("declaration ids register themselves");
    }

    #[test]
    fn parses_bodyless_parser_declarations_from_docs() {
        let tree = parse_source(
            r"
pub parser parse_player_command: Parser<PlayerCommand, ParseError>
pub parser parse_image_header<'a>: Parser<ImageHeader<'a>, ParseError>
",
        )
        .expect("bodyless parser declarations parse");
        assert_eq!(tree.items().len(), 2);
        for item in tree.items() {
            let Item::Parser(parser) = item else {
                panic!("expected parser declaration");
            };
            assert!(parser.body().is_empty());
            assert!(parser.body_statements().is_empty());
            assert!(parser.signature_tail().contains("Parser<"));
        }

        let hir = lower_to_hir(&tree).expect("bodyless parser declarations lower");
        validate_typecheck_ready(&hir).expect("bodyless parser declarations are typecheck-ready");
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect("bodyless parser declarations typecheck");
    }

    #[test]
    fn parses_extern_rust_module_declaration_from_docs() {
        let tree = parse_source(
            r#"
extern rust mod mini_games::truck from crate "truck_game" {
    pub event TruckEvent
    pub type TruckResult
    pub fn score_to_rank(score: i32) -> Rank
    pub activity truck_game: Activity<TruckInput, TruckResult>
}
"#,
        )
        .expect("extern rust module parses");
        let Item::ExternMod(item) = &tree.items()[0] else {
            panic!("expected extern module item");
        };
        assert_eq!(item.abi(), "rust");
        assert_eq!(item.path(), "mini_games::truck");
        assert_eq!(item.source(), Some(r#"crate "truck_game""#));
        assert!(item.body().contains("pub activity truck_game"));

        let hir = lower_to_hir(&tree).expect("extern rust module lowers");
        assert!(matches!(
            hir.declarations(),
            [HirTopLevelDecl::ExternMod(_)]
        ));
        validate_typecheck_ready(&hir).expect("extern module is typecheck-ready");
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect("extern module typechecks");
    }

    #[test]
    fn typecheck_reports_wrong_choice_target_kind() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    choice #choice.opening.first {
        #choice.opening.listen "聞く" -> #asset.bg.room
    }
}
"#,
        )
        .expect("bad choice target fixture parses");
        let hir = lower_to_hir(&tree).expect("bad choice target lowers");
        let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
            .expect_err("choice target must be a flow ref");

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("choice target"))
        );
    }

    #[test]
    fn lowering_rejects_unstructured_raw_items() {
        let tree = parse_source("unknown top level syntax").expect("raw item is syntax-preserved");
        let errors = lower_to_hir(&tree).expect_err("raw item cannot lower");
        assert!(errors[0].message().contains("raw"));
    }
}
