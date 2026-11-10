import { SrcLetGo } from './src-letgo';

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
	// A pick from the embedded player while handed off, waiting for its
	// stream to attach.
	private pick = false;
	// The attached stream's engine was started by the page, and nothing
	// has stopped it for good since: the only engine a return restarts.
	private engine = false;
	// The attached stream's manifest is in. A handoff's stopLoad() before
	// it is cuts it off for good (hls.js destroys the playlist loaders),
	// so a return then asks for the stream again. Before any stream it
	// reads as in: there is nothing to ask for. A stream with no engine
	// (an mp4) never reports one, and the page's reload does nothing for
	// it.
	private manifest = true;
	// A stream the element plays from its src, let go of at the handoff,
	// and the playhead it was at ($lib/play/src-letgo).
	private readonly src = new SrcLetGo();

	/** `restart` resumes what the handoff stopped, when no start is
	 *  held. `reload` asks again for a stream whose manifest the handoff
	 *  cut off. */
	constructor(
		private readonly restart: () => void,
		private readonly reload: () => void = () => {}
	) {}

	/** The attached stream's manifest is in. */
	manifestParsed(): void {
		this.manifest = true;
	}

	get handedOff(): boolean {
		return this.off;
	}

	handOff(): void {
		this.off = true;
		this.pick = false;
	}

	/** The viewer is back on the embedded player. Answers the playhead
	 *  a stream let go of is to resume at, if one was let go of. */
	comeBack(): number | null {
		const point = this.src.take();
		this.pick = false;
		if (!this.off) return point;
		this.off = false;
		const start = this.held;
		this.held = null;
		if (start) start();
		else if (this.engine) this.restart();
		else if (!this.manifest) this.reload();
		return point;
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
		this.pick = false;
	}

	/** The viewer picked another stream from the embedded player (an
	 *  episode, Reload): the handoff ends as that stream attaches, and
	 *  the stream it replaces is not started again meanwhile. A later
	 *  handoff, or the viewer playing again, cancels a pick that never
	 *  attached. */
	picked(): void {
		if (this.off) this.pick = true;
	}

	/** A stream attaches: its engine is a new one, not started yet. True
	 *  when a pick ended the handoff with it (the element plays on its
	 *  own again). */
	attaching(): boolean {
		this.engine = false;
		this.manifest = false;
		if (!this.pick) return false;
		this.forget();
		return true;
	}

	/** The page started the engine of the attached stream. */
	engineStarted(): void {
		this.engine = true;
	}

	/** Something stopped that engine for good (the fragment-loop guard). */
	engineStopped(): void {
		this.engine = false;
	}

	/** Whether a recovery whose eviction has answered stands down:
	 *  while handed off, resolving again would only load the episode
	 *  behind the other player, unless it is a Reload the viewer still
	 *  means (no handoff since it was pressed). */
	recoveryStandsDown(manualReload: boolean): boolean {
		return this.off && !(manualReload && this.pick);
	}

	/** A stream the element plays from its src is let go of, at
	 *  `playhead`; a playhead at zero (before the first frame) gives
	 *  nothing back. */
	letGo(url: string, playhead: number): void {
		this.src.letGo(url, playhead);
	}

	/** Whether an attach of `url` waits for the viewer: it is the stream
	 *  let go of. Another stream (a pick, a Reload's fresh session) ends
	 *  the wait and forgets the playhead. */
	attachWaits(url: string): boolean {
		return this.src.attachWaits(url);
	}

	/** The point a Reload resumes at: the playhead a stream was let go
	 *  of at, in place of the unloaded element's zero. */
	reloadPoint(current: number): number {
		return this.src.reloadPoint(current);
	}

	/** Forgets `start` if it is the one held: its stream is gone. */
	drop(start: () => void): void {
		if (this.held === start) this.held = null;
	}
}
