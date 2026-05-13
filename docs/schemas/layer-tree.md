# Layer Tree schema

Layer Tree は描画、入力、Agent 観測、headless capture の共通 schema である。

関連:

- [Layered Rendering](../03-presentation/layered-rendering.md)
- [Layered Input](../02-runtime/layered-input.md)
- [Agent Protocol](agent-protocol.md)

## LayerTree

```rust
pub struct LayerTree {
    pub revision: LayerRevision,
    pub root: LayerId,
    pub nodes: Vec<LayerNodeRecord>,
    pub render_order: Vec<LayerId>,
    pub input_order: Vec<LayerId>,
    pub object_id_map: Vec<ObjectIdRecord>,
    pub hash: LayerTreeHash,
}
```

`render_order` と `input_order` は別に保存する。通常は input_order は render_order の逆順だが、modal、capture、debug overlay、semantic-only layer により差が出る。

## LayerNodeRecord

```rust
pub struct LayerNodeRecord {
    pub id: String,
    pub entity: Option<String>,
    pub public_id: Option<String>,
    pub kind: String,
    pub parent: Option<String>,
    pub children: Vec<String>,

    pub z: i32,
    pub visible: bool,
    pub opacity: f32,
    pub transform: Transform2DRecord,
    pub clip: Option<ClipRecord>,
    pub blend: String,

    pub bbox: Option<BBoxRecord>,
    pub polygon: Option<PolygonRecord>,
    pub mask: Option<MaskRecord>,

    pub render: LayerRenderRecord,
    pub input: LayerInputRecord,
    pub observation_source: String,
}
```

## LayerInputRecord

```rust
pub struct LayerInputRecord {
    pub policy: String,
    pub hit_test: String,
    pub focus: String,
    pub keyboard: String,
    pub accepts: Vec<String>,
    pub blocks_lower_layers: bool,
}
```

Example JSON:

```json
{
  "id": "layer.choices",
  "entity": "layer.choices",
  "kind": "Choice",
  "z": 200,
  "visible": true,
  "bbox": { "space": "LogicalViewport", "x": 420, "y": 512, "w": 350, "h": 120 },
  "input": {
    "policy": "HitTest",
    "hit_test": "UiLayout",
    "focus": "FocusableChildren",
    "keyboard": "FocusedOnly",
    "accepts": ["PointerClick", "SemanticInvoke"],
    "blocks_lower_layers": false
  }
}
```

## ActionTarget and layer

Agent action target は layer を持つ。

```rust
pub struct ActionTargetRecord {
    pub id: String,
    pub target: String,
    pub layer: String,
    pub role: String,
    pub label: Option<String>,
    pub bbox: Option<BBoxRecord>,
    pub preferred_action: AgentActionRecord,
}
```

これにより Agent は「対象がどの layer にあり、modal に隠れていないか」を判断できる。

## RoutedInputRecord

```rust
pub struct RoutedInputRecord {
    pub raw_id: String,
    pub phase: String,
    pub target_layer: Option<String>,
    pub target_entity: Option<String>,
    pub target_ui_node: Option<String>,
    pub route: Vec<String>,
    pub disposition: String,
}
```

Replay trace には `LayerTreeHash` と `RoutedInputRecord` を保存する。

```json
{
  "tick": 182,
  "layer_tree_hash": "b3:layer...",
  "raw": { "kind": "PointerClick", "x": 595, "y": 536 },
  "routed": {
    "target_layer": "layer.choices",
    "target_entity": "choice.opening.listen",
    "route": ["layer.choices", "layer.dialogue", "layer.root"],
    "disposition": "Emit"
  }
}
```
