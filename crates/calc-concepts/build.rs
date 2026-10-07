use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const CONTENT_DIRECTORY: &str = "../../content";
const GENERATED_FILE: &str = "embedded_content.rs";

fn collect(directory: &Path, root: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            collect(&path, root, files);
        } else if let Ok(relative) = path.strip_prefix(root) {
            let name = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            files.push((name, path.clone()));
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let root = manifest.join(CONTENT_DIRECTORY);
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();
    let output = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join(GENERATED_FILE);
    let mut generated = String::from("pub(crate) const EMBEDDED_CONTENT: &[(&str, &[u8])] = &[\n");
    for (name, path) in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let absolute = fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        generated.push_str(&format!(
            "    ({name:?}, include_bytes!({:?})),\n",
            absolute.display().to_string()
        ));
    }
    generated.push_str("];\n");
    if let Ok(mut file) = fs::File::create(&output) {
        let _ = file.write_all(generated.as_bytes());
    }
}
