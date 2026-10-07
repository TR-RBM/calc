use std::collections::BTreeMap;

use calc_asm::{Case, Count, LoopProblem, Place, Refusal};
use calc_i18n::{Locale, Message, render};

pub use calc_asm::{FunctionReport, UNRESOLVED_CALL, is_general_register};

const HEXADECIMAL_PREFIX: &str = "0x";
const PLURAL_OTHER: u64 = 2;
const ARGUMENT_REGISTERS: [[&str; 2]; 6] = [
    ["rdi", "edi"],
    ["rsi", "esi"],
    ["rdx", "edx"],
    ["rcx", "ecx"],
    ["r8", "r8d"],
    ["r9", "r9d"],
];

pub fn assembly_reports(source: &str, given: &BTreeMap<String, i128>) -> Vec<FunctionReport> {
    calc_asm::analyze(&calc_asm::parse(source), given)
}

pub fn is_argument_register(name: &str) -> bool {
    ARGUMENT_REGISTERS
        .iter()
        .flatten()
        .any(|register| *register == name)
}

fn place_text(place: Place, locale: &Locale) -> String {
    match place {
        Place::Line(line) => render(
            &Message::CliAsmPlaceLine {
                line: line.to_string(),
            },
            locale,
        )
        .to_string(),
        Place::Address(address) => format!("{HEXADECIMAL_PREFIX}{address:x}"),
    }
}

fn loop_message(problem: &LoopProblem, place: String) -> Message {
    match problem {
        LoopProblem::SecondExit => Message::CliAsmLoopSecondExit { place },
        LoopProblem::UnconditionalLatch => Message::CliAsmLoopUnconditional { place },
        LoopProblem::NoCounter => Message::CliAsmLoopNoCounter { place },
        LoopProblem::Step => Message::CliAsmLoopStep { place },
        LoopProblem::Unsigned => Message::CliAsmLoopUnsigned { place },
        LoopProblem::DependsOnData => Message::CliAsmLoopData { place },
        LoopProblem::DependsOnOuterCounter => Message::CliAsmLoopOuterCounter { place },
        LoopProblem::NeverEnds => Message::CliAsmLoopNeverEnds { place },
        LoopProblem::Wraps => Message::CliAsmLoopWraps { place },
        LoopProblem::ChangesStack => Message::CliAsmLoopStack { place },
        LoopProblem::Overlapping => Message::CliAsmLoopOverlapping { place },
    }
}

pub fn assembly_refusal_text(refusal: &Refusal, locale: &Locale) -> String {
    let place = |place: Place| place_text(place, locale);
    let message = match refusal {
        Refusal::IndirectJump(at) => Message::CliAsmIndirectJump { place: place(*at) },
        Refusal::JumpOutside(at, name) => Message::CliAsmJumpOutside {
            place: place(*at),
            name: name.clone(),
        },
        Refusal::Recursion(at, name) => Message::CliAsmRecursion {
            place: place(*at),
            name: name.clone(),
        },
        Refusal::RepeatPrefix(at) => Message::CliAsmRepeatPrefix { place: place(*at) },
        Refusal::UnsupportedControl(at, mnemonic) => Message::CliAsmUnsupportedJump {
            place: place(*at),
            mnemonic: mnemonic.clone(),
        },
        Refusal::RunsPastEnd => Message::CliAsmRunsPastEnd,
        Refusal::TooManyPaths => Message::CliAsmTooManyPaths {
            limit: calc_asm::PATH_LIMIT.to_string(),
        },
        Refusal::LeavesWithoutReturning(at, name) => Message::CliAsmLeavesWithoutReturning {
            place: place(*at),
            name: name.clone(),
        },
        Refusal::Trap(at, mnemonic) => Message::CliAsmTrap {
            place: place(*at),
            mnemonic: mnemonic.clone(),
        },
        Refusal::CallsTooDeep(at, name) => Message::CliAsmCallsTooDeep {
            place: place(*at),
            name: name.clone(),
            limit: calc_asm::CALL_DEPTH_LIMIT.to_string(),
        },
        Refusal::DataRange => Message::CliAsmDataRange,
        Refusal::Loop(at, problem) => loop_message(problem, place(*at)),
        Refusal::InCallee(at, name, inner) => Message::CliAsmInCallee {
            place: place(*at),
            name: name.clone(),
            reason: assembly_refusal_text(inner, locale),
        },
    };
    render(&message, locale).to_string()
}

fn plural(count: &Count) -> u64 {
    if count.low == count.high {
        count.low.parse::<u64>().unwrap_or(PLURAL_OTHER)
    } else {
        PLURAL_OTHER
    }
}

fn count_text(count: &Count, locale: &Locale) -> String {
    if count.low == count.high {
        count.low.clone()
    } else {
        render(
            &Message::CliAsmBetween {
                low: count.low.clone(),
                high: count.high.clone(),
            },
            locale,
        )
        .to_string()
    }
}

pub fn assembly_case_text(case: &Case, locale: &Locale) -> String {
    let counts = match &case.outcome {
        Ok(counts) => counts,
        Err(refusal) => return assembly_refusal_text(refusal, locale),
    };
    let message = match (&counts.reads, &counts.writes) {
        (Some(reads), Some(writes)) => Message::CliAsmCounts {
            instructions: count_text(&counts.instructions, locale),
            instruction_count: plural(&counts.instructions),
            reads: count_text(reads, locale),
            read_count: plural(reads),
            writes: count_text(writes, locale),
            write_count: plural(writes),
        },
        _ => Message::CliAsmCountsWithoutMemory {
            instructions: count_text(&counts.instructions, locale),
            instruction_count: plural(&counts.instructions),
        },
    };
    render(&message, locale).to_string()
}

pub fn assembly_input_text(name: &str, locale: &Locale) -> String {
    let message = match ARGUMENT_REGISTERS
        .iter()
        .position(|aliases| aliases.contains(&name))
    {
        Some(position) => Message::CliAsmInputArgument {
            name: name.to_owned(),
            position: u64::try_from(position + 1).unwrap_or(PLURAL_OTHER),
        },
        None => Message::CliAsmInputRegister {
            name: name.to_owned(),
        },
    };
    render(&message, locale).to_string()
}
