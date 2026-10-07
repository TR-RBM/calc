common-kind-exact = exact
common-kind-symbolic = symbolic
common-kind-numeric = numeric
common-kind-sampled = sampled
common-kind-measured = measured
common-kind-derived = derived
common-kind-running = running
common-kind-differs = differs
common-kind-stale = stale
common-kind-failed = failed
common-kind-definition = definition

common-result-kind-exact-rational = exact rational
common-result-kind-exact-complex = exact complex number, rational parts
common-result-kind-machine-complex = complex number of two machine floats
common-result-kind-algebraic = exact, with roots (algebraic)
common-result-kind-symbolic = exact, as an expression (symbolic)
common-result-kind-machine-float = machine float

common-precision-exact = exact
common-precision-f32 = f32
common-precision-f64 = f64

common-backend-automatic = automatic
common-backend-cpu = CPU
common-backend-simd = SIMD
common-backend-gpu = GPU
common-backend-none = none

common-gpu-no-adapter = No graphics adapter is available, so this runs without the GPU backend.
common-gpu-exact-case-failed = The graphics adapter did not compute a whole-number check exactly, so this runs without the GPU backend.
common-gpu-exact-case-failed-operation = The graphics adapter did not compute { $operation } exactly on whole numbers, so this runs without the GPU backend.
common-gpu-case-not-run = A check could not be run on the graphics adapter, so this runs without the GPU backend.
common-gpu-case-not-run-operation = The check of { $operation } could not be run on the graphics adapter, so this runs without the GPU backend.

common-time-of-day = { $hour }:{ $minute }:{ $second }
common-date-and-time = { $year }-{ $month }-{ $day } { $time }
common-status-replay-not-run = replay: not run
common-status-replay-running = replay: running
common-status-replay-verified = replay: verified { $time }
common-status-replay-differs = replay: differs { $time }
common-status-results = { $count ->
    [one] { $count } result
   *[other] { $count } results
}
common-status-running = { $count ->
   *[other] { $count } running
}
common-status-differ = { $count ->
    [one] { $count } differs
   *[other] { $count } differ
}
common-status-locale = { $locale }
common-status-precision = precision: { $precision }
common-status-backend = backend: { $backend }

common-running-progress = { $percent }%  { $backend }  { $elapsed }

common-record-value = value
common-record-kind = number
common-record-machine-type = machine type
common-record-uncertainty = uncertainty
common-record-coverage-factor = coverage factor
common-record-uncertainty-budget = uncertainty budget
common-record-rounding-error = rounding error
common-record-rounding-unknown = unknown, rounded in machine arithmetic
common-record-rounding-measure-last-place = in units of the last place
common-record-rounding-measure-relative = relative
common-record-unit = unit
common-record-method = computed
common-record-parameters = parameters
common-record-convergence = convergence
common-record-converged = converged after { $iterations ->
    [one] { $iterations } iteration
   *[other] { $iterations } iterations
}
common-record-not-converged = not converged after { $iterations ->
    [one] { $iterations } iteration
   *[other] { $iterations } iterations
}
common-record-iteration-limit = iteration limit reached after { $iterations ->
    [one] { $iterations } iteration
   *[other] { $iterations } iterations
}
common-record-error-estimate = error estimate
common-record-condition = condition number
common-record-condition-absolute = absolute
common-record-condition-relative = relative
common-record-notes = notes
common-record-ran-on = ran on
common-record-run-time = run time
common-record-how-it-ran = how it ran
common-record-modes-all-native = every operation natively
common-record-modes-all-integer-exact = every operation integer-exact
common-record-modes-all-exact-in-both-forms = every operation exact in both forms
common-record-modes-native-count = { $count ->
    [one] { $count } natively
   *[other] { $count } natively
}
common-record-modes-integer-exact-count = { $count ->
    [one] { $count } integer-exact
   *[other] { $count } integer-exact
}
common-record-modes-exact-in-both-forms-count = { $count ->
    [one] { $count } exact in both forms
   *[other] { $count } exact in both forms
}
common-record-modes-by-operation = by operation
common-record-modes-native = natively
common-record-modes-integer-exact = integer-exact
common-record-modes-exact-in-both-forms = exact in both forms
common-record-modes-not-offered = not offered
common-record-modes-what-native-means = A native mode says the checks passed on this adapter at this start. It is not a claim that the operation is correct for every operand.
common-record-run-time-nanoseconds = { $time } ns, measured on this run
common-record-run-time-microseconds = { $time } µs, measured on this run
common-record-run-time-milliseconds = { $time } ms, measured on this run
common-record-run-time-seconds = { $time } s, measured on this run
common-record-computed-at = computed at
common-record-produced-by = computed by version
common-record-depends-on = depends on
common-record-used-by = used by
common-record-references = references
common-record-seed = seed
common-record-generator = generator
common-record-replay = replay
common-record-none = none

common-replay-verified = verified
common-replay-differs = differs
common-replay-not-compared = not compared
common-replay-failed-to-parse = input does not parse

common-kind-ways = ways
common-kind-not-reached = not reached

common-criterion-fewest-measurements = fewest measurements
common-criterion-fewest-steps = fewest steps
common-criterion-exactness = exactness
common-criterion-error-bound = error bound
common-criterion-gate-count = gate count
common-criterion-smallest-uncertainty = smallest uncertainty
common-criterion-conditioning = conditioning
common-criterion-rule-id = rule id
common-criterion-front = front

common-solve-wanted = wanted
common-solve-given = given
common-solve-criterion = criterion
common-solve-cap = cap
common-solve-tie-break = then
common-solve-shown = { $count } shown
common-solve-more-exist = more exist
common-solve-obtainable = { $count ->
    [one] { $count } quantity obtainable
   *[other] { $count } quantities obtainable
}
common-solve-no-bound = no bound
common-solve-rank = rank
common-solve-inputs = inputs
common-solve-rule = rule
common-solve-conditions = conditions
common-solve-quantity = quantity
common-solve-mark = mark
common-solve-value = value
common-solve-mark-given = given
common-solve-mark-yes = yes
common-solve-mark-no = no
common-solve-mark-unmarked = unmarked
common-solve-condition-holds = holds
common-solve-condition-fails = does not hold
common-solve-condition-undecided = undecided
common-solve-dominated = dominated
common-solve-further-input = one more input
common-solve-bound = bound
common-solve-other-ways = other ways
common-solve-obtainable-list = obtainable
common-solve-not-obtainable-list = not obtainable
common-solve-between = between
common-solve-score-exact = exact
common-solve-score-machine = machine
common-solve-score-zero = zero
common-solve-score-documented = documented

common-kind-reachable = reachable
common-solve-quantities = { $count ->
    [one] { $count } quantity
   *[other] { $count } quantities
}
common-solve-shown-of = { $shown } of { $total } shown
common-solve-nothing-follows = nothing follows from this given
common-solve-if-also-known = if you also know { $names }
common-solve-steps = steps
common-solve-derivation = derivation
common-solve-sources = sources
common-solve-object = object

common-method-worst-case = as the lowest and the highest value over every combination of the tolerances, each exact, at a corner calc proved
common-method-chemistry-composition = from the formula, element by element
common-method-chemistry-balance = as the smallest whole coefficients that conserve every element and the charge, from the exact kernel of the reaction
common-method-chemistry-check = by checking the written coefficients for every element and the charge
common-method-nuclear-nuclide = from the notation: mass number, atomic number and neutrons
common-method-nuclear-check = by checking the reaction as written, a missing coefficient counting as 1, for mass number, charge and electron lepton number
common-method-exact-evaluation = exactly, without rounding
common-method-plan-evaluation = in machine arithmetic
common-method-way-evaluation = along the chosen way
common-method-named = by method { $name }
common-record-order = order
common-record-form = form
common-record-partition = partition
common-record-draws = draws
common-record-flips = flips
common-record-fewest-flips = fewest flips
common-record-most-flips = most flips
common-record-average-flips = average flips
common-record-gaps = gaps
common-record-gaps-used = gaps used
common-record-passes = passes
common-record-shuffles = shuffles
common-record-expected-shuffles = expected shuffles
common-record-tallies = counter updates
common-record-key-range = key range
common-record-from-positions = from positions
common-record-comparisons = comparisons
common-record-writes = writes
common-record-key-evaluations = key
common-record-fewest-comparisons = fewest comparisons
common-record-most-comparisons = most comparisons
common-record-average-comparisons = average comparisons
common-record-fewest-writes = fewest writes
common-record-most-writes = most writes
common-record-average-writes = average writes
common-record-lower-bound = lower bound
common-record-described-in = described in
common-sort-increasing = by increasing key
common-sort-decreasing = by decreasing key
common-sort-positions = { $positions }, the place each entry of the result had in the input, counting from one
common-sort-counted = { $count }, counted on this input
common-sort-key-evaluations = evaluated { $count ->
    [one] once
   *[other] { $count } times
}, once for each entry, before the entries are ordered
common-sort-extreme = { $formula } = { $value } at n = { $length }, taken over every order of n distinct entries; { $provenance }
common-sort-extreme-powers-of-two = { $formula } = { $value } at n = { $length }, taken over every order of n distinct entries, for n a power of two; { $provenance }
common-sort-average = { $formula } = { $value } at n = { $length }, the mean over every order of n distinct entries, each equally likely; { $provenance }
common-sort-average-powers-of-two = { $formula } = { $value } at n = { $length }, the mean over every order of n distinct entries, each equally likely, for n a power of two; { $provenance }
common-sort-extreme-constant = { $value } at every n, taken over every order of n distinct entries; { $provenance }
common-sort-average-constant = { $value } at every n, the mean over every order of n distinct entries, each equally likely; { $provenance }
common-sort-extreme-over-choices = { $formula } = { $value } at n = { $length }, taken over every order of n distinct entries and every choice of pivots; { $provenance }
common-sort-expected-over-choices = { $formula } = { $value } at n = { $length }, the expected value when every pivot is drawn uniformly, the same for every order of n distinct entries; { $provenance }
common-sort-extreme-constant-over-choices = { $value } at every n, taken over every order of n distinct entries and every choice of pivots; { $provenance }
common-sort-expected-constant-over-choices = { $value } at every n, the expected value when every pivot is drawn uniformly, the same for every order of n distinct entries; { $provenance }
common-sort-cost-holds-from = { $formula }, which holds from n = { $from } on and so says nothing at n = { $length }; { $provenance }
common-sort-derived = derived here and checked against every order of up to { $limit } entries
common-sort-fitted = a closed form fitted to the counts and checked against every order of up to { $limit } entries, not derived for every n
common-sort-derived-from = derived from { $source } and checked against every order of up to { $limit } entries
common-sort-derived-over-choices = derived here and checked against every order of up to { $limit } entries with every sequence of pivot choices; this run's pivots came from the generator
common-sort-derived-from-over-choices = derived from { $source } and checked against every order of up to { $limit } entries with every sequence of pivot choices; this run's pivots came from the generator
common-sort-derived-expected-over-choices = derived here and checked against every order of up to { $limit } entries with every sequence of pivot choices, each weighted by its probability; this run's pivots came from the generator, not from that average
common-sort-derived-from-expected-over-choices = derived from { $source } and checked against every order of up to { $limit } entries with every sequence of pivot choices, each weighted by its probability; this run's pivots came from the generator, not from that average
common-sort-lower-bound = ceil(log2(n!)) = { $bound } at n = { $length }: no sort by comparisons makes fewer comparisons in its worst case on n distinct entries
common-sort-lower-bound-without-comparisons = ceil(log2(n!)) does not apply: this sort makes no comparisons, which is how it can go below it
common-sort-tallies = { $count }, counted on this input: one for each entry as it is counted, one for each prefix sum, and one for each entry as it is placed
common-sort-radix-tallies = { $count }, counted on this input: in each pass, one for each entry as it is counted, one for each of the base − 1 prefix sums, and one for each entry as it is placed
common-sort-bead-tallies = { $count }, counted on this input: one for each bead as it falls onto its pole, and one for each bead as its row is read, so twice the sum of the numbers
common-sort-no-positions = none: bead sort rebuilds each value from the beads in its row and moves no entry, so no entry of the result has a place it came from
common-sort-passes = { $passes ->
    [0] 0 in base { $base }: every key equals the least, so no digit is left to sort by
    [one] 1 in base { $base }, one for each digit of the largest key less the least
   *[other] { $passes } in base { $base }, one for each digit of the largest key less the least, the lowest digit first
}
common-sort-shuffles = { $count ->
    [one] 1 of at most { $limit }, with n − 1 exchanges
   *[other] { $count } of at most { $limit }, each with n − 1 exchanges
}
common-sort-expected-shuffles = { $factorial } = n! at n = { $length } from any order of n distinct keys but the sorted one, and 0 from the sorted one, since each shuffle sorts such a list with probability 1/n!; equal keys, m1, m2, … of each value, make it n!/(m1!·m2!·…); from Gruber, Holzer and Ruepp 2007, not checked here, since the draws have no bound
common-sort-expected-shuffles-none = 0: a list of no more than one entry is sorted and never shuffled
common-sort-unbounded = none: the shuffles have no bound, so neither has this count; limit= stops the sort
common-sort-per-shuffle = 2*(n − 1) for each shuffle, times the expected shuffles
common-sort-per-pass-of-the-keys = 2*n in each pass, the same for every order of the same keys; the number of passes depends on the keys and not on n alone, so no formula in n is stated
common-sort-key-range = { $range ->
    [one] 1 value, since the smallest key is also the largest, with one counter
   *[other] { $range } values from the smallest key to the largest, one counter for each
}
common-sort-flips = { $count ->
    [one] 1, counted on this input; a flip reverses the first k entries of the list, for some k
   *[other] { $count }, counted on this input; a flip reverses the first k entries of the list, for some k
}
common-sort-draws = { $count ->
    [one] 1 block
   *[other] { $count } blocks
} of philox4x32_10 at seed { $seed }, stream 0, from index 0 on, counted on this input: one for each part of two or more entries, and one more for each block rejected so that every place is equally likely
common-sort-shuffle-draws = { $count ->
    [one] 1 block
   *[other] { $count } blocks
} of philox4x32_10 at seed { $seed }, stream 1, from index 0 on, counted on this input: one for each position of each shuffle, and one more for each block rejected so that every place is equally likely
common-generator-layout = key: the seed { $seed } as two 32-bit words, low word first; counter: index + 2^64 · stream with index { $index } and stream { $stream }, as four 32-bit words, low word first; the same seed, stream and index give the same words on every machine
common-generator-statistical = a statistical generator: anyone who knows the seed, the stream and the index can compute its words, so it is not suitable for cryptography or for anything an adversary must not predict
common-sort-source-vitter-flajolet-1990 = J. S. Vitter and P. Flajolet, Average-Case Analysis of Algorithms and Data Structures, Handbook of Theoretical Computer Science, vol. A, 1990, section { $section }, DOI 10.1016/B978-0-444-88071-0.50014-X
common-sort-source-short-vitter-flajolet-1990 = Vitter and Flajolet 1990, section { $section }
common-sort-source-wikipedia-insertion-sort = Wikipedia, Insertion sort, section Variants (CC BY-SA 4.0)
common-sort-source-short-wikipedia-insertion-sort = Wikipedia, Insertion sort
common-sort-source-wikipedia-bubble-sort = Wikipedia, Bubble sort, sections Pseudocode implementation and Optimizing bubble sort (CC BY-SA 4.0)
common-sort-source-short-wikipedia-bubble-sort = Wikipedia, Bubble sort
common-sort-source-flajolet-golin-1994 = P. Flajolet and M. Golin, Mellin transforms and asymptotics: the mergesort recurrence, Acta Informatica 31 (1994), section 1, DOI 10.1007/BF01177551
common-sort-source-short-flajolet-golin-1994 = Flajolet and Golin 1994
common-sort-source-wikipedia-heapsort = Wikipedia, Heapsort, the standard implementation with a bottom-up heap construction (CC BY-SA 4.0)
common-sort-source-short-wikipedia-heapsort = Wikipedia, Heapsort
common-sort-source-wikipedia-quicksort-lomuto = Wikipedia, Quicksort, section Lomuto partition scheme (CC BY-SA 4.0)
common-sort-source-wikipedia-quicksort-hoare = Wikipedia, Quicksort, section Hoare partition scheme (CC BY-SA 4.0), after C. A. R. Hoare, Quicksort, The Computer Journal 5 (1962), DOI 10.1093/comjnl/5.1.10
common-sort-source-short-wikipedia-quicksort = Wikipedia, Quicksort
common-sort-source-wikipedia-counting-sort = Wikipedia, Counting sort, section Pseudocode, after Cormen, Leiserson, Rivest and Stein, Introduction to Algorithms, section 8.2 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-counting-sort = Wikipedia, Counting sort
common-sort-source-cormen-randomized-quicksort = T. H. Cormen, C. E. Leiserson, R. L. Rivest and C. Stein, Introduction to Algorithms, section 7.3, A randomized version of quicksort
common-sort-source-short-cormen-randomized-quicksort = Cormen, Leiserson, Rivest and Stein, section 7.3
common-sort-source-wikipedia-selection-sort-variants = Wikipedia, Selection sort, section Variants (CC BY-SA 4.0)
common-sort-source-short-wikipedia-selection-sort-variants = Wikipedia, Selection sort
common-sort-source-wikipedia-cocktail-shaker-sort = Wikipedia, Cocktail shaker sort, section Pseudocode, after D. E. Knuth, The Art of Computer Programming, vol. 3, 1973, pp. 110–111 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-cocktail-shaker-sort = Wikipedia, Cocktail shaker sort
common-sort-source-wikipedia-gnome-sort = Wikipedia, Gnome sort, section Pseudocode, after D. Grune, Gnome Sort, 2000 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-gnome-sort = Wikipedia, Gnome sort
common-sort-source-wikipedia-odd-even-sort = Wikipedia, Odd–even sort, section Algorithm, after N. Habermann, Parallel Neighbor Sort, 1972 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-odd-even-sort = Wikipedia, Odd–even sort
common-sort-source-wikipedia-comb-sort = Wikipedia, Comb sort, section Pseudocode, after S. Lacey and R. Box, A Fast, Easy Sort, Byte 16(4), 1991 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-comb-sort = Wikipedia, Comb sort
common-sort-source-wikipedia-cycle-sort = Wikipedia, Cycle sort, section Implementation, after B. K. Haddon, Cycle-Sort: A Linear Sorting Method, The Computer Journal 33(4), 1990, DOI 10.1093/comjnl/33.4.365 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-cycle-sort = Wikipedia, Cycle sort
common-sort-source-wikipedia-pancake-sorting = Wikipedia, Pancake sorting, section Algorithm, after W. H. Gates and C. H. Papadimitriou, Bounds for Sorting by Prefix Reversal, Discrete Mathematics 27, 1979, DOI 10.1016/0012-365X(79)90068-2 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-pancake-sorting = Wikipedia, Pancake sorting
common-sort-source-wikipedia-shellsort = Wikipedia, Shellsort, sections Example and Gap sequences, after D. L. Shell, A high-speed sorting procedure, Communications of the ACM 2(7), 1959, D. E. Knuth, The Art of Computer Programming, vol. 3, 1973, and M. Ciura, Best Increments for the Average Case of Shellsort, 2001 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-shellsort = Wikipedia, Shellsort
common-sort-source-wikipedia-merge-sort-bottom-up = Wikipedia, Merge sort, section Bottom-up implementation, after D. E. Knuth, The Art of Computer Programming, vol. 3, section 5.2.4 (CC BY-SA 4.0)
common-sort-source-wikipedia-merge-sort-natural = Wikipedia, Merge sort, section Natural merge sort, after D. E. Knuth, The Art of Computer Programming, vol. 3, section 5.2.4 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-merge-sort = Wikipedia, Merge sort
common-sort-source-wikipedia-radix-sort = Wikipedia, Radix sort, section Least significant digit, after D. E. Knuth, The Art of Computer Programming, vol. 3, section 5.2.5 (CC BY-SA 4.0)
common-sort-source-short-wikipedia-radix-sort = Wikipedia, Radix sort
common-sort-source-arulanandham-calude-dinneen-2002 = J. J. Arulanandham, C. S. Calude, M. J. Dinneen, Bead-Sort: A Natural Sorting Algorithm, Bulletin of the EATCS 76 (2002), and Wikipedia, Bead sort (CC BY-SA 4.0)
common-sort-source-short-arulanandham-calude-dinneen-2002 = Arulanandham, Calude and Dinneen 2002
common-sort-source-batcher-1968-bitonic = K. E. Batcher, Sorting networks and their applications, Proceedings of the AFIPS Spring Joint Computer Conference 1968, 307–314, and Wikipedia, Bitonic sorter (CC BY-SA 4.0)
common-sort-source-short-batcher-1968-bitonic = Batcher 1968
common-sort-source-gruber-holzer-ruepp-2007 = H. Gruber, M. Holzer, O. Ruepp, Sorting the Slow Way: An Analysis of Perversely Awful Randomized Sorting Algorithms, Fun with Algorithms 2007, LNCS 4475, 183–197
common-sort-source-short-gruber-holzer-ruepp-2007 = Gruber, Holzer and Ruepp 2007
common-sort-form-full = full: n - 1 passes, each over the whole list
common-sort-form-shrinking = shrinking: n - 1 passes, each ending before the entries the earlier passes placed
common-sort-form-early-exit = early_exit: shrinking passes, stopping after a pass without an exchange
common-sort-form-last-exchange = last_exchange: each pass ends where the one before made its last exchange (Knuth's Algorithm B)
common-sort-shaker-form-full = full: every scan covers the whole list, and the sort stops after a scan without an exchange
common-sort-shaker-form-shrinking = shrinking: each scan ends one entry before the scan on its side ended, and the sort stops after a scan without an exchange
common-sort-shaker-form-last-exchange = last_exchange: each scan ends where the scan before it in the same direction made its last exchange
common-sort-odd-even-form-until-sorted = until_sorted: rounds of the odd pairs, then the even pairs, until a round makes no exchange
common-sort-odd-even-form-fixed-passes = fixed_passes: exactly n phases, alternating odd and even pairs, which always suffice
common-sort-comb-form-lacey-box = lacey_box: the gap starts at n and becomes floor(10*gap/13) before each pass, 9 and 10 become 11, and passes with gap 1 repeat until one makes no exchange (Lacey and Box 1991)
common-sort-gaps-shell = shell: the gaps n/2, n/4, … rounded down, down to 1 (Shell 1959)
common-sort-gaps-knuth = knuth: the gaps 1, 4, 13, 40, … = (3^k − 1)/2 that are not greater than ceil(n/3) and smaller than n, the largest first (Knuth 1973, after Pratt 1971)
common-sort-gaps-ciura = ciura: the gaps 701, 301, 132, 57, 23, 10, 4, 1 that are smaller than n, as Ciura found them in 2001; the sequence is not extended beyond 701
common-sort-gaps-used = { $gaps } at n = { $length }, the gaps the rule above gives at this n, the largest first
common-sort-gaps-used-none = none at n = { $length }: the rule above gives no gap at this n, so no pass was made
common-sort-partition-lomuto-last = lomuto with pivot=last: the last entry is the pivot, one scan from the left moves every entry not after it to the front, then the pivot takes its place
common-sort-partition-hoare-first = hoare with pivot=first: the first entry is the pivot, two scans move towards each other and exchange the pairs that stand on the wrong sides
common-sort-partition-lomuto-random = lomuto with pivot=random: for each part, philox4x32_10 draws one of its positions, each equally likely, that entry is exchanged into the last position and is the pivot, then the partition runs as with pivot=last
common-sort-not-in-closed-form = not stated: calc knows no closed form for it that it evaluates
common-method-insertion-sort = by insertion sort, which is stable: entries with equal keys keep their input order; the held entry is written back even where it stays
common-method-binary-insertion-sort = by binary insertion sort, which is stable: entries with equal keys keep their input order; an entry is written only where it moves
common-method-selection-sort = by selection sort, which is not stable: entries with equal keys may leave their input order
common-method-bubble-sort = by bubble sort, which is stable: entries with equal keys keep their input order
common-method-merge-sort = by top-down merge sort, which is stable: entries with equal keys keep their input order; each merge writes into scratch storage and copies back
common-method-heap-sort = by heap sort, which is not stable: entries with equal keys may leave their input order
common-method-quick-sort = by quicksort, which is not stable: entries with equal keys may leave their input order
common-method-counting-sort = by counting sort, which is stable: entries with equal keys keep their input order; it makes no comparisons, it counts each key, sums the counts, places every entry into an output list from the back and copies it back
common-method-double-selection-sort = by double selection sort, which is not stable: entries with equal keys may leave their input order; each pass compares the rest in pairs to find its least and its greatest entry, and puts them at its two ends
common-method-cocktail-shaker-sort = by cocktail shaker sort, which is stable: entries with equal keys keep their input order; it scans forward and back in turn, exchanging each pair of neighbours out of order
common-method-gnome-sort = by gnome sort, which is stable: entries with equal keys keep their input order; it steps forward while two neighbours are in order, and where they are not it exchanges them and steps back
common-method-odd-even-sort = by odd–even sort, which is stable: entries with equal keys keep their input order; it compares the neighbours at odd positions, then those at even positions, exchanging each pair out of order
common-method-comb-sort = by comb sort, which is not stable: entries with equal keys may leave their input order; it compares entries a gap apart and exchanges them if out of order, shrinking the gap from pass to pass
common-method-cycle-sort = by cycle sort, which is not stable: entries with equal keys may leave their input order; it counts the entries smaller than an entry to find its place and moves it there, so each entry is written at most once
common-method-pancake-sort = by pancake sort, which is not stable: entries with equal keys may leave their input order; it only reverses beginnings of the list, flipping the greatest unsorted entry to the front and then into its place
common-method-shell-sort = by Shell sort, which is not stable: entries with equal keys may leave their input order; it makes an insertion sort of the entries a gap apart for each gap of the sequence, the largest first and 1 last
common-method-bottom-up-merge-sort = by bottom-up merge sort, which is stable: entries with equal keys keep their input order; it merges runs of 1, then 2, then 4 entries and so on, each pass over the whole list, copying every entry through a scratch list and back
common-method-natural-merge-sort = by natural merge sort, which is stable: entries with equal keys keep their input order; it first finds the runs that are already in order, then merges neighbouring runs in pairs, pass after pass, copying every entry through a scratch list and back
common-method-radix-sort = by radix sort, which is stable: entries with equal keys keep their input order; it sorts by the lowest digit of each key less the least key, in the base named by base=, then by the next digit, and so on, each pass a counting sort that makes no comparisons
common-method-bead-sort = by bead sort: each number is a row of that many beads, one on each pole from the first; the beads fall, so each pole keeps one bead for every number that reaches it, and each row is read back as the number of its beads; it makes no comparisons, and it rebuilds the values from the beads rather than moving the entries
common-method-bitonic-sort = by bitonic sort, which is not stable: a network that sorts blocks of 2, 4, 8, … entries, each block made of one rising and one falling half and merged by comparing entries half a block apart, then a quarter, and so on; it makes the same comparisons whatever the entries are
common-method-bogo-sort = by bogo sort, which is not stable: it tests whether the list is sorted, comparing neighbours from the left up to the first pair out of order, and while it is not, shuffles the whole list by Fisher–Yates with positions drawn from philox4x32_10; it ends with probability 1 but has no bound, so limit= caps the shuffles
common-method-philox4x32-10 = by Philox4x32-10, the counter-based random number generator of J. K. Salmon, M. A. Moraes, R. O. Dror and D. E. Shaw, Parallel random numbers: as easy as 1, 2, 3, SC11, 2011, DOI 10.1145/2063384.2063405; the result is its block of four words, each a whole number from 0 to 2^32 − 1
common-result-kind-machine-float-64 = machine float, 64-bit (f64)
common-result-kind-machine-float-32 = machine float, 32-bit (f32)
common-record-propagation = propagation
common-record-propagated-first-order = first order from { $count ->
    [one] { $count } uncorrelated input
   *[other] { $count } uncorrelated inputs
}
common-solve-line-ways = { $count ->
    [one] { $count } way found
   *[other] { $count } ways found
}
common-solve-line-solved = { $count ->
    [one] solved along { $count } way
   *[other] solved along { $count } ways
}
common-solve-line-not-reached = { $count ->
    [0] the rules lead to it from no further input
    [one] { $count } further input would reach it
   *[other] { $count } further input sets would reach it
}
common-solve-line-reachable = { $count ->
    [0] the given leads to no further quantity
    [one] { $count } quantity follows from the given
   *[other] { $count } quantities follow from the given
}
common-result-kind-exact-integer = exact integer
common-result-kind-exact-decimal = exact decimal
common-result-kind-exact-fraction = exact fraction
common-result-kind-proven-range = a range holding every value its inputs' tolerances allow; both ends are exact
common-record-rounding-at-most = at most { $bound }, absolute
common-result-kind-derived-from-measured = { $count ->
    [one] derived from { $count } measured input
   *[other] derived from { $count } measured inputs
}
common-record-input = input
common-record-digits = digits
common-value-digit-count = { $count ->
    [one] { $count } digit
   *[other] { $count } digits
}
common-value-fraction-digit-counts = { $numerator } and { $denominator } digits
common-solve-bound-at-most = at most { $expression }, from { $inputs }
common-solve-bound-at-least = at least { $expression }, from { $inputs }
common-solve-bound-strictly-between = above { $lower } and below { $upper }, from { $inputs }
common-solve-bound-from-inputs = from { $inputs }

common-unit-override-system-not-applied = the unit system { $system } was not found, so it is not applied
common-unit-override-kind-not-applied = the quantity kind { $kind } was not found, so its unit is not applied
common-unit-override-unit-not-applied = the unit { $unit } for { $kind } was not found, so it is not applied
common-kind-unit-not-a-unit = { $unit } is not a unit
common-kind-unit-other-dimension = { $unit } is not a unit of { $kind }
common-kind-unit-compound-not-allowed = { $kind } is shown in one unit, not in several
common-kind-unit-compound-not-descending = the units of { $unit } must go from the largest to the smallest
common-kind-unit-scale-required = { $kind } is shown on a scale: kelvin, celsius or fahrenheit
common-kind-unit-not-a-display-unit = { $unit } cannot be chosen as the unit in which { $kind } is shown

common-quantity-kind-length = length
common-quantity-kind-area = area
common-quantity-kind-volume = volume
common-quantity-kind-angle = angle
common-quantity-kind-count = count
common-quantity-kind-ratio = ratio
common-quantity-kind-mass = mass
common-quantity-kind-time = time
common-quantity-kind-speed = speed
common-quantity-kind-force = force
common-quantity-kind-energy = energy
common-quantity-kind-temperature = temperature
common-quantity-kind-temperature-difference = temperature difference
common-view-decimal-places = decimal places
common-view-digits-of = digits of
common-view-encloses = encloses
common-view-of-exact-value = the exact value
common-view-of-machine-value = the machine value
common-view-encloses-exact-value = the exact value
common-view-encloses-machine-value = the machine value
common-view-truncated = truncated
common-view-remainder = remainder
common-view-remainder-zero = 0, the value ends within these places
common-view-uncertainty = uncertainty
common-view-uncertainty-below-place = smaller than the last place shown
common-view-uncertainty-not-a-number = the uncertainty of this value is not a single number
common-view-expansion = expansion
common-view-expansion-finite = finite
common-view-expansion-period = recurring from place { $start }, period length { $length }
common-view-expansion-period-beyond-limit = recurring, with its period ending beyond place { $limit }
common-view-expansion-not-known-to-recur = not known to recur; the digits come from a proven enclosure
common-view-significant-digits = significant digits
common-view-lower = lower bound
common-view-upper = upper bound
common-view-width = width
common-view-precision = precision
common-view-reached = reached
common-view-not-reached = stopped at the precision limit, interval as proven
common-unit-source-written = written
common-unit-source-this-session = this session
common-unit-source-preference = your preference
common-unit-source-curriculum = curriculum { $name }
common-unit-source-stored-unit = stored unit
common-unit-source-as-computed = as computed
common-record-stored-as = stored as
common-picture-missing = missing
common-picture-unresolved = unresolved
common-picture-may-be-hit = may be hit
common-picture-marked = precision exhausted
common-picture-marked-columns = precision exhausted: { $marked } of { $columns } columns not resolved at this precision
common-picture-width-columns = may be hit: { $wide } of { $columns } columns not resolved at this width
common-picture-width-meaning = may be hit: not resolved at this width
common-picture-below-bounds = the sampled values vary less than their rounding bounds
common-picture-back-face = back face
common-picture-provisional = still computing
common-picture-grid-limit = positions exhausted at this scale
common-picture-value-limit = precision exhausted at this scale
common-picture-unknown-bounds = bounds unknown
common-picture-inside = inside
common-picture-undecided = undecided
common-picture-argument = argument
common-picture-modulus = modulus
common-picture-sketch = sketch, not to scale
common-picture-unit-joiner = in
common-picture-angle-arc = angle
common-picture-right-angle = right angle
common-picture-equal-sides = equal sides
common-picture-direction = direction
common-picture-hypotenuse = hypotenuse
common-picture-leg = leg
common-picture-height = height
common-picture-real-part = real part of { $name }
common-picture-imaginary-part = imaginary part of { $name }

# The language reference
common-reference-group-numbers = Numbers
common-reference-group-uncertainty = Uncertainty
common-reference-group-units = Units
common-reference-group-operators = Operators
common-reference-group-relations = Relations and logic
common-reference-group-calls = Calls
common-reference-group-keyword-arguments = Keyword arguments
common-reference-group-arrays = Arrays
common-reference-group-binders = Binders
common-reference-group-statements = Statements

common-reference-constant-pi = pi
common-reference-constant-pi-meaning = the ratio of a circle's circumference to its diameter
common-reference-constant-e = e
common-reference-constant-e-meaning = Euler's number, the base of the natural logarithm
common-reference-constant-imaginary-unit = imaginary unit
common-reference-constant-imaginary-unit-meaning = the number whose square is minus one
common-reference-constant-infinity = infinity
common-reference-constant-infinity-meaning = a value above every number, and below every number with a minus sign
common-reference-typed-literal-rational = exact fraction
common-reference-typed-literal-rational-meaning = a fraction that stays exact, numerator over denominator
common-reference-typed-literal-f32 = 32-bit machine number
common-reference-typed-literal-f32-meaning = a number stored as a 32-bit binary floating-point number
common-reference-typed-literal-f64 = 64-bit machine number
common-reference-typed-literal-f64-meaning = a number stored as a 64-bit binary floating-point number
common-reference-literal-decimal = decimal number
common-reference-literal-decimal-meaning = a number written with a decimal point, which stays exact
common-reference-literal-exponent = power-of-ten form
common-reference-literal-exponent-meaning = a number followed by e and the power of ten it is multiplied by
common-reference-literal-line-label = line label
common-reference-literal-line-label-meaning = the result of an earlier line, r and the line's number
common-reference-operator-uncertain = measured value
common-reference-operator-uncertain-meaning = a value with its standard uncertainty
common-reference-operator-uncertain-expanded = measured value with a coverage factor
common-reference-operator-uncertain-expanded-meaning = a value with an expanded uncertainty and the factor k it was multiplied by
common-reference-operator-convert-unit = unit conversion
common-reference-operator-convert-unit-meaning = the same quantity written in another unit
common-reference-form-quantity = quantity
common-reference-form-quantity-meaning = a number, or a constant such as pi, with a unit after it
common-reference-form-unit-expression = unit expression
common-reference-form-unit-expression-meaning = units multiplied, divided and raised to powers
common-reference-form-degree-sign = degree
common-reference-form-degree-sign-meaning = an angle in degrees, written deg or with the degree sign
common-reference-operator-add = addition
common-reference-operator-add-meaning = the sum of two values
common-reference-operator-sub = subtraction
common-reference-operator-sub-meaning = the difference of two values
common-reference-operator-mul = multiplication
common-reference-operator-mul-meaning = the product of two values
common-reference-operator-div = division
common-reference-operator-div-meaning = the quotient of two values
common-reference-operator-neg = negation
common-reference-operator-neg-meaning = the value with the opposite sign
common-reference-operator-pow = power
common-reference-operator-pow-meaning = the base raised to the exponent
common-reference-operator-factorial = factorial
common-reference-operator-factorial-meaning = the product of the whole numbers from 1 to n
common-reference-operator-percent = percent
common-reference-operator-percent-meaning = the number divided by one hundred
common-reference-operator-round = round
common-reference-operator-round-meaning = the nearest whole number, a half going away from zero
common-reference-operator-binomial = binomial coefficient
common-reference-operator-binomial-meaning = how many ways k things can be chosen from n
common-reference-operator-sinh = hyperbolic sine
common-reference-operator-sinh-meaning = half the difference of e to the x and e to the minus x
common-reference-operator-cosh = hyperbolic cosine
common-reference-operator-cosh-meaning = half the sum of e to the x and e to the minus x
common-reference-operator-tanh = hyperbolic tangent
common-reference-operator-tanh-meaning = the hyperbolic sine divided by the hyperbolic cosine
common-reference-operator-log = logarithm to a base
common-reference-operator-log-meaning = the power to which the base must be raised to give x
common-reference-operator-log2 = binary logarithm
common-reference-operator-log2-meaning = the power to which 2 must be raised to give x
common-reference-operator-log10 = common logarithm
common-reference-operator-log10-meaning = the power to which 10 must be raised to give x
common-reference-operator-gcd = greatest common divisor
common-reference-operator-gcd-meaning = the largest whole number that divides both
common-reference-operator-count = count
common-reference-operator-count-meaning = how many entries a list or matrix holds
common-reference-operator-total = total
common-reference-operator-total-meaning = the sum of every entry, exactly
common-reference-operator-mean = arithmetic mean
common-reference-operator-mean-meaning = the sum of the entries divided by how many there are
common-reference-operator-median = median
common-reference-operator-median-meaning = the middle entry once they are sorted, or the mean of the middle two
common-reference-operator-transpose = transpose
common-reference-operator-transpose-meaning = the matrix with its rows and columns exchanged
common-reference-operator-determinant = determinant
common-reference-operator-determinant-meaning = the number that says by how much a square matrix scales a volume
common-reference-operator-trace = trace
common-reference-operator-trace-meaning = the sum of the entries on the diagonal of a square matrix
common-reference-operator-inverse = inverse
common-reference-operator-inverse-meaning = the matrix that multiplied by a square matrix gives the identity
common-reference-operator-rank = rank
common-reference-operator-rank-meaning = the number of linearly independent rows of a matrix
common-reference-operator-eigenvalues = eigenvalues
common-reference-operator-eigenvalues-meaning = the real numbers t for which the matrix minus t times the identity is not invertible
common-reference-operator-kernel = kernel
common-reference-operator-kernel-meaning = a basis of the vectors a matrix sends to zero, one column per basis vector
common-reference-operator-eigenvectors = eigenvectors
common-reference-operator-eigenvectors-meaning = a basis of every eigenspace, as columns in the order of the eigenvalues from the smallest
common-reference-operator-at = entry
common-reference-operator-at-meaning = the entry of a list at that position, counting from one
common-reference-operator-bit-and = bitwise and
common-reference-operator-bit-and-meaning = the whole number whose bits are set where both whole numbers from 0 have them set
common-reference-operator-bit-or = bitwise or
common-reference-operator-bit-or-meaning = the whole number whose bits are set where either whole number from 0 has them set
common-reference-operator-bit-xor = bitwise exclusive or
common-reference-operator-bit-xor-meaning = the whole number whose bits are set where exactly one of the two whole numbers from 0 has them set; ^ stays the power
common-reference-operator-shift-left = shift left
common-reference-operator-shift-left-meaning = x times 2 to the power n, exactly
common-reference-operator-shift-right = shift right
common-reference-operator-shift-right-meaning = x divided by 2 to the power n, rounded down, for x from 0
common-reference-operator-bit-not = bitwise not in a type
common-reference-operator-bit-not-meaning = every bit of x flipped within the type it lies in, such as u8 or i16
common-reference-operator-wrap = wrap into a type
common-reference-operator-wrap-meaning = x reduced into the range of the type by whole multiples of 2 to the power of its width, as a register of that width would hold it
common-reference-operator-in-type = value in a type
common-reference-operator-in-type-meaning = x shown as a value of the type, refused where it lies outside the type's range
common-reference-operator-radix = in a base
common-reference-operator-radix-meaning = a whole number written in base 16, 2, 8 or 10; with a type, a negative number is written as its two's complement at the type's width
common-reference-operator-bytes = as bytes
common-reference-operator-bytes-meaning = a whole number written as its bytes in the order named, be for the most significant first, le for the least
common-reference-operator-between = value in a range
common-reference-operator-between-meaning = a value somewhere between the two ends, both included; the line then answers its lowest and highest value
common-reference-operator-tolerance = value with a tolerance
common-reference-operator-tolerance-meaning = x with a tolerance of the share p either way, as a data sheet writes 25 Ω ± 10 %; unlike ±, which states a measurement uncertainty, it states hard limits
common-reference-operator-substance = chemical substance
common-reference-operator-substance-meaning = a substance written by its formula, as chem'H2O'; standing alone it answers the composition calc read
common-reference-operator-reaction = chemical reaction
common-reference-operator-reaction-meaning = a reaction written as chem'reactants -> products'; without coefficients it is balanced exactly, and with any coefficient written it is checked, a missing one counting as 1
common-reference-operator-molar-mass = molar mass
common-reference-operator-molar-mass-meaning = the molar mass of a substance as a range in g/mol, from the standard atomic weights CIAAW 2024 and the molar mass constant CODATA 2022
common-reference-typed-literal-chemistry = chemical formula
common-reference-typed-literal-chemistry-meaning = a chemical formula or reaction between chem' and ', read by its own grammar and never as units or names
common-reference-operator-nuclide = nuclide
common-reference-operator-nuclide-meaning = a nucleus or a particle written as nuc'^14C'; standing alone it answers what calc read
common-reference-operator-nuclear-reaction = nuclear reaction
common-reference-operator-nuclear-reaction-meaning = a nuclear reaction written as nuc'reactants -> products'; it is checked as written, a missing coefficient counting as 1, and balanced exactly where that fails and one balance exists
common-reference-operator-q-value = Q value
common-reference-operator-q-value-meaning = the energy a nuclear reaction releases, as a range in keV, from the atomic masses AME2020 and the constants CODATA 2022
common-reference-operator-philox4x32-10 = Philox4x32-10
common-reference-operator-philox4x32-10-meaning = the four 32-bit words of the counter-based random number generator Philox4x32-10 at a seed, a stream and an index, each from 0 to 2^64 − 1; not for cryptography
common-reference-operator-rational-part = rational part
common-reference-operator-rational-part-meaning = the rational part of an exact value built from rationals and square roots: 17 in 17 + 12 * sqrt(2)
common-reference-operator-coefficient-of = coefficient of a term
common-reference-operator-coefficient-of-meaning = the coefficient of a term in calc's normal form of a value; the term is sqrt(n) with n free of square factors: 12 for sqrt(2) in 17 + 12 * sqrt(2)
common-reference-operator-re = real part
common-reference-operator-re-meaning = the real part of a complex number: 3 in 3 + 4 * i
common-reference-operator-im = imaginary part
common-reference-operator-im-meaning = the imaginary part of a complex number, a real number: 4 in 3 + 4 * i
common-reference-operator-conj = complex conjugate
common-reference-operator-conj-meaning = the complex conjugate, the same number with its imaginary part negated: 3 - 4 * i for 3 + 4 * i
common-reference-typed-literal-nuclear = nuclear notation
common-reference-typed-literal-nuclear-meaning = a nuclide or a nuclear reaction between nuc' and ', read by its own grammar and never as units or names
common-reference-operator-lcm = least common multiple
common-reference-operator-lcm-meaning = the smallest positive whole number both divide
common-reference-operator-mod = remainder
common-reference-operator-mod-meaning = what is left of a after taking away whole multiples of b, never negative
common-reference-operator-equal = equality
common-reference-operator-equal-meaning = holds when both sides are the same value
common-reference-operator-not-equal = inequality
common-reference-operator-not-equal-meaning = holds when the two sides differ
common-reference-operator-less = less than
common-reference-operator-less-meaning = holds when the left side is below the right side
common-reference-operator-less-or-equal = less than or equal
common-reference-operator-less-or-equal-meaning = holds when the left side is not above the right side
common-reference-operator-greater = greater than
common-reference-operator-greater-meaning = holds when the left side is above the right side
common-reference-operator-greater-or-equal = greater than or equal
common-reference-operator-greater-or-equal-meaning = holds when the left side is not below the right side
common-reference-operator-and = and
common-reference-operator-and-meaning = holds when both sides hold
common-reference-operator-or = or
common-reference-operator-or-meaning = holds when at least one side holds
common-reference-operator-not = not
common-reference-operator-not-meaning = holds when its operand does not hold
common-reference-operator-sqrt = square root
common-reference-operator-sqrt-meaning = the value at or above zero whose square is the operand
common-reference-operator-abs = absolute value
common-reference-operator-abs-meaning = the value without its sign
common-reference-operator-mul-add = multiply and add
common-reference-operator-mul-add-meaning = x times y plus z, rounded once instead of twice
common-reference-operator-floor = round down
common-reference-operator-floor-meaning = the largest whole number that is not above the value
common-reference-operator-ceil = round up
common-reference-operator-ceil-meaning = the smallest whole number that is not below the value
common-reference-operator-trunc = cut off
common-reference-operator-trunc-meaning = the whole part of the value, towards zero
common-reference-operator-round-ties-even = round, half to even
common-reference-operator-round-ties-even-meaning = the nearest whole number, and at exactly one half the even one
common-reference-operator-copysign = take the sign
common-reference-operator-copysign-meaning = the first value with the sign of the second
common-reference-operator-min = smaller value
common-reference-operator-min-meaning = the smaller of two values
common-reference-operator-max = larger value
common-reference-operator-max-meaning = the larger of two values
common-reference-operator-exp = exponential function
common-reference-operator-exp-meaning = e raised to the value
common-reference-operator-ln = natural logarithm
common-reference-operator-ln-meaning = the exponent that e is raised to in order to give the value
common-reference-operator-sin = sine
common-reference-operator-sin-meaning = the sine of an angle
common-reference-operator-cos = cosine
common-reference-operator-cos-meaning = the cosine of an angle
common-reference-operator-tan = tangent
common-reference-operator-tan-meaning = the tangent of an angle
common-reference-operator-asin = arc sine
common-reference-operator-asin-meaning = the angle whose sine is the value
common-reference-operator-acos = arc cosine
common-reference-operator-acos-meaning = the angle whose cosine is the value
common-reference-operator-atan = arc tangent
common-reference-operator-atan-meaning = the angle whose tangent is the value
common-reference-operator-atan2 = arc tangent of two values
common-reference-operator-atan2-meaning = the angle of the point x, y measured from the positive x axis
common-reference-operator-select = choice
common-reference-operator-select-meaning = the second value where the condition holds, otherwise the third
common-reference-operator-complex = complex number
common-reference-operator-complex-meaning = the number with the given real and imaginary part
common-reference-operator-to-f32 = as a 32-bit machine number
common-reference-operator-to-f32-meaning = the value rounded to a 32-bit binary floating-point number
common-reference-operator-to-f64 = as a 64-bit machine number
common-reference-operator-to-f64-meaning = the value rounded to a 64-bit binary floating-point number
common-reference-operator-exact = exact value
common-reference-operator-exact-meaning = the value as an exact number, computed without machine arithmetic
common-reference-operator-enclosure-lower = the lower end of a proven enclosure
common-reference-operator-enclosure-lower-meaning = the largest decimal of n significant digits that is at most the value, proven
common-reference-operator-enclosure-upper = the upper end of a proven enclosure
common-reference-operator-enclosure-upper-meaning = the smallest decimal of n significant digits that is at least the value, proven
common-reference-operator-from-celsius = from degrees Celsius
common-reference-operator-from-celsius-meaning = a reading in degrees Celsius as a temperature
common-reference-operator-from-fahrenheit = from degrees Fahrenheit
common-reference-operator-from-fahrenheit-meaning = a reading in degrees Fahrenheit as a temperature
common-reference-operator-to-celsius = in degrees Celsius
common-reference-operator-to-celsius-meaning = a temperature as a reading in degrees Celsius
common-reference-operator-to-fahrenheit = in degrees Fahrenheit
common-reference-operator-to-fahrenheit-meaning = a temperature as a reading in degrees Fahrenheit
common-reference-keyword-argument-shape = shape
common-reference-keyword-argument-shape-meaning = how a sum or a product is grouped while it is computed
common-reference-keyword-argument-side = side
common-reference-keyword-argument-side-meaning = the side a limit is taken from
common-reference-keyword-argument-coverage = coverage factor
common-reference-keyword-argument-coverage-meaning = the factor a standard uncertainty was multiplied by
common-reference-form-array = list
common-reference-form-array-meaning = values in order, between square brackets
common-reference-form-matrix = matrix
common-reference-form-matrix-meaning = rows of values, with a semicolon between the rows
common-reference-binder-lambda = function
common-reference-binder-lambda-meaning = a function of its variable: the variable, then the body
common-reference-binder-sum = sum
common-reference-binder-sum-meaning = the sum of the body over the variable, from the lower to the upper bound
common-reference-binder-sum-halving = sum in halves
common-reference-binder-sum-halving-meaning = the same sum, added in halves, which keeps the rounding error smaller
common-reference-binder-product = product
common-reference-binder-product-meaning = the product of the body over the variable, from the lower to the upper bound
common-reference-binder-product-halving = product in halves
common-reference-binder-product-halving-meaning = the same product, multiplied in halves, which keeps the rounding error smaller
common-reference-binder-integral = integral
common-reference-binder-integral-meaning = the integral of the body over the variable between the two bounds
common-reference-binder-limit = limit
common-reference-binder-limit-meaning = the value the body approaches as the variable goes to the point
common-reference-binder-limit-left = limit from the left
common-reference-binder-limit-left-meaning = the limit taken over values below the point
common-reference-binder-limit-right = limit from the right
common-reference-binder-limit-right-meaning = the limit taken over values above the point
common-reference-binder-derivative = derivative
common-reference-binder-derivative-meaning = the derivative of the body by the variable
common-reference-binder-root = real root
common-reference-binder-root-meaning = the k-th real root of the body as a polynomial in the variable, counted from the smallest
common-reference-binder-taylor = Taylor polynomial
common-reference-binder-taylor-meaning = the polynomial of order n about the point a whose derivatives there agree with those of the body
common-reference-binder-insertion-sort = insertion sort
common-reference-binder-insertion-sort-meaning = the list sorted by insertion sort, by the key if one is given, with its comparisons and writes counted; order=decreasing sorts by decreasing key
common-reference-binder-merge-sort = merge sort
common-reference-binder-merge-sort-meaning = the list sorted by top-down merge sort, by the key if one is given, with its comparisons and writes counted
common-reference-binder-heap-sort = heap sort
common-reference-binder-heap-sort-meaning = the list sorted by heap sort, by the key if one is given, with its comparisons and writes counted; not stable
common-reference-binder-quick-sort = quicksort
common-reference-binder-quick-sort-meaning = the list sorted by quicksort with the partition and pivot named, by the key if one is given, with its comparisons and writes counted; not stable
common-reference-binder-counting-sort = counting sort
common-reference-binder-counting-sort-meaning = the list sorted by counting sort, by the key if one is given, which must be a whole number, with its writes and counter updates counted
common-reference-binder-double-selection-sort = double selection sort
common-reference-binder-double-selection-sort-meaning = the list sorted by double selection sort, by the key if one is given, with its comparisons and writes counted
common-reference-binder-cocktail-shaker-sort = cocktail shaker sort
common-reference-binder-cocktail-shaker-sort-meaning = the list sorted by cocktail shaker sort in the form named by form=, by the key if one is given, with its comparisons and writes counted
common-reference-binder-gnome-sort = gnome sort
common-reference-binder-gnome-sort-meaning = the list sorted by gnome sort, by the key if one is given, with its comparisons and writes counted
common-reference-binder-odd-even-sort = odd–even sort
common-reference-binder-odd-even-sort-meaning = the list sorted by odd–even transposition sort in the form named by form=, by the key if one is given, with its comparisons and writes counted
common-reference-binder-comb-sort = comb sort
common-reference-binder-comb-sort-meaning = the list sorted by comb sort in the form named by form=, by the key if one is given, with its comparisons and writes counted
common-reference-binder-cycle-sort = cycle sort
common-reference-binder-cycle-sort-meaning = the list sorted by cycle sort, by the key if one is given, with its comparisons and writes counted
common-reference-binder-pancake-sort = pancake sort
common-reference-binder-pancake-sort-meaning = the list sorted by pancake sort, by the key if one is given, with its comparisons, writes and flips counted
common-reference-binder-shell-sort = Shell sort
common-reference-binder-shell-sort-meaning = the list sorted by Shell sort with the gap sequence named by gaps=, by the key if one is given, with its comparisons and writes counted
common-reference-binder-bottom-up-merge-sort = bottom-up merge sort
common-reference-binder-bottom-up-merge-sort-meaning = the list sorted by bottom-up merge sort, by the key if one is given, with its comparisons and writes counted
common-reference-binder-natural-merge-sort = natural merge sort
common-reference-binder-natural-merge-sort-meaning = the list sorted by natural merge sort, which merges the runs already in order, by the key if one is given, with its comparisons and writes counted
common-reference-binder-radix-sort = radix sort
common-reference-binder-radix-sort-meaning = the list sorted by radix sort in the base named by base=, by the key if one is given, which must be a whole number, with its writes and counter updates counted
common-reference-binder-bead-sort = bead sort
common-reference-binder-bead-sort-meaning = the list of whole numbers of at least 0, rebuilt in order by bead sort, with every bead counted as it falls and as it is read
common-reference-binder-bitonic-sort = bitonic sort
common-reference-binder-bitonic-sort-meaning = the list of 2^k entries sorted by Batcher's bitonic network, by the key if one is given, with its comparisons and writes counted
common-reference-binder-bogo-sort = bogo sort
common-reference-binder-bogo-sort-meaning = the list sorted by shuffling it until it is sorted, with shuffles drawn from philox4x32_10 with the seed given and at most the limit given, with its comparisons, writes and draws counted
common-reference-binder-binary-insertion-sort = binary insertion sort
common-reference-binder-binary-insertion-sort-meaning = the list sorted by binary insertion sort, by the key if one is given, with its comparisons and writes counted
common-reference-binder-selection-sort = selection sort
common-reference-binder-selection-sort-meaning = the list sorted by selection sort, by the key if one is given, with its comparisons and writes counted; not stable
common-reference-binder-bubble-sort = bubble sort
common-reference-binder-bubble-sort-meaning = the list sorted by bubble sort in the form named by form=, by the key if one is given, with its comparisons and writes counted
common-reference-form-comment = comment
common-reference-form-comment-meaning = text after a number sign, which is not computed
common-reference-statement-naming = naming
common-reference-statement-naming-meaning = gives a name to a value, so later lines can use it
common-reference-statement-function-naming = function naming
common-reference-statement-function-naming-meaning = gives a name to a function of its parameters

common-working-steps = working
common-working-step-reference = the value of { $line }
common-working-backend-unavailable = the backend that computed this line is not available here, so its steps are not shown
common-working-backend-unavailable-named = the backend { $backend } that computed this line is not available here, so its steps are not shown
common-working-approximate-operations = this line uses an operation whose result may differ between backends, so its steps are not shown
common-working-result-differs = the working gives { $shown } for this line, which differs from the value it holds
common-working-further-steps = { $count ->
    [one] { $count } further step
   *[other] { $count } further steps
}
common-record-recognized = concept
common-record-offer = offer
common-recognized-no-concept-set = the concept set did not load, so nothing was recognized
common-recognized-no-patterns = the concept set's patterns could not be built, so nothing was recognized
common-recognized-matcher-failed = the matcher could not run, so nothing was recognized

common-lens-explore = Explore
common-lens-learn = Learn
common-lens-train = Train
common-lens-read = Read
common-concept-statement = statement
common-concept-intuition = intuition
common-concept-examples = examples
common-concept-misconception = misconception
common-concept-lens = lens
common-concept-shown-by = has something in
common-concept-shown-by-none = nothing yet
common-concept-prerequisites = comes after
common-concept-sources = sources
common-concept-exercises = exercises
common-concept-activities = activities
common-concept-activity = { $kind } with { $shapes }, { $variation }
common-activity-variation-identical = each as it is
common-activity-variation-orientation = turned
common-activity-variation-size = in different sizes
common-activity-shape-matching = matching shapes
common-activity-shape-circle = circle
common-activity-shape-square = square
common-activity-shape-triangle = triangle
common-reference-operator-smallest = smallest
common-reference-operator-smallest-meaning = the least entry of a list or matrix
common-reference-operator-largest = largest
common-reference-operator-largest-meaning = the greatest entry of a list or matrix
common-reference-operator-sorted = sorted
common-reference-operator-sorted-meaning = the same list with its entries in increasing order
common-note-temperature-difference = this is a temperature difference in { $unit }, not a reading on that scale; for a reading write { $reading }
common-note-temperature-difference-inside = this value is a temperature difference in { $unit } and not a reading on that scale; a reading is written with { $operator }
common-note-temperature-difference-entry = the entry { $written } at position { $position } is a temperature difference in { $unit }, not a reading on that scale, and beside a reading it counts as that difference above absolute zero; for a reading write { $reading }
common-note-temperature-difference-entries = the entries in { $unit } are temperature differences, not readings on that scale; a reading is written with { $operator }
common-note-antiderivative = one antiderivative; every other differs from it by a constant
common-note-taylor-polynomial = the Taylor polynomial, which agrees with the function near the point and is not the function itself
common-note-radix-decimal = in decimal { $value }
common-note-chemistry-charge = charge { $charge }
common-note-chemistry-each-side = each side holds { $counts }
common-note-chemistry-charge-each-side = the charge on each side is { $charge }
common-note-chemistry-tables = standard atomic weights CIAAW 2024; molar mass constant CODATA 2022, taken as its value ± 2 standard uncertainties
common-note-chemistry-natural-interval = a range of natural variation that CIAAW gives: { $elements }
common-note-chemistry-expanded-uncertainty = CIAAW's expanded uncertainty, a bound and not a standard uncertainty: { $elements }
common-note-nuclear-measured-mass = AME2020 holds a measured atomic mass for it
common-note-nuclear-estimated-mass = AME2020 holds only an estimated mass for it, taken from trends of the mass surface
common-note-nuclear-no-mass = AME2020 holds no mass for it
common-note-nuclear-each-side = each side holds mass number { $mass }, charge { $charge } and electron lepton number { $leptons }
common-note-nuclear-photons = the number of photons is not fixed by any conserved quantity and is taken as written
common-note-nuclear-q-tables = atomic masses AME2020; u·c² and m_e·c² CODATA 2022; each taken as its value ± 2 standard uncertainties, and a printed mass also ± 3 units of its last printed digit; the ends are added, so the range holds whatever the correlations between the masses
common-note-nuclear-q-atomic = an atomic-mass Q value: it differs from the Q value for bare nuclei by the change in electron binding
common-note-nuclear-q-ground-state = from ground state to ground state
common-note-nuclear-q-capture = an electron capture is given without the binding energy of the captured electron
common-note-nuclear-q-positron = the energy is given before the positron annihilates
common-note-nuclear-q-decay-impossible = as a decay this cannot happen for the neutral atom: its Q value is below 0
common-note-nuclear-q-sign-undecided = the sign of this Q value is not decided at this coverage: its range contains 0
common-note-radix-written-decimal = written in decimal
common-note-worst-case-low = lowest where { $corner }
common-note-worst-case-high = highest where { $corner }
common-note-free-names = written in the free names { $names }, which no line gives a value
common-note-overflowed-to-infinity = every operand was finite and the result is not: the machine format cannot hold a number this large
common-note-not-a-number-from-finite-operands = every operand was finite and the result is not a number: the machine arithmetic has no value for this one
common-note-bound-not-smaller-than-value = the rounding error is not smaller than the value, so these digits say nothing about it; the same line without { $conversion } is evaluated exactly
common-note-temperature-difference-mixed = this value is a temperature difference, not a reading on a temperature scale
common-note-temperature-reading = this is a reading on the { $scale } scale
common-method-exact-after-conversion = exactly, after rounding each to_f64 or to_f32 argument to a machine value
common-note-machine-conversion = { $written } rounds { $argument } to the machine value { $machine }; the difference is { $difference }
common-reading-number = decimal, rounded to { $digits } significant digits
common-reading-computed = from the exact value above, rounded half away from zero
common-reading-below = { $distance } below the exact value
common-reading-above = { $distance } above the exact value
common-reading-at-most = at most { $distance }
common-note-zero-to-the-zero = 0^0 is taken as 1, the empty product, as the binomial theorem and power series need it; that x^y has no limit as x and y both go to 0 is a different question
common-record-valid = valid
common-where-not-zero = { $count ->
    [one] wherever { $denominators } is not zero
   *[other] wherever { $denominators } are not zero
}
common-valid-where = { $count ->
    [one] { $denominators }; where it is zero, the line has no value
   *[other] { $denominators }; where one of them is zero, the line has no value
}
