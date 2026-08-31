//! Parser for the FUSE emulator project's Z80 conformance corpus
//! (`tests.in` / `tests.expected`).
//!
//! This module is deliberately free of any dependency on the `z80` crate: it is pure
//! data in, pure data out. That lets `tests/fuse_format.rs` exercise it — and prove it
//! actually runs — independently of whether the CPU core compiles yet.
//!
//! # The grammar, as derived from the corpus itself
//!
//! Both files are sequences of blank-line-separated blocks, one block per vector, in
//! the same order, with matching names.
//!
//! `tests.in` block:
//! ```text
//! <name>
//! AF BC DE HL AF' BC' DE' HL' IX IY SP PC        12 x hex16
//! I R IFF1 IFF2 IM halted tstates                I,R hex8; rest decimal
//! <addr16> <byte8>... -1                         zero or more memory blocks
//! -1                                             end-of-memory sentinel
//! ```
//!
//! `tests.expected` block:
//! ```text
//! <name>
//!     <time> <MR|MW|MC|PR|PW|PC> <addr16> [<byte8>]    zero or more bus events
//! AF BC DE HL AF' BC' DE' HL' IX IY SP PC
//! I R IFF1 IFF2 IM halted tstates                tstates here is the ACTUAL total
//! <addr16> <byte8>... -1                         zero or more memory blocks
//! ```
//! (no `-1` sentinel — the blank line ends the block).
//!
//! The `tstates` field means different things in the two files: in `tests.in` it is the
//! T-state count to *run until*; in `tests.expected` it is the count actually consumed.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

/// File name of the initial-state half of the corpus.
pub const SETUP_FILE: &str = "tests.in";
/// File name of the expected-state half of the corpus.
pub const EXPECTED_FILE: &str = "tests.expected";

const REGISTER_FIELDS: usize = 12;
const STATE_FIELDS: usize = 7;
const END_OF_MEMORY: &str = "-1";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// A malformed line in one of the corpus files, carrying enough context to fix the
/// file without opening a debugger: which file, which line number, the line itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub file: String,
    pub line_no: usize,
    pub line: String,
    pub reason: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {}\n  in: {:?}",
            self.file, self.line_no, self.reason, self.line
        )
    }
}

impl Error for ParseError {}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// The twelve 16-bit registers a vector specifies, in corpus order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Registers {
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub af_shadow: u16,
    pub bc_shadow: u16,
    pub de_shadow: u16,
    pub hl_shadow: u16,
    pub ix: u16,
    pub iy: u16,
    pub sp: u16,
    pub pc: u16,
}

impl Registers {
    /// Name/value pairs in corpus order, for field-by-field mismatch reporting.
    pub fn named(&self) -> [(&'static str, u16); REGISTER_FIELDS] {
        [
            ("AF", self.af),
            ("BC", self.bc),
            ("DE", self.de),
            ("HL", self.hl),
            ("AF'", self.af_shadow),
            ("BC'", self.bc_shadow),
            ("DE'", self.de_shadow),
            ("HL'", self.hl_shadow),
            ("IX", self.ix),
            ("IY", self.iy),
            ("SP", self.sp),
            ("PC", self.pc),
        ]
    }

    /// The accumulator — high byte of `AF`.
    pub fn a(&self) -> u8 {
        (self.af >> 8) as u8
    }

    /// The flag byte — low byte of `AF`, undocumented bits 3 and 5 included.
    pub fn f(&self) -> u8 {
        self.af as u8
    }
}

/// The interrupt/refresh/halt state line, plus the T-state field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    pub i: u8,
    pub r: u8,
    pub iff1: bool,
    pub iff2: bool,
    pub im: u8,
    pub halted: bool,
    /// In a setup: run until the T-state count reaches this. In an expectation: the
    /// exact number of T-states the instruction must consume.
    pub t_states: u32,
}

impl State {
    /// Name/value pairs for the non-T-state fields, for mismatch reporting.
    pub fn named(&self) -> [(&'static str, u32); 6] {
        [
            ("I", u32::from(self.i)),
            ("R", u32::from(self.r)),
            ("IFF1", u32::from(self.iff1)),
            ("IFF2", u32::from(self.iff2)),
            ("IM", u32::from(self.im)),
            ("halted", u32::from(self.halted)),
        ]
    }
}

/// A contiguous run of bytes at a base address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryBlock {
    pub start: u16,
    pub bytes: Vec<u8>,
}

impl MemoryBlock {
    /// The addresses this block covers, wrapping at the top of the 64K space exactly
    /// as the Z80's address bus does.
    pub fn addresses(&self) -> impl Iterator<Item = (u16, u8)> + '_ {
        self.bytes
            .iter()
            .enumerate()
            .map(|(offset, byte)| (self.start.wrapping_add(offset as u16), *byte))
    }
}

/// The kind of bus access recorded in a `tests.expected` event line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    MemoryRead,
    MemoryWrite,
    MemoryContend,
    PortRead,
    PortWrite,
    PortContend,
}

impl EventKind {
    fn from_token(token: &str) -> Option<Self> {
        match token {
            "MR" => Some(Self::MemoryRead),
            "MW" => Some(Self::MemoryWrite),
            "MC" => Some(Self::MemoryContend),
            "PR" => Some(Self::PortRead),
            "PW" => Some(Self::PortWrite),
            "PC" => Some(Self::PortContend),
            _ => None,
        }
    }

    /// The token as it appears in the corpus.
    pub fn token(self) -> &'static str {
        match self {
            Self::MemoryRead => "MR",
            Self::MemoryWrite => "MW",
            Self::MemoryContend => "MC",
            Self::PortRead => "PR",
            Self::PortWrite => "PW",
            Self::PortContend => "PC",
        }
    }
}

/// One line of the expected bus-access trace: when, what, where, and (for data
/// accesses) which byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusEvent {
    pub at_t_state: u32,
    pub kind: EventKind,
    pub addr: u16,
    pub data: Option<u8>,
}

impl EventKind {
    /// `MC` / `PC` — a contention check.
    ///
    /// Each one states, exactly, **which address was on the bus during that single
    /// T-state**. That is the strongest claim the corpus makes about the address bus, and
    /// it is precisely what `Bus::tick(addr)` reports, so the two compare directly with no
    /// modelling in between.
    ///
    /// Note what a contention event does *not* say: an `MC` at the start of a four-T-state
    /// opcode fetch says nothing about T-states 1 to 3 of that fetch. The harness asserts
    /// only the T-states the corpus actually pins down.
    pub fn is_contention(self) -> bool {
        matches!(self, Self::MemoryContend | Self::PortContend)
    }

    /// `MR` / `MW` / `PR` / `PW` — a byte actually moved.
    pub fn is_transfer(self) -> bool {
        !self.is_contention()
    }
}

/// A byte moved across the bus, without a timestamp.
///
/// Timing deliberately lives in the contention-point comparison instead. The corpus times
/// `MR`/`MW` at the *end* of their machine cycle and `PR`/`PW` by the ULA's I/O contention
/// pattern, while the core reports a transfer whenever it happens to call the method — so
/// a timestamp here would be asserting the core's internal call ordering, which is an
/// implementation detail, not a hardware fact. What a transfer must get right is *what*
/// moved *where*, in *what order*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    pub kind: EventKind,
    pub addr: u16,
    pub data: u8,
}

impl fmt::Display for Transfer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {:04x}={:02x}",
            self.kind.token(),
            self.addr,
            self.data
        )
    }
}

/// The prefix byte that turns the *following* opcode into a different instruction set.
/// Everything behind one of these is M2 scope, not M1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Prefix {
    Cb,
    Ed,
    Dd,
    Fd,
}

impl Prefix {
    /// `None` for every un-prefixed opcode — the M1 instruction set.
    pub fn from_opcode(opcode: u8) -> Option<Self> {
        match opcode {
            0xCB => Some(Self::Cb),
            0xED => Some(Self::Ed),
            0xDD => Some(Self::Dd),
            0xFD => Some(Self::Fd),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Cb => "CB",
            Self::Ed => "ED",
            Self::Dd => "DD",
            Self::Fd => "FD",
        }
    }
}

/// One `tests.in` block: the machine state to load before stepping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setup {
    pub name: String,
    pub registers: Registers,
    pub state: State,
    pub memory: Vec<MemoryBlock>,
}

impl Setup {
    /// The byte the vector places at the initial `PC`.
    ///
    /// Absent memory reads as `0x00` (`NOP`) because that is what the 64K RAM the
    /// harness hands the CPU actually contains — the model and the machine agree.
    pub fn opcode_at_pc(&self) -> u8 {
        self.byte_at(self.registers.pc).unwrap_or(0x00)
    }

    /// The byte this vector defines at `addr`, if it defines one.
    pub fn byte_at(&self, addr: u16) -> Option<u8> {
        self.memory
            .iter()
            .flat_map(MemoryBlock::addresses)
            .find(|(a, _)| *a == addr)
            .map(|(_, byte)| byte)
    }

    /// The prefix this vector's first opcode fetch selects, or `None` for M1 scope.
    ///
    /// Classification is by the byte actually at `PC`, never by the vector's *name* —
    /// the name is a label, the fetched byte is what the CPU will decode.
    pub fn prefix(&self) -> Option<Prefix> {
        Prefix::from_opcode(self.opcode_at_pc())
    }
}

/// One `tests.expected` block: the state the CPU must be in afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expectation {
    pub name: String,
    pub events: Vec<BusEvent>,
    pub registers: Registers,
    pub state: State,
    pub memory: Vec<MemoryBlock>,
}

impl Expectation {
    /// Every T-state the corpus pins an address to, as `(t_state, address)`.
    pub fn contention_points(&self) -> impl Iterator<Item = (u32, u16)> + '_ {
        self.events
            .iter()
            .filter(|event| event.kind.is_contention())
            .map(|event| (event.at_t_state, event.addr))
    }

    /// The bytes the instruction must move, in order.
    pub fn transfers(&self) -> Vec<Transfer> {
        self.events
            .iter()
            .filter(|event| event.kind.is_transfer())
            .filter_map(|event| {
                event.data.map(|data| Transfer {
                    kind: event.kind,
                    addr: event.addr,
                    data,
                })
            })
            .collect()
    }
}

/// A setup paired with its expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vector {
    pub setup: Setup,
    pub expected: Expectation,
}

impl Vector {
    pub fn name(&self) -> &str {
        &self.setup.name
    }

    /// True when the first opcode fetch is un-prefixed — i.e. this vector is in M1 scope.
    pub fn is_m1_scope(&self) -> bool {
        self.setup.prefix().is_none()
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// The directory the corpus is expected in: `<workspace>/testdata/fuse`.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("testdata")
        .join("fuse")
}

/// Load and pair the whole corpus.
///
/// Returns `Ok(None)` when the files are simply not there — they are gitignored and
/// fetched on demand (see `testdata/README.md`), so absence is a normal state of a
/// fresh clone, not a malformed corpus. `Err` means the files exist but are broken,
/// which is always a real problem worth failing on.
pub fn load_corpus(dir: &Path) -> Result<Option<Vec<Vector>>, ParseError> {
    let setup_path = dir.join(SETUP_FILE);
    let expected_path = dir.join(EXPECTED_FILE);
    if !setup_path.is_file() || !expected_path.is_file() {
        return Ok(None);
    }

    let read = |path: &Path| -> Result<String, ParseError> {
        std::fs::read_to_string(path).map_err(|err| ParseError {
            file: path.display().to_string(),
            line_no: 0,
            line: String::new(),
            reason: format!("cannot read the corpus file: {err}"),
        })
    };

    let setups = parse_setups(SETUP_FILE, &read(&setup_path)?)?;
    let expectations = parse_expectations(EXPECTED_FILE, &read(&expected_path)?)?;
    pair(setups, expectations).map(Some)
}

/// Set this environment variable to `1` in CI to turn "corpus absent" from a skip into
/// a hard failure. Locally, absence is normal; in CI, a silently skipped conformance
/// suite is a green tick that proves nothing.
pub const REQUIRE_CORPUS_ENV: &str = "Z80_FUSE_REQUIRED";

/// The corpus, or a printed explanation of why there isn't one.
///
/// This is the single decision point for "what happens when `testdata/fuse` is missing",
/// shared by every test binary so they cannot drift apart:
///
/// * present and well-formed -> `Some(vectors)`
/// * present and malformed   -> panic (a broken corpus is never acceptable)
/// * absent, normally        -> `None`, with a message naming the fetch instructions
/// * absent, with [`REQUIRE_CORPUS_ENV`] set -> panic
pub fn corpus_or_skip() -> Option<Vec<Vector>> {
    let dir = corpus_dir();
    let loaded = load_corpus(&dir).unwrap_or_else(|err| panic!("malformed FUSE corpus.\n{err}"));

    if let Some(vectors) = loaded {
        return Some(vectors);
    }

    let required = std::env::var(REQUIRE_CORPUS_ENV).is_ok_and(|value| value == "1");
    let message = format!(
        "FUSE corpus not found in {}.\n\
         The vectors are gitignored and fetched on demand — see testdata/README.md.\n\
         Set {REQUIRE_CORPUS_ENV}=1 to make this a hard failure (do this in CI).",
        dir.display()
    );
    assert!(!required, "{message}");
    println!("SKIPPING conformance run: {message}");
    None
}

/// Zip the two halves, insisting they describe the same vectors in the same order.
pub fn pair(setups: Vec<Setup>, expectations: Vec<Expectation>) -> Result<Vec<Vector>, ParseError> {
    if setups.len() != expectations.len() {
        return Err(ParseError {
            file: format!("{SETUP_FILE} / {EXPECTED_FILE}"),
            line_no: 0,
            line: String::new(),
            reason: format!(
                "corpus halves disagree on length: {} setups vs {} expectations",
                setups.len(),
                expectations.len()
            ),
        });
    }

    let mut vectors = Vec::with_capacity(setups.len());
    for (index, (setup, expected)) in setups.into_iter().zip(expectations).enumerate() {
        if setup.name != expected.name {
            return Err(ParseError {
                file: format!("{SETUP_FILE} / {EXPECTED_FILE}"),
                line_no: 0,
                line: String::new(),
                reason: format!(
                    "vector {index} is named {:?} in {SETUP_FILE} but {:?} in {EXPECTED_FILE}",
                    setup.name, expected.name
                ),
            });
        }
        vectors.push(Vector { setup, expected });
    }
    Ok(vectors)
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// One physical line, carrying its own provenance so any field error can name it.
#[derive(Clone, Copy)]
struct Line<'a> {
    file: &'a str,
    no: usize,
    text: &'a str,
}

impl<'a> Line<'a> {
    fn fields(&self) -> Vec<&'a str> {
        self.text.split_whitespace().collect()
    }

    fn err(&self, reason: impl Into<String>) -> ParseError {
        ParseError {
            file: self.file.to_owned(),
            line_no: self.no,
            line: self.text.to_owned(),
            reason: reason.into(),
        }
    }

    fn radix<T: TryFrom<u32>>(&self, token: &str, radix: u32, what: &str) -> Result<T, ParseError> {
        let raw = u32::from_str_radix(token, radix)
            .map_err(|err| self.err(format!("{what}: {token:?} is not base-{radix} ({err})")))?;
        T::try_from(raw).map_err(|_| self.err(format!("{what}: {token:?} is out of range")))
    }

    fn hex16(&self, token: &str, what: &str) -> Result<u16, ParseError> {
        self.radix(token, 16, what)
    }

    fn hex8(&self, token: &str, what: &str) -> Result<u8, ParseError> {
        self.radix(token, 16, what)
    }

    fn decimal(&self, token: &str, what: &str) -> Result<u32, ParseError> {
        self.radix(token, 10, what)
    }

    fn boolean(&self, token: &str, what: &str) -> Result<bool, ParseError> {
        match token {
            "0" => Ok(false),
            "1" => Ok(true),
            other => Err(self.err(format!("{what}: expected 0 or 1, found {other:?}"))),
        }
    }
}

/// Split a corpus file into blank-line-separated blocks of numbered lines.
fn split_blocks<'a>(file: &'a str, text: &'a str) -> Vec<Vec<Line<'a>>> {
    let mut blocks = Vec::new();
    let mut current: Vec<Line<'a>> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        if raw.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(Line {
            file,
            no: index + 1,
            text: raw,
        });
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

fn parse_registers(line: Line<'_>) -> Result<Registers, ParseError> {
    let fields = line.fields();
    if fields.len() != REGISTER_FIELDS {
        return Err(line.err(format!(
            "register line: expected {REGISTER_FIELDS} values, found {}",
            fields.len()
        )));
    }
    let mut values = [0u16; REGISTER_FIELDS];
    for (slot, token) in values.iter_mut().zip(&fields) {
        *slot = line.hex16(token, "register line")?;
    }
    Ok(Registers {
        af: values[0],
        bc: values[1],
        de: values[2],
        hl: values[3],
        af_shadow: values[4],
        bc_shadow: values[5],
        de_shadow: values[6],
        hl_shadow: values[7],
        ix: values[8],
        iy: values[9],
        sp: values[10],
        pc: values[11],
    })
}

fn parse_state(line: Line<'_>) -> Result<State, ParseError> {
    let fields = line.fields();
    if fields.len() != STATE_FIELDS {
        return Err(line.err(format!(
            "state line: expected {STATE_FIELDS} values (I R IFF1 IFF2 IM halted tstates), found {}",
            fields.len()
        )));
    }
    let im = line.decimal(fields[4], "state line: IM")?;
    if im > 2 {
        return Err(line.err(format!("state line: IM must be 0, 1 or 2, found {im}")));
    }
    Ok(State {
        i: line.hex8(fields[0], "state line: I")?,
        r: line.hex8(fields[1], "state line: R")?,
        iff1: line.boolean(fields[2], "state line: IFF1")?,
        iff2: line.boolean(fields[3], "state line: IFF2")?,
        im: im as u8,
        halted: line.boolean(fields[5], "state line: halted")?,
        t_states: line.decimal(fields[6], "state line: tstates")?,
    })
}

/// `<addr16> <byte8>... -1`
fn parse_memory_block(line: Line<'_>) -> Result<MemoryBlock, ParseError> {
    let fields = line.fields();
    let Some((last, head)) = fields.split_last() else {
        return Err(line.err("memory line: empty"));
    };
    if *last != END_OF_MEMORY {
        return Err(line.err(format!(
            "memory line: must end with the {END_OF_MEMORY} sentinel, found {last:?}"
        )));
    }
    let Some((addr, byte_tokens)) = head.split_first() else {
        return Err(line.err("memory line: missing the base address"));
    };
    let mut bytes = Vec::with_capacity(byte_tokens.len());
    for token in byte_tokens {
        bytes.push(line.hex8(token, "memory line: byte")?);
    }
    Ok(MemoryBlock {
        start: line.hex16(addr, "memory line: base address")?,
        bytes,
    })
}

/// A line is a bus event when its second field names an event kind. That grammar test
/// is used rather than "starts with whitespace" because indentation is a rendering
/// detail and the token is the actual signal.
fn as_event(line: Line<'_>) -> Result<Option<BusEvent>, ParseError> {
    let fields = line.fields();
    let Some(kind) = fields.get(1).and_then(|token| EventKind::from_token(token)) else {
        return Ok(None);
    };
    if fields.len() < 3 || fields.len() > 4 {
        return Err(line.err(format!(
            "event line: expected `<time> {} <addr> [<data>]`, found {} fields",
            kind.token(),
            fields.len()
        )));
    }
    let data = match fields.get(3) {
        Some(token) => Some(line.hex8(token, "event line: data")?),
        None => None,
    };
    Ok(Some(BusEvent {
        at_t_state: line.decimal(fields[0], "event line: time")?,
        kind,
        addr: line.hex16(fields[2], "event line: address")?,
        data,
    }))
}

/// Parse the whole of `tests.in`.
pub fn parse_setups(file: &str, text: &str) -> Result<Vec<Setup>, ParseError> {
    let blocks = split_blocks(file, text);
    let mut setups = Vec::with_capacity(blocks.len());
    for block in blocks {
        setups.push(parse_setup(&block)?);
    }
    Ok(setups)
}

fn parse_setup(block: &[Line<'_>]) -> Result<Setup, ParseError> {
    let [name, registers, state, rest @ ..] = block else {
        return Err(block[0].err(format!(
            "setup block: expected at least a name, a register line and a state line, found {} lines",
            block.len()
        )));
    };

    let Some((terminator, memory_lines)) = rest.split_last() else {
        return Err(state.err(format!(
            "setup block: missing the {END_OF_MEMORY} end-of-memory sentinel"
        )));
    };
    if terminator.text.trim() != END_OF_MEMORY {
        return Err(terminator.err(format!(
            "setup block: expected the {END_OF_MEMORY} end-of-memory sentinel"
        )));
    }

    let mut memory = Vec::with_capacity(memory_lines.len());
    for line in memory_lines {
        memory.push(parse_memory_block(*line)?);
    }

    Ok(Setup {
        name: name.text.trim().to_owned(),
        registers: parse_registers(*registers)?,
        state: parse_state(*state)?,
        memory,
    })
}

/// Parse the whole of `tests.expected`.
pub fn parse_expectations(file: &str, text: &str) -> Result<Vec<Expectation>, ParseError> {
    let blocks = split_blocks(file, text);
    let mut expectations = Vec::with_capacity(blocks.len());
    for block in blocks {
        expectations.push(parse_expectation(&block)?);
    }
    Ok(expectations)
}

fn parse_expectation(block: &[Line<'_>]) -> Result<Expectation, ParseError> {
    let Some((name, body)) = block.split_first() else {
        return Err(ParseError {
            file: String::from(EXPECTED_FILE),
            line_no: 0,
            line: String::new(),
            reason: String::from("expectation block: empty"),
        });
    };

    let mut events = Vec::new();
    let mut rest = body;
    while let Some((line, tail)) = rest.split_first() {
        match as_event(*line)? {
            Some(event) => {
                events.push(event);
                rest = tail;
            }
            None => break,
        }
    }

    let [registers, state, memory_lines @ ..] = rest else {
        return Err(name.err(format!(
            "expectation block {:?}: missing the register and/or state line after {} event(s)",
            name.text.trim(),
            events.len()
        )));
    };

    let mut memory = Vec::with_capacity(memory_lines.len());
    for line in memory_lines {
        memory.push(parse_memory_block(*line)?);
    }

    Ok(Expectation {
        name: name.text.trim().to_owned(),
        events,
        registers: parse_registers(*registers)?,
        state: parse_state(*state)?,
        memory,
    })
}
