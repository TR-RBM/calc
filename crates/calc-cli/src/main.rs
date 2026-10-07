#![deny(clippy::print_stdout, clippy::print_stderr)]

mod arguments;
mod asm;
mod completion;
mod concept;
mod gpu;
mod language;
mod ode;
mod output;
mod plot;
mod read;
mod run;
mod session_commands;
mod solve;
mod text;
mod units;

use std::ffi::OsString;
use std::io::{self, Read};
use std::process::ExitCode;

use calc_app::{Clock, SystemClock};

use crate::run::{Context, run};

const POSIX_LOCALE_VARIABLES: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];

const ANSWER_STACK_BYTES: usize = 64 * 1024 * 1024;
const PANIC_EXIT: u8 = 101;

fn main() -> ExitCode {
    let answered = std::thread::Builder::new()
        .stack_size(ANSWER_STACK_BYTES)
        .spawn(answer)
        .map(|worker| worker.join().unwrap_or(PANIC_EXIT));
    ExitCode::from(answered.unwrap_or_else(|_| answer()))
}

fn answer() -> u8 {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    let posix_locale_values = POSIX_LOCALE_VARIABLES
        .iter()
        .map(|variable| std::env::var(variable).unwrap_or_default())
        .collect();
    let read_file = |path: &std::path::Path| std::fs::read(path);
    let write_file = |path: &std::path::Path, bytes: &[u8]| std::fs::write(path, bytes);
    let check_output = |path: &std::path::Path| -> io::Result<()> {
        let existed = path.exists();
        let opened = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path);
        if !existed && opened.is_ok() {
            let _ = std::fs::remove_file(path);
        }
        opened.map(|_| ())
    };
    let read_standard_input = || {
        let mut bytes = Vec::new();
        io::stdin().lock().read_to_end(&mut bytes).map(|_| bytes)
    };
    let clock = || -> Box<dyn Clock> { Box::new(SystemClock::new()) };
    let context = Context {
        posix_locale_values,
        read_file: &read_file,
        read_standard_input: &read_standard_input,
        write_file: &write_file,
        check_output: &check_output,
        clock: &clock,
    };
    run(
        &arguments,
        &context,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}
