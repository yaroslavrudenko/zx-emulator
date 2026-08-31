//! Shared test harness — the ONE home for every fixture the integration tests use.
//!
//! Layering, innermost first. Only the last of these knows the `z80` crate exists:
//!
//! | module      | depends on            | purpose                                    |
//! |-------------|-----------------------|--------------------------------------------|
//! | `flags`     | —                     | the `F` bit layout (SSOT)                   |
//! | `vectors`   | —                     | FUSE corpus model + parser                  |
//! | `reference` | `flags`               | independent ALU flag model + opcodes        |
//! | `report`    | `flags`, `vectors`    | field-level mismatch diagnostics            |
//! | `machine`   | `vectors`, **`z80`**  | the test bus and the CPU contract seam      |
//!
//! Keeping `z80` out of the first four means `tests/fuse_format.rs` can include them
//! directly and run even while the CPU core is still being written.

// Each integration-test binary compiles this whole tree but uses a subset of it: the
// parser binary never builds a `Machine`, the property-test binary never parses the
// corpus. Without this the unused half of the tree fails `clippy -D warnings` in every
// binary. This is the standard `tests/common` situation and the allow is permanent, so
// `#[expect]` would itself warn in the binaries where the items *are* used.
#![allow(dead_code)]

pub mod flags;
pub mod machine;
pub mod reference;
pub mod report;
pub mod vectors;
