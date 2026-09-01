//! Copy the `bundled` feature's payload where `include_bytes!` can reach it, or fail the build.
//!
//! # Why a build script rather than `include_bytes!(env!(…))` directly
//!
//! That one-liner works and gets one thing wrong: nothing tells `cargo` to rebuild when the
//! environment variable changes, so pointing `ZX_BUNDLE_MEDIA` at a different game and building
//! again can produce a binary containing the **previous** one. A stale payload is exactly the
//! failure this whole mechanism exists to avoid — a binary that starts, runs, and is not what
//! was asked for. `cargo:rerun-if-env-changed` and `cargo:rerun-if-changed` are what fix it and
//! they can only be emitted from here.
//!
//! The second reason is the error. `include_bytes!(env!("ZX_BUNDLE_MEDIA"))` on an unset
//! variable says *"environment variable `ZX_BUNDLE_MEDIA` not defined at compile time"*, which
//! is true and tells a person nothing about what to do. This file fails with the path it
//! actually looked at.
//!
//! # The rule this exists to keep
//!
//! **The mechanism is committed; the payload never is.** `testdata/README.md` records that
//! nothing in this repository is redistributed except the Sinclair ROMs, on a permission quoted
//! there with its author and date. A game is covered by no such permission, so a binary with
//! one compiled in is a redistribution this project has no right to make. The payload therefore
//! lives outside the repository — `testdata/` is `.gitignore`d apart from its README and the
//! ROMs named individually — and the default build neither needs it nor looks for it.
//!
//! With the feature **off** this script writes two empty placeholders and does nothing else.

use std::path::{Path, PathBuf};

/// Absolute path, or a path relative to the workspace root, of a ROM image to embed.
const ROM: &str = "ZX_BUNDLE_ROM";

/// Absolute path, or a path relative to the workspace root, of a tape or snapshot to embed.
const MEDIA: &str = "ZX_BUNDLE_MEDIA";

/// The extensions a build can embed, because they are the ones the emulator can load.
///
/// # A second copy of `src/media.rs`'s `EXTENSIONS`, and why
///
/// A build script runs *before* the crate it builds, so it cannot call `media::kind_of` and
/// there is no third place to put the list that both could reach without a fourth crate. The
/// copy is kept honest by `tests/bundled_extensions.rs`, which reads **this file as text** and
/// asserts every entry here is something `media::kind_of` recognises. That is the shape this
/// project prefers when duplication is genuinely forced: not a promise, a gate.
///
/// **`tzx` is deliberately absent and its absence is the loud kind.** Most commercial games
/// ship as `.tzx` because `.tap` cannot represent a turbo loader at all, so it is the most
/// likely thing to point this at — and a build that embedded one would produce an artefact
/// that starts and cannot load its own payload. `crates/spectrum` is gaining
/// `tape::tzx::parse`, producing the same pulse train `tap::parse` produces; when it lands,
/// `tzx` is added here, an arm is added to `media::insert`, and the refusal row in
/// `media::NOT_YET` is deleted. Three deletions and no rewrite, which is what `docs/M6.md`
/// Decision 5 bought by choosing a pulse train over a block list.
const LOADABLE: &[&str] = &["rom", "tap", "tzx", "z80", "sna"];

fn main() {
    // Emitted unconditionally. A `rerun-if-env-changed` that only appears when the feature is
    // on would not fire on the build that *turns the feature on*, which is the first build that
    // needs it.
    println!("cargo:rerun-if-env-changed={ROM}");
    println!("cargo:rerun-if-env-changed={MEDIA}");
    println!("cargo:rerun-if-changed=build.rs");

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo always sets OUT_DIR"));
    let bundled = std::env::var_os("CARGO_FEATURE_BUNDLED").is_some();

    let rom = embed(&out, "rom", ROM, bundled);
    let media = embed(&out, "media", MEDIA, bundled);

    if bundled && rom.is_empty() && media.is_empty() {
        // Loud, at build time, and this is the whole point of the check. The alternative is a
        // standalone binary that launches, opens a window, and shows nothing — which looks like
        // a broken emulator rather than a build that was never given anything to embed.
        panic!(
            "the `bundled` feature is on and neither {ROM} nor {MEDIA} is set, so there is \
             nothing to embed.\n\
             \n\
             Set at least one, to an absolute path or a path relative to the workspace root:\n\
             \n    ZX_BUNDLE_ROM=testdata/roms/48.rom \\\n    \
             ZX_BUNDLE_MEDIA=testdata/games/your.tap \\\n    \
             cargo build --release --manifest-path crates/frontend/Cargo.toml \\\n        \
             --features bundled --bin zx\n\
             \n\
             The payload is never committed — see testdata/README.md."
        );
    }

    // Read back by `src/bundle.rs`. Empty means "this build embedded none".
    println!("cargo:rustc-env=ZX_BUNDLE_ROM_NAME={rom}");
    println!("cargo:rustc-env=ZX_BUNDLE_MEDIA_NAME={media}");
}

/// Copy what `variable` names into `OUT_DIR/bundle_<slot>.bin`; return the name it will be
/// known by, or an empty string if this build embeds nothing in that slot.
///
/// The name is the path's **file name**, because that is what `media::kind_of` reads an
/// extension off and what `host::partition` sorts into ROMs and media. So the embedded payload
/// is addressed exactly as a file on disk would be, which is what keeps one loader and one
/// `partition` for all four sources.
fn embed(out: &Path, slot: &str, variable: &str, bundled: bool) -> String {
    let destination = out.join(format!("bundle_{slot}.bin"));

    let Some(raw) = std::env::var_os(variable).filter(|value| !value.is_empty()) else {
        // `include_bytes!` needs a file whether or not this slot is used, and an empty one is
        // cheaper than a `cfg` the rest of the crate would have to carry.
        std::fs::write(&destination, [])
            .unwrap_or_else(|error| panic!("cannot write {}: {error}", destination.display()));
        return String::new();
    };

    if !bundled {
        // Set but unused. Said out loud rather than ignored: a person who exported the variable
        // and forgot `--features bundled` gets an ordinary build and no explanation otherwise.
        println!(
            "cargo:warning={variable} is set but the `bundled` feature is off, so nothing is \
             embedded; add --features bundled"
        );
        std::fs::write(&destination, [])
            .unwrap_or_else(|error| panic!("cannot write {}: {error}", destination.display()));
        return String::new();
    }

    let source = resolve(Path::new(&raw));
    println!("cargo:rerun-if-changed={}", source.display());

    // Checked here rather than left to produce a legible refusal at run time, because the two
    // failures are not equivalent: a refusal at run time is a standalone binary somebody built,
    // copied to another machine and double-clicked before finding out. `docs/M8.md`'s standing
    // preference is for the failure that happens where it can still be acted on.
    let extension = source
        .extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !LOADABLE.iter().any(|&known| known == extension) {
        panic!(
            "{variable} names {}, and this emulator cannot load a .{extension}\n\
             \n\
             It embeds: {}\n\
             \n\
             .tzx is the common one and is not in that list. `.tap` cannot represent a turbo \
             loader, which is what most commercial games use, so `.tzx` support is being added \
             to crates/spectrum — until it lands, a .tzx cannot be embedded or loaded.",
            source.display(),
            LOADABLE
                .iter()
                .map(|known| format!(".{known}"))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }

    let bytes = std::fs::read(&source).unwrap_or_else(|error| {
        panic!(
            "{variable} names {} and it cannot be read: {error}\n\
             Paths are absolute, or relative to the workspace root. The payload is not \
             committed — see testdata/README.md.",
            source.display(),
        )
    });
    if bytes.is_empty() {
        panic!("{variable} names {}, which is empty", source.display());
    }
    std::fs::write(&destination, &bytes)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", destination.display()));

    Path::new(&raw)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| panic!("{variable} names {:?}, which has no file name", raw))
}

/// An absolute path unchanged; a relative one against the workspace root.
///
/// A build script's working directory is its own package directory, so a path a person typed
/// relative to where they ran `cargo` would resolve somewhere they did not mean. The workspace
/// root is the one anchor that matches how every path in this repository's documentation is
/// written — `testdata/roms/48.rom`, not `../../testdata/roms/48.rom`.
fn resolve(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let manifest = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo always sets CARGO_MANIFEST_DIR"),
    );
    // crates/frontend -> crates -> the workspace root.
    manifest
        .parent()
        .and_then(Path::parent)
        .unwrap_or(&manifest)
        .join(path)
}
