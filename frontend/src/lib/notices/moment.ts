/**
 * What the window is doing, for the arrival rules (./arrival): whether
 * a video plays and whether something is fullscreen. Read from the
 * document itself — media events and `fullscreenchange` — so the
 * player needs no hook of its own, and a video in picture-in-picture,
 * which stays in the document, counts as playing.
 */
import type { Moment } from './arrival';

/** The part of a video element this reads. */
export interface VideoLike {
	readonly paused: boolean;
	readonly ended: boolean;
}

/** The part of the document this reads. */
export interface MomentDocument {
	readonly fullscreenElement: unknown;
	querySelectorAll(selector: 'video'): ArrayLike<VideoLike>;
	addEventListener(type: string, listener: () => void, capture?: boolean): void;
	removeEventListener(type: string, listener: () => void, capture?: boolean): void;
}

export function momentOf(doc: MomentDocument): Moment {
	const videos = Array.from(doc.querySelectorAll('video'));
	return {
		playing: videos.some((v) => !v.paused && !v.ended),
		fullscreen: doc.fullscreenElement != null
	};
}

/** Media events don't bubble; they are caught on their way down. */
const MEDIA_EVENTS = ['play', 'playing', 'pause', 'ended', 'emptied'] as const;

/** Calls `onChange` with the moment whenever it may have changed, and
 *  once now. Returns the unsubscribe. */
export function watchMoment(doc: MomentDocument, onChange: (m: Moment) => void): () => void {
	const fire = () => onChange(momentOf(doc));
	doc.addEventListener('fullscreenchange', fire);
	for (const e of MEDIA_EVENTS) doc.addEventListener(e, fire, true);
	fire();
	return () => {
		doc.removeEventListener('fullscreenchange', fire);
		for (const e of MEDIA_EVENTS) doc.removeEventListener(e, fire, true);
	};
}
