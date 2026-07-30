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
