use calc_app::{
    BackendKind, BackendRun, Domain, LineState, LineSummary, MatchedSummary, ReadingDistance,
    RecognitionSummary, RecognitionUnavailable, RecordSummary, RoundingSummary, SessionStatus,
    ValueForm, backend_message, concept_name, diagnostic_message, method_message, modes_block,
    modes_summary, note_describes_the_quantity, precision_message, preference_message,
    propagation_message, recognition_unavailable_message, record_kind_message,
    replay_state_message, rounding_bound_message, rounding_message, run_time_message,
    solve_line_message, valid_where,
};
use calc_app::{
    BubbleForm, Case, CombForm, CostSummary, Gaps, Measure, OddEvenForm, Over, Partition,
    Provenance, ShakerForm, SortMethod, SortSummary, Source,
};
use calc_app::{
    DECIMAL_PLACES_LIMIT, DigitsOf, DigitsUncertainty, Expansion, Number, ResultKind, ResultValue,
    ViewOutcome, WorkingOutcome, digits_text, display_unit_text, shortened_digits, value_text,
    working_operation_message, working_refusal_message,
};
use calc_app::{TracePlace, TraceStep, TracedEntry};
use calc_i18n::{Locale, Localized, Message, render};
use std::cmp::Ordering;

const LIST_SEPARATOR: &str = ", ";
const TERMINAL_WIDTH: usize = 80;
const OPERATION_COLUMN: usize = 24;
const SENTENCE_COLUMN: usize = 2;
const ROW_INDENT: usize = 2;
const STEP_INDENT: usize = 2;
const PADDING: char = ' ';

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowOptions {
    pub how_it_ran: bool,
    pub offer: bool,
}

pub fn terse_row(summary: &LineSummary, locale: &Locale) -> Localized {
    let message = match &summary.state {
        LineState::Result(record) => {
            let with_unit = |value: &str| match &record.unit {
                Some(unit) => render(
                    &Message::CliQuantity {
                        value: unit_operand(value),
                        unit: unit.clone(),
                    },
                    locale,
                )
                .to_string(),
                None => value.to_owned(),
            };
            let value = match shortened_digits(&record.value) {
                Some((shortened, counts)) => render(
                    &Message::CliTerseElided {
                        value: with_unit(&shortened),
                        counts: digit_counts(&counts, locale),
                    },
                    locale,
                )
                .to_string(),
                None => with_unit(&record.value),
            };
            match (&record.uncertainty, record.kind) {
                (Some(uncertainty), _) => Message::CliTerseMeasured {
                    value,
                    uncertainty: with_unit(&uncertainty.standard),
                    kind: render(&record_kind_message(record), locale).to_string(),
                },
                (None, ResultKind::MachineFloat) => Message::CliTerseMachine { value },
                (None, _) if record.value_form == Some(ValueForm::ProvenRange) => {
                    Message::CliTerseRange { value }
                }
                (None, _) => match record.notes.iter().find_map(valid_where) {
                    Some(excluding) => Message::CliTerseExactWhere {
                        value,
                        condition: render(&crate::run::where_not_zero(&excluding, locale), locale)
                            .to_string(),
                    },
                    None => Message::CliTerseExact { value },
                },
            }
        }
        LineState::Failed(detail) => Message::CliTerseFailed {
            detail: render(&on_the_command_line(detail, &summary.input), locale).to_string(),
        },
        LineState::Defined => Message::CliTerseOther {
            state: render(&Message::CliLineDefined, locale)
                .to_string()
                .trim()
                .to_owned(),
        },
        LineState::NotEvaluated => Message::CliTerseOther {
            state: render(&Message::CliLineNotEvaluated, locale)
                .to_string()
                .trim()
                .to_owned(),
        },
        LineState::Solve(_) | LineState::Picture | LineState::Orbit(_) => Message::CliTerseOther {
            state: summary.input.clone(),
        },
    };
    render(&message, locale)
}

fn on_the_command_line(message: &Message, input: &str) -> Message {
    match message {
        Message::ErrorRelationIsAClaim { .. } => Message::CliRelationIsAClaim {
            reading: input.trim().to_owned(),
        },
        Message::ErrorRelationAboutFreeNames { names, .. } => Message::CliRelationAboutFreeNames {
            reading: input.trim().to_owned(),
            names: names.clone(),
        },
        other => other.clone(),
    }
}

pub fn line_rows(
    summary: &LineSummary,
    reading: Option<Reading<'_>>,
    shown: RowOptions,
    locale: &Locale,
) -> Vec<Localized> {
    let header = render(
        &Message::CliLineInput {
            line: summary.label.clone(),
            input: summary.input.clone(),
        },
        locale,
    );
    let mut rows = vec![header];
    match &summary.state {
        LineState::Result(record) => rows.extend(record_rows(record, reading, shown, locale)),
        LineState::Failed(message) => rows.push(render(
            &Message::CliLineFailed {
                detail: render(&on_the_command_line(message, &summary.input), locale).to_string(),
            },
            locale,
        )),
        LineState::NotEvaluated => rows.push(render(&Message::CliLineNotEvaluated, locale)),
        LineState::Picture => rows.push(render(&Message::CliLinePicture, locale)),
        LineState::Defined => rows.push(render(&Message::CliLineDefined, locale)),
        LineState::Orbit(orbit) => rows.push(render(&orbit_message(*orbit), locale)),
        LineState::Solve(solve) => rows.push(render(
            &Message::CliLineSolve {
                state: render(&solve_line_message(*solve), locale).to_string(),
            },
            locale,
        )),
    }
    rows
}

pub fn status_row(status: &SessionStatus, locale: &Locale) -> Localized {
    let text = |message: &Message| render(message, locale).to_string();
    let replay = text(&replay_state_message(status.replay, locale));
    let results = text(&Message::CommonStatusResults {
        count: status.results,
    });
    let running = text(&Message::CommonStatusRunning {
        count: status.running,
    });
    let locale_name = text(&Message::CommonStatusLocale {
        locale: locale.tag().to_owned(),
    });
    let precision = text(&Message::CommonStatusPrecision {
        precision: text(&precision_message(status.precision)),
    });
    let backend = text(&Message::CommonStatusBackend {
        backend: text(&preference_message(status.backend)),
    });
    let message = match status.differing {
        Some(count) => Message::CliStatusReplayed {
            replay,
            results,
            running,
            differing: text(&Message::CommonStatusDiffer { count }),
            locale: locale_name,
            precision,
            backend,
        },
        None => Message::CliStatus {
            replay,
            results,
            running,
            locale: locale_name,
            precision,
            backend,
        },
    };
    render(&message, locale)
}

pub fn orbit_message(orbit: calc_app::Orbit) -> Message {
    let limit = orbit.limit.to_string();
    match (orbit.class, orbit.count) {
        (calc_app::ESCAPED_CELL, Some(count)) => Message::CliLineOrbitEscaped {
            count: count.to_string(),
            limit,
        },
        (calc_app::ESCAPED_CELL, None) => Message::CliLineOrbitCountMissing { limit },
        (calc_app::INSIDE_CELL, _) => Message::CliLineOrbitInside { limit },
        _ => Message::CliLineOrbitUndecided { limit },
    }
}

fn ran_on_row(runs: &[BackendRun], locale: &Locale) -> Option<String> {
    let text = |message: &Message| render(message, locale).to_string();
    let selected: Vec<(BackendKind, Domain)> = runs
        .iter()
        .filter_map(|run| run.selected.map(|kind| (kind, run.width)))
        .collect();
    let [(first, _), rest @ ..] = selected.as_slice() else {
        return None;
    };
    if rest.iter().all(|(kind, _)| kind == first) {
        return Some(text(&backend_message(Some(*first))));
    }
    Some(listed(
        &selected
            .iter()
            .map(|(kind, width)| {
                text(&Message::CliRanOnWidth {
                    backend: text(&backend_message(Some(*kind))),
                    width: text(&width_message(*width)),
                })
            })
            .collect::<Vec<String>>(),
        locale,
    ))
}

fn width_message(width: Domain) -> Message {
    match width {
        Domain::F32 => Message::CommonPrecisionF32,
        Domain::F64 => Message::CommonPrecisionF64,
    }
}

fn run_modes(run: &BackendRun, locale: &Locale) -> String {
    let text = |message: &Message| render(message, locale).to_string();
    if run.modes.is_empty() {
        return text(&Message::CliModesOneWay);
    }
    modes_summary(&run.modes)
        .iter()
        .map(text)
        .collect::<Vec<String>>()
        .join(LIST_SEPARATOR)
}

pub fn how_it_ran_answer(record: &RecordSummary, locale: &Locale) -> String {
    let text = |message: &Message| render(message, locale).to_string();
    if record.runs.iter().all(|run| run.selected.is_none()) {
        return text(&Message::CliModesNoBackend);
    }
    match record.runs.as_slice() {
        [run] => run_modes(run, locale),
        runs => runs
            .iter()
            .map(|run| {
                text(&Message::CliModesEvaluation {
                    width: text(&width_message(run.width)),
                    modes: run_modes(run, locale),
                })
            })
            .collect::<Vec<String>>()
            .join(LIST_SEPARATOR),
    }
}

pub fn modes_rows(record: &RecordSummary, locale: &Locale) -> Vec<Localized> {
    let several = record.runs.len() > 1;
    record
        .runs
        .iter()
        .flat_map(|run| run_rows(run, several, locale))
        .collect()
}

fn run_rows(run: &BackendRun, names_its_width: bool, locale: &Locale) -> Vec<Localized> {
    if run.modes.is_empty() {
        return Vec::new();
    }
    let mut rows: Vec<Localized> = modes_block(&run.modes)
        .into_iter()
        .flat_map(|(label, operations)| {
            let label = render(&label, locale).to_string();
            let label = if names_its_width {
                render(
                    &Message::CliModesBlockLabel {
                        width: render(&width_message(run.width), locale).to_string(),
                        label,
                    },
                    locale,
                )
                .to_string()
            } else {
                label
            };
            wrapped_operations(&operations)
                .into_iter()
                .enumerate()
                .map(|(position, names)| {
                    let label = if position == 0 {
                        label.clone()
                    } else {
                        String::new()
                    };
                    render(
                        &Message::CliModesRow {
                            label,
                            operations: names,
                        },
                        locale,
                    )
                })
                .collect::<Vec<Localized>>()
        })
        .collect();
    rows.extend(wrapped_sentence(
        &render(&Message::CliModesWhatNativeMeans, locale).to_string(),
        locale,
    ));
    rows
}

fn wrapped_value(value: &str, label_cells: usize) -> Vec<String> {
    let room = TERMINAL_WIDTH.saturating_sub(label_cells + ROW_INDENT + 1);
    let mut rows = Vec::new();
    let mut row = String::new();
    for word in value.split_whitespace() {
        let candidate = if row.is_empty() {
            word.to_owned()
        } else {
            format!("{row} {word}")
        };
        if !row.is_empty() && candidate.chars().count() > room {
            rows.push(row);
            row = word.to_owned();
        } else {
            row = candidate;
        }
    }
    rows.push(row);
    rows
}

fn wrapped_sentence(sentence: &str, locale: &Locale) -> Vec<Localized> {
    let mut rows = Vec::new();
    let mut row = String::new();
    for word in sentence.split_whitespace() {
        let candidate = if row.is_empty() {
            word.to_owned()
        } else {
            format!("{row} {word}")
        };
        if !row.is_empty() && SENTENCE_COLUMN + candidate.chars().count() > TERMINAL_WIDTH {
            rows.push(render(&Message::CliModesSentenceRow { text: row }, locale));
            row = word.to_owned();
        } else {
            row = candidate;
        }
    }
    rows.push(render(&Message::CliModesSentenceRow { text: row }, locale));
    rows
}

fn wrapped_operations(operations: &[&str]) -> Vec<String> {
    let mut rows = Vec::new();
    let mut row = String::new();
    for name in operations {
        let candidate = if row.is_empty() {
            (*name).to_owned()
        } else {
            format!("{row}{LIST_SEPARATOR}{name}")
        };
        if !row.is_empty() && OPERATION_COLUMN + candidate.chars().count() > TERMINAL_WIDTH {
            rows.push(format!("{row}{}", LIST_SEPARATOR.trim_end()));
            row = (*name).to_owned();
        } else {
            row = candidate;
        }
    }
    rows.push(row);
    rows
}

fn recognized_value(matched: &MatchedSummary, locale: &Locale) -> String {
    let Some(unnamed) = matched.unnamed else {
        return render(
            &recognition_unavailable_message(RecognitionUnavailable::ConceptSetNotLoaded),
            locale,
        )
        .to_string();
    };
    let mut parts: Vec<String> = matched
        .concepts
        .iter()
        .filter_map(|identifier| concept_name(identifier, locale))
        .collect();
    if unnamed > 0 {
        parts.push(render(&Message::CliRecognizedUnnamed { count: unnamed }, locale).to_string());
    }
    if matched.truncated == Some(true) {
        parts.push(render(&Message::CliRecognizedMore, locale).to_string());
    }
    listed(&parts, locale)
}

fn digit_counts(counts: &[u64], locale: &Locale) -> String {
    let named =
        |count: &u64| render(&Message::CommonValueDigitCount { count: *count }, locale).to_string();
    match counts {
        [count] => named(count),
        [numerator, denominator] => render(
            &Message::CommonValueFractionDigitCounts {
                numerator: numerator.to_string(),
                denominator: denominator.to_string(),
            },
            locale,
        )
        .to_string(),
        several => listed(&several.iter().map(named).collect::<Vec<String>>(), locale),
    }
}

fn listed(parts: &[String], locale: &Locale) -> String {
    let Some((last, head)) = parts.split_last() else {
        return String::new();
    };
    if head.is_empty() {
        return last.clone();
    }
    render(
        &Message::CliRecognizedList {
            head: head.join(LIST_SEPARATOR),
            last: last.clone(),
        },
        locale,
    )
    .to_string()
}

fn unit_operand(value: &str) -> String {
    let mut depth = 0_i32;
    let mut previous = ' ';
    let mut spans_an_operator = false;
    let characters: Vec<char> = value.chars().collect();
    for (position, character) in characters.iter().enumerate() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            '/' if depth == 0 => spans_an_operator = true,
            '+' | '-' if depth == 0 && previous == ' ' => {
                let spaced_after = characters.get(position + 1) == Some(&' ');
                spans_an_operator |= spaced_after;
            }
            _ => {}
        }
        previous = *character;
    }
    if spans_an_operator {
        format!("({value})")
    } else {
        value.to_owned()
    }
}

pub type Reading<'a> = (&'a str, Option<&'a ReadingDistance>);

const SECTION_INSERTION: &str = "3.2";
const SECTION_SELECTION: &str = "3.7";
const SECTION_BUBBLE: &str = "3.4";

fn source_message(source: Source) -> Message {
    match source {
        Source::VitterFlajolet1990Insertion => Message::CommonSortSourceVitterFlajolet1990 {
            section: SECTION_INSERTION.to_owned(),
        },
        Source::VitterFlajolet1990Selection => Message::CommonSortSourceVitterFlajolet1990 {
            section: SECTION_SELECTION.to_owned(),
        },
        Source::VitterFlajolet1990Bubble => Message::CommonSortSourceVitterFlajolet1990 {
            section: SECTION_BUBBLE.to_owned(),
        },
        Source::WikipediaInsertionSort => Message::CommonSortSourceWikipediaInsertionSort,
        Source::WikipediaBubbleSort => Message::CommonSortSourceWikipediaBubbleSort,
        Source::FlajoletGolin1994 => Message::CommonSortSourceFlajoletGolin1994,
        Source::WikipediaHeapsort => Message::CommonSortSourceWikipediaHeapsort,
        Source::WikipediaQuicksortLomuto => Message::CommonSortSourceWikipediaQuicksortLomuto,
        Source::WikipediaQuicksortHoare => Message::CommonSortSourceWikipediaQuicksortHoare,
        Source::WikipediaCountingSort => Message::CommonSortSourceWikipediaCountingSort,
        Source::CormenRandomizedQuicksort => Message::CommonSortSourceCormenRandomizedQuicksort,
        Source::WikipediaSelectionSortVariants => {
            Message::CommonSortSourceWikipediaSelectionSortVariants
        }
        Source::WikipediaCocktailShakerSort => Message::CommonSortSourceWikipediaCocktailShakerSort,
        Source::WikipediaGnomeSort => Message::CommonSortSourceWikipediaGnomeSort,
        Source::WikipediaOddEvenSort => Message::CommonSortSourceWikipediaOddEvenSort,
        Source::WikipediaCombSort => Message::CommonSortSourceWikipediaCombSort,
        Source::WikipediaCycleSort => Message::CommonSortSourceWikipediaCycleSort,
        Source::WikipediaPancakeSorting => Message::CommonSortSourceWikipediaPancakeSorting,
        Source::WikipediaShellsort => Message::CommonSortSourceWikipediaShellsort,
        Source::WikipediaMergeSortBottomUp => Message::CommonSortSourceWikipediaMergeSortBottomUp,
        Source::WikipediaMergeSortNatural => Message::CommonSortSourceWikipediaMergeSortNatural,
        Source::WikipediaRadixSort => Message::CommonSortSourceWikipediaRadixSort,
        Source::ArulanandhamCaludeDinneen2002 => {
            Message::CommonSortSourceArulanandhamCaludeDinneen2002
        }
        Source::Batcher1968Bitonic => Message::CommonSortSourceBatcher1968Bitonic,
        Source::GruberHolzerRuepp2007 => Message::CommonSortSourceGruberHolzerRuepp2007,
    }
}

fn short_source_message(source: Source) -> Message {
    match source {
        Source::VitterFlajolet1990Insertion => Message::CommonSortSourceShortVitterFlajolet1990 {
            section: SECTION_INSERTION.to_owned(),
        },
        Source::VitterFlajolet1990Selection => Message::CommonSortSourceShortVitterFlajolet1990 {
            section: SECTION_SELECTION.to_owned(),
        },
        Source::VitterFlajolet1990Bubble => Message::CommonSortSourceShortVitterFlajolet1990 {
            section: SECTION_BUBBLE.to_owned(),
        },
        Source::WikipediaInsertionSort => Message::CommonSortSourceShortWikipediaInsertionSort,
        Source::WikipediaBubbleSort => Message::CommonSortSourceShortWikipediaBubbleSort,
        Source::FlajoletGolin1994 => Message::CommonSortSourceShortFlajoletGolin1994,
        Source::WikipediaHeapsort => Message::CommonSortSourceShortWikipediaHeapsort,
        Source::WikipediaQuicksortLomuto | Source::WikipediaQuicksortHoare => {
            Message::CommonSortSourceShortWikipediaQuicksort
        }
        Source::WikipediaCountingSort => Message::CommonSortSourceShortWikipediaCountingSort,
        Source::CormenRandomizedQuicksort => {
            Message::CommonSortSourceShortCormenRandomizedQuicksort
        }
        Source::WikipediaSelectionSortVariants => {
            Message::CommonSortSourceShortWikipediaSelectionSortVariants
        }
        Source::WikipediaCocktailShakerSort => {
            Message::CommonSortSourceShortWikipediaCocktailShakerSort
        }
        Source::WikipediaGnomeSort => Message::CommonSortSourceShortWikipediaGnomeSort,
        Source::WikipediaOddEvenSort => Message::CommonSortSourceShortWikipediaOddEvenSort,
        Source::WikipediaCombSort => Message::CommonSortSourceShortWikipediaCombSort,
        Source::WikipediaCycleSort => Message::CommonSortSourceShortWikipediaCycleSort,
        Source::WikipediaPancakeSorting => Message::CommonSortSourceShortWikipediaPancakeSorting,
        Source::WikipediaShellsort => Message::CommonSortSourceShortWikipediaShellsort,
        Source::WikipediaMergeSortBottomUp | Source::WikipediaMergeSortNatural => {
            Message::CommonSortSourceShortWikipediaMergeSort
        }
        Source::WikipediaRadixSort => Message::CommonSortSourceShortWikipediaRadixSort,
        Source::ArulanandhamCaludeDinneen2002 => {
            Message::CommonSortSourceShortArulanandhamCaludeDinneen2002
        }
        Source::Batcher1968Bitonic => Message::CommonSortSourceShortBatcher1968Bitonic,
        Source::GruberHolzerRuepp2007 => Message::CommonSortSourceShortGruberHolzerRuepp2007,
    }
}

fn form_message(method: SortMethod) -> Option<Message> {
    Some(match method {
        SortMethod::Bubble(form) => bubble_form_message(form),
        SortMethod::CocktailShaker(ShakerForm::Full) => Message::CommonSortShakerFormFull,
        SortMethod::CocktailShaker(ShakerForm::Shrinking) => Message::CommonSortShakerFormShrinking,
        SortMethod::CocktailShaker(ShakerForm::LastExchange) => {
            Message::CommonSortShakerFormLastExchange
        }
        SortMethod::OddEven(OddEvenForm::UntilSorted) => Message::CommonSortOddEvenFormUntilSorted,
        SortMethod::OddEven(OddEvenForm::FixedPasses) => Message::CommonSortOddEvenFormFixedPasses,
        SortMethod::Comb(CombForm::LaceyBox) => Message::CommonSortCombFormLaceyBox,
        SortMethod::Shell(Gaps::Shell) => Message::CommonSortGapsShell,
        SortMethod::Shell(Gaps::Knuth) => Message::CommonSortGapsKnuth,
        SortMethod::Shell(Gaps::Ciura) => Message::CommonSortGapsCiura,
        _ => return None,
    })
}

fn bubble_form_message(form: BubbleForm) -> Message {
    match form {
        BubbleForm::Full => Message::CommonSortFormFull,
        BubbleForm::Shrinking => Message::CommonSortFormShrinking,
        BubbleForm::EarlyExit => Message::CommonSortFormEarlyExit,
        BubbleForm::LastExchange => Message::CommonSortFormLastExchange,
    }
}

fn cost_label(cost: &CostSummary) -> Message {
    match (cost.measure, cost.case) {
        (Measure::Comparisons, Case::Best) => Message::CommonRecordFewestComparisons,
        (Measure::Comparisons, Case::Worst) => Message::CommonRecordMostComparisons,
        (Measure::Comparisons, Case::Average) => Message::CommonRecordAverageComparisons,
        (Measure::Writes, Case::Best) => Message::CommonRecordFewestWrites,
        (Measure::Writes, Case::Worst) => Message::CommonRecordMostWrites,
        (Measure::Writes, Case::Average) => Message::CommonRecordAverageWrites,
        (Measure::Flips, Case::Best) => Message::CommonRecordFewestFlips,
        (Measure::Flips, Case::Worst) => Message::CommonRecordMostFlips,
        (Measure::Flips, Case::Average) => Message::CommonRecordAverageFlips,
    }
}

fn sort_fields(sort: &SortSummary, text: &dyn Fn(&Message) -> String) -> Vec<(Message, String)> {
    let provenance = |cost: &CostSummary| {
        let limit = cost.checked_up_to.to_string();
        match (cost.provenance, cost.over) {
            (
                Provenance::DerivedAndChecked { basis: None },
                Over::Orders | Over::OrdersAtPowersOfTwo,
            ) => text(&Message::CommonSortDerived { limit }),
            (Provenance::DerivedAndChecked { basis: None }, Over::OrdersAndPivotChoices) => {
                text(&if cost.case == Case::Average {
                    Message::CommonSortDerivedExpectedOverChoices { limit }
                } else {
                    Message::CommonSortDerivedOverChoices { limit }
                })
            }
            (
                Provenance::DerivedAndChecked {
                    basis: Some(source),
                },
                Over::Orders | Over::OrdersAtPowersOfTwo,
            ) => text(&Message::CommonSortDerivedFrom {
                source: text(&short_source_message(source)),
                limit,
            }),
            (
                Provenance::DerivedAndChecked {
                    basis: Some(source),
                },
                Over::OrdersAndPivotChoices,
            ) => {
                let source = text(&short_source_message(source));
                text(&if cost.case == Case::Average {
                    Message::CommonSortDerivedFromExpectedOverChoices { source, limit }
                } else {
                    Message::CommonSortDerivedFromOverChoices { source, limit }
                })
            }
            (Provenance::FittedAndChecked, _) => text(&Message::CommonSortFitted { limit }),
            (Provenance::PerPassOfTheKeys, _) => text(&Message::CommonSortPerPassOfTheKeys),
            (Provenance::NotInClosedForm, _) => text(&Message::CommonSortNotInClosedForm),
            (Provenance::Unbounded, _) => text(&Message::CommonSortUnbounded),
            (Provenance::PerShuffle, _) => text(&Message::CommonSortPerShuffle),
        }
    };
    let order = if sort.is_decreasing {
        Message::CommonSortDecreasing
    } else {
        Message::CommonSortIncreasing
    };
    let mut fields = Vec::new();
    if let Some(form) = form_message(sort.method) {
        let label = if matches!(sort.method, SortMethod::Shell(_)) {
            Message::CommonRecordGaps
        } else {
            Message::CommonRecordForm
        };
        fields.push((label, text(&form)));
    }
    if let Some(gaps) = &sort.gaps {
        let used = if gaps.is_empty() {
            Message::CommonSortGapsUsedNone {
                length: sort.length.clone(),
            }
        } else {
            Message::CommonSortGapsUsed {
                gaps: gaps.clone(),
                length: sort.length.clone(),
            }
        };
        fields.push((Message::CommonRecordGapsUsed, text(&used)));
    }
    if let SortMethod::Quick(partition) = sort.method {
        let message = match partition {
            Partition::LomutoLast => Message::CommonSortPartitionLomutoLast,
            Partition::HoareFirst => Message::CommonSortPartitionHoareFirst,
            Partition::LomutoRandom { .. } => Message::CommonSortPartitionLomutoRandom,
        };
        fields.push((Message::CommonRecordPartition, text(&message)));
    }
    fields.extend([
        (Message::CommonRecordOrder, text(&order)),
        (
            Message::CommonRecordDescribedIn,
            text(&source_message(sort.source)),
        ),
        (
            Message::CommonRecordFromPositions,
            text(&match &sort.from_positions {
                Some(positions) => Message::CommonSortPositions {
                    positions: positions.clone(),
                },
                None => Message::CommonSortNoPositions,
            }),
        ),
    ]);
    if let Some(count) = &sort.key_evaluations {
        fields.push((
            Message::CommonRecordKeyEvaluations,
            text(&Message::CommonSortKeyEvaluations { count: *count }),
        ));
    }
    fields.push((
        Message::CommonRecordComparisons,
        text(&Message::CommonSortCounted {
            count: sort.comparisons.clone(),
        }),
    ));
    fields.push((
        Message::CommonRecordWrites,
        text(&Message::CommonSortCounted {
            count: sort.writes.clone(),
        }),
    ));
    if let Some(flips) = sort.flips {
        fields.push((
            Message::CommonRecordFlips,
            text(&Message::CommonSortFlips { count: flips }),
        ));
    }
    if let (Some(draws), Some(seed)) = (sort.draws, sort.seed) {
        fields.push((
            Message::CommonRecordDraws,
            text(&if matches!(sort.method, SortMethod::Bogo { .. }) {
                Message::CommonSortShuffleDraws {
                    count: draws,
                    seed: seed.to_string(),
                }
            } else {
                Message::CommonSortDraws {
                    count: draws,
                    seed: seed.to_string(),
                }
            }),
        ));
    }
    if let Some(tallies) = &sort.tallies {
        let counted = if sort.method == SortMethod::Bead {
            Message::CommonSortBeadTallies {
                count: tallies.clone(),
            }
        } else if sort.base.is_some() {
            Message::CommonSortRadixTallies {
                count: tallies.clone(),
            }
        } else {
            Message::CommonSortTallies {
                count: tallies.clone(),
            }
        };
        fields.push((Message::CommonRecordTallies, text(&counted)));
    }
    if let (Some(shuffles), Some(limit)) = (sort.shuffles, sort.limit) {
        fields.push((
            Message::CommonRecordShuffles,
            text(&Message::CommonSortShuffles {
                count: shuffles,
                limit: limit.to_string(),
            }),
        ));
    }
    if matches!(sort.method, SortMethod::Bogo { .. }) {
        let expected = match &sort.expected_shuffles {
            Some(factorial) => Message::CommonSortExpectedShuffles {
                factorial: factorial.clone(),
                length: sort.length.clone(),
            },
            None => Message::CommonSortExpectedShufflesNone,
        };
        fields.push((Message::CommonRecordExpectedShuffles, text(&expected)));
    }
    if let (Some(passes), Some(base)) = (sort.passes, sort.base) {
        fields.push((
            Message::CommonRecordPasses,
            text(&Message::CommonSortPasses {
                passes,
                base: base.to_string(),
            }),
        ));
    }
    if let Some(range) = &sort.key_range {
        fields.push((
            Message::CommonRecordKeyRange,
            text(&Message::CommonSortKeyRange { range: *range }),
        ));
    }
    for cost in &sort.costs {
        let Some(formula) = &cost.formula else {
            let stated = match cost.provenance {
                Provenance::PerPassOfTheKeys => Message::CommonSortPerPassOfTheKeys,
                Provenance::Unbounded => Message::CommonSortUnbounded,
                Provenance::PerShuffle => Message::CommonSortPerShuffle,
                _ => Message::CommonSortNotInClosedForm,
            };
            fields.push((cost_label(cost), text(&stated)));
            continue;
        };
        let Some(value) = &cost.value else {
            fields.push((
                cost_label(cost),
                text(&Message::CommonSortCostHoldsFrom {
                    formula: formula.clone(),
                    from: cost.holds_from.to_string(),
                    length: sort.length.clone(),
                    provenance: provenance(cost),
                }),
            ));
            continue;
        };
        if formula == value {
            let (value, provenance) = (value.clone(), provenance(cost));
            let stated = match (cost.case, cost.over) {
                (Case::Average, Over::Orders) => {
                    Message::CommonSortAverageConstant { value, provenance }
                }
                (Case::Best | Case::Worst, Over::Orders) => {
                    Message::CommonSortExtremeConstant { value, provenance }
                }
                (Case::Average, Over::OrdersAndPivotChoices) => {
                    Message::CommonSortExpectedConstantOverChoices { value, provenance }
                }
                (Case::Best | Case::Worst, Over::OrdersAndPivotChoices) => {
                    Message::CommonSortExtremeConstantOverChoices { value, provenance }
                }
                (Case::Average, Over::OrdersAtPowersOfTwo) => {
                    Message::CommonSortAveragePowersOfTwo {
                        formula: value.clone(),
                        value,
                        length: sort.length.clone(),
                        provenance,
                    }
                }
                (Case::Best | Case::Worst, Over::OrdersAtPowersOfTwo) => {
                    Message::CommonSortExtremePowersOfTwo {
                        formula: value.clone(),
                        value,
                        length: sort.length.clone(),
                        provenance,
                    }
                }
            };
            fields.push((cost_label(cost), text(&stated)));
            continue;
        }
        let (formula, value, length, provenance) = (
            formula.clone(),
            value.clone(),
            sort.length.clone(),
            provenance(cost),
        );
        let stated = match (cost.case, cost.over) {
            (Case::Average, Over::Orders) => Message::CommonSortAverage {
                formula,
                value,
                length,
                provenance,
            },
            (Case::Best | Case::Worst, Over::Orders) => Message::CommonSortExtreme {
                formula,
                value,
                length,
                provenance,
            },
            (Case::Average, Over::OrdersAndPivotChoices) => {
                Message::CommonSortExpectedOverChoices {
                    formula,
                    value,
                    length,
                    provenance,
                }
            }
            (Case::Best | Case::Worst, Over::OrdersAndPivotChoices) => {
                Message::CommonSortExtremeOverChoices {
                    formula,
                    value,
                    length,
                    provenance,
                }
            }
            (Case::Average, Over::OrdersAtPowersOfTwo) => Message::CommonSortAveragePowersOfTwo {
                formula,
                value,
                length,
                provenance,
            },
            (Case::Best | Case::Worst, Over::OrdersAtPowersOfTwo) => {
                Message::CommonSortExtremePowersOfTwo {
                    formula,
                    value,
                    length,
                    provenance,
                }
            }
        };
        fields.push((cost_label(cost), text(&stated)));
    }
    let lower_bound = match &sort.lower_bound {
        Some(bound) => Message::CommonSortLowerBound {
            bound: bound.clone(),
            length: sort.length.clone(),
        },
        None => Message::CommonSortLowerBoundWithoutComparisons,
    };
    fields.push((Message::CommonRecordLowerBound, text(&lower_bound)));
    fields
}

pub fn record_rows(
    record: &RecordSummary,
    reading: Option<Reading<'_>>,
    shown: RowOptions,
    locale: &Locale,
) -> Vec<Localized> {
    let text = |message: &Message| render(message, locale).to_string();
    let with_unit = |value: &str| match &record.unit {
        Some(unit) => text(&Message::CliQuantity {
            value: unit_operand(value),
            unit: unit.clone(),
        }),
        None => value.to_owned(),
    };
    let elided = shortened_digits(&record.value);
    let written = match &elided {
        Some((shortened, _)) => shortened.clone(),
        None => record.value.clone(),
    };
    let mut fields = vec![(Message::CommonRecordValue, with_unit(&written))];
    let mut breaks: Vec<usize> = Vec::new();
    if let Some((_, counts)) = &elided {
        fields.push((
            Message::CommonRecordDigits,
            text(&Message::CliRecordDigitCounts {
                counts: digit_counts(counts, locale),
            }),
        ));
    }
    if let Some(uncertainty) = &record.uncertainty {
        let value = text(&Message::CliUncertainty {
            standard: with_unit(&uncertainty.standard),
            coverage_factor: uncertainty.coverage_factor.clone(),
        });
        fields.push((Message::CommonRecordUncertainty, value));
        if let Some(inputs) = uncertainty.propagated_from_inputs {
            fields.push((
                Message::CommonRecordPropagation,
                text(&propagation_message(inputs)),
            ));
        }
    }
    fields.push((
        Message::CommonRecordKind,
        text(&record_kind_message(record)),
    ));
    match (
        &record.rounding_error,
        rounding_message(&record.rounding_error),
    ) {
        (RoundingSummary::Bound(bound), _) => {
            fields.push((
                Message::CommonRecordRoundingError,
                text(&rounding_bound_message(with_unit(bound))),
            ));
        }
        (_, Some(message)) => fields.push((Message::CommonRecordRoundingError, text(&message))),
        (_, None) => {}
    }
    fields.push((
        Message::CommonRecordMethod,
        text(&method_message(&record.method)),
    ));
    if let Some(sort) = &record.sort {
        fields.extend(sort_fields(sort, &text));
    }
    for note in &record.notes {
        match valid_where(note) {
            Some(excluding) => fields.push((
                Message::CommonRecordValid,
                text(&Message::CommonValidWhere {
                    count: excluding.len() as u64,
                    denominators: text(&crate::run::where_not_zero(&excluding, locale)),
                }),
            )),
            None => fields.push((Message::CommonRecordNotes, text(&diagnostic_message(note)))),
        }
    }
    if let Some(row) = ran_on_row(&record.runs, locale) {
        fields.push((Message::CommonRecordRanOn, row));
    }
    if shown.how_it_ran {
        fields.push((
            Message::CommonRecordHowItRan,
            how_it_ran_answer(record, locale),
        ));
    }
    if let Some((reading, distance)) = reading {
        breaks.push(fields.len());
        fields.push((Message::CommonRecordValue, with_unit(reading)));
        fields.push((
            Message::CommonRecordKind,
            text(&Message::CommonReadingNumber {
                digits: calc_app::READING_DIGITS.to_string(),
            }),
        ));
        fields.push((
            Message::CommonRecordMethod,
            text(&Message::CommonReadingComputed),
        ));
        if let Some(distance) = distance {
            let message = match distance {
                ReadingDistance::Below(distance) => Message::CommonReadingBelow {
                    distance: with_unit(distance),
                },
                ReadingDistance::Above(distance) => Message::CommonReadingAbove {
                    distance: with_unit(distance),
                },
                ReadingDistance::AtMost(distance) => Message::CommonReadingAtMost {
                    distance: with_unit(distance),
                },
            };
            fields.push((Message::CommonRecordRoundingError, text(&message)));
        }
        for note in record
            .notes
            .iter()
            .filter(|note| note_describes_the_quantity(note))
        {
            fields.push((Message::CommonRecordNotes, text(&diagnostic_message(note))));
        }
        breaks.push(fields.len());
    }
    if let Some(run_time) = record.measured_run_time {
        fields.push((
            Message::CommonRecordRunTime,
            text(&run_time_message(run_time)),
        ));
    }
    match &record.recognition {
        RecognitionSummary::NotRun => {}
        RecognitionSummary::Running => fields.push((
            Message::CommonRecordRecognized,
            text(&Message::CliRecognizedRunning),
        )),
        RecognitionSummary::Matched(matched) => fields.push((
            Message::CommonRecordRecognized,
            recognized_value(matched, locale),
        )),
        RecognitionSummary::NothingMatched => fields.push((
            Message::CommonRecordRecognized,
            text(&Message::CliRecognizedNothing),
        )),
        RecognitionSummary::CutShort(_) => fields.push((
            Message::CommonRecordRecognized,
            text(&Message::CliRecognizedCutShort),
        )),
        RecognitionSummary::Unavailable(reason) => fields.push((
            Message::CommonRecordRecognized,
            text(&recognition_unavailable_message(*reason)),
        )),
    }
    if let (true, RecognitionSummary::Matched(matched)) = (shown.offer, &record.recognition) {
        fields.push((
            Message::CommonRecordOffer,
            text(&match matched.truncated {
                Some(true) => Message::CliOfferCutShort,
                Some(false) => Message::CliOfferWhole,
                None => Message::CliOfferNotSaid,
            }),
        ));
    }
    if !record.dependencies.is_empty() {
        fields.push((
            Message::CommonRecordDependsOn,
            record.dependencies.join(LIST_SEPARATOR),
        ));
    }
    let labels: Vec<String> = fields.iter().map(|(label, _)| text(label)).collect();
    let width = labels
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0);
    labels
        .into_iter()
        .zip(fields)
        .enumerate()
        .flat_map(|(position, (label, (name, value)))| {
            let separator = breaks
                .contains(&position)
                .then(Localized::blank)
                .into_iter();
            let padding = PADDING.to_string().repeat(width - label.chars().count());
            let label = format!("{label}{padding}");
            let blank = PADDING.to_string().repeat(label.chars().count());
            let values = if name == Message::CommonRecordHowItRan {
                wrapped_value(&value, label.chars().count())
            } else {
                vec![value]
            };
            separator
                .chain(values.into_iter().enumerate().map(|(position, value)| {
                    let label = if position == 0 {
                        label.clone()
                    } else {
                        blank.clone()
                    };
                    render(&Message::CliRecordRow { label, value }, locale)
                }))
                .collect::<Vec<Localized>>()
        })
        .collect()
}

fn aligned_rows(fields: Vec<(Message, String)>, locale: &Locale) -> Vec<Localized> {
    indented_rows(
        fields
            .into_iter()
            .map(|(label, value)| (0, label, value))
            .collect(),
        locale,
    )
}

fn indented_rows(fields: Vec<(usize, Message, String)>, locale: &Locale) -> Vec<Localized> {
    let text = |message: &Message| render(message, locale).to_string();
    let labels: Vec<String> = fields
        .iter()
        .map(|(depth, label, _)| {
            let indent = PADDING.to_string().repeat(depth * STEP_INDENT);
            format!("{indent}{}", text(label))
        })
        .collect();
    let width = labels
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0);
    labels
        .into_iter()
        .zip(fields)
        .map(|(label, (_, _, value))| {
            let padding = PADDING.to_string().repeat(width - label.chars().count());
            render(
                &Message::CliRecordRow {
                    label: format!("{label}{padding}"),
                    value,
                },
                locale,
            )
        })
        .collect()
}

fn path_text(path: &[usize]) -> String {
    path.iter()
        .map(usize::to_string)
        .collect::<Vec<String>>()
        .join(".")
}

pub fn view_rows(outcome: &ViewOutcome, locale: &Locale) -> Vec<Localized> {
    let text = |message: &Message| render(message, locale).to_string();
    let written = |value: String, unit: &Option<String>| match unit {
        Some(unit) => text(&Message::CliQuantity {
            value: unit_operand(&value),
            unit: display_unit_text(unit),
        }),
        None => value,
    };
    let quantity = |value: &Number, unit: &Option<String>| {
        let value = value_text(&ResultValue::Number(value.clone()));
        match unit {
            Some(unit) => text(&Message::CliQuantity {
                value,
                unit: display_unit_text(unit),
            }),
            None => value,
        }
    };
    let fields = match outcome {
        ViewOutcome::Digits(outcome) => {
            let digits = &outcome.digits;
            let mut fields = vec![
                (Message::CommonViewDecimalPlaces, digits.places.to_string()),
                (
                    Message::CommonViewDigitsOf,
                    text(&match outcome.of {
                        DigitsOf::ExactValue => Message::CommonViewOfExactValue,
                        DigitsOf::MachineValue => Message::CommonViewOfMachineValue,
                    }),
                ),
                (
                    Message::CommonViewTruncated,
                    written(digits_text(&digits.digits), &outcome.unit),
                ),
            ];
            if let Some(remainder) = &digits.remainder {
                fields.push((
                    Message::CommonViewRemainder,
                    if digits.terminates {
                        text(&Message::CommonViewRemainderZero)
                    } else {
                        written(digits_text(remainder), &outcome.unit)
                    },
                ));
            }
            if let Some(uncertainty) = &outcome.uncertainty {
                fields.push((
                    Message::CommonViewUncertainty,
                    match uncertainty {
                        DigitsUncertainty::Standard {
                            standard,
                            terminates: true,
                        } => text(&Message::CliViewUncertainty {
                            standard: quantity(standard, &outcome.unit),
                        }),
                        DigitsUncertainty::Standard {
                            standard,
                            terminates: false,
                        } => text(&Message::CliViewUncertaintyCut {
                            standard: quantity(standard, &outcome.unit),
                        }),
                        DigitsUncertainty::BelowThePlace => {
                            text(&Message::CommonViewUncertaintyBelowPlace)
                        }
                        DigitsUncertainty::NotAScalar => {
                            text(&Message::CommonViewUncertaintyNotANumber)
                        }
                    },
                ));
            }
            fields.push((
                Message::CommonViewExpansion,
                text(&match digits.expansion {
                    Expansion::Ends => Message::CommonViewExpansionFinite,
                    Expansion::Recurs(period) => Message::CommonViewExpansionPeriod {
                        start: period.start.to_string(),
                        length: period.length.to_string(),
                    },
                    Expansion::RecursBeyondLimit => Message::CommonViewExpansionPeriodBeyondLimit {
                        limit: DECIMAL_PLACES_LIMIT.to_string(),
                    },
                    Expansion::NotKnownToRecur => Message::CommonViewExpansionNotKnownToRecur,
                }),
            ));
            fields
        }
        ViewOutcome::Working(WorkingOutcome::Refused(refusal)) => {
            vec![(
                Message::CommonWorkingSteps,
                text(&working_refusal_message(refusal)),
            )]
        }
        ViewOutcome::Working(WorkingOutcome::Steps {
            steps,
            further_steps,
            further_paths,
            rules: _,
        }) => {
            let mut fields: Vec<(usize, Message, String)> = steps
                .iter()
                .map(|step| {
                    let operands: Vec<&str> = step
                        .operands
                        .iter()
                        .map(|operand| operand.value.as_str())
                        .collect();
                    (
                        step.path.len(),
                        working_operation_message(&step.operation),
                        format!("{} = {}", operands.join(LIST_SEPARATOR), step.result),
                    )
                })
                .collect();
            if *further_steps > 0 {
                fields.push((
                    0,
                    Message::CommonWorkingSteps,
                    text(&Message::CommonWorkingFurtherSteps {
                        count: u64::try_from(*further_steps).unwrap_or(u64::MAX),
                    }),
                ));
                fields.extend(further_paths.iter().map(|path| {
                    (
                        0,
                        Message::CliWorkingAskFor,
                        text(&Message::CliWorkingPath {
                            path: path_text(path),
                        }),
                    )
                }));
            }
            return indented_rows(fields, locale);
        }
        ViewOutcome::Enclosure(outcome) => {
            let enclosure = &outcome.enclosure;
            vec![
                (
                    Message::CommonViewSignificantDigits,
                    enclosure.significant_digits.to_string(),
                ),
                (
                    Message::CommonViewEncloses,
                    text(&match outcome.of {
                        DigitsOf::ExactValue => Message::CommonViewEnclosesExactValue,
                        DigitsOf::MachineValue => Message::CommonViewEnclosesMachineValue,
                    }),
                ),
                (
                    Message::CommonViewLower,
                    quantity(&enclosure.lower, &outcome.unit),
                ),
                (
                    Message::CommonViewUpper,
                    quantity(&enclosure.upper, &outcome.unit),
                ),
                (
                    Message::CommonViewWidth,
                    quantity(&enclosure.width, &outcome.unit),
                ),
                (
                    Message::CommonViewPrecision,
                    text(if enclosure.reached {
                        &Message::CommonViewReached
                    } else {
                        &Message::CommonViewNotReached
                    }),
                ),
            ]
        }
    };
    aligned_rows(fields, locale)
}

fn place_number(place: &TracePlace) -> String {
    match place {
        TracePlace::Position(position)
        | TracePlace::Held {
            taken_from: position,
        }
        | TracePlace::Scratch(position) => position.to_string(),
    }
}

fn traced_message(entry: &TracedEntry) -> Message {
    match (&entry.key, &entry.place) {
        (None, TracePlace::Position(position)) => Message::CliTraceAt {
            value: entry.value.clone(),
            position: position.to_string(),
        },
        (None, TracePlace::Held { taken_from }) => Message::CliTraceHeld {
            value: entry.value.clone(),
            position: taken_from.to_string(),
        },
        (Some(key), TracePlace::Position(position)) => Message::CliTraceKeyedAt {
            value: entry.value.clone(),
            key: key.clone(),
            position: position.to_string(),
        },
        (Some(key), TracePlace::Held { taken_from }) => Message::CliTraceKeyedHeld {
            value: entry.value.clone(),
            key: key.clone(),
            position: taken_from.to_string(),
        },
        (None, TracePlace::Scratch(place)) => Message::CliTraceInScratch {
            value: entry.value.clone(),
            position: place.to_string(),
        },
        (Some(key), TracePlace::Scratch(place)) => Message::CliTraceKeyedInScratch {
            value: entry.value.clone(),
            key: key.clone(),
            position: place.to_string(),
        },
    }
}

fn trace_step_message(step: &TraceStep, locale: &Locale) -> Message {
    let text = |message: &Message| render(message, locale).to_string();
    match step {
        TraceStep::Compare {
            left,
            right,
            outcome,
        } => {
            let named =
                |entry: &TracedEntry| entry.key.clone().unwrap_or_else(|| entry.value.clone());
            let verdict = match outcome {
                Some(Ordering::Less) => Message::CliTraceSmaller { value: named(left) },
                Some(Ordering::Greater) => Message::CliTraceLarger { value: named(left) },
                Some(Ordering::Equal) => Message::CliTraceEqual,
                None => Message::CliTraceUndecided,
            };
            Message::CliTraceCompare {
                left: text(&traced_message(left)),
                right: text(&traced_message(right)),
                verdict: text(&verdict),
            }
        }
        TraceStep::Tally { value, counter } => Message::CliTraceTally {
            value: value.clone(),
            counter: counter.to_string(),
        },
        TraceStep::Draw {
            from,
            to,
            chosen,
            draws,
        } => Message::CliTraceDraw {
            from: from.to_string(),
            to: to.to_string(),
            chosen: chosen.to_string(),
            draws: *draws,
        },
        TraceStep::DigitPass { place, least } => Message::CliTraceDigitPass {
            place: place.to_string(),
            least: least.clone(),
        },
        TraceStep::DigitTally {
            value,
            digit,
            counter,
        } => Message::CliTraceDigitTally {
            value: value.clone(),
            digit: digit.to_string(),
            counter: counter.to_string(),
        },
        TraceStep::BeadFalls { value, pole } => Message::CliTraceBeadFalls {
            value: value.clone(),
            pole: pole.to_string(),
        },
        TraceStep::BeadRead { pole, row } => Message::CliTraceBeadRead {
            pole: pole.to_string(),
            row: row.to_string(),
        },
        TraceStep::Rebuild {
            row,
            position,
            beads,
        } => Message::CliTraceRebuild {
            row: row.to_string(),
            position: position.to_string(),
            beads: beads.to_string(),
        },
        TraceStep::Shuffle { number } => Message::CliTraceShuffle {
            number: number.to_string(),
        },
        TraceStep::ShuffleDraw {
            position,
            chosen,
            draws,
        } => Message::CliTraceShuffleDraw {
            position: position.to_string(),
            chosen: chosen.to_string(),
            draws: *draws,
        },
        TraceStep::BitonicMerge { block, distance } => Message::CliTraceBitonicMerge {
            block: block.to_string(),
            distance: distance.to_string(),
        },
        TraceStep::Pass { gap } => Message::CliTracePass {
            gap: gap.to_string(),
        },
        TraceStep::Flip { length } => Message::CliTraceFlip {
            length: length.to_string(),
        },
        TraceStep::Decrement { counter } => Message::CliTraceDecrement {
            counter: counter.to_string(),
        },
        TraceStep::PrefixSum { from, to } => Message::CliTracePrefixSum {
            from: from.to_string(),
            to: to.to_string(),
        },
        TraceStep::Exchange { left, right } => Message::CliTraceExchange {
            left: text(&traced_message(left)),
            right: text(&traced_message(right)),
        },
        TraceStep::Write { value, from, to } => match (from, to) {
            (TracePlace::Held { taken_from }, TracePlace::Position(to)) => Message::CliTracePut {
                value: value.clone(),
                from: taken_from.to_string(),
                to: to.to_string(),
            },
            (TracePlace::Position(from), TracePlace::Scratch(to)) => Message::CliTraceToScratch {
                value: value.clone(),
                from: from.to_string(),
                to: to.to_string(),
            },
            (TracePlace::Scratch(from), TracePlace::Position(to)) => Message::CliTraceFromScratch {
                value: value.clone(),
                from: from.to_string(),
                to: to.to_string(),
            },
            (from, to) => Message::CliTraceMove {
                value: value.clone(),
                from: place_number(from),
                to: place_number(to),
            },
        },
    }
}

pub fn trace_rows(steps: Option<&[TraceStep]>, locale: &Locale) -> Vec<Localized> {
    let Some(steps) = steps else {
        return vec![render(&Message::CliTraceNotASort, locale)];
    };
    let mut rows = vec![render(&Message::CliTraceHeading, locale)];
    let width = steps.len().to_string().len();
    for (index, step) in steps.iter().enumerate() {
        let number = format!("{:>width$}", index + 1);
        let step = render(&trace_step_message(step, locale), locale).to_string();
        rows.push(render(&Message::CliTraceStep { number, step }, locale));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_app::{
        BackendKind, Precision, Preference, ReplayState, ResultKind, SolveLineSummary,
        UncertaintySummary, UtcTimestamp, ValueForm,
    };

    fn record() -> RecordSummary {
        RecordSummary {
            measured_run_time: None,
            recognition: RecognitionSummary::NotRun,
            notes: Vec::new(),
            value: "24.525".to_owned(),
            unit: Some("N".to_owned()),
            kind: ResultKind::MachineFloat,
            value_form: Some(ValueForm::Machine64),
            uncertainty: None,
            rounding_error: RoundingSummary::Bound("4e-15".to_owned()),
            method: "plan_evaluation".to_owned(),
            runs: vec![BackendRun {
                selected: Some(BackendKind::Cpu),
                width: Domain::F64,
                modes: Vec::new(),
            }],
            dependencies: vec!["r1".to_owned(), "r2".to_owned()],
            sort: None,
        }
    }

    fn summary(state: LineState) -> LineSummary {
        LineSummary {
            label: "r3".to_owned(),
            name: Some("F".to_owned()),
            input: "F = m*a".to_owned(),
            identifies_number: false,
            state,
        }
    }

    fn texts(rows: Vec<Localized>) -> Vec<String> {
        rows.into_iter().map(|row| row.to_string()).collect()
    }

    #[test]
    fn result_rows_align_values_after_the_longest_label() {
        let rows = line_rows(
            &summary(LineState::Result(Box::new(record()))),
            None,
            RowOptions::default(),
            &Locale::source(),
        );
        assert_eq!(
            texts(rows),
            vec![
                "r3  F = m*a",
                "  value           24.525 N",
                "  number          machine float, 64-bit (f64)",
                "  rounding error  at most 4e-15 N, absolute",
                "  computed        in machine arithmetic",
                "  ran on          CPU",
                "  depends on      r1, r2",
            ]
        );
    }

    fn mode(
        operation: calc_app::PlanOp,
        mode: calc_app::ExecutionMode,
    ) -> calc_app::OperationModeUse {
        calc_app::OperationModeUse { operation, mode }
    }

    #[test]
    fn an_exact_line_says_no_backend_ran_an_operation() {
        let mut exact = record();
        exact.runs = Vec::new();

        assert_eq!(
            how_it_ran_answer(&exact, &Locale::source()),
            "evaluated exactly, so no backend ran an operation"
        );
    }

    #[test]
    fn a_backend_without_modes_says_it_ran_every_operation_the_same_way() {
        let ran = record();

        assert_eq!(
            how_it_ran_answer(&ran, &Locale::source()),
            "every operation the same way"
        );
    }

    #[test]
    fn the_answer_of_a_backend_with_modes_counts_them() {
        let mut ran = record();
        ran.runs[0].modes = vec![
            mode(calc_app::PlanOp::Add, calc_app::ExecutionMode::Native),
            mode(calc_app::PlanOp::Mul, calc_app::ExecutionMode::IntegerExact),
        ];

        assert_eq!(
            how_it_ran_answer(&ran, &Locale::source()),
            "1 natively, 1 integer-exact"
        );
    }

    #[test]
    fn the_how_it_ran_row_carries_the_label_of_the_cockpit_row() {
        let ran = record();

        let rows = texts(record_rows(
            &ran,
            None,
            RowOptions {
                how_it_ran: true,
                offer: false,
            },
            &Locale::source(),
        ));

        assert!(
            rows.iter()
                .any(|row| row.contains("how it ran")
                    && row.contains("every operation the same way"))
        );
    }

    #[test]
    fn the_rows_name_each_mode_and_the_operations_it_ran() {
        let mut ran = record();
        ran.runs[0].modes = vec![
            mode(calc_app::PlanOp::Add, calc_app::ExecutionMode::Native),
            mode(calc_app::PlanOp::Mul, calc_app::ExecutionMode::IntegerExact),
        ];

        let rows = texts(modes_rows(&ran, &Locale::source()));

        assert!(rows[0].starts_with("  natively  add"));
        assert!(rows[1].starts_with("  integer-exact  mul"));
    }

    #[test]
    fn the_operations_no_mode_names_are_not_offered() {
        let mut ran = record();
        ran.runs[0].modes = vec![mode(calc_app::PlanOp::Add, calc_app::ExecutionMode::Native)];

        let rows = texts(modes_rows(&ran, &Locale::source()));

        assert!(rows.iter().any(|row| row.contains("not offered")));
    }

    #[test]
    fn the_block_ends_with_what_a_native_mode_means() {
        let mut ran = record();
        ran.runs[0].modes = vec![mode(calc_app::PlanOp::Add, calc_app::ExecutionMode::Native)];

        let rows = texts(modes_rows(&ran, &Locale::source()));

        let sentence: String = rows
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(sentence.contains("It is not a claim"), "rows were {rows:?}");
    }

    #[test]
    fn no_row_of_the_block_is_wider_than_the_terminal() {
        let mut ran = record();
        ran.runs[0].modes = calc_app::PlanOp::ALL
            .into_iter()
            .map(|operation| mode(operation, calc_app::ExecutionMode::Native))
            .collect();

        for row in texts(modes_rows(&ran, &Locale::source())) {
            assert!(
                row.chars().count() <= 80,
                "{} columns: {row}",
                row.chars().count()
            );
        }
    }

    fn matched(concepts: &[&str], truncated: Option<bool>) -> RecognitionSummary {
        counted(concepts, Some(0), truncated)
    }

    fn counted(
        concepts: &[&str],
        unnamed: Option<u64>,
        truncated: Option<bool>,
    ) -> RecognitionSummary {
        RecognitionSummary::Matched(MatchedSummary {
            concepts: concepts.iter().map(|name| (*name).to_owned()).collect(),
            unnamed,
            truncated,
            truncated_by: None,
        })
    }

    fn recognition_row(recognition: RecognitionSummary) -> Option<String> {
        let mut shown = record();
        shown.recognition = recognition;
        texts(record_rows(
            &shown,
            None,
            RowOptions::default(),
            &Locale::source(),
        ))
        .into_iter()
        .find(|row| row.trim_start().starts_with("concept"))
    }

    #[test]
    fn a_recognition_that_never_ran_has_no_row() {
        assert_eq!(recognition_row(RecognitionSummary::NotRun), None);
    }

    #[test]
    fn a_running_recognition_says_it_is_still_running() {
        let row = recognition_row(RecognitionSummary::Running).expect("a row");

        assert!(row.contains("still being recognized"));
    }

    #[test]
    fn a_recognition_that_matched_names_its_concepts_in_the_locale() {
        let identifier = "pythagorean-theorem";
        let name = concept_name(identifier, &Locale::source()).expect("the concept has a name");

        let row = recognition_row(matched(&[identifier], Some(false))).expect("a row");

        assert!(row.contains(&name) && !row.contains(identifier), "{row}");
    }

    #[test]
    fn a_concept_this_version_cannot_name_is_counted_and_never_written() {
        let row = recognition_row(counted(&["pythagorean-theorem"], Some(1), Some(false)))
            .expect("a row");

        assert!(
            row.ends_with("Pythagorean theorem, and one concept this version does not know"),
            "{row}"
        );
    }

    #[test]
    fn concepts_this_version_cannot_name_are_counted_together() {
        let row = recognition_row(counted(&[], Some(2), Some(false))).expect("a row");

        assert!(
            row.ends_with("2 concepts this version does not know"),
            "{row}"
        );
    }

    #[test]
    fn a_row_of_names_a_count_and_a_truncation_takes_one_and() {
        let row =
            recognition_row(counted(&["pythagorean-theorem"], Some(1), Some(true))).expect("a row");

        assert_eq!(row.matches(" and ").count(), 1, "{row}");
        assert!(
            row.ends_with("Pythagorean theorem, one concept this version does not know, and more"),
            "{row}"
        );
    }

    #[test]
    fn a_concept_set_that_did_not_load_counts_nothing_and_says_so() {
        let row =
            recognition_row(counted(&["pythagorean-theorem"], None, Some(false))).expect("a row");

        assert!(row.contains("concept set did not load"), "{row}");
    }

    #[test]
    fn an_offer_that_was_cut_short_says_there_is_more() {
        let row = recognition_row(matched(&["pythagorean-theorem"], Some(true))).expect("a row");

        assert!(row.ends_with(", and more"), "{row}");
    }

    #[test]
    fn an_offer_that_was_not_cut_short_says_nothing_about_more() {
        let row = recognition_row(matched(&["pythagorean-theorem"], Some(false))).expect("a row");

        assert!(!row.contains("and more"), "{row}");
    }

    #[test]
    fn an_offer_that_does_not_say_whether_it_was_cut_short_claims_nothing() {
        let row = recognition_row(matched(&["pythagorean-theorem"], None)).expect("a row");

        assert!(!row.contains("and more"), "{row}");
    }

    #[test]
    fn a_recognition_that_matched_nothing_says_so() {
        let row = recognition_row(RecognitionSummary::NothingMatched).expect("a row");

        assert!(row.contains("none in the concept set matches"));
    }

    #[test]
    fn a_recognition_that_could_not_run_says_why() {
        let row = recognition_row(RecognitionSummary::Unavailable(
            calc_app::RecognitionUnavailable::ConceptSetNotLoaded,
        ))
        .expect("a row");

        assert!(row.contains("concept set did not load"));
    }

    #[test]
    fn measured_run_time_is_a_row_of_its_own() {
        let mut measured = record();
        measured.measured_run_time = Some(std::time::Duration::from_micros(1235));

        let rows = line_rows(
            &summary(LineState::Result(Box::new(measured))),
            None,
            RowOptions::default(),
            &Locale::source(),
        );

        assert!(
            texts(rows).contains(&"  run time        1.24 ms, measured on this run".to_owned())
        );
    }

    #[test]
    fn uncertainty_row_names_the_coverage_factor() {
        let mut measured = record();
        measured.uncertainty = Some(UncertaintySummary {
            standard: "0.11".to_owned(),
            coverage_factor: "1".to_owned(),
            propagated_from_inputs: Some(2),
        });
        let rows = texts(line_rows(
            &summary(LineState::Result(Box::new(measured))),
            None,
            RowOptions::default(),
            &Locale::source(),
        ));
        assert_eq!(
            (rows[2].as_str(), rows[3].as_str()),
            (
                "  uncertainty     ± 0.11 N, standard uncertainty, coverage factor k = 1",
                "  propagation     first order from 2 uncorrelated inputs"
            )
        );
    }

    #[test]
    fn measured_result_kind_names_its_measured_inputs() {
        let mut measured = record();
        measured.uncertainty = Some(UncertaintySummary {
            standard: "0.11".to_owned(),
            coverage_factor: "1".to_owned(),
            propagated_from_inputs: Some(2),
        });
        let rows = texts(line_rows(
            &summary(LineState::Result(Box::new(measured))),
            None,
            RowOptions::default(),
            &Locale::source(),
        ));
        assert_eq!(rows[4], "  number          derived from 2 measured inputs");
    }

    #[test]
    fn exact_result_states_its_method_and_leaves_out_rounding_and_backend() {
        let exact = RecordSummary {
            measured_run_time: None,
            recognition: RecognitionSummary::NotRun,
            notes: Vec::new(),
            value: "5".to_owned(),
            unit: None,
            kind: ResultKind::ExactRational,
            value_form: Some(ValueForm::ExactInteger),
            uncertainty: None,
            rounding_error: RoundingSummary::Exact,
            method: "exact_evaluation".to_owned(),
            runs: Vec::new(),
            dependencies: Vec::new(),
            sort: None,
        };
        let rows = texts(line_rows(
            &summary(LineState::Result(Box::new(exact))),
            None,
            RowOptions::default(),
            &Locale::source(),
        ));
        assert_eq!(
            rows,
            vec![
                "r3  F = m*a",
                "  value     5",
                "  number    exact integer",
                "  computed  exactly, without rounding",
            ]
        );
    }

    #[test]
    fn thirty_two_bit_machine_float_states_its_width() {
        let mut narrow = record();
        narrow.value_form = Some(ValueForm::Machine32);
        let rows = texts(line_rows(
            &summary(LineState::Result(Box::new(narrow))),
            None,
            RowOptions::default(),
            &Locale::source(),
        ));
        assert_eq!(rows[2], "  number          machine float, 32-bit (f32)");
    }

    #[test]
    fn unknown_rounding_error_names_how_it_was_rounded() {
        let mut unbounded = record();
        unbounded.rounding_error = RoundingSummary::Unknown;
        let rows = texts(line_rows(
            &summary(LineState::Result(Box::new(unbounded))),
            None,
            RowOptions::default(),
            &Locale::source(),
        ));
        assert_eq!(
            rows[3],
            "  rounding error  unknown, rounded in machine arithmetic"
        );
    }

    #[test]
    fn method_without_its_own_message_names_its_identifier() {
        let mut other = record();
        other.method = "gauss_kronrod_adaptive".to_owned();
        let rows = texts(line_rows(
            &summary(LineState::Result(Box::new(other))),
            None,
            RowOptions::default(),
            &Locale::source(),
        ));
        assert_eq!(
            rows[4],
            "  computed        by method gauss_kronrod_adaptive"
        );
    }

    #[test]
    fn solve_line_states_how_many_ways_were_found() {
        let rows = texts(line_rows(
            &summary(LineState::Solve(SolveLineSummary::Ways { found: 3 })),
            None,
            RowOptions::default(),
            &Locale::source(),
        ));
        assert_eq!(rows[1], "  3 ways found");
    }

    #[test]
    fn failed_line_shows_its_error_message() {
        let failed = LineState::Failed(Message::ErrorDivisionByZero {
            reading: "m/0".to_owned(),
        });
        let rows = texts(line_rows(
            &summary(failed),
            None,
            RowOptions::default(),
            &Locale::source(),
        ));
        assert_eq!(rows, vec!["r3  F = m*a", "  m/0 divides by zero"]);
    }

    #[test]
    fn not_evaluated_line_says_so() {
        let rows = texts(line_rows(
            &summary(LineState::NotEvaluated),
            None,
            RowOptions::default(),
            &Locale::source(),
        ));
        assert_eq!(rows[1], "  saved before it was computed");
    }

    fn status(replay: ReplayState, differing: Option<u64>) -> SessionStatus {
        SessionStatus {
            replay,
            results: 2,
            running: 0,
            differing,
            precision: Precision::F64,
            backend: Preference::Automatic,
        }
    }

    #[test]
    fn status_row_without_a_replay_leaves_out_the_differing_count() {
        assert_eq!(
            status_row(&status(ReplayState::NotRun, None), &Locale::source()).to_string(),
            "replay: not run  2 results  0 running  en  precision: f64  backend: automatic"
        );
    }

    fn replayed_at() -> UtcTimestamp {
        UtcTimestamp::from_milliseconds_since_unix_epoch(1_789_476_067_312)
    }

    #[test]
    fn status_row_after_a_replay_today_states_the_time_of_day() {
        let replay = ReplayState::Differs {
            at: replayed_at(),
            today: true,
        };

        assert_eq!(
            status_row(&status(replay, Some(1)), &Locale::source()).to_string(),
            "replay: differs 12:41:07  2 results  0 running  1 differs  en  precision: f64  backend: automatic"
        );
    }

    #[test]
    fn status_row_after_a_replay_on_another_day_states_its_date() {
        let replay = ReplayState::Verified {
            at: replayed_at(),
            today: false,
        };

        assert_eq!(
            status_row(&status(replay, Some(0)), &Locale::source()).to_string(),
            "replay: verified 2026-09-15 12:41:07  2 results  0 running  0 differ  en  precision: f64  backend: automatic"
        );
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;
    use calc_app::{BackendKind, ResultKind};

    fn record() -> RecordSummary {
        RecordSummary {
            measured_run_time: None,
            recognition: RecognitionSummary::NotRun,
            notes: Vec::new(),
            value: "1.4142135623730951".to_owned(),
            unit: None,
            kind: ResultKind::MachineFloat,
            value_form: None,
            uncertainty: None,
            rounding_error: RoundingSummary::Exact,
            method: "machine_evaluation".to_owned(),
            runs: vec![BackendRun {
                selected: Some(BackendKind::Cpu),
                width: Domain::F64,
                modes: Vec::new(),
            }],
            dependencies: Vec::new(),
            sort: None,
        }
    }

    #[test]
    fn the_how_it_ran_row_holds_the_terminal_width_in_every_locale() {
        for locale in Locale::shipped() {
            for row in record_rows(
                &record(),
                None,
                RowOptions {
                    how_it_ran: true,
                    offer: false,
                },
                &locale,
            ) {
                assert!(
                    row.to_string().chars().count() <= 80,
                    "{} columns: {row}",
                    row.to_string().chars().count()
                );
            }
        }
    }
}
