#!/bin/sh
# Fixture for the ambient-variable audit: a name exported without a
# value.
#
# `export NAME` assigns nothing. It marks a name for the environment,
# and whatever the calling shell already put there survives — so the
# read below is as ambient as it would be with no export at all.
# Counting the bare form as an assignment removes the name from the
# audit and lets a generic guard through.

export GENERIC_GUARD

if [ -n "$GENERIC_GUARD" ]; then
    exit 0
fi
