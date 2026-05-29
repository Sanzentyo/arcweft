# ADT と pattern matching

## enum

```arcw
pub enum Route {
    Opening,
    AliceIntro { from_choice: Ref<ChoiceOption> },
    BadEnd(BadEndReason),
}

pub enum BadEndReason {
    SilentTooLong,
    FailedTruckGame { score: i32 },
}
```

## match

```arcw
fn route_title(route: Route) -> String {
    match route {
        .Opening => "Opening",
        .AliceIntro { .. } => "Alice",
        .BadEnd(reason) => bad_end_title(reason),
    }
}
```

match は exhaustive check される。

## anonymous sum

匿名直和は variant 名ではなく型そのもので分岐する。

```arcw
fn read_payload(path: VirtualPath) -> String | Bytes {
    if is_text(path) { read_text(path) } else { read_bytes(path) }
}

match read_payload(path) {
    text: String => render_text(text)
    bytes: Bytes => render_bytes(bytes)
}
```

`A | B` は型だけで区別できる private helper、local return、error set、
variadic argument に向く。branch 名が意味を持つ場合は `enum` を使う。

```arcw
// OK: type identity is enough
String | Bytes
IoError | ParseError

// Not anonymous sum syntax. Use enum when labels matter.
enum Payload {
    Text(String),
    Binary(Bytes),
}
```

`String | String` や transparent alias が同じ型に消える `Name | Email` は
取り出し時に区別できないためエラー。型レベルで区別したい場合は
refined/nominal newtype を使う。

## derive

```arcw
#[derive(Clone, Debug, Format, Serialize, Eq)]
pub enum GameEvent {
    StartGame,
    ChoiceSelected { id: Ref<ChoiceOption> },
}
```

`Format` は deferred logging 用。

## struct

```arcw
pub struct SettingsInput {
    text_speed: f32,
    master_volume: f32,
}
```

## newtype

```arcw
pub type PlayerName = String
where len(self) >= 1
where len(self) <= 16
```

## Option / Result

組み込み ADT として扱う。

```arcw
Option<T> = Some(T) | None
Result<T, E> = Ok(T) | Err(E)
```

