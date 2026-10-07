pub const LARGEST_INTEGER_WIDTH: u32 = 65_536;

pub const COEFFICIENT_NOT_WRITTEN: i64 = -1;

const WIDTH_CODE: i64 = 0;
const SIGNED_CODE: i64 = 1;
const UNSIGNED_CODE: i64 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntegerTypeSpelling {
    Width,
    Signed,
    Unsigned,
}

impl IntegerTypeSpelling {
    pub fn named(signed: bool) -> Self {
        if signed { Self::Signed } else { Self::Unsigned }
    }

    pub fn code(self) -> i64 {
        match self {
            Self::Width => WIDTH_CODE,
            Self::Signed => SIGNED_CODE,
            Self::Unsigned => UNSIGNED_CODE,
        }
    }

    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            WIDTH_CODE => Some(Self::Width),
            SIGNED_CODE => Some(Self::Signed),
            UNSIGNED_CODE => Some(Self::Unsigned),
            _ => None,
        }
    }

    pub fn is_signed(self) -> bool {
        self == Self::Signed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spelling_survives_its_code() {
        for spelling in [
            IntegerTypeSpelling::Width,
            IntegerTypeSpelling::Signed,
            IntegerTypeSpelling::Unsigned,
        ] {
            assert_eq!(
                IntegerTypeSpelling::from_code(spelling.code()),
                Some(spelling)
            );
        }
    }

    #[test]
    fn a_width_alone_is_not_signed() {
        assert!(!IntegerTypeSpelling::Width.is_signed());
        assert!(!IntegerTypeSpelling::Unsigned.is_signed());
        assert!(IntegerTypeSpelling::Signed.is_signed());
    }

    #[test]
    fn an_unknown_code_is_no_spelling() {
        assert_eq!(IntegerTypeSpelling::from_code(3), None);
    }
}
