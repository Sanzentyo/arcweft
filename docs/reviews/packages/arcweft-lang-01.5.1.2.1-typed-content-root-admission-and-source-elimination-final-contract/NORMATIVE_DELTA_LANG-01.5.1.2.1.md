# Normative delta for Lang-01.5.1.2.1

This document identifies every Lang-01.5.1.2 decision changed by the
Source-elimination reconciliation.

| Lang-01.5.1.2 area | Superseded decision | Final decision |
| --- | --- | --- |
| root family enum | included `Source` | exactly Character, Resource, Activity |
| accepted target enum | included Source entity target | no Source target; wrong/removed names use ordinary resolver outcomes |
| source-owned root | authored `source` declaration / `EntityKind::Source` | typed `res` Resource and abstract Activity are the retained source-owned semantic roots |
| Stream relationship | Source-like producer could be treated as content | no callable or Stream-returning function is a root by type/execution mode |
| reference inventory | could depend on Source/runtime producer ownership | typed selected-profile entity/resource/activity/generated references only |
| ProjectIndex | Source/content graph relation was a possible carrier | mandatory `AcceptedProjectContent` embedded in final index |
| source `content` deletion | could be staged after retaining old graph path | deletion is atomic with manifest fact publication |
| diagnostics | Source-specific family/removal handling possible | ordinary unknown/ambiguity/visibility/wrong-family diagnostics only |
| bundle/watch/LSP | could retain Source family entries | no Source entry, symbol, watch target, or compatibility node |
| deletion inventory | Source enum/type/runtime possibly retained temporarily | direct complete deletion with final Stream model |
| tests | Source family positive rows | replaced by negative absence rows and explicit Stream-callable non-root rows |

The following Lang-01.5.1.2 decisions are retained unchanged in principle and
bound to current production types:

- separate binary bytes outside `SourceDocument`;
- separate binary overlay seeds;
- complete `CharacterPackage` construction and validation;
- explicit optional Character absence;
- one canonical topology revision;
- atomic candidate publication;
- no directory scan, compatibility reader, or last-known-good candidate
  acceptance.
