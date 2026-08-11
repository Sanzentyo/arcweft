# Input and parent archive ledger

| Input | Role | SHA-256 | Validation actually performed |
| --- | --- | --- | --- |
| `2026-07-24-lang-01.3.1.2.2.1-curried-stream-wire-allocation-reconciliation.md` | sole request | `a07f05f4ad901e323eec528b31cb0064d26f0ae23fe91cbfec9758d215a2f170` | read in full; copied verbatim as SOURCE_REQUEST.md |
| `lang-01.3.1.2-typed-stream-runtime-wire-model-final-contract(1).zip` | parent | `5043483c42259ce638b93bea7f6426d6ef169a7e22aeb426e86b7e1168a60105` | all 9 files read; 4,332 lines; JSON/CSV parsed; archive integrity checked |
| `Lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction-final-contract(1).zip` | parent correction | `66809a1280a507f69bb78d9df3bf7af227a91cd68b86cf8771cbf9ee20aa856a` | all 18 files read; 10,946 lines; both manifests and 530-case matrix checked |
| `arcweft-lang-01.3.1.2.2-curried-external-stream-final-contract(1).zip` | child correction | `d1bd7fb5301509ca88be7c9d3662942ca88472d11143499c0c3067d626df9418` | all 41 files read; 5,740 lines; manifest, 168-case validator, and host fixtures checked |
| `Rust Skill.txt` | Rust skill | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` | read completely |
| `前提(Sanzentyo-arcweft).txt` | project premise | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` | read completely |

## Parent archive verification results

```text
parent0: zip/list/UTF-8/JSON/CSV PASS; 9 files; 180780 payload bytes
parent1: zip/list/MANIFEST.json/MANIFEST.sha256/JSON/CSV PASS; 18 files; 679884 payload bytes
parent2: zip/list/manifest.json/MANIFEST.sha256/JSON PASS; 41 files; 227793 payload bytes
parent2 validation/verify_contract.py: PASS; 168 cases; manifest entries 40
parent2 host/validate_host_fixtures.py: PASS; 4 canonical valid fixtures; 2 invalid fixtures rejected
```

The attached names include `(1)` upload suffixes; normative archive identities
are established by SHA-256 and internal contents, not filename spelling.
