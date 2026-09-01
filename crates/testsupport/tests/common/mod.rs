//! The repository as a citation gate sees it: a list of files, their text, and the rule for
//! turning a path somebody wrote in prose into one of them.
//!
//! # Why the walk lives here and not in `crates/testsupport/src`
//!
//! This crate's library is *"where the corpora live, and what happens when one of them is
//! missing"*, and that is one task. A repository walk is a second one. Putting it behind
//! `pub` in `src/lib.rs` would widen the crate's stated job and its public surface for the
//! benefit of three test binaries that all live four directories away — so it goes in the
//! integration tree, which is the shape `crates/z80/tests/common/`,
//! `crates/spectrum/tests/common/` and `crates/frontend/tests/common/` already use for
//! exactly this reason.
//!
//! # Why the walk is here rather than in the crate whose files it reads
//!
//! Every citation in this repository crosses a crate boundary: `docs/STATUS.md` cites
//! `crates/z80`, `crates/z80/tests/codegen.rs` cites `docs/ARCHITECTURE.md`, and a document
//! at the root cites all of them. There is no crate that owns the population, so a
//! per-crate gate would either duplicate the walk five times or make each crate's suite
//! depend on documents it does not own — the coupling `crates/frontend/tests/portability.rs`
//! declines to create when it refuses to sweep a sibling crate's `src/`.
//!
//! `crates/testsupport` is the one crate here whose subject is already the whole workspace.
//! It is `publish = false`, it already computes the workspace root and already gates that
//! computation with `the_testdata_root_resolves_to_the_workspace`, so the coupling to the
//! repository's layout is one this crate has rather than one it acquires.

#![allow(dead_code)] // Each test binary compiles this module and uses only the part it needs.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A file the gates read as text.
pub struct Document {
    /// Repository-relative, with `/` separators — the spelling a citation uses.
    pub path: String,
    /// The whole file.
    pub text: String,
}

/// Whether `byte` can appear in a path as this repository's prose writes one.
///
/// Here rather than in each gate because both of them need it and neither owns it: it is what
/// separates a path from the two other things that get written in backticks and end in `.rs`
/// — a **glob** (`m7_*.rs`, a family named rather than a file cited) and a **command**
/// (`ls -1 …`, `git log -1 -- …`). Both are correct prose, neither is a citation, and both
/// were caught by the name gate on its first run against the tree.
#[must_use]
pub fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'/' | b'-')
}

/// Whether `byte` can appear in a Rust identifier.
#[must_use]
pub fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// What a path written in prose turned out to name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Exactly one file in this repository.
    Exact(String),
    /// More than one, so the citation does not say which.
    Ambiguous(Vec<String>),
    /// None — a file outside this repository, or a path that no longer exists.
    Unknown,
}

/// The repository, walked once.
pub struct Repository {
    root: PathBuf,
    files: BTreeSet<String>,
    documents: Vec<Document>,
}

/// Directories that are not repository content.
///
/// A leading `.` is the convention for tooling state — `.git`, and on this machine a
/// `.agent-workspace` holding whole *copies* of `crates/`, which a naive walk pulls in and
/// which would give every function two definitions and every citation two homes. `target` is
/// build output. Nothing else is excluded, so a new document at the root is scanned the day
/// it appears rather than the day somebody remembers to list it.
fn is_pruned(name: &str) -> bool {
    name.starts_with('.') || name == "target"
}

/// The fewest files this repository can plausibly hold.
///
/// A floor rather than an equality, for the reason
/// `crates/frontend/tests/portability.rs` gives about its own: it only has to separate
/// *present* from *absent*, and a walk that found nothing reads as **0**. The tree held 134
/// `.md` and `.rs` files when this was written.
const FEWEST_DOCUMENTS: usize = 90;

impl Repository {
    /// Walk the workspace once.
    ///
    /// # Panics
    ///
    /// If a directory cannot be read, or a `.md` / `.rs` file is not valid UTF-8. Both are
    /// louder failures than silently walking less of the tree than the caller believes.
    #[must_use]
    pub fn walk() -> Self {
        let root = workspace_root();
        let mut repository = Self {
            files: BTreeSet::new(),
            documents: Vec::new(),
            root,
        };
        repository.descend(&repository.root.clone());
        repository.documents.sort_by(|a, b| a.path.cmp(&b.path));
        repository
    }

    fn descend(&mut self, directory: &Path) {
        let entries = std::fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("a readable directory entry").path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if path.is_dir() {
                if !is_pruned(name) {
                    self.descend(&path);
                }
            } else {
                self.admit(&path);
            }
        }
    }

    fn admit(&mut self, path: &Path) {
        let Some(relative) = self.relative(path) else {
            return;
        };
        let is_document = matches!(path.extension().and_then(|e| e.to_str()), Some("md" | "rs"));
        if is_document {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"));
            self.documents.push(Document {
                path: relative.clone(),
                text,
            });
        }
        self.files.insert(relative);
    }

    fn relative(&self, path: &Path) -> Option<String> {
        let suffix = path.strip_prefix(&self.root).ok()?;
        Some(suffix.to_str()?.replace(std::path::MAIN_SEPARATOR, "/"))
    }

    /// Every `.md` and `.rs` file, sorted, so a failure message is the same on two machines.
    #[must_use]
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }

    /// Whether `relative` names a file in this repository.
    #[must_use]
    pub fn contains(&self, relative: &str) -> bool {
        self.files.contains(relative)
    }

    /// The text of a file this repository contains.
    ///
    /// # Panics
    ///
    /// If the file is absent or is not valid UTF-8.
    #[must_use]
    pub fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.root.join(relative))
            .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
    }

    /// Turn a path as written in prose into the file it names.
    ///
    /// Three rules, tried in this order, and the order is the argument:
    ///
    /// 1. **Repository-relative as written.** `crates/spectrum/src/ula.rs`. A path that
    ///    resolves from the root is what the writer wrote and needs no interpretation.
    /// 2. **Relative to the citing file's own directory.** `M7.md` inside `docs/` is
    ///    `docs/M7.md`. This is second rather than first so that `README.md` in a document
    ///    under `docs/` reaches the root `README.md`, which is what it means — `docs/` has
    ///    no `README.md`, and if it ever gains one this order will have to be revisited
    ///    deliberately rather than silently.
    /// 3. **A unique suffix, on component boundaries.** `ula.rs` is
    ///    `crates/spectrum/src/ula.rs` because nothing else ends in it. `lib.rs` is five
    ///    files, so it resolves to nothing and is reported as such: a citation that names
    ///    one of five files is not a citation a machine can follow, and guessing which one
    ///    would be the gate inventing the redundancy the citation lacks.
    #[must_use]
    pub fn resolve(&self, citing: &str, cited: &str) -> Resolution {
        if self.contains(cited) {
            return Resolution::Exact(cited.to_owned());
        }
        if let Some(sibling) = self.sibling_of(citing, cited)
            && self.contains(&sibling)
        {
            return Resolution::Exact(sibling);
        }
        let suffix = format!("/{cited}");
        let matches: Vec<String> = self
            .files
            .iter()
            .filter(|candidate| candidate.ends_with(&suffix))
            .cloned()
            .collect();
        match matches.len() {
            0 => Resolution::Unknown,
            1 => Resolution::Exact(matches[0].clone()),
            _ => Resolution::Ambiguous(matches),
        }
    }

    /// `cited` read as a path relative to the directory `citing` lives in.
    fn sibling_of(&self, citing: &str, cited: &str) -> Option<String> {
        let directory = citing.rsplit_once('/').map(|(head, _)| head)?;
        Some(format!("{directory}/{cited}"))
    }

    /// The assertion every walk in this repository carries: that it looked at the subject.
    ///
    /// Without it a walk that visited nothing produces a confident green for a question it
    /// never asked — `docs/STATUS.md`'s *"a count of zero and an absence of the subject are
    /// the same observation"*.
    ///
    /// # Panics
    ///
    /// If fewer than [`FEWEST_DOCUMENTS`] files were found.
    pub fn assert_the_walk_found_the_repository(&self) {
        assert!(
            self.documents.len() >= FEWEST_DOCUMENTS,
            "the walk found {} documents under {}; it is not looking at the repository",
            self.documents.len(),
            self.root.display(),
        );
    }
}

/// `<workspace>` — this crate sits at `crates/testsupport`, the same depth as every other.
#[must_use]
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("the workspace root, two directories above this crate")
}

/// A backticked run of text, and where it sat on its line.
pub struct Span<'line> {
    /// Byte offset of the opening backtick.
    pub start: usize,
    /// Byte offset one past the closing backtick.
    pub end: usize,
    /// What was between them.
    pub body: &'line str,
}

/// Every `` `…` `` on one line, left to right.
///
/// Backticks rather than a language-aware parse, because the population this grades is
/// prose: a citation is a thing a person wrote in a sentence, in Markdown or in a `//!`
/// block, and in both places the convention for "this is code, not English" is a backtick.
/// A span that opens and never closes is not a span, so the trailing fragment is dropped.
#[must_use]
pub fn backtick_spans(line: &str) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut rest = line;
    let mut consumed = 0;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else {
            break;
        };
        spans.push(Span {
            start: consumed + open,
            end: consumed + open + 1 + close + 1,
            body: &after[..close],
        });
        consumed += open + 1 + close + 1;
        rest = &after[close + 1..];
    }
    spans
}
