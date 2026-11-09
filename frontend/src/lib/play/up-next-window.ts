/**
 * When the player's "Up next" card shows over a finale: once the video
 * has ended. Through the credits, and any scene or preview after them,
 * the player behaves as on any episode. A seek back puts the card away
 * again, and the end brings it back.
 *
 * "Ended" is the element's own ended state, or a playhead within
 * `END_SLACK_S` of a known length. The slack is a precaution: where the
 * media buffered for a stream played through hls.js ends short of the
 * length the stream declared, the playhead can stop a fraction of a
 * second before `duration` and the element never reports its end, so
 * the card would never come. On a stream that ends normally it costs
 * the card showing up to a second before the last frame.
 */

/** How far short of its length a stream may stop and still count as
 *  ended. */
export const END_SLACK_S = 1;

/** Whether the video has ended: the element says so, or the playhead is
 *  within `END_SLACK_S` of a known length. The play page holds a length
 *  it has not learned as zero, and that decides nothing. */
export function atVideoEnd(currentTime: number, duration: number, ended: boolean): boolean {
	if (ended) return true;
	return Number.isFinite(duration) && duration > 0 && duration - currentTime <= END_SLACK_S;
}

/** What the play page knows when it decides whether the card shows. */
export interface UpNextState {
	/** `finaleCard` found something to name. */
	readonly card: boolean;
	/** The player's own next-episode step is open: not the entry's end. */
	readonly hasNext: boolean;
	readonly resumeHolding: boolean;
	readonly dismissed: boolean;
	/** The <video> itself is fullscreen: nothing else on the page is drawn. */
	readonly videoFullscreen: boolean;
	/** The video has ended (`atVideoEnd`). */
	readonly atEnd: boolean;
}

/** Whether the Up next card shows. */
export function upNextShown(state: UpNextState): boolean {
	return (
		state.card &&
		!state.hasNext &&
		!state.resumeHolding &&
		!state.dismissed &&
		!state.videoFullscreen &&
		state.atEnd
	);
}
