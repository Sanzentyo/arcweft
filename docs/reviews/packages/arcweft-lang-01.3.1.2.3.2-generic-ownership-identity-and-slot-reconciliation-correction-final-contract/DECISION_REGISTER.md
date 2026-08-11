# Decision register

All result-changing decisions are closed. The selected value is normative.

| ID | Decision | Selected result |
|---|---|---|
| D-001 | Execution ID owner | existing arcweft_core::runtime_id |
| D-002 | Execution ID representation | private NonZeroU64 |
| D-003 | Execution ID mint | shared RuntimeExecutionDomain only |
| D-004 | Execution ID allocation | domain-monotonic, first 1, never reused |
| D-005 | Execution exhaustion | u64::MAX succeeds then Exhausted |
| D-006 | Execution identity across restart | preserved |
| D-007 | Execution identity across replay | preserved; replay cannot mint a replacement |
| D-008 | Empty restore collision | reject when domain cursor has passed preserved ID |
| D-009 | Activation scope | one runtime-host-shared domain |
| D-010 | Exclusivity proof | linear domain reservation, not &mut driver |
| D-011 | Active cardinality | at most one |
| D-012 | Replacement identity | same execution ID and exact expected epoch |
| D-013 | Activation failure owner | fresh candidate returned |
| D-014 | Replacement failure owner | both active and fresh owners returned |
| D-015 | Record field identity | one-based accepted ordinal |
| D-016 | Anonymous record order | accepted authored order |
| D-017 | Nominal record order | accepted schema/layout order |
| D-018 | Record duplicate names | reject before ID publication |
| D-019 | Nominal ID storage | derived from existing value-vector position; no side table |
| D-020 | Local static identity | RuntimeLocalDeclarationId in runtime plan |
| D-021 | Capture static identity | RuntimeCaptureSlotId in capture-plan order |
| D-022 | HIR mapping | transient sema/runtime-plan maps, then discarded |
| D-023 | Dynamic local identity | execution-wide RuntimeLocalSlotId |
| D-024 | Shadowing | always fresh dynamic slot |
| D-025 | Slot reuse | never; spare capacity may be reused |
| D-026 | Initial revision | 1 |
| D-027 | Revision mutation | exactly +1 per committed affected-slot mutation |
| D-028 | Revision overflow | prepare-time error before reservation/take |
| D-029 | Slot states | Vacant/Live/Moved/Dropped |
| D-030 | Owner union | exact eight variants |
| D-031 | Owner union purpose | diagnostic evidence, not storage |
| D-032 | Owner ordering | manual tag 0..7 then fields |
| D-033 | Owner rendering | inherent lowercase canonical diagnostic string |
| D-034 | Transaction identity | execution + monotonic nonzero ordinal |
| D-035 | Failed transaction ordinal | not reused |
| D-036 | Transaction endpoints | whole storage slots |
| D-037 | Value path purpose | nested owner/error evidence, not storage key |
| D-038 | Prepared Copy | owns complete checked unrestricted duplicate |
| D-039 | Prepared Move | source remains exact live reserved slot |
| D-040 | Prepared Drop | source remains exact live reserved slot; no arbitrary value |
| D-041 | Reservation storage | integrated in actual slot cell |
| D-042 | Repeated participant | only compatible repeated CopySource; all conflicts reject before store lookup |
| D-043 | Duplicate affine owner | global transaction scan, deterministic occurrence |
| D-044 | Prepare failure owner | original transaction returned |
| D-045 | Commit mismatch owner | non-recommittable aborted transaction returned |
| D-046 | Infallibility point | successful RuntimeCommitPermit construction |
| D-047 | After permit | no semantic failure/allocation/check/callback |
| D-048 | Prepare limits | 4096/4096/1048576/64/262144/67108864 |
| D-049 | Prepare precedence | 10 fixed ranks |
| D-050 | Commit precedence | 8 fixed ranks |
| D-051 | Path segments | 10 exact segment variants/tags |
| D-052 | Path ordering | manual lexicographic, prefix first |
| D-053 | Iterator remainder | unconsumed suffix only, absolute original index |
| D-054 | Dense sequence path | root only |
| D-055 | Traversal authority | one internal visitor shared by classifier/owner/snapshot |
| D-056 | Persisted cursors | domain execution + occurrence/local/transaction/affine |
| D-057 | Cursor restore | Next strictly above represented max; persisted Exhausted intrinsically retains MAX high-water |
| D-058 | Affine cursor | persisted, never guessed/recomputed |
| D-059 | Live save carrier | closed snapshot, not RuntimeBinding/RuntimeValue Serde |
| D-060 | Float snapshot | bit wrappers; PartialEq not Eq |
| D-061 | Restore validation | 12 fixed stages before reservation/activation |
| D-062 | Digest | one domain-separated identity section in existing owner |
| D-063 | Save with transaction | blocked with typed count |
| D-064 | ABI/wire allocation | none in this correction |
| D-065 | Classifier | preserve b76465c result |
| D-066 | First production execution construction | G1.2-E |
| D-067 | First serialization of identities | G1.2-D |
| D-068 | Affine token/Stream handle | not constructible in G1.2 |
| D-069 | Behavior placement | existing owner inherent impl; no extension trait |
| D-070 | Compatibility | no aliases, dual readers/writers, shims, source gates |
| D-071 | Core dependencies | no HIR/sema/runtime-plan/driver dependency |
| D-072 | G1.3/G1.4 | not started until G1.2-F accepted |

Decision count: **72**.

No row is `TBD`, conditional on implementation taste, or delegated to a compatibility path.
