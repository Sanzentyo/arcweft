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

