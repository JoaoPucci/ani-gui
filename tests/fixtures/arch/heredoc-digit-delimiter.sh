#!/bin/sh
# Fixture: a heredoc delimiter that is not an identifier.
#
# The delimiter is any word. `123` is legal, and an opener that only
# recognises identifiers leaves this body in the source, where the
# inert line below is read as an assignment.

cat <<123
GENERIC_GUARD=this line is data, not an assignment
123

if [ -n "$GENERIC_GUARD" ]; then
    exit 0
fi
