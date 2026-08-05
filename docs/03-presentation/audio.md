# Audio / Spatial / TTS / BGM

音声は後付けの再生APIではなく、`AudioGraph` と `AudioDesiredState` として構造化する。

## 目的

- SE / voice / BGM の通常再生。
- BGM loop、stem、adaptive music、ducking、snapshot mix。
- 立体音響、listener、spatial source。
- 自動読み上げ API。
- BGM 作成 / precompose / procedural cue。
- Agent / test / bench から観測可能。
- native / web 両対応。

## crate

```text
arcweft-audio-core
  AudioCommand, AudioDesiredState, AudioGraph, Cue, Bus

arcweft-audio-device-cpal
  CPAL adapter for native/Web audio input and output

arcweft-audio-mixer
  mixer, bus, send, ducking, loudness, snapshots

arcweft-audio-spatial
  listener, source, panner, HRTF/binaural adapter

arcweft-audio-tts
  TtsProvider, VoiceProfile, SpeechRequest, viseme/phoneme events

arcweft-audio-bgm
  BGM cue, stem, adaptive music, loop, transition, music state

arcweft-audio-authoring
  BGM creation DSL, MIDI-like events, pattern, arrangement, pre-render

arcweft-audio-kira
  native backend adapter

arcweft-audio-web
  WebAudio backend adapter
```

## Audio device I/O

Arcweft uses CPAL as the primary low-level audio I/O backend. CPAL is wrapped by `arcweft-audio-device-cpal`, so the DSL never opens devices directly.

```text
Native:
  CPAL -> input/output streams -> Arcweft mixer/capture graph

Web:
  browser permission/device bootstrap via web-sys MediaDevices
  CPAL wasm-bindgen/WebAudio path where viable
  AudioWorklet path for low latency when atomics and deployment headers are available
```

Microphone input is treated as a capture source and is specified in [Microphone / Camera Capture Devices](capture-devices.md).

## AudioDesiredState

Audio は imperative に鳴らすのではなく、できるだけ desired state と command を併用する。

```rust
pub struct AudioDesiredState {
    pub bgm: Option<BgmDesiredState>,
    pub ambience: Vec<AmbienceDesiredState>,
    pub spatial_sources: Vec<SpatialSourceDesiredState>,
    pub mixer_snapshot: MixerSnapshotRef,
}
```

瞬間的な音は command。

```rust
pub enum AudioCommand {
    EnsureBgm(BgmCommand),
    StopBgm { id: StableAudioId, fade: Duration },
    PlayOneShot { event_id: EventId, cue: Ref<Cue>, volume: f32 },
    PlayVoice { line: Ref<Say>, cue: Ref<Cue>, sync: VoiceSyncPolicy },
    SetMixerSnapshot { snapshot: Ref<MixerSnapshot>, transition: Duration },
    SetListener(ListenerTransform),
    UpdateSpatialSource(SpatialSourceCommand),
    SynthesizeSpeech(TtsRequest),
}
```

`PlayOneShot` は `event_id` で重複再生を防ぐ。

## Cue

```arcw
pub cue @cue.se.click from "audio/se/click.ogg" {
    bus = @bus.se
    loudness = -18 LUFS
}

pub cue @cue.voice.alice.001 from "audio/voice/alice/001.ogg" {
    bus = @bus.voice
    character = @character.alice
    transcript = "おはよう。"
    subtitle = @say.opening.greeting
}
```

## Mixer / Bus

```arcw
pub audio bus @bus.master { volume = 1.0 }
pub audio bus @bus.bgm parent @bus.master { volume = 0.8 }
pub audio bus @bus.voice parent @bus.master { volume = 1.0 }
pub audio bus @bus.se parent @bus.master { volume = 0.9 }
```

Mixer snapshot:

```arcw
pub mixer snapshot dialogue {
    @bus.bgm.volume = -8db over 300ms
    @bus.voice.volume = 0db
}

pub mixer snapshot normal {
    @bus.bgm.volume = 0db over 600ms
}
```

Ducking:

```arcw
pub ducking voice_over_bgm {
    trigger = @bus.voice
    target = @bus.bgm
    amount = -6db
    attack = 120ms
    release = 500ms
}
```

## BGM 再生

```arcw
pub bgm alice_theme {
    bus = @bus.bgm
    stem @stem.piano from "audio/bgm/alice/piano.ogg"
    stem @stem.strings from "audio/bgm/alice/strings.ogg"

    section @music.intro {
        stems = [@stem.piano]
        loop = 0s..16s
    }

    section @music.main {
        stems = [@stem.piano, @stem.strings]
        loop = 16s..64s
    }

    transition @music.intro -> @music.main {
        quantize = bar
        crossfade = 2bars
    }
}
```

使用:

```arcw
let theme = bgm(@bgm.alice_theme, section = @music.intro, fade_in = 1s)
theme.section(@music.main)
```

## Adaptive music

```arcw
pub music state alice_theme {
    intensity: f32 = 0.0
    danger: bool = false
}

pub adaptive bgm truck_chase {
    stem @stem.base from "audio/bgm/truck/base.ogg"
    stem @stem.drums from "audio/bgm/truck/drums.ogg"
    stem @stem.danger from "audio/bgm/truck/danger.ogg"

    rule {
        if state.intensity > 0.5 { stem.enable(@stem.drums, fade=2bars) }
        if state.danger { stem.enable(@stem.danger, fade=1bar) }
    }
}
```

Activity からは `MusicStateUpdate` を出す。

```arcw
EffectRequest::Audio(AudioCommand::SetMusicState {
    bgm: @bgm.truck_chase,
    key: "intensity",
    value: result.speed_ratio,
})
```

## BGM 作成 / authoring

BGM を完全にAI生成する前提ではなく、まずはゲーム内で使える「作曲データ・編曲データ・ループ/ステム定義」を扱う。

```arcw
pub music pattern soft_piano {
    tempo = 92bpm
    key = A_minor
    meter = 4/4

    track piano instrument @instrument.piano {
        bar 1: chord Am arp up velocity 64
        bar 2: chord F  arp up velocity 60
        bar 3: chord C  arp up velocity 60
        bar 4: chord G  arp up velocity 62
    }
}

pub bgm generated.alice_theme compose {
    use pattern @music.pattern.soft_piano
    arrange {
        intro bars 1..4
        loop main bars 1..16
        outro bars 17..20
    }
    render target = "audio/bgm/generated/alice_theme.ogg"
}
```

生成系は `Need<Result<AudioHandle, AudioError>, TaskError>`。

```arcw
let bgm = try await compose_bgm(@bgm.generated.alice_theme) with {
    pending p => { scene.show(@scene.loading_audio); text.show("BGMを生成中"); progress.set(p.ratio) }
}
```

## TTS / 自動読み上げ

TTS は `SpeechRequest` として扱い、同期イベントを返す。

```arcw
pub voice profile @voice.alice.tts {
    character = @character.alice
    provider = "local"      // local / web / external / product-config
    language = "ja-JP"
    style = "soft"
    pitch = 1.05
    rate = 0.95
}
```

読み上げ:

```arcw
let speech = try await tts.speak(@voice.alice.tts, "おはよう。") with {
    pending p => scene.show(@scene.loading_voice); progress.set(p.ratio)
}

voice(speech.audio, speaker = alice)
```

`TtsResult`:

```rust
pub struct TtsResult {
    pub audio: AudioHandle,
    pub transcript: String,
    pub phonemes: Vec<PhonemeSpan>,
    pub visemes: Vec<VisemeSpan>,
    pub duration: Duration,
}
```

字幕・口パク・表情同期に使う。

```arcw
alice.say(voice=@voice.alice.tts)[おはよう。[p]]
with {
    fallback = subtitle_only
    cache = true
}
```

## Spatial audio

```arcw
pub audio listener @listener.main {
    position = camera.position
    forward = camera.forward
    up = vec3(0, 1, 0)
}

pub spatial source @audio_source.truck_engine {
    cue = @se.truck_engine_loop
    position = truck.position
    radius = 2.0
    attenuation = inverse_square
    doppler = true
    bus = @bus.se
}
```

2Dノベルゲームでも、左右の立ち位置に応じた panning を使える。

```arcw
voice(@voice.alice.opening.001, speaker = alice, spatial = true) {
    position = character_position(@character.alice).to_audio_pos()
    mode = screen_space
}
```

## Web / Native 差分

Native:

- mixer backend: Kira/CPAL adapter 等。
- spatial: engine side panner / optional HRTF。
- TTS: local/provider/plugin。

Web:

- WebAudio backend。
- audio unlock が必要。
- TTS は browser SpeechSynthesis または external/provider だが、製品では権限とキャッシュを考慮。

## Contract

```arcw
pub cue @cue.voice.alice.001 from "audio/voice/alice/001.ogg"
requires duration <= 10s
ensures loudness in -24LUFS..-14LUFS
```

BGM:

```arcw
pub bgm alice_theme
ensures all sections have loop_points
ensures no stem clips
```

## Logging / Signal

```arcw
pub signal current_bgm: Watch<Ref<Bgm>>
pub signal audio_bus_levels: Watch<OrderedMap<Ref<AudioBus>, f32>>
pub signal tts_progress: Watch<f32>

log.info(
    "bgm section changed {bgm:?} -> {section:?}",
    bgm = @bgm.alice_theme,
    section = @music.main,
)
```

## Test

```arcw
test @test.bgm_loop_points audio {
    audio.render(@bgm.alice_theme, section=@music.main, duration=64bars)
    assert(no_clicks_at_loop())
    assert((-24LUFS..-14LUFS).contains(loudness))
}

test @test.tts_alice_subtitle_sync audio {
    let tts = try tts.synthesize_now(@voice.alice.tts, "おはよう。")
    assert(tts.duration > 0s)
    assert(tts.transcript == "おはよう。")
    assert(tts.visemes.len() > 0)
}
```

## Agent / MCP

```bash
arcw agent audio state --json
arcw agent audio set-bus bus.bgm -6db
arcw agent audio mute bus.voice
arcw agent audio render-bgm bgm.alice_theme --out alice_theme.wav
arcw agent tts preview voice.alice.tts "おはよう" --out voice.wav
```

MCP tools:

```text
arcweft.audio_state
arcweft.audio_set_bus
arcweft.audio_render_bgm
arcweft.tts_preview
arcweft.audio_wait_until_finished
```

## 実装方針

1. `arcweft-audio-core` で型と command を定義。
2. `arcweft-audio-mixer` で bus/snapshot/ducking を実装。
3. `arcweft-audio-bgm` で loop/stem/adaptive music を実装。
4. `arcweft-audio-tts` で provider trait と cache を実装。
5. native/web backend を adapter として分離。
6. Agent/test/bench から観測できるように signal/metric を出す。

