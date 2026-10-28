#!/bin/sh

# Architectural invariant: a workflow refreshes the distro's apt
# sources and nothing else.
#
# The runner image ships third-party apt sources beside Ubuntu's own
# (Microsoft's packages repository among them). A plain package-list
# refresh reads every one of them and exits non-zero when any answers
# badly — a 403 from that repository once failed a required check
# before a single test ran, on a pull request that touched nothing
# the job installs. None of our steps installs from those sources, so
# the refresh names the distro's deb822 file as the only source list
# and disables the parts directory:
#
#     sudo apt-get update -o Dir::Etc::sourcelist=/etc/apt/sources.list.d/ubuntu.sources -o Dir::Etc::sourceparts=-
#
# An HTTP error from Ubuntu's own mirror still fails the refresh (an
# unreachable one is a warning, as apt has always treated it), and the
# install that follows still fails the step when a package cannot be
# had; only the sources we never use stop being able to break the job.
#
# The rule is syntactic: every line of a workflow, or of a local
# composite action (.github/actions/*/action.yml — the cached apt
# install refreshes there), on which `apt` or `apt-get` is followed by
# an `update` word — ended by whitespace, punctuation or the end of
# the line — must be exactly that command, leading indentation and a
# leading `run:` or `- run:` key aside. A line spelling the refresh
# any other way — bare, with extra flags, with `|| true`, chained with
# `;` or `&&` — is a finding, and so is a comment that happens to
# spell it, so write comments around the refresh without naming the
# command.
#
# What this does NOT check: a refresh reached indirectly (a script
# the workflow runs, `add-apt-repository` refreshing on its own, a
# third-party action doing it), or one split across a line
# continuation in a way that leaves no `update` word on the `apt`
# line. Deciding what a
# multi-line shell command does means parsing shell, which AGENTS.md
# §2 puts out of bounds here; a green run says nothing about those.

set -eu

REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$REPO_ROOT"

if [ ! -d .github/workflows ]; then
    printf 'arch/apt_sources: no .github/workflows directory — skipping\n'
    exit 0
fi

CANON='sudo apt-get update -o Dir::Etc::sourcelist=/etc/apt/sources.list.d/ubuntu.sources -o Dir::Etc::sourceparts=-'
PATTERN='(^|[^[:alnum:]_-])apt(-get)?([[:space:]].*)?[[:space:]]update([^[:alnum:]_-]|$)'

failed=0
for wf in .github/workflows/*.yml .github/workflows/*.yaml \
    .github/actions/*/action.yml .github/actions/*/action.yaml; do
    [ -f "$wf" ] || continue
    hits=$(grep -nE "$PATTERN" "$wf" || true)
    [ -n "$hits" ] || continue
    while IFS= read -r hit; do
        line=${hit#*:}
        # Indentation and a one-line `run:` key are YAML's, not the
        # command's.
        trimmed=$(printf '%s\n' "$line" | sed -E 's/^[[:space:]]*(-[[:space:]]+)?(run:[[:space:]]*)?//')
        if [ "$trimmed" != "$CANON" ]; then
            printf 'arch/apt_sources FAIL: %s:%s refreshes apt sources the job never uses; write it as:\n    %s\n' \
                "$wf" "${hit%%:*}" "$CANON" >&2
            failed=1
        fi
    done <<EOF
$hits
EOF
done

if [ "$failed" -eq 0 ]; then
    printf 'arch/apt_sources PASS\n'
fi
exit "$failed"
