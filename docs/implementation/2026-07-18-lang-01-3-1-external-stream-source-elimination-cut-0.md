# Lang-01.3.1 external Stream origin / Source elimination Cut 0

Date: 2026-07-18

Original repository basis: `9a63ac5512cd75947ba70195681e43ab968f9f12`

Package:
`Lang-01.3.1-external-stream-origin-source-elimination-final-contract.zip`

Package SHA-256:
`7B8589D51B2B089A27DB4BCEA762C81C75DD74DB0F75D580D2860570EEC73B65`

## Outcome

The package inventory and production tree were audited. The runtime direction
is accepted:

- remove the top-level runtime `source` declaration;
- remove public `Source<T, E>`;
- keep `Stream<T, E>` as the sole asynchronous sequence abstraction;
- move valid lifecycle, identity, policy, queue, replay, privacy, permission,
  cancellation, persistence, and hot-swap behavior to Stream-owned typed data;
- retain compiler provenance such as `arcweft-source`, `SourceDocument`,
  `SourceAnchor`, source ranges, and AWBC source maps; and
- add no compatibility parser, alias, dual reader, source gate, CSS path, or
  Takumi path.

Production implementation did not start in this cut because the returned
package contained an author-surface conflict and did not fix the public and
serialized runtime shapes needed by the required compile-clean migration
order.

The later returned
[Lang-01.3.1.1 external Stream callable surface reconciliation](../reviews/requests/2026-07-18-lang-01.3.1.1-external-stream-callable-surface-reconciliation.md)
settled the callable direction as ordinary `fn -> Stream<T, E>`, without
restoring `stream fn`. The first returned runtime-wire package remained
internally inconsistent; its required correction is
[Lang-01.3.1.2.1 typed Stream runtime/wire contract correction](../reviews/requests/2026-07-19-lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction.md).

## Package verification

The package contains:

- 78 unique required decisions, `RD-001` through `RD-078`;
- 218 unique test cases, `TM-001` through `TM-218`;
- `OPEN_QUESTIONS=0`; and
- 17 content entries covered by `MANIFEST.sha256`.

All listed manifest hashes matched. The package describes design and test work
only and declares `IMPLEMENTATION_PERFORMED=false`.

## Production inventory at intake

The audited revision still contained the complete duplicate runtime Source
path:

- syntax: `SourceItem`, `Item::Source`, parser dispatch, CST kind, lints, and
  fixtures;
- HIR: `HirSource` and `HirTopLevelDecl::Source`;
- sema: `TypeKind::Source`, `EntityKind::Source`, source-specific checking,
  project index, symbols, resolution, lifetime, and trait paths;
- public ADT: `Source<T, E>`;
- RuntimePlan/core: `SourcePlan`, `SourcePolicy`, `SourceRuntimeState`,
  `SourceEventKind`, `RuntimeSourceEvent`, `RuntimePlan.source_plans`,
  `RuntimeStepInput.source_events`, and separate source/stream engines;
- AWBC: Source tables, policies, handlers, opcodes, fiber state, codec,
  verifier, VM, product-step, and snapshot paths; and
- bundle, runtime-driver, runtime-host, native/Web player, compiler, CLI, LSP,
  tooling, examples, and stable design-document consumers.

The retained Stream path imported `SourceEventKind`, while runtime input and
effects carried separate source and stream event collections. This confirmed a
duplicate owner rather than a naming-only problem.

The inventory was produced by one-off source inspection. It is review evidence,
not an automated source gate.

## Required ownership transfer

| Current owner | Current behavior | Required Stream owner |
| --- | --- | --- |
| `SourcePlan.id` | one source ID | immutable stream-definition identity plus allocated runtime-instance identity |
| `SourcePlan` item/error types | source interface types | typed Stream definition/interface and handle layout |
| `SourcePlan.from` | runtime origin expression | closed typed origin: external, generator, or derived |
| `SourcePolicy` | capacity, overflow, replay, privacy | validated definition/profile policy plus per-instance runtime state |
| source event handlers | handler-source storage and special callable subset | ordinary typed Stream transformations/consumers or observation policy |
| `SourceRuntimeState` | queue, closed, error, overflow count | generation-owned Stream instance, sequence cursor, queue, and explicit terminal state |
| `SourceEventKind::Item` | item ingress | instance-keyed ordered Stream host event |
| source error/progress/disconnect/revocation/end | partly untyped lifecycle | closed typed lifecycle with domain errors separated from host ABI failures |
| `RuntimeStepInput.source_events` | second host event ingress | sole typed `stream_events` ingress |
| source runtime effects | open/close/observation | typed Stream requests and privacy-safe observations |
| source engine | second queue/step owner | one Stream instance/scheduler owner |
| AWBC Source records/opcodes | second executable and wire path | one Stream table/state/opcode family |
| Source save/hot-swap rows | second persistence identity | Stream instance/fiber validation and external-live save blockers |

## Defects that must not be copied into the final Stream model

- `StreamRuntimeState::close` cleared its queue and could discard items emitted
  before terminal observation.
- retained Stream events reused `SourceEventKind`, so the retained abstraction
  did not own its own lifecycle contract.
- the AWBC stream transform lowerer linearized mutually exclusive `if` and
  `match` paths.
- current `ForNext` lowering bound a queue target instead of implementing
  suspending ordered iteration.

The final migration needs direct behavioral, codec, tamper, persistence, and
host tests for these invariants. Zero-hit source scans prescribed by the
package are superseded by the repository source-gate prohibition.

## Original surface conflict and resolution

The package simultaneously required an exact external `stream fn` spelling
that was absent from its source request and rejected ordinary
`fn -> Stream<T, E>`. Lang-01.1.1, which it claimed to inherit, had already
removed `stream fn` and selected the ordinary-function substrate.

The reconciled direction is:

- external origins are ordinary members of `extern capability`;
- those members use ordinary bodyless `fn` syntax returning `Stream<T, E>`;
- capability and operation identity provide the stable external definition;
- calling the member returns an affine Stream handle and emits a typed open
  request;
- an authored ordinary `fn -> Stream<T, E>` with own-scope `yield` is a
  generator;
- one without own-scope `yield` is an immediate Stream passthrough; and
- no source-role keyword or role attribute is added.

This surface decision is complete. The exact Stream definition/instance,
origin, policy, lifecycle, AWBC, save, and host-wire types remain blocked on
the returned Lang-01.3.1.2.1 correction.

## Required implementation order

1. Finish the Lang-01.1.1 ordinary-function/generator substrate.
2. Apply the accepted Lang-01.3.1.1 callable surface.
3. Obtain and apply the corrected Lang-01.3.1.2.1 core/AWBC/save contract.
4. Migrate syntax, HIR/sema/callable catalog, then RuntimePlan/core.
5. Perform the AWBC/data-format cut atomically before publishing a state with
   both Source and Stream paths.
6. Integrate Lang-01.2 through the shared callable catalog.
7. Integrate optional singleton declarations only through the final typed
   resource owner.
8. Integrate provider/profile selection through Lang-01.5's single manifest
   decoder.

## Completion boundary

This Cut 0 is an intake and design-reconciliation record, not production
completion. No parser recognizer, removed-syntax diagnostic, compatibility
surface, source gate, Rust production change, schema change, or fixture
migration is claimed here.
