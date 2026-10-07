# calc-i18n

## What it does

The i18n system: the message table, message rendering and locale resolution.

The build script calls `calc-i18n-build`, which reads `locales/<locale>/<area>.ftl` at the repository root, parses them with the in-house Fluent subset parser in `src/catalog.rs`, runs the generation checks and writes the `Message` enum and the embedded locale tables. The subset is messages, variables, string literal placeables and selectors on the plural categories `one` and `other` or on exact integers. A variable used as a selector becomes a `u64` field, every other variable a `String` field, in the order of first use in the English message.

The build fails when a file does not parse, a key breaks the key format, a key sits in the wrong area file, a translation has a key or placeholder that English lacks, a translation selects on a variable that English uses as text, a placeholder name is not a Rust field name, two keys give the same variant name, a locale directory is not a BCP 47 tag, or a language has no plural rule.

`render` turns a `Message` into `Localized` text in a `Locale`, falling back to English for a missing translation. `resolve_locale` takes the sources in priority order, a BCP 47 tag or a POSIX locale value, and matches the result against the shipped locales. It reads no environment. The frontends pass the values in.

Another repository uses the same system for its own messages. Its crate has a build script that calls `calc_i18n_build::write_catalog` with its own `locales/` directory and includes the generated file. It brings `calc_i18n::pattern` and `calc_i18n::CatalogMessage` into its crate root with `use`, because the generated code names them through `crate::`. It renders with `render` in a `CatalogLocale` of its own message type, resolved with `resolve_locale_in`. `Locale` is calc's own `CatalogLocale<Message>`.

Adding a language means adding its directory under `locales/` and, when its plural rule is not one and other, the rule in `src/plural.rs` and `src/catalog.rs`.

## How to test

`cargo test -p calc-i18n`

`Message::key` gives the key of a message, so a JSON output can carry the key where a screen carries the rendered words. The keys of the language reference are `common-reference-group-<group>` for a heading and `common-reference-<kind>-<name>` with its `-meaning` for a construct; `calc-app` maps a construct to them and tests that every construct of the generated reference has both.

The completeness test checks that every shipped locale translates every English key. The literal scan test checks the sources of `calc-cli` and `calc-cockpit` outside `#[cfg(test)]` items. A literal it flags that is not user-facing goes in `src/literal_scan_allowlist.txt`, one literal per line, spelled as in the source.
