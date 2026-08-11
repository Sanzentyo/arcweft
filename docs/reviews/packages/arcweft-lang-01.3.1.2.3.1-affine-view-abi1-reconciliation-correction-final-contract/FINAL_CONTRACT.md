# Final correction contract

## C1. Precedence

This package is mandatory after both corrected parents and before production implementation crosses either parent's public switch. It supersedes only the rows named in `SUPERSESSION_MATRIX.md`. Every other parent decision and every retained parent test remains required.

Direct user authority fixes `AWBC_ABI_VERSION` at 1. Parent statements requiring ABI 2 are premise corrections, not retained requirements.

## C2. Sole generic ownership authority

`RuntimeValue` remains the sole executable value graph. `RuntimeValueOwnership::{Unrestricted, Affine}`, the opaque `RuntimeAffineOwnerToken`, checked unrestricted duplication, explicit move, exact capture plans, consuming sequence behavior, closed cloneable `RuntimePayload`, and Stream owner/table decisions remain as selected by Lang-01.3.1.2.3.

Live `RuntimeValue`, `RuntimeBinding`, `RuntimeEnv`, affine aggregates, closures, iterators, VM frames, and fibers do not implement unconditional `Clone` or generic Serde after the final cut.

## C3. AWBC ABI 1 ownership completion

ABI 1 is directly replaced with the ownership-complete register model:

- Stream opcodes remain `0x27`, `0x28`, `0x29`;
- `CopyValue { dst, src } = 0x2a`;
- the existing `Move` consumes its source;
- the existing `Drop = 0x1f` performs prepared table-aware language drop;
- every operand has inherent `Borrow | Copy | Consume | Destination` metadata;
- verifier state is `Uninitialized | Live | Moved | Dropped` plus cleanup and transaction facts;
- all traps are mutation-atomic.

`0x2b..=0x7f` remain unknown. There is no ABI-2 symbol or compatibility reader in final production.

## C4. Activation-domain uniqueness

A dormant snapshot may be copied. Runnable authority is acquired only through the unique `RuntimeExecutionActivationAuthority` of one `RuntimeExecutionDomain`.

Within one domain, at most one `RuntimeDriver` owns an active lease for an `ExecutionInstanceId`. Empty restore fails while any driver in the same domain is active for that execution. Replacement names and consumes the target driver's exact current lease, verifies that the candidate execution is not active in another driver, and transfers activation atomically after old owner retirement. The candidate may preserve the same execution ID or switch to a different inactive execution ID. Candidate preparation creates no runnable token or lease.

Separate execution domains/processes are separate runtime universes. Core does not claim distributed exclusivity; adapters may coordinate domains externally without changing runtime owner IDs or creating a second core authority.

## C5. Allocator continuation

`RuntimeExecutionSnapshotV2` contains the exact affine owner allocator cursor. It is the first never-issued ordinal or `Exhausted`; it is never reconstructed from live values. Snapshot validation requires execution equality and that every recorded owner ordinal is strictly below the cursor unless exhausted. Restore installs the exact cursor before execution publication. The first post-restore mint uses that cursor and advances it once.

## C6. Drop typestate

Drop preparation occurs through `RuntimeOwnershipTransaction`. It reserves the exact source slot, removes and owns the exact `RuntimeValue`, records source revision/owner set/domain operations, and leaves no independent value parameter at commit. Abandoning the transaction restores the source reservation without releasing language owners. Successful commit releases nested affine owners once and terminalizes the same slot.

## C7. Snapshot Rust equality

`RuntimeValueSnapshotV2` derives `Clone`, `Debug`, `PartialEq`, `Serialize`, and `Deserialize`; it does not implement `Eq`. Canonical codec bytes and semantic digests, not Rust `Eq`, own strict save validation. Evidence-only integer/newtype rows may retain `Eq`.

## C8. View ownership admission

The current Lang-01.5.1.1.2 View language admits only statically `Unrestricted` values at retained/render boundaries:

- parameters and defaults;
- mount state and locals retained across frames;
- repeat source/item/key;
- nested View arguments;
- environment bindings used by render/direct-await;
- computed text/resource/property values;
- exported runtime values.

A type whose closed layout may contain an affine leaf is rejected before compiler publication. This includes open/opaque/generic values without exact unrestricted evidence. Defaults are corrected from “any accepted ordinary value” to “any accepted ordinary unrestricted value.”

Pure render and direct-await captured inputs are checked copies. Direct-await result/state retained by the View must also be unrestricted. Handler input is moved exactly once from the event/request owner into the handler frame; handler captures from retained View state are checked copies. Generic affine values may exist transiently inside a handler according to the generic runtime rules, but cannot be committed into retained View state, parameter/default storage, repeat inventory, export storage, or render cache in this cut.

This restriction does not decide the later source-level `mount`/`ViewHandle` contract. A future scoped presentation handle belongs to its lifecycle owner and cleanup stack, not to generic retained View render state.

## C9. Ownership-aware View product

Every `ViewValueInputBinding` serializes exact transfer intent. Current accepted combinations are:

| Program role/source | Transfer | Static ownership requirement |
|---|---|---|
| Pure: parameter/local/repeat/environment | Copy | Unrestricted |
| DirectAwait: parameter/local/repeat/environment | Copy | Unrestricted |
| DirectAwait ready/error/denied retained payload | Move into retained slot, then reusable only if Unrestricted | Unrestricted |
| Handler: handler input | Move | exact input type; payload owner consumed once |
| Handler: parameter/local/environment capture | Copy | Unrestricted |

No View cross-section binding uses a borrow: a borrow may exist only within one AWBC instruction and never crosses invocation, frame commit, suspension, save, or safe point.

Product validation joins function role, input source, transfer mode, exact type/layout ownership, and AWBC ABI 1. A mismatch rejects before runtime publication.

## C10. View save

Session save remains schema 2. Live `RuntimeBinding` Serde is removed. View parameter/state/local/repeat/handler-frame values use the sole `RuntimeValueSnapshotV2` traversal owned by the whole-execution snapshot. View-specific rows carry typed coordinates and refer to those dormant values; they do not define a parallel value DTO.

Unrestricted View values are restored by ordinary snapshot materialization. Any tampered View retained slot containing an affine owner is rejected before activation. Active handler frames follow generic snapshot eligibility and activation rules.

## C11. Wire-enforced static requirement

The ViewProgram transcript adds sorted, unique `static_requirements` before `static_fragments`. Each row identifies an exact definition/subtree subject, carries an attribute source reference for diagnostics, and has a deterministic requirement digest excluding source spelling/ranges.

The complete requirement-digest set is part of `ViewProgramSemanticDigest`. Each requirement must have exactly one certificate for the same subject with `proof_origin = AuthoredRequired`. `AuthoredRequired` without a requirement, a requirement without a certificate, duplicate requirements, subject mismatch, stale program identity, or origin downgrade is a hard invalid product. Unannotated subjects may have an `Automatic` certificate or no certificate; absence selects dynamic execution only when no requirement exists.

## C12. Static fragment dispatch

Subjects carry exact half-open instruction spans and ancestry under one accepted program revision. Sibling spans are disjoint. Ancestor/descendant containment is allowed; partial overlap is invalid.

At runtime the evaluator selects the first valid fragment at the outermost entered subject. While that fragment executes, descendant fragment dispatch is suppressed. If an ancestor is dynamic, certified descendants may be selected when their subject boundary is reached. Static and dynamic paths publish identical retained lifecycle, observation, source, state, input, handler, resource, and save effects.

## C13. Atomicity

Snapshot preparation, activation reservation, owner allocator validation, View product validation, static requirement/certificate joins, fragment selection, View frame evaluation, restore, and hot replacement stage all fallible work before publication. Failure preserves the active execution, activation lease, allocator cursor, View mount state, Stream table, save lineage, and observation revision.

## C14. Deletion

The final cut deletes:

- every ABI-2 constant/name/test expectation;
- per-driver restore activation entrypoints that bypass the domain authority;
- independent-value `RuntimePreparedDrop::commit(value, ...)`;
- `Eq` expectations for `RuntimeValueSnapshotV2`;
- View input bindings without transfer intent;
- retained View admission of may-be-affine values;
- live `RuntimeBinding` View save serialization;
- certificate-origin validation without serialized requirement evidence;
- ambiguous fragment overlap/dispatch fallback.

No alias, V2 ABI wrapper, old reader, source gate, or compatibility shim remains.
