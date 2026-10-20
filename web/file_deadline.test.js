const { readFileSync } = require("node:fs");
const { test } = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const path = require("node:path");

const source = readFileSync(path.join(__dirname, "zx_page.js"), "utf8");

function page() {
    const requests = [];
    const completions = [];
    const timers = new Map();
    let nextTimer = 0;
    let plugin;
    class XHR {
        constructor() { this.readyState = 0; this.status = 0; this.aborted = false; requests.push(this); }
        open() {}
        send() {}
        abort() { this.aborted = true; this.readyState = 0; }
        finish(status, bytes = new ArrayBuffer(2)) {
            this.status = status;
            this.response = bytes;
            this.readyState = 4;
            this.onreadystatechange();
        }
    }
    const context = vm.createContext({
        XMLHttpRequest: XHR,
        location: { search: "" },
        miniquad_add_plugin(value) { plugin = value; },
        wasm_exports: { file_loaded(id) { completions.push(id); } },
        wasm_memory: { buffer: new ArrayBuffer(8) },
        TextEncoder, TextDecoder,
        document: { addEventListener() {} },
        setTimeout(callback, delay) { const id = ++nextTimer; timers.set(id, { callback, delay }); return id; },
        clearTimeout(id) { timers.delete(id); },
        console: { error() {}, warn() {} },
    });
    context.window = context;
    vm.runInContext(source, context);
    const imports = { env: {} };
    imports.env.fs_load_file = vm.runInContext(`(ptr, len) => {
        const id = ++globalThis.nextFile;
        const xhr = new XMLHttpRequest();
        xhr.open('GET', 'rom', true);
        xhr.responseType = 'arraybuffer';
        xhr.onreadystatechange = function () {
            if (this.readyState !== 4) return;
            globalThis.files[id] = this.status === 200 ? new Uint8Array(this.response) : null;
            wasm_exports.file_loaded(id);
        };
        xhr.send();
        return id;
    }`, context);
    context.nextFile = 0;
    context.files = {};
    plugin.register_plugin(imports);
    return { requests, completions, timers, context, imports, XHR };
}

test("timed-out XHR is aborted and settles once; sibling completion and late success remain isolated", () => {
    const { requests, completions, timers, context, imports, XHR } = page();
    const first = imports.env.fs_load_file(0, 0);
    assert.equal(context.XMLHttpRequest, XHR, "constructor restored immediately");
    const second = imports.env.fs_load_file(0, 0);
    assert.equal(requests.length, 2);
    assert.equal([...timers.values()][0].delay, 5000);
    requests[1].finish(200);
    assert.deepEqual(completions, [second]);
    assert.equal(timers.size, 1, "successful sibling clears only its own deadline");
    const [timeout] = timers.values();
    timeout.callback();
    assert.equal(requests[0].aborted, true, "the underlying request was aborted");
    assert.deepEqual(completions, [second, first], "timed-out request completed as failure once");
    assert.equal(context.files[first], undefined, "no successful byte buffer published");
    requests[0].finish(200);
    assert.deepEqual(completions, [second, first], "late success cannot complete twice");
    assert.equal(context.files[first], undefined, "late success cannot publish bytes");
});

test("XHR constructor is restored even when the pinned import throws", () => {
    const { context, imports, XHR } = page();
    imports.env.fs_load_file = (() => { throw new Error("synthetic startup failure"); });
    // Re-register around the throwing import, just as miniquad does before wasm instantiation.
    let plugin;
    const sandbox = { ...context, miniquad_add_plugin(value) { plugin = value; } };
    sandbox.window = sandbox;
    vm.runInNewContext(source, sandbox);
    plugin.register_plugin(imports);
    assert.throws(() => imports.env.fs_load_file(0, 0), /synthetic startup failure/);
    assert.equal(sandbox.XMLHttpRequest, XHR);
});
