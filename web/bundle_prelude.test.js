const { readFileSync } = require("node:fs");
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const path = require("node:path");

const html = readFileSync(path.join(__dirname, "index.html"), "utf8");
const bundle = readFileSync(path.join(__dirname, "mq_js_bundle.js"), "utf8");
const vendorTag = '<script src="mq_js_bundle.js"></script>';
const vendorAt = html.indexOf(vendorTag);

// `runInNewContext` uses a proxy global that accepts undeclared strict assignments.
// Each child gets a real, fresh classic-script global via `runInThisContext` instead.
function runBundle(prelude) {
    const child = `
        const vm = require("node:vm");
        const fs = require("node:fs");
        globalThis.document = { querySelector() { return { focus() {} }; } };
        globalThis.window = {};
        try {
            if (process.argv[2]) vm.runInThisContext(process.argv[2]);
            vm.runInThisContext(fs.readFileSync(process.argv[1], "utf8"));
            process.stdout.write(JSON.stringify({
                plugin: typeof globalThis.register_plugin,
                names: globalThis.plugins.map((entry) => entry.name),
                loader: typeof globalThis.load,
            }));
        } catch (error) {
            process.stderr.write(error.name + ": " + error.message);
            process.exitCode = 3;
        }
    `;
    return spawnSync(process.execPath,
        ["-e", child, path.join(__dirname, "mq_js_bundle.js"), prelude],
        { encoding: "utf8" });
}

test("classic HTML prelude makes the pinned strict bundle's quad-net plugin load", () => {
    assert.ok(bundle.startsWith('"use strict";'), "test the pinned strict bundle, not a substitute");
    assert.equal(bundle.match(/register_plugin=function/g)?.length, 1, "one undeclared assignment is the subject");
    assert.ok(vendorAt >= 0, "the vendor script must be loaded by this page");
    const prelude = [...html.slice(0, vendorAt).matchAll(/<script>([\s\S]*?)<\/script>/g)]
        .map((script) => script[1]).join("\n");
    assert.ok(prelude, "an inline classic prelude must execute before the vendor script");

    const withoutPrelude = runBundle("");
    assert.equal(withoutPrelude.status, 3, "strict vendor must fail without a global binding");
    assert.match(withoutPrelude.stderr, /ReferenceError: register_plugin is not defined/);
    const withPrelude = runBundle(prelude);
    assert.equal(withPrelude.status, 0, withPrelude.stderr);
    const browser = JSON.parse(withPrelude.stdout);
    assert.equal(browser.plugin, "function");
    assert.ok(browser.names.includes("quad_net"));
    assert.equal(browser.loader, "function", "the wasm loader remains available");
});
