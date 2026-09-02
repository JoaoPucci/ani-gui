/**
 * Playback-position carry-over for the stale-stream recovery.
 *
 * The recovery swaps the whole session — evict, fresh resolve, new
 * media URL — and the re-attach starts the video at zero, so a
 * mid-episode stall used to cost the user their place on top of the
 * interruption. This keeps the position across exactly that swap:
 * the recovery captures where playback was, and the next media
 * attach for the SAME episode consumes it once.
 *
 * A different episode discards the capture instead of consuming it —
 * if the user picks another episode while the recovery is in flight,
 * seeking their old position into the new episode would be the stale
 * intent this module exists to avoid (the same lesson the strip
 * pager's unreflected marker learned). Near-zero positions are not
 * worth carrying: the restart the user resents is losing minutes,
 * not a second.
 */
export class RecoveryResume {
	/** Remember where playback stood when a recovery began. */
	capture(episode: number, currentTime: number): void {
		void episode;
		void currentTime;
	}

	/** The position the media attach for `episode` should seek to, or
	 *  null. Consuming clears the capture either way: a match hands
	 *  the position over once, a mismatch discards stale intent. */
	consume(episode: number): number | null {
		void episode;
		return null;
	}
}
