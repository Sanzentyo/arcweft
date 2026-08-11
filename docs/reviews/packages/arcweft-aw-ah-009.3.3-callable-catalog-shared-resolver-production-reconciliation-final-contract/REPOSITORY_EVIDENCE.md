# Repository evidence

## 1. Evidence basis and honesty

- Repository inspected through the authenticated GitHub connector: `Sanzentyo/arcweft`
- Current `main` inspected through the GitHub connector:
  `9fd6ee8fb2814ff04dc7a3e4ef413b86b7f4ac4d`
- Current commit subject: `Add deterministic launch profile selection`
- Reconciliation basis requested:
  `328e362f811896ebf866002c458fe0b970976654`, Jujutsu `wopypppm`
- Connector comparison: current main is six commits ahead of the requested
  basis. The four commits after the AW-AH-009.3 audit changed syntax item
  dispatch/top-level grammar, player-scene layout consumption, and deterministic
  launch-profile selection. The callable/checker, sema environment, HIR
  project/symbol, registration, and adapter-manifest files inspected for this
  contract are unchanged across that delta. The launch change determines which
  profile is selected before adapter manifests are loaded; it does not change
  `AdapterManifest::id` or callable publication identity.
- Current Jujutsu change: unavailable through the connector.

The repository was not cloned into the artifact container; production files
were inspected through the authenticated GitHub connector at the exact current
commit. No production file was modified.
No cargo command is claimed as newly run by this artifact creation.

The original AW-AH-009.3 ZIP binary was not mounted. The following upstream
evidence was available and inspected:

- exact archive filename and SHA-256;
- delivery summary;
- status sidecar (`READY_FOR_IMPLEMENTATION`, `IMPLEMENTATION`, zero open
  decisions);
- governing original request;
- the current repository's AW-AH-009.3 production-reconciliation audit and
  decided-substrate note, which record the upstream package audit.

The upstream package identity is therefore fixed and cross-checked, but this
artifact does not claim a second byte-for-byte extraction of its members.

## 2. Instructions inspected

The complete root `AGENTS.md` at current main was read, including repository
ownership, API/visibility, migration, validation, no-source-gate, compatibility,
and canonical structural-audit rules. The complete supplied Rust skill was
read. Relevant enforced findings include:

- prefer typed/newtype APIs and deliberate visibility;
- add behavior directly to Arcweft-owned enum inherent implementations;
- do not work around an owned enum with extension traits or ad hoc conversion
  helpers;
- do not add compatibility wrappers, deprecated dual readers, source gates, or
  removed-syntax diagnostics;
- validate with focused checks, workspace fmt/check/clippy/test, and the
  canonical structure audit;
- prove dependencies through Cargo metadata and public APIs.

## 3. Repository implementation notes inspected

### 3.1 AW-AH-009.3 production reconciliation audit

`docs/implementation/2026-07-16-aw-ah-009-3-production-reconciliation-audit.md`
records that:

- the original contract archive is valid and selected an implementation
  outcome;
- current checker resolution spans many free and selected families;
- migrating only the high-level resolver list would change behavior or leave
  two successful resolvers;
- AW-AH-009.3.1, .3.2, and .3.3 are required production reconciliation cuts;
- the repository-recorded validation passed at the audited commit;
- the recorded structural audit reported 2,958 files, 1,457 Rust files,
  681,517 physical Rust lines, zero structural errors, and 129 warnings.

Those are repository-recorded results, not commands rerun in this artifact
runtime.

### 3.2 Decided substrate note

`docs/implementation/2026-07-16-aw-ah-009-3-signature-help-decided-substrate.md`
confirms that source position conversion, HIR source identity, and project
symbol identity already exist, while exact call ranges, accepted HIR request
leasing, and callable catalog/resolver reconciliation are owned by .3.1, .3.2,
and .3.3 respectively.

## 4. Free-call checker evidence

### 4.1 Main dispatch

`crates/arcweft-lang-sema/src/checker/expr.rs` at current main performs free-call
checking in the observed order:

1. user FX definition validation;
2. FX constructor call;
3. expected enum variant/Result/Option construction;
4. builtin call;
5. Agent intrinsic;
6. presentation call;
7. environment/project/well-known function signature path, including virtual
   path, arguments, effects, return/effect result, and curried state;
8. path-call fallback;
9. selected call;
10. evaluated speaker/function-value/non-callable call.

The same file contains Result `Ok`/`Err` expected-type behavior, ordinary
function-value checking, selected dispatch, collection/domain/integer/capacity
methods, trait outcome normalization, and current recovery diagnostics.

### 4.2 Path and function-value helpers

`crates/arcweft-lang-sema/src/checker/expr/callable.rs` contains current path
call behavior including promotion/assumption, character/speaker values, local
function values, fixed spread, curried/partial groups, higher-order effect
callables, virtual path checks, and result/effect recording.

`crates/arcweft-lang-sema/src/checker/expr/signature_call.rs` contains current
signature argument mapping/checking. The reconciliation retains its accepted
semantics but eliminates it as a separate successful signature model.

### 4.3 Closed free families

The following owners were inspected:

- `checker/expr/fx.rs` — exact FX constructor and property/value behavior;
- `checker/expr/enum_variant.rs` — expected project enum variants;
- `checker/expr/builtin.rs` and builtin matches in `expr.rs` — builtin and
  numeric operation tables;
- `checker/expr/agent.rs` — complete Agent intrinsic inventory and semantic
  checks;
- `checker/presentation.rs` — presentation call names, arguments, results, open
  and closed named behavior, and state mutation.

The current presentation `check_character_look_arg` checks the look expression
without a structural expected type. This is the exact production seam closed by
the target presentation schema.

## 5. Selected/method evidence

`checker/expr.rs` establishes the current selected precedence:

- drop special form;
- inherent phase containing `traverse`, `parallel`, environment methods,
  collection methods, presentation-handle lifecycle, integer methods, domain
  methods, and capacity methods;
- trait methods;
- data-last fallback;
- final unknown/legacy method behavior.

The inspected domain rows include:

- `FxSampleContext.ordinal_phase`;
- `Vec<ObservedObject>.require_role`;
- `Map.get`;
- all Probe comparison aliases;
- `Diagnostics.has_error`;
- `RagContextPack.summary`;
- `context`/`with_context` for Need/Option/Result;
- character speaker `face` and `say`;
- `traverse` and `parallel`.

`crates/arcweft-lang-sema/src/checker/expr/method_fallback.rs` confirms data-last
visibility, direct-final versus next-curried parameter shapes, ambiguity,
effects, fixed spread, and shadow warnings.

`crates/arcweft-lang-sema/src/traits.rs` confirms typed trait catalog resolution
with missing, inherent, unique, and ambiguous outcomes. The target does not
replace this engine.

## 6. Dialogue evidence

The dialogue syntax/model and checker paths were inspected in:

- `crates/arcweft-lang-syntax/src/ast/dialogue.rs`;
- `crates/arcweft-lang-sema/src/checker/module.rs`;
- `crates/arcweft-lang-sema/src/checker/line_plan.rs`;
- dialogue call handling in `checker/expr.rs` and associated checker methods.

The accepted option inventory includes `id`, `text_key`, `voice`, `look`,
`stage`, `portrait`, `focus`, `cleanup`, `view`, `source_locale`, `hooks`,
`style`, `rich_text`, and open line arguments. Current content-token validation
is broader than option-expression typing and remains separate in the target.

## 7. Environment and registration evidence

### 7.1 Current environment

`crates/arcweft-lang-sema/src/env/base.rs` defines current `FunctionSignature`,
parameter, method signature, function/method maps, effects, and builder APIs.
The maps currently have overwrite-style insertion rather than one immutable
ordered overload catalog.

`crates/arcweft-lang-sema/src/env/registered.rs` confirms that
`RegisteredTypeCheckEnv` currently retains the base environment, character
facts, external-owner facts, and world/revision identity, but no callable
catalog.

### 7.2 Registration

`crates/arcweft-lang-sema/src/registration/model.rs` and
`registration/registrar.rs` confirm the registered semantic world transaction,
character/external owner publication, project symbol world/revision, and
current atomic candidate construction. The target attaches catalog validation
to this existing publication boundary instead of introducing a second world.

## 8. HIR and symbol evidence

The following were inspected:

- `crates/arcweft-lang-hir/src/symbol/identity.rs` —
  `CallableDeclarationId` and its package/module/owner/name identity;
- `crates/arcweft-lang-hir/src/model.rs` — functions, `FnSignature`, parameter
  groups/parameters, defaults, kinds, docs, and source-bearing HIR model;
- `crates/arcweft-lang-hir/src/project.rs` — ordered module/source project
  structure;
- project symbol table identity and source lookup paths referenced by the
  production audit.

`CallableDeclarationId` already derives deterministic identity traits and is
reused directly. `HirProject` has complete module knowledge, so modules with no
callables can be published without filesystem discovery. No current canonical
project method catalog justifies synthesizing source `impl` methods.

## 9. Adapter and Rust metadata evidence

`crates/arcweft-adapter-context/src/manifest.rs` confirms:

- manifest `id` and display metadata;
- functions, methods, effects, host calls, Rust functions/types, and tooling
  documentation;
- current application into `TypeCheckEnv`;
- typed Rust package/name/path/signature metadata.

`crates/arcweft-adapter-context/src/standard.rs` confirms the six accepted
standard manifest identities: sans-io, native-http, inference-tensor,
system-info, native-file, and math.

`crates/arcweft-rust-abi/src/lib.rs` confirms typed Rust parameter/signature
metadata. Current parameter metadata does not universally provide docs, so the
target explicitly represents missing documentation rather than fabricating it.

## 10. Character nominal evidence

`crates/arcweft-lang-sema/src/types/character_nominal.rs` defines the existing
structural model:

```text
Look { character: CharacterId }
Part { character: CharacterId }
Variant { character: CharacterId, part: CharacterPartId }
```

The target consumes these variants unchanged. Identity is character/part typed
identity, not local display spelling. Same-spelling tests are therefore
structural cross-owner tests, not string comparisons.

## 11. Type-key evidence

`crates/arcweft-lang-sema/src/types.rs` confirms that `TypeKind` supports
structural equality and hashing but is not an ordered display-key type. The
method catalog therefore uses `HashMap<ReceiverMethodKey, ...>` and stores a
separate typed candidate order. The design does not add `Ord` by formatting a
type.

## 12. Original AW-AH-009.3 evidence

Available original sidecars identify:

```text
STATUS=READY_FOR_IMPLEMENTATION
OUTCOME=IMPLEMENTATION
OPEN_RESULT_CHANGING_DECISIONS=0
ARCHIVE_SHA256=cdd1d7b764da238a6e4e8f3e774a3384017c8da5ffaea1969f2af279102a7cd5
```

The original summary selects one native position-aware semantic query over the
accepted registered world; ordinary, presentation, dialogue, project, adapter,
method, constructor, overload, function-value, partial, and recovered calls;
structural character nominal identity; and deletion of a competing word-only
fallback. This reconciliation preserves those decisions and supplies the
missing production-level model.

## 13. Validation still required by implementation

Because this archive is design-only and production changes are prohibited, the
following are prescribed but not claimed as newly executed:

- focused HIR/sema/registration/adapter tests;
- workspace `cargo fmt`, `cargo check`, Clippy with warnings denied, and tests;
- the canonical Rust structure audit;
- final Cargo metadata and public visibility assertions;
- exact migration parity and old-dispatch test instrumentation.

`IMPLEMENTATION_HANDOFF.md` fixes the commands and gates. A later implementation
must record actual exits and must not convert this repository evidence into a
claim that unrun commands passed.
