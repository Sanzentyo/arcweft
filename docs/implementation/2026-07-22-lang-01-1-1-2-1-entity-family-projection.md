# Lang-01.1.1.2.1 entity-family applied-type projection

## Intake

- package: `docs/reviews/packages/zips/lang-01.1.1.2.1-entity-family-applied-type-projection-final-contract.zip`
- package SHA-256: `FDAFDCA7B5D6682504A901274EB05C7B74C19816063C9A959C69DE0157A01906`
- baseline: `4fd6331dc342d30a7f4ac7774852b60801866ef7`
- package verification: 17/17 members matched the manifest; 71 matrix rows were present

The package is implementation-ready. It retains `Ref<EntityFamily>` and
defines it as a contextual checked projection, not an accepted nominal record,
string fallback, or second resolver.

## Implemented boundary

- `BuiltinTypeConstructor::Ref`, `Speaker`, and `SpeakerPreset` share the same
  typed `EntityFamily` argument expectation and projection inventory.
- Authored fixed entity families are supplied by one `EntityKind` inventory;
  open `Other`, project, external, accepted, const, generic, and ordinary type
  products do not become entity-family arguments.
- Wrong argument categories retain typed `TypeArgumentKind` evidence. Existing
  unknown, ambiguous, inaccessible, detached, recovery, and poison causes are
  reused instead of emitting a second diagnostic.
- Direct `Ref` remains a reserved language constructor. A qualified catalog
  path such as `pkg.Ref` remains independently owned and resolves through the
  accepted catalog.
- Callable parameter and return publication consume the same checked Ref
  product. Entry schema expansion consumes that product and then rejects it as
  a non-persistable canonical data shape; it does not encode it as `Named`.
- Compiler interface digests record the structural authored generic and its
  entity-family child. Repeated `Ref<Character>` signatures are stable and
  differ from `Ref<Flow>`.
- LSP hover, completion, references, definition, and rename select exact
  checked nominal node facts. The contextual completion list contains only
  authored fixed entity families.
- The current-pass `Ref<Flow>` check fixtures now pass without an opaque Ref
  fallback or local type converter.

No runtime, bytecode schema, or wire-format variant was added.

## Contract adjudication

Two rows in `TEST_MATRIX.csv` conflict with the package's requirement to keep
the existing resolver work accounting unchanged. They state work `1` for bare
`Ref` and work `3` for `Ref<Character, String>`, while the accepted resolver
already charges every visited node and every emitted diagnostic. The resulting
stable totals are respectively `2` and `4`.

The implementation preserves that existing generic rule and tests it
explicitly. Changing only Ref would create a one-off accounting exception;
changing all diagnostics would exceed this package and invalidate existing
budget contracts. This is an implementation-time correction to an internally
inconsistent matrix, not a new design request.

## Validation

Passed in the active cut:

- 42 focused sema nominal tests;
- all 6 `nominal_resolution_matrix` integration tests;
- 8 compiler persistent-fact tests;
- 4 LSP nominal-type tests;
- accepted callable Ref parameter/result publication;
- persisted-entry Ref rejection;
- current-pass check fixtures 014 and 015; and
- current-pass run fixtures after explicit flow return annotations.

The canonical structural audit at
`structure-audits/lang-01-1-1-2-1-entity-family-projection-2026-07-22/`
scanned 3,582 files, 1,880 Rust files, 873,410 physical Rust LOC, and 94
manifests with zero errors and 138 warnings. The audit initially found two
error-level size boundaries introduced by the combined active cut. Cohesive
access-expression handling and dialogue failure-policy validation were moved to
child modules, reducing `checker/expr.rs` to 2,472 LOC and
`checker/module.rs` to 2,429 LOC without behavior changes.

The workspace all-target/all-feature check and Clippy route pass, including
`-D warnings`. The ordinary workspace test route reaches 946/946 sema tests and
passes the Ref and direct-propagation coverage. Its only remaining failures are
the two CLI specification fixtures that publish `FsError` from an
`extern capability`; they are blocked on the already identified associated-type
AST/HIR publication switch and are not Ref projection failures.

Tier 2 passes 18 of 22 MCP/Agent-observe tests. The four remaining failures are
individually reproducible and concern stale MCP response assumptions: two tests
expect a JSON `false` where the current response omits the field, one expects an
obsolete resource-link array shape, and one cannot initialize the selected
player-backed observe fixture. They do not exercise Ref projection, but remain
an explicit later validation-harness reconciliation item rather than being
counted as passing.

## Next slice

Lang-01.1.1.2.2 is already present and internally verified. It must project
adapter/Rust callable and ADT publications through this same accepted world,
without modifying the Ref source/work/poison contract or adding a second
resolver. There is currently no request to send.
