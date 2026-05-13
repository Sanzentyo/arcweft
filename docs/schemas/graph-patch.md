# GraphPatch Schema Sketch

```json
{
  "base": {
    "program_hash": "b3:...",
    "jj_change_id": "qtnqlkkm",
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

