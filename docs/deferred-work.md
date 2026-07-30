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
