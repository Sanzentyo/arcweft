use thiserror::Error;

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
