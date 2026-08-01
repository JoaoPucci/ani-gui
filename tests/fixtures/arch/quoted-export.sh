#!/bin/sh
# Fixture: an export that is printed, not performed.
#
# The text sits inside double quotes, which the shell expands but does
# not execute. An unanchored `export ...=` pattern matches it anywhere
# on the line and treats the diagnostic as ownership.

printf '%s\n' "export GENERIC_GUARD="

if [ -n "$GENERIC_GUARD" ]; then
    exit 0
fi
