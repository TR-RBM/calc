use crate::plural::{PluralCategory, PluralRule};

pub type Pattern = &'static [Segment];

#[derive(Debug)]
pub enum Segment {
    Text(&'static str),
    Argument(usize),
    Select(Select),
}

#[derive(Debug)]
pub struct Select {
    pub selector: usize,
    pub variants: &'static [Variant],
    pub default: &'static [Inline],
}

#[derive(Debug)]
pub struct Variant {
    pub key: VariantKey,
    pub elements: &'static [Inline],
}

#[derive(Debug)]
pub enum Inline {
    Text(&'static str),
    Argument(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariantKey {
    Category(PluralCategory),
    Exact(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argument<'a> {
    Text(&'a str),
    Count(u64),
}

#[derive(Debug)]
pub struct LocaleEntry {
    pub tag: &'static str,
    pub plural_rule: PluralRule,
    pub patterns: &'static [Option<Pattern>],
}
