# 日本語概要

この ZIP は、Lang-01.5.1.1.1 の dialogue profile owner/admission 境界を、
Arcweft `main` の `0c8cb74dd96116a8b987cc419c9a280b6cabe4a4` に対して具体化した最終契約です。

元の依頼文は 2026-07-21 付で `resolved` になっており、再 dispatch しない
よう明記されています。また現行 `main` には、その解決内容に沿った実装が
入っています。そのため、この成果物は「未実装設計の新規依頼」ではなく、
次の二つを兼ねます。

- 実装判断を残さない、自己完結した最終設計
- 現行実装がどの設計を満たしているかを示す as-built 契約

中心となる結論は、decoder/source map/spec は `arcweft-launch` のまま、
checked admission は compiler が一つの `ValidatedViewProduct` に対して行い、
ランタイムにも持ち込む六要素 revision 値だけを cycle-free な
`arcweft-dialogue` に置く、というものです。

今回その場で実行したのは ZIP と内部 manifest の機械検証です。Arcweft の
Cargo/Clippy/Tier 2 は、この環境に checkout がないため再実行していません。
リポジトリの実装記録に残る過去の通過結果と、今回の実行結果は明確に分離して
あります。
