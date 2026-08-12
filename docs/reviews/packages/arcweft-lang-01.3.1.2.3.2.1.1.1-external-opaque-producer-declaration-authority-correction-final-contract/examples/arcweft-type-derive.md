# Required derive example

```rust
use arcweft_rust_abi_macros::ArcweftType;

#[derive(ArcweftType)]
#[arcweft(opaque_producer = "example.gameplay")]
pub struct PlayerScore {
    value: i64,
}

#[derive(ArcweftType)]
#[arcweft(opaque_producer = "example.gameplay")]
pub enum Rank {
    Bronze,
    Silver,
    Gold,
}
```

The repeated producer is intentional: the two exact nominal identities share a
producer domain. Neither exact identity accepts the other at runtime because
external admission is exact.
