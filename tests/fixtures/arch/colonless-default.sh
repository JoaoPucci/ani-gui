#!/bin/sh
# Fixture for the ambient-variable audit. Not a check and not run.
#
# A generic name read with a POSIX default that omits the colon, and
# assigned elsewhere in the same file. The assignment takes it out of
# the "read but never assigned" path, so only the default-expansion
# path can catch it.
#
# Lives outside tests/arch on purpose: the audit scans that directory,
# so a fixture kept there would be flagged as a real finding.

SKIP_NESTED=1
probe_dash="${SKIP_NESTED-}"
probe_equals="${SKIP_NESTED=fallback}"
