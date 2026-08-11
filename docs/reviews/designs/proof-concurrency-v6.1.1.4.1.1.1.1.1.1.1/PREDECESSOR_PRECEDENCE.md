# Predecessor precedence and direct audit

## Order of authority

1. **This package and its embedded 2026-07-30 request** are authoritative for
   E13 Select producer reachability, central projection, `?.` decomposition,
   final member payload, poison/diagnostic obligations, source applicability,
   accounting, rollback, and deletion targets.
2. **Proof v6.1.1.4.1.1 source-owner correction** remains authoritative for the
   sole typed source index/query, typed owner resolution, liveness payloads,
   source identity, presence/status, and validation order. This package only
   supplies the corrected E13 applicability rows.
3. **Proof v6.1.1.4.1.1.1.1 tail-owner/generator correction** is the final
   synthetic-role admission, production-generator, liveness, fingerprint, and
   descendant-accounting authority. It supersedes the historical synthetic
   parent where it says so.
4. **Proof v6.1.1.4.1.1.1 synthetic-role package** is retained only through the
   non-tail admission rows expressly preserved by the tail correction. E13
   imports its `RecoveryOperand` vocabulary solely to prove that E13 never
   allocates such a child.
5. **Proof v6.1.1.4.1 leaf/expression package**, as corrected by later accepted
   packages, remains authoritative for non-E13 expression families, qualified
   arenas, original final owners, `HirName`, known-family poison, and retained
   Try rows.

The two rejected Select archives are transport/history evidence only. Their
usable conclusions are reproduced in the current repository request and the
intakes. They supply no independent schema, source map, diagnostic key, limit,
or compatibility authority.

## Directly audited archives

| Archive | Bytes | SHA-256 | Members | Result |
|---|---:|---|---:|---|
| leaf/expression | 64,523 | `61e2ee166bff158fe83dcf1484b7b9380a81f60d865377503400d27d238cc708` | 20 | retained subject to later corrections |
| source owner | 91,023 | `2bcd3f78efb76442c2698a24251c4d874f7a941c5a8985649ea157100908a72e` | 24 | accepted |
| synthetic role parent | 33,968 | `a9603b3cc758d95dada69310f87a2dc26b7a2ce0ea8b6e0de39de4aa51e75024` | 18 | historical parent; retained only as incorporated by tail |
| tail/generator | 50,036 | `69dc42fc7c985fed638d08d694ed301291a50af3cefa7117321d4219be7e6471` | 23 | accepted |

The 67 leaf/source/tail rows were opened and hashed directly during the
immediately preceding E13 continuation and are carried forward byte-for-byte.
The new request explicitly allows prior analysis as working material, and the
2026-07-30 intake independently confirms all 67 rows. The synthetic-role ZIP
was fetched from the repository blob, opened directly, CRC-checked, and all 18
members were rehashed for this replacement. `PREDECESSOR_MEMBER_AUDIT.tsv`
contains every actual member, including each manifest.
