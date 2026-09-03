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
const MIN_CARRY_SECONDS = 1;

export class RecoveryResume {
	private pending: { showId: string; episode: number; at: number } | null = null;

	/** Remember where playback stood when a recovery began. */
	capture(showId: string, episode: number, currentTime: number): void {
		this.pending = currentTime >= MIN_CARRY_SECONDS ? { showId, episode, at: currentTime } : null;
	}

	/** The position the media attach for `episode` of `showId` should
	 *  seek to, or null. Consuming clears the capture either way: a
	 *  match hands the position over once, a mismatch discards stale
	 *  intent. */
	consume(showId: string, episode: number): number | null {
		const pending = this.pending;
		this.pending = null;
		return pending !== null && pending.showId === showId && pending.episode === episode
			? pending.at
			: null;
	}
}

/** The shared carrier, module-level like the singleton video whose
 *  playback it describes: a recovery can begin while the play route
 *  is unmounted (PiP) and land in a fresh mount, so the pending
 *  position must not die with a component instance. */
export const recoveryResume = new RecoveryResume();
