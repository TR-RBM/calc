use crate::costs::Source;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BubbleForm {
    Full,
    Shrinking,
    EarlyExit,
    LastExchange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShakerForm {
    Full,
    Shrinking,
    LastExchange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OddEvenForm {
    UntilSorted,
    FixedPasses,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gaps {
    Shell,
    Knuth,
    Ciura,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombForm {
    LaceyBox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Partition {
    LomutoLast,
    HoareFirst,
    LomutoRandom { seed: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Method {
    Insertion,
    BinaryInsertion,
    Selection,
    Bubble(BubbleForm),
    Merge,
    Heap,
    Quick(Partition),
    Counting,
    DoubleSelection,
    CocktailShaker(ShakerForm),
    Gnome,
    OddEven(OddEvenForm),
    Comb(CombForm),
    Cycle,
    Pancake,
    Shell(Gaps),
    BottomUpMerge,
    NaturalMerge,
    Radix { base: u64 },
    Bead,
    Bitonic,
    Bogo { seed: u64, limit: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Order {
    Increasing,
    Decreasing,
}

pub fn is_stable(method: Method) -> bool {
    match method {
        Method::Bead | Method::Bitonic | Method::Bogo { .. } => false,
        Method::Insertion
        | Method::BinaryInsertion
        | Method::Bubble(_)
        | Method::Merge
        | Method::Counting
        | Method::CocktailShaker(_)
        | Method::Gnome
        | Method::OddEven(_)
        | Method::BottomUpMerge
        | Method::NaturalMerge
        | Method::Radix { .. } => true,
        Method::Selection
        | Method::Heap
        | Method::Quick(_)
        | Method::DoubleSelection
        | Method::Comb(_)
        | Method::Cycle
        | Method::Pancake
        | Method::Shell(_) => false,
    }
}

pub fn method_source(method: Method) -> Source {
    match method {
        Method::Insertion => Source::VitterFlajolet1990Insertion,
        Method::Selection => Source::VitterFlajolet1990Selection,
        Method::Bubble(BubbleForm::LastExchange) => Source::VitterFlajolet1990Bubble,
        Method::BinaryInsertion => Source::WikipediaInsertionSort,
        Method::Merge => Source::FlajoletGolin1994,
        Method::Heap => Source::WikipediaHeapsort,
        Method::Quick(Partition::LomutoLast) => Source::WikipediaQuicksortLomuto,
        Method::Quick(Partition::LomutoRandom { .. }) => Source::CormenRandomizedQuicksort,
        Method::Quick(Partition::HoareFirst) => Source::WikipediaQuicksortHoare,
        Method::Counting => Source::WikipediaCountingSort,
        Method::DoubleSelection => Source::WikipediaSelectionSortVariants,
        Method::CocktailShaker(_) => Source::WikipediaCocktailShakerSort,
        Method::Gnome => Source::WikipediaGnomeSort,
        Method::OddEven(_) => Source::WikipediaOddEvenSort,
        Method::Comb(CombForm::LaceyBox) => Source::WikipediaCombSort,
        Method::Cycle => Source::WikipediaCycleSort,
        Method::Pancake => Source::WikipediaPancakeSorting,
        Method::Shell(_) => Source::WikipediaShellsort,
        Method::BottomUpMerge => Source::WikipediaMergeSortBottomUp,
        Method::NaturalMerge => Source::WikipediaMergeSortNatural,
        Method::Radix { .. } => Source::WikipediaRadixSort,
        Method::Bead => Source::ArulanandhamCaludeDinneen2002,
        Method::Bitonic => Source::Batcher1968Bitonic,
        Method::Bogo { .. } => Source::GruberHolzerRuepp2007,
        Method::Bubble(BubbleForm::Full | BubbleForm::Shrinking | BubbleForm::EarlyExit) => {
            Source::WikipediaBubbleSort
        }
    }
}
