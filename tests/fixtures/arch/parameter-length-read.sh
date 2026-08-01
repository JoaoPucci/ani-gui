#!/bin/sh
# Fixture: a name read through the length operator.
#
# `${#NAME}` is a read of NAME. The `#` sits where the patterns expect
# a letter, so none of them match and the read is invisible.

if [ "${#GENERIC_GUARD}" -gt 0 ]; then
    exit 0
fi
