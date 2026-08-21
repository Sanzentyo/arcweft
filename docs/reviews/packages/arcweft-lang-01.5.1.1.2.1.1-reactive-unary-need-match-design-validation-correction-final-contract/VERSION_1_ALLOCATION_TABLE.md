# Version-1 allocation and identity table

| Owner | Marker/value | Allocation/order | Persistence | Equality domain |
|---|---:|---|---|---|
| AWBC ABI | 1 | existing header | bundle | AWBC program digest |
| ViewProgram transcript | 1 | existing AWFB field | bundle | accepted program revision |
| Need subscription | 1 | table per program | bundle | revision + local ID |
| Need snapshot | 1 | session subrecord | save | snapshot content root |
| Need replay | 1 | ordered journal | replay | session/generation |
| Need replacement | 1 | candidate transaction | evidence | old/new revisions |
| subscription local ID | nonzero u32 | canonical node order | bundle | one revision |
| semantic ID | 32 bytes | domain-separated semantic hash | bundle/save/generated | semantic contract |
| contract digest | 32 bytes | types/producer/policy/selector | bundle/save/replacement | byte equality |
| producer generation | existing u64 | replacement owner | save/replay | generation |
| NeedId | existing typed identity | verified NeedHandle | runtime/save/replay | generation + value |
| cursor | epoch + sequence | task publication owner | save/replay | one journal |
| observer | mount + subscription | bind transaction | runtime/save | live mount |
| retained arm | observer + ordinal + arm digest | first materialization | runtime/save | arm contract |
| invalidation revision | nonwrapping u64 | per observer | runtime/save | observer |

No touched marker becomes 2. No legacy marker, compatibility bit, alias
discriminant, or optional old table is allocated.
