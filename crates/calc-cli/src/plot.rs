use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

use calc_app::{
    AxisDisplayUnit, AxisState, ColourLegend, IterationLimitRule, Job, JobState, LineId,
    PictureDefaults, PictureEvent, PictureParameter, PictureSlot, PlotRequest, Session,
    axis_title_text, exact_number, plot_error_message, plot_json, registered_backends,
    session_error_message,
};
use calc_i18n::{LanguageTag, Locale, Message, render};
use calc_render::geometry::ScaleFactor;
use calc_render::image::encode_png;
use calc_render::theme::Theme;
use calc_render::{
    AxisRange, FontSet, Interval, LayoutRequest, LegendRequest, PictureText, RenderError,
    RenderRequest, Scene, ViewKind, ViewRequest, plot_layout, render_scene,
};

use crate::arguments::Input;
use crate::completion::{OptionSpec, Value, flag, free, is_listed};
use crate::gpu::{ProbeOutcome, Probes, probing_session};
use crate::output::Output;
use crate::run::{Context, EXIT_FAILURE, EXIT_SUCCESS, EXIT_USAGE, locale_asked_for, open_file};

pub const PLOT_COMMAND: &str = "plot";

const INVOCATION_SLOT: PictureSlot = PictureSlot(0);

pub(crate) const VIEW_OPTION_NAME: &str = "--view";
pub(crate) const PARAM_OPTION_NAME: &str = "--param";
pub(crate) const SIZE_OPTION_NAME: &str = "--size";
pub(crate) const JSON_OPTION_NAME: &str = "--json";
pub(crate) const LOCALE_OPTION_NAME: &str = "--locale";
pub(crate) const OPTION_PREFIX_TEXT: &str = "--";
pub(crate) const SESSION_FILE_SUFFIX: &str = ".calc";
pub(crate) const PLOT_TEXT_SIZE: u16 = 14;
pub(crate) const PLOT_SCALE_PERCENT: u16 = 100;
pub(crate) const PLOT_DEFAULT_WIDTH: u16 = 800;
pub(crate) const PLOT_DEFAULT_HEIGHT: u16 = 600;

const OUTPUT_OPTION: &str = "--output";
const VIEW_OPTION: &str = "--view";
const PARAM_OPTION: &str = "--param";
const SIZE_OPTION: &str = "--size";
const JSON_OPTION: &str = "--json";
const SETTLE_OPTION: &str = "--settle";
const LIMIT_OPTION: &str = "--limit";
const FIXED_RULE: &str = "fixed";
const FOLLOWING_RULE: &str = "following";
const RULE_SEPARATOR: char = ':';
const LIMIT_SEPARATOR: char = ',';
const LARGEST_ITERATIONS: u32 = 65_536;
const LOCALE_OPTION: &str = "--locale";
const OPTION_PREFIX: &str = "--";

pub(crate) static OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        name: OUTPUT_OPTION,
        values: &[Value::File],
        description: Message::CliCompleteOutput,
    },
    free(VIEW_OPTION, Message::CliCompleteView),
    free(PARAM_OPTION, Message::CliCompleteParam),
    free(SIZE_OPTION, Message::CliCompleteSize),
    free(LIMIT_OPTION, Message::CliCompleteLimit),
    flag(SETTLE_OPTION, Message::CliCompleteSettle),
    flag(JSON_OPTION, Message::CliCompleteJson),
    OptionSpec {
        name: LOCALE_OPTION,
        values: &[Value::Locale],
        description: Message::CliCompleteLocale,
    },
];
const RANGE_SEPARATOR: &str = "..";
const PARAMETER_SEPARATOR: char = '=';
const SIZE_SEPARATOR: char = 'x';
const SESSION_FILE_EXTENSION: &str = ".calc";
const DEFAULT_WIDTH: u16 = 800;
const DEFAULT_HEIGHT: u16 = 600;
const TEXT_SIZE: u16 = 14;
const SCALE_PERCENT: u16 = 100;
const DECIMAL_SEPARATOR: char = '.';
const PICTURE_SCALE: calc_render::Scale = calc_render::Scale::Linear;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewOption {
    pub range: String,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlotInvocation {
    pub input: Input,
    pub line: Option<String>,
    pub output: PathBuf,
    pub views: Vec<ViewOption>,
    pub parameters: Vec<String>,
    pub width: u16,
    pub height: u16,
    pub is_settle: bool,
    pub limit: Option<IterationLimitRule>,
    pub is_json: bool,
    pub locale: Option<LanguageTag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlotUsageError {
    MissingOptionValue(String),
    UnknownOption(String),
    UnexpectedArgument(String),
    InvalidLocale(String),
    InvalidSize(String),
    InvalidLimit(String),
    LimitGivenTwice,
    NoInput,
    NoOutput,
    NoLine,
    NoCoordinates,
}

impl PlotUsageError {
    pub(crate) fn message(&self) -> Message {
        match self {
            Self::MissingOptionValue(option) => Message::CliErrorMissingOptionValue {
                option: option.clone(),
            },
            Self::UnknownOption(option) => Message::CliErrorUnknownOption {
                option: option.clone(),
            },
            Self::UnexpectedArgument(argument) => Message::CliErrorUnexpectedArgument {
                argument: argument.clone(),
            },
            Self::InvalidLocale(value) => Message::CliErrorInvalidLocale {
                value: value.clone(),
            },
            Self::InvalidSize(value) => Message::CliErrorInvalidSize {
                value: value.clone(),
            },
            Self::InvalidLimit(value) => Message::CliErrorInvalidLimit {
                value: value.clone(),
            },
            Self::LimitGivenTwice => Message::CliErrorLimitGivenTwice,
            Self::NoInput => Message::CliErrorNoInput,
            Self::NoOutput => Message::CliErrorPlotNeedsOutput,
            Self::NoLine => Message::CliErrorPlotNeedsLine,
            Self::NoCoordinates => Message::CliErrorReadNeedsCoordinates,
        }
    }
}

pub(crate) fn text_of(argument: &OsString) -> Result<String, PlotUsageError> {
    argument
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| PlotUsageError::UnexpectedArgument(argument.to_string_lossy().into_owned()))
}

pub(crate) fn size_of(text: &str) -> Option<(u16, u16)> {
    let (width, height) = text.split_once(SIZE_SEPARATOR)?;
    let width = width.parse::<u16>().ok().filter(|width| *width > 0)?;
    let height = height.parse::<u16>().ok().filter(|height| *height > 0)?;
    Some((width, height))
}

fn iterations_of(text: &str, largest: u32) -> Option<u32> {
    text.parse::<u32>()
        .ok()
        .filter(|value| (1..=largest).contains(value))
}

fn limit_of(text: &str) -> Option<IterationLimitRule> {
    let (rule, values) = text.split_once(RULE_SEPARATOR)?;
    match rule {
        FIXED_RULE => Some(IterationLimitRule::Fixed {
            iterations: iterations_of(values, LARGEST_ITERATIONS)?,
        }),
        FOLLOWING_RULE => {
            let parts: Vec<&str> = values.split(LIMIT_SEPARATOR).collect();
            let [base, per_halving, cap] = parts.as_slice() else {
                return None;
            };
            Some(IterationLimitRule::FollowingDepth {
                base: iterations_of(base, u32::MAX)?,
                per_halving: iterations_of(per_halving, u32::MAX)?,
                cap: iterations_of(cap, LARGEST_ITERATIONS)?,
            })
        }
        _ => None,
    }
}

pub fn parse_plot_arguments(arguments: &[OsString]) -> Result<PlotInvocation, PlotUsageError> {
    let mut input = None;
    let mut line = None;
    let mut output = None;
    let mut views = Vec::new();
    let mut parameters = Vec::new();
    let mut size = (DEFAULT_WIDTH, DEFAULT_HEIGHT);
    let mut is_settle = false;
    let mut limit = None;
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
            option if option.starts_with(OPTION_PREFIX) && !is_listed(OPTIONS, option) => {
                return Err(PlotUsageError::UnknownOption(text));
            }
            OUTPUT_OPTION => output = Some(PathBuf::from(value(OUTPUT_OPTION)?)),
            PARAM_OPTION => parameters.push(value(PARAM_OPTION)?),
            SIZE_OPTION => {
                let given = value(SIZE_OPTION)?;
                size = size_of(&given).ok_or(PlotUsageError::InvalidSize(given))?;
            }
            LOCALE_OPTION => {
                let given = value(LOCALE_OPTION)?;
                locale = Some(
                    LanguageTag::parse(&given).map_err(|_| PlotUsageError::InvalidLocale(given))?,
                );
            }
            VIEW_OPTION => {
                let range = value(VIEW_OPTION)?;
                let unit = match remaining.peek().and_then(|next| next.to_str()) {
                    Some(next) if !next.starts_with(OPTION_PREFIX) && input.is_some() => {
                        remaining.next();
                        Some(next.to_owned())
                    }
                    _ => None,
                };
                views.push(ViewOption { range, unit });
            }
            SETTLE_OPTION => is_settle = true,
            LIMIT_OPTION => {
                if limit.is_some() {
                    return Err(PlotUsageError::LimitGivenTwice);
                }
                let given = value(LIMIT_OPTION)?;
                limit = Some(limit_of(&given).ok_or(PlotUsageError::InvalidLimit(given))?);
            }
            JSON_OPTION => is_json = true,
            option if option.starts_with(OPTION_PREFIX) => {
                return Err(PlotUsageError::UnknownOption(text));
            }
            _ if input.is_none() => {
                input = Some(if text.ends_with(SESSION_FILE_EXTENSION) {
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
    Ok(PlotInvocation {
        input,
        line,
        output: output.ok_or(PlotUsageError::NoOutput)?,
        views,
        parameters,
        width: size.0,
        height: size.1,
        is_settle,
        limit,
        is_json,
        locale,
    })
}

pub(crate) fn picture_text(locale: &Locale, defaults: &PictureDefaults) -> PictureText {
    let word = |message: Message| render(&message, locale).to_string();
    let mut text = PictureText {
        axis_titles: defaults
            .axis_titles
            .iter()
            .map(|titles| {
                titles
                    .iter()
                    .map(|title| axis_title_text(title, locale))
                    .collect()
            })
            .collect(),
        legend_titles: vec![defaults.value_name.clone()],
        unit_joiner: word(Message::CommonPictureUnitJoiner),
        sketch: word(Message::CommonPictureSketch),
        ..PictureText::default()
    };
    text.roles.missing = word(Message::CommonPictureMissing);
    text.roles.unresolved = word(Message::CommonPictureUnresolved);
    text.roles.may_be_hit = word(Message::CommonPictureMayBeHit);
    text.roles.marked = word(Message::CommonPictureMarked);
    text.roles.back_face = word(Message::CommonPictureBackFace);
    text.roles.provisional = word(Message::CommonPictureProvisional);
    text.precision.grid_limit = word(Message::CommonPictureGridLimit);
    text.precision.value_limit = word(Message::CommonPictureValueLimit);
    text.precision.unknown_bounds = word(Message::CommonPictureUnknownBounds);
    text.precision.varies_below_bounds = word(Message::CommonPictureBelowBounds);
    text.escape_time.inside = word(Message::CommonPictureInside);
    text.escape_time.undecided = word(Message::CommonPictureUndecided);
    text.domain_legend.argument = word(Message::CommonPictureArgument);
    text.domain_legend.modulus = word(Message::CommonPictureModulus);
    text.mark_keys.angle_arc = word(Message::CommonPictureAngleArc);
    text.mark_keys.right_angle_square = word(Message::CommonPictureRightAngle);
    text.mark_keys.right_angle_arc_with_dot = word(Message::CommonPictureRightAngle);
    text.mark_keys.equal_ticks = word(Message::CommonPictureEqualSides);
    text.mark_keys.direction = word(Message::CommonPictureDirection);
    text.designations.hypotenuse = word(Message::CommonPictureHypotenuse);
    text.designations.leg = word(Message::CommonPictureLeg);
    text.designations.height = word(Message::CommonPictureHeight);
    text
}

pub(crate) fn view_request(view: &calc_app::ViewState) -> ViewRequest {
    if view.camera.is_some() {
        return ViewRequest::of_kind(ViewKind::Space);
    }
    ViewRequest::plane(
        view.axes
            .iter()
            .map(|axis| {
                axis.range.clone().map(|range| AxisRange {
                    range,
                    scale: PICTURE_SCALE,
                })
            })
            .collect(),
    )
}

pub(crate) fn legend_requests(
    legend: Option<ColourLegend>,
    text: &PictureText,
) -> Vec<LegendRequest> {
    legend
        .map(|legend| LegendRequest {
            legend,
            title: text.legend_titles.first().cloned().unwrap_or_default(),
            class_names: [
                text.escape_time.inside.clone(),
                text.escape_time.undecided.clone(),
            ],
            bar_words: [
                text.domain_legend.argument.clone(),
                text.domain_legend.modulus.clone(),
            ],
        })
        .into_iter()
        .collect()
}

pub(crate) fn render_error_message(error: &RenderError, width: u16, height: u16) -> Message {
    match error {
        RenderError::TextDoesNotFit(_) | RenderError::Layout(_) => {
            Message::ErrorRenderTextDoesNotFit {
                width: width.to_string(),
                height: height.to_string(),
            }
        }
        other => Message::ErrorRenderFailed {
            code: format!("{other:?}")
                .split(|character: char| !character.is_alphanumeric())
                .next()
                .unwrap_or_default()
                .to_owned(),
        },
    }
}

pub(crate) struct Failure(pub Box<Message>);

pub(crate) fn failure(message: Message) -> Failure {
    Failure(Box::new(message))
}

pub(crate) fn open_session(
    input: &Input,
    line_name: Option<&String>,
    context: &Context<'_>,
) -> Result<(Session, LineId, ProbeOutcome), Failure> {
    match input {
        Input::Expression(text) => {
            let (mut session, probe) =
                probing_session(Session::new((context.clock)(), registered_backends()));
            let line = session
                .enter(text)
                .map_err(|error| failure(session_error_message(&error, text)))?;
            Ok((session, line, probe))
        }
        Input::SessionFile(path) => {
            let (session, probe) = open_file(path, context).map_err(Failure)?;
            let name = line_name.cloned().unwrap_or_default();
            let line = session
                .resolve(&name)
                .ok_or(failure(Message::ErrorUnknownLabel { line: name }))?;
            Ok((session, line, probe))
        }
    }
}

pub(crate) fn applied_views(
    defaults: &PictureDefaults,
    options: &[ViewOption],
) -> Result<Vec<calc_app::ViewState>, Failure> {
    let mut views = defaults.views.clone();
    let Some(first) = views.first_mut() else {
        return Ok(views);
    };
    for (axis, option) in options.iter().enumerate() {
        let invalid = || {
            failure(Message::CliErrorInvalidView {
                value: option.range.clone(),
            })
        };
        let (lower, upper) = option
            .range
            .split_once(RANGE_SEPARATOR)
            .ok_or_else(invalid)?;
        let range = Interval {
            lower: exact_number(lower).ok_or_else(invalid)?,
            upper: exact_number(upper).ok_or_else(invalid)?,
        };
        let state = AxisState {
            range: Some(range),
            unit: option
                .unit
                .clone()
                .map_or(AxisDisplayUnit::Coherent, AxisDisplayUnit::Unit),
        };
        match first.axes.get_mut(axis) {
            Some(existing) => *existing = state,
            None => first.axes.push(state),
        }
    }
    Ok(views)
}

pub(crate) fn parameters_of(options: &[String]) -> Result<Vec<PictureParameter>, Failure> {
    options
        .iter()
        .map(|option| {
            let invalid = || {
                failure(Message::CliErrorInvalidParameter {
                    value: option.clone(),
                })
            };
            let (name, value) = option.split_once(PARAMETER_SEPARATOR).ok_or_else(invalid)?;
            Ok(PictureParameter {
                name: name.trim().to_owned(),
                value: exact_number(value).ok_or_else(invalid)?,
            })
        })
        .collect()
}

pub(crate) fn sampled_scene(
    session: &mut Session,
    request: &PlotRequest,
) -> Result<Scene, Failure> {
    let (events, received) = channel();
    let mut job = session
        .plot(request, events)
        .map_err(|error| failure(plot_error_message(&error)))?;
    while job.step() == JobState::Pending {}
    let mut bounds = None;
    for event in received.try_iter() {
        match event {
            PictureEvent::Samples { .. } => {}
            PictureEvent::Bounds { scene, .. } => bounds = Some(*scene),
            PictureEvent::Failed { error, .. } => {
                return Err(failure(plot_error_message(&calc_app::PlotError::Sample(
                    error,
                ))));
            }
        }
    }
    bounds.ok_or(failure(Message::CliErrorViewNotFinished))
}

fn value_step(scene: &Scene, view: usize) -> Option<f64> {
    let calc_render::View::View2(plane) = scene.views.get(view)? else {
        return None;
    };
    let width = plane.y.range.upper.sub_exact(&plane.y.range.lower).ok()?;
    Some(width.round_to_f64_ties_even() / f64::from(plane.y.divisions.max(1)))
}

fn column_counts(scene: &Scene) -> (usize, usize, usize) {
    scene
        .frames
        .iter()
        .flat_map(|frame| frame.layers.iter())
        .filter_map(|layer| {
            let columns = layer.columns.as_ref()?;
            let step = value_step(scene, usize::try_from(layer.view.0).ok()?)?;
            Some((columns, step))
        })
        .fold((0, 0, 0), |(marked, wide, total), (columns, step)| {
            (
                marked + columns.marked.iter().filter(|marked| **marked).count(),
                wide + columns.unresolved_at_width(step),
                total + columns.marked.len(),
            )
        })
}

pub(crate) fn precision_notices(scene: &Scene) -> Vec<Message> {
    let mut grid = false;
    let mut value = false;
    let mut unknown = false;
    let mut below = false;
    for limit in scene
        .frames
        .iter()
        .flat_map(|frame| frame.layers.iter())
        .filter_map(|layer| layer.precision.as_ref())
    {
        grid |= !limit.grid_exhausted.is_empty();
        value |= limit.unresolved_samples > 0;
        unknown |= limit.unknown_bounds > 0;
        below |= limit.varies_below_bounds;
    }
    let (marked, wide, columns) = column_counts(scene);
    let mut notices: Vec<Message> = [
        (grid, Message::CommonPictureGridLimit),
        (value && marked == 0, Message::CommonPictureValueLimit),
        (unknown, Message::CommonPictureUnknownBounds),
    ]
    .into_iter()
    .filter(|(applies, _)| *applies)
    .map(|(_, message)| message)
    .collect();
    if wide > 0 {
        notices.push(Message::CommonPictureWidthColumns {
            wide: wide.to_string(),
            columns: columns.to_string(),
        });
    }
    if marked > 0 {
        notices.push(Message::CommonPictureMarkedColumns {
            marked: marked.to_string(),
            columns: columns.to_string(),
        });
    }
    if below && marked == 0 {
        notices.push(Message::CommonPictureBelowBounds);
    }
    notices
}

fn output_not_writable(path: &Path, error: &std::io::Error) -> Message {
    let path = path.display().to_string();
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => Message::CliErrorOutputPermissionDenied { path },
        std::io::ErrorKind::NotFound => Message::CliErrorOutputDirectoryMissing { path },
        std::io::ErrorKind::IsADirectory => Message::CliErrorOutputIsADirectory { path },
        _ => Message::CliErrorOutputNotWritable { path },
    }
}

fn draw(
    invocation: &PlotInvocation,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    probes: &mut Probes,
) -> Result<(), Failure> {
    if let Err(error) = (context.check_output)(&invocation.output) {
        return Err(failure(output_not_writable(&invocation.output, &error)));
    }
    let (mut session, line, probe) =
        open_session(&invocation.input, invocation.line.as_ref(), context)?;
    probes.watch(probe);
    let parameters = if invocation.parameters.is_empty() {
        None
    } else {
        Some(parameters_of(&invocation.parameters)?)
    };
    let defaults = session
        .picture_defaults(line, parameters.as_deref())
        .map_err(|error| failure(plot_error_message(&error)))?;
    let views = applied_views(&defaults, &invocation.views)?;
    let defaults = PictureDefaults {
        axis_titles: session
            .axis_titles(line, parameters.as_deref(), &views)
            .map_err(|error| failure(plot_error_message(&error)))?,
        ..defaults
    };
    let mut text = picture_text(locale, &defaults);
    let fonts = FontSet::bundled().map_err(|_| {
        failure(Message::ErrorRenderFailed {
            code: String::from("fonts"),
        })
    })?;
    let scale = ScaleFactor::from_percent(SCALE_PERCENT).map_err(|_| {
        failure(Message::ErrorRenderFailed {
            code: String::from("scale"),
        })
    })?;
    let layout = plot_layout(
        &LayoutRequest {
            width: invocation.width,
            height: invocation.height,
            text_size: TEXT_SIZE,
            scale,
            views: views.iter().map(view_request).collect(),
            legends: legend_requests(defaults.legend, &text),
        },
        &fonts,
    )
    .map_err(|error| {
        failure(render_error_message(
            &RenderError::Layout(error),
            invocation.width,
            invocation.height,
        ))
    })?;
    let laid_out: Vec<ViewRequest> = views.iter().map(view_request).collect();
    let request = PlotRequest {
        line,
        slot: INVOCATION_SLOT,
        iteration_limit: invocation.limit,
        views: Some(views.clone()),
        divisions: layout.views.iter().map(|view| view.divisions()).collect(),
        parameters,
    };
    let scene = sampled_scene(&mut session, &request)?;
    if column_counts(&scene).1 > 0 {
        text.precision.width_columns =
            render(&Message::CommonPictureWidthMeaning, locale).to_string();
    }
    if invocation.is_settle {
        let shown = settled_views(&views, &scene);
        settled(
            &mut session,
            line,
            &shown,
            &request,
            invocation,
            context,
            locale,
            output,
        )?;
    }
    let rendered = render_scene(
        &RenderRequest {
            scene: &scene,
            settled: Some(&laid_out),
            frame: 0,
            width: invocation.width,
            height: invocation.height,
            text_size: TEXT_SIZE,
            scale,
            theme: Theme::light(),
            decimal_separator: DECIMAL_SEPARATOR,
            text: &text,
        },
        &fonts,
    )
    .map_err(|error| {
        failure(render_error_message(
            &error,
            invocation.width,
            invocation.height,
        ))
    })?;
    let bytes = encode_png(&rendered.image).map_err(|error| {
        failure(Message::ErrorImageNotEncoded {
            code: format!("{error:?}")
                .split(|character: char| !character.is_alphanumeric())
                .next()
                .unwrap_or_default()
                .to_owned(),
        })
    })?;
    let path_text = invocation.output.display().to_string();
    (context.write_file)(&invocation.output, &bytes).map_err(|_| {
        failure(Message::CliErrorFileNotWritten {
            path: path_text.clone(),
        })
    })?;
    if invocation.is_json {
        output
            .json(&plot_json(
                &path_text,
                invocation.width,
                invocation.height,
                &views,
                &scene,
            ))
            .map_err(|_| failure(Message::CliErrorOutputFailed))?;
    } else {
        output
            .line(&render(
                &Message::CliPlotWritten {
                    path: path_text,
                    width: invocation.width.to_string(),
                    height: invocation.height.to_string(),
                },
                locale,
            ))
            .map_err(|_| failure(Message::CliErrorOutputFailed))?;
        for notice in precision_notices(&scene) {
            let notice = render(&notice, locale).to_string();
            output
                .line(&render(&Message::CliPlotNotice { notice }, locale))
                .map_err(|_| failure(Message::CliErrorOutputFailed))?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn settled_views(requested: &[calc_app::ViewState], scene: &Scene) -> Vec<calc_app::ViewState> {
    scene
        .views
        .iter()
        .enumerate()
        .map(|(index, view)| {
            let requested = requested.get(index);
            calc_app::ViewState {
                axes: view
                    .axes()
                    .into_iter()
                    .enumerate()
                    .map(|(axis, shown)| AxisState {
                        range: Some(shown.range.clone()),
                        unit: requested
                            .and_then(|view| view.axes.get(axis))
                            .map_or(AxisDisplayUnit::Coherent, |state| state.unit.clone()),
                    })
                    .collect(),
                camera: requested.and_then(|view| view.camera.clone()),
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn settled(
    session: &mut Session,
    line: LineId,
    views: &[calc_app::ViewState],
    request: &PlotRequest,
    invocation: &PlotInvocation,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
) -> Result<(), Failure> {
    let Input::SessionFile(path) = &invocation.input else {
        return Err(failure(Message::CliErrorSettleNeedsSession));
    };
    session
        .settle_picture(&calc_app::SettleRequest {
            iteration_limit: invocation.limit,
            line,
            views: views.to_vec(),
            parameters: request.parameters.clone().unwrap_or_default(),
        })
        .map_err(|error| failure(plot_error_message(&error)))?;
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

pub fn run_plot(
    arguments: &[OsString],
    context: &Context<'_>,
    standard_output: &mut dyn Write,
    standard_error: &mut dyn Write,
) -> u8 {
    let mut output = Output::new(standard_output);
    let mut errors = Output::new(standard_error);
    let invocation = match parse_plot_arguments(arguments) {
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
    let outcome = draw(&invocation, context, &locale, &mut output, &mut probes);
    probes.report(&locale, &mut errors);
    match outcome {
        Ok(()) => EXIT_SUCCESS,
        Err(Failure(message)) => {
            let detail = render(message.as_ref(), &locale).to_string();
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
    fn expression_with_output_is_a_plot_invocation() {
        let invocation =
            parse_plot_arguments(&arguments(&["x^2", "--output", "square.png"])).unwrap();
        assert_eq!(
            (invocation.input, invocation.output, invocation.width),
            (
                Input::Expression("x^2".to_owned()),
                PathBuf::from("square.png"),
                800
            )
        );
    }

    #[test]
    fn settle_is_read_from_its_option() {
        let invocation = parse_plot_arguments(&arguments(&[
            "s.calc", "r1", "--output", "a.png", "--settle",
        ]))
        .unwrap();

        assert!(invocation.is_settle);
    }

    #[test]
    fn a_fixed_limit_names_its_iterations() {
        let invocation = parse_plot_arguments(&arguments(&[
            "escape_time {\"form\":\"quadratic_parameter\"}",
            "--output",
            "m.png",
            "--limit",
            "fixed:500",
        ]))
        .unwrap();

        assert_eq!(
            invocation.limit,
            Some(IterationLimitRule::Fixed { iterations: 500 })
        );
    }

    #[test]
    fn a_following_limit_names_its_base_step_and_cap() {
        let invocation = parse_plot_arguments(&arguments(&[
            "escape_time {\"form\":\"quadratic_parameter\"}",
            "--output",
            "m.png",
            "--limit",
            "following:256,64,65536",
        ]))
        .unwrap();

        assert_eq!(
            invocation.limit,
            Some(IterationLimitRule::FollowingDepth {
                base: 256,
                per_halving: 64,
                cap: 65_536
            })
        );
    }

    #[test]
    fn a_limit_above_the_largest_iteration_count_is_a_usage_error() {
        let outcome = parse_plot_arguments(&arguments(&[
            "x^2",
            "--output",
            "a.png",
            "--limit",
            "fixed:65537",
        ]))
        .err();

        assert_eq!(
            outcome,
            Some(PlotUsageError::InvalidLimit("fixed:65537".to_owned()))
        );
    }

    #[test]
    fn a_limit_of_zero_iterations_is_a_usage_error() {
        let outcome = parse_plot_arguments(&arguments(&[
            "x^2", "--output", "a.png", "--limit", "fixed:0",
        ]))
        .err();

        assert_eq!(
            outcome,
            Some(PlotUsageError::InvalidLimit("fixed:0".to_owned()))
        );
    }

    #[test]
    fn a_limit_without_a_rule_is_a_usage_error() {
        let outcome =
            parse_plot_arguments(&arguments(&["x^2", "--output", "a.png", "--limit", "500"])).err();

        assert_eq!(
            outcome,
            Some(PlotUsageError::InvalidLimit("500".to_owned()))
        );
    }

    #[test]
    fn a_following_limit_with_two_numbers_is_a_usage_error() {
        let outcome = parse_plot_arguments(&arguments(&[
            "x^2",
            "--output",
            "a.png",
            "--limit",
            "following:256,64",
        ]))
        .err();

        assert_eq!(
            outcome,
            Some(PlotUsageError::InvalidLimit("following:256,64".to_owned()))
        );
    }

    #[test]
    fn a_second_limit_is_a_usage_error() {
        let outcome = parse_plot_arguments(&arguments(&[
            "x^2",
            "--output",
            "a.png",
            "--limit",
            "fixed:100",
            "--limit",
            "fixed:200",
        ]))
        .err();

        assert_eq!(outcome, Some(PlotUsageError::LimitGivenTwice));
    }

    #[test]
    fn view_option_takes_a_range_and_an_optional_unit() {
        let invocation = parse_plot_arguments(&arguments(&[
            "x * 1 m", "--output", "a.png", "--view", "0..1", "--view", "0..100", "cm",
        ]))
        .unwrap();
        assert_eq!(
            invocation.views,
            vec![
                ViewOption {
                    range: "0..1".to_owned(),
                    unit: None
                },
                ViewOption {
                    range: "0..100".to_owned(),
                    unit: Some("cm".to_owned())
                }
            ]
        );
    }

    #[test]
    fn size_option_takes_width_by_height() {
        let invocation =
            parse_plot_arguments(&arguments(&["x", "--output", "a.png", "--size", "320x200"]))
                .unwrap();
        assert_eq!((invocation.width, invocation.height), (320, 200));
    }

    #[test]
    fn malformed_size_is_an_error() {
        assert_eq!(
            parse_plot_arguments(&arguments(&["x", "--output", "a.png", "--size", "320"])),
            Err(PlotUsageError::InvalidSize("320".to_owned()))
        );
    }

    #[test]
    fn plot_without_output_is_an_error() {
        assert_eq!(
            parse_plot_arguments(&arguments(&["x^2"])),
            Err(PlotUsageError::NoOutput)
        );
    }

    #[test]
    fn session_file_plot_needs_its_line() {
        assert_eq!(
            parse_plot_arguments(&arguments(&["lab.calc", "--output", "a.png"])),
            Err(PlotUsageError::NoLine)
        );
    }

    #[test]
    fn session_file_plot_names_its_line() {
        let invocation =
            parse_plot_arguments(&arguments(&["lab.calc", "r2", "--output", "a.png"])).unwrap();
        assert_eq!(invocation.line.as_deref(), Some("r2"));
    }
}
