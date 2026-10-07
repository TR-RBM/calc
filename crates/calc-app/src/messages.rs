use calc_core::{Diagnostic, ParameterValue, ResultValue};

use crate::json::Json;
use calc_exec::{BackendKind, Preference};
use calc_expr::{BuildError, SymbolError};
use calc_i18n::Message;
use calc_syntax::{
    Keyword, LARGEST_DECIMAL_EXPONENT, ParseError, ParseErrorKind, RecognisedAttempt,
    corrected_text,
};
use calc_units::UnitProductError;

use calc_viz::SampleError;

use crate::gpu_registration::GpuRefusal;
use crate::picture::PlotError;
use crate::reading::ReadError;
use crate::recognition::RecognitionUnavailable;
use crate::session::{Precision, SessionError};
use crate::session_file::{
    JsonPath, LoadError, PathSegment, SaveError, line_label, operation_name,
};
use crate::summary::value_text;
use crate::views::{ViewError, ViewKind};

const NAME_DATA: &str = "name";
const FIRST_DATA: &str = "first";
const COMPARISONS_DATA: &str = "comparisons";
const RANGE_DATA: &str = "range";
const UNNAMED_GENERATORS: [&str; 4] = ["random", "rand", "randint", "rng"];
const WRITES_DATA: &str = "writes";
const SECOND_DATA: &str = "second";
const SIDE_DATA: &str = "side";
const OPERATION_DATA: &str = "operation";
const METHOD_DATA: &str = "method";
const TRANSPOSE_OPERATION: &str = "transpose";
const DETERMINANT_OPERATION: &str = "determinant";
const LEFT_SIDE: &str = "left";
const RIGHT_SIDE: &str = "right";
const CONSTANT_DATA: &str = "constant";
const LINE_DATA: &str = "line";
const WRITTEN_DATA: &str = "written";
const DIFFERENCE_DATA: &str = "difference";
const NARROW_DATA: &str = "narrow";
const WIDE_DATA: &str = "wide";
const ATTEMPT_DATA: &str = "attempt";
const REPLACEMENT_DATA: &str = "replacement";
const CORRECTED_DATA: &str = "corrected";
const COLUMN_DATA: &str = "column";
const UNIT_ENDED_AT_SPACE_ATTEMPT: &str = "unit_ended_at_space";
const KIND_DATA: &str = "kind";
const PRECISION_DATA: &str = "precision";
const LONGEST_SHOWN_ARGUMENT_DIGITS: usize = 40;
const EQUALS: &str = " = ";
const OPERATOR_DATA: &str = "operator";
const ROLE_DATA: &str = "role";
const ROOT_DEGREE_ROLE: &str = "root_degree";
const OPERAND_DATA: &str = "operand";
const MAGNITUDE_DATA: &str = "magnitude";
const EXPONENT_DATA: &str = "exponent";
const LIMIT_DIGITS_DATA: &str = "limit_digits";
const READING_DATA: &str = "reading";
const DIGITS_DATA: &str = "digits";

const COUNT_DATA: &str = "count";
const CONVERSION_DATA: &str = "conversion";
const ARGUMENT_DATA: &str = "argument";
const LIMIT_DATA: &str = "limit";
const UNIT_SYMBOL_DATA: &str = "unit";
const OPERATOR_DATA_NAME: &str = "operator";
const READING_TEXT_DATA: &str = "reading";
const LIMIT_BITS_DATA: &str = "limit_bits";
const ESTIMATED_DIGITS_DATA: &str = "estimated_digits";
const PATH_MEMBER_SEPARATOR: &str = ".";
const LIST_SEPARATOR: &str = ", ";
const FIRST_COLUMN: usize = 1;
const COUNT_ERROR_CODE: &str = "count_too_large";
const UNIT_PRINT_ERROR_CODE: &str = "unit_not_printable";
const PARSE_BUILD_ERROR_CODE: &str = "expression_not_built";
const PARSE_SYMBOL_ERROR_CODE: &str = "symbol_not_found";
const PARSE_UNIT_ERROR_CODE: &str = "unit_not_found";

fn data_value(diagnostic: &Diagnostic, name: &str) -> Option<String> {
    match diagnostic.data.get(name) {
        Some(ParameterValue::Value(value)) => Some(value_text(value)),
        _ => None,
    }
}

fn data_digits(diagnostic: &Diagnostic, name: &str) -> Option<String> {
    match diagnostic.data.get(name) {
        Some(ParameterValue::Value(ResultValue::Number(number))) => {
            Some(crate::summary::digits_text(number))
        }
        _ => None,
    }
}

fn operation_of(diagnostic: &Diagnostic) -> Option<&str> {
    match diagnostic.data.get(OPERATION_DATA) {
        Some(ParameterValue::Identifier(operation)) => Some(operation.as_str()),
        _ => None,
    }
}

fn data_identifier(diagnostic: &Diagnostic, name: &str) -> String {
    match diagnostic.data.get(name) {
        Some(ParameterValue::Identifier(value)) => value.clone(),
        _ => String::new(),
    }
}

fn operand_quantity(operand: String, value: String, plain: Option<String>) -> String {
    if operand == value || plain.is_some_and(|plain| operand == plain) {
        value
    } else {
        format!("{operand}{EQUALS}{value}")
    }
}

fn negated_operand(diagnostic: &Diagnostic) -> Option<String> {
    data_value(diagnostic, MAGNITUDE_DATA)
}

fn domain_message(diagnostic: &Diagnostic, reading: Option<String>) -> Message {
    let operator = data_identifier(diagnostic, OPERATOR_DATA);
    let operand = data_value(diagnostic, OPERAND_DATA);
    let exponent = data_value(diagnostic, EXPONENT_DATA);
    match (operator.as_str(), operand, exponent, reading) {
        ("sqrt", Some(operand), _, _) => match negated_operand(diagnostic) {
            Some(magnitude) => Message::ErrorDomainSqrtComplex { operand, magnitude },
            None => Message::ErrorDomainSqrt { operand },
        },
        ("ln", Some(operand), _, _) => match negated_operand(diagnostic) {
            Some(magnitude) => Message::ErrorDomainLnComplex { operand, magnitude },
            None => Message::ErrorDomainLn { operand },
        },
        ("asin" | "acos", Some(operand), _, _) => Message::ErrorDomainArcSine { operator, operand },
        ("tan", Some(operand), _, _) => Message::ErrorDomainTan { operand },
        ("factorial", Some(operand), _, _) => Message::ErrorDomainFactorial { operand },
        ("pow", Some(base), Some(exponent), _) => Message::ErrorDomainPower { base, exponent },
        (_, _, _, Some(reading)) => Message::ErrorOutsideDomain { reading },
        _ => Message::ErrorOutsideDomainPlain { operator },
    }
}

fn argument_message(diagnostic: &Diagnostic, reading: Option<String>) -> Message {
    let operator = data_identifier(diagnostic, OPERATOR_DATA);
    let (Some(argument), Some(limit)) = (
        data_value(diagnostic, ARGUMENT_DATA),
        data_value(diagnostic, LIMIT_DATA),
    ) else {
        return Message::ErrorIntegerArgumentOutOfRangePlain { operator };
    };
    let plain = data_digits(diagnostic, ARGUMENT_DATA);
    let written = argument.trim_start_matches('-').chars().count();
    let digits = match &plain {
        Some(plain) => plain.chars().filter(char::is_ascii_digit).count(),
        None => written,
    };
    let is_long = written > LONGEST_SHOWN_ARGUMENT_DIGITS;
    let operand = match operator.as_str() {
        "pow" => data_value(diagnostic, EXPONENT_DATA),
        _ => data_value(diagnostic, OPERAND_DATA),
    };
    let is_root_degree = data_identifier(diagnostic, ROLE_DATA) == ROOT_DEGREE_ROLE;
    match (operator.as_str(), operand, is_long) {
        ("pow", Some(exponent), true) if is_root_degree => Message::ErrorRootDegreeOutOfRangeLong {
            exponent,
            digits: digits.to_string(),
            limit,
        },
        ("pow", Some(exponent), false) if is_root_degree => Message::ErrorRootDegreeOutOfRange {
            exponent,
            degree: argument,
            limit,
        },
        ("pow", Some(exponent), true) => Message::ErrorExponentOutOfRangeLong {
            exponent,
            digits: digits.to_string(),
            limit,
        },
        ("pow", Some(exponent), false) => Message::ErrorExponentOutOfRange {
            exponent: operand_quantity(exponent, argument, plain),
            limit,
        },
        ("factorial", Some(operand), true) => Message::ErrorFactorialOutOfRangeLong {
            operand,
            digits: digits.to_string(),
            limit,
        },
        ("factorial", Some(operand), false) => Message::ErrorFactorialOutOfRange {
            operand: operand_quantity(operand, argument, plain),
            limit,
        },
        _ => match reading {
            Some(reading) => Message::ErrorIntegerArgumentOutOfRange {
                reading,
                argument,
                limit,
            },
            None => Message::ErrorIntegerArgumentOutOfRangePlain { operator },
        },
    }
}

fn size_message(diagnostic: &Diagnostic, reading: Option<String>) -> Message {
    let sizes = (
        reading,
        data_value(diagnostic, ESTIMATED_DIGITS_DATA),
        data_value(diagnostic, LIMIT_DIGITS_DATA),
        data_value(diagnostic, LIMIT_BITS_DATA),
    );
    let (Some(reading), Some(digits), Some(limit_digits), Some(limit)) = sizes else {
        return Message::ErrorResultTooLargePlain;
    };
    match diagnostic.code.as_str() {
        "number_too_large" => Message::ErrorNumberTooLarge {
            reading,
            digits,
            limit_digits,
            limit,
        },
        "intermediate_step_too_large" => Message::ErrorIntermediateStepTooLarge {
            reading,
            digits,
            limit_digits,
            limit,
        },
        _ => Message::ErrorResultTooLarge {
            reading,
            digits,
            limit_digits,
            limit,
        },
    }
}

fn read_message(
    reading: Option<String>,
    with_reading: fn(String) -> Message,
    without_reading: Message,
) -> Message {
    reading.map_or(without_reading, with_reading)
}

fn keys_differ_in_dimension(diagnostic: &Diagnostic) -> Message {
    let identifier = |name: &str| data_identifier(diagnostic, name);
    let value = |name: &str| data_value(diagnostic, name).unwrap_or_default();
    if diagnostic.code == "sort_keyed_keys_differ_in_dimension" {
        return Message::ErrorSortKeyedKeysDifferInDimension {
            first: value("first"),
            first_position: value("first_position"),
            second: value("second"),
            second_position: value("second_position"),
            left_quantity: crate::dimension_mismatch::quantity_code(&identifier("left_quantity")),
            left_unit: identifier("left_unit"),
            right_quantity: crate::dimension_mismatch::quantity_code(&identifier("right_quantity")),
            right_unit: identifier("right_unit"),
        };
    }
    Message::ErrorSortKeysDifferInDimension {
        first: value("first"),
        first_position: value("first_position"),
        second: value("second"),
        second_position: value("second_position"),
        left_quantity: crate::dimension_mismatch::quantity_code(&identifier("left_quantity")),
        left_unit: identifier("left_unit"),
        right_quantity: crate::dimension_mismatch::quantity_code(&identifier("right_quantity")),
        right_unit: identifier("right_unit"),
    }
}

fn dimension_mismatch_sides(diagnostic: &Diagnostic, reading: String) -> Message {
    let identifier = |name: &str| data_identifier(diagnostic, name);
    let suggestion = identifier("suggestion");
    Message::ErrorDimensionMismatchSides {
        reading,
        left_quantity: crate::dimension_mismatch::quantity_code(&identifier("left_quantity")),
        left_unit: identifier("left_unit"),
        left_units: identifier("left_units"),
        right_quantity: crate::dimension_mismatch::quantity_code(&identifier("right_quantity")),
        right_unit: identifier("right_unit"),
        right_units: identifier("right_units"),
        suggestion_count: u64::from(!suggestion.is_empty()),
        suggestion,
    }
}

const KINDS_BY_CODE: [&str; 6] = [
    "frequency",
    "activity",
    "absorbed_dose",
    "equivalent_dose",
    "luminous_intensity",
    "luminous_flux",
];

fn kind_code(kind: &str) -> u64 {
    KINDS_BY_CODE
        .iter()
        .position(|known| *known == kind)
        .map_or(0, |position| position as u64 + 1)
}

pub fn diagnostic_message_reading(diagnostic: &Diagnostic, reading: &str) -> Message {
    let mut read = diagnostic.clone();
    read.data.insert(
        READING_DATA.to_owned(),
        ParameterValue::Value(ResultValue::Expression(reading.to_owned())),
    );
    diagnostic_message(&read)
}

pub fn diagnostic_message(diagnostic: &Diagnostic) -> Message {
    let identifier = |name: &str| data_identifier(diagnostic, name);
    let reading = data_value(diagnostic, READING_DATA);
    match diagnostic.code.as_str() {
        "temperature_difference" if !diagnostic.data.contains_key(UNIT_SYMBOL_DATA) => {
            Message::CommonNoteTemperatureDifferenceMixed
        }
        "zero_to_the_zero" => Message::CommonNoteZeroToTheZero,
        "kind_mismatch" => Message::ErrorKindMismatch {
            reading: reading.clone().unwrap_or_default(),
            left: kind_code(&identifier("left_kind")),
            right: kind_code(&identifier("right_kind")),
        },
        "temperature_reading" => Message::CommonNoteTemperatureReading {
            scale: identifier("scale"),
        },
        "temperature_difference_as_reading" => Message::ErrorTemperatureDifferenceAsReading {
            written: identifier("written"),
            number: identifier("number"),
            unit: identifier(UNIT_SYMBOL_DATA),
            reading: identifier(READING_TEXT_DATA),
        },
        "below_absolute_zero" => Message::ErrorBelowAbsoluteZero {
            reading: reading.clone().unwrap_or_default(),
            limit: identifier("limit"),
        },
        "machine_conversion" => Message::CommonNoteMachineConversion {
            written: identifier("written"),
            argument: identifier("argument"),
            machine: identifier("machine"),
            difference: identifier("difference"),
        },
        "temperature_difference" => match diagnostic.data.get(READING_TEXT_DATA) {
            Some(ParameterValue::Identifier(reading)) => Message::CommonNoteTemperatureDifference {
                unit: identifier(UNIT_SYMBOL_DATA),
                reading: reading.clone(),
            },
            _ => Message::CommonNoteTemperatureDifferenceInside {
                unit: identifier(UNIT_SYMBOL_DATA),
                operator: identifier(OPERATOR_DATA_NAME),
            },
        },
        "temperature_difference_entry" => Message::CommonNoteTemperatureDifferenceEntry {
            written: identifier(WRITTEN_DATA),
            position: identifier("position"),
            unit: identifier(UNIT_SYMBOL_DATA),
            reading: identifier(READING_TEXT_DATA),
        },
        "temperature_difference_entries" => Message::CommonNoteTemperatureDifferenceEntries {
            unit: identifier(UNIT_SYMBOL_DATA),
            operator: identifier(OPERATOR_DATA_NAME),
        },
        "enclosure_not_applicable" => Message::ErrorEnclosureCallNotApplicable,
        "enclosure_digits_not_whole" => Message::ErrorEnclosureDigitsNotWhole,
        "enclosure_digits_out_of_range" => Message::ErrorSignificantDigitsOutOfRange {
            digits: identifier(DIGITS_DATA),
            limit: calc_numbers::SIGNIFICANT_DIGITS_LIMIT.to_string(),
        },
        "antiderivative" => Message::CommonNoteAntiderivative,
        "worst_case_low" => Message::CommonNoteWorstCaseLow {
            corner: identifier("corner"),
        },
        "worst_case_high" => Message::CommonNoteWorstCaseHigh {
            corner: identifier("corner"),
        },
        "tolerance_written_twice" => Message::ErrorToleranceWrittenTwice {
            reading: reading.clone().unwrap_or_default(),
        },
        "worst_case_too_many" => Message::ErrorWorstCaseTooMany {
            limit: data_identifier(diagnostic, "limit"),
        },
        "worst_case_unsupported" => Message::ErrorWorstCaseUnsupported,
        "worst_case_endpoint_not_exact" => Message::ErrorWorstCaseEndpointNotExact {
            reading: reading.clone().unwrap_or_default(),
        },
        "worst_case_endpoint_not_enclosed" => Message::ErrorWorstCaseEndpointNotEnclosed {
            reading: reading.clone().unwrap_or_default(),
        },
        "worst_case_ends_of_two_dimensions" => Message::ErrorWorstCaseEndsOfTwoDimensions {
            reading: reading.clone().unwrap_or_default(),
        },
        "worst_case_divisor_reaches_zero" => Message::ErrorWorstCaseDivisorReachesZero {
            reading: reading.clone().unwrap_or_default(),
        },
        "worst_case_range_in_function_body" => Message::ErrorWorstCaseRangeInFunctionBody {
            name: data_identifier(diagnostic, "name"),
        },
        "worst_case_endpoints_reversed" => Message::ErrorWorstCaseEndpointsReversed {
            reading: reading.clone().unwrap_or_default(),
        },
        "worst_case_negative_tolerance" => Message::ErrorWorstCaseNegativeTolerance {
            reading: reading.clone().unwrap_or_default(),
        },
        "worst_case_not_monotone" => Message::ErrorWorstCaseNotMonotone {
            reading: reading.clone().unwrap_or_default(),
        },
        "chemistry_charge" => Message::CommonNoteChemistryCharge {
            charge: identifier("charge"),
        },
        "chemistry_each_side" => Message::CommonNoteChemistryEachSide {
            counts: identifier("counts"),
        },
        "chemistry_charge_each_side" => Message::CommonNoteChemistryChargeEachSide {
            charge: identifier("charge"),
        },
        "chemistry_tables" => Message::CommonNoteChemistryTables,
        "chemistry_natural_interval" => Message::CommonNoteChemistryNaturalInterval {
            elements: identifier("elements"),
        },
        "chemistry_expanded_uncertainty" => Message::CommonNoteChemistryExpandedUncertainty {
            elements: identifier("elements"),
        },
        "nuclear_measured_mass" => Message::CommonNoteNuclearMeasuredMass,
        "nuclear_estimated_mass_note" => Message::CommonNoteNuclearEstimatedMass,
        "nuclear_no_mass_note" => Message::CommonNoteNuclearNoMass,
        "nuclear_each_side" => Message::CommonNoteNuclearEachSide {
            mass: identifier("mass"),
            charge: identifier("charge"),
            leptons: identifier("leptons"),
        },
        "nuclear_photons" => Message::CommonNoteNuclearPhotons,
        "nuclear_q_tables" => Message::CommonNoteNuclearQTables,
        "nuclear_q_atomic" => Message::CommonNoteNuclearQAtomic,
        "nuclear_q_ground_state" => Message::CommonNoteNuclearQGroundState,
        "nuclear_q_capture" => Message::CommonNoteNuclearQCapture,
        "nuclear_q_positron" => Message::CommonNoteNuclearQPositron,
        "nuclear_q_decay_impossible" => Message::CommonNoteNuclearQDecayImpossible,
        "nuclear_q_sign_undecided" => Message::CommonNoteNuclearQSignUndecided,
        "nuclear_reaction_alone" => Message::ErrorNuclearReactionAlone {
            reading: reading.clone().unwrap_or_default(),
        },
        "nuclear_nuclide_not_a_number" => Message::ErrorNuclearNuclideNotANumber {
            reading: reading.clone().unwrap_or_default(),
        },
        "nuclear_empty" => Message::ErrorNuclearEmpty {
            reading: reading.clone().unwrap_or_default(),
        },
        "nuclear_mass_number_not_conserved" => Message::ErrorNuclearMassNumberNotConserved {
            reading: reading.clone().unwrap_or_default(),
            reactants: identifier("reactants"),
            products: identifier("products"),
        },
        "nuclear_charge_not_conserved" => Message::ErrorNuclearChargeNotConserved {
            reading: reading.clone().unwrap_or_default(),
            reactants: identifier("reactants"),
            products: identifier("products"),
        },
        "nuclear_lepton_number_not_conserved" => Message::ErrorNuclearLeptonNumberNotConserved {
            reading: reading.clone().unwrap_or_default(),
            reactants: identifier("reactants"),
            products: identifier("products"),
            neutrinos: identifier("neutrinos"),
        },
        "nuclear_mass_number_not_conserved_balance" => {
            Message::ErrorNuclearMassNumberNotConservedBalance {
                reading: reading.clone().unwrap_or_default(),
                reactants: identifier("reactants"),
                products: identifier("products"),
                balance: identifier("balance"),
            }
        }
        "nuclear_charge_not_conserved_balance" => Message::ErrorNuclearChargeNotConservedBalance {
            reading: reading.clone().unwrap_or_default(),
            reactants: identifier("reactants"),
            products: identifier("products"),
            balance: identifier("balance"),
        },
        "nuclear_lepton_number_not_conserved_balance" => {
            Message::ErrorNuclearLeptonNumberNotConservedBalance {
                reading: reading.clone().unwrap_or_default(),
                reactants: identifier("reactants"),
                products: identifier("products"),
                neutrinos: identifier("neutrinos"),
                balance: identifier("balance"),
            }
        }
        "nuclear_q_value_needs_a_reaction" => Message::ErrorNuclearQValueNeedsAReaction {
            reading: reading.clone().unwrap_or_default(),
        },
        "nuclear_q_value_does_not_balance" => Message::ErrorNuclearQValueDoesNotBalance {
            reading: reading.clone().unwrap_or_default(),
        },
        "nuclear_estimated_mass" => Message::ErrorNuclearEstimatedMass {
            reading: reading.clone().unwrap_or_default(),
            nuclide: identifier("nuclide"),
        },
        "nuclear_no_mass" => Message::ErrorNuclearNoMass {
            reading: reading.clone().unwrap_or_default(),
            nuclide: identifier("nuclide"),
        },
        "chemistry_reaction_alone" => Message::ErrorChemistryReactionAlone {
            reading: reading.clone().unwrap_or_default(),
        },
        "chemistry_substance_not_a_number" => Message::ErrorChemistrySubstanceNotANumber {
            reading: reading.clone().unwrap_or_default(),
        },
        "chemistry_too_many_species" => Message::ErrorChemistryTooManySpecies {
            reading: reading.clone().unwrap_or_default(),
            limit: identifier("limit"),
            count: identifier("count"),
        },
        "chemistry_no_reaction_with_these_sides" => {
            Message::ErrorChemistryNoReactionWithTheseSides {
                reading: reading.clone().unwrap_or_default(),
                count: identifier("count"),
            }
        }
        "chemistry_empty" => Message::ErrorChemistryEmpty {
            reading: reading.clone().unwrap_or_default(),
        },
        "chemistry_element_not_conserved" => Message::ErrorChemistryElementNotConserved {
            reading: reading.clone().unwrap_or_default(),
            element: identifier("element"),
            reactants: identifier("reactants"),
            products: identifier("products"),
        },
        "chemistry_charge_not_conserved" => Message::ErrorChemistryChargeNotConserved {
            reading: reading.clone().unwrap_or_default(),
            reactants: identifier("reactants"),
            products: identifier("products"),
        },
        "chemistry_not_unique" => Message::ErrorChemistryNotUnique {
            reading: reading.clone().unwrap_or_default(),
            count: identifier("count"),
            reactions: identifier("reactions"),
        },
        "chemistry_impossible" => Message::ErrorChemistryImpossible {
            reading: reading.clone().unwrap_or_default(),
        },
        "chemistry_takes_no_part" => Message::ErrorChemistryTakesNoPart {
            reading: reading.clone().unwrap_or_default(),
            species: identifier("species"),
        },
        "chemistry_other_side" => Message::ErrorChemistryOtherSide {
            reading: reading.clone().unwrap_or_default(),
            species: identifier("species"),
        },
        "chemistry_molar_mass_needs_a_substance" => {
            Message::ErrorChemistryMolarMassNeedsASubstance {
                reading: reading.clone().unwrap_or_default(),
            }
        }
        "chemistry_no_standard_atomic_weight" => Message::ErrorChemistryNoStandardAtomicWeight {
            reading: reading.clone().unwrap_or_default(),
            element: identifier("element"),
        },
        "chemistry_molar_mass_of_an_ion" => Message::ErrorChemistryMolarMassOfAnIon {
            reading: reading.clone().unwrap_or_default(),
        },
        "chemistry_molar_mass_of_no_element" => Message::ErrorChemistryMolarMassOfNoElement {
            reading: reading.clone().unwrap_or_default(),
        },
        "radix_decimal" => Message::CommonNoteRadixDecimal {
            value: identifier("value"),
        },
        "radix_written_decimal" => Message::CommonNoteRadixWrittenDecimal,
        "taylor_polynomial" => Message::CommonNoteTaylorPolynomial,
        "taylor_not_analytic" => Message::ErrorTaylorNotAnalytic,
        "taylor_order_not_whole" => Message::ErrorTaylorOrderNotWhole,
        "taylor_order_too_high" => Message::ErrorTaylorOrderTooHigh,
        "taylor_not_defined_at_point" => Message::ErrorTaylorNotDefinedAtPoint,
        "free_names" => Message::CommonNoteFreeNames {
            names: identifier("names"),
        },
        "overflowed_to_infinity" => Message::CommonNoteOverflowedToInfinity,
        "not_a_number_from_finite_operands" => Message::CommonNoteNotANumberFromFiniteOperands,
        "bound_not_smaller_than_value" => Message::CommonNoteBoundNotSmallerThanValue {
            conversion: identifier(CONVERSION_DATA),
        },
        "undefined_name" if UNNAMED_GENERATORS.contains(&identifier(NAME_DATA).as_str()) => {
            Message::ErrorUndefinedUnnamedGenerator {
                name: identifier(NAME_DATA),
            }
        }
        "undefined_name" => match diagnostic.data.get(CONSTANT_DATA) {
            Some(ParameterValue::Identifier(constant)) => Message::ErrorUndefinedNameConstant {
                name: identifier(NAME_DATA),
                constant: constant.clone(),
            },
            _ => Message::ErrorUndefinedName {
                name: identifier(NAME_DATA),
            },
        },
        RECOGNISED_ATTEMPT_CODE if identifier(ATTEMPT_DATA) == UNIT_ENDED_AT_SPACE_ATTEMPT => {
            match diagnostic.data.get(CORRECTED_DATA) {
                Some(ParameterValue::Identifier(corrected)) => Message::ErrorUnitEndedAtSpace {
                    name: identifier(NAME_DATA),
                    replacement: identifier(REPLACEMENT_DATA),
                    corrected: corrected.clone(),
                    column: identifier(COLUMN_DATA),
                },
                _ => Message::ErrorUnitEndedAtSpacePlain {
                    name: identifier(NAME_DATA),
                    replacement: identifier(REPLACEMENT_DATA),
                    column: identifier(COLUMN_DATA),
                },
            }
        }
        "ambiguous_temperature_sign" => Message::ErrorParseAmbiguousTemperatureSign {
            written: identifier(WRITTEN_DATA),
            reading: identifier(READING_DATA),
            difference: identifier(DIFFERENCE_DATA),
            column: identifier(COLUMN_DATA),
        },
        "ambiguous_application" => Message::ErrorParseAmbiguousApplication {
            written: identifier(WRITTEN_DATA),
            narrow: identifier(NARROW_DATA),
            wide: identifier(WIDE_DATA),
            column: identifier(COLUMN_DATA),
        },
        "parse_error" => Message::ErrorInputNotParsed {
            column: identifier(COLUMN_DATA),
        },
        "dependency_failed" => Message::ErrorDependencyFailed {
            line: identifier(LINE_DATA),
        },
        "reference_cycle" => Message::ErrorReferenceCycle {
            line: identifier(LINE_DATA),
        },
        "relation_is_a_claim" => {
            let reading = reading.unwrap_or_default();
            match diagnostic.data.get("names") {
                Some(ParameterValue::Identifier(names)) => Message::ErrorRelationAboutFreeNames {
                    reading,
                    names: names.clone(),
                },
                _ => Message::ErrorRelationIsAClaim { reading },
            }
        }
        "division_by_zero" => read_message(
            reading,
            |reading| Message::ErrorDivisionByZero { reading },
            Message::ErrorDivisionByZeroPlain,
        ),
        "outside_domain" => domain_message(diagnostic, reading),
        "remainder_not_computed" => match reading {
            Some(reading) => Message::ErrorRemainderNotComputed { reading },
            None => Message::ErrorRemainderNotComputedPlain,
        },
        "power_of_zero_without_value" => match reading {
            Some(reading) => Message::ErrorPowerOfZeroWithoutValue { reading },
            None => Message::ErrorPowerOfZeroWithoutValuePlain,
        },
        "power_of_zero_sign_undecided" => match reading {
            Some(reading) => Message::ErrorPowerOfZeroSignUndecided { reading },
            None => Message::ErrorPowerOfZeroSignUndecidedPlain,
        },
        "not_in_radical_field" => {
            let operator = data_identifier(diagnostic, OPERATOR_DATA);
            match reading {
                Some(reading) => Message::ErrorNotInRadicalField { operator, reading },
                None => Message::ErrorNotInRadicalFieldPlain { operator },
            }
        }
        "not_a_square_root_term" => match (reading, data_digits(diagnostic, ARGUMENT_DATA)) {
            (Some(reading), Some(radicand)) => {
                Message::ErrorNotASquareRootTermMultiple { reading, radicand }
            }
            (Some(reading), None) => Message::ErrorNotASquareRootTerm { reading },
            (None, _) => Message::ErrorNotASquareRootTermPlain,
        },
        "integer_form" => {
            let reading = reading.unwrap_or_default();
            match data_identifier(diagnostic, "problem").as_str() {
                "negative" => Message::ErrorIntegerNegative { reading },
                "shift_too_large" => Message::ErrorIntegerShiftTooLarge {
                    reading,
                    limit: data_identifier(diagnostic, "limit"),
                },
                "not_whole_bytes" => Message::ErrorIntegerNotWholeBytes {
                    reading,
                    kind: data_identifier(diagnostic, "type"),
                },
                "outside_type" => Message::ErrorIntegerOutsideType {
                    reading,
                    kind: data_identifier(diagnostic, "type"),
                    low: data_value(diagnostic, "low").unwrap_or_default(),
                    high: data_value(diagnostic, "high").unwrap_or_default(),
                },
                _ => Message::ErrorIntegerNotWhole { reading },
            }
        }
        "integer_argument_out_of_range" => argument_message(diagnostic, reading),
        "unsupported_operator" => match reading {
            Some(reading) => Message::ErrorUnsupportedOperator { reading },
            None => Message::ErrorUnsupportedOperatorPlain {
                operator: identifier(OPERATOR_DATA),
            },
        },
        "unsupported_subexpression" => read_message(
            reading,
            |reading| Message::ErrorUnsupportedSubexpression { reading },
            Message::ErrorUnsupportedExpression,
        ),
        "unsupported_expression" | "not_lowerable" => Message::ErrorUnsupportedExpression,
        "limit_not_finite" => match diagnostic.data.get(SIDE_DATA) {
            Some(ParameterValue::Identifier(side)) => match side.as_str() {
                LEFT_SIDE => Message::ErrorLimitNotFiniteFromLeft,
                RIGHT_SIDE => Message::ErrorLimitNotFiniteFromRight,
                _ => Message::ErrorLimitNotFinite,
            },
            _ => Message::ErrorLimitNotFinite,
        },
        "limit_not_a_rational_function" => Message::ErrorLimitNotARationalFunction,
        "root_not_a_polynomial" => Message::ErrorRootNotAPolynomial,
        "root_of_zero" => Message::ErrorRootOfZero,
        "root_index_not_whole" => Message::ErrorRootIndexNotWhole,
        "root_none_real" => Message::ErrorRootNoneReal,
        "root_index_out_of_range" => Message::ErrorRootIndexOutOfRange {
            count: identifier(COUNT_DATA),
        },
        "limit_undecided" => Message::ErrorLimitUndecided,
        "integrand_not_a_polynomial" => Message::ErrorIntegrandNotAPolynomial,
        "integral_without_bounds" => Message::ErrorIntegralWithoutBounds,
        "integrand_pole_in_interval" => Message::ErrorIntegrandPoleInInterval,
        "integrand_high_degree_factor" => Message::ErrorIntegrandHighDegreeFactor {
            degree: identifier(COUNT_DATA),
        },
        "integrand_repeated_quadratic" => Message::ErrorIntegrandRepeatedQuadratic,
        "integral_bound_not_exact" => Message::ErrorIntegralBoundNotExact,
        "array_shapes_differ" => Message::ErrorArrayShapesDiffer,
        "array_not_square" => Message::ErrorArrayNotSquare,
        "array_not_invertible" => Message::ErrorArrayNotInvertible,
        "array_entries_not_exact" => Message::ErrorArrayEntriesNotExact,
        "array_entries_not_rational" => Message::ErrorArrayEntriesNotRational,
        "array_eigenvalues_not_real" => Message::ErrorArrayEigenvaluesNotReal,
        "array_eigenvalues_not_radical" => Message::ErrorArrayEigenvaluesNotRadical,
        "array_empty" => match operation_of(diagnostic) {
            Some(TRANSPOSE_OPERATION) => Message::ErrorArrayEmptyTranspose,
            Some(DETERMINANT_OPERATION) => Message::ErrorArrayEmptyDeterminant,
            _ => Message::ErrorArrayEmpty,
        },
        "array_not_a_matrix" => match operation_of(diagnostic) {
            Some(DETERMINANT_OPERATION) => Message::ErrorArrayNotAMatrixDeterminant,
            Some(TRANSPOSE_OPERATION) => Message::ErrorArrayNotAMatrixTranspose,
            _ => Message::ErrorArrayNotAMatrix,
        },
        "array_position_not_whole" => Message::ErrorArrayPositionNotWhole,
        "array_position_outside" => Message::ErrorArrayPositionOutside,
        "array_not_comparable" => match (
            data_value(diagnostic, FIRST_DATA),
            data_value(diagnostic, SECOND_DATA),
        ) {
            (Some(first), Some(second)) => Message::ErrorArrayNotOrdered { first, second },
            _ => Message::ErrorArrayNotComparable,
        },
        "sort_not_comparable" => match (
            data_value(diagnostic, FIRST_DATA),
            data_value(diagnostic, SECOND_DATA),
            data_value(diagnostic, COMPARISONS_DATA).and_then(|count| count.parse().ok()),
            data_value(diagnostic, WRITES_DATA).and_then(|count| count.parse().ok()),
        ) {
            (Some(first), Some(second), Some(comparisons), Some(writes)) => {
                Message::ErrorSortNotOrdered {
                    first,
                    second,
                    comparisons,
                    writes,
                }
            }
            _ => Message::ErrorArrayNotComparable,
        },
        "array_records_need_key" => Message::ErrorArrayRecordsNeedKey,
        "array_key_not_whole" => match data_value(diagnostic, FIRST_DATA) {
            Some(key) => Message::ErrorArrayKeyNotWhole {
                key,
                method: data_identifier(diagnostic, METHOD_DATA),
            },
            None => Message::ErrorArrayKeyNotWholeEntry {
                method: data_identifier(diagnostic, METHOD_DATA),
            },
        },
        "generator_seed_machine" => Message::ErrorGeneratorSeedMachine {
            value: data_value(diagnostic, FIRST_DATA).unwrap_or_default(),
        },
        "generator_stream_machine" => Message::ErrorGeneratorStreamMachine {
            value: data_value(diagnostic, FIRST_DATA).unwrap_or_default(),
        },
        "generator_index_machine" => Message::ErrorGeneratorIndexMachine {
            value: data_value(diagnostic, FIRST_DATA).unwrap_or_default(),
        },
        "generator_seed_refused" => Message::ErrorGeneratorSeedRefused {
            value: data_value(diagnostic, FIRST_DATA).unwrap_or_default(),
        },
        "generator_stream_refused" => Message::ErrorGeneratorStreamRefused {
            value: data_value(diagnostic, FIRST_DATA).unwrap_or_default(),
        },
        "generator_index_refused" => Message::ErrorGeneratorIndexRefused {
            value: data_value(diagnostic, FIRST_DATA).unwrap_or_default(),
        },
        "generator_layout" => Message::CommonGeneratorLayout {
            seed: data_value(diagnostic, "seed").unwrap_or_default(),
            stream: data_value(diagnostic, "stream").unwrap_or_default(),
            index: data_value(diagnostic, "index").unwrap_or_default(),
        },
        "generator_statistical" => Message::CommonGeneratorStatistical,
        "array_key_with_unit" => match data_value(diagnostic, FIRST_DATA) {
            Some(key) => Message::ErrorArrayKeyWithUnit {
                key,
                method: data_identifier(diagnostic, METHOD_DATA),
            },
            None => Message::ErrorArrayKeyWithUnitEntry {
                method: data_identifier(diagnostic, METHOD_DATA),
            },
        },
        "array_function_key_machine" => match data_value(diagnostic, FIRST_DATA) {
            Some(key) => Message::ErrorArrayFunctionKeyMachine {
                key,
                method: data_identifier(diagnostic, METHOD_DATA),
            },
            None => Message::ErrorArrayFunctionKeyMachineEntry {
                method: data_identifier(diagnostic, METHOD_DATA),
            },
        },
        "array_range_too_wide" => Message::ErrorArrayRangeTooWide,
        "sort_base_too_small" => Message::ErrorSortBaseTooSmall,
        "sort_bead_no_key" => Message::ErrorSortBeadNoKey,
        "sort_keys_differ_in_dimension" | "sort_keyed_keys_differ_in_dimension" => {
            keys_differ_in_dimension(diagnostic)
        }
        "sort_limit_reached" => Message::ErrorSortLimitReached {
            limit: data_value(diagnostic, LIMIT_DATA).unwrap_or_default(),
            comparisons: data_value(diagnostic, "comparisons").unwrap_or_default(),
            writes: data_value(diagnostic, "writes").unwrap_or_default(),
            draws: data_value(diagnostic, "draws").unwrap_or_default(),
        },
        "sort_length_not_power_of_two" => Message::ErrorSortLengthNotPowerOfTwo {
            length: data_value(diagnostic, "length").unwrap_or_default(),
            below: data_value(diagnostic, "below").unwrap_or_default(),
            above: data_value(diagnostic, "above").unwrap_or_default(),
        },
        "sort_bead_below_zero" => Message::ErrorSortBeadBelowZero {
            key: data_value(diagnostic, FIRST_DATA).unwrap_or_default(),
        },
        "sort_too_many_beads" => Message::ErrorSortTooManyBeads {
            beads: data_value(diagnostic, "beads").unwrap_or_default(),
            limit: data_value(diagnostic, LIMIT_DATA).unwrap_or_default(),
        },
        "sort_base_too_large" => Message::ErrorSortBaseTooLarge {
            limit: data_value(diagnostic, LIMIT_DATA).unwrap_or_default(),
        },
        "sort_range_too_wide" => match (
            data_value(diagnostic, RANGE_DATA),
            data_value(diagnostic, LIMIT_DATA),
        ) {
            (Some(range), Some(limit)) => Message::ErrorSortRangeTooWide { range, limit },
            _ => Message::ErrorArrayRangeTooWide,
        },
        "array_entry_without_value" => Message::ErrorArrayEntryWithoutValue,
        "order_not_real" => Message::ErrorOrderNotReal {
            side: identifier("side"),
        },
        "array_not_real" => match data_value(diagnostic, FIRST_DATA) {
            Some(entry) => Message::ErrorArrayNotReal { entry },
            None => Message::ErrorArrayNotRealEntry,
        },
        "array_not_a_list" => Message::ErrorArrayNotAList,
        "array_machine_entry" => match reading {
            Some(reading) => Message::ErrorArrayMachineEntry { reading },
            None => Message::ErrorArrayMachineEntryPlain,
        },
        "array_units_differ" => Message::ErrorArrayUnitsDiffer,
        "array_ranks_differ" => Message::ErrorArrayRanksDiffer,
        "array_nested" => Message::ErrorArrayNested,
        "array_body_and_point" => Message::ErrorArrayBodyAndPoint,
        "array_power" => Message::ErrorArrayPower,
        "array_product_of_lists" => Message::ErrorArrayProductOfLists,
        "array_not_rectangular" => Message::ErrorArrayNotRectangular,
        "array_unsupported" => Message::ErrorArrayUnsupported,
        "unsupported_constant" => read_message(
            reading,
            |reading| Message::ErrorUnsupportedConstant { reading },
            Message::ErrorUnsupportedConstantPlain,
        ),
        "not_finite" => read_message(
            reading,
            |reading| Message::ErrorNotFinite { reading },
            Message::ErrorNotFinitePlain,
        ),
        "machine_number_in_exact_evaluation" => read_message(
            reading,
            |reading| Message::ErrorMachineNumberInExact { reading },
            Message::ErrorMachineNumberInExactPlain,
        ),
        "expression_pool_full" => Message::ErrorExpressionTooLarge,
        "result_too_large" | "intermediate_step_too_large" | "number_too_large" => {
            size_message(diagnostic, reading)
        }
        "index_range_too_long" => read_message(
            reading,
            |reading| Message::ErrorIndexRangeTooLong { reading },
            Message::ErrorIndexRangeTooLongPlain,
        ),
        "dimension_mismatch" => match (reading, diagnostic.data.get("left_quantity")) {
            (Some(reading), Some(_)) => dimension_mismatch_sides(diagnostic, reading),
            (reading, _) => read_message(
                reading,
                |reading| Message::ErrorDimensionMismatch { reading },
                Message::ErrorDimensionMismatchPlain,
            ),
        },
        "dimensioned_argument" => read_message(
            reading,
            |reading| Message::ErrorDimensionedArgument { reading },
            Message::ErrorDimensionedArgumentPlain,
        ),
        "dimensioned_power_exponent_not_constant" => read_message(
            reading,
            |reading| Message::ErrorDimensionedPowerExponentNotConstant { reading },
            Message::ErrorDimensionedPowerExponentNotConstantPlain,
        ),
        "fractional_dimension" => read_message(
            reading,
            |reading| Message::ErrorFractionalDimension { reading },
            Message::ErrorFractionalDimensionPlain,
        ),
        "dimension_out_of_range" => match reading {
            Some(reading) => Message::ErrorDimensionOutOfRange {
                reading,
                lowest: i8::MIN.to_string(),
                highest: i8::MAX.to_string(),
            },
            None => Message::ErrorDimensionOutOfRangePlain,
        },
        "no_backend_selected" => Message::ErrorNoBackend,
        "precision_not_supported" => Message::ErrorPrecisionNotSupported {
            precision: identifier(PRECISION_DATA),
        },
        "result_kind_not_representable" => Message::ErrorResultKindNotRepresentable {
            kind: identifier(KIND_DATA),
        },
        "result_not_representable" => Message::ErrorResultNotRepresentable,
        "solve_failed" => Message::ErrorSolveFailed,
        "negative_uncertainty" => read_message(
            reading,
            |reading| Message::ErrorNegativeUncertainty { reading },
            Message::ErrorNegativeUncertaintyPlain,
        ),
        "nested_uncertainty" => read_message(
            reading,
            |reading| Message::ErrorNestedUncertainty { reading },
            Message::ErrorNestedUncertaintyPlain,
        ),
        "coverage_factor_below_one" => read_message(
            reading,
            |reading| Message::ErrorCoverageFactorBelowOne { reading },
            Message::ErrorCoverageFactorBelowOnePlain,
        ),
        "uncertainty_not_propagated" => Message::ErrorUncertaintyNotPropagated,
        code => Message::ErrorInternal {
            code: code.to_owned(),
        },
    }
}

pub fn view_error_message(error: &ViewError) -> Message {
    match error {
        ViewError::UnknownLine(line) => Message::ErrorUnknownLine {
            line: line_label(*line),
        },
        ViewError::LineRefused { diagnostic, .. } => diagnostic_message(diagnostic),
        ViewError::NotApplicable {
            line,
            view: ViewKind::Digits,
        } => Message::ErrorDigitsNotApplicable {
            line: line_label(*line),
        },
        ViewError::NotApplicable {
            line,
            view: ViewKind::Enclose,
        } => Message::ErrorEncloseNotApplicable {
            line: line_label(*line),
        },
        ViewError::NotApplicable {
            line,
            view: ViewKind::Working,
        } => Message::ErrorWorkingNotApplicable {
            line: line_label(*line),
        },
        ViewError::HoldsFreeNames { line, names } => Message::ErrorHoldsFreeNames {
            line: line_label(*line),
            names: names.join(", "),
        },
        ViewError::PlacesAboveLimit { places, limit } => Message::ErrorPlacesAboveLimit {
            places: places.to_string(),
            limit: limit.to_string(),
        },
        ViewError::SignificantDigitsOutOfRange { digits, limit } => {
            Message::ErrorSignificantDigitsOutOfRange {
                digits: digits.to_string(),
                limit: limit.to_string(),
            }
        }
        ViewError::OverBudget { budget_bits } => Message::ErrorEnclosureOverBudget {
            bits: budget_bits.to_string(),
        },
        ViewError::Undetermined { places, digits } => Message::ErrorDigitsUndetermined {
            places: places.to_string(),
            digits: digits.to_string(),
        },
    }
}

fn position_text(index: usize) -> String {
    index.saturating_add(1).to_string()
}

pub fn plot_error_message(error: &PlotError) -> Message {
    match error {
        PlotError::UnknownLine(line) => Message::ErrorUnknownLine {
            line: line_label(*line),
        },
        PlotError::NotPlottable(line) => Message::ErrorPlotNotPlottable {
            line: line_label(*line),
        },
        PlotError::LineNumberTooLarge(line) => Message::ErrorPlotLineNumberTooLarge {
            line: line_label(*line),
        },
        PlotError::Expansion(diagnostic) => diagnostic_message(diagnostic),
        PlotError::ViewCount { expected, found } => Message::ErrorPlotViewCount {
            expected: expected.to_string(),
            found: found.to_string(),
        },
        PlotError::ViewAxisCount {
            expected, found, ..
        } => Message::ErrorPlotViewAxisCount {
            expected: expected.to_string(),
            found: found.to_string(),
        },
        PlotError::UnknownUnit { axis, .. } => Message::ErrorPlotUnknownUnit {
            axis: position_text(*axis),
        },
        PlotError::UnitOfOtherDimension { axis, .. } => Message::ErrorPlotUnitOfOtherDimension {
            axis: position_text(*axis),
        },
        PlotError::RangeMissing { view, axis } => Message::ErrorPlotRangeMissing {
            view: position_text(*view),
            axis: position_text(*axis),
        },
        PlotError::EmptyInterval { axis, .. } => Message::ErrorPlotEmptyInterval {
            axis: position_text(*axis),
        },
        PlotError::UnknownParameter(name) => {
            Message::ErrorPlotUnknownParameter { name: name.clone() }
        }
        PlotError::NoAxisLeft(line) => Message::ErrorPlotNoAxisLeft {
            line: line_label(*line),
        },
        PlotError::DivisionsMissing { axis, .. } => Message::ErrorPlotDivisionsMissing {
            axis: position_text(*axis),
        },
        PlotError::IterationLimitNotApplicable(line) => {
            Message::ErrorPlotIterationLimitNotApplicable {
                line: line_label(*line),
            }
        }
        PlotError::ParametersNotApplicable(line) => Message::ErrorPlotParametersNotApplicable {
            line: line_label(*line),
        },
        PlotError::ViewKindMismatch(line) => Message::ErrorPlotViewKindMismatch {
            line: line_label(*line),
        },
        PlotError::ValueKindNotPlottable(line) => Message::ErrorPlotValueKindNotPlottable {
            line: line_label(*line),
        },
        PlotError::TooManyAxisVariables(line) => Message::ErrorPlotTooManyAxisVariables {
            line: line_label(*line),
        },
        PlotError::Sample(error) => sample_error_message(error),
    }
}

pub fn recognition_unavailable_message(reason: RecognitionUnavailable) -> Message {
    match reason {
        RecognitionUnavailable::ConceptSetNotLoaded => Message::CommonRecognizedNoConceptSet,
        RecognitionUnavailable::PatternsNotBuilt => Message::CommonRecognizedNoPatterns,
        RecognitionUnavailable::MatcherFailed => Message::CommonRecognizedMatcherFailed,
    }
}

pub fn read_error_message(error: &ReadError) -> Message {
    match error {
        ReadError::SceneNotComplete => Message::ErrorReadSceneNotComplete,
        ReadError::LayerHasNoReadings { layer } => Message::ErrorReadLayerHasNoReadings {
            layer: layer.to_string(),
        },
        ReadError::CoordinateCount { expected, found } => Message::ErrorReadCoordinateCount {
            expected: expected.to_string(),
            found: found.to_string(),
        },
        ReadError::CoordinateNotExact { axis } => Message::ErrorReadCoordinateNotExact {
            axis: position_text(*axis),
        },
        ReadError::ReadingNotBuilt => Message::ErrorReadNotBuilt,
        ReadError::ReadingNeedsFunctionForm(line) => Message::ErrorReadNeedsFunctionForm {
            line: line_label(*line),
        },
        ReadError::Plot(error) => plot_error_message(error),
        ReadError::NotEntered(error) => session_error_message(error, ""),
    }
}

pub fn sample_error_message(error: &SampleError) -> Message {
    let internal = |code: &str| Message::ErrorPlotSamplingFailed {
        code: code.to_owned(),
    };
    match error {
        SampleError::SampleLimitExceeded { requested, limit } => Message::ErrorPlotSampleLimit {
            requested: requested.to_string(),
            limit: limit.to_string(),
        },
        SampleError::MeshTooLarge { vertices } => Message::ErrorPlotMeshTooLarge {
            vertices: vertices.to_string(),
        },
        SampleError::IterationLimitTooLarge { iterations } => {
            Message::ErrorPlotIterationLimitTooLarge {
                iterations: iterations.to_string(),
            }
        }
        SampleError::EmptyInterval { axis } => Message::ErrorPlotEmptyInterval {
            axis: position_text(*axis),
        },
        SampleError::Lower(_) => Message::ErrorPlotNotLowerable,
        SampleError::Select(_) => Message::ErrorNoBackend,
        SampleError::Quantity(_) | SampleError::ValueDimensionMismatch { .. } => {
            Message::ErrorDimensionMismatchPlain
        }
        SampleError::AxisCount { .. } => internal("axis_count"),
        SampleError::BoundEvaluation { .. } => internal("bound_evaluation"),
        SampleError::BoundNotRational { .. } => internal("bound_not_rational"),
        SampleError::NoDivisions { .. } => internal("no_divisions"),
        SampleError::BoundNotExact { .. } => internal("bound_not_exact"),
        SampleError::ComplexAxesDiffer => internal("complex_axes_differ"),
        SampleError::ExpressionNotDetachable(_) => internal("expression_not_detachable"),
        SampleError::AxisUnitNotSupported { .. } => internal("axis_unit_not_supported"),
        SampleError::ConversionNotBuilt(_) => internal("conversion_not_built"),
        SampleError::SymbolNotDetachable(_) => internal("symbol_not_detachable"),
        SampleError::ReducedExpression(_) => internal("reduced_expression"),
        SampleError::Run(_) => internal("backend_run_failed"),
        SampleError::Scene(_) => internal("scene_not_valid"),
        SampleError::RunFinished => internal("run_finished"),
        SampleError::EscapeTimeSettingsMissing => internal("escape_time_settings_missing"),
        SampleError::StageOrder => internal("stage_order"),
        SampleError::SamplesPending => internal("samples_pending"),
        SampleError::RefinementNeedsEscapeTime => internal("refinement_needs_escape_time"),
    }
}

fn defined_name_in(input: &str) -> Option<String> {
    let (name, _) = input.split_once('=')?;
    let name = name.trim();
    let is_a_name = !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || "_(), ".contains(character));
    is_a_name.then(|| name.to_owned())
}

pub fn session_error_json_line(error: &SessionError, input: &str, number: usize) -> Vec<u8> {
    let value = match error {
        SessionError::Parse(parse_error) => parse_error_value(parse_error, input),
        other => {
            let (code, data) = session_error_data(other, input);
            let data: Vec<(&str, &str)> = data
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect();
            crate::solve::typed_error_value(code, &data)
        }
    };
    let value = naming_its_input(value, input);
    let Json::Object(mut members) = value else {
        return crate::json::write_one_line(&value).into_bytes();
    };
    members.push((
        LINE_NUMBER_MEMBER.to_owned(),
        Json::string(&number.to_string()),
    ));
    crate::json::write_one_line(&Json::Object(members)).into_bytes()
}

fn session_error_data(
    error: &SessionError,
    input: &str,
) -> (&'static str, Vec<(&'static str, String)>) {
    match error {
        SessionError::Parse(_) => (PARSE_ERROR_CODE, Vec::new()),
        SessionError::Symbol(SymbolError::KindConflict { .. }) => {
            ("name_kind_conflict", Vec::new())
        }
        SessionError::Symbol(SymbolError::TableFull) => ("expression_too_large", Vec::new()),
        SessionError::Symbol(SymbolError::UnknownSymbolId(_)) => ("internal", Vec::new()),
        SessionError::NameExists(name) => ("name_exists", vec![("name", name.clone())]),
        SessionError::NameInUse { line, dependents } => (
            "name_in_use",
            vec![
                ("line", line_label(*line)),
                (
                    "dependents",
                    dependents
                        .iter()
                        .map(|dependent| line_label(*dependent))
                        .collect::<Vec<_>>()
                        .join(LIST_SEPARATOR),
                ),
            ],
        ),
        SessionError::ReferenceCycle(line) => (
            "reference_cycle",
            vec![(
                "name",
                defined_name_in(input).unwrap_or_else(|| line_label(*line)),
            )],
        ),
        SessionError::UnknownLine(line) => ("unknown_line", vec![("line", line_label(*line))]),
        SessionError::LineNumbersExhausted => ("line_numbers_exhausted", Vec::new()),
        SessionError::InvalidRequest(request_error) => (
            "invalid_request",
            vec![
                ("reason", request_error.diagnostic().code),
                ("path", json_path_text(&request_error.path)),
            ],
        ),
        SessionError::NotASolveLine(line) => {
            ("not_a_solve_line", vec![("line", line_label(*line))])
        }
        SessionError::MachineLineNotApplicable(line) => (
            "machine_line_not_applicable",
            vec![("line", line_label(*line))],
        ),
        SessionError::HoldsFreeNames { line, names } => (
            "holds_free_names",
            vec![("line", line_label(*line)), ("names", names.join(", "))],
        ),
    }
}

pub fn session_error_message(error: &SessionError, input: &str) -> Message {
    match error {
        SessionError::Parse(parse_error) => parse_error_message(parse_error, input),
        SessionError::Symbol(SymbolError::KindConflict { .. }) => Message::ErrorNameKindConflict,
        SessionError::Symbol(SymbolError::TableFull) => Message::ErrorExpressionTooLarge,
        SessionError::Symbol(SymbolError::UnknownSymbolId(_)) => Message::ErrorInternal {
            code: PARSE_SYMBOL_ERROR_CODE.to_owned(),
        },
        SessionError::NameExists(name) => Message::ErrorNameExists { name: name.clone() },
        SessionError::NameInUse { line, dependents } => Message::ErrorNameInUse {
            line: line_label(*line),
            dependents: dependents
                .iter()
                .map(|dependent| line_label(*dependent))
                .collect::<Vec<_>>()
                .join(LIST_SEPARATOR),
        },
        SessionError::ReferenceCycle(line) => Message::ErrorReferenceCycle {
            line: defined_name_in(input).unwrap_or_else(|| line_label(*line)),
        },
        SessionError::UnknownLine(line) => Message::ErrorUnknownLine {
            line: line_label(*line),
        },
        SessionError::LineNumbersExhausted => Message::ErrorLineNumbersExhausted,
        SessionError::InvalidRequest(request_error) => {
            let diagnostic = request_error.diagnostic();
            Message::ErrorInvalidRequest {
                code: diagnostic.code,
                path: json_path_text(&request_error.path),
            }
        }
        SessionError::NotASolveLine(line) => Message::ErrorNotASolveLine {
            line: line_label(*line),
        },
        SessionError::MachineLineNotApplicable(line) => Message::ErrorMachineLineNotApplicable {
            line: line_label(*line),
        },
        SessionError::HoldsFreeNames { line, names } => Message::ErrorHoldsFreeNames {
            line: line_label(*line),
            names: names.join(", "),
        },
    }
}

const AMBIGUOUS_APPLICATION_CODE: &str = "ambiguous_application";
const AMBIGUOUS_TEMPERATURE_SIGN_CODE: &str = "ambiguous_temperature_sign";
const FUNCTION_DATA: &str = "function";
const SIGN_DATA: &str = "sign";
const RECOGNISED_ATTEMPT_CODE: &str = "recognised_attempt";

pub fn recognised_attempt_json(error: &ParseError, input: &str) -> Option<Vec<u8>> {
    recognised_attempt_value(error, input)
        .map(|value| crate::json::write_canonical(&value).into_bytes())
}

fn recognised_attempt_value(error: &ParseError, input: &str) -> Option<Json> {
    let ParseErrorKind::RecognisedAttempt(attempt) = error.kind else {
        return None;
    };
    let column = input
        .get(..error.span.start)
        .map_or(FIRST_COLUMN, |before| before.chars().count() + FIRST_COLUMN)
        .to_string();
    let corrected = corrected_text(input).unwrap_or_default();
    Some(crate::solve::typed_error_value(
        RECOGNISED_ATTEMPT_CODE,
        &[
            (ATTEMPT_DATA, attempt.name()),
            (COLUMN_DATA, &column),
            (CORRECTED_DATA, &corrected),
            (REPLACEMENT_DATA, attempt.replacement().unwrap_or_default()),
        ],
    ))
}

const PARSE_ERROR_CODE: &str = "parse_error";

pub fn parse_error_json(error: &ParseError, input: &str) -> Vec<u8> {
    crate::json::write_canonical(&parse_error_value(error, input)).into_bytes()
}

pub fn parse_error_json_naming_input(error: &ParseError, input: &str) -> Vec<u8> {
    crate::json::write_canonical(&naming_its_input(parse_error_value(error, input), input))
        .into_bytes()
}

const LINE_NUMBER_MEMBER: &str = "line_number";

fn naming_its_input(value: Json, input: &str) -> Json {
    let Json::Object(mut members) = value else {
        return value;
    };
    let after_code = members
        .iter()
        .position(|(name, _)| name == "code")
        .map_or(0, |position| position + 1);
    members.insert(after_code, ("input".to_owned(), Json::string(input)));
    Json::Object(members)
}

pub fn parse_error_json_line(error: &ParseError, input: &str) -> Vec<u8> {
    crate::json::write_one_line(&naming_its_input(parse_error_value(error, input), input))
        .into_bytes()
}

fn parse_error_value(error: &ParseError, input: &str) -> Json {
    if let Some(value) = recognised_attempt_value(error, input) {
        return value;
    }
    let column = input
        .get(..error.span.start)
        .map_or(FIRST_COLUMN, |before| before.chars().count() + FIRST_COLUMN)
        .to_string();
    match error.kind {
        ParseErrorKind::AmbiguousApplication(application) => crate::solve::typed_error_value(
            AMBIGUOUS_APPLICATION_CODE,
            &[
                (COLUMN_DATA, &column),
                (FUNCTION_DATA, &application.function(input)),
                (NARROW_DATA, &application.narrow(input)),
                (WIDE_DATA, &application.wide(input)),
                (WRITTEN_DATA, &application.written(input)),
            ],
        ),
        ParseErrorKind::AmbiguousTemperatureSign(sign) => crate::solve::typed_error_value(
            AMBIGUOUS_TEMPERATURE_SIGN_CODE,
            &[
                (COLUMN_DATA, &column),
                (DIFFERENCE_DATA, &sign.difference(input)),
                (READING_DATA, &sign.reading(input)),
                (SIGN_DATA, sign.sign()),
                (WRITTEN_DATA, &sign.written(input)),
            ],
        ),
        ParseErrorKind::FractionBeforeUnit(fraction) => crate::solve::typed_error_value(
            PARSE_ERROR_CODE,
            &[
                (KIND_DATA, parse_error_kind_name(&error.kind)),
                (COLUMN_DATA, &column),
                (WRITTEN_DATA, &fraction.written(input)),
                ("fraction", &fraction.as_a_fraction(input)),
                ("reciprocal", &fraction.as_a_reciprocal(input)),
            ],
        ),
        ParseErrorKind::AmbiguousUnit => {
            let name = input.get(error.span.clone()).unwrap_or_default();
            let readings = calc_units::ambiguous_readings(name)
                .unwrap_or_default()
                .join(", ");
            crate::solve::typed_error_value(
                PARSE_ERROR_CODE,
                &[
                    (KIND_DATA, parse_error_kind_name(&error.kind)),
                    (COLUMN_DATA, &column),
                    (WRITTEN_DATA, name),
                    ("readings", &readings),
                ],
            )
        }
        ParseErrorKind::NestedTooDeeply { limit }
        | ParseErrorKind::ChainTooLong { limit }
        | ParseErrorKind::ExpressionTooDeep { limit } => crate::solve::typed_error_value(
            PARSE_ERROR_CODE,
            &[
                (KIND_DATA, parse_error_kind_name(&error.kind)),
                (COLUMN_DATA, &column),
                (LIMIT_DATA, &limit.to_string()),
            ],
        ),
        _ => crate::solve::typed_error_value(
            PARSE_ERROR_CODE,
            &[
                (KIND_DATA, parse_error_kind_name(&error.kind)),
                (COLUMN_DATA, &column),
            ],
        ),
    }
}

pub(crate) fn parse_error_kind_name(kind: &ParseErrorKind) -> &'static str {
    match kind {
        ParseErrorKind::UnexpectedCharacter => "unexpected_character",
        ParseErrorKind::UnexpectedToken => "unexpected_token",
        ParseErrorKind::UnexpectedEnd => "unexpected_end",
        ParseErrorKind::InvalidTypedLiteral => "invalid_typed_literal",
        ParseErrorKind::ExponentTooLarge => "exponent_too_large",
        ParseErrorKind::NotAUnit => "not_a_unit",
        ParseErrorKind::AtomicMassUnit => "atomic_mass_unit",
        ParseErrorKind::NotAUnitJoinedToAUnit => "not_a_unit_joined_to_a_unit",
        ParseErrorKind::AmbiguousUnit => "ambiguous_unit",
        ParseErrorKind::TypeExpected => "type_expected",
        ParseErrorKind::ByteOrderMissing => "byte_order_missing",
        ParseErrorKind::UnitExponentOutOfRange => "unit_exponent_out_of_range",
        ParseErrorKind::UnitExponentNotWhole => "unit_exponent_not_whole",
        ParseErrorKind::ReservedName => "reserved_name",
        ParseErrorKind::ChainedRelation => "chained_relation",
        ParseErrorKind::RaggedArray => "ragged_array",
        ParseErrorKind::UnknownKeyword => "unknown_keyword",
        ParseErrorKind::DuplicateKeyword => "duplicate_keyword",
        ParseErrorKind::PositionalAfterKeyword => "positional_after_keyword",
        ParseErrorKind::InvalidKeywordValue { .. } => "invalid_keyword_value",
        ParseErrorKind::MissingKeyword { .. } => "missing_keyword",
        ParseErrorKind::PivotNotBuiltForPartition => "pivot_not_built_for_partition",
        ParseErrorKind::MissingPivot { .. } => "missing_pivot",
        ParseErrorKind::MissingSeed => "missing_seed",
        ParseErrorKind::MissingBase => "missing_base",
        ParseErrorKind::MissingShuffleSeed => "missing_shuffle_seed",
        ParseErrorKind::MissingLimit => "missing_limit",
        ParseErrorKind::InvalidLimit => "invalid_limit",
        ParseErrorKind::InvalidBase => "invalid_base",
        ParseErrorKind::InvalidSeed => "invalid_seed",
        ParseErrorKind::SeedWithoutRandomPivot => "seed_without_random_pivot",
        ParseErrorKind::MissingDifferential => "missing_differential",
        ParseErrorKind::NestedTooDeeply { .. } => "nested_too_deeply",
        ParseErrorKind::ChainTooLong { .. } => "chain_too_long",
        ParseErrorKind::ExpressionTooDeep { .. } => "expression_too_deep",
        ParseErrorKind::ArityMismatch { .. } => "arity_mismatch",
        ParseErrorKind::Build(_) => "expression_not_built",
        ParseErrorKind::Symbol(_) => "symbol_not_available",
        ParseErrorKind::UnitProduct(_) => "unit_product_invalid",
        ParseErrorKind::AmbiguousApplication(_) => AMBIGUOUS_APPLICATION_CODE,
        ParseErrorKind::AmbiguousTemperatureSign(_) => "ambiguous_temperature_sign",
        ParseErrorKind::FractionBeforeUnit(_) => "fraction_before_unit",
        ParseErrorKind::PercentInASum(_) => "percent_in_a_sum",
        ParseErrorKind::Chemistry(_) => "chemistry",
        ParseErrorKind::Nuclear(_) => "nuclear",
        ParseErrorKind::RecognisedAttempt(_) => RECOGNISED_ATTEMPT_CODE,
    }
}

fn chemistry_parse_message(
    problem: calc_syntax::ChemistryProblem,
    name: String,
    column: String,
) -> Message {
    use calc_syntax::ChemistryProblem;
    match problem {
        ChemistryProblem::Empty => Message::ErrorParseChemistryEmpty { column },
        ChemistryProblem::UnknownElement => {
            Message::ErrorParseChemistryUnknownElement { name, column }
        }
        ChemistryProblem::WrongCase => Message::ErrorParseChemistryWrongCase { name, column },
        ChemistryProblem::AmbiguousCharge => {
            Message::ErrorParseChemistryAmbiguousCharge { name, column }
        }
        ChemistryProblem::SignBeforeNumber => {
            Message::ErrorParseChemistrySignBeforeNumber { name, column }
        }
        ChemistryProblem::AttachedNumber => {
            Message::ErrorParseChemistryAttachedNumber { name, column }
        }
        ChemistryProblem::OxidationState => {
            Message::ErrorParseChemistryOxidationState { name, column }
        }
        ChemistryProblem::SumWithoutSpaces => {
            Message::ErrorParseChemistrySumWithoutSpaces { name, column }
        }
        ChemistryProblem::DotSeparator => Message::ErrorParseChemistryDotSeparator { name, column },
        ChemistryProblem::DecimalCount => Message::ErrorParseChemistryDecimalCount { name, column },
        ChemistryProblem::Resonance => Message::ErrorParseChemistryResonance { name, column },
        ChemistryProblem::Nuclide => Message::ErrorParseChemistryNuclide { name, column },
        ChemistryProblem::Equilibrium => Message::ErrorParseChemistryEquilibrium { name, column },
        ChemistryProblem::FractionalCoefficient => {
            Message::ErrorParseChemistryFractionalCoefficient { name, column }
        }
        ChemistryProblem::TwoArrows => Message::ErrorParseChemistryTwoArrows { name, column },
        ChemistryProblem::EmptySide => Message::ErrorParseChemistryEmptySide { column },
        ChemistryProblem::UnclosedBracket => Message::ErrorParseChemistryUnclosedBracket { column },
        ChemistryProblem::UnexpectedCharacter => {
            Message::ErrorParseChemistryUnexpectedCharacter { name, column }
        }
        ChemistryProblem::CoefficientWithoutReaction => {
            Message::ErrorParseChemistryCoefficientWithoutReaction { name, column }
        }
        ChemistryProblem::WrongArrow => Message::ErrorParseChemistryWrongArrow { name, column },
        ChemistryProblem::NoArrow => Message::ErrorParseChemistryNoArrow { column },
        ChemistryProblem::RepeatedSpecies => {
            Message::ErrorParseChemistryRepeatedSpecies { name, column }
        }
        ChemistryProblem::ZeroCount => Message::ErrorParseChemistryZeroCount { name, column },
        ChemistryProblem::ZeroCoefficient => {
            Message::ErrorParseChemistryZeroCoefficient { name, column }
        }
        ChemistryProblem::ZeroCharge => Message::ErrorParseChemistryZeroCharge { name, column },
        ChemistryProblem::CountTooLarge => {
            Message::ErrorParseChemistryCountTooLarge { name, column }
        }
        ChemistryProblem::Isotope => Message::ErrorParseChemistryIsotope { name, column },
        ChemistryProblem::DanglingHydrateDot => {
            Message::ErrorParseChemistryDanglingHydrateDot { name, column }
        }
        ChemistryProblem::CoefficientOutsideLiteral => {
            Message::ErrorParseChemistryCoefficientOutsideLiteral { name, column }
        }
    }
}

fn nuclear_parse_message(
    problem: calc_syntax::NuclearProblem,
    name: String,
    column: String,
) -> Message {
    use calc_syntax::NuclearProblem;
    match problem {
        NuclearProblem::Empty => Message::ErrorParseNuclearEmpty { column },
        NuclearProblem::UnknownSpecies => Message::ErrorParseNuclearUnknownSpecies { name, column },
        NuclearProblem::AttachedNumber => Message::ErrorParseNuclearAttachedNumber { name, column },
        NuclearProblem::AtomSymbol => Message::ErrorParseNuclearAtomSymbol { name, column },
        NuclearProblem::BareElement => Message::ErrorParseNuclearBareElement { name, column },
        NuclearProblem::ParticleOrElement => {
            Message::ErrorParseNuclearParticleOrElement { name, column }
        }
        NuclearProblem::SchoolLetter => Message::ErrorParseNuclearSchoolLetter { name, column },
        NuclearProblem::UnsignedElectron => {
            Message::ErrorParseNuclearUnsignedElectron { name, column }
        }
        NuclearProblem::NeutrinoFlavour => {
            Message::ErrorParseNuclearNeutrinoFlavour { name, column }
        }
        NuclearProblem::Charge => Message::ErrorParseNuclearCharge { name, column },
        NuclearProblem::MassBelowCharge => {
            Message::ErrorParseNuclearMassBelowCharge { name, column }
        }
        NuclearProblem::NumbersDisagree => {
            Message::ErrorParseNuclearNumbersDisagree { name, column }
        }
        NuclearProblem::AtomicNumberDisagrees { written, symbol } => {
            Message::ErrorParseNuclearAtomicNumberDisagrees {
                name,
                written: written.to_string(),
                element: u8::try_from(symbol)
                    .ok()
                    .and_then(calc_syntax::element_symbol)
                    .unwrap_or_default()
                    .to_owned(),
                expected: symbol.to_string(),
                column,
            }
        }
        NuclearProblem::Isomer => Message::ErrorParseNuclearIsomer { name, column },
        NuclearProblem::OtherLepton => Message::ErrorParseNuclearOtherLepton { name, column },
        NuclearProblem::CompactForm => Message::ErrorParseNuclearCompactForm { name, column },
        NuclearProblem::ElementName => Message::ErrorParseNuclearElementName { name, column },
        NuclearProblem::Resonance => Message::ErrorParseNuclearResonance { name, column },
        NuclearProblem::Equilibrium => Message::ErrorParseNuclearEquilibrium { name, column },
        NuclearProblem::WrongArrow => Message::ErrorParseNuclearWrongArrow { name, column },
        NuclearProblem::NoArrow => Message::ErrorParseNuclearNoArrow { column },
        NuclearProblem::TwoArrows => Message::ErrorParseNuclearTwoArrows { name, column },
        NuclearProblem::EmptySide => Message::ErrorParseNuclearEmptySide { column },
        NuclearProblem::ZeroCoefficient => {
            Message::ErrorParseNuclearZeroCoefficient { name, column }
        }
        NuclearProblem::FractionalCoefficient => {
            Message::ErrorParseNuclearFractionalCoefficient { name, column }
        }
        NuclearProblem::CoefficientWithoutReaction => {
            Message::ErrorParseNuclearCoefficientWithoutReaction { name, column }
        }
        NuclearProblem::NumberTooLarge => Message::ErrorParseNuclearNumberTooLarge { name, column },
        NuclearProblem::CoefficientOutsideLiteral => {
            Message::ErrorParseNuclearCoefficientOutsideLiteral { name, column }
        }
    }
}

pub fn parse_error_message(error: &ParseError, input: &str) -> Message {
    let column = input
        .get(..error.span.start)
        .map_or(FIRST_COLUMN, |before| before.chars().count() + FIRST_COLUMN)
        .to_string();
    let name = input.get(error.span.clone()).unwrap_or_default().to_owned();
    match error.kind {
        ParseErrorKind::UnexpectedCharacter => {
            Message::ErrorParseUnexpectedCharacter { name, column }
        }
        ParseErrorKind::UnexpectedToken => Message::ErrorParseUnexpectedToken { name, column },
        ParseErrorKind::UnexpectedEnd => Message::ErrorParseUnexpectedEnd { column },
        ParseErrorKind::InvalidTypedLiteral => Message::ErrorParseInvalidTypedLiteral { column },
        ParseErrorKind::ExponentTooLarge => Message::ErrorParseExponentTooLarge {
            name,
            limit: LARGEST_DECIMAL_EXPONENT.to_string(),
            column,
        },
        ParseErrorKind::NotAUnit => Message::ErrorParseNotAUnit { name, column },
        ParseErrorKind::AtomicMassUnit => Message::ErrorParseAtomicMassUnit { name, column },
        ParseErrorKind::NotAUnitJoinedToAUnit => {
            Message::ErrorParseNotAUnitJoinedToAUnit { name, column }
        }
        ParseErrorKind::TypeExpected => Message::ErrorParseTypeExpected { name, column },
        ParseErrorKind::ByteOrderMissing => Message::ErrorParseByteOrderMissing { column },
        ParseErrorKind::AmbiguousUnit => {
            let readings = calc_units::ambiguous_readings(&name).unwrap_or_default();
            Message::ErrorParseAmbiguousUnit {
                name,
                column,
                readings: readings.join(", "),
            }
        }
        ParseErrorKind::UnitExponentOutOfRange
        | ParseErrorKind::UnitProduct(
            UnitProductError::ExponentOutOfRange | UnitProductError::PiExponentOutOfRange,
        ) => Message::ErrorParseUnitExponentOutOfRange {
            name,
            lowest: i8::MIN.to_string(),
            highest: i8::MAX.to_string(),
            column,
        },
        ParseErrorKind::UnitExponentNotWhole => {
            Message::ErrorParseUnitExponentNotWhole { name, column }
        }
        ParseErrorKind::ReservedName => Message::ErrorParseReservedName { name, column },
        ParseErrorKind::ChainedRelation => Message::ErrorParseChainedRelation { column },
        ParseErrorKind::RaggedArray => Message::ErrorParseRaggedArray { column },
        ParseErrorKind::UnknownKeyword => Message::ErrorParseUnknownKeyword { name, column },
        ParseErrorKind::DuplicateKeyword => Message::ErrorParseDuplicateKeyword { name, column },
        ParseErrorKind::PositionalAfterKeyword => {
            Message::ErrorParsePositionalAfterKeyword { name, column }
        }
        ParseErrorKind::MissingKeyword { keyword } => match keyword {
            Keyword::Form => Message::ErrorParseBubbleSortForm { column },
            Keyword::ShakerForm => Message::ErrorParseCocktailShakerSortForm { column },
            Keyword::OddEvenForm => Message::ErrorParseOddEvenSortForm { column },
            Keyword::CombForm => Message::ErrorParseCombSortForm { column },
            Keyword::Gaps => Message::ErrorParseShellSortGaps { column },
            Keyword::Partition => Message::ErrorParseQuickSortPartition { column },
            Keyword::Shape | Keyword::Side | Keyword::Order | Keyword::Pivot => {
                Message::ErrorParseMissingKeyword {
                    keyword: keyword.name().to_owned(),
                    values: keyword.values().join(", "),
                    column,
                }
            }
        },
        ParseErrorKind::MissingSeed => Message::ErrorParseMissingSeed { column },
        ParseErrorKind::MissingBase => Message::ErrorParseMissingBase { column },
        ParseErrorKind::MissingShuffleSeed => Message::ErrorParseMissingShuffleSeed { column },
        ParseErrorKind::MissingLimit => Message::ErrorParseMissingLimit { column },
        ParseErrorKind::InvalidLimit => Message::ErrorParseInvalidLimit { column },
        ParseErrorKind::InvalidBase => Message::ErrorParseInvalidBase { column },
        ParseErrorKind::InvalidSeed => Message::ErrorParseInvalidSeed { column },
        ParseErrorKind::SeedWithoutRandomPivot => {
            Message::ErrorParseSeedWithoutRandomPivot { column }
        }
        ParseErrorKind::MissingPivot { built } => Message::ErrorParseMissingPivot {
            built: built.to_owned(),
            column,
        },
        ParseErrorKind::PivotNotBuiltForPartition => {
            Message::ErrorParsePivotNotBuiltForPartition { name, column }
        }
        ParseErrorKind::InvalidKeywordValue { keyword } => Message::ErrorParseInvalidKeywordValue {
            name,
            keyword: keyword.name().to_owned(),
            values: keyword.values().join(", "),
            column,
        },
        ParseErrorKind::MissingDifferential => Message::ErrorParseMissingDifferential { column },
        ParseErrorKind::NestedTooDeeply { limit } => Message::ErrorParseNestedTooDeeply {
            limit: limit.to_string(),
            column,
        },
        ParseErrorKind::ChainTooLong { limit } => Message::ErrorParseChainTooLong {
            limit: limit.to_string(),
            column,
        },
        ParseErrorKind::ExpressionTooDeep { limit } => Message::ErrorParseExpressionTooDeep {
            limit: limit.to_string(),
            column,
        },
        ParseErrorKind::Chemistry(problem) => chemistry_parse_message(problem, name, column),
        ParseErrorKind::Nuclear(problem) => nuclear_parse_message(problem, name, column),
        ParseErrorKind::PercentInASum(percent) => Message::ErrorParsePercentInASum {
            written: percent.written(input),
            literal: percent.literal(input),
            relative: percent.relative(input),
            column,
        },
        ParseErrorKind::FractionBeforeUnit(fraction) => Message::ErrorParseFractionBeforeUnit {
            written: fraction.written(input),
            fraction: fraction.as_a_fraction(input),
            reciprocal: fraction.as_a_reciprocal(input),
            column,
        },
        ParseErrorKind::AmbiguousTemperatureSign(sign) => {
            Message::ErrorParseAmbiguousTemperatureSign {
                written: sign.written(input),
                reading: sign.reading(input),
                difference: sign.difference(input),
                column,
            }
        }
        ParseErrorKind::AmbiguousApplication(application) => {
            Message::ErrorParseAmbiguousApplication {
                written: application.written(input),
                narrow: application.narrow(input),
                wide: application.wide(input),
                column,
            }
        }
        ParseErrorKind::RecognisedAttempt(attempt) => match (attempt, corrected_text(input)) {
            (RecognisedAttempt::GroupingComma, _) => {
                Message::ErrorParseAttemptGroupingComma { column }
            }
            (RecognisedAttempt::ColonOrTime, _) => Message::ErrorParseAttemptColonOrTime { column },
            (RecognisedAttempt::DoubleStarPower, Some(corrected)) => {
                Message::ErrorParseAttemptDoubleStarPower { corrected, column }
            }
            (RecognisedAttempt::DoubleStarPower, None) => {
                Message::ErrorParseAttemptDoubleStarPowerPlain { column }
            }
            (RecognisedAttempt::CommaBetweenDigits, Some(corrected)) => {
                Message::ErrorParseAttemptCommaBetweenDigits { corrected, column }
            }
            (RecognisedAttempt::CommaBetweenDigits, None) => {
                Message::ErrorParseAttemptCommaBetweenDigitsPlain { column }
            }
            (RecognisedAttempt::AbsoluteValueBars, Some(corrected)) => {
                Message::ErrorParseAttemptAbsoluteValueBars { corrected, column }
            }
            (RecognisedAttempt::AbsoluteValueBars, None) => {
                Message::ErrorParseAttemptAbsoluteValueBarsPlain { column }
            }
            (RecognisedAttempt::TimesSign, Some(corrected)) => {
                Message::ErrorParseAttemptTimesSign { corrected, column }
            }
            (RecognisedAttempt::TimesSign, None) => {
                Message::ErrorParseAttemptTimesSignPlain { column }
            }
            (
                RecognisedAttempt::DivisionSign | RecognisedAttempt::ColonDivision,
                Some(corrected),
            ) => Message::ErrorParseAttemptDivisionSign { corrected, column },
            (RecognisedAttempt::DivisionSign | RecognisedAttempt::ColonDivision, None) => {
                Message::ErrorParseAttemptDivisionSignPlain { column }
            }
            (RecognisedAttempt::DegreeAfterName, Some(corrected)) => {
                Message::ErrorParseAttemptDegreeAfterName { corrected, column }
            }
            (RecognisedAttempt::DegreeAfterName, None) => {
                Message::ErrorParseAttemptDegreeAfterNamePlain { column }
            }
        },
        ParseErrorKind::ArityMismatch { expected, found }
        | ParseErrorKind::Build(BuildError::ArityMismatch { expected, found }) => {
            Message::ErrorParseArityMismatch {
                expected: u64::try_from(expected).unwrap_or(u64::MAX),
                found: found.to_string(),
                column,
            }
        }
        ParseErrorKind::Symbol(SymbolError::KindConflict { .. }) => Message::ErrorNameKindConflict,
        ParseErrorKind::Build(BuildError::TableFull(_) | BuildError::ShapeTooLarge)
        | ParseErrorKind::Symbol(SymbolError::TableFull)
        | ParseErrorKind::UnitProduct(UnitProductError::TableFull) => {
            Message::ErrorExpressionTooLarge
        }
        ParseErrorKind::Build(_) => Message::ErrorInternal {
            code: PARSE_BUILD_ERROR_CODE.to_owned(),
        },
        ParseErrorKind::Symbol(SymbolError::UnknownSymbolId(_)) => Message::ErrorInternal {
            code: PARSE_SYMBOL_ERROR_CODE.to_owned(),
        },
        ParseErrorKind::UnitProduct(UnitProductError::UnknownUnitId(_)) => Message::ErrorInternal {
            code: PARSE_UNIT_ERROR_CODE.to_owned(),
        },
    }
}

pub fn load_error_message(error: &LoadError, path: &str) -> Message {
    let path = path.to_owned();
    let invalid_at = |json_path: &JsonPath| Message::ErrorSessionInvalid {
        path: path.clone(),
        member: json_path_text(json_path),
    };
    match error {
        LoadError::Syntax(_) => Message::ErrorSessionNotJson { path: path.clone() },
        LoadError::UnsupportedVersion { found, supported } => Message::ErrorSessionNewerVersion {
            path: path.clone(),
            found: found.to_string(),
            supported: supported.to_string(),
        },
        LoadError::DependencyCycle(line) => Message::ErrorSessionDependencyCycle {
            path: path.clone(),
            line: line_label(*line),
        },
        LoadError::DuplicateMember(json_path)
        | LoadError::MissingMember(json_path)
        | LoadError::UnknownMember(json_path)
        | LoadError::InvalidValue(json_path)
        | LoadError::DecimalDoesNotMatchBits(json_path)
        | LoadError::DuplicateLineId(json_path)
        | LoadError::LineNumberNotBelowNext(json_path)
        | LoadError::DuplicateName(json_path)
        | LoadError::NameDoesNotMatchInput(json_path)
        | LoadError::MissingDependency(json_path) => invalid_at(json_path),
    }
}

pub fn save_error_message(error: &SaveError) -> Message {
    let code = match error {
        SaveError::CountTooLarge(_) => COUNT_ERROR_CODE,
        SaveError::UnitNotPrintable(_) => UNIT_PRINT_ERROR_CODE,
    };
    Message::ErrorInternal {
        code: code.to_owned(),
    }
}

pub fn method_message(name: &str) -> Message {
    match name {
        "exact_evaluation" => Message::CommonMethodExactEvaluation,
        "exact_after_conversion" => Message::CommonMethodExactAfterConversion,
        "plan_evaluation" => Message::CommonMethodPlanEvaluation,
        "way_evaluation" => Message::CommonMethodWayEvaluation,
        "worst_case" => Message::CommonMethodWorstCase,
        "chemistry_composition" => Message::CommonMethodChemistryComposition,
        "chemistry_balance" => Message::CommonMethodChemistryBalance,
        "chemistry_check" => Message::CommonMethodChemistryCheck,
        "nuclear_nuclide" => Message::CommonMethodNuclearNuclide,
        "nuclear_check" => Message::CommonMethodNuclearCheck,
        "insertion_sort" => Message::CommonMethodInsertionSort,
        "binary_insertion_sort" => Message::CommonMethodBinaryInsertionSort,
        "selection_sort" => Message::CommonMethodSelectionSort,
        "bubble_sort" => Message::CommonMethodBubbleSort,
        "merge_sort" => Message::CommonMethodMergeSort,
        "heap_sort" => Message::CommonMethodHeapSort,
        "quick_sort" => Message::CommonMethodQuickSort,
        "counting_sort" => Message::CommonMethodCountingSort,
        "double_selection_sort" => Message::CommonMethodDoubleSelectionSort,
        "cocktail_shaker_sort" => Message::CommonMethodCocktailShakerSort,
        "gnome_sort" => Message::CommonMethodGnomeSort,
        "odd_even_sort" => Message::CommonMethodOddEvenSort,
        "comb_sort" => Message::CommonMethodCombSort,
        "cycle_sort" => Message::CommonMethodCycleSort,
        "pancake_sort" => Message::CommonMethodPancakeSort,
        "shell_sort" => Message::CommonMethodShellSort,
        "bottom_up_merge_sort" => Message::CommonMethodBottomUpMergeSort,
        "natural_merge_sort" => Message::CommonMethodNaturalMergeSort,
        "radix_sort" => Message::CommonMethodRadixSort,
        "bead_sort" => Message::CommonMethodBeadSort,
        "bitonic_sort" => Message::CommonMethodBitonicSort,
        "bogo_sort" => Message::CommonMethodBogoSort,
        "philox4x32_10" => Message::CommonMethodPhilox4x3210,
        other => Message::CommonMethodNamed {
            name: other.to_owned(),
        },
    }
}

pub fn precision_message(precision: Precision) -> Message {
    match precision {
        Precision::Exact => Message::CommonPrecisionExact,
        Precision::F32 => Message::CommonPrecisionF32,
        Precision::F64 => Message::CommonPrecisionF64,
    }
}

pub fn preference_message(preference: Preference) -> Message {
    match preference {
        Preference::Automatic => Message::CommonBackendAutomatic,
        Preference::Only(kind) => backend_message(Some(kind)),
    }
}

pub fn gpu_refusal_message(refusal: &GpuRefusal) -> Message {
    match refusal {
        GpuRefusal::NoAdapter => Message::CommonGpuNoAdapter,
        GpuRefusal::ExactCaseFailed(None) => Message::CommonGpuExactCaseFailed,
        GpuRefusal::ExactCaseFailed(Some(operation)) => {
            Message::CommonGpuExactCaseFailedOperation {
                operation: operation_name(*operation).to_owned(),
            }
        }
        GpuRefusal::CaseNotRun(None) => Message::CommonGpuCaseNotRun,
        GpuRefusal::CaseNotRun(Some(operation)) => Message::CommonGpuCaseNotRunOperation {
            operation: operation_name(*operation).to_owned(),
        },
    }
}

pub fn backend_message(kind: Option<BackendKind>) -> Message {
    match kind {
        Some(BackendKind::Cpu) => Message::CommonBackendCpu,
        Some(BackendKind::Simd) => Message::CommonBackendSimd,
        Some(BackendKind::Gpu) => Message::CommonBackendGpu,
        None => Message::CommonBackendNone,
    }
}

pub(crate) fn json_path_text(path: &JsonPath) -> String {
    let mut text = String::new();
    for segment in &path.0 {
        match segment {
            PathSegment::Member(name) => {
                if !text.is_empty() {
                    text.push_str(PATH_MEMBER_SEPARATOR);
                }
                text.push_str(name);
            }
            PathSegment::Index(position) => text.push_str(&format!("[{position}]")),
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_core::{METHOD_NAMES, ResultValue};
    use calc_exec::PlanOp;
    use calc_numbers::{Integer, Number};
    use std::collections::BTreeMap;

    fn diagnostic(code: &str, data: &[(&str, &str)]) -> Diagnostic {
        Diagnostic {
            code: code.to_owned(),
            data: data
                .iter()
                .map(|(name, value)| {
                    (
                        (*name).to_owned(),
                        ParameterValue::Identifier((*value).to_owned()),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
        }
    }

    #[test]
    fn parse_error_json_carries_kind_and_column() {
        let error = ParseError {
            kind: ParseErrorKind::UnexpectedEnd,
            span: 3..3,
        };
        assert_eq!(
            parse_error_json(&error, "2 +"),
            crate::solve::typed_error_json(
                "parse_error",
                &[("kind", "unexpected_end"), ("column", "4")]
            )
        );
    }

    #[test]
    fn a_batch_line_that_does_not_parse_names_its_input() {
        let error = ParseError {
            kind: ParseErrorKind::UnexpectedEnd,
            span: 3..3,
        };
        let json = String::from_utf8(parse_error_json_line(&error, "2 +")).unwrap();

        assert_eq!(
            json,
            "{\"code\": \"parse_error\", \"input\": \"2 +\", \"data\": {\"kind\": \"unexpected_end\", \"column\": \"4\"}}"
        );
    }

    #[test]
    fn ambiguous_application_json_carries_both_readings() {
        let input = "sin pi/2";
        let error =
            calc_syntax::parse_expression(&mut calc_expr::ExprPool::new(), input).unwrap_err();

        assert_eq!(
            String::from_utf8(parse_error_json(&error, input)).unwrap(),
            String::from_utf8(crate::solve::typed_error_json(
                "ambiguous_application",
                &[
                    ("column", "7"),
                    ("function", "sin"),
                    ("narrow", "sin(pi)/2"),
                    ("wide", "sin(pi/2)"),
                    ("written", "sin pi/2")
                ]
            ))
            .unwrap()
        );
    }

    #[test]
    fn ambiguous_temperature_sign_json_carries_the_reading_and_the_difference() {
        let input = "20 \u{00B0}C";
        let error =
            calc_syntax::parse_expression(&mut calc_expr::ExprPool::new(), input).unwrap_err();

        let json = String::from_utf8(parse_error_json(&error, input)).unwrap();

        assert_eq!(
            json,
            String::from_utf8(crate::solve::typed_error_json(
                "ambiguous_temperature_sign",
                &[
                    ("column", "4"),
                    ("difference", "20 degC"),
                    ("reading", "from_celsius(20)"),
                    ("sign", "celsius"),
                    ("written", "20 \u{00B0}C")
                ]
            ))
            .unwrap()
        );
    }

    #[test]
    fn parse_error_json_of_a_recognised_attempt_is_the_recognised_attempt() {
        let error = ParseError {
            kind: ParseErrorKind::RecognisedAttempt(RecognisedAttempt::DoubleStarPower),
            span: 2..4,
        };
        assert_eq!(
            parse_error_json(&error, "10**6"),
            recognised_attempt_json(&error, "10**6").unwrap()
        );
    }

    #[test]
    fn recognised_attempt_json_carries_attempt_column_replacement_and_correction() {
        let error = ParseError {
            kind: ParseErrorKind::RecognisedAttempt(RecognisedAttempt::DoubleStarPower),
            span: 2..4,
        };
        let json = String::from_utf8(recognised_attempt_json(&error, "10**6").unwrap()).unwrap();
        assert_eq!(
            json,
            String::from_utf8(crate::solve::typed_error_json(
                "recognised_attempt",
                &[
                    ("attempt", "double_star_power"),
                    ("column", "3"),
                    ("corrected", "10^6"),
                    ("replacement", "^")
                ]
            ))
            .unwrap()
        );
    }

    #[test]
    fn recognised_attempt_maps_to_its_own_message_with_the_column() {
        let error = ParseError {
            kind: ParseErrorKind::RecognisedAttempt(RecognisedAttempt::DoubleStarPower),
            span: 1..3,
        };
        assert_eq!(
            parse_error_message(&error, "2**3"),
            Message::ErrorParseAttemptDoubleStarPower {
                corrected: "2^3".to_owned(),
                column: "2".to_owned()
            }
        );
    }

    #[test]
    fn unit_ended_at_space_diagnostic_maps_to_its_own_message() {
        let message = diagnostic_message(&diagnostic(
            "recognised_attempt",
            &[
                ("attempt", "unit_ended_at_space"),
                ("name", "s"),
                ("replacement", "m/s^2"),
                ("corrected", "9.81 m/s^2"),
                ("column", "7"),
            ],
        ));
        assert_eq!(
            message,
            Message::ErrorUnitEndedAtSpace {
                name: "s".to_owned(),
                replacement: "m/s^2".to_owned(),
                corrected: "9.81 m/s^2".to_owned(),
                column: "7".to_owned(),
            }
        );
    }

    #[test]
    fn unit_ended_at_space_without_a_corrected_text_maps_to_the_plain_message() {
        let message = diagnostic_message(&diagnostic(
            "recognised_attempt",
            &[
                ("attempt", "unit_ended_at_space"),
                ("name", "s"),
                ("replacement", "m/s^2"),
                ("column", "7"),
            ],
        ));
        assert_eq!(
            message,
            Message::ErrorUnitEndedAtSpacePlain {
                name: "s".to_owned(),
                replacement: "m/s^2".to_owned(),
                column: "7".to_owned(),
            }
        );
    }

    #[test]
    fn celsius_sign_parse_error_names_the_reading_and_the_difference() {
        let input = "20 \u{00B0}C";
        let error =
            calc_syntax::parse_expression(&mut calc_expr::ExprPool::new(), input).unwrap_err();
        assert_eq!(
            parse_error_message(&error, input),
            Message::ErrorParseAmbiguousTemperatureSign {
                written: "20 \u{00B0}C".to_owned(),
                reading: "from_celsius(20)".to_owned(),
                difference: "20 degC".to_owned(),
                column: "4".to_owned(),
            }
        );
    }

    #[test]
    fn degree_sign_after_a_name_names_its_correction() {
        let input = "x\u{00B0}";
        let error =
            calc_syntax::parse_expression(&mut calc_expr::ExprPool::new(), input).unwrap_err();
        assert_eq!(
            parse_error_message(&error, input),
            Message::ErrorParseAttemptDegreeAfterName {
                corrected: "x * 1\u{00B0}".to_owned(),
                column: "2".to_owned(),
            }
        );
    }

    #[test]
    fn ambiguous_application_parse_error_names_both_readings() {
        let input = "sin \u{03C0}/2";
        let error =
            calc_syntax::parse_expression(&mut calc_expr::ExprPool::new(), input).unwrap_err();
        assert_eq!(
            parse_error_message(&error, input),
            Message::ErrorParseAmbiguousApplication {
                written: "sin \u{03C0}/2".to_owned(),
                narrow: "sin(\u{03C0})/2".to_owned(),
                wide: "sin(\u{03C0}/2)".to_owned(),
                column: "6".to_owned(),
            }
        );
    }

    #[test]
    fn ambiguous_temperature_sign_diagnostic_maps_to_the_same_message() {
        let message = diagnostic_message(&diagnostic(
            "ambiguous_temperature_sign",
            &[
                ("sign", "celsius"),
                ("written", "20 \u{00B0}C"),
                ("column", "4"),
                ("difference", "20 degC"),
                ("reading", "from_celsius(20)"),
            ],
        ));
        assert_eq!(
            message,
            Message::ErrorParseAmbiguousTemperatureSign {
                written: "20 \u{00B0}C".to_owned(),
                reading: "from_celsius(20)".to_owned(),
                difference: "20 degC".to_owned(),
                column: "4".to_owned(),
            }
        );
    }

    #[test]
    fn ambiguous_application_diagnostic_maps_to_the_same_message() {
        let message = diagnostic_message(&diagnostic(
            "ambiguous_application",
            &[
                ("function", "sin"),
                ("written", "sin pi/2"),
                ("column", "7"),
                ("narrow", "sin(pi)/2"),
                ("wide", "sin(pi/2)"),
            ],
        ));
        assert_eq!(
            message,
            Message::ErrorParseAmbiguousApplication {
                written: "sin pi/2".to_owned(),
                narrow: "sin(pi)/2".to_owned(),
                wide: "sin(pi/2)".to_owned(),
                column: "7".to_owned(),
            }
        );
    }

    #[test]
    fn input_not_parsed_diagnostic_names_the_column() {
        let message = diagnostic_message(&diagnostic("parse_error", &[("column", "3")]));
        assert_eq!(
            message,
            Message::ErrorInputNotParsed {
                column: "3".to_owned()
            }
        );
    }

    #[test]
    fn undefined_name_diagnostic_names_the_name() {
        let message = diagnostic_message(&diagnostic("undefined_name", &[("name", "x")]));
        assert_eq!(
            message,
            Message::ErrorUndefinedName {
                name: "x".to_owned()
            }
        );
    }

    fn read(code: &str, reading: &str, counts: &[(&str, u64)]) -> Diagnostic {
        let mut data: BTreeMap<String, ParameterValue> = counts
            .iter()
            .map(|(name, count)| {
                (
                    (*name).to_owned(),
                    ParameterValue::Value(ResultValue::Number(Number::Integer(Integer::from(
                        *count,
                    )))),
                )
            })
            .collect();
        data.insert(
            "reading".to_owned(),
            ParameterValue::Value(ResultValue::Expression(reading.to_owned())),
        );
        Diagnostic {
            code: code.to_owned(),
            data,
        }
    }

    #[test]
    fn division_by_zero_names_the_division() {
        assert_eq!(
            diagnostic_message(&read("division_by_zero", "1/(2 - 2)", &[])),
            Message::ErrorDivisionByZero {
                reading: "1/(2 - 2)".to_owned()
            }
        );
    }

    #[test]
    fn outside_domain_of_another_operator_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("outside_domain", "acosh(0)", &[])),
            Message::ErrorOutsideDomain {
                reading: "acosh(0)".to_owned()
            }
        );
    }

    #[test]
    fn unsupported_operator_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("unsupported_operator", "floor(x)", &[])),
            Message::ErrorUnsupportedOperator {
                reading: "floor(x)".to_owned()
            }
        );
    }

    #[test]
    fn unsupported_subexpression_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("unsupported_subexpression", "f(2)", &[])),
            Message::ErrorUnsupportedSubexpression {
                reading: "f(2)".to_owned()
            }
        );
    }

    #[test]
    fn unsupported_constant_names_the_constant() {
        assert_eq!(
            diagnostic_message(&read("unsupported_constant", "gamma", &[])),
            Message::ErrorUnsupportedConstant {
                reading: "gamma".to_owned()
            }
        );
    }

    #[test]
    fn not_finite_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("not_finite", "f64'inf'", &[])),
            Message::ErrorNotFinite {
                reading: "f64'inf'".to_owned()
            }
        );
    }

    #[test]
    fn machine_number_in_exact_names_the_number() {
        assert_eq!(
            diagnostic_message(&read("machine_number_in_exact_evaluation", "f64'0.5'", &[])),
            Message::ErrorMachineNumberInExact {
                reading: "f64'0.5'".to_owned()
            }
        );
    }

    #[test]
    fn index_range_too_long_names_the_sum() {
        assert_eq!(
            diagnostic_message(&read("index_range_too_long", "sum(k, 1, 10^30, k)", &[])),
            Message::ErrorIndexRangeTooLong {
                reading: "sum(k, 1, 10^30, k)".to_owned()
            }
        );
    }

    #[test]
    fn dimension_mismatch_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("dimension_mismatch", "1 m + 1 s", &[])),
            Message::ErrorDimensionMismatch {
                reading: "1 m + 1 s".to_owned()
            }
        );
    }

    #[test]
    fn dimensioned_argument_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("dimensioned_argument", "sin(1 m)", &[])),
            Message::ErrorDimensionedArgument {
                reading: "sin(1 m)".to_owned()
            }
        );
    }

    #[test]
    fn dimensioned_power_exponent_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read(
                "dimensioned_power_exponent_not_constant",
                "(1 m)^x",
                &[]
            )),
            Message::ErrorDimensionedPowerExponentNotConstant {
                reading: "(1 m)^x".to_owned()
            }
        );
    }

    #[test]
    fn fractional_dimension_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("fractional_dimension", "sqrt(1 m)", &[])),
            Message::ErrorFractionalDimension {
                reading: "sqrt(1 m)".to_owned()
            }
        );
    }

    #[test]
    fn dimension_out_of_range_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("dimension_out_of_range", "(1 m)^200", &[])),
            Message::ErrorDimensionOutOfRange {
                reading: "(1 m)^200".to_owned(),
                lowest: "-128".to_owned(),
                highest: "127".to_owned()
            }
        );
    }

    #[test]
    fn negative_uncertainty_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("negative_uncertainty", "1 +- -0.1", &[])),
            Message::ErrorNegativeUncertainty {
                reading: "1 +- -0.1".to_owned()
            }
        );
    }

    #[test]
    fn nested_uncertainty_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("nested_uncertainty", "1 +- (1 +- 1)", &[])),
            Message::ErrorNestedUncertainty {
                reading: "1 +- (1 +- 1)".to_owned()
            }
        );
    }

    #[test]
    fn coverage_factor_below_one_names_the_reading() {
        assert_eq!(
            diagnostic_message(&read("coverage_factor_below_one", "2 +- 0.1 (k=0.5)", &[])),
            Message::ErrorCoverageFactorBelowOne {
                reading: "2 +- 0.1 (k=0.5)".to_owned()
            }
        );
    }

    fn with(mut diagnostic: Diagnostic, name: &str, value: ParameterValue) -> Diagnostic {
        diagnostic.data.insert(name.to_owned(), value);
        diagnostic
    }

    fn text(value: &str) -> ParameterValue {
        ParameterValue::Value(ResultValue::Expression(value.to_owned()))
    }

    fn word(value: &str) -> ParameterValue {
        ParameterValue::Identifier(value.to_owned())
    }

    fn big(value: Integer) -> ParameterValue {
        ParameterValue::Value(ResultValue::Number(Number::Integer(value)))
    }

    fn domain(operator: &str, operand: &str) -> Diagnostic {
        with(
            with(read("outside_domain", "r", &[]), "operator", word(operator)),
            "operand",
            text(operand),
        )
    }

    #[test]
    fn square_root_of_a_negative_number_says_what_sqrt_needs() {
        assert_eq!(
            diagnostic_message(&domain("sqrt", "-1")),
            Message::ErrorDomainSqrt {
                operand: "-1".to_owned()
            }
        );
    }

    #[test]
    fn logarithm_outside_its_domain_says_what_ln_needs() {
        assert_eq!(
            diagnostic_message(&domain("ln", "0")),
            Message::ErrorDomainLn {
                operand: "0".to_owned()
            }
        );
    }

    #[test]
    fn arcsine_outside_its_domain_names_the_operator_and_range() {
        assert_eq!(
            diagnostic_message(&domain("acos", "2")),
            Message::ErrorDomainArcSine {
                operator: "acos".to_owned(),
                operand: "2".to_owned()
            }
        );
    }

    #[test]
    fn tangent_at_a_pole_names_the_point() {
        assert_eq!(
            diagnostic_message(&domain("tan", "pi/2")),
            Message::ErrorDomainTan {
                operand: "pi/2".to_owned()
            }
        );
    }

    #[test]
    fn factorial_outside_its_domain_says_what_it_needs() {
        assert_eq!(
            diagnostic_message(&domain("factorial", "1.5")),
            Message::ErrorDomainFactorial {
                operand: "1.5".to_owned()
            }
        );
    }

    #[test]
    fn power_of_a_negative_base_names_base_and_exponent() {
        let power = with(domain("pow", "-8"), "exponent", text("1/3"));
        assert_eq!(
            diagnostic_message(&power),
            Message::ErrorDomainPower {
                base: "-8".to_owned(),
                exponent: "1/3".to_owned()
            }
        );
    }

    #[test]
    fn stored_domain_error_without_data_names_the_operator() {
        let old = with(
            Diagnostic {
                code: "outside_domain".to_owned(),
                data: BTreeMap::new(),
            },
            "operator",
            word("sqrt"),
        );
        assert_eq!(
            diagnostic_message(&old),
            Message::ErrorOutsideDomainPlain {
                operator: "sqrt".to_owned()
            }
        );
    }

    fn too_large_argument(
        operator: &str,
        operand_name: &str,
        operand: &str,
        argument: Integer,
    ) -> Diagnostic {
        let base = read(
            "integer_argument_out_of_range",
            "r",
            &[("limit", 4_294_967_295)],
        );
        with(
            with(
                with(base, "operator", word(operator)),
                operand_name,
                text(operand),
            ),
            "argument",
            big(argument),
        )
    }

    #[test]
    fn too_large_exponent_names_the_exponent_and_its_value() {
        assert_eq!(
            diagnostic_message(&too_large_argument(
                "pow",
                "exponent",
                "100^10",
                Integer::from(10_i64).pow(20)
            )),
            Message::ErrorExponentOutOfRange {
                exponent: "100^10 = 1e20".to_owned(),
                limit: "4294967295".to_owned()
            }
        );
    }

    #[test]
    fn too_large_literal_exponent_is_named_once() {
        assert_eq!(
            diagnostic_message(&too_large_argument(
                "pow",
                "exponent",
                "40000000000",
                Integer::from(40_000_000_000_i64)
            )),
            Message::ErrorExponentOutOfRange {
                exponent: "4e10".to_owned(),
                limit: "4294967295".to_owned()
            }
        );
    }

    #[test]
    fn exponent_with_a_long_value_names_its_digit_count() {
        assert_eq!(
            diagnostic_message(&too_large_argument(
                "pow",
                "exponent",
                "4^(5^6)",
                &Integer::from(10_i64).pow(41) + &Integer::one()
            )),
            Message::ErrorExponentOutOfRangeLong {
                exponent: "4^(5^6)".to_owned(),
                digits: "42".to_owned(),
                limit: "4294967295".to_owned()
            }
        );
    }

    #[test]
    fn exponent_whose_zeros_fit_in_the_exponent_form_keeps_its_value() {
        assert_eq!(
            diagnostic_message(&too_large_argument(
                "pow",
                "exponent",
                "4^(5^6)",
                Integer::from(10_i64).pow(41)
            )),
            Message::ErrorExponentOutOfRange {
                exponent: "4^(5^6) = 1e41".to_owned(),
                limit: "4294967295".to_owned()
            }
        );
    }

    #[test]
    fn too_large_root_degree_names_the_exponent_and_the_degree() {
        let degree = with(
            too_large_argument("pow", "exponent", "1/10^20", Integer::from(10_i64).pow(20)),
            "role",
            word("root_degree"),
        );
        assert_eq!(
            diagnostic_message(&degree),
            Message::ErrorRootDegreeOutOfRange {
                exponent: "1/10^20".to_owned(),
                degree: "1e20".to_owned(),
                limit: "4294967295".to_owned()
            }
        );
    }

    #[test]
    fn root_degree_with_a_long_value_names_its_digit_count() {
        let degree = with(
            too_large_argument(
                "pow",
                "exponent",
                "1/10^41",
                &Integer::from(10_i64).pow(41) + &Integer::one(),
            ),
            "role",
            word("root_degree"),
        );
        assert_eq!(
            diagnostic_message(&degree),
            Message::ErrorRootDegreeOutOfRangeLong {
                exponent: "1/10^41".to_owned(),
                digits: "42".to_owned(),
                limit: "4294967295".to_owned()
            }
        );
    }

    #[test]
    fn too_large_factorial_names_the_number() {
        assert_eq!(
            diagnostic_message(&too_large_argument(
                "factorial",
                "operand",
                "10^10",
                Integer::from(10_000_000_000_i64)
            )),
            Message::ErrorFactorialOutOfRange {
                operand: "10^10 = 1e10".to_owned(),
                limit: "4294967295".to_owned()
            }
        );
    }

    #[test]
    fn factorial_with_a_long_number_names_its_digit_count() {
        assert_eq!(
            diagnostic_message(&too_large_argument(
                "factorial",
                "operand",
                "10^41",
                &Integer::from(10_i64).pow(41) + &Integer::one()
            )),
            Message::ErrorFactorialOutOfRangeLong {
                operand: "10^41".to_owned(),
                digits: "42".to_owned(),
                limit: "4294967295".to_owned()
            }
        );
    }

    #[test]
    fn stored_argument_error_without_data_names_the_operator() {
        let old = with(
            Diagnostic {
                code: "integer_argument_out_of_range".to_owned(),
                data: BTreeMap::new(),
            },
            "operator",
            word("pow"),
        );
        assert_eq!(
            diagnostic_message(&old),
            Message::ErrorIntegerArgumentOutOfRangePlain {
                operator: "pow".to_owned()
            }
        );
    }

    const SIZES: &[(&str, u64)] = &[
        ("estimated_digits", 100_001),
        ("limit_digits", 78_914),
        ("limit_bits", 262_144),
    ];

    #[test]
    fn result_too_large_names_digits_and_both_limits() {
        assert_eq!(
            diagnostic_message(&read("result_too_large", "10^100000", SIZES)),
            Message::ErrorResultTooLarge {
                reading: "10^100000".to_owned(),
                digits: "100001".to_owned(),
                limit_digits: "78914".to_owned(),
                limit: "262144".to_owned()
            }
        );
    }

    #[test]
    fn intermediate_step_too_large_names_the_step_digits_and_both_limits() {
        assert_eq!(
            diagnostic_message(&read("intermediate_step_too_large", "10^100000", SIZES)),
            Message::ErrorIntermediateStepTooLarge {
                reading: "10^100000".to_owned(),
                digits: "100001".to_owned(),
                limit_digits: "78914".to_owned(),
                limit: "262144".to_owned()
            }
        );
    }

    #[test]
    fn too_large_number_names_digits_and_both_limits() {
        assert_eq!(
            diagnostic_message(&read("number_too_large", "1.5e-99999", SIZES)),
            Message::ErrorNumberTooLarge {
                reading: "1.5e-99999".to_owned(),
                digits: "100001".to_owned(),
                limit_digits: "78914".to_owned(),
                limit: "262144".to_owned()
            }
        );
    }

    #[test]
    fn stored_size_error_without_data_says_the_result_is_too_large() {
        let old = Diagnostic {
            code: "result_too_large".to_owned(),
            data: BTreeMap::new(),
        };
        assert_eq!(diagnostic_message(&old), Message::ErrorResultTooLargePlain);
    }

    #[test]
    fn stored_division_by_zero_without_a_reading_keeps_its_message() {
        let old = Diagnostic {
            code: "division_by_zero".to_owned(),
            data: BTreeMap::new(),
        };
        assert_eq!(diagnostic_message(&old), Message::ErrorDivisionByZeroPlain);
    }

    #[test]
    fn long_reading_is_passed_on_in_full() {
        let long = format!("0.{}15", "0".repeat(100));
        assert_eq!(
            diagnostic_message(&read("not_finite", &long, &[])),
            Message::ErrorNotFinite { reading: long }
        );
    }

    #[test]
    fn plot_axis_errors_count_axes_from_one() {
        assert_eq!(
            plot_error_message(&PlotError::EmptyInterval { view: 0, axis: 1 }),
            Message::ErrorPlotEmptyInterval {
                axis: "2".to_owned()
            }
        );
    }

    #[test]
    fn axis_count_mismatch_keeps_its_fixed_code() {
        assert_eq!(
            plot_error_message(&PlotError::Sample(calc_viz::SampleError::AxisCount {
                expected: 1,
                found: 2
            })),
            Message::ErrorPlotSamplingFailed {
                code: "axis_count".to_owned()
            }
        );
    }

    #[test]
    fn sample_limit_names_the_requested_count_and_the_limit() {
        assert_eq!(
            sample_error_message(&calc_viz::SampleError::SampleLimitExceeded {
                requested: 20,
                limit: 10
            }),
            Message::ErrorPlotSampleLimit {
                requested: "20".to_owned(),
                limit: "10".to_owned()
            }
        );
    }

    #[test]
    fn mesh_too_large_names_the_vertex_count() {
        assert_eq!(
            sample_error_message(&calc_viz::SampleError::MeshTooLarge { vertices: 70_000 }),
            Message::ErrorPlotMeshTooLarge {
                vertices: "70000".to_owned()
            }
        );
    }

    #[test]
    fn iteration_limit_too_large_names_the_iterations() {
        assert_eq!(
            sample_error_message(&calc_viz::SampleError::IterationLimitTooLarge {
                iterations: 5000
            }),
            Message::ErrorPlotIterationLimitTooLarge {
                iterations: "5000".to_owned()
            }
        );
    }

    #[test]
    fn sampling_errors_without_their_own_message_keep_a_fixed_code() {
        use calc_viz::SampleError;
        let table = [
            (SampleError::NoDivisions { axis: 0 }, "no_divisions"),
            (SampleError::BoundNotExact { axis: 0 }, "bound_not_exact"),
            (SampleError::ComplexAxesDiffer, "complex_axes_differ"),
            (
                SampleError::AxisUnitNotSupported { axis: 0 },
                "axis_unit_not_supported",
            ),
            (SampleError::RunFinished, "run_finished"),
            (
                SampleError::EscapeTimeSettingsMissing,
                "escape_time_settings_missing",
            ),
            (SampleError::StageOrder, "stage_order"),
            (SampleError::SamplesPending, "samples_pending"),
            (
                SampleError::RefinementNeedsEscapeTime,
                "refinement_needs_escape_time",
            ),
        ];

        let codes: Vec<Message> = table
            .iter()
            .map(|(error, _)| sample_error_message(error))
            .collect();

        assert_eq!(
            codes,
            table
                .iter()
                .map(|(_, code)| Message::ErrorPlotSamplingFailed {
                    code: (*code).to_owned()
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn line_number_too_large_has_its_own_message() {
        assert_eq!(
            plot_error_message(&PlotError::LineNumberTooLarge(
                crate::LineId::from_number(1).unwrap()
            )),
            Message::ErrorPlotLineNumberTooLarge {
                line: "r1".to_owned()
            }
        );
    }

    #[test]
    fn unknown_diagnostic_code_is_an_internal_error_with_the_code() {
        let message = diagnostic_message(&diagnostic("batch_not_built", &[]));
        assert_eq!(
            message,
            Message::ErrorInternal {
                code: "batch_not_built".to_owned()
            }
        );
    }

    #[test]
    fn parse_error_column_counts_characters_from_one() {
        let error = ParseError {
            kind: ParseErrorKind::UnexpectedToken,
            span: 5..6,
        };
        let message = parse_error_message(&error, "π + )");
        assert_eq!(
            message,
            Message::ErrorParseUnexpectedToken {
                name: ")".to_owned(),
                column: "5".to_owned()
            }
        );
    }

    #[test]
    fn unexpected_character_names_the_character() {
        let error = ParseError {
            kind: ParseErrorKind::UnexpectedCharacter,
            span: 4..5,
        };
        assert_eq!(
            parse_error_message(&error, "2 + @"),
            Message::ErrorParseUnexpectedCharacter {
                name: "@".to_owned(),
                column: "5".to_owned()
            }
        );
    }

    #[test]
    fn not_a_unit_error_names_the_written_name() {
        let error = ParseError {
            kind: ParseErrorKind::NotAUnit,
            span: 2..5,
        };
        let message = parse_error_message(&error, "2 abc");
        assert_eq!(
            message,
            Message::ErrorParseNotAUnit {
                name: "abc".to_owned(),
                column: "3".to_owned()
            }
        );
    }

    #[test]
    fn name_exists_error_names_the_name() {
        let message = session_error_message(&SessionError::NameExists("F".to_owned()), "");
        assert_eq!(
            message,
            Message::ErrorNameExists {
                name: "F".to_owned()
            }
        );
    }

    #[test]
    fn invalid_member_load_error_names_the_json_path() {
        let path = JsonPath(vec![
            PathSegment::Member("lines".to_owned()),
            PathSegment::Index(0),
            PathSegment::Member("name".to_owned()),
        ]);
        let message = load_error_message(&LoadError::InvalidValue(path), "a.calc");
        assert_eq!(
            message,
            Message::ErrorSessionInvalid {
                path: "a.calc".to_owned(),
                member: "lines[0].name".to_owned()
            }
        );
    }

    #[test]
    fn every_listed_method_name_has_its_own_message() {
        let without_message: Vec<&str> = METHOD_NAMES
            .into_iter()
            .filter(|name| matches!(method_message(name), Message::CommonMethodNamed { .. }))
            .collect();

        assert_eq!(without_message, Vec::<&str>::new());
    }

    #[test]
    fn uncertainty_not_propagated_has_its_own_message() {
        let failure = Diagnostic {
            code: "uncertainty_not_propagated".to_owned(),
            data: BTreeMap::new(),
        };

        assert_eq!(
            diagnostic_message(&failure),
            Message::ErrorUncertaintyNotPropagated
        );
    }

    #[test]
    fn unknown_method_name_is_carried_as_data() {
        assert_eq!(
            method_message("gauss_kronrod_adaptive"),
            Message::CommonMethodNamed {
                name: "gauss_kronrod_adaptive".to_owned()
            }
        );
    }

    #[test]
    fn a_machine_without_an_adapter_is_not_a_fault() {
        assert_eq!(
            gpu_refusal_message(&GpuRefusal::NoAdapter),
            Message::CommonGpuNoAdapter
        );
    }

    #[test]
    fn a_failed_exact_case_names_the_operation_it_failed_on() {
        assert_eq!(
            gpu_refusal_message(&GpuRefusal::ExactCaseFailed(Some(PlanOp::Add))),
            Message::CommonGpuExactCaseFailedOperation {
                operation: "add".to_owned()
            }
        );
    }

    #[test]
    fn a_failed_exact_case_of_no_operation_names_none() {
        assert_eq!(
            gpu_refusal_message(&GpuRefusal::ExactCaseFailed(None)),
            Message::CommonGpuExactCaseFailed
        );
    }

    #[test]
    fn a_case_that_could_not_run_names_the_operation_it_covers() {
        assert_eq!(
            gpu_refusal_message(&GpuRefusal::CaseNotRun(Some(PlanOp::RoundTiesEven))),
            Message::CommonGpuCaseNotRunOperation {
                operation: "round_ties_even".to_owned()
            }
        );
    }

    #[test]
    fn a_case_that_could_not_run_without_an_operation_names_none() {
        assert_eq!(
            gpu_refusal_message(&GpuRefusal::CaseNotRun(None)),
            Message::CommonGpuCaseNotRun
        );
    }

    #[test]
    fn automatic_preference_maps_to_the_automatic_word() {
        assert_eq!(
            preference_message(Preference::Automatic),
            Message::CommonBackendAutomatic
        );
    }
}
