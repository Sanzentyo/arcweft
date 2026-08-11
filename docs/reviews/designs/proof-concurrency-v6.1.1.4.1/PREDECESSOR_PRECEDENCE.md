# Predecessor precedence and expression disposition

## Authority order

| Priority | Source | SHA-256 / repository identity | Binding decision |
|---:|---|---|---|
| 1 | This v6.1.1.4.1 correction | this archive | closes missing leaf/payload decisions only |
| 2 | AW-AH-009.4.2 | `05E825DDE033F308F24FC1F6E504B4C26BBA2D61FD33852CE880DC666BA8F2A8` | ID, dialogue outer records, source/candidate roles and ordinals |
| 3 | AW-AH-009.3.3.4 | `DD8096DEDEF9FE2446291B3849DCEABD8BB5192B88533AA12FEE2DFC3CCEC484` | associated type receiver, generic arity, value/nominal precedence |
| 4 | AW-AH-009.3.3.3.1 | `060332BC62273C34F267F0F15767FE6BBD328BE177CB8035E83F210267AB0D41` | overload/candidate accounting |
| 5 | AW-AH-009.3.3 | `9D1F989F5E0E698AEFF1098DD7ECEE7E01A66616A00A0571EE333A3B1B7DDC78` | one callable catalog/resolver |
| 6 | AW-AH-009.3.1 | `6EDE771A895AF981A583FDFD50A080F2ECA57BF7A2925216CF725F7DBB418588` | ordinary call surface |
| 7 | AW-AH-009.4 | `A86044FEA7AAFF3EC3829DFA0AD6552C88377CA61FA2911C3B96EA34CA0FFA5E` | Character/Dialogue runtime direction |
| 8 | Proof v6.1.1 | `1B7DE5F2C10A5B29D67C72011E4272DF9A76AF8907FD21FE162DE54809FC69EF` | qualified arenas, snapshots, transactions, source map, base inventory |
| 9 | GitHub-visible AW-AH-007/008 evidence | `DBF72681E97377FC6A5B592579BF29F1E5640105ACF1D4446D13D0209FCFD209` | RichText validation/limits; old D3.2 placement is not authority |
| 10 | Current typed owner code at `ac9ce44fe9423efd85280e26832dd30c725b3b34` | Git blob identities in `REPOSITORY_EVIDENCE.md` | current language families and direct projections |

The rejected SHA `414F95F8EF4C5F3ABCCE163F0C9B01F124098F0BAC856F174AF09B5C1E7D564B` and the later NOT_READY archive SHA `9ccb9af261a3d55bddefe570b4902d9ba6395725904f88bf389b4565e5bd8374` have no normative schema authority.

## Expression disposition

| Baseline variant | Disposition | Final decision |
|---|---|---|
| `Unit` | retained | exact unit expression; no literal Unit |
| `Literal` | field-corrected | closed typed literal families |
| `EntityReference(HirEntityReference)` | payload-replaced | EntityReference(HirIdRef) |
| `LifetimePath` | field-corrected | runtime registry path only |
| `Path` | field-corrected | root-preserving typed path |
| `ShortVariant` | field-corrected | HirName payload |
| `Placeholder` | field-corrected | PartialApplication | PipeLeft |
| `Tuple` | retained | ordered same-arena ExprIds |
| `BracketSequence` | retained | ordered same-arena ExprIds |
| `NumericBracketSequence` | field-corrected | one ExprId, ID-less arbitrary-precision elements |
| `ArrayRepeat` | retained | value and length ExprIds |
| `Call(inline fields)` | payload-replaced | Call(HirCallExpr) |
| `Select` | retained | target ExprId + HirName |
| `DialogueCall` | deleted | ordinary expression variant cannot be constructed |
| `Index` | retained | target/index ExprIds |
| `Pipe` | retained | left/right ExprIds |
| `Try` | retained | operand + exact authored form |
| `Await` | field-corrected | operand + preserve/propagate result |
| `Thread` | field-corrected | name/mode/scope/ordered flow body |
| `Range` | retained | optional endpoints + inclusive |
| `Record` | field-corrected | typed path + closed fields |
| `RecordLiteral` | field-corrected | closed fields |
| `Binary` | retained | HIR-owned op including implication |
| `Borrow` | retained | HIR-owned borrow kind |
| `Dereference` | retained | operand |
| `Closure` | field-corrected | scope/params/result/body/captures |
| `Unary` | retained | HIR-owned op + operand |
| `Block` | field-corrected | scope/statements/explicit or synthetic tail |
| `ComputationBlock` | field-corrected | kind/scope/statements/tail |
| `NamedBlock` | field-corrected | name/scope/statements/tail |
| `If` | field-corrected | condition/then/else; omitted else synthetic Unit |
| `IfLet` | field-corrected | pattern/scrutinee/guard/scoped branches |
| `Match` | field-corrected | scrutinee + closed arms |
| `MemoBlock` | deleted | removed unreleased syntax; no carrier/diagnostic |
| `Error` | retained | generic unclassified recovery only |
| `DialogueContentApplication` | added | accepted AW-AH-009.4.2 outer payload |
| `PostfixBracket` | added | accepted bounded candidate payload |

The baseline table has 37 rows because it includes the two deleted variants and the two later additions; the resulting enum has exactly 35 variants.

## Fixed adjacent decisions

The dedicated declaration-member arena, `HirStmtKind::IfLet(HirIfLetStmt)`, and source component `UnsafeAuditInsertion` remain fixed. The expression contract does not compress statement IfLet into an ExprId and does not restore a raw unsafe-audit range.
