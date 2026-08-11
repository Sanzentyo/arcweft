# Test matrix

This matrix contains 122 direct rows. Tests SHALL exercise typed APIs,
observable behavior, codecs, compile-fail boundaries, deterministic artifacts,
and dependency metadata. No test passes or fails by scanning checked-in source
for a spelling, symbol, snippet, or file location.

## Shared rollback assertion

Every row whose publication effect says “no publication” also asserts
pointer-identical prior accepted carrier/index/catalog/watch state, unchanged
accepted generation, and absence of a candidate cache or bundle namespace.

| ID | Owner/API | Setup/input | Exact expected result | Publication/revision effect |
| --- | --- | --- | --- | --- |
| A-001 | arcweft-project::content | Construct Character target | family=Character; file_backed=true | no publication |
| A-002 | arcweft-project::content | Construct Resource target | family=Resource; file_backed=false | no publication |
| A-003 | arcweft-project::content | Construct Activity target | family=Activity; file_backed=false | no publication |
| A-004 | sema content resolver | Root resolves to Character through canonical spelling | AcceptedContentRootTarget::Character | candidate continues |
| A-005 | sema content resolver | Root resolves through compact Character alias | same canonical CharacterId | candidate continues |
| A-006 | sema content resolver | Root resolves through qualified Character alias | same canonical CharacterId | candidate continues |
| A-007 | sema content resolver | Root resolves to typed res declaration | AcceptedContentRootTarget::Resource exact identity triple | candidate continues |
| A-008 | sema content resolver | Root resolves to abstract Activity | AcceptedContentRootTarget::Activity | candidate continues |
| A-009 | sema content resolver | Unknown root | project.content.unknown_root at root span | no candidate product |
| A-010 | sema content resolver | Two visible candidates collide | project.content.ambiguous_root with deterministic candidates | no candidate product |
| A-011 | sema content resolver | Target exists but is private across boundary | project.content.invisible_root + declaration range | no candidate product |
| A-012 | sema content resolver | Target is View | project.content.wrong_family actual=View | no candidate product |
| A-013 | sema content resolver | Target is Asset | project.content.wrong_family actual=Asset | no candidate product |
| A-014 | sema content resolver | Ordinary function returns Stream | project.content.wrong_family actual=Callable | no candidate product |
| A-015 | sema content resolver | Authored generator returns Stream | project.content.wrong_family actual=Callable | no candidate product |
| A-016 | sema content resolver | External capability returns Stream | project.content.wrong_family actual=Callable | no candidate product |
| A-017 | parser/sema | Removed source declaration spelling | ordinary current grammar/type failure; no Source node | no candidate product |
| A-018 | public API compile-fail | Attempt to name Source family/target variant | compile failure because variant/type absent | n/a |
| B-001 | ManifestSourceMap | Content unit table path | exact revision-bound span | candidate continues |
| B-002 | ManifestSourceMap | Root ordinal 0 path | exact array element span | candidate continues |
| B-003 | ManifestSourceMap | Nested root ordinal path | correct element without text search | candidate continues |
| B-004 | ManifestSourceMap | Visibility path | exact visibility value span | candidate continues |
| B-005 | ManifestSourceMap | Demand path | exact demand value span | candidate continues |
| B-006 | ManifestSourceMap | Profile content table path | exact table span | candidate continues |
| B-007 | ManifestSourceMap | Residency/placement/compression paths | three exact value spans | candidate continues |
| B-008 | ManifestSourceMap | Span belongs to another document identity | ManifestSourceEvidenceMissing/revision mismatch | no candidate product |
| B-009 | AcceptedContentUnitFact::try_new | Empty roots | typed invariant error | no fact |
| B-010 | AcceptedContentUnitFact::try_new | Duplicate/non-contiguous ordinals | typed invariant error | no fact |
| B-011 | AcceptedProjectContent::try_new | Two facts share target but differ ordinal/policy | both facts retained | candidate continues |
| B-012 | ProjectSemanticIndex | Build with accepted content | accessors return pointer-shared authority | candidate continues |
| B-013 | ProjectSemanticIndex API | Query roots/references by typed target | deterministic exact records | no reparse/I/O |
| B-014 | public API compile-fail | Attempt to construct published index without content | constructor inaccessible/missing argument | n/a |
| C-001 | project-loader + CharacterPackage | Selected Character manifest and every exact layer on disk | complete Arc<CharacterPackage> | candidate continues |
| C-002 | path mapper | @character.npc.alice | assets/npc/alice.awchar exact contained path | candidate continues |
| C-003 | project-loader | Required Character manifest NotFound | RequiredContentRootAbsent | no publication |
| C-004 | project-loader | Manifest exists but permission denied | I/O error, not absence | no publication |
| C-005 | CharacterPackage | Manifest invalid UTF-8 | present-invalid error | no publication |
| C-006 | CharacterPackage | Manifest malformed JSON/schema | decode/manifest error | no publication |
| C-007 | CharacterPackage | Manifest Character ID mismatches resolved target | identity mismatch | no publication |
| C-008 | CharacterPackage | One named layer missing | MissingLayerPayload / I/O missing layer | no publication |
| C-009 | CharacterPackage | Duplicate explicit layer payload seed | DuplicateLayerPayload | no publication |
| C-010 | CharacterPackage | Explicit payload not named by manifest | UnreferencedLayerPayload or unconsumed binary overlay | no publication |
| C-011 | CharacterPackage | PNG signature valid but stream truncated | InvalidLayerPng | no publication |
| C-012 | CharacterPackage | PNG has trailing/corrupt frame data | InvalidLayerPng after complete decode/finish | no publication |
| C-013 | CharacterPackage | PNG dimensions differ from manifest rect | LayerDimensionsMismatch | no publication |
| C-014 | CharacterPackage | Two manifest variants claim the same asset path where package ownership requires one payload identity | manifest/package structured error | no publication |
| C-015 | project-loader | Same Character target appears in two content units | one loaded package; two root facts | candidate continues |
| C-016 | AcceptedProfileProject | Present Character fact but package absent | carrier invariant error | no publication |
| C-017 | AcceptedProfileProject | Loaded package has no present fact | carrier invariant error | no publication |
| C-018 | AcceptedProfileProject | Layer topology record omitted | carrier invariant error | no publication |
| C-019 | bundle projection | Complete accepted package | existing BundleCharacterPackage conversion emits exact manifest/layers | bundle succeeds |
| C-020 | instrumented filesystem | Extra disk file under .awchar not named by manifest | never enumerated or admitted | revision/bundle unchanged |
| D-001 | presence classifier | Optional Character manifest NotFound; no typed references | AbsentOptional exact record | candidate continues |
| D-002 | presence classifier | Optional Character manifest NotFound; source entity reference exists | ReferencedOptionalContentRootAbsent | no publication |
| D-003 | presence classifier | Optional Character manifest NotFound; Resource value reference exists | ReferencedOptionalContentRootAbsent | no publication |
| D-004 | presence classifier | Optional Character manifest NotFound; Activity binding reference exists | ReferencedOptionalContentRootAbsent | no publication |
| D-005 | presence classifier | Optional Character manifest NotFound; generated metadata reference exists | ReferencedOptionalContentRootAbsent | no publication |
| D-006 | presence classifier | Only root declaration names Character | does not count as reference; absence accepted | candidate continues |
| D-007 | presence classifier | Reference occurs in dead code of admitted module | counts; referenced absence error | no publication |
| D-008 | presence classifier | Reference exists only in module not admitted by selected profile | does not count | candidate continues |
| D-009 | presence classifier | Optional manifest present and valid | Present | candidate continues |
| D-010 | presence classifier | Optional manifest present but corrupt | fail-closed package error | no publication |
| D-011 | AcceptedContentRootFact | Attempt AbsentOptional for Resource | typed invariant error | no fact |
| D-012 | AcceptedContentRootFact | Attempt AbsentOptional for Activity | typed invariant error | no fact |
| D-013 | semantic resolver | Optional Resource unknown | unknown root; optional does not mask | no publication |
| D-014 | semantic resolver | Optional Activity invisible | invisible root; optional does not mask | no publication |
| E-001 | ProfileTopologyOverlaySet | Duplicate text path | overlay_duplicate | no reads/publication |
| E-002 | ProfileTopologyOverlaySet | Duplicate binary path | overlay_duplicate | no reads/publication |
| E-003 | ProfileTopologyOverlaySet | Same path in text and binary | overlay_kind_conflict | no reads/publication |
| E-004 | overlay resolver | Text overlay for arcw.toml | strict decoder consumes overlay once | candidate continues |
| E-005 | overlay resolver | Text overlay for .arcw module | SourceDocument contains exact overlay text | candidate continues |
| E-006 | overlay resolver | Text overlay for Character manifest | source-backed manifest uses overlay identity | candidate continues |
| E-007 | overlay resolver | Binary overlay for Character PNG | CharacterPackage retains exact overlay bytes | candidate continues |
| E-008 | overlay resolver | Binary bytes supplied through text type | rejected by API/type boundary | n/a or no publication |
| E-009 | overlay resolver | Unconsumed binary layer overlay | overlay_unconsumed | no publication |
| E-010 | overlay resolver | Overlay path escapes containment | containment error | no publication |
| E-011 | ProjectTopologyRevision | arcw.toml byte mutation | revision differs | new candidate revision only |
| E-012 | ProjectTopologyRevision | source module byte mutation | revision differs | new candidate revision only |
| E-013 | ProjectTopologyRevision | generated metadata byte mutation | revision differs | new candidate revision only |
| E-014 | ProjectTopologyRevision | Character manifest byte mutation | revision differs | new candidate revision only |
| E-015 | ProjectTopologyRevision | layer byte mutation | revision differs | new candidate revision only |
| E-016 | ProjectTopologyRevision | resource registry digest mutation | revision differs | new candidate revision only |
| E-017 | ProjectTopologyRevision | optional absent becomes present | revision differs | new candidate revision only |
| E-018 | ProjectTopologyRevision | optional present becomes absent | revision differs | new candidate revision only |
| E-019 | ProjectTopologyRevision | root/policy mutation in manifest | revision differs | new candidate revision only |
| E-020 | ProjectTopologyRevision | disk replaced by byte-identical overlay | revision equal | may reuse accepted carrier |
| E-021 | ProjectTopologyRevision | overlay version changes; bytes equal | revision equal | no accepted generation required |
| E-022 | ProjectTopologyRevision | resource insertion order changes | revision equal | deterministic transcript |
| F-001 | bundle | Absent optional Character | no Character package/file entry | bundle succeeds |
| F-002 | bundle | Two facts same present Character | one package record/files | bundle succeeds |
| F-003 | bundle consistency | Topology has accepted present Character resources | bundle inventory exactly matches | bundle succeeds |
| F-004 | watch | Present manifest/layers | MustExist exact entries | watch inventory published with carrier |
| F-005 | watch | Absent optional Character | one OptionalMayAppear manifest entry | watch inventory published with carrier |
| F-006 | watch | Optional manifest appears | schedule full rebuild; no direct package mutation | accepted carrier unchanged until success |
| F-007 | watch | Extra unreferenced disk layer appears | no inferred watch entry | accepted carrier unchanged |
| F-008 | LSP | Successful text+binary overlay rebuild | one AcceptedProfileProject generation | atomic publication |
| F-009 | LSP | Request uses old revision/generation | typed stale result | no mixed facts |
| F-010 | LSP | Definition for accepted Character layer | uses accepted source/package provenance | no disk rescan |
| F-011 | Agent/CLI | Inspect absent optional root | typed absence + expected path + revision | no fake package |
| F-012 | compiler | Query manifest content facts | reads ProjectSemanticIndex accepted_content | no manifest decode |
| F-013 | cache | Same SourceSetRevision but binary layer changed | cache miss because topology revision changed | new namespace |
| F-014 | cache | Failed candidate revision | no accepted cache namespace | prior namespace intact |
| G-001 | transaction | Unknown root after source/sema candidate exists | all candidate objects dropped | accepted pointers/generation unchanged |
| G-002 | transaction | Missing required manifest after root resolution | all candidate objects dropped | accepted pointers/generation unchanged |
| G-003 | transaction | Corrupt layer after other packages built | all candidate packages dropped | accepted pointers/generation unchanged |
| G-004 | transaction | Referenced optional absence | no partial topology/index/catalog | accepted pointers/generation unchanged |
| G-005 | transaction | Revision transcript duplicate present record | revision error | accepted pointers/generation unchanged |
| G-006 | AcceptedProfileProject | Topology/index package ID, package version, profile ID, or topology revision differ | exact AcceptedProjectIdentityMismatch variant for the mismatched axis | no publication |
| G-007 | publisher CAS | Two concurrent candidates; later-started commits first | stale earlier completion discarded | winner remains |
| G-008 | publisher | Byte-identical rebuild | pointer/generation may remain identical | no duplicate publication |
| G-009 | limits | Exact topology resource count limit | success | candidate can publish |
| G-010 | limits | One over topology resource count | bounded typed error | no publication |
| G-011 | limits | Exact aggregate overlay byte limit | success | candidate can publish |
| G-012 | limits | One over overlay byte limit | bounded typed error | no publication |
| G-013 | limits | Exact reference/work limit | success with all references retained | candidate can publish |
| G-014 | limits | One over reference/work limit | work_limit_exceeded, no truncation | no publication |
| G-015 | arithmetic | u32/u64 transcript conversion overflow | typed overflow error | no publication |
| G-016 | deletion behavior | Source family has no accepted target/project fact | typed current API proves absence | no consumer output |
| G-017 | deletion behavior | source content declaration in parser fixture | ordinary grammar recovery, following declaration preserved | no executable HIR node |
| G-018 | deletion behavior | Old Source wire/tag fixture | strict current decoder rejects unknown tag | no compatibility read |
| G-019 | architecture | arcweft-project dependency graph | no path to sema/loader/LSP | structural gate passes |
| G-020 | architecture | core/data crates under instrumented tests | no filesystem calls; bytes supplied by adapters | structural/behavior gate passes |

## Required parity fixtures

At least one maintained schema-1 fixture SHALL exercise all three retained
families in one content unit, one absent unreferenced optional Character in a
second unit, disk and overlay variants, and bundle/watch/LSP projections from
the same accepted carrier.
