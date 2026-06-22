use arcweft_interaction_model::audio::MicrophoneConstraints;
use js_sys::{Float32Array, Object, Reflect};
use rtrb::{Consumer, Producer, PushError, RingBuffer};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioContext, AudioWorkletNode, AudioWorkletNodeOptions, GainNode, MediaStream,
    MediaStreamAudioSourceNode, MediaStreamConstraints, MessageEvent,
};

pub struct BrowserMicrophone {
    context: AudioContext,
    stream: MediaStream,
    source: MediaStreamAudioSourceNode,
    worklet: AudioWorkletNode,
    silent_output: GainNode,
    samples: Consumer<f32>,
    dropped_samples: Arc<AtomicU64>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
}

impl BrowserMicrophone {
    pub async fn request(
        constraints: MicrophoneConstraints,
        worklet_module_url: &str,
        sample_capacity: usize,
    ) -> Result<Self, BrowserMicrophoneError> {
        if sample_capacity == 0 {
            return Err(BrowserMicrophoneError::Configuration(
                "sample ring capacity must be non-zero".to_owned(),
            ));
        }
        let window = web_sys::window().ok_or(BrowserMicrophoneError::MissingWindow)?;
        let devices = window
            .navigator()
            .media_devices()
            .map_err(|error| BrowserMicrophoneError::MediaDevices(format!("{error:?}")))?;
        let audio = Object::new();
        Reflect::set(
            &audio,
            &JsValue::from_str("channelCount"),
            &JsValue::from_f64(f64::from(constraints.channels.max(1))),
        )
        .map_err(|error| BrowserMicrophoneError::Constraints(format!("{error:?}")))?;
        Reflect::set(
            &audio,
            &JsValue::from_str("echoCancellation"),
            &JsValue::from_bool(constraints.echo_cancellation),
        )
        .map_err(|error| BrowserMicrophoneError::Constraints(format!("{error:?}")))?;
        Reflect::set(
            &audio,
            &JsValue::from_str("noiseSuppression"),
            &JsValue::from_bool(constraints.noise_suppression),
        )
        .map_err(|error| BrowserMicrophoneError::Constraints(format!("{error:?}")))?;
        Reflect::set(
            &audio,
            &JsValue::from_str("autoGainControl"),
            &JsValue::from_bool(constraints.auto_gain_control),
        )
        .map_err(|error| BrowserMicrophoneError::Constraints(format!("{error:?}")))?;
        if let Some(rate) = constraints.preferred_sample_rate_hz {
            Reflect::set(
                &audio,
                &JsValue::from_str("sampleRate"),
                &JsValue::from_f64(f64::from(rate)),
            )
            .map_err(|error| BrowserMicrophoneError::Constraints(format!("{error:?}")))?;
        }
        let media_constraints = MediaStreamConstraints::new();
        media_constraints.set_audio(&audio.into());
        let media = devices
            .get_user_media_with_constraints(&media_constraints)
            .map_err(|error| BrowserMicrophoneError::Permission(format!("{error:?}")))?;
        let stream = JsFuture::from(media)
            .await
            .map_err(|error| BrowserMicrophoneError::Permission(format!("{error:?}")))?
            .dyn_into::<MediaStream>()
            .map_err(|error| BrowserMicrophoneError::Permission(format!("{error:?}")))?;

        let context = AudioContext::new()
            .map_err(|error| BrowserMicrophoneError::Context(format!("{error:?}")))?;
        let worklet = context
            .audio_worklet()
            .map_err(|error| BrowserMicrophoneError::Worklet(format!("{error:?}")))?;
        JsFuture::from(
            worklet
                .add_module(worklet_module_url)
                .map_err(|error| BrowserMicrophoneError::Worklet(format!("{error:?}")))?,
        )
        .await
        .map_err(|error| BrowserMicrophoneError::Worklet(format!("{error:?}")))?;
        let source = context
            .create_media_stream_source(&stream)
            .map_err(|error| BrowserMicrophoneError::Graph(format!("{error:?}")))?;
        let options = AudioWorkletNodeOptions::new();
        options.set_number_of_inputs(1);
        options.set_number_of_outputs(1);
        let node =
            AudioWorkletNode::new_with_options(&context, "arcweft-microphone-capture", &options)
                .map_err(|error| BrowserMicrophoneError::Worklet(format!("{error:?}")))?;
        source
            .connect_with_audio_node(&node)
            .map_err(|error| BrowserMicrophoneError::Graph(format!("{error:?}")))?;
        let silent_output = context
            .create_gain()
            .map_err(|error| BrowserMicrophoneError::Graph(format!("{error:?}")))?;
        silent_output.gain().set_value(0.0);
        node.connect_with_audio_node(&silent_output)
            .and_then(|_| silent_output.connect_with_audio_node(&context.destination()))
            .map_err(|error| BrowserMicrophoneError::Graph(format!("{error:?}")))?;

        let (producer, consumer) = RingBuffer::new(sample_capacity);
        let producer = Rc::new(RefCell::new(producer));
        let dropped_samples = Arc::new(AtomicU64::new(0));
        let callback_drops = Arc::clone(&dropped_samples);
        let callback_producer = Rc::clone(&producer);
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event| {
            let values = Float32Array::new(&event.data());
            let mut producer = callback_producer.borrow_mut();
            for index in 0..values.length() {
                if let Err(PushError::Full(_)) = producer.push(values.get_index(index)) {
                    callback_drops.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
        node.port()
            .set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        node.port().start();

        Ok(Self {
            context,
            stream,
            source,
            worklet: node,
            silent_output,
            samples: consumer,
            dropped_samples,
            _on_message: on_message,
        })
    }

    pub fn drain_samples(&mut self, output: &mut Vec<f32>, maximum: usize) {
        output.extend((0..maximum).map_while(|_| self.samples.pop().ok()));
    }

    pub fn dropped_sample_count(&self) -> u64 {
        self.dropped_samples.load(Ordering::Relaxed)
    }

    pub fn sample_rate_hz(&self) -> u32 {
        self.context.sample_rate() as u32
    }

    pub fn stop(&self) {
        for track in self.stream.get_tracks().iter() {
            if let Ok(track) = track.dyn_into::<web_sys::MediaStreamTrack>() {
                track.stop();
            }
        }
        let _ = self.context.close();
    }

    pub fn source(&self) -> &MediaStreamAudioSourceNode {
        &self.source
    }

    pub fn worklet(&self) -> &AudioWorkletNode {
        &self.worklet
    }

    pub fn silent_output(&self) -> &GainNode {
        &self.silent_output
    }
}

impl Drop for BrowserMicrophone {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Error)]
pub enum BrowserMicrophoneError {
    #[error("invalid browser microphone configuration: {0}")]
    Configuration(String),
    #[error("browser window is unavailable")]
    MissingWindow,
    #[error("MediaDevices is unavailable: {0}")]
    MediaDevices(String),
    #[error("failed to build microphone constraints: {0}")]
    Constraints(String),
    #[error("microphone permission or device request failed: {0}")]
    Permission(String),
    #[error("failed to create microphone AudioContext: {0}")]
    Context(String),
    #[error("failed to load or create microphone AudioWorklet: {0}")]
    Worklet(String),
    #[error("failed to build microphone WebAudio graph: {0}")]
    Graph(String),
}
