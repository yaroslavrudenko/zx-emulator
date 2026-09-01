//! Media compiled into the binary — the third answer to *"where do the bytes come from?"*
//!
//! # Four sources, one path
//!
//! | Source | How the names arrive | How the bytes arrive |
//! |---|---|---|
//! | a command line | `std::env::args` | the filesystem |
//! | a URL | [`crate::host::arguments_from_query`] | an HTTP fetch |
//! | **this module** | [`arguments`] | [`bytes`], from the binary itself |
//! | a file dropped on the window | `macroquad::input::get_dropped_files` | the drop event |
//!
//! Every one of them ends at the same [`crate::host::partition`] and the same
//! [`crate::media::insert`]. That is not tidiness: a second way to say *"which ROM"* is a
//! second thing that can disagree about `--rom`, about repeats, about case — and the whole
//! argument for routing a query string into a `Vec<String>` holds here for the same reason.
//! **This module supplies names and bytes; it decides nothing.**
//!
//! So an embedded payload is addressed by its **file name**, exactly as a file on disk is.
//! `media::kind_of` reads the extension, `partition` sorts ROMs from media, and neither is told
//! which source it is holding.
//!
//! # The mechanism is committed and the payload never is
//!
//! `testdata/README.md` records that this repository redistributes nothing except the Sinclair
//! ROMs, on a permission quoted there with its author and its date. **A game is covered by no
//! such permission**, so a binary with one compiled in is a redistribution nobody here is
//! entitled to make. The payload therefore lives outside the repository, the `bundled` feature
//! is off by default, and `build.rs` fails the build rather than producing an artefact that
//! starts and shows nothing.
//!
//! # What a standalone build owes the person running it
//!
//! [`acknowledgement`] answers a question `web/index.html` answers differently for the same
//! reason. Amstrad's permission asks that *"the program/manual"* carry a note, and for a
//! double-clicked binary the **window is both**: there is no README on the path between the
//! artefact and its user, and a `--about` flag is the case `crate::main`'s own status bar
//! already rejects — *"a readout somebody has to know to switch on is the silent case with
//! extra steps."* So the note is drawn under the picture, permanently, and `F1` does not hide
//! it.
//!
//! **It is shown only when the embedded ROM is actually Sinclair's**, decided by looking rather
//! than assuming: every one of the three committed ROMs contains the bytes `Sinclair`, and a
//! ROM that does not is somebody else's work that Amstrad's permission says nothing about.
//! Printing their notice over a ROM they did not write would be a false statement in the one
//! place this repository is most careful not to make one.

/// The file name of the embedded ROM, or empty. Set by `build.rs`.
#[cfg(feature = "bundled")]
const ROM_NAME: &str = env!("ZX_BUNDLE_ROM_NAME");

/// The embedded ROM. Zero-length when `ROM_NAME` is empty.
#[cfg(feature = "bundled")]
const ROM_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bundle_rom.bin"));

/// The file name of the embedded tape or snapshot, or empty. Set by `build.rs`.
#[cfg(feature = "bundled")]
const MEDIA_NAME: &str = env!("ZX_BUNDLE_MEDIA_NAME");

/// The embedded tape or snapshot. Zero-length when `MEDIA_NAME` is empty.
#[cfg(feature = "bundled")]
const MEDIA_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bundle_media.bin"));

/// The marker that says a ROM image is a Sinclair one.
///
/// Present in all three committed ROMs — `48.rom`, `128-0.rom` and `128-1.rom` — checked on
/// 2026-09-01 with `LC_ALL=C grep -qa Sinclair testdata/roms/*.rom`, which is the command to
/// re-run rather than a fact to trust. The Spectrum's character set maps the ASCII letters
/// straight through, so the copyright line is stored as these bytes literally.
#[cfg(feature = "bundled")]
const SINCLAIR: &[u8] = b"Sinclair";

/// Everything this build embedded, as `(file name, bytes)`, in the order a command line would
/// have named them: the ROM first, because the ROM is what the machine is made of.
#[must_use]
pub fn entries() -> &'static [(&'static str, &'static [u8])] {
    #[cfg(feature = "bundled")]
    {
        // `match` on the two emptiness cases rather than building a `Vec`, so the return type
        // stays a `&'static` slice and a bundled build allocates nothing to answer this.
        return match (ROM_NAME.is_empty(), MEDIA_NAME.is_empty()) {
            (false, false) => &[(ROM_NAME, ROM_BYTES), (MEDIA_NAME, MEDIA_BYTES)],
            (false, true) => &[(ROM_NAME, ROM_BYTES)],
            (true, false) => &[(MEDIA_NAME, MEDIA_BYTES)],
            (true, true) => &[],
        };
    }
    #[cfg(not(feature = "bundled"))]
    &[]
}

/// The arguments a bundled build supplies when nobody named anything.
///
/// A ROM is emitted behind `--rom`, exactly as [`crate::host::arguments_from_query`] does for a
/// query string's `rom` key, and for the same reason: it makes the ROM's identity explicit
/// rather than inferred from an extension, so an embedded ROM whose file name has none still
/// builds the machine.
#[must_use]
pub fn arguments() -> Vec<String> {
    entries()
        .iter()
        .flat_map(|&(name, _)| {
            if crate::media::kind_of(name) == Some(crate::media::Kind::Rom) {
                vec!["--rom".to_owned(), name.to_owned()]
            } else {
                vec![name.to_owned()]
            }
        })
        .collect()
}

/// The embedded bytes `name` refers to, if this build has them.
///
/// Consulted by each binary's loader **before** the filesystem or the network, which is what
/// makes a standalone build need no files at all. The consequence, stated rather than
/// discovered: in a bundled build an embedded name **shadows** a file of the same name on disk.
/// That is the intended behaviour — the artefact is meant to be self-contained — and it is only
/// reachable in a build somebody deliberately turned the feature on for.
#[must_use]
pub fn bytes(name: &str) -> Option<&'static [u8]> {
    entries()
        .iter()
        .find(|&&(embedded, _)| embedded == name)
        .map(|&(_, bytes)| bytes)
}

/// The notice a build that embeds a Sinclair ROM must show the person running it.
///
/// [`None`] when nothing is embedded, and when what is embedded is not a Sinclair ROM. See this
/// module's header for why that distinction is drawn by looking at the bytes.
#[must_use]
pub fn acknowledgement() -> Option<&'static str> {
    #[cfg(feature = "bundled")]
    {
        let sinclair = entries().iter().any(|&(_, bytes)| {
            bytes
                .windows(SINCLAIR.len())
                .any(|window| window == SINCLAIR)
        });
        if sinclair {
            // The permission's own words, not a paraphrase. `testdata/README.md` quotes Cliff
            // Lawson of Amstrad plc in full and is the single source for the sourcing; this is
            // the sentence he asks the program to carry.
            return Some(
                "Amstrad have kindly given their permission for the redistribution of their \
                 copyrighted material but retain that copyright.",
            );
        }
    }
    None
}
