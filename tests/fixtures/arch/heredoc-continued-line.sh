#!/bin/sh
# Fixture: one command split across lines, opening a heredoc on each.
#
# A trailing backslash continues the command, so both redirections
# belong to it. Scanning line by line finds the first, then reads the
# continuation — which carries the second opener — as the first body.

cat <<ONE \
    <<TWO
this body is data
ONE
GENERIC_GUARD=this body is data too
TWO

if [ -n "$GENERIC_GUARD" ]; then
    exit 0
fi
