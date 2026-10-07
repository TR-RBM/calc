use calc_viz::SamplingDiagnostics;

const WHOLE_PERCENT: u64 = 100;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PictureText {
    pub axis_titles: Vec<Vec<String>>,
    pub legend_titles: Vec<String>,
    pub unit_joiner: String,
    pub domain_legend: DomainLegendText,
    pub mark_keys: MarkKeyText,
    pub sketch: String,
    pub designations: DesignationText,
    pub precision: PrecisionText,
    pub escape_time: EscapeTimeText,
    pub roles: RoleText,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoleText {
    pub missing: String,
    pub unresolved: String,
    pub may_be_hit: String,
    pub marked: String,
    pub back_face: String,
    pub provisional: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DomainLegendText {
    pub argument: String,
    pub modulus: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkKeyText {
    pub angle_arc: String,
    pub right_angle_square: String,
    pub right_angle_arc_with_dot: String,
    pub equal_ticks: String,
    pub direction: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesignationText {
    pub hypotenuse: String,
    pub leg: String,
    pub height: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PrecisionText {
    pub grid_limit: String,
    pub value_limit: String,
    pub unknown_bounds: String,
    pub marked_columns: String,
    pub width_columns: String,
    pub varies_below_bounds: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EscapeTimeText {
    pub inside: String,
    pub undecided: String,
    pub undecided_share: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndecidedShare {
    Percent(u8),
    BelowOnePercent,
    AboveNinetyNinePercent,
}

pub fn undecided_share(diagnostics: &SamplingDiagnostics) -> Option<UndecidedShare> {
    let undecided = diagnostics.undecided_cells;
    let total = diagnostics
        .escaped_cells
        .checked_add(diagnostics.inside_cells)?
        .checked_add(undecided)?;
    if total == 0 {
        return None;
    }
    let scaled = u128::from(undecided) * u128::from(WHOLE_PERCENT);
    let total_wide = u128::from(total);
    let quotient = scaled / total_wide;
    let twice_remainder = (scaled % total_wide) * 2;
    let rounds_up =
        twice_remainder > total_wide || (twice_remainder == total_wide && quotient % 2 == 1);
    let percent = u8::try_from(quotient + u128::from(rounds_up)).ok()?;
    Some(if undecided > 0 && percent == 0 {
        UndecidedShare::BelowOnePercent
    } else if undecided < total && u64::from(percent) == WHOLE_PERCENT {
        UndecidedShare::AboveNinetyNinePercent
    } else {
        UndecidedShare::Percent(percent)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostics(escaped: u64, inside: u64, undecided: u64) -> SamplingDiagnostics {
        SamplingDiagnostics {
            escaped_cells: escaped,
            inside_cells: inside,
            undecided_cells: undecided,
            ..SamplingDiagnostics::default()
        }
    }

    #[test]
    fn share_is_rounded_to_the_nearest_whole_percent() {
        assert_eq!(
            undecided_share(&diagnostics(2, 0, 1)),
            Some(UndecidedShare::Percent(33))
        );
    }

    #[test]
    fn share_on_a_tie_rounds_to_the_even_percent() {
        assert_eq!(
            undecided_share(&diagnostics(195, 0, 5)),
            Some(UndecidedShare::Percent(2))
        );
    }

    #[test]
    fn small_share_above_zero_is_below_one_percent() {
        assert_eq!(
            undecided_share(&diagnostics(999, 0, 1)),
            Some(UndecidedShare::BelowOnePercent)
        );
    }

    #[test]
    fn share_just_below_all_is_above_ninety_nine_percent() {
        assert_eq!(
            undecided_share(&diagnostics(1, 0, 999)),
            Some(UndecidedShare::AboveNinetyNinePercent)
        );
    }

    #[test]
    fn no_undecided_cell_is_zero_percent() {
        assert_eq!(
            undecided_share(&diagnostics(3, 4, 0)),
            Some(UndecidedShare::Percent(0))
        );
    }

    #[test]
    fn grid_without_cells_has_no_share() {
        assert_eq!(undecided_share(&diagnostics(0, 0, 0)), None);
    }
}
