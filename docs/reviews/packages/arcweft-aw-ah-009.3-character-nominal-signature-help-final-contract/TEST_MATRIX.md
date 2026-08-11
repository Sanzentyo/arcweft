# Direct test matrix

All tests assert typed enums, IDs, ranges, indices, and structured fields.
No test or script reads checked-in source or documentation to search for a
symbol, spelling, snippet, module path, or file location.

## 1. Surface and active-parameter tests

| ID | Setup and cursor | Exact expected result |
| --- | --- | --- |
| S01 | `show(@character.alice, |)` | one presentation signature; active parameter `look`; `look.expectation == Known(CharacterNominal(look(Alice)))` |
| S02 | `show(|@character.alice)` | active parameter `character`; structural look remains Alice after semantic fact collection |
| S03 | `show(@character.alice, look=|.happy)` | active parameter `look`; canonical Alice look type |
| S04 | `show(@character.alice, target=|@target.stage)` | active parameter `target`; no look-spelling inference |
| S05 | `hide(|@character.alice)` | active parameter `character`; no character nominal parameter in signature |
| S06 | `ref.show(|@character.alice)` | active parameter `character`; no synthetic look parameter |
| S07 | `view(|@view.dialogue)` | active parameter `view`, known `Ref<View>` |
| S08 | `menu(@view.menu, depth=|0)` | active parameter `depth`, known `I32` |
| S09 | `overlay(@view.o, visible=|true)` | active parameter `visible`, known `Bool` |
| S10 | `bg(|@asset.room)` | active asset/source parameter |
| S11 | `image(@asset.hero, opacity=|1.0)` | active `opacity`; typed or explicit unconstrained schema value according to current semantic rule |
| S12 | `player_viewport(width=|1280)` | active `width`; deterministic player-viewport candidate ID |
| S13 | `clear.bg(target=|@target.main)` | active `target` |
| S14 | colon line `alice(look=|.happy): Hi` | one dialogue `SpeakerLine` signature; active `look`; Alice nominal |
| S15 | canonical content call `alice.say(look=|.happy)[Hi]` | one `ContentCall` signature; active `look`; Alice nominal |
| S16 | `alice.say(|)[Hi]` | empty list active next option; when Alice is character, active `look` according to schema order |
| S17 | colon line `alice: Hi` with cursor at colon | `NotApplicable(CursorOutsideArgumentList)` |
| S18 | cursor in dialogue content brackets | no signature help |
| S19 | current inline `[move ...]` tag | `NotApplicable(UnsupportedSurface)` |
| S20 | accepted selected method `actor.stage.show(|...)` | method candidate from receiver type; correct first parameter |
| S21 | documented but unregistered `actor.stage.move(|...)` | `NotApplicable(UnknownCallee)`; no synthetic method |
| S22 | project source function with character nominal parameter | project candidate ID and typed nominal active parameter |
| S23 | extern capability function with character nominal parameter | accepted project/environment candidate and typed nominal |
| S24 | adapter-only function with typed nominal parameter | one environment adapter candidate; typed nominal preserved |
| S25 | enum variant constructor whose payload is character nominal | enum-variant candidate; payload active and typed |
| S26 | `Ok(|value)` under expected `Result<CharacterLook, E>` | result constructor; active payload is Alice look nominal |
| S27 | function value of type `fn(CharacterLook<Alice>) -> Unit` | `FunctionValue` candidate; generated display name `arg1`; typed nominal |
| S28 | curried function second call group | parameters come from `remaining_param_group(0)`, not first group |
| S29 | flow reference in `goto` | no signature help; flow is not synthesized as a function |
| S30 | source `impl` method absent from project method catalog | no signature help unless accepted ordinary method lookup resolves it |

## 2. Character identity and spelling tests

| ID | Setup | Exact expected result |
| --- | --- | --- |
| C01 | canonical owner spelling | result candidate owner is canonical `CharacterId`; type label is canonical `source_label()` |
| C02 | compact supported owner spelling | same typed owner as canonical; authored callee/argument text remains compact in display |
| C03 | qualified project spelling | same typed owner; qualified authored display retained |
| C04 | authored alias `hero` for Alice | alias displayed; candidate/type identity remains Alice; docs include canonical owner |
| C05 | Alice look `happy` and Bob look `happy` | distinct `CharacterNominalType` values and candidate expectations |
| C06 | Alice look `happy` and Alice `face`-part variant `happy` | distinct family/part identity |
| C07 | Alice `face` and `body` part variants both named `happy` | distinct part identity |
| C08 | two overloads differ only by Alice/Bob nominal | semantic argument type selects the correct overload; label spelling is irrelevant |
| C09 | spoofed alias equal to another owner's source label | resolver follows typed symbol binding; no label parser or cross-owner selection |
| C10 | missing `show` character argument | partial `show` help; look type `Unavailable(UnknownCharacterOwner)` and typed diagnostic |
| C11 | character argument resolves to non-character | same unavailable owner behavior; never global look lookup |
| C12 | accepted callable references an unknown part | unavailable part reason retains the exact nominal; partial help |

## 3. Positional, named, reordered, duplicate, and spread tests

| ID | Setup and cursor | Exact expected result |
| --- | --- | --- |
| A01 | `f(|a, b)` | first positional parameter active |
| A02 | `f(a, |b)` | second positional parameter active |
| A03 | `f(second=|b, first=a)` | named `second` active despite reordered source |
| A04 | `f(first=a, second=|b)` | named `second` active |
| A05 | `f(first=a, first=|b)` | same parameter active; `DuplicateNamedArgument` at second name |
| A06 | `f(first=a, |b)` where first was named | next unbound positional parameter active |
| A07 | `f(unknown=|x)` with `OpenNamed` | open-named parameter active, no unknown-name diagnostic |
| A08 | same without `OpenNamed` | no active parameter for candidate; `UnknownNamedArgument` |
| A09 | `f(...|items)` with positional rest | rest active |
| A10 | spread without rest | no active parameter; `UnsupportedSpread`; later positional mapping stopped |
| A11 | extra positional without rest | `ExtraArgument`; no active parameter |
| A12 | missing required fixed parameter | candidate not viable; structured missing-required diagnostic after list boundary |
| A13 | optional/defaulted parameter omitted | candidate remains viable |
| A14 | positional targets name-bound parameter | `ParameterAlreadyBound`; mapping advances deterministically |

## 4. Nested, partial, recovery, and boundary tests

| ID | Source/cursor | Exact expected result |
| --- | --- | --- |
| R01 | `outer(inner(|x), y)` | inner call selected |
| R02 | `outer(inner(x), |y)` | outer call selected |
| R03 | same-range wrapper for content call and inner expression call | typed surface rank picks the intentionally shared argument list; no traversal-order dependence |
| R04 | `f(|)` | active parameter 0 |
| R05 | `f( | )` | active parameter 0 |
| R06 | `f(a| , b)` in whitespace before comma | current argument active |
| R07 | `f(a,| b)` on comma boundary/after comma | next argument active |
| R08 | `f(a, |)` trailing comma | one-past-last slot; next unbound/rest parameter active |
| R09 | `f(a |)` no trailing comma | final argument active |
| R10 | cursor at `close.start` | preceding/trailing rule applies; result present |
| R11 | cursor at `close.end` | outside; `NotApplicable` |
| R12 | cursor on opening `(` itself | outside; point immediately after is inside |
| R13 | `f(` missing close | partial help, active slot 0, `MissingCloseDelimiter` recovery and diagnostic |
| R14 | `f(a,` missing close | next slot active, partial help |
| R15 | recovered named value `f(name=)` | named parameter active; `RecoveredArgument` |
| R16 | malformed raw expression with no typed call boundary | semantic-unavailable error, not a source search |
| R17 | comments/strings contain `adapter_fn(` near cursor | no call node, no result |
| R18 | partial/curried nested calls | correct current parameter group and deepest call |
| R19 | 64 nested calls | succeeds when other budgets permit |
| R20 | 65 nested calls | `LimitExceeded(NestedCalls, 65, 64)`, no cache |

## 5. Overload and deterministic presentation tests

| ID | Setup | Exact expected result |
| --- | --- | --- |
| O01 | one viable overload | active signature points to it |
| O02 | two viable, one exact type vs one compatible | exact candidate uniquely active |
| O03 | fixed parameter vs rest for same argument | fixed candidate uniquely active |
| O04 | two incomparable viable candidates | no active signature; `AmbiguousOverload` |
| O05 | no viable candidate | all bounded candidates visible; `NoViableSignature` |
| O06 | insertion order of candidate records reversed | byte-for-byte equal semantic result and LSP result |
| O07 | identical labels, different typed IDs | both retained |
| O08 | exact duplicate typed candidate record | coalesced once |
| O09 | missing documentation | `None`, no fabricated placeholder |
| O10 | project source docs plus adapter docs | selected origin's fixed documentation priority only |
| O11 | non-ASCII authored alias and parameter | LSP parameter tuple offsets are correct UTF-16 code units |
| O12 | incomplete parameter type | label uses `?`, typed unavailable reason retained |

## 6. Native/adapter precedence tests

| ID | Setup | Exact expected result |
| --- | --- | --- |
| P01 | native project only | project candidate |
| P02 | adapter only | normalized adapter environment candidate |
| P03 | same authored name in project and adapter | project wins; adapter not merged |
| P04 | project symbol of same name is non-callable | non-callable result; no adapter fallback |
| P05 | reserved presentation name plus project function `show` | presentation special wins, matching fixed resolver precedence |
| P06 | accepted standard and adapter overloads under distinct typed IDs | ordered overload set, no word resolver |
| P07 | duplicate same `EnvironmentCallableId` during build | accepted-world construction fails atomically |
| P08 | defensively injected corrupt same-rank duplicate | `AmbiguousAuthority`, request error, no cache |
| P09 | same name appears in Rust metadata but cursor is outside an argument list | no result |
| P10 | same name appears under different character owners | exact semantic owner selected; word spelling cannot select the wrong owner |

## 7. Position and stale identity tests

| ID | Setup | Exact expected result |
| --- | --- | --- |
| I01 | valid negotiated UTF-8 position | exact byte offset |
| I02 | valid negotiated UTF-16 position across non-BMP scalar | exact byte offset |
| I03 | UTF-8 character in middle of scalar | `SplitUtf8Scalar` |
| I04 | UTF-16 position splits surrogate pair | `SplitUtf16Scalar` |
| I05 | line or character out of bounds | checked position error, LSP `InvalidParams` |
| I06 | HIR `SourceDocumentIdentity` differs from snapshot | stale document, LSP `ContentModified` |
| I07 | project table module identity differs | stale project source |
| I08 | symbol world differs between table/environment | stale world |
| I09 | symbol revision differs | stale symbol revision |
| I10 | LSP version changes after query starts | stale document version; computed value discarded |
| I11 | accepted generation changes after query starts | stale generation; computed value discarded |
| I12 | profile mapping changes after query starts | stale profile pointer |
| I13 | character revision changes | stale character revision |
| I14 | character digest changes | stale character digest |
| I15 | successful replacement with unchanged digest | generation changes and old cache is not reused |
| I16 | failed rebuild with unchanged accepted world | prior accepted pointer/generation/cache preserved |
| I17 | failed rebuild after document change | new document/HIR cannot pair with old project identity; stale error |
| I18 | document close | document entries evicted and no result from closed URI |
| I19 | workspace removal | owned profile caches cleared |
| I20 | shutdown | admission closed and accepted cache cleared |
| I21 | public query API compile-fail attempt to pass `SourceSnapshotId` | type mismatch; proves selected API does not consume snapshot identity without scanning source |
| I22 | attempted conversion between `SourceSnapshotId` and `SourceDocumentIdentity` | no applicable owned conversion; compile-fail direct type evidence |

## 8. Limit, work, cancellation, and cache tests

| ID | Boundary | Exact expected result |
| --- | --- | --- |
| L01 | source bytes 8,388,608 | succeeds when all other limits fit |
| L02 | source bytes 8,388,609 | source-byte limit error, no cache |
| L03 | 4,096 candidate call nodes visited | succeeds |
| L04 | 4,097 candidate call nodes | candidate-call limit error |
| L05 | 64 overloads | succeeds and deterministic order |
| L06 | 65 overloads | overload limit error; no truncation |
| L07 | 128 parameters | succeeds |
| L08 | 129 parameters | parameter limit error |
| L09 | 512 recovery nodes | succeeds |
| L10 | 513 recovery nodes | recovery limit error |
| L11 | exactly 262,144 work units | succeeds |
| L12 | next charged operation at 262,145 | work limit error before operation |
| L13 | exactly 32 ordinary diagnostics | all 32 retained, omitted zero |
| L14 | 33 diagnostics | first 31 sorted diagnostics plus `DiagnosticsTruncated`; omitted two (the displaced ordinary item and any later item) |
| L15 | checked counter addition overflow | `ArithmeticOverflow` for exact counter; no cache |
| L16 | label UTF-16/u32 conversion overflow using test seam | arithmetic failure; no cache |
| L17 | cancellation set before first probe | `Cancelled`; LSP `RequestCancelled` |
| L18 | cancellation set during bounded resolver loop | same; no partial/cache |
| L19 | deadline elapsed before or during query | `DeadlineExceeded`; LSP `ServerCancelled` |
| L20 | cache reaches 512 then inserts one | deterministic LRU/tie-key eviction; returned semantic result unchanged |
| L21 | cache access-clock overflow | cache clears/resets; successful result returned and reinserted |
| L22 | poisoned cache mutex | cache clears and query continues uncached; semantic result unchanged |
| L23 | stale final stamp after a cache miss | computed result discarded and not inserted |
| L24 | cache hit under exact full key | byte-for-byte equal value; final stamp still checked |
| L25 | same byte offset under different LSP version, generation, character digest, or source identity | cache miss for every changed field |

## 9. No-bypass and architecture tests

| ID | Direct evidence | Expected |
| --- | --- | --- |
| N01 | integration request for adapter callable through LSP | sema candidate ID appears; no adapter-specific result shape |
| N02 | same call checked by type checker and queried for signature | both consume the same resolved target/candidate ID |
| N03 | label text altered while typed candidate ID/type stay fixed | resolution and cache identity unchanged |
| N04 | source contains another same-name call closer as text but not as typed range | exact typed containing call wins |
| N05 | semantic world is unavailable | typed request failure; handler does not call `profile.typecheck_env()` fallback |
| N06 | profile has accepted world plus failed candidate rebuild | only accepted world fields appear in result/cache stamp |
| N07 | unknown callee matching Rust export suffix | no word/suffix fallback |
| N08 | structural audit over Cargo metadata | dependency direction remains syntax -> HIR -> sema -> LSP |
| N09 | public/API visibility tests | syntax constructors remain parser-only; result constructors remain crate-owned |
| N10 | direct parser recovery tests | malformed calls prove typed boundary behavior without source-spelling gates |

## 10. Deterministic test fixture policy

Fixtures construct typed worlds through public/crate-owned registration APIs.
They may use test-only checked constructors already conventional in the owning
crate. They must not create impossible raw integer IDs, deserialize private
identity bytes, parse presentation labels as identity, or enforce absence by
scanning repository source.
