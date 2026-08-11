# Diagnostic and source contract

## 1. Typed diagnostic variants and stable codes

`EffectDiagnostic` remains the owning enum. Add these variants and delete the
obsolete generic upper-bound variant:

```rust
pub enum EffectDiagnostic {
    ClosedRowMissing(ClosedRowMissingDiagnostic),
    TraitOmittedRowMissing(TraitOmittedRowMissingDiagnostic),
    TraitClosedRowMissing(TraitClosedRowMissingDiagnostic),
    // unrelated still-valid effect variants remain
}

pub struct MissingEffectCause {
    effect: EffectId,
    trace: EffectTrace,
}
```

Exact code mapping:

| Parent row | Variant | Stable code |
|---|---|---|
| E015 | `TraitOmittedRowMissing` | `sema.trait.effect.omitted_row_missing` |
| E016 | `TraitClosedRowMissing` | `sema.trait.effect.closed_row_missing` |
| E022 | `ClosedRowMissing` | `sema.effect.closed_row_missing` |
| E023 | `ClosedRowMissing` | `sema.effect.closed_row_missing` |

`UpperBoundExceeded` and `AWF-EFX-001` are removed in the same authority switch.
There is no compatibility mapping and no simultaneous old/new emission.

## 2. Exact payloads

```rust
pub struct ClosedRowMissingDiagnostic {
    callable: CheckedCallableId,
    permitted: EffectRow,
    inferred: EffectRow,
    causes: Box<[MissingEffectCause]>,
}

pub struct TraitOmittedRowMissingDiagnostic {
    requirement: CheckedCallableId,
    implementation: CheckedCallableId,
    inferred: EffectRow,
    causes: Box<[MissingEffectCause]>,
}

pub struct TraitClosedRowMissingDiagnostic {
    requirement: CheckedCallableId,
    implementation: CheckedCallableId,
    permitted: EffectRow,
    inferred: EffectRow,
    causes: Box<[MissingEffectCause]>,
}
```

`causes` contains one entry per missing effect, sorted by `EffectId`. One
diagnostic represents one violated contract owner even when several effects are
missing. The rows in a diagnostic are immutable payload snapshots; they are not
registry authority.

Exact messages:

- E015: `trait method omits an effect row; its closed-empty contract does not permit {effects}`
- E016: `trait method effect row does not permit {effects} used by its implementation`
- E022/E023: `callable effect row does not permit {effects}`

`{effects}` is the canonical comma-separated `EffectId` order.

## 3. Primary and related spans

### E015 — omitted bodyless requirement

- **Primary:** exact trait requirement method-name span. This span owns the
  implicit closed-empty contract.
- **Related 1:** exact implementation method-name span.
- **Related after that:** shortest trace steps from the implementation root to
  each direct effect site, sorted by effect ID.
- There is no zero-width fabricated effect-clause span.

### E016 — explicit closed trait row

- **Primary:** whole span of the first authored `effects` clause in source order.
- **Related:** later `effects` clause whole spans, requirement method-name span,
  implementation method-name span, then trace steps.
- If the closed row comes from the existing typed signature-row syntax without
  an `effects` clause, its exact typed-row `TypeSourceEvidence` span is primary.

### E022 — direct Await omitted by own closed row

- **Primary:** first authored row source as above.
- **Related terminal:** exact `await` keyword span that records
  `control.suspend`.
- The trace contains zero call edges.

### E023 — transitive callee propagation

- **Primary:** first authored row source of the closed caller.
- **Related:** exact call-expression spans for the selected shortest typed path,
  in root-to-leaf order, then the exact terminal effect span.
- For `control.suspend`, the terminal is the exact Await keyword span.

### Standard requirement without authored source

A programmatically installed standard requirement uses structured standard
source metadata. If its implementation violates the row, the implementation
method-name span is primary and the standard symbol is a non-source note. No
fake file/range is generated. This does not change the authored E015 rule.

## 4. Typed trace model

```rust
pub struct EffectTrace {
    effect: EffectId,
    root: CheckedEffectCallableId,
    steps: Box<[EffectTraceStep]>,
    terminal: EffectTerminal,
}

pub struct EffectTraceStep {
    caller: CheckedEffectCallableId,
    callee: CheckedEffectCallableId,
    call: SourceSpan,
}

pub struct EffectTerminal {
    owner: CheckedEffectCallableId,
    effect: EffectId,
    source: SourceSpan,
}
```

The current path/string/line/column `EffectSite` and text-note-only trace are
replaced. Display positions are derived only at rendering time from the exact
accepted source document.

## 5. Deterministic shortest trace

A trace is selected independently for every missing effect.

1. Nodes are `CheckedEffectCallableId`.
2. Local/project call edges are unit weight and carry an exact call span.
3. Direct-effect terminals carry an exact effect span.
4. Run breadth-first search from the violated callable/implementation.
5. Sort outgoing edges by:

```text
(call.source.id,
 call.source.revision,
 call.range.start,
 call.range.end,
 callee CheckedEffectCallableId)
```

6. Among terminals at the minimum edge count, choose the lexicographically
   smallest complete edge-key sequence, then terminal source key.
7. Track the best `(distance, path_key)` per `(callable, effect)` so cycles are
   finite and deterministic.
8. A direct effect has zero steps and only a terminal.
9. A typed function/method value invocation contributes its known target edge.
   An unknown row is rejected before trace construction.

This algorithm replaces any dependence on map insertion order, display names,
filesystem order, or source reparsing.

## 6. Related-label wording and ordering

For each cause, labels are:

- call step: `call propagates effect {effect} to {callee}`;
- terminal Await: `await introduces control.suspend`;
- other terminal: `operation introduces {effect}`.

Shared spans across multiple causes are coalesced into one related label listing
all relevant effects in canonical order. Related labels are ordered:

1. additional authored contract sources;
2. requirement method name when not primary;
3. implementation method name;
4. traces by effect ID, each root-to-terminal.

Diagnostics are globally sorted by primary source identity/range, stable code,
requirement ID, implementation/callable ID, then missing effect set.

## 7. Own-contract and trait-contract precedence

A method body can violate two independent contracts:

1. its own authored bounded row; and
2. a trait requirement row.

The checker validates the own contract first, then every trait conformance. It
emits one typed diagnostic for each distinct violated owner. This is intentional
semantic evidence, not legacy/new dual emission. Identical diagnostics from
multiple inherited paths are coalesced only when owner IDs and payloads are
identical.

## 8. Source validity

A typed diagnostic is published only with source spans valid for its checked
context. A mismatched project/source revision produces the existing
transaction/source-validation failure and the stale report is discarded. The
diagnostic renderer does not fall back to a name span, line/column, or parsed
text from another revision.

## 9. CLI and LSP projection

`EffectDiagnostic` owns one inherent:

```rust
pub fn diagnostic(&self, catalog: &CheckedCallableCatalog) -> Diagnostic;
```

`TypeCheckError::diagnostic()` delegates to it. The resulting
`arcweft_source::Diagnostic` contains the stable code, message, primary
`SourceSpan`, related labels, and canonical payload notes.

- CLI uses the accepted source line index to display that diagnostic.
- LSP validates each span against the request snapshot and converts byte ranges
  to protocol ranges.
- Both consumers receive the same stable code, message, primary range, related
  range order, and effect IDs.
- Neither consumer reparses source, recognizes `effects` spelling, or has a
  trait-specific formatting branch.

## 10. Legacy migration boundary

Delete:

- `EffectDiagnosticKind::UpperBoundExceeded`;
- code `AWF-EFX-001`;
- the old generic row-overflow renderer;
- text-only `EffectTrace` note projection; and
- any source-name/path/line/column reconstruction used for these rows.

Retain unrelated `AWF-EFX-*` variants only until their own contracts replace
them; this package does not rename a distinct capability/forbidden/pure error
merely for consistency. There is no alias from `AWF-EFX-001` to the new code.

## 11. Exact diagnostic tests

The test matrix requires structural equality of typed diagnostics before text
rendering, then equality of CLI/LSP code/ranges/related ordering after
projection. Snapshot-only message tests are insufficient by themselves.
