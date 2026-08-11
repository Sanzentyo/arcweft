# Compile-clean implementation and deletion order

No gate may be merged half-migrated. Each gate ends with `cargo fmt`, targeted
tests, workspace check, and Clippy before proceeding.

## A0 — intake and repin

1. Materialize a clean checkout at `2585f527b02808305b3a8cab0442eb522e8d0352`.
2. Verify `git rev-parse HEAD` exactly and record dirty state.
3. Re-read root and every scoped `AGENTS.md` covering touched files.
4. Verify parent ZIP hashes in `PARENT_ARTIFACTS.sha256`.
5. Re-run the foundation focused tests proving accepted IDs/path/slot codecs.
6. Search for moved declarations since the pinned commit only if implementing
   on a descendant; reopen a decision solely for a concrete result-changing
   conflict.

Exit: no production edit yet; exact baseline recorded.

## A1 — nominal layout authority and checked type

One atomic compile-clean cut:

1. change the existing compiler `RuntimeSchemaProjection::nominal` to project
   `RuntimeTypeSchema` once, call `try_layout_hash`, compare the checked
   `NominalSchemaDigest`, and return typed projection errors on failure;
2. route accepted ordinary nominal-record layout projection through those same
   `schema`/`layout_hash` methods without retaining an entry role schema;
3. add `RuntimeNominalRecordLayout`, field type, and layout error to the existing
   core nominal-record owner;
4. add `layout: TypeLayoutHash` to `RuntimeCheckedType::Nominal`;
5. move value-predicate behavior into `RuntimeCheckedType::accepts_value` and
   delete the free helper;
6. add `layout` to `RuntimeResolvedNominal` and `RuntimeTypeShape::Named`;
7. add `RuntimeResolvedNominalRecord` and fact errors;
8. update compiler/sema fact publication to build one checked defining-order
   descriptor and Arc-intern it;
9. replace `nominals`/`pattern_nominals` record fact maps and APIs with
   `nominal_records`/`pattern_nominal_records`;
10. update all checked-type producers, variants/select consumers, AWBC type
   projection, and tests; and
11. delete identity-only record fact APIs in this same cut.

Exit gates:

- core has no HIR/sema/runtime-plan dependency;
- no nominal checked type lacks layout;
- every layout hash is an observed `RuntimeTypeSchema::try_layout_hash` result;
- role `NominalSchemaDigest` parity tests pass and direct digest-to-layout
  projection is deleted;
- no local layout hash fabrication or type-only fallback;
- descriptor publication tests pass;
- workspace check/Clippy green.

## A2 — nominal expression and pattern projection

One atomic compile-clean cut:

1. add `RuntimeNominalRecordExpr`, field-expression carrier, initializer error,
   and `RuntimeExpr::NominalRecord`;
2. change final-HIR nominal record lowering to consume
   `facts.nominal_record(id)` and the checked initializer constructor;
3. retain initializer expressions in authored order with accepted IDs;
4. change `RuntimePattern::Record` owner to shared nominal layout;
5. change final pattern lowering to consume `pattern_nominal_record`;
6. replace positional nominal matching with descriptor name-to-ID mapping;
7. add evaluator/structured engine/AWBC verifier/VM branches;
8. add plan/root validation of deserialized nominal expression carriers; and
9. delete nominal-to-anonymous lowering and positional zip in this cut.

Exit gates:

- every `RuntimeExpr` and `RuntimePattern` match is exhaustive;
- `[z,a]` initializer order evaluates `z` before `a` but stores `[a,z]`;
- nominal identity/layout survives final HIR through execution;
- workspace check/Clippy green.

## A3 — anonymous and record-column admitted carriers

One atomic compile-clean cut:

1. make `RuntimeFieldValue` and `RecordSeqField` fields private and add accepted
   one-based IDs plus inherent accessors;
2. add `RuntimeRecordAdmissionError` and `RuntimeValue::try_record`;
3. extend existing `RuntimeSeqError` with count/identity variants;
4. add `RecordSeq::try_from_accepted_fields`;
5. change public `RuntimeSeq::record_columns` to pair input;
6. migrate literal materialization, record row columnarization, `into_values`,
   `tail_from`, `value_at`, pure helpers, engine, AWBC, and tests to preserve IDs;
7. delete `RecordSeq::new` and all raw field literals; and
8. add trybuild failures for raw carriers and old constructor/signature.

Exit gates:

- IDs are contiguous in authored/stored order;
- record-sequence precedence tests pass;
- no `RecordSeqError` symbol exists;
- no raw field carrier construction compiles;
- workspace check/Clippy green.

## A4 — nominal runtime value admission and unchecked deletion

One atomic compile-clean cut:

1. add `try_from_accepted_layout`, `validate_against_layout`, `field_id`, and
   `field` to the existing value owner;
2. extend `RuntimeNominalRecordError`;
3. implement authored-order evaluation plus ephemeral ID scatter;
4. migrate constants, pure evaluator, structured engine, AWBC VM/verifier,
   root/replay, runtime driver, snapshots, bundles, and save restore;
5. require ingress validation before owner traversal/activation;
6. delete public `RuntimeNominalRecordValue::new` and `validate_shape`; and
7. add trybuild failures for the old constructor.

Exit gates:

- no unchecked nominal value constructor remains;
- identity/layout/count/type precedence tests pass;
- canonical anonymous/nominal bytes remain distinct and stable;
- workspace check/Clippy green.

## A5 — shared visitor convergence

1. update the existing ownership visitor to consume stored/derived field IDs;
2. migrate ownership classifier, duplicate-owner ranking, nesting/node
   accounting, snapshot owner graph, replay/save traversal, and diagnostics;
3. delete every duplicate recursive record walk or make it delegate to the sole
   owner; and
4. prove exact `RecordField`, `RecordColumn`, and `NominalRecordField` paths.

Exit gates:

- no name-derived visitor path;
- classifier/visitor parity over the full runtime value matrix;
- accepted path order/Serde/fixed-LE goldens byte-identical;
- workspace check/Clippy green.

## A6 — persistence, ABI, and full closure

1. validate interim plan Serde descriptor/field identities at every load path;
2. preserve runtime canonical value bytes and existing snapshot projection
   decisions;
3. update bundle/save fixtures without a dual reader;
4. update AWBC exhaustive consumers while keeping ABI 1/codec 8;
5. run all tests in `TEST_MATRIX.csv`;
6. run core all-features tests, workspace all-targets/all-features check,
   Clippy `-D warnings`, workspace tests, Tier 2 where applicable, structure
   audit, `cargo fmt --all`, and `git diff --check`; and
7. confirm no compatibility layer, source gate, production overlay, or stale
   deleted symbol remains.

Only after A6 may G1.2-B through G1.2-F continue.
