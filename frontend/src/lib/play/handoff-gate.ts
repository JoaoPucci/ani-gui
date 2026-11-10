/**
 * Whether the episode on the play page has been handed to another
 * player (the external player, Syncplay), and what waits for the
 * viewer to come back to the embedded one.
 *
 * Once the episode is handed off nothing may start the embedded video
 * again by itself: the page stops the engine loading, and an engine
 * start that comes meanwhile (a stream attaching, whatever attached
 * it) is held here rather than run. The viewer coming back — playing
 * the embedded video, picking an episode, reloading — runs the held
 * start, the latest one only; with none held it restarts what the
 * handoff stopped.
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
	 *  handoff and what it held are forgotten, and nothing runs.
	 *  (Signature only so far.) */
	forget(): void {}

	/** Forgets `start` if it is the one held: its stream is gone. */
	drop(start: () => void): void {
		if (this.held === start) this.held = null;
	}
}
