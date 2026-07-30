# Deferred work

Valid work that was found during a change and deliberately not done in
that change. Each entry says what the work is, why it was deferred,
and what "done" would look like — enough that someone who was not in
the conversation can pick it up.

This file is tracked, so an entry survives leaving the checkout it was
written in and a pull request thread can cite it. That is the whole
point: the internal planning directory is git-ignored and this
repository has issues disabled, so neither can hold a record anyone
else is able to read.

Adding an entry is not a way to avoid the work. `AGENTS.md` §14 lists
it third of four options, after doing it here and doing it in its own
pull request.

Remove an entry when the work lands, in the change that lands it.

---

## Where arch-layer self-tests belong in the test taxonomy

`tests/arch/deferral_record_test.sh` exercises the predicate, path
parser, probe and signal handling of `tests/arch/deferral_record.sh`
directly, using its own assertion helpers rather than a framework.

Review asked whether that belongs in the bats-core suite, reading
`AGENTS.md` §2's "Bash changes require bats-core coverage" as covering
it. Two readings are available and the text supports both:

- The bash line scopes to the shell *product*. Every bats suite under
  `tests/bash/**` targets the vendored `ani-cli` script, and
  `tests/arch/` is a separate tier in the `docs/testing.md` pyramid
  with its own runner and its own `arch.yml` workflow. Eight arch
  scripts predate this one and none has bats coverage.
- The line covers any shell change, in which case those eight are
  uncovered too and the gap is repository-wide.

**Done looks like:** §2 states which reading is correct. If the second,
a follow-up ports the arch self-tests — all of them, not only this one
— to bats.

The risk that motivates the question is a bespoke harness that fails
to report failures. That much was measured and currently holds:
breaking an assertion produces `arch/deferral_record_test: FAILED` in
`run-all.sh` and a nonzero exit.

## `arch/bash_portability` fails on `master`

`ani-cli diverges from upstream by 41 lines (max 4)`, so
`bash tests/arch/run-all.sh` exits nonzero on a clean checkout. Every
other check in the suite passes.

The carried fork patches are listed in `AGENTS.md` §3 and are
deliberate, so either the ceiling no longer reflects the patch set we
intend to carry, or a patch landed without being recorded there.

**Done looks like:** the divergence is reconciled against §3's list —
ceiling raised to match the intended patches, or an unrecorded patch
documented or removed — and the suite exits zero on `master`.

## How much verification the deferral invariant should carry

`tests/arch/deferral_record.sh`, its self-test, and
`tests/arch/agents_contract.sh` grew through fifteen review findings on
one pull request. Each was real and each was fixed. The pattern in the
later ones is worth recording: several were introduced by the fix
before them — the fenced-import gap arrived with the symlink work, and
the nested-fence gap arrived with the fence work.

That is what happens when a checking apparatus outgrows the thing it
checks. The policy it guards is two paragraphs of prose that a person
reads once; the checks around it are now several hundred lines with
their own test suite, and every change to them has a fair chance of
opening a new gap somewhere else.

Nothing here is wrong as it stands. The open question is where the
line sits — whether this level of rigour is what the repository wants
around a documentation invariant, or whether some of it should be
simplified back on the grounds that a reviewer reading `AGENTS.md`
catches the realistic failures more cheaply than a parser that has to
agree with Markdown about fence nesting.

**Done looks like:** a decision, written into `AGENTS.md` §2 or
`docs/testing.md`, about how much verification an invariant of this
kind should carry — so the next one is built to that standard rather
than to whatever a review round happens to surface.

**Update.** The count reached twenty-eight findings, and the shape is
now clear enough to name a specific change rather than only a
question.

Everything the two checks do falls into one of two kinds. Constraints
— declare the record path, one `record-path` mention per file, no
fence in CLAUDE.md or in the section, marker at column zero — each
closed its category permanently and generated no further findings.
Interpretation — locating the policy section by parsing Markdown —
has now taken five rounds of rules (delimiter character, run length,
info string, indent, tab expansion) and each arrived after the
previous had shipped.

The proposal is to stop parsing document structure. Verify that the
heading appears exactly once and that `record-path` appears exactly
once, and drop section-body extraction along with the fence tracking
it needs. What that gives up is a contrived case — someone wrapping
the entire live policy in a fenced block — which breaks the rendered
document visibly and which no amount of tracking has yet been shown
to catch reliably anyway. What it buys is termination.

That is a design reversal on a load-bearing check, so it is the
maintainer's call rather than something to slip into a review round.

**The limit, stated precisely.** Two cases have to be distinguished,
and only one of them is detectable.

A policy element *duplicated* in an inert position — a second heading
inside a fence, an HTML comment, a blockquote — is caught by counting,
because the copy makes two. That holds for every inerting construct
without knowing what any of them are, and it is asserted for fences
and for HTML comments.

A policy existing *only* in an inert position — the whole section
fenced, or commented out, with no live copy — is not detectable
without parsing Markdown and HTML. Fence tracking currently catches
the fenced variant and nothing else, which is worse than not catching
it: it implies a completeness the check does not have, and it has cost
six rounds of rules with two defects introduced by its own fixes.

The recommendation is therefore to drop the tracking and say plainly
that the check verifies the policy's *declaration*, not that the
document renders as intended. A wholly-inert policy section is visible
to anyone who opens the rendered file and to any reviewer of the diff
that made it inert; a parser is not the right instrument for it.

## The awk failure that exits zero

`deferral_record.sh` pointed at a missing AGENTS.md printed
`awk: fatal: cannot open file` and still exited 0. That was found
while reproducing a different defect, and it is the same silent-pass
shape as everything else here: the check reported success having read
nothing.

The immediate cause was fixed — a stray `REPO_ROOT` can no longer
redirect the script — but the exit status is not defended in its own
right. A tool inside the pipeline can fail without the script noticing.

**Done looks like:** the awk passes' failure is detected and turns
into a nonzero exit, asserted by pointing the check at a path that
does not exist.

## Three open findings on the deferral checks

Raised in review and not yet fixed. Recorded here so they survive the
thread.

**Intent-to-add detected by porcelain, not by index metadata.**
`record_is_recoverable` rejects `git add -N` entries by matching `" A"`
in `git status --porcelain`. If the file is then deleted from the
working tree the porcelain line changes, and the check stops
recognising it. Reading the intent-to-add bit from index metadata is
the robust form.

**Done looks like:** the predicate identifies intent-to-add from the
index rather than from working-tree status, with a case for the
deleted-file variant.

The failure message is wrong for this case too. `why_unrecoverable`
sees that `ls-files` succeeds and reports "tracked as a symlink or
submodule rather than a file", which is neither true nor actionable —
the fix is `git add` on a path that is already, in a sense, added.
Reporting the wrong reason has already been a defect on this branch
once, for symlinks; the same argument applies. Fix it in the same
change as the detection.

**Setext H2 does not end the section.** The body scan recognises
`^ {0,3}## `. A heading written as text followed by a line of `-` is
also an H2, so the section runs past it into the next one.

This belongs to the Markdown-interpretation layer described above, and
the same argument applies: it is the eighth rule of an open set. The
recommendation remains to stop parsing document structure rather than
to add setext handling.

**The signal probe re-execs by `$0`.** `deferral_record_test.sh`
re-runs itself as `sh "$0"` for the cancellation case. Launched as
`sh ./deferral_record_test.sh` from `tests/arch`, the re-exec resolves
against a different working directory. An absolute path fixes it.

**Done looks like:** the probe re-execs by absolute path, asserted by
running the suite from `tests/arch` as well as from the root.
