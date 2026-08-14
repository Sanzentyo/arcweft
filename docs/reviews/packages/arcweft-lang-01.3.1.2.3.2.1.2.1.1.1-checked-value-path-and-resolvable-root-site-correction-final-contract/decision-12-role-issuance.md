# Decision 12 — role issuance through current TypeCheckEnv and AcceptedNominalWorld

## Standard nominal registration

The six fixed authored roles are inserted by the current `TypeCheckEnv` owner before an `AcceptedNominalWorld` exists:

```rust
impl TypeCheckEnv {
    #[must_use]
    fn with_standard_character_dialogue_role_nominals(self) -> Self;
}
```

`with_standard_builtins` calls this method immediately after standard accepted/presentation nominals and before standard callable families. The method registers exactly Stage, Portrait, Focus, Cleanup, Hook, and RichText as zero-arity standard opaque accepted nominals with producer `std.character_dialogue`, the exact retained paths/origins, and no public source-name recognizer. Style is not registered.

Standard callable schemas use the existing `TypeKind` owner with one new internal coordinate:

```rust
pub enum TypeKind {
    CharacterDialogueRole(CharacterDialogueRuntimeRole),
    // existing variants
}
```

All behavior is added to `TypeKind`'s inherent/normalization matches. The six relevant `TypeKind::Named` placeholders are removed in the same change. There is no extension trait or string table.

## Atomic world projection

Owner: `crates/arcweft-lang-sema/src/character_dialogue/runtime_types.rs`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeRoleRegistry {
    world: AcceptedNominalWorldStamp,
    declarations: [CharacterDialogueRuntimeRoleDeclaration; 6],
    style_semantic: TypeKind,
    style_checked: RuntimeCheckedType,
}

impl AcceptedNominalWorld {
    pub(crate) fn try_project_character_dialogue_runtime_roles(
        &self,
    ) -> Result<CharacterDialogueRuntimeRoleRegistry,
                CharacterDialogueRuntimeRoleDeclarationError>;
}

impl CharacterDialogueRuntimeRoleRegistry {
    pub fn declaration(
        &self,
        role: CharacterDialogueRuntimeRole,
    ) -> Option<&CharacterDialogueRuntimeRoleDeclaration>;
    pub fn semantic_type(&self, role: CharacterDialogueRuntimeRole) -> &TypeKind;
    pub fn checked_type(&self, role: CharacterDialogueRuntimeRole) -> &RuntimeCheckedType;
    #[must_use] pub const fn world(&self) -> &AcceptedNominalWorldStamp;
}
```

The projection resolves each exact `AcceptedNominalId` through `AcceptedNominalWorld::typecheck_env()` and `nominal_catalog()`, verifies owner `Standard`, origin `Domain`, zero arity, opaque producer, world stamp, semantic identity, and checked projection, then constructs the six-element array atomically. Style is derived as the retained ordered Choice of `EntityRef<Style>` and accepted RichText. A Style authored row is an error.

The registry is non-Serde, has private fields, no public constructor, and no post-publication insertion method. `CharacterDialogueRuntimeRoleDeclaration::try_from_standard_accepted_nominal` remains crate-private.

## Registrar order

Both current registrar publication paths use this exact order:

1. build the final `Arc<AcceptedNominalWorld>`;
2. call `try_project_character_dialogue_runtime_roles` and wrap the complete registry in `Arc`;
3. build standard/registered callable schemas, substituting every `TypeKind::CharacterDialogueRole` through that registry;
4. fail if any role coordinate or relevant `Named` placeholder survives normalization;
5. build `CharacterDialogueCustomFieldRegistry` from the same nominal-world stamp;
6. finish callable metadata/digests;
7. construct one `RegisteredTypeCheckEnv` containing nominal world, role registry, custom-field registry, Rust metadata, callables, characters, and all existing digests;
8. publish the `Arc<RegisteredTypeCheckEnv>` only after every component succeeds.

`RegisteredTypeCheckEnv` gains:

```rust
character_dialogue_roles: Arc<CharacterDialogueRuntimeRoleRegistry>,

pub fn character_dialogue_roles(
    &self,
) -> &CharacterDialogueRuntimeRoleRegistry;
```

There is no `AcceptedNominalEnvironment`, no post-publication mutation, no second environment allocation with different roles, and no role declaration issued by runtime-driver/dialogue. The exact current owners `TypeCheckEnv`, `AcceptedNominalWorld`, registrar, and `RegisteredTypeCheckEnv` form the sole issuance path.
