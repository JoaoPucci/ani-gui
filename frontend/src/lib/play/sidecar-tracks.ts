/**
 * Sidecar subtitle tracks: the `.vtt` files a provider lists beside
 * a stream, outside the playlist, which nothing in the manifest would
 * ever tell the player about.
 *
 * The play page keeps only the session id across navigation, so the
 * tracks are asked for by id from the proxy — the same origin the
 * media URL is built against — and each becomes one `<track>` on the
 * singleton video, where the browser renders it natively and the
 * captions picker already lists it. The pure parts (the URL, the
 * attribute mapping) are tested on their own; the DOM adapter is
 * exercised by the acceptance test on the mounted page.
 */

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
 */
export async function fetchSidecarTracks(
	apiBase: string,
	sessionId: string,
	signal?: AbortSignal
): Promise<SidecarTrack[]> {
	try {
		const res = await fetch(sidecarTracksUrl(apiBase, sessionId), { signal });
		if (!res.ok) return [];
		const listed: unknown = await res.json();
		return Array.isArray(listed) ? (listed as SidecarTrack[]) : [];
	} catch {
		return [];
	}
}

/** Append one `<track>` per listing; the returned function removes them. */
export function attachSidecarTracks(video: HTMLVideoElement, tracks: SidecarTrack[]): () => void {
	const elements = tracks.map((track) => {
		const attrs = trackAttributes(track);
		const el = document.createElement('track');
		el.setAttribute('kind', attrs.kind);
		el.setAttribute('srclang', attrs.srclang);
		el.setAttribute('label', attrs.label);
		el.setAttribute('src', attrs.src);
		if (attrs.default) el.setAttribute('default', '');
		video.appendChild(el);
		return el;
	});
	return () => {
		for (const el of elements) el.remove();
	};
}

/**
 * Ask for a session's tracks and attach whatever comes back, unless
 * the source moved on first. The returned function cancels the ask
 * and removes any attached tracks — it belongs with the source's
 * other cleanups.
 */
export function armSidecarTracks(
	video: HTMLVideoElement,
	apiBase: string,
	sessionId: string
): () => void {
	const controller = new AbortController();
	let detach: (() => void) | null = null;
	void fetchSidecarTracks(apiBase, sessionId, controller.signal).then((tracks) => {
		if (controller.signal.aborted || tracks.length === 0) return;
		detach = attachSidecarTracks(video, tracks);
	});
	return () => {
		controller.abort();
		detach?.();
	};
}
