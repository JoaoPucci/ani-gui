/**
 * Whether the episode on the play page has been handed to another
 * player (the external player, Syncplay), and what waits for the
 * viewer to come back to the embedded one.
 *
 * Once the episode is handed off nothing may start the embedded video
 * again by itself: the page stops the engine loading, and an engine
 * start that comes meanwhile (a stream attaching, whatever attached
 * it) is held here rather than run. The viewer coming back by playing
 * the embedded video runs the held start, the latest one only; with
 * none held it restarts what the handoff stopped. A stream the viewer
 * picks instead (an episode, Reload) replaces this one: the handoff is
 * forgotten as it attaches, and nothing held runs.
 */
export class HandoffGate {
	private off = false;
	private held: (() => void) | null = null;

	/** `restart` resumes what the handoff stopped, when no start is held. */
	constructor(private readonly restart: () => void) {}

	get handedOff(): boolean {
		return this.off;
	}

	handOff(): void {
		this.off = true;
	}

	/** The viewer is back on the embedded player. */
	comeBack(): void {
		if (!this.off) return;
		this.off = false;
		const start = this.held;
		this.held = null;
		if (start) start();
		else this.restart();
	}

	/** Runs `start` now, or, while handed off, when the viewer comes
	 *  back; a later held start replaces an earlier one. */
	whenWatching(start: () => void): void {
		if (this.off) this.held = start;
		else start();
	}

	/** The viewer picked another stream, which replaces this one: the
	 *  handoff and what it held are forgotten, and nothing runs. */
	forget(): void {
		this.off = false;
		this.held = null;
	}

	/** The viewer picked another stream from the embedded player (an
	 *  episode, Reload). (Signature only so far.) */
	picked(): void {}

	/** A stream attaches. Whether a pick ended the handoff with it.
	 *  (Signature only so far.) */
	attaching(): boolean {
		return false;
	}

	/** The page started the engine of the attached stream. (Signature
	 *  only so far.) */
	engineStarted(): void {}

	/** Something stopped that engine for good. (Signature only so far.) */
	engineStopped(): void {}

	/** Whether a recovery whose eviction has answered stands down.
	 *  (Signature only so far.) */
	recoveryStandsDown(manualReload: boolean): boolean {
		void manualReload;
		return false;
	}

	/** Forgets `start` if it is the one held: its stream is gone. */
	drop(start: () => void): void {
		if (this.held === start) this.held = null;
	}
}
