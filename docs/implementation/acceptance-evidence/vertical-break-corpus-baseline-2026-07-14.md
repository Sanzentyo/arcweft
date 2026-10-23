# AW-AH-015 corpus baseline

Policy: `balanced_v1`
Corpus schema: `arcweft.vertical-break-quality-corpus.v1`
Corpus cases: 14
License: `CC0-1.0`
Review state: repository-owner approved on 2026-07-14 for the unchanged 14-case baseline

## Baseline summary

| Category | Cases | Baseline result |
| --- | ---: | --- |
| Normal plans | 12 | Preferred and acceptable offsets coincide |
| Expected forced overflow | 2 | Narrow cells and one unbreakable sequence only |
| Direction-parity groups | 1 pair | `vertical_rl` and `vertical_lr` both `[4, 8]` |
| Strictness differentiation | 2 | loose `[3, 6]`; strict `[1, 5]` |
| Scale samples per case | 4 | `0.5x`, `1x`, `2x`, `4x` use identical offsets |

## Reviewed offsets

| Case | Preferred offsets | Status | Primary review purpose |
| --- | --- | --- | --- |
| `jp-prose-balanced-rl` | `[4, 8]` | normal | prose, comma compression |
| `jp-prose-balanced-lr` | `[4, 8]` | normal | direction parity |
| `closing-opening-loose` | `[3, 6]` | normal | loose pair preference |
| `closing-opening-strict` | `[1, 5]` | normal | strict pair preference |
| `leader-pair-normal` | `[1, 4]` | normal | leader keep-together |
| `legal-hanging-beats-early-break` | `[3]` | normal | bounded hanging versus early break |
| `ruby-adjacent-body-plan` | `[3]` | normal | ruby-adjacent body extent |
| `mixed-latin-digits` | `[2, 4]` | normal | sideways Latin and text-combine advance |
| `emoji-grapheme-clusters` | `[3, 6]` | normal | emoji/ZWJ cluster atomicity |
| `exact-fit-column` | `[]` | normal | exact fit |
| `wide-single-column` | `[]` | normal | wide container |
| `narrow-forced-overflow` | `[1, 2]` | forced overflow | unavoidable per-cluster overflow |
| `pathological-unbreakable` | `[]` | forced overflow | shortest legal terminal escape |
| `styled-run-partial-restart` | `[0]` | normal | continuation restart contract |

## Change gate

A proposed policy revision fails automated review when it:

- enables any hard-prohibited boundary;
- selects an offset outside a case's reviewed acceptable set;
- changes a preferred offset without an explicit owner-reviewed delta;
- introduces forced overflow in a previously normal case;
- removes the expected forced-overflow status without explaining the changed
  metric or break opportunity;
- loses uniform-scale or direction parity.

Automated success is necessary but not sufficient. The owner must review the
case-by-case delta and update the approval status explicitly; regenerating the
manifest or goldens is not approval.
