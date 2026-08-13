# Producer, consumer, and deletion migration plan

The machine-readable inventory is `PRODUCER_CONSUMER_DELETION_INVENTORY.csv`. The following cutover rules are binding.

## Owner-first migration

1. Extend the original `RuntimeCatalogDigestRole` inherent implementation with stable ordinal/domain/cardinality/canonicalization/capability behavior. Do not create an extension trait or ad-hoc helper.
2. Implement private typed candidate catalogs, derived role digests, and `RuntimeCatalogDigestRoleRoot` admission under the existing admission owner.
3. Correlate plan and AWBC into one `AdmittedRuntimeGeneration` aggregate.
4. Add the private/scoped `RuntimeConstructionAuthority` and update the original `RuntimeNominalRecordAdmissionDomain` implementation to consume it.
5. Add dedicated external producer and CharacterDialogue typed façades.

## Consumer migration

Migrate every VM/fiber/executor/product-step/session/player/bundle/project/root/replay/restore/hot-swap consumer to admitted aggregate handles. Migrate normalize/clear/patch to transactional candidate reconstruction. Migrate all test fixtures to build through the same public/raw admission boundary rather than private unchecked constructors, except focused unit fixtures inside the invariant owner.

## Deletion closure

After exact callers migrate, delete—not merely deprecate:

- public or reachable nominal/layout constructors from raw IDs/digests;
- producer-provided operational catalog/root/generation authority;
- independent plan and AWBC admission/execution handles;
- `From`, `Default`, Serde, or raw-parts constructors for admitted wrappers/capabilities;
- descriptorless CharacterDialogue nominal wrappers and arbitrary checked-type custom entry paths;
- duplicate role ordinal/domain/canonicalization side tables or extension traits;
- compatibility adapters, source gates, fallback readers, dual readers, and old-version branches.

## Inventory completion rule

Before deletion, regenerate the inventory from the exact implementation commit and require every production definition/reference and test fixture matching the relevant symbols to have one disposition: retained owner, migrated consumer, deleted bypass, or explicit negative compile fixture. An unexplained match blocks acceptance.
