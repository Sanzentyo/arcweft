# AW-AH-009 character nominal type identity

Date: 2026-07-14

Status: structural identity core and existing completion/hover path implemented;
remaining original acceptance items split into standalone design requests

Representative source package:
`C:\Users\sanze\.codex\codex-remote-attachments\019f5945-ad7b-76f0-ad40-15ace86d23d3\1BAFF90F-5065-4C10-8D7D-8AF4AD37D5E0\1-arcweft-aw-ah-009-character-nominal-type-identity-reupload-20260714.zip`

All four re-uploads were byte-identical: 57,875 bytes, SHA-256
`F8EC458561105D3B1751998B75886664AA4087874878E1F10AE9CB11E1206B0B`.
The package targets base commit `52d6fade`; that is the parent of the current
Jujutsu working-copy change. Independent AW-AH-003, AW-AH-015, and native Style
work were present in the shared checkout and are outside this note.

## Implemented result

`TypeKind::CharacterNominal(CharacterNominalType)` is now the sole semantic
representation for manifest-derived enum families:

- `Look { character: CharacterId }`
- `Part { character: CharacterId }`
- `Variant { character: CharacterId, part: CharacterPartId }`

The IDs remain validated domain types from `arcweft-character`. Equality and
hashing compare typed structure rather than formatted names. Inherent
`source_label()` and `Display` produce deterministic diagnostic/tooling labels;
there is no inverse parser and a same-looking `TypeKind::Named` is a different
type.

`TypeCheckEnv` registers enum inventories under those structural keys and maps
the current canonical and compact character symbols to one `CharacterId`.
Character presentation checking resolves that symbol before selecting the
expected look type. The previous string constructors and
`character_look_character` prefix/suffix parser were deleted directly, with no
dual representation or compatibility alias.

LSP completion uses the owned type label while retaining the structural
inventory key. Character hover accepts typed expected-type evidence from sema;
without that evidence it returns a deterministic ambiguity result when the
same local spelling belongs to multiple character families or owners instead of
selecting the first loaded manifest.

Focused tests cover typed inventory registration, label/display separation,
equal hashing, cross-character/family/part inequality, nested collection and
function identity, typed profile loading, existing character metadata, and
cross-character completion/hover provenance.

## Acceptance audit and explicit non-goals of this cut

| Original acceptance area | Current result |
| --- | --- |
| Structural look/part/variant identity and validated IDs | Implemented |
| Equality/hash independent of labels | Implemented and directly tested |
| Expected-type enum inventory and character look checking | Implemented for the existing environment path |
| Completion and hover without label parsing | Implemented and directly tested |
| Persisted query/AWBC/bundle/save/plugin codec | No real boundary found; none added |
| Fallible duplicate/conflict registration and arbitrary module/import aliases | Design-gated; AW-AH-009.1 |
| Complete structured mismatch/source-provenance contract across compiler/LSP | Design-gated; AW-AH-009.1 |
| Character go-to-definition and rename | Design-gated; AW-AH-009.2 |
| Character-aware signature help or evidence-backed non-applicability | Design-gated; AW-AH-009.3 |

The implementation-ready substrate is complete, but AW-AH-009 as originally
written is not marked fully complete until the following independently
throwable requests are resolved:

- [`AW-AH-009.1 registration, alias, and diagnostic contract`](../reviews/requests/2026-07-14-aw-ah-009.1-character-nominal-registration-alias-diagnostics-contract.md)
- [`AW-AH-009.2 definition and rename contract`](../reviews/requests/2026-07-14-aw-ah-009.2-character-nominal-definition-rename-contract.md)
- [`AW-AH-009.3 signature-help contract`](../reviews/requests/2026-07-14-aw-ah-009.3-character-nominal-signature-help-contract.md)

These requests must preserve the implemented structural identity unless they
demonstrate a concrete flaw.

## Package deviations

The package's design direction was retained, but its transformation and status
claims were not accepted blindly:

- The dry-run reconstruction script fails even on its stated base shape at the
  `hover.rs` import replacement because it expects an indented closing brace
  that the base file does not contain.
- Its changed-file inventory omits the existing `character_completions` caller,
  which must pass the new typed-hover argument.
- Its old hover test assumes first-match behavior for `.smile`; the final typed
  contract correctly treats a look/variant collision as ambiguous without an
  expected type, so the provenance test now supplies the structural look type.
- Its implementation note says there are no remaining TODOs while explicitly
  excluding definition and rename and omitting signature help, contrary to the
  packaged original request. Those items are recorded above rather than being
  silently treated as complete.
- AW-AH-003 additions in `types.rs` and `env.rs` were locally integrated and
  preserved rather than overwritten by the reconstruction script.

## Changed files

- `crates/arcweft-lang-sema/src/types.rs`
- `crates/arcweft-lang-sema/src/env.rs`
- `crates/arcweft-lang-sema/src/checker/presentation.rs`
- `crates/arcweft-lang-sema/tests/character_manifest_types.rs`
- `crates/arcweft-lsp/src/features/completion.rs`
- `crates/arcweft-lsp/src/features/character_metadata.rs`
- `crates/arcweft-lsp/src/features/hover.rs`
- `crates/arcweft-lsp/tests/character_completions.rs`
- `crates/arcweft-lsp/tests/character_manifest_profile.rs`
- `crates/arcweft-lsp/tests/character_nominal_identity.rs`
- this implementation note and the three follow-up requests linked above

## Structural audit

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-character-nominal-type-identity
```

The audit measured Jujutsu change `tuqmpsrwrnnp` over base commit `52d6fade`.
It scanned 2,730 files, 1,299 Rust files, 635,093 Rust physical LOC, and 90
package manifests. The result is zero error-level violations and 127 warnings;
the complete generated report is in
[`structure-audits/aw-ah-009-character-nominal-type-identity/`](structure-audits/aw-ah-009-character-nominal-type-identity/violations.md).

Relevant current-checkout measurements:

| Path | Role | Bytes | Physical LOC | Embedded tests |
| --- | --- | ---: | ---: | --- |
| `crates/arcweft-lang-sema/src/types.rs` | production semantic type owner | 30,144 | 918 | no |
| `crates/arcweft-lang-sema/src/env.rs` | production semantic environment registry | 46,714 | 1,398 | yes |
| `crates/arcweft-lang-sema/src/checker/presentation.rs` | production presentation checker | 23,267 | 572 | no |
| `crates/arcweft-lsp/src/features/hover.rs` | production hover orchestration | 34,104 | 1,001 | yes |
| `crates/arcweft-lsp/src/features/completion.rs` | production completion | 8,700 | 233 | no |
| `crates/arcweft-lsp/src/features/character_metadata.rs` | production character metadata | 6,999 | 211 | no |
| `crates/arcweft-lang-sema/tests/character_manifest_types.rs` | integration test | 7,155 | 184 | no |
| `crates/arcweft-lsp/tests/character_nominal_identity.rs` | integration test | 3,960 | 130 | no |

`env.rs` was already above the 1,200-LOC warning threshold at the base (1,365
physical lines) and already contained embedded tests. This cut adds the small,
cohesive manifest-registration map and queries to the environment that owns the
inventory; it adds no embedded tests and does not cross an error threshold.
Moving only these methods would not resolve the pre-existing registry hotspot,
so broader environment decomposition remains a separate structural concern.

No Cargo dependency changed. Normal dependency fan-in/fan-out remains 7/10 for
`arcweft-lang-sema` and 0/25 for `arcweft-lsp`; the intended direction remains
`arcweft-character -> arcweft-lang-sema -> arcweft-lsp`.

## Validation

Completed during the focused implementation loop:

```bash
cargo fmt -p arcweft-lang-sema -p arcweft-lsp -- --check
cargo check -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features
cargo test -p arcweft-lang-sema --test character_manifest_types --all-features
cargo test -p arcweft-lsp --test character_manifest_profile --test character_completions --test character_nominal_identity --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features -- -D warnings
git diff --check -- <AW-AH-009 paths>
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-character-nominal-type-identity
```

All final commands pass: two sema integration tests and five LSP integration
tests pass, clippy is clean with warnings denied, the scoped diff check is
clean, and the structural audit reports zero errors.

One earlier combined test invocation reached its 64-second wrapper timeout
while waiting for the shared build directory. On retry the sema tests passed
and the old untyped `.smile` hover assertion failed because the new contract
correctly detected its look/variant ambiguity. The test now supplies the typed
look expectation, and the complete final command above passes. Slow Tier 2 MCP
and visual suites are outside this sema/LSP-only risk area.
