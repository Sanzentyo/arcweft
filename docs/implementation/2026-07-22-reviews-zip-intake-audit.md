# `docs/reviews` ZIP intake audit — 2026-07-22

## Why this audit exists

Several returned contract ZIPs had been placed directly under `docs/reviews/`
without a package-specific intake record. Work resumed from the active dirty
tree and the most recently named package without first reconciling that
directory, so those packages were not surfaced promptly. This was a workflow
failure, not evidence that the files were absent.

This audit is the repository-visible intake ledger. The outer archive hashes
and every archive's internal manifest/hash list were verified. No package was
treated as implementation-ready merely because its filename said
`final-contract`.

## Inventory

| Package | SHA-256 | Intake state | Next action |
| --- | --- | --- | --- |
| [`arcweft-lang-01.1.1.1-final-contract-c957a61e4a0b.zip`](../reviews/packages/zips/arcweft-lang-01.1.1.1-final-contract-c957a61e4a0b.zip) | `024A13F98A7F46764A79CCBBD8F7ED317C30A4F5E24332E6AE1E2FF7B2A7E18C` | Active implementation | Complete exact prefix/postfix propagation source evidence, typed boundary checking, tooling projection, full contract matrix, Tier 2, and structural audit. |
| [`arcweft-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation-final-contract.zip) | `FF695EADEF1A4C833D86F53CA5E9010C7DF3D3643418109980B0E9F1D6CFE1AB` | Implemented substrate; final adapter publication cut remains | The shared resolver and entity-family projection are implemented. Complete Lang-01.1.1.2.2 through the same accepted world. |
| [`lang-01.1.1.2.1-entity-family-applied-type-projection-final-contract.zip`](../reviews/packages/zips/lang-01.1.1.2.1-entity-family-applied-type-projection-final-contract.zip) | `FDAFDCA7B5D6682504A901274EB05C7B74C19816063C9A959C69DE0157A01906` | Received, internally verified, and implemented in the active cut | `Ref<EntityFamily>` now uses the same checked contextual projection as `Speaker` and `SpeakerPreset`; focused resolver, callable, entry, persistence, fixture, and LSP tests pass. |
| [`2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip`](../reviews/packages/zips/2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip) | `4518DC6D81A6435B7514CE7BDCD3887DF87A857A8BC9EAA5DF14DF62DBD59C95` | Received; internally verified and implementation-ready | Its 197-row matrix replaces string nominal carriers with accepted Rust-package/environment owner-qualified identities, projects publications through the accepted world, and migrates Rust ADT metadata atomically. Implement it after the active entity-family cut is validated to avoid overlapping resolver edits. |
| [`Lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction-final-contract.zip`](../reviews/packages/zips/Lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction-final-contract.zip) | `66809A1280A507F69BB78D9DF3BF7AF227A91CD68B86CF8771CBF9EE20AA856A` | Superseded by Lang-01.3.1.2.2 | Do not implement its last-group-only curried projection. |
| [`arcweft-lang-01.3.1.2.2-curried-external-stream-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.3.1.2.2-curried-external-stream-final-contract.zip) | `D1BD7FB5301509CA88BE7C9D3662942CA88472D11143499C0C3067D626DF9418` | Received; internally verified and implementation-ready after Cut 0 re-intake | Its 168-row matrix preserves every curried argument group through signature, product, runtime, ABI 2, codec 8, host, save, and hot-reload boundaries. Re-read current callable/sema/runtime paths after the active Try cut because the package pins `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`. |
| [`arcweft-lang-01.3.1.2.2.1-curried-stream-wire-allocation-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.3.1.2.2.1-curried-stream-wire-allocation-reconciliation-final-contract.zip) | `8ADED7B1CB5D92F9D820C2CC82121AC6D070F3CF26D1618DC23FF144081090AD` | Received, fully verified, and implementation-ready at the Lang-01.3 dependency position | Use `0x27 OpenStream`, `0x28 FinishStream`, and `0x29 ApplyExternalStreamGroup`; replace the flat callable/function/argument/wire owners in place and delete old codec/Source readers at the prescribed atomic switches. No correction request remains. |
| [`arcweft-lang-01.1.1.3-effect-trait-contract-and-dynamic-dispatch-production-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.1.1.3-effect-trait-contract-and-dynamic-dispatch-production-reconciliation-final-contract.zip) | `4FD834564C458639CD4EBE46615E4EC79C54F91D686439AAAACCC7F2B3714B5E` | Semantics accepted; both catalog corrections returned | Retain its E017 supersession, typed trait-effect semantics/diagnostics, and deletion inventory. Implement its exact-record/checked-consumer boundary through the returned `.3.1` and `.3.1.1` contracts rather than the superseded copied signature/source facts. |
| [`arcweft-lang-01.1.1.3.1-checked-callable-catalog-authority-and-consumer-scope-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.1.1.3.1-checked-callable-catalog-authority-and-consumer-scope-reconciliation-final-contract.zip) | `0E4746ABD1589F0228ADC62A074DFC07EEC92F3DF4DBC432E58138FD21500F4C` | Catalog authority accepted; trait-validator correction returned | Retain its exact accepted-record Arc, checked-catalog, consumer, transaction, and deletion decisions. Its final pre-check validator payload and `CallableFamily::TraitMethod` projection are supplied by the verified `.3.1.1` return. The archive has 12 total members/11 valid non-self manifest rows; its “12 non-self” prose is a nonblocking count typo. |
| `arcweft-lang-01.1.1.3.1.1-trait-validator-and-resolver-family-identity-reconciliation-final-contract.zip` (verified direct attachment) | `58330347E6759B38770D512BCAA682A1B3949EF46AFF24462F45C23ED851BC63` | Received, internally verified, and implementation-ready at the Lang-01.1.1 dependency position | Delete the old trait/local-index identity first, install role-only `CallableValidator::Method`, join exact accepted records to checked IDs/conformance, and retain `TraitMethod` only through record/resolved family projection. The existing `.3.1.1` request is fulfilled; no follow-up remains. |
| [`arcweft-lang-01.4.2-resource-extension-manifest-wire-contract-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.4.2-resource-extension-manifest-wire-contract-final-contract.zip) | `01F308C08FE818E247E41E94278EB2D69D5A12AC597794A9109390840C0D95D3` | Superseded by the repository-grounded Lang-01.4.2.1 redelivery | Do not implement the unvalidated package. |
| [`arcweft-lang-01.4.2.1-resource-extension-manifest-repository-reconciliation-final-contract-main-5821a3ca.zip`](../reviews/packages/zips/arcweft-lang-01.4.2.1-resource-extension-manifest-repository-reconciliation-final-contract-main-5821a3ca.zip) | `7DC625446304FE2FFA73027AA518853DF56982BD486347D6F81142D8EAF6ACC0` | Received; internally verified and implementation-ready | After Lang-01.1.1.2 and the existing resource registry/retained-identity substrate, implement the Sans-I/O strict manifest crate, single decoder/encoder, atomic registry publication, AWFB section 22, and runtime digest validation. Public `res` authority switching remains a later cohort. |
| [`arcweft-lang-01.5.1.1.1-dialogue-profile-owner-and-admission-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.1-dialogue-profile-owner-and-admission-reconciliation-final-contract.zip) | `8B7FE4D8DA08B793AB039E612CCE5A27AF3EC34E39B9FA07533C81C1F901350F` | Accepted as-built confirmation; internally verified | Confirms the existing launch/dialogue/compiler admission authority and resolved request disposition. Do not create another production owner or redispatch the request. |
| [`arcweft-lang-01.5.1.2-typed-content-root-admission-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.2-typed-content-root-admission-final-contract.zip) | `CA72FD70C657A11B7BECDB331D131177B6DEFD6094D034BBECFC3AF1A232E1C0` | Safe binary-topology subset implemented; root-family wording superseded | Retain only the already verified substrate and use Lang-01.5.1.2.1 for the closed family switch. |
| [`arcweft-lang-01.5.1.2.1-content-root-family-source-elimination-reconciliation-final-contract-main-5821a3ca.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.2.1-content-root-family-source-elimination-reconciliation-final-contract-main-5821a3ca.zip) | `C91C2C635C13EB68D46C5D0D4A6F3ECDE0192546BE32175EB2A24FAA54FDE699` | Superseded by the 2026-08-08 corrected return | Retain as audit input only. Do not implement its wider Flow/View/Action/Asset/Signal/Metric/Layer root-family set. |
| [`arcweft-lang-01.5.1.2.1-typed-content-root-admission-and-source-elimination-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.2.1-typed-content-root-admission-and-source-elimination-final-contract.zip) | `B1C170170F0EB782FA85E96D933B328F09F4008FAE5EB7BDE20381235DDD7FCB` | Accepted, internally verified, and implementation-ready after prerequisites | Final roots are exactly Character, Resource, and Activity; Source and source `content` are deleted directly. Use one `AcceptedProjectContent`/`ProjectSemanticIndex`/`AcceptedProfileProject` authority and its 122-row matrix. |
| [`arcweft-lang-01.5.1.3-generated-artifact-runtime-binding-fail-closed-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.3-generated-artifact-runtime-binding-fail-closed-final-contract.zip) | `342D38E521C14F2CCE340355F4F4BC07241C8BFA89DA9B7C324B169869482027` | Accepted, internally verified, and implementation-ready after prerequisites | Supersedes the earlier `arcweft-artifact-binding` owner selection. Use `arcweft-runtime-binding`, typed product-local IDs, exact fixed-slot registration, and fail-closed pre-host gates; artifact execution remains out of scope. |
| [`arcweft-aw-ah-009.3.3.3-unchecked-call-family-migration-evidence-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-aw-ah-009.3.3.3-unchecked-call-family-migration-evidence-reconciliation-final-contract.zip) | `BAE928C475214AB141DF108B1B2C2A34D7E1AFCF61110145C7B59074D79AA76E` | Mechanically verified; semantically superseded in three areas | Retain its Drop/Promotion taxonomy and Speaker only as a current-phase observation with no final-completion credit. Do not implement its CapacityMethod spread rejection, frozen-carrier Dialogue fixture, 20/3/46 cardinality, or ambiguous physical-once wording. AW-AH-009.3.3.3.1 requests the narrow correction against the accepted .3.3.4 and AW-AH-009.4 chains. |
| [`arcweft-aw-ah-009.3.3.3.1-capacity-dialogue-and-overload-accounting-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-aw-ah-009.3.3.3.1-capacity-dialogue-and-overload-accounting-reconciliation-final-contract.zip) | `060332BC62273C34F267F0F15767FE6BBD328BE177CB8035E83F210267AB0D41` | Received and internally verified; accepted with one explicit precedence adjudication | Implement its phase ledger, Speaker/Dialogue gate, and physical-versus-retained accounting. Its contradictory `CAP-005` is package-local drift: AW-AH-009.3.3.4 T08/C17 remain fully authoritative that bare `Vec` is a typed arity failure. No follow-up request is needed. |
| [`arcweft-aw-ah-009.3.3.4-typed-associated-capacity-callee-authority-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-aw-ah-009.3.3.4-typed-associated-capacity-callee-authority-reconciliation-final-contract.zip) | `DD8096DEDEF9FE2446291B3849DCEABD8BB5192B88533AA12FEE2DFC3CCEC484` | Received, internally verified, and implementation-ready | Implement the typed parenthesized associated-callee/source-map/nominal-receiver route through the single shared resolver and delete the old string capacity dispatcher in the same compiling switch. |
| [`arcweft-aw-ah-009.4-character-dialogue-first-class-runtime-final-contract.zip`](../reviews/packages/zips/arcweft-aw-ah-009.4-character-dialogue-first-class-runtime-final-contract.zip) | `A86044FEA7AAFF3EC3829DFA0AD6552C88377CA61FA2911C3B96EA34CA0FFA5E` | Accepted final contract; all 19 members internally reverified | Use its runtime-value CharacterDialogue model and deletion boundary as the parent authority. Its implementation follows the required Proof typed-HIR substrate and the .4.2/.4.3 public switch rather than preserving `.say` or Speaker/SpeakerPreset. |
| [`arcweft-aw-ah-009.4.2-dialogue-content-application-syntax-hir-ownership-production-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-aw-ah-009.4.2-dialogue-content-application-syntax-hir-ownership-production-reconciliation-final-contract.zip) | `05E825DDE033F308F24FC1F6E504B4C26BBA2D61FD33852CE880DC666BA8F2A8` | Accepted and implementation-ready; all 16 members internally reverified | After the required Proof typed syntax/HIR/project authority exists, publish the typed bracket/colon content application with ordinary CharacterFactory/Reconfigure calls and delete the frozen syntax/HIR readers in the same authority switch. |
| [`arcweft-aw-ah-009.4.3-source-site-line-identity-project-diagnostics-production-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-aw-ah-009.4.3-source-site-line-identity-project-diagnostics-production-reconciliation-final-contract.zip) | `FD9F97D37B857991120DD5E5E5DB27953257121FC48C79BEEF4FA03DF1F23396` | Accepted and implementation-ready; all 17 members internally reverified | Land accepted source-site line identity, project collision transaction, diagnostics, and runtime-plan consumption with .4.2 as one public Dialogue authority switch; remove the Speaker family/ID rather than granting both old and final rows matrix credit. |
| [`arcweft-aw-ah-009.4.1.2.1-tts-runtime-intent-envelope-architecture-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-aw-ah-009.4.1.2.1-tts-runtime-intent-envelope-architecture-reconciliation-final-contract.zip) | `CCF4DA80B64D4C2246EF652C035A46E088505A4DFC1DE702CFD59BCF45A3BB30` | Received and internally verified; not implementation-ready | Retain its core-safe bridge direction, but do not implement until AW-AH-009.4.1.2.1.1 closes replay/save validation, queued old evidence, sequence-bearing admission atomicity, reachable cap rows, construction authority, and generic AWBC verification. The requested producer sidecars were absent. |

## Dependency order

1. Validate the implemented Lang-01.1.1.2.1 entity-family projection, then
   apply the returned Lang-01.1.1.2.2 package. The latter projects adapter/Rust
   callable publications through the same accepted world instead of equating
   `Named` and accepted nominal identities. It must not introduce a second
   resolver or alter Lang-01.1.1.2.1 work, poison, or source accounting.
2. Re-intake and apply the returned Lang-01.3.1.2.2 correction before Stream
   runtime/AWBC/host/save work. Its pinned baseline predates the active
   propagation and shared-callable changes, so the Cut 0 owner comparison is
   mandatory.
3. Apply the corrected Lang-01.5.1.2.1 after the ordinary-function/generator
   and external-Stream Source-elimination prerequisites, before closed
   Character/Resource/Activity content-root admission and remaining
   source-content deletion. The older wider-family package is superseded.
4. Apply the repository-validated Lang-01.4.2.1 after Lang-01.1.1.2 and the
   resource registry/retained-identity substrate, before adding the public
   extension-manifest decoder, canonical encoder, or AWFB publication.
5. Keep AW-AH-009.4.1.2.1 out of production until both the lower TTS package's
   Lang-01.4/Lang-01.5.1 entry gates and the AW-AH-009.4.1.2.1.1 correction are
   closed. The received runtime-envelope ZIP is evidence for the selected
   direction, not permission to infer the missing public APIs.

## Current design dispatch

The Lang-01.1.1.3.1.1 request has returned, is verified, and must not be sent
again. It closes the typed pre-check validator payload after
`TraitCallableId` deletion and the trait-method observational family projection
without reopening the exact accepted-record Arc model, E017S, effect-row
semantics, consumer scope, or ordinary-call resolution. No follow-up request
arises from this return.

The returned `CAP-005` conflict remains resolved by its own package precedence
in favor of AW-AH-009.3.3.4 T08/C17 and does not justify another design round.
The original requests and returns remain immutable audit inputs and must not be
silently replaced or resent.

AW-AH-009.3.3.4 has returned and is accepted implementation-ready, so it must
not be resent. Its static associated-capacity authority switch is production
work now, and its bare-`Vec` arity decision remains authoritative. The
package-local `CAP-005` row is closed by that precedence adjudication rather
than tracked as a pending correction.
The TTS
[`AW-AH-009.4.1.2.1.1`](../reviews/requests/2026-07-24-aw-ah-009.4.1.2.1.1-tts-runtime-envelope-transaction-and-validation-closure.md)
request exists and remains unreturned, but it is deliberately held under the
current TTS skip decision rather than dispatched now.

Lang-01.5.1.3 has returned again with a corrected final owner and is classified
`ACCEPTED_IMPLEMENTATION_READY_DEFERRED` in the
[2026-08-08 correction intake](2026-08-08-lang-01-5-1-correction-returns-intake.md).
It must not be sent again. The returned adapter publication, curried Stream,
content-root, and resource-manifest packages are likewise production work and
must not be resent. Lang-01.5.1.1.1 is now also retained as an as-built
confirmation and must not be dispatched again.

## Ongoing intake rule

At task start and each reviewable push cut point, compare every recursive
`docs/reviews/**/*.zip` archive against package-specific intake/completion notes
and recorded SHA-256 values. A ZIP directly under `docs/reviews/` is an inbox
item: inspect it before selecting the next production slice, then move it to
`docs/reviews/packages/zips/` unchanged and extract its searchable contents to
`docs/reviews/packages/<zip-basename>/`. Classify every archive as
implementation-ready, active, blocked by a named request,
superseded/duplicate, or invalid as delivered. Do not silently leave a returned
archive outside the task dependency graph.
