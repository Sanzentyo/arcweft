# Package summary

- Sequence: Lang-01.5.1.2.1
- Status: READY_FOR_IMPLEMENTATION
- Open questions: 0
- Repository commit: `0c8cb74dd96116a8b987cc419c9a280b6cabe4a4`
- Test rows: 122
- Production implementation: not performed

The contract selects Character/Resource/Activity as the sole content-root
families, removes Source and Stream-callable promotion, reuses the existing
binary/CharacterPackage/ProjectTopologyRevision substrate, defines exact
optional-absence and reference semantics, embeds manifest content facts in the
final ProjectSemanticIndex, and atomically publishes topology plus index through
AcceptedProfileProject. Bundle, watch, LSP, cache, Agent, and CLI consume that
same carrier.
