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
 *
 * "Switch to it now" carries its point across to the picked show's
 * player through the same carrier, with `carry`, when it opens the
 * card's episode: there the point is the one the confirmation named,
 * 0:00 included, so it is kept exactly and wins over a position the
 * pick kept. A switch to the pick's own episode carries none.
 */
import { nextWrite } from './write-order';

const MIN_CARRY_SECONDS = 1;

export class RecoveryResume {
	private pending: { showId: string; episode: number; at: number; written: number } | null = null;

	/** Remember where playback stood when a recovery began. */
	capture(showId: string, episode: number, currentTime: number): void {
		this.pending =
			currentTime >= MIN_CARRY_SECONDS
				? { showId, episode, at: currentTime, written: nextWrite() }
				: null;
	}

	/** Carry `at` seconds to the next attach of `episode` of `showId`
	 *  exactly as asked: a switch to a picked show says where it picks
	 *  up, and 0:00 is a point there, not the want of one, so the pick's
	 *  own saved position does not take its place. A value that is no
	 *  number of seconds carries nothing. */
	carry(showId: string, episode: number, at: number): void {
		this.pending =
			Number.isFinite(at) && at >= 0 ? { showId, episode, at, written: nextWrite() } : null;
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

	/** Forgets a capture for `showId` — its positions are forgotten
	 *  with its history row — unless it was taken after `since`
	 *  (write-order.ts), once the row was gone. */
	forgetShow(showId: string, since = Infinity): void {
		if (this.pending?.showId === showId && this.pending.written < since) this.pending = null;
	}

	/** Forgets any capture — the history is cleared. */
	forgetAll(): void {
		this.pending = null;
	}
}

/** The shared carrier, module-level: a recovery can begin, the
 *  viewer leave before it lands, and the episode be opened again in a
 *  fresh mount, so the pending position must not die with a component
 *  instance. */
export const recoveryResume = new RecoveryResume();
