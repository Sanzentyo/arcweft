use std::collections::BTreeSet;

use crate::{
    callable::CheckedCallSemanticOperandSource,
    checked_rich_text::{
        CheckedAttachedContentArgument, CheckedContentEmission, CheckedDialogueToken,
        CheckedRichTextReport, LengthUnit,
    },
    final_analysis::{
        CheckedCompileTimeScalar, CheckedCompileTimeScalarKind, CheckedExpressionResolution,
        CheckedTextProxyApplication, CheckedTextProxyValueOrigin, FinalSemanticAnalysis,
        FinalSemanticAnalysisError, TextProxyDeclarationDiagnosticCause,
        TextProxyDefaultExpectation,
    },
};

use super::{analyze, fixture};

fn object_applications(report: &FinalSemanticAnalysis) -> Vec<&CheckedTextProxyApplication> {
    fn visit<'a>(
        report: &'a CheckedRichTextReport,
        applications: &mut Vec<&'a CheckedTextProxyApplication>,
    ) {
        for token in report.content().tokens() {
            let CheckedDialogueToken::ContentInsert(insertion) = token else {
                continue;
            };
            if let CheckedContentEmission::ObjectSpan(application) = insertion.emission() {
                applications.push(application);
            }
            if let CheckedAttachedContentArgument::Present {
                checked_content, ..
            } = insertion.argument()
            {
                visit(checked_content, applications);
            }
        }
    }

    let mut applications = Vec::new();
    for (_, expression) in report.expressions() {
        if let CheckedExpressionResolution::DialogueApplication { rich_text, .. } =
            expression.resolution()
        {
            visit(rich_text, &mut applications);
        }
    }
    applications
}

fn object_application(report: &FinalSemanticAnalysis) -> &CheckedTextProxyApplication {
    let applications = object_applications(report);
    if applications.len() != 1 {
        panic!("expected exactly one generic Object application");
    }
    applications
        .into_iter()
        .next()
        .expect("the length was checked above")
}

#[test]
fn generic_object_scalar_schema_preserves_declaration_order_and_origins() {
    let fixture = fixture(
        r##"
pub enum Tone {
    Calm,
    Bright,
}

#[text_proxy(
    role = "keyword",
    layer = "foreground",
    depth = 2.5px,
    hit_test = true,
    bool_value = true,
    int_value = -7i64,
    milli_value = 1.25,
    ratio_value = 0.75,
    length_value = 3.5px,
    angle_value = 90deg,
    duration_value = 2s,
    public_id_value = "tag",
    text_value = "hello",
    color_value = rgb("#102030"),
    tone_value = .Bright
)]
pub struct ScalarProxy {
    bool_value: bool
    int_value: i64
    milli_value: Milli
    ratio_value: Ratio
    length_value: Length
    angle_value: Angle
    duration_value: Duration
    public_id_value: PublicId
    text_value: String
    color_value: Color
    tone_value: Tone
    absent_value: Option<String>
}

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = ScalarProxy, bool_value = false, tone_value = .Calm)[scalar]]
    return "done"
}
"##,
        None,
    );
    let report = analyze(&fixture).expect("typed scalar object is accepted");
    assert!(
        report.diagnostics().is_empty(),
        "{:#?}",
        report.diagnostics()
    );

    let application = object_application(&report);
    let metadata = application.metadata();
    assert_eq!(
        metadata.role().map(|value| value.value().as_str()),
        Some("keyword")
    );
    assert_eq!(
        metadata.layer().map(|value| value.value().as_str()),
        Some("foreground")
    );
    assert_eq!(
        metadata.depth().map(|value| value.value().milli()),
        Some(2_500)
    );
    assert!(metadata.hit_test().value());
    assert!(
        metadata
            .role()
            .is_some_and(|value| value.origin().coordinate().is_some())
    );
    assert!(
        metadata
            .layer()
            .is_some_and(|value| value.origin().coordinate().is_some())
    );
    assert!(
        metadata
            .depth()
            .is_some_and(|value| value.origin().coordinate().is_some())
    );
    assert!(metadata.hit_test().origin().coordinate().is_some());

    let [
        bool_value,
        int_value,
        milli_value,
        ratio_value,
        length_value,
        angle_value,
        duration_value,
        public_id_value,
        text_value,
        color_value,
        tone_value,
        absent_value,
    ] = application.fields()
    else {
        panic!("declaration-order text-proxy fields");
    };
    for field in &application.fields()[..11] {
        assert!(field.origin().coordinate().is_some());
    }
    assert!(absent_value.origin().coordinate().is_none());
    assert!(matches!(
        bool_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::Inline { .. })
            && matches!(field.value(), Some(CheckedCompileTimeScalar::Bool(false)))
    ));
    assert!(matches!(
        int_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::AttributeDefault(_))
            && matches!(field.value(), Some(CheckedCompileTimeScalar::Int(-7)))
    ));
    assert!(matches!(
        milli_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::AttributeDefault(_))
            && matches!(field.value(), Some(CheckedCompileTimeScalar::Milli(value)) if value.0 == 1_250)
    ));
    assert!(matches!(
        ratio_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::AttributeDefault(_))
            && matches!(field.value(), Some(CheckedCompileTimeScalar::Ratio(value)) if value.0 == 750)
    ));
    assert!(matches!(
        length_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::AttributeDefault(_))
            && matches!(
                field.value(),
                Some(CheckedCompileTimeScalar::Length(value))
                    if value.milli == 3_500 && value.unit == LengthUnit::Px
            )
    ));
    assert!(matches!(
        angle_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::AttributeDefault(_))
            && matches!(
                field.value(),
                Some(CheckedCompileTimeScalar::Angle(value)) if value.milli_degrees == 90_000
            )
    ));
    assert!(matches!(
        duration_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::AttributeDefault(_))
            && matches!(
                field.value(),
                Some(CheckedCompileTimeScalar::Duration(value)) if value.millis == 2_000
            )
    ));
    assert!(matches!(
        public_id_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::AttributeDefault(_))
            && matches!(
                field.value(),
                Some(CheckedCompileTimeScalar::PublicId(value)) if value.as_str() == "tag"
            )
    ));
    assert!(matches!(
        text_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::AttributeDefault(_))
            && matches!(field.value(), Some(CheckedCompileTimeScalar::Text(value)) if value == "hello")
    ));
    assert!(matches!(
        color_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::AttributeDefault(_))
            && matches!(field.value(), Some(CheckedCompileTimeScalar::Color(_)))
    ));
    assert!(matches!(
        tone_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::Inline { .. })
            && matches!(field.value(), Some(CheckedCompileTimeScalar::Enum(value)) if value.ordinal() == 0)
    ));
    assert!(matches!(
        absent_value,
        field if matches!(field.origin(), CheckedTextProxyValueOrigin::Absent)
            && field.value().is_none()
    ));
}

#[test]
fn generic_object_discriminator_and_arguments_are_evaluated_once() {
    let fixture = fixture(
        r#"
#[text_proxy]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit, channel = "once")[once]]
    return "done"
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("typed object is accepted");
    assert!(
        !report.expressions().any(|(_, expression)| matches!(
            expression.resolution(),
            CheckedExpressionResolution::CompileTimeCallee(_)
        )),
        "the static Content namespace is not an expression fact"
    );
    let owner = report
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(
                expression.resolution(),
                CheckedExpressionResolution::ContentApplication(_)
            )
            .then_some(owner)
        })
        .expect("object content application expression");
    let (_, call) = report
        .calls()
        .find(|(call_owner, _)| *call_owner == owner)
        .expect("object call fact");
    let selected = call
        .selected_application()
        .expect("selected object call application");
    let [semantic] = selected.core().execution().semantic_operands() else {
        panic!("one exact Object type discriminator");
    };
    assert!(matches!(
        semantic.source(),
        CheckedCallSemanticOperandSource::TextProxyObject { argument, .. }
            if argument.get() == 1
    ));

    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == owner)
        .collect::<Vec<_>>();
    let mut arguments = BTreeSet::new();
    for evaluation in &physical {
        assert!(
            arguments.insert(evaluation.argument().get()),
            "one physical evaluation per authored Object argument"
        );
    }
    assert_eq!(arguments.len(), 2, "id and channel are each visited once");
    assert!(arguments.contains(&0));
    assert!(arguments.contains(&2));
}

#[test]
fn generic_object_rejects_invalid_scalar_values() {
    let length = fixture(
        r#"
#[text_proxy]
pub struct LengthProxy { value: Length }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = LengthProxy, value = 2deg)[bad]]
    return "done"
}
"#,
        None,
    );
    assert!(analyze(&length).is_err());

    let ratio = fixture(
        r#"
#[text_proxy]
pub struct RatioProxy { value: Ratio }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = RatioProxy, value = 1.001)[bad]]
    return "done"
}
"#,
        None,
    );
    assert!(analyze(&ratio).is_err());
}

#[test]
fn declaration_defaults_reject_effectful_and_path_expressions() {
    let effectful = fixture(
        r#"
fn read_value() -> i64 effects { fs.read } { 7i64 }

#[text_proxy(value = read_value())]
pub struct EffectfulProxy { value: i64 }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = EffectfulProxy)[bad]]
    return "done"
}
"#,
        None,
    );
    let Err(FinalSemanticAnalysisError::InvalidTextProxyDeclarations { diagnostics }) =
        analyze(&effectful)
    else {
        panic!("expected typed effectful-default diagnostics");
    };
    let [diagnostic] = diagnostics.as_ref() else {
        panic!("one effectful-default diagnostic");
    };
    assert_eq!(diagnostic.name(), Some("value"));
    assert_eq!(
        diagnostic.expected(),
        &TextProxyDefaultExpectation::Scalar(CheckedCompileTimeScalarKind::Int)
    );
    assert_eq!(
        diagnostic.cause(),
        &TextProxyDeclarationDiagnosticCause::Expression
    );

    let path = fixture(
        r#"
#[text_proxy(value = choice)]
pub struct PathProxy { value: String }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = PathProxy)[bad]]
    return "done"
}
"#,
        None,
    );
    let Err(FinalSemanticAnalysisError::InvalidTextProxyDeclarations { diagnostics }) =
        analyze(&path)
    else {
        panic!("expected typed path-default diagnostics");
    };
    let [diagnostic] = diagnostics.as_ref() else {
        panic!("one path-default diagnostic");
    };
    assert_eq!(diagnostic.name(), Some("value"));
    assert_eq!(
        diagnostic.expected(),
        &TextProxyDefaultExpectation::Scalar(CheckedCompileTimeScalarKind::Text)
    );
    assert_eq!(
        diagnostic.cause(),
        &TextProxyDeclarationDiagnosticCause::Expression
    );
}

#[test]
fn payload_enum_fields_are_rejected_instead_of_becoming_open_objects() {
    let fixture = fixture(
        r#"
pub enum PayloadTone {
    Calm,
    Bright(i64),
}

#[text_proxy]
pub struct PayloadProxy { tone: PayloadTone }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = PayloadProxy, tone = .Calm)[bad]]
    return "done"
}
"#,
        None,
    );
    assert!(analyze(&fixture).is_err());
}

#[test]
fn malformed_object_metadata_is_rejected_before_object_insertion() {
    let fixture = fixture(
        r#"
#[text_proxy]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit, depth = 2em)[invalid]]
    return "done"
}
"#,
        None,
    );
    assert!(
        fixture.project.analysis_view().is_err(),
        "invalid Object metadata unit must be rejected before semantic analysis"
    );
}
