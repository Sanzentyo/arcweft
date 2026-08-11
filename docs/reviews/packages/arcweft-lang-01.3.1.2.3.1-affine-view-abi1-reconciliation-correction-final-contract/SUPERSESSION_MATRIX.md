# Supersession matrix

## Affine parent corrections

| Parent location | Final correction |
|---|---|
| all `ABI 2` / `AWBC_ABI_VERSION = 2` statements | `AWBC_ABI_VERSION = 1`; ownership semantics replaced in place |
| `AWBC_ABI2_OWNERSHIP_CONTRACT.md` | superseded by `ABI1_OWNERSHIP_WIRE.md` |
| restore entrypoints on `RuntimeDriver` | deleted; activation is owned by `RuntimeExecutionDomain` |
| snapshot copy claim “only one candidate can acquire exclusive driver replacement” | replaced by domain-wide active lease rule |
| `RuntimeSnapshotImageV2` without allocator cursor | corrected exact schema includes allocator snapshot |
| restore allocator continuation unspecified | exact persisted cursor installed |
| `RuntimePreparedDrop::commit(self, value, domains)` | deleted; prepared drop owns exact value and transaction commits it |
| `RuntimeValueSnapshotV2: Eq` in snapshot prose | removed; exact type is `PartialEq` only |
| tamper order “ABI 2” | “ABI 1” |
| implementation step labels “protected ABI2” | “protected ownership-complete ABI1” |

All other generic ownership, capture, sequence, payload, plan constant, Stream, host/replay, and parent-test decisions remain retained.

## View parent corrections

| Parent location | Final correction |
|---|---|
| defaults may return any accepted ordinary value | any accepted ordinary **Unrestricted** value |
| `ViewValueInputBinding` without transfer/ownership | corrected exact fields added |
| generic retained View runtime values may be affine | rejected in current cut |
| AWBC ABI 1 “no ownership change” | ABI 1 retained but ownership-complete semantics required |
| save says root/mount live `RuntimeBinding`s carry generic values | dormant whole-execution snapshots only |
| transcript lacks static requirements | `static_requirements` added before fragments |
| `AuthoredRequired` certificate without independent requirement authority | exact requirement digest required |
| `STA-013` non-overlap by identity only | strict span containment and outermost dispatch |
| `STA-057` unimplementable evidence | made implementable by requirement rows |

All other final-HIR View catalog, resource/image, dynamic execution, parameter coordinate, export, work limit, runtime parity, and deletion decisions remain retained.
