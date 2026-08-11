# Mandatory direct test matrix

Normative rows: **184**. Every row requires direct behavioral or typed-API evidence; a source-text/file-placement scan is not acceptable.

| ID | Test owner | Fixture / action | Expected typed result | Diagnostic / transaction evidence |
|---|---|---|---|---|
| COMMON-001 | syntax retained_document/public attachment/HIR/project tests | mixed document with seven retained declarations, `res`, function, proof, and Style | source-order exact item inventory; no Asset item and no top-level flow fallback | no diagnostics |
| COMMON-002 | syntax retained_document/public attachment/HIR/project tests | same mixed document with LF | green text byte-equal and exact attached ranges | no diagnostics |
| COMMON-003 | syntax retained_document/public attachment/HIR/project tests | same mixed document with CRLF | green text byte-equal; ranges count CR bytes | no diagnostics |
| COMMON-004 | syntax retained_document/public attachment/HIR/project tests | Unicode in docs, comments, strings, and View expressions | UTF-8 lossless nodes and UTF-16 LSP projection from source spans | no source rescanning |
| COMMON-005 | syntax retained_document/public attachment/HIR/project tests | contiguous docs and attributes before Character | prefix nodes attach to Character item | no diagnostics |
| COMMON-006 | syntax retained_document/public attachment/HIR/project tests | blank logical line between docs and Character | docs do not attach; Character begins at keyword | ordinary orphan-prefix recovery |
| COMMON-007 | syntax retained_document/public attachment/HIR/project tests | ordinary comment between attribute and Character | attribute run ends and does not attach across comment | ordinary current-grammar recovery |
| COMMON-008 | syntax retained_document/public attachment/HIR/project tests | `pub character A {}` | Visibility::Public node | no diagnostics |
| COMMON-009 | syntax retained_document/public attachment/HIR/project tests | `pub(crate) view V() { Panel {} }` | Visibility::Crate node | no diagnostics |
| COMMON-010 | syntax retained_document/public attachment/HIR/project tests | `pub(super) action A()` | Visibility::Super node | no diagnostics |
| COMMON-011 | syntax retained_document/public attachment/HIR/project tests | `pub(in game) signal S: Watch<bool>` | typed visibility recovery; declaration poisoned | common invalid visibility on exact qualifier |
| COMMON-012 | syntax retained_document/public attachment/HIR/project tests | omitted explicit IDs on all seven declarations | family-derived PublicIds from DeclarationName | no diagnostics |
| COMMON-013 | syntax retained_document/public attachment/HIR/project tests | family-correct absolute IDs on all seven | DeclarationPublicId nodes and exact PublicId values | no diagnostics |
| COMMON-014 | syntax retained_document/public attachment/HIR/project tests | wrong-family absolute declaration ID | WrongFamilyReference under header; no symbol | syntax.declaration.wrong_family_id + keyword related range |
| COMMON-015 | syntax retained_document/public attachment/HIR/project tests | relative/family-relative declaration ID | DeclarationPublicId retained; no symbol | syntax.declaration.relative_id |
| COMMON-016 | syntax retained_document/public attachment/HIR/project tests | malformed declaration `@` spelling | MissingDeclarationId/ErrorNode at exact bytes | syntax.declaration.malformed_id |
| COMMON-017 | syntax retained_document/public attachment/HIR/project tests | missing declaration name | zero-width MissingName | syntax.declaration.missing_name |
| COMMON-018 | syntax retained_document/public attachment/HIR/project tests | keyword used as declaration name | MissingName plus ErrorNode; no symbol | syntax.declaration.invalid_name |
| COMMON-019 | syntax retained_document/public attachment/HIR/project tests | dotted declaration name | single invalid-name recovery; no split symbols | syntax.declaration.invalid_name |
| COMMON-020 | syntax retained_document/public attachment/HIR/project tests | duplicate local name in one module | first symbol retained, duplicate declaration poisoned | project duplicate-name with first related span |
| COMMON-021 | syntax retained_document/public attachment/HIR/project tests | duplicate explicit PublicId across modules | one project identity, duplicate rejected atomically | project duplicate-public-id with both source spans |
| COMMON-022 | syntax retained_document/public attachment/HIR/project tests | same local name across retained family and ordinary callable | one module namespace collision | project duplicate-name |
| COMMON-023 | syntax retained_document/public attachment/HIR/project tests | private retained declaration | stable semantic PublicId but module-private accessibility | no diagnostics in declaring module |
| COMMON-024 | syntax retained_document/public attachment/HIR/project tests | cross-module inaccessible private reference | typed resolver cause Inaccessible | one accessibility diagnostic |
| COMMON-025 | syntax retained_document/public attachment/HIR/project tests | legal `pub use` re-export | same project symbol/PublicId, new local binding only | no diagnostics |
| COMMON-026 | syntax retained_document/public attachment/HIR/project tests | family-relative body/member reference | EntityRefSyntax::FamilyRelative and expected family | resolved through one project table |
| COMMON-027 | syntax retained_document/public attachment/HIR/project tests | resolved reference has wrong family | typed reference remains, declaration poisoned | wrong-family resolver diagnostic on exact node |
| COMMON-028 | syntax retained_document/public attachment/HIR/project tests | regular-project top-level expression | one ErrorItem and no executable HIR | ordinary item recovery |
| COMMON-029 | syntax retained_document/public attachment/HIR/project tests | removed `content` followed by valid View | ErrorItem then View sibling | ordinary item recovery only |
| COMMON-030 | syntax retained_document/public attachment/HIR/project tests | 16,384 top-level items | transaction succeeds at exact inclusive limit | no fatal error |
| COMMON-031 | syntax retained_document/public attachment/HIR/project tests | 16,385th top-level item | no tree/generation/diagnostic commit | fatal SyntaxLimit::TopLevelItems |
| COMMON-032 | syntax retained_document/public attachment/HIR/project tests | 1,024 recoverable diagnostics | transaction succeeds at exact diagnostic limit | 1,024 diagnostics |
| COMMON-033 | syntax retained_document/public attachment/HIR/project tests | 1,025th diagnostic | full transaction rollback | fatal SyntaxLimit::Diagnostics |
| COMMON-034 | syntax retained_document/public attachment/HIR/project tests | 1,048,576 identity-bearing nodes | transaction succeeds at exact identity limit | no fatal error |
| COMMON-035 | syntax retained_document/public attachment/HIR/project tests | attached declaration to Rowan and back | same database/lineage/snapshot/node and range | no lookup error |
| COMMON-036 | syntax retained_document/public attachment/HIR/project tests | handle resolved in another syntax database | no node returned | typed wrong-database/lineage error |
| COMMON-037 | syntax retained_document/public attachment/HIR/project tests | lower valid retained item and all typed children | source-backed HIR IDs allocated from exact syntax IDs | no clone or reparse evidence |
| COMMON-038 | syntax retained_document/public attachment/HIR/project tests | register retained HIR project | one ProjectSymbolTable owns symbols and facets | no duplicate registry |
| COMMON-039 | syntax retained_document/public attachment/HIR/project tests | lower poisoned retained item | HirItemKind::Error only; no project symbol | upstream poison retained |
| COMMON-040 | syntax retained_document/public attachment/HIR/project tests | successful parse after fatal attempted parse | same generation and IDs as control run | rollback determinism evidence |
| ASSET-001 | arcweft-id/project catalog/CLI bundle tests | top-level `asset bg {}` | ordinary ErrorItem; no retained declaration kind | no removed-specific diagnostic |
| ASSET-002 | arcweft-id/project catalog/CLI bundle tests | public syntax API type check | no Item::Asset or AssetDeclaration type | compile-fail for attempted construction |
| ASSET-003 | arcweft-id/project catalog/CLI bundle tests | HIR API type check | no HirItemKind::Asset and no asset ItemId | compile-fail/direct exhaustive match |
| ASSET-004 | arcweft-id/project catalog/CLI bundle tests | virtual path `bg/Room.png` | AssetId `asset.bg.room` | no diagnostics |
| ASSET-005 | arcweft-id/project catalog/CLI bundle tests | virtual path `BG/Room.PNG` | lowercase AssetId normalization | no diagnostics |
| ASSET-006 | arcweft-id/project catalog/CLI bundle tests | virtual path `ui/main-menu.webp` | AssetId `asset.ui.main_menu` | no diagnostics |
| ASSET-007 | arcweft-id/project catalog/CLI bundle tests | virtual path `voice/alice/greeting.ogg` | final extension stripped only | AssetId `asset.voice.alice.greeting` |
| ASSET-008 | arcweft-id/project catalog/CLI bundle tests | virtual path containing space/non-ASCII punctuation | catalog entry rejected | typed invalid asset ID component |
| ASSET-009 | arcweft-id/project catalog/CLI bundle tests | empty/effectively extension-only path | no AssetId allocated | typed invalid asset virtual path |
| ASSET-010 | arcweft-id/project catalog/CLI bundle tests | two paths same stem with different extensions | catalog transaction rejects collision and keeps neither new duplicate | both paths reported |
| ASSET-011 | arcweft-id/project catalog/CLI bundle tests | typed `res` field references `@asset:.bg.room` | resolved ProjectAssetSymbol; no asset HIR item | no diagnostics when included |
| ASSET-012 | arcweft-id/project catalog/CLI bundle tests | reference to absent/excluded asset | unresolved catalog symbol | structured missing asset diagnostic |
| ASSET-013 | arcweft-id/project catalog/CLI bundle tests | bundle admission for asset | bytes/digest/media/inclusion owned by catalog/bundle | no source metadata fabricated |
| ASSET-014 | arcweft-id/project catalog/CLI bundle tests | asset file changed/removed between project generations | new catalog generation; old catalog handle stale/absent | deterministic liveness result |
| CHAR-001 | character_grammar/public AST/HIR/Character registry tests | canonical docs/attribute/pub/id/name/alias/display body | all Character typed nodes and one literal expression | no diagnostics |
| CHAR-002 | character_grammar/public AST/HIR/Character registry tests | `character Alice {}` | empty typed CharacterBody and derived `character.Alice` | no diagnostics |
| CHAR-003 | character_grammar/public AST/HIR/Character registry tests | alias omitted | surface_alias None | no diagnostics |
| CHAR-004 | character_grammar/public AST/HIR/Character registry tests | `as` without identifier | MissingName alias child | syntax.character.missing_alias zero-width |
| CHAR-005 | character_grammar/public AST/HIR/Character registry tests | duplicate display_name | both CharacterDisplayNameMember nodes | duplicate primary + first related |
| CHAR-006 | character_grammar/public AST/HIR/Character registry tests | unknown `voice` member | ErrorDeclarationMember | syntax.character.unknown_member |
| CHAR-007 | character_grammar/public AST/HIR/Character registry tests | display_name missing `=` | typed member and missing assignment | exact syntax character assignment diagnostic |
| CHAR-008 | character_grammar/public AST/HIR/Character registry tests | display_name missing value | MissingMemberValue/Expression | exact zero-width diagnostic |
| CHAR-009 | character_grammar/public AST/HIR/Character registry tests | display_name non-constant | ExprId retained; no product symbol projection | semantic constant-string diagnostic |
| CHAR-010 | character_grammar/public AST/HIR/Character registry tests | unclosed body before View | Character missing close then View sibling | syntax.declaration.missing_close |
| CHAR-011 | character_grammar/public AST/HIR/Character registry tests | Character PublicId duplicate | duplicate symbol rejected | both declaration spans |
| CHAR-012 | character_grammar/public AST/HIR/Character registry tests | Character alias duplicate | one Character registry alias owner | accepted alias duplicate diagnostic |
| CHAR-013 | character_grammar/public AST/HIR/Character registry tests | omitted display label with alias | product fallback label is alias without synthetic ExprId | no diagnostics |
| CHAR-014 | character_grammar/public AST/HIR/Character registry tests | omitted display label and alias | product fallback label is DeclarationName | no diagnostics |
| CHAR-015 | character_grammar/public AST/HIR/Character registry tests | ordinary CharacterDialogue call | resolves same Character project symbol | no Character-specific call parser |
| CHAR-016 | character_grammar/public AST/HIR/Character registry tests | HIR lowering | HirCharacterDeclaration with optional ExprId and exact source slots | no cloned EntityDeclItem |
| VIEW-001 | view_grammar/public AST/HIR/compiler View tests | canonical fixed signature/export/fragment | ViewDeclaration, Parameter, export, ViewFragment, typed expressions | no diagnostics |
| VIEW-002 | view_grammar/public AST/HIR/compiler View tests | zero-parameter `view V() {}` | required empty FixedParameterGroup and empty fragment | no diagnostics |
| VIEW-003 | view_grammar/public AST/HIR/compiler View tests | parameter with default | PatternId/TypeId/ExprId | no diagnostics when type-compatible |
| VIEW-004 | view_grammar/public AST/HIR/compiler View tests | destructuring parameter | typed TuplePattern retained; declaration poisoned | syntax.view.invalid_parameter |
| VIEW-005 | view_grammar/public AST/HIR/compiler View tests | rest parameter | typed RestPattern retained | syntax.view.invalid_parameter exact pattern range |
| VIEW-006 | view_grammar/public AST/HIR/compiler View tests | keyword parameter name | typed invalid binding recovery | syntax.view.invalid_parameter |
| VIEW-007 | view_grammar/public AST/HIR/compiler View tests | missing fixed parameter group | typed missing group and body retained | syntax.view.missing_parameters |
| VIEW-008 | view_grammar/public AST/HIR/compiler View tests | missing parameter colon/type | MissingType under Parameter | syntax.parameter.missing_type |
| VIEW-009 | view_grammar/public AST/HIR/compiler View tests | authored return arrow | error child before body | syntax.view.return_not_allowed |
| VIEW-010 | view_grammar/public AST/HIR/compiler View tests | authored where clause | typed header recovery and body retained | syntax.declaration.unexpected_header |
| VIEW-011 | view_grammar/public AST/HIR/compiler View tests | leading export | ViewExportDeclaration with typed local/public paths | no diagnostics |
| VIEW-012 | view_grammar/public AST/HIR/compiler View tests | malformed export missing `part` | typed export recovery | syntax.view.export_missing_part |
| VIEW-013 | view_grammar/public AST/HIR/compiler View tests | malformed export missing `as` | typed export recovery | syntax.view.export_missing_as |
| VIEW-014 | view_grammar/public AST/HIR/compiler View tests | malformed export missing public path | MissingName/path child | syntax.view.export_missing_public |
| VIEW-015 | view_grammar/public AST/HIR/compiler View tests | export after first View value | typed export retained | syntax.view.misplaced_export |
| VIEW-016 | view_grammar/public AST/HIR/compiler View tests | 256 exports | exact-limit success | no fatal error |
| VIEW-017 | view_grammar/public AST/HIR/compiler View tests | 257th export | full syntax transaction rollback | fatal SyntaxLimit::ViewExports |
| VIEW-018 | view_grammar/public AST/HIR/compiler View tests | invalid View value line | ErrorExpression in ViewFragment | syntax.view.invalid_value |
| VIEW-019 | view_grammar/public AST/HIR/compiler View tests | unclosed nested View expression before Signal | nested + View missing close, Signal sibling | exact delimiter diagnostics |
| VIEW-020 | view_grammar/public AST/HIR/compiler View tests | View callable lookup | same View ItemId and project symbol callable facet | no second callable item |
| VIEW-021 | view_grammar/public AST/HIR/compiler View tests | View catalog admission | ViewId(PublicId) distinct from dense registry ID | deterministic catalog mapping |
| VIEW-022 | view_grammar/public AST/HIR/compiler View tests | HIR lowering | parameters/exports/values IDs with exact source slots | no generic function-body copy |
| ACTION-001 | action_grammar/public AST/HIR/channel tests | canonical typed Action signature | ActionDeclaration + ActionSignature + ordered parameters | no diagnostics |
| ACTION-002 | action_grammar/public AST/HIR/channel tests | zero-parameter Action | empty fixed group; Unit payload | no diagnostics |
| ACTION-003 | action_grammar/public AST/HIR/channel tests | trailing semicolon | same Action AST/HIR payload | no diagnostics |
| ACTION-004 | action_grammar/public AST/HIR/channel tests | missing parameter group | typed missing group | syntax.action.missing_parameters |
| ACTION-005 | action_grammar/public AST/HIR/channel tests | destructuring parameter | typed pattern retained; poisoned | syntax.action.invalid_parameter |
| ACTION-006 | action_grammar/public AST/HIR/channel tests | missing parameter type | MissingType | syntax.parameter.missing_type |
| ACTION-007 | action_grammar/public AST/HIR/channel tests | parameter default | default Expr retained under recovery | syntax.action.default_not_allowed |
| ACTION-008 | action_grammar/public AST/HIR/channel tests | return arrow | trailing error node | syntax.action.return_not_allowed |
| ACTION-009 | action_grammar/public AST/HIR/channel tests | braced body | body retained only as ErrorNode | syntax.action.body_not_allowed |
| ACTION-010 | action_grammar/public AST/HIR/channel tests | generic/where/effect tail | typed/current-header recovery | syntax.declaration.trailing_syntax/unexpected_header |
| ACTION-011 | action_grammar/public AST/HIR/channel tests | 256 parameters | exact fixed-parameter limit succeeds | no fatal error |
| ACTION-012 | action_grammar/public AST/HIR/channel tests | 257th parameter | full transaction rollback | fatal SyntaxLimit::FixedParameters |
| ACTION-013 | action_grammar/public AST/HIR/channel tests | duplicate parameter name | typed parameters retained; no callable facet | semantic duplicate binding |
| ACTION-014 | action_grammar/public AST/HIR/channel tests | duplicate Action ID/name | no overload set | project duplicate diagnostic |
| ACTION-015 | action_grammar/public AST/HIR/channel tests | send/receive schema lookup | same Action ItemId ordered payload | no signature text parsing |
| ACTION-016 | action_grammar/public AST/HIR/channel tests | HIR lowering | HirActionDeclaration + ParameterIds/LocalIds | no body or result payload |
| ACTIVITY-001 | activity_grammar/public AST/HIR/manifest admission tests | canonical full abstract Activity | all five typed section families and typed descendants | no diagnostics |
| ACTIVITY-002 | activity_grammar/public AST/HIR/manifest admission tests | empty Activity body | defaults deterministic/stateless and empty interface | no diagnostics |
| ACTIVITY-003 | activity_grammar/public AST/HIR/manifest admission tests | mode deterministic | ActivityMode::Deterministic | no diagnostics |
| ACTIVITY-004 | activity_grammar/public AST/HIR/manifest admission tests | all closed mode values | owned enum variants | no diagnostics |
| ACTIVITY-005 | activity_grammar/public AST/HIR/manifest admission tests | unknown mode | typed policy recovery; no interface symbol | syntax.activity unknown-mode diagnostic |
| ACTIVITY-006 | activity_grammar/public AST/HIR/manifest admission tests | all lifecycle values | owned enum variants | no diagnostics |
| ACTIVITY-007 | activity_grammar/public AST/HIR/manifest admission tests | unknown lifecycle | typed policy recovery | syntax.activity unknown-lifecycle diagnostic |
| ACTIVITY-008 | activity_grammar/public AST/HIR/manifest admission tests | duplicate section | both typed sections | duplicate primary + first related |
| ACTIVITY-009 | activity_grammar/public AST/HIR/manifest admission tests | out-of-order section | typed sections retained | syntax.activity.section_order |
| ACTIVITY-010 | activity_grammar/public AST/HIR/manifest admission tests | input/output ports | ordered ActivityPort nodes and TypeIds | no diagnostics |
| ACTIVITY-011 | activity_grammar/public AST/HIR/manifest admission tests | duplicate port across input/output | both ports retained | syntax.activity.duplicate_port + first related |
| ACTIVITY-012 | activity_grammar/public AST/HIR/manifest admission tests | port missing name | MissingName in ActivityPort | syntax.activity.missing_port_name |
| ACTIVITY-013 | activity_grammar/public AST/HIR/manifest admission tests | port missing colon/type | MissingType | syntax.activity.missing_port_type |
| ACTIVITY-014 | activity_grammar/public AST/HIR/manifest admission tests | port initializer | error child | syntax.activity.port_initializer_not_allowed |
| ACTIVITY-015 | activity_grammar/public AST/HIR/manifest admission tests | 256 total ports | exact ActivityPorts limit succeeds | no fatal error |
| ACTIVITY-016 | activity_grammar/public AST/HIR/manifest admission tests | 257th total port | full transaction rollback | fatal SyntaxLimit::ActivityPorts |
| ACTIVITY-017 | activity_grammar/public AST/HIR/manifest admission tests | requires then ensures | typed clause ExprIds in order | no diagnostics |
| ACTIVITY-018 | activity_grammar/public AST/HIR/manifest admission tests | requires after ensures | typed clauses retained | syntax.activity.contract_order |
| ACTIVITY-019 | activity_grammar/public AST/HIR/manifest admission tests | 65th total contract clause | full transaction rollback | fatal SyntaxLimit::ContractClauses |
| ACTIVITY-020 | activity_grammar/public AST/HIR/manifest admission tests | source `from rust "crate"` tail | no origin child; declaration poisoned | ordinary unexpected/trailing header diagnostic |
| ACTIVITY-021 | activity_grammar/public AST/HIR/manifest admission tests | missing manifest binding at executable compile | abstract HIR remains; no runtime product | compiler missing Activity binding with declaration span |
| ACTIVITY-022 | activity_grammar/public AST/HIR/manifest admission tests | incompatible manifest interface | binding rejected transactionally | structured interface mismatch with both owners |
| SIGNAL-001 | signal_grammar/public AST/HIR/sema tests | canonical Watch signal | SignalObservableType with typed generic Type | no diagnostics |
| SIGNAL-002 | signal_grammar/public AST/HIR/sema tests | canonical Stream signal | two-argument Stream type | no diagnostics |
| SIGNAL-003 | signal_grammar/public AST/HIR/sema tests | canonical Sample signal | one-argument Sample type | no diagnostics |
| SIGNAL-004 | signal_grammar/public AST/HIR/sema tests | unknown Counter head | typed TypeId retained; no symbol publication | semantic invalid observable head |
| SIGNAL-005 | signal_grammar/public AST/HIR/sema tests | Stream wrong arity | typed generic application retained | semantic observable arity diagnostic |
| SIGNAL-006 | signal_grammar/public AST/HIR/sema tests | missing colon | zero-width recovery | syntax.signal.missing_colon |
| SIGNAL-007 | signal_grammar/public AST/HIR/sema tests | missing type | MissingType | syntax.signal.missing_type |
| SIGNAL-008 | signal_grammar/public AST/HIR/sema tests | initializer | typed expression under recovery | syntax.signal.initializer_not_allowed |
| SIGNAL-009 | signal_grammar/public AST/HIR/sema tests | braced/policy tail | ErrorNode; no extra source policy | syntax.declaration.trailing_syntax |
| SIGNAL-010 | signal_grammar/public AST/HIR/sema tests | flow-body signal statement plus top-level Signal | statement node and one declaration node | no classification ambiguity |
| SIGNAL-011 | signal_grammar/public AST/HIR/sema tests | wrong-family/duplicate Signal identity | no symbol published for invalid duplicate | header/project diagnostics |
| SIGNAL-012 | signal_grammar/public AST/HIR/sema tests | HIR lowering | HirSignalDeclaration with one TypeId | no type-string parsing |
| METRIC-001 | metric_grammar/public AST/HIR/sema/runtime schema tests | canonical gauge with unit/labels | closed kind, value type, members, labels | no diagnostics |
| METRIC-002 | metric_grammar/public AST/HIR/sema/runtime schema tests | canonical histogram with buckets | typed bracket expression and Bucket roles | no diagnostics |
| METRIC-003 | metric_grammar/public AST/HIR/sema/runtime schema tests | counter kind | MetricKind::Counter | no diagnostics with numeric type/no buckets |
| METRIC-004 | metric_grammar/public AST/HIR/sema/runtime schema tests | unknown kind | typed MetricKind recovery | syntax.metric.unknown_kind |
| METRIC-005 | metric_grammar/public AST/HIR/sema/runtime schema tests | missing kind | missing kind node | syntax.metric.missing_kind |
| METRIC-006 | metric_grammar/public AST/HIR/sema/runtime schema tests | missing colon/type/body | MissingType and MissingBody | exact zero-width diagnostics |
| METRIC-007 | metric_grammar/public AST/HIR/sema/runtime schema tests | duplicate unit | both MetricUnitMember nodes | duplicate primary + first related |
| METRIC-008 | metric_grammar/public AST/HIR/sema/runtime schema tests | out-of-order members | typed members retained | syntax.metric.member_order |
| METRIC-009 | metric_grammar/public AST/HIR/sema/runtime schema tests | unit is non-string expression | typed expression retained | syntax.metric.unit_not_string |
| METRIC-010 | metric_grammar/public AST/HIR/sema/runtime schema tests | duplicate label | both MetricLabel nodes | syntax.metric.duplicate_label + first related |
| METRIC-011 | metric_grammar/public AST/HIR/sema/runtime schema tests | 64 labels | exact MetricLabels limit succeeds | no fatal error |
| METRIC-012 | metric_grammar/public AST/HIR/sema/runtime schema tests | 65th label | full transaction rollback | fatal SyntaxLimit::MetricLabels |
| METRIC-013 | metric_grammar/public AST/HIR/sema/runtime schema tests | empty buckets | typed empty list | syntax.metric.empty_buckets |
| METRIC-014 | metric_grammar/public AST/HIR/sema/runtime schema tests | buckets non-sequence expression | typed Expr under member | syntax.metric.buckets_not_sequence |
| METRIC-015 | metric_grammar/public AST/HIR/sema/runtime schema tests | 1,024 buckets | exact MetricBuckets limit succeeds | no fatal error |
| METRIC-016 | metric_grammar/public AST/HIR/sema/runtime schema tests | 1,025th bucket | full transaction rollback | fatal SyntaxLimit::MetricBuckets |
| METRIC-017 | metric_grammar/public AST/HIR/sema/runtime schema tests | counter/gauge with buckets | typed HIR retained but no admitted metric symbol | semantic kind/buckets incompatibility |
| METRIC-018 | metric_grammar/public AST/HIR/sema/runtime schema tests | histogram non-increasing/non-finite buckets | constant values retained | semantic bucket order/finite diagnostic |
| METRIC-019 | metric_grammar/public AST/HIR/sema/runtime schema tests | non-numeric value type or invalid label type | typed TypeIds retained | semantic metric type capability diagnostic |
| METRIC-020 | metric_grammar/public AST/HIR/sema/runtime schema tests | HIR lowering | HirMetricDeclaration and DeclarationMemberIds | no kind/type/member string parsing |
| LAYER-001 | layer_grammar/public AST/HIR/project/presentation tests | canonical Layer with all member classes | Layer kind/body/member/policy/ref nodes | no diagnostics |
| LAYER-002 | layer_grammar/public AST/HIR/project/presentation tests | empty body | owned kind defaults applied later; no synthetic expressions | no diagnostics |
| LAYER-003 | layer_grammar/public AST/HIR/project/presentation tests | each closed authored LayerKind | owned LayerKind variant/default phase | no diagnostics |
| LAYER-004 | layer_grammar/public AST/HIR/project/presentation tests | authored `root` kind | typed invalid kind recovery | unknown/reserved Layer kind diagnostic |
| LAYER-005 | layer_grammar/public AST/HIR/project/presentation tests | missing/unknown kind | LayerKindNode recovery | exact kind diagnostic |
| LAYER-006 | layer_grammar/public AST/HIR/project/presentation tests | duplicate singleton member | both typed members | duplicate primary + first related |
| LAYER-007 | layer_grammar/public AST/HIR/project/presentation tests | unknown member | ErrorDeclarationMember | syntax.layer.unknown_member |
| LAYER-008 | layer_grammar/public AST/HIR/project/presentation tests | missing assignment/value | MissingMemberValue | exact zero-width member diagnostic |
| LAYER-009 | layer_grammar/public AST/HIR/project/presentation tests | all RenderPhase values | owned phase enum | no diagnostics |
| LAYER-010 | layer_grammar/public AST/HIR/project/presentation tests | unknown phase | LayerPolicyValue recovery | closed phase diagnostic |
| LAYER-011 | layer_grammar/public AST/HIR/project/presentation tests | all input policies | owned input enum | no diagnostics |
| LAYER-012 | layer_grammar/public AST/HIR/project/presentation tests | all hit-test policies | owned hit-test enum | no diagnostics |
| LAYER-013 | layer_grammar/public AST/HIR/project/presentation tests | all capture policies | owned capture enum | no diagnostics |
| LAYER-014 | layer_grammar/public AST/HIR/project/presentation tests | all accessibility policies | owned accessibility enum | no diagnostics |
| LAYER-015 | layer_grammar/public AST/HIR/project/presentation tests | wrong-family absolute parent/view/activity ref | WrongFamilyReference | exact syntax wrong-family diagnostic |
| LAYER-016 | layer_grammar/public AST/HIR/project/presentation tests | family-relative/imported ref resolves wrong family | typed resolver cause | semantic wrong-family diagnostic |
| LAYER-017 | layer_grammar/public AST/HIR/project/presentation tests | parent missing/cycle | resolved Layer refs retained; no presentation product | project missing-parent/cycle diagnostics |
| LAYER-018 | layer_grammar/public AST/HIR/project/presentation tests | view and activity both present | both members retained; declaration inadmissible | semantic conflicting content diagnostic |
| LAYER-019 | layer_grammar/public AST/HIR/project/presentation tests | kind/content mismatch | typed reference retained | semantic Layer content-kind diagnostic |
| LAYER-020 | layer_grammar/public AST/HIR/project/presentation tests | 64 members | exact LayerMembers limit succeeds | no fatal error |
| LAYER-021 | layer_grammar/public AST/HIR/project/presentation tests | 65th member | full transaction rollback | fatal SyntaxLimit::LayerMembers |
| LAYER-022 | layer_grammar/public AST/HIR/project/presentation tests | HIR/presentation lowering | HirLayerDeclaration member IDs then deterministic LayerTree order | no free string policy/family helpers |

## Matrix completion rule

- Every row is implemented as a focused unit/integration/compile-fail test in the named owning layer.
- Range rows assert exact half-open UTF-8 byte ranges and, where LSP is involved, both UTF-8 and UTF-16 projection.
- Fatal rows compare the post-failure database/snapshot/ID state with a control run from the same pre-state.
- Poison rows assert both inspectable typed syntax/HIR error evidence and absence of project/runtime publication.
- Deletion rows compile against public or crate-owned APIs; they do not open repository files and search for symbol names.
