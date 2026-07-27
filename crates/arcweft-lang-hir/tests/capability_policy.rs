use arcweft_lang_hir::{lower::lower_document_to_hir, model::HirTopLevelDecl};
use arcweft_lang_syntax::{
    expr::Expr,
    parser::{ParseOptions, parse_document_with_source},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

fn effect_label(effect: &Expr) -> Option<String> {
    match effect {
        Expr::Path(path) => Some(path.as_label().to_owned()),
        Expr::Select(select) => Some(format!(
            "{}.{}",
            effect_label(select.target())?,
            select.member()
        )),
        _ => None,
    }
}

#[test]
fn retained_capability_lowers_only_function_effect_facts() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://lang-hir/capability/retained-functions.arcw")
                .expect("retained capability fixture source ID"),
            SourceName::path("lang-hir/capability/retained-functions.arcw"),
            r"
pub extern capability fs {
    type Path

    fn read_text(path: Path) -> Need<String, FsError>
        effects { fs.read }

    fn write_text(path: Path)(text: String) -> Need<Unit, FsError>
        effects { fs.write }
}
",
        )
        .expect("retained capability fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());

    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .expect("canonical capability lowers");
    let [HirTopLevelDecl::ExternCapability(capability)] = hir.declarations() else {
        panic!("expected one external capability declaration");
    };

    assert_eq!(capability.id(), "fs");
    assert_eq!(
        capability
            .functions()
            .iter()
            .map(|function| function.signature().name())
            .collect::<Vec<_>>(),
        ["read_text", "write_text"]
    );
    assert_eq!(
        capability
            .functions()
            .iter()
            .map(|function| {
                let [effect] = function.effects() else {
                    panic!("expected one path effect per function");
                };
                effect_label(effect).expect("effect is a dotted path")
            })
            .collect::<Vec<_>>(),
        ["fs.read", "fs.write"]
    );
}

#[test]
fn candidate_member_never_contributes_a_hir_policy_fact() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://lang-hir/capability/recovered-policy.arcw")
                .expect("recovered capability fixture source ID"),
            SourceName::path("lang-hir/capability/recovered-policy.arcw"),
            r"
extern capability fs {
    policy legacy { allow = fs.read }
    fn read_text(path: String) -> String effects { fs.read }
}
",
        )
        .expect("recovered capability fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());

    // The public parser's atomic grammar switch is a later stage. This test
    // only fixes the current HIR boundary: recovery must never synthesize a
    // capability-policy declaration or displace retained functions.
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .expect("retained capability function lowers");
    let [HirTopLevelDecl::ExternCapability(capability)] = hir.declarations() else {
        panic!("expected one external capability declaration");
    };

    assert_eq!(capability.functions().len(), 1);
    assert_eq!(capability.functions()[0].signature().name(), "read_text");
}
