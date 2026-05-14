# Audio / BGM / TTS example

```awft
pub audio bus @bus.master { volume = 1.0 }
pub audio bus @bus.bgm parent @bus.master { volume = 0.8 }
pub audio bus @bus.voice parent @bus.master { volume = 1.0 }

pub cue @cue.voice.alice.001 from "audio/voice/alice/001.ogg" {
    bus = @bus.voice
    character = @character.alice
    transcript = "おはよう。"
}

pub bgm @bgm.alice_theme {
    bus = @bus.bgm
    stem @stem.piano from "audio/bgm/alice/piano.ogg"
    stem @stem.strings from "audio/bgm/alice/strings.ogg"

    section @music.intro { stems = [@stem.piano]; loop = 0s..16s }
    section @music.main { stems = [@stem.piano, @stem.strings]; loop = 16s..64s }

    transition @music.intro -> @music.main { quantize = bar; crossfade = 2bars }
}

pub voice profile @voice.alice.tts {
    character = @character.alice
    provider = "local"
    language = "ja-JP"
    style = "soft"
}

flow @flow.voice_demo demo(state: GameState) -> Result<FlowExit, FlowError> {
    command audio.ensure_bgm(@bgm.alice_theme) { section = @music.intro; fade_in = 1s }

    let speech = try await tts.speak(@voice.alice.tts, "おはよう。") with {
        pending p => scene @scene.loading_voice { progress p.ratio }
    }

    play voice speech.audio
    say alice "おはよう。"
    Ok(FlowExit::Done)
}
```

