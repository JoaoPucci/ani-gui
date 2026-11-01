/**
 * Sidecar subtitle tracks: the `.vtt` files a provider lists beside
 * a stream, outside the playlist, which nothing in the manifest would
 * ever tell the player about.
 *
 * The play page keeps only the session id across navigation, so the
 * tracks are asked for by id from the proxy — at the origin the media
 * URL carries, which the backend built on its own address, so the ask
 * reaches the backend whether the preload bridge is present or the
 * base came from the environment — and each becomes one `<track>` on
 * the play page's video, where the browser renders it natively and the
 * captions picker already lists it. The pure parts (the URL, the
 * attribute mapping) are tested on their own; the DOM adapter is
 * exercised by the acceptance test on the mounted page.
 */

import { applyCaptionPick } from './caption-pick';
import { startingTrack } from './caption-start';

/** One listed track, as the proxy answers it. */
export interface SidecarTrack {
	lang: string;
	label: string;
	default: boolean;
	/** A proxy URL — the upstream one never reaches the renderer. */
	url: string;
}

/** The proxy's listing of a session's tracks. */
export function sidecarTracksUrl(apiBase: string, sessionId: string): string {
	const base = apiBase.replace(/\/+$/, '');
	return `${base}/s/${encodeURIComponent(sessionId)}/subtitles`;
}

/** What one listed track sets on its `<track>`. */
export interface TrackAttributes {
	kind: 'subtitles';
	srclang: string;
	label: string;
	src: string;
	default: boolean;
}

export function trackAttributes(track: SidecarTrack): TrackAttributes {
	return {
		kind: 'subtitles',
		srclang: track.lang,
		label: track.label,
		src: track.url,
		default: track.default === true
	};
}

/**
 * The session's tracks, or none: a listing that fails to load is the
 * same as a session without tracks — playback must never wait on it.
 * `mediaUrl` is the session's media URL; the listing lives at its
 * origin.
 */
export async function fetchSidecarTracks(
	mediaUrl: string,
	sessionId: string,
	signal?: AbortSignal
): Promise<SidecarTrack[]> {
	try {
		const origin = new URL(mediaUrl).origin;
		const res = await fetch(sidecarTracksUrl(origin, sessionId), { signal });
		if (!res.ok) return [];
		const listed: unknown = await res.json();
		return Array.isArray(listed) ? (listed as SidecarTrack[]) : [];
	} catch {
		return [];
	}
}

/**
 * Append one `<track>` per listing; the returned function removes them.
 * The track the player starts on ({@link startingTrack}, for the
 * user's `locale`) alone carries the `default` attribute.
 */
export function attachSidecarTracks(
	video: HTMLVideoElement,
	tracks: SidecarTrack[],
	locale = '',
	remembered?: string
): () => void {
	const start = startingTrack(tracks, locale, remembered);
	const elements = tracks.map((track, i) => {
		const attrs = { ...trackAttributes(track), default: i === start };
		const el = document.createElement('track');
		el.setAttribute('kind', attrs.kind);
		el.setAttribute('srclang', attrs.srclang);
		el.setAttribute('label', attrs.label);
		el.setAttribute('src', attrs.src);
		if (attrs.default) el.setAttribute('default', '');
		video.appendChild(el);
		return el;
	});
	applyStart(video, start === null ? null : elements[start]);
	return () => {
		for (const el of elements) el.remove();
	};
}

/**
 * The start applied to the video's tracks, not left to the `default`
 * attribute alone: the browser honours the attribute only while no
 * subtitle track shows, and a track the stream's playlist carries may
 * already be showing. Every subtitle track is turned off — disabled,
 * so none is fetched — and the start's track, if any, turned on.
 */
function applyStart(video: HTMLVideoElement, start: HTMLTrackElement | null): void {
	applyCaptionPick(video.textTracks, 'off');
	if (start) start.track.mode = 'showing';
}

/**
 * Ask for a session's tracks and attach whatever comes back, unless
 * the source moved on first. The returned function cancels the ask
 * and removes any attached tracks — it belongs with the source's
 * other cleanups.
 */
export function armSidecarTracks(
	video: HTMLVideoElement,
	mediaUrl: string,
	sessionId: string,
	locale = '',
	remembered?: string
): () => void {
	const controller = new AbortController();
	let detach: (() => void) | null = null;
	void fetchSidecarTracks(mediaUrl, sessionId, controller.signal).then((tracks) => {
		if (controller.signal.aborted || tracks.length === 0) return;
		detach = attachSidecarTracks(video, tracks, locale, remembered);
	});
	return () => {
		controller.abort();
		detach?.();
	};
}
