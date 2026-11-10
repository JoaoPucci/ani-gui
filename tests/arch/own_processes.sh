#!/bin/sh
# Architectural invariant: test code signals only processes it started.
#
# A test stops what it launched by the pid it recorded, or by a process
# group or session it created itself. It never picks a process out of
# the process table — by name, by path, by command line, by
# environment, or by walking parent links from one it knows — because
# a table read can hand it anything that happens to match: a reused
# pid, a process reparented after its parent exited, a process of the
# desktop. A test that walked from a relaunched app to "its parent"
# once signalled the user's service manager, which ended the whole
# desktop session of whoever was running the suite. docs/testing.md
# describes how the e2e suite stops its apps instead.
#
# A syntactic constraint, comments included: none of the tokens below
# appears in the test trees at all, whatever it is used for. That
# includes reading the table only to look at it — an assertion that
# finds "the helper the app started" by scanning is the same selection,
# one step from a kill.
#
#   - the tools that select processes by pattern or list the table:
#     pkill, killall, pgrep, pidof, and ps followed by options, dashed
#     or BSD-style (`ps -eo`, `ps aux`), or named as a program passed
#     to a spawn;
#   - the parent-pid field's name, under any capitalisation (ps's
#     column, the `PPid:` line of /proc/<pid>/status);
#   - enumerating /proc: a glob over its entries, or /proc itself
#     named as a whole — quoted, or followed by a space or the end of
#     the line — which is how a listing (`ls /proc`, `find /proc`,
#     `readdirSync('/proc')`, `os.listdir('/proc')`) names it. A path
#     into one known process, `/proc/${pid}/stat`, is allowed.
#
# Trees scanned: electron/e2e, electron/lib, electron/scripts, tools,
# tests — this file excepted, since it has to name what it forbids.
#
# What this does not cover, deliberately: it cannot tell that a signal
# sent by a recorded pid reaches the process the pid was recorded for,
# nor that a group signalled is one the test created. Those are what
# electron/lib/own-group.cjs establishes for the e2e suite, and what
# its own tests exercise. A selection spelled in none of the forms
# above passes here: another language's process library, a listing of
# a /proc path built at run time, or a walk of parent links that reads
# the field out of /proc/<pid>/stat under another name. None exists
# today; a reviewer has to catch a new one. Production code outside
# electron/lib (electron/main.js, the backend) is not scanned: what it
# stops is its own children, tracked by the handles that spawned them.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

SELF='tests/arch/own_processes.sh'

# One alternative per line, joined below.
patterns='\b(pkill|killall|pgrep|pidof)\b
(^|[^[:alnum:]_./-])ps +[-a-zA-Z]
["'"'"'`]ps["'"'"'`]
[Pp][Pp][Ii][Dd]
/proc/(\[0-9\]|\*)
/proc/?(["'"'"'`[:space:]]|$)'
regex=$(printf '%s\n' "$patterns" | paste -sd'|' -)

found=$(grep -rnE \
    --exclude-dir=node_modules --exclude-dir=dist --exclude-dir=.bats-vendor \
    --exclude-dir=playwright-report --exclude-dir=test-results \
    -e "$regex" \
    electron/e2e electron/lib electron/scripts tools tests |
    grep -v "^$SELF:" || true)

if [ -n "$found" ]; then
    printf 'arch/own_processes FAIL: test code selects processes from the process table.\n' >&2
    printf 'Signal only what the test started: its recorded pid, or a group it created\n' >&2
    printf '(electron/lib/own-group.cjs for the e2e suite; docs/testing.md):\n%s\n' "$found" >&2
    exit 1
fi
exit 0
