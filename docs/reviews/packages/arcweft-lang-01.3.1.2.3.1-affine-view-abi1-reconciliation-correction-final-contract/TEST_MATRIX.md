# Correction test matrix

The machine-readable matrix contains **234** unique rows.

| Group | Rows | Scope |
|---|---:|---|
| `ABI` | 24 | corrected ABI contract |
| `ACT` | 28 | corrected ACT contract |
| `ALC` | 24 | corrected ALC contract |
| `ATM` | 10 | corrected ATM contract |
| `DEL` | 10 | corrected DEL contract |
| `DRP` | 18 | corrected DRP contract |
| `FRG` | 22 | corrected FRG contract |
| `INT` | 16 | corrected INT contract |
| `REQ` | 28 | corrected REQ contract |
| `SNP` | 12 | corrected SNP contract |
| `VOW` | 42 | corrected VOW contract |

Every row is additive to retained parent tests except where `SUPERSESSION_MATRIX.md` changes an expected ABI number, schema field, or old invalid API. Production execution is mandatory before implementation completion; no row is claimed green by this design archive.
