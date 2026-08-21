# Accepted parent precedence

Retained without redesign:

1. `FinalSemanticAnalysis -> CheckedViewCatalog` is the sole checked View authority.
2. Generic `CheckedViewExecution::Match` / `ViewInstruction::Match`.
3. Ordinary AWBC synthetic functions and `RuntimeValue`; no View VM.
4. Typed resources, cross-section bindings, canonical digests, strict decode,
   transactional publication, static proof, bounded work, save/replay/replacement.
5. Program-local coordinates are meaningful only under the exact accepted program revision.

Only stale direct-Await rows are superseded.
