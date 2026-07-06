# 入力パース

外部入力は必ず `Parser<T, ParseError>` を通す。

```arcw
pub type Parser<T, E>
```

parse は `Result<T, ParseError>` を返す。

```arcw
fn parse<T>(parser: Parser<T, ParseError>, input: String) -> Result<T, ParseError>
```

## Player command parser

```arcw
pub enum PlayerCommand {
    Choose { id: Ref<ChoiceOption> },
    Advance,
    OpenSettings,
}

pub parser parse_player_command: Parser<PlayerCommand, ParseError>
ensures result.is_ok() => result.unwrap().is_allowed_in_current_flow()
ensures result.is_err() => result.err().span.len() > 0
{
    alt {
        "choose" ws id: ref_id<ChoiceOption>() =>
            PlayerCommand.Choose { id },

        "advance" =>
            PlayerCommand.Advance,

        "settings" =>
            PlayerCommand.OpenSettings,
    }
}
```

## Agent script parser

```arcw
pub parser parse_agent_document: Parser<Document, ParseError> {
    parse_document(dialect = SourceDialect.agent)
}

agent @agent.smoke effects { agent.observe } {
    let frame = try observe()
    expect(frame.actions.len().ge(0), "observation is available")
    return "done"
}
```

## ref_id<T>()

```arcw
pub parser ref_id<T>: Parser<Ref<T>, ParseError>
where T: EntityKind
```

入力例:

```text
choice.opening.listen
@choice.opening.listen
@<choice.opening.listen@sem:b3_9f2a1c>
```

## ParseError

```arcw
pub struct ParseError {
    pub span: TextRange,
    pub expected: Vec<ExpectedToken>,
    pub found: Option<String>,
    pub message: String,
    pub recovery: Vec<RecoverySuggestion>,
}
```

## zero-copy parser

```arcw
pub parser parse_image_header<'a>: Parser<ImageHeader<'a>, ParseError>
input &'a [u8]
ensures result.is_ok() => result.unwrap().width > 0
{
    ...
}
```

borrow は await/yield を跨げない。




