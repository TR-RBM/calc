use calc_expr::{BinderKind, ExprId, ExprPool, Head, NodeView, Operator, SortMethod, SymbolKind};
use calc_numbers::{Integer, Number};

use crate::exact_evaluation::{closed_square_root_sum, square_root_sum_expression};
use std::cmp::Ordering;

use crate::exact_rational::ExactRational;
use crate::real_roots::every_root_is_real;
use crate::solve::roots_of_coefficients;
use crate::square_root_sum::SquareRootSum;

const EIGENVALUE_NAME: &str = "t";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayExpression {
    pub shape: Vec<u32>,
    pub elements: Vec<ExprId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayOperation {
    Transpose,
    Determinant,
    Trace,
    Inverse,
    Rank,
    Eigenvalues,
    Kernel,
    Eigenvectors,
}

impl ArrayOperation {
    pub fn name(self) -> &'static str {
        match self {
            ArrayOperation::Transpose => "transpose",
            ArrayOperation::Determinant => "determinant",
            ArrayOperation::Trace => "trace",
            ArrayOperation::Inverse => "inverse",
            ArrayOperation::Rank => "rank",
            ArrayOperation::Eigenvalues => "eigenvalues",
            ArrayOperation::Kernel => "kernel",
            ArrayOperation::Eigenvectors => "eigenvectors",
        }
    }
}

fn matrix_refusal(array: &ArrayExpression, operation: ArrayOperation) -> ArrayError {
    match array.shape[..] {
        [0] => ArrayError::Empty(operation),
        _ => ArrayError::NotAMatrix(operation),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayError {
    ShapesDiffer,
    ProductOfLists,
    NotRectangular,
    NotSquare,
    NotAMatrix(ArrayOperation),
    Empty(ArrayOperation),
    EmptyList,
    PositionNotWhole,
    PositionOutsideList,
    NotComparable(ExprId, ExprId),
    NotReal(ExprId),
    EntryWithoutValue(ExprId),
    KeyNotWhole(ExprId, SortMethod),
    FunctionKeyNotWhole(ExprId, SortMethod),
    KeyWithUnit(ExprId, SortMethod),
    SortRefused(crate::sorting::SortRefusal),
    RangeTooWide,
    GeneratorRefused(crate::generator::GeneratorRefusal),
    RecordsNeedKey,
    NotAList,
    UnitsDiffer,
    RanksDiffer,
    Nested,
    BodyAndPointAreArrays,
    PowerOfArray,
    NotInvertible,
    EntriesNotExact,
    EntriesNotRational,
    EigenvaluesNotReal,
    EigenvaluesNotRadical,
    Unsupported,
}

use crate::sorting::{OrderedEntry, proven_order};

type Shaped<T> = Result<T, ArrayError>;

fn cell_count(shape: &[u32]) -> usize {
    shape
        .iter()
        .map(|length| *length as usize)
        .product::<usize>()
}

fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> Shaped<ExprId> {
    pool.apply(Head::Operator(operator), arguments)
        .map_err(|_| ArrayError::Unsupported)
}

fn matrix_product(
    pool: &mut ExprPool,
    left: &ArrayExpression,
    right: &ArrayExpression,
) -> Shaped<ArrayExpression> {
    let (rows, inner) = (left.shape[0] as usize, left.shape[1] as usize);
    let (other, columns) = (right.shape[0] as usize, right.shape[1] as usize);
    if inner != other {
        return Err(ArrayError::ShapesDiffer);
    }
    let mut elements = Vec::with_capacity(rows * columns);
    for row in 0..rows {
        for column in 0..columns {
            let mut total: Option<ExprId> = None;
            for step in 0..inner {
                let first = left.elements[row * inner + step];
                let second = right.elements[step * columns + column];
                let term = apply(pool, Operator::Mul, &[first, second])?;
                total = Some(match total {
                    Some(running) => apply(pool, Operator::Add, &[running, term])?,
                    None => term,
                });
            }
            elements.push(total.ok_or(ArrayError::ShapesDiffer)?);
        }
    }
    Ok(ArrayExpression {
        shape: vec![left.shape[0], right.shape[1]],
        elements,
    })
}

fn elementwise(
    pool: &mut ExprPool,
    operator: Operator,
    left: &ArrayExpression,
    right: &ArrayExpression,
) -> Shaped<ArrayExpression> {
    if left.shape != right.shape {
        return Err(ArrayError::ShapesDiffer);
    }
    let mut elements = Vec::with_capacity(left.elements.len());
    for (first, second) in left.elements.iter().zip(&right.elements) {
        elements.push(apply(pool, operator, &[*first, *second])?);
    }
    Ok(ArrayExpression {
        shape: left.shape.clone(),
        elements,
    })
}

fn scaled(
    pool: &mut ExprPool,
    operator: Operator,
    array: &ArrayExpression,
    scalar: ExprId,
    scalar_is_left: bool,
) -> Shaped<ArrayExpression> {
    let mut elements = Vec::with_capacity(array.elements.len());
    for element in &array.elements {
        let arguments = if scalar_is_left {
            [scalar, *element]
        } else {
            [*element, scalar]
        };
        elements.push(apply(pool, operator, &arguments)?);
    }
    Ok(ArrayExpression {
        shape: array.shape.clone(),
        elements,
    })
}

pub fn array_expression(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<Shaped<ArrayExpression>> {
    let view = pool.node(expression).ok()?;
    match view {
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            if cell_count(&shape) != elements.len() {
                return Some(Err(ArrayError::NotRectangular));
            }
            if elements
                .iter()
                .any(|element| matches!(pool.node(*element), Ok(NodeView::Array { .. })))
            {
                return Some(Err(ArrayError::Nested));
            }
            Some(Ok(ArrayExpression { shape, elements }))
        }
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => {
            let arguments = arguments.to_vec();
            combined(pool, operator, &arguments)
        }
        NodeView::Bind {
            binder: BinderKind::Sort(_),
            ..
        } => crate::sorting::named_sort(pool, expression, calc_sort::StepRecording::Skip).map(
            |outcome| {
                outcome
                    .map(|(array, _)| array)
                    .map_err(|failure| failure.to_array_error())
            },
        ),
        _ => None,
    }
}

fn generated_list(
    pool: &mut ExprPool,
    seed: ExprId,
    stream: ExprId,
    index: ExprId,
) -> Shaped<ArrayExpression> {
    let block = crate::generator::generated(pool, seed, stream, index)
        .map_err(ArrayError::GeneratorRefused)?;
    let mut elements = Vec::with_capacity(block.words.len());
    for word in block.words {
        let element = pool
            .number(Number::Integer(Integer::from(u64::from(word))))
            .map_err(|_| ArrayError::Unsupported)?;
        elements.push(element);
    }
    Ok(ArrayExpression {
        shape: vec![4],
        elements,
    })
}

fn combined(
    pool: &mut ExprPool,
    operator: Operator,
    arguments: &[ExprId],
) -> Option<Shaped<ArrayExpression>> {
    match (operator, arguments) {
        (Operator::Neg, [single]) => {
            let single = array_expression(pool, *single)?;
            Some(single.and_then(|array| {
                let mut elements = Vec::with_capacity(array.elements.len());
                for element in &array.elements {
                    elements.push(apply(pool, Operator::Neg, &[*element])?);
                }
                Ok(ArrayExpression {
                    shape: array.shape,
                    elements,
                })
            }))
        }
        (Operator::Pow, [base, _]) => {
            let base = array_expression(pool, *base)?;
            Some(match base {
                Ok(_) => Err(ArrayError::PowerOfArray),
                Err(error) => Err(error),
            })
        }
        (Operator::Sorted, [single]) => {
            let single = array_expression(pool, *single)?;
            Some(single.and_then(|array| {
                if array.elements.is_empty() {
                    return Ok(array);
                }
                let sorted = ordered(pool, &array)?;
                Ok(ArrayExpression {
                    shape: vec![sorted.len() as u32],
                    elements: sorted.iter().map(|entry| entry.expression).collect(),
                })
            }))
        }
        (Operator::Philox4x32_10, [seed, stream, index]) => {
            let (seed, stream, index) = (*seed, *stream, *index);
            Some(generated_list(pool, seed, stream, index))
        }
        (Operator::Transpose, [single]) => {
            let single = array_expression(pool, *single)?;
            Some(single.and_then(|array| transposed(&array)))
        }
        (Operator::Inverse, [single]) => {
            let single = array_expression(pool, *single)?;
            Some(single.and_then(|array| inverse_of(pool, &array)))
        }
        (Operator::Eigenvalues, [single]) => {
            let single = array_expression(pool, *single)?;
            Some(single.and_then(|array| eigenvalues_of(pool, &array)))
        }
        (Operator::Kernel, [single]) => {
            let single = array_expression(pool, *single)?;
            Some(single.and_then(|array| kernel_of(pool, &array)))
        }
        (Operator::Eigenvectors, [single]) => {
            let single = array_expression(pool, *single)?;
            Some(single.and_then(|array| eigenvectors_of(pool, &array)))
        }
        (Operator::Add | Operator::Sub | Operator::Mul | Operator::Div, [left, right]) => {
            let (left, right) = (*left, *right);
            let left_array = array_expression(pool, left);
            let right_array = array_expression(pool, right);
            match (left_array, right_array) {
                (None, None) => None,
                (Some(Err(error)), _) | (_, Some(Err(error))) => Some(Err(error)),
                (Some(Ok(left_array)), Some(Ok(right_array))) => {
                    Some(both(pool, operator, &left_array, &right_array))
                }
                (Some(Ok(array)), None) => Some(scaled(pool, operator, &array, right, false)),
                (None, Some(Ok(array))) => match operator {
                    Operator::Add | Operator::Sub | Operator::Mul => {
                        Some(scaled(pool, operator, &array, left, true))
                    }
                    _ => Some(Err(ArrayError::Unsupported)),
                },
            }
        }
        _ => None,
    }
}

fn both(
    pool: &mut ExprPool,
    operator: Operator,
    left: &ArrayExpression,
    right: &ArrayExpression,
) -> Shaped<ArrayExpression> {
    match operator {
        Operator::Add | Operator::Sub => elementwise(pool, operator, left, right),
        Operator::Mul if left.shape.len() == 2 && right.shape.len() == 2 => {
            matrix_product(pool, left, right)
        }
        Operator::Mul if left.shape.len() != right.shape.len() => Err(ArrayError::RanksDiffer),
        Operator::Mul => Err(ArrayError::ProductOfLists),
        _ => Err(ArrayError::Unsupported),
    }
}

fn transposed(array: &ArrayExpression) -> Shaped<ArrayExpression> {
    let [rows, columns] = array.shape[..] else {
        return Err(matrix_refusal(array, ArrayOperation::Transpose));
    };
    let (rows, columns) = (rows as usize, columns as usize);
    let mut elements = Vec::with_capacity(array.elements.len());
    for column in 0..columns {
        for row in 0..rows {
            elements.push(array.elements[row * columns + column]);
        }
    }
    Ok(ArrayExpression {
        shape: vec![array.shape[1], array.shape[0]],
        elements,
    })
}

fn square_order(array: &ArrayExpression, operation: ArrayOperation) -> Shaped<usize> {
    let [rows, columns] = array.shape[..] else {
        return Err(matrix_refusal(array, operation));
    };
    if rows != columns {
        return Err(ArrayError::NotSquare);
    }
    Ok(rows as usize)
}

fn exact_entries(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<Vec<SquareRootSum>> {
    array
        .elements
        .iter()
        .map(|element| closed_square_root_sum(pool, *element).ok_or(ArrayError::EntriesNotExact))
        .collect()
}

fn trace_of(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<ExprId> {
    let order = square_order(array, ArrayOperation::Trace)?;
    let mut total: Option<ExprId> = None;
    for position in 0..order {
        let entry = array.elements[position * order + position];
        total = Some(match total {
            Some(running) => apply(pool, Operator::Add, &[running, entry])?,
            None => entry,
        });
    }
    total.ok_or(ArrayError::Empty(ArrayOperation::Trace))
}

fn row_reduced(rows: &mut [Vec<SquareRootSum>], width: usize) -> Option<usize> {
    let mut rank = 0;
    for column in 0..width {
        let Some(found) = (rank..rows.len()).find(|row| !rows[*row][column].is_zero()) else {
            continue;
        };
        rows.swap(rank, found);
        let scale = rows[rank][column].reciprocal()?;
        for entry in rows[rank].iter_mut() {
            *entry = entry.times(&scale);
        }
        let pivot_row = rows[rank].clone();
        for (other, row) in rows.iter_mut().enumerate() {
            if other == rank || row[column].is_zero() {
                continue;
            }
            let factor = row[column].clone();
            for (entry, pivot_entry) in row.iter_mut().zip(&pivot_row) {
                *entry = entry.minus(&pivot_entry.times(&factor));
            }
        }
        rank += 1;
        if rank == rows.len() {
            break;
        }
    }
    Some(rank)
}

fn rank_of(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<ExprId> {
    let [rows, columns] = array.shape[..] else {
        return Err(matrix_refusal(array, ArrayOperation::Rank));
    };
    let (rows, columns) = (rows as usize, columns as usize);
    let entries = exact_entries(pool, array)?;
    let mut matrix: Vec<Vec<SquareRootSum>> =
        entries.chunks(columns.max(1)).map(<[_]>::to_vec).collect();
    matrix.truncate(rows);
    let rank = row_reduced(&mut matrix, columns).ok_or(ArrayError::Unsupported)?;
    pool.number(Integer::from(i64::try_from(rank).unwrap_or(i64::MAX)).into())
        .map_err(|_| ArrayError::Unsupported)
}

fn kernel_vectors(
    mut matrix: Vec<Vec<SquareRootSum>>,
    columns: usize,
) -> Option<Vec<Vec<SquareRootSum>>> {
    row_reduced(&mut matrix, columns)?;
    let mut pivots: Vec<(usize, usize)> = Vec::new();
    for (row, line) in matrix.iter().enumerate() {
        if let Some(column) = line.iter().position(|entry| !entry.is_zero()) {
            pivots.push((row, column));
        }
    }
    let free: Vec<usize> = (0..columns)
        .filter(|column| !pivots.iter().any(|(_, pivot)| pivot == column))
        .collect();
    let mut vectors = Vec::with_capacity(free.len());
    for chosen in &free {
        let mut vector = vec![SquareRootSum::zero(); columns];
        vector[*chosen] = SquareRootSum::from_rational(ExactRational::one());
        for (row, column) in &pivots {
            vector[*column] = matrix[*row][*chosen].negated();
        }
        vectors.push(vector);
    }
    Some(vectors)
}

fn columns_of(
    pool: &mut ExprPool,
    vectors: &[Vec<SquareRootSum>],
    length: usize,
) -> Shaped<ArrayExpression> {
    if vectors.is_empty() {
        return Ok(ArrayExpression {
            shape: vec![0],
            elements: Vec::new(),
        });
    }
    let mut elements = Vec::with_capacity(length * vectors.len());
    for position in 0..length {
        for vector in vectors {
            elements.push(
                square_root_sum_expression(pool, &vector[position])
                    .map_err(|_| ArrayError::Unsupported)?,
            );
        }
    }
    Ok(ArrayExpression {
        shape: vec![
            u32::try_from(length).map_err(|_| ArrayError::Unsupported)?,
            u32::try_from(vectors.len()).map_err(|_| ArrayError::Unsupported)?,
        ],
        elements,
    })
}

fn kernel_of(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<ArrayExpression> {
    let [rows, columns] = array.shape[..] else {
        return Err(matrix_refusal(array, ArrayOperation::Kernel));
    };
    let (rows, columns) = (rows as usize, columns as usize);
    let entries = exact_entries(pool, array)?;
    let matrix: Vec<Vec<SquareRootSum>> = entries
        .chunks(columns.max(1))
        .take(rows)
        .map(<[_]>::to_vec)
        .collect();
    let vectors = kernel_vectors(matrix, columns).ok_or(ArrayError::Unsupported)?;
    columns_of(pool, &vectors, columns)
}

fn eigenvectors_of(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<ArrayExpression> {
    let order = square_order(array, ArrayOperation::Eigenvectors)?;
    let entries = exact_entries(pool, array)?;
    let values = eigenvalue_expressions(pool, array)?;
    let mut vectors = Vec::new();
    for value in values {
        let value = closed_square_root_sum(pool, value).ok_or(ArrayError::EigenvaluesNotRadical)?;
        let shifted: Vec<Vec<SquareRootSum>> = (0..order)
            .map(|row| {
                (0..order)
                    .map(|column| {
                        let entry = entries[row * order + column].clone();
                        if row == column {
                            entry.minus(&value)
                        } else {
                            entry
                        }
                    })
                    .collect()
            })
            .collect();
        vectors.extend(kernel_vectors(shifted, order).ok_or(ArrayError::Unsupported)?);
    }
    columns_of(pool, &vectors, order)
}

fn inverse_of(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<ArrayExpression> {
    let order = square_order(array, ArrayOperation::Inverse)?;
    let entries = exact_entries(pool, array)?;
    let mut augmented: Vec<Vec<SquareRootSum>> = (0..order)
        .map(|row| {
            let mut line = entries[row * order..(row + 1) * order].to_vec();
            for column in 0..order {
                line.push(SquareRootSum::from_rational(if column == row {
                    ExactRational::one()
                } else {
                    ExactRational::zero()
                }));
            }
            line
        })
        .collect();
    let rank = row_reduced(&mut augmented, order).ok_or(ArrayError::Unsupported)?;
    if rank < order {
        return Err(ArrayError::NotInvertible);
    }
    let mut elements = Vec::with_capacity(order * order);
    for line in &augmented {
        for value in &line[order..] {
            elements.push(
                square_root_sum_expression(pool, value).map_err(|_| ArrayError::Unsupported)?,
            );
        }
    }
    Ok(ArrayExpression {
        shape: array.shape.clone(),
        elements,
    })
}

fn eigenvalues_of(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<ArrayExpression> {
    let roots = eigenvalue_expressions(pool, array)?;
    Ok(ArrayExpression {
        shape: vec![u32::try_from(roots.len()).map_err(|_| ArrayError::Unsupported)?],
        elements: roots,
    })
}

fn eigenvalue_expressions(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<Vec<ExprId>> {
    let order = square_order(array, ArrayOperation::Eigenvalues)?;
    let entries: Vec<ExactRational> = exact_entries(pool, array)?
        .iter()
        .map(|entry| entry.as_rational().ok_or(ArrayError::EntriesNotRational))
        .collect::<Shaped<_>>()?;
    let at =
        |matrix: &[ExactRational], row: usize, column: usize| matrix[row * order + column].clone();
    let mut coefficients = vec![ExactRational::zero(); order + 1];
    coefficients[order] = ExactRational::one();
    let mut running = vec![ExactRational::zero(); order * order];
    for step in 1..=order {
        let mut product = vec![ExactRational::zero(); order * order];
        for row in 0..order {
            for column in 0..order {
                let mut total = ExactRational::zero();
                for inner in 0..order {
                    total = total
                        .plus(&at(&entries, row, inner).multiply(&at(&running, inner, column)));
                }
                if row == column {
                    total = total.plus(&coefficients[order + 1 - step]);
                }
                product[row * order + column] = total;
            }
        }
        running = product;
        let mut trace = ExactRational::zero();
        for row in 0..order {
            for inner in 0..order {
                trace = trace.plus(&at(&entries, row, inner).multiply(&at(&running, inner, row)));
            }
        }
        let divisor = ExactRational::from_i64(i64::try_from(step).unwrap_or(i64::MAX));
        coefficients[order - step] = trace
            .negated()
            .divide(&divisor)
            .ok_or(ArrayError::Unsupported)?;
    }
    if !every_root_is_real(&coefficients).ok_or(ArrayError::Unsupported)? {
        return Err(ArrayError::EigenvaluesNotReal);
    }
    let unknown = pool
        .intern_symbol(EIGENVALUE_NAME, SymbolKind::Variable)
        .map_err(|_| ArrayError::Unsupported)?;
    let coefficients = coefficients
        .into_iter()
        .map(SquareRootSum::from_rational)
        .collect();
    roots_of_coefficients(pool, coefficients, unknown).map_err(|_| ArrayError::Unsupported)
}

fn determinant_of(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<ExprId> {
    let [rows, columns] = array.shape[..] else {
        return Err(matrix_refusal(array, ArrayOperation::Determinant));
    };
    if rows != columns {
        return Err(ArrayError::NotSquare);
    }
    let order = rows as usize;
    if order == 1 {
        return Ok(array.elements[0]);
    }
    let mut total: Option<ExprId> = None;
    for column in 0..order {
        let minor = ArrayExpression {
            shape: vec![rows - 1, columns - 1],
            elements: (1..order)
                .flat_map(|row| {
                    (0..order)
                        .filter(|other| *other != column)
                        .map(move |other| (row, other))
                })
                .map(|(row, other)| array.elements[row * order + other])
                .collect(),
        };
        let cofactor = determinant_of(pool, &minor)?;
        let term = apply(pool, Operator::Mul, &[array.elements[column], cofactor])?;
        let operator = if column % 2 == 0 {
            Operator::Add
        } else {
            Operator::Sub
        };
        total = Some(match total {
            Some(running) => apply(pool, operator, &[running, term])?,
            None => {
                if column % 2 == 0 {
                    term
                } else {
                    apply(pool, Operator::Neg, &[term])?
                }
            }
        });
    }
    total.ok_or(ArrayError::Unsupported)
}

fn entry_at(pool: &ExprPool, array: &ArrayExpression, index: ExprId) -> Shaped<ExprId> {
    let position = match pool.node(index).map_err(|_| ArrayError::Unsupported)? {
        NodeView::Number(number) => match pool
            .number_value(number)
            .map_err(|_| ArrayError::Unsupported)?
        {
            Number::Integer(value) => value.to_i64(),
            _ => None,
        },
        _ => None,
    };
    let position = position.ok_or(ArrayError::PositionNotWhole)?;
    if position < 1 {
        return Err(ArrayError::PositionOutsideList);
    }
    array
        .elements
        .get((position - 1) as usize)
        .copied()
        .ok_or(ArrayError::PositionOutsideList)
}

fn folded(pool: &mut ExprPool, operator: Operator, array: &ArrayExpression) -> Shaped<ExprId> {
    let mut total: Option<ExprId> = None;
    for element in &array.elements {
        total = Some(match total {
            Some(running) => apply(pool, operator, &[running, *element])?,
            None => *element,
        });
    }
    total.ok_or(ArrayError::EmptyList)
}

fn merged(
    pool: &mut ExprPool,
    left: Vec<OrderedEntry>,
    right: Vec<OrderedEntry>,
) -> Shaped<Vec<OrderedEntry>> {
    let mut result = Vec::with_capacity(left.len() + right.len());
    let mut left = left.into_iter().peekable();
    let mut right = right.into_iter().peekable();
    while let (Some(first), Some(second)) = (left.peek(), right.peek()) {
        let take_right = proven_order(pool, first, second)? == Ordering::Greater;
        let next = if take_right {
            right.next()
        } else {
            left.next()
        };
        result.extend(next);
    }
    result.extend(left);
    result.extend(right);
    Ok(result)
}

fn merge_sorted(pool: &mut ExprPool, mut entries: Vec<OrderedEntry>) -> Shaped<Vec<OrderedEntry>> {
    if entries.len() < 2 {
        return Ok(entries);
    }
    let second_half = entries.split_off(entries.len() / 2);
    let first = merge_sorted(pool, entries)?;
    let second = merge_sorted(pool, second_half)?;
    merged(pool, first, second)
}

fn ordered(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<Vec<OrderedEntry>> {
    if array.elements.is_empty() {
        return Err(ArrayError::EmptyList);
    }
    let entries = array
        .elements
        .iter()
        .map(|element| crate::sorting::real_entry(pool, *element))
        .collect::<Shaped<Vec<_>>>()?;
    if let Some((first, second)) = crate::sorting::first_dimension_clash(pool, &entries) {
        return Err(ArrayError::SortRefused(
            crate::sorting::SortRefusal::KeysDifferInDimension {
                first,
                second,
                first_key: entries[first].expression,
                second_key: entries[second].expression,
                by_key: false,
            },
        ));
    }
    merge_sorted(pool, entries)
}

fn median_of(pool: &mut ExprPool, array: &ArrayExpression) -> Shaped<ExprId> {
    if array.elements.is_empty() {
        return Err(ArrayError::EmptyList);
    }
    let sorted = ordered(pool, array)?;
    let count = sorted.len();
    if count % 2 == 1 {
        return Ok(sorted[count / 2].expression);
    }
    let lower = sorted[count / 2 - 1].expression;
    let upper = sorted[count / 2].expression;
    let total = apply(pool, Operator::Add, &[lower, upper])?;
    let two = pool
        .number(Integer::from(2_i64).into())
        .map_err(|_| ArrayError::Unsupported)?;
    apply(pool, Operator::Div, &[total, two])
}

pub fn array_reduction(
    pool: &mut ExprPool,
    operator: Operator,
    arguments: &[ExprId],
) -> Option<Shaped<ExprId>> {
    let array = array_expression(pool, *arguments.first()?)?;
    let array = match array {
        Ok(array) => array,
        Err(error) => return Some(Err(error)),
    };
    Some(match operator {
        Operator::Count => {
            let count = Integer::from(array.elements.len() as i64);
            pool.number(count.into())
                .map_err(|_| ArrayError::Unsupported)
        }
        Operator::Total if array.elements.is_empty() => pool
            .number(Integer::from(0_i64).into())
            .map_err(|_| ArrayError::Unsupported),
        Operator::Total => folded(pool, Operator::Add, &array),
        Operator::Mean => folded(pool, Operator::Add, &array).and_then(|total| {
            let count = Integer::from(array.elements.len() as i64);
            let count = pool
                .number(count.into())
                .map_err(|_| ArrayError::Unsupported)?;
            apply(pool, Operator::Div, &[total, count])
        }),
        Operator::Median => median_of(pool, &array),
        Operator::Smallest => ordered(pool, &array).map(|sorted| sorted[0].expression),
        Operator::Largest => {
            ordered(pool, &array).map(|sorted| sorted[sorted.len() - 1].expression)
        }
        Operator::Determinant => determinant_of(pool, &array),
        Operator::Trace => trace_of(pool, &array),
        Operator::Rank => rank_of(pool, &array),
        Operator::At => match arguments.get(1) {
            Some(index) => entry_at(pool, &array, *index),
            None => Err(ArrayError::Unsupported),
        },
        _ => Err(ArrayError::Unsupported),
    })
}

fn derivative_of_each_cell(
    pool: &mut ExprPool,
    expression: ExprId,
    arguments: &[ExprId],
    body: ExprId,
) -> Shaped<ExprId> {
    let point = arguments.first().copied();
    if let Some(array) = array_expression(pool, body) {
        if point.is_some_and(|point| array_expression(pool, point).is_some()) {
            return Err(ArrayError::BodyAndPointAreArrays);
        }
        let array = array?;
        let cells: Vec<(Vec<ExprId>, ExprId)> = array
            .elements
            .iter()
            .map(|element| (arguments.to_vec(), *element))
            .collect();
        return derivatives_of(pool, expression, &array.shape, &cells);
    }
    let Some(point) = point else {
        return Ok(expression);
    };
    let Some(array) = array_expression(pool, point) else {
        return Ok(expression);
    };
    let array = array?;
    let cells: Vec<(Vec<ExprId>, ExprId)> = array
        .elements
        .iter()
        .map(|element| {
            let mut taken = arguments.to_vec();
            taken[0] = *element;
            (taken, body)
        })
        .collect();
    derivatives_of(pool, expression, &array.shape, &cells)
}

fn derivatives_of(
    pool: &mut ExprPool,
    expression: ExprId,
    shape: &[u32],
    cells: &[(Vec<ExprId>, ExprId)],
) -> Shaped<ExprId> {
    let name = pool.bound_name(expression).map(str::to_string);
    let mut built = Vec::with_capacity(cells.len());
    for (arguments, body) in cells {
        let cell = pool
            .bind(BinderKind::Derivative, arguments, *body)
            .map_err(|_| ArrayError::Unsupported)?;
        if let Some(name) = &name {
            pool.record_bound_name(cell, name)
                .map_err(|_| ArrayError::Unsupported)?;
        }
        built.push(cell);
    }
    pool.array(shape, &built)
        .map_err(|_| ArrayError::Unsupported)
}

pub fn reduce_arrays(pool: &mut ExprPool, expression: ExprId) -> Shaped<ExprId> {
    let view = pool.node(expression).map_err(|_| ArrayError::Unsupported)?;
    let (head, arguments) = match view {
        NodeView::Apply { head, arguments } => (head, arguments.to_vec()),
        NodeView::Bind {
            binder: BinderKind::Derivative,
            arguments,
            body,
        } => {
            let (arguments, body) = (arguments.to_vec(), body);
            return derivative_of_each_cell(pool, expression, &arguments, body);
        }
        _ => return Ok(expression),
    };
    let mut reduced = Vec::with_capacity(arguments.len());
    for argument in &arguments {
        reduced.push(reduce_arrays(pool, *argument)?);
    }
    if let Head::Operator(operator) = head
        && matches!(
            operator,
            Operator::Count
                | Operator::Total
                | Operator::Mean
                | Operator::Median
                | Operator::Determinant
                | Operator::Trace
                | Operator::Rank
                | Operator::Smallest
                | Operator::Largest
                | Operator::At
        )
    {
        return match array_reduction(pool, operator, &reduced) {
            Some(result) => result,
            None => Err(ArrayError::NotAList),
        };
    }
    if reduced == arguments {
        return Ok(expression);
    }
    pool.apply(head, &reduced)
        .map_err(|_| ArrayError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_syntax::parse_expression;

    fn shaped(text: &str) -> Option<Shaped<ArrayExpression>> {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).ok()?;
        array_expression(&mut pool, expression)
    }

    fn shape_of(text: &str) -> Vec<u32> {
        shaped(text).unwrap().unwrap().shape
    }

    #[test]
    fn a_list_keeps_its_length_as_its_shape() {
        assert_eq!(shape_of("[1, 2, 3]"), vec![3]);
    }

    #[test]
    fn a_matrix_keeps_its_rows_and_columns() {
        assert_eq!(shape_of("[1, 2; 3, 4]"), vec![2, 2]);
    }

    #[test]
    fn a_sum_of_two_lists_is_taken_elementwise() {
        assert_eq!(shape_of("[1, 2] + [3, 4]"), vec![2]);
    }

    #[test]
    fn a_sum_of_lists_of_different_length_is_refused() {
        assert_eq!(
            shaped("[1, 2] + [3, 4, 5]"),
            Some(Err(ArrayError::ShapesDiffer))
        );
    }

    #[test]
    fn a_product_of_two_matrices_takes_the_inner_dimension() {
        assert_eq!(shape_of("[1, 2; 3, 4] * [1, 0; 0, 1]"), vec![2, 2]);
    }

    #[test]
    fn a_product_of_two_lists_is_refused_as_ambiguous() {
        assert_eq!(
            shaped("[1, 2] * [3, 4]"),
            Some(Err(ArrayError::ProductOfLists))
        );
    }

    #[test]
    fn a_list_times_a_number_is_taken_elementwise() {
        assert_eq!(shape_of("[1, 2, 3] * 2"), vec![3]);
    }

    #[test]
    fn a_number_times_a_list_is_taken_elementwise() {
        assert_eq!(shape_of("2 * [1, 2, 3]"), vec![3]);
    }

    #[test]
    fn a_list_divided_by_a_number_is_taken_elementwise() {
        assert_eq!(shape_of("[2, 4] / 2"), vec![2]);
    }

    #[test]
    fn a_number_minus_a_list_is_taken_elementwise() {
        assert_eq!(shape_of("0 - [1, 2, 3]"), vec![3]);
    }

    #[test]
    fn a_number_divided_by_a_list_is_refused() {
        assert_eq!(shaped("2 / [1, 2]"), Some(Err(ArrayError::Unsupported)));
    }

    #[test]
    fn an_expression_with_no_array_in_it_is_not_an_array() {
        assert_eq!(shaped("2 + 3"), None);
    }
}
