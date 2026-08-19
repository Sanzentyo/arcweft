# Explicit extension receiver implementation — 2026-08-19

## State

- Base Git revision: `dca42641414eb0b9fcec353ef0d3aab402eb723e`.
- Working tree at implementation: dirty only with this receiver cut.
- Performed: typed `self: Type` syntax/HIR retention, callable-schema receiver
  coordinate, checked method indexing, resolver/compiler projection, bare
  `self` local-path ownership, and deletion of the implicit data-last fallback
  identity/resolver family.
- Standard `map` overload migration: not performed in this cut; it is the next
  independently reviewable catalog cut.

## Final owner

`CallableSignatureSchema::extension_receiver` is the single semantic owner of
the receiver coordinate. The exact parameter type at that coordinate owns
owned/shared/mutable mode. The one free-callable record remains in the free
catalog and the checked method index references that same identity; no wrapper
method or duplicated callable record is created.

Parser projection uses a typed `ExtensionReceiverMarker`, HIR retains
`HirParameterKind::ExtensionReceiver`, and source-index validation correlates
both without rereading source spelling. Bare `self` is an identifier path;
`self::name` remains an explicit module-root path.

The accepted receiver positions are deliberately closed:

1. first parameter of the first group; or
2. sole parameter of the second and final group.

The second form supports `map(f)(value)`, `value |> map(f)`, and
`value.map(f)` without carrying a hidden receiver through arbitrary additional
partial-call groups.

## Deleted authority

The cut removes `DataLastCallableId`, `CallableCandidateId::DataLast`, the
data-last callable/diagnostic families, shape-only fallback scans, and
`resolve_data_last_method`. Dot syntax can no longer discover an ordinary free
function from name and type coincidence.

## Structural disposition

The callable schema/catalog remains cohesive: it owns accepted declaration
shape and lookup projection. Syntax owns lossless classification, HIR owns
source identity and locals, sema owns selection/argument mapping, and the
compiler consumes the ordinary resolved call. No dependency direction or
public crate boundary is reversed. The change extends repeated schema/digest
projection and therefore requires the canonical structure audit at the push
cut.

## Validation

Passed at the reviewable cut:

- `cargo check --workspace --all-targets --all-features` (baseline and changed
  workspace checks);
- syntax receiver-marker focused test;
- HIR receiver-first/data-last focused test;
- sema free/dot identity focused test;
- sema data-last final-group focused test;
- `cargo test -p arcweft-lang-sema --lib` — 203 passed;
- the two LSP regressions exposed by the workspace tier, after restricting
  runtime pure-helper admission to ordinary functions without an authored
  effect row;
- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets --all-features` — exit 0 with the
  repository's existing non-denied warnings;
- `just test-doc`;
- `just structure-audit` and `just structure-audit-gate` — 0 blocking
  violations.

`just test-workspace` passed the non-CLI workspace tier and the CLI library,
binary, and focused runtime tests. Its final directory-wide fixture gate failed
on four already scheduled convergence surfaces: current-pass LetElse HIR
coverage, dialogue localization policy, multiline trait-method lowering, and
dialogue line-plan execution. These failures are retained as the positive
fixture-gate work item; no allowlist, ignored test, or weakened expectation was
introduced by this cut.
