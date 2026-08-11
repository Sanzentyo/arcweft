# Complete positive, negative, revision, consumer, and transaction test matrix

Every test is normative for production completion. Tests are grouped by semantic
purpose, but IDs define a stable review ledger. Exact diagnostic wording may
follow repository conventions; target category, source evidence, failure order,
revision behavior, and publication outcome are mandatory.

| ID | Category | Scope | Scenario | Required result/evidence |
|---|---|---|---|---|
| TM-001 | family-positive | manifest/sema | Exact character target appears at content-units.<id>.roots | Accepted once with CharacterId / Character package, exact occurrence spans, and closed ContentRootFamily. |
| TM-002 | family-positive | manifest/sema | Exact flow target appears at content-units.<id>.roots | Accepted once with EntityId + Flow, exact occurrence spans, and closed ContentRootFamily. |
| TM-003 | family-positive | manifest/sema | Exact view target appears at content-units.<id>.roots | Accepted once with EntityId + View, exact occurrence spans, and closed ContentRootFamily. |
| TM-004 | family-positive | manifest/sema | Exact action target appears at content-units.<id>.roots | Accepted once with EntityId + Action, exact occurrence spans, and closed ContentRootFamily. |
| TM-005 | family-positive | manifest/sema | Exact activity target appears at content-units.<id>.roots | Accepted once with EntityId + Activity, exact occurrence spans, and closed ContentRootFamily. |
| TM-006 | family-positive | manifest/sema | Exact asset target appears at content-units.<id>.roots | Accepted once with EntityId + Asset, exact occurrence spans, and closed ContentRootFamily. |
| TM-007 | family-positive | manifest/sema | Exact signal target appears at content-units.<id>.roots | Accepted once with EntityId + Signal, exact occurrence spans, and closed ContentRootFamily. |
| TM-008 | family-positive | manifest/sema | Exact metric target appears at content-units.<id>.roots | Accepted once with EntityId + Metric, exact occurrence spans, and closed ContentRootFamily. |
| TM-009 | family-positive | manifest/sema | Exact layer target appears at content-units.<id>.roots | Accepted once with EntityId + Layer, exact occurrence spans, and closed ContentRootFamily. |
| TM-010 | family-positive | manifest/sema | Exact configured res target appears at content-units.<id>.roots | Accepted once with ResourceDeclarationIdentity, exact occurrence spans, and closed ContentRootFamily. |
| TM-011 | wrong-position | manifest/profile resolver | Exact character reference is supplied as profiles.<profile>.entry | Rejected by the ordinary Entry-target resolver with actual typed target evidence; no content root or partial profile. |
| TM-012 | wrong-position | manifest/profile resolver | Exact flow reference is supplied as profiles.<profile>.entry | Rejected by the ordinary Entry-target resolver with actual typed target evidence; no content root or partial profile. |
| TM-013 | wrong-position | manifest/profile resolver | Exact view reference is supplied as profiles.<profile>.entry | Rejected by the ordinary Entry-target resolver with actual typed target evidence; no content root or partial profile. |
| TM-014 | wrong-position | manifest/profile resolver | Exact action reference is supplied as profiles.<profile>.entry | Rejected by the ordinary Entry-target resolver with actual typed target evidence; no content root or partial profile. |
| TM-015 | wrong-position | manifest/profile resolver | Exact activity reference is supplied as profiles.<profile>.entry | Rejected by the ordinary Entry-target resolver with actual typed target evidence; no content root or partial profile. |
| TM-016 | wrong-position | manifest/profile resolver | Exact asset reference is supplied as profiles.<profile>.entry | Rejected by the ordinary Entry-target resolver with actual typed target evidence; no content root or partial profile. |
| TM-017 | wrong-position | manifest/profile resolver | Exact signal reference is supplied as profiles.<profile>.entry | Rejected by the ordinary Entry-target resolver with actual typed target evidence; no content root or partial profile. |
| TM-018 | wrong-position | manifest/profile resolver | Exact metric reference is supplied as profiles.<profile>.entry | Rejected by the ordinary Entry-target resolver with actual typed target evidence; no content root or partial profile. |
| TM-019 | wrong-position | manifest/profile resolver | Exact layer reference is supplied as profiles.<profile>.entry | Rejected by the ordinary Entry-target resolver with actual typed target evidence; no content root or partial profile. |
| TM-020 | wrong-position | manifest/profile resolver | Exact configured res reference is supplied as profiles.<profile>.entry | Rejected by the ordinary Entry-target resolver with actual typed target evidence; no content root or partial profile. |
| TM-021 | wrong-position | manifest decoder | A root array is authored under profiles.<profile>.content.<unit> | Strict unknown-field/schema failure; no second root reader. |
| TM-022 | wrong-position | manifest decoder | A root is authored in external-module metadata path fields | Strict type/schema failure; field is not a root position. |
| TM-023 | family-negative | sema | An Entry target appears in content-unit roots | WrongContentRootFamily with Entry declaration span. |
| TM-024 | family-negative | sema | A Choice or ChoiceOption appears in roots | WrongContentRootFamily with exact nested entity kind. |
| TM-025 | family-negative | sema | DialogueLine or Text appears in roots | WrongContentRootFamily with exact generated/scoped kind. |
| TM-026 | family-negative | sema | Input/Button/Style appears in roots | WrongContentRootFamily with exact scoped/runtime kind. |
| TM-027 | family-negative | sema | Scene/Capture/Hook appears in roots | WrongContentRootFamily with actual kind. |
| TM-028 | family-negative | sema | Slot/Target/presentation target/scroll region appears in roots | Wrong family or wrong symbol kind; scoped retained evidence. |
| TM-029 | configured-resource | resource resolver | @image.x or @voice.x has no exact configured resource declaration | Wrong/unknown root result; prefix alone does not accept. |
| TM-030 | configured-resource | resource resolver | @image.x has one exact accepted configured resource declaration | ConfiguredResource target with exact ResourceDeclarationIdentity. |
| TM-031 | configured-resource | resource resolver | Two resource declarations claim one canonical public identity | ConfiguredResourceIdentityCollision with both declaration spans; no first-wins. |
| TM-032 | unknown | sema | Unknown prefix and no exact configured declaration | UnknownContentRootFamily/Target using ordinary resolver distinction. |
| TM-033 | syntax | manifest decoder | Malformed root reference or whitespace/control marker | InvalidContentRootReference at exact selection/value span. |
| TM-034 | alias | symbol resolver | Visible alias/reexport resolves to an accepted authored entity | Accepted canonical original EntityId; occurrence and binding spans retained. |
| TM-035 | ambiguity | symbol resolver | Two visible aliases resolve ambiguously | AmbiguousContentRootTarget with all candidates sorted canonically. |
| TM-036 | visibility | symbol resolver | Private/inaccessible target is named | InaccessibleContentRootTarget with declaration and binding spans. |
| TM-037 | visibility | sema finalizer | Public content unit names package/private target | ContentRootVisibilityEscalation at unit visibility plus target evidence. |
| TM-038 | visibility | sema finalizer | Content unit visibility is no broader than target | Accepted; ProjectIndex stores accepted unit visibility. |
| TM-039 | revision | sema finalizer | Symbol revision differs from candidate/reference inventory | ContentRootWorldMismatch before presence/consumer construction. |
| TM-040 | revision | sema finalizer | Resource declaration world differs from topology registry/metadata | ContentRootWorldMismatch; no configured-resource target published. |
| TM-041 | source-removal | parser | Author-facing source declaration is parsed after migration | Normal parser rejection/recovery; no SourceItem accepted node. |
| TM-042 | source-removal | HIR | Removed source syntax reaches lowering recovery | No HirSource/HirTopLevelDecl::Source executable node. |
| TM-043 | source-removal | sema compile test | Code attempts to construct EntityKind::Source | Compile-fail after enum removal; no alias. |
| TM-044 | source-removal | sema compile test | Code attempts to construct TypeKind::Source | Compile-fail after enum removal; no alias. |
| TM-045 | source-removal | content admission | Manifest root formerly naming a Source declaration has no final target | Ordinary unknown/wrong-target diagnostic; no accepted target/fact. |
| TM-046 | source-removal | ProjectIndex | Candidate includes removed Source-family reference | No content fact, relation, compatibility entity, or partial index. |
| TM-047 | source-removal | bundle | Candidate includes removed Source-family reference | No bundle entry/table/section; transaction fails. |
| TM-048 | source-removal | watch | Candidate includes removed Source-family reference | No watch path; transaction fails. |
| TM-049 | source-removal | LSP | Candidate includes removed Source-family reference | No Source/root symbol; ordinary diagnostic only. |
| TM-050 | source-removal | serialization/structural | Search typed topology/bundle/cache schemas for a Source root variant/tag through API inventory | No variant/tag; proof is typed schema/codec tests, not a source gate. |
| TM-051 | content-removal | parser | Source content declaration is parsed after migration | Normal parser rejection/recovery; no EntityDeclKind::Content/ContentDeclBody accepted. |
| TM-052 | content-removal | HIR/sema | Removed source content syntax is lowered/checked | No executable content node, EntityKind::Content, or old root relation. |
| TM-053 | content-removal | ProjectIndex | Manifest content unit is admitted | Direct ProjectContentUnitFact/ProjectContentRootFact; no compatibility Content entity. |
| TM-054 | content-removal | LSP | Manifest content unit is published | Manifest symbol/fact only; no source content declaration symbol. |
| TM-055 | compatibility | compile-fail | Code uses old source-named content-root family type or SourceEntity variant | Compile-fail; no type/variant alias. |
| TM-056 | stream-negative | sema | Ordinary fn returns Stream and immediately passes through another Stream | WrongContentRootSymbolKind(Callable); return type ignored for admission. |
| TM-057 | stream-negative | sema | Ordinary authored fn returns Stream and has own-scope yield | WrongContentRootSymbolKind(Callable); generator mode ignored. |
| TM-058 | stream-negative | sema | Extern capability operation returns Stream | WrongContentRootSymbolKind(Callable/External); external origin ignored. |
| TM-059 | stream-negative | symbol resolver | Alias/reexport points to Stream callable | Canonical callable declaration shown; no accepted content target. |
| TM-060 | stream-negative | sema | Function is named source, content, feed, or stream | Wrong callable kind; name heuristic absent. |
| TM-061 | stream-negative | sema | Non-Stream function is named like an accepted family | Wrong callable kind; family spelling cannot convert callable. |
| TM-062 | stream-negative | runtime/bundle | Accepted project contains Stream callables not referenced as roots | Normal callable/runtime behavior; no content root/bundle content entry created. |
| TM-063 | stream-negative | structural | AcceptedContentRootTarget exhaustive match | No Callable/Generator/ExternalStream variant. |
| TM-064 | presence | loader/finalizer | Required Character present and valid | Present Character accepted with exact package. |
| TM-065 | presence | loader | Required Character absent | RequiredRootMissing; no candidate publication. |
| TM-066 | presence | loader/finalizer | Optional selected Character present and valid | Present accepted; referenced_by includes Profile. |
| TM-067 | presence | loader | Optional selected Character absent | OptionalRootReferencedMissing(Profile). |
| TM-068 | presence | loader/finalizer | Optional unselected Character present and valid | Present accepted; referenced_by None unless runtime reference. |
| TM-069 | presence | finalizer | Optional unselected Character absent and unreferenced | Accepted OptionalAbsent fact and revision absence record. |
| TM-070 | presence | finalizer | Optional unselected Character absent with typed runtime reference | OptionalRootReferencedMissing(Runtime) at exact reference. |
| TM-071 | presence | finalizer | Optional selected Character absent with typed runtime reference | OptionalRootReferencedMissing(ProfileAndRuntime). |
| TM-072 | presence | loader | Optional/unselected Character present but corrupt | Exact Character validation failure; optional does not mask invalid. |
| TM-073 | presence | sema | Optional authored entity is unresolved | Unknown target; not an optional absence. |
| TM-074 | presence | resource resolver | Optional configured resource is unresolved | Unknown target; not an optional absence. |
| TM-075 | presence | reference collector | Typed reference occurs only in statically dead branch | Counts as runtime reference; absent Character fails closed. |
| TM-076 | presence | grouping | Same Character appears required and optional across units | Shared target is required; one package acquired. |
| TM-077 | presence | grouping | Same Character appears optional in selected and unselected units | Shared target is profile-referenced; absence fails. |
| TM-078 | presence | grouping | Same absent optional Character appears in several unselected units | One watch state; one absence record per occurrence. |
| TM-079 | presence | grouping | Runtime reference targets a shared Character | Every occurrence gets runtime reference classification. |
| TM-080 | presence | reservation | Absent Character reservation matches exact typed reference | Content admission missing diagnostic, not generic unknown symbol. |
| TM-081 | presence | reservation | Inspect symbol/catalog/runtime outputs for absence reservation | Reservation absent from all outputs. |
| TM-082 | character | package | Complete Character package with exact valid PNG bytes | One shared CharacterPackage; exact bytes retained unchanged. |
| TM-083 | character | package | Character manifest path absent | Required/optional state machine selects exact missing result. |
| TM-084 | character | package | Manifest names a missing layer | CharacterLayerMissing with manifest asset evidence. |
| TM-085 | character | package | Manifest contains duplicate canonical layer path | Duplicate/membership failure; no first-wins. |
| TM-086 | character | package | Layer payload is corrupt/truncated PNG | Complete decoder failure; no header-only acceptance. |
| TM-087 | character | package | PNG dimensions differ from manifest | CharacterLayerDimensionsMismatch with expected/actual. |
| TM-088 | character | package | Manifest CharacterId differs from root identity | CharacterIdentityMismatch. |
| TM-089 | character | package | SourceBackedCharacterManifest document identity mismatches supplied document | ManifestSourceIdentityMismatch. |
| TM-090 | character | loader | Unreferenced binary layer overlay is supplied | UnconsumedContentBinaryOverlay/UnreferencedCharacterLayerPayload. |
| TM-091 | character | loader | Unrelated file exists in .awchar directory | Ignored; no directory enumeration and no revision effect. |
| TM-092 | character | containment | Manifest asset path attempts escape/non-normal form | Typed path/containment failure before I/O outside project. |
| TM-093 | character | dependency | Dependency-owned Character layer seed has wrong owner/kind/logical path | Typed seed failure; no path guessing. |
| TM-094 | character | sharing | Two roots share identical Character package | One Arc package and one present resource set; occurrence facts remain separate. |
| TM-095 | overlay | loader | Manifest text from disk versus identical text overlay | Same decoded model, spans/document identity policy, and topology revision. |
| TM-096 | overlay | loader | Character binary from disk versus identical binary overlay | Same Character package and topology revision. |
| TM-097 | overlay | revision | One manifest/source/metadata/PNG overlay byte changes | Corresponding resource digest and topology revision change. |
| TM-098 | overlay | loader | Text and binary overlay target same logical path with wrong payload kind | Typed payload-kind conflict; no lossy document conversion. |
| TM-099 | overlay | transaction | Overlay revision becomes stale during candidate build | StaleContentRootOverlay; no partial publication. |
| TM-100 | revision | topology | Manifest root spelling/order/unit/demand/visibility changes | Manifest record changes and topology revision changes. |
| TM-101 | revision | topology | Selected source entity target/family/visibility changes | Source record changes and topology revision changes. |
| TM-102 | revision | topology | Generated metadata changes configured-resource/reference facts | Metadata record changes and topology revision changes. |
| TM-103 | revision | topology | Resource type registry semantic digest changes | 0x20 semantic record and topology revision change. |
| TM-104 | revision | topology | Character manifest/layer/package bytes change | 0x04/0x05 record and topology revision change. |
| TM-105 | revision | topology | Character transitions present to optional absent | Present records replaced by 0x80 absence record; revision changes. |
| TM-106 | revision | topology | Package ID/version or selected profile changes | Header/resources and revision change. |
| TM-107 | revision | determinism | Input maps/seeds are inserted in different orders | Identical canonical transcript/revision and diagnostics. |
| TM-108 | revision | duplicates | Duplicate canonical present/semantic/absence key | Typed duplicate error even when bytes/digest match. |
| TM-109 | revision | determinism | Identical transcript is evaluated twice | Identical accepted targets and consumer projections; divergence is defect. |
| TM-110 | revision | compiler | Checked program same but topology revision differs | ProgramHash/cache key differs through inherent constructor. |
| TM-111 | reference | typed HIR | Authored source contains exact reference to present accepted root | Runtime reference fact with exact whole/selection spans. |
| TM-112 | reference | generated metadata | Accepted generated metadata contributes typed reference | Fact bound to metadata SourceDocument and topology revision. |
| TM-113 | reference | configured resource | Typed ResourceRef/retained field references accepted root | Fact emitted from typed value path/source span; no string scan. |
| TM-114 | reference | configured resource | String field text resembles @character.x | No reference fact; schema type is not reference. |
| TM-115 | reference | identity | ResourceRef<T>, AssetRef<P>, RetainedIdentityRef<K> use wrong category | Typed schema/type mismatch before content reference admission. |
| TM-116 | reference | revision | Reference inventory symbol/topology revision is stale | ContentRootWorldMismatch; no presence reconciliation. |
| TM-117 | ProjectIndex | projection | Present root admitted | One unit fact, one occurrence fact, typed contains/resolves relations. |
| TM-118 | ProjectIndex | projection | Optional absent Character admitted | Occurrence fact with OptionalAbsent; no target relation/symbol. |
| TM-119 | ProjectIndex | projection | Alias root admitted | Target relation uses canonical original identity; alias span retained as evidence. |
| TM-120 | ProjectIndex | structural | Inspect graph/entity enum outputs | No EntityKind::Content/Source compatibility node and no old PublicId content-root edge. |
| TM-121 | bundle | projection | Required/present roots compiled | Bundle seeds/entries derive only from accepted inventory and existing resource codecs. |
| TM-122 | bundle | projection | Optional absent Character compiled | No payload/section; absence remains metadata/fact only. |
| TM-123 | bundle | projection | Authored entity root compiled | Reachability/partition seed only; no invented bytes. |
| TM-124 | bundle | projection | Configured resource root compiled | Existing resource bundle-section/codec owner used. |
| TM-125 | watch | projection | Present topology accepted | Watch set equals exact present resource paths with MustExist. |
| TM-126 | watch | projection | Optional absent Character accepted | Exact expected manifest path has OptionalMayAppear; no directory glob. |
| TM-127 | watch | projection | Authored entity/configured resource/callable root case | No extra filesystem discovery/watch path. |
| TM-128 | LSP | projection | Accepted present roots | Manifest symbols/links point to canonical target definitions. |
| TM-129 | LSP | projection | Optional absent Character | Diagnostic/status only; no Character symbol. |
| TM-130 | LSP | projection | Removed Source/content syntax/root | No compatibility symbol; ordinary diagnostics. |
| TM-131 | consumer-consistency | instrumented integration | Bundle/watch/LSP are given an I/O/parse counter after acceptance | Zero manifest/source/resource rescans; all consume same inventory. |
| TM-132 | consumer-consistency | revision | All consumers report revision keys | Same topology/symbol/program identities across ProjectIndex/bundle/watch/LSP. |
| TM-133 | transaction | atomic integration | Failure during manifest decode | Typed failure; no accepted manifest/topology/world/content/index/consumer state. |
| TM-134 | transaction | atomic integration | Failure during profile selection | Typed failure; no accepted candidate or consumer state. |
| TM-135 | transaction | atomic integration | Failure during project containment/path | Typed failure; no out-of-root I/O result or partial state. |
| TM-136 | transaction | atomic integration | Failure during generated metadata hash/identity/ABI | Typed failure; no symbol/resource/content state. |
| TM-137 | transaction | atomic integration | Failure during Character package validation | Typed failure; no package/topology/content state. |
| TM-138 | transaction | atomic integration | Failure during symbol collision/ambiguity | Typed failure; no content/index/consumer state. |
| TM-139 | transaction | atomic integration | Failure during visibility | Typed failure; no content/index/consumer state. |
| TM-140 | transaction | atomic integration | Failure during wrong revision/stale overlay | Typed failure; no replacement cache/watch/LSP state. |
| TM-141 | transaction | atomic integration | Failure during runtime reference to absent optional root | Typed failure; no accepted content/bundle state. |
| TM-142 | transaction | atomic integration | Failure during consumer candidate construction | Typed failure; no partial publication of earlier candidate products. |
| TM-143 | transaction | reload | A prior accepted snapshot exists and new candidate fails | API returns failure for attempted revision; prior snapshot is never labelled current/final/fallback success. |
| TM-144 | transaction | reload | New candidate succeeds after previous failure | One coherent new publication replaces prior state atomically. |
| TM-145 | diagnostics | determinism | Same multi-error candidate built with different map/thread order | Same bounded diagnostic order and primary failure class. |
| TM-146 | budgets | loader/sema | Resource/root/reference work exceeds configured limit | Typed limit failure; no partial publication. |
| TM-147 | budgets | arithmetic | Count/string/resource length conversion overflows | Typed arithmetic overflow distinct from ordinary limit. |
| TM-148 | structural | API | EntityKind classification implementation | Inherent exhaustive method; no extension trait or loader-local string table. |
| TM-149 | structural | API | Configured resource resolution implementation | One accepted index inherent lookup; no duplicate map/helper. |
| TM-150 | structural | API | Project graph content support | Original graph enums extended in place; no parallel ad hoc graph. |
| TM-151 | structural | API | Old source-named family/target APIs used by downstream compile test | Compile-fail; no alias/dual reader. |
| TM-152 | structural | codec | Topology/bundle/LSP serialization roundtrip | No Source root/family/provisional tag. |
| TM-153 | structural | parser/compiler | Removed syntax fixtures | Parser rejection and absence of executable typed node; not a repository scan. |
| TM-154 | structural | layering | Cargo dependency/feature audit | Core/data-format crates remain Sans I/O; no LSP/CLI/filesystem dependency inversion. |
| TM-155 | structural | format/lint | Production implementation cut | cargo fmt --check and cargo clippy --workspace --all-targets --all-features -- -D warnings pass. |
| TM-156 | workspace | tests | Production implementation cut | Focused and cargo test --workspace pass. |
| TM-157 | Tier 2 | repository harness | Production implementation cut | Tier 2 harness passes with recorded command/output. |
| TM-158 | structural audit | repository | Production implementation cut | File metrics, ownership, duplicate-table, dependency, and consumer-rescan audit recorded. |
| TM-159 | package | final-contract archive | This delivery archive validation | All hashes/entries/state/lints pass; fallback=false; no production patch files. |
| TM-160 | intake | repository | Returned ZIP is added to docs/reviews | Package-specific intake records outer hash, internal hashes, pinned head, and implementation state before coding. |

## Required command-level production validation

At the final implementation cut, record exact repository revision and command
output for focused tests, workspace tests, formatting, clippy with warnings
denied, Tier 2, and structural audit. This final-contract delivery itself does
not claim those production commands were executed because production code was
explicitly left unchanged and no checkout was available.

## No source-gate substitution

TM-041 through TM-055 and TM-153 are proven through parser, HIR, sema,
compiler, codec, ProjectIndex, bundle, watch, and LSP behavior. A grep/substring
zero-hit check is not a substitute and must not be installed as an automated
source gate.
