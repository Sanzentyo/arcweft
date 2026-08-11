# Requirements traceability

| Input requirement | Normative closure | Test rows |
| --- | --- | --- |
| Complete `.awchar` carrier | one `Arc<CharacterPackage>` in `LoadedCharacterPackage`; exact logical/host provenance | C-001–C-020 |
| Binary overlay ownership | separate text/binary seeds in checked `ProfileTopologyOverlaySet` | E-001–E-010 |
| One revision | existing `ProjectTopologyRevision` over exact effective bytes, semantic digest, absences | E-011–E-022 |
| Required/optional semantics | exact NotFound-only absence and typed selected-profile references | D-001–D-014, C-003–C-013 |
| Closed families | Character, Resource, Activity only; no Source/Stream callable | A-001–A-018 |
| Manifest-owned ProjectIndex facts | `AcceptedProjectContent` required by final index; old relation deleted | B-009–B-014, G-016–G-018 |
| Bundle/watch/LSP same inventory | one `AcceptedProfileProject` and direct projections | F-001–F-014 |
| Missing manifest/layer | structured required absence or package error | C-003, C-008 |
| Duplicate/unreferenced layer | package/overlay typed rejection | C-009, C-010, E-009 |
| Corrupt PNG/package | complete decoder and dimension checks | C-005–C-014 |
| Mismatched Character identity | package admission identity error | C-007 |
| Nested path mapping | typed Character ID to contained path after resolution | C-002 |
| Optional absent unreferenced | explicit absence fact and optional watch | D-001, F-005 |
| Optional absent referenced | exact structured diagnostic with reference evidence | D-002–D-005 |
| Optional present corrupt | fail-closed | D-010 |
| Source-owned semantic roots injected | Resource/Activity typed targets in accepted content | A-007, A-008, B-012, F-012 |
| Unknown/wrong family | ordinary typed resolver diagnostics | A-009–A-016 |
| Disk/overlay parity | same effective bytes, same typed products/revision | E-004–E-008, E-020–E-021 |
| No partial publication | candidate-local products and one commit carrier | G-001–G-008 |
| No directory scan/reparse | exact named paths and accepted facts only | C-020, F-007, F-012 |
| Source elimination correction | direct deletion; no compatibility node/tag | A-017–A-018, G-016–G-018 |
| Sans-I/O | lower owner graph and metadata/behavior validation | G-019–G-020 |
| `OPEN_QUESTIONS=0` | `OPEN_QUESTIONS.md` exactly `none` | archive validation |
