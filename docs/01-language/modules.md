# module / use / pub / lazy use

## module

```awft
mod game::logic::affection
```

item はデフォルト private。

```awft
fn internal_helper(...) -> ...
pub fn public_helper(...) -> ...
pub(crate) fn crate_helper(...) -> ...
pub(super) fn parent_helper(...) -> ...
```

## use

```awft
use game::prelude::*
use game::logic::affection::{affection_of, has_affection_at_least}
use game::ui::settings as settings_ui
pub use game::types::{GameState, GameEvent}
```

## lazy use

```awft
lazy use mini_games::truck::{truck_game, TruckResult}
lazy use game::shaders::heavy::{crt_postprocess}
```

`lazy use` は export summary だけを読み、body parse/typecheck/compile/load は初回使用まで遅延する。

## eager use

```awft
eager use game::generated::route_map::{RouteMap}
```

import 時副作用は禁止。`eager use` は compile/cache/diagnostics の都合で使う。

## ModuleItem

DSL、Rust、WASM、precompile 生成物を統一する。

```awft
pub struct ModuleItem {
    pub entity_id: EntityId,
    pub public_id: PublicId,
    pub path: ModulePath,
    pub kind: ModuleItemKind,
    pub visibility: Visibility,
    pub signature: ItemSignature,
    pub origin: ItemOrigin,
    pub lazy_policy: LazyPolicy,
}
```

```awft
pub enum ItemOrigin {
    DslSource(SourceAnchor),
    MacroGenerated(MacroExpansionId),
    RustStatic(RustExportId),
    RustDylib(RustExportId),
    WasmComponent(WasmExportId),
    PrecompiledBundle(BundleItemId),
}
```

## Rust export

```awft
extern rust mod mini_games::truck from crate "truck_game" {
    pub event TruckEvent
    pub type TruckResult
    pub fn score_to_rank(score: i32) -> Rank
    pub activity truck_game: Activity<TruckInput, TruckResult>
}
```

