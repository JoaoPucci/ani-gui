#!/bin/sh
# Create a directory under the given parent whose name a shell, a TOML
# string, a printf format or a regular expression would read specially,
# and print its absolute path. From the repository root:
#
#     cd backend && TMPDIR="$(sh ../tools/hostile-tmpdir.sh "${TMPDIR:-/tmp}")" cargo test --workspace
#
# Nearly every path the backend tests stage comes from the temporary
# directory. A test that writes such a path into a stub script or a
# config file without quoting it passes under `/tmp` and breaks for
# anyone whose temporary directory has an apostrophe, a double quote or
# a backslash in its name. CI runs the backend tests with TMPDIR
# pointing here, so a test like that fails on the pull request that
# adds it. A test that stages under another root is not reached: the
# download cases in `tests/backend_stop.rs` stage under
# CARGO_TARGET_TMPDIR, because they hard-link the backend in.
#
# The name carries an apostrophe, double quotes, a `#`, a `$` before a
# word, backticks, the backslash sequences `\t` and `\n`, a `%s`,
# brackets, a `*`, a `;`, a `&`, a `|` and spaces. It does not carry a
# `:`. PATH-style lists have no escape for their separator, so the
# tests that put a staged directory on one cannot pass under it. It
# does not carry a newline either: CI hands the path on through
# GITHUB_ENV, which is read one line at a time.
#
# Running it again with the same parent prints the same path.

set -eu

if [ "$#" -ne 1 ] || [ -z "$1" ]; then
    printf 'usage: hostile-tmpdir.sh <parent directory>\n' >&2
    exit 2
fi
if [ ! -d "$1" ]; then
    printf 'hostile-tmpdir.sh: %s is not a directory\n' "$1" >&2
    exit 1
fi

parent=$(CDPATH='' cd -- "$1" && pwd)
# Single-quoted, so the shell reading this file takes every character
# of the name literally, backslashes included; the `$x` and the
# backticks are meant to stay unexpanded.
# shellcheck disable=SC2016
name='ani-gui it'\''s "q" #h $x `b` \t \n %s [s] * ; & |'
dir="$parent/$name"
mkdir -p "$dir"
printf '%s\n' "$dir"
