# arcweft-lang-01.5.1.1.2.1.1-reactive-unary-need-match-design-validation-correction-final-contract

## 結論

**DESIGN READY / IMPLEMENTATION NOT YET CLAIMED.**

この ZIP は Lang-01.5.1.1.2.1 の完全設計を、design-validation correction
に従って独立利用可能な形で再提出します。production code、patch、overlay、
fixture 変更は含みません。

Repository authority:

- current `origin/main`: `cec30b57fa734efb059d7b846b397ac7d2b0701a`
- inspected production parent: `0fa8a3b845b2dc966f181f450a1ca1f36e49d966`
- current main is one documentation-only commit ahead; no Rust file differs.
- failed return SHA-256 supplied by request: `C5857AFCFCDDC88D2F642C4B4ACB0E61A68BBC4AC0BE42755BA9C2593B20E732`

## Final selections

- sole checked owner: `FinalSemanticAnalysis::checked_views -> CheckedViewCatalog`;
- one typed subscription per checked Need-producing expression;
- session HIR identity plus product-local ID, semantic ID, and contract digest;
- exact AWBC function/task-plan and verified `NeedId`, never source text,
  copied endpoint data, or a RuntimeValue handle;
- generation/cursor publication journal with deterministic coalescing/fanout;
- generic Match, AWBC pattern tables, typed binding registers, ordinary RuntimeValue;
- nested `Ready(Result::Err(_))` and `Ready(Option::None)`;
- transactional NotStarted start intent, `JoinSameKey`, producer-owned cancellation;
- strict version-1 in-place wire cut and complete old-Await deletion;
- live subscription as exact static-proof contaminant;
- explicit atomicity scopes and inclusive work limits.

## Reading order

1. `FINAL_CONTRACT.md`
2. `OWNERS_AND_APIS.md`
3. `RUST_SCHEMAS.md`
4. `PUBLICATION_SEMANTICS.md`
5. `MATCH_EXECUTION.md`
6. `WIRE_CODEC_SAVE_REPLAY_REPLACEMENT.md`
7. `FAILURE_PRECEDENCE_AND_ATOMICITY.md`
8. `WORK_ACCOUNTING.md`
9. `IMPLEMENTATION_SEQUENCE.md`
10. traceability, source/consumer/deletion/test matrices, and `VERIFICATION.md`

`OPEN_QUESTIONS.md` is exactly `none`. `SHA256SUMS` covers every other
payload. `tools/validate_package.py` checks artifacts, decisions, evidence,
matrices, v1 markers, manifest completeness/hashes, and exact open-question bytes.

## Verification boundary

Actually executed: input/skill reading; connected GitHub evidence at the full
SHA; package construction; manifest generation; staging validator; ZIP CRC;
clean extraction; extracted manifest and validator. Arcweft production Cargo,
test, Clippy, fmt, docs, generated, platform, and Tier-2 gates are design
admission requirements and are not claimed as already passed.
