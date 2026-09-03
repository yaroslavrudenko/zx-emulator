//! The ULA: the bus the Z80 sees, and the clock it runs against.
//!
//! One chip does four jobs that look unrelated and are not: it draws the screen, it stalls
//! the CPU, it reads the keyboard, and it raises the 50 Hz interrupt. Drawing and stalling
//! are the same mechanism seen from two sides — the address the ULA is fetching at T-state
//! `t` is exactly what makes a CPU wanting that bank wait.
//!
//! # Where a T-state is charged
//!
//! `crates/z80` calls the transfer method **first** and then ticks that cycle's T-states,
//! so when `fetch`/`read`/`write`/`in_port`/`out_port` runs, this clock stands at the
//! *start* of the machine cycle making the access. That is where a stall belongs: each
//! transfer prices its own contention **once**, and the ticks that follow are that cycle's
//! own T-states, already paid for. A tick arriving with no cycle outstanding is a standalone
//! internal cycle, and contends on its own account at its own frame position.
//!
//! Telling those two apart is a matter of counting, because every cycle's length is known
//! the moment it opens: an M1 fetch and a port access are four T-states, a memory read or
//! write three. That the machine can recognise an M1 fetch **at all** is what [`Bus::fetch`]
//! bought. Routed through one `read`, a four-T-state opcode fetch and a three-T-state read
//! followed by one internal cycle emit byte-identical streams — same address, same order,
//! same count — and this machine used to reconstruct the boundary by deferring a T-state and
//! seeing whether a fifth arrived. That guess was wrong for the read-modify-write family,
//! costing one contention point per execution; `docs/MACHINE.md` records the whole episode.
//!
//! # What is modelled and what is not
//!
//! | | |
//! |---|---|
//! | Memory contention | yes, per access, against the current slot map |
//! | Internal-cycle contention | yes, per T-state |
//! | I/O contention | yes, the four-case ULA port pattern |
//! | Keyboard | yes |
//! | Kempston joystick | **yes** — port `0x1F`, five switches, active high; see [`crate::joystick`] |
//! | The Kempston **decode** | **primary** — `A5 = A6 = A7 = 0` from the Issue 4 (1989) schematic, so the window is the whole low byte `0x00..=0x1F` and **no high-byte line is consulted**; see [`crate::joystick::KEMPSTON_PORT_MASK`] |
//! | Border colour | latched, **and recorded per rendered row** — a tape load shows its bands; see `screen::BorderTrace` |
//! | Floating bus | **no** — an undecoded port reads [`FLOATING_BUS_BYTE`] |
//! | Speaker | **yes** — bit 4 of a `0xFE` write drives the beeper; M7 closed this |
//! | `MIC` | **no** — bit 3 is still discarded, and what that costs is in `MIC_BIT` |
//! | The `0xFE` port's **four** output levels | **no** — the speaker is two-level. `MIC` shifting it is a magnitude with no source |
//! | AY-3-8912 | **yes on a 128, absent on a 48K** — `0xFFFD` select and read, `0xBFFD` write; see [`crate::ay`] |
//! | The AY's **address decode** | **derived, and the weakest claim in M7** — no source states it; see `AY_PORT_MASK` |
//! | `EAR` input | **yes** — bit 6 of a `0xFE` read follows [`crate::tape::Tape::level`] |
//! | `EAR` reaching the **speaker** | **yes** — the same level arrives as [`crate::audio::Sample::tape`], because the socket feeds the amplifier as well as the ULA; M8 closed this |
//! | Issue 2 / issue 3 `EAR` readback | **no** — writing bit 3 or 4 does not change what bit 6 reads |
//! | Paging port `0x7FFD` writes | **yes**, on the partial decode — A15 and A1 both reset |
//! | Paging port `0x7FFD` **reads** | **no** — and on real hardware they are destructive; see below |
//! | Interrupt-acknowledge contention | **yes, charged once** — M7 fixed it; see below |
//!
//! ## Reading `0x7FFD` is destructive on the hardware, and this machine does not model it
//!
//! The World of Spectrum reference says *"Reading from 0x7ffd produces no special results:
//! floating bus values will be returned as would be returned from any other port not attached to
//! any hardware."* **Three sources say otherwise, one of them primary.** The Sinclair *Servicing
//! Manual* §4.12.11 has the latch firing on an *"I/O read **or** write cycle"*; MAME's address
//! map comments *"reading from this port does write to it by value from data bus"*; and the
//! Sinclair Wiki is blunt about the consequence:
//!
//! > *"Reads from port 0x7ffd cause a crash, as the 128's HAL10H8 chip does not distinguish
//! > between reads and writes to this port, resulting in a floating data bus being used to set
//! > the paging registers."*
//!
//! So an `IN` from any address matching the write decode latches whatever the data bus is
//! floating at into the paging register. **[`Ula::in_port`] does not do this**, and the reason is
//! not oversight: the value latched *is the floating bus*, which this machine does not model —
//! [`FLOATING_BUS_BYTE`] is a constant. Implementing it would page to a byte we invented, on
//! every read, which is a louder wrong answer than not implementing it at all. It is named here
//! so that it is the first thing suspected if 128 software ever misbehaves after an `IN`.
//!
//! ## The `EAR` bit, and where the tape gets its time
//!
//! M6 turned the `EAR` row from **no** into **yes**, and the sentence that used to stand here
//! — *"the tape port M6 brings"* — is what a milestone boundary does to a doc comment.
//! `docs/STATUS.md` records that class: *"every one of those comments was true when written…
//! they are falsified at milestone boundaries, because that is when the claims they encode
//! stop holding."*
//!
//! With no tape in the drive the bit still reads **low**, which is the state an issue 3
//! machine idles in and is what `UNDRIVEN_INPUT_BITS` has always produced.
//! `crates/spectrum/tests/keyboard_matrix.rs` pins the literal `0xBF` a real `IN A,(0xFE)`
//! returns across the whole membrane, so this change is graded from outside this file.
//!
//! What is **not** modelled is the issue 2 / issue 3 readback: writing bit 3 or 4 of a `0xFE`
//! write and reading bit 6 back. It is a real cause of *"loads on one emulator and not
//! another"*, and it is named here so it is the first thing suspected if a specific loader
//! fails.
//!
//! Contention means the clock does not advance one T-state at a time, so the tape cannot be
//! driven from [`Ula::tick`] alone: a stall is elapsed time the tape has to see. `Ula::advance`
//! is the one place that moves both, and every `Clock::advance` call site routes through it,
//! so the two cannot drift.
//!
//! The sampling point is an approximation and its size is known. [`Ula::in_port`] runs after
//! the cycle's contention stall is charged and **before** its four nominal T-states arrive as
//! ticks, so the level is read up to four T-states early relative to where real hardware
//! latches it. The ROM's loader distinguishes an 855-T-state half-bit from a 1710-T-state one,
//! so four T-states is far inside its tolerance; a turbo loader with tighter margins is where
//! it could matter, and a turbo loader failing is what would decide it.
//!
//! ## The acknowledge cycle — M7 made it one machine cycle, and the argument that deferred it was wrong about *why*
//!
//! Every other cycle reaches this bus as a transfer callback followed by its own T-states,
//! so its stall is charged **once**. An interrupt acknowledge reads no memory — the Z80
//! asserts `/IORQ` in place of `/MREQ` and the device answers on the data bus — so until M7
//! it had no callback at all, and `crates/z80` delivered it as seven bare [`Bus::tick`] calls
//! at the refresh address. Recorded off a real `Cpu`:
//!
//! ```text
//!   IM 1   IC@IR:1 x7   MW@sp-1:3   MW@sp-2:3
//!   IM 2   IC@IR:1 x7   MW@sp-1:3   MW@sp-2:3   MR@vector:3   MR@vector+1:3
//! ```
//!
//! Each of those seven arrived with no cycle outstanding, so [`Ula::tick`] treated it as a
//! standalone internal cycle and contended it on its own account. The hardware performs
//! **one** machine cycle there — M1 stretched by two wait states — so a contended `IR` was
//! charged seven stalls where it owes one.
//!
//! [`Bus::acknowledge`] closes it, on the [`Bus::fetch`] precedent: a defaulted method, so
//! adding it broke nothing, and [`Ula`] implements it as the contention charge plus arming
//! `covered_t_states`, which is what every other cycle already does.
//!
//! ### What the old paragraph got right, what it got wrong, and what is still true
//!
//! It said the defect was unobservable and that *"a fix would be an unverifiable guess"*. The
//! first clause was **true of software running on the machine and false of a test driving the
//! bus**, which has existed since M5 — `Cpu::new`, `Cpu::interrupt` and `Ula::new` are all
//! public, and a test that puts the clock inside the contended window and points `IR` at a
//! contended bank separates the two models by a measurable number of T-states. The second
//! clause was wrong in kind: no *magnitude* was being invented. One cycle or seven is a
//! **structural** question, and `docs/Z80-REFERENCE.md` already answered it in this
//! repository's own words — one machine cycle, therefore one contention point.
//!
//! **The 48K unobservability argument is real, and it is stronger than "not currently
//! covered".** `/INT` is asserted for the frame's first [`crate::timing::INTERRUPT_T_STATES`] and
//! acceptance cannot be deferred past the window's end, so an accepted interrupt *cannot*
//! reach [`crate::timing::FIRST_CONTENDED_T_STATE`] — 14335 — at any address, for any program.
//!
//! **It survives the 128 unchanged, and that was re-derived on the 128's own numbers rather
//! than inherited.** Its window opens at frame T-state 0 exactly as the 48K's does and its
//! contention begins at **14362**, so the gap is ≈14300 T-states on both machines. The
//! conclusion never depended on the number: the offset would have to be **under about forty**
//! for an acknowledge to reach it, and no candidate value for any machine in this family is
//! near that. *(This read 14361 and called it "the disputed constant". It is no longer disputed
//! and it is no longer 14361: the 128 edition of `tests/timing_oracle.rs` was run on 2026-09-02
//! and 14362 is the unique zero over `14355..=14370`. The margin the paragraph rests on is
//! unchanged by one T-state, which is the point it was making.)*
//!
//! **`NMI` is the exception, and it is why this is now gated rather than merely pinned.** An
//! `NMI` has no window — nothing stops one being raised at any frame position — so a
//! `Cpu::nmi` with `IR` in a contended bank inside the fetch window is reachable and is
//! measurable on the clock. A Spectrum has no `NMI` source, so no *guest* can get there; a
//! test can, and does.
//!
//! So the register keeps both halves: **the fix closes a model defect and closes no
//! verification gap.** Nothing grades this against hardware, and — the part worth carrying —
//! nothing can, because no software on either machine reaches the case.
//!
//! The floating bus is the interesting omission. Everything needed to model it is already
//! here — the clock knows where in the fetch window it is — but the byte-to-phase mapping
//! is exactly the kind of claim `docs/MACHINE.md` says must not be guessed: there is no
//! oracle for it, and software that reads it (`Arkanoid`, `Aquaplane`) is M6's problem.
//! Returning a constant is wrong in a way that is *visible*; a plausible guess would be
//! wrong in a way that is not.

use z80::{Bus, MEMORY_ACCESS_T_STATES, OPCODE_FETCH_T_STATES, PORT_ACCESS_T_STATES};

use crate::audio::{Audio, Sample};
use crate::ay::Ay;
use crate::joystick::{Joystick, KEMPSTON_PORT_MASK, KEMPSTON_PORT_SELECT};
use crate::keyboard::Keyboard;
use crate::memory::Memory;
use crate::screen::{BorderTrace, Colour};
use crate::tape::Tape;
use crate::timing::Clock;

/// What the data bus reads as when nothing drives it.
///
/// A 48K floats high, which is why mode 0 and mode 1 interrupts behave alike on this
/// machine: `0xFF` decodes as `RST 38h`, the same address mode 1 vectors to.
pub const FLOATING_BUS_BYTE: u8 = 0xFF;

/// The ULA answers every port whose bit 0 is clear, and decodes no other address line.
///
/// This is why the port is written `0xFE` but responds at `0x7FFE`, `0xBFFE` and every
/// other even address — the high half is free for the keyboard to use as a row selector.
const ULA_PORT_SELECT: u16 = 0x0001;

/// The paging port answers every address with **A15 and A1 both reset**, and decodes nothing
/// else.
///
/// From the World of Spectrum *128K Technical Information* reference, verbatim: *"the hardware
/// will respond to any port address with bits 1 and 15 reset."* So `0x7FFD` is one member of a
/// large family and not the decode — writing the mask rather than an equality is what separates
/// a correct decode from a lucky one, exactly as [`ULA_PORT_SELECT`] does for `0xFE`.
///
/// **This is the best-sourced figure in M7, and it has a primary witness.** The Sinclair
/// *Servicing Manual for Spectrum 128* §4.12.11 states it from the circuit rather than from
/// behaviour: *"BANK is decoded (set high) from IORQ and W/WR active low (I/O read or write
/// cycle) and **ZA1 and ZA15 low** (address 7FFDH)."* Fuse's decode table carries the identical
/// mask (`{ 0x8002, 0x0000, … }`), and MAME's is the same rule spelled as an address map. Three
/// lineages, one of them the manufacturer's own schematic description.
const PAGING_PORT_SELECT: u16 = 0x8002;

// The two decodes select on **disjoint** address lines — A0 against A15 and A1 — so neither
// constrains the other and the two families genuinely overlap: any address with A0, A1 and A15
// all reset is claimed by both, and on the hardware both respond. That is why `out_port` is a
// run of independent `if`s and never a `match` on the port. It said "**two** independent `if`s"
// until M7 gave the AY two arms of its own and made it four; the count is left out rather than
// re-pinned, because it is the *shape* this comment is about and the count has now been wrong
// once already.
const _: () = assert!(ULA_PORT_SELECT & PAGING_PORT_SELECT == 0);
const _: () = assert!(0x7FFD & PAGING_PORT_SELECT == 0, "the paging port decodes");
const _: () = assert!(0x00FE & ULA_PORT_SELECT == 0, "the ULA port decodes");
const _: () = assert!(
    0x00FC & (ULA_PORT_SELECT | PAGING_PORT_SELECT) == 0,
    "and they overlap"
);

// **The two *canonical* addresses do not overlap, and this assertion pair is what established
// it.** The comment above originally claimed `0x00FE` was claimed by both devices; the compiler
// refused it. `0x00FE` has **A1 set**, so it reaches the ULA and not the paging port, and
// `0x7FFD` has **A0 set**, so it reaches the paging port and not the ULA. The families overlap
// and their published members do not — a sharper statement than "the two collide" and a more
// useful one, because it means the `if`/`if` shape is invisible to ordinary software and
// matters only for a program addressing either port some other way. That is exactly the case a
// mask gets right and an equality gets wrong, so the shape is kept; nothing here grades it,
// because nothing in reach exercises it.
const _: () = assert!(
    0x00FE & PAGING_PORT_SELECT != 0,
    "0xFE is not a paging write"
);
const _: () = assert!(
    0x7FFD & ULA_PORT_SELECT != 0,
    "0x7FFD is not a border write"
);

/// The AY answers three address lines: **A15 and A1**, plus **A14** to choose which of its two
/// ports.
///
/// **This is the least-supported claim in M7 and it is written here rather than buried.**
/// `docs/M7.md` says so in terms: *"no source was found stating which lines decode `0xFFFD`
/// and `0xBFFD`"*, and this mask is inferred from the two well-attested addresses plus the
/// style of the `0x7FFD` decode beside it — which **does** have a primary witness in the
/// Sinclair *Servicing Manual*, and this does not. Treat it as derived, and treat a game that
/// misbehaves reaching the AY some unusual way as evidence about this constant first.
///
/// What the inference rests on, stated so a reader can weigh it rather than take it:
/// `0xFFFD` and `0xBFFD` differ in **A14 alone**, both have A15 set and A1 clear, and the
/// 128's other partially-decoded ports each answer a two- or three-line pattern of exactly
/// this shape. That is consistent with the addresses and it is not a citation.
const AY_PORT_MASK: u16 = 0xC002;

/// The address the AY's register-select and register-read port answers: A15 and A14 set.
const AY_SELECT_PORT: u16 = 0xC000;

/// The address the AY's register-write port answers: A15 set, A14 clear.
const AY_WRITE_PORT: u16 = 0x8000;

const _: () = assert!(
    0xFFFD & AY_PORT_MASK == AY_SELECT_PORT,
    "the select port decodes"
);
const _: () = assert!(
    0xBFFD & AY_PORT_MASK == AY_WRITE_PORT,
    "the write port decodes"
);
const _: () = assert!(AY_SELECT_PORT != AY_WRITE_PORT, "A14 tells them apart");

// **The AY and the paging port cannot collide, and this is the assertion that says so once so
// nobody re-derives it.** The paging port needs A15 *reset* and both AY ports need it *set*,
// so the two families are disjoint on a line neither leaves free. `M7.md` Decision 2 calls
// this a genuine structural guarantee and it is worth having the compiler hold it.
const _: () = assert!(AY_SELECT_PORT & PAGING_PORT_SELECT != 0);
const _: () = assert!(AY_WRITE_PORT & PAGING_PORT_SELECT != 0);

// The AY and the ULA **do** collide, on any address with A0, A1 reset and A15 set — `0xFFFC`
// is one. Neither published address is such an address, because both have A0 set, so this is
// unreachable by software that addresses either port the ordinary way. It is the same shape
// as the ULA/paging overlap two constants above, and it is handled the same way: `out_port` is
// independent `if`s so both devices act, and `in_port` fixes an order and says why at the site.
const _: () = assert!(0xFFFC & ULA_PORT_SELECT == 0 && 0xFFFC & AY_PORT_MASK == AY_SELECT_PORT);
const _: () = assert!(0xFFFD & ULA_PORT_SELECT != 0, "0xFFFD is not a ULA port");
const _: () = assert!(0xBFFD & ULA_PORT_SELECT != 0, "0xBFFD is not a ULA port");

/// Bits of a `0xFE` write that set the border colour.
const BORDER_MASK: u8 = 0x07;

/// Bit 4 of a `0xFE` write: the speaker.
///
/// **The 48K's entire sound output**, and every 48K game's music. It is the same byte the
/// border comes out of bits 0–2 of, which is why the beeper is a `crates/spectrum` concern —
/// being audible does not make a ULA output a frontend's business. `docs/M8.md` Decision 9
/// ruled it here and this constant is what that ruling landed as.
const SPEAKER_BIT: u8 = 0x10;

/// Bit 3 of a `0xFE` write: the `MIC` output.
///
/// **Still not modelled, and it is a narrower gap than it was.** `MIC` is tape *save*, which
/// no milestone has claimed. What it also does — on real hardware it shifts the speaker's
/// output level slightly, giving the port four output levels rather than two — is a
/// **magnitude** with no source this project can adjudicate, and inventing a ratio between two
/// bits would be exactly the kind of plausible number `docs/M7.md` Decision 6 rules out. So
/// the speaker follows bit 4 alone and this constant exists to name what is left out rather
/// than to leave it unnamed.
///
/// `crates/spectrum/tests/tape_rom_timings.rs` watches this bit from outside the machine, and
/// its account is unaffected: the ROM's `SA-BYTES` toggles `MIC` and the border together and
/// leaves bit 4 alone, so tape saving still emits no beeper edges.
const MIC_BIT: u8 = 0x08;

const _: () = assert!(
    SPEAKER_BIT & BORDER_MASK == 0,
    "the speaker is not a border bit"
);
const _: () = assert!(SPEAKER_BIT & MIC_BIT == 0, "and it is not MIC either");

/// Bits 5 and 7 of a `0xFE` read, which nothing drives.
///
/// They float high. Bit 6 used to be described here as part of the same set — *"the `EAR`
/// input, low with nothing connected to the tape socket"* — because until M6 that was the
/// only state it could be in. It is now driven by [`Ula::ear_bit`], and the constant is
/// unchanged: with no tape the level is low, so `IN A,(0xFE)` still returns the same literal
/// it did at M5.
const UNDRIVEN_INPUT_BITS: u8 = 0xA0;

/// Bit 6 of a `0xFE` read: the `EAR` input, driven by the tape.
const EAR_BIT: u8 = 0x40;

// The tape must drive a bit nothing else claims, and the two constants are written
// independently — bit 6 from the port's documented layout, bits 5 and 7 from the keyboard's.
const _: () = assert!(EAR_BIT & UNDRIVEN_INPUT_BITS == 0);

/// The bus the CPU is wired to: memory, ports, the keyboard, and the frame clock.
#[derive(Debug)]
pub struct Ula {
    memory: Memory,
    keyboard: Keyboard,
    /// The Kempston joystick, which is an interface on the bus rather than a key.
    ///
    /// Always present, on both machines. A real Spectrum has one only if somebody plugged it
    /// in, and modelling that would mean an `Option` whose `None` differs from its `Some`
    /// **only** in what an undecoded port reads — which is a distinction no software can act
    /// on, since a game that reads `0x1F` and finds nothing held behaves identically either
    /// way. See `crate::joystick`.
    joystick: Joystick,
    clock: Clock,
    /// T-states of the machine cycle in progress whose contention is already charged.
    ///
    /// Set by whichever transfer opened the cycle and spent one per tick; a tick arriving
    /// with it at zero is a standalone internal cycle. It tracks the **CPU's** stream, so a
    /// caller driving this bus directly — a test reading a port, say — arms it without ever
    /// spending it. That costs nothing to such a caller, who is reading a value rather than
    /// measuring time, and the next transfer overwrites it.
    covered_t_states: u8,
    /// Where the border was as the beam went down the frame, and what it is now.
    ///
    /// **One field, not two.** The colour showing now and the record of where it changed are
    /// one datum seen at two resolutions, and [`crate::screen::BorderTrace`] owns both — so
    /// there is no pair to disagree. [`Ula::border`] reads through it.
    border: BorderTrace,
    tape: Tape,
    /// Times a guest has read the `EAR` line since power-on. See [`Ula::ear_reads`].
    ///
    /// It counts a **guest's** reads and not the machine's own work, which is why it sits
    /// beside the drive rather than inside it: a cassette turning with nobody listening moves
    /// this not at all, and a machine listening with an empty drive moves it at full rate.
    /// Those two are the cases a drive's own `is_playing` cannot tell apart, and separating
    /// them is the whole of what this field is for.
    ear_reads: u64,
    /// The machine's sound: the beeper, the tape's `EAR` line, the AY on a machine that has
    /// one, and the samples they have produced.
    ///
    /// **This read *"Nothing in `Ula::tick` touches this"*, and `Ula::advance` three hundred
    /// lines below has touched it since M8 routed the tape to the speaker** — which made the two
    /// sentences contradict inside one file, the field's doc asserting what its only per-T-state
    /// caller disproves. What is true now: `tick` reaches this **only through the tape's edge**,
    /// about 32 times a frame during a load and never at all with an empty drive, at a measured
    /// **+0.9 µs a frame** against a 20,000 µs budget (`cargo bench -p spectrum --bench frame`,
    /// 2026-09-03, `tape_playing_48k` against `drained_48k`). Everything else generating late is
    /// unchanged — see [`crate::audio`] for why that is not merely cheaper but produces an
    /// identical stream.
    audio: Audio,
}

impl Ula {
    /// A ULA at the start of frame zero, fronting `memory`, with no tape in the drive.
    ///
    /// **The frame geometry comes from `memory`**, not from a second argument and not from a
    /// default. A 128's frame is 1020 T-states longer than a 48K's, and a machine that got its
    /// memory map from one model and its clock from the other would raise its interrupt early
    /// every frame and drift by a whole frame every 69 seconds — silently, because nothing
    /// else would move. Taking both from one value makes that unrepresentable rather than
    /// merely unlikely.
    #[must_use]
    pub fn new(memory: Memory) -> Self {
        let model = memory.model();
        Self {
            memory,
            keyboard: Keyboard::new(),
            joystick: Joystick::new(),
            clock: Clock::with_timing(model.timing()),
            covered_t_states: 0,
            border: BorderTrace::new(Colour::BLACK),
            tape: Tape::default(),
            ear_reads: 0,
            // The sound chip comes from the same value the clock and the map do, for the same
            // reason: a machine with a 48K's memory and a 128's chip is a machine nobody
            // built, and taking every model-dependent thing from one place makes it
            // unrepresentable rather than merely unlikely.
            audio: Audio::new(model),
        }
    }

    /// Put the clock back to the start of frame zero, clear the border, and restore the
    /// power-on memory map.
    ///
    /// RAM, the keyboard, **the joystick** and **the tape** are left alone: a reset button does
    /// not clear RAM, does not lift the keys, does not let go of the stick, and does not rewind
    /// a cassette. The ROM's own start-up clears what it relies on. *(The joystick was missing
    /// from this list, which is the kind of omission that reads as a decision: it is a peripheral
    /// on the far side of an edge connector and a reset cannot reach it, exactly as it cannot
    /// reach the membrane.)*
    ///
    /// **The map is the M7 addition, and the old sentence was right about RAM and incomplete
    /// about the rest.** Reset is the only thing that clears the paging lock, so it is the only
    /// way a locked 128 ever pages again — which makes `Memory::reset` part of what the reset
    /// button does rather than an extra. It is a no-op on a 48K by construction, because that
    /// machine's reset value is the value it already holds.
    ///
    /// The clock keeps its machine's geometry: a reset is not a change of model.
    /// **Sound is reset too**, which the sentence above about RAM and the tape does not cover
    /// and which is a different kind of claim: the reset line genuinely reaches the AY, so a
    /// 128 that was playing goes quiet. What is *not* discarded is the sample buffer — those
    /// samples describe sound the machine really made before the button was pressed, and a
    /// consumer that has not drained them yet is owed them.
    ///
    /// **[`Ula::ear_reads`] is not reset either, and that is load-bearing rather than an
    /// oversight.** It is a running total whose only use is to be sampled twice and
    /// subtracted, so a caller holding an earlier reading would underflow against a counter
    /// that went backwards — the reset button would silently corrupt a rate nobody reset.
    pub fn reset(&mut self) {
        self.clock = Clock::with_timing(self.clock.timing());
        self.covered_t_states = 0;
        self.border.set(Colour::BLACK);
        self.memory.reset();
        self.audio.reset();
    }

    /// The memory this ULA fronts.
    #[must_use]
    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    /// The memory this ULA fronts, mutably.
    pub fn memory_mut(&mut self) -> &mut Memory {
        &mut self.memory
    }

    /// The keyboard.
    #[must_use]
    pub fn keyboard(&self) -> &Keyboard {
        &self.keyboard
    }

    /// The keyboard, mutably — how a frontend or a test presses a key.
    pub fn keyboard_mut(&mut self) -> &mut Keyboard {
        &mut self.keyboard
    }

    /// The Kempston joystick.
    #[must_use]
    pub fn joystick(&self) -> Joystick {
        self.joystick
    }

    /// The Kempston joystick, mutably — how a frontend pushes it.
    pub fn joystick_mut(&mut self) -> &mut Joystick {
        &mut self.joystick
    }

    /// The frame clock.
    #[must_use]
    pub fn clock(&self) -> Clock {
        self.clock
    }

    /// The colour last written to the border.
    #[must_use]
    pub fn border(&self) -> Colour {
        self.border.current()
    }

    /// Where the border was as the beam went down the frame.
    ///
    /// `pub(crate)` because the only caller is [`crate::Spectrum::render`], which is what a
    /// frontend already calls — so the stripes arrive with **no** public API change at all.
    /// A public accessor here would be a type nothing outside this crate can do anything
    /// with, which `docs/M6.md` Decision 1 calls decoration.
    pub(crate) fn border_trace(&self) -> &BorderTrace {
        &self.border
    }

    /// The sound chip, on a machine that has one.
    ///
    /// `None` on a 48K, which does not contain the chip at all — so the `Option` is the
    /// machine's shape rather than a caller's convenience. Everything an [`Ay`] exposes is
    /// read-only, because the only things that may change it are a guest's `OUT` and a
    /// snapshot restore, and both already have a route.
    #[must_use]
    pub fn ay(&self) -> Option<&Ay> {
        self.audio.ay()
    }

    /// The sound chip, mutably. The snapshot applier's route, and nothing else's.
    pub(crate) fn ay_mut(&mut self) -> Option<&mut Ay> {
        self.audio.ay_mut()
    }

    /// Generate the sound up to now and hand over everything since the last call.
    ///
    /// **The only route to the machine's audio**, and the whole of the consumer's job is to
    /// call it once a frame and copy what it returns. It borrows rather than allocating; the
    /// buffer behind it is allocated once, at construction, and refilled in place.
    ///
    /// The sources arrive **separate** — [`crate::audio::Sample`] carries the AY's three
    /// channels, the beeper and the tape's `EAR` line as distinct numbers — because mixing them
    /// is the frontend's job and because the AY's own gate must not be falsifiable by the
    /// beeper. *(This said "the two sources … the AY's three channels and the beeper". The tape
    /// became the third when M8 routed the `EAR` line to the speaker, and this crate's own
    /// coverage table above has named it since.)*
    ///
    /// Calling it twice in a row yields the second call nothing, which is what taking means.
    /// A consumer that calls it less often than once a frame loses samples and can find out
    /// how many from [`Ula::dropped_samples`].
    pub fn take_samples(&mut self) -> &[Sample] {
        let now = self.clock.t_states();
        self.audio.take(now)
    }

    /// Samples lost because [`Ula::take_samples`] was not called often enough.
    ///
    /// Zero for a consumer draining once a frame. Reported rather than swallowed, because an
    /// unexplained gap in audio gets blamed on everything except the buffer that caused it.
    #[must_use]
    pub fn dropped_samples(&self) -> u64 {
        self.audio.dropped()
    }

    /// Times a guest has read the `EAR` line since power-on.
    ///
    /// A running total, in the shape of [`Ula::dropped_samples`] and [`Clock::frames`]:
    /// **monotonic**, never reset — not by [`Ula::reset`], not by a snapshot — so a caller that
    /// samples it twice can always subtract. See [`crate::Spectrum::ear_reads`] for what the
    /// number is for and why it is a count rather than a verdict.
    #[must_use]
    pub fn ear_reads(&self) -> u64 {
        self.ear_reads
    }

    /// Whether the ULA is holding `/INT` low right now.
    #[must_use]
    pub fn interrupt_asserted(&self) -> bool {
        self.clock.interrupt_asserted()
    }

    /// Set the border **without performing an `OUT`**.
    ///
    /// The one thing a snapshot applier needs that no existing method provides, and the
    /// reason is concrete rather than stylistic. Routing a restore through [`Ula::out_port`]
    /// charges a four-T-state I/O cycle that never happened, and two consequences follow that
    /// `crates/spectrum/tests/snapshot_apply.rs` **measured** — both leaving R1 and R3 green,
    /// which is why they need assertions of their own:
    ///
    /// - **The tape moves.** A port cycle advances the clock by its contention stall, and the
    ///   tape advances with the clock. Restoring a snapshot is not elapsed time. This one
    ///   survives whatever order the applier's setters run in.
    /// - **`covered_t_states` is left armed at 4**, so the next four **bare ticks** are
    ///   treated as an open cycle's own and skip their contention. An interrupt acknowledge is
    ///   seven bare ticks with no transfer between them.
    ///
    /// The second bullet used to read *"the first four ticks of the next **instruction**"*, in
    /// this comment and in `docs/M6.md`, and that is **false**: every instruction opens with a
    /// fetch, and `begin_memory_cycle` assigns `covered_t_states` unconditionally, so a stale
    /// value is overwritten before it can be spent. Nothing reachable through
    /// [`crate::Spectrum::step`] can see it. The correction is recorded rather than applied
    /// silently, because the setter is still unavoidable and a reader who checked the stated
    /// reason would have found it did not hold.
    ///
    /// It does not touch the clock, the tape, or the cycle bookkeeping, because setting state
    /// is not elapsed time.
    pub(crate) fn set_border(&mut self, border: Colour) {
        // Through `BorderTrace::set`, which drops the record as well as setting the colour:
        // the machine being restored into did not paint the bands the saved machine did, and
        // keeping them would draw a history that never happened here. Same reason this setter
        // exists at all — a restore is not elapsed time.
        self.border.set(border);
    }

    /// Put the clock at `frame_t_state`, leaving the frame counter alone.
    ///
    /// The applier's second unavoidable setter. [`Clock::advance`] is `pub(crate)` and
    /// [`Ula::clock`] returns a `Copy` **by value**, so there is no route to this machine's
    /// clock from outside that is not a silent no-op — which is deliberate, and is why the
    /// applier needs a setter here rather than reaching through the accessor.
    ///
    /// The frame **counter** is not touched: it is the machine's uptime since power-on, the
    /// boot gate asserts on it and the FLASH phase derives from it, so rewinding it on a load
    /// would make one number mean two things. That is a convention — no format carries a
    /// frame count — and its cost is a snapshot taken mid-flash rendering inverted for up to
    /// [`crate::screen::FLASH_FRAMES`] frames after loading.
    ///
    /// The tape does not move either: restoring a snapshot is not elapsed time.
    /// **The audio's time base moves with it, and does not generate a sample.** This is the
    /// one operation that can move the clock *backwards*, and an integrator that met a
    /// backwards jump would either render nothing forever or manufacture a frame of sound out
    /// of the discontinuity. `Audio::rebase` is the explicit re-basing that avoids both, and
    /// it is called here rather than hidden inside the generator so that a future caller who
    /// moves the clock has to think about it.
    pub(crate) fn set_frame_t_state(&mut self, frame_t_state: u32) {
        self.clock.set_frame_t_state(frame_t_state);
        self.audio.rebase(self.clock.t_states());
    }

    /// Put `tape` in the drive, stopped and wound to wherever it stands.
    pub(crate) fn insert_tape(&mut self, tape: Tape) {
        self.tape = tape;
    }

    /// The tape in the drive, to read — where the head is, and whether it is moving.
    ///
    /// The immutable half of the pair below, added because [`crate::Spectrum::tape`] cannot
    /// reach a private field of this struct without it. Nothing else in this file needs it:
    /// [`Ula::ear_bit`] reads `self.tape` directly, being inside the type.
    pub(crate) fn tape(&self) -> &Tape {
        &self.tape
    }

    /// The tape in the drive — how anything starts, stops or rewinds it.
    pub(crate) fn tape_mut(&mut self) -> &mut Tape {
        &mut self.tape
    }

    /// Let `t_states` elapse: the clock moves, the tape moves with it, and the speaker hears
    /// the tape if the tape moved far enough to flip.
    ///
    /// **The one place time passes.** *(The summary line above used to name two of the three
    /// things this does. The third is conditional, which is why it now says so rather than
    /// being left out.)* Contention advances the clock by a stall of 0–6 T-states outside any
    /// `Ula::tick`, so a tape driven from `tick` alone would run slow by exactly the contention
    /// a loader suffers — and would do it silently. Every `Clock::advance` call site in this
    /// file routes through here for that reason;
    /// `crates/spectrum/tests/tape_signal.rs` asserts that a stalled access moves the tape.
    ///
    /// **The edge's timestamp is approximated, and by how much is worth writing down.** The
    /// clock has already advanced by the whole of `t_states` when the level is read, so an edge
    /// that fell *inside* the advance is stamped at its **end** — late by up to `t_states`. That
    /// is 1 for a `tick`, up to 6 for a contention stall and up to 12 across a contended I/O
    /// cycle, against a 32-T-state sample window: at worst the edge lands one sample later than
    /// it should. The read side carries the mirror-image approximation and states it to the
    /// T-state above; this is the write side of the same trade, and it is taken for the same
    /// reason — a mid-advance timestamp would need the tape to report *when* it flipped rather
    /// than *that* it did, which is a wider signature for a third of a sample.
    #[inline]
    fn advance(&mut self, t_states: u32) {
        self.clock.advance(t_states);
        // **And the tape reaches the speaker, not only bit 6.** On a real machine the `EAR`
        // socket feeds the amplifier as well as the ULA's input, which is why a loading tape is
        // audible. Routing it to [`Ula::ear_bit`] alone loads tapes correctly and in silence.
        //
        // **The `if` is load-bearing and it was measured.** This first read the level
        // unconditionally and let `Audio::set_tape` compare — the shape `set_beeper` uses, and
        // the wrong one here, because the *timestamp* argument is evaluated at the call site
        // before any guard can discard it. `Clock::t_states` is a `u64` multiply-add, and paying
        // it on every elapsed T-state cost **+21.9% on `benches/frame.rs`'s `quiet_48k`** — a
        // machine with no tape in the drive. `Tape::advance` already knew when the level moved;
        // now it says so, and the multiply happens only when there is something to timestamp.
        if self.tape.advance(t_states) {
            self.audio
                .set_tape(self.tape.level(), self.clock.t_states());
        }
    }

    /// Bit 6 of a `0xFE` read: the level the tape is driving the `EAR` line to.
    #[inline]
    fn ear_bit(&self) -> u8 {
        if self.tape.level() { EAR_BIT } else { 0 }
    }

    /// Charge the stall a contended access starting *now* at `address` would suffer.
    ///
    /// **The hottest line in the emulator, and M7 does not add a branch to it.** Whether an
    /// address is contended has been a question about the slot map rather than the address
    /// range since M5, so the 128's four contended banks are a different array literal and not
    /// a different test. What did change is that the delay's inputs are fields of the
    /// machine's [`Clock`] rather than compile-time constants — same branches, a load instead
    /// of an immediate.
    #[inline]
    fn contend(&mut self, address: u16) {
        if self.memory.is_contended(address) {
            self.advance(self.clock.delay_now());
        }
    }

    /// The stall an I/O cycle at `port` suffers, in T-states.
    ///
    /// Four cases, and they are not a simplification of one rule — the ULA contends
    /// because it owns the port *and* because the address happens to be in contended
    /// memory, and the two combine:
    ///
    /// | Address contended | ULA port | Pattern |
    /// |---|---|---|
    /// | no | no | `N:4` |
    /// | no | yes | `N:1, C:3` |
    /// | yes | no | `C:1, C:1, C:1, C:1` |
    /// | yes | yes | `C:1, C:3` |
    ///
    /// `C` applies the ULA's delay unconditionally: the address test is already the row.
    /// Each stall shifts the ones after it, which is why the offsets accumulate. The whole
    /// stall is charged before the cycle's four nominal T-states arrive as ticks; the
    /// clock ends in the same place, and nothing observes it in between.
    #[inline]
    fn port_delay(&self, port: u16) -> u32 {
        let contended_address = self.memory.is_contended(port);
        let ula_port = port & ULA_PORT_SELECT == 0;
        match (contended_address, ula_port) {
            (false, false) => 0,
            (false, true) => self.delay_after(1),
            (true, true) => {
                let first = self.delay_after(0);
                first + self.delay_after(first + 1)
            }
            (true, false) => {
                let first = self.delay_after(0);
                let second = self.delay_after(first + 1);
                let third = self.delay_after(first + second + 2);
                let fourth = self.delay_after(first + second + third + 3);
                first + second + third + fourth
            }
        }
    }

    /// The ULA delay at the frame position `offset` T-states from now.
    #[inline]
    fn delay_after(&self, offset: u32) -> u32 {
        self.clock.delay_after(offset)
    }

    /// Open a memory machine cycle of `t_states`, charging its contention.
    #[inline]
    fn begin_memory_cycle(&mut self, address: u16, t_states: u8) {
        self.contend(address);
        self.covered_t_states = t_states;
    }

    /// Open an I/O machine cycle, charging its contention.
    #[inline]
    fn begin_port_cycle(&mut self, port: u16) {
        self.advance(self.port_delay(port));
        self.covered_t_states = PORT_ACCESS_T_STATES;
    }
}

impl Bus for Ula {
    #[inline]
    fn fetch(&mut self, address: u16) -> u8 {
        self.begin_memory_cycle(address, OPCODE_FETCH_T_STATES);
        self.memory.read(address)
    }

    #[inline]
    fn read(&mut self, address: u16) -> u8 {
        self.begin_memory_cycle(address, MEMORY_ACCESS_T_STATES);
        self.memory.read(address)
    }

    #[inline]
    fn write(&mut self, address: u16, value: u8) {
        self.begin_memory_cycle(address, MEMORY_ACCESS_T_STATES);
        self.memory.write(address, value);
    }

    /// The AY is asked **before** the floating-bus fallback, and the order is load-bearing.
    ///
    /// `0xFFFD` has A0 set, so it falls into the "no ULA port, therefore floating bus" arm
    /// that used to be the second line of this function. An AY arm placed after it would
    /// never run, and the machine would read as *"the sound chip is write-only"* — which is
    /// silent, plausible, and exactly the kind of defect that shows up as one game misbehaving
    /// years later. `docs/M7.md` Decision 2 names this ordering specifically.
    ///
    /// **Where the two devices genuinely collide, this fixes an order and that is a ruling.**
    /// An address with A0 and A1 reset and A15, A14 set — `0xFFFC`, say — is claimed by the
    /// ULA *and* by the AY, and on the hardware both drive the data bus at once. There is no
    /// right answer to be found by thinking harder about it. The AY wins here because it
    /// decodes three address lines to the ULA's one, so giving it priority changes the fewest
    /// addresses; nothing in reach exercises the case, because every published address for
    /// either port has A0 the other way.
    #[inline]
    fn in_port(&mut self, port: u16) -> u8 {
        self.begin_port_cycle(port);
        if port & AY_PORT_MASK == AY_SELECT_PORT
            && let Some(ay) = self.audio.ay()
        {
            return ay.read();
        }
        // **The joystick answers before the ULA, and that is a ruling.** Its decode is three
        // address lines to the ULA's one, and `KEMPSTON_PORT_MASK` consults **no high-byte line
        // at all** — which `joystick.rs`'s own tests assert deliberately — so the overlap is
        // every address with A0, A5, A6 and A7 clear: **8192 of 65536**, `0x7F00` and `0xFE00`
        // among them. It is real on hardware, where a fitted Kempston and the ULA would both
        // drive the data bus. There is no right answer to find by thinking harder, so the
        // narrower decode wins.
        //
        // *(This described the overlap as "every even port from `0x00` to `0x1E`" — the low byte
        // only, 16 addresses, 512 times too few, and a description a mask consulting no high line
        // cannot have. It also cited "the AY two arms below" as the same ruling. Both halves of
        // that were wrong: the AY's arm **in this function** is a single `if`, and it is above
        // rather than below; the only two-arm AY site is in `out_port`, where the arms are
        // unordered and everything that matches fires, so no decode "wins" there at all.)*
        //
        // Nothing in reach exercises it: the keyboard is scanned at `0xFE` and every
        // "is any key down" idiom is `IN A,(0xFE)` with a high half, whose low byte has A5
        // set. `tests/kempston.rs` pins the choice so it is not silently reversed.
        if port & KEMPSTON_PORT_MASK == KEMPSTON_PORT_SELECT {
            return self.joystick.read();
        }
        if port & ULA_PORT_SELECT != 0 {
            return FLOATING_BUS_BYTE;
        }
        // Counted here rather than inside [`Ula::ear_bit`], which stays a pure read of the
        // line. This is the arm where the `EAR` bit actually reaches a guest — the three above
        // return before it — so this is the one place *"the machine looked at the tape"* is
        // true, whether or not the guest went on to test bit 6.
        self.ear_reads += 1;
        self.keyboard.read(port) | UNDRIVEN_INPUT_BITS | self.ear_bit()
    }

    /// **Four** independent `if`s, **never a `match` on the port**.
    ///
    /// The ULA decodes A0 alone, the paging port decodes A15 and A1, and the AY's select and
    /// write ports take an arm each, so the families overlap — `0x00FE` is claimed by two of
    /// them, and on the hardware every device that matches responds. A `match` would silently
    /// pick one and stop the others answering, which is the kind of defect that shows up as
    /// "this game works on one emulator and not another" rather than as a failure.
    ///
    /// *(This said **two**, and stopped being true when M7 added the AY's pair — which the
    /// comment beside those very arms explains at length. The number is what a reader checks
    /// the function against, so it is corrected here rather than dropped; the shape argument
    /// under it never depended on the count and is unchanged.)*
    #[inline]
    fn out_port(&mut self, port: u16, value: u8) {
        self.begin_port_cycle(port);
        if port & ULA_PORT_SELECT == 0 {
            // Bit 4 is the speaker and it is **no longer dropped**: this line's predecessor
            // read *"discarded until M6 and M8 want them"*, `M8.md` Decision 9 overruled it —
            // the beeper is a ULA output and therefore the machine's — and the comment then
            // sat here for a milestone calling itself *"an open finding, not a deferral"*.
            // It is closed. `MIC`, bit 3, is still dropped, and `MIC_BIT` says what that costs.
            //
            // `set_beeper` compares before it renders, so a border-only write — which is what
            // almost every write to this port is — costs one comparison and no work.
            //
            // **One write, one event, two consumers.** The border and the speaker come out of
            // the same byte at the same instant, so the clock is read once and handed to both.
            // What they need from it differs and that is why there is one *record* rather than
            // two: the border must be remembered until something renders, so it has a history;
            // the speaker is turned into samples immediately, so it has none. Keeping a shared
            // log and deriving both from it was considered and rejected — the two have
            // different lifetimes (a frame against a consumer's drain schedule) and different
            // overflow semantics, and coupling them would let a display-shaped bound drop audio.
            let colour = Colour::new(value & BORDER_MASK);
            if colour != self.border.current() {
                self.border.record(self.clock, colour);
            }
            self.audio
                .set_beeper(value & SPEAKER_BIT != 0, self.clock.t_states());
        }
        if port & PAGING_PORT_SELECT == 0 {
            // A 48K absorbs this: it powers on with the lock bit set, and `Memory` needs no
            // model check to know that. `M7.md` Decision 1.
            self.memory.write_paging_port(value);
        }
        // The AY's two ports differ in A14 alone and both need A15 set, so neither can be the
        // paging port above. They are independent `if`s for the same reason every other decode
        // here is: on the hardware every device that matches answers, and a `match` on the port
        // would silently pick one. A machine with no chip reaches nothing, which is what a
        // 48K's `OUT (0xBFFD)` does.
        if port & AY_PORT_MASK == AY_SELECT_PORT {
            self.audio.select_ay(value);
        }
        if port & AY_PORT_MASK == AY_WRITE_PORT {
            let now = self.clock.t_states();
            self.audio.write_ay(value, now);
        }
    }

    /// One machine cycle, `t_states` long, with the refresh address on the bus and no
    /// transfer — the acknowledge of an accepted interrupt.
    ///
    /// Charged exactly like every other cycle: contention once at the position the cycle
    /// opens, then `t_states` ticks that are already paid for. Before this existed the seven
    /// ticks arrived bare and each contended on its own account, so a contended `IR` was
    /// charged seven stalls where it owes one. See the module documentation for why that was
    /// left standing until M7 and why the reason given for leaving it was wrong.
    #[inline]
    fn acknowledge(&mut self, address: u16, t_states: u8) {
        self.begin_memory_cycle(address, t_states);
    }

    #[inline]
    fn tick(&mut self, address: u16) {
        match self.covered_t_states.checked_sub(1) {
            // Inside an open cycle: its contention was charged when the cycle opened.
            Some(remaining) => self.covered_t_states = remaining,
            // Nothing open: a standalone internal cycle, contending on its own account.
            None => self.contend(address),
        }
        self.advance(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::Key;
    use crate::memory::PAGE_SIZE;
    use crate::timing::{FIRST_CONTENDED_T_STATE, T_STATES_PER_FRAME};

    /// An uncontended address in the top slot, and a contended one in the screen bank.
    const UNCONTENDED: u16 = 0x8000;
    const CONTENDED: u16 = 0x4000;

    fn ula() -> Ula {
        Ula::new(Memory::spectrum_48k(&[0; PAGE_SIZE]).expect("a page-sized ROM"))
    }

    /// Move the clock to `frame_t_state` without going through the bus.
    fn at(frame_t_state: u32) -> Ula {
        let mut ula = ula();
        ula.clock.advance(frame_t_state);
        ula
    }

    /// One memory-read machine cycle as the core issues it: the transfer, then three ticks.
    fn read_cycle(ula: &mut Ula, address: u16) {
        ula.read(address);
        for _ in 0..3 {
            ula.tick(address);
        }
    }

    #[test]
    fn an_uncontended_read_costs_exactly_its_three_t_states() {
        let mut ula = at(FIRST_CONTENDED_T_STATE);
        read_cycle(&mut ula, UNCONTENDED);
        assert_eq!(ula.clock.frame_t_state(), FIRST_CONTENDED_T_STATE + 3);
    }

    #[test]
    fn a_contended_read_is_stalled_by_the_pattern_at_its_start() {
        for (offset, expected) in [(0, 6), (1, 5), (5, 1), (6, 0), (7, 0)] {
            let start = FIRST_CONTENDED_T_STATE + offset;
            let mut ula = at(start);
            read_cycle(&mut ula, CONTENDED);
            assert_eq!(
                ula.clock.frame_t_state(),
                start + expected + 3,
                "a read starting at +{offset} should stall {expected}"
            );
        }
    }

    #[test]
    fn contention_outside_the_fetch_window_is_free() {
        let mut ula = at(FIRST_CONTENDED_T_STATE - 1);
        read_cycle(&mut ula, CONTENDED);
        assert_eq!(ula.clock.frame_t_state(), FIRST_CONTENDED_T_STATE + 2);
    }

    #[test]
    fn an_opcode_fetch_contends_once_and_not_four_times() {
        // Four ticks at the fetch address are one machine cycle, not four internal ones.
        let start = FIRST_CONTENDED_T_STATE;
        let mut ula = at(start);
        ula.fetch(CONTENDED);
        for _ in 0..4 {
            ula.tick(CONTENDED);
        }
        assert_eq!(
            ula.clock.frame_t_state(),
            start + 6 + 4,
            "delay(+0)=6 for the fetch's stall, then its four T-states and nothing else"
        );
    }

    #[test]
    fn a_fetch_and_a_read_charge_the_same_tick_stream_differently() {
        // What `Bus::fetch` bought, as the one assertion that could not be written before
        // it existed. Both runs are a transfer followed by four ticks at one address —
        // byte-identical streams. An M1 cycle is four T-states, so all four are covered; a
        // memory read is three, so the fourth tick is an internal cycle and contends.
        let start = FIRST_CONTENDED_T_STATE;

        let mut fetched = at(start);
        fetched.fetch(CONTENDED);
        for _ in 0..4 {
            fetched.tick(CONTENDED);
        }
        assert_eq!(fetched.clock.frame_t_state(), start + 6 + 4);

        let mut read = at(start);
        read.read(CONTENDED);
        for _ in 0..4 {
            read.tick(CONTENDED);
        }
        //   read   at +0 -> delay(0)=6, clock +6, three T-states -> +9
        //   tick 4 at +9 -> internal, delay(9)=5, clock +14, +1 -> +15
        assert_eq!(
            read.clock.frame_t_state(),
            start + 15,
            "the read's fourth tick is an internal cycle and must be charged"
        );
    }

    #[test]
    fn internal_cycles_after_a_read_each_contend() {
        // The `JR` shape: read the displacement, then five internal cycles at the same
        // address. Six contention points, priced at six successive positions.
        let start = FIRST_CONTENDED_T_STATE;
        let mut ula = at(start);
        ula.read(CONTENDED);
        for _ in 0..8 {
            ula.tick(CONTENDED);
        }

        // Priced by hand from the pattern, one stall at a time:
        //   read   at +0  -> 6, clock +6, three T-states -> +9
        //   tick 4 at +9  -> (9)%8=1 -> 5, clock +14, +1 -> +15
        //   tick 5 at +15 -> (15)%8=7 -> 0, +1 -> +16
        //   tick 6 at +16 -> 6, clock +22, +1 -> +23
        //   tick 7 at +23 -> (23)%8=7 -> 0, +1 -> +24
        //   tick 8 at +24 -> 6, +1 -> +31
        assert_eq!(ula.clock.frame_t_state(), start + 31);
    }

    #[test]
    fn internal_cycles_on_an_uncontended_refresh_address_are_free() {
        // `ADD HL,BC` executing from contended memory: the fetch stalls, the seven
        // internal cycles sit on the refresh address, which points into the ROM.
        let start = FIRST_CONTENDED_T_STATE;
        let mut ula = at(start);
        ula.fetch(CONTENDED);
        for _ in 0..4 {
            ula.tick(CONTENDED);
        }
        for _ in 0..7 {
            ula.tick(0x3F00);
        }
        assert_eq!(ula.clock.frame_t_state(), start + 6 + 11);
    }

    #[test]
    fn a_write_cycle_is_three_t_states_and_its_internal_cycles_are_charged() {
        // `LDIR`: write, then two internal cycles at the write address. A write is never
        // four T-states, so nothing here is deferred.
        let start = FIRST_CONTENDED_T_STATE;
        let mut ula = at(start);
        ula.write(CONTENDED, 0);
        for _ in 0..5 {
            ula.tick(CONTENDED);
        }
        //   write  at +0 -> 6, clock +6, three T-states -> +9
        //   tick 4 at +9 -> (9)%8=1 -> 5, clock +14, +1 -> +15
        //   tick 5 at +15 -> (15)%8=7 -> 0, +1 -> +16
        assert_eq!(ula.clock.frame_t_state(), start + 16);
    }

    #[test]
    fn the_read_modify_write_internal_cycle_is_charged() {
        // The half of `INC (HL)` that this machine used to get wrong: a read, one internal
        // cycle at the address just read, then the write-back. Under the deferral heuristic
        // that internal cycle was indistinguishable from an opcode fetch's fourth T-state
        // and its stall was dropped, which cost one contention point on every execution of
        // the whole read-modify-write family. All three cycles must now be charged.
        let start = FIRST_CONTENDED_T_STATE;
        let mut ula = at(start);
        ula.read(CONTENDED);
        for _ in 0..3 {
            ula.tick(CONTENDED);
        }
        ula.tick(CONTENDED);
        ula.write(CONTENDED, 0);
        for _ in 0..3 {
            ula.tick(CONTENDED);
        }
        //   read     at +0  -> delay(0)=6, clock +6, three T-states -> +9
        //   internal at +9  -> delay(9)=5, clock +14, one T-state   -> +15
        //   write    at +15 -> delay(15)=0, three T-states          -> +18
        //
        // The heuristic reached +17 by dropping the stall at +9 and then charging the write
        // four T-states early at +10, where the pattern happens to stall 4 — so the visible
        // error was one T-state, not the five the missing stall suggests. Each stall shifts
        // the ones after it, which is why a missing one cannot be added to the total.
        assert_eq!(ula.clock.frame_t_state(), start + 18);
    }

    #[test]
    fn the_four_port_contention_cases_are_all_distinct() {
        // Priced by hand from the published table, at a frame position where the pattern
        // starts at 6. `C:n` stalls, then n T-states pass, so each stall shifts the next.
        //
        //   N:4                  0x8001  0
        //   N:1, C:3             0x8000  delay(+1)=5
        //   C:1, C:3             0x4000  delay(+0)=6, then delay(+7)=0
        //   C:1, C:1, C:1, C:1   0x4001  6, delay(+7)=0, delay(+8)=6, delay(+15)=0
        let start = FIRST_CONTENDED_T_STATE;
        let cases = [(0x8001_u16, 0_u32), (0x8000, 5), (0x4000, 6), (0x4001, 12)];
        for (port, stall) in cases {
            let mut ula = at(start);
            ula.in_port(port);
            for _ in 0..4 {
                ula.tick(port);
            }
            assert_eq!(
                ula.clock.frame_t_state(),
                start + stall + 4,
                "port {port:#06X} should stall {stall} T-states"
            );
        }
    }

    #[test]
    fn reading_the_keyboard_port_reports_the_selected_half_row() {
        let mut ula = ula();
        ula.keyboard_mut().press(Key::Enter);
        assert_eq!(ula.in_port(0xBFFE), UNDRIVEN_INPUT_BITS | 0x1E);
        assert_eq!(ula.in_port(0xFEFE), UNDRIVEN_INPUT_BITS | 0x1F);
    }

    #[test]
    fn an_undecoded_port_reads_as_a_floating_bus() {
        let mut ula = ula();
        assert_eq!(ula.in_port(0xFFFF), FLOATING_BUS_BYTE);
    }

    #[test]
    fn writing_the_ula_port_latches_the_border_colour() {
        let mut ula = ula();
        assert_eq!(ula.border(), Colour::BLACK);
        // Bits above 2 are MIC, the speaker, and nothing.
        ula.out_port(0x00FE, 0xF8 | 5);
        assert_eq!(ula.border(), Colour::new(5));
    }

    #[test]
    fn writing_a_port_the_ula_does_not_own_leaves_the_border_alone() {
        let mut ula = ula();
        ula.out_port(0x00FE, 2);
        ula.out_port(0x7FFD, 5);
        assert_eq!(ula.border(), Colour::new(2));
    }

    #[test]
    fn the_clock_is_the_bus_and_it_rolls_over_mid_instruction() {
        // MACHINE.md Decision 2: nothing stops a machine cycle on a frame boundary.
        let mut ula = at(T_STATES_PER_FRAME - 2);
        read_cycle(&mut ula, UNCONTENDED);
        assert_eq!(ula.clock.frames(), 1);
        assert_eq!(ula.clock.frame_t_state(), 1);
    }

    #[test]
    fn with_no_tape_the_ear_bit_reads_low() {
        // The literal a real `IN A,(0xFE)` returns on an idle machine with no key held.
        // `tests/keyboard_matrix.rs` pins the same value from outside this file, across the
        // whole membrane, which is what makes the M6 change graded rather than asserted here.
        let mut ula = ula();
        assert_eq!(ula.in_port(0xFEFE), 0xBF);
        assert_eq!(
            0xBF,
            UNDRIVEN_INPUT_BITS | 0x1F,
            "bit 6 clear, bits 0-4 high"
        );
    }

    #[test]
    fn the_ear_bit_follows_the_tape_and_nothing_else_does() {
        // Bit 6 tracks the level; the keyboard bits must not move with it.
        let mut ula = ula();
        ula.insert_tape(Tape::new(vec![4, 4]));
        assert_eq!(ula.in_port(0xFEFE) & EAR_BIT, 0, "low before playback");

        ula.tape_mut().play();
        ula.tick(UNCONTENDED);
        ula.tick(UNCONTENDED);
        ula.tick(UNCONTENDED);
        ula.tick(UNCONTENDED);
        assert_eq!(ula.in_port(0xFEFE), 0xBF | EAR_BIT, "high after one pulse");
        assert_eq!(
            ula.in_port(0xFEFE) & !EAR_BIT,
            0xBF,
            "and nothing else moved"
        );
    }

    #[test]
    fn a_contention_stall_moves_the_tape_and_not_only_the_ticks() {
        // The property `Ula::advance` exists for. A contended read at this phase costs
        // 6 T-states of stall and then its 3 nominal ones; an uncontended one costs 3. A tape
        // driven from `tick` alone would see 3 in both cases and run slow by exactly the
        // contention a loader suffers — silently, because nothing else would move.
        //
        // The first half-period is 9 T-states long, so the level flips at the end of the
        // contended read and does not flip at the end of the uncontended one. One tape, two
        // addresses, and the only difference between the runs is the stall.
        for (address, flipped) in [(CONTENDED, true), (UNCONTENDED, false)] {
            let mut ula = at(FIRST_CONTENDED_T_STATE);
            ula.insert_tape(Tape::new(vec![9, 100]));
            ula.tape_mut().play();
            read_cycle(&mut ula, address);
            assert_eq!(
                ula.tape.level(),
                flipped,
                "a read at {address:#06X} left the tape at the wrong place"
            );
        }
    }

    #[test]
    fn setting_the_border_charges_no_machine_cycle() {
        // Why `set_border` exists at all. `out_port` would move the clock by an I/O cycle's
        // contention and leave `covered_t_states` armed at four, so the next instruction's
        // first four ticks would be charged as an open cycle and skip their contention.
        let mut ula = at(FIRST_CONTENDED_T_STATE);
        ula.set_border(Colour::new(5));
        assert_eq!(ula.border(), Colour::new(5));
        assert_eq!(ula.clock.frame_t_state(), FIRST_CONTENDED_T_STATE);
        assert_eq!(ula.covered_t_states, 0, "no cycle is open");

        // The same border through the bus does all three of those things.
        let mut through_the_bus = at(FIRST_CONTENDED_T_STATE);
        through_the_bus.out_port(0x00FE, 5);
        assert_ne!(
            through_the_bus.clock.frame_t_state(),
            FIRST_CONTENDED_T_STATE
        );
        assert_eq!(through_the_bus.covered_t_states, PORT_ACCESS_T_STATES);
    }

    #[test]
    fn setting_the_frame_position_moves_neither_the_frame_counter_nor_the_tape() {
        let mut ula = ula();
        ula.insert_tape(Tape::new(vec![10, 10]));
        ula.tape_mut().play();
        ula.clock.advance(T_STATES_PER_FRAME * 2 + 5);

        ula.set_frame_t_state(30_000);
        assert_eq!(ula.clock.frame_t_state(), 30_000);
        assert_eq!(ula.clock.frames(), 2, "uptime is not rewound by a load");
        assert!(!ula.tape.level(), "a restore is not elapsed time");
    }

    #[test]
    fn reset_does_not_rewind_the_tape() {
        // Pressing reset does not rewind a cassette.
        let mut ula = ula();
        ula.insert_tape(Tape::new(vec![2, 2, 2]));
        ula.tape_mut().play();
        ula.tick(UNCONTENDED);
        ula.tick(UNCONTENDED);
        assert!(ula.tape.level());
        ula.reset();
        assert!(ula.tape.level(), "the head stays where it was");
        assert_eq!(ula.clock, Clock::new());
    }

    #[test]
    fn reset_returns_the_clock_to_the_start_without_disturbing_memory() {
        let mut ula = ula();
        ula.memory_mut().write(0x8000, 0xA5);
        ula.clock.advance(1234);
        ula.out_port(0x00FE, 3);
        ula.reset();
        assert_eq!(ula.clock, Clock::new());
        assert_eq!(ula.border(), Colour::BLACK);
        assert_eq!(ula.memory().read(0x8000), 0xA5);
    }
}
