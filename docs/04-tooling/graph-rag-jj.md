# Graph / RAG / Jujutsu

## Source of truth

- `.arcw` source と `.arcweft/entities.toml` / `.arcweft/links.toml` が versioned。
- graph DB、RAG index、history index は派生 cache。

```text
game/**/*.arcw
  ↓ parse/lower
Typed Narrative Graph
  ↓
GraphPatch / RAG / JJ history / Visual editor
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

LLM は raw text ではなく GraphPatch を返す。

```json
{
  "base": { "program_hash": "b3:...", "jj_change_id": "qtnqlkkm" },
  "ops": [
    {
      "op": "InsertAfter",
      "after": "say.opening.001",
      "node": {
        "kind": "Say",
        "id_policy": "auto",
        "payload": { "speaker": "alice", "text": "でも、夢の中では君もそこにいた。" }
      }
    }
  ]
}
```

## Jujutsu history

表示する履歴:

- Revision history: jj log
- Entity history: node semantic hash changes
- Operation history: jj op log

Node history:

```rust
pub struct EntityHistoryEntry {
    pub node_id: NodeId,
    pub change_id: String,
    pub operation_id: Option<String>,
    pub author_timestamp: DateTimeUtc,
    pub semantic_hash_before: Option<SemanticHash>,
    pub semantic_hash_after: SemanticHash,
    pub summary: String,
}
```

## RAG

Hybrid retrieval:

```text
lexical search
vector search
graph traversal
history search
diagnostics/test search
summary/community retrieval
```

RAG context pack:

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


