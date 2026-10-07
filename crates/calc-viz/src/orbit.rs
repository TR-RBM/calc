use calc_exec::{Backend, Batch, Domain, Preference, escape_time_iteration_plan, select};
use calc_numbers::Number;

use crate::escape_time::{
    ESCAPED_CELL, EscapeTimeForm, INSIDE_CELL, IterationLimitRule, UNDECIDED_CELL,
    is_proven_inside, iterations_for, plan_form,
};
use crate::record::Interval as ExactInterval;
use crate::sampling::SampleError;

const ESCAPED_FLAG: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Orbit {
    pub class: u8,
    pub count: Option<u32>,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrbitRequest {
    pub form: EscapeTimeForm,
    pub limit_rule: IterationLimitRule,
    pub real_axis: ExactInterval,
    pub domain: Domain,
    pub preference: Preference,
    pub real: Number,
    pub imaginary: Number,
}

pub fn escape_time_limit(
    limit_rule: IterationLimitRule,
    form: &EscapeTimeForm,
    real_axis: &ExactInterval,
) -> Option<u32> {
    iterations_for(limit_rule, form, real_axis)
}

pub fn read_orbit(backends: &[&dyn Backend], request: &OrbitRequest) -> Result<Orbit, SampleError> {
    let limit = iterations_for(request.limit_rule, &request.form, &request.real_axis)
        .ok_or(SampleError::EscapeTimeSettingsMissing)?;
    let plan = escape_time_iteration_plan(
        request.domain,
        plan_form(&request.form, request.domain),
        limit,
    )
    .map_err(|_| SampleError::IterationLimitTooLarge { iterations: limit })?;
    let coordinates = [&request.real, &request.imaginary];
    let inputs = match request.domain {
        Domain::F32 => Batch::from_f32_columns(
            1,
            coordinates
                .iter()
                .map(|value| vec![value.round_to_f32_ties_even()])
                .collect(),
        ),
        Domain::F64 => Batch::from_f64_columns(
            1,
            coordinates
                .iter()
                .map(|value| vec![value.round_to_f64_ties_even()])
                .collect(),
        ),
    }
    .map_err(|_| SampleError::EscapeTimeSettingsMissing)?;
    let selection = select(backends, &plan, 1, request.preference).map_err(SampleError::Select)?;
    let mut outputs = Batch::zeroed(
        request.domain,
        plan.output_channel_count(),
        plan.output_length(1),
    );
    let mut prepared = selection.prepared;
    prepared
        .run(&inputs, &mut outputs)
        .map_err(SampleError::Run)?;
    let channel = |index: usize| match request.domain {
        Domain::F32 => outputs
            .f32_channel(index)
            .and_then(|values| values.first().copied())
            .map(f64::from),
        Domain::F64 => outputs
            .f64_channel(index)
            .and_then(|values| values.first().copied()),
    };
    let count = channel(0).ok_or(SampleError::EscapeTimeSettingsMissing)?;
    let escaped = channel(1).ok_or(SampleError::EscapeTimeSettingsMissing)?;
    let is_inside = request.form == EscapeTimeForm::QuadraticParameter
        && is_proven_inside(&request.real, &request.imaginary);
    let class = if is_inside {
        INSIDE_CELL
    } else if escaped == ESCAPED_FLAG {
        ESCAPED_CELL
    } else {
        UNDECIDED_CELL
    };
    Ok(Orbit {
        class,
        count: (class == ESCAPED_CELL).then(|| whole(count)).flatten(),
        limit,
    })
}

fn whole(value: f64) -> Option<u32> {
    match Number::F64(value).to_exact().ok()? {
        Number::Integer(integer) => integer.to_i64().and_then(|count| u32::try_from(count).ok()),
        Number::Rational(_) | Number::F32(_) | Number::F64(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use calc_exec_cpu::CpuBackend;
    use calc_numbers::Integer;

    use super::*;

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn request(real: Number, imaginary: Number, limit_rule: IterationLimitRule) -> OrbitRequest {
        OrbitRequest {
            form: EscapeTimeForm::QuadraticParameter,
            limit_rule,
            real_axis: ExactInterval {
                lower: fraction(-5, 2),
                upper: Number::from(1_i64),
            },
            domain: Domain::F64,
            preference: Preference::Automatic,
            real,
            imaginary,
        }
    }

    fn read(request: &OrbitRequest) -> Orbit {
        let backend = CpuBackend::new();
        read_orbit(&[&backend], request).unwrap()
    }

    #[test]
    fn point_far_outside_escapes_with_its_count() {
        let orbit = read(&request(
            Number::from(2_i64),
            Number::from(2_i64),
            IterationLimitRule::Fixed { iterations: 32 },
        ));

        assert_eq!(
            (orbit.class, orbit.count, orbit.limit),
            (ESCAPED_CELL, Some(2), 32)
        );
    }

    #[test]
    fn point_proven_inside_is_inside_without_a_count() {
        let orbit = read(&request(
            Number::from(0_i64),
            Number::from(0_i64),
            IterationLimitRule::Fixed { iterations: 32 },
        ));

        assert_eq!((orbit.class, orbit.count), (INSIDE_CELL, None));
    }

    #[test]
    fn point_that_neither_escapes_nor_is_proven_inside_is_undecided() {
        let orbit = read(&request(
            fraction(-3, 4),
            fraction(1, 4),
            IterationLimitRule::Fixed { iterations: 8 },
        ));

        assert_eq!((orbit.class, orbit.count), (UNDECIDED_CELL, None));
    }

    #[test]
    fn limit_of_a_depth_following_rule_is_the_limit_of_the_view() {
        let orbit = read(&request(
            Number::from(2_i64),
            Number::from(2_i64),
            IterationLimitRule::DEFAULT,
        ));

        assert_eq!(orbit.limit, 256);
    }
}
