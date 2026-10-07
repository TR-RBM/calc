# Contributing

This page describes how to build and test calc and which rules its code and documents follow. Contributions are accepted under the licences of the project: code under Apache-2.0 ([LICENSE](LICENSE)), learning content under `content/` under CC-BY-SA 4.0 ([LICENSE-CONTENT](LICENSE-CONTENT)). By submitting a change you agree that it is licensed that way.

This repository receives releases as snapshots: each release is one commit that holds the whole tree of that release. The history between two releases is not published here. A change you send is reviewed against the latest snapshot and appears, when it is accepted, in the next one.

## Build

calc is one Rust workspace. The command line is the crate `crates/calc-cli` and builds the binary `calc`:

```sh
cargo build --release -p calc-cli
```

You need stable Rust 1.98 or later. Nightly features are not used. The licences of all dependencies are checked against `deny.toml` with `cargo deny check licenses bans sources`; a change that adds a dependency says why it is needed, and its licence must be on that list.

## The gate

```sh
tools/check
```

A change is ready when `tools/check` passes on it. It runs, cheapest first and stopping at the first failure, the three cargo lines under Checks in [README.md](README.md): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`. Each check prints one line, `ok`, `FAILED` or `not run`, and its full output is kept under `target/check-logs/`.

## Code rules

These are the rules the existing code follows. Reviewers hold changes to them; formatting, clippy and the message checks are enforced by tools.

- **Rust only**, apart from the shell scripts under `tools/`.
- **No comments in code.** Names and structure carry the meaning.
- **Every text a person reads goes through the message catalogue.** The messages are Fluent files under `locales/<locale>/<area>.ftl`, English first. The build fails when a translation has a key or a placeholder that English lacks, a test fails when a shipped locale leaves an English key untranslated, and another scans the command line's sources for text written directly in code. German addresses the reader as `du`. Identifiers a program reads, such as JSON field names, option names and verdict names, are never translated.
- **Exact by default.** A result is exact, or an enclosure with proven bounds, or says that it is neither. A faster method that changes an answer, or turns an exact answer into an approximate one, is not an optimisation; it is a change of behaviour and is described as one.
- **Ambiguity is reported, not resolved.** Where a written form has more than one reading in mathematics itself, calc says so and says what to write instead.
- **One README per crate.** Each crate has a `README.md` with the headings What it does and How to test.
- **Unit tests**: one behaviour per test, no test framework beyond the language's own.

## Documents

- A change of behaviour says, in the text that comes with it, what it changes and why, what was run with which results, and what was not tested.
- A user-visible change updates the crate's README, the help text in `locales/` and, for the input language, `calc help language`.
- Plain sentences, short headings, no decoration. Write for a reader who was in no earlier conversation. Never describe something planned as if it existed.

## Commits and changes

- A commit message is one line of at most 72 characters that says what the change does to the code, in the imperative. A body follows after a blank line only where the diff cannot show something a later reader needs.
- A commit message names no person, no conversation and no review state.
- Keep one change to one subject.
- Never commit credentials or private keys.

## Security

Do not report a vulnerability in a public change or issue. See [SECURITY.md](SECURITY.md).
