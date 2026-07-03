use crate::{effect_diagnostics::EffectDiagnostic, types::TypeKind};
use arcweft_source::{Diagnostic, DiagnosticSeverity};
use thiserror::Error;

/// Semantic type-checking diagnostic.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckError {
    message: String,
    kind: TypeCheckErrorKind,
}

/// Machine-readable type-checking diagnostic family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeCheckErrorKind {
    /// General diagnostic while older checker paths are being structured.
    Message,
    /// A function or method argument did not match the declared parameter type.
    ArgumentTypeMismatch {
        function: String,
        argument: String,
        expected: TypeKind,
        actual: TypeKind,
    },
    /// An assignment target is not an executable lvalue in the current source grammar.
    UnsupportedAssignmentTarget { target: String, reason: String },
    /// An `extern rust mod` declaration references a package without loaded ABI metadata.
    MissingRustPackageMetadata { package: String },
    /// An `extern rust mod` member is not present in loaded ABI metadata.
    MissingRustExport { package: String, export: String },
    /// An `extern rust mod` function signature differs from loaded ABI metadata.
    RustExportSignatureMismatch {
        package: String,
        export: String,
        expected: String,
        actual: String,
    },
    /// An inline dialogue function call omitted explicit error handling.
    InlineCallErrorPolicyMissing { function: String },
    /// An inline dialogue function call declared more than one failure policy.
    InlineFailurePolicyConflict { function: String },
    /// An inline dialogue function call declared an unknown failure policy.
    UnknownInlineFailurePolicy { function: String, policy: String },
    /// A structured error or warning from transitive effect analysis.
    Effect { diagnostic: EffectDiagnostic },
    /// A structured trait / impl / associated-type diagnostic.
    Trait { diagnostic: TraitDiagnostic },
}

/// Semantic diagnostic emitted by the trait catalog and coherence substrate.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TraitDiagnostic {
    message: String,
    kind: TraitDiagnosticKind,
}

/// Machine-readable trait diagnostic family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraitDiagnosticKind {
    DuplicateTrait {
        name: String,
    },
    UnknownTrait {
        name: String,
    },
    DuplicateAssociatedType {
        trait_name: String,
        assoc: String,
    },
    DuplicateAssociatedTypeAssignment {
        trait_name: String,
        assoc: String,
    },
    UnknownAssociatedType {
        trait_name: String,
        assoc: String,
    },
    MissingAssociatedType {
        trait_name: String,
        target: String,
        assoc: String,
    },
    MissingRequiredMethod {
        trait_name: String,
        target: String,
        method: String,
    },
    MissingRequiredMethodBody {
        trait_name: String,
        target: String,
        method: String,
    },
    ImplMethodSignatureMismatch {
        trait_name: String,
        method: String,
    },
    DuplicateMethod {
        trait_name: String,
        method: String,
    },
    UnknownTraitMethod {
        trait_name: String,
        method: String,
    },
    DuplicateImpl {
        trait_name: String,
        target: String,
    },
    OverlappingImpl {
        trait_name: String,
        existing: String,
        candidate: String,
    },
    OrphanImpl {
        trait_name: String,
        target: String,
    },
    PubImplUnsupported {
        impl_head: String,
    },
    AssociatedTypeDefaultUnsupported {
        trait_name: String,
        assoc: String,
    },
    AssociatedTypeConstructorUnsupported {
        trait_name: String,
        assoc: String,
    },
    TraitDefaultMethodUnsupported {
        trait_name: String,
        method: String,
    },
    AssociatedTypeInInherentImpl {
        assoc: String,
    },
    AmbiguousProjection {
        subject: String,
        assoc: String,
    },
    AmbiguousMethod {
        method: String,
        traits: Vec<String>,
    },
    RawTraitMember {
        trait_name: String,
        raw: String,
    },
    RawImplMember {
        impl_head: String,
        raw: String,
    },
}

/// Non-fatal semantic lint emitted by type analysis.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckWarning {
    message: String,
    kind: TypeCheckWarningKind,
}

/// Machine-readable type-checking warning family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeCheckWarningKind {
    /// A public signature exposes an anonymous sum type that should be nominal.
    PublicAbiAnonymousSum {
        /// Public signature position that exposes the anonymous sum.
        context: String,
        /// Source-level type expression for the anonymous sum.
        type_ref: String,
    },
    /// A structured warning from transitive effect analysis.
    Effect { diagnostic: EffectDiagnostic },
}

/// Syntax-to-HIR readiness error for the future type checker.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckReadinessError {
    message: String,
}

impl TypeCheckReadinessError {
    pub(crate) fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable readiness failure.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::new(DiagnosticSeverity::Error, self.message.clone()).with_code("sema.readiness")
    }
}

impl TypeCheckError {
    pub(crate) fn new(message: String) -> Self {
        Self {
            message,
            kind: TypeCheckErrorKind::Message,
        }
    }

    pub(crate) fn argument_type_mismatch(
        function: impl Into<String>,
        argument: impl Into<String>,
        expected: TypeKind,
        actual: TypeKind,
    ) -> Self {
        let function = function.into();
        let argument = argument.into();
        let message = format!(
            "function `{function}` argument `{argument}` must have type {expected:?}, found {actual:?}"
        );
        Self {
            message,
            kind: TypeCheckErrorKind::ArgumentTypeMismatch {
                function,
                argument,
                expected,
                actual,
            },
        }
    }

    pub(crate) fn unsupported_assignment_target(
        target: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let target = target.into();
        let reason = reason.into();
        Self {
            message: format!("unsupported assignment target `{target}`: {reason}"),
            kind: TypeCheckErrorKind::UnsupportedAssignmentTarget { target, reason },
        }
    }

    pub(crate) fn inline_call_error_policy_missing(function: impl Into<String>) -> Self {
        let function = function.into();
        Self {
            message: format!(
                "inline dialogue call `{function}` must declare `on_error`, `fallback`, or `discard_error`"
            ),
            kind: TypeCheckErrorKind::InlineCallErrorPolicyMissing { function },
        }
    }

    pub(crate) fn inline_failure_policy_conflict(function: impl Into<String>) -> Self {
        let function = function.into();
        Self {
            message: format!(
                "inline dialogue call `{function}` declares multiple failure policies; use exactly one of `on_error`, `fallback`, or `discard_error`"
            ),
            kind: TypeCheckErrorKind::InlineFailurePolicyConflict { function },
        }
    }

    pub(crate) fn unknown_inline_failure_policy(
        function: impl Into<String>,
        policy: impl Into<String>,
    ) -> Self {
        let function = function.into();
        let policy = policy.into();
        Self {
            message: format!(
                "inline dialogue call `{function}` uses unknown inline failure policy `{policy}`"
            ),
            kind: TypeCheckErrorKind::UnknownInlineFailurePolicy { function, policy },
        }
    }

    pub(crate) fn effect(diagnostic: EffectDiagnostic) -> Self {
        Self {
            message: diagnostic.message().to_owned(),
            kind: TypeCheckErrorKind::Effect { diagnostic },
        }
    }

    pub(crate) fn trait_diagnostic(diagnostic: TraitDiagnostic) -> Self {
        Self {
            message: diagnostic.message().to_owned(),
            kind: TypeCheckErrorKind::Trait { diagnostic },
        }
    }

    pub(crate) fn missing_rust_package_metadata(package: impl Into<String>) -> Self {
        let package = package.into();
        Self {
            message: format!(
                "extern rust module imports crate `{package}`, but no Rust ABI metadata for that crate was loaded"
            ),
            kind: TypeCheckErrorKind::MissingRustPackageMetadata { package },
        }
    }

    pub(crate) fn missing_rust_export(
        package: impl Into<String>,
        export: impl Into<String>,
    ) -> Self {
        let package = package.into();
        let export = export.into();
        Self {
            message: format!(
                "extern rust module imports `{export}` from crate `{package}`, but the export is missing from loaded Rust ABI metadata"
            ),
            kind: TypeCheckErrorKind::MissingRustExport { package, export },
        }
    }

    pub(crate) fn rust_export_signature_mismatch(
        package: impl Into<String>,
        export: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        let package = package.into();
        let export = export.into();
        let expected = expected.into();
        let actual = actual.into();
        Self {
            message: format!(
                "extern rust export `{export}` from crate `{package}` has signature `{actual}`, but Arcweft declared `{expected}`"
            ),
            kind: TypeCheckErrorKind::RustExportSignatureMismatch {
                package,
                export,
                expected,
                actual,
            },
        }
    }

    /// Human-readable type-checking failure.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Machine-readable diagnostic family and structured fields.
    pub const fn kind(&self) -> &TypeCheckErrorKind {
        &self.kind
    }

    /// Stable compiler-wide diagnostic code.
    pub fn stable_code(&self) -> String {
        typecheck_error_code(&self.kind)
    }

    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::new(DiagnosticSeverity::Error, self.message.clone())
            .with_code(self.stable_code())
    }
}

impl TraitDiagnostic {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn kind(&self) -> &TraitDiagnosticKind {
        &self.kind
    }

    pub const fn code(&self) -> &'static str {
        trait_diagnostic_code(&self.kind)
    }

    pub fn duplicate_trait(name: impl Into<String>) -> Self {
        let name = name.into();
        Self::new(
            format!("duplicate trait declaration `{name}`"),
            TraitDiagnosticKind::DuplicateTrait { name },
        )
    }

    pub fn unknown_trait(name: impl Into<String>) -> Self {
        let name = name.into();
        Self::new(
            format!("unknown trait `{name}`"),
            TraitDiagnosticKind::UnknownTrait { name },
        )
    }

    pub fn duplicate_associated_type(
        trait_name: impl Into<String>,
        assoc: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let assoc = assoc.into();
        Self::new(
            format!("trait `{trait_name}` declares associated type `{assoc}` more than once"),
            TraitDiagnosticKind::DuplicateAssociatedType { trait_name, assoc },
        )
    }

    pub fn duplicate_associated_type_assignment(
        trait_name: impl Into<String>,
        assoc: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let assoc = assoc.into();
        Self::new(
            format!("impl `{trait_name}` assigns associated type `{assoc}` more than once"),
            TraitDiagnosticKind::DuplicateAssociatedTypeAssignment { trait_name, assoc },
        )
    }

    pub fn unknown_associated_type(
        trait_name: impl Into<String>,
        assoc: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let assoc = assoc.into();
        Self::new(
            format!("trait `{trait_name}` has no associated type `{assoc}`"),
            TraitDiagnosticKind::UnknownAssociatedType { trait_name, assoc },
        )
    }

    pub fn missing_associated_type(
        trait_name: impl Into<String>,
        target: impl Into<String>,
        assoc: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let target = target.into();
        let assoc = assoc.into();
        Self::new(
            format!("impl `{trait_name}` for `{target}` is missing associated type `{assoc}`"),
            TraitDiagnosticKind::MissingAssociatedType {
                trait_name,
                target,
                assoc,
            },
        )
    }

    pub fn missing_required_method(
        trait_name: impl Into<String>,
        target: impl Into<String>,
        method: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let target = target.into();
        let method = method.into();
        Self::new(
            format!("impl `{trait_name}` for `{target}` is missing required method `{method}`"),
            TraitDiagnosticKind::MissingRequiredMethod {
                trait_name,
                target,
                method,
            },
        )
    }

    pub fn missing_required_method_body(
        trait_name: impl Into<String>,
        target: impl Into<String>,
        method: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let target = target.into();
        let method = method.into();
        Self::new(
            format!("impl `{trait_name}` for `{target}` method `{method}` must have a body"),
            TraitDiagnosticKind::MissingRequiredMethodBody {
                trait_name,
                target,
                method,
            },
        )
    }

    pub fn impl_method_signature_mismatch(
        trait_name: impl Into<String>,
        method: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let method = method.into();
        Self::new(
            format!("impl method `{method}` does not match trait `{trait_name}` requirement"),
            TraitDiagnosticKind::ImplMethodSignatureMismatch { trait_name, method },
        )
    }

    pub fn duplicate_method(trait_name: impl Into<String>, method: impl Into<String>) -> Self {
        let trait_name = trait_name.into();
        let method = method.into();
        Self::new(
            format!("trait or impl `{trait_name}` defines method `{method}` more than once"),
            TraitDiagnosticKind::DuplicateMethod { trait_name, method },
        )
    }

    pub fn unknown_trait_method(trait_name: impl Into<String>, method: impl Into<String>) -> Self {
        let trait_name = trait_name.into();
        let method = method.into();
        Self::new(
            format!("trait `{trait_name}` has no required method `{method}`"),
            TraitDiagnosticKind::UnknownTraitMethod { trait_name, method },
        )
    }

    pub fn duplicate_impl(trait_name: impl Into<String>, target: impl Into<String>) -> Self {
        let trait_name = trait_name.into();
        let target = target.into();
        Self::new(
            format!("duplicate impl `{trait_name}` for `{target}`"),
            TraitDiagnosticKind::DuplicateImpl { trait_name, target },
        )
    }

    pub fn overlapping_impl(
        trait_name: impl Into<String>,
        existing: impl Into<String>,
        candidate: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let existing = existing.into();
        let candidate = candidate.into();
        Self::new(
            format!(
                "impl `{trait_name}` for `{candidate}` overlaps existing impl for `{existing}`"
            ),
            TraitDiagnosticKind::OverlappingImpl {
                trait_name,
                existing,
                candidate,
            },
        )
    }

    pub fn orphan_impl(trait_name: impl Into<String>, target: impl Into<String>) -> Self {
        let trait_name = trait_name.into();
        let target = target.into();
        Self::new(
            format!("impl `{trait_name}` for `{target}` violates Arcweft orphan rules"),
            TraitDiagnosticKind::OrphanImpl { trait_name, target },
        )
    }

    pub fn pub_impl_unsupported(impl_head: impl Into<String>) -> Self {
        let impl_head = impl_head.into();
        Self::new(
            format!("`{impl_head}` cannot declare explicit visibility in seq08.1"),
            TraitDiagnosticKind::PubImplUnsupported { impl_head },
        )
    }

    pub fn associated_type_default_unsupported(
        trait_name: impl Into<String>,
        assoc: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let assoc = assoc.into();
        Self::new(
            format!(
                "trait `{trait_name}` associated type `{assoc}` default is parsed but not implemented in seq08.1"
            ),
            TraitDiagnosticKind::AssociatedTypeDefaultUnsupported { trait_name, assoc },
        )
    }

    pub fn associated_type_constructor_unsupported(
        trait_name: impl Into<String>,
        assoc: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let assoc = assoc.into();
        Self::new(
            format!(
                "trait `{trait_name}` associated type constructor `{assoc}` is parsed but not implemented in seq08.1"
            ),
            TraitDiagnosticKind::AssociatedTypeConstructorUnsupported { trait_name, assoc },
        )
    }

    pub fn trait_default_method_unsupported(
        trait_name: impl Into<String>,
        method: impl Into<String>,
    ) -> Self {
        let trait_name = trait_name.into();
        let method = method.into();
        Self::new(
            format!(
                "trait `{trait_name}` default method `{method}` is parsed but not implemented in seq08.1"
            ),
            TraitDiagnosticKind::TraitDefaultMethodUnsupported { trait_name, method },
        )
    }

    pub fn associated_type_in_inherent_impl(assoc: impl Into<String>) -> Self {
        let assoc = assoc.into();
        Self::new(
            format!("inherent impl cannot assign associated type `{assoc}`"),
            TraitDiagnosticKind::AssociatedTypeInInherentImpl { assoc },
        )
    }

    pub fn ambiguous_projection(subject: impl Into<String>, assoc: impl Into<String>) -> Self {
        let subject = subject.into();
        let assoc = assoc.into();
        Self::new(
            format!("associated type projection `{subject}::{assoc}` is ambiguous"),
            TraitDiagnosticKind::AmbiguousProjection { subject, assoc },
        )
    }

    pub fn ambiguous_method<'a>(
        method: impl Into<String>,
        traits: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        let method = method.into();
        let traits = traits
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        Self::new(
            format!(
                "method `{method}` is provided by multiple traits: {}",
                traits.join(", ")
            ),
            TraitDiagnosticKind::AmbiguousMethod { method, traits },
        )
    }

    pub fn raw_trait_member(trait_name: impl Into<String>, raw: impl Into<String>) -> Self {
        let trait_name = trait_name.into();
        let raw = raw.into();
        Self::new(
            format!("raw trait member in `{trait_name}` is not semantically supported: {raw}"),
            TraitDiagnosticKind::RawTraitMember { trait_name, raw },
        )
    }

    pub fn raw_impl_member(impl_head: impl Into<String>, raw: impl Into<String>) -> Self {
        let impl_head = impl_head.into();
        let raw = raw.into();
        Self::new(
            format!("raw impl member in `{impl_head}` is not semantically supported: {raw}"),
            TraitDiagnosticKind::RawImplMember { impl_head, raw },
        )
    }

    fn new(message: String, kind: TraitDiagnosticKind) -> Self {
        Self { message, kind }
    }
}

impl TypeCheckWarning {
    pub(crate) fn public_abi_anonymous_sum(
        context: impl Into<String>,
        type_ref: impl Into<String>,
    ) -> Self {
        let context = context.into();
        let type_ref = type_ref.into();
        Self {
            message: format!(
                "{context} exposes anonymous sum `{type_ref}`; public ABI and save data are more stable with a nominal enum"
            ),
            kind: TypeCheckWarningKind::PublicAbiAnonymousSum { context, type_ref },
        }
    }

    pub(crate) fn effect(diagnostic: EffectDiagnostic) -> Self {
        Self {
            message: diagnostic.message().to_owned(),
            kind: TypeCheckWarningKind::Effect { diagnostic },
        }
    }

    /// Human-readable type-analysis warning.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Machine-readable warning family and structured fields.
    pub const fn kind(&self) -> &TypeCheckWarningKind {
        &self.kind
    }

    /// Stable compiler-wide diagnostic code.
    pub fn stable_code(&self) -> String {
        typecheck_warning_code(&self.kind)
    }

    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::new(DiagnosticSeverity::Warning, self.message.clone())
            .with_code(self.stable_code())
    }
}

fn typecheck_error_code(kind: &TypeCheckErrorKind) -> String {
    match kind {
        TypeCheckErrorKind::Message => "sema.typecheck".to_owned(),
        TypeCheckErrorKind::ArgumentTypeMismatch { .. } => {
            "sema.typecheck.argument_type_mismatch".to_owned()
        }
        TypeCheckErrorKind::UnsupportedAssignmentTarget { .. } => {
            "sema.typecheck.unsupported_assignment_target".to_owned()
        }
        TypeCheckErrorKind::MissingRustPackageMetadata { .. } => {
            "sema.extern_rust.missing_metadata".to_owned()
        }
        TypeCheckErrorKind::MissingRustExport { .. } => {
            "sema.extern_rust.missing_export".to_owned()
        }
        TypeCheckErrorKind::RustExportSignatureMismatch { .. } => {
            "sema.extern_rust.signature_mismatch".to_owned()
        }
        TypeCheckErrorKind::InlineCallErrorPolicyMissing { .. } => {
            "sema.dialogue.inline_error_policy_missing".to_owned()
        }
        TypeCheckErrorKind::InlineFailurePolicyConflict { .. } => {
            "sema.dialogue.inline_failure_policy_conflict".to_owned()
        }
        TypeCheckErrorKind::UnknownInlineFailurePolicy { .. } => {
            "sema.dialogue.unknown_inline_failure_policy".to_owned()
        }
        TypeCheckErrorKind::Effect { diagnostic } => diagnostic.code().as_str().to_owned(),
        TypeCheckErrorKind::Trait { diagnostic } => diagnostic.code().to_owned(),
    }
}

const fn trait_diagnostic_code(kind: &TraitDiagnosticKind) -> &'static str {
    match kind {
        TraitDiagnosticKind::DuplicateTrait { .. } => "sema.trait.duplicate_trait",
        TraitDiagnosticKind::UnknownTrait { .. } => "sema.trait.unknown_trait",
        TraitDiagnosticKind::DuplicateAssociatedType { .. } => {
            "sema.trait.duplicate_associated_type"
        }
        TraitDiagnosticKind::DuplicateAssociatedTypeAssignment { .. } => {
            "sema.trait.duplicate_associated_type_assignment"
        }
        TraitDiagnosticKind::UnknownAssociatedType { .. } => "sema.trait.unknown_associated_type",
        TraitDiagnosticKind::MissingAssociatedType { .. } => "sema.trait.missing_associated_type",
        TraitDiagnosticKind::MissingRequiredMethod { .. } => "sema.trait.missing_required_method",
        TraitDiagnosticKind::MissingRequiredMethodBody { .. } => {
            "sema.trait.missing_required_method_body"
        }
        TraitDiagnosticKind::ImplMethodSignatureMismatch { .. } => {
            "sema.trait.impl_method_signature_mismatch"
        }
        TraitDiagnosticKind::DuplicateMethod { .. } => "sema.trait.duplicate_method",
        TraitDiagnosticKind::UnknownTraitMethod { .. } => "sema.trait.unknown_trait_method",
        TraitDiagnosticKind::DuplicateImpl { .. } => "sema.trait.duplicate_impl",
        TraitDiagnosticKind::OverlappingImpl { .. } => "sema.trait.overlapping_impl",
        TraitDiagnosticKind::OrphanImpl { .. } => "sema.trait.orphan_impl",
        TraitDiagnosticKind::PubImplUnsupported { .. } => "sema.trait.pub_impl_unsupported",
        TraitDiagnosticKind::AssociatedTypeDefaultUnsupported { .. } => {
            "sema.trait.associated_type_default_unsupported"
        }
        TraitDiagnosticKind::AssociatedTypeConstructorUnsupported { .. } => {
            "sema.trait.associated_type_constructor_unsupported"
        }
        TraitDiagnosticKind::TraitDefaultMethodUnsupported { .. } => {
            "sema.trait.default_method_unsupported"
        }
        TraitDiagnosticKind::AssociatedTypeInInherentImpl { .. } => {
            "sema.trait.associated_type_in_inherent_impl"
        }
        TraitDiagnosticKind::AmbiguousProjection { .. } => "sema.trait.ambiguous_projection",
        TraitDiagnosticKind::AmbiguousMethod { .. } => "sema.trait.ambiguous_method",
        TraitDiagnosticKind::RawTraitMember { .. } => "sema.trait.raw_trait_member",
        TraitDiagnosticKind::RawImplMember { .. } => "sema.trait.raw_impl_member",
    }
}

fn typecheck_warning_code(kind: &TypeCheckWarningKind) -> String {
    match kind {
        TypeCheckWarningKind::PublicAbiAnonymousSum { .. } => {
            "sema.public_abi.anonymous_sum".to_owned()
        }
        TypeCheckWarningKind::Effect { diagnostic } => diagnostic.code().as_str().to_owned(),
    }
}
