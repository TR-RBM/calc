mod activity;
mod application_place;
mod assembly;
mod claim;
mod concept;
mod decimal_text;
mod dimension_mismatch;
mod display_units;
mod displayed_value;
mod escape_time_line;
mod gpu_registration;
mod instruments;
mod json;
mod language_reference;
mod line_picture;
mod messages;
mod modes;
mod ode;
mod picture;
mod platform;
mod preferences;
mod readable;
pub use readable::{READING_DIGITS, ReadingDistance};
mod reading;
mod recognition;
mod replay;
mod result_record;
mod search;
mod session;
mod session_file;
mod solve;
mod solve_answer;
mod solve_form;
mod solve_request;
mod sort_summary;
mod summary;
mod unit_display;
mod unit_override;
mod value_equality;
mod views;
mod working_rules;

use calc_exec::SharedParallelism;
use std::sync::Arc;

use calc_exec_cpu::CpuBackend;
#[cfg(target_arch = "x86_64")]
use calc_exec_simd::SimdBackend;
use calc_exec_wgpu::{WgpuDevice, WgpuDeviceOptions};

pub use activity::{
    Placement, Scale, ShapeMatching, ShapePiece, beginnings_with_activities, concept_activities,
    fits,
};
pub use application_place::{
    KindUnitProblem, PlaceContext, Pointer, PointerDoor, PointerEntry, ShippedUnitSystem,
    check_kind_unit, checked_kind_unit, coherent_kind_unit, kind_unit_problem_message,
    override_kinds, pointer_block, search_answers, shipped_unit_systems,
};
pub use assembly::{
    FunctionReport, UNRESOLVED_CALL, assembly_case_text, assembly_input_text,
    assembly_refusal_text, assembly_reports, is_argument_register, is_general_register,
};
pub use calc_concepts::QuantityKind;
pub use calc_concepts::{ActivityDefinition, ActivityKind, ActivityShape, ActivityVariation};
pub use calc_core::Verdict;
pub use calc_core::{
    BoundKind, BoundSide, Combine, Criterion, ErrorBound, Exactness, IdentityVerdict, RangeVerdict,
    RationalForm, RecognitionTruncation, ResultKind, ResultValue, Truncation,
};
pub use calc_core::{Factorization, Primality, PrimeFactor, SolveRefusal, SystemRefusal};
pub use calc_exec::{Backend, BackendKind, Domain, ExecutionMode, PlanOp, Preference};
pub use calc_numbers::{DECIMAL_PLACES_LIMIT, Expansion, Integer, Number, enclose_between};
pub use calc_sort::{
    BubbleForm, Case, CombForm, EXHAUSTIVE_CHECK_LIMIT, Gaps, Measure, Method as SortMethod,
    OddEvenForm, Over, Partition, Provenance, ShakerForm, Source,
};
pub use calc_syntax::{
    Arguments, Associativity, Construct, ConstructKind, KeywordArgument, KeywordValue,
    LanguageReference, Precedence, PrecedenceRow, PrefixEntry, ReferenceEntry, ReferenceGroup,
    Spelling, SpellingMode, UnitEntry,
};
pub use calc_viz::{
    ColourLegend, ESCAPED_CELL, EscapeTimeForm, INSIDE_CELL, IterationLimitRule, Orbit, Primitive,
    Reading, UNDECIDED_CELL,
};
pub use claim::{
    ClaimOutcome, ExpandOutcome, FactorOutcome, IDENTITY_FRACTION_BOUND,
    IDENTITY_FRACTION_DENOMINATORS, IDENTITY_SEARCH_FROM, IDENTITY_SEARCH_TO, IdentityOutcome,
    IdentitySearch, IntervalText, SEARCH_LIMIT, SearchOutcome, SearchedValue, SolveOutcome,
    UndecidedPoints, claim_of, claim_takes_zero_to_the_zero, expansion_of, factorization_of,
    find_matches, identity_of, solutions_of, unknown_name_json,
};
pub use concept::{
    ConceptError, ConceptLens, ConceptOutcome, ConceptWording, OPERATOR_SIGNS, activity_kind_name,
    activity_shape_name, concept, concept_error_message, concept_identifiers, concept_json,
    concept_name, prose_groups, unglued_signs,
};
pub use display_units::{
    Area, CurriculumUnits, DecidedBy, DisplayDecision, DisplayQuestion, DisplaySources,
    DisplayUnit, UnitSystem, UnitsChoice, UnitsChoiceError, ValuePlace, decide_display_unit,
    display_dimension, resolve_display_unit, unit_source_message, units_of_the_dimension_of,
};
pub use displayed_value::{DisplayedForms, DisplayedValue, KeptCoherent};
pub use escape_time_line::{EscapeTimeLine, parse_escape_time_line};
pub use gpu_registration::{GpuRefusal, GpuRegistration, LazyGpuBackend, registration_of};
pub use instruments::{InstrumentFacts, LineInstrument, applying_instruments};
pub use json::JsonSyntaxError;
pub use language_reference::{
    LocalizedReference, ReferenceSection, ReferenceWords, construct_words, language_reference_json,
    localized_language_reference, reference_group_message,
};
pub use line_picture::{
    AxisDisplayUnit, CameraProjection, LineReading, Picture, PictureAxis, PictureCamera,
    PictureParameter, PictureView,
};
pub use messages::{
    backend_message, diagnostic_message, diagnostic_message_reading, gpu_refusal_message,
    load_error_message, method_message, parse_error_json, parse_error_json_line,
    parse_error_json_naming_input, parse_error_message, plot_error_message, precision_message,
    preference_message, read_error_message, recognised_attempt_json,
    recognition_unavailable_message, sample_error_message, save_error_message,
    session_error_json_line, session_error_message, view_error_message,
};
pub use modes::{
    every_operation_message, mode_count_message, mode_label, modes_block, modes_summary,
    operations_of,
};
pub use ode::{
    OdeComponent, OdeError, OdeHalted, OdeReport, OdeStepLimit, OdeTime, OdeTimePoint,
    OdeUnreadable, ode_report_json, solve_ode,
};
pub use picture::{
    AxisState, AxisTitle, CompletedScene, PictureDefaults, PictureEvent, PictureShape, PictureSlot,
    PlotError, PlotJob, PlotRequest, ReadingAxis, SettleRequest, ViewState, axis_title_text,
    exact_number, pan_range, plot_json, settle_gesture_range, zoom_range,
};
pub use platform::{
    Clock, FixedClock, Job, JobState, MemoryStorage, NativeStorage, ScopedThreadParallelism,
    Storage, StorageCompletion, StorageError, StorageLocation, SystemClock,
};
pub use preferences::{
    PreferenceChange, PreferenceCompletion, PreferenceEvent, PreferenceStore, Preferences,
    StoreState, Theme,
};
pub use reading::{ReadCoordinate, ReadError, ReadEvent, ReadJob, ReadRequest, snapped_coordinate};
pub use recognition::{RecognitionEvent, RecognitionJob, RecognitionState, RecognitionUnavailable};
pub use replay::{ComparedState, Difference, LineComparison, ReplayReport};
pub use result_record::{
    BackendUse, CalculatorVersion, LineId, OperationModeUse, ResultRecord, SkipCause,
    SkippedBackendUse, UtcTimestamp,
};
pub use search::{ConstructHit, SearchContext, SearchEntry, SearchResult, search};
pub use session::{
    KindUnit, Line, Outcome, Precision, RoleMark, Session, SessionError, Settings, UnitOverride,
    note_describes_the_quantity, valid_where,
};
pub use session_file::FORMAT_VERSION;
pub use session_file::{JsonPath, LoadError, PathSegment, SaveError, operation_name};
pub use solve::{
    EvaluationContext, ReplyStatus, SolveError, SolveFailure, SolveReply, solve, typed_error_json,
};
pub use solve_answer::{
    AnswerBody, AnswerBounds, AnswerCondition, AnswerQuantity, AnswerWay, BoundEnd, BoundFrom,
    ConditionParameter, FurtherInput, OutcomeStatus, QuantityMark, ReachableAnswer,
    ReachableCondition, ReachableEntry, SolveAnswer, SolveCriterion, StoredScore, WayListing,
};
pub use solve_form::{
    FormError, FormErrorCode, FormRow, NameForm, QuantityName, ScoreText, SolveForm,
    SolveVocabulary, cockpit_criterion, criterion_message, form_error_message, person_text,
    quantity_mark_message, request_from_form, score_text,
};
pub use solve_request::{
    GivenInput, Phase, RequestError, RequestErrorCode, SolveRequest, parse_request,
};
pub use sort_summary::{CostSummary, SortSummary, TracePlace, TraceStep, TracedEntry};
pub use summary::{
    BackendRun, LineState, LineSummary, MatchedSummary, RecognitionSummary, RecordSummary,
    ReplayState, RoundingSummary, SessionStatus, SolveLineSummary, UncertaintySummary, ValueForm,
    digits_text, exact_number_text, kind_message, propagation_message, record_kind_message,
    replay_state_message, rounding_bound_message, rounding_message, run_time_message,
    shortened_digits, solve_line_message, value_text, value_text_in,
};
pub use unit_display::display_unit_text;
pub use unit_override::{
    AppliedOverride, OverrideProblem, UnitOverrideError, override_problem_message,
    quantity_kind_message,
};
pub use value_equality::values_equal;
pub use views::{
    AvailableViews, DigitsOf, DigitsOutcome, DigitsUncertainty, EnclosureOutcome, MachineFormat,
    ResultView, ViewError, ViewEvent, ViewJob, ViewKind, ViewOutcome, WorkingOperand,
    WorkingOperation, WorkingOutcome, WorkingRefusal, WorkingStepLine, view_outcome_json,
    view_outcome_json_line, working_operation_message, working_refusal_message,
};
pub use working_rules::{RenderedRule, WorkingCondition, WorkingRule, rendered_rule};

pub fn registered_backends() -> Vec<Box<dyn Backend>> {
    let parallelism = SharedParallelism::new(ScopedThreadParallelism::available());
    let mut backends: Vec<Box<dyn Backend>> =
        vec![Box::new(CpuBackend::with_parallelism(parallelism))];
    backends.extend(simd_backends());
    backends
}

#[cfg(target_arch = "x86_64")]
fn simd_backends() -> Vec<Box<dyn Backend>> {
    vec![Box::new(SimdBackend::new())]
}

#[cfg(not(target_arch = "x86_64"))]
fn simd_backends() -> Vec<Box<dyn Backend>> {
    Vec::new()
}

pub fn probed_gpu_registration() -> GpuRegistration {
    match WgpuDevice::new(WgpuDeviceOptions::default()) {
        Ok(device) => registration_of(device),
        Err(_) => GpuRegistration::refused(GpuRefusal::NoAdapter),
    }
}

pub fn registered_backends_with(registration: GpuRegistration) -> Vec<Box<dyn Backend>> {
    let mut backends = registered_backends();
    if let Some(gpu) = registration.into_backend() {
        backends.push(gpu);
    }
    backends
}

pub fn lazy_gpu_backend() -> (Box<dyn Backend>, Arc<LazyGpuBackend>) {
    LazyGpuBackend::shared(Box::new(probed_gpu_registration))
}
