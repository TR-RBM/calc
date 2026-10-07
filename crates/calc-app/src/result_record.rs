use std::time::{Duration, SystemTime};

use calc_core::{ComputedResult, RecognitionTruncation};

use crate::recognition::RecognitionUnavailable;
use calc_exec::{
    Backend, BackendKind, Domain, ExecutionMode, PlanOp, Preference, SkipReason, SkippedBackend,
    Unsupported,
};
use calc_numbers::Number;

const MILLISECONDS_PER_SECOND: u64 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendUse {
    pub preference: Preference,
    pub width: Domain,
    pub selected: Option<BackendKind>,
    pub skipped: Vec<SkippedBackendUse>,
    pub approximate_operations: bool,
    pub modes: Vec<OperationModeUse>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationModeUse {
    pub operation: PlanOp,
    pub mode: ExecutionMode,
}

impl BackendUse {
    pub fn modes_of(backend: &dyn Backend) -> Vec<OperationModeUse> {
        backend
            .execution_modes()
            .into_iter()
            .map(|(operation, mode)| OperationModeUse { operation, mode })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipCause {
    UnsupportedDomain,
    UnsupportedOperation,
    BatchTooLong,
    PrepareFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkippedBackendUse {
    pub backend: BackendKind,
    pub cause: SkipCause,
}

impl From<&SkippedBackend> for SkippedBackendUse {
    fn from(skipped: &SkippedBackend) -> Self {
        let cause = match skipped.reason {
            SkipReason::Unsupported(Unsupported::Domain(_)) => SkipCause::UnsupportedDomain,
            SkipReason::Unsupported(
                Unsupported::Operation(_)
                | Unsupported::IterationCount(_)
                | Unsupported::ApproximateOperationInIteration(_),
            ) => SkipCause::UnsupportedOperation,
            SkipReason::Unsupported(Unsupported::DeviceRefused) => SkipCause::PrepareFailed,
            SkipReason::BatchTooLong { .. } => SkipCause::BatchTooLong,
            SkipReason::PrepareFailed(_) => SkipCause::PrepareFailed,
        };
        Self {
            backend: skipped.kind,
            cause,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UtcTimestamp {
    milliseconds_since_unix_epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CalculatorVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecognizedBinding {
    pub variable: String,
    pub expression: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecognizedConcept {
    pub concept: String,
    pub pattern: String,
    pub coverage: Number,
    pub site: String,
    pub bindings: Vec<RecognizedBinding>,
    pub holds: Vec<Option<bool>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecognitionLimitsUsed {
    pub cap: u32,
    pub work_budget: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recognized {
    pub concept_set_version: String,
    pub limits: Option<RecognitionLimitsUsed>,
    pub truncated: Option<bool>,
    pub truncated_by: Option<RecognitionTruncation>,
    pub matches: Vec<RecognizedConcept>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordRecognition {
    NotRun,
    Ran(Recognized),
    Unavailable(RecognitionUnavailable),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultRecord {
    computed: ComputedResult,
    backend: Vec<BackendUse>,
    dependencies: Vec<LineId>,
    recognition: RecordRecognition,
    computed_at: UtcTimestamp,
    duration: Duration,
    produced_by: CalculatorVersion,
}

impl LineId {
    pub fn from_number(number: u64) -> Option<Self> {
        (number >= 1).then_some(Self(number))
    }

    pub fn number(self) -> u64 {
        self.0
    }
}

impl BackendUse {
    pub fn machine_evaluation(preference: Preference, width: Domain) -> Self {
        Self {
            preference,
            width,
            selected: None,
            skipped: Vec::new(),
            approximate_operations: false,
            modes: Vec::new(),
        }
    }
}

impl UtcTimestamp {
    pub fn from_milliseconds_since_unix_epoch(milliseconds: u64) -> Self {
        Self {
            milliseconds_since_unix_epoch: milliseconds,
        }
    }

    pub fn from_system_time(time: SystemTime) -> Option<Self> {
        let since_epoch = time.duration_since(SystemTime::UNIX_EPOCH).ok()?;
        let milliseconds = since_epoch
            .as_secs()
            .checked_mul(MILLISECONDS_PER_SECOND)?
            .checked_add(u64::from(since_epoch.subsec_millis()))?;
        Some(Self::from_milliseconds_since_unix_epoch(milliseconds))
    }

    pub fn milliseconds_since_unix_epoch(self) -> u64 {
        self.milliseconds_since_unix_epoch
    }
}

impl CalculatorVersion {
    pub fn current() -> Self {
        Self {
            major: env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or_default(),
            minor: env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or_default(),
            patch: env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or_default(),
        }
    }
}

impl ResultRecord {
    pub fn new(
        computed: ComputedResult,
        backend: Vec<BackendUse>,
        dependencies: impl IntoIterator<Item = LineId>,
        computed_at: UtcTimestamp,
        duration: Duration,
        produced_by: CalculatorVersion,
    ) -> Self {
        let mut unique_dependencies = Vec::new();
        for dependency in dependencies {
            if !unique_dependencies.contains(&dependency) {
                unique_dependencies.push(dependency);
            }
        }
        Self {
            computed,
            backend,
            dependencies: unique_dependencies,
            recognition: RecordRecognition::NotRun,
            computed_at,
            duration,
            produced_by,
        }
    }

    pub fn with_sources(mut self, sources: impl IntoIterator<Item = String>) -> Self {
        self.computed = sources.into_iter().fold(self.computed, |computed, source| {
            computed.with_source(&source)
        });
        self
    }

    pub fn with_recognized(self, recognized: Recognized) -> Self {
        self.with_recognition(RecordRecognition::Ran(recognized))
    }

    pub fn with_recognition(mut self, recognition: RecordRecognition) -> Self {
        self.recognition = recognition;
        self
    }

    pub fn recognized(&self) -> Option<&Recognized> {
        match &self.recognition {
            RecordRecognition::Ran(recognized) => Some(recognized),
            RecordRecognition::NotRun | RecordRecognition::Unavailable(_) => None,
        }
    }

    pub fn recognition(&self) -> &RecordRecognition {
        &self.recognition
    }

    pub fn sources(&self) -> impl Iterator<Item = &str> {
        self.computed.sources()
    }

    pub fn computed(&self) -> &ComputedResult {
        &self.computed
    }

    pub fn backend(&self) -> &[BackendUse] {
        &self.backend
    }

    pub fn dependencies(&self) -> &[LineId] {
        &self.dependencies
    }

    pub fn computed_at(&self) -> UtcTimestamp {
        self.computed_at
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn produced_by(&self) -> CalculatorVersion {
        self.produced_by
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_core::{Method, ResultKind, ResultValue};
    use calc_numbers::Number;

    fn line(number: u64) -> LineId {
        LineId::from_number(number).unwrap()
    }

    fn computed() -> ComputedResult {
        ComputedResult::new(
            ResultKind::MachineFloat,
            ResultValue::Number(Number::F64(24.525)),
            None,
            Method::named("first_order_propagation"),
        )
        .unwrap()
    }

    fn record(dependencies: Vec<LineId>) -> ResultRecord {
        ResultRecord::new(
            computed(),
            vec![BackendUse {
                preference: Preference::Automatic,
                width: Domain::F64,
                selected: Some(BackendKind::Cpu),
                skipped: Vec::new(),
                approximate_operations: false,
                modes: Vec::new(),
            }],
            dependencies,
            UtcTimestamp::from_milliseconds_since_unix_epoch(1_789_476_067_312),
            Duration::from_nanos(300),
            CalculatorVersion::current(),
        )
    }

    #[test]
    fn skipped_backend_keeps_backend_and_cause_of_the_session_format() {
        let skipped = SkippedBackend {
            backend_index: 2,
            kind: BackendKind::Gpu,
            reason: SkipReason::Unsupported(Unsupported::Domain(calc_exec::Domain::F64)),
        };

        assert_eq!(
            SkippedBackendUse::from(&skipped),
            SkippedBackendUse {
                backend: BackendKind::Gpu,
                cause: SkipCause::UnsupportedDomain
            }
        );
    }

    #[test]
    fn batch_too_long_is_its_own_cause() {
        let skipped = SkippedBackend {
            backend_index: 0,
            kind: BackendKind::Simd,
            reason: SkipReason::BatchTooLong {
                requested: 10,
                largest: 5,
            },
        };

        assert_eq!(
            SkippedBackendUse::from(&skipped).cause,
            SkipCause::BatchTooLong
        );
    }

    #[test]
    fn sources_are_sorted_without_duplicates() {
        let record = ResultRecord::new(
            ComputedResult::new(
                ResultKind::ExactRational,
                ResultValue::Number(Number::from(1_i64)),
                None,
                Method::named("exact_evaluation"),
            )
            .unwrap(),
            Vec::new(),
            [],
            UtcTimestamp::from_milliseconds_since_unix_epoch(0),
            Duration::ZERO,
            CalculatorVersion::current(),
        )
        .with_sources(["b".to_owned(), "a".to_owned(), "b".to_owned()]);
        assert_eq!(record.sources().collect::<Vec<_>>(), ["a", "b"]);
    }

    #[test]
    fn line_numbers_start_at_one() {
        assert_eq!(LineId::from_number(0), None);
        assert_eq!(line(3).number(), 3);
    }

    #[test]
    fn dependencies_keep_first_use_order_without_duplicates() {
        let record = record(vec![line(2), line(1), line(2)]);

        assert_eq!(record.dependencies(), &[line(2), line(1)]);
    }

    #[test]
    fn record_keeps_the_computed_result() {
        assert_eq!(record(Vec::new()).computed(), &computed());
    }

    #[test]
    fn record_keeps_backend_time_and_duration() {
        let record = record(Vec::new());

        assert_eq!(
            record.backend().first().and_then(|used| used.selected),
            Some(BackendKind::Cpu)
        );
        assert_eq!(
            record.computed_at().milliseconds_since_unix_epoch(),
            1_789_476_067_312
        );
        assert_eq!(record.duration(), Duration::from_nanos(300));
    }

    #[test]
    fn timestamp_from_system_time_drops_sub_millisecond_part() {
        let time = SystemTime::UNIX_EPOCH + Duration::new(12, 345_678_901);

        let timestamp = UtcTimestamp::from_system_time(time).unwrap();

        assert_eq!(timestamp.milliseconds_since_unix_epoch(), 12_345);
    }

    #[test]
    fn timestamp_before_unix_epoch_is_none() {
        let time = SystemTime::UNIX_EPOCH - Duration::from_secs(1);

        assert_eq!(UtcTimestamp::from_system_time(time), None);
    }

    #[test]
    fn current_version_matches_package_version() {
        let version = CalculatorVersion::current();

        assert_eq!(
            format!("{}.{}.{}", version.major, version.minor, version.patch),
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn a_machine_evaluation_starts_with_no_backend_and_its_width() {
        let backend =
            BackendUse::machine_evaluation(Preference::Only(BackendKind::Simd), Domain::F32);

        assert_eq!((backend.selected, backend.width), (None, Domain::F32));
        assert!(!backend.approximate_operations);
    }
}
