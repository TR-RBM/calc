# calc-asm

## What it does

Reads x86-64 assembly and counts, per function, the instructions it runs and the memory reads and writes they make, exactly, as formulas in the values the function receives.

`parse` reads NASM, GCC's Intel syntax as `gcc -masm=intel -S` and Compiler Explorer write it, and the output of `objdump -d -M intel`. A function starts at a declared or unreachable-by-fall-through label, and in objdump output at each symbol and each address a `call` targets. `analyze` follows each function along every path. A branch decided by the entry values splits the answer into cases with conditions; a branch decided by data makes the count a range. A counted loop is solved exactly from its counter's start, step and bound, nested loops multiply, and a call to a function in the input is counted with the caller's argument values. What cannot be counted is a typed `Refusal`: an indirect jump, a jump out of the function, recursion, a `rep` prefix, a loop whose count depends on data, on an outer counter, on an unsigned comparison or on a second exit, or a counter that would wrap.

Loops as compilers write them are counted by residue class of the bound where a loop is unrolled or vectorised, with exact fractions in the formula; from the test in the middle of a rotated loop; twice where a second exit's condition never changes inside the loop; through two counters moving towards each other; and through counters an outer loop steps together. Register widths are kept for constants, and a formula that no longer fits its register becomes unknown. A path keeps the values it has excluded, and a refusal that claims a wrap or an endless loop is made only on a path calc is sure is taken.

The crate has no user-facing text and no workspace dependencies. `calc-app` maps its reports to messages, and `calc asm` shows them.

## How to test

`cargo test -p calc-asm`

The fixtures under `tests/fixtures/` are GCC's output for the functions in `funcs.c`, and the counts expected from them were measured by single-stepping each call. Every other expected count in `tests/counts.rs` was counted by hand from the listing it is written beside: GCC's `-O0` and `-O2` output for a sum, a data-dependent count, nested loops and `square`, a NASM counted loop, a count-down loop that wraps below one, a call with a known argument, and objdump output with a call between two functions.
