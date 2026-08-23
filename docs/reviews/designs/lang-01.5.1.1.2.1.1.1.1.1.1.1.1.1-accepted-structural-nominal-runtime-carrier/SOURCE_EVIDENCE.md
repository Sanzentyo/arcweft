# Source evidence

## Repository preflight

- inspected Git commit: `9a5d30d25620541c3f2975d31e04e04e3bc9514c`;
- origin/main: the same full SHA;
- branch: `main`;
- production paths were clean at that commit; the shared checkout contained
  only separately owned documentation/package intake work before this design;
- `cargo metadata --no-deps --format-version 1`: passed; and
- design path: absent before creation.

The validator checks HEAD, Cargo metadata, request bytes, and every blob below.

## Exact source blobs

| Path | Git blob | Evidence |
|---|---|---|
| `Cargo.toml` | `5545e64710263085a959aa2fc024f4e7d72bf3f3` | workspace |
| `crates/arcweft-core/Cargo.toml` | `22091bfdea72b6168576181b2cc62fdd25a75ada` | lower layer |
| `crates/arcweft-core/src/value.rs` | `5ba6bf6a1e2b800f72fbca1fec013e7ff387031a` | live value/field/variant |
| `crates/arcweft-core/src/value/nominal_record.rs` | `b14815895681bcb146d99352fe7e6e40dcbfe242` | record layout/value |
| `crates/arcweft-core/src/value/record_id.rs` | `8edc08d59c6c927da486b4b487fd8b2e8c50e5e9` | field ID |
| `crates/arcweft-core/src/value/awbc_save.rs` | `9e7cb04c910c73ee17b38ac7571c64e3be3804ad` | snapshot/raw restore |
| `crates/arcweft-core/src/pattern.rs` | `19a0b8a6e1f35babe9ecdddebe7402b3c73a9fdb` | checked types/variant ID |
| `crates/arcweft-core/src/entry/schema.rs` | `81f6cf0a98549e7efd6068425cae40963dfe889a` | schema/layout/value digest |
| `crates/arcweft-core/src/entry/identity.rs` | `dabe45e0cf5ddae15ddcc741e83d6b1a8ee0bfcf` | nominal/layout IDs |
| `crates/arcweft-core/src/plan/type_kind.rs` | `d10e1da9969fa8dbe5ab61f40be6f971fb2ac74a` | plan type algebra |
| `crates/arcweft-core/src/plan/type_table.rs` | `41cb23f4d5518753b588f3a20f0e7855104cd32b` | atomic type graph |
| `crates/arcweft-core/src/plan/nominal_record_domains.rs` | `92e4f6e14afdeb9e5702536fe8eeb84db2a939c0` | record domains |
| `crates/arcweft-core/src/plan/variant_domains.rs` | `87a50126150d1aca64bdcdb8767317fd91e5115d` | variant domains |
| `crates/arcweft-core/src/plan.rs` | `298c58952d320ed688635d0d62cc9ba4f2393fc6` | plan predicate join |
| `crates/arcweft-core/src/awbc/schema.rs` | `825470354b16cdcb4354087287aad94dfb707b2f` | AWBC rows/tags |
| `crates/arcweft-core/src/awbc/codec/types.rs` | `1830e672f67c9ff88247b1e53f843e8d74109580` | allocation owner |
| `crates/arcweft-core/src/awbc/codec/wire.rs` | `bbdf6f2dc3c624b23f78f8d92730a64d76946440` | shortest varint |
| `crates/arcweft-core/src/awbc/type_projection.rs` | `4db73763ac82b6abcbb0066bb8e1bf9187bd2635` | checked inverse |
| `crates/arcweft-core/src/awbc/fiber.rs` | `3d3355654235e2f42ff28f4e08d681bbecbf9acf` | snapshot/program validation |
| `crates/arcweft-lang-sema/src/env/nominal.rs` | `a7fa3bf959f805d5b1a6280347cb8bf3a9ab87dd` | semantic/opaque catalog |
| `crates/arcweft-lang-sema/src/env/enums.rs` | `3f3a42267d5f1f98e093b949b697e47aa484bde7` | record order loss |
| `crates/arcweft-lang-sema/src/env/rust_metadata.rs` | `4d1b6aba5a4abd78e8afe83461382d7483a68f2d` | metadata/substitution |
| `crates/arcweft-lang-sema/src/registration/environment_input.rs` | `8c6415c981892e7ee90b0cf4f97965775fde0434` | opaque-only input |
| `crates/arcweft-lang-sema/src/registration/model.rs` | `ebc85dae7379de643b50caf453208eb6496d5ad5` | registered world |
| `crates/arcweft-lang-sema/src/registration/registrar.rs` | `48532ea695297fc445d3d79f8301ec4f63ccc254` | atomic publication |
| `crates/arcweft-lang-sema/src/registration/environment_digest.rs` | `ad2e2ad5f330f9bdc16cf967643b24f5dcb690df` | joined digest |
| `crates/arcweft-lang-sema/src/callable/projection.rs` | `109bf95513cf1e4e776155f000e009b0c3ccc067` | metadata projection |
| `crates/arcweft-lang-sema/src/types/digest.rs` | `8ee1d3cbe090a52bbacc361d8ee223c899b675f8` | instantiation ID |
| `crates/arcweft-lang-sema/src/final_analysis/nominal_schema.rs` | `9a554e2801847c73cfb5db56b12a3e39c756c8a0` | projection owner |
| `crates/arcweft-lang-sema/src/final_analysis/report.rs` | `ac4fafc7c029c7821b4d73a315e744bb55533dc1` | generation lease |
| `crates/arcweft-lang-sema/src/ownership.rs` | `81268cd65cded080e61424d5e53539a85b34f31b` | fail-closed branch |
| `crates/arcweft-adapter-sema/src/registration/input.rs` | `6327adddac472e754197a61c614d57166f45578f` | source producer/fake opaque |
| `crates/arcweft-runtime-plan/src/semantic_facts.rs` | `4cfec588efd4d14cabbde3eb3b84490878eb83c4` | normalized facts |
| `crates/arcweft-runtime-plan/src/awbc_lower.rs` | `6079d987c65630399a51d842880b2a4455c22f5f` | plan to AWBC |
| `crates/arcweft-compiler/src/lower.rs` | `8aa20063dea7e0186b54f745de803560072d2f07` | legal bridge |
| `crates/arcweft-runtime-driver/src/session/persistence.rs` | `7e484089e5b49b8798e0b8f06dd511eb0bf25ade` | candidate/swap |

Cargo blob identities and dependency expectations also appear in the machine
contract so the validator proves direction instead of inferring it.

## Result-changing current gaps

1. Rust accepted inventory fabricates an opaque carrier for every metadata row.
2. Enum record fields are collected into BTreeMap, losing order and replacing
   duplicates.
3. Accepted structural ownership deliberately fails closed.
4. RuntimeCheckedType has no structural record predicate and nominal variants
   do not retain layout.
5. Nominal records have a public unchecked constructor used by snapshot restore.
6. Variant values are directly constructible from public enum fields.
7. Canonical structural records sort by name instead of field identity.
8. AWBC constants duplicate names already owned by type descriptors.

These are the concrete reasons for the selected in-place extensions. No
unrelated owner is replaced.
