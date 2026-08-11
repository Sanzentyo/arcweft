# Worked canonical examples

All JSON below is a single canonical UTF-8 line with no BOM or trailing data. Line breaks
are added only around code fences. `RuntimePayload` keeps the existing serde shape; the
examples use its existing `RuntimeValue::String` representation.

## 1. Definition-key example

Accepted external operation facts:

| Field | Value |
| --- | --- |
| Origin | External (tag 0) |
| Module / ABI | `pkg.example.weather` / `33` repeated 32 bytes |
| Capability / operation | `weather` / `updates` |
| Callable ID / contract | `extern:pkg.example.weather/weather.updates` / `44` repeated 32 bytes |
| Item / error layout | `11`×32 / `22`×32 |
| Parameters | location: PositionalOnly+Required; units: PositionalOrNamed+Defaulted; fields: RestNamed+Optional |
| Effects | `net.read`, `weather.observe` in canonical order |

Transcript byte count: `394`  
Transcript hex:

```text
617263776566742e73747265616d2e646566696e6974696f6e2e7631000013706b672e6578616d706c652e776561746865723333333333333333333333333333333333333333333333333333333333333333077765617468657207757064617465732a65787465726e3a706b672e6578616d706c652e776561746865722f776561746865722e75706461746573444444444444444444444444444444444444444444444444444444444444444411111111111111111111111111111111111111111111111111111111111111112222222222222222222222222222222222222222222222222222222222222222030001086c6f636174696f6e55555555555555555555555555555555555555555555555555555555555555550000010105756e697473666666666666666666666666666666666666666666666666666666666666666601020201066669656c64737777777777777777777777777777777777777777777777777777777777777777040102086e65742e726561640f776561746865722e6f627365727665
```

BLAKE3-256 `RuntimeStreamDefinitionKey`:

```text
06c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd
```

Changing source range/file/display label leaves the key unchanged. Changing any listed
semantic/provider field changes it.

## 2. Canonical shared host open request

```json
{"type":"open","request":"1","instance":{"definition_key":"06c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd","generation":"7","ordinal":"42"},"definition":"06c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd","module":"pkg.example.weather","module_abi":"3333333333333333333333333333333333333333333333333333333333333333","capability":"weather","operation":"updates","arguments":{"values":[{"type":"value","value":{"String":"Tokyo"}},{"type":"value","value":{"String":"metric"}},{"type":"omitted"}]},"item_layout":"1111111111111111111111111111111111111111111111111111111111111111","error_layout":"2222222222222222222222222222222222222222222222222222222222222222","policy":{"backpressure":{"type":"latest_only"},"replay":"hash_only","privacy":"recordable","permission":"at_open","consumer_drop":"discard_queued","replay_retention":"through_tombstone","terminal_error_replay":"digest","restart":"same_provider","provider_replacement":"same_origin","limits":{"max_queue_items":"1","max_queue_bytes":"1048576","max_item_bytes":"1048576","max_replay_records":"16","max_replay_payload_bytes":"1048576","max_replay_total_bytes":"2097152","max_lifetime_events":"1000","max_lifetime_items":"900","max_recoverable_errors":"100","max_progress_events":"1000","max_deliveries":"1000","max_restart_attempts":"8","max_provider_replacements":"8"}}}
```

The three argument entries are declaration ordered: explicit `location`, materialized
default `units`, and omitted Optional rest-named `fields`. A JSON number in any Stream
integer field, including a nested limit, is invalid.

## 3. Canonical item event, outcome, observation, and close request

Item ingress:

```json
{"instance":{"definition_key":"06c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd","generation":"7","ordinal":"42"},"sequence":"8","kind":{"type":"item","type_layout":"1111111111111111111111111111111111111111111111111111111111111111","payload":{"String":"hello"}}}
```

Per-event accepted outcome:

```json
{"instance":{"definition_key":"06c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd","generation":"7","ordinal":"42"},"sequence":"8","disposition":{"type":"accepted","commit":"9"}}
```

Progress observation (control/telemetry, not a Stream item):

```json
{"instance":{"definition_key":"06c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd","generation":"7","ordinal":"42"},"commit":"10","kind":{"type":"progress","progress":{"phase":"running","completed":"3","total":"10","unit":"items"}}}
```

Close request:

```json
{"type":"close","request":"2","instance":{"definition_key":"06c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd","generation":"7","ordinal":"42"},"reason":"completed"}
```

Native, web, and Agent adapters must pass these exact bytes through the same core codec.

## 4. AWBC codec-8 record snippets

The snippets use one-byte canonical varints because every example ID is below 128.

| Record | Fields | Hex |
| --- | --- | --- |
| `AwbcParameter` | Some(string 5), type 7, PositionalOnly, Required | `01 05 07 00 00` |
| `AwbcRuntimeType::StreamHandle` | tag 21, item type 3, error type 4 | `15 03 04` |
| `OpenStream` | dst 2, definition 3, args [Value(reg4), Omitted] | `27 02 03 02 00 04 01` |
| `NextStream` | stream 2, dst 3, resume 4, ready block 5 | `8f 02 03 04 05` |
| `YieldStream` | stream 2, value 6, resume 4, continuation 7 | `90 02 06 04 07` |

For `OpenStream`, the bytes decompose as `27 02 03 02 00 04 01`: opcode, destination,
definition, vector length two, Value tag/register, Omitted tag.

## 5. HashOnly replay example

The existing canonical runtime-value bytes for `RuntimeValue::String("hello")` are:

```text
070500000068656c6c6f
```

The replay digest transcript binds definition key, instance generation/ordinal, record ID,
commit, ingress sequence, Item event/hash-domain tags, type layout, byte length, and those
payload bytes.

Transcript byte count: `155`  
Transcript hex:

```text
617263776566742e73747265616d2e7265706c61792e626f64792e76310006c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd07000000000000002a0000000000000003000000000000000900000000000000010800000000000000020011111111111111111111111111111111111111111111111111111111111111110a00000000000000070500000068656c6c6f
```

BLAKE3-256 digest stored in `StreamReplayRecordBody::Digest`:

```text
947ea4e1562aaadb16a5541cb6f9d229a9ebbe451e9aa40dda0564e30f9a14eb
```

The record stores this digest and byte count but not `hello` or its canonical bytes.

## 6. Canonical save-schema-2 Stream table fragment

This is the complete `streams` field for one releasable End tombstone. It is not an
ellipsis or pseudo-JSON.

```json
{"next_instance_ordinal":"43","next_consumer_lease":"6","next_producer_lease":"2","next_request_id":"3","next_tombstone_ordinal":"2","total_queue_bytes":"0","total_replay_bytes":"0","entries":[{"kind":"tombstone","entry":{"key":{"definition_key":"06c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd","generation":"7","ordinal":"42"},"definition":"0","item_layout":"1111111111111111111111111111111111111111111111111111111111111111","error_layout":"2222222222222222222222222222222222222222222222222222222222222222","consumer":{"type":"dropped","final_lease":"5"},"terminal":{"commit":"3","ingress":"3","reason":{"type":"end"},"result_emitted":true,"observation_emitted":true},"close":{"type":"acknowledged","id":"2"},"replay":{"next_record_id":"2","records":[{"id":"1","commit":"3","ingress":"3","event":"end","body":{"type":"event_only"},"accounted_bytes":"0"}],"retained_payload_bytes":"0","retained_total_bytes":"0","stored_records":"1","evicted_records":"0","skipped_records":"0"},"counters":{"consumed_envelopes":"4","accepted_items":"1","accepted_recoverable_errors":"0","progress_events":"1","delivered_items":"1","delivered_recoverable_errors":"0","overflow_drop_oldest":"0","overflow_drop_newest":"0","overflow_coalesced":"0","restarts":"0","provider_replacements":"0"},"last_error":null,"ordinal":"1","pins_generation":false}}]}
```

Restore first validates sorted unique keys and global cursors/accounting, then local
state/replay, then scans every fiber/runtime value for affine handles and reciprocal
producer references. Only after all stages pass is the candidate swapped into the active
executor.

## 7. Exhaustion transition example

Resolved `max_lifetime_items = 2` and external cursor starts at sequence 0:

| Input | Before | Commit | After |
| --- | --- | --- | --- |
| Item seq 0 | accepted_items 0 | enqueue item, commit 0 | accepted_items 1, cursor Next(1) |
| Item seq 1 | accepted_items 1 | enqueue item, commit 1 | accepted_items 2, cursor Next(2) |
| Item seq 2 | accepted_items 2 (limit reached) | consume envelope only; no payload/queue mutation; terminalize RuntimeLimit::LifetimeItems at commit 2; emit result/observation/one close request | terminal, cursor Next(3) |
| Repeated seq 3 | terminal | Rejected(Terminal) only | table, queue, terminal flags, request IDs, counters byte-identical |

At external cursor `Next(u64::MAX)`, the matching MAX envelope follows the same
terminalization pattern with `RuntimeLimit::EventSequence` and cursor `Exhausted`.
Malformed/stale/wrong-typed envelopes never use terminalization; they are rejections with
no mutation.

## 8. Dropped-consumer example

Before drop: queue `[delivery 4 Item("hello"), delivery 5 RecoverableError("retry")]`,
privacy Redacted, replay Full, policy DrainAndRetainReplay, ThroughTombstone.

The registry performs one synchronous staged transition:

1. mark consumer Dropped/Pending;
2. project delivery 4 to Summary (Redacted forbids Payload);
3. project delivery 5 to Summary;
4. remove and drop both queue payload owners in FIFO order;
5. set queue bytes to zero and cleanup Complete;
6. emit exactly one external close or local producer cancellation;
7. after producer/close settlement, replace Live with Tombstone carrying the bounded
   summaries.

A repeated drop/cleanup adds no replay record, counter, close request, cancellation, or
observation. No consumer is required to drain the now-unobservable queue.

## 9. Bundle Stream fingerprint example

Transcript byte count: `232`  
Transcript hex:

```text
617263776566742e62756e646c652e73747265616d2d66696e6765727072696e742e763100aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0106c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725ddbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc0000ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
```

BLAKE3-256 fingerprint:

```text
7ee9a40fbd4025897267158027764cca41dba79dce5b56d14fec7283764600b3
```

The `aa`, `bb`, `cc`, `dd`, and `ee` repeated-byte digests stand respectively for the
complete existing AWBC executable identity, handle layout, state layout, resolved profile,
and adapter requirements. The definition key already binds callable/provider/effect and
item/error layout facts.

## 10. Hard rejection examples

- AWBC magic followed by codec version 7: `UnsupportedCodec { actual: 7, expected: 8 }`;
- codec 8 header with ABI 1: `UnsupportedAbi { actual: 1, expected: 2 }`;
- codec 8 function-kind tag 4 or instruction `0x1e`: `UnknownTag` at the tag offset;
- bundle schema 5 or `awbc_v1`: outer bundle error before AWBC dispatch;
- save envelope schema version 1: unsupported schema before payload migration lookup;
- JSON `"generation":7`, `"generation":"07"`, duplicate `generation`, or BOM:
  strict shared JSON rejection before state lookup.
