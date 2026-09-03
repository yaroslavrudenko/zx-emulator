// The page's half of `crates/page`: the query string, the download, the audio bridge, and the
// canvas focus.
//
// *This line said **"and the whole of the JavaScript this project wrote"**, in a directory whose
// other hand-written file, `zx_audio_worklet.js`, is two hundred lines of this project's
// JavaScript and is loaded by the function forty lines below.* `web/README.md`'s table said the
// same thing one row above the row that listed the worklet. Both were written when this was the
// only file and neither was revisited when the second one arrived, which is the shape this
// repository catalogues most often — a sentence that was a measurement, left standing as a
// claim.
//
// Registered as a miniquad plugin before the module is instantiated, so that the **five**
// functions `crates/page/src/lib.rs` declares across its two `unsafe extern "C"` blocks exist in
// `importObject.env` by the time WebAssembly links against them. *That figure also said "three",
// from before the audio seam existed.* It is written here rather than dropped because
// `web/gate.sh` asserts all five by name against the built module, so this is a count with a
// gate on it rather than a copy without one — and if it drifts again, the gate is what says so.
//
// ## The failure this file is shaped around
//
// `gl.js`'s `add_missing_functions_stabs` walks the module's imports and replaces any the page
// did not provide with a stub that logs a warning and returns `undefined`. So a page served
// *without* this file still starts, still boots a ROM, still takes the keyboard — and `F2`
// calls a function that succeeds and saves nothing. `docs/M8.md` Decision 4 names that as the
// one failure this route must not have.
//
// The whole mitigation is one number: `undefined` crosses the wasm ABI as **0**, so success is
// **1** and everything else — including a stub, including a throw — is a refusal that
// `crates/page`'s `handoff` reports and the status line prints. Do not change `STARTED` to 0.
//
// ## The contract with the Rust side
//
// Names, argument order and the return convention are fixed by `crates/page/src/lib.rs`.
// `PLUGIN_VERSION` here must equal that file's and `STARTED` must equal its `STARTED`;
// `web/gate.sh` reads both literals out of both files and compares them, because miniquad's own
// runtime check is a `console.error` that does not stop the page, and `STARTED` has no runtime
// check at all.

(function () {
    "use strict";

    // Must equal `PLUGIN_VERSION` in `crates/page/src/lib.rs`. Bump both together whenever an
    // import is renamed, a signature changes, or the return convention moves.
    const PLUGIN_VERSION = 1;

    // The only value that means a download started. See the header, and `STARTED` in
    // `crates/page/src/lib.rs`.
    const STARTED = 1;
    const REFUSED = 0;

    // What `zx_audio_push` and `zx_audio_rate` answer when there is nothing to push into. See
    // `crates/page`'s `audio_push`: three situations, one number, because the Rust side's
    // response to all of them is identical.
    const NO_DEVICE = -1;
    const NO_RATE = 0;

    // `gl.js` hardcodes `document.querySelector("#glcanvas")`. The id is not configurable.
    const CANVAS = "#glcanvas";

    // Long enough for the browser to have read the Blob, short enough not to hold a snapshot's
    // worth of bytes alive for the session. Revoking immediately races the download in some
    // browsers; never revoking leaks one Blob per `F2`.
    const REVOKE_AFTER_MS = 60000;

    // Built once. Both are stateless and were being constructed per call — the encoder on every
    // `zx_query_length` and `zx_query_copy`, the decoder on every `F2`. The *bytes* are
    // deliberately not cached: `zx_query_copy` re-reads `location.search` on purpose, because
    // `history.replaceState` can change it between the two calls.
    const ENCODER = new TextEncoder();
    const DECODER = new TextDecoder();

    // `location.search` as UTF-8, leading `?` included — the exact string
    // `frontend::host::arguments_from_query` is documented against. Not decoded: a value here
    // names a file the page will fetch over HTTP, so it is going straight back into a URL.
    function queryBytes() {
        return ENCODER.encode(window.location.search);
    }

    // A view over wasm memory, created fresh on every call. `wasm_memory.buffer` is detached
    // and replaced when the module's memory grows, so a view held across a Rust call would
    // alias a dead buffer. Nothing here is held.
    function view(pointer, length) {
        return new Uint8Array(wasm_memory.buffer, pointer, length);
    }

    // Wrap an import so that a JavaScript exception becomes a refusal the Rust side can read.
    //
    // **A JavaScript exception thrown out of a wasm import does not return a value — it traps
    // the instance.** The emulator does not fail the operation; it stops existing, leaving a
    // console message and a frozen canvas. That is reachable from three of these five imports
    // without going anywhere exotic: `new Float32Array(wasm_memory.buffer, pointer, count)` and
    // `new Uint8Array(...)` both throw `RangeError` on a misaligned offset or an out-of-range
    // length, and `TextEncoder.encode` can throw on allocation failure.
    //
    // *One of the five was defensive and four were not.* `zx_offer_download` carried its own
    // `try`/`catch` and returned `REFUSED`; the doctrine that made it do so — *"`undefined`
    // crosses as 0, so success is 1 and everything else is a refusal"* — is stated in this
    // file's header as a property of the whole seam, and was implemented for one fifth of it.
    // It is one helper rather than five `try` blocks so that the next import gets it by
    // construction instead of by remembering.
    //
    // Reported to the console as well as returned, because the return value reaches a status
    // line with room for one sentence and this reaches a console with a stack.
    function refusing(name, refusal, body) {
        return function (...args) {
            try {
                return body(...args);
            } catch (error) {
                console.error("zx_page: " + name + " failed", error);
                return refusal;
            }
        };
    }

    // ---- audio -------------------------------------------------------------------------
    //
    // The device lives in `zx_audio_worklet.js`; this is the bridge to it. Three states matter
    // and `zx_audio_rate` reports them as one number, because the Rust side's response to all
    // three is identical — generate nothing:
    //
    //   no AudioContext        -> 0   (no Web Audio at all)
    //   context not running    -> 0   (autoplay policy, or a later suspend)
    //   worklet not yet loaded -> 0   (addModule is asynchronous)
    //
    // Rust polls it every frame until it is non-zero, then builds a resampler for that exact
    // rate. Reporting the rate before the context is *running* would have samples queue up
    // silently and then play late, all at once, which sounds like a fault rather than a delay.
    let audioContext = null;
    let audioNode = null;

    // The two running totals that make `zx_audio_push` answer the question it is documented to
    // answer: **how many samples are queued ahead of the caller, counting the ones just handed
    // over.**
    //
    // *This used to return `audioQueued`, the worklet's last periodic report of its own backlog.*
    // That answer excluded the chunk being posted and lagged by up to `REPORT_EVERY` render
    // quanta, so it was neither of the two things a caller could reasonably assume, and it
    // disagreed with what `crates/page`'s desktop `push` returns for the same call. Three
    // contracts for one function stopped being a documentation defect the moment
    // `frontend::audio::Resampler::track` began steering the output rate from this number every
    // 20 ms: a control loop whose sensor systematically under-reads by a whole frame has a bias
    // built into it that no amount of tuning can see.
    //
    // `postMessage` is **ordered**, so the samples in flight can be accounted for exactly rather
    // than estimated. `audioConsumed` is the worklet's count of samples it will never play again
    // — drained to the device, or dropped at its ring's ceiling — and everything this page has
    // pushed and the worklet has not yet finished with is `audioPushed - audioConsumed`, whether
    // it is sitting in the ring or still travelling. That is the answer, and it is exact for the
    // in-flight part rather than approximate.
    //
    // What is left over is the worklet's drain *since* its last report, which no message has
    // carried yet. It is bounded by `REPORT_EVERY` quanta — 8 x 128 = 1024 samples, about
    // 21.3 ms at 48 kHz, just inside one emulated frame — and it can only make this answer too
    // **large**, never too small. Erring toward "the queue is deeper than you think" makes the
    // loop produce slightly less audio, which is the safe direction: the failure being guarded
    // against is a backlog that climbs for the life of the tab.
    let audioPushed = 0;
    let audioConsumed = 0;

    // Buffers the worklet has finished copying out of, posted back here for reuse. The pool
    // replaces ~50 fresh allocations a second — one per pushed frame — whose backing stores
    // became garbage on the audio thread, the one thread in this page with a hard deadline to
    // miss. An empty pool is never an error and never waits: `zx_audio_push` allocates a fresh
    // buffer on a miss, so the worst case is exactly the per-frame allocation this pool exists
    // to avoid — no blocking, no dropping, on either thread.
    const freeBuffers = [];

    // The most buffers the pool parks. In flight at once is posted-not-yet-returned, which is
    // 2-3 at one post per 20 ms frame against a ~1 ms return round-trip; more parked than this
    // means something posts without popping, and those are left to the collector rather than
    // hoarded. A context that leaves `running` stops the pushes and the returns together —
    // `zx_audio_push` answers "no device" before it touches the pool — so a suspended tab
    // parks at most this many buffers, ~16 KiB at 48 kHz, for as long as it stays suspended.
    const FREE_LIST_MAX = 4;

    // Pool allocation granularity: 4096 bytes = 1024 samples of headroom at 48 kHz, so the
    // ±1-sample drift of `Resampler::track`'s rate correction never forces a reallocation.
    const POOL_GRANULARITY_BYTES = 4096;

    function audioStart() {
        if (audioContext !== null) {
            return;
        }
        const Ctor = window.AudioContext || window.webkitAudioContext;
        if (!Ctor) {
            console.warn("zx_page: no Web Audio in this browser; the emulator will be silent");
            return;
        }
        audioContext = new Ctor();
        audioContext.audioWorklet
            .addModule("zx_audio_worklet.js")
            .then(function () {
                audioNode = new AudioWorkletNode(audioContext, "zx-processor", {
                    numberOfInputs: 0,
                    outputChannelCount: [2],
                });
                audioNode.port.onmessage = function (event) {
                    // The worklet posts exactly two shapes: a number — its consumed count —
                    // and an exhausted chunk's `ArrayBuffer`, coming home for reuse. *This
                    // comment said "one number and nothing else"* until the buffer pool added
                    // the second shape. Checked rather than read blind, exactly as before: an
                    // unexpected shape would put `undefined` into the arithmetic above and
                    // make every subsequent push answer `NaN`, which reaches Rust as 0 and
                    // reads as "the queue is empty" forever. The two shapes are disjoint on
                    // one ordered port, so the consumed contract is untouched, and anything
                    // else still falls through ignored.
                    if (typeof event.data === "number") {
                        audioConsumed = event.data;
                        return;
                    }
                    if (event.data instanceof ArrayBuffer && freeBuffers.length < FREE_LIST_MAX) {
                        freeBuffers.push(event.data);
                    }
                };
                audioNode.connect(audioContext.destination);
            })
            .catch(function (error) {
                // Named loudly. A page that is silent for an unstated reason is the failure
                // this whole project is written against, and the console is the only surface
                // this function has.
                console.error("zx_page: the audio worklet failed to load", error);
            });
    }

    // An AudioContext starts suspended until the user has interacted with the page — every
    // desktop browser enforces this and none of them can be argued out of it. `gl.js`'s own
    // audio plugin does exactly the same thing for the same reason.
    function audioResume() {
        audioStart();
        if (audioContext === null || audioContext.state === "running") {
            return;
        }
        audioContext.resume().catch(function (error) {
            // *This call had no `.catch`, so a rejection was an unhandled rejection and nothing
            // else* — in a file that catches `addModule` forty lines up and says why in the
            // comment there. A context that refuses to resume is the whole of "why is there no
            // sound", and it is the one thing a person can act on.
            console.error("zx_page: the audio context refused to resume", error);
        });
    }

    function registerAudio(importObject) {
        importObject.env.zx_audio_rate = refusing("zx_audio_rate", NO_RATE, function () {
            if (audioContext === null || audioNode === null) {
                return NO_RATE;
            }
            return audioContext.state === "running" ? audioContext.sampleRate : NO_RATE;
        });

        importObject.env.zx_audio_push = refusing(
            "zx_audio_push",
            NO_DEVICE,
            function (pointer, count) {
                // **A context that is not `running` reports "no device", and nothing is
                // posted.** The same condition `zx_audio_rate` already applies, at the other
                // end of the same seam, and the reason it has to be here too is that nobody is
                // still asking `zx_audio_rate`: `crates/frontend`'s frame loop polls it only
                // until a resampler exists. So a context that *leaves* `running` afterwards —
                // an iOS interruption, an output-device change, any `suspend` — stops the
                // worklet's `process` being called, and nothing drains the ring. Posting into
                // that would grow memory and latency for as long as the tab is open while the
                // control loop, fed a frozen depth, ran open loop on a constant.
                //
                // `NO_DEVICE` already means exactly this to every caller, and
                // `Resampler::track` already ignores it.
                if (
                    audioNode === null ||
                    audioContext === null ||
                    audioContext.state !== "running"
                ) {
                    return NO_DEVICE;
                }
                // Nothing to post. Returning before the pool matters: the worklet ignores an
                // empty chunk without posting its buffer back, so an empty push that popped
                // the pool would quietly shrink it by one buffer each time.
                if (count === 0) {
                    return audioPushed - audioConsumed;
                }
                // The copy out of wasm memory **stays** — a view over wasm memory would alias
                // a buffer that can grow, detach and be reused underneath it, and the worklet
                // owns what it is handed on another thread for as long as it likes. This is
                // the copy `crates/page`'s SAFETY comment relies on, and the `set` below is
                // the wasm view's last read: everything after it touches only the copy.
                //
                // What changed is the copy's *destination*. *It was `.slice()`* — a fresh
                // buffer per frame, ~3,834 bytes at 48 kHz, fifty times a second, freed on the
                // audio thread — where a recycled one does the same job with no allocation in
                // steady state. The pool self-sizes: a miss, or a pooled buffer too small for
                // a grown `count`, allocates fresh — page-granular (`POOL_GRANULARITY_BYTES`,
                // whose comment carries the arithmetic), and no device-rate constant is
                // copied into this file.
                const bytes = count * Float32Array.BYTES_PER_ELEMENT;
                let buffer = freeBuffers.pop();
                if (buffer === undefined || buffer.byteLength < bytes) {
                    buffer = new ArrayBuffer(
                        Math.ceil(bytes / POOL_GRANULARITY_BYTES) * POOL_GRANULARITY_BYTES,
                    );
                }
                const samples = new Float32Array(buffer, 0, count);
                samples.set(new Float32Array(wasm_memory.buffer, pointer, count));
                // A view posted with its buffer in the transfer list keeps its offset and
                // length across the port: the worklet's `chunk.length` is still exactly
                // `count` while `chunk.buffer` is the full page-rounded capacity — no wrapper
                // object per frame. The transfer detaches `samples` on this side, and nothing
                // below reads it.
                audioNode.port.postMessage(samples, [buffer]);
                audioPushed += count;
                return audioPushed - audioConsumed;
            },
        );
    }

    function registerPlugin(importObject) {
        registerAudio(importObject);
        importObject.env.zx_query_length = refusing("zx_query_length", 0, function () {
            return queryBytes().length;
        });

        importObject.env.zx_query_copy = refusing(
            "zx_query_copy",
            0,
            function (destination, capacity) {
                const bytes = queryBytes();
                // `min` because the Rust side sized its buffer from a *previous* call to
                // `zx_query_length`, and nothing guarantees the URL did not change between the
                // two — `history.replaceState` can do it without a reload. Writing past the
                // buffer would be a heap overflow in a language that has no way to notice.
                const count = Math.min(bytes.length, capacity);
                view(destination, count).set(bytes.subarray(0, count));
                return count;
            },
        );

        importObject.env.zx_offer_download = refusing(
            "zx_offer_download",
            REFUSED,
            function (namePtr, nameLen, bytesPtr, bytesLen) {
                const name = DECODER.decode(view(namePtr, nameLen));
                // `.slice()` **copies**. The Blob outlives this call and wasm memory may grow
                // and detach its buffer afterwards, so a Blob built over a live view would be
                // a download of whatever happened to be at that address later — or of
                // nothing. This is the copy `crates/page`'s SAFETY comment relies on.
                const blob = new Blob([view(bytesPtr, bytesLen).slice()], {
                    type: "application/octet-stream",
                });
                const url = URL.createObjectURL(blob);
                const anchor = document.createElement("a");
                anchor.href = url;
                anchor.download = name;
                anchor.style.display = "none";
                document.body.appendChild(anchor);
                // **Both reads of wasm memory are already finished here, and `crates/page`'s
                // SAFETY comment now says so rather than claiming this cannot re-enter.**
                // `HTMLElement.click()` synchronously dispatches a bubbling `click` through
                // `document`, so any listener a future version of this page adds runs *inside*
                // this call and may grow wasm memory. Nothing above this line would notice,
                // because nothing above this line still holds a view. Keep it that way: a new
                // read of `view(...)` placed below this call would be a use-after-detach with
                // no diagnostic.
                anchor.click();
                anchor.remove();
                setTimeout(function () {
                    URL.revokeObjectURL(url);
                }, REVOKE_AFTER_MS);
                return STARTED;
            },
        );
    }

    // miniquad binds `canvas.onkeydown`, not `window.onkeydown`, so an unfocused canvas
    // receives no keys at all and the emulator looks dead. A fresh page load does not focus
    // it. `window.ondrop` is on the window, so dragging a file in works either way — an
    // asymmetry worth knowing before somebody reports that the keyboard is broken but
    // drag-and-drop is fine.
    function focusCanvas() {
        const canvas = document.querySelector(CANVAS);
        if (canvas !== null) {
            canvas.focus();
        }
    }

    // Both listeners, because the two ways into this emulator are a click and a keystroke, and
    // a person who lands on the page and immediately types should not have to click first to
    // get sound.
    //
    // *Both carried `{ once: true }`, on the reasoning that "after the context is running they
    // have nothing left to do".* That is true of the state they were written for and of no
    // other, and it cost two things:
    //
    //   - **The one keydown attempt could be spent on a key that grants nothing.** Browsers
    //     deliberately withhold sticky user activation for modifier and navigation keys, and
    //     this emulator binds SYMBOL SHIFT to **Ctrl and Tab** (`web/index.html`). A person who
    //     lands on the page and types — which `index.html` explicitly invites, with *"Sound
    //     starts on your first click or keystroke"* — could burn the listener on a keystroke
    //     that could never have started audio, and then never get another.
    //   - **There was no way back.** If the context later left `running`, both listeners were
    //     already gone and `audioStart` early-returns, so the tab was silent for the rest of its
    //     life with nothing a user could do about it.
    //
    // Leaving them bound fixes both, and costs a function call per keystroke: `audioResume`
    // reaches its early return in two comparisons once the context exists and is running. A
    // `statechange` listener was considered and is bigger for less — this is the same mechanism
    // the user is already performing, and it needs no second state machine.
    document.addEventListener("pointerdown", audioResume);
    document.addEventListener("keydown", audioResume);

    miniquad_add_plugin({
        name: "zx_page",
        version: PLUGIN_VERSION,
        register_plugin: registerPlugin,
        on_init: function () {
            focusCanvas();
            // Created eagerly so `sampleRate` is known and the worklet is already loading by
            // the time the user's first gesture arrives; it stays suspended until then.
            audioStart();
        },
    });
}());
