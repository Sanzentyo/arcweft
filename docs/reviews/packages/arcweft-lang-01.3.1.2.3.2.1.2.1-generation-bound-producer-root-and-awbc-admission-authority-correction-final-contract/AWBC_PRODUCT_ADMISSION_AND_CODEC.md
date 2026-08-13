# AWBC generation contract, codec, admission, and VM authority

## 1. Current defect

The current AWBC program is independently Serde/canonically encoded and
structurally verified. Public VM/fiber/product-step APIs accept raw
`AwbcProgram`. Therefore plan-only admission cannot close nominal construction
or CharacterDialogue activation.

This contract makes the AWBC artifact self-describing for authority and makes
whole-product admission mandatory.

## 2. Raw `AwbcProgram` field

`AwbcProgram` retains the current header and table set. It gains one required
private field:

```rust
generation_contract: RuntimeGenerationContractDeclaration
```

The read-only accessor is public. Runtime-plan lowering uses
`try_with_generation_contract`.

Making the field private prevents external struct literals from accidentally
omitting it, but does not treat privacy as authority. Serde can still construct
hostile raw programs.

## 3. Canonical codec

After the current fixed header and before the strings table, encode:

```text
u32 generation_contract_len
canonical serialized RuntimeGenerationContractDeclaration
```

The length is checked before allocation. All current table bytes follow in
their current order.

The program/product digest includes the exact contract bytes. `AWBC_ABI_VERSION`
and `AWBC_CODEC_VERSION` remain `1`. There is no old decoder branch and no
"contract absent" marker.

## 4. Standalone admission order

`AwbcProgram::try_admit(self)` performs:

1. current header magic and fixed version-`1` checks;
2. current AWBC structural verifier, including table/range/function/block/
   instruction/type checks;
3. generation declaration shape;
4. CharacterDialogue role/custom declaration;
5. custom digest;
6. nominal catalog consistency;
7. project roots derived from AWBC runtime types, signatures, frame slots,
   constants, patterns, instructions, entries, roots, save-visible state, and
   product tables;
8. producer root traversal;
9. exact claimed authorization equality;
10. global missing/unreachable catalog equality;
11. AWBC/declaration root correlation;
12. generation identity recomputation;
13. atomic `AdmittedRuntimeGeneration` and `AdmittedAwbcProduct`.

No fiber or VM frame is allocated before step 13.

## 5. Plan-paired admission

`AdmittedRuntimePlan::try_admit_awbc(raw)` performs:

1. AWBC header/version;
2. AWBC structural verifier;
3. raw AWBC generation-contract local validation;
4. exact identity comparison with plan generation;
5. exact canonical body-byte comparison;
6. AWBC table/root correlation with that same declaration;
7. construction of `AdmittedAwbcProduct` reusing the plan aggregate.

It does not rebuild a catalog or producer map.

## 6. Exact root inventory from AWBC

The AWBC root projection includes:

- every `AwbcRuntimeType`;
- every signature parameter/result;
- every frame slot;
- typed constants;
- typed patterns;
- record/variant instruction type references;
- callable, flow, entry, host/intrinsic, task, stream, source, root, content,
  and persistence-visible types;
- any product table that can publish RuntimeValue.

Dense table IDs are resolved first. The root coordinate is the typed semantic
coordinate preserved by lowering, never the dense index alone.

## 7. Non-Serde admitted wrapper

`AdmittedAwbcProduct` owns raw program plus the admitted aggregate. It has only
read-only accessors needed by verifier/VM/lowering. It has no:

- Serde;
- Default;
- public fields;
- public constructor;
- `Deref<Target = AwbcProgram>`;
- raw `into_program`;
- raw replacement;
- method returning an independently owned catalog.

## 8. VM and fiber signatures

Low-level owners become crate-private:

```rust
pub(crate) fn step(
    product: &AdmittedAwbcProduct,
    fiber: &mut AwbcFiber,
    budget: &mut AwbcBudget,
) -> Result<AwbcStepOutcome, AwbcVmError>;

pub(crate) fn step_with_host<H: AwbcHost>(
    product: &AdmittedAwbcProduct,
    fiber: &mut AwbcFiber,
    budget: &mut AwbcBudget,
    host: &mut H,
) -> Result<AwbcStepOutcome, AwbcVmError>;
```

Fiber constructors and resume/restore APIs accept the admitted product and
store `RuntimeGenerationIdentity` in every serializable resume state.

`MakeRecord` obtains a project or producer shape from the admitted aggregate and
then calls crate-private nominal construction. It does not read an arbitrary
layout table and invoke construction directly.

## 9. Product-step executor

`AwbcProductStepExecutor` owns `AdmittedAwbcProduct`, not raw program.

- `for_entry`/`for_function` are crate-private or take an admitted product;
- construction cannot call only `verify`;
- replacement takes a completely admitted candidate;
- same-generation replacement requires identical canonical contract;
- cross-generation replacement goes through runtime-driver hot-swap policy;
- a failed replacement retains program, fiber, budget, and host state.

The existing raw `replace_program_preserving_state` is deleted.

## 10. `AwbcAdmissionError`

Owner: `arcweft_core::awbc::admission`.

```rust
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AwbcAdmissionError {
    #[error("AWBC header is invalid: {source}")]
    Header {
        #[source]
        source: AwbcCodecError,
    },

    #[error("AWBC program verification failed: {source}")]
    Verify {
        #[source]
        source: AwbcVerifyError,
    },

    #[error("AWBC generation contract is invalid: {source}")]
    GenerationContract {
        #[source]
        source: RuntimeGenerationContractError,
    },

    #[error("AWBC typed root inventory differs from the generation contract")]
    RootInventory {
        source: AwbcRootInventoryError,
    },

    #[error("AWBC generation differs from admitted plan: {source}")]
    Generation {
        #[source]
        source: RuntimeGenerationMismatch,
    },

    #[error("AWBC and plan claim equal generation identity but unequal contract bytes")]
    ContractCollision {
        identity: RuntimeGenerationIdentity,
    },
}
```

`AwbcVerifyError` remains structural. It is not extended to mean operational
admission.

## 11. Codec/decode errors

Malformed generation-contract bytes map through `AwbcCodecError` with byte
offset and section `GenerationContract`. Parsed semantic failures map through
`AwbcAdmissionError::GenerationContract`.

A bundle/driver boundary retains both typed layers; it does not format either
as a string.

## 12. AOT/JIT/codegen

AOT/JIT/runtime accelerator code receives `&AdmittedAwbcProduct` and may compile
only after admission. Generated code retains the generation identity and calls
runtime nominal/producer operations through admitted handles.

A compiled artifact with mismatched identity/body is rejected before linking or
activation. Unsupported codegen returns its existing typed unsupported error;
it never falls back to raw interpreter execution.

## 13. No duplicate authority

The generation image owns one admitted aggregate. VM, fiber, product-step,
interpreter, AOT, JIT, host calls, dialogue, restore, and View all borrow from
that aggregate.

No AWBC table, VM-local map, codegen side table, or product cache copies nominal
layouts/role/custom descriptors as a second operational authority.
