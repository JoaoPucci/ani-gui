#!/bin/sh
# Fixture for the ambient-variable audit: a guard whose comparison
# spells a hash inside quotes.
#
# A `#` opens a comment only where a word could start and only outside
# quotes. Stripping from the first `#` on the line regardless discards
# the rest of this expression, so the audit never sees the name at all
# and reports that nothing ambient is read here.

if [ "#${GENERIC_GUARD-}" = '#skip' ]; then
    exit 0
fi
