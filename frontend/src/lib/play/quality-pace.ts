/**
 * How often the quality governor may move on its own: the grace a
 * seek gets, the cooldown between steps down, and the healthy window
 * before a step back up, which doubles with every step down so a
 * level that cannot be held is retried less and less often.
 *
 * The window restarts on every sign of trouble and on every move to a
 * level that has proven nothing yet, for an Auto play and a pinned
 * play alike — the two paths differ only in what moves the level:
 *
 * | Event                          | Auto                   | Pinned                                | Window                      |
 * |--------------------------------|------------------------|---------------------------------------|-----------------------------|
 * | Stall outside a seek's grace   | steps down, or stays   | stays                                 | step: widens; stay: restarts|
 * | Stall inside a seek's grace    | ignored                | ignored                               | —                           |
 * | Segments fail (the hold)       | steps down / release   | steps down / release                  | step: widens; release: —    |
 * | Playlist fails (the hold)      | nearest loadable level | same; a pin on it moves down          | restarts                    |
 * | Switch over a side rendition   | follows                | follows; pin kept above: a fall       | restarts; that fall: widens |
 * | Removal re-seats the hold      | nearest survivor       | nearest survivor, pin carried         | restarts (kept level: no)   |
 * | Healthy climb                  | one level, to ceiling  | one level, to the pin                 | restarts                    |
 * | Pick / Auto                    | Auto: fresh start      | pin = pick: nothing to climb to       | Auto: reset; pick: untouched|
 *
 * "Widens" counts a fall: the next window is the next longer one. A
 * climb retries one level per window, so a level the engine skipped
 * over a failing rendition is tried once and then left; a pick leaves
 * the play at its pin, where no climb is due.
 */

/** A stall this soon after a seek is the seek's own wait for media,
 *  not the level failing to keep up. */
export const SEEK_GRACE_MS = 3_000;

/** The least time between two steps down: a step's first segment at
 *  the lower level needs time to arrive, and the stall that caused
 *  the step is not a second reason to take another. */
export const STEP_DOWN_COOLDOWN_MS = 8_000;

/** How long playback must run healthy before a step back up: the
 *  first entry after one step down, each later one after another. */
export const HEALTHY_WINDOWS_MS = [30_000, 60_000, 120_000, 240_000] as const;

/** Seconds of buffer ahead that count as healthy. */
export const HEALTHY_BUFFER_S = 15;

export class QualityPace {
	private stepDowns = 0;
	private lastStepDownAt = -Infinity;
	private lastChangeAt = -Infinity;
	private lastSeekAt = -Infinity;

	/** The viewer seeked. */
	seeking(now: number): void {
		this.lastSeekAt = now;
	}

	/** Whether `now` is inside a seek's grace: a stall then is the
	 *  seek's own wait for media. */
	inSeekGrace(now: number): boolean {
		return now - this.lastSeekAt < SEEK_GRACE_MS;
	}

	/** Whether `now` is inside the cooldown after a step down. */
	coolingDown(now: number): boolean {
		return now - this.lastStepDownAt < STEP_DOWN_COOLDOWN_MS;
	}

	/** Playback was in trouble at `now`, or moved to a level that has
	 *  proven nothing yet: the healthy window starts over. */
	troubled(now: number): void {
		this.lastChangeAt = now;
	}

	/** The level stepped down at `now`. */
	steppedDown(now: number): void {
		this.stepDowns += 1;
		this.lastStepDownAt = now;
		this.lastChangeAt = now;
	}

	/** Whether playback at `now`, with `bufferAheadS` in hand, has run
	 *  healthy long enough since the last change to climb a level. */
	mayClimb(now: number, bufferAheadS: number): boolean {
		if (this.stepDowns === 0 || bufferAheadS < HEALTHY_BUFFER_S) return false;
		const window = HEALTHY_WINDOWS_MS[Math.min(this.stepDowns, HEALTHY_WINDOWS_MS.length) - 1];
		return now - this.lastChangeAt >= window;
	}

	/** The level climbed at `now`. */
	climbed(now: number): void {
		this.lastChangeAt = now;
	}

	/** The choice went back to auto at `now`: what came before is no
	 *  trouble of the level auto starts at. The cooldown stands. */
	restart(now: number): void {
		this.stepDowns = 0;
		this.lastChangeAt = now;
	}
}
