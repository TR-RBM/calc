#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExprId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NumberId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub(crate) u32);

pub(crate) fn position(index: u32) -> usize {
    usize::try_from(index).unwrap_or(usize::MAX)
}

pub(crate) fn next_index(length: usize, entry_limit: u32) -> Option<u32> {
    u32::try_from(length)
        .ok()
        .filter(|index| *index < entry_limit)
}
