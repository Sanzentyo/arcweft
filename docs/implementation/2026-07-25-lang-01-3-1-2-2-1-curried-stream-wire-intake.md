# Lang-01.3.1.2.2.1 curried Stream wire-allocation return intake

Date: 2026-07-25

## Status

`RETURNED_ACCEPTED_IMPLEMENTATION_READY_DEFERRED`.

The returned archive is retained at:

- [`arcweft-lang-01.3.1.2.2.1 curried Stream wire-allocation reconciliation`](../reviews/packages/zips/arcweft-lang-01.3.1.2.2.1-curried-stream-wire-allocation-reconciliation-final-contract.zip),
  SHA-256
  `8ADED7B1CB5D92F9D820C2CC82121AC6D070F3CF26D1618DC23FF144081090AD`,
  56,620 bytes.

The ZIP contains 24 file members plus its root directory entry.
`manifest.json` covers all 22 other payload members, and
`MANIFEST.sha256` covers those payloads plus `manifest.json`; every recorded
length and SHA-256 matches. The request copied into the ZIP matches repository
request SHA-256
`A07F05F4AD901E323EEC528B31CB0064D26F0AE23FE91CBFEC9758D215A2F170`.
All six worked byte vectors reproduce their declared byte counts and SHA-256
values. The JSON, CSV, and Markdown test matrices contain the same 105 unique
rows. `FINAL_STATUS.md` reports `READY_FOR_IMPLEMENTATION` and zero open
questions.

The three parent archive hashes and internal manifests were independently
rechecked and match the package ledger:

- base Lang-01.3.1.2:
  `5043483C42259CE638B93BEA7F6426D6EF169A7E22AEB426E86B7E1168A60105`;
- Lang-01.3.1.2.1:
  `66809A1280A507F69BB78D9DF3BF7AF227A91CD68B86CF8771CBF9EE20AA856A`;
- Lang-01.3.1.2.2:
  `D1BD7FB5301509CA88BE7C9D3662942CA88472D11143499C0C3067D626DF9418`.

The package inspected current pushed Git commit
`0b7e095f4193b9f7fbbc95cc350a626a8a63640a`, corresponding locally to Jujutsu
change `pxulxlkmwqztnrwykmtowvvlkruusooy`.

## Reconciled wire allocation

The former parent/child opcode conflict is closed:

```text
0x27 OpenStream
0x28 FinishStream
0x29 ApplyExternalStreamGroup
```

Codec 8 continues to reject removed bytes `0x1c`, `0x1d`, `0x1e`, and `0x20`.
Current `0x22 CallTraitMethod` and `0x23 RegisterCleanup` remain live and are
not misclassified as Source removals. Current pushed `main` ends its
non-terminator allocation at `0x26`, so `0x29` is collision-free.

## Sole owners

- `RuntimeCallableBoundarySignature` is changed in place from flat parameters
  to ordered groups and `(group, parameter)` coordinates. No Stream-specific
  parallel signature family is created.
- `RuntimeFunctionValue` is changed in place to the closed
  `Closure | ExternalStreamPartial` enum.
- `RuntimeExternalStreamArgumentProduct` is the one argument product across
  runtime evaluation, host request, save/restore, and fingerprinting.
- `RuntimeStreamDefinitionId` remains the RuntimePlan/AWBC table index;
  `RuntimeStreamDefinitionKey`, `StreamGeneration`,
  `StreamInstanceOrdinal`, and the complete `StreamInstanceKey` retain their
  distinct accepted meanings.
- Child spellings `StreamDefinitionId`, `GenerationId`, `StreamInstanceId`,
  and `RuntimeTypeLayoutHash` are not aliases and are not published.

## Deletion-driven implementation order

1. P3 may add accepted-sema external binding evidence only; it publishes no
   Stream runtime or wire owner.
2. P4+C1 deletes the flat callable fields and old `RuntimeFunctionValue` shape,
   then fixes all compile errors toward grouped coordinates, the sole enum, and
   the canonical product.
3. P5+C2 removes runtime name lookup and flat compiler projections in favor of
   one accepted-sema-to-core projection.
4. C3 adds non-final group application and an atomic final Open while public
   AWBC remains codec 7.
5. P6+C4 is one codec-8 authority switch: old Source/Stream opcode and table
   readers, flat Open, and obsolete function/tag branches are deleted in the
   same complete cut that adds ABI 2, the final three opcodes, verifier, VM,
   lowerer, and codegen. No codec-7 reader survives.
6. P7+C5 removes flat endpoint/host argument DTOs and serializes the core
   request and canonical product directly.
7. P8+C6 deletes old save/flat-partial/Source persistence translation and
   switches bundle/save2/restore/hot-reload atomically.

Steps 5 through 7 are a protected migration group and are not partially
published to `main`. The 105-row matrix, focused tests, workspace check,
strict Clippy, `just test-workspace`, Tier 2, Cargo metadata, and structural
audit are required before that public switch.

## Dependency position

The contract is ready and no correction request is needed. Production work
remains deferred to the established Lang-01.3 dependency position; accepting
this ZIP does not move Stream ahead of the current AW-AH/Proof/RichText/
ordinary-function sequence. No compatibility alias, flat adapter, dual codec,
Source shim, source gate, CSS path, or Takumi path is introduced meanwhile.
