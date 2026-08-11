# Compile-clean implementation order

This is the sole accepted interleave of Lang-01.3.1.2.1 Cuts 3--8 (P3--P8)
and Lang-01.3.1.2.2 Cuts 1--6 (C1--C6).

| Step | Parent/child cuts | Compile-clean publication boundary | Must not exist after step |
| ---: | --- | --- | --- |
| 1 | P3 | Shared accepted-sema external binding evidence only; no Stream runtime/wire publication | no new runtime/wire owner |
| 2 | P4 + C1 | Parent identities/lifecycle/table plus grouped in-place callable boundary, canonical product, and sole RuntimeFunctionValue enum; all core matches/serde/snapshot traversal updated atomically | flat external boundary; child identity aliases; two function-value shapes |
| 3 | P5 + C2 | RuntimePlan sole definition table and compiler's one accepted-sema-to-core group projection | runtime name lookup; flattening projection |
| 4 | C3 over P4/P5 | Structured non-final application and atomic final Open using parent table/lifecycle; still codec 7 externally | codec-8 reader/writer; early Open |
| 5 | P6 + C4 | One ABI2/codec8 commit containing all tables, tags, opcodes 0x27/0x28/0x29, verifier, VM, lowerer, codegen, removed-byte rejection | incomplete codec8; flat Open; conflicting opcode meanings |
| 6 | P7 + C5 | Shared strict host request/serde owner carrying StreamInstanceKey and canonical product | endpoint DTO; adapter flattening |
| 7 | P8 + C6 | Bundle/save2/restore/hot reload, partial snapshots, generation pins, exact fingerprints and blockers | old save/flat partial shape |

Steps 5--7 are the parent protected migration group. They may be reviewable as
separate commits, but main/release publication occurs only after all are
complete and the full validation matrix passes.

## Step-specific gates

### Step 1 — P3

Consume existing `CallableSignatureSchema`, `CallTargetFacts`, and exact slot
coordinates. Add missing behavior only as inherent methods on their owning
types. Do not create a Stream-specific resolver.

### Step 2 — P4 + C1

Change the parent enums/types in place. In one commit:

- establish final parent identity names;
- replace flat `RuntimeCallableBoundarySignature.parameters` with grouped
  coordinates;
- introduce the sole canonical product;
- change `RuntimeFunctionValue` into the closed two-variant enum;
- update all constructors, matches, ownership, serde, nesting, digest, and
  snapshot traversal so the workspace compiles with no old shape.

### Step 3 — P5 + C2

The compiler performs one bounded traversal of accepted sema facts and emits
the grouped boundary, authored evaluation plan, canonical slots, declaration
digest, default fingerprints, and external signature fingerprint. No source
reparse, resolver rerun, name matching, or recovery from debug strings.

### Step 4 — C3

Implement group application through inherent behavior on
`RuntimeFunctionValue`/owned partial/product types. Prepare and validate the
entire Open commit before mutable table/request access. Keep public AWBC at
codec 7 until Step 5.

### Step 5 — P6 + C4

Change ABI/codec constants, program table order, callable metadata tables,
Stream definition table, runtime/constant tags, opcode map, codec, verifier,
VM, lowerer, and compiled codegen in the same complete cut. No old reader,
source table, flat Open, or provisional opcode branch.

### Step 6 — P7 + C5

Serialize the core request directly. Native, Web, and Agent must produce the
same canonical bytes and perform no grouping/flattening/name lookup.

### Step 7 — P8 + C6

Update fingerprints and save schema atomically; validate restore into a
candidate state in the parent order. Active partials retain exact generation
and captured product; no translation/rebinding.

## Required final gates

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
just test-tier2
cargo metadata --no-deps --format-version 1
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The implementation note records exact commit/Jujutsu identities, commands,
failures/retries, and structural measurements. No source-text removal gate is
an acceptance condition.
