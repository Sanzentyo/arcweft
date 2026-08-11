# Presence and family rules

## 1. Closed family table

| Root family | Typed target | Ownership | File-backed | Allowed in `content-units.<id>.roots[]` | Presence rule |
| --- | --- | --- | --- | --- | --- |
| Character | `CharacterId` | Character nominal/package owners | yes | yes | exact `.awchar` manifest and all named layers, or optional absence |
| Resource | `ResourceDeclarationIdentity` | typed `res` registry | no | yes | semantic declaration must exist and be visible |
| Activity | `ActivityId` | abstract Activity symbol/manifest binding owners | no | yes | semantic identity and selected binding facts must be valid |

The profile-content table selects policy for a content unit. It does not accept
an independent root-family value.

## 2. Explicitly invalid families

All other entity families, callable/function values, packaged Asset identities,
Stream values, Stream-producing functions, and removed Source identities are
wrong-family outcomes. A name that resolves to one of those targets receives
`project.content.wrong_family`; an unresolved removed spelling receives the
ordinary `project.content.unknown_root` or parser/type error appropriate to the
current grammar.

## 3. Required/optional state table

| Demand | Character manifest state | Typed references | Result |
| --- | --- | --- | --- |
| Required | present and fully valid | any | `Present` |
| Required | exact path `NotFound` | any | `RequiredContentRootAbsent` |
| Required | present but invalid/unreadable | any | underlying fail-closed error |
| Optional | present and fully valid | any | `Present` |
| Optional | exact path `NotFound` | none | `AbsentOptional(record)` |
| Optional | exact path `NotFound` | one or more | `ReferencedOptionalContentRootAbsent` |
| Optional | present but invalid/unreadable | any | underlying fail-closed error |

## 4. Exact absence boundary

Absence means only that opening the exact contained Character manifest path
returns the platform's `NotFound` category. The following are not absence:

- invalid UTF-8;
- malformed manifest;
- manifest Character ID mismatch;
- permission denied or transient I/O;
- path escape/symlink containment failure;
- missing or unreadable named layer;
- corrupt or truncated PNG;
- wrong dimensions;
- duplicate or unreferenced explicitly supplied layer payload;
- overlay kind mismatch.

## 5. Absence fact

`ProjectTopologyAbsenceRecord` is retained unchanged as the canonical revision
record. The same record is embedded in
`AcceptedContentRootPresence::AbsentOptional`, so semantic inspection, watch,
and revision identity refer to one exact fact rather than reconstructed fields.

## 6. Reference inventory

Reference collection is typed and selected-profile-bounded. It includes all
admitted modules and selected generated metadata, not only dynamically
reachable code. That conservative rule prevents dead-code or future profile
entry changes from silently converting a referenced missing package into an
accepted absence.

References are deterministically ordered. The absence diagnostic uses the root
entry span as primary, the demand span and expected path as related evidence,
and each retained reference range as a bounded related location.
