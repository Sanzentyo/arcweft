# Closed decisions

| ID | Decision | Binding consequence |
|---|---|---|
| D-001 | Catalog digest inputs are assertions, not authority. | Core always canonicalizes and derives before comparison. |
| D-002 | `RuntimeCatalogDigestRole` is closed and owns all role behavior in its original inherent `impl`. | No extension trait, helper table, text switch, or consumer-local duplicate behavior. |
| D-003 | `RuntimeCatalogDigestRoleRoot` is complete and unique. | Exactly one entry per required role; missing/duplicate/unknown roles fail. |
| D-004 | Digest derivation is acyclic. | role catalog → role digest → role root → generation; never reverse/self-reference. |
| D-005 | `AdmittedRuntimeGeneration` is the sole operational aggregate. | Plan, AWBC, catalogs, layouts, capabilities, and activation all share object identity. |
| D-006 | Construction requires opaque scoped capability. | No raw digest/root/layout/nominal constructor grants authority. |
| D-007 | External producer capability is narrowed by producer, role, and accepted layout closure. | Producer cannot emit undeclared/wrong-role/wrong-generation values. |
| D-008 | Admitted roots and capabilities are non-Serde. | Save/replay persist raw assertions and re-admit. |
| D-009 | CharacterDialogue uses typed role façades. | No descriptorless wrapper, arbitrary checked type, or `Dynamic` escape. |
| D-010 | Normalize/clear/patch/restore are transactional. | Validate reconstructed complete value before one publish. |
| D-011 | Pair admission is atomic. | Plan/AWBC disagreement publishes neither side. |
| D-012 | Existing Arcweft 32-byte digest primitive is retained. | This correction standardizes framing and ownership, not the hash algorithm. |
| D-013 | Schema/ABI/codec/digest/protocol version remains `1`. | Unknown versions fail; no v0/v1 dual reader or compatibility path. |
| D-014 | Migration ends in deletion. | Old constructors/readers are absent, not merely deprecated. |
| D-015 | Errors are typed and deterministic. | Consumers do not parse display strings; paths/roles/expected/actual are fields. |
| D-016 | Canonicalization is bounded and work-accounted. | No unbounded map iteration, recursive hash amplification, or allocation-before-limit. |

## Closed questions

`OPEN_QUESTIONS=0`. No external authority is required to implement the contract at the requested repository baseline.

## Version policy

All Arcweft-owned versions remain `1`. Golden vectors and exact codecs freeze the corrected v1 meaning; no version bump is used to avoid completing the migration.
