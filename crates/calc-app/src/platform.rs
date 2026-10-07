use std::collections::HashMap;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Instant, SystemTime};

use calc_exec::Parallelism;

use crate::result_record::UtcTimestamp;

pub trait Clock: Send + Sync {
    fn now_utc(&self) -> UtcTimestamp;
    fn monotonic_nanoseconds(&self) -> u64;
    fn resolution_nanoseconds(&self) -> u64;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    Pending,
    Finished,
}

pub trait Job: Send {
    fn step(&mut self) -> JobState;
}

pub struct SystemClock {
    start: Instant,
    resolution: OnceLock<u64>,
}

const RESOLUTION_SAMPLES: usize = 1000;

impl SystemClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            resolution: OnceLock::new(),
        }
    }

    fn measured_resolution(&self) -> u64 {
        let mut smallest = u64::MAX;
        let mut previous = self.monotonic_nanoseconds();
        for _ in 0..RESOLUTION_SAMPLES {
            let reading = self.monotonic_nanoseconds();
            let step = reading.saturating_sub(previous);
            if step > 0 && step < smallest {
                smallest = step;
            }
            previous = reading;
        }
        smallest
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_utc(&self) -> UtcTimestamp {
        UtcTimestamp::from_system_time(SystemTime::now())
            .unwrap_or_else(|| UtcTimestamp::from_milliseconds_since_unix_epoch(0))
    }

    fn monotonic_nanoseconds(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    fn resolution_nanoseconds(&self) -> u64 {
        *self
            .resolution
            .get_or_init(|| self.measured_resolution().max(1))
    }
}

pub struct FixedClock {
    time: UtcTimestamp,
    step_nanoseconds: u64,
    resolution_nanoseconds: u64,
    readings: AtomicU64,
}

const FIXED_RESOLUTION_NANOSECONDS: u64 = 1;

impl FixedClock {
    pub const fn new(time: UtcTimestamp, step_nanoseconds: u64) -> Self {
        Self::with_resolution(time, step_nanoseconds, FIXED_RESOLUTION_NANOSECONDS)
    }

    pub const fn with_resolution(
        time: UtcTimestamp,
        step_nanoseconds: u64,
        resolution_nanoseconds: u64,
    ) -> Self {
        Self {
            time,
            step_nanoseconds,
            resolution_nanoseconds,
            readings: AtomicU64::new(0),
        }
    }
}

impl Clock for FixedClock {
    fn now_utc(&self) -> UtcTimestamp {
        self.time
    }

    fn monotonic_nanoseconds(&self) -> u64 {
        let step = self.step_nanoseconds;
        self.readings
            .fetch_update(AtomicOrdering::SeqCst, AtomicOrdering::SeqCst, |reading| {
                Some(reading.saturating_add(step))
            })
            .unwrap_or_else(|reading| reading)
    }

    fn resolution_nanoseconds(&self) -> u64 {
        self.resolution_nanoseconds
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScopedThreadParallelism {
    worker_count: NonZeroUsize,
}

impl ScopedThreadParallelism {
    pub fn new(worker_count: NonZeroUsize) -> Self {
        Self { worker_count }
    }

    pub fn available() -> Self {
        Self::new(std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN))
    }
}

impl Parallelism for ScopedThreadParallelism {
    fn worker_count(&self) -> NonZeroUsize {
        self.worker_count
    }

    fn run_all<'scope>(&self, tasks: Vec<Box<dyn FnOnce() + Send + 'scope>>) {
        let workers = self.worker_count.get().min(tasks.len());
        if workers <= 1 {
            for task in tasks {
                task();
            }
            return;
        }
        let mut groups: Vec<Vec<Box<dyn FnOnce() + Send + 'scope>>> =
            (0..workers).map(|_| Vec::new()).collect();
        for (index, task) in tasks.into_iter().enumerate() {
            if let Some(group) = groups.get_mut(index % workers) {
                group.push(task);
            }
        }
        std::thread::scope(|scope| {
            for group in groups {
                scope.spawn(move || {
                    for task in group {
                        task();
                    }
                });
            }
        });
    }
}

const TEMPORARY_SUFFIX: &str = ".tmp";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StorageLocation {
    path: PathBuf,
}

impl StorageLocation {
    pub fn from_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    NotFound,
    ReadFailed(ErrorKind),
    WriteFailed(ErrorKind),
}

pub type StorageCompletion<T> = Box<dyn FnOnce(Result<T, StorageError>) + Send>;

pub trait Storage: Send + Sync {
    fn read(&self, location: &StorageLocation, done: StorageCompletion<Vec<u8>>);
    fn write_atomically(
        &self,
        location: &StorageLocation,
        bytes: Vec<u8>,
        done: StorageCompletion<()>,
    );
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NativeStorage;

impl Storage for NativeStorage {
    fn read(&self, location: &StorageLocation, done: StorageCompletion<Vec<u8>>) {
        let result = std::fs::read(location.path()).map_err(|error| match error.kind() {
            ErrorKind::NotFound => StorageError::NotFound,
            kind => StorageError::ReadFailed(kind),
        });
        done(result);
    }

    fn write_atomically(
        &self,
        location: &StorageLocation,
        bytes: Vec<u8>,
        done: StorageCompletion<()>,
    ) {
        let mut temporary = OsString::from(location.path().as_os_str());
        temporary.push(format!(".{}{TEMPORARY_SUFFIX}", std::process::id()));
        let temporary = PathBuf::from(temporary);
        let parent_created = match location.path().parent() {
            Some(parent) if !parent.as_os_str().is_empty() => std::fs::create_dir_all(parent),
            _ => Ok(()),
        };
        let result = parent_created
            .and_then(|()| std::fs::write(&temporary, &bytes))
            .and_then(|()| {
                std::fs::rename(&temporary, location.path()).inspect_err(|_| {
                    std::fs::remove_file(&temporary).ok();
                })
            })
            .map_err(|error| StorageError::WriteFailed(error.kind()));
        done(result);
    }
}

#[derive(Debug, Default)]
pub struct MemoryStorage {
    files: Mutex<HashMap<StorageLocation, Vec<u8>>>,
    refuses_writes: bool,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn refusing_writes() -> Self {
        Self {
            files: Mutex::default(),
            refuses_writes: true,
        }
    }

    pub fn insert(&self, location: &StorageLocation, bytes: Vec<u8>) {
        self.files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(location.clone(), bytes);
    }

    pub fn contents(&self, location: &StorageLocation) -> Option<Vec<u8>> {
        self.files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(location)
            .cloned()
    }
}

impl Storage for MemoryStorage {
    fn read(&self, location: &StorageLocation, done: StorageCompletion<Vec<u8>>) {
        let found = self.contents(location);
        done(found.ok_or(StorageError::NotFound));
    }

    fn write_atomically(
        &self,
        location: &StorageLocation,
        bytes: Vec<u8>,
        done: StorageCompletion<()>,
    ) {
        if self.refuses_writes {
            done(Err(StorageError::WriteFailed(ErrorKind::PermissionDenied)));
            return;
        }
        self.insert(location, bytes);
        done(Ok(()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_measures_a_positive_resolution() {
        let clock = SystemClock::new();

        let resolution = clock.resolution_nanoseconds();

        assert!(resolution > 0);
    }

    #[test]
    fn system_clock_keeps_its_measured_resolution() {
        let clock = SystemClock::new();

        let first = clock.resolution_nanoseconds();

        assert_eq!(clock.resolution_nanoseconds(), first);
    }

    #[test]
    fn a_fixed_clock_reports_the_resolution_it_was_given() {
        let time = UtcTimestamp::from_milliseconds_since_unix_epoch(0);
        let clock = FixedClock::with_resolution(time, 250, 30);

        assert_eq!(clock.resolution_nanoseconds(), 30);
    }

    #[test]
    fn fixed_clock_always_reads_the_same_time() {
        let time = UtcTimestamp::from_milliseconds_since_unix_epoch(1_000);
        let clock = FixedClock::new(time, 250);
        clock.now_utc();

        assert_eq!(clock.now_utc(), time);
    }

    #[test]
    fn fixed_clock_reading_stops_at_the_largest_value() {
        let clock = FixedClock::new(
            UtcTimestamp::from_milliseconds_since_unix_epoch(0),
            u64::MAX,
        );
        clock.monotonic_nanoseconds();
        clock.monotonic_nanoseconds();

        assert_eq!(clock.monotonic_nanoseconds(), u64::MAX);
    }

    #[test]
    fn fixed_clock_advances_by_its_step_per_reading() {
        let clock = FixedClock::new(UtcTimestamp::from_milliseconds_since_unix_epoch(0), 250);
        clock.monotonic_nanoseconds();

        assert_eq!(clock.monotonic_nanoseconds(), 250);
    }

    #[test]
    fn monotonic_time_never_goes_backward() {
        let clock = SystemClock::new();

        let first = clock.monotonic_nanoseconds();
        let second = clock.monotonic_nanoseconds();

        assert!(second >= first);
    }

    #[test]
    fn system_time_is_after_the_unix_epoch() {
        let clock = SystemClock::new();

        assert!(clock.now_utc().milliseconds_since_unix_epoch() > 0);
    }

    #[test]
    fn scoped_threads_run_every_task_once() {
        let finished = AtomicU64::new(0);
        let tasks: Vec<Box<dyn FnOnce() + Send + '_>> = (0..8)
            .map(|_| {
                let finished = &finished;
                Box::new(move || {
                    finished.fetch_add(1, AtomicOrdering::SeqCst);
                }) as Box<dyn FnOnce() + Send + '_>
            })
            .collect();

        ScopedThreadParallelism::new(NonZeroUsize::new(3).unwrap()).run_all(tasks);

        assert_eq!(finished.into_inner(), 8);
    }

    #[test]
    fn available_parallelism_has_at_least_one_worker() {
        let parallelism = ScopedThreadParallelism::available();

        assert!(parallelism.worker_count() >= NonZeroUsize::MIN);
    }

    fn location(name: &str) -> StorageLocation {
        StorageLocation::from_path(PathBuf::from(name))
    }

    fn read_now(
        storage: &dyn Storage,
        location: &StorageLocation,
    ) -> Result<Vec<u8>, StorageError> {
        let (sender, receiver) = std::sync::mpsc::channel();
        storage.read(
            location,
            Box::new(move |result| {
                sender.send(result).ok();
            }),
        );
        receiver.recv().unwrap_or(Err(StorageError::NotFound))
    }

    #[test]
    fn memory_storage_reads_what_was_written() {
        let storage = MemoryStorage::new();
        let place = location("preferences.json");
        storage.write_atomically(&place, b"{}".to_vec(), Box::new(|_| {}));

        assert_eq!(read_now(&storage, &place), Ok(b"{}".to_vec()));
    }

    #[test]
    fn memory_storage_reports_a_missing_location_as_not_found() {
        let storage = MemoryStorage::new();

        assert_eq!(
            read_now(&storage, &location("absent.json")),
            Err(StorageError::NotFound)
        );
    }

    #[test]
    fn refusing_memory_storage_reports_a_failed_write_and_keeps_nothing() {
        let storage = MemoryStorage::refusing_writes();
        let place = location("preferences.json");
        let (sender, receiver) = std::sync::mpsc::channel();

        storage.write_atomically(
            &place,
            b"{}".to_vec(),
            Box::new(move |result| {
                sender.send(result).ok();
            }),
        );

        assert_eq!(
            (receiver.recv().ok(), storage.contents(&place)),
            (
                Some(Err(StorageError::WriteFailed(ErrorKind::PermissionDenied))),
                None
            )
        );
    }

    struct OwnedDirectory {
        path: PathBuf,
    }

    impl OwnedDirectory {
        fn new(name: &str) -> OwnedDirectory {
            let path =
                std::env::temp_dir().join(format!("calc-app-test-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&path).ok();
            OwnedDirectory { path }
        }
    }

    impl Drop for OwnedDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.path).ok();
        }
    }

    #[test]
    fn native_storage_creates_a_missing_parent_directory_before_writing() {
        let directory = OwnedDirectory::new("missing-parent");
        let place =
            StorageLocation::from_path(directory.path.join("calculator").join("preferences.json"));
        let (sender, receiver) = std::sync::mpsc::channel();

        NativeStorage.write_atomically(
            &place,
            b"{}".to_vec(),
            Box::new(move |result| {
                sender.send(result).ok();
            }),
        );

        assert_eq!(receiver.recv().ok(), Some(Ok(())));
        assert_eq!(read_now(&NativeStorage, &place), Ok(b"{}".to_vec()));
    }

    #[test]
    fn native_storage_leaves_no_temporary_file_after_a_write() {
        let directory = OwnedDirectory::new("no-temporary");
        let place = StorageLocation::from_path(directory.path.join("preferences.json"));

        NativeStorage.write_atomically(&place, b"{}".to_vec(), Box::new(|_| {}));

        let names: Vec<_> = std::fs::read_dir(&directory.path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [OsString::from("preferences.json")]);
    }

    #[test]
    fn native_storage_reports_a_missing_file_as_not_found() {
        let directory = OwnedDirectory::new("not-found");
        let place = StorageLocation::from_path(directory.path.join("preferences.json"));

        assert_eq!(
            read_now(&NativeStorage, &place),
            Err(StorageError::NotFound)
        );
    }
}
