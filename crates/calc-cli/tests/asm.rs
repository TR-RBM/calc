use std::io::Write;
use std::process::{Command, Output, Stdio};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

const SQUARE: &str = "\"square(int)\":
        push    rbp
        mov     rbp, rsp
        mov     DWORD PTR [rbp-4], edi
        mov     eax, DWORD PTR [rbp-4]
        imul    eax, eax
        pop     rbp
        ret
";

const TOTAL: &str = "total:
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

fn calc_with_input(arguments: &[&str], input: &str) -> Output {
    let mut child = Command::new(CALC)
        .args(arguments)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("calc runs");
    child
        .stdin
        .take()
        .expect("standard input")
        .write_all(input.as_bytes())
        .expect("input is written");
    child.wait_with_output().expect("calc ends")
}

fn shown(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn calc_asm_counts_square_exactly() {
    let output = calc_with_input(&["asm", "-", "--locale", "en"], SQUARE);
    assert!(output.status.success());
    let text = shown(&output);
    assert!(text.contains("square(int)"), "{text}");
    assert!(
        text.contains("7 instructions, 3 memory reads and 2 memory writes"),
        "{text}"
    );
}

#[test]
fn calc_asm_says_why_it_gives_no_cycles_on_x86_64() {
    let output = calc_with_input(&["asm", "-", "--locale", "en"], SQUARE);
    assert!(shown(&output).contains("not determined by the code"));
}

#[test]
fn calc_asm_answers_a_loop_as_a_formula_in_german() {
    let output = calc_with_input(&["asm", "-", "--locale", "de"], TOTAL);
    let text = shown(&output);
    assert!(text.contains("esi >= 1  4 * esi + 5 Befehle"), "{text}");
    assert!(text.contains("das zweite ganzzahlige Argument"), "{text}");
}

#[test]
fn calc_asm_takes_a_given_register() {
    let output = calc_with_input(&["asm", "-", "--given", "esi=14", "--locale", "en"], TOTAL);
    assert!(
        shown(&output).contains("61 instructions, 15 memory reads"),
        "{}",
        shown(&output)
    );
}

#[test]
fn calc_asm_refuses_a_given_that_is_not_a_register() {
    let output = calc_with_input(&["asm", "-", "--given", "n=14", "--locale", "en"], TOTAL);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn calc_asm_names_a_missing_function() {
    let output = calc_with_input(&["asm", "-", "--function", "main", "--locale", "en"], TOTAL);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(text.contains("main is not a function"), "{text}");
}

#[test]
fn calc_asm_fails_where_a_count_is_refused() {
    let output = calc_with_input(&["asm", "-", "--locale", "en"], "dispatch:\n    jmp rax\n");
    assert_eq!(output.status.code(), Some(1));
    assert!(shown(&output).contains("computed at run time"));
}
