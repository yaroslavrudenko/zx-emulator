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
use crate::decode::{AluOp, Condition, Operand};
use crate::flags;
use crate::registers::{PairBase, index, pair};
use crate::{Cpu, StepError};

impl<B: Bus> Cpu<B> {
    /// Execute one already-fetched opcode.
    ///
    /// `base` is the pair standing in for `HL`, which is always [`pair::HL`] for the
    /// un-prefixed set.
    ///
    /// The `CB`, `DD`, `ED` and `FD` prefixes are reported as
    /// [`StepError::UnsupportedPrefix`] rather than panicking; their instruction sets
    /// arrive in M2.
    pub(crate) fn execute(&mut self, opcode: u8, base: PairBase) -> Result<(), StepError> {
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
            0x21 => self.load_pair_immediate(base),     // LD HL,nn
            0x31 => self.load_pair_immediate(pair::SP), // LD SP,nn

            // `ADD HL,ss` takes the base twice: `DD 29` is `ADD IX,IX`, so both the
            // destination and the operand follow the prefix.
            0x09 => self.add_pair(base, pair::BC), // ADD HL,BC
            0x19 => self.add_pair(base, pair::DE), // ADD HL,DE
            0x29 => self.add_pair(base, base),     // ADD HL,HL
            0x39 => self.add_pair(base, pair::SP), // ADD HL,SP

            0x02 => self.store_a_indirect(pair::BC), // LD (BC),A
            0x12 => self.store_a_indirect(pair::DE), // LD (DE),A
            0x0A => self.load_a_indirect(pair::BC),  // LD A,(BC)
            0x1A => self.load_a_indirect(pair::DE),  // LD A,(DE)

            0x22 => self.store_pair_absolute(base), // LD (nn),HL
            0x2A => self.load_pair_absolute(base),  // LD HL,(nn)
            0x32 => self.store_a_absolute(),        // LD (nn),A
            0x3A => self.load_a_absolute(),         // LD A,(nn)

            0x03 => self.increment_pair(pair::BC), // INC BC
            0x13 => self.increment_pair(pair::DE), // INC DE
            0x23 => self.increment_pair(base),     // INC HL
            0x33 => self.increment_pair(pair::SP), // INC SP
            0x0B => self.decrement_pair(pair::BC), // DEC BC
            0x1B => self.decrement_pair(pair::DE), // DEC DE
            0x2B => self.decrement_pair(base),     // DEC HL
            0x3B => self.decrement_pair(pair::SP), // DEC SP

            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                self.increment_operand(Operand::destination(opcode), base);
            } // INC r / INC (HL)
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                self.decrement_operand(Operand::destination(opcode), base);
            } // DEC r / DEC (HL)
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                self.load_operand_immediate(Operand::destination(opcode), base);
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
                    base,
                );
            }

            // ---- 0x80–0xBF: ALU A,r -------------------------------------------------
            0x80..=0xBF => {
                self.alu_with_operand(AluOp::from_opcode(opcode), Operand::source(opcode), base);
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

            0xC1 => self.pop_into(pair::BC),  // POP BC
            0xD1 => self.pop_into(pair::DE),  // POP DE
            0xE1 => self.pop_into(base),      // POP HL
            0xF1 => self.pop_into(pair::AF),  // POP AF
            0xC5 => self.push_pair(pair::BC), // PUSH BC
            0xD5 => self.push_pair(pair::DE), // PUSH DE
            0xE5 => self.push_pair(base),     // PUSH HL
            0xF5 => self.push_pair(pair::AF), // PUSH AF

            0xC3 => self.jump_unconditional(),       // JP nn
            0xC9 => self.return_unconditional(),     // RET
            0xCD => self.call_unconditional(),       // CALL nn
            0xD3 => self.output_immediate(),         // OUT (n),A
            0xDB => self.input_immediate(),          // IN A,(n)
            0xD9 => self.regs.exchange_shadow_set(), // EXX
            0xE3 => self.exchange_stack_pair(base),  // EX (SP),HL
            0xE9 => self.jump_to_pair(base),         // JP (HL)
            0xEB => self.regs.exchange_de_hl(),      // EX DE,HL — never an index register
            0xF3 => self.disable_interrupts(),       // DI
            0xF9 => self.load_sp_from_pair(base),    // LD SP,HL
            0xFB => self.enable_interrupts(),        // EI

            0xCB | 0xDD | 0xED | 0xFD => return Err(self.unsupported_prefix(opcode)),
        }
        Ok(())
    }

    // -----------------------------------------------------------------------------
    // Operand access
    // -----------------------------------------------------------------------------

    /// Read the operand a 3-bit field names, taking the memory-cycle cost when the field
    /// names `(HL)`.
    fn read_operand(&mut self, operand: Operand, base: PairBase) -> u8 {
        match operand.register_index(base) {
            Some(register) => self.regs.get(register),
            None => {
                let address = self.regs.pair(base);
                self.read_byte(address)
            }
        }
    }

    /// Write the operand a 3-bit field names, taking the memory-cycle cost when the field
    /// names `(HL)`.
    fn write_operand(&mut self, operand: Operand, base: PairBase, value: u8) {
        match operand.register_index(base) {
            Some(register) => self.regs.set(register, value),
            None => {
                let address = self.regs.pair(base);
                self.write_byte(address, value);
            }
        }
    }

    /// The extra T-state the read-modify-write forms spend holding a value between the
    /// read and the write-back. The register forms have no such cycle, which is the whole
    /// difference between `INC r` at 4 T-states and `INC (HL)` at 11.
    fn tick_read_modify_delay(&mut self, operand: Operand, base: PairBase) {
        if operand == Operand::MemHl {
            // Corpus vector `34`: the operand address stays on the bus for this cycle.
            let address = self.regs.pair(base);
            self.internal_cycles(address, 1);
        }
    }

    // -----------------------------------------------------------------------------
    // 8-bit loads
    // -----------------------------------------------------------------------------

    /// `LD r,r'` and its `(HL)` forms.
    fn load_operand_operand(&mut self, destination: Operand, source: Operand, base: PairBase) {
        let value = self.read_operand(source, base);
        self.write_operand(destination, base, value);
    }

    /// `LD r,n` and `LD (HL),n`.
    fn load_operand_immediate(&mut self, destination: Operand, base: PairBase) {
        let value = self.fetch_byte();
        self.write_operand(destination, base, value);
    }

    /// `LD A,(BC)` and `LD A,(DE)`.
    fn load_a_indirect(&mut self, base: PairBase) {
        let address = self.regs.pair(base);
        let value = self.read_byte(address);
        self.regs.set_a(value);
    }

    /// `LD (BC),A` and `LD (DE),A`.
    fn store_a_indirect(&mut self, base: PairBase) {
        let address = self.regs.pair(base);
        let value = self.regs.a();
        self.write_byte(address, value);
    }

    /// `LD A,(nn)`.
    fn load_a_absolute(&mut self) {
        let address = self.fetch_word();
        let value = self.read_byte(address);
        self.regs.set_a(value);
    }

    /// `LD (nn),A`.
    fn store_a_absolute(&mut self) {
        let address = self.fetch_word();
        let value = self.regs.a();
        self.write_byte(address, value);
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
    fn load_pair_absolute(&mut self, base: PairBase) {
        let address = self.fetch_word();
        let low = self.read_byte(address);
        let high = self.read_byte(address.wrapping_add(1));
        self.regs.set_pair(base, u16::from_le_bytes([low, high]));
    }

    /// `LD (nn),HL`.
    fn store_pair_absolute(&mut self, base: PairBase) {
        let address = self.fetch_word();
        let [high, low] = self.regs.pair(base).to_be_bytes();
        self.write_byte(address, low);
        self.write_byte(address.wrapping_add(1), high);
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
    fn pop_into(&mut self, base: PairBase) {
        let value = self.pop_word();
        self.regs.set_pair(base, value);
    }

    /// `EX (SP),HL` — the longest un-prefixed instruction at 19 T-states.
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

        self.regs.set_pair(base, u16::from_be_bytes([high, low]));
    }

    // -----------------------------------------------------------------------------
    // 8-bit arithmetic and logic
    // -----------------------------------------------------------------------------

    /// `ALU A,r` and its `(HL)` form.
    fn alu_with_operand(&mut self, operation: AluOp, operand: Operand, base: PairBase) {
        let value = self.read_operand(operand, base);
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
    fn increment_operand(&mut self, operand: Operand, base: PairBase) {
        let value = self.read_operand(operand, base);
        self.tick_read_modify_delay(operand, base);
        let (result, flags) = flags::inc8(value, self.regs.f());
        self.write_flags(flags);
        self.write_operand(operand, base, result);
    }

    /// `DEC r` and `DEC (HL)`.
    fn decrement_operand(&mut self, operand: Operand, base: PairBase) {
        let value = self.read_operand(operand, base);
        self.tick_read_modify_delay(operand, base);
        let (result, flags) = flags::dec8(value, self.regs.f());
        self.write_flags(flags);
        self.write_operand(operand, base, result);
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
        let flags = flags::scf(self.regs.a(), self.regs.f());
        self.write_flags(flags);
    }

    /// `CCF`.
    fn complement_carry_flag(&mut self) {
        let flags = flags::ccf(self.regs.a(), self.regs.f());
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
    fn add_pair(&mut self, destination: PairBase, operand: PairBase) {
        // The 16-bit add occupies two internal machine cycles (4 + 3) after M1, all seven
        // T-states on IR — corpus vector `09`.
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 7);
        let addend = self.regs.pair(operand);
        let (result, flags) = flags::add16(self.regs.pair(destination), addend, self.regs.f());
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

    /// `JP nn`.
    fn jump_unconditional(&mut self) {
        let target = self.fetch_word();
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
    fn jump_conditional(&mut self, condition: Condition) {
        let target = self.fetch_word();
        if condition.holds(self.regs.f()) {
            self.regs.set_pc(target);
        }
    }

    /// `JP (HL)`. Despite the notation there is no memory access: the jump target is the
    /// register pair itself, which is why this is the only four T-state jump.
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
    fn jump_relative(&mut self, offset: i8) {
        let target = self.regs.pc().wrapping_add_signed(i16::from(offset));
        self.regs.set_pc(target);
    }

    /// `CALL nn`.
    fn call_unconditional(&mut self) {
        let target = self.fetch_word();
        // Corpus vector `cd`: the internal cycle holds the last operand byte's address.
        let last_operand = self.regs.pc().wrapping_sub(1);
        self.call_to(target, last_operand);
    }

    /// `CALL cc,nn`.
    fn call_conditional(&mut self, condition: Condition) {
        let target = self.fetch_word();
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
    fn restart(&mut self, opcode: u8) {
        /// Bits 5–3 scaled by eight — already in place in the opcode.
        const TARGET_MASK: u8 = 0x38;
        // `RST` has no operands, so its internal cycle holds IR instead — corpus
        // vector `ff`.
        let refresh = self.regs.refresh_address();
        self.call_to(u16::from(opcode & TARGET_MASK), refresh);
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
    fn return_unconditional(&mut self) {
        let target = self.pop_word();
        self.regs.set_pc(target);
    }

    /// `RET cc`.
    ///
    /// The condition test extends M1 by one T-state whether or not the branch is taken, so
    /// an untaken `RET cc` costs five T-states — the cheapest conditional on the chip, and
    /// the reason it is the idiomatic early-out.
    fn return_conditional(&mut self, condition: Condition) {
        // The condition test extends M1 by one T-state, on IR.
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, 1);
        if condition.holds(self.regs.f()) {
            let target = self.pop_word();
            self.regs.set_pc(target);
        }
    }

    // -----------------------------------------------------------------------------
    // I/O
    // -----------------------------------------------------------------------------

    /// `IN A,(n)`.
    ///
    /// The port address carries the accumulator in its high half. This form affects no
    /// flags — unlike the `ED`-prefixed `IN r,(C)`, which does.
    fn input_immediate(&mut self) {
        let low = self.fetch_byte();
        let port = u16::from_be_bytes([self.regs.a(), low]);
        let value = self.read_port(port);
        self.regs.set_a(value);
    }

    /// `OUT (n),A`. The accumulator supplies both the data and the high half of the port
    /// address.
    fn output_immediate(&mut self) {
        let low = self.fetch_byte();
        let value = self.regs.a();
        let port = u16::from_be_bytes([value, low]);
        self.write_port(port, value);
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
