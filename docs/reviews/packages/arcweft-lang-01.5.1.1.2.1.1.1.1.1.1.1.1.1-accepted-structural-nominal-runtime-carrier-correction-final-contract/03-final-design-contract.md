# Final design contract

## 1. Normative scope

この契約は、checker が **accepted** と判定した structural nominal 値を runtime、generic match、snapshot、restart restore の全経路で同じ意味のまま運ぶ carrier を確定する。対象外は parser surface syntax の追加、公開 user ABI の拡張、production patch の作成である。

規範語 `MUST` / `MUST NOT` は実装 admission 条件である。`OPEN_QUESTIONS=0`。

## 2. Single authority and ownership

| concern | normative owner | rule |
|---|---|---|
| runtime discriminant | existing enum `RuntimeValue` in `crates/arcweft-runtime/src/value.rs` | `AcceptedStructuralNominal` variant をこの enum に追加する。別 enum に同じ discriminant を複製しない。 |
| payload data | `AcceptedStructuralNominalRuntimeCarrier` | nominal identity、accepted structural/layout identity、declaration-order children を所有する。fields は非公開。 |
| construction | `RuntimeValue` inherent `impl` | checker/admission と restore の validating constructors だけが carrier を作れる。 |
| projection/match | `RuntimeValue` inherent `impl` | generic match、coverage transcript、debug view は同じ borrow-only projection を使う。 |
| encode/decode dispatch | `RuntimeValue` inherent `impl` | explicit wire tag から payload codec へ dispatch する。extension trait は作らない。 |
| restore publication | existing runtime restore coordinator | carrier decode/resolve/validate 完了後だけ task/handle/value graph を publish する。 |

`AcceptedStructuralNominalRuntimeCarrier` 自身の inherent `impl` は local invariant の validation と field access に限る。runtime enum の variant dispatch を carrier 側 trait に反転させない。

## 3. Concrete Rust shape

次を normative API とする。current source に同義型がある場合は alias/new wrapper を増やさず、その既存型名へ 1:1 置換する。ただし field 意味、visibility、fallibility、owner は変更しない。

```rust
#[derive(Debug)]
pub(crate) struct AcceptedStructuralNominalRuntimeCarrier {
    nominal: AcceptedNominalTypeId,
    layout: AcceptedStructuralLayoutId,
    fields: Box<[RuntimeValue]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcceptedStructuralNominalCarrierError {
    UnknownNominal,
    UnknownLayout,
    NominalLayoutMismatch,
    FieldCount { expected: u32, actual: u32 },
    FieldType { index: u32 },
    NonCanonicalIdentity,
    UnsupportedWireVersion { found: u16 },
    TrailingBytes,
}

impl AcceptedStructuralNominalRuntimeCarrier {
    pub(crate) fn try_new(
        nominal: AcceptedNominalTypeId,
        layout: AcceptedStructuralLayoutId,
        fields: Box<[RuntimeValue]>,
        accepted: &AcceptedNominalCatalog,
    ) -> Result<Self, AcceptedStructuralNominalCarrierError>;

    pub(crate) fn nominal(&self) -> &AcceptedNominalTypeId;
    pub(crate) fn layout(&self) -> &AcceptedStructuralLayoutId;
    pub(crate) fn fields(&self) -> &[RuntimeValue];
}

impl RuntimeValue {
    pub(crate) fn try_accepted_structural_nominal(
        nominal: AcceptedNominalTypeId,
        layout: AcceptedStructuralLayoutId,
        fields: Box<[RuntimeValue]>,
        accepted: &AcceptedNominalCatalog,
    ) -> Result<Self, AcceptedStructuralNominalCarrierError>;

    pub(crate) fn accepted_structural_nominal(
        &self,
    ) -> Option<&AcceptedStructuralNominalRuntimeCarrier>;

    pub(crate) fn encode_canonical(
        &self,
        out: &mut impl CanonicalWrite,
    ) -> Result<(), RuntimeValueEncodeError>;

    pub(crate) fn decode_pending(
        input: &mut impl CanonicalRead,
    ) -> Result<PendingRuntimeValue, RuntimeValueDecodeError>;
}
```

### 3.1 Existing enum edit

`RuntimeValue` の元定義へ次の variant を直接追加する。

```rust
AcceptedStructuralNominal(Box<AcceptedStructuralNominalRuntimeCarrier>),
```

Box は recursive runtime value の enum size を bounded にするために variant payload へ一度だけ置く。`fields` も boxed slice とし、`Vec` capacity や allocator state を semantic/wire state として保持しない。current enum が既に全 variant を Box 化する別方針を持つ場合は AGENTS/clippy policy に従い一重 boxing を維持し、二重 Box は禁止する。

### 3.2 Constructor authority

`try_new` は次をこの順で検証する。

1. `nominal` が accepted catalog に resolve する。
2. `layout` が resolve し、その owner nominal が `nominal` と exact match する。
3. layout field count と `fields.len()` が一致し、`usize -> u32` 変換が checked である。
4. declaration order の各 child value が catalog の accepted child type と一致する。
5. identity bytes/digest が canonical form である。
6. 成功後にだけ `Self` を返す。途中状態を global registry、task slot、snapshot object table に publish しない。

checker 側は accepted semantic facts を保持する token/catalog view を渡す。runtime 側が checker の判定を structural re-inference してはならない。restore 側も unchecked field assignment をせず、resolve 済み catalog view で同じ `try_new` を呼ぶ。

## 4. Identity and equality semantics

| operation | nominal | layout | fields | result |
|---|---:|---:|---:|---|
| exact runtime equality | exact | exact | recursive exact | equal |
| same layout/fields, different nominal | different | any | same | not equal; not same accepted carrier |
| same nominal/fields, different accepted layout | same | different | same | reject construction or not equal according to catalog version; never silently coerce |
| generic nominal pattern | pattern nominal must equal | layout must be admitted for nominal | children matched in declaration order | match |
| structural-only pattern explicitly requested by language | nominal ignored only at that pattern boundary | structural layout checked | children matched | match result cannot be reused as nominal proof |
| snapshot compatibility | exact stable identity bytes | exact canonical layout identity/digest | recursively decodable | compatible |

`Hash`, digest、memoization key、coverage transcript key を実装する場合は `(wire_version, nominal, layout, fields)` の順を用いる。pointer address、intern table slot、process-local `TypeId` hash を含めない。

## 5. Match and coverage integration

1. matcher は `RuntimeValue::accepted_structural_nominal()` を一度だけ呼び、payload の private fields を別 module から destructure しない。
2. nominal pattern は nominal identity を先に比較し、不一致なら child traversal を開始しない。
3. structural pattern は language semantics が明示的に structural である場合だけ layout/children を見る。nominal acceptance の evidence を生成しない。
4. generic substitution/transcript には stable nominal + layout identity を記録し、display name や field-name text を authority にしない。
5. coverage closure は全 accepted variants の projection mapping を exhaustively match する。`_ => structural`、`Debug` string、unknown-as-empty は禁止する。
6. `RuntimeValue` に variant が追加されたら matcher、encoder、decoder、drop/visit/size accounting の exhaustive match を compiler error で追従させる。wildcard arm で封じない。

## 6. Canonical byte grammar

Multi-byte integer は little-endian。length は canonical unsigned LEB128 (`u32` 範囲)、overlong encoding を拒否する。enum discriminant の Rust 表現を wire にしない。

```text
accepted_structural_nominal :=
    tag:u8                    ; assigned explicitly in RuntimeValue wire-tag table
    version:u16le             ; 0x0001
    nominal_len:uleb32
    nominal_bytes[nominal_len]; canonical AcceptedNominalTypeId encoding
    layout_len:uleb32
    layout_bytes[layout_len]  ; canonical AcceptedStructuralLayoutId encoding/digest
    field_count:uleb32
    repeated field_count {
        child_len:uleb32
        child_runtime_value[child_len]
    }
```

Normative decoder rules:

- unknown `tag` は `UnknownRuntimeValueTag`、known tag + unknown version は `UnsupportedWireVersion`。
- `nominal_len`、`layout_len`、`field_count`、`child_len` は allocation 前に configured limits と remaining bytes の双方で検査する。
- identity decoder は canonical form を再 encode して bytes equality を確認するか、同等の canonical decoder guarantee を持つ。
- child は `PendingRuntimeValue` として decode し、catalog/object references 解決前に accepted carrier を構築しない。
- frame 末尾の trailing bytes を拒否する。
- encoder は常に v1 を生成し、legacy implicit enum discriminant を出力しない。

### 6.1 Wire tag allocation

既存 `RuntimeValue` wire-tag table の未使用値を **その table の owner moduleで**割り当てる。request/current source が特定値を既に予約している場合はその値を使う。値が未予約なら実装 PR で table の max+1 を割り当て、同じ commit で golden bytes を固定する。payload module が独自 tag registry を持つことは禁止する。

## 7. Two-phase restore

```text
Decode phase
  bytes -> PendingRuntimeValue::AcceptedStructuralNominal { raw nominal, raw layout, pending children }
       no registry/task publication

Resolve/validate phase
  resolve nominal -> resolve layout -> resolve children
  -> AcceptedStructuralNominalRuntimeCarrier::try_new(...)
  -> PreparedRuntimeValue
       still not externally visible

Commit phase
  restore coordinator atomically installs all prepared values/handles/tasks
  -> RuntimeValue::AcceptedStructuralNominalRuntimeCarrier

Abort
  any failure drops pending/prepared graph and leaves pre-restore observable state unchanged
```

Forward references は pending object IDs で保持し、resolve 完了前に raw pointer/`Arc` placeholder を semantic value として露出しない。同じ snapshot 内の duplicate object ID、catalog mismatch、cycle policy 違反は typed restore error として commit 前に返す。

## 8. Error mapping

| failure | constructor error | decode/restore surface | retryable |
|---|---|---|---:|
| nominal absent | `UnknownNominal` | catalog resolution failure | catalog supplied anew only |
| layout absent | `UnknownLayout` | catalog resolution failure | catalog supplied anew only |
| identity/layout disagree | `NominalLayoutMismatch` | corrupt/incompatible snapshot | no |
| wrong arity | `FieldCount` | corrupt value / implementation bug | no |
| child type mismatch | `FieldType { index }` | corrupt/incompatible snapshot | no |
| non-canonical identity | `NonCanonicalIdentity` | malformed bytes | no |
| unknown version | `UnsupportedWireVersion` | compatibility rejection | newer decoder only |
| trailing bytes | `TrailingBytes` | malformed frame | no |

Errors retain stable identity/digest and index, but do not require cloning affine runtime children or dumping user payload into logs.

## 9. Visibility and API hygiene

- payload fields: private to owner module.
- constructors/projections: `pub(crate)` or narrower, matching actual call graph.
- no `From<(nominal, layout, fields)>` because it cannot validate.
- no `Default` for accepted carrier.
- no blanket `Clone` when `RuntimeValue` is affine; derive only traits satisfied by semantic ownership.
- no extension trait for `RuntimeValue`. Missing behavior is added to its original inherent `impl`.
- no helper whose only purpose is to reproduce an enum `match` outside the owner module.
- display/debug may delegate to a semantic formatter but is never persisted or compared.

## 10. Compatibility and migration

- New snapshots encode explicit v1 tag/version.
- A legacy snapshot is accepted only if current source already has a documented legacy grammar that can recover both nominal and accepted layout identities losslessly. Structural bytes alone cannot be upgraded by guessing nominal identity.
- If legacy data lacks nominal identity, decoder returns a dedicated incompatibility error; it does not select the sole currently-known nominal declaration.
- Unknown future versions are retained only as opaque bytes if the surrounding runtime already defines an opaque-forwarding contract; otherwise reject. They must not masquerade as accepted carriers.
- Catalog identity/version is validated before commit. Recompiled declarations with same display name but changed stable identity are incompatible.

## 11. Invariants checklist

1. Every `RuntimeValue::AcceptedStructuralNominal` value came through a validating constructor.
2. `nominal` and `layout.owner_nominal` are exact equal.
3. fields are declaration ordered and exact arity.
4. each field satisfies the accepted child type at the same index.
5. nominal and layout encodings are canonical and process independent.
6. match/equality/hash/encode all observe the same triplet `(nominal, layout, fields)`.
7. restore publishes no partial value graph.
8. no duplicate nominal side table or match-only shadow carrier exists.
9. every owner enum variant has explicit match/codec/visitor handling.
10. all request traceability rows have executable test IDs.
