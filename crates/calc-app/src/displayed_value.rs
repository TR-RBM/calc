use calc_concepts::QuantityKind;
use calc_core::evaluate_exact;
use calc_core::{ComputedResult, RationalForm, ResultKind, ResultValue, RoundingError};
use calc_expr::{BuiltinConstant, Head, NodeView, Operator};
use calc_expr::{ExprId, ExprPool};
use calc_numbers::{Integer, Number};
use calc_syntax::{PrintMode, parse_expression, print_expression};
use calc_units::{
    Dimension, DisplayConversionError, DisplayTarget, DisplayedNumber, DisplayedPiMultiple, UnitId,
    convert_for_display, convert_pi_multiple_for_display, scale_spread_for_display,
};

use crate::display_units::{
    Area, CurriculumUnits, DecidedBy, DisplayDecision, DisplayQuestion, DisplayUnit, UnitsChoice,
    ValuePlace, is_named_dimensionless_unit,
};
use crate::readable::{
    PickMagnitude, ReadableValue, exact_reading, expression_reading, measured_texts, terminates,
    value_is_one_unit, written_units,
};
use crate::result_record::{LineId, ResultRecord};
use crate::session::UnresolvedNames;
use crate::session::{Line, Outcome, Session};
use crate::summary::{
    LineState, LineSummary, RoundingSummary, number_text, value_form, value_text_in,
};
use crate::unit_display::display_text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayedForms {
    pub displayed: RationalForm,
    pub coherent: RationalForm,
}

impl DisplayedForms {
    pub fn fractions() -> Self {
        Self {
            displayed: RationalForm::Fraction,
            coherent: RationalForm::Fraction,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeptCoherent {
    ExactValueWouldBeRounded,
    NotConvertible,
    NotAScalar,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayedValue {
    pub unit: Option<String>,
    pub decided_by: DecidedBy,
    pub kept_coherent: Option<KeptCoherent>,
    pub value: String,
    pub number: Option<Number>,
    pub rounded_to_significant_digits: Option<u32>,
    pub uncertainty: Option<String>,
    pub bound: Option<String>,
    pub coherent_value: String,
    pub coherent_unit: Option<String>,
    pub reading: Option<String>,
    pub reading_distance: Option<crate::readable::ReadingDistance>,
}

fn target_of(unit: &DisplayUnit) -> Option<DisplayTarget> {
    match unit {
        DisplayUnit::Unit(unit) => Some(DisplayTarget::Unit(*unit)),
        DisplayUnit::Compound(parts) => parts.last().copied().map(DisplayTarget::Unit),
        DisplayUnit::Scale(scale) => Some(DisplayTarget::Scale(*scale)),
    }
}

fn scalar(value: &ResultValue) -> Option<&Number> {
    match value {
        ResultValue::Number(number) => Some(number),
        ResultValue::Complex { .. } | ResultValue::Array { .. } | ResultValue::Expression(_) => {
            None
        }
    }
}

fn measured_pair(
    value: Option<&Number>,
    uncertainty: Option<&ResultValue>,
) -> Option<(String, String)> {
    measured_texts(value?, scalar(uncertainty?)?)
}

fn scaled_spread<T>(
    spread: Option<&ResultValue>,
    scale: impl Fn(&Number) -> Option<T>,
) -> Result<Option<T>, KeptCoherent> {
    let Some(spread) = spread else {
        return Ok(None);
    };
    scalar(spread)
        .and_then(scale)
        .map(Some)
        .ok_or(KeptCoherent::NotConvertible)
}

fn stored_bound(computed: &ComputedResult) -> Option<&ResultValue> {
    match computed.rounding_error() {
        RoundingError::Bound(bound) => Some(bound),
        RoundingError::None | RoundingError::Unknown => None,
    }
}

fn bound_of(computed: &ComputedResult) -> Option<&Number> {
    match computed.rounding_error() {
        RoundingError::Bound(bound) => scalar(bound),
        RoundingError::None | RoundingError::Unknown => None,
    }
}

impl Session {
    fn unit_symbol(&self, target: DisplayTarget) -> Option<String> {
        match target {
            DisplayTarget::Unit(unit) => display_text(self.pool(), unit),
            DisplayTarget::Scale(scale) => Some(scale.display_symbol().to_string()),
        }
    }

    fn coherent_unit_id(&self, computed: &ComputedResult) -> UnitId {
        computed
            .unit()
            .unwrap_or_else(|| self.pool().units().dimensionless())
    }

    pub fn displayed_forms(&mut self, id: LineId, decision: &DisplayDecision) -> DisplayedForms {
        let coherent_unit = match self.line(id).map(Line::outcome) {
            Some(Outcome::Result(record)) => record.computed().unit(),
            _ => return DisplayedForms::fractions(),
        };
        let displayed_unit = match decision.unit.as_ref().and_then(target_of) {
            Some(DisplayTarget::Unit(unit)) => Some(unit),
            Some(DisplayTarget::Scale(_)) | None => coherent_unit,
        };
        DisplayedForms {
            displayed: self.rational_form_in(id, displayed_unit),
            coherent: self.rational_form_of(id),
        }
    }

    fn coherent_shown(
        &self,
        computed: &ComputedResult,
        decision: &DisplayDecision,
        kept_coherent: Option<KeptCoherent>,
        form: RationalForm,
    ) -> DisplayedValue {
        self.shown_unconverted(computed, decision, kept_coherent, form, None)
    }

    fn shown_unconverted(
        &self,
        computed: &ComputedResult,
        decision: &DisplayDecision,
        kept_coherent: Option<KeptCoherent>,
        form: RationalForm,
        label: Option<UnitId>,
    ) -> DisplayedValue {
        let coherent = computed
            .unit()
            .and_then(|unit| display_text(self.pool(), unit));
        let unit = label
            .and_then(|unit| display_text(self.pool(), unit))
            .or_else(|| coherent.clone());
        let written = value_text_in(computed.value(), form);
        let uncertainty = computed
            .uncertainty()
            .map(|uncertainty| uncertainty.standard());
        let measured = measured_pair(scalar(computed.value()), uncertainty);
        let (value, uncertainty) = match measured {
            Some((value, uncertainty)) => (value, Some(uncertainty)),
            None => (written, uncertainty.map(crate::summary::value_text)),
        };
        DisplayedValue {
            unit,
            decided_by: decision.decided_by,
            kept_coherent,
            value: value.clone(),
            number: None,
            rounded_to_significant_digits: None,
            uncertainty,
            bound: bound_of(computed).map(number_text),
            coherent_value: value,
            coherent_unit: coherent,
            reading: None,
            reading_distance: None,
        }
    }

    fn needs_no_conversion(&self, coherent: UnitId, target: DisplayTarget) -> bool {
        match target {
            DisplayTarget::Unit(unit) => self
                .pool()
                .units()
                .conversion_factor(coherent, unit)
                .is_ok_and(|factor| factor.is_one()),
            DisplayTarget::Scale(_) => false,
        }
    }

    pub fn displayed_value(
        &self,
        record: &ResultRecord,
        decision: &DisplayDecision,
        significant_digits: Option<u32>,
        forms: DisplayedForms,
    ) -> DisplayedValue {
        let computed = record.computed();
        let Some(target) = decision.unit.as_ref().and_then(target_of) else {
            return self.coherent_shown(computed, decision, None, forms.coherent);
        };
        let Some(value) = scalar(computed.value()) else {
            let label = match target {
                DisplayTarget::Unit(unit)
                    if self.needs_no_conversion(self.coherent_unit_id(computed), target) =>
                {
                    Some(unit)
                }
                _ => None,
            };
            return self.shown_unconverted(
                computed,
                decision,
                Some(KeptCoherent::NotAScalar),
                forms.coherent,
                label,
            );
        };
        let table = self.pool().units();
        let coherent = self.coherent_unit_id(computed);
        let converted =
            match convert_for_display(table, value, coherent, target, significant_digits) {
                Ok(converted) => converted,
                Err(DisplayConversionError::ExactValueWouldBeRounded) => {
                    return self.coherent_shown(
                        computed,
                        decision,
                        Some(KeptCoherent::ExactValueWouldBeRounded),
                        forms.coherent,
                    );
                }
                Err(_) => {
                    return self.coherent_shown(
                        computed,
                        decision,
                        Some(KeptCoherent::NotConvertible),
                        forms.coherent,
                    );
                }
            };
        let spread =
            |spread: &Number| scale_spread_for_display(table, spread, coherent, target).ok();
        let scaled = scaled_spread(
            computed
                .uncertainty()
                .map(|uncertainty| uncertainty.standard()),
            spread,
        )
        .and_then(|uncertainty| {
            scaled_spread(stored_bound(computed), spread).map(|bound| (uncertainty, bound))
        });
        let (uncertainty, bound) = match scaled {
            Ok(scaled) => scaled,
            Err(reason) => {
                return self.coherent_shown(computed, decision, Some(reason), forms.coherent);
            }
        };
        let (shown, rounded_to_significant_digits) = match converted {
            DisplayedNumber::Exact(number) | DisplayedNumber::Unscaled(number) => (number, None),
            DisplayedNumber::Rounded {
                value,
                significant_digits,
            } => (value, Some(significant_digits)),
        };
        let coherent_shown = self.coherent_shown(computed, decision, None, forms.coherent);
        let measured = uncertainty
            .as_ref()
            .and_then(|uncertainty| measured_texts(&shown, uncertainty));
        let (value, uncertainty) = match measured {
            Some((value, uncertainty)) => (value, Some(uncertainty)),
            None => (
                match rounded_to_significant_digits {
                    Some(_) => number_text(&shown),
                    None => value_text_in(&ResultValue::Number(shown.clone()), forms.displayed),
                },
                uncertainty.as_ref().map(number_text),
            ),
        };
        let bound = bound.as_ref().map(number_text);
        DisplayedValue {
            unit: self.unit_symbol(target),
            decided_by: decision.decided_by,
            kept_coherent: None,
            value,
            number: Some(shown),
            rounded_to_significant_digits,
            reading: None,
            reading_distance: None,
            uncertainty,
            bound,
            coherent_value: coherent_shown.coherent_value,
            coherent_unit: coherent_shown.coherent_unit,
        }
    }
}

pub(crate) fn exact_expression_text(pool: &mut ExprPool, expression: ExprId) -> Option<String> {
    exact_expression_text_in(pool, expression, calc_core::RationalForm::Decimal)
}

pub(crate) fn exact_expression_text_in(
    pool: &mut ExprPool,
    expression: ExprId,
    form: calc_core::RationalForm,
) -> Option<String> {
    let written = with_written_fractions(pool, expression, true, form)?;
    print_expression(pool, written, PrintMode::Ascii).ok()
}

pub(crate) fn entry_expression_text(pool: &mut ExprPool, expression: ExprId) -> Option<String> {
    let written =
        with_written_fractions(pool, expression, false, calc_core::RationalForm::Decimal)?;
    print_expression(pool, written, PrintMode::Ascii).ok()
}

fn carries_a_coefficient(head: Head) -> bool {
    matches!(
        head,
        Head::Operator(
            Operator::Add | Operator::Sub | Operator::Mul | Operator::Div | Operator::Neg
        )
    )
}

fn fraction_parts(number: &Number) -> Option<(Integer, Integer)> {
    match number {
        Number::Rational(rational) if !rational.denominator().is_one() => {
            Some((rational.numerator().clone(), rational.denominator().clone()))
        }
        _ => None,
    }
}

fn divided_by(pool: &mut ExprPool, dividend: ExprId, divisor: Integer) -> Option<ExprId> {
    let divisor = pool.number(Number::Integer(divisor)).ok()?;
    pool.apply(Head::Operator(Operator::Div), &[dividend, divisor])
        .ok()
}

fn multiplied_by(pool: &mut ExprPool, factor: Integer, expression: ExprId) -> Option<ExprId> {
    if factor.is_one() {
        return Some(expression);
    }
    if factor.negated().is_one() {
        return pool
            .apply(Head::Operator(Operator::Neg), &[expression])
            .ok();
    }
    let factor = pool.number(Number::Integer(factor)).ok()?;
    pool.apply(Head::Operator(Operator::Mul), &[factor, expression])
        .ok()
}

fn literal_fraction(pool: &ExprPool, expression: ExprId) -> Option<(Integer, Integer)> {
    let NodeView::Number(number) = pool.node(expression).ok()? else {
        return None;
    };
    fraction_parts(pool.number_value(number).ok()?)
}

fn scaled_product(
    pool: &mut ExprPool,
    arguments: &[ExprId],
    form: calc_core::RationalForm,
) -> Option<ExprId> {
    let [left, right] = arguments else {
        return None;
    };
    let (numerator, denominator, other) = match literal_fraction(pool, *left) {
        Some((numerator, denominator)) => (numerator, denominator, *right),
        None => {
            let (numerator, denominator) = literal_fraction(pool, *right)?;
            (numerator, denominator, *left)
        }
    };
    let other = with_written_fractions(pool, other, true, form)?;
    let scaled = multiplied_by(pool, numerator, other)?;
    divided_by(pool, scaled, denominator)
}

fn with_written_fractions(
    pool: &mut ExprPool,
    expression: ExprId,
    is_a_coefficient: bool,
    form: calc_core::RationalForm,
) -> Option<ExprId> {
    match pool.node(expression).ok()? {
        NodeView::Number(number) => {
            let value = pool.number_value(number).ok()?;
            let keeps_a_decimal = matches!(form, calc_core::RationalForm::Decimal);
            if !is_a_coefficient && keeps_a_decimal && terminates(value) {
                return Some(expression);
            }
            let Some((numerator, denominator)) = fraction_parts(value) else {
                return Some(expression);
            };
            let numerator = pool.number(Number::Integer(numerator)).ok()?;
            divided_by(pool, numerator, denominator)
        }
        NodeView::Symbol(_) | NodeView::Bound(_) => Some(expression),
        NodeView::Apply { head, arguments } => {
            let arguments = arguments.to_vec();
            if head == Head::Operator(Operator::Mul)
                && let Some(product) = scaled_product(pool, &arguments, form)
            {
                return Some(product);
            }
            let inside = carries_a_coefficient(head);
            let written = arguments
                .into_iter()
                .map(|argument| with_written_fractions(pool, argument, inside, form))
                .collect::<Option<Vec<_>>>()?;
            pool.apply(head, &written).ok()
        }
        NodeView::Quantity { value, unit } => {
            let value = with_written_fractions(pool, value, is_a_coefficient, form)?;
            pool.quantity(value, unit).ok()
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let written = elements
                .into_iter()
                .map(|element| with_written_fractions(pool, element, false, form))
                .collect::<Option<Vec<_>>>()?;
            pool.array(&shape, &written).ok()
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments = arguments.to_vec();
            let body = with_written_fractions(pool, body, false, form)?;
            let written = arguments
                .into_iter()
                .map(|argument| with_written_fractions(pool, argument, false, form))
                .collect::<Option<Vec<_>>>()?;
            let bound = pool.bind(binder, &written, body).ok()?;
            let name = pool.bound_name(expression).map(str::to_string);
            if let Some(name) = name {
                pool.record_bound_name(bound, &name).ok()?;
            }
            Some(bound)
        }
    }
}

pub(crate) fn pi_power_text(
    pool: &mut ExprPool,
    coefficient: &Number,
    exponent: i32,
) -> Option<String> {
    let (numerator, denominator) = match coefficient {
        Number::Integer(integer) => (integer.clone(), Integer::one()),
        Number::Rational(rational) => {
            (rational.numerator().clone(), rational.denominator().clone())
        }
        Number::F32(_) | Number::F64(_) => return None,
    };
    let pi = pool.symbol(BuiltinConstant::Pi.symbol()).ok()?;
    let power_of_pi = match exponent.unsigned_abs() {
        0 => return None,
        1 => pi,
        power => {
            let power = pool
                .number(Number::Integer(Integer::from(u64::from(power))))
                .ok()?;
            pool.apply(Head::Operator(Operator::Pow), &[pi, power])
                .ok()?
        }
    };
    let integer_node =
        |pool: &mut ExprPool, value: Integer| pool.number(Number::Integer(value)).ok();
    let apply = |pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]| {
        pool.apply(Head::Operator(operator), arguments).ok()
    };
    let expression = if exponent > 0 {
        let scaled = if numerator.is_one() {
            power_of_pi
        } else if numerator.negated().is_one() {
            apply(pool, Operator::Neg, &[power_of_pi])?
        } else {
            let factor = integer_node(pool, numerator)?;
            apply(pool, Operator::Mul, &[factor, power_of_pi])?
        };
        if denominator.is_one() {
            scaled
        } else {
            let divisor = integer_node(pool, denominator)?;
            apply(pool, Operator::Div, &[scaled, divisor])?
        }
    } else {
        let divisor = if denominator.is_one() {
            power_of_pi
        } else {
            let factor = integer_node(pool, denominator)?;
            apply(pool, Operator::Mul, &[factor, power_of_pi])?
        };
        let dividend = integer_node(pool, numerator)?;
        apply(pool, Operator::Div, &[dividend, divisor])?
    };
    print_expression(pool, expression, PrintMode::Ascii).ok()
}

impl Session {
    fn written_unit(&self, id: LineId, computed: &ComputedResult) -> Option<DisplayUnit> {
        let value = self.definition_value(id)?;
        match self.pool().node(value).ok()? {
            NodeView::Quantity { unit, .. } => Some(DisplayUnit::Unit(unit)),
            NodeView::Apply {
                head: Head::Operator(Operator::ConvertUnit),
                ..
            } => computed.unit().map(DisplayUnit::Unit),
            _ => None,
        }
    }

    fn calculate_question(&mut self, id: LineId, computed: &ComputedResult) -> DisplayQuestion {
        let dimension = computed
            .unit()
            .and_then(|unit| self.pool().units().dimension(unit).ok())
            .unwrap_or(Dimension::DIMENSIONLESS);
        DisplayQuestion {
            area: Area::Calculate,
            place: ValuePlace::Derived,
            dimension,
            stored: computed.unit().or_else(|| self.named_unit_inside(id)),
            kind: self.temperature_kind(id),
            written: self.written_unit(id, computed),
            stated: None,
            readable: self.readable_value(id, computed),
        }
    }

    fn temperature_kind(&mut self, id: LineId) -> Option<QuantityKind> {
        let expression = self
            .expanded_expression(id, UnresolvedNames::Undefined)
            .ok()?;
        (crate::session::temperature_role(self.pool(), expression)?
            == crate::session::TemperatureRole::Reading)
            .then_some(QuantityKind::Temperature)
    }

    fn named_unit_inside(&mut self, id: LineId) -> Option<UnitId> {
        let expression = self
            .expanded_expression(id, UnresolvedNames::Undefined)
            .ok()?;
        written_units(self.pool(), expression)
            .into_iter()
            .find(|unit| {
                is_named_dimensionless_unit(self.pool(), *unit)
                    && value_is_one_unit(self.pool(), expression, *unit)
            })
    }

    fn readable_value(&mut self, id: LineId, computed: &ComputedResult) -> Option<ReadableValue> {
        let expression = self
            .expanded_expression(id, UnresolvedNames::Undefined)
            .ok()?;
        let inside = written_units(self.pool(), expression);
        let composed = crate::readable::composed_written_unit(self.pool_mut(), expression)
            .filter(|unit| !inside.contains(unit));
        let magnitude = match computed.value() {
            ResultValue::Number(number) => PickMagnitude::Exact(number.to_exact().ok()?),
            ResultValue::Expression(text) => {
                let text = text.clone();
                let stored = parse_expression(self.pool_mut(), &text).ok()?;
                match self.pi_power_of(computed.value()) {
                    Some((coefficient, exponent)) => PickMagnitude::PiPower {
                        coefficient,
                        exponent,
                    },
                    None => PickMagnitude::Expression(stored),
                }
            }
            ResultValue::Complex { .. } | ResultValue::Array { .. } => return None,
        };
        let needs_termination = match computed.value() {
            ResultValue::Number(Number::Integer(_)) => true,
            ResultValue::Number(Number::Rational(_)) => {
                self.rational_form_of(id) == RationalForm::Decimal
            }
            _ => false,
        };
        Some(ReadableValue {
            inside,
            composed,
            magnitude,
            needs_termination,
        })
    }

    fn picked_forms(
        &mut self,
        id: LineId,
        decision: &DisplayDecision,
        question: &DisplayQuestion,
    ) -> DisplayedForms {
        let forms = self.displayed_forms(id, decision);
        let terminates = question
            .readable
            .as_ref()
            .is_some_and(|readable| readable.needs_termination);
        if decision.picked && terminates {
            return DisplayedForms {
                displayed: RationalForm::Decimal,
                ..forms
            };
        }
        forms
    }

    fn reading_of(
        &mut self,
        id: LineId,
        computed: &ComputedResult,
        decision: &DisplayDecision,
        displayed: &DisplayedValue,
        form: RationalForm,
    ) -> Option<String> {
        let is_expression = matches!(computed.value(), ResultValue::Expression(_));
        if computed.uncertainty().is_some() || (displayed.kept_coherent.is_some() && !is_expression)
        {
            return None;
        }
        if matches!(decision.unit, Some(DisplayUnit::Compound(_))) {
            return None;
        }
        match (computed.kind(), computed.value()) {
            (ResultKind::ExactRational, value) => {
                let number = match &displayed.number {
                    Some(number) => number.clone(),
                    None => match value {
                        ResultValue::Number(number) => number.clone(),
                        _ => return None,
                    },
                };
                let prints_as_a_decimal = form == RationalForm::Decimal && terminates(&number);
                if prints_as_a_decimal || !matches!(number, Number::Rational(_)) {
                    return None;
                }
                exact_reading(&number)
            }
            (ResultKind::Algebraic | ResultKind::Symbolic, ResultValue::Expression(text)) => {
                if let Some(number) = &displayed.number {
                    return matches!(number, Number::Rational(_))
                        .then(|| exact_reading(number))
                        .flatten();
                }
                let text = text.clone();
                let stored = parse_expression(self.pool_mut(), &text).ok()?;
                let coherent = self.coherent_unit_id(computed);
                let target = match decision.unit.as_ref().and_then(target_of) {
                    Some(DisplayTarget::Unit(unit)) if displayed.kept_coherent.is_none() => unit,
                    Some(DisplayTarget::Scale(_)) => return None,
                    _ => coherent,
                };
                let factor = self
                    .pool()
                    .units()
                    .conversion_factor(coherent, target)
                    .ok()?;
                expression_reading(self.pool_mut(), stored, &factor)
            }
            _ => {
                let _ = id;
                None
            }
        }
    }

    fn pi_power_of(&mut self, value: &ResultValue) -> Option<(Number, i32)> {
        let ResultValue::Expression(text) = value else {
            return None;
        };
        let expression = parse_expression(self.pool_mut(), text).ok()?;
        evaluate_exact(self.pool_mut(), expression)
            .ok()?
            .pi_power()
            .map(|(coefficient, exponent)| (coefficient.clone(), exponent))
    }

    fn apply_pi_multiple(
        &mut self,
        summary: &mut LineSummary,
        computed: &ComputedResult,
        decision: &DisplayDecision,
        coefficient: &Number,
        pi_exponent: i32,
    ) -> DisplayedValue {
        let coherent_shown = self.coherent_shown(computed, decision, None, RationalForm::Fraction);
        let Some(target) = decision.unit.as_ref().and_then(target_of) else {
            return coherent_shown;
        };
        let coherent = self.coherent_unit_id(computed);
        let shown = convert_pi_multiple_for_display(
            self.pool().units(),
            coefficient,
            pi_exponent,
            coherent,
            target,
        );
        let (value, number) = match shown {
            Ok(DisplayedPiMultiple::Rational(number)) => (Some(number_text(&number)), Some(number)),
            Ok(DisplayedPiMultiple::PiPower {
                coefficient,
                exponent,
            }) => (pi_power_text(self.pool_mut(), &coefficient, exponent), None),
            Err(DisplayConversionError::ExactValueWouldBeRounded) => {
                return DisplayedValue {
                    kept_coherent: Some(KeptCoherent::ExactValueWouldBeRounded),
                    ..coherent_shown
                };
            }
            Err(_) => (None, None),
        };
        let Some(value) = value else {
            return DisplayedValue {
                kept_coherent: Some(KeptCoherent::NotConvertible),
                ..coherent_shown
            };
        };
        let unit = self.unit_symbol(target);
        if let LineState::Result(record_summary) = &mut summary.state {
            record_summary.value.clone_from(&value);
            record_summary.unit.clone_from(&unit);
        }
        DisplayedValue {
            unit,
            value,
            number,
            ..coherent_shown
        }
    }

    pub fn displayed_line_summary(
        &mut self,
        id: LineId,
        preference: Option<&UnitsChoice>,
        coherent_only: bool,
    ) -> Option<LineSummary> {
        self.displayed_line(id, preference, None, coherent_only)
            .map(|(summary, _)| summary)
    }

    pub fn displayed_line(
        &mut self,
        id: LineId,
        preference: Option<&UnitsChoice>,
        curriculum: Option<&CurriculumUnits>,
        coherent_only: bool,
    ) -> Option<(LineSummary, Option<(DisplayDecision, DisplayedValue)>)> {
        let line = self.line(id)?.clone();
        let mut summary = self.line_summary(&line);
        let Outcome::Result(record) = line.outcome() else {
            return Some((summary, None));
        };
        let question = self.calculate_question(id, record.computed());
        let decision = self.display_unit(&question, preference, curriculum, coherent_only);
        if let Some((coefficient, pi_exponent)) = self.pi_power_of(record.computed().value()) {
            let mut displayed = self.apply_pi_multiple(
                &mut summary,
                record.computed(),
                &decision,
                &coefficient,
                pi_exponent,
            );
            displayed.reading = self.reading_of(
                id,
                record.computed(),
                &decision,
                &displayed,
                RationalForm::Fraction,
            );
            displayed.reading_distance = displayed
                .reading
                .as_deref()
                .and_then(|reading| crate::readable::reading_distance(reading, None));
            return Some((summary, Some((decision, displayed))));
        }
        let forms = self.picked_forms(id, &decision, &question);
        let mut displayed = self.displayed_value(record, &decision, None, forms);
        displayed.reading = self.reading_of(
            id,
            record.computed(),
            &decision,
            &displayed,
            forms.displayed,
        );
        let exact = match (&displayed.number, record.computed().value()) {
            (Some(number), _) => Some(number.clone()),
            (None, ResultValue::Number(number)) => Some(number.clone()),
            _ => None,
        };
        displayed.reading_distance = displayed
            .reading
            .as_deref()
            .and_then(|reading| crate::readable::reading_distance(reading, exact.as_ref()));
        if let LineState::Result(record_summary) = &mut summary.state {
            if let (Some(number), ResultKind::ExactRational) =
                (&displayed.number, record.computed().kind())
            {
                record_summary.value_form =
                    value_form(&ResultValue::Number(number.clone()), forms.displayed);
            }
            record_summary.value.clone_from(&displayed.value);
            record_summary.unit.clone_from(&displayed.unit);
            if let (Some(uncertainty), Some(standard)) =
                (record_summary.uncertainty.as_mut(), &displayed.uncertainty)
            {
                uncertainty.standard.clone_from(standard);
            }
            if let (RoundingSummary::Bound(bound), Some(scaled)) =
                (&mut record_summary.rounding_error, &displayed.bound)
            {
                bound.clone_from(scaled);
            }
        }
        Some((summary, Some((decision, displayed))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_place::shipped_unit_systems;
    use crate::display_units::UnitSystem;
    use crate::session::tests::session;
    use calc_concepts::QuantityKind;
    use calc_i18n::Locale;
    use calc_units::TemperatureScale;
    use std::collections::BTreeMap;

    fn record(session: &Session, id: LineId) -> ResultRecord {
        match session.line(id).unwrap().outcome() {
            Outcome::Result(record) => (**record).clone(),
            other => panic!("expected a result, found {other:?}"),
        }
    }

    fn decision(unit: Option<DisplayUnit>) -> DisplayDecision {
        DisplayDecision {
            unit,
            decided_by: DecidedBy::Preference,
            kind: None,
            curriculum: None,
            picked: false,
        }
    }

    fn shown(
        input: &str,
        unit: impl FnOnce(&mut Session) -> Option<DisplayUnit>,
    ) -> DisplayedValue {
        let mut session = session();
        let id = session.enter(input).unwrap();
        let unit = unit(&mut session);
        let record = record(&session, id);
        let forms = session.displayed_forms(id, &decision(unit.clone()));
        session.displayed_value(&record, &decision(unit), None, forms)
    }

    fn named(session: &mut Session, symbol: &str) -> Option<DisplayUnit> {
        Some(DisplayUnit::Unit(
            session.pool_mut().units_mut().lookup(symbol).unwrap(),
        ))
    }

    #[test]
    fn five_kilometres_shown_in_kilometres_report_the_coherent_metres() {
        let displayed = shown("5 km", |session| named(session, "km"));

        assert_eq!(
            (
                displayed.value.as_str(),
                displayed.unit.as_deref(),
                displayed.coherent_value.as_str(),
                displayed.coherent_unit.as_deref()
            ),
            ("5", Some("km"), "5000", Some("m"))
        );
    }

    #[test]
    fn five_kilometres_shown_in_the_coherent_unit_are_metres() {
        let displayed = shown("5 km", |_| None);

        assert_eq!(
            (displayed.value.as_str(), displayed.unit.as_deref()),
            ("5000", Some("m"))
        );
    }

    #[test]
    fn deciding_step_is_reported_with_the_value() {
        let displayed = shown("5 km", |session| named(session, "km"));

        assert_eq!(displayed.decided_by, DecidedBy::Preference);
    }

    #[test]
    fn thirty_degrees_are_shown_in_degrees_correctly_rounded() {
        let displayed = shown("to_f64(30 deg)", |session| named(session, "deg"));

        assert_eq!(
            (
                displayed.value.as_str(),
                displayed.unit.as_deref(),
                displayed.rounded_to_significant_digits
            ),
            ("29.999999999999997", Some("°"), Some(17))
        );
    }

    #[test]
    fn bound_of_thirty_degrees_is_scaled_into_degrees() {
        let displayed = shown("to_f64(30 deg)", |session| named(session, "deg"));

        assert_eq!(displayed.bound.as_deref(), Some("2.544443745170814e-14"));
    }

    #[test]
    fn fahrenheit_reading_is_shown_on_its_scale_as_a_fraction() {
        let displayed = shown("from_fahrenheit(98.6)", |_| {
            Some(DisplayUnit::Scale(TemperatureScale::Fahrenheit))
        });

        assert_eq!(
            (
                displayed.value.as_str(),
                displayed.unit.as_deref(),
                displayed.coherent_value.as_str(),
                displayed.coherent_unit.as_deref()
            ),
            ("493/5", Some("°F"), "6203/20", Some("K"))
        );
    }

    #[test]
    fn exact_value_keeps_its_coherent_unit_rather_than_being_rounded() {
        let displayed = shown("1/2", |session| named(session, "deg"));

        assert_eq!(
            (displayed.value.as_str(), displayed.kept_coherent),
            ("1/2", Some(KeptCoherent::ExactValueWouldBeRounded))
        );
    }

    #[test]
    fn celsius_reading_is_shown_on_its_scale_as_a_decimal() {
        let displayed = shown("from_celsius(20.5)", |_| {
            Some(DisplayUnit::Scale(TemperatureScale::Celsius))
        });

        assert_eq!(
            (displayed.value.as_str(), displayed.coherent_value.as_str()),
            ("20.5", "293.65")
        );
    }

    #[test]
    fn decimal_quantity_shown_in_its_written_unit_is_a_decimal() {
        let displayed = shown("1.5 km/h", |session| {
            let expression = calc_syntax::parse_expression(session.pool_mut(), "1 km/h").unwrap();
            match session.pool().node(expression).unwrap() {
                calc_expr::NodeView::Quantity { unit, .. } => Some(DisplayUnit::Unit(unit)),
                _ => None,
            }
        });

        assert_eq!(
            (displayed.value.as_str(), displayed.coherent_value.as_str()),
            ("1.5", "5/12")
        );
    }

    #[test]
    fn requested_digits_are_the_one_rounding_of_a_converted_machine_value() {
        let mut session = session();
        let id = session.enter("to_f64(30 deg)").unwrap();
        let unit = named(&mut session, "deg");
        let record = record(&session, id);

        let forms = session.displayed_forms(id, &decision(unit.clone()));
        let displayed = session.displayed_value(&record, &decision(unit), Some(4), forms);

        assert_eq!(
            (
                displayed.value.as_str(),
                displayed.rounded_to_significant_digits
            ),
            ("30", Some(4))
        );
    }

    fn summary_value_and_unit(summary: &LineSummary) -> (String, Option<String>) {
        match &summary.state {
            LineState::Result(record) => (record.value.clone(), record.unit.clone()),
            other => panic!("expected a result, found {other:?}"),
        }
    }

    fn displayed_summary(input: &str, preference: Option<&str>) -> LineSummary {
        let mut session = session();
        let id = session.enter(input).unwrap();
        let overrides: Vec<&str> = preference.into_iter().collect();
        let choice = UnitsChoice::from_settings(None, &overrides).unwrap();
        session
            .displayed_line_summary(id, preference.map(|_| &choice), false)
            .unwrap()
    }

    fn deciding_step(input: &str, preference: Option<&str>) -> Option<DecidedBy> {
        let mut session = session();
        let id = session.enter(input).unwrap();
        let overrides: Vec<&str> = preference.into_iter().collect();
        let choice = UnitsChoice::from_settings(None, &overrides).unwrap();
        let (_, display) = session
            .displayed_line(id, preference.map(|_| &choice), None, false)
            .unwrap();
        display.map(|(decision, _)| decision.decided_by)
    }

    #[test]
    fn displayed_line_reports_the_preference_that_chose_its_unit() {
        assert_eq!(
            deciding_step("2 km + 3 km", Some("length=m")),
            Some(DecidedBy::Preference)
        );
    }

    #[test]
    fn displayed_line_of_a_failed_line_has_no_display() {
        assert_eq!(deciding_step("1 / 0", None), None);
    }

    #[test]
    fn written_kilometres_print_as_kilometres() {
        let summary = displayed_summary("5 km", None);

        assert_eq!(
            summary_value_and_unit(&summary),
            ("5".to_string(), Some("km".to_string()))
        );
    }

    #[test]
    fn written_unit_comes_before_the_preference() {
        let summary = displayed_summary("5 km", Some("length=m"));

        assert_eq!(
            summary_value_and_unit(&summary),
            ("5".to_string(), Some("km".to_string()))
        );
    }

    #[test]
    fn sum_of_lengths_prints_in_the_preferred_metres() {
        let summary = displayed_summary("2 km + 3 km", Some("length=m"));

        assert_eq!(
            summary_value_and_unit(&summary),
            ("5000".to_string(), Some("m".to_string()))
        );
    }

    #[test]
    fn sum_of_lengths_prints_in_the_preferred_kilometres() {
        let summary = displayed_summary("2 km + 3 km", Some("length=km"));

        assert_eq!(
            summary_value_and_unit(&summary),
            ("5".to_string(), Some("km".to_string()))
        );
    }

    #[test]
    fn rounding_bound_prints_in_the_displayed_unit() {
        let summary = displayed_summary("to_f64(30 deg)", Some("angle=deg"));

        let LineState::Result(record) = &summary.state else {
            panic!("expected a result");
        };
        assert_eq!(
            record.rounding_error,
            RoundingSummary::Bound("2.544443745170814e-14".to_string())
        );
    }

    #[test]
    fn kind_word_follows_the_displayed_value() {
        let summary = displayed_summary("1.5 km/h", None);

        let LineState::Result(record) = &summary.state else {
            panic!("expected a result");
        };
        assert_eq!(
            (record.value.as_str(), record.value_form),
            ("1.5", Some(crate::summary::ValueForm::ExactDecimal))
        );
    }

    #[test]
    fn spread_that_does_not_convert_keeps_the_whole_value_coherent() {
        let bound = ResultValue::Number(Number::from(1_i64));

        let scaled = scaled_spread(Some(&bound), |_| None::<Number>);

        assert_eq!(scaled, Err(KeptCoherent::NotConvertible));
    }

    #[test]
    fn absent_spread_needs_no_conversion() {
        let scaled = scaled_spread(None, |_| None::<Number>);

        assert_eq!(scaled, Ok(None));
    }

    #[test]
    fn thirty_degrees_print_as_thirty_degrees_exactly() {
        let summary = displayed_summary("30 deg", None);

        let LineState::Result(record) = &summary.state else {
            panic!("expected a result");
        };
        assert_eq!(
            (
                record.value.as_str(),
                record.unit.as_deref(),
                record.kind,
                &record.rounding_error
            ),
            (
                "30",
                Some("°"),
                ResultKind::Symbolic,
                &RoundingSummary::Exact
            )
        );
    }

    #[test]
    fn a_frequency_keeps_the_hertz_that_was_written_inside_it() {
        let summary = displayed_summary("2 * 3 Hz", None);

        assert_eq!(
            summary_value_and_unit(&summary),
            ("6".to_owned(), Some("Hz".to_owned()))
        );
    }

    #[test]
    fn an_irradiance_keeps_the_watts_per_square_metre_written_inside_it() {
        let summary = displayed_summary("pi * 1 W/m^2", None);

        assert_eq!(
            summary_value_and_unit(&summary).1,
            Some("W/m\u{b2}".to_owned())
        );
    }

    #[test]
    fn a_value_with_no_written_unit_of_its_dimension_keeps_the_coherent_one() {
        let summary = displayed_summary("1/(0.5 s)", None);

        assert_eq!(
            summary_value_and_unit(&summary),
            ("2".to_owned(), Some("s\u{207b}\u{b9}".to_owned()))
        );
    }

    fn measured(input: &str) -> (String, Option<String>) {
        let summary = displayed_summary(input, None);
        let LineState::Result(record) = &summary.state else {
            panic!("expected a result");
        };
        (
            record.value.clone(),
            record
                .uncertainty
                .as_ref()
                .map(|uncertainty| uncertainty.standard.clone()),
        )
    }

    #[test]
    fn a_measured_value_is_written_to_the_place_its_uncertainty_shows() {
        assert_eq!(measured("(2 +- 0.1) * 3").0, "6.00".to_owned());
    }

    #[test]
    fn a_measured_uncertainty_keeps_two_significant_digits() {
        let (value, uncertainty) = measured("G_N * 5.97e24 kg / (6.371e6 m)^2");

        assert_eq!(
            (value, uncertainty.map(|shown| shown.contains("0.00022"))),
            ("9.81668".to_owned(), Some(true))
        );
    }

    #[test]
    fn the_uncertainty_is_rounded_before_the_value_follows_it() {
        assert_eq!(measured("(1.23456 +- 0.0999) * 1").0, "1.23".to_owned());
    }

    #[test]
    fn a_measured_value_past_the_window_takes_the_uncertainty_to_its_own_power() {
        let (value, uncertainty) = measured("(6.02e23 +- 1e21) * 1");

        assert_eq!(
            (value, uncertainty.map(|shown| shown.contains("0.010e23"))),
            ("6.020e23".to_owned(), Some(true))
        );
    }

    #[test]
    fn an_uncertainty_larger_than_its_value_writes_both_to_the_same_place() {
        let (value, uncertainty) = measured("(2 +- 5) * 1");

        assert_eq!(
            (value, uncertainty.map(|shown| shown.contains("5.0"))),
            ("2.0".to_owned(), Some(true))
        );
    }

    #[test]
    fn an_uncertainty_above_the_point_writes_the_value_to_its_place() {
        assert_eq!(measured("(12345 +- 120) * 1").0, "12340".to_owned());
    }

    #[test]
    fn a_line_with_no_uncertainty_keeps_the_form_it_had() {
        assert_eq!(measured("1/3").0, "1/3".to_owned());
    }

    #[test]
    fn cosine_of_an_angle_is_not_written_in_the_angle_unit() {
        let summary = displayed_summary("cos(30 deg)", None);

        assert_eq!(
            summary_value_and_unit(&summary),
            ("sqrt(3) / 2".to_owned(), None)
        );
    }

    #[test]
    fn a_number_added_to_an_angle_is_written_in_no_unit() {
        let summary = displayed_summary("sqrt(2)/2 + 0 * 1 deg", None);

        assert_eq!(
            summary_value_and_unit(&summary),
            ("sqrt(2) / 2".to_owned(), None)
        );
    }

    #[test]
    fn a_squared_angle_is_not_written_in_the_angle_unit() {
        let summary = displayed_summary("(30 deg)^2", None);

        assert_eq!(summary_value_and_unit(&summary).1, None);
    }

    #[test]
    fn an_angle_scaled_by_a_number_keeps_its_angle_unit() {
        let summary = displayed_summary("30 deg * 2", None);

        assert_eq!(
            summary_value_and_unit(&summary),
            ("60".to_owned(), Some("\u{b0}".to_owned()))
        );
    }

    #[test]
    fn a_sum_of_two_angles_keeps_its_angle_unit() {
        let summary = displayed_summary("30 deg + 15 deg", None);

        assert_eq!(
            summary_value_and_unit(&summary),
            ("45".to_owned(), Some("\u{b0}".to_owned()))
        );
    }

    #[test]
    fn sixth_of_pi_is_stored_as_an_exact_expression() {
        let mut session = session();
        let id = session.enter("pi/6").unwrap();

        assert_eq!(
            record(&session, id).computed().value(),
            &ResultValue::Expression("pi / 6".to_string())
        );
    }

    #[test]
    fn negative_sixth_of_pi_is_written_with_a_leading_minus() {
        let mut session = session();
        let id = session.enter("-pi/6").unwrap();

        assert_eq!(
            record(&session, id).computed().value(),
            &ResultValue::Expression("-pi / 6".to_string())
        );
    }

    #[test]
    fn thirty_degrees_display_carries_the_unit_and_the_coherent_pi_multiple() {
        let mut session = session();
        let id = session.enter("30 deg").unwrap();

        let (_, display) = session.displayed_line(id, None, None, false).unwrap();

        let (decision, displayed) = display.unwrap();
        assert_eq!(
            (
                decision.decided_by,
                displayed.value.as_str(),
                displayed.unit.as_deref(),
                displayed.coherent_value.as_str(),
                displayed.kept_coherent
            ),
            (DecidedBy::Written, "30", Some("°"), "pi / 6", None)
        );
    }

    fn stored_value(input: &str) -> ResultValue {
        let mut session = session();
        let id = session.enter(input).unwrap();
        record(&session, id).computed().value().clone()
    }

    #[test]
    fn square_of_pi_over_six_is_stored_exactly() {
        assert_eq!(
            stored_value("pi^2/6"),
            ResultValue::Expression("pi^2 / 6".to_string())
        );
    }

    #[test]
    fn reciprocal_of_pi_is_stored_exactly() {
        assert_eq!(
            stored_value("1/pi"),
            ResultValue::Expression("1 / pi".to_string())
        );
    }

    #[test]
    fn negative_power_of_pi_text_reads_back_to_the_same_power() {
        let mut session = session();
        let id = session.enter("-pi^2/6").unwrap();
        let value = record(&session, id).computed().value().clone();

        assert_eq!(
            session.pi_power_of(&value),
            Some((
                Number::fraction(&Integer::from(-1_i64), &Integer::from(6_i64)).unwrap(),
                2
            ))
        );
    }

    #[test]
    fn one_square_degree_shows_exactly_in_square_degrees() {
        let summary = displayed_summary("1 deg^2", None);

        assert_eq!(
            summary_value_and_unit(&summary),
            ("1".to_string(), Some("°²".to_string()))
        );
    }

    #[test]
    fn square_of_pi_over_six_has_its_enclosure_view() {
        let mut session = session();
        let id = session.enter("pi^2/6").unwrap();
        let ResultValue::Expression(text) = record(&session, id).computed().value().clone() else {
            panic!("expected an expression");
        };
        let expression = calc_syntax::parse_expression(session.pool_mut(), &text).unwrap();

        let enclosure =
            calc_core::enclose_decimal(session.pool_mut(), expression, 20, &|| false).unwrap();

        let decimal = |digits: i128| {
            Number::fraction(&Integer::from(digits), &Integer::from(10_i64).pow(19)).unwrap()
        };
        assert_eq!(
            (enclosure.lower, enclosure.upper, enclosure.reached),
            (
                decimal(16_449_340_668_482_264_364),
                decimal(16_449_340_668_482_264_365),
                true
            )
        );
    }

    fn si_like_system() -> UnitSystem {
        UnitSystem {
            identifier: "si".to_owned(),
            displayed: BTreeMap::from([
                (QuantityKind::Length, "mm, cm, m, km".to_owned()),
                (QuantityKind::Volume, "mm^3, cm^3, m^3".to_owned()),
                (QuantityKind::Time, "s".to_owned()),
                (QuantityKind::Speed, "m/s".to_owned()),
            ]),
        }
    }

    fn readable_display(input: &str) -> (LineSummary, DisplayedValue) {
        let mut session = session();
        session.set_unit_systems(vec![si_like_system()]);
        let id = session.enter(input).unwrap();
        let choice = UnitsChoice {
            system: Some("si".to_owned()),
            overrides: BTreeMap::new(),
        };
        let (summary, display) = session
            .displayed_line(id, Some(&choice), None, false)
            .unwrap();
        let (_, displayed) = display.unwrap();
        (summary, displayed)
    }

    #[test]
    fn speed_times_seconds_reads_in_metres_with_its_reading() {
        let (_, displayed) = readable_display("100 km/h * 5 s");

        assert_eq!(
            (
                displayed.value.as_str(),
                displayed.unit.as_deref(),
                displayed.reading.as_deref()
            ),
            ("1250/9", Some("m"), Some("138.9"))
        );
    }

    #[test]
    fn speed_times_ninety_seconds_reads_as_two_and_a_half_kilometres() {
        let (_, displayed) = readable_display("100 km/h * 90 s");

        assert_eq!(
            (
                displayed.value.as_str(),
                displayed.unit.as_deref(),
                displayed.reading.as_deref()
            ),
            ("2.5", Some("km"), None)
        );
    }

    #[test]
    fn litres_per_second_times_an_hour_reads_in_litres() {
        let (_, displayed) = readable_display("5 l/s * 1 h");

        assert_eq!(
            (displayed.value.as_str(), displayed.unit.as_deref()),
            ("18000", Some("L"))
        );
    }

    fn shipped_si() -> UnitSystem {
        let shipped = shipped_unit_systems(&Locale::source())
            .into_iter()
            .find(|system| system.identifier == "si")
            .expect("the si system is shipped");
        UnitSystem {
            identifier: shipped.identifier,
            displayed: shipped.displayed,
        }
    }

    #[test]
    fn sum_of_plain_numbers_prints_without_a_unit() {
        let displayed = display_in_system("2 + 3", shipped_si());

        assert_eq!(value_and_unit(&displayed), ("5".to_owned(), None));
    }

    #[test]
    fn written_degrees_keep_their_unit() {
        let displayed = display_in_system("30 deg", shipped_si());

        assert_eq!(
            value_and_unit(&displayed),
            ("30".to_owned(), Some("\u{b0}".to_owned()))
        );
    }

    #[test]
    fn sine_of_a_plain_number_prints_without_a_unit() {
        let displayed = display_in_system("sin(1)", shipped_si());

        assert_eq!(displayed.unit, None);
    }

    #[test]
    fn ratio_of_two_lengths_prints_without_a_unit() {
        let displayed = display_in_system("4 m / 2 m", shipped_si());

        assert_eq!(value_and_unit(&displayed), ("2".to_owned(), None));
    }

    #[test]
    fn torque_is_not_read_as_energy() {
        let displayed = display_in_system("2 N * 3 m", shipped_si());

        assert_eq!(displayed.unit.as_deref(), Some("N\u{b7}m"));
    }

    #[test]
    fn a_torque_from_mass_and_acceleration_is_not_read_as_energy() {
        let displayed = display_in_system("2 kg * 3 m/s^2 * 4 m", shipped_si());

        assert_eq!(displayed.unit.as_deref(), Some("kg\u{b7}m\u{b2}/s\u{b2}"));
    }

    #[test]
    fn a_torque_scaled_by_a_number_is_still_a_torque() {
        for input in ["2 N * 3 m * 2", "0.5 * (2 N * 3 m)", "(2 N * 3 m) / 7"] {
            let displayed = display_in_system(input, shipped_si());

            assert_eq!(displayed.unit.as_deref(), Some("N\u{b7}m"), "{input}");
        }
    }

    #[test]
    fn a_torque_added_to_a_torque_keeps_the_unit() {
        let displayed = display_in_system("2 N * 3 m + 1 N * 1 m", shipped_si());

        assert_eq!(displayed.unit.as_deref(), Some("N\u{b7}m"));
    }

    #[test]
    fn a_written_joule_still_reads_in_joules() {
        let displayed = display_in_system("5 J", shipped_si());

        assert_eq!(displayed.unit.as_deref(), Some("J"));
    }

    #[test]
    fn a_length_still_takes_its_list() {
        let displayed = display_in_system("1000 m + 1500 m", shipped_si());

        assert_eq!(displayed.unit.as_deref(), Some("km"));
    }

    #[test]
    fn a_temperature_in_kelvin_keeps_its_written_kelvin() {
        let displayed = display_in_system("300 K", shipped_si());

        assert_eq!(displayed.unit.as_deref(), Some("K"));
    }

    #[test]
    fn machine_value_of_an_angle_keeps_its_written_degrees() {
        let displayed = display_in_system("to_f64(30 deg)", shipped_si());

        assert_eq!(displayed.unit.as_deref(), Some("\u{b0}"));
    }

    #[test]
    fn angle_in_radians_takes_the_declared_radians() {
        let displayed = display_in_system("0.5 rad + 0.5 rad", shipped_si());

        assert_eq!(displayed.unit.as_deref(), Some("rad"));
    }

    fn us_like_system() -> UnitSystem {
        UnitSystem {
            identifier: "us".to_owned(),
            displayed: BTreeMap::from([(QuantityKind::Length, "in, ft, mi".to_owned())]),
        }
    }

    fn display_in_system(input: &str, system: UnitSystem) -> DisplayedValue {
        let mut session = session();
        let identifier = system.identifier.clone();
        session.set_unit_systems(vec![system]);
        let id = session.enter(input).unwrap();
        let choice = UnitsChoice {
            system: Some(identifier),
            overrides: BTreeMap::new(),
        };
        let (_, display) = session
            .displayed_line(id, Some(&choice), None, false)
            .unwrap();
        display.unwrap().1
    }

    fn table_row(input: &str) -> (String, Option<String>, Option<String>) {
        let displayed = display_in_system(input, shipped_si());
        (
            displayed.value.clone(),
            displayed.unit.clone(),
            displayed.reading.clone(),
        )
    }

    #[test]
    fn a_written_unit_too_large_to_read_gives_way_to_the_chosen_list() {
        let displayed = display_in_system("100 km/h * 5 s", us_like_system());

        assert_eq!(displayed.unit, Some("ft".to_owned()));
    }

    #[test]
    fn a_written_unit_that_reads_keeps_the_step() {
        let displayed = display_in_system("5 l/s * 1 h", shipped_si());

        assert_eq!(displayed.unit, Some("L".to_owned()));
    }

    #[test]
    fn a_speed_over_five_hours_reads_in_kilometres() {
        assert_eq!(
            table_row("100 km/h * 5 h"),
            ("500".to_owned(), Some("km".to_owned()), None)
        );
    }

    #[test]
    fn a_speed_over_five_seconds_reads_in_metres_with_its_reading() {
        assert_eq!(
            table_row("100 km/h * 5 s"),
            (
                "1250/9".to_owned(),
                Some("m".to_owned()),
                Some("138.9".to_owned())
            )
        );
    }

    #[test]
    fn a_speed_over_ninety_seconds_reads_as_two_and_a_half_kilometres() {
        assert_eq!(
            table_row("100 km/h * 90 s"),
            ("2.5".to_owned(), Some("km".to_owned()), None)
        );
    }

    #[test]
    fn a_flow_over_an_hour_reads_in_the_litres_written_inside_it() {
        assert_eq!(
            table_row("5 l/s * 1 h"),
            ("18000".to_owned(), Some("L".to_owned()), None)
        );
    }

    #[test]
    fn a_unit_written_inside_a_compound_on_the_right_is_a_candidate() {
        assert_eq!(
            table_row("1 h * 5 l/s"),
            ("18000".to_owned(), Some("L".to_owned()), None)
        );
    }

    #[test]
    fn a_unit_written_inside_a_compound_on_the_left_is_a_candidate() {
        assert_eq!(
            table_row("5 s * 100 km/h"),
            (
                "1250/9".to_owned(),
                Some("m".to_owned()),
                Some("138.9".to_owned())
            )
        );
    }

    fn value_and_unit(displayed: &DisplayedValue) -> (String, Option<String>) {
        (displayed.value.clone(), displayed.unit.clone())
    }

    #[test]
    fn half_a_metre_reads_in_centimetres() {
        let displayed = display_in_system("0.5 * 1 m", si_like_system());

        assert_eq!(
            value_and_unit(&displayed),
            ("50".to_owned(), Some("cm".to_owned()))
        );
    }

    #[test]
    fn just_below_a_kilometre_stays_in_metres() {
        let displayed = display_in_system("999.9 * 1 m", si_like_system());

        assert_eq!(
            value_and_unit(&displayed),
            ("999.9".to_owned(), Some("m".to_owned()))
        );
    }

    #[test]
    fn eighteen_inches_read_as_one_and_a_half_feet() {
        let displayed = display_in_system("sqrt(0.20903184 m^2)", us_like_system());

        assert_eq!(
            value_and_unit(&displayed),
            ("1.5".to_owned(), Some("ft".to_owned()))
        );
    }

    #[test]
    fn sixteen_inches_stay_inches_because_feet_would_be_a_fraction() {
        let displayed = display_in_system("sqrt(0.16516096 m^2)", us_like_system());

        assert_eq!(
            value_and_unit(&displayed),
            ("16".to_owned(), Some("in".to_owned()))
        );
    }

    #[test]
    fn a_metre_in_the_us_list_falls_back_to_the_coherent_metre() {
        let displayed = display_in_system("sqrt(1 m^2)", us_like_system());

        assert_eq!(
            value_and_unit(&displayed),
            ("1".to_owned(), Some("m".to_owned()))
        );
    }

    #[test]
    fn a_fraction_that_ends_within_four_digits_gets_no_reading() {
        let displayed = display_in_system("3/2 * 1 m", si_like_system());

        assert_eq!(
            (displayed.value.as_str(), displayed.reading.as_deref()),
            ("3/2", None)
        );
    }

    #[test]
    fn a_multiple_of_pi_reads_with_four_digits() {
        let mut session = session();
        let id = session.enter("pi/6").unwrap();

        let (_, display) = session.displayed_line(id, None, None, false).unwrap();

        assert_eq!(display.unwrap().1.reading.as_deref(), Some("0.5236"));
    }

    #[test]
    fn a_machine_value_gets_no_reading() {
        let mut session = session();
        let id = session.enter("to_f64(1/3 * 1 m)").unwrap();

        let (_, display) = session.displayed_line(id, None, None, false).unwrap();

        assert_eq!(display.unwrap().1.reading, None);
    }

    #[test]
    fn a_large_fraction_reads_in_exponent_form() {
        let mut session = session();
        let id = session.enter("10^7/3").unwrap();

        let (_, display) = session.displayed_line(id, None, None, false).unwrap();

        assert_eq!(display.unwrap().1.reading.as_deref(), Some("3.333e6"));
    }

    #[test]
    fn a_measured_length_picks_its_unit_and_scales_its_uncertainty() {
        let displayed = display_in_system("(2500 +- 1) * 1 m", si_like_system());

        assert_eq!(
            (
                displayed.value.as_str(),
                displayed.unit.as_deref(),
                displayed.uncertainty.as_deref(),
                displayed.reading.as_deref()
            ),
            ("2.5000", Some("km"), Some("0.0010"), None)
        );
    }
}
