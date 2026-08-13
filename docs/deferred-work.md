# Deferred work

Valid work that was found during a change and deliberately not done in
that change. An entry says what the work is and why it waited, plus
anything genuinely surprising about it — enough for someone who was
not in the conversation to pick it back up.

These are reminders, not specifications. An entry does not have to
enumerate the files that will change, state acceptance criteria, or
define what "done" looks like; whoever takes the work scopes it
against the code as it is then, which is the only scoping worth
trusting. So an entry that states something false is a defect, and an
entry that leaves things out is not.

This file is tracked, so an entry survives leaving the checkout it was
written in and a pull request thread can cite it. That is the whole
point: the internal planning directory is git-ignored and this
repository has issues disabled, so neither can hold a record anyone
else is able to read.

Adding an entry is not a way to avoid the work. `AGENTS.md` §14 lists
it third of four options, after doing it here and doing it in its own
pull request.

Write an entry about the code, not about the work in flight. An entry
stands on its own: what is wrong, where it lives, and what should
change, all readable without following anything. If taking the links
out would take the point with them, the entry is not written yet.

On that condition a commit sha or a merged pull-request number is
welcome. Both resolve forever, and where an entry is about this
repository's own history they are the precise evidence for it. Cite
them underneath the explanation, never in place of it — the reader
should reach for one to confirm what the entry already told them, not
to find out what it meant.

What stays out is state that expires: a branch name, a commit count, a
sentence like "the PR for this is open". A rotted entry is worse than
no entry, because it sends the next reader to rebuild finished work or
to reason from a state that no longer exists. References out of the
repository that do not rot — an upstream issue, a specification — are
fine on the same terms.

Remove an entry when the work lands, in the change that lands it.

---

## Known gaps in the deferral checks

What `tests/arch/deferral_record.sh` gets wrong today.

**Intent-to-add is read out of porcelain rather than index metadata.**
`record_is_recoverable` rejects a `git add -N` entry by matching `" A"`
in `git status --porcelain`. Delete that file from the working tree
afterwards and the porcelain line changes, so the check stops
recognising the state it is there to catch. Reading the intent-to-add
bit from index metadata is the robust form. A deleted-file variant of
the same state is worth handling in the same change.

The failure message is wrong for this case too. `why_unrecoverable`
sees `ls-files` succeed and reports "tracked as a symlink or submodule
rather than a file" — neither true nor actionable, when what the
reader needs is `git add` on a path that is already, in a sense,
added. A check that names the wrong reason is a defect in its own
right rather than a cosmetic one, because it sends whoever hits it to
fix something that is not broken. Correct the message in the change
that corrects the detection.

**A setext H2 does not end the section.** The body scan recognises
`^ {0,3}## ` only. A heading written as a line of text underlined with
`-` is also an H2, so the scan runs past it into the section below.

Adding setext handling is the wrong response. This is the
regex-interpretation trap `AGENTS.md` §2 describes, where each rule
added reveals the next one; the way out is to stop parsing document
structure, not to parse more of it.

---

# Backlog

Work that is known, wanted and not scheduled. Kept here rather than in
an agent's session state, which nobody else can read and which
disappears when the session does. An item leaves this list by being
done or by being decided against in writing.

**Treat every entry as a lead, not a fact.** These were carried across
from session state and reviewing them turned up more than a dozen
errors — mostly work that had already shipped, a couple of problems
that never existed as described. Check an item against the code before
starting it, and delete it when you find it done.

## Correctness in the app


## Testing and CI

- **The CRAP ratchet disagrees between CI and local** — 26 against 25 —
  and three files sit at 29.7–30.0, right on the high-risk boundary.
- **The pre-commit hook and strict TDD are in tension — for frontend
  commits.** `frontend-test` is the only hook command that runs tests,
  so a frontend `test(red):` commit fails by construction and is
  rejected. It has been worked around with `--no-verify`, which
  disables every other check too; the workaround is the bug.

  The non-obvious part: pre-commit cannot see the commit message. Git
  writes `COMMIT_EDITMSG` after pre-commit runs, even for `git commit
  -m` — verified with a probe hook. So the gate has to move to
  `commit-msg` and skip only for a `test(red):` subject.

## Interface

- **Localised content fetch** — synopsis and episode titles.
- **Franchise and season grouping** across surfaces.
- **Play-page keep-alive → normal reload with a persisted position.**
- **Search has no sort *direction* control.** Sorting by relevance,
  title, year and rating ships, as do subtype filter chips; only
  ascending/descending is absent. Name any further filters wanted
  before starting, rather than reading this as filters being missing.
- **Update notifier is not resilient to GitHub rate limits.**
- **Adopt the `documentPictureInPicture` browser API** for the player's
  pop-out window. Not a request to write documentation — "Document
  Picture-in-Picture" is the W3C API's name.

  Electron exposes the API but omits the window-creation glue, so it
  cannot work today: electron/electron#39633, open since 2023. Do not
  re-attempt until that lands. PiP as it exists now — the singleton
  video that survives navigation — ships and is described in
  `README.md` and `docs/architecture.md`.
- **Illustrated brand assets** — post-1.0.

- **Bundle the curl-impersonate transport in the Windows package.**
  The native anidb client needs an impersonating curl to get past the
  provider's TLS-fingerprint front; without one the resolver falls
  through to plain curl, which the front answers with the
  interstitial. The Linux packages stage one; Windows is untouched,
  and the open question there is which impersonate build runs under
  Git Bash and how it ships. Surprising: ani-cli 5.0 itself has the
  same gap on Windows.

  This is a shipped defect on a supported platform, not work that is
  merely wanted. A Windows build installs, opens and browses metadata,
  then fails every play and every download. `AGENTS.md` §15 exists
  because it was deferred rather than blocking, and the deferral was
  the wrong call: a runtime dependency that lands for one packaged
  platform is unfinished, not delivered. `tests/arch/linux_deps.sh`
  asserts the Linux half of exactly this and says in its own header
  that the Windows installer has the same shape; the missing
  counterpart is the check that would have caught it.

- **Nothing enforces the red-before-green pairing.** `AGENTS.md` §2
  requires a `test(red):` predecessor for anything that introduces a
  `feat` or a `fix`, and spells the verification out as a mechanical
  procedure, but nothing runs it. Unpaired commits have reached
  master; each was caught by a reviewer reading the log by hand, or
  missed.

  Four things trip an implementation, and the first is what let the
  known violations through:

  - **The subject is the type, not the scope.** The rule covers every
    `feat` and `fix`, so a gate keyed on `feat(green):`/`fix(green):`
    misses a bare `fix:` — which is the form every unpaired commit
    took.
  - **Ancestry is not pairing.** Asking whether the branch contains
    *a* red passes a green whose red landed on a separately-merged
    branch. Narrowing that to "this green has a red **ancestor**" is
    still too weak: once one honest pair lands, its red is an
    ancestor of everything after it, so the next unpaired green
    inherits it and passes. The gate has to attribute a specific red
    to a specific green — §2 reads that off adjacency, each green's
    parent being its red — not merely find one somewhere behind it.

    Direction matters too, wherever an ancestry test does get used:
    `--is-ancestor <green> <red>` is not the ancestry question
    negated. It exits nonzero for the separately-merged case exactly
    as it does for a correct pair, so all it detects on its own is a
    red committed after its green.
  - **Upstream commits are exempt** (§2). A sync merge imports
    upstream history verbatim, so its `feat` commits have no red of
    ours and cannot acquire one. Provenance is mechanical: reachable
    from the sync merge's second parent.
  - **It has to run before the merge.** A squash merge fuses a
    branch's reds and greens into a single commit, so for anything
    landed that way master's history no longer holds a pairing to
    check and a gate reading master cannot reconstruct one. This
    repository merges both ways, which makes the shortfall silent
    rather than obvious. Run the gate over `master..<branch-head>`
    while the branch is still there. `0dccb527` is one of the fused
    commits, carrying its tests and its fix together.

  Related to the pre-commit and TDD tension above — both are about
  giving the contract teeth instead of restating it.

- **Per-episode audio-mode caps.** Availability answers whether a
  show carries the requested mode, not which of its episodes do, so
  a partly dubbed show advertises its whole listing under the dub
  key and an undubbed episode fails when clicked.

  This is a regression the anidb switch introduced, not longstanding
  behaviour. allanime returned `availableEpisodesDetail` with
  separate `sub` and `dub` arrays, so a single show fetch gave the
  exact per-mode cap and the per-mode extras for free. anidb's
  episode listing carries no audio at all — it lives on each
  episode's languages row — so the same answer now costs one request
  per episode.

  Deriving the prefix anyway was attempted and withdrawn: a
  sub-linear search cannot vouch for rows it never fetched, and
  three review rounds each found the next place where an unfetched
  row got counted (the tail, the front, then anywhere below a
  bisected boundary). Whoever picks this up needs a cheaper source
  of per-episode audio, or a budget for the full scan on listings
  small enough to afford it — not a smarter search over the same
  requests.

## The bundled script's remaining purpose

- **Decide what the ani-cli auto-update setting is for**, now that
  nothing in the app runs the script.

  The toggle is on by default and makes a network request at every
  launch. What it updates is `$XDG_CACHE_HOME/ani-gui/ani-cli` — a
  copy the app maintains for itself. That copy is not on `PATH`, no
  packaging exposes it (the `.deb` postinst symlinks only the
  `ani-gui` launcher), and an `ani-cli` the user installed separately
  is never touched. Playback, downloads and availability all resolve
  natively and read none of it.

  So the setting currently buys a user nothing they can observe. The
  copy describes it accurately, which makes the emptiness visible
  rather than fixing it. Three ways out, and picking one is a
  maintainer call: expose the maintained copy as the terminal
  command so the toggle means what it says; keep the script as an
  inert bundled artifact and retire the updater with its setting and
  its diagnostics panel; or drop the script from the bundle
  altogether and point terminal users at upstream.

## Housekeeping

- **Snapshot `$0`: preserve the basename as well as the directory**, if
  a script ever needs it.
