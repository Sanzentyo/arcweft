# module / use / pub / lazy use

## module

```awft
mod game::logic::affection
mod crate::game::routes::opening
mod self::routes::opening
mod super::shared
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
use crate::game::prelude::*
use self::characters::{alice, bob}
use super::common::{route_gate, shared_flags}
```

Module paths support Rust-like roots:

```text
crate   current package / crate root
self    current module
super   parent module
parent  reserved alias for super
```

`crate`, `self`, and `super` are canonical. If `parent` is accepted by a parser,
formatters should normalize it to `super`.

Grammar summary:

```text
ModulePath :=
    ("crate" "::" | "self" "::" | "super" "::" | "parent" "::")? IdentPath
```

```awft
mod parent::shared
```

normalizes to:

```awft
mod super::shared
```

Do not use `@.name` / `@..name` / `@super.name` relative ID syntax in module
paths. Those forms are only for ID-bearing contexts such as dialogue line IDs
and choice option IDs.

```awft
alice(id=@.greeting):       // relative dialogue line ID
use self::characters::alice // module-relative import
```

This is invalid:

```awft
use .characters::{alice}
```

Write:

```awft
use self::characters::{alice}
```

If `parent::` is accepted in `use`, it normalizes the same way:

```awft
use parent::common::{route_gate}
```

to:

```awft
use super::common::{route_gate}
```

## lazy use

```awft
lazy use mini_games::truck::{truck_game, TruckResult}
lazy use game::shaders::heavy::{crt_postprocess}
lazy use crate::mini_games::truck::{truck_game, TruckResult}
lazy use self::generated::route_map::{RouteMap}
lazy use super::shared::{SharedRouteState}
```

`lazy use` は export summary だけを読み、body parse/typecheck/compile/load は初回使用まで遅延する。

## eager use

```awft
eager use game::generated::route_map::{RouteMap}
eager use self::generated::route_map::{RouteMap}
eager use crate::game::generated::route_map::{RouteMap}
```

import 時副作用は禁止。`eager use` は compile/cache/diagnostics の都合で使う。

`lazy use` / `eager use` でも通常の `use` と同じ module-root 規則を使う。
`@.generated` / `@..generated` / `@super.generated` のような relative ID 形式は import path
ではなく ID 文脈専用なので使わない。

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
