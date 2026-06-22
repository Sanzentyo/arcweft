class ArcweftMicrophoneCaptureProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.pending = [];
    this.pendingSamples = 0;
    this.flushSamples = 1024;
  }

  process(inputs, outputs) {
    const input = inputs[0];
    const output = outputs[0];
    if (!input || input.length === 0) {
      return true;
    }

    const channels = input.length;
    const frames = input[0].length;
    const interleaved = new Float32Array(frames * channels);
    for (let frame = 0; frame < frames; frame += 1) {
      for (let channel = 0; channel < channels; channel += 1) {
        interleaved[frame * channels + channel] = input[channel][frame] || 0;
      }
    }
    this.pending.push(interleaved);
    this.pendingSamples += interleaved.length;

    for (const channel of output) {
      channel.fill(0);
    }

    if (this.pendingSamples >= this.flushSamples) {
      const joined = new Float32Array(this.pendingSamples);
      let offset = 0;
      for (const block of this.pending) {
        joined.set(block, offset);
        offset += block.length;
      }
      this.pending.length = 0;
      this.pendingSamples = 0;
      this.port.postMessage(joined, [joined.buffer]);
    }
    return true;
  }
}

registerProcessor(
  "arcweft-microphone-capture",
  ArcweftMicrophoneCaptureProcessor,
);
