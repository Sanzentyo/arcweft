# Serde, codec, save, replay, and AWBC boundary

## Serializable forms

Only raw declarations/assertions are serializable: raw plan, raw AWBC, bundle/project data, external producer declarations, save/replay payloads, generation/root/role digest assertions, and domain runtime payloads. Their bytes do not carry operational authority.

## Non-serializable forms

The following are deliberately non-Serde and have private fields/constructors:

- `RuntimeCatalogDigestRoleRoot`;
- `AdmittedRuntimeGeneration`;
- `RuntimeConstructionAuthority`;
- admitted plan/AWBC executable handles;
- admitted nominal/layout/catalog handles;
- external producer value builders.

Do not implement manual Serde that reconstructs these from bytes. Do not use a global registry during deserialization to smuggle authority back in.

## Wire assertions

A raw assertion consists of grammar version, stable role ordinal where applicable, and expected digest bytes. Core derives canonical bytes from the typed candidate and compares exact bytes. Assertions may improve diagnostics/cache lookup but never skip canonicalization or validation.

## Save and replay

Save/replay stores the raw domain state plus expected generation/root identities. Restore resolves the referenced runtime plan/AWBC through the normal pair-admission pipeline (or requires the active aggregate to match), re-admits all nested nominal values with the current catalog, and only then reconstructs domain state. A stale, foreign, missing, or conflicting generation is a typed failure before activation.

## AWBC

Raw AWBC is quarantined. The executable handle is issued from the same admitted aggregate as the plan and role root. VM/fiber/executor APIs accepting raw `AwbcProgram` are migrated and deleted. Codec decode alone never yields executable authority.

## Versions

Schema, ABI, codec, digest grammar, and protocol remain `1`. Unknown versions produce typed errors. No legacy grammar, heuristic reinterpretation, fallback decode, or dual reader is retained.
