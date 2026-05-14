# 用語集

| 用語 | 意味 |
|---|---|
| EntityId | rename しても変わらない内部実体 ID。 |
| PublicId | ユーザーが見たり DSL で書く ID。例: `flow.opening`。 |
| SemanticHash | 内容や意味の fingerprint。RAG 更新や履歴追跡に使う。 |
| Ref<T> | Entity への非 null 参照。`@flow.opening` は `Ref<Flow>`。 |
| Need<T, E> | 時間がかかる可能性がある値。暗黙 force 禁止。 |
| Result<T, E> | 成功/失敗。例外ではなく `?` で伝播。 |
| Option<T> | 値がない可能性。null の代替。 |
| Flow | ノベルゲームの逐次進行。suspend/resume 可能。 |
| Reducer | State と Event から Update を作る純粋状態遷移。 |
| View | State から描画/表示仕様を作る純粋関数。 |
| Activity | ノベル本編、ミニゲーム、外部 plugin を統一する実行単位。 |
| ModuleItem | DSL/Rust/WASM/precompile 由来の関数・型・Event・Activity・shader など。 |
| Agent Debug Bus | LLM/CLI/MCP が観測・操作するためのデバッグバス。 |
| Contract | `requires`、`ensures`、`invariant` などの契約。 |
| Parser | 外部入力から型付き値を作る組み込み parser。 |
| Signal | 監視用状態。Watch/Stream/Counter/Gauge/Sample。 |
| Cue | 再生可能な音声イベントや BGM 断片。 |
| AudioGraph | mixer、bus、stem、spatial source、TTS、BGM 生成を統合する音声グラフ。 |
