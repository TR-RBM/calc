error-parse-unexpected-character = unexpected character { $name } at column { $column }
error-parse-unexpected-token = unexpected { $name } at column { $column }
error-parse-unexpected-end = the input ends too early at column { $column }
error-parse-invalid-typed-literal = invalid typed literal at column { $column }
error-parse-exponent-too-large = the exponent in { $name } is above { $limit } in magnitude, at column { $column }
error-parse-not-a-unit = { $name } is not a unit at column { $column }
error-parse-atomic-mass-unit = { $name } is the atomic mass unit, whose value in kilograms is measured, and calc's units all have exact factors; write it through the constant m_u, as 12 * m_u, or 2e-26 kg / m_u for a mass in atomic mass units, at column { $column }
error-parse-not-a-unit-joined-to-a-unit = { $name } is not a unit at column { $column }: a name joined to a unit by * or / without spaces is read as part of the unit; to multiply or divide by { $name }, put a space on each side of the * or /
error-parse-ambiguous-unit = { $name } at column { $column } is written for more than one unit; write one of { $readings }, which each mean one
error-parse-type-expected = { $name } at column { $column } is not an integer type; write one such as u8, i16 or u32
error-parse-byte-order-missing = bytes at column { $column } needs its byte order: write bytes be for the most significant byte first or bytes le for the least significant first
error-parse-unit-exponent-out-of-range = the power of { $name } is outside { $lowest } to { $highest }, at column { $column }
error-parse-unit-exponent-not-whole = a unit takes only whole powers, and { $name } is not a whole number, at column { $column }
error-parse-reserved-name = { $name } is a reserved name at column { $column }
error-parse-chained-relation = relations cannot be chained; join them with and, at column { $column }
error-parse-ragged-array = the array rows have different lengths at column { $column }
error-parse-unknown-keyword = unknown keyword argument { $name } at column { $column }
error-parse-duplicate-keyword = keyword argument { $name } is given twice at column { $column }
error-parse-positional-after-keyword = { $name } at column { $column } comes after a keyword argument; write every positional argument before the keyword arguments
error-parse-invalid-keyword-value = { $name } is not a value of the keyword argument { $keyword } at column { $column }; { $keyword } takes { $values }
error-parse-missing-keyword = the keyword argument { $keyword } is needed at column { $column }; it takes { $values }
error-parse-bubble-sort-form = bubble sort is taught in forms that count differently, so name one at column { $column }: form=full makes n - 1 passes over the whole list; form=shrinking ends each pass before the entries already placed; form=early_exit also stops after a pass without an exchange; form=last_exchange ends each pass at the last exchange of the pass before
error-parse-missing-pivot = the keyword argument pivot is needed at column { $column }; with this partition write pivot={ $built }
error-parse-missing-seed = pivot=random draws its pivots from philox4x32_10, and the seed decides which, so write seed= with a whole number from 0 to 2^64 − 1 at column { $column }
error-parse-invalid-seed = seed= takes a whole number from 0 to 2^64 − 1 written in digits, at column { $column }
error-parse-seed-without-random-pivot = seed= at column { $column } is for pivot=random, and this sort draws no pivots
error-array-key-not-whole = { $method } counts whole numbers, and { $key } is not one; choose a sort by comparisons for it
error-array-key-not-whole-entry = { $method } counts whole numbers, and a key is not one; choose a sort by comparisons for it
error-array-key-with-unit = { $method } counts whole numbers, and { $key } carries a unit, so whether it is a whole number at all, and which one, depends on the unit it is counted in; count it in the unit you mean by dividing by that unit, such as 1 cm
error-array-key-with-unit-entry = { $method } counts whole numbers, and a key carries a unit, so whether it is a whole number at all, and which one, depends on the unit it is counted in; count it in the unit you mean by dividing by that unit, such as 1 cm
error-array-function-key-machine = the key function gives { $key }, a machine number, and { $method } counts only exact whole numbers; write a key that computes exactly, without a machine conversion
error-array-function-key-machine-entry = the key function gives a machine number, and { $method } counts only exact whole numbers; write a key that computes exactly, without a machine conversion
error-array-range-too-wide = the keys span more values than counting sort keeps counters for
error-generator-seed-machine = the seed of philox4x32_10 is a whole number written exactly, and { $value } is a machine number; write the seed without a machine conversion
error-generator-stream-machine = the stream of philox4x32_10 is a whole number written exactly, and { $value } is a machine number; write the stream without a machine conversion
error-generator-index-machine = the index of philox4x32_10 is a whole number written exactly, and { $value } is a machine number; write the index without a machine conversion
error-generator-seed-refused = the seed of philox4x32_10 is a whole number from 0 to 2^64 − 1 written without a unit, and { $value } is not one
error-generator-stream-refused = the stream of philox4x32_10 is a whole number from 0 to 2^64 − 1 written without a unit, and { $value } is not one
error-generator-index-refused = the index of philox4x32_10 is a whole number from 0 to 2^64 − 1 written without a unit, and { $value } is not one
error-sort-range-too-wide = the keys span { $range } values, and counting sort keeps at most { $limit } counters; choose a sort by comparisons for them
error-parse-quick-sort-partition = quicksort is taught with partitions that count differently, so name one at column { $column }: partition=lomuto, pivot=last scans once from the left with the last entry as pivot; partition=hoare, pivot=first scans from both ends with the first entry as pivot
error-parse-cocktail-shaker-sort-form = cocktail shaker sort is taught in forms that count differently, so name one at column { $column }: form=full scans the whole list each time; form=shrinking ends each scan one entry earlier; form=last_exchange ends each scan where the one before it made its last exchange
error-parse-odd-even-sort-form = odd–even sort is taught in two forms that count differently, so name one at column { $column }: form=until_sorted repeats rounds until one makes no exchange; form=fixed_passes makes exactly n phases
error-parse-comb-sort-form = comb sort is taught with shrink factors that count differently, so name the form at column { $column }: form=lacey_box shrinks the gap by 1.3 with the rule of 11; other forms are not built yet
error-parse-shell-sort-gaps = Shell sort is taught with gap sequences that count differently, so name one at column { $column }: gaps=shell halves the gap from n/2; gaps=knuth uses (3^k − 1)/2 up to ceil(n/3); gaps=ciura uses 1, 4, 10, 23, 57, 132, 301, 701
error-parse-missing-base = radix sort sorts digit by digit, and the base decides the digits and every count, so name one at column { $column }: base=10 sorts by decimal digits, base=2 by binary digits, base=256 by bytes
error-parse-invalid-base = base= takes a whole number from 2 to 65536 written in digits, at column { $column }
error-parse-missing-shuffle-seed = bogo sort draws its shuffles from philox4x32_10, and the seed decides them, so write seed= with a whole number from 0 to 2^64 − 1 at column { $column }
error-parse-missing-limit = bogo sort shuffles until the list is sorted, which has no bound, so name the most shuffles you allow with limit= at column { $column }, as in limit=1000
error-parse-invalid-limit = limit= takes a whole number from 0 to 2^64 − 1 written in digits, at column { $column }
error-sort-base-too-small = the base of radix sort is at least 2: a base of 0 or 1 has no digits to sort by
error-sort-base-too-large = the base of radix sort is at most { $limit }, since each pass keeps one counter for each digit value
error-sort-bead-no-key = bead sort rebuilds each value from its beads and moves no entry, so it cannot carry an entry by its key; sort the numbers themselves with bead_sort(a), or choose a sort that moves entries
error-sort-limit-reached = bogo sort stopped at its limit of { $limit } shuffles without sorting, so there is no list to show; counted up to there: comparisons { $comparisons }, writes { $writes }, draws { $draws }; it ends with probability 1, and a larger limit= or another seed= lets it go on
error-sort-length-not-power-of-two = bitonic sort is a network for 2^k entries, and this list has { $length }, between { $below } and { $above }; a form for other lengths is not built yet, so choose a sort such as merge_sort for this list
error-sort-bead-below-zero = bead sort lays out each number as that many beads, and { $key } is below 0, so it has no beads to lay out; choose radix_sort or a sort by comparisons for it
error-sort-too-many-beads = bead sort moves every bead, one for each unit of each number, and these numbers have { $beads } beads, more than the { $limit } it moves; choose radix_sort or counting_sort for them
error-parse-pivot-not-built-for-partition = pivot={ $name } at column { $column } is not built for this partition: quick sort pairs partition=lomuto with pivot=last or pivot=random, and partition=hoare with pivot=first, and other pivots are not built yet
error-parse-missing-differential = the integral has no differential such as dx at column { $column }
error-parse-nested-too-deeply = the expression goes more than { $limit } levels deep; split it into named lines, at column { $column }
error-parse-chain-too-long = the expression joins more than { $limit } operations in one chain; split it into named lines, at column { $column }
error-parse-expression-too-deep = brackets and chains together put this expression more than { $limit } levels deep; split it into named lines, at column { $column }
error-parse-ambiguous-application = { $written } can mean { $narrow } or { $wide }; write the one you mean, at column { $column }
error-parse-ambiguous-temperature-sign = { $written } can mean the temperature, written { $reading }, or a difference of that many degrees, written { $difference }; write the one you mean, at column { $column }
error-parse-fraction-before-unit = { $written } can mean { $fraction } or { $reciprocal }; write the one you mean, at column { $column }
error-parse-percent-in-a-sum = { $written } can mean { $literal } or { $relative }; write the one you mean, at column { $column }
error-parse-attempt-degree-after-name = ° stands after a number, a measured value, a group or a constant such as pi, and after no other name: for an angle in degrees write { $corrected }, at column { $column }
error-parse-attempt-degree-after-name-plain = ° stands after a number, a measured value, a group or a constant such as pi, and after no other name: for an angle in degrees multiply by 1°, at column { $column }
error-parse-attempt-double-star-power = a power is written with ^, so write { $corrected }, at column { $column }
error-parse-attempt-comma-between-digits = the decimal point is written with ., so write { $corrected }, at column { $column }
error-parse-attempt-grouping-comma = numbers are written without grouping commas, and the decimal point is ., as in 1000 or 2.5, at column { $column }
error-parse-attempt-absolute-value-bars = absolute value bars are written with abs, so write { $corrected }, at column { $column }
error-parse-attempt-times-sign = multiplying numbers is written with * or ·, so write { $corrected }, at column { $column }
error-parse-attempt-division-sign = division is written with /, so write { $corrected }, at column { $column }
error-parse-attempt-double-star-power-plain = a power is written with ^, as in 2^3, at column { $column }
error-parse-attempt-comma-between-digits-plain = the decimal point is written with ., as in 2.5, at column { $column }
error-parse-attempt-absolute-value-bars-plain = absolute value bars are written with abs, as in abs(x - 3), at column { $column }
error-parse-attempt-times-sign-plain = multiplying numbers is written with * or ·, as in 2 * 3, at column { $column }
error-parse-attempt-division-sign-plain = division is written with /, as in 6 / 3, at column { $column }
error-parse-attempt-colon-or-time = a colon is not used: division is written with /, as in 80 / 20, and a time is not a number here, at column { $column }
error-parse-arity-mismatch = expected { $expected ->
    [one] { $expected } argument
   *[other] { $expected } arguments
}, found { $found }, at column { $column }
error-name-kind-conflict = the name is already used for another kind of name

error-expression-too-large = the expression is too large
error-result-too-large = { $reading } would have at least { $digits } digits; the limit is about { $limit_digits } digits ({ $limit } bits)
error-intermediate-step-too-large = the intermediate step { $reading } would have at least { $digits } digits; the limit is about { $limit_digits } digits ({ $limit } bits), so evaluation stopped there
error-undefined-name = { $name } is not defined
error-undefined-unnamed-generator = calc has no { $name }: which generator draws a number decides the number, so a generator is asked for by its name; write philox4x32_10(seed, stream, index)
error-undefined-name-constant = { $name } is not defined; the physical constant is written { $constant }
error-input-not-parsed = the input no longer parses, at column { $column }
error-unit-ended-at-space = the unit ends at the space before the operator, so { $name } is read as a name and not as a unit: write the unit without spaces as { $replacement }, as in { $corrected }, at column { $column }
error-unit-ended-at-space-plain = the unit ends at the space before the operator, so { $name } is read as a name and not as a unit: write the unit without spaces as { $replacement }, at column { $column }
error-reference-cycle = { $line } would depend on itself
error-name-exists = { $name } already exists
error-name-in-use = { $line } cannot be renamed because { $dependents } use it
error-unknown-line = { $line } does not exist
error-line-numbers-exhausted = the session has no free line numbers
error-dependency-failed = { $line } has no result
error-precision-not-supported = precision { $precision } is not supported yet
error-result-kind-not-representable = a result of kind { $kind } cannot be shown yet
error-result-not-representable = the result cannot be represented
error-division-by-zero = { $reading } divides by zero
error-relation-is-a-claim = { $reading } is a claim, not a value: it is decided, not computed
error-relation-about-free-names = { $reading } is a claim about { $names }, not a value: it is decided or solved, not computed
error-outside-domain = { $reading } is not defined
error-not-in-radical-field = { $operator } takes apart a value built from rationals and square roots, and { $reading } is not one
error-remainder-not-computed = calc computes { $reading } only where both arguments are rational numbers or square roots of them; it does not compute it for other values yet
error-power-of-zero-without-value = { $reading } has no value: a power of 0 has a value only when the real part of its exponent is above 0, and here the real part is 0
error-power-of-zero-sign-undecided = { $reading } has a value only once the sign of its exponent is known: 0 to a power above 0 is 0, to a power below 0 it divides by zero, and to the power 0 it is 1; calc could prove the exponent neither above 0 nor below 0
error-not-a-square-root-term = the second argument of coefficient_of is sqrt(n), with n a whole number above 1 that no square other than 1 divides, such as sqrt(2); { $reading } is not one
error-not-a-square-root-term-multiple = the second argument of coefficient_of is sqrt(n), with n a whole number above 1 that no square other than 1 divides; { $reading } is a multiple of sqrt({ $radicand }), so ask for sqrt({ $radicand })
error-tolerance-written-twice = { $reading } is written more than once in this line, and calc cannot tell whether it is one component or several: name it, as R = { $reading }, and use the name where it is the same component, or name each component on its own line
error-worst-case-too-many = a line may carry at most { $limit } toleranced values
error-worst-case-unsupported = calc cannot take the worst case of this line
error-worst-case-endpoint-not-exact = the ends of { $reading } must be exact values
error-worst-case-endpoints-reversed = { $reading } has its lower end above its upper end
error-worst-case-negative-tolerance = the tolerance in { $reading } is negative; a tolerance is a share from 0
error-worst-case-not-monotone = calc cannot show that the line only rises or only falls while { $reading } runs through its range, so it gives no worst case rather than one that could be wrong
error-worst-case-divisor-reaches-zero = { $reading } divides by a value that can be zero while its ranges run through their ends, or that calc cannot show stays away from zero
error-worst-case-range-in-function-body = the body of { $name } holds a range, which could be one part for every call or one part per call; name the range and use the name in { $name }, or pass the range to { $name } as an argument
error-worst-case-ends-of-two-dimensions = the two ends of { $reading } measure different quantities; both ends take the same unit or units of one quantity
error-worst-case-endpoint-not-enclosed = calc cannot bound the ends of { $reading } closely enough to take a worst case; an end such as sqrt(2) is not supported yet
error-integer-not-whole = { $reading } needs a whole number, and this one is not
error-integer-negative = { $reading } needs a number from 0: a negative number has bits only at a fixed width, and wrap(x, u32) gives them as a number from 0
error-integer-shift-too-large = { $reading } shifts by more than { $limit } places or by a negative number of places
error-integer-not-whole-bytes = { $reading }: { $kind } is not a whole number of bytes
error-integer-outside-type = { $reading } lies outside { $kind }, which holds { $low } to { $high }; wrap(x, { $kind }) reduces it into { $kind } on purpose
error-integer-argument-out-of-range = in { $reading }, the argument { $argument } is above the largest allowed argument { $limit }
error-unsupported-operator = { $reading } cannot be evaluated yet
error-unsupported-expression = this expression cannot be evaluated yet
error-unsupported-subexpression = { $reading } cannot be evaluated yet
error-unsupported-constant = the constant { $reading } cannot be evaluated exactly
error-not-finite = { $reading } is not finite
error-machine-number-in-exact = the machine number { $reading } cannot be evaluated exactly
error-quantity-not-converted = the quantity must be converted to a unit first
error-dimension-mismatch = { $reading } combines quantities of different dimensions
error-dimensioned-argument = { $reading } needs a dimensionless argument
error-dimensioned-power-exponent-not-constant = the exponent in { $reading } is not a constant, and its base has a unit
error-fractional-dimension = { $reading } takes a fractional power of a unit
error-dimension-out-of-range = the unit powers in { $reading } are outside { $lowest } to { $highest }
error-empty-range = the range is empty
error-index-range-too-long = the index range of { $reading } has more terms than can be counted

error-no-backend = no backend can run this evaluation
error-backend-not-registered = the { $backend } backend is not available
error-backend-rejected = the { $backend } backend cannot run this evaluation
error-out-of-device-memory = the device is out of memory
error-device-lost = the device was lost

error-cancelled = the evaluation was cancelled
error-not-applicable = { $instrument } does not apply to this line
error-renderer-unavailable = pictures cannot be drawn in the terminal yet
error-internal = internal error { $code }

error-file-not-found = { $path } does not exist
error-file-permission-denied = no permission to access { $path }
error-file-unreadable = { $path } cannot be read
error-file-unwritable = { $path } cannot be written
error-session-invalid = { $path } is not a valid session file at { $member }
error-session-not-json = { $path } is not a JSON session file; a file of expressions, one per line, runs with calc --batch < { $path }
error-session-dependency-cycle = { $path } has a dependency cycle through { $line }
error-session-newer-version = { $path } has format version { $found }, this Calculator reads version { $supported }

error-invalid-request = the request is invalid at { $path }: { $code }
error-not-a-solve-line = { $line } is not a solve line
error-solve-failed = the given-and-wanted search failed

error-solve-missing-wanted = wanted: name the quantity you want
error-solve-unknown-wanted = wanted: { $name } is not a known quantity
error-solve-unknown-given-quantity = given { $row }: { $name } is not a known quantity
error-solve-unknown-object = given { $row }: { $name } is not a known object
error-solve-malformed-given = given { $row }: { $text } is not a value with an optional uncertainty and unit
error-solve-empty-criterion = criterion: choose at least one criterion
error-cockpit-no-display = no display was found: the cockpit needs a graphical session with DISPLAY or WAYLAND_DISPLAY set, and the calc command works without one
error-cockpit-window-system-refused = the window system refused to start the cockpit: { $reason }
error-cockpit-window-not-opened = the cockpit window could not be opened ({ $code })
error-cockpit-presentation-failed = the cockpit could not show its frame in the window ({ $code })
error-cockpit-fonts-unreadable = the bundled fonts could not be read ({ $code })
error-cockpit-scale-unusable = the window scale cannot be used ({ $code })
error-cockpit-canvas-failed = the cockpit could not create its drawing surface ({ $code })
error-cockpit-drawing-failed = the cockpit could not draw its window ({ $code })
error-negative-uncertainty = the uncertainty in { $reading } is negative
error-nested-uncertainty = { $reading } gives an uncertainty its own uncertainty
error-coverage-factor-below-one = the coverage factor in { $reading } is below 1
error-uncertainty-not-propagated = the uncertainty cannot be propagated through this expression
error-division-by-zero-plain = division by zero
error-outside-domain-plain = { $operator } is not defined for this argument
error-not-in-radical-field-plain = { $operator } takes apart only a value built from rationals and square roots
error-remainder-not-computed-plain = calc computes mod only where both arguments are rational numbers or square roots of them
error-power-of-zero-without-value-plain = a power of 0 has a value only when the real part of its exponent is above 0, and here the real part is 0
error-power-of-zero-sign-undecided-plain = a power of 0 has a value only once the sign of its exponent is known, and calc could prove the exponent neither above 0 nor below 0
error-not-a-square-root-term-plain = the second argument of coefficient_of is sqrt(n), with n a whole number above 1 that no square other than 1 divides, such as sqrt(2)
error-domain-sqrt = sqrt needs a number of at least 0, and { $operand } is below 0
error-domain-ln = ln needs a number above 0, and { $operand } is not above 0
error-domain-sqrt-complex = sqrt needs a number of at least 0, and { $operand } is below 0; for the complex root write i * sqrt({ $magnitude })
error-domain-ln-complex = ln needs a number above 0, and { $operand } is not above 0; for the complex logarithm write ln({ $magnitude }) + i*pi
error-domain-arc-sine = { $operator } needs a number from -1 to 1, and { $operand } is outside that range
error-domain-tan = tan is not defined at { $operand }, an odd multiple of pi / 2
error-domain-factorial = ! needs a whole number of at least 0, and { $operand } is not one
error-domain-power = a power whose exponent is not a whole number is computed only for a base above 0, and { $base } is not above 0 (exponent { $exponent })
error-integer-argument-out-of-range-plain = the argument of { $operator } is too large
error-exponent-out-of-range = the exponent { $exponent } is above the largest allowed exponent { $limit }
error-exponent-out-of-range-long = the exponent { $exponent } has { $digits } digits, above the largest allowed exponent { $limit }
error-root-degree-out-of-range = the exponent { $exponent } takes a root of degree { $degree }, above the largest allowed degree { $limit }
error-root-degree-out-of-range-long = the exponent { $exponent } takes a root whose degree has { $digits } digits, above the largest allowed degree { $limit }
error-factorial-out-of-range = the number { $operand } before ! is above the largest allowed number before !, { $limit }
error-factorial-out-of-range-long = the number { $operand } before ! has { $digits } digits, above the largest allowed number before !, { $limit }
error-number-too-large = { $reading } would need about { $digits } digits; the limit is about { $limit_digits } digits ({ $limit } bits)
error-result-too-large-plain = the result is too large
error-unsupported-operator-plain = { $operator } cannot be evaluated yet
error-unsupported-constant-plain = this constant cannot be evaluated exactly
error-not-finite-plain = the result is not finite
error-machine-number-in-exact-plain = a machine number cannot be evaluated exactly
error-index-range-too-long-plain = an index range has more terms than can be counted
error-dimension-mismatch-plain = the expression combines quantities of different dimensions
error-dimension-mismatch-sides = { $reading }: { $left_quantity ->
    [0] one side is a pure number
    [1] { $left_unit } measures length (units: { $left_units })
    [2] { $left_unit } measures mass (units: { $left_units })
    [3] { $left_unit } measures time (units: { $left_units })
    [4] { $left_unit } measures electric current (units: { $left_units })
    [5] { $left_unit } measures temperature (units: { $left_units })
    [6] { $left_unit } measures amount of substance (units: { $left_units })
    [7] { $left_unit } measures luminous intensity or luminous flux (units: { $left_units })
    [8] { $left_unit } measures information (units: { $left_units })
    [9] { $left_unit } measures area (units: { $left_units })
    [10] { $left_unit } measures volume (units: { $left_units })
    [11] { $left_unit } measures speed (units: { $left_units })
    [12] { $left_unit } measures acceleration (units: { $left_units })
    [13] { $left_unit } measures force (units: { $left_units })
    [14] { $left_unit } measures pressure (units: { $left_units })
    [15] { $left_unit } measures energy or torque (units: { $left_units })
    [16] { $left_unit } measures power (units: { $left_units })
    [17] { $left_unit } measures frequency or activity (units: { $left_units })
    [18] { $left_unit } measures electric charge (units: { $left_units })
    [19] { $left_unit } measures voltage (units: { $left_units })
    [20] { $left_unit } measures capacitance (units: { $left_units })
    [21] { $left_unit } measures resistance (units: { $left_units })
    [22] { $left_unit } measures conductance (units: { $left_units })
    [23] { $left_unit } measures magnetic flux (units: { $left_units })
    [24] { $left_unit } measures magnetic flux density (units: { $left_units })
    [25] { $left_unit } measures inductance (units: { $left_units })
    [26] { $left_unit } measures data rate (units: { $left_units })
    [27] { $left_unit } measures density (units: { $left_units })
    [28] { $left_unit } measures absorbed or equivalent dose (units: { $left_units })
    [29] { $left_unit } measures illuminance (units: { $left_units })
    [30] { $left_unit } measures catalytic activity (units: { $left_units })
   *[other] { $left_unit } measures a quantity calc has no name for (units: { $left_units })
} and { $right_quantity ->
    [0] one side is a pure number
    [1] { $right_unit } measures length (units: { $right_units })
    [2] { $right_unit } measures mass (units: { $right_units })
    [3] { $right_unit } measures time (units: { $right_units })
    [4] { $right_unit } measures electric current (units: { $right_units })
    [5] { $right_unit } measures temperature (units: { $right_units })
    [6] { $right_unit } measures amount of substance (units: { $right_units })
    [7] { $right_unit } measures luminous intensity or luminous flux (units: { $right_units })
    [8] { $right_unit } measures information (units: { $right_units })
    [9] { $right_unit } measures area (units: { $right_units })
    [10] { $right_unit } measures volume (units: { $right_units })
    [11] { $right_unit } measures speed (units: { $right_units })
    [12] { $right_unit } measures acceleration (units: { $right_units })
    [13] { $right_unit } measures force (units: { $right_units })
    [14] { $right_unit } measures pressure (units: { $right_units })
    [15] { $right_unit } measures energy or torque (units: { $right_units })
    [16] { $right_unit } measures power (units: { $right_units })
    [17] { $right_unit } measures frequency or activity (units: { $right_units })
    [18] { $right_unit } measures electric charge (units: { $right_units })
    [19] { $right_unit } measures voltage (units: { $right_units })
    [20] { $right_unit } measures capacitance (units: { $right_units })
    [21] { $right_unit } measures resistance (units: { $right_units })
    [22] { $right_unit } measures conductance (units: { $right_units })
    [23] { $right_unit } measures magnetic flux (units: { $right_units })
    [24] { $right_unit } measures magnetic flux density (units: { $right_units })
    [25] { $right_unit } measures inductance (units: { $right_units })
    [26] { $right_unit } measures data rate (units: { $right_units })
    [27] { $right_unit } measures density (units: { $right_units })
    [28] { $right_unit } measures absorbed or equivalent dose (units: { $right_units })
    [29] { $right_unit } measures illuminance (units: { $right_units })
    [30] { $right_unit } measures catalytic activity (units: { $right_units })
   *[other] { $right_unit } measures a quantity calc has no name for (units: { $right_units })
}, so they cannot be added, compared or converted into each other{ $suggestion_count ->
    [0] .
   *[other] . Did you mean { $suggestion }?
}
error-sort-keys-differ-in-dimension = { $first } at position { $first_position } and { $second } at position { $second_position } cannot be put in one order: { $left_quantity ->
    [0] one side is a pure number
    [1] { $left_unit } measures length
    [2] { $left_unit } measures mass
    [3] { $left_unit } measures time
    [4] { $left_unit } measures electric current
    [5] { $left_unit } measures temperature
    [6] { $left_unit } measures amount of substance
    [7] { $left_unit } measures luminous intensity or luminous flux
    [8] { $left_unit } measures information
    [9] { $left_unit } measures area
    [10] { $left_unit } measures volume
    [11] { $left_unit } measures speed
    [12] { $left_unit } measures acceleration
    [13] { $left_unit } measures force
    [14] { $left_unit } measures pressure
    [15] { $left_unit } measures energy or torque
    [16] { $left_unit } measures power
    [17] { $left_unit } measures frequency or activity
    [18] { $left_unit } measures electric charge
    [19] { $left_unit } measures voltage
    [20] { $left_unit } measures capacitance
    [21] { $left_unit } measures resistance
    [22] { $left_unit } measures conductance
    [23] { $left_unit } measures magnetic flux
    [24] { $left_unit } measures magnetic flux density
    [25] { $left_unit } measures inductance
    [26] { $left_unit } measures data rate
    [27] { $left_unit } measures density
    [28] { $left_unit } measures absorbed or equivalent dose
    [29] { $left_unit } measures illuminance
    [30] { $left_unit } measures catalytic activity
   *[other] { $left_unit } measures a quantity calc has no name for
} and { $right_quantity ->
    [0] one side is a pure number
    [1] { $right_unit } measures length
    [2] { $right_unit } measures mass
    [3] { $right_unit } measures time
    [4] { $right_unit } measures electric current
    [5] { $right_unit } measures temperature
    [6] { $right_unit } measures amount of substance
    [7] { $right_unit } measures luminous intensity or luminous flux
    [8] { $right_unit } measures information
    [9] { $right_unit } measures area
    [10] { $right_unit } measures volume
    [11] { $right_unit } measures speed
    [12] { $right_unit } measures acceleration
    [13] { $right_unit } measures force
    [14] { $right_unit } measures pressure
    [15] { $right_unit } measures energy or torque
    [16] { $right_unit } measures power
    [17] { $right_unit } measures frequency or activity
    [18] { $right_unit } measures electric charge
    [19] { $right_unit } measures voltage
    [20] { $right_unit } measures capacitance
    [21] { $right_unit } measures resistance
    [22] { $right_unit } measures conductance
    [23] { $right_unit } measures magnetic flux
    [24] { $right_unit } measures magnetic flux density
    [25] { $right_unit } measures inductance
    [26] { $right_unit } measures data rate
    [27] { $right_unit } measures density
    [28] { $right_unit } measures absorbed or equivalent dose
    [29] { $right_unit } measures illuminance
    [30] { $right_unit } measures catalytic activity
   *[other] { $right_unit } measures a quantity calc has no name for
}, so they cannot be added, compared or converted into each other.
error-sort-keyed-keys-differ-in-dimension = the key { $first } of the entry at position { $first_position } and the key { $second } of the entry at position { $second_position } cannot be put in one order: { $left_quantity ->
    [0] one side is a pure number
    [1] { $left_unit } measures length
    [2] { $left_unit } measures mass
    [3] { $left_unit } measures time
    [4] { $left_unit } measures electric current
    [5] { $left_unit } measures temperature
    [6] { $left_unit } measures amount of substance
    [7] { $left_unit } measures luminous intensity or luminous flux
    [8] { $left_unit } measures information
    [9] { $left_unit } measures area
    [10] { $left_unit } measures volume
    [11] { $left_unit } measures speed
    [12] { $left_unit } measures acceleration
    [13] { $left_unit } measures force
    [14] { $left_unit } measures pressure
    [15] { $left_unit } measures energy or torque
    [16] { $left_unit } measures power
    [17] { $left_unit } measures frequency or activity
    [18] { $left_unit } measures electric charge
    [19] { $left_unit } measures voltage
    [20] { $left_unit } measures capacitance
    [21] { $left_unit } measures resistance
    [22] { $left_unit } measures conductance
    [23] { $left_unit } measures magnetic flux
    [24] { $left_unit } measures magnetic flux density
    [25] { $left_unit } measures inductance
    [26] { $left_unit } measures data rate
    [27] { $left_unit } measures density
    [28] { $left_unit } measures absorbed or equivalent dose
    [29] { $left_unit } measures illuminance
    [30] { $left_unit } measures catalytic activity
   *[other] { $left_unit } measures a quantity calc has no name for
} and { $right_quantity ->
    [0] one side is a pure number
    [1] { $right_unit } measures length
    [2] { $right_unit } measures mass
    [3] { $right_unit } measures time
    [4] { $right_unit } measures electric current
    [5] { $right_unit } measures temperature
    [6] { $right_unit } measures amount of substance
    [7] { $right_unit } measures luminous intensity or luminous flux
    [8] { $right_unit } measures information
    [9] { $right_unit } measures area
    [10] { $right_unit } measures volume
    [11] { $right_unit } measures speed
    [12] { $right_unit } measures acceleration
    [13] { $right_unit } measures force
    [14] { $right_unit } measures pressure
    [15] { $right_unit } measures energy or torque
    [16] { $right_unit } measures power
    [17] { $right_unit } measures frequency or activity
    [18] { $right_unit } measures electric charge
    [19] { $right_unit } measures voltage
    [20] { $right_unit } measures capacitance
    [21] { $right_unit } measures resistance
    [22] { $right_unit } measures conductance
    [23] { $right_unit } measures magnetic flux
    [24] { $right_unit } measures magnetic flux density
    [25] { $right_unit } measures inductance
    [26] { $right_unit } measures data rate
    [27] { $right_unit } measures density
    [28] { $right_unit } measures absorbed or equivalent dose
    [29] { $right_unit } measures illuminance
    [30] { $right_unit } measures catalytic activity
   *[other] { $right_unit } measures a quantity calc has no name for
}, so they cannot be added, compared or converted into each other.
error-dimensioned-argument-plain = a function needs a dimensionless argument
error-dimensioned-power-exponent-not-constant-plain = an exponent is not a constant, and its base has a unit
error-fractional-dimension-plain = the expression takes a fractional power of a unit
error-dimension-out-of-range-plain = the unit powers in the expression are out of range
error-negative-uncertainty-plain = an uncertainty is negative
error-nested-uncertainty-plain = an uncertainty carries its own uncertainty
error-coverage-factor-below-one-plain = the coverage factor is below 1
error-holds-free-names = { $line } holds the free name { $names } and so has no single value to show this way; a line that gives { $names } a value makes one
error-machine-line-not-applicable = { $line } cannot be evaluated as a machine line: machine evaluation takes an expression or naming line that has a result
error-digits-not-applicable = { $line } has no decimal places view: it takes an exact rational or a finite machine value
error-enclose-not-applicable = { $line } has no enclosure: it takes a real value without an uncertainty whose operations all have a proven enclosure
error-enclosure-call-not-applicable = this takes no enclosure: it must be a real value without an uncertainty whose operations all have a proven enclosure
error-enclosure-digits-not-whole = the number of significant digits must be a whole number from 1 to 5000
error-working-not-applicable = { $line } has no working: it takes a line with an operation whose result was computed
error-places-above-limit = { $places } places is above the limit of { $limit } places
error-significant-digits-out-of-range = { $digits } significant digits is outside 1 to { $limit }
error-enclosure-over-budget = the enclosure needs numbers wider than { $bits } bits and stopped there
error-digits-undetermined = place { $places } could not be decided: the value was refined to about { $digits } significant digits and the interval still straddles that place
error-unknown-label = { $line } does not name a line
error-plot-not-plottable = { $line } cannot be plotted: a picture takes an expression, naming or function line with a free variable
error-plot-view-count = { $found } views are given and the picture has { $expected }
error-plot-view-axis-count = { $found } axes are given and the picture has { $expected }
error-plot-unknown-unit = the unit of axis { $axis } is not a unit
error-plot-unit-of-other-dimension = the unit of axis { $axis } measures another quantity than the axis
error-plot-empty-interval = the range of axis { $axis } has an upper bound that is not above its lower bound
error-plot-unknown-parameter = { $name } is not a free variable of the line
error-plot-divisions-missing = axis { $axis } has no divisions
error-plot-sampling-failed = the picture could not be sampled ({ $code })
error-render-failed = the picture could not be drawn ({ $code })
error-render-text-does-not-fit = the picture's words do not fit an image of { $width } by { $height } pixels
error-image-not-encoded = the image could not be encoded ({ $code })
error-plot-view-kind-mismatch = { $line } has one axis variable, so it is drawn in a plane view and not in a space view
error-plot-iteration-limit-not-applicable = { $line } is not an escape-time line, so it takes no iteration limit
error-plot-parameters-not-applicable = { $line } is an escape-time line, so it takes no parameter values
error-plot-value-kind-not-plottable = { $line } has two axis variables and a complex value, which no picture shows yet
error-plot-too-many-axis-variables = { $line } has more than two axis variables; give the others a value with --param
error-plot-line-number-too-large = { $line } has a line number too large to name its picture
error-plot-sample-limit = the picture would need { $requested } samples, more than the limit of { $limit }
error-plot-mesh-too-large = the surface would need { $vertices } vertices, more than a picture can hold
error-plot-iteration-limit-too-large = { $iterations } iterations are more than the largest allowed limit
error-plot-not-lowerable = this expression cannot be drawn in machine arithmetic yet
error-plot-range-missing = axis { $axis } of view { $view } has no range, and a settled picture needs one
error-read-scene-not-complete = the picture of that generation is not finished, so nothing can be read from it
error-read-layer-has-no-readings = layer { $layer } offers no readings
error-read-coordinate-count = the layer takes { $expected } coordinates, and { $found } were given
error-read-coordinate-not-exact = coordinate { $axis } cannot be read as an exact value
error-read-needs-function-form = { $line } is not a function naming line, so a reading of it cannot be kept as a line
error-read-not-built = the reading could not be built from the picture

error-unknown-concept = there is no concept { $concept }
error-ambiguous-concept-name = { $name } names more than one concept: { $candidates }
error-concept-set-not-loaded = the concept set could not be loaded, so concepts are not available
error-plot-no-axis-left = every variable of { $line } is given a value, so the picture has no axis to draw over

error-array-shapes-differ = the two sides have different shapes, so they cannot be combined entry by entry
error-array-product-of-lists = a product of two lists is ambiguous: it could multiply matching entries, or be the scalar product, so calc does not choose one; a matrix product is written with matrices, as in [1, 2; 3, 4] * [1, 0; 0, 1]
error-array-body-and-point = a derivative with a list in what is differentiated and a list of points is ambiguous: it could pair each entry with the point beside it, or take every entry at every point, so calc does not choose one; write one point for the whole of it, as in diff([x, x^2], x, 2), or one entry for each point
error-array-not-rectangular = the rows of this matrix do not all have the same length
error-array-unsupported = this combination of a number and a list is not defined
error-array-not-square = a square matrix is needed here, with as many rows as columns
error-array-not-invertible = this matrix is not invertible: its rows are not linearly independent, so its determinant is zero
error-array-entries-not-exact = calc takes this of a matrix only where every entry is an exact number
error-array-entries-not-rational = calc takes the eigenvalues of a matrix only where every entry is a rational number
error-array-eigenvalues-not-real = this matrix has eigenvalues that are not real numbers; calc lists eigenvalues only when all of them are real
error-array-eigenvalues-not-radical = this matrix has an eigenvalue calc writes as rootof, and calc gives eigenvectors only where every eigenvalue is a rational number or a sum of square roots
error-array-empty = this list has no entries, so there is nothing to answer with
error-array-empty-transpose = this list has no entries, so there is nothing to transpose
error-array-empty-determinant = this list has no entries, so there is nothing to take a determinant of
error-array-not-a-matrix-transpose = transpose takes a matrix, and this is a list
error-array-not-a-matrix = a matrix is needed here, and this is a list
error-array-not-a-matrix-determinant = a determinant takes a matrix, and this is a list
error-array-position-not-whole = the position of an entry is a whole number
error-array-position-outside = there is no entry at that position
error-array-not-comparable = the entries could not all be compared exactly, so there is no middle one
error-array-not-ordered = calc could not decide which of { $first } and { $second } is larger, so it cannot order the list
error-array-not-real = { $entry } is not a real number, and the complex numbers have no order, so the list cannot be ordered
error-array-not-real-entry = an entry is not a real number, and the complex numbers have no order, so the list cannot be ordered
error-order-not-real = { $side } is not a real number, and the complex numbers have no order, so the comparison has no truth value; = and != are decided for complex numbers
error-array-entry-without-value = an entry of this list has no value, so the list cannot be ordered
error-sort-not-ordered = calc could not decide which of { $first } and { $second } is larger, so it cannot order the list; until then it made { $comparisons ->
    [one] { $comparisons } comparison
   *[other] { $comparisons } comparisons
}, this one included, and { $writes ->
    [one] { $writes } write
   *[other] { $writes } writes
}
error-array-records-need-key = the rows of a matrix are sorted by a key: write it as a function of the row, for example r |-> at(r, 1)
error-array-machine-entry = { $reading } is a machine number, and calc does not compute a list that holds one yet; compute it on a line of its own
error-array-machine-entry-plain = an entry is a machine number, and calc does not compute a list that holds one yet; compute it on a line of its own
error-array-not-a-list = this is not a list or a matrix
error-limit-not-finite = the two sides do not approach one number here: the denominator becomes zero while the numerator does not
error-limit-not-finite-from-left = from the left the value passes every bound in size, so there is no number it approaches: the denominator becomes zero while the numerator does not
error-limit-not-finite-from-right = from the right the value passes every bound in size, so there is no number it approaches: the denominator becomes zero while the numerator does not
error-limit-not-a-rational-function = a limit is taken here only where the body is built from +, -, *, /, whole powers, sin, cos, tan, exp, ln and atan in the variable; calc does not know this one is continuous at the point and will not assume it
error-taylor-not-analytic = a Taylor polynomial is taken here only where the body is built from +, -, *, /, whole powers, sin, cos, tan, exp, ln and atan in the variable
error-taylor-order-not-whole = the fourth argument of taylor is the order, a whole number from 0
error-taylor-order-too-high = the order of a Taylor polynomial is at most 64 here
error-taylor-not-defined-at-point = the body or one of its derivatives is not defined at the point, so it has no Taylor polynomial there
error-root-not-a-polynomial = a root is taken here only where the body is a polynomial with rational coefficients in the variable, and this one is not
error-root-of-zero = every number is a root of the zero polynomial, so it has no k-th root
error-root-index-not-whole = the third argument of rootof counts the real roots from the smallest, so it is a whole number from 1
error-root-none-real = this polynomial has no real root
error-root-index-out-of-range = this polynomial has { $count } real roots, so the third argument of rootof runs from 1 to { $count }
error-limit-undecided = calc could not decide this limit
error-integrand-not-a-polynomial = an integral is taken here only where the integrand is a polynomial or a quotient of two polynomials with rational coefficients in the variable, and this one is not
error-integrand-pole-in-interval = the integrand has a pole between the bounds, so this integral is not a number
error-integrand-high-degree-factor = the denominator has an irreducible factor of degree { $degree }, and calc integrates a quotient only where every such factor is of degree one or two
error-integrand-repeated-quadratic = the denominator has a repeated irreducible quadratic factor, which calc does not integrate yet
error-integral-bound-not-exact = the integrand has a pole, and calc places a pole against the bounds only where both bounds are exact numbers
error-integral-without-bounds = an integral needs a lower and an upper bound to give a number
error-array-units-differ = the entries of this list do not all carry the same unit
error-array-ranks-differ = one side is a list and the other a matrix; calc multiplies two matrices or a number and either, and does not treat a list as a one-row matrix
error-array-nested = a list whose entries are themselves lists is accepted by the input language and calc does not compute it yet; a matrix is written with its rows separated by semicolons, as [2, 1; 1, 3]
error-array-power = calc does not raise a list or a matrix to a power yet
error-parse-chemistry-empty = the chemical formula is empty, at column { $column }
error-parse-chemistry-unknown-element = { $name } is not an element symbol, at column { $column }
error-parse-chemistry-wrong-case = { $name } is not an element symbol as written; symbols start with a capital, as Co for cobalt, and CO is carbon and oxygen, so calc does not guess, at column { $column }
error-parse-chemistry-ambiguous-charge = { $name } does not say which digits count atoms and which count the charge; write the charge after ^, as NH4^+ for ammonium or Fe^3+ for iron with charge 3+, at column { $column }
error-parse-chemistry-sign-before-number = { $name } puts a sign before a number, which is a charge written sign first or a nuclide's mass number; write a charge as Fe^3+, and a nuclide in nuc'…', as nuc'He-4', at column { $column }
error-parse-chemistry-attached-number = { $name } starts with a number attached to it, which is a mass number or a coefficient; write 2 H2O with a space inside a reaction, and isotopes are not read yet, at column { $column }
error-parse-chemistry-oxidation-state = { $name } reads as an oxidation state or as the element symbols its letters spell; oxidation states are not read yet, at column { $column }
error-parse-chemistry-sum-without-spaces = { $name } has a sign between two species; write a sum with spaces, as H2 + O2, or a charge after ^, at column { $column }
error-parse-chemistry-dot-separator = { $name } holds a dot, which is not a hydrate dot here; write · or *, as CuSO4·5H2O, at column { $column }
error-parse-chemistry-decimal-count = { $name } is either a hydrate written with a dot or a count that is not whole; write a hydrate with · or *, as CuSO4·5H2O, and counts that are not whole are not read yet, at column { $column }
error-parse-chemistry-resonance = { $name } marks resonance, not a reaction; write a reaction with ->, at column { $column }
error-parse-chemistry-nuclide = { $name } is a nuclide, which chem'…' does not read; write nuclides and nuclear reactions in nuc'…', as nuc'^14C', at column { $column }
error-parse-chemistry-equilibrium = { $name } marks an equilibrium, which is not read yet; write a reaction with ->, at column { $column }
error-parse-chemistry-fractional-coefficient = { $name } is a coefficient that is not whole, which is not read yet; write whole coefficients, at column { $column }
error-parse-chemistry-two-arrows = a reaction has one arrow, and { $name } is a second one, at column { $column }
error-parse-chemistry-empty-side = a reaction needs species on both sides of its arrow, at column { $column }
error-parse-chemistry-unclosed-bracket = a bracket in the chemical formula is not closed, at column { $column }
error-parse-chemistry-unexpected-character = { $name } is not part of a chemical formula, at column { $column }
error-parse-chemistry-coefficient-without-reaction = { $name } is a coefficient, and a coefficient belongs to a reaction; a substance alone is written without one, as chem'H2O', at column { $column }
error-chemistry-reaction-alone = { $reading } is a reaction, and a reaction stands on a line of its own
error-chemistry-substance-not-a-number = { $reading } is a substance, not a number; ask for one of its properties, as molar_mass({ $reading })
error-chemistry-too-many-species = { $reading } has no single balance: its species allow { $count } independent reactions, and the smallest are listed only for at most { $limit } species
error-chemistry-empty = { $reading } holds no species
error-chemistry-element-not-conserved = { $reading } does not balance: { $element } counts { $reactants } on the left and { $products } on the right
error-chemistry-charge-not-conserved = { $reading } does not balance: the charge is { $reactants } on the left and { $products } on the right
error-chemistry-not-unique = { $reading } has no single balance: its species allow { $count } independent reactions, and the smallest are { $reactions }
error-chemistry-impossible = { $reading } cannot be balanced: no coefficients but zero conserve every element and the charge
error-chemistry-takes-no-part = { $species } cannot take part in { $reading }: every balance gives it the coefficient 0
error-chemistry-other-side = { $species } can take part in { $reading } only on the other side of the arrow
error-chemistry-molar-mass-needs-a-substance = { $reading } needs a substance written as chem'…', as molar_mass(chem'H2O')
error-chemistry-no-standard-atomic-weight = { $element } has no standard atomic weight, so { $reading } has no molar mass here
error-chemistry-molar-mass-of-an-ion = { $reading } asks for the molar mass of a charged species, and a molar mass is taken here only of a neutral substance
error-chemistry-molar-mass-of-no-element = { $reading } holds no element
error-parse-chemistry-wrong-arrow = { $name } is not a reaction arrow; write a reaction with ->, at column { $column }
error-parse-chemistry-no-arrow = a sum of species needs an arrow to be a reaction; write reactants -> products, at column { $column }
error-parse-chemistry-repeated-species = { $name } is written more than once in the reaction; write each species once, at column { $column }
error-parse-chemistry-zero-count = { $name } is a count of 0 or one that starts with 0; if the letter O was meant, write O, as in HO2, at column { $column }
error-parse-chemistry-zero-coefficient = { $name } is a coefficient of 0 or one that starts with 0; leave out a species that takes no part, and write a coefficient without a leading 0, at column { $column }
error-parse-chemistry-zero-charge = { $name } is a charge of 0; a neutral species is written without a charge, as in Fe, at column { $column }
error-parse-chemistry-count-too-large = { $name } makes a number above 9223372036854775807, and calc reads no larger number in a chemical formula, at column { $column }
error-parse-chemistry-isotope = { $name } is an isotope symbol, deuterium or tritium, and isotopes are not read yet, at column { $column }
error-parse-chemistry-dangling-hydrate-dot = { $name } is a hydrate dot without a constituent on each side; write both parts, as CuSO4·5H2O, at column { $column }
error-parse-chemistry-coefficient-outside-literal = a number before { $name } is a coefficient outside the formula; a coefficient belongs inside a reaction, as chem'2 H2 + O2 -> 2 H2O', at column { $column }
error-chemistry-no-reaction-with-these-sides = { $reading } has no reaction with these sides: its species allow { $count } independent balances, and none has every coefficient on the side it is written on
error-temperature-difference-as-reading = { $written } is a temperature difference, not a reading; for the reading of { $number } { $unit } write { $reading }
error-below-absolute-zero = { $reading } is below absolute zero, { $limit }
error-parse-nuclear-empty = the nuclear reaction is empty, at column { $column }
error-parse-nuclear-unknown-species = { $name } is not a nuclide or a particle calc reads; write a nuclide as ^14C or C-14, and a particle as n, p, d, t, α, γ, e-, e+, ν_e or ν̄_e, at column { $column }
error-parse-nuclear-attached-number = { $name } starts with a number attached to it, which is a mass number or a coefficient; write the nuclide as ^14C or C-14, and a coefficient with a space, as 3 n, at column { $column }
error-parse-nuclear-atom-symbol = { $name } is the symbol of an atom, deuterium or tritium, and a nuclear reaction counts nuclei; write d or ^2H, or t or ^3H, at column { $column }
error-parse-nuclear-bare-element = { $name } is an element, not a nuclide; write its mass number, as ^14C or C-14, at column { $column }
error-parse-nuclear-particle-or-element = { $name } is an element, or the neutron or proton written as a capital, and calc does not guess between them; write n for the neutron, p for the proton, or the element with its mass number, as ^14N, at column { $column }
error-parse-nuclear-school-letter = { $name } is a single letter calc does not read as a particle; write ^3He for the helion, γ for a photon and α for an α particle, at column { $column }
error-parse-nuclear-unsigned-electron = { $name } does not say whether it is an electron or a positron; write e- or e+, or ^0_-1e or ^0_+1e, at column { $column }
error-parse-nuclear-neutrino-flavour = { $name } names no neutrino flavour; write ν_e or nu_e for the electron neutrino, and ν̄_e or anti_nu_e for the electron antineutrino, at column { $column }
error-parse-nuclear-charge = { $name } carries a charge mark, and a nuclear reaction reads nuclei and particles without one; ions are not read here, at column { $column }
error-parse-nuclear-mass-below-charge = { $name } has a mass number of 0 or one below its atomic number, and no such nucleus exists, at column { $column }
error-parse-nuclear-numbers-disagree = the numbers written with { $name } do not belong to it: a nuclide's atomic number is that of its symbol, an electron is ^0_-1e, a positron ^0_+1e, a neutron ^1_0n and a proton ^1_1p, at column { $column }
error-parse-nuclear-atomic-number-disagrees = { $name } writes the atomic number { $written }, and { $element } has the atomic number { $expected }; write { $expected } below the mass number, or leave the atomic number out, at column { $column }
error-parse-nuclear-isomer = { $name } is a nuclear isomer, and isomers are not read yet, at column { $column }
error-parse-nuclear-other-lepton = { $name } is a muon, a tau lepton or one of their neutrinos, and only the electron family is read yet, at column { $column }
error-parse-nuclear-compact-form = { $name } is written in the compact form target(in,out)product, which is not read yet; write the same reaction with an arrow, as ^14N + α -> ^17O + p, at column { $column }
error-parse-nuclear-element-name = { $name } names an element in words, which is not read; write its symbol, as C-14, at column { $column }
error-parse-nuclear-resonance = { $name } marks resonance, not a reaction; write a reaction with ->, at column { $column }
error-parse-nuclear-equilibrium = { $name } marks an equilibrium, which is not read; write a reaction with ->, at column { $column }
error-parse-nuclear-wrong-arrow = { $name } is not a reaction arrow; write a reaction with ->, at column { $column }
error-parse-nuclear-no-arrow = a sum of particles needs an arrow to be a reaction; write reactants -> products, at column { $column }
error-parse-nuclear-two-arrows = a reaction has one arrow, and { $name } is a second one, at column { $column }
error-parse-nuclear-empty-side = a reaction needs particles on both sides of its arrow, at column { $column }
error-parse-nuclear-zero-coefficient = { $name } is a coefficient of 0 or one that starts with 0; leave out a particle that takes no part, and write a coefficient without a leading 0, at column { $column }
error-parse-nuclear-fractional-coefficient = { $name } is a coefficient that is not whole, which is not read; write whole coefficients, at column { $column }
error-parse-nuclear-coefficient-without-reaction = { $name } is a coefficient, and a coefficient belongs to a reaction; a nuclide alone is written without one, as nuc'^14C', at column { $column }
error-parse-nuclear-number-too-large = { $name } makes a number above 9223372036854775807, and calc reads no larger number in a nuclear reaction, at column { $column }
error-parse-nuclear-coefficient-outside-literal = a number before { $name } is a coefficient outside the reaction; a coefficient belongs inside it, as nuc'4 p -> ^4He + 2 e+ + 2 ν_e', at column { $column }
error-nuclear-reaction-alone = { $reading } is a reaction, and a reaction stands on a line of its own
error-nuclear-nuclide-not-a-number = { $reading } is a nuclide, not a number; ask for the Q value of a reaction, as q_value(nuc'^2H + ^3H -> ^4He + n')
error-nuclear-empty = { $reading } holds no particle other than photons
error-nuclear-mass-number-not-conserved = { $reading } does not balance: the mass number is { $reactants } on the left and { $products } on the right
error-nuclear-charge-not-conserved = { $reading } does not balance: the charge is { $reactants } on the left and { $products } on the right
error-nuclear-lepton-number-not-conserved = { $reading } does not balance: mass number and charge do, but the electron lepton number is { $reactants } on the left and { $products } on the right, and { $neutrinos } on the right would balance it
error-nuclear-q-value-needs-a-reaction = { $reading } needs a nuclear reaction written as nuc'…', as q_value(nuc'^2H + ^3H -> ^4He + n')
error-nuclear-q-value-does-not-balance = { $reading } has no Q value here, because its reaction does not balance; the reaction on a line of its own says why
error-nuclear-estimated-mass = { $nuclide } has only an estimated mass in AME2020, taken from trends of the mass surface, so { $reading } has no Q value here
error-nuclear-no-mass = { $nuclide } has no mass in AME2020, so { $reading } has no Q value here
error-nuclear-mass-number-not-conserved-balance = { $reading } does not balance: the mass number is { $reactants } on the left and { $products } on the right; as arithmetic only, the one set of coefficients that balances these particles is { $balance }, which does not say that such a reaction happens
error-nuclear-charge-not-conserved-balance = { $reading } does not balance: the charge is { $reactants } on the left and { $products } on the right; as arithmetic only, the one set of coefficients that balances these particles is { $balance }, which does not say that such a reaction happens
error-nuclear-lepton-number-not-conserved-balance = { $reading } does not balance: mass number and charge do, but the electron lepton number is { $reactants } on the left and { $products } on the right, and { $neutrinos } on the right would balance it; as arithmetic only, the one set of coefficients that balances these particles is { $balance }, which does not say that such a reaction happens
error-kind-mismatch = { $reading }: one side is { $left ->
    [1] frequency
    [2] activity
    [3] absorbed dose
    [4] equivalent dose
    [5] luminous intensity
   *[6] luminous flux
} and the other { $right ->
    [1] frequency
    [2] activity
    [3] absorbed dose
    [4] equivalent dose
    [5] luminous intensity
   *[6] luminous flux
}; they share a unit but are different quantities, so they cannot be added, compared or converted into each other
