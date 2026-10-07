use calc_numbers::{ExactArithmeticError, Number};

use crate::formula::Formula;
use crate::method::{BubbleForm, Gaps, Method, OddEvenForm, Partition, ShakerForm};

pub const BOGO_CHECK_LIMIT: u64 = 6;

pub const EXHAUSTIVE_CHECK_LIMIT: u64 = 8;

pub const PIVOT_CHOICE_CHECK_LIMIT: u64 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Over {
    Orders,
    OrdersAndPivotChoices,
    OrdersAtPowersOfTwo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Measure {
    Comparisons,
    Writes,
    Flips,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Case {
    Best,
    Worst,
    Average,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    VitterFlajolet1990Insertion,
    VitterFlajolet1990Selection,
    VitterFlajolet1990Bubble,
    WikipediaInsertionSort,
    WikipediaBubbleSort,
    FlajoletGolin1994,
    WikipediaHeapsort,
    WikipediaQuicksortLomuto,
    WikipediaQuicksortHoare,
    WikipediaCountingSort,
    CormenRandomizedQuicksort,
    WikipediaSelectionSortVariants,
    WikipediaCocktailShakerSort,
    WikipediaGnomeSort,
    WikipediaOddEvenSort,
    WikipediaCombSort,
    WikipediaCycleSort,
    WikipediaPancakeSorting,
    WikipediaShellsort,
    WikipediaMergeSortBottomUp,
    WikipediaMergeSortNatural,
    WikipediaRadixSort,
    ArulanandhamCaludeDinneen2002,
    Batcher1968Bitonic,
    GruberHolzerRuepp2007,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Provenance {
    DerivedAndChecked { basis: Option<Source> },
    FittedAndChecked,
    NotInClosedForm,
    PerPassOfTheKeys,
    Unbounded,
    PerShuffle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cost {
    pub measure: Measure,
    pub case: Case,
    pub formula: Option<Formula>,
    pub holds_from: u64,
    pub provenance: Provenance,
    pub over: Over,
    pub checked_up_to: u64,
}

impl Cost {
    pub fn value_at(&self, length: u64) -> Option<Result<Number, ExactArithmeticError>> {
        let formula = self.formula.as_ref()?;
        (length >= self.holds_from).then(|| formula.value_at(length))
    }
}

pub fn costs(method: Method) -> Vec<Cost> {
    match method {
        Method::Insertion => insertion_costs(),
        Method::BinaryInsertion => binary_insertion_costs(),
        Method::Selection => selection_costs(),
        Method::Bubble(form) => bubble_costs(form),
        Method::Merge => merge_costs(),
        Method::Heap => heap_costs(),
        Method::Quick(Partition::LomutoLast) => lomuto_costs(),
        Method::Quick(Partition::HoareFirst) => hoare_costs(),
        Method::Quick(Partition::LomutoRandom { .. }) => random_lomuto_costs(),
        Method::Counting => counting_costs(),
        Method::DoubleSelection => double_selection_costs(),
        Method::CocktailShaker(form) => cocktail_shaker_costs(form),
        Method::Gnome => gnome_costs(),
        Method::OddEven(form) => odd_even_costs(form),
        Method::Comb(_) => comb_costs(),
        Method::Cycle => cycle_costs(),
        Method::Pancake => pancake_costs(),
        Method::Shell(gaps) => shell_costs(gaps),
        Method::BottomUpMerge => bottom_up_merge_costs(),
        Method::NaturalMerge => natural_merge_costs(),
        Method::Radix { .. } => radix_costs(),
        Method::Bead => bead_costs(),
        Method::Bitonic => bitonic_costs(),
        Method::Bogo { .. } => bogo_costs(),
    }
}

fn length_less(whole: i64) -> Formula {
    Formula::Difference(Box::new(Formula::Length), Box::new(Formula::Whole(whole)))
}

fn pairs_over(divisor: i64) -> Formula {
    Formula::Quotient(
        Box::new(Formula::Product(vec![Formula::Length, length_less(1)])),
        Box::new(Formula::Whole(divisor)),
    )
}

fn twice(formula: Formula) -> Formula {
    Formula::Product(vec![Formula::Whole(2), formula])
}

fn length_less_harmonic() -> Formula {
    Formula::Difference(Box::new(Formula::Length), Box::new(Formula::Harmonic))
}

fn stated(
    measure: Measure,
    case: Case,
    formula: Formula,
    holds_from: u64,
    basis: Option<Source>,
) -> Cost {
    Cost {
        measure,
        case,
        formula: Some(formula),
        holds_from,
        provenance: Provenance::DerivedAndChecked { basis },
        over: Over::Orders,
        checked_up_to: EXHAUSTIVE_CHECK_LIMIT,
    }
}

fn not_in_closed_form(measure: Measure, case: Case) -> Cost {
    Cost {
        measure,
        case,
        formula: None,
        holds_from: 0,
        provenance: Provenance::NotInClosedForm,
        over: Over::Orders,
        checked_up_to: EXHAUSTIVE_CHECK_LIMIT,
    }
}

fn insertion_costs() -> Vec<Cost> {
    let source = Some(Source::VitterFlajolet1990Insertion);
    vec![
        stated(Measure::Comparisons, Case::Best, length_less(1), 1, None),
        stated(Measure::Comparisons, Case::Worst, pairs_over(2), 0, None),
        stated(
            Measure::Comparisons,
            Case::Average,
            Formula::Difference(
                Box::new(Formula::Sum(vec![pairs_over(4), Formula::Length])),
                Box::new(Formula::Harmonic),
            ),
            0,
            source,
        ),
        stated(Measure::Writes, Case::Best, length_less(1), 1, None),
        stated(
            Measure::Writes,
            Case::Worst,
            Formula::Sum(vec![pairs_over(2), length_less(1)]),
            1,
            None,
        ),
        stated(
            Measure::Writes,
            Case::Average,
            Formula::Sum(vec![pairs_over(4), length_less(1)]),
            1,
            source,
        ),
    ]
}

fn binary_search_depths() -> Formula {
    let depth = Formula::FloorLog2(Box::new(Formula::Index));
    let full_levels = Formula::Product(vec![Formula::Index, depth.clone()]);
    let last_level = twice(Formula::Difference(
        Box::new(Formula::Index),
        Box::new(Formula::PowerOfTwo(Box::new(depth))),
    ));
    Formula::SumOver {
        from: 2,
        body: Box::new(Formula::Quotient(
            Box::new(Formula::Sum(vec![full_levels, last_level])),
            Box::new(Formula::Index),
        )),
    }
}

fn binary_insertion_costs() -> Vec<Cost> {
    vec![
        stated(
            Measure::Comparisons,
            Case::Best,
            Formula::SumOver {
                from: 1,
                body: Box::new(Formula::FloorLog2(Box::new(Formula::Index))),
            },
            0,
            None,
        ),
        stated(
            Measure::Comparisons,
            Case::Worst,
            Formula::SumOver {
                from: 1,
                body: Box::new(Formula::CeilLog2(Box::new(Formula::Index))),
            },
            0,
            None,
        ),
        stated(
            Measure::Comparisons,
            Case::Average,
            binary_search_depths(),
            0,
            None,
        ),
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, None),
        stated(
            Measure::Writes,
            Case::Worst,
            Formula::Sum(vec![pairs_over(2), length_less(1)]),
            1,
            None,
        ),
        stated(
            Measure::Writes,
            Case::Average,
            Formula::Sum(vec![pairs_over(4), length_less_harmonic()]),
            0,
            Some(Source::VitterFlajolet1990Insertion),
        ),
    ]
}

fn selection_costs() -> Vec<Cost> {
    let source = Some(Source::VitterFlajolet1990Selection);
    vec![
        stated(Measure::Comparisons, Case::Best, pairs_over(2), 0, source),
        stated(Measure::Comparisons, Case::Worst, pairs_over(2), 0, source),
        stated(
            Measure::Comparisons,
            Case::Average,
            pairs_over(2),
            0,
            source,
        ),
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, None),
        stated(Measure::Writes, Case::Worst, twice(length_less(1)), 1, None),
        stated(
            Measure::Writes,
            Case::Average,
            twice(length_less_harmonic()),
            0,
            None,
        ),
    ]
}

fn bubble_costs(form: BubbleForm) -> Vec<Cost> {
    let source = Some(Source::VitterFlajolet1990Bubble);
    let exchanges = [
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, source),
        stated(
            Measure::Writes,
            Case::Worst,
            Formula::Product(vec![Formula::Length, length_less(1)]),
            0,
            source,
        ),
        stated(Measure::Writes, Case::Average, pairs_over(2), 0, source),
    ];
    let comparisons = match form {
        BubbleForm::Full => {
            let passes = Formula::Product(vec![length_less(1), length_less(1)]);
            vec![
                stated(Measure::Comparisons, Case::Best, passes.clone(), 1, None),
                stated(Measure::Comparisons, Case::Worst, passes.clone(), 1, None),
                stated(Measure::Comparisons, Case::Average, passes, 1, None),
            ]
        }
        BubbleForm::Shrinking => vec![
            stated(Measure::Comparisons, Case::Best, pairs_over(2), 0, None),
            stated(Measure::Comparisons, Case::Worst, pairs_over(2), 0, None),
            stated(Measure::Comparisons, Case::Average, pairs_over(2), 0, None),
        ],
        BubbleForm::EarlyExit | BubbleForm::LastExchange => vec![
            stated(Measure::Comparisons, Case::Best, length_less(1), 1, None),
            stated(Measure::Comparisons, Case::Worst, pairs_over(2), 0, None),
            not_in_closed_form(Measure::Comparisons, Case::Average),
        ],
    };
    comparisons.into_iter().chain(exchanges).collect()
}

fn ceil_log2_of_length() -> Formula {
    Formula::CeilLog2(Box::new(Formula::Length))
}

fn merged_entries() -> Formula {
    Formula::Difference(
        Box::new(Formula::Product(vec![
            Formula::Length,
            ceil_log2_of_length(),
        ])),
        Box::new(Formula::PowerOfTwo(Box::new(ceil_log2_of_length()))),
    )
}

fn merge_costs() -> Vec<Cost> {
    let source = Some(Source::FlajoletGolin1994);
    let writes = twice(Formula::Sum(vec![merged_entries(), Formula::Length]));
    vec![
        stated(
            Measure::Comparisons,
            Case::Best,
            Formula::SumOver {
                from: 1,
                body: Box::new(Formula::BitCount(Box::new(Formula::Difference(
                    Box::new(Formula::Index),
                    Box::new(Formula::Whole(1)),
                )))),
            },
            0,
            source,
        ),
        stated(
            Measure::Comparisons,
            Case::Worst,
            Formula::Sum(vec![merged_entries(), Formula::Whole(1)]),
            1,
            source,
        ),
        stated(
            Measure::Comparisons,
            Case::Average,
            Formula::MergeAverage,
            0,
            source,
        ),
        stated(Measure::Writes, Case::Best, writes.clone(), 1, None),
        stated(Measure::Writes, Case::Worst, writes.clone(), 1, None),
        stated(Measure::Writes, Case::Average, writes, 1, None),
    ]
}

fn heap_costs() -> Vec<Cost> {
    [Measure::Comparisons, Measure::Writes]
        .into_iter()
        .flat_map(|measure| {
            [Case::Best, Case::Worst, Case::Average]
                .into_iter()
                .map(move |case| not_in_closed_form(measure, case))
        })
        .collect()
}

fn lomuto_costs() -> Vec<Cost> {
    let depth = Formula::FloorLog2(Box::new(Formula::Length));
    let best = Formula::Sum(vec![
        Formula::Difference(
            Box::new(Formula::Product(vec![
                Formula::Sum(vec![Formula::Length, Formula::Whole(1)]),
                depth.clone(),
            ])),
            Box::new(Formula::PowerOfTwo(Box::new(Formula::Sum(vec![
                depth,
                Formula::Whole(1),
            ])))),
        ),
        Formula::Whole(2),
    ]);
    let average = Formula::Difference(
        Box::new(twice(Formula::Product(vec![
            Formula::Sum(vec![Formula::Length, Formula::Whole(1)]),
            Formula::Harmonic,
        ]))),
        Box::new(Formula::Product(vec![Formula::Whole(4), Formula::Length])),
    );
    vec![
        stated(Measure::Comparisons, Case::Best, best, 1, None),
        stated(Measure::Comparisons, Case::Worst, pairs_over(2), 0, None),
        stated(Measure::Comparisons, Case::Average, average, 0, None),
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, None),
        not_in_closed_form(Measure::Writes, Case::Worst),
        not_in_closed_form(Measure::Writes, Case::Average),
    ]
}

fn hoare_costs() -> Vec<Cost> {
    vec![
        not_in_closed_form(Measure::Comparisons, Case::Best),
        not_in_closed_form(Measure::Comparisons, Case::Worst),
        not_in_closed_form(Measure::Comparisons, Case::Average),
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, None),
        not_in_closed_form(Measure::Writes, Case::Worst),
        not_in_closed_form(Measure::Writes, Case::Average),
    ]
}

fn counting_costs() -> Vec<Cost> {
    let writes = twice(Formula::Length);
    vec![
        stated(Measure::Comparisons, Case::Best, Formula::Whole(0), 0, None),
        stated(
            Measure::Comparisons,
            Case::Worst,
            Formula::Whole(0),
            0,
            None,
        ),
        stated(
            Measure::Comparisons,
            Case::Average,
            Formula::Whole(0),
            0,
            None,
        ),
        stated(Measure::Writes, Case::Best, writes.clone(), 0, None),
        stated(Measure::Writes, Case::Worst, writes.clone(), 0, None),
        stated(Measure::Writes, Case::Average, writes, 0, None),
    ]
}

fn random_lomuto_costs() -> Vec<Cost> {
    lomuto_costs()
        .into_iter()
        .map(|cost| Cost {
            over: Over::OrdersAndPivotChoices,
            checked_up_to: PIVOT_CHOICE_CHECK_LIMIT,
            ..cost
        })
        .collect()
}

fn fitted(measure: Measure, case: Case, formula: Formula, holds_from: u64) -> Cost {
    Cost {
        provenance: Provenance::FittedAndChecked,
        ..stated(measure, case, formula, holds_from, None)
    }
}

fn floor_half() -> Formula {
    Formula::Floor(Box::new(Formula::Quotient(
        Box::new(Formula::Length),
        Box::new(Formula::Whole(2)),
    )))
}

fn inversion_writes() -> [Cost; 3] {
    [
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, None),
        stated(
            Measure::Writes,
            Case::Worst,
            Formula::Product(vec![Formula::Length, length_less(1)]),
            0,
            None,
        ),
        stated(Measure::Writes, Case::Average, pairs_over(2), 0, None),
    ]
}

fn double_selection_costs() -> Vec<Cost> {
    let half = floor_half();
    let every_case = Formula::Sum(vec![
        Formula::Quotient(
            Box::new(Formula::Product(vec![
                half.clone(),
                Formula::Difference(
                    Box::new(Formula::Product(vec![Formula::Whole(3), half.clone()])),
                    Box::new(Formula::Whole(1)),
                ),
            ])),
            Box::new(Formula::Whole(2)),
        ),
        Formula::Product(vec![
            Formula::Whole(2),
            half.clone(),
            Formula::Difference(
                Box::new(Formula::Length),
                Box::new(Formula::Product(vec![Formula::Whole(2), half])),
            ),
        ]),
    ]);
    vec![
        stated(
            Measure::Comparisons,
            Case::Best,
            every_case.clone(),
            0,
            None,
        ),
        stated(
            Measure::Comparisons,
            Case::Worst,
            every_case.clone(),
            0,
            None,
        ),
        stated(Measure::Comparisons, Case::Average, every_case, 0, None),
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, None),
        fitted(Measure::Writes, Case::Worst, twice(length_less(1)), 1),
        fitted(
            Measure::Writes,
            Case::Average,
            twice(length_less_harmonic()),
            0,
        ),
    ]
}

fn cocktail_shaker_costs(form: ShakerForm) -> Vec<Cost> {
    let worst = match form {
        ShakerForm::Full => Formula::Product(vec![Formula::Length, length_less(1)]),
        ShakerForm::Shrinking | ShakerForm::LastExchange => pairs_over(2),
    };
    let mut costs = vec![
        stated(Measure::Comparisons, Case::Best, length_less(1), 1, None),
        fitted(Measure::Comparisons, Case::Worst, worst, 0),
        not_in_closed_form(Measure::Comparisons, Case::Average),
    ];
    costs.extend(inversion_writes());
    costs
}

fn gnome_costs() -> Vec<Cost> {
    let average = Formula::Difference(
        Box::new(Formula::Sum(vec![pairs_over(2), Formula::Length])),
        Box::new(Formula::Harmonic),
    );
    let mut costs = vec![
        stated(Measure::Comparisons, Case::Best, length_less(1), 1, None),
        stated(
            Measure::Comparisons,
            Case::Worst,
            Formula::Product(vec![Formula::Length, length_less(1)]),
            0,
            None,
        ),
        stated(Measure::Comparisons, Case::Average, average, 0, None),
    ];
    costs.extend(inversion_writes());
    costs
}

fn odd_even_costs(form: OddEvenForm) -> Vec<Cost> {
    let mut costs = match form {
        OddEvenForm::FixedPasses => vec![
            stated(Measure::Comparisons, Case::Best, pairs_over(2), 0, None),
            stated(Measure::Comparisons, Case::Worst, pairs_over(2), 0, None),
            stated(Measure::Comparisons, Case::Average, pairs_over(2), 0, None),
        ],
        OddEvenForm::UntilSorted => vec![
            stated(Measure::Comparisons, Case::Best, length_less(1), 1, None),
            fitted(
                Measure::Comparisons,
                Case::Worst,
                Formula::Product(vec![
                    length_less(1),
                    Formula::Sum(vec![
                        Formula::Floor(Box::new(Formula::Quotient(
                            Box::new(Formula::Sum(vec![Formula::Length, Formula::Whole(1)])),
                            Box::new(Formula::Whole(2)),
                        ))),
                        Formula::Whole(1),
                    ]),
                ]),
                1,
            ),
            not_in_closed_form(Measure::Comparisons, Case::Average),
        ],
    };
    costs.extend(inversion_writes());
    costs
}

fn comb_costs() -> Vec<Cost> {
    vec![
        not_in_closed_form(Measure::Comparisons, Case::Best),
        not_in_closed_form(Measure::Comparisons, Case::Worst),
        not_in_closed_form(Measure::Comparisons, Case::Average),
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, None),
        not_in_closed_form(Measure::Writes, Case::Worst),
        not_in_closed_form(Measure::Writes, Case::Average),
    ]
}

fn cycle_costs() -> Vec<Cost> {
    let squared = Formula::Product(vec![Formula::Length, Formula::Length]);
    let worst = Formula::Sum(vec![
        Formula::Quotient(
            Box::new(Formula::Product(vec![
                Formula::Whole(3),
                Formula::Product(vec![Formula::Length, length_less(1)]),
            ])),
            Box::new(Formula::Whole(2)),
        ),
        Formula::Whole(1),
    ]);
    let average = Formula::Difference(
        Box::new(Formula::Sum(vec![
            Formula::Quotient(
                Box::new(Formula::Product(vec![Formula::Whole(3), squared])),
                Box::new(Formula::Whole(2)),
            ),
            Formula::Quotient(
                Box::new(Formula::Product(vec![Formula::Whole(5), Formula::Length])),
                Box::new(Formula::Whole(2)),
            ),
        ])),
        Box::new(Formula::Sum(vec![
            Formula::Whole(1),
            Formula::Harmonic,
            Formula::Product(vec![Formula::Whole(2), Formula::Length, Formula::Harmonic]),
        ])),
    );
    vec![
        stated(Measure::Comparisons, Case::Best, pairs_over(2), 0, None),
        fitted(Measure::Comparisons, Case::Worst, worst, 2),
        fitted(Measure::Comparisons, Case::Average, average, 1),
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, None),
        stated(Measure::Writes, Case::Worst, Formula::Length, 2, None),
        stated(Measure::Writes, Case::Average, length_less(1), 1, None),
    ]
}

fn pancake_costs() -> Vec<Cost> {
    let index_less_one = Formula::Difference(Box::new(Formula::Index), Box::new(Formula::Whole(1)));
    let part = Formula::Quotient(
        Box::new(Formula::Product(vec![
            Formula::Whole(2),
            Formula::Sum(vec![
                Formula::Product(vec![
                    Formula::Floor(Box::new(Formula::Quotient(
                        Box::new(Formula::Index),
                        Box::new(Formula::Whole(2)),
                    ))),
                    index_less_one.clone(),
                ]),
                Formula::Floor(Box::new(Formula::Quotient(
                    Box::new(Formula::Product(vec![
                        index_less_one.clone(),
                        index_less_one,
                    ])),
                    Box::new(Formula::Whole(4)),
                ))),
            ]),
        ])),
        Box::new(Formula::Index),
    );
    let flips_average = Formula::Difference(
        Box::new(Formula::Sum(vec![
            twice(Formula::Length),
            Formula::Whole(1),
        ])),
        Box::new(Formula::Product(vec![Formula::Whole(3), Formula::Harmonic])),
    );
    vec![
        stated(Measure::Comparisons, Case::Best, pairs_over(2), 0, None),
        stated(Measure::Comparisons, Case::Worst, pairs_over(2), 0, None),
        stated(Measure::Comparisons, Case::Average, pairs_over(2), 0, None),
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, None),
        fitted(
            Measure::Writes,
            Case::Worst,
            Formula::Product(vec![Formula::Length, length_less(1)]),
            0,
        ),
        stated(
            Measure::Writes,
            Case::Average,
            Formula::SumOver {
                from: 2,
                body: Box::new(part),
            },
            0,
            None,
        ),
        stated(Measure::Flips, Case::Best, Formula::Whole(0), 0, None),
        fitted(
            Measure::Flips,
            Case::Worst,
            Formula::Difference(
                Box::new(twice(Formula::Length)),
                Box::new(Formula::Whole(3)),
            ),
            2,
        ),
        stated(Measure::Flips, Case::Average, flips_average, 1, None),
    ]
}

fn shell_costs(gaps: Gaps) -> Vec<Cost> {
    vec![
        stated(
            Measure::Comparisons,
            Case::Best,
            Formula::GapSum(gaps),
            0,
            None,
        ),
        not_in_closed_form(Measure::Comparisons, Case::Worst),
        not_in_closed_form(Measure::Comparisons, Case::Average),
        stated(Measure::Writes, Case::Best, Formula::GapSum(gaps), 0, None),
        not_in_closed_form(Measure::Writes, Case::Worst),
        not_in_closed_form(Measure::Writes, Case::Average),
    ]
}

fn pass_width() -> Formula {
    Formula::PowerOfTwo(Box::new(Formula::Index))
}

fn full_merges() -> Formula {
    Formula::Floor(Box::new(Formula::Quotient(
        Box::new(Formula::Length),
        Box::new(twice(pass_width())),
    )))
}

fn left_over() -> Formula {
    Formula::Difference(
        Box::new(Formula::Length),
        Box::new(Formula::Product(vec![
            Formula::Whole(2),
            pass_width(),
            full_merges(),
        ])),
    )
}

fn partial_merge() -> Formula {
    Formula::Floor(Box::new(Formula::Quotient(
        Box::new(left_over()),
        Box::new(Formula::Sum(vec![pass_width(), Formula::Whole(1)])),
    )))
}

fn every_pass_copies() -> Formula {
    twice(Formula::Product(vec![
        Formula::Length,
        Formula::CeilLog2(Box::new(Formula::Length)),
    ]))
}

fn bottom_up_merge_costs() -> Vec<Cost> {
    let fewest = Formula::SumOver {
        from: 0,
        body: Box::new(Formula::Sum(vec![
            Formula::Product(vec![full_merges(), pass_width()]),
            Formula::Product(vec![
                partial_merge(),
                Formula::Difference(Box::new(left_over()), Box::new(pass_width())),
            ]),
        ])),
    };
    let most = Formula::SumOver {
        from: 0,
        body: Box::new(Formula::Sum(vec![
            Formula::Product(vec![
                full_merges(),
                Formula::Difference(Box::new(twice(pass_width())), Box::new(Formula::Whole(1))),
            ]),
            Formula::Product(vec![
                partial_merge(),
                Formula::Difference(Box::new(left_over()), Box::new(Formula::Whole(1))),
            ]),
        ])),
    };
    vec![
        stated(Measure::Comparisons, Case::Best, fewest, 0, None),
        stated(Measure::Comparisons, Case::Worst, most, 0, None),
        not_in_closed_form(Measure::Comparisons, Case::Average),
        stated(Measure::Writes, Case::Best, every_pass_copies(), 1, None),
        stated(Measure::Writes, Case::Worst, every_pass_copies(), 1, None),
        stated(Measure::Writes, Case::Average, every_pass_copies(), 1, None),
    ]
}

fn natural_merge_costs() -> Vec<Cost> {
    vec![
        stated(Measure::Comparisons, Case::Best, length_less(1), 1, None),
        not_in_closed_form(Measure::Comparisons, Case::Worst),
        not_in_closed_form(Measure::Comparisons, Case::Average),
        stated(Measure::Writes, Case::Best, Formula::Whole(0), 0, None),
        stated(Measure::Writes, Case::Worst, every_pass_copies(), 1, None),
        not_in_closed_form(Measure::Writes, Case::Average),
    ]
}

fn per_pass_of_the_keys(case: Case) -> Cost {
    Cost {
        provenance: Provenance::PerPassOfTheKeys,
        ..not_in_closed_form(Measure::Writes, case)
    }
}

fn radix_costs() -> Vec<Cost> {
    vec![
        stated(Measure::Comparisons, Case::Best, Formula::Whole(0), 0, None),
        stated(
            Measure::Comparisons,
            Case::Worst,
            Formula::Whole(0),
            0,
            None,
        ),
        stated(
            Measure::Comparisons,
            Case::Average,
            Formula::Whole(0),
            0,
            None,
        ),
        per_pass_of_the_keys(Case::Best),
        per_pass_of_the_keys(Case::Worst),
        per_pass_of_the_keys(Case::Average),
    ]
}

fn bead_costs() -> Vec<Cost> {
    [Case::Best, Case::Worst, Case::Average]
        .into_iter()
        .flat_map(|case| {
            [
                stated(Measure::Comparisons, case, Formula::Whole(0), 0, None),
                stated(Measure::Writes, case, Formula::Length, 0, None),
            ]
        })
        .collect()
}

fn bitonic_comparisons() -> Formula {
    let depth = || Formula::FloorLog2(Box::new(Formula::Length));
    Formula::Quotient(
        Box::new(Formula::Product(vec![
            Formula::Length,
            depth(),
            Formula::Sum(vec![depth(), Formula::Whole(1)]),
        ])),
        Box::new(Formula::Whole(4)),
    )
}

fn bitonic_costs() -> Vec<Cost> {
    let at_powers_of_two = |cost: Cost| Cost {
        over: Over::OrdersAtPowersOfTwo,
        ..cost
    };
    vec![
        at_powers_of_two(stated(
            Measure::Comparisons,
            Case::Best,
            bitonic_comparisons(),
            1,
            Some(Source::Batcher1968Bitonic),
        )),
        at_powers_of_two(stated(
            Measure::Comparisons,
            Case::Worst,
            bitonic_comparisons(),
            1,
            Some(Source::Batcher1968Bitonic),
        )),
        at_powers_of_two(stated(
            Measure::Comparisons,
            Case::Average,
            bitonic_comparisons(),
            1,
            Some(Source::Batcher1968Bitonic),
        )),
        at_powers_of_two(not_in_closed_form(Measure::Writes, Case::Best)),
        at_powers_of_two(not_in_closed_form(Measure::Writes, Case::Worst)),
        at_powers_of_two(fitted(
            Measure::Writes,
            Case::Average,
            bitonic_comparisons(),
            1,
        )),
    ]
}

fn bogo_costs() -> Vec<Cost> {
    let checked = |cost: Cost| Cost {
        checked_up_to: BOGO_CHECK_LIMIT,
        ..cost
    };
    let without = |measure: Measure, case: Case, provenance: Provenance| Cost {
        provenance,
        ..not_in_closed_form(measure, case)
    };
    vec![
        checked(stated(
            Measure::Comparisons,
            Case::Best,
            length_less(1),
            1,
            None,
        )),
        without(Measure::Comparisons, Case::Worst, Provenance::Unbounded),
        without(
            Measure::Comparisons,
            Case::Average,
            Provenance::NotInClosedForm,
        ),
        checked(stated(
            Measure::Writes,
            Case::Best,
            Formula::Whole(0),
            0,
            None,
        )),
        without(Measure::Writes, Case::Worst, Provenance::Unbounded),
        without(Measure::Writes, Case::Average, Provenance::PerShuffle),
    ]
}
