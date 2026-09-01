#!/bin/sh
# Assemble the directory that is the page.
#
# `docs/M8.md` Decision 8 (YAGNI): M8 acquires no domain, no CI job, no hosting provider and no
# cache policy. What it is responsible for is "a directory that is a working page", and
# "anything that can serve static files over HTTP serves it, and that is the whole contract."
# This script builds that directory.
#
#     sh web/build.sh
#     cd target/web && python3 -m http.server 8000     # then http://localhost:8000/
#
# It writes to `target/web/` and nowhere else. That is deliberate: `target/` is already the
# only entry in `.gitignore`, so the assembled page — which contains a copy of a Sinclair ROM —
# cannot be committed by accident, and no `.gitignore` change is needed to keep it out.
# `testdata/README.md` names every committed ROM individually, precisely so that a new copy of
# one has to be argued for rather than swept in by a glob.
#
# `file://` will not work and this is not a bug to chase. macroquad's `load_file` is an
# `XMLHttpRequest`, and a page opened from disk cannot issue one against a sibling file. It has
# to be served.

set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/.." && pwd)
out="$root/target/web"

echo "==> building zx.wasm"
cargo build --release \
    --manifest-path "$root/crates/frontend/Cargo.toml" \
    --target wasm32-unknown-unknown \
    --bin zx

wasm="$root/target/wasm32-unknown-unknown/release/zx.wasm"
[ -f "$wasm" ] || { echo "no artefact at $wasm" >&2; exit 1; }

echo "==> assembling $out"
mkdir -p "$out/testdata/roms"
cp "$here/index.html" "$here/mq_js_bundle.js" "$here/zx_page.js" "$here/zx_audio_worklet.js" "$out/"
cp "$wasm" "$out/zx.wasm"

# The ROM the shell's DEFAULT_ROM names, at the path it names, so that a bare URL with no query
# string boots. The relative path is resolved against the page by the XHR, which is why the
# directory structure is reproduced rather than flattened.
copied_rom=no
for rom in 48.rom 128-0.rom 128-1.rom; do
    if [ -f "$root/testdata/roms/$rom" ]; then
        cp "$root/testdata/roms/$rom" "$out/testdata/roms/$rom"
        copied_rom=yes
    fi
done

if [ "$copied_rom" = no ]; then
    # Named rather than skipped, in the shape `testdata/README.md` uses for every absent
    # corpus: a page assembled without a ROM loads, and then fails in the browser with a fetch
    # error that looks like a deployment problem rather than a missing file.
    echo "!!  no ROM found under $root/testdata/roms/" >&2
    echo "!!  the page will build and will not boot; see testdata/README.md" >&2
fi

echo
echo "==> $out"
ls -l "$out"
[ "$copied_rom" = yes ] && ls -l "$out/testdata/roms"
echo
echo "serve it (file:// will not work — load_file is an XHR):"
echo "    cd $out && python3 -m http.server 8000"
echo "then open http://localhost:8000/"
