# Accepted catalog and publication contract

## 1. Publication transaction

`AdapterSemanticRegistration::source_backed_facts` remains the top adapter-sema
transaction. Before emitting any accepted inventory row, it renders source,
builds the producer payload source map, and projects every adapter-native and
Rust-export producer into core. All producer projections are precomputed in
semantic authored order. Any error aborts the entire facts result; no catalog,
external row, digest, or partial source-backed environment is published.

## 2. Source kind and source attachment

Adapter-native rows use `ExternalOpaqueProducerSourceKind::AdapterNominal` and
the producer payload range in their `nominal` generated-source line. Rust rows
use `RustExport` and their `rust-type` line. The source span contains only the
exact UTF-8 producer bytes, excluding `opaque-producer=`, the decimal byte
length, and the colon. This makes diagnostics select user-relevant evidence
without reconstructing source text.

The source map owns:

```rust
opaque_producers: BTreeMap<EnvironmentPublicationItemId, SourceRange>
```

and inherent methods:

```rust
fn insert_opaque_producer(
    &mut self,
    item: EnvironmentPublicationItemId,
    range: SourceRange,
) -> Result<(), AdapterRegistrationFactsError>;

fn opaque_producer_range(
    &self,
    item: &EnvironmentPublicationItemId,
) -> Result<SourceRange, AdapterRegistrationFactsError>;
```

Duplicate/missing map entries are typed invariant errors and abort publication.

## 3. Projection algorithm

For either lower-layer ID:

1. clone its exact string into `RuntimeOpaqueTypeProducerId::try_new`;
2. on core spelling failure, return `InvalidOpaqueProducer` with source kind,
   exact string, payload span, and `RuntimeIdentityError` source;
3. if the exact string starts with `std.`, return
   `ReservedOpaqueProducer` with the same evidence;
4. return the typed core ID.

The lower owners normally make steps 2 and 3 unreachable. They remain at the
cross-layer boundary so a future decoder/constructor defect fails closed.

## 4. Mandatory accepted evidence

`AcceptedNominalInventoryInput::runtime_producer` is populated for every
adapter-native and Rust-export type row. The registrar calls:

```rust
AcceptedNominalRecord::try_new_opaque(
    nominal.id().clone(),
    nominal.arity(),
    nominal.runtime_producer().clone(),
    nominal.origin().clone(),
    nominal.source().clone(),
)
```

There is no producerless `AcceptedNominalSemantics::Opaque` variant. There is
no successful `TypeKind::Named` runtime projection for these rows.

## 5. Instantiation and substitution

Accepted nominal instantiation copies the record producer into
`AcceptedNominalType::new`. Generic substitution rebuilds only arguments and
preserves the exact producer:

```rust
AcceptedNominalType::new(
    nominal.declaration().clone(),
    substituted_arguments,
    nominal.runtime_producer().clone(),
)
```

A producer is a domain owner, not a function of arguments. Generic
instantiations under one declaration therefore share one producer but retain
distinct semantic identities through declaration plus normalized arguments.

## 6. Admission projection

Every external accepted row projects to:

```rust
RuntimeOpaqueTypeOwner::exact(
    accepted.runtime_producer().clone(),
    accepted.semantic_identity(),
)
```

External data cannot select `ProducerWide`. The fixed CharacterDialogue top
continues to call the parent package's dedicated producer-wide constructor.

## 7. Collision and accounting

Nominal identity duplicate detection, capacities, and work accounting run only
after all producer-required-field/spelling/reservation/model/package checks.
Producer equality is never a duplicate condition and has no independent
capacity/index. Atomic catalog publication remains the final step.
