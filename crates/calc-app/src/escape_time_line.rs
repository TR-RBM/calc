use calc_numbers::Number;
use calc_viz::{EscapeTimeForm, IterationLimitRule};

use crate::json::{self, Json};
use crate::session_file::{JsonPath, LoadError, count, members, number, number_json, optional};
use crate::solve_request::{RequestError, RequestErrorCode};

pub(crate) const ESCAPE_TIME_PREFIX: &str = "escape_time ";
const REQUEST_OPENING: char = '{';
const REQUEST_MEMBERS: [&str; 2] = ["form", "c"];
pub(crate) const READING_PREFIX: &str = "escape_time_reading ";
const READING_MEMBERS: [&str; 4] = ["form", "c", "at", "limit"];
const COMPLEX_MEMBERS: [&str; 2] = ["real", "imaginary"];
const PARAMETER_FORM: &str = "quadratic_parameter";
const INITIAL_FORM: &str = "quadratic_initial";
const RULE_MEMBER: &str = "rule";
const FIXED_RULE: &str = "fixed";
const FOLLOWING_DEPTH_RULE: &str = "following_depth";
const FIXED_MEMBERS: [&str; 2] = [RULE_MEMBER, "iterations"];
const FOLLOWING_DEPTH_MEMBERS: [&str; 4] = [RULE_MEMBER, "base", "per_halving", "cap"];
const LARGEST_ITERATIONS: u32 = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EscapeTimeLine {
    pub form: EscapeTimeForm,
}

impl EscapeTimeLine {
    pub fn line_text(&self) -> String {
        format!(
            "{ESCAPE_TIME_PREFIX}{}",
            json::write_one_line(&self.to_json())
        )
    }

    pub fn axis_name(&self) -> &'static str {
        match self.form {
            EscapeTimeForm::QuadraticParameter => "c",
            EscapeTimeForm::QuadraticInitial { .. } => "z",
        }
    }

    fn to_json(&self) -> Json {
        match &self.form {
            EscapeTimeForm::QuadraticParameter => {
                Json::object(vec![("form", Json::string(PARAMETER_FORM))])
            }
            EscapeTimeForm::QuadraticInitial {
                c_real,
                c_imaginary,
            } => Json::object(vec![
                ("form", Json::string(INITIAL_FORM)),
                (
                    "c",
                    Json::object(vec![
                        ("real", number_json(c_real)),
                        ("imaginary", number_json(c_imaginary)),
                    ]),
                ),
            ]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EscapeTimeReadingLine {
    pub form: EscapeTimeForm,
    pub at_real: Number,
    pub at_imaginary: Number,
    pub limit: u32,
}

impl EscapeTimeReadingLine {
    pub fn line_text(&self) -> String {
        let mut members = match &self.form {
            EscapeTimeForm::QuadraticParameter => vec![("form", Json::string(PARAMETER_FORM))],
            EscapeTimeForm::QuadraticInitial {
                c_real,
                c_imaginary,
            } => vec![
                ("form", Json::string(INITIAL_FORM)),
                (
                    "c",
                    Json::object(vec![
                        ("real", number_json(c_real)),
                        ("imaginary", number_json(c_imaginary)),
                    ]),
                ),
            ],
        };
        members.push((
            "at",
            Json::object(vec![
                ("real", number_json(&self.at_real)),
                ("imaginary", number_json(&self.at_imaginary)),
            ]),
        ));
        members.push(("limit", Json::Count(u64::from(self.limit))));
        format!(
            "{READING_PREFIX}{}",
            json::write_one_line(&Json::object(members))
        )
    }
}

fn reading_members<'json>(
    json: &'json Json,
    path: &JsonPath,
) -> Result<Vec<Option<&'json Json>>, RequestError> {
    let Json::Object(object) = json else {
        return Err(malformed(path));
    };
    let mut found: Vec<Option<&Json>> = vec![None; READING_MEMBERS.len()];
    for (name, member) in object {
        let position = READING_MEMBERS
            .iter()
            .position(|allowed| allowed == name)
            .ok_or_else(|| malformed(&path.member(name)))?;
        if found[position].replace(member).is_some() {
            return Err(malformed(&path.member(name)));
        }
    }
    Ok(found)
}

pub fn parse_escape_time_reading_line(bytes: &[u8]) -> Result<EscapeTimeReadingLine, RequestError> {
    let root = JsonPath::default();
    let document = json::parse(bytes).map_err(|_| malformed(&root))?;
    let found = reading_members(&document, &root)?;
    let present = |position: usize| found[position].filter(|member| **member != Json::Null);
    let form_path = root.member("form");
    let centre_path = root.member("c");
    let name = match present(0) {
        Some(Json::String(name)) => name.as_str(),
        _ => return Err(malformed(&form_path)),
    };
    let form = match (name, present(1)) {
        (PARAMETER_FORM, None) => EscapeTimeForm::QuadraticParameter,
        (INITIAL_FORM, Some(json)) => {
            let (c_real, c_imaginary) = centre(json, &centre_path)?;
            EscapeTimeForm::QuadraticInitial {
                c_real,
                c_imaginary,
            }
        }
        (PARAMETER_FORM | INITIAL_FORM, _) => return Err(malformed(&centre_path)),
        _ => return Err(malformed(&form_path)),
    };
    let at_path = root.member("at");
    let (at_real, at_imaginary) = centre(present(2).ok_or_else(|| malformed(&at_path))?, &at_path)?;
    let limit_path = root.member("limit");
    let limit = match present(3) {
        Some(Json::Count(limit)) => u32::try_from(*limit)
            .ok()
            .filter(|limit| (1..=LARGEST_ITERATIONS).contains(limit))
            .ok_or_else(|| invalid(&limit_path))?,
        _ => return Err(malformed(&limit_path)),
    };
    Ok(EscapeTimeReadingLine {
        form,
        at_real,
        at_imaginary,
        limit,
    })
}

pub(crate) fn escape_time_reading_from_line_text(
    text: &str,
) -> Option<Result<EscapeTimeReadingLine, RequestError>> {
    text.strip_prefix(READING_PREFIX)
        .filter(|request| request.trim_start().starts_with(REQUEST_OPENING))
        .map(|request| parse_escape_time_reading_line(request.as_bytes()))
}

fn malformed(path: &JsonPath) -> RequestError {
    RequestError::new(RequestErrorCode::MalformedRequest, path.clone())
}

fn invalid(path: &JsonPath) -> RequestError {
    RequestError::new(RequestErrorCode::InvalidValue, path.clone())
}

fn request_members<'json>(
    json: &'json Json,
    path: &JsonPath,
) -> Result<Vec<Option<&'json Json>>, RequestError> {
    let Json::Object(object) = json else {
        return Err(malformed(path));
    };
    let mut found: Vec<Option<&Json>> = vec![None; REQUEST_MEMBERS.len()];
    for (name, member) in object {
        let position = REQUEST_MEMBERS
            .iter()
            .position(|allowed| allowed == name)
            .ok_or_else(|| malformed(&path.member(name)))?;
        if found[position].replace(member).is_some() {
            return Err(malformed(&path.member(name)));
        }
    }
    Ok(found)
}

fn exact(json: &Json, path: &JsonPath) -> Result<Number, RequestError> {
    match number(json, path) {
        Ok(exact @ (Number::Integer(_) | Number::Rational(_))) => Ok(exact),
        Ok(Number::F32(_) | Number::F64(_)) | Err(_) => Err(invalid(path)),
    }
}

fn centre(json: &Json, path: &JsonPath) -> Result<(Number, Number), RequestError> {
    let Json::Object(object) = json else {
        return Err(malformed(path));
    };
    let mut found: Vec<Option<&Json>> = vec![None; COMPLEX_MEMBERS.len()];
    for (name, member) in object {
        let position = COMPLEX_MEMBERS
            .iter()
            .position(|allowed| allowed == name)
            .ok_or_else(|| malformed(&path.member(name)))?;
        if found[position].replace(member).is_some() {
            return Err(malformed(&path.member(name)));
        }
    }
    let present = |position: usize| {
        found[position]
            .filter(|member| **member != Json::Null)
            .ok_or_else(|| malformed(&path.member(COMPLEX_MEMBERS[position])))
    };
    Ok((
        exact(present(0)?, &path.member("real"))?,
        exact(present(1)?, &path.member("imaginary"))?,
    ))
}

pub fn parse_escape_time_line(bytes: &[u8]) -> Result<EscapeTimeLine, RequestError> {
    let root = JsonPath::default();
    let document = json::parse(bytes).map_err(|_| malformed(&root))?;
    let found = request_members(&document, &root)?;
    let form_path = root.member("form");
    let centre_path = root.member("c");
    let name = match found[0].filter(|member| **member != Json::Null) {
        Some(Json::String(name)) => name.as_str(),
        _ => return Err(malformed(&form_path)),
    };
    let centre_json = found[1].filter(|member| **member != Json::Null);
    let form = match (name, centre_json) {
        (PARAMETER_FORM, None) => EscapeTimeForm::QuadraticParameter,
        (INITIAL_FORM, Some(json)) => {
            let (c_real, c_imaginary) = centre(json, &centre_path)?;
            EscapeTimeForm::QuadraticInitial {
                c_real,
                c_imaginary,
            }
        }
        (PARAMETER_FORM | INITIAL_FORM, _) => return Err(malformed(&centre_path)),
        _ => return Err(malformed(&form_path)),
    };
    Ok(EscapeTimeLine { form })
}

pub(crate) fn escape_time_from_line_text(
    text: &str,
) -> Option<Result<EscapeTimeLine, RequestError>> {
    text.strip_prefix(ESCAPE_TIME_PREFIX)
        .filter(|request| request.trim_start().starts_with(REQUEST_OPENING))
        .map(|request| parse_escape_time_line(request.as_bytes()))
}

pub(crate) fn iteration_limit_json(rule: Option<IterationLimitRule>) -> Json {
    Json::optional(rule.map(|rule| match rule {
        IterationLimitRule::Fixed { iterations } => Json::object(vec![
            (RULE_MEMBER, Json::string(FIXED_RULE)),
            ("iterations", Json::Count(u64::from(iterations))),
        ]),
        IterationLimitRule::FollowingDepth {
            base,
            per_halving,
            cap,
        } => Json::object(vec![
            (RULE_MEMBER, Json::string(FOLLOWING_DEPTH_RULE)),
            ("base", Json::Count(u64::from(base))),
            ("per_halving", Json::Count(u64::from(per_halving))),
            ("cap", Json::Count(u64::from(cap))),
        ]),
    }))
}

fn iterations(json: &Json, path: &JsonPath, largest: u32) -> Result<u32, LoadError> {
    u32::try_from(count(json, path)?)
        .ok()
        .filter(|value| (1..=largest).contains(value))
        .ok_or_else(|| LoadError::InvalidValue(path.clone()))
}

pub(crate) fn iteration_limit(
    json: &Json,
    path: &JsonPath,
) -> Result<Option<IterationLimitRule>, LoadError> {
    let Some(json) = optional(json) else {
        return Ok(None);
    };
    let rule_path = path.member(RULE_MEMBER);
    let Json::Object(object) = json else {
        return Err(LoadError::InvalidValue(path.clone()));
    };
    let named = object
        .iter()
        .find(|(name, _)| name == RULE_MEMBER)
        .map(|(_, member)| member)
        .ok_or_else(|| LoadError::MissingMember(rule_path.clone()))?;
    match named {
        Json::String(name) if name == FIXED_RULE => {
            let found = members(json, path, &FIXED_MEMBERS)?;
            Ok(Some(IterationLimitRule::Fixed {
                iterations: iterations(found[1], &path.member("iterations"), LARGEST_ITERATIONS)?,
            }))
        }
        Json::String(name) if name == FOLLOWING_DEPTH_RULE => {
            let found = members(json, path, &FOLLOWING_DEPTH_MEMBERS)?;
            Ok(Some(IterationLimitRule::FollowingDepth {
                base: iterations(found[1], &path.member("base"), u32::MAX)?,
                per_halving: iterations(found[2], &path.member("per_halving"), u32::MAX)?,
                cap: iterations(found[3], &path.member("cap"), LARGEST_ITERATIONS)?,
            }))
        }
        _ => Err(LoadError::InvalidValue(rule_path)),
    }
}

pub(crate) fn is_escape_time_text(text: &str) -> bool {
    escape_time_from_line_text(text).is_some()
}

#[cfg(test)]
mod tests {
    use calc_numbers::Integer;

    use super::*;
    use crate::session_file::PathSegment;

    fn path(segments: &[&str]) -> JsonPath {
        JsonPath(
            segments
                .iter()
                .map(|segment| PathSegment::Member((*segment).to_owned()))
                .collect(),
        )
    }

    fn error(text: &str) -> RequestError {
        parse_escape_time_line(text.as_bytes()).unwrap_err()
    }

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    #[test]
    fn parameter_form_needs_no_further_member() {
        assert_eq!(
            parse_escape_time_line(br#"{"form": "quadratic_parameter"}"#),
            Ok(EscapeTimeLine {
                form: EscapeTimeForm::QuadraticParameter
            })
        );
    }

    #[test]
    fn initial_form_takes_its_centre_as_exact_values() {
        assert_eq!(
            parse_escape_time_line(
                br#"{"form": "quadratic_initial", "c": {"real": {"type": "rational", "numerator": "-4", "denominator": "5"}, "imaginary": {"type": "integer", "digits": "0"}}}"#
            ),
            Ok(EscapeTimeLine {
                form: EscapeTimeForm::QuadraticInitial {
                    c_real: fraction(-4, 5),
                    c_imaginary: Number::from(0_i64),
                }
            })
        );
    }

    #[test]
    fn parameter_form_with_a_centre_is_malformed_at_that_member() {
        assert_eq!(
            error(
                r#"{"form": "quadratic_parameter", "c": {"real": {"type": "integer", "digits": "0"}, "imaginary": {"type": "integer", "digits": "0"}}}"#
            ),
            RequestError::new(RequestErrorCode::MalformedRequest, path(&["c"]))
        );
    }

    #[test]
    fn initial_form_without_a_centre_is_malformed_at_that_member() {
        assert_eq!(
            error(r#"{"form": "quadratic_initial"}"#),
            RequestError::new(RequestErrorCode::MalformedRequest, path(&["c"]))
        );
    }

    #[test]
    fn unknown_form_is_malformed_at_the_form_member() {
        assert_eq!(
            error(r#"{"form": "cubic_parameter"}"#),
            RequestError::new(RequestErrorCode::MalformedRequest, path(&["form"]))
        );
    }

    #[test]
    fn member_beyond_the_request_is_malformed_at_its_name() {
        assert_eq!(
            error(r#"{"form": "quadratic_parameter", "limit": 32}"#),
            RequestError::new(RequestErrorCode::MalformedRequest, path(&["limit"]))
        );
    }

    #[test]
    fn machine_value_in_the_centre_is_an_invalid_value() {
        assert_eq!(
            error(
                r#"{"form": "quadratic_initial", "c": {"real": {"type": "f64", "bits": "0"}, "imaginary": {"type": "integer", "digits": "0"}}}"#
            ),
            RequestError::new(RequestErrorCode::InvalidValue, path(&["c", "real"]))
        );
    }

    #[test]
    fn line_text_is_read_back_as_the_same_request() {
        let line = EscapeTimeLine {
            form: EscapeTimeForm::QuadraticInitial {
                c_real: fraction(-4, 5),
                c_imaginary: fraction(39, 250),
            },
        };

        assert_eq!(
            escape_time_from_line_text(&line.line_text()),
            Some(Ok(line))
        );
    }

    #[test]
    fn reading_line_text_is_read_back_as_the_same_request() {
        let reading = EscapeTimeReadingLine {
            form: EscapeTimeForm::QuadraticParameter,
            at_real: fraction(-3, 4),
            at_imaginary: fraction(1, 4),
            limit: 256,
        };

        assert_eq!(
            escape_time_reading_from_line_text(&reading.line_text()),
            Some(Ok(reading))
        );
    }

    #[test]
    fn reading_line_of_the_initial_form_carries_its_centre_before_the_point() {
        let reading = EscapeTimeReadingLine {
            form: EscapeTimeForm::QuadraticInitial {
                c_real: fraction(-4, 5),
                c_imaginary: fraction(39, 250),
            },
            at_real: Number::from(0_i64),
            at_imaginary: Number::from(0_i64),
            limit: 32,
        };

        let text = reading.line_text();

        assert!(
            text.starts_with(r#"escape_time_reading {"form": "quadratic_initial", "c": {"real": "#),
            "{text}"
        );
    }

    #[test]
    fn reading_line_without_a_point_is_malformed_at_that_member() {
        assert_eq!(
            parse_escape_time_reading_line(br#"{"form": "quadratic_parameter", "limit": 32}"#)
                .unwrap_err(),
            RequestError::new(RequestErrorCode::MalformedRequest, path(&["at"]))
        );
    }

    #[test]
    fn reading_line_with_a_limit_beyond_the_largest_is_an_invalid_value() {
        assert_eq!(
            parse_escape_time_reading_line(
                br#"{"form": "quadratic_parameter", "at": {"real": {"type": "integer", "digits": "0"}, "imaginary": {"type": "integer", "digits": "0"}}, "limit": 65537}"#
            )
            .unwrap_err(),
            RequestError::new(RequestErrorCode::InvalidValue, path(&["limit"]))
        );
    }

    #[test]
    fn a_picture_line_is_not_read_as_a_reading_line() {
        assert_eq!(
            escape_time_reading_from_line_text(r#"escape_time {"form": "quadratic_parameter"}"#),
            None
        );
    }

    #[test]
    fn text_without_the_keyword_is_not_an_escape_time_line() {
        assert_eq!(escape_time_from_line_text("x^2"), None);
    }
}
