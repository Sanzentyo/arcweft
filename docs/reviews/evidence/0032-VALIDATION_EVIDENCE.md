# Validation evidence

## 1. Material actually inspected

The following were read completely where supplied locally:

- `Rust Skill.txt`;
- `前提(Sanzentyo-arcweft).txt`;
- the supplied Lang-01.3.1.2.3.2.1.2 request;
- root `AGENTS.md` and scoped `crates/AGENTS.md`, `docs/AGENTS.md`, and
  `docs/reviews/AGENTS.md` at the pinned commit;
- retained parent final contract, Rust owners/APIs, nominal projection,
  precedence, persistence, implementation order, inventory, and test matrix;
- retained opaque child Rust owners/APIs and producer projection contract; and
- locally materialized exact-commit source blobs for core nominal records,
  checked types, opaque values, value owner/path evidence, dialogue schema and
  domain type, runtime-plan semantic facts, and crate map.

Exact pinned GitHub raw/tree source was additionally inspected for:

- dialogue `character_dialogue/typed_value.rs` and `patch.rs`;
- core `value/ownership/path.rs`, `plan.rs`, and
  `plan/entry_inventory.rs`;
- runtime-driver `session.rs`, `session_save.rs`, `view_runtime.rs`, and source
  tree; and
- parent package searchable mirrors and implementation/test sidecars.

The source findings used by this contract include the public unchecked
constructor and `validate_shape`, crate-private checked constructor, exact
validation order, current CharacterDialogue expected-layout/root/custom/inline
nominal wrappers, the `Named("Dynamic")` custom value field, direct typed-value
Deserialize, descriptorless normalize/empty/patch rebuilds, the dialogue-local
ordinal path, the runtime-plan interning map that is currently dropped, and the
owner-only checked nominal variant branch.

## 2. Retained artifact hash evidence

`PARENT_ARTIFACTS.sha256` records the two SHA-256 values mandated by the request.
The searchable retained package mirrors were inspected. The binary ZIP bytes
were not downloaded into this execution environment, so this archive does not
claim to have recomputed those two retained ZIP hashes. Implementers must run
`sha256sum -c PARENT_ARTIFACTS.sha256` against repository-retained ZIP bytes at
G0.

## 3. Repository/build evidence boundary

This environment had no local Git checkout and its command sandbox could not
reach GitHub directly. Exact immutable files were therefore inspected through
GitHub raw/tree retrieval, but the repository was not cloned. The following
were **not** run here and are not represented as green:

- `git status`/clean-tree verification;
- Cargo check/test;
- Clippy;
- rustfmt;
- structural audit;
- Tier 2 commands; or
- implementation tests.

They are normative G0/G9/G10 acceptance commands in this package. Static design
closure is complete; build closure belongs to implementation.

## 4. Archive checks actually executed

The package builder performs and records:

1. byte equality of `SOURCE_REQUEST.md` and the supplied request;
2. exact `OPEN_QUESTIONS.txt` content `OPEN_QUESTIONS=0\n`;
3. absence of production/patch extensions and forbidden top-level build files;
4. JSON parsing and metadata assertions;
5. CSV parsing and row-count assertions;
6. version-policy text assertions;
7. sorted SHA-256 manifest generation for every file except the manifest;
8. deterministic ZIP creation with fixed timestamps, permissions, and order;
9. extraction into a fresh directory;
10. extracted manifest verification;
11. second deterministic ZIP generation and byte comparison; and
12. final ZIP SHA-256 calculation.

The concrete results are appended below by the package builder after all files
exist.

## 5. Executed result summary

- request copy byte-identical: `PASS`
- `OPEN_QUESTIONS=0` exact content: `PASS`
- production/patch extension scan: `PASS`
- `contract.json` parse: `PASS`
- inventory CSV rows: `56` (`PASS`)
- test CSV rows: `80` (`PASS`)
- fixed version decision present: `PASS`
- manifest generation/verification: `PASS`
- fresh extraction verification: `PASS`
- deterministic ZIP reproduction: `PASS`
