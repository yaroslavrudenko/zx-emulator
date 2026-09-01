//! Gate: the AY's two ports, driven by a guest executing real `OUT`s and `IN`s.
//!
//! # What is graded, and at what evidence class
//!
//! | | Class | Why |
//! |---|---|---|
//! | A guest's `OUT (C),A` reaches the chip through the CPU, the bus and the decode | **proven** | Nothing about the chip's own tests could see this: `ay.rs` calls `select`/`write` directly |
//! | A guest's `IN A,(C)` reads a register back | **proven**, and it is the arm that would silently not exist | The AY's read arm sits *before* the floating-bus fallback in `Ula::in_port`. Placed after it, the machine reads as *"the chip is write-only"* — silent, plausible, and only visible here |
//! | A 48K answers neither port | **proven** | The chip is absent, not idle |
//! | `R15` does not exist, and a read of it floats | **proven** for the behaviour, **ruling** for the value |
//! | The write masks, through the guest's own route | **derived** — graded against the transcription |
//! | **Which address lines decode the two ports** | **derived, and it is the weakest claim in M7** | `docs/M7.md`: *"no source was found stating which lines decode `0xFFFD` and `0xBFFD`."* Everything below grades the machine against that inference and **cannot** discover the inference is wrong |
//!
//! # The decode, and what a green run here is worth
//!
//! The two published addresses are well attested and the *rule* behind them is not. So the
//! family assertions below — that `0xFDFD` selects and `0xFFFF` does not — grade this crate
//! against `ula::AY_PORT_MASK` and nothing else. They are worth having for the same reason the
//! `0x7FFD` family assertions are: a mask and an equality behave identically on the published
//! address and differently everywhere else, so writing the mask is a decision that needs a
//! gate even when the mask itself is unsourced. **What would settle the mask is a schematic or
//! software observed to depend on it, and neither is in reach.**

mod common;
mod m7_common;

use common::machine;
use m7_common::{
    AY_SELECT_PORT, AY_WRITE_PORT, OUT_C_A_STEPS, ay_peek as peek, ay_peek_via as peek_via,
    ay_poke as poke, ay_poke_via as poke_via, machine_128, out_c_a, run_program,
};

/// Where the test programs are assembled: bank 2, which no paging value moves.
const PROGRAM: u16 = common::PROLOGUE;

/// The canonical register-select and register-read port.
const AY_SELECT: u16 = AY_SELECT_PORT;

/// The canonical register-write port.
const AY_WRITE: u16 = AY_WRITE_PORT;

/// The paging port, which shares no address with either of the AY's.
const PAGING: u16 = 0x7FFD;

/// What a read of a register the chip does not have returns — the undriven bus.
///
/// Transcribed as the literal the machine's own convention produces, not imported, so that a
/// change to `FLOATING_BUS_BYTE` shows up here as a disagreement rather than as agreement.
const FLOATS: u8 = 0xFF;

/// Registers the AY-3-8912 has, `R0`-`R14`.
const REGISTERS: u8 = 15;

#[test]
fn a_guest_out_reaches_the_chip_and_a_guest_in_reads_it_back() {
    // The wiring, both directions, and the assertion that would be green with either arm of
    // `Ula::out_port` / `Ula::in_port` missing entirely.
    let mut machine = machine_128();
    poke(&mut machine, 7, 0b0011_1110);
    assert_eq!(
        machine.ay().and_then(|ay| ay.register(7)),
        Some(0b0011_1110),
        "the write must reach the register the latch names"
    );
    assert_eq!(peek(&mut machine), 0b0011_1110, "and read back through IN");
    assert_eq!(
        machine.ay().map(spectrum::Ay::selected),
        Some(7),
        "and the latch must hold what was written to it"
    );
}

#[test]
fn the_select_latch_decides_where_a_write_lands() {
    // Two writes through one data port, distinguished only by what was latched between them.
    // A model that ignored the latch would put both in the same place.
    let mut machine = machine_128();
    poke(&mut machine, 0, 0xAA);
    poke(&mut machine, 2, 0x55);
    let ay = machine.ay().expect("a 128 has a chip");
    assert_eq!(ay.register(0), Some(0xAA));
    assert_eq!(ay.register(2), Some(0x55));
    assert_eq!(ay.register(4), Some(0), "and nowhere else");
}

#[test]
fn a_48k_answers_neither_port() {
    // The chip is absent, not idle. A model that carried one anyway would let 48K software
    // write registers into hardware the machine does not contain — silently, since nothing on
    // a 48K would ever read them back to notice.
    let mut machine = machine();
    assert!(machine.ay().is_none());
    poke(&mut machine, 7, 0x3F);
    assert!(machine.ay().is_none(), "and still none after a write");
    assert_eq!(
        peek(&mut machine),
        FLOATS,
        "a 48K's 0xFFFD read is an undecoded port, which floats"
    );
}

#[test]
fn the_chip_has_fifteen_registers_and_the_sixteenth_is_not_one_of_them() {
    // `.z80` version 3 reserves sixteen bytes for a chip with fifteen registers, and
    // `docs/M7.md` Decision 6 names the trap: whatever a model puts in the sixteenth
    // *"round-trips perfectly and is invisible to every round trip"*. So it is asserted here,
    // where a round trip is not what is doing the looking.
    let mut machine = machine_128();
    poke(&mut machine, REGISTERS - 1, 0xA5);
    assert_eq!(
        machine.ay().and_then(|ay| ay.register(REGISTERS - 1)),
        Some(0xA5),
        "R14 is the one I/O port an AY-3-8912 has"
    );

    poke(&mut machine, REGISTERS, 0x5A);
    assert_eq!(
        machine.ay().and_then(|ay| ay.register(REGISTERS)),
        None,
        "R15 is port B, which an AY-3-8912 does not have"
    );
    assert_eq!(
        peek(&mut machine),
        FLOATS,
        "and a guest reading it gets the bus, because nothing is driving it"
    );
    assert_eq!(
        machine.ay().and_then(|ay| ay.register(REGISTERS - 1)),
        Some(0xA5),
        "the discarded write must not have landed somewhere else instead"
    );
}

#[test]
fn selecting_an_address_the_chip_does_not_decode_behaves_as_an_absent_register() {
    // Two mechanisms that are distinct on the hardware and identical from the CPU: `R15`,
    // which an `-8910` has and an `-8912` does not, and an address of 16 or more, which
    // deselects the chip so several can share a bus. `ay::ABSENT_REGISTER` documents the merge;
    // this is what holds it.
    let mut machine = machine_128();
    poke(&mut machine, 3, 0x0C);
    for address in [16_u8, 64, 200, 255] {
        poke(&mut machine, address, 0x77);
        assert_eq!(
            peek_via(&mut machine, AY_SELECT),
            FLOATS,
            "address {address}"
        );
    }
    assert_eq!(
        machine.ay().and_then(|ay| ay.register(3)),
        Some(0x0C),
        "and none of those writes landed in a real register"
    );
}

#[test]
fn the_narrow_registers_drop_their_missing_bits_on_the_way_through_the_bus() {
    // Graded against the transcription in `ay::WRITE_MASK`, which is what it is worth. It is
    // worth having anyway because software **reads registers back**, so a wrong mask is a
    // wrong value handed to a guest rather than merely a wrong internal state — and this is
    // the path a guest actually takes.
    let expected: [u8; REGISTERS as usize] = [
        0xFF, 0x0F, 0xFF, 0x0F, 0xFF, 0x0F, 0x1F, 0xFF, 0x1F, 0x1F, 0x1F, 0xFF, 0xFF, 0x0F, 0xFF,
    ];
    let mut machine = machine_128();
    for (register, &mask) in expected.iter().enumerate() {
        let register = u8::try_from(register).expect("fifteen registers");
        poke(&mut machine, register, 0xFF);
        assert_eq!(peek(&mut machine), mask, "register {register}");
    }
}

// ---------------------------------------------------------------------------
// The decode. Everything below grades this crate against an inference.
// ---------------------------------------------------------------------------

#[test]
fn the_two_ports_are_told_apart_by_one_address_line() {
    // A14 and nothing else. A model that decoded them as equalities would behave identically
    // here and differently on every other member of either family, which is the whole reason
    // a mask is written rather than a `match` on two addresses.
    assert_eq!(AY_SELECT ^ AY_WRITE, 0x4000, "A14 is the only difference");

    let mut machine = machine_128();
    // Writing the *select* port with a register number does not write a register...
    poke(&mut machine, 1, 0x0A);
    run_program(&mut machine, PROGRAM, &out_c_a(AY_SELECT, 5), OUT_C_A_STEPS);
    assert_eq!(
        machine.ay().and_then(|ay| ay.register(1)),
        Some(0x0A),
        "an OUT to the select port must latch, not write"
    );
    assert_eq!(machine.ay().map(spectrum::Ay::selected), Some(5));
}

#[test]
fn the_decode_is_a_mask_and_the_family_answers_with_the_published_address() {
    // `0xFDFD` and `0xBDFD` differ from the published pair only in address lines the inferred
    // decode does not read, so on this model they behave identically. **This grades the mask
    // against itself and nothing else** — no source states the rule — and it is recorded that
    // way rather than presented as evidence about the hardware.
    let mut machine = machine_128();
    poke_via(&mut machine, 0xFDFD, 0xBDFD, 6, 0x1F);
    assert_eq!(
        machine.ay().and_then(|ay| ay.register(6)),
        Some(0x1F),
        "an address matching A15, A14 and A1 must reach the same chip"
    );
    assert_eq!(peek_via(&mut machine, 0xFDFD), 0x1F);
}

#[test]
fn an_address_with_a1_set_is_not_the_chips() {
    // The one line that separates the AY's family from most of the address space. `0xFFFF`
    // has A15 and A14 set exactly as `0xFFFD` does and differs in A1, so a decode that had
    // dropped A1 would answer it.
    let mut machine = machine_128();
    poke(&mut machine, 8, 0x0F);
    run_program(&mut machine, PROGRAM, &out_c_a(0xFFFF, 2), OUT_C_A_STEPS);
    assert_eq!(
        machine.ay().map(spectrum::Ay::selected),
        Some(8),
        "0xFFFF must not have relatched"
    );
    assert_eq!(peek_via(&mut machine, 0xFFFF), FLOATS, "nor answer a read");
}

#[test]
fn the_paging_port_and_the_chip_cannot_be_reached_by_one_address() {
    // `docs/M7.md` Decision 2 calls this a structural guarantee: the paging port needs A15
    // reset and both AY ports need it set, so no address is claimed by both. `ula.rs` asserts
    // it at compile time; this is the same fact through the machine, which is what catches a
    // decode that was *written* disjointly and *wired* wrongly.
    let mut machine = machine_128();
    poke(&mut machine, 0, 0x42);
    let before = machine.memory().slots();

    run_program(&mut machine, PROGRAM, &out_c_a(PAGING, 0x03), OUT_C_A_STEPS);
    assert_ne!(machine.memory().slots(), before, "the paging write took");
    assert_eq!(
        machine.ay().map(spectrum::Ay::selected),
        Some(0),
        "and left the sound chip's latch alone"
    );
    assert_eq!(machine.ay().and_then(|ay| ay.register(0)), Some(0x42));

    // And the converse: an AY write does not page.
    let map = machine.memory().slots();
    poke(&mut machine, 1, 0x07);
    assert_eq!(machine.memory().slots(), map);
}

#[test]
fn where_the_chip_and_the_ula_collide_the_chip_answers_the_read_and_both_take_the_write() {
    // **A ruling, not a measurement.** An address with A0 and A1 reset and A15, A14 set is
    // claimed by both devices, and on the hardware both drive the data bus at once. There is
    // no right answer to find by thinking harder, so `Ula::in_port` fixes an order and this
    // pins it — because an unpinned ruling is one somebody silently reverses.
    //
    // Nothing in reach exercises it: every published address for either port has A0 the other
    // way round, which is why this is the only place in the suite that names `0xFFFC`.
    let mut machine = machine_128();
    poke(&mut machine, 8, 0x0D);
    assert_eq!(
        peek_via(&mut machine, 0xFFFC),
        0x0D,
        "the chip decodes three address lines to the ULA's one, so it answers first"
    );

    // On a **write** both act, which is well defined and is why `out_port` is independent
    // `if`s rather than a `match`. `0xBFFC` is an AY data write *and* a border write.
    run_program(&mut machine, PROGRAM, &out_c_a(0xBFFC, 0x05), OUT_C_A_STEPS);
    assert_eq!(machine.border().index(), 0x05, "the ULA took bits 0-2");
    assert_eq!(
        machine.ay().and_then(|ay| ay.register(8)),
        Some(0x05),
        "and the chip took the same byte as register data"
    );
}

#[test]
fn a_reset_silences_the_chip() {
    // The reset line reaches the AY. A 128 reset while a tune was playing must come back
    // quiet, and every register — not only the amplitudes — must be back at power-on.
    let mut machine = machine_128();
    for register in 0..REGISTERS {
        poke(&mut machine, register, 0xFF);
    }
    assert_ne!(machine.ay().and_then(|ay| ay.register(0)), Some(0));

    machine.reset();
    let ay = machine
        .ay()
        .expect("a 128 still has its chip after a reset");
    for register in 0..REGISTERS {
        assert_eq!(ay.register(register), Some(0), "register {register}");
    }
    assert_eq!(ay.selected(), 0, "and the latch too");
}
