//! Shared test harness — the ONE home for every fixture the integration tests use.
//!
//! Layering, innermost first. Only the last two know the `z80` crate exists:
//!
//! | module      | depends on            | purpose                                    |
//! |-------------|-----------------------|--------------------------------------------|
//! | `flags`     | —                     | the `F` bit layout (SSOT)                   |
//! | `vectors`   | —                     | FUSE corpus model + parser, and the shared corpus-absent policy |
//! | `reference` | `flags`               | independent ALU flag model + opcodes        |
//! | `report`    | `flags`, `vectors`    | field-level mismatch diagnostics            |
//! | `machine`   | `vectors`, **`z80`**  | the per-instruction test bus — the FUSE contract seam |
//! | `cpm`       | **`z80`**             | the whole-program shell the `zex` exercisers run in |
//!
//! Keeping `z80` out of the first four means `tests/fuse_format.rs` can include them
//! directly and run even while the CPU core is still being written.
//!
//! `machine` and `cpm` are separate on purpose and share no bus: one logs every T-state
//! because a 30-T-state vector asserts on the log, the other logs nothing because a run of
//! 10¹⁰ T-states cannot. `cpm`'s own module documentation carries the full comparison.

// Each integration-test binary compiles this whole tree but uses a subset of it: the
// parser binary never builds a `Machine`, the property-test binary never parses the
// corpus. Without this the unused half of the tree fails `clippy -D warnings` in every
// binary. This is the standard `tests/common` situation and the allow is permanent, so
// `#[expect]` would itself warn in the binaries where the items *are* used.
#![allow(dead_code)]

pub mod cpm;
pub mod flags;
pub mod machine;
pub mod reference;
pub mod report;
pub mod vectors;
