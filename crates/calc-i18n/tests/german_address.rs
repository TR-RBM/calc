use std::fs;
use std::path::Path;

const GERMAN_LOCALE: &str = "../../locales/de";

const FORMAL_WORDS: [&str; 8] = [
    "Sie", "Ihr", "Ihre", "Ihrem", "Ihren", "Ihrer", "Ihres", "Ihnen",
];

fn words(line: &str) -> Vec<String> {
    line.split(|character: char| !character.is_alphabetic())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

fn formal_words_in(text: &str) -> Vec<String> {
    text.lines()
        .enumerate()
        .flat_map(|(number, line)| {
            words(line)
                .into_iter()
                .filter(|word| FORMAL_WORDS.contains(&word.as_str()))
                .map(move |word| format!("{}: {word}", number + 1))
        })
        .collect()
}

#[test]
fn no_german_string_addresses_the_reader_formally() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(GERMAN_LOCALE);
    let mut read = 0;
    let mut found = Vec::new();
    for entry in fs::read_dir(&directory).expect("the German locale is a directory") {
        let path = entry.expect("a locale file").path();
        let text = fs::read_to_string(&path).expect("a locale file is UTF-8");
        read += 1;
        found.extend(
            formal_words_in(&text)
                .into_iter()
                .map(|place| format!("{}:{place}", path.display())),
        );
    }
    assert!(read > 0, "the German locale holds no file to check");
    assert!(
        found.is_empty(),
        "German addresses the reader as du: {}",
        found.join(", ")
    );
}

#[test]
fn the_check_would_find_a_formal_address() {
    assert!(!formal_words_in("cli-found-too-many = engen Sie die Bereiche ein").is_empty());
    assert!(formal_words_in("cli-found-too-many = enge die Bereiche ein").is_empty());
}
