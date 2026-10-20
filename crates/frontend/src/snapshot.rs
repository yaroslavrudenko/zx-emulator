//! Native snapshot reservation; the public `host::save` overwrite contract is unchanged.

use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::{self, File, OpenOptions};
#[cfg(not(target_arch = "wasm32"))]
use std::io::{self, Read, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;

use frontend::host::SaveError;
#[cfg(target_arch = "wasm32")]
use frontend::media;
use spectrum::Spectrum;

use super::{SAVE_STEM, SNAPSHOT_EXTENSION};

/// The status line while a writer thread is saving: the one copy of the words.
#[cfg(not(target_arch = "wasm32"))]
const SAVING: &str = "saving snapshot";

#[cfg(not(target_arch = "wasm32"))]
const MAX_PROBES: u32 = 1000;
#[cfg(not(target_arch = "wasm32"))]
const MAX_STAGE_SCAN_ENTRIES: usize = 4096;
#[cfg(not(target_arch = "wasm32"))]
const MAX_STAGE_SCAN_FILES: usize = 64;
// A 128K image has eight 16KB pages plus its header; this ceiling also limits hostile leftovers.
#[cfg(not(target_arch = "wasm32"))]
const MAX_STAGE_BYTES: u64 = 256 * 1024;
#[cfg(not(target_arch = "wasm32"))]
const STAGE_GLOB: &str = ".zx-snapshot-*-*.tmp";
#[cfg(not(target_arch = "wasm32"))]
const STARTUP_SCAN_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);
const SHUTDOWN_DRAIN: std::time::Duration = std::time::Duration::from_millis(200);
pub(super) const MAX_COMPLETED_OUTCOMES: usize = 2;
#[cfg(not(target_arch = "wasm32"))]
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(not(target_arch = "wasm32"))]
struct StagedSnapshot {
    writing: std::path::PathBuf,
    published: Option<std::path::PathBuf>,
    file: File,
}

#[cfg(not(target_arch = "wasm32"))]
struct Recovery {
    stage: std::path::PathBuf,
    final_path: std::path::PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl StagedSnapshot {
    fn create(directory: &Path, sequence: u64) -> io::Result<Self> {
        let writing = directory.join(format!(
            ".zx-snapshot-{}-{sequence}.writing",
            std::process::id()
        ));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&writing)?;
        let staged = Self {
            writing,
            published: None,
            file,
        };
        staged.file.try_lock()?;
        Ok(staged)
    }

    fn finish(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all(bytes)?;
        self.file.sync_all()?;
        let ready = self.writing.with_extension("tmp");
        // A same-directory hard link is atomic and no-clobber. `rename` replaces an older
        // same-PID/sequence `.tmp` on Unix and Windows, so it is unsafe on PID reuse.
        fs::hard_link(&self.writing, &ready)?;
        self.published = Some(ready);
        Ok(())
    }

    fn path(&self) -> &Path {
        self.published.as_deref().unwrap_or(&self.writing)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for StagedSnapshot {
    fn drop(&mut self) {
        // Both paths were created by this guard; never remove a collided historical path.
        if let Some(path) = &self.published
            && let Err(error) = fs::remove_file(path)
        {
            eprintln!(
                "zx: could not remove owned published staging link {}: {error}",
                path.display()
            );
        }
        if let Err(error) = fs::remove_file(&self.writing) {
            eprintln!(
                "zx: could not remove owned writing file {}: {error}",
                self.writing.display()
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn stage(directory: &Path, bytes: &[u8]) -> io::Result<StagedSnapshot> {
    for _ in 0..MAX_PROBES {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        match StagedSnapshot::create(directory, sequence) {
            Ok(mut staged) => match staged.finish(bytes) {
                Ok(()) => return Ok(staged),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            },
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "snapshot staging names exhausted",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn next_after_existing(directory: &Path) -> io::Result<u32> {
    let mut taken = Vec::new();
    for entry in fs::read_dir(directory)? {
        let name = entry?.file_name();
        let number = name
            .to_str()
            .and_then(|name| name.strip_prefix(SAVE_STEM))
            .and_then(|name| name.strip_prefix('-'))
            .and_then(|name| name.strip_suffix(SNAPSHOT_EXTENSION))
            .and_then(|name| name.strip_suffix('.'))
            .and_then(|name| name.parse::<u32>().ok());
        if let Some(number) = number {
            taken.push(number);
        }
    }
    taken.sort_unstable();
    let mut next = 1_u32;
    for number in taken {
        if number > next {
            break;
        }
        if number == next {
            next = next.checked_add(1).ok_or_else(|| {
                io::Error::new(io::ErrorKind::AlreadyExists, "snapshot numbers exhausted")
            })?;
        }
    }
    Ok(next)
}

/// Reserve exactly one path; a collision is never a reason to overwrite another save.
#[cfg(all(test, not(target_arch = "wasm32")))]
fn save(directory: &Path, bytes: &[u8]) -> Result<String, SaveError> {
    save_with_recovery(directory, bytes, None)
}

#[cfg(not(target_arch = "wasm32"))]
fn save_with_recovery(
    directory: &Path,
    bytes: &[u8],
    recovery: Option<&std::sync::mpsc::Sender<Recovery>>,
) -> Result<String, SaveError> {
    let staged = stage(directory, bytes).map_err(|source| SaveError::Write {
        path: directory.display().to_string(),
        source,
    })?;
    let mut number = next_after_existing(directory).map_err(|source| SaveError::Write {
        path: directory.display().to_string(),
        source,
    })?;

    for _ in 0..MAX_PROBES {
        let name = format!("{SAVE_STEM}-{number}.{SNAPSHOT_EXTENSION}");
        let path = directory.join(&name);
        if let Some(sender) = recovery {
            // After shutdown the parent is gone; the staged guard still owns cleanup.
            let _ = sender.send(Recovery {
                stage: staged.path().to_owned(),
                final_path: path.clone(),
            });
        }
        match fs::hard_link(staged.path(), &path) {
            Ok(()) => return Ok(name),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && number < u32::MAX => {
                number += 1;
            }
            Err(source) => {
                return Err(SaveError::Write {
                    path: path.display().to_string(),
                    source,
                });
            }
        }
    }
    Err(SaveError::Write {
        path: directory
            .join(format!("{SAVE_STEM}-{}.{SNAPSHOT_EXTENSION}", number - 1))
            .display()
            .to_string(),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("snapshot reservation exhausted after {MAX_PROBES} collisions"),
        ),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn is_stage_name(name: &str) -> bool {
    name.strip_prefix(".zx-snapshot-")
        .and_then(|name| name.strip_suffix(".tmp"))
        .and_then(|numbers| numbers.split_once('-'))
        .is_some_and(|(pid, sequence)| {
            pid.parse::<u32>().is_ok() && sequence.parse::<u64>().is_ok()
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn unknown_scan(reason: &str) -> String {
    format!("snapshot UNKNOWN: {reason}; inspect ALL {STAGE_GLOB} in cwd")
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_stage(file: &File, bytes: &mut Vec<u8>) -> Result<(), String> {
    bytes.clear();
    file.take(MAX_STAGE_BYTES + 1)
        .read_to_end(bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_STAGE_BYTES {
        return Err("staged image exceeds read limit".to_owned());
    }
    spectrum::snapshot::z80::parse(bytes)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Read-only startup inventory of published `.tmp` stages; no final file is touched.
/// Pre-publication `.writing` is never scanned, even when unlocked: a crash before the
/// `.tmp` link leaves it for manual inspection, but it is not a recoverable ready image.
#[cfg(not(target_arch = "wasm32"))]
fn scan_stages(directory: &Path) -> Option<String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!(
                "zx: snapshot startup scan failed in {}: {error}",
                directory.display()
            );
            return Some(unknown_scan("scan unavailable"));
        }
    };
    let (mut valid, mut invalid, mut scanned) = (0_usize, 0_usize, 0_usize);
    let mut bytes = Vec::new();
    for entry in entries {
        if scanned == MAX_STAGE_SCAN_ENTRIES || valid + invalid == MAX_STAGE_SCAN_FILES {
            return Some(unknown_scan("stage scan capped"));
        }
        scanned += 1;
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("zx: snapshot stage directory entry failed: {error}");
                return Some(unknown_scan("stage scan incomplete"));
            }
        };
        if !entry.file_name().to_str().is_some_and(is_stage_name) {
            continue;
        }
        let path = entry.path();
        match entry.file_type() {
            Ok(file_type) if file_type.is_file() => {}
            Ok(_) => {
                eprintln!("zx: unsafe non-file snapshot stage: {}", path.display());
                return Some(unknown_scan("unsafe stage path"));
            }
            Err(error) => {
                eprintln!(
                    "zx: cannot inspect snapshot stage {}: {error}",
                    path.display()
                );
                return Some(unknown_scan("stage type unavailable"));
            }
        }
        let file = match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(file) => file,
            Err(error) => {
                eprintln!("zx: cannot open snapshot stage {}: {error}", path.display());
                return Some(unknown_scan("stage lock unavailable"));
            }
        };
        match file.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => continue,
            Err(fs::TryLockError::Error(error)) => {
                eprintln!("zx: cannot lock snapshot stage {}: {error}", path.display());
                return Some(unknown_scan("stage lock failed"));
            }
        }
        match validate_stage(&file, &mut bytes) {
            Ok(()) => valid += 1,
            Err(error) => {
                invalid += 1;
                eprintln!("zx: invalid staged snapshot {}: {error}", path.display());
            }
        }
    }
    (valid + invalid > 0).then(|| format!(
        "snapshot UNKNOWN: {valid} valid / {invalid} invalid {STAGE_GLOB} in cwd; inspect ALL, parse before exclusive link; never overwrite"
    ))
}

/// One startup-owned scanner. It never blocks the window thread; a late worker is abandoned,
/// never mistaken for a completed scan. Filesystem calls themselves cannot be cancelled.
#[cfg(not(target_arch = "wasm32"))]
pub(super) struct StartupScan {
    receiver: std::sync::mpsc::Receiver<(std::time::Instant, Option<String>)>,
    handle: Option<JoinHandle<()>>,
    deadline: std::time::Instant,
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) enum ScanPoll {
    Pending,
    Finished(Option<String>),
}

#[cfg(not(target_arch = "wasm32"))]
impl StartupScan {
    pub(super) fn start(directory: std::path::PathBuf) -> io::Result<Self> {
        Self::start_with(move || scan_stages(&directory))
    }

    fn start_with(job: impl FnOnce() -> Option<String> + Send + 'static) -> io::Result<Self> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let handle = std::thread::Builder::new().spawn(move || {
            let notice = job();
            if sender.send((std::time::Instant::now(), notice)).is_err() {
                eprintln!("zx: snapshot startup scan finished after deadline; result abandoned");
            }
        })?;
        Ok(Self {
            receiver,
            handle: Some(handle),
            deadline: std::time::Instant::now() + STARTUP_SCAN_DEADLINE,
        })
    }

    pub(super) fn poll(&mut self) -> ScanPoll {
        use std::sync::mpsc::TryRecvError;
        match self.receiver.try_recv() {
            Ok((completed_at, notice)) if completed_at <= self.deadline => {
                ScanPoll::Finished(notice)
            }
            Ok(_) => ScanPoll::Finished(Some(unknown_scan("stage scan finished late"))),
            Err(TryRecvError::Empty) if std::time::Instant::now() >= self.deadline => {
                ScanPoll::Finished(Some(unknown_scan("stage scan timed out")))
            }
            Err(TryRecvError::Disconnected) => {
                ScanPoll::Finished(Some(unknown_scan("stage scanner stopped")))
            }
            Err(TryRecvError::Empty) => ScanPoll::Pending,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for StartupScan {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take()
            && handle.is_finished()
            && let Err(error) = handle.join()
        {
            eprintln!("zx: snapshot startup scanner panicked: {error:?}");
        }
    }
}

/// One owned native write at a time; the frame thread polls its completion.
pub(super) struct Writer {
    pending: Option<JoinHandle<Result<String, SaveError>>>,
    completed: VecDeque<String>,
    #[cfg(not(target_arch = "wasm32"))]
    recovery: Option<std::sync::mpsc::Receiver<Recovery>>,
}

impl Writer {
    pub(super) const fn new() -> Self {
        Self {
            pending: None,
            completed: VecDeque::new(),
            #[cfg(not(target_arch = "wasm32"))]
            recovery: None,
        }
    }

    pub(super) fn request(&mut self, machine: &Spectrum) -> String {
        if !self.available() {
            return self.unavailable_message();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let state = match machine.snapshot() {
                Ok(state) => state,
                Err(error) => return error.to_string(),
            };
            self.admit_recoverable(move |recovery| {
                let bytes = spectrum::snapshot::z80::write(&state);
                save_with_recovery(Path::new("."), &bytes, Some(&recovery))
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            let bytes = match media::save(machine) {
                Ok(bytes) => bytes,
                Err(error) => return error.to_string(),
            };
            let name = format!("{SAVE_STEM}-1.{SNAPSHOT_EXTENSION}");
            match page::offer_download(&name, &bytes) {
                page::Handoff::Started => format!("saved {name}"),
                _ => SaveError::Download { name }.to_string(),
            }
        }
    }

    pub(super) fn outstanding(&mut self) -> usize {
        self.collect_finished();
        self.completed.len() + usize::from(self.pending.is_some())
    }

    fn available(&mut self) -> bool {
        self.collect_finished();
        self.pending.is_none() && self.completed.len() < MAX_COMPLETED_OUTCOMES
    }

    fn unavailable_message(&self) -> String {
        if self.completed.len() == MAX_COMPLETED_OUTCOMES {
            "snapshot pending outcomes; wait for reports".to_owned()
        } else {
            "snapshot save already in progress".to_owned()
        }
    }

    fn collect_finished(&mut self) {
        if self.completed.len() < MAX_COMPLETED_OUTCOMES
            && let Some(previous) = self.reap()
        {
            self.completed.push_back(previous);
        }
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    fn admit(
        &mut self,
        job: impl FnOnce() -> Result<String, SaveError> + Send + 'static,
    ) -> String {
        if !self.available() {
            return self.unavailable_message();
        }
        self.enqueue(job)
            .map_or_else(|refusal| refusal, |()| SAVING.to_owned())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn admit_recoverable(
        &mut self,
        job: impl FnOnce(std::sync::mpsc::Sender<Recovery>) -> Result<String, SaveError>
        + Send
        + 'static,
    ) -> String {
        if !self.available() {
            return self.unavailable_message();
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        // **Keyed on the outcome, not on the message.** This compared the returned text with
        // `"saving snapshot"`, so rewording the status line would have silently stopped arming
        // recovery (DECISION.md item 13).
        let outcome = self.enqueue(move || job(sender));
        if outcome.is_ok() {
            self.recovery = Some(receiver);
        }
        outcome.map_or_else(|refusal| refusal, |()| SAVING.to_owned())
    }

    /// Start `job` on the writer thread. A refusal is the status-line text, formatted here once:
    /// the status bar is its only reader, and both callers map the outcome the same way.
    #[cfg(not(target_arch = "wasm32"))]
    fn enqueue(
        &mut self,
        job: impl FnOnce() -> Result<String, SaveError> + Send + 'static,
    ) -> Result<(), String> {
        let handle = std::thread::Builder::new()
            .spawn(job)
            .map_err(|error| format!("could not start snapshot writer: {error}"))?;
        self.pending = Some(handle);
        Ok(())
    }

    fn reap(&mut self) -> Option<String> {
        if !self.pending.as_ref().is_some_and(JoinHandle::is_finished) {
            return None;
        }
        self.pending.take().map(|handle| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.recovery = None;
            }
            match handle.join() {
                Ok(Ok(name)) => format!("saved {name}"),
                Ok(Err(error)) => error.to_string(),
                Err(error) => format!("snapshot writer panicked: {error:?}"),
            }
        })
    }

    pub(super) fn poll(&mut self) -> Option<String> {
        self.completed.pop_front().or_else(|| self.reap())
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        for message in self.completed.drain(..) {
            eprintln!("zx: {message} before shutdown");
        }
        if let Some(handle) = self.pending.take() {
            let deadline = std::time::Instant::now() + SHUTDOWN_DRAIN;
            while !handle.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            if handle.is_finished() {
                match handle.join() {
                    Ok(Ok(name)) => eprintln!("zx: saved {name} during shutdown"),
                    Ok(Err(error)) => eprintln!("zx: snapshot failed during shutdown: {error}"),
                    Err(error) => {
                        eprintln!("zx: snapshot writer panicked during shutdown: {error:?}")
                    }
                }
            } else {
                // A permanently blocked std::fs operation cannot be cancelled or joined within
                // a finite deadline. The worker remains owned until this explicit timeout.
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(recovery) = self.recovery.take()
                    && let Some(latest) = recovery.try_iter().last()
                {
                    eprintln!(
                        "zx: snapshot outcome UNKNOWN after {}ms; stage={} final={}. On restart validate the staged .z80 image before recovering with an exclusive link; do not overwrite a final file.",
                        SHUTDOWN_DRAIN.as_millis(),
                        latest.stage.display(),
                        latest.final_path.display(),
                    );
                } else {
                    eprintln!(
                        "zx: snapshot outcome UNKNOWN after {}ms before staging; inspect .zx-snapshot-*-*.tmp and snapshot-*.z80 on restart, validate before recovery.",
                        SHUTDOWN_DRAIN.as_millis(),
                    );
                }
                drop(handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn directory() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "zx-snapshot-{}-{now}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("test snapshot directory");
        path
    }

    fn fill_snapshot_entropy(machine: &mut Spectrum) {
        use spectrum::memory::{BANK_COUNT, BankIndex};
        for bank in 0..BANK_COUNT {
            let bytes = machine.memory_mut().bank_mut(BankIndex::new(bank as u8));
            let mut entropy = 0x9e37_79b9_u32 ^ bank as u32;
            for value in bytes.iter_mut() {
                entropy ^= entropy << 13;
                entropy ^= entropy >> 17;
                entropy ^= entropy << 5;
                *value = entropy as u8;
            }
        }
    }

    #[test]
    fn native_worker_encoding_matches_existing_z80_for_both_models() {
        for mut machine in [
            Spectrum::new(&[0; 16 * 1024]).expect("48K"),
            Spectrum::spectrum_128(&[0; 16 * 1024], &[0; 16 * 1024]).expect("128K"),
        ] {
            fill_snapshot_entropy(&mut machine);
            let mut old_us = Vec::with_capacity(10);
            let mut admission_us = Vec::with_capacity(10);
            for _ in 0..10 {
                let started = std::time::Instant::now();
                let old = frontend::media::save(&machine).expect("existing encoder");
                old_us.push(started.elapsed().as_micros());
                let started = std::time::Instant::now();
                let captured = machine.snapshot().expect("complete instruction");
                admission_us.push(started.elapsed().as_micros());
                let from_worker =
                    std::thread::spawn(move || spectrum::snapshot::z80::write(&captured))
                        .join()
                        .expect("encoder worker");
                assert_eq!(
                    from_worker, old,
                    "off-thread encoding must preserve .z80 bytes"
                );
            }
            old_us.sort_unstable();
            admission_us.sort_unstable();
            eprintln!(
                "F2 {:?} high-entropy capture+encode median={}us capture-only median={}us",
                machine.model(),
                old_us[5],
                admission_us[5]
            );
        }
    }

    fn measure_sync_save(machine: &Spectrum, directory: &Path) -> (u128, Vec<u8>) {
        let started = std::time::Instant::now();
        let bytes = frontend::media::save(machine).expect("complete snapshot");
        let name = save(directory, &bytes).expect("synchronous atomic publication");
        let elapsed = started.elapsed().as_micros();
        (
            elapsed,
            fs::read(directory.join(name)).expect("published bytes"),
        )
    }

    fn measure_worker_save(machine: &Spectrum, directory: &Path) -> (u128, u128, Vec<u8>) {
        let started = std::time::Instant::now();
        let state = machine.snapshot().expect("complete instruction");
        let path = directory.to_owned();
        let mut writer = Writer::new();
        assert_eq!(
            writer.admit(move || {
                let bytes = spectrum::snapshot::z80::write(&state);
                save_with_recovery(&path, &bytes, None)
            }),
            "saving snapshot"
        );
        let admission = started.elapsed().as_micros();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let completed = loop {
            if let Some(message) = writer.poll() {
                break message;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "worker did not finish"
            );
            std::thread::yield_now();
        };
        let total = started.elapsed().as_micros();
        let name = completed
            .strip_prefix("saved ")
            .expect("truthful worker success");
        (
            admission,
            total,
            fs::read(directory.join(name)).expect("published bytes"),
        )
    }

    #[test]
    fn matched_complete_native_snapshot_operations_preserve_bytes_without_a_speed_claim() {
        for mut machine in [
            Spectrum::new(&[0; 16 * 1024]).expect("48K"),
            Spectrum::spectrum_128(&[0; 16 * 1024], &[0; 16 * 1024]).expect("128K"),
        ] {
            fill_snapshot_entropy(&mut machine);
            let synchronous_dir = directory();
            let worker_dir = directory();
            let (mut sync_us, mut admission_us, mut total_us) =
                (Vec::new(), Vec::new(), Vec::new());
            for index in 0..10 {
                let (sync, worker) = if index % 2 == 0 {
                    (
                        measure_sync_save(&machine, &synchronous_dir),
                        measure_worker_save(&machine, &worker_dir),
                    )
                } else {
                    let worker = measure_worker_save(&machine, &worker_dir);
                    (measure_sync_save(&machine, &synchronous_dir), worker)
                };
                assert_eq!(sync.1, worker.2, "same full .z80 file on both routes");
                sync_us.push(sync.0);
                admission_us.push(worker.0);
                total_us.push(worker.1);
            }
            sync_us.sort_unstable();
            admission_us.sort_unstable();
            total_us.sort_unstable();
            eprintln!(
                "F2 {:?} complete atomic save median sync={}us worker_total={}us UI_admission={}us (n=10, alternating; not historical before/after)",
                machine.model(),
                sync_us[5],
                total_us[5],
                admission_us[5]
            );
        }
    }

    #[test]
    fn staged_file_is_locked_through_exclusive_publication() {
        let directory = directory();
        let staged = stage(&directory, b"complete image").expect("locked stage");
        let other = OpenOptions::new()
            .read(true)
            .write(true)
            .open(staged.path())
            .expect("second handle");
        assert!(
            matches!(other.try_lock(), Err(std::fs::TryLockError::WouldBlock)),
            "a second handle must not classify a live stage as orphaned"
        );
        let final_path = directory.join("snapshot-1.z80");
        fs::hard_link(staged.path(), &final_path).expect("macOS hard link under held lock");
        assert!(
            matches!(other.try_lock(), Err(std::fs::TryLockError::WouldBlock)),
            "lock survives final publication"
        );
        drop(staged);
        assert_eq!(
            fs::read(final_path).expect("published data"),
            b"complete image"
        );
    }

    #[test]
    fn scanner_cannot_seize_new_writing_file_before_writer_locks_and_completes_f2() {
        let directory = directory();
        let sequence = u64::MAX - 1;
        let writing = directory.join(format!(
            ".zx-snapshot-{}-{sequence}.writing",
            std::process::id()
        ));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&writing)
            .expect("created but not yet locked");
        assert!(
            scan_stages(&directory).is_none(),
            "scanner must ignore pre-publication writing files, even before lock"
        );
        file.try_lock().expect("scanner cannot steal writer lock");
        let mut staged = StagedSnapshot {
            writing,
            published: None,
            file,
        };
        let bytes = frontend::media::save(&Spectrum::new(&[0; 16 * 1024]).expect("ROM"))
            .expect("complete F2 image");
        staged.finish(&bytes).expect("atomic ready publication");
        assert!(
            scan_stages(&directory).is_none(),
            "locked ready link is not abandoned"
        );
        let final_path = directory.join("snapshot-1.z80");
        fs::hard_link(staged.path(), &final_path).expect("exclusive final publication");
        drop(staged);
        assert_eq!(fs::read(final_path).expect("saved F2 image"), bytes);
    }

    #[test]
    fn existing_tmp_is_never_replaced_by_a_reused_pid_and_sequence() {
        let directory = directory();
        let sequence = u64::MAX;
        let ready = directory.join(format!(
            ".zx-snapshot-{}-{sequence}.tmp",
            std::process::id()
        ));
        fs::write(&ready, b"historical stage").expect("previous process stage");
        let mut staged = StagedSnapshot::create(&directory, sequence).expect("own writing stage");
        assert_eq!(
            staged
                .finish(b"new snapshot")
                .expect_err("no-clobber publication")
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        drop(staged);
        assert_eq!(
            fs::read(ready).expect("historical stage intact"),
            b"historical stage"
        );
        assert!(
            !directory
                .join(format!(
                    ".zx-snapshot-{}-{sequence}.writing",
                    std::process::id()
                ))
                .exists(),
            "only new owned writing file was cleaned"
        );
    }

    #[test]
    fn crash_barriers_distinguish_writing_from_ready_without_false_saved() {
        if let (Some(directory), Some(phase)) = (
            std::env::var_os("ZX_STAGE_CRASH_DIR"),
            std::env::var_os("ZX_STAGE_CRASH_PHASE"),
        ) {
            let directory = PathBuf::from(directory);
            let phase = phase.to_str().expect("phase UTF-8");
            let bytes = frontend::media::save(&Spectrum::new(&[0; 16 * 1024]).expect("ROM"))
                .expect("complete image");
            if phase == "before-lock" {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(directory.join(format!(".zx-snapshot-{}-0.writing", std::process::id())))
                    .expect("create without lock");
                std::process::exit(0);
            } else {
                let mut staged = StagedSnapshot::create(&directory, 0).expect("locked writer");
                match phase {
                    "after-lock" => {}
                    "after-write" => {
                        staged.file.write_all(&bytes).expect("write before publish");
                        staged.file.sync_all().expect("sync before publish");
                    }
                    "after-tmp" | "after-final" => {
                        staged.finish(&bytes).expect("publish tmp no-clobber");
                        if phase == "after-final" {
                            fs::hard_link(staged.path(), directory.join("snapshot-1.z80"))
                                .expect("publish final no-clobber");
                        }
                    }
                    _ => panic!("unexpected phase: {phase}"),
                }
                fs::write(directory.join("ready"), []).expect("barrier reached");
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while !directory.join("release").exists() {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "parent did not release child"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                std::process::exit(0); // Bypass guard Drop while its File lock is still held.
            }
        }

        for phase in [
            "before-lock",
            "after-lock",
            "after-write",
            "after-tmp",
            "after-final",
        ] {
            let directory = directory();
            let child = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args(["--exact", "snapshot::tests::crash_barriers_distinguish_writing_from_ready_without_false_saved"])
                .env("ZX_STAGE_CRASH_DIR", &directory)
                .env("ZX_STAGE_CRASH_PHASE", phase)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn().expect("child process");
            let child_pid = child.id();
            if phase != "before-lock" {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while !directory.join("ready").exists() {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "child did not reach barrier"
                    );
                    std::thread::yield_now();
                }
                assert!(
                    scan_stages(&directory).is_none(),
                    "{phase}: held lock must hide active stages"
                );
                fs::write(directory.join("release"), []).expect("release child");
            }
            let output = child.wait_with_output().expect("child exit");
            assert!(
                output.status.success(),
                "{phase}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let writing = directory.join(format!(".zx-snapshot-{child_pid}-0.writing"));
            assert!(
                writing.exists(),
                "{phase}: crashed writing artifact is preserved"
            );
            let ready = matches!(phase, "after-tmp" | "after-final");
            if ready {
                let notice = scan_stages(&directory).expect("published tmp is recoverable UNKNOWN");
                assert!(
                    notice.contains("UNKNOWN") && notice.contains("1 valid / 0 invalid"),
                    "{phase}: {notice}"
                );
            } else {
                assert!(
                    scan_stages(&directory).is_none(),
                    "{phase}: .writing is not a ready snapshot"
                );
            }
            let final_path = directory.join("snapshot-1.z80");
            assert_eq!(
                final_path.exists(),
                phase == "after-final",
                "{phase}: final publication boundary"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn stage_scan_does_not_follow_a_symlink_outside_its_directory() {
        let outside = directory().join("untouched.z80");
        let directory = directory();
        fs::write(&outside, b"outside bytes").expect("outside fixture");
        std::os::unix::fs::symlink(
            &outside,
            directory.join(format!(".zx-snapshot-{}-3.tmp", std::process::id())),
        )
        .expect("symlink fixture");
        let notice = scan_stages(&directory).expect("unsafe stage warned");
        assert!(
            notice.contains("UNKNOWN") && notice.contains("unsafe stage path"),
            "{notice}"
        );
        assert_eq!(
            fs::read(&outside).expect("outside intact"),
            b"outside bytes"
        );
    }

    #[test]
    fn final_name_is_absent_until_staged_bytes_are_complete() {
        let directory = directory();
        let path = directory.join("snapshot-1.z80");
        let bytes = vec![0x5a; 64 * 1024];
        let staged = stage(&directory, &bytes).expect("complete staging write");
        assert!(
            !path.exists(),
            "an incomplete final snapshot must not be visible"
        );
        assert_eq!(fs::read(staged.path()).expect("staged bytes"), bytes);
        fs::hard_link(staged.path(), &path).expect("publish without clobber");
        assert_eq!(fs::read(&path).expect("published bytes"), bytes);
        drop(staged);
        assert_eq!(fs::read(&path).expect("published survives cleanup"), bytes);
    }

    #[test]
    fn many_existing_names_use_one_directory_scan_then_one_atomic_reservation() {
        let directory = directory();
        for number in 1..1000 {
            fs::write(
                directory.join(format!("snapshot-{number}.z80")),
                b"previous",
            )
            .expect("occupied candidate");
        }
        let stem = directory.join(SAVE_STEM);
        let before = std::time::Instant::now();
        let old = frontend::host::free_path(stem.to_str().expect("UTF-8 path"), SNAPSHOT_EXTENSION)
            .expect("old probe path");
        let old_us = before.elapsed().as_micros();
        let after = std::time::Instant::now();
        let name = save(&directory, b"new").expect("first unused candidate");
        let new_us = after.elapsed().as_micros();
        eprintln!(
            "C04 999 occupied: old_exists_probe_us={old_us} new_scan_reserve_write_us={new_us}"
        );
        assert_eq!(old, directory.join(&name).display().to_string());
        assert_eq!(name, "snapshot-1000.z80");
        assert_eq!(fs::read(directory.join(&name)).expect("saved"), b"new");
        fs::write(directory.join("snapshot-1001.z80"), b"other writer")
            .expect("forced next collision");
        assert_eq!(
            save(&directory, b"next").expect("collision retry"),
            "snapshot-1002.z80"
        );
        assert_eq!(
            fs::read(directory.join("snapshot-1001.z80")).expect("original"),
            b"other writer"
        );
    }

    #[test]
    fn a_gap_is_first_free_even_with_a_higher_existing_name() {
        let directory = directory();
        fs::write(directory.join("snapshot-1.z80"), b"old one").expect("occupied first");
        fs::write(directory.join("snapshot-3.z80"), b"old three").expect("occupied third");
        let name = save(&directory, b"new two").expect("reserve first gap");
        assert_eq!(name, "snapshot-2.z80");
        assert_eq!(
            fs::read(directory.join(name)).expect("gap file"),
            b"new two"
        );
        assert_eq!(
            fs::read(directory.join("snapshot-3.z80")).expect("older file"),
            b"old three"
        );
    }

    #[test]
    fn u32_max_snapshot_name_cannot_hide_the_first_free_name() {
        let directory = directory();
        fs::write(
            directory.join(format!("snapshot-{}.z80", u32::MAX)),
            b"old maximum",
        )
        .expect("occupied maximum");
        let name = save(&directory, b"new one").expect("maximum is not exhaustion");
        assert_eq!(name, "snapshot-1.z80");
        assert_eq!(
            fs::read(directory.join(name)).expect("first free"),
            b"new one"
        );
    }

    #[test]
    fn finished_unpolled_save_admits_next_and_preserves_both_outcomes() {
        let directory = directory();
        let mut writer = Writer::new();
        let first_dir = directory.clone();
        assert_eq!(
            writer.admit(move || save(&first_dir, b"first")),
            "saving snapshot"
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !writer.pending.as_ref().is_some_and(JoinHandle::is_finished) {
            assert!(std::time::Instant::now() < deadline, "first worker stalled");
            std::thread::yield_now();
        }
        let second_dir = directory.clone();
        assert_eq!(
            writer.admit(move || save(&second_dir, b"second")),
            "saving snapshot"
        );
        assert_eq!(writer.poll(), Some("saved snapshot-1.z80".to_owned()));
        let second = loop {
            if let Some(message) = writer.poll() {
                break message;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "second worker stalled"
            );
            std::thread::yield_now();
        };
        assert_eq!(second, "saved snapshot-2.z80");
        assert_eq!(
            fs::read(directory.join("snapshot-1.z80")).expect("first file"),
            b"first"
        );
        assert_eq!(
            fs::read(directory.join("snapshot-2.z80")).expect("second file"),
            b"second"
        );
    }

    fn wait_until_finished(writer: &Writer) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !writer.pending.as_ref().is_some_and(JoinHandle::is_finished) {
            assert!(
                std::time::Instant::now() < deadline,
                "snapshot worker stalled"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn three_fast_saves_preserve_all_accepted_outcomes_or_refuse_admission() {
        let mut writer = Writer::new();
        assert_eq!(writer.enqueue(|| Ok("A".to_owned())), Ok(()));
        wait_until_finished(&writer);
        assert_eq!(writer.admit(|| Ok("B".to_owned())), "saving snapshot");
        wait_until_finished(&writer);
        let third = writer.admit(|| Ok("C".to_owned()));
        assert_eq!(writer.poll(), Some("saved A".to_owned()));
        assert_eq!(writer.poll(), Some("saved B".to_owned()));
        if third == "saving snapshot" {
            wait_until_finished(&writer);
            assert_eq!(writer.poll(), Some("saved C".to_owned()));
        } else {
            assert!(
                third.contains("pending outcomes"),
                "explicit refusal: {third}"
            );
        }
    }

    #[test]
    fn old_success_never_overwrites_a_new_partial_instruction_error() {
        let mut writer = Writer::new();
        assert_eq!(writer.enqueue(|| Ok("A".to_owned())), Ok(()));
        wait_until_finished(&writer);
        let mut machine = Spectrum::new(&[0xdd; 16 * 1024]).expect("48K ROM");
        machine.step();
        let mut status = crate::Status::new();
        crate::act(
            crate::Hotkey::SaveSnapshot,
            &mut machine,
            &mut status,
            &mut writer,
        );
        assert!(
            status.message.contains("instruction in progress"),
            "{}",
            status.message
        );
        crate::report_snapshot_completion(&mut status, &mut writer, true);
        crate::report_snapshot_completion(&mut status, &mut writer, false);
        assert!(
            status.message.contains("instruction in progress"),
            "{}",
            status.message
        );
        assert_eq!(status.previous_snapshots.len(), 1);
        assert_eq!(status.previous_snapshots[0], "saved A");
        crate::report_snapshot_completion(&mut status, &mut writer, false);
        assert_eq!(status.previous_snapshots.len(), 1, "A appears exactly once");
        assert!(status.message.contains("instruction in progress"));
    }

    #[test]
    fn old_success_stays_separate_while_new_save_completes() {
        use std::sync::mpsc::sync_channel;
        let mut writer = Writer::new();
        assert_eq!(writer.enqueue(|| Ok("A".to_owned())), Ok(()));
        wait_until_finished(&writer);
        let (release, waiting) = sync_channel(1);
        let mut status = crate::Status::new();
        let older = writer.outstanding();
        let message = writer.admit(move || {
            waiting
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("release B");
            Ok("B".to_owned())
        });
        status.snapshot_request(older, message);
        assert_eq!(status.message, "saving snapshot");
        crate::report_snapshot_completion(&mut status, &mut writer, true);
        crate::report_snapshot_completion(&mut status, &mut writer, false);
        assert_eq!(status.message, "saving snapshot");
        assert_eq!(status.previous_snapshots.len(), 1);
        assert_eq!(
            status.previous_snapshots.front().map(String::as_str),
            Some("saved A")
        );
        release.send(()).expect("complete B");
        wait_until_finished(&writer);
        crate::report_snapshot_completion(&mut status, &mut writer, false);
        assert_eq!(status.message, "saved B");
        assert_eq!(status.previous_snapshots.len(), 1);
        assert_eq!(
            status.message_rows().collect::<Vec<_>>(),
            [(1.0, "saved A"), (2.0, "saved B")],
            "the latest completion is the top row, not the previous one"
        );
        crate::report_snapshot_completion(&mut status, &mut writer, false);
        assert_eq!(
            status.previous_snapshots.len(),
            1,
            "neither outcome repeats"
        );
    }

    #[test]
    fn two_older_outcomes_are_bounded_and_never_replace_current_refusal() {
        let mut writer = Writer::new();
        assert_eq!(writer.enqueue(|| Ok("A".to_owned())), Ok(()));
        wait_until_finished(&writer);
        assert_eq!(writer.admit(|| Ok("B".to_owned())), "saving snapshot");
        wait_until_finished(&writer);
        let mut status = crate::Status::new();
        let older = writer.outstanding();
        status.snapshot_request(older, writer.admit(|| Ok("C".to_owned())));
        assert!(status.message.contains("pending outcomes"));
        crate::report_snapshot_completion(&mut status, &mut writer, true);
        for _ in 0..2 {
            crate::report_snapshot_completion(&mut status, &mut writer, false);
        }
        assert_eq!(status.previous_snapshots.len(), MAX_COMPLETED_OUTCOMES);
        assert_eq!(status.previous_snapshots[0], "saved A");
        assert_eq!(status.previous_snapshots[1], "saved B");
        crate::report_snapshot_completion(&mut status, &mut writer, false);
        assert_eq!(status.previous_snapshots.len(), MAX_COMPLETED_OUTCOMES);
        assert!(status.message.contains("pending outcomes"));
    }

    #[test]
    fn startup_process_reports_valid_and_incomplete_stages_without_publishing_any() {
        if let Some(directory) = std::env::var_os("ZX_STAGE_SCAN_CHILD_DIR") {
            let mut scan = Some(StartupScan::start(PathBuf::from(directory)).expect("start scan"));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while !scan
                .as_ref()
                .and_then(|scan| scan.handle.as_ref())
                .is_some_and(JoinHandle::is_finished)
            {
                assert!(std::time::Instant::now() < deadline, "scan did not finish");
                std::thread::yield_now();
            }
            let mut status = crate::Status::new();
            crate::report_startup_scan(&mut status, &mut scan);
            assert!(scan.is_none(), "startup result delivered exactly once");
            let notice = status.recovery_notice.as_deref().expect("visible UNKNOWN");
            crate::tests::assert_fits("startup UNKNOWN", notice);
            println!("startup_notice={notice}");
            return;
        }
        let directory = directory();
        let bytes = frontend::media::save(&Spectrum::new(&[0; 16 * 1024]).expect("ROM"))
            .expect("complete snapshot");
        assert!(spectrum::snapshot::z80::parse(&bytes[..bytes.len() / 2]).is_err());
        for (name, content) in [
            (".zx-snapshot-123-1.tmp", bytes.as_slice()),
            (".zx-snapshot-123-2.tmp", b"invalid snapshot".as_slice()),
            (".zx-snapshot-123-3.tmp", &bytes[..bytes.len() / 2]),
        ] {
            fs::write(directory.join(name), content).expect("stage fixture");
        }
        fs::write(directory.join("snapshot-1.z80"), b"existing final").expect("older final");
        let child = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args(["--exact", "snapshot::tests::startup_process_reports_valid_and_incomplete_stages_without_publishing_any", "--nocapture"])
            .env("ZX_STAGE_SCAN_CHILD_DIR", &directory)
            .output()
            .expect("child scan");
        assert!(
            child.status.success(),
            "{}",
            String::from_utf8_lossy(&child.stderr)
        );
        let output = String::from_utf8(child.stdout).expect("UTF-8 notice");
        for expected in [
            "UNKNOWN",
            "1 valid",
            "2 invalid",
            ".zx-snapshot-*-*.tmp",
            "exclusive",
        ] {
            assert!(output.contains(expected), "{expected} missing: {output}");
        }
        assert_eq!(
            fs::read(directory.join("snapshot-1.z80")).expect("old final"),
            b"existing final"
        );
        assert_eq!(
            fs::read(directory.join(".zx-snapshot-123-1.tmp")).expect("valid stage"),
            bytes
        );
    }

    #[test]
    fn startup_scan_ignores_current_process_incomplete_f2_stage() {
        let directory = directory();
        let active_pid = std::process::id();
        let older_pid = active_pid.wrapping_add(1);
        let active = directory.join(format!(".zx-snapshot-{active_pid}-1.writing"));
        let mut active_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&active)
            .expect("active private stage");
        active_file.try_lock().expect("writer holds lock");
        active_file
            .write_all(b"in-progress F2 bytes")
            .expect("partial write");
        assert!(
            scan_stages(&directory).is_none(),
            "locked writer is not abandoned UNKNOWN"
        );
        let reused_pid = directory.join(format!(".zx-snapshot-{active_pid}-2.tmp"));
        let older = directory.join(format!(".zx-snapshot-{older_pid}-1.tmp"));
        let valid = frontend::media::save(&Spectrum::new(&[0; 16 * 1024]).expect("ROM"))
            .expect("complete older stage");
        fs::write(&reused_pid, &valid).expect("stale reused PID");
        fs::write(&older, &valid).expect("older stage");
        let notice = scan_stages(&directory).expect("both historical stages are visible");
        assert!(notice.contains("2 valid / 0 invalid"), "{notice}");
        assert!(
            notice.contains(STAGE_GLOB),
            "all older paths remain discoverable"
        );
        assert_eq!(
            fs::read(&active).expect("active stage preserved"),
            b"in-progress F2 bytes"
        );
        assert_eq!(fs::read(&reused_pid).expect("reused PID preserved"), valid);
        assert_eq!(fs::read(&older).expect("older stage preserved"), valid);
    }

    #[test]
    fn many_stages_stop_at_a_named_bound_and_refer_to_every_remaining_path() {
        let directory = directory();
        for sequence in 0..=MAX_STAGE_SCAN_FILES {
            fs::write(
                directory.join(format!(".zx-snapshot-123-{sequence}.tmp")),
                b"invalid",
            )
            .expect("stage fixture");
        }
        let notice = scan_stages(&directory).expect("bounded scan warns");
        assert!(
            notice.contains("UNKNOWN") && notice.contains("capped") && notice.contains(STAGE_GLOB),
            "{notice}"
        );
        crate::tests::assert_fits("bounded stage scan", &notice);
        assert!(
            directory
                .join(format!(".zx-snapshot-123-{}.tmp", MAX_STAGE_SCAN_FILES))
                .exists(),
            "scan cannot remove a path it did not inspect"
        );
    }

    #[test]
    fn pending_scan_cannot_replace_current_partial_instruction_error() {
        use std::sync::mpsc::sync_channel;
        let (entered, started) = sync_channel(1);
        let (release, blocked) = sync_channel(1);
        let notice = "snapshot outcome UNKNOWN: valid stage in cwd";
        let mut scan = Some(
            StartupScan::start_with(move || {
                entered.send(()).expect("scan started");
                blocked
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("release scan");
                Some(notice.to_owned())
            })
            .expect("scanner"),
        );
        started
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("scan entered");
        let mut status = crate::Status::new();
        let mut machine = Spectrum::new(&[0xdd; 16 * 1024]).expect("48K ROM");
        machine.step();
        crate::act(
            crate::Hotkey::SaveSnapshot,
            &mut machine,
            &mut status,
            &mut Writer::new(),
        );
        let current = status.message.to_string();
        assert!(current.contains("instruction in progress"), "{current}");
        crate::report_startup_scan(&mut status, &mut scan);
        assert_eq!(status.message, current);
        release.send(()).expect("release scan");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while !scan
            .as_ref()
            .and_then(|scan| scan.handle.as_ref())
            .is_some_and(JoinHandle::is_finished)
        {
            assert!(std::time::Instant::now() < deadline, "scan completion");
            std::thread::yield_now();
        }
        crate::report_startup_scan(&mut status, &mut scan);
        assert!(scan.is_none());
        assert_eq!(status.message, current, "UNKNOWN must not replace F2 error");
        assert_eq!(
            status.message_rows().collect::<Vec<_>>(),
            [(1.0, notice), (2.0, current.as_str())]
        );
    }

    #[test]
    fn completed_scan_before_f2_shows_unknown_without_replacing_opening_message() {
        let notice = "snapshot outcome UNKNOWN: invalid stage in cwd";
        let mut scan =
            Some(StartupScan::start_with(move || Some(notice.to_owned())).expect("scanner"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while !scan
            .as_ref()
            .and_then(|scan| scan.handle.as_ref())
            .is_some_and(JoinHandle::is_finished)
        {
            assert!(std::time::Instant::now() < deadline, "scan completion");
            std::thread::yield_now();
        }
        let mut status = crate::Status::new();
        crate::report_startup_scan(&mut status, &mut scan);
        assert!(scan.is_none());
        assert_eq!(status.message, crate::OPENING_MESSAGE);
        assert_eq!(status.message_rows().next(), Some((1.0, notice)));
    }

    #[test]
    fn blocked_startup_scan_returns_visible_unknown_at_deadline() {
        use std::sync::mpsc::sync_channel;
        let (entered, started) = sync_channel(1);
        let (release, blocked) = sync_channel(1);
        let mut scan = StartupScan::start_with(move || {
            entered.send(()).expect("worker started");
            blocked
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("worker released");
            None
        })
        .expect("start scanner");
        started
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("scanner entered");
        scan.deadline = std::time::Instant::now() - std::time::Duration::from_millis(1);
        let mut scan = Some(scan);
        let mut status = crate::Status::new();
        crate::report_startup_scan(&mut status, &mut scan);
        assert!(scan.is_none(), "blocked scan must settle once");
        let notice = status.recovery_notice.as_deref().expect("visible UNKNOWN");
        assert!(
            notice.contains("UNKNOWN") && notice.contains(STAGE_GLOB),
            "{notice}"
        );
        assert_eq!(status.message, crate::OPENING_MESSAGE);
        crate::tests::assert_fits("scan timeout", notice);
        release.send(()).expect("release scan");
    }

    #[test]
    fn a_failed_write_is_reported_only_after_the_worker_completes() {
        let mut writer = Writer::new();
        assert_eq!(
            writer.enqueue(|| Err(SaveError::Write {
                path: "snapshot-1.z80".to_owned(),
                source: io::Error::new(io::ErrorKind::PermissionDenied, "read-only directory"),
            })),
            Ok(())
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let message = loop {
            if let Some(message) = writer.poll() {
                break message;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "writer did not finish"
            );
            std::thread::yield_now();
        };
        assert!(message.contains("read-only directory"), "{message}");
        assert!(!message.starts_with("saved "));
    }

    #[test]
    fn a_blocked_writer_does_not_stop_frames_or_keyboard_input() {
        use spectrum::keyboard::Key;
        use std::sync::mpsc::sync_channel;
        use std::time::Duration;

        let (started_tx, started_rx) = sync_channel(1);
        let (release_tx, release_rx) = sync_channel(1);
        let mut writer = Writer::new();
        assert_eq!(
            writer.enqueue(move || {
                started_tx.send(()).expect("notify parent");
                release_rx
                    .recv_timeout(Duration::from_secs(5))
                    .expect("release writer");
                Ok("snapshot-1.z80".to_owned())
            }),
            Ok(())
        );
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("writer started");
        assert!(writer.poll().is_none(), "enqueue is not completion");

        let mut machine = Spectrum::new(&[0; 16 * 1024]).expect("16 KB ROM");
        let mut pacer = crate::Pacer::new();
        machine.keyboard_mut().press(Key::A);
        for _ in 0..10 {
            crate::run_tick(
                crate::Tick::Paced(crate::Speed::REAL_TIME),
                Duration::from_millis(20),
                &mut pacer,
                || 0.0,
                || {
                    machine.run_frame();
                    let _ = machine.take_samples();
                },
            );
        }
        assert_eq!(machine.frames(), 10);
        assert!(machine.keyboard().is_pressed(Key::A));
        machine.keyboard_mut().release(Key::A);
        assert!(!machine.keyboard().is_pressed(Key::A));

        release_tx.send(()).expect("release writer");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let completed = loop {
            if let Some(message) = writer.poll() {
                break message;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "writer did not complete"
            );
            std::thread::yield_now();
        };
        assert_eq!(completed, "saved snapshot-1.z80");
    }

    #[test]
    fn shutdown_at_staging_and_publication_barriers_keeps_final_file_atomic() {
        use std::sync::mpsc::sync_channel;
        use std::time::{Duration, Instant};
        for published_before_shutdown in [false, true] {
            let directory = directory();
            let final_path = directory.join("snapshot-1.z80");
            let (ready, staged_rx) = sync_channel(1);
            let (release, waiting) = sync_channel(1);
            let (done, finished) = sync_channel(1);
            let mut writer = Writer::new();
            let worker_dir = directory.clone();
            let worker_final = final_path.clone();
            assert_eq!(
                writer.enqueue(move || {
                    let staged = stage(&worker_dir, b"complete image").expect("staged and synced");
                    if published_before_shutdown {
                        fs::hard_link(staged.path(), &worker_final).expect("publish");
                    }
                    ready.send(staged.path().to_owned()).expect("staging path");
                    waiting
                        .recv_timeout(Duration::from_secs(5))
                        .expect("release");
                    if !published_before_shutdown {
                        fs::hard_link(staged.path(), &worker_final).expect("publish after drain");
                    }
                    drop(staged);
                    done.send(()).expect("completion");
                    Ok("snapshot-1.z80".to_owned())
                }),
                Ok(())
            );
            let staging_path = staged_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("stage reached");
            assert_eq!(final_path.exists(), published_before_shutdown);
            assert!(staging_path.exists());
            let began = Instant::now();
            drop(writer);
            assert!(began.elapsed() < Duration::from_secs(1), "bounded drain");
            assert_eq!(final_path.exists(), published_before_shutdown);
            release.send(()).expect("release staged worker");
            finished
                .recv_timeout(Duration::from_secs(1))
                .expect("worker completed");
            assert_eq!(
                fs::read(&final_path).expect("published bytes"),
                b"complete image"
            );
            assert!(!staging_path.exists(), "stage cleaned after completion");
        }
    }

    #[test]
    fn shutdown_reports_exact_staged_and_final_paths_for_recovery() {
        use std::sync::mpsc::sync_channel;
        use std::time::Duration;

        if let Some(directory) = std::env::var_os("ZX_STAGED_SHUTDOWN_CHILD_DIR") {
            let directory = PathBuf::from(directory);
            let bytes = frontend::media::save(&Spectrum::new(&[0; 16 * 1024]).expect("ROM"))
                .expect("complete CPU state");
            let (ready, staged_path) = sync_channel(1);
            let (_release, blocked) = sync_channel::<()>(1);
            let mut writer = Writer::new();
            writer.admit_recoverable(move |recovery| {
                let staged = stage(&directory, &bytes).expect("synced bytes");
                let final_path = directory.join("snapshot-1.z80");
                recovery
                    .send(Recovery {
                        stage: staged.path().to_owned(),
                        final_path,
                    })
                    .expect("recovery identity");
                ready
                    .send(staged.path().to_owned())
                    .expect("staging identity");
                blocked
                    .recv_timeout(Duration::from_secs(5))
                    .expect("held at publication");
                Ok("snapshot-1.z80".to_owned())
            });
            let path = staged_path
                .recv_timeout(Duration::from_secs(1))
                .expect("staged");
            eprintln!("child_stage={}", path.display());
            drop(writer);
            // Simulate process exit while std::fs cannot be cancelled. Drop must report a
            // recoverable identity before this exit; the staging file is intentionally retained.
            std::process::exit(0);
        }

        let directory = directory();
        let child = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "snapshot::tests::shutdown_reports_exact_staged_and_final_paths_for_recovery",
                "--nocapture",
            ])
            .env("ZX_STAGED_SHUTDOWN_CHILD_DIR", &directory)
            .output()
            .expect("child shutdown result");
        assert!(
            child.status.success(),
            "{}",
            String::from_utf8_lossy(&child.stderr)
        );
        let diagnostics = String::from_utf8(child.stderr).expect("diagnostic UTF-8");
        let staged = diagnostics
            .lines()
            .find_map(|line| line.strip_prefix("child_stage="))
            .map(PathBuf::from)
            .expect("child staging identity");
        let final_path = directory.join("snapshot-1.z80");
        assert!(
            diagnostics.contains(&format!("stage={}", staged.display())),
            "{diagnostics}"
        );
        assert!(
            diagnostics.contains(&format!("final={}", final_path.display())),
            "{diagnostics}"
        );
        let bytes = fs::read(&staged).expect("stage survives blocked process exit");
        spectrum::snapshot::z80::parse(&bytes).expect("recovery validates the complete image");
        assert!(
            !final_path.exists(),
            "incomplete publication must not be visible"
        );
        fs::hard_link(&staged, &final_path).expect("recover exclusively without overwrite");
        assert_eq!(fs::read(final_path).expect("recovered"), bytes);
    }

    #[test]
    fn shutdown_does_not_join_a_stalled_filesystem_worker() {
        use std::sync::mpsc::sync_channel;
        use std::time::{Duration, Instant};
        let (entered, started) = sync_channel(1);
        let (release, blocked) = sync_channel(1);
        let (finished, observed) = sync_channel(1);
        let mut writer = Writer::new();
        writer
            .enqueue(move || {
                entered.send(()).expect("started");
                blocked
                    .recv_timeout(Duration::from_secs(5))
                    .expect("release");
                finished.send(()).expect("finished");
                Ok("snapshot-1.z80".to_owned())
            })
            .expect("the writer thread starts");
        started
            .recv_timeout(Duration::from_secs(1))
            .expect("worker entered");
        let began = Instant::now();
        drop(writer);
        assert!(
            began.elapsed() < Duration::from_secs(1),
            "shutdown blocked on worker"
        );
        release.send(()).expect("release worker after shutdown");
        observed
            .recv_timeout(Duration::from_secs(1))
            .expect("worker finished safely");
    }

    #[test]
    fn two_processes_reserve_distinct_paths_after_the_same_barrier() {
        if let Some(directory) = std::env::var_os("ZX_SNAPSHOT_CHILD_DIR") {
            let directory = PathBuf::from(directory);
            let pid = std::process::id();
            fs::write(directory.join(format!("ready-{pid}")), []).expect("ready marker");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while fs::read_dir(&directory)
                .expect("barrier directory")
                .map(|entry| entry.expect("barrier entry"))
                .filter(|entry| entry.file_name().to_string_lossy().starts_with("ready-"))
                .count()
                < 2
            {
                assert!(std::time::Instant::now() < deadline, "barrier timed out");
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let content = format!("child-{pid}");
            save(&directory, content.as_bytes()).expect("exclusive reservation");
            return;
        }

        let directory = directory();
        let executable = std::env::current_exe().expect("test binary");
        let mut children = Vec::with_capacity(2);
        for _ in 0..2 {
            children.push(std::process::Command::new(&executable)
                .args(["--exact", "snapshot::tests::two_processes_reserve_distinct_paths_after_the_same_barrier"])
                .env("ZX_SNAPSHOT_CHILD_DIR", &directory)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn().expect("child test process"));
        }
        for child in children {
            let result = child.wait_with_output().expect("child result");
            assert!(
                result.status.success(),
                "child failed: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        let snapshots: Vec<_> = fs::read_dir(&directory)
            .expect("snapshot directory")
            .map(|entry| entry.expect("snapshot entry"))
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("snapshot-"))
            .map(|entry| fs::read(entry.path()).expect("snapshot bytes"))
            .collect();
        assert_eq!(snapshots.len(), 2);
        assert_ne!(
            snapshots[0], snapshots[1],
            "one process overwrote the other"
        );
    }
}
