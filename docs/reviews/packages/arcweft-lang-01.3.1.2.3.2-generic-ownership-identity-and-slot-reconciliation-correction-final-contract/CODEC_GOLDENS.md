# Canonical codec golden vectors

`CODEC_GOLDENS.json` is the machine-readable authority. This file explains the same vectors.

## 1. Primitive/composite vectors

| Name | Type | Typed value | Bytes (hex) | Length | Canonical JSON |
|---|---|---|---|---:|---|
| `execution-1` | `ExecutionInstanceId` | `1` | `0100000000000000` | 8 | `"1"` |
| `execution-max` | `ExecutionInstanceId` | `18446744073709551615` | `ffffffffffffffff` | 8 | `"18446744073709551615"` |
| `cursor-next-1` | `RuntimeIdCursor` | `Next(1)` | `000100000000000000` | 9 | `{"state":"next","value":"1"}` |
| `cursor-exhausted` | `RuntimeIdCursor` | `Exhausted` | `01` | 1 | `{"state":"exhausted"}` |
| `record-field-1` | `RuntimeRecordFieldId` | `1` | `01000000` | 4 | `1` |
| `local-slot-2` | `RuntimeLocalSlotId` | `2` | `0200000000000000` | 8 | `"2"` |
| `slot-revision-3` | `RuntimeSlotRevision` | `3` | `0300000000000000` | 8 | `"3"` |
| `affine-owner-1-7` | `RuntimeAffineOwnerId` | `execution=1,ordinal=7` | `01000000000000000700000000000000` | 16 | `{"execution":"1","ordinal":"7"}` |
| `transaction-1-9` | `RuntimeOwnershipTransactionId` | `execution=1,ordinal=9` | `01000000000000000900000000000000` | 16 | `{"execution":"1","ordinal":"9"}` |
| `owned-slot-environment-local` | `RuntimeOwnedSlotId` | `exec/1/env/2` | `0001000000000000000200000000000000` | 17 | `{"kind":"environment_local","execution":"1","local":"2"}` |
| `owned-slot-closure-capture` | `RuntimeOwnedSlotId` | `exec/1/closure/2/capture/3` | `010100000000000000020000000000000003000000` | 21 | `{"kind":"closure_capture","execution":"1","closure":"2","capture":3}` |
| `owned-slot-awbc-register` | `RuntimeOwnedSlotId` | `exec/1/awbc/fiber/2/frame/3/register/4` | `0201000000000000000200000000000000030000000000000004000000` | 29 | `{"kind":"awbc_register","execution":"1","fiber":"2","frame":"3","register":4}` |
| `owned-slot-awbc-frame-local` | `RuntimeOwnedSlotId` | `exec/1/awbc/fiber/2/frame/3/local/4` | `0301000000000000000200000000000000030000000000000004000000` | 29 | `{"kind":"awbc_frame_local","execution":"1","fiber":"2","frame":"3","local":4}` |
| `owned-slot-mailbox-lane` | `RuntimeOwnedSlotId` | `exec/1/mailbox/2/lane/3` | `040100000000000000020000000000000003000000` | 21 | `{"kind":"mailbox_lane","execution":"1","mailbox":"2","lane":3}` |
| `owned-slot-child-packet` | `RuntimeOwnedSlotId` | `exec/1/child/2/packet/3` | `050100000000000000020000000000000003000000` | 21 | `{"kind":"child_packet","execution":"1","child":"2","packet":3}` |
| `owned-slot-transfer-packet` | `RuntimeOwnedSlotId` | `exec/1/transfer/2/packet/3` | `060100000000000000020000000000000003000000` | 21 | `{"kind":"transfer_packet","execution":"1","transfer":"2","packet":3}` |
| `owned-slot-cleanup-slot` | `RuntimeOwnedSlotId` | `exec/1/cleanup/2/slot/3` | `070100000000000000020000000000000003000000` | 21 | `{"kind":"cleanup_slot","execution":"1","scope":"2","slot":3}` |
| `path-root` | `RuntimeValuePath` | `[]` | `00000000` | 4 | `[]` |
| `path-record2-seq4-variant` | `RuntimeValuePath` | `[RecordField(2), SequenceElement(4), VariantPayload]` | `03000000030200000001040000000000000007` | 19 | `[{"kind":"record_field","field":2},{"kind":"sequence_element","index":"4"},{"kind":"variant_payload"}]` |
| `path-capture3-iterator6` | `RuntimeValuePath` | `[FunctionCapture(3), IteratorRemainder(6)]` | `020000000603000000080600000000000000` | 18 | `[{"kind":"function_capture","capture":3},{"kind":"iterator_remainder","index":"6"}]` |
| `path-witness-state` | `RuntimeValuePath` | `[IteratorWitnessState]` | `0100000009` | 5 | `[{"kind":"iterator_witness_state"}]` |
| `identity-snapshot` | `RuntimeExecutionIdentitySnapshotV2` | `execution1 cursors10/20/30/40` | `0100000000000000000a00000000000000001400000000000000001e00000000000000002800000000000000` | 44 | `{"execution":"1","next_occurrence":{"state":"next","value":"10"},"next_local_slot":{"state":"next","value":"20"},"next_ownership_transaction":{"state":"next","value":"30"},"next_affine_owner":{"state":"next","value":"40"}}` |
| `domain-snapshot` | `RuntimeExecutionDomainSnapshotV2` | `next_execution3 + epoch2 + core identity snapshot` | `00030000000000000002000000000000000100000000000000000a00000000000000001400000000000000001e00000000000000002800000000000000` | 61 | `{"next_execution":{"state":"next","value":"3"},"activation_epoch":"2","active":{"execution":"1","next_occurrence":{"state":"next","value":"10"},"next_local_slot":{"state":"next","value":"20"},"next_ownership_transaction":{"state":"next","value":"30"},"next_affine_owner":{"state":"next","value":"40"}}}` |

Hex strings contain no separators in the machine file. All multibyte values are little-endian.

## 2. Worked owned-slot bytes

### Environment local

```text
00                                      variant tag
01 00 00 00 00 00 00 00                 execution 1
02 00 00 00 00 00 00 00                 local slot 2
```

### AWBC register

```text
02                                      variant tag
01 00 00 00 00 00 00 00                 execution 1
02 00 00 00 00 00 00 00                 fiber 2
03 00 00 00 00 00 00 00                 frame 3
04 00 00 00                             register 4
```

## 3. Worked path bytes

`path-record2-seq4-variant`:

```text
03 00 00 00                             segment count 3
03 02 00 00 00                          RecordField(2)
01 04 00 00 00 00 00 00 00              SequenceElement(4)
07                                      VariantPayload
```

`path-capture3-iterator6`:

```text
02 00 00 00                             segment count 2
06 03 00 00 00                          FunctionCapture(3)
08 06 00 00 00 00 00 00 00              IteratorRemainder(6)
```

## 4. Identity snapshot bytes

`identity-snapshot` is 44 bytes:

```text
execution:                  u64 1
next_occurrence:            tag 0 + u64 10
next_local_slot:            tag 0 + u64 20
next_ownership_transaction: tag 0 + u64 30
next_affine_owner:          tag 0 + u64 40
```

`domain-snapshot` encodes `tag 0 + u64 3` for `next_execution`, then epoch u64 2, then the 44-byte core active identity snapshot, for 61 bytes total.

## 5. Invalid binary vectors

| Name | Target | Bytes | Required error |
|---|---|---|---|
| `execution-zero` | `ExecutionInstanceId` | `0000000000000000` | reject zero nonzero ID |
| `execution-truncated` | `ExecutionInstanceId` | `01010101010101` | reject wrong fixed length |
| `execution-trailing` | `ExecutionInstanceId` | `010000000000000000` | reject trailing byte |
| `cursor-unknown-tag` | `RuntimeIdCursor` | `02` | reject unknown tag |
| `cursor-next-zero` | `RuntimeIdCursor` | `000000000000000000` | reject zero next value |
| `cursor-exhausted-trailing` | `RuntimeIdCursor` | `0100` | reject trailing byte |
| `record-field-zero` | `RuntimeRecordFieldId` | `00000000` | reject zero field ID |
| `owned-slot-unknown-tag` | `RuntimeOwnedSlotId` | `08` | reject unknown variant tag |
| `owned-slot-env-zero-execution` | `RuntimeOwnedSlotId` | `0000000000000000000200000000000000` | reject zero nested execution |
| `owned-slot-env-zero-local` | `RuntimeOwnedSlotId` | `0001000000000000000000000000000000` | reject zero nested local |
| `path-unknown-tag` | `RuntimeValuePath` | `010000000a` | reject unknown segment tag |
| `path-record-zero` | `RuntimeValuePath` | `010000000300000000` | reject zero record field ID |
| `path-count-truncated` | `RuntimeValuePath` | `0200000007` | reject missing second segment |
| `path-trailing` | `RuntimeValuePath` | `0000000000` | reject trailing byte |
| `domain-zero-epoch` | `RuntimeExecutionDomainSnapshotV2` | `00030000000000000000000000000000000100000000000000000a00000000000000001400000000000000001e00000000000000002800000000000000` | reject zero activation epoch in driver envelope |
| `identity-zero-affine-cursor` | `RuntimeExecutionIdentitySnapshotV2` | `0100000000000000000a00000000000000001400000000000000001e00000000000000000000000000000000` | reject zero Next cursor |
| `domain-truncated` | `RuntimeExecutionDomainSnapshotV2` | `00030000000000000002000000000000000100000000000000000a00000000000000001400000000000000001e000000000000000028000000000000` | reject truncated active snapshot |
| `domain-trailing` | `RuntimeExecutionDomainSnapshotV2` | `00030000000000000002000000000000000100000000000000000a00000000000000001400000000000000001e0000000000000000280000000000000000` | reject trailing byte |

## 6. Invalid JSON vectors

| Name | Target | Input | Required error |
|---|---|---|---|
| `u64-number-token` | `ExecutionInstanceId` | `1` | reject numeric token |
| `u64-zero` | `ExecutionInstanceId` | `"0"` | reject zero |
| `u64-leading-zero` | `ExecutionInstanceId` | `"01"` | reject leading zero |
| `u64-plus` | `ExecutionInstanceId` | `"+1"` | reject sign |
| `u64-minus` | `ExecutionInstanceId` | `"-1"` | reject sign |
| `u64-space` | `ExecutionInstanceId` | `" 1"` | reject whitespace |
| `u64-overflow` | `ExecutionInstanceId` | `"18446744073709551616"` | reject overflow |
| `cursor-extra-field` | `RuntimeIdCursor` | `{"state":"exhausted","value":"1"}` | reject extra field |
| `cursor-duplicate-state` | `RuntimeIdCursor` | `{"state":"next","state":"exhausted","value":"1"}` | reject duplicate field |
| `owned-unknown-kind` | `RuntimeOwnedSlotId` | `{"kind":"other","execution":"1"}` | reject unknown kind |
| `owned-missing-field` | `RuntimeOwnedSlotId` | `{"kind":"environment_local","execution":"1"}` | reject missing local |
| `path-u64-number` | `RuntimeValuePath` | `[{"kind":"sequence_element","index":4}]` | reject numeric u64 index token |
| `path-zero-record` | `RuntimeValuePath` | `[{"kind":"record_field","field":0}]` | reject zero typed ID |
| `identity-driver-epoch-field` | `RuntimeExecutionIdentitySnapshotV2` | `{"execution":"1","activation_epoch":"2","next_occurrence":{"state":"next","value":"10"},"next_local_slot":{"state":"next","value":"20"},"next_ownership_transaction":{"state":"next","value":"30"},"next_affine_owner":{"state":"next","value":"40"}}` | reject driver-owned epoch as unknown core field |
| `domain-missing-epoch` | `RuntimeExecutionDomainSnapshotV2` | `{"next_execution":{"state":"next","value":"3"},"active":{"execution":"1","next_occurrence":{"state":"next","value":"10"},"next_local_slot":{"state":"next","value":"20"},"next_ownership_transaction":{"state":"next","value":"30"},"next_affine_owner":{"state":"next","value":"40"}}}` | reject missing activation_epoch |

## 7. Required golden test

For every valid vector:

1. decode the exact bytes/JSON;
2. compare the typed value and all private-field accessors;
3. encode again;
4. require exact byte/string equality; and
5. run on native plus every supported Web/headless/Agent codec consumer.

For every invalid vector, reject before activation and before any persistent cursor or slot mutation.

## 8. Ordering goldens

Owned-slot order:

```text
environment-local
closure-capture
awbc-register
awbc-frame-local
mailbox-lane
child-packet
transfer-packet
cleanup-slot
```

Path order:

```text
[]
[TupleElement(0)]
[TupleElement(0), VariantPayload]
[TupleElement(1)]
[SequenceElement(0)]
[RecordField(1)]
[RecordField(2)]
[NominalRecordField(1)]
[FunctionCapture(1)]
[VariantPayload]
[IteratorRemainder(4)]
[IteratorWitnessState]
```

These lists are compared against the manual `Ord` implementations; enum discriminant casts are not used.
