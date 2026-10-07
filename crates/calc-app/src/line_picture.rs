use calc_numbers::Number;
use calc_units::TemperatureScale;

use calc_viz::IterationLimitRule;

use crate::escape_time_line::{iteration_limit, iteration_limit_json};
use crate::json::Json;
use crate::result_record::LineId;
use crate::session_file::{
    JsonPath, LoadError, array, boolean, count, identifier, line_from_label, line_label, members,
    number, number_json, optional, string,
};

const ORTHOGRAPHIC: &str = "orthographic";
const PERSPECTIVE: &str = "perspective";
const UNIT_MEMBER: &str = "unit";
const SCALE_MEMBER: &str = "scale";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub views: Vec<PictureView>,
    pub parameters: Vec<PictureParameter>,
    pub iteration_limit: Option<IterationLimitRule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureView {
    pub axes: Vec<PictureAxis>,
    pub camera: Option<PictureCamera>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureAxis {
    pub lower: Number,
    pub upper: Number,
    pub unit: AxisDisplayUnit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AxisDisplayUnit {
    Coherent,
    Unit(String),
    TemperatureScale(TemperatureScale),
}

fn axis_unit_json(unit: &AxisDisplayUnit) -> Json {
    match unit {
        AxisDisplayUnit::Coherent => Json::Null,
        AxisDisplayUnit::Unit(text) => Json::object(vec![(UNIT_MEMBER, Json::string(text))]),
        AxisDisplayUnit::TemperatureScale(scale) => {
            Json::object(vec![(SCALE_MEMBER, Json::string(scale.name()))])
        }
    }
}

fn axis_unit(json: &Json, path: &JsonPath) -> Result<AxisDisplayUnit, LoadError> {
    let Some(json) = optional(json) else {
        return Ok(AxisDisplayUnit::Coherent);
    };
    let Json::Object(object) = json else {
        return Err(LoadError::InvalidValue(path.clone()));
    };
    match object.first().map(|(name, _)| name.as_str()) {
        Some(UNIT_MEMBER) => {
            let found = members(json, path, &[UNIT_MEMBER])?;
            let unit_path = path.member(UNIT_MEMBER);
            let text = string(found[0], &unit_path)?;
            if text.is_empty() {
                return Err(LoadError::InvalidValue(unit_path));
            }
            Ok(AxisDisplayUnit::Unit(text.to_owned()))
        }
        _ => {
            let found = members(json, path, &[SCALE_MEMBER])?;
            let scale_path = path.member(SCALE_MEMBER);
            match TemperatureScale::from_name(string(found[0], &scale_path)?) {
                Some(scale @ (TemperatureScale::Celsius | TemperatureScale::Fahrenheit)) => {
                    Ok(AxisDisplayUnit::TemperatureScale(scale))
                }
                Some(TemperatureScale::Kelvin) | None => Err(LoadError::InvalidValue(scale_path)),
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureCamera {
    pub azimuth_degrees: Number,
    pub elevation_degrees: Number,
    pub projection: CameraProjection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CameraProjection {
    Orthographic,
    Perspective { field_of_view_degrees: Number },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureParameter {
    pub name: String,
    pub value: Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineReading {
    pub source: LineId,
    pub view: PictureView,
    pub divisions: Vec<u32>,
    pub at: Vec<Number>,
    pub snapped: bool,
}

fn view_json(view: &PictureView) -> Json {
    let axes = view
        .axes
        .iter()
        .map(|axis| {
            Json::object(vec![
                ("lower", number_json(&axis.lower)),
                ("upper", number_json(&axis.upper)),
                ("unit", axis_unit_json(&axis.unit)),
            ])
        })
        .collect();
    let camera = view.camera.as_ref().map(|camera| {
        let projection = match &camera.projection {
            CameraProjection::Orthographic => Json::string(ORTHOGRAPHIC),
            CameraProjection::Perspective {
                field_of_view_degrees,
            } => Json::object(vec![(PERSPECTIVE, number_json(field_of_view_degrees))]),
        };
        Json::object(vec![
            ("azimuth_degrees", number_json(&camera.azimuth_degrees)),
            ("elevation_degrees", number_json(&camera.elevation_degrees)),
            ("projection", projection),
        ])
    });
    Json::object(vec![
        ("axes", Json::Array(axes)),
        ("camera", Json::optional(camera)),
    ])
}

pub(crate) fn picture_json(picture: Option<&Picture>) -> Json {
    Json::optional(picture.map(|picture| {
        Json::object(vec![
            (
                "views",
                Json::Array(picture.views.iter().map(view_json).collect()),
            ),
            (
                "parameters",
                Json::Array(
                    picture
                        .parameters
                        .iter()
                        .map(|parameter| {
                            Json::object(vec![
                                ("name", Json::string(&parameter.name)),
                                ("value", number_json(&parameter.value)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "iteration_limit",
                iteration_limit_json(picture.iteration_limit),
            ),
        ])
    }))
}

pub(crate) fn reading_json(reading: Option<&LineReading>) -> Json {
    Json::optional(reading.map(|reading| {
        Json::object(vec![
            ("source", Json::String(line_label(reading.source))),
            ("view", view_json(&reading.view)),
            (
                "divisions",
                Json::Array(
                    reading
                        .divisions
                        .iter()
                        .map(|count| Json::Count(u64::from(*count)))
                        .collect(),
                ),
            ),
            (
                "at",
                Json::Array(reading.at.iter().map(number_json).collect()),
            ),
            ("snapped", Json::Boolean(reading.snapped)),
        ])
    }))
}

fn is_below(lower: &Number, upper: &Number) -> bool {
    match lower.sub_exact(upper) {
        Ok(Number::Integer(difference)) => difference.is_negative(),
        Ok(Number::Rational(difference)) => difference.numerator().is_negative(),
        Ok(Number::F32(_) | Number::F64(_)) | Err(_) => false,
    }
}

fn exact_number(json: &Json, path: &JsonPath) -> Result<Number, LoadError> {
    match number(json, path)? {
        exact @ (Number::Integer(_) | Number::Rational(_)) => Ok(exact),
        Number::F32(_) | Number::F64(_) => Err(LoadError::InvalidValue(path.clone())),
    }
}

fn view(json: &Json, path: &JsonPath) -> Result<PictureView, LoadError> {
    let parts = members(json, path, &["axes", "camera"])?;
    let axes_path = path.member("axes");
    let axes = array(parts[0], &axes_path)?
        .iter()
        .enumerate()
        .map(|(position, axis)| {
            let axis_path = axes_path.index(position);
            let bounds = members(axis, &axis_path, &["lower", "upper", "unit"])?;
            let lower = exact_number(bounds[0], &axis_path.member("lower"))?;
            let upper_path = axis_path.member("upper");
            let upper = exact_number(bounds[1], &upper_path)?;
            if !is_below(&lower, &upper) {
                return Err(LoadError::InvalidValue(upper_path));
            }
            Ok(PictureAxis {
                lower,
                upper,
                unit: axis_unit(bounds[2], &axis_path.member("unit"))?,
            })
        })
        .collect::<Result<Vec<_>, LoadError>>()?;
    let camera_path = path.member("camera");
    let camera = optional(parts[1])
        .map(|json| {
            let found = members(
                json,
                &camera_path,
                &["azimuth_degrees", "elevation_degrees", "projection"],
            )?;
            let projection_path = camera_path.member("projection");
            let projection = match found[2] {
                Json::String(name) if name == ORTHOGRAPHIC => CameraProjection::Orthographic,
                Json::Object(_) => {
                    let perspective = members(found[2], &projection_path, &[PERSPECTIVE])?;
                    CameraProjection::Perspective {
                        field_of_view_degrees: exact_number(
                            perspective[0],
                            &projection_path.member(PERSPECTIVE),
                        )?,
                    }
                }
                _ => return Err(LoadError::InvalidValue(projection_path)),
            };
            Ok(PictureCamera {
                azimuth_degrees: exact_number(found[0], &camera_path.member("azimuth_degrees"))?,
                elevation_degrees: exact_number(
                    found[1],
                    &camera_path.member("elevation_degrees"),
                )?,
                projection,
            })
        })
        .transpose()?;
    Ok(PictureView { axes, camera })
}

pub(crate) fn picture(
    json: &Json,
    path: &JsonPath,
    is_escape_time: bool,
) -> Result<Option<Picture>, LoadError> {
    optional(json)
        .map(|json| {
            let parts = members(json, path, &["views", "parameters", "iteration_limit"])?;
            let views_path = path.member("views");
            let views = array(parts[0], &views_path)?
                .iter()
                .enumerate()
                .map(|(position, element)| view(element, &views_path.index(position)))
                .collect::<Result<Vec<_>, _>>()?;
            let parameters_path = path.member("parameters");
            let mut parameters: Vec<PictureParameter> = Vec::new();
            for (position, element) in array(parts[1], &parameters_path)?.iter().enumerate() {
                let parameter_path = parameters_path.index(position);
                let found = members(element, &parameter_path, &["name", "value"])?;
                let name_path = parameter_path.member("name");
                let name = identifier(found[0], &name_path)?.to_owned();
                if parameters.iter().any(|parameter| parameter.name == name) {
                    return Err(LoadError::InvalidValue(name_path));
                }
                parameters.push(PictureParameter {
                    name,
                    value: exact_number(found[1], &parameter_path.member("value"))?,
                });
            }
            let limit_path = path.member("iteration_limit");
            let limit = iteration_limit(parts[2], &limit_path)?;
            if limit.is_some() && !is_escape_time {
                return Err(LoadError::InvalidValue(limit_path));
            }
            Ok(Picture {
                views,
                parameters,
                iteration_limit: limit,
            })
        })
        .transpose()
}

pub(crate) fn reading(json: &Json, path: &JsonPath) -> Result<Option<LineReading>, LoadError> {
    optional(json)
        .map(|json| {
            let parts = members(
                json,
                path,
                &["source", "view", "divisions", "at", "snapped"],
            )?;
            let source_path = path.member("source");
            let source = line_from_label(string(parts[0], &source_path)?)
                .ok_or_else(|| LoadError::InvalidValue(source_path.clone()))?;
            let reading_view = view(parts[1], &path.member("view"))?;
            let divisions_path = path.member("divisions");
            let divisions = array(parts[2], &divisions_path)?
                .iter()
                .enumerate()
                .map(|(position, element)| {
                    let division_path = divisions_path.index(position);
                    count(element, &division_path).and_then(|divisions| {
                        u32::try_from(divisions)
                            .ok()
                            .filter(|divisions| *divisions > 0)
                            .ok_or(LoadError::InvalidValue(division_path))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if divisions.len() != reading_view.axes.len() {
                return Err(LoadError::InvalidValue(divisions_path));
            }
            let at_path = path.member("at");
            let at = array(parts[3], &at_path)?
                .iter()
                .enumerate()
                .map(|(position, element)| exact_number(element, &at_path.index(position)))
                .collect::<Result<Vec<_>, _>>()?;
            if at.len() != reading_view.axes.len() {
                return Err(LoadError::InvalidValue(at_path));
            }
            if let Some(position) =
                at.iter()
                    .zip(&reading_view.axes)
                    .position(|(coordinate, axis)| {
                        is_below(coordinate, &axis.lower) || is_below(&axis.upper, coordinate)
                    })
            {
                return Err(LoadError::InvalidValue(at_path.index(position)));
            }
            Ok(LineReading {
                source,
                view: reading_view,
                divisions,
                at,
                snapped: boolean(parts[4], &path.member("snapped"))?,
            })
        })
        .transpose()
}
