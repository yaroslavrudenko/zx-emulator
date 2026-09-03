//! Which machine this is — the one discriminator everything model-dependent derives from.
//!
//! # Why a discriminator exists at all, when `docs/M7.md` says it does not need to
//!
//! `M7.md` Decision 1's central finding is that **a 48K's memory map is exactly what paging
//! port value `0x20` derives, and its inability to page is exactly the lock bit already being
//! set** — so `Memory` needs no model check to absorb a write to `0x7FFD`, and it does not have
//! one. That finding is load-bearing and it holds: `Memory::write_paging_port`
//! returns early on the lock bit and asks nothing about the machine.
//!
//! What the port byte cannot derive is the rest of the machine. Three things differ between
//! the two models and **none of them is a function of `0x7FFD`**:
//!
//! - which banks the ULA contends — bank 5 against banks 1, 3, 5 and 7;
//! - the frame's geometry — [`Timing`], and 69888 T-states against 70908;
//! - which banks exist at all, which is what a snapshot's bank set is checked against.
//!
//! A 48K standing at `paging_port == 0x00` is not a 128, and a 128 that has just written `0x20`
//! is not a 48K, so the port byte is the wrong thing to ask. The alternative to this enum is
//! three unrelated fields that can disagree with each other; [`Model`] is the single source they
//! are all derived from, which is the same shape `Memory` uses for the port byte itself.
//!
//! # What is deliberately **not** here
//!
//! `paging_port_at_reset` appears in `M7.md`'s sketch as a `Memory` field. It is a method on
//! [`Model`] instead, because it is a function of the model and storing it as well would be two
//! representations of one datum — the defect `M7.md` Decision 2 rejects two paragraphs later
//! under *"One representation, derived once"*. The same argument applies to it.

use crate::memory::BANK_COUNT;
use crate::timing::Timing;

/// A machine this crate can be.
///
/// `#[non_exhaustive]` because the +2 grey is a ROM pair rather than a variant (`M7.md`
/// Decision 10) but the +2A/+3 genuinely is one, and adding it must not be a breaking change
/// for a downstream `match`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Model {
    /// The 48K: one ROM, three RAM banks, paging locked from power-on.
    Spectrum48K,
    /// The 128: two ROMs, eight RAM banks, `0x7FFD` live until something sets its lock bit.
    Spectrum128,
}

impl Model {
    /// The `0x7FFD` value this machine powers on and resets to.
    ///
    /// `M7.md` Decision 1's equation, and it is the whole reason a 48K needs no special case
    /// anywhere in [`crate::memory`]:
    ///
    /// ```text
    ///   0x20 = 0b0010_0000
    ///          bits 0-2 = 000  ->  bank 0 at 0xC000
    ///          bit  3   = 0    ->  screen is bank 5
    ///          bit  4   = 0    ->  ROM page 0
    ///          bit  5   = 1    ->  paging locked, which is what a 48K cannot do
    /// ```
    ///
    /// A 128 powers on at `0x00`: bank 0, screen bank 5, the 128 editor ROM, and paging live.
    pub(crate) const fn paging_port_at_reset(self) -> u8 {
        match self {
            Self::Spectrum48K => 0x20,
            Self::Spectrum128 => 0x00,
        }
    }

    /// Which RAM banks the ULA contends, indexed by bank number.
    ///
    /// **This is a per-bank property and not an address range**, which is why
    /// [`crate::memory::Memory::is_contended`] has consulted the slot map since M5 rather than
    /// the address. On a 48K the two are indistinguishable because the one contended bank is
    /// nailed to `0x4000`; on a 128 a contended bank can be paged into `0xC000` and the
    /// distinction becomes the difference between a correct machine and a demo that tears.
    ///
    /// **Evidence, and it is not the same for the two rows.** The 48K's single contended bank
    /// is established: `crates/spectrum/tests/timing_oracle.rs` grades this machine against
    /// hardware-measured T-state counts and would not survive the set being wrong.
    ///
    /// The 128's four are **derived**, from the World of Spectrum *128K Technical Information*
    /// reference — *"Memory banks 1,3,5 and 7 are contended"* — with the Sinclair Wiki as a
    /// descendant rather than a second witness. Four independent implementations agree (Fuse,
    /// MAME, rustzx, ZEsarUX), and **Fuse reaches it by construction rather than by constant**:
    /// `for( i = 0; i < 16; i++ ) memory_ram_set_16k_contention( i, i & 1 ? contention : 0 )` —
    /// set per *page*, so contention follows the bank into whichever slot it is paged. That is
    /// the same shape as this function, and it is the part worth noticing: it corroborates the
    /// *mechanism* and not merely the set.
    ///
    /// # The official service manual says **4–7**, and it is wrong about the silicon
    ///
    /// The one place in M7 where a **primary** hardware document contradicts what is
    /// implemented, so it is recorded rather than quietly overridden. The Sinclair *Servicing
    /// Manual for Spectrum 128* §4.11 describes the contended path as selecting *"a page in the
    /// range 4-7"*. The Sinclair Wiki accounts for the discrepancy as a manufacturing defect:
    ///
    /// > *"Due to a bug either in the 128's HAL10H8 chip or in the PCB, memory banks 1, 3, 5 and
    /// > 7 are contended (and the rest uncontended) as opposed to 4, 5, 6 and 7 as documented in
    /// > the service manual… The paging scheme documented in the manual would have been
    /// > implemented as was (presumably) originally intended had the B0 and B2 inputs to the
    /// > HAL10H8 been reversed."*
    ///
    /// The +2A/+3 — a different chip, and out of scope — does contend 4–7, consistent with the
    /// manual describing an intended design the toastrack's silicon did not implement. **1, 3,
    /// 5, 7 is what the hardware does.** Note that the two rules agree on 5 and 7, the screen
    /// banks, so only 1 and 3 against 4 and 6 are actually at stake — which is exactly why a
    /// model that took the manual's word would pass anything that only exercises the screen.
    pub(crate) const fn contended_banks(self) -> [bool; BANK_COUNT] {
        match self {
            // Only bank 5, the one the screen lives in and the only one a 48K reaches through
            // a contended slot. Marking the others would be a claim about hardware it lacks.
            Self::Spectrum48K => [false, false, false, false, false, true, false, false],
            // 1, 3, 5, 7 — the odd banks, in whichever slot they are paged into.
            Self::Spectrum128 => [false, true, false, true, false, true, false, true],
        }
    }

    /// Every RAM bank this machine has, ascending.
    ///
    /// **Not every bank it can currently address**, and the difference is the whole reason
    /// [`crate::memory::Memory::bank`] exists. A 48K's three banks happen to be exactly the
    /// three its slot map exposes, which is why M6 could close the dropped-bank seam by
    /// comparing a snapshot's bank set against the slot map. On a 128 that premise dissolves:
    /// five of the eight banks have no address at any given moment and are still part of the
    /// machine's state. `M7.md` Decision 7 names this as easy to miss.
    ///
    /// The 48K's three are asserted against its slot map in
    /// `the_forty_eight_ks_bank_list_is_what_its_slot_map_exposes`, so the two cannot drift.
    pub(crate) const fn banks(self) -> &'static [u8] {
        match self {
            Self::Spectrum48K => &[0, 2, 5],
            Self::Spectrum128 => &[0, 1, 2, 3, 4, 5, 6, 7],
        }
    }

    /// Whether this machine has an AY-3-8912.
    ///
    /// The 128 does; the 48K does not, and the difference is a chip that is physically absent
    /// rather than a chip that is idle. That is why [`crate::audio`] holds an `Option<Ay>`
    /// rather than an `Ay` and a flag: a 48K's `0xBFFD` write reaches nothing, its `0xFFFD`
    /// read floats, and [`crate::Spectrum::ay`] says so instead of returning a chip the
    /// machine does not contain.
    ///
    /// The +2 (grey) has one and is not a variant here — it is a ROM pair, `M7.md` Decision
    /// 10 — so no row is missing.
    pub(crate) const fn has_ay(self) -> bool {
        match self {
            Self::Spectrum48K => false,
            Self::Spectrum128 => true,
        }
    }

    /// This machine's frame geometry.
    pub(crate) const fn timing(self) -> Timing {
        match self {
            Self::Spectrum48K => Timing::SPECTRUM_48K,
            Self::Spectrum128 => Timing::SPECTRUM_128,
        }
    }
}

impl core::fmt::Display for Model {
    /// Spelled the way the machines are spelled, because this reaches a user through
    /// [`crate::ModelMismatch`]'s message.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Spectrum48K => "48K",
            Self::Spectrum128 => "128",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Memory, PAGE_SIZE, Slot};

    const MODELS: [Model; 2] = [Model::Spectrum48K, Model::Spectrum128];

    #[test]
    fn the_forty_eight_ks_reset_port_derives_its_whole_map() {
        // `M7.md` Decision 1's equation, bit by bit, as an assertion rather than a paragraph.
        // The map itself is checked against `SPECTRUM_48K_SLOTS` at compile time in
        // `crate::memory`; this checks the *reading* of the byte that the compile-time
        // assertion depends on.
        let port = Model::Spectrum48K.paging_port_at_reset();
        assert_eq!(port & 0x07, 0, "bank 0 at 0xC000");
        assert_eq!(port & 0x08, 0, "screen is bank 5");
        assert_eq!(port & 0x10, 0, "ROM page 0");
        assert_ne!(port & 0x20, 0, "paging locked — a 48K cannot page");
    }

    #[test]
    fn a_128_powers_on_unlocked_and_a_48k_does_not() {
        // The single fact that lets `write_paging_port` carry no model check at all.
        assert_eq!(Model::Spectrum128.paging_port_at_reset(), 0x00);
        assert_ne!(
            Model::Spectrum48K.paging_port_at_reset() & 0x20,
            0,
            "if this ever became unlocked, a 48K would start paging"
        );
    }

    #[test]
    fn the_contended_sets_are_the_published_ones_and_the_48k_is_a_subset() {
        let forty_eight = Model::Spectrum48K.contended_banks();
        let one_two_eight = Model::Spectrum128.contended_banks();

        let contended = |set: [bool; BANK_COUNT]| -> Vec<usize> {
            set.into_iter()
                .enumerate()
                .filter_map(|(bank, yes)| yes.then_some(bank))
                .collect()
        };
        assert_eq!(contended(forty_eight), vec![5]);
        assert_eq!(contended(one_two_eight), vec![1, 3, 5, 7]);

        // Not decoration: a 128 that failed to contend bank 5 would still pass every gate
        // that only ever reaches bank 5 through slot 1, because a 48K's map puts it there.
        for (bank, &is_contended) in forty_eight.iter().enumerate() {
            assert!(
                !is_contended || one_two_eight[bank],
                "bank {bank} is contended on a 48K and not on a 128"
            );
        }
    }

    #[test]
    fn the_contended_banks_are_exactly_the_odd_ones_on_a_128() {
        // Stated as the rule as well as the list. The two sources agree that the set is
        // 1, 3, 5, 7; a transcription that dropped one would still look plausible as a list
        // and would not survive being read as "the odd banks".
        for (bank, is_contended) in Model::Spectrum128.contended_banks().into_iter().enumerate() {
            assert_eq!(is_contended, bank % 2 == 1, "bank {bank}");
        }
    }

    #[test]
    fn the_forty_eight_ks_bank_list_is_what_its_slot_map_exposes() {
        // The two halves of "which banks does this machine have" must not drift. On a 48K
        // they coincide, and asserting it here is what makes `Model::banks` the safe
        // replacement for M6's `exposed_banks`-based guard when the 128 breaks the
        // coincidence.
        let memory = Memory::spectrum_48k(&[0; PAGE_SIZE]).expect("a page-sized ROM");
        let mut exposed: Vec<u8> = memory
            .slots()
            .into_iter()
            .filter_map(|slot| match slot {
                Slot::Bank(bank) => Some(bank.get()),
                Slot::Rom(_) => None,
            })
            .collect();
        exposed.sort_unstable();
        assert_eq!(Model::Spectrum48K.banks(), exposed.as_slice());
    }

    #[test]
    fn a_128_has_every_bank_and_a_48k_does_not() {
        assert_eq!(Model::Spectrum128.banks().len(), BANK_COUNT);
        assert_eq!(Model::Spectrum48K.banks().len(), 3);
        // The five a 48K lacks are exactly the ones a 128 snapshot carries and a 48K cannot
        // hold — `Spectrum::restore`'s refusal is about these.
        let missing: Vec<u8> = Model::Spectrum128
            .banks()
            .iter()
            .copied()
            .filter(|bank| !Model::Spectrum48K.banks().contains(bank))
            .collect();
        assert_eq!(missing, vec![1, 3, 4, 6, 7]);
    }

    #[test]
    fn every_bank_list_is_ascending_and_in_range() {
        // **Neither of this list's two real consumers depends on the order**, and the comment
        // here used to say both did — naming "the snapshot writer and the applier", where the
        // applier in fact iterates `Snapshot::banks()` and the writer's use is a presence check.
        // The order is asserted anyway, and the reason is the honest one: `snapshot::pages_of`
        // *is* order-dependent, it feeds the `.z80` writer's page blocks, and its 48K order is
        // the opposite of this one — so a reader comparing the two needs each to be pinned
        // rather than incidental. Ascending here, address order there, both on purpose.
        for model in MODELS {
            let banks = model.banks();
            assert!(banks.is_sorted(), "{model} lists its banks out of order");
            assert!(banks.iter().all(|&bank| usize::from(bank) < BANK_COUNT));
        }
    }

    #[test]
    fn the_two_models_have_different_frame_geometry() {
        // The negative that `timing_oracle.rs` established by mutation: a single shared
        // contention constant across both models is refuted, so these must not coincide.
        let forty_eight = Model::Spectrum48K.timing();
        let one_two_eight = Model::Spectrum128.timing();
        assert_ne!(forty_eight.frame_t_states(), one_two_eight.frame_t_states());
        assert_ne!(
            forty_eight.first_contended_t_state(),
            one_two_eight.first_contended_t_state()
        );
    }

    #[test]
    fn only_the_128_has_a_sound_chip() {
        // The one fact that decides whether `0xFFFD` and `0xBFFD` decode at all, and whether
        // `Spectrum::ay` has anything to return. A 48K with a chip would answer those ports
        // and would be a machine nobody ever built.
        assert!(!Model::Spectrum48K.has_ay());
        assert!(Model::Spectrum128.has_ay());
    }

    #[test]
    fn a_model_names_itself_in_a_way_a_user_can_read() {
        assert_eq!(Model::Spectrum48K.to_string(), "48K");
        assert_eq!(Model::Spectrum128.to_string(), "128");
    }
}
