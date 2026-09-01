//! The two cursors everything in [`crate::snapshot`] consumes and produces through.
//!
//! # Why this module exists at all
//!
//! A snapshot parser reads attacker-controlled lengths, counts and page numbers, and this
//! crate builds with `panic = "abort"` in release — so a panic is not a recoverable error,
//! it kills the process, and `catch_unwind` is not available as a backstop. `docs/M6.md`
//! Decision 6 therefore sets the requirement as *remove the constructs that can panic*
//! rather than *do not panic on the inputs we tested*.
//!
//! With `unsafe_code = "forbid"` there are exactly three panic sources in safe Rust a
//! hostile file can reach: slice indexing, arithmetic overflow, and an explicit
//! `panic!`/`unwrap`/`expect`. This module closes the first one **by construction**:
//!
//! > **`snapshot/` contains no indexing expression at all.** Every byte that enters or
//! > leaves a buffer goes through [`Reader`] or [`Writer`], and every slice access in the
//! > whole module is **total** — `split_first`, `first`, `get`, `split_at_checked` and
//! > `split_at_mut_checked` all return an `Option`, so the failing case is a value the
//! > caller must handle rather than an abort.
//!
//! That is a claim about the source, so it is a test rather than a sentence:
//! `crate::snapshot::tests::there_is_no_indexing_anywhere_in_the_snapshot_module` scans
//! this module's own source and asserts the count is zero, and the scanner it uses has its
//! own failing cases so it cannot be a tautology.
//!
//! # Why the writer lives here too
//!
//! Because the property above is about *slicing*, not about reading, and a decompressor
//! that fills a fixed page needs a bounded destination for exactly the same reason a parser
//! needs a bounded source. Keeping both here means the count the gate makes is a count over
//! one file.

use super::Error;

/// A forward-only cursor over an untrusted byte string.
///
/// Every method that can run out of input returns [`Error::Truncated`] naming the absolute
/// file offset it stopped at, which is why [`Reader::at`] exists: a sub-reader over the
/// `.z80` additional header still reports offsets a reader of `docs/M6.md` or of the format
/// description can look up.
pub(super) struct Reader<'a> {
    /// What has not been consumed yet. Shrinks from the front; never re-indexed.
    remaining: &'a [u8],
    /// Absolute offset of `remaining[0]` within the file, for error messages.
    consumed: usize,
}

impl<'a> Reader<'a> {
    /// A cursor over `bytes`, whose first byte is at file offset zero.
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self::at(bytes, 0)
    }

    /// A cursor over `bytes`, whose first byte is at file offset `offset`.
    pub(super) const fn at(bytes: &'a [u8], offset: usize) -> Self {
        Self {
            remaining: bytes,
            consumed: offset,
        }
    }

    /// The absolute file offset of the next unread byte.
    pub(super) const fn offset(&self) -> usize {
        self.consumed
    }

    /// Whether every byte has been consumed.
    pub(super) const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    /// The next byte without consuming it, or `None` at the end of the input.
    pub(super) fn peek(&self) -> Option<u8> {
        self.remaining.first().copied()
    }

    /// The next byte.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] at the end of the input.
    pub(super) fn u8(&mut self) -> Result<u8, Error> {
        match self.remaining.split_first() {
            Some((&byte, tail)) => {
                self.remaining = tail;
                // INVARIANT: `consumed` counts bytes of a slice that already exists, so it
                // is bounded by `isize::MAX` and cannot overflow under `overflow-checks`.
                self.consumed += 1;
                Ok(byte)
            }
            None => Err(self.truncated(1)),
        }
    }

    /// The next two bytes as a little-endian word.
    ///
    /// Both formats store every 16-bit quantity this way, including the ones whose halves
    /// the header stores apart — `A` and `F` are separate bytes and are **not** read here.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] if fewer than two bytes remain. A word cut in half consumes its
    /// first byte before failing, so the reported offset names the byte that was missing
    /// rather than the one that was present.
    pub(super) fn u16_le(&mut self) -> Result<u16, Error> {
        let low = self.u8()?;
        let high = self.u8()?;
        Ok(u16::from_le_bytes([low, high]))
    }

    /// The next `count` bytes.
    ///
    /// **This is one of the module's two slicing sites**, and the only one on the read side.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] if fewer than `count` bytes remain, leaving the cursor where it
    /// was so the error's `offset` names the start of the field that did not fit.
    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        match self.remaining.split_at_checked(count) {
            Some((head, tail)) => {
                self.remaining = tail;
                // INVARIANT: as `u8` above — `count <= remaining.len()` on this branch.
                self.consumed += count;
                Ok(head)
            }
            None => Err(self.truncated(count)),
        }
    }

    /// Consume `count` bytes without looking at them.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] if fewer than `count` bytes remain.
    pub(super) fn skip(&mut self, count: usize) -> Result<(), Error> {
        self.take(count).map(|_| ())
    }

    /// Assert that nothing is left.
    ///
    /// Strict rather than tolerant, and deliberately: a `.z80` with unexplained bytes after
    /// its last page is either a format we do not understand or a file we have misparsed,
    /// and both are worth saying out loud. `docs/M6.md` makes the same ruling for a short
    /// decompression, with the same escape hatch — an observed real-world file that is
    /// legitimately longer is what would change it.
    ///
    /// # Errors
    ///
    /// [`Error::TrailingBytes`] if any byte is unread.
    pub(super) fn finish(self) -> Result<(), Error> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(Error::TrailingBytes {
                offset: self.consumed,
                extra: self.remaining.len(),
            })
        }
    }

    /// The error for wanting `needed` bytes and not having them.
    const fn truncated(&self, needed: usize) -> Error {
        Error::Truncated {
            offset: self.consumed,
            needed,
            available: self.remaining.len(),
        }
    }
}

/// The destination is full: whatever asked for room cannot have it.
///
/// Deliberately not an [`Error`]. A [`Writer`] does not know which page it is filling or
/// where that page's block started in the file, and inventing an offset here would put a
/// wrong number in a message a user is meant to act on. The caller, which knows both,
/// attaches them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Full;

/// A forward-only cursor over a fixed-size destination.
///
/// The bound is the point. `docs/M6.md` Decision 6: **no allocation is ever sized from the
/// file.** A decompressor fills a page-sized buffer through this, so a run-length count of
/// 60000 is a [`Full`] and not a `Vec` growing to whatever the file asked for.
pub(super) struct Writer<'a> {
    /// Room not yet written. Shrinks from the front; never re-indexed.
    remaining: &'a mut [u8],
    /// Bytes written so far, which is what an under-filled page reports.
    written: usize,
}

impl<'a> Writer<'a> {
    /// A cursor that fills `destination` and stops there.
    pub(super) fn new(destination: &'a mut [u8]) -> Self {
        Self {
            remaining: destination,
            written: 0,
        }
    }

    /// Bytes written so far.
    pub(super) const fn written(&self) -> usize {
        self.written
    }

    /// Bytes the destination holds in total — what an under-filled page reports against.
    pub(super) const fn capacity(&self) -> usize {
        self.written + self.remaining.len()
    }

    /// Whether the destination is exactly full.
    pub(super) const fn is_full(&self) -> bool {
        self.remaining.is_empty()
    }

    /// `count` bytes of room.
    ///
    /// **This is the module's other slicing site**, and the only one on the write side.
    ///
    /// # Errors
    ///
    /// [`Full`] if fewer than `count` bytes of room remain, leaving the cursor untouched.
    fn room(&mut self, count: usize) -> Result<&mut [u8], Full> {
        // Checked before the split rather than after, so a refused request leaves the cursor
        // exactly where it was: a `split_at_mut_checked` that fails has already consumed the
        // borrow, and there would be no way to put it back.
        if count > self.remaining.len() {
            return Err(Full);
        }
        match core::mem::take(&mut self.remaining).split_at_mut_checked(count) {
            Some((head, tail)) => {
                self.remaining = tail;
                // INVARIANT: as `Reader::take` — bounded by the destination's length.
                self.written += count;
                Ok(head)
            }
            // Unreachable, because `count` was just checked against the length. The checked
            // split is used anyway rather than the panicking one, so the impossibility does
            // not have to be trusted; and the verdict on this branch is the same one the
            // guard above gives.
            None => Err(Full),
        }
    }

    /// Write one byte.
    ///
    /// # Errors
    ///
    /// [`Full`] if the destination has no room left.
    pub(super) fn push(&mut self, byte: u8) -> Result<(), Full> {
        self.fill(byte, 1)
    }

    /// Write `byte` `count` times.
    ///
    /// # Errors
    ///
    /// [`Full`] if fewer than `count` bytes of room remain. A run that does not fit writes
    /// **nothing**, rather than as much of itself as fits: a partially applied run is a
    /// silently wrong page, and a refused one is an error the caller reports.
    pub(super) fn fill(&mut self, byte: u8, count: usize) -> Result<(), Full> {
        self.room(count)?.fill(byte);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reader_hands_back_the_bytes_in_order() {
        let mut reader = Reader::new(&[0x01, 0x02, 0x03, 0x04, 0x05]);
        assert_eq!(reader.u8(), Ok(0x01));
        assert_eq!(reader.u16_le(), Ok(0x0302));
        assert_eq!(reader.take(2), Ok(&[0x04, 0x05][..]));
        assert!(reader.is_empty());
        assert_eq!(reader.finish(), Ok(()));
    }

    #[test]
    fn every_read_of_an_empty_reader_is_truncated_rather_than_a_panic() {
        // The whole point of the type. Each of these is an indexing panic in the shape this
        // module refuses to write, and each is a `Result` here.
        let mut reader = Reader::new(&[]);
        assert!(reader.u8().is_err());
        assert!(reader.u16_le().is_err());
        assert!(reader.take(1).is_err());
        assert!(reader.take(usize::MAX).is_err());
        assert!(reader.skip(9999).is_err());
        assert_eq!(reader.peek(), None);
    }

    #[test]
    fn a_word_cut_in_half_is_truncated_and_names_the_missing_byte() {
        let mut reader = Reader::new(&[0xAA]);
        assert_eq!(
            reader.u16_le(),
            Err(Error::Truncated {
                offset: 1,
                needed: 1,
                available: 0
            }),
            "the low byte was there; the offset must name the high byte that was not"
        );
    }

    #[test]
    fn a_take_that_does_not_fit_leaves_the_cursor_where_it_was() {
        // So the error's offset names the start of the field that did not fit, not the end
        // of the file.
        let mut reader = Reader::new(&[0x01, 0x02, 0x03]);
        assert_eq!(reader.u8(), Ok(0x01));
        assert_eq!(
            reader.take(10),
            Err(Error::Truncated {
                offset: 1,
                needed: 10,
                available: 2
            })
        );
        assert_eq!(reader.offset(), 1, "a failed take must not consume");
        assert_eq!(reader.take(2), Ok(&[0x02, 0x03][..]));
    }

    #[test]
    fn a_sub_reader_reports_absolute_offsets() {
        // The `.z80` additional header is parsed as its own reader; its errors still have to
        // name offsets somebody can find in the format description.
        let mut reader = Reader::at(&[0x00], 32);
        assert_eq!(reader.offset(), 32);
        assert_eq!(reader.u8(), Ok(0x00));
        assert_eq!(
            reader.u8(),
            Err(Error::Truncated {
                offset: 33,
                needed: 1,
                available: 0
            })
        );
    }

    #[test]
    fn unread_bytes_are_a_finding_rather_than_a_shrug() {
        let mut reader = Reader::new(&[0x01, 0x02, 0x03]);
        assert_eq!(reader.u8(), Ok(0x01));
        assert_eq!(
            reader.finish(),
            Err(Error::TrailingBytes {
                offset: 1,
                extra: 2
            })
        );
    }

    #[test]
    fn a_writer_fills_and_then_refuses() {
        let mut page = [0_u8; 4];
        let mut writer = Writer::new(&mut page);
        assert_eq!(writer.push(0xAA), Ok(()));
        assert_eq!(writer.fill(0xBB, 2), Ok(()));
        assert_eq!(writer.written(), 3);
        assert!(!writer.is_full());
        assert_eq!(writer.push(0xCC), Ok(()));
        assert!(writer.is_full());
        assert_eq!(writer.push(0xDD), Err(Full));
        assert_eq!(page, [0xAA, 0xBB, 0xBB, 0xCC]);
    }

    #[test]
    fn a_run_that_does_not_fit_writes_nothing_at_all() {
        // A partially applied run is a silently wrong page. This is the difference between
        // an error a caller reports and a machine that loads and misbehaves.
        let mut page = [0_u8; 4];
        let mut writer = Writer::new(&mut page);
        assert_eq!(writer.push(0x11), Ok(()));
        assert_eq!(writer.fill(0xFF, 255), Err(Full));
        assert_eq!(writer.written(), 1, "the refused run wrote nothing");
        assert_eq!(page, [0x11, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn a_hostile_count_is_refused_rather_than_allocating_anything() {
        // `docs/M6.md` Decision 6: no allocation is ever sized from the file. The
        // destination is fixed, so a count of `usize::MAX` is one comparison.
        let mut page = [0_u8; 16];
        let mut writer = Writer::new(&mut page);
        assert_eq!(writer.fill(0xFF, usize::MAX), Err(Full));
        assert_eq!(writer.fill(0x00, 17), Err(Full));
        assert_eq!(writer.written(), 0);
        assert_eq!(
            writer.capacity(),
            16,
            "a refused write does not shrink the page"
        );
    }

    #[test]
    fn a_zero_length_operation_is_legal_and_moves_nothing() {
        // A run-length count of zero is legal in the `.z80` scheme, and it must not be a
        // special case anywhere: it writes nothing and the loop still progresses because
        // its four bytes were consumed on the read side.
        let mut page = [0_u8; 1];
        let mut writer = Writer::new(&mut page);
        assert_eq!(writer.fill(0xFF, 0), Ok(()));
        assert_eq!(writer.written(), 0);

        let mut reader = Reader::new(&[]);
        assert_eq!(reader.take(0), Ok(&[][..]));
    }
}
