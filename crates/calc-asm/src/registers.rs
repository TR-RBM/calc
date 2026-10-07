const GENERAL: [(&str, [&str; 4]); 16] = [
    ("rax", ["eax", "ax", "al", "ah"]),
    ("rbx", ["ebx", "bx", "bl", "bh"]),
    ("rcx", ["ecx", "cx", "cl", "ch"]),
    ("rdx", ["edx", "dx", "dl", "dh"]),
    ("rsi", ["esi", "si", "sil", ""]),
    ("rdi", ["edi", "di", "dil", ""]),
    ("rbp", ["ebp", "bp", "bpl", ""]),
    ("rsp", ["esp", "sp", "spl", ""]),
    ("r8", ["r8d", "r8w", "r8b", ""]),
    ("r9", ["r9d", "r9w", "r9b", ""]),
    ("r10", ["r10d", "r10w", "r10b", ""]),
    ("r11", ["r11d", "r11w", "r11b", ""]),
    ("r12", ["r12d", "r12w", "r12b", ""]),
    ("r13", ["r13d", "r13w", "r13b", ""]),
    ("r14", ["r14d", "r14w", "r14b", ""]),
    ("r15", ["r15d", "r15w", "r15b", ""]),
];

pub const GENERAL_FAMILIES: [&str; 16] = [
    "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11", "r12", "r13",
    "r14", "r15",
];

pub const CALLER_SAVED: [&str; 9] = ["rax", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11"];

const VECTOR_PREFIXES: [&str; 5] = ["xmm", "ymm", "zmm", "mm", "k"];
const OTHER: [&str; 8] = ["rip", "eip", "fs", "gs", "cs", "ds", "es", "ss"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    Full,
    Double,
    Partial,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Register {
    pub family: String,
    pub width: Width,
}

pub fn register(name: &str) -> Option<Register> {
    let name = name.to_ascii_lowercase();
    for (family, aliases) in GENERAL {
        if name == family {
            return Some(Register {
                family: family.to_owned(),
                width: Width::Full,
            });
        }
        if let Some(position) = aliases
            .iter()
            .position(|alias| !alias.is_empty() && *alias == name)
        {
            let width = if position == 0 {
                Width::Double
            } else {
                Width::Partial
            };
            return Some(Register {
                family: family.to_owned(),
                width,
            });
        }
    }
    let vector = VECTOR_PREFIXES.iter().any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
    });
    (vector || OTHER.contains(&name.as_str())).then(|| Register {
        family: name.clone(),
        width: Width::Full,
    })
}

pub fn is_general(family: &str) -> bool {
    GENERAL.iter().any(|(name, _)| *name == family)
}

pub fn name_at(family: &str, width: Width) -> String {
    GENERAL
        .iter()
        .find(|(name, _)| *name == family)
        .map_or_else(
            || family.to_owned(),
            |(name, aliases)| match width {
                Width::Double => aliases[0].to_owned(),
                Width::Full | Width::Partial => (*name).to_owned(),
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edx_is_the_double_word_of_rdx() {
        assert_eq!(
            register("EDX"),
            Some(Register {
                family: "rdx".to_owned(),
                width: Width::Double
            })
        );
    }

    #[test]
    fn xmm_registers_are_their_own_family() {
        assert_eq!(
            register("xmm12").map(|found| found.family),
            Some("xmm12".to_owned())
        );
    }

    #[test]
    fn a_label_is_not_a_register() {
        assert_eq!(register("loop"), None);
    }
}
