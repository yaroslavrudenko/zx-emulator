//! Gate: each block family spends its five repeat T-states on a **different** register's
//! address, and a contention model that used the wrong one is caught here.
//!
//! # Why this exists
//!
//! `docs/Z80-REFERENCE.md` records a rule found by reading a trace rather than a
//! specification:
//!
//! > `LDIR`/`CPIR`/`INIR`/`OTIR` and their decrementing twins spend their five repeat
//! > T-states on **the last address that was on the bus** — which is a different register
//! > in each family: `DE` for the transfers (the write), `HL` for the compares (the read),
//! > `HL` for the inputs (the write), and the **port** (`BC` after `B`'s decrement) for the
//! > outputs.
//!
//! Nothing graded that against contention. `contention_magnitude.rs` listed the family in
//! its *what is not graded here*: *"the block instructions repeat, and each family spends
//! its five repeat T-states on a different register's address. Nothing here reaches them,
//! and they are the shape most likely to expose a wrong internal-cycle address."* It is the
//! most likely shape precisely because **the address is what contention prices**, and these
//! four instructions are the only place in the instruction set where four different
//! registers answer the same question.
//!
//! # A total cannot separate one register from another, so this file does not use one
//!
//! Putting a register in the screen bank and reading off a bigger number proves that
//! *something* in the instruction contended. It cannot say **which** cycle did, because
//! every cycle that touches that register moved at once. The read, the write and the
//! repeat all shift together.
//!
//! What separates them is that the repeating and non-repeating members of a family emit
//! **byte-identical machine-cycle streams up to the repeat** — `LDI` and `LDIR` differ in
//! bit 4 of the opcode and in nothing the bus can see until the five cycles arrive. So
//!
//! ```text
//!     cost(LDIR) - cost(LDI)
//! ```
//!
//! taken from the same frame position is exactly the price of those five cycles, at
//! whatever address they ride, with every other cycle in the instruction cancelling. The
//! table below then does the discriminating part: for each family it puts the **claimed**
//! repeat register in contended memory with the others free, and separately puts each of
//! the **other** candidate registers there with the claimed one free. The claim is that the
//! difference moves in the first case and is exactly [`BLOCK_REPEAT_T_STATES`] in the
//! second — which is asserted as a claim, not merely implied by a pile of integers.
//!
//! The candidates are not invented. They are every address the instruction actually drives:
//! `HL`, `DE`, the port, the refresh address `IR`, and `PC`. Each is contended in its own
//! row while the rest stay free.
//!
//! ## The design was measured, not argued
//!
//! Making `LDIR` repeat on `HL` instead of `DE` — one word in
//! `crates/z80/src/instructions.rs` — moves the table like this:
//!
//! | row | in the screen bank | correct | mutated | |
//! |---|---|---|---|---|
//! | 1 | `DE` only | 47 / 48 | **29 / 30** | caught |
//! | 2 | `HL` only | 27 / 21 | **39 / 33** | caught |
//! | 3 | nothing | 21 / 21 | 21 / 21 | blind |
//! | 4 | **`HL` and `DE` both** | 55 / 48 | **55 / 48** | **blind** |
//!
//! Row 4 is the whole argument. It is the shape a total-based gate naturally takes — put the
//! operands in the contended bank, read off a bigger number — and it is **byte-for-byte
//! identical under the mutation**, because the ULA's stall is a function of *when*, so two
//! contended addresses are interchangeable to it. A file built only of row 4 would have been
//! decoration. Rows 1 and 2 are the ones that bite, and they exist because the register was
//! separated from the total on purpose.
//!
//! # How the expected values were obtained
//!
//! Every figure was derived **before** the emulator was measured, and never by adjusting an
//! observed one — `docs/STATUS.md` records what that costs. A recording bus was attached to
//! a real `Cpu`, the machine-cycle stream printed rather than assumed, the published delay
//! rule applied to that list by hand, and the result cross-checked against a second
//! implementation of the rule written with no sight of `crates/spectrum`. The recorder was
//! validated against a known answer first: `INC (HL)` must decompose as `pc:4, hl:3, hl:1,
//! hl:3` and cost 26 at phase 0 and 19 at phase 7, and it does.
//!
//! # What is not graded here
//!
//! - **Whether the published pattern is right.** It is the emulator community's figure for
//!   an issue 3 48K and this project has no oracle for it, exactly as for the memory and
//!   I/O patterns — see `contention_magnitude.rs` and `io_contention.rs`.
//! - **The phase.** Every position is relative to
//!   [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE], so every
//!   assertion here survives that constant being wrong. That is `contention_phase.rs`.
//! - **The block instructions' flags and results.** `P/V` from `BC != 0` rather than
//!   parity, and bit 5 of `F` from bit 1 of `A + transferred byte`, are graded by the FUSE
//!   vectors in `crates/z80`. This file grades only time.
//! - **What `INI`/`OUTI` read from or write to the port.** Only when.
//! - **Multi-iteration runs of the compare, input and output families.**
//!   [`a_repeating_transfer_is_re_priced_at_every_iteration`] drives `LDIR` for four
//!   iterations; the other three families are driven one iteration at a time. Their repeat
//!   mechanism is the same `repeat_block` call, which is the *by construction* argument
//!   this project distrusts on principle — so it is recorded here rather than relied on.
//! - **An interrupt accepted mid-loop.** The rewind is graded, so the loop is *shown* to be
//!   interruptible; nothing here actually interrupts one. `docs/STATUS.md` records that
//!   interrupt acceptance has no oracle in this project at all.

mod common;

use common::{
    CONTENDED_CODE, UNCONTENDED_CODE, advance_to, cost_of_running, elapsed, machine, set_pc,
    with_cpu_state, write_program,
};
use spectrum::timing::FIRST_CONTENDED_T_STATE;

/// T-states a repeating block instruction spends before re-running itself.
///
/// The published Z80 figure — 21 T-states while repeating against 16 on the final pass —
/// written here as the **expectation**. Deliberately not read from `crates/z80`: a constant
/// taken from the implementation agrees with any implementation.
const BLOCK_REPEAT_T_STATES: u64 = 5;

/// T-states a block instruction costs on the pass that does **not** repeat.
const NOMINAL_PLAIN: u64 = 16;

/// T-states it costs on a pass that does.
const NOMINAL_REPEATING: u64 = NOMINAL_PLAIN + BLOCK_REPEAT_T_STATES;

/// The two positions within the ULA's eight-T-state group that every case is measured at.
///
/// Phase 0 is where the pattern stalls most and phase 7 where it stalls nothing, so a model
/// that had the pattern rotated would have to be wrong in the same direction at both.
const PHASES: [u32; 2] = [0, 7];

/// `A`, chosen so `CPI`/`CPIR` never match the zero byte at `(HL)`.
///
/// The searching forms exit on **either** term — the counter running out or a match — so a
/// match would silently turn a repeating pass into a non-repeating one and every compare
/// row would measure the wrong instruction. Nothing else in this file reads `A`.
const ACCUMULATOR_NEVER_MATCHES_MEMORY: u16 = 0xFF00;

/// Uncontended addresses for the operands, clear of both code areas and of the prologue.
const HL_FREE: u16 = 0xC100;
const DE_FREE: u16 = 0xC200;

/// Their counterparts in the screen bank — the only bank a 48K contends.
const HL_HELD: u16 = 0x4100;
const DE_HELD: u16 = 0x4200;

/// `I`, putting the refresh address `IR` in the screen bank.
///
/// `R` is pinned to zero alongside it so `IR` is a fixed address rather than whatever the
/// positioning prologue left behind. Every value `R` could take is in the same bank, so the
/// pin is for legibility rather than correctness.
const INTERRUPT_VECTOR_IN_THE_SCREEN_BANK: u8 = 0x40;

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

/// One member of a family: the non-repeating encoding and the repeating one.
///
/// They are held as a pair because the pair is the instrument — see the module
/// documentation. A `Form` alone measures a total; the pair measures the repeat.
struct Form {
    name: &'static str,
    plain: &'static [u8],
    repeating: &'static [u8],
}

/// One configuration: what it puts in the screen bank, and what that must cost.
struct Case {
    /// What this row puts in the contended bank, in words.
    contended: &'static str,
    /// Whether that is the register the family's repeat is claimed to ride.
    ///
    /// This is the claim under test. `true` demands that the five repeat cycles cost
    /// *more* than their nominal five; `false` demands they cost **exactly** five, however
    /// much the rest of the instruction was stalled.
    is_the_repeat_address: bool,
    code_at: u16,
    hl: u16,
    de: u16,
    bc: u16,
    i: u8,
    /// The non-repeating form's total, at [`PHASES`].
    plain: [u64; 2],
    /// The repeating form's total, at the same two positions.
    repeating: [u64; 2],
}

/// A family, its four addresses, and the configurations that separate them.
struct Family {
    name: &'static str,
    /// The register `docs/Z80-REFERENCE.md` says the five repeat T-states ride.
    repeats_on: &'static str,
    /// The incrementing and decrementing members.
    ///
    /// Both are driven against the same expectations, and that is not an assumption: on a
    /// single pass the two emit **identical** cycle streams — the step direction only
    /// changes where the pointer lands afterwards — which was confirmed by recording both
    /// and comparing the streams address by address. `contention_magnitude.rs` lists
    /// *"`DEC`, and the `IY` half of every indexed form"* among the things it takes on the
    /// *by construction* argument; here the twin is measured instead.
    forms: [Form; 2],
    cases: &'static [Case],
}

/// `LDI`/`LDIR`/`LDD`/`LDDR` — the repeat rides `DE`, the address of the write.
///
/// The derivation, at phase 0 with `DE` in the screen bank and everything else free.
/// `d(k)` is the published pattern `[6, 5, 4, 3, 2, 1, 0, 0]` indexed by `k mod 8`, and the
/// cycle list is the recorded one: `pc:4, pc+1:4, hl:3, de:3, de:1 x2` and five more `de:1`
/// when it repeats.
///
/// ```text
///   M1 C000 at +0   free                   4 -> +4
///   M1 C001 at +4   free                   4 -> +8
///   MR C100 at +8   free                   3 -> +11
///   MW 4200 at +11  d(11)=3  -> +14        3 -> +17
///   IC 4200 at +17  d(17)=5  -> +22        1 -> +23
///   IC 4200 at +23  d(23)=0  -> +23        1 -> +24    LDI ends here          24
///   IC 4200 at +24  d(24)=6  -> +30        1 -> +31    the repeat's five begin
///   IC 4200 at +31  d(31)=0  -> +31        1 -> +32
///   IC 4200 at +32  d(32)=6  -> +38        1 -> +39
///   IC 4200 at +39  d(39)=0  -> +39        1 -> +40
///   IC 4200 at +40  d(40)=6  -> +46        1 -> +47    LDIR ends here         47
///                                                      the repeat cost        23
/// ```
///
/// A machine that repeated on `HL` instead would spend those five cycles at `0xC100`,
/// where nothing contends, and reach **29** — so this row alone separates the two rules by
/// eighteen T-states. The second row is the converse and is the more interesting half: with
/// `HL` contended and `DE` free the whole instruction is stalled, and the repeat must still
/// cost exactly five.
static TRANSFER_CASES: &[Case] = &[
    Case {
        contended: "DE, the address of the write",
        is_the_repeat_address: true,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_HELD,
        bc: 2,
        i: 0x00,
        plain: [24, 25],
        repeating: [47, 48],
    },
    Case {
        contended: "HL, the address of the read",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_HELD,
        de: DE_FREE,
        bc: 2,
        i: 0x00,
        plain: [22, 16],
        repeating: [27, 21],
    },
    Case {
        contended: "nothing",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 2,
        i: 0x00,
        plain: [16, 16],
        repeating: [21, 21],
    },
    Case {
        contended: "HL and DE both",
        is_the_repeat_address: true,
        code_at: UNCONTENDED_CODE,
        hl: HL_HELD,
        de: DE_HELD,
        bc: 2,
        i: 0x00,
        plain: [32, 25],
        repeating: [55, 48],
    },
];

/// `CPI`/`CPIR`/`CPD`/`CPDR` — the repeat rides `HL`, the address of the read.
///
/// The compares touch **one** memory address, so the discriminating rows are not another
/// operand but the two addresses a wrong implementation would plausibly reach for: the
/// refresh address `IR`, which is where the internal cycles that follow an opcode fetch
/// genuinely do sit, and `PC`.
///
/// The derivation at phase 0 with `HL` in the screen bank, from the recorded list
/// `pc:4, pc+1:4, hl:3, hl:1 x5` plus five more `hl:1`:
///
/// ```text
///   M1 C000 at +0   free              4 -> +4
///   M1 C001 at +4   free              4 -> +8
///   MR 4100 at +8   d(8)=6   -> +14   3 -> +17
///   IC 4100 at +17  d(17)=5  -> +22   1 -> +23
///   IC 4100 at +23  d(23)=0  -> +23   1 -> +24
///   IC 4100 at +24  d(24)=6  -> +30   1 -> +31
///   IC 4100 at +31  d(31)=0  -> +31   1 -> +32
///   IC 4100 at +32  d(32)=6  -> +38   1 -> +39   CPI ends here            39
///   IC 4100 at +39  d(39)=0  -> +39   1 -> +40
///   IC 4100 at +40  d(40)=6  -> +46   1 -> +47
///   IC 4100 at +47  d(47)=0  -> +47   1 -> +48
///   IC 4100 at +48  d(48)=6  -> +54   1 -> +55
///   IC 4100 at +55  d(55)=0  -> +55   1 -> +56   CPIR ends here           56
///                                                the repeat cost          17
/// ```
///
/// Seventeen rather than five plus five sixes, and the reason is the alternation
/// `an_internal_run_on_a_contended_refresh_address_is_charged_at_every_t_state` derives in
/// `contention_magnitude.rs`: a contended one-T-state internal cycle at group position
/// `k <= 6` stalls `6 - k` and therefore ends at position 7, where the stall is zero. A
/// contended internal run strictly alternates charged, free, charged, free — so five cycles
/// entering the run on a zero collect `0 + 6 + 0 + 6 + 0 = 12` of stall on top of their five
/// T-states.
static COMPARE_CASES: &[Case] = &[
    Case {
        contended: "HL, the address of the read",
        is_the_repeat_address: true,
        code_at: UNCONTENDED_CODE,
        hl: HL_HELD,
        de: DE_FREE,
        bc: 2,
        i: 0x00,
        plain: [39, 32],
        repeating: [56, 49],
    },
    Case {
        contended: "IR, the refresh address",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 2,
        i: INTERRUPT_VECTOR_IN_THE_SCREEN_BANK,
        plain: [16, 16],
        repeating: [21, 21],
    },
    Case {
        contended: "nothing",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 2,
        i: 0x00,
        plain: [16, 16],
        repeating: [21, 21],
    },
    Case {
        contended: "PC, the code itself",
        is_the_repeat_address: false,
        code_at: CONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 2,
        i: 0x00,
        plain: [26, 19],
        repeating: [31, 24],
    },
];

/// `INI`/`INIR`/`IND`/`INDR` — the repeat rides `HL`, the address of the **write**.
///
/// Three candidates here, and that is what makes the input family the richest row of the
/// four: the instruction drives `IR` for one internal cycle, then the port, then `HL` for
/// the write. Its recorded stream is `pc:4, pc+1:4, ir:1, port:4, hl:3`.
///
/// The derivation at phase 0 with `HL` in the screen bank, the port at `0x8002` — an
/// uncontended address, but an even one, so it is still the ULA's own port and still
/// charges the `N:1, C:3` case:
///
/// ```text
///   M1 C000 at +0   free                              4 -> +4
///   M1 C001 at +4   free                              4 -> +8
///   IC 0002 at +8   IR is 0x0002, free                1 -> +9
///   PR 8002 at +9   N:1,C:3 -> d(9+1)=d(10)=4 -> +13  4 -> +17
///   MW 4100 at +17  d(17)=5              -> +22       3 -> +25   INI ends here    25
///   IC 4100 at +25  d(25)=5              -> +30       1 -> +31
///   IC 4100 at +31  d(31)=0              -> +31       1 -> +32
///   IC 4100 at +32  d(32)=6              -> +38       1 -> +39
///   IC 4100 at +39  d(39)=0              -> +39       1 -> +40
///   IC 4100 at +40  d(40)=6              -> +46       1 -> +47   INIR ends here   47
///                                                                the repeat cost  22
/// ```
///
/// The port row is the discriminating one: `BC = 0x4002` puts the **port** in the contended
/// range while `HL` stays free, and the repeat must still cost exactly five. A machine that
/// left the port on the bus — which is what the *output* family genuinely does — would fail
/// that row and pass every other one in this table.
static INPUT_CASES: &[Case] = &[
    Case {
        contended: "HL, the address of the write",
        is_the_repeat_address: true,
        code_at: UNCONTENDED_CODE,
        hl: HL_HELD,
        de: DE_FREE,
        bc: 0x8002,
        i: 0x00,
        plain: [25, 26],
        repeating: [47, 48],
    },
    Case {
        contended: "the port",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 0x4002,
        i: 0x00,
        plain: [21, 22],
        repeating: [26, 27],
    },
    Case {
        contended: "IR, the refresh address",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 0x8002,
        i: INTERRUPT_VECTOR_IN_THE_SCREEN_BANK,
        plain: [28, 21],
        repeating: [33, 26],
    },
    Case {
        contended: "nothing",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 0x8002,
        i: 0x00,
        plain: [20, 21],
        repeating: [25, 26],
    },
];

/// `OUTI`/`OTIR`/`OUTD`/`OTDR` — the repeat rides the **port**, and `B` has already been
/// decremented.
///
/// This is the family whose rule is most easily got wrong twice over: the repeat rides
/// neither of the instruction's two memory addresses, and the port it rides is not the one
/// `BC` held when the instruction started. `block_output` decrements `B` *before* forming
/// the port, so `BC = 0x8002` transfers to port `0x7F02` — and `0x7F02` is contended while
/// `0x8002` is not.
///
/// The first two rows exploit exactly that, and they are the sharpest pair in this file:
///
/// | `BC` on entry | port used | contended? | expected repeat |
/// |---|---|---|---|
/// | `0x8002` | `0x7F02` | yes | more than five |
/// | `0x4002` | `0x3F02` | **no**, though `0x4002` would be | exactly five |
///
/// A machine that formed the port from `B` *before* the decrement gets both rows backwards
/// while agreeing with the published 21 T-states everywhere uncontended.
///
/// The derivation of the first, at phase 0, from the recorded stream
/// `pc:4, pc+1:4, ir:1, hl:3, port:4` plus five `port:1`:
///
/// ```text
///   M1 C000 at +0   free                                     4 -> +4
///   M1 C001 at +4   free                                     4 -> +8
///   IC 0002 at +8   IR is 0x0002, free                       1 -> +9
///   MR C100 at +9   free                                     3 -> +12
///   PW 7F02 at +12  contended address and the ULA's port, so C:1,C:3:
///                   a = d(12) = 2; then d(12+2+1) = d(15) = 0; stall 2 -> +14
///                                                            4 -> +18   OUTI ends   18
///   IC 7F02 at +18  d(18)=4  -> +22                          1 -> +23
///   IC 7F02 at +23  d(23)=0  -> +23                          1 -> +24
///   IC 7F02 at +24  d(24)=6  -> +30                          1 -> +31
///   IC 7F02 at +31  d(31)=0  -> +31                          1 -> +32
///   IC 7F02 at +32  d(32)=6  -> +38                          1 -> +39   OTIR ends   39
///                                                            the repeat cost        21
/// ```
static OUTPUT_CASES: &[Case] = &[
    Case {
        contended: "the port, formed after B's decrement (0x80 -> 0x7F)",
        is_the_repeat_address: true,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 0x8002,
        i: 0x00,
        plain: [18, 19],
        repeating: [39, 40],
    },
    Case {
        contended: "the port only before B's decrement (0x40 -> 0x3F)",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 0x4002,
        i: 0x00,
        plain: [17, 18],
        repeating: [22, 23],
    },
    Case {
        contended: "HL, the address of the read",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_HELD,
        de: DE_FREE,
        bc: 0x0202,
        i: 0x00,
        plain: [25, 26],
        repeating: [30, 31],
    },
    Case {
        contended: "IR, the refresh address",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 0x0202,
        i: INTERRUPT_VECTOR_IN_THE_SCREEN_BANK,
        plain: [25, 18],
        repeating: [30, 23],
    },
    Case {
        contended: "nothing",
        is_the_repeat_address: false,
        code_at: UNCONTENDED_CODE,
        hl: HL_FREE,
        de: DE_FREE,
        bc: 0x0202,
        i: 0x00,
        plain: [17, 18],
        repeating: [22, 23],
    },
];

/// One rule, four addresses — `docs/Z80-REFERENCE.md`'s table, as a gate.
static FAMILIES: [Family; 4] = [
    Family {
        name: "transfer",
        repeats_on: "DE",
        forms: [
            Form {
                name: "LDI/LDIR",
                plain: &[0xED, 0xA0],
                repeating: &[0xED, 0xB0],
            },
            Form {
                name: "LDD/LDDR",
                plain: &[0xED, 0xA8],
                repeating: &[0xED, 0xB8],
            },
        ],
        cases: TRANSFER_CASES,
    },
    Family {
        name: "compare",
        repeats_on: "HL",
        forms: [
            Form {
                name: "CPI/CPIR",
                plain: &[0xED, 0xA1],
                repeating: &[0xED, 0xB1],
            },
            Form {
                name: "CPD/CPDR",
                plain: &[0xED, 0xA9],
                repeating: &[0xED, 0xB9],
            },
        ],
        cases: COMPARE_CASES,
    },
    Family {
        name: "input",
        repeats_on: "HL",
        forms: [
            Form {
                name: "INI/INIR",
                plain: &[0xED, 0xA2],
                repeating: &[0xED, 0xB2],
            },
            Form {
                name: "IND/INDR",
                plain: &[0xED, 0xAA],
                repeating: &[0xED, 0xBA],
            },
        ],
        cases: INPUT_CASES,
    },
    Family {
        name: "output",
        repeats_on: "the port",
        forms: [
            Form {
                name: "OUTI/OTIR",
                plain: &[0xED, 0xA3],
                repeating: &[0xED, 0xB3],
            },
            Form {
                name: "OUTD/OTDR",
                plain: &[0xED, 0xAB],
                repeating: &[0xED, 0xBB],
            },
        ],
        cases: OUTPUT_CASES,
    },
];

// ---------------------------------------------------------------------------
// Driving one measurement
// ---------------------------------------------------------------------------

/// Run `program` once from `case`'s code area, with the clock at `at`, and report the cost.
fn cost_of(case: &Case, program: &[u8], at: u32) -> u64 {
    let mut machine = machine();
    advance_to(&mut machine, at);
    write_program(&mut machine, case.code_at, program);
    with_cpu_state(&mut machine, |state| {
        state.hl = case.hl;
        state.de = case.de;
        state.bc = case.bc;
        state.af = ACCUMULATOR_NEVER_MATCHES_MEMORY;
        state.i = case.i;
        state.r = 0;
    });
    cost_of_running(&mut machine, case.code_at, 1)
}

// ---------------------------------------------------------------------------
// The gates
// ---------------------------------------------------------------------------

#[test]
fn every_block_form_costs_its_published_length_outside_the_fetch_window() {
    // The control for the whole file, and the only check anywhere that the machine's clock
    // agrees with the published 16 and 21 for these sixteen encodings.
    //
    // It is taken **outside** the ULA's fetch window rather than out of an uncontended bank,
    // because two of the four families cannot be made free by choosing a bank: `INI` and
    // `OUTI` address the ULA's own port whenever `C` is even, and the ULA contends its own
    // port at an uncontended address too. Position is the only axis that frees all four.
    //
    // Every register placement in the table is driven here, which is the point: contention
    // is a property of *when*, so at this position none of them may cost anything extra. A
    // model that stalled on address alone would pass the whole file and fail this test.
    const BEFORE_DISPLAY: u32 = 200;
    let at = FIRST_CONTENDED_T_STATE - BEFORE_DISPLAY;

    for family in &FAMILIES {
        for form in &family.forms {
            for case in family.cases {
                assert_eq!(
                    cost_of(case, form.plain, at),
                    NOMINAL_PLAIN,
                    "{} ({} family, {} contended) must cost its published {NOMINAL_PLAIN} \
                     T-states while the ULA is in the top border",
                    form.name,
                    family.name,
                    case.contended
                );
                assert_eq!(
                    cost_of(case, form.repeating, at),
                    NOMINAL_REPEATING,
                    "{} ({} family, {} contended) must cost its published \
                     {NOMINAL_REPEATING} T-states while repeating in the top border",
                    form.name,
                    family.name,
                    case.contended
                );
            }
        }
    }
}

#[test]
fn each_family_spends_its_repeat_t_states_on_its_own_register() {
    // The discriminating gate. For every family, every member, every configuration and both
    // phases: the two absolute totals, and then the claim itself.
    for family in &FAMILIES {
        for form in &family.forms {
            for case in family.cases {
                for (index, phase) in PHASES.into_iter().enumerate() {
                    let at = FIRST_CONTENDED_T_STATE + phase;
                    let plain = cost_of(case, form.plain, at);
                    let repeating = cost_of(case, form.repeating, at);

                    assert_eq!(
                        plain, case.plain[index],
                        "{} with {} contended, at phase +{phase}: the non-repeating pass \
                         must cost {} T-states",
                        form.name, case.contended, case.plain[index]
                    );
                    assert_eq!(
                        repeating, case.repeating[index],
                        "{} with {} contended, at phase +{phase}: the repeating pass must \
                         cost {} T-states",
                        form.name, case.contended, case.repeating[index]
                    );

                    // The two forms are identical up to the repeat, so this difference is
                    // the price of the five repeat cycles and of nothing else.
                    let repeat = repeating - plain;
                    if case.is_the_repeat_address {
                        assert!(
                            repeat > BLOCK_REPEAT_T_STATES,
                            "{} repeats on {}, and this row puts {} in the screen bank — so \
                             its five repeat T-states must be stalled. They cost \
                             {repeat}, which is the free price",
                            form.name,
                            family.repeats_on,
                            case.contended
                        );
                    } else {
                        assert_eq!(
                            repeat, BLOCK_REPEAT_T_STATES,
                            "{} repeats on {}, which this row leaves uncontended — so its \
                             five repeat T-states must cost exactly {BLOCK_REPEAT_T_STATES} \
                             however much the rest of the instruction was stalled. Putting \
                             {} in the screen bank moved them to {repeat}, which means the \
                             repeat is riding the wrong address",
                            form.name, family.repeats_on, case.contended
                        );
                    }
                }
            }
        }
    }
}

/// T-states the four iterations cost, in order.
///
/// Derived in [`a_repeating_transfer_is_re_priced_at_every_iteration`]. Four different
/// numbers for four executions of the same two bytes is the property being gated.
const ITERATION_COSTS: [u64; 4] = [47, 48, 36, 16];

/// `BC` on entry, and therefore how many iterations the loop runs.
const ITERATIONS: u16 = 4;

#[test]
fn a_repeating_transfer_is_re_priced_at_every_iteration() {
    // **Why four.** The repeat is `PC -= 2` with one `step()` per iteration, so a gate has
    // to choose a count. Four is the smallest run that opens on four *different* columns of
    // the ULA's pattern — 0, 47, 95 and 131 — the last of which is past the end of the
    // 128-T-state fetch window, and it is also the count that makes the final pass the
    // **exit**, so the loop's termination is graded by the same run. A model that priced the
    // repeat once and multiplied would agree with the first iteration and fail the rest.
    //
    // `LDIR` out of uncontended code with `DE` in the screen bank, from phase 0. `DE`
    // advances by one per iteration, so each pass writes to a different address in the same
    // bank; the cycle shape is identical and only the entry column moves.
    //
    //   iteration 0, opening at column 0 — the derivation in `TRANSFER_CASES`      47
    //
    //   iteration 1, opening at column 47:
    //     M1 C000 at +47   free              4 -> +51
    //     M1 C001 at +51   free              4 -> +55
    //     MR C101 at +55   free              3 -> +58
    //     MW 4201 at +58   d(58)=4  -> +62   3 -> +65
    //     IC 4201 at +65   d(65)=5  -> +70   1 -> +71
    //     IC 4201 at +71   d(71)=0  -> +71   1 -> +72
    //     IC 4201 at +72   d(72)=6  -> +78   1 -> +79
    //     IC 4201 at +79   d(79)=0  -> +79   1 -> +80
    //     IC 4201 at +80   d(80)=6  -> +86   1 -> +87
    //     IC 4201 at +87   d(87)=0  -> +87   1 -> +88
    //     IC 4201 at +88   d(88)=6  -> +94   1 -> +95                              48
    //
    //   iteration 2, opening at column 95 — this is the one that straddles the edge of the
    //   fetch window, and it is why the run is worth driving at all:
    //     M1 C000 at +95   free              4 -> +99
    //     M1 C001 at +99   free              4 -> +103
    //     MR C102 at +103  free              3 -> +106
    //     MW 4202 at +106  d(106)=4 -> +110  3 -> +113
    //     IC 4202 at +113  d(113)=5 -> +118  1 -> +119
    //     IC 4202 at +119  d(119)=0 -> +119  1 -> +120
    //     IC 4202 at +120  d(120)=6 -> +126  1 -> +127
    //     IC 4202 at +127  d(127)=0 -> +127  1 -> +128
    //     IC 4202 at +128  column 128 is past the fetch window, free  1 -> +129
    //     IC 4202 at +129  free              1 -> +130
    //     IC 4202 at +130  free              1 -> +131                             36
    //
    //   iteration 3, opening at column 131 — entirely in the border, and `BC` reaches zero
    //   so it does not repeat: the plain sixteen                                   16
    //
    //                                                                       total 147
    let mut machine = machine();
    advance_to(&mut machine, FIRST_CONTENDED_T_STATE);
    write_program(&mut machine, UNCONTENDED_CODE, &[0xED, 0xB0]);
    with_cpu_state(&mut machine, |state| {
        state.hl = HL_FREE;
        state.de = DE_HELD;
        state.bc = ITERATIONS;
        state.af = ACCUMULATOR_NEVER_MATCHES_MEMORY;
        state.r = 0;
    });
    set_pc(&mut machine, UNCONTENDED_CODE);

    let mut costs = Vec::with_capacity(ITERATIONS as usize);
    let mut program_counters = Vec::with_capacity(ITERATIONS as usize);
    for _ in 0..ITERATIONS {
        let before = elapsed(&machine);
        machine.step();
        costs.push(elapsed(&machine) - before);
        program_counters.push(machine.cpu_state().pc);
    }

    assert_eq!(
        costs,
        ITERATION_COSTS.to_vec(),
        "the same two bytes, executed four times from one starting position, must cost four \
         different amounts: each iteration opens at a different column of the ULA's pattern \
         and the last of the four is in the border"
    );
    assert_eq!(
        costs.iter().sum::<u64>(),
        ITERATION_COSTS.iter().sum::<u64>(),
    );

    // The rewind, which is what makes a 64 KB `LDIR` interruptible: every repeating pass
    // leaves `PC` back on the `ED` byte, and only the exit pass steps past the instruction.
    let (exit, repeating) = program_counters
        .split_last()
        .expect("four iterations were driven");
    assert!(
        repeating.iter().all(|pc| *pc == UNCONTENDED_CODE),
        "a repeating pass must rewind PC onto its own opcode, leaving the instruction \
         interruptible between iterations: {program_counters:04X?}"
    );
    assert_eq!(
        *exit,
        UNCONTENDED_CODE + 2,
        "the pass that exhausts BC must step past the instruction"
    );

    let state = machine.cpu_state();
    assert_eq!(state.bc, 0, "the counter must be exhausted");
    assert_eq!(state.hl, HL_FREE + ITERATIONS, "HL walks up one per pass");
    assert_eq!(state.de, DE_HELD + ITERATIONS, "DE walks up one per pass");
    assert_eq!(
        state.r,
        u8::try_from(2 * ITERATIONS).expect("eight refreshes"),
        "R advances by two per iteration — the instruction re-fetches both of its own \
         opcode bytes each pass, which is what the rewind means"
    );
}
