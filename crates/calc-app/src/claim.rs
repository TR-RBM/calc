use std::collections::HashMap;
use std::fmt;

use calc_core::{Diagnostic, ParameterValue, Verdict, check_relation};
use calc_expr::{ExprId, ExprPool, SymbolId, SymbolKind, substitute_symbols};
use calc_numbers::{Integer, Number};
use calc_syntax::{ParseError, parse_expression};

use crate::result_record::LineId;
use crate::session::{Session, SessionError};

#[derive(Clone, Debug, PartialEq)]
pub enum ClaimOutcome {
    Checked(Verdict),
    OverRanges(calc_core::RangeVerdict),
    Definition,
    UnknownName(Diagnostic),
    Refused(Diagnostic),
    NotARelation,
    Unreadable(ParseError),
}

#[derive(Clone, Debug, PartialEq)]
pub enum IdentityOutcome {
    Decided(calc_core::IdentityVerdict),
    HoldsWhereDefined(Vec<String>),
    FailsWhereDefined(Vec<String>),
    OnlyInstance(Verdict, Vec<(String, String)>),
    Searched(IdentitySearch),
    Definition,
    UnknownName(Diagnostic),
    Refused(Diagnostic),
    NotARelation,
    Unreadable(ParseError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchedValue {
    numerator: i64,
    denominator: i64,
}

impl SearchedValue {
    const fn whole(value: i64) -> Self {
        Self {
            numerator: value,
            denominator: 1,
        }
    }

    fn number(self) -> Option<Number> {
        Number::from(self.numerator)
            .div_exact(&Number::from(self.denominator))
            .ok()
    }
}

impl fmt::Display for SearchedValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denominator == 1 {
            write!(formatter, "{}", self.numerator)
        } else {
            write!(formatter, "{}/{}", self.numerator, self.denominator)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentitySearch {
    Refuted(Vec<(String, SearchedValue)>),
    Stopped(Vec<(String, SearchedValue)>),
    NothingFound,
    NothingFoundWithFractions,
    NotRun,
    NothingToVary,
}

pub const IDENTITY_SEARCH_FROM: i64 = -16;

pub const IDENTITY_SEARCH_TO: i64 = 16;

pub const IDENTITY_FRACTION_DENOMINATORS: [i64; 3] = [2, 3, 4];

pub const IDENTITY_FRACTION_BOUND: i64 = 2;

fn names_a_label(error: &ParseError, text: &str) -> bool {
    error.kind == calc_syntax::ParseErrorKind::ReservedName
        && text
            .get(error.span.clone())
            .is_some_and(|name| crate::session_file::line_from_label(name).is_some())
}

fn entered(session: &mut Session, text: &str) -> Result<LineId, SessionError> {
    match session.enter(text) {
        Err(SessionError::NameExists(_) | SessionError::ReferenceCycle(_)) => {
            session.enter_question(text)
        }
        Err(SessionError::Parse(error)) if !names_a_label(&error, text) => session
            .enter_question(text)
            .map_err(|_| SessionError::Parse(error)),
        other => other,
    }
}

pub fn unknown_name_json(diagnostic: &Diagnostic) -> Vec<u8> {
    let named: Vec<(&str, &str)> = diagnostic
        .data
        .iter()
        .filter_map(|(name, value)| match value {
            ParameterValue::Identifier(text) => Some((name.as_str(), text.as_str())),
            _ => None,
        })
        .collect();
    crate::solve::typed_error_json(&diagnostic.code, &named)
}

fn free_label(pool: &ExprPool, expression: ExprId) -> Option<String> {
    let mut symbols = Vec::new();
    free_names(pool, expression, &mut symbols);
    symbols.iter().find_map(|symbol| {
        let name = pool.symbol_name(*symbol).ok()?;
        crate::session_file::line_from_label(name).map(|_| name.to_owned())
    })
}

fn not_entered(error: SessionError) -> ClaimOutcome {
    match error {
        SessionError::Parse(error) => ClaimOutcome::Unreadable(error),
        SessionError::ReferenceCycle(line) => {
            ClaimOutcome::UnknownName(Session::reference_cycle(line))
        }
        _ => ClaimOutcome::NotARelation,
    }
}

pub fn claim_of(session: &mut Session, text: &str) -> ClaimOutcome {
    match entered(session, text) {
        Ok(id) => session.check_line(id),
        Err(error) => not_entered(error),
    }
}

pub fn identity_of(session: &mut Session, claim: &str) -> IdentityOutcome {
    let id = match entered(session, claim) {
        Ok(id) => id,
        Err(error) => {
            return match not_entered(error) {
                ClaimOutcome::Unreadable(error) => IdentityOutcome::Unreadable(error),
                ClaimOutcome::UnknownName(diagnostic) => IdentityOutcome::UnknownName(diagnostic),
                _ => IdentityOutcome::NotARelation,
            };
        }
    };
    if session.is_definition(id) {
        return IdentityOutcome::Definition;
    }
    let Some((pool, expression)) = session.asked(id) else {
        return IdentityOutcome::NotARelation;
    };
    if let Some(label) = free_label(pool, expression) {
        return IdentityOutcome::UnknownName(Session::undefined_name(&label));
    }
    if let Some(diagnostic) = crate::session::unit_refusal(pool, expression) {
        return IdentityOutcome::Refused(diagnostic);
    }
    match identity_of_expression(pool, expression) {
        IdentityOutcome::OnlyInstance(verdict, _) => {
            IdentityOutcome::OnlyInstance(verdict, session.substitution_in(id))
        }
        decided => decided,
    }
}

fn written_denominators(pool: &ExprPool, excluded: &[ExprId]) -> Option<Vec<String>> {
    excluded
        .iter()
        .map(|expression| {
            calc_syntax::print_expression(pool, *expression, calc_syntax::PrintMode::Ascii).ok()
        })
        .collect()
}

fn identity_of_expression(pool: &mut ExprPool, expression: ExprId) -> IdentityOutcome {
    match calc_core::check_identity(pool, expression) {
        Some(calc_core::Identity::HoldsWhereDefined(excluded))
            if !names_that_vary(pool, expression).is_empty() =>
        {
            match written_denominators(pool, &excluded) {
                Some(written) => IdentityOutcome::HoldsWhereDefined(written),
                None => IdentityOutcome::Searched(IdentitySearch::NothingToVary),
            }
        }
        Some(calc_core::Identity::FailsWhereDefined(excluded))
            if !names_that_vary(pool, expression).is_empty() =>
        {
            match written_denominators(pool, &excluded) {
                Some(written) => IdentityOutcome::FailsWhereDefined(written),
                None => IdentityOutcome::Searched(IdentitySearch::NothingToVary),
            }
        }
        Some(
            calc_core::Identity::HoldsWhereDefined(_) | calc_core::Identity::FailsWhereDefined(_),
        ) => undecided_identity(pool, expression),
        Some(calc_core::Identity::Decided(calc_core::IdentityVerdict::Undecided)) => {
            undecided_identity(pool, expression)
        }
        Some(calc_core::Identity::Decided(calc_core::IdentityVerdict::HoldsEverywhere))
            if names_that_vary(pool, expression).is_empty() =>
        {
            IdentityOutcome::OnlyInstance(Verdict::Holds, Vec::new())
        }
        Some(calc_core::Identity::Decided(calc_core::IdentityVerdict::FailsEverywhere))
            if names_that_vary(pool, expression).is_empty() =>
        {
            IdentityOutcome::OnlyInstance(Verdict::Fails, Vec::new())
        }
        Some(calc_core::Identity::Decided(verdict)) => IdentityOutcome::Decided(verdict),
        None if calc_core::is_relation(pool, expression) => undecided_identity(pool, expression),
        None => IdentityOutcome::NotARelation,
    }
}

fn names_that_vary(pool: &ExprPool, expression: ExprId) -> Vec<(SymbolId, String)> {
    let mut symbols = Vec::new();
    free_names(pool, expression, &mut symbols);
    symbols
        .iter()
        .filter(|symbol| !calc_core::is_a_constant(pool, **symbol))
        .filter_map(|symbol| {
            pool.symbol_name(*symbol)
                .ok()
                .map(|name| (*symbol, name.to_owned()))
        })
        .filter(|(_, name)| crate::session_file::line_from_label(name).is_none())
        .collect()
}

fn undecided_identity(pool: &mut ExprPool, expression: ExprId) -> IdentityOutcome {
    let named = names_that_vary(pool, expression);
    if named.is_empty() {
        return match check_relation(pool, expression) {
            Some(Verdict::Holds) => IdentityOutcome::OnlyInstance(Verdict::Holds, Vec::new()),
            Some(Verdict::Fails) => IdentityOutcome::OnlyInstance(Verdict::Fails, Vec::new()),
            _ => IdentityOutcome::Searched(IdentitySearch::NothingToVary),
        };
    }
    let symbols: Vec<SymbolId> = named.iter().map(|(symbol, _)| *symbol).collect();
    let names: Vec<String> = named.into_iter().map(|(_, name)| name).collect();
    let search = first_counterexample(pool, expression, &names, &symbols);
    let searched = match (&search, names.as_slice(), symbols.as_slice()) {
        (IdentitySearch::NothingFound, [name], [symbol]) => {
            fraction_counterexample(pool, expression, name, *symbol)
        }
        _ => search,
    };
    IdentityOutcome::Searched(searched)
}

fn searched_values() -> Vec<SearchedValue> {
    let mut values = vec![SearchedValue::whole(0)];
    for size in 1..=IDENTITY_SEARCH_TO {
        values.push(SearchedValue::whole(size));
        values.push(SearchedValue::whole(-size));
    }
    values
}

fn searched_fractions() -> Vec<SearchedValue> {
    let mut values = Vec::new();
    for denominator in IDENTITY_FRACTION_DENOMINATORS {
        for numerator in 1..IDENTITY_FRACTION_BOUND * denominator {
            if Integer::from(numerator)
                .gcd(&Integer::from(denominator))
                .is_one()
            {
                for signed in [numerator, -numerator] {
                    values.push(SearchedValue {
                        numerator: signed,
                        denominator,
                    });
                }
            }
        }
    }
    values
}

fn fraction_counterexample(
    pool: &mut ExprPool,
    expression: ExprId,
    name: &str,
    symbol: SymbolId,
) -> IdentitySearch {
    for value in searched_fractions() {
        match verdict_at(pool, expression, &[symbol], &[value]) {
            Some(Verdict::Fails) => return IdentitySearch::Refuted(vec![(name.to_owned(), value)]),
            Some(Verdict::Holds) => {}
            _ => return IdentitySearch::NothingFound,
        }
    }
    IdentitySearch::NothingFoundWithFractions
}

fn next_index(indices: &mut [usize], width: usize) -> bool {
    for index in indices.iter_mut().rev() {
        if *index + 1 < width {
            *index += 1;
            return true;
        }
        *index = 0;
    }
    false
}

fn first_counterexample(
    pool: &mut ExprPool,
    expression: ExprId,
    names: &[String],
    symbols: &[SymbolId],
) -> IdentitySearch {
    let values = searched_values();
    let width = values.len();
    let mut count: u64 = 1;
    for _ in names {
        match count.checked_mul(width as u64) {
            Some(product) if product <= SEARCH_LIMIT => count = product,
            _ => return IdentitySearch::NotRun,
        }
    }
    let mut indices = vec![0; names.len()];
    loop {
        let assignment: Vec<SearchedValue> = indices.iter().map(|index| values[*index]).collect();
        let named = || names.iter().cloned().zip(assignment.clone()).collect();
        match verdict_at(pool, expression, symbols, &assignment) {
            Some(Verdict::Fails) => return IdentitySearch::Refuted(named()),
            Some(Verdict::Holds) => {}
            _ => return IdentitySearch::Stopped(named()),
        }
        if !next_index(&mut indices, width) {
            return IdentitySearch::NothingFound;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpandOutcome {
    Expanded(String),
    ExpandedWhere(String, Vec<String>),
    DividesByZero(String),
    Definition(String),
    NotAlgebraic,
    Unreadable(ParseError),
}

pub fn expansion_of(session: &mut Session, text: &str) -> ExpandOutcome {
    let id = match entered(session, text) {
        Ok(id) => id,
        Err(SessionError::Parse(error)) => return ExpandOutcome::Unreadable(error),
        Err(_) => return ExpandOutcome::NotAlgebraic,
    };
    if session.is_definition(id) {
        return ExpandOutcome::Definition(defined_name(session, id));
    }
    let Some((pool, expression)) = session.asked(id) else {
        return ExpandOutcome::NotAlgebraic;
    };
    if let Some(division) = crate::session::division_by_identical_zero(pool, expression) {
        return match calc_syntax::print_expression(pool, division, calc_syntax::PrintMode::Ascii) {
            Ok(reading) => ExpandOutcome::DividesByZero(reading),
            Err(_) => ExpandOutcome::NotAlgebraic,
        };
    }
    let Some((expanded, excluding)) = crate::session::normal_form_with_condition(pool, expression)
    else {
        return ExpandOutcome::NotAlgebraic;
    };
    match calc_syntax::print_expression(pool, expanded, calc_syntax::PrintMode::Ascii) {
        Ok(text) if excluding.is_empty() => ExpandOutcome::Expanded(text),
        Ok(text) => ExpandOutcome::ExpandedWhere(text, excluding),
        Err(_) => ExpandOutcome::NotAlgebraic,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FactorOutcome {
    Factored(calc_core::Factorization),
    FactoredPolynomial(String),
    NotRational,
    Definition(String),
    Unreadable(ParseError),
}

pub fn factorization_of(session: &mut Session, text: &str) -> FactorOutcome {
    let id = match entered(session, text) {
        Ok(id) => id,
        Err(SessionError::Parse(error)) => return FactorOutcome::Unreadable(error),
        Err(_) => return FactorOutcome::NotRational,
    };
    if session.is_definition(id) {
        return FactorOutcome::Definition(defined_name(session, id));
    }
    let Some((pool, expression)) = session.asked(id) else {
        return FactorOutcome::NotRational;
    };
    let mut names = Vec::new();
    free_names(pool, expression, &mut names);
    if let [name] = names.as_slice() {
        return match calc_core::factored_polynomial(pool, expression, *name) {
            Ok(factored) => {
                match calc_syntax::print_expression(pool, factored, calc_syntax::PrintMode::Ascii) {
                    Ok(text) => FactorOutcome::FactoredPolynomial(text),
                    Err(_) => FactorOutcome::NotRational,
                }
            }
            Err(_) => FactorOutcome::NotRational,
        };
    }
    let Ok(evaluation) = calc_core::evaluate_exact(pool, expression) else {
        return FactorOutcome::NotRational;
    };
    if evaluation.unit().is_some() {
        return FactorOutcome::NotRational;
    }
    match evaluation
        .rational_value()
        .and_then(calc_core::factorization)
    {
        Some(factors) => FactorOutcome::Factored(factors),
        None => FactorOutcome::NotRational,
    }
}

fn defined_name(session: &Session, id: LineId) -> String {
    session
        .line(id)
        .and_then(|line| line.name())
        .unwrap_or_default()
        .to_owned()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolveOutcome {
    Solved(Vec<String>),
    SolvedRealOnly(Vec<String>),
    SolvedSystem {
        values: Vec<(String, String)>,
        free: Vec<String>,
    },
    SystemWithoutSolution,
    SolvedInequality {
        name: String,
        intervals: Vec<IntervalText>,
    },
    RefusedSystem(calc_core::SystemRefusal),
    Definition(String),
    Refused(calc_core::SolveRefusal),
    Unreadable(ParseError),
}

fn isolated_value(pool: &ExprPool, equation: ExprId, unknown: SymbolId) -> Option<ExprId> {
    let Ok(calc_expr::NodeView::Apply {
        head: calc_expr::Head::Operator(calc_expr::Operator::Equal),
        arguments: [left, right],
    }) = pool.node(equation)
    else {
        return None;
    };
    let is_unknown = |side: ExprId| matches!(pool.node(side), Ok(calc_expr::NodeView::Symbol(symbol)) if symbol == unknown);
    let holds_unknown = |side: ExprId| {
        let mut found = Vec::new();
        free_names(pool, side, &mut found);
        found.contains(&unknown)
    };
    match (*left, *right) {
        (named, value) | (value, named) if is_unknown(named) && !holds_unknown(value) => {
            Some(value)
        }
        _ => None,
    }
}

fn free_names(pool: &ExprPool, expression: ExprId, found: &mut Vec<SymbolId>) {
    match pool.node(expression) {
        Ok(calc_expr::NodeView::Symbol(symbol)) => {
            if !found.contains(&symbol) {
                found.push(symbol);
            }
        }
        Ok(calc_expr::NodeView::Apply { arguments, .. }) => {
            for argument in arguments.iter().copied() {
                free_names(pool, argument, found);
            }
        }
        Ok(calc_expr::NodeView::Quantity { value, .. }) => free_names(pool, value, found),
        Ok(calc_expr::NodeView::Array { elements, .. }) => {
            for element in elements.iter().copied() {
                free_names(pool, element, found);
            }
        }
        _ => {}
    }
}

fn solution_text(pool: &mut ExprPool, root: ExprId, form: calc_core::RationalForm) -> String {
    let written = match calc_core::evaluate_exact(pool, root) {
        Ok(evaluation) => {
            if let Some(number) = evaluation.rational_value() {
                return crate::summary::value_text_in(
                    &calc_core::ResultValue::Number(number.clone()),
                    form,
                );
            }
            evaluation.expression()
        }
        Err(_) => root,
    };
    crate::displayed_value::exact_expression_text_in(pool, written, form).unwrap_or_default()
}

fn constant_solution_text(
    pool: &mut ExprPool,
    root: ExprId,
    form: calc_core::RationalForm,
) -> String {
    if let Ok(evaluation) = calc_core::evaluate_exact(pool, root)
        && let Some(number) = evaluation.rational_value()
    {
        return crate::summary::value_text_in(
            &calc_core::ResultValue::Number(number.clone()),
            form,
        );
    }
    crate::displayed_value::exact_expression_text_in(pool, root, form).unwrap_or_default()
}

pub fn solutions_of(session: &mut Session, equation: &str) -> SolveOutcome {
    let id = match entered(session, equation) {
        Ok(id) => id,
        Err(SessionError::Parse(error)) => return SolveOutcome::Unreadable(error),
        Err(_) => return SolveOutcome::Refused(calc_core::SolveRefusal::NotAPolynomial),
    };
    let asked = if session.is_definition(id) {
        let name = defined_name(session, id);
        match session.naming_as_equation(id) {
            Some(equation) => Some(equation),
            None => return SolveOutcome::Definition(name),
        }
    } else {
        session.asked(id)
    };
    let Some((pool, expression)) = asked else {
        return SolveOutcome::Refused(calc_core::SolveRefusal::NotAPolynomial);
    };
    let form = calc_core::literal_form(pool, expression);
    let mut names = Vec::new();
    free_names(pool, expression, &mut names);
    names.retain(|symbol| !calc_core::is_a_constant(pool, *symbol));
    if let Ok(calc_expr::NodeView::Array { elements, .. }) = pool.node(expression) {
        let equations = elements.to_vec();
        return system_of(pool, &equations, &names, form);
    }
    let unknown = match names.as_slice() {
        [] => return SolveOutcome::Refused(calc_core::SolveRefusal::NoName),
        [single] => *single,
        several => {
            let mut spelled: Vec<String> = several
                .iter()
                .filter_map(|symbol| pool.symbol_name(*symbol).ok().map(str::to_owned))
                .collect();
            spelled.sort();
            return SolveOutcome::Refused(calc_core::SolveRefusal::SeveralNames(spelled));
        }
    };
    if calc_core::is_inequality(pool, expression) {
        return inequality_of(pool, expression, unknown, form);
    }
    if let Some(value) = isolated_value(pool, expression, unknown) {
        return SolveOutcome::Solved(vec![solution_text(pool, value, form)]);
    }
    match calc_core::solutions(pool, expression, unknown) {
        Err(calc_core::SolveRefusal::NotAPolynomial) => {
            match calc_core::constant_coefficient_solutions(pool, expression, unknown) {
                Ok(roots) => SolveOutcome::Solved(
                    roots
                        .into_iter()
                        .map(|root| constant_solution_text(pool, root, form))
                        .collect(),
                ),
                Err(calc_core::SolveRefusal::CoefficientOutsideTheField(atom)) => {
                    let written =
                        calc_syntax::print_expression(pool, atom, calc_syntax::PrintMode::Ascii)
                            .unwrap_or_default();
                    SolveOutcome::Refused(calc_core::SolveRefusal::ConstantCoefficients(vec![
                        written,
                    ]))
                }
                Err(
                    refusal @ (calc_core::SolveRefusal::EveryNumber
                    | calc_core::SolveRefusal::ConstantCoefficientDegree(_)
                    | calc_core::SolveRefusal::UnprovenCoefficient),
                ) => SolveOutcome::Refused(refusal),
                Err(_) => SolveOutcome::Refused(calc_core::SolveRefusal::NotAPolynomial),
            }
        }
        Ok(roots) => {
            let mut shown = Vec::with_capacity(roots.len());
            for root in roots {
                shown.push(solution_text(pool, root, form));
            }
            match calc_core::leaves_out_non_real_roots(pool, expression, unknown, shown.len()) {
                Some(true) => SolveOutcome::SolvedRealOnly(shown),
                _ => SolveOutcome::Solved(shown),
            }
        }
        Err(refusal) => SolveOutcome::Refused(refusal),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntervalText {
    pub from: Option<(String, bool)>,
    pub to: Option<(String, bool)>,
}

fn inequality_of(
    pool: &mut ExprPool,
    expression: ExprId,
    unknown: SymbolId,
    form: calc_core::RationalForm,
) -> SolveOutcome {
    let name = pool
        .symbol_name(unknown)
        .map(str::to_owned)
        .unwrap_or_default();
    match calc_core::inequality_solutions(pool, expression, unknown) {
        Ok(intervals) => {
            let mut written = Vec::with_capacity(intervals.len());
            for interval in intervals {
                let mut end = |end: Option<calc_core::IntervalEnd>| {
                    end.map(|end| (solution_text(pool, end.value, form), end.included))
                };
                let from = end(interval.from);
                let to = end(interval.to);
                written.push(IntervalText { from, to });
            }
            SolveOutcome::SolvedInequality {
                name,
                intervals: written,
            }
        }
        Err(refusal) => SolveOutcome::Refused(refusal),
    }
}

fn system_of(
    pool: &mut ExprPool,
    equations: &[ExprId],
    names: &[SymbolId],
    form: calc_core::RationalForm,
) -> SolveOutcome {
    let spelled = |pool: &ExprPool, symbol: SymbolId| {
        pool.symbol_name(symbol)
            .map(str::to_owned)
            .unwrap_or_default()
    };
    match calc_core::solve_system(pool, equations, names) {
        Ok(calc_core::SystemOutcome::Solved { values, free }) => {
            let mut shown = Vec::with_capacity(values.len());
            for (name, value) in values {
                let name = spelled(pool, name);
                shown.push((name, solution_text(pool, value, form)));
            }
            SolveOutcome::SolvedSystem {
                values: shown,
                free: free.into_iter().map(|name| spelled(pool, name)).collect(),
            }
        }
        Ok(calc_core::SystemOutcome::NoSolution) => SolveOutcome::SystemWithoutSolution,
        Err(refusal) => SolveOutcome::RefusedSystem(refusal),
    }
}

pub const SEARCH_LIMIT: u64 = 2_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchOutcome {
    Found {
        found: Vec<Vec<i64>>,
        undecided: Option<UndecidedPoints>,
    },
    TooManyCandidates(u64),
    Unreadable(ParseError),
    NotARelation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndecidedPoints {
    pub first: Vec<i64>,
    pub count: u64,
}

fn candidate_count(ranges: &[(String, i64, i64)]) -> Option<u64> {
    let mut total: u64 = 1;
    for (_, from, to) in ranges {
        let width = to.checked_sub(*from)?.checked_add(1)?;
        total = total.checked_mul(u64::try_from(width).ok()?)?;
        if total > SEARCH_LIMIT {
            return Some(total);
        }
    }
    Some(total)
}

fn verdict_at(
    pool: &mut ExprPool,
    expression: ExprId,
    symbols: &[SymbolId],
    values: &[SearchedValue],
) -> Option<Verdict> {
    let mut replacements = HashMap::new();
    for (symbol, value) in symbols.iter().zip(values) {
        replacements.insert(*symbol, pool.number(value.number()?).ok()?);
    }
    let instance = substitute_symbols(pool, expression, &replacements).ok()?;
    check_relation(pool, instance)
}

fn next_assignment(values: &mut [i64], ranges: &[(String, i64, i64)]) -> bool {
    for (index, value) in values.iter_mut().enumerate().rev() {
        if *value < ranges[index].2 {
            *value += 1;
            return true;
        }
        *value = ranges[index].1;
    }
    false
}

pub fn find_matches(
    ranges: &[(String, i64, i64)],
    claim: &str,
    wants_failures: bool,
) -> SearchOutcome {
    search_assignments(ranges, claim, wants_failures, false)
}

fn search_assignments(
    ranges: &[(String, i64, i64)],
    claim: &str,
    wants_failures: bool,
    first_only: bool,
) -> SearchOutcome {
    if ranges.is_empty() {
        return SearchOutcome::NotARelation;
    }
    match candidate_count(ranges) {
        Some(count) if count <= SEARCH_LIMIT => count,
        Some(count) => return SearchOutcome::TooManyCandidates(count),
        None => return SearchOutcome::TooManyCandidates(SEARCH_LIMIT),
    };
    let mut pool = ExprPool::new();
    let expression = match parse_expression(&mut pool, claim) {
        Ok(expression) => expression,
        Err(error) => return SearchOutcome::Unreadable(error),
    };
    let mut symbols = Vec::with_capacity(ranges.len());
    for (name, _, _) in ranges {
        let Ok(symbol) = pool.intern_symbol(name, SymbolKind::Variable) else {
            return SearchOutcome::NotARelation;
        };
        symbols.push(symbol);
    }
    let mut values: Vec<i64> = ranges.iter().map(|(_, from, _)| *from).collect();
    let mut found = Vec::new();
    let mut undecided: Option<UndecidedPoints> = None;
    loop {
        let assignment: Vec<SearchedValue> = values
            .iter()
            .map(|value| SearchedValue::whole(*value))
            .collect();
        match verdict_at(&mut pool, expression, &symbols, &assignment) {
            Some(Verdict::Holds) if !wants_failures => found.push(values.clone()),
            Some(Verdict::Fails) if wants_failures => found.push(values.clone()),
            Some(Verdict::Holds | Verdict::Fails) => {}
            Some(_) => match undecided.as_mut() {
                Some(points) => points.count += 1,
                None => {
                    undecided = Some(UndecidedPoints {
                        first: values.clone(),
                        count: 1,
                    });
                }
            },
            None => return SearchOutcome::NotARelation,
        }
        if first_only && !found.is_empty() {
            return SearchOutcome::Found { found, undecided };
        }
        if !next_assignment(&mut values, ranges) {
            break;
        }
    }
    SearchOutcome::Found { found, undecided }
}

pub fn claim_takes_zero_to_the_zero(claim: &str) -> bool {
    let mut pool = ExprPool::new();
    parse_expression(&mut pool, claim)
        .is_ok_and(|expression| crate::session::holds_zero_to_the_zero(&mut pool, expression, true))
}
