# REPOSITORY EVIDENCE

## Inspection basis

| Item | Evidence |
|---|---|
| Repository | `Sanzentyo/arcweft` |
| Default branch | `main` |
| Inspected revision | `a8403dcb26d78e6cafee3576d5933e9952d8305b` — `Parse modules and imports in the shadow grammar` |
| Callable substrate revision | `f420ee8fbf244351e11fd5f793b07e7cdd3f1b6a` — `Implement callable catalog shared resolver substrate` |
| Revision comparison | Inspected revision is one commit ahead; intervening files are syntax module/use grammar and audit docs only, with no callable/sema changes |
| Root policy | `AGENTS.md` at blob `c41ff4d2b3baadda3e9f975c7de3e5a6678f8758`, read in full |
| User Rust skill | `/mnt/data/Rust Skill.txt`, SHA-256 `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`, read in full |
| Task request | SHA-256 `fada8baca5a145aea1597385b609aee199be0b7122c485706e949380ff23d621`, read in full |

## Relevant files at inspected revision

| Path | Blob SHA | Evidence used |
|---|---|---|
| `crates/arcweft-lang-sema/src/callable/identity.rs` | `8e235cc0283b5114580307101a98d1f013888fbf` | Two-argument Curried constructor; wrapper-first then zero check; no schema field; candidate hierarchy |
| `crates/arcweft-lang-sema/src/callable/error.rs` | `21e24585b834ecf53776c2966723f7e74680f979` | Current `MissingGroup`; existing `InvalidCallGroup`; stable diagnostic mapping |
| `crates/arcweft-lang-sema/src/callable/resolver.rs` | `2a1bcbcc1c7db22040c4c3af281c1906b2534930` | Schema-owning resolved constructor; full schema storage; two currently accepted Curried representations |
| `crates/arcweft-lang-sema/src/callable/schema.rs` | `aa3cb08d34eca23cbbca9298de793d24c10c7b36` | Nonempty contiguous groups; group 0 Initial; later groups Curried; exact `group(index)` lookup |
| `crates/arcweft-lang-sema/src/callable/tests.rs` | `3c08096d1991ab4e8ac5fa9ab3285b1d46e84583` | Existing zero and Curried-wrapper tests; generic resolved failure; missing provider/positive/corrupt rows |
| `crates/arcweft-lang-sema/src/callable/catalog.rs` | `224a47fb6a318af590b39af371339bcb78a2a4d8` | Immutable records carry `Arc<CallableSignatureSchema>`; typed project/environment records |
| `crates/arcweft-lang-sema/src/callable.rs` | `2b2c5a9f6fbf41a7fbe9149159de86393a0775f0` | Public reexports; no alternate public curried product |
| `docs/implementation/2026-07-17-aw-ah-009-3-3-callable-catalog-cut-1.md` | `6873b52afc1bb4b9ec4fe59f49cd67db10c8ab9c` | Cut 1 preservation intent, schema-aware boundary, single current production resolver, historical validation |
| `docs/reviews/requests/2026-07-16-aw-ah-009.3.3.1-curried-callable-group-validation-contract-correction.md` | `b09947ba5d1ca320ea65b5caedf73f6a764879e6` | Checked-in copy matches the correction topic and required decisions |

## Call-site and representation searches

Repository-scoped code search at the inspected revision found:

- `CurriedCallableId::try_new` only in the request, Cut 1 implementation note, and callable tests; no production caller currently requires migration.
- `CallableIdentityError::MissingGroup` only in the identity implementation, request, and callable tests.
- `CallableInstantiation::Curried` only in the resolved product implementation, request, and callable tests.
- `ResolveCallError::InvalidCallGroup` has no current producer outside its definition/mapping.
- `ResolveCallOutcome` is currently substrate/reexport only; the shared production resolver migration has not yet created a second successful route.

These searches support a direct public error correction with compiler-guided call-site updates and no shim.

## Concrete source findings

### Context-free constructor

The constructor can inspect only `base` and `next_group`. It rejects `Curried`/`DataLast` bases and group zero, then stores the two values. It cannot prove project or environment schema membership.

### Existing schema-owning boundary

`ResolvedCallable::try_new` receives and stores `Arc<CallableSignatureSchema>`, so it already has the exact evidence needed to classify missing groups.

### Duplicate success arm

`instantiation_matches` currently accepts both a Curried wrapper pair and an unwrapped base ID with Curried instantiation. The latter is the concrete safety/contract flaw that justifies the one narrow substrate correction beyond prose/error naming.

### Existing typed resolver error

`ResolveCallError::InvalidCallGroup { candidate, group }` and `CallableDiagnosticCode::InvalidCallGroup` already exist and are mapped. No new resolver error or code is required.

## Historical validation evidence

The checked-in Cut 1 implementation note reports that, after its rebase, the following passed:

- `cargo fmt --all -- --check`;
- `cargo check -p arcweft-lang-sema --all-targets`;
- `cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings`;
- focused callable tests: 28 passed;
- canonical structural audit: 0 errors, 130 warnings.

These are repository-recorded historical results. They were not rerun for this design-only request.

## Verification performed for this archive

Performed now:

- full read of the attached request;
- full read of the user Rust skill;
- full read of current root `AGENTS.md`;
- GitHub connector inspection of current `main`, relevant blobs, commit history, and commit comparison;
- repository-scoped symbol/call-site searches;
- deterministic archive construction;
- member reopening and byte-for-byte comparison;
- sorted manifest digest/size verification;
- final ZIP SHA-256 verification.

Not performed now:

- no Arcweft checkout mutation;
- no Rust implementation;
- no Cargo build, test, clippy, fmt, or structural-audit execution;
- no runtime or LSP integration execution.

The implementation handoff therefore distinguishes source-confirmed design facts, historical repository validation, and commands that the implementer must execute.
