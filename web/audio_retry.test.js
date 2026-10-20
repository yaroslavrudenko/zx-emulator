const { readFileSync } = require("node:fs");
const { test } = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const path = require("node:path");

const source = readFileSync(path.join(__dirname, "zx_page.js"), "utf8");
const workletSource = readFileSync(path.join(__dirname, "zx_audio_worklet.js"), "utf8");

for (const closeFails of [false, true]) test(`worklet retry after close rejection=${closeFails}`, async () => {
    let rejectClose = closeFails;
    const contexts = [];
    const listeners = {};
    let plugin;
    class AudioContext {
        constructor() {
            this.state = "suspended";
            this.destination = {};
            this.sampleRate = 48000;
            this.audioWorklet = {
                addModule: () => contexts.length === 1
                    ? Promise.reject(new Error("first load failed")) : Promise.resolve(),
            };
            contexts.push(this);
        }
        resume() { this.state = "running"; return Promise.resolve(); }
        close() {
            if (rejectClose) return Promise.reject(new Error("close refused"));
            this.state = "closed";
            return Promise.resolve();
        }
    }
    class AudioWorkletNode {
        constructor() { this.port = {}; }
        connect() {}
    }
    vm.runInNewContext(source, {
        window: { AudioContext, location: { search: "" } }, AudioWorkletNode,
        document: { addEventListener(name, callback) { listeners[name] = callback; },
            querySelector() { return null; } },
        miniquad_add_plugin(value) { plugin = value; },
        console: { error() {}, warn() {} }, TextEncoder, TextDecoder,
    });
    plugin.on_init();
    await new Promise(setImmediate);
    assert.equal(contexts.length, 1);
    assert.equal(contexts[0].state, closeFails ? "suspended" : "closed");
    listeners.pointerdown();
    await new Promise(setImmediate);
    assert.equal(contexts.length, closeFails ? 1 : 2);
    assert.ok(contexts.filter((context) => context.state !== "closed").length <= 1,
        "each gesture must preserve the one-live-context invariant");
    if (closeFails) {
        rejectClose = false;
        listeners.pointerdown();
        await new Promise(setImmediate);
        listeners.pointerdown();
        await new Promise(setImmediate);
    }
    assert.equal(contexts.length, 2);
    assert.equal(contexts[1].state, "running");
});

test("stalled worklet bounds transferred buffers and recovers on consumption", async () => {
    for (const sampleRate of [48000, 8000]) {
        const samplesPerFrame = sampleRate / 50;
        const listeners = {};
        const posted = [];
        let plugin;
        let node;
        class AudioContext {
            constructor() {
                this.state = "running";
                this.sampleRate = sampleRate;
                this.destination = {};
                this.audioWorklet = { addModule: () => Promise.resolve() };
            }
        }
        class AudioWorkletNode {
            constructor() { this.port = { postMessage(chunk) { posted.push(chunk); } }; node = this; }
            connect() {}
        }
        const memory = { buffer: new ArrayBuffer(4096) };
        const imports = { env: {} };
        vm.runInNewContext(source, {
            window: { AudioContext, location: { search: "" } }, AudioWorkletNode,
            wasm_memory: memory,
            document: { addEventListener(name, callback) { listeners[name] = callback; },
                querySelector() { return null; } },
            miniquad_add_plugin(value) { plugin = value; },
            console: { error() {}, warn() {} }, TextEncoder, TextDecoder,
        });
        plugin.register_plugin(imports);
        plugin.on_init();
        await new Promise(setImmediate);
        for (let iteration = 0; iteration < 100; iteration += 1) {
            imports.env.zx_audio_push(0, samplesPerFrame);
        }
        assert.ok(posted.length <= 5, `stalled ${sampleRate}Hz worklet retained ${posted.length} transfers`);
        const before = posted.length;
        node.port.onmessage({ data: samplesPerFrame });
        imports.env.zx_audio_push(0, samplesPerFrame);
        assert.equal(posted.length, before + 1, "consumed report restores one buffer of credit");
    }
});

test("8kHz normal 50Hz production does not exhaust credit before consumed reports", async () => {
    const sampleRate = 8000;
    let Processor;
    let node;
    let accepted = 0;
    class AudioWorkletProcessor {
        constructor() {
            this.port = { postMessage(value) {
                if (typeof value === "number") node.port.onmessage({ data: value });
            } };
        }
    }
    vm.runInNewContext(workletSource, {
        AudioWorkletProcessor, Float32Array, sampleRate,
        registerProcessor(_name, constructor) { Processor = constructor; },
    });
    const worklet = new Processor();
    let plugin;
    class AudioContext {
        constructor() {
            this.state = "running";
            this.sampleRate = sampleRate;
            this.destination = {};
            this.audioWorklet = { addModule: () => Promise.resolve() };
        }
    }
    class AudioWorkletNode {
        constructor() {
            this.port = { postMessage(chunk) { accepted += 1; worklet.accept(chunk); } };
            node = this;
        }
        connect() {}
    }
    const imports = { env: {} };
    vm.runInNewContext(source, {
        window: { AudioContext, location: { search: "" } }, AudioWorkletNode,
        wasm_memory: { buffer: new ArrayBuffer(4096) },
        document: { addEventListener() {}, querySelector() { return null; } },
        miniquad_add_plugin(value) { plugin = value; },
        console: { error() {}, warn() {} }, TextEncoder, TextDecoder,
    });
    plugin.register_plugin(imports);
    plugin.on_init();
    await new Promise(setImmediate);
    const frame = [new Float32Array(128), new Float32Array(128)];
    for (let tick = 0; tick < 7; tick += 1) {
        const depth = imports.env.zx_audio_push(0, 160);
        assert.ok(depth <= sampleRate / 10, "100ms credit still bounds the page");
        worklet.process([], [frame]);
    }
    assert.equal(accepted, 7, "every normal frame must reach the device at 8kHz");
    assert.ok(worklet.consumed > 0, "the worklet actually drained samples");
});

test("pending close retains context ownership until close settles", async () => {
    const contexts = [];
    const listeners = {};
    let plugin;
    let finishClose;
    class AudioContext {
        constructor() {
            this.state = "suspended";
            this.sampleRate = 48000;
            this.destination = {};
            this.audioWorklet = { addModule: () => contexts.length === 1
                ? Promise.reject(new Error("first load failed")) : Promise.resolve() };
            contexts.push(this);
        }
        resume() { return Promise.resolve(); }
        close() { return new Promise((resolve) => { finishClose = () => {
            this.state = "closed"; resolve();
        }; }); }
    }
    vm.runInNewContext(source, {
        window: { AudioContext, location: { search: "" } },
        AudioWorkletNode: class { constructor() { this.port = {}; } connect() {} },
        document: { addEventListener(name, callback) { listeners[name] = callback; },
            querySelector() { return null; } },
        miniquad_add_plugin(value) { plugin = value; },
        console: { error() {}, warn() {} }, TextEncoder, TextDecoder,
    });
    plugin.on_init();
    await new Promise(setImmediate);
    listeners.pointerdown();
    assert.equal(contexts.filter((context) => context.state !== "closed").length, 1);
    finishClose();
    await new Promise(setImmediate);
    listeners.pointerdown();
    await new Promise(setImmediate);
    assert.equal(contexts.length, 2);
    assert.equal(contexts.filter((context) => context.state !== "closed").length, 1);
});
