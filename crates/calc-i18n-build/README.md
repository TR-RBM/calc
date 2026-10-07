# calc-i18n-build

## What it does

Generates the message catalogue of calc-i18n from a directory of Fluent files: `locales/<locale>/<area>.ftl`. `write_catalog(locales_directory, output_file)` parses the files with calc-i18n's Fluent subset parser, runs the same generation checks, and writes the `Message` enum, its keys, the source and locale tables, and an implementation of `CatalogMessage`. calc-i18n's build script calls it with calc's own `locales/`. A build script in another repository calls it with that repository's own, so both projects use one i18n system and neither copies it.

The parser, the language tags and the plural rules are calc-i18n's own source files, included by path, so there is one copy of each.

## How to test

`cargo test -p calc-i18n-build`. The test writes a small catalogue with one locale in a temporary directory and checks the generated file.
