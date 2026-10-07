use calc_expr::{ExprId, ExprPool, Head, NodeView, Operator, SymbolId};

use crate::polynomial::{AtomTable, linear_combination, polynomial_expression, polynomial_of};
use crate::square_root_sum::SquareRootSum;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemRefusal {
    NotAnEquation,
    NotLinear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemOutcome {
    Solved {
        values: Vec<(SymbolId, ExprId)>,
        free: Vec<SymbolId>,
    },
    NoSolution,
}

fn rows_of(
    pool: &mut ExprPool,
    equations: &[ExprId],
    unknowns: &[SymbolId],
    atoms: &mut AtomTable,
) -> Result<Vec<Vec<SquareRootSum>>, SystemRefusal> {
    let width = unknowns.len();
    let mut rows = Vec::with_capacity(equations.len());
    for equation in equations {
        let (left, right) = match pool.node(*equation) {
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::Equal),
                arguments: [left, right],
            }) => (*left, *right),
            _ => return Err(SystemRefusal::NotAnEquation),
        };
        let difference = pool
            .apply(Head::Operator(Operator::Sub), &[left, right])
            .map_err(|_| SystemRefusal::NotLinear)?;
        let polynomial = polynomial_of(pool, difference, atoms).ok_or(SystemRefusal::NotLinear)?;
        if atoms.expressions().len() > width {
            return Err(SystemRefusal::NotLinear);
        }
        let mut row = vec![SquareRootSum::zero(); width + 1];
        for (monomial, coefficient) in polynomial.terms() {
            match monomial.factors() {
                [] => row[width] = coefficient.negated(),
                [(atom, 1)] => row[*atom] = coefficient.clone(),
                _ => return Err(SystemRefusal::NotLinear),
            }
        }
        rows.push(row);
    }
    Ok(rows)
}

fn reduced(rows: &mut [Vec<SquareRootSum>], width: usize) -> Option<Vec<(usize, usize)>> {
    let mut pivots = Vec::new();
    let mut next_row = 0;
    for column in 0..width {
        let Some(found) = (next_row..rows.len()).find(|row| !rows[*row][column].is_zero()) else {
            continue;
        };
        rows.swap(next_row, found);
        let scale = rows[next_row][column].reciprocal()?;
        for entry in rows[next_row].iter_mut() {
            *entry = entry.times(&scale);
        }
        let pivot_row = rows[next_row].clone();
        for (other, row) in rows.iter_mut().enumerate() {
            if other == next_row || row[column].is_zero() {
                continue;
            }
            let factor = row[column].clone();
            for (entry, pivot_entry) in row.iter_mut().zip(&pivot_row) {
                *entry = entry.minus(&pivot_entry.times(&factor));
            }
        }
        pivots.push((next_row, column));
        next_row += 1;
        if next_row == rows.len() {
            break;
        }
    }
    Some(pivots)
}

pub fn solve_system(
    pool: &mut ExprPool,
    equations: &[ExprId],
    unknowns: &[SymbolId],
) -> Result<SystemOutcome, SystemRefusal> {
    let mut atoms = AtomTable::default();
    for unknown in unknowns {
        let symbol = pool
            .symbol(*unknown)
            .map_err(|_| SystemRefusal::NotLinear)?;
        atoms.index_of(symbol);
    }
    let width = unknowns.len();
    let mut rows = rows_of(pool, equations, unknowns, &mut atoms)?;
    let pivots = reduced(&mut rows, width).ok_or(SystemRefusal::NotLinear)?;
    let inconsistent = rows
        .iter()
        .any(|row| row[..width].iter().all(SquareRootSum::is_zero) && !row[width].is_zero());
    if inconsistent {
        return Ok(SystemOutcome::NoSolution);
    }
    let pivot_columns: Vec<usize> = pivots.iter().map(|(_, column)| *column).collect();
    let free_columns: Vec<usize> = (0..width)
        .filter(|column| !pivot_columns.contains(column))
        .collect();
    let mut values = Vec::with_capacity(pivots.len());
    for (row, column) in &pivots {
        let terms: Vec<(usize, SquareRootSum)> = free_columns
            .iter()
            .filter(|free| !rows[*row][**free].is_zero())
            .map(|free| (*free, rows[*row][*free].negated()))
            .collect();
        let combination = linear_combination(rows[*row][width].clone(), &terms)
            .ok_or(SystemRefusal::NotLinear)?;
        let written =
            polynomial_expression(pool, &combination, &atoms).ok_or(SystemRefusal::NotLinear)?;
        values.push((unknowns[*column], written));
    }
    Ok(SystemOutcome::Solved {
        values,
        free: free_columns
            .iter()
            .map(|column| unknowns[*column])
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::SymbolKind;
    use calc_syntax::{PrintMode, parse_expression, print_expression};

    fn solved(equations: &[&str], names: &[&str]) -> Result<SystemOutcome, SystemRefusal> {
        let mut pool = ExprPool::new();
        let equations: Vec<ExprId> = equations
            .iter()
            .map(|text| parse_expression(&mut pool, text).expect("it parses"))
            .collect();
        let unknowns: Vec<SymbolId> = names
            .iter()
            .map(|name| {
                pool.intern_symbol(name, SymbolKind::Variable)
                    .expect("interned")
            })
            .collect();
        let outcome = solve_system(&mut pool, &equations, &unknowns)?;
        if let SystemOutcome::Solved { values, .. } = &outcome {
            for (_, value) in values {
                print_expression(&pool, *value, PrintMode::Ascii).expect("it prints");
            }
        }
        Ok(outcome)
    }

    #[test]
    fn two_equations_in_two_names_have_one_solution() {
        let outcome = solved(&["x + y = 3", "x - y = 1"], &["x", "y"]).unwrap();
        assert!(matches!(outcome, SystemOutcome::Solved { ref free, .. } if free.is_empty()));
    }

    #[test]
    fn parallel_equations_have_no_solution() {
        assert_eq!(
            solved(&["x + y = 1", "x + y = 2"], &["x", "y"]),
            Ok(SystemOutcome::NoSolution)
        );
    }

    #[test]
    fn a_dependent_system_leaves_a_name_free() {
        let outcome = solved(&["x + y = 3", "2*x + 2*y = 6"], &["x", "y"]).unwrap();
        assert!(matches!(outcome, SystemOutcome::Solved { ref free, .. } if free.len() == 1));
    }

    #[test]
    fn a_product_of_names_is_not_linear() {
        assert_eq!(
            solved(&["x*y = 1", "x + y = 2"], &["x", "y"]),
            Err(SystemRefusal::NotLinear)
        );
    }
}
