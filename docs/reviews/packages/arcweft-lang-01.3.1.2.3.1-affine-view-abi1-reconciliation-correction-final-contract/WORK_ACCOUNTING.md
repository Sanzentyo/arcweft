# Work accounting and limits

Parent limits remain authoritative. This correction adds these bounded units:

| Work | Limit/charging rule |
|---|---|
| activation table lookup | one ordered-map lookup per fresh/restore/replacement attempt |
| active entries per domain | existing runtime execution/session limit; checked before insertion |
| allocator cursor validation | one charge per recorded owner occurrence/table/tombstone/cleanup row; owner set already traversed once |
| prepared drop source reservation | one slot lookup and one value graph traversal; owner leaves charged once |
| View input ownership validation | one charge per `ViewValueInputBinding`; reuses exact type/layout memo |
| static requirement validation | one charge per requirement row; included in existing certificate count limit |
| fragment overlap validation | sort by `(start asc, end desc, subject id)` then one stack pass, O(n log n) sort + O(n) validation |
| runtime fragment dispatch | one subject-entry check; ancestor-active flag makes descendant suppression O(1) |

No new unbounded graph, registry, source scan, or endpoint cache is introduced. Exact-limit and one-over tests are required for active entries, owner rows, View inputs, requirement rows, fragment nesting depth, and fragment count.
