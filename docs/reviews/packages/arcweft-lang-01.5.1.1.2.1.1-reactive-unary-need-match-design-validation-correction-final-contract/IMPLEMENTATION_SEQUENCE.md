# Deletion-driven compile-clean implementation sequence

No production source is included here. Each cut must compile before the next;
product construction stays fail-closed until every required consumer exists.

## Cut 0 — freeze

Rebase current main, read current root/nested AGENTS, repeat Await/Need consumer
scan, update evidence if Rust changed, and land compile-fail structural tests first.

## Cut 1 — genuinely missing parent substrate, unreachable

1. Add final-analysis `CheckedViewCatalog` ownership/publication.
2. Add generic checked/product/runtime Match, selector output binding contract,
   MatchBinding/NeedState-capable ordinary AWBC invocation.
3. Add accepted typed coordinates/digests.
4. Keep View product construction fail-closed; do not adapt old Await.

## Cut 2 — checked Need facts

1. Add CheckedViewNeedMatch/Subscription with exact generation, HIR IDs, Need/T,
   producer, arms/patterns/bindings/coverage/effects/source/ownership.
2. Replace static DirectAwait reason with LiveNeedSubscription.
3. Publish complete catalog atomically; lower construction still fails closed.

## Cut 3 — core/runtime authority

1. Add inherent RuntimeNeedState cursor/digest/projection methods.
2. Add strict v1 subscription DTO/table and cross-section validation.
3. Add journal, observer table, invalidations, start intents, save/restore/replay,
   and scratch replacement reconciliation.
4. Add focused/property/tamper tests; no backend selector.

## Cut 4 — atomic consumer switch and deletion

In one compile-clean cut:

1. compiler emits generic Match + subscriptions + producer/selector AWBC;
2. bundle model/codec/validation/merge/digest/source-map switch;
3. runtime catalog/evaluator/fingerprint/replacement/save/replay switch;
4. native/Web/headless/Agent/generated consume shared output;
5. delete every old Await row in DELETION_MATRIX;
6. reject old bytes, adding no compatibility surface.

No interval may compile with both old and new authorities enabled.

## Cut 5 — admission

Run focused, tamper, differential, save/replay/replacement, exact/one-over,
structure/API absence, workspace check/test, Clippy, fmt, docs, generated,
native/Web/headless/Agent, platform, and Tier-2 gates. Only then report
implementation READY.
