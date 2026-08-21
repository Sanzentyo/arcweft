# Failure precedence and atomicity

| Rank | Boundary | Selected first failure | Atomic result |
|---:|---|---|---|
| 1 | HIR/project generation | snapshot/world/revision mismatch | no semantic report |
| 2 | ordinary sema | type/Need shape/pattern/guard/coverage/effect | no checked View fact |
| 3 | ownership/persistence | affine/borrow/must-drop/unsnapshotable | no subscription |
| 4 | checked catalog | missing/duplicate/inconsistent fact/source | no catalog |
| 5 | authored static | first live-subscription contaminant | no required-static product |
| 6 | compiler scratch | missing AWBC/binding/type/source-map join | no CompiledProject |
| 7 | strict decode | envelope/version/tag/unknown/canonical/budget | no validated product |
| 8 | cross-section/catalog | function/task/type/digest/span/ID | no runtime catalog |
| 9 | publication generation | unknown future generation/Need | reject whole batch |
| 10 | publication cursor | same-cursor conflict/corruption | reject whole batch |
| 11 | publication payload | type/ownership/depth/digest/fanout | reject whole batch |
| 12 | generic Match/frame | no arm/bad output/span/render | no frame/mount/start intent |
| 13 | restore | marker/identity/cursor/payload/arm/queue/limit | session unchanged |
| 14 | replacement | candidate/map/reconcile/intent failure | runtime unchanged |

Retired generation is dropped before payload work. Current stale cursor is dropped
before type projection. Exact duplicate only pays bounded digest comparison.
No-op dispositions do not hide hard error in another row of the same batch.

Atomic scopes:

1. semantic report;
2. compiler product;
3. decoded catalog;
4. publication journal plus invalidations;
5. frame/mount/locals/retained arms/start intents;
6. restore;
7. replacement catalog/mappings/mounts/observers/queues.

A committed publication is independent of a later frame failure and is not rolled
back. Post-swap task side effects are explicit committed intents.
