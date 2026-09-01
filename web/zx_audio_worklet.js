// The browser's audio device: a ring buffer drained by the audio thread.
//
// ## Why an AudioWorklet and not the bundle's own audio
//
// `web/mq_js_bundle.js` carries quad-snd's audio plugin, and its one entry point for sound data
// is `audio_add_buffer`, which is `decodeAudioData` — an **encoded file**. There is no call in
// it that accepts samples, and `macroquad`'s Rust side exposes none either (grepping
// `macroquad-0.4.16/src/audio.rs` for `f32`, `sample`, `frames` or `pcm` matches nothing). An
// emulator generates samples fifty times a second; it does not have a file. So the device is
// written here.
//
// ## Why a worklet and not a ScriptProcessorNode
//
// `ScriptProcessorNode` runs on the main thread, which is also where the emulator runs — so a
// slow frame would be a gap in the sound. An `AudioWorklet` runs on the audio thread and pulls
// from a queue at the device's own rate, which is what makes a stall a *late* frame rather than
// a *silent* one.
//
// ## Why samples are posted rather than shared
//
// The obvious design is a ring buffer in wasm memory that the worklet reads directly. It needs
// `SharedArrayBuffer`, which needs `Cross-Origin-Opener-Policy` and `Cross-Origin-Embedder-Policy`
// headers — and `docs/M8.md` Decision 8 rules deployment and its headers out of scope, on the
// grounds that "anything that can serve static files over HTTP serves it, and that is the whole
// contract". A design needing two response headers would quietly break that contract.
//
// `postMessage` with a transferred `ArrayBuffer` costs one copy out of wasm memory per frame —
// about 960 floats fifty times a second — and needs no headers at all. That is the trade, taken
// deliberately: a copy nobody will measure against a deployment constraint everybody would hit.

/// Process calls between reports of the backlog to the main thread.
///
/// The main thread needs the queue depth so Rust can see whether it is running ahead or behind,
/// and a message per `process` call would be ~375 a second for a number nobody reads that often.
const REPORT_EVERY = 16;

class ZxProcessor extends AudioWorkletProcessor {
    constructor() {
        super();
        /// Chunks handed over by the main thread, oldest first.
        this.chunks = [];
        /// How far into `chunks[0]` the device has read.
        this.offset = 0;
        /// Samples queued across every chunk.
        this.queued = 0;
        /// Process calls since the last report.
        this.sinceReport = 0;

        this.port.onmessage = (event) => {
            const chunk = event.data;
            if (chunk && chunk.length > 0) {
                this.chunks.push(chunk);
                this.queued += chunk.length;
            }
        };
    }

    process(inputs, outputs) {
        const output = outputs[0];
        if (!output || output.length === 0) {
            return true;
        }
        const frames = output[0].length;

        for (let index = 0; index < frames; index += 1) {
            // Drop chunks the device has finished with. A `while` and not an `if`: a chunk can
            // be empty, and one that never got dropped would stall the queue forever.
            while (this.chunks.length > 0 && this.offset >= this.chunks[0].length) {
                this.chunks.shift();
                this.offset = 0;
            }

            // **Underrun is silence, not the last sample repeated.** A repeat is a buzz that
            // sounds like a defect in the emulator; silence sounds like a gap, which is what it
            // is — and the status bar is already counting the dropped frames that caused it.
            let sample = 0;
            if (this.chunks.length > 0) {
                sample = this.chunks[0][this.offset];
                this.offset += 1;
                this.queued -= 1;
            }

            // Mono written to every channel. The Spectrum has one speaker.
            for (let channel = 0; channel < output.length; channel += 1) {
                output[channel][index] = sample;
            }
        }

        this.sinceReport += 1;
        if (this.sinceReport >= REPORT_EVERY) {
            this.sinceReport = 0;
            this.port.postMessage({ queued: this.queued });
        }

        // Never false. Returning false lets the browser garbage-collect the node, and a node
        // collected during a quiet passage would take the rest of the session's sound with it.
        return true;
    }
}

registerProcessor("zx-processor", ZxProcessor);
