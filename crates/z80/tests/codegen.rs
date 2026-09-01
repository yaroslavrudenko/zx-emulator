//! The codegen gate — the assembly claims in `docs/ARCHITECTURE.md`, as assertions.
//!
//! # Why this file exists
//!
//! `ARCHITECTURE.md`'s *Measured* section carried a row claiming zero bounds checks in the
//! execute path. It was true when written at M1, falsified at M2, and survived M3, M4 and
//! M5 unchallenged. The section also carried an explicit instruction to re-run the
//! measurement after M2 — naming the exact milestone that would break it — and nothing
//! enforced it. `STATUS.md` records the lesson: **an unenforced instruction to re-measure
//! is the same defect as an unrun gate.** A test would have caught it on the next commit.
//!
//! So the deterministic half of that section lives here instead of in prose. What is *not*
//! here is listed under "Deliberately not asserted", below, with the reason — a row that
//! cannot be made stable is better left ungated and labelled than turned into a test that
//! goes red for unrelated reasons and gets muted within a month.
//!
//! # A count is a property of the probe, not of the core
//!
//! The same core has measured 15 bounds checks against `Cpu<Ula>`, 10 and 11 against two
//! earlier array-free probes, and [`EXPECTED_BOUNDS_CHECKS`] against the one below. None of
//! those numbers is wrong; they exercise different call sites and monomorphise over
//! different `Bus` types. **A bare integer is not a measurement.** This file therefore
//! carries its own subject — [`PROBE_MAIN`] — and builds it itself, so the number it
//! asserts is reproducible by construction rather than by memory.
//!
//! The probe is built in **release** regardless of how `cargo test` was invoked. These are
//! claims about the optimised artifact, and a gate that only checks them under
//! `--release` is a gate that nothing runs.
//!
//! # Deliberately not asserted
//!
//! - **Throughput** (`329x` / `145x` real-time) and the two percentage costs
//!   (`overflow-checks`, the unproven bank index). Wall-clock figures on a shared machine
//!   move by more than the effects they measure — one earlier pass watched 309x become
//!   300x from load alone. A flaky perf assertion is worse than none; those rows stay in
//!   `ARCHITECTURE.md`, labelled as ungated, with the command that re-runs them.
//! - **"`Bus::read` compiles to one instruction"**. The instruction count of a bus method
//!   is a property of *that bus*, not of the core, and the original measurement did not
//!   record which bus it used. What is gated instead is the half that *is* a property of
//!   the core: [`no_bus_method_survives_as_an_out_of_line_call`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------------------
// The pinned expectations
// ---------------------------------------------------------------------------------------

/// The toolchain the numbers below were pinned against.
///
/// `rust-toolchain.toml` says `channel = "stable"`, which floats, so this *will* eventually
/// disagree. That is not a flaw in the gate: a failure message that can say "your rustc is
/// not the pinned one" turns an otherwise baffling red into a two-minute re-pin.
const PINNED_RUSTC: &str = "1.98.0";

/// `panic_bounds_check` call sites attributable to `crates/z80/src`, for [`PROBE_MAIN`].
///
/// **0 at M1, 7 from M2 onwards.**
///
/// # What this integer is, before what it means
///
/// It counts **`bl` instructions, not bounds checks.** LLVM tail-merges cold blocks, so
/// several checks can share one call site: in one measured variant, five `b.hi` branches
/// reach three of them. And it is a property of the **inliner** as much as of the source —
/// holding the probe, the toolchain and the semantics fixed and varying only inlining
/// decisions, the same M2 core measures 0, 3, 5, 7 and 10. That is why the same core has
/// also reported 10, 11 and 15 under other probes, and why the subject line above is
/// load-bearing rather than decorative.
///
/// # Why M2 — measured, replacing an inference that named the wrong cause
///
/// This comment used to say *"M2 made the operand field a runtime value where M1 had
/// constants, so the register-file index stopped being provable."* That was a hypothesis,
/// and nine builds of one fixed probe — M1, M2, the working tree and six single-edit
/// variants — have since falsified it. **One of its two clauses describes a change that
/// never happened**: `Operand::source`/`destination` already indexed `OPERANDS` with
/// `opcode & 0x07` at M1, so the operand field was never a constant.
///
/// What actually happens:
///
/// - All seven sites are `Registers::get`/`set`'s `self.regs[index.0]`. Proving a range at
///   that one expression takes the count to **0**; nothing else in the crate indexes that
///   array.
/// - **Four of the seven are in `load_pair_absolute` and `store_pair_absolute`, which are
///   byte-identical between M1 and M2.** What changed is the call graph. At M1 each had a
///   single call site reached with `base = pair::HL`, constant-propagated in from `step`.
///   M2 gave each a **second** caller on the new `ED` page and made both arguments runtime
///   values, at which point LLVM declines to inline them — in its own words,
///   `too costly to inline (cost=585/615, threshold=525)` — and in the shared out-of-line
///   copy the `PairBase` parameter carries no provable range.
/// - The runtime base is therefore the **operand, not the cause**: grafting M2's entire
///   prefix-consuming `dispatch` onto the M1 core leaves the count at **0**, and adding a
///   second caller on top of that leaves it at 0 as well, because on a core that size LLVM
///   still inlines both and knows the base at every site.
/// - The remaining **three sites are not settled.** They sit in the fully-inlined body, are
///   reachable, and are *not* removed by proving the base's range — only by proving the
///   index at the array itself. This says so rather than rounding four measured checks up
///   to seven.
///
/// The full account, with the eight-tree table, is in `docs/ARCHITECTURE.md`.
const EXPECTED_BOUNDS_CHECKS: usize = 7;

/// Where those checks sit, so a change says *which* index moved rather than only that one did.
const EXPECTED_BOUNDS_SITES: &[(&str, u32, usize)] = &[
    ("src/registers.rs", 143, 3), // `self.regs[index.0]`
    ("src/registers.rs", 148, 3), // `self.regs[index.0] = value`
    ("src/instructions.rs", 0, 1),
];

/// Decode's jump tables, largest first. **Two at M1 (119 + 64); the 124-entry table
/// arrived with M2.**
///
/// # Attributed by outlining, not by guesswork
///
/// This comment used to read *"the un-prefixed table, the `CB` table, and the block the two
/// share"*, and `ARCHITECTURE.md` used to call the third one *"the prefix dispatch"*. Both
/// are wrong, and outlining the three dispatch functions says so directly:
///
/// - **124 is `Cpu::<B>::execute_ed`'s** — the `ED` page M2 added, which is why the table
///   count moved at exactly that milestone.
/// - **119 + 64 are `Cpu::<B>::execute`'s**, and are the two M1 already had.
/// - **`dispatch` builds no table at all.** It matches five values and lowers to
///   comparisons — so "the prefix dispatch" names the one function here that is not a jump
///   table.
/// - **The `CB` page builds none either.** It decodes arithmetically through
///   `CbOp::from_opcode`.
const EXPECTED_JUMP_TABLES: &[usize] = &[124, 119, 64];

/// A floor on how much of the core the probe actually dragged in.
///
/// Without this, every "count is zero" assertion below passes vacuously if the probe stops
/// exercising the core — which is exactly the shape of failure this project has caught
/// twice before: a harness reporting green while verifying nothing. It earned its keep on
/// its first run, catching a build that emitted assembly *without* linking and so never
/// ran LTO: the core stayed in its own rlib, and every count below would have read a
/// triumphant zero.
///
/// The floor is deliberately loose and is calibrated to the current core: measured **1427
/// at M5** and **618 at M1**, when only the un-prefixed opcodes existed. It only has to
/// separate "the core is in this artifact" from "it is not" — and a probe that stops
/// driving the core reads **0**, so the gap it must span is three orders of magnitude, not
/// a few per cent. Lower it if the core is ever deliberately made smaller.
const MINIMUM_Z80_LOCATIONS: usize = 1_000;

// ---------------------------------------------------------------------------------------
// The subject
// ---------------------------------------------------------------------------------------

/// The probe: a `Cpu<NullBus>` forced to be emitted, with a bus that owns no array.
///
/// Two properties make it a usable subject, and both are deliberate. **`NullBus` owns no
/// array**, so every surviving `panic_bounds_check` must come from `crates/z80/src` — a bus
/// holding a `[u8; 65536]` contributes its own and the number stops saying anything about
/// the core. And **`main` drives `step`, `interrupt` and `nmi` in all three interrupt
/// modes**, so the whole execute path is reachable and cannot be eliminated as dead.
const PROBE_MAIN: &str = r##"
use std::hint::black_box;
use z80::{Bus, Cpu, CpuState, InterruptMode};

struct NullBus {
    t_states: u64,
    last: u16,
}

impl Bus for NullBus {
    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        self.last = addr;
        (addr as u8) ^ ((addr >> 8) as u8)
    }

    #[inline]
    fn write(&mut self, addr: u16, val: u8) {
        self.last = addr ^ u16::from(val);
    }

    #[inline]
    fn in_port(&mut self, port: u16) -> u8 {
        self.last = port;
        (port >> 8) as u8
    }

    #[inline]
    fn out_port(&mut self, port: u16, val: u8) {
        self.last = port ^ u16::from(val);
    }

    #[inline]
    fn tick(&mut self, addr: u16) {
        self.t_states = self.t_states.wrapping_add(1);
        self.last ^= addr;
    }
}

fn drive(mode: InterruptMode, steps: u32) -> u64 {
    let mut cpu = Cpu::new(NullBus { t_states: 0, last: 0 });
    cpu.set_state(CpuState {
        pc: black_box(0x8000),
        sp: black_box(0x7FF0),
        i: black_box(0x39),
        iff1: true,
        iff2: true,
        im: mode,
        ..CpuState::default()
    });

    let mut acc = 0u64;
    for _ in 0..black_box(steps) {
        acc = acc.wrapping_add(u64::from(cpu.step()));
    }
    acc = acc.wrapping_add(u64::from(cpu.interrupt(black_box(0xC7))));
    acc = acc.wrapping_add(u64::from(cpu.nmi()));

    let state = cpu.state();
    acc.wrapping_add(u64::from(state.pc))
        .wrapping_add(u64::from(state.af))
        .wrapping_add(u64::from(state.r))
        .wrapping_add(cpu.bus().t_states)
}

fn main() {
    let mut acc = 0u64;
    for mode in [InterruptMode::Mode0, InterruptMode::Mode1, InterruptMode::Mode2] {
        acc = acc.wrapping_add(drive(mode, 4096));
    }
    // No `println!`: the standard output machinery allocates, and this probe is also the
    // subject of the "allocates nothing" measurement.
    std::process::exit((black_box(acc) & 0x3F) as i32);
}
"##;

/// The probe's manifest.
///
/// The profile is byte-for-byte the shipped `[profile.release]` plus `debug = true` —
/// which is exactly what the workspace's own `[profile.bench]` does, and is what puts
/// `.loc` records in the assembly so a count can be attributed to a source line.
/// [`the_probe_profile_still_matches_the_shipped_release_profile`] checks that claim
/// rather than trusting this comment, because a comment asserting a property the code does
/// not provide is this project's most-recorded defect.
const PROBE_PROFILE: &[(&str, &str)] = &[
    ("opt-level", "3"),
    ("lto", "\"fat\""),
    ("codegen-units", "1"),
    ("panic", "\"abort\""),
    ("overflow-checks", "true"),
];

// ---------------------------------------------------------------------------------------
// Building and reading the artifact
// ---------------------------------------------------------------------------------------

/// The `z80` crate under test.
fn z80_crate() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The workspace root — `crates/z80/../..`.
fn workspace_root() -> PathBuf {
    z80_crate()
        .parent()
        .and_then(Path::parent)
        .expect("crates/z80 always has two ancestors")
        .to_path_buf()
}

/// Where cargo is putting build output for the run that is executing this test.
fn target_root() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"))
}

/// Write `contents` to `path` only if it would change, so an unchanged probe does not
/// force cargo to rebuild it on every run.
fn write_if_changed(path: &Path, contents: &str) {
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == contents) {
        return;
    }
    std::fs::write(path, contents).unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
}

/// Materialise the probe crate and build it in release, returning the emitted assembly.
fn build_probe() -> (String, String) {
    let probe = target_root().join("codegen-gate").join("probe");
    std::fs::create_dir_all(probe.join("src")).expect("creating the probe crate directory");

    let profile = PROBE_PROFILE
        .iter()
        .map(|(k, v)| format!("{k} = {v}\n"))
        .collect::<String>();
    let manifest = format!(
        "[package]\n\
         name = \"codegen-probe\"\n\
         version = \"0.0.0\"\n\
         edition = \"2024\"\n\
         publish = false\n\
         \n\
         # Detached from the zx-emulator workspace on purpose: the probe pins its own\n\
         # profile, so a change to the workspace's release profile shows up as a\n\
         # deliberate re-pin here rather than silently moving every number it produces.\n\
         [workspace]\n\
         \n\
         [dependencies]\n\
         z80 = {{ path = {:?} }}\n\
         \n\
         [profile.release]\n\
         {profile}debug = true\n",
        z80_crate()
    );

    write_if_changed(&probe.join("Cargo.toml"), &manifest);
    write_if_changed(&probe.join("src").join("main.rs"), PROBE_MAIN);

    let build_dir = probe.join("target");
    let command = format!(
        "cd {} && CARGO_TARGET_DIR={} {} rustc --release --offline -- --emit=asm,link",
        probe.display(),
        build_dir.display(),
        env!("CARGO"),
    );

    let output = Command::new(env!("CARGO"))
        .args(["rustc", "--release", "--offline", "--", "--emit=asm,link"])
        .current_dir(&probe)
        .env("CARGO_TARGET_DIR", &build_dir)
        .output()
        .unwrap_or_else(|e| panic!("could not run cargo to build the codegen probe: {e}"));

    assert!(
        output.status.success(),
        "the codegen probe did not build, so nothing below was measured.\n\
         \n\
         command: {command}\n\
         status : {}\n\
         \n\
         --- cargo stderr ---\n{}\n\
         \n\
         `--offline` is deliberate: the gate must not reach the network. The probe's only\n\
         dependency is the `z80` crate by path, which pulls `thiserror` from the registry\n\
         cache the workspace build already populated. If that cache is cold, build the\n\
         workspace once first.",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );

    let deps = build_dir.join("release").join("deps");
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(&deps)
        .expect("the probe's deps directory")
        .flatten()
    {
        let path = entry.path();
        let is_probe_asm = path.extension().is_some_and(|e| e == "s")
            && path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("codegen_probe-"));
        if !is_probe_asm {
            continue;
        }
        let modified = entry.metadata().and_then(|m| m.modified()).expect("mtime");
        if newest.as_ref().is_none_or(|(best, _)| modified > *best) {
            newest = Some((modified, path));
        }
    }
    let asm = newest
        .unwrap_or_else(|| {
            panic!(
                "no codegen_probe-*.s in {} after a successful build",
                deps.display()
            )
        })
        .1;

    let text =
        std::fs::read_to_string(&asm).unwrap_or_else(|e| panic!("reading the assembly: {e}"));
    (text, command)
}

/// Everything the gate reads out of one build, so the probe is built once per test binary.
struct Facts {
    /// How the artifact was produced, for pasting into a failure message.
    command: String,
    /// `panic_bounds_check` call sites attributable to `crates/z80/src`, by `file:line`.
    bounds: BTreeMap<(String, u32), usize>,
    /// Register-indirect calls (`blr`, `call *%r`) attributable to `crates/z80/src`.
    indirect_calls: usize,
    /// Out-of-line definitions of `Bus` trait methods.
    out_of_line_bus_methods: Vec<String>,
    /// Entry counts of the jump tables owned by code the probe compiled, largest first.
    jump_tables: Vec<usize>,
    /// How many machine instructions carry a `.loc` naming a `crates/z80/src` file.
    z80_locations: usize,
    /// The rustc that produced it.
    rustc: String,
}

fn facts() -> &'static Facts {
    static FACTS: OnceLock<Facts> = OnceLock::new();
    FACTS.get_or_init(|| {
        let (asm, command) = build_probe();
        parse(&asm, command)
    })
}

/// Read the emitted assembly.
///
/// Attribution rule, and it is the reason the numbers here are trustworthy: `.loc` is
/// emitted only when the location *changes*, so the first instructions of a function
/// inherit whatever location the previous function left behind. The walk therefore
/// **clears the current location at every function label**. An earlier version of this
/// parser did not, and confidently attributed a thread-local initialiser at the top of
/// `main` to `instructions.rs:911`. Under-attributing is safe here; over-attributing
/// invents findings.
fn parse(asm: &str, command: String) -> Facts {
    let mut files: BTreeMap<&str, String> = BTreeMap::new();
    let mut location: Option<(String, u32)> = None;
    let mut function: Option<String> = None;

    let mut bounds: BTreeMap<(String, u32), usize> = BTreeMap::new();
    let mut indirect_calls = 0usize;
    let mut out_of_line_bus_methods = Vec::new();
    let mut z80_locations = 0usize;

    // Jump tables: `LJTIn_m:` followed by one entry directive per target.
    let mut tables: BTreeMap<String, usize> = BTreeMap::new();
    let mut table: Option<String> = None;
    let mut table_owner: BTreeMap<String, String> = BTreeMap::new();

    for raw in asm.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix(".file") {
            // `.file N "dir" "name"` or `.file N "name"`.
            let quoted: Vec<&str> = rest.split('"').skip(1).step_by(2).collect();
            if let (Some(index), Some(last)) = (rest.split_whitespace().next(), quoted.last()) {
                let path = if quoted.len() >= 2 {
                    format!("{}/{}", quoted[0], last)
                } else {
                    (*last).to_string()
                };
                files.insert(index.trim(), path);
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix(".loc") {
            let mut parts = rest.split_whitespace();
            if let (Some(index), Some(number)) = (parts.next(), parts.next()) {
                let path = files.get(index).cloned().unwrap_or_default();
                let number = number.parse().unwrap_or(0);
                if path.contains("crates/z80/src") {
                    z80_locations += 1;
                }
                location = Some((path, number));
            }
            continue;
        }

        // A label. Local labels start with `L` on Mach-O and `.L` on ELF.
        if let Some(name) = line
            .strip_suffix(':')
            .filter(|n| !n.contains(char::is_whitespace))
        {
            if let Some(id) = name
                .strip_prefix("LJTI")
                .or_else(|| name.strip_prefix(".LJTI"))
            {
                table = Some(id.to_string());
                tables.insert(id.to_string(), 0);
                continue;
            }
            // Only a *function* label ends a location. Local labels (`LBB`, `Ltmp`, `.L`)
            // sit inside a function and the location carries across them — clearing there
            // strips almost every call site of its attribution and silently reads zero.
            if !name.starts_with('L') && !name.starts_with(".L") {
                function = Some(name.to_string());
                location = None;
                if name.contains("3bus3Bus") {
                    out_of_line_bus_methods.push(name.to_string());
                }
            }
            table = None;
            continue;
        }

        if let Some(id) = table.clone() {
            let entry = ["\t.long", "\t.short", "\t.byte", "\t.quad", "\t.word"]
                .iter()
                .any(|d| line.starts_with(d) || trimmed.starts_with(d.trim_start()));
            if entry {
                *tables.entry(id).or_default() += 1;
                continue;
            }
            if !trimmed.is_empty()
                && !trimmed.starts_with(".p2align")
                && !trimmed.starts_with(".align")
            {
                table = None;
            }
        }

        // Which function references a table decides whose table it is.
        for token in line.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.')) {
            if let Some(id) = token
                .strip_prefix("LJTI")
                .or_else(|| token.strip_prefix(".LJTI"))
                && let Some(owner) = function.clone()
            {
                table_owner.entry(id.to_string()).or_insert(owner);
            }
        }

        let in_z80 = location
            .as_ref()
            .is_some_and(|(p, _)| p.contains("crates/z80/src"));

        if trimmed.starts_with("bl\t") || trimmed.starts_with("bl ") || trimmed.starts_with("call")
        {
            if trimmed.contains("panic_bounds_check")
                && let Some((path, number)) = location.clone().filter(|_| in_z80)
            {
                let short = path
                    .rsplit("crates/z80/")
                    .next()
                    .unwrap_or(&path)
                    .to_string();
                *bounds.entry((short, number)).or_default() += 1;
            }
            continue;
        }

        if in_z80 && (trimmed.starts_with("blr\t") || trimmed.starts_with("blr ")) {
            indirect_calls += 1;
        }
    }

    // Only tables owned by code this probe compiled — std drags in ~19 of its own.
    let mut jump_tables: Vec<usize> = tables
        .iter()
        .filter(|(id, entries)| {
            **entries > 1
                && table_owner
                    .get(*id)
                    .is_some_and(|o| o.contains("codegen_probe") || o.contains("3z80"))
        })
        .map(|(_, entries)| *entries)
        .collect();
    jump_tables.sort_unstable_by(|a, b| b.cmp(a));

    let rustc = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());

    Facts {
        command,
        bounds,
        indirect_calls,
        out_of_line_bus_methods,
        jump_tables,
        z80_locations,
        rustc,
    }
}

/// The preamble every failure message shares: what was measured, and how to redo it.
fn context(facts: &Facts) -> String {
    let drift = if facts.rustc.contains(PINNED_RUSTC) {
        format!("rustc matches the pin ({PINNED_RUSTC}), so this is a real codegen change")
    } else {
        format!(
            "rustc is NOT the pinned {PINNED_RUSTC} — it is `{}`. Treat this as a re-pin, \
             not a regression: re-measure, update the constant in this file AND the row in \
             docs/ARCHITECTURE.md, and move the pin",
            facts.rustc
        )
    };
    format!(
        "  subject: the NullBus probe in crates/z80/tests/codegen.rs (a Bus owning no array)\n\
         \x20 build  : {}\n\
         \x20 rustc  : {}\n\
         \x20 note   : {drift}\n",
        facts.command, facts.rustc,
    )
}

// ---------------------------------------------------------------------------------------
// The assertions
// ---------------------------------------------------------------------------------------

/// The positive control. Every "count is zero" assertion below is vacuous if the probe
/// stopped dragging the core in, and a vacuous pass is indistinguishable from a real one.
#[test]
fn the_probe_actually_exercises_the_core() {
    let facts = facts();
    assert!(
        facts.z80_locations >= MINIMUM_Z80_LOCATIONS,
        "the probe emitted only {} source locations naming crates/z80/src, below the floor \
         of {MINIMUM_Z80_LOCATIONS}. Every other assertion in this file is measuring \
         an artifact that does not contain the core, so their greens mean nothing.\n{}",
        facts.z80_locations,
        context(facts),
    );
}

/// `ARCHITECTURE.md`: register indexing is **not** proven in range — 0 at M1, 7 here since M2.
#[test]
fn bounds_checks_in_the_execute_path_have_not_moved() {
    let facts = facts();
    let total: usize = facts.bounds.values().sum();
    let observed: Vec<String> = facts
        .bounds
        .iter()
        .map(|((file, line), n)| format!("{n} at {file}:{line}"))
        .collect();
    let expected: Vec<String> = EXPECTED_BOUNDS_SITES
        .iter()
        .map(|(file, line, n)| format!("{n} at {file}:{line}"))
        .collect();

    assert_eq!(
        total,
        EXPECTED_BOUNDS_CHECKS,
        "panic_bounds_check count changed: pinned {EXPECTED_BOUNDS_CHECKS}, measured {total}.\n\
         \x20 pinned sites  : {}\n\
         \x20 measured sites: {}\n\
         {}\n\
         This number is a property of THIS probe AND of the inliner's budget on the day. \
         The same core measures 15 against `Cpu<Ula>`, measured 10 and 11 against two \
         earlier probes, and measures 0, 3, 5, 7 and 10 under inlining changes alone — do \
         not compare it with any of those. It counts `bl` instructions, not checks: LLVM \
         tail-merges cold blocks, so several checks can share one call site.\n\
         It was 0 at M1 and {EXPECTED_BOUNDS_CHECKS} from M2 on. Measured cause: four of \
         the seven are in `load_pair_absolute`/`store_pair_absolute`, byte-identical \
         between M1 and M2, which M2 gave a second caller on the ED page with runtime \
         arguments — LLVM then refuses to inline them (cost 585/615 against a threshold of \
         525) and the shared out-of-line copy's `PairBase` parameter has no provable range. \
         The other three sit in the inlined body and are NOT settled.\n\
         If this went DOWN, someone made the index provable: update the constant here and \
         the row in docs/ARCHITECTURE.md, and say which change did it.",
        expected.join(", "),
        observed.join(", "),
        context(facts),
    );
}

/// `ARCHITECTURE.md`: *the execute path allocates nothing*.
///
/// # This is asserted structurally, and the artifact count was tried first and dropped
///
/// The obvious check — count `bl __rust_alloc` sites attributable to `crates/z80/src` in
/// the probe's assembly — was written, and then **proven insensitive**: a mutation that
/// made `Cpu::step` build a `Vec` on every call left that count at zero, because the call
/// to the allocator sits inside `alloc`'s own code and carries `alloc`'s `.loc`, not the
/// caller's. It would have been an assertion that could not fail, which is the defect this
/// project has caught more times than any other. It is gone rather than left in looking
/// reassuring.
///
/// What replaces it is stronger, not weaker. `crates/z80` is
/// `#![cfg_attr(not(test), no_std)]` with no `extern crate alloc`, so in every non-test
/// build **allocation does not compile**. That is a guarantee about all builds rather than
/// an observation about one, and the mutation above had to delete the `no_std` attribute
/// before it could allocate at all — which is exactly what these two lines catch.
#[test]
fn the_execute_path_allocates_nothing() {
    let lib = std::fs::read_to_string(z80_crate().join("src").join("lib.rs"))
        .expect("crates/z80/src/lib.rs");
    assert!(
        lib.contains("#![cfg_attr(not(test), no_std)]"),
        "crates/z80/src/lib.rs no longer declares `#![cfg_attr(not(test), no_std)]`. That \
         declaration is what makes the allocation claim structural instead of a property of \
         one build, so losing it weakens the row above even if the count stays at 0."
    );
    assert!(
        !lib.contains("extern crate alloc"),
        "crates/z80/src/lib.rs now pulls in `alloc`, so the core can allocate. The \
         'allocates nothing' row in docs/ARCHITECTURE.md needs rewriting, not re-pinning."
    );
}

/// `ARCHITECTURE.md`: *`#[inline]` makes cross-crate inlining happen — no call to any bus method*.
#[test]
fn no_bus_method_survives_as_an_out_of_line_call() {
    let facts = facts();
    assert!(
        facts.out_of_line_bus_methods.is_empty(),
        "{} `Bus` method(s) were emitted out of line instead of being inlined into the \
         core:\n    {}\n{}\n\
         `Bus::read`, `write` and `fetch` are the hottest calls in the emulator; the whole \
         reason `Cpu` is generic over `B` rather than boxed is that these disappear. Note \
         this is a property of the build unit as much as of the annotations: the same core \
         compiled as a library object (`cargo rustc -p spectrum --release --lib`) does emit \
         `Ula::tick` out of line 124 times despite its `#[inline]`.",
        facts.out_of_line_bus_methods.len(),
        facts.out_of_line_bus_methods.join("\n    "),
        context(facts),
    );
}

/// `ARCHITECTURE.md`: *monomorphised, no `dyn` on the execute path* — indirect calls zero.
#[test]
fn the_execute_path_makes_no_indirect_call() {
    let facts = facts();
    assert_eq!(
        facts.indirect_calls,
        0,
        "{} register-indirect call(s) are attributable to crates/z80/src. The claim is that \
         the core monomorphises and never dispatches through a pointer.\n{}\n\
         The construct that can produce one is a function-pointer parameter, and there are \
         two: `rotate_digit` and `rotate_a` in instructions.rs. Whether LLVM specialises \
         them away depends on the build unit — a whole-program LTO binary like this probe \
         devirtualises both, while the `Cpu<Ula>` library object keeps one.",
        facts.indirect_calls,
        context(facts),
    );

    let source = z80_crate().join("src");
    for file in [
        "lib.rs",
        "decode.rs",
        "instructions.rs",
        "registers.rs",
        "flags.rs",
    ] {
        let text = std::fs::read_to_string(source.join(file)).expect(file);
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or(line);
            assert!(
                !(code.contains("dyn ") || code.contains("Box<") || code.contains("Rc<")),
                "crates/z80/src/{file}:{} introduces dynamic dispatch or a heap pointer:\n    {}\n\
                 The 'no dyn on the execute path' row in docs/ARCHITECTURE.md rests on there \
                 being none.",
                number + 1,
                line.trim(),
            );
        }
    }
}

/// `ARCHITECTURE.md`: *decode lowers to a jump table, not a compare chain*.
#[test]
fn decode_still_lowers_to_jump_tables() {
    let facts = facts();
    assert_eq!(
        facts.jump_tables,
        EXPECTED_JUMP_TABLES,
        "decode's jump tables changed: pinned {EXPECTED_JUMP_TABLES:?}, measured {:?}.\n{}\n\
         There were TWO at M1 (119 + 64) and three from M2. Attributed by outlining: 124 \
         is `execute_ed`'s (the ED page M2 added) and 119 + 64 are `execute`'s. `dispatch` \
         builds no table — it matches five values and lowers to comparisons — and the CB \
         page decodes arithmetically through `CbOp::from_opcode`.\n\
         If the list is now empty or much shorter, decode has lowered to a compare \
         chain and the row is false. If it merely changed size, the opcode set moved and \
         both the constant here and the row in docs/ARCHITECTURE.md need re-pinning.",
        facts.jump_tables,
        context(facts),
    );
}

/// The probe's profile has to keep matching the profile whose behaviour is being claimed.
///
/// Without this the gate could quietly measure `opt-level = 0` after somebody retunes the
/// workspace, and go green while describing a build nobody ships.
#[test]
fn the_probe_profile_still_matches_the_shipped_release_profile() {
    let manifest = std::fs::read_to_string(workspace_root().join("Cargo.toml"))
        .expect("the workspace manifest");
    let release = manifest
        .split("[profile.release]")
        .nth(1)
        .expect("the workspace has a [profile.release]")
        .split("\n[")
        .next()
        .expect("a section always has a body");

    for (key, value) in PROBE_PROFILE {
        let found = release.lines().find_map(|line| {
            let (k, v) = line.split_once('=')?;
            (k.trim() == *key).then(|| v.trim().to_string())
        });
        assert_eq!(
            found.as_deref(),
            Some(*value),
            "the workspace's [profile.release] sets `{key}` to {:?}, but this gate builds \
             its probe with {value:?}. Every number in this file describes a build nobody \
             ships until the two agree — reconcile PROBE_PROFILE with the workspace \
             manifest, then re-measure and re-pin.",
            found.as_deref().unwrap_or("<unset>"),
        );
    }
}
