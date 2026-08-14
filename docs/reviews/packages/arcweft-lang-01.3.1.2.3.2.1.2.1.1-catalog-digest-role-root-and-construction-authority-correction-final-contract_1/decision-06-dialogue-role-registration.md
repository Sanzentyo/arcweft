# Decision 06 — standard registration and role-coordinate substitution

## Typed semantic coordinate

Add one variant to the existing sema owner:

```rust
pub enum TypeKind {
    CharacterDialogueRole(CharacterDialogueRuntimeRole),
}
```

This is an internal semantic coordinate, not a final checked type and not an authored path. Put all new match behavior in the existing `TypeKind` inherent implementation and normalized-type projection context; do not create an extension trait or name table.

## Standard registration path

The accepted nominal environment's standard registration performs these steps exactly once for each member of `CharacterDialogueRuntimeRole::AUTHORED_BASE`:

1. Construct the fixed `AcceptedNominalId` from owner `Standard` and the exact one-segment accepted path from `CHARACTER_DIALOGUE_ROLE_TABLE.csv`.
2. Register a zero-arity `AcceptedNominalRecord` with `AcceptedNominalOrigin::Domain`, current `AcceptedNominalWorldStamp`, and `AcceptedNominalSemantics::Opaque { producer: std.character_dialogue }`.
3. Obtain the row's existing `RuntimeSemanticTypeId` from the accepted nominal catalog.
4. Project it into `RuntimeCheckedType::Opaque { owner: RuntimeOpaqueTypeOwner::exact(...) }` through the accepted nominal projection, not by locally constructing identity bytes.
5. Call the crate-private constructor from Decision 04 and insert by role into `AcceptedCharacterDialogueRuntimeTypes`.
6. Duplicate role, nominal, path, or semantic identity fails before publication. The complete six-row candidate is published atomically.
7. Derive Style from EntityRef<Style> and the already accepted RichText row; Style has no independent registry row.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedCharacterDialogueRuntimeTypes {
    world: AcceptedNominalWorldStamp,
    declarations: [CharacterDialogueRuntimeRoleDeclaration; 6],
    style_semantic: TypeKind,
    style_checked: RuntimeCheckedType,
}

impl AcceptedCharacterDialogueRuntimeTypes {
    pub(crate) fn try_from_standard_environment(
        environment: &AcceptedNominalEnvironment,
        world: AcceptedNominalWorldStamp,
    ) -> Result<Self, CharacterDialogueRuntimeRoleDeclarationError>;

    pub fn declaration(
        &self,
        role: CharacterDialogueRuntimeRole,
    ) -> Option<&CharacterDialogueRuntimeRoleDeclaration>;

    pub fn semantic_type(
        &self,
        role: CharacterDialogueRuntimeRole,
    ) -> &TypeKind;

    pub fn checked_type(
        &self,
        role: CharacterDialogueRuntimeRole,
    ) -> &RuntimeCheckedType;

    pub const fn world(&self) -> &AcceptedNominalWorldStamp;
}
```

## Callable schema substitution

The standard callable family declares roles directly:

```rust
TypeKind::CharacterDialogueRole(CharacterDialogueRuntimeRole::Stage)
TypeKind::CharacterDialogueRole(CharacterDialogueRuntimeRole::Portrait)
TypeKind::CharacterDialogueRole(CharacterDialogueRuntimeRole::Focus)
TypeKind::CharacterDialogueRole(CharacterDialogueRuntimeRole::Cleanup)
TypeKind::Seq(Box::new(TypeKind::CharacterDialogueRole(CharacterDialogueRuntimeRole::Hook)))
TypeKind::CharacterDialogueRole(CharacterDialogueRuntimeRole::Style)
TypeKind::CharacterDialogueRole(CharacterDialogueRuntimeRole::RichText)
```

During accepted-world schema publication, each coordinate is substituted through `AcceptedCharacterDialogueRuntimeTypes::semantic_type`. After that phase, any remaining coordinate yields `LeakedRoleCoordinate`; any old relevant `Named` row yields `LeakedNamed`. Aliases and source normalization resolve to the typed callable parameter before substitution. There is no reverse recognition of `DialogueStage`, `RichTextStyle`, display labels, path strings, or any `Named` spelling.
