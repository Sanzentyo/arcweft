# Top-level declaration reduction

Date: 2026-07-16

Production baseline: `1aa5ad6d395e` (`main`)

## Decision

Arcweft will reduce its author-facing top-level declaration vocabulary. A
dedicated declaration remains justified only when it owns at least one of the
following boundaries:

- an execution model that cannot be expressed as an ordinary function body;
- a stable global identity consumed by packages, runtime, save/replay, or
  Agent tooling;
- a security, trust, launch, or host-capability boundary;
- a body grammar that is materially different from an ordinary expression;
- manifest discovery that must occur independently of ordinary symbol use.

Function roles are not separate declaration families merely because they have
a special return type or policy. Resource kinds are types rather than an
ever-growing list of lexer keywords. Build/profile choices belong to typed
manifests unless source ownership is demonstrably required.

This decision supersedes stable-language documentation and package acceptance
items that require provisional top-level families solely because they already
exist in syntax, HIR, or fixtures. It does not by itself delete typed runtime
substrates such as `StreamPlan` or `SourcePlan`.

## Production evidence at the baseline

The current syntax `Item` and HIR declaration inventories still include
`Hook`, `MemoFn`, `Parser`, `Callable`, `State`, `DialogueDefaults`, and
`Source`. The corresponding canonical grammar still advertises those forms.

Three families are clear scaffolds and may be removed without a replacement
author syntax:

- `hook` stores target and phase as strings and never lowers into the normal
  runtime plan;
- `memo fn` stores its signature as a string and never lowers into a memo
  runtime plan;
- `parser` stores its signature tail as a string and never lowers into an
  executable parser program.

Their final removal must delete the classifier, parser, typed AST, HIR,
semantic traversal, symbol/tooling branches, and spelling-specific tests. The
ordinary current grammar recovery path is the only final rejection behavior;
there will be no compatibility parser, removed-syntax diagnostic, or source
gate.

Other reductions are migration-gated because production contains meaningful
substrate or because the replacement binding is not yet fixed:

- `stream fn` lowers into `StreamPlan` and AWBC stream tables;
- `source` lowers into `SourcePlan`, AWBC source tables, runtime stepping, and
  snapshot state;
- root `state` and `reducer` need one typed entry/manifest binding before their
  dedicated declarations can disappear;
- Agent controllers have dedicated compilation/runtime treatment even though
  their source shape overlaps ordinary functions;
- dialogue defaults, content roots, concrete extern modules, and concrete
  Activity implementations need a typed manifest/profile destination;
- image/audio/motion/rig declarations need one typed resource surface before
  their individual keywords are removed.

## Proof-concurrency impact

The proof-concurrency Stage 1 item-dispatch cut remains useful parser
infrastructure, but its then-current `SyntaxKind` inventory is not a final
language contract. Further Stage 1 work must use the reduced vocabulary. As
families are removed, their CST kinds and exhaustive inventory cases must be
deleted rather than preserved as historical syntax.

## Independent design requests

The unresolved decisions are intentionally split so they can be designed in
parallel without stopping the scaffold-removal cut:

- [function suspension roles](../reviews/requests/2026-07-16-lang-01.1-function-suspension-role-unification-final-contract.md)
- [state, reducer, and Agent entry binding](../reviews/requests/2026-07-16-lang-01.2-state-reducer-agent-entry-binding-final-contract.md)
- [live source authoring](../reviews/requests/2026-07-16-lang-01.3-live-source-authoring-unification-final-contract.md)
- [typed resource declaration surface](../reviews/requests/2026-07-16-lang-01.4-typed-resource-declaration-surface-final-contract.md)
- [build/profile metadata extraction](../reviews/requests/2026-07-16-lang-01.5-build-profile-metadata-extraction-final-contract.md)

Later production review replaced two returned author/runtime decisions with
independent correction requests. These corrections are authoritative wherever
they conflict with the earlier Lang-01.1 and Lang-01.3 requests or packages:

- [direct-style suspension and Stream generator correction](../reviews/requests/2026-07-17-lang-01.1.1-direct-style-suspension-generator-contract-correction.md):
  ordinary `fn` may suspend through `await` and `control.suspend`; do not add
  `#[task]` or `#[stream]`;
- [external Stream origin and Source elimination correction](../reviews/requests/2026-07-17-lang-01.3.1-external-stream-origin-source-elimination-contract-correction.md):
  remove source-visible `Source<T, E>` and represent host input as a typed
  external `Stream` origin while preserving policy and lifecycle behavior.

The earlier requests remain historical sequence inputs, not implementation
authority for the superseded decisions. Neither correction authorizes an
implementation until its own implementation-ready final contract is returned.

Lang-01.6 is now
[resolved](../reviews/requests/2026-07-16-lang-01.6-trusted-axiom-surface-final-decision.md):
the trust boundary remains visible as `#[verify.trusted(...)]` metadata on an
ordinary proof, while the separate `trusted axiom` declaration and
`TrustedAxiomItem` are deleted. Proof-concurrency can therefore complete its
typed proof migration without preserving the obsolete declaration family.

## Implementation order

1. Remove `hook`, `memo fn`, and `parser` end to end.
2. Update the canonical grammar and maintained examples to stop advertising
   those forms.
3. Apply each returned final contract directly, without dual readers or
   transitional aliases.
4. Remove regular-project top-level statements after fixtures have explicit
   flow/function owners.
5. Resume proof Stage 1 against the reduced declaration inventory.

## Non-goals of the first cut

- deleting internal stream/source runtime plans;
- inventing an `async fn`, resource abbreviation, or source attribute before
  its contract is returned;
- moving dialogue/profile/build metadata to an untyped string manifest;
- retaining old spellings for migration.

## Structural audit

The decision/request cut ran the canonical structural audit against the exact
baseline checkout:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/top-level-declaration-reduction-2026-07-16
```

It scanned 2,971 files, including 1,459 Rust files and 681,661 physical Rust
LOC across 90 package manifests. It reported 0 errors and 129 existing
repository-wide warnings. This documentation-only cut changes no Rust file,
crate dependency, public Rust API, or generated format.
