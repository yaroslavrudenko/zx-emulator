//! Inventory of the frontend's explicitly target-routed private seams.
//!
//! # Why an absence is worth a test
//!
//! `docs/M8.md` Decision 7 originally claimed that no frontend source contained a target
//! branch, so host tests and browser code were identical. That was true on 2026-09-01 but is
//! not the present contract: native file reads and F2 snapshot writing now route through
//! private `main.rs` and `snapshot.rs` branches. This gate confines their exact placements;
//! the headless modules still have no explicit target routing. Neither their lack of `cfg`
//! nor a host test proves equal behaviour across targets through dependencies or WebGL.
//!
//! Both native and wasm builds are required for the two I/O seams. A live browser is still
//! required for pixels, input and audio: a source inventory cannot certify any of them.
//!
//! # Scope, and why it is narrower than the document's grep
//!
//! `docs/M8.md`'s command sweeps `crates/frontend/src/`, `crates/spectrum/src/` and
//! `crates/z80/src/`. This test sweeps **only this crate**. Asserting a property of a sibling
//! crate's source from here would make another crate's ordinary refactor turn this crate's
//! suite red, which is a coupling a test should not create — and it would be asserting it in
//! the wrong place, since the two machine crates are already `#![no_std]`-shaped and are proven
//! to build for `wasm32` by the build itself.
//!
//! # The F2/C05/C06 target-specific boundary
//!
//! The native snapshot writer and native disk loader cannot share the browser's synchronous
//! download/HTTP path without blocking native F2 or native startup. Their private routing lives
//! only in `main.rs` and `snapshot.rs`; the headless machine modules still have no target branch.
//! Host tests cannot certify these two seams for the browser: both target builds must run.
//! `docs/M8.md` Decision 7's original whole-crate T1 claim no longer holds for these files.
//!
//! # What it cannot see
//!
//! Everything that is *not* `#[cfg]`. A branch-free headless module can still behave
//! differently through dependencies, pointer width or floating-point rounding.

use std::path::{Path, PathBuf};

/// The needle. Catches `cfg(target_arch`, `cfg(target_os`, `cfg(target_family` and every other
/// spelling in one string, which is exactly what the document's own `grep` matches.
const TARGET_CONDITIONAL: &str = "cfg(target";

/// The fewest `.rs` files this crate can plausibly have.
///
/// A floor rather than an equality, for the reason `docs/STATUS.md` gives about the codegen
/// probe's own floor: it only has to separate *present* from *absent*, and a walk that found
/// nothing reads as **0**. `src/` held ten files when this was written; the floor is set well
/// below that so an ordinary refactor does not have to touch this number, and well above zero
/// so a broken walk cannot pass.
const FEWEST_SOURCES: usize = 6;

/// Every `.rs` file under `directory`, recursively.
fn sources(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let entries = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry.expect("a readable directory entry").path();
        if path.is_dir() {
            found.extend(sources(&path));
        } else if path.extension().is_some_and(|suffix| suffix == "rs") {
            found.push(path);
        }
    }
    found
}

/// This crate's `src/`.
fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn target_seams(source: &str) -> Vec<String> {
    let lines: Vec<_> = source.lines().collect();
    let mut seams = Vec::new();
    let mut owner = "module";
    for (at, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.split_once("fn ") {
            owner = name.1.split('(').next().unwrap_or("module");
        }
        if !trimmed.starts_with("#[cfg") || !trimmed.contains("target_") {
            continue;
        }
        let target = match trimmed {
            "#[cfg(target_arch = \"wasm32\")]" => "wasm",
            "#[cfg(not(target_arch = \"wasm32\"))]" => "native",
            "#[cfg(all(test, not(target_arch = \"wasm32\")))]" => "native-test",
            _ => "unsupported",
        };
        let next = lines[at + 1..]
            .iter()
            .map(|line| line.trim())
            .find(|line| !line.is_empty() && !line.starts_with("//") && *line != "#[test]")
            .expect("target attribute must guard a declaration or block");
        let anchor = if next == "{" { owner } else { next };
        seams.push(format!("{target}:{anchor}"));
    }
    seams.sort();
    seams
}

#[test]
fn headless_machine_modules_do_not_branch_on_the_target() {
    let files = sources(&source_root());
    let mut routed_files = std::collections::BTreeSet::new();

    // The "I was not looking at the thing" assertion, and it is not optional here: this test's
    // verdict is an *absence*, so a walk that visited nothing produces a confident green for a
    // question it never asked. `docs/STATUS.md`: "a count of zero and an absence of the subject
    // are the same observation."
    assert!(
        files.len() >= FEWEST_SOURCES,
        "the walk found {} source files under {}; it is not looking at the crate",
        files.len(),
        source_root().display(),
    );

    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        for (number, line) in text.lines().enumerate() {
            // Comments are skipped, and the reason is that this gate caught its own author.
            // Adding a row to `src/lib.rs`'s coverage table that *described* this test — and
            // therefore quoted the string it looks for — turned it red. A commented-out `cfg`
            // is not code, and a documentation table that cannot name the thing it documents
            // is a gate that makes the codebase worse.
            //
            // It costs nothing here: an attribute is not a comment, so
            // `#[cfg(target_os = "macos")]` on a real line is still caught. That was
            // re-confirmed by mutation after this filter was added, rather than assumed —
            // a filter is exactly the kind of change that can quietly make a gate unable to
            // fail, which is `docs/STATUS.md`'s standing warning about greens that cannot go
            // red.
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.trim_start().starts_with("#[cfg") && line.contains("target_") {
                let specific = path
                    .file_name()
                    .is_some_and(|name| name == "main.rs" || name == "snapshot.rs");
                assert!(
                    specific,
                    "{}:{} routes by target outside the private file/snapshot host seams: {}",
                    path.display(),
                    number + 1,
                    line.trim()
                );
                routed_files.insert(path.file_name().expect("source filename"));
            }
        }
    }
    // F2 native encoding and C05/C06 native disk I/O cannot share the browser's host behavior.
    // T1 host tests now prove only the headless modules; both effective-target builds are required.
    assert_eq!(
        routed_files,
        [
            std::ffi::OsStr::new("main.rs"),
            std::ffi::OsStr::new("snapshot.rs")
        ]
        .into_iter()
        .collect(),
        "the two private native/browser seams must both be present"
    );
}

#[test]
fn private_target_seams_match_the_exact_approved_inventory() {
    const MAIN: &[&str] = &[
        "native:const NATIVE_READ_DRAIN: Duration = Duration::from_millis(200);",
        "native:const NATIVE_READ_DEADLINE: Duration = Duration::from_secs(5);",
        "native:workers: std::sync::Arc<NativeWorkers>,",
        "native:struct NativeWorkers(std::sync::Mutex<Vec<std::thread::JoinHandle<()>>>);",
        "native:type ReadCompletion = (std::time::Instant, std::io::Result<Vec<u8>>);",
        "native:impl NativeWorkers {",
        "native:impl Drop for NativeWorkers {",
        "native:workers: std::sync::Arc::new(NativeWorkers(std::sync::Mutex::new(Vec::new()))),",
        "native:let mut startup_scan = match snapshot::StartupScan::start(std::path::PathBuf::from(\".\")) {",
        "native:report_startup_scan(&mut status, &mut startup_scan);",
        "native:fn report_startup_scan(status: &mut Status, scan: &mut Option<snapshot::StartupScan>) {",
        "wasm:load",
        "native:load",
        "native:fn native_reads_overlap_and_preserve_distinct_file_errors() {",
        "native:fn stalled_native_readers_refuse_a_third_without_retaining_more_than_two() {",
        "native:fn spawn_failure_reaps_finished_readers_before_returning_error() {",
        "native:fn native_read_deadline_is_finite_without_claiming_cancelled_io() {",
        "native:fn abandoned_native_read_does_not_hold_shutdown() {",
    ];
    const SNAPSHOT: &[&str] = &[
        "native:use std::fs::{self, File, OpenOptions};",
        "native:use std::io::{self, Read, Write};",
        "native:use std::path::Path;",
        "native:use std::sync::atomic::{AtomicU64, Ordering};",
        "wasm:use frontend::media;",
        "native:const MAX_PROBES: u32 = 1000;",
        "native:const MAX_STAGE_BYTES: u64 = 256 * 1024;",
        "native:const MAX_STAGE_SCAN_ENTRIES: usize = 4096;",
        "native:const MAX_STAGE_SCAN_FILES: usize = 64;",
        "native:const SAVING: &str = \"saving snapshot\";",
        "native:const STAGE_GLOB: &str = \".zx-snapshot-*-*.tmp\";",
        "native:const STARTUP_SCAN_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);",
        "native:static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);",
        "native:struct StagedSnapshot {",
        "native:struct Recovery {",
        "native:impl StagedSnapshot {",
        "native:impl Drop for StagedSnapshot {",
        "native:fn stage(directory: &Path, bytes: &[u8]) -> io::Result<StagedSnapshot> {",
        "native:fn next_after_existing(directory: &Path) -> io::Result<u32> {",
        "native:fn is_stage_name(name: &str) -> bool {",
        "native:fn unknown_scan(reason: &str) -> String {",
        "native:fn validate_stage(file: &File, bytes: &mut Vec<u8>) -> Result<(), String> {",
        "native:fn scan_stages(directory: &Path) -> Option<String> {",
        "native:pub(super) struct StartupScan {",
        "native:pub(super) enum ScanPoll {",
        "native:impl StartupScan {",
        "native:impl Drop for StartupScan {",
        "native-test:fn save(directory: &Path, bytes: &[u8]) -> Result<String, SaveError> {",
        "native:fn save_with_recovery(",
        "native:recovery: Option<std::sync::mpsc::Receiver<Recovery>>,",
        "native:recovery: None,",
        "native:request",
        "wasm:request",
        "native-test:fn admit(",
        "native:fn admit_recoverable(",
        "native:fn enqueue(",
        "native:reap",
        "native:if let Some(recovery) = self.recovery.take()",
    ];
    for (name, expected) in [("main.rs", MAIN), ("snapshot.rs", SNAPSHOT)] {
        let path = source_root().join(name);
        let text = std::fs::read_to_string(&path).expect("approved private seam source");
        let mut expected: Vec<_> = expected.iter().map(|anchor| (*anchor).to_owned()).collect();
        expected.sort();
        assert_eq!(
            target_seams(&text),
            expected,
            "{} target routing drifted",
            path.display()
        );
        if name == "main.rs" {
            let mutated = text.replacen(
                "machine.render(&mut self.frame);",
                "#[cfg(target_os = \"macos\")]\n        machine.render(&mut self.frame);",
                1,
            );
            assert_ne!(mutated, text, "positive control must land");
            assert_ne!(
                target_seams(&mutated),
                expected,
                "gate must reject a new video target branch"
            );
        }
    }
}

#[test]
fn wasm_seams_still_call_http_and_download_without_native_filesystem() {
    let main = std::fs::read_to_string(source_root().join("main.rs")).expect("window source");
    let snapshot =
        std::fs::read_to_string(source_root().join("snapshot.rs")).expect("snapshot source");
    let wasm_load = main
        .split_once("#[cfg(target_arch = \"wasm32\")]")
        .expect("browser loader branch")
        .1
        .split_once("#[cfg(not(target_arch = \"wasm32\"))]")
        .expect("native loader branch follows")
        .0;
    assert!(wasm_load.contains("macroquad::file::load_file(path)"));
    let wasm_save = snapshot
        .split_once("#[cfg(target_arch = \"wasm32\")]")
        .expect("browser import")
        .1
        .split_once("#[cfg(target_arch = \"wasm32\")]")
        .expect("browser F2 branch")
        .1
        .split_once("fn available(")
        .expect("browser branch ends before writer admission")
        .0;
    assert!(wasm_save.contains("media::save(machine)"));
    assert!(wasm_save.contains("page::offer_download(&name, &bytes)"));
}

#[test]
fn the_target_conditional_code_is_where_it_was_moved_to() {
    // The page ABI still confines unsafe FFI to `crates/page`; private native file and
    // snapshot routes above do not add a public page probe or move the ABI.
    //
    // Reading a sibling crate is exactly what the test above declines to do, and the difference
    // is the direction: asserting that another crate does **not** contain something couples this
    // suite to that crate's every edit, and asserting that it **does** contain the thing this
    // crate delegated to it is a statement about this crate's own design.
    let glue = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page/src/lib.rs")
        .canonicalize()
        .expect("crates/page/src/lib.rs, which crates/frontend depends on");
    let text = std::fs::read_to_string(&glue).expect("a readable crates/page/src/lib.rs");

    assert!(
        text.contains(TARGET_CONDITIONAL),
        "{} must retain the target-conditional page ABI independent of the private native loader",
        glue.display(),
    );
    assert!(
        text.contains("unsafe"),
        "{} was supposed to be where the `unsafe` went",
        glue.display(),
    );
}

#[test]
fn this_crate_still_forbids_unsafe() {
    // `docs/M8.md` Decision 4 spent a page on keeping `unsafe_code = "forbid"` here, and the
    // whole argument for a separate crate collapses the moment somebody relaxes this line to
    // `deny` to make one thing work. A manifest is not usually a thing to assert about; this
    // one is, because the alternative to `forbid` is not a compile error but a review nobody
    // is scheduled to perform.
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains(r#"unsafe_code = "forbid""#),
        "crates/frontend no longer forbids unsafe; `docs/M8.md` Decision 4 says what that costs",
    );
}
