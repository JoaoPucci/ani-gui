/**
 * Playback held while something asks the viewer a question over the
 * player — the change-show picker. The video is paused while it is
 * open, and plays again when it closes only if it was playing when it
 * opened: a viewer who had paused comes back to a paused video.
 */

/** The part of a media element the hold uses. */
export interface Playable {
	readonly paused: boolean;
	pause(): void;
	play(): Promise<void> | void;
}

/** Pause `media` (when there is one) and return what releases the
 *  hold. Released twice, it plays at most once. */
export function holdPlayback(media: Playable | null | undefined): () => void {
	const wasPlaying = media !== null && media !== undefined && !media.paused;
	if (wasPlaying) media.pause();
	let released = false;
	return () => {
		if (released) return;
		released = true;
		if (wasPlaying && media?.paused) void Promise.resolve(media.play()).catch(() => {});
	};
}
