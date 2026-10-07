use std::process::Command;

fn commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn is_dirty() -> bool {
    Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|text| !text.trim().is_empty())
}

fn main() {
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    let mark = if is_dirty() { "+uncommitted" } else { "" };
    println!("cargo:rustc-env=CALC_COMMIT={}{mark}", commit());
    println!(
        "cargo:rustc-env=CALC_TARGET={}",
        std::env::var("TARGET").unwrap_or_default()
    );
}
