//! The public state surface: `Cpu::state` / `Cpu::set_state` and the interrupt-enable
//! window they have to agree about.
//!
//! The FUSE corpus cannot reach any of this. Every vector loads a state, runs one
//! instruction and compares — it never loads a *second* state mid-flight, so the one thing
//! `set_state` must do beyond copying registers, namely reset the parts of the CPU that
//! are not registers, has no vector anywhere in the suite. That gap is the subject here.

mod common;

use common::machine::Machine;
use common::vectors::{MemoryBlock, Registers, Setup, State};

/// The address the one-instruction programs are assembled at.
const PROGRAM_START: u16 = 0x0000;

const EI: u8 = 0xFB;
const NOP: u8 = 0x00;

#[test]
fn ei_opens_the_one_instruction_interrupt_delay_window() {
    let mut machine = Machine::load(&program(&[EI, NOP]));

    machine.step();

    assert!(
        machine.ei_pending(),
        "after EI the CPU must not accept an interrupt until the following instruction has \
         run — games depend on `EI; RET` returning before the next frame interrupt lands"
    );
}

#[test]
fn the_ei_window_closes_after_the_next_instruction() {
    let mut machine = Machine::load(&program(&[EI, NOP]));

    machine.step();
    machine.step();

    assert!(
        !machine.ei_pending(),
        "the delay lasts exactly one instruction; leaving it open would postpone every \
         subsequent interrupt too"
    );
}

/// The reviewer's gap: loading a snapshot must not leave the previous machine's `EI`
/// window open.
#[test]
fn set_state_closes_the_pending_ei_window() {
    let mut machine = Machine::load(&program(&[EI, NOP]));
    machine.step();
    assert!(
        machine.ei_pending(),
        "precondition failed: EI did not open the window, so this test proves nothing"
    );

    machine.set_state(&Registers::default(), &State::default());

    assert!(
        !machine.ei_pending(),
        "set_state replaces the whole machine state, so it must close the EI window too. \
         Left stale, a freshly loaded snapshot swallows the first interrupt after the load \
         — one dropped frame interrupt, once, non-reproducibly, which is precisely the kind \
         of defect that gets blamed on the game instead of the emulator"
    );
}

/// Every field must survive a `set_state` -> `state` round trip.
///
/// The values are all distinct so that a mis-wiring in the seam — `de_shadow` written from
/// `hl_shadow`, `ix` and `iy` transposed — cannot pass by coincidence, which an all-zero or
/// all-`0xFF` fixture would happily do.
#[test]
fn set_state_round_trips_every_field() {
    let registers = Registers {
        af: 0x0102,
        bc: 0x0304,
        de: 0x0506,
        hl: 0x0708,
        af_shadow: 0x090a,
        bc_shadow: 0x0b0c,
        de_shadow: 0x0d0e,
        hl_shadow: 0x0f10,
        ix: 0x1112,
        iy: 0x1314,
        sp: 0x1516,
        pc: 0x1718,
    };
    let state = State {
        i: 0x19,
        r: 0x7f,
        iff1: true,
        iff2: false,
        im: 2,
        halted: true,
        t_states: 0,
    };
    let mut machine = Machine::load(&program(&[NOP]));

    machine.set_state(&registers, &state);
    let (got_registers, got_state) = machine.snapshot();

    assert_eq!(registers, got_registers, "registers did not round-trip");
    assert_eq!(
        State {
            // The T-state count belongs to the bus, not to the CPU state, so it is the one
            // field a round trip cannot preserve.
            t_states: got_state.t_states,
            ..state
        },
        got_state,
        "machine state did not round-trip",
    );
}

/// Assemble a program at [`PROGRAM_START`] with an otherwise-zeroed machine.
fn program(bytes: &[u8]) -> Setup {
    Setup {
        name: String::from("cpu_state"),
        registers: Registers {
            pc: PROGRAM_START,
            ..Registers::default()
        },
        state: State::default(),
        memory: vec![MemoryBlock {
            start: PROGRAM_START,
            bytes: bytes.to_vec(),
        }],
    }
}
