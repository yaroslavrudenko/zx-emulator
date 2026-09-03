// The browser's audio device: a preallocated ring buffer drained by the audio thread.
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
//
// ## Why a preallocated ring and not an array of chunks
//
// **This was `chunks = []`, pushed unconditionally and drained with `shift()`, and every defect
// below followed from that one shape.**
//
//   - It had **no ceiling at all**, so the browser's audio queue was unbounded — while
//     `crates/page/src/lib.rs`'s desktop twin refuses exactly that, in writing: *"Dropped rather
//     than grown. An unbounded queue does not fix an emulator running fast; it converts a small
//     permanent error into a latency that climbs for as long as the session lasts."* Two devices,
//     one documented rule, honoured by one of them. There was no insertion point at which a
//     ceiling *could* be enforced without inventing one.
//   - A chunk boundary can fall anywhere, so every sample paid a five-load `while` guard that
//     could only be true about fifty times a second — ~240,000 property loads to catch 50 events.
//   - The same boundary blocked a block copy to the second output channel, so the mono sample was
//     stored per channel per sample: 96,000 indexed stores a second where two `memcpy`s would do.
//   - `Array.prototype.shift()` is O(n) in chunk count, so the drop path degraded quadratically
//     exactly when the queue was already pathological.
//
// One `Float32Array` with masked read and write indices answers all four, and the fix is smaller
// than the thing it replaces.
//
// ## The ceiling is this ring's capacity, and it is deliberately the *last* bound
//
// **`BUFFER_MILLISECONDS` is not copied into this file, and must not be.** It is `pub` in
// `crates/page/src/lib.rs`, and a second copy of it here would be a new instance of exactly the
// Rust/JavaScript duplication this seam already has too much of. The bound that matters is
// enforced twice before anything here can fire — `frontend::audio::Resampler::track` holds the
// queue near half the buffer, and `crates/frontend`'s frame loop refuses to push past a full one
// — so this ring is a backstop against a producer that has already escaped both, and its size
// only has to be *comfortably larger* than their figure rather than equal to it.
//
// 16384 samples is 341 ms at 48 kHz and 171 ms at 96 kHz, so it clears the main thread's 100 ms
// bound at every output rate a browser will plausibly run, and it is a power of two so the
// indices mask instead of branching. 64 KiB, allocated once, for the whole session.
//
// **When it is full the OLDEST samples go**, which is the opposite of what the desktop ring does
// and is right for the same reason the desktop's is: the discard has to land on the audio least
// worth keeping. On the desktop `push` is the only writer and the newest samples are the ones not
// yet committed to. Here the ring is already the last resort — something upstream has failed and
// the queue is a latency nobody asked for — so the samples worth keeping are the ones about to
// play, and the stale front of the queue is what a listener would experience as the delay.

// Samples the ring holds. A power of two, so the indices mask rather than divide.
//
// `//` and not `///`, throughout this file: `///` is Rust's doc syntax and in JavaScript it is
// an ordinary comment that no tool consumes. It was used here for the field comments below,
// which read as documentation and were not. JSDoc's `/** */` would be consumed — by nothing this
// project runs, so plain comments are the honest spelling.
const CAPACITY = 16384;

// Turns any index into a ring offset. See `CAPACITY`.
const MASK = CAPACITY - 1;

// Process calls between reports of the backlog to the main thread.
//
// The main thread needs the queue depth so Rust can see whether it is running ahead or behind,
// and a message per `process` call would be ~375 a second for a number nobody reads that often.
//
// **8 and not 16, because the number is now a control loop's feedback rather than a readout.**
// `frontend::audio::Resampler::track` corrects the output rate from this depth once per emulated
// frame — every 20 ms — and at 16 the depth refreshed every ~42.7 ms, so the loop applied the
// same stale error twice before seeing its own effect. A controller acting on measurements
// slower than its own period is how a limit cycle starts, and a limit cycle in this loop is
// audible as pitch wobble. At 8 the refresh is ~21.3 ms, just inside the frame.
const REPORT_EVERY = 8;

class ZxProcessor extends AudioWorkletProcessor {
    constructor() {
        super();
        // Every sample the device has been handed and has not yet played. Allocated once.
        this.ring = new Float32Array(CAPACITY);
        // Where `process` reads next, and where `onmessage` writes next. Both masked.
        this.read = 0;
        this.write = 0;
        // Samples sitting between them. Kept explicitly because `write === read` is ambiguous
        // between empty and full, and the alternative — never filling the last slot — would
        // make the capacity not a power of two after all.
        this.queued = 0;
        // Samples this worklet will never play again: drained to the device, or dropped.
        //
        // **This is the whole of the periodic report, and one number is enough.** The main
        // thread knows how many samples it has ever posted; the answer `zx_audio_push` owes its
        // caller is "how many are queued ahead of you, counting the ones you just handed over",
        // which is `pushed - consumed` — the samples handed over that have not been finished
        // with, whether they are in this ring or still in flight on the message port.
        // `postMessage` is ordered, so nothing can overtake the report that carries this.
        //
        // Both counters are exact integers well inside a double's 2^53, so their difference is
        // exact: at ~48,000 samples a second, 2^53 is longer than five thousand years.
        //
        // Dropping increments this, deliberately. A dropped sample is one the device will never
        // play, so counting it as consumed is what keeps the main thread's answer truthful
        // instead of leaving a phantom in the queue depth forever.
        this.consumed = 0;
        // Process calls since the last report. See `REPORT_EVERY`.
        this.sinceReport = 0;

        this.port.onmessage = (event) => {
            this.accept(event.data);
        };
    }

    // Copy one chunk into the ring, dropping the oldest samples if it does not fit.
    accept(chunk) {
        if (!chunk || chunk.length === 0) {
            return;
        }
        // A chunk larger than the whole ring keeps only its newest `CAPACITY` samples — the same
        // rule as the overflow below, applied before the arithmetic rather than as a special
        // case inside it. It cannot happen at any rate this emulator runs (a frame is ~960
        // samples against 16384) and is handled because "cannot happen" is not a bounds check.
        const start = chunk.length > CAPACITY ? chunk.length - CAPACITY : 0;
        const taken = chunk.length - start;

        const overflow = this.queued + taken - CAPACITY;
        if (overflow > 0) {
            this.read = (this.read + overflow) & MASK;
            this.queued -= overflow;
            this.consumed += overflow;
        }

        // At most two `set()` calls: one to the end of the ring, one from the start if the write
        // wrapped. `CAPACITY` being a power of two is what makes the wrap a mask rather than a
        // modulo.
        const first = Math.min(taken, CAPACITY - this.write);
        this.ring.set(chunk.subarray(start, start + first), this.write);
        if (taken > first) {
            this.ring.set(chunk.subarray(start + first, start + taken), 0);
        }
        this.write = (this.write + taken) & MASK;
        this.queued += taken;
    }

    process(inputs, outputs) {
        const output = outputs[0];
        if (!output || output.length === 0) {
            return true;
        }
        const channel = output[0];
        const frames = channel.length;
        const available = Math.min(this.queued, frames);

        // Two block copies at most, for the same wrap reason as `accept`.
        const first = Math.min(available, CAPACITY - this.read);
        channel.set(this.ring.subarray(this.read, this.read + first), 0);
        if (available > first) {
            channel.set(this.ring.subarray(0, available - first), first);
        }

        // **Underrun is silence, not the last sample repeated.** A repeat is a buzz that sounds
        // like a defect in the emulator; silence sounds like a gap, which is what it is — and the
        // status bar is already counting the dropped frames that caused it. `crates/page`'s
        // desktop `drain` makes the same choice for the same reason.
        if (available < frames) {
            channel.fill(0, available);
        }

        this.read = (this.read + available) & MASK;
        this.queued -= available;
        this.consumed += available;

        // Mono written to every channel. The Spectrum has one speaker. One `memcpy` per extra
        // channel, rather than one indexed store per channel per sample.
        for (let index = 1; index < output.length; index += 1) {
            output[index].set(channel);
        }

        this.sinceReport += 1;
        if (this.sinceReport >= REPORT_EVERY) {
            this.sinceReport = 0;
            // A number, not an object literal. Structured-cloning `{ consumed: n }` allocated a
            // wrapper on the audio thread ~23 times a second for one field.
            //
            // The two `subarray` views above are what is left, and they are named rather than
            // claimed away: they are short-lived, non-escaping views a JIT can scalar-replace,
            // which an object crossing a port never is. "`process` allocates nothing" is not
            // quite assertable; "`process` allocates nothing that outlives it" is.
            this.port.postMessage(this.consumed);
        }

        // Never false. Returning false lets the browser garbage-collect the node, and a node
        // collected during a quiet passage would take the rest of the session's sound with it.
        return true;
    }
}

registerProcessor("zx-processor", ZxProcessor);
