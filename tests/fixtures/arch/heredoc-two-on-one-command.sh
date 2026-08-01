#!/bin/sh
# Fixture: two heredocs opened by one command.
#
# The shell reads the bodies in order, one per redirection. Tracking a
# single delimiter finds the end of the first and then reads the
# second body as ordinary source.

cat <<FIRST <<SECOND
this body is data
FIRST
GENERIC_GUARD=this body is data too
SECOND

if [ -n "$GENERIC_GUARD" ]; then
    exit 0
fi
