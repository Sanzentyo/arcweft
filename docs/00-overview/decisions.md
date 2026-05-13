# 決定事項と設計原則

## 言語・型

1. DSL は Rust 風の `mod` / `use` / `pub` / `fn` / `enum` / `match` を持つ。
2. 型パラメータは `<>` を使う。例: `List<T>`, `Result<T, E>`, `Ref<Flow>`。
3. Entity 参照は通常 `#flow.opening`、境界が必要なとき `#<activity.truck_game>.run(...)`。
4. コメント・docs のリンクは `[[flow.opening]]`。
5. 属性・構造化リンクは `@link<Flow>(#flow.opening, level = soft)`。
6. `null` はない。欠損は `Option<T>`、失敗は `Result<T, E>`、遅延は `Need<T, E>`。
7. ADT をサポートし、`match` は exhaustive check を行う。
8. 関数は純粋関数を基本とし、effect は `Command` / `Need` / `Task` として値で返す。
9. 関数はカリー化・部分適用・チェイン・パイプをサポートする。
10. Object Hook は first-class item とし、対象・phase・check policy・when・effects を持つ。
11. Memoization は pure computation に限定し、explicit key と dependency hash によって invalidation する。

## 実行

1. Core は Sans I/O。
2. reducer と view は `await` 禁止。
3. flow 内の `await` は `pending` branch を必須にする。
4. task fn 内では裸 `await` を許可できる。
5. lazy import / lazy use は明示的・局所的・非再帰的。
6. import 時副作用は禁止。
7. Cranelift JIT は native-only の最適化バックエンド。VM が正で、JIT は同値性検査を必須にする。
8. Hook output は phase boundary で deterministic order に commit し、hook の同期再入を禁止する。
9. Memo cache の hit/miss は replay の state hash に影響しない。

## ID / 参照 / 履歴

1. 内部実体 ID は `EntityId`。rename しても変わらない。
2. ユーザー向け名前は `PublicId`。rename 可能。
3. 内容 fingerprint は `SemanticHash`。
4. RAG・履歴・GraphPatch は `EntityId + SemanticHash` で扱う。
5. ID は省略可能。LSP inlay hint と code action で自動生成・固定化できる。
6. Jujutsu 履歴は node-level semantic history として再構成する。

## UI / Render / Audio

1. Game Native UI は構造化 View tree として持つ。
2. HTML/CSS は native Servo / Web DOM の別 backend。
3. WGSL shader は ModuleItem として扱い、typed params/resources/capability を持つ。
4. SVG は build-time に Vector IR へ normalize する。
5. Typst 級組版は RichText と TypesetBlock に分ける。
6. Audio は mixer graph / bus / cue / spatial / TTS / BGM authoring を一体で扱う。

## Tooling / Verification

1. 契約プログラミングは `requires` / `ensures` / `invariant` / `modifies` / `effects` / `decreases` を持つ。
2. 契約は runtime check、debug check、SMT、Kani/Creusot、test generation に流せる。
3. 入力はすべて `Parser<T, ParseError>` を通す。
4. Logging は defmt 風の deferred structured logging。
5. Signal は Watch / Stream / Counter / Gauge / Sample。
5a. Signal と state path は hook trigger および memo invalidation の依存として使える。
6. Test / bench / Agent Debug Bus は同じ観測・操作基盤を使う。



## Layer / Hook / Memo

1. Layer は描画順だけでなく、入力 routing、hit-test、focus、modal、Agent 観測の単位である。
2. Input は RawInputEvent を LayerTree と UiTree で routing してから `FrameInput` に渡す。
3. Hook は Object + Phase + CheckPolicy + Condition + Body で定義する。
4. Hook は direct state mutation をしない。状態変更は Command / Event を通す。
5. Hook は phase capability によって読み取り・変更可能範囲を制限する。
6. Memoization は pure computation を基本対象にし、task memo は `Need` と scheduler に統合する。
7. Memo key は args / dependencies / source hash / profile を含む。
8. Memo の有無で state hash が変わってはならない。

## Object Hooks / Memoization

- Object Hook は Entity / Layer / UI node / Signal / State path へ attach できる。
- Hook は state を直接変更せず、Event / Command / Signal / Log / Assert を返す。
- Hook phase と check schedule は明示する。暗黙 every-frame hook は禁止または warning。
- Hook condition は純粋式で、重い条件は memo 化する。
- Memoization は純粋・決定的な計算だけを対象にし、scope と dependency fingerprint を持つ。
- Need task coalescing は memo key system と共有する。
