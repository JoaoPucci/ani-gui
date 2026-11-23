/**
 * The spotlight on a jumped-to episode, on a show's page and in the
 * player: after "Jump to", the target tile is scrolled to the centre
 * and highlighted, and the highlight ends after `SPOTLIGHT_MS`.
 *
 * One call is one run of the page's effect. The effect re-runs when
 * the episode list changes, because the tile may only exist once the
 * right page of episodes has loaded; a run that finds no tile does
 * nothing, and the next one tries again. Only the run that finds the
 * tile starts the timer that ends the highlight.
 *
 * The scroll takes the motion preference's behaviour, a glide with
 * motion on and a jump under reduced motion.
 */
import { scrollBehavior } from '$lib/motion/scroll';

/** How long a found tile stays highlighted. */
export const SPOTLIGHT_MS = 3200;

/** What a run needs from the page; `browserSpotlightHost()` is the real one. */
export interface SpotlightHost {
	findTile(episode: number): Element | null;
	scrollTo(tile: Element): void;
	nextFrame(cb: () => void): number;
	cancelFrame(id: number): void;
	setTimer(cb: () => void, ms: number): number;
	clearTimer(id: number): void;
}

/** The selector for an episode's tile (`<li data-ep-num=…>`). */
export function episodeTileSelector(episode: number): string {
	return `[data-ep-num="${episode}"]`;
}

/** The document and window. Web globals are wrapped in arrows: called
 *  as a method of this object, `setTimeout` throws "Illegal invocation"
 *  in Chromium. */
export function browserSpotlightHost(): SpotlightHost {
	return {
		findTile: (episode) => document.querySelector(episodeTileSelector(episode)),
		scrollTo: (tile) => tile.scrollIntoView({ behavior: scrollBehavior(), block: 'center' }),
		nextFrame: (cb) => requestAnimationFrame(() => cb()),
		cancelFrame: (id) => cancelAnimationFrame(id),
		setTimer: (cb, ms) => window.setTimeout(cb, ms),
		clearTimer: (id) => window.clearTimeout(id)
	};
}

/**
 * Waits a frame for the tile to be laid out, then scrolls to it and
 * calls `onEnd` once `SPOTLIGHT_MS` has passed. Returns the cleanup,
 * which cancels whichever of the two is still pending.
 */
export function spotlightEpisode(
	episode: number,
	onEnd: () => void,
	host: SpotlightHost = browserSpotlightHost()
): () => void {
	let timerId: number | undefined;
	let frameId: number | undefined = host.nextFrame(() => {
		frameId = undefined;
		const tile = host.findTile(episode);
		if (!tile) return;
		host.scrollTo(tile);
		timerId = host.setTimer(() => {
			timerId = undefined;
			onEnd();
		}, SPOTLIGHT_MS);
	});
	return () => {
		if (frameId !== undefined) host.cancelFrame(frameId);
		if (timerId !== undefined) host.clearTimer(timerId);
		frameId = timerId = undefined;
	};
}
