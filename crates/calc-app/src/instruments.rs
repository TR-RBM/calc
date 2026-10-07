use crate::session::Outcome;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineInstrument {
    Inspect,
    Method,
    Plot,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InstrumentFacts {
    pub plottable: bool,
    pub naming_shortened: bool,
}

pub fn applying_instruments(outcome: &Outcome, facts: InstrumentFacts) -> Vec<LineInstrument> {
    let mut applying = Vec::new();
    if shows_inspect(outcome, facts) {
        applying.push(LineInstrument::Inspect);
    }
    if matches!(outcome, Outcome::Answer(_) | Outcome::Reachable(_)) {
        applying.push(LineInstrument::Method);
    }
    if facts.plottable {
        applying.push(LineInstrument::Plot);
    }
    applying
}

const fn shows_inspect(outcome: &Outcome, facts: InstrumentFacts) -> bool {
    match outcome {
        Outcome::Defined => facts.naming_shortened,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solve_answer::ReachableAnswer;
    use calc_core::Diagnostic;
    use std::collections::BTreeMap;

    fn failed() -> Outcome {
        Outcome::Error(Diagnostic {
            code: "undefined_name".to_owned(),
            data: BTreeMap::new(),
        })
    }

    fn reachable() -> Outcome {
        Outcome::Reachable(Box::new(ReachableAnswer {
            concept_set_version: 1,
            object: "area".to_owned(),
            truncated: false,
            reachable_total: 0,
            entries: Vec::new(),
            quantities: Vec::new(),
        }))
    }

    fn nothing() -> InstrumentFacts {
        InstrumentFacts::default()
    }

    fn plottable() -> InstrumentFacts {
        InstrumentFacts {
            plottable: true,
            naming_shortened: false,
        }
    }

    fn shortened() -> InstrumentFacts {
        InstrumentFacts {
            plottable: false,
            naming_shortened: true,
        }
    }

    #[test]
    fn a_result_line_shows_inspect() {
        assert_eq!(
            applying_instruments(&Outcome::NotEvaluated, nothing()),
            vec![LineInstrument::Inspect]
        );
    }

    #[test]
    fn a_failed_line_shows_inspect() {
        assert_eq!(
            applying_instruments(&failed(), nothing()),
            vec![LineInstrument::Inspect]
        );
    }

    #[test]
    fn a_line_that_was_asked_what_is_reachable_shows_method() {
        assert_eq!(
            applying_instruments(&reachable(), nothing()),
            vec![LineInstrument::Inspect, LineInstrument::Method]
        );
    }

    #[test]
    fn a_definition_whose_naming_fits_shows_no_instrument() {
        assert_eq!(applying_instruments(&Outcome::Defined, nothing()), vec![]);
    }

    #[test]
    fn a_definition_whose_naming_was_shortened_shows_inspect() {
        assert_eq!(
            applying_instruments(&Outcome::Defined, shortened()),
            vec![LineInstrument::Inspect]
        );
    }

    #[test]
    fn a_definition_with_a_scene_shows_plot() {
        assert_eq!(
            applying_instruments(&Outcome::Defined, plottable()),
            vec![LineInstrument::Plot]
        );
    }

    #[test]
    fn a_line_with_a_scene_shows_plot_after_inspect() {
        assert_eq!(
            applying_instruments(&Outcome::NotEvaluated, plottable()),
            vec![LineInstrument::Inspect, LineInstrument::Plot]
        );
    }
}
