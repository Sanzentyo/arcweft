use arcweft_lang_hir::{lower::lower_document_to_hir, model::HirTopLevelDecl};
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

#[test]
fn ordinary_struct_functions_and_entry_are_the_only_role_owners_in_hir() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://lang-hir/entry/role-owners.arcw")
                .expect("entry role fixture source ID"),
            SourceName::path("lang-hir/entry/role-owners.arcw"),
            r"
struct GameState {
    score: i32
}

fn initial_game_state() -> GameState {
    initial_game_state()
}

fn reduce_game(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
{
    reduce_game(state, event)
}

entry game @entry.game.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}
",
        )
        .expect("entry role fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .expect("ordinary role owners lower");

    assert_eq!(
        hir.functions()
            .iter()
            .map(|function| function.signature().name())
            .collect::<Vec<_>>(),
        ["initial_game_state", "reduce_game"]
    );
    assert_eq!(
        hir.declarations()
            .iter()
            .filter(|declaration| matches!(declaration, HirTopLevelDecl::Struct(_)))
            .count(),
        1
    );
    assert_eq!(
        hir.declarations()
            .iter()
            .filter(|declaration| matches!(declaration, HirTopLevelDecl::Entry(_)))
            .count(),
        1
    );
}
