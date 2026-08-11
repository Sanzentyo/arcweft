# PRODUCTION RECONCILIATION

## Inspected baseline

- Repository: `Sanzentyo/arcweft`
- Default branch: `main`
- Inspected revision: `a8403dcb26d78e6cafee3576d5933e9952d8305b`
- Callable substrate commit: `f420ee8fbf244351e11fd5f793b07e7cdd3f1b6a`
- The inspected revision is one commit ahead of the substrate commit, and that intervening commit changes shadow module/import grammar and documentation only. No callable/sema file changed between the two revisions.

## Concrete defects found

### Defect 1 — schema-less error claims schema membership

`CurriedCallableId::try_new` receives only a candidate ID and group index. It rejects group zero using `CallableIdentityError::MissingGroup`, but it cannot determine whether any nonzero group exists in a project or environment schema.

The correction is not to add world state. It is to rename the context-free zero rule precisely and move nonzero membership classification to the schema-owning boundary.

### Defect 2 — two successful curried representations

The current `instantiation_matches` accepts both:

1. `CallableCandidateId::Curried(id)` plus matching `CallableInstantiation::Curried`; and
2. the unwrapped base ID plus `CallableInstantiation::Curried` when the group exists.

The second arm creates two successful representations for the same logical curried target. It is a concrete defect under the one-product/one-resolver contract and SHALL be removed.

### Defect 3 — absent group loses its typed resolver error

The current resolved constructor folds failed `instantiation_matches`, including an absent curried schema group, into `ResolveCallError::InvalidResolvedCallable`. The repository already defines `ResolveCallError::InvalidCallGroup` and maps it to `CallableDiagnosticCode::InvalidCallGroup`; that existing typed error is the correct public result.

## Substrate preserved

No evidence justifies redesigning:

- candidate identity hierarchy;
- schema storage or contiguous-group validation;
- `Arc<CallableSignatureSchema>` ownership;
- origin/authority matching;
- catalog records or ordering;
- equivalent-source representation;
- accepted request/world leasing;
- character, presentation, dialogue, data-last, or family schemas;
- current old checker before the migration deletion gate.

The correction edits the owning Arcweft enums and constructors directly. It does not add extension traits, free-standing conversion helpers, or wrapper APIs.

## One-success-boundary flow

```text
accepted request/world validation
        |
        v
shared resolver obtains base candidate + full schema + requested next group
        |
        v
CurriedCallableId::try_new(base, group)
  - wrapper/zero checks only
        |
        v
canonical Curried candidate + matching Curried instantiation
        |
        v
ResolvedCallable::try_new(... full schema ...)
  - canonical pair check
  - schema.group(group) check
  - existing origin/authority/limit/equivalent checks
        |
        +--> InvalidCallGroup / InvalidResolvedCallable
        |
        v
exactly one ResolveCallOutcome::Resolved product
```

There is no signature-only resolver, compatibility fallback, old-resolver retry, or base-ID curried success.

## Provider-neutral behavior

Project, standard, and adapter candidates use different typed IDs/owners but the same group rule. The resolver does not branch on provider to decide schema existence; it supplies the provider's already resolved full schema to the same `ResolvedCallable::try_new` boundary.

## Corrupt-world containment

The final boundary is defensive even if a typed nonzero curried ID was built before schema lookup or an internal catalog/request association is inconsistent. An absent group produces `InvalidCallGroup` and terminates resolution. No provider retry or old route can turn the invalid candidate into success.

A crate-private typed corrupt-world fixture is permitted for tests. A public unchecked constructor, serialized corruption path, source scan, or hidden global state is not.

## Old resolver coexistence and deletion

The repository note confirms that Cut 1 is substrate only and that the existing checker is still the sole successful production resolver. Therefore this correction does not prematurely delete old checks.

During migration:

- do not add a second duplicate membership check in a new helper;
- route new curried construction through `ResolvedCallable::try_new`;
- keep the legacy check only while the legacy route is still the production owner;
- delete the legacy check together with that old route once typed integration evidence proves the shared route is exclusive.

This is temporary implementation coexistence, not a compatibility path. There is never more than one successful route for a migrated family.

## Explicit non-involvement

The correction does not touch or introduce CSS, Takumi, rendering, parser spelling migration, source gates, dual readers, serialization versions, or catalog compatibility machinery.
