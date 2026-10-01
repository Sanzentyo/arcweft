use crate::{
    checked_rich_text::CheckedDisplayWitness,
    final_analysis::{
        CheckedLocalReadMode, CheckedLocalUseError, FinalSemanticAnalysisError,
        analyzer::display::DisplayConformanceRejection,
    },
    types::TypeKind,
};

use super::{analyze, fixture};

fn source(method: &str, extra: &str) -> String {
    format!(
        r#"
struct RouteInfo {{ label: String }}
impl DisplayText for RouteInfo {{
    {method}
}}
fn render(value: RouteInfo) -> Content {{ fmt(value) }}
{extra}
"#
    )
}

const VALID_METHOD: &str = "fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> { Ok(fmt(self.label)) }";

#[test]
fn project_display_text_seals_exact_method_and_closed_type() {
    let fixture = fixture(&source(VALID_METHOD, ""), None);
    let report = analyze(&fixture).expect("valid standard DisplayText implementation");
    let conformance = report
        .calls()
        .filter_map(|(_, call)| {
            call.selected_application()?
                .format_call()?
                .witness()
                .project_conformance()
        })
        .next()
        .expect("fmt(RouteInfo) selects the project method");
    assert!(matches!(conformance.target(), TypeKind::ProjectNominal(_)));
    assert_eq!(conformance.method_ordinal(), 0);
    assert_eq!(
        conformance.method_declaration().method().as_str(),
        "display_text"
    );
    assert!(conformance.type_arguments().is_empty());
    assert!(matches!(
        report
            .display_witness_for_fmt_type(conformance.target())
            .expect("closed lookup"),
        Some(CheckedDisplayWitness::Project(_))
    ));
}

#[test]
fn display_context_and_error_fields_use_standard_record_identity() {
    let cases = [
        (
            "locale",
            "fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> { let locale = ctx.locale; Ok(fmt(locale)) }",
            "",
        ),
        (
            "style",
            "fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> { let selected_style = ctx.style; Ok(fmt(self.label)) }",
            "",
        ),
        (
            "currency",
            "fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> { let currency = ctx.currency; Ok(fmt(self.label)) }",
            "",
        ),
        (
            "error",
            VALID_METHOD,
            "fn error_message(value: DisplayError) -> String { value.message }",
        ),
    ];
    let mut failures = Vec::new();
    for (label, method, extra) in cases {
        let source = source(method, extra);
        let (_, parsed) = super::parse(
            "arcweft-test://sema/display-context",
            "display-context.arcw",
            &source,
        );
        if !parsed.diagnostics().is_empty() {
            failures.push(format!("{label} parser: {:#?}", parsed.diagnostics()));
            continue;
        }
        let fixture = fixture(&source, None);
        if let Err(error) = analyze(&fixture) {
            failures.push(format!("{label}: {error:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn pure_nested_closure_call_is_valid_display_method() {
    let fixture = fixture(
        &source(
            "fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> { let label = self.label; let render = || fmt(label); Ok(render()) }",
            "",
        ),
        None,
    );
    analyze(&fixture).expect("pure nested closure call retains an empty DisplayText effect row");
}

#[test]
fn effectful_nested_closure_call_is_rejected_by_display_method() {
    let fixture = fixture(
        &source(
            "fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> { let render = || effectful(); Ok(render()) }",
            "fn effectful() -> Content effects { fs.read } { fmt(\"effect\") }",
        ),
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::InvalidDisplayTextImpl {
            reason: DisplayConformanceRejection::Effects,
            ..
        })
    ));
}

#[test]
fn display_text_rejects_wrong_signature_and_duplicate_impl() {
    for (method, reason) in [
        (
            "fn wrong(self, ctx: DisplayContext) -> Result<Content, DisplayError> { Ok(fmt(self.label)) }",
            DisplayConformanceRejection::MethodInventory,
        ),
        (
            "fn display_text(self, ctx: String) -> Result<Content, DisplayError> { Ok(fmt(self.label)) }",
            DisplayConformanceRejection::Parameters,
        ),
        (
            "fn display_text(self, ctx: DisplayContext) -> Content { fmt(self.label) }",
            DisplayConformanceRejection::Result,
        ),
    ] {
        let fixture = fixture(&source(method, ""), None);
        assert!(
            matches!(
                analyze(&fixture),
                Err(FinalSemanticAnalysisError::InvalidDisplayTextImpl { reason: actual, .. }) if actual == reason
            ),
            "wrong signature must be rejected: {method}"
        );
    }
    let duplicate = source(
        VALID_METHOD,
        &format!("impl DisplayText for RouteInfo {{ {VALID_METHOD} }}"),
    );
    let fixture = fixture(&duplicate, None);
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::DuplicateDisplayTextImpl { .. })
    ));
}

#[test]
fn project_homonym_cannot_satisfy_standard_display_text() {
    let text = format!(
        "pub trait DisplayText {{ fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> }}\n{}",
        source(VALID_METHOD, ""),
    );
    let fixture = fixture(&text, None);
    let actual = analyze(&fixture).err();
    assert!(
        matches!(
            &actual,
            Some(FinalSemanticAnalysisError::InvalidDisplayTextImpl {
                reason: DisplayConformanceRejection::TraitIdentity,
                ..
            })
        ),
        "project homonym analysis result: {actual:?}"
    );
}

#[test]
fn unrelated_module_homonym_does_not_hide_standard_display_text() {
    let fixture = fixture(
        &source(VALID_METHOD, ""),
        Some(
            "pub trait DisplayText { fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> }",
        ),
    );
    analyze(&fixture).expect("unimported child trait does not bind the root impl path");
}

#[test]
fn direct_interpolation_rejects_suspending_source() {
    let fixture = fixture(
        r#"
pub character alice { display = "Alice" }
fn speak(need: Need<i64>) {
    alice[#[await need]];
}
"#,
        None,
    );
    let actual = analyze(&fixture).err();
    assert!(
        matches!(
            &actual,
            Some(FinalSemanticAnalysisError::ImpureDialogueInterpolation { .. })
        ),
        "suspending interpolation must be rejected: {actual:?}"
    );
}

#[test]
fn project_display_text_rejects_nominal_with_affine_handle_field() {
    let fixture = fixture(
        r#"
struct RouteInfo { label: String, voice: VoiceHandle }
impl DisplayText for RouteInfo {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> {
        Ok(fmt(self.label))
    }
}
pub character alice { display = "Alice" }
fn speak(value: RouteInfo) { alice[#[fmt(value)]]; }
"#,
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::NominalSchemaProjection(
            super::super::NominalSchemaProjectionError::UnsupportedLeaf { ty, .. }
        )) if *ty == TypeKind::VoiceHandle
    ));
}

#[test]
fn content_with_affine_effect_capture_reaches_fmt_operand() {
    let fixture = fixture(
        r##"
pub character alice {}
fn callback(voice: VoiceHandle) {}
fn emphasis() -> Color { rgb("#a8b5ff") }
fn format_body()[body: DialogueContent] -> DialogueContent { fmt(body, color=emphasis()) }
fn speak(voice: VoiceHandle) {
    alice[#format_body()[hello [call callback(voice)]]];
}
"##,
        None,
    );
    let report = analyze(&fixture).expect("effect-capturing Content passes through fmt");
    assert!(
        report
            .expressions()
            .any(|(_, expression)| matches!(expression.value_type(), Some(TypeKind::VoiceHandle)))
    );
    assert!(
        report
            .checked_local_uses()
            .value_transfers()
            .any(|(_, row)| {
                report
                    .local(row.local())
                    .is_some_and(|binding| binding.ty() == &TypeKind::VoiceHandle)
                    && row.mode() == CheckedLocalReadMode::Move
            })
    );
}

#[test]
fn affine_content_capture_cannot_be_used_again_after_formatter_construction() {
    let fixture = fixture(
        r##"
pub character alice {}
fn callback(voice: VoiceHandle) {}
fn emphasis() -> Color { rgb("#a8b5ff") }
fn format_body()[body: DialogueContent] -> DialogueContent { fmt(body, color=emphasis()) }
fn speak(voice: VoiceHandle) {
    alice[#format_body()[hello [call callback(voice)]]];
    callback(voice);
}
"##,
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
        "second affine use must be rejected: {actual:?}"
    );
}
