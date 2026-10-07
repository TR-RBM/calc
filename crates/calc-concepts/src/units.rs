use std::collections::BTreeMap;

use calc_expr::{ExprPool, NodeView};
use calc_numbers::Number;
use calc_units::{Dimension, ScaleFactor, TemperatureScale, UnitId};

use crate::document::{Document, FieldReader, Section};
use crate::error::{LoadError, LoadErrorKind};
use crate::model::{DisplayUnit, QuantityKind, UnitDeclaration, UnitSystem};

const DIMENSIONLESS_TEXT: &str = "1";
const COMPOUND_SEPARATOR: char = ' ';
const LIST_SEPARATOR: &str = ", ";
const STAGE_SEPARATOR: char = ' ';
const UNIT_SECTION: &str = "Unit";
const ENGLISH: &str = "en";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayUnitError {
    Invalid(String),
    OtherDimension(String),
    CompoundNotAllowed,
    CompoundNotDescending(String),
}

struct ResolvedUnit {
    dimension: Dimension,
    scale: ScaleFactor,
}

fn resolve_unit(pool: &mut ExprPool, text: &str) -> Option<ResolvedUnit> {
    if text == DIMENSIONLESS_TEXT {
        return Some(ResolvedUnit {
            dimension: Dimension::DIMENSIONLESS,
            scale: ScaleFactor::one(),
        });
    }
    if text.is_empty() || text.contains(char::is_whitespace) {
        return None;
    }
    let one = pool.number(Number::from(1)).ok()?;
    let expression = calc_syntax::parse_expression(pool, &format!("1 {text}")).ok()?;
    let unit: UnitId = match pool.node(expression).ok()? {
        NodeView::Quantity { value, unit } if value == one => unit,
        _ if expression == one => {
            return Some(ResolvedUnit {
                dimension: Dimension::DIMENSIONLESS,
                scale: ScaleFactor::one(),
            });
        }
        _ => return None,
    };
    let units = pool.units();
    Some(ResolvedUnit {
        dimension: units.dimension(unit).ok()?,
        scale: units.scale_factor(unit).ok()?.clone(),
    })
}

fn is_larger(left: &ScaleFactor, right: &ScaleFactor) -> bool {
    left.pi_exponent() == right.pi_exponent()
        && left.numerator() * right.denominator() > right.numerator() * left.denominator()
}

pub fn check_display_unit(
    pool: &mut ExprPool,
    kind: QuantityKind,
    text: &str,
) -> Result<DisplayUnit, DisplayUnitError> {
    read_display_unit(pool, kind, text, true)
}

pub(crate) fn read_display_unit(
    pool: &mut ExprPool,
    kind: QuantityKind,
    text: &str,
    compound_allowed: bool,
) -> Result<DisplayUnit, DisplayUnitError> {
    if kind == QuantityKind::Temperature {
        return TemperatureScale::from_name(text)
            .map(DisplayUnit::Scale)
            .ok_or_else(|| DisplayUnitError::Invalid(text.to_string()));
    }
    let parts: Vec<&str> = text.split(COMPOUND_SEPARATOR).collect();
    if parts.len() > 1 && !(compound_allowed && kind.allows_compound()) {
        return Err(DisplayUnitError::CompoundNotAllowed);
    }
    let mut previous: Option<ScaleFactor> = None;
    for part in &parts {
        let resolved = resolve_unit(pool, part)
            .ok_or_else(|| DisplayUnitError::Invalid((*part).to_string()))?;
        if resolved.dimension != kind.dimension() {
            return Err(DisplayUnitError::OtherDimension((*part).to_string()));
        }
        if let Some(larger) = &previous
            && !is_larger(larger, &resolved.scale)
        {
            return Err(DisplayUnitError::CompoundNotDescending(text.to_string()));
        }
        previous = Some(resolved.scale);
    }
    Ok(DisplayUnit::Units(
        parts.iter().map(|part| (*part).to_string()).collect(),
    ))
}

fn scale_of(pool: &mut ExprPool, unit: &DisplayUnit) -> Option<ScaleFactor> {
    match unit {
        DisplayUnit::Units(parts) => match parts.as_slice() {
            [single] => resolve_unit(pool, single).map(|resolved| resolved.scale),
            _ => None,
        },
        DisplayUnit::Scale(_) => None,
    }
}

pub(crate) fn read_displayed_list(
    pool: &mut ExprPool,
    kind: QuantityKind,
    text: &str,
) -> Result<Vec<DisplayUnit>, LoadErrorKind> {
    let mut units = Vec::new();
    for entry in text.split(LIST_SEPARATOR) {
        let unit =
            read_display_unit(pool, kind, entry, true).map_err(|error| error_kind(kind, error))?;
        units.push(unit);
    }
    if units.len() == 1 {
        return Ok(units);
    }
    if units
        .iter()
        .any(|unit| matches!(unit, DisplayUnit::Scale(_)))
    {
        return Err(LoadErrorKind::ScaleNotAlone(text.to_string()));
    }
    let mut scales = Vec::new();
    for unit in &units {
        scales.push(
            scale_of(pool, unit)
                .ok_or_else(|| LoadErrorKind::CompoundNotAlone(text.to_string()))?,
        );
    }
    for pair in scales.windows(2) {
        let [smaller, larger] = pair else {
            continue;
        };
        if larger.pi_exponent() != smaller.pi_exponent() {
            return Err(LoadErrorKind::DisplayedListMixesPi(text.to_string()));
        }
        if !is_larger(larger, smaller) {
            return Err(LoadErrorKind::DisplayedListNotAscending(text.to_string()));
        }
    }
    Ok(units)
}

fn error_kind(kind: QuantityKind, error: DisplayUnitError) -> LoadErrorKind {
    let kind_name = kind.name().to_string();
    match error {
        DisplayUnitError::Invalid(text) => LoadErrorKind::InvalidDisplayUnit(text),
        DisplayUnitError::OtherDimension(unit) => LoadErrorKind::DisplayUnitOfOtherDimension {
            kind: kind_name,
            unit,
        },
        DisplayUnitError::CompoundNotAllowed => LoadErrorKind::CompoundNotAllowed(kind_name),
        DisplayUnitError::CompoundNotDescending(text) => LoadErrorKind::CompoundNotDescending(text),
    }
}

fn unit_error(
    document: &Document,
    line: usize,
    kind: QuantityKind,
    error: DisplayUnitError,
) -> LoadError {
    document.error(line, error_kind(kind, error))
}

pub(crate) fn read_unit_system(
    identifier: &str,
    document: &Document,
    texts: &[(String, Document)],
    pool: &mut ExprPool,
) -> Result<UnitSystem, LoadError> {
    crate::load::check_title(document, identifier)?;
    FieldReader::new(document, &document.fields, 1).finish()?;
    let mut units = Vec::new();
    for section in &document.sections {
        if section.kind != UNIT_SECTION {
            return Err(document.error(
                section.line,
                LoadErrorKind::UnknownSection(section.kind.clone()),
            ));
        }
        units.push(read_unit_section(document, section, false, pool)?);
    }
    let mut names = BTreeMap::new();
    for (locale, text) in texts {
        FieldReader::new(text, &text.fields, 1).finish()?;
        if let Some(section) = text.sections.first() {
            return Err(text.error(
                section.line,
                LoadErrorKind::UnknownSection(section.kind.clone()),
            ));
        }
        names.insert(locale.clone(), text.title.clone());
    }
    if !names.contains_key(ENGLISH) {
        return Err(document.error(1, LoadErrorKind::MissingEnglishText(identifier.to_string())));
    }
    Ok(UnitSystem {
        identifier: identifier.to_string(),
        names,
        units,
    })
}

fn stage<'text>(
    declaration: &'text UnitDeclaration,
    first_group: Option<&'text str>,
) -> Option<&'text str> {
    declaration.from.as_deref().or(first_group)
}

pub(crate) fn check_unique_stages(
    document: &Document,
    first_group: Option<&str>,
    sections: &[(&Section, &UnitDeclaration)],
) -> Result<(), LoadError> {
    for (position, (section, declaration)) in sections.iter().enumerate() {
        let repeated = sections[..position].iter().any(|(_, earlier)| {
            earlier.kind == declaration.kind
                && stage(earlier, first_group) == stage(declaration, first_group)
        });
        if repeated {
            return Err(document.error(
                section.line,
                LoadErrorKind::DuplicateUnitSection(section.name.clone()),
            ));
        }
    }
    Ok(())
}

pub(crate) fn read_unit_section(
    document: &Document,
    section: &Section,
    in_curriculum: bool,
    pool: &mut ExprPool,
) -> Result<UnitDeclaration, LoadError> {
    let (kind_text, from) = match section.name.split_once(STAGE_SEPARATOR) {
        Some((kind_text, group)) => (kind_text, Some(group.to_string())),
        None => (section.name.as_str(), None),
    };
    let kind = QuantityKind::from_name(kind_text).ok_or_else(|| {
        document.error(
            section.line,
            LoadErrorKind::UnknownQuantityKind(kind_text.to_string()),
        )
    })?;
    if let (false, Some(group)) = (in_curriculum, &from) {
        return Err(document.error(
            section.line,
            LoadErrorKind::UnitStageOutsideCurriculum(group.clone()),
        ));
    }
    crate::load::no_blocks(document, section, 0)?;
    let mut reader = FieldReader::new(document, &section.fields, section.line);
    let displayed_field = reader.required_value("Displayed")?;
    let posed_field = if in_curriculum {
        reader.optional("Posed")
    } else {
        None
    };
    reader.finish()?;
    let displayed = read_displayed_list(pool, kind, &displayed_field.value)
        .map_err(|error| document.error(displayed_field.line, error))?;
    let posed = posed_field
        .map(|field| {
            read_display_unit(pool, kind, &field.value, false)
                .map_err(|error| unit_error(document, field.line, kind, error))
        })
        .transpose()?;
    Ok(UnitDeclaration {
        kind,
        from,
        posed,
        displayed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(kind: QuantityKind, text: &str) -> Result<DisplayUnit, DisplayUnitError> {
        read_display_unit(&mut ExprPool::new(), kind, text, true)
    }

    fn units(texts: &[&str]) -> DisplayUnit {
        DisplayUnit::Units(texts.iter().map(|text| (*text).to_string()).collect())
    }

    #[test]
    fn length_takes_a_length_unit() {
        assert_eq!(read(QuantityKind::Length, "km"), Ok(units(&["km"])));
    }

    #[test]
    fn area_takes_a_squared_length() {
        assert_eq!(read(QuantityKind::Area, "m^2"), Ok(units(&["m^2"])));
    }

    #[test]
    fn volume_takes_litres() {
        assert_eq!(read(QuantityKind::Volume, "L"), Ok(units(&["L"])));
    }

    #[test]
    fn angle_takes_degrees() {
        assert_eq!(read(QuantityKind::Angle, "deg"), Ok(units(&["deg"])));
    }

    #[test]
    fn count_takes_the_dimensionless_one() {
        assert_eq!(read(QuantityKind::Count, "1"), Ok(units(&["1"])));
    }

    #[test]
    fn ratio_takes_the_dimensionless_one() {
        assert_eq!(read(QuantityKind::Ratio, "1"), Ok(units(&["1"])));
    }

    #[test]
    fn mass_takes_a_mass_unit() {
        assert_eq!(read(QuantityKind::Mass, "lb"), Ok(units(&["lb"])));
    }

    #[test]
    fn time_takes_a_compound() {
        assert_eq!(read(QuantityKind::Time, "h min"), Ok(units(&["h", "min"])));
    }

    #[test]
    fn speed_takes_a_unit_quotient() {
        assert_eq!(read(QuantityKind::Speed, "km/h"), Ok(units(&["km/h"])));
    }

    #[test]
    fn force_takes_newtons() {
        assert_eq!(read(QuantityKind::Force, "N"), Ok(units(&["N"])));
    }

    #[test]
    fn energy_takes_joules() {
        assert_eq!(read(QuantityKind::Energy, "kJ"), Ok(units(&["kJ"])));
    }

    #[test]
    fn temperature_takes_a_scale_name() {
        assert_eq!(
            read(QuantityKind::Temperature, "celsius"),
            Ok(DisplayUnit::Scale(TemperatureScale::Celsius))
        );
    }

    #[test]
    fn temperature_difference_takes_a_difference_unit() {
        assert_eq!(
            read(QuantityKind::TemperatureDifference, "degF"),
            Ok(units(&["degF"]))
        );
    }

    #[test]
    fn temperature_with_a_unit_instead_of_a_scale_is_invalid() {
        assert_eq!(
            read(QuantityKind::Temperature, "degC"),
            Err(DisplayUnitError::Invalid("degC".to_string()))
        );
    }

    #[test]
    fn unknown_unit_is_invalid() {
        assert_eq!(
            read(QuantityKind::Length, "furlong"),
            Err(DisplayUnitError::Invalid("furlong".to_string()))
        );
    }

    #[test]
    fn expression_that_is_not_a_unit_is_invalid() {
        assert_eq!(
            read(QuantityKind::Length, "m+1"),
            Err(DisplayUnitError::Invalid("m+1".to_string()))
        );
    }

    #[test]
    fn unit_of_another_dimension_is_rejected() {
        assert_eq!(
            read(QuantityKind::Speed, "km"),
            Err(DisplayUnitError::OtherDimension("km".to_string()))
        );
    }

    #[test]
    fn compound_part_of_another_dimension_is_rejected() {
        assert_eq!(
            read(QuantityKind::Length, "m s"),
            Err(DisplayUnitError::OtherDimension("s".to_string()))
        );
    }

    #[test]
    fn compound_for_a_kind_that_does_not_allow_it_is_rejected() {
        assert_eq!(
            read(QuantityKind::Speed, "km/h m/s"),
            Err(DisplayUnitError::CompoundNotAllowed)
        );
    }

    #[test]
    fn compound_where_the_caller_does_not_allow_one_is_rejected() {
        assert_eq!(
            read_display_unit(&mut ExprPool::new(), QuantityKind::Time, "h min", false),
            Err(DisplayUnitError::CompoundNotAllowed)
        );
    }

    #[test]
    fn compound_in_ascending_order_is_rejected() {
        assert_eq!(
            read(QuantityKind::Length, "cm m"),
            Err(DisplayUnitError::CompoundNotDescending("cm m".to_string()))
        );
    }

    #[test]
    fn compound_repeating_a_unit_is_rejected() {
        assert_eq!(
            read(QuantityKind::Mass, "kg kg"),
            Err(DisplayUnitError::CompoundNotDescending("kg kg".to_string()))
        );
    }
}
