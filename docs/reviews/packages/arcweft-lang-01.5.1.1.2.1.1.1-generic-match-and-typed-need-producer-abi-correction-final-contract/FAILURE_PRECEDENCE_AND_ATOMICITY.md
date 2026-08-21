# Failure precedence and atomicity

## Compile/product precedence

Failures are reported in this deterministic order; later layers do not run after an earlier failure:

1. resource registry integrity;
2. HIR/symbol generation lease;
3. generic CheckedMatch completeness and child fact validity;
4. ownership classification and static View admission;
5. runtime type projection/interner limits;
6. AWBC selector and Need producer construction;
7. ordinary AWBC structural/code/type verification;
8. View reactive bundle cross-section validation;
9. cross-section/resource/content-root digest equality;
10. final product publication.

An error never publishes an empty checked View catalog, partial AWBC function, or partial section.

## Runtime selector precedence

1. active generation/content root;
2. section/site/result digest;
3. nominal owner and case;
4. tuple presence/count;
5. nested value types;
6. output/local/disposition rows;
7. duplicate local and store revision;
8. transaction commit.

All checks through 7 are read-only. Commit swaps staged values atomically. On any error, selected body execution and observer publication do not begin.

## Runtime Need precedence

1. active generation and verified product;
2. raw value is dedicated NeedHandle;
3. unique producer contract binding;
4. producer/result/payload/task-plan integrity;
5. payload type digest;
6. argument count/type/ownership/limits;
7. canonical NeedId recomputation;
8. resource registry equality;
9. journal lookup/create;
10. observer publication and optional start intent transaction.

A String fails at step 2. A wrong generation fails before journal lookup. A contract or argument error cannot create a NotStarted row. Duplicate start intents are deduplicated only after the fully verified journal key and `JoinSameKey` task key are known.

## Restore/replacement precedence

Manifest/container and save DTO integrity precede object decoding; recursive structural/limit validation precedes active-product binding; all producer and selector values are verified before the replacement transaction commits. Failure leaves the old generation/session active and unmodified.
