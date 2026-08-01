#!/bin/sh
# Fixture: a guard spelled in lower case.
#
# `skip_nested` is as legal an environment variable as `SKIP_NESTED`,
# and exporting it reaches this read the same way. The shouting is a
# convention, not a rule the shell enforces.

if [ -n "${skip_nested-}" ]; then
    exit 0
fi
