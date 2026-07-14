# AW-AH-015 vertical-break quality policy — implementation design

Date: 2026-07-14
Target source revision: `52d6fadee2a0eee2fc3a565c4a2354e325eb49a1`
Policy identity: `balanced_v1`

## Decision

`arcweft-text-layout` owns a closed `VerticalBreakPolicy`, the checked objective,
total tie-break, bounded explain result, and the production planner. The first
contract has one preset and does not expose arbitrary weights. `TextLayoutRequest`
contains the closed identity so serde rejects unknown names and `TextLayoutHash`
includes the policy identity.

Priority is enforced in this order:

1. UAX #14 and typed JLREQ prohibited boundaries are absent from the graph.
2. Required JLREQ `keep_together` boundaries are absent from the graph.
3. Forced non-hanging overflow is minimized before every aesthetic term.
4. Allowed punctuation hanging, intermediate raggedness, short final columns,
   generated pair preference, and column creation are scored.
5. Fewer columns win after equal overflow and soft cost.
6. Equal tuples prefer the lexicographically later break-offset vector, which
   preserves deterministic fill-forward behavior without floating epsilon tests.

Renderers receive the selected common layout and never rescore breaks. The same
inline plan is therefore used by `vertical_rl`, `vertical_lr`, Native, Web, and
headless consumers.

## Hard constraints and terminal escape

For a boundary before cluster `i`, the planner consults existing owners only:

- `break_allowed_before` from UAX #14;
- `jlreq_punctuation::is_line_end_prohibited_cluster` for the left cluster;
- `jlreq_punctuation::is_line_head_prohibited_cluster` for the right cluster;
- `pair_adjustment_for_clusters(...).keep_together` from the generated table.

A rejected boundary is not assigned a large cost; it is not a DP edge.

Non-hanging overflow is an escape, not an aesthetic choice. From each column
start, a candidate with non-hanging overflow is considered only when it is the
shortest legal fragment. Consequently a later overflowing fragment cannot be
chosen strategically when an earlier legal fragment fits. The selected plan is
marked `ForcedOverflow`, and explain data records amount and column count.

A partially occupied column from an adjacent styled run is continued unless its
first legal fragment would require non-hanging overflow. In that case the plan
contains offset `0`, meaning restart at a fresh column. A fragment that fits by
permitted punctuation hanging still continues the existing column.

## Normalization and scale behavior

The representative unit is the lower median positive shaped cluster advance for
the hard-line segment. Lower median is deterministic, robust against a minority
of compressed punctuation cells, and scales with the actual font metrics.

All non-negative physical metrics are converted once to unsigned fixed point:

```text
normalized(x) = floor(x / representative_advance * 4096 + 0.5)
```

One representative em is therefore `4096` units. Capacity, used advance,
remaining space, overflow, and hanging allowance share this unit. After
normalization, all comparisons and cost calculations are integer-only.

Uniformly scaling every advance, origin, cursor, and container dimension
preserves ratios and break offsets. A decision may change only when a value
crosses the explicit half-quantum normalization boundary. The corpus is tested
at `0.5x`, `1x`, `2x`, and `4x`.

Bounds:

- maximum segment: `4096` clusters;
- maximum normalized metric: `4096em` (`16,777,216` units);
- powers and coefficient multiplication: checked `u128`;
- accumulated objective: checked `u64`/`u32`;
- invalid, negative, non-finite, out-of-domain, overflowed, and resource-limit
  inputs return `VerticalBreakError`; no NaN, infinity, saturation, panic, or
  zero-substitution participates in selection.

## Objective tuple

The plan tuple is minimized lexicographically:

```text
(
  forced_overflow_units,
  forced_overflow_columns,
  soft_cost,
  column_count,
  reverse_lexicographic_break_offsets
)
```

The last component is expressed operationally as “prefer lexicographically
later offsets” after the four integer fields are equal.

### Soft terms

All ratio terms use checked integer exponentiation and round half-up.

| Term | Equation | Maximum per column | Meaning |
| --- | --- | ---: | --- |
| Intermediate raggedness | `round(4096 * (remaining / capacity)^3)` | 4096 | Strongly discourages visibly empty intermediate columns while preserving small slack. |
| Final shortness | `round(1536 * (deficit / threshold)^2)` where `threshold = min(capacity / 3, 2em)` | 1536 | Applies only to a final column after at least one prior column. Ordinary final raggedness is free. |
| Hanging | `round(192 * (used_hanging / allowed_hanging)^2)` | 192 | Permits defined punctuation hanging but lets a good earlier break compete. |
| Intermediate break | `64` | 64 | Prevents gratuitous extra columns when other terms are close. |
| Pair preference | `16 * generated_break_penalty` | table-bounded | Converts the existing typed strictness table into objective points without duplicating punctuation facts. |

### Calibration rationale

The coefficients were selected from a discrete review grid against the checked-in
14-case corpus, not copied from the removed floating literals.

- A generated closing/opening penalty of `5`, `25`, or `100` maps to `80`,
  `400`, or `1600` points. These are comparable to cubic raggedness at roughly
  `27%`, `46%`, and `73%` empty capacity, respectively, which makes loose,
  normal, and strict presets observably distinct without turning preferences
  into hard constraints.
- Full permitted hanging costs `192`, comparable to about `36%` intermediate
  raggedness. Thus a small punctuation overhang can beat a very early legal
  break, while a nearly full earlier column can still win.
- The `64` break charge is comparable to `25%` cubic raggedness and only
  discourages an otherwise marginal extra column.
- Final shortness is bounded below intermediate full-raggedness because Japanese
  narrative composition accepts a short final column but should still avoid an
  isolated tiny tail when a comparable partition exists.

The corpus baseline records the chosen offsets and known forced-overflow cases.
Changing a coefficient or equation requires explicit delta review; passing unit
tests alone does not approve a new default.

## Hanging policy

Eligibility remains owned by `jlreq_punctuation::is_hanging_cluster`, currently
closing punctuation and middle dots. Maximum allowance is half of the cluster's
already resolved advance. Compression therefore happens before hanging and is
not duplicated by the policy.

Allowed hanging contributes only the bounded soft term. Overflow beyond the
allowance contributes to the first objective field and invokes the shortest-
legal-fragment escape rule.

## Final-column policy

The final column pays no ordinary raggedness, pair preference, or intermediate
break charge. If it follows another column and uses less than
`min(capacity / 3, 2em)`, a quadratic short-final term applies. A one-column
paragraph is never penalized merely for being short.

## Explain contract

`VerticalBreakExplain` is produced by the same evaluation that selects the
plan. It contains:

- policy identity and representative-advance bits;
- normalized initial/full capacities and unit scale;
- partial-column restart status;
- objective tuple and normal/forced-overflow status;
- aggregate counts for each rejected hard-constraint kind;
- up to 64 selected-column term records plus an omitted count;
- whether objective fields alone won or the later-offset tie-break was used.

The trace is renderer-neutral and bounded. It does not include rejected text or
an unbounded candidate graph.

## Configuration and hash boundary

`TextLayoutRequest.vertical_break_policy` serializes as the closed string
`"balanced_v1"`. Serde rejects missing fields unless the enclosing caller uses
an explicit default, malformed values, object-shaped values, and unknown names.
There is no separate persisted codec and no legacy reader because the previous
literals had no serialized identity.

The text-layout hash domain advances to `arcweft.text-layout.v2` and hashes the
policy stable ID. No selectable legacy objective remains.

## Corpus and approval rule

The manifest is
`crates/arcweft-text-layout/tests/fixtures/vertical_break_quality/v1/manifest.json`.
All phrases are original and released as `CC0-1.0`. Records contain case ID,
source, writing direction, metric fixture, container/cursor dimensions,
strictness, cluster advances, UAX opportunities, preferred and acceptable
breaks, expected status, and tags. No absolute path is stored.

Coverage includes prose, dialogue punctuation, loose/strict pair preference,
leaders, hanging, ruby-adjacent body extent, mixed Latin/digits, emoji grapheme
clusters, exact/wide/narrow columns, an unbreakable sequence, styled-run
continuation, and `vertical_rl`/`vertical_lr` parity.

Automated review thresholds are all zero for hard-invariant regressions,
preferred/acceptable break drift, and new forced-overflow cases. The manifest is
marked `owner_approved`: the repository owner's 2026-07-14 instruction to apply
the delivery records approval of the unchanged 14-case baseline. Regenerating
expected offsets cannot preserve or confer approval for a changed baseline; a
case-by-case owner review is required.

## Structural audit

The canonical audit was run against Jujutsu change
`tuqmpsrwrnnpwxponvnuryorrnmszwwt`:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/aw-ah-015-vertical-break-quality
```

It scanned 2,723 repository files, including 1,298 Rust files and 634,630
physical Rust LOC, and reported zero errors and 127 existing workspace
warnings. Exact machine-readable results are checked in under
[`structure-audits/aw-ah-015-vertical-break-quality`](../structure-audits/aw-ah-015-vertical-break-quality/violations.md).

| Path | Bytes | Physical LOC | Classification and responsibility |
| --- | ---: | ---: | --- |
| `src/lib.rs` | 1,687 | 43 | facade; deliberate public layout and vertical-break surface |
| `src/config.rs` | 5,405 | 143 | production; request policy and structured error boundary |
| `src/document_hash.rs` | 8,108 | 234 | production; deterministic layout/cache identity |
| `src/document_layout.rs` | 34,184 | 968 | production; shared shaping and layout orchestration |
| `src/vertical_break.rs` | 781 | 20 | responsibility facade |
| `src/vertical_break/model.rs` | 9,256 | 267 | production; closed policy, checked errors, result and explain models |
| `src/vertical_break/planner.rs` | 25,886 | 801 | production; cohesive bounded fixed-point dynamic-programming planner |
| `src/vertical_break/tests.rs` | 10,403 | 374 | focused unit-test module |
| `tests/vertical_break_quality.rs` | 8,531 | 263 | integration test; approved corpus, codec, and scale invariance |

No production file crosses the 1,200-LOC warning threshold. The dedicated
planner is one line above the ordinary 300–800 preferred range but remains one
cohesive algorithm with normalization, candidate evaluation, reconstruction,
and checked arithmetic; splitting those internal phases into public or
cross-module contracts would weaken rather than clarify ownership.
`arcweft-text-layout` has seven dependency edges (six normal and the new
test-only `serde_json` edge) and five workspace consumers. No normal dependency
direction changed.

## Compatibility-free migration order

1. Review and approve this specification, manifest, and baseline report.
2. Add the closed policy and checked planner modules.
3. Add the request field, typed error propagation, and hash identity.
4. Replace the previous local floating objective and remove
   `document_vertical.rs`; do not preserve legacy weights.
5. Run focused policy/corpus tests, text-layout tests, workspace check/clippy,
   common Native/Web/headless parity tests, and structural audit.
6. Review every corpus delta before changing `balanced_v1` or adding a new
   versioned preset.

## Non-goals retained

No JLREQ data, shaping, ruby semantics, horizontal breaking, glyph rasterization,
or backend composition is redesigned. There are no font-specific exceptions,
backend-local weights, arbitrary authored coefficients, or compatibility aliases.
