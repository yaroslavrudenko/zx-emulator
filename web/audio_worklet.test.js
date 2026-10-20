const { readFileSync } = require("node:fs");
const { test } = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const path = require("node:path");

const source = readFileSync(path.join(__dirname, "zx_audio_worklet.js"), "utf8");
const CAPACITY = processor().ring.length;

function processor(sampleRate = 48000) {
    let Registered;
    class AudioWorkletProcessor {
        constructor() { this.port = { postMessage() {} }; }
    }
    vm.runInNewContext(source, {
        AudioWorkletProcessor, Float32Array, sampleRate,
        registerProcessor(_name, constructor) { Registered = constructor; },
    });
    return new Registered();
}

for (const rate of [48000, 96000, 768000]) {
    test(`ring at ${rate} Hz covers the 100ms device buffer without unbounded growth`, () => {
        const capacity = processor(rate).ring.length;
        assert.equal(capacity & (capacity - 1), 0);
        assert.ok(capacity >= rate / 10);
        assert.ok(capacity <= 131072);
    });
}

test("8kHz overflow sends consumed credit for both discarded and drained samples", () => {
    const audio = processor(8000);
    const reports = [];
    audio.port.postMessage = (value) => {
        if (typeof value === "number") reports.push(value);
    };
    const length = audio.ring.length + 1;
    audio.accept(new Float32Array(length));
    assert.equal(audio.consumed, 1, "ring overflow is reported as loss, not queued audio");
    audio.process([], [[new Float32Array(128)]]);
    assert.deepEqual(reports, [129], "one quantum at 8kHz reports lost plus played samples");
});

for (const length of [CAPACITY + 1, 20000]) {
    test(`oversized chunk of ${length} accounts for every discarded sample`, () => {
        const audio = processor();
        audio.accept(Float32Array.from({ length }, (_, index) => index));
        assert.equal(audio.queued, CAPACITY);
        assert.equal(audio.consumed, length - CAPACITY);
        const channel = new Float32Array(128);
        audio.process([], [[channel]]);
        assert.equal(audio.consumed + audio.queued, length);
        assert.equal(channel[0], length - CAPACITY);
    });
}
