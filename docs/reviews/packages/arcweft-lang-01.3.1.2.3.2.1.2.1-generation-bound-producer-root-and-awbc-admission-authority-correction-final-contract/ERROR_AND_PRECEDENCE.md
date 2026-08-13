# Typed errors, path/source evidence, and deterministic precedence

## 1. Ownership rule

Each error is added to the enum that already owns the failing boundary.

- core nominal/catalog/root/generation errors remain in core;
- plan errors extend the existing `RuntimePlanError`;
- AWBC admission is owned by `awbc::admission`;
- sema/runtime-plan projection retains typed semantic source evidence;
- dialogue errors retain role/custom/value paths;
- runtime-driver, bundle, save, replay, View, player, and codegen errors wrap
  typed sources.

No parallel helper enum shadows an existing owner and no typed source is
flattened to a free-form string when crossing a typed boundary.

## 2. Plan/product admission precedence

This order is normative:

1. syntax/Serde/codec/header checks and fixed version `1`;
2. existing structural bytecode/plan verification;
3. canonical project/producer root and role/custom declaration shape;
4. canonical custom digest recomputation;
5. catalog key/descriptor scalar and structural consistency;
6. independent project root traversal;
7. independent producer root traversal;
8. exact derived-versus-claimed authorization equality;
9. missing catalog key, extra claimed key, conflicting descriptor, and
   unreachable catalog row in their detailed owner order;
10. plan/AWBC typed-root inventory correlation;
11. generation identity and canonical-body correlation;
12. atomic issuance of one admitted execution authority.

No later error is observable when an earlier stage fails.

## 3. Declaration-local order

Within a generation declaration:

1. top-level counts;
2. project-root order/duplicates;
3. producer order/duplicates;
4. producer payload kind;
5. required CharacterDialogue producer ID;
6. role presence/order;
7. derived Style equality;
8. custom field order/duplicates/count;
9. each field's View order/duplicates/count;
10. closed checked-type depth/work;
11. claimed authorization order/duplicates.

## 4. Catalog order

Within the nominal catalog:

1. layout count;
2. catalog-key order/duplicate;
3. key versus descriptor scalar mismatch;
4. defining field count;
5. one-based field-ID derivation;
6. defining field name/order;
7. first field checked-type structural failure;
8. conflicting same-key descriptor.

## 5. Producer authorization order

Producers are processed by producer ID. Roots are processed by canonical root
coordinate. After deriving a set, compare in key order:

1. duplicate claimed key;
2. first derived-only key -> missing authorization;
3. first claimed-only key -> extra authorization;
4. first claimed key missing from catalog;
5. continue to global reachability.

Claimed rows do not affect traversal or reachability.

## 6. Producer lookup and CharacterDialogue schema construction

The required public order is:

1. admitted generation identity;
2. exact producer identity;
3. canonical role/custom declaration identity and custom digest;
4. Character/View catalog digest correlation;
5. nominal/semantic/layout lookup and producer authorization;
6. nested field validation;
7. schema publication.

For a nominal lookup itself:

1. producer admitted;
2. nominal exists;
3. semantic identity matches;
4. layout matches;
5. producer set contains exact key.

For an existing nominal value after lookup:

1. nominal identity;
2. layout;
3. field count;
4. defensive field-ID derivation;
5. first field predicate in defining order.

## 7. Runtime checked value order

At one value path:

1. nesting/work budget;
2. outer RuntimeValue shape;
3. checked-type-specific owner;
4. Variant ordinal;
5. Variant name;
6. payload presence;
7. recursive payload;
8. Choice unique-branch result;
9. nominal descriptor lookup and validation;
10. domain validation.

`RuntimeValuePath` and `RuntimeCheckedTypePath` are retained separately.

## 8. CharacterDialogue decode order

1. schema generation;
2. opaque producer;
3. tuple18 shape/count;
4. character field;
5. exact opaque semantic identity derived from character;
6. fixed fields in tuple index order;
7. nested role/custom nominal validation;
8. Character/View/custom domain rules;
9. canonical re-encode equality when required;
10. publication.

Voice index 5 follows the nested precedence in its dedicated grammar.

## 9. Patch order and atomicity

1. schema/value generation;
2. operation count/limits;
3. path syntax;
4. duplicate/prefix-overlap paths;
5. resolve every path against original value and checked type;
6. replacement/clear eligibility for every operation;
7. validate every replacement value;
8. clone candidate;
9. apply operations in canonical path order;
10. rebuild nominal nodes through admitted shapes on unwind;
11. whole-value checked validation;
12. CharacterDialogue domain/canonical validation;
13. publish.

Any failure returns the original value byte-identical.

## 10. Restore/replay/View order

1. persisted codec/version;
2. artifact admission;
3. saved versus target generation;
4. target catalog/schema correlation;
5. value validation;
6. ownership/fiber/root reconstruction;
7. replay transition or View mount;
8. session publication.

## 11. Typed mapping examples

### RuntimePlan

```text
RuntimePlanError::GenerationContract {
  source: RuntimeGenerationContractError::Producer {
    source: RuntimeProducerContractError::ExtraAuthorization { ... }
  }
}
```

### AWBC

```text
AwbcAdmissionError::GenerationContract {
  source: RuntimeGenerationContractError::Custom {
    source: CharacterDialogueRuntimeCustomFieldError::DigestMismatch { ... }
  }
}
```

### Dialogue

```text
CharacterDialogueValueError::RoleValue {
  role,
  path,
  source: RuntimeNominalRecordTreeError
}
```

### Driver restore

```text
SessionRestoreError::RuntimeValue {
  slot,
  path,
  source: RuntimeNominalRecordTreeError
}
```

### View

```text
ViewRuntimeError::Generation {
  source: RuntimeGenerationMismatch
}
```

## 12. Projection errors

`CharacterDialogueRuntimeTypeError` and
`RuntimeSemanticFactsError` retain:

- typed role enum;
- accepted semantic `TypeId`;
- accepted world stamp;
- source range;
- original type-projection source.

Missing role, duplicate role, unresolved role coordinate, leaked Named type,
world mismatch, and derived Style mismatch are distinct variants.

## 13. No string flattening

Display strings are produced only by `Error::fmt`. Stored sources remain typed
through bundle, save, driver, player, agent, and CLI layers. JSON-RPC/CLI
adapters may serialize a stable boundary code plus formatted message, but the
Rust error retains its source chain and path.
