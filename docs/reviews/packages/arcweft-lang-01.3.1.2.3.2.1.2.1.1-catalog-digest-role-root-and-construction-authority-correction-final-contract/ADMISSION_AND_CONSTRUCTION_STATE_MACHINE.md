# Admission and construction state machine

## State A — raw quarantine

Inputs are parsed into raw, Serde-capable declarations. Apply syntactic, cardinality, size, nesting, and work-budget checks. No operational catalog, executable program, generation handle, or constructor exists in this state.

## State B — typed catalog closure

Resolve producer declarations, nominal identities/layouts, exact checked types, CharacterDialogue roles, custom-field metadata, Character/View references, and plan/AWBC bindings into typed candidate catalogs. Reject unresolved, duplicate, ambiguous, foreign-owner, or out-of-scope references.

## State C — per-role canonical derivation

For each closed `RuntimeCatalogDigestRole` role, invoke behavior on the original enum implementation to canonicalize the typed catalog and derive its digest. Compare any role assertion only after derivation. Candidate role catalogs remain private.

## State D — role-root admission

Check exactly one candidate for every required role and no unknown role; order by stable ordinal; derive the root; compare the root assertion. Construct private `RuntimeCatalogDigestRoleRoot` only here.

## State E — pair and generation admission

Derive canonical plan/AWBC binding material. Require plan and AWBC to resolve to the same producer root, role root, catalogs, and binding. Derive `RuntimeGenerationIdentity` and compare all assertions. Build one private `AdmittedRuntimeGeneration` aggregate.

## State F — atomic publication

Publish the aggregate, admitted executable handles, and root as one state transition. No intermediate catalog/root/plan/AWBC handle escapes. Existing active state remains intact on failure.

## State G — scoped authority issuance

A runtime owner asks the aggregate for a closed role and, where applicable, exact producer. Validate role capability eligibility and issue an opaque capability referencing the aggregate and allowed layout closure. A producer gets only the narrower external façade.

## State H — value construction

Resolve a typed admitted layout handle; validate role/producer/generation and all input fields; recursively validate nested nominal generation; construct a candidate; run `validate_against_layout`; return/publish only on success.

## State I — mutation, restore, replay, and hot swap

- Normalize/clear/patch: construct full candidate and atomically replace.
- Restore/replay: deserialize raw data, repeat A–H against the active or replacement generation, then activate.
- Hot swap: build a complete replacement A–H off-path; atomically swap active aggregate. Old capabilities fail as stale.

## Forbidden transitions

- A → executable/runtime value
- raw digest bytes → D/E/G
- producer assertion → admitted catalog/root/capability
- independently admitted plan and AWBC → E
- deserialized admitted handle → F/G
- old-generation capability → H/I
- partially validated patch → published mutation
