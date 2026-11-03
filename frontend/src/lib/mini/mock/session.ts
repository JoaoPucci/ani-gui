/**
 * The mockup's mini-player session: the one playback the simulated
 * window holds, and how it ends.
 *
 * The real window will be a second Electron window that takes the
 * stream over at a handed point (docs/proposals/mini-player.md,
 * section 3). The mockup moves the play page's own video element into
 * a window drawn inside the main one, so the picture and its native
 * subtitles are the real ones, and so that the main window can go
 * back to where the viewer came from while it plays, the page hands
 * the session its own teardown instead of running it on leaving.
 *
 * At most one session: opening another ends the one before with its
 * own teardown. `returnToApp` and `close` end it; the first restores
 * the page's player, the second stops playback.
 */

export interface MiniShow {
	id: string;
	title: string;
	episode: number;
	accent: string;
	/** Where "Back to the app" goes when the play page is gone. */
	playHref: string;
}

export interface MiniSessionHooks {
	/** The page's own teardown: stops the engine, releases the video. */
	dispose(): void;
	/** Puts the video back in the page's player; null once the page is gone. */
	restore: (() => void) | null;
}

export interface MiniSession {
	video: HTMLVideoElement;
	show: MiniShow;
	hooks: MiniSessionHooks;
}

export interface MiniSessions {
	readonly current: MiniSession | null;
	open(session: MiniSession): void;
	holds(video: HTMLVideoElement): boolean;
	/** The play page is leaving: its restore no longer applies. */
	pageGone(video: HTMLVideoElement): void;
	/** Ends the session; says whether the page took the video back
	 *  ('restored') or the caller has to navigate to `playHref`. */
	returnToApp(): 'restored' | 'navigate' | null;
	close(): void;
	subscribe(listener: (session: MiniSession | null) => void): () => void;
}

export function createMiniSessions(): MiniSessions {
	return {
		current: null,
		open() {},
		holds: () => false,
		pageGone() {},
		returnToApp: () => null,
		close() {},
		subscribe: () => () => {}
	};
}
