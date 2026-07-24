use crate::{callable::CallableFamily, env::TypeCheckEnv, types::TypeKind};

use super::{FamilyCase, SignatureFixture, assert_family_case, fixture_with_environment};

#[test]
fn closed_inherent_method_families_share_checker_and_signature_identity() {
    let cases = [
        (
            r"
fn main() -> Unit {
    let values: Vec<i64> = [1i64]
    values.len()
    ()
}
",
            FamilyCase::accepted("values.len()", ")", CallableFamily::CollectionMethod, 0),
        ),
        (
            r"
fn main() -> Unit {
    let values: Vec<i64> = [1i64]
    values.len(1i64)
    ()
}
",
            FamilyCase::rejected(
                "values.len(1i64)",
                "1i64",
                CallableFamily::CollectionMethod,
                1,
            ),
        ),
        (
            r"
fn main() -> Unit {
    7i64.min(3i64)
    ()
}
",
            FamilyCase::accepted("7i64.min(3i64)", "3i64", CallableFamily::IntegerMethod, 1),
        ),
        (
            r"
fn main() -> Unit {
    7i64.min(true)
    ()
}
",
            FamilyCase::rejected("7i64.min(true)", "true", CallableFamily::IntegerMethod, 1),
        ),
        (
            r"
fn main() -> Unit {
    Vec<i64>.with_capacity(2i64)
    ()
}
",
            FamilyCase::accepted(
                "Vec<i64>.with_capacity(2i64)",
                "2i64",
                CallableFamily::CapacityMethod,
                1,
            ),
        ),
        (
            r"
fn main() -> Unit {
    Vec<i64>.with_capacity(missing)
    ()
}
",
            FamilyCase::clean_recovery(
                "Vec<i64>.with_capacity(missing)",
                "missing",
                CallableFamily::CapacityMethod,
                1,
            ),
        ),
    ];
    for (source, case) in cases {
        assert_family_case(&SignatureFixture::new(source), case);
    }
}

#[test]
fn drop_family_uses_shared_resolution_for_clean_and_recovery_arguments() {
    for (source, case) in [
        (
            r"
fn main() -> Unit {
    7i64.drop()
    ()
}
",
            FamilyCase::accepted("7i64.drop()", ")", CallableFamily::Drop, 0),
        ),
        (
            r"
fn main() -> Unit {
    7i64.drop(missing)
    ()
}
",
            FamilyCase::clean_recovery("7i64.drop(missing)", "missing", CallableFamily::Drop, 1),
        ),
    ] {
        assert_family_case(&SignatureFixture::new(source), case);
    }
}

#[test]
fn environment_seeded_method_families_share_checker_and_signature_identity() {
    let cases = [
        (
            r"
fn main() -> Unit {
    handle.show()
    ()
}
",
            TypeCheckEnv::standard().with_symbol("handle", TypeKind::presentation_handle("View")),
            FamilyCase::accepted(
                "handle.show()",
                ")",
                CallableFamily::PresentationHandleMethod,
                0,
            ),
        ),
        (
            r"
fn main() -> Unit {
    handle.show(1i64)
    ()
}
",
            TypeCheckEnv::standard().with_symbol("handle", TypeKind::presentation_handle("View")),
            FamilyCase::rejected(
                "handle.show(1i64)",
                "1i64",
                CallableFamily::PresentationHandleMethod,
                1,
            ),
        ),
        (
            r"
fn main() -> Unit {
    probe.eq(7i64)
    ()
}
",
            TypeCheckEnv::standard().with_symbol("probe", TypeKind::Probe(Box::new(TypeKind::I64))),
            FamilyCase::accepted("probe.eq(7i64)", "7i64", CallableFamily::DomainMethod, 1),
        ),
        (
            r"
fn main() -> Unit {
    probe.eq(true)
    ()
}
",
            TypeCheckEnv::standard().with_symbol("probe", TypeKind::Probe(Box::new(TypeKind::I64))),
            FamilyCase::rejected("probe.eq(true)", "true", CallableFamily::DomainMethod, 1),
        ),
        (
            r"
fn main() -> Unit {
    stage.acquire(scope)
    ()
}
",
            TypeCheckEnv::standard()
                .with_symbol("stage", TypeKind::Named("StageApi".to_owned()))
                .with_symbol("scope", TypeKind::Named("PresentationLifetime".to_owned())),
            FamilyCase::accepted(
                "stage.acquire(scope)",
                "scope",
                CallableFamily::StageMethod,
                1,
            ),
        ),
        (
            r"
fn main() -> Unit {
    stage.acquire(true)
    ()
}
",
            TypeCheckEnv::standard().with_symbol("stage", TypeKind::Named("StageApi".to_owned())),
            FamilyCase::rejected(
                "stage.acquire(true)",
                "true",
                CallableFamily::StageMethod,
                1,
            ),
        ),
    ];
    for (source, environment, case) in cases {
        assert_family_case(&fixture_with_environment(source, environment), case);
    }
}

#[test]
fn trait_and_data_last_families_share_checker_and_signature_identity() {
    let trait_cases = [
        (
            r#"
struct Score {}

trait Threshold {
    fn above(self, min: i64) -> String
}

impl Threshold for Score {
    fn above(self, min: i64) -> String {
        "trait"
    }
}

fn main(score: Score) -> Unit {
    score.above(7i64)
    ()
}
"#,
            FamilyCase::accepted("score.above(7i64)", "7i64", CallableFamily::TraitMethod, 1),
        ),
        (
            r#"
struct Score {}

trait Threshold {
    fn above(self, min: i64) -> String
}

impl Threshold for Score {
    fn above(self, min: i64) -> String {
        "trait"
    }
}

fn main(score: Score) -> Unit {
    score.above(true)
    ()
}
"#,
            FamilyCase::rejected("score.above(true)", "true", CallableFamily::TraitMethod, 1),
        ),
        (
            r"
fn above(min: i64, value: i64) -> bool {
    value > min
}

fn main() -> Unit {
    let score: i64 = 9i64
    score.above(7i64)
    ()
}
",
            FamilyCase::accepted("score.above(7i64)", "7i64", CallableFamily::DataLast, 1),
        ),
        (
            r"
fn above(min: i64, value: i64) -> bool {
    value > min
}

fn main() -> Unit {
    let score: i64 = 9i64
    score.above(true)
    ()
}
",
            FamilyCase::rejected("score.above(true)", "true", CallableFamily::DataLast, 1),
        ),
    ];
    for (source, case) in trait_cases {
        assert_family_case(&SignatureFixture::new(source), case);
    }
}
