# calc-syntax

## What it does

Parses the input language into expression nodes, and prints nodes back as text.

`parse_statement` reads one line and returns a `Statement`:

- `Naming` for `name = expression`, with the name interned as a `Variable` symbol.
- `FunctionNaming` for `f(x, y) = expression`, with `f` interned as a `Function` symbol of that arity. The value is one `Lambda` binder per parameter, outermost for the first parameter.
- `Expression` for everything else.

`parse_expression` reads an expression without a statement form. `print_expression` prints an expression in `PrintMode::Ascii` or `PrintMode::Unicode`. Parsing the printed text gives the same `ExprId` in both modes.

Parsing runs in three steps. The lexer makes tokens and records whether a space came before each one. The parser builds a syntax tree by the precedence table of the input language. The builder turns the tree into pool nodes: it resolves bound names to de Bruijn indices, looks up unit names in the pool's unit table, and interns symbols. Errors are `ParseError` values with a kind and a byte range. Printing errors are `PrintError` values. Neither carries text.

These are the readings of the input language where its definition leaves details open:

- Numeric literals are exact. An exponent with a magnitude above `LARGEST_DECIMAL_EXPONENT` (100000) is `ExponentTooLarge`.
- A minus folds into a numeric literal only when the literal follows it without a space, and when the literal is not followed by `^`, a superscript or `!`.
- A unit expression stands after a numeric literal, a typed literal, an uncertainty group or a grouping parenthesis, separated by a space. Any name in that position is a unit name, and a name that is not a unit is `NotAUnit`. A unit expression that resolves to the dimensionless unit adds no `Quantity` node.
- A numeral divided by a numeral that carries a unit or a percent sign, as in `1/2 kg`, `1/2 %` or `100 * 1/2 %`, is `FractionBeforeUnit`, and the error names both readings. So is any numerator without a unit, `(1+1)/2 kg` or `x/2 %`; a numerator that holds a unit is a quantity divided by a quantity, `5.5 km / 2 h`.
- Keyword arguments exist for `shape=left|halving` on `sum` and `product`, and `side=left|right|both` on `limit`. No built-in operator has keyword parameters yet, so any other keyword is `UnknownKeyword`.
- A binder variable may use the name of a constant, such as `i`, and shadows it inside the body. A naming statement may not use a reserved name.
- The mathematical binder forms `∫ body dx, lo..hi`, `Σ body, i=lo..hi`, `∏ body, k=lo..hi` and `lim body, x→a` take the rest of the expression up to the comma as their body, and their last bound extends over an additive expression. The body of `∫` ends at the first name `d<variable>` that is followed by a comma, a closing bracket or the end. `d/dx body` takes a body at the level of a prefix function application. Inside other expressions these forms need parentheses.
- Every operator that has no infix form is called by name: `sqrt`, `abs`, `exp`, `ln`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `floor`, `ceil`, `trunc`, `round_ties_even`, `copysign`, `min`, `max`, `mul_add`, `select`, `complex`, `to_f32`, `to_f64`, `exact`, `uncertain`, `uncertain_expanded`, `convert_unit`, and the temperature conversions `from_celsius`, `from_fahrenheit`, `to_celsius` and `to_fahrenheit`, which are reserved and print in call form in both modes, such as `from_celsius(20.5)`. `uncertain`, `uncertain_expanded` and `convert_unit` also exist so that every node can be printed, for example `convert_unit(x, y)` when the target is not a unit quantity.

Recognised attempts:

- A closed list, `RecognisedAttempt`, names input that the language rejects but whose intent is clear where it stands, each with its replacement as a token of the language:
  - `**` for `^`;
  - a comma between digits outside any parentheses or brackets for the decimal point, unless exactly three digits follow, which is a grouping comma with no replacement, because `1,000` may mean one thousand;
  - a pair of bars around a non-empty operand for `abs`, never a single bar and never a bar right after a digit, so the point `(2|5)` and divisibility `3|12` are not attempts;
  - `×` for `*`, and `÷` or a colon between two operands for `/`. A colon with one or two digits before it and exactly two after, such as `80:20` or `6:30`, may be a division or a time, so it is its own attempt with no replacement.
- When parsing fails and the error lies on such an attempt, the error becomes `RecognisedAttempt` with the attempt's byte range. A text that parses is never changed, and characters the language accepts, such as `·` and `−`, are never attempts. `corrected_text` writes the input with every attempt replaced, and returns it only when that text parses, so a message never suggests text that fails.
- Deliberately not listed, each with a test: `2x3` and `2 x 3`, where `x` may be a variable, a comma followed by a space, a point and divisibility.

Printing rules:

- The printer uses the fewest parentheses that keep the tree. Equations print with `==`, so a printed equation is never read as a naming statement.
- Exact integers print as digits. Six or more trailing zeros print as an exponent, as in `1e30`. Rationals print as a decimal when it is finite and as `q'n/d'` otherwise.
- `F32` and `F64` values print as typed literals with the shortest decimal that reads back to the same bits. The quiet NaN prints as `nan`, and any other NaN prints in the `bits:` form.
- The Unicode mode uses `·`, `±`, `≠`, `≤`, `≥`, `→`, `↦`, `π`, `∞`, `√` and superscript integers. Binders print in their canonical call form in both modes.
- A bound variable prints with the name recorded in the pool. A name that would capture a free symbol or an outer bound variable of the same spelling gets a suffix such as `x_1`.
- A `Quantity` with the dimensionless unit, and an array shape that the parser cannot produce, such as a one-row matrix, are `PrintError`s.

Answer notations:

- `AnswerNotation` holds the seven notation modes as typed values. `NotationValues` collects mode values by their value names, as a curriculum file writes them, and `notation` turns them into an `AnswerNotation`, reporting a missing mode or an unknown value name. A set value is a list separated by `, ` without repeats.
- `AnswerNotation::check` returns the conflicts between notation modes. The parentheses mark with `letters_and_parentheses` is no conflict.
- `AnswerNotation::modes_reading_less_than` returns the modes in which a later notation reads less than an earlier one, for the curriculum check: the decimal separator must stay equal, sign sets may only grow, `juxtaposition` and `mixed_numbers` may only move forward in their order, and `coordinates` and `recurring_mark` may only move from `none` to one value and then stay.
- `read_answer` does not exist yet.

The language reference:

- `operator_construct` gives the reference construct of one operator, so `calc-app` names a working step's operation by the same key the reference uses.
- `language_reference` builds the typed language reference from the same tables the lexer, the parser and the printer read: the token tables of `lexer`, the call, binder and reserved name tables of `names`, the precedence levels and the unit table of `calc-units`.
- An entry holds its group, its typed construct, every spelling with a symbol, a pattern, a mode, an example and the byte ranges of the operands in that example, the canonical form per print mode, its precedence and its arguments. It holds no user-facing text.
- Tests prove every claim: each example parses, every spelling of an entry gives one `ExprId`, each canonical form parses back to it, and every token character, call name, reserved word and binder name occurs in a spelling.

A `chem'…'` literal is read by its own grammar in `chemistry`, never by the expression language: element symbols in their proper case, counts, nested brackets, hydrates with `·` or `*`, states, charges after `^` or as a bare sign, the electron `e-`, and reactions with `->`. Every spelling with two readings is refused as a `ChemistryProblem` that the message names, and nuclide notation is reserved. The builder keeps the written text as character codes, so the printer repeats the literal as written.

A `nuc'…'` literal is read by its own grammar in `nuclear`: nuclides as `^14C`, `¹⁴C`, `C-14` or with the atomic number below (`^14_6C`), the particles n, p, d, t, α, γ, e-, e+, ν_e and ν̄_e in the spellings the grammar accepts, the school forms `^0_-1e` and `^1_0n` checked against the particle, and reactions with `->` and whole coefficients. Every form with two readings, and every form not read yet (isomers, muons and taus, the compact A(a,b)B form), is refused as a `NuclearProblem`. The builder stores each species as its `NuclearParticle` codes beside its canonical text. Inside `chem'…'` nuclide notation and `e^+` are refused with a message that names `nuc'…'`.

## How to test

`cargo test -p calc-syntax`

There is one test for each construct of the input language and each error kind the parser reports. One round-trip test parses a list of lines that covers every construct and every operator. It prints each expression in both modes, parses the text again and checks that the `ExprId` is unchanged. The round-trip test was checked by three mutations: removing the parentheses for a right operand of equal precedence, removing the parentheses around a negated literal, and folding a minus before `^`. Each made a test fail.
