use crate::chemistry::AtomicWeightEntry;

pub(crate) const STANDARD_ATOMIC_WEIGHTS_2024: [(u8, AtomicWeightEntry); 84] = [
    (
        1,
        AtomicWeightEntry::Interval {
            low: 100784,
            high: 100811,
            places: 5,
        },
    ),
    (
        2,
        AtomicWeightEntry::Expanded {
            value: 4002602,
            uncertainty: 2,
            places: 6,
        },
    ),
    (
        3,
        AtomicWeightEntry::Interval {
            low: 6938,
            high: 6997,
            places: 3,
        },
    ),
    (
        4,
        AtomicWeightEntry::Expanded {
            value: 90121831,
            uncertainty: 5,
            places: 7,
        },
    ),
    (
        5,
        AtomicWeightEntry::Interval {
            low: 10806,
            high: 10821,
            places: 3,
        },
    ),
    (
        6,
        AtomicWeightEntry::Interval {
            low: 120096,
            high: 120116,
            places: 4,
        },
    ),
    (
        7,
        AtomicWeightEntry::Interval {
            low: 1400643,
            high: 1400728,
            places: 5,
        },
    ),
    (
        8,
        AtomicWeightEntry::Interval {
            low: 1599903,
            high: 1599977,
            places: 5,
        },
    ),
    (
        9,
        AtomicWeightEntry::Expanded {
            value: 18998403162,
            uncertainty: 5,
            places: 9,
        },
    ),
    (
        10,
        AtomicWeightEntry::Expanded {
            value: 201797,
            uncertainty: 6,
            places: 4,
        },
    ),
    (
        11,
        AtomicWeightEntry::Expanded {
            value: 2298976928,
            uncertainty: 2,
            places: 8,
        },
    ),
    (
        12,
        AtomicWeightEntry::Interval {
            low: 24304,
            high: 24307,
            places: 3,
        },
    ),
    (
        13,
        AtomicWeightEntry::Expanded {
            value: 269815384,
            uncertainty: 3,
            places: 7,
        },
    ),
    (
        14,
        AtomicWeightEntry::Interval {
            low: 28084,
            high: 28086,
            places: 3,
        },
    ),
    (
        15,
        AtomicWeightEntry::Expanded {
            value: 30973761998,
            uncertainty: 5,
            places: 9,
        },
    ),
    (
        16,
        AtomicWeightEntry::Interval {
            low: 32059,
            high: 32076,
            places: 3,
        },
    ),
    (
        17,
        AtomicWeightEntry::Interval {
            low: 35446,
            high: 35457,
            places: 3,
        },
    ),
    (
        18,
        AtomicWeightEntry::Interval {
            low: 39792,
            high: 39963,
            places: 3,
        },
    ),
    (
        19,
        AtomicWeightEntry::Expanded {
            value: 390983,
            uncertainty: 1,
            places: 4,
        },
    ),
    (
        20,
        AtomicWeightEntry::Expanded {
            value: 40078,
            uncertainty: 4,
            places: 3,
        },
    ),
    (
        21,
        AtomicWeightEntry::Expanded {
            value: 44955907,
            uncertainty: 4,
            places: 6,
        },
    ),
    (
        22,
        AtomicWeightEntry::Expanded {
            value: 47867,
            uncertainty: 1,
            places: 3,
        },
    ),
    (
        23,
        AtomicWeightEntry::Expanded {
            value: 509415,
            uncertainty: 1,
            places: 4,
        },
    ),
    (
        24,
        AtomicWeightEntry::Expanded {
            value: 519961,
            uncertainty: 6,
            places: 4,
        },
    ),
    (
        25,
        AtomicWeightEntry::Expanded {
            value: 54938043,
            uncertainty: 2,
            places: 6,
        },
    ),
    (
        26,
        AtomicWeightEntry::Expanded {
            value: 55845,
            uncertainty: 2,
            places: 3,
        },
    ),
    (
        27,
        AtomicWeightEntry::Expanded {
            value: 58933194,
            uncertainty: 3,
            places: 6,
        },
    ),
    (
        28,
        AtomicWeightEntry::Expanded {
            value: 586934,
            uncertainty: 4,
            places: 4,
        },
    ),
    (
        29,
        AtomicWeightEntry::Expanded {
            value: 63546,
            uncertainty: 3,
            places: 3,
        },
    ),
    (
        30,
        AtomicWeightEntry::Expanded {
            value: 6538,
            uncertainty: 2,
            places: 2,
        },
    ),
    (
        31,
        AtomicWeightEntry::Expanded {
            value: 69723,
            uncertainty: 1,
            places: 3,
        },
    ),
    (
        32,
        AtomicWeightEntry::Expanded {
            value: 72630,
            uncertainty: 8,
            places: 3,
        },
    ),
    (
        33,
        AtomicWeightEntry::Expanded {
            value: 74921595,
            uncertainty: 6,
            places: 6,
        },
    ),
    (
        34,
        AtomicWeightEntry::Expanded {
            value: 78971,
            uncertainty: 8,
            places: 3,
        },
    ),
    (
        35,
        AtomicWeightEntry::Interval {
            low: 79901,
            high: 79907,
            places: 3,
        },
    ),
    (
        36,
        AtomicWeightEntry::Expanded {
            value: 83798,
            uncertainty: 2,
            places: 3,
        },
    ),
    (
        37,
        AtomicWeightEntry::Expanded {
            value: 854678,
            uncertainty: 3,
            places: 4,
        },
    ),
    (
        38,
        AtomicWeightEntry::Expanded {
            value: 8762,
            uncertainty: 1,
            places: 2,
        },
    ),
    (
        39,
        AtomicWeightEntry::Expanded {
            value: 88905838,
            uncertainty: 2,
            places: 6,
        },
    ),
    (
        40,
        AtomicWeightEntry::Expanded {
            value: 91222,
            uncertainty: 3,
            places: 3,
        },
    ),
    (
        41,
        AtomicWeightEntry::Expanded {
            value: 9290637,
            uncertainty: 1,
            places: 5,
        },
    ),
    (
        42,
        AtomicWeightEntry::Expanded {
            value: 9595,
            uncertainty: 1,
            places: 2,
        },
    ),
    (
        44,
        AtomicWeightEntry::Expanded {
            value: 10107,
            uncertainty: 2,
            places: 2,
        },
    ),
    (
        45,
        AtomicWeightEntry::Expanded {
            value: 10290549,
            uncertainty: 2,
            places: 5,
        },
    ),
    (
        46,
        AtomicWeightEntry::Expanded {
            value: 10642,
            uncertainty: 1,
            places: 2,
        },
    ),
    (
        47,
        AtomicWeightEntry::Expanded {
            value: 1078682,
            uncertainty: 2,
            places: 4,
        },
    ),
    (
        48,
        AtomicWeightEntry::Expanded {
            value: 112414,
            uncertainty: 4,
            places: 3,
        },
    ),
    (
        49,
        AtomicWeightEntry::Expanded {
            value: 114818,
            uncertainty: 1,
            places: 3,
        },
    ),
    (
        50,
        AtomicWeightEntry::Expanded {
            value: 118710,
            uncertainty: 7,
            places: 3,
        },
    ),
    (
        51,
        AtomicWeightEntry::Expanded {
            value: 121760,
            uncertainty: 1,
            places: 3,
        },
    ),
    (
        52,
        AtomicWeightEntry::Expanded {
            value: 12760,
            uncertainty: 3,
            places: 2,
        },
    ),
    (
        53,
        AtomicWeightEntry::Expanded {
            value: 12690447,
            uncertainty: 3,
            places: 5,
        },
    ),
    (
        54,
        AtomicWeightEntry::Expanded {
            value: 131293,
            uncertainty: 6,
            places: 3,
        },
    ),
    (
        55,
        AtomicWeightEntry::Expanded {
            value: 13290545196,
            uncertainty: 6,
            places: 8,
        },
    ),
    (
        56,
        AtomicWeightEntry::Expanded {
            value: 137327,
            uncertainty: 7,
            places: 3,
        },
    ),
    (
        57,
        AtomicWeightEntry::Expanded {
            value: 13890547,
            uncertainty: 7,
            places: 5,
        },
    ),
    (
        58,
        AtomicWeightEntry::Expanded {
            value: 140116,
            uncertainty: 1,
            places: 3,
        },
    ),
    (
        59,
        AtomicWeightEntry::Expanded {
            value: 14090766,
            uncertainty: 1,
            places: 5,
        },
    ),
    (
        60,
        AtomicWeightEntry::Expanded {
            value: 144242,
            uncertainty: 3,
            places: 3,
        },
    ),
    (
        62,
        AtomicWeightEntry::Expanded {
            value: 15036,
            uncertainty: 2,
            places: 2,
        },
    ),
    (
        63,
        AtomicWeightEntry::Expanded {
            value: 151964,
            uncertainty: 1,
            places: 3,
        },
    ),
    (
        64,
        AtomicWeightEntry::Expanded {
            value: 157249,
            uncertainty: 2,
            places: 3,
        },
    ),
    (
        65,
        AtomicWeightEntry::Expanded {
            value: 158925354,
            uncertainty: 7,
            places: 6,
        },
    ),
    (
        66,
        AtomicWeightEntry::Expanded {
            value: 162500,
            uncertainty: 1,
            places: 3,
        },
    ),
    (
        67,
        AtomicWeightEntry::Expanded {
            value: 164930329,
            uncertainty: 5,
            places: 6,
        },
    ),
    (
        68,
        AtomicWeightEntry::Expanded {
            value: 167259,
            uncertainty: 3,
            places: 3,
        },
    ),
    (
        69,
        AtomicWeightEntry::Expanded {
            value: 168934219,
            uncertainty: 5,
            places: 6,
        },
    ),
    (
        70,
        AtomicWeightEntry::Expanded {
            value: 173045,
            uncertainty: 10,
            places: 3,
        },
    ),
    (
        71,
        AtomicWeightEntry::Expanded {
            value: 17496669,
            uncertainty: 5,
            places: 5,
        },
    ),
    (
        72,
        AtomicWeightEntry::Expanded {
            value: 178486,
            uncertainty: 6,
            places: 3,
        },
    ),
    (
        73,
        AtomicWeightEntry::Expanded {
            value: 18094788,
            uncertainty: 2,
            places: 5,
        },
    ),
    (
        74,
        AtomicWeightEntry::Expanded {
            value: 18384,
            uncertainty: 1,
            places: 2,
        },
    ),
    (
        75,
        AtomicWeightEntry::Expanded {
            value: 186207,
            uncertainty: 1,
            places: 3,
        },
    ),
    (
        76,
        AtomicWeightEntry::Expanded {
            value: 19023,
            uncertainty: 3,
            places: 2,
        },
    ),
    (
        77,
        AtomicWeightEntry::Expanded {
            value: 192217,
            uncertainty: 2,
            places: 3,
        },
    ),
    (
        78,
        AtomicWeightEntry::Expanded {
            value: 195084,
            uncertainty: 9,
            places: 3,
        },
    ),
    (
        79,
        AtomicWeightEntry::Expanded {
            value: 196966570,
            uncertainty: 4,
            places: 6,
        },
    ),
    (
        80,
        AtomicWeightEntry::Expanded {
            value: 200592,
            uncertainty: 3,
            places: 3,
        },
    ),
    (
        81,
        AtomicWeightEntry::Interval {
            low: 204382,
            high: 204385,
            places: 3,
        },
    ),
    (
        82,
        AtomicWeightEntry::Interval {
            low: 20614,
            high: 20794,
            places: 2,
        },
    ),
    (
        83,
        AtomicWeightEntry::Expanded {
            value: 20898040,
            uncertainty: 1,
            places: 5,
        },
    ),
    (
        90,
        AtomicWeightEntry::Expanded {
            value: 2320377,
            uncertainty: 4,
            places: 4,
        },
    ),
    (
        91,
        AtomicWeightEntry::Expanded {
            value: 23103588,
            uncertainty: 1,
            places: 5,
        },
    ),
    (
        92,
        AtomicWeightEntry::Expanded {
            value: 23802891,
            uncertainty: 3,
            places: 5,
        },
    ),
];
