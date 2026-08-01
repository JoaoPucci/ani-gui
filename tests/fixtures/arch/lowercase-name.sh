#!/bin/sh
# Fixture for the ambient-variable audit: a guard spelled in lower
# case.
#
# Shell names are not required to be upper case. `skip_nested` is as
# legal an environment variable as `SKIP_NESTED`, and exporting it
# reaches this read exactly the same way — the convention that
# environment variables are shouted is a convention, not a rule the
# shell enforces.
#
# Patterns anchored on `[A-Z]` see none of this.

if [ -n "${skip_nested-}" ]; then
    exit 0
fi
