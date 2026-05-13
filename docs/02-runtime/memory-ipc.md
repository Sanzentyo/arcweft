# shared memory / IPC

## Control plane / Data plane

```text
Control Plane:
  小さいevent、command、lifecycle、error
  serializeしてreplayに残す

Data Plane:
  大きい配列、mesh、telemetry、audio buffer、physics state
  shared memory / mmap / ring buffer / frame arena
  replayにはhashやsummaryだけ残す
```

## Memory descriptor

```rust
#[repr(C)]
pub struct SharedSliceDesc {
    pub buffer_id: u64,
    pub offset: u64,
    pub len: u64,
    pub stride: u32,
    pub type_id: u128,
    pub layout_hash: u128,
    pub epoch: u64,
}
```

## MemoryLease

```rust
pub struct MemoryLease {
    pub id: MemoryLeaseId,
    pub owner: ActivityId,
    pub access: AccessMode,
    pub lifetime: LeaseLifetime,
    pub layout: MemoryLayoutSpec,
}

pub enum AccessMode {
    ReadOnly,
    WriteOnly,
    ReadWriteExclusive,
    SingleWriterMultiReader,
}
```

## 共有メモリ型の制約

必須:

- `#[repr(C)]` または明示 layout。
- pointer/reference/Drop なし。
- endian/alignment/version/layout_hash 明示。

禁止:

- `Vec<T>`
- `String`
- `Box<T>`
- `HashMap<K, V>`
- trait object
- raw pointer

## zero-copy borrow

```rust
borrow lease as telemetry: &'lease [TruckTelemetry] {
    let speed = telemetry.last()?.speed
}
```

borrow は await/yield を跨げない。

