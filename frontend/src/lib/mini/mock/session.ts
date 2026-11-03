/**
 * The mockup's mini-player session: the one playback the simulated
 * window holds, and how it ends.
 *
 * The real window will be a second Electron window that takes the
 * stream over at a handed point (the mini-player proposal, PR #345,
 * section 3). The mockup moves the play page's own video element into
 * a window drawn inside the main one, so the picture and its native
 * subtitles are the real ones, and so that the main window can go
 * back to where the viewer came from while it plays, the page hands
 * the session its own teardown instead of running it on leaving.
 *
 * At most one session: opening another closes the one before.
 * `returnToApp` and `close` end it. Returning gives the video back to
 * a play page still there, or runs the page's teardown once that page
 * is gone. Closing stops playback: paused and given back to a page
 * still there, or torn down.
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
	/** Puts the video back in the page's player; null where the page
	 *  cannot take it back. The session forgets it once the page goes. */
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
	let current: MiniSession | null = null;
	// The held page's restore, until that page leaves.
	let restore: (() => void) | null = null;
	// Callbacks to call, not state anything renders.
	const listeners = new Set<(session: MiniSession | null) => void>();
	const set = (next: MiniSession | null) => {
		current = next;
		for (const listener of [...listeners]) listener(current);
	};
	const end = (): MiniSession | null => {
		const s = current;
		if (s) set(null);
		return s;
	};
	const close = () => {
		const s = end();
		if (!s) return;
		const back = restore;
		restore = null;
		if (back) {
			s.video.pause();
			back();
		} else {
			s.hooks.dispose();
		}
	};
	return {
		get current() {
			return current;
		},
		open(session) {
			if (current && current !== session) close();
			restore = session.hooks.restore;
			set(session);
		},
		holds: (video) => current?.video === video,
		pageGone(video) {
			if (current?.video === video) restore = null;
		},
		returnToApp() {
			const s = end();
			if (!s) return null;
			const back = restore;
			restore = null;
			if (back) {
				back();
				return 'restored';
			}
			s.hooks.dispose();
			return 'navigate';
		},
		close,
		subscribe(listener) {
			listeners.add(listener);
			listener(current);
			return () => {
				listeners.delete(listener);
			};
		}
	};
}
