# Implementation records

[Current implementation status](current-status.md) is the single authority for live acceptance, source reconciliation, and remaining work.

Actual validation, failures, and full Git identities are preserved in the
[convergence validation history](convergence-evidence-history.md) and
[other validation history](evidence-history.md). These are historical evidence;
the current status belongs only in the linked ledger.

## Operational policies

- [Implementation evidence instructions](AGENTS.md)
- [Agent workflow](agent-workflow.md)
- [Test execution policy](test-execution-policy.md)
- [Structural audit policy](structural-audit-policy.md)
- [Rust lint suppression tracker](lint-allow-tracker.md)
- [Current image presentation contract](../03-presentation/image-presentation-objects.md)
- [Current text layout contract](../03-presentation/text-typesetting.md)

Maintained specifications and accepted contracts live under docs/00-overview/
through docs/05-build-and-security/ and docs/schemas/. Removed notes remain
recoverable through Git and the evidence ledgers. [Review evidence](../reviews/evidence-history.md)
preserves validation records from removed design packages; dated receipts in
[`acceptance-evidence/`](acceptance-evidence/) also retain historical failures.
