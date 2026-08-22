# Crate and type dependency matrix

## 1. Final crate edges

| From | To | Allowed | Purpose |
|---|---|---:|---|
| `arcweft-core` | `arcweft-id`, `arcweft-source`, lower runtime/data crates | yes | existing Sans-I/O core semantics |
| `arcweft-core` | `arcweft-view` | **no** | forbidden reverse dependency |
| `arcweft-core` | `arcweft-bundle` | **no** | forbidden reverse dependency |
| `arcweft-runtime-plan` | `arcweft-core` | yes | lower checked products into core plan builder |
| `arcweft-runtime-plan` | sema/compiler products through their accepted API | yes | function/endpoint/effect/task seeds |
| `arcweft-bundle` | `arcweft-core` | yes | decode private core image and supply authority |
| `arcweft-bundle` | `arcweft-view` | yes | own actual validated View product |
| `arcweft-view` | `arcweft-core` | only existing legitimate runtime-facing types, not required by this join | no core reverse edge |
| compiler/sema | `arcweft-view` identity products | yes at accepted compiler layer | Cut 3 actual program/site/admission |
| scheduler/runtime hosts | sealed core plan APIs | yes | consume immutable task-plan keys |

The current inspected `arcweft-core/Cargo.toml` has no `arcweft-view`
dependency. The current `arcweft-bundle/Cargo.toml` depends on both core and
View and is therefore the legitimate production join layer.

## 2. Cross-layer protocol surface

Core exports only:

- opaque completed digest types with read-only byte access;
- owner-bound `RuntimeTaskPlanBuildCoordinate` values minted by builder/decode;
- field-private non-Clone base/request values minted by the encoder;
- typed read-only base getters; and
- `ViewTaskPlanAuthority::task_plan_semantic_digest`.

Core does not export:

- `ViewProgramIdProjection`;
- `ViewMatchSiteIdProjection`;
- `CheckedViewMatchAdmissionDigestProjection`;
- a raw View binding DTO;
- a general transcript sink/visitor callback;
- a public final-digest byte constructor; or
- an extension trait implemented on a core enum.

## 3. Upper implementation ownership

`arcweft-bundle::resource_codec::view::validated` evolves the current
`ValidatedViewProgramResource` in place. It can name both:

- core types (`RuntimeTaskPlanBuildCoordinate`, opaque request/base getters,
  `TaskPlanSemanticDigest` return); and
- actual View types (`ViewProgramId`, accepted revision, stable site, checked
  admission).

The binding is not moved into a new utility crate, compiler side table, or
runtime scheduler. The existing validated program resource is the production
upper owner.

## 4. Build/decode dependency proof

### Ordinary builder

```text
runtime-plan -> core builder -> core encoder -> core seal -> RuntimePlan
```

No View crate or registry participates.

### View builder

```text
compiler Cut 3 ----\
                     -> bundle validated View resource
runtime-plan coords -/               |
                                     v
runtime-plan/core builder -> core opaque request -> authority -> typed digest
```

### Bundle decode

```text
bundle decoder -> private core plan image -> coordinate tokens
bundle decoder -> actual View image + tokens -> validated View authority
private core plan image + authority -> common core seal -> RuntimePlan
outer bundle -> atomic publication
```

## 5. Cargo metadata structural gate

Implementation acceptance runs:

```bash
cargo metadata --format-version 1 --no-deps
```

The gate parses JSON package/dependency identities. It rejects any direct
`arcweft-core -> arcweft-view` or `arcweft-core -> arcweft-bundle` edge and
requires `arcweft-bundle` to retain direct core and View dependencies. This is a
structured graph check, not source-text spelling inspection.
