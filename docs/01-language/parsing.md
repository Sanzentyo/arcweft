# 入力パース

外部入力は必ず `Parser<T, ParseError>` を通す。

```awft
pub type Parser<T, E>
```

parse は `Result<T, ParseError>` を返す。

```awft
fn parse<T>(parser: Parser<T, ParseError>, input: String) -> Result<T, ParseError>
```

## Player command parser

```awft
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
            PlayerCommand::Choose { id },

        "advance" =>
            PlayerCommand::Advance,

        "settings" =>
            PlayerCommand::OpenSettings,
    }
}
```

## Agent script parser

```awft
pub parser parse_agent_script: Parser<List<AgentScriptCommand>, ParseError> {
    many line {
        alt {
            "observe" => AgentScriptCommand::Observe,
            "choose" ws target: ref_id<ChoiceOption>() =>
                AgentScriptCommand::Choose { target },
            "wait signal" ws sig: ref_id<Signal>() ws op: compare_op() ws value: value() =>
                AgentScriptCommand::WaitSignal { signal: sig, op, value },
        }
    }
}
```

## ref_id<T>()

```awft
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

```awft
pub struct ParseError {
    pub span: TextRange,
    pub expected: List<ExpectedToken>,
    pub found: Option<String>,
    pub message: String,
    pub recovery: List<RecoverySuggestion>,
}
```

## zero-copy parser

```awft
pub parser parse_image_header<'a>: Parser<ImageHeader<'a>, ParseError>
input &'a [u8]
ensures result.is_ok() => result.unwrap().width > 0
{
    ...
}
```

borrow は await/yield を跨げない。


