# Native/Web/Agent host JSON

All hosts serialize the core `RuntimeStreamOpenRequest` directly. There is no
native DTO, Web DTO, Agent DTO, endpoint conversion helper, or compatibility
shape. The three adapters must produce byte-identical canonical JSON for equal
requests.

## 1. Top-level shape

```json
{
  "kind": "open_stream",
  "definition": "64-lowercase-hex",
  "declaration": "64-lowercase-hex",
  "generation": "decimal-u64",
  "instance": "decimal-u64",
  "signature": "64-lowercase-hex",
  "capability": "typed capability id",
  "operation": "typed operation id",
  "arguments": {
    "completed_groups": 3,
    "coordinates": [
      { "group": 0, "parameter": 0 },
      { "group": 1, "parameter": 0 },
      { "group": 2, "parameter": 0 }
    ],
    "values": [
      {
        "kind": "explicit",
        "type_layout": "64-lowercase-hex",
        "digest": "64-lowercase-hex",
        "value": { "kind": "string", "value": "room-7" }
      },
      {
        "kind": "defaulted",
        "default": "64-lowercase-hex",
        "type_layout": "64-lowercase-hex",
        "digest": "64-lowercase-hex",
        "value": { "kind": "u64", "value": "50" }
      },
      {
        "kind": "rest_named",
        "value_type_layout": "64-lowercase-hex",
        "entries": [
          {
            "name": "locale",
            "type_layout": "64-lowercase-hex",
            "digest": "64-lowercase-hex",
            "value": { "kind": "string", "value": "ja-JP" }
          }
        ]
      }
    ]
  },
  "policy": {}
}
```

`policy` is the unchanged strict Lang-01.3.1.2.1 `StreamPolicy` JSON object. This
correction neither adds nor removes a policy field; each host must invoke the
same parent-owned decoder for it. The empty object in package fixtures is a
policy stub used only by the correction-field validator, not a newly selected
production default.

## 2. Argument-value shapes

### Explicit

```json
{
  "kind": "explicit",
  "type_layout": "64-lowercase-hex",
  "digest": "64-lowercase-hex",
  "value": { "kind": "..." }
}
```

### Defaulted

```json
{
  "kind": "defaulted",
  "default": "64-lowercase-hex",
  "type_layout": "64-lowercase-hex",
  "digest": "64-lowercase-hex",
  "value": { "kind": "..." }
}
```

### Omitted optional

```json
{ "kind": "omitted_optional" }
```

No `null` payload is accepted.

### Positional rest

```json
{
  "kind": "rest_positional",
  "item_type_layout": "64-lowercase-hex",
  "items": [
    {
      "type_layout": "64-lowercase-hex",
      "digest": "64-lowercase-hex",
      "value": { "kind": "..." }
    }
  ]
}
```

Item order is authored source order.

### Named rest

```json
{
  "kind": "rest_named",
  "value_type_layout": "64-lowercase-hex",
  "entries": [
    {
      "name": "canonical-name",
      "type_layout": "64-lowercase-hex",
      "digest": "64-lowercase-hex",
      "value": { "kind": "..." }
    }
  ]
}
```

Names are unique and strictly increasing by raw UTF-8 bytes.

## 3. Strict decoding

All correction-owned object types use typed deserialization equivalent to
`#[serde(deny_unknown_fields)]`. Duplicate fields are errors. Unknown enum tags,
unknown fields, missing fields, wrong JSON scalar kinds, mixed-case/non-64-byte
hex, noncanonical decimal strings, and trailing input are errors.

A decoder validates into temporary owned data and validates the complete product
before making the request visible to a host provider. A rejected request causes no
provider call and no host-side instance registration.

The strictness boundary includes:

- top-level request fields;
- `arguments` fields;
- coordinate fields;
- every value variant;
- every rest item/entry; and
- the parent `StreamPolicy` object.

## 4. Integer policy

- `group`, `parameter`, and `completed_groups` are bounded structural integers and
  are JSON numbers in `0..=65535`.
- generation, instance, sequence, counters, capacities, `u64`, `u128`, `usize`,
  `i64`, `i128`, and `isize` runtime values use canonical base-10 strings.
- zero is `"0"`.
- no leading plus, no leading zero except zero itself, no whitespace, no exponent,
  and no decimal point are accepted.
- negative signed values have one leading `-`; `"-0"` is rejected.

This preserves exact integer behavior in JavaScript/Web hosts.

## 5. Canonical JSON byte order

Canonical writers emit UTF-8 with no insignificant whitespace and fields in this
order:

```text
request: kind, definition, declaration, generation, instance, signature,
         capability, operation, arguments, policy
arguments: completed_groups, coordinates, values
coordinate: group, parameter
explicit: kind, type_layout, digest, value
defaulted: kind, default, type_layout, digest, value
omitted_optional: kind
rest_positional: kind, item_type_layout, items
rest_named: kind, value_type_layout, entries
checked item/entry: [name,] type_layout, digest, value
```

Runtime payload object order remains the existing canonical `RuntimePayload`
order. Object key order is not used to validate semantic identity, but canonical
writers must use this order for byte parity and fingerprints over serialized host
fixtures.

## 6. Parity rule

For one checked core request:

```text
native_json(request) == web_json(request) == agent_json(request)
```

The adapters may transport those bytes differently, but they do not rename,
flatten, drop, regroup, or reconstruct arguments. Provider-specific acquisition
and I/O occur after strict decode and do not change the request contract.
