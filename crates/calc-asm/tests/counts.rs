use std::collections::BTreeMap;

use calc_asm::{Case, Count, LoopProblem, Refusal, analyze, parse};

fn range(count: &Count) -> String {
    if count.low == count.high {
        count.low.clone()
    } else {
        format!("{}..{}", count.low, count.high)
    }
}

fn case_text(case: &Case) -> String {
    let conditions = case.conditions.join(", ");
    match &case.outcome {
        Ok(counts) => format!(
            "[{conditions}] {} / {} / {}",
            range(&counts.instructions),
            counts.reads.as_ref().map_or("?".to_owned(), range),
            counts.writes.as_ref().map_or("?".to_owned(), range),
        ),
        Err(refusal) => format!("[{conditions}] {refusal:?}"),
    }
}

fn cases(source: &str, function: &str, given: &[(&str, i128)]) -> Vec<String> {
    let given: BTreeMap<String, i128> = given
        .iter()
        .map(|(name, value)| ((*name).to_owned(), *value))
        .collect();
    let report = analyze(&parse(source), &given)
        .into_iter()
        .find(|report| report.name == function)
        .expect("the function is in the input");
    match report.outcome {
        Ok(cases) => cases.iter().map(case_text).collect(),
        Err(refusal) => vec![format!("{refusal:?}")],
    }
}

const SQUARE: &str = "\"square(int)\":
        push    rbp
        mov     rbp, rsp
        mov     DWORD PTR [rbp-4], edi
        mov     eax, DWORD PTR [rbp-4]
        imul    eax, eax
        pop     rbp
        ret
";

const NASM_SUM: &str = "bits 64
section .text
global total
total:
    xor     eax, eax
    xor     ecx, ecx
    test    esi, esi
    jle     .done
.next:
    add     eax, [rdi + rcx*4]
    inc     ecx
    cmp     ecx, esi
    jl      .next
.done:
    ret
";

const COUNT_DOWN: &str = "spin:
    mov     ecx, edi
.again:
    dec     ecx
    jnz     .again
    ret
caller:
    mov     edi, 5
    call    spin
    ret
";

const POINTER_LOOP: &str = "\"sum\":
\ttest\tesi, esi
\tjle\t.L6
\tmov\tesi, esi
\txor\teax, eax
\tlea\trdx, [rdi+rsi*4]
\t.p2align 4
.L5:
\tadd\teax, DWORD PTR [rdi]
\tadd\trdi, 4
\tcmp\trdi, rdx
\tjne\t.L5
\tret
.L6:
\txor\teax, eax
\tret
";

const COUNT_NEGATIVE: &str = r#""count_negative":
	push	rbp
	mov	rbp, rsp
	mov	QWORD PTR -24[rbp], rdi
	mov	DWORD PTR -28[rbp], esi
	mov	DWORD PTR -8[rbp], 0
	mov	DWORD PTR -4[rbp], 0
	jmp	.L8
.L10:
	mov	eax, DWORD PTR -4[rbp]
	cdqe
	lea	rdx, 0[0+rax*4]
	mov	rax, QWORD PTR -24[rbp]
	add	rax, rdx
	mov	eax, DWORD PTR [rax]
	test	eax, eax
	jns	.L9
	add	DWORD PTR -8[rbp], 1
.L9:
	add	DWORD PTR -4[rbp], 1
.L8:
	mov	eax, DWORD PTR -4[rbp]
	cmp	eax, DWORD PTR -28[rbp]
	jl	.L10
	mov	eax, DWORD PTR -8[rbp]
	pop	rbp
	ret
"#;

const FILL: &str = r#""fill":
	push	rbp
	mov	rbp, rsp
	mov	QWORD PTR -24[rbp], rdi
	mov	DWORD PTR -28[rbp], esi
	mov	DWORD PTR -32[rbp], edx
	mov	DWORD PTR -8[rbp], 0
	jmp	.L13
.L16:
	mov	DWORD PTR -4[rbp], 0
	jmp	.L14
.L15:
	mov	eax, DWORD PTR -8[rbp]
	imul	eax, DWORD PTR -32[rbp]
	mov	edx, eax
	mov	eax, DWORD PTR -4[rbp]
	add	eax, edx
	cdqe
	lea	rdx, 0[0+rax*4]
	mov	rax, QWORD PTR -24[rbp]
	add	rax, rdx
	mov	ecx, DWORD PTR -8[rbp]
	mov	edx, DWORD PTR -4[rbp]
	add	edx, ecx
	mov	DWORD PTR [rax], edx
	add	DWORD PTR -4[rbp], 1
.L14:
	mov	eax, DWORD PTR -4[rbp]
	cmp	eax, DWORD PTR -32[rbp]
	jl	.L15
	add	DWORD PTR -8[rbp], 1
.L13:
	mov	eax, DWORD PTR -8[rbp]
	cmp	eax, DWORD PTR -28[rbp]
	jl	.L16
	nop
	nop
	pop	rbp
	ret
"#;

const OBJDUMP: &str = "0000000000001120 <twice>:
    1120:\t8d 04 3f             \tlea    eax,[rdi+rdi*1]
    1123:\tc3                   \tret

0000000000001130 <quad>:
    1130:\te8 eb ff ff ff       \tcall   1120 <twice>
    1135:\t01 c0                \tadd    eax,eax
    1137:\tc3                   \tret
";

#[test]
fn a_function_without_branches_has_one_exact_count() {
    assert_eq!(cases(SQUARE, "square(int)", &[]), ["[] 7 / 3 / 2"]);
}

#[test]
fn a_counted_nasm_loop_is_a_formula_in_its_bound() {
    assert_eq!(
        cases(NASM_SUM, "total", &[]),
        [
            "[esi >= 1] 4 * esi + 5 / esi + 1 / 0",
            "[esi <= 0] 5 / 1 / 0"
        ]
    );
}

#[test]
fn a_given_register_turns_the_formula_into_a_number() {
    assert_eq!(cases(NASM_SUM, "total", &[("esi", 14)]), ["[] 61 / 15 / 0"]);
}

#[test]
fn a_count_down_loop_wraps_where_its_counter_starts_at_or_below_zero() {
    assert_eq!(
        cases(COUNT_DOWN, "spin", &[]),
        [
            "[edi <= 0] Loop(Line(5), Wraps)",
            "[edi >= 1] 2 * edi + 2 / 1 / 0"
        ]
    );
}

#[test]
fn a_call_with_a_known_argument_counts_the_callee_exactly() {
    assert_eq!(cases(COUNT_DOWN, "caller", &[]), ["[] 15 / 2 / 1"]);
}

#[test]
fn a_pointer_stepping_by_four_counts_its_elements() {
    assert_eq!(
        cases(POINTER_LOOP, "sum", &[]),
        [
            "[esi >= 1] 4 * esi + 6 / esi + 1 / 0",
            "[esi <= 0] 4 / 1 / 0"
        ]
    );
}

#[test]
fn a_branch_on_data_gives_a_range() {
    assert_eq!(
        cases(COUNT_NEGATIVE, "count_negative", &[]),
        [
            "[esi >= 1] 12 * esi + 13..13 * esi + 13 / 6 * esi + 5..7 * esi + 5 / esi + 5..2 * esi + 5",
            "[esi <= 0] 13 / 5 / 5"
        ]
    );
}

#[test]
fn nested_loops_multiply_their_bounds() {
    assert_eq!(
        cases(FILL, "fill", &[]),
        [
            "[edx <= 0, esi >= 1] 9 * esi + 14 / 5 * esi + 4 / 2 * esi + 5",
            "[esi <= 0] 14 / 4 / 5",
            "[edx >= 1, esi >= 1] 17 * edx * esi + 9 * esi + 14 / 9 * edx * esi + 5 * esi + 4 / 2 * edx * esi + 2 * esi + 5"
        ]
    );
}

#[test]
fn objdump_output_is_read_by_its_symbols_and_calls() {
    assert_eq!(cases(OBJDUMP, "quad", &[]), ["[] 5 / 2 / 1"]);
}

#[test]
fn an_indirect_jump_is_refused() {
    let source = "dispatch:\n    jmp rax\n";
    assert_eq!(
        cases(source, "dispatch", &[]),
        [format!(
            "{:?}",
            Refusal::IndirectJump(calc_asm::Place::Line(2))
        )]
    );
}

#[test]
fn a_repeat_prefix_is_refused() {
    let source = "copy:\n    mov rcx, rdx\n    rep movsb\n    ret\n";
    assert_eq!(
        cases(source, "copy", &[]),
        [format!(
            "{:?}",
            Refusal::RepeatPrefix(calc_asm::Place::Line(3))
        )]
    );
}

#[test]
fn a_call_outside_the_input_is_named() {
    let source = "greet:\n    call puts\n    ret\n";
    let report = analyze(&parse(source), &BTreeMap::new()).remove(0);
    assert_eq!(report.outside, ["puts"]);
}

#[test]
fn a_loop_whose_count_depends_on_memory_is_refused_as_such() {
    let source = "walk:\n.again:\n    mov rdi, [rdi]\n    test rdi, rdi\n    jne .again\n    ret\n";
    assert_eq!(
        cases(source, "walk", &[]),
        [format!(
            "{:?}",
            Refusal::Loop(calc_asm::Place::Line(5), LoopProblem::DependsOnData)
        )]
    );
}

fn instructions_at(source: &str, function: &str, given: &[(&str, i128)]) -> String {
    let lines = cases(source, function, given);
    assert_eq!(lines.len(), 1, "{lines:?}");
    lines[0]
        .trim_start_matches("[] ")
        .split(" / ")
        .next()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn a_vectorised_loop_matches_the_measured_counts() {
    let source = include_str!("fixtures/sum_O3.txt");
    for (n, measured) in [
        (0, "4"),
        (1, "14"),
        (2, "18"),
        (3, "19"),
        (4, "29"),
        (5, "34"),
        (13, "44"),
        (100, "149"),
    ] {
        assert_eq!(
            instructions_at(source, "sum", &[("esi", n)]),
            measured,
            "n = {n}"
        );
    }
}

#[test]
fn a_vectorised_loop_is_a_formula_per_residue_of_its_bound() {
    let source = include_str!("fixtures/sum_O3.txt");
    let lines = cases(source, "sum", &[]);
    assert!(
        lines.contains(
            &"[esi >= 5, esi mod 4 = 1] 5/4 * esi + 111/4 / 1/4 * esi + 7/4 / 0".to_owned()
        ),
        "{lines:?}"
    );
}

#[test]
fn an_unswitched_nested_loop_matches_the_measured_counts() {
    let source = include_str!("fixtures/fill_O2.txt");
    for (m, n, measured) in [(3, 4, "101"), (5, 0, "44"), (0, 5, "6"), (4, 1, "71")] {
        assert_eq!(
            instructions_at(source, "fill", &[("esi", m), ("edx", n)]),
            measured,
            "m = {m}, n = {n}"
        );
    }
}

#[test]
fn an_unrolled_count_down_splits_by_parity() {
    let source = include_str!("fixtures/count_down_O2.txt");
    assert_eq!(
        cases(source, "count_down", &[]),
        [
            "[edi >= 3, edi mod 2 = 1] 2 * edi + 11 / 1 / 0",
            "[edi = 1] 12 / 1 / 0",
            "[edi >= 2, edi mod 2 = 0] 2 * edi + 8 / 1 / 0",
            "[edi <= 0] 5 / 1 / 0"
        ]
    );
}

#[test]
fn two_counters_meeting_in_the_middle_are_counted() {
    let source = include_str!("fixtures/reverse_O0.txt");
    for (n, measured) in [(0, "16"), (1, "16"), (2, "47"), (5, "78"), (6, "109")] {
        assert_eq!(
            instructions_at(source, "reverse", &[("esi", n)]),
            measured,
            "n = {n}"
        );
    }
}

#[test]
fn a_division_by_a_magic_multiplication_is_exact_for_a_given_value() {
    let source = include_str!("fixtures/sum_step_O3.txt");
    for (n, measured) in [(1, "14"), (9, "19"), (10, "41"), (13, "47"), (100, "114")] {
        assert_eq!(
            instructions_at(source, "sum_step", &[("esi", n)]),
            measured,
            "n = {n}"
        );
    }
}

#[test]
fn an_unrolled_factorial_matches_the_measured_counts() {
    let source = include_str!("fixtures/factorial_O2.txt");
    for (n, measured) in [
        (0, "5"),
        (1, "5"),
        (2, "13"),
        (3, "15"),
        (7, "27"),
        (8, "34"),
    ] {
        assert_eq!(
            instructions_at(source, "factorial", &[("edi", n)]),
            measured,
            "n = {n}"
        );
    }
}

#[test]
fn a_chain_of_comparisons_keeps_every_case_it_excludes() {
    let source = include_str!("fixtures/grade_O0.txt");
    assert_eq!(
        cases(source, "grade", &[]),
        [
            "[edi <= 1, edi != 0] 27 / 12 / 2",
            "[edi = 0] 25 / 11 / 2",
            "[edi = 2] 21 / 9 / 2",
            "[edi = 3] 17 / 7 / 2",
            "[edi = 4] 13 / 5 / 2",
            "[edi >= 6] 10 / 4 / 2",
            "[edi = 5] 9 / 3 / 2"
        ]
    );
}

#[test]
fn a_bit_test_on_a_known_formula_is_decided_by_residue() {
    let source = include_str!("fixtures/array_max_O2.txt");
    for (n, measured) in [
        (1, "4"),
        (2, "17"),
        (3, "20"),
        (4, "28"),
        (5, "29"),
        (8, "46"),
        (9, "47"),
    ] {
        assert_eq!(
            instructions_at(source, "array_max", &[("esi", n)]),
            measured,
            "n = {n}"
        );
    }
    let lines = cases(source, "array_max", &[]);
    assert!(
        lines.iter().all(|line| !line.contains("Wraps")),
        "{lines:?}"
    );
}

const UNSIGNED_AFTER_SUB: &str = "pick:
        mov     eax, edi
        sub     rax, 5
        ja      .Lhigh
        mov     eax, 1
        ret
.Lhigh:
        mov     eax, 2
        nop
        ret
";

#[test]
fn an_unsigned_jump_after_sub_compares_the_operands_not_the_result() {
    assert_eq!(
        cases(UNSIGNED_AFTER_SUB, "pick", &[("edi", 0)]),
        vec!["[] 5 / 1 / 0".to_owned()]
    );
    assert_eq!(
        cases(UNSIGNED_AFTER_SUB, "pick", &[("edi", 9)]),
        vec!["[] 6 / 1 / 0".to_owned()]
    );
}

const UNSIGNED_AFTER_ADD: &str = "pick:
        add     edi, 1
        ja      .Lhigh
        mov     eax, 1
        ret
.Lhigh:
        mov     eax, 2
        nop
        ret
";

#[test]
fn an_unsigned_jump_after_add_is_not_read_as_a_comparison() {
    assert_eq!(
        cases(UNSIGNED_AFTER_ADD, "pick", &[("edi", 3)]),
        vec!["[] 4..5 / 1 / 0".to_owned()]
    );
}

#[test]
fn a_32_bit_counter_taken_from_a_64_bit_address_is_counted_in_32_bits() {
    let source = include_str!("fixtures/poly_Os.txt");
    assert_eq!(
        cases(source, "poly", &[("esi", -1), ("edx", 0)]),
        ["[] 9 / 1 / 0"]
    );
    assert_eq!(cases(source, "poly", &[("esi", 4)]), ["[] 37 / 5 / 0"]);
}

#[test]
fn a_32_bit_argument_keeps_its_32_bit_name_in_the_formula() {
    let source = include_str!("fixtures/poly_Os.txt");
    assert_eq!(
        cases(source, "poly", &[]),
        [
            "[esi >= 1] 7 * esi + 9 / esi + 1 / 0",
            "[esi <= 0] 9 / 1 / 0"
        ]
    );
}

#[test]
fn data_dependent_alternatives_are_one_range_containing_each_measurement() {
    let source = include_str!("fixtures/array_max_O2_v3.txt");
    for (esi, measured) in [(3, 27), (4, 29), (5, 39), (6, 37)] {
        let found = cases(source, "array_max", &[("esi", esi)]);
        let [case] = found.as_slice() else {
            panic!("one case for esi = {esi}: {found:?}");
        };
        let instructions = case
            .trim_start_matches("[] ")
            .split(" / ")
            .next()
            .expect("an instruction count");
        let (low, high) = instructions
            .split_once("..")
            .unwrap_or((instructions, instructions));
        let low: i128 = low.parse().expect("a whole low end");
        let high: i128 = high.parse().expect("a whole high end");
        assert!(low <= measured && measured <= high, "esi = {esi}: {case}");
    }
}

#[test]
fn no_two_rows_that_can_hold_for_one_input_give_two_exact_counts() {
    let source = include_str!("fixtures/array_max_O2_v3.txt");
    let found = cases(source, "array_max", &[]);
    let rows_for_three: Vec<&String> = found
        .iter()
        .filter(|case| case.starts_with("[esi = 3]") || case.starts_with("[esi >= 3, esi <= 7"))
        .collect();
    assert!(
        rows_for_three.iter().all(|case| !case.contains("] 27 /")),
        "{rows_for_three:?}"
    );
}

const CALLEE_WRITES_CALLER_FRAME: &str = "count_after_set:
        push    rbp
        mov     rbp, rsp
        sub     rsp, 16
        mov     DWORD PTR [rbp-4], 10
        lea     rdi, [rbp-4]
        call    set_two
        mov     eax, DWORD PTR [rbp-4]
.Lloop:
        sub     eax, 1
        jne     .Lloop
        leave
        ret
set_two:
        mov     DWORD PTR [rdi], 2
        ret
";

#[test]
fn a_callee_given_a_pointer_into_the_frame_may_change_it() {
    let found = cases(CALLEE_WRITES_CALLER_FRAME, "count_after_set", &[]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("DependsOnData"), "{found:?}");
}

const STOPS_AT_UD2: &str = "checked:
        test    edi, edi
        js      .Lbad
        mov     eax, edi
        ret
.Lbad:
        ud2
";

#[test]
fn a_path_that_reaches_ud2_is_a_trap_not_a_finished_call() {
    let found = cases(STOPS_AT_UD2, "checked", &[("edi", -1)]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("Trap"), "{found:?}");
    assert_eq!(
        cases(STOPS_AT_UD2, "checked", &[("edi", 5)]),
        ["[] 4 / 1 / 0"]
    );
}

const SIGN_EXTENDED_BYTE: &str = "down_from_byte:
        mov     BYTE PTR [rsp-1], -1
        movsx   eax, BYTE PTR [rsp-1]
        test    eax, eax
        js      .Lnegative
        nop
        nop
.Lnegative:
        ret
";

#[test]
fn movsx_of_a_negative_byte_is_negative() {
    assert_eq!(
        cases(SIGN_EXTENDED_BYTE, "down_from_byte", &[]),
        ["[] 5 / 2 / 1"]
    );
}

#[test]
fn a_relocation_names_the_function_a_call_in_an_object_file_reaches() {
    let source = include_str!("fixtures/relocatable_O0_relocations.txt");
    assert_eq!(cases(source, "use", &[]), ["[] 17 / 6 / 5"]);
}

#[test]
fn a_call_the_linker_has_not_filled_in_is_counted_as_outside() {
    let source = include_str!("fixtures/relocatable_O0.txt");
    assert_eq!(cases(source, "use", &[]), ["[] 10 / 3 / 3"]);
}

#[test]
fn a_jump_into_a_cold_part_follows_it_and_stops_at_abort() {
    let source = include_str!("fixtures/cold_part_O2.txt");
    let negative = cases(source, "checked", &[("edi", -1)]);
    assert!(negative[0].contains("Trap"), "{negative:?}");
    assert_eq!(cases(source, "checked", &[("edi", 4)]), ["[] 17 / 1 / 0"]);
}

#[test]
fn a_call_to_a_bare_address_in_a_stripped_binary_is_followed() {
    let source = include_str!("fixtures/stripped_O2.txt");
    assert_eq!(cases(source, "0x1120", &[("edi", 3)]), ["[] 19 / 1 / 0"]);
}

#[test]
fn movsx_of_a_byte_register_sign_extends_from_eight_bits() {
    let source = include_str!("fixtures/widths_nasm.txt");
    let found = cases(source, "movsx_const", &[]);
    assert!(found[0].starts_with("[] 15 /"), "{found:?}");
}

#[test]
fn a_zero_extended_argument_compared_at_64_bits_splits_on_its_sign() {
    let source = include_str!("fixtures/widths_nasm.txt");
    let found = cases(source, "zext_cmp", &[("esi", -3)]);
    assert!(found[0].starts_with("[] 7 /"), "{found:?}");
}

#[test]
fn the_low_half_of_a_64_bit_argument_is_its_own_input() {
    let source = include_str!("fixtures/widths_nasm.txt");
    let found = cases(source, "low32", &[("rdi", 4_294_967_301)]);
    assert!(found[0].starts_with("[] 18 /"), "{found:?}");
    let source = include_str!("fixtures/wide_O0.txt");
    for rdi in [4_294_967_301, -4_294_967_291] {
        let found = cases(source, "wide", &[("rdi", rdi)]);
        assert!(found[0].starts_with("[] 47 /"), "rdi = {rdi}: {found:?}");
    }
}

#[test]
fn a_switch_through_a_table_keeps_its_exact_unsigned_rows() {
    let source = include_str!("fixtures/grade_O2.txt");
    assert_eq!(
        cases(source, "grade", &[]),
        [
            "[edi <= 5 (unsigned)] 6 / 2 / 0",
            "[edi >= 6 (unsigned)] 4 / 1 / 0"
        ]
    );
}

#[test]
fn an_exception_or_a_thread_exit_leaves_without_ending_the_program() {
    let source = include_str!("fixtures/throws_cpp.txt");
    let found = cases(source, "_Z7checkedi", &[("edi", -1)]);
    assert!(found[0].contains("LeavesWithoutReturning"), "{found:?}");
    let source = include_str!("fixtures/thread_exit.txt");
    let found = cases(source, "worker", &[("rdi", 1)]);
    assert!(found[0].contains("LeavesWithoutReturning"), "{found:?}");
}

#[test]
fn a_cold_part_in_assembly_source_is_followed_to_abort() {
    let source = include_str!("fixtures/cold_part_source_O2.txt");
    let found = cases(source, "checked", &[("edi", -1)]);
    assert!(found[0].contains("Trap"), "{found:?}");
}

const ZERO_EXTENDED_COUNT_DOWN: &str = "zloop:
    mov     eax, edi
.l:
    dec     rax
    jnz     .l
    ret
";

#[test]
fn a_zero_extended_counter_wraps_only_where_it_starts_at_zero() {
    assert_eq!(
        cases(ZERO_EXTENDED_COUNT_DOWN, "zloop", &[]),
        [
            "[edi = 0] Loop(Line(5), Wraps)",
            "[edi >= 1] 2 * edi + 2 / 1 / 0",
            "[edi <= -1] 2 * edi + 8589934594 / 1 / 0"
        ]
    );
}

#[test]
fn a_zero_extended_counter_below_zero_counts_as_its_unsigned_value() {
    let found = cases(ZERO_EXTENDED_COUNT_DOWN, "zloop", &[("edi", -1)]);
    assert!(found[0].starts_with("[] 8589934592 /"), "{found:?}");
}

#[test]
fn a_refused_row_over_a_few_values_is_answered_value_by_value() {
    let source = include_str!("fixtures/array_max_O2_v3.txt");
    let found = cases(source, "array_max", &[]);
    assert!(
        !found.iter().any(|case| case.contains("DataRange")),
        "{found:?}"
    );
    for (value, given) in [(3, "27"), (5, "37"), (7, "45")] {
        let rows: Vec<&String> = found
            .iter()
            .filter(|case| case.starts_with(&format!("[esi = {value}] ")))
            .collect();
        assert_eq!(rows.len(), 1, "esi = {value}: {found:?}");
        let alone = cases(source, "array_max", &[("esi", value)]);
        assert!(alone[0].starts_with(&format!("[] {given}")), "{alone:?}");
        assert_eq!(
            rows[0].split_once("] ").map(|(_, rest)| rest),
            alone[0].split_once("] ").map(|(_, rest)| rest)
        );
    }
}
