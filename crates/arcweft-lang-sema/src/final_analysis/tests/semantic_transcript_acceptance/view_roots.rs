use super::*;
use crate::semantic_coordinate::{CheckedSemanticPath, CheckedSemanticPathStep};
use arcweft_lang_hir::{
    body_edges::HirBodyChild,
    item::HirItemKind,
    project::{
        HirDeclarationBodyRootRole, HirDeclarationParameterRootChild,
        HirDeclarationParameterRootRole,
    },
};

fn assert_view_roots(source: &str, parameter_count: usize, match_ordinal: u32) {
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("View root matrix checks");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let symbol = world
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.owner() == arcweft_lang_hir::symbol::CallableDeclarationOwner::View)
        .expect("one View declaration");
    let declaration = report
        .accepted_root_catalog()
        .topology()
        .declaration(symbol.declaration())
        .expect("accepted View topology");
    let HirItemKind::View(view) = module
        .resolve_item(declaration.body().source_item())
        .expect("View item")
        .kind()
    else {
        panic!("View declaration owner")
    };
    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    assert_eq!(view.parameters().len(), parameter_count);
    assert_eq!(declaration.body().parameter_roots().len(), parameter_count);
    let mut parameter_paths = Vec::new();
    for (index, (parameter, root)) in view
        .parameters()
        .iter()
        .zip(declaration.body().parameter_roots())
        .enumerate()
    {
        let parameter_index = u32::try_from(index).expect("small parameter inventory");
        assert_eq!(
            root.role(),
            HirDeclarationParameterRootRole::Pattern {
                group: 0,
                parameter: parameter_index
            }
        );
        assert_eq!(
            root.child(),
            HirDeclarationParameterRootChild::Pattern(parameter.pattern())
        );
        let pattern = report
            .pattern(parameter.pattern())
            .expect("checked View parameter pattern");
        let pattern_path = coordinates
            .pattern(parameter.pattern())
            .expect("accepted parameter path");
        assert_eq!(
            pattern_path.path().steps(),
            [CheckedSemanticPathStep::ParameterPattern {
                group: 0,
                parameter: parameter_index
            }]
        );
        for local in parameter.locals() {
            assert_eq!(
                report
                    .local(*local)
                    .expect("checked parameter binding")
                    .ty(),
                pattern.ty()
            );
            let binding = coordinates
                .binding(*local)
                .expect("accepted parameter binding coordinate");
            assert_eq!(binding.root(), pattern_path.path().root());
        }
        parameter_paths.push(pattern_path.path().clone());
    }
    assert_eq!(view.values().len(), 3, "prefix, Match, suffix values");
    assert_eq!(declaration.body().roots().len(), 3);
    let mut value_paths = Vec::new();
    for (index, (value, root)) in view
        .values()
        .iter()
        .zip(declaration.body().roots())
        .enumerate()
    {
        let ordinal = u32::try_from(index).expect("three View values");
        assert_eq!(
            root.role(),
            HirDeclarationBodyRootRole::ViewValue { ordinal }
        );
        let [child] = root.projection().children() else {
            panic!("one expression per View value root")
        };
        assert_eq!(child.child(), HirBodyChild::Expression(*value));
        assert!(report.expression(*value).is_some(), "checked View value");
        let path = coordinates
            .expression(*value)
            .expect("accepted View value path");
        assert_eq!(
            path.steps(),
            [CheckedSemanticPathStep::DeclarationBody(
                HirDeclarationBodyRootRole::ViewValue { ordinal }
            )]
        );
        value_paths.push(path);
    }
    let match_owner = view.values()[usize::try_from(match_ordinal).expect("small ordinal")];
    assert!(matches!(
        module
            .resolve_expr(match_owner)
            .expect("Match value")
            .kind(),
        HirExprKind::Match(_)
    ));
    let match_path = &value_paths[usize::try_from(match_ordinal).expect("small ordinal")];
    assert!(
        parameter_paths
            .iter()
            .all(|path| !path.is_at_or_below(match_path)),
        "declaration parameters are separate from Match descendants"
    );
    assert!(
        value_paths.iter().enumerate().all(|(index, path)| {
            index == usize::try_from(match_ordinal).expect("small ordinal")
                || !path.is_at_or_below(match_path)
        }),
        "sibling View values do not enter the Match transcript"
    );
    assert_captures_below_view_match(&report, &coordinates, match_path, &parameter_paths);
    let product =
        super::checked_match_product(&report, project, module, &world.symbols, match_owner);
    assert_eq!(product.arms().len(), 2, "accepted exhaustive View Match");
}

fn assert_captures_below_view_match(
    report: &FinalSemanticAnalysis,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    match_path: &CheckedSemanticPath,
    parameters: &[CheckedSemanticPath],
) {
    let mut capture_count = 0;
    for (owner, checked) in report.expressions() {
        let CheckedExpressionResolution::Closure(closure) = checked.resolution() else {
            continue;
        };
        let path = coordinates
            .expression(owner)
            .expect("accepted handler closure path");
        assert!(
            path.is_at_or_below(match_path),
            "handler closure belongs to a Match arm"
        );
        for selected in closure.captures() {
            let capture = selected.capture();
            let checked = report.capture(capture).expect("checked capture binding");
            assert_eq!(report.selected_capture(capture), Some(selected));
            let binding = coordinates
                .binding(selected.local())
                .expect("capture source binding coordinate");
            assert!(
                parameters
                    .iter()
                    .any(|path| binding.path().is_at_or_below(path)),
                "capture joins its View parameter pattern path: {:?}; parameters: {parameters:?}",
                binding.path()
            );
            assert_eq!(
                report
                    .local(selected.local())
                    .expect("captured View parameter")
                    .ty(),
                checked.ty()
            );
            capture_count += 1;
        }
    }
    assert_eq!(
        capture_count,
        report.captures().len(),
        "all selected handler captures were checked"
    );
}

#[test]
fn checked_match_transcript_preserves_view_parameter_and_value_root_paths() {
    let original = r#"
view Main(first: f32, second: f32) {
    Text("prefix")
    match true {
        true => Button().fx(wave(speed = first))
        false => Button().fx(wave(speed = second))
    }
    Text("suffix")
}
"#;
    let changed = original.replace("speed = first", "speed = second");
    let revised = format!(
        "fn unrelated() -> i64 {{ 99i64 }}\n{}",
        original.replace("match true", "match  true")
    );
    for source in [original, changed.as_str(), revised.as_str()] {
        assert_view_roots(source, 2, 1);
    }
    super::body_roots::assert_match_sensitivity_and_source_revision_invariance(
        "View body value",
        original,
        &changed,
        &revised,
    );
    let sibling = original
        .replace("prefix", "changed prefix")
        .replace("suffix", "changed suffix");
    assert_eq!(
        source_match_digest(original),
        source_match_digest(&sibling),
        "sibling View value changes stay outside Match meaning"
    );
    let relocated = original.replace("    Text(\"prefix\")\n", "").replace(
        "    Text(\"suffix\")",
        "    Text(\"prefix\")\n    Text(\"suffix\")",
    );
    assert_view_roots(&relocated, 2, 0);
    assert_ne!(
        source_match_digest(original),
        source_match_digest(&relocated),
        "moving Match to a different authored View value coordinate changes its identity"
    );
}

#[test]
fn checked_match_transcript_preserves_view_handler_capture_meaning() {
    let original = r#"
view Main(first: DialogueView, second: DialogueView) {
    Text("prefix")
    match true {
        true => Button().on_click { first.primary_action }
        false => Button().on_click { second.primary_action }
    }
    Text("suffix")
}
"#;
    let changed = original.replace("first.primary_action", "second.primary_action");
    let revised = format!(
        "fn unrelated() -> i64 {{ 99i64 }}\n{}",
        original.replace("match true", "match  true")
    );
    for source in [original, changed.as_str(), revised.as_str()] {
        assert_view_roots(source, 2, 1);
    }
    let world = super::fixture(original, None);
    let report = super::analyze(&world).expect("handler Match checks");
    assert_eq!(
        report.captures().len(),
        2,
        "both handler closures capture their View parameter"
    );
    super::body_roots::assert_match_sensitivity_and_source_revision_invariance(
        "View handler capture",
        original,
        &changed,
        &revised,
    );
}
