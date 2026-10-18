/**
 * Which level the player loads, decided here rather than by the
 * engine's bandwidth estimate.
 *
 * hls.js left to itself starts from a low estimate of the bandwidth
 * and measures each segment's load to correct it. Every segment
 * reaches it through the app's local proxy, which spaces requests to
 * the host, so the time a segment waits in that line reads as a slow
 * network and the estimate stays low: a `best` setting played the
 * master's lowest level. The governor takes the choice instead. It
 * starts at the level the setting names and stays there while
 * playback is healthy. Playback that stalls with nothing buffered
 * steps it one level down, at most once per cooldown; a level whose
 * playlist cannot be loaded is left for the rest of the stream; and
 * once playback has run healthy for a while with a buffer in hand it
 * steps back up, one level per window, towards the setting's level —
 * the window doubling with every step down, so a level that cannot
 * be held is retried less and less often rather than in a loop.
 *
 * A level the viewer picks is pinned: stalls do not move it; a failure
 * moves the play below it, and healthy playback brings the play back
 * up to it, never above; only a level the engine cannot load moves the
 * pin itself down. "Auto" hands the choice back.
 *
 * What the governor holds and the moves that need no clock live in
 * `quality-hold.ts`; how often it may move, in `quality-pace.ts`.
 */

import { QualityHold, type QualityMove, type QualityReason } from './quality-hold';
import { followedPin } from './quality-pinlevel';
import { QualityPace } from './quality-pace';

export type { QualityMove, QualityReason } from './quality-hold';
export {
	HEALTHY_BUFFER_S,
	HEALTHY_WINDOWS_MS,
	SEEK_GRACE_MS,
	STEP_DOWN_COOLDOWN_MS
} from './quality-pace';

export class QualityGovernor extends QualityHold {
	private readonly pace = new QualityPace();

	/** The viewer seeked. */
	seeking(now: number): void {
		this.pace.seeking(now);
	}

	/** Playback stalled with nothing buffered. */
	stall(now: number): QualityMove | null {
		if (!this.engaged || this.pace.inSeekGrace(now)) return null;
		// A pin is never stepped down for a stall.
		const held = this.pinLevel !== null || this.pace.coolingDown(now);
		const lower = held ? null : this.neighbour(this.current, -1);
		// A stall that moves nothing — under a pin, at the lowest level,
		// or inside the cooldown — is trouble all the same: the climb
		// waits for a window without any.
		if (lower === null) {
			this.pace.troubled(now);
			return null;
		}
		return this.stepDown(lower, now, 'stall');
	}

	/** The engine gave up on `level`'s segments after its retries. Not
	 *  a stall to weigh: the level just failed, so the step is taken at
	 *  once, pinned or not — and with nothing below it, the engine's own
	 *  fallback is left to try what it can. */
	fragmentsFailed(level: number, now: number): QualityMove | null {
		if (!this.engaged || level !== this.current) return null;
		const lower = this.neighbour(this.current, -1);
		if (lower === null) return this.release();
		return this.stepDown(lower, now, 'fragments-failed');
	}

	/** A segment of the main rendition was buffered. */
	healthy(now: number, bufferAheadS: number): QualityMove | null {
		if (!this.engaged || !this.pace.mayClimb(now, bufferAheadS)) return null;
		const higher = this.neighbour(this.current, 1);
		// Never above the setting's level — nor above the pin, which a
		// pinned play climbs back to after a fall.
		const cap = this.pinLevel ?? this.ceiling;
		if (higher === null || this.ladder.indexOf(higher) > this.ladder.indexOf(cap)) return null;
		this.pace.climbed(now);
		return this.move(higher, 'healthy', 'load');
	}

	/** hls.js left the level held for `level` over a failure of a
	 *  side rendition — audio or subtitles in a group the held level
	 *  has. That is no trouble of the video: the governor holds the
	 *  level the engine moved to — above the setting's level too, when
	 *  the levels below share the failing group — without counting a
	 *  step down. The level left is left for the stream: hls.js forgets
	 *  a level's load errors once one of its video segments buffers, so
	 *  a climb back would fail the same audio and repeat the switch. A
	 *  level that failed, or the one held, is no move. */
	follow(level: number, now: number, blame = true): QualityMove | null {
		if (!this.engaged || level === this.current || this.unavailable.has(level)) return null;
		// `blame` false: the level held was not the one the engine left —
		// a removal re-seated the hold on a level that never played.
		if (blame) this.unavailable.add(this.current);
		// A pin on the level left moves to where the engine went; one the
		// engine moved above can no longer be honoured and is let go.
		this.pinLevel = followedPin(
			this.ladder,
			this.unavailable,
			this.pinLevel,
			this.current,
			level,
			blame
		);
		// Left below a pin it can return to, the play is in a fall: the
		// climb window opens and widens, as after a step down. An Auto
		// play has nothing to return to that the engine did not judge.
		if (this.pinLevel !== null && this.pinLevel !== level) this.pace.steppedDown(now);
		else this.settled(now);
		return this.move(level, 'followed', 'load');
	}

	protected override restarted(now: number): void {
		this.pace.restart(now);
	}

	/** A move to a level that has proven nothing yet restarts the
	 *  healthy window. */
	protected override settled(now: number | undefined): void {
		if (now !== undefined) this.pace.troubled(now);
	}

	private stepDown(level: number, now: number, reason: QualityReason): QualityMove {
		this.pace.steppedDown(now);
		return this.move(level, reason, 'load');
	}
}
