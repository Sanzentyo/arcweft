# Proof concurrency v6.1.1.2 retained global-identity implementation

## Accepted package

The implementation source of truth is
`arcweft-proof-concurrency-v6.1.1.2-retained-global-identity-declaration-grammar-reconciliation-final-contract.zip`.
Its SHA-256 is
`7be398ebe2cefa2daefa963c7c8c6efb0b2389bb015edf36e585fb8b770242b1`.
All 18 archive members match `MANIFEST.txt`, the entries are in lexical order,
and the manifest self-entry uses the required 64-zero rule.

Intake started from local Git parent
`6b97057a0a430179175682494e07c7529554933b` and Jujutsu working-copy change
`xpzvlyvqvtvowssyxlpswsnpkwnspxqr`. The package itself was designed against
Git `27227bbc8e1d5c78d7b35c2865bad8fb6d00fca9`; implementation reconciles the
contract with newer accepted resource, callable, Character, View, and project
source-index work rather than restoring an older shape.

## Completion contract

The package contains 184 normative direct-test rows. Completion requires all
of the following:

1. retain `asset` as a catalog/reference family with no authored declaration;
2. implement private one-pass typed grammar rows for `character`, `view`,
   `action`, `activity`, `signal`, `metric`, and `layer`;
3. close their success, lossless, malformed, recovery, ambiguity, and inclusive
   budget tests before changing the public AST;
4. atomically replace the generic/stringly public entity declaration path with
   seven attached typed declarations;
5. lower attached declarations into arena-owned HIR and the single project
   symbol authority without cloning or reparsing source strings;
6. migrate Character, View callable, Action, Activity, Signal, Metric, Layer,
   formatter, LSP, CLI, Agent, runtime-plan, bundle, and manifest consumers;
7. delete the generic entity AST/HIR, raw signature/body storage, cloned View
   callable projection, and all removed-family readers; and
8. pass focused tests, stable-feature workspace check and strict Clippy,
   formatter, workspace suite, structural audit, and affected Tier 2 tests.

`res` remains the independent configured-resource declaration. No generic
`entity` declaration, authored `asset`, compatibility reader, removed-spelling
diagnostic, CSS/Takumi route, or source gate is permitted.

## Ordered implementation cuts

| Cut | Scope | Status |
| --- | --- | --- |
| 0 | package integrity and production reconciliation | complete |
| 1 | owned identity vocabulary, shared private header nodes/roles/limits, classification inventory | complete |
| 2 | private Character and Action grammar plus direct tests | complete |
| 3 | private Signal and Metric grammar plus direct tests | complete |
| 4 | private Activity grammar plus direct tests | complete |
| 5 | private Layer grammar plus direct tests | complete |
| 6 | private View grammar integration with typed common expression descendants | complete |
| 7 | complete reduced Stage 1 declaration inventory gate | complete |
| 8 | atomic attached public AST switch and generic entity deletion | pending |
| 9 | typed HIR/project-symbol and downstream migration | pending |
| 10 | docs/examples/fixtures and obsolete-path deletion | pending |
| 11 | full validation, structural audit, Tier 2, commit/push cleanup | pending |

## Current evidence

The complete seven-row private inventory now emits directly through
`ShadowDocumentParser` and `GrammarBudget`. Character and Action own typed
headers/body or signatures; Signal owns a typed common Type child whose closed
observable head/arity is intentionally deferred to sema; Metric owns typed
kind, value type, unit, labels, and buckets; Activity owns abstract policies,
ports, and contracts; Layer owns typed singleton members, family-checked
references, and closed policy values; and View owns one fixed signature, a
leading export block, and a typed common-expression fragment. The private
classifier maps removed
`asset`, `content`, `extern mod`, `dialogue defaults`, `source`, `state`, and
regular top-level statements to ordinary `ErrorItem` recovery.

Direct tests cover canonical and malformed rows, all seven shared-header
missing/wrong-family/relative-ID/keyword-name cases, sibling preservation,
prefix attachment, LF/CRLF/Unicode losslessness, mixed documents, and every
new narrow inclusive budget. The Stage 1 close-out also directly exercises the
inclusive global limits for 16,384 top-level items, 1,048,576 identity-bearing
nodes, and 1,024 diagnostics, including one-over exhaustion and fresh-budget
recovery. Duplicate declarations and sections now retain exact first and
duplicate ranges; malformed Action and View signatures retain exact recovery
ranges; View values retain typed common-expression descendants or an
`ErrorExpression`; and dotted namespace calls at the top level recover as
ordinary `ErrorItem` nodes rather than being misclassified as declarations.

`cargo test -p arcweft-id` passed 6 tests, and the latest
`cargo test -p arcweft-lang-syntax --lib` passed all 373 tests. The final
Activity header recovery uses the generic current-grammar
`syntax.declaration.unexpected_header` diagnostic. The temporary
concrete-origin spelling recognizer, its dedicated diagnostic code, and its
spelling-specific test have been removed as required by the repository-wide
removed-syntax policy. The Stage 1 close-out reran the all-targets strict
Clippy gate for `arcweft-id` and `arcweft-lang-syntax` with `-D warnings`
successfully. The repository structural audit scanned 3,404 files and reported
0 errors and 131 warnings. No public syntax reader has been switched,
preserving exactly one public reader until the atomic public AST cut.

Validation results will be recorded here as cuts close. Passing compilation
alone does not complete this package.
