# Repository-aware evidence

## Pinned repository state

- Repository: `Sanzentyo/arcweft` (private; read through the GitHub connector)
- Branch resolved: `main`
- Commit: `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`
- Root `AGENTS.md` blob: `e91f99213dde67953beda6aa078c370a8dc4541d`
- Request blob in that commit: `6d24910f7961c56faaffddea5cfa6775b48578a1`
- Uploaded request Git blob: `6d24910f7961c56faaffddea5cfa6775b48578a1`

The uploaded request is byte-identical to the request committed on the pinned
`main` revision. During package construction `main` advanced by two commits from
`126f7ece0f69062f1cbea3e753cd04af5ead2056`; the final baseline was repinned to
`5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`. The intervening diff changed
AGENTS policy, nominal-type publication, parser/HIR/sema/runtime-plan surfaces,
and package-intake documentation, but did not change core value, AWBC, fiber,
swap, save, step, callable-coordinate, or callable-limit owners used by this
contract. Relevant changed files were reread at the final revision.

## Production observations that constrain this contract

| Repository path | Blob | Observed constraint |
| --- | --- | --- |
| `AGENTS.md` | `e91f99213dde67953beda6aa078c370a8dc4541d` | Typed ownership, deterministic behavior, Sans-I/O core/data crates, no compatibility shims for unreleased formats, and behavior on owning enums. |
| `crates/arcweft-lang-sema/src/callable/arguments.rs` | `3e298b76c734bcc2ef3b4d19389d3503a5d7e899` | The shared resolver's parameter identity is `(group, parameter)` and the argument mapper already owns positional, named, default, and rest mapping. |
| `crates/arcweft-lang-sema/src/callable/facts.rs` | inspected at pinned commit | Accepted call facts retain selected declaration, current/next group, slot mapping, and `CallableParameterCoordinate`; RuntimePlan must consume these facts directly. |
| `crates/arcweft-lang-sema/src/callable/schema.rs` | inspected at pinned commit | Signatures are nested parameter groups with passing and presence metadata. |
| `crates/arcweft-lang-sema/src/callable/identity.rs` | `484dc6ad790a1a194cc91293700799024adc411b` | Group and parameter indices are checked `u16` newtypes. |
| `crates/arcweft-lang-sema/src/callable/limits.rs` | `7f81748e6de6c6fca41b6c8d81272042e82cdb2e` | Production limits are 16 groups and 128 total parameters per callable. |
| `crates/arcweft-core/src/value.rs` | `25ee59e63f9354d357d283f067ab1123804b0d89` | Current `RuntimeFunctionValue` stores a flat remaining-parameter vector and flat captures. Its current partial application cannot retain callable group coordinates. |
| `crates/arcweft-core/src/awbc/vm.rs` | `d58ed848c966da827cfdd6c002df3adf113175ef` | Current AWBC `ApplyFunction` performs arity-only flat partial application. It cannot implement external Stream group validation without a typed owning variant. |
| `crates/arcweft-core/src/awbc/schema.rs` | `01c0d41efb396db7292b9104b30a035441ca4372` | Current main is AWBC ABI 1 / codec 7; frame signatures are flat; closure opcodes end at `0x26`; terminators end at `0x8e`. |
| `crates/arcweft-core/src/awbc/codec.rs` | `e7eac3360909457abd7a60652efb70f7ff693532` | The codec is strict, budgeted, rejects unknown tags, and currently accepts only codec 7. |
| `crates/arcweft-core/src/awbc/fiber.rs` | `5f46f3fc91fce24b0a9b58b8fa26c62b15dd0570` | Fiber state owns generation and register `RuntimeValue`s; exact resume cursors exist specifically to prevent replay. |
| `crates/arcweft-runtime-driver/src/swap.rs` | `4538adbcb918aea083ef3e6ed518f11ca3b5d01a` | Hot reload has content-only, code-compatible, code-generational, and restart-required classes and can pin retired generations. |
| `crates/arcweft-runtime-driver/src/session_save.rs` | `0048e65ec5e15191da46afbd3c53de235b6aa287` | Current save schema is 1; Product AWBC snapshots recursively validate `RuntimeValue`s and generation ownership. |
| `crates/arcweft-core/src/step.rs` | `92a34b177b7474ccaafaaa8922b6d332fb535c95` | Host requests are Sans-I/O data; the existing large-integer JSON policy serializes wide integers as decimal strings. |
| `crates/arcweft-runtime-plan/src/function_values.rs` | `862527458bdfdfb817cb7bd12f6ea706a3bf7b5c` | Current ordinary curried functions are still represented as nested function expressions; this does not supply external Stream coordinate ownership. |
| `crates/arcweft-runtime-plan/Cargo.toml` | `0da1f5555668fd3868ee758117972bc11472bbe5` | RuntimePlan production code does not depend on sema. |
| `crates/arcweft-compiler/Cargo.toml` | `49ac0ac6fc4ad949e33f35e5e3d15dac098e3a11` | The compiler is the valid boundary that can consume sema facts and emit core/RuntimePlan types. |
| `crates/arcweft-lang-hir/src/symbol/identity.rs` | `6ac5696f8f5c1296dd64f7fcdac7d048b3c7227f` | Callable declarations already have typed package/module/owner/owner-path/name identity, including `ExternCapability`. |

## Consequences

1. Currying remains legal because Lang-01.3.1.1 requires ordinary external
   `fn -> Stream<T, E>` callables to use the shared callable semantics.
2. Core owns a language-free runtime coordinate counterpart; the compiler performs
   one checked projection from sema. RuntimePlan does not add a sema dependency.
3. The owning `RuntimeFunctionValue` enum receives the external Stream partial
   variant. A sidecar map, extension trait, endpoint helper, or name rule would
   violate the repository's ownership policy.
4. ABI 2 / codec 8 / save schema 2 are one atomic unreleased-format replacement.
   Codec 7 and Source-shaped runtime layouts are not accepted by the new reader.
5. The correction is a design artifact only. No production repository file was
   modified by this package.
