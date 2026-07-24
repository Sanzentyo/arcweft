use crate::{
    callable::CallableFamily,
    env::{FunctionParam, FunctionSignature, TypeCheckEnv},
    types::TypeKind,
};

use super::{FamilyCase, SignatureFixture, assert_family_case, fixture_with_environment};

#[test]
fn closed_language_free_families_use_one_shared_checker_and_signature_path() {
    let cases = [
        (
            r"
fn main() -> Unit {
    Fx.color()
    ()
}
",
            FamilyCase::accepted("Fx.color()", ")", CallableFamily::Fx, 0),
        ),
        (
            r"
fn main() -> Unit {
    Fx.color(1i64)
    ()
}
",
            FamilyCase::rejected("Fx.color(1i64)", "1i64", CallableFamily::Fx, 1),
        ),
        (
            r"
fn main() -> Unit {
    sin(1.0f32)
    ()
}
",
            FamilyCase::accepted("sin(1.0f32)", "1.0f32", CallableFamily::Builtin, 1),
        ),
        (
            r"
fn main() -> Unit {
    sin(true)
    ()
}
",
            FamilyCase::rejected("sin(true)", "true", CallableFamily::Builtin, 1),
        ),
        (
            r"
fn main() -> Unit {
    viewport()
    ()
}
",
            FamilyCase::accepted("viewport()", ")", CallableFamily::Agent, 0),
        ),
        (
            r"
fn main() -> Unit {
    viewport(1i64)
    ()
}
",
            FamilyCase::rejected("viewport(1i64)", "1i64", CallableFamily::Agent, 1),
        ),
        (
            r"
fn main() -> Unit {
    player_viewport()
    ()
}
",
            FamilyCase::accepted("player_viewport()", ")", CallableFamily::Presentation, 0),
        ),
        (
            r"
fn main() -> Unit {
    player_viewport(1i64)
    ()
}
",
            FamilyCase::rejected(
                "player_viewport(1i64)",
                "1i64",
                CallableFamily::Presentation,
                1,
            ),
        ),
    ];

    for (source, case) in cases {
        assert_family_case(&SignatureFixture::new(source), case);
    }
}

#[test]
fn open_recovery_free_families_check_each_argument_once_through_shared_resolution() {
    let promotion_cases = [
        (
            r"
fn main() -> Unit {
    promote(7i64)
    ()
}
",
            FamilyCase::accepted("promote(7i64)", "7i64", CallableFamily::Promotion, 1),
        ),
        (
            r"
fn main() -> Unit {
    promote(missing)
    ()
}
",
            FamilyCase::clean_recovery("promote(missing)", "missing", CallableFamily::Promotion, 1),
        ),
    ];
    for (source, case) in promotion_cases {
        assert_family_case(&SignatureFixture::new(source), case);
    }

    for (source, case) in [
        (
            r"
fn main(speaker_value: Speaker<Character>) -> Unit {
    speaker_value(7i64)
    ()
}
",
            FamilyCase::accepted("speaker_value(7i64)", "7i64", CallableFamily::Speaker, 1),
        ),
        (
            r"
fn main(speaker_value: Speaker<Character>) -> Unit {
    speaker_value(missing)
    ()
}
",
            FamilyCase::clean_recovery(
                "speaker_value(missing)",
                "missing",
                CallableFamily::Speaker,
                1,
            ),
        ),
    ] {
        assert_family_case(&SignatureFixture::new(source), case);
    }
}

#[test]
fn expected_nominal_constructor_families_share_checker_and_signature_identity() {
    let cases = [
        (
            r"
enum Mood {
    WithScore(i64),
}

fn main() -> Unit {
    let mood: Mood = .WithScore(7i64)
    ()
}
",
            FamilyCase::accepted(
                ".WithScore(7i64)",
                "7i64",
                CallableFamily::EnumConstructor,
                1,
            ),
        ),
        (
            r"
enum Mood {
    WithScore(i64),
}

fn main() -> Unit {
    let mood: Mood = .WithScore(true)
    ()
}
",
            FamilyCase::rejected(
                ".WithScore(true)",
                "true",
                CallableFamily::EnumConstructor,
                1,
            ),
        ),
        (
            r"
fn main() -> Unit {
    let value: Result<i64, String> = Ok(7i64)
    ()
}
",
            FamilyCase::accepted("Ok(7i64)", "7i64", CallableFamily::ResultConstructor, 1),
        ),
        (
            r"
fn main() -> Unit {
    let value: Result<i64, String> = Ok(true)
    ()
}
",
            FamilyCase::rejected("Ok(true)", "true", CallableFamily::ResultConstructor, 1),
        ),
        (
            r"
fn main() -> Unit {
    let value: Option<i64> = Some(7i64)
    ()
}
",
            FamilyCase::accepted("Some(7i64)", "7i64", CallableFamily::OptionConstructor, 1),
        ),
        (
            r"
fn main() -> Unit {
    let value: Option<i64> = Some(true)
    ()
}
",
            FamilyCase::rejected("Some(true)", "true", CallableFamily::OptionConstructor, 1),
        ),
    ];

    for (source, case) in cases {
        assert_family_case(&SignatureFixture::new(source), case);
    }
}

#[test]
fn project_lexical_and_function_value_families_share_identity() {
    let project_cases = [
        (
            r"
fn project_value(value: i64) -> i64 {
    value
}

fn main() -> Unit {
    project_value(7i64)
    ()
}
",
            FamilyCase::accepted("project_value(7i64)", "7i64", CallableFamily::Project, 1),
        ),
        (
            r"
fn project_value(value: i64) -> i64 {
    value
}

fn main() -> Unit {
    project_value(true)
    ()
}
",
            FamilyCase::rejected("project_value(true)", "true", CallableFamily::Project, 1),
        ),
        (
            r"
fn project_value(value: i64) -> i64 {
    value
}

fn main() -> Unit {
    let local = project_value
    local(7i64)
    ()
}
",
            FamilyCase::accepted("local(7i64)", "7i64", CallableFamily::Lexical, 1),
        ),
        (
            r"
fn project_value(value: i64) -> i64 {
    value
}

fn main() -> Unit {
    let local = project_value
    local(true)
    ()
}
",
            FamilyCase::rejected("local(true)", "true", CallableFamily::Lexical, 1),
        ),
        (
            r"
fn apply(value: i64, f: i64 -> i64) -> i64 {
    f(value)
}
",
            FamilyCase::accepted("f(value)", "value", CallableFamily::FunctionValue, 1),
        ),
        (
            r"
fn apply(value: bool, f: i64 -> i64) -> i64 {
    f(value)
}
",
            FamilyCase::selected_poisoned("f(value)", "value", CallableFamily::FunctionValue, 1),
        ),
    ];
    for (source, case) in project_cases {
        assert_family_case(&SignatureFixture::new(source), case);
    }
}

#[test]
fn environment_family_shares_checker_and_signature_identity() {
    let publication = super::super::publication(
        "adapter.signature-migration-environment",
        "environment_value",
        [super::super::single_parameter_schema(TypeKind::I64)],
    );
    let accepted = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    environment_value(7i32)
    ()
}
",
        publication,
    );
    assert_family_case(
        &accepted,
        FamilyCase::accepted(
            "environment_value(7i32)",
            "7i32",
            CallableFamily::Environment,
            1,
        ),
    );

    let rejected = fixture_with_environment(
        r"
fn main() -> Unit {
    environment_value(true)
    ()
}
",
        TypeCheckEnv::standard().with_function_signature(
            "environment_value",
            FunctionSignature::new(
                TypeKind::I64,
                [FunctionParam::required("value", TypeKind::I64)],
            ),
        ),
    );
    assert_family_case(
        &rejected,
        FamilyCase::rejected(
            "environment_value(true)",
            "true",
            CallableFamily::Environment,
            1,
        ),
    );
}
