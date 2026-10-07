# calc-concepts

## What it does

Loads the content under `content/`, validates it, and hands the ways to `calc-core` as a `RuleSet`.

The build script lists every file under `content/` and embeds it with `include_bytes!`. `load_embedded` loads those files, and `load` loads any list of `(path, bytes)` pairs, which the tests use. The crate reads nothing at run time.

`load` returns a `ConceptSet`:

- `concepts`: one `ConceptNode` per `concepts/<concept>/node.md`, with its level, prerequisites, corpus blocks it rests on, curriculum references, sources, exercises, ways and bounds. Its localized texts come from `concepts/<concept>/<locale>.md` and are keyed by locale. A rounded number in those texts is written `{<expression> | <places>}` and loads as `≈` and the value rounded half away from zero to that many places, with the decimal mark of `notation/<locale>.md`. A typed `≈` or typed decimal outside braces fails the load.
- `objects`: one `ObjectKind` per `objects/<object>.md`. Each role has its identifier `object.role`, its dimension and its quantity kind. Localized names come from `objects/<object>.<locale>.md` in two forms: the heading form, which is the file's title for the object and `Name` for a role, and the running form for use inside a sentence, `Running` before the first section for the object and in each role section. Both are required.
- `sources`: one `Source` per `sources/<source>.md`. `Source::tier` derives the licence tier from the licence:
  - `bundle` for `CC0-`, `CC-BY-<version>`, `CC-BY-SA-` and `CC-PDDC`;
  - `fetch` for every `CC-BY-NC` and `CC-BY-ND` licence;
  - `link` for everything else, including `unknown`.
- `curricula`: one `Curriculum` per `curricula/<curriculum>/curriculum.md`, with its locale, documents, ordered groups with their concepts, its own prerequisite edges and its notation versions as `AnswerNotation` values of `calc-syntax`. `shipped.md` beside it gives the shipped notation versions, and `<locale>.md` the curriculum's name and localized group and concept names. `Curriculum::concept_order` lists the concepts by group, then in the order of each group.
- `unit_systems`: one `UnitSystem` per `unit-systems/<system>.md`, with its `## Unit <kind>` declarations and its names from `unit-systems/<system>.<locale>.md`, whose title is the name in that locale. English is required.
- `retired`: the identifiers listed in `retired.md`, one per line under the title `# retired`.
- `version`: the concept set version. It is FNV-1a over the files in path order, each file contributing its path, a zero byte, its bytes and a zero byte.

Every file follows a strict form, and loading fails on anything else. The `LoadError` names the file and the line, or line 0 for an error about the whole set. Loading checks:

- The form: title, `Key: value` lines, `## Kind name` sections, `calc` blocks. It rejects unknown keys, duplicate keys and sections, and text outside the form.
- Required keys and values.
- Paths: titles match file names, identifiers follow the identifier syntax, and corpus IDs have the block form.
- Uniqueness within each kind: sources, object kinds, concepts, and ways with bounds. A concept and an object kind may share a name, as `circle` does.
- No retired identifier appears again.
- References: prerequisites, sources, assumed concepts, and roles. The output and inputs of a way or bound belong to one object kind.
- Dimensions are unit expressions of `calc-units`, or `1`. Quantity kinds come from the closed list.
- Every formula, condition and bound expression parses in the input language as an anonymous function with one parameter per input.
- Every concept and object kind has English text.
- Learning prerequisites form an acyclic graph, checked with the `calc-core` graph module.
- A concept with no prerequisites is declared `Start: yes`, and a declared beginning has none, so a forgotten prerequisite fails the load instead of making a new beginning.
- `## Activity <name>` subsections hold `Kind` (`shape-matching`), `Shapes` (two or more of `circle`, `square`, `triangle`), `Varies` (`nothing`, `orientation`, `size`) and `Sources`, read into `ConceptNode::activities` in file order.

A curriculum file has the keys `Locale`, `Groups` and `Documents`, one `## Group <group>` with `Concepts` per group, `## Concept <concept>` with `Prerequisites` for the curriculum's own edges, and `## Notation <identifier> <version>` sections. Lowercase mode keys and `Source <mode>` are accepted inside `## Notation` sections only. A locale file has the curriculum's name as its title and `## Group` and `## Concept` sections with `Name`. Loading a curriculum checks:

- The locale form, group identifiers without repeats, a section for every group and no section for any other, every concept known and in exactly one group, and every document a known source.
- The prerequisite check: every prerequisite in a `## Concept` section is in the curriculum and comes earlier in the concept order. The node's own `Prerequisites` are checked for cycles above, not for order here.
- The notation check over all seven modes: at least one notation; all seven mode keys with value names; no conflict; a `Source <mode>` for every mode that names a known source, with `Sources` equal to the set of those; versions 1, 2, 3 without gaps and one `From` group per identifier; `From` a group of the curriculum, one identifier per group, and a notation for the first group; no later version and no later group's highest version reading less; and every section of `shipped.md` present in the curriculum with the same seven values. A source with the licence `unknown` counts as a recorded licence.
- Retired identifiers: neither the curriculum nor a notation identifier is retired.

A `## Unit <kind>` subsection, in a curriculum or a unit system, declares display units. `Displayed` is required. `Posed` is allowed in curricula only and is never a compound. Loading checks:

- The kind is a quantity kind of the closed list, which now includes `temperature` and `temperature-difference`.
- For `temperature`, the value is a scale name of `calc-units`: `kelvin`, `celsius` or `fahrenheit`.
- Otherwise the value is a unit expression that `calc-syntax` reads after a number as a unit, or `1` for a dimensionless kind, and its dimension is the kind's dimension.
- A compound, units separated by one space, is allowed only in `Displayed` for `time`, `length` and `mass`. Its parts each have the kind's dimension and descend strictly by scale factor, so a unit never repeats.
- `Displayed` is an ordered list of units separated by `, `, read into `UnitDeclaration::displayed` in its written order. Every entry passes the checks above. A list of more than one entry holds no scale and no compound, ascends strictly by scale factor, so a repeated scale factor such as `dm^3, L` is refused, and no two of its units differ by a power of π, because the order is checked exactly. Relative factors with other primes, such as the 12 and 5280 of `in, ft, mi`, are allowed; the pick in `calc-app` decides where a value terminates.
- In a curriculum, `## Unit <kind> <group>` stages a subsection: `from` holds the group, which must be a group of the curriculum, and a subsection without a group applies from the first group. Two subsections for one kind that apply from the same group are refused. A unit system has no stages.

`check_display_unit` runs the check of one `Displayed` entry outside a file, for the Units preference and the session override.

Recognition patterns:

- Every way has the required key `Recognized`, `yes` or `no`. A way with `yes` is a derived pattern: its formula is the pattern and its conditions are the guard.
- A `## Pattern <name>` section holds an explicit pattern: an optional `Relation`, a `calc` block with the pattern function, and an optional second block with the guard.
- Loading checks every pattern, derived or explicit: at least one parameter; parameter names that parse as free variables, so no constant, keyword, label or operator name, and no name twice; no free name in the body other than a constant; every parameter used; a body that is not just a variable; a guard with the same number of parameters; an explicit pattern's relation among the relations of the ways; and no explicit pattern equivalent to a recognized way of its concept.
- The equivalence for that last check follows the equivalence of expressions with variables renamed one to one: identical nodes, numbers and constants by identity, `Add` and `Mul` chains of equal length in any pairing, every other node with its arguments in order.
- `ConceptSet::patterns` builds a `RecognitionPattern` per derived and explicit pattern in the caller's pool, sorted by concept and identifier, with its variables, its function and its conditions.

`ConceptSet::rule_set` builds a `calc_core::RuleSet` in the caller's pool:

- quantities: every role;
- rules: every way, sorted by identifier, with the formula and conditions as lambda chains;
- the exact flag, derived from the formula;
- the sources.

Permutations add one renamed rule per renaming, with the identifier suffix `~<n>`. `ConceptSet::bounds` builds the bounds the same way.

A formula is exact when it uses only exact numbers, the constants `pi` and `e`, bound variables and the operators `+`, `-`, `*`, `/`, negation, power, square root, absolute value, minimum and maximum. So `2 * pi * r` and Heron's formula are exact, and a formula with `sin` or a machine number is not. This follows exact evaluation in `calc-core`, which keeps multiples of `pi` and square roots exact.

Formula parameters are positional. Ideally parameter names would match the inputs, but role names such as `side-a` are not identifiers of the input language, so the content names parameters `a`, `b`, `g`, and only the parameter count is checked.

Not supported yet:
- **Scene sections:** these are a load error until scenes need them.
- **Assumed concepts:** they are checked but not passed to `calc-core`, because `Rule` has no field for them.

## How to test

`cargo test -p calc-concepts`

Loader tests build small file sets in memory and change one thing per test, one test per rejected case. The curriculum tests use one fixture curriculum with two groups, one edge and two notations, and change one line per check. The shipped content test loads the embedded content files and compares the list of rule identifiers, one rule's inputs and exactness, the bound and the licence tiers with values written out in the test. It then runs `search_ways` over them. Four mutations each made a test fail:
- dropping the square root from the exact operators;
- skipping the parameter count check;
- allowing inputs from another object kind;
- making identifiers unique across kinds.

Three mutations of the pattern checks each made a test fail: letting a mapped variable pair with any other, skipping the chain length comparison, and skipping the unused variable check. Three mutations of the unit checks each made a test fail: letting equal scale factors descend, skipping the dimension check, and allowing compounds for every kind. Four mutations of the curriculum checks each made a test fail: allowing a prerequisite at the same position, allowing two notations from one group, comparing only the length of `Sources`, and skipping the comparison of a shipped version.
