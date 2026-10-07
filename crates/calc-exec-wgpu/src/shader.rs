use wgpu::naga;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShaderValidationError {
    Parse { message: String },
    Invalid { message: String },
}

pub fn validate_shader(text: &str) -> Result<(), ShaderValidationError> {
    let module =
        naga::front::wgsl::parse_str(text).map_err(|error| ShaderValidationError::Parse {
            message: error.emit_to_string(text),
        })?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|error| ShaderValidationError::Invalid {
        message: error.emit_to_string(text),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_conformance::{operation_case, reduce_case};
    use calc_exec::{Domain, OperationClass, PlanOp, ReduceOperation, ReduceShape};
    use calc_exec_gpu::{emit_map, emit_reduce, lower_plan};

    fn emitted(plan: &calc_exec::Plan) -> Vec<String> {
        let kernel = lower_plan(plan).unwrap();
        let mut texts = vec![emit_map(&kernel).unwrap()];
        texts.extend(emit_reduce(&kernel).unwrap());
        texts
    }

    #[test]
    fn map_shader_of_every_gpu_operation_validates() {
        let failures: Vec<(PlanOp, ShaderValidationError)> = PlanOp::ALL
            .into_iter()
            .filter(|operation| {
                operation.class() == OperationClass::CorrectlyRounded
                    && *operation != PlanOp::MulAdd
            })
            .flat_map(|operation| {
                let case = operation_case(operation, Domain::F32).unwrap();
                emitted(&case.plan).into_iter().filter_map(move |text| {
                    validate_shader(&text).err().map(|error| (operation, error))
                })
            })
            .collect();

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn reduce_shader_of_every_operation_validates() {
        let failures: Vec<ShaderValidationError> = [
            ReduceOperation::Add,
            ReduceOperation::Mul,
            ReduceOperation::Min,
            ReduceOperation::Max,
        ]
        .into_iter()
        .flat_map(|operation| {
            let case = reduce_case(operation, ReduceShape::Halving, 5, Domain::F32).unwrap();
            emitted(&case.plan)
        })
        .filter_map(|text| validate_shader(&text).err())
        .collect();

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn broken_shader_is_reported() {
        let text = "@compute @workgroup_size(1) fn main() { let value: f32 = missing; }";

        let validated = validate_shader(text);

        assert!(matches!(
            validated,
            Err(ShaderValidationError::Parse { .. })
        ));
    }

    #[test]
    fn type_mismatch_is_reported() {
        let text = "@compute @workgroup_size(1) fn main() { let value: f32 = 1u; }";

        let validated = validate_shader(text);

        assert!(validated.is_err());
    }

    #[test]
    fn escape_time_iteration_shader_validates_at_the_cap() {
        let failures: Vec<ShaderValidationError> = [
            calc_exec::EscapeTimePlanForm::Parameter,
            calc_exec::EscapeTimePlanForm::Initial {
                c_real: calc_exec::Constant::F32(-1.0),
                c_imaginary: calc_exec::Constant::F32(0.0),
            },
        ]
        .into_iter()
        .flat_map(|form| {
            let plan = calc_exec::escape_time_iteration_plan(Domain::F32, form, 65_536).unwrap();
            emitted(&plan)
        })
        .filter_map(|text| validate_shader(&text).err())
        .collect();

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn iteration_case_shaders_validate() {
        let failures: Vec<ShaderValidationError> = calc_conformance::iteration_cases(Domain::F32)
            .iter()
            .filter(|case| lower_plan(&case.plan).is_ok())
            .flat_map(|case| emitted(&case.plan))
            .filter_map(|text| validate_shader(&text).err())
            .collect();

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn escape_time_shader_length_does_not_grow_with_the_cap() {
        let lines = |iterations| {
            let plan = calc_exec::escape_time_iteration_plan(
                Domain::F32,
                calc_exec::EscapeTimePlanForm::Parameter,
                iterations,
            )
            .unwrap();
            emit_map(&lower_plan(&plan).unwrap())
                .unwrap()
                .lines()
                .count()
        };

        let counts = (lines(256), lines(65_536));

        assert_eq!(counts.0, counts.1);
    }
}
