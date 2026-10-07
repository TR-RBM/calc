# calc-expr

## What it does

Holds the expression representation.

`ExprPool` owns all expressions of a session: the nodes, the argument lists, the number table, the symbol table and one `UnitTable` from `calc-units`. Nodes are referred to by `ExprId`, numbers by `NumberId` and symbols by `SymbolId`. Each id is a `u32` newtype, is `Copy`, and has meaning only in the pool that issued it.

A node is one of seven kinds: `Number`, `Symbol`, `Bound`, `Apply`, `Bind`, `Quantity` and `Array`. Nodes are added through the constructor methods `number`, `symbol`, `bound`, `apply`, `bind`, `quantity` and `array`. Nodes are read through `node`, which returns the borrowed view `NodeView`. The stored layout is private.

Nodes are immutable and hash-consed. Adding a node equal to an existing one returns the existing id, so structural equality is id equality. No algebraic law is applied: `Add(Add(a, b), c)` and `Add(a, Add(b, c))` stay different. Numbers are interned by bit identity, so `-0.0` and `0.0` differ, and two NaNs with the same bits are one number.

Bound variables are de Bruijn indices, so alpha-equivalent binders share one id. The name a user wrote for a bound variable is recorded per `Bind` node with `record_bound_name`, and the first recorded name is kept.

`Operator` is the closed enum of built-in operators with a fixed arity. It includes `Uncertain`, `UncertainExpanded`, `ConvertUnit`, `Factorial`, `ToF32`, `ToF64` and `ToExact`. The temperature conversions `FromCelsius`, `FromFahrenheit`, `ToCelsius` and `ToFahrenheit` are unary. `apply` checks the argument count against the operator's arity, or against the arity of a `Function` symbol. `bind` checks the argument count of each binder kind: none for `Lambda`, two bounds for `Sum` and `Product`, none or two for `Integral`, and one point for `Limit` and `Derivative`. `array` checks that the element count equals the product of the shape.

The symbol table maps one name to one symbol, with kind `Variable`, `Constant` or `Function` with arity. `pi`, `e`, `i` and `inf` are pre-registered constants with fixed ids, given by `BuiltinConstant`. Interning a known name with another kind is `SymbolError::KindConflict`.

`compact` consumes the pool and returns a new pool with only the nodes reachable from the given roots, and a `CompactionMap` from old to new expression ids. Symbol ids and unit ids stay the same. Number ids are renumbered. Bound names of reachable binders are kept.

Every table holds at most `u32::MAX` entries. Adding past that is `TableFull`. An id that the pool did not issue is an `Unknown...Id` error.

`substitute_symbols` replaces symbols by expressions in one simultaneous step and rebuilds the nodes above them, keeping the written shape and the recorded bound names. Every replacement must be closed, meaning it has no bound variable that points outside it, which `is_closed` checks. A closed replacement needs no index shifting under a binder. Substituting expressions with loose bound variables, and index shifting, are not implemented yet.

`bound_by_name` binds, under every binder whose bound name is recorded, each free variable symbol of that name in the binder's body, so that a name put into a line after it was parsed is the name a binder in that line binds.

`open_lambda_chain` opens the leading `Bind(Lambda)` binders of a function: each bound variable becomes a `Variable` symbol named by the binder's recorded name, outermost first, with `_1`, `_2`, … added when the name clashes with a free symbol of the body or a symbol of another kind. `apply_lambda` substitutes closed arguments for the leading binders, one per argument, shifting an argument's loose bound indices past the binders of the body it is placed under, and reports a wrong argument count as a typed `LambdaError`.

## How to test

`cargo test -p calc-expr`

There is one test per pool behaviour: hash-consing, kept written shape, number identity by bits, symbol interning and conflicts, arity and binder checks, arrays, quantities, errors for unknown ids and full tables, and compaction. The tests for hash-consing and arity were each checked by changing the code so that the behaviour breaks and confirming that a test fails.
