# Diagnostics and failure precedence

## 1. Stable diagnostic codes

| Code | Typed variant | Primary evidence |
| --- | --- | --- |
| `project.content.manifest_source_evidence_missing` | `ManifestSourceEvidenceMissing` | typed manifest token path |
| `project.content.unknown_root` | `UnknownContentRoot` | root element span |
| `project.content.ambiguous_root` | `AmbiguousContentRoot` | root element span + ordered candidates |
| `project.content.invisible_root` | `InvisibleContentRoot` | root element + declaration span |
| `project.content.wrong_family` | `WrongContentRootFamily` | root element + resolved declaration |
| `project.content.required_root_absent` | `RequiredContentRootAbsent` | root element + demand span + expected path |
| `project.content.optional_root_referenced_absent` | `ReferencedOptionalContentRootAbsent` | root element + bounded typed references |
| `project.content.overlay_duplicate` | `ProfileTopologyOverlaySetError::Duplicate*` | duplicate overlay inputs |
| `project.content.overlay_kind_conflict` | `ProfileTopologyOverlaySetError::CrossKindConflict` | text/binary overlay pair |
| `project.content.overlay_unconsumed` | `UnconsumedTopologyOverlay` | exact overlay path/kind |
| `project.content.character_package_invalid` | Character package wrapper | manifest/root span + package error |
| `project.content.accepted_identity_mismatch` | `AcceptedProjectIdentityMismatch` | exact package/version/profile/revision pair |
| `project.content.accepted_carrier_invariant` | `AcceptedProfileProjectInvariant` | typed fact/Character/path/document evidence |
| `project.content.stale_candidate` | publication CAS result | request/accepted generation |
| `project.content.work_limit_exceeded` | bounded admission error | phase and charged/exact limit |

The display message is not identity. CLI, Agent, and LSP project the same typed
variant and code.

## 2. Failure precedence within one root

After strict manifest/source evidence succeeds, a root fails in this order:

1. unknown target;
2. project-symbol ambiguity/collision;
3. visibility;
4. wrong family;
5. path/containment mapping for Character;
6. exact manifest I/O classification;
7. required absence;
8. referenced optional absence;
9. manifest decode/identity/package validation for a present root;
10. revision/final carrier invariant.

This order prevents filesystem details from leaking for an unresolved or
invisible symbol, preserves first-fatal ordering across authored roots, and
still prevents “optional” from masking malformed present data because package
validation runs whenever the exact manifest is present.

## 3. Cross-root ordering

Content units sort by `ContentUnitId`; roots use authored ordinal. The first
fatal error is deterministic. Diagnostics accumulated before a fatal bounded
limit are sorted by source document identity, byte range, code, and stable
candidate identity. No hash-map iteration order is observable.

## 4. Related reference bounds

The diagnostic retains the complete typed reference inventory in the internal
error subject to the existing candidate work/diagnostic limit. LSP presentation
emits a deterministic bounded prefix and a count of omitted references. Limit
exhaustion fails the candidate; it does not silently truncate semantic
admission.

## 5. Removed Source spelling

There is no `source_removed`, `source_content_root_removed`, or similar
historical code. Removed syntax receives ordinary current-grammar recovery;
an old entity reference that no longer resolves receives ordinary unknown or
wrong-family behavior through current typed authorities.
