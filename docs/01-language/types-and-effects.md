# Option / Result / Need / lifetime

## null はない

```awft
let next: Option<Ref<Flow>> = None
let next = Some(@flow.alice_intro)
```

使用:

```awft
match next {
    Some(flow) => goto flow
    None => goto @flow.title
}
```

## Result

```awft
pub enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

`?` で伝播。

```awft
fn parse_choice(input: String) -> Result<Ref<ChoiceOption>, ParseError> {
    let id = parse_ref(input)?
    Ok(id)
}
```

## Need

```awft
pub enum Need<T, E> {
    NotStarted,
    Pending(Progress),
    Ready(T),
    Err(E),
    Cancelled,
}
```

暗黙 force は禁止。

```awft
let bg = asset.image(@asset.bg.room) // Need<ImageHandle, AssetError>
```

flow で使うには:

```awft
let bg = try await asset.image(@asset.bg.room) with {
    pending p => scene.show(@scene.loading); progress.set(p.ratio)
}
```

## suspension boundary

以下は suspension boundary。

- `await`
- `yield frame`
- `select`
- `thread`
- `defer`
- `lazy let` capture

`&'frame T`、`&'lease T`、`&mut T` は boundary を跨げない。

## lifetime

基本は推論。明示は zero-copy / Rust extern / shared memory / parser などで使う。

```awft
fn first<'a>(xs: &'a [ChoiceView]) -> Option<&'a ChoiceView> { ... }
```

組み込み lifetime:

```text
'frame   現在frame内
'flow    suspendしないflow部分
'scene   scene scope
'asset   asset lease
'lease   shared memory lease
'static  bundle定数
```

## EntityRef と BorrowRef

```awft
Ref<Flow>       // ID参照。lifetime不要。非null。
&'a T           // メモリ借用。lifetime必要。
```

`Ref<T>` は borrow ではない。

## Handle と Borrow

```awft
ImageHandle       // frameを跨げるowned handle
&'asset [Rgba8]   // borrow block内だけ
```

```awft
borrow bg.pixels() as pixels: &'asset [Rgba8] {
    let average = pixels.average_color()
}
```


