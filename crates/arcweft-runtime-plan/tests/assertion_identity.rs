use arcweft_core::effect::{RuntimeAssertionGuardId, RuntimeAssertionProfile};
use arcweft_lang_hir::symbol::{
    CallableDeclarationId, CallableDeclarationOwner, CallablePackageId,
};
use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};
use arcweft_runtime_plan::assertion_identity::{
    AssertionConditionIndex, derive_runtime_assertion_guard,
};

fn canonical_module(segments: &[&str]) -> CanonicalModulePath {
    CanonicalModulePath::from_segments(
        segments
            .iter()
            .map(|segment| ModuleSegment::new(*segment).unwrap()),
    )
}

fn callable_id(
    package: &CallablePackageId,
    module: &CanonicalModulePath,
    owner: CallableDeclarationOwner,
    owner_path: &[&str],
    name: &str,
) -> CallableDeclarationId {
    CallableDeclarationId::try_new_in_owner_path(
        package.clone(),
        module.clone(),
        owner,
        owner_path
            .iter()
            .map(|segment| ModuleSegment::new(*segment).unwrap()),
        name,
    )
    .unwrap()
}

struct GuardFixture {
    package: CallablePackageId,
    other_package: CallablePackageId,
    module: CanonicalModulePath,
    other_module: CanonicalModulePath,
    callable: CallableDeclarationId,
    other_callable: CallableDeclarationId,
    other_owner_callable: CallableDeclarationId,
    other_owner_path_callable: CallableDeclarationId,
    first: AssertionConditionIndex,
    second: AssertionConditionIndex,
}

impl GuardFixture {
    fn new() -> Self {
        let package = CallablePackageId::try_new("story").unwrap();
        let module = canonical_module(&["chapter", "opening"]);
        let callable = callable_id(
            &package,
            &module,
            CallableDeclarationOwner::Function,
            &["scene"],
            "run",
        );
        Self {
            other_package: CallablePackageId::try_new("story.extra").unwrap(),
            other_module: canonical_module(&["chapter", "ending"]),
            other_callable: callable_id(
                &package,
                &module,
                CallableDeclarationOwner::Function,
                &["scene"],
                "resume",
            ),
            other_owner_callable: callable_id(
                &package,
                &module,
                CallableDeclarationOwner::View,
                &["scene"],
                "run",
            ),
            other_owner_path_callable: callable_id(
                &package,
                &module,
                CallableDeclarationOwner::Function,
                &["chapter", "scene"],
                "run",
            ),
            first: AssertionConditionIndex::try_new(0, 2).unwrap(),
            second: AssertionConditionIndex::try_new(1, 2).unwrap(),
            package,
            module,
            callable,
        }
    }

    fn expected(&self) -> RuntimeAssertionGuardId {
        derive_runtime_assertion_guard(
            &self.package,
            &self.module,
            &self.callable,
            7,
            self.first,
            RuntimeAssertionProfile::Always,
        )
    }

    fn changed_seed_guards(&self) -> [RuntimeAssertionGuardId; 8] {
        [
            derive_runtime_assertion_guard(
                &self.other_package,
                &self.module,
                &self.callable,
                7,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.other_module,
                &self.callable,
                7,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.other_callable,
                7,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.other_owner_callable,
                7,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.other_owner_path_callable,
                7,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.callable,
                8,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.callable,
                7,
                self.second,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.callable,
                7,
                self.first,
                RuntimeAssertionProfile::DebugOnly,
            ),
        ]
    }
}

#[test]
fn guard_derivation_uses_every_typed_seed_field_and_is_deterministic() {
    let fixture = GuardFixture::new();
    let expected = fixture.expected();
    assert_eq!(
        expected.as_bytes(),
        &[
            0x5f, 0x3b, 0x1c, 0xcf, 0xea, 0x6b, 0xac, 0x47, 0x5e, 0xba, 0x86, 0xa0, 0x78, 0xc9,
            0xa8, 0x98,
        ]
    );
    assert_eq!(fixture.expected(), expected);

    let variants = fixture.changed_seed_guards();
    for variant in variants {
        assert_ne!(variant, expected);
    }
}
