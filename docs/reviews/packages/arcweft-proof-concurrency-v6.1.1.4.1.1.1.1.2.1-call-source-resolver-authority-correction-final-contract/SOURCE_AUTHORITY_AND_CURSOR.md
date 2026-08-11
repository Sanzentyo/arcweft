# Sole source authority and cursor contract

## Authority

The only final-HIR component map is:

```rust
HirSourceIndex {
    components: BTreeMap<HirSourceQuery, HirSourceSite>,
}

HirModule::source_site(
    expected_source: &SourceDocumentIdentity,
    query: &HirSourceQuery,
) -> Result<HirSourceLookup, HirSourceQueryError>
```

- owner whole: arena-slot metadata;
- component present: `HirSourcePresence::Present(HirSourceSite)`;
- optional absent: `HirSourcePresence::AbsentOptional`;
- owner clean/poisoned: `HirSourceOwnerStatus`;
- inapplicable role: typed `HirSourceQueryError::RoleNotApplicable`;
- no stored `RoleNotApplicable`, no stored second Whole, no raw range reader.

Validation precedence:

1. owner database/module/kind/liveness;
2. role applicability;
3. role ordinal validity;
4. expected `SourceDocumentIdentity`;
5. source revision;
6. retained source length;
7. committed presence and owner status.

A present-invalid token keeps its `SourceSpan` or checked
`HirInsertionPoint`; semantic invalidity appears through owner status/issue, not
by deleting the source site.

## Ordinary active argument

The inherent final-HIR cursor view reads only committed source-query results and
the exact expected source identity.

For list `(a, b)`:

- cursor in `(` token: outside;
- cursor exactly `open.end()`: slot 0;
- cursor before first comma: slot 0;
- cursor at `comma.start()` or after it: slot 1;
- cursor at `close.start()` with no trailing comma: slot 1;
- cursor at/after `close.end()`: outside.

For `(a,)`, trailing comma start is one-past slot 1.  
For `()`, `open.end() == close.start()` selects slot 0.  
Missing close uses the committed recovery-end insertion as the inclusive
interior end.

## Explicit type-application active slot

The same rule applies to `<...>`:

- direct `<` or turbofish `::<` token bytes are outside;
- `open_angle.end()` selects type slot 0;
- a comma starts the following type slot;
- an authored trailing comma starts one-past;
- without trailing comma the final type argument remains active at
  `close_angle.start()`;
- an empty/recovered list selects slot 0 at its insertion/recovery end;
- outside returns typed not-applicable.

The type-application cursor is independent from the call argument cursor and
from generic arguments inside the associated receiver.
