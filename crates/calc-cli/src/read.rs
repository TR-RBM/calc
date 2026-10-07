use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::channel;

use calc_app::{
    Job, JobState, PictureSlot, PlotRequest, ReadCoordinate, ReadEvent, ReadRequest, Session,
    exact_number, plot_error_message, read_error_message,
};
use calc_i18n::{LanguageTag, Locale, Message, render};
use calc_render::geometry::ScaleFactor;
use calc_render::{FontSet, LayoutRequest, plot_layout};

use crate::arguments::Input;
use crate::completion::{OptionSpec, Value, flag, free, is_listed};
use crate::gpu::Probes;
use crate::output::Output;
use crate::plot::{
    Failure, JSON_OPTION_NAME, LOCALE_OPTION_NAME, OPTION_PREFIX_TEXT, PARAM_OPTION_NAME,
    PLOT_DEFAULT_HEIGHT, PLOT_DEFAULT_WIDTH, PLOT_SCALE_PERCENT, PLOT_TEXT_SIZE, PlotUsageError,
    SESSION_FILE_SUFFIX, SIZE_OPTION_NAME, VIEW_OPTION_NAME, ViewOption, applied_views, failure,
    legend_requests, open_session, parameters_of, picture_text, sampled_scene, size_of, text_of,
    view_request,
};
use crate::run::{Context, EXIT_FAILURE, EXIT_SUCCESS, EXIT_USAGE, locale_asked_for};
use crate::text::{RowOptions, line_rows, orbit_message, record_rows};

pub const READ_COMMAND: &str = "read";

const INVOCATION_SLOT: PictureSlot = PictureSlot(0);

const AT_OPTION: &str = "--at";
const LAYER_OPTION: &str = "--layer";
const COMMIT_OPTION: &str = "--commit";

pub(crate) static OPTIONS: &[OptionSpec] = &[
    free(AT_OPTION, Message::CliCompleteAt),
    free(LAYER_OPTION, Message::CliCompleteLayer),
    flag(COMMIT_OPTION, Message::CliCompleteCommit),
    free(VIEW_OPTION_NAME, Message::CliCompleteView),
    free(PARAM_OPTION_NAME, Message::CliCompleteParam),
    free(SIZE_OPTION_NAME, Message::CliCompleteSize),
    flag(JSON_OPTION_NAME, Message::CliCompleteJson),
    OptionSpec {
        name: LOCALE_OPTION_NAME,
        values: &[Value::Locale],
        description: Message::CliCompleteLocale,
    },
];
const COORDINATE_SEPARATOR: char = ',';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadInvocation {
    pub input: Input,
    pub line: Option<String>,
    pub at: Vec<String>,
    pub layer: usize,
    pub views: Vec<ViewOption>,
    pub parameters: Vec<String>,
    pub width: u16,
    pub height: u16,
    pub is_commit: bool,
    pub is_json: bool,
    pub locale: Option<LanguageTag>,
}

pub fn parse_read_arguments(arguments: &[OsString]) -> Result<ReadInvocation, PlotUsageError> {
    let mut input = None;
    let mut line = None;
    let mut at = Vec::new();
    let mut layer = 0;
    let mut views = Vec::new();
    let mut parameters = Vec::new();
    let mut size = (PLOT_DEFAULT_WIDTH, PLOT_DEFAULT_HEIGHT);
    let mut is_commit = false;
    let mut is_json = false;
    let mut locale = None;
    let mut remaining = arguments.iter().peekable();
    while let Some(argument) = remaining.next() {
        let text = text_of(argument)?;
        let mut value = |option: &str| {
            remaining
                .next()
                .ok_or_else(|| PlotUsageError::MissingOptionValue(option.to_owned()))
                .and_then(text_of)
        };
        match text.as_str() {
            option if option.starts_with(OPTION_PREFIX_TEXT) && !is_listed(OPTIONS, option) => {
                return Err(PlotUsageError::UnknownOption(text));
            }
            AT_OPTION => {
                at = value(AT_OPTION)?
                    .split(COORDINATE_SEPARATOR)
                    .map(|coordinate| coordinate.trim().to_owned())
                    .collect();
            }
            LAYER_OPTION => {
                let given = value(LAYER_OPTION)?;
                layer = given
                    .parse::<usize>()
                    .map_err(|_| PlotUsageError::UnexpectedArgument(given))?;
            }
            PARAM_OPTION_NAME => parameters.push(value(PARAM_OPTION_NAME)?),
            SIZE_OPTION_NAME => {
                let given = value(SIZE_OPTION_NAME)?;
                size = size_of(&given).ok_or(PlotUsageError::InvalidSize(given))?;
            }
            LOCALE_OPTION_NAME => {
                let given = value(LOCALE_OPTION_NAME)?;
                locale = Some(
                    LanguageTag::parse(&given).map_err(|_| PlotUsageError::InvalidLocale(given))?,
                );
            }
            VIEW_OPTION_NAME => {
                let range = value(VIEW_OPTION_NAME)?;
                let unit = match remaining.peek().and_then(|next| next.to_str()) {
                    Some(next) if !next.starts_with(OPTION_PREFIX_TEXT) && input.is_some() => {
                        remaining.next();
                        Some(next.to_owned())
                    }
                    _ => None,
                };
                views.push(ViewOption { range, unit });
            }
            COMMIT_OPTION => is_commit = true,
            JSON_OPTION_NAME => is_json = true,
            option if option.starts_with(OPTION_PREFIX_TEXT) => {
                return Err(PlotUsageError::UnknownOption(text));
            }
            _ if input.is_none() => {
                input = Some(if text.ends_with(SESSION_FILE_SUFFIX) {
                    Input::SessionFile(PathBuf::from(text))
                } else {
                    Input::Expression(text)
                });
            }
            _ if matches!(input, Some(Input::SessionFile(_))) && line.is_none() => {
                line = Some(text);
            }
            _ => return Err(PlotUsageError::UnexpectedArgument(text)),
        }
    }
    let input = input.ok_or(PlotUsageError::NoInput)?;
    if matches!(input, Input::SessionFile(_)) && line.is_none() {
        return Err(PlotUsageError::NoLine);
    }
    if at.is_empty() {
        return Err(PlotUsageError::NoCoordinates);
    }
    Ok(ReadInvocation {
        input,
        line,
        at,
        layer,
        views,
        parameters,
        width: size.0,
        height: size.1,
        is_commit,
        is_json,
        locale,
    })
}

fn coordinates_of(invocation: &ReadInvocation) -> Result<Vec<ReadCoordinate>, Failure> {
    invocation
        .at
        .iter()
        .map(|text| {
            exact_number(text)
                .map(ReadCoordinate::Exact)
                .ok_or_else(|| {
                    failure(Message::CliErrorInvalidCoordinate {
                        value: text.clone(),
                    })
                })
        })
        .collect()
}

fn plotted(
    session: &mut Session,
    line: calc_app::LineId,
    invocation: &ReadInvocation,
    locale: &Locale,
) -> Result<u64, Failure> {
    let parameters = if invocation.parameters.is_empty() {
        None
    } else {
        Some(parameters_of(&invocation.parameters)?)
    };
    let defaults = session
        .picture_defaults(line, parameters.as_deref())
        .map_err(|error| failure(plot_error_message(&error)))?;
    let views = applied_views(&defaults, &invocation.views)?;
    let text = picture_text(locale, &defaults);
    let fonts = FontSet::bundled().map_err(|_| {
        failure(Message::ErrorRenderFailed {
            code: String::from("fonts"),
        })
    })?;
    let scale = ScaleFactor::from_percent(PLOT_SCALE_PERCENT).map_err(|_| {
        failure(Message::ErrorRenderFailed {
            code: String::from("scale"),
        })
    })?;
    let layout = plot_layout(
        &LayoutRequest {
            width: invocation.width,
            height: invocation.height,
            text_size: PLOT_TEXT_SIZE,
            scale,
            views: views.iter().map(view_request).collect(),
            legends: legend_requests(defaults.legend, &text),
        },
        &fonts,
    )
    .map_err(|_| {
        failure(Message::ErrorRenderTextDoesNotFit {
            width: invocation.width.to_string(),
            height: invocation.height.to_string(),
        })
    })?;
    let request = PlotRequest {
        line,
        slot: INVOCATION_SLOT,
        iteration_limit: None,
        views: Some(views),
        divisions: layout.views.iter().map(|view| view.divisions()).collect(),
        parameters,
    };
    sampled_scene(session, &request)?;
    session
        .latest_picture_generation(line, INVOCATION_SLOT)
        .ok_or(failure(Message::ErrorReadSceneNotComplete))
}

fn saved(
    session: &Session,
    invocation: &ReadInvocation,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
) -> Result<(), Failure> {
    let Input::SessionFile(path) = &invocation.input else {
        return Ok(());
    };
    let bytes = session
        .save_to_bytes()
        .map_err(|error| failure(calc_app::save_error_message(&error)))?;
    let path_text = path.display().to_string();
    (context.write_file)(path, &bytes).map_err(|_| {
        failure(Message::CliErrorFileNotWritten {
            path: path_text.clone(),
        })
    })?;
    output
        .line(&render(
            &Message::CliSessionWritten { path: path_text },
            locale,
        ))
        .map_err(|_| failure(Message::CliErrorOutputFailed))
}

fn take(
    invocation: &ReadInvocation,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    probes: &mut Probes,
) -> Result<(), Failure> {
    let (mut session, line, probe) =
        open_session(&invocation.input, invocation.line.as_ref(), context)?;
    probes.watch(probe);
    let generation = plotted(&mut session, line, invocation, locale)?;
    let request = ReadRequest {
        line,
        slot: INVOCATION_SLOT,
        generation,
        layer: invocation.layer,
        at: coordinates_of(invocation)?,
        commit: invocation.is_commit,
    };
    if invocation.is_commit {
        let id = session
            .commit_reading(&request)
            .map_err(|error| failure(read_error_message(&error)))?;
        let line = session
            .line(id)
            .ok_or(failure(Message::ErrorReadSceneNotComplete))?;
        if invocation.is_json {
            let json = session
                .line_json(line)
                .map_err(|error| failure(calc_app::save_error_message(&error)))?;
            output
                .json(&json)
                .map_err(|_| failure(Message::CliErrorOutputFailed))?;
        } else {
            for row in line_rows(
                &session.line_summary(line),
                None,
                RowOptions::default(),
                locale,
            ) {
                output
                    .line(&row)
                    .map_err(|_| failure(Message::CliErrorOutputFailed))?;
            }
        }
        return saved(&session, invocation, context, locale, output);
    }
    let (events, received) = channel();
    let mut job = session
        .read(&request, events)
        .map_err(|error| failure(read_error_message(&error)))?;
    while job.step() == JobState::Pending {}
    let record = match received.try_iter().last() {
        Some(ReadEvent::OrbitRead { orbit, .. }) => {
            if invocation.is_json {
                return output
                    .json(&session.orbit_json(orbit))
                    .map_err(|_| failure(Message::CliErrorOutputFailed));
            }
            return output
                .line(&render(&orbit_message(orbit), locale))
                .map_err(|_| failure(Message::CliErrorOutputFailed));
        }
        Some(ReadEvent::ReadingFinished { value, .. }) => value,
        Some(ReadEvent::ReadingFailed { error, .. }) => {
            return Err(failure(calc_app::diagnostic_message(&error)));
        }
        None => return Err(failure(Message::ErrorReadSceneNotComplete)),
    };
    if invocation.is_json {
        let json = session
            .reading_json(line, &record)
            .map_err(|error| failure(calc_app::save_error_message(&error)))?;
        output
            .json(&json)
            .map_err(|_| failure(Message::CliErrorOutputFailed))
    } else {
        for row in record_rows(
            &session.record_summary(&record),
            None,
            RowOptions::default(),
            locale,
        ) {
            output
                .line(&row)
                .map_err(|_| failure(Message::CliErrorOutputFailed))?;
        }
        Ok(())
    }
}

pub fn run_read(
    arguments: &[OsString],
    context: &Context<'_>,
    standard_output: &mut dyn Write,
    standard_error: &mut dyn Write,
) -> u8 {
    let mut output = Output::new(standard_output);
    let mut errors = Output::new(standard_error);
    let invocation = match parse_read_arguments(arguments) {
        Ok(invocation) => invocation,
        Err(error) => {
            let locale = match error {
                PlotUsageError::InvalidLocale(_) => Locale::source(),
                _ => crate::run::usage_error_locale(arguments, context),
            };
            let detail = render(&error.message(), &locale).to_string();
            let _ = errors
                .line(&render(&Message::CliError { detail }, &locale))
                .and_then(|()| errors.line(&render(&Message::CliHelpUsage, &locale)));
            return EXIT_USAGE;
        }
    };
    let locale = locale_asked_for(invocation.locale.as_ref(), context, &mut errors);
    let mut probes = Probes::default();
    let outcome = take(&invocation, context, &locale, &mut output, &mut probes);
    probes.report(&locale, &mut errors);
    match outcome {
        Ok(()) => EXIT_SUCCESS,
        Err(Failure(message)) => {
            let detail = render(&message, &locale).to_string();
            let _ = errors.line(&render(&Message::CliError { detail }, &locale));
            EXIT_FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(texts: &[&str]) -> Vec<OsString> {
        texts.iter().map(OsString::from).collect()
    }

    #[test]
    fn coordinates_are_taken_one_per_axis_from_a_comma_list() {
        let invocation =
            parse_read_arguments(&arguments(&["grid.calc", "r1", "--at", "1/2, 3"])).unwrap();

        assert_eq!(invocation.at, vec!["1/2".to_owned(), "3".to_owned()]);
    }

    #[test]
    fn a_read_without_at_is_a_usage_error() {
        let outcome = parse_read_arguments(&arguments(&["grid.calc", "r1"])).err();

        assert_eq!(outcome, Some(PlotUsageError::NoCoordinates));
    }

    #[test]
    fn commit_and_layer_are_read_from_their_options() {
        let invocation = parse_read_arguments(&arguments(&[
            "grid.calc",
            "r1",
            "--at",
            "1",
            "--layer",
            "2",
            "--commit",
        ]))
        .unwrap();

        assert_eq!((invocation.layer, invocation.is_commit), (2, true));
    }

    #[test]
    fn a_session_file_without_a_line_is_a_usage_error() {
        let outcome = parse_read_arguments(&arguments(&["grid.calc", "--at", "1"])).err();

        assert_eq!(outcome, Some(PlotUsageError::NoLine));
    }
}
