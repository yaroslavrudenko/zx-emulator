// The page's half of `crates/page`, and the whole of the JavaScript this project wrote.
//
// Registered as a miniquad plugin before the module is instantiated, so that the three
// functions `crates/page/src/lib.rs` declares in its `unsafe extern "C"` block exist in
// `importObject.env` by the time WebAssembly links against them.
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
// `PLUGIN_VERSION` here must equal that file's, and miniquad checks it: `init_plugins` calls
// `wasm_exports.zx_page_crate_version()` and `console.error`s on a mismatch. That is a weak
// check — it does not stop the page — which is why `web/gate.sh` also asserts the three imports
// exist in the built module by name.

(function () {
    "use strict";

    // Must equal `PLUGIN_VERSION` in `crates/page/src/lib.rs`. Bump both together whenever an
    // import is renamed, a signature changes, or the return convention moves.
    const PLUGIN_VERSION = 1;

    // The only value that means a download started. See the header, and `STARTED` in
    // `crates/page/src/lib.rs`.
    const STARTED = 1;
    const REFUSED = 0;

    // `gl.js` hardcodes `document.querySelector("#glcanvas")`. The id is not configurable.
    const CANVAS = "#glcanvas";

    // Long enough for the browser to have read the Blob, short enough not to hold a snapshot's
    // worth of bytes alive for the session. Revoking immediately races the download in some
    // browsers; never revoking leaks one Blob per `F2`.
    const REVOKE_AFTER_MS = 60000;

    // `location.search` as UTF-8, leading `?` included — the exact string
    // `frontend::host::arguments_from_query` is documented against. Not decoded: a value here
    // names a file the page will fetch over HTTP, so it is going straight back into a URL.
    function queryBytes() {
        return new TextEncoder().encode(window.location.search);
    }

    // A view over wasm memory, created fresh on every call. `wasm_memory.buffer` is detached
    // and replaced when the module's memory grows, so a view held across a Rust call would
    // alias a dead buffer. Nothing here is held.
    function view(pointer, length) {
        return new Uint8Array(wasm_memory.buffer, pointer, length);
    }

    // ---- audio -------------------------------------------------------------------------
    //
    // The device lives in `zx_audio_worklet.js`; this is the bridge to it. Three states matter
    // and `zx_audio_rate` reports them as one number, because the Rust side's response to all
    // three is identical — generate nothing:
    //
    //   no AudioContext        -> 0   (no Web Audio at all)
    //   context suspended      -> 0   (autoplay policy: no user gesture yet)
    //   worklet not yet loaded -> 0   (addModule is asynchronous)
    //
    // Rust polls it every frame until it is non-zero, then builds a resampler for that exact
    // rate. Reporting the rate before the context is *running* would have samples queue up
    // silently and then play late, all at once, which sounds like a fault rather than a delay.
    let audioContext = null;
    let audioNode = null;
    let audioQueued = 0;

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
                    audioQueued = event.data.queued;
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
        if (audioContext !== null && audioContext.state === "suspended") {
            audioContext.resume();
        }
    }

    function registerAudio(importObject) {
        importObject.env.zx_audio_rate = function () {
            if (audioContext === null || audioNode === null) {
                return 0;
            }
            return audioContext.state === "running" ? audioContext.sampleRate : 0;
        };

        importObject.env.zx_audio_push = function (pointer, count) {
            if (audioNode === null) {
                return -1;
            }
            // `.slice()` **copies** out of wasm memory. The buffer is then transferred to the
            // worklet, which owns it on another thread for as long as it likes — a view over
            // wasm memory would alias a buffer that can grow, detach and be reused underneath
            // it. This is the copy `crates/page`'s SAFETY comment relies on.
            const samples = new Float32Array(wasm_memory.buffer, pointer, count).slice();
            audioNode.port.postMessage(samples, [samples.buffer]);
            return audioQueued;
        };
    }

    function registerPlugin(importObject) {
        registerAudio(importObject);
        importObject.env.zx_query_length = function () {
            return queryBytes().length;
        };

        importObject.env.zx_query_copy = function (destination, capacity) {
            const bytes = queryBytes();
            // `min` because the Rust side sized its buffer from a *previous* call to
            // `zx_query_length`, and nothing guarantees the URL did not change between the
            // two — `history.replaceState` can do it without a reload. Writing past the
            // buffer would be a heap overflow in a language that has no way to notice.
            const count = Math.min(bytes.length, capacity);
            view(destination, count).set(bytes.subarray(0, count));
            return count;
        };

        importObject.env.zx_offer_download = function (namePtr, nameLen, bytesPtr, bytesLen) {
            try {
                const name = new TextDecoder().decode(view(namePtr, nameLen));
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
                anchor.click();
                anchor.remove();
                setTimeout(function () {
                    URL.revokeObjectURL(url);
                }, REVOKE_AFTER_MS);
                return STARTED;
            } catch (error) {
                // Reported here as well as returned, because the return value reaches a status
                // line with room for one sentence and this reaches a console with a stack.
                console.error("zx_page: the download failed", error);
                return REFUSED;
            }
        };
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
    // get sound. `once` on each: after the context is running they have nothing left to do.
    document.addEventListener("pointerdown", audioResume, { once: true });
    document.addEventListener("keydown", audioResume, { once: true });

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
