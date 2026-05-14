以下の方針で整理するのがよいです。特に `crates/arcweft-lang-syntax/src/tests.rs` が巨大化していて、**parser / lower / typecheck / resolve / feature regression が 1 ファイルに混在**しているので、まずここを機能別 `mod` に分けるのが一番効果があります。

なお、`src/tests.rs` はかなり大きく、取得結果が途中で切れる箇所があったため、最後に載せる **`cargo test -- --list` の before/after 比較**を移行完了条件にしてください。これを入れれば、移動漏れ・test 名消失・二重登録を機械的に検出できます。

## 確認した test の所在

現状、`#[test]` があるファイルはこの範囲です。

```text
crates/arcweft-id/src/lib.rs
crates/arcweft-need/src/lib.rs
crates/arcweft-dialogue/src/lib.rs

crates/arcweft-lang-syntax/src/tests.rs
crates/arcweft-lang-syntax/tests/parser_p0.rs
crates/arcweft-lang-syntax/tests/parser_p1.rs
```

`tokio::test` / `rstest` / `proptest` は検索上は見つかりませんでした。

---

# 推奨する全体方針

## 1. `arcweft-lang-syntax` は最優先で分割する

`crates/arcweft-lang-syntax/src/lib.rs` は今のまま、

```rust
#[cfg(test)]
mod tests;
```

を維持して、`src/tests.rs` をディレクトリ化します。

```text
crates/arcweft-lang-syntax/src/tests/
  mod.rs
  support.rs

  parser_basics.rs
  expressions.rs
  patterns.rs
  declarations.rs
  contracts.rs

  dialogue.rs
  line_plan.rs
  choice.rs
  await_.rs
  control_flow.rs

  lower_hir.rs
  typecheck.rs
  resolve_symbols.rs
  diagnostics.rs
```

`await` は Rust の予約語なので、ファイル名・mod 名は `await_` にします。

`mod.rs` はこうします。

```rust
mod support;

mod parser_basics;
mod expressions;
mod patterns;
mod declarations;
mod contracts;

mod dialogue;
mod line_plan;
mod choice;
mod await_;
mod control_flow;

mod lower_hir;
mod typecheck;
mod resolve_symbols;
mod diagnostics;
```

## 2. `tests/parser_p0.rs` と `tests/parser_p1.rs` は残す

これは `arcweft_lang_syntax::{...}` を外部 crate として import しているので、**公開 API の integration test / milestone regression test** として価値があります。

したがって、`src/tests/` に吸収せず、当面はこのまま残すのがよいです。

```text
crates/arcweft-lang-syntax/tests/parser_p0.rs
crates/arcweft-lang-syntax/tests/parser_p1.rs
```

役割はこう整理します。

```text
tests/parser_p0.rs
  public API から P0 parser 仕様を確認する integration test

tests/parser_p1.rs
  public API から P1 structured syntax 仕様を確認する integration test

src/tests/*
  crate 内部の AST / HIR / checker / resolver を使う unit regression test
```

---

# `arcweft-lang-syntax/src/tests.rs` の分割案

## `support.rs`

今の `tests.rs` 冒頭にある helper をここに移します。

```rust
use crate::{Expr, Pattern, VariantPatternPayload};

pub(super) fn variant_tuple_binding(
    pattern: &Pattern,
    variant: &str,
    binding: &str,
) -> bool {
    matches!(
        pattern,
        Pattern::Variant {
            path: None,
            name,
            payload: Some(VariantPatternPayload::Tuple(items)),
        } if name == variant
            && matches!(items.as_slice(), [Pattern::Ident(name)] if name == binding)
    )
}

pub(super) fn ident_pattern(pattern: &Pattern, expected: &str) -> bool {
    matches!(pattern, Pattern::Ident(name) if name == expected)
}

pub(super) fn expr_path_eq(expr: &Expr, expected: &str) -> bool {
    match expr {
        Expr::Path(path) => path == expected,
        Expr::Field { target, field } => {
            expected
                .rsplit_once('.')
                .is_some_and(|(prefix, expected_field)| {
                    expected_field == field && expr_path_eq(target, prefix)
                })
        }
        _ => false,
    }
}
```

将来的には、以下もここに追加すると便利です。

```rust
pub(super) fn parse_lower_ready(source: &str) -> HirModule { ... }
pub(super) fn parse_lower_typecheck(source: &str, env: &TypeCheckEnv) { ... }
pub(super) fn first_flow(tree: &SyntaxTree) -> &Flow { ... }
pub(super) fn first_hir_flow(hir: &HirModule) -> &HirFlow { ... }
```

ただし最初の移行では、helper を増やしすぎず、まず **テスト移動だけ**を完了させるのが安全です。

---

## `parser_basics.rs`

目的: parser の基本構造、トップレベル構文、entity ref、scenario command など。

入れる test:

```text
stub_is_now_real_source_parser
parses_module_use_and_pub_flow
parses_scenario_command_args_as_expressions
parses_delimited_entity_refs_with_semantic_hashes
```

このファイルは「Arcweft syntax の最低限の構文が AST に落ちる」系に限定します。

---

## `contracts.rs`

目的: `requires` / `ensures` / `effects` / `reads` / `modifies` / `assume` など契約句。

入れる test:

```text
parses_flow_contracts_before_body_block
parses_documented_contract_clauses_and_logical_ops
```

将来的に contract typecheck が増えたら、ここに parser 側と checker 側をまとめて置いてよいです。`contracts_parse.rs` / `contracts_typecheck.rs` まで増やすのは、まだ不要そうです。

---

## `dialogue.rs`

目的: speaker line / content call / fragment / dialogue callee / dialogue 後続 block。

入れる test:

```text
parses_fragment_as_flow_like_body
parses_colon_form_with_inline_bracket_content
typechecks_character_method_and_speaker_preset_dialogue_callees
parses_bare_block_after_dialogue_as_unnamed_scope
```

`typechecks_character_method_and_speaker_preset_dialogue_callees` は typecheck を含みますが、主題は dialogue callee の仕様なので `dialogue.rs` に置くのが読みやすいです。

---

## `line_plan.rs`

目的: `with:` / same-line plan / multiline plan / timed cue / cancel / out / line result。

入れる test:

```text
parses_colon_speaker_with_indented_line_plan
parses_bracket_speaker_call_with_with_colon_plan
parses_same_line_line_plan_attachments
parses_multiline_line_result_binding_with_plan
rejects_at_bracket_timed_cue_as_raw_line_plan_item
```

`parses_multiline_line_result_binding_with_plan` は `try alice.say(...)[...] with:` の `Ok` / `Err` result 型チェックも含むので、`typecheck.rs` に置きたくなりますが、主題は line result plan なので `line_plan.rs` がよいです。

---

## `choice.rs`

目的: `choice` block、option、dynamic option、match、choice plan、choice readiness。

入れる test:

```text
parses_choice_block_inside_flow
rejects_old_at_choice_syntax
parses_choice_option_with_condition
parses_choice_option_block_and_value_output
parses_dynamic_choice_options_from_for_loop
parses_choice_match_items_and_collects_arm_options
choice_body_raw_items_are_not_typecheck_ready
parses_choice_plan_option_in_sugar_label_key_and_value
```

後半に存在する可能性が高い choice 系 regression もここにまとめます。

```text
choice option select blocks
choice plan bodies
choice expression bindings
scoped relative choice IDs
dynamic choice option field typecheck
```

---

## `await_.rs`

目的: `await ... with` / wait-view / bound await / parenthesized await / branch pattern / Need<T, E>。

現時点で直接見えている test 名は出力上すべて確認しきれていませんが、コミット履歴・実装内容から、このカテゴリは必ず分離したほうがよいです。

ここに入れる対象:

```text
parse task await expressions
lower await wait-view branches
reject borrows across await boundaries
align await propagation and line-plan out syntax
parse bound await-with expressions
typecheck dotted await locals
bind variant await payloads
parse multiline await-with bindings
parse parenthesized await-with bindings
```

ファイル名は `await_.rs`。テスト名は既存名を変えずに移動するのが安全です。

---

## `control_flow.rs`

目的: `if` / `if let` / `match` / `loop` / `while` / `for` / `select` / `borrow` / `scope` / `source locale` など flow item の制御構造。

入れる test:

```text
typecheck_rejects_locals_escaping_named_and_bare_scopes
```

そのほか、`src/tests.rs` 後半にあるはずの以下のような test をここに入れます。

```text
loop expression binding
scope expression binding
if-let / while-let pattern binding
match arm guard / pattern binding
for-loop item binding
break / continue label validation
borrow block validation
source locale block lowering
select block parsing/lowering
```

ただし `choice match` は `choice.rs` に置きます。通常の `match` は `control_flow.rs` です。

---

## `declarations.rs`

目的: トップレベル宣言・型宣言・callable 宣言。

ここに入れる対象:

```text
entity declarations
source declarations
parser declarations
bodyless parser declarations
memo declarations
stream / dialogue function kinds
extern mod declarations
trait method signatures
associated type parameters
impl members
struct / enum / state / type alias declarations
```

`parser_p1.rs` 側にある `dialogue_defaults_are_preserved_as_top_level_declarations` は integration test のまま残しますが、内部 test に同種のものがあるならここに入れます。

---

## `expressions.rs`

目的: expression parser の unit regression。

`src/tests.rs` 内で expression 単体に寄っているものがあればここに移します。

```text
literal parsing
binary/unary operators
range expressions
field/index/method/call expressions
try / pipe / computation block expressions
```

ただし、すでに `tests/parser_p0.rs` に以下があるので、こちらは integration test として残します。

```text
pratt_parser_keeps_documented_precedence
generic_expr_brackets_are_indexes_not_dialogue_calls
field_and_index_are_structured_for_later_typechecking
```

---

## `patterns.rs`

目的: pattern parser / pattern binding。

入れる対象:

```text
function parameter pattern parsing
variant tuple payload binding
record pattern fields
wildcard / ident / raw pattern readiness
let-else pattern binding
await branch variant payload binding
```

ただし await branch 専用なら `await_.rs` に置いてよいです。

---

## `lower_hir.rs`

目的: feature 固有ではなく、HIR lowering 全体の契約を見る test。

ここに入れるべきもの:

```text
AST item が正しい HirTopLevelDecl / HirFlowItem に落ちること
raw item が HIR 上でどう保持されるか
lower_to_hir の失敗条件
typecheck_ready 前提の HIR invariants
```

ただし、`choice` / `dialogue` / `await` など主題がはっきりしている test は、それぞれの feature file に置いたほうがよいです。`lower_hir.rs` は「どこにも属さない HIR 契約」に限定します。

---

## `typecheck.rs`

目的: minimal checker の横断仕様。

入れる test:

```text
typechecks_flow_signature_parameters_as_locals
```

さらに後半にあると思われる以下の checker 専用 test をここに入れます。

```text
function return type mismatch
entity kind mismatch
goto destination typecheck
include target typecheck
dialogue callee typecheck
line-plan output type merging
choice output type
await Need<T, E> typecheck
borrow across suspension boundary
loop break type unification
raw expression readiness errors
```

ただし feature 主題が強いものは、その feature file に置いて OK です。

例:

```text
typechecks_character_method_and_speaker_preset_dialogue_callees
  -> dialogue.rs

choice_body_raw_items_are_not_typecheck_ready
  -> choice.rs

rejects_at_bracket_timed_cue_as_raw_line_plan_item
  -> line_plan.rs
```

`typecheck.rs` は横断的な checker test のみにすると読みやすくなります。

---

## `resolve_symbols.rs`

目的: symbol collection / registry / HIR reference validation。

入れる対象:

```text
collect_symbol_uses
registry_from_hir
validate_hir_references
NameRegistry
SymbolUseKind
entity reference validation
raw expr symbol use
```

この領域は parser / checker と混ざりやすいので、専用 mod を置いたほうが後で拡張しやすいです。

---

## `diagnostics.rs`

目的: feature に属さない parse error / recovery suggestion / old syntax rejection。

入れる対象:

```text
generic parse error shape
recovery suggestions
trailing garbage
unclosed block
unknown top-level syntax
```

ただし、feature 固有の rejection は feature 側に置きます。

```text
rejects_old_at_choice_syntax
  -> choice.rs

rejects_at_bracket_timed_cue_as_raw_line_plan_item
  -> line_plan.rs

function_signatures_reject_trailing_garbage
  -> integration tests/parser_p1.rs のまま
```

---

# integration test の整理

## `tests/parser_p0.rs`

これは今のまま残します。

役割:

```text
P0 parser public API regression
```

現在の test:

```text
pratt_parser_keeps_documented_precedence
generic_expr_brackets_are_indexes_not_dialogue_calls
hash_is_entity_ref_and_slash_comments_are_comments
doc_comments_attach_to_function_and_parameters
field_and_index_are_structured_for_later_typechecking
```

整理上の分類:

```text
expressions / parser basics / doc comments / public API
```

ただし外部 crate API を使っているので、`src/tests/expressions.rs` には移さないほうがよいです。

## `tests/parser_p1.rs`

これも今のまま残します。

役割:

```text
P1 structured syntax public API regression
```

現在の test:

```text
function_signatures_keep_generics_curried_groups_and_where_clauses
function_signatures_reject_trailing_garbage
dialogue_line_options_are_structured_not_raw_args
hook_headers_keep_when_priority_once_and_effects
dialogue_defaults_are_preserved_as_top_level_declarations
```

整理上の分類:

```text
signature / dialogue options / hook header / dialogue defaults / public API
```

---

# 他 crate の test 整理案

## `arcweft-id`

今は `src/lib.rs` 内に小さな `#[cfg(test)] mod tests` があります。

現在の test:

```text
public_id_rejects_reference_marker
public_id_rejects_reserved_prefix
text_key_accepts_domain_key
```

この規模なら inline のままで問題ありません。

分けるなら:

```text
crates/arcweft-id/src/tests.rs
```

中身:

```rust
mod validation {
    use super::*;

    #[test]
    fn public_id_rejects_reference_marker() { ... }

    #[test]
    fn public_id_rejects_reserved_prefix() { ... }

    #[test]
    fn text_key_accepts_domain_key() { ... }
}
```

ただし優先度は低いです。

## `arcweft-need`

現在の test:

```text
progress_rejects_out_of_range_ratio
need_maps_ready_only
```

これも inline のままで十分です。

分けるなら:

```text
crates/arcweft-need/src/tests.rs
```

```rust
mod progress;
mod need_state;
```

程度でよいです。

## `arcweft-dialogue`

現在の test:

```text
models_speaker_preset_and_line_plan_out
builder_api_builds_dialogue_line_from_concise_call_shape
```

ここは将来増えそうなので、分ける価値があります。

```text
crates/arcweft-dialogue/src/tests/
  mod.rs
  model.rs
  builder.rs
  content.rs
```

当面はこうで十分です。

```rust
// crates/arcweft-dialogue/src/tests/mod.rs
mod model;
mod builder;
```

対応:

```text
model.rs
  models_speaker_preset_and_line_plan_out

builder.rs
  builder_api_builds_dialogue_line_from_concise_call_shape
```

---

# 最終的なおすすめ tree

まずはこれが一番バランスがよいです。

```text
crates/
  arcweft-id/
    src/
      lib.rs                # 既存 inline tests のままで可

  arcweft-need/
    src/
      lib.rs                # 既存 inline tests のままで可

  arcweft-dialogue/
    src/
      lib.rs
      tests/
        mod.rs
        model.rs
        builder.rs

  arcweft-lang-syntax/
    src/
      lib.rs
      tests/
        mod.rs
        support.rs

        parser_basics.rs
        expressions.rs
        patterns.rs
        declarations.rs
        contracts.rs

        dialogue.rs
        line_plan.rs
        choice.rs
        await_.rs
        control_flow.rs

        lower_hir.rs
        typecheck.rs
        resolve_symbols.rs
        diagnostics.rs

    tests/
      parser_p0.rs
      parser_p1.rs
```

---

# 移行チェックリスト

## 事前確認

```bash
cargo test --workspace -- --list \
  | sed -E 's/: test$//' \
  | awk -F'::' '{print $NF}' \
  | sort \
  > /tmp/arcweft-tests.before.leaf

cargo test --workspace -- --list \
  | sort \
  > /tmp/arcweft-tests.before.full

rg -n '#\[(tokio::)?test\]|#\[test\]|#\[cfg\(test\)\]|mod tests|rstest|proptest!' crates \
  > /tmp/arcweft-test-sites.before
```

`leaf` は関数名だけを比較するためのものです。mod 分割後は full path が変わるので、まずは leaf name の一致を見るのが安全です。

## 移行作業

```text
[ ] crates/arcweft-lang-syntax/src/tests.rs を src/tests/mod.rs に置き換える
[ ] 既存 helper を support.rs に移す
[ ] test 関数名は最初の移行では変更しない
[ ] parser_basics.rs に基本 parser test を移す
[ ] contracts.rs に contract test を移す
[ ] dialogue.rs に dialogue / fragment / callee test を移す
[ ] line_plan.rs に with / out / cancel / timed cue test を移す
[ ] choice.rs に choice 系 test を移す
[ ] await_.rs に await / wait-view 系 test を移す
[ ] control_flow.rs に scope / loop / match / borrow / select 系 test を移す
[ ] declarations.rs に top-level declaration 系 test を移す
[ ] typecheck.rs に横断 checker test を移す
[ ] resolve_symbols.rs に registry / reference / symbol use test を移す
[ ] diagnostics.rs に feature 非依存の rejection / parse error test を移す
```

## 移行後確認

```bash
cargo test --workspace -- --list \
  | sed -E 's/: test$//' \
  | awk -F'::' '{print $NF}' \
  | sort \
  > /tmp/arcweft-tests.after.leaf

diff -u /tmp/arcweft-tests.before.leaf /tmp/arcweft-tests.after.leaf
```

この diff が空なら、少なくとも **test 関数名単位では抜けがありません**。

次に full path を確認します。

```bash
cargo test --workspace -- --list \
  | sort \
  > /tmp/arcweft-tests.after.full

diff -u /tmp/arcweft-tests.before.full /tmp/arcweft-tests.after.full
```

これは mod path が変わるので差分が出ます。ここでは「消えていないか」「意図しない duplicate がないか」を見ます。

最後に実行します。

```bash
cargo test -p arcweft-lang-syntax
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets
```

---

# 移動時の判断ルール

迷ったらこの順番で置き場所を決めるとブレにくいです。

```text
1. その test が一番守っている言語機能は何か？
   choice / dialogue / line_plan / await / control_flow など

2. 特定機能より checker 契約が主題か？
   そうなら typecheck.rs

3. HIR の構造そのものが主題か？
   そうなら lower_hir.rs

4. registry / collect_symbol_uses / reference validation が主題か？
   そうなら resolve_symbols.rs

5. parse error の形だけが主題か？
   そうなら diagnostics.rs
```

このルールだと、たとえば `choice_body_raw_items_are_not_typecheck_ready` は checker を使っていますが、主題は choice body の raw item なので `choice.rs` に置きます。

---

# 実装上の注意

最初の移行では、**test 名を絶対に変えない**のがよいです。
名前変更まで同時にやると、`cargo test -- --list` の比較で「移動漏れ」と「rename」が混ざって確認しにくくなります。

順序としてはこれがおすすめです。

```text
1. test 名を変えずに mod 分割だけ行う
2. before/after の leaf test list が一致することを確認
3. workspace test を通す
4. その後、必要なら test 名を整理する
```

結論としては、`arcweft-lang-syntax/src/tests.rs` は **feature 別の flat module 構成**に分け、`parser_p0.rs` / `parser_p1.rs` は public API integration test として残すのが最も安全で、今後の拡張にも強いです。
