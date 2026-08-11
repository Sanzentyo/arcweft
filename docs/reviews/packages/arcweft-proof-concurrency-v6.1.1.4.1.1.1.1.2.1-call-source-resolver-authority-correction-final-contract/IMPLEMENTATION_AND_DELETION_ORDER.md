# Implementation and deletion order

Each numbered cut compiles before the next. The public switch is one deletion
cut, not a compatibility interval.

1. **Central final-HIR schema**
   - extend original Call-owned enums/impls with recovered callee/name/value/type
     states and canonical issue derivation;
   - extend original `HirLimit` and source-role enums;
   - add private constructors and read-only accessors.
2. **Central attached projection**
   - add `ExpressionProjection::Call`, child roles, and component roles;
   - parser emits current `=`/postfix-spread/type-application components into the
     one pending projection;
   - validate source identity/revision/length/order.
3. **Transactional lowering/source**
   - lower attached Call into final HIR;
   - generate only missing recovery children;
   - stage only `HirSourceIndex` rows;
   - derive singular root poison from canonical structural issues.
4. **Callee classification**
   - preserve same-revision dot value/nominal evidence;
   - value-first/nominal-second; explicit `::` nominal-only;
   - project-aware arity validation; zero-resolver terminal path.
5. **Shared resolver/facts**
   - pass ordered call type arguments once;
   - replace lossy optional-name fields in the existing
     `CheckedCallArgumentFact`;
   - extend existing work reports/counters;
   - keep 256-candidate `CallableLimits`.
6. **Signature/LSP/tooling**
   - derive argument/type active slots exclusively from final source queries;
   - project facts using expected source identity;
   - add Proof two-witness projection over complete facts.
7. **Single public authority switch**
   - migrate all consumers;
   - delete detached final-HIR/tooling reads, old span scanners, static Capacity
     helper/early branch, optional-name interpretation, and old fixtures;
   - fix every compile error directly.
8. **Validation**
   - focused parse->attach->lower->resolve/query tests;
   - workspace check/clippy/test policy;
   - compile-fail visibility/Serde/dependency checks;
   - structural audit; no source gate.

No cut introduces an alias, wrapper, extension trait, compatibility module,
dual reader, source reparse, or old-spelling diagnostic.
