#!/usr/bin/env bats
#
# `cited_paths` — the parser that turns AGENTS.md into the list of
# declared record paths.
#
# It runs before the predicate that checks whether a record is
# recoverable, so anything it drops is never checked at all. A dropped
# path is a silent pass, not a visible failure, which is why the
# permissive cases below matter more than the strict ones: git accepts
# names with spaces, tabs, no extension and no slash, and a citation
# using any of them has to survive the parse to be checked.

load '../helpers/loader'

setup() {
    # The check is sourced as a library, under the guard that stops it
    # executing, and pointed at this repository.
    export ARCH_DEFERRAL_RECORD_LIB=1
    export ARCH_REPO_ROOT="$REPO_ROOT"
    # shellcheck disable=SC1091
    . "$REPO_ROOT/tests/arch/deferral_record.sh"
}

# A declared record: the marker line, exactly as the section writes it.
declared() { printf '<!-- record-path: %s -->\n' "$1" | cited_paths; }

# A mention in running prose, which is not a declaration.
mentioned() { printf 'text about `%s` in a sentence\n' "$1" | cited_paths; }

@test "parses an ordinary dotted file" {
    [ "$(declared 'docs/follow-ups.md')" = 'docs/follow-ups.md' ]
}

@test "parses a name with neither slash nor dot" {
    # git permits it, so a citation using it must survive.
    [ "$(declared 'FOLLOWUPS')" = 'FOLLOWUPS' ]
}

@test "parses a name containing a space" {
    [ "$(declared 'docs/follow ups.md')" = 'docs/follow ups.md' ]
}

@test "parses a name with no extension" {
    [ "$(declared '.planning/follow-ups')" = '.planning/follow-ups' ]
}

@test "parses a directory as the record" {
    [ "$(declared 'docs/follow-ups/')" = 'docs/follow-ups/' ]
    [ "$(declared '.planning/')" = '.planning/' ]
}

@test "parses a record at the repository root" {
    [ "$(declared 'follow-ups.md')" = 'follow-ups.md' ]
    [ "$(declared './follow-ups.md')" = './follow-ups.md' ]
    [ "$(declared './docs/follow-ups.md')" = './docs/follow-ups.md' ]
}

@test "parses names with surrounding whitespace git permits" {
    [ "$(declared 'trailing-space.md ')" = 'trailing-space.md ' ]
    tabbed="$(printf 'trailing-tab.md\t')"
    [ "$(declared "$tabbed")" = "$tabbed" ]
    [ "$(declared ' leading-space.md')" = ' leading-space.md' ]
}

@test "an indented marker is not a declaration" {
    # Four spaces makes a line code in Markdown, and the boundary is
    # not something this check should have an opinion about — so a
    # declaration starts at column zero, full stop, and anything
    # indented is an example.
    indented() { printf '%s<!-- record-path: docs/x.md -->\n' "$1" | cited_paths; }
    for pad in '    ' '  ' ' '; do
        [ -z "$(indented "$pad")" ]
    done
}

@test "prose is not a declaration" {
    [ -z "$(mentioned 'piped')" ]
    [ -z "$(mentioned '#N · Title')" ]
    [ -z "$(mentioned 'git check-ignore')" ]
    [ -z "$(mentioned 'https://example.com/x')" ]
}
