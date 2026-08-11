# Try followed by Select: identity and source contract

## Required parser sequence

The final lossless expression event stream removes `?.` from the combined
postfix-Select token path. For expression context, the lexer/event parser emits:

```text
Identifier(target)  Question  Dot  Identifier(member-or-missing)
```

The central projection sequence is:

```text
Path(target)
-> ExpressionProjection::Try { form: PostfixQuestion }
-> ExpressionProjection::Select(SyntaxSelectedMember::{Name|Missing})
```

The Try and Select each retain their own attached `SyntaxNodeId` and final
qualified `ExprId`. The outer `HirSelectExpr.target` is the Try `ExprId`. The
Select payload does not contain an optional flag or delimiter kind.

## Exact geometry

### `target?.member`

```text
target           0..6
?                6..7
.                7..8
member           8..14

Try Whole        0..7
Try Operand      0..6
Try Operator     6..7
Select Whole     0..14
Select Target    0..7
SelectedMember   8..14 (Span)
```

### `target?.`

```text
target           0..6
?                6..7
.                7..8
MissingName      8..8

Try Whole        0..7
Try Operand      0..6
Try Operator     6..7
Select Whole     0..8
Select Target    0..7
SelectedMember   8..8 (Insertion)
```

The outer Target site includes the full authored Try span. The Try separately
owns Operand and Operator components through its retained accepted source
roles. Source queries never infer the `?` or `.` by scanning text.

## Allocation and accounting

Both forms allocate exactly three HIR expression slots: Path, Try, Select.
They stage three slot-Whole metadata rows. E13 stages two Select components;
Try stages its retained two components. The full component total is the
retained one-segment Path child manifest plus four. `Name` charges its authored
member bytes; `Missing` charges zero. Neither form allocates a synthetic child.

The clean form contributes zero diagnostics. The missing form contributes one
Select-root HIR diagnostic at the outer SelectedMember insertion. The Try is
clean in both rows and no combined `OptionalDot` diagnostic exists.

## Deletion

The coherent parser/attachment switch deletes:

- `?.` from the combined Select-token recognition;
- any `OptionalDot` enum/flag/branch;
- any lowering that maps `?.` directly to a two-slot Select; and
- every test/source/cache expectation that omits the Try identity.

The accepted postfix Try implementation and all non-E13 Try rows remain
unchanged.
