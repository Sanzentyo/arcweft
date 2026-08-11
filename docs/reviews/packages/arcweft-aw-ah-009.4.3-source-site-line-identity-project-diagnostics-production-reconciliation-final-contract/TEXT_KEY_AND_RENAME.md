# Text key and rename contract

## 1. Exact derivation

```text
DialogueLineId:  say.<complete body>
DialogueTextKey: text.<complete body>
```

Derivation removes exactly the first `say.` prefix and prepends `text.`. It
performs no segment interpretation. Because `text.` is one byte longer than
`say.`, a 256-byte line ID can produce a 257-byte derived key; that site is
AW-CD-025 unless the author supplies a valid shorter explicit key.

## 2. Provenance

```rust
pub enum DialogueTextKeyOrigin {
    Explicit,
    Derived,
}
```

- explicit source: exact `text_key` value SourceSpan;
- derived source: application SourceSpan;
- derived explanation: optional explicit line-ID coordinate span;
- accepted record: typed key, origin, and source site.

No synthetic zero range or source string is stored.

## 3. Explicit key validation

An explicit key must be a typed absolute `HirIdRef` whose body is
`text.<nonempty tail>`, at most 256 UTF-8 bytes. Unknown/dynamic expressions are
AW-CD-023; wrong, relative, or family-relative forms are AW-CD-024; duplicate
coordinates are AW-CD-027.

## 4. Uniqueness

Text keys are not a project uniqueness namespace. Multiple accepted lines may
share one explicit localization key. `AcceptedDialogueLineInventory` may expose
a tooling multimap from text key to line indexes, but no acceptance error is
created. Derived keys remain unique as a consequence of line-ID uniqueness.

## 5. Collision publication

A candidate carries a validated text key so later stages do not reinterpret it.
Nevertheless, accepted text-key facts exist only inside a successfully returned
`AcceptedDialogueLineInventory`. Any line collision or project failure publishes
none of the candidate text keys.

## 6. Reference indexing

The existing project semantic/reference index gains line-reference records
produced from typed expected-family `HirIdRef` facts. It does not search source
text or parse generic strings. Each record contains:

```rust
pub struct AcceptedDialogueLineReference {
    target: DialogueLineId,
    source: SourceSpan,
    module: HirPackageModuleKey,
    expression: ExprId,
}
```

The line declaration/source site comes only from
`HirProject::dialogue_lines()`. Reference resolution, go-to-definition, find
references, and rename all use the same accepted generation.

## 7. Character rename

Character registration/alias/display-name rename never edits line IDs, text
keys, or line-reference indexes. There is no mapping from a line ID back to
CharacterId.

## 8. Explicit line rename

For an explicit line ID, rename:

1. validates a new `DialogueLineId`;
2. checks the project collision candidate transaction for the proposed value;
3. replaces the exact authored ID value span;
4. replaces typed line references from the accepted reference index; and
5. rebuilds/accepts the project transactionally.

If the text key is derived, it changes with the line. If explicit, it remains
unchanged.

## 9. Generated line rename

A generated ID has no authored ID value. Rename uses AW-AH-009.4.2 source
component/insertion facts to materialize exactly one immediate `id = @say.*`
coordinate:

- append to the existing immediate outer configuration call using its typed
  argument list/comma insertion site; or
- create the immediate configuration call around the target using exact target
  and bracket/colon component spans.

It then updates typed references and rebuilds the project. If no clean checked
insertion site exists, rename is unavailable; tooling does not source-scan or
fabricate a range.

This operation is a direct move from generated to `ExplicitAbsolute` origin,
not a compatibility alias.

## 10. Line rename independence

Line rename does not rename a flow, callable, Character, alias, local variable,
module, or package. Owner renames are separate refactors and may naturally
change generated prefixes; tooling previews resulting line/reference changes
from a candidate project build before applying edits.

## 11. Localization/runtime consumers

Localization and later runtime-plan lowering receive typed line/text facts from
the accepted project. Runtime-plan converts the durable line ID to its runtime
path using an owned checked conversion and carries the text key directly. No
consumer calls `split`, strips `say`, or looks for a character segment.
