# Typed Input Decoding

Arcweft does not define a special top-level parser declaration or a parser-only
expression grammar. Input decoding is ordinary typed computation: a function
accepts an input value and returns `Result<T, E>`, while `match`, typed cursor
APIs, and standard codecs perform the work.

This keeps function signatures, contracts, effects, HIR, tooling, and runtime
lowering on the normal function path.

## Small command languages

Exact commands use an ordinary function and `match`:

```arcw
pub enum PlayerCommand {
    Advance,
    OpenSettings,
}

pub fn decode_player_command(input: String)
    -> Result<PlayerCommand, ParseError>
{
    match input.trim() {
        "advance" => Ok(.Advance)
        "settings" => Ok(.OpenSettings)
        _ => Err(ParseError.unknown_command(input))
    }
}
```

More structured text can use a standard `TextCursor` from an ordinary
function. Binary protocols use a `ByteCursor`. Cursor APIs own byte/character
boundaries, end-of-input checks, resource limits, and error spans; branching
still uses ordinary control flow.

```arcw
pub fn decode_sensor_frame(bytes: &[u8])
    -> Result<SensorFrame, UsbParseError>
{
    let cursor = ByteCursor.new(bytes)
    let version = cursor.read_u8()?
    let value = cursor.read_i16_le()?
    let flags = cursor.read_u16_le()?
    cursor.expect_end()?

    Ok(SensorFrame { version, value, flags })
}
```

## Standard data formats

JSON, TOML, YAML, CSV, and other standard formats use typed codec APIs rather
than author-defined parser syntax:

```arcw
let config = data.decode(bytes, .Json, data.shape<Config>())?
```

Compiler source parsing, Agent protocol parsing, and CLI command parsing remain
owned by their compiler or tooling crates. An Arcweft project does not redeclare
those parsers in source.

## Errors

Parse errors are ordinary structured error values:

```arcw
pub struct ParseError {
    pub span: TextRange,
    pub expected: Vec<ExpectedToken>,
    pub found: Option<String>,
    pub message: String,
    pub recovery: Vec<RecoverySuggestion>,
}
```

Reusable cursor and codec APIs produce consistent spans and expected-token
information. Decoders must check trailing input, enforce input-size limits, and
avoid retaining a borrow beyond the input lifetime.

## Final rules

1. Decoding is an ordinary `fn -> Result<T, E>`.
2. Value branching uses ordinary `match` and patterns.
3. Text and byte consumption use typed library APIs, not a second language.
4. Standard formats use the standard codec surface.
5. Specialized compiler/tool parsers stay in their owning crates.
