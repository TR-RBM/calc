#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluralCategory {
    One,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluralRule {
    OneOther,
}

impl PluralRule {
    pub fn category(self, count: u64) -> PluralCategory {
        match self {
            Self::OneOther if count == 1 => PluralCategory::One,
            Self::OneOther => PluralCategory::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_other_rule_gives_one_for_one() {
        assert_eq!(PluralRule::OneOther.category(1), PluralCategory::One);
    }

    #[test]
    fn one_other_rule_gives_other_for_zero() {
        assert_eq!(PluralRule::OneOther.category(0), PluralCategory::Other);
    }

    #[test]
    fn one_other_rule_gives_other_for_two() {
        assert_eq!(PluralRule::OneOther.category(2), PluralCategory::Other);
    }
}
