# GraphPatch Schema Sketch

```json
{
  "base": {
    "program_hash": "b3:...",
    "git_commit": "70e24164373e7898ff9ef83f56f4c48523ce108e",
    "graph_revision": "gr:..."
  },
  "ops": [
    {
      "op": "RenameEntity",
      "entity": "ent_01J8...",
      "old_public_id": "say.opening.001",
      "new_public_id": "say.opening.greeting",
      "keep_alias": true,
      "update_references": true
    },
    {
      "op": "InsertAfter",
      "after": "ent_01J8...",
      "node": {
        "kind": "Say",
        "id_policy": "auto",
        "payload": {
          "speaker": "alice",
          "text": "でも、夢の中では君もそこにいた。"
        }
      }
    }
  ]
}
```
