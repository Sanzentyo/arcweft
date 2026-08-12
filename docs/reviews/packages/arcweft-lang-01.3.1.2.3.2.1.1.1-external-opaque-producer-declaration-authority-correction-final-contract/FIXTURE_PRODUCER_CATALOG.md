# Fixture producer catalog and selection rule

## Rule

Every maintained fixture authors an explicit reviewed literal of the form:

```text
fixture.<owning-crate-or-suite>.<semantic-domain-or-case>
```

The value is stable fixture data, not generated from a Rust type name, accepted
path, package ID, file path, test function, hash, or schema. Fixtures that test
shared-domain behavior intentionally repeat one literal. Fixtures that are not
about producer equality use distinct semantic-domain literals to prevent
accidental coupling. Negative fixtures contain the exact invalid input under
test.

## Existing macro pass fixtures

| Rust item | Explicit producer |
|---|---|
| `PlayerScore` | `fixture.rust-abi.player-score` |
| `Rank` | `fixture.rust-abi.rank` |
| `Pair<T, U>` | `fixture.rust-abi.pair` |

Existing type-related compile-fail fixtures whose intended error is lifetime,
const generic, or reference field must gain a valid producer so that the old
intended diagnostic remains first. Function-only export failure fixtures do not
need a type producer.

## Standard production adapter constants

| Declarations | Explicit producer |
|---|---|
| native HTTP `HttpRequestContext` | `arcweft.adapter.native-http` |
| inference tensor `Conv2dApi`, `InferApi`, `TensorF32` | `arcweft.adapter.inference-tensor` |

These are reviewed production constants. Their spelling is not formatted from
adapter IDs or nominal paths. Multiple inference identities intentionally share
one domain.

## Common fixture domains

- adapter codec JSON/TOML: `fixture.adapter-codec.shared`
- adapter-sema native: `fixture.adapter-sema.native`
- adapter-sema Rust export: `fixture.adapter-sema.rust`
- lang-sema instantiation: `fixture.lang-sema.instantiate`
- lang-sema substitution: `fixture.lang-sema.substitute`
- loader/compiler/LSP: `fixture.project.external-types`
- desktop Rust exports: an explicit product-owned literal such as
  `arcweft.desktop.runtime`, reviewed with the owning adapter; never a crate-name
  derivation performed in code.
