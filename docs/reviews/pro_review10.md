# Codex作業指示書: Arcweft相対ID / EntityRef曖昧性の完全解消

対象リポジトリ: `Sanzentyo/arcweft`  
対象ブランチ: `main`

## 0. 絶対方針

このリポジトリはまだ開発中で、既存ユーザー互換を考慮しない。  
**旧構文を残す、migration modeを置く、旧構文の履歴をエラーメッセージに出す、旧表記への書き換え提案を出す、という対応は禁止。**

やることは、現行canonical grammarへ完全移行すること。

具体的には次を徹底する。

- `@` はEntityRefまたはID-bearing context専用のRelativeIdにだけ使う。
- `@command` 形のシナリオコマンドは禁止。
- `@derive(...)` 形の属性は禁止。
- `@choice ...` 形の旧choice構文は禁止。
- `@memo` 形は禁止。
- `.hoge` はRelativeIdではない。**すべて構文エラーで落とす。**
- `@.hoge` と `@..hoge` は許可する。
- `@...hoge` 以上は受理してよいがlint/diagnostic対象にする。
- `@super.hoge` / `@super.super.hoge` は許可する。
- 一般EntityRef文脈で `@.hoge`, `@..hoge`, `@super.hoge` を使うことは禁止。
- EntityRefの一般相対参照として、**family-qualified relative EntityRef** を採用する。
  - 例: `@flow:.next`, `@frag:.intro`, `@asset:.room`
  - `goto @.next` は禁止。
  - `goto @flow:.next` は許可。

## 1. 主要な設計変更

### 1.1 EntityRefとRelativeIdをAST上で分離する

現状、`EntityRef` が `relative: bool` と `relative_parent_depth` を持っている。これはやめる。

対象箇所:

- `crates/arcweft-lang-syntax/src/ast.rs`
  - 目安: `EntityRef` 定義周辺、現行mainでおおむね `L80-L105`
  - 現状:
    ```rust
    pub struct EntityRef {
        body: String,
        delimited: bool,
        relative: bool,
        relative_parent_depth: usize,
        range: TextRange,
    }
    ```
  - 変更後の方向:
    ```rust
    pub struct EntityRef {
        body: String,
        delimited: bool,
        range: TextRange,
    }

    pub enum IdRef {
        Absolute(EntityRef),
        Relative(RelativeId),
    }

    pub struct RelativeId {
        suffix: String,
        parent_depth: usize,
        spelling: RelativeIdSpelling,
        range: TextRange,
    }

    pub enum RelativeIdSpelling {
        DotRun,
        SuperChain,
    }
    ```

`EntityRef` は必ず絶対参照にする。  
`RelativeId` はdialogue line ID、text_key、choice ID、option ID、label text keyなどのID-bearing contextにだけ出現させる。

### 1.2 family-qualified relative EntityRefを追加する

禁止するもの:

```awft
goto @.next
include @.intro
window=@.side
signal @.changed <- true
````

採用するもの:

```awft
goto @flow:.next
include @frag:.intro
window=@textbox:.side
signal @signal:.changed <- true
```

構文案:

```text
RelativeEntityRef := '@' EntityFamily ':' RelativeTail
EntityFamily      := Ident
RelativeTail      := DotRelativeTail | SuperRelativeTail
DotRelativeTail   := '.' Ident ('.' Ident)*
                   | '..' Ident ('.' Ident)*
                   | '...' Ident ('.' Ident)*   # allowed, but lint
SuperRelativeTail := 'super' ('.' 'super')* '.' Ident ('.' Ident)*
```

解決規則:

```text
@flow:.next
  -> current flow family/root contextから flow.{...}.next へ解決

@flow:..sibling
  -> parent flow-relative contextから解決

@frag:.intro
  -> fragment familyとして解決
```

この構文は一般EntityRef文脈でだけ使う。
ID-bearing contextでは従来通り `@.suffix`, `@..suffix`, `@super.suffix` を使ってよい。

## 2. token方針

既存の文字列splitをこれ以上増やさない。
RelativeId / RelativeEntityRef / EntityRefの判定はtoken stream上で行う。

対象箇所:

* `crates/arcweft-lang-syntax/src/cst.rs`

  * `CstRelativeId` 定義周辺: 目安 `L55-L68`
  * `CstEntityRef` 定義周辺: 目安 `L70-L82`
  * `lex_cst`, `next_token`, `split_leading_entity_ref_parts`, `split_leading_relative_id`, `starts_leading_relative_id` 周辺: 目安 `L780-L1100`
* `crates/arcweft-lang-syntax/src/parser.rs`

  * import周辺: 目安 `L1-L35`
  * `parse_optional_entity_ref`, `parse_required_entity_ref`, `parse_line_options`, `parse_choice_*` 周辺

やること:

1. `CstRelativeEntityRef` を追加する。
2. `split_leading_relative_entity_ref` を追加する。
3. `split_leading_relative_id` はID-bearing context専用にする。
4. `split_leading_entity_ref_parts` は `@.`, `@..`, `@super.` を一般EntityRefとして返してはいけない。
5. `@family:.suffix` / `@family:..suffix` / `@family:super.suffix` をtokenとして読み分ける。
6. `@...suffix` はparseは通すが、diagnostic/lintを出せるように `parent_depth >= 2` かつ `DotRun` を保持する。

## 3. `.hoge` は完全禁止

`.hoge` はRelativeIdとして扱わない。
`.hoge` はvariant shorthandなど、既存の式/パターン文脈で意味がある場合だけ許可する。ID文脈では必ずエラー。

対象箇所:

* `crates/arcweft-lang-syntax/src/parser.rs`

  * `parse_line_options` 周辺
  * `parse_choice_id` / `parse_option_id` / compact choice arm parser 周辺
  * `parse_dialogue_defaults` のID受理周辺
* `crates/arcweft-lang-syntax/src/tests/dialogue.rs`

  * relative dialogue line option tests周辺: 目安 `L120-L190`
* `crates/arcweft-lang-syntax/src/tests/choice.rs`

  * relative choice ID tests周辺: 目安 `L300-L390`

追加テスト:

```rust
#[test]
fn rejects_bare_dot_relative_ids_in_id_contexts() {
    for source in [
        r#"alice(id=.greeting): hello"#,
        r#"alice(text_key=.greeting): hello"#,
        r#"choice .first { @.listen "聞く" -> @flow.next }"#,
        r#"choice @.first { .listen "聞く" -> @flow.next }"#,
        r#"option .listen { label = "聞く" }"#,
        r#"label(id=.choice_label) = "聞く""#,
    ] {
        let errors = parse_errors(source);
        assert!(
            errors.iter().any(|e| e.message().contains("relative IDs must start with `@.`")),
            "expected bare-dot relative id error for {source:?}, got {errors:?}"
        );
    }
}
```

エラーメッセージは旧構文の履歴を出さない。
例:

```text
relative IDs must start with `@.`
```

## 4. `@..` までは通常許可、`@...` 以上はlint/diagnostic

許可:

```awft
@.suffix
@..suffix
@super.suffix
```

許可するがlint対象:

```awft
@...suffix
@....suffix
```

lint文言案:

```text
deep dot-relative ID is hard to read; prefer `@super.super.suffix`
```

ただし、これはparse errorではない。

対象箇所:

* `crates/arcweft-lang-syntax/src/cst.rs`

  * `split_leading_relative_id` 周辺: 目安 `L950-L1050`
* `crates/arcweft-lang-syntax/src/lower.rs`

  * `relative_scopes`, `append_relative_suffix` 周辺: 目安 `L430-L500`
* lintの置き場がまだなければ、当面はparse diagnosticsではなくsyntax check層へ追加する。

  * 候補: `crates/arcweft-lang-syntax/src/check.rs`

追加テスト:

```rust
#[test]
fn accepts_current_and_parent_relative_ids_but_lints_deep_dot_runs() {
    parse_ok(r#"alice(id=@.greeting): hello"#);
    parse_ok(r#"alice(id=@..shared): hello"#);

    let parsed = arcweft_lang_syntax::parse_source(
        r#"alice(id=@...too_deep): hello"#
    );
    assert!(parsed.errors().is_empty());

    // lint/check APIがある場合:
    // assert!(lint_errors.iter().any(|e| e.message().contains("prefer `@super.super")));
}
```

## 5. family-qualified relative EntityRefを実装する

これは採用する。

### 5.1 AST追加

対象:

* `crates/arcweft-lang-syntax/src/ast.rs`

  * `EntityRef` 定義周辺: 目安 `L80-L105`

追加案:

```rust
pub enum EntityRef {
    Absolute {
        body: String,
        delimited: bool,
        range: TextRange,
    },
    Relative {
        family: String,
        relative: RelativeId,
        range: TextRange,
    },
}
```

または、EntityRefを絶対専用に保ちたい場合:

```rust
pub enum EntityRefSyntax {
    Absolute(EntityRef),
    FamilyRelative(FamilyRelativeEntityRef),
}

pub struct FamilyRelativeEntityRef {
    family: String,
    relative: RelativeId,
    range: TextRange,
}
```

推奨は後者。
HIR/registryへ渡す前に必ず絶対 `EntityRef` に正規化する。

### 5.2 parser追加

対象:

* `crates/arcweft-lang-syntax/src/cst.rs`

  * `CstEntityRef` / `CstRelativeId` 周辺: 目安 `L55-L82`
  * `split_leading_entity_ref_parts` 周辺: 目安 `L900-L1050`
* `crates/arcweft-lang-syntax/src/parser.rs`

  * `parse_required_entity_ref` 周辺
  * `parse_optional_entity_ref` 周辺
  * `Stmt::Goto`, `Include`, `LineOption window`, `HookTarget`, `Signal`, `ChoiceAction::Goto` がEntityRefを読む箇所

追加テスト:

```rust
#[test]
fn parses_family_qualified_relative_entity_refs() {
    let tree = parse_ok(r#"
flow @flow.opening opening {
    goto @flow:.next
    include @frag:.alice_enters
    signal @signal:.choice_visible <- true
}
"#);

    // ASTではFamilyRelativeとして保持してもよい。
    // HIR lowering後は絶対EntityRefへ正規化されていることを確認する。
}
```

禁止テスト:

```rust
#[test]
fn rejects_unqualified_relative_entity_refs_in_reference_contexts() {
    for source in [
        r#"flow @flow.opening opening { goto @.next }"#,
        r#"flow @flow.opening opening { include @.intro }"#,
        r#"flow @flow.opening opening { signal @.changed <- true }"#,
        r#"hook @hook.test on @.target phase AfterLayout {}"#,
    ] {
        let errors = parse_errors(source);
        assert!(
            errors.iter().any(|e| e.message().contains("relative entity references must include a family")),
            "expected family-qualified relative entity ref error for {source:?}, got {errors:?}"
        );
    }
}
```

## 6. named scope / ID contextの解決を構造化する

現在のloweringは、choice optionなどで文字列splitに近い方法を使っている。
これをID context stackで解決する。

対象:

* `crates/arcweft-lang-syntax/src/lower.rs`

  * `LowerContext` 定義周辺: 目安 `L120-L150`
  * `normalize_choice_id`: 目安 `L390-L410`
  * `normalize_option_id`: 目安 `L410-L430`
  * `normalize_text_key_id`: 目安 `L430-L455`
  * `normalize_line_id`: 目安 `L455-L480`
  * `normalize_line_text_key`: 目安 `L480-L510`
  * `build_line_entity_ref`: 目安 `L510-L540`
  * `relative_scopes`: 目安 `L540-L555`
  * `append_relative_suffix`: 目安 `L555-L570`

変更案:

```rust
#[derive(Clone, Debug, Default)]
struct LowerContext {
    flow_slug: Option<String>,
    scopes: Vec<String>,
    choice_stack: Vec<IdPath>,
    line_counters: HashMap<String, usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IdPath {
    family: IdFamily,
    segments: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum IdFamily {
    Say,
    Text,
    Voice,
    Choice,
    ChoiceOption,
    Flow,
    Fragment,
    Asset,
    Signal,
    Textbox,
    Style,
    Other(String),
}
```

必須要件:

* `parent_depth` が実在するIDスコープより深い場合はエラー。
* `@..x` は許可。
* `@...x` 以上はlint対象だが、親スコープが足りるなら解決する。
* `append_relative_suffix` のような単純文字列popは廃止または内部限定にする。
* HIR/registryに登録されるIDは完全正規化済みにする。

## 7. ナレーター予約語を増やす

現状、`speaker_slug` は `"地の文"` と `"narrator"` をnarrator扱いしている。
これを増やす。

対象:

* `crates/arcweft-lang-syntax/src/lower.rs`

  * `speaker_slug` 周辺: 目安 `L575-L610`

予約語として扱う候補:

```text
地の文
地文
ナレーター
ナレータ
ナレーション
語り
語り手
Narrator
narrator
NARRATOR
Narration
narration
VoiceOver
voiceover
voice_over
VO
vo
V.O.
v.o.
Off
off
Offscreen
offscreen
O.S.
o.s.
Script
script
StageDirection
stage_direction
ト書き
脚本
```

実装方針:

* 内部slugはすべて `"narrator"` に正規化する。
* 大文字小文字、ピリオド、アンダースコア差分はある程度吸収する。
* ただし任意のspeaker名を勝手にnarratorへ寄せない。
* 予約語一覧は関数内matchに直書きでもよいが、できれば `const NARRATOR_ALIASES` にする。

例:

```rust
fn is_narrator_alias(input: &str) -> bool {
    let normalized = input
        .trim()
        .trim_end_matches(".say")
        .replace('.', "")
        .replace('_', "")
        .to_lowercase();

    matches!(
        normalized.as_str(),
        "地の文"
            | "地文"
            | "ナレーター"
            | "ナレータ"
            | "ナレーション"
            | "語り"
            | "語り手"
            | "narrator"
            | "narration"
            | "voiceover"
            | "vo"
            | "off"
            | "offscreen"
            | "os"
            | "script"
            | "stagedirection"
            | "ト書き"
            | "脚本"
    )
}
```

追加テスト:

```rust
#[test]
fn narrator_aliases_normalize_to_narrator_slug() {
    for speaker in [
        "地の文",
        "ナレーター",
        "ナレーション",
        "語り手",
        "narrator",
        "Narrator",
        "VO",
        "V.O.",
        "O.S.",
        "ト書き",
        "脚本",
    ] {
        let tree = parse_ok(format!(
            r#"
flow @flow.opening opening {{
    {speaker}(id=@.line):
        text[p]
}}
"#
        ));

        let hir = lower_to_hir(&tree).expect("lower");
        let HirFlowItem::Dialogue(line) = &hir.flows()[0].body()[0] else {
            panic!("expected dialogue");
        };
        assert_eq!(
            line.id().expect("id").body(),
            "say.opening.narrator.line"
        );
    }
}
```

## 8. canonical以外の旧`@`構文を完全削除する

### 8.1 `@derive(...)` 属性を禁止

対象:

* `crates/arcweft-lang-syntax/src/cst.rs`

  * `classify_top_level_line`: 目安 `L500-L515`
  * 現状、`trimmed.starts_with('@')` をAttribute扱いしている。
* `crates/arcweft-lang-syntax/src/parser.rs`

  * `CstTopLevelLineKind::Attribute` 分岐: 目安 `L210-L225`
  * `parse_attribute` 関数周辺
* `crates/arcweft-lang-syntax/src/tests/parser_basics.rs`

  * `parses_attributes_and_wiki_links`: 目安 `L150-L170`

変更:

* Attributeは `#[...]` のみ許可する。
* `@derive(Debug)` はparse error。
* エラーに「旧」や「migration」は書かない。

テスト修正:

```rust
#[test]
fn parses_attributes_and_wiki_links() {
    let tree = parse_ok(
        r#"
/// links to [[flow.alice_intro]]
#[derive(Debug)]
flow @flow.opening opening {}
"#,
    );

    assert_eq!(tree.wiki_links()[0].body(), "flow.alice_intro");
    assert!(matches!(&tree.items()[0], Item::Attribute(attr) if attr.name() == "derive"));
    assert!(matches!(&tree.items()[1], Item::Flow(_)));
}
```

追加禁止テスト:

```rust
#[test]
fn rejects_at_prefixed_attributes() {
    let errors = parse_errors("@derive(Debug)\nflow @flow.opening opening {}");
    assert!(
        errors.iter().any(|e| e.message().contains("attributes must use `#[...]`"))
    );
}
```

### 8.2 `@bg`, `@show` などのシナリオコマンドを禁止

対象:

* `crates/arcweft-lang-syntax/src/parser.rs`

  * `parse_flow_item_until_indent` 周辺
  * `parse_scenario_command` 周辺
* `crates/arcweft-lang-syntax/src/cst.rs`

  * `is_typed_stmt` / flow item分類周辺: 目安 `L650-L760`
* `crates/arcweft-lang-syntax/src/tests/parser_basics.rs`

  * `parses_module_use_and_pub_flow`: 目安 `L15-L40`
  * `parses_scenario_command_args_as_expressions`: 目安 `L40-L70`
* `crates/arcweft-lang-syntax/src/tests/dialogue.rs`

  * `parses_fragment_as_flow_like_body`: 目安 `L1-L25`

変更後:

```awft
bg(@asset.bg.room, fade = 300ms)
show(alice, normal, at = right, fade = 220ms)
```

禁止:

```awft
@bg @asset.bg.room fade=300ms
@show alice normal at=right fade=220ms
```

テスト修正例:

```rust
#[test]
fn parses_scenario_command_args_as_expressions() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    show(alice, normal, at = right, fade = 220ms)
}
"#,
    );

    // ScenarioCommandまたはStmt::Expr(Expr::Call)のどちらに正規化するかは実装に合わせる。
    // ただし `@show` は絶対に受理しない。
}
```

追加禁止テスト:

```rust
#[test]
fn rejects_at_prefixed_scenario_commands() {
    for source in [
        r#"flow @flow.opening opening { @bg @asset.bg.room fade=300ms }"#,
        r#"flow @flow.opening opening { @show alice normal }"#,
    ] {
        let errors = parse_errors(source);
        assert!(
            errors.iter().any(|e| e.message().contains("scenario commands are function calls")),
            "expected @command rejection for {source:?}, got {errors:?}"
        );
    }
}
```

### 8.3 `@choice` を完全禁止

対象:

* `crates/arcweft-lang-syntax/src/cst.rs`

  * `CstFlowItemKind::OldChoiceAttribute`: 目安 `L130-L140`
  * `classify_flow_item`: 目安 `L620-L650`
* `crates/arcweft-lang-syntax/src/parser.rs`

  * `reject_old_choice_attribute` または相当処理
* `crates/arcweft-lang-syntax/src/tests/choice.rs`

  * `rejects_sigiled_choice_keyword_syntax`: 目安 `L25-L45`

変更:

* `CstFlowItemKind::OldChoiceAttribute` を削除する。
* `@choice` を特別扱いしない。
* `@choice @choice.opening.first { ... }` は通常の不正構文として落とす。
* エラーメッセージに「old」「旧」「migration」は入れない。

期待する正しい構文:

```awft
choice @choice.opening.first {
    @choice.opening.listen "聞いてみる" -> @flow.alice_intro
}
```

### 8.4 `@memo` を完全禁止

対象:

* `crates/arcweft-lang-syntax/src/cst.rs`

  * `CstTopLevelLineKind::OldMemoAttribute`: 目安 `L95-L105`
  * `classify_top_level_line`: 目安 `L500-L515`
* `crates/arcweft-lang-syntax/src/parser.rs`

  * `reject_old_memo_attribute`: 目安 `L310-L335`

変更:

* `OldMemoAttribute` variantを削除する。
* `reject_old_memo_attribute` を削除する。
* `@memo` は通常の不正構文として落とす。
* エラーメッセージに履歴を残さない。

正しい構文:

```awft
memo fn cached_value(...) -> Type { ... }

let value = memo(scope=scene, key=(score)) {
    score
}
```

## 9. 一般EntityRef文脈で禁止すべきパターン

次はそのまま禁止として残す。
ただし `@family:.suffix` は採用する。

禁止:

```awft
goto @.next
goto @..next
goto @super.next

include @.intro
include @..intro
include @super.intro

window=@.side
style=@.dream
hooks=[@.hook]

hook @hook.test
on @.target
phase AfterLayout
{}

signal @.changed <- true

use @.characters::{alice}
mod @.routes
```

許可:

```awft
goto @flow:.next
include @frag:.intro
window=@textbox:.side
style=@style:.dream
hooks=[@hook:.dialogue_read]
signal @signal:.changed <- true
```

対象:

* `crates/arcweft-lang-syntax/src/parser.rs`

  * `parse_stmt` / `goto` / `include` / `signal` / hook target parser周辺
  * line option parser内の `window`, `hooks`, `style`
* `crates/arcweft-lang-syntax/src/tests/parser_basics.rs`

  * `rejects_relative_id_syntax_in_module_and_use_paths`: 目安 `L90-L115`
* `crates/arcweft-lang-syntax/src/tests/dialogue.rs`

  * line options tests周辺
* `crates/arcweft-lang-syntax/src/tests/resolve_symbols.rs`

  * registry resolution tests周辺

## 10. compact choice armの動的IDをparse段階で落とす

現状、`route.choice_id "Dynamic label" -> ...` はraw寄りに流れてtypecheck-readyで落ちる。
これはparse段階で落とす。

対象:

* `crates/arcweft-lang-syntax/src/parser.rs`

  * compact choice arm parser周辺
* `crates/arcweft-lang-syntax/src/tests/choice.rs`

  * `rejects_dynamic_id_in_compact_choice_arm`: 目安 `L250-L280`

禁止:

```awft
choice @choice.opening.routes {
    route.choice_id "Dynamic label" -> @flow.alice_intro
}
```

許可:

```awft
choice @choice.opening.routes {
    option route in opening_routes(state) {
        id = route.choice_id
        label = route.label
        select { goto route.target }
    }
}
```

エラー文言案:

```text
compact choice arms require a static option ID
```

追加/修正テスト:

```rust
#[test]
fn rejects_dynamic_id_in_compact_choice_arm_at_parse_time() {
    let errors = parse_errors(
        r#"
choice @choice.opening.routes {
    route.choice_id "Dynamic label" -> @flow.alice_intro
}
"#,
    );

    assert!(
        errors.iter().any(|e| e.message().contains("compact choice arms require a static option ID")),
        "expected parse-time compact choice id diagnostic, got {errors:?}"
    );
}
```

## 11. docs更新

対象:

* `docs/01-language/grammar.md`

  * lexical conventions / EntityRef / RelativeId周辺: 目安 `L5-L35`
  * dialogue / choice ID周辺: 目安 `L85-L170`
* `docs/01-language/ids-and-references.md`

  * 相対IDセクション: 目安 `L70-L230`
* `docs/00-overview/decisions.md`

  * Entity, attribute, command syntax decision: 目安 `L25-L55`
  * Scope and relative ID decision: 目安 `L95-L160`
* `docs/01-language/modules.md`

  * module/use relative ID禁止周辺: 目安 `L1-L80`
* `docs/examples/scope-relative-ids.md`

  * コメントに `#` を使っている箇所: 目安 `L70-L80`

docsで必ず反映すること:

```text
EntityRef:
  @flow.opening
  @asset.bg.room
  @<flow.alice_intro@sem:...>

FamilyRelativeEntityRef:
  @flow:.next
  @frag:.intro
  @asset:.room

RelativeId:
  @.suffix
  @..suffix
  @super.suffix
  @super.super.suffix
```

禁止明記:

```text
Bare `.suffix` is never a relative ID.
General references must not use `@.suffix`; use `@family:.suffix` or an absolute `@family.name`.
Attributes use only `#[...]`.
Scenario operations are ordinary function calls.
```

`docs/examples/scope-relative-ids.md` のコメントは `#` ではなく `//` にする。

修正例:

```awft
alice(id=@.greeting):        // relative ID context
use self::characters::alice  // module path context
goto @flow.opening.next      // ordinary entity reference
goto @flow:.next             // family-qualified relative entity reference
```

## 12. 必須テスト実行

Codexは作業後に次を実行する。

```bash
cargo test -p arcweft-lang-syntax
cargo test
```

可能なら追加で:

```bash
cargo fmt
cargo clippy --all-targets --all-features
```

## 13. 完了条件

* `EntityRef` は相対IDを直接保持しない。
* ID-bearing contextでは `IdRef` / `RelativeId` を使う。
* `.hoge` はID文脈で必ずエラー。
* `@.hoge` / `@..hoge` はID文脈で許可。
* `@...hoge` 以上はparse可、lint/diagnostic対象。
* 一般参照では `@.hoge` / `@..hoge` / `@super.hoge` を禁止。
* 一般参照では `@flow:.next` のようなfamily-qualified relative EntityRefを許可。
* `@derive`, `@bg`, `@show`, `@choice`, `@memo` は完全禁止。
* エラーメッセージに旧構文・migration・後方互換の説明を残さない。
* ナレーター予約語が `narrator` slugへ正規化される。
* compact choice armの動的IDはparse段階で落ちる。
* docsとtestsが新方針に一致している。

```
