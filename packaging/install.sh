#!/bin/sh
set -eu

usage() {
    echo "usage: install.sh [install | uninstall]" >&2
    echo "  PREFIX   where to install, /usr/local by default" >&2
    echo "  DESTDIR  a staging directory placed before PREFIX, empty by default" >&2
    exit 2
}

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix=${PREFIX:-/usr/local}
destdir=${DESTDIR:-}
binary=${CALC_BINARY:-$here/calc}
completion=${CALC_COMPLETION:-$here/completion}
documents=${CALC_DOCUMENTS:-$here}

bindir=$destdir$prefix/bin
bashdir=$destdir$prefix/share/bash-completion/completions
fishdir=$destdir$prefix/share/fish/vendor_completions.d
docdir=$destdir$prefix/share/doc/calc

case ${1:-install} in
    install)
        [ -f "$binary" ] || { echo "install.sh: no calc binary at $binary" >&2; exit 1; }
        install -d "$bindir" "$bashdir" "$fishdir" "$docdir"
        install -m 0755 "$binary" "$bindir/calc"
        install -m 0644 "$completion/calc.bash" "$bashdir/calc"
        install -m 0644 "$completion/calc.fish" "$fishdir/calc.fish"
        for document in README.md LICENSE LICENSE-CONTENT; do
            install -m 0644 "$documents/$document" "$docdir/$document"
        done
        echo "installed $bindir/calc"
        ;;
    uninstall)
        rm -f "$bindir/calc" "$bashdir/calc" "$fishdir/calc.fish" \
            "$docdir/README.md" "$docdir/LICENSE" "$docdir/LICENSE-CONTENT"
        rmdir "$docdir" 2> /dev/null || true
        echo "removed $bindir/calc"
        ;;
    *) usage ;;
esac
