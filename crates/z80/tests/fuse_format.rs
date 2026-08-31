//! Self-tests for the harness itself: the corpus parser, the corpus's own invariants,
//! and the independent ALU reference model.
//!
//! This binary deliberately includes only the modules that do **not** depend on the `z80`
//! crate, so it compiles and runs while the CPU core is still being written. That matters
//! for more than convenience: if the only proof that the parser works lives in a binary
//! that cannot build, then "the tests pass" and "the tests never ran" look identical.
//!
//! Run it on its own with:
//! ```text
//! cargo test -p z80 --test fuse_format
//! ```

// These three modules are included by path rather than through `common/mod.rs`, which
// would drag in `common/machine.rs` and with it the `z80` crate — the dependency this
// binary exists to avoid.
//
// Each carries `allow(dead_code)` because this binary uses a subset of each module (it
// never builds a `Machine`, so `Registers::a()` and friends go untouched here while being
// load-bearing in `fuse_vectors.rs`). The allow is permanent and correct, so `#[expect]`
// would itself warn in whichever binary does use the items.
#[allow(dead_code)]
#[path = "common/flags.rs"]
mod flags;

#[allow(dead_code)]
#[path = "common/vectors.rs"]
mod vectors;

#[allow(dead_code)]
#[path = "common/reference.rs"]
mod reference;

#[allow(dead_code)]
#[path = "common/report.rs"]
mod report;

use reference::{Binary, Unary};
use vectors::{EventKind, ParseError, Prefix, Transfer};

// ---------------------------------------------------------------------------
// Corpus invariants
// ---------------------------------------------------------------------------

/// Mirrors the floor in `fuse_vectors.rs`: catches a truncated download, not drift.
const MIN_M1_VECTORS: usize = 250;

#[test]
fn corpus_parses_and_the_two_halves_align() {
    let Some(corpus) = vectors::corpus_or_skip() else {
        return;
    };

    let m1 = corpus.iter().filter(|v| v.is_m1_scope()).count();
    println!(
        "parsed {} vectors: {m1} un-prefixed (M1), {} prefixed (M2)",
        corpus.len(),
        corpus.len() - m1,
    );

    assert!(!corpus.is_empty(), "the corpus parsed to zero vectors");
    assert!(
        m1 >= MIN_M1_VECTORS,
        "only {m1} un-prefixed vectors, expected at least {MIN_M1_VECTORS}"
    );
    for vector in &corpus {
        assert!(
            !vector.name().is_empty(),
            "a vector parsed with an empty name"
        );
        assert!(
            !vector.setup.memory.is_empty(),
            "vector {:?} defines no memory, so it has no opcode to execute",
            vector.name()
        );
    }
}

/// The rule `TestBus::in_port` implements, verified against the corpus rather than
/// assumed: an unattached Z80 `IN` reads back the high half of the address bus.
///
/// This is how the harness earns the right to model ports at all without reading another
/// emulator's source — the behaviour is derivable from the expected data.
#[test]
fn unattached_in_port_returns_the_high_address_byte() {
    let Some(corpus) = vectors::corpus_or_skip() else {
        return;
    };

    let reads: Vec<_> = corpus
        .iter()
        .flat_map(|vector| vector.expected.events.iter())
        .filter(|event| event.kind == EventKind::PortRead)
        .collect();

    assert!(
        !reads.is_empty(),
        "no PR events in the corpus — the port model cannot be validated"
    );
    println!("validated the IN model against {} PR events", reads.len());

    for event in reads {
        assert_eq!(
            Some((event.addr >> 8) as u8),
            event.data,
            "PR at port {:04x} expected to read the high address byte",
            event.addr,
        );
    }
}

/// Vectors are classified by the opcode byte at `PC`, never by their name. This checks
/// the two agree across the whole corpus — a mismatch would mean the parser has put the
/// memory image or `PC` somewhere wrong.
#[test]
fn prefix_classification_agrees_with_the_vector_name() {
    let Some(corpus) = vectors::corpus_or_skip() else {
        return;
    };

    for vector in &corpus {
        let name = vector.name();
        let leading = u8::from_str_radix(&name[..2], 16)
            .unwrap_or_else(|_| panic!("vector name {name:?} does not start with two hex digits"));
        assert_eq!(
            Prefix::from_opcode(leading),
            vector.setup.prefix(),
            "vector {name:?}: its name implies one prefix, the byte at PC {:04x} implies another",
            vector.setup.registers.pc,
        );
    }
}

// ---------------------------------------------------------------------------
// Parser — golden blocks
// ---------------------------------------------------------------------------

const GOLDEN_SETUP: &str = concat!(
    "02\n",
    "5600 0001 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000\n",
    "00 00 0 0 0 0     1\n",
    "0000 02 -1\n",
    "-1\n",
);

const GOLDEN_EXPECTATION: &str = concat!(
    "02\n",
    "    0 MC 0000\n",
    "    4 MR 0000 02\n",
    "    4 MC 0001\n",
    "    7 MW 0001 56\n",
    "5600 0001 0000 0000 0000 0000 0000 0000 0000 0000 0000 0001\n",
    "00 01 0 0 0 0 7\n",
    "0001 56 -1\n",
);

#[test]
fn parser_reads_a_known_setup_block() -> Result<(), ParseError> {
    let setups = vectors::parse_setups("golden.in", GOLDEN_SETUP)?;
    let [setup] = &setups[..] else {
        panic!("expected exactly one block, got {}", setups.len());
    };

    assert_eq!("02", setup.name);
    assert_eq!(0x5600, setup.registers.af);
    assert_eq!(0x0001, setup.registers.bc);
    assert_eq!(0x0000, setup.registers.pc);
    assert_eq!(0, setup.state.i);
    assert!(!setup.state.iff1 && !setup.state.iff2 && !setup.state.halted);
    assert_eq!(0, setup.state.im);
    assert_eq!(1, setup.state.t_states, "the setup's tstates is a budget");
    assert_eq!(1, setup.memory.len());
    assert_eq!(0x0000, setup.memory[0].start);
    assert_eq!(vec![0x02], setup.memory[0].bytes);

    assert_eq!(0x02, setup.opcode_at_pc());
    assert_eq!(None, setup.prefix(), "0x02 is LD (BC),A — un-prefixed");
    Ok(())
}

#[test]
fn parser_reads_a_known_expectation_block() -> Result<(), ParseError> {
    let expectations = vectors::parse_expectations("golden.expected", GOLDEN_EXPECTATION)?;
    let [expected] = &expectations[..] else {
        panic!("expected exactly one block, got {}", expectations.len());
    };

    assert_eq!("02", expected.name);
    assert_eq!(4, expected.events.len());

    let first = expected.events[0];
    assert_eq!(EventKind::MemoryContend, first.kind);
    assert_eq!(0, first.at_t_state);
    assert_eq!(0x0000, first.addr);
    assert_eq!(None, first.data, "contention events carry no data byte");

    let fetch = expected.events[1];
    assert_eq!(EventKind::MemoryRead, fetch.kind);
    assert_eq!(4, fetch.at_t_state);
    assert_eq!(Some(0x02), fetch.data);

    assert_eq!(0x0001, expected.registers.pc);
    assert_eq!(1, expected.state.r, "R increments on the M1 fetch");
    assert_eq!(
        7, expected.state.t_states,
        "the expectation's tstates is the total"
    );
    assert_eq!(1, expected.memory.len());
    assert_eq!(0x0001, expected.memory[0].start);
    assert_eq!(vec![0x56], expected.memory[0].bytes);
    Ok(())
}

/// The two views the trace assertion is built on, taken from a block whose every event is
/// written out above: contention points say *where the bus was when*, transfers say *what
/// moved where*.
#[test]
fn expectation_splits_into_contention_points_and_transfers() -> Result<(), ParseError> {
    let expectations = vectors::parse_expectations("golden.expected", GOLDEN_EXPECTATION)?;
    let [expected] = &expectations[..] else {
        panic!("expected exactly one block, got {}", expectations.len());
    };

    assert_eq!(
        vec![(0, 0x0000), (4, 0x0001)],
        expected.contention_points().collect::<Vec<_>>(),
        "the two MC events, and only those",
    );
    assert_eq!(
        vec![
            Transfer {
                kind: EventKind::MemoryRead,
                addr: 0x0000,
                data: 0x02,
            },
            Transfer {
                kind: EventKind::MemoryWrite,
                addr: 0x0001,
                data: 0x56,
            },
        ],
        expected.transfers(),
        "the opcode fetch and the write, in order",
    );
    Ok(())
}

/// Every contention point must name a T-state inside the instruction's own duration.
///
/// This is what makes indexing the core's per-T-state address log by the corpus's T-state
/// safe: if the corpus ever named a T-state at or past the total, the harness would be
/// asserting against a tick that a correct core would have no reason to emit.
#[test]
fn every_contention_point_falls_inside_the_instruction() {
    let Some(corpus) = vectors::corpus_or_skip() else {
        return;
    };

    let mut points = 0usize;
    for vector in &corpus {
        let total = vector.expected.state.t_states;
        for (t_state, addr) in vector.expected.contention_points() {
            points += 1;
            assert!(
                t_state < total,
                "vector {:?}: contention at T{t_state} (address {addr:04x}) but the \
                 instruction only lasts {total} T-states",
                vector.name(),
            );
        }
    }
    println!(
        "checked {points} contention points across {} vectors",
        corpus.len()
    );
    assert!(
        points > 0,
        "the corpus produced no contention points at all"
    );
}

#[test]
fn parser_pairs_the_two_halves() -> Result<(), ParseError> {
    let setups = vectors::parse_setups("golden.in", GOLDEN_SETUP)?;
    let expectations = vectors::parse_expectations("golden.expected", GOLDEN_EXPECTATION)?;
    let paired = vectors::pair(setups, expectations)?;

    assert_eq!(1, paired.len());
    assert_eq!("02", paired[0].name());
    assert!(paired[0].is_m1_scope());
    Ok(())
}

#[test]
fn pairing_rejects_misaligned_halves() -> Result<(), ParseError> {
    let setups = vectors::parse_setups("golden.in", GOLDEN_SETUP)?;
    let renamed = GOLDEN_EXPECTATION.replacen("02\n", "03\n", 1);
    let expectations = vectors::parse_expectations("golden.expected", &renamed)?;

    assert!(
        vectors::pair(setups, expectations).is_err(),
        "a name mismatch between the halves must not be silently accepted"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Parser — malformed input
// ---------------------------------------------------------------------------

/// Each case is `(what is wrong, the block, the line the error must point at)`.
///
/// The assertion is on the reported **line number**, a typed field — not on the message
/// text, which would make the test brittle against any rewording. The line number is also
/// the thing that actually matters: a parse error that cannot say where it happened is
/// not diagnosable.
#[test]
fn parser_rejects_malformed_setup_blocks() {
    let cases: &[(&str, String, usize)] = &[
        (
            "missing the end-of-memory sentinel",
            GOLDEN_SETUP.replace("0000 02 -1\n-1\n", "0000 02 -1\n"),
            4,
        ),
        (
            "a register value that is not hexadecimal",
            GOLDEN_SETUP.replace("5600 0001", "zzzz 0001"),
            2,
        ),
        (
            "too few register values",
            GOLDEN_SETUP.replace(
                "5600 0001 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000",
                "5600 0001 0000 0000 0000 0000 0000 0000 0000 0000 0000",
            ),
            2,
        ),
        (
            "an interrupt mode above 2",
            GOLDEN_SETUP.replace("00 00 0 0 0 0", "00 00 0 0 3 0"),
            3,
        ),
        (
            "an IFF flag that is neither 0 nor 1",
            GOLDEN_SETUP.replace("00 00 0 0 0 0", "00 00 2 0 0 0"),
            3,
        ),
        (
            "too few state fields",
            GOLDEN_SETUP.replace("00 00 0 0 0 0     1\n", "00 00 0 0 0 0\n"),
            3,
        ),
        (
            "a memory line with no sentinel",
            GOLDEN_SETUP.replace("0000 02 -1\n", "0000 02\n"),
            4,
        ),
    ];

    for (description, text, expected_line) in cases {
        let result = vectors::parse_setups("malformed.in", text);
        let error = result.err().unwrap_or_else(|| {
            panic!("{description}: expected a parse error, but the block was accepted")
        });
        assert_eq!(
            *expected_line, error.line_no,
            "{description}: the error points at the wrong line ({error})",
        );
    }
}

#[test]
fn parser_rejects_an_expectation_without_a_state_line() {
    let text = concat!(
        "02\n",
        "    4 MR 0000 02\n",
        "5600 0001 0000 0000 0000 0000 0000 0000 0000 0000 0000 0001\n"
    );
    assert!(
        vectors::parse_expectations("malformed.expected", text).is_err(),
        "an expectation block missing its state line must be rejected"
    );
}

// ---------------------------------------------------------------------------
// The bus-trace assertion — proving the gate bites
// ---------------------------------------------------------------------------

/// `ADD HL,BC` as the corpus records it: a four-T-state opcode fetch, then **seven
/// separate one-T-state** internal cycles with `IR` on the bus.
///
/// This is the vector the per-T-state `tick(addr)` signature exists for, so the assertion
/// that polices it is exercised here directly — no CPU required, which means these two
/// tests keep working while the core is mid-change.
const GOLDEN_INTERNAL_CYCLES: &str = concat!(
    "09\n",
    "    0 MC 0000\n",
    "    4 MR 0000 09\n",
    "    4 MC 0001\n",
    "    5 MC 0001\n",
    "    6 MC 0001\n",
    "    7 MC 0001\n",
    "    8 MC 0001\n",
    "    9 MC 0001\n",
    "   10 MC 0001\n",
    "0030 5678 0000 f134 0000 0000 0000 0000 0000 0000 0000 0001\n",
    "00 01 0 0 0 0 11\n",
);

fn add_hl_bc() -> vectors::Expectation {
    let mut expectations = vectors::parse_expectations("golden.expected", GOLDEN_INTERNAL_CYCLES)
        .expect("the golden ADD HL,BC block parses");
    expectations.pop().expect("exactly one block")
}

#[test]
fn contention_assertion_accepts_a_correct_per_t_state_log() {
    let expected = add_hl_bc();

    // What a correct core reports: `PC` through the four fetch T-states, then `IR` for
    // each of the seven internal ones. Eleven calls for eleven T-states.
    let mut ticks = vec![0x0000u16; 4];
    ticks.extend(vec![0x0001u16; 7]);
    assert_eq!(11, ticks.len(), "the instruction lasts 11 T-states");

    assert_eq!(
        Vec::<report::Mismatch>::new(),
        report::compare_contention(&expected, &ticks),
        "a per-T-state log matching the corpus must raise nothing",
    );
}

/// The defect the trait change exists to prevent, reproduced without a CPU.
#[test]
fn contention_assertion_rejects_a_batched_tick_log() {
    let expected = add_hl_bc();

    // What a core that charges a whole run of internal cycles in one call reports: one
    // entry for the fetch and one for the entire seven-T-state internal run. Two entries
    // where eleven are required — and contention depends on `t mod 8`, so the six missing
    // T-states are six contention decisions the machine can never make.
    let batched = vec![0x0000u16, 0x0001u16];

    let mismatches = report::compare_contention(&expected, &batched);

    assert_eq!(
        7,
        mismatches.len(),
        "every internal T-state the corpus names must be reported, got: {mismatches:?}",
    );
    assert_eq!("bus address at T4", mismatches[0].field);
    assert_eq!("bus address at T10", mismatches[6].field);
    assert!(
        mismatches[0]
            .note
            .as_deref()
            .is_some_and(|note| note.contains("never batched")),
        "the failure must say why, not just that",
    );
}

#[test]
fn transfer_assertion_catches_wrong_missing_and_extra_bytes() {
    let expected = add_hl_bc().transfers();
    assert_eq!(1, expected.len(), "ADD HL,BC moves only its opcode byte");

    let fetch = expected[0];
    let wrong_byte = Transfer {
        data: 0x08,
        ..fetch
    };
    let wrong_address = Transfer {
        addr: 0x0001,
        ..fetch
    };

    let cases: &[(&str, Vec<Transfer>, usize)] = &[
        ("identical", vec![fetch], 0),
        ("wrong byte", vec![wrong_byte], 1),
        ("wrong address", vec![wrong_address], 1),
        ("nothing transferred", Vec::new(), 1),
        ("an extra transfer", vec![fetch, wrong_byte], 1),
    ];

    for (description, actual, want) in cases {
        assert_eq!(
            *want,
            report::compare_transfers(&expected, actual).len(),
            "{description}",
        );
    }
}

// ---------------------------------------------------------------------------
// Flag layout
// ---------------------------------------------------------------------------

#[test]
fn flag_layout_is_the_standard_z80_f_byte() {
    assert_eq!(0b1000_0000, flags::S);
    assert_eq!(0b0100_0000, flags::Z);
    assert_eq!(0b0010_0000, flags::Y, "undocumented bit 5");
    assert_eq!(0b0001_0000, flags::H);
    assert_eq!(0b0000_1000, flags::X, "undocumented bit 3");
    assert_eq!(0b0000_0100, flags::PV);
    assert_eq!(0b0000_0010, flags::N);
    assert_eq!(0b0000_0001, flags::C);
    assert_eq!(0b0010_1000, flags::UNDOCUMENTED);

    // Every bit is named exactly once, and the eight together cover the byte.
    let covered = flags::BITS.iter().fold(0u8, |acc, (_, mask)| acc | mask);
    assert_eq!(0xff, covered, "the named flags must cover the whole F byte");
}

#[test]
fn flag_difference_reporting_names_the_undocumented_bits() {
    assert_eq!(
        vec!["Y(bit5)", "X(bit3)"],
        flags::differences(0b0010_1000, 0b0000_0000),
        "a core that drops bits 3 and 5 must be told exactly that",
    );
    assert!(flags::differences(0x5a, 0x5a).is_empty());
    assert!(flags::describe(0xff).ends_with("C=1"));
}

#[test]
fn parity_is_set_for_an_even_number_of_bits() {
    let cases: &[(u8, bool)] = &[
        (0x00, true),
        (0x01, false),
        (0x03, true),
        (0x7f, false),
        (0xff, true),
    ];
    for (value, expected) in cases {
        assert_eq!(
            *expected,
            flags::even_parity(*value),
            "parity of {value:#04x}"
        );
    }
}

// ---------------------------------------------------------------------------
// The independent ALU reference model
// ---------------------------------------------------------------------------

/// Hand-computed from the Zilog flag rules. If the reference model itself drifts, the
/// property tests in `alu_flags.rs` would drift with it silently — so the reference is
/// pinned here against values worked out by hand, not generated.
#[test]
fn reference_alu_matches_hand_computed_flags() {
    struct Case {
        op: Binary,
        a: u8,
        operand: u8,
        carry_in: bool,
        result: u8,
        flags: u8,
    }

    let cases = [
        Case {
            op: Binary::Add,
            a: 0x38,
            operand: 0x11,
            carry_in: false,
            result: 0x49,
            flags: flags::X,
        },
        Case {
            op: Binary::Add,
            a: 0x7f,
            operand: 0x01,
            carry_in: false,
            result: 0x80,
            flags: flags::S | flags::H | flags::PV,
        },
        Case {
            op: Binary::Adc,
            a: 0xff,
            operand: 0x00,
            carry_in: true,
            result: 0x00,
            flags: flags::Z | flags::H | flags::C,
        },
        Case {
            op: Binary::Sub,
            a: 0x00,
            operand: 0x01,
            carry_in: false,
            result: 0xff,
            flags: flags::S | flags::Y | flags::H | flags::X | flags::N | flags::C,
        },
        Case {
            op: Binary::Sbc,
            a: 0x00,
            operand: 0x00,
            carry_in: true,
            result: 0xff,
            flags: flags::S | flags::Y | flags::H | flags::X | flags::N | flags::C,
        },
        Case {
            op: Binary::And,
            a: 0x0f,
            operand: 0xf0,
            carry_in: false,
            result: 0x00,
            flags: flags::Z | flags::H | flags::PV,
        },
        Case {
            op: Binary::Xor,
            a: 0xff,
            operand: 0x00,
            carry_in: false,
            result: 0xff,
            flags: flags::S | flags::Y | flags::X | flags::PV,
        },
        Case {
            op: Binary::Or,
            a: 0x00,
            operand: 0x00,
            carry_in: false,
            result: 0x00,
            flags: flags::Z | flags::PV,
        },
        // CP leaves A alone and takes bits 3 and 5 from the OPERAND (0xff), not from the
        // result (0x01). SUB with the same operands would report flags 0x13.
        Case {
            op: Binary::Cp,
            a: 0x00,
            operand: 0xff,
            carry_in: false,
            result: 0x00,
            flags: flags::Y | flags::H | flags::X | flags::N | flags::C,
        },
    ];

    for case in cases {
        let outcome = case.op.apply(case.a, case.operand, case.carry_in);
        assert_eq!(
            case.result,
            outcome.result,
            "{} a={:02x} operand={:02x}",
            case.op.mnemonic(),
            case.a,
            case.operand,
        );
        assert_eq!(
            case.flags,
            outcome.flags,
            "{} a={:02x} operand={:02x}\n  expected {}\n  actual   {}",
            case.op.mnemonic(),
            case.a,
            case.operand,
            flags::describe(case.flags),
            flags::describe(outcome.flags),
        );
    }
}

#[test]
fn reference_cp_and_sub_differ_only_in_the_undocumented_bits_and_the_accumulator() {
    for a in [0x00u8, 0x42, 0x80, 0xff] {
        for operand in [0x00u8, 0x01, 0x7f, 0xff] {
            let subtracted = Binary::Sub.apply(a, operand, false);
            let compared = Binary::Cp.apply(a, operand, false);

            assert_eq!(a, compared.result, "CP must not write back to A");
            assert_eq!(
                subtracted.flags & !flags::UNDOCUMENTED,
                compared.flags & !flags::UNDOCUMENTED,
                "CP and SUB must agree on every documented flag (a={a:02x} operand={operand:02x})",
            );
            assert_eq!(
                operand & flags::UNDOCUMENTED,
                compared.flags & flags::UNDOCUMENTED,
                "CP takes bits 3 and 5 from the operand (a={a:02x} operand={operand:02x})",
            );
        }
    }
}

#[test]
fn reference_inc_and_dec_preserve_the_carry_flag() {
    for value in [0x00u8, 0x0f, 0x7f, 0x80, 0xff] {
        for carry_in in [false, true] {
            for op in Unary::ALL {
                let outcome = op.apply(value, carry_in);
                assert_eq!(
                    carry_in,
                    outcome.flags & flags::C != 0,
                    "{} must leave C untouched (value={value:02x}, carry_in={carry_in})",
                    op.mnemonic(),
                );
            }
        }
    }
}

#[test]
fn reference_unary_matches_hand_computed_flags() {
    let cases: &[(Unary, u8, bool, u8, u8)] = &[
        (Unary::Inc, 0xff, true, 0x00, flags::Z | flags::H | flags::C),
        (
            Unary::Inc,
            0x7f,
            false,
            0x80,
            flags::S | flags::H | flags::PV,
        ),
        (
            Unary::Dec,
            0x80,
            false,
            0x7f,
            flags::Y | flags::H | flags::X | flags::PV | flags::N,
        ),
        (Unary::Dec, 0x01, false, 0x00, flags::Z | flags::N),
    ];

    for (op, value, carry_in, result, expected) in cases {
        let outcome = op.apply(*value, *carry_in);
        assert_eq!(*result, outcome.result, "{} on {value:02x}", op.mnemonic());
        assert_eq!(
            *expected,
            outcome.flags,
            "{} on {value:02x}\n  expected {}\n  actual   {}",
            op.mnemonic(),
            flags::describe(*expected),
            flags::describe(outcome.flags),
        );
    }
}

#[test]
fn binary_opcodes_follow_the_regular_encoding() {
    let expected: [(Binary, u8); 8] = [
        (Binary::Add, 0x80),
        (Binary::Adc, 0x88),
        (Binary::Sub, 0x90),
        (Binary::Sbc, 0x98),
        (Binary::And, 0xa0),
        (Binary::Xor, 0xa8),
        (Binary::Or, 0xb0),
        (Binary::Cp, 0xb8),
    ];
    for (op, opcode) in expected {
        assert_eq!(opcode, op.opcode_with_b(), "{}", op.mnemonic());
    }
    assert_eq!(0x04, Unary::Inc.opcode_on_b());
    assert_eq!(0x05, Unary::Dec.opcode_on_b());
}
