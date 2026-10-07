use calc_exec::{ExecutionMode, PlanOp};
use calc_i18n::Message;

use crate::result_record::OperationModeUse;
use crate::session_file::operation_name;

const MODE_ORDER: [ExecutionMode; 3] = [
    ExecutionMode::Native,
    ExecutionMode::IntegerExact,
    ExecutionMode::ExactInBothForms,
];

pub const fn mode_label(mode: ExecutionMode) -> Message {
    match mode {
        ExecutionMode::Native => Message::CommonRecordModesNative,
        ExecutionMode::IntegerExact => Message::CommonRecordModesIntegerExact,
        ExecutionMode::ExactInBothForms => Message::CommonRecordModesExactInBothForms,
    }
}

pub fn operations_of(modes: &[OperationModeUse], mode: ExecutionMode) -> Vec<&'static str> {
    PlanOp::ALL
        .into_iter()
        .filter(|operation| {
            modes
                .iter()
                .any(|used| used.operation == *operation && used.mode == mode)
        })
        .map(operation_name)
        .collect()
}

pub const fn every_operation_message(mode: ExecutionMode) -> Message {
    match mode {
        ExecutionMode::Native => Message::CommonRecordModesAllNative,
        ExecutionMode::IntegerExact => Message::CommonRecordModesAllIntegerExact,
        ExecutionMode::ExactInBothForms => Message::CommonRecordModesAllExactInBothForms,
    }
}
pub const fn mode_count_message(mode: ExecutionMode, count: u64) -> Message {
    match mode {
        ExecutionMode::Native => Message::CommonRecordModesNativeCount { count },
        ExecutionMode::IntegerExact => Message::CommonRecordModesIntegerExactCount { count },
        ExecutionMode::ExactInBothForms => {
            Message::CommonRecordModesExactInBothFormsCount { count }
        }
    }
}
pub fn modes_summary(modes: &[OperationModeUse]) -> Vec<Message> {
    let counted: Vec<(ExecutionMode, u64)> = MODE_ORDER
        .into_iter()
        .map(|mode| {
            let count = modes.iter().filter(|used| used.mode == mode).count();
            (mode, u64::try_from(count).unwrap_or(u64::MAX))
        })
        .filter(|(_, count)| *count > 0)
        .collect();
    match counted.as_slice() {
        [(mode, _)] => vec![every_operation_message(*mode)],
        counted => counted
            .iter()
            .map(|(mode, count)| mode_count_message(*mode, *count))
            .collect(),
    }
}
pub fn modes_block(modes: &[OperationModeUse]) -> Vec<(Message, Vec<&'static str>)> {
    let mut rows: Vec<(Message, Vec<&'static str>)> = MODE_ORDER
        .into_iter()
        .filter_map(|mode| {
            let operations = operations_of(modes, mode);
            (!operations.is_empty()).then(|| (mode_label(mode), operations))
        })
        .collect();
    let not_offered: Vec<&'static str> = PlanOp::ALL
        .into_iter()
        .filter(|operation| !modes.iter().any(|used| used.operation == *operation))
        .map(operation_name)
        .collect();
    if !not_offered.is_empty() {
        rows.push((Message::CommonRecordModesNotOffered, not_offered));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn used(operation: PlanOp, mode: ExecutionMode) -> OperationModeUse {
        OperationModeUse { operation, mode }
    }

    #[test]
    fn a_mode_with_no_operation_has_no_row() {
        let rows = modes_block(&[used(PlanOp::Add, ExecutionMode::Native)]);

        assert!(
            !rows
                .iter()
                .any(|(label, _)| *label == Message::CommonRecordModesIntegerExact)
        );
    }

    #[test]
    fn the_rows_keep_the_order_of_the_modes() {
        let rows = modes_block(&[
            used(PlanOp::Mul, ExecutionMode::IntegerExact),
            used(PlanOp::Add, ExecutionMode::Native),
        ]);

        assert_eq!(
            rows.iter().map(|(label, _)| label.clone()).next(),
            Some(Message::CommonRecordModesNative)
        );
    }

    #[test]
    fn an_operation_no_mode_names_is_not_offered() {
        let rows = modes_block(&[used(PlanOp::Add, ExecutionMode::Native)]);
        let not_offered = rows
            .iter()
            .find(|(label, _)| *label == Message::CommonRecordModesNotOffered)
            .map(|(_, operations)| operations.clone())
            .expect("the not offered row is there");

        assert_eq!(not_offered.len(), PlanOp::ALL.len() - 1);
    }

    #[test]
    fn everything_offered_leaves_out_the_not_offered_row() {
        let modes: Vec<OperationModeUse> = PlanOp::ALL
            .into_iter()
            .map(|operation| used(operation, ExecutionMode::Native))
            .collect();

        let rows = modes_block(&modes);

        assert!(
            !rows
                .iter()
                .any(|(label, _)| *label == Message::CommonRecordModesNotOffered)
        );
    }
}
