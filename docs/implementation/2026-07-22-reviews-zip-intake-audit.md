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
| [`arcweft-lang-01.1.1.1-final-contract-c957a61e4a0b.zip`](../reviews/packages/arcweft-lang-01.1.1.1-final-contract-c957a61e4a0b.zip) | `024A13F98A7F46764A79CCBBD8F7ED317C30A4F5E24332E6AE1E2FF7B2A7E18C` | Active implementation | Complete exact prefix/postfix propagation source evidence, typed boundary checking, tooling projection, full contract matrix, Tier 2, and structural audit. |
| [`arcweft-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation-final-contract.zip`](../reviews/packages/arcweft-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation-final-contract.zip) | `FF695EADEF1A4C833D86F53CA5E9010C7DF3D3643418109980B0E9F1D6CFE1AB` | Implemented substrate; final adapter publication cut remains | The shared resolver and entity-family projection are implemented. Complete Lang-01.1.1.2.2 through the same accepted world. |
| [`lang-01.1.1.2.1-entity-family-applied-type-projection-final-contract.zip`](../reviews/packages/lang-01.1.1.2.1-entity-family-applied-type-projection-final-contract.zip) | `FDAFDCA7B5D6682504A901274EB05C7B74C19816063C9A959C69DE0157A01906` | Received, internally verified, and implemented in the active cut | `Ref<EntityFamily>` now uses the same checked contextual projection as `Speaker` and `SpeakerPreset`; focused resolver, callable, entry, persistence, fixture, and LSP tests pass. |
| [`2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip`](../reviews/packages/2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip) | `4518DC6D81A6435B7514CE7BDCD3887DF87A857A8BC9EAA5DF14DF62DBD59C95` | Received; internally verified and implementation-ready | Its 197-row matrix replaces string nominal carriers with accepted Rust-package/environment owner-qualified identities, projects publications through the accepted world, and migrates Rust ADT metadata atomically. Implement it after the active entity-family cut is validated to avoid overlapping resolver edits. |
| [`Lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction-final-contract.zip`](../reviews/packages/Lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction-final-contract.zip) | `66809A1280A507F69BB78D9DF3BF7AF227A91CD68B86CF8771CBF9EE20AA856A` | Superseded by Lang-01.3.1.2.2 | Do not implement its last-group-only curried projection. |
| [`arcweft-lang-01.3.1.2.2-curried-external-stream-final-contract.zip`](../reviews/packages/arcweft-lang-01.3.1.2.2-curried-external-stream-final-contract.zip) | `D1BD7FB5301509CA88BE7C9D3662942CA88472D11143499C0C3067D626DF9418` | Received; internally verified and implementation-ready after Cut 0 re-intake | Its 168-row matrix preserves every curried argument group through signature, product, runtime, ABI 2, codec 8, host, save, and hot-reload boundaries. Re-read current callable/sema/runtime paths after the active Try cut because the package pins `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`. |
| [`arcweft-lang-01.4.2-resource-extension-manifest-wire-contract-final-contract.zip`](../reviews/packages/arcweft-lang-01.4.2-resource-extension-manifest-wire-contract-final-contract.zip) | `01F308C08FE818E247E41E94278EB2D69D5A12AC597794A9109390840C0D95D3` | Superseded by the repository-grounded Lang-01.4.2.1 redelivery | Do not implement the unvalidated package. |
| [`arcweft-lang-01.4.2.1-resource-extension-manifest-repository-reconciliation-final-contract-main-5821a3ca.zip`](../reviews/packages/arcweft-lang-01.4.2.1-resource-extension-manifest-repository-reconciliation-final-contract-main-5821a3ca.zip) | `7DC625446304FE2FFA73027AA518853DF56982BD486347D6F81142D8EAF6ACC0` | Received; internally verified and implementation-ready | After Lang-01.1.1.2 and the existing resource registry/retained-identity substrate, implement the Sans-I/O strict manifest crate, single decoder/encoder, atomic registry publication, AWFB section 22, and runtime digest validation. Public `res` authority switching remains a later cohort. |
| [`arcweft-lang-01.5.1.2-typed-content-root-admission-final-contract.zip`](../reviews/packages/arcweft-lang-01.5.1.2-typed-content-root-admission-final-contract.zip) | `CA72FD70C657A11B7BECDB331D131177B6DEFD6094D034BBECFC3AF1A232E1C0` | Safe binary-topology subset implemented; root-family wording superseded | Retain only the already verified substrate and use Lang-01.5.1.2.1 for the closed family switch. |
| [`arcweft-lang-01.5.1.2.1-content-root-family-source-elimination-reconciliation-final-contract-main-5821a3ca.zip`](../reviews/packages/arcweft-lang-01.5.1.2.1-content-root-family-source-elimination-reconciliation-final-contract-main-5821a3ca.zip) | `C91C2C635C13EB68D46C5D0D4A6F3ECDE0192546BE32175EB2A24FAA54FDE699` | Received; internally verified and implementation-ready after dependency re-intake | Its 160-row matrix closes roots to Character, Flow, View, Action, Activity, Asset, Signal, Metric, Layer, and exact configured resources; `Source`, callable, Stream-return, and name heuristics remain excluded. |
| [`arcweft-aw-ah-009.3.3.3-unchecked-call-family-migration-evidence-reconciliation-final-contract.zip`](../reviews/packages/arcweft-aw-ah-009.3.3.3-unchecked-call-family-migration-evidence-reconciliation-final-contract.zip) | `BAE928C475214AB141DF108B1B2C2A34D7E1AFCF61110145C7B59074D79AA76E` | Mechanically verified; semantically superseded in three areas | Retain its Drop/Promotion taxonomy and Speaker only as a current-phase observation with no final-completion credit. Do not implement its CapacityMethod spread rejection, frozen-carrier Dialogue fixture, 20/3/46 cardinality, or ambiguous physical-once wording. AW-AH-009.3.3.3.1 requests the narrow correction against the accepted .3.3.4 and AW-AH-009.4 chains. |
| [`arcweft-aw-ah-009.3.3.3.1-capacity-dialogue-and-overload-accounting-reconciliation-final-contract.zip`](../reviews/packages/arcweft-aw-ah-009.3.3.3.1-capacity-dialogue-and-overload-accounting-reconciliation-final-contract.zip) | `060332BC62273C34F267F0F15767FE6BBD328BE177CB8035E83F210267AB0D41` | Received and internally verified; accepted with one explicit precedence adjudication | Implement its phase ledger, Speaker/Dialogue gate, and physical-versus-retained accounting. Its contradictory `CAP-005` is package-local drift: AW-AH-009.3.3.4 T08/C17 remain fully authoritative that bare `Vec` is a typed arity failure. No follow-up request is needed. |
| [`arcweft-aw-ah-009.3.3.4-typed-associated-capacity-callee-authority-reconciliation-final-contract.zip`](../reviews/packages/arcweft-aw-ah-009.3.3.4-typed-associated-capacity-callee-authority-reconciliation-final-contract.zip) | `DD8096DEDEF9FE2446291B3849DCEABD8BB5192B88533AA12FEE2DFC3CCEC484` | Received, internally verified, and implementation-ready | Implement the typed parenthesized associated-callee/source-map/nominal-receiver route through the single shared resolver and delete the old string capacity dispatcher in the same compiling switch. |
| [`arcweft-aw-ah-009.4-character-dialogue-first-class-runtime-final-contract.zip`](../reviews/packages/arcweft-aw-ah-009.4-character-dialogue-first-class-runtime-final-contract.zip) | `A86044FEA7AAFF3EC3829DFA0AD6552C88377CA61FA2911C3B96EA34CA0FFA5E` | Accepted final contract; all 19 members internally reverified | Use its runtime-value CharacterDialogue model and deletion boundary as the parent authority. Its implementation follows the required Proof typed-HIR substrate and the .4.2/.4.3 public switch rather than preserving `.say` or Speaker/SpeakerPreset. |
| [`arcweft-aw-ah-009.4.2-dialogue-content-application-syntax-hir-ownership-production-reconciliation-final-contract.zip`](../reviews/packages/arcweft-aw-ah-009.4.2-dialogue-content-application-syntax-hir-ownership-production-reconciliation-final-contract.zip) | `05E825DDE033F308F24FC1F6E504B4C26BBA2D61FD33852CE880DC666BA8F2A8` | Accepted and implementation-ready; all 16 members internally reverified | After the required Proof typed syntax/HIR/project authority exists, publish the typed bracket/colon content application with ordinary CharacterFactory/Reconfigure calls and delete the frozen syntax/HIR readers in the same authority switch. |
| [`arcweft-aw-ah-009.4.3-source-site-line-identity-project-diagnostics-production-reconciliation-final-contract.zip`](../reviews/packages/arcweft-aw-ah-009.4.3-source-site-line-identity-project-diagnostics-production-reconciliation-final-contract.zip) | `FD9F97D37B857991120DD5E5E5DB27953257121FC48C79BEEF4FA03DF1F23396` | Accepted and implementation-ready; all 17 members internally reverified | Land accepted source-site line identity, project collision transaction, diagnostics, and runtime-plan consumption with .4.2 as one public Dialogue authority switch; remove the Speaker family/ID rather than granting both old and final rows matrix credit. |
| [`arcweft-aw-ah-009.4.1.2.1-tts-runtime-intent-envelope-architecture-reconciliation-final-contract.zip`](../reviews/packages/arcweft-aw-ah-009.4.1.2.1-tts-runtime-intent-envelope-architecture-reconciliation-final-contract.zip) | `CCF4DA80B64D4C2246EF652C035A46E088505A4DFC1DE702CFD59BCF45A3BB30` | Received and internally verified; not implementation-ready | Retain its core-safe bridge direction, but do not implement until AW-AH-009.4.1.2.1.1 closes replay/save validation, queued old evidence, sequence-bearing admission atomicity, reachable cap rows, construction authority, and generic AWBC verification. The requested producer sidecars were absent. |

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
3. Re-intake and apply Lang-01.5.1.2.1 after the ordinary-function/generator and
   external-Stream Source-elimination prerequisites, before closed content-root
   admission and remaining source-content deletion.
4. Apply the repository-validated Lang-01.4.2.1 after Lang-01.1.1.2 and the
   resource registry/retained-identity substrate, before adding the public
   extension-manifest decoder, canonical encoder, or AWFB publication.
5. Keep AW-AH-009.4.1.2.1 out of production until both the lower TTS package's
   Lang-01.4/Lang-01.5.1 entry gates and the AW-AH-009.4.1.2.1.1 correction are
   closed. The received runtime-envelope ZIP is evidence for the selected
   direction, not permission to infer the missing public APIs.

## Current design dispatch

No design request should be dispatched now. The returned `CAP-005` conflict is
resolved by its own package precedence in favor of AW-AH-009.3.3.4 T08/C17;
it does not justify another design round. The original requests and returns
remain immutable audit inputs and must not be silently replaced or resent.

AW-AH-009.3.3.4 has returned and is accepted implementation-ready, so it must
not be resent. Its static associated-capacity authority switch is production
work now, and its bare-`Vec` arity decision remains authoritative. The
package-local `CAP-005` row is closed by that precedence adjudication rather
than tracked as a pending correction.
The TTS
[`AW-AH-009.4.1.2.1.1`](../reviews/requests/2026-07-24-aw-ah-009.4.1.2.1.1-tts-runtime-envelope-transaction-and-validation-closure.md)
request exists and remains unreturned, but it is deliberately held under the
current TTS skip decision rather than dispatched now.

Lang-01.5.1.3 has returned and is classified
`ACCEPTED_IMPLEMENTATION_READY_DEFERRED` in its
[package intake](2026-07-24-lang-01-5-1-3-generated-artifact-runtime-binding-intake.md).
It must not be sent again. The returned adapter publication, curried Stream,
content-root, and resource-manifest packages are likewise production work and
must not be resent. Lang-01.5.1.1.1 was resolved by its corrected parent
redelivery and also must not be dispatched again.

## Ongoing intake rule

At task start and each reviewable push cut point, compare every recursive
`docs/reviews/**/*.zip` archive against package-specific intake/completion notes
and recorded SHA-256 values. A ZIP directly under `docs/reviews/` is an inbox
item: inspect it before selecting the next production slice, then move it to
`docs/reviews/packages/`. Classify every archive as implementation-ready,
active, blocked by a named request, superseded/duplicate, or invalid as
delivered. Do not silently leave a returned archive outside the task dependency
graph.
