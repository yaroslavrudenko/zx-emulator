//! `path:line` citations, and how far a gate can follow one.
//!
//! # A bare line number is a citation with no redundancy
//!
//! This repository holds a hundred-odd coordinates of the form `<path>:<number>`, pointing
//! into files that grow by hundreds of lines in a day. Nothing inside such a citation can be
//! compared against its target: the number either is the line the writer meant or it is not,
//! and no amount of reading either end will say which. So it cannot fail loudly. It can only
//! drift, silently, and go on being followed by readers who land on unrelated code and
//! assume they misread the sentence.
//!
//! That is a different defect from the one `cited_names.rs` grades. A phantom test name is
//! *absent* and can be proven absent. A stale line number is *present*, plausible, and wrong.
//!
//! # The drift has no author, so moving citations around cannot fix it
//!
//! There is a tempting reading of all this, and it is worth heading off because it looks like
//! the solution and is not one. The reading: citations rot because they live in *documents*,
//! and the cure is to move them into the tests — the way `crates/z80/tests/codegen.rs` carries
//! each `docs/ARCHITECTURE.md` claim it grades in its own doc comment, and the way every one
//! of its citations is still correct while the ones pointing *at* it had gone stale.
//!
//! That reading is half right, and the half it gets wrong is the expensive half.
//!
//! **Measured, during the afternoon this file was written.** `docs/M7.md` grew from 1984 lines
//! to 2022 while three people worked on unrelated things, and a citation into it that was
//! green when the session started was red when it ended. Nobody touched the citing sentence.
//! Nobody touched the cited sentence. Nobody was careless. The number was right when it was
//! written and became wrong because a file grew somewhere above it — and **no convention about
//! where the citation lives changes that by one line**.
//!
//! Co-location helps in exactly one case: where the gate *is* the fact's home. `codegen.rs`'s
//! citations hold up because the person who changes the bounds-check count opens `codegen.rs`
//! to change it. Moving `docs/STATUS.md`'s citation of a line in `crates/z80/src/instructions.rs`
//! into some test's doc comment would move it no closer at all to the thing that shifts it.
//!
//! So the only move that removes this failure mode is **removing the line number**: cite a
//! test by *name*, which `cited_names.rs` grades exactly and which nothing can silently
//! invalidate; or, where a coordinate is genuinely the right thing to write, put the anchor
//! beside it and let the anchored gate below hold it to account.
//!
//! # Two instruments, and the weaker one is the one that always applies
//!
//! **Landing.** Every citation whose path resolves in this repository must name a line the
//! file actually has, and that line must have something on it. A coordinate pointing past
//! the end of a file is wrong beyond argument; a coordinate pointing at a **blank line** is
//! wrong beyond argument too, because no sentence is ever about an empty line. This is a
//! smoke alarm rather than a proof — a citation can drift twenty lines and land on other
//! code — but it costs nothing, it applies to every citation, and it cannot produce a false
//! accusation.
//!
//! **Anchoring.** Where the prose around a citation quotes what is supposed to be at that
//! line, the quote is checkable, and then the gate is a proof rather than an alarm. This
//! reaches far fewer citations and says something much stronger about the ones it reaches:
//! it names the line the anchor is *actually* on, which turns a red into a one-character
//! fix.
//!
//! [`the_anchored_form_reaches_far_less_than_the_landing_form`] pins the gap between the two
//! populations, because the honest report of a gate's coverage is worth more than the
//! coverage, and a number in a comment is a number nobody re-runs.
//!
//! # Specimens, and the limit that no walker can pass
//!
//! **The population this gate walks contains specimens as well as pointers, and no amount of
//! parsing tells them apart — only the author can, at the moment of writing.**
//!
//! A live pointer and a coordinate quoted *because* it is broken are the same characters. The
//! documents here are full of the second kind, deliberately and to their credit:
//! `docs/STATUS.md` keeps a section of retracted references so a reader who acted on one finds
//! out that they did, `docs/M8.md` strikes superseded rows rather than deleting them, and
//! `crates/spectrum/src/lib.rs` quotes each replaced coverage verdict verbatim beside the one
//! that replaced it. A gate cannot read any of that, and a gate that guesses will eventually
//! tell somebody to repoint a coordinate whose *brokenness is the sentence's subject* — which
//! re-issues the wrong exoneration the passage was written to record. That is not a missed
//! finding, it is the gate actively making the repository worse.
//!
//! So the author marks it, with [`RETRACTION`], and the gate obeys the mark. The reason it is
//! a mark in the prose rather than a list beside the gate is written at that constant, and it
//! is the same reason this whole file exists: a list would rot, and it would rot in the
//! direction where a stale entry silences a true finding instead of raising a false one.
//!
//! The twelve-citation audit that prompted these gates reached the same place from the other
//! side. Seven of the twelve had drifted and are mechanically catchable — that is what the two
//! instruments below do. The other five named something that never existed at all, and nothing
//! could have caught those except opening the target once, at the moment of writing. Both
//! halves come back to the writer rather than the walker; the gates only make the writer's
//! lapses cheap to find.
//!
//! ## A third shape, found while writing this and deliberately left ungated
//!
//! A specimen carries a claim of its own — *this coordinate is broken* — and **that claim can
//! go stale in the opposite direction from drift**. Observed live: `docs/STATUS.md` says
//! *"~~`README.md:101`~~ is still a blank line"*, and while these gates were being written an
//! unrelated edit to `README.md` gave line 101 a sentence about `LD-BYTES`. The passage is now
//! wrong, and the landing gate is **structurally blind to it** — its whole question is whether
//! a cited line has content, and this one now does. It went from red to green by the target
//! changing, which is the same non-event that turns a good citation bad.
//!
//! Gating it would mean a second mark meaning *"I assert this coordinate is empty"*, invented
//! on a single observation, and a mark nobody has a habit of writing is a mark nobody writes.
//! It is recorded here instead, because a limit somebody has looked at and declined is worth
//! more than a limit nobody noticed — and because the next person to meet this shape should
//! know it was seen rather than missed.
//!
//! # What neither can reach, stated rather than skipped
//!
//! A citation that names a file this workspace does not contain — miniquad's sources, read
//! while writing the browser host — and a citation that names one of five files called
//! `lib.rs`. Both are declared in [`THE_GATE_CANNOT_FOLLOW`] with the reason, so that the
//! set cannot grow quietly: a *new* unfollowable citation reddens
//! [`every_citation_the_gate_cannot_follow_is_declared`].
//!
//! ## That paragraph described half the population, and the half it left out was ungraded
//!
//! It said "a citation", and every gate in this file meant by that *a path with a `:NNN`
//! after it*. A backticked path written without a line number reached none of them:
//! [`citations`] never saw it, and `cited_names.rs`'s path gate looks only under `tests/`.
//! So the declaration requirement — the thing this section is about — was enforced on a
//! citation when a number happened to follow it and on nothing otherwise, and the register
//! below looked complete because the population it was complete *for* was the smaller one.
//!
//! Two keycode files, siblings in the same dependency, cited for the same reason, settled it:
//! the X11 one was declared and the Wayland one was not, and no fact about either file
//! explained the difference. [`cited_paths`] closes that, and the register grew from
//! three groups to seven the moment it did — which is the honest measure of how much was
//! going ungraded, and why the growth arrived together with a second end on
//! [`every_declared_unfollowable_path_is_still_unfollowable`] to keep the register from
//! becoming somewhere to put findings.

mod common;

use common::{Repository, Resolution, backtick_spans};

// ---------------------------------------------------------------------------------------
// What the gate cannot follow, declared
// ---------------------------------------------------------------------------------------

/// A group of cited paths that resolve to nothing here, and why.
struct Unfollowable {
    /// The paths, spelled as the documents spell them.
    paths: &'static [&'static str],
    /// What they are, and why no gate can chase them.
    why: &'static str,
}

/// Every cited path that does not name exactly one file in this repository.
///
/// Not exemptions — *records*. A citation nobody can follow is a real (smaller) defect, and
/// the alternative to writing it down is skipping it in a `println!`, which libtest captures
/// on success. This project has already paid for that once: with a corpus moved aside the
/// suite exited 0 with the same test count, because the notice was a captured line of text.
const THE_GATE_CANNOT_FOLLOW: [Unfollowable; 7] = [
    Unfollowable {
        paths: &[
            "src/native/windows.rs",
            "windows.rs",
            "windows/keycodes.rs",
            "src/native/apple/apple_util.rs",
            "apple_util.rs",
            "src/native/linux_x11/keycodes.rs",
            "linux_x11/keycodes.rs",
            "src/native/linux_wayland/keycodes.rs",
            "src/native/wasm/fs.rs",
            "src/native/wasm/webgl.rs",
        ],
        why: "miniquad's own sources, cited by docs/M8.md while working out what the host \
              seam had to provide. They are a dependency's files, not this repository's, and \
              their line numbers move when the dependency moves rather than when we commit. \
              The Wayland keycode file joined this list when the gate widened to bare paths: \
              it had been cited twice with no line number while the X11 file beside it was \
              declared, and the only difference between the two was the punctuation.",
    },
    Unfollowable {
        paths: &["gl.js", "js/gl.js", "js/mq_js_bundle.js", "js/README.md"],
        why: "miniquad's JavaScript shim and the directory it ships in, cited for the same \
              reason and with the same standing: files in a crate we depend on. The bundle is \
              vendored into web/ by web/build.sh, so web/mq_js_bundle.js does resolve here — \
              the js/ spelling names the copy upstream, which is the one the surrounding \
              sentences are about.",
    },
    Unfollowable {
        paths: &["macroquad-0.4.16/src/audio.rs"],
        why: "macroquad's source, read out of the vendored registry copy while working out \
              how the desktop audio backend behaves. Unfollowable twice over: it is a \
              dependency's file, and the version is spelled into the path, so it names one \
              checkout on one machine rather than anything this repository can hold.",
    },
    Unfollowable {
        paths: &[".github/workflows/ci.yml"],
        why: "The place a workflow would have to sit before GitHub would run it, and there is \
              nothing there: the workflow was written and could not be pushed, so it sits at \
              ci/ci.yml instead, which is the whole subject of ci/README.md. Every sentence \
              citing this path is describing the absence rather than pointing at a file. It \
              would be unfollowable even if it existed — the walk prunes dotted directories \
              as tooling state, which is why ci_claims.rs asks the filesystem about this one \
              directly rather than asking the walk.",
    },
    Unfollowable {
        paths: &["z00m128/zxs-rom/LICENSE.md"],
        why: "An upstream repository's path, not a path in this tree: it names where the \
              Sinclair ROM's licence lives on GitHub, which is the address a reader needs and \
              is not something any walk of this workspace can reach.",
    },
    Unfollowable {
        paths: &["machine_cycle.rs", "crates/spectrum/src/machine_cycle.rs"],
        why: "A file deleted on purpose, discussed in the past tense in four documents that \
              have to be able to name it. Recorded here rather than in a sibling of \
              cited_names.rs's DELETED_ON_PURPOSE register, and the choice is worth stating: \
              that register grades its entries from both ends, and the end that matters for a \
              deleted file — that it is still absent, so the entry is not hiding a real \
              citation — is exactly what \
              every_declared_unfollowable_path_is_still_unfollowable already asserts about \
              every path here. The end it does not have is that somebody still discusses the \
              name, and a second struct, a second constant and a second test for one entry \
              buys that at the price of two places to look. So the end was added to the \
              register that exists instead, where it grades all seven groups.",
    },
    Unfollowable {
        paths: &[
            "lib.rs",
            "src/lib.rs",
            "tests/common/mod.rs",
            "flags.rs",
            "reader.rs",
            "boot.rs",
        ],
        why: "A filename that two or more files in this workspace answer to, so a citation \
              spelled this way names none of them: five crates have a lib.rs and a src/lib.rs, \
              four have a tests/common/mod.rs, and flags.rs, reader.rs and boot.rs each name \
              two. They are followed here rather than hidden because writing them down is the \
              only way the set stops growing: this is a citation with even less redundancy \
              than a bare line number, and the honest fix is to spell the crate.",
    },
];

// ---------------------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------------------

/// File suffixes a citation can name. Everything this repository writes coordinates into.
const CITED_SUFFIXES: [&str; 7] = [".rs", ".md", ".sh", ".toml", ".js", ".html", ".yml"];

/// How far past a citation's closing backtick the next one may sit and still be its anchor.
///
/// Long enough for a connective — *"is"*, *"— "*, *"implements"*, *"carries four"* — and
/// short enough that the next clause of a sentence is out of range. It is a coarse filter
/// and it is not what makes the anchoring safe; [`anchor_line`]'s uniqueness rule is.
const ANCHOR_REACH: usize = 24;

/// How far from the cited line an anchor may sit and still count as landing on it.
///
/// A citation names the first line of what it means, and what it means can be a signature or
/// a macro call that `rustfmt` wrapped. Two lines covers that and nothing else: every stale
/// coordinate this gate has found was out by ten or more.
const WRAPPED_LINE_TOLERANCE: usize = 2;

/// The shortest anchor worth chasing. Below this a match is a coincidence.
const SHORTEST_ANCHOR: usize = 3;

/// Markdown's strikethrough — this repository's mark for text that is quoted rather than meant.
///
/// # Why a mark in the prose, and not a list kept here
///
/// **The population this gate walks contains specimens as well as pointers, and no amount of
/// parsing tells them apart.** A coordinate quoted *because* it is broken looks exactly like
/// a coordinate that has broken, and this project's documents argue about their own
/// corrections at length — `docs/STATUS.md` has a whole section of retracted references, and
/// `crates/spectrum/src/lib.rs` quotes each superseded coverage verdict verbatim beside the
/// one that replaced it. Those passages are doing the most valuable thing in the repository
/// and a gate that reddens on them is teaching people to delete gates.
///
/// The obvious answer is a registry: a list, over here, of the coordinates that are specimens.
/// It is the wrong answer, and wrong in the dangerous direction. It rots exactly as the
/// citations rot — and where a stale citation raises a false alarm, **a stale exemption
/// silences a true one**. The gate would go quiet about a real defect and nobody would know.
///
/// So the mark lives in the prose, at the point of quoting, and it is one the repository
/// already uses for precisely this: `docs/STATUS.md` strikes twenty passages,
/// `docs/M8.md` thirteen, and several already strike whole coordinates —
/// `~~`crates/frontend/src/host.rs:44`~~`. It is **self-grading by construction**, which is
/// what a registry can never be: the mark *is* the text, so it travels when the file grows,
/// and the edit that stops something being a specimen removes the mark in the same keystroke
/// and puts the citation back under the gate. There is nothing to keep in step.
///
/// The italic parenthetical was considered as a second mark and refused: `*(`…`)*` is a
/// general aside here (`*(a`, `*(empty`, `*(Memory Refresh…`) as often as it is a retraction,
/// so honouring it would silence findings rather than exempt specimens.
const RETRACTION: &str = "~~";

/// A coordinate somebody wrote, and the words next to it that might describe its target.
struct Citation {
    /// Line of the *citing* document, so a failure can be opened.
    at: usize,
    /// The path as written.
    path: String,
    /// Whether the path was inherited from earlier on the line rather than written here.
    inherited: bool,
    /// First cited line, one-based.
    first: usize,
    /// Last cited line — equal to `first` unless the citation names a range.
    last: usize,
    /// Text that may be quoting what sits at the cited line.
    anchors: Vec<String>,
}

/// What a `<path>:<number>` inside one backtick span parsed to.
struct Coordinate {
    path: Option<String>,
    first: usize,
    last: usize,
    tail: String,
}

/// Every citation in one document.
///
/// The bare `:NNN` form — `` `read_target` (`:NNN`) `` — inherits the path from the last
/// coordinate written out earlier on the same line, and **is admitted only to the anchored
/// gate**. The inheritance is a guess: a sentence recounting a coordinate it has just
/// retracted will hand the wrong file to the next number, and `docs/M8.md` contains exactly
/// such a sentence. Under the anchored gate a wrong guess costs nothing, because the anchor
/// will not be found in the wrong file and the citation drops out of reach — the anchor
/// validates the resolution as well as the line. Under the landing gate it would be a false
/// accusation, which is why it is kept out of it.
fn citations(text: &str) -> Vec<Citation> {
    text.lines()
        .enumerate()
        .flat_map(|(index, line)| citations_on(line, index + 1))
        .collect()
}

/// Every citation on one line, left to right — and the order is the point: a bare `:NNN`
/// takes its file from the last coordinate written out before it.
fn citations_on(line: &str, at: usize) -> Vec<Citation> {
    let spans = backtick_spans(line);
    let mut found = Vec::new();
    let mut inherited: Option<String> = None;
    for (position, span) in spans.iter().enumerate() {
        let Some(coordinate) = coordinate(span.body) else {
            continue;
        };
        if let Some(written) = &coordinate.path {
            inherited = Some(written.clone());
        }
        // A struck coordinate is quoted, not meant. Dropped here rather than filtered at each
        // gate, so that the safe behaviour is the one nobody has to remember: a filter applied
        // in four places is a filter forgotten in one, and forgetting it costs a false
        // accusation against a passage doing the right thing.
        if is_retracted(line, span) {
            continue;
        }
        let Some(path) = inherited.clone() else {
            continue;
        };
        found.push(Citation {
            at,
            inherited: coordinate.path.is_none(),
            anchors: anchors_beside(&coordinate, line, &spans, position),
            path,
            first: coordinate.first,
            last: coordinate.last,
        });
    }
    found
}

/// The one anchor a coordinate can be shown to own.
///
/// # Bound by punctuation first, and by proximity only when nothing else claims it
///
/// This returns **at most one** anchor, and the singularity is the fix for a real defect
/// rather than tidiness. It used to return every candidate — the in-span tail, the bracketed
/// name before, *and* the span after — and on a dense row that pairs the **n**th number with
/// the **(n+1)**th name. `docs/STATUS.md` carries exactly such a row:
///
/// > … `fn write_target` (`:NNN`) and `fn tick_read_modify_delay` (`:NNN`) all take `Target` …
///
/// The first number's bracketed name is its own and correct; the span *after* it is the next
/// citation's name, and taking both meant grading a correct coordinate against somebody
/// else's function. It reported a stale line number for a row in which every pairing was
/// right — a false accusation, which is the one output a gate must never produce, and the
/// one this file exists to prevent other people's sentences from suffering.
///
/// So the order is by **binding strength**, and the first binding wins outright:
///
/// 1. **Inside the span** — `instructions.rs:NNN fn resolve`. The citation says what is there.
/// 2. **Bracketed** — `` `name` (`:NNN`) ``. A parenthesis closing around the number, not a
///    guess about distance.
/// 3. **The span after**, and only then. Proximity is the weakest binding there is, so it is
///    consulted only when the citation has offered nothing stronger.
fn anchors_beside(
    coordinate: &Coordinate,
    line: &str,
    spans: &[common::Span<'_>],
    position: usize,
) -> Vec<String> {
    if !coordinate.tail.is_empty() {
        return vec![coordinate.tail.clone()];
    }
    if let Some(bracketed) = bracketing_anchor(line, spans, position) {
        return vec![bracketed];
    }
    following_anchor(line, spans, position)
        .into_iter()
        .collect()
}

/// The byte ranges of every struck run on one line.
///
/// Marks are paired left to right; an unclosed one closes nothing and exempts nothing, which
/// is the safe direction for a rule whose job is to *suppress* findings.
fn struck_runs(line: &str) -> Vec<(usize, usize)> {
    let marks: Vec<usize> = line.match_indices(RETRACTION).map(|(at, _)| at).collect();
    marks
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[opens, closes]| (opens, closes + RETRACTION.len()))
        .collect()
}

/// Whether a span sits wholly inside a struck run.
fn is_retracted(line: &str, span: &common::Span<'_>) -> bool {
    struck_runs(line)
        .iter()
        .any(|&(opens, closes)| span.start >= opens && span.end <= closes)
}

/// How many coordinates on one line are struck.
fn retracted_coordinates(line: &str) -> usize {
    backtick_spans(line)
        .iter()
        .filter(|span| coordinate(span.body).is_some() && is_retracted(line, span))
        .count()
}

/// Whether `name` is bracketed to `number` in the `` `name` (`:NNN`) `` shape.
fn brackets(line: &str, name: &common::Span<'_>, number: &common::Span<'_>) -> bool {
    let between = line[name.end..number.start].trim();
    (between.is_empty() || between == "(")
        && coordinate(number.body).is_some_and(|parsed| parsed.path.is_none())
}

/// The span immediately before a bare `:NNN`, when it is bracketing it.
fn bracketing_anchor(line: &str, spans: &[common::Span<'_>], position: usize) -> Option<String> {
    let previous = spans.get(position.checked_sub(1)?)?;
    brackets(line, previous, spans.get(position)?).then(|| previous.body.to_owned())
}

/// The span just after a citation, when nothing else has a stronger claim on it.
fn following_anchor(line: &str, spans: &[common::Span<'_>], position: usize) -> Option<String> {
    let next = spans.get(position + 1)?;
    // A sibling coordinate is not a description of this one.
    if coordinate(next.body).is_some() {
        return None;
    }
    // Nor is a name that the coordinate *after it* has bracketed to itself. This is the other
    // half of the pairing fix: the strong binding wins wherever it appears on the line, not
    // only when it is the one being examined.
    if spans
        .get(position + 2)
        .is_some_and(|beyond| brackets(line, next, beyond))
    {
        return None;
    }
    let between = &line[spans[position].end..next.start];
    let starts_a_new_clause = between.contains([',', ';', '.', '|']);
    (between.len() <= ANCHOR_REACH && !starts_a_new_clause).then(|| next.body.to_owned())
}

/// Every backticked path in `text` that carries no coordinate, with its line number.
///
/// # A path with no number used to be graded by nothing at all
///
/// [`citations`] sees a path only when a `:NNN` follows it, and `cited_names.rs` grades a
/// bare path only when it sits under a `tests/` directory. Between those two rules lay a
/// population neither reached: a backticked source path, written without a line number,
/// outside `tests/`. The specimen that made the gap impossible to argue with is
/// `src/native/linux_wayland/keycodes.rs` — miniquad's, cited twice, resolving to nothing and
/// declared nowhere — sitting beside `src/native/linux_x11/keycodes.rs`, which is declared in
/// [`THE_GATE_CANNOT_FOLLOW`] below. Nothing separates the two files. What separated them was
/// that somebody happened to write a number after the X11 one, and a gate that grades a path
/// when a number follows it and lets it through when one does not is grading the punctuation.
///
/// A struck span is dropped for the reason [`citations_on`] drops one: a path quoted because
/// it is gone is not a pointer, and asking for it to be repointed re-issues the wrong
/// exoneration.
fn cited_paths(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for span in backtick_spans(line) {
            if is_bare_path(span.body) && !is_retracted(line, &span) {
                found.push((index + 1, span.body.to_owned()));
            }
        }
    }
    found
}

/// Whether a span's body is a path written without a coordinate.
///
/// A suffix on its own — `` `.rs` ``, `` `.md` ``, which this repository's prose writes when
/// it means the extension rather than a file — is refused by requiring a stem in front of it.
/// A `:` is not a path byte, so a coordinate can never reach here and one citation cannot be
/// counted by both populations.
fn is_bare_path(body: &str) -> bool {
    CITED_SUFFIXES
        .iter()
        .any(|suffix| body.len() > suffix.len() && body.ends_with(suffix))
        && body.bytes().all(common::is_path_byte)
}

/// Parse one backtick span's contents as a coordinate.
fn coordinate(body: &str) -> Option<Coordinate> {
    if let Some(digits) = body.trim().strip_prefix(':') {
        let (first, last, tail) = line_range(digits)?;
        return tail.is_empty().then_some(Coordinate {
            path: None,
            first,
            last,
            tail: String::new(),
        });
    }
    let colon = path_colon(body)?;
    let start = body[..colon]
        .bytes()
        .rposition(|byte| !common::is_path_byte(byte))
        .map_or(0, |before| before + 1);
    let (first, last, tail) = line_range(&body[colon + 1..])?;
    Some(Coordinate {
        path: Some(body[start..colon].to_owned()),
        first,
        last,
        tail,
    })
}

/// The offset of the `:` that separates a file from a line number.
fn path_colon(body: &str) -> Option<usize> {
    body.bytes().enumerate().position(|(offset, byte)| {
        byte == b':'
            && CITED_SUFFIXES
                .iter()
                .any(|suffix| body[..offset].ends_with(suffix))
            && body[offset + 1..].starts_with(|c: char| c.is_ascii_digit())
    })
}

/// `123`, or `123-456`, and whatever followed it.
fn line_range(text: &str) -> Option<(usize, usize, String)> {
    let (first, rest) = leading_number(text)?;
    let Some((last, tail)) = rest.strip_prefix('-').and_then(leading_number) else {
        return Some((first, first, rest.trim().to_owned()));
    };
    Some((first, last.max(first), tail.trim().to_owned()))
}

fn leading_number(text: &str) -> Option<(usize, &str)> {
    let end = text
        .bytes()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(text.len());
    Some((text[..end].parse().ok()?, &text[end..]))
}

// ---------------------------------------------------------------------------------------
// The two verdicts, as functions of their inputs
// ---------------------------------------------------------------------------------------

/// Whether a citation names a line the file has, with something on it.
#[derive(Debug, PartialEq, Eq)]
enum Landing {
    /// The cited span holds text.
    OnContent,
    /// The file is shorter than the number.
    PastTheEnd,
    /// The file has the line and it is empty.
    OnNothing,
}

/// Grade the landing. A pure function of the citation and the file, so
/// [`the_line_gate_is_capable_of_failing`] can exercise every outcome without touching a
/// real file.
fn landing(first: usize, last: usize, lines: &[&str]) -> Landing {
    if last > lines.len() || first == 0 {
        return Landing::PastTheEnd;
    }
    match lines[first - 1..last]
        .iter()
        .any(|line| !line.trim().is_empty())
    {
        true => Landing::OnContent,
        false => Landing::OnNothing,
    }
}

/// Where an anchor is, when it is somewhere unambiguous.
///
/// **Uniqueness is what makes the anchored gate safe.** A string that appears once in a file
/// is a coordinate; a string that appears five times is a concept, and prose naming a concept
/// next to a citation is not claiming the concept lives at that line. The tree contains both.
/// *"`instructions.rs:NNN` is `shuffle(self.regs.a(), memory)`"* quotes a line that occurs
/// once and is checkable; *"(`:NNN`) does the same for `DDCB`"* names something that occurs
/// four times and is not a claim about that line at all. Without this rule the second shape
/// produces confident false accusations, which is the one thing a gate must never do — both
/// shapes are real sentences from `docs/STATUS.md`, with the numbers taken out here so that
/// this comment does not become a citation of its own.
///
/// A bare identifier is looked up as a **definition** first, because that is what the prose
/// means by it: *"`read_target` (`:NNN`)"* says the function is declared there, not that the
/// name first appears there.
fn anchor_line(lines: &[&str], anchor: &str) -> Option<(usize, String)> {
    if anchor.len() < SHORTEST_ANCHOR {
        return None;
    }
    if is_plain_identifier(anchor) {
        let declaration = format!("fn {anchor}");
        if let Some(at) = only_line_containing(lines, &declaration) {
            return Some((at, declaration));
        }
    }
    only_line_containing(lines, anchor).map(|at| (at, anchor.to_owned()))
}

fn only_line_containing(lines: &[&str], needle: &str) -> Option<usize> {
    let mut hits = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(needle));
    let (index, _) = hits.next()?;
    hits.next().is_none().then_some(index + 1)
}

fn is_plain_identifier(text: &str) -> bool {
    text.bytes().all(common::is_identifier_byte)
        && text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
}

// ---------------------------------------------------------------------------------------
// The gates
// ---------------------------------------------------------------------------------------

/// The fewest citations the walk must find before its silence means anything.
const FEWEST_CITATIONS: usize = 60;

#[test]
fn every_line_citation_lands_on_a_line_that_has_something_on_it() {
    let repository = Repository::walk();
    repository.assert_the_walk_found_the_repository();

    let mut graded = 0;
    let mut adrift = Vec::new();
    for document in repository.documents() {
        for citation in citations(&document.text).iter().filter(|c| !c.inherited) {
            let Resolution::Exact(target) = repository.resolve(&document.path, &citation.path)
            else {
                continue;
            };
            graded += 1;
            let text = repository.read(&target);
            let target_lines: Vec<&str> = text.lines().collect();
            let verdict = landing(citation.first, citation.last, &target_lines);
            if verdict != Landing::OnContent {
                adrift.push(format!(
                    "{}:{} cites {}:{} — {target} has {} lines and that one is {}",
                    document.path,
                    citation.at,
                    citation.path,
                    citation.first,
                    target_lines.len(),
                    if verdict == Landing::PastTheEnd {
                        "not there"
                    } else {
                        "empty"
                    },
                ));
            }
        }
    }

    assert!(
        graded >= FEWEST_CITATIONS,
        "only {graded} citations were graded; the extractor is not reading the documents",
    );
    assert!(
        adrift.is_empty(),
        "{} citation(s) point at nothing:\n  {}\n\n\
         Each of these is one of two things and only the person who wrote the sentence can \
         say which.\n\
         \x20 - **A pointer that drifted.** Correct the number — and write the anchor beside \
         it while you are there, which is what moves it from this alarm to a proof.\n\
         \x20 - **A specimen**: a coordinate quoted *because* it is broken. This repository's \
         documents do that at length, and repointing one at whatever occupies that line today \
         re-issues the wrong exoneration the passage exists to record. Mark a specimen by \
         striking it — {RETRACTION}`like this`{RETRACTION} — the way `docs/M8.md` already \
         strikes its superseded rows. The mark travels with the text, so unlike a list kept \
         beside the gate it cannot go stale and start silencing real findings.",
        adrift.len(),
        adrift.join("\n  "),
    );
}

#[test]
fn every_anchored_line_citation_lands_on_its_anchor() {
    let repository = Repository::walk();
    repository.assert_the_walk_found_the_repository();

    let mut anchored = 0;
    let mut adrift = Vec::new();
    for document in repository.documents() {
        for citation in citations(&document.text) {
            let Resolution::Exact(target) = repository.resolve(&document.path, &citation.path)
            else {
                continue;
            };
            let text = repository.read(&target);
            let lines: Vec<&str> = text.lines().collect();
            for anchor in &citation.anchors {
                let Some((at, needle)) = anchor_line(&lines, anchor) else {
                    continue;
                };
                anchored += 1;
                if !within_tolerance(at, citation.first, citation.last) {
                    adrift.push(format!(
                        "{}:{} cites {}:{} as `{needle}`, which is at {target}:{at}",
                        document.path, citation.at, citation.path, citation.first,
                    ));
                }
            }
        }
    }

    // Zero anchored citations is not a green, it is the documents having stopped quoting what
    // they cite — which is the redundancy this instrument runs on, and losing it is the
    // regression rather than the relief.
    assert!(
        anchored >= 3,
        "only {anchored} citation(s) carried a checkable anchor; the strongest half of this \
         gate is now grading almost nothing",
    );
    assert!(
        adrift.is_empty(),
        "{} citation(s) name a line their own sentence contradicts:\n  {}\n\n\
         The line the anchor is actually on is given above, so a drifted pointer is a \
         one-number correction rather than an investigation. Before making it, check that the \
         sentence is pointing rather than quoting: a passage recounting a coordinate it has \
         already retracted wants the retraction mark ({RETRACTION}`…`{RETRACTION}), not a \
         fresh number.",
        adrift.len(),
        adrift.join("\n  "),
    );
}

/// The fewest struck coordinates this repository can plausibly hold.
///
/// A floor, for the same reason every other walk here carries one. The retraction mark fails
/// *open* if the detector breaks — struck citations would be graded again and raise false
/// alarms rather than hide real ones — but a detector that silently stopped matching would
/// take the whole exemption with it, and the first anyone knew would be a red on a passage
/// that has been right all along.
const FEWEST_RETRACTIONS: usize = 3;

#[test]
fn retracted_coordinates_are_left_alone_and_the_mark_is_still_in_use() {
    let repository = Repository::walk();
    repository.assert_the_walk_found_the_repository();

    let struck: usize = repository
        .documents()
        .iter()
        .flat_map(|document| document.text.lines())
        .map(retracted_coordinates)
        .sum();

    assert!(
        struck >= FEWEST_RETRACTIONS,
        "only {struck} struck coordinate(s) found; either this repository stopped marking its \
         retracted references — in which case every one of them is about to be reported as \
         drift — or the mark is no longer being recognised. Both need looking at before this \
         gate's silences can be trusted.",
    );
}

fn within_tolerance(at: usize, first: usize, last: usize) -> bool {
    at + WRAPPED_LINE_TOLERANCE >= first && at <= last + WRAPPED_LINE_TOLERANCE
}

#[test]
fn every_citation_the_gate_cannot_follow_is_declared() {
    let repository = Repository::walk();
    repository.assert_the_walk_found_the_repository();
    let declared: Vec<&str> = THE_GATE_CANNOT_FOLLOW
        .iter()
        .flat_map(|group| group.paths.iter().copied())
        .collect();

    let mut undeclared = Vec::new();
    for document in repository.documents() {
        // Both populations, through one rule. A coordinate's path and a path written with no
        // coordinate are the same claim about the same file, and grading only the first is
        // what left `cited_paths`'s specimen undeclared for as long as it was.
        let coordinates = citations(&document.text)
            .into_iter()
            .filter(|citation| !citation.inherited)
            .map(|citation| (citation.at, citation.path));
        for (at, path) in coordinates.chain(cited_paths(&document.text)) {
            let followed = repository.resolve(&document.path, &path);
            if matches!(followed, Resolution::Exact(_)) || declared.contains(&path.as_str()) {
                continue;
            }
            undeclared.push(format!(
                "{}:{at} cites `{path}`, which is {}",
                document.path,
                match followed {
                    Resolution::Ambiguous(candidates) => format!("{} files here", candidates.len()),
                    _ => "no file here".to_owned(),
                },
            ));
        }
    }
    assert!(
        undeclared.is_empty(),
        "{} citation(s) name a path no gate can follow and that nothing declares:\n  {}\n\n\
         Either spell the path so it resolves, or add it to THE_GATE_CANNOT_FOLLOW with the \
         reason. Silence is the one option this repository has already paid for.",
        undeclared.len(),
        undeclared.join("\n  "),
    );
}

#[test]
fn every_declared_unfollowable_path_is_still_unfollowable() {
    // The other end of the allowlist, for the reason `cited_names.rs` gives about its own: a
    // declaration that has become false is an exemption hiding a real citation.
    //
    // There are two ends, and the register carried only the first until the gate widened to
    // bare paths and the register went from three groups to seven. A list that can only grow
    // is the quiet place a real finding goes to be silenced, and quadrupling it is exactly
    // when that stops being theoretical — so the second end came in with the growth rather
    // than after it. `cited_names.rs`'s DELETED_ON_PURPOSE has graded both ends of its own
    // list since it was written; this is the same discipline, on the list that just grew.
    let repository = Repository::walk();
    for group in &THE_GATE_CANNOT_FOLLOW {
        assert!(!group.paths.is_empty() && group.why.len() > 40);
        for path in group.paths {
            assert!(
                !matches!(
                    repository.resolve("docs/STATUS.md", path),
                    Resolution::Exact(_)
                ),
                "`{path}` is declared unfollowable and now resolves to exactly one file. {}",
                group.why,
            );
            // And that somebody still cites it. One mention is this file's own declaration,
            // so the floor is two: an entry nobody cites any more is a record of nothing, and
            // leaving it here is how the register becomes a place to put findings.
            //
            // What this end can and cannot see, since a floor that is quietly blind is worse
            // than no floor: it is a substring count, so it catches a dead entry whose
            // spelling is unique — every miniquad path, the upstream licence, the workflow —
            // and it is blind to a dead entry in the bare-filename group, because correcting
            // `flags.rs` to the crate-qualified spelling leaves the substring behind in the
            // corrected path. That group's entries are therefore held only by the first end.
            // Making the second reach them needs the mention counted as a whole cited token
            // rather than as text, which is the extraction `cited_paths` already performs and
            // is a bigger change than the one this gate was growing for.
            let mentions = repository
                .documents()
                .iter()
                .filter(|document| document.text.contains(path))
                .count();
            assert!(
                mentions >= 2,
                "`{path}` is declared unfollowable but only {mentions} file(s) mention it — \
                 this one being the declaration. Nothing cites it any more, so delete the \
                 entry rather than leaving the register carrying it.",
            );
        }
    }
}

#[test]
fn the_anchored_form_reaches_far_less_than_the_landing_form() {
    // The coverage report, as an assertion rather than a sentence — because a sentence about
    // coverage is exactly the kind of claim this whole directory exists to stop trusting.
    //
    // Most citations here carry no quote of their target at all, so most of this population
    // is graded only by the smoke alarm. Saying so out loud is the honest half of shipping
    // the instrument: the landing gate cannot tell a coordinate that drifted onto other code
    // from one that never moved.
    let repository = Repository::walk();
    let mut total = 0;
    let mut anchored = 0;
    for document in repository.documents() {
        for citation in citations(&document.text) {
            let Resolution::Exact(target) = repository.resolve(&document.path, &citation.path)
            else {
                continue;
            };
            total += 1;
            let text = repository.read(&target);
            let lines: Vec<&str> = text.lines().collect();
            if citation
                .anchors
                .iter()
                .any(|anchor| anchor_line(&lines, anchor).is_some())
            {
                anchored += 1;
            }
        }
    }
    assert!(
        total >= FEWEST_CITATIONS,
        "only {total} citations resolve at all"
    );
    assert!(
        anchored * 2 <= total,
        "{anchored} of {total} citations are anchored — more than half, which would be a \
         welcome surprise and means this assertion's premise is out of date rather than that \
         anything is broken",
    );
}

// ---------------------------------------------------------------------------------------
// The positive control
// ---------------------------------------------------------------------------------------

#[test]
fn the_line_gate_is_capable_of_failing() {
    // Synthetic on both sides — a citing sentence and a cited file, neither of them real —
    // so this control edits nothing, depends on nothing, and cannot start passing because
    // somebody reformatted a document. The coordinates are assembled rather than written for
    // the reason `cited_names.rs` records: a control containing a real-looking phantom
    // citation is caught by the gate it is a control for.
    let target: Vec<&str> = vec!["fn resolve(", "", "    body", "", "fn other("];

    // First that the instrument says yes, because an extractor returning nothing would make
    // every gate above vacuously green.
    assert_eq!(landing(1, 1, &target), Landing::OnContent);
    assert_eq!(landing(1, 3, &target), Landing::OnContent);
    assert_eq!(
        anchor_line(&target, "resolve"),
        Some((1, "fn resolve".to_owned()))
    );

    // Then each way it says no.
    //
    // `OnNothing` is the row that caught its own author, and the story is the best argument
    // this repository has for the discipline it keeps insisting on. Proving the landing gate
    // could fail meant pointing a real citation at a real blank line; the first attempt aimed
    // at line 2 of `crates/testsupport/src/lib.rs`, the run came back **green**, and the
    // obvious reading was that the gate could not fail. It was the opposite: line 2 is `//!`,
    // which is not blank, so the gate was right and the *mutation* was the thing that had not
    // mutated anything. Retargeted at line 40 — the first genuinely empty line — it went red
    // immediately.
    //
    // A mangled substitution and an unbreakable guard produce byte-identical output, and the
    // only thing separating them is checking that the property you meant to break is the one
    // you actually broke.
    assert_eq!(
        landing(2, 2, &target),
        Landing::OnNothing,
        "an empty line is not a target"
    );
    assert_eq!(
        landing(6, 6, &target),
        Landing::PastTheEnd,
        "the file has five lines"
    );
    assert_eq!(
        landing(0, 0, &target),
        Landing::PastTheEnd,
        "lines are one-based"
    );

    // And that a repeated string is refused rather than guessed at, which is the rule the
    // anchored gate's freedom from false accusations rests on.
    assert_eq!(
        anchor_line(&target, "fn "),
        None,
        "`fn ` is on two lines and anchors neither"
    );
    assert_eq!(anchor_line(&target, "absent"), None);

    // The parser, on the three shapes the documents actually use.
    let written = coordinate("instructions.rs:638 fn resolve").expect("an explicit coordinate");
    assert_eq!(written.path.as_deref(), Some("instructions.rs"));
    assert_eq!((written.first, written.last), (638, 638));
    assert_eq!(written.tail, "fn resolve");

    let ranged = coordinate("docs/M7.md:1466-1467").expect("a range");
    assert_eq!((ranged.first, ranged.last), (1466, 1467));

    let bare = coordinate(":685").expect("a bare coordinate");
    assert!(bare.path.is_none() && bare.first == 685);

    assert!(
        coordinate("crates/z80/tests/alu_flags.rs").is_none(),
        "a path is not a citation"
    );
    assert!(
        coordinate("Key::position").is_none(),
        "a Rust path is not a citation"
    );

    // Finally the whole extractor over a line in the shape `docs/STATUS.md` writes, including
    // the inheritance: two coordinates, the second of which names no file of its own.
    let sentence = format!(
        "`{}:638 fn resolve` is the point, and `read_target` (`:685`) takes it by value",
        "instructions.rs",
    );
    let found = citations(&sentence);
    assert_eq!(found.len(), 2, "both coordinates on the line must be seen");
    assert!(!found[0].inherited && found[0].anchors.contains(&"fn resolve".to_owned()));
    assert!(
        found[1].inherited && found[1].anchors.contains(&"read_target".to_owned()),
        "a bare coordinate must inherit the path and take the name beside it as its anchor",
    );

    // And the population that carries no coordinate at all, which reached none of the gates
    // above until `cited_paths` was written and which therefore has never been watched fail.
    // Assembled for the reason everything above is: a real-looking path written out here is a
    // citation of this file's own, and a control caught by the gate it is a control for is
    // worse than no control.
    let bare = format!("the shim lives in `{}/{}`", "js", "gl.js");
    assert_eq!(
        cited_paths(&bare),
        vec![(1, "js/gl.js".to_owned())],
        "a path written without a line number is still a citation and must be seen as one",
    );

    // Then each way it says no, and each of the three is a false accusation it would
    // otherwise make: an extension named in prose, a coordinate already graded by the other
    // population, and a path quoted because it is gone.
    assert!(
        cited_paths("every `.md` and `.rs` file").is_empty(),
        "a bare suffix names the extension rather than a file",
    );
    assert!(
        cited_paths(&format!("`{}:638 fn resolve`", "instructions.rs")).is_empty(),
        "a coordinate belongs to the other population and must not be counted twice",
    );
    assert!(
        cited_paths(&format!(
            "{RETRACTION}`{}/{}`{RETRACTION} — moved upstream",
            "js", "gl.js"
        ))
        .is_empty(),
        "a struck path is quoted history, exactly as a struck coordinate is",
    );
}

#[test]
fn each_number_on_a_crowded_line_binds_to_its_own_name() {
    // The regression test for a live false accusation. `docs/STATUS.md` carries a row listing
    // nine functions with nine line numbers, alternating; the gate reported one of them as
    // stale because it paired the **n**th number with the **(n+1)**th name, and every pairing
    // in that row was in fact correct.
    //
    // Built here rather than read from the document, so it stays a test of the *rule* when
    // somebody rewrites the row — and so the numbers can be deliberately interleaved in a way
    // that makes an ordinal mismatch impossible to miss.
    let numbers = [820, 828, 838];
    let names = [
        "fn read_target",
        "fn write_target",
        "fn tick_read_modify_delay",
    ];
    let row = format!(
        "`{}:773 fn resolve` is the point; `{}` (`:{}`), `{}` (`:{}`) and `{}` (`:{}`) all \
         take `Target` by value",
        "instructions.rs", names[0], numbers[0], names[1], numbers[1], names[2], numbers[2],
    );

    let found = citations(&row);
    assert_eq!(
        found.len(),
        4,
        "one coordinate written out and three bracketed"
    );

    // The first is anchored from inside its own span and must take nothing else — the name
    // that follows it belongs to the next citation.
    assert_eq!(found[0].anchors, vec!["fn resolve".to_owned()]);

    // And each bracketed number takes the name it is bracketed to, not the one after it.
    for (index, (number, name)) in numbers.iter().zip(names).enumerate() {
        let citation = &found[index + 1];
        assert_eq!(citation.first, *number);
        assert_eq!(
            citation.anchors,
            vec![name.to_owned()],
            "`{name}` (`:{number}`) must bind to itself and to nothing else; pairing it with \
             the following name is the defect this test exists for",
        );
    }
}

#[test]
fn a_struck_coordinate_is_quoted_rather_than_meant() {
    // The specimen mark, proven in both directions. A control that only showed the mark
    // suppressing something would not show that it suppresses *only* what it is pointed at,
    // and an exemption that over-reaches silences real findings — the failure this mark was
    // chosen over a registry to avoid.
    let live = format!("the row cites `{}:665` for the beeper", "docs/M7.md");
    assert_eq!(
        citations(&live).len(),
        1,
        "an unstruck coordinate is a citation"
    );

    let struck = format!(
        "{RETRACTION}`{}:665`{RETRACTION} — audio was reassigned to M8",
        "docs/M7.md",
    );
    assert!(
        citations(&struck).is_empty(),
        "a struck coordinate is quoted history and must leave the population entirely",
    );

    // The suppression stops at the closing mark. A struck row that goes on to cite something
    // live must still have that live citation graded.
    let both = format!(
        "{RETRACTION}`{}:665`{RETRACTION} was the claim; `{}:1` is where it went",
        "docs/M7.md", "docs/M8.md",
    );
    let survivors = citations(&both);
    assert_eq!(survivors.len(), 1, "only the struck half is suppressed");
    assert_eq!(survivors[0].path, "docs/M8.md");

    // An unclosed mark closes nothing and must exempt nothing, because a rule that suppresses
    // findings has to fail in the direction that reports too much rather than too little.
    let unclosed = format!(
        "{RETRACTION}`{}:665` and then no closing mark",
        "docs/M7.md"
    );
    assert_eq!(
        citations(&unclosed).len(),
        1,
        "a dangling retraction mark must not silence the rest of the line",
    );
}
