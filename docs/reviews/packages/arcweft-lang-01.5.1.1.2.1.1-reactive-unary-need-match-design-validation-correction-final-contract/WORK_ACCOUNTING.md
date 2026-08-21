# Exact bounded-work contract

All limits are inclusive. Counters use checked addition. The owner checks before
allocation, newly admitted traversal, AWBC invocation, diagnostic append, or
authoritative mutation. Exact-limit succeeds; one-over returns the typed owner
error with pre-call state byte-identical.
| Limit | Inclusive | Charged owner | Check | One-over |
|---|---|---|---|---|
| max_subscriptions_per_program | 65536 | compiler/bundle subscription rows | before allocation/traversal/invocation/mutation | max_subscriptions_per_program exceeded; authoritative transaction unchanged |
| max_subscriptions_per_definition | 4096 | checked/compiler rows in one View | before allocation/traversal/invocation/mutation | max_subscriptions_per_definition exceeded; authoritative transaction unchanged |
| max_subscriptions_per_mount | 1024 | runtime observer bindings | before allocation/traversal/invocation/mutation | max_subscriptions_per_mount exceeded; authoritative transaction unchanged |
| max_match_arms | 256 | one generic Match | before allocation/traversal/invocation/mutation | max_match_arms exceeded; authoritative transaction unchanged |
| max_pattern_nodes_per_match | 4096 | checked/AWBC pattern graph | before allocation/traversal/invocation/mutation | max_pattern_nodes_per_match exceeded; authoritative transaction unchanged |
| max_bindings_per_arm | 256 | selector output bindings | before allocation/traversal/invocation/mutation | max_bindings_per_arm exceeded; authoritative transaction unchanged |
| max_pattern_ops_per_evaluation | 65536 | AWBC selector budget | before allocation/traversal/invocation/mutation | max_pattern_ops_per_evaluation exceeded; authoritative transaction unchanged |
| max_publications_per_batch | 65536 | runtime publication input | before allocation/traversal/invocation/mutation | max_publications_per_batch exceeded; authoritative transaction unchanged |
| max_live_need_journals | 65536 | generation-bound journal | before allocation/traversal/invocation/mutation | max_live_need_journals exceeded; authoritative transaction unchanged |
| max_observers_per_need | 16384 | one journal fanout | before allocation/traversal/invocation/mutation | max_observers_per_need exceeded; authoritative transaction unchanged |
| max_live_need_observers | 262144 | runtime observer table | before allocation/traversal/invocation/mutation | max_live_need_observers exceeded; authoritative transaction unchanged |
| max_invalidation_queue | 65536 | queued observer invalidations | before allocation/traversal/invocation/mutation | max_invalidation_queue exceeded; authoritative transaction unchanged |
| max_retained_arms_per_observer | 256 | retained local-state branches | before allocation/traversal/invocation/mutation | max_retained_arms_per_observer exceeded; authoritative transaction unchanged |
| max_payload_depth | 128 | ordinary RuntimeValue nesting | before allocation/traversal/invocation/mutation | max_payload_depth exceeded; authoritative transaction unchanged |
| max_payload_nodes | 65536 | ordinary RuntimeValue nodes | before allocation/traversal/invocation/mutation | max_payload_nodes exceeded; authoritative transaction unchanged |
| max_payload_bytes | 16777216 | one Ready payload snapshot | before allocation/traversal/invocation/mutation | max_payload_bytes exceeded; authoritative transaction unchanged |
| max_restore_publications | 65536 | snapshot publication table | before allocation/traversal/invocation/mutation | max_restore_publications exceeded; authoritative transaction unchanged |
| max_restore_observers | 262144 | snapshot observer table | before allocation/traversal/invocation/mutation | max_restore_observers exceeded; authoritative transaction unchanged |
| max_restore_retained_arms | 262144 | snapshot retained arm rows | before allocation/traversal/invocation/mutation | max_restore_retained_arms exceeded; authoritative transaction unchanged |
| max_restore_payload_bytes | 67108864 | total snapshot Ready bytes | before allocation/traversal/invocation/mutation | max_restore_payload_bytes exceeded; authoritative transaction unchanged |
| max_replacement_subscription_mappings | 262144 | old/new semantic joins | before allocation/traversal/invocation/mutation | max_replacement_subscription_mappings exceeded; authoritative transaction unchanged |
| max_diagnostics | 1024 | one phase accumulation | before allocation/traversal/invocation/mutation | max_diagnostics exceeded; authoritative transaction unchanged |
## Complexity

- catalog: O(V+E+P+B);
- subscription canonicalization: O(S log S);
- publication: O(N log N + fanout);
- one selector: bounded pattern nodes/ops/arms/bindings/payload;
- save/restore: linear in canonical rows and payload nodes/bytes;
- replacement: O(S log S + retained arms);
- diagnostics: bounded stable order up to max_diagnostics.

Stale generation/cursor rows count against batch count but not payload projection.
Duplicate equality pays only bounded canonical digest work.
