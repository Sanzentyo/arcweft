# Repository-Aware Validation

## 1. Baseline freeze

Repository: `Sanzentyo/arcweft`  
Branch: `main`  
Frozen commit: `4fd6331dc342d30a7f4ac7774852b60801866ef7`  
Commit message: `Implement project nominal type resolution`

`main` was queried at the start of repository inspection and immediately before artifact freeze. Both checks returned the same SHA.

The contract is therefore against that exact latest `main`, not a historical approximation.

## 2. Access and write boundary

Repository evidence was read through the configured GitHub connector at the exact commit. All repository actions were read-only:

- commit search/fetch;
- file search;
- file fetch;
- response-resource scrolling.

No connector write action, GitHub mutation, local repository patch, commit, branch, or pull request was created.

The artifact was generated only under `/mnt/data`. Production repository writes: **0**.

## 3. Full instruction inputs reviewed

Before design freeze, these inputs were read through their final line:

- latest root `AGENTS.md` at the pinned commit;
- `/mnt/data/Rust Skill.txt`;
- `/mnt/data/前提(Sanzentyo-arcweft).txt`;
- the complete Lang-01.1.1.2.2 request.

Input hashes are in `evidence/BASELINE.json`.

## 4. Repository facts validated

### 4.1 Layer and dependency facts

Validated from root `Cargo.toml`, crate Cargo manifests, and `AGENTS.md`:

- the repository requires `syntax -> HIR -> sema -> runtime-plan/verify -> tooling`;
- Rust ABI is data/codecs only;
- adapter-context optionally depends on sema;
- sema does not depend on adapter-context;
- sema already depends on workspace `blake3`;
- Rust edition is 2024 and workspace Rust version is 1.96.

The selected design respects this direction by putting neutral input/projection authority in sema and manifest conversion in adapter-context.

### 4.2 Current defect chain

Validated from:

- `crates/arcweft-rust-abi/src/lib.rs`;
- `crates/arcweft-adapter-context/src/manifest.rs`;
- `crates/arcweft-adapter-context/src/publication.rs`;
- `crates/arcweft-lang-sema/src/types.rs`;
- `crates/arcweft-lang-sema/src/callable/schema.rs`.

The current production chain contains `ArcweftRustTypeRef::Named`, `AdapterTypeKind::Named`, context-free conversion to `TypeKind::Named`, and final publication records. The authored path produces `AcceptedNominal` and exact schema equality does not equate them.

### 4.3 Accepted nominal authority

Validated from:

- `crates/arcweft-lang-sema/src/env/nominal.rs`;
- `crates/arcweft-lang-sema/src/types/nominal.rs`;
- `crates/arcweft-lang-sema/src/nominal/resolver/engine/resolution.rs`.

Repository facts:

- the concrete owner type is `AcceptedNominalOwnerId`;
- Rust owner is `RustPackage(RustPackageId)`;
- accepted IDs carry owner + canonical `TypePath`;
- catalog exact records are globally keyed by exact path;
- catalog records carry arity and `Exact`/`Opaque`/`Character` semantics;
- the authored resolver checks arity, resolves nested arguments, and constructs accepted nominal identity;
- the accepted catalog already owns a stable digest.

The contract reuses these types and moves shared record instantiation behavior to the original accepted record implementation.

### 4.4 Registration order

Validated from:

- `crates/arcweft-lang-sema/src/registration/model.rs`;
- `crates/arcweft-lang-sema/src/registration/registrar.rs`;
- `crates/arcweft-lang-sema/src/callable/builder.rs`.

Current order constructs `AcceptedNominalWorld` before project/environment callable catalog finish. Environment publications are currently supplied prebuilt in the request.

The selected correction changes only the missing boundary: requests carry typed inputs; registrar projects after world construction and before builder admission. The existing transaction remains the final authority.

### 4.5 Source-backed fact split

Validated from:

- `crates/arcweft-adapter-context/src/manifest/registration.rs`;
- `crates/arcweft-project-loader/src/environment.rs`;
- `crates/arcweft-cli/src/app/project.rs`.

Current source-backed facts carry generated documents/external facts, while CLI/compiler separately build final publications before registration. The contract removes this split and routes all publication/metadata inputs through source-backed project facts.

### 4.6 Non-callable metadata

Validated from:

- `crates/arcweft-adapter-context/src/manifest.rs`;
- `crates/arcweft-lang-sema/src/env/base.rs`;
- `crates/arcweft-lang-sema/src/env/enums.rs`;
- `crates/arcweft-lang-sema/src/types/substitution.rs`.

Current Rust type exports and enum metadata contain string/`Named` identity, but enum payloads and generic substitution already have typed `TypeKind` structure. The contract therefore migrates Rust metadata to an accepted-ID catalog and reuses the existing recursive substitution behavior.

### 4.7 Candidate/tooling identity

Validated from:

- `crates/arcweft-lang-sema/src/callable/identity.rs`;
- `crates/arcweft-lang-sema/src/callable/catalog.rs`;
- `crates/arcweft-lang-sema/src/signature/project.rs`;
- `crates/arcweft-lsp/src/features/hover.rs`;
- `crates/arcweft-lsp/src/features/nominal_types.rs`.

Method keys already contain exact receiver `TypeKind`; semantic signature projection already carries exact parameter/result `TypeKind`. Correct publication therefore propagates accepted IDs through candidate selection and tooling without introducing a separate compatibility rule.

### 4.8 Persistent identity

Validated from:

- `crates/arcweft-project/src/persistent_object/schema.rs`;
- `crates/arcweft-project/src/persistent_object/payload.rs`;
- `crates/arcweft-compiler/src/persistent.rs`;
- `crates/arcweft-compiler/src/incremental.rs`.

The existing persistent key already contains `environment_digest`; no new field is necessary. Some current compiler snapshots use zero placeholders. The contract populates the existing field from the registered environment where a complete world exists and keeps persistent schema shape/version unchanged.

## 5. Contract consistency validation

The package validator checks:

- all mandatory files exist;
- baseline SHA is consistent across machine and human documents;
- final decisions are single-valued;
- required owner/publication/metadata/digest/rollback terms are present;
- no unresolved decision placeholder exists;
- no alternative design section exists;
- the test matrix is parseable, has unique IDs, and covers every required category;
- JSON evidence/decisions parse;
- request and skill hashes match;
- `MANIFEST.sha256` covers all package files except itself and has correct hashes.

The final matrix contains **197** typed-API tests.

## 6. Validation actually performed now

Performed:

1. latest-main SHA verification twice;
2. full instruction/request review;
3. commit-pinned source inspection of 49 repository files;
4. layer/dependency/order reconciliation;
5. exact contract/API/error/digest/test traceability checks;
6. machine validator execution;
7. deterministic ZIP construction;
8. `unzip -t` integrity check;
9. SHA-256 calculation for package files and ZIP.

## 7. Production compile/test boundary

This artifact contains no production implementation and is not a Cargo workspace. Consequently, repository `cargo fmt`, `cargo clippy`, and `cargo test` are implementation acceptance commands rather than evidence that can be produced by this contract-only ZIP.

The exact commands are prescribed in `IMPLEMENTATION-MAP.md`. They must be run on the production implementation commit; this package does not claim they were run now.

This does not weaken the repository-aware validation of the contract: the contract’s source facts, dependency placement, API ownership, construction order, and consumer consequences were checked directly against the pinned repository source.

## 8. Inspection inventory

See `evidence/INSPECTED-FILES.tsv` for the complete commit-pinned file list and the repository fact taken from each file.
