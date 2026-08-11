# Recovery, ambiguity, limits, and failure atomicity

## 1. Recovery table

| Source condition | Retained typed result | Diagnostic owner and site | Executable result |
|---|---|---|---|
| nested target call missing `)` | existing recovered `CallExpr` remains complete target; outer postfix still parsed | existing `syntax.expression.missing_call_close` at the byte where `[` begins or the ordinary call boundary | no |
| missing `]` | selected/ambiguous/invalid postfix node with `RecoveredMissing` terminator | generic `syntax.expression.missing_postfix_bracket_close` at the exact selected boundary | no |
| empty `[]` | index failure `EmptyPayload`; recovered dialogue application with missing content when it is the only viable interpretation | existing generic expected-content recovery at `open_bracket.end()`; no invented text | no |
| colon head without inline or indented content | colon dialogue application with `DialogueIndentation::Missing` and missing content site | existing generic expected-content/indentation recovery at exact insertion | no |
| malformed/misaligned indented content | content/body retained; typed misalignment issue; recovered application | ordinary dialogue/indentation diagnostic at the exact prefix/content site | no |
| malformed `with:` or `with {}` after valid attachment prefix | existing recovered `LinePlan` stays attached | existing line-plan grammar diagnostics | no |
| `with` after blank/comment line, wrong prefix, or following statement | no attachment; token remains for following statement/error owner | following owner uses existing diagnostics | application unaffected |
| `foo()[content]` | complete existing call target plus generic postfix application | ordinary call and postfix owners | according to selected clean meaning |
| one ordinary expression payload | clean index candidate; dialogue candidate only if the existing dialogue grammar independently accepts it | no speculative diagnostic | sema if ambiguous, otherwise yes |
| controls/RichText/interpolation/line breaks | dialogue candidate | existing dialogue grammar | yes when clean and target checks |
| two viable interpretations | one `PostfixBracket::Ambiguous` with two typed candidates | no syntax diagnostic when both clean; sema owns unresolved ambiguity | only after typed sema resolution |
| no viable interpretation | one `PostfixBracket::Invalid` with two failure summaries | at most one final summary per interpretation | no |
| bare block without `with` | never a plan | block/following statement owner | application unaffected |

## 2. Missing-close boundary selection

The generic postfix parser searches only its token cursor and existing owner
boundaries. At bracket nesting depth zero, the first of these ends a missing
close payload:

1. an owner-provided end;
2. a physical line ending when the expression owner cannot continue across it;
3. comma, semicolon, close parenthesis, close bracket, close brace, or fat
   arrow owned by the parent;
4. an eligible top-level `with` plan prefix;
5. end of expression/document.

Nested delimiters, interpolation bodies, RichText/control delimiters, strings,
and comments do not end the payload. The selected boundary is stored in
`PostfixBracketRecoveryBoundarySyntax`; no source search is repeated later.

## 3. Ambiguity diagnostics

Candidate parsing stages diagnostics transactionally. Publication rules are:

- a selected viable candidate publishes only its own recovery diagnostics;
- an ambiguous node publishes recovery diagnostics for each recovered retained
  candidate, ordered index then dialogue, and none for clean candidates;
- an invalid node publishes at most one typed failure summary for index then
  one for dialogue, after shared lexical/delimiter diagnostics;
- a failed candidate that loses to a clean selected candidate emits no
  speculative diagnostic;
- sema ambiguity/no-match diagnostics use typed candidates and source roles,
  never callee spelling.

## 4. Existing syntax budgets charged

No new configurable dialogue limit exists. Work charges the current owners:

```text
MAX_CALL_ARGUMENTS = 128
MAX_CALLBACK_PARAMETERS = 128
MAX_NESTED_CALLS = 32
MAX_EXPR_RECOVERY_NODES = 256
MAX_EXPR_DIAGNOSTICS = 128
```

The existing grammar document transaction also charges every token read, CST
node/event, expression node, missing token, and syntax diagnostic through its
current parser budgets. Dialogue content and line plan continue to charge their
existing grammar work. Indentation issues charge the existing expression/
diagnostic cap and cannot grow after it is exhausted.

Exact postfix work is bounded:

```text
CST emission passes per postfix payload: 1
candidate attempts: 2
candidate payload passes: at most 2
retained candidate results: at most 2
retained final failure summaries: at most 2
final failure diagnostics: at most 2
```

Each payload token is visited at most once by CST emission and once by each
candidate attempt, apart from already-budgeted nested parser operations.

## 5. Existing HIR budgets charged

All accepted proof-concurrency v6.1.1 `HirLimits` defaults remain unchanged.
The implementation charges:

- one expression slot for the source-backed application root;
- normal source-backed slots for the target/nested ordinary syntax;
- ambiguous candidate-only expression/statement/pattern slots against their
  existing arena limits;
- one scope slot for an attached plan root and normal nested scopes;
- one source-component entry for each published non-`Whole` role;
- one normal HIR diagnostic for each committed recovery/semantic lowering
  diagnostic;
- normal work/cancellation checks for every candidate child and source site.

No new client setting, environment variable, feature switch, or dialogue-only
capacity is introduced.

## 6. Limit exhaustion policy

Parser/CST/AST and HIR limit exhaustion is fatal to the owning transaction. It
is not represented as a recovered dialogue candidate or a truncation marker.
The transaction returns the existing typed limit failure and publishes no
partial node, diagnostic suffix, ID allocation, source-map component, scope, or
module snapshot.

The exact-limit case succeeds; the first one-over case fails atomically.

## 7. Checked arithmetic

Every source offset/range operation uses checked add/subtract/conversion.
Required checks include:

- UTF-8 boundary and `offset <= document.len()`;
- `start <= end` and range containment;
- delimiter byte lengths;
- head/base/body/dedent ordering;
- CRLF two-byte containment;
- argument ordinal conversion to `u16`;
- candidate preorder ordinal conversion to `u32`;
- component source-site range projection;
- source revision/document agreement.

Saturating, wrapping, clamping, and fallback zero ranges are prohibited in the
new path.

## 8. Internal failures versus user diagnostics

Internal parser/HIR failures:

- checked arithmetic overflow/underflow;
- non-UTF-8 offset;
- source range outside the bound document;
- impossible child ordering/overlap;
- invalid delimiter spelling/length for a typed surface;
- prefix width not matching bytes;
- source node identity owned by another snapshot;
- candidate root kind/target mismatch;
- source component using another document/revision;
- arena/source-map/transaction invariant failure;
- limit exhaustion or cancellation.

User diagnostics:

- ordinary missing delimiters/expressions;
- malformed dialogue content/control/RichText/interpolation;
- missing colon/bracket content;
- indentation misalignment;
- malformed attached line plan;
- semantic ambiguity/no viable typed interpretation;
- duplicate/malformed coordinate values and later type errors.

Internal failures never become recoverable source diagnostics. User errors
never bypass the transaction or create executable poisoned HIR.

## 9. Atomicity

The syntax document builder stages CST events, attachments, diagnostics, and
node identities until validation succeeds. HIR lowering stages slots, values,
scopes, synthetic ordinals, source components, diagnostics, and retirement
changes in the accepted transaction. Commit publishes all of them together;
rollback publishes none.
