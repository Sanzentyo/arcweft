# Layer Manifest schema

Layer manifest は bundle 内の標準 layer、入力方針、capture 方針を記録する。

関連: [Layer System](../03-presentation/layers.md)

## YAML/TOML equivalent

```toml
[layer."layer.world.background"]
kind = "World"
z = -1000
input = "observe_only"
hit_test = "none"
capture = ["color", "object_id"]

[layer."layer.world.characters"]
kind = "Character"
z = 0
input = "pass_through"
hit_test = "bbox"
capture = ["color", "object_id", "mask"]

[layer."layer.ui.game"]
kind = "NativeUi"
z = 1000
input = "block_below_on_hit"
hit_test = "ui_tree"
focus = "ui_tree_order"
capture = ["color", "object_id", "mask", "actions"]

[layer."layer.ui.modal"]
kind = "Modal"
z = 3000
input = "modal"
hit_test = "ui_tree"
focus = "trap"
capture = ["color", "object_id", "mask", "actions"]

[layer."layer.debug.agent"]
kind = "Debug"
z = 9000
input = "observe_only"
hit_test = "none"
capture = ["overlay"]
```

## JSON schema sketch

```json
{
  "layers": [
    {
      "id": "layer.ui.game",
      "kind": "NativeUi",
      "z": 1000,
      "input": {
        "enabled": true,
        "priority": 1000,
        "route": "BlockBelowOnHit",
        "hit_test": "UiTree",
        "focus": "UiTreeOrder"
      },
      "render": {
        "visible": true,
        "opacity": 1.0,
        "target": "MainSurface"
      },
      "capture": {
        "color": true,
        "object_id": true,
        "mask": true,
        "actions": true
      }
    }
  ]
}
```

## Validation

- `Modal` layer は `input.route = Modal` を推奨。
- `Debug` layer は product mode では capability 必須。
- `HtmlUi` layer は Native Servo / Web DOM のどちらでも同じ `PublicId` を使う。
- `ObjectIdPass` を有効にする layer は object id namespace を持つ。
- 同じ `z` の sibling は tree order で安定化する。
