use std::process::Command;

const CALC: &str = env!("CARGO_BIN_EXE_calc");
const FIXTURE: &str = include_str!("fixtures/chemistry_shaped.tsv");

struct Recorded {
    input: String,
    status: i32,
    answer: String,
}

fn unescape(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            plain.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => plain.push('\n'),
            Some('t') => plain.push('\t'),
            Some('\\') => plain.push('\\'),
            Some(other) => panic!("the fixture holds an unknown escape \\{other}"),
            None => panic!("the fixture ends a line in a lone backslash"),
        }
    }
    plain
}

fn recorded() -> Vec<Recorded> {
    FIXTURE
        .lines()
        .map(|line| {
            let mut fields = line.splitn(3, '\t');
            let input = unescape(fields.next().expect("an input"));
            let status = fields
                .next()
                .expect("a status")
                .parse()
                .expect("a whole-number status");
            let answer = unescape(fields.next().expect("an answer"));
            Recorded {
                input,
                status,
                answer,
            }
        })
        .collect()
}

fn answered(input: &str) -> (i32, String) {
    let output = Command::new(CALC)
        .args([input, "--terse", "--locale", "en"])
        .env_clear()
        .output()
        .expect("calc runs");
    let mut shown = String::from_utf8(output.stdout).expect("output is UTF-8");
    shown.push_str(&String::from_utf8(output.stderr).expect("output is UTF-8"));
    (output.status.code().expect("calc exits"), shown)
}

#[test]
fn every_chemistry_shaped_input_keeps_the_answer_it_had() {
    let changed: Vec<String> = recorded()
        .iter()
        .filter_map(|line| {
            let (status, answer) = answered(&line.input);
            (status != line.status || answer != line.answer).then(|| {
                format!(
                    "{:?}\n  was  {} {:?}\n  now  {} {:?}",
                    line.input, line.status, line.answer, status, answer
                )
            })
        })
        .collect();
    assert!(
        changed.is_empty(),
        "{} of {} recorded inputs changed their answer:\n{}",
        changed.len(),
        recorded().len(),
        changed.join("\n")
    );
}

#[test]
fn the_fixture_names_every_input_once() {
    let lines = recorded();
    let mut inputs: Vec<&str> = lines.iter().map(|line| line.input.as_str()).collect();
    inputs.sort_unstable();
    inputs.dedup();
    assert_eq!(inputs.len(), lines.len());
}
