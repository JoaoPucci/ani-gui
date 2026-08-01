#!/bin/sh
# Fixture for the ambient-variable audit. Not a check and not run.
#
# An uppercase loop variable is assigned by the `for` itself, so it is
# an ordinary local rather than something the calling shell supplies.
# A pattern that recognises `NAME=` and `export NAME` but not
# `for NAME` reports it as ambient and blocks unrelated changes.
#
# Lives outside tests/arch on purpose: the audit scans that directory,
# so a fixture kept there would be flagged as a real finding.

for SCRIPT in one two; do
    printf '%s\n' "$SCRIPT"
done
