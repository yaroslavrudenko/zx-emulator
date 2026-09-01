//! The one number this crate's correctness turns on, against literals.
//!
//! # Why a three-line function gets its own gate
//!
//! `docs/M8.md` Decision 4 lists the rules it holds non-negotiable for the download route, and
//! one of them is about a failure that reports success:
//!
//! > *"`gl.js`'s `add_missing_functions_stabs` replaces any import the JS does not provide with
//! > a stub that logs a warning to the console. So **a download whose shim was not deployed
//! > produces a page where `F2` reports success and nothing is saved.** That is a gate that
//! > grades less than it appears to, in the product, and the mitigation is that the Rust side
//! > must not report `saved …` on the strength of having made the call — the JS must return a
//! > value the Rust side checks."*
//!
//! The stub returns JavaScript `undefined`, which crosses the wasm ABI as **0**. So the whole
//! mitigation reduces to one property — *zero is not success* — and that property is a literal
//! comparison against a literal, which is exactly the shape this repository trusts.
//!
//! **What it does not do**, stated because the gate is cheap and the temptation to over-read it
//! is not: it does not prove a shim exists, does not prove one that exists works, and does not
//! prove a file reaches anybody's disk. It proves that if the far side says anything other than
//! *"started"* — including saying nothing at all — the caller is told no.

use page::{Handoff, handoff};

#[test]
fn a_stub_that_returns_nothing_is_a_refusal_and_not_a_success() {
    // The load-bearing row. `undefined` from `add_missing_functions_stabs`' replacement stub
    // arrives as 0, so this is the *deployed-without-the-shim* case, which is the one that
    // would otherwise print "saved snapshot-1.z80" over a page that saved nothing.
    assert_eq!(handoff(0), Handoff::Refused, "zero must never mean success");
}

#[test]
fn one_is_the_only_code_that_means_a_download_started() {
    assert_eq!(handoff(1), Handoff::Started);
}

#[test]
fn every_other_code_is_a_refusal() {
    // A positive control in the sense `docs/STATUS.md` means: if `handoff` were rewritten to
    // return `Started` unconditionally, the two tests above would still have `handoff(1)` to
    // pass on and only `handoff(0)` to fail on. This one fails on six more values, so a
    // constant-`Started` implementation cannot survive by accident, and a mistyped comparison
    // — `>=`, `!=`, a sign slip — is caught by the negative and the large values rather than
    // only by zero.
    for code in [-1, 2, 3, 255, -255, i32::MIN, i32::MAX] {
        assert_eq!(
            handoff(code),
            Handoff::Refused,
            "{code} is not the success code and must not be read as one",
        );
    }
}

#[test]
fn the_success_code_is_not_the_stub_code() {
    // Structural, and it is the assertion whose failure means "I was not looking at the
    // thing": the entire mitigation is that these two differ. Written as a comparison of the
    // function's own answers rather than of the constant, because the constant is private and
    // a test that reached for it would be grading the definition against itself.
    assert_ne!(handoff(0), handoff(1));
}
