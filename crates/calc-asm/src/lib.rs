mod analyze;
mod linear;
mod parse;
mod registers;
mod semantics;

pub use analyze::{
    CALL_DEPTH_LIMIT, Case, Count, Counts, FunctionReport, LoopProblem, PATH_LIMIT, Refusal,
    analyze,
};
pub use parse::{Place, Program, Syntax, UNRESOLVED_CALL, parse};

pub fn is_general_register(name: &str) -> bool {
    registers::register(name).is_some_and(|found| registers::is_general(&found.family))
}
