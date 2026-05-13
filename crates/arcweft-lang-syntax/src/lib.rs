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
    ChoiceBlock, ChoiceItem, ChoiceOption, ChoicePlan, ChoicePlanItem, ContentCall, ContractClause,
    DialogueToken, EntityRef, EnumItem, EnumVariant, Flow, FlowItem, FlowKind, ForBlock,
    FunctionItem, HookItem, IfBlock, ImplItem, Item, LineOptions, LinePlan, LinePlanItem, MatchArm,
    MatchBlock, MemoFn, ModuleDecl, ParserItem, Pattern, ScenarioCommand, ScopeBlock, SelectBlock,
    SelectBranch, SelectBranchHead, SourceLocaleBlock, SpeakerLine, StateField, StateItem, Stmt,
    StructField, StructItem, SyntaxTree, TextRange, TraitItem, TraitMember, TypeAliasItem, UseItem,
    Visibility, WikiLink,
};
pub use check::{
    EntityKind, TypeCheckEnv, TypeCheckError, TypeCheckReadinessError, TypeKind, typecheck_hir,
    validate_typecheck_ready,
};
pub use expr::{BinaryOp, Expr, Literal, Placeholder, parse_expr};
pub use lower::{
    HirAwait, HirAwaitBranch, HirBorrow, HirChoice, HirChoiceOption, HirDialogue, HirFlow,
    HirFlowItem, HirFor, HirIf, HirLowerError, HirMatch, HirMatchArm, HirModule, HirScope,
    HirSelect, HirSelectBranch, lower_to_hir,
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
        ContractClause, DialogueToken, EntityKind, Expr, FlowItem, FlowKind, HirFlowItem, Item,
        LinePlanItem, NameRegistry, Pattern, SelectBranchHead, Stmt, SymbolUseKind, TraitMember,
        TypeCheckEnv, TypeKind, TypeRef, Visibility, collect_symbol_uses, lower_to_hir,
        parse_dialogue_tokens, parse_fn_signature, parse_source, parse_stub, parse_type_ref,
        registry_from_hir, typecheck_hir, validate_hir_references, validate_typecheck_ready,
    };

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
        assert!(matches!(&plan.items()[1], LinePlanItem::CancelRule(_)));
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
    fn parses_compat_at_bracket_timed_cue() {
        let tree = parse_source(
            r"
alice[おはよう。[p]]
with:
    at(0.42s)[alice.stage.face(worried)]
",
        )
        .expect("compat at bracket cue parses");

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
        assert!(matches!(&choice.items()[0], ChoiceItem::For { .. }));
        let option = &choice.options()[0];
        assert!(option.label_text_key().is_some());
        assert!(option.value().is_some());
        assert!(matches!(option.action(), ChoiceAction::Out(_)));
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
        assert_eq!(tree.uses()[1].tree(), "parent::common::{route_gate}");

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

        assert!(matches!(&tree.items()[0], Item::Hook(_)));
        assert!(matches!(&tree.items()[1], Item::MemoFn(_)));
        assert!(matches!(&tree.items()[2], Item::Parser(_)));
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
        assert!(matches!(state.fields()[1].default(), Expr::Raw(raw) if raw == "Config {}"));

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
            TraitMember::AssociatedType { name, value: None } if name == "Item"
        ));
        assert!(matches!(
            &mappable.members()[2],
            TraitMember::Function { signature } if signature.starts_with("fn map")
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
        assert!(impl_item.body().contains("Some(x)"));

        let hir = lower_to_hir(&tree).expect("syntax-only trait/impl items do not block HIR");
        assert!(hir.flows().is_empty());
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

        let hir = lower_to_hir(&tree).expect("function item is syntax-only for now");
        assert_eq!(hir.flows().len(), 1);
        assert_eq!(hir.flows()[0].id().expect("flow id").body(), "flow.opening");
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
            SelectBranchHead::Event(Pattern::Variant(name)) if name == ".Back"
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
        .expect("if and match fixture parses");
        let hir = lower_to_hir(&tree).expect("if and match fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("state.ready", TypeKind::Bool)
            .with_symbol("next", TypeKind::Named("Option<Ref<Flow>>".to_owned()));

        typecheck_hir(&hir, &env).expect("if and match fixture typechecks");
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
