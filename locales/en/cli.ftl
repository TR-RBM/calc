cli-help-usage =
    Usage:
      calc <expression> | <file.calc> [<line>] [--digits <n> | --enclose <n>] [--json]
      calc - --begin | --enter <text> | --machine-line <line> <f64 or f32> | --solve <json> | --choose-wanted <line> <role>
      calc solve --json [--request <json>]
      calc plot <expression> | <file.calc> <line> --output <path> [picture options]
      calc read <file.calc> <line> --at <coordinates> [picture options] [--commit]
      calc concept <identifier or name> [--lens <area>] [--json]
      calc asm <file.s> | - [--given <register>=<n>]... [--function <name>]
      calc help language [--json]
      calc --help
cli-help-page =
    calc — a calculator that keeps a result exact as long as the mathematics allows

    Synopsis:
      calc <expression> [--digits <n> | --enclose <n> | --working [<path>]]
           [--json] [--locale <tag>]
           [--units <system>] [--unit <kind>=<unit>]... | [--coherent-units]
      calc <file.calc> [<line>] [--digits <n> | --enclose <n> | --inspect]
           [--json]
      calc - --begin
      calc - --enter <text>
      calc - --machine-line <line> <f64 or f32>
      calc - --solve <json> | --choose-wanted <line> <role>
      calc solve --json [--request <json>]
      calc plot <expression> | <file.calc> <line> --output <path>
           [--view <lower>..<upper> [unit]]... [--param <name>=<value>]...
           [--size <width>x<height>] [--limit <rule>] [--settle]
      calc read <file.calc> <line> --at <coordinates>
           [--layer <index>] [--commit] [picture options]
      calc concept <identifier or name> [--lens <area>] [--json]
      calc asm <file.s> | - [--given <register>=<n>]... [--function <name>]
      calc help language [--json]
      calc --complete -- <word>...

    Description:
      Without a subcommand calc evaluates one expression, or opens a session
      file and prints its lines. A result stays exact where the mathematics
      allows it; where it does not, calc names the machine format it used and
      the size of the rounding error.

      solve          for a program rather than for a person: it reads a JSON
                     request and answers in JSON which ways lead to a wanted
                     quantity, what is still missing, and what already follows
                     from the quantities you have.
      plot           draws the picture of an expression or of a line into a
                     PNG file.
      read           takes a reading of that picture at coordinates you type,
                     and can keep the reading as a new line of the session.
      concept        prints a concept, named either by its identifier or by
                     the name a row of the session shows you.
      asm            counts the instructions and memory accesses of x86-64
                     assembly, from NASM, from GCC's Intel syntax or from
                     objdump -d -M intel, exactly, as a formula in the
                     registers a function receives, and says why the code
                     does not determine the cycles.
      help language  prints every construct the input language accepts, with
                     an example of each.

    Options:
      --digits <n>      show n decimal places, with the remainder that makes
                        them exact
      --enclose <n>     show a proven interval to n significant digits
      --json            print in the JSON form of the session file, for a
                        program
      --locale <tag>    choose the language by BCP 47 tag
      --units <system>  show values in the units of this system: si or
                        us-customary
      --unit <kind>=<unit>
                        show one quantity kind in this unit, once per kind
      --coherent-units  show every value in its coherent unit
      --begin           write an empty session to standard output, to pipe into
                        the commands below
      --enter <text>    add a line with this text to the session and evaluate it
      --machine-line <line> <f64 or f32>
                        add a line that evaluates <line> in machine arithmetic
      --solve <json>    add a line that asks for the ways to a quantity
      --choose-wanted <line> <role>
                        turn a reachable line into a search for ways to that
                        quantity
      --request <json>  take the solve request as an argument instead of from
                        the input
      --output <path>   write the picture to this PNG file
      --view <lower>..<upper> [unit]
                        set the next axis of the picture in exact numbers
      --param <name>=<value>
                        give a free variable an exact value
      --size <width>x<height>
                        the image size in pixels, 800x600 by default
      --limit fixed:<n> | following:<base>,<per halving>,<cap>
                        the iteration limit of an escape-time picture
      --settle          keep the shown view and parameters in the session file
      --at <coordinates>
                        the coordinates to read, one per axis, comma separated
      --layer <index>   the layer to read, the first one by default
      --commit          keep the reading as a new line of the session file
      --how-it-ran      say how the backend ran each operation of this line
      --trace           show every step of a named sort, such as
                        insertion_sort
      --batch           read one expression per line from the input and
                        answer each, in order
      --terse           answer each line in one line: the value, its unit and
                        whether it is exact
      --find <name>=<from>..<to>
                        try every whole number in the range for that name and
                        name each one that makes the claim hold
      --counter         name the numbers of the range that make the claim
                        fail instead of the ones that make it hold
      --check           read each line as a claim such as x = y and say
                        whether it holds, exactly
      --identity        read each line as a claim about names and say whether
                        it holds for every value of them, by algebra, or
                        wherever the denominators it was written with are not
                        zero
      --expand          multiply out products and powers of sums and collect
                        the terms, exactly
      --factor          write each line's exact rational value as a product of
                        prime powers, and say which primes are proven
      --solve-for       read each line as an equation or inequality in one
                        name, or a list of equations, and give every solution,
                        exactly
      --ode <system>    enclose the solution of diff(x, t) = …; … with a
                        proven bound, given --initial and --at
      --initial <values>
                        the start of --ode, such as "t = 0 s; x = 1 m";
                        units carry through
      --at <times>      the times --ode encloses the solution at, such as
                        "t = 1 s, 10 s"
      --recognize       name the concepts in each line of a batch, which costs
                        far more than the arithmetic
      --replay          compute every line of a session file again and say
                        whether it still gives what was stored
      --inspect         show the record of one line, with what recognition did
                        and how complete its offer is
      --lens <area>     the lens a concept opens in: explore, learn, train
                        or read, explore by default
      --given <register>=<n>
                        count as if the register held this whole number on
                        entry, once per register
      --function <name> count only this function of the assembly
      --working [<path>]
                        show how the result was reached, step by step
      --version         show the version, the commit it was built from, the
                        target it runs on and the session file format
      --help            show this help
      --complete -- <word>...
                        list what may follow these words, one per line with
                        a description after a tab, for a shell's completion

    Exit status:
      0  the work was done: a value, a picture, a reading, a solve answer
         of ways, front, solved or reachable, or a claim that holds
      1  the work failed, and standard error says why; under --check and
         --identity, a claim calc proved false, or one that names something
         with no value
      2  the command line was not understood: an unknown flag, a missing
         argument, an argument that is not of its kind
      3  what was read could not be used: a solve request that is not valid,
         or a line that is not a claim calc can judge
      4  it was used and did not reach what it wanted: a solve request that
         says what is missing, an undecided claim, a --find with no match,
         or an --ode the method could not enclose
      5  under --complete only: file names fit at that place as well

    Examples:
      calc "1/3 + 1/6"
          1/2, exact: a fraction is answered as a fraction
      calc "0.1 + 0.2"
          0.3, exact, and the next line shows what a machine makes of it
      calc "to_f64(0.1 + 0.2)"
          0.30000000000000004, machine, with its rounding error
      calc "2/3" --digits 5
          0.66666 and the remainder 1/150000 that makes it exact
      calc "sqrt(2)" --enclose 12
          1.41421356237 and 1.41421356238, which the exact value lies between
      calc "200 + 15%"
          neither reading: it says the two this can mean and asks for one
      calc "3 kg * 9.81 m/s^2"
          29.43 N, exact: the unit is computed, not carried along
      calc "(9.81 +- 0.02) m/s^2 * 2 s"
          19.620 m/s ± 0.040 m/s, the uncertainty propagated
      calc "100 km / 1 h" --units us-customary
          (781250/12573) mi/h: a system changes the unit a value is
          shown in, never the value
      calc "integral(x^2, x, 0, 1)"
          1/3, exact
      calc --identity "(x+1)^2 = x^2 + 2*x + 1"
          holds for every value, which is a proof and not a sample of values
      calc --identity "sqrt(x^2) = x"
          not an identity: it fails at x = -1
      calc --check "2 + 2 = 4"
          holds, decided exactly, and undecided where calc cannot decide
      calc --find "n=1..200" --counter "n^2 > n"
          n = 1, the one number in the range that makes it fail
      calc --expand "(x+1)*(x-1)"
          x^2 - 1, and --solve-for "x^2 = 4" answers -2, 2
      calc --ode "diff(y, t) = -y" --initial "t = 0; y = 1" --at "t = 1"
          y at t = 1, between two decimals around exp(-1), with a proven bound
      calc --batch --json < lines.txt
          one JSON object per line, in the order the lines were read
      calc "100 km/h * 5 s" --json
          the line for a program, in coherent units
      calc plot "sin(x)" --output wave.png
          the curve as a PNG file, sampled at the width of the picture
      calc read session.calc r1 --at 0.5 --commit
          the value of that line at x = 0.5, kept as a new line
cli-error = error: { $detail }
cli-terse-other = { $state }
cli-terse-exact = { $value } exact
cli-terse-range = { $value } range, both ends exact
cli-terse-machine = { $value } machine
cli-record-digit-counts = { $counts }, in full with --json
cli-terse-elided = { $value } ({ $counts })
cli-terse-measured = { $value } ± { $uncertainty } { $kind }
cli-terse-failed = error { $detail }
cli-language-not-computed = {"    "}the input language accepts this, and calc does not compute it yet
cli-found = { $assignment }
cli-found-none = nothing in the range makes it hold
cli-found-no-counter = nothing in the range makes it fail
cli-found-none-decided = no value calc could decide in the range makes it hold
cli-found-no-counter-decided = no value calc could decide in the range makes it fail
cli-found-undecided = { $count ->
    [one] calc could not decide the claim at { $assignment }
   *[other] calc could not decide the claim at { $count } values, the first { $assignment }
}
cli-found-too-many = { $count } candidates is above the limit of { $limit }; narrow the ranges
cli-claim-definition = defines  { $claim }
cli-claim-holds = holds  { $claim }
cli-claim-fails = FAILS  { $claim }
cli-claim-undecided = undecided  { $claim }
cli-claim-holds-throughout = holds for every value of its ranges  { $claim }
cli-claim-fails-throughout = FAILS for every value of its ranges  { $claim }
cli-claim-holds-in-part = FAILS: holds for part of its ranges only  { $claim }
cli-claim-not-a-relation = not a relation  { $claim }
cli-claim-unknown-name = unknown name  { $claim }: { $detail }
cli-claim-refused = refused  { $detail }
cli-error-no-input = no expression or session file given
cli-error-unknown-option = unknown option { $option }
cli-error-missing-option-value = option { $option } needs a value
cli-error-unexpected-argument = unexpected argument { $argument }
cli-error-invalid-locale = { $value } is not a BCP 47 language tag
cli-error-invalid-count = option { $option } needs a whole number, not { $value }
cli-error-view-without-line = a view of a session file needs the line it shows
cli-error-option-given-twice = option { $option } is given more than once
cli-error-descending-range = the range { $value } runs downward, and a range is searched upward; write { $ascending }
cli-error-coherent-units-with-chosen-units = --coherent-units shows every value in its coherent unit and cannot be given with --units or --unit
cli-error-two-forms-of-one-line = a line is shown in one form at a time: --inspect, --digits, --enclose or --working
cli-error-instrument-not-applicable = { $option } does not apply to { $line }, which has { $options }
cli-instrument-none = no other form
cli-error-unknown-unit-system = { $value } is not a unit system; calc has { $systems }
cli-error-unit-needs-kind-and-unit = option --unit needs <kind>=<unit>, not { $value }
cli-error-unknown-quantity-kind = { $value } is not a kind that --unit can set; the kinds it can set are { $kinds }
cli-error-kind-given-twice = option --unit gives the unit of { $kind } more than once
cli-error-view-not-finished = the view finished without an answer
cli-error-output-failed = the output could not be written

cli-line-input = { $line }  { $input }
cli-record-row = {"  "}{ $label }  { $value }
cli-line-failed = {"  "}{ $detail }
cli-line-not-evaluated = {"  "}saved before it was computed
cli-quantity = { $value } { $unit }
cli-uncertainty = ± { $standard }, standard uncertainty, coverage factor k = { $coverage_factor }
cli-view-uncertainty = ± { $standard }
cli-view-uncertainty-cut = ± { $standard }, cut off at this place
cli-status = { $replay }  { $results }  { $running }  { $locale }  { $precision }  { $backend }
cli-status-replayed = { $replay }  { $results }  { $running }  { $differing }  { $locale }  { $precision }  { $backend }
cli-line-solve = {"  "}{ $state }
cli-line-picture = {"  "}a picture, drawn by plot
cli-line-defined = {"  "}defines a function for use in other lines
cli-line-orbit-escaped = {"  "}escaped after { $count } of at most { $limit } iterations, a count with no rounding guarantee
cli-line-orbit-count-missing = {"  "}escaped within { $limit } iterations, and the count of that escape was not kept
cli-line-orbit-inside = {"  "}inside the set, proven, with a limit of { $limit } iterations
cli-line-orbit-undecided = {"  "}undecided after { $limit } iterations
cli-plot-written = { $path }, { $width } by { $height } pixels
cli-plot-notice = {"  "}{ $notice }
cli-error-invalid-view = option --view needs <lower>..<upper> in exact numbers, not { $value }
cli-error-invalid-parameter = option --param needs <name>=<exact value>, not { $value }
cli-error-invalid-size = option --size needs <width>x<height> in pixels, not { $value }
cli-error-plot-needs-output = calc plot needs --output <path>
cli-error-plot-needs-line = calc plot of a session file needs the line to draw
cli-error-file-not-written = { $path } could not be written
cli-error-output-permission-denied = { $path } cannot be written: permission denied
cli-error-output-directory-missing = { $path } cannot be written: there is no such directory
cli-error-output-is-a-directory = { $path } cannot be written: it is a directory
cli-error-output-not-writable = { $path } cannot be written
cli-error-invalid-coordinate = { $value } is not an exact coordinate
cli-error-read-needs-coordinates = calc read needs --at with one coordinate per axis
cli-session-written = session written to { $path }
cli-error-settle-needs-session = --settle needs a session file to write the settled picture into
cli-error-invalid-limit = option --limit needs fixed:<iterations> or following:<base>,<per halving>,<cap> in whole numbers, not { $value }
cli-error-limit-given-twice = option --limit is given more than once
cli-reading = { $value } ≈ { $reading }
cli-language-entry = {"  "}{ $name } — { $meaning }
cli-language-entry-without-words = {"  "}{ $name }
cli-language-spelling = {"    "}{ $mode }  { $pattern }  as in { $example }
cli-language-precedence-left = {"    "}level { $level }, groups from the left
cli-language-precedence-right = {"    "}level { $level }, groups from the right
cli-language-precedence-none = {"    "}level { $level }, does not group
cli-language-arguments = {"    "}{ $least } to { $largest } arguments
cli-language-keyword = {"    "}keyword argument { $name }, a value of kind { $value }
cli-language-units = Units
cli-language-unit = {"  "}{ $symbol }, shown as { $display_symbol }
cli-language-prefixes = Prefixes
cli-language-prefix = {"  "}{ $symbol }, ten to the { $exponent }
cli-error-unknown-help-topic = unknown help topic { $topic }
cli-language-precedence-table = Precedence, from the strongest binding
cli-language-level-left = {"  "}level { $level }  { $name }, groups from the left
cli-language-level-right = {"  "}level { $level }  { $name }, groups from the right
cli-language-level-none = {"  "}level { $level }  { $name }, does not group
cli-language-unit-with-prefixes = {"  "}{ $symbol }, shown as { $display_symbol }, takes decimal prefixes
cli-modes-row = {"  "}{ $label }{"  "}{ $operations }
cli-modes-what-native-means = A native mode says the checks passed on this adapter at this start. It is not a claim that the operation is correct for every operand.
cli-modes-one-way = every operation the same way
cli-modes-no-backend = evaluated exactly, so no backend ran an operation
cli-ran-on-width = { $backend } ({ $width })
cli-modes-evaluation = { $width }: { $modes }
cli-modes-block-label = { $width } { $label }
cli-modes-sentence-row = {"  "}{ $text }
cli-recognized-running = still being recognized
cli-recognized-nothing = none in the concept set matches
cli-recognized-more = more
cli-recognized-list = { $head }, and { $last }
cli-recognized-unnamed = { $count ->
    [one] one concept this version does not know
   *[other] { $count } concepts this version does not know
}
cli-recognized-cut-short = no concept matched before the search was cut short
cli-offer-cut-short = the search was cut short before every concept was tried
cli-offer-whole = every concept was tried
cli-offer-not-said = this file does not say whether every concept was tried
cli-asm-heading = { $name }
cli-asm-row = {"  "}{ $label }{"  "}{ $value }
cli-asm-label-always = always
cli-asm-label-count = count
cli-asm-label-cycles = cycles
cli-asm-label-outside = not counted
cli-asm-label-assumes = assumes
cli-asm-counts = { $instructions } { $instruction_count ->
    [one] instruction
   *[other] instructions
}, { $reads } { $read_count ->
    [one] memory read
   *[other] memory reads
} and { $writes } { $write_count ->
    [one] memory write
   *[other] memory writes
}
cli-asm-counts-without-memory = { $instructions } { $instruction_count ->
    [one] instruction
   *[other] instructions
}; memory accesses are not counted, because an instruction calc has no row for touches memory
cli-asm-between = between { $low } and { $high }
cli-asm-input-argument = the value in { $name } on entry, the { $position ->
    [1] first
    [2] second
    [3] third
    [4] fourth
    [5] fifth
   *[other] sixth
} integer argument in the System V calling convention
cli-asm-input-register = the value in { $name } on entry
cli-asm-outside = calls to { $names }, which are not in the input
cli-asm-unresolved-call = an address the linker has not filled in yet (objdump -dr names it)
cli-asm-indirect-call = an address computed at run time
cli-asm-assumes-no-overflow = no arithmetic on a loop counter or its bound overflows its register
cli-asm-assumes-separate-stack = writes through a computed address do not change the function's own stack variables
cli-asm-cycles-x86-64 = not determined by the code: an x86-64 core runs instructions out of order and several at once, so its cycles depend on caches, branch prediction and the clock
cli-asm-place-line = line { $line }
cli-asm-indirect-jump = the jump at { $place } goes to an address computed at run time, which calc cannot follow
cli-asm-jump-outside = the jump at { $place } leaves the function for { $name }, which is not in the input
cli-asm-recursion = the call at { $place } calls { $name } while it is still running; calc does not count recursion
cli-asm-repeat-prefix = the instruction at { $place } repeats as often as rcx says; calc does not count a rep prefix yet
cli-asm-unsupported-jump = { $mnemonic } at { $place } is a jump calc does not follow yet
cli-asm-runs-past-end = the function runs past its last instruction
cli-asm-too-many-paths = the function has more than { $limit } paths through it
cli-asm-calls-too-deep = the call at { $place } to { $name } is more than { $limit } calls deep; calc stops following calls there
cli-asm-trap = on these inputs the function stops at { $mnemonic } in { $place }, which ends the program rather than returning, so there is no count of a finished call
cli-asm-leaves-without-returning = on these inputs the function leaves at { $name } in { $place } without returning to its caller, by an exception, a thread exit or a long jump, so there is no count of a finished call
cli-asm-data-range = the count varies with data in a way calc cannot bound
cli-asm-in-callee = in { $name }, called at { $place }: { $reason }
cli-asm-loop-second-exit = the loop ending at { $place } has a second way out, which calc does not count yet
cli-asm-loop-unconditional = the loop ending at { $place } has no condition at its end
cli-asm-loop-no-counter = calc finds no counter that the loop ending at { $place } steps and tests
cli-asm-loop-step = the loop ending at { $place } steps its counter by an amount that does not divide the distance to its bound
cli-asm-loop-unsigned = the loop ending at { $place } compares its counter without sign, which calc does not count yet
cli-asm-loop-data = how often the loop ending at { $place } runs depends on data in memory or on a value calc cannot follow
cli-asm-loop-outer-counter = how often the loop ending at { $place } runs depends on an outer loop's counter; that sum is not counted yet
cli-asm-loop-never-ends = the loop ending at { $place } does not end by its condition here
cli-asm-loop-wraps = the counter of the loop ending at { $place } wraps around before the loop ends here
cli-asm-loop-stack = the loop ending at { $place } moves the stack pointer, which calc does not count yet
cli-asm-loop-overlapping = the loops at { $place } overlap without one lying inside the other
cli-error-asm-needs-input = calc asm needs a file of assembly, or - to read it from the input
cli-error-asm-given = --given needs a register and a whole number, as edx=14, not { $value }
cli-error-asm-no-functions = { $path } holds no instructions calc can read
cli-error-asm-no-function = { $name } is not a function in { $path }
cli-concept-heading = { $identifier }  { $name }
cli-concept-row = {"  "}{ $label }{"  "}{ $value }
cli-concept-lens-empty = {"  "}{ $lens } has nothing to show for this concept yet
cli-error-concept-needs-identifier = calc concept needs a concept identifier or name
cli-error-unknown-lens = { $value } is not one of explore, learn, train or read
cli-error-invalid-path = option --working needs a path of step numbers separated by dots, not { $value }
cli-working-ask-for = ask for
cli-working-path = --working { $path }

cli-identity-holds-everywhere = holds for every value
cli-identity-fails-everywhere = fails for every value
cli-identity-not-everywhere = not an identity: the two sides are not equal as polynomials in their names
cli-identity-holds-only-instance = holds, and there is nothing in it to vary
cli-identity-fails-only-instance = fails, and there is nothing in it to vary
cli-identity-holds-with = holds, with { $substitution }
cli-identity-fails-with = fails, with { $substitution }
cli-identity-holds-where-defined = { $count ->
    [one] holds wherever { $denominators } is not zero
   *[other] holds wherever { $denominators } are not zero
}
cli-identity-exclusion-list = { $head } and { $last }
cli-identity-fails-where-defined = { $count ->
    [one] fails wherever { $denominators } is not zero
   *[other] fails wherever { $denominators } are not zero
}
cli-identity-undecided = undecided: calc could not bring the difference to zero by algebra alone
cli-identity-refuted = not an identity: it fails at { $witness }
cli-identity-undecided-searched = undecided: calc could not bring the difference to zero by algebra alone, and no whole number from { $from } to { $to } refutes it, which is evidence and not proof
cli-identity-undecided-searched-fractions = undecided: calc could not bring the difference to zero by algebra alone, and neither a whole number from { $from } to { $to } nor a fraction with a denominator from { $smallest } to { $largest } between -{ $bound } and { $bound } refutes it, which is evidence and not proof
cli-identity-undecided-stopped = undecided: calc could not bring the difference to zero by algebra alone, and the search for a counterexample stopped at { $witness }, where the claim could not be decided
cli-identity-undecided-not-searched = undecided: calc could not bring the difference to zero by algebra alone, and the search for a counterexample did not run, because the names take more assignments than the limit
cli-expanded = { $expression }
cli-not-algebraic = this expression is not a polynomial in its names, so it has no expanded form

cli-solved = { $solutions }
cli-solved-nothing = no number solves this equation
cli-solved-real-only = { $solutions }; its other solutions are not real numbers, and --solve-for does not list them yet
cli-solved-nothing-real = no real number solves this equation; its solutions are not real numbers, and --solve-for does not list them yet
cli-solved-root-of-note = rootof(p, x, k) is the k-th real root of p, counted from the smallest; --enclose gives its digits
cli-solved-inequality = { $intervals }
cli-solved-inequality-every = every number solves this inequality
cli-solved-inequality-none = no number solves this inequality
cli-inequality-or = or
cli-factored = { $factors }
cli-factor-probable = { $factor } is probably prime: calc proves a number prime only below 3317044064679887385961981
cli-factor-not-split = { $factor } is not prime, and calc did not find its factors
cli-factor-not-rational = calc factors an exact rational number or a polynomial in one name with rational coefficients, and this line is neither
cli-solved-system = { $values }
cli-solved-system-free = { $values }, for every value of { $free }
cli-solved-system-nothing = no numbers solve all of these equations at once
cli-solve-system-not-linear = calc solves a list of equations only where each is linear in its names
cli-solve-system-not-equations = calc solves a list only where every entry in it is an equation
cli-solve-no-name = this equation has no name to solve for
cli-solve-several-names = this equation holds more than one name ({ $names }); calc solves for one and does not choose which
cli-solve-every-number = every number solves this equation
cli-solve-degree-too-high = the degree of this equation is { $degree }, which is above what calc solves
cli-solve-radical-coefficients = this equation has a square root in a coefficient, and calc solves such an equation at degree 1, or at degree 2 where the discriminant is rational, and this one is of degree { $degree }
cli-solve-not-a-polynomial = calc solves an equation only where it is a polynomial in one name
cli-solve-constant-coefficients = calc solves an equation with constants in its coefficients only where they are built from rational numbers, square roots of rational numbers, pi and e by sums, products and quotients, and { $names } is not; it does not solve with such a coefficient yet
cli-solve-constant-coefficient-degree = with pi or e in its coefficients, calc solves an equation of degree 1, and this one is of degree { $degree }; it does not solve such equations yet
cli-solve-unproven-coefficient = a coefficient of this equation holds pi and e together, and its enclosure does not prove it nonzero, which calc needs before it solves; it does not solve such an equation yet
cli-solve-square-part-unknown = calc could not split { $number } into primes it can prove, so it cannot find the square root this equation needs and gives no answer rather than lose a root

cli-version =
    calc { $version } ({ $commit })
    target { $target }
    session file format { $format }
cli-claim-unreadable = unreadable  { $claim }: { $detail }

cli-complete-command-solve = answer a JSON request for the ways to a quantity
cli-complete-command-plot = draw the picture of an expression into a PNG file
cli-complete-command-read = take a reading of a picture at coordinates
cli-complete-command-concept = print a concept, by identifier or by name
cli-complete-command-asm = count the instructions of x86-64 assembly
cli-complete-command-help = print the input language
cli-complete-topic-language = every construct the input language accepts
cli-complete-json = print in JSON, for a program
cli-complete-locale = choose the language
cli-complete-help = show the help page
cli-complete-version = show the version
cli-complete-digits = show n decimal places, with the remainder
cli-complete-enclose = show a proven interval to n significant digits
cli-complete-working = show how the result was reached
cli-complete-inspect = show the record of one line
cli-complete-units = show values in the units of a system
cli-complete-unit = show one quantity kind in this unit
cli-complete-coherent-units = show every value in its coherent unit
cli-complete-how-it-ran = say how the backend ran each operation
cli-complete-trace = show every step of a named sort
cli-trace-heading = {"  "}steps
cli-trace-not-a-sort = --trace shows the steps of a named sort, such as insertion_sort, and this line is not one
cli-trace-step = {"  "}{ $number }  { $step }
cli-trace-at = { $value } at position { $position }
cli-trace-held = { $value } (held aside from position { $position })
cli-trace-keyed-at = { $value } (key { $key }) at position { $position }
cli-trace-keyed-held = { $value } (key { $key }, held aside from position { $position })
cli-trace-in-scratch = { $value } in scratch place { $position }
cli-trace-keyed-in-scratch = { $value } (key { $key }) in scratch place { $position }
cli-trace-compare = compare { $left } with { $right }: { $verdict }
cli-trace-smaller = { $value } is smaller
cli-trace-larger = { $value } is larger
cli-trace-equal = the keys are equal
cli-trace-undecided = calc could not decide
cli-trace-move = move { $value } from position { $from } to position { $to }
cli-trace-put = put { $value }, held aside from position { $from }, into position { $to }
cli-trace-to-scratch = copy { $value } from position { $from } into scratch place { $to }
cli-trace-from-scratch = copy { $value } from scratch place { $from } back into position { $to }
cli-trace-exchange = exchange { $left } with { $right }
cli-trace-tally = count { $value } in counter { $counter }
cli-trace-prefix-sum = add counter { $from } to counter { $to }
cli-trace-decrement = take 1 from counter { $counter }
cli-trace-draw = draw a pivot for positions { $from } to { $to }: position { $chosen }, after { $draws ->
    [one] 1 block
   *[other] { $draws } blocks
}
cli-trace-flip = flip the first { $length } entries
cli-trace-pass = insertion sort of the entries { $gap } apart
cli-trace-digit-pass = counting sort by digit { $place }, counted from the lowest, of each key less the least key, { $least }
cli-trace-digit-tally = count { $value } in counter { $counter }, the counter of digit { $digit }
cli-trace-bead-falls = { $value } drops a bead onto pole { $pole }
cli-trace-bead-read = the bead of pole { $pole } in row { $row } from the bottom is counted for that row
cli-trace-rebuild = write { $beads }, the beads of row { $row } from the bottom, at position { $position }
cli-trace-bitonic-merge = merge in blocks of { $block }, rising and falling in turn: compare entries { $distance } apart
cli-trace-shuffle = shuffle { $number }, by Fisher–Yates from the last position
cli-trace-shuffle-draw = draw a position from 1 to { $position } for position { $position }: position { $chosen }, after { $draws ->
    [one] 1 draw
   *[other] { $draws } draws
}
cli-complete-replay = compute every line of a session file again
cli-complete-batch = answer one expression per line of the input
cli-complete-recognize = name the concepts in each line
cli-complete-check = say whether each claim holds, exactly
cli-complete-terse = answer each line in one line
cli-complete-find = try every whole number of a range
cli-complete-counter = name the numbers that make the claim fail
cli-complete-identity = say whether a claim holds for every value
cli-complete-expand = multiply out and collect the terms
cli-complete-factor = write each value as a product of prime powers
cli-complete-solve-for = give every solution of an equation
cli-complete-begin = write an empty session
cli-complete-enter = add a line and evaluate it
cli-complete-machine-line = add a line in machine arithmetic
cli-complete-solve = add a line that asks for the ways to a quantity
cli-complete-choose-wanted = search for the ways to a line's quantity
cli-complete-request = take the request as an argument
cli-complete-output = write the picture to this PNG file
cli-complete-view = set the next axis in exact numbers
cli-complete-param = give a free variable an exact value
cli-complete-size = the image size in pixels
cli-complete-limit = the iteration limit of an escape-time picture
cli-complete-settle = keep the view and the parameters in the session file
cli-complete-at = the coordinates to read
cli-complete-layer = the layer to read
cli-complete-commit = keep the reading as a new line
cli-complete-given = count as if this whole number were in the register
cli-complete-function = count only this function
cli-complete-lens = the lens a concept opens in
cli-complete-format-f64 = binary floating point with 64 bits
cli-complete-format-f32 = binary floating point with 32 bits
cli-note-unknown-locale = calc has no language { $requested }, so it answers in { $answered }; it has { $shipped }
cli-claim-note = note: { $note }
cli-batch-line-refused = line { $number }, { $input }: { $detail }
cli-terse-exact-where = { $value } exact, { $condition }
cli-expanded-where = { $expression }, { $condition }
cli-relation-is-a-claim = { $reading } is a claim, not a value; calc decides it with --check: calc "{ $reading }" --check
cli-relation-about-free-names = { $reading } is a claim about { $names }, not a value; calc decides it for every value with --identity, looks for a counterexample with --find and solves it with --solve-for
cli-complete-ode = enclose the solution of an initial value problem with a proven bound
cli-complete-initial = the start of --ode: the time and the value of each name at it
cli-complete-ode-at = the times --ode encloses the solution at
cli-error-ode-without = --ode, --initial and --at are needed together, and { $option } is missing
cli-ode-time = { $name } = { $value }
cli-ode-at = at { $time }
cli-ode-between = between { $lower } and { $upper }
cli-ode-label-interval-bound = interval bound { $name }
cli-ode-interval-bound = { $width }, the width of the enclosure at { $time }, with rounding and every step before it included
cli-ode-whole-run = over the whole run
cli-ode-label-step-bound = step bound { $name }
cli-ode-step-bound = { $bound }, the largest truncation one step adds; it is not a bound on the solution
cli-ode-label-method = method
cli-ode-method = { $steps ->
    [one] Lohner-type interval Taylor series of order { $order }, one step; every enclosure above is proven
   *[other] Lohner-type interval Taylor series of order { $order }, { $steps } steps; every enclosure above is proven
}
cli-ode-unreadable = { $part } could not be read: { $detail }
cli-ode-not-an-assignment = { $part } is not an assignment such as t = 0 or x = 1 m
cli-ode-not-a-derivative = { $part } is not an equation diff(x, t) = …; --ode reads each part between semicolons as the derivative of one name by the time, set equal to its right side
cli-ode-time-names-differ = the equations take derivatives by { $first } and by { $second }; --ode takes every derivative by the same name
cli-ode-component-twice = { $name } has two equations; --ode takes one for each name
cli-ode-no-initial-value = --initial gives no value for { $name }
cli-ode-not-in-the-system = { $name } is neither the time nor a name with an equation, and --initial and --at give only those
cli-ode-no-times = --at gives no time to enclose the solution at
cli-ode-unknown-name = the right side of { $part } uses { $name }, which is neither the time nor a name with an equation
cli-ode-unit-mismatch = the right side of the equation for { $name } has the unit { $found }, and { $name } per { $time } has the unit { $wanted }, with the units given in --initial
cli-ode-time-unit-mismatch = { $part } is not a time in the unit the start was given in, or in one convertible to it
cli-ode-unit-with-offset = { $name } is given in a unit with an offset, such as °C; give it in a unit without one, such as K
cli-ode-not-a-real-number = { $part } does not give a real number calc can enclose
cli-ode-time-not-rational = { $part } is not an exact rational time; --ode steps between exact times, so a time is written as a whole number, a fraction or a decimal
cli-ode-unsupported = the right side uses { $part }, which --ode has no Taylor series for yet
cli-ode-division-by-zero = the right side divides by zero in { $part }
cli-ode-stop-before-start = a time in --at lies before the start; --ode encloses the solution forward in time only
cli-ode-not-enclosed = calc enclosed the solution up to { $time } and could enclose no step past it, even after { $halvings } halvings of its length; the solution may grow without bound there or leave the domain of its right side
cli-ode-too-many-steps = after { $steps } steps, the last of { $step }, the solution was enclosed only up to { $time }, short of { $stop }; it changes on a time scale much shorter than the interval, so the interval needs more steps than --ode takes yet
cli-ode-inconsistent = calc built a problem it cannot integrate; this is a fault in calc
cli-ode-rounded-time = { $time } (rounded down to { $digits } significant digits)
cli-ode-units-do-not-combine = the right side of the equation for { $name } combines quantities whose units do not fit together, such as a length added to a time, or a time inside exp
