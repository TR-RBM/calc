use calc_app::{Number, OdeError, OdeReport, OdeTime, ResultValue, display_unit_text, value_text};
use calc_app::{enclose_between, parse_error_message};
use calc_i18n::{Locale, Localized, Message, render};

const PADDING: char = ' ';
const READABLE_LENGTH: usize = 20;
const ROUNDED_DIGITS: u32 = 15;

pub(crate) enum OdeFault {
    Input,
    Method,
    Calc,
}

fn quantity(value: &Number, unit: Option<&String>, locale: &Locale) -> String {
    let value = value_text(&ResultValue::Number(value.clone()));
    match unit {
        Some(unit) => render(
            &Message::CliQuantity {
                value,
                unit: display_unit_text(unit),
            },
            locale,
        )
        .to_string(),
        None => value,
    }
}

fn time_text(name: &str, time: &Number, unit: Option<&String>, locale: &Locale) -> String {
    render(
        &Message::CliOdeTime {
            name: name.to_owned(),
            value: quantity(time, unit, locale),
        },
        locale,
    )
    .to_string()
}

fn aligned(fields: Vec<(String, String)>, locale: &Locale) -> Vec<Localized> {
    let width = fields
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0);
    fields
        .into_iter()
        .map(|(label, value)| {
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

pub(crate) fn ode_rows(report: &OdeReport, locale: &Locale) -> Vec<Localized> {
    let text = |message: &Message| render(message, locale).to_string();
    let time_unit = report.time_unit.as_ref();
    let mut rows = Vec::new();
    for point in &report.points {
        let at = match &point.written {
            Some(written) => render(
                &Message::CliOdeTime {
                    name: report.time_name.clone(),
                    value: written.clone(),
                },
                locale,
            )
            .to_string(),
            None => time_text(&report.time_name, &point.time, time_unit, locale),
        };
        rows.push(render(&Message::CliOdeAt { time: at.clone() }, locale));
        let mut fields = Vec::new();
        for (component, value) in report.components.iter().zip(&point.values) {
            let unit = component.unit.as_ref();
            fields.push((
                component.name.clone(),
                text(&Message::CliOdeBetween {
                    lower: quantity(&value.lower, unit, locale),
                    upper: quantity(&value.upper, unit, locale),
                }),
            ));
            fields.push((
                text(&Message::CliOdeLabelIntervalBound {
                    name: component.name.clone(),
                }),
                text(&Message::CliOdeIntervalBound {
                    width: quantity(&value.width, unit, locale),
                    time: at.clone(),
                }),
            ));
        }
        rows.extend(aligned(fields, locale));
    }
    rows.push(render(&Message::CliOdeWholeRun, locale));
    let mut fields = Vec::new();
    for component in &report.components {
        fields.push((
            text(&Message::CliOdeLabelStepBound {
                name: component.name.clone(),
            }),
            text(&Message::CliOdeStepBound {
                bound: quantity(&component.step_bound, component.unit.as_ref(), locale),
            }),
        ));
    }
    fields.push((
        text(&Message::CliOdeLabelMethod),
        text(&Message::CliOdeMethod {
            order: report.order.to_string(),
            steps: u64::try_from(report.steps).unwrap_or(u64::MAX),
        }),
    ));
    rows.extend(aligned(fields, locale));
    rows
}

fn reached_text(time: &OdeTime, value: &Number, locale: &Locale) -> String {
    let exact = time_text(&time.name, value, time.unit.as_ref(), locale);
    if exact.chars().count() <= READABLE_LENGTH {
        return exact;
    }
    match enclose_between(value, value, ROUNDED_DIGITS) {
        Ok(rounded) => render(
            &Message::CliOdeRoundedTime {
                time: time_text(&time.name, &rounded.lower, time.unit.as_ref(), locale),
                digits: ROUNDED_DIGITS.to_string(),
            },
            locale,
        )
        .to_string(),
        Err(_) => exact,
    }
}

pub(crate) fn ode_error(error: &OdeError, locale: &Locale) -> (Message, OdeFault) {
    let at = |time: &OdeTime, value: &Number| reached_text(time, value, locale);
    match error {
        OdeError::Unreadable(unreadable) => (
            Message::CliOdeUnreadable {
                part: unreadable.part.clone(),
                detail: render(
                    &parse_error_message(&unreadable.error, &unreadable.part),
                    locale,
                )
                .to_string(),
            },
            OdeFault::Input,
        ),
        OdeError::NotAnAssignment { part } => (
            Message::CliOdeNotAnAssignment { part: part.clone() },
            OdeFault::Input,
        ),
        OdeError::NotADerivative { part } => (
            Message::CliOdeNotADerivative { part: part.clone() },
            OdeFault::Input,
        ),
        OdeError::TimeNamesDiffer { first, second } => (
            Message::CliOdeTimeNamesDiffer {
                first: first.clone(),
                second: second.clone(),
            },
            OdeFault::Input,
        ),
        OdeError::ComponentTwice { name } => (
            Message::CliOdeComponentTwice { name: name.clone() },
            OdeFault::Input,
        ),
        OdeError::NoInitialValue { name } => (
            Message::CliOdeNoInitialValue { name: name.clone() },
            OdeFault::Input,
        ),
        OdeError::NotInTheSystem { name } => (
            Message::CliOdeNotInTheSystem { name: name.clone() },
            OdeFault::Input,
        ),
        OdeError::NoTimes => (Message::CliOdeNoTimes, OdeFault::Input),
        OdeError::UnknownName { name, part } => (
            Message::CliOdeUnknownName {
                name: name.clone(),
                part: part.clone(),
            },
            OdeFault::Input,
        ),
        OdeError::UnitMismatch {
            name,
            time,
            found,
            wanted,
        } => (
            Message::CliOdeUnitMismatch {
                name: name.clone(),
                time: time.clone(),
                found: display_unit_text(found),
                wanted: display_unit_text(wanted),
            },
            OdeFault::Input,
        ),
        OdeError::UnitsDoNotCombine { name } => (
            Message::CliOdeUnitsDoNotCombine { name: name.clone() },
            OdeFault::Input,
        ),
        OdeError::TimeUnitMismatch { part } => (
            Message::CliOdeTimeUnitMismatch { part: part.clone() },
            OdeFault::Input,
        ),
        OdeError::UnitWithOffset { name } => (
            Message::CliOdeUnitWithOffset { name: name.clone() },
            OdeFault::Input,
        ),
        OdeError::NotARealNumber { part } => (
            Message::CliOdeNotARealNumber { part: part.clone() },
            OdeFault::Input,
        ),
        OdeError::TimeNotRational { part } => (
            Message::CliOdeTimeNotRational { part: part.clone() },
            OdeFault::Input,
        ),
        OdeError::Unsupported { part } => (
            Message::CliOdeUnsupported { part: part.clone() },
            OdeFault::Input,
        ),
        OdeError::DivisionByZero { part } => (
            Message::CliOdeDivisionByZero { part: part.clone() },
            OdeFault::Input,
        ),
        OdeError::StopBeforeStart => (Message::CliOdeStopBeforeStart, OdeFault::Input),
        OdeError::NotEnclosed(halted) => (
            Message::CliOdeNotEnclosed {
                time: at(&halted.time, &halted.at),
                halvings: halted.halvings.to_string(),
            },
            OdeFault::Method,
        ),
        OdeError::TooManySteps(limit) => (
            Message::CliOdeTooManySteps {
                steps: limit.steps.to_string(),
                step: quantity(&limit.step, limit.time.unit.as_ref(), locale),
                time: at(&limit.time, &limit.at),
                stop: at(&limit.time, &limit.stop),
            },
            OdeFault::Method,
        ),
        OdeError::Inconsistent => (Message::CliOdeInconsistent, OdeFault::Calc),
    }
}
