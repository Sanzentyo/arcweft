# Closed decision log

| ID | Decision |
|---|---|
| `V-001` | CheckedViewCatalog in FinalSemanticAnalysis is the sole semantic execution catalog. |
| `V-002` | Every accepted definition and reachable View/value ExprId has exactly one catalog row. |
| `V-003` | Ordinary synthetic AWBC functions returning RuntimeValue own all dynamic values; Fx is projection-only. |
| `V-004` | ResourceRefValue uses its existing nominal RuntimeValue layout through inherent contextual conversion. |
| `V-005` | Existing AWVP kind 9/magic/common schema/field 1 are retained; the unreleased transcript is directly replaced. |
| `V-006` | No AWBC tag/ABI change and no session save schema change are required. |
| `V-007` | Every dynamic-capable authored field is native Constant or required projected Program; no silent defaults. |
| `V-008` | Image is ResourceRef<Image> on EmitImage, not a ViewElementKind. |
| `V-009` | PNG/JPEG still and typed GIF/WebP animation are current; APNG is excluded. |
| `V-010` | Automatic proof and #[static] share one typed result; absence of certificate selects dynamic execution. |
| `V-011` | Certificates and fragments are serialized and recomputed/validated from accepted artifact facts; source sema is not rerun. |
| `V-012` | One runtime evaluator provides dynamic/certified parity and keeps all lifecycle work. |
| `V-013` | Defaults evaluate in callee order/environment; explicit arguments evaluate in caller environment. |
| `V-014` | Exports bind ViewId/ViewProgramId/ViewExportContractDigest plus typed node/instruction/site/part coordinates; the enclosing accepted program supplies the revision, never HIR ID, ordinal, or source text. |
| `V-015` | Session save v2 persists semantic mount/binding state only; artifact identity binds certificates indirectly. |
| `V-016` | ViewInstruction::Match is added to the owning enum with inherent behavior. |
| `V-017` | MissingCheckedViewProjection and stale product tests are deleted in protected compiler/product cut C4. |
| `V-018` | All work is hard-bounded and candidate failure is atomic. |
| `V-019` | Syntax/HIR IDs and CheckedViewCatalogGeneration are session-only non-Serde facts; persistent identity reuses ViewId/ViewProgramId/AcceptedViewProgramRevision. |
| `V-020` | Node/instruction/site/parameter/local IDs are dense program-local coordinates scoped by exact accepted revision, not syntax-derived hashes. |
