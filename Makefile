PREFIX ?= /usr/local
DESTDIR ?=
CARGO ?= cargo
CALC_BINARY ?= target/release/calc

.PHONY: build install uninstall check

build:
	$(CARGO) build --release --locked -p calc-cli

install:
	PREFIX='$(PREFIX)' DESTDIR='$(DESTDIR)' CALC_BINARY='$(CALC_BINARY)' \
		CALC_COMPLETION=crates/calc-cli/completion CALC_DOCUMENTS=. \
		packaging/install.sh install

uninstall:
	PREFIX='$(PREFIX)' DESTDIR='$(DESTDIR)' packaging/install.sh uninstall

check:
	tools/check
