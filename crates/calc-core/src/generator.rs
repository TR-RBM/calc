use calc_expr::{ExprId, ExprPool, Head, NodeView, Operator};
use calc_numbers::Number;

use crate::exact_evaluation::evaluate_exact;

pub const PHILOX_GENERATOR: &str = "philox4x32_10_1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedBlock {
    pub words: [u32; 4],
    pub seed: u64,
    pub stream: u64,
    pub index: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratorArgument {
    Seed,
    Stream,
    Index,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratorRefusal {
    pub argument: GeneratorArgument,
    pub expression: ExprId,
}

pub fn philox_block(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<Result<GeneratedBlock, GeneratorRefusal>> {
    let NodeView::Apply {
        head: Head::Operator(Operator::Philox4x32_10),
        arguments: [seed, stream, index],
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let (seed, stream, index) = (*seed, *stream, *index);
    Some(generated(pool, seed, stream, index))
}

pub(crate) fn generated(
    pool: &mut ExprPool,
    seed: ExprId,
    stream: ExprId,
    index: ExprId,
) -> Result<GeneratedBlock, GeneratorRefusal> {
    let seed = sixty_four_bits(pool, seed, GeneratorArgument::Seed)?;
    let stream = sixty_four_bits(pool, stream, GeneratorArgument::Stream)?;
    let index = sixty_four_bits(pool, index, GeneratorArgument::Index)?;
    Ok(GeneratedBlock {
        words: calc_kernels::philox4x32_10_at(seed, stream, index),
        seed,
        stream,
        index,
    })
}

fn sixty_four_bits(
    pool: &mut ExprPool,
    expression: ExprId,
    argument: GeneratorArgument,
) -> Result<u64, GeneratorRefusal> {
    let refusal = GeneratorRefusal {
        argument,
        expression,
    };
    if crate::sorting::is_written_in_a_unit(pool, expression)
        || crate::quantities::to_coherent_units(pool, expression)
            .is_ok_and(|coherent| coherent.unit.is_some())
    {
        return Err(refusal);
    }
    let value = evaluate_exact(pool, expression)
        .ok()
        .and_then(|evaluation| evaluation.rational_value().cloned());
    match value {
        Some(Number::Integer(integer)) => integer
            .to_i128()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(refusal),
        _ => Err(refusal),
    }
}
