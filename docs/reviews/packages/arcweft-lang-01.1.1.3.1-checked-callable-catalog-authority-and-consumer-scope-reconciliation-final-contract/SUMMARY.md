# Summary

## Decision

Keep the accepted callable catalog. Do not create or publish the parent package's copied checked-record catalog shape.

The one authoritative relationship is:

```text
Arc<RegisteredCallableCatalog>
  └─ exact Arc<CallableRecord> for each accepted project/environment/standard callable
       └─ retained by CheckedCallableFacts in one immutable Arc<CheckedCallableCatalog>
            ├─ shared unchanged by TypeCheckReport
            ├─ shared unchanged by ProjectSemanticIndex
            ├─ validated and consumed by compiler and LSP accepted snapshots
            └─ projected, never copied as authority, into Agent/runtime/persistent outputs
```

For registered checking, every `CheckedCallableFacts::record` is `Arc::ptr_eq` to the record stored by the accepted `RegisteredCallableCatalog`. For detached checking there is no accepted project catalog: the private checked builder creates each detached `Arc<CallableRecord>` once and moves that same Arc directly into the final checked catalog; no detached public registry or alternate reader exists.

## Corrected checked record

```rust
pub struct CheckedCallableFacts {
    id: CheckedCallableId,
    record: Arc<CallableRecord>,
    execution: CheckedCallableExecution,
    effects: CheckedCallableEffects,
    interface_digest: CallableInterfaceDigest,
}

pub enum CheckedCallableEffects {
    Body {
        contract: CallableEffectContract,
        inferred: EffectRow,
    },
    BodylessTraitRequirement {
        contract: CallableEffectContract,
    },
    RecordFixed,
}
```

`RecordFixed` delegates to `CallableRecord::schema().effects()`; it stores no row. The parent package's `ExternalOrStandard { exposed: EffectRow }` is deleted. Project/detached source callables use ID-only effect schemas; their declared contract and inferred row exist only in checked facts. `exposed_row()` and `actual_row()` are inherent methods on `CheckedCallableFacts`; consumers do not reproduce the branch.

## Accepted record extension

`CallableRecord` remains the owner of:

- candidate/declaration identity;
- lookup key and authority rank;
- provider and publication provenance;
- `Arc<CallableSignatureSchema>`;
- documentation;
- exact source spans;
- declaration access policy;
- Rust provenance; and
- fixed environment/standard rows.

`CallableRecord::try_new` becomes crate-private. `CallableDeclarationOwner`, `CallableDeclarationKey`, `CallableCandidateId`, and their inherent implementations are extended in their original owner modules for trait requirement, trait implementation, inherent method, detached, environment, and standard behavior. No helper trait or ad hoc family match is authorized.

## Identity closure

The parent structural identities remain, with one required addition: accepted environment callables use their existing `EnvironmentCallableId` as structural identity. Checked identity becomes exact over project world/revision, accepted catalog digest, detached source identity, environment catalog digest, and standard catalog version. A stale or foreign ID never falls back to a name, raw HIR, source text, or reconstructed signature.

Live project relations use `CheckedCallableId`; durable Agent and persistent identities use a canonical digest of `CallableDeclarationKey` or `EnvironmentCallableId`. Transient `CheckedCallableId` is never serialized into interface summaries.

## Consumer closure

- `TypeCheckReport` owns one `Arc<CheckedCallableCatalog>` and removes separate public callable-execution/effect-row authorities.
- `ProjectSemanticIndex` retains the same Arc, keys project symbols by `CallableDeclarationKey`, stores the validated `CheckedCallableId`, and removes signature/source/effect copies and spelling lookup.
- `ProjectGraphSymbolRef::Callable` contains `CheckedCallableId`, not `QualifiedName`.
- LSP resolves source location or call-target facts to typed identity, validates the accepted generation, and queries the exact record.
- Agent callable IDs derive from structural digests; names are display-only.
- Persistent interface summaries serialize structural callable identity plus record-derived signature/interface digests; they do not rebuild signatures from HIR or serialize checked-context handles.
- Runtime trait lowering retains the parent contract's one-way checked digest projection and typed conformance index; runtime never resolves source or effect rows.

## Transaction and publication

Registration freezes the accepted catalog before checking. A private checked builder then creates pending shells, runs the existing effect fixed point and trait conformance, validates all rows, and freezes one immutable catalog. Only after freeze are `TypeCheckReport`, `ProjectSemanticIndex`, compiler products, LSP accepted environment, Agent payloads, and persistent summaries publishable. Rollback removes every shell, edge, conformance, closure row, source index entry, and effect-variable mutation after the checkpoint. Any stale generation or failed validation publishes nothing.

## Migration rule

Delete or make uncallable the old copies and fallbacks first, then repair compile fallout toward the final owner. In particular delete:

- `TraitCallableId` and string/local-index effect identities;
- copied trait requirement/implementation signatures and rows where the accepted record is authoritative;
- `CallableEffectSchema::Project.declared`;
- resolver-created empty rows and copied requirement-as-implementation rows;
- `ProjectCallableSymbol.signature`, `.source`, copied effects, and `project_callable(QualifiedName)`;
- raw-HIR interface signature reconstruction and `decl:{index}:...` identity;
- Agent `owner:name` callable identity and name fallback;
- local-index/string runtime trait identity and `(usize, String)` witness lookup; and
- legacy generic `AWF-EFX-001` / `UpperBoundExceeded` effect reporting.

No compatibility alias, shim, dual reader, source gate, removed-syntax diagnostic, CSS path, or Takumi path is introduced.
