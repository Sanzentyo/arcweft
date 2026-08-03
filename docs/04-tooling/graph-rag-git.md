# Graph / RAG / Git

## Source of truth

- `.arcw` source and `.arcweft/entities.toml` / `.arcweft/links.toml` are
  versioned in Git.
- The full Git commit object ID is the repository revision authority.
- Graph databases, RAG indexes, entity-history indexes, and GraphPatch operation
  indexes are rebuildable derived caches.

```text
game/**/*.arcw
  ↓ parse/lower
Typed Narrative Graph
  ↓
GraphPatch / RAG / Git history / Visual editor
```

## Graph nodes

```rust
pub enum NodeKind {
    Module,
    StateType,
    StateField,
    EventType,
    Reducer,
    Flow,
    Say,
    Choice,
    ChoiceOption,
    Await,
    Command,
    AssetRef,
    ShaderRef,
    ActivityCall,
    ViewPanel,
    AudioCue,
    Bgm,
    Invariant,
    TestCase,
    DocumentNote,
}
```

## Edges

```rust
pub enum EdgeKind {
    Contains,
    Next,
    Branch,
    Goto,
    ReadsState,
    WritesState,
    EmitsEvent,
    UsesAsset,
    UsesShader,
    CallsActivity,
    OpensView,
    PlaysAudio,
    UsesBgm,
    Requires,
    Ensures,
    Mentions,
    Foreshadows,
    SimilarTo,
}
```

## GraphPatch

An LLM returns a typed GraphPatch rather than an unstructured source rewrite.
The base revision uses the full Git commit object ID. A patch operation ID is a
GraphPatch identity and is not a second version-control revision.

```json
{
  "base": {
    "program_hash": "b3:...",
    "git_commit": "70e24164373e7898ff9ef83f56f4c48523ce108e"
  },
  "ops": [
    {
      "id": "patch-op.1",
      "op": "InsertAfter",
      "after": "say.opening.001",
      "node": {
        "kind": "Say",
        "id_policy": "auto",
        "payload": {
          "speaker": "alice",
          "text": "でも、夢の中では君もそこにいた。"
        }
      }
    }
  ]
}
```

## Git and entity history

The tooling exposes three related but distinct histories:

- repository revision history from `git log`;
- entity history derived from semantic-hash changes between Git commits; and
- GraphPatch operation history retained in the rebuildable debug index.

```rust
pub struct EntityHistoryEntry {
    pub node_id: NodeId,
    pub git_commit: String,
    pub patch_operation_id: Option<String>,
    pub author_timestamp: DateTimeUtc,
    pub semantic_hash_before: Option<SemanticHash>,
    pub semantic_hash_after: SemanticHash,
    pub summary: String,
}
```

Git is the only version-control identity. The operation ID may locate one
operation inside a GraphPatch, but it must never be accepted as a repository
revision or used to require a second VCS checkout.

## RAG

Hybrid retrieval combines:

```text
lexical search
vector search
graph traversal
Git/entity history search
diagnostics/test search
summary/community retrieval
```

```rust
pub struct RagContextPack {
    pub task: AgentTask,
    pub repo: RepoContext,
    pub graph_slice: GraphSlice,
    pub source_spans: Vec<SourceSpanText>,
    pub summaries: Vec<ContextSummary>,
    pub histories: Vec<EntityHistoryEntry>,
    pub diagnostics: Vec<Diagnostic>,
    pub tests: Vec<TestReference>,
    pub constraints: Vec<Constraint>,
    pub patch_schema: JsonSchema,
}
```
