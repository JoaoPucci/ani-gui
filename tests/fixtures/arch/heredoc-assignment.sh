#!/bin/sh
# Fixture: an assignment that is text, not code.
#
# A heredoc body is data. The shell hands it to a command and never
# parses it as source, so the line below assigns nothing. Read as an
# assignment, it makes the name look local and the ambient read
# underneath stops being reported.

cat <<'INERT'
GENERIC_GUARD=this line is data, not an assignment
INERT

# Read plainly, with no default: a defaulted read is reported whether
# or not anything assigns the name, so it would never reach the
# assignment pass — which is the pass the heredoc line corrupts.
if [ -n "$GENERIC_GUARD" ]; then
    exit 0
fi
