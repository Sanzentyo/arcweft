# Stable synthetic-key fingerprint transcript

## 1. Owner and API

`arcweft_lang_hir::identity` owns the encoder and the opaque output:

```rust
pub const SYNTHETIC_KEY_FINGERPRINT_INPUT_LEN: usize = 51;

pub struct SyntheticKeyFingerprintInput([u8; 51]);

impl SyntheticKey {
    pub fn fingerprint_input(self) -> SyntheticKeyFingerprintInput;
}

impl SyntheticKeyFingerprintInput {
    pub const fn as_bytes(&self) -> &[u8; 51];
}
```

The tuple field and constructor are private. The API exposes canonical read-only bytes, not a raw HIR ID, numeric slot accessor, decoder, or persisted wire format.

## 2. Exact byte layout

The transcript is exactly 51 bytes:

| offset | length | value |
|---:|---:|---|
| 0 | 29 | ASCII `arcweft-hir-synthetic-key-v1` followed by one NUL byte |
| 29 | 1 | owner-kind tag |
| 30 | 8 | process-local `HirDatabaseId` nonzero value, unsigned u64 little-endian |
| 38 | 4 | `HirModuleId` nonzero module slot, unsigned u32 little-endian |
| 42 | 4 | owner HIR nonzero slot, unsigned u32 little-endian |
| 46 | 1 | `SyntheticRole` tag |
| 47 | 4 | ordinal, unsigned u32 little-endian |

There are no variable-length fields. The domain's trailing NUL is the sole domain/field separator; fixed widths make further separators or lengths unnecessary. Any future layout or tag reinterpretation requires a different domain version tag.

## 3. Stable owner tags

| tag | owner / `HirIdKind` |
|---:|---|
| `0x01` | Item |
| `0x02` | Scope |
| `0x03` | Local |
| `0x04` | Expr |
| `0x05` | Stmt |
| `0x06` | Type |
| `0x07` | Pattern |
| `0x08` | Capture |

`0x00` and `0x09..=0xff` are reserved in v1. Tags are emitted by an inherent match on `SyntheticOwner`; they are not `mem::discriminant`, enum declaration indexes, or `as u8` casts.

## 4. Stable role tags

| tag | role |
|---:|---|
| `0x01` | ImplicitUnitTail |
| `0x02` | PredicateBoolReturn |
| `0x03` | ProofUnitReturn |
| `0x04` | ElidedRegion |
| `0x05` | RecoveryOperand |
| `0x06` | PostconditionResult |
| `0x07` | DesugaredTemporary |
| `0x08` | MissingRequiredTail |
| `0x09` | DestructuredBinding |
| `0x0a` | ClosureEnvironment |
| `0x0b` | ClosureCapture |
| `0x0c` | ContractRequiresScope |
| `0x0d` | ContractEnsuresScope |
| `0x0e` | ForIterator |
| `0x0f` | ForNextValue |
| `0x10` | IfLetScrutinee |
| `0x11` | WhileLetScrutinee |
| `0x12` | MatchScrutinee |
| `0x13` | PatternRest |
| `0x14` | PostfixIndexCandidateExpression |
| `0x15` | DialogueContentCandidateExpression |

`0x00` and `0x16..=0xff` are reserved in v1. The tag method is an explicit inherent match.

## 5. Session qualification and digest boundary

The process-local database ID is always encoded. Therefore equal source in a new process/database may produce a different transcript. This is deliberate: the transcript fingerprints the database-qualified session key, not a portable source artifact.

The HIR identity layer emits transcript bytes only. It defines no digest algorithm and no digest output type. An existing higher cache/fingerprint owner may feed the 51 bytes into its already accepted domain-separated hasher. A persisted build artifact must combine portable project/source identity and must not use this process-local transcript alone.

`std::hash::Hash` output is explicitly non-normative and is never copied into the transcript.

## 6. Fixed vectors

### Vector A: Type-owned elided region

```text
owner kind     Type = 0x06
database       1
module slot    2
HIR slot       3
role           ElidedRegion = 0x04
ordinal        0
```

Expected 51-byte lowercase hexadecimal transcript:

```text
617263776566742d6869722d73796e7468657469632d6b65792d76310006010000000000000002000000030000000400000000
```

### Vector B: nested dialogue candidate

```text
owner kind     Expr = 0x04
database       0x0102030405060708
module slot    0x0a0b0c0d
HIR slot       0x11121314
role           DialogueContentCandidateExpression = 0x15
ordinal        7
```

Expected transcript:

```text
617263776566742d6869722d73796e7468657469632d6b65792d7631000408070605040302010d0c0b0a141312111507000000
```

## 7. Collision-separation obligations

Tests mutate one field at a time and require different transcripts for:

- every owner tag;
- database ID;
- module slot;
- HIR slot;
- every role tag; and
- ordinal.

The same valid key must reproduce the same 51 bytes. Transcript lexicographic order is not required to match structural `Ord`; equality and collision separation are the requirements.
