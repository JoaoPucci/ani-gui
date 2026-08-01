#!/bin/sh
# Fixture for the ambient-variable audit. Not a check and not run.
#
# A two-character environment name is legal, and a guard reading one
# inherits whatever the calling shell exported just as a longer name
# does. Paired with an assignment so only the default-expansion path
# can catch it.
#
# Lives outside tests/arch on purpose: the audit scans that directory,
# so a fixture kept there would be flagged as a real finding.

NO=1
probe_short="${NO-}"
