use crate::parse::{Instruction, Operand};

const CONDITIONAL_JUMPS: [&str; 32] = [
    "je", "jz", "jne", "jnz", "jl", "jnge", "jle", "jng", "jg", "jnle", "jge", "jnl", "jb", "jc",
    "jnae", "jbe", "jna", "ja", "jnbe", "jae", "jnb", "jnc", "js", "jns", "jo", "jno", "jp", "jpe",
    "jnp", "jpo", "jcxz", "jecxz",
];

const WRITE_ONLY_DESTINATION: [&str; 30] = [
    "mov", "movzx", "movsx", "movsxd", "movabs", "movq", "movd", "movdqa", "movdqu", "movaps",
    "movups", "movapd", "movupd", "movss", "movsd", "movhps", "movlps", "movnti", "movntdq",
    "vmovdqa", "vmovdqu", "vmovaps", "vmovups", "vmovss", "vmovsd", "vmovq", "vmovd", "movbe",
    "cvtsi2sd", "cvtsi2ss",
];

const READ_WRITE_DESTINATION: [&str; 32] = [
    "add", "sub", "and", "or", "xor", "adc", "sbb", "inc", "dec", "neg", "not", "shl", "sal",
    "shr", "sar", "rol", "ror", "rcl", "rcr", "xchg", "xadd", "cmpxchg", "bts", "btr", "btc",
    "shld", "shrd", "addss", "addsd", "subss", "subsd", "imul",
];

const READ_ONLY: [&str; 26] = [
    "cmp",
    "test",
    "bt",
    "push",
    "div",
    "idiv",
    "mul",
    "movmskps",
    "ucomiss",
    "ucomisd",
    "comiss",
    "comisd",
    "pmulld",
    "paddq",
    "paddd",
    "psubd",
    "pxor",
    "pand",
    "por",
    "pmovsxdq",
    "punpckhqdq",
    "mulss",
    "mulsd",
    "divss",
    "divsd",
    "sqrtsd",
];

const NO_MEMORY: [&str; 14] = [
    "lea",
    "nop",
    "endbr64",
    "endbr32",
    "prefetcht0",
    "prefetcht1",
    "prefetcht2",
    "prefetchnta",
    "prefetchw",
    "cdq",
    "cqo",
    "cdqe",
    "cwde",
    "pause",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Plain,
    Return,
    Jump,
    ConditionalJump,
    Call,
    SystemCall,
    Unsupported,
}

pub fn control(instruction: &Instruction) -> Control {
    let mnemonic = instruction.mnemonic.as_str();
    match mnemonic {
        "ret" | "retq" | "retn" => Control::Return,
        "jmp" | "jmpq" => Control::Jump,
        "call" | "callq" => Control::Call,
        "syscall" | "sysenter" | "int" | "int3" | "ud2" | "hlt" => Control::SystemCall,
        "loop" | "loope" | "loopne" | "loopz" | "loopnz" | "jcxz" | "jecxz" | "jrcxz" => {
            Control::Unsupported
        }
        _ if CONDITIONAL_JUMPS.contains(&mnemonic) => Control::ConditionalJump,
        _ => Control::Plain,
    }
}

pub fn has_repeat_prefix(instruction: &Instruction) -> bool {
    instruction
        .prefix
        .as_deref()
        .is_some_and(|prefix| prefix.starts_with("rep"))
        && !matches!(instruction.mnemonic.as_str(), "ret" | "retq")
}

pub fn writes_destination(mnemonic: &str) -> bool {
    WRITE_ONLY_DESTINATION.contains(&mnemonic)
        || READ_WRITE_DESTINATION.contains(&mnemonic)
        || mnemonic == "lea"
        || mnemonic == "pop"
        || mnemonic.starts_with("set")
        || mnemonic.starts_with("cmov")
        || !(READ_ONLY.contains(&mnemonic) || NO_MEMORY.contains(&mnemonic))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Accesses {
    pub reads: i128,
    pub writes: i128,
}

pub fn accesses(instruction: &Instruction) -> Option<Accesses> {
    let mnemonic = instruction.mnemonic.as_str();
    let mut counted = Accesses::default();
    match mnemonic {
        "push" => counted.writes += 1,
        "pop" => counted.reads += 1,
        "call" | "callq" => counted.writes += 1,
        "ret" | "retq" | "retn" => counted.reads += 1,
        "leave" | "leaveq" => counted.reads += 1,
        _ => {}
    }
    let memory_positions: Vec<usize> = instruction
        .operands
        .iter()
        .enumerate()
        .filter(|(_, operand)| matches!(operand, Operand::Memory(_)))
        .map(|(position, _)| position)
        .collect();
    if instruction.operands.is_empty()
        && let Some(string) = string_accesses(mnemonic)
    {
        return Some(string);
    }
    if memory_positions.is_empty() || NO_MEMORY.contains(&mnemonic) {
        return Some(counted);
    }
    let known = WRITE_ONLY_DESTINATION.contains(&mnemonic)
        || READ_WRITE_DESTINATION.contains(&mnemonic)
        || READ_ONLY.contains(&mnemonic)
        || mnemonic.starts_with("set")
        || mnemonic.starts_with("cmov")
        || matches!(mnemonic, "pop" | "call" | "callq" | "jmp" | "jmpq");
    if !known {
        return None;
    }
    for position in memory_positions {
        let is_destination = position == 0 && instruction.operands.len() > 1;
        let single = instruction.operands.len() == 1;
        if mnemonic.starts_with("set")
            || mnemonic == "pop"
            || (WRITE_ONLY_DESTINATION.contains(&mnemonic) && is_destination)
        {
            counted.writes += 1;
        } else if READ_WRITE_DESTINATION.contains(&mnemonic) && (is_destination || single) {
            if mnemonic == "imul" && instruction.operands.len() > 1 {
                counted.reads += 1;
            } else {
                counted.reads += 1;
                counted.writes += 1;
            }
        } else {
            counted.reads += 1;
        }
    }
    Some(counted)
}

fn string_accesses(mnemonic: &str) -> Option<Accesses> {
    let stem = mnemonic.trim_end_matches(['b', 'w', 'd', 'q']);
    let (reads, writes) = match stem {
        "movs" => (1, 1),
        "stos" => (0, 1),
        "lods" | "scas" => (1, 0),
        "cmps" => (2, 0),
        _ => return None,
    };
    Some(Accesses { reads, writes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn first(text: &str) -> Instruction {
        parse(text).functions[0].instructions[0].clone()
    }

    #[test]
    fn a_store_is_one_write() {
        assert_eq!(
            accesses(&first("f:\n mov DWORD PTR [rbp-4], edi\n")),
            Some(Accesses {
                reads: 0,
                writes: 1
            })
        );
    }

    #[test]
    fn adding_to_memory_reads_and_writes() {
        assert_eq!(
            accesses(&first("f:\n add DWORD PTR [rdi], 1\n")),
            Some(Accesses {
                reads: 1,
                writes: 1
            })
        );
    }

    #[test]
    fn lea_does_not_touch_memory() {
        assert_eq!(
            accesses(&first("f:\n lea rax, [rbp-8]\n")),
            Some(Accesses::default())
        );
    }

    #[test]
    fn an_unknown_instruction_with_memory_is_not_counted() {
        assert_eq!(
            accesses(&first("f:\n vfmadd231ps ymm0, ymm1, [rax]\n")),
            None
        );
    }
}
