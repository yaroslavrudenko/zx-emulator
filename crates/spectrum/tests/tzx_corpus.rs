//! Real `.tzx` files, if there are any: our reading of the format against somebody else's.
//!
//! # What a foreign file grades that nothing else here can
//!
//! Every other `.tzx` gate builds its bytes in the test, so a shared misreading of the block
//! framing — a length taken from the wrong offset, a word read big-endian, a body measured one
//! byte short — would be invisible to all of them, because the same misreading would sit on both
//! sides of the comparison. That is measured rather than assumed: `docs/STATUS.md` records the
//! whole third-party snapshot corpus staying **green** under a symmetric mutation.
//!
//! A file somebody else's tool wrote breaks that symmetry, and this sweep uses the strongest
//! break available: **the ROM's own `LD-BYTES` loads the file's first block.** The block's flag
//! and length are read out of the file directly, the tape comes from our converter, and the
//! parity check is Sinclair's. Three parties, one of whom wrote the file, one of whom wrote the
//! loader, and neither of whom is us.
//!
//! # Why this is opportunistic rather than a gate, and why that is said out loud
//!
//! Every other corpus here is **fetched**, and `crates/testsupport` makes its absence a
//! failure that names the fetch — because a gate whose corpus is absent by default is a gate
//! that runs nowhere, which `docs/STATUS.md` records three times.
//!
//! **There is no fetch to name for this one.** `.tzx` is the format commercial games ship in;
//! a search of the sources this workspace already uses found none, and games may not be
//! redistributed, so no game is committed and a fresh clone finds exactly one file in
//! `testdata/games/`: `PROVENANCE.md`, the record of what the corpus is and where each file came
//! from, which ships precisely because none of them may. Applying the shared policy would make
//! every clone fail with instructions nobody can follow, which is a worse failure than the one
//! it guards against.
//!
//! So this sweep **skips when there is nothing to sweep**, and says so. It claims nothing on a
//! clone with no files; it becomes the strongest instrument in the tape subsystem the moment
//! one appears. That is a deliberate departure from the shared policy and it belongs in the
//! open register rather than in a comment — `docs/M6.md` records it under the `.tzx` section.

use spectrum::tape::{Tape, tzx};
use spectrum::timing::T_STATES_PER_FRAME;
use spectrum::{Model, Spectrum};
use std::path::{Path, PathBuf};
use z80::CpuState;

/// Where a user's own `.tzx` files may live. A `.tzx` in either is gitignored.
const CORPUS_DIRS: [&str; 2] = ["tzx", "games"];

/// `ID 10 - Standard Speed Data Block`, the one shape whose timings are fixed by the format and
/// which the ROM's own loader can therefore read.
const STANDARD_SPEED_DATA: u8 = 0x10;

/// Bytes before the first block: `"ZXTape!"`, the marker, and the two revision numbers.
const HEADER_LENGTH: usize = 10;

/// `LD-BYTES`, the ROM's tape loader.
const LD_BYTES: u16 = 0x0556;

const STUB: u16 = 0xC000;
const LOADED: u16 = 0xC100;
const FAILED: u16 = 0xC200;
const DESTINATION: u16 = 0x8000;
const STACK: u16 = 0xBF00;

/// T-states to allow. A header block's pilot tone alone is 8064 edges of 2168 T-states.
const BUDGET: u64 = 30_000_000;

/// Every `.tzx` under `testdata/`, in a stable order.
fn corpus() -> Vec<PathBuf> {
    testsupport::reject_obsolete_env();

    let mut files = Vec::new();
    for name in CORPUS_DIRS {
        let dir = testsupport::testdata_dir().join(name);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        files.extend(
            entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("tzx"))
                }),
        );
    }
    files.sort();
    files
}

/// Announce an empty corpus rather than passing silently.
fn announce_empty() {
    println!(
        "no .tzx files found under testdata/{{{}}} — this sweep verified nothing. \
         Drop any .tzx there and it will be graded against the real ROM.",
        CORPUS_DIRS.join(",")
    );
}

fn sinclair_rom() -> Option<Vec<u8>> {
    let path = testsupport::testdata_dir().join("roms").join("48.rom");
    match std::fs::read(&path) {
        Ok(rom) => Some(rom),
        Err(_) => {
            testsupport::skip_absent_corpus("the Sinclair 48K ROM", &path);
            None
        }
    }
}

/// The body length of a block that carries **no signal**, or `None` for anything else.
///
/// Real files open with a preamble of metadata, and a sweep that only looks at file offset
/// `HEADER_LENGTH` sees the preamble instead of the tape. `MarioBros.tzx` opens with an archive
/// info block and a text description, so it was skipped in silence while
/// `ManicMiner.tzx` — which opens with the data — was graded: **one of the two files handed to
/// this sweep, with nothing said about the other.**
///
/// `None` for every block that carries signal, deliberately and including the ones whose length
/// is perfectly well known. Stepping over one of those would start the ROM's load in the middle
/// of a tape and report a pass for a file this sweep never reached, which is a worse outcome than
/// the silent skip it replaces.
///
/// Transcribed from the description's own length column rather than read from `spectrum::tape`,
/// which is the module under test: `[00]+01` for `ID 21` and `ID 30`, `00` for `ID 22`,
/// `[01]+02` for `ID 31`, `[00,01]+02` for `ID 32`, `[00]*03+01` for `ID 33`, `[10,11,12,13]+14`
/// for `ID 35`, and a flat `09` for the glue block. A mistake in any of these lands the walk
/// mid-file, so the ROM below is handed nonsense and the sweep goes **red** — this cannot fail
/// quietly.
fn metadata_body_length(bytes: &[u8], at: usize) -> Option<usize> {
    let byte = |offset: usize| bytes.get(at + 1 + offset).copied().map(usize::from);
    let word = |offset: usize| {
        let low = bytes.get(at + 1 + offset).copied()?;
        let high = bytes.get(at + 2 + offset).copied()?;
        Some(usize::from(u16::from_le_bytes([low, high])))
    };
    match bytes.get(at).copied()? {
        0x22 => Some(0),
        0x21 | 0x30 => Some(1 + byte(0)?),
        0x31 => Some(2 + byte(1)?),
        0x32 => Some(2 + word(0)?),
        0x33 => Some(1 + 3 * byte(0)?),
        0x35 => Some(0x14 + word(0x10)? + (word(0x12)? << 16)),
        0x5A => Some(9),
        _ => None,
    }
}

/// Where the first signal-carrying block is, having stepped over any metadata preamble.
///
/// Terminates because every step consumes at least the ID byte, and the offset is bounded by the
/// file's length.
fn first_signal_block(bytes: &[u8]) -> Option<usize> {
    let mut at = HEADER_LENGTH;
    while at < bytes.len() {
        let Some(body) = metadata_body_length(bytes, at) else {
            return Some(at);
        };
        at = at.checked_add(1)?.checked_add(body)?;
    }
    None
}

/// The first signal block's flag byte and payload length, read **without** `spectrum::tape`.
///
/// A standard-speed block's body is a pause word, a length word, and then the data — so the flag
/// is five bytes past the ID and the length word three. Read here by hand so the ROM below is
/// given arguments that owe nothing to the converter under test: if our framing disagreed with
/// this reading, the loader would be told the wrong length and the load would fail.
///
/// `Err` carries the ID that stopped the walk, so a file this sweep cannot grade is **named with
/// its reason** rather than counted.
fn first_standard_block(bytes: &[u8]) -> Result<(u8, u16), Option<u8>> {
    let at = first_signal_block(bytes).ok_or(None)?;
    let id = bytes.get(at).copied().ok_or(None)?;
    if id != STANDARD_SPEED_DATA {
        return Err(Some(id));
    }
    let low = bytes.get(at + 3).copied().ok_or(None)?;
    let high = bytes.get(at + 4).copied().ok_or(None)?;
    let flag = bytes.get(at + 5).copied().ok_or(None)?;

    // The declared length counts the flag byte and the parity byte; `LD-BYTES` is given the
    // number of bytes between them.
    let declared = u16::from_le_bytes([low, high]);
    Ok((flag, declared.checked_sub(2).ok_or(None)?))
}

fn write_bytes(machine: &mut Spectrum, at: u16, bytes: &[u8]) {
    for (offset, &byte) in (0..).zip(bytes) {
        machine.memory_mut().write(at.wrapping_add(offset), byte);
    }
}

/// The loader stub: set the loader's arguments, call it, and park somewhere `PC` names.
fn loader_stub(flag: u8, length: u16) -> Vec<u8> {
    let mut code = vec![0xF3]; // DI
    code.extend([0xDD, 0x21]); // LD IX,nn
    code.extend(DESTINATION.to_le_bytes());
    code.push(0x11); // LD DE,nn
    code.extend(length.to_le_bytes());
    code.extend([0x3E, flag]); // LD A,flag
    code.push(0x37); // SCF — load rather than verify
    code.push(0xCD); // CALL nn
    code.extend(LD_BYTES.to_le_bytes());
    code.push(0xF3); // DI — undo SA/LD-RET's EI
    code.push(0xD2); // JP NC,nn
    code.extend(FAILED.to_le_bytes());
    code.push(0xC3); // JP nn
    code.extend(LOADED.to_le_bytes());
    code
}

fn park(at: u16) -> Vec<u8> {
    let mut code = vec![0xC3];
    code.extend(at.to_le_bytes());
    code
}

fn elapsed(machine: &Spectrum) -> u64 {
    machine.frames() * u64::from(T_STATES_PER_FRAME) + u64::from(machine.frame_t_state())
}

/// Run the ROM's loader against `tape` and report where it parked.
fn load_first_block(rom: &[u8], tape: Tape, flag: u8, length: u16) -> Option<u16> {
    let mut machine = Spectrum::new(rom).expect("the 48K ROM is one page");
    write_bytes(&mut machine, STUB, &loader_stub(flag, length));
    write_bytes(&mut machine, LOADED, &park(LOADED));
    write_bytes(&mut machine, FAILED, &park(FAILED));
    machine.set_cpu_state(CpuState {
        pc: STUB,
        sp: STACK,
        iff1: false,
        iff2: false,
        ..CpuState::default()
    });
    machine.insert_tape(tape);
    machine.tape_mut().play();

    let start = elapsed(&machine);
    while elapsed(&machine) - start < BUDGET {
        let pc = machine.cpu_state().pc;
        if pc == LOADED || pc == FAILED {
            return Some(pc);
        }
        machine.step();
    }
    None
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into()
}

#[test]
fn every_tzx_in_the_corpus_parses() {
    // A file this converter refuses is the signal every strict ruling in `tape/tzx.rs` names as
    // its escape hatch — an unhandled block ID, a length the description gives two answers for,
    // a ceiling reached by a real tape. So a refusal fails loudly with the reason rather than
    // being counted as a tolerable outcome, because the reason is the whole finding.
    let files = corpus();
    if files.is_empty() {
        announce_empty();
        return;
    }

    const MODELS: [Model; 2] = [Model::Spectrum48K, Model::Spectrum128];

    let mut pulses = 0_usize;
    let mut graded = 0_usize;
    for path in &files {
        let name = name_of(path);
        let bytes = std::fs::read(path).expect("a corpus file that was just listed");
        for model in MODELS {
            let tape = tzx::parse(&bytes, model)
                .unwrap_or_else(|error| panic!("{name} was refused on a {model:?}: {error}"));
            assert!(!tape.pulses().is_empty(), "{name} carries no signal at all");
            pulses += tape.pulses().len();
            graded += 1;
        }
    }
    println!("tzx corpus: {} files, {pulses} half-periods", files.len());

    // The floor, and it sits **after** the early return above rather than replacing it: an
    // absent corpus still skips, deliberately, because there is no fetch to name for `.tzx` and
    // a gate that fails on every fresh clone is worse than one that says it did nothing. What
    // this catches is the other failure — a corpus that is *present* and not actually graded.
    assert_eq!(
        graded,
        files.len() * MODELS.len(),
        "every file must be converted on every model, or this sweep counted what it skipped"
    );
}

#[test]
fn the_rom_loads_the_first_block_of_every_tzx_that_opens_with_one() {
    // **The assertion that owes nothing to this project.** A standard-speed block ends with the
    // XOR of every byte before it, applied by whoever recorded the tape, and `LD-BYTES` checks
    // it. So a block split one byte wrong, a bit order reversed, or a pilot tone the loader
    // cannot sync to all show up as a failed load — against a file we did not write and a
    // loader we did not write.
    //
    // Most turbo-loaded games open with a standard-speed BASIC loader, which is exactly the
    // block this reaches. Files that do not are counted and named rather than skipped in
    // silence, so the sweep says what it covered rather than only that nothing failed.
    let files = corpus();
    if files.is_empty() {
        announce_empty();
        return;
    }
    let Some(rom) = sinclair_rom() else {
        return;
    };

    let mut loaded = 0_usize;
    let mut skipped = Vec::new();
    for path in &files {
        let name = name_of(path);
        let bytes = std::fs::read(path).expect("a corpus file that was just listed");
        let (flag, length) = match first_standard_block(&bytes) {
            Ok(block) => block,
            // Named with the ID that stopped the walk. A skip reading only as a count is how
            // this sweep came to grade one of the two files it was handed without saying so.
            Err(id) => {
                skipped.push(format!("{name} (first signal block is {id:?})"));
                continue;
            }
        };
        let tape = tzx::parse(&bytes, Model::Spectrum48K).expect("it parsed in the sweep above");

        assert_eq!(
            load_first_block(&rom, tape, flag, length),
            Some(LOADED),
            "{name}: the ROM's own loader did not accept the first block of this file"
        );
        loaded += 1;
    }

    println!(
        "tzx corpus: the ROM loaded the first block of {loaded} of {} files; \
         {} did not reach a standard-speed block: {skipped:?}",
        files.len(),
        skipped.len()
    );

    // The floor this sweep went without. Every file could fail to open with a standard-speed
    // block — that is what a turbo-only corpus looks like — and the sweep would have reported a
    // pass having run the ROM zero times. It is deliberately *not* `loaded == files.len()`: a
    // file whose first signal block is a turbo one genuinely cannot be graded by `LD-BYTES`, and
    // demanding otherwise would make a legitimate corpus red.
    assert!(
        loaded > 0,
        "{} files present and the ROM was never run: {skipped:?}",
        files.len()
    );
}
