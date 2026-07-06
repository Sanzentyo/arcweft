# macro / template / precompile

## 宣言的 macro

```arcw
macro choice_route($id, $label, $target) {
    option $id {
        label = $label
        select { goto $target }
    }
}

choice @choice.opening.first {
    choice_route!(@choice.opening.listen, "聞いてみる", @flow.alice_intro)
}
```

## attribute macro

```arcw
#[derive(Serialize, Debug, Format)]
pub enum GameEvent { ... }
```

## template

Template は型付きの高水準 macro。

```arcw
template route_gate(
    character: Ref<Character>,
    required: i32,
    target: Ref<Flow>,
) -> FlowFragment {
    if state |> has_affection_at_least(character, required) {
        goto target
    } else {
        goto @flow.locked
    }
}
```

## precompile

Rust/WASM で実装された precompiler を module item として扱う。

```arcw
extern precompile mod route_tools from wasm "tools/route_tools.wasm" {
    pub macro generate_route_map
    pub derive RouteCoverage
}

@generate_route_map(root = @flow.opening)
mod generated_routes
```

## 出力

precompile 出力は raw text ではなく構造化する。

```arcw
pub enum PrecompileOutput {
    ExpandedAst(AstFragment),
    GraphPatch(GraphPatch),
    ModuleItems(Vec<ModuleItem>),
    Diagnostics(Vec<Diagnostic>),
}
```

## precompile demand

precompile 生成物も `ModuleItem` として扱う。利用側の構文は通常の
`use` で、生成・読み込みタイミングは compiler-owned demand model と
build cache が決める。

```arcw
use generated.route_map.{RouteMap}
```

## hygiene と source map

- macro expansion は source map を持つ。
- generated entity にも EntityId を付ける。
- Graph/RAG/JJ history は macro-generated item を追跡できる。


