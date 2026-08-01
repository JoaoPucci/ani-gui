#!/bin/sh
# Fixture for the ambient-variable audit. Not a check and not run.
#
# A single uppercase letter is a legal environment variable name, so a
# guard reading one inherits whatever the caller exported. Paired with
# an assignment so only the default-expansion path can catch it.
#
# Lives outside tests/arch on purpose: the audit scans that directory,
# so a fixture kept there would be flagged as a real finding.

X=1
probe_single="${X-}"
