/**
 * The order in which this session wrote what a history removal
 * forgets later: kept positions (watch-position.ts) and a recovery's
 * pending point (resume-after-recovery.ts). A removal notes the moment
 * its rows are gone, and its cleanup — which can wait on Kitsu — then
 * forgets only what was written before that moment, never a point the
 * user made since. A renderer reload ends the cleanup with the session,
 * so the order lives in memory: whatever an earlier session wrote came
 * before.
 */
let last = 0;

/** The next moment in the session's write order, later than every
 *  moment it handed out before. */
export function nextWrite(): number {
	last += 1;
	return last;
}
