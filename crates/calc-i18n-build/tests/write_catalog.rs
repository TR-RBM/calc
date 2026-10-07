use std::fs;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("calc-i18n-build-{}-{name}", std::process::id()));
    fs::create_dir_all(directory.join("locales").join("en")).expect("directory is created");
    fs::write(
        directory.join("locales").join("en").join("common.ftl"),
        "common-greeting = hello { $name }\n",
    )
    .expect("file is written");
    directory
}

#[test]
fn a_catalogue_from_another_directory_is_generated() {
    let directory = fixture("generated");
    let output = directory.join("message.rs");

    calc_i18n_build::write_catalog(&directory.join("locales"), &output)
        .expect("catalogue is written");

    let generated = fs::read_to_string(&output).expect("file is read");
    assert!(
        generated.contains("CommonGreeting { name: String }"),
        "{generated}"
    );
    assert!(
        generated.contains("impl crate::CatalogMessage for Message"),
        "{generated}"
    );
}

#[test]
fn a_missing_directory_is_an_error() {
    let directory = fixture("missing");

    let result = calc_i18n_build::write_catalog(
        &directory.join("no-such-locales"),
        &directory.join("message.rs"),
    );

    assert!(result.is_err());
}
