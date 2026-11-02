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
import { expectCaptions, showingCaption } from './caption-watch';

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
 * user's `locale`) alone carries the `default` attribute, and is
 * already showing when it joins the video.
 *
 * Chromium runs its own subtitle selection a moment after tracks join
 * a video. It leaves alone any track a script has set since the track
 * joined; of the rest, while a subtitle track shows — or one did when
 * the last change to a joined track was made — it turns on one more,
 * even beside a track already showing. A start switched on after its
 * tracks joined left that second track showing: Arabic over English on
 * a nine-language episode. So:
 *
 * - whatever shows is turned off first — a track the stream's playlist
 *   carries may be — so nothing shows when the new tracks join (hls.js
 *   then switches its own tracks against the start: logged in
 *   docs/deferred-work.md as "Sidecar subtitles on a stream that also
 *   carries its own");
 * - the start's track is switched on before it joins, which the
 *   selection does not count as a script's change, and every other
 *   track joins disabled, never fetched;
 * - the tracks are switched off before they are removed, so the next
 *   session's tracks do not join a video that last showed one — with
 *   no start of its own, that session would get one anyway;
 * - once the start's file is in, the start is set again on the video
 *   ({@link redrawOnLoad}). Chromium draws captions only once a mode
 *   change on a joined track has found one showing, and a start that
 *   joined already showing is changed by nothing after: its cues went
 *   active and none was drawn. The file arrives a fetch after the
 *   tracks joined, by when the selection — a zero-delay task — has
 *   run, and the start, set by a script since joining, is left alone
 *   by any later one.
 */
export function attachSidecarTracks(
	video: HTMLVideoElement,
	tracks: SidecarTrack[],
	locale = '',
	remembered?: string
): () => void {
	const start = startingTrack(tracks, locale, remembered);
	applyCaptionPick(video.textTracks, 'off');
	const elements = tracks.map((track, i) => {
		const attrs = { ...trackAttributes(track), default: i === start };
		const el = document.createElement('track');
		el.setAttribute('kind', attrs.kind);
		el.setAttribute('srclang', attrs.srclang);
		el.setAttribute('label', attrs.label);
		el.setAttribute('src', attrs.src);
		if (attrs.default) {
			el.setAttribute('default', '');
			el.track.mode = 'showing';
		}
		video.appendChild(el);
		return el;
	});
	if (start !== null) redrawOnLoad(video, elements[start]);
	// What the app set, so the browser's own captions menu can be told
	// from it ($lib/play/caption-watch).
	expectCaptions(video, showingCaption(video.textTracks));
	return () => {
		// The tracks going are no pick of the viewer's.
		expectCaptions(video, null);
		for (const el of elements) {
			if (el.track.mode !== 'disabled') el.track.mode = 'disabled';
			el.remove();
		}
	};
}

/**
 * When the start's file is in, switch the start off and on again on
 * the video, so the browser draws it. Only while it is still the one
 * showing on this video: a track the viewer has moved off, or one
 * that has left with its session, is not touched. The mode it ends on
 * is the one the app set, so no caption change is reported as a pick.
 */
function redrawOnLoad(video: HTMLVideoElement, el: HTMLTrackElement): void {
	el.addEventListener(
		'load',
		() => {
			if (el.parentNode !== video || el.track.mode !== 'showing') return;
			el.track.mode = 'hidden';
			el.track.mode = 'showing';
		},
		{ once: true }
	);
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
		// The session going is no pick of the viewer's, whether it had
		// sidecar tracks or not.
		expectCaptions(video, null);
		detach?.();
	};
}
