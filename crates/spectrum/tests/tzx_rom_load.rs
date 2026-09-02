//! The real ROM loads a `.tzx` through the `EAR` bit, and then runs what it loaded.
//!
//! # What this is
//!
//! `tape_rom_load.rs` is the M6 gate for `.tap` — **T2** (the ROM's own `LD-BYTES` loads a tape
//! we generated) and **T3** (a program we wrote is loaded from tape and then *executes*). This
//! file is the same two tiers through the `.tzx` front door, and it adds one thing `.tap` cannot
//! express at all.
//!
//! | | |
//! |---|---|
//! | **T2** | `ID 10`, `ID 11`, and a block assembled from `ID 12` + `ID 13` + `ID 14` all load |
//! | **T3** | a program we wrote, stored as a `.tzx`, loaded by the ROM and then executed |
//! | T4 | a **turbo** game reaches its title screen — observation, corpus-dependent, **not done** |
//!
//! The third T2 case is the interesting one. A real turbo loader's tape is not one block; it is
//! a pilot tone, then a sync sequence, then a data run, each written down separately, because
//! that is the only way a non-standard sync can be described. Assembling a standard block out of
//! `ID 12`, `ID 13` and `ID 14` and handing it to the ROM grades all three primitives at once
//! against a loader this project did not write — if any of their field offsets were wrong, the
//! ROM would not sync.
//!
//! # Why this grades the machine and a ROM trap would not
//!
//! `docs/M6.md` Decision 4, unchanged: the ROM's edge-counting loop executes `IN A,($FE)`
//! thousands of times, each a real port machine cycle through [`spectrum::ula`], each contended
//! by the four-case I/O rule, each reading bit 6 of a level that is a function of the frame
//! clock's absolute position. **Nothing here supplies a byte to the CPU by any route other than
//! a port read**, and `no_shortcut_past_the_ear_bit_for_a_tzx_either` is the standing assertion
//! that says so for this format too.
//!
//! # What is not graded here
//!
//! **Timing precision**, exactly as for `.tap`: the ROM's thresholds are hundreds of T-states
//! wide, so a converter some way off would still load. `tzx_rom_timings.rs` grades the values.
//!
//! And **a genuinely turbo block** — one whose bits are faster than the ROM's — because the ROM
//! cannot read one. Nothing *the ROM loads* can be turbo, so nothing in this file grades the five
//! timing fields of `ID 11` at anything other than the ROM's own values, where they are
//! indistinguishable from constants.
//!
//! > That used to read *"Nothing in this repository can"*, and it was true until
//! > `tzx_turbo_load.rs` landed. It supplies the missing half: a loader written here, running from
//! > RAM, counting edges on `IN A,($FE)` itself, decoding a whole screen at pilot 1400 / sync 500 /
//! > bit0 500 / bit1 1200 — and, with its own thresholds retuned, at up to 3.56× the ROM's data
//! > rate. Its `a_turbo_block_the_rom_cannot_read_is_decoded_by_a_loader_of_our_own` is the gate,
//! > and `the_files_one_bit_length_is_what_drives_the_pulse_train` is the negative control this
//! > file could not write.

use spectrum::tape::{Tape, tzx};
use spectrum::timing::T_STATES_PER_FRAME;
use spectrum::{Model, Spectrum};
use z80::CpuState;

/// `LD-BYTES`, the ROM's tape loader.
///
/// Entered with `IX` at the destination, `DE` holding the length, `A` the expected flag byte and
/// **carry set** for load-rather-than-verify.
const LD_BYTES: u16 = 0x0556;

/// The flag byte of a data block, which is what every block below is.
const DATA_FLAG: u8 = 0xFF;

/// Where the loader stub lives: uncontended RAM in the top slot, clear of everything else.
const STUB: u16 = 0xC000;
/// Where the stub parks after a load that reported success.
const LOADED: u16 = 0xC100;
/// Where it parks after one that reported failure, so the two are told apart by `PC` alone.
const FAILED: u16 = 0xC200;
/// Where the tape's contents are loaded to.
const DESTINATION: u16 = 0x9000;
/// Where a loaded program writes its signature. Clear of the destination and of the stub.
const SIGNATURE: u16 = 0xA000;
/// The stub's stack. Uncontended, and clear of every other address here.
const STACK: u16 = 0xBF00;

/// T-states to allow. A data block is 3224 pilot edges of 2168 T-states plus its bits, so a
/// little over 7 million; this is comfortably more and still under three seconds of emulated
/// time.
const BUDGET: u64 = 12_000_000;

/// The bytes T2 puts on tape. Distinct, non-zero, and not a run: a loader that dropped a byte or
/// repeated one would produce a different array rather than a plausible one.
const PAYLOAD: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];

/// The ten-byte `.tzx` header: `"ZXTape!"`, the end-of-text marker, and revision 1.20.
const HEADER: [u8; 10] = [b'Z', b'X', b'T', b'a', b'p', b'e', b'!', 0x1A, 0x01, 0x14];

/// The ROM's standard timings, as the format description's `ID 11` defaults give them.
const PILOT: u16 = 2168;
const SYNC_FIRST: u16 = 667;
const SYNC_SECOND: u16 = 735;
const BIT_ZERO: u16 = 855;
const BIT_ONE: u16 = 1710;
const DATA_PILOT_PULSES: u16 = 3223;

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

/// The bytes that go on tape: the flag, the payload, and the parity byte the ROM checks.
///
/// The parity is computed here rather than taken from the converter, because the ROM checks it
/// and a wrong one is how a load reports failure — which is the negative control below.
fn block_data(payload: &[u8]) -> Vec<u8> {
    let mut data = vec![DATA_FLAG];
    data.extend_from_slice(payload);
    data.push(payload.iter().fold(DATA_FLAG, |sum, &byte| sum ^ byte));
    data
}

/// A three-byte little-endian length, which is how `ID 11` and `ID 14` size their payloads.
fn length_24(bytes: &[u8]) -> Vec<u8> {
    let length = u32::try_from(bytes.len()).expect("a short block");
    length
        .to_le_bytes()
        .get(..3)
        .expect("a 24-bit length")
        .to_vec()
}

/// A `.tzx` holding one `ID 10 - Standard Speed Data Block`.
fn standard_speed(payload: &[u8]) -> Vec<u8> {
    let data = block_data(payload);
    let mut file = HEADER.to_vec();
    file.push(0x10);
    file.extend(0_u16.to_le_bytes()); // 0x00 WORD pause after this block, ms
    file.extend(
        u16::try_from(data.len())
            .expect("a short block")
            .to_le_bytes(),
    ); // 0x02 WORD
    file.extend(data); // 0x04 BYTE[N]
    file
}

/// A `.tzx` holding one `ID 11 - Turbo Speed Data Block` carrying the ROM's own values.
fn turbo_speed(payload: &[u8]) -> Vec<u8> {
    let data = block_data(payload);
    let mut file = HEADER.to_vec();
    file.push(0x11);
    file.extend(PILOT.to_le_bytes()); //             0x00 WORD
    file.extend(SYNC_FIRST.to_le_bytes()); //        0x02 WORD
    file.extend(SYNC_SECOND.to_le_bytes()); //       0x04 WORD
    file.extend(BIT_ZERO.to_le_bytes()); //          0x06 WORD
    file.extend(BIT_ONE.to_le_bytes()); //           0x08 WORD
    file.extend(DATA_PILOT_PULSES.to_le_bytes()); // 0x0A WORD
    file.push(8); //                                 0x0C BYTE  used bits in the last byte
    file.extend(0_u16.to_le_bytes()); //             0x0D WORD  pause, ms
    file.extend(length_24(&data)); //                0x0F BYTE[3]
    file.extend(data); //                            0x12 BYTE[N]
    file
}

/// A `.tzx` that builds a standard block out of the three primitive signal blocks.
///
/// This is the shape a real turbo loader's tape has, because a non-standard sync sequence can
/// only be written down as its own block. Here the three primitives are given the ROM's own
/// numbers, so what comes out must be indistinguishable from a standard block — which is what
/// makes the ROM able to grade it.
fn assembled_from_primitives(payload: &[u8]) -> Vec<u8> {
    let data = block_data(payload);
    let mut file = HEADER.to_vec();

    // ID 12 - Pure Tone: 0x00 WORD length of one pulse, 0x02 WORD number of pulses.
    file.push(0x12);
    file.extend(PILOT.to_le_bytes());
    file.extend(DATA_PILOT_PULSES.to_le_bytes());

    // ID 13 - Pulse sequence: 0x00 BYTE number of pulses, 0x01 WORD[N] their lengths.
    file.push(0x13);
    file.push(2);
    file.extend(SYNC_FIRST.to_le_bytes());
    file.extend(SYNC_SECOND.to_le_bytes());

    // ID 14 - Pure Data Block: 0x00 WORD zero bit, 0x02 WORD one bit, 0x04 BYTE used bits,
    // 0x05 WORD pause, 0x07 BYTE[3] length, 0x0A BYTE[N] data.
    file.push(0x14);
    file.extend(BIT_ZERO.to_le_bytes());
    file.extend(BIT_ONE.to_le_bytes());
    file.push(8);
    file.extend(0_u16.to_le_bytes());
    file.extend(length_24(&data));
    file.extend(data);
    file
}

/// The loader stub: set the loader's arguments, call it, and park somewhere `PC` names.
///
/// `DI` twice on purpose. The first is redundant — `LD-BYTES` does its own — and is there so
/// nothing can be accepted between entry and it. The second matters: `SA/LD-RET` executes `EI`
/// on the way out, so without it the machine would leave this stub with interrupts on.
fn loader_stub(destination: u16, length: u16, after_load: u16) -> Vec<u8> {
    let mut code = vec![0xF3]; // DI
    code.extend([0xDD, 0x21]); // LD IX,nn
    code.extend(destination.to_le_bytes());
    code.push(0x11); // LD DE,nn
    code.extend(length.to_le_bytes());
    code.extend([0x3E, DATA_FLAG]); // LD A,0xFF
    code.push(0x37); // SCF — load rather than verify
    code.push(0xCD); // CALL nn
    code.extend(LD_BYTES.to_le_bytes());
    code.push(0xF3); // DI — undo SA/LD-RET's EI
    code.push(0xD2); // JP NC,nn — carry clear means the load failed
    code.extend(FAILED.to_le_bytes());
    code.push(0xC3); // JP nn
    code.extend(after_load.to_le_bytes());
    code
}

/// `JP <here>` — a one-instruction park loop, so an address is a verdict.
fn park(at: u16) -> Vec<u8> {
    let mut code = vec![0xC3];
    code.extend(at.to_le_bytes());
    code
}

fn write_bytes(machine: &mut Spectrum, at: u16, bytes: &[u8]) {
    for (offset, &byte) in (0..).zip(bytes) {
        machine.memory_mut().write(at.wrapping_add(offset), byte);
    }
}

/// A machine holding the ROM, the stub, and `tape`, positioned to run the stub.
fn machine_with(
    rom: &[u8],
    tape: Tape,
    destination: u16,
    length: u16,
    after_load: u16,
) -> Spectrum {
    let mut machine = Spectrum::new(rom).expect("the 48K ROM is one page");
    write_bytes(
        &mut machine,
        STUB,
        &loader_stub(destination, length, after_load),
    );
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
    machine
}

/// T-states since power-on, so a budget can span the hundred frames a block takes.
fn elapsed(machine: &Spectrum) -> u64 {
    machine.frames() * u64::from(T_STATES_PER_FRAME) + u64::from(machine.frame_t_state())
}

/// Run until `PC` reaches one of `targets`, and return which. `None` if the budget ran out.
fn run_until(machine: &mut Spectrum, targets: &[u16]) -> Option<u16> {
    let start = elapsed(machine);
    while elapsed(machine) - start < BUDGET {
        let pc = machine.cpu_state().pc;
        if targets.contains(&pc) {
            return Some(pc);
        }
        machine.step();
    }
    None
}

fn read_bytes(machine: &Spectrum, at: u16, length: usize) -> Vec<u8> {
    (0..length)
        .map(|offset| {
            let offset = u16::try_from(offset).expect("a short read");
            machine.memory().read(at.wrapping_add(offset))
        })
        .collect()
}

fn tape_from(file: &[u8]) -> Tape {
    tzx::parse(file, Model::Spectrum48K).expect("a well-formed .tzx")
}

// ---------------------------------------------------------------------------------------
// T2 — the ROM's loader accepts a tape we generated, in each of the three shapes
// ---------------------------------------------------------------------------------------

#[test]
fn the_roms_own_loader_reads_a_tzx_through_the_ear_bit() {
    // Three files carrying the same block by three different routes through the format: as one
    // standard-speed block, as a turbo block whose every timing is a field in the file, and as
    // a pilot tone plus a sync sequence plus a data run — which is the shape a real turbo
    // loader's tape has, and the only one that grades `ID 12`, `ID 13` and `ID 14`.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let length = u16::try_from(PAYLOAD.len()).expect("a short payload");

    for (name, file) in [
        ("ID 10, standard speed", standard_speed(&PAYLOAD)),
        ("ID 11, turbo speed", turbo_speed(&PAYLOAD)),
        ("ID 12 + ID 13 + ID 14", assembled_from_primitives(&PAYLOAD)),
    ] {
        let mut machine = machine_with(&rom, tape_from(&file), DESTINATION, length, LOADED);
        assert_eq!(
            run_until(&mut machine, &[LOADED, FAILED]),
            Some(LOADED),
            "{name}: LD-BYTES did not report a successful load"
        );
        assert_eq!(
            read_bytes(&machine, DESTINATION, PAYLOAD.len()),
            PAYLOAD,
            "{name}: the bytes that arrived are not the bytes we recorded"
        );
        assert_eq!(machine.fault(), None, "{name}");
    }
}

#[test]
fn the_three_shapes_are_the_same_signal() {
    // ...and they are the same signal, not merely three that happen to load. The ROM's
    // thresholds are hundreds of T-states wide, so "it loaded" is a weak equality; this is the
    // strong one, and it is what would go red if one route drifted from the others.
    let standard = tape_from(&standard_speed(&PAYLOAD));
    let turbo = tape_from(&turbo_speed(&PAYLOAD));
    let assembled = tape_from(&assembled_from_primitives(&PAYLOAD));

    assert_eq!(standard.pulses(), turbo.pulses());
    assert_eq!(standard.pulses(), assembled.pulses());
    assert_eq!(
        standard.pulses().len(),
        usize::from(DATA_PILOT_PULSES) + 2 + 16 * (PAYLOAD.len() + 2),
        "a data pilot, the sync pair, and sixteen half-periods per byte"
    );
}

#[test]
fn a_corrupt_block_is_refused_rather_than_loaded() {
    // The negative control, and the reason the test above proves anything. Without it, a loader
    // that reported success unconditionally — or a gate that read the destination before
    // anything wrote it — would look identical to a working one.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let mut file = standard_speed(&PAYLOAD);
    let last = file.len() - 1;
    let parity = file.get_mut(last).expect("a non-empty file");
    *parity ^= 0xFF; // a wrong parity byte, which is what a damaged tape looks like

    let length = u16::try_from(PAYLOAD.len()).expect("a short payload");
    let mut machine = machine_with(&rom, tape_from(&file), DESTINATION, length, LOADED);
    assert_eq!(
        run_until(&mut machine, &[LOADED, FAILED]),
        Some(FAILED),
        "LD-BYTES accepted a block whose parity byte is wrong"
    );
}

#[test]
fn no_shortcut_past_the_ear_bit_for_a_tzx_either() {
    // `docs/M6.md` Decision 4's standing assertion, restated for this format. The tape is
    // inserted and **never played**, so the `EAR` line never moves; if any path existed that
    // handed the loader its block without the signal, this would still load and this test would
    // go red.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let length = u16::try_from(PAYLOAD.len()).expect("a short payload");
    let file = standard_speed(&PAYLOAD);
    let mut machine = machine_with(&rom, tape_from(&file), DESTINATION, length, LOADED);
    machine.tape_mut().stop();

    assert_eq!(
        run_until(&mut machine, &[LOADED, FAILED]),
        None,
        "a load completed with the tape stopped, so something other than the signal supplied it"
    );
    assert_eq!(
        read_bytes(&machine, DESTINATION, PAYLOAD.len()),
        [0; PAYLOAD.len()],
        "nothing may reach the destination without the signal"
    );
    // And it ran out of budget in the *right* place: `LD-START` retries forever until BREAK, so
    // a machine genuinely waiting for an edge is still inside the ROM — a different observation
    // from having wandered off, and both otherwise read as "the budget expired".
    assert!(
        machine.cpu_state().pc < 0x4000,
        "the machine should still be in the ROM's edge-detection loop, not at {:#06X}",
        machine.cpu_state().pc
    );
}

// ---------------------------------------------------------------------------------------
// T3 — a program we wrote, loaded from a `.tzx` and executed
// ---------------------------------------------------------------------------------------

/// What the loaded program computes and stores. It appears **nowhere** in the tape's bytes,
/// which is what separates "the program ran" from "the bytes arrived".
const COMPUTED_SIGNATURE: u16 = 0x2345;
const ADDEND: u16 = 0x1234;
const AUGEND: u16 = 0x1111;

/// The program T3 puts on tape: add two numbers, store the sum, and park.
fn payload_program(at: u16) -> Vec<u8> {
    let mut code = vec![0x21]; // LD HL,nn
    code.extend(ADDEND.to_le_bytes());
    code.push(0x11); // LD DE,nn
    code.extend(AUGEND.to_le_bytes());
    code.push(0x19); // ADD HL,DE
    code.push(0x22); // LD (nn),HL
    code.extend(SIGNATURE.to_le_bytes());
    let park_at = at + u16::try_from(code.len()).expect("a short program");
    code.extend(park(park_at));
    code
}

#[test]
fn a_program_we_wrote_loads_from_a_tzx_and_runs() {
    // The milestone's gate, through the format that will actually carry a game. A program we
    // wrote, recorded as a **turbo** block — so every timing the ROM's loader measures came out
    // of a field in the file — loaded by the ROM's own routine reading the `EAR` bit, jumped to,
    // and observed to have computed something.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let program = payload_program(DESTINATION);
    assert!(
        !program
            .windows(2)
            .any(|pair| pair == COMPUTED_SIGNATURE.to_le_bytes()),
        "the signature must not appear in the program's own bytes, or loading would prove it"
    );

    let file = turbo_speed(&program);
    let length = u16::try_from(program.len()).expect("a short program");
    // The stub jumps to the loaded code rather than to a park, so the machine runs it.
    let mut machine = machine_with(&rom, tape_from(&file), DESTINATION, length, DESTINATION);

    let park_at = DESTINATION + length - 3;
    assert_eq!(
        run_until(&mut machine, &[park_at, FAILED]),
        Some(park_at),
        "the loaded program did not reach its own park loop"
    );
    assert_eq!(
        read_bytes(&machine, DESTINATION, program.len()),
        program,
        "the program in RAM is not the program on the tape"
    );
    assert_eq!(
        u16::from_le_bytes([
            machine.memory().read(SIGNATURE),
            machine.memory().read(SIGNATURE + 1),
        ]),
        COMPUTED_SIGNATURE,
        "the program did not run, or ran and computed the wrong answer"
    );
    assert_eq!(machine.fault(), None);
}

#[test]
fn nothing_is_at_the_signature_address_before_the_program_runs() {
    // The positive control for the gate above: the address it reads is empty until the loaded
    // program writes it, so a stale value cannot be mistaken for a result. `docs/STATUS.md`
    // records why this is not optional — "a count of zero and an absence of the subject are the
    // same observation".
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let program = payload_program(DESTINATION);
    let file = turbo_speed(&program);
    let length = u16::try_from(program.len()).expect("a short program");
    let machine = machine_with(&rom, tape_from(&file), DESTINATION, length, DESTINATION);
    assert_eq!(read_bytes(&machine, SIGNATURE, 2), [0, 0]);
}

// ---------------------------------------------------------------------------------------
// The structural blocks, through the real loader
// ---------------------------------------------------------------------------------------

#[test]
fn a_loop_block_makes_the_rom_load_the_same_block_twice() {
    // The control-flow half, graded by the ROM rather than by counting pulses. A two-pass loop
    // over one data block is a tape carrying that block twice, so a machine that loads, resets
    // its destination and loads again must see the same bytes both times — which it can only do
    // if the second pass really is on the tape.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let data = block_data(&PAYLOAD);
    let mut file = HEADER.to_vec();
    file.extend([0x24, 0x02, 0x00]); // ID 24 - Loop start, two repetitions
    file.push(0x10); //                 ID 10 - Standard speed data block
    file.extend(0_u16.to_le_bytes());
    file.extend(
        u16::try_from(data.len())
            .expect("a short block")
            .to_le_bytes(),
    );
    file.extend(data);
    file.push(0x25); //                 ID 25 - Loop end

    let tape = tape_from(&file);
    let single = tape_from(&standard_speed(&PAYLOAD));
    assert_eq!(
        tape.pulses().len(),
        2 * single.pulses().len(),
        "two passes must be twice the signal"
    );

    let length = u16::try_from(PAYLOAD.len()).expect("a short payload");
    let mut machine = machine_with(&rom, tape, DESTINATION, length, LOADED);
    assert_eq!(
        run_until(&mut machine, &[LOADED, FAILED]),
        Some(LOADED),
        "the first pass did not load"
    );
    assert_eq!(read_bytes(&machine, DESTINATION, PAYLOAD.len()), PAYLOAD);

    // Wipe the destination and load the loop's second pass with a fresh stub.
    write_bytes(&mut machine, DESTINATION, &[0; PAYLOAD.len()]);
    machine.set_cpu_state(CpuState {
        pc: STUB,
        sp: STACK,
        iff1: false,
        iff2: false,
        ..CpuState::default()
    });
    assert_eq!(
        run_until(&mut machine, &[LOADED, FAILED]),
        Some(LOADED),
        "the loop's second pass did not load"
    );
    assert_eq!(
        read_bytes(&machine, DESTINATION, PAYLOAD.len()),
        PAYLOAD,
        "the second pass carried different bytes from the first"
    );
}
