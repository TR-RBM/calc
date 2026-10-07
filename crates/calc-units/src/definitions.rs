use crate::dimension::{BASE_DIMENSION_COUNT, Dimension};

pub(crate) struct NamedUnitDefinition {
    pub symbol: &'static str,
    pub aliases: &'static [&'static str],
    pub display_symbol: &'static str,
    pub exponents: [i8; BASE_DIMENSION_COUNT],
    pub scale_numerator: u64,
    pub scale_denominator: u64,
    pub scale_power_of_ten: i8,
    pub scale_pi_exponent: i32,
    pub accepts_prefix: bool,
}

impl NamedUnitDefinition {
    pub fn dimension(&self) -> Dimension {
        Dimension::from_exponents(self.exponents)
    }
}

const SCALE_ONE: u64 = 1;
const GRAMS_PER_KILOGRAM: u64 = 1000;
const DEGREES_PER_HALF_TURN: u64 = 180;

const fn coherent(
    symbol: &'static str,
    exponents: [i8; BASE_DIMENSION_COUNT],
) -> NamedUnitDefinition {
    NamedUnitDefinition {
        symbol,
        aliases: &[],
        display_symbol: symbol,
        exponents,
        scale_numerator: SCALE_ONE,
        scale_power_of_ten: 0,
        scale_denominator: SCALE_ONE,
        scale_pi_exponent: 0,
        accepts_prefix: true,
    }
}

pub(crate) static BASE_UNITS: [NamedUnitDefinition; BASE_DIMENSION_COUNT] = [
    coherent("m", [1, 0, 0, 0, 0, 0, 0, 0]),
    NamedUnitDefinition {
        symbol: "kg",
        aliases: &[],
        display_symbol: "kg",
        exponents: [0, 1, 0, 0, 0, 0, 0, 0],
        scale_numerator: SCALE_ONE,
        scale_power_of_ten: 0,
        scale_denominator: SCALE_ONE,
        scale_pi_exponent: 0,
        accepts_prefix: false,
    },
    coherent("s", [0, 0, 1, 0, 0, 0, 0, 0]),
    coherent("A", [0, 0, 0, 1, 0, 0, 0, 0]),
    coherent("K", [0, 0, 0, 0, 1, 0, 0, 0]),
    coherent("mol", [0, 0, 0, 0, 0, 1, 0, 0]),
    coherent("cd", [0, 0, 0, 0, 0, 0, 1, 0]),
    coherent("bit", INFORMATION),
];

pub(crate) static DERIVED_UNITS: [NamedUnitDefinition; 21] = [
    coherent("rad", [0, 0, 0, 0, 0, 0, 0, 0]),
    coherent("sr", [0, 0, 0, 0, 0, 0, 0, 0]),
    coherent("Hz", [0, 0, -1, 0, 0, 0, 0, 0]),
    coherent("N", [1, 1, -2, 0, 0, 0, 0, 0]),
    coherent("Pa", [-1, 1, -2, 0, 0, 0, 0, 0]),
    coherent("J", [2, 1, -2, 0, 0, 0, 0, 0]),
    coherent("W", [2, 1, -3, 0, 0, 0, 0, 0]),
    coherent("C", [0, 0, 1, 1, 0, 0, 0, 0]),
    coherent("V", [2, 1, -3, -1, 0, 0, 0, 0]),
    coherent("F", [-2, -1, 4, 2, 0, 0, 0, 0]),
    NamedUnitDefinition {
        symbol: "\u{03A9}",
        aliases: &["\u{2126}"],
        display_symbol: "\u{03A9}",
        exponents: [2, 1, -3, -2, 0, 0, 0, 0],
        scale_numerator: SCALE_ONE,
        scale_power_of_ten: 0,
        scale_denominator: SCALE_ONE,
        scale_pi_exponent: 0,
        accepts_prefix: true,
    },
    coherent("S", [-2, -1, 3, 2, 0, 0, 0, 0]),
    coherent("Wb", [2, 1, -2, -1, 0, 0, 0, 0]),
    coherent("T", [0, 1, -2, -1, 0, 0, 0, 0]),
    coherent("H", [2, 1, -2, -2, 0, 0, 0, 0]),
    coherent("lm", [0, 0, 0, 0, 0, 0, 1, 0]),
    coherent("lx", [-2, 0, 0, 0, 0, 0, 1, 0]),
    coherent("Bq", [0, 0, -1, 0, 0, 0, 0, 0]),
    coherent("Gy", [2, 0, -2, 0, 0, 0, 0, 0]),
    coherent("Sv", [2, 0, -2, 0, 0, 0, 0, 0]),
    coherent("kat", [0, 0, -1, 0, 0, 1, 0, 0]),
];

pub(crate) static GRAM: NamedUnitDefinition = NamedUnitDefinition {
    symbol: "g",
    aliases: &[],
    display_symbol: "g",
    exponents: [0, 1, 0, 0, 0, 0, 0, 0],
    scale_numerator: SCALE_ONE,
    scale_denominator: GRAMS_PER_KILOGRAM,
    scale_power_of_ten: 0,
    scale_pi_exponent: 0,
    accepts_prefix: true,
};

pub(crate) static DEGREE_CELSIUS_DIFFERENCE: NamedUnitDefinition = NamedUnitDefinition {
    symbol: "degC",
    aliases: &[],
    display_symbol: "\u{00B0}C",
    exponents: [0, 0, 0, 0, 1, 0, 0, 0],
    scale_numerator: SCALE_ONE,
    scale_denominator: SCALE_ONE,
    scale_power_of_ten: 0,
    scale_pi_exponent: 0,
    accepts_prefix: false,
};

pub(crate) static DEGREE: NamedUnitDefinition = NamedUnitDefinition {
    symbol: "deg",
    aliases: &["\u{00B0}"],
    display_symbol: "\u{00B0}",
    exponents: [0, 0, 0, 0, 0, 0, 0, 0],
    scale_numerator: SCALE_ONE,
    scale_denominator: DEGREES_PER_HALF_TURN,
    scale_power_of_ten: 0,
    scale_pi_exponent: 1,
    accepts_prefix: false,
};

const fn outside_si(
    symbol: &'static str,
    display_symbol: &'static str,
    exponents: [i8; BASE_DIMENSION_COUNT],
    scale_numerator: u64,
    scale_denominator: u64,
) -> NamedUnitDefinition {
    NamedUnitDefinition {
        symbol,
        aliases: &[],
        display_symbol,
        exponents,
        scale_numerator,
        scale_denominator,
        scale_power_of_ten: 0,
        scale_pi_exponent: 0,
        accepts_prefix: false,
    }
}

const fn outside_si_prefixed(
    symbol: &'static str,
    display_symbol: &'static str,
    exponents: [i8; BASE_DIMENSION_COUNT],
    scale_numerator: u64,
    scale_denominator: u64,
) -> NamedUnitDefinition {
    NamedUnitDefinition {
        symbol,
        aliases: &[],
        display_symbol,
        exponents,
        scale_numerator,
        scale_denominator,
        scale_power_of_ten: 0,
        scale_pi_exponent: 0,
        accepts_prefix: true,
    }
}

const fn outside_si_power_of_ten(
    symbol: &'static str,
    display_symbol: &'static str,
    exponents: [i8; BASE_DIMENSION_COUNT],
    scale_numerator: u64,
    scale_power_of_ten: i8,
    accepts_prefix: bool,
) -> NamedUnitDefinition {
    NamedUnitDefinition {
        symbol,
        aliases: &[],
        display_symbol,
        exponents,
        scale_numerator,
        scale_denominator: SCALE_ONE,
        scale_power_of_ten,
        scale_pi_exponent: 0,
        accepts_prefix,
    }
}

const TIME: [i8; BASE_DIMENSION_COUNT] = [0, 0, 1, 0, 0, 0, 0, 0];
const INFORMATION: [i8; BASE_DIMENSION_COUNT] = [0, 0, 0, 0, 0, 0, 0, 1];
const BITS_PER_BYTE: u64 = 8;

pub(crate) static BYTE: NamedUnitDefinition = NamedUnitDefinition {
    symbol: "B",
    aliases: &[],
    display_symbol: "B",
    exponents: INFORMATION,
    scale_numerator: BITS_PER_BYTE,
    scale_denominator: SCALE_ONE,
    scale_power_of_ten: 0,
    scale_pi_exponent: 0,
    accepts_prefix: true,
};

pub(crate) const SMALLEST_INFORMATION_PREFIX_POWER: i8 = 3;

pub(crate) static BINARY_PREFIXES: [(&str, u32); 6] = [
    ("Ki", 10),
    ("Mi", 20),
    ("Gi", 30),
    ("Ti", 40),
    ("Pi", 50),
    ("Ei", 60),
];
const LENGTH: [i8; BASE_DIMENSION_COUNT] = [1, 0, 0, 0, 0, 0, 0, 0];
const MASS: [i8; BASE_DIMENSION_COUNT] = [0, 1, 0, 0, 0, 0, 0, 0];
const VOLUME: [i8; BASE_DIMENSION_COUNT] = [3, 0, 0, 0, 0, 0, 0, 0];
const TEMPERATURE: [i8; BASE_DIMENSION_COUNT] = [0, 0, 0, 0, 1, 0, 0, 0];

pub(crate) static WEEK: NamedUnitDefinition = NamedUnitDefinition {
    symbol: "wk",
    aliases: &["week"],
    display_symbol: "wk",
    exponents: TIME,
    scale_numerator: 604_800,
    scale_denominator: SCALE_ONE,
    scale_power_of_ten: 0,
    scale_pi_exponent: 0,
    accepts_prefix: false,
};

pub(crate) static UNITS_ACCEPTED_FOR_USE_WITH_THE_SI: [NamedUnitDefinition; 5] = [
    outside_si("min", "min", TIME, 60, 1),
    outside_si("h", "h", TIME, 3600, 1),
    outside_si("d", "d", TIME, 86400, 1),
    NamedUnitDefinition {
        symbol: "L",
        aliases: &["l"],
        display_symbol: "L",
        exponents: VOLUME,
        scale_numerator: 1,
        scale_denominator: 1000,
        scale_power_of_ten: 0,
        scale_pi_exponent: 0,
        accepts_prefix: true,
    },
    outside_si("t", "t", MASS, 1000, 1),
];

pub(crate) static INTERNATIONAL_YARD_AND_POUND_UNITS: [NamedUnitDefinition; 5] = [
    outside_si("in", "in", LENGTH, 127, 5000),
    outside_si("ft", "ft", LENGTH, 381, 1250),
    outside_si("yd", "yd", LENGTH, 1143, 1250),
    outside_si("mi", "mi", LENGTH, 201168, 125),
    outside_si("lb", "lb", MASS, 45359237, 100000000),
];

const ENERGY: [i8; BASE_DIMENSION_COUNT] = [2, 1, -2, 0, 0, 0, 0, 0];
const PRESSURE: [i8; BASE_DIMENSION_COUNT] = [-1, 1, -2, 0, 0, 0, 0, 0];

pub(crate) static UNITS_OF_A_128: [NamedUnitDefinition; 3] = [
    outside_si_power_of_ten("eV", "eV", ENERGY, 1_602_176_634, -28, true),
    outside_si_power_of_ten("bar", "bar", PRESSURE, 1, 5, true),
    outside_si("atm", "atm", PRESSURE, 101_325, 1),
];

const POWER: [i8; BASE_DIMENSION_COUNT] = [2, 1, -3, 0, 0, 0, 0, 0];

pub(crate) static UNITS_OF_A_253: [NamedUnitDefinition; 12] = [
    outside_si_prefixed("cal_th", "cal_th", ENERGY, 523, 125),
    outside_si_prefixed("cal_IT", "cal_IT", ENERGY, 10_467, 2500),
    outside_si("a", "a", TIME, 31_557_600, 1),
    outside_si("gal_US", "gal_US", VOLUME, 473_176_473, 125_000_000_000),
    outside_si("gal_imp", "gal_imp", VOLUME, 454_609, 100_000_000),
    outside_si(
        "floz_US",
        "floz_US",
        VOLUME,
        473_176_473,
        16_000_000_000_000,
    ),
    outside_si("floz_imp", "floz_imp", VOLUME, 454_609, 16_000_000_000),
    outside_si("oz_av", "oz_av", MASS, 45_359_237, 1_600_000_000),
    outside_si("oz_t", "oz_t", MASS, 19_439_673, 625_000_000),
    outside_si(
        "hp_I",
        "hp_I",
        POWER,
        37_284_993_579_113_511,
        50_000_000_000_000,
    ),
    outside_si("hp_M", "hp_M", POWER, 588_399, 800),
    outside_si("hp_E", "hp_E", POWER, 746, 1),
];

pub(crate) struct AmbiguousName {
    pub name: &'static str,
    pub readings: &'static [&'static str],
    pub takes_prefix: bool,
}

pub(crate) static AMBIGUOUS_NAMES: [AmbiguousName; 8] = [
    AmbiguousName {
        name: "cal",
        readings: &["cal_th", "cal_IT"],
        takes_prefix: true,
    },
    AmbiguousName {
        name: "Cal",
        readings: &["kcal_th", "kcal_IT"],
        takes_prefix: false,
    },
    AmbiguousName {
        name: "yr",
        readings: &["a", "365 d", "366 d"],
        takes_prefix: false,
    },
    AmbiguousName {
        name: "gal",
        readings: &["gal_US", "gal_imp"],
        takes_prefix: false,
    },
    AmbiguousName {
        name: "oz",
        readings: &["oz_av", "oz_t", "floz_US", "floz_imp"],
        takes_prefix: false,
    },
    AmbiguousName {
        name: "hp",
        readings: &["hp_I", "hp_M", "hp_E"],
        takes_prefix: false,
    },
    AmbiguousName {
        name: "MOA",
        readings: &["arcmin", "IPHY"],
        takes_prefix: false,
    },
    AmbiguousName {
        name: "mil",
        readings: &["mrad", "mil_NATO", "thou"],
        takes_prefix: false,
    },
];

const AREA: [i8; BASE_DIMENSION_COUNT] = [2, 0, 0, 0, 0, 0, 0, 0];
const SPEED: [i8; BASE_DIMENSION_COUNT] = [1, 0, -1, 0, 0, 0, 0, 0];

pub(crate) static UNITS_OF_A_252: [NamedUnitDefinition; 9] = [
    outside_si_prefixed("Wh", "Wh", ENERGY, 3600, 1),
    outside_si("ha", "ha", AREA, 10_000, 1),
    outside_si("au", "au", LENGTH, 149_597_870_700, 1),
    NamedUnitDefinition {
        symbol: "pc",
        aliases: &[],
        display_symbol: "pc",
        exponents: LENGTH,
        scale_numerator: 96_939_420_213_600_000,
        scale_denominator: 1,
        scale_power_of_ten: 0,
        scale_pi_exponent: -1,
        accepts_prefix: false,
    },
    outside_si("ly", "ly", LENGTH, 9_460_730_472_580_800, 1),
    outside_si("nmi", "nmi", LENGTH, 1852, 1),
    outside_si("kn", "kn", SPEED, 463, 900),
    outside_si("psi", "psi", PRESSURE, 8_896_443_230_521, 1_290_320_000),
    outside_si("mmHg", "mmHg", PRESSURE, 26_664_477_483, 200_000_000),
];

pub(crate) static TORR: NamedUnitDefinition = NamedUnitDefinition {
    symbol: "Torr",
    aliases: &["torr"],
    display_symbol: "Torr",
    exponents: PRESSURE,
    scale_numerator: 20_265,
    scale_denominator: 152,
    scale_power_of_ten: 0,
    scale_pi_exponent: 0,
    accepts_prefix: true,
};

const PLANE_ANGLE: [i8; BASE_DIMENSION_COUNT] = [0, 0, 0, 0, 0, 0, 0, 0];

const fn angle_of_pi(symbol: &'static str, denominator: u64) -> NamedUnitDefinition {
    NamedUnitDefinition {
        symbol,
        aliases: &[],
        display_symbol: symbol,
        exponents: PLANE_ANGLE,
        scale_numerator: 1,
        scale_denominator: denominator,
        scale_power_of_ten: 0,
        scale_pi_exponent: 1,
        accepts_prefix: false,
    }
}

pub(crate) static UNITS_OF_A_273: [NamedUnitDefinition; 5] = [
    angle_of_pi("arcmin", 10_800),
    angle_of_pi("arcsec", 648_000),
    angle_of_pi("mil_NATO", 3_200),
    outside_si("IPHY", "IPHY", PLANE_ANGLE, 1, 3_600),
    outside_si("thou", "thou", LENGTH, 127, 5_000_000),
];

pub(crate) static DEGREE_FAHRENHEIT_DIFFERENCE: NamedUnitDefinition =
    outside_si("degF", "\u{00B0}F", TEMPERATURE, 5, 9);

pub(crate) struct PrefixDefinition {
    pub symbol: &'static str,
    pub aliases: &'static [&'static str],
    pub power_of_ten: i8,
}

const fn prefix(symbol: &'static str, power_of_ten: i8) -> PrefixDefinition {
    PrefixDefinition {
        symbol,
        aliases: &[],
        power_of_ten,
    }
}

pub(crate) static PREFIXES: [PrefixDefinition; 24] = [
    prefix("Q", 30),
    prefix("R", 27),
    prefix("Y", 24),
    prefix("Z", 21),
    prefix("E", 18),
    prefix("P", 15),
    prefix("T", 12),
    prefix("G", 9),
    prefix("M", 6),
    prefix("k", 3),
    prefix("h", 2),
    prefix("da", 1),
    prefix("d", -1),
    prefix("c", -2),
    prefix("m", -3),
    PrefixDefinition {
        symbol: "\u{00B5}",
        aliases: &["\u{03BC}"],
        power_of_ten: -6,
    },
    prefix("n", -9),
    prefix("p", -12),
    prefix("f", -15),
    prefix("a", -18),
    prefix("z", -21),
    prefix("y", -24),
    prefix("r", -27),
    prefix("q", -30),
];

pub(crate) static KINDS_THAT_SHARE_A_UNIT: [(&str, &str); 6] = [
    ("Hz", "frequency"),
    ("Bq", "activity"),
    ("Gy", "absorbed_dose"),
    ("Sv", "equivalent_dose"),
    ("cd", "luminous_intensity"),
    ("lm", "luminous_flux"),
];
