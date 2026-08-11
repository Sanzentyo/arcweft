# Test matrix

This matrix is normative implementation work, not a record of tests run by this
design package. Every row must be implemented with typed APIs, direct behavior,
canonical codecs, compile-fail fixtures, structured projections, or Cargo
metadata. Source-text searching is not a correctness gate.

| ID | Area | Requirement | Method | Required result |
|---|---|---|---|---|
| ID-001 | Identity | Exact `tts_speaker.game.akane` constructs and round-trips. | unit + serde | Value is byte-identical and ordered/hashable. |
| ID-002 | Identity | `character.game.akane` is rejected as `TtsSpeakerId`. | unit | `WrongFamily` with expected `tts_speaker`; no conversion. |
| ID-003 | Identity | `speaker.game.akane`, device, bus, and provider-key-shaped strings are rejected. | table unit | All fail nominal construction. |
| ID-004 | Identity | A 256-byte speaker ID is accepted and 257 bytes is rejected. | boundary unit | Exact one-over error fields. |
| ID-005 | Identity | Speaker/profile scalar budget is enforced independently of UTF-8 bytes. | Unicode boundary unit | No allocation/panic; exact counts. |
| ID-006 | Identity | Provider IDs require `tts_provider.` plus lowercase local grammar. | table unit | Uppercase and wrong family fail. |
| ID-007 | Identity | Provider ID 96/97-byte boundary. | boundary unit | 96 accepted; 97 rejected. |
| ID-008 | Identity | Profile IDs require `tts_voice_profile.*`; Character/profile IDs cannot cross-deserialize. | serde negative | Wrong-family decode fails. |
| ID-009 | Provider key | Empty, NUL, control, CR/LF, and surrounding whitespace fail. | table unit | Exact `TtsProviderKeyError`; value never formatted. |
| ID-010 | Provider key | Provider key 256-byte/128-scalar and one-over limits. | boundary unit | Independent byte/scalar enforcement. |
| ID-011 | Serde | All nominal string types deserialize only through constructors. | serde tamper | Malformed/wrong-family/oversized forms fail. |
| ID-012 | Redaction | Provider key and SecretRef `Debug` do not expose restricted suffix/value. | format unit | Sentinel secret/key absent from output. |
| ID-013 | Locale | Canonical locale cases (`ja-JP`, `zh-Hant-TW`) pass; underscore/case/extensions fail. | table unit | Exact BCP subset and 64-byte cap. |
| ID-014 | Style/option | Lower local style/option IDs and 64/65-byte boundary. | boundary unit | Canonical forms only. |
| ID-015 | SecretRef | `secret.tts.acme` passes; raw/token-like and non-reference values fail. | unit + manifest decode | Only locator accepted. |
| ID-016 | Fingerprint | Canonical request fingerprint is stable across process runs and independent map insertion order. | golden vector | Exact 32-byte golden digest. |
| ID-017 | Fingerprint | Task/Need/cancel IDs, attempt, progress, clock, trace ID, result bytes do not alter fingerprint. | property unit | All excluded fields leave digest unchanged. |
| ID-018 | Fingerprint | Text byte, selected binding, provider-key digest, option, catalog/ABI/artifact change alters fingerprint. | property unit | Each included field changes digest. |
| MAP-001 | Profile | Canonical profile record validates locales/defaults/rate/pitch. | unit | Accepted immutable profile. |
| MAP-002 | Profile | Profile locale order is retained and duplicates fail. | unit | First locale remains default; duplicate diagnostic. |
| MAP-003 | Character mapping | One Character with one default and multiple priorities validates. | catalog unit | Exact sorted Character group. |
| MAP-004 | Character mapping | Character group with profiles and no default fails. | catalog negative | `missing-character-default`. |
| MAP-005 | Character mapping | Two defaults for one Character fail. | catalog negative | Structured conflicting profiles/ranges. |
| MAP-006 | Character mapping | Duplicate Character selection priority fails. | catalog negative | First/later profile evidence. |
| MAP-007 | Character mapping | One profile bound to two Characters fails. | catalog negative | No shared implicit Character identity. |
| MAP-008 | Selection | Explicit profile selects exactly that profile. | request preparer | No lexical/default substitution. |
| MAP-009 | Selection | Explicit Character+profile requires exact binding. | request preparer negative | `CharacterProfileMismatch`. |
| MAP-010 | Selection | Character without explicit profile and locale selects lowest eligible unique priority. | request preparer | Exact expected profile. |
| MAP-011 | Selection | Character without locale selects the exact default. | request preparer | Default marker, not lexical order. |
| MAP-012 | Selection | Missing Character profile fails. | request preparer negative | `MissingCharacterProfile`. |
| MAP-013 | Selection | Ambiguous/corrupt mapping is rejected rather than picking first. | defensive catalog tamper | `mapping-ambiguous`. |
| MAP-014 | Selection | Direct logical speaker request requires explicit locale. | type/lowering negative | Missing argument diagnostic. |
| MAP-015 | No fallback | Character ID text cannot become speaker/profile/provider key. | typed runtime negative | Unknown mapping even when texts match. |
| MAP-016 | No fallback | Display name, declaration name, alias, and variable name cannot select TTS. | integration negative | No provider dispatch; typed mapping error. |
| MAP-017 | Locale | No language-only or region fallback occurs. | request preparer negative | `en` does not select `en-US`; exact failure. |
| MAP-018 | Options | Profile defaults apply only when request option absent; explicit options override exactly. | unit | Expected selected values. |
| MAP-019 | Options | Unsupported style/rate/pitch/format is rejected; no clamping/fallback. | table preparer negative | Typed unsupported error. |
| MAP-019A | Format selection | Absent format chooses the first provider-supported format in WAV/FLAC/OGG/MP3/AAC order. | request preparer table | Chosen format is deterministic and fingerprinted. |
| MAP-020 | Provider binding | One logical speaker may have multiple providers/locales with unique priority. | catalog unit | Canonical binding order. |
| MAP-021 | Provider binding | Same provider key cannot bind two logical speakers. | catalog negative | Redacted duplicate-key diagnostic. |
| MAP-022 | Provider binding | Binding locale/style outside generated capabilities fails. | accepted join negative | Capability mismatch with ranges. |
| MAP-023 | Provider selection | Availability filtering selects next pre-dispatch eligible priority and pins that snapshot. | preparer + snapshot | Selected evidence matches snapshot; no later live reselection. |
| MAP-024 | Provider selection | No eligible available provider yields typed provider-unavailable. | preparer negative | No adapter call. |
| MAP-025 | Provider selection | After dispatch, retry remains on same provider/key. | scripted executor | No cross-provider event. |
| SRC-001 | Typed res | Valid `std.audio.TtsVoiceProfile` resource type-checks and lowers. | parser/HIR/sema/lowering | Exact profile and optional Character binding. |
| SRC-002 | Typed res | `character` field uses retained `CharacterRef`, not `ResourceRef<Character>`. | compile-fail/API | Wrong wrapper rejected. |
| SRC-003 | Typed res | Wrong-family profile public ID fails at identity boundary. | sema negative | Stable identity diagnostic/range. |
| SRC-004 | Typed res | Duplicate field, unknown field, missing required speaker/locales fail. | table parser/sema | Current typed-resource diagnostics. |
| SRC-005 | Grammar | No dedicated `voice`, `voice profile`, `speaker`, or TTS declaration parses. | parser negative fixture | Ordinary current grammar error; no removed-name code. |
| SRC-006 | Function | All three standard signatures accept valid typed arguments and return `Need<TtsAudioAsset,TtsError>`. | sema + lowering | Exact nominal return/effect. |
| SRC-007 | Function | Character-valued argument is named `character`. | signature golden | No `speaker` spelling. |
| SRC-008 | Function | Logical speaker argument is named `tts_speaker`. | signature golden | No `speaker`/`voice` spelling. |
| SRC-009 | Function | `speaker=` and `voice=` named arguments are ordinary unknown arguments. | compile-fail fixture | No removed-name diagnostic. |
| SRC-010 | Function | `tts.synthesis` is an ordinary unknown call. | compile-fail fixture | No alias or special message. |
| SRC-011 | Function | Source cannot construct `TtsProviderId`/provider key/SecretRef for synthesis. | visibility + compile-fail | No public callable path. |
| SRC-012 | Options | Extension options use the closed scalar value enum; raw record/JSON is rejected. | type-check negative | No untyped payload. |
| SRC-013 | Limits | Empty text, exact max text, byte one-over, scalar one-over. | source/runtime boundary | Exact RequestLimit fields. |
| SRC-014 | Effects | Calling TTS without declaring/authorizing `tts.synthesize` fails. | effect sema + runtime policy | No task dispatch. |
| SRC-015 | Tooling | Signature help lists exact order/names/defaults/return/effect. | LSP golden | No old names. |
| SRC-016 | Tooling | Hover and go-to-definition resolve profile and Character owners. | LSP integration | Provider metadata remains non-source. |
| SRC-017 | Tooling | Completion proposes `res` descriptor/function fields only. | LSP completion | No removed keywords/provider key. |
| MAN-001 | Manifest | Schema-1 manifest with no `tts` decodes to empty TTS spec. | sole decoder | Backward absence without alternate reader. |
| MAN-001A | Non-blocking catalog | Profile/dialogue metadata publishes with an empty provider catalog when executable TTS is not authorized. | accepted topology integration | Profile catalog valid; dialogue projection unaffected. |
| MAN-002 | Manifest | Canonical provider/binding manifest decodes with exact source ranges. | sole decoder + source map | All table/key/value/array-element ranges present. |
| MAN-003 | Manifest | Duplicate provider table/key is rejected with first/later ranges. | decoder negative | No last-wins behavior. |
| MAN-004 | Manifest | Duplicate binding table/key is rejected with first/later ranges. | decoder negative | No coalescing. |
| MAN-005 | Manifest | Unknown TTS/provider/binding/public-config structural field is rejected. | decoder negative | Strict closed schema. |
| MAN-006 | Manifest | Malformed/wrong-family/oversized provider/binding/speaker/key fails. | table negative | Stable typed diagnostics. |
| MAN-007 | Manifest | Inline credential value, multiline secret, table, or array in `credential-ref` fails. | decoder negative | `secret-value-forbidden`; no echo. |
| MAN-008 | Manifest | Valid SecretRef is retained without resolving value during decode. | decoder + fake resolver | Resolver call count remains zero. |
| MAN-009 | Manifest | Missing module import/export/provider metadata fails accepted join. | join negative | Atomic candidate rejected. |
| MAN-010 | Manifest | Artifact identity mismatch fails. | join tamper | `adapter.artifact-mismatch`. |
| MAN-011 | Manifest | ABI mismatch fails. | join tamper | `adapter.abi-mismatch`. |
| MAN-012 | Manifest | Raw metadata and payload hash mismatch independently fail. | join tamper | Expected/actual structured hashes. |
| MAN-013 | Metadata | Generated `tts_providers` canonical round trip. | codec golden | Byte-identical canonical JSON. |
| MAN-014 | Metadata | Unsorted/duplicate provider, locale, style, format, option schema, or config fields fail. | codec negative | Decoder does not sort. |
| MAN-015 | Metadata | Unknown field/enum/protocol/transport and schema 0/2 fail. | codec negative | Only final schema 1 accepted. |
| MAN-016 | Metadata | TTS field mutation changes payload and ABI digest. | digest property | Both semantic identities bind field. |
| MAN-017 | Metadata | Provider identity in manifest and generated export must match. | join negative | No string fallback. |
| MAN-018 | Metadata | Accepted target family is derived from the enclosing AdapterTarget; no duplicate TTS transport field exists. | join + provider-codec tamper | Rust/Wasm/Process coordinate matches target; tampered catalog fails. |
| MAN-019 | Config | Unknown public config key/type/range/enum value fails with source range. | join table negative | No provider-specific parser. |
| MAN-020 | Capability | Binding locale/style must be a subset of generated capability inventory. | join negative | Exact unsupported diagnostic. |
| MAN-020A | Capability | Extension option ID/type/range/enum is validated against generated provider schema. | join/preparer table | Unsupported or invalid option rejects candidate before dispatch. |
| MAN-021 | Atomicity | Any late TTS join error publishes none of profile/provider/callable/capability topology. | transaction integration | Prior accepted generation unchanged. |
| MAN-022 | Historical | Manifest `speaker` field is a generic unknown field. | decoder negative | No dedicated removed-name diagnostic. |
| MAN-023 | Parser ownership | All loader entry points consume the same typed `SourceBackedManifest` result. | API integration | No direct provider TOML/JSON decode path. |
| COD-001 | Profile codec | Canonical profile catalog encode/decode/re-encode is byte-identical. | golden round trip | Section code 22/schema 1. |
| COD-002 | Provider codec | Canonical restricted provider catalog round trip is byte-identical. | golden round trip | Section code 23/schema 1. |
| COD-003 | Ordering | Unsorted profile/provider/binding/Character records fail. | tamper table | No decoder sorting. |
| COD-004 | Duplicate | Every duplicate ID, locale, style, priority, provider-key coordinate fails. | tamper table | Exact duplicate error. |
| COD-005 | Truncation | Truncate at every fixed header and length-prefixed field boundary. | generated tamper | Always `Truncated`, never panic/allocate over budget. |
| COD-006 | Trailing | Append one byte to each complete catalog payload. | tamper | Trailing data rejected. |
| COD-007 | Unknown | Unknown enum/bool/option discriminant fails. | tamper | No unknown preservation. |
| COD-008 | Noncanonical | Bool 2, option 2, invalid UTF-8, noncanonical identifier/case fail. | tamper | Exact rejection. |
| COD-009 | Lengths | Declared count/length overflow and allocation multiplication overflow fail. | tamper/fuzz property | Budget check before allocation. |
| COD-010 | One-over | Profile/provider/binding/string/payload count exactly one over each limit fails. | boundary generated | Exact limit error. |
| COD-011 | Digest | Inner semantic digest mutation and content mutation independently fail. | tamper | Digest mismatch. |
| COD-012 | Artifact | AWFB stored/content/index/manifest/artifact binding tamper fails. | container tamper | Existing container errors. |
| COD-013 | Required section | Executable TTS call without profile/provider required section fails. | bundle validation | No launch topology. |
| COD-014 | Privacy | Profile section contains no provider/key/credential; public summaries omit provider section restricted fields. | typed decode/projection | Sentinel absent from projections. |
| COD-015 | Schema | Section schema 0/2 and wrong magic fail; no alternate reader. | tamper | Only schema 1. |
| COD-016 | Unknown section | Known TTS section with unknown inner field fails even when outer section optional. | container+inner negative | Closed inner schema. |
| PRO-001 | AWTP | Canonical 32-byte header and every frame kind golden vector. | codec golden | Exact bytes. |
| PRO-002 | AWTP | Bad magic/version/nonzero flags/unknown kind fail. | tamper table | No event emitted. |
| PRO-003 | AWTP | Payload over per-kind maximum and global request maximum fails before allocation. | boundary tamper | Protocol failure. |
| PRO-004 | AWTP | Truncated header/payload and trailing bytes fail. | tamper | No partial acceptance. |
| PRO-005 | Negotiation | Matching provider/export/artifact/ABI/capability/protocol succeeds. | scripted process/Wasm | Executor published. |
| PRO-006 | Negotiation | Each mismatching coordinate independently fails publication. | table negative | Exact mismatch code. |
| PRO-007 | Request | Canonical provider request round trip retains exact text/key/options and credential slot only. | codec round trip | No credential value. |
| PRO-008 | Chunks | Contiguous chunks from sequence zero complete successfully. | state-machine unit | Expected bytes/digest. |
| PRO-009 | Chunks | Gap, duplicate, out-of-order, or chunk after terminal fails. | state-machine negative | ProtocolFailure. |
| PRO-010 | State | Completed/Failed/Cancelled before Accepted fails. | state-machine negative | Exact stage/code. |
| PRO-011 | State | Second terminal event or progress regression fails/discards as specified. | state-machine negative | One terminal outcome. |
| PRO-012 | Completed | Chunk count, total bytes, and digest mismatch independently fail. | tamper | No result publication. |
| PRO-013 | Parity | Typed Rust, typed Wasm component, and process AWTP execute identical semantic vectors. | cross-transport simulation | Same public outcome/progress/errors; AWTP bytes are process-only. |
| ADP-001 | Success | Scripted executor success yields complete validated `TtsAudioAsset`. | host integration | Ready after validation only. |
| ADP-002 | Audio validation | Wrong format, empty, corrupt, changing stream, >2 channels, sample/duration limits fail. | audio-codec integration | Typed `InvalidAudio`. |
| ADP-003 | Result limit | 32 MiB exact accepted when valid; first byte over aborts/spool cleans. | boundary integration | `ResultTooLarge`. |
| ADP-004 | Buffering | 4 MiB threshold selects memory/spool exactly and materializes identical result. | fake spool integration | No source-visible path. |
| ADP-005 | Progress | Phase/value progress is monotonic and correctly mapped to Task/Need. | scripted events | Typed progress payload. |
| ADP-006 | Progress | More than 128 progress events coalesce deterministically. | stress unit | At most 128 published, final values retained. |
| ADP-007 | Cancellation | Queued cancellation removes task and releases generation pin/lease state. | manual scheduler | Cancelled; executor not called. |
| ADP-008 | Cancellation | Active cooperative/abortable cancellation sends once and cleans. | scripted executor | One Cancelled terminal. |
| ADP-009 | Cancellation | Provider with no cancellation is cut off/aborted at 5 s cleanup deadline. | manual clock | Late completion discarded. |
| ADP-010 | Cleanup | Completion/failure/protocol failure/cancel/timeout/retry/adapter loss/shutdown all release lease/spool/buffer. | fault-injection matrix | Zero leaked owned resources. |
| ADP-011 | Timeout | Queue wait counts toward timeout. | manual clock | Timeout before submit. |
| ADP-012 | Timeout | Active timeout cancels/aborts and emits exact typed fields. | manual clock | No retry after deadline. |
| ADP-013 | Retry | Retryable RateLimited/Unavailable/Transport with idempotency uses 250 ms then 1000 ms. | manual clock script | At most configured 1–3 attempts. |
| ADP-014 | Retry | No idempotency, authentication, authorization, invalid request, internal nonretryable do not retry. | table script | One attempt. |
| ADP-015 | Retry | Each retry uses same provider/binding/key digest/fingerprint/config/artifact. | executor capture | All coordinates equal. |
| ADP-016 | Retry | Partial bytes and prior lease are destroyed before next attempt. | fault script | No concatenation/leak. |
| ADP-017 | Concurrency | Global active limit 32 and one-over queueing behavior. | stress simulation | Never >32 active. |
| ADP-018 | Concurrency | Per-provider min(metadata, host) limit and fair other-provider progress. | stress simulation | Per-provider cap respected. |
| ADP-019 | Queue | Queue cap 256; 257th fails without evicting earlier tasks. | stress simulation | Typed QueueLimit. |
| ADP-020 | Queue | Deterministic priority/epoch/sequence/TaskId ordering. | simulation golden | Exact dispatch order. |
| ADP-021 | Registry | Duplicate executor ownership and policy call without implementation fail. | builder unit | Existing structured registry errors. |
| ADP-022 | Secret | Secret resolution occurs only after all policy/catalog/queue checks. | mock resolver ordering | Call count/order exact. |
| ADP-023 | Secret | Missing/failed credential resolves to typed provider/capability failure with no dispatch. | mock negative | No secret/value in output. |
| ADP-024 | Redaction | Sentinel provider key/credential/text/audio bytes absent from request/result Debug, logs, metrics labels, diagnostics, Agent/MCP. | capture integration | No sentinel occurrence in structured projections. |
| ADP-025 | Provider error | Structured provider class/code/message/retryable maps exactly and sanitizes control/secrets/text. | table script | Bounded typed ProviderFailure. |
| RUN-001 | Task identity | Same selected request produces exact `tts.v1.<64hex>` TaskKey. | unit golden | Stable TaskKey. |
| RUN-001A | Preparation boundary | Typed TTS template emits only the intent variant; `dispatch_requested_tasks` prepares it before registry/HostTaskDispatch. | runtime-driver integration | Final dispatch contains selected request and final TaskKey; host never sees intent. |
| RUN-001B | Preparation failure | Catalog/availability preparation failure queues typed Err for the same epoch/sequence and emits no HostTaskDispatch. | runtime-driver negative | Need reaches exact TtsError; executor call count zero. |
| RUN-002 | Task join | Identical requests JoinSameKey and receive one provider call/result. | scheduler integration | One call, two Need observers. |
| RUN-003 | Task separation | Text/locale/option/binding/catalog/availability/artifact difference prevents join. | table scheduler | Distinct task keys. |
| RUN-004 | Nondeterminism | Same fingerprint may yield different valid provider bytes; result digests record each without claiming equality. | two scripted runs | Both accepted, fingerprints same, digests differ. |
| RUN-005 | Typed error | Provider failure reaches `Need::Err(TtsError)` without string fallback. | end-to-end | Nominal layout exact. |
| RUN-006 | Replay | Recorded success injects exact bytes and suppresses provider dispatch. | record/replay integration | Zero executor calls; digest validates. |
| RUN-007 | Replay | Missing/omitted TTS result bytes fail replay instead of network fallback. | replay negative | Explicit replay error. |
| RUN-008 | Replay | Typed provider failure round-trips under corrected replay schema 1. | record/replay | Exact error payload. |
| RUN-009 | Save | Active TTS task yields existing HostTasks save blocker. | session integration | No TTS-specific snapshot. |
| RUN-010 | Save | Queued events/generation pins also block save. | session integration | Existing blocker coordinates. |
| RUN-011 | Save | Completed retained asset saves/restores under per-asset limit. | save round trip | Exact bytes/receipt/digest. |
| RUN-012 | Save | Per-asset and 256 MiB aggregate one-over fail before encode allocation. | boundary save | Structured limit error. |
| RUN-013 | Hot reload | Active request pins old generation and completes through old executor. | reload simulation | No mid-flight remap. |
| RUN-014 | Hot reload | Queued request migrates only when full compatibility tuple equal. | reload table | Same TaskKey/selected evidence. |
| RUN-015 | Hot reload | Each tuple mismatch cancels queued request with CatalogChanged. | table simulation | No silent reselection. |
| RUN-016 | Atomicity | Rejected candidate catalog leaves prior active generation intact. | reload negative | Prior new requests still succeed. |
| RUN-017 | Non-blocking | Dialogue/View character projection builds/runs/saves/observes with no TTS catalog/capability/provider/credential. | integration | No TTS lookup; projection valid. |
| RUN-018 | Non-blocking | Unavailable TTS only fails the TTS Need and does not mutate dialogue/View state. | integration | Dialogue observations unchanged. |
| RUN-019 | Debug | Ordinary debug label contains only text byte count/locale/format. | format unit | No sensitive/public voice IDs. |
| RUN-020 | Privacy | Default Agent/MCP/accessibility/capture omit TTS profile/speaker/provider/key/text/bytes/digest. | projection integration | Only allowed safe summary where explicit API used. |
| RUN-021 | Privacy | Privileged audio debug may show profile/speaker/provider/fingerprint prefix but not key/secret/text/bytes. | policy integration | Exact field allow-list. |
| STR-001 | Dependency | `cargo metadata` proves allowed graph and all forbidden edges absent. | metadata assertion test | Exact graph in ownership document. |
| STR-002 | Dependency | Provider SDK/process/network/secret crates are absent from lower TTS/audio/core/manifest/metadata crates. | metadata feature/package audit | No forbidden dependency. |
| STR-003 | I/O boundary | Sans-I/O simulation constructs/selects/encodes without clock/fs/env/network calls. | test harness with no host services | All lower tests pass. |
| STR-004 | Ownership | Exactly one HostAdapter owns `tts.synthesize`; missing/duplicate implementation fails. | registry typed API | No dispatch ambiguity. |
| STR-004A | Stage visibility | Host adapter and replay APIs reject/are unconstructible from `TtsSynthesisIntent`; only selected requests cross the boundary. | trybuild + typed API | No unprepared TTS task can dispatch or persist. |
| STR-005 | Parser ownership | All manifest consumers require the accepted typed decoder output and immutable handles. | compile/API integration | No provider-specific reader API exists. |
| STR-006 | Visibility | Facade re-exports only source-facing profile/options/result/error; provider key/secret/protocol internals inaccessible. | trybuild/public API test | Expected compile failures. |
| STR-007 | Owning behavior | Validation/formatting/task-key methods are inherent on owning Arcweft types. | API review + trybuild | No extension trait/helper substitute in public API. |
| STR-008 | No legacy API | Old Rust request struct, old named args, old metadata fields, and old wire constructors fail to compile/decode. | trybuild + decoder negative | No compatibility interval. |
| STR-009 | Formatting | `cargo fmt --all --check`. | command | Pass. |
| STR-010 | Check | `cargo check --workspace --all-targets --all-features`. | command | Pass. |
| STR-011 | Clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings`. | command | Pass. |
| STR-012 | Workspace tests | `cargo test --workspace --all-targets --all-features`. | command | Pass. |
| STR-013 | Reproducibility | Generated metadata/catalog/bundle/protocol golden artifacts rebuild byte-identically. | two clean builds | SHA-256 equality. |
| STR-014 | Tier 2 | Applicable repository Tier 2 and native/Web/headless parity suites. | project commands | Pass or exact owner-based N/A record. |
| STR-015 | Fuzz/property | Bounded codec/property harness covers arbitrary bytes without panic/unbounded allocation. | property/fuzz corpus | All malformed input rejected safely. |

## Matrix accounting

```text
NORMATIVE_ROWS=179
EXECUTED_BY_THIS_PACKAGE=0
IMPLEMENTATION_REQUIRED=179
```

Focused test modules should preserve these IDs in test names or case metadata so
implementation evidence can be joined back to this matrix without relying on
source-text occurrence counts.
