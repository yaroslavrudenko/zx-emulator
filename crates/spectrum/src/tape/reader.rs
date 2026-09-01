//! The cursor [`tzx`](super::tzx) consumes through, so that nothing in it indexes a slice.
//!
//! # Why this module exists
//!
//! The same reason [`crate::snapshot`]'s reader does, and `docs/M6.md` Decision 6 states it
//! once for both: this crate builds with `panic = "abort"` in release, so a panic on a hostile
//! file is not a recoverable error — it kills the process, and `catch_unwind` is not available
//! as a backstop. A `.tzx` is a file of attacker-controlled block lengths, counts and jump
//! offsets. The requirement is therefore not *"do not panic on the inputs we tested"*; it is
//! **remove the constructs that can panic**.
//!
//! With `unsafe_code = "forbid"` a hostile file can reach exactly three panic sources in safe
//! Rust — slice indexing, arithmetic overflow, and an explicit `panic!`/`unwrap`/`expect`. The
//! property that closes the first is about **totality**, not about routing:
//!
//! > **`tape/` contains no indexing expression, and no panicking slice call.** Every byte a file
//! > hands over is taken with a total, `Option`-returning slice API — `split_first`,
//! > `split_first_chunk`, `split_at_checked` — so a length the file got wrong is a `None` to
//! > handle rather than a bounds check to lose.
//!
//! **That is not the same as saying every byte arrives through [`Reader`], and the sentence here
//! used to say exactly that.** It was false in three places at once: [`tap`](super::tap) uses no
//! `Reader` at all and slices three times directly, [`tzx`](super::tzx) splits a flag off a block
//! body without one, and `Reader` itself slices twice rather than once — [`Reader::u8`] is a
//! `split_first`. Safety survived all three, because every one of those calls is total; the
//! *argument* for safety did not, and an argument a reader trusts is one he stops checking behind.
//!
//! So the claim is now the one that holds, and both halves of it are tests rather than sentences:
//! `crate::tape::tests::there_is_no_indexing_anywhere_in_the_tape_module` scans every production
//! source here and asserts no `a[i]` exists — with its own positive **and** negative cases, so it
//! cannot be a scanner that finds nothing while asserting nothing is there — and
//! `nothing_in_the_tape_module_can_panic_on_purpose` forbids the panicking slice calls that are
//! not index expressions and would otherwise slip past it, `split_at` and `copy_from_slice` among
//! them.
//!
//! # What [`Reader`] is actually for, then
//!
//! Not totality — the callers can get that from `std` directly, and `tap` does. It is for
//! **offsets**: `.tzx` reads about thirty fields across twenty block types, each of which must
//! report the absolute file position it failed at, and threading that by hand through thirty call
//! sites is where the arithmetic goes wrong. `tap` reads three fields and needs no cursor to keep
//! count of them, which is why it does not have one.
//!
//! # Why it is not [`crate::snapshot`]'s reader
//!
//! Because that one returns [`snapshot::Error`](crate::snapshot::Error) from every method, and
//! a malformed tape is not a malformed snapshot — `docs/M6.md` is explicit that the two error
//! types stay separate. Sharing the cursor would mean making it generic over its error, which
//! is a refactor of a module this milestone does not otherwise touch, for one caller's benefit.
//! The two are also not the same cursor: this one needs the 24- and 32-bit lengths `.tzx` uses
//! and needs no `peek`, `finish` or sub-reader.
//!
//! If a third parser ever appears, that is the point at which one generic cursor pays for
//! itself. It does not pay for itself at two.

use super::Error;

/// A forward-only cursor over an untrusted byte string.
///
/// Every method that can run out of input returns [`Error::Truncated`] naming the **absolute**
/// file offset it stopped at, which is why [`Reader::at`] exists: a reader over one block's
/// body still reports offsets a reader of the format description can look up.
pub(super) struct Reader<'a> {
    /// What has not been consumed yet. Shrinks from the front; never re-indexed.
    remaining: &'a [u8],
    /// Absolute offset of the next unread byte within the file, for error messages.
    consumed: usize,
}

impl<'a> Reader<'a> {
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

    /// Everything not yet consumed, without consuming it.
    ///
    /// What makes a block's length measurable before the block is taken: a scan reads a
    /// block's prefix fields from a throwaway cursor over this, learns the body's length, and
    /// then takes the whole body from the real cursor in one piece.
    pub(super) const fn rest(&self) -> &'a [u8] {
        self.remaining
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
                // INVARIANT: `consumed` counts bytes of a slice that already exists, so it is
                // bounded by `isize::MAX` and cannot overflow under `overflow-checks = true`.
                self.consumed += 1;
                Ok(byte)
            }
            None => Err(self.truncated(1)),
        }
    }

    /// The next two bytes as a little-endian word.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] if fewer than two bytes remain.
    pub(super) fn u16_le(&mut self) -> Result<u16, Error> {
        let low = self.u8()?;
        let high = self.u8()?;
        Ok(u16::from_le_bytes([low, high]))
    }

    /// The next **three** bytes as a little-endian length.
    ///
    /// `.tzx` writes the length of a turbo, pure-data or direct-recording block's payload as a
    /// `BYTE[3]`, which is the one width neither format's other fields use. Widened to `u32`
    /// on the way out, so nothing downstream can narrow it by accident.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] if fewer than three bytes remain.
    pub(super) fn u24_le(&mut self) -> Result<u32, Error> {
        let low = self.u8()?;
        let middle = self.u8()?;
        let high = self.u8()?;
        Ok(u32::from_le_bytes([low, middle, high, 0]))
    }

    /// The next four bytes as a little-endian long word.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] if fewer than four bytes remain.
    pub(super) fn u32_le(&mut self) -> Result<u32, Error> {
        let low = self.u8()?;
        let second = self.u8()?;
        let third = self.u8()?;
        let high = self.u8()?;
        Ok(u32::from_le_bytes([low, second, third, high]))
    }

    /// The next `count` bytes.
    ///
    /// `split_at_checked` rather than `split_at`: the `count` is a length the file chose, and the
    /// panicking form of this call is the one construct that would turn a wrong length into a
    /// dead process. It is one of the names the module's panic gate forbids by spelling.
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

    /// The error for wanting `needed` bytes and not having them.
    const fn truncated(&self, needed: usize) -> Error {
        Error::Truncated {
            offset: self.consumed,
            needed,
            available: self.remaining.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reader_hands_back_the_bytes_in_order() {
        let mut reader = Reader::at(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07], 0);
        assert_eq!(reader.u8(), Ok(0x01));
        assert_eq!(reader.u16_le(), Ok(0x0302));
        assert_eq!(reader.u24_le(), Ok(0x0006_0504));
        assert_eq!(reader.take(1), Ok(&[0x07][..]));
        assert!(reader.is_empty());
    }

    #[test]
    fn the_wide_lengths_are_little_endian_and_do_not_sign_extend() {
        // The three-byte length is the field a `.tzx` turbo block sizes its payload with, and
        // reading it big-endian is the classic way to misparse one. `0xFF 0xFF 0x7F` is
        // asymmetric, so the two readings differ.
        let mut reader = Reader::at(&[0xFF, 0xFF, 0x7F], 0);
        assert_eq!(reader.u24_le(), Ok(0x007F_FFFF));

        // ...and the top byte of a 24-bit length is never a sign bit.
        let mut reader = Reader::at(&[0x00, 0x00, 0xFF], 0);
        assert_eq!(reader.u24_le(), Ok(0x00FF_0000));

        let mut reader = Reader::at(&[0x01, 0x02, 0x03, 0x04], 0);
        assert_eq!(reader.u32_le(), Ok(0x0403_0201));
    }

    #[test]
    fn every_read_of_an_empty_reader_is_truncated_rather_than_a_panic() {
        // The whole point of the type. Each of these is an indexing panic in the shape this
        // module refuses to write, and each is a `Result` here.
        let mut reader = Reader::at(&[], 0);
        assert!(reader.u8().is_err());
        assert!(reader.u16_le().is_err());
        assert!(reader.u24_le().is_err());
        assert!(reader.u32_le().is_err());
        assert!(reader.take(1).is_err());
        assert!(reader.take(usize::MAX).is_err());
        assert!(reader.skip(9999).is_err());
    }

    #[test]
    fn a_wide_field_cut_short_names_the_byte_that_was_missing() {
        let mut reader = Reader::at(&[0xAA, 0xBB], 0);
        assert_eq!(
            reader.u24_le(),
            Err(Error::Truncated {
                offset: 2,
                needed: 1,
                available: 0
            }),
            "two bytes were there; the offset must name the third that was not"
        );
    }

    #[test]
    fn a_take_that_does_not_fit_leaves_the_cursor_where_it_was() {
        // So the error's offset names the start of the field that did not fit, not the end of
        // the file — which is what makes the message something a reader of the format
        // description can act on.
        let mut reader = Reader::at(&[0x01, 0x02, 0x03], 0);
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
    fn a_reader_over_a_block_body_reports_absolute_offsets() {
        let mut reader = Reader::at(&[0x00], 0x1234);
        assert_eq!(reader.offset(), 0x1234);
        assert_eq!(reader.u8(), Ok(0x00));
        assert_eq!(
            reader.u8(),
            Err(Error::Truncated {
                offset: 0x1235,
                needed: 1,
                available: 0
            })
        );
    }

    #[test]
    fn rest_shows_what_is_left_without_consuming_it() {
        // What the block scan measures a body's length from before taking it.
        let mut reader = Reader::at(&[0x01, 0x02, 0x03], 0);
        assert_eq!(reader.u8(), Ok(0x01));
        assert_eq!(reader.rest(), &[0x02, 0x03]);
        assert_eq!(reader.offset(), 1, "looking is not consuming");
        assert_eq!(reader.rest(), &[0x02, 0x03], "and it is idempotent");
    }

    #[test]
    fn a_zero_length_take_is_legal_and_moves_nothing() {
        let mut reader = Reader::at(&[], 5);
        assert_eq!(reader.take(0), Ok(&[][..]));
        assert_eq!(reader.offset(), 5);
    }
}
