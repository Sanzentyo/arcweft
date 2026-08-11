# Source request disposition and authority

## Originating request

The originating `2026-07-20-lang-01.5.1.1.1-dialogue-profile-owner-and-admission-reconciliation.md`
now begins with:

```text
Status: resolved on 2026-07-21
Do not dispatch this request.
```

It records that the corrected Lang-01.5.1.1 redelivery with SHA-256
`58bcc3a8b03414e7cca2b08cdd3770517a22b7b09b568a1f34fa2dc34956d506`
resolved the four conflicts that motivated Lang-01.5.1.1.1:

1. owner/layer placement;
2. real admission stage;
3. source-map duplication; and
4. exact `inline-failure` wire and policy representation.

## Why this archive exists

The user explicitly requested a concrete design ZIP. The correct response is
not to pretend that the resolved request is still an open implementation
blocker. This archive instead freezes the final decisions, maps them to current
source, supplies exact diagnostics/tests/migration rules, and makes the
verification boundary explicit.

## Authority order used

For conflicts, this package applies the current repository policy:

1. current source at `0c8cb74dd96116a8b987cc419c9a280b6cabe4a4`;
2. maintained stable documentation and implementation records at that commit;
3. the selected corrected package recorded by repository intake;
4. the resolved request as historical requirement evidence;
5. superseded returned-package rows only as rejected alternatives.

## Superseded rows

The following choices are non-authoritative and must not be revived:

- moving `DialogueProfileSpec`, the decoder, or a dialogue-specific source map
  into `arcweft-manifest-model`;
- making project-loader validate against runtime-driver's catalog;
- constructing a second View/Style catalog before compiler product acceptance;
- spelling the field or table `inline_failure`;
- introducing a bridge policy enum, alias, dual spelling, or dual reader;
- placing the reusable revision value in compiler when runtime-plan must import
  it, which would produce a compiler/runtime-plan cycle.

## Version-control status field

The historical request template asked for a Jujutsu change ID. Current root
`AGENTS.md` makes Git the sole version-control authority and explicitly forbids
recording or requiring Jujutsu identities. This archive therefore records the
full Git SHA only. This is a policy correction, not an unresolved decision.
