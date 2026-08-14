# Decision 08 — checked context and nominal domain issuance order

`RuntimeCheckedValueContext` has no public constructor. It is issued only after
a real site has resolved through an admitted wrapper:

- `AdmittedRuntimePlan::checked_value_context(plan_site, limits)`;
- `AdmittedRuntimeProduct::checked_value_context(origin, limits)`;
- `AdmittedRuntimeProduct::nominal_record_domain(origin, domain_id)`.

The context borrows the admitted generation parent and one resolved type/domain
row. Project and producer domains remain distinct enum variants. No nominal
`checked_values()` context is issued from a semantic world, raw type
declaration, projection builder, raw plan, or raw AWBC object.

Compile order is consequently non-circular: generation issuer -> raw lowerer ->
plan admission -> AWBC/pair admission -> site context/domain issuance -> value
construction/VM/restore. `CONTEXT_ISSUANCE_MAP.csv` gives every constructor and
consumer. No placeholder context, optional parent, or later rebind exists.
