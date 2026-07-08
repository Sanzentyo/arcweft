# Memoization Cache schema

Memoization Cache は `memo fn`、`memo task`、View layout cache、typeset cache、shader/JIT artifact cache の共通 metadata schema である。

関連:

- [Object Hooks and Memoization](../01-language/hooks-and-memoization.md)
- [Hook runtime](../02-runtime/hooks-memoization.md)
- [Cranelift JIT](../02-runtime/cranelift-jit.md)

## MemoCacheRecord

```rust
pub struct MemoCacheRecord {
    pub schema_version: u32,
    pub cache_id: String,
    pub scope: String,
    pub entries: Vec<MemoEntryRecord>,
    pub stats: MemoStatsRecord,
}
```

## MemoEntryRecord

```rust
pub struct MemoEntryRecord {
    pub key_hash: String,
    pub function: String,
    pub function_semantic_hash: String,
    pub args_hash: String,
    pub dependency_snapshot: Vec<DependencyVersionRecord>,
    pub value_kind: String,
    pub value_hash: String,
    pub created_tick: u64,
    pub last_used_tick: u64,
    pub size_bytes: u64,
}
```

Example:

```json
{
  "key_hash": "b3:memo-key...",
  "function": "fn.visible_choices",
  "function_semantic_hash": "sem:b3:9f2a...",
  "args_hash": "b3:args...",
  "dependency_snapshot": [
    { "dep": "GameState.route", "version": 12 },
    { "dep": "GameState.affection[character.alice]", "version": 8 }
  ],
  "value_kind": "Vec<ChoiceView>",
  "value_hash": "b3:value...",
  "created_tick": 180,
  "last_used_tick": 182,
  "size_bytes": 384
}
```

## MemoStatsRecord

```rust
pub struct MemoStatsRecord {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub invalidations: u64,
    pub bytes: u64,
}
```

## JIT cache entry

```rust
pub struct JitCacheRecord {
    pub key_hash: String,
    pub function: String,
    pub function_semantic_hash: String,
    pub target_triple: String,
    pub cpu_features: Vec<String>,
    pub cranelift_version: String,
    pub clif_hash: String,
    pub native_artifact_hash: Option<String>,
    pub vm_equivalence: String,
}
```

## Invalidation rules

```text
function semantic hash changed:
  invalidate all entries for function

dependency version changed:
  invalidate entries whose dependency snapshot is stale

hot reload patch committed:
  invalidate affected function/entity/layer/View subtree caches

profile/build mode changed:
  invalidate JIT and debug-sensitive memo entries
```

