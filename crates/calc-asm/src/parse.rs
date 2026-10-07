use std::collections::HashMap;

use crate::registers::register;

const SIZE_WORDS: [&str; 16] = [
    "byte", "word", "dword", "qword", "tword", "tbyte", "oword", "xmmword", "ymmword", "zmmword",
    "ptr", "strict", "near", "short", "rel", "far",
];
const IGNORED_PREFIXES: [&str; 7] = ["notrack", "bnd", "data16", "cs", "ds", "addr32", "rex.w"];
const REPEAT_PREFIXES: [&str; 6] = ["rep", "repe", "repz", "repne", "repnz", "lock"];
const NASM_DIRECTIVES: [&str; 14] = [
    "bits", "default", "section", "segment", "global", "extern", "align", "alignb", "cpu", "org",
    "use64", "use32", "common", "static",
];
const DATA_DIRECTIVES: [&str; 18] = [
    "db", "dw", "dd", "dq", "dt", "do", "dy", "dz", "resb", "resw", "resd", "resq", "rest", "reso",
    "resy", "resz", "equ", "times",
];
const UNCONDITIONAL_ENDS: [&str; 6] = ["ret", "retq", "retn", "jmp", "jmpq", "ud2"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syntax {
    Assembly,
    Objdump,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Line(usize),
    Address(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Memory {
    pub base: Option<String>,
    pub index: Option<String>,
    pub scale: i128,
    pub displacement: i128,
    pub has_symbol: bool,
    pub size: Option<i128>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operand {
    Register(String),
    Immediate(i128),
    Memory(Memory),
    Target(String),
    Address { label: String, name: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub place: Place,
    pub prefix: Option<String>,
    pub mnemonic: String,
    pub operands: Vec<Operand>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub instructions: Vec<Instruction>,
    pub labels: HashMap<String, usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub syntax: Syntax,
    pub functions: Vec<Function>,
}

pub fn parse(source: &str) -> Program {
    let is_objdump = source.lines().any(|line| objdump_header(line).is_some());
    if is_objdump {
        Program {
            syntax: Syntax::Objdump,
            functions: parse_objdump(source),
        }
    } else {
        Program {
            syntax: Syntax::Assembly,
            functions: parse_assembly(source),
        }
    }
}

fn objdump_header(line: &str) -> Option<(u64, String)> {
    let line = line.trim_end();
    let (address, rest) = line.split_once(' ')?;
    if address.len() < 4 || !address.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let name = rest.strip_prefix('<')?.strip_suffix(">:")?;
    Some((u64::from_str_radix(address, 16).ok()?, name.to_owned()))
}

fn address_label(address: u64) -> String {
    format!("0x{address:x}")
}

fn parse_objdump(source: &str) -> Vec<Function> {
    let mut functions: Vec<Function> = Vec::new();
    for line in source.lines() {
        if let Some((address, name)) = objdump_header(line) {
            let local = functions.last().is_some_and(|current| {
                name.strip_prefix(current.name.as_str())
                    .is_some_and(|rest| rest.starts_with('.'))
            });
            if let (true, Some(current)) = (local, functions.last_mut()) {
                current
                    .labels
                    .insert(address_label(address), current.instructions.len());
            } else {
                functions.push(Function {
                    name,
                    ..Function::default()
                });
            }
            continue;
        }
        if let Some(symbol) = objdump_relocation(line) {
            if let Some(last) = functions
                .last_mut()
                .and_then(|current| current.instructions.last_mut())
                .filter(|last| is_branch(&last.mnemonic))
            {
                last.operands = vec![Operand::Target(symbol)];
            }
            continue;
        }
        let Some((address, text)) = objdump_instruction(line) else {
            continue;
        };
        let Some(current) = functions.last_mut() else {
            continue;
        };
        if let Some(instruction) = parse_instruction(&text, Place::Address(address), true) {
            if let Some(previous) = current.instructions.last_mut()
                && matches!(previous.mnemonic.as_str(), "call" | "callq")
                && matches!(
                    previous.operands.first(),
                    Some(Operand::Address { label, .. }) if *label == address_label(address)
                )
            {
                previous.operands = vec![Operand::Target(UNRESOLVED_CALL.to_owned())];
            }
            current
                .labels
                .insert(address_label(address), current.instructions.len());
            current.instructions.push(instruction);
        }
    }
    functions.retain(|function| !function.instructions.is_empty());
    split_at_call_targets(functions)
}

fn instruction_address(instruction: &Instruction) -> Option<u64> {
    match instruction.place {
        Place::Address(address) => Some(address),
        Place::Line(_) => None,
    }
}

fn split_at_call_targets(functions: Vec<Function>) -> Vec<Function> {
    let targets: std::collections::BTreeSet<String> = functions
        .iter()
        .flat_map(|function| &function.instructions)
        .filter(|instruction| matches!(instruction.mnemonic.as_str(), "call" | "callq"))
        .filter_map(|instruction| match instruction.operands.first() {
            Some(Operand::Address { label, .. }) => Some(label.clone()),
            _ => None,
        })
        .collect();
    let mut pieces = Vec::new();
    for function in functions {
        let mut current = Function {
            name: function.name.clone(),
            ..Function::default()
        };
        for (index, instruction) in function.instructions.into_iter().enumerate() {
            let label = instruction_address(&instruction).map(address_label);
            let is_target = label.as_ref().is_some_and(|label| targets.contains(label));
            if index > 0 && is_target && !current.instructions.is_empty() {
                pieces.push(std::mem::take(&mut current));
                current.name = label.clone().unwrap_or_default();
            }
            if let Some(label) = label {
                current.labels.insert(label, current.instructions.len());
            }
            current.instructions.push(instruction);
        }
        if !current.instructions.is_empty() {
            pieces.push(current);
        }
    }
    pieces
}

pub const UNRESOLVED_CALL: &str = "(relocation)";

fn is_branch(mnemonic: &str) -> bool {
    mnemonic.starts_with('j') || matches!(mnemonic, "call" | "callq")
}

fn objdump_relocation(line: &str) -> Option<String> {
    let (offset, rest) = line.trim_start().split_once(": ")?;
    if offset.is_empty() || !offset.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let (kind, target) = rest.trim().split_once(char::is_whitespace)?;
    if !kind.starts_with("R_X86_64_") {
        return None;
    }
    let target = target.trim();
    let symbol = target
        .split(['+', '-'])
        .next()
        .unwrap_or(target)
        .trim_end_matches("@PLT");
    Some(symbol.to_owned())
}

fn objdump_instruction(line: &str) -> Option<(u64, String)> {
    let (address, rest) = line.trim_start().split_once(":\t")?;
    if !address.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let address = u64::from_str_radix(address, 16).ok()?;
    let parts: Vec<&str> = rest.split('\t').collect();
    let text = match parts.as_slice() {
        [single] if is_raw_bytes(single) => return None,
        [single] => (*single).to_owned(),
        [bytes, instruction, ..] if is_raw_bytes(bytes) => (*instruction).to_owned(),
        [first, ..] => (*first).to_owned(),
        [] => return None,
    };
    Some((address, text))
}

fn is_raw_bytes(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && trimmed
            .split(' ')
            .all(|byte| byte.len() == 2 && byte.chars().all(|c| c.is_ascii_hexdigit()))
}

fn strip_comment(line: &str) -> &str {
    let mut in_quotes = false;
    for (index, character) in line.char_indices() {
        match character {
            '"' => in_quotes = !in_quotes,
            ';' | '#' if !in_quotes => return &line[..index],
            _ => {}
        }
    }
    line
}

fn split_label(line: &str) -> (Option<String>, &str) {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix('"')
        && let Some((name, after)) = rest.split_once('"')
        && let Some(after) = after.strip_prefix(':')
    {
        return (Some(name.to_owned()), after);
    }
    if let Some(name) = trimmed.trim_end().strip_suffix(':')
        && !name.ends_with(':')
        && !name.contains(['[', '\t'])
        && name.contains('(')
    {
        return (Some(name.to_owned()), "");
    }
    let end = trimmed
        .find(|c: char| !(c.is_ascii_alphanumeric() || "_.$@?".contains(c)))
        .unwrap_or(trimmed.len());
    let (name, after) = trimmed.split_at(end);
    match after.strip_prefix(':') {
        Some(rest) if !name.is_empty() && !rest.starts_with(':') => (Some(name.to_owned()), rest),
        _ => (None, trimmed),
    }
}

fn is_directive(text: &str) -> bool {
    let mut words = text.split_whitespace();
    let Some(first) = words.next() else {
        return true;
    };
    let first = first.to_ascii_lowercase();
    first.starts_with('.')
        || first.starts_with('%')
        || first.starts_with('[')
        || NASM_DIRECTIVES.contains(&first.as_str())
        || DATA_DIRECTIVES.contains(&first.as_str())
        || words
            .next()
            .is_some_and(|second| DATA_DIRECTIVES.contains(&second.to_ascii_lowercase().as_str()))
}

fn declared_functions(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let text = strip_comment(line).trim();
            let mut words = text.split_whitespace();
            let keyword = words.next()?.to_ascii_lowercase();
            matches!(keyword.as_str(), "global" | ".globl" | ".global").then(|| {
                words
                    .flat_map(|word| word.split(','))
                    .filter(|name| !name.is_empty())
                    .map(|name| name.trim_matches('"').to_owned())
                    .collect::<Vec<_>>()
            })
        })
        .flatten()
        .collect()
}

fn parse_assembly(source: &str) -> Vec<Function> {
    let declared = declared_functions(source);
    let mut functions: Vec<Function> = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let (label, rest) = split_label(strip_comment(line));
        if let Some(label) = label {
            let is_local = label.starts_with('.');
            let current_open = functions.last().is_some_and(|function| {
                function
                    .instructions
                    .last()
                    .is_none_or(|last| !UNCONDITIONAL_ENDS.contains(&last.mnemonic.as_str()))
            });
            let starts_function =
                !is_local && (declared.contains(&label) || !current_open || functions.is_empty());
            let orphan_local = is_local && functions.is_empty();
            match functions.last_mut() {
                Some(current) if !starts_function => {
                    current.labels.insert(label, current.instructions.len());
                }
                _ if orphan_local => {}
                _ => functions.push(Function {
                    name: label,
                    ..Function::default()
                }),
            }
        }
        let text = rest.trim();
        if text.is_empty() || is_directive(text) {
            continue;
        }
        if functions.is_empty() {
            functions.push(Function::default());
        }
        if let (Some(current), Some(instruction)) = (
            functions.last_mut(),
            parse_instruction(text, Place::Line(index + 1), false),
        ) {
            current.instructions.push(instruction);
        }
    }
    functions.retain(|function| !function.instructions.is_empty());
    functions
}

fn parse_instruction(text: &str, place: Place, is_objdump: bool) -> Option<Instruction> {
    let mut rest = text.trim();
    let mut prefix = None;
    loop {
        let (word, after) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let lower = word.to_ascii_lowercase();
        if IGNORED_PREFIXES.contains(&lower.as_str()) {
            rest = after.trim_start();
        } else if REPEAT_PREFIXES.contains(&lower.as_str()) && !after.trim().is_empty() {
            prefix = Some(lower);
            rest = after.trim_start();
        } else {
            break;
        }
    }
    let (mnemonic, operand_text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    if mnemonic.is_empty() {
        return None;
    }
    let mut operands: Vec<Operand> = split_operands(operand_text)
        .iter()
        .map(|operand| parse_operand(operand, is_objdump))
        .map(|operand| match operand {
            Operand::Immediate(value) if is_objdump && is_branch(mnemonic) => u64::try_from(value)
                .map_or(Operand::Immediate(value), |address| Operand::Address {
                    label: address_label(address),
                    name: String::new(),
                }),
            other => other,
        })
        .collect();
    if let Some(bits) = operands.iter().find_map(operand_bits) {
        for operand in &mut operands {
            if let Operand::Immediate(value) = operand {
                *value = signed_at(*value, bits);
            }
        }
    }
    Some(Instruction {
        place,
        prefix,
        mnemonic: mnemonic.to_ascii_lowercase(),
        operands,
    })
}

fn operand_bits(operand: &Operand) -> Option<u32> {
    match operand {
        Operand::Register(name) => {
            let found = register(name)?;
            match found.width {
                crate::registers::Width::Full if crate::registers::is_general(&found.family) => {
                    Some(64)
                }
                crate::registers::Width::Full => None,
                crate::registers::Width::Double => Some(32),
                crate::registers::Width::Partial => {
                    let byte = name.ends_with('l')
                        || name.ends_with('b')
                        || name.len() == 2 && name.ends_with('h');
                    Some(if byte { 8 } else { 16 })
                }
            }
        }
        Operand::Memory(memory) => memory.size.and_then(|size| u32::try_from(size * 8).ok()),
        _ => None,
    }
}

fn signed_at(value: i128, bits: u32) -> i128 {
    if bits >= 127 {
        return value;
    }
    let span = 1_i128 << bits;
    if value >= span / 2 && value < span {
        value - span
    } else {
        value
    }
}

fn split_operands(text: &str) -> Vec<String> {
    let mut operands = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();
    for character in text.chars() {
        match character {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            _ => {}
        }
        if character == ',' && depth == 0 {
            operands.push(current.trim().to_owned());
            current.clear();
        } else {
            current.push(character);
        }
    }
    if !current.trim().is_empty() {
        operands.push(current.trim().to_owned());
    }
    operands
}

fn number(text: &str) -> Option<i128> {
    let text = text.trim();
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest.trim()),
        None => (false, text.strip_prefix('+').unwrap_or(text).trim()),
    };
    let lower = digits.to_ascii_lowercase();
    let value = if let Some(hex) = lower.strip_prefix("0x") {
        i128::from_str_radix(hex, 16).ok()?
    } else if let Some(hex) = lower.strip_suffix('h').filter(|hex| {
        hex.starts_with(|c: char| c.is_ascii_digit()) && hex.chars().all(|c| c.is_ascii_hexdigit())
    }) {
        i128::from_str_radix(hex, 16).ok()?
    } else if let Some(binary) = lower.strip_prefix("0b") {
        i128::from_str_radix(binary, 2).ok()?
    } else {
        lower.parse::<i128>().ok()?
    };
    Some(if negative { -value } else { value })
}

fn without_size_words(text: &str) -> String {
    text.split_whitespace()
        .filter(|word| !SIZE_WORDS.contains(&word.to_ascii_lowercase().as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn size_of(text: &str) -> Option<i128> {
    text.split_whitespace()
        .find_map(|word| match word.to_ascii_lowercase().as_str() {
            "byte" => Some(1),
            "word" => Some(2),
            "dword" => Some(4),
            "qword" => Some(8),
            "tword" | "tbyte" => Some(10),
            "oword" | "xmmword" => Some(16),
            "ymmword" => Some(32),
            "zmmword" => Some(64),
            _ => None,
        })
}

fn parse_operand(text: &str, is_objdump: bool) -> Operand {
    let has_ptr = text.to_ascii_lowercase().contains("ptr");
    let size = size_of(text);
    let cleaned = without_size_words(text);
    if let Some(open) = cleaned.find('[') {
        let close = cleaned.rfind(']').unwrap_or(cleaned.len());
        let inside = cleaned.get(open + 1..close).unwrap_or_default();
        let mut memory = parse_address(inside);
        memory.size = size;
        let before = cleaned[..open].trim();
        let before = before
            .rsplit_once(':')
            .map_or(before, |(segment, rest)| {
                memory.has_symbol |= register(segment.trim()).is_some();
                rest
            })
            .trim();
        if !before.is_empty() {
            match number(before) {
                Some(value) => memory.displacement += value,
                None => memory.has_symbol = true,
            }
        }
        return Operand::Memory(memory);
    }
    if has_ptr {
        return Operand::Memory(Memory {
            base: None,
            index: None,
            scale: 1,
            displacement: 0,
            has_symbol: true,
            size,
        });
    }
    let first = cleaned.split_whitespace().next().unwrap_or_default();
    if is_objdump
        && !first.is_empty()
        && first.chars().all(|c| c.is_ascii_hexdigit())
        && cleaned.contains('<')
        && let Ok(address) = u64::from_str_radix(first, 16)
    {
        let name = cleaned
            .split_once('<')
            .and_then(|(_, rest)| rest.split_once('>'))
            .map_or("", |(name, _)| name);
        let name = name.split_once('+').map_or(name, |(base, _)| base);
        let name = name.split_once('-').map_or(name, |(base, _)| base);
        return Operand::Address {
            label: address_label(address),
            name: name.to_owned(),
        };
    }
    if register(&cleaned).is_some() {
        return Operand::Register(cleaned.to_ascii_lowercase());
    }
    if let Some(value) = number(&cleaned) {
        return Operand::Immediate(value);
    }
    let unquoted = cleaned.replace('"', "");
    let name = unquoted
        .strip_suffix("@PLT")
        .or_else(|| unquoted.strip_suffix("@plt"))
        .unwrap_or(&unquoted);
    Operand::Target(name.to_owned())
}

fn parse_address(inside: &str) -> Memory {
    let mut memory = Memory {
        base: None,
        index: None,
        scale: 1,
        displacement: 0,
        has_symbol: false,
        size: None,
    };
    let inside = inside.rsplit_once(':').map_or(inside, |(segment, rest)| {
        if register(segment.trim()).is_some() {
            memory.has_symbol = true;
        }
        rest
    });
    let mut terms: Vec<(bool, String)> = Vec::new();
    let mut current = String::new();
    let mut negative = false;
    for character in inside.chars() {
        if (character == '+' || character == '-') && !current.trim().is_empty() {
            terms.push((negative, current.trim().to_owned()));
            current.clear();
            negative = character == '-';
        } else if character == '-' {
            negative = !negative;
        } else if character != '+' {
            current.push(character);
        }
    }
    if !current.trim().is_empty() {
        terms.push((negative, current.trim().to_owned()));
    }
    for (negative, term) in terms {
        if let Some((left, right)) = term.split_once('*') {
            let (name, factor) = if register(left.trim()).is_some() {
                (left.trim(), right.trim())
            } else {
                (right.trim(), left.trim())
            };
            if register(name).is_some() {
                memory.index = Some(name.to_ascii_lowercase());
                memory.scale = number(factor).unwrap_or(1);
            } else {
                memory.has_symbol = true;
            }
        } else if register(&term).is_some() {
            if memory.base.is_none() {
                memory.base = Some(term.to_ascii_lowercase());
            } else {
                memory.index = Some(term.to_ascii_lowercase());
            }
        } else if let Some(value) = number(&term) {
            memory.displacement += if negative { -value } else { value };
        } else {
            memory.has_symbol = true;
        }
    }
    if memory.base.as_deref() == Some("rip") {
        memory.has_symbol = true;
    }
    memory
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = "\"square(int)\":\n        push    rbp\n        mov     rbp, rsp\n        mov     DWORD PTR [rbp-4], edi\n        mov     eax, DWORD PTR [rbp-4]\n        imul    eax, eax\n        pop     rbp\n        ret\n";

    #[test]
    fn a_quoted_label_names_a_function() {
        let program = parse(SQUARE);
        assert_eq!(program.functions.len(), 1);
        assert_eq!(program.functions[0].name, "square(int)");
        assert_eq!(program.functions[0].instructions.len(), 7);
    }

    #[test]
    fn a_stack_operand_has_base_and_displacement() {
        let program = parse(SQUARE);
        assert_eq!(
            program.functions[0].instructions[2].operands[0],
            Operand::Memory(Memory {
                base: Some("rbp".to_owned()),
                index: None,
                scale: 1,
                displacement: -4,
                has_symbol: false,
                size: Some(4),
            })
        );
    }

    #[test]
    fn a_label_inside_a_function_does_not_start_another() {
        let program =
            parse("delay:\n    mov ecx, 100\nagain:\n    dec ecx\n    jnz again\n    ret\n");
        assert_eq!(program.functions.len(), 1);
        assert_eq!(program.functions[0].labels.get("again"), Some(&1));
    }

    #[test]
    fn objdump_lines_become_instructions_with_address_labels() {
        let text = "0000000000001760 <f>:\n    1760:\t31 c0                \txor    eax,eax\n    1762:\t7e 13                \tjle    177c <f.end>\n\n000000000000177c <f.end>:\n    177c:\tc3                   \tret\n";
        let program = parse(text);
        assert_eq!(program.syntax, Syntax::Objdump);
        assert_eq!(program.functions.len(), 1);
        assert_eq!(
            program.functions[0].instructions[1].operands[0],
            Operand::Address {
                label: "0x177c".to_owned(),
                name: "f.end".to_owned()
            }
        );
        assert_eq!(program.functions[0].labels.get("0x177c"), Some(&2));
    }

    #[test]
    fn a_displacement_before_the_brackets_is_added() {
        let program = parse("f:\n mov DWORD PTR -8[rbp], 0\n ret\n");
        let Operand::Memory(memory) = &program.functions[0].instructions[0].operands[0] else {
            panic!("a memory operand");
        };
        assert_eq!(
            (memory.base.as_deref(), memory.displacement),
            (Some("rbp"), -8)
        );
    }

    #[test]
    fn nasm_hexadecimal_with_suffix_is_a_number() {
        assert_eq!(number("0FFh"), Some(255));
        assert_eq!(number("-4"), Some(-4));
    }
}
