# Non-Goals and Explicit Prohibitions

## 1. Out of scope for this request

This contract does not redesign:

- Lang-01.1.1.2.1 contextual `Ref<Entity>` projection;
- syntax, HIR, or project nominal declaration ownership;
- the implemented source-backed project nominal resolver;
- alias expansion semantics;
- callable query budget or poison accounting;
- character nominal admission;
- runtime-plan or verifier type ownership;
- Rust function generic binders;
- Rust method export ABI;
- public language import syntax;
- external module purity/effect validation;
- the registered-world transaction shape beyond the publication boundary;
- AWBO object fields or schema version.

## 2. No second nominal resolver

Adapter/Rust publication does not parse or resolve a source spelling. It carries an exact accepted owner/path candidate and validates that candidate against `AcceptedNominalWorld`.

The following are not introduced:

- an adapter nominal resolver;
- a Rust package suffix resolver;
- a terminal-name index;
- a source-label parser;
- a lookup that tries multiple owners;
- a lookup whose result depends on registration insertion order.

## 3. No string semantic identity

No accepted Rust or adapter-native export may be represented by:

- `ArcweftRustTypeRef::Named`;
- `AdapterTypeKind::Named`;
- `TypeKind::Named`;
- `HashSet<String>` package export membership;
- a formatted `rust_path`;
- a display label;
- a terminal type name.

`TypeKind::Named` remains available only for the repository’s existing internal/host-produced semantics outside this publication path. This contract does not globally delete that variant.

## 4. No compatibility surface

Because the affected contracts are unpublished, the implementation adds none of:

- dual readers;
- deprecated constructors;
- aliases;
- migration shims;
- old-spelling-specific diagnostics;
- compatibility traits;
- feature flags selecting old/new behavior;
- a schema/version bump.

The final shape replaces the old shape in place.

## 5. No partial admission

A publication error does not:

- omit the failing callable;
- omit the failing overload;
- replace the failing node with an unchecked type;
- publish primitives while skipping nominals;
- reuse a record from the previous world;
- admit metadata while rejecting callables, or vice versa;
- emit a persistent digest for a failed candidate world.

## 6. No dependency inversion

`arcweft-lang-sema` does not depend on adapter-context or Rust ABI wire types. Adapter-context constructs sema-owned neutral inputs under its optional sema dependency.

No callback trait, extension trait, dynamic resolver object, global registry, or I/O service is added to sema.

## 7. No ad hoc behavior around owned enums

Behavior required on arcweft-owned types is implemented on the original type:

- accepted instantiation on `AcceptedNominalRecord`;
- exact accepted lookup/stamp/projection on `AcceptedNominalWorld`;
- generic-owner variant on `GenericTypeOwnerId`;
- semantic digest on `TypeKind`;
- schema digest on `CallableSignatureSchema`;
- enum payload substitution on the existing enum payload type.

An implementation must not add an extension trait or one-off helper match to avoid changing the original owned enum/impl.

## 8. No source-text evidence tests

Tests do not prove behavior by scanning source files for spellings, counting implementation symbols, or checking a path to a private helper. They use typed public or crate-owned APIs and observable semantic outcomes.

Generated source documents are tested through their typed source maps and source spans, not by searching text for a type name.

## 9. No unbounded work

The correction does not add an unbounded recursive walk, diagnostic vector, mount table, metadata catalog, or publication cache. Existing nominal, aggregation, catalog, and callable limits remain authoritative.

## 10. No production code in this artifact

This ZIP contains a contract, evidence, a test matrix, and an artifact validator. It contains no patch, modified repository source file, or replacement production module.
