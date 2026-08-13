# Raw plan/AWBC execution API migration

The final implementation state contains no public execution or publication path
that accepts a raw `RuntimePlan` or raw `AwbcProgram` without consuming it
through complete admission.

## 1. Core VM

| Current surface | Final surface |
|---|---|
| public `awbc::vm::step(&AwbcProgram, ...)` | `pub(crate)` and first parameter `&AdmittedAwbcProduct` |
| public `awbc::vm::step_with_host(&AwbcProgram, ...)` | `pub(crate)` and first parameter `&AdmittedAwbcProduct` |
| VM `MakeRecord` reads raw layout and constructs | obtains admitted project/producer shape and constructs through it |
| VM errors stringify nominal construction | typed `AwbcVmError::Nominal { path, source }` |

No public re-export of raw step functions remains.

## 2. Fiber

| Current surface | Final surface |
|---|---|
| `FiberState::for_function(&AwbcProgram, ...)` | crate-private constructor taking `&AdmittedAwbcProduct` |
| `FiberState::for_entry(&AwbcProgram, ...)` | crate-private constructor taking `&AdmittedAwbcProduct` |
| resume with raw program | resume with admitted product and exact generation check |
| serialized fiber without semantic generation | required `RuntimeGenerationIdentity` field |
| restore then verify later | generation and program admission before frame/register restoration |

A fiber never owns a raw program or independent nominal catalog.

## 3. Product-step

| Current surface | Final surface |
|---|---|
| `AwbcProductStepExecutor::for_entry(raw, ...)` | crate-private/admitted-only `for_entry(AdmittedAwbcProduct, ...)` |
| `for_function(raw, ...)` | admitted-only |
| constructor calls only `verify()` | complete admission is a precondition |
| `replace_program_preserving_state(raw)` | deleted |
| raw replacement | `try_replace_product(candidate: AdmittedAwbcProduct)` with generation/contract and state preflight |

Replacement is atomic and preserves the old executor on failure.

## 4. `ArcweftRuntimeExecutor`

The unreleased raw constructors are deleted and directly replaced:

```rust
pub fn try_from_awbc_product(
    program: AwbcProgram,
    entry: &EntryRuntimeId,
) -> Result<Self, ArcweftRuntimeExecutorError>;

pub fn try_from_awbc_product_function(
    program: AwbcProgram,
    function: AwbcFunctionId,
) -> Result<Self, ArcweftRuntimeExecutorError>;

pub(crate) fn from_admitted_awbc_product(
    product: AdmittedAwbcProduct,
    target: AwbcExecutionTarget,
) -> Result<Self, ArcweftRuntimeExecutorError>;

pub fn try_replace_product(
    &mut self,
    candidate: AdmittedAwbcProduct,
) -> Result<(), ArcweftRuntimeExecutorError>;
```

The two public convenience functions consume raw input and perform complete
admission internally. They publish `Self` only after target lookup and fiber
construction succeed. The old `replace_product_awbc_program` name and raw
argument are deleted.

## 5. `Engine`

Final constructors:

```rust
pub fn new(plan: AdmittedRuntimePlan) -> Result<Self, EngineError>;
pub fn for_flow(
    plan: AdmittedRuntimePlan,
    flow: FlowRuntimeId,
) -> Result<Self, EngineError>;
pub fn for_entry(
    plan: AdmittedRuntimePlan,
    entry: EntryRuntimeId,
) -> Result<Self, EngineError>;

pub fn try_from_raw_plan(plan: RuntimePlan) -> Result<Self, EngineError>;
```

`try_from_raw_plan` is the only raw convenience; it consumes and fully admits.
`Engine` stores the admitted plan/generation, not a cloned raw plan.

## 6. `BytecodeProgram`

`BytecodeProgram` remains a serialization/quarantine carrier. Ambiguous raw
conversion names are replaced directly:

```rust
pub fn from_raw_runtime_plan(plan: RuntimePlan) -> Self;
pub fn into_raw_runtime_plan(self) -> Result<RuntimePlan, BytecodeProgramError>;
pub fn into_raw_awbc_program(self) -> Result<AwbcProgram, BytecodeProgramError>;

pub fn try_admit(
    self,
) -> Result<AdmittedBytecodeProgram, BytecodeProgramAdmissionError>;
```

Old `from_runtime_plan` and `into_runtime_plan` are deleted with no aliases.
`AdmittedBytecodeProgram` is an enum over admitted plan/AWBC forms and does not
expose raw inner objects.

AOT/plan/AWBC conversions return raw quarantine or admitted wrappers explicitly;
they never smuggle an independent catalog.

## 7. Runtime-driver generation image

`ProgramGeneration` and `GenerationRuntimeImage` own:

- host `GenerationId`;
- semantic `RuntimeGenerationIdentity`;
- admitted plan and/or admitted AWBC product sharing one aggregate;
- runtime state.

Session construction accepts a generation image, not raw program/plan.

The old `GenerationRuntimeImage::into_runtime` is deleted if it returns bare
runtime state. The selected replacement is `into_parts` returning both admitted
generation authority and runtime state in one non-mixable object.

## 8. Session construction and hot swap

- session constructors consume an admitted generation image;
- bundle convenience constructors decode raw artifacts, admit completely, bind
  Character/View catalogs, and only then create a session;
- hot swap admits the candidate off to the side;
- same-generation replacement requires identical canonical contract;
- cross-generation replacement uses existing typed migration policy;
- no old producer/catalog handle survives a successful transition;
- any error leaves active session/image unchanged.

## 9. Restore and replay

Restore APIs receive an admitted generation image or explicit
`&AdmittedRuntimeGeneration`.

Order:

1. decode fixed version-`1` snapshot;
2. compare saved generation identity;
3. validate AWBC/program target identity;
4. admit Character/View catalogs and dialogue schema;
5. validate every RuntimeValue;
6. reconstruct ownership/fiber/root/View/session state;
7. publish.

Root replay validates values before transition application. No restore method
accepts raw catalog, producer ID, layout hash, or custom digest as authority.

## 10. Bundle and AWFB

Bundle/AWFB decoding returns raw quarantine artifacts. Activation entry points
consume those artifacts and run full plan/AWBC admission.

Bundle canonical digest includes AWBC bytes, which include the generation
contract. Bundle-level version remains `1`.

A bundle containing both plan and AWBC must use plan-paired AWBC admission and
therefore one aggregate. A bundle cannot activate two same-identity,
different-body contracts.

## 11. Save/session snapshots

Save formats record `RuntimeGenerationIdentity` wherever persisted RuntimeValue,
fiber, root, View, dialogue, or session state depends on a generation.

Current fixed version remains `1`; unreleased fields are directly replaced.
Save/load does not serialize operational handles.

Typed errors preserve generation mismatch, nominal lookup/tree path, producer,
role/custom coordinate, and snapshot location.

## 12. View runtime

View mount, input admission, restored state, and generated View product
activation receive the admitted generation and admitted View registry.

CharacterDialogue values are decoded through
`CharacterDialogueRuntimeSchema` before mount. A View cannot request a
producer shape by string or carry a catalog copied from another generation.

## 13. Native, Web, and headless players

All player startup paths use one high-level consuming function:

```text
bundle/raw artifact
  -> decode
  -> complete plan/AWBC admission
  -> catalog correlation
  -> generation image
  -> player runtime
```

No player exposes raw VM/fiber constructors. Web/Wasm adapters store the
admitted generation identity in opaque Rust-owned state, not in caller-
modifiable JavaScript strings.

## 14. Agent runner, MCP, CLI, and tooling

Agent/MCP/CLI commands may inspect raw artifacts, verify them diagnostically, or
print errors. Commands that run/preview/replay must call the same full admission
path as players.

A `verify` command is not an execution command. Its success cannot be passed as
a token to bypass admission.

## 15. Runtime accelerator, JIT, and AOT

Compiler backends take admitted products. Cache keys include generation
identity and canonical contract bytes/digest.

Generated code calls nominal construction through admitted shapes. A backend
that cannot carry the aggregate returns its existing typed unsupported result;
it does not fall back to raw VM execution.

## 16. Tests and helpers

All fixture builders choose one of:

- raw malformed artifact builder, used only for admission-negative tests;
- checked projection builder producing raw canonical artifact;
- admitted fixture builder calling real admission.

No test helper constructs operational handles, role types, custom catalog
digests, or nominal values from raw scalars.

Obsolete internal call sites fail workspace compilation; external visibility/
constructor guarantees use trybuild compile-fail tests. Source-spelling grep is
discovery only, never the closure proof.
