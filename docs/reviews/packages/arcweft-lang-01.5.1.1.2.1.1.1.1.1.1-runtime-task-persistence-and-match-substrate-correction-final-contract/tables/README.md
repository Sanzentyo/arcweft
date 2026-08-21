# CSV table equivalents

Each CSV is generated from the corresponding `machine/*.json` row set. List and
map cells use compact JSON, not a second ad-hoc delimiter grammar.

| CSV | JSON authority |
|---|---|
| `producer_execution_truth_table.csv` | `machine/producer_execution_truth_table.json` |
| `ownership_matrix.csv` | `machine/ownership_matrix.json` |
| `persistence_schemas.csv` | `machine/persistence_schemas.json` |
| `expression_transcripts.csv` | `machine/expression_transcripts.json` |
| `literal_transcripts.csv` | `machine/expression_transcripts.json` |
| `pattern_transcripts.csv` | `machine/pattern_transcripts.json` |
| `deletion_matrix.csv` | `machine/deletion_matrix.json` |
| `tests.csv` | `machine/tests.json` |
| `source_evidence.csv` | `machine/source_evidence.json` |
| `compile_cuts.csv` | `machine/compile_cuts.json` |
| `owner_api_map.csv` | `machine/owner_api_map.json` |
| `requirement_traceability.csv` | `machine/traceability.json` |

The validator reconstructs normalized rows from each CSV and compares them with
the JSON authority. A missing row, duplicate key, reordered source-order table,
or altered cell fails validation.
