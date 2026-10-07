use std::env;
use std::path::Path;

const LOCALES_DIRECTORY: &str = "../../locales";
const GENERATED_FILE: &str = "message.rs";
const MANIFEST_DIRECTORY_VARIABLE: &str = "CARGO_MANIFEST_DIR";
const OUTPUT_DIRECTORY_VARIABLE: &str = "OUT_DIR";

#[allow(dead_code)]
#[derive(Debug)]
enum BuildError {
    MissingEnvironment(&'static str),
    Catalog(calc_i18n_build::BuildScriptError),
}

fn main() -> Result<(), BuildError> {
    let manifest_directory = env::var_os(MANIFEST_DIRECTORY_VARIABLE)
        .ok_or(BuildError::MissingEnvironment(MANIFEST_DIRECTORY_VARIABLE))?;
    let output_directory = env::var_os(OUTPUT_DIRECTORY_VARIABLE)
        .ok_or(BuildError::MissingEnvironment(OUTPUT_DIRECTORY_VARIABLE))?;
    calc_i18n_build::write_catalog(
        &Path::new(&manifest_directory).join(LOCALES_DIRECTORY),
        &Path::new(&output_directory).join(GENERATED_FILE),
    )
    .map_err(BuildError::Catalog)
}
