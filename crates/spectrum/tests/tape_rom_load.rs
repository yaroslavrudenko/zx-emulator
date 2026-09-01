//! **The M6 gate.** The real ROM loads a tape through the `EAR` bit, and then runs it.
//!
//! # What this is, in the project's own vocabulary
//!
//! `docs/M6.md` Decision 8 splits the milestone's evidence into four tiers, and this file is
//! **T2 and T3**:
//!
//! | | | |
//! |---|---|---|
//! | **T2** | the ROM's own `LD-BYTES` loads a tape we generated | measured — binary pass/fail against a program we did not write |
//! | **T3** | a program **we wrote** is loaded from tape by the ROM and then **executes** | measured |
//! | T4 | a real game reaches its title screen | observation, corpus-dependent, absent by default, **not done** |
//!
//! **T3 is what replaces "a real game runs" as the gate**, and that is a correction to
//! `docs/MACHINE.md`'s milestone table made in the open. T4 is observation in this project's
//! vocabulary and cannot be automated in a repository that may not carry games; what M6 does
//! is shrink the un-automatable residue from *the whole gate* to *the part that genuinely
//! cannot be committed*.
//!
//! # Why this grades the machine and a ROM trap would not
//!
//! The cheap way to pass a tape milestone is to watch for `PC` reaching `0x0556`, write the
//! block straight into the buffer `IX`/`DE` describe, and return with the flags set. It works,
//! it is about fifty lines, and **the gate would then be grading the trap** — because the trap
//! bypasses the ULA, the contention model, the frame clock, the interrupt window and the port
//! decoding, which is every part of the machine M5 could not grade.
//!
//! What actually happens below instead: the ROM's edge-counting loop executes `IN A,($FE)`
//! thousands of times, each one a real port machine cycle through [`spectrum::ula`], each one
//! contended by the four-case I/O rule, each one reading bit 6 out of a signal whose level is
//! a function of the frame clock's absolute position. Every byte that arrives does so because
//! the ROM measured an interval between two edges and got the same answer our converter
//! intended. **Nothing here supplies a byte to the CPU by any route other than a port read.**
//!
//! `docs/M6.md` also rules that if a trap is ever added as a debugging convenience it must be
//! **off by default and these gates must assert it is off**. There is no trap to assert
//! against yet; `no_shortcut_exists_past_the_ear_bit` is the standing assertion that the only
//! thing that makes a load work is the signal, and it is what would go red the day one landed
//! and was left on.
//!
//! # What is not graded here
//!
//! - **Timing precision.** The ROM's thresholds are hundreds of T-states wide, so a converter
//!   that was some way off would still load. `tape_rom_timings.rs` is what grades the values,
//!   against the ROM's own writer.
//! - **Contention during a load.** It is exercised on every one of those port reads and it is
//!   not graded, because the loader does not depend on it.
//! - **Turbo loaders.** `.tap` cannot represent one at any speed.

use spectrum::Spectrum;
use spectrum::tape::{Tape, tap};
use spectrum::timing::T_STATES_PER_FRAME;
use z80::CpuState;

/// `LD-BYTES`, the ROM's tape loader.
///
/// Entered with `IX` at the destination, `DE` holding the length, `A` the expected flag byte
/// and **carry set** for load-rather-than-verify. It performs its own `DI`, pushes
/// `SA/LD-RET` as its return, and comes back with carry set on success.
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

/// The bytes T2 puts on tape. Distinct, non-zero, and not a run: a loader that dropped a byte
/// or repeated one would produce a different array rather than a plausible one.
const PAYLOAD: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];

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

/// A `.tap` file holding one data block: a length word, the flag, `payload`, and its parity.
///
/// The parity byte is computed here rather than taken from the converter, because the ROM
/// checks it and a wrong one is how a load reports failure — which is the negative control
/// `a_corrupt_block_is_refused_rather_than_loaded` needs.
fn tap_file(payload: &[u8]) -> Vec<u8> {
    let parity = payload.iter().fold(DATA_FLAG, |sum, &byte| sum ^ byte);
    let length = u16::try_from(payload.len() + 2).expect("a short block");
    let mut file = length.to_le_bytes().to_vec();
    file.push(DATA_FLAG);
    file.extend_from_slice(payload);
    file.push(parity);
    file
}

/// The loader stub: set the loader's arguments, call it, and park somewhere `PC` names.
///
/// `DI` twice on purpose. The first is redundant — `LD-BYTES` does its own — and is there so
/// nothing can be accepted between entry and it. The second matters: `SA/LD-RET` executes
/// `EI` on the way out, so without it the machine would leave this stub with interrupts on and
/// vector out of the park loop.
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

fn write_bytes(machine: &mut Spectrum, at: u16, bytes: &[u8]) {
    for (offset, &byte) in (0..).zip(bytes) {
        machine.memory_mut().write(at.wrapping_add(offset), byte);
    }
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

// ---------------------------------------------------------------------------------------
// T2 — the ROM's loader accepts a tape we generated
// ---------------------------------------------------------------------------------------

#[test]
fn the_roms_own_loader_reads_our_tape_through_the_ear_bit() {
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let tape = tap::parse(&tap_file(&PAYLOAD)).expect("a well-formed block");
    let length = u16::try_from(PAYLOAD.len()).expect("a short payload");
    let mut machine = machine_with(&rom, tape, DESTINATION, length, LOADED);

    assert_eq!(
        run_until(&mut machine, &[LOADED, FAILED]),
        Some(LOADED),
        "LD-BYTES did not report a successful load"
    );
    assert_eq!(
        read_bytes(&machine, DESTINATION, PAYLOAD.len()),
        PAYLOAD,
        "the bytes that arrived are not the bytes we recorded"
    );
    assert_eq!(machine.fault(), None);
}

#[test]
fn a_corrupt_block_is_refused_rather_than_loaded() {
    // The negative control, and the reason the test above proves anything. Without it, a
    // loader that reported success unconditionally — or a gate that read the destination
    // before anything wrote it — would look identical to a working one.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let mut file = tap_file(&PAYLOAD);
    let last = file.len() - 1;
    file[last] ^= 0xFF; // a wrong parity byte, which is what a damaged tape looks like

    let tape = tap::parse(&file).expect("a structurally valid block");
    let length = u16::try_from(PAYLOAD.len()).expect("a short payload");
    let mut machine = machine_with(&rom, tape, DESTINATION, length, LOADED);

    assert_eq!(
        run_until(&mut machine, &[LOADED, FAILED]),
        Some(FAILED),
        "LD-BYTES accepted a block whose parity byte is wrong"
    );
}

#[test]
fn no_shortcut_exists_past_the_ear_bit() {
    // `docs/M6.md` Decision 4's standing assertion. The tape is inserted and **never
    // played**, so the `EAR` line never moves; if any path existed that handed the loader its
    // block without the signal — a `PC` trap at 0x0556, an auto-start that supplied data, a
    // future debugging convenience left switched on — this would still load and this test
    // would go red.
    //
    // It is deliberately the mirror of the gate above rather than a separate mechanism: same
    // ROM, same stub, same tape, same budget, and one difference.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let tape = tap::parse(&tap_file(&PAYLOAD)).expect("a well-formed block");
    let length = u16::try_from(PAYLOAD.len()).expect("a short payload");
    let mut machine = machine_with(&rom, tape, DESTINATION, length, LOADED);
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
    // And it ran out of budget in the *right* place. `LD-START` retries `LD-EDGE-1` forever
    // until BREAK is pressed, so a machine that is genuinely waiting for an edge is still
    // inside the ROM — which is a different observation from having wandered off, and both
    // would otherwise read as "the budget expired".
    assert!(
        machine.cpu_state().pc < 0x4000,
        "the machine should still be in the ROM's edge-detection loop, not at {:#06X}",
        machine.cpu_state().pc
    );
}

// ---------------------------------------------------------------------------------------
// T3 — a program we wrote, loaded from tape and executed
// ---------------------------------------------------------------------------------------

/// What the loaded program computes and stores. It appears **nowhere** in the tape's bytes,
/// which is what separates "the program ran" from "the bytes arrived".
const COMPUTED_SIGNATURE: u16 = 0x2345;

/// The two operands, which do appear in the tape and must not be mistaken for the answer.
const ADDEND: u16 = 0x1234;
const AUGEND: u16 = 0x1111;

/// The program T3 puts on tape: add two numbers, store the sum, and park.
///
/// It **computes** rather than copies, so a machine that loaded the bytes and never executed
/// them writes nothing to [`SIGNATURE`] — and a machine that executed them wrongly writes
/// something other than [`COMPUTED_SIGNATURE`]. Storing a constant would have been graded by
/// the load alone.
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
fn a_program_we_wrote_loads_from_tape_and_runs() {
    // The milestone's gate. A program we wrote, recorded as a `.tap`, loaded by the ROM's own
    // routine reading the `EAR` bit, jumped to, and observed to have computed something.
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

    let tape = tap::parse(&tap_file(&program)).expect("a well-formed block");
    let length = u16::try_from(program.len()).expect("a short program");
    // The stub jumps to the loaded code rather than to a park, so the machine runs it.
    let mut machine = machine_with(&rom, tape, DESTINATION, length, DESTINATION);

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
    // records why this is not optional — *"a count of zero and an absence of the subject are
    // the same observation"*.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let program = payload_program(DESTINATION);
    let tape = tap::parse(&tap_file(&program)).expect("a well-formed block");
    let length = u16::try_from(program.len()).expect("a short program");
    let machine = machine_with(&rom, tape, DESTINATION, length, DESTINATION);
    assert_eq!(read_bytes(&machine, SIGNATURE, 2), [0, 0]);
}
