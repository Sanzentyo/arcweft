//! Match-root evidence for checked facts selected by the enclosing callable,
//! dialogue, suspension, or registered presentation context.

use super::*;

struct ContextualMeaningCase {
    row: ExpressionCorpusRow,
    changed: String,
}

fn contextual_case(
    name: &'static str,
    source: String,
    changed: String,
    fixture: ExpressionCorpusFixture,
    shapes: &'static [ExprShapeFamily],
    resolutions: &'static [ExpressionResolutionFamily],
) -> ContextualMeaningCase {
    ContextualMeaningCase {
        row: ExpressionCorpusRow {
            name,
            exact_path_families: true,
            source,
            fixture,
            shapes,
            resolutions,
            values: &[],
            selects: &[],
        },
        changed,
    }
}

fn contextual_expression_corpus_cases() -> Vec<ContextualMeaningCase> {
    const DIALOGUE_SHAPES: &[ExprShapeFamily] = &[
        ExprShapeFamily::Literal,
        ExprShapeFamily::EntityReference,
        ExprShapeFamily::Path,
        ExprShapeFamily::Call,
        ExprShapeFamily::Block,
        ExprShapeFamily::Match,
        ExprShapeFamily::AttachedContentApplication,
        ExprShapeFamily::PostfixBracket,
    ];
    const DIALOGUE_RESOLUTIONS: &[ExpressionResolutionFamily] = &[
        ExpressionResolutionFamily::Structural,
        ExpressionResolutionFamily::Literal,
        ExpressionResolutionFamily::Value,
        ExpressionResolutionFamily::DialogueLineCoordinate,
        ExpressionResolutionFamily::DialogueTextKeyCoordinate,
        ExpressionResolutionFamily::CharacterDialogueFactory,
        ExpressionResolutionFamily::DialogueApplication,
        ExpressionResolutionFamily::PostfixBracket,
    ];
    const OBJECT_SHAPES: &[ExprShapeFamily] = &[
        ExprShapeFamily::Literal,
        ExprShapeFamily::EntityReference,
        ExprShapeFamily::Path,
        ExprShapeFamily::Block,
        ExprShapeFamily::Match,
        ExprShapeFamily::AttachedContentApplication,
    ];
    const OBJECT_RESOLUTIONS: &[ExpressionResolutionFamily] = &[
        ExpressionResolutionFamily::Structural,
        ExpressionResolutionFamily::Literal,
        ExpressionResolutionFamily::Value,
        ExpressionResolutionFamily::TypeValue,
        ExpressionResolutionFamily::CompileTimeScalar,
        ExpressionResolutionFamily::DialogueApplication,
        ExpressionResolutionFamily::ContentApplication,
    ];
    let await_source = |operand: &str| {
        format!(
            "fn root(flag: bool, first: Need<i64>, second: Need<i64>) -> i64 {{\n\
             match flag {{\n\
                 true => await {operand}\n\
                 false => 0i64\n\
             }}\n\
         }}\n"
        )
    };
    let implicit = |value: &str| {
        format!(
            "fn apply(callback: i64 -> i64, value: i64) -> i64 {{ callback(value) }}\n{}",
            bool_match_i64_source(&format!("apply(_ + {value}, 2i64)")),
        )
    };
    let short_variant = |variant: &str| {
        format!(
            "enum Flag {{ On, Off }}\n\
         fn root(flag: bool) -> Flag {{\n\
             match flag {{\n\
                 true => .{variant}\n\
                 false => .Off\n\
             }}\n\
         }}\n"
        )
    };
    let dialogue = |line: &str, text_key: &str| {
        format!(
            "pub character alice {{}}\n{}",
            bool_match_i64_source(&format!(
                "{{ alice(id=@say.story.{line}, text_key=@text.story.{text_key})[hello]; 1i64 }}"
            )),
        )
    };
    let reconfigure = |locale: &str| {
        format!(
            "pub character alice {{}}\n{}",
            bool_match_i64_source(&format!(
                "{{ let dialogue = alice(); let patched = dialogue(source_locale=\"{locale}\"); 1i64 }}"
            )),
        )
    };
    let line_reference = |line: &str| {
        format!(
            "pub character alice {{}}\n\
         fn opening() {{ alice(id=@say.story.first)[hello]; }}\n\
         fn alternate() {{ alice(id=@say.story.second)[world]; }}\n\
         fn root(flag: bool) -> Ref<DialogueLine> {{\n\
             match flag {{\n\
                 true => @say.story.{line}\n\
                 false => @say.story.first\n\
             }}\n\
         }}\n"
        )
    };
    let stage_look = |locale: &str| {
        format!(
            "pub character akane {{}}\n{}",
            bool_match_i64_source(&format!(
                "{{ let configured = akane(look=.normal, source_locale=\"{locale}\"); 1i64 }}"
            )),
        )
    };
    let agent_field = |field: &str| {
        format!(
            "fn root(flag: bool) -> String {{\n\
             match flag {{\n\
                 true => observation.{field}\n\
                 false => \"\"\n\
             }}\n\
         }}\n"
        )
    };
    let object = |nominal: &str, identity: &str| {
        format!(
            "#[text_proxy(role=\"keyword\", hit_test=true, channel=\"fallback\")]\n\
         pub struct KeywordHit {{ channel: String, weight: Option<i64> }}\n\
         #[text_proxy(role=\"keyword\", hit_test=true, channel=\"fallback\")]\n\
         pub struct AlternateHit {{ channel: String, weight: Option<i64> }}\n\
         pub character alice {{}}\n{}",
            bool_match_i64_source(&format!(
                "{{ alice[#object(id=@.{identity}, type={nominal}, weight=3)[typed]]; 1i64 }}"
            )),
        )
    };
    let closed_enum = |speed: &str| {
        format!(
            "pub character alice {{}}\n{}",
            bool_match_i64_source(&format!(
                "{{ alice[#fx(wave(phase=.glyph_transform, speed={speed}))[text]]; 1i64 }}"
            )),
        )
    };
    use ExprShapeFamily::{
        Await, Binary, Block, Call, EntityReference, Literal, Match, Path, Placeholder, Select,
        ShortVariant,
    };
    use ExpressionCorpusFixture::{CharacterNominal, RegisteredObservation, Standard};
    use ExpressionResolutionFamily as Resolution;
    vec![
        contextual_case(
            "Await",
            await_source("first"),
            await_source("second"),
            Standard,
            &[Literal, Path, Await, Match],
            &[
                Resolution::Structural,
                Resolution::Literal,
                Resolution::Value,
                Resolution::Await,
            ],
        ),
        contextual_case(
            "implicit callable",
            implicit("1i64"),
            implicit("3i64"),
            Standard,
            &[Literal, Path, Placeholder, Call, Binary, Match],
            &[
                Resolution::Structural,
                Resolution::Literal,
                Resolution::Value,
                Resolution::Call,
                Resolution::ImplicitCallable,
                Resolution::ImplicitParameter,
            ],
        ),
        contextual_case(
            "project short variant",
            short_variant("On"),
            short_variant("Off"),
            Standard,
            &[Path, ShortVariant, Match],
            &[
                Resolution::Structural,
                Resolution::Value,
                Resolution::Variant,
            ],
        ),
        contextual_case(
            "dialogue line coordinate",
            dialogue("greeting", "greeting"),
            dialogue("alternate", "greeting"),
            Standard,
            DIALOGUE_SHAPES,
            DIALOGUE_RESOLUTIONS,
        ),
        contextual_case(
            "dialogue text-key coordinate",
            dialogue("greeting", "greeting"),
            dialogue("greeting", "alternate"),
            Standard,
            DIALOGUE_SHAPES,
            DIALOGUE_RESOLUTIONS,
        ),
        contextual_case(
            "dialogue reconfigure",
            reconfigure("ja-JP"),
            reconfigure("en-US"),
            Standard,
            &[Literal, Path, Call, Block, Match],
            &[
                Resolution::Structural,
                Resolution::Literal,
                Resolution::Value,
                Resolution::CharacterDialogueFactory,
                Resolution::CharacterDialogueReconfigure,
            ],
        ),
        contextual_case(
            "dialogue line reference",
            line_reference("first"),
            line_reference("second"),
            Standard,
            &[EntityReference, Path, Match],
            &[
                Resolution::Structural,
                Resolution::Value,
                Resolution::DialogueLineReference,
            ],
        ),
        contextual_case(
            "registered StageLook in factory patch",
            stage_look("ja-JP"),
            stage_look("en-US"),
            CharacterNominal,
            &[Literal, Path, ShortVariant, Call, Block, Match],
            &[
                Resolution::Structural,
                Resolution::Literal,
                Resolution::Value,
                Resolution::StageLook,
                Resolution::CharacterDialogueFactory,
            ],
        ),
        contextual_case(
            "Agent field",
            agent_field("state_hash"),
            agent_field("render_hash"),
            RegisteredObservation,
            &[Literal, Path, Select, Match],
            &[
                Resolution::Structural,
                Resolution::Literal,
                Resolution::Value,
                Resolution::Select,
            ],
        ),
        contextual_case(
            "Object nominal discriminator",
            object("KeywordHit", "hotspot"),
            object("AlternateHit", "hotspot"),
            Standard,
            OBJECT_SHAPES,
            OBJECT_RESOLUTIONS,
        ),
        contextual_case(
            "Object constant public ID",
            object("KeywordHit", "hotspot"),
            object("KeywordHit", "alternate"),
            Standard,
            OBJECT_SHAPES,
            OBJECT_RESOLUTIONS,
        ),
        contextual_case(
            "closed Fx enum",
            closed_enum("1.0"),
            closed_enum("2.0"),
            Standard,
            &[
                Literal,
                Path,
                ShortVariant,
                Call,
                Block,
                Match,
                ExprShapeFamily::AttachedContentApplication,
            ],
            &[
                Resolution::Structural,
                Resolution::Literal,
                Resolution::Value,
                Resolution::CompileTimeEnum,
                Resolution::Call,
                Resolution::CompileTimeScalar,
                Resolution::DialogueApplication,
                Resolution::ContentApplication,
            ],
        ),
    ]
}

pub(super) fn contextual_expression_corpus_rows() -> Vec<ExpressionCorpusRow> {
    contextual_expression_corpus_cases()
        .into_iter()
        .map(|case| case.row)
        .collect()
}

#[test]
fn lifetime_path_below_match_rejects_before_publication() {
    let source = bool_match_i64_source("'line.focus?");
    let world = super::super::fixture(&source, None);
    let project = world
        .project
        .analysis_view()
        .expect("lifetime-path HIR is executable");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owners = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::LifetimePath(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    assert_eq!(owners.len(), 1);
    let match_expression = module
        .expressions()
        .find_map(|(_, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(expression)
        })
        .expect("Match expression");
    assert!(
        match_expression
            .kind()
            .direct_expression_children()
            .contains(&owners[0])
    );
    let error = super::super::analyze(&world)
        .expect_err("the sole LifetimePath checker rejects this value family");
    assert!(
        matches!(
            error,
            FinalSemanticAnalysisError::ExpressionTypeUnavailable { .. }
        ),
        "{error:?}"
    );
}

#[test]
fn style_value_fact_is_outside_match_descendants() {
    // StyleValue is issued only at direct Style property roots recorded by
    // collect_style_body_value_kinds. A nested call does not inherit that map.
    let source = format!(
        "pub style theme {{\n .sample {{\n color = rgba(247, 232, 255, 255)\n }}\n}}\n{}",
        bool_match_i64_source("1i64"),
    );
    let world = super::super::fixture(&source, None);
    let report = super::super::analyze(&world).expect("Style root and Match body are accepted");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let match_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match body");
    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let match_path = coordinates
        .expression(match_owner)
        .expect("accepted Match path");
    let styles = report
        .expressions()
        .filter_map(|(owner, checked)| {
            matches!(
                checked.resolution(),
                CheckedExpressionResolution::StyleValue(_)
            )
            .then_some(owner)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        styles.len(),
        1,
        "direct Color property has a checked StyleValue"
    );
    for owner in styles {
        assert!(
            !coordinates
                .expression(owner)
                .expect("Style value path")
                .is_at_or_below(&match_path)
        );
    }
    let observed = accepted_match_expression_corpus_observation(&source);
    assert!(
        !observed
            .resolutions
            .contains(&ExpressionResolutionFamily::StyleValue)
    );
}

#[test]
fn checked_match_contextual_expression_corpus_retains_families_and_meaning() {
    for case in contextual_expression_corpus_cases() {
        let row = case.row;
        let first = accepted_match_expression_corpus_observation_for_fixture(
            &row.fixture.build(&row.source),
        );
        let second = accepted_match_expression_corpus_observation_for_fixture(
            &row.fixture.build(&case.changed),
        );
        assert_eq!(
            first.shapes,
            row.shapes.iter().copied().collect(),
            "{} exact shapes",
            row.name
        );
        assert_eq!(
            first.resolutions,
            row.resolutions.iter().copied().collect(),
            "{} exact resolutions",
            row.name
        );
        assert_eq!(first.shapes, second.shapes, "{} retains shape", row.name);
        assert_eq!(
            first.resolutions, second.resolutions,
            "{} retains resolution",
            row.name
        );
        assert_eq!(
            first
                .match_type
                .as_ref()
                .map(|ty| ty.semantic_identity_digest().expect("closed Match type")),
            second
                .match_type
                .as_ref()
                .map(|ty| ty.semantic_identity_digest().expect("closed Match type")),
            "{} retains value type",
            row.name
        );
        assert_ne!(
            first.semantic_digest, second.semantic_digest,
            "{} changes meaning",
            row.name
        );
        let revised = format!(
            "fn unrelated() -> i64 {{ 99i64 }}\n{}",
            row.source.replace("true =>", "true  =>  ")
        );
        let first_world = row.fixture.build(&row.source);
        let revised_world = row.fixture.build(&revised);
        let original_match = outermost(&match_observations_for_fixture(&first_world));
        let revised_match = outermost(&match_observations_for_fixture(&revised_world));
        assert_ne!(
            original_match.owner, revised_match.owner,
            "{} changes HIR ID",
            row.name
        );
        assert_ne!(
            original_match.source_start, revised_match.source_start,
            "{} changes span",
            row.name
        );
        assert_eq!(
            original_match.digest, revised_match.digest,
            "{} preserves meaning",
            row.name
        );
    }
}
