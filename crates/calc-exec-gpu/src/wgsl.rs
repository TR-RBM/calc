use std::fmt::Write;

use calc_exec::{PlanOp, ReduceOperation};

use crate::kernel::{
    BinaryOperation, Comparison, Expression, Kernel, KernelIteration, LocalId, UnaryOperation,
};
use crate::modes::{OperationModes, plan_comparison};
use crate::validate::{KernelError, validate};

pub const WORKGROUP_SIZE: u32 = 64;
pub const WORKGROUPS_PER_DIMENSION: u32 = 65_535;
pub const MAP_ENTRY_POINT: &str = "map_main";
pub const REDUCE_ENTRY_POINT: &str = "reduce_main";

const SIGN_MASK: &str = "0x80000000u";
const MAGNITUDE_MASK: &str = "0x7fffffffu";
const FORBIDDEN_TYPE: &str = "f16";

const PRELUDE_TAIL: &str = "\
struct Size {
    element_count: u32,
    invocations: u32,
}

fn sf_less_equal(a: u32, b: u32) -> bool {
    return !sf_is_nan(a) && !sf_is_nan(b) && !sf_greater(a, b);
}

fn sf_equal(a: u32, b: u32) -> bool {
    return !sf_is_nan(a) && !sf_is_nan(b) && (a == b || ((a | b) & 0x7fffffffu) == 0u);
}






";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderTextError {
    HalfPrecision { offset: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitError {
    Kernel(KernelError),
    Text(ShaderTextError),
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

pub fn check_shader_text(text: &str) -> Result<(), ShaderTextError> {
    for (offset, _) in text.match_indices(FORBIDDEN_TYPE) {
        let before = text[..offset].chars().next_back();
        let after = text[offset + FORBIDDEN_TYPE.len()..].chars().next();
        let stands_alone = !before.is_some_and(is_identifier_character)
            && !after.is_some_and(is_identifier_character);
        if stands_alone {
            return Err(ShaderTextError::HalfPrecision { offset });
        }
    }
    Ok(())
}

fn name(local: LocalId) -> String {
    format!("v{}", local.0)
}

fn body_name(iteration: usize) -> impl Fn(LocalId) -> String {
    move |local| format!("b{iteration}_{}", local.0)
}

fn state_name(iteration: usize, slot: u32) -> String {
    format!("s{iteration}_{slot}")
}

pub fn statement_text(index: usize, expression: &Expression) -> String {
    statement_text_with(index, expression, OperationModes::EXACT)
}

pub fn statement_text_with(index: usize, expression: &Expression, modes: OperationModes) -> String {
    expression_text(&format!("v{index}"), expression, &name, modes)
}

fn native_call(name: &str, left: &str, right: &str) -> String {
    format!("bitcast<u32>({name}(bitcast<f32>({left}), bitcast<f32>({right})))")
}

fn rounding_text(builtin: &str, value: &str, operation: PlanOp, modes: OperationModes) -> String {
    if modes.is_native(operation) {
        return format!("bitcast<u32>({builtin}(bitcast<f32>({value})))");
    }
    let exact = match operation {
        PlanOp::Floor => "sf_floor",
        PlanOp::Ceil => "sf_ceil",
        PlanOp::Trunc => "sf_trunc",
        _ => "sf_round_ties_even",
    };
    format!("{exact}({value})")
}

fn native_comparison(comparison: Comparison, left: &str, right: &str) -> String {
    let operator = match comparison {
        Comparison::Less => "<",
        Comparison::LessOrEqual => "<=",
        Comparison::Greater => ">",
        Comparison::GreaterOrEqual => ">=",
        Comparison::Equal => "==",
        Comparison::NotEqual => "!=",
    };
    format!("bitcast<f32>({left}) {operator} bitcast<f32>({right})")
}

fn expression_text(
    target: &str,
    expression: &Expression,
    name: &dyn Fn(LocalId) -> String,
    modes: OperationModes,
) -> String {
    match *expression {
        Expression::Input { channel } => {
            format!("let {target}: u32 = inputs[{channel}u * size.element_count + element];")
        }
        Expression::State { .. } | Expression::Outer { .. } | Expression::Iterate { .. } => {
            String::new()
        }
        Expression::IterationState { iteration, slot } => {
            format!("let {target} = {};", state_name(iteration.index(), slot))
        }
        Expression::Constant { index: constant } => {
            format!("let {target}: u32 = constants[{constant}u];")
        }
        Expression::Unary(operation, argument) => {
            let value = name(argument);
            let text = match operation {
                UnaryOperation::Neg => format!("{value} ^ {SIGN_MASK}"),
                UnaryOperation::Abs => format!("{value} & {MAGNITUDE_MASK}"),
                UnaryOperation::Sqrt => format!("sf_sqrt({value})"),
                UnaryOperation::Floor => rounding_text("floor", &value, PlanOp::Floor, modes),
                UnaryOperation::Ceil => rounding_text("ceil", &value, PlanOp::Ceil, modes),
                UnaryOperation::Trunc => rounding_text("trunc", &value, PlanOp::Trunc, modes),
                UnaryOperation::RoundTiesEven => {
                    rounding_text("round", &value, PlanOp::RoundTiesEven, modes)
                }
            };
            format!("let {target}: u32 = {text};")
        }
        Expression::Binary(operation, left, right) => {
            let (left_name, right_name) = (name(left), name(right));
            let text = match operation {
                BinaryOperation::Add => format!("sf_add({left_name}, {right_name})"),
                BinaryOperation::Sub => format!("sf_sub({left_name}, {right_name})"),
                BinaryOperation::Mul => format!("sf_mul({left_name}, {right_name})"),
                BinaryOperation::Div => format!("sf_div({left_name}, {right_name})"),
                BinaryOperation::Min if modes.is_native(PlanOp::Min) => {
                    native_call("min", &left_name, &right_name)
                }
                BinaryOperation::Max if modes.is_native(PlanOp::Max) => {
                    native_call("max", &left_name, &right_name)
                }
                BinaryOperation::Min => format!("sf_minimum({left_name}, {right_name})"),
                BinaryOperation::Max => format!("sf_maximum({left_name}, {right_name})"),
                BinaryOperation::CopySign => {
                    format!("({left_name} & {MAGNITUDE_MASK}) | ({right_name} & {SIGN_MASK})")
                }
            };
            format!("let {target}: u32 = {text};")
        }
        Expression::Compare(comparison, left, right) => {
            let (left_name, right_name) = (name(left), name(right));
            let text = match comparison {
                _ if modes.is_native(plan_comparison(comparison)) => {
                    native_comparison(comparison, &left_name, &right_name)
                }
                Comparison::Less => format!("sf_greater({right_name}, {left_name})"),
                Comparison::LessOrEqual => format!("sf_less_equal({left_name}, {right_name})"),
                Comparison::Greater => format!("sf_greater({left_name}, {right_name})"),
                Comparison::GreaterOrEqual => format!("sf_less_equal({right_name}, {left_name})"),
                Comparison::Equal => format!("sf_equal({left_name}, {right_name})"),
                Comparison::NotEqual => format!("!sf_equal({left_name}, {right_name})"),
            };
            format!("let {target}: bool = {text};")
        }
        Expression::And(left, right) => {
            format!("let {target}: bool = {} && {};", name(left), name(right))
        }
        Expression::Or(left, right) => {
            format!("let {target}: bool = {} || {};", name(left), name(right))
        }
        Expression::Not(argument) => format!("let {target}: bool = !{};", name(argument)),
        Expression::Select {
            condition,
            when_true,
            when_false,
        } => format!(
            "let {target}: u32 = select({}, {}, {});",
            name(when_false),
            name(when_true),
            name(condition)
        ),
    }
}

pub fn loop_text(statement: usize, iteration: &KernelIteration) -> String {
    loop_text_with(statement, iteration, OperationModes::EXACT)
}

pub fn loop_text_with(
    statement: usize,
    iteration: &KernelIteration,
    modes: OperationModes,
) -> String {
    let mut text = String::new();
    for (slot, initial) in iteration.initial.iter().enumerate() {
        let slot = u32::try_from(slot).unwrap_or(u32::MAX);
        let _ = writeln!(
            text,
            "    var {} = {};",
            state_name(statement, slot),
            name(*initial)
        );
    }
    let counter = format!("n{statement}");
    let _ = writeln!(
        text,
        "    for (var {counter}: u32 = 0u; {counter} < {}u; {counter} = {counter} + 1u) {{",
        iteration.maximum_count
    );
    let body = body_name(statement);
    for (index, expression) in iteration.body.iter().enumerate() {
        let target = body(LocalId(u32::try_from(index).unwrap_or(u32::MAX)));
        let line = match *expression {
            Expression::State { slot } => {
                format!("let {target} = {};", state_name(statement, slot))
            }
            Expression::Outer { local } => format!("let {target} = {};", name(local)),
            _ => expression_text(&target, expression, &body, modes),
        };
        let _ = writeln!(text, "        {line}");
    }
    let _ = writeln!(
        text,
        "        if ({}) {{\n            break;\n        }}",
        body(iteration.exit)
    );
    for (slot, next) in iteration.next.iter().enumerate() {
        let slot = u32::try_from(slot).unwrap_or(u32::MAX);
        let _ = writeln!(
            text,
            "        {} = {};",
            state_name(statement, slot),
            body(*next)
        );
    }
    text.push_str("    }\n");
    text
}

fn invocation_index(name: &str) -> String {
    format!(
        "    let {name} = id.x + id.y * {}u;\n    if ({name} >= size.invocations) {{\n        return;\n    }}\n",
        WORKGROUP_SIZE * WORKGROUPS_PER_DIMENSION
    )
}

pub fn emit_map(kernel: &Kernel) -> Result<String, EmitError> {
    emit_map_with(kernel, OperationModes::EXACT)
}

pub fn emit_map_with(kernel: &Kernel, modes: OperationModes) -> Result<String, EmitError> {
    validate(kernel).map_err(EmitError::Kernel)?;
    let mut text = String::from(crate::softfloat::SOFTFLOAT_WGSL);
    text.push_str(PRELUDE_TAIL);
    text.push_str(
        "\n@group(0) @binding(0) var<storage, read> inputs: array<u32>;\n\
         @group(0) @binding(1) var<storage, read> constants: array<u32>;\n\
         @group(0) @binding(2) var<storage, read_write> outputs: array<u32>;\n\
         @group(0) @binding(3) var<uniform> size: Size;\n\n",
    );
    let _ = writeln!(text, "@compute @workgroup_size({WORKGROUP_SIZE})");
    let _ = writeln!(
        text,
        "fn {MAP_ENTRY_POINT}(@builtin(global_invocation_id) id: vec3<u32>) {{"
    );
    text.push_str(&invocation_index("element"));
    for (index, expression) in kernel.map.statements.iter().enumerate() {
        match *expression {
            Expression::Iterate { iteration } => {
                if let Some(found) = usize::try_from(iteration)
                    .ok()
                    .and_then(|number| kernel.map.iterations.get(number))
                {
                    text.push_str(&loop_text_with(index, found, modes));
                }
            }
            _ => {
                let _ = writeln!(
                    text,
                    "    {}",
                    statement_text_with(index, expression, modes)
                );
            }
        }
    }
    for (channel, result) in kernel.map.results.iter().enumerate() {
        let _ = writeln!(
            text,
            "    outputs[{channel}u * size.element_count + element] = {};",
            name(*result)
        );
    }
    text.push_str("}\n");
    check_shader_text(&text).map_err(EmitError::Text)?;
    Ok(text)
}

pub fn combine_text(operation: ReduceOperation) -> &'static str {
    match operation {
        ReduceOperation::Add => "sf_add(left, right)",
        ReduceOperation::Mul => "sf_mul(left, right)",
        ReduceOperation::Min => "sf_minimum(left, right)",
        ReduceOperation::Max => "sf_maximum(left, right)",
    }
}

pub fn emit_reduce(kernel: &Kernel) -> Result<Option<String>, EmitError> {
    validate(kernel).map_err(EmitError::Kernel)?;
    let Some(reduce) = kernel.reduce else {
        return Ok(None);
    };
    let mut text = String::from(crate::softfloat::SOFTFLOAT_WGSL);
    text.push_str(PRELUDE_TAIL);
    text.push_str(
        "\n@group(0) @binding(0) var<storage, read> source: array<u32>;\n\
         @group(0) @binding(1) var<storage, read> triples: array<u32>;\n\
         @group(0) @binding(2) var<storage, read_write> destination: array<u32>;\n\
         @group(0) @binding(3) var<uniform> size: Size;\n\n",
    );
    let _ = writeln!(text, "@compute @workgroup_size({WORKGROUP_SIZE})");
    let _ = writeln!(
        text,
        "fn {REDUCE_ENTRY_POINT}(@builtin(global_invocation_id) id: vec3<u32>) {{"
    );
    text.push_str(&invocation_index("index"));
    let _ = write!(
        text,
        "    let left_position = triples[3u * index];\n\
         \x20   let right_position = triples[3u * index + 1u];\n\
         \x20   let result_position = triples[3u * index + 2u];\n\
         \x20   let left = source[left_position];\n\
         \x20   if (right_position == {}u) {{\n\
         \x20       destination[result_position] = left;\n\
         \x20       return;\n\
         \x20   }}\n\
         \x20   let right = source[right_position];\n\
         \x20   destination[result_position] = {};\n\
         }}\n",
        crate::kernel::CARRY,
        combine_text(reduce.operation)
    );
    check_shader_text(&text).map_err(EmitError::Text)?;
    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{
        BufferKind, CONSTANTS, INPUTS, MapPass, OUTPUTS, REDUCE_TARGET, ReducePass, TRIPLES,
    };

    const A: LocalId = LocalId(0);
    const B: LocalId = LocalId(1);
    const C: LocalId = LocalId(2);

    fn statement(expression: Expression) -> String {
        statement_text(3, &expression)
    }

    fn small_kernel(reduce: Option<ReduceOperation>) -> Kernel {
        Kernel {
            buffers: vec![
                BufferKind::Values,
                BufferKind::Values,
                BufferKind::Values,
                BufferKind::Values,
                BufferKind::Indices,
            ],
            constants: vec![2.0],
            map: MapPass {
                inputs: INPUTS,
                constants: CONSTANTS,
                outputs: OUTPUTS,
                input_channel_count: 1,
                statements: vec![
                    Expression::Input { channel: 0 },
                    Expression::Constant { index: 0 },
                    Expression::Binary(BinaryOperation::Mul, A, B),
                ],
                results: vec![C],
                iterations: Vec::new(),
            },
            reduce: reduce.map(|operation| ReducePass {
                operation,
                source: OUTPUTS,
                target: REDUCE_TARGET,
                triples: TRIPLES,
            }),
        }
    }

    #[test]
    fn every_shader_carries_the_soft_float_functions() {
        let text = emit_map(&small_kernel(None)).unwrap();

        assert!(text.starts_with(crate::softfloat::SOFTFLOAT_WGSL));
    }

    #[test]
    fn the_prelude_comparisons_are_false_for_nan() {
        let helpers = PRELUDE_TAIL.contains("fn sf_equal(a: u32, b: u32) -> bool {\n    return !sf_is_nan(a) && !sf_is_nan(b) && (a == b || ((a | b) & 0x7fffffffu) == 0u);\n}")
            && PRELUDE_TAIL.contains("fn sf_less_equal(a: u32, b: u32) -> bool {\n    return !sf_is_nan(a) && !sf_is_nan(b) && !sf_greater(a, b);\n}");

        assert!(helpers);
    }

    #[test]
    fn input_reads_its_channel_of_the_flat_buffer() {
        let text = statement(Expression::Input { channel: 2 });

        assert_eq!(
            text,
            "let v3: u32 = inputs[2u * size.element_count + element];"
        );
    }

    #[test]
    fn constant_is_read_from_the_constants_buffer() {
        let text = statement(Expression::Constant { index: 5 });

        assert_eq!(text, "let v3: u32 = constants[5u];");
    }

    #[test]
    fn negation_flips_the_sign_bit() {
        let text = statement(Expression::Unary(UnaryOperation::Neg, A));

        assert_eq!(text, "let v3: u32 = v0 ^ 0x80000000u;");
    }

    #[test]
    fn absolute_value_clears_the_sign_bit() {
        let text = statement(Expression::Unary(UnaryOperation::Abs, A));

        assert_eq!(text, "let v3: u32 = v0 & 0x7fffffffu;");
    }

    #[test]
    fn rounding_functions_call_the_soft_float() {
        let texts = [
            UnaryOperation::Sqrt,
            UnaryOperation::Floor,
            UnaryOperation::Ceil,
            UnaryOperation::Trunc,
            UnaryOperation::RoundTiesEven,
        ]
        .map(|operation| statement(Expression::Unary(operation, A)));

        assert_eq!(
            texts,
            [
                "let v3: u32 = sf_sqrt(v0);",
                "let v3: u32 = sf_floor(v0);",
                "let v3: u32 = sf_ceil(v0);",
                "let v3: u32 = sf_trunc(v0);",
                "let v3: u32 = sf_round_ties_even(v0);",
            ]
        );
    }

    #[test]
    fn arithmetic_calls_the_soft_float_in_operand_order() {
        let texts = [
            BinaryOperation::Add,
            BinaryOperation::Sub,
            BinaryOperation::Mul,
            BinaryOperation::Div,
        ]
        .map(|operation| statement(Expression::Binary(operation, B, A)));

        assert_eq!(
            texts,
            [
                "let v3: u32 = sf_add(v1, v0);",
                "let v3: u32 = sf_sub(v1, v0);",
                "let v3: u32 = sf_mul(v1, v0);",
                "let v3: u32 = sf_div(v1, v0);",
            ]
        );
    }

    #[test]
    fn minimum_and_maximum_call_the_ieee_helpers() {
        let texts = [BinaryOperation::Min, BinaryOperation::Max]
            .map(|operation| statement(Expression::Binary(operation, A, B)));

        assert_eq!(
            texts,
            [
                "let v3: u32 = sf_minimum(v0, v1);",
                "let v3: u32 = sf_maximum(v0, v1);"
            ]
        );
    }

    #[test]
    fn copy_sign_combines_bits() {
        let text = statement(Expression::Binary(BinaryOperation::CopySign, A, B));

        assert_eq!(
            text,
            "let v3: u32 = (v0 & 0x7fffffffu) | (v1 & 0x80000000u);"
        );
    }

    #[test]
    fn comparison_is_false_for_nan_without_relying_on_the_compiler() {
        let text = statement(Expression::Compare(Comparison::Less, A, B));

        assert_eq!(text, "let v3: bool = sf_greater(v1, v0);");
    }

    #[test]
    fn not_equal_is_true_for_nan() {
        let text = statement(Expression::Compare(Comparison::NotEqual, A, B));

        assert_eq!(text, "let v3: bool = !sf_equal(v0, v1);");
    }

    #[test]
    fn logic_uses_boolean_operators() {
        let texts = [
            statement(Expression::And(A, B)),
            statement(Expression::Or(A, B)),
            statement(Expression::Not(A)),
        ];

        assert_eq!(
            texts,
            [
                "let v3: bool = v0 && v1;",
                "let v3: bool = v0 || v1;",
                "let v3: bool = !v0;",
            ]
        );
    }

    #[test]
    fn select_passes_false_value_first() {
        let text = statement(Expression::Select {
            condition: C,
            when_true: A,
            when_false: B,
        });

        assert_eq!(text, "let v3: u32 = select(v1, v0, v2);");
    }

    #[test]
    fn map_shader_writes_each_result_into_its_channel() {
        let text = emit_map(&small_kernel(None)).unwrap();

        assert!(text.ends_with(
            "    let v2: u32 = sf_mul(v0, v1);\n    outputs[0u * size.element_count + element] = v2;\n}\n"
        ));
    }

    #[test]
    fn kernel_without_reduce_has_no_reduce_shader() {
        let text = emit_reduce(&small_kernel(None)).unwrap();

        assert_eq!(text, None);
    }

    #[test]
    fn reduce_shader_combines_with_the_named_operation() {
        let text = emit_reduce(&small_kernel(Some(ReduceOperation::Min)))
            .unwrap()
            .unwrap();

        assert!(text.contains("    destination[result_position] = sf_minimum(left, right);\n"));
    }

    fn every<T: Copy + PartialEq + std::fmt::Debug>(first: T, after: fn(T) -> Option<T>) -> Vec<T> {
        let mut found = vec![first];
        while let Some(next) = after(*found.last().unwrap_or(&first)) {
            assert!(
                !found.contains(&next),
                "{next:?} appears twice in the chain"
            );
            found.push(next);
        }
        found
    }

    fn after_binary(operation: BinaryOperation) -> Option<BinaryOperation> {
        match operation {
            BinaryOperation::Add => Some(BinaryOperation::Sub),
            BinaryOperation::Sub => Some(BinaryOperation::Mul),
            BinaryOperation::Mul => Some(BinaryOperation::Div),
            BinaryOperation::Div => Some(BinaryOperation::Min),
            BinaryOperation::Min => Some(BinaryOperation::Max),
            BinaryOperation::Max => Some(BinaryOperation::CopySign),
            BinaryOperation::CopySign => None,
        }
    }

    fn after_unary(operation: UnaryOperation) -> Option<UnaryOperation> {
        match operation {
            UnaryOperation::Sqrt => Some(UnaryOperation::Neg),
            UnaryOperation::Neg => Some(UnaryOperation::Abs),
            UnaryOperation::Abs => Some(UnaryOperation::Floor),
            UnaryOperation::Floor => Some(UnaryOperation::Ceil),
            UnaryOperation::Ceil => Some(UnaryOperation::Trunc),
            UnaryOperation::Trunc => Some(UnaryOperation::RoundTiesEven),
            UnaryOperation::RoundTiesEven => None,
        }
    }

    fn after_comparison(comparison: Comparison) -> Option<Comparison> {
        match comparison {
            Comparison::Less => Some(Comparison::LessOrEqual),
            Comparison::LessOrEqual => Some(Comparison::Greater),
            Comparison::Greater => Some(Comparison::GreaterOrEqual),
            Comparison::GreaterOrEqual => Some(Comparison::Equal),
            Comparison::Equal => Some(Comparison::NotEqual),
            Comparison::NotEqual => None,
        }
    }

    fn called_soft_float_names(text: &str) -> Vec<String> {
        let mut found = Vec::new();
        for (offset, _) in text.match_indices("sf_") {
            let rest = &text[offset..];
            let length = rest
                .find(|character: char| !is_identifier_character(character))
                .unwrap_or(rest.len());
            if rest[length..].starts_with('(') {
                found.push(rest[..length].to_string());
            }
        }
        found
    }

    #[test]
    fn every_soft_float_function_the_emitter_calls_is_defined() {
        let mut kernel = small_kernel(Some(ReduceOperation::Min));
        for operation in every(BinaryOperation::Add, after_binary) {
            kernel
                .map
                .statements
                .push(Expression::Binary(operation, A, B));
        }
        for operation in every(UnaryOperation::Sqrt, after_unary) {
            kernel.map.statements.push(Expression::Unary(operation, A));
        }
        for comparison in every(Comparison::Less, after_comparison) {
            kernel
                .map
                .statements
                .push(Expression::Compare(comparison, A, B));
        }
        let texts = [
            emit_map(&kernel).unwrap(),
            emit_reduce(&kernel).unwrap().unwrap(),
        ];

        for text in texts {
            for called in called_soft_float_names(&text) {
                assert!(
                    text.contains(&format!("fn {called}(")),
                    "{called} is called and not defined"
                );
            }
        }
    }

    #[test]
    fn invalid_kernel_is_not_emitted() {
        let mut kernel = small_kernel(None);
        kernel.map.results.clear();

        let text = emit_map(&kernel);

        assert_eq!(text, Err(EmitError::Kernel(KernelError::NoResults)));
    }

    #[test]
    fn half_precision_enable_is_rejected() {
        let text = "enable f16;\n@compute @workgroup_size(1) fn main() {}";

        let checked = check_shader_text(text);

        assert_eq!(checked, Err(ShaderTextError::HalfPrecision { offset: 7 }));
    }

    #[test]
    fn half_precision_type_is_rejected() {
        let text = "fn main() { let value: f16 = 1.0f; }";

        let checked = check_shader_text(text);

        assert_eq!(checked, Err(ShaderTextError::HalfPrecision { offset: 23 }));
    }

    #[test]
    fn identifier_containing_f16_is_accepted() {
        let text = "fn main() { let buffer_f16x: f32 = 1.0; }";

        let checked = check_shader_text(text);

        assert_eq!(checked, Ok(()));
    }

    #[test]
    fn emitted_shaders_contain_no_half_precision() {
        let kernel = small_kernel(Some(ReduceOperation::Add));

        let texts = [
            emit_map(&kernel).unwrap(),
            emit_reduce(&kernel).unwrap().unwrap(),
        ];

        assert!(texts.iter().all(|text| check_shader_text(text).is_ok()));
    }

    #[test]
    fn iteration_becomes_a_fixed_count_loop_that_breaks_at_the_exit() {
        let iteration = KernelIteration {
            initial: vec![A],
            body: vec![
                Expression::State { slot: 0 },
                Expression::Outer { local: B },
                Expression::Binary(BinaryOperation::Mul, LocalId(0), LocalId(1)),
                Expression::Compare(Comparison::Greater, LocalId(0), LocalId(1)),
            ],
            next: vec![LocalId(2)],
            exit: LocalId(3),
            maximum_count: 256,
        };

        let text = loop_text(4, &iteration);

        assert_eq!(
            text,
            "    var s4_0 = v0;\n\
             \x20   for (var n4: u32 = 0u; n4 < 256u; n4 = n4 + 1u) {\n\
             \x20       let b4_0 = s4_0;\n\
             \x20       let b4_1 = v1;\n\
             \x20       let b4_2: u32 = sf_mul(b4_0, b4_1);\n\
             \x20       let b4_3: bool = sf_greater(b4_0, b4_1);\n\
             \x20       if (b4_3) {\n\
             \x20           break;\n\
             \x20       }\n\
             \x20       s4_0 = b4_2;\n\
             \x20   }\n"
        );
    }

    #[test]
    fn iteration_state_reads_the_loop_variable() {
        let text = statement(Expression::IterationState {
            iteration: LocalId(4),
            slot: 2,
        });

        assert_eq!(text, "let v3 = s4_2;");
    }
}
