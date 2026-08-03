#!/bin/sh
# Fixture: a check that a stray environment can redirect.
#
# This is the defect the whole environment-sensitivity case exists to
# catch — twice for real, with `REPO_ROOT` and with `SKIP_NESTED`. A
# generic name arrives from whatever shell the suite happens to run
# in, the check quietly does something else, and it still exits 0.
#
# It exists so the case that hunts for this has something it is known
# to find. Asserting only against the real checks means the day the
# hunt stops working, it reports ok.

if [ -n "${SKIP_NESTED:-}" ]; then
    printf 'fixture: skipped a case because SKIP_NESTED was set\n'
    exit 0
fi

printf 'fixture: ran every case\n'
