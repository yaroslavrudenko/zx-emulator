//! A gate may only mention a pipeline by deferring to the file that says whether one exists.
//!
//! # The claim, and why it is the expensive kind of wrong
//!
//! Three gates in this workspace carry `#[ignore]` because they are too slow for a default
//! `cargo test` — the two `zex` exercisers and the MEMPTR oracle. Each explains itself in its
//! ignore text, and each of the three used to end with the same six words: that a pipeline
//! runs it the other way.
//!
//! There is no pipeline. `ci/README.md` opens with the reason and does not soften it:
//! *"Until it is installed, this repository has no CI."* The workflow was written, reviewed
//! and committed, and could never be pushed, because the credential available to the sessions
//! working here does not carry GitHub's `workflow` scope — so it sits at `ci/ci.yml`, outside
//! `.github/workflows/`, where nothing consults it.
//!
//! An ignore reason is read at exactly the moment somebody is deciding whether a skipped test
//! matters. A sentence there saying it runs elsewhere is the most effective possible way to
//! stop them running it — this project's own headline defect, recorded in `docs/STATUS.md` as
//! *"an `#[ignore]`d gate that no pipeline executes is not a gate"*, said in the one place a
//! reader will believe it.
//!
//! # The rule, and why it is not "do not say CI"
//!
//! **If you have come here to simplify this into "the reason must not contain `CI`", read
//! this paragraph first: that gate was specified, and it was wrong before it was written.**
//! It is the obvious rule and it is a lexical one, and the tree falsified it within a day.
//! The three reasons were corrected — while this file was being written — to say *"this
//! repository has no CI, and `ci/README.md` says why"*, which is the truest sentence
//! available about the subject and **contains the forbidden word**. The lexical gate would
//! have reddened on the correction.
//!
//! That is the worst thing a gate can do. A gate that punishes a bug teaches people to fix
//! bugs; a gate that punishes a *fix* teaches them to delete gates, and they are right to.
//! The word is not the defect — asserting an untrue thing is, and no rule about vocabulary
//! can tell an assertion from a denial.
//!
//! So the rule is structural rather than linguistic, and it is the one this repository
//! already lives by everywhere else — **a fact has one home, and everything else points at
//! it**:
//!
//! > A reason may name `CI` when a pipeline exists, or when it points the reader at
//! > [`THE_FILE_THAT_OWNS_THE_FACT`]. Otherwise it is restating, in a place nobody will
//! > revisit, something it does not own.
//!
//! That fails the original wording, which asserted a pipeline and cited nothing. It passes
//! both the correction and the future in which somebody installs the workflow — and in that
//! future the first clause makes the gate stand down by itself, with nobody needing to
//! remember it exists. It also passes a reason that says nothing about pipelines at all,
//! which is most of them.
//!
//! # What it cannot see, said plainly
//!
//! Whether the sentence is *true*. `"CI runs it. See ci/README.md."* satisfies this gate and
//! contradicts itself. What the gate guarantees is narrower and is the part that decays: that
//! every mention of the pipeline carries the address of the file that knows, so a reader who
//! doubts the sentence is one hop from the answer and a corrector has one place to correct.

mod common;

/// The word this repository uses when it means *a pipeline*.
///
/// Matched as an upper-case standalone word. `CI` upper-case is how the claim is spelled here
/// and how `crates/testsupport` spells the environment variable whose presence *proves* a
/// pipeline is running. `ci/` lower-case is a directory that exists and is not a claim about
/// anything, which is why it is not the needle.
const A_PIPELINE: &str = "CI";

/// The one file in this repository that says whether there is a pipeline.
const THE_FILE_THAT_OWNS_THE_FACT: &str = "ci/README.md";

/// Where a workflow has to sit before GitHub will run it.
const WHERE_A_WORKFLOW_RUNS_FROM: &str = ".github/workflows";

/// The attribute, spelled in two pieces so that this file cannot match its own gate.
///
/// Written out, the assembled examples in [`the_ci_gate_is_capable_of_failing`] would be
/// found by [`ignore_reasons`] walking this very file, and the gate would redden on its own
/// control — correctly, which is worse than incorrectly.
/// `crates/frontend/tests/portability.rs` records the same collision from the other
/// direction: a gate whose subject includes its own source has to be written so that
/// describing it is not doing it.
const ATTRIBUTE: &str = "#[ignore";

/// Whether `reason` names a pipeline, as a whole word.
///
/// Substring matching would fire on `SPECIFIC` and on `CIRCUIT`; this asks for the word.
fn names_a_pipeline(reason: &str) -> bool {
    let bounded = |index: usize| {
        let before = reason[..index].bytes().next_back();
        let after = reason[index + A_PIPELINE.len()..].bytes().next();
        [before, after]
            .into_iter()
            .flatten()
            .all(|byte| !byte.is_ascii_alphanumeric() && byte != b'_')
    };
    reason
        .match_indices(A_PIPELINE)
        .any(|(index, _)| bounded(index))
}

/// Whether `reason` may name a pipeline: it defers to the file that owns the fact.
fn defers_to_the_owner(reason: &str) -> bool {
    reason.contains(THE_FILE_THAT_OWNS_THE_FACT)
}

/// Every `#[ignore = "…"]` reason in one source file, with the line it starts on.
///
/// A hand-rolled scan rather than a parse, for the reason `crates/frontend/build.rs`'s own
/// text-reading gate gives: the alternative is not a better instrument, it is no instrument.
/// It is deliberately shallow: a bare `#[ignore]` with no reason is passed over, because the
/// text between the attribute and the next quotation mark is then not an `=`.
///
/// # Passing over a reasonless attribute and passing over a reason are not the same act
///
/// They used to be. Both took the same `continue`, so an attribute whose reason this scan
/// could not read left the population **without saying so** — and the shape that does it is
/// ordinary Rust: `= r"…"` and `= r#"…"#` put an `r` between the `=` and the quote, the trim
/// comes out `= r#`, and the reason is dropped as silently as if it had never been written.
///
/// A gate that declines to grade an input and says nothing is this file's own subject said
/// backwards — the whole argument of the header above is that a claim nobody re-reads is
/// worse than no claim. So the cases are separated by what sits between the attribute and the
/// next quotation mark. An `=` is a reason this scan can read. A `]` is a bare attribute
/// carrying no reason, and there is nothing to grade. **Nothing at all** is [`ATTRIBUTE`]'s
/// own text inside a string literal — this file, at the constant, and the quotation mark
/// found is that literal's closing one — which is likewise not a reason. Anything else is a
/// reason that exists and cannot be read, and it is refused out loud.
///
/// There is no raw-string ignore reason in the tree today, so this is a hole closed before it
/// is stepped in rather than a defect repaired; the fix, when somebody meets it, is either an
/// ordinary string literal or teaching this scan the `r#` form deliberately.
fn ignore_reasons(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = source[cursor..].find(ATTRIBUTE) {
        let attribute = cursor + offset;
        cursor = attribute + ATTRIBUTE.len();
        let Some(quote) = source[cursor..].find('"') else {
            break;
        };
        let between = source[cursor..cursor + quote].trim();
        if between != "=" {
            assert!(
                between.is_empty() || between.starts_with(']'),
                "line {} carries an ignore reason spelled `{between}\"`, which this scan \
                 cannot read — a raw string literal, most likely. Grading it as written would \
                 grade the wrong text and skipping it would be the silence this file exists \
                 to argue against, so it is refused. Write the reason as an ordinary string \
                 literal, or teach ignore_reasons the form on purpose.",
                source[..attribute].matches('\n').count() + 1,
            );
            continue;
        }
        let opens = cursor + quote + 1;
        let Some(length) = literal_length(&source[opens..]) else {
            break;
        };
        found.push((
            source[..attribute].matches('\n').count() + 1,
            source[opens..opens + length].to_owned(),
        ));
        cursor = opens + length;
    }
    found
}

/// The length of a Rust string literal's body, given the text just past its opening quote.
fn literal_length(text: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, byte) in text.bytes().enumerate() {
        match (escaped, byte) {
            (true, _) => escaped = false,
            (false, b'\\') => escaped = true,
            (false, b'"') => return Some(index),
            (false, _) => {}
        }
    }
    None
}

/// Whether a workflow is installed where GitHub would run it from.
///
/// Checked on the filesystem rather than through the repository walk, because that walk
/// prunes dotted directories as tooling state — which `.github` is, right up until the moment
/// it is the subject.
fn a_pipeline_exists() -> bool {
    common::workspace_root()
        .join(WHERE_A_WORKFLOW_RUNS_FROM)
        .is_dir()
}

#[test]
fn no_ignored_gate_names_a_pipeline_without_pointing_at_the_file_that_knows() {
    let repository = common::Repository::walk();
    repository.assert_the_walk_found_the_repository();

    // The deferral has to be somewhere to defer *to*. Demanding a pointer at a file that does
    // not exist would be this gate committing the defect two directories away from it.
    assert!(
        repository.contains(THE_FILE_THAT_OWNS_THE_FACT),
        "{THE_FILE_THAT_OWNS_THE_FACT} is gone, so the deferral this gate requires would be a \
         phantom citation. Move the fact somewhere and name it here.",
    );

    let mut reasons = 0;
    let mut unsourced = Vec::new();
    for document in repository
        .documents()
        .iter()
        .filter(|document| document.path.ends_with(".rs"))
    {
        for (line, reason) in ignore_reasons(&document.text) {
            reasons += 1;
            if names_a_pipeline(&reason) && !defers_to_the_owner(&reason) {
                unsourced.push(format!("{}:{line}", document.path));
            }
        }
    }

    // The walk-found-nothing assertion. One is enough here: this workspace has carried at
    // least one `#[ignore]` since M3, and the control below is what proves the reader can
    // read, so this only has to separate *looked* from *did not look*.
    assert!(
        reasons >= 1,
        "no ignored gate was found anywhere in the workspace; the reader is not reading",
    );
    assert!(
        unsourced.is_empty() || a_pipeline_exists(),
        "{} ignored gate(s) talk about a pipeline without naming {}, and {} does not \
         exist:\n  {}\n\n\
         `ci/README.md`: \"Until it is installed, this repository has no CI.\" An ignore \
         reason is read at the moment somebody decides whether a skipped test matters, so it \
         is the worst place in the repository to keep a private copy of that fact. Point at \
         the file instead. If a pipeline has since been installed, this gate stands down on \
         its own.",
        unsourced.len(),
        THE_FILE_THAT_OWNS_THE_FACT,
        WHERE_A_WORKFLOW_RUNS_FROM,
        unsourced.join("\n  "),
    );
}

#[test]
fn the_ci_gate_is_capable_of_failing() {
    // Both halves, and the first is the one a control usually forgets: a reader that found
    // nothing would make the gate above vacuously green forever. The attribute is assembled
    // rather than written so that this file is not itself an instance of what it forbids.
    let attribute = |reason: &str| format!("#[{} = {reason:?}]\nfn slow() {{}}\n", "ignore");

    // The wording that was there, and the exact reason this file exists.
    let asserted = "5.8 billion instructions. Run with --release -- --ignored; CI runs it \
                    that way.";
    let read = ignore_reasons(&attribute(asserted));
    assert_eq!(read.len(), 1, "the reader stopped seeing an ignore reason");
    assert!(
        names_a_pipeline(&read[0].1) && !defers_to_the_owner(&read[0].1),
        "the sentence this gate exists for is no longer caught",
    );

    // The wording that replaced it, which must pass: it names the pipeline and hands the
    // reader the file that says whether there is one.
    let corrected = "5.8 billion instructions. Run it with `cargo test --release`. Nothing \
                     runs that automatically: this repository has no CI, and `ci/README.md` \
                     says why.";
    assert!(
        names_a_pipeline(corrected) && defers_to_the_owner(corrected),
        "a truthful denial that cites its source must be allowed",
    );

    // A true statement with no source is still a private copy of somebody else's fact, and is
    // how three stale copies came to exist in the first place.
    assert!(
        names_a_pipeline("this repository has no CI") && !defers_to_the_owner("no CI"),
        "an unsourced restatement is the shape being forbidden, not an exception to it",
    );

    // And the word is a word.
    for innocent in [
        "a SPECIFIC number",
        "the CIRCUIT diagram",
        "see ci/README.md",
        "Cirrus",
    ] {
        assert!(
            !names_a_pipeline(innocent),
            "{innocent} does not name a pipeline"
        );
    }
    for named in ["CI runs it", "run under CI", "(CI)", "CI."] {
        assert!(names_a_pipeline(named), "{named} names a pipeline");
    }

    // A bare `#[ignore]` carries no reason and must not be read as carrying the next string
    // literal in the file, which is how a shallow scan usually goes wrong.
    let bare = format!(
        "#[{}]\nfn slow() {{}}\nconst WHY: &str = \"CI runs it\";\n",
        "ignore"
    );
    assert!(
        ignore_reasons(&bare).is_empty(),
        "a reasonless #[ignore] swallowed an unrelated string literal",
    );

    // The escape handling, because a reason containing a quotation mark is the one input that
    // would silently truncate every reason after it in the same file.
    let quoting = ignore_reasons(&attribute(r#"a "quoted" word and then CI runs it"#));
    let [(_, reason)] = quoting.as_slice() else {
        panic!("an escaped quotation mark ended the scan early");
    };
    assert!(names_a_pipeline(reason));
}
