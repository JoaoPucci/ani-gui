# Proposal: one Continue Watching card per franchise

**Status**: not planned. The rest of the watch-order plan this file
held is built and documented where the app's documentation describes
it: the franchise read in
[`title-resolution.md`, "Kitsu's relations, walked into a franchise"](../title-resolution.md#kitsus-relations-walked-into-a-franchise),
and the show page's strip, the order setting, the player's Up next
card, Continue Watching's hand-off and the franchise page in
[`architecture.md`, "Watch order"](../architecture.md#watch-order).
What follows is the one option that was considered and not built, to
be revisited if hunting for the part of a franchise one was watching
is still a complaint.

## The option

Continue Watching would show one card per franchise — the most
recently watched entry — with the franchise named as an eyebrow
("JoJo's Bizarre Adventure · *Golden Wind*"). The entries would stay
separate everywhere else: their own pages, histories and tracker
entries.

**Edge cases.** Two entries of one franchise watched in parallel (a
rewatch of part 1 while part 6 airs) would collapse to one card and
hide the other — the case Netflix avoids by owning the numbering. Rows
without a Kitsu id would not group. Grouping on the home page means
walking the franchise for every Continue row on load (Kitsu requests
on the busiest screen), or recording franchise membership as rows are
written.

**Trackers.** None.

**Cost.** Medium, with the most risk of the options considered: it
changes what Continue means.

## Why it waits

Most of what it promises is already delivered: Continue Watching's
hand-off moves a finished entry's card on to the next title, the
finished entry giving way once the next one is started, and the
strip's "You're watching" card and the franchise page's Resume name
the title being watched from any page of the franchise. What one card
per franchise would add beyond that is hiding parallel watches, which
is the part most likely to surprise.
