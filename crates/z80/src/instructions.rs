//! The un-prefixed instruction set: the decode table and one handler per instruction.
//!
//! # The table is a table
//!
//! [`Cpu::execute`] is a single `match` over all 256 opcode values, with no catch-all arm.
//! That is deliberate: exhaustiveness is then a compile-time property, so an opcode can
//! never be quietly forgotten and silently behave as a `NOP` — the failure mode that makes
//! an emulator bug take a week to find. The compiler lowers the match to a jump table, so
//! the explicitness costs nothing at runtime, and unlike a table of function pointers it
//! leaves every handler inlinable.
//!
//! Every arm is one call. The regular blocks of the opcode map are matched as ranges or
//! or-patterns and their operand fields decoded by [`crate::decode`], because the
//! regularity is real — `LD r,r'` genuinely is one instruction with two operand fields,
//! and writing its sixty-three encodings out separately would be sixty-three chances to
//! mistype a register rather than one.
//!
//! # The `base` parameter
//!
//! Every handler that touches `HL` takes a [`PairBase`] rather than assuming `pair::HL`.
//! Un-prefixed instructions pass `pair::HL`; M2's `DD` and `FD` prefixes will pass
//! `pair::IX` and `pair::IY` to this same table, so the entire index-register instruction
//! set costs one extra argument rather than a second copy of the decoder.
//!
//! # Timing
//!
//! T-states are charged where they are spent: each memory access charges its own machine
//! cycle, and cycles in which the Z80 computes rather than transfers are charged
//! explicitly by [`Cpu::internal_cycles`]. Each such call carries the published
//! machine-cycle breakdown in a comment, because those extra cycles are otherwise the
//! least obvious part of the code and the easiest to get wrong.

use crate::bus::Bus;
use crate::decode::{AluOp, CbOp, Condition, Index, Operand, ShiftOp, Target, ed_pair};
use crate::flags;
use crate::registers::{PairBase, RegIndex, index, pair};
use crate::{Cpu, InterruptMode, PREFIX_CB, PREFIX_DD, PREFIX_ED, PREFIX_FD};

/// T-states the Z80 spends adding a displacement to an index register.
const INDEX_COMPUTATION: u8 = 5;

/// The same computation when an operand byte fetched after the displacement has already
/// spent three of its T-states.
const INDEX_COMPUTATION_AFTER_FETCH: u8 = 2;

/// T-states the 16-bit `ADC`/`SBC` pair arithmetic spends after its opcode fetch.
const PAIR_ARITHMETIC: u8 = 7;

/// T-states `RRD`/`RLD` spend shuffling nibbles between memory and the accumulator.
const DIGIT_ROTATE: u8 = 4;

/// T-states a repeating block instruction spends before re-running itself.
const BLOCK_REPEAT: u8 = 5;

/// What a block instruction leaves behind for the repeat machinery.
///
/// `last_address` is the address that was last on the bus, which is where the repeat's
/// internal cycles are driven — and it differs per family: `DE` for the transfers (the
/// write), `HL` for the compares (the read) and the inputs (the write), but the **port**
/// for the outputs. Corpus vectors `edb0`, `edb1`, `edb2` and `edb3` each show a different
/// address for the same five cycles, which is why this travels with the outcome rather than
/// being recomputed from registers that have since moved.
struct BlockOutcome {
    /// Whether the loop condition still holds.
    repeat: bool,
    /// The address last driven on the bus.
    last_address: u16,
}

/// Whether a block instruction walks its pointers up or down.
///
/// The increment/decrement bit is 3 of the opcode: `LDI` is `A0` and `LDD` is `A8`.
fn block_step(opcode: u8) -> i8 {
    if opcode & 0x08 == 0 { 1 } else { -1 }
}

/// Whether this encoding is one of the repeating forms.
///
/// Bit 4 separates `LDI` (`A0`) from `LDIR` (`B0`), uniformly across all four families.
const fn repeats(opcode: u8) -> bool {
    opcode & 0x10 != 0
}

/// `MEMPTR` after a store whose data comes from the accumulator.
///
/// Boo-boo and Kladov give the same two-part formula three times — for `LD (addr),A`,
/// `LD (rp),A` and `OUT (port),A` — and it is not `address + 1`:
///
/// ```text
/// MEMPTR_low = (address + 1) & #FF,  MEMPTR_hi = A
/// ```
///
/// **The carry does not propagate.** `LD (0x40FF),A` with `A = 0x12` leaves `MEMPTR` at
/// `0x1200`, not `0x1300` and not `0x4100`: the low half is incremented in isolation and the
/// high half is overwritten by the accumulator, which is the whole quirk. Written as one
/// function because three handlers must not each re-derive an eight-bit wrap.
///
/// The three forms that *read* through the same addressing modes — `LD A,(addr)`,
/// `LD A,(rp)`, `IN A,(port)` — take the ordinary `address + 1` instead, carry and all. The
/// asymmetry is the measurement's, not a transcription slip.
fn accumulator_store_memptr(address: u16, a: u8) -> u16 {
    let [_, low] = address.wrapping_add(1).to_be_bytes();
    u16::from_be_bytes([a, low])
}

impl<B: Bus> Cpu<B> {
    /// Execute one already-fetched opcode.
    ///
    /// `base` is the pair standing in for `HL`, which is always [`pair::HL`] for the
    /// un-prefixed set.
    ///
    /// The prefix bytes never reach here: [`Cpu::dispatch`] consumes them first.
    pub(crate) fn execute(&mut self, opcode: u8, index: Index) {
        match opcode {
            // ---- 0x00–0x3F ----------------------------------------------------------
            0x00 => {} // NOP — the opcode fetch is the whole instruction
            0x08 => self.regs.exchange_af(), // EX AF,AF'
            0x10 => self.decrement_and_jump(), // DJNZ e
            0x18 => self.jump_relative_unconditional(), // JR e
            0x20 | 0x28 | 0x30 | 0x38 => {
                self.jump_relative_conditional(Condition::from_relative_jump_opcode(opcode));
            } // JR cc,e

            0x01 => self.load_pair_immediate(pair::BC), // LD BC,nn
            0x11 => self.load_pair_immediate(pair::DE), // LD DE,nn
            0x21 => self.load_pair_immediate(index.base()), // LD HL,nn
            0x31 => self.load_pair_immediate(pair::SP), // LD SP,nn

            // `ADD HL,ss` takes the base twice: `DD 29` is `ADD IX,IX`, so both the
            // destination and the operand follow the prefix.
            0x09 => self.add_pair(index.base(), pair::BC), // ADD HL,BC
            0x19 => self.add_pair(index.base(), pair::DE), // ADD HL,DE
            0x29 => self.add_pair(index.base(), index.base()), // ADD HL,HL
            0x39 => self.add_pair(index.base(), pair::SP), // ADD HL,SP

            0x02 => self.store_a_indirect(pair::BC), // LD (BC),A
            0x12 => self.store_a_indirect(pair::DE), // LD (DE),A
            0x0A => self.load_a_indirect(pair::BC),  // LD A,(BC)
            0x1A => self.load_a_indirect(pair::DE),  // LD A,(DE)

            0x22 => self.store_pair_absolute(index.base()), // LD (nn),HL
            0x2A => self.load_pair_absolute(index.base()),  // LD HL,(nn)
            0x32 => self.store_a_absolute(),                // LD (nn),A
            0x3A => self.load_a_absolute(),                 // LD A,(nn)

            0x03 => self.increment_pair(pair::BC), // INC BC
            0x13 => self.increment_pair(pair::DE), // INC DE
            0x23 => self.increment_pair(index.base()), // INC HL
            0x33 => self.increment_pair(pair::SP), // INC SP
            0x0B => self.decrement_pair(pair::BC), // DEC BC
            0x1B => self.decrement_pair(pair::DE), // DEC DE
            0x2B => self.decrement_pair(index.base()), // DEC HL
            0x3B => self.decrement_pair(pair::SP), // DEC SP

            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                self.increment_operand(Operand::destination(opcode), index);
            } // INC r / INC (HL)
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                self.decrement_operand(Operand::destination(opcode), index);
            } // DEC r / DEC (HL)
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                self.load_operand_immediate(Operand::destination(opcode), index);
            } // LD r,n / LD (HL),n

            0x07 => self.rotate_a(flags::rlca),   // RLCA
            0x0F => self.rotate_a(flags::rrca),   // RRCA
            0x17 => self.rotate_a(flags::rla),    // RLA
            0x1F => self.rotate_a(flags::rra),    // RRA
            0x27 => self.decimal_adjust_a(),      // DAA
            0x2F => self.complement_a(),          // CPL
            0x37 => self.set_carry_flag(),        // SCF
            0x3F => self.complement_carry_flag(), // CCF

            // ---- 0x40–0x7F: LD r,r' -------------------------------------------------
            // HALT occupies the encoding that would otherwise mean `LD (HL),(HL)`, which
            // is why it sits in the middle of the load block rather than with the other
            // control instructions.
            0x76 => self.halt(),
            0x40..=0x7F => {
                self.load_operand_operand(
                    Operand::destination(opcode),
                    Operand::source(opcode),
                    index,
                );
            }

            // ---- 0x80–0xBF: ALU A,r -------------------------------------------------
            0x80..=0xBF => {
                self.alu_with_operand(AluOp::from_opcode(opcode), Operand::source(opcode), index);
            }

            // ---- 0xC0–0xFF ----------------------------------------------------------
            0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 => {
                self.return_conditional(Condition::from_opcode(opcode));
            } // RET cc
            0xC2 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA => {
                self.jump_conditional(Condition::from_opcode(opcode));
            } // JP cc,nn
            0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => {
                self.call_conditional(Condition::from_opcode(opcode));
            } // CALL cc,nn
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
                self.alu_with_immediate(AluOp::from_opcode(opcode));
            } // ALU A,n
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => self.restart(opcode), // RST p

            0xC1 => self.pop_into(pair::BC),      // POP BC
            0xD1 => self.pop_into(pair::DE),      // POP DE
            0xE1 => self.pop_into(index.base()),  // POP HL
            0xF1 => self.pop_into(pair::AF),      // POP AF
            0xC5 => self.push_pair(pair::BC),     // PUSH BC
            0xD5 => self.push_pair(pair::DE),     // PUSH DE
            0xE5 => self.push_pair(index.base()), // PUSH HL
            0xF5 => self.push_pair(pair::AF),     // PUSH AF

            0xC3 => self.jump_unconditional(),       // JP nn
            0xC9 => self.return_unconditional(),     // RET
            0xCD => self.call_unconditional(),       // CALL nn
            0xD3 => self.output_immediate(),         // OUT (n),A
            0xDB => self.input_immediate(),          // IN A,(n)
            0xD9 => self.regs.exchange_shadow_set(), // EXX
            0xE3 => self.exchange_stack_pair(index.base()), // EX (SP),HL
            0xE9 => self.jump_to_pair(index.base()), // JP (HL)
            0xEB => self.regs.exchange_de_hl(),      // EX DE,HL — never an index register
            0xF3 => self.disable_interrupts(),       // DI
            0xF9 => self.load_sp_from_pair(index.base()), // LD SP,HL
            0xFB => self.enable_interrupts(),        // EI

            // Unreachable by construction: `Cpu::dispatch` consumes all four prefix bytes
            // before calling here. The arm exists because the match is exhaustive over all
            // 256 encodings — which is what stops an opcode being silently forgotten — and
            // there is no error left to report now that every prefix decodes.
            PREFIX_CB | PREFIX_DD | PREFIX_ED | PREFIX_FD => {}
        }
    }

    /// Execute an `ED`-prefixed instruction.
    ///
    /// The `ED` page is sparse: about a quarter of it is assigned and the rest decodes as a
    /// two-byte `NOP`. Unlike `DD`/`FD`, `ED` ignores any index prefix that preceded it —
    /// `DD ED 44` is just `NEG`.
    pub(crate) fn execute_ed(&mut self) {
        let opcode = self.fetch_opcode();
        match opcode {
            0x40 | 0x48 | 0x50 | 0x58 | 0x60 | 0x68 | 0x70 | 0x78 => self.input_from_c(opcode),
            0x41 | 0x49 | 0x51 | 0x59 | 0x61 | 0x69 | 0x71 | 0x79 => self.output_to_c(opcode),
            0x42 | 0x52 | 0x62 | 0x72 => self.subtract_pair_with_carry(ed_pair(opcode)),
            0x4A | 0x5A | 0x6A | 0x7A => self.add_pair_with_carry(ed_pair(opcode)),
            0x43 | 0x53 | 0x63 | 0x73 => self.store_pair_absolute(ed_pair(opcode)),
            0x4B | 0x5B | 0x6B | 0x7B => self.load_pair_absolute(ed_pair(opcode)),
            0x44 | 0x4C | 0x54 | 0x5C | 0x64 | 0x6C | 0x74 | 0x7C => self.negate_a(),
            0x45 | 0x4D | 0x55 | 0x5D | 0x65 | 0x6D | 0x75 | 0x7D => self.return_from_interrupt(),
            0x46 | 0x4E | 0x66 | 0x6E => self.select_interrupt_mode(InterruptMode::Mode0),
            0x56 | 0x76 => self.select_interrupt_mode(InterruptMode::Mode1),
            0x5E | 0x7E => self.select_interrupt_mode(InterruptMode::Mode2),
            0x47 => self.load_special_from_a(index::I),
            0x4F => self.load_special_from_a(index::R),
            0x57 => self.load_a_from_special(index::I),
            0x5F => self.load_a_from_special(index::R),
            0x67 => self.rotate_digit_right(),
            0x6F => self.rotate_digit_left(),

            0xA0 | 0xA8 | 0xB0 | 0xB8 => {
                let outcome = self.block_transfer(opcode);
                self.repeat_block(opcode, outcome);
            }
            0xA1 | 0xA9 | 0xB1 | 0xB9 => {
                let outcome = self.block_compare(opcode);
                self.repeat_block(opcode, outcome);
            }
            0xA2 | 0xAA | 0xB2 | 0xBA => {
                let outcome = self.block_input(opcode);
                self.repeat_block(opcode, outcome);
            }
            0xA3 | 0xAB | 0xB3 | 0xBB => {
                let outcome = self.block_output(opcode);
                self.repeat_block(opcode, outcome);
            }

            // Every other `ED` encoding is unassigned and behaves as a two-byte `NOP`.
            // That is the hardware's own rule rather than a fallback: the Z80 decodes these
            // and deliberately does nothing, in eight T-states.
            _unassigned => {}
        }
    }

    /// `IN r,(C)` — read the port named by the whole of `BC`.
    ///
    /// Encoding `110` names no register: the byte is read, the flags are set from it, and
    /// the value is discarded. That form is written `IN (C)` or `IN F,(C)`.
    ///
    /// `MEMPTR = BC + 1`. Boo-boo and Kladov write the rule as `IN A,(C)`, naming the one
    /// encoding whose destination is the accumulator, but it is stated in terms of the port
    /// and the port is the whole of `BC` for all eight — the destination register is chosen
    /// after the cycle and cannot reach back into it. The exerciser agrees: `IN R,(C)` and
    /// `IN (C)` are separate groups from `IN A,(C)` and all three move together.
    fn input_from_c(&mut self, opcode: u8) {
        let port = self.regs.pair(pair::BC);
        let value = self.read_port(port);
        self.set_memptr(port.wrapping_add(1));
        if let Some(register) = Operand::destination(opcode).register_index(pair::HL) {
            self.regs.set(register, value);
        }
        let flags = flags::prefixed::reported_byte(value, self.regs.f());
        self.write_flags(flags);
    }

    /// `OUT (C),r`. Encoding `110` names no register and outputs zero.
    ///
    /// `MEMPTR = BC + 1`, for the reason given at [`Cpu::input_from_c`] — the document names
    /// `OUT (C),A` and the rule belongs to the port. Note that this is **not** the
    /// accumulator-store quirk: `OUT (C),r` addresses its port with `BC`, so there is no `A`
    /// in the address for `MEMPTR`'s high byte to inherit. `OUT (n),A`, whose port high byte
    /// *is* `A`, is the one that takes [`accumulator_store_memptr`].
    fn output_to_c(&mut self, opcode: u8) {
        let value = match Operand::destination(opcode).register_index(pair::HL) {
            Some(register) => self.regs.get(register),
            None => 0,
        };
        let port = self.regs.pair(pair::BC);
        self.write_port(port, value);
        self.set_memptr(port.wrapping_add(1));
    }

    /// `ADC HL,ss`.
    ///
    /// `MEMPTR = rp1_before_operation + 1`, `rp1` being the destination — always `HL` here,
    /// because `ED` ignores an index prefix. See [`Cpu::add_pair`], which takes the same rule
    /// with a destination the prefix *can* move.
    fn add_pair_with_carry(&mut self, operand: PairBase) {
        let addend = self.regs.pair(operand);
        let carry = self.carry_flag();
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, PAIR_ARITHMETIC);
        let augend = self.regs.pair(pair::HL);
        self.set_memptr(augend.wrapping_add(1));
        let (result, flags) = flags::prefixed::adc16(augend, addend, carry);
        self.regs.set_pair(pair::HL, result);
        self.write_flags(flags);
    }

    /// `SBC HL,ss`.
    ///
    /// `MEMPTR = rp1_before_operation + 1` — the document gives `ADD`, `ADC` and `SBC` one
    /// rule, and subtraction is no exception to it: the latch takes the *minuend* plus one,
    /// not the difference.
    fn subtract_pair_with_carry(&mut self, operand: PairBase) {
        let subtrahend = self.regs.pair(operand);
        let carry = self.carry_flag();
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, PAIR_ARITHMETIC);
        let minuend = self.regs.pair(pair::HL);
        self.set_memptr(minuend.wrapping_add(1));
        let (result, flags) = flags::prefixed::sbc16(minuend, subtrahend, carry);
        self.regs.set_pair(pair::HL, result);
        self.write_flags(flags);
    }

    /// `NEG` — subtract the accumulator from zero.
    fn negate_a(&mut self) {
        let (result, flags) = flags::prefixed::neg(self.regs.a());
        self.regs.set_a(result);
        self.write_flags(flags);
    }

    /// `RETN` and `RETI`.
    ///
    /// Both restore `IFF1` from `IFF2` — the copy a non-maskable interrupt made on its way
    /// in — which is how interrupts resume after an NMI handler. `RETI` differs only in
    /// signalling the daisy chain, which this core has no bus for.
    ///
    /// `MEMPTR = addr`, as for `RET` — the document lists `RETI` beside it and the exerciser
    /// grades `RETN`, `RETI` and their interaction as three separate groups.
    fn return_from_interrupt(&mut self) {
        self.interrupts.iff1 = self.interrupts.iff2;
        let target = self.pop_word();
        self.regs.set_pc(target);
        self.set_memptr(target);
    }

    /// `IM 0`, `IM 1`, `IM 2`.
    fn select_interrupt_mode(&mut self, mode: InterruptMode) {
        self.interrupts.mode = mode;
    }

    /// `LD I,A` and `LD R,A`. Neither affects the flags.
    fn load_special_from_a(&mut self, register: RegIndex) {
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 1);
        let value = self.regs.a();
        self.regs.set(register, value);
    }

    /// `LD A,I` and `LD A,R` — the only way software can read `IFF2`.
    fn load_a_from_special(&mut self, register: RegIndex) {
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 1);
        let value = self.regs.get(register);
        self.regs.set_a(value);
        let flags = flags::prefixed::load_a_from_interrupt_register(
            value,
            self.interrupts.iff2,
            self.regs.f(),
        );
        self.write_flags(flags);
    }

    /// `RRD` — rotate one BCD digit right through `A` and `(HL)`.
    fn rotate_digit_right(&mut self) {
        self.rotate_digit(|a, memory| {
            let stored = (a << 4) | (memory >> 4);
            let accumulator = (a & 0xF0) | (memory & 0x0F);
            (accumulator, stored)
        });
    }

    /// `RLD` — rotate one BCD digit left through `A` and `(HL)`.
    fn rotate_digit_left(&mut self) {
        self.rotate_digit(|a, memory| {
            let stored = (memory << 4) | (a & 0x0F);
            let accumulator = (a & 0xF0) | (memory >> 4);
            (accumulator, stored)
        });
    }

    /// The shared body of `RRD` and `RLD`: read `(HL)`, shuffle nibbles between it and the
    /// accumulator, write it back. Four internal T-states pay for the shuffle.
    ///
    /// `MEMPTR = HL + 1` — one rule for both, which is why it sits here rather than being
    /// written twice above.
    fn rotate_digit(&mut self, shuffle: fn(u8, u8) -> (u8, u8)) {
        let address = self.regs.pair(pair::HL);
        let memory = self.read_byte(address);
        self.internal_cycles(address, DIGIT_ROTATE);
        self.set_memptr(address.wrapping_add(1));
        let (accumulator, stored) = shuffle(self.regs.a(), memory);
        self.write_byte(address, stored);
        self.regs.set_a(accumulator);
        let flags = flags::prefixed::reported_byte(accumulator, self.regs.f());
        self.write_flags(flags);
    }

    /// `LDI` and `LDD` — copy one byte from `(HL)` to `(DE)`.
    ///
    /// **Neither touches `MEMPTR`.** Boo-boo and Kladov list `LDIR`/`LDDR` and not `LDI`/`LDD`,
    /// and the two statements are one statement: the repeating forms' rule is *"when BC == 1:
    /// MEMPTR is not changed"* — `BC == 1` being the last iteration — so a form that never
    /// repeats never changes it either. The write lives in [`Cpu::repeat_block`], where the
    /// rewind is, and this handler having no `set_memptr` call is the rule rather than an
    /// omission.
    fn block_transfer(&mut self, opcode: u8) -> BlockOutcome {
        let step = block_step(opcode);
        let source = self.regs.pair(pair::HL);
        let value = self.read_byte(source);
        let destination = self.regs.pair(pair::DE);
        self.write_byte(destination, value);
        self.internal_cycles(destination, 2);

        self.regs
            .set_pair(pair::HL, source.wrapping_add_signed(i16::from(step)));
        self.regs
            .set_pair(pair::DE, destination.wrapping_add_signed(i16::from(step)));
        let remaining = self.decrement_byte_counter();
        let flags =
            flags::prefixed::block_transfer(self.regs.a(), value, remaining != 0, self.regs.f());
        self.write_flags(flags);

        BlockOutcome {
            repeat: remaining != 0,
            last_address: destination,
        }
    }

    /// `CPI` and `CPD` — compare `A` against `(HL)` without storing the difference.
    ///
    /// The one instruction pair that reads `MEMPTR` in order to write it: *"CPI — MEMPTR =
    /// MEMPTR + 1"*, *"CPD — MEMPTR = MEMPTR - 1"*. That is what makes the whole register
    /// measurable, and therefore what every other rule in this file was measured with — a
    /// `CPD` loop walks the latch down one at a time while `BIT n,(HL)` reports bits 11 and
    /// 13, and the borrow says where the other fourteen were.
    ///
    /// It applies to `CPIR`/`CPDR` too, on the iteration that stops: *"when BC=1 or A=(HL):
    /// exactly as CPI"*. The repeating iterations take the instruction address instead, which
    /// [`Cpu::repeat_block`] writes over the top of this one — the order matters not at all,
    /// since only the value left behind is observable.
    fn block_compare(&mut self, opcode: u8) -> BlockOutcome {
        let step = block_step(opcode);
        let source = self.regs.pair(pair::HL);
        let value = self.read_byte(source);
        self.internal_cycles(source, 5);

        self.regs
            .set_pair(pair::HL, source.wrapping_add_signed(i16::from(step)));
        let remaining = self.decrement_byte_counter();
        let flags =
            flags::prefixed::block_compare(self.regs.a(), value, remaining != 0, self.regs.f());
        self.write_flags(flags);
        self.set_memptr(self.wz.wrapping_add_signed(i16::from(step)));

        BlockOutcome {
            // The searching forms stop on *either* term — the counter running out, or a
            // match. Corpus `edb9` exits on the counter, `edb1` on the match.
            repeat: remaining != 0 && (flags & crate::flags::ZERO) == 0,
            last_address: source,
        }
    }

    /// `INI` and `IND` — read a port into `(HL)`.
    ///
    /// The port is addressed with `B` still at its old value; the decrement happens after.
    /// `MEMPTR` follows that same port: *"INI — MEMPTR = BC_before_decrementing_B + 1"*,
    /// *"IND — ... - 1"*. The 2006 document adds that `INIR`/`INDR` are *"exactly as INI/IND
    /// on each execution"*, and that part of it does not hold — an iteration that repeats
    /// overwrites the latch again from the instruction address. See [`Cpu::repeat_block`].
    fn block_input(&mut self, opcode: u8) -> BlockOutcome {
        let step = block_step(opcode);
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 1);

        let port = self.regs.pair(pair::BC);
        let value = self.read_port(port);
        let destination = self.regs.pair(pair::HL);
        self.write_byte(destination, value);
        self.set_memptr(port.wrapping_add_signed(i16::from(step)));

        self.regs
            .set_pair(pair::HL, destination.wrapping_add_signed(i16::from(step)));
        let counter = self.decrement_b();
        // `INI` derives its carry from `C + 1`, `IND` from `C - 1`.
        let index = self.regs.get(index::C).wrapping_add_signed(step);
        let flags = flags::prefixed::block_io(counter, value, index);
        self.write_flags(flags);

        BlockOutcome {
            repeat: counter != 0,
            last_address: destination,
        }
    }

    /// `OUTI` and `OUTD` — write `(HL)` to a port.
    ///
    /// Here `B` is decremented *before* the transfer, so the port carries the new value — and
    /// `MEMPTR` inherits that difference for free, because the rule is again the port either
    /// side: *"OUTI — MEMPTR = BC_after_decrementing_B + 1"*, *"OUTD — ... - 1"*. Against
    /// `INI`'s *before*, this is the one place the two I/O families' `MEMPTR` rules diverge,
    /// and neither handler states it twice: each writes the port it actually drove.
    fn block_output(&mut self, opcode: u8) -> BlockOutcome {
        let step = block_step(opcode);
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 1);

        let source = self.regs.pair(pair::HL);
        let value = self.read_byte(source);
        let counter = self.decrement_b();
        let port = self.regs.pair(pair::BC);
        self.write_port(port, value);
        self.set_memptr(port.wrapping_add_signed(i16::from(step)));

        self.regs
            .set_pair(pair::HL, source.wrapping_add_signed(i16::from(step)));
        // The output forms derive their carry from `L` after the pointer has moved.
        let index = self.regs.get(pair::HL.low());
        let flags = flags::prefixed::block_io(counter, value, index);
        self.write_flags(flags);

        BlockOutcome {
            repeat: counter != 0,
            // The output forms leave the *port* on the bus, not a memory address: corpus
            // `edb3` drives its repeat cycles at `02e0`, which is `BC` after the decrement.
            last_address: port,
        }
    }

    /// Re-run a block instruction by stepping `PC` back onto its own two opcode bytes.
    ///
    /// The Z80 has no internal loop: while the condition holds it rewinds `PC` by two and
    /// lets the next M1 cycle fetch `ED` and the opcode again. That is why corpus vector
    /// `edb0` shows `MR@0000 MR@0001` sixteen times, why `R` advances by two per iteration,
    /// and why one [`Cpu::step`] is one pass rather than the whole loop — the instruction
    /// stays interruptible between iterations, which is what lets a 64 KB `LDIR` coexist
    /// with a 50 Hz frame interrupt.
    ///
    /// Five extra T-states pay for the rewind, driven on whichever address was last on the
    /// bus.
    ///
    /// # The rewind is also a `MEMPTR` rule
    ///
    /// For the transfers and the compares, an iteration that repeats leaves `MEMPTR` at
    /// *"PC + 1, where PC = instruction address"* — and the instruction address is precisely
    /// what the rewind has just computed, so the two facts share the one subtraction rather
    /// than deriving it twice. Boo-boo and Kladov state it for `LDIR`/`LDDR` outright and for
    /// `CPIR`/`CPDR` as the complement of their stopping case.
    ///
    /// `PC` here is the `ED` byte, which is what `PC - 2` lands on: an index prefix before it
    /// is not rewound onto, and the hardware re-fetches from the `ED`.
    ///
    /// # One rule, four families — and half of it is newer than the other half
    ///
    /// It applies to **all eight** repeating forms, and that took two separate discoveries
    /// twenty years apart. Boo-boo and Kladov's 2006 measurements give it for `LDIR`/`LDDR`
    /// and, as the complement of their stopping case, for `CPIR`/`CPDR` — but explicitly
    /// exempt the I/O forms, giving `INIR` as *"exactly as INI on each execution"*. That
    /// exemption is **wrong**, and the correction is David Banks's, from tracing real parts
    /// (one Zilog NMOS and two NEC NMOS): the repeat's extra five T-states load the latch in
    /// the I/O families exactly as in the other two. MAME took the same rule for `inir`,
    /// `indr`, `otir` and `otdr`, and Patrik Rak's exerciser carries CRCs that expect it.
    ///
    /// So this handler ends with no per-family condition. It briefly had one — the I/O half
    /// implemented from the 2006 document, which is how `102 INIR->NOP'` and `103 INDR->NOP'`
    /// came to be the last two failing groups after every other rule had landed. **The
    /// uniform rule is the one that explains more than it was fitted to**, which is this
    /// project's own test for whether a rule is real: two of those four instructions are the
    /// only ones any available oracle can see, and the rule covers the other two anyway.
    ///
    /// `OTIR`/`OTDR` are therefore implemented on the hardware evidence alone and are graded
    /// by nothing here: the exerciser's self-overwriting `->NOP'` trick has no output
    /// counterpart, so it has no group for them.
    fn repeat_block(&mut self, opcode: u8, outcome: BlockOutcome) {
        if !repeats(opcode) || !outcome.repeat {
            return;
        }
        self.internal_cycles(outcome.last_address, BLOCK_REPEAT);
        let rewound = self.regs.pc().wrapping_sub(2);
        self.regs.set_pc(rewound);
        self.set_memptr(rewound.wrapping_add(1));
    }

    /// Decrement `BC` and report what is left — the block instructions' loop counter.
    fn decrement_byte_counter(&mut self) -> u16 {
        let remaining = self.regs.pair(pair::BC).wrapping_sub(1);
        self.regs.set_pair(pair::BC, remaining);
        remaining
    }

    /// Decrement `B` and report what is left — the I/O block instructions' counter.
    fn decrement_b(&mut self) -> u8 {
        let counter = self.regs.get(index::B).wrapping_sub(1);
        self.regs.set(index::B, counter);
        counter
    }

    /// Execute a `CB`-prefixed instruction: the rotates, shifts and bit operations.
    ///
    /// # `DDCB` puts the displacement before the opcode
    ///
    /// A plain `CB` instruction fetches its opcode with an ordinary M1 cycle. A `DD CB`
    /// one does not: the byte order is `DD` `CB` `d` `op`, so the operand is resolved
    /// *before* the operation is known, and both `d` and `op` arrive as three-T-state
    /// **memory reads** rather than M1 cycles — `R` advances twice for a four-byte
    /// instruction, not four times. A decoder that assumed the opcode follows the prefix
    /// would read the two bytes in the wrong order and refresh twice too often.
    ///
    /// # The undocumented register copy
    ///
    /// In the `DDCB` set the low three bits of the opcode are not wasted. Only `110` is the
    /// documented memory-only form; the other seven encodings write the result to `(IX+d)`
    /// **and** copy it into `B`, `C`, `D`, `E`, `H`, `L` or `A`. Corpus vector `ddcb00`
    /// shows both destinations taking `0x43`. That register is never the substituted `IXh`
    /// or `IXl` — the prefix has already been spent on the address.
    pub(crate) fn execute_cb(&mut self, index: Index) {
        let (opcode, target) = if index.is_displaced() {
            let displacement = self.fetch_signed_byte();
            let opcode = self.fetch_byte();
            let effective = self.indexed_address(index, displacement);
            // The opcode fetch has already spent three of the five computation T-states.
            let last_fetched = self.regs.pc().wrapping_sub(1);
            self.internal_cycles(last_fetched, INDEX_COMPUTATION_AFTER_FETCH);
            (opcode, Target::Memory(effective))
        } else {
            let opcode = self.fetch_opcode();
            (opcode, self.resolve_only(Operand::source(opcode), index))
        };

        let value = self.read_target(target);
        match CbOp::from_opcode(opcode) {
            CbOp::Bit(bit_index) => self.test_bit(value, bit_index, target),
            CbOp::Shift(shift) => {
                let (result, flags) = self.apply_shift(shift, value);
                self.write_flags(flags);
                self.store_cb_result(target, result, index, opcode);
            }
            // `RES` and `SET` define no flags at all.
            CbOp::Reset(bit_index) => {
                self.store_cb_result(target, value & !(1 << bit_index), index, opcode);
            }
            CbOp::Set(bit_index) => {
                self.store_cb_result(target, value | (1 << bit_index), index, opcode);
            }
        }
    }

    /// One rotate or shift from the `CB` set.
    fn apply_shift(&self, shift: ShiftOp, value: u8) -> (u8, u8) {
        let carry_in = self.carry_flag();
        match shift {
            ShiftOp::Rlc => flags::prefixed::rlc(value),
            ShiftOp::Rrc => flags::prefixed::rrc(value),
            ShiftOp::Rl => flags::prefixed::rl(value, carry_in),
            ShiftOp::Rr => flags::prefixed::rr(value, carry_in),
            ShiftOp::Sla => flags::prefixed::sla(value),
            ShiftOp::Sra => flags::prefixed::sra(value),
            ShiftOp::Sll => flags::prefixed::sll(value),
            ShiftOp::Srl => flags::prefixed::srl(value),
        }
    }

    /// `BIT n,s` — the one `CB` group that produces no result to store.
    fn test_bit(&mut self, value: u8, bit_index: u8, target: Target) {
        let undocumented_source = match target {
            Target::Register(_) => value,
            // Both memory forms take bits 3 and 5 from the high byte of `MEMPTR`, which is
            // the single rule behind what look like two. `BIT n,(IX+d)` has just set
            // `MEMPTR` to its effective address, so it reads that back; `BIT n,(HL)` never
            // touches it, so it reads whatever the previous instruction left — zero in the
            // corpus, which is exactly what vectors `cb46`..`cb7e` expect.
            Target::Memory(_) => self.wz.to_be_bytes()[0],
        };
        let flags = flags::prefixed::bit(value, bit_index, self.regs.f(), undocumented_source);
        self.write_flags(flags);
        // The memory form still holds the value for one internal T-state.
        self.tick_read_modify_delay(target);
    }

    /// Store a `CB` result: the read-modify delay, the write-back, and — for the `DDCB`
    /// forms — the undocumented copy into a register.
    fn store_cb_result(&mut self, target: Target, result: u8, index: Index, opcode: u8) {
        self.tick_read_modify_delay(target);
        self.write_target(target, result);
        if index.is_displaced() {
            // The copy names a real register: `pair::HL` rather than the index, because the
            // prefix does not reach this half. Encoding `110` names memory and copies
            // nowhere, which is the documented form.
            if let Some(register) = Operand::source(opcode).register_index(pair::HL) {
                self.regs.set(register, result);
            }
        }
    }

    // -----------------------------------------------------------------------------
    // Operand access
    // -----------------------------------------------------------------------------

    /// Resolve an operand to the place its value lives, computing an effective address at
    /// most once.
    ///
    /// This is the single point where a memory operand's address is determined — and for a
    /// `DD`/`FD` prefix that means **fetching the displacement byte**, which can only
    /// happen once. Everything downstream takes the resulting [`Target`] by value, so
    /// `INC (IX+d)` reads, waits and writes back at one address arrived at once.
    ///
    /// `register_base` is separate from `index` because the prefix reaches the register
    /// halves and the memory operand independently — see [`Index::for_register_half`].
    ///
    /// Deliberately charges **no** T-states: the address computation costs five, but an
    /// operand byte fetched after the displacement spends three of them, so only the
    /// caller knows how many are left. See [`Cpu::tick_index_computation`].
    fn resolve(&mut self, operand: Operand, index: Index, register_base: PairBase) -> Target {
        match operand.register_index(register_base) {
            Some(register) => Target::Register(register),
            None if index.is_displaced() => {
                let displacement = self.fetch_signed_byte();
                Target::Memory(self.indexed_address(index, displacement))
            }
            None => Target::Memory(self.regs.pair(index.base())),
        }
    }

    /// Compute an `(IX+d)` effective address, recording it in `MEMPTR` as the hardware does.
    ///
    /// *"Any instruction with (INDEX+d): MEMPTR = INDEX+d"* — one line covering the whole
    /// `DD`/`FD` set, which is why this is one write site rather than thirty. Keeping it at
    /// the single place an indexed address is computed is what makes `BIT n,(IX+d)` come out
    /// right with no special case, and it was the first of these rules to land.
    fn indexed_address(&mut self, index: Index, displacement: i8) -> u16 {
        let effective = self
            .regs
            .pair(index.base())
            .wrapping_add_signed(i16::from(displacement));
        self.set_memptr(effective);
        effective
    }

    /// [`Cpu::resolve`] for a single-operand instruction, where there is no second operand
    /// for the prefix to interfere with.
    fn resolve_only(&mut self, operand: Operand, index: Index) -> Target {
        self.resolve(operand, index, index.base())
    }

    /// Charge the address-computation cycles an `(IX+d)` operand owes.
    ///
    /// The Z80 owes five T-states between fetching the displacement and touching memory,
    /// and any operand byte fetched in between spends three of them — which is why
    /// `LD (IX+d),n` and the `DDCB` forms charge two rather than five. Corpus vectors
    /// `dd7e` (five, on the displacement's address) and `dd36` (two, on the immediate's
    /// address) are the two shapes; both put the cycles on the last byte fetched.
    fn tick_index_computation(&mut self, index: Index, touches_memory: bool, count: u8) {
        if index.is_displaced() && touches_memory {
            let last_fetched = self.regs.pc().wrapping_sub(1);
            self.internal_cycles(last_fetched, count);
        }
    }

    /// Read a resolved target, taking the memory-cycle cost for the memory form.
    fn read_target(&mut self, target: Target) -> u8 {
        match target {
            Target::Register(register) => self.regs.get(register),
            Target::Memory(address) => self.read_byte(address),
        }
    }

    /// Write a resolved target, taking the memory-cycle cost for the memory form.
    fn write_target(&mut self, target: Target, value: u8) {
        match target {
            Target::Register(register) => self.regs.set(register, value),
            Target::Memory(address) => self.write_byte(address, value),
        }
    }

    /// The extra T-state the read-modify-write forms spend holding a value between the read
    /// and the write-back. The register forms have no such cycle, which is the whole
    /// difference between `INC r` at 4 T-states and `INC (HL)` at 11.
    fn tick_read_modify_delay(&mut self, target: Target) {
        if let Target::Memory(address) = target {
            // Corpus vector `34`: the operand address stays on the bus for this cycle.
            self.internal_cycles(address, 1);
        }
    }

    // -----------------------------------------------------------------------------
    // 8-bit loads
    // -----------------------------------------------------------------------------

    /// `LD r,r'` and its `(HL)` forms.
    fn load_operand_operand(&mut self, destination: Operand, source: Operand, index: Index) {
        // INVARIANT: at most one operand can be `MemHl`. The encoding that would mean
        // `LD (HL),(HL)` is `HALT`, and `execute` routes `0x76` before the `0x40..=0x7F`
        // arm — so this is the only reason resolving *both* operands is safe. `resolve`
        // fetches a displacement byte for an indexed memory operand, and two of those would
        // consume two bytes of the instruction stream.
        debug_assert!(
            !(destination == Operand::MemHl && source == Operand::MemHl),
            "LD (HL),(HL) is HALT and must never reach here",
        );

        // When either half is the memory operand the prefix is spent on the address, so the
        // register half is *not* substituted: `DD 74` is `LD (IX+d),H`, storing real `H`.
        let touches_memory = destination == Operand::MemHl || source == Operand::MemHl;
        let register_base = index.for_register_half(touches_memory).base();

        let source = self.resolve(source, index, register_base);
        let destination = self.resolve(destination, index, register_base);
        self.tick_index_computation(index, touches_memory, INDEX_COMPUTATION);

        let value = self.read_target(source);
        self.write_target(destination, value);
    }

    /// `LD r,n` and `LD (HL),n`.
    fn load_operand_immediate(&mut self, destination: Operand, index: Index) {
        // Resolved before the immediate is fetched: `LD (IX+d),n` carries the displacement
        // *before* the immediate, so resolving afterwards would read the stream out of
        // order. That intervening fetch is also why only two computation T-states remain.
        let target = self.resolve_only(destination, index);
        let value = self.fetch_byte();
        self.tick_index_computation(
            index,
            matches!(target, Target::Memory(_)),
            INDEX_COMPUTATION_AFTER_FETCH,
        );
        self.write_target(target, value);
    }

    /// `LD A,(BC)` and `LD A,(DE)`.
    ///
    /// `MEMPTR = rp + 1`, the ordinary sixteen-bit increment. Contrast
    /// [`Cpu::store_a_indirect`], which is the same addressing mode in the other direction and
    /// takes a different rule.
    fn load_a_indirect(&mut self, base: PairBase) {
        let address = self.regs.pair(base);
        let value = self.read_byte(address);
        self.set_memptr(address.wrapping_add(1));
        self.regs.set_a(value);
    }

    /// `LD (BC),A` and `LD (DE),A`.
    ///
    /// `MEMPTR_low = (rp + 1) & #FF`, `MEMPTR_hi = A` — the accumulator-store quirk, shared
    /// verbatim with `LD (nn),A` and `OUT (n),A`. See [`accumulator_store_memptr`] for why the
    /// carry stops at the byte boundary.
    fn store_a_indirect(&mut self, base: PairBase) {
        let address = self.regs.pair(base);
        let value = self.regs.a();
        self.write_byte(address, value);
        self.set_memptr(accumulator_store_memptr(address, value));
    }

    /// `LD A,(nn)`.
    ///
    /// `MEMPTR = addr + 1`.
    fn load_a_absolute(&mut self) {
        let address = self.fetch_word();
        let value = self.read_byte(address);
        self.set_memptr(address.wrapping_add(1));
        self.regs.set_a(value);
    }

    /// `LD (nn),A`.
    ///
    /// `MEMPTR_low = (addr + 1) & #FF`, `MEMPTR_hi = A` — see [`Cpu::store_a_indirect`].
    fn store_a_absolute(&mut self) {
        let address = self.fetch_word();
        let value = self.regs.a();
        self.write_byte(address, value);
        self.set_memptr(accumulator_store_memptr(address, value));
    }

    // -----------------------------------------------------------------------------
    // 16-bit loads
    // -----------------------------------------------------------------------------

    /// `LD dd,nn`.
    fn load_pair_immediate(&mut self, base: PairBase) {
        let value = self.fetch_word();
        self.regs.set_pair(base, value);
    }

    /// `LD HL,(nn)`. The Z80 stores words low byte first.
    ///
    /// `MEMPTR = addr + 1` — which is also the address of the second byte this reads, so the
    /// latch simply holds where the transfer got to.
    fn load_pair_absolute(&mut self, base: PairBase) {
        let address = self.fetch_word();
        let low = self.read_byte(address);
        let high = self.read_byte(address.wrapping_add(1));
        self.set_memptr(address.wrapping_add(1));
        self.regs.set_pair(base, u16::from_le_bytes([low, high]));
    }

    /// `LD (nn),HL`.
    ///
    /// `MEMPTR = addr + 1`. The pair stores take the plain rule and **not** the accumulator
    /// quirk, even though they are stores: the document gives `LD (addr),rp` and
    /// `LD rp,(addr)` one line together, and separates `LD (addr),A` onto its own.
    fn store_pair_absolute(&mut self, base: PairBase) {
        let address = self.fetch_word();
        let [high, low] = self.regs.pair(base).to_be_bytes();
        self.write_byte(address, low);
        self.write_byte(address.wrapping_add(1), high);
        self.set_memptr(address.wrapping_add(1));
    }

    /// `LD SP,HL`.
    fn load_sp_from_pair(&mut self, base: PairBase) {
        // M1 runs to six T-states while the pair is transferred (4 + 2), on IR — corpus
        // vector `f9`.
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 2);
        let value = self.regs.pair(base);
        self.regs.set_sp(value);
    }

    /// `PUSH qq`.
    fn push_pair(&mut self, base: PairBase) {
        // M1 runs to five T-states before the two writes begin (4 + 1), on IR.
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 1);
        let value = self.regs.pair(base);
        self.push_word(value);
    }

    /// `POP qq`.
    ///
    /// `POP AF` reaches the flags register through this same path, which is why it can
    /// restore the undocumented bits 3 and 5 that no arithmetic instruction would
    /// produce — the pair is written as two raw bytes with no flag rule involved.
    ///
    /// # The Q fork — a choice, not a finding
    ///
    /// It writes `F` without going through [`Cpu::write_flags`], so it does **not** set the
    /// flag latch. Whether real silicon latches here is genuinely contested, and the same
    /// question applies to `EX AF,AF'`:
    ///
    /// - **Variant A (implemented):** neither latches, so `Q` is left clear and a following
    ///   `SCF` sees `Q != F`.
    /// - **Variant B:** both latch, making `Q == F`, at which point `SCF`'s rule collapses
    ///   to the accumulator-only form.
    ///
    /// Both were implemented and measured: `zexall` scores 67/67 under either, and no FUSE
    /// vector can reach the fork because each is a single instruction. **Nothing available
    /// to this project decides it.** Variant A is chosen for being the more commonly
    /// documented model — that is the whole justification. See [`crate::flags::scf`].
    fn pop_into(&mut self, base: PairBase) {
        let value = self.pop_word();
        self.regs.set_pair(base, value);
    }

    /// `EX (SP),HL` — the longest un-prefixed instruction at 19 T-states.
    ///
    /// `MEMPTR = rp value after the operation` — the word that came off the stack, not the
    /// stack address it came from and not the word that went onto it. The one rule in the set
    /// whose operand is a *value* rather than an address, which is what makes it worth
    /// stating: `MEMPTR` is an address latch, and here it latches something that is only an
    /// address if the program meant it as one.
    fn exchange_stack_pair(&mut self, base: PairBase) {
        let stack = self.regs.sp();
        let low = self.read_byte(stack);
        let high = self.read_byte(stack.wrapping_add(1));
        // One internal T-state extends the high-byte read cycle (3 + 1), still on SP + 1
        // — corpus vector `e3`.
        self.internal_cycles(stack.wrapping_add(1), 1);

        let [pair_high, pair_low] = self.regs.pair(base).to_be_bytes();
        self.write_byte(stack.wrapping_add(1), pair_high);
        self.write_byte(stack, pair_low);
        // Two more extend the final write cycle (3 + 2), on SP.
        self.internal_cycles(stack, 2);

        let exchanged = u16::from_be_bytes([high, low]);
        self.regs.set_pair(base, exchanged);
        self.set_memptr(exchanged);
    }

    // -----------------------------------------------------------------------------
    // 8-bit arithmetic and logic
    // -----------------------------------------------------------------------------

    /// `ALU A,r` and its `(HL)` form.
    fn alu_with_operand(&mut self, operation: AluOp, operand: Operand, index: Index) {
        let target = self.resolve_only(operand, index);
        self.tick_index_computation(
            index,
            matches!(target, Target::Memory(_)),
            INDEX_COMPUTATION,
        );
        let value = self.read_target(target);
        self.apply_alu(operation, value);
    }

    /// `ALU A,n`.
    fn alu_with_immediate(&mut self, operation: AluOp) {
        let value = self.fetch_byte();
        self.apply_alu(operation, value);
    }

    /// Apply one accumulator ALU operation to an already-obtained operand.
    fn apply_alu(&mut self, operation: AluOp, value: u8) {
        let a = self.regs.a();
        let carry_in = self.carry_flag();
        let (result, flags) = match operation {
            AluOp::Add => flags::add8(a, value),
            AluOp::Adc => flags::adc8(a, value, carry_in),
            AluOp::Sub => flags::sub8(a, value),
            AluOp::Sbc => flags::sbc8(a, value, carry_in),
            AluOp::And => flags::and8(a, value),
            AluOp::Xor => flags::xor8(a, value),
            AluOp::Or => flags::or8(a, value),
            // `CP` compares by subtracting and discarding the difference, so the
            // accumulator is written back unchanged.
            AluOp::Cp => (a, flags::cp8(a, value)),
        };
        self.regs.set_a(result);
        self.write_flags(flags);
    }

    /// `INC r` and `INC (HL)`.
    fn increment_operand(&mut self, operand: Operand, index: Index) {
        let target = self.resolve_only(operand, index);
        self.tick_index_computation(
            index,
            matches!(target, Target::Memory(_)),
            INDEX_COMPUTATION,
        );
        let value = self.read_target(target);
        self.tick_read_modify_delay(target);
        let (result, flags) = flags::inc8(value, self.regs.f());
        self.write_flags(flags);
        self.write_target(target, result);
    }

    /// `DEC r` and `DEC (HL)`.
    fn decrement_operand(&mut self, operand: Operand, index: Index) {
        let target = self.resolve_only(operand, index);
        self.tick_index_computation(
            index,
            matches!(target, Target::Memory(_)),
            INDEX_COMPUTATION,
        );
        let value = self.read_target(target);
        self.tick_read_modify_delay(target);
        let (result, flags) = flags::dec8(value, self.regs.f());
        self.write_flags(flags);
        self.write_target(target, result);
    }

    /// `DAA`.
    fn decimal_adjust_a(&mut self) {
        let (result, flags) = flags::daa(self.regs.a(), self.regs.f());
        self.regs.set_a(result);
        self.write_flags(flags);
    }

    /// `CPL`.
    fn complement_a(&mut self) {
        let (result, flags) = flags::cpl(self.regs.a(), self.regs.f());
        self.regs.set_a(result);
        self.write_flags(flags);
    }

    /// `SCF`.
    fn set_carry_flag(&mut self) {
        let flags = flags::scf(self.regs.a(), self.regs.f(), self.q_prev);
        self.write_flags(flags);
    }

    /// `CCF`.
    fn complement_carry_flag(&mut self) {
        let flags = flags::ccf(self.regs.a(), self.regs.f(), self.q_prev);
        self.write_flags(flags);
    }

    /// `RLCA`, `RRCA`, `RLA` and `RRA`.
    ///
    /// The four accumulator rotates differ only in which way the bits move; the flag rule
    /// is identical, so the rule is passed in rather than repeated. Note that these are
    /// *not* the `CB`-prefixed rotates applied to `A`: those set sign, zero and parity,
    /// while these four leave them untouched.
    fn rotate_a(&mut self, rotate: fn(u8, u8) -> (u8, u8)) {
        let (result, flags) = rotate(self.regs.a(), self.regs.f());
        self.regs.set_a(result);
        self.write_flags(flags);
    }

    // -----------------------------------------------------------------------------
    // 16-bit arithmetic
    // -----------------------------------------------------------------------------

    /// `ADD HL,ss`.
    ///
    /// Both operands take a base because `DD 29` is `ADD IX,IX`: the prefix substitutes
    /// the destination and the source together.
    ///
    /// `MEMPTR = rp1_before_operation + 1`, and `rp1` is the **destination** — so `ADD IX,BC`
    /// latches `IX + 1` while `ADD HL,BC` latches `HL + 1`, and the prefix that already moved
    /// the destination moves the rule with it at no cost. `ADD IX,rr` and `ADD IY,rr` are
    /// separate groups in the exerciser from `ADD HL,rr` for exactly this reason.
    ///
    /// Reading the destination **before** the add is the whole content of *before_operation*:
    /// this is one of two rules in the set that would be silently wrong if the write happened
    /// a line later, and it is why `augend` is bound rather than the pair being read twice.
    fn add_pair(&mut self, destination: PairBase, operand: PairBase) {
        // The 16-bit add occupies two internal machine cycles (4 + 3) after M1, all seven
        // T-states on IR — corpus vector `09`.
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 7);
        let addend = self.regs.pair(operand);
        let augend = self.regs.pair(destination);
        self.set_memptr(augend.wrapping_add(1));
        let (result, flags) = flags::add16(augend, addend, self.regs.f());
        self.regs.set_pair(destination, result);
        self.write_flags(flags);
    }

    /// `INC ss`. The 16-bit increment and decrement affect no flags at all, which is what
    /// makes them usable as pointer arithmetic inside a flag-sensitive loop.
    fn increment_pair(&mut self, base: PairBase) {
        // M1 runs to six T-states while the incrementer works (4 + 2), on IR — corpus
        // vector `03`.
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 2);
        let value = self.regs.pair(base).wrapping_add(1);
        self.regs.set_pair(base, value);
    }

    /// `DEC ss`. Affects no flags, as [`Cpu::increment_pair`] does not.
    fn decrement_pair(&mut self, base: PairBase) {
        // M1 runs to six T-states while the incrementer works (4 + 2), on IR.
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 2);
        let value = self.regs.pair(base).wrapping_sub(1);
        self.regs.set_pair(base, value);
    }

    // -----------------------------------------------------------------------------
    // Jumps, calls and returns
    // -----------------------------------------------------------------------------

    /// Fetch a 16-bit branch target, recording it in `MEMPTR`.
    ///
    /// `JP nn`, `JP cc,nn`, `CALL nn` and `CALL cc,nn` share one `MEMPTR` rule, and it is the
    /// one rule in the set stated with an explicit scope: *"JP (except JP rp) / CALL addr
    /// (even in case of conditional call/jp, independantly on condition satisfied or not) —
    /// MEMPTR = addr"*. The latch follows the **operand fetch**, not the branch.
    ///
    /// Fetching and latching together is what keeps that true at all four call sites without
    /// four chances to put the write on the taken side of an `if`. It is also the exception
    /// that shapes its neighbours: [`Cpu::jump_relative`] and [`Cpu::return_conditional`] latch
    /// only when they actually jump, because the document's other line says *"JR/DJNZ/RET/RETI/
    /// RST (**jumping to** addr)"* — a qualification it would not have written twice by
    /// accident, having just written the opposite for `JP`.
    ///
    /// `JP (HL)` is excepted by name and takes no rule at all; see [`Cpu::jump_to_pair`].
    fn fetch_branch_target(&mut self) -> u16 {
        let target = self.fetch_word();
        self.set_memptr(target);
        target
    }

    /// `JP nn`.
    fn jump_unconditional(&mut self) {
        let target = self.fetch_branch_target();
        self.regs.set_pc(target);
    }

    /// `JP cc,nn`.
    ///
    /// The operand is fetched whether or not the branch is taken, so the conditional jump
    /// costs ten T-states either way — the Z80 has no branch penalty here.
    ///
    /// # Deliberate divergence from the FUSE trace
    ///
    /// The corpus records no `MR` for the not-taken path of `JP cc`, `JR cc`, `CALL cc` and
    /// the last `DJNZ`; roughly 21 vectors carry a documented exception for it. The read is
    /// performed here anyway, because three things say the cycle is a real one:
    ///
    /// - Zilog gives `JP cc,nn` a *single* cycle count, 10 T. `CALL cc` (17/10) and
    ///   `RET cc` (11/5) have two, because a machine cycle genuinely disappears. A cycle
    ///   that does not happen changes the count.
    /// - `PC` still advances by three, and on the Z80 `PC` increments *as part of* the
    ///   operand-fetch cycle.
    /// - `MEMPTR`/`WZ` is loaded with `nn` by `JP cc,nn` whether or not the condition
    ///   holds — a hardware measurement from `BIT n,(HL)`, not a documentation claim — and
    ///   bytes that were never read cannot load it.
    ///
    /// The corpus's own asymmetry is the tell: `CALL cc` really does lose machine cycles
    /// when not taken, `JP cc` does not, and the trace treats both alike. Nothing
    /// observable differs on a side-effect-free bus; addresses and timing are identical.
    ///
    /// **The third argument has since been made observable rather than merely asserted.** It
    /// was written here as a reason to perform a read the corpus does not record; the read now
    /// also loads the latch, through [`Cpu::fetch_branch_target`], and `112 JP CC,NN` is a
    /// group the MEMPTR exerciser grades. An argument that was only an argument when this
    /// comment was written is now a gate.
    fn jump_conditional(&mut self, condition: Condition) {
        let target = self.fetch_branch_target();
        if condition.holds(self.regs.f()) {
            self.regs.set_pc(target);
        }
    }

    /// `JP (HL)`. Despite the notation there is no memory access: the jump target is the
    /// register pair itself, which is why this is the only four T-state jump.
    ///
    /// **It leaves `MEMPTR` alone**, and that is a rule rather than an omission: the document
    /// writes *"JP (except JP rp)"*, carving this one encoding out of the family whose every
    /// other member latches its target. The same absence of a memory access explains both
    /// facts — nothing here forms an address, so there is nothing for an address latch to
    /// catch, and there is no operand-fetch cycle to spend.
    fn jump_to_pair(&mut self, base: PairBase) {
        let target = self.regs.pair(base);
        self.regs.set_pc(target);
    }

    /// `JR e`.
    fn jump_relative_unconditional(&mut self) {
        let offset = self.fetch_signed_byte();
        // Five internal T-states while the target address is computed, all on the
        // displacement byte's own address — corpus vector `18`.
        let displacement = self.regs.pc().wrapping_sub(1);
        self.internal_cycles(displacement, 5);
        self.jump_relative(offset);
    }

    /// `JR cc,e`. Unlike the absolute conditional jump, this one *is* cheaper when not
    /// taken (7 against 12) because the address computation is skipped.
    fn jump_relative_conditional(&mut self, condition: Condition) {
        let offset = self.fetch_signed_byte();
        if condition.holds(self.regs.f()) {
            let displacement = self.regs.pc().wrapping_sub(1);
            self.internal_cycles(displacement, 5);
            self.jump_relative(offset);
        }
    }

    /// `DJNZ e` — decrement `B` and branch while it is non-zero. Affects no flags.
    fn decrement_and_jump(&mut self) {
        // M1 runs to five T-states while B is decremented (4 + 1), on IR — corpus
        // vector `10`.
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 1);
        // `B` falls during M1, before the displacement cycle begins.
        let counter = self.regs.get(index::B).wrapping_sub(1);
        self.regs.set(index::B, counter);
        let offset = self.fetch_signed_byte();
        if counter != 0 {
            let displacement = self.regs.pc().wrapping_sub(1);
            self.internal_cycles(displacement, 5);
            self.jump_relative(offset);
        }
    }

    /// Apply a relative displacement to `PC`.
    ///
    /// The displacement is measured from the address *after* the operand byte, which is
    /// where `PC` already points once the operand has been fetched.
    ///
    /// `MEMPTR = addr`, the address jumped **to**, which is why the write is here rather than
    /// beside the displacement fetch: `JR e` has no absolute operand to latch, so the latch
    /// cannot be loaded until the addition has happened. All three callers — `JR e`, `JR cc,e`
    /// and `DJNZ e` — reach this only when the branch is taken, which is what the document's
    /// *"(jumping to addr)"* qualification asks for. A not-taken `JR cc` leaves the latch
    /// untouched, and that is the whole difference from `JP cc,nn` a few lines above.
    fn jump_relative(&mut self, offset: i8) {
        let target = self.regs.pc().wrapping_add_signed(i16::from(offset));
        self.regs.set_pc(target);
        self.set_memptr(target);
    }

    /// `CALL nn`.
    fn call_unconditional(&mut self) {
        let target = self.fetch_branch_target();
        // Corpus vector `cd`: the internal cycle holds the last operand byte's address.
        let last_operand = self.regs.pc().wrapping_sub(1);
        self.call_to(target, last_operand);
    }

    /// `CALL cc,nn`.
    ///
    /// The latch is loaded by the operand fetch and so survives the condition failing, exactly
    /// as `JP cc,nn`'s does — see [`Cpu::fetch_branch_target`]. Unlike `JP cc`, the *machine
    /// cycles* here genuinely do differ by outcome (17 T-states against 10), which is why the
    /// push and the internal cycle sit inside the branch and the latch does not.
    fn call_conditional(&mut self, condition: Condition) {
        let target = self.fetch_branch_target();
        if condition.holds(self.regs.f()) {
            let last_operand = self.regs.pc().wrapping_sub(1);
            self.call_to(target, last_operand);
        }
    }

    /// `RST p` — `11 ttt 111`, a call to the page-zero address `ttt * 8`.
    ///
    /// The eight restart vectors are why the bottom of the address space is reserved: a
    /// one-byte call is precious in an eight-bit program, and the Spectrum's ROM puts its
    /// most-used routines there.
    /// `MEMPTR = addr`. `RST` is listed with the jumps and returns rather than with `CALL`,
    /// and the reason is visible in this handler: it has no operand fetch for the latch to
    /// follow, so the destination is all there is — which is also why the write is here and
    /// not in the shared [`Cpu::call_to`], where it would be a second writer for a `CALL` that
    /// has already latched the same value at its operand fetch.
    fn restart(&mut self, opcode: u8) {
        /// Bits 5–3 scaled by eight — already in place in the opcode.
        const TARGET_MASK: u8 = 0x38;
        // `RST` has no operands, so its internal cycle holds IR instead — corpus
        // vector `ff`.
        let refresh = self.regs.refresh_address();
        let target = u16::from(opcode & TARGET_MASK);
        self.set_memptr(target);
        self.call_to(target, refresh);
    }

    /// The shared tail of `CALL` and `RST`: one internal T-state, push the return address,
    /// jump.
    ///
    /// `RST p` is exactly a call to a fixed address and has exactly these machine cycles,
    /// so the two instructions share one implementation. The whole difference in their
    /// timing — 17 T-states against 11 — is the two operand-fetch cycles `CALL` needs to
    /// read its target and `RST` gets for free from its own opcode.
    fn call_to(&mut self, target: u16, internal_address: u16) {
        self.internal_cycles(internal_address, 1);
        let return_address = self.regs.pc();
        self.push_word(return_address);
        self.regs.set_pc(target);
    }

    /// `RET`.
    ///
    /// `MEMPTR = addr` — the address returned to, which the stack has just supplied.
    fn return_unconditional(&mut self) {
        let target = self.pop_word();
        self.regs.set_pc(target);
        self.set_memptr(target);
    }

    /// `RET cc`.
    ///
    /// The condition test extends M1 by one T-state whether or not the branch is taken, so
    /// an untaken `RET cc` costs five T-states — the cheapest conditional on the chip, and
    /// the reason it is the idiomatic early-out.
    ///
    /// `MEMPTR` follows suit and is loaded **only when the branch is taken**, unlike
    /// `JP cc,nn` and `CALL cc,nn`. Here that needs no appeal to the document's wording: an
    /// untaken `RET cc` performs no stack read at all, so there is no address in the
    /// instruction for the latch to hold. The conditional jumps differ precisely because they
    /// fetch their operand either way.
    fn return_conditional(&mut self, condition: Condition) {
        // The condition test extends M1 by one T-state, on IR.
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 1);
        if condition.holds(self.regs.f()) {
            let target = self.pop_word();
            self.regs.set_pc(target);
            self.set_memptr(target);
        }
    }

    // -----------------------------------------------------------------------------
    // I/O
    // -----------------------------------------------------------------------------

    /// `IN A,(n)`.
    ///
    /// The port address carries the accumulator in its high half. This form affects no
    /// flags — unlike the `ED`-prefixed `IN r,(C)`, which does.
    ///
    /// `MEMPTR = (A_before_operation << 8) + port + 1` — which is the whole sixteen-bit port
    /// address plus one, **carry and all**, so `IN A,($FF)` with `A = 0x40` leaves `0x4100`.
    /// Its mirror image `OUT (n),A` truncates instead. Reading `A` before the port supplies
    /// its result is the *before_operation* in the formula, and here it is not a subtlety —
    /// the same accumulator is the address's high half and the destination.
    fn input_immediate(&mut self) {
        let low = self.fetch_byte();
        let port = u16::from_be_bytes([self.regs.a(), low]);
        let value = self.read_port(port);
        self.set_memptr(port.wrapping_add(1));
        self.regs.set_a(value);
    }

    /// `OUT (n),A`. The accumulator supplies both the data and the high half of the port
    /// address.
    ///
    /// `MEMPTR_low = (port + 1) & #FF`, `MEMPTR_hi = A` — the third instruction taking the
    /// accumulator-store quirk, alongside `LD (nn),A` and `LD (rp),A`. Where `IN A,(n)` a few
    /// lines above carries into the high byte, this one does not, and the pair of them is the
    /// clearest statement of the asymmetry in the whole set: identical addressing, opposite
    /// direction, different rule.
    fn output_immediate(&mut self) {
        let low = self.fetch_byte();
        let value = self.regs.a();
        let port = u16::from_be_bytes([value, low]);
        self.write_port(port, value);
        self.set_memptr(accumulator_store_memptr(port, value));
    }

    // -----------------------------------------------------------------------------
    // Control
    // -----------------------------------------------------------------------------

    /// `HALT` — stop until an interrupt arrives.
    fn halt(&mut self) {
        self.halted = true;
        // The Z80 holds PC on the HALT opcode itself rather than running past it. The
        // fetch has already advanced PC, so step it back: `PC` then names the instruction
        // the CPU is stopped on, which is what a debugger and a snapshot both want, and
        // what an accepted interrupt steps past.
        let held = self.regs.pc().wrapping_sub(1);
        self.regs.set_pc(held);
    }

    /// `DI` — clear both interrupt flip-flops.
    fn disable_interrupts(&mut self) {
        self.interrupts.iff1 = false;
        self.interrupts.iff2 = false;
    }

    /// `EI` — set both interrupt flip-flops, deferring acceptance by one instruction.
    ///
    /// An interrupt is not accepted until after the instruction *following* `EI`, so an
    /// `EI` / `RET` pair always returns before servicing anything. Software relies on this
    /// to leave an interrupt handler without immediately re-entering it.
    fn enable_interrupts(&mut self) {
        self.interrupts.iff1 = true;
        self.interrupts.iff2 = true;
        self.interrupts.ei_pending = true;
    }

    // -----------------------------------------------------------------------------
    // Stack primitives
    // -----------------------------------------------------------------------------

    /// Push a word, high byte first, growing the stack downwards.
    pub(crate) fn push_word(&mut self, value: u16) {
        let [high, low] = value.to_be_bytes();
        let stack = self.regs.sp().wrapping_sub(1);
        self.write_byte(stack, high);
        let stack = stack.wrapping_sub(1);
        self.write_byte(stack, low);
        self.regs.set_sp(stack);
    }

    /// Pop a word, low byte first.
    fn pop_word(&mut self) -> u16 {
        let stack = self.regs.sp();
        let low = self.read_byte(stack);
        let high = self.read_byte(stack.wrapping_add(1));
        self.regs.set_sp(stack.wrapping_add(2));
        u16::from_le_bytes([low, high])
    }
}
