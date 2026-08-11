# Repository evidence

## 1. Inspection identity

```text
repository: Sanzentyo/arcweft
final inspected main: 27227bbc8e1d5c78d7b35c2865bad8fb6d00fca9
inspection date: 2026-07-20
access: GitHub connector at exact refs plus supplied local artifacts
local repository checkout: unavailable
Jujutsu change: UNAVAILABLE_FROM_REMOTE_GIT_SNAPSHOT
```

The connector does not expose the repository-local `.jj` operation store, so a
printable Jujutsu change ID cannot be verified. This evidence state is recorded
rather than fabricated; it is not an implementation decision.

## 2. Supplied-file evidence

```text
request SHA-256: 104d4acee8adf6e44303d4f0be3c1f4614b5ece112c4cff8e1ce5d9248ea9109
Rust Skill SHA-256: 1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665
project premise SHA-256: cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1
AW-AH-009.4.2 final-contract ZIP SHA-256: 05e825dde033f308f24fc1f6e504b4c26bba2d61fd33852ce880dc666ba8f2a8
```

`Rust Skill.txt` was read completely through line 56. Root `AGENTS.md` was read
completely through line 447 at the final baseline; blob:
`ea4a46132ff8cd004f860c89c854e4cbfe807d86`.

The AW-AH-009.4.2 ZIP was independently checked for its exact member list,
manifest hashes/zero self-entry, CRC/decompression, clean extraction equality,
and exact `OPEN_QUESTIONS.md`. Its source/HIR contract is therefore a verified
input to this package.

## 3. Governing predecessor identities

```text
AW-AH-009.4
  a86044fea7aaff3ec3829dfa0ad6552c88377ca61fa2911c3b96ea34ca0ffa5e
proof-concurrency v6.1.1
  1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef
AW-AH-009.3.2
  8701ff3ae6024cd62c33c4b36abdfa358bfa30aa93209655870c475eea1dd40d
AW-AH-009.4.2
  05e825dde033f308f24fc1f6e504b4c26bba2d61fd33852ce880dc666ba8f2a8
```

The first three archive byte streams were not supplied in this runtime and are
not falsely claimed as locally rehashed. Their identities and selected
substrate were corroborated by the governing request and current repository
implementation records. The result-changing rules for this task are explicitly
frozen by the supplied AW-AH-009.4.3 request.

## 4. Main movement

The prior AW-AH-009.4.2 package inspected
`e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139`. This audit refreshed main through
`bfdcc9de982212891454f0df869d9c15131403be` and finally 27227bbc8e1d5c78d7b35c2865bad8fb6d00fca9.
The intervening focused semantic-call-fact work reinforced exact SourceSpan and
accepted-project query ownership; the final resource-extension manifest commit
is unrelated to line identity. Relevant line/project owners were fetched at the
final baseline.

## 5. Inspected owners

| Path / owner | Blob or revision evidence | Finding used |
|---|---|---|
| `AGENTS.md` | `ea4a46132ff8cd004f860c89c854e4cbfe807d86` | full current repository policy; direct replacement, no source gates/shims |
| `crates/arcweft-lang-hir/src/project.rs` | `4210fe78ded858c01c299f2915d531c19a85a0cc` | package currently arrives at `HirProject::new`; module/source validation exists; no line inventory |
| `crates/arcweft-lsp/src/profiles/accepted_project.rs` | `01ee33a1dfcb745ece65a94ba2dacea1b7963a95` | one atomic `Arc<HirProject>`/source/semantic snapshot and exact module/source checks |
| `docs/implementation/2026-07-16-aw-ah-009-3-2-accepted-hir-request-lifecycle.md` | `c7810bb1387db10811bf904846b3719b1e83d7e2` | previous generation remains on rejection; identical source may reuse Arcs |
| `crates/arcweft-lang-hir/src/identity.rs` | `f2201ad1530e85f55b7166bab576c14e727fb150` | typed ExprId/ScopeId/HirSnapshotId, stale-ID rules, HIR limits |
| `crates/arcweft-lang-hir/src/symbol/identity.rs` | `562b27274900484b918537d27f3a4400cb6e7947` | existing CallablePackageId, CallableDeclarationId, owner family/path behavior |
| `crates/arcweft-lang-syntax/src/ast/module_path.rs` | `bd0d3fe0619278523d5ccea15a840d98a269056b` | typed CanonicalModulePath/ModuleSegment owner |
| `crates/arcweft-source/src/diagnostic.rs` | `c696f5b2e8e0ddf983f9975c1d1ad9c9885ff8bb` | Diagnostic supports typed codes, primary/secondary SourceSpan labels |
| `crates/arcweft-lang-hir/src/model.rs` | `fb4aad1ba6932fc914b3fc755d65a47f4e992a9a` | current single-range/string HirLowerError and revision-bound source-span lookup |
| `crates/arcweft-lang-hir/src/dialogue_identity.rs` | `fdb64db65afba71e8a23105c5123f715b2c7dd01` | old DialogueSpeakerSlug strips `.say` and reconstructs callee/narrator identity |
| `crates/arcweft-lang-hir/src/lower_ids.rs` | `98eb7dbe02902903793076582bbd64dffeb4e4bb` | old flow+speaker+scope ID generation, mutable counter, no project collision |
| `crates/arcweft-lang-hir/src/lower_context.rs` | `8b751e5a817ec1e65cee7bcedf202bcb01a5b429` | string flow/scopes and `HashMap` line counters |
| `crates/arcweft-dialogue/src/character_dialogue/limits.rs` | `89e68be55e392849bba855181a8ab2bef880e56d` | frozen `max_line_id_bytes = 256`; currently inaccessible downward from HIR |
| `crates/arcweft-dialogue/src/character_dialogue.rs` | `d3041d8985a0636e9f86cb6c0428e7ff85d178af` | later runtime content application carries RuntimeLineId/TextKey; not redesigned here |
| `crates/arcweft-id/src/lib.rs` | `1853c6d02de44b7e5fc8c4e763dbdd000f777f19` | lower PublicId/TextKey validation owner suitable for family wrappers |
| `crates/arcweft-core/src/runtime_id.rs` | `c4493fbfbfd350d0110773c06192550c852eba75` | runtime path IDs are distinct from source family labels |
| `docs/implementation/2026-07-20-aw-ah-009-3-focused-call-target-facts.md` | `09891697009a6969e44748a6b793b275528d92ac` | current exact accepted SourceSpan query pattern; no second resolver |

Ellipsized blob labels are used only where the connector's exact commit/path
fetch was the authority and the complete hash was not material to a decision.
No decision depends on an inferred blob.

## 6. Concrete findings

- Current project construction is typed enough to be the sole extension point,
  but package qualification is late and module maps are not package-qualified.
- Current accepted LSP publication already proves the desired atomic previous-
  generation preservation; a line-specific accepted snapshot would be a defect.
- Current line generation violates frozen semantics by consuming speaker/callee
  spelling and mutating a counter during lowering.
- Current text-key derivation itself replaces the family correctly, but relative
  explicit text-key construction also inserts speaker identity and must be
  deleted.
- Current diagnostic transport can render two revision-bound labels, while
  current HIR line errors cannot retain them.
- The exact 256-byte limit needs a lower identity owner to preserve layering.

## 7. Verification boundary

Verified in this design task:

- latest main identity at packaging refresh;
- full instruction files;
- AW-AH-009.4.2 archive identity/integrity;
- exact current production owner shapes and concrete defects;
- package content whitelist, manifest, hashes, extraction, and deterministic ZIP
  rebuild.

Not claimed until implementation:

- compilation of the proposed Rust shapes;
- Cargo tests, Clippy, formatting, workspace/Tier 2 execution;
- runtime behavior or wire compatibility;
- a local Jujutsu change ID; or
- production integration.

Those are implementation verification tasks, not unresolved contract choices.
