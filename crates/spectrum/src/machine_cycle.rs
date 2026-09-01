//! Recovering machine-cycle boundaries from a stream of per-T-state ticks.
//!
//! # Why this exists
//!
//! The ULA charges contention **once per transfer cycle** and **once per T-state of an
//! internal cycle**. That distinction is the whole reason `Bus::tick` is called per
//! T-state rather than batched, and it is why a machine has to know which of the two it is
//! looking at.
//!
//! The `Bus` trait does not say. What it gives is: a transfer callback (`read`, `write`,
//! `in_port`, `out_port`) followed by that cycle's ticks, each carrying the address the
//! Z80 drives. So a transfer *opens* a cycle, its own T-states are already paid for, and
//! any tick outside such a window is a standalone internal cycle. That is what this type
//! tracks.
//!
//! # The one case the contract cannot resolve
//!
//! Cycle **lengths** are known for three of the four transfers — a write is three
//! T-states, a port access is four — but a `read` is three T-states for an operand fetch
//! and **four** for an M1 opcode fetch, and the core issues both as `read` followed by
//! ticks at the same address. Two real instruction shapes therefore produce a
//! byte-identical stream:
//!
//! | Stream | `LD A,B` (M1) | `INC (HL)` (read + read-modify delay) |
//! |---|---|---|
//! | `read(A)`, `tick(A)`×4 | one contention | **two** contentions |
//!
//! The resolution used here is to **defer** the fourth tick. An opcode fetch is exactly
//! four T-states, so a *fifth* tick at the same address proves the run was a three-T-state
//! read followed by internal cycles — at which point the deferred T-state is charged at
//! the position it actually occupied, and the model is exact. If no fifth tick arrives the
//! deferred T-state is dropped, which is right for an M1 fetch and loses exactly one
//! contention point for a read followed by exactly one internal cycle at the same address.
//!
//! Charging it late is not an approximation: only the deferred T-state itself has elapsed
//! since, and it consumed no contention, so adding the stall now lands the clock exactly
//! where inserting it at the time would have.
//!
//! # What the residual error is, precisely
//!
//! One contention point — 0 to 6 T-states, and only when the address is contended — for
//! each execution of an instruction that performs **exactly one** internal cycle at the
//! address it has just read:
//!
//! - `INC`/`DEC (HL)` and `(IX+d)`/`(IY+d)`
//! - `BIT`/`SET`/`RES` and the `CB` rotates and shifts on `(HL)` and `(IX+d)`
//! - `EX (SP),HL` and `EX (SP),IX`/`IY`
//!
//! Runs of *more* than one internal cycle after a read — `JR`, `DJNZ`, `(IX+d)` address
//! computation, `RLD`/`RRD`, `CPI`/`CPD` and their repeating forms — are exact, because
//! the fifth tick resolves them. So is every opcode fetch, which is the common case by a
//! wide margin.
//!
//! `crates/z80`'s `Bus` gaining a defaulted `fn fetch(&mut self, addr) -> u8` would close
//! the gap completely and is non-breaking. `docs/MACHINE.md` names it; it is not added
//! speculatively, and this module is the evidence that it is now wanted.

/// What one `Bus::tick` costs, once the cycle it belongs to is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickCost {
    /// Inside a transfer's own machine cycle. Its contention was charged when the transfer
    /// opened the cycle, so this T-state is free.
    Covered,
    /// A standalone internal cycle: it contends on its own account, at its own position.
    Internal,
    /// The fourth T-state of a run that began with a read. Free for now — see the module
    /// documentation.
    Deferred,
    /// A fifth T-state at the same address, which an opcode fetch cannot produce. The run
    /// was a read plus internal cycles: the carried frame position is where the deferred
    /// T-state sat, and it must be charged there before this one is charged at the clock's
    /// current position.
    Resolved(u32),
}

/// T-states a memory read or write cycle occupies.
const MEMORY_CYCLE: u8 = 3;

/// T-states an I/O port cycle occupies.
const PORT_CYCLE: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// No cycle is open: every tick stands alone.
    Idle,
    /// A transfer's own T-states are still being delivered.
    ///
    /// `deferrable` marks a cycle opened by a `read`, whose true length is three or four
    /// and is not yet decided.
    Transfer {
        address: u16,
        owed: u8,
        deferrable: bool,
    },
    /// A read's three T-states are spent and a fourth arrived at the same address.
    Undecided { address: u16, at: u32 },
}

/// The machine cycle currently in progress, reconstructed from the tick stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineCycle {
    state: State,
}

impl Default for MachineCycle {
    fn default() -> Self {
        Self::new()
    }
}

impl MachineCycle {
    /// No cycle in progress.
    #[must_use]
    pub const fn new() -> Self {
        Self { state: State::Idle }
    }

    /// A memory read has begun at `address`: three T-states are covered, and a fourth may
    /// be if this turns out to be an opcode fetch.
    #[inline]
    pub const fn open_read(&mut self, address: u16) {
        self.state = State::Transfer {
            address,
            owed: MEMORY_CYCLE,
            deferrable: true,
        };
    }

    /// A memory write has begun at `address`. A write cycle is always three T-states, so
    /// nothing about it is ambiguous.
    #[inline]
    pub const fn open_write(&mut self, address: u16) {
        self.state = State::Transfer {
            address,
            owed: MEMORY_CYCLE,
            deferrable: false,
        };
    }

    /// An I/O cycle has begun at `port`. Always four T-states.
    #[inline]
    pub const fn open_port(&mut self, port: u16) {
        self.state = State::Transfer {
            address: port,
            owed: PORT_CYCLE,
            deferrable: false,
        };
    }

    /// Account for one T-state at `address`, the clock standing at `frame_t_state`.
    #[inline]
    pub const fn absorb(&mut self, address: u16, frame_t_state: u32) -> TickCost {
        match self.state {
            State::Transfer {
                address: open,
                owed,
                deferrable,
            } if open == address => {
                if owed > 0 {
                    self.state = State::Transfer {
                        address: open,
                        owed: owed - 1,
                        deferrable,
                    };
                    TickCost::Covered
                } else if deferrable {
                    self.state = State::Undecided {
                        address: open,
                        at: frame_t_state,
                    };
                    TickCost::Deferred
                } else {
                    self.state = State::Idle;
                    TickCost::Internal
                }
            }
            State::Undecided { address: open, at } if open == address => {
                self.state = State::Idle;
                TickCost::Resolved(at)
            }
            // Every other shape ends the run: a tick at a different address, or one with
            // nothing open. A T-state deferred by a cycle that has now ended was the last
            // T-state of an opcode fetch and is already paid for, so it is dropped.
            _ => {
                self.state = State::Idle;
                TickCost::Internal
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cost of each tick in a run, so a whole machine cycle reads as one assertion.
    fn run(cycle: &mut MachineCycle, address: u16, ticks: u32) -> Vec<TickCost> {
        (0..ticks)
            .map(|offset| cycle.absorb(address, 100 + offset))
            .collect()
    }

    #[test]
    fn an_opcode_fetch_is_one_cycle_of_four_t_states() {
        let mut cycle = MachineCycle::new();
        cycle.open_read(0x8000);
        assert_eq!(
            run(&mut cycle, 0x8000, 4),
            vec![
                TickCost::Covered,
                TickCost::Covered,
                TickCost::Covered,
                TickCost::Deferred
            ]
        );
        // The next event is another fetch: the deferred T-state is dropped, so the fetch
        // contends exactly once.
        cycle.open_read(0x8001);
        assert_eq!(run(&mut cycle, 0x8001, 1), vec![TickCost::Covered]);
    }

    #[test]
    fn a_memory_read_followed_by_internal_cycles_is_resolved_exactly() {
        // The `JR` / `(IX+d)` shape: read the displacement, then five internal cycles at
        // the same address. All six contention points must survive.
        let mut cycle = MachineCycle::new();
        cycle.open_read(0x4321);
        let costs = run(&mut cycle, 0x4321, 8);
        assert_eq!(
            costs,
            vec![
                TickCost::Covered,
                TickCost::Covered,
                TickCost::Covered,
                TickCost::Deferred,
                TickCost::Resolved(103),
                TickCost::Internal,
                TickCost::Internal,
                TickCost::Internal,
            ],
            "the deferred T-state must be charged at the position it occupied"
        );
    }

    #[test]
    fn a_write_cycle_is_never_ambiguous() {
        // `LDIR` writes then performs two internal cycles at the write address. A write is
        // always three T-states, so both internal cycles are charged.
        let mut cycle = MachineCycle::new();
        cycle.open_write(0xC000);
        assert_eq!(
            run(&mut cycle, 0xC000, 5),
            vec![
                TickCost::Covered,
                TickCost::Covered,
                TickCost::Covered,
                TickCost::Internal,
                TickCost::Internal,
            ]
        );
    }

    #[test]
    fn a_port_cycle_covers_four_t_states() {
        let mut cycle = MachineCycle::new();
        cycle.open_port(0x7FFE);
        assert_eq!(
            run(&mut cycle, 0x7FFE, 5),
            vec![
                TickCost::Covered,
                TickCost::Covered,
                TickCost::Covered,
                TickCost::Covered,
                TickCost::Internal,
            ]
        );
    }

    #[test]
    fn internal_cycles_at_a_different_address_end_the_run() {
        // `ADD HL,BC`: an opcode fetch, then seven internal cycles on the refresh address.
        // All seven must be charged, and the fetch's deferred T-state dropped.
        let mut cycle = MachineCycle::new();
        cycle.open_read(0x8000);
        run(&mut cycle, 0x8000, 4);
        assert_eq!(run(&mut cycle, 0x003F, 7), vec![TickCost::Internal; 7]);
    }

    #[test]
    fn a_run_with_nothing_open_is_all_internal_cycles() {
        // The interrupt acknowledge cycle: seven T-states with no transfer at all.
        let mut cycle = MachineCycle::new();
        assert_eq!(run(&mut cycle, 0x3F00, 7), vec![TickCost::Internal; 7]);
    }

    #[test]
    fn the_documented_loss_is_exactly_one_contention_point() {
        // `INC (HL)`: read, one internal cycle at the same address, write. The internal
        // cycle is indistinguishable from an opcode fetch's fourth T-state and is dropped.
        // This test exists to pin the size of the known error, not to bless it.
        let mut cycle = MachineCycle::new();
        cycle.open_read(0x4000);
        let read = run(&mut cycle, 0x4000, 4);
        assert_eq!(read[3], TickCost::Deferred);
        cycle.open_write(0x4000);
        assert_eq!(
            run(&mut cycle, 0x4000, 3),
            vec![TickCost::Covered; 3],
            "the deferred T-state is dropped, costing one contention point"
        );
    }
}
