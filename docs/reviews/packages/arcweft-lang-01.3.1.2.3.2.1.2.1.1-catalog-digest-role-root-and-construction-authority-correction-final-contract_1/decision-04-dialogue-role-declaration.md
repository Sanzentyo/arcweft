# Decision 04 — typed CharacterDialogue role declaration

## Rust-shaped declaration

Owner: new responsibility module `crates/arcweft-lang-sema/src/character_dialogue/runtime_types.rs`; it is re-exported narrowly by the existing `character_dialogue` module.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeRoleDeclaration {
    role: CharacterDialogueRuntimeRole,
    nominal: AcceptedNominalType,
    semantic_identity: RuntimeSemanticTypeId,
    checked_type: RuntimeCheckedType,
    world: AcceptedNominalWorldStamp,
    origin: AcceptedNominalOrigin,
    declaration: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum CharacterDialogueRuntimeRoleDeclarationError {
    #[error("Style is derived and cannot have an authored declaration")]
    AuthoredStyle,
    #[error("missing CharacterDialogue role declaration {role:?}")]
    Missing { role: CharacterDialogueRuntimeRole },
    #[error("duplicate CharacterDialogue role declaration {role:?}")]
    Duplicate {
        role: CharacterDialogueRuntimeRole,
        first: Option<SourceSpan>,
        second: Option<SourceSpan>,
    },
    #[error("role {role:?} has non-standard accepted nominal owner")]
    WrongOwner { role: CharacterDialogueRuntimeRole, actual: AcceptedNominalOwnerId },
    #[error("role {role:?} has wrong accepted nominal path")]
    WrongPath { role: CharacterDialogueRuntimeRole, actual: TypePath },
    #[error("role {role:?} must have zero generic arguments")]
    WrongArity { role: CharacterDialogueRuntimeRole, actual: usize },
    #[error("role {role:?} has wrong opaque producer")]
    WrongProducer {
        role: CharacterDialogueRuntimeRole,
        actual: Option<RuntimeOpaqueTypeProducerId>,
    },
    #[error("role {role:?} has wrong accepted nominal origin")]
    WrongOrigin { role: CharacterDialogueRuntimeRole, actual: AcceptedNominalOrigin },
    #[error("role {role:?} belongs to a different accepted world")]
    WrongWorld {
        role: CharacterDialogueRuntimeRole,
        expected: AcceptedNominalWorldStamp,
        actual: AcceptedNominalWorldStamp,
    },
    #[error("role {role:?} is unresolved")]
    Unresolved { role: CharacterDialogueRuntimeRole, source: Option<SourceSpan> },
    #[error("role coordinate escaped into a final semantic type")]
    LeakedRoleCoordinate { role: CharacterDialogueRuntimeRole },
    #[error("role {role:?} retained a Named placeholder")]
    LeakedNamed { role: CharacterDialogueRuntimeRole, name: String },
    #[error("role {role:?} projected to a different closed runtime type")]
    ClosedTypeMismatch {
        role: CharacterDialogueRuntimeRole,
        expected: RuntimeCheckedType,
        actual: RuntimeCheckedType,
    },
    #[error("role {role:?} semantic identity differs from its accepted nominal")]
    SemanticIdentityMismatch {
        role: CharacterDialogueRuntimeRole,
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
}

impl CharacterDialogueRuntimeRoleDeclaration {
    pub(crate) fn try_from_standard_accepted_nominal(
        role: CharacterDialogueRuntimeRole,
        nominal: AcceptedNominalType,
        semantic_identity: RuntimeSemanticTypeId,
        checked_type: RuntimeCheckedType,
        world: AcceptedNominalWorldStamp,
        origin: AcceptedNominalOrigin,
        declaration: Option<SourceSpan>,
    ) -> Result<Self, CharacterDialogueRuntimeRoleDeclarationError>;

    pub const fn role(&self) -> CharacterDialogueRuntimeRole;
    pub const fn nominal(&self) -> &AcceptedNominalType;
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn checked_type(&self) -> &RuntimeCheckedType;
    pub const fn world(&self) -> &AcceptedNominalWorldStamp;
    pub const fn origin(&self) -> AcceptedNominalOrigin;
    pub const fn declaration(&self) -> Option<&SourceSpan>;
}
```

The declaration is not Serde and has no public constructor. It is issued only while the accepted nominal environment registers the exact standard nominal row. The retained `AcceptedNominalType`, `AcceptedNominalOwnerId::Standard`, `AcceptedNominalOrigin::Domain`, accepted world stamp, `SourceSpan`, opaque producer, and `RuntimeSemanticTypeId` are the source evidence. No role is recognized from a display label after registration.

## Original lower enum behavior

Add behavior to the current owner, not a helper trait or side table:

```rust
impl CharacterDialogueRuntimeRole {
    pub const ALL: [Self; 7] = [
        Self::Stage, Self::Portrait, Self::Focus, Self::Cleanup,
        Self::Hook, Self::Style, Self::RichText,
    ];

    pub const AUTHORED_BASE: [Self; 6] = [
        Self::Stage, Self::Portrait, Self::Focus,
        Self::Cleanup, Self::Hook, Self::RichText,
    ];

    pub const fn canonical_tag(self) -> u8 { self as u8 }
    pub const fn is_authored_base(self) -> bool { !matches!(self, Self::Style) }
}
```

Existing `repr(u8)` ordinals and serde names remain: Stage 0/`stage`, Portrait 1/`portrait`, Focus 2/`focus`, Cleanup 3/`cleanup`, Hook 4/`hook`, Style 5/`style`, RichText 6/`rich_text`.
