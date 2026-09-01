//! A ZX Spectrum 48K: paged memory, the ULA, contention, the keyboard, and the frame loop.
//!
//! ```no_run
//! use spectrum::{Key, Spectrum};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let rom = std::fs::read("testdata/roms/48.rom")?;
//! let mut machine = Spectrum::new(&rom)?;
//!
//! machine.run_frames(100);              // the ROM's start-up and copyright message
//! machine.keyboard_mut().press(Key::P);
//! machine.run_frames(2);
//! # Ok(())
//! # }
//! ```
//!
//! # The two facts this crate is built around
//!
//! **The machine owns the clock.** [`z80::Cpu::step`] returns T-states, but contention adds
//! time on this side of the bus — measured at M1, where a contended bus left `step`'s
//! return identical to a flat run while the bus's own clock diverged. So the frame is
//! driven by counting [`z80::Bus::tick`] calls, and nothing here adds up what `step`
//! returns. A machine that did would get the instruction count right and the time wrong,
//! with nothing failing.
//!
//! **One step can overrun the frame.** A run of `DD`/`FD` prefixes is *one* instruction and
//! guest memory decides how long it is, so there is no maximum to stop before.
//! [`Spectrum::run_frame`] therefore watches for the frame *counter* to change rather than
//! trying to land on 69888, and [`timing::Clock`] rolls over however far it is pushed.
//!
//! # Verification, honestly
//!
//! M1–M4 had oracles: FUSE grades an instruction, `zexdoc` and `zexall` grade the
//! processor. **Nothing grades a machine.** What is asserted here, and by what:
//!
//! | Property | Evidence |
//! |---|---|
//! | Memory map, slot indirection, ROM write-protection | unit tests in [`memory`] |
//! | Screen address layout | unit tests in [`screen`], including the bijection over all 6144 bytes |
//! | Keyboard matrix | unit tests in [`keyboard`], every key on its own half-row |
//! | Frame length, interrupt window, contention pattern | unit tests in [`timing`] |
//! | Machine-cycle reconstruction | unit tests in [`machine_cycle`] |
//! | All of it together | **the boot gate**: the ROM reaching `© 1982 Sinclair Research Ltd` |
//! | Contention *phase* — [`timing::FIRST_CONTENDED_T_STATE`] | **nothing** |
//! | Floating bus | **nothing** — not modelled; see [`ula`] |
//! | Progressive drawing: multicolour, border stripes | **nothing** — not modelled; see [`screen`] |
//!
//! The last three rows are the point of this table. `docs/MACHINE.md` asks for what is
//! *not* covered to be written down rather than inferred from the absence of a failing
//! test, and those are the answers.

#![deny(missing_docs)]

pub mod keyboard;
pub mod machine_cycle;
pub mod memory;
pub mod screen;
pub mod timing;
pub mod ula;

pub use keyboard::{Key, Keyboard};
pub use memory::{Memory, RomSizeError};
pub use screen::{Colour, Frame};
pub use ula::{FLOATING_BUS_BYTE, Ula};

use z80::{Cpu, CpuState, StepError};

/// A ZX Spectrum 48K.
///
/// Owns the CPU, which owns the [`Ula`], which owns the [`Memory`]. That chain is the
/// `crates/z80` ownership model rather than a choice made here: the core takes its bus by
/// value so the calls monomorphise, and reaches it back out through
/// [`z80::Cpu::bus_mut`].
#[derive(Debug)]
pub struct Spectrum {
    cpu: Cpu<Ula>,
}

impl Spectrum {
    /// Build a 48K holding `rom`, at the start of frame zero.
    ///
    /// # Errors
    ///
    /// [`RomSizeError`] if `rom` is not exactly one 16 KB page.
    pub fn new(rom: &[u8]) -> Result<Self, RomSizeError> {
        Ok(Self {
            cpu: Cpu::new(Ula::new(Memory::spectrum_48k(rom)?)),
        })
    }

    /// Press the reset button: CPU and ULA back to their power-on state, RAM untouched.
    pub fn reset(&mut self) {
        self.cpu.set_state(CpuState::default());
        self.cpu.bus_mut().reset();
    }

    /// Offer the interrupt if the ULA is raising one, then run one instruction.
    ///
    /// Returns the T-states the CPU charged. **That number is not the clock** — contention
    /// is added on the bus's side and is not included. It is returned because the CPU
    /// returns it, and it is useful for asserting an instruction's nominal length; use
    /// [`Spectrum::frame_t_state`] for time.
    ///
    /// The interrupt is *offered*, not forced: [`z80::Cpu::interrupt`] declines while
    /// `iff1` is clear or the `EI` window is open, and returns zero having changed nothing.
    /// The acceptance rule lives there and is deliberately not repeated here — including
    /// as a "once per frame" guard, which would be exactly that duplication.
    pub fn step(&mut self) -> u32 {
        if self.cpu.bus().interrupt_asserted() {
            let accepted = self.cpu.interrupt(FLOATING_BUS_BYTE);
            if accepted != 0 {
                return accepted;
            }
        }
        self.cpu.step()
    }

    /// Run until the frame counter advances.
    ///
    /// The loop watches the counter rather than a T-state budget, which is what makes an
    /// instruction that overruns the frame a non-event: it lands in the next frame and the
    /// overshoot is carried, exactly as it is on the hardware — including the case where
    /// the overshoot is long enough to miss the following interrupt.
    pub fn run_frame(&mut self) {
        let target = self.frames() + 1;
        while self.frames() < target {
            // Deliberately discarded: the frame is driven by the bus's tick count, never
            // by summing what `step` returns. See the crate documentation.
            let _ = self.step();
        }
    }

    /// Run `count` frames.
    pub fn run_frames(&mut self, count: u64) {
        for _ in 0..count {
            self.run_frame();
        }
    }

    /// Frames completed since power-on.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.cpu.bus().clock().frames()
    }

    /// T-states elapsed since the start of the current frame, contention included.
    #[must_use]
    pub fn frame_t_state(&self) -> u32 {
        self.cpu.bus().clock().frame_t_state()
    }

    /// Draw the current screen into `frame`.
    ///
    /// A snapshot of the screen as it stands now, not a record of what the ULA drew during
    /// the frame — see [`screen`] for what that costs.
    pub fn render(&self, frame: &mut Frame) {
        let ula = self.cpu.bus();
        screen::render(
            ula.memory(),
            ula.border(),
            screen::flash_phase(self.frames()),
            frame,
        );
    }

    /// The complete CPU state — what a `.z80` or `.sna` snapshot carries.
    #[must_use]
    pub fn cpu_state(&self) -> CpuState {
        self.cpu.state()
    }

    /// Replace the complete CPU state.
    pub fn set_cpu_state(&mut self, state: CpuState) {
        self.cpu.set_state(state);
    }

    /// What stopped the most recent operation short, if anything.
    ///
    /// Should always be `None` on a Spectrum: the only fault the core can raise is a mode 0
    /// interrupt supplying a byte that is not an `RST`, and this machine's bus floats to
    /// `0xFF`, which is `RST 38h`. A fault here is a finding, not a condition to handle.
    #[must_use]
    pub fn fault(&self) -> Option<StepError> {
        self.cpu.fault()
    }

    /// The ULA.
    #[must_use]
    pub fn ula(&self) -> &Ula {
        self.cpu.bus()
    }

    /// The ULA, mutably.
    pub fn ula_mut(&mut self) -> &mut Ula {
        self.cpu.bus_mut()
    }

    /// The address space.
    #[must_use]
    pub fn memory(&self) -> &Memory {
        self.cpu.bus().memory()
    }

    /// The address space, mutably.
    pub fn memory_mut(&mut self) -> &mut Memory {
        self.cpu.bus_mut().memory_mut()
    }

    /// The keyboard.
    #[must_use]
    pub fn keyboard(&self) -> &Keyboard {
        self.cpu.bus().keyboard()
    }

    /// The keyboard, mutably — how a frontend or a test presses a key.
    pub fn keyboard_mut(&mut self) -> &mut Keyboard {
        self.cpu.bus_mut().keyboard_mut()
    }

    /// The colour last written to the border.
    #[must_use]
    pub fn border(&self) -> Colour {
        self.cpu.bus().border()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::PAGE_SIZE;
    use crate::timing::{INTERRUPT_T_STATES, T_STATES_PER_FRAME};

    /// A ROM of `NOP`s: the CPU runs off the end of it into RAM and wraps, forever.
    fn nop_machine() -> Spectrum {
        Spectrum::new(&[0x00; PAGE_SIZE]).expect("a page-sized ROM")
    }

    #[test]
    fn a_rom_of_the_wrong_size_is_refused() {
        assert!(Spectrum::new(&[0x00; 100]).is_err());
    }

    #[test]
    fn a_fresh_machine_starts_at_the_top_of_frame_zero() {
        let machine = nop_machine();
        assert_eq!((machine.frames(), machine.frame_t_state()), (0, 0));
        assert_eq!(machine.cpu_state().pc, 0);
    }

    #[test]
    fn the_frame_loop_is_driven_by_the_bus_and_not_by_step_returns() {
        // MACHINE.md Decision 1, as a failing case rather than as prose. Running from the
        // ROM the two accounts agree, because nothing there is contended...
        let mut machine = nop_machine();
        let mut charged = 0_u32;
        while machine.frames() == 0 {
            charged += machine.step();
        }
        assert_eq!(machine.frames(), 1);
        assert_eq!(
            charged,
            T_STATES_PER_FRAME + machine.frame_t_state(),
            "uncontended, `step` returns and the bus clock must agree exactly"
        );

        // ...and running the same instructions from the contended bank they do not. A
        // machine that summed `step` returns would run this frame roughly 20% long, and
        // no test anywhere would fail.
        let mut machine = nop_machine();
        let mut state = machine.cpu_state();
        state.pc = 0x4000;
        machine.set_cpu_state(state);
        let mut charged = 0_u32;
        while machine.frames() == 0 {
            charged += machine.step();
        }
        assert!(
            charged < T_STATES_PER_FRAME,
            "contention adds time on the bus's side, so the CPU must charge less than a \
             frame ({charged} of {T_STATES_PER_FRAME})"
        );
    }

    #[test]
    fn one_frame_of_nops_takes_the_whole_frame_budget() {
        let mut machine = nop_machine();
        machine.run_frame();
        assert_eq!(machine.frames(), 1);
        assert!(
            machine.frame_t_state() < 4,
            "the overshoot should be less than one NOP, not {}",
            machine.frame_t_state()
        );
    }

    /// Enable interrupts in `mode`, leaving everything else as it stands.
    fn enable_interrupts(machine: &mut Spectrum, mode: z80::InterruptMode) {
        let mut state = machine.cpu_state();
        state.iff1 = true;
        state.iff2 = true;
        state.im = mode;
        state.sp = 0xFF00;
        machine.set_cpu_state(state);
    }

    #[test]
    fn an_interrupt_is_declined_while_the_cpu_has_them_disabled() {
        // Power-on state is `iff1` clear, so every offer this frame is refused: the
        // machine keeps executing, and nothing is pushed.
        let mut machine = nop_machine();
        machine.run_frame();
        assert!(!machine.cpu_state().iff1);
        assert_eq!(machine.cpu_state().sp, u16::MAX, "nothing was pushed");
        assert!(
            machine.cpu_state().pc > 0x1000,
            "the CPU should have run a frame of NOPs, not vectored"
        );
        assert_eq!(machine.fault(), None);
    }

    #[test]
    fn an_enabled_interrupt_is_accepted_at_the_top_of_the_frame() {
        // Run a frame with interrupts off, so the acceptance under test is the one at the
        // start of frame 1 rather than the one waiting at power-on.
        let mut machine = nop_machine();
        machine.run_frame();
        enable_interrupts(&mut machine, z80::InterruptMode::Mode1);
        assert_eq!(machine.frames(), 1);

        machine.step(); // the first step of the new frame sees `/INT` low

        assert_eq!(
            machine.cpu_state().pc,
            0x0038,
            "mode 1 vectors to 0x0038 and the machine must not reimplement the rule"
        );
        assert!(
            !machine.cpu_state().iff1,
            "acceptance clears both flip-flops"
        );
        assert!(!machine.cpu_state().iff2);
        assert_eq!(
            machine.cpu_state().sp,
            0xFEFE,
            "the return address was pushed"
        );
    }

    #[test]
    fn the_interrupt_is_a_window_and_an_offer_past_it_is_not_made() {
        let mut machine = nop_machine();
        while machine.frame_t_state() < INTERRUPT_T_STATES {
            machine.step();
        }
        enable_interrupts(&mut machine, z80::InterruptMode::Mode1);

        let before = machine.cpu_state().pc;
        machine.step();
        assert_ne!(machine.cpu_state().pc, 0x0038, "the line is no longer low");
        assert!(machine.cpu_state().pc > before);
        assert_eq!(machine.cpu_state().sp, 0xFF00, "nothing was pushed");
    }

    #[test]
    fn an_interrupt_is_offered_again_on_the_next_frame() {
        let mut machine = nop_machine();
        machine.run_frame();
        enable_interrupts(&mut machine, z80::InterruptMode::Mode1);
        machine.step();
        assert_eq!(machine.cpu_state().pc, 0x0038);

        // Acceptance cleared `iff1`; the handler's `EI` is what lets the next one in.
        machine.run_frame();
        enable_interrupts(&mut machine, z80::InterruptMode::Mode1);
        machine.step();
        assert_eq!(machine.cpu_state().pc, 0x0038);
        assert_eq!(machine.frames(), 2);
    }

    #[test]
    fn reset_returns_the_cpu_and_the_clock_but_keeps_ram() {
        let mut machine = nop_machine();
        machine.memory_mut().write(0x8000, 0xA5);
        machine.run_frames(2);
        machine.reset();
        assert_eq!((machine.frames(), machine.frame_t_state()), (0, 0));
        assert_eq!(machine.cpu_state().pc, 0);
        assert_eq!(machine.memory().read(0x8000), 0xA5);
    }

    #[test]
    fn a_spectrum_never_faults_because_its_bus_floats_to_rst_38h() {
        // Mode 0 is the only mode `Cpu::interrupt` can decline with a fault, and it does
        // so for any byte that is not an `RST`. A 48K's bus floats to 0xFF, which is
        // `RST 38h` — so the two modes land in the same place and the fault cannot happen.
        let mut machine = nop_machine();
        machine.run_frame();
        enable_interrupts(&mut machine, z80::InterruptMode::Mode0);
        machine.step();
        assert_eq!(machine.fault(), None);
        assert_eq!(machine.cpu_state().pc, 0x0038, "0xFF decodes as RST 38h");
    }

    #[test]
    fn rendering_takes_the_border_and_the_flash_phase_from_the_machine() {
        // The wiring, which nothing in `screen`'s own tests can see: the border comes from
        // the ULA's latch and the FLASH phase from this machine's frame count. The border
        // is set the only way anything ever sets it — an `OUT` the guest performs.
        use z80::Bus;

        let mut machine = nop_machine();
        machine.ula_mut().out_port(0x00FE, 2);
        for offset in 0..screen::ATTRIBUTE_FILE_LEN {
            // FLASH, INK 0, PAPER 7 — every cell swaps on the second half of the cycle.
            machine
                .memory_mut()
                .write(screen::ATTRIBUTE_FILE + offset as u16, 0x80 | 0x38);
        }

        let mut frame = Frame::new();
        machine.render(&mut frame);
        assert_eq!(
            frame.pixel(0, 0),
            Some(Colour::new(2)),
            "border came through"
        );
        assert_eq!(
            frame.pixel(screen::BORDER, screen::BORDER),
            Some(Colour::new(7)),
            "frame 0 is the first half of the FLASH cycle: paper is paper"
        );

        machine.run_frames(screen::FLASH_FRAMES);
        assert_eq!(machine.frames(), screen::FLASH_FRAMES);
        machine.render(&mut frame);
        assert_eq!(
            frame.pixel(screen::BORDER, screen::BORDER),
            Some(Colour::new(0)),
            "frame 16 is the second half: ink and paper have swapped"
        );
    }
}
