# logging / signal / assert / test / bench

## Deferred structured logging

defmt 風に、format string を compile-time intern し、実行時は template id と typed args だけを送る。

```awft
log.info("selected choice {choice:?}", choice = selected.id)
```

LogFrame:

```awft
pub struct LogFrame {
    pub tick: TickId,
    pub monotonic_ns: u64,
    pub level: LogLevel,
    pub template_id: LogTemplateId,
    pub module: ModuleId,
    pub entity: Option<EntityId>,
    pub span: Option<SpanId>,
    pub args: PackedArgs,
}
```

## Signal

```awft
pub signal @signal.current_flow: Watch<Ref<Flow>>
pub signal @signal.loading_progress: Watch<f32>
pub signal @signal.choice_visible: Watch<Vec<Ref<ChoiceOption>>>
pub metric gauge @metric.frame_time_ms: f32
```

更新:

```awft
signal.set(@signal.current_flow, @flow.opening)
signal.set(@signal.loading_progress, p.ratio)
metric.set(@metric.frame_time_ms, frame_time.ms())
```

Signal kind:

```awft
Watch     最新値
Stream    全イベント
Counter   カウンタ
Gauge     現在値
Sample    サンプル列
```

## assert

```awft
assert(state.affection[@character.alice] >= 0)
assert_eq(route_title(@flow.opening), "Opening")
debug_assert(choices.len() > 0)
assert_ok(load_config())
assert_some(state.current_bg)
```

`assert(...)` は常に有効な runtime assertion として扱う。
`debug_assert(...)` は debug/test profile でのみ強制される assertion として
runtime plan に残す。失敗時は `AssertionEvent` として
log/signal/trace/crash bundle に流す。

## Test

`test` is a top-level declaration. The canonical form is:

```text
test ID KIND { ... }
```

`KIND` is a test adapter category such as `scenario`, `visual`, `audio`, or
`fixture`. Parser/HIR support treats the body as command-like test plan rows and
exposes it through the Sans I/O test manifest; actual execution is delegated to
headless/player adapters.

```awft
test @test.opening_listen_route scenario {
    start @flow.opening

    expect log.info contains "enter flow"
    expect signal @signal.current_flow == @flow.opening

    wait object @choice.opening.listen visible
    choose @choice.opening.listen

    expect signal @signal.current_flow eventually == @flow.alice_intro
    expect no_assertion_failures
}
```

Visual test:

```awft
test @test.opening_choices_visual visual {
    start @flow.opening
    wait object @choice.opening.listen visible
    capture image overlay as "opening_choices.png"
    assert_bbox @choice.opening.listen within rect(400, 500, 400, 80)
}
```

## Bench

`bench` is a top-level declaration. The canonical form is:

```text
bench ID { ... }
```

Bench bodies use command-like sections such as `setup`, `measure`, `assert`,
and `report`. The language layer preserves these sections for tooling; timing,
offline rendering/audio, allocation counters, and perf collection are backend
adapter responsibilities.

```awft
bench @bench.opening_pipeline {
    setup { let state = fixture<GameState>("states/opening.json") }

    measure iterations = 10_000 {
        black_box(opening_choices())
            .filter(choice_available(state))
            .map(choice_to_view(state))
            .collect<Vec<ChoiceView>>()
    }

    assert(metric.allocations <= 2)
}
```

Audio bench:

```awft
bench @bench.bgm_mix_120s {
    setup { play @bgm.alice_theme section @music.main }
    measure duration = 120s { render_audio_offline }
    report { cpu_time, peak_buffer_bytes, loudness }
}
```



## Hook / memo observation

hook と memo は logging / signal / test / bench の対象である。

CLI:

```bash
arcw hook list
arcw hook trace --target choice.opening.listen
arcw memo stats
arcw memo invalidate --entity flow.opening
```

Test:

```awft
test @test.choice_hook_fires scenario {
    start @flow.opening
    wait object @choice.opening.listen visible
    expect hook @hook.opening.choice_visible fired
    expect signal @signal.choice_visible == true
}
```

Bench:

```awft
bench @bench.memo_hit_rate {
    measure iterations = 10000 {
        opening_choices().map(choice_to_view(state)).collect<Vec<ChoiceView>>()
    }
    assert(metric.value(@metric.memo_hit_rate) >= 0.95)
}
```

