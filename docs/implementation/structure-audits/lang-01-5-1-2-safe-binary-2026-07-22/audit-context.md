# Lang-01.5.1.2 safe binary topology audit context

Date: 2026-07-22  
Parent revision: `bd08d1fa7b31`  
Scope: exact binary topology resources, Character package payload admission,
and source-map projections retained by the sole manifest decode.

## Changed production hotspots

| Path | Owner | Bytes | Physical LOC | Role |
|---|---|---:|---:|---|
| `crates/arcweft-project/src/content.rs` | `arcweft-project` | 18,948 | 569 | Typed binary resources and canonical topology-revision transcript |
| `crates/arcweft-character/src/package.rs` | `arcweft-character` | 11,278 | 321 | Exact package bytes plus complete PNG/membership/dimension admission |
| `crates/arcweft-launch/src/accepted.rs` | `arcweft-launch` | 24,735 | 741 | Source-bound content/profile projections from the sole manifest decode |
| `crates/arcweft-project-loader/src/topology/model.rs` | `arcweft-project-loader` | 34,526 | 1,090 | Typed topology input, retained payload, package, and watch models |
| `crates/arcweft-project-loader/src/topology/loader.rs` | `arcweft-project-loader` | 49,533 | 1,250 | Atomic topology acquisition and publication transaction |

Workspace dependency fan-in/fan-out is `4/8` for `arcweft-project`, `12/7`
for `arcweft-character`, `4/9` for `arcweft-launch`, and `2/20` for
`arcweft-project-loader`. The new dependencies follow the existing direction:
the I/O-owning project loader consumes Sans-I/O project/Character/launch
models, and none of those lower crates depend on the loader.

`topology/loader.rs` is 50 LOC above the production warning threshold. Its
reviewed responsibility remains one atomic topology transaction: it binds
workspace/dependency ownership, enforces one shared budget, acquires disjoint
text/binary payloads, and publishes only after every claim is validated. The
binary path deliberately shares the same claim table and budget rather than
introducing a second loader or publication path. It is below the error
threshold and did not grow by the 300 LOC structural trigger. A future split
should move a cohesive acquisition context with its shared state, not expose
the builder fields or duplicate claim logic merely to reduce line count.

## Audit result

The canonical audit reports one repository-wide error at
`crates/arcweft-lang-sema/src/checker/module.rs` (2,523 LOC). That file is
unchanged by this slice; its decomposition remains a separate active cleanup
item. The generated reports record 137 warnings, including the reviewed
`topology/loader.rs` threshold above. No new crate-layer inversion or duplicate
binary resource authority was identified.

