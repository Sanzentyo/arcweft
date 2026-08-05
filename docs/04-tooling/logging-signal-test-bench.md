# logging / signal / assert / test / bench

## Deferred structured logging

defmt 風に、format string を compile-time intern し、実行時は template id と typed args だけを送る。

```arcw
log.info("selected choice {choice:?}", choice = selected.id)
```

LogFrame:

```arcw
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

```arcw
pub signal current_flow: Watch<Ref<Flow>>
pub signal loading_progress: Watch<f32>
pub signal choice_visible: Watch<Vec<Ref<ChoiceOption>>>
pub metric gauge frame_time_ms: f32
```

更新:

```arcw
signal.set(@signal.current_flow, @flow.opening)
signal.set(@signal.loading_progress, p.ratio)
metric.set(@metric.frame_time_ms, frame_time.ms())
```

Signal kind:

```arcw
Watch     最新値
Stream    全イベント
Counter   カウンタ
Gauge     現在値
Sample    サンプル列
```

## assert

```arcw
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
`fixture`. Parser/HIR support treats the body as canonical call-based test plan
statements and exposes it through the Sans I/O test manifest; actual execution
is delegated to headless/player adapters.

```arcw
test @test.opening_listen_route scenario {
    goto @flow.opening

    expect.log(.info, contains="enter flow")
    expect.signal(@signal.current_flow, @flow.opening)

    wait.object(@choice.opening.listen, state=.visible)
    choose(@choice.opening.listen)

    expect.signal(@signal.current_flow, @flow.alice_intro)
    expect.no_assertion_failures()
}
```

Visual test:

```arcw
test @test.opening_choices_visual visual {
    goto @flow.opening
    wait.object(@choice.opening.listen, state=.visible)
    capture.image(.overlay, path="opening_choices.png")
    assert.bbox(@choice.opening.listen, within=rect(400, 500, 400, 80))
}
```

## Bench

`bench` is a top-level declaration. The canonical form is:

```text
bench ID { ... }
```

Bench bodies use structured sections such as `setup`, `measure`, `assert`,
and `report`. The language layer preserves these sections for tooling; timing,
offline rendering/audio, allocation counters, and perf collection are backend
adapter responsibilities.

```arcw
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

```arcw
bench @bench.bgm_mix_120s {
    setup { audio.play(@bgm.alice_theme, section=@music.main) }
    measure duration = 120s { audio.render_offline() }
    report { cpu_time, peak_buffer_bytes, loudness }
}
```



## Dispatch and subsystem-cache observation

Tooling observes owner-local routed events and subsystem-specific cache metrics.

CLI:

```bash
arcw trace input --target choice.opening.listen
arcw cache stats --owner view
```

Test:

```arcw
test @test.choice_action_routes scenario {
    goto @flow.opening
    wait.object(@choice.opening.listen, state=.visible)
    act.click(@choice.opening.listen)
    expect.action(@action.choice.select, target=@choice.opening.listen)
}
```

Bench:

```arcw
bench @bench.view_cache_hit_rate {
    measure iterations = 10000 {
        opening_choices().map(choice_to_view(state)).collect<Vec<ChoiceView>>()
    }
    assert(metric.value(@metric.view_cache_hit_rate) >= 0.95)
}
```


